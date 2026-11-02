"""Prepare the weapon part library: each gestalt mesh once, one material slot per fragment.

A BL2 weapon type keeps every part of every manufacturer in one gestalt skeletal mesh; the game draws only the
fragments its parts name (tools/weapon_recipe.part_fragments; docs/verification/WEAPON_PART_ASSETS.md). Instead of a
filtered mesh per rolled gun (tools/filter_gestalt_gltf.py), the library imports each gestalt once and the host
shows or hides fragments at run time (OpenWillowWeaponAssembly.cpp). For that every fragment becomes its own glTF
primitive with its own material, named after the fragment, so it imports as one UE material slot and section.

Inputs are the UModel 1590 glTF export of the gestalt mesh and its fragment table (ow-package --properties of the
GestaltDef, read by filter_gestalt_gltf.gestalt_parts). Outputs (game-derived, ignored local/ only):
  <out>/<Type>/SK_Gestalt_<Type>.gltf/.bin   the split mesh
  <out>/<Type>/fragments.json                fragment -> slot, triangles, source sections
  <out>/library.json                         index of the types written

  python tools/weapon_library.py meshes --gestalt local/gestalt \\
      --gltf local/external/umodel/gestalt/Startup/SkeletalMesh3 --output local/weapon_library

The split keeps vertices, skin and nodes unchanged; only index ranges are regrouped, so the triangle total and every
fragment's triangle count equal the cooked table (checked here, and again in UE by the assembly's triangle report).
"""
import argparse
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from filter_gestalt_gltf import gestalt_parts  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
TYPES = ['Pistol', 'SMG', 'AssaultRifle', 'Shotgun', 'SniperRifle', 'Launcher']
UE_ROOT = '/Game/OpenWillow/Weapons/Library'


def mesh_asset(kind):
    return f'{UE_ROOT}/{kind}/SK_Gestalt_{kind}.SK_Gestalt_{kind}'


def group_fragments(parts):
    """Fragment name -> its index ranges, in first-seen table order; a name may own several ranges."""
    grouped = {}
    for part in parts:
        grouped.setdefault(part['SkeletalMeshFragmentName'], []).append(
            {'material': part['MaterialIndex'], 'first_index': part['FirstIndex'], 'triangles': part['NumPrimitives']})
    return grouped


def section_starts(gltf, parts):
    """Index offset of each glTF primitive inside the LOD index buffer; refuses an export that does not match the table."""
    starts, first = [], 0
    for material, primitive in enumerate(gltf['meshes'][0]['primitives']):
        count = gltf['accessors'][primitive['indices']]['count']
        expected = 3 * sum(p['NumPrimitives'] for p in parts if p['MaterialIndex'] == material)
        if primitive.get('material') != material or count != expected:
            raise ValueError(f'section {material}: {count} indices, gestalt ranges give {expected}')
        starts.append(first)
        first += count
    return starts


