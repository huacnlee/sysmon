//! Application-level resource accounting. Every visible process contributes once;
//! known helper processes inherit their application's identity where possible.
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppUsage {
    /// Stable opaque identity. It contains no executable or bundle path.
    pub id: String,
    pub name: String,
    /// Sum of process CPU percentages; 100% represents one fully used core.
    pub cpu: f32,
    /// Sum of reported process memory, in bytes (shared pages may overlap).
    pub memory: u64,
    pub processes: usize,
}

#[cfg(target_os = "ios")]
pub fn collect() -> Vec<AppUsage> {
    Vec::new()
}

#[cfg(not(target_os = "ios"))]
pub fn collect(system: &sysinfo::System) -> Vec<AppUsage> {
    aggregate(system.processes().values().map(|process| {
        ProcessUsage {
            pid: process.pid().as_u32(),
            parent: process.parent().map(|pid| pid.as_u32()),
            name: process.name().to_string_lossy().into_owned(),
            executable: process
                .exe()
                .map(|path| path.to_string_lossy().into_owned()),
            cpu: process.cpu_usage(),
            memory: process.memory(),
        }
    }))
}

#[derive(Clone, Debug)]
struct ProcessUsage {
    pid: u32,
    parent: Option<u32>,
    name: String,
    executable: Option<String>,
    cpu: f32,
    memory: u64,
}

#[derive(Clone)]
struct Identity {
    key: String,
    name: String,
}

fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(100)
        .collect()
}

fn basename(value: &str) -> &str {
    value.rsplit(['/', '\\']).next().unwrap_or(value)
}

fn without_exe(value: &str) -> &str {
    if value
        .get(value.len().saturating_sub(4)..)
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".exe"))
    {
        &value[..value.len() - 4]
    } else {
        value
    }
}

fn bundle_identity(path: &str) -> Option<Identity> {
    // The outer app owns nested login items and Helper.app bundles as well.
    let normalized = path.replace('\\', "/");
    for (index, _) in normalized.match_indices(".app") {
        let end = index + 4;
        if end != normalized.len() && normalized.as_bytes()[end] != b'/' {
            continue;
        }
        let bundle = &normalized[..end];
        let name = basename(bundle).strip_suffix(".app")?;
        return Some(Identity {
            key: format!("bundle:{bundle}"),
            name: clean(name),
        });
    }
    None
}

