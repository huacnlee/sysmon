//! sysmon: a native, live system dashboard.
use nsh_core::ui::prelude::*;
use std::{sync::mpsc, time::Duration};
mod app_list;
mod apps;
mod charts;
mod disk_io;
mod gpu;
mod live_views;
mod mobile;
#[cfg(any(target_os = "ios", target_os = "android", test))]
mod mobile_device;
mod motion;
#[cfg(any(target_os = "android", test))]
mod platform_android;
#[cfg(target_os = "ios")]
mod platform_ios;
mod providers;
mod titlebar;
mod visuals;
mod workloads;
#[cfg(not(target_os = "ios"))]
use sysinfo::{Disks, Networks, System};

#[native_module("sysmon/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}
const BG: u32 = 0x101010;
const PANEL: u32 = 0x181818;
const MUTED: u32 = 0x929296;
const COLORS: [u32; 13] = [
    0x398ef5, 0x398ef5, 0x9885ed, 0xd89b13, 0x20b48c, 0xdc5489, 0x9885ed, 0x39b84a, 0xd55abb,
    0x559ee5, 0xe36b36, 0xae83ef, 0x20b4d2,
];
const TABS: [&str; 13] = [
    "Overview",
    "CPU",
    "Memory",
    "Disk",
    "Network",
    "GPU",
    "Neural",
    "Battery",
    "Sound",
    "Bluetooth",
    "Projects",
    "Agents",
    "Containers",
];
const ICONS: [&str; 13] = [
    "⊞", "⌘", "▥", "▤", "◎", "▱", "◈", "▰", "♫", "ᛒ", "▱", "♧", "⬡",
];
#[derive(Clone, Default, PartialEq)]
struct Sample {
    cpu: f32,
    gpu: Option<f32>,
    gpu_history: Vec<f32>,
    gpu_times: Vec<f64>,
    disk_read: u64,
    disk_write: u64,
    disk_io_available: bool,
    disk_counter: Option<(String, u64, u64)>,
    cpu_available: bool,
    memory_available: bool,
    disk_available: bool,
    network_available: bool,
    last_at: Option<std::time::Instant>,
    volumes: Vec<(String, u64, u64)>,
    interfaces: Vec<(String, u64, u64)>,
    disk_history: Vec<f32>,
    network_history: Vec<f32>,
    upload_history: Vec<f32>,
    cpu_times: Vec<f64>,
    memory_times: Vec<f64>,
    disk_times: Vec<f64>,
    network_times: Vec<f64>,
    workloads: workloads::Workloads,
    memory: u64,
    total: u64,
    free: u64,
    disk: u64,
    rx: u64,
    tx: u64,
    cores: usize,
    processes: Vec<(String, f32, u64)>,
    apps: Vec<apps::AppUsage>,
    history: Vec<f32>,
    memory_history: Vec<f32>,
}
#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn sample(system: &mut System, net: &mut Networks, old: &Sample) -> Sample {
    system.refresh_all();
    net.refresh(true);
    let disks = Disks::new_with_refreshed_list();
    let mut procs: Vec<_> = system
        .processes()
        .values()
        .map(|p| {
            (
                p.name().to_string_lossy().into_owned(),
                p.cpu_usage(),
                p.memory(),
            )
        })
        .collect();
    procs.sort_by(|a, b| b.1.total_cmp(&a.1));
    let now = std::time::Instant::now();
    let elapsed = old
        .last_at
        .map(|v| now.duration_since(v).as_secs_f64())
        .unwrap_or(2.)
        .max(0.001);
    let interfaces: Vec<_> = net
        .iter()
        .filter(|(name, _)| !matches!(name.as_str(), "lo" | "lo0"))
        .map(|(name, n)| {
            (
                name.clone(),
                (n.received() as f64 / elapsed) as u64,
                (n.transmitted() as f64 / elapsed) as u64,
            )
        })
        .collect();
    let rx = interfaces.iter().map(|v| v.1).sum::<u64>();
    let tx = interfaces.iter().map(|v| v.2).sum::<u64>();
    let volumes: Vec<_> = disks
        .iter()
        .map(|d| {
            (
                d.mount_point().display().to_string(),
                d.total_space(),
                d.available_space(),
            )
        })
        .collect();
    let primary = if cfg!(target_os = "windows") {
        std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into())
    } else {
        "/".into()
    };
    let root = select_volume(&volumes, &primary);
    let disk = root.map_or(0, |v| v.1);
    let free = root.map_or(0, |v| v.2);
    let selected_disk = root.and_then(|r| {
        disks
            .iter()
            .find(|d| d.mount_point().display().to_string() == r.0)
    });
    let disk_counter = selected_disk.and_then(|d| {
        let usage = d.usage();
        // sysinfo reports all-zero counters on unsupported providers. Do not
        // call those an idle device; retain a baseline only after real counters.
        (usage.total_read_bytes > 0 || usage.total_written_bytes > 0).then(|| {
            (
                format!(
                    "{}:{}",
                    d.name().to_string_lossy(),
                    d.mount_point().display()
                ),
                usage.total_read_bytes,
                usage.total_written_bytes,
            )
        })
    });
    let io = disk_io::rates(old.disk_counter.as_ref(), disk_counter.as_ref(), elapsed);
    let mut disk_history = old.disk_history.clone();
    if disk > 0 {
        disk_history.push(100. - free as f32 / disk as f32 * 100.);
    }
    if disk_history.len() > 60 {
        disk_history.remove(0);
    }
    let mut network_history = old.network_history.clone();
    if old.last_at.is_some() && !interfaces.is_empty() {
        network_history.push(rx as f32);
    }
    if network_history.len() > 60 {
        network_history.remove(0);
    }
    let cpu = system.global_cpu_usage();
    let mut history = old.history.clone();
    if old.last_at.is_some() && !system.cpus().is_empty() {
        history.push(cpu);
    }
    if history.len() > 60 {
        history.remove(0);
    }
    let mut memory_history = old.memory_history.clone();
    if system.total_memory() > 0 {
        memory_history.push(system.used_memory() as f32 / system.total_memory() as f32 * 100.);
    }
    if memory_history.len() > 60 {
        memory_history.remove(0);
    }
    let mut next = Sample {
        gpu: gpu::collect(),
        disk_read: io.map_or(0, |v| v.0),
        disk_write: io.map_or(0, |v| v.1),
        disk_io_available: io.is_some(),
        disk_counter,
        cpu_available: old.last_at.is_some() && !system.cpus().is_empty(),
        memory_available: system.total_memory() > 0,
        disk_available: disk > 0,
        network_available: old.last_at.is_some() && !interfaces.is_empty(),
        last_at: Some(now),
        volumes,
        interfaces,
        disk_history,
        network_history,
        apps: apps::collect(system),
        workloads: workloads::collect(system),
        cpu,
        memory: system.used_memory(),
        total: system.total_memory(),
        free,
        disk,
        rx,
        tx,
        cores: system.cpus().len(),
        processes: procs,
        history,
        memory_history,
        ..Sample::default()
    };
    record_sample_times(old, &mut next);
    next
}
fn gb(n: u64) -> String {
    if n < 1_000_000_000 {
        format!("{:.0} MB", n as f64 / 1e6)
    } else {
        format!("{:.2} GB", n as f64 / 1e9)
    }
}
fn rate(n: u64) -> String {
    match n {
        0..=999 => format!("{n} B/s"),
        1000..=999_999 => format!("{:.1} kB/s", n as f64 / 1e3),
        1_000_000..=999_999_999 => format!("{:.1} MB/s", n as f64 / 1e6),
        _ => format!("{:.1} GB/s", n as f64 / 1e9),
    }
}

