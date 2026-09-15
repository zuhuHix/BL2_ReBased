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


def decode_alpha_maps(payload, terrain):
    """Optional per-layer, per-vertex byte arrays at the start of the actor tail.

    Six of eight installed Sanctuary terrains begin their remaining tail with
    u32 count == len(Layers), then count * (u32 vertex_count, vertex_count
    bytes). Anything else leaves the tail opaque. These arrays do not reproduce
    the owned PF_G8 weightmap textures, so which source the native renderer
    blends with, and how, remains UNVERIFIED; they are retained as data only.
    """
    start = terrain['opaque_tail_offset']
    count = terrain['width'] * terrain['height']
    layers = len(terrain['layers'])
    if start + 4 > len(payload) or struct.unpack_from('<I', payload, start)[0] != layers:
        return None
    maps, position = [], start + 4
    for _ in range(layers):
        if position + 4 + count > len(payload) or struct.unpack_from('<I', payload, position)[0] != count:
            return None
        maps.append(list(payload[position + 4:position + 4 + count]))
        position += 4 + count
    return {'maps': maps, 'end': position, 'indexing': 'AlphaMapIndex UNVERIFIED',
            'blending': 'UNVERIFIED'}


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


# Observed component tail after the bounds tree, identical in all 15 installed
# Sanctuary components: a u16 array, count * 14 opaque bytes, a 72-byte block
# ending in (1, own export index), the vertex array (8-byte elements), two
# retained words, then the u16 index strip. Element meanings beyond the
# validated vertex X/Y/height and strip indices remain UNVERIFIED.
OPAQUE_RECORD_STRIDE = 14
OPAQUE_BLOCK_BYTES = 72


def strip_triangles(indices):
    """Hardware strip order; degenerate repeats are skipped, parity retained."""
    for i in range(len(indices) - 2):
        a, b, c = indices[i:i + 3]
        if a == b or b == c or a == c:
            continue
        yield (a, b, c) if i % 2 == 0 else (b, a, c)


