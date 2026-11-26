"""Measure an isolated release app; record permission state and exact scope.
Run with an existing Python via mise. Never grants Accessibility permission.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument("--binary")
parser.add_argument("--synthetic", choices=["static", "animated"])
parser.add_argument("--seconds", type=float, default=60)
parser.add_argument("--with-app", action="store_true")
parser.add_argument("--output", required=True)
args = parser.parse_args()
if args.seconds < 1:
    parser.error("--seconds must be at least 1")
binary = str(Path(args.binary or ("target/release/examples/idle_fixture" if args.synthetic else "dist/Pipmark.app/Contents/MacOS/Pipmark")).resolve())

def stats(pid):
    fields = subprocess.check_output(["ps", "-p", str(pid), "-o", "time=,rss="], text=True).split()
    parts = fields[0].split(":")
    cpu = sum(float(value) * 60 ** power for power, value in enumerate(reversed(parts)))
    return cpu, int(fields[1])

with tempfile.TemporaryDirectory(prefix="badgeify-idle-") as directory:
    config = Path(directory) / "settings.json"
    items = [{"id": "com.apple.finder", "name": "Finder", "enabled": True,
              "type": "app", "path": "/System/Library/CoreServices/Finder.app"}] if args.with_app else []
    config.write_text(json.dumps({"show_on_start": False, "items": items}))
    env = {**os.environ, "BADGEIFY_CONFIG": str(config)}
    if args.synthetic:
        diagnostic = {"success": False, "not_probed": "synthetic data; no AX calls"}
        command = [binary, args.synthetic, str(args.seconds + 6)]
    else:
        permission = subprocess.run([binary, "status", "--id", "com.apple.finder"], env=env, capture_output=True, text=True)
        diagnostic = json.loads(permission.stdout or permission.stderr)
        command = [binary]
    process = subprocess.Popen(command, env=env, stdout=subprocess.PIPE if args.synthetic else subprocess.DEVNULL, stderr=subprocess.PIPE)
    try:
        time.sleep(5)
        if process.poll() is not None:
            raise RuntimeError(process.stderr.read().decode())
        start_cpu, start_rss = stats(process.pid)
        start = time.monotonic()
        peak_rss = start_rss
        while time.monotonic() - start < args.seconds:
            time.sleep(max(0, min(5, args.seconds - (time.monotonic() - start))))
            if process.poll() is not None:
                raise RuntimeError("App exited during idle measurement")
            end_cpu, rss = stats(process.pid)
            peak_rss = max(peak_rss, rss)
        elapsed = time.monotonic() - start
        report = {"binary": binary, "duration_seconds": elapsed,
                  "configured_apps": 4 if args.synthetic else len(items), "settings_window": "closed",
                  "status_probe": diagnostic, "cpu_seconds_delta": end_cpu - start_cpu,
                  "average_cpu_percent": 100 * (end_cpu - start_cpu) / elapsed,
                  "peak_sampled_rss_mib": peak_rss / 1024,
                  "measurement": "ps cumulative CPU time delta, 0.01 second resolution; 5 second warmup",
                  "limitation": "No badge changes generated; no successful AX observation unless status_probe.success is true"}
        if args.synthetic:
            stdout, stderr = process.communicate(timeout=10)
            if process.returncode != 0:
                raise RuntimeError(stderr.decode())
            report["fixture"] = json.loads(stdout)
            report["limitation"] = "Synthetic unread data; production AppKit controls, timer and worker, but no AX IPC. Compositor visibility depends on the desktop session."
        Path(args.output).write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2), flush=True)
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
