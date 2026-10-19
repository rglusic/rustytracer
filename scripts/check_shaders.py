#!/usr/bin/env python3
"""Compile the complete embedded Metal library, including test probes, without a GPU."""

from pathlib import Path
import subprocess


def main():
    root = Path(__file__).resolve().parents[1]
    shaders = root / "src/tracer/metal/shaders"
    output = root / "target/shader-check"
    output.mkdir(parents=True, exist_ok=True)
    source = output / "rustytracer.metal"
    source.write_text("\n".join((shaders / name).read_text() for name in
                                ["common.metal", "bsdf.metal", "integrator.metal", "linear.metal", "hardware.metal", "probes.metal"]))
    subprocess.run(["xcrun", "metal", "-std=metal3.1", "-fno-fast-math", "-c", str(source),
                    "-o", str(output / "rustytracer.air")], check=True)
    print("Metal shaders compiled successfully")


if __name__ == "__main__":
    main()