/// Consistent raised surfaces: a lit top edge and a restrained soft shadow.
/// These are static native effects; live readings do not rebuild the card.
pub(crate) trait PanelSurface: Styled + Sized {
    fn panel(self) -> Self {
        self.bg(rgb(0x171719))
            .border_1()
            .border_color(rgb(0x404046))
            .shadow([
                nsh_core::BoxShadow {
                    offset_y: px(2.),
                    blur: px(10.),
                    spread: px(-6.),
                    color: rgb(0x000000).opacity(0.16).into(),
                    ..Default::default()
                },
                nsh_core::BoxShadow {
                    offset_y: px(1.),
                    blur: px(0.),
                    color: rgb(0xffffff).opacity(0.025).into(),
                    inset: true,
                    ..Default::default()
                },
            ])
    }
}
impl<T: Styled + Sized> PanelSurface for T {}
fn label(s: impl Into<String>) -> impl IntoElement {
    text(s.into()).text_size(px(11.)).text_color(rgb(MUTED))
}
fn title(s: impl Into<String>, color: u32) -> impl IntoElement {
    text(s.into())
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(color))
}
fn row(a: impl Into<String>, b: impl Into<String>) -> impl IntoElement {
    view()
        .flex_row()
        .justify_between()
        .items_center()
        .child(label(a))
        .child(text(b.into()).text_size(px(12.)))
}
fn unavailable(tab: usize) -> impl IntoElement {
    let (heading, body) = match tab {
        5 => (
            "GPU telemetry unavailable",
            "GPU utilization requires a supported platform telemetry provider.",
        ),
        6 => (
            "Neural Engine telemetry unavailable",
            "This device does not expose Neural Engine activity to this app.",
        ),
        7 => (
            "Battery information unavailable",
            "No battery telemetry provider is connected.",
        ),
        8 => (
            "Audio devices",
            "Per-app audio controls require an audio routing provider.",
        ),
        9 => (
            "Bluetooth devices",
            "No Bluetooth device provider is connected.",
        ),
        10 => (
            "Development projects",
            "Project discovery is not connected yet.",
        ),
        11 => (
            "Your coding assistants",
            "Connect an account to view usage limits and running sessions.",
        ),
        _ => (
            "Containers",
            "Docker telemetry is not connected. Start Docker to make its local engine available.",
        ),
    };
    view()
        .flex_1()
        .min_h(px(0.))
        .justify_center()
        .panel()
        .rounded(px(8.))
        .p_5()
        .gap_3()
        .child(title(format!("{}  {}", ICONS[tab], heading), COLORS[tab]))
        .child(label(body))
}
#[component]
pub fn App() -> impl IntoElement {
    titlebar::install();
    let tab = signal(0_usize);
    let paused = signal(false);
    let expanded_workload = signal(String::new());
    let data = signal(Sample::default());
    let extras = signal(providers::Extras::default());
    let ready = signal(false);
    let (request, requests) = mpsc::sync_channel::<()>(1);
    let (results, receiver) = mpsc::sync_channel::<Sample>(1);
    std::thread::spawn(move || {
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let mut system = System::new_all();
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let mut network = Networks::new_with_refreshed_list();
        let mut previous = Sample::default();
        while requests.recv().is_ok() {
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            let next = sample(&mut system, &mut network, &previous);
            #[cfg(any(target_os = "ios", target_os = "android"))]
            let next = mobile_sample(&previous);
            previous = next.clone();
            if results.send(next).is_err() {
                break;
            }
        }
    });
    let _ = request.try_send(());
    set_interval(Duration::from_secs(2), move || {
        if !paused.get_untracked() {
            let _ = request.try_send(());
        }
    });
    let (extra_request, extra_requests) = mpsc::sync_channel::<()>(1);
    let (extra_results, extra_receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        while extra_requests.recv().is_ok() {
            if extra_results.send(providers::collect()).is_err() {
                break;
            }
        }
    });
    let _ = extra_request.try_send(());
    set_interval(Duration::from_secs(30), move || {
        if !paused.get_untracked() {
            let _ = extra_request.try_send(());
        }
    });
    set_interval(Duration::from_millis(100), move || {
        if let Ok(next) = receiver.try_recv() {
            if !paused.get_untracked() {
                data.set(next);
                ready.set(true);
            }
        }
        if let Ok(next) = extra_receiver.try_recv() {
            if !paused.get_untracked() {
                extras.set(next);
            }
        }
    });
    // OS is fixed for the lifetime of the host. Keep the chosen shell as the
    // direct root so it receives the full viewport rather than a region wrapper.
    if is_desktop() {
        desktop_shell(tab, paused, expanded_workload, data, extras, ready).into_element()
    } else {
        mobile::shell(data, extras, paused, ready).into_element()
    }
}
fn desktop_shell(
    tab: Signal<usize>,
    paused: Signal<bool>,
    expanded_workload: Signal<String>,
    data: Signal<Sample>,
    extras: Signal<providers::Extras>,
    ready: Signal<bool>,
) -> impl IntoElement {
    nsh_base::safe_area_view()
        .w_full()
        .h_full()
        .flex_1()
        .bg(rgb(BG))
        .text_color(rgb(0xeeeef1))
        .child(titlebar::bar(tab, paused, data))
        .child(
            view()
                .flex_1()
                .min_h(px(0.))
                .min_w(px(0.))
                .p(move || px(if window().width.0 < 600. { 10. } else { 16. }))
                .child(
                    view()
                        .flex_1()
                        .min_h(px(0.))
                        .w_full()
                        .test_id("desktop-content")
                        .overflow_hidden()
                        .child(dynamic(move || {
                            if !ready.get() {
                                return view()
                                    .p_5()
                                    .child(label("Reading your device…"))
                                    .into_element();
                            }
                            let current = tab.get();
                            let content = match current {
                                0 => live_views::overview(data, extras, tab, paused).into_element(),
                                1..=4 => view()
                                    .flex_1()
                                    .h_full()
                                    .min_h(px(0.))
                                    .gap_2()
                                    .child(live_views::detail(data, current, paused))
                                    .when(current <= 2, |v| {
                                        v.child(app_list::table_from_data(data, current == 2))
                                    })
                                    .into_element(),
                                10 | 11 => workload_page(data, extras, current, expanded_workload)
                                    .into_element(),
                                12 => container_page(extras).into_element(),
                                5 => live_views::gpu_detail(data, extras, paused).into_element(),
                                7 | 8 => dynamic(move || {
                                    device_page(&extras.get(), current).into_element()
                                })
                                .into_element(),
                                9 => dynamic(move || bluetooth_page(extras).into_element())
                                    .into_element(),
                                i => unavailable(i).into_element(),
                            };
                            motion::enter(
                                view()
                                    .w_full()
                                    .h_full()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .child(content),
                                0.,
                            )
                            .into_element()
                        })),
                ),
        )
}
#[cfg(not(target_os = "ios"))]
fn system_uptime() -> u64 {
    System::uptime()
}
#[cfg(target_os = "ios")]
fn system_uptime() -> u64 {
    0
}
#[cfg(not(target_os = "ios"))]
fn system_name() -> String {
    System::name().unwrap_or_else(|| std::env::consts::OS.into())
}
#[cfg(target_os = "ios")]
fn system_name() -> String {
    "iOS · App scope".into()
}
#[cfg(not(target_os = "ios"))]
fn system_host() -> String {
    System::host_name().unwrap_or_else(|| "This device".into())
}
#[cfg(target_os = "ios")]
fn system_host() -> String {
    "This device".into()
}

