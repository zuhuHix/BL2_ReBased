"""Visual census of Borderlands 2 weapon parts, gestalt fragments and weapon skins.

For every WeaponPartDefinition in the installed packages: which gestalt mesh fragments
the part draws, which weapon gestalt those fragments belong to, what slot it fills and
which material instance (skin) it applies. Also each weapon gestalt's fragment table
(triangles and index ranges per fragment), the WeaponTypeDefinition fields that hide
bones or name a gestalt, and a manufacturer x gestalt matrix.

Provenance: our own code. Game data is read with ow-package (--exports and
--properties-batch) through tools/weapon_recipe.py, and the gestalt tables come from
local/gestalt/ via tools/filter_gestalt_gltf.py. The output is game-derived: it is
written only under the ignored local/ tree and never into a tracked file.

Rules and their status:
- Fragments a part draws: weapon_recipe.part_fragments(). Confirmed on 6 guns in game
  (docs/verification/WEAPON_VISUALS.md); for every other part UNVERIFIED. The *_None
  rule (draws nothing) is part of the same UNVERIFIED beyond those guns.
- Gestalt of a part: the one weapon gestalt whose fragment set contains all of the
  part's fragments. When several gestalts hold them, the weapon class named in the part's
  path breaks the tie among those candidates (gestalt_source 'fragments+path'). Both are
  heuristics, UNVERIFIED as game rules.
- Slot: PartType, else a set Material, else the path's second component, else a slot
  name in the leaf. Heuristic, UNVERIFIED.
- Skin weapon class: a weapon-class word in the MaterialInstanceConstant path, else in
  the path of a part that uses it. Heuristic.
- Skin manufacturers and rarity_tokens: name-derived, UNVERIFIED. Material parts carry no
  manufacturer field, so the maker comes from the part leaf Mat_<Maker>_<N> (N is the rarity
  index) and from the MIC leaf Mati_<Maker><Rarity><Type>.
- Matrix manufacturer: the body fragment's name (SMG_Body_Hyperion -> Hyperion), else the
  part's ManufacturerDefinitionForScopeMovie leaf, else Unknown. A skin with name-derived
  makers is counted under those instead. Heuristic.
- Skin resolvable: a MaterialInstanceConstant with that leaf name exists under --materials
  (UModel export roots). It says nothing about whether the material renders correctly.
- --live: compares this census with the live SDK census (tools/real_game/scripts/
  weapon_visual_census.py). It checks the gestalt LOD0 tables, part fragments, materials and
  vector parameters, skin sets with their live parent chains, and weapon bones to hide.
  Object paths are matched case-insensitively because live and cooked paths can differ in case.

Usage:
  python tools/weapon_part_census.py --game "<BL2 dir>" [--reader build/Release/ow-package.exe]
      [--gestalt local/gestalt] [--materials <umodel export root> ...]
      [--live local/realgame/wpa/live_visual_census.json]
      --output local/wpa/census/visual_census.json
"""
import argparse
import collections
import itertools
import json
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
REPO = TOOLS.parent
sys.path.insert(0, str(TOOLS))
import weapon_recipe  # noqa: E402
from filter_gestalt_gltf import gestalt_parts  # noqa: E402
from render_weapon_previews import find_props  # noqa: E402

GESTALTS = {
    'Pistol': 'Weap_Pistol.GestaltDef_Pistol',
    'SMG': 'Weap_SMG.GestaltDef_SMG',
    'AssaultRifle': 'Weap_AssaultRifles.GestaltDef_AssaultRifle',
    'Shotgun': 'Weap_Shotguns.GestaltDef_Shotgun',
    'SniperRifle': 'Weap_SniperRifles.GestaltDef_SniperRifle',
    'Launcher': 'Weap_Launchers.GestaltDef_Launcher',
}
WEAPON_CLASS_WORDS = [('Pistol', 'pistol'), ('SMG', 'smg'), ('AssaultRifle', 'assaultrifle'),
                      ('Shotgun', 'shotgun'), ('SniperRifle', 'sniper'), ('Launcher', 'launcher')]
