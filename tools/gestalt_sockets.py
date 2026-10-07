"""Decode each weapon family's gestalt definition (part table, bounds, socket mappings) and its mesh sockets.

Writes, under ignored local/gestalt/ (game-derived data, never committed):
- <Kind>.json         ow-package --properties of the GestaltDef with tools/gestalt-arrays.schema: the part table that
                      tools/filter_gestalt_gltf.py reads, plus GestaltPartBounds and GestaltSocketMappings.
- <Kind>.sockets.json per fragment name: its reference-pose bounds and its sockets keyed by the original socket name
                      (Muzzle, EjectPort, FrontSight, ...). Each socket has the mangled name, bone, location, rotation, scale
                      and, when a UModel glTF is given, `mesh_location`: the same point in mesh space in the reference pose.

How it fits together (docs/verification/WEAPON_VISUALS.md section 11):
- A GestaltSocketMapping says "fragment F calls its socket <Original>; in the shared mesh that socket is <Mangled>".
- The shared GestaltSkeletalMesh holds the sockets (SkeletalMeshSocket objects: bone, relative location/rotation/scale).
  The cooked stream omits fields that equal the engine default, so a missing rotation is 0 and a missing scale is 1.
- Units are the game's (centimetres, mesh axes of the cooked data). The UModel glTF is the same mesh in metres with y and z
  swapped; the host imports it back, so the mesh space of the host equals the cooked one (checked against the fragment bounds
  in `check_inside_bounds`; for the host side see WEAPON_VISUALS.md).

  python tools/gestalt_sockets.py --game "<BL2 install>" [--gltf local/external/umodel/gestalt/Startup/SkeletalMesh3]
"""
import argparse
import json
import math
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = ROOT / 'tools/gestalt-arrays.schema'
# family -> GestaltDef export path in Startup.upk (the six families the slice pipeline uses)
FAMILIES = {
    'Pistol': 'Weap_Pistol.GestaltDef_Pistol',
    'SMG': 'Weap_SMG.GestaltDef_SMG',
    'AssaultRifle': 'Weap_AssaultRifles.GestaltDef_AssaultRifle',
    'Shotgun': 'Weap_Shotguns.GestaltDef_Shotgun',
    'SniperRifle': 'Weap_SniperRifles.GestaltDef_SniperRifle',
    'Launcher': 'Weap_Launchers.GestaltDef_Launcher',
}


def fields(entries):
    """A decoded struct's [{name, value}, ...] -> {name: value}."""
    return {f['name']: f['value'] for f in entries}


def vector(value, default=(0.0, 0.0, 0.0)):
    return [value['X'], value['Y'], value['Z']] if value else list(default)


def property_value(data, name):
    """Value of the named top-level property of one --properties object (None when absent or undecoded)."""
    for p in data['properties']:
        if p['name'] == name and p.get('status') != 'unsupported':
            return p['value']
    return None


def parse_socket(data):
    """One SkeletalMeshSocket export -> (SocketName, {bone, location, rotation, scale}).

    Omitted fields are the engine defaults: location 0, rotation 0 (a Rotator in 65536 per turn), scale 1.
    """
    rotation = property_value(data, 'RelativeRotation')
    return property_value(data, 'SocketName'), {
        'bone': property_value(data, 'BoneName'),
        'location': vector(property_value(data, 'RelativeLocation')),
        'rotation': [rotation['Pitch'], rotation['Yaw'], rotation['Roll']] if rotation else [0, 0, 0],
        'scale': vector(property_value(data, 'RelativeScale'), (1.0, 1.0, 1.0)),
    }


def parse_bounds(value):
    """GestaltPartBounds[i] struct fields -> (fragment name, bounds dict)."""
    row = fields(value)
    box = fields(row['ReferencePoseBounds'])
    return row['SkeletalMeshFragmentName'], {'origin': vector(box.get('Origin')), 'extent': vector(box.get('BoxExtent')),
                                             'radius': box.get('SphereRadius', 0.0)}


