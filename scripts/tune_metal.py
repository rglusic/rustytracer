#!/usr/bin/env python3
"""Sweep Metal sample batch and threadgroup sizes sequentially."""

import argparse
import json
from pathlib import Path
import subprocess


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=root / "target/release/rustytracer")
    parser.add_argument("--scene", type=Path, default=root / "worlds/physical_room.json")
    parser.add_argument("--traversal", choices=["linear", "hardware"], default="linear")
    parser.add_argument("--output", type=Path, default=root / "target/tuning")
    parser.add_argument("--width", type=int, default=1920)
    parser.add_argument("--height", type=int, default=1080)
    parser.add_argument("--samples", type=int, default=128)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--warmup", type=int, default=1)
    parser.add_argument("--batches", type=int, nargs="+", default=[16, 32, 64])
    parser.add_argument("--threads", type=int, nargs="+", default=[64, 128, 256])
    args = parser.parse_args()
    if not (1 <= args.repeats <= 100 and 0 <= args.warmup <= 100):
        parser.error("repeats must be 1..100 and warmup 0..100")
    if not all(1 <= value <= 2**32 - 1 for value in [args.width, args.height, args.samples]):
        parser.error("dimensions and samples must be positive u32 values")
    if not 0 <= args.seed <= 2**32 - 1:
        parser.error("seed must be a u32 value")
    if not all(1 <= value <= 1024 for value in args.batches + args.threads):
        parser.error("batch and threadgroup sizes must be 1..1024")
    args.output.mkdir(parents=True, exist_ok=True)
    results = []
    for batch in args.batches:
        for threads in args.threads:
            stem = args.output / f"batch-{batch}-threads-{threads}"
            command = [str(args.binary.resolve()), "--backend", "metal",
                       "--scene", str(args.scene), "--traversal", args.traversal,
                       "--width", str(args.width), "--height", str(args.height), "--samples", str(args.samples),
                       "--seed", str(args.seed), "--batch-size", str(batch),
                       "--threadgroup-size", str(threads), "--benchmark", str(args.repeats), "--warmup", str(args.warmup),
                       "--output", str(stem.with_suffix(".png")),
                       "--metrics", str(stem.with_suffix(".json"))]
            subprocess.run(command, cwd=root, check=True)
            report = json.loads(stem.with_suffix(".json").read_text())
            report["command"] = command
            results.append(report)
    results.sort(key=lambda report: report["median_render_and_encoding_seconds"])
    (args.output / "tuning.json").write_text(json.dumps(results, indent=2) + "\n")
    for result in results:
        settings = result["settings"]
        print(f"batch={settings['batch_size']:2d}, threads={settings['threadgroup_size']:3d}: "
              f"{result['median_render_and_encoding_seconds']:.6f}s")


if __name__ == "__main__":
    main()
