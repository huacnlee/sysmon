//! Read-only GPU busy percentages exposed by a platform driver.
//!
//! Multiple devices report independently: the headline is the busiest GPU,
//! never a sum of unrelated engine percentages. Missing counters stay missing.

/// Driver-reported utilization of the busiest visible GPU, in 0..=100 percent.
pub fn collect() -> Option<f32> {
    platform_collect()
}

#[cfg(any(target_os = "macos", target_os = "linux", test))]
fn percent(value: f64) -> Option<f32> {
    (value.is_finite() && (0. ..=100.).contains(&value)).then_some(value as f32)
}

#[cfg(target_os = "macos")]
fn platform_collect() -> Option<f32> {
    macos::collect()
}

#[cfg(target_os = "linux")]
fn platform_collect() -> Option<f32> {
    let entries = std::fs::read_dir("/sys/class/drm").ok()?;
    entries
        .flatten()
        .filter(|entry| {
            entry.file_name().to_str().is_some_and(|name| {
                name.strip_prefix("card").is_some_and(|suffix| {
                    !suffix.is_empty() && suffix.bytes().all(|v| v.is_ascii_digit())
                })
            })
        })
        .filter_map(|entry| {
            std::fs::read_to_string(entry.path().join("device/gpu_busy_percent"))
                .ok()
                .and_then(|value| parse_percent(&value))
        })
        .max_by(f32::total_cmp)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_collect() -> Option<f32> {
    None
}

#[cfg(any(target_os = "linux", test))]
fn parse_percent(value: &str) -> Option<f32> {
    percent(value.trim().parse().ok()?)
}

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        ffi::{CStr, c_char, c_void},
        ptr,
    };

    type CfRef = *const c_void;
    type IoObject = u32;
    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOServiceMatching(name: *const c_char) -> *mut c_void;
        fn IOServiceGetMatchingServices(
            port: u32,
            matching: *mut c_void,
            iter: *mut IoObject,
        ) -> i32;
        fn IOIteratorNext(iter: IoObject) -> IoObject;
        fn IOObjectRelease(object: IoObject) -> i32;
        fn IORegistryEntryCreateCFProperty(
            entry: IoObject,
            key: CfRef,
            allocator: CfRef,
            options: u32,
        ) -> CfRef;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(
            allocator: CfRef,
            value: *const c_char,
            encoding: u32,
        ) -> CfRef;
        fn CFRelease(value: CfRef);
        fn CFGetTypeID(value: CfRef) -> usize;
        fn CFDictionaryGetTypeID() -> usize;
        fn CFNumberGetTypeID() -> usize;
        fn CFDictionaryGetValue(dictionary: CfRef, key: CfRef) -> CfRef;
        fn CFNumberGetValue(number: CfRef, kind: isize, value: *mut c_void) -> bool;
    }
    struct OwnedCf(CfRef);
    impl OwnedCf {
        fn string(value: &CStr) -> Option<Self> {
            // SAFETY: the input is NUL terminated; UTF-8 is a public CF encoding.
            let value =
                unsafe { CFStringCreateWithCString(ptr::null(), value.as_ptr(), 0x08000100) };
            (!value.is_null()).then_some(Self(value))
        }
    }
    impl Drop for OwnedCf {
        fn drop(&mut self) {
            // SAFETY: each wrapper owns exactly one CF Create-rule reference.
            unsafe { CFRelease(self.0) };
        }
    }
    struct OwnedIo(IoObject);
    impl Drop for OwnedIo {
        fn drop(&mut self) {
            // SAFETY: iterator and service handles are retained by IOKit's API.
            unsafe { IOObjectRelease(self.0) };
        }
    }
    fn dictionary_percent(dictionary: CfRef, key: &CStr) -> Option<f32> {
        let key = OwnedCf::string(key)?;
        // SAFETY: collect verifies the dictionary type and retains it throughout
        // this call. CFDictionaryGetValue is borrowed; never release that value.
        let number = unsafe { CFDictionaryGetValue(dictionary, key.0) };
        if number.is_null() || unsafe { CFGetTypeID(number) != CFNumberGetTypeID() } {
            return None;
        }
        let mut value = 0_f64;
        // SAFETY: 13 is kCFNumberDoubleType; the destination is a writable f64.
        if unsafe { CFNumberGetValue(number, 13, (&mut value as *mut f64).cast()) } {
            super::percent(value)
        } else {
            None
        }
    }
    pub(super) fn collect() -> Option<f32> {
        let key = OwnedCf::string(c"PerformanceStatistics")?;
        // SAFETY: IOServiceMatching creates a CF dictionary and the matching
        // call consumes that reference on success and failure. Port 0 selects
        // the default IOKit main port, without privilege elevation.
        let matching = unsafe { IOServiceMatching(c"IOAccelerator".as_ptr()) };
        if matching.is_null() {
            return None;
        }
        let mut iterator = 0;
        if unsafe { IOServiceGetMatchingServices(0, matching, &mut iterator) } != 0 || iterator == 0
        {
            return None;
        }
        let iterator = OwnedIo(iterator);
        let mut maximum: Option<f32> = None;
        loop {
            // SAFETY: the live iterator yields independently retained services.
            let entry = unsafe { IOIteratorNext(iterator.0) };
            if entry == 0 {
                break;
            }
            let entry = OwnedIo(entry);
            // SAFETY: entry and key are alive, default allocator and options.
            let statistics =
                unsafe { IORegistryEntryCreateCFProperty(entry.0, key.0, ptr::null(), 0) };
            if statistics.is_null() {
                continue;
            }
            let statistics = OwnedCf(statistics);
            if unsafe { CFGetTypeID(statistics.0) != CFDictionaryGetTypeID() } {
                continue;
            }
            // Prefer whole-device counters. Tiler and renderer counters are
            // individual engines and cannot substitute for device utilization.
            let value = [
                c"Device Utilization %",
                c"GPU Core Utilization %",
                c"GPU Activity(%)",
            ]
            .into_iter()
            .find_map(|key| dictionary_percent(statistics.0, key));
            if let Some(value) = value {
                maximum = Some(maximum.map_or(value, |previous| previous.max(value)));
            }
        }
        maximum
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gpu_percent_preserves_idle_and_rejects_driver_sentinels() {
        assert_eq!(parse_percent(" 0\n"), Some(0.));
        assert_eq!(parse_percent("100"), Some(100.));
        assert_eq!(parse_percent("17.5"), Some(17.5));
        for invalid in ["", "-1", "101", "NaN", "inf", "unavailable", "17%"] {
            assert_eq!(parse_percent(invalid), None, "{invalid}");
        }
    }
}
