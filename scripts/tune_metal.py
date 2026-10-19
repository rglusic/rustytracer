#!/usr/bin/env python3
"""Sweep Metal sample batch and threadgroup sizes sequentially."""

import argparse
import json
from pathlib import Path
import subprocess


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scene", type=Path, default=root / "worlds/closed_room.json")
    parser.add_argument("--traversal", choices=["linear", "hardware"], default="linear")
    parser.add_argument("--output", type=Path, default=root / "target/tuning")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    results = []
    for batch in [16, 32, 64]:
        for threads in [64, 128, 256]:
            stem = args.output / f"batch-{batch}-threads-{threads}"
            command = [str(root / "target/release/rustytracer"), "--backend", "metal",
                       "--scene", str(args.scene), "--traversal", args.traversal,
                       "--width", "1920", "--height", "1080", "--samples", "128",
                       "--seed", "42", "--batch-size", str(batch),
                       "--threadgroup-size", str(threads), "--benchmark", "3", "--warmup", "1",
                       "--output", str(stem.with_suffix(".png")),
                       "--metrics", str(stem.with_suffix(".json"))]
            subprocess.run(command, cwd=root, check=True)
            results.append(json.loads(stem.with_suffix(".json").read_text()))
    results.sort(key=lambda report: report["median_render_and_encoding_seconds"])
    (args.output / "tuning.json").write_text(json.dumps(results, indent=2) + "\n")
    for result in results:
        settings = result["settings"]
        print(f"batch={settings['batch_size']:2d}, threads={settings['threadgroup_size']:3d}: "
              f"{result['median_render_and_encoding_seconds']:.6f}s")


if __name__ == "__main__":
    main()