WANTED_CLASSES = {'WeaponPartDefinition', 'WeaponTypeDefinition', 'GestaltSkeletalMeshDefinition'}
SCHEMA_LINES = weapon_recipe.SCHEMA_LINES + ['MaterialVectorParameterValues=StructProperty:VectorParameterValue']
PART_TYPE_SLOTS = {'WP_Body': 'Body', 'WP_Grip': 'Grip', 'WP_Barrel': 'Barrel', 'WP_Sight': 'Sight',
                   'WP_Stock': 'Stock', 'WP_Elemental': 'Elemental', 'WP_Accessory': 'Accessory',
                   'WP_Material': 'Material'}
PATH_SLOTS = {'body': 'Body', 'grip': 'Grip', 'barrel': 'Barrel', 'sight': 'Sight', 'stock': 'Stock',
              'elemental': 'Elemental', 'accessory': 'Accessory', 'manufacturermaterials': 'Material'}
LEAF_SLOTS = ['Body', 'Grip', 'Barrel', 'Sight', 'Stock', 'Elemental', 'Accessory']
BODY_MANUFACTURER = re.compile(r'^[A-Za-z]+_Body_([A-Za-z]+)')
GESTALT_NAME = re.compile(r'GestaltDef_\w+$')
# Name parsing for skins (UNVERIFIED: read from object names, not from game fields).
RARITY_WORDS = ['Uncommon', 'Common', 'Rare', 'Epic', 'Legendary', 'Unique', 'Effervescent', 'Emerald',
                'Diamond', 'Quartz', 'Raid', 'Pearlescent', 'Seraphim']
MATERIAL_LEAF = re.compile(r'^Mat_([A-Za-z]+?)_(\d+)(?=_|$)')
MIC_LEAF = re.compile(r'^(?:MasterMati|Mati)_([A-Z][a-z]+)_?(' + '|'.join(RARITY_WORDS) + r')?')
NAME_DERIVED = {
    'skins.manufacturers': 'UNVERIFIED: maker parsed from the Mat_ part leaf and the Mati_ MIC leaf',
    'skins.rarity_tokens': 'UNVERIFIED: index:N from the Mat_ part leaf, rarity word from the MIC leaf',
    'matrix.skins': 'UNVERIFIED: a skin counts under its name-derived makers when it has any',
    'parts.gestalt_source fragments+path': 'UNVERIFIED: tie broken by the weapon class named in the path',
}
CASE_NOTE = 'Object paths are compared case-insensitively; live and cooked paths can differ in case.'


def leaf(path):
    return path.rsplit('.', 1)[-1] if path else None


def strings(value):
    """Every non-'None' string in a decoded value (a string, or a list of them)."""
    if isinstance(value, str):
        return [] if value == 'None' else [value]
    if isinstance(value, list):
        return [s for item in value for s in strings(item)]
    return []


def slot_for(path, part_type, material):
    """(slot, source): PartType, then a set Material, then the path's second component, then the leaf name."""
    if part_type in PART_TYPE_SLOTS:
        return PART_TYPE_SLOTS[part_type], 'PartType'
    if material:
        return 'Material', 'Material'
    components = path.split('.')
    if len(components) > 1 and components[1].lower() in PATH_SLOTS:
        return PATH_SLOTS[components[1].lower()], 'path'
    name = leaf(path).lower()
    for slot in LEAF_SLOTS:
        if slot.lower() in name:
            return slot, 'leaf_name'
    return 'Unknown', 'none'


def weapon_class_for(path):
    """Weapon class from a name word (SMG in Common_GunMaterials.Materials.SMG.Mati_X), or None.

    Sniper skins sit under a 'sniper' folder (Aster_GunMaterials.Materials.sniper), so that
    word is accepted too.
    """
    words = [word.lower() for word in re.split(r'[._]', path or '')]
    for weapon_class, word in WEAPON_CLASS_WORDS:
        if any(w.startswith(word) for w in words):
            return weapon_class
    return None


