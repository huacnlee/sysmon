# sysmon

A native system monitor for Windows, macOS, Linux, iOS and Android, built with Rust and NSH. It brings live resource readings, application activity and local developer workloads into one workspace. Desktop navigation uses a sidebar and a custom draggable TitleBar with native window controls; compact screens use a horizontal category switcher.

## Screenshots

[Desktop](docs/screenshots/desktop.png) · [iOS simulator](docs/screenshots/mobile.png) · [Android emulator](docs/screenshots/android.png)

These captures show the native macOS host at desktop and compact window sizes.

## Install

After binary assets are published to a GitHub Release:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/huacnlee/sysmon/main/install.sh | sh
```

The installer supports macOS and Linux, downloads the matching architecture archive, verifies `SHA256SUMS`, and installs `sysmon` into `~/.local/bin`. Add that directory to `PATH`, then run `sysmon`. On macOS it also adds `~/Applications/sysmon.app`; on Linux it installs a desktop launcher and icon. No Rust or NSH installation is required. Linux requires GTK 4 system libraries; macOS requires macOS 14 or newer. macOS bundles are ad-hoc signed, not notarized.

Run the same command to upgrade. To pin a release, download the script and run `sh install.sh v0.1.1`. Optional environment variables: `SYSMON_VERSION`, `SYSMON_INSTALL_DIR`, `SYSMON_BIN_DIR`, and `SYSMON_APPS_DIR`. Windows and mobile builds use the platform-specific commands below; this shell installer covers macOS and Linux only.

Release archives must exist for your OS and CPU architecture. The release workflow builds native packages on `macos-15` and `ubuntu-24.04`; manual workflow runs produce downloadable CI artifacts without publishing a release. Existing source-only releases cannot be installed with this command until binary assets and checksums have been attached.

To build an archive locally (Python 3.11+, Rust and platform SDK required):

```sh
python3 -m unittest discover -s tests -p 'test_*.py' -v
python3 scripts/build-release.py --version 0.1.1
```

Archives appear in `.nsh/dist`. Pushing a matching version tag builds both platforms, runs installer and Rust tests, and publishes archives with `SHA256SUMS` to GitHub Releases. The requested version must match both `Cargo.toml` and `nsh.json`.

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
