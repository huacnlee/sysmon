#!/usr/bin/env python3
"""Build an installable, self-contained nsh app for the current platform."""
import argparse, json, hashlib, os, platform, plistlib, shutil, subprocess, tarfile, tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
def run(*command,**kwargs):
    subprocess.run(command,cwd=ROOT,check=True,**kwargs)

LAUNCHER='''#!/bin/sh
set -eu
launcher=$0
while [ -L "$launcher" ]; do
  link=$(readlink "$launcher")
  case "$link" in /*) launcher=$link;; *) launcher=$(dirname "$launcher")/$link;; esac
done
root=$(CDPATH= cd -- "$(dirname -- "$launcher")" && pwd)
if [ "${1:-}" = --version ] || [ "${1:-}" = -V ]; then
  read -r version target < "$root/.sysmon-install"
  echo "sysmon ${version#v}"
  exit 0
fi
if [ -d "$root/sysmon.app" ]; then
  exec "$root/sysmon.app/Contents/MacOS/sysmon" "$@"
fi
exec "$root/sysmon-host" "$@"
'''

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--version',required=True)
    args=parser.parse_args()
    version=args.version.removeprefix('v')
    config=json.loads((ROOT/'nsh.json').read_text())
    if config['version']!=version:
        raise SystemExit('Release version must match nsh.json')
    system=platform.system()
    if system not in ('Darwin','Linux'):raise SystemExit('Build on macOS or Linux')
    if platform.machine() not in ('arm64','aarch64','x86_64','amd64'):
        raise SystemExit('Unsupported CPU architecture')
    arch='aarch64' if platform.machine() in ('arm64','aarch64') else 'x86_64'
    target=f"{arch}-{'apple-darwin' if system=='Darwin' else 'unknown-linux-gnu'}"
    build=ROOT/'.nsh/package'
    build.mkdir(parents=True,exist_ok=True)
    stage=build/f'sysmon-{version}-{target}'
    if stage.exists():shutil.rmtree(stage)
    with tempfile.TemporaryDirectory(dir=build) as temporary:
        output=Path(temporary)
        run('nsh','package','--target','macos' if system=='Darwin' else 'linux','--name','sysmon',
            '--app-version',version,'--out',str(output))
        if system=='Darwin':
            stage.mkdir()
            shutil.move(output/'sysmon.app',stage/'sysmon.app')
        else:
            shutil.move(output/'sysmon',stage)
    if system=='Darwin':
        app=stage/'sysmon.app/Contents'
        resources=app/'Resources'
        with (app/'Info.plist').open('rb') as file:
            info=plistlib.load(file)
        info.update(CFBundleName='sysmon',CFBundleDisplayName='sysmon',CFBundleIconFile='AppIcon',
                    CFBundleShortVersionString=version,CFBundleVersion=version,LSMinimumSystemVersion='14.0')
        with (app/'Info.plist').open('wb') as file:
            plistlib.dump(info,file)
        run('sips','-s','format','png',str(ROOT/'assets/icon.svg'),'--out',str(build/'icon.png'))
        iconset=build/'AppIcon.iconset'
        if iconset.exists():shutil.rmtree(iconset)
        iconset.mkdir()
        for size in [16,32,128,256,512]:
            for scale in [1,2]:
                name=f'icon_{size}x{size}'+('@2x' if scale==2 else '')+'.png'
                run('sips','-z',str(size*scale),str(size*scale),str(build/'icon.png'),'--out',str(iconset/name))
        run('iconutil','-c','icns',str(iconset),'-o',str(resources/'AppIcon.icns'))
        bundle=resources/'nsh'
    else:
        (stage/'sysmon').rename(stage/'sysmon-host')
        resources=stage/'resources';resources.mkdir()
        (stage/f"{config['id']}.desktop").unlink(missing_ok=True)
        (stage/'sysmon.desktop').write_text('[Desktop Entry]\nType=Application\nName=sysmon\nComment=Monitor local system resources\nExec=sysmon\nTerminal=false\nCategories=System;Utility;\nIcon=sysmon\nStartupWMClass=dev.sysmon.app\n')
        bundle=stage/'nsh'
    if not (bundle/'bundle.nshc').is_file():
        raise SystemExit('Packaged TSX bundle is missing')
    metadata=json.loads((bundle/'app.json').read_text())
    if metadata['id']!=config['id'] or metadata['version']!=version:
        raise SystemExit('Packaged app identity or version differs from the release')
    launcher=stage/'sysmon'
    launcher.write_text(LAUNCHER)
    launcher.chmod(0o755)
    (stage/'.sysmon-install').write_text(f'v{version} {target}\n')
    shutil.copy2(ROOT/'assets/icon.svg',resources/'icon.svg')
    shutil.copy2(ROOT/'LICENSE',stage/'LICENSE')
    shutil.copy2(ROOT/'assets/LICENSE.nsh',stage/'LICENSE.nsh')
    if system=='Darwin':run('codesign','--force','--deep','--sign','-','--timestamp=none',str(stage/'sysmon.app'))
    reported=subprocess.check_output([stage/'sysmon','--version'],text=True).strip()
    if reported!=f'sysmon {version}':
        raise SystemExit(f'Unexpected launcher version: {reported}')
    dist=ROOT/'.nsh/dist';dist.mkdir(exist_ok=True)
    archive=dist/f'sysmon-v{version}-{target}.tar.gz'
    with tarfile.open(archive,'w:gz') as tar:
        for child in sorted(stage.iterdir()):tar.add(child,arcname=child.name)
    print('Built',archive)
    print('SHA256',hashlib.sha256(archive.read_bytes()).hexdigest())
if __name__=='__main__':main()