fn workload_page(
    data: Signal<Sample>,
    extras: Signal<providers::Extras>,
    tab: usize,
    expanded: Signal<String>,
) -> impl IntoElement {
    let color = COLORS[tab];
    view()
        .flex_1()
        .h_full()
        .min_h(px(0.))
        .gap_2()
        .child(view().flex_shrink_0().child(dynamic(move || {
            let s = data.get();
            let items = if tab == 10 {
                &s.workloads.projects
            } else {
                &s.workloads.agents
            };
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(summary_tile(
                    if tab == 10 {
                        "Development projects"
                    } else {
                        "Your coding assistants"
                    },
                    items.len().to_string(),
                    "Discovered on this device",
                    color,
                ))
                .child(summary_tile(
                    "Memory",
                    gb(items.iter().map(|i| i.memory).sum()),
                    "Combined resident memory",
                    color,
                ))
                .child(summary_tile(
                    "CPU",
                    format!("{:.1}%", items.iter().map(|i| i.cpu).sum::<f32>()),
                    "100% equals one core",
                    color,
                ))
                .child(summary_tile(
                    "Processes",
                    items
                        .iter()
                        .map(|i| i.running_processes)
                        .sum::<usize>()
                        .to_string(),
                    "Observed processes",
                    color,
                ))
                .into_element()
        })))
        .when(tab == 11, |v| {
            v.child(label(
                "Local sessions only · account quotas and token counts are not collected.",
            ))
        })
        .child(
            view()
                .panel()
                .rounded(px(8.))
                .p(px(10.))
                .flex_1()
                .min_h(px(0.))
                .gap_2()
                .child(
                    view()
                        .flex_row()
                        .flex_shrink_0()
                        .justify_between()
                        .child(title(
                            if tab == 10 {
                                "Projects"
                            } else {
                                "Local sessions"
                            },
                            color,
                        ))
                        .child(label("CPU / MEMORY")),
                )
                .child(
                    scroll()
                        .flex_1()
                        .min_h(px(0.))
                        .test_id("workload-list")
                        .child(dynamic(move || {
                            workload_rows(&data.get(), &extras.get(), tab, expanded).into_element()
                        })),
                ),
        )
}
fn workload_rows(
    s: &Sample,
    e: &providers::Extras,
    tab: usize,
    expanded: Signal<String>,
) -> impl IntoElement {
    let items = if tab == 10 {
        &s.workloads.projects
    } else {
        &s.workloads.agents
    };
    let color = COLORS[tab];
    view()
        .gap_2()
        .when(items.is_empty(), |v| {
            v.child(view().py_6().child(label(
                if cfg!(any(target_os = "ios", target_os = "android")) {
                    "This platform restricts discovery of other applications."
                } else if tab == 10 {
                    "No development processes detected. Start a development server to see it here."
                } else {
                    "No coding assistant processes detected."
                },
            )))
        })
        .children(items.iter().map(|item| {
            let key = format!("{}:{}:{}", tab, item.name, item.pid);
            let click_key = key.clone();
            let open = expanded.get() == key;
            let ports: Vec<_> = e
                .listening_ports
                .iter()
                .filter(|(pid, _)| item.pids.contains(pid))
                .map(|(_, port)| port.clone())
                .collect();
            view()
                .rounded(px(8.))
                .bg(rgb(if open { 0x222224 } else { PANEL }))
                .p_3()
                .gap_2()
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .on_press(move || {
                            motion::layout();
                            expanded.update(|v| {
                                if *v == click_key {
                                    v.clear();
                                } else {
                                    *v = click_key.clone();
                                }
                            });
                        })
                        .child(
                            view()
                                .flex_row()
                                .gap_2()
                                .items_center()
                                .child(visuals::badge_icon(tab, color))
                                .child(
                                    view()
                                        .gap_1()
                                        .child(title(item.name.clone(), 0xf2f2f5))
                                        .child(label(item.detail.clone())),
                                ),
                        )
                        .child(
                            view()
                                .flex_row()
                                .flex_wrap()
                                .items_center()
                                .gap_2()
                                .children(ports.iter().take(5).map(|port| chip(port, color)))
                                .child(chip(
                                    if item.cpu < 0.1 { "Low CPU" } else { "Active" },
                                    if item.cpu < 0.1 { MUTED } else { 0x3abd70 },
                                ))
                                .child(label(format!("{:.1}%", item.cpu)))
                                .child(
                                    text(gb(item.memory))
                                        .text_size(px(16.))
                                        .font_weight(FontWeight::SEMIBOLD),
                                )
                                .child(label(if open { "⌄" } else { "›" })),
                        ),
                )
                .when(open, |v| {
                    v.child(
                        view()
                            .gap_2()
                            .pl_4()
                            .child(row(
                                "Longest-running process",
                                duration(item.started_seconds),
                            ))
                            .child(row(
                                "Observed processes",
                                item.running_processes.to_string(),
                            ))
                            .child(
                                view().flex_row().flex_wrap().gap_2().children(
                                    item.pids
                                        .iter()
                                        .map(|pid| chip(&format!("PID {}", pid), MUTED)),
                                ),
                            )
                            .child(label(
                                "CPU activity alone does not determine whether a server is idle.",
                            )),
                    )
                })
        }))
}

