//! Optional, read-only device telemetry. Call `collect` on a background worker.
//! Missing commands, denied access, disconnected engines and unsupported platforms
//! remain unavailable rather than becoming fabricated zero values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Battery {
    pub percent: f32,
    pub status: String,
    pub remaining: Option<String>,
    pub health: Option<f32>,
    pub cycles: Option<u32>,
    pub power_w: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Container {
    pub name: String,
    pub image: String,
    pub memory: String,
    pub cpu: String,
    pub ports: String,
    pub project: String,
    pub status: String,
    pub memory_used: u64,
    pub memory_limit: u64,
    pub cpu_percent: f32,
    pub network_rx: u64,
    pub network_tx: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extras {
    pub battery: Option<Battery>,
    pub battery_status: String,
    pub device_model: Option<String>,
    pub os_version: Option<String>,
    pub gpu: Option<String>,
    pub sound: Option<String>,
    pub containers: Vec<Container>,
    pub container_status: String,
    pub bluetooth: Vec<String>,
    pub bluetooth_status: String,
    pub listening_ports: Vec<(u32, String)>,
}

/// Optional telemetry is deliberately separate from the fast system sampler.
/// Desktop commands have bounded execution times; mobile platforms do not launch
/// subprocesses and report their capability limit explicitly.
pub fn collect() -> Extras {
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let (containers, container_status) = containers();
        let (bluetooth, bluetooth_status) = bluetooth();
        Extras {
            battery: battery(),
            gpu: gpu(),
            sound: sound(),
            containers,
            container_status,
            bluetooth,
            bluetooth_status,
            listening_ports: listening_ports(),
            ..Extras::default()
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let device = crate::mobile_device::collect();
        Extras {
            battery: device.battery,
            battery_status: device.battery_status,
            sound: device.sound,
            device_model: device.model,
            os_version: device.os_version,
            container_status: "Local Docker telemetry is unavailable on this platform.".into(),
            bluetooth_status:
                "Bluetooth device access requires a platform permission provider on this device."
                    .into(),
            ..Extras::default()
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[derive(Debug)]
enum CommandError {
    Missing,
    TimedOut,
    Failed,
}

/// A separate Unix process group lets cancellation stop descendants which still
/// hold inherited pipes open, rather than only terminating the direct child.
#[cfg(any(target_os = "macos", target_os = "linux"))]
struct ProcessTree(u32);

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        // process_group(0) below makes the child's PID its process-group ID.
        // A negative PID targets the whole group. SIGKILL is 9 on both systems.
        unsafe {
            let _ = kill(-(self.0 as i32), 9);
        }
    }
}

/// Windows jobs include child processes automatically. Closing our last job
/// handle kills the contained tree; failure to establish containment aborts the
/// query instead of allowing an uncontained provider process to continue.
#[cfg(target_os = "windows")]
struct ProcessTree(*mut std::ffi::c_void);

#[cfg(target_os = "windows")]
impl ProcessTree {
    fn new(child: &std::process::Child) -> Result<Self, CommandError> {
        use std::{ffi::c_void, os::windows::io::AsRawHandle};
        #[repr(C)]
        #[derive(Default)]
        struct BasicLimit {
            process_time: i64,
            job_time: i64,
            flags: u32,
            min_working_set: usize,
            max_working_set: usize,
            active_process_limit: u32,
            affinity: usize,
            priority_class: u32,
            scheduling_class: u32,
        }
        #[repr(C)]
        #[derive(Default)]
        struct IoCounters {
            read_ops: u64,
            write_ops: u64,
            other_ops: u64,
            read_bytes: u64,
            write_bytes: u64,
            other_bytes: u64,
        }
        #[repr(C)]
        #[derive(Default)]
        struct ExtendedLimit {
            basic: BasicLimit,
            io: IoCounters,
            process_memory: usize,
            job_memory: usize,
            peak_process_memory: usize,
            peak_job_memory: usize,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> *mut c_void;
            fn SetInformationJobObject(
                job: *mut c_void,
                class: i32,
                info: *const c_void,
                len: u32,
            ) -> i32;
            fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
        }
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(CommandError::Failed);
        }
        let guard = Self(job);
        let mut limit = ExtendedLimit::default();
        limit.basic.flags = 0x2000; // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        let configured = unsafe {
            SetInformationJobObject(
                job,
                9,
                &limit as *const _ as *const c_void,
                std::mem::size_of::<ExtendedLimit>() as u32,
            )
        };
        if configured == 0 || unsafe { AssignProcessToJobObject(job, child.as_raw_handle()) } == 0 {
            return Err(CommandError::Failed);
        }
        Ok(guard)
    }
}

