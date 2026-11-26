"""Create and round-trip verify a local release archive; never uploads anything."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument("--bundle", default=str(root / "dist/Pipmark.app"))
args = parser.parse_args()
bundle = Path(args.bundle).resolve()

def verify(app):
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
    with (app / "Contents/Info.plist").open("rb") as file:
        info = plistlib.load(file)
    executable = app / "Contents/MacOS" / info["CFBundleExecutable"]
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise RuntimeError("Missing executable or executable permission")
    icon = app / "Contents/Resources" / (info["CFBundleIconFile"] + ".icns")
    if not icon.is_file():
        raise RuntimeError("Missing application icon")
    help_text = subprocess.check_output([str(executable), "--help"], text=True)
    if "status --id" not in help_text:
        raise RuntimeError("Packaged executable failed the CLI smoke check")
    return info, executable

info, executable = verify(bundle)
architectures = subprocess.check_output(["lipo", "-archs", str(executable)], text=True).split()
architecture = "universal" if len(architectures) > 1 else architectures[0]
version = info["CFBundleShortVersionString"]
# Metadata becomes a filename only after validation.
if not version or any(c not in "0123456789.-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ" for c in version):
    raise RuntimeError("Version is not suitable for a release filename")
signature = subprocess.run(["codesign", "-d", "--verbose=4", str(bundle)], check=True, capture_output=True, text=True).stderr
signing = "ad-hoc" if "Signature=adhoc" in signature else "developer-id" if "Authority=Developer ID Application:" in signature else "certificate"
name = f"pipmark-{version}-{architecture}" + ("-developer-id" if signing == "developer-id" else "-signed" if signing == "certificate" else "")
destination = root / "dist" / f"{name}.zip"
with tempfile.TemporaryDirectory(prefix="badgeify-package-", dir=root / "dist") as temporary:
    temporary = Path(temporary)
    archive = temporary / destination.name
    subprocess.run(["ditto", "-c", "-k", "--keepParent", str(bundle), str(archive)], check=True)
    extracted = temporary / "extracted"
    subprocess.run(["ditto", "-x", "-k", str(archive), str(extracted)], check=True)
    restored_info, restored_executable = verify(extracted / bundle.name)
    if restored_info != info or hashlib.sha256(restored_executable.read_bytes()).digest() != hashlib.sha256(executable.read_bytes()).digest():
        raise RuntimeError("Archive round trip changed bundle metadata or executable")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    notarized = subprocess.run(["xcrun", "stapler", "validate", str(extracted / bundle.name)], capture_output=True).returncode == 0
    manifest = {
        "archive": destination.name,
        "version": version,
        "bundle_identifier": info["CFBundleIdentifier"],
        "minimum_macos": info["LSMinimumSystemVersion"],
        "architectures": architectures,
        "bytes": archive.stat().st_size,
        "sha256": digest,
        "signing": signing,
        "notarization": "stapled_and_verified" if notarized else "not_verified",
        "verification": ["original signature", "extracted signature", "executable SHA-256 equality", "extracted CLI --help"],
    }
    manifest_file = temporary / f"{name}.json"
    manifest_file.write_text(json.dumps(manifest, indent=2) + "\n")
    checksum_file = temporary / f"{name}.zip.sha256"
    checksum_file.write_text(f"{digest}  {destination.name}\n")
    os.replace(archive, destination)
    os.replace(manifest_file, destination.with_suffix(".json"))
    os.replace(checksum_file, destination.with_suffix(".zip.sha256"))
print(json.dumps(manifest, indent=2))