fn executable_identity(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    // Windows paths are normally case insensitive. Preserve Unix case so
    // case-sensitive installations remain distinct.
    if normalized.as_bytes().get(1) == Some(&b':') || normalized.starts_with("//") {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

fn opaque_id(key: &str) -> String {
    // Fixed FNV-1a rather than the randomly seeded/version-dependent std hasher.
    // This hides paths from UI data; it is an identity, not a security primitive.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in key.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("app-{hash:016x}")
}

fn readable_name(raw: &str) -> String {
    let raw = without_exe(basename(raw));
    let name = match raw.to_ascii_lowercase().as_str() {
        "chrome" | "google-chrome" | "google-chrome-stable" => "Google Chrome",
        "chromium" | "chromium-browser" => "Chromium",
        "firefox" | "firefox-bin" => "Firefox",
        "code" | "code-insiders" => "Visual Studio Code",
        "msedge" | "microsoft-edge" | "microsoft-edge-stable" => "Microsoft Edge",
        "brave" | "brave-browser" => "Brave Browser",
        "safari" => "Safari",
        "cursor" => "Cursor",
        "slack" => "Slack",
        "discord" => "Discord",
        "teams" | "ms-teams" => "Microsoft Teams",
        "zoom" => "Zoom",
        "spotify" => "Spotify",
        _ => raw,
    };
    let result = clean(name);
    if result.is_empty() {
        "Unknown application".into()
    } else {
        result
    }
}

fn direct_identity(process: &ProcessUsage) -> Identity {
    if let Some(bundle) = process.executable.as_deref().and_then(bundle_identity) {
        return bundle;
    }
    let executable = process
        .executable
        .as_deref()
        .map(basename)
        .filter(|name| !name.is_empty())
        .unwrap_or(&process.name);
    let name = readable_name(executable);
    Identity {
        key: process
            .executable
            .as_deref()
            .filter(|path| !path.is_empty())
            .map_or_else(
                || format!("name:{}", name.to_lowercase()),
                |path| format!("exe:{}", executable_identity(path)),
            ),
        name,
    }
}

fn is_helper(process: &ProcessUsage) -> bool {
    let name = format!(
        "{} {}",
        process.name,
        process.executable.as_deref().map(basename).unwrap_or("")
    )
    .to_ascii_lowercase();
    [
        "helper",
        "renderer",
        "webcontent",
        "web content",
        "gpu-process",
        "crashpad",
        "plugin-container",
        "webkitwebprocess",
        "webkitnetworkprocess",
        "webkit.networking",
        "qtwebengineprocess",
    ]
    .iter()
    .any(|kind| name.contains(kind))
}

fn identity(process: &ProcessUsage, processes: &HashMap<u32, ProcessUsage>) -> Identity {
    if process
        .executable
        .as_deref()
        .and_then(bundle_identity)
        .is_some()
        || !is_helper(process)
    {
        return direct_identity(process);
    }
    let mut current = process;
    let mut visited = HashSet::new();
    for _ in 0..16 {
        if !visited.insert(current.pid) {
            break;
        }
        let Some(parent) = current.parent.and_then(|pid| processes.get(&pid)) else {
            break;
        };
        // Do not assign an orphan helper to launchd, systemd or the kernel.
        if parent.pid <= 1 {
            break;
        }
        if parent
            .executable
            .as_deref()
            .and_then(bundle_identity)
            .is_some()
            || !is_helper(parent)
        {
            return direct_identity(parent);
        }
        current = parent;
    }
    direct_identity(process)
}

fn aggregate(observations: impl IntoIterator<Item = ProcessUsage>) -> Vec<AppUsage> {
    let processes: HashMap<_, _> = observations.into_iter().map(|p| (p.pid, p)).collect();
    let mut apps = BTreeMap::<String, AppUsage>::new();
    for process in processes.values() {
        let id = identity(process, &processes);
        let public_id = opaque_id(&id.key);
        let app = apps.entry(id.key).or_insert_with(|| AppUsage {
            id: public_id,
            name: id.name,
            ..Default::default()
        });
        if process.cpu.is_finite() {
            app.cpu += process.cpu.max(0.0);
        }
        app.memory = app.memory.saturating_add(process.memory);
        app.processes += 1;
    }
    let mut apps: Vec<_> = apps.into_values().collect();
    apps.sort_by(|a, b| {
        b.memory
            .cmp(&a.memory)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.id.cmp(&b.id))
    });
    apps
}

#[cfg(test)]
mod tests {
    use super::*;
    fn process(pid: u32, parent: Option<u32>, name: &str, path: &str, memory: u64) -> ProcessUsage {
        ProcessUsage {
            pid,
            parent,
            name: name.into(),
            executable: Some(path.into()),
            cpu: 2.5,
            memory,
        }
    }
    #[test]
    fn nested_mac_helpers_belong_to_outer_application() {
        let apps = aggregate([
            process(
                10,
                Some(1),
                "Google Chrome",
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                100,
            ),
            process(
                11,
                Some(10),
                "Google Chrome Helper",
                "/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Helper.app/Contents/MacOS/Google Chrome Helper",
                200,
            ),
        ]);
        assert_eq!(apps.len(), 1);
        assert_eq!(
            apps[0],
            AppUsage {
                id: opaque_id("bundle:/Applications/Google Chrome.app"),
                name: "Google Chrome".into(),
                cpu: 5.0,
                memory: 300,
                processes: 2
            }
        );
    }
    #[test]
    fn same_named_installs_keep_distinct_stable_opaque_ids() {
        let apps = aggregate([
            process(10, None, "editor", "/opt/stable/editor", 10),
            process(11, None, "editor", "/opt/nightly/editor", 20),
            process(
                12,
                None,
                "My App",
                "/Applications/My App.app/Contents/MacOS/My App",
                30,
            ),
            process(
                13,
                None,
                "My App",
                "/Users/alice/Applications/My App.app/Contents/MacOS/My App",
                40,
            ),
        ]);
        assert_eq!(apps.len(), 4);
        assert_ne!(apps[0].id, apps[1].id);
        assert_ne!(apps[2].id, apps[3].id);
        for app in &apps {
            assert!(app.id.starts_with("app-"));
            assert_eq!(app.id.len(), 20);
            assert!(!app.id.contains('/'));
        }
        let restarted = aggregate([process(99, None, "editor", "/opt/stable/editor", 999)]);
        let original = apps.iter().find(|app| app.memory == 10).unwrap();
        assert_eq!(original.id, restarted[0].id);
        assert_eq!(opaque_id("hello"), "app-a430d84680aabd0b");
    }
    #[test]
    fn windows_paths_differing_only_in_case_group_together() {
        let apps = aggregate([
            process(10, None, "editor.exe", "C:\\Tools\\Editor.exe", 10),
            process(11, None, "editor.exe", "c:\\tools\\editor.EXE", 20),
        ]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].processes, 2);
    }
    #[test]
    fn windows_and_linux_executables_have_readable_names() {
        assert_eq!(
            readable_name("C:\\Program Files\\Chrome\\chrome.EXE"),
            "Google Chrome"
        );
        assert_eq!(readable_name("/usr/bin/code"), "Visual Studio Code");
        assert_eq!(readable_name("/opt/custom/editor"), "editor");
    }
    #[test]
    fn helpers_inherit_parents_without_losing_accounting() {
        let apps = aggregate([
            process(10, Some(1), "firefox", "/usr/bin/firefox", 100),
            process(
                11,
                Some(10),
                "Web Content",
                "/usr/lib/firefox/plugin-container",
                200,
            ),
            process(12, Some(11), "helper", "/usr/lib/firefox/helper", 50),
            process(20, Some(1), "bash", "/usr/bin/bash", 10),
        ]);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].name, "Firefox");
        assert_eq!(apps[0].processes, 3);
        assert_eq!(apps.iter().map(|a| a.memory).sum::<u64>(), 360);
        assert_eq!(apps.iter().map(|a| a.cpu).sum::<f32>(), 10.0);
        assert_eq!(apps.iter().map(|a| a.processes).sum::<usize>(), 4);
    }
    #[test]
    fn cycles_and_missing_parents_do_not_drop_processes() {
        let apps = aggregate([
            process(10, Some(11), "helper", "/bin/helper", 10),
            process(11, Some(10), "helper", "/bin/helper", 20),
            process(12, Some(99), "renderer", "/bin/renderer", 30),
        ]);
        assert_eq!(apps.iter().map(|a| a.processes).sum::<usize>(), 3);
        assert_eq!(apps.iter().map(|a| a.memory).sum::<u64>(), 60);
    }
    #[test]
    fn app_names_do_not_expose_full_paths_or_control_characters() {
        let apps = aggregate([process(
            10,
            None,
            "secret",
            "/private/secret/folder/my\napp",
            10,
        )]);
        assert_eq!(apps[0].name, "myapp");
    }
}
