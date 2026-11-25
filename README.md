# sysmon

A native system monitor for Windows, macOS, Linux, iOS and Android, built with Rust and NSH. It brings live resource readings, application activity and local developer workloads into one workspace.

<img width="1232" height="872" alt="image" src="https://github.com/user-attachments/assets/8593d5b9-c861-49f5-b580-d951fa833053" />

## Screenshots

[Desktop](docs/screenshots/desktop.png) · [iOS simulator](docs/screenshots/mobile.png) · [Android emulator](docs/screenshots/android.png)

These captures show native desktop, iOS simulator and Android emulator builds.

## Install

After binary assets are published to a GitHub Release:

```sh
curl -sSL https://raw.githubusercontent.com/huacnlee/sysmon/main/install.sh | sh
```

The installer supports macOS and Linux, downloads the matching architecture archive, verifies `SHA256SUMS`, and installs `sysmon` into `~/.local/bin`. Add that directory to `PATH`, then run `sysmon`. On macOS it also adds `~/Applications/sysmon.app`; on Linux it installs a desktop launcher and icon. No Rust or NSH installation is required. Linux requires GTK 4 system libraries; macOS requires macOS 14 or newer. macOS bundles are ad-hoc signed, not notarized.

Run the same command to upgrade. To pin a release, download the script and run `sh install.sh v0.1.5`. Optional environment variables: `SYSMON_VERSION`, `SYSMON_INSTALL_DIR`, `SYSMON_BIN_DIR`, and `SYSMON_APPS_DIR`. Windows and mobile builds use the platform-specific commands below; this shell installer covers macOS and Linux only.

Release archives must exist for your OS and CPU architecture. The release workflow builds native packages on `macos-15` and `ubuntu-24.04`; manual workflow runs produce downloadable CI artifacts without publishing a release. Existing source-only releases cannot be installed with this command until binary assets and checksums have been attached.

To build an archive locally (Python 3.11+, Rust, the NSH nightly CLI and the platform SDK required):

```sh
python3 -m unittest discover -s tests -p 'test_*.py' -v
python3 scripts/build-release.py --version 0.1.7
```

Archives appear in `.nsh/dist`. Pushing a matching version tag builds both platforms, runs the installer and app tests, and publishes archives with `SHA256SUMS` to GitHub Releases. The requested version must match `nsh.json`.

## Development

Install Rust stable and the NSH nightly CLI, plus the platform SDKs described in [PLATFORMS.md](PLATFORMS.md). `scripts/setup-nightly.sh` builds the pinned nightly into `.nsh/bin/nsh`.

```sh
scripts/setup-nightly.sh && export PATH="$PWD/.nsh/bin:$PATH"
nsh run                            # native desktop
nsh run --target macos
nsh run --target windows
nsh run --target linux
nsh run --target ios               # iOS simulator, on macOS
nsh run --target android           # emulator or connected device
```

Run each target on a host with its platform SDK installed. sysmon is a TSX app: `App.tsx` mounts the interface in `src/`, and the collectors are the Rust native module in `ext/sysmon-native` (an `nshm` crate, imported as `sysmon-native`). The UI owns presentation and history; the module samples the device on its own thread and returns readings as JSON.

| Path | Contents |
| --- | --- |
| `src/model.ts`, `src/monitor.ts` | Reading types, formatting, history and the sampling loop |
| `src/ui.tsx`, `src/motion.tsx`, `src/chart.tsx` | The app's styled components: panels, odometer readings, live charts |
| `src/pages/`, `src/Desktop.tsx`, `src/Mobile.tsx` | Pages and the desktop and phone shells |
| `theme.json` | Design tokens for every color the app draws |
| `ext/sysmon-native/` | CPU, memory, disk, network, GPU, process, battery and Docker collectors |

## Monitoring and limits