def split(gltf, blob, parts):
    """Rewrite gltf/blob in place: one primitive per fragment. Returns the fragment records for fragments.json."""
    primitives = gltf['meshes'][0]['primitives']
    if len(gltf['meshes']) != 1 or len(gltf['buffers']) != 1:
        raise ValueError('expected one mesh and one buffer')
    starts = section_starts(gltf, parts)
    source_materials = [m.get('name', f'material_{i}') for i, m in enumerate(gltf.get('materials', []))]
    new_primitives, materials, records = [], [], []
    for slot, (name, ranges) in enumerate(group_fragments(parts).items()):
        indices = []
        for r in ranges:
            primitive = primitives[r['material']]
            accessor = gltf['accessors'][primitive['indices']]
            if accessor['componentType'] != 5123:
                raise ValueError('expected 16-bit indices')
            view = gltf['bufferViews'][accessor['bufferView']]
            base = view.get('byteOffset', 0) + accessor.get('byteOffset', 0)
            local = r['first_index'] - starts[r['material']]
            count = 3 * r['triangles']
            if local < 0 or local + count > accessor['count']:
                raise ValueError(f'{name}: range outside section {r["material"]}')
            indices += struct.unpack_from(f'<{count}H', blob, base + 2 * local)
        # All ranges of one fragment must share the vertex attributes of one source primitive (UModel writes one
        # vertex buffer for the whole LOD, so this holds; checked rather than assumed).
        attributes = {json.dumps(primitives[r['material']]['attributes'], sort_keys=True) for r in ranges}
        if len(attributes) != 1:
            raise ValueError(f'{name}: ranges use different vertex attributes')
        while len(blob) % 4:
            blob.append(0)
        gltf['bufferViews'].append({'buffer': 0, 'byteOffset': len(blob), 'byteLength': 2 * len(indices),
                                    'target': 34963})
        blob += struct.pack(f'<{len(indices)}H', *indices)
        gltf['accessors'].append({'bufferView': len(gltf['bufferViews']) - 1, 'componentType': 5123,
                                  'count': len(indices), 'type': 'SCALAR'})
        primitive = dict(primitives[ranges[0]['material']])
        primitive['indices'] = len(gltf['accessors']) - 1
        primitive['material'] = slot
        new_primitives.append(primitive)
        materials.append({'name': name, 'pbrMetallicRoughness': {'baseColorFactor': [0.3, 0.3, 0.3, 1.0]}})
        records.append({'name': name, 'slot': slot, 'triangles': len(indices) // 3,
                        'source_sections': sorted({r['material'] for r in ranges}),
                        'source_materials': sorted({source_materials[r['material']] for r in ranges
                                                    if r['material'] < len(source_materials)})})
    gltf['meshes'][0]['primitives'] = new_primitives
    gltf['materials'] = materials
    return records


def to_ue(point):
    """glTF (metres, Y up) -> the imported UE mesh's component space (cm, Z up): (100x, 100z, 100y). The host checks this
    mapping at run time (the union of all fragment bounds must equal the imported mesh's bounds) and warns otherwise; the
    first guess, -100z, was mirrored in Y and the check caught it on all six types (2026-10-23)."""
    x, y, z = point
    return (100.0 * x, 100.0 * z, 100.0 * y)


def fragment_bounds(gltf, blob):
    """{fragment: [min x, min y, min z, max x, max y, max z]} in UE component space, from the vertices each fragment's
    triangles use (bind pose). The inventory preview frames a gun from the union of its shown fragments."""
    out = {}
    for primitive, material in zip(gltf['meshes'][0]['primitives'], gltf['materials']):
        accessor = gltf['accessors'][primitive['attributes']['POSITION']]
        view = gltf['bufferViews'][accessor['bufferView']]
        base = view.get('byteOffset', 0) + accessor.get('byteOffset', 0)
        stride = view.get('byteStride') or 12
        if accessor['componentType'] != 5126 or accessor['type'] != 'VEC3':
            raise ValueError('expected float VEC3 positions')
        index_accessor = gltf['accessors'][primitive['indices']]
        index_view = gltf['bufferViews'][index_accessor['bufferView']]
        start = index_view.get('byteOffset', 0) + index_accessor.get('byteOffset', 0)
        indices = set(struct.unpack_from(f'<{index_accessor["count"]}H', blob, start))
        points = [to_ue(struct.unpack_from('<3f', blob, base + stride * i)) for i in indices]
        lows, highs = [min(p[a] for p in points) for a in range(3)], [max(p[a] for p in points) for a in range(3)]
        out[material['name']] = [round(v, 3) for v in lows + highs]
    return out


