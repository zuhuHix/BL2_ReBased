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
import zlib

from prepare_level import Scene, transform, values


MAX_SAMPLES = 4_000_000


def _png_gray(path):
    """Read the bounded 8-bit grayscale channel emitted by the reader.

    The texture command writes a PNG, rather than exposing a native texture
    buffer to this diagnostic.  This small decoder accepts only the formats
    emitted by ``assets.cpp`` (8-bit grayscale/RGB/RGBA), validates every
    row/filter extent, and returns source texel values without resampling.
    It deliberately rejects other PNG variants instead of guessing.
    """
    raw = Path(path).read_bytes()
    if raw[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError('weightmap is not a PNG')
    position, header, compressed = 8, None, bytearray()
    while position + 12 <= len(raw):
        size = struct.unpack_from('>I', raw, position)[0]
        end = position + 12 + size
        if end > len(raw):
            raise ValueError('truncated weightmap PNG chunk')
        kind = raw[position + 4:position + 8]
        body = raw[position + 8:position + 8 + size]
        if kind == b'IHDR':
            if size != 13:
                raise ValueError('invalid weightmap PNG header')
            header = struct.unpack('>IIBBBBB', body)
        elif kind == b'IDAT':
            compressed.extend(body)
        elif kind == b'IEND':
            if end != len(raw):
                raise ValueError('trailing weightmap PNG bytes')
            break
        position = end
    if header is None:
        raise ValueError('weightmap PNG has no header')
    width, height, depth, color, compression, filtering, interlace = header
    if (not width or not height or depth != 8 or compression != 0 or
            filtering != 0 or interlace != 0 or color not in (0, 2, 6)):
        raise ValueError('unsupported weightmap PNG encoding')
    channels = {0: 1, 2: 3, 6: 4}[color]
    row_bytes = width * channels
    decoded = zlib.decompress(bytes(compressed))
    if len(decoded) != height * (row_bytes + 1):
        raise ValueError('weightmap PNG scanline extent mismatch')
    rows, previous, cursor = [], bytearray(row_bytes), 0
    for _ in range(height):
        filter_type = decoded[cursor]
        cursor += 1
        current = bytearray(decoded[cursor:cursor + row_bytes])
        cursor += row_bytes
        if filter_type not in (0, 1, 2, 3, 4):
            raise ValueError('unsupported weightmap PNG filter')
        for i in range(row_bytes):
            left = current[i - channels] if i >= channels else 0
            up = previous[i]
            up_left = previous[i - channels] if i >= channels else 0
            if filter_type == 1:
                current[i] = (current[i] + left) & 255
            elif filter_type == 2:
                current[i] = (current[i] + up) & 255
            elif filter_type == 3:
                current[i] = (current[i] + ((left + up) // 2)) & 255
            elif filter_type == 4:
                estimate = left + up - up_left
                pa, pb, pc = abs(estimate - left), abs(estimate - up), abs(estimate - up_left)
                predictor = left if pa <= pb and pa <= pc else up if pb <= pc else up_left
                current[i] = (current[i] + predictor) & 255
        rows.append([current[i * channels] for i in range(width)])
        previous = current
    return {'width': width, 'height': height, 'values': rows,
            'sha256': hashlib.sha256(bytes(v for row in rows for v in row)).hexdigest()}


def _field_stats(rows):
    flat = [int(v) for row in rows for v in row]
    if not flat:
        raise ValueError('empty weightmap/alpha field')
    mean = sum(flat) / len(flat)
    variance = sum((v - mean) ** 2 for v in flat) / len(flat)
    return {'count': len(flat), 'min': min(flat), 'max': max(flat),
            'mean': mean, 'unique': len(set(flat)),
            'sha256': hashlib.sha256(bytes(flat)).hexdigest(),
            'nonconstant': len(set(flat)) > 1,
            'variance': variance}


def _oriented(rows, flip_x=False, flip_y=False, transpose=False):
    """Return a bounded view copy in target row-major orientation."""
    source = rows[::-1] if flip_y else rows
    source = [row[::-1] if flip_x else row[:] for row in source]
    if transpose:
        source = [list(column) for column in zip(*source)]
    return source


def _nearest(rows, x, y):
    return rows[min(len(rows) - 1, max(0, int(math.floor(y + .5))))][
        min(len(rows[0]) - 1, max(0, int(math.floor(x + .5))))]


def _bilinear(rows, x, y):
    x = min(len(rows[0]) - 1, max(0.0, x)); y = min(len(rows) - 1, max(0.0, y))
    x0, y0 = int(math.floor(x)), int(math.floor(y)); x1 = min(x0 + 1, len(rows[0]) - 1); y1 = min(y0 + 1, len(rows) - 1)
    fx, fy = x - x0, y - y0
    return ((1 - fy) * ((1 - fx) * rows[y0][x0] + fx * rows[y0][x1]) +
            fy * ((1 - fx) * rows[y1][x0] + fx * rows[y1][x1]))


def _score_field(reference, candidate):
    ref = [int(v) for row in reference for v in row]
    got = [float(v) for row in candidate for v in row]
    if len(ref) != len(got):
        raise ValueError('mapping shape mismatch')
    errors = [abs(a - b) for a, b in zip(ref, got)]
    mean_ref, mean_got = sum(ref) / len(ref), sum(got) / len(got)
    den_ref = math.sqrt(sum((v - mean_ref) ** 2 for v in ref))
    den_got = math.sqrt(sum((v - mean_got) ** 2 for v in got))
    covariance = sum((a - mean_ref) * (b - mean_got) for a, b in zip(ref, got))
    return {'exact': sum(e == 0 for e in errors), 'count': len(errors),
            'exact_fraction': sum(e == 0 for e in errors) / len(errors),
            'mae': sum(errors) / len(errors), 'max_error': max(errors),
            'correlation': covariance / (den_ref * den_got) if den_ref and den_got else None}


def _mapping_candidates(source, target_width, target_height):
    """Yield deterministic crop/orientation/resampling hypotheses.

    These are diagnostics only.  No hypothesis is promoted to native terrain
    semantics: a mapping is ``verified`` only for an exact nonconstant field
    match, and the report retains all coordinates and interpolation choices.
    """
    sw, sh = source['width'], source['height']
    rows = source['values']
    orientations = [(False, False, False), (True, False, False),
                    (False, True, False), (True, True, False)]
    seen = set()
    def emit(name, candidate, details):
        key = (name, json.dumps(details, sort_keys=True))
        if key not in seen:
            seen.add(key); yield name, candidate, details
    # Direct integer crops are the only operation previously checked. Include
    # all small edge offsets; large texture borders are also tested centrally.
    if sw >= target_width and sh >= target_height:
        x_offsets = list(range(sw - target_width + 1)) if sw - target_width <= 4 else [0, (sw - target_width) // 2, sw - target_width]
        y_offsets = list(range(sh - target_height + 1)) if sh - target_height <= 4 else [0, (sh - target_height) // 2, sh - target_height]
        for transpose in (False, True):
            cw, ch = (target_height, target_width) if transpose else (target_width, target_height)
            if sw < cw or sh < ch:
                continue
            xs = list(range(sw - cw + 1)) if sw - cw <= 4 else [0, (sw - cw) // 2, sw - cw]
            ys = list(range(sh - ch + 1)) if sh - ch <= 4 else [0, (sh - ch) // 2, sh - ch]
            for x0 in xs:
                for y0 in ys:
                    crop = [row[x0:x0 + cw] for row in rows[y0:y0 + ch]]
                    for fx, fy, _ in orientations:
                        out = _oriented(crop, fx, fy, transpose)
                        details = {'crop': [x0, y0, cw, ch], 'flip_x': fx, 'flip_y': fy, 'transpose': transpose, 'resampling': 'none'}
                        yield from emit('integer_crop', out, details)
    # Endpoint and texel-center mappings cover normalized and padded grids.
    for transpose in (False, True):
        ow, oh = (sh, sw) if transpose else (sw, sh)
        for fx, fy, _ in orientations:
            oriented = _oriented(rows, fx, fy, transpose)
            for mode in ('endpoint', 'center'):
                for interpolation in ('nearest', 'bilinear'):
                    out = []
                    for ty in range(target_height):
                        row = []
                        for tx in range(target_width):
                            if mode == 'endpoint':
                                sx = tx * (ow - 1) / max(1, target_width - 1)
                                sy = ty * (oh - 1) / max(1, target_height - 1)
                            else:
                                sx = (tx + .5) * ow / target_width - .5
                                sy = (ty + .5) * oh / target_height - .5
                            row.append(_nearest(oriented, sx, sy) if interpolation == 'nearest' else _bilinear(oriented, sx, sy))
                        out.append(row)
                    details = {'crop': [0, 0, ow, oh], 'flip_x': fx, 'flip_y': fy, 'transpose': transpose, 'resampling': mode + '_' + interpolation}
                    yield from emit('normalized_resample', out, details)


def analyze_weightmap_correspondence(alpha, weightmaps):
    """Compare decoded native-tail alpha fields to owned PF_G8 maps.

    Returns identity, dimensions, value hashes/statistics, and the best
    bounded mapping hypotheses.  It proves only observed byte correspondence;
    layer blending, filters and any source-side composition remain explicit.
    """
    maps = []
    for index, values_ in enumerate(alpha['maps']):
        rows = [list(values_[y * alpha['width']:(y + 1) * alpha['width']]) for y in range(alpha['height'])]
        maps.append({'index': index, 'width': alpha['width'], 'height': alpha['height'],
                     'stats': _field_stats(rows), 'values': rows})
    textures = []
    for item in weightmaps:
        decoded = _png_gray(item['file'])
        textures.append({**item, **decoded, 'stats': _field_stats(decoded['values'])})
    comparisons = []
    for field in maps:
        ranked = []
        for texture in textures:
            for name, candidate, details in _mapping_candidates(texture, field['width'], field['height']):
                score = _score_field(field['values'], candidate)
                ranked.append({'texture': texture['identity'], 'texture_dimensions': [texture['width'], texture['height']],
                               'field_index': field['index'], 'hypothesis': name, **details, **score})
        ranked.sort(key=lambda row: (-row['exact'], row['mae'], -(row['correlation'] or -2)))
        comparisons.append({'field_index': field['index'], 'field_stats': field['stats'], 'best': ranked[:8],
                            'nonconstant_exact': [r for r in ranked if r['exact'] == r['count'] and field['stats']['nonconstant']]})
    exact = [r for row in comparisons for r in row['nonconstant_exact']]
    return {'alpha_dimensions': [alpha['width'], alpha['height']],
            'alpha_fields': [{'index': m['index'], 'stats': m['stats']} for m in maps],
            'weightmaps': [{'identity': t['identity'], 'dimensions': [t['width'], t['height']], 'stats': t['stats']} for t in textures],
            'comparisons': comparisons,
            'resolution': 'verified_exact_nonconstant' if exact else 'UNVERIFIED',
            'verified_exact_nonconstant': exact,
            'scope': 'byte comparison of native-tail alpha arrays against PF_G8 values; native composition/filter/slope semantics UNVERIFIED'}


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
    # Per-vertex layer weights are stored at NumPatches * WeightmapTesselationLevel + 1
    # samples per axis (observed: level 2 on Sanctuary_P Terrain_2 and Sanctuary_Land Terrain_3).
    tessellation = p.get('WeightmapTesselationLevel', 1)
    if type(tessellation) is not int or tessellation <= 0:
        raise ValueError('Invalid weightmap tessellation level')
    weight_grid = [dims[0] * tessellation + 1, dims[1] * tessellation + 1]
    if weight_grid[0] * weight_grid[1] > MAX_SAMPLES:
        raise ValueError('Terrain weight sample budget exceeded')
    return {**remainder(payload, start, end), 'width': width, 'height': height,
            'weightmap_tessellation': tessellation, 'weight_grid': weight_grid,
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
    count = terrain['weight_grid'][0] * terrain['weight_grid'][1]
    layers = len(terrain['layers'])
    if start + 4 > len(payload) or struct.unpack_from('<I', payload, start)[0] != layers:
        return None
    maps, position = [], start + 4
    for _ in range(layers):
        if position + 4 + count > len(payload) or struct.unpack_from('<I', payload, position)[0] != count:
            return None
        maps.append(list(payload[position + 4:position + 4 + count]))
        position += 4 + count
    return {'maps': maps, 'width': terrain['width'], 'height': terrain['height'], 'end': position, 'indexing': 'AlphaMapIndex UNVERIFIED',
            'blending': 'UNVERIFIED'}


def decode_weighted_materials(payload, terrain, alpha, index):
    """The cooked per-layer weights and their texture pairing, after the alpha maps.

    Observed on all eight installed Sanctuary terrains (2026-09-22): u32 count,
    then per entry ``Data[grid]`` bytes, i32 SizeX, i32 SizeY, i32 terrain
    reference (this actor), i32 TerrainMaterial reference; then u32 count and
    that many TerrainWeightMapTexture references, entry i pairing with weight
    i. Every Data array equalled its texture's PF_G8 texels (31 of 31), which
    is the check `prepare_terrain.py` repeats before trusting the pairing.
    Anything else leaves the tail opaque and returns None.
    """
    gw, gh = terrain['weight_grid']
    count = gw * gh
    position = alpha['end'] if alpha else terrain['opaque_tail_offset']
    if position + 4 > len(payload):
        return None
    entries = struct.unpack_from('<I', payload, position)[0]
    if not 0 < entries <= 64:
        return None
    position += 4
    weights = []
    for _ in range(entries):
        if position + 4 + count + 16 > len(payload) or struct.unpack_from('<I', payload, position)[0] != count:
            return None
        data = payload[position + 4:position + 4 + count]
        size_x, size_y, owner, material = struct.unpack_from('<4i', payload, position + 4 + count)
        if (size_x, size_y) != (gw, gh) or owner != index or material == 0:
            return None
        weights.append({'data': data, 'material': material})
        position += 4 + count + 16
    if position + 4 > len(payload) or struct.unpack_from('<I', payload, position)[0] != entries:
        return None
    textures = struct.unpack_from('<%di' % entries, payload, position + 4)
    if any(t <= 0 for t in textures):
        return None
    for weight, texture in zip(weights, textures):
        weight['texture'] = texture
    return {'weights': weights, 'end': position + 4 + 4 * entries,
            'pairing': 'WeightedMaterials[i] <-> WeightedTextureMaps[i]; byte equality checked per texture'}


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
                            'dimensions': [r['diagnostic']['width'], r['diagnostic']['height']],
                            'fields': [{'index': i, 'stats': _field_stats([
                                m[y * r['diagnostic']['width']:(y + 1) * r['diagnostic']['width']]
                                for y in range(r['diagnostic']['height'])])} for i, m in enumerate(alpha['maps'])],
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
        if args.weightmaps:
            for terrain_record in selected:
                if terrain_record['class'] != 'Engine.Terrain' or 'diagnostic' not in terrain_record:
                    continue
                alpha = decode_alpha_maps(payloads[terrain_record['index']], terrain_record['diagnostic'])
                owned = []
                for weightmap in selected:
                    if (weightmap['class'] != 'Engine.TerrainWeightMapTexture' or
                            weightmap.get('outer') != terrain_record['index'] or
                            'decoded_weightmap' not in weightmap):
                        continue
                    decoded = weightmap['decoded_weightmap']
                    owned.append({'identity': package + ':' + weightmap['path'],
                                  'file': args.output / decoded['file']})
                try:
                    if alpha is not None and owned:
                        correspondence = analyze_weightmap_correspondence(alpha, owned)
                    else:
                        inventory = []
                        for item in owned:
                            decoded = _png_gray(item['file'])
                            inventory.append({'identity': item['identity'],
                                              'dimensions': [decoded['width'], decoded['height']],
                                              'stats': _field_stats(decoded['values'])})
                        correspondence = {
                            'alpha_dimensions': [terrain_record['diagnostic']['width'],
                                                 terrain_record['diagnostic']['height']],
                            'alpha_fields': [], 'weightmaps': inventory,
                            'comparisons': [],
                            'resolution': 'UNVERIFIED_NO_NATIVE_ALPHA_ARRAY',
                            'verified_exact_nonconstant': [],
                            'scope': ('PF_G8 inventory only; this terrain has no decoded '
                                      'native-tail alpha array sequence')}
                    correspondence['layers'] = []
                    for layer in terrain_record['diagnostic']['layers']:
                        layer_values = values(layer)
                        setup = layer_values.get('Setup') or {}
                        correspondence['layers'].append({
                            'name': layer_values.get('Name'),
                            'alpha_map_index': layer_values.get('AlphaMapIndex'),
                            'hidden': bool(layer_values.get('Hidden', False)),
                            'setup': setup.get('path'),
                        })
                    terrain_record['diagnostic']['weightmap_correspondence'] = correspondence
                except (OSError, ValueError, zlib.error) as error:
                    report['errors'].append({'package': package, 'index': terrain_record['index'],
                                             'error': 'Weightmap correspondence: ' + str(error)})
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
    if args.weightmaps:
        correspondence = {
            'schema': 'terrain-weightmap-correspondence-v1',
            'status': 'diagnostic_only',
            'packages': {},
            'scope': ('PF_G8 source texel dimensions/values and bounded alpha-field '
                      'hypotheses; layer blending, filters and native semantics remain UNVERIFIED')}
        for package, rows in report['packages'].items():
            entries = []
            for record in rows:
                diagnostic = record.get('diagnostic', {})
                if record.get('class') == 'Engine.Terrain' and 'weightmap_correspondence' in diagnostic:
                    entries.append({'identity': package + ':' + record['path'],
                                    'export_index': record['index'],
                                    'correspondence': diagnostic['weightmap_correspondence']})
            correspondence['packages'][package] = entries
        (args.output / 'layer-correspondence.json').write_text(
            json.dumps(correspondence, indent=2), encoding='utf-8')
    print(json.dumps({'output': str(output), 'errors': len(report['errors'])}))
    return bool(report['errors'])


if __name__ == '__main__':
    raise SystemExit(main())
