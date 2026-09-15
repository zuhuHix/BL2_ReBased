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
import json
import math
from pathlib import Path
from prepare_level import Scene, props, transform, values
from terrain_decode import (decode_alpha_maps, decode_component, decode_component_geometry,
                            decode_terrain, validate_component_bounds, validate_leaf_coverage)

TERRAIN_CLASSES = ('Engine.Terrain', 'Engine.TerrainComponent')
COLLISION_POLICY = 'native_strip_triangle_mesh_v1'


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

    def interior(candidates, member):
        # Prefer cells whose four neighbours share their state, then flat cells.
        def rank(c):
            corners = corner_heights(terrain, *c)
            return -sum(member(n) for n in neighbours(*c)), max(corners) - min(corners)
        ranked = sorted(candidates, key=rank)
        return ranked[0] if ranked else None

    stand = interior(list(cells), lambda c: c in cells)
    hole = interior([h for h in holes if any(n in cells for n in neighbours(*h))], lambda c: c not in cells)
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
    return {'source': path, 'cells': len(cells), 'holes': len(holes), 'stand': probe(*stand),
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
    probes, summary = [], {'terrains': 0, 'components': 0, 'cells': 0, 'triangles': 0, 'rejected': []}
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
                    setup = by_index.get(v.get('Setup', {}).get('index'))
                    entry = {'name': v.get('Name'), 'alpha_map_index': v.get('AlphaMapIndex'),
                             'hidden': bool(v.get('Hidden', False)),
                             'setup': level + ':' + setup['path'] if setup else None, 'materials': []}
                    for filtered in (values(setup['data']['properties']).get('Materials', []) if setup and 'data' in setup else []):
                        f = values(filtered)
                        tm = by_index.get(f.get('Material', {}).get('index'))
                        if tm is None or 'data' not in tm:
                            continue
                        t = values(tm['data']['properties'])
                        entry['materials'].append({
                            'terrain_material': level + ':' + tm['path'],
                            'material': t.get('Material', {}).get('index'),
                            'material_path': t.get('Material', {}).get('path'),
                            'mapping_scale': t.get('MappingScale'),
                            'local_to_mapping': mapping_scale(t),
                            'filter': {k: f.get(k) for k in ('UseNoise', 'NoiseScale', 'NoisePercent', 'Alpha')}})
                    layers.append(entry)
                weightmaps = [level + ':' + r['path'] for r in records
                              if r['class'] == 'Engine.TerrainWeightMapTexture' and r['outer'] == record['index']]
                alpha = decode_alpha_maps(payloads[record['index']], terrain)
                # Labeled visual approximation: the layer with the largest mean
                # per-vertex alpha, indexed by AlphaMapIndex, when those arrays
                # decode exactly. This is not the native blend.
                chosen, method = None, 'neutral_constant'
                if alpha is not None:
                    means = [sum(m) / len(m) for m in alpha['maps']]
                    ranked = sorted(range(len(layers)), key=lambda i: -means[layers[i]['alpha_map_index']]
                                    if isinstance(layers[i]['alpha_map_index'], int)
                                    and 0 <= layers[i]['alpha_map_index'] < len(means) else 0)
                    for i in ranked:
                        candidates = [m for m in layers[i]['materials'] if isinstance(m['material'], int) and m['material'] > 0]
                        if candidates and not layers[i]['hidden']:
                            chosen, method = (i, candidates[0]), 'terrain_dominant_alpha_layer_v1'
                            break
                material_name, mapping = None, [1.0, 1.0]
                if chosen is not None:
                    layer_index, tm = chosen
                    key = scene.resolve(level, tm['material'])
                    material_name = scene.material(key)
                    if tm['local_to_mapping']:
                        mapping = tm['local_to_mapping']
                    else:
                        issues.append({'object': level + ':' + record['path'], 'terrain': True,
                                       'error': 'Approximation: non-diagonal LocalToMapping; terrain UVs left in patch units'})
                approximation = {
                    'method': method, 'status': 'partial_unverified',
                    'layer': layers[chosen[0]]['name'] if chosen else None,
                    'terrain_material': chosen[1]['terrain_material'] if chosen else None,
                    'uv_selection': 'patch coordinates times LocalToMapping diagonal; native mapping semantics UNVERIFIED',
                    'omitted': ['layer weight blending', 'height/slope/noise filters', 'lightmaps',
                                'decorations', 'foliage']}
                issues.append({'object': level + ':' + record['path'], 'terrain': True,
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
                            'alpha_maps': None if alpha is None else {
                                'layers': len(alpha['maps']), 'indexing': alpha['indexing'], 'blending': alpha['blending']},
                            'surface_approximation': approximation,
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
        'materials': 'single_layer_approximation',
        'terrains': summary['terrains'], 'components': summary['components'],
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