fn container_status_color(status: &str) -> u32 {
    let state = status.to_ascii_lowercase();
    if state.contains("unhealthy") || state.starts_with("dead") {
        0xe97979
    } else if state.starts_with("restarting")
        || state.contains("paused")
        || state.contains("health: starting")
    {
        0xe5ad61
    } else if state.starts_with("up") || state.starts_with("running") {
        0x8cbf96
    } else {
        MUTED
    }
}
fn container_page(extras: Signal<providers::Extras>) -> impl IntoElement {
    view()
        .flex_1()
        .h_full()
        .min_h(px(0.))
        .gap_2()
        .child(view().flex_shrink_0().child(dynamic(move || {
            container_summary(&extras.get()).into_element()
        })))
        .child(
            view()
                .flex_shrink_0()
                .border_1()
                .border_color(rgb(0x404046))
                .rounded(px(8.))
                .p(px(10.))
                .gap_2()
                .child(title("Containers", COLORS[12]))
                .child(
                    text(move || {
                        extras.with(|e| {
                            if e.container_status.is_empty() {
                                "Discovering Docker engine…".into()
                            } else {
                                e.container_status.clone()
                            }
                        })
                    })
                    .text_size(px(11.))
                    .text_color(rgb(MUTED)),
                ),
        )
        .child(
            scroll()
                .flex_1()
                .min_h(px(0.))
                .test_id("containers-list")
                .child(dynamic(move || {
                    container_rows(&extras.get()).into_element()
                })),
        )
}
fn container_summary(e: &providers::Extras) -> impl IntoElement {
    let reporting = e
        .containers
        .iter()
        .filter(|c| c.memory != "Unavailable" && !c.memory.is_empty())
        .count();
    let memory = e.containers.iter().map(|c| c.memory_used).sum::<u64>();
    let cpu_reporting = e
        .containers
        .iter()
        .filter(|c| c.cpu != "Unavailable" && !c.cpu.is_empty())
        .count();
    let cpu = e.containers.iter().map(|c| c.cpu_percent).sum::<f32>();
    let ports = e.containers.iter().filter(|c| !c.ports.is_empty()).count();
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(summary_tile(
            "Containers",
            e.containers.len().to_string(),
            "Discovered through Docker",
            COLORS[12],
        ))
        .child(summary_tile(
            "Memory",
            reading(reporting > 0, gb(memory)),
            &format!("{} of {} reporting", reporting, e.containers.len()),
            COLORS[12],
        ))
        .child(summary_tile(
            "CPU",
            reading(cpu_reporting > 0, format!("{:.1}%", cpu)),
            "Combined container activity",
            COLORS[12],
        ))
        .child(summary_tile(
            "With published ports",
            ports.to_string(),
            "Containers exposing ports",
            COLORS[12],
        ))
}
fn container_rows(e: &providers::Extras) -> impl IntoElement {
    let mut groups: std::collections::BTreeMap<String, Vec<&providers::Container>> =
        std::collections::BTreeMap::new();
    for c in &e.containers {
        groups
            .entry(if c.project.is_empty() {
                "Standalone".into()
            } else {
                c.project.clone()
            })
            .or_default()
            .push(c);
    }
    view().gap_2()
 .children(groups.into_iter().map(|(name,containers)|{
  let group_memory=containers.iter().map(|c|c.memory_used).sum::<u64>();
  view().panel().rounded(px(8.)).p_3().gap_3()
  .child(view().flex_row().justify_between().items_center().gap_2().child(view().flex_1().min_w(px(0.)).gap_1().child(title(name,0xeeeef1)).child(label(format!("{} containers",containers.len())))).child(text(reading(containers.iter().any(|c|c.memory!="Unavailable"),gb(group_memory))).text_size(px(18.))))
  .children(containers.into_iter().map(|c|{
   let known=c.memory!="Unavailable"&&!c.memory.is_empty();let usage=c.memory_used as f32/c.memory_limit.max(1) as f32*100.;
   view().gap_2().border_1().border_color(rgb(0x29292b)).rounded(px(10.)).p_3()
   .child(view().flex_row().flex_wrap().justify_between().items_center().gap_2()
    .child(view().flex_1().min_w(px(160.)).gap_1().child(view().flex_row().items_center().gap_2().child(visuals::icon(12,COLORS[12],17.)).child(title(c.name.clone(),0xeeeef1))).child(label(c.image.clone())))
    .child(view().gap_1().child(text(c.memory.clone()).text_size(px(13.))).child(label(format!("CPU {}",c.cpu)))))
   .child(view().flex_row().flex_wrap().justify_between().gap_2().child(chip(&c.status,container_status_color(&c.status))).when(!c.ports.is_empty(),|v|v.child(chip(&c.ports,COLORS[12]))))
   .when(known,|v|v.child(view().flex_row().flex_wrap().items_center().gap_2().child(visuals::progress(usage,if usage>85.{0xe8ac63}else{COLORS[12]},120.)).child(label(format!("{:.1}% of {} limit",usage,gb(c.memory_limit)))).child(label(format!("Transferred ↓ {} / ↑ {}",gb(c.network_rx),gb(c.network_tx))))))
  }))
 }))
 .when(e.containers.is_empty(),|v|v.child(label("No container rows are available. Start Docker and check access to its configured engine.")))
}

