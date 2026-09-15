"""Bounded Sanctuary terrain diagnostics; does not emit native floor geometry.

Schema struct labels describe observed tagged streams, not recovered UE3 types.
Native topology, flag meanings, layer blending and remainder remain UNVERIFIED.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct

from prepare_level import Scene, transform, values


MAX_SAMPLES = 4_000_000


def native_offset(payload, data, expected_prefix):
    if data.get('property_offset') != expected_prefix:
        raise ValueError('Unsupported property prefix')
    consumed = data.get('consumed_bytes')
    if type(consumed) is not int or consumed < 8:
        raise ValueError('Invalid property consumption')
    offset = expected_prefix + consumed
    if offset > len(payload) or data.get('trailing_bytes') != len(payload) - offset:
        raise ValueError('Property/payload extent mismatch')
    return offset


def remainder(payload, start, end):
    tail = payload[end:]
    return {'native_offset': start, 'consumed_native_bytes': end - start,
            'opaque_tail_offset': end, 'opaque_tail_bytes': len(tail),
            'opaque_tail_sha256': hashlib.sha256(tail).hexdigest(),
            'opaque_tail_status': 'UNVERIFIED'}


def decode_terrain(payload, data):
    p = values(data['properties'])
    dims = [p.get('NumPatchesX'), p.get('NumPatchesY')]
    if any(type(v) is not int or v <= 0 for v in dims):
        raise ValueError('Invalid terrain patch dimensions')
    width, height = dims[0] + 1, dims[1] + 1
    count = width * height
    if count > MAX_SAMPLES:
        raise ValueError('Terrain sample budget exceeded')
    start = native_offset(payload, data, 26)
    end = start + 8 + 3 * count
    if end > len(payload):
        raise ValueError('Truncated terrain arrays')
    if struct.unpack_from('<I', payload, start)[0] != count:
        raise ValueError('Height count does not match patch dimensions')
    flag_start = start + 4 + 2 * count
    if struct.unpack_from('<I', payload, flag_start)[0] != count:
        raise ValueError('Flag count does not match patch dimensions')
    heights = list(struct.unpack_from('<' + str(count) + 'H', payload, start + 4))
    flags = list(payload[flag_start + 4:end])
    pose = transform(p)
    if not all(math.isfinite(v) for group in pose.values() for v in group):
        raise ValueError('Nonfinite terrain transform')
    return {**remainder(payload, start, end), 'width': width, 'height': height,
            'heights': heights, 'flags': flags, 'actor_transform': pose,
            'layers': p.get('Layers', []), 'components': p.get('TerrainComponents', []),
            'height_convention': 'row-major +X/+Y; local Z=(sample-32768)/128; corroborated by component bounds',
            'flag_semantics': 'UNVERIFIED', 'topology': 'UNVERIFIED',
            'layer_blending': 'UNVERIFIED'}


def decode_component(payload, data):
    start = native_offset(payload, data, 8)
    if len(payload) - start < 4:
        raise ValueError('Truncated component node count')
    count = struct.unpack_from('<I', payload, start)[0]
    if not 0 < count <= MAX_SAMPLES or count > (len(payload) - start - 4) // 37:
        raise ValueError('Invalid or truncated component nodes')
    nodes = []
    for i in range(count):
        row = struct.unpack_from('<6fBI4H', payload, start + 4 + i * 37)
        bounds, valid, leaf, words = row[:6], row[6], row[7], row[8:]
        if not all(math.isfinite(v) for v in bounds) or any(bounds[k] > bounds[k + 3] for k in range(3)):
            raise ValueError('Invalid node bounds')
        if valid != 1 or leaf not in (0, 1):
            raise ValueError('Unsupported node flags')
        # Observed sparse branches use 0xffff in absent child slots.
        if leaf == 0 and (all(v == 65535 for v in words) or
                          any(v != 65535 and (v >= count or v <= i) for v in words)):
            raise ValueError('Invalid node child reference')
        if leaf == 1 and (words[2] == 0 or words[3] == 0):
            raise ValueError('Invalid leaf dimensions')
        nodes.append({'bounds': list(bounds), 'leaf': bool(leaf),
                      'grid' if leaf else 'children': list(words)})
    end = start + 4 + count * 37
    return {**remainder(payload, start, end), 'nodes': nodes,
            'node_semantics': 'observed bounds/tree prefix only; native collision and topology UNVERIFIED',
            'properties': values(data['properties'])}


def validate_component_bounds(terrain, component):
    """Corroborate the observed height/transform convention against source bounds."""
    p = component['properties']
    x, y = p.get('SectionBaseX', 0), p.get('SectionBaseY', 0)
    sx, sy = p.get('SectionSizeX'), p.get('SectionSizeY')
    width, height = terrain['width'], terrain['height']
    if (any(type(v) is not int for v in (x, y, sx, sy)) or min(x, y) < 0 or
            min(sx, sy) <= 0 or x + sx >= width or y + sy >= height):
        raise ValueError('Component grid exceeds terrain')
    pose = terrain['actor_transform']
    pitch, yaw, roll = pose['rotation']
    if pitch != 0 or roll != 0 or any(s <= 0 for s in pose['scale']):
        raise ValueError('Uninspected bounds transform')
    samples = [terrain['heights'][yy * width + xx]
               for yy in range(y, y + sy + 1) for xx in range(x, x + sx + 1)]
    low = [x - 1 / 64, y - 1 / 64, (min(samples) - 32768) / 128 - 1 / 64]
    high = [x + sx + 1 / 64, y + sy + 1 / 64, (max(samples) - 32768) / 128 + 1 / 64]
    c, s = math.cos(math.radians(yaw)), math.sin(math.radians(yaw))
    points = []
    for xx in (low[0], high[0]):
        for yy in (low[1], high[1]):
            for zz in (low[2], high[2]):
                xxw, yyw, zzw = [v * scale for v, scale in zip((xx, yy, zz), pose['scale'])]
                points.append([pose['location'][0] + c * xxw - s * yyw,
                               pose['location'][1] + s * xxw + c * yyw,
                               pose['location'][2] + zzw])
    expected = [min(v[i] for v in points) for i in range(3)] + [max(v[i] for v in points) for i in range(3)]
    error = max(abs(a - b) for a, b in zip(expected, component['nodes'][0]['bounds']))
    if error > .02:
        raise ValueError(f'Height-grid/source bounds disagree by {error:.6g}')
    return {'status': 'matches_observed_padded_bounds', 'max_error_cm': error,
            'tolerance_cm': .02, 'native_topology': 'UNVERIFIED'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--packages', nargs='+', default=['Sanctuary_P', 'Sanctuary_Land'])
    parser.add_argument('--weightmaps', action='store_true', help='Also decode PF_G8 weightmaps into the local output')
    args = parser.parse_args()
    scene = Scene(args.reader, args.game, args.output)
    report = {'status': 'diagnostic_only', 'packages': {}, 'errors': []}
    for package in args.packages:
        records = scene.call(package, '--terrain-records', Path(__file__).with_name('terrain-arrays.schema'))
        by_index = {r['index']: r for r in records}
        selected = [r for r in records if 'data' in r or 'error' in r]
        targets = [r for r in selected if r['class'] in ('Engine.Terrain', 'Engine.TerrainComponent')]
        payloads = {r['index']: bytes(r['payload']) for r in scene.call(package, '--payloads', *[r['index'] for r in targets])} if targets else {}
        for r in selected:
            try:
                if 'error' in r:
                    raise ValueError(r['error'])
                if r['class'] in ('Engine.Terrain', 'Engine.TerrainComponent'):
                    decoder = decode_terrain if r['class'] == 'Engine.Terrain' else decode_component
                    r['diagnostic'] = decoder(payloads[r['index']], r['data'])
                elif r['class'] == 'Engine.TerrainWeightMapTexture' and args.weightmaps:
                    filename = f'{package}_{r["index"]}_weightmap.png'
                    r['decoded_weightmap'] = scene.call(package, '--texture', r['index'],
                        '--property-offset', 4, '--output', args.output / filename, '--tfc', scene.cooked)
                    r['decoded_weightmap']['file'] = filename
                    r['decoded_weightmap']['layer_blending'] = 'UNVERIFIED'
                if r['class'] in ('Engine.Model', 'Engine.ModelComponent', 'Engine.Polys', 'Engine.BrushComponent'):
                    owner = by_index.get(r['outer'], {})
                    root_owner = owner.get('class') == 'Engine.Level' and owner.get('path') == 'TheWorld.PersistentLevel'
                    root_model = (owner.get('class') == 'Engine.Model' and
                                  by_index.get(owner.get('outer'), {}).get('path') == 'TheWorld.PersistentLevel')
                    r['bsp_scope'] = ('root_level' if root_owner else
                                      'root_model_polys' if r['class'] == 'Engine.Polys' and root_model else
                                      'non_root_owned')
            except ValueError as error:
                report['errors'].append({'package': package, 'index': r['index'], 'error': str(error)})
        for r in selected:
            if r['class'] == 'Engine.TerrainComponent' and 'diagnostic' in r:
                try:
                    parent = by_index.get(r['outer'], {})
                    if parent.get('class') != 'Engine.Terrain' or 'diagnostic' not in parent:
                        raise ValueError('Missing decoded terrain owner')
                    r['diagnostic']['bounds_validation'] = validate_component_bounds(parent['diagnostic'], r['diagnostic'])
                except ValueError as error:
                    report['errors'].append({'package': package, 'index': r['index'], 'error': str(error)})
        report['packages'][package] = selected
    output = args.output / 'terrain_diagnostics.json'
    output.write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps({'output': str(output), 'errors': len(report['errors'])}))
    return bool(report['errors'])


if __name__ == '__main__':
    raise SystemExit(main())
