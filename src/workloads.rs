//! Read-only discovery of developer workloads. Command arguments and full paths
//! never leave this module; the UI receives short names and aggregated metrics.
use std::collections::BTreeMap;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use std::path::Path;

#[derive(Clone, Default, PartialEq)]
pub struct Workloads {
    pub projects: Vec<Workload>,
    pub agents: Vec<Workload>,
}

#[derive(Clone, Default, PartialEq)]
pub struct Workload {
    pub name: String,
    pub detail: String,
    pub memory: u64,
    /// Sum of the processes' CPU usage; may exceed 100% on multicore systems.
    pub cpu: f32,
    /// A representative process, for identification only.
    pub pid: u32,
    /// All observed process IDs, sorted for stable port matching.
    pub pids: Vec<u32>,
    /// Age in seconds of the longest-running process in this group.
    /// This is not the time since a project or assistant session was started.
    pub started_seconds: u64,
    /// Number of observed live processes, including processes sleeping between work.
    pub running_processes: usize,
}

/// Mobile operating systems restrict discovery of other applications' processes.
/// Returning no results does not imply that no other applications are running.
#[cfg(target_os = "android")]
pub fn collect(_system: &sysinfo::System) -> Workloads {
    Workloads::default()
}

#[cfg(target_os = "ios")]
pub fn collect() -> Workloads {
    Workloads::default()
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn collect(system: &sysinfo::System) -> Workloads {
    let observations = system.processes().values().filter_map(|process| {
        let name = process.name().to_string_lossy();
        let executable = process
            .exe()
            .and_then(Path::file_name)
            .map(|s| s.to_string_lossy());
        let mut agent = agent_name(&name).or_else(|| executable.as_deref().and_then(agent_name));
        // Node-based CLIs keep the package's entry point in the second argument.
        // Inspect the entry-point path, never subsequent prompts or user arguments.
        if agent.is_none() && is_script_host(&name) {
            agent = process
                .cmd()
                .get(1)
                .and_then(|s| agent_entry_point(&s.to_string_lossy()));
        }
        if agent.is_none() && !is_development_process(&name) {
            return None;
        }
        let project = process.cwd().and_then(project_identity);
        Some(Observation {
            project,
            agent,
            process_name: short_name(&name),
            memory: process.memory(),
            cpu: process.cpu_usage(),
            pid: process.pid().as_u32(),
            started_seconds: process.run_time(),
        })
    });
    aggregate(observations)
}

fn executable_name(name: &str) -> String {
    name.rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim_end_matches(".exe")
        .to_ascii_lowercase()
}

fn agent_name(name: &str) -> Option<&'static str> {
    match executable_name(name).as_str() {
        "claude" | "claude-code" => Some("Claude Code"),
        "codex" | "codex-cli" => Some("Codex"),
        "cursor" | "cursor-agent" => Some("Cursor"),
        "copilot" | "github-copilot" => Some("GitHub Copilot"),
        _ => None,
    }
}

fn agent_entry_point(path: &str) -> Option<&'static str> {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    if normalized.contains("/@anthropic-ai/claude-code/") {
        Some("Claude Code")
    } else if normalized.contains("/@openai/codex/") {
        Some("Codex")
    } else if normalized.contains("/@github/copilot/") {
        Some("GitHub Copilot")
    } else {
        agent_name(&normalized)
    }
}

fn is_script_host(name: &str) -> bool {
    matches!(
        executable_name(name).as_str(),
        "node" | "nodejs" | "bun" | "deno"
    )
}

