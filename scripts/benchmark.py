#!/usr/bin/env python3
"""Run sequential CPU/Metal benchmarks and generate reproducible sphere scenes."""

import argparse
import json
import math
import platform
from pathlib import Path
from benchmark_common import DEFAULT_BINARY, ROOT, render_report, write_json


def sphere_scene(count):
    scene = json.loads((ROOT / "worlds/physical_room.json").read_text())
    light = scene["spheres"][1]
    side = math.ceil((count - 1) ** (1 / 3))
    spheres = [light]
    spacing = 8.0 / max(side, 1)
    for index in range(count - 1):
        x = index % side
        y = (index // side) % side
        z = index // (side * side)
        spheres.append({
            "center": [-4.0 + (x + 0.5) * spacing,
                       -1.5 + (y + 0.5) * spacing,
                       -4.0 + (z + 0.5) * spacing],
            "radius": spacing * 0.30,
            "mat": [{"kind": "diffuse", "albedo": [0.75, 0.85, 0.95]},
                    {"kind": "conductor", "eta": [0.2, 0.9, 1.1], "k": [3.9, 2.5, 2.1], "roughness": 0.3},
                    {"kind": "dielectric", "ior": 1.5, "absorption": [0.1, 0.02, 0.01]}][index % 3],
        })
    scene["spheres"] = spheres
    return scene


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--output", type=Path, default=ROOT / "target/performance-physics")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--warmup", type=int, default=1)
    parser.add_argument("--width", type=int, default=1920)
    parser.add_argument("--height", type=int, default=1080)
    parser.add_argument("--samples", type=int, default=256)
    parser.add_argument("--spheres", type=int, nargs="+", default=[16, 64, 256, 1024, 4096])
    parser.add_argument("--skip-room", action="store_true")
    parser.add_argument("--generate-only", action="store_true")
    args = parser.parse_args()
    if not (1 <= args.repeats <= 100 and 0 <= args.warmup <= 100):
        parser.error("repeats must be 1..100 and warmup 0..100")
    if min(args.width, args.height, args.samples) <= 0 or min(args.spheres) < 2:
        parser.error("dimensions and samples must be positive; sphere counts must be at least two")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    cases = []
    if not args.skip_room:
        cases.append(("room", ROOT / "worlds/physical_room.json", args.width, args.height, args.samples))
    for count in args.spheres:
        scene = args.output / f"spheres-{count}.json"
        scene.write_text(json.dumps(sphere_scene(count), indent=2) + "\n")
        cases.append((f"spheres-{count}", scene, 256, 144, 16))
    if args.generate_only:
        return
    results = []
    for name, scene, width, height, samples in cases:
        modes = [("metal", "linear"), ("metal", "hardware")]
        if name == "room":
            modes.insert(0, ("cpu", "linear"))
        for backend, traversal in modes:
            stem = f"{name}-{backend}-{traversal}"
            report = render_report(
                args.binary, scene, args.output / f"{stem}.png",
                backend=backend, traversal=traversal, width=width, height=height,
                samples=samples, repeats=args.repeats, warmup=args.warmup,
            )
            report["case"] = name
            results.append(report)
    summary = {"platform": platform.platform(), "results": results}
    write_json(args.output / "summary.json", summary)
    for report in results:
        print(f"{report['case']:14s} {report['backend']:5s} {report['traversal']:8s} "
              f"median {report['median_render_and_encoding_seconds']:.6f}s")


if __name__ == "__main__":
    main()
