//! Android readings available to an ordinary, sandboxed application.
//!
//! CPU and RSS describe this process. Storage describes `/data`, not the
//! read-only operating-system partition. Restricted network counters stay
//! unavailable; interface names alone do not prove that counters are readable.

use std::collections::BTreeMap;
#[cfg(target_os = "android")]
use std::{cell::RefCell, fs, mem, time::Instant};

#[cfg(target_os = "android")]
#[derive(Clone, Default, Debug)]
pub struct AndroidSample {
    pub cpu: f32,
    pub memory: u64,
    pub total: u64,
    pub free: u64,
    pub disk: u64,
    pub rx: u64,
    pub tx: u64,
    pub cores: usize,
    pub cpu_available: bool,
    pub memory_available: bool,
    pub disk_available: bool,
    pub network_available: bool,
    pub interfaces: Vec<(String, u64, u64)>,
}

#[cfg(target_os = "android")]
struct Previous {
    at: Instant,
    cpu_seconds: Option<f64>,
    interfaces: Option<BTreeMap<String, (u64, u64)>>,
}

#[cfg(target_os = "android")]
thread_local! {
    static PREVIOUS: RefCell<Option<Previous>> = const { RefCell::new(None) };
}

fn resident_bytes(statm: &str, page_size: u64) -> Option<u64> {
    if page_size == 0 {
        return None;
    }
    statm
        .split_whitespace()
        .nth(1)?
        .parse::<u64>()
        .ok()?
        .checked_mul(page_size)
}

fn status_resident_bytes(status: &str) -> Option<u64> {
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let mut fields = line.split_whitespace();
    fields.next()?;
    let value = fields.next()?.parse::<u64>().ok()?;
    (fields.next()? == "kB").then_some(value)?.checked_mul(1024)
}