fn device_page(e: &providers::Extras, tab: usize) -> impl IntoElement {
    let (heading, value, body) = match tab {
        5 => (
            "Graphics processor",
            e.gpu.clone().unwrap_or_else(|| "Unavailable".into()),
            "Hardware identity is available when the device provider exposes it. Utilization and per-app GPU attribution are not currently collected.",
        ),
        7 => (
            "Battery",
            e.battery
                .as_ref()
                .map(|b| b.status.clone())
                .unwrap_or_else(|| "No battery information".into()),
            "Readings come from the system power service and refresh every 30 seconds.",
        ),
        _ => (
            "Audio output",
            e.sound
                .clone()
                .unwrap_or_else(|| "No audio information".into()),
            "Current system output information. Per-app mixing requires a platform audio-routing provider and is not available here.",
        ),
    };
    view().flex_1().h_full().min_h(px(0.)).gap_2().child(view().panel().rounded(px(8.)).p_4().gap_3().flex_1().min_h(px(0.))
  .child(view().flex_row().items_center().gap_2().child(visuals::badge_icon(tab,COLORS[tab])).child(title(heading,0xeeeef1)))
  .child(text(value).text_size(px(20.)).font_weight(FontWeight::MEDIUM)).child(label(body)))
 .when(tab==7,|v|v.child(view().flex_row().flex_wrap().gap_2()
  .child(summary_tile("Charge",e.battery.as_ref().map(|b|format!("{:.0}%",b.percent)).unwrap_or_else(||"Unavailable".into()),"Current battery level",COLORS[7]))
  .child(summary_tile("Remaining",e.battery.as_ref().and_then(|b|b.remaining.clone()).unwrap_or_else(||"Unavailable".into()),"System estimate",COLORS[7]))
  .child(summary_tile("Health",e.battery.as_ref().and_then(|b|b.health).map(|v|format!("{:.0}%",v)).unwrap_or_else(||"Unavailable".into()),"Full capacity / design capacity",COLORS[7]))
  .child(summary_tile("Power draw",e.battery.as_ref().and_then(|b|b.power_w).map(|v|format!("{:.1} W",v)).unwrap_or_else(||"Unavailable".into()),"Reported battery power",COLORS[7]))))
 .when(tab==7,|v|v.child(view().border_1().border_color(rgb(0x404046)).rounded(px(8.)).p_3().gap_2()
  .child(row("Cycle count",e.battery.as_ref().and_then(|b|b.cycles).map(|v|v.to_string()).unwrap_or_else(||"Unavailable".into())))
  .child(label("Unavailable fields are not exposed by this device or its current power provider."))))
}

