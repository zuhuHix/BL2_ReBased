"""Recompute each local recipe's gestalt fragments from its chosen parts and rebuild its filtered glTF.

Why: the first recipes took only GestaltModeSkeletalMeshName from each part. The running game also draws the
part's AdditionalGestaltModeSkeletalMeshNames (body variants) and nothing for *_None parts
(tools/weapon_recipe.part_fragments; docs/verification/WEAPON_VISUALS.md). This script keeps every recipe's
rolled parts, stats and name as they are and only replaces `gestalt_fragments`/`unresolved_fragments`, `sockets`
(tools/weapon_slice_gear.attach_sockets) and the <id>.gltf/.bin beside it, so no part is re-rolled. Inputs and
outputs are ignored local/ files. With --sockets-only nothing but `sockets` changes (no fragment or mesh rewrite).

  python tools/weapon_refresh_fragments.py --game "<BL2 install>" --dir local/items/slice local/items \\
      --gestalt local/gestalt --gltf local/external/umodel/gestalt/Startup/SkeletalMesh3 [--ids infinity_3 ...] [--sockets-only]
Prints one line per recipe: id, fragments, triangle total of the rebuilt mesh, the sockets attached.
"""
import argparse
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import weapon_recipe  # noqa: E402
import weapon_slice_gear  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]


def triangles(gltf_path):
    doc = json.loads(gltf_path.read_text(encoding='utf-8'))
    return sum(doc['accessors'][p['indices']]['count'] for p in doc['meshes'][0]['primitives']) // 3


def hidden_bones(package, recipe):
    """Bones the weapon type hides on its meshes (BoneToHideOnMesh, AdditionalBoneToHideOnMesh)."""
    props = package.props(recipe['weapon_type']) if recipe.get('weapon_type') else {}
    return [b for b in (props.get('BoneToHideOnMesh'), props.get('AdditionalBoneToHideOnMesh')) if b and b != 'None']


def drop_hidden_bone_triangles(gltf_path, bones):
    """Remove the triangles whose vertices are mostly skinned to one of `bones`; returns the number removed.

    The running game hides those bones (WillowWeapon HideBonesInMesh, docs/verification/NATIVE_WEAPON_VISUALS.md), e.g. the
    Jakobs pistol's MoonClip and Bullet, which are alternate magazine pieces that lie outside the gun in the bind pose.
    The host plays no weapon animation, so the pieces are cut from the glTF instead. A vertex counts for the bone of its
    largest weight; a triangle goes when at least two of its vertices do.
    """
    import numpy as np
    doc = json.loads(gltf_path.read_text(encoding='utf-8'))
    blob = bytearray((gltf_path.parent / doc['buffers'][0]['uri']).read_bytes())
    names = [doc['nodes'][j].get('name') for j in doc['skins'][0]['joints']]
    hide = {i for i, n in enumerate(names) if n in set(bones)}
    types = {5121: 'u1', 5123: '<u2', 5125: '<u4', 5126: '<f4'}
    widths = {'SCALAR': 1, 'VEC4': 4}

    def read(index):
        acc = doc['accessors'][index]
        view = doc['bufferViews'][acc['bufferView']]
        count, width = acc['count'], widths[acc['type']]
        dtype = np.dtype(types[acc['componentType']])
        offset = view.get('byteOffset', 0) + acc.get('byteOffset', 0)
        stride = view.get('byteStride') or dtype.itemsize * width
        if stride != dtype.itemsize * width:
            raise RuntimeError('interleaved attribute buffers are not handled')
        return np.frombuffer(bytes(blob), dtype=dtype, count=count * width, offset=offset).reshape(count, width)

    removed = 0
    for primitive in doc['meshes'][0]['primitives']:
        joints, weights = read(primitive['attributes']['JOINTS_0']), read(primitive['attributes']['WEIGHTS_0'])
        dominant = joints[np.arange(len(joints)), np.argmax(weights, axis=1)]
        hidden = np.isin(dominant, list(hide))
        tris = read(primitive['indices']).reshape(-1, 3)
        keep = tris[hidden[tris].sum(axis=1) < 2]
        removed += len(tris) - len(keep)
        while len(blob) % 4:
            blob.append(0)
        data = keep.astype('<u2').tobytes()
        doc['bufferViews'].append({'buffer': 0, 'byteOffset': len(blob), 'byteLength': len(data), 'target': 34963})
        blob += data
        doc['accessors'].append({'bufferView': len(doc['bufferViews']) - 1, 'componentType': 5123,
                                 'count': int(keep.size), 'type': 'SCALAR'})
        primitive['indices'] = len(doc['accessors']) - 1
    doc['buffers'][0] = {'uri': doc['buffers'][0]['uri'], 'byteLength': len(blob)}
    (gltf_path.parent / doc['buffers'][0]['uri']).write_bytes(bytes(blob))
    gltf_path.write_text(json.dumps(doc), encoding='utf-8')
    return removed


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install folder')
    parser.add_argument('--dir', type=Path, nargs='+', required=True, help='recipe folders (<id>.json with <id>.gltf)')
    parser.add_argument('--gestalt', type=Path, required=True)
    parser.add_argument('--gltf', type=Path, required=True)
    parser.add_argument('--schema', type=Path, default=ROOT / 'local/items/slice/slice_gear.schema')
    parser.add_argument('--ids', nargs='*', help='only these recipe ids')
    parser.add_argument('--sockets-only', action='store_true',
                        help='only attach `sockets` to each recipe as it stands; fragments, meshes and bones are left alone')
    args = parser.parse_args()

    startup = args.game / 'WillowGame/CookedPCConsole/Startup.upk'
    package = weapon_recipe.Package(str(Path(args.reader).resolve()), startup, str(args.schema.resolve()))
    for folder in args.dir:
        for path in sorted(folder.glob('*.json')):
            if args.ids and path.stem not in args.ids:
                continue
            recipe = json.loads(path.read_text(encoding='utf-8-sig'))
            if 'parts' not in recipe or 'gestalt' not in recipe:
                continue  # manifests and schemas
            if args.sockets_only:
                status = weapon_slice_gear.attach_sockets(recipe, args.gestalt)
                path.write_text(json.dumps(recipe, indent=1), encoding='utf-8')
                print(f'{path.stem}: sockets={sorted(recipe.get("sockets", {}))} ({status})')
                continue
            recipe.pop('unresolved_fragments', None)
            recipe['gestalt_fragments'] = sorted({n for p in recipe['parts'].values()
                                                  for n in weapon_recipe.part_fragments(package, p['part'])})
            sockets = weapon_slice_gear.attach_sockets(recipe, args.gestalt)
            status = weapon_slice_gear.build_mesh(path, recipe, args.gestalt, args.gltf)
            gltf = path.with_suffix('.gltf')
            total = triangles(gltf) if status == 'ok' and gltf.exists() else None  # the game's totals (before hiding bones)
            bones = hidden_bones(package, recipe)
            recipe['bones_to_hide'] = bones
            cut = drop_hidden_bone_triangles(gltf, bones) if bones and status == 'ok' else 0
            path.write_text(json.dumps(recipe, indent=1), encoding='utf-8')
            print(f'{path.stem}: {status} fragments={recipe["gestalt_fragments"]} '
                  f'unresolved={recipe.get("unresolved_fragments", [])} triangles={total} hidden bones={bones} cut={cut} '
                  f'sockets={sorted(recipe.get("sockets", {}))} ({sockets})')


if __name__ == '__main__':
    main()
