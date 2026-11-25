//! One reading of the device. The sampler keeps only what a rate needs (the
//! previous counters and when they were read); history belongs to the app.
use serde::Serialize;
use std::time::Instant;

use crate::{apps, workloads};

/// A mounted volume: capacity and free space in bytes.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Volume {
    pub name: String,
    pub total: u64,
    pub free: u64,
}

/// A network interface's transfer rates in bytes per second.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Interface {
    pub name: String,
    pub rx: u64,
    pub tx: u64,
}

/// Read and write rates of the selected volume, in bytes per second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct DiskIo {
    pub read: u64,
    pub write: u64,
}

/// A reading. `None` means unavailable: restricted, unsupported, or a rate
/// still waiting for its baseline. It never stands for a measured zero.
#[derive(Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    /// Seconds since the sampler started; pauses stay visible as gaps.
    pub at: f64,
    /// CPU utilization, 0–100, normalized by logical cores.
    pub cpu: Option<f32>,
    pub cores: usize,
    /// Resident memory in use, in bytes.
    pub memory: Option<u64>,
    /// Physical memory, in bytes (0 when unknown).
    pub total: u64,
    /// The primary volume (`/`, or the Windows system drive).
    pub disk: Option<Volume>,
    pub disk_io: Option<DiskIo>,
    /// Download and upload over every visible interface, loopback excluded.
    pub network: Option<Interface>,
    /// Utilization of the busiest visible GPU, 0–100.
    pub gpu: Option<f32>,
    pub volumes: Vec<Volume>,
    pub interfaces: Vec<Interface>,
    pub apps: Vec<apps::AppUsage>,
    pub workloads: workloads::Workloads,
}

/// State carried between readings. Owned by one worker thread.
pub struct Sampler {
    started: Instant,
    last: Option<Instant>,
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    system: sysinfo::System,
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    networks: sysinfo::Networks,
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    disk_counter: Option<(String, u64, u64)>,
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            last: None,
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            system: sysinfo::System::new_all(),
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            networks: sysinfo::Networks::new_with_refreshed_list(),
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            disk_counter: None,
        }
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    pub fn read(&mut self) -> Reading {
        use sysinfo::Disks;
        let system = &mut self.system;
        system.refresh_all();
        self.networks.refresh(true);
        let disks = Disks::new_with_refreshed_list();
        let now = Instant::now();
        let baseline = self.last.is_some();
        let elapsed = self
            .last
            .map(|v| now.duration_since(v).as_secs_f64())
            .unwrap_or(2.)
            .max(0.001);
        let interfaces: Vec<_> = self
            .networks
            .iter()
            .filter(|(name, _)| !matches!(name.as_str(), "lo" | "lo0"))
            .map(|(name, n)| Interface {
                name: name.clone(),
                rx: (n.received() as f64 / elapsed) as u64,
                tx: (n.transmitted() as f64 / elapsed) as u64,
            })
            .collect();
        let volumes: Vec<_> = disks
            .iter()
            .map(|d| Volume {
                name: d.mount_point().display().to_string(),
                total: d.total_space(),
                free: d.available_space(),
            })
            .collect();
        let primary = if cfg!(target_os = "windows") {
            std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into())
        } else {
            "/".into()
        };
        let root = select_volume(&volumes, &primary).cloned();
        let disk_counter = root.as_ref().and_then(|r| {
            let d = disks
                .iter()
                .find(|d| d.mount_point().display().to_string() == r.name)?;
            let usage = d.usage();
            // sysinfo reports all-zero counters on unsupported providers. Do not
            // call those an idle device; keep a baseline only after real counters.
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
        let io = crate::disk_io::rates(self.disk_counter.as_ref(), disk_counter.as_ref(), elapsed);
        self.disk_counter = disk_counter;
        self.last = Some(now);
        let network = (baseline && !interfaces.is_empty()).then(|| Interface {
            name: "All interfaces".into(),
            rx: interfaces.iter().map(|v| v.rx).sum(),
            tx: interfaces.iter().map(|v| v.tx).sum(),
        });
        Reading {
            at: now.duration_since(self.started).as_secs_f64(),
            cpu: (baseline && !system.cpus().is_empty()).then(|| system.global_cpu_usage()),
            cores: system.cpus().len(),
            memory: (system.total_memory() > 0).then(|| system.used_memory()),
            total: system.total_memory(),
            disk: root.filter(|v| v.total > 0),
            disk_io: io.map(|(read, write)| DiskIo { read, write }),
            network,
            gpu: crate::gpu::collect(),
            volumes,
            interfaces: if baseline { interfaces } else { Vec::new() },
            apps: apps::collect(system),
            workloads: workloads::collect(system),
        }
    }

    #[cfg(any(target_os = "ios", target_os = "android"))]
    pub fn read(&mut self) -> Reading {
        #[cfg(target_os = "ios")]
        let v = crate::platform_ios::collect();
        #[cfg(target_os = "android")]
        let v = crate::platform_android::collect();
        let now = Instant::now();
        self.last = Some(now);
        let disk = v.disk_available.then(|| Volume {
            name: if cfg!(target_os = "android") {
                "Data volume".into()
            } else {
                "Sandbox volume".into()
            },
            total: v.disk,
            free: v.free,
        });
        let network = v.network_available.then(|| Interface {
            name: "Visible interfaces".into(),
            rx: v.rx,
            tx: v.tx,
        });
        #[cfg(target_os = "android")]
        let interfaces = v
            .interfaces
            .iter()
            .map(|(name, rx, tx)| Interface {
                name: name.clone(),
                rx: *rx,
                tx: *tx,
            })
            .collect();
        #[cfg(target_os = "ios")]
        let interfaces = network.iter().cloned().collect();
        Reading {
            at: now.duration_since(self.started).as_secs_f64(),
            cpu: v.cpu_available.then_some(v.cpu),
            cores: v.cores,
            memory: v.memory_available.then_some(v.memory),
            total: v.total,
            volumes: disk.iter().cloned().collect(),
            disk,
            disk_io: None,
            network,
            gpu: None,
            interfaces,
            apps: Vec::new(),
            workloads: workloads::Workloads::default(),
        }
    }
}

