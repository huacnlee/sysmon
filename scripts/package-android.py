#!/usr/bin/env python3
"""Package Android with the checked NSH status-bar fix, without editing the SDK.

Requires Python 3.11+, Git, Cargo, JDK 17+, Android SDK/NDK, and the CLI built
from the same pinned NSH revision. Unknown arguments go to `nsh package`.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
PATCH = ROOT / "patches/nsh-android-status-bar.patch"
ABIS = {
    "arm64-v8a": "aarch64-linux-android28-clang",
    "armeabi-v7a": "armv7a-linux-androideabi28-clang",
    "x86_64": "x86_64-linux-android28-clang",
}


def run(*args, **kwargs):
    return subprocess.run([str(a) for a in args], cwd=ROOT, check=True, **kwargs)


def nsh_source():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT, text=True))
    cores = [p for p in metadata["packages"] if p["name"] == "nsh-core"]
    if len(cores) != 1:
        raise RuntimeError("Expected exactly one pinned nsh-core dependency")
    core = cores[0]
    source = core.get("source") or ""
    revision = source.rsplit("#", 1)[-1]
    if not source.startswith("git+") or not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise RuntimeError("This workaround requires sysmon's pinned Git NSH dependency")
    checkout = Path(core["manifest_path"]).resolve().parents[2]
    if not (checkout / "crates/nsh-platform-android/java").is_dir():
        raise RuntimeError("NSH checkout layout changed; review the Android workaround")
    return checkout, revision, core["version"]


def matching_cli(checkout, revision, version, requested):
    suffix = ".exe" if os.name == "nt" else ""
    candidates = [requested] if requested else [
        checkout / f"target/release/nsh{suffix}",
        checkout / f"target/debug/nsh{suffix}",
        shutil.which("nsh"),
    ]
    for candidate in candidates:
        if not candidate or not Path(candidate).is_file():
            continue
        output = subprocess.check_output([str(candidate), "--version"], text=True).strip()
        match = re.fullmatch(r"nsh ([^ ]+) \(([0-9a-f]{7,40})\)", output)
        if match and match[1] == version and revision.startswith(match[2]):
            return Path(candidate).resolve()
    raise RuntimeError(
        f"Install the matching NSH CLI or pass --nsh PATH:\n"
        f"cargo install --git https://github.com/longbridge/nsh --rev {revision} --locked nsh-cli")


def version_key(path):
    return tuple(int(n) for n in re.findall(r"\d+", path.name))


def android_sdk(abis):
    configured = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    defaults = [Path.home() / "Library/Android/sdk", Path.home() / "Android/Sdk"]
    if os.environ.get("LOCALAPPDATA"):
        defaults.append(Path(os.environ["LOCALAPPDATA"]) / "Android/Sdk")
    sdk = Path(configured) if configured else next((p for p in defaults if p.is_dir()), None)
    if not sdk or not sdk.is_dir():
        raise RuntimeError("Android SDK not found; set ANDROID_HOME")
    host = "windows-x86_64" if os.name == "nt" else (
        "darwin-x86_64" if sys.platform == "darwin" else "linux-x86_64")
    compiler_suffix = ".cmd" if os.name == "nt" else ""
    def valid_ndk(path):
        tools = path / "toolchains/llvm/prebuilt" / host / "bin"
        return all((tools / (ABIS[abi] + compiler_suffix)).is_file() for abi in abis)
    explicit = os.environ.get("ANDROID_NDK_HOME")
    candidates = [Path(explicit)] if explicit else sorted(
        (sdk / "ndk").glob("*"), key=version_key, reverse=True)
    ndk = next((p for p in candidates if valid_ndk(p)), None)
    if not ndk:
        raise RuntimeError("No usable Android NDK; set ANDROID_NDK_HOME to a complete NDK installation")
    d8_name = "d8.bat" if os.name == "nt" else "d8"
    build_tools = sorted((sdk / "build-tools").glob("*"), key=version_key, reverse=True)
    d8 = next((p / d8_name for p in build_tools if (p / d8_name).is_file()), None)
    if not d8:
        raise RuntimeError("Android SDK build-tools not found")
    return sdk.resolve(), ndk.resolve(), d8.resolve()


def build_runtime(checkout, work, sdk, d8, abis):
    java = work / "java"
    shutil.copytree(checkout / "crates/nsh-platform-android/java", java)
    # Normalize the copy for checked patching on Git/Windows CRLF checkouts.
    for path in java.rglob("*.java"):
        path.write_text(path.read_text(encoding="utf-8"), encoding="utf-8", newline="\n")
    # Apply only to the ignored copy. Fail closed if an NSH update changes the
    # patch context, rather than silently shipping a partly applied workaround.
    directory = work.relative_to(ROOT).as_posix()
    run("git", "apply", "--check", "--directory", directory, PATCH)
    run("git", "apply", "--directory", directory, PATCH)
    upstream = (checkout / "crates/xtask/src/android.rs").read_text(encoding="utf-8")
    compile_sdk = re.search(r"const COMPILE_SDK: u32 = (\d+);", upstream)
    minimum_sdk = re.search(r"const MIN_SDK: u32 = (\d+);", upstream)
    if not compile_sdk or not minimum_sdk or minimum_sdk[1] != "28":
        raise RuntimeError("NSH Android SDK requirements changed; review this helper")
    android_jar = sdk / f"platforms/android-{compile_sdk[1]}/android.jar"
    if not android_jar.is_file():
        raise RuntimeError(f"Install Android SDK platform android-{compile_sdk[1]}")
    runtime_root = work / "runtime"
    output = runtime_root / "android/release"
    classes = output / "classes"
    classes.mkdir(parents=True)
    run("javac", "--release", "17", "-Xlint:-options", "-encoding", "UTF-8",
        "-classpath", android_jar, "-d", classes, *sorted(java.rglob("*.java")))
    run("jar", "cf", output / "classes.jar", "-C", classes, ".")
    run(d8, "--min-api", minimum_sdk[1], "--lib", android_jar,
        "--output", output, output / "classes.jar")
    # NSH uses these directory names to select ABIs. It builds the actual app
    # libnsh.so from the original pinned Rust crates; no substitute library ships.
    for abi in abis:
        (output / "lib" / abi).mkdir(parents=True)
    return runtime_root


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--nsh", type=Path, help="CLI executable from the pinned NSH revision")
    parser.add_argument("--abi", action="append", choices=ABIS,
                        help="Android ABI; repeat to include more (default: arm64-v8a)")
    options, forwarded = parser.parse_known_args()
    if forwarded[:1] == ["--"]:
        forwarded.pop(0)
    if any(a == "--target" or a.startswith("--target=") for a in forwarded):
        parser.error("Android target is selected by this helper")
    abis = list(dict.fromkeys(options.abi or ["arm64-v8a"]))
    checkout, revision, version = nsh_source()
    cli = matching_cli(checkout, revision, version, options.nsh)
    sdk, ndk, d8 = android_sdk(abis)
    cache = ROOT / ".nsh/android-runtime"
    cache.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=f"{revision[:7]}-", dir=cache) as temporary:
        runtime = build_runtime(checkout, Path(temporary), sdk, d8, abis)
        env = {**os.environ, "ANDROID_HOME": str(sdk), "ANDROID_NDK_HOME": str(ndk),
               "NSH_RUNTIME": str(runtime)}
        print(f"Packaging with NSH {revision[:7]}, patched Java and {', '.join(abis)}", flush=True)
        run(cli, "package", ROOT, "--target", "android", *forwarded, env=env)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        # Forwarded signing options may contain passwords. Never echo argv.
        command = error.cmd[0] if isinstance(error.cmd, (list, tuple)) else "command"
        print(f"error: {Path(command).name} exited with status {error.returncode}", file=sys.stderr)
        sys.exit(1)
    except (RuntimeError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
