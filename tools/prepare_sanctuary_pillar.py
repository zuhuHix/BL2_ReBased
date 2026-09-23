"""Bake one frame of UModel's Sanctuary central-pillar MD5 exports to OBJ.

This reads the documented MD5 text interchange format, not a cooked UE3
serialization layout. It is deliberately limited to the observed four-section,
17-joint Sanctuary pillar and rejects changes in that shape.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
import uuid

from prepare_level import Scene, material_index, props, values


PILLAR_SHADERS = (
    'Mati_SancSpire_Top', 'material_1',
    'Mati_SancSpire_Base', 'Mati_SancSpire_Tile',
)
MATERIAL_PATHS = tuple('Prop_SancBuildings_02.Material.' + name for name in (
    'Mati_SancSpire_Top', 'Mati_SancSpire_Topinner',
    'Mati_SancSpire_Base', 'Mati_SancSpire_Tile'))
UV_ATLAS = ((0.34, 1.0, 0.33, 0.0), (1.0, 1.0, 0.0, 0.0),
            (0.33, 1.0, 0.67, 0.0), (0.33, 1.0, 0.0, 0.0))
# Master_SancSpire's Luminosity_Channel static mask, as overridden per instance
# (Top=G, Base=B, Tile=R). Observed 2026-09-23 by locating the mask node's
# ExpressionGUID in each instance's trailing static-parameter bytes; not a
# decoded layout. Topinner samples a flat stub, so its Color stands alone.
LUMINOSITY_CHANNEL = (1, None, 2, 0)
LUMINOSITY_TEXTURES = ('SancSpire_Lum', 'StubWhite_Gray', 'SancSpire_Lum', 'SancSpire_Lum')
PILLAR_SOURCE = 'TheWorld.PersistentLevel.SkeletalMeshActor_2.SkeletalMeshComponent_345'
PILLAR_MESH = 'Prop_SancBuildings_02.Meshes.Skel_SanctuaryCentralPillar'
# Sanctuary_Dynamic Episode_8, InterpData_0, group "Spire", move track 16:
# first IMF_World position before the takeoff Matinee raises the spire.
PREFLIGHT_LOCATION = [8424.0, 632.0, 543.999756]
JOINT = re.compile(r'^\s*"([^"]+)"\s+(-?\d+)\s+\(\s*([^)]+)\)\s+\(\s*([^)]+)\)\s*$')
VERT = re.compile(r'^\s*vert\s+(\d+)\s+\(\s*([^)]+)\)\s+(\d+)\s+(\d+)\s*$')
TRI = re.compile(r'^\s*tri\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s*$')
WEIGHT = re.compile(r'^\s*weight\s+(\d+)\s+(\d+)\s+(\S+)\s+\(\s*([^)]+)\)\s*$')


def block(text, label):
    found = re.search(r'(?m)^' + re.escape(label) + r'\s*\{([^{}]*)\}', text)
    if not found:
        raise ValueError(f'Missing MD5 block: {label}')
    return found.group(1)


def numbers(value):
    return tuple(float(part) for part in value.split())


def quaternion(xyz):
    x, y, z = xyz
    w = -math.sqrt(max(0.0, 1.0 - x*x - y*y - z*z))
    return x, y, z, w


def multiply(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (aw*bx + ax*bw + ay*bz - az*by,
            aw*by - ax*bz + ay*bw + az*bx,
            aw*bz + ax*by - ay*bx + az*bw,
            aw*bw - ax*bx - ay*by - az*bz)


def rotate(q, value):
    x, y, z, w = q
    vx, vy, vz = value
    tx, ty, tz = (2*(y*vz-z*vy), 2*(z*vx-x*vz), 2*(x*vy-y*vx))
    return (vx + w*tx + y*tz - z*ty,
            vy + w*ty + z*tx - x*tz,
            vz + w*tz + x*ty - y*tx)


def joints_from(block_text):
    joints = []
    for line in block_text.splitlines():
        if not line.strip():
            continue
        match = JOINT.match(line)
        if not match:
            raise ValueError(f'Unsupported MD5 joint: {line}')
        name, parent, position, orientation = match.groups()
        joints.append((name, int(parent), numbers(position), quaternion(numbers(orientation))))
    if len(joints) != 17 or joints[0][0] != 'Root':
        raise ValueError('Unexpected Sanctuary pillar skeleton')
    return joints


def read_mesh(path):
    text = Path(path).read_text(encoding='utf-8')
    if 'MD5Version 10' not in text or 'numJoints 17' not in text or 'numMeshes 4' not in text:
        raise ValueError('Unexpected Sanctuary pillar MD5 mesh header')
    joints = joints_from(block(text, 'joints'))
    sections = []
    for body in re.findall(r'(?m)^mesh\s*\{([^{}]*)\}', text):
        shader = re.search(r'\bshader\s+"([^"]+)"', body)
        if shader is None:
            raise ValueError('MD5 mesh has no shader')
        verts, tris, weights = {}, {}, {}
        for line in body.splitlines():
            if match := VERT.match(line):
                index, uv, first, count = match.groups()
                verts[int(index)] = (numbers(uv), int(first), int(count))
            elif match := TRI.match(line):
                index, a, b, c = match.groups()
                tris[int(index)] = (int(a), int(b), int(c))
            elif match := WEIGHT.match(line):
                index, joint, bias, position = match.groups()
                weights[int(index)] = (int(joint), float(bias), numbers(position))
        if sorted(verts) != list(range(len(verts))) or sorted(tris) != list(range(len(tris))) or sorted(weights) != list(range(len(weights))):
            raise ValueError('MD5 mesh indices have gaps')
        for label, parsed in (('numverts', verts), ('numtris', tris), ('numweights', weights)):
            declared = re.search(r'\b' + label + r'\s+(\d+)', body)
            if declared is None or int(declared.group(1)) != len(parsed):
                raise ValueError(f'Pillar {label} count differs from parsed records')
        if any(any(vertex not in verts for vertex in triangle) for triangle in tris.values()):
            raise ValueError('Pillar triangle references a missing vertex')
        sections.append((shader.group(1), verts, tris, weights))
    if tuple(section[0] for section in sections) != PILLAR_SHADERS:
        raise ValueError('Unexpected Sanctuary pillar material slots')
    return joints, sections


def read_animation(path, frame):
    text = Path(path).read_text(encoding='utf-8')
    if 'MD5Version 10' not in text or 'numJoints 17' not in text or 'numAnimatedComponents 102' not in text:
        raise ValueError('Unexpected Sanctuary pillar MD5 animation header')
    names, parents = [], []
    for line in block(text, 'hierarchy').splitlines():
        if not line.strip():
            continue
        match = re.match(r'^\s*"([^"]+)"\s+(-?\d+)\s+(\d+)\s+(\d+)\s*$', line)
        if not match:
            raise ValueError(f'Unsupported MD5 hierarchy entry: {line}')
        name, parent, flags, start = match.groups()
        if int(flags) != 63 or int(start) != len(names)*6:
            raise ValueError('Pillar animation uses an unsupported component layout')
        names.append(name)
        parents.append(int(parent))
    if len(names) != 17:
        raise ValueError('Pillar animation joint count changed')
    frames = re.search(r'\bnumFrames\s+(\d+)', text)
    if frames is None or not 0 <= frame < int(frames.group(1)):
        raise ValueError('Pillar animation frame is out of range')
    values = [numbers(line) for line in block(text, f'frame {frame}').splitlines() if line.strip()]
    if len(values) != 17 or any(len(value) != 6 for value in values):
        raise ValueError('Pillar animation frame has an unexpected shape')
    world = []
    for i, (local, parent) in enumerate(zip(values, parents)):
        position, orientation = local[:3], quaternion(local[3:])
        if parent >= 0:
            if parent >= i:
                raise ValueError('Pillar animation joints are not parent ordered')
            parent_position, parent_rotation = world[parent]
            rotated = rotate(parent_rotation, position)
            position = tuple(a+b for a, b in zip(parent_position, rotated))
            orientation = multiply(parent_rotation, orientation)
        world.append((position, orientation))
    return names, world


def skinned_vertices(section, world):
    _, verts, _, weights = section
    positions = []
    for uv, first, count in verts.values():
        if count < 1 or first < 0 or first+count > len(weights):
            raise ValueError('Pillar vertex has invalid weights')
        result = [0.0, 0.0, 0.0]
        total = 0.0
        for index in range(first, first+count):
            bone, bias, offset = weights[index]
            if bone >= len(world) or bias < 0:
                raise ValueError('Pillar weight is invalid')
            position, orientation = world[bone]
            point = tuple(a+b for a, b in zip(position, rotate(orientation, offset)))
            for axis in range(3):
                result[axis] += bias*point[axis]
            total += bias
        if abs(total-1.0) > 0.002:
            raise ValueError('Pillar vertex weights do not sum to one')
        positions.append((result[0], -result[1], result[2]))
    return positions


def bake_spire_diffuse(color_path, luminosity_path, channel, atlas, output_path):
    """Approximate one instance's diffuse as 2 x Luminosity[channel] x Color strip.

    UNVERIFIED: the cooked master keeps only its parameter nodes, so the
    combine step is an assumption chosen to leave the mid-grey Color tint
    near neutral. The Color strip is its Color_UV_Scale & Offset window,
    stretched over the full UV square the Luminosity map uses.
    """
    from PIL import Image
    import numpy as np
    su, sv, ou, ov = atlas
    if sv != 1.0 or ov != 0.0:
        raise ValueError('Spire Color transform is not a horizontal strip')
    luminosity = Image.open(luminosity_path).convert('RGB')
    color = Image.open(color_path).convert('RGB')
    strip = color.crop((round(ou*color.width), 0, round((ou+su)*color.width), color.height))
    strip = strip.resize(luminosity.size, Image.BILINEAR)
    detail = np.asarray(luminosity, dtype=np.float32)[..., channel:channel+1] / 255
    tint = np.asarray(strip, dtype=np.float32) / 255
    rgb = np.clip(2.0 * detail * tint, 0.0, 1.0)
    Image.fromarray((rgb*255 + 0.5).astype(np.uint8)).convert('RGBA').save(output_path)


def bake(mesh_path, animation_path, frame, output, uv_atlas=None):
    joints, sections = read_mesh(mesh_path)
    names, world = read_animation(animation_path, frame)
    if names != [joint[0] for joint in joints]:
        raise ValueError('Pillar mesh and animation bones differ')
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    report = []
    for slot, section in enumerate(sections):
        shader, verts, tris, _ = section
        positions = skinned_vertices(section, world)
        lows = [min(point[axis] for point in positions) for axis in range(3)]
        highs = [max(point[axis] for point in positions) for axis in range(3)]
        lines = [f'# source: Skel_SanctuaryCentralPillar Open frame {frame}, slot {slot}',
                 f'g pillar_s{slot}']
        lines.extend('v ' + ' '.join(format(v, '.9g') for v in point) for point in positions)
        def atlas_uv(uv):
            if uv_atlas is None:
                return uv
            su, sv, ou, ov = uv_atlas[slot]
            return (uv[0]*su+ou, uv[1]*sv+ov)
        # UModel's MD5 V is top-down like UE; OBJ V is bottom-up. Flip it the
        # same way ow-package's own OBJ writer does (src/assets.cpp).
        lines.extend('vt {} {}'.format(*(format(v, '.9g') for v in (lambda u: (u[0], 1 - u[1]))(atlas_uv(verts[i][0]))))
                     for i in range(len(verts)))
        # MD5 is right handed. Mirroring its Y into UE3 coordinates reverses
        # triangle order. The host OBJ adapter handles the UE5 OBJ convention.
        lines.extend(f'f {c+1}/{c+1} {b+1}/{b+1} {a+1}/{a+1}'
                     for a, b, c in (tris[i] for i in range(len(tris))))
        filename = f'center_pillar_s{slot}.obj'
        (output / filename).write_text('\n'.join(lines) + '\n', encoding='utf-8')
        report.append({'slot': slot, 'shader': shader, 'file': filename,
                       'vertices': len(verts), 'triangles': len(tris),
                       'bounds_min': lows, 'bounds_max': highs})
    return report


def add_to_scene(scene_path, reader, game, mesh_path, animation_path):
    """Add the lowered preflight pillar as four frozen visual sections."""
    scene_path = Path(scene_path).resolve()
    output = scene_path.parent
    manifest = json.loads(scene_path.read_text(encoding='utf-8'))
    if manifest.get('map') != 'Sanctuary_P' or 'Sanctuary_Dynamic' not in manifest.get('levels', []):
        raise ValueError('Central pillar requires a prepared Sanctuary_P scene')
    prepared = Scene(Path(reader), Path(game), output)
    records = prepared.load('Sanctuary_Dynamic')
    actor = next((r for r in records.values() if r['path'] == PILLAR_SOURCE), None)
    mesh = next((r for r in records.values() if r['path'] == PILLAR_MESH), None)
    if actor is None or actor['class'] != 'Engine.SkeletalMeshComponent' or mesh is None or mesh['class'] != 'Engine.SkeletalMesh':
        raise ValueError('Observed Sanctuary central-pillar component/mesh identity changed')
    material_ids = []
    for slot, path in enumerate(MATERIAL_PATHS):
        key = ('Sanctuary_Dynamic', material_index(records, path))
        material = props(records[key[1]])
        texture_values = {values(entry).get('ParameterName'): values(entry).get('ParameterValue')
                          for entry in material.get('TextureParameterValues', [])}
        color = texture_values.get('Color', {})
        expected_color = ('SancPillarTopInner_Dif' if slot == 1 else 'SancSpire_Col')
        if not color.get('path', '').endswith('.' + expected_color):
            raise ValueError(f'Unexpected pillar color atlas in slot {slot}')
        transform = next((values(entry).get('ParameterValue') for entry in material.get('VectorParameterValues', [])
                          if values(entry).get('ParameterName') == 'Color_UV_Scale & Offset'), None)
        observed = tuple(transform[k] for k in ('R', 'G', 'B', 'A')) if transform else None
        if observed is None or any(abs(a-b) > 0.001 for a, b in zip(observed, UV_ATLAS[slot])):
            raise ValueError(f'Unexpected pillar color UV transform in slot {slot}: {observed}')
        luminosity = texture_values.get('Luminosity', {})
        if not luminosity.get('path', '').endswith('.' + LUMINOSITY_TEXTURES[slot]):
            raise ValueError(f'Unexpected pillar luminosity texture in slot {slot}')
        name = prepared.material(key)
        definition = prepared.materials[name]
        color_key = prepared.resolve(key[0], color)
        color_file = prepared.texture(color_key, 'diffuse')
        sources = [prepared.identity(color_key)]
        if LUMINOSITY_CHANNEL[slot] is None:
            definition['channels']['diffuse'] = color_file
        else:
            luminosity_key = prepared.resolve(key[0], luminosity)
            luminosity_file = prepared.texture(luminosity_key, 'luminosity')
            baked = name + '_spire_diffuse.png'
            bake_spire_diffuse(output / color_file, output / luminosity_file,
                               LUMINOSITY_CHANNEL[slot], UV_ATLAS[slot], output / baked)
            definition['channels']['diffuse'] = baked
            sources.append(prepared.identity(luminosity_key) + ' channel ' + 'RGB'[LUMINOSITY_CHANNEL[slot]])
        # p_emissive defaults to 0 and no instance raises it; the glow belongs
        # to the takeoff sequence, not the landed town.
        definition['channels'].pop('emissive', None)
        definition['surface_approximation'] = {
            'method': 'spire_luminosity_times_color_v1', 'status': 'partial_unverified',
            'source_textures': sources,
            'combine': 'UNVERIFIED 2 x luminosity x color strip (cooked graph stripped)',
            'omitted': ['emissive (p_emissive 0 before takeoff)', 'dynamic material parameters']}
        material_ids.append(name)
    sections = bake(mesh_path, animation_path, 0, output)
    mesh_id = hashlib.sha256(('Sanctuary_Dynamic:' + PILLAR_MESH + ':Open:0').encode()).hexdigest()[:24]
    manifest['actors'] = [a for a in manifest['actors'] if a['source'] != PILLAR_SOURCE]
    manifest['issues'] = [i for i in manifest['issues'] if i.get('object') != PILLAR_SOURCE]
    manifest['materials'].update(prepared.materials)
    manifest['meshes'][mesh_id] = {
        'source': 'Sanctuary_Dynamic:' + PILLAR_MESH,
        'sections': [{'slot': item['slot'], 'file': item['file'], 'material': material_ids[item['slot']]}
                     for item in sections],
        'collision': {'status': 'absent', 'hulls': []},
        'pose': {'clip': 'Open', 'frame': 0, 'source': 'UModel MD5 animation'}}
    manifest['actors'].append({
        'source': PILLAR_SOURCE, 'level': 'Sanctuary_Dynamic', 'mesh': mesh_id,
        'transform': {'actor': {'location': PREFLIGHT_LOCATION,
                                'rotation': [0, 0, 0], 'scale': [0.67, 0.67, 0.67]},
                      'component': {'location': [0, 0, 0], 'rotation': [0, 0, 0],
                                    'scale': [1, 1, 1]}},
        'materials': material_ids, 'static': False, 'collision_enabled': False,
        'center_pillar': True})
    manifest['center_pillar_policy'] = {
        'state': 'landed_preflight', 'pose': 'Open frame 0',
        'placement_source': 'Sanctuary_Dynamic Episode_8 InterpData_0 Spire first world key',
        'placement': PREFLIGHT_LOCATION,
        'mesh_source': 'UModel build 1590 MD5 export',
        'mesh_sha256': hashlib.sha256(Path(mesh_path).read_bytes()).hexdigest(),
        'animation_sha256': hashlib.sha256(Path(animation_path).read_bytes()).hexdigest(),
        'visual_validation': 'UNVERIFIED against paired original-game camera',
        'collision': 'absent'}
    manifest['issues'].append({'object': PILLAR_SOURCE,
                               'error': 'Approximation: frozen Open frame 0; animation, collision and full UE3 material graph are not reconstructed'})
    temporary = scene_path.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    temporary.replace(scene_path)
    return {'source': PILLAR_SOURCE, 'mesh': mesh_id, 'sections': sections,
            'materials': material_ids, 'pose': 'Open frame 0'}


def export_md5(umodel, game, output):
    cooked = Path(game).resolve() / 'WillowGame/CookedPCConsole'
    if not (cooked / 'Sanctuary_Dynamic.upk').is_file():
        raise ValueError('Installed Sanctuary_Dynamic.upk is missing')
    if Path(umodel).name.lower() != 'umodel.exe':
        raise ValueError('Expected the external UModel executable')
    destination = Path(output).resolve() / ('center-pillar-umodel-' + uuid.uuid4().hex[:8])
    destination.mkdir(parents=True, exist_ok=True)
    for object_name in ('Skel_SanctuaryCentralPillar', 'Anim_CentralPillar'):
        command = [str(Path(umodel).resolve()), f'-path={cooked}', '-game=border',
                   '-export', '-md5', '-lods', '-dds', '-nooverwrite', '-uncook',
                   f'-out={destination}', 'Sanctuary_Dynamic', object_name]
        run = subprocess.run(command, capture_output=True, text=True, encoding='utf-8')
        (destination / (object_name + '.log')).write_text(
            'Command: ' + ' '.join(command) + '\n' + run.stdout + run.stderr, encoding='utf-8')
        if run.returncode or 'ERROR:' in run.stdout + run.stderr:
            raise ValueError(f'UModel export failed for {object_name}: {run.stdout} {run.stderr}')
    mesh = destination / 'Prop_SancBuildings_02/SkeletalMesh3/Skel_SanctuaryCentralPillar.md5mesh'
    animation = destination / 'Anim_Sanctuary/AnimSet/Anim_CentralPillar/Open.md5anim'
    if not mesh.is_file() or not animation.is_file():
        raise ValueError('UModel did not export the expected pillar mesh and Open animation')
    return mesh, animation


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mesh', type=Path)
    parser.add_argument('--animation', type=Path)
    parser.add_argument('--umodel', type=Path, help='Export the two objects with external UModel first')
    parser.add_argument('--frame', type=int, default=0)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--scene', type=Path, help='Prepared Sanctuary scene.json to enrich')
    parser.add_argument('--reader', type=Path)
    parser.add_argument('--game', type=Path)
    args = parser.parse_args()
    if args.umodel:
        if args.mesh or args.animation or not args.game:
            parser.error('--umodel requires --game and replaces --mesh/--animation')
        args.mesh, args.animation = export_md5(args.umodel, args.game, args.output)
    if not args.mesh or not args.animation:
        parser.error('Pass --umodel and --game, or both --mesh and --animation')
    if args.scene:
        if not args.reader or not args.game or args.frame != 0 or args.output.resolve() != args.scene.resolve().parent:
            parser.error('Scene integration requires --reader, --game, frame 0, and output at the scene directory')
        result = add_to_scene(args.scene, args.reader, args.game, args.mesh, args.animation)
    else:
        result = bake(args.mesh, args.animation, args.frame, args.output)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()