def build_type(kind, gestalt_dir, gltf_dir, output):
    parts = gestalt_parts(gestalt_dir / f'{kind}.json')
    source = gltf_dir / f'GestaltDef_{kind}_GestaltSkeletalMesh.gltf'
    gltf = json.loads(source.read_text(encoding='utf-8'))
    blob = bytearray((source.parent / gltf['buffers'][0]['uri']).read_bytes())
    records = split(gltf, blob, parts)
    total = sum(p['NumPrimitives'] for p in parts)
    if sum(r['triangles'] for r in records) != total:
        raise ValueError(f'{kind}: split triangles differ from the table')
    folder = output / kind
    folder.mkdir(parents=True, exist_ok=True)
    stem = f'SK_Gestalt_{kind}'
    (folder / f'{stem}.bin').write_bytes(bytes(blob))
    gltf['buffers'][0] = {'uri': f'{stem}.bin', 'byteLength': len(blob)}
    (folder / f'{stem}.gltf').write_text(json.dumps(gltf), encoding='utf-8')
    joints = [gltf['nodes'][j].get('name') for j in gltf['skins'][0]['joints']]
    bounds = fragment_bounds(gltf, blob)
    for record in records:
        record['bounds'] = bounds[record['name']]
    table = {'type': kind, 'mesh': mesh_asset(kind), 'source_gltf': str(source), 'triangles': total,
             'fragments': records, 'bones': joints}
    (folder / 'fragments.json').write_text(json.dumps(table, indent=1), encoding='utf-8')
    return table


GESTALTS = {'Pistol': 'Weap_Pistol.GestaltDef_Pistol', 'SMG': 'Weap_SMG.GestaltDef_SMG',
            'AssaultRifle': 'Weap_AssaultRifles.GestaltDef_AssaultRifle', 'Shotgun': 'Weap_Shotguns.GestaltDef_Shotgun',
            'SniperRifle': 'Weap_SniperRifles.GestaltDef_SniperRifle', 'Launcher': 'Weap_Launchers.GestaltDef_Launcher'}


def skin_asset(leaf):
    return f'{UE_ROOT}/Skins/MI_{leaf}.MI_{leaf}'


def skin_assets(mics):
    """MIC path -> UE instance path; leaf names that occur twice get a numeric suffix (stable: sorted order)."""
    seen, out = {}, {}
    for mic in sorted(mics):
        leaf = mic.rsplit('.', 1)[-1]
        seen[leaf] = seen.get(leaf, 0) + 1
        out[mic] = skin_asset(leaf if seen[leaf] == 1 else f'{leaf}_{seen[leaf]}')
    return out


class SkinFacts:
    """Installed facts for one skin: the MIC's own package first (static parameters, textures), Startup.upk for the rest.

    Master_Gun and its parameter defaults live in Startup.upk; a DLC skin's MIC and textures live in its DLC package.
    """

    def __init__(self, reader, game):
        from prepare_weapon_paint import InstalledFacts
        self.reader, self.game, self.make = reader, game, InstalledFacts
        self.startup = InstalledFacts(reader, game / 'WillowGame/CookedPCConsole/Startup.upk')
        self.cache = {}

    def bind(self, package):
        if not package or Path(package).name.lower() == 'startup.upk':
            return self.startup
        if package not in self.cache:
            # A DLC Content folder has no Engine.upk beside it; the engine class defaults come from the base install.
            from prepare_weapon_paint import PackageReader
            own = self.make.__new__(self.make)
            own.package, own.engine, own.cache = PackageReader(self.reader, package), self.startup.engine, {}
            self.cache[package] = own
        own, startup = self.cache[package], self.startup

        class Bound:
            def base_defaults(self, name):
                return startup.base_defaults(name) or own.base_defaults(name)

            def static_channels(self, name):
                return own.static_channels(name) or startup.static_channels(name)

            def texture_address(self, name):
                found = own.texture_address(name)
                return found if found is not None else startup.texture_address(name)

            def texture_srgb(self, name):
                found = own.texture_srgb(name)
                return found if found is not None else startup.texture_srgb(name)

            def part_vectors(self, part):
                return {}
        return Bound()


