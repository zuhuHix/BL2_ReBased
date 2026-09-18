"""Cross-check BSP surface texture-axis fields against the editor's Polys exports.

Cooked packages strip the persistent-level root Model's ``Polys`` to an empty
array, but volume-owned Models (blocking, kill, post-process, cull-distance
volumes and so on) keep theirs. An FPoly record stores Base, Normal, TextureU
and TextureV as explicit vectors next to its vertex list, and it is written by
different engine code than the Model's 60-byte surface records. If our reading
of surface ints ``s[2]``, ``s[4]`` and ``s[5]`` as a point-pool index and two
vector-pool indices is right, dereferencing them must reproduce those vectors.

This tests the field roles only. It says nothing about the texel scale that
turns projected distances into texture repeats, and nothing about rendering.
Game-derived output stays under ``local/``.

The FPoly layout is not asserted from memory: each record is read as twelve
floats, a vertex count and that many vertices, followed by a fixed-size tail
whose length is recovered from the data (the only size for which every record
of the array parses with a unit normal and the array is consumed exactly). The
tail is kept as an opaque digest; because its size is inferred, an array
truncated by whole words inside that tail is indistinguishable from a shorter
tail, which cannot affect the vectors compared here. A poly's stored normal is
not trusted for matching: hand-edited brush faces keep a stale one, so the
plane a poly is matched on is computed from its own vertices.
"""
import argparse
from collections import Counter
import json
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bsp_decode import Cursor, digest, dot, sub, cross, finite, read_surfaces, surface_axes  # noqa: E402
from terrain_decode import native_offset  # noqa: E402

MAX_POLYS = 100_000
MAX_POLY_VERTICES = 1024
TAIL_LIMIT = 512
POLY_PLANE_TOLERANCE = 0.1   # cm; volume brushes sit at world coordinates up to ~1e5
POINT_TOLERANCE = 0.01       # cm; the pool merges near-identical points
VECTOR_TOLERANCE = 1e-3      # the pool merges near-identical vectors
CONTROL_SLOTS = (1, 2, 4, 5, 6, 7, 13, 14)  # every int slot of the surface record except material and normal


def vertex_normal(vertices):
    """Unit normal of a polygon from its vertices (Newell), or None when degenerate."""
    n = (0.0, 0.0, 0.0)
    for a, b in zip(vertices, vertices[1:] + vertices[:1]):
        n = tuple(x + y for x, y in zip(n, cross(a, b)))
    length = dot(n, n) ** 0.5
    return tuple(x / length for x in n) if length > 1e-9 else None


def walk_polys(region, count, tail):
    """Parse ``count`` FPoly records with a ``tail``-byte opaque suffix, or return None."""
    c = Cursor(region, 0)
    polys = []
    try:
        for _ in range(count):
            values = c.read('12f')
            n = c.read('i')[0]
            if not 3 <= n <= MAX_POLY_VERTICES:
                return None
            vertices = [c.read('3f') for _ in range(n)]
            extra = c.take(tail)
            finite(values)
            for v in vertices:
                finite(v)
            normal = values[3:6]
            if abs(dot(normal, normal) - 1) > 1e-3:
                return None
            computed = vertex_normal(vertices)
            stale = computed is None or any(abs(dot(sub(v, vertices[0]), normal)) > POLY_PLANE_TOLERANCE
                                            for v in vertices)
            polys.append({'base': values[0:3], 'normal': normal, 'texture_u': values[6:9],
                          'texture_v': values[9:12], 'vertices': vertices, 'vertex_normal': computed,
                          'stale_normal': stale, 'opaque_tail': digest(extra)})
    except ValueError:
        return None
    if c.offset != len(region):
        return None
    return polys


def decode_polys(payload, record):
    """FPoly records of a Polys export: (polys, tail_bytes). Empty arrays yield ([], None)."""
    c = Cursor(payload, native_offset(payload, record['data'], 4))
    count, capacity, owner = c.read('iii')
    if not 0 <= count <= capacity or count > MAX_POLYS or owner != record['index']:
        raise ValueError('Unsupported Polys header')
    region = payload[c.offset:]
    if count == 0:
        if region:
            raise ValueError('Unexpected Polys remainder')
        return [], None
    for tail in range(0, TAIL_LIMIT + 1, 4):
        polys = walk_polys(region, count, tail)
        if polys is not None:
            return polys, tail
    raise ValueError('No consistent FPoly record size')


def close(a, b, tolerance):
    return deviation(a, b) <= tolerance


def deviation(a, b):
    return max(abs(x - y) for x, y in zip(a, b))


def faces(poly, normal):
    """Whether either the poly's stored normal or its vertex-derived normal matches.

    Hand-edited brushes were observed with a stale stored normal, and one with a
    reversed winding but a correct stored normal, so neither alone decides.
    """
    return any(n is not None and dot(n, normal) > 0.999 for n in (poly['normal'], poly['vertex_normal']))


def matching_poly(polys, plane):
    """The single FPoly whose vertices lie on ``plane`` facing the same way, else None."""
    normal, d = plane[:3], plane[3]
    found = [p for p in polys
             if faces(p, normal)
             and all(abs(dot(v, normal) - d) <= POLY_PLANE_TOLERANCE for v in p['vertices'])]
    return found[0] if len(found) == 1 else None


