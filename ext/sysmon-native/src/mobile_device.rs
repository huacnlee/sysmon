//! Public, read-only mobile device readings. Missing telemetry stays missing.
use crate::providers::Battery;

#[cfg(any(target_os = "ios", target_os = "android"))]
#[derive(Default)]
pub(crate) struct DeviceReadings {
    pub battery: Option<Battery>,
    pub battery_status: String,
    pub sound: Option<String>,
    pub model: Option<String>,
    pub os_version: Option<String>,
}

#[cfg(target_os = "ios")]
pub(crate) fn collect() -> DeviceReadings {
    // The collector runs on a worker. UIDevice must only be accessed on the
    // UIKit main thread; return owned values so no UIKit objects cross threads.
    dispatch2::run_on_main(|mtm| {
        use objc2_ui_kit::{UIDevice, UIDeviceBatteryState};
        let device = UIDevice::currentDevice(mtm);
        device.setBatteryMonitoringEnabled(true);
        let level = device.batteryLevel();
        let state = device.batteryState();
        let status = match state {
            UIDeviceBatteryState::Charging => "Charging",
            UIDeviceBatteryState::Full => "Fully charged",
            UIDeviceBatteryState::Unplugged => "On battery",
            _ => "Charge state unavailable",
        };
        let battery = battery_reading(level as f64 * 100., status);
        DeviceReadings {
            battery_status: if battery.is_some() {
                "Battery level and charge state reported by iOS. Health, cycle count, power draw and time remaining are not exposed to this app.".into()
            } else {
                "iOS has not reported a battery level. Simulators do not expose a physical battery; on a device, try refreshing after monitoring starts.".into()
            },
            battery,
            sound: ios_audio_output(),
            model: Some(device.model().to_string()),
            os_version: Some(format!(
                "{} {}",
                device.systemName(),
                device.systemVersion()
            )),
        }
    })
}

#[cfg(target_os = "ios")]
fn ios_audio_output() -> Option<String> {
    use objc2_avf_audio::AVAudioSession;
    // SAFETY: called only inside collect's main-thread closure. Read the app's
    // current route without activating a session or changing other audio.
    let names: Vec<String> = unsafe {
        let outputs = AVAudioSession::sharedInstance().currentRoute().outputs();
        (0..outputs.count())
            .map(|index| outputs.objectAtIndex(index).portName().to_string())
            .filter(|name| !name.is_empty())
            .collect()
    };
    (!names.is_empty()).then(|| names.join(" · "))
}

#[cfg(target_os = "android")]
pub(crate) fn collect() -> DeviceReadings {
    let battery = android_battery(std::path::Path::new("/sys/class/power_supply"));
    let model = property("ro.product.model");
    let manufacturer = property("ro.product.manufacturer");
    DeviceReadings {
        battery_status: if battery.is_some() {
            "Battery level and charge state reported by the accessible system power supply. Other battery fields are not collected.".into()
        } else {
            "Android does not expose battery telemetry through the accessible system power supply on this device. Battery health, cycle count and power draw are unavailable.".into()
        },
        battery,
        sound: None,
        model: match (manufacturer, model) {
            (Some(make), Some(model))
                if !model.to_lowercase().starts_with(&make.to_lowercase()) =>
            {
                Some(format!("{make} {model}"))
            }
            (_, model) => model,
        },
        os_version: property("ro.build.version.release")
            .map(|version| format!("Android {version}")),
    }
}

#[cfg(target_os = "android")]
fn property(name: &str) -> Option<String> {
    use std::ffi::{CStr, CString, c_char};
    unsafe extern "C" {
        fn __system_property_get(name: *const c_char, value: *mut c_char) -> i32;
    }
    let name = CString::new(name).ok()?;
    // Bionic's public system property API requires PROP_VALUE_MAX (92 bytes).
    let mut value = [0 as c_char; 92];
    // SAFETY: name is NUL terminated and value has PROP_VALUE_MAX writable bytes.
    let length = unsafe { __system_property_get(name.as_ptr(), value.as_mut_ptr()) };
    if length <= 0 {
        return None;
    }
    // SAFETY: __system_property_get writes a terminated string on success.
    Some(
        unsafe { CStr::from_ptr(value.as_ptr()) }
            .to_string_lossy()
            .into_owned(),
    )
}

#[cfg(any(target_os = "android", test))]
fn android_battery(root: &std::path::Path) -> Option<Battery> {
    let supplies = std::fs::read_dir(root).ok()?;
    for entry in supplies.flatten() {
        let path = entry.path();
        let kind = std::fs::read_to_string(path.join("type")).unwrap_or_default();
        if !kind.trim().eq_ignore_ascii_case("battery") {
            continue;
        }
        let capacity = match std::fs::read_to_string(path.join("capacity"))
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
        {
            Some(capacity) => capacity,
            None => continue,
        };
        let raw_status = std::fs::read_to_string(path.join("status")).unwrap_or_default();
        let status = match raw_status.trim().to_ascii_lowercase().as_str() {
            "charging" => "Charging",
            "discharging" => "On battery",
            "full" => "Fully charged",
            "not charging" => "Connected · not charging",
            _ => "Charge state unavailable",
        };
        if let Some(battery) = battery_reading(capacity, status) {
            return Some(battery);
        }
    }
    None
}

#[cfg(any(target_os = "ios", target_os = "android", test))]
fn battery_reading(percent: f64, status: &str) -> Option<Battery> {
    if !percent.is_finite() || !(0. ..=100.).contains(&percent) {
        return None;
    }
    Some(Battery {
        percent: percent as f32,
        status: status.into(),
        ..Battery::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_unknown_sentinels_and_nonfinite_values_stay_unavailable() {
        for value in [-100., -1., 101., f64::NAN, f64::INFINITY] {
            assert!(battery_reading(value, "Unknown").is_none());
        }
        for value in [0., 41.5, 100.] {
            let battery = battery_reading(value, "Charging").unwrap();
            assert_eq!(battery.percent, value as f32);
            assert!(battery.health.is_none());
            assert!(battery.power_w.is_none());
        }
    }

    #[test]
    fn android_selects_battery_supply_and_handles_missing_or_invalid_fields() {
        let root = std::env::temp_dir().join(format!("sysmon-battery-{}", std::process::id()));
        std::fs::create_dir_all(root.join("usb")).unwrap();
        std::fs::create_dir_all(root.join("battery")).unwrap();
        std::fs::write(root.join("usb/type"), "USB\n").unwrap();
        std::fs::write(root.join("usb/capacity"), "100\n").unwrap();
        std::fs::write(root.join("battery/type"), "Battery\n").unwrap();
        assert!(android_battery(&root).is_none());
        std::fs::write(root.join("battery/capacity"), "-1\n").unwrap();
        assert!(android_battery(&root).is_none());
        std::fs::write(root.join("battery/capacity"), "0\n").unwrap();
        let battery = android_battery(&root).unwrap();
        assert_eq!(battery.percent, 0.);
        assert_eq!(battery.status, "Charge state unavailable");
        std::fs::write(root.join("battery/status"), "Charging\n").unwrap();
        assert_eq!(android_battery(&root).unwrap().status, "Charging");
        std::fs::remove_dir_all(root).unwrap();
    }
}