def prepare_skins(census, inventory, roots, facts, output):
    """skins.json: per skin MIC the prepare_weapon_paint result (status prepared) or the reason it is not."""
    from prepare_weapon_paint import prepare_recipe
    assets = skin_assets(census['skins'])
    # tools/export_weapon_skins.py exports each skin under its own root (leaf names repeat across packages, and the
    # resolver finds MICs by leaf name), so a skin is prepared from its own root; `roots` is the fallback.
    listed = (inventory or {}).get('skins', [])
    exported = {entry['mic']: entry for entry in listed} if isinstance(listed, list) else listed
    packages = {p.name.lower(): p for p in list(facts.game.glob('WillowGame/CookedPCConsole/*.upk'))
                + list(facts.game.glob('DLC/*/Compat/Content/*.upk'))}
    result = {}
    for mic, skin in sorted(census['skins'].items()):
        entry = exported.get(mic) or {}
        package = packages.get(str(entry.get('package_used') or '').lower())
        skin_root = roots[0] / entry['root'] if entry.get('root') else roots[0]
        pseudo = {'material': mic, 'weapon_type': str(skin.get('weapon_class') or ''), 'parts': {}}
        try:
            data = prepare_recipe(pseudo, 'skin', skin_root, f'{UE_ROOT}/Unused', facts.bind(str(package) if package else None))
            data['asset'] = assets[mic]
            data.pop('mesh', None)
            result[mic] = {'status': 'prepared', 'asset': assets[mic], 'paint': data}
        except Exception as error:  # recorded per skin; the batch continues
            result[mic] = {'status': 'failed', 'asset': assets[mic], 'reason': f'{type(error).__name__}: {error}'}
    (output / 'skins.json').write_text(json.dumps(result, indent=1), encoding='utf-8')
    return result


def live_vectors(live_part):
    """{name: [r, g, b, a]} from a live census part's MaterialVectorParameterValues (weapon_visual_census.py shape)."""
    out = {}
    for entry in (live_part or {}).get('vectors') or []:
        value = entry.get('ParameterValue') or {}
        if entry.get('ParameterName') and all(c in value for c in 'RGBA'):
            out[str(entry['ParameterName'])] = [float(value[c]) for c in 'RGBA']
    return out