#[cfg(any(target_os = "ios", target_os = "android"))]
fn mobile_sample(old: &Sample) -> Sample {
    #[cfg(target_os = "ios")]
    let v = platform_ios::collect();
    #[cfg(target_os = "android")]
    let v = platform_android::collect();
    let mut history = old.history.clone();
    if v.cpu_available {
        history.push(v.cpu);
    }
    if history.len() > 60 {
        history.remove(0);
    }
    let mut memory_history = old.memory_history.clone();
    if v.memory_available && v.total > 0 {
        memory_history.push(v.memory as f32 / v.total as f32 * 100.);
    }
    if memory_history.len() > 60 {
        memory_history.remove(0);
    }
    let mut next = Sample {
        cpu_available: v.cpu_available,
        memory_available: v.memory_available,
        disk_available: v.disk_available,
        network_available: v.network_available,
        last_at: Some(std::time::Instant::now()),
        volumes: if v.disk_available {
            vec![(
                if cfg!(target_os = "android") {
                    "Data volume".into()
                } else {
                    "Sandbox volume".into()
                },
                v.disk,
                v.free,
            )]
        } else {
            Vec::new()
        },
        interfaces: if v.network_available {
            vec![("Visible interfaces".into(), v.rx, v.tx)]
        } else {
            Vec::new()
        },
        disk_history: history_push(
            &old.disk_history,
            if v.disk_available {
                Some(100. - v.free as f32 / v.disk.max(1) as f32 * 100.)
            } else {
                None
            },
        ),
        network_history: history_push(
            &old.network_history,
            if v.network_available {
                Some(v.rx as f32)
            } else {
                None
            },
        ),
        cpu: v.cpu,
        memory: v.memory,
        total: v.total,
        free: v.free,
        disk: v.disk,
        rx: v.rx,
        tx: v.tx,
        cores: v.cores,
        history,
        memory_history,
        workloads: workloads::Workloads::default(),
        processes: Vec::new(),
        apps: Vec::new(),
        ..Sample::default()
    };
    #[cfg(target_os = "android")]
    {
        next.interfaces = v.interfaces;
    }
    record_sample_times(old, &mut next);
    next
}
/// Stamp the actual capture times; pauses remain gaps rather than fabricated
/// two-second intervals. All vectors retain the same bounded 60-sample window.
fn record_sample_times(old: &Sample, next: &mut Sample) {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let now = START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64();
    next.gpu_history = history_push(&old.gpu_history, next.gpu);
    next.gpu_times = history_push(&old.gpu_times, next.gpu.map(|_| now));
    next.cpu_times = history_push(&old.cpu_times, next.cpu_available.then_some(now));
    next.memory_times = history_push(
        &old.memory_times,
        (next.memory_available && next.total > 0).then_some(now),
    );
    next.disk_times = history_push(&old.disk_times, next.disk_available.then_some(now));
    next.network_times = history_push(&old.network_times, next.network_available.then_some(now));
    next.upload_history = history_push(
        &old.upload_history,
        next.network_available.then_some(next.tx as f32),
    );
}
fn volume_network_panel(data: Signal<Sample>, tab: usize) -> impl IntoElement {
    let rows =
        dynamic(move || {
            let s = data.get();
            view()
                .gap_2()
                .when(tab == 3, |v| {
                    v.children(s.volumes.iter().map(|(name, total, free)| {
                        row(name, format!("{} free / {}", gb(*free), gb(*total)))
                    }))
                })
                .when(tab == 4, |v| {
                    v.children(s.interfaces.iter().map(|(name, rx, tx)| {
                        row(name, format!("↓ {}    ↑ {}", rate(*rx), rate(*tx)))
                    }))
                })
                .into_element()
        });
    view()
        .panel()
        .rounded(px(8.))
        .p(px(10.))
        .gap_2()
        .when(is_desktop(), |v| v.flex_1().min_h(px(0.)))
        .when(tab == 3, |v| {
            v.child(
                view()
                    .flex_shrink_0()
                    .flex_row()
                    .flex_wrap()
                    .gap_3()
                    .child(
                        text(move || {
                            data.with(|s| {
                                format!("Read {}", reading(s.disk_io_available, rate(s.disk_read)))
                            })
                        })
                        .text_size(px(12.)),
                    )
                    .child(
                        text(move || {
                            data.with(|s| {
                                format!(
                                    "Write {}",
                                    reading(s.disk_io_available, rate(s.disk_write))
                                )
                            })
                        })
                        .text_size(px(12.)),
                    ),
            )
        })
        .child(view().flex_shrink_0().child(title(
            if tab == 3 {
                "Volumes"
            } else {
                "Network interfaces"
            },
            COLORS[tab],
        )))
        .child(if is_desktop() {
            scroll()
                .flex_1()
                .min_h(px(0.))
                .test_id("volume-interfaces-list")
                .child(rows)
                .into_element()
        } else {
            view().child(rows).into_element()
        })
        .when(cfg!(target_os = "ios"), |v| {
            v.child(label(
                "iOS exposes only sandbox-visible volume and interface information.",
            ))
        })
}