- Live views stay mounted between samples. Application lists are virtualized, so only nearby rows are mounted while the complete dataset remains searchable and scrollable.
- Large readings use native digit reels with stable decimal points and units. Reduced motion switches readings immediately.
- Transitions, card entrances, reading updates and chart geometry run through NSH native animated values; no JavaScript runs per frame. Reduced-motion settings are respected.
- Charts use actual samples, gradient fills, live endpoints and pointer readouts. Memory graphs explicitly label their windowed vertical range; their time axis uses actual capture timestamps. Pauses remain visible as gaps. Network plots show download and upload in bytes per second rather than normalized percentages. Hover or press and drag to inspect a sample.
- Core readings update every two seconds. Device and Docker information refresh every 30 seconds. Blocking collectors run away from the UI thread.
- Pause freezes displayed samples. History starts at launch, holds up to 60 samples, and is not persisted. Rate measurements wait for a baseline.
- Application lists support live search, CPU/memory/name sorting and expandable resource details.
- Desktop application lists aggregate processes. Per-app CPU at 100% means one core; summed resident memory can count shared pages more than once.
- Developer projects and coding assistants are discovered from local processes, with listening ports and process duration. Docker containers are grouped by Compose project. The app does not read account quotas, prompt contents or token usage.
- The desktop TitleBar always shows CPU, GPU, memory and selected-volume disk read/write rates, with unavailable readings shown explicitly. macOS GPU utilization uses IOKit; Linux uses driver-exported GPU busy counters when available. GPU model, battery, audio output, Bluetooth devices and Docker statistics appear when their local providers are available. Unsupported or restricted readings are identified explicitly.
- Per-app audio mixing, Neural Engine utilization and process-control actions are not implemented.
- On iOS and Android, CPU and resident memory describe sysmon itself. CPU is normalized to device logical-core capacity. iOS battery and current app audio-route information come from public system APIs; Android battery readings appear only when its power-supply data is accessible. Mobile navigation omits desktop-only application, developer-workload and container pages. Storage and network counters describe the volume and interfaces visible to the sandbox. Android readings depend on OS access restrictions. Mobile installations do not gain desktop-wide process visibility.
- Data stays on the device. There is no analytics or remote telemetry service. The app does not terminate other processes or change device settings.

## Checks

```sh
nsh check     # Rust module and TypeScript types
nsh build     # bundle, class and export warnings
nsh test      # Rust unit tests, then tests/*.test.tsx headless
cargo clippy --manifest-path ext/sysmon-native/Cargo.toml --all-targets -- --deny warnings
cargo fmt --manifest-path ext/sysmon-native/Cargo.toml --check
```

For mobile module checks, first install the corresponding Rust target:

```sh
cargo check --manifest-path ext/sysmon-native/Cargo.toml --locked --target aarch64-apple-ios
cargo check --manifest-path ext/sysmon-native/Cargo.toml --locked --target aarch64-apple-ios-sim
cargo check --manifest-path ext/sysmon-native/Cargo.toml --locked --target aarch64-linux-android
```

[Native platform CI](.github/workflows/platforms.yml) is configured to type-check, build and test the app on macOS and Linux, and to compile the native module for iOS devices, iOS simulators and Android. Consult an actual workflow run for its result; configuration alone does not prove a passing build. Compile checks do not replace device testing or verify signing and distribution.

## Packaging

```sh
nsh package --target macos
nsh package --target windows
nsh package --target linux
nsh package --target ios-simulator
nsh package --target android
```

Signed iOS device packages require a signing identity and provisioning profile. See [PLATFORMS.md](PLATFORMS.md) for platform requirements and commands. Generated runners, packages and local test captures are stored under `.nsh/`; the native module builds under `ext/sysmon-native/target/`.

Android packages need JDK 17+ and the Android SDK/NDK. Pass signing flags such as `--keystore` and `--key-alias` to `nsh package`, with `NSH_KEYSTORE_PASSWORD` for the password.