def path_classes(path):
    """Weapon classes named by the words of a path (package, group and leaf tokens, case-insensitive).

    A whole 'AR' word means AssaultRifle (AR_Accessory_Bayonet_1); the other classes match a word prefix,
    so 'Pistols' and 'SniperRifles' count.
    """
    found = set()
    for word in re.split(r'[._]', path or ''):
        word = word.lower()
        if word == 'ar':
            found.add('AssaultRifle')
        found.update(weapon_class for weapon_class, prefix in WEAPON_CLASS_WORDS if word.startswith(prefix))
    return found


def gestalt_for(fragments, fragment_sets, path=None):
    """(Type, gestalt_source) of a part from its fragments.

    The one weapon gestalt holding all of `fragments` decides ('fragments'). When several hold them, the
    weapon class named in the part's path decides if exactly one candidate matches ('fragments+path').
    Otherwise 'ambiguous' with source None; a set no gestalt holds is 'unresolved'; no fragments gives (None, None).
    """
    if not fragments:
        return None, None
    hits = [weapon_type for weapon_type, names in fragment_sets.items() if set(fragments) <= names]
    if len(hits) == 1:
        return hits[0], 'fragments'
    if not hits:
        return 'unresolved', None
    named = path_classes(path) & set(hits)
    if len(named) == 1:
        return named.pop(), 'fragments+path'
    return 'ambiguous', None


def unresolved_fragments(fragments, fragment_sets):
    """Fragments that no weapon gestalt contains."""
    known = set().union(*fragment_sets.values())
    return [fragment for fragment in fragments if fragment not in known]


def resolve_parts(parts, fragment_sets):
    """Fill gestalt_type, gestalt_source and unresolved_fragments of every part record in place."""
    for path, part in parts.items():
        if part['decode_error']:
            part['gestalt_type'] = 'decode_error'
            part['gestalt_source'] = None
            continue
        part['gestalt_type'], part['gestalt_source'] = gestalt_for(part['fragments'], fragment_sets, path)
        part['unresolved_fragments'] = unresolved_fragments(part['fragments'], fragment_sets)


def gestalt_report(rows):
    """Fragment table from gestalt_parts() rows; a name listed twice sums its triangles and keeps both ranges."""
    fragments = {}
    for row in rows:
        entry = fragments.setdefault(row['SkeletalMeshFragmentName'], {'triangles': 0, 'ranges': []})
        entry['triangles'] += row['NumPrimitives']
        entry['ranges'].append({'material': row['MaterialIndex'], 'first_index': row['FirstIndex'],
                                'triangles': row['NumPrimitives']})
    return fragments


def gestalt_entries(tables, parts):
    """{Type: {path, fragments, triangles_total, referenced_by_parts, unreferenced_fragments}}."""
    counts = collections.Counter(f for part in parts.values() for f in part['fragments'])
    entries = {}
    for weapon_type, (path, rows) in tables.items():
        fragments = gestalt_report(rows)
        entries[weapon_type] = {
            'path': path,
            'fragments': fragments,
            'triangles_total': sum(row['NumPrimitives'] for row in rows),
            'referenced_by_parts': {f: counts[f] for f in fragments},
            'unreferenced_fragments': [f for f in fragments if not counts[f]],
        }
    return entries


def vector_params(entries):
    """MaterialVectorParameterValues -> {ParameterName: [R, G, B, A]}."""
    found = {}
    for entry in entries if isinstance(entries, list) else []:
        value = entry.get('ParameterValue') or {}
        found[entry.get('ParameterName')] = [value.get(k) for k in 'RGBA']
    return found