#[cfg(target_os = "windows")]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        }
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
static LIVE_READERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Drain both pipes while waiting: even a verbose external tool cannot deadlock
/// the timeout loop by filling its stdout/stderr pipe. Retain at most 2 MiB.
/// Process-tree cleanup happens before both reader threads are joined, on every
/// exit path, so a background grandchild cannot keep the readers alive.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn command(
    program: &str,
    args: &[&str],
    timeout: std::time::Duration,
) -> Result<String, CommandError> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        thread,
        time::Instant,
    };
    fn reader(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<Vec<u8>> {
        thread::spawn(move || {
            #[cfg(test)]
            struct ReaderGuard;
            #[cfg(test)]
            impl Drop for ReaderGuard {
                fn drop(&mut self) {
                    LIVE_READERS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                }
            }
            #[cfg(test)]
            let _guard = {
                LIVE_READERS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                ReaderGuard
            };
            let mut result = Vec::new();
            let mut block = [0_u8; 8192];
            loop {
                match pipe.read(&mut block) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let keep = n.min((2 * 1024 * 1024_usize).saturating_sub(result.len()));
                        result.extend_from_slice(&block[..keep]);
                    }
                }
            }
            result
        })
    }
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            CommandError::Missing
        } else {
            CommandError::Failed
        }
    })?;
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let tree = ProcessTree(child.id());
    #[cfg(target_os = "windows")]
    let tree = match ProcessTree::new(&child) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    // Both pipes were requested above and are present after a successful spawn.
    let stdout = reader(child.stdout.take().expect("piped stdout"));
    let stderr = reader(child.stderr.take().expect("piped stderr"));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(std::time::Duration::from_millis(10))
            }
            Ok(None) => break Err(CommandError::TimedOut),
            Err(_) => break Err(CommandError::Failed),
        }
    };
    // This also clears descendants left behind by a parent which exited normally.
    // Closing Windows jobs and SIGKILLing Unix groups close inherited pipe ends.
    drop(tree);
    let _ = child.kill();
    let _ = child.wait();
    let bytes = stdout.join().map_err(|_| CommandError::Failed);
    let _ = stderr.join();
    if !status?.success() {
        return Err(CommandError::Failed);
    }
    Ok(String::from_utf8_lossy(&bytes?).trim().to_owned())
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn query(program: &str, args: &[&str]) -> Option<String> {
    command(program, args, std::time::Duration::from_secs(3))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

#[cfg(target_os = "macos")]
fn battery() -> Option<Battery> {
    let mut battery = parse_pmset(&query("/usr/bin/pmset", &["-g", "batt"])?)?;
    if let Some(power) = query(
        "/usr/sbin/system_profiler",
        &["SPPowerDataType", "-detailLevel", "mini"],
    ) {
        battery.health = power
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("Maximum Capacity:")
                    .and_then(|s| s.trim().trim_end_matches('%').parse::<f32>().ok())
            })
            .filter(|v| (0.0..=100.0).contains(v));
        battery.cycles = power.lines().find_map(|line| {
            line.trim()
                .strip_prefix("Cycle Count:")
                .and_then(|s| s.trim().parse().ok())
        });
    }
    if let Some(raw) = query("/usr/sbin/ioreg", &["-r", "-c", "AppleSmartBattery"]) {
        let signed_current = ioreg_number(&raw, "InstantAmperage").map(|v| v as i64);
        battery.power_w = signed_current
            .zip(ioreg_number(&raw, "Voltage"))
            .map(|(ma, mv)| (ma as f64 * mv as f64 / 1e6).abs() as f32)
            .filter(|v| v.is_finite());
    }
    Some(battery)
}