fn bluetooth_page(extras: Signal<providers::Extras>) -> impl IntoElement {
    view()
        .panel()
        .rounded(px(8.))
        .p_4()
        .gap_3()
        .flex_1()
        .h_full()
        .min_h(px(0.))
        .child(
            view()
                .flex_shrink_0()
                .child(title("Bluetooth devices", COLORS[9])),
        )
        .child(
            view().flex_shrink_0().child(
                text(move || {
                    extras.with(|e| {
                        if e.bluetooth_status.is_empty() {
                            "Discovering available devices…".into()
                        } else {
                            e.bluetooth_status.clone()
                        }
                    })
                })
                .text_size(px(11.))
                .text_color(rgb(MUTED)),
            ),
        )
        .child(
            scroll()
                .flex_1()
                .min_h(px(0.))
                .test_id("bluetooth-list")
                .child(dynamic(move || {
                    view()
                        .gap_2()
                        .children(
                            extras
                                .get()
                                .bluetooth
                                .into_iter()
                                .map(|name| view().py_3().child(text(name).text_size(px(16.)))),
                        )
                        .into_element()
                })),
        )
}

fn history_push<T: Copy>(old: &[T], value: Option<T>) -> Vec<T> {
    let mut result = old.to_vec();
    if let Some(v) = value {
        result.push(v);
    }
    if result.len() > 60 {
        result.remove(0);
    }
    result
}

