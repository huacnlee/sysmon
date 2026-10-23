# sysmon

A native system monitor for Windows, macOS, Linux, iOS and Android, built with Rust and NSH. It brings live resource readings, application activity and local developer workloads into one workspace. Desktop navigation uses a sidebar and a custom draggable TitleBar with native window controls; compact screens use a horizontal category switcher.

## Screenshots

[Desktop](docs/screenshots/desktop.png) · [iOS simulator](docs/screenshots/mobile.png) · [Android emulator](docs/screenshots/android.png)

These captures show the native macOS host at desktop and compact window sizes.

## Development

Install Rust stable and NSH 0.2.0, plus the platform SDKs described in [PLATFORMS.md](PLATFORMS.md).

```sh
cargo install nsh-cli --version 0.2.0
cargo run                         # native desktop
nsh run --target macos
nsh run --target windows
nsh run --target linux
nsh run --target ios               # iOS simulator, on macOS
nsh run --target android           # emulator or connected device
```

Run each target on a host with its platform SDK installed. The application root is `App` in `src/lib.rs`, configured by `nsh.json`. NSH generates native mobile runners from this Rust root; a TypeScript UI entry is not required. The optional native module import is `sysmon/native`.

## Monitoring and limits

- Live views stay mounted between samples. Application lists are virtualized, so only nearby rows are mounted while the complete dataset remains searchable and scrollable.
- Transitions, springs, card entrances, reading updates and chart geometry run through NSH native animation bindings; there is no Rust frame loop rebuilding the interface. Reduced-motion settings are respected.
- Charts use actual samples, gradient fills, live endpoints and pointer readouts. Memory graphs explicitly label their windowed vertical range; their time axis grows from the samples collected since launch, up to two minutes.
- Core readings update every two seconds. Device and Docker information refresh every 30 seconds. Blocking collectors run away from the UI thread.
- Pause freezes displayed samples. History starts at launch, holds up to 60 samples, and is not persisted. Rate measurements wait for a baseline.
- Application lists support live search, CPU/memory/name sorting and expandable resource details.
- Desktop application lists aggregate processes. Per-app CPU at 100% means one core; summed resident memory can count shared pages more than once.
- Developer projects and coding assistants are discovered from local processes, with listening ports and process duration. Docker containers are grouped by Compose project. The app does not read account quotas, prompt contents or token usage.
- GPU model, battery, audio output, Bluetooth devices and Docker statistics appear when their local providers are available. Unsupported or restricted readings are identified explicitly.
- Per-app audio mixing, Neural Engine utilization and process-control actions are not implemented.
- On iOS, CPU and resident memory describe sysmon itself. Storage and network counters describe the volume and interfaces visible to the sandbox. Android readings depend on OS access restrictions. Mobile installations do not gain desktop-wide process visibility.
- Data stays on the device. There is no analytics or remote telemetry service. The app does not terminate other processes or change device settings.

## Checks

```sh
cargo check --locked --all-targets
cargo test --locked --lib
```

For mobile library checks, first install the corresponding Rust target:

```sh
cargo check --locked --lib --target aarch64-apple-ios
cargo check --locked --lib --target aarch64-apple-ios-sim
cargo check --locked --lib --target aarch64-linux-android
```

[Native platform CI](.github/workflows/platforms.yml) is configured to check desktop builds and unit tests on Windows, macOS and Linux, and library compilation for iOS devices, iOS simulators and Android. Consult an actual workflow run for its result; configuration alone does not prove a passing build. Compile checks do not replace device testing or verify signing and distribution.

## Packaging

```sh
nsh package --target macos
nsh package --target windows
nsh package --target linux
nsh package --target ios-simulator
nsh package --target android
```

Signed iOS device packages require a signing identity and provisioning profile. See [PLATFORMS.md](PLATFORMS.md) for platform requirements and commands. Generated runners, packages and local test captures are stored under `.nsh/`; Cargo builds stay under `target/`.