def decode_part(package, path):
    """Properties one part needs; an error is recorded in decode_error and never raised."""
    decoded = {'part_type': None, 'material': None, 'vector_params': {}, 'first_person_particle': False,
               'manufacturer': None, 'fragments': [], 'decode_error': None}
    try:
        props = package.props(path)
        decoded.update(
            part_type=props.get('PartType'),
            material=props.get('Material'),
            vector_params=vector_params(props.get('MaterialVectorParameterValues')),
            first_person_particle=bool(props.get('FirstPersonParticle')),
            manufacturer=leaf(props.get('ManufacturerDefinitionForScopeMovie')),
            fragments=weapon_recipe.part_fragments(package, path),
        )
    except Exception as error:  # one bad export must not stop the census
        decoded['decode_error'] = f'{type(error).__name__}: {error}'[:300]
    return decoded


def part_entry(path, package, packages, decoded):
    slot, source = slot_for(path, decoded['part_type'], decoded['material'])
    return {
        'package': package,
        'packages': packages,
        'slot': slot,
        'slot_source': source,
        'gestalt_type': None,
        'gestalt_source': None,
        'unresolved_fragments': [],
        'draws_nothing_rule': leaf(path).lower().endswith('_none'),
        'fragments': decoded['fragments'],
        'material': decoded['material'],
        'vector_params': decoded['vector_params'],
        'first_person_particle': decoded['first_person_particle'],
        'manufacturer': decoded['manufacturer'],
        'decode_error': decoded['decode_error'],
    }


def weapon_type_record(package, path):
    """Bones hidden on the mesh and the gestalt the type names (the first value ending GestaltDef_<X>)."""
    record = {'bones_to_hide': [], 'gestalt_field': None, 'decode_error': None}
    try:
        props = package.props(path)
        for key in ('BoneToHideOnMesh', 'AdditionalBoneToHideOnMesh'):
            record['bones_to_hide'] += strings(props.get(key))
        record['gestalt_field'] = next(({'field': key, 'value': value}
                                        for key, raw in props.items() for value in strings(raw)
                                        if GESTALT_NAME.search(value)), None)
    except Exception as error:
        record['decode_error'] = f'{type(error).__name__}: {error}'[:300]
    return record


def package_rank(path, game):
    """Startup first, then base packages, then DLC; each group by name."""
    group = 0 if path.name == 'Startup.upk' else 2 if 'DLC' in path.relative_to(game).parts else 1
    return group, path.name


def package_paths(game):
    found = sorted(game.glob('WillowGame/CookedPCConsole/*.upk')) + sorted(game.glob('DLC/*/Compat/Content/*.upk'))
    return sorted(dict.fromkeys(found), key=lambda path: package_rank(path, game))


def scan_one(reader, package):
    result = subprocess.run([reader, str(package), '--exports'], capture_output=True, text=True, timeout=300)
    if result.returncode:
        return None, result.stderr.strip()[:200]
    hits = []
    for export in json.loads(result.stdout):
        cls = export['class'].split('.')[-1]
        if cls in WANTED_CLASSES:
            hits.append({'path': export['path'], 'class': cls})
    return hits, None


def scan_packages(game, reader):
    """([(package, hits)] in package order for packages holding a wanted export, [failed packages])."""
    packages = package_paths(game)
    with ThreadPoolExecutor(8) as pool:
        results = list(pool.map(lambda package: scan_one(reader, package), packages))
    scanned, failed = [], []
    for package, (hits, error) in zip(packages, results):
        if error:
            failed.append({'package': package.name, 'error': error})
        elif hits:
            scanned.append((package, hits))
    return scanned, failed


def decode_objects(scanned, owners, reader, schema):
    """Decode each part and weapon type once, from the first package (in scan order) that exports it."""
    parts, types, failed = {}, {}, []
    for package_path, hits in scanned:
        new = [h for h in hits if h['path'] not in parts and h['path'] not in types
               and not leaf(h['path']).startswith('Default__')]
        if not new:
            continue
        try:
            package = weapon_recipe.Package(reader, package_path, schema)
            package.preload([h['path'] for h in new])
        except Exception as error:
            failed.append({'package': package_path.name, 'error': str(error)[:200]})
            continue
        for hit in new:
            path = hit['path']
            if hit['class'] == 'WeaponPartDefinition':
                parts[path] = part_entry(path, package_path.name, owners[path],
                                         decode_part(package, path))
            elif hit['class'] == 'WeaponTypeDefinition':
                types[path] = weapon_type_record(package, path)
    return parts, types, failed