#[cfg(any(target_os = "macos", test))]
fn parse_pmset(output: &str) -> Option<Battery> {
    let line = output.lines().find(|l| l.contains('%'))?;
    let percent_end = line.find('%')?;
    let digits: String = line[..percent_end]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let percent = digits
        .parse::<f32>()
        .ok()
        .filter(|p| (0.0..=100.0).contains(p))?;
    let status = line[percent_end + 1..]
        .split(';')
        .map(str::trim)
        .find(|s| !s.is_empty())
        .unwrap_or("Unknown")
        .to_owned();
    let remaining = line[percent_end + 1..]
        .split(';')
        .map(str::trim)
        .find(|s| s.contains("remaining") || s.contains("until charged"))
        .and_then(|s| {
            let time = s.split_whitespace().next()?;
            let (hours, minutes) = time.split_once(':')?;
            let hours = hours.parse::<u32>().ok()?;
            let minutes = minutes.parse::<u32>().ok().filter(|m| *m < 60)?;
            Some(format!("{hours}h {minutes}m"))
        });
    Some(Battery {
        percent,
        status,
        remaining,
        ..Battery::default()
    })
}

#[cfg(target_os = "linux")]
fn battery() -> Option<Battery> {
    // `type` identifies batteries without assuming a BAT0 device name.
    let devices = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for device in devices.flatten() {
        let path = device.path();
        if std::fs::read_to_string(path.join("type"))
            .ok()
            .as_deref()
            .map(str::trim)
            != Some("Battery")
        {
            continue;
        }
        let percent = std::fs::read_to_string(path.join("capacity"))
            .ok()?
            .trim()
            .parse::<f32>()
            .ok()?;
        if !(0.0..=100.0).contains(&percent) {
            continue;
        }
        let status = std::fs::read_to_string(path.join("status"))
            .unwrap_or_else(|_| "Unknown".into())
            .trim()
            .to_owned();
        let read = |name: &str| {
            std::fs::read_to_string(path.join(name))
                .ok()?
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && *v >= 0.0)
        };
        let full = read("energy_full")
            .zip(read("energy_full_design"))
            .or_else(|| read("charge_full").zip(read("charge_full_design")));
        let health = full.and_then(|(full, design)| {
            if design > 0.0 {
                Some((full / design * 100.0) as f32)
            } else {
                None
            }
        });
        let cycles = read("cycle_count").map(|v| v as u32);
        let power_w = read("power_now").map(|v| (v / 1e6) as f32).or_else(|| {
            read("current_now")
                .zip(read("voltage_now"))
                .map(|(a, v)| (a * v / 1e12) as f32)
        });
        let energy = read("energy_now")
            .zip(read("power_now"))
            .or_else(|| read("charge_now").zip(read("current_now")));
        let remaining = if status.eq_ignore_ascii_case("Discharging") {
            energy
                .filter(|(_, rate)| *rate > 0.0)
                .map(|(now, rate)| format_hours(now / rate))
        } else {
            None
        };
        return Some(Battery {
            percent,
            status,
            remaining,
            health,
            cycles,
            power_w,
        });
    }
    None
}

#[cfg(target_os = "windows")]
fn battery() -> Option<Battery> {
    let output = powershell(
        "Get-CimInstance Win32_Battery | Select-Object -First 1 | ForEach-Object { '{0}|{1}' -f $_.EstimatedChargeRemaining,$_.BatteryStatus }",
    )?;
    let (percent, code) = output.trim().split_once('|')?;
    let percent = percent
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|p| (0.0..=100.0).contains(p))?;
    let status = match code.trim() {
        "1" => "Discharging",
        "2" => "AC power",
        "3" => "Fully charged",
        "6" | "7" | "8" | "9" => "Charging",
        _ => "Unknown",
    }
    .to_owned();
    Some(Battery {
        percent,
        status,
        ..Battery::default()
    })
}

#[cfg(target_os = "windows")]
fn powershell(script: &str) -> Option<String> {
    query(
        "powershell.exe",
        &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ],
    )
}

#[cfg(target_os = "macos")]
fn gpu() -> Option<String> {
    let output = query(
        "/usr/sbin/system_profiler",
        &["SPDisplaysDataType", "-detailLevel", "mini"],
    )?;
    output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Chipset Model:")
            .map(|s| s.trim().to_owned())
    })
}

