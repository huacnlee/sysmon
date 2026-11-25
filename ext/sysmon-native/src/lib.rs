//! System telemetry for the sysmon TSX app: one native module, `Sysmon`,
//! whose readings cross to TSX as JSON. Collectors run off the engine thread.
// Phones read their own process only; the desktop-wide collectors stay
// compiled (and tested) but unused there.
#![cfg_attr(any(target_os = "ios", target_os = "android"), allow(dead_code))]
mod apps;
mod disk_io;
mod gpu;
#[cfg(any(target_os = "ios", target_os = "android", test))]
mod mobile_device;
#[cfg(any(target_os = "android", test))]
mod platform_android;
#[cfg(target_os = "ios")]
mod platform_ios;
mod providers;
mod sampler;
mod workloads;

use nsh_core::prelude::*;
use std::sync::{Mutex, OnceLock, mpsc};

type Reply = mpsc::Sender<sampler::Reading>;

/// The sampler lives on one thread for the app's lifetime: rate baselines,
/// including the mobile collectors' thread-local ones, stay with it.
fn sampler() -> &'static Mutex<mpsc::Sender<Reply>> {
    static WORKER: OnceLock<Mutex<mpsc::Sender<Reply>>> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (requests, receiver) = mpsc::channel::<Reply>();
        std::thread::Builder::new()
            .name("sysmon-sampler".into())
            .spawn(move || {
                let mut sampler = sampler::Sampler::new();
                while let Ok(reply) = receiver.recv() {
                    let _ = reply.send(sampler.read());
                }
            })
            .expect("start the sampler thread");
        Mutex::new(requests)
    })
}

fn read() -> Result<String, String> {
    let (reply, answer) = mpsc::channel();
    sampler()
        .lock()
        .map_err(|_| "The sampler stopped".to_owned())?
        .send(reply)
        .map_err(|_| "The sampler stopped".to_owned())?;
    let reading = answer
        .recv()
        .map_err(|_| "The sampler stopped".to_owned())?;
    serde_json::to_string(&reading).map_err(|error| error.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    name: String,
    host: String,
    arch: &'static str,
    scope: &'static str,
}

/// Reads the device's resources and the processes this app may observe.
#[module]
pub struct Sysmon;

#[module]
impl Sysmon {
    /// Takes a reading: CPU, memory, storage, network, GPU, applications and
    /// developer workloads, as JSON. Rates compare with the previous reading,
    /// so the first one reports them as unavailable.
    pub async fn sample() -> Result<String, String> {
        blocking(read).await.map_err(|error| error.to_string())?
    }

    /// Reads the slower, optional providers: battery, audio output, GPU
    /// model, Bluetooth devices, Docker containers and listening ports, as JSON.
    pub async fn extras() -> Result<String, String> {
        blocking(|| serde_json::to_string(&providers::collect()))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())
    }

    /// Describes this device, as JSON: its operating system, host name,
    /// architecture and what the app may observe (`desktop`, `ios`, `android`).
    pub fn device() -> String {
        let scope = if cfg!(target_os = "ios") {
            "ios"
        } else if cfg!(target_os = "android") {
            "android"
        } else {
            "desktop"
        };
        #[cfg(not(target_os = "ios"))]
        let (name, host) = (
            sysinfo::System::name().unwrap_or_else(|| std::env::consts::OS.into()),
            sysinfo::System::host_name().unwrap_or_else(|| "This device".into()),
        );
        #[cfg(target_os = "ios")]
        let (name, host) = ("iOS · App scope".to_owned(), "This device".to_owned());
        serde_json::to_string(&Device {
            name,
            host,
            arch: std::env::consts::ARCH,
            scope,
        })
        .unwrap_or_default()
    }

    /// Seconds since the device started, or `null` where the system does not tell.
    pub fn uptime() -> Option<f64> {
        #[cfg(not(target_os = "ios"))]
        return Some(sysinfo::System::uptime() as f64);
        #[cfg(target_os = "ios")]
        return None;
    }
}

#[native_module("sysmon-native")]
fn native(registry: &mut Registry) {
    registry.module::<Sysmon>();
}
