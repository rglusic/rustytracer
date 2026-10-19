#!/usr/bin/env python3
"""Measure linear-radiance error against an analytical spherical-light solution and optional PBRT v3."""
import argparse
import json
from pathlib import Path
import statistics
from benchmark_common import DEFAULT_BINARY, ROOT, render_report, run_command, write_json


def render(binary, scene, output, backend, traversal, samples, seed, reference=None, mis=True):
    return render_report(
        binary, scene, output, backend=backend, traversal=traversal,
        width=16, height=16, samples=samples, seed=seed,
        reference=reference, mis=mis, quiet=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=DEFAULT_BINARY)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/physics-validation')
    parser.add_argument('--pbrt', type=Path, help='PBRT v3 executable, built in RGB mode')
    parser.add_argument('--cpu-only', action='store_true')
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    binary = args.binary.resolve()
    camera = {'lookfrom': [0, 0, 0], 'lookat': [0, 0, -1], 'fov': 0.001}
    # The primary hit is almost exactly the point (0,0,-1). For the on-axis sphere,
    # L = albedo * Le * radius^2 / distance^2 = 0.5 * 4 / 16 = 0.125.
    # Across this tiny field of view the analytical variation is below 1e-10.
    scene = {'camera': camera, 'planes': [{'origin': [0, 0, -1], 'normal': [0, 0, 1],
             'mat': {'kind': 'diffuse', 'albedo': [0.5] * 3}}],
             'spheres': [{'center': [0, 0, 3], 'radius': 1,
                          'mat': {'kind': 'emitter', 'emission': [4] * 3}}]}
    reference_scene = {'camera': camera, 'planes': [{'origin': [0, 0, -1], 'normal': [0, 0, 1],
                      'mat': {'kind': 'emitter', 'emission': [0.125] * 3}}], 'spheres': []}
    scene_path = out / 'sphere-light.json'
    reference_path = out / 'reference-scene.json'
    scene_path.write_text(json.dumps(scene, indent=2) + '\n')
    reference_path.write_text(json.dumps(reference_scene, indent=2) + '\n')
    reference = out / 'analytic.exr'
    render(binary, reference_path, reference, 'cpu', 'linear', 1, 1)
    modes = [('cpu', 'linear')]
    if not args.cpu_only:
        modes += [('metal', 'linear'), ('metal', 'hardware')]
    results = []
    for backend, traversal in modes:
        for mis in [False, True]:
            for samples in [16, 64, 256, 1024]:
                records = []
                for seed in [1, 2, 3, 4]:
                    name = f'{backend}-{traversal}-mis-{mis}-spp-{samples}-seed-{seed}'
                    report = render(binary, scene_path, out / f'{name}.exr', backend, traversal,
                                    samples, seed, reference=reference, mis=mis)
                    records.append(report['runs'][0])
                result = {'backend': backend, 'traversal': traversal, 'mis': mis, 'samples': samples,
                          'seeds': [1, 2, 3, 4], 'runs': records,
                          'mean_mse': statistics.mean(r['radiance_mse'] for r in records),
                          'median_render_seconds': statistics.median(r['render_seconds'] for r in records)}
                results.append(result)
                print(f'{backend:5} {traversal:8} MIS={str(mis):5} spp={samples:4} '
                      f'MSE={result["mean_mse"]:.8g}, render={result["median_render_seconds"]:.6f}s', flush=True)
    for backend, traversal in modes:
        final = [r for r in results if r['backend'] == backend and r['traversal'] == traversal and r['samples'] == 1024]
        assert final[1]['mean_mse'] < final[0]['mean_mse'], 'MIS did not reduce measured MSE'
        assert final[1]['mean_mse'] < 0.0001, 'Analytical light result outside expected Monte Carlo error'
    summary = {'integrator': 'physical-rgb-v1', 'analytical_radiance': [0.125] * 3,
               'resolution': [16, 16], 'field_of_view_degrees': 0.001, 'results': results}
    if args.pbrt:
        pbrt_scene = (ROOT / 'validation/sphere-light.pbrt').read_text()
        pbrt_output = out / 'pbrt.exr'
        pbrt_path = out / 'sphere-light.pbrt'
        pbrt_path.write_text(pbrt_scene.replace('REFERENCE_OUTPUT.exr', str(pbrt_output)))
        run_command([args.pbrt.resolve(), '--quiet', '--nthreads', '8', pbrt_path], quiet=True)
        report = render(binary, scene_path, out / 'rustytracer-vs-pbrt.exr', 'cpu', 'linear',
                        32768, 42, reference=pbrt_output)
        mse = report['runs'][0]['radiance_mse']
        assert mse < 0.00001, f'PBRT reference error: {mse}'
        summary['pbrt_comparison'] = {'pbrt_version': 3, 'pbrt_samples': 32768,
                                      'rustytracer_samples': 32768, 'radiance_mse': mse}
        print(f'PBRT v3 independent reference MSE={mse:.8g}', flush=True)
    write_json(out / 'summary.json', summary)


if __name__ == '__main__':
    main()