#[cfg(target_os = "linux")]
fn gpu() -> Option<String> {
    let output = query("lspci", &["-mm"])?;
    output
        .lines()
        .find(|line| {
            line.contains("VGA compatible")
                || line.contains("3D controller")
                || line.contains("Display controller")
        })
        .map(|s| {
            s.split('"')
                .filter(|x| !x.trim().is_empty())
                .skip(3)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.trim().is_empty())
}

#[cfg(target_os = "windows")]
fn gpu() -> Option<String> {
    powershell("Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name")
}

#[cfg(target_os = "macos")]
fn sound() -> Option<String> {
    // Reading volume is non-invasive; do not request microphone access.
    let output = query(
        "/usr/bin/osascript",
        &["-e", "output volume of (get volume settings)"],
    )?;
    let volume = output.parse::<u8>().ok().filter(|v| *v <= 100)?;
    let muted = query(
        "/usr/bin/osascript",
        &["-e", "output muted of (get volume settings)"],
    )?;
    Some(if muted == "true" {
        "System output · Muted".into()
    } else {
        format!("System output · {volume}%")
    })
}

#[cfg(target_os = "linux")]
fn sound() -> Option<String> {
    query("pactl", &["get-default-sink"])
}

#[cfg(target_os = "windows")]
fn sound() -> Option<String> {
    powershell(
        "Get-CimInstance Win32_SoundDevice | Where-Object Status -eq 'OK' | Select-Object -ExpandProperty Name",
    )
}

#[cfg(target_os = "macos")]
fn bluetooth() -> (Vec<String>, String) {
    match command(
        "/usr/sbin/system_profiler",
        &["SPBluetoothDataType", "-detailLevel", "mini"],
        std::time::Duration::from_secs(3),
    ) {
        Ok(output) => parse_bluetooth_macos(&output),
        Err(CommandError::TimedOut) => (
            vec![],
            "Bluetooth report did not respond within 3 seconds.".into(),
        ),
        Err(_) => (
            vec![],
            "Bluetooth device information is unavailable. Check Bluetooth access permissions."
                .into(),
        ),
    }
}

#[cfg(any(target_os = "macos", test))]
fn parse_bluetooth_macos(output: &str) -> (Vec<String>, String) {
    let state = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("State:").map(str::trim));
    let mut section: Option<(&str, usize)> = None;
    let mut devices = Vec::new();
    let mut device_indent = None;
    for line in output.lines() {
        let indent = line.len() - line.trim_start().len();
        let content = line.trim();
        if content == "Connected:" || content == "Not Connected:" {
            section = Some((
                if content == "Connected:" {
                    "Connected"
                } else {
                    "Not connected"
                },
                indent,
            ));
            device_indent = None;
            continue;
        }
        if content.is_empty() {
            continue;
        }
        if let Some((connection, section_indent)) = section {
            if indent <= section_indent {
                section = None;
                continue;
            }
            let expected_indent = *device_indent.get_or_insert(indent);
            if indent == expected_indent && content.ends_with(':') {
                let name = content.trim_end_matches(':').trim();
                if !name.is_empty() {
                    devices.push(format!("{name} · {connection}"));
                }
            }
        }
    }
    let status = match state {
        Some("Off") => "Bluetooth is off.".to_owned(),
        Some(s) => format!("Bluetooth {s} · {} reported devices", devices.len()),
        None => "Bluetooth device details are unavailable. Access may be restricted.".into(),
    };
    (devices, status)
}

#[cfg(target_os = "linux")]
fn bluetooth() -> (Vec<String>, String) {
    let timeout = std::time::Duration::from_secs(3);
    // Prefer a filtered read. Older BlueZ versions do not accept the filter;
    // their fallback list must not be described as a list of connected devices.
    match command("bluetoothctl", &["devices", "Connected"], timeout) {
        Ok(output) if !bluetooth_error(&output) => {
            let devices = parse_bluez_devices(&output);
            return (devices, "Connected Bluetooth devices · BlueZ".into());
        }
        Err(CommandError::Missing) => {
            return (
                vec![],
                "Bluetooth device information requires bluetoothctl (BlueZ).".into(),
            );
        }
        _ => {}
    }
    match command("bluetoothctl", &["devices"], timeout) {
        Ok(output) if !bluetooth_error(&output) => (parse_bluez_devices(&output), "Known Bluetooth devices · connection state unavailable".into()),
        Err(CommandError::TimedOut) => (vec![], "Bluetooth service did not respond within 3 seconds.".into()),
        _ => (vec![], "Bluetooth device access is unavailable. Check permissions, the Bluetooth adapter and the BlueZ service.".into()),
    }
}