/// Reading can fail on current Android versions even when interface names are
/// visible. In that case no network sample is fabricated from zero counters.
fn network_counters(text: &str) -> Option<BTreeMap<String, (u64, u64)>> {
    let mut result = BTreeMap::new();
    for line in text.lines() {
        let Some((name, counters)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name == "lo" || name.is_empty() {
            continue;
        }
        let counters: Vec<_> = counters.split_whitespace().collect();
        if counters.len() < 16 {
            return None;
        }
        result.insert(
            name.to_owned(),
            (counters[0].parse().ok()?, counters[8].parse().ok()?),
        );
    }
    (!result.is_empty()).then_some(result)
}

fn cpu_percent(current: f64, previous: f64, elapsed: f64, cores: usize) -> Option<f32> {
    if !current.is_finite()
        || !previous.is_finite()
        || current < previous
        || !elapsed.is_finite()
        || elapsed <= 0.
        || cores == 0
    {
        return None;
    }
    Some(((current - previous) / elapsed / cores as f64 * 100.).clamp(0., 100.) as f32)
}

#[cfg(target_os = "android")]
pub fn collect() -> AndroidSample {
    let now = Instant::now();
    // SAFETY: these sysconf selectors take no pointer arguments.
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let logical_cores = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
    let cores = usize::try_from(logical_cores).unwrap_or(0);
    let page_size = u64::try_from(page_size).unwrap_or(0);
    let total = u64::try_from(pages)
        .ok()
        .and_then(|n| n.checked_mul(page_size))
        .unwrap_or(0);
    let memory = fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| resident_bytes(&s, page_size))
        .or_else(|| {
            fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|s| status_resident_bytes(&s))
        });

    let mut usage: libc::rusage = unsafe { mem::zeroed() };
    // SAFETY: RUSAGE_SELF only queries our own process with a valid output.
    let cpu_seconds = (unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } == 0).then(|| {
        usage.ru_utime.tv_sec as f64
            + usage.ru_utime.tv_usec as f64 / 1e6
            + usage.ru_stime.tv_sec as f64
            + usage.ru_stime.tv_usec as f64 / 1e6
    });
    let mut volume: libc::statvfs = unsafe { mem::zeroed() };
    // SAFETY: /data is a static NUL-terminated path, volume is writable.
    let disk_available = unsafe { libc::statvfs(c"/data".as_ptr(), &mut volume) } == 0
        && volume.f_blocks > 0
        && volume.f_frsize > 0;
    let counters = fs::read_to_string("/proc/net/dev")
        .ok()
        .and_then(|s| network_counters(&s));

    let (cpu, interfaces) = PREVIOUS.with(|previous| {
        let mut previous = previous.borrow_mut();
        let mut cpu = None;
        let mut interfaces = Vec::new();
        if let Some(last) = previous.as_ref() {
            let elapsed = now.duration_since(last.at).as_secs_f64();
            if let (Some(current), Some(prior)) = (cpu_seconds, last.cpu_seconds) {
                cpu = cpu_percent(current, prior, elapsed, cores);
            }
            if let (Some(current), Some(prior)) = (&counters, &last.interfaces) {
                for (name, &(rx, tx)) in current {
                    if let Some(&(last_rx, last_tx)) = prior.get(name) {
                        // Counter reset/replaced interfaces require a new baseline.
                        if let (Some(rx), Some(tx)) =
                            (rx.checked_sub(last_rx), tx.checked_sub(last_tx))
                        {
                            if elapsed > 0. {
                                interfaces.push((
                                    name.clone(),
                                    (rx as f64 / elapsed) as u64,
                                    (tx as f64 / elapsed) as u64,
                                ));
                            }
                        }
                    }
                }
            }
        }
        *previous = Some(Previous {
            at: now,
            cpu_seconds,
            interfaces: counters,
        });
        (cpu, interfaces)
    });

    AndroidSample {
        cpu: cpu.unwrap_or(0.),
        memory: memory.unwrap_or(0),
        total,
        free: if disk_available {
            (volume.f_bavail as u64).saturating_mul(volume.f_frsize as u64)
        } else {
            0
        },
        disk: if disk_available {
            (volume.f_blocks as u64).saturating_mul(volume.f_frsize as u64)
        } else {
            0
        },
        rx: interfaces.iter().map(|(_, rx, _)| rx).sum(),
        tx: interfaces.iter().map(|(_, _, tx)| tx).sum(),
        cores,
        cpu_available: cpu.is_some(),
        memory_available: memory.is_some(),
        disk_available,
        network_available: !interfaces.is_empty(),
        interfaces,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rss_uses_resident_not_virtual_pages_and_checks_units() {
        assert_eq!(
            resident_bytes("9000 23 10 1 0 20 0\n", 4096),
            Some(23 * 4096)
        );
        assert_eq!(resident_bytes("9000", 4096), None);
        assert_eq!(resident_bytes("9000 23", 0), None);
        assert_eq!(resident_bytes("0 18446744073709551615", 4096), None);
        assert_eq!(
            status_resident_bytes("Name:\tsysmon\nVmRSS:\t123 kB\n"),
            Some(123 * 1024)
        );
        assert_eq!(status_resident_bytes("VmRSS: 123 MB"), None);
    }

    #[test]
    fn network_requires_readable_complete_non_loopback_counters() {
        let text = "Inter-| Receive | Transmit\n lo: 100 0 0 0 0 0 0 0 100 0 0 0 0 0 0 0\n wlan0: 1234 1 0 0 0 0 0 0 456 1 0 0 0 0 0 0\n";
        assert_eq!(
            network_counters(text).unwrap().get("wlan0"),
            Some(&(1234, 456))
        );
        assert!(network_counters("").is_none());
        assert!(network_counters("wlan0: 0 1 2").is_none());
        assert!(network_counters("lo: 1 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0").is_none());
    }

    #[test]
    fn cpu_is_normalized_to_all_cores_and_requires_valid_baseline() {
        assert_eq!(cpu_percent(14., 10., 2., 4), Some(50.));
        assert_eq!(cpu_percent(10., 10., 2., 4), Some(0.));
        assert_eq!(cpu_percent(14., 10., 0., 4), None);
        assert_eq!(cpu_percent(9., 10., 2., 4), None);
        assert_eq!(cpu_percent(f64::NAN, 10., 2., 4), None);
    }
}