def maker_name(token):
    """'BanditMade' -> 'Bandit': a 'Made' suffix on a material-maker token names the same maker."""
    return token[:-len('Made')] if token.endswith('Made') and len(token) > len('Made') else token


def material_name(name):
    """(maker, rarity index) from a Mat_<Maker>_<N>[_suffix] part leaf; (None, None) when it has no such form."""
    match = MATERIAL_LEAF.match(name or '')
    return (maker_name(match.group(1)), match.group(2)) if match else (None, None)


def mic_name(name):
    """(maker, rarity word) from a Mati_<Maker><Rarity><Type> or MasterMati_ leaf; (None, None) otherwise."""
    match = MIC_LEAF.match(name or '')
    return (match.group(1), match.group(2)) if match else (None, None)


def build_skins(parts, roots):
    """{MIC path: {used_by_parts, weapon_class, resolvable, manufacturers, rarity_tokens}}.

    resolvable is None when no --materials roots were given. manufacturers and rarity_tokens are name-derived
    (see NAME_DERIVED): makers and rarity from the using parts' leaves and the MIC leaf; index:N for a part's index.
    """
    skins = {}
    for path, part in parts.items():
        if part['material']:
            skins.setdefault(part['material'], {'used_by_parts': []})['used_by_parts'].append(path)
    for mic, skin in skins.items():
        weapon_class = weapon_class_for(mic)
        makers, rarity = set(), set()
        for path in skin['used_by_parts']:
            weapon_class = weapon_class or weapon_class_for(path)
            maker, index = material_name(leaf(path))
            if maker:
                makers.add(maker)
            if index:
                rarity.add(f'index:{index}')
        mic_maker, mic_rarity = mic_name(leaf(mic))
        if mic_maker:
            makers.add(mic_maker)
        if mic_rarity:
            rarity.add(mic_rarity)
        skin['weapon_class'] = weapon_class
        skin['resolvable'] = None if roots is None else find_props(leaf(mic), roots) is not None
        skin['manufacturers'] = sorted(makers)
        skin['rarity_tokens'] = sorted(rarity)
    return skins


def matrix_manufacturer(part):
    for fragment in part['fragments']:
        match = BODY_MANUFACTURER.match(fragment)
        if match:
            return match.group(1)
    return part['manufacturer'] or 'Unknown'


def build_matrix(parts, skins):
    """{gestalt Type: {manufacturer: {body_parts, skins}}}; skins are counted once per manufacturer."""
    matrix = {}

    def cell(weapon_type, manufacturer):
        return matrix.setdefault(weapon_type, {}).setdefault(manufacturer, {'body_parts': 0, 'skins': 0})

    for part in parts.values():
        if part['gestalt_type'] in GESTALTS and part['slot'] == 'Body':
            cell(part['gestalt_type'], matrix_manufacturer(part))['body_parts'] += 1
    for skin in skins.values():
        if skin['weapon_class'] in GESTALTS:
            makers = set(skin['manufacturers']) or {matrix_manufacturer(parts[p]) for p in skin['used_by_parts']}
            for manufacturer in makers:
                cell(skin['weapon_class'], manufacturer)['skins'] += 1
    return matrix


