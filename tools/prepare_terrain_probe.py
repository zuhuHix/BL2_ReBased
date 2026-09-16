"""Prepare a separate, non-colliding Terrain_10 height-grid diagnostic.

All cells are drawn with a fixed diagonal and colors keyed to raw flag bytes.
This deliberately does not assign visibility/diagonal semantics to those bits.
It never updates the prepared Sanctuary scene or claims native floor coverage.
"""
import argparse
import json
import math
from pathlib import Path
from prepare_level import Scene, props, transform
from terrain_decode import decode_terrain


def grid_obj(width, height, heights, flags, selected):
    if width < 2 or height < 2 or len(heights) != width * height or len(flags) != width * height:
        raise ValueError('Invalid height-grid dimensions')
    lines = []
    for y in range(height):
        for x in range(width):
            z = (heights[y * width + x] - 32768) / 128
            lines.append(f'v {x} {y} {z:.9g}')
    for y in range(height):
        for x in range(width):
            lines.append(f'vt {x / (width - 1):.9g} {y / (height - 1):.9g}')
    for y in range(height):
        for x in range(width):
            xl, xr = max(x - 1, 0), min(x + 1, width - 1)
            yl, yr = max(y - 1, 0), min(y + 1, height - 1)
            dx = (heights[y * width + xr] - heights[y * width + xl]) / (128 * (xr - xl))
            dy = (heights[yr * width + x] - heights[yl * width + x]) / (128 * (yr - yl))
            length = math.sqrt(dx * dx + dy * dy + 1)
            lines.append(f'vn {-dx / length:.9g} {-dy / length:.9g} {1 / length:.9g}')
    lines.append('g diagnostic')
    triangles = 0
    for y in range(height - 1):
        for x in range(width - 1):
            a = y * width + x
            if flags[a] != selected:
                continue
            b, c, d = a + 1, a + width, a + width + 1
            for face in ((a, c, b), (b, c, d)):
                lines.append('f ' + ' '.join(f'{i+1}/{i+1}/{i+1}' for i in face))
                triangles += 1
    return '\n'.join(lines) + '\n', triangles


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', required=True, type=Path)
    parser.add_argument('--game', required=True, type=Path)
    parser.add_argument('--output', type=Path, default=Path('local/terrain-probe'))
    args = parser.parse_args()
    reader = Scene(args.reader, args.game, args.output)
    rows = reader.call('Sanctuary_P', '--terrain-records', Path(__file__).with_name('terrain-arrays.schema'))
    selected = [r for r in rows if r['path'] == 'TheWorld.PersistentLevel.Terrain_10'
                and r['class'] == 'Engine.Terrain']
    if len(selected) != 1:
        raise ValueError('Expected the inspected Terrain_10')
    row = selected[0]
    p = props(row)
    if (p.get('NumPatchesX'), p.get('NumPatchesY'), p.get('MaxTesselationLevel')) != (20, 25, 1):
        raise ValueError('Uninspected Terrain_10 dimensions/tessellation')
    decoded = decode_terrain(bytes(reader.call('Sanctuary_P', '--payload', row['index'])), row['data'])
    width, height = decoded['width'], decoded['height']
    colors = {0: [.35, .35, .35], 1: [.45, .22, .08], 2: [.08, .3, .45], 3: [.4, .1, .3]}
    sections, materials = [], {}
    for flag in sorted(set(decoded['flags'])):
        obj, triangles = grid_obj(width, height, decoded['heights'], decoded['flags'], flag)
        if not triangles:
            continue
        name = f'raw_flag_{flag}'
        filename = name + '.obj'
        (args.output / filename).write_text(obj)
        sections.append({'slot': len(sections), 'file': filename, 'material': name})
        materials[name] = {'source': 'Diagnostic:raw_terrain_flag_' + str(flag),
                           'channels': {}, 'constant_diffuse': colors.get(flag, [.5, .1, .1])}
    pose = transform(p)
    if any(pose['rotation']):
        raise ValueError('Diagnostic camera expects the inspected unrotated Terrain_10')
    location, scale = pose['location'], pose['scale']
    scene = {'schema': 1, 'map': 'TerrainProbe_P', 'levels': ['Sanctuary_P'],
             'dynamic_policy': 'frozen', 'visual_validation': 'diagnostic_only',
             'collision_policy': 'disabled_diagnostic', 'materials': materials,
             'meshes': {'terrain_grid': {'source': 'Diagnostic:Sanctuary_P.Terrain_10',
                                        'sections': sections, 'collision': {'status': 'absent', 'hulls': []}}},
             'actors': [{'source': row['path'], 'level': 'Sanctuary_P', 'mesh': 'terrain_grid',
                         'transform': {'actor': pose, 'component': transform({}, True)},
                         'materials': [], 'static': True, 'collision_enabled': False,
                         'native_skybox': False, 'hidden_visual': False}],
             'camera': {'location': [location[0] + (width - 1) * scale[0] / 2,
                                     location[1] - 1500, location[2] + 3000],
                        'rotation': [-40, 90, 0], 'scale': [1, 1, 1]},
             'issues': [{'object': row['path'], 'error': 'Diagnostic: all cells; fixed diagonal; raw flag colors; no native holes, materials or collision'}],
             'terrain_diagnostic': {'source_index': row['index'], 'width': width, 'height': height,
                                    'topology': 'UNVERIFIED fixed diagonal/all cells',
                                    'height_convention': 'corroborated by source component bounds',
                                    'raw_flag_colors': colors}}
    (args.output / 'scene.json').write_text(json.dumps(scene, indent=2) + '\n')
    print(json.dumps({'vertices': width * height, 'triangles': (width - 1) * (height - 1) * 2,
                      'sections': len(sections), 'collision': False, 'map': scene['map']}))


if __name__ == '__main__':
    main()
