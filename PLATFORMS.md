# sysmon platforms

sysmon shares its TSX interface (`App.tsx`) and its Rust telemetry module
(`ext/sysmon-native`) across Windows, macOS, Linux, iOS and Android. NSH
supplies a native platform host and renders through each platform's views; no
browser is involved.

| Platform | Development | Package |
| --- | --- | --- |
| Windows | `nsh run --target windows` | `nsh package --target windows` |
| macOS | `nsh run --target macos` | `nsh package --target macos` |
| Linux | `nsh run --target linux` | `nsh package --target linux` |
| iOS simulator | `nsh run --target ios` | `nsh package --target ios-simulator` |
| Android | `nsh run --target android` | `nsh package --target android` |

Every target builds the native module into its runner; `nsh run` with no
target runs the desktop app on the current host.

## Toolchains

- Rust stable with the intended platform target installed, and the NSH nightly
  CLI (`scripts/setup-nightly.sh`).
- macOS: Xcode command line tools.
- Linux: GTK 4.14 or newer development libraries and clang (Ubuntu 24.04:
  `sudo apt-get install libgtk-4-dev clang`).
- Windows: Visual Studio C++ build tools and the Windows SDK.
- iOS 15 or newer: Xcode on macOS and an installed simulator runtime; device distribution
  additionally needs signing credentials and a provisioning profile.
- Android 9 (API 28) or newer: Android SDK, NDK, JDK, and an emulator or USB-debuggable device.

For a signed iOS device package:

```sh
nsh package --target ios --sign 'Apple Distribution: Your Name (TEAMID)' --profile AppStore.mobileprovision
```

## Platform permissions

Desktop system data is collected locally with sysinfo on Windows, macOS and Linux.
Android uses public libc APIs for its own CPU/RSS, physical memory and `/data`
capacity; restricted interface counters remain unavailable. iOS uses public Darwin APIs directly: `getrusage(RUSAGE_SELF)` for
sysmon's CPU usage, `task_info` for sysmon's resident memory, `hw.memsize` for
physical memory, `statvfs` for the sandbox's volume, and `getifaddrs` for visible
interface counters. CPU is normalized by the logical core count. Its first
sample establishes a baseline; subsequent samples show rates.

Mobile OS sandboxing limits process enumeration, other apps' resource usage and
some device metrics. iOS and Android CPU and resident-memory values describe sysmon itself,
not the whole device. Such restricted data must be shown as unavailable rather
than represented as a successful zero measurement. Desktop permissions and
available kernel interfaces can also restrict metrics.

The workflow in `.github/workflows/platforms.yml` checks desktop libraries and
executables on each desktop OS, plus the shared mobile library for iOS devices,
iOS simulators, Android arm64 and Android x86_64. These compile checks do not
prove device runtime behavior, installer signing, or store acceptance; use the
development commands above for runtime verification on each target.

## Local build validation

Before the move to TSX, the 0.1.0 Rust source passed `cargo check` for macOS,
Windows (MinGW-w64 cross-check), Linux, iOS devices, iOS simulators and Android
on 2026-10-23. The TSX app passes `nsh check`, `nsh build` and `nsh test` on
Linux. Run the commands in the README's Checks section for each target again
after a change.

These results verify compilation and headless behavior. They do not claim
Windows GUI execution, mobile device execution, signed installers, or a
completed GitHub Actions run.

The README screenshots show the native macOS host at desktop and compact sizes.