def build_index(census, output, live=None):
    """library.json parts / skins / weapon_types sections from the census (tools/weapon_part_census.py).

    With the live census (tools/real_game/scripts/weapon_visual_census.py), a part's vector parameters come from the running
    game where it has any: 22 Torgue material parts carry a p_EmissiveColor live that the cooked parts do not (2026-10-23;
    the documented live-versus-cooked data difference, REALGAME_GROUND_TRUTH.md). `vector_source` records which was used.
    """
    live_parts = {k.lower(): v for k, v in ((live or {}).get('parts') or {}).items()}
    index = json.loads((output / 'library.json').read_text(encoding='utf-8')) if (output / 'library.json').exists() else {}
    live_gestalts = (live or {}).get('gestalts') or {}
    for kind, entry in index.get('types', {}).items():
        entry['gestalt'] = GESTALTS.get(kind)
        # Material per gestalt section, read live (2026-10-23): section 0 is None (the gun's own instance goes there); the
        # pistol gestalt's section 1 holds its mesh material (a Dahl skin), which the game leaves on that slot for every gun.
        materials = (live_gestalts.get(GESTALTS.get(kind)) or {}).get('mesh_materials')
        if materials is not None:
            entry['section_materials'] = materials
        else:
            entry.pop('section_materials', None)
            print(f'warning: no live section materials for {kind}; guns drawing a fragment from another section will use '
                  f'their pre-built mesh (pass --live)')
        table = output / kind / 'fragments.json'
        if table.exists():
            records = json.loads(table.read_text(encoding='utf-8'))['fragments']
            entry['fragment_sections'] = {r['name']: r['source_sections'][0] for r in records if r.get('source_sections')}
    parts = {}
    for path, part in census['parts'].items():
        record = {'fragments': part.get('fragments') or [], 'slot': part.get('slot')}
        if part.get('gestalt_type') in GESTALTS:
            record['gestalt'] = part['gestalt_type']
        if part.get('material'):
            record['skin'] = part['material']
        found = live_vectors(live_parts.get(path.lower()))
        if found:
            record['vectors'], record['vector_source'] = found, 'live'
        elif part.get('vector_params'):
            record['vectors'], record['vector_source'] = part['vector_params'], 'cooked'
        parts[path] = record
    index['parts'] = parts
    skins_file = output / 'skins.json'
    prepared = json.loads(skins_file.read_text(encoding='utf-8')) if skins_file.exists() else {}
    report_file = output / 'import_report.json'
    imported = json.loads(report_file.read_text(encoding='utf-8')).get('skins', {}) if report_file.exists() else {}
    assets = skin_assets(census['skins'])
    index['skins'] = {mic: {'asset': assets[mic], 'prepared': (prepared.get(mic) or {}).get('status'),
                            'imported': (imported.get(mic) or {}).get('status')} for mic in census['skins']}
    index['weapon_types'] = {path: {'bones_to_hide': [b for b in (t.get('bones_to_hide') or []) if b and b != 'None']}
                             for path, t in census.get('weapon_types', {}).items()}
    (output / 'library.json').write_text(json.dumps(index, indent=1), encoding='utf-8')
    return index


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest='command', required=True)
    meshes = sub.add_parser('meshes', help='split every gestalt glTF into one slot per fragment')
    meshes.add_argument('--gestalt', type=Path, default=ROOT / 'local/gestalt')
    meshes.add_argument('--gltf', type=Path, default=ROOT / 'local/external/umodel/gestalt/Startup/SkeletalMesh3')
    meshes.add_argument('--types', nargs='*', default=TYPES)
    skins = sub.add_parser('skins', help='prepare every census skin through prepare_weapon_paint (skins.json)')
    skins.add_argument('--materials', type=Path, nargs='+', default=[ROOT / 'local/external/umodel/weapon-skins'])
    skins.add_argument('--inventory', type=Path, default=ROOT / 'local/external/umodel/weapon-skins/inventory.json')
    skins.add_argument('--reader', type=Path, default=ROOT / 'build/Release/ow-package.exe')
    skins.add_argument('--game', type=Path, required=True)
    index_parser = sub.add_parser('index', help='write the parts, skins and weapon_types sections of library.json')
    index_parser.add_argument('--live', type=Path, default=ROOT / 'local/realgame/wpa/live_visual_census.json')
    for command in (meshes, skins, sub.choices['index']):
        command.add_argument('--output', type=Path, default=ROOT / 'local/weapon_library')
        if command is not meshes:
            command.add_argument('--census', type=Path, default=ROOT / 'local/wpa/census/visual_census.json')
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to((ROOT / 'local').resolve()):
        parser.error('output must stay under local/')
    args.output.mkdir(parents=True, exist_ok=True)
    if args.command == 'skins':
        census = json.loads(args.census.read_text(encoding='utf-8'))
        inventory = json.loads(args.inventory.read_text(encoding='utf-8')) if args.inventory.exists() else None
        result = prepare_skins(census, inventory, args.materials, SkinFacts(args.reader, args.game), args.output)
        counts = {}
        for entry in result.values():
            counts[entry['status']] = counts.get(entry['status'], 0) + 1
        print(f'skins: {counts}')
        for mic, entry in result.items():
            if entry['status'] != 'prepared':
                print(f"  {mic}: {entry['reason']}")
        return
    if args.command == 'index':
        live = json.loads(args.live.read_text(encoding='utf-8')) if args.live and args.live.exists() else None
        index = build_index(json.loads(args.census.read_text(encoding='utf-8')), args.output, live)
        print(f"library.json: {len(index.get('types', {}))} types, {len(index['parts'])} parts, {len(index['skins'])} skins, "
              f"{sum(1 for p in index['parts'].values() if p.get('vector_source') == 'live')} parts with live vectors, "
              f"{sum(1 for t in index['weapon_types'].values() if t['bones_to_hide'])} weapon types hiding bones")
        return
    index = {}
    for kind in args.types:
        table = build_type(kind, args.gestalt, args.gltf, args.output)
        index[kind] = {'mesh': table['mesh'], 'gestalt': GESTALTS.get(kind), 'fragments': len(table['fragments']),
                       'triangles': table['triangles'],
                       'fragment_bounds': {r['name']: r['bounds'] for r in table['fragments']}}
        print(f"{kind}: {len(table['fragments'])} fragment slots, {table['triangles']} triangles -> {table['mesh']}")
    path = args.output / 'library.json'
    known = json.loads(path.read_text(encoding='utf-8')) if path.exists() else {}
    known.setdefault('types', {}).update(index)
    path.write_text(json.dumps(known, indent=1), encoding='utf-8')


if __name__ == '__main__':
    main()