def build_summary(parts, skins, gestalts, decode_errors, package_errors):
    checked = [skin['resolvable'] for skin in skins.values() if skin['resolvable'] is not None]
    return {
        'parts': len(parts),
        'parts_per_slot': dict(sorted(collections.Counter(p['slot'] for p in parts.values()).items())),
        'parts_per_gestalt_type': dict(sorted(collections.Counter(
            p['gestalt_type'] or 'none' for p in parts.values()).items())),
        'parts_per_gestalt_source': dict(sorted(collections.Counter(
            p.get('gestalt_source') or 'none' for p in parts.values()).items())),
        'parts_with_unresolved_fragments': sorted(p for p, r in parts.items() if r['unresolved_fragments']),
        'fragments_never_referenced': {t: g['unreferenced_fragments'] for t, g in gestalts.items()
                                       if g['unreferenced_fragments']},
        'skins': len(skins),
        'skins_per_weapon_class': dict(sorted(collections.Counter(
            s['weapon_class'] or 'unknown' for s in skins.values()).items())),
        # name-derived (UNVERIFIED); a skin with makers from two manufacturers counts under both
        'skins_per_manufacturer': dict(sorted(collections.Counter(
            m for s in skins.values() for m in (s.get('manufacturers') or ['Unknown'])).items())),
        'skins_resolvable': sum(checked) if checked else None,
        'skins_unresolvable': len(checked) - sum(checked) if checked else None,
        'decode_errors': decode_errors,
        'package_errors': package_errors,
    }


def canonical(value):
    """Lower-case path or name for matching; None for a missing value or the string 'None'."""
    return None if not value or value == 'None' else value.lower()


def first_difference(cooked, live):
    """{index, cooked, live} at the first position where two lists differ (a short list pads with None), or None."""
    for index, (a, b) in enumerate(itertools.zip_longest(cooked, live)):
        if a != b:
            return {'index': index, 'cooked': a, 'live': b}
    return None


def live_fragments(path, live_part):
    """A live part's fragments by part_fragments()'s rule: the gestalt name, then the non-'None' additional names."""
    if leaf(path).lower().endswith('_none'):
        return []
    return [name for name in [live_part['gestalt']] + live_part['additional'] if name and name != 'None']


def vectors_differing(cooked, live, tolerance=1e-4):
    """Parameter names that only one side has, or whose RGBA channels differ by more than tolerance."""
    def close(a, b):
        return (a == b) if a is None or b is None else abs(a - b) <= tolerance

    def same(name):
        return name in cooked and name in live and len(cooked[name]) == len(live[name]) and all(
            close(a, b) for a, b in zip(cooked[name], live[name]))

    return sorted({name for name in list(cooked) + list(live) if not same(name)}, key=str)


def compare_gestalts(live_gestalts, tables):
    """Per weapon gestalt: cooked LOD0 table (fragment, material, first index, triangles) against the live LOD0 table."""
    by_path = {canonical(path): record for path, record in live_gestalts.items()}
    compared = {}
    for weapon_type, (path, rows) in tables.items():
        cooked = [(r['SkeletalMeshFragmentName'], int(r['MaterialIndex']), int(r['FirstIndex']),
                   int(r['NumPrimitives'])) for r in rows]
        record = by_path.get(canonical(path))
        live = [] if record is None or not record['lods'] else [
            (p['name'], int(p['material']), int(p['first_index']), int(p['triangles'])) for p in record['lods'][0]]
        compared[weapon_type] = {'path': path, 'live_found': record is not None, 'rows': len(cooked),
                                 'live_rows': len(live), 'equal': record is not None and cooked == live,
                                 'first_difference': first_difference(cooked, live)}
    return compared


