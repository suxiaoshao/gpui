#!/usr/bin/env python3
"""Build a disposable macOS app to test .icon versus dynamic Dock images."""
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
APP = HERE.parents[3]
BUILD = Path(tempfile.mkdtemp(prefix="gupi-icon-probe-"))
BUNDLE = BUILD / "Gupi Icon Probe.app"
RESOURCES = BUNDLE / "Contents/Resources"
RESOURCES.mkdir(parents=True)
(BUNDLE / "Contents/MacOS").mkdir()


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


for name in ["Classic", "Color"]:
    icon = BUILD / f"{name}.icon"
    shutil.copytree(APP / "build-assets/icon/Gupi.icon", icon)
    if name == "Color":
        shutil.copyfile(APP / "assets/brand/logo-color.svg", icon / "Assets/logo.svg")
        config = json.loads((icon / "icon.json").read_text())
        config["groups"][0]["layers"][0].pop("fill-specializations", None)
        (icon / "icon.json").write_text(json.dumps(config, indent=2))

compiled = BUILD / "compiled"
compiled.mkdir()
run("xcrun", "actool", BUILD / "Classic.icon", BUILD / "Color.icon", "--compile", compiled,
    "--output-partial-info-plist", BUILD / "generated.plist", "--app-icon", "Classic",
    "--include-all-app-icons", "--platform", "macosx", "--target-device", "mac",
    "--minimum-deployment-target", "26.0")
for path in compiled.iterdir():
    if path.is_file():
        shutil.copyfile(path, RESOURCES / path.name)

# A raster alternate is exported from the same Icon Composer source, not a bare logo.
xcode = Path(subprocess.check_output(["xcode-select", "-p"], text=True).strip())
ictool = xcode.parent / "Applications/Icon Composer.app/Contents/Executables/ictool"
run(ictool, BUILD / "Color.icon", "--export-preview", "macOS", "Default", "512", "512", "1",
    RESOURCES / "color.png")
shutil.copyfile(APP / "assets/brand/tray-template.png", RESOURCES / "tray-template.png")
info = plistlib.loads((BUILD / "generated.plist").read_bytes())
info.update(dict(CFBundleIdentifier="top.sushao.gupi.icon-probe", CFBundleName="Gupi Icon Probe",
            CFBundleExecutable="icon-probe", CFBundlePackageType="APPL", CFBundleVersion="1",
            NSHighResolutionCapable=True, LSMinimumSystemVersion="26.0"))
(BUNDLE / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
run("xcrun", "swiftc", HERE / "main.swift", "-o", BUNDLE / "Contents/MacOS/icon-probe")
run("codesign", "--force", "--sign", "-", BUNDLE)
print(f"\nProbe app: {BUNDLE}")
print(f"Icon Composer sources: {BUILD / 'Classic.icon'} and {BUILD / 'Color.icon'}")
