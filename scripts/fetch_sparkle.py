"""Fetch a pinned application dependency, verifying before extraction."""
import hashlib
from pathlib import Path
import subprocess
import urllib.request
root = Path(__file__).resolve().parent.parent
archive = root / 'target/Sparkle-2.10.1.tar.xz'
digest = '1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27'
archive.parent.mkdir(exist_ok=True)
if not archive.exists():
    urllib.request.urlretrieve('https://github.com/sparkle-project/Sparkle/releases/download/2.10.1/Sparkle-2.10.1.tar.xz', archive)
if hashlib.sha256(archive.read_bytes()).hexdigest() != digest:
    raise SystemExit('Sparkle archive checksum mismatch')
destination = root / 'target/sparkle'
destination.mkdir(exist_ok=True)
subprocess.run(['tar', '-xJf', str(archive), '-C', str(destination)], check=True)