def live_compare(live, tables, parts, skins, types):
    """Cooked census against the live SDK census. Only paths present in both are compared field by field."""
    report = {'note': CASE_NOTE, 'mismatches': []}
    report['gestalts'] = compare_gestalts(live['gestalts'], tables)
    for weapon_type, entry in report['gestalts'].items():
        if not entry['equal']:
            report['mismatches'].append({'kind': 'gestalt_table', 'path': entry['path'],
                                         'detail': entry['first_difference'] or f'live found: {entry["live_found"]}'})

    live_parts = {canonical(path): (path, record) for path, record in live['parts'].items()}
    cooked_keys = {canonical(path) for path in parts}
    both = [path for path in parts if canonical(path) in live_parts]
    counts = collections.Counter()
    for path in both:
        cooked, live_part = parts[path], live_parts[canonical(path)][1]
        live_frags = live_fragments(path, live_part)
        if cooked['fragments'] == live_frags:
            counts['fragments_equal'] += 1
        else:
            counts['fragments_differ'] += 1
            report['mismatches'].append({'kind': 'fragments', 'path': path, 'cooked': cooked['fragments'],
                                         'live': live_frags})
        if canonical(cooked['material']) == canonical(live_part['material']):
            counts['material_equal'] += 1
        else:
            counts['material_differ'] += 1
            report['mismatches'].append({'kind': 'material', 'path': path, 'cooked': cooked['material'],
                                         'live': live_part['material']})
        differing = vectors_differing(cooked['vector_params'], vector_params(live_part['vectors']))
        if differing:
            counts['vectors_differ'] += 1
            report['mismatches'].append({'kind': 'vectors', 'path': path, 'names': differing})
        else:
            counts['vectors_equal'] += 1
    report['parts'] = {
        'in_both': len(both), 'cooked_only': len(parts) - len(both), 'live_only': len(live_parts) - len(both),
        'cooked_only_examples': [p for p in parts if canonical(p) not in live_parts][:5],
        'live_only_examples': sorted(p for k, (p, _) in live_parts.items() if k not in cooked_keys)[:5],
        **{key: counts[key] for key in ('fragments_equal', 'fragments_differ', 'material_equal', 'material_differ',
                                        'vectors_equal', 'vectors_differ')},
    }

    live_mics = {canonical(path): (path, chain) for path, chain in live['mics'].items()}
    skin_keys = {canonical(path) for path in skins}
    common = [path for path in skins if canonical(path) in live_mics]
    report['skins'] = {
        'in_both': len(common), 'cooked_only': len(skins) - len(common), 'live_only': len(live_mics) - len(common),
        'cooked_only_examples': [s for s in skins if canonical(s) not in live_mics][:5],
        'live_only_examples': sorted(p for k, (p, _) in live_mics.items() if k not in skin_keys)[:5],
        'parent_chains': {path: [link['path'] for link in live_mics[canonical(path)][1]] for path in common},
    }

    live_types = {canonical(path): (path, record) for path, record in live['weapon_types'].items()}
    common_types = [path for path in types if canonical(path) in live_types]
    bones_equal = 0
    for path in common_types:
        cooked_bones = types[path]['bones_to_hide']
        live_bones = [name for value in live_types[canonical(path)][1]['bones_to_hide'] for name in strings(value)]
        if cooked_bones == live_bones:
            bones_equal += 1
        else:
            report['mismatches'].append({'kind': 'bones_to_hide', 'path': path, 'cooked': cooked_bones,
                                         'live': live_bones})
    report['weapon_types'] = {
        'in_both': len(common_types), 'cooked_only': len(types) - len(common_types),
        'live_only': len(live_types) - len(common_types), 'bones_equal': bones_equal,
        'bones_differ': len(common_types) - bones_equal,
    }
    return report


def print_live_compare(report):
    gestalts = report['gestalts']
    parts, skins, types = report['parts'], report['skins'], report['weapon_types']
    print(f'live gestalt LOD0 tables equal {sum(g["equal"] for g in gestalts.values())}/{len(gestalts)}')
    print(f'live parts: both {parts["in_both"]}, cooked-only {parts["cooked_only"]}, '
          f'live-only {parts["live_only"]} (examples {parts["live_only_examples"][:2]})')
    print(f'  fragments equal {parts["fragments_equal"]} differ {parts["fragments_differ"]}; '
          f'material equal {parts["material_equal"]} differ {parts["material_differ"]}; '
          f'vectors equal {parts["vectors_equal"]} differ {parts["vectors_differ"]}')
    print(f'live skins: both {skins["in_both"]}, cooked-only {skins["cooked_only"]}, live-only {skins["live_only"]}')
    print(f'live weapon types: both {types["in_both"]}, cooked-only {types["cooked_only"]}, '
          f'live-only {types["live_only"]}; bones equal {types["bones_equal"]} differ {types["bones_differ"]}')
    print(f'live mismatches {len(report["mismatches"])} (first 10; {CASE_NOTE})')
    for mismatch in report['mismatches'][:10]:
        print('  ' + json.dumps(mismatch)[:400])