/// The primary volume, by its mount point; the first one when none matches.
pub fn select_volume<'a>(volumes: &'a [Volume], primary: &str) -> Option<&'a Volume> {
    let normalize = |name: &str| {
        name.replace('\\', "/")
            .trim_end_matches('/')
            .to_ascii_lowercase()
    };
    let desired = normalize(primary);
    volumes
        .iter()
        .find(|v| normalize(&v.name) == desired)
        .or_else(|| volumes.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(name: &str, total: u64) -> Volume {
        Volume {
            name: name.into(),
            total,
            free: total / 2,
        }
    }

    #[test]
    fn primary_volume_matches_windows_drive_and_unix_root() {
        let v = vec![volume("D:\\", 10), volume("C:\\", 20)];
        assert_eq!(select_volume(&v, "c:").unwrap().total, 20);
        assert_eq!(select_volume(&v, "D:").unwrap().total, 10);
        let v = vec![volume("/mnt/data", 10), volume("/", 20)];
        assert_eq!(select_volume(&v, "/").unwrap().total, 20);
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    #[test]
    fn rates_wait_for_a_baseline_instead_of_reporting_zero() {
        let mut sampler = Sampler::new();
        let first = sampler.read();
        assert!(first.cpu.is_none());
        assert!(first.network.is_none());
        assert!(first.disk_io.is_none());
        let second = sampler.read();
        assert!(second.at > first.at);
        assert!(second.cpu.is_some());
    }

    #[test]
    fn readings_serialize_unavailable_as_null() {
        let json = serde_json::to_value(Reading::default()).unwrap();
        assert!(json["cpu"].is_null());
        assert!(json["diskIo"].is_null());
        assert_eq!(json["workloads"]["projects"], serde_json::json!([]));
    }
}