def fragment_sockets(table, sockets):
    """{fragment: {'bounds': ..., 'sockets': {Original: {socket, bone, location, rotation, scale}}}}.

    `table` is the GestaltDef's --properties object, `sockets` {Mangled: socket dict} from parse_socket. A mapping whose
    mangled socket is not in the mesh is listed under 'missing' instead of being guessed.
    """
    result, missing = {}, []
    for value in property_value(table, 'GestaltPartBounds') or []:
        name, bounds = parse_bounds(value)
        result.setdefault(name, {'bounds': bounds, 'sockets': {}})
    for value in property_value(table, 'GestaltSocketMappings') or []:
        row = fields(value)
        fragment, original, mangled = row['SkeletalMeshFragmentName'], row['OriginalSocketName'], row['MangledSocketName']
        entry = result.setdefault(fragment, {'bounds': None, 'sockets': {}})
        if mangled not in sockets:
            missing.append({'fragment': fragment, 'original': original, 'mangled': mangled})
        elif original in entry['sockets']:
            raise RuntimeError(f'{fragment} maps {original} twice')
        else:
            entry['sockets'][original] = {'socket': mangled, **sockets[mangled]}
    return result, missing


# ----------------------------------------------------------------------------------- reference pose (glTF)


def quat_matrix(q):
    x, y, z, w = q
    return [[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]]


def mat_vec(m, v):
    return [sum(m[r][c] * v[c] for c in range(3)) for r in range(3)]


def mat_mul(a, b):
    return [[sum(a[r][k] * b[k][c] for k in range(3)) for c in range(3)] for r in range(3)]


def bone_pose(gltf, bone):
    """(rotation matrix, translation) of a bone in glTF mesh space in the reference pose (node chain up to the root)."""
    nodes = gltf['nodes']
    parent = {child: i for i, node in enumerate(nodes) for child in node.get('children', [])}
    index = next((i for i, n in enumerate(nodes) if n.get('name') == bone), None)
    if index is None:
        raise KeyError(bone)
    chain = [index]
    while chain[-1] in parent:
        chain.append(parent[chain[-1]])
    rotation, translation = [[1, 0, 0], [0, 1, 0], [0, 0, 1]], [0.0, 0.0, 0.0]
    for i in reversed(chain):  # root first
        node = nodes[i]
        local_rotation = quat_matrix(node.get('rotation', [0, 0, 0, 1]))
        offset = mat_vec(rotation, node.get('translation', [0, 0, 0]))
        translation = [translation[k] + offset[k] for k in range(3)]
        rotation = mat_mul(rotation, local_rotation)
    return rotation, translation


def mesh_location(gltf, bone, location):
    """Socket location in cooked mesh space (cm): bone pose x location, glTF metres with y and z swapped back.

    The bone frame is the glTF node frame conjugated by the same y/z swap as the points (the swap is its own inverse), so a
    bone-local offset in the cooked data is applied as it stands. The swap changes handedness, which is why the cooked data
    and the UModel export agree only after it (UNVERIFIED as a statement about UModel; the fragment-bounds check in
    check_inside_bounds is the evidence).
    """
    rotation, translation = bone_pose(gltf, bone)
    swap = [[1, 0, 0], [0, 0, 1], [0, 1, 0]]
    rotation = mat_mul(mat_mul(swap, rotation), swap)
    translation = mat_vec(swap, translation)
    point = mat_vec(rotation, location)
    return [round(100.0 * translation[k] + point[k], 4) for k in range(3)]


def check_inside_bounds(point, bounds, margin=5.0):
    """True when `point` lies within the fragment's reference-pose box grown by `margin` cm (a muzzle sits just past the front face: at most 3.3 cm in the six families)."""
    return all(abs(point[k] - bounds['origin'][k]) <= bounds['extent'][k] + margin for k in range(3))


# ----------------------------------------------------------------------------------- reader calls


