"""Shared renderer invocation and report handling for benchmark tools."""

import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "target/release/rustytracer"


def run_command(command, *, quiet=False):
    command = [str(value) for value in command]
    result = subprocess.run(command, cwd=ROOT, capture_output=quiet, text=True)
    if result.returncode:
        details = f"\n{result.stdout}\n{result.stderr}" if quiet else ""
        raise RuntimeError(f"Command failed ({result.returncode}): {command}{details}")
    return result


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def render_report(binary, scene, output, *, backend, traversal, width, height,
                  samples, seed=42, repeats=1, warmup=0, batch=None, threads=None,
                  reference=None, mis=True, quiet=False):
    output = output.resolve()
    metrics = output.with_suffix(".json")
    command = [
        binary.resolve(), "--scene", scene.resolve(),
        "--backend", backend, "--traversal", traversal,
        "--width", width, "--height", height, "--samples", samples, "--seed", seed,
        "--benchmark", repeats, "--warmup", warmup,
        "--output", output, "--metrics", metrics,
    ]
    for flag, value in [("--batch-size", batch), ("--threadgroup-size", threads),
                        ("--reference", reference.resolve() if reference else None)]:
        if value is not None:
            command += [flag, value]
    if not mis:
        command.append("--no-light-sampling")
    run_command(command, quiet=quiet)
    report = json.loads(metrics.read_text())
    report["command"] = [str(value) for value in command]
    return report
