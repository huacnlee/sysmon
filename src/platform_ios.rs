//! Sandboxed iOS collection through public Darwin APIs.
//!
//! CPU and resident memory belong to sysmon, not other applications. Physical
//! memory, sandbox-volume capacity and visible interface counters are device
//! context. No process enumeration, private API or elevated entitlement is used.

use std::{cell::RefCell, collections::BTreeMap, ffi::CStr, mem, ptr, time::Instant};

#[derive(Clone, Default, Debug)]
pub struct IosSample {
    /// Current app CPU, normalized by logical core count (0..100).
    pub cpu: f32,
    /// Current app resident memory in bytes.
    pub memory: u64,
    /// Device physical memory in bytes.
    pub total: u64,
    pub free: u64,
    pub disk: u64,
    /// Bytes per second across interfaces visible to the sandbox.
    pub rx: u64,
    pub tx: u64,
    pub cores: usize,
    pub cpu_available: bool,
    pub memory_available: bool,
    pub disk_available: bool,
    pub network_available: bool,
}

struct Previous {
    at: Instant,
    cpu_seconds: Option<f64>,
    interfaces: BTreeMap<String, (u32, u32)>,
}

thread_local! {
    static PREVIOUS: RefCell<Option<Previous>> = const { RefCell::new(None) };
}

// Public Darwin <net/if_var.h> struct if_data prefix, packed at 4 bytes.
// The tail is intentionally omitted: only bytes through ifi_obytes are read.
#[repr(C, packed(4))]
struct InterfaceDataPrefix {
    type_and_lengths: [u8; 8],
    mtu: u32,
    metric: u32,
    baudrate: u32,
    packets_in: u32,
    errors_in: u32,
    packets_out: u32,
    errors_out: u32,
    collisions: u32,
    bytes_in: u32,
    bytes_out: u32,
}

fn interfaces() -> Option<BTreeMap<String, (u32, u32)>> {
    let mut list = ptr::null_mut();
    // SAFETY: getifaddrs writes its owned linked-list pointer to `list`.
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return None;
    }
    let mut counters = BTreeMap::new();
    let mut cursor = list;
    while !cursor.is_null() {
        // SAFETY: the complete getifaddrs list remains alive until freeifaddrs.
        let entry = unsafe { &*cursor };
        if !entry.ifa_addr.is_null()
            && !entry.ifa_name.is_null()
            && !entry.ifa_data.is_null()
            && unsafe { (*entry.ifa_addr).sa_family as i32 } == libc::AF_LINK
            && entry.ifa_flags & libc::IFF_LOOPBACK as u32 == 0
            && entry.ifa_flags & libc::IFF_UP as u32 != 0
        {
            // SAFETY: AF_LINK ifa_data is Darwin's public struct if_data.
            // Read only its prefix using an unaligned copy.
            let data = unsafe { ptr::read_unaligned(entry.ifa_data.cast::<InterfaceDataPrefix>()) };
            let name = unsafe { CStr::from_ptr(entry.ifa_name) }
                .to_string_lossy()
                .into_owned();
            counters.insert(name, (data.bytes_in, data.bytes_out));
        }
        cursor = entry.ifa_next;
    }
    // SAFETY: list was allocated by the successful getifaddrs call above.
    unsafe { libc::freeifaddrs(list) };
    Some(counters)
}

fn physical_memory() -> u64 {
    let mut bytes = 0_u64;
    let mut len = mem::size_of_val(&bytes);
    // SAFETY: the destination is a u64 with its exact size, as hw.memsize requires.
    let result = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&mut bytes as *mut u64).cast(),
            &mut len,
            ptr::null_mut(),
            0,
        )
    };
    if result == 0 { bytes } else { 0 }
}

pub fn collect() -> IosSample {
    let now = Instant::now();
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let mut task: libc::mach_task_basic_info = unsafe { mem::zeroed() };
    let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
    // SAFETY: query only our own task with the matching buffer and word count.
    #[allow(deprecated)] // libc exposes the public mach_task_self trap via this wrapper.
    let memory_available = unsafe {
        libc::task_info(
            libc::mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            (&mut task as *mut libc::mach_task_basic_info).cast(),
            &mut count,
        ) == libc::KERN_SUCCESS
    };
    let mut usage: libc::rusage = unsafe { mem::zeroed() };
    // SAFETY: RUSAGE_SELF only queries this app and writes a complete rusage.
    let cpu_available = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) == 0 };
    let cpu_seconds = usage.ru_utime.tv_sec as f64
        + usage.ru_utime.tv_usec as f64 / 1e6
        + usage.ru_stime.tv_sec as f64
        + usage.ru_stime.tv_usec as f64 / 1e6;
    let sandbox = std::env::var("HOME")
        .ok()
        .and_then(|p| std::ffi::CString::new(p).ok());
    let mut volume: libc::statvfs = unsafe { mem::zeroed() };
    // SAFETY: the path is NUL-terminated and the output is a valid statvfs.
    let disk_available = sandbox
        .as_ref()
        .is_some_and(|path| unsafe { libc::statvfs(path.as_ptr(), &mut volume) == 0 });
    let visible = interfaces();
    let network_available = visible.as_ref().is_some_and(|items| !items.is_empty());
    let visible = visible.unwrap_or_default();
    let (cpu, rx, tx, cpu_baseline, network_baseline) = PREVIOUS.with(|previous| {
        let mut previous = previous.borrow_mut();
        let mut values = (0_f32, 0_u64, 0_u64, false, false);
        if let Some(last) = previous.as_ref() {
            let elapsed = now.duration_since(last.at).as_secs_f64().max(0.001);
            if cpu_available {
                if let Some(last_cpu_seconds) = last.cpu_seconds {
                    values.0 = (((cpu_seconds - last_cpu_seconds).max(0.) / elapsed) / cores as f64
                        * 100.)
                        .clamp(0., 100.) as f32;
                    values.3 = true;
                }
            }
            for (name, &(rx, tx)) in &visible {
                if let Some(&(last_rx, last_tx)) = last.interfaces.get(name) {
                    values.1 += (rx.wrapping_sub(last_rx) as f64 / elapsed) as u64;
                    values.2 += (tx.wrapping_sub(last_tx) as f64 / elapsed) as u64;
                    values.4 = true;
                }
            }
        }
        *previous = Some(Previous {
            at: now,
            cpu_seconds: cpu_available.then_some(cpu_seconds),
            interfaces: visible,
        });
        values
    });
    IosSample {
        cpu,
        memory: if memory_available {
            task.resident_size
        } else {
            0
        },
        total: physical_memory(),
        free: if disk_available {
            u64::from(volume.f_bavail) * volume.f_frsize
        } else {
            0
        },
        disk: if disk_available {
            u64::from(volume.f_blocks) * volume.f_frsize
        } else {
            0
        },
        rx,
        tx,
        cores,
        cpu_available: cpu_available && cpu_baseline,
        memory_available,
        disk_available,
        network_available: network_available && network_baseline,
    }
}