def run_reader(reader, package, *args):
    done = subprocess.run([str(reader), str(package), *map(str, args)], capture_output=True, text=True)
    if done.returncode:
        raise RuntimeError(f'ow-package {args}: {done.stderr.strip()}')
    return done.stdout


def decode_family(reader, package, exports, path, mesh_gltf):
    table = json.loads(run_reader(reader, package, '--properties', exports[path], '--property-offset', 4,
                                  '--array-schema', SCHEMA))
    mesh = property_value(table, 'GestaltSkeletalMesh')
    mesh_data = json.loads(run_reader(reader, package, '--properties', mesh['index'], '--property-offset', 4,
                                      '--array-schema', SCHEMA))
    handle = tempfile.NamedTemporaryFile('w', suffix='.txt', delete=False)
    try:
        with handle:
            handle.write('\n'.join(str(s['index']) for s in property_value(mesh_data, 'Sockets') or []) + '\n')
        lines = run_reader(reader, package, '--properties-batch', handle.name, '--property-offset', 4).splitlines()
    finally:
        Path(handle.name).unlink(missing_ok=True)
    sockets = {}
    for line in lines:
        data = json.loads(line)
        if 'error' in data:
            raise RuntimeError(f'socket {data["index"]}: {data["error"]}')
        name, socket = parse_socket(data)
        sockets[name] = socket
    fragments, missing = fragment_sockets(table, sockets)
    if mesh_gltf:
        for entry in fragments.values():
            for socket in entry['sockets'].values():
                socket['mesh_location'] = mesh_location(mesh_gltf, socket['bone'], socket['location'])
    return table, mesh['path'], sockets, fragments, missing


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install folder')
    parser.add_argument('--output', type=Path, default=ROOT / 'local/gestalt')
    parser.add_argument('--gltf', type=Path, help='folder of GestaltDef_<Kind>_GestaltSkeletalMesh.gltf (adds mesh_location)')
    parser.add_argument('--kinds', nargs='*', default=list(FAMILIES))
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to((ROOT / 'local').resolve()):
        parser.error('--output must stay under local/')
    args.output.mkdir(parents=True, exist_ok=True)
    reader = Path(args.reader).resolve()
    package = args.game / 'WillowGame/CookedPCConsole/Startup.upk'
    exports = {e['path']: e['index'] for e in json.loads(run_reader(reader, package, '--exports'))}
    failed = False
    for kind in args.kinds:
        gltf = None
        if args.gltf:
            source = args.gltf / f'GestaltDef_{kind}_GestaltSkeletalMesh.gltf'
            gltf = json.loads(source.read_text(encoding='utf-8')) if source.is_file() else None
        table, mesh, sockets, fragments, missing = decode_family(reader, package, exports, FAMILIES[kind], gltf)
        (args.output / f'{kind}.json').write_text(json.dumps(table), encoding='utf-8')
        out = {'kind': kind, 'gestalt': FAMILIES[kind], 'mesh': mesh, 'units': 'game centimetres, cooked mesh axes',
               'rotation_units': 'Rotator, 65536 per turn', 'reference_pose': 'mesh_location' if gltf else 'not computed',
               'missing_mappings': missing, 'fragments': fragments}
        (args.output / f'{kind}.sockets.json').write_text(json.dumps(out, indent=1), encoding='utf-8')
        mapped = sum(len(f['sockets']) for f in fragments.values())
        outside = [f'{name}.{original}' for name, f in fragments.items() if f['bounds'] and gltf
                   for original, s in f['sockets'].items() if original == 'Muzzle'
                   and not check_inside_bounds(s['mesh_location'], f['bounds'])]
        print(f'{kind}: {len(fragments)} fragments, {len(sockets)} mesh sockets, {mapped} mappings, '
              f'{len(missing)} without a socket, muzzles outside their fragment bounds: {outside}')
        failed = failed or bool(missing)
    sys.exit(1 if failed else 0)


if __name__ == '__main__':
    main()
