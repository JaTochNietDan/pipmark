"""Sign a separate local release copy using an existing Developer ID identity."""
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
source = root / "dist/Pipmark.app"
destination = root / "dist/developer-id/Pipmark.app"
identities = subprocess.check_output(["security", "find-identity", "-v", "-p", "codesigning"], text=True)
matching = [match.group(1) for line in identities.splitlines()
            if "Developer ID Application:" in line
            for match in [re.search(r"\b([0-9A-F]{40})\b", line)] if match]
identity = os.environ.get("BADGEIFY_SIGN_IDENTITY")
if identity is None:
    if len(matching) != 1:
        raise SystemExit("Set BADGEIFY_SIGN_IDENTITY to an existing Developer ID identity SHA-1.")
    identity = matching[0]
if identity.upper() not in matching:
    raise SystemExit("BADGEIFY_SIGN_IDENTITY must identify an available Developer ID Application identity.")
subprocess.run(["codesign", "--verify", "--deep", "--strict", str(source)], check=True)
destination.parent.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix="signing-", dir=destination.parent) as folder:
    candidate = Path(folder) / source.name
    subprocess.run(["ditto", str(source), str(candidate)], check=True)
    framework = candidate / "Contents/Frameworks/Sparkle.framework/Versions/B"
    # Sign nested code inside-out using our identity for hardened-runtime library validation.
    for nested in [framework / "XPCServices/Downloader.xpc",
                   framework / "XPCServices/Installer.xpc",
                   framework / "Updater.app", framework / "Autoupdate",
                   candidate / "Contents/Frameworks/Sparkle.framework", candidate]:
        subprocess.run(["codesign", "--force", "--options", "runtime", "--timestamp",
                        "--sign", identity, str(nested)], check=True, timeout=30)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(candidate)], check=True)
    details = subprocess.run(["codesign", "-d", "--verbose=4", str(candidate)],
                             check=True, capture_output=True, text=True).stderr
    if not all(value in details for value in ["Authority=Developer ID Application:", "(runtime)", "Timestamp="]):
        raise RuntimeError("Release signing requirements were not met")
    previous = Path(folder) / "previous.app"
    if destination.exists():
        os.replace(destination, previous)
    try:
        os.replace(candidate, destination)
    except BaseException:
        if previous.exists():
            os.replace(previous, destination)
        raise
print(json.dumps({"bundle": str(destination), "developer_id_signed": True,
                  "hardened_runtime": True, "secure_timestamp": True,
                  "signature_verified": True, "notarization": "not_submitted"}, indent=2))
