#!/usr/bin/env python3
"""Build an installable, self-contained nsh app for the current platform."""
import argparse, json, hashlib, os, platform, plistlib, shutil, subprocess, sys, tarfile
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
def run(*command,**kwargs):
    subprocess.run(command,cwd=ROOT,check=True,**kwargs)
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--version', required=True)
    args=parser.parse_args()
    version=args.version.removeprefix('v')
    import tomllib
    package_version=tomllib.loads((ROOT/'Cargo.toml').read_text())['package']['version']
    if version != package_version or json.loads((ROOT/'nsh.json').read_text())['version'] != version:
        raise SystemExit('Release version must match Cargo.toml and nsh.json')
    system=platform.system()
    if system not in ('Darwin','Linux'):raise SystemExit('Build on macOS or Linux')
    if platform.machine() not in ('arm64', 'aarch64', 'x86_64', 'amd64'):
        raise SystemExit('Unsupported CPU architecture')
    target=f"{'aarch64' if platform.machine() in ('arm64','aarch64') else 'x86_64'}-{'apple-darwin' if system=='Darwin' else 'unknown-linux-gnu'}"
    build=ROOT/'.nsh/package'
    build.mkdir(parents=True,exist_ok=True)
    env={**os.environ,'DISKTREE_VERSION':version}
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--format-version','1','--locked'],cwd=ROOT,env=env,text=True))
    cargo=Path(metadata['target_directory'])
    run('cargo','build','--manifest-path',str(ROOT/'Cargo.toml'),'--release','--locked','--no-default-features',env=env)
    stage=build/f'sysmon-{version}-{target}'
    if stage.exists():shutil.rmtree(stage)
    stage.mkdir()
    if system=='Darwin':
        app=stage/'sysmon.app/Contents'
        (app/'MacOS').mkdir(parents=True)
        resources=app/'Resources';resources.mkdir()
        shutil.copy2(cargo/'release/sysmon',app/'MacOS/sysmon')
        with (app/'Info.plist').open('wb') as file:
            plistlib.dump(dict(CFBundleExecutable='sysmon',CFBundleName='sysmon',CFBundleIconFile='AppIcon',CFBundleIdentifier='dev.sysmon.app',CFBundleShortVersionString=version,CFBundleVersion=version,CFBundlePackageType='APPL',LSMinimumSystemVersion='14.0',NSHighResolutionCapable=True),file)
        run('sips','-s','format','png',str(ROOT/'assets/icon.svg'),'--out',str(build/'icon.png'))
        iconset=build/'AppIcon.iconset'
        if iconset.exists():shutil.rmtree(iconset)
        iconset.mkdir()
        for size in [16,32,128,256,512]:
            for scale in [1,2]:
                name=f'icon_{size}x{size}'+('@2x' if scale==2 else '')+'.png'
                run('sips','-z',str(size*scale),str(size*scale),str(build/'icon.png'),'--out',str(iconset/name))
        run('iconutil','-c','icns',str(iconset),'-o',str(resources/'AppIcon.icns'))
        launcher=stage/'sysmon'
        launcher.write_text('''#!/bin/sh
set -eu
launcher=$0
while [ -L "$launcher" ]; do
  link=$(readlink "$launcher")
  case "$link" in /*) launcher=$link;; *) launcher=$(dirname "$launcher")/$link;; esac
done
root=$(CDPATH= cd -- "$(dirname -- "$launcher")" && pwd)
exec "$root/sysmon.app/Contents/MacOS/sysmon" "$@"
''')
        launcher.chmod(0o755)
    else:
        resources=stage/'resources';resources.mkdir()
        shutil.copy2(cargo/'release/sysmon',stage/'sysmon')
        (stage/'sysmon.desktop').write_text('[Desktop Entry]\nType=Application\nName=sysmon\nComment=Monitor local system resources\nExec=sysmon\nTerminal=false\nCategories=System;Utility;\nIcon=sysmon\nStartupWMClass=dev.sysmon.app\n')
    shutil.copy2(ROOT/'assets/icon.svg',resources/'icon.svg')
    shutil.copy2(ROOT/'LICENSE',stage/'LICENSE')
    runtime=next(p for p in metadata['packages'] if p['name']=='nsh-core')
    if runtime['license'] != 'Apache-2.0':
        raise SystemExit('Review NSH license before packaging')
    shutil.copy2(ROOT/'assets/LICENSE.nsh',stage/'LICENSE.nsh')
    if system=='Darwin':run('codesign','--force','--deep','--sign','-','--timestamp=none',str(stage/'sysmon.app'))
    # Validate the CLI wrapper before archiving; smoke-package checks native scanning.
    run(str(stage/'sysmon'),'--version')
    dist=ROOT/'.nsh/dist';dist.mkdir(exist_ok=True)
    archive=dist/f'sysmon-v{version}-{target}.tar.gz'
    with tarfile.open(archive,'w:gz') as tar:
        for child in sorted(stage.iterdir()):tar.add(child,arcname=child.name)
    print('Built',archive)
    print('SHA256',hashlib.sha256(archive.read_bytes()).hexdigest())
if __name__=='__main__':main()