fn detail_stats(s: &Sample, tab: usize) -> impl IntoElement {
    let top = if tab == 2 {
        s.apps.iter().max_by_key(|a| a.memory)
    } else {
        s.apps.iter().max_by(|a, b| a.cpu.total_cmp(&b.cpu))
    };
    let peak = s.history.iter().copied().fold(0_f32, f32::max);
    let avg = s.history.iter().sum::<f32>() / s.history.len().max(1) as f32;
    let stats: Vec<(&str, String, String)> = match tab {
        1 => vec![
            (
                "Average",
                reading(
                    s.cpu_available && !s.history.is_empty(),
                    format!("{:.1}%", avg),
                ),
                "Since launch · last 60 samples".into(),
            ),
            (
                "Peak",
                reading(
                    s.cpu_available && !s.history.is_empty(),
                    format!("{:.1}%", peak),
                ),
                "Last 60 samples".into(),
            ),
            (
                "Cores",
                reading(s.cores > 0, s.cores.to_string()),
                "Logical processors".into(),
            ),
            (
                "Top app",
                top.map_or("Unavailable".into(), |a| a.name.clone()),
                top.map_or("Restricted or no data".into(), |a| {
                    format!("{:.1}% CPU", a.cpu)
                }),
            ),
        ],
        2 => vec![
            (
                "Physical memory",
                reading(s.total > 0, gb(s.total)),
                "Device capacity".into(),
            ),
            (
                "Used",
                reading(s.memory_available, gb(s.memory)),
                if cfg!(any(target_os = "ios", target_os = "android")) {
                    "Resident memory".into()
                } else {
                    "System memory".into()
                },
            ),
            (
                "Available",
                if cfg!(any(target_os = "ios", target_os = "android")) {
                    "Restricted".into()
                } else {
                    reading(s.memory_available, gb(s.total.saturating_sub(s.memory)))
                },
                "System memory".into(),
            ),
            (
                "Top app",
                top.map_or("Unavailable".into(), |a| a.name.clone()),
                top.map_or("Restricted or no data".into(), |a| gb(a.memory)),
            ),
        ],
        3 => vec![
            (
                "Used",
                reading(s.disk_available, gb(s.disk.saturating_sub(s.free))),
                "Selected volume".into(),
            ),
            (
                "Free",
                reading(s.disk_available, gb(s.free)),
                "Available storage".into(),
            ),
            (
                "Capacity",
                reading(s.disk_available, gb(s.disk)),
                "Selected volume".into(),
            ),
            (
                "Volumes",
                reading(s.disk_available, s.volumes.len().to_string()),
                "Visible mounted volumes".into(),
            ),
        ],
        _ => vec![
            (
                "Downloading",
                reading(s.network_available, rate(s.rx)),
                "All visible interfaces".into(),
            ),
            (
                "Uploading",
                reading(s.network_available, rate(s.tx)),
                "All visible interfaces".into(),
            ),
            (
                "Interfaces",
                reading(s.network_available, s.interfaces.len().to_string()),
                "Loopback excluded".into(),
            ),
            (
                "Peak download",
                reading(
                    s.network_available && !s.network_history.is_empty(),
                    rate(s.network_history.iter().copied().fold(0_f32, f32::max) as u64),
                ),
                "Last 60 samples · graph auto scales".into(),
            ),
        ],
    };
    let compact = is_desktop() && window().width.0 < 900.;
    view()
        .flex_row()
        .flex_wrap()
        .gap(px(if compact { 8. } else { 12. }))
        .children(
            stats
                .into_iter()
                .filter(|(heading, _, _)| {
                    is_desktop() || !matches!(*heading, "Top app" | "Available")
                })
                .map(|(heading, value, note)| {
                    view()
                        .flex_1()
                        .min_w(px(if compact {
                            ((window().width.0 - 40.) / 2.).max(0.)
                        } else {
                            200.
                        }))
                        .panel()
                        .rounded(px(8.))
                        .when(compact, |v| {
                            v.h(px(44.))
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .px(px(10.))
                                .gap_2()
                        })
                        .when(!compact, |v| v.p_3().gap_2())
                        .tooltip(note.clone())
                        .child(title(heading, COLORS[tab]))
                        .child(
                            text(value)
                                .text_size(px(if compact { 14. } else { 22. }))
                                .font_weight(FontWeight::SEMIBOLD),
                        )
                        .when(!compact, |v| v.child(label(note)))
                }),
        )
}

fn reading(available: bool, value: String) -> String {
    if available {
        value
    } else {
        "Unavailable".into()
    }
}
#[cfg(not(target_os = "ios"))]
fn select_volume<'a>(
    volumes: &'a [(String, u64, u64)],
    primary: &str,
) -> Option<&'a (String, u64, u64)> {
    let normalize = |name: &str| {
        name.replace('\\', "/")
            .trim_end_matches('/')
            .to_ascii_lowercase()
    };
    let desired = normalize(primary);
    volumes
        .iter()
        .find(|v| normalize(&v.0) == desired)
        .or_else(|| volumes.first())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_readings_never_look_like_successful_zero() {
        assert_eq!(reading(false, "0 GB".into()), "Unavailable");
        assert_eq!(reading(true, "0 kB/s".into()), "0 kB/s");
    }
    #[test]
    fn container_status_only_marks_healthy_running_states_green() {
        assert_eq!(container_status_color("Up 2 hours (healthy)"), 0x8cbf96);
        assert_eq!(container_status_color("Up 2 hours (unhealthy)"), 0xe97979);
        assert_eq!(
            container_status_color("Restarting (1) 30 seconds ago"),
            0xe5ad61
        );
        assert_eq!(container_status_color("Up 2 hours (Paused)"), 0xe5ad61);
        assert_eq!(container_status_color("Exited (1)"), MUTED);
    }
    #[cfg(not(target_os = "ios"))]
    #[test]
    fn primary_volume_matches_windows_drive_and_unix_root() {
        let v = vec![("D:\\".into(), 10, 5), ("C:\\".into(), 20, 7)];
        assert_eq!(select_volume(&v, "c:").unwrap().1, 20);
        assert_eq!(select_volume(&v, "D:").unwrap().1, 10);
        let v = vec![("/mnt/data".into(), 10, 5), ("/".into(), 20, 7)];
        assert_eq!(select_volume(&v, "/").unwrap().1, 20);
    }
}
fn chip(text_value: &str, color: u32) -> impl IntoElement + use<> {
    view()
        .px_2()
        .py_1()
        .rounded(px(8.))
        .bg(rgb(color).opacity(0.12))
        .child(
            text(text_value.to_owned())
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(color)),
        )
}
fn duration(seconds: u64) -> String {
    if seconds >= 86400 {
        format!("{}d {}h", seconds / 86400, seconds % 86400 / 3600)
    } else if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, seconds % 3600 / 60)
    } else {
        format!("{}m", seconds / 60)
    }
}
fn summary_tile(heading: &str, value: String, note: &str, color: u32) -> impl IntoElement {
    view()
        .flex_1()
        .min_w(move || {
            px(if window().width.0 < 900. {
                ((window().width.0 - 40.) / 2.).max(0.)
            } else {
                190.
            })
        })
        .panel()
        .rounded(px(8.))
        .p(px(10.))
        .gap(px(6.))
        .child(title(heading.to_owned(), color))
        .child(
            text(value)
                .text_size(px(18.))
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(label(note))
}