fn is_development_process(name: &str) -> bool {
    let name = executable_name(name);
    matches!(
        name.as_str(),
        "node"
            | "nodejs"
            | "bun"
            | "deno"
            | "cargo"
            | "rustc"
            | "rust-analyzer"
            | "go"
            | "npm"
            | "pnpm"
            | "yarn"
            | "uv"
            | "pytest"
            | "ruby"
            | "php"
            | "java"
            | "gradle"
            | "dotnet"
    ) || name
        .strip_prefix("python")
        .is_some_and(|suffix| suffix.chars().all(|c| c.is_ascii_digit() || c == '.'))
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn project_identity(cwd: &Path) -> Option<(String, String)> {
    // Markers improve grouping for commands launched from subdirectories. Keep
    // the full identity as an internal key so same-named directories stay apart.
    let root = cwd
        .ancestors()
        .take(8)
        .find(|path| {
            [
                ".git",
                "Cargo.toml",
                "package.json",
                "pyproject.toml",
                "go.mod",
                "composer.json",
                "Gemfile",
            ]
            .iter()
            .any(|marker| path.join(marker).exists())
        })
        .unwrap_or(cwd);
    let name = root.file_name()?.to_string_lossy();
    if name.is_empty() {
        return None;
    }
    Some((root.to_string_lossy().into_owned(), short_name(&name)))
}

fn short_name(name: &str) -> String {
    // Prevent control characters from creating misleading rows in the UI.
    name.chars().filter(|c| !c.is_control()).take(80).collect()
}

struct Observation {
    project: Option<(String, String)>,
    agent: Option<&'static str>,
    process_name: String,
    memory: u64,
    cpu: f32,
    pid: u32,
    started_seconds: u64,
}

#[derive(Default)]
struct Group {
    workload: Workload,
    count: usize,
    kinds: std::collections::BTreeSet<String>,
}
impl Group {
    fn add(&mut self, item: &Observation, name: String) {
        if self.count == 0 {
            self.workload.name = name;
            self.workload.pid = item.pid;
        }
        self.count += 1;
        self.workload.pids.push(item.pid);
        self.workload.started_seconds = self.workload.started_seconds.max(item.started_seconds);
        self.workload.running_processes = self.count;
        self.workload.memory = self.workload.memory.saturating_add(item.memory);
        if item.cpu.is_finite() {
            self.workload.cpu += item.cpu.max(0.0);
        }
        self.kinds.insert(short_name(&item.process_name));
    }
    fn finish(mut self) -> Workload {
        self.workload.pids.sort_unstable();
        self.workload.pids.dedup();
        if let Some(pid) = self.workload.pids.first() {
            self.workload.pid = *pid;
        }
        let kinds = self.kinds.into_iter().collect::<Vec<_>>().join(", ");
        self.workload.detail = format!(
            "{} {} · {}",
            self.count,
            if self.count == 1 {
                "process"
            } else {
                "processes"
            },
            kinds
        );
        self.workload
    }
}

fn aggregate(observations: impl IntoIterator<Item = Observation>) -> Workloads {
    let mut projects = BTreeMap::<String, Group>::new();
    let mut agents = BTreeMap::<String, Group>::new();
    for item in observations {
        if let Some((identity, name)) = &item.project {
            projects
                .entry(identity.clone())
                .or_default()
                .add(&item, name.clone());
        }
        if let Some(agent) = item.agent {
            let identity = format!(
                "{}\0{}",
                agent,
                item.project.as_ref().map_or("", |p| p.0.as_str())
            );
            let name = match &item.project {
                Some((_, project)) => format!("{agent} · {project}"),
                None => agent.to_owned(),
            };
            agents.entry(identity).or_default().add(&item, name);
        }
    }
    let sorted = |groups: BTreeMap<String, Group>| {
        let mut rows: Vec<_> = groups.into_values().map(Group::finish).collect();
        rows.sort_by(|a, b| b.memory.cmp(&a.memory).then_with(|| a.name.cmp(&b.name)));
        rows
    };
    Workloads {
        projects: sorted(projects),
        agents: sorted(agents),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observation(
        path: &str,
        name: &str,
        agent: Option<&'static str>,
        memory: u64,
    ) -> Observation {
        Observation {
            project: Some((path.into(), name.into())),
            agent,
            process_name: "node".into(),
            memory,
            cpu: 2.0,
            pid: 42,
            started_seconds: 60,
        }
    }
    #[test]
    fn groups_processes_without_merging_same_named_projects() {
        let data = aggregate([
            observation("/a/app", "app", None, 10),
            observation("/a/app", "app", None, 20),
            observation("/b/app", "app", None, 15),
        ]);
        assert_eq!(data.projects.len(), 2);
        assert_eq!(data.projects[0].memory, 30);
        assert_eq!(data.projects[0].cpu, 4.0);
        assert_eq!(data.projects[0].detail, "2 processes · node");
        assert!(!data.projects[0].detail.contains("/a"));
    }
    #[test]
    fn associates_agent_processes_with_their_project() {
        let data = aggregate([
            observation("/a/app", "app", Some("Codex"), 10),
            observation("/a/app", "app", Some("Codex"), 20),
            observation("/b/docs", "docs", Some("Codex"), 15),
        ]);
        assert_eq!(data.agents.len(), 2);
        assert_eq!(data.agents[0].name, "Codex · app");
        assert_eq!(data.agents[0].memory, 30);
    }
    #[test]
    fn exposes_all_pids_longest_uptime_and_process_count() {
        let mut younger = observation("/a/app", "app", Some("Codex"), 10);
        younger.pid = 90;
        younger.started_seconds = 120;
        let mut older = observation("/a/app", "app", Some("Codex"), 20);
        older.pid = 20;
        older.started_seconds = 3600;
        let data = aggregate([younger, older]);
        for group in [&data.projects[0], &data.agents[0]] {
            assert_eq!(group.pids, vec![20, 90]);
            assert_eq!(group.pid, 20);
            assert_eq!(group.started_seconds, 3600);
            assert_eq!(group.running_processes, 2);
        }
    }
    #[test]
    fn matches_executables_and_entry_points_not_arbitrary_prompts() {
        assert_eq!(agent_name("C:\\tools\\codex.exe"), Some("Codex"));
        assert_eq!(
            agent_entry_point("/node_modules/@anthropic-ai/claude-code/cli.js"),
            Some("Claude Code")
        );
        assert_eq!(agent_name("my-codex-notes.txt"), None);
        assert_eq!(agent_entry_point("please ask codex about this"), None);
        assert!(is_development_process("python3.12"));
        assert!(!is_development_process("python-notes"));
        assert_eq!(short_name("node\nsecret"), "nodesecret");
    }
}