def compare_model(vectors, points, surfaces, polys):
    """Per-surface status plus negative controls for the other int slots."""
    results, controls = [], Counter()
    for i, s in enumerate(surfaces):
        try:
            axes = surface_axes(s, vectors, points)
        except ValueError:
            results.append({'surface': i, 'status': 'invalid_reference'})
            continue
        poly = matching_poly(polys, s[8:12])
        if poly is None:
            # Not evidence either way: the plane match is what keeps the test
            # non-circular. The identical-axes count is reported for context only
            # (editor vertex lists were observed stale against the compiled Model).
            identical = sum(close(q['base'], axes['base'], POINT_TOLERANCE)
                            and close(q['texture_u'], axes['texture_u'], VECTOR_TOLERANCE)
                            and close(q['texture_v'], axes['texture_v'], VECTOR_TOLERANCE) for q in polys)
            results.append({'surface': i, 'status': 'no_unique_poly', 'identical_axes_polys': identical})
            continue
        deviations = {'base': deviation(axes['base'], poly['base']),
                      'texture_u': deviation(axes['texture_u'], poly['texture_u']),
                      'texture_v': deviation(axes['texture_v'], poly['texture_v'])}
        agree = (deviations['base'] <= POINT_TOLERANCE and deviations['texture_u'] <= VECTOR_TOLERANCE
                 and deviations['texture_v'] <= VECTOR_TOLERANCE)
        results.append({'surface': i, 'status': 'agree' if agree else 'differ', 'deviation': deviations})
        # Would any other int slot have reproduced the base point or an axis? A
        # coincidental match here would weaken the identification.
        for slot in CONTROL_SLOTS:
            value = s[slot]
            if 0 <= value < len(points) and close(points[value], poly['base'], POINT_TOLERANCE):
                controls['s[%d]=base' % slot] += 1
            if 0 <= value < len(vectors):
                if close(vectors[value], poly['texture_u'], VECTOR_TOLERANCE):
                    controls['s[%d]=texture_u' % slot] += 1
                if close(vectors[value], poly['texture_v'], VECTOR_TOLERANCE):
                    controls['s[%d]=texture_v' % slot] += 1
    return results, controls


class Reader:
    def __init__(self, reader, game):
        self.reader, self.game = reader, game
        self.schema = Path(__file__).with_name('terrain-arrays.schema')

    def call(self, package, *args):
        run = subprocess.run([str(self.reader), str(package), *map(str, args)],
                             capture_output=True, text=True, encoding='utf-8')
        if run.returncode:
            raise ValueError(run.stderr.strip())
        return json.loads(run.stdout)

    def packages(self, names):
        cooked = self.game / 'WillowGame/CookedPCConsole'
        if names:
            return [cooked / (n if n.lower().endswith('.upk') else n + '.upk') for n in names]
        return sorted(p for p in cooked.glob('*.upk') if not p.name.endswith('_SF.upk'))


def check_package(reader, package):
    """Compare every Model with a non-empty Polys child in one package; None if there is none."""
    exports = reader.call(package, '--exports')
    polys_exports = [e for e in exports if e['class'] == 'Engine.Polys' and e['size'] > 24]
    if not polys_exports:
        return None
    records = {r['index']: r for r in reader.call(package, '--terrain-records', reader.schema)}
    pairs = []
    for e in polys_exports:
        polys_record = records.get(e['index'])
        model_record = records.get(e['outer_index'])
        if polys_record and model_record and model_record['class'] == 'Engine.Model':
            pairs.append((model_record, polys_record))
    indices = [r['index'] for pair in pairs for r in pair]
    payloads = {r['index']: bytes(r['payload']) for r in reader.call(package, '--payloads', *indices)}
    report = {'package': package.stem, 'models': [], 'controls': Counter(), 'errors': []}
    for model_record, polys_record in pairs:
        entry = {'model': model_record['path'], 'polys': polys_record['path']}
        try:
            vectors, points, nodes, surfaces = read_surfaces(payloads[model_record['index']], model_record)
            polys, tail = decode_polys(payloads[polys_record['index']], polys_record)
        except ValueError as error:
            entry['error'] = str(error)
            report['errors'].append(entry)
            continue
        results, controls = compare_model(vectors, points, surfaces, polys)
        report['controls'].update(controls)
        entry.update(surfaces=len(surfaces), polys=len(polys), poly_tail_bytes=tail,
                     stale_poly_normals=sum(p['stale_normal'] for p in polys),
                     statuses=dict(Counter(r['status'] for r in results)),
                     max_deviation={k: max((r['deviation'][k] for r in results if 'deviation' in r), default=None)
                                    for k in ('base', 'texture_u', 'texture_v')},
                     surfaces_detail=results)
        report['models'].append(entry)
    report['controls'] = dict(report['controls'])
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--reader', type=Path, required=True, help='ow-package executable')
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install root')
    parser.add_argument('--packages', nargs='*', default=[], help='cooked package names; default: every non-_SF package')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    reader = Reader(args.reader.resolve(), args.game.resolve())
    reports, totals, controls = [], Counter(), Counter()
    packages_with_polys = 0
    for package in reader.packages(args.packages):
        try:
            report = check_package(reader, package)
        except ValueError as error:
            reports.append({'package': package.stem, 'error': str(error)})
            totals['package_error'] += 1
            continue
        if report is None:
            continue
        packages_with_polys += 1
        reports.append(report)
        for entry in report['models']:
            totals.update(entry['statuses'])
            totals['stale_poly_normals'] += entry['stale_poly_normals']
            totals['no_unique_poly_with_identical_axes'] += sum(
                1 for s in entry['surfaces_detail'] if s.get('identical_axes_polys') == 1)
        totals['model_error'] += len(report['errors'])
        controls.update(report['controls'])
    summary = {'packages_with_polys': packages_with_polys,
               'models': sum(len(r.get('models', [])) for r in reports),
               'surfaces': dict(totals), 'controls': dict(controls)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({'summary': summary, 'packages': reports}, indent=1), encoding='utf-8')
    print(json.dumps(summary, indent=1))
    return 1 if totals['differ'] or totals['invalid_reference'] or totals['model_error'] or totals['package_error'] else 0


if __name__ == '__main__':
    sys.exit(main())
