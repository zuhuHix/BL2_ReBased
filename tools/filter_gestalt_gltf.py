"""Keep only selected gestalt part fragments of a UModel skeletal-mesh glTF.

A BL2 GestaltSkeletalMeshDefinition stores every part variant in one mesh.
Its GestaltInfos[0].Parts entries name a fragment and give its MaterialIndex,
FirstIndex (into the LOD index buffer) and NumPrimitives (triangles). Those
fields are read by `ow-package --properties` with a local array schema; this
script only consumes that JSON and the local UModel glTF, both under ignored
local/ directories.

Assumption (checked below, not a format guarantee): UModel writes one glTF
primitive per LOD section in material order, so a section's triangles start
at the sum of the earlier sections' index counts.
"""
import argparse
import json
import struct
from pathlib import Path


def gestalt_parts(properties_json):
    data = json.loads(Path(properties_json).read_text(encoding='utf-8-sig'))
    infos = next(p for p in data['properties'] if p['name'] == 'GestaltInfos')
    parts = infos['value'][0][0]
    if parts['name'] != 'Parts' or parts.get('status') == 'unsupported':
        raise RuntimeError('GestaltInfos[0].Parts was not decoded; pass the array schema')
    return [{f['name']: f['value'] for f in part} for part in parts['value']]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--gltf', required=True)
    parser.add_argument('--gestalt', required=True, help='ow-package --properties JSON of the GestaltDef')
    parts = parser.add_mutually_exclusive_group(required=True)
    parts.add_argument('--parts', nargs='+', help='SkeletalMeshFragmentName values to keep')
    parts.add_argument('--recipe', help='tools/weapon_recipe.py output; keeps its gestalt_fragments')
    parser.add_argument('--output', required=True, help='output .gltf; a .bin is written beside it')
    args = parser.parse_args()

    source = Path(args.gltf)
    gltf = json.loads(source.read_text(encoding='utf-8'))
    if len(gltf['meshes']) != 1 or len(gltf['buffers']) != 1:
        raise RuntimeError('expected one mesh and one buffer')
    blob = bytearray((source.parent / gltf['buffers'][0]['uri']).read_bytes())
    primitives = gltf['meshes'][0]['primitives']
    parts = gestalt_parts(args.gestalt)

    # Section triangle totals must match the gestalt ranges exactly, or the
    # section-order assumption above does not hold for this export.
    starts, first = [], 0
    for material, primitive in enumerate(primitives):
        count = gltf['accessors'][primitive['indices']]['count']
        expected = 3 * sum(p['NumPrimitives'] for p in parts if p['MaterialIndex'] == material)
        if primitive.get('material') != material or count != expected:
            raise RuntimeError(f'section {material}: {count} indices, gestalt ranges give {expected}')
        starts.append(first)
        first += count

    wanted = set(args.parts) if args.parts else set(
        json.loads(Path(args.recipe).read_text(encoding='utf-8'))['gestalt_fragments'])
    found = {p['SkeletalMeshFragmentName'] for p in parts} & wanted
    if found != wanted:
        raise RuntimeError(f'unknown fragments: {sorted(wanted - found)}')

    kept_primitives, report = [], []
    for material, primitive in enumerate(primitives):
        accessor = gltf['accessors'][primitive['indices']]
        view = gltf['bufferViews'][accessor['bufferView']]
        if accessor['componentType'] != 5123:
            raise RuntimeError('expected 16-bit indices')
        base = view.get('byteOffset', 0) + accessor.get('byteOffset', 0)
        indices = []
        for part in parts:
            if part['MaterialIndex'] != material or part['SkeletalMeshFragmentName'] not in wanted:
                continue
            local = part['FirstIndex'] - starts[material]
            count = 3 * part['NumPrimitives']
            indices += struct.unpack_from(f'<{count}H', blob, base + 2 * local)
            report.append({'fragment': part['SkeletalMeshFragmentName'], 'material': material,
                           'triangles': part['NumPrimitives']})
        if not indices:
            continue
        while len(blob) % 4:
            blob.append(0)
        gltf['bufferViews'].append({'buffer': 0, 'byteOffset': len(blob), 'byteLength': 2 * len(indices),
                                    'target': 34963})
        blob += struct.pack(f'<{len(indices)}H', *indices)
        gltf['accessors'].append({'bufferView': len(gltf['bufferViews']) - 1, 'componentType': 5123,
                                  'count': len(indices), 'type': 'SCALAR'})
        kept = dict(primitive)
        kept['indices'] = len(gltf['accessors']) - 1
        kept_primitives.append(kept)

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    binary = output.with_suffix('.bin')
    binary.write_bytes(blob)
    gltf['buffers'][0] = {'uri': binary.name, 'byteLength': len(blob)}
    gltf['meshes'][0]['primitives'] = kept_primitives
    output.write_text(json.dumps(gltf), encoding='utf-8')
    print(json.dumps({'output': str(output), 'fragments': report}, indent=1))


if __name__ == '__main__':
    main()
