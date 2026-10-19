#!/usr/bin/env python3
"""Compile the embedded Metal library and probes; optionally check or apply formatting."""

import argparse
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    formatting = parser.add_mutually_exclusive_group()
    formatting.add_argument("--check-format", action="store_true")
    formatting.add_argument("--format", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    shaders = root / "src/tracer/metal/shaders"
    names = (shaders / "sources.txt").read_text().splitlines() + ["probes.metal"]
    sources = [shaders / name for name in names if name]
    if args.check_format or args.format:
        for path in sources:
            original = path.read_text()
            formatted = subprocess.run(
                ["xcrun", "clang-format", "--style=file", f"--assume-filename={path.with_suffix('.cpp')}"],
                cwd=root, input=original, capture_output=True, text=True, check=True,
            ).stdout
            if args.format:
                path.write_text(formatted)
            elif original != formatted:
                parser.exit(1, f"{path.relative_to(root)} needs formatting; run scripts/check_shaders.py --format\n")
    output = root / "target/shader-check"
    output.mkdir(parents=True, exist_ok=True)
    source = output / "rustytracer.metal"
    source.write_text("\n".join(path.read_text() for path in sources))
    subprocess.run(
        ["xcrun", "metal", "-std=metal3.1", "-fno-fast-math",
         f"-fmodules-cache-path={output / 'module-cache'}", "-c", str(source),
         "-o", str(output / "rustytracer.air")], check=True,
    )
    print("Metal shaders compiled successfully")


if __name__ == "__main__":
    main()