#[cfg(target_os = "linux")]
fn bluetooth_error(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    [
        "no default controller",
        "not available",
        "invalid command",
        "invalid argument",
        "failed",
        "error",
        "usage:",
    ]
    .iter()
    .any(|s| lower.contains(s))
}

#[cfg(any(target_os = "linux", test))]
fn parse_bluez_devices(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let device = line.trim().strip_prefix("Device ")?;
            let (address, name) = device.split_once(' ')?;
            Some(format!("{} · {}", name.trim(), address))
        })
        .collect()
}

#[cfg(target_os = "windows")]
fn bluetooth() -> (Vec<String>, String) {
    match command("powershell.exe", &[
        "-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
        "$ErrorActionPreference='Stop'; Get-PnpDevice -Class Bluetooth -Status OK | Select-Object -ExpandProperty FriendlyName",
    ], std::time::Duration::from_secs(3)) {
        Ok(output) => (output.lines().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect(), "Available Bluetooth devices and adapters · Windows reports status OK".into()),
        Err(CommandError::TimedOut) => (vec![], "Bluetooth device query did not respond within 3 seconds.".into()),
        Err(_) => (vec![], "Bluetooth device information is unavailable. Check device access permissions and the Bluetooth service.".into()),
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn containers() -> (Vec<Container>, String) {
    let listing = match command(
        "docker",
        &[
            "ps",
            "--format",
            "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Ports}}\t{{.Label \"com.docker.compose.project\"}}\t{{.Status}}",
        ],
        std::time::Duration::from_secs(3),
    ) {
        Ok(s) => s,
        Err(CommandError::Missing) => {
            return (
                vec![],
                "Docker CLI is not installed or is not on PATH.".into(),
            );
        }
        Err(CommandError::TimedOut) => {
            return (
                vec![],
                "Docker engine did not respond within 3 seconds.".into(),
            );
        }
        Err(CommandError::Failed) => {
            return (
                vec![],
                "Docker engine is unavailable. Start Docker and check local engine access.".into(),
            );
        }
    };
    if listing.is_empty() {
        return (
            vec![],
            "Docker is connected. No containers are running.".into(),
        );
    }
    let stats = command(
        "docker",
        &[
            "stats",
            "--no-stream",
            "--format",
            "{{.ID}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}",
        ],
        std::time::Duration::from_secs(4),
    )
    .ok();
    let result = parse_containers(&listing, stats.as_deref());
    let status = if stats.is_some() {
        format!("{} running containers · Docker", result.len())
    } else {
        format!(
            "{} running containers · resource statistics unavailable",
            result.len()
        )
    };
    (result, status)
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows", test))]
fn parse_containers(listing: &str, stats: Option<&str>) -> Vec<Container> {
    listing
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.splitn(6, '\t').collect();
            if fields.len() < 3 {
                return None;
            }
            let stat = stats
                .and_then(|s| {
                    s.lines()
                        .find(|row| row.split('\t').next() == Some(fields[0]))
                })
                .map(|s| s.splitn(4, '\t').collect::<Vec<_>>());
            let stat_field = |i: usize| stat.as_ref().and_then(|s| s.get(i)).copied().unwrap_or("");
            let parsed_memory = stat_field(2)
                .split_once('/')
                .and_then(|(used, limit)| parse_bytes(used).zip(parse_bytes(limit)));
            // Docker reports 0B / 0B (including 0% CPU) before it has usable
            // statistics for restarting containers. This is not idle activity.
            let placeholder = parsed_memory.is_some_and(|(_, limit)| limit == 0);
            let memory_numbers = parsed_memory.filter(|(_, limit)| *limit > 0);
            let cpu_number = stat_field(1)
                .trim()
                .strip_suffix('%')
                .and_then(|percent| percent.trim().parse::<f32>().ok())
                .filter(|value| value.is_finite() && *value >= 0. && !placeholder);
            let network_numbers = stat_field(3)
                .split_once('/')
                .and_then(|(rx, tx)| parse_bytes(rx).zip(parse_bytes(tx)));
            Some(Container {
                project: fields
                    .get(4)
                    .copied()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Standalone")
                    .to_owned(),
                status: fields.get(5).copied().unwrap_or("Running").to_owned(),
                memory_used: memory_numbers.map_or(0, |m| m.0),
                memory_limit: memory_numbers.map_or(0, |m| m.1),
                cpu_percent: cpu_number.unwrap_or(0.0),
                network_rx: network_numbers.map_or(0, |n| n.0),
                network_tx: network_numbers.map_or(0, |n| n.1),
                name: fields[1].to_owned(),
                image: fields[2].to_owned(),
                ports: fields.get(3).copied().unwrap_or("").to_owned(),
                cpu: if cpu_number.is_some() {
                    stat_field(1).to_owned()
                } else {
                    "Unavailable".into()
                },
                memory: if memory_numbers.is_some() {
                    stat_field(2).to_owned()
                } else {
                    "Unavailable".into()
                },
            })
        })
        .collect()
}

#[cfg(any(target_os = "macos", test))]
fn ioreg_number(output: &str, key: &str) -> Option<u64> {
    let marker = format!("\"{key}\"");
    let tail = output
        .split_once(&marker)?
        .1
        .trim_start()
        .strip_prefix('=')?
        .trim_start();
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

#[cfg(target_os = "linux")]
fn format_hours(hours: f64) -> String {
    let minutes = (hours * 60.0).round() as u64;
    format!("{}h {}m", minutes / 60, minutes % 60)
}

/// Docker emits SI network units and binary memory units. Preserve that distinction.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows", test))]
fn parse_bytes(input: &str) -> Option<u64> {
    let s = input.trim();
    let split = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    let value = s[..split]
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)?;
    let factor = match s[split..].trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1.0,
        "kb" => 1e3,
        "mb" => 1e6,
        "gb" => 1e9,
        "tb" => 1e12,
        "kib" => 1024.0,
        "mib" => 1024.0 * 1024.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((value * factor).round() as u64)
}

#[cfg(target_os = "macos")]
fn listening_ports() -> Vec<(u32, String)> {
    query("/usr/sbin/lsof", &["-nP", "-iTCP", "-sTCP:LISTEN", "-Fpn"])
        .map(|s| parse_lsof_ports(&s))
        .unwrap_or_default()
}

#[cfg(any(target_os = "macos", test))]
fn parse_lsof_ports(output: &str) -> Vec<(u32, String)> {
    let mut pid = None;
    let mut result = Vec::new();
    for line in output.lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = p.parse::<u32>().ok();
        } else if let Some(endpoint) = line.strip_prefix('n') {
            if let (Some(pid), Some(port)) = (pid, endpoint_port(endpoint)) {
                result.push((pid, port));
            }
        }
    }
    result.sort();
    result.dedup();
    result
}

