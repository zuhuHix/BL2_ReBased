"""Add corroborated terrain floors to an already prepared local scene.

Each TerrainComponent becomes one mesh whose cells follow the native index
strip. In every installed Sanctuary component that strip agrees with the actor
flag bytes (bit 0: no cell, bit 1: flipped diagonal) and with the component
bounds leaves, so holes and diagonals are not assumed. Source layer, setup,
material, mapping and weightmap identities are retained on the mesh record.
The visual is one labeled single-layer approximation; native layer blending,
UV semantics and lightmaps remain UNVERIFIED. Collision is opt-in and must be
checked in the runtime terrain test separately from these scripts.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import zlib
from prepare_level import Scene, props, transform, values
from terrain_decode import (decode_alpha_maps, decode_component, decode_component_geometry, decode_weighted_materials,
                            decode_terrain, validate_component_bounds, validate_leaf_coverage)

TERRAIN_CLASSES = ('Engine.Terrain', 'Engine.TerrainComponent')
COLLISION_POLICY = 'native_strip_triangle_mesh_v1'

# This is a local evidence contract.  It is intentionally stricter than the
# old dominant-alpha approximation: a weighted graph is emitted only when a
# separate, repeatable correspondence run has recorded the source identity,
# dimensions and sampling policy for every weightmap/layer pair.
WEIGHTMAP_EVIDENCE_FILES = (
    'terrain-weightmap-mapping.json',
    'layer-correspondence.json',
)
VALIDATED_EVIDENCE_STATUS = {'validated'}
SUPPORTED_MAPPING_AXES = {'xy'}
SUPPORTED_RESAMPLING = {'nearest', 'bilinear'}
SUPPORTED_SAMPLING = {'vertex', 'vertex_center', 'normalized_vertex', 'normalized_center'}
FALLBACK_COLORS = (
    [0.24, 0.28, 0.32], [0.38, 0.31, 0.22], [0.32, 0.36, 0.28],
    [0.48, 0.44, 0.35], [0.22, 0.34, 0.42], [0.42, 0.27, 0.34],
    [0.35, 0.35, 0.35], [0.55, 0.48, 0.30],
)


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    return a if pa <= pb and pa <= pc else b if pb <= pc else c


def read_rgba_png(path):
    """Read the bounded PNG emitted by ow-package without a third-party codec.

    The reader currently emits 8-bit, non-interlaced RGBA PNGs.  Supporting
    all standard row filters here lets the manifest hash the actual PF_G8
    values, rather than treating a filename or dimensions as proof of data
    identity.  This function is intentionally not a game-format decoder.
    """
    raw = Path(path).read_bytes()
    if raw[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError('weightmap is not PNG')
    pos, idat, width = 8, bytearray(), None
    height = color_type = bit_depth = interlace = None
    while pos < len(raw):
        if pos + 12 > len(raw):
            raise ValueError('truncated PNG chunk')
        size = struct.unpack_from('>I', raw, pos)[0]
        end = pos + 12 + size
        if end > len(raw):
            raise ValueError('PNG chunk exceeds file')
        kind = raw[pos + 4:pos + 8]
        data = raw[pos + 8:pos + 8 + size]
        if kind == b'IHDR':
            if len(data) != 13:
                raise ValueError('invalid PNG IHDR')
            width, height, bit_depth, color_type, compression, filtering, interlace = struct.unpack(
                '>IIBBBBB', data)
            if (not width or not height or bit_depth != 8 or color_type != 6 or
                    compression != 0 or filtering != 0 or interlace != 0):
                raise ValueError('unsupported PNG encoding')
        elif kind == b'IDAT':
            idat.extend(data)
        elif kind == b'IEND':
            break
        pos = end
    if width is None or height is None or not idat:
        raise ValueError('PNG has no image data')
    decoded = zlib.decompress(bytes(idat))
    stride = width * 4
    expected = height * (stride + 1)
    if len(decoded) != expected:
        raise ValueError('PNG scanline extent mismatch')
    rows, at, previous = [], 0, bytearray(stride)
    for _ in range(height):
        kind = decoded[at]
        current = bytearray(decoded[at + 1:at + 1 + stride])
        at += stride + 1
        for i, value in enumerate(current):
            left = current[i - 4] if i >= 4 else 0
            up = previous[i]
            upper_left = previous[i - 4] if i >= 4 else 0
            if kind == 1:
                current[i] = (value + left) & 255
            elif kind == 2:
                current[i] = (value + up) & 255
            elif kind == 3:
                current[i] = (value + ((left + up) // 2)) & 255
            elif kind == 4:
                current[i] = (value + _paeth(left, up, upper_left)) & 255
            elif kind != 0:
                raise ValueError('unsupported PNG row filter')
        rows.append(bytes(current))
        previous = current
    pixels = b''.join(rows)
    values = bytes(pixels[i] for i in range(0, len(pixels), 4))
    if any(pixels[i] != pixels[i + 1] or pixels[i] != pixels[i + 2] or pixels[i + 3] != 255
           for i in range(0, len(pixels), 4)):
        raise ValueError('PF_G8 PNG is not opaque grayscale')
    return {'width': width, 'height': height, 'values': values,
            'sha256': hashlib.sha256(values).hexdigest(),
            'minimum': min(values), 'maximum': max(values),
            'nonzero': sum(1 for value in values if value)}


def weightmap_inventory(scene, records, level, output):
    """Decode every owned PF_G8 map and retain identity/value evidence."""
    inventory = []
    for record in records:
        key = (level, record['index'])
        identity = level + ':' + record['path']
        entry = {'source': identity, 'index': record['index'], 'path': record['path'],
                 'format': 'PF_G8', 'status': 'unavailable'}
        try:
            filename = scene.texture(key, 'weightmap')
            pixels = read_rgba_png(output / filename)
            entry.update({'file': filename, 'status': 'decoded',
                          'width': pixels['width'], 'height': pixels['height'],
                          'value_sha256': pixels['sha256'],
                          'value_min': pixels['minimum'], 'value_max': pixels['maximum'],
                          'nonzero_samples': pixels['nonzero'],
                          'value_encoding': 'one PF_G8 byte per pixel; RGBA export verified'})
        except (OSError, ValueError) as error:
            entry['error'] = str(error)
        inventory.append(entry)
    return inventory


def load_weightmap_evidence(scene):
    """Load only an explicitly validated, ignored local correspondence report."""
    candidates = [scene.output / name for name in WEIGHTMAP_EVIDENCE_FILES]
    candidates += [scene.output.parent / 'terrain-weightmaps' / 'layer-correspondence.json']
    for path in candidates:
        if path.is_file():
            try:
                evidence = json.loads(path.read_text(encoding='utf-8'))
            except (OSError, ValueError) as error:
                return {'status': 'invalid', 'file': str(path), 'error': str(error)}
            evidence['_file'] = str(path)
            return evidence
    return {'status': 'missing'}


def _evidence_rows(evidence, actor):
    """Accept the stable list form and the convenient actor->rows form."""
    rows = evidence.get('mappings', evidence.get('layers'))
    if isinstance(rows, dict):
        rows = rows.get(actor) or rows.get(actor.rsplit(':', 1)[-1])
    if rows is None and isinstance(evidence.get('terrains'), dict):
        rows = evidence['terrains'].get(actor) or evidence['terrains'].get(actor.rsplit(':', 1)[-1])
    return rows if isinstance(rows, list) else None


def validated_layer_rows(evidence, actor, layers, weightmaps):
    """Return rows only when every visible layer has complete evidence.

    The correspondence tool may carry extra rows for hidden/native layers;
    those are preserved in diagnostics but do not enable a partial graph.
    """
    if evidence.get('status') not in VALIDATED_EVIDENCE_STATUS:
        return None, 'weightmap correspondence is ' + str(evidence.get('status', 'missing'))
    rows = _evidence_rows(evidence, actor)
    if not rows:
        return None, 'validated correspondence has no rows for ' + actor
    by_layer = {}
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get('layer_index'), int):
            return None, 'correspondence row has no integer layer_index'
        by_layer[row['layer_index']] = row
    visible = [i for i, layer in enumerate(layers) if not layer.get('hidden')]
    if any(i not in by_layer for i in visible):
        return None, 'validated correspondence does not cover every visible layer'
    by_source = {item['source']: item for item in weightmaps}
    selected = []
    for index in visible:
        row = by_layer[index]
        source = row.get('weightmap_source') or row.get('source')
        wm = by_source.get(source)
        if wm is None and isinstance(row.get('weightmap'), str):
            wm = by_source.get(row['weightmap'])
        if wm is None or wm.get('status') != 'decoded':
            return None, 'correspondence references unavailable weightmap for layer ' + str(index)
        mapping = row.get('mapping') or row.get('sample') or {}
        if not isinstance(mapping, dict) or mapping.get('axis') not in SUPPORTED_MAPPING_AXES:
            return None, 'layer ' + str(index) + ' has unsupported or missing axis evidence'
        if mapping.get('resampling') not in SUPPORTED_RESAMPLING:
            return None, 'layer ' + str(index) + ' has unsupported or missing resampling evidence'
        if mapping.get('sampling') not in SUPPORTED_SAMPLING:
            return None, 'layer ' + str(index) + ' has unsupported or missing sampling coordinates'
        crop = mapping.get('crop')
        if (not isinstance(crop, list) or len(crop) != 2 or
                any(type(value) is not int or value < 0 for value in crop)):
            return None, 'layer ' + str(index) + ' lacks a nonnegative two-axis crop'
        if not isinstance(mapping.get('scale'), list) or len(mapping['scale']) != 2:
            return None, 'layer ' + str(index) + ' lacks two-axis sampling scale'
        if not isinstance(mapping.get('offset', [0, 0]), list) or len(mapping.get('offset', [0, 0])) != 2:
            return None, 'layer ' + str(index) + ' lacks two-axis sampling offset'
        if (wm.get('width'), wm.get('height')) != tuple(row.get('weightmap_dimensions',
                                                                [wm.get('width'), wm.get('height')])):
            return None, 'layer ' + str(index) + ' has stale weightmap dimensions'
        if row.get('value_sha256') != wm.get('value_sha256'):
            return None, 'layer ' + str(index) + ' has stale weightmap values'
        current_layer = layers[index]
        current_names = {current_layer.get('name')}
        current_names.update(item.get('terrain_material') for item in current_layer.get('materials', []))
        current_names.update(item.get('material_path') for item in current_layer.get('materials', []))
        if row.get('layer_name') not in current_names and row.get('terrain_material') not in current_names:
            return None, 'layer ' + str(index) + ' is not associated with the current layer identity'
        if row.get('status', 'validated') not in VALIDATED_EVIDENCE_STATUS:
            return None, 'layer ' + str(index) + ' is not validated'
        selected.append({**row, 'weightmap': wm, 'mapping': mapping})
    return selected, None


def fallback_color(index):
    return list(FALLBACK_COLORS[index % len(FALLBACK_COLORS)])


def material_key(scene, package, candidate):
    """Resolved (package, index) of a layer material, or None."""
    if candidate.get('material_key'):
        return tuple(candidate['material_key'])
    if isinstance(candidate.get('material'), int) and candidate['material'] > 0:
        return scene.resolve(package, candidate['material'])
    return None


def package_weight_evidence(scene, level, record, terrain, alpha, payload, layers, inventory, output):
    """Correspondence read from the cooked WeightedMaterials block, or an error.

    Each cooked weight array must equal its paired PF_G8 texture texel for
    texel and its TerrainMaterial must belong to a layer; otherwise no row is
    trusted and the caller keeps the single-layer fallback.
    """
    weighted = decode_weighted_materials(payload, terrain, alpha, record['index'])
    if weighted is None:
        return None, 'no cooked WeightedMaterials block decoded'
    by_texture = {row['index']: row for row in inventory}
    gw, gh = terrain['weight_grid']
    tessellation = terrain['weightmap_tessellation']
    rows = []
    for i, weight in enumerate(weighted['weights']):
        texture = by_texture.get(weight['texture'])
        if texture is None or texture.get('status') != 'decoded':
            return None, f'weight {i} references an undecoded weightmap texture'
        pixels = read_rgba_png(output / texture['file'])
        width, height = pixels['width'], pixels['height']
        if width < gw or height < gh:
            return None, f'weight {i} grid exceeds texture {texture["path"]}'
        crop = bytes(pixels['values'][y * width + x] for y in range(gh) for x in range(gw))
        if crop != weight['data']:
            return None, f'weight {i} differs from texture {texture["path"]}'
        key = scene.resolve(level, weight['material'])
        identity = scene.identity(key) if key else None
        layer_index = next((n for n, layer in enumerate(layers)
                            if any(m['terrain_material'] == identity for m in layer['materials'])), None)
        if layer_index is None:
            return None, f'weight {i} material {identity} is not in any layer'
        rows.append({
            'layer_index': layer_index, 'layer_name': layers[layer_index]['name'],
            'terrain_material': identity, 'weightmap_source': texture['source'],
            'weightmap_dimensions': [width, height], 'value_sha256': texture['value_sha256'],
            'status': 'validated',
            # Host mesh UVs are patch coordinates; weight texel (x, y) sits at
            # patch (x, y) / tessellation, sampled at its centre.
            'mapping': {'axis': 'xy', 'sampling': 'vertex_center', 'resampling': 'bilinear',
                        'crop': [0, 0], 'scale': [tessellation / width, tessellation / height],
                        'offset': [0.5 / width, 0.5 / height],
                        'grid': [gw, gh], 'tessellation': tessellation}})
    return {'status': 'validated',
            '_file': 'package:' + level + ':' + record['path'] + ':WeightedMaterials',
            'source': weighted['pairing'], 'mappings': rows}, None


def terrain_blend_definition(scene, actor, layers, weightmaps, evidence):
    """Build a serializable weighted terrain material definition.

    Returning ``None`` is deliberate: the caller must retain a visible,
    explicit fallback when correspondence is missing or incomplete.  The
    function never chooses a dominant layer and never invents a crop or axis.
    """
    rows, error = validated_layer_rows(evidence, actor, layers, weightmaps)
    if rows is None:
        return None, error
    blend_layers, inventory = [], []
    for row in rows:
        index = row['layer_index']
        layer = layers[index]
        candidates = [item for item in layer.get('materials', [])
                      if material_key(scene, actor.split(':', 1)[0], item)]
        tiling = [1.0, 1.0]
        if not candidates:
            # Evidence can still be useful for a layer whose source material
            # is unavailable; its explicit colour is part of the graph.
            source_material, material_name, channels = None, None, {}
        else:
            candidate = candidates[0]
            key = material_key(scene, actor.split(':', 1)[0], candidate)
            tiling = candidate.get('local_to_mapping') or tiling
            material_name = scene.material(key)
            source_material = candidate.get('material_path') or scene.identity(key)
            channels = dict(scene.materials[material_name].get('channels', {}))
        inventory.append({'layer_index': index, 'source_material': source_material,
                          'material': material_name, 'channels': channels,
                          'status': 'resolved' if material_name else 'unavailable_material'})
        diffuse = channels.get('diffuse')
        blend_layers.append({
            'layer_index': index, 'name': layer.get('name'),
            'source_material': source_material, 'material': material_name,
            'diffuse': diffuse,
            'diffuse_status': 'available' if diffuse else 'unavailable_texture',
            'fallback_color': None if diffuse else fallback_color(index),
            'weightmap': row['weightmap']['source'],
            'weightmap_file': row['weightmap'].get('file'),
            'weightmap_dimensions': [row['weightmap']['width'], row['weightmap']['height']],
            'weightmap_value_sha256': row['weightmap'].get('value_sha256'),
            'mapping': row['mapping'],
            # Layer colour tiles by the TerrainMaterial's diagonal LocalToMapping
            # over patch coordinates, as the single-layer path does.
            'diffuse_mapping': {'scale': list(tiling), 'offset': [0.0, 0.0]},
            'filters': {'slope': 'UNVERIFIED', 'noise': 'UNVERIFIED',
                        'native_semantics': 'UNVERIFIED'},
        })
    return {
        'method': 'terrain_weighted_sum_v2',
        'status': 'validated_mapping_partial_native',
        'mapping_evidence': evidence.get('_file'),
        'mapping_status': evidence.get('status'),
        'mesh_uv_policy': 'patch_coordinates_v1',
        'layers': blend_layers,
        'texture_inventory': inventory,
        'unverified': ['slope_filters', 'noise', 'lightmaps', 'native_blend_semantics'],
        'blend_operation': 'sum(layer_color * validated_weight_sample); no native normalization asserted',
    }, None


def layer_material_inventory(scene, actor, layers):
    """Resolve every terrain-layer material even when mapping evidence is absent."""
    package = actor.split(':', 1)[0]
    inventory = []
    for index, layer in enumerate(layers):
        entries = []
        for candidate in layer.get('materials', []):
            record = {'terrain_material': candidate.get('terrain_material'),
                      'material_path': candidate.get('material_path'),
                      'status': 'unavailable_material', 'material': None, 'channels': {},
                      'diffuse': None, 'diffuse_status': 'unavailable_material',
                      'fallback_color': fallback_color(index)}
            key = material_key(scene, package, candidate)
            if key:
                try:
                    name = scene.material(key)
                    record.update({'status': 'resolved', 'material': name,
                                   'source': scene.identity(key),
                                   'channels': dict(scene.materials[name].get('channels', {}))})
                    record['diffuse'] = record['channels'].get('diffuse')
                    record['diffuse_status'] = ('available' if record['diffuse']
                                                else 'unavailable_texture')
                except (KeyError, ValueError) as error:
                    record['error'] = str(error)
            entries.append(record)
        inventory.append({'layer_index': index, 'name': layer.get('name'),
                          'hidden': bool(layer.get('hidden')), 'materials': entries,
                          'fallback_color': fallback_color(index),
                          'status': 'resolved' if any(e['status'] == 'resolved' for e in entries)
                          else 'unavailable_material'})
    return inventory


def cell_faces(x, y, flipped, lw, x0, y0):
    """Two triangles per cell, wound so the host pipeline draws them facing +Z.

    The native strip's parity-derived orientation differs between unflipped
    and flipped cells; whether the game culls terrain is UNVERIFIED, so the
    emitted winding is chosen geometrically rather than copied from the strip.
    """
    a = (y - y0) * lw + (x - x0)
    b, c, d = a + 1, a + lw, a + lw + 1
    return ((a, c, b), (b, c, d)) if flipped else ((a, c, d), (a, d, b))


def component_obj(terrain, section, cells, mapping):
    """OBJ in terrain patch units; UV = LocalToMapping scale of patch coordinates."""
    x0, y0, sx, sy = section
    width, heights = terrain['width'], terrain['heights']
    lw, lh = sx + 1, sy + 1
    if lw < 2 or lh < 2 or x0 < 0 or y0 < 0 or x0 + sx >= width or y0 + sy >= terrain['height']:
        raise ValueError('Invalid terrain section')
    su, sv = mapping
    lines = []
    for y in range(y0, y0 + lh):
        for x in range(x0, x0 + lw):
            lines.append(f'v {x} {y} {(heights[y * width + x] - 32768) / 128:.9g}')
    for y in range(y0, y0 + lh):
        for x in range(x0, x0 + lw):
            # The reader writes 1 - V for OBJ; keep the same convention.
            lines.append(f'vt {x * su:.9g} {1 - y * sv:.9g}')
    for y in range(y0, y0 + lh):
        for x in range(x0, x0 + lw):
            xl, xr = max(x - 1, 0), min(x + 1, width - 1)
            yl, yr = max(y - 1, 0), min(y + 1, terrain['height'] - 1)
            dx = (heights[y * width + xr] - heights[y * width + xl]) / (128 * (xr - xl))
            dy = (heights[yr * width + x] - heights[yl * width + x]) / (128 * (yr - yl))
            length = math.sqrt(dx * dx + dy * dy + 1)
            lines.append(f'vn {-dx / length:.9g} {-dy / length:.9g} {1 / length:.9g}')
    lines.append('g terrain')
    triangles = 0
    for x, y, flipped in cells:
        if not (x0 <= x < x0 + sx and y0 <= y < y0 + sy):
            raise ValueError('Cell outside component section')
        for face in cell_faces(x, y, flipped, lw, x0, y0):
            lines.append('f ' + ' '.join(f'{i + 1}/{i + 1}/{i + 1}' for i in face))
            triangles += 1
    return '\n'.join(lines) + '\n', triangles


def mapping_scale(terrain_material):
    """Accept only a pure axis-aligned LocalToMapping; anything else is unscaled."""
    m = terrain_material.get('LocalToMapping')
    if not isinstance(m, dict):
        return None
    planes = [m.get(k, {}) for k in ('XPlane', 'YPlane', 'ZPlane', 'WPlane')]
    su, sv = planes[0].get('X'), planes[1].get('Y')
    expected = [{'X': su}, {'Y': sv}, {'Z': planes[2].get('Z')}, {'W': 1}]
    for plane, keep in zip(planes, expected):
        for axis in 'XYZW':
            value = plane.get(axis, 0)
            if not isinstance(value, (int, float)) or not math.isfinite(value):
                return None
            if axis not in keep and abs(value) > 1e-9:
                return None
    if not all(isinstance(v, (int, float)) and math.isfinite(v) and v > 0 for v in (su, sv)):
        return None
    return [float(su), float(sv)]


def corner_heights(terrain, x, y):
    width, heights = terrain['width'], terrain['heights']
    return [(heights[yy * width + xx] - 32768) / 128 for yy in (y, y + 1) for xx in (x, x + 1)]


def world_point(pose, x, y, z):
    yaw = math.radians(pose['rotation'][1])
    c, s = math.cos(yaw), math.sin(yaw)
    xw, yw, zw = [v * scale for v, scale in zip((x, y, z), pose['scale'])]
    return [pose['location'][0] + c * xw - s * yw,
            pose['location'][1] + s * xw + c * yw,
            pose['location'][2] + zw]


def runtime_probes(terrain, pose, path, components):
    """World-space cell probes for the separate runtime walking check.

    Points sit 150 cm above the highest corner of their cell; surface ranges
    are the cell's corner heights so the test can tell resting on this floor
    from resting on other geometry.
    """
    cells = {(x, y): flipped for geometry in components for x, y, flipped in geometry['cells']}
    if not cells:
        return None
    holes = [(x, y) for y in range(terrain['height'] - 1) for x in range(terrain['width'] - 1)
             if (x, y) not in cells]

    def probe(x, y, lift=150):
        corners = corner_heights(terrain, x, y)
        world = [world_point(pose, x + dx, y + dy, z)[2]
                 for (dx, dy), z in zip(((0, 0), (1, 0), (0, 1), (1, 1)), corners)]
        return {'cell': [x, y],
                'point': world_point(pose, x + .5, y + .5, max(corners) + lift / pose['scale'][2]),
                'surface': [min(world), max(world)]}

    def neighbours(x, y):
        return [(x + dx, y + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))]

    def interior(candidates, member, count=1):
        # Prefer cells whose four neighbours share their state, then flat cells;
        # further picks stay several cells apart so other geometry covering one
        # spot does not hide the whole floor from the runtime check.
        def rank(c):
            corners = corner_heights(terrain, *c)
            return -sum(member(n) for n in neighbours(*c)), max(corners) - min(corners)
        chosen = []
        for c in sorted(candidates, key=rank):
            if all(max(abs(c[0] - o[0]), abs(c[1] - o[1])) >= 4 for o in chosen):
                chosen.append(c)
            if len(chosen) == count:
                break
        return chosen

    stands = interior(list(cells), lambda c: c in cells, count=6)
    hole = interior([h for h in holes if any(n in cells for n in neighbours(*h))], lambda c: c not in cells)
    hole = hole[0] if hole else None
    seam = None
    sections = [g['section'] for g in components]

    def inside(section, x, y):
        return section[0] <= x < section[0] + section[2] and section[1] <= y < section[1] + section[3]

    for x, y in sorted(cells):
        for nx, ny in neighbours(x, y):
            if (nx, ny) in cells and not any(inside(s, x, y) and inside(s, nx, ny) for s in sections):
                first, second = probe(x, y), probe(nx, ny)
                mid = [(a + b) / 2 for a, b in zip(first['point'], second['point'])]
                direction = [b - a for a, b in zip(first['point'][:2], second['point'][:2])]
                length = math.hypot(*direction)
                direction = [v / length for v in direction]
                top = max(first['point'][2], second['point'][2])
                seam = {'cells': [[x, y], [nx, ny]],
                        'start': [mid[0] - 120 * direction[0], mid[1] - 120 * direction[1], top],
                        'end': [mid[0] + 120 * direction[0], mid[1] + 120 * direction[1], top],
                        'surface': [min(first['surface'][0], second['surface'][0]),
                                    max(first['surface'][1], second['surface'][1])]}
                break
        if seam:
            break
    return {'source': path, 'cells': len(cells), 'holes': len(holes), 'stand': probe(*stands[0]),
            'stands': [probe(*c) for c in stands],
            'hole': probe(*hole) if hole else None, 'seam': seam}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--scene', type=Path, required=True)
    parser.add_argument('--collision', action='store_true',
                        help='Mark terrain meshes for triangle collision; verify with the runtime terrain test')
    parser.add_argument('--only', nargs='*', help='Restrict to these terrain actor paths (default: all)')
    args = parser.parse_args()
    if not (args.game / 'Binaries/Win32/Borderlands2.exe').is_file():
        parser.error('An installed Borderlands 2 is required')
    filename = args.scene / 'scene.json'
    manifest = json.loads(filename.read_text(encoding='utf-8'))
    if manifest['schema'] != 1 or manifest['dynamic_policy'] != 'frozen':
        parser.error('Unsupported scene schema/policy')
    scene = Scene(args.reader, args.game, args.scene,
                  include_dlc=manifest.get('package_scope') == 'base_and_dlc')
    scene.materials = {name: dict(material) for name, material in manifest['materials'].items()}
    for name in scene.materials:
        scene.materials[name].pop('_channel_priority', None)
    # Reuse PNGs already decoded from the same install; material() only adds new ones.
    decode = scene.texture

    def texture(key, channel):
        if key is not None:
            cached = scene.filename(key, '_' + channel + '.png')
            if (scene.output / cached).is_file():
                return cached
        return decode(key, channel)

    scene.texture = texture
    actors = [a for a in manifest['actors'] if 'terrain' not in a]
    meshes = {name: mesh for name, mesh in manifest['meshes'].items() if 'terrain' not in mesh}
    issues = [i for i in manifest['issues'] if not i.get('terrain')]
    schema = Path(__file__).with_name('terrain-arrays.schema')
    probes, summary = [], {'terrains': 0, 'components': 0, 'cells': 0, 'triangles': 0, 'rejected': [],
                           'weightmaps': 0, 'weightmaps_decoded': 0,
                           'weighted_materials': 0, 'material_fallbacks': 0}
    correspondence = load_weightmap_evidence(scene)
    for level in manifest['levels']:
        records = scene.call(level, '--terrain-records', schema)
        by_index = {r['index']: r for r in records}
        terrains = [r for r in records if r['class'] == 'Engine.Terrain' and 'data' in r
                    and (not args.only or r['path'] in args.only)]
        if not terrains:
            continue
        components = [r for r in records if r['class'] == 'Engine.TerrainComponent' and 'data' in r]
        payloads = {r['index']: bytes(r['payload']) for r in
                    scene.call(level, '--payloads', *[r['index'] for r in terrains + components])}
        scene.load(level)
        for record in terrains:
            try:
                terrain = decode_terrain(payloads[record['index']], record['data'])
                pose = terrain['actor_transform']
                p = values(record['data']['properties'])
                if not p.get('TerrainComponents'):
                    raise ValueError('Terrain without components')
                layers = []
                for layer in terrain['layers']:
                    v = values(layer)
                    # Sanctuary_Land layers reference setups and TerrainMaterials
                    # imported from Sanctuary_P; resolve both through the scene.
                    setup_key = scene.resolve(level, v.get('Setup', {}))
                    setup = by_index.get(setup_key[1]) if setup_key and setup_key[0] == level else None
                    if setup is not None and 'data' in setup:
                        setup_properties = values(setup['data']['properties'])
                    elif setup_key:
                        setup_properties = values(scene.call(setup_key[0], '--properties', setup_key[1],
                                                             '--property-offset', 4, '--array-schema', schema)['properties'])
                    else:
                        setup_properties = {}
                    entry = {'name': v.get('Name'), 'alpha_map_index': v.get('AlphaMapIndex'),
                             'hidden': bool(v.get('Hidden', False)),
                             'setup': scene.identity(setup_key) if setup_key else None, 'materials': []}
                    for filtered in setup_properties.get('Materials', []):
                        f = values(filtered)
                        key = scene.resolve(setup_key[0], f.get('Material', {}))
                        if key is None:
                            continue
                        tm = by_index.get(key[1]) if key[0] == level else None
                        if tm is not None and 'data' in tm:
                            t = values(tm['data']['properties'])
                        else:
                            t = values(scene.call(key[0], '--properties', key[1], '--property-offset', 4)['properties'])
                        tm_package, tm_identity = key[0], scene.identity(key)
                        resolved = scene.resolve(tm_package, t.get('Material', {}))
                        entry['materials'].append({
                            'terrain_material': tm_identity,
                            'material': t.get('Material', {}).get('index') if tm_package == level else None,
                            'material_key': list(resolved) if resolved else None,
                            'material_path': t.get('Material', {}).get('path'),
                            'mapping_scale': t.get('MappingScale'),
                            'local_to_mapping': mapping_scale(t),
                            'filter': {k: f.get(k) for k in ('UseNoise', 'NoiseScale', 'NoisePercent', 'Alpha')}})
                    layers.append(entry)
                weightmap_records = [r for r in records
                                     if r['class'] == 'Engine.TerrainWeightMapTexture'
                                     and r['outer'] == record['index']]
                weightmap_inventory_rows = weightmap_inventory(scene, weightmap_records, level, args.scene)
                weightmaps = [row['source'] for row in weightmap_inventory_rows]
                summary['weightmaps'] += len(weightmap_inventory_rows)
                summary['weightmaps_decoded'] += sum(row['status'] == 'decoded'
                                                     for row in weightmap_inventory_rows)
                alpha = decode_alpha_maps(payloads[record['index']], terrain)
                actor_identity = level + ':' + record['path']
                layer_inventory = layer_material_inventory(scene, actor_identity, layers)
                evidence, evidence_error = package_weight_evidence(
                    scene, level, record, terrain, alpha, payloads[record['index']], layers,
                    weightmap_inventory_rows, args.scene)
                if evidence is None:
                    issues.append({'object': actor_identity, 'terrain': True,
                                   'error': 'Cooked weight pairing not trusted: ' + evidence_error})
                weighted, mapping_error = terrain_blend_definition(
                    scene, actor_identity, layers, weightmap_inventory_rows, evidence or correspondence)
                material_name, mapping = None, [1.0, 1.0]
                if weighted is not None:
                    material_name = 'TerrainBlend_' + hashlib.sha256(actor_identity.encode()).hexdigest()[:24]
                    scene.materials[material_name] = {
                        'source': actor_identity + ':weighted_material', 'channels': {},
                        'lighting_model': 'MLM_DefaultLit', 'blend_mode': 'BLEND_Opaque',
                        'two_sided': False, 'terrain_blend': weighted}
                    summary['weighted_materials'] += 1
                    approximation = {
                        'method': weighted['method'], 'status': weighted['status'],
                        'mapping_evidence': weighted['mapping_evidence'],
                        'layers': [layer['layer_index'] for layer in weighted['layers']],
                        'blend_operation': weighted['blend_operation'],
                        'unverified': weighted['unverified']}
                    # The host mesh uses patch coordinates. Every texture and
                    # weightmap UV transform is explicit in the evidence rows;
                    # no LocalToMapping diagonal is silently reused here.
                    issues.append({'object': actor_identity, 'terrain': True,
                                   'error': 'Approximation: validated weight mapping drives a host weighted sum; slope/noise/lightmaps/native blend semantics remain UNVERIFIED'})
                else:
                    # Without validated weightmap evidence, keep the earlier
                    # labeled visual approximation: draw the whole terrain with
                    # the layer whose decoded per-vertex alpha has the largest
                    # mean.  This is not the native blend; it only avoids
                    # shipping an untextured surface while the evidence gate
                    # stays closed.
                    summary['material_fallbacks'] += 1
                    chosen, method = None, 'neutral_constant'
                    if alpha is not None:
                        means = [sum(m) / len(m) for m in alpha['maps']]
                        ranked = sorted(range(len(layers)), key=lambda i: -means[layers[i]['alpha_map_index']]
                                        if isinstance(layers[i]['alpha_map_index'], int)
                                        and 0 <= layers[i]['alpha_map_index'] < len(means) else 0)
                        for i in ranked:
                            candidates = [m for m in layers[i]['materials'] if material_key(scene, level, m)]
                            if candidates and not layers[i]['hidden']:
                                chosen, method = (i, candidates[0]), 'terrain_dominant_alpha_layer_v1'
                                break
                    if chosen is None:
                        # Terrains whose tail does not carry the alpha arrays
                        # cannot be ranked; use the first visible layer that
                        # resolves to a material rather than an untextured
                        # surface.  Layer order is a labeled guess, not a
                        # recovered blend.
                        for i, layer in enumerate(layers):
                            candidates = [m for m in layer['materials'] if material_key(scene, level, m)]
                            if candidates and not layer['hidden']:
                                chosen, method = (i, candidates[0]), 'terrain_first_material_layer_v1'
                                break
                    if chosen is not None:
                        layer_index, tm = chosen
                        material_name = scene.material(material_key(scene, level, tm))
                        if tm['local_to_mapping']:
                            mapping = tm['local_to_mapping']
                        else:
                            issues.append({'object': actor_identity, 'terrain': True,
                                           'error': 'Approximation: non-diagonal LocalToMapping; terrain UVs left in patch units'})
                    approximation = {
                        'method': method, 'status': 'partial_unverified',
                        'reason': mapping_error,
                        'mapping_evidence': correspondence.get('_file'),
                        'layer': layers[chosen[0]]['name'] if chosen else None,
                        'terrain_material': chosen[1]['terrain_material'] if chosen else None,
                        'uv_selection': 'patch coordinates times LocalToMapping diagonal; native mapping semantics UNVERIFIED',
                        'unverified': ['weightmap/layer correspondence', 'slope_filters',
                                       'noise', 'lightmaps', 'native_blend_semantics'],
                        'omitted': ['layer weight blending', 'height/slope/noise filters', 'lightmaps',
                                    'decorations', 'foliage']}
                    issues.append({'object': actor_identity, 'terrain': True,
                                   'error': f'Approximation: terrain drawn with single layer {approximation["layer"]!r} ({method}); native layer blend, filters and lightmaps not reconstructed'})
                geometries = []
                for ref in p['TerrainComponents']:
                    component = by_index.get(ref.get('index'))
                    if component is None or 'data' not in component or component['outer'] != record['index']:
                        raise ValueError('Terrain component is missing or not owned by the terrain')
                    diagnostic = decode_component(payloads[component['index']], component['data'])
                    diagnostic['bounds_validation'] = validate_component_bounds(terrain, diagnostic)
                    geometry = decode_component_geometry(payloads[component['index']], component['data'],
                                                         terrain, diagnostic, component['index'])
                    geometry['leaf_validation'] = validate_leaf_coverage(diagnostic, geometry)
                    geometries.append((component, geometry))
                # Components must tile the patch grid exactly once.
                tiled = {}
                for component, geometry in geometries:
                    x0, y0, sx, sy = geometry['section']
                    for y in range(y0, y0 + sy):
                        for x in range(x0, x0 + sx):
                            if (x, y) in tiled:
                                raise ValueError('Overlapping terrain components')
                            tiled[x, y] = component['index']
                if len(tiled) != (terrain['width'] - 1) * (terrain['height'] - 1):
                    raise ValueError('Terrain components do not cover the patch grid')
                for component, geometry in geometries:
                    if not geometry['cells']:
                        continue
                    identity = level + ':' + component['path']
                    name = scene.filename((level, component['index']), '')
                    file = name + '_s0.obj'
                    obj, triangles = component_obj(terrain, geometry['section'], geometry['cells'], mapping)
                    (args.scene / file).write_text(obj)
                    meshes[name] = {
                        'source': identity,
                        'sections': [{'slot': 0, 'file': file, 'material': material_name}],
                        'collision': {'status': 'triangle_mesh' if args.collision else 'absent', 'hulls': [],
                                      'source': identity, 'policy': COLLISION_POLICY if args.collision else None,
                                      'topology': geometry['topology']},
                        'terrain': {
                            'actor': level + ':' + record['path'], 'section': geometry['section'],
                            'cells': len(geometry['cells']),
                            'flipped_cells': sum(1 for c in geometry['cells'] if c[2]),
                            'height_convention': terrain['height_convention'],
                            'topology': geometry['topology'], 'facing': geometry['facing'],
                            'leaf_validation': geometry['leaf_validation'],
                            'bounds_validation': diagnostic['bounds_validation']['status'],
                            'layers': layers, 'weightmaps': weightmaps,
                            'weightmap_inventory': weightmap_inventory_rows,
                            'alpha_maps': None if alpha is None else {
                                'layers': len(alpha['maps']), 'indexing': alpha['indexing'], 'blending': alpha['blending']},
                            'surface_approximation': approximation,
                            'material_recovery': {
                                'status': 'weighted' if weighted is not None else 'dominant_layer_fallback',
                                'material': material_name,
                                'layer_texture_inventory': layer_inventory,
                                'mapping_error': mapping_error},
                            'opaque_tail_bytes': geometry['opaque_tail_bytes'],
                            'opaque_tail_status': 'UNVERIFIED'}}
                    actors.append({'source': component['path'], 'level': level, 'mesh': name,
                                   'transform': {'actor': pose, 'component': transform({}, True)},
                                   'materials': [], 'static': True,
                                   'collision_enabled': bool(args.collision),
                                   'native_skybox': False, 'hidden_visual': False,
                                   'terrain': level + ':' + record['path']})
                    summary['components'] += 1
                    summary['cells'] += len(geometry['cells'])
                    summary['triangles'] += triangles
                summary['terrains'] += 1
                probe = runtime_probes(terrain, pose, level + ':' + record['path'], [g for _, g in geometries])
                if probe:
                    probes.append(probe)
            except ValueError as error:
                summary['rejected'].append({'object': level + ':' + record['path'], 'error': str(error)})
                issues.append({'object': level + ':' + record['path'], 'terrain': True,
                               'error': 'Terrain rejected: ' + str(error)})
    manifest['actors'], manifest['meshes'], manifest['materials'] = actors, meshes, scene.materials
    manifest['issues'] = issues + [dict(i, terrain=True) for i in scene.issues]
    manifest['terrain_policy'] = {
        'geometry': 'native_component_index_strip_v1',
        'collision': COLLISION_POLICY if args.collision else 'disabled',
        'materials': 'validated_weighted_sum_with_dominant_layer_fallback',
        'weightmap_evidence': ('cooked WeightedMaterials/WeightedTextureMaps byte equality per terrain'
                               if summary['weighted_materials'] else correspondence.get('_file')),
        'weightmap_evidence_status': 'validated' if summary['weighted_materials'] else correspondence.get('status', 'missing'),
        'unverified': ['slope_filters', 'noise', 'lightmaps', 'native_blend_semantics'],
        'terrains': summary['terrains'], 'components': summary['components'],
        'weightmaps': summary['weightmaps'],
        'weightmaps_decoded': summary['weightmaps_decoded'],
        'weighted_materials': summary['weighted_materials'],
        'material_fallbacks': summary['material_fallbacks'],
        'rejected': summary['rejected']}
    manifest['visual_validation'] = 'pending'
    temporary = filename.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    temporary.replace(filename)
    (args.scene / 'terrain-runtime.json').write_text(json.dumps(
        {'collision': bool(args.collision), 'probes': probes}, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({**summary, 'placements': len(actors), 'meshes': len(meshes),
                      'materials': len(scene.materials), 'issues': len(manifest['issues'])}))
    return bool(summary['rejected'])


if __name__ == '__main__':
    raise SystemExit(main())