def decode_component_geometry(payload, data, terrain, component, index):
    """Bounded walk to the native vertex array and index strip; no offset scanning.

    Every step validates against the corroborated height grid and the raw flag
    bytes: vertices must be the section's row-major grid with matching heights,
    each strip triangle must fill half of one cell, cells with bit 0 set must be
    absent, and bit 1 must select the flipped diagonal. Any disagreement rejects
    the component rather than emitting a guessed floor.
    """
    p = component['properties']
    x0, y0 = p.get('SectionBaseX', 0), p.get('SectionBaseY', 0)
    sx, sy = p.get('SectionSizeX'), p.get('SectionSizeY')
    if any(type(v) is not int for v in (x0, y0, sx, sy)) or min(x0, y0) < 0 or min(sx, sy) <= 0:
        raise ValueError('Invalid component section')
    width, height = terrain['width'], terrain['height']
    if x0 + sx >= width or y0 + sy >= height:
        raise ValueError('Component grid exceeds terrain')
    start = component['opaque_tail_offset']
    if start + 8 > len(payload):
        raise ValueError('Truncated component record array')
    size, count = struct.unpack_from('<II', payload, start)
    if size != 2 or count > MAX_SAMPLES:
        raise ValueError('Unsupported component record array')
    block = start + 8 + count * (2 + OPAQUE_RECORD_STRIDE)
    vertex_start = block + OPAQUE_BLOCK_BYTES
    if vertex_start + 8 > len(payload):
        raise ValueError('Truncated component vertex array')
    if struct.unpack_from('<II', payload, vertex_start - 8) != (1, index):
        raise ValueError('Component block does not end in the observed self reference')
    lw, lh = sx + 1, sy + 1
    vertex_count = lw * lh
    if struct.unpack_from('<II', payload, vertex_start) != (8, vertex_count):
        raise ValueError('Component vertex array header does not match the section grid')
    vertices_end = vertex_start + 8 + 8 * vertex_count
    if vertices_end + 16 > len(payload):
        raise ValueError('Truncated component vertex array')
    heights = terrain['heights']
    retained = []
    for i in range(vertex_count):
        vx, vy, h, gx, gy = struct.unpack_from('<BBHhh', payload, vertex_start + 8 + i * 8)
        if (vx, vy) != (i % lw, i // lw) or h != heights[(y0 + vy) * width + x0 + vx]:
            raise ValueError('Component vertex disagrees with the terrain height grid')
        retained.append([gx, gy])
    words = struct.unpack_from('<II', payload, vertices_end)
    size, count = struct.unpack_from('<II', payload, vertices_end + 8)
    index_start = vertices_end + 16
    if size != 2 or count > MAX_SAMPLES or index_start + 2 * count > len(payload):
        raise ValueError('Unsupported or truncated component index strip')
    indices = struct.unpack_from('<' + str(count) + 'H', payload, index_start)
    if any(v >= vertex_count for v in indices):
        raise ValueError('Component strip index out of range')
    cells = {}
    for tri in strip_triangles(indices):
        xs, ys = [v % lw for v in tri], [v // lw for v in tri]
        cx, cy = min(xs), min(ys)
        if max(xs) - cx != 1 or max(ys) - cy != 1:
            raise ValueError('Component strip triangle spans more than one cell')
        cells.setdefault((cx, cy), []).append(frozenset(tri))
    flags = terrain['flags']
    result = []
    for cy in range(sy):
        for cx in range(sx):
            flag = flags[(y0 + cy) * width + x0 + cx]
            tris = cells.pop((cx, cy), None)
            if flag & 1:
                if tris is not None:
                    raise ValueError('Strip covers a cell whose flag bit 0 is set')
                continue
            a = cy * lw + cx
            diagonal = {a + 1, a + lw} if flag & 2 else {a, a + lw + 1}
            if tris is None or len(tris) != 2 or tris[0] == tris[1] or not all(diagonal <= t for t in tris):
                raise ValueError('Strip cell triangulation disagrees with flag bits')
            result.append([x0 + cx, y0 + cy, bool(flag & 2)])
    if cells:
        raise ValueError('Strip covers cells outside the component section')
    end = index_start + 2 * count
    return {**remainder(payload, start, end), 'section': [x0, y0, sx, sy], 'cells': result,
            'vertex_array_offset': vertex_start, 'index_strip_offset': vertices_end + 8,
            'strip_indices': count, 'retained_vertex_words': retained, 'retained_words': list(words),
            'record_array_count': int(struct.unpack_from('<I', payload, start + 4)[0]),
            'topology': 'native index strip agrees with flag bits 0 (hole) and 1 (diagonal)',
            'facing': 'UNVERIFIED: strip parity gives opposite orientation for flipped cells',
            'retained_semantics': 'UNVERIFIED'}


def validate_leaf_coverage(component, geometry):
    """Every strip cell lies in one leaf; every leaf holds at least one strip cell."""
    x0, y0, sx, sy = geometry['section']
    covered = {}
    for number, node in enumerate(component['nodes']):
        if not node['leaf']:
            continue
        gx, gy, gsx, gsy = node['grid']
        if gx + gsx > sx or gy + gsy > sy:
            raise ValueError('Leaf exceeds component section')
        for cy in range(gy, gy + gsy):
            for cx in range(gx, gx + gsx):
                if (cx, cy) in covered:
                    raise ValueError('Overlapping leaves')
                covered[cx, cy] = number
    used = set()
    for cx, cy, _ in geometry['cells']:
        leaf = covered.get((cx - x0, cy - y0))
        if leaf is None:
            raise ValueError('Strip cell outside every leaf')
        used.add(leaf)
    if len(used) != sum(node['leaf'] for node in component['nodes']):
        raise ValueError('Leaf without strip cells')
    return {'status': 'leaves_and_strip_agree', 'leaves': len(used),
            'cells': len(geometry['cells']), 'leaf_cells': len(covered)}


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
                    if r['class'] == 'Engine.Terrain':
                        alpha = decode_alpha_maps(payloads[r['index']], r['diagnostic'])
                        r['diagnostic']['alpha_maps'] = alpha and {
                            'layers': len(alpha['maps']), 'consumed_to': alpha['end'],
                            'means': [sum(m) / len(m) for m in alpha['maps']],
                            'indexing': alpha['indexing'], 'blending': alpha['blending']}
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
                    geometry = decode_component_geometry(payloads[r['index']], r['data'], parent['diagnostic'],
                                                         r['diagnostic'], r['index'])
                    geometry['leaf_validation'] = validate_leaf_coverage(r['diagnostic'], geometry)
                    r['diagnostic']['geometry'] = geometry
                except ValueError as error:
                    report['errors'].append({'package': package, 'index': r['index'], 'error': str(error)})
        report['packages'][package] = selected
    output = args.output / 'terrain_diagnostics.json'
    output.write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps({'output': str(output), 'errors': len(report['errors'])}))
    return bool(report['errors'])


if __name__ == '__main__':
    raise SystemExit(main())