def output_under_local(output, local):
    """True when `output` is inside the local/ directory (game-derived output must stay there)."""
    return local in output.resolve().parents


def load_gestalt_tables(gestalt_dir):
    """{Type: (path, gestalt_parts rows)} from local/gestalt/<Type>.json."""
    return {weapon_type: (path, gestalt_parts(gestalt_dir / f'{weapon_type}.json'))
            for weapon_type, path in GESTALTS.items()}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--reader', default=str(REPO / 'build' / 'Release' / 'ow-package.exe'))
    parser.add_argument('--gestalt', default=str(REPO / 'local' / 'gestalt'))
    parser.add_argument('--materials', nargs='+', help='UModel export roots to check skins against')
    parser.add_argument('--live', help='live SDK census (weapon_visual_census.py output) to compare with')
    parser.add_argument('--output', required=True, help='JSON output; must be under local/')
    args = parser.parse_args()

    output = Path(args.output).resolve()
    if not output_under_local(output, (REPO / 'local').resolve()):
        parser.error('--output must be inside local/ (game-derived output is never tracked)')
    started = time.perf_counter()
    reader = str(Path(args.reader).resolve())
    schema = output.parent / 'weapon_part_census.schema'
    schema.parent.mkdir(parents=True, exist_ok=True)
    schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')

    tables = load_gestalt_tables(Path(args.gestalt))
    fragment_sets = {t: set(gestalt_report(rows)) for t, (_, rows) in tables.items()}
    scanned, package_errors = scan_packages(Path(args.game), reader)
    owners = {}
    for package_path, hits in scanned:
        for hit in hits:
            owners.setdefault(hit['path'], []).append(package_path.name)
    parts, types, failed = decode_objects(scanned, owners, reader, str(schema.resolve()))
    package_errors += failed
    resolve_parts(parts, fragment_sets)

    gestalts = gestalt_entries(tables, parts)
    not_weapon = sorted({h['path'] for _, hits in scanned for h in hits if h['class'] == 'GestaltSkeletalMeshDefinition'
                         and h['path'] not in GESTALTS.values() and not leaf(h['path']).startswith('Default__')})
    roots = [Path(root) for root in args.materials] if args.materials else None
    skins = build_skins(parts, roots)
    decode_errors = sum(1 for p in parts.values() if p['decode_error']) + sum(
        1 for t in types.values() if t['decode_error'])
    summary = build_summary(parts, skins, gestalts, decode_errors, len(package_errors))
    result = {
        'gestalts': gestalts,
        'not_weapon_gestalt': not_weapon,
        'parts': parts,
        'skins': skins,
        'weapon_types': types,
        'matrix': build_matrix(parts, skins),
        'package_errors': package_errors,
        'summary': summary,
        'unverified': NAME_DERIVED,
    }
    if args.live:
        result['live_compare'] = live_compare(json.loads(Path(args.live).read_text(encoding='utf-8')),
                                              tables, parts, skins, types)
        print_live_compare(result['live_compare'])
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=1), encoding='utf-8')

    print(f'parts {summary["parts"]}  slots {summary["parts_per_slot"]}')
    print(f'gestalt types {summary["parts_per_gestalt_type"]}')
    print(f'unresolved-fragment parts {len(summary["parts_with_unresolved_fragments"])}')
    print(f'skins {summary["skins"]}  classes {summary["skins_per_weapon_class"]}  '
          f'resolvable {summary["skins_resolvable"]}  unresolvable {summary["skins_unresolvable"]}')
    print(f'decode errors {decode_errors}  package errors {len(package_errors)}  not-weapon gestalts {len(not_weapon)}')
    print(f'wrote {output} in {time.perf_counter() - started:.1f} s')


if __name__ == '__main__':
    main()