#[cfg(target_os = "linux")]
fn listening_ports() -> Vec<(u32, String)> {
    query("ss", &["-ltnpH"])
        .map(|s| parse_ss_ports(&s))
        .unwrap_or_default()
}

#[cfg(any(target_os = "linux", test))]
fn parse_ss_ports(output: &str) -> Vec<(u32, String)> {
    let mut result = Vec::new();
    for line in output.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        let Some(port) = fields.get(3).and_then(|endpoint| endpoint_port(endpoint)) else {
            continue;
        };
        for occurrence in line.split("pid=").skip(1) {
            let digits: String = occurrence
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(pid) = digits.parse::<u32>() {
                result.push((pid, port.clone()));
            }
        }
    }
    result.sort();
    result.dedup();
    result
}

#[cfg(target_os = "windows")]
fn listening_ports() -> Vec<(u32, String)> {
    query("netstat.exe", &["-ano", "-p", "tcp"])
        .map(|s| parse_netstat_ports(&s))
        .unwrap_or_default()
}

#[cfg(any(target_os = "windows", test))]
fn parse_netstat_ports(output: &str) -> Vec<(u32, String)> {
    let mut result = Vec::new();
    for line in output.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 5
            || !fields[0].eq_ignore_ascii_case("TCP")
            || !fields[3].eq_ignore_ascii_case("LISTENING")
        {
            continue;
        }
        if let (Ok(pid), Some(port)) = (fields[4].parse::<u32>(), endpoint_port(fields[1])) {
            result.push((pid, port));
        }
    }
    result.sort();
    result.dedup();
    result
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows", test))]
fn endpoint_port(endpoint: &str) -> Option<String> {
    let port = endpoint.rsplit_once(':')?.1.parse::<u16>().ok()?;
    Some(port.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn listener_parsers_map_pid_to_port_and_drop_connections() {
        assert_eq!(
            parse_lsof_ports("p42\nn*:3000\nn[::1]:3000\np99\nn127.0.0.1:8000\n"),
            vec![(42, "3000".into()), (99, "8000".into())]
        );
        assert_eq!(
            parse_ss_ports(
                "LISTEN 0 128 127.0.0.1:3000 0.0.0.0:* users:((\"node\",pid=42,fd=8))\nLISTEN 0 128 [::]:8000 [::]:* users:((\"python\",pid=99,fd=4))"
            ),
            vec![(42, "3000".into()), (99, "8000".into())]
        );
        assert_eq!(
            parse_netstat_ports(
                "TCP 0.0.0.0:3000 0.0.0.0:0 LISTENING 42\nTCP 127.0.0.1:5000 127.0.0.1:9999 ESTABLISHED 42\nTCP [::]:8000 [::]:0 LISTENING 99"
            ),
            vec![(42, "3000".into()), (99, "8000".into())]
        );
    }
    #[test]
    fn docker_metrics_preserve_units_projects_and_network_totals() {
        let result = parse_containers(
            "aaa\tpostgres\tpostgres:16\t5433->5432/tcp\tstorefront\tUp 5 hours",
            Some("aaa\t1.6%\t312MiB / 2GiB\t1.5MB / 700kB"),
        );
        let c = &result[0];
        assert_eq!(c.project, "storefront");
        assert_eq!(c.status, "Up 5 hours");
        assert_eq!(c.memory_used, 312 * 1024 * 1024);
        assert_eq!(c.memory_limit, 2 * 1024 * 1024 * 1024);
        assert_eq!(c.cpu_percent, 1.6);
        assert_eq!((c.network_rx, c.network_tx), (1_500_000, 700_000));
        assert_eq!(parse_bytes("Not available"), None);
        assert_eq!(parse_bytes("NaNB"), None);
    }
    #[test]
    fn docker_placeholder_stats_do_not_report_successful_zero_activity() {
        let listing = "aaa\trestarting\tpostgres:16\t\tstorefront\tRestarting (1) 10 seconds ago";
        for usage in ["0B / 0B", "0 B / 0 B", "0MiB / 0GiB", "128MiB / 0B"] {
            let stats = format!("aaa\t0.00%\t{usage}\t0B / 0B");
            let result = parse_containers(listing, Some(&stats));
            let container = &result[0];
            assert_eq!(container.memory, "Unavailable", "{usage}");
            assert_eq!(container.cpu, "Unavailable", "{usage}");
            assert_eq!(container.cpu_percent, 0.);
            assert_eq!((container.memory_used, container.memory_limit), (0, 0));
            assert!(container.status.starts_with("Restarting"));
        }
        // A genuine zero with an observed positive limit remains reportable.
        let result = parse_containers(listing, Some("aaa\t0.00%\t0B / 2GiB\t0B / 0B"));
        assert_eq!(result[0].cpu, "0.00%");
        assert_eq!(result[0].memory, "0B / 2GiB");
        assert_eq!(result[0].memory_used, 0);
        assert_eq!(result[0].memory_limit, 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn docker_invalid_percent_and_memory_fields_are_unavailable() {
        let listing = "aaa\tpostgres\tpostgres:16";
        for percent in ["", "NaN%", "inf%", "-1.0%", "bad%", "10", "10%%"] {
            let stats = format!("aaa\t{percent}\t312MiB / 2GiB\t0B / 0B");
            let result = parse_containers(listing, Some(&stats));
            assert_eq!(result[0].cpu, "Unavailable", "{percent}");
            assert_eq!(result[0].cpu_percent, 0.);
            assert_eq!(result[0].memory, "312MiB / 2GiB");
        }
        for usage in ["", "--", "NaNB / 2GiB", "312MiB /", "-1B / 2GiB"] {
            let stats = format!("aaa\t125.50%\t{usage}\t0B / 0B");
            let result = parse_containers(listing, Some(&stats));
            assert_eq!(result[0].memory, "Unavailable", "{usage}");
            assert_eq!((result[0].memory_used, result[0].memory_limit), (0, 0));
            assert_eq!(result[0].cpu, "125.50%");
            assert_eq!(result[0].cpu_percent, 125.5);
        }
        let incomplete = parse_containers(listing, Some("aaa\t\t"));
        assert_eq!(incomplete[0].cpu, "Unavailable");
        assert_eq!(incomplete[0].memory, "Unavailable");
    }

    #[test]
    fn battery_estimation_and_signed_current_are_not_fabricated() {
        assert!(
            parse_pmset("-InternalBattery-0 63%; discharging; (no estimate)")
                .unwrap()
                .remaining
                .is_none()
        );
        assert_eq!(
            ioreg_number(
                "  | \"InstantAmperage\" = 18446744073709548779",
                "InstantAmperage"
            )
            .unwrap() as i64,
            -2837
        );
        assert_eq!(
            ioreg_number("\"CycleCount\" = 215", "CycleCount"),
            Some(215)
        );
    }
    #[test]
    fn bluetooth_preserves_connection_state_and_ignores_device_attributes() {
        let report = "Bluetooth:\n      Bluetooth Controller:\n          State: On\n      Connected:\n          Keyboard:\n              Address: 11:22:33:44:55:66\n              RSSI: -42\n      Not Connected:\n          Headphones:\n              Firmware Version: 1.2\n";
        let (devices, status) = parse_bluetooth_macos(report);
        assert_eq!(
            devices,
            vec!["Keyboard · Connected", "Headphones · Not connected"]
        );
        assert!(status.contains("2 reported devices"));
        assert!(
            parse_bluetooth_macos("Bluetooth:\n")
                .1
                .contains("restricted")
        );
        assert_eq!(
            parse_bluez_devices("Device 11:22:33:44:55:66 Keyboard\nController ignored"),
            vec!["Keyboard · 11:22:33:44:55:66"]
        );
    }
    #[test]
    fn battery_absence_and_invalid_values_are_not_zero_percent() {
        assert_eq!(parse_pmset("Now drawing from 'AC Power'"), None);
        assert_eq!(parse_pmset("-InternalBattery-0 120%; charging;"), None);
        assert_eq!(
            parse_pmset("-InternalBattery-0 63%; discharging; 5:39 remaining").unwrap(),
            Battery {
                percent: 63.,
                status: "discharging".into(),
                remaining: Some("5h 39m".into()),
                ..Battery::default()
            }
        );
    }
    #[test]
    fn docker_rows_join_stats_by_id_not_output_order() {
        let listing =
            "aaa\tpostgres\tpostgres:16\t0.0.0.0:5433->5432/tcp\nbbb\tredis\tredis:7\t6379/tcp";
        let containers = parse_containers(
            listing,
            Some("bbb\t0.3%\t14MiB / 1GiB\naaa\t1.6%\t312MiB / 2GiB"),
        );
        assert_eq!(containers.len(), 2);
        assert_eq!(containers[0].cpu, "1.6%");
        assert_eq!(containers[1].memory, "14MiB / 1GiB");
        assert_eq!(
            parse_containers("aaa\tpostgres\tpostgres:16", None)[0].cpu,
            "Unavailable"
        );
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn cancellation_terminates_the_whole_tree_and_joins_pipe_readers() {
        let marker =
            std::env::temp_dir().join(format!("sysmon-provider-tree-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let start = std::time::Instant::now();
        let result = command(
            "/bin/sh",
            &[
                "-c",
                "(sleep 0.4; printf leaked > \"$1\") & wait",
                "sysmon-test",
                marker.to_str().unwrap(),
            ],
            std::time::Duration::from_millis(50),
        );
        assert!(matches!(result, Err(CommandError::TimedOut)));
        assert!(
            start.elapsed() < std::time::Duration::from_millis(350),
            "reader joins waited for the descendant"
        );
        std::thread::sleep(std::time::Duration::from_millis(450));
        assert_eq!(LIVE_READERS.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(
            !marker.exists(),
            "background descendant survived cancellation"
        );
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn successful_parent_does_not_leave_background_readers() {
        let start = std::time::Instant::now();
        assert_eq!(
            command(
                "/bin/sh",
                &["-c", "sleep 2 & printf done"],
                std::time::Duration::from_millis(100)
            )
            .unwrap(),
            "done"
        );
        assert!(start.elapsed() < std::time::Duration::from_millis(350));
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn subprocesses_have_a_deadline() {
        let start = std::time::Instant::now();
        assert!(matches!(
            command("/bin/sleep", &["5"], std::time::Duration::from_millis(50)),
            Err(CommandError::TimedOut)
        ));
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }
}
