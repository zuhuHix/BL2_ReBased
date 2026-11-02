"""Weapon recipes for checking the visual weapon assembly: a sample of guns from installed data.

Three sample sets, all rolled from Startup.upk and written only under local/:
- matrix: every WeaponBalanceDefinition rolled once (tools/weapon_recipe.py, seed 1, game stage 30).
  One gun per (weapon type, manufacturer) among guns whose gestalt is a weapon gestalt. Inside a
  group a balance without a legendary/unique/etech marker in its path is preferred.
- unique: one gun per balance whose path names a legendary, unique, etech, pearl, seraph or mission.
- skins: for every census skin material that no matrix or unique gun uses: a copy of a matrix gun of
  the same gestalt class whose Material part is replaced by a part that uses the skin, whose material
  is the skin and whose stats are copied from the base gun. Only the visuals are meant to be checked.

Every gun gets stats from tools/weapon_stats.py (its evaluate(), the same call as its CLI), so the
host can read stats.card. A gun whose roll or stats fail is listed in sample_index.json and not
written. The choices above are this tool's sampling rules, not game rules; nothing here is verified
in game. Rolls are seeded and do not reproduce the game's random sequence (weapon_recipe.pick()).

Usage:
  python tools/weapon_assembly_sample.py --game "<BL2 dir>" [--reader build/Release/ow-package.exe]
      [--census local/wpa/census/visual_census.json] [--gestalt local/gestalt]
      [--output local/wpa/sample] [--sets matrix unique skins]
"""
import argparse
import copy
import json
import re
import sys
import time
from collections import Counter, OrderedDict
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
REPO = TOOLS.parent
sys.path.insert(0, str(TOOLS))
import filter_gestalt_gltf  # noqa: E402
import skill_stats  # noqa: E402
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402
import weapon_slice_gear  # noqa: E402

GAME_PACKAGE = Path('WillowGame') / 'CookedPCConsole' / 'Startup.upk'
BALANCE_CLASS = 'WillowGame.WeaponBalanceDefinition'
WEAPON_GESTALTS = ('Pistol', 'SMG', 'AssaultRifle', 'Shotgun', 'SniperRifle', 'Launcher')
LEGENDARY_MARKERS = ('Legendary', 'Unique', 'Etech')
UNIQUE_MARKERS = LEGENDARY_MARKERS + ('Pearl', 'Seraph', 'Mission')
SEED = 1
GAME_STAGE = 30
SETS = ('matrix', 'unique', 'skins')
SET_PREFIX = {'matrix': 'm', 'unique': 'u', 'skins': 's'}
SKIN_REASON = 'skin sweep: Material part replaced'
# Balances are decoded in batches of this many (each crawl is one reader process per level).
CRAWL_CHUNK = 20


def leaf(path):
    return path.rsplit('.', 1)[-1]


def gestalt_type(gestalt):
    """'Weap_Pistol.GestaltDef_Pistol' -> 'Pistol'; None without a gestalt."""
    return leaf(gestalt).removeprefix('GestaltDef_') if gestalt else None


def is_weapon_gun(recipe):
    return gestalt_type(recipe.get('gestalt')) in WEAPON_GESTALTS


def make_id(prefix, name):
    """('m', 'WT_Hyperion_SMG') -> 'm_wt_hyperion_smg' (sample ids are [a-z0-9_]+)."""
    slug = re.sub(r'[^a-z0-9]+', '_', name.lower()).strip('_')
    return f'{prefix}_{slug}'


def unique_id(prefix, name, taken):
    """make_id(), with _2, _3 ... appended while the id is already in `taken`."""
    base = make_id(prefix, name)
    candidate, number = base, 2
    while candidate in taken:
        candidate = f'{base}_{number}'
        number += 1
    return candidate


def prefer_non_legendary(recipes):
    """The first recipe whose balance path has no legendary/unique/etech marker, else the first."""
    for recipe in recipes:
        if not any(marker in recipe['balance'] for marker in LEGENDARY_MARKERS):
            return recipe
    return recipes[0]


def select_matrix(rolled):
    """OrderedDict id -> (recipe, reason): one gun per (weapon type, manufacturer), in roll order."""
    groups = OrderedDict()
    for recipe in rolled:
        if is_weapon_gun(recipe):
            groups.setdefault((recipe['weapon_type'], recipe.get('manufacturer')), []).append(recipe)
    picked = OrderedDict()
    for (weapon_type, manufacturer), recipes in groups.items():
        gid = unique_id(SET_PREFIX['matrix'], leaf(weapon_type), picked)
        reason = f'matrix: {leaf(weapon_type)} x {leaf(manufacturer or "none")}'
        picked[gid] = (prefer_non_legendary(recipes), reason)
    return picked


def select_unique(rolled):
    """(OrderedDict id -> (recipe, reason), skipped) for balances whose path names a unique/legendary/etc."""
    picked, skipped = OrderedDict(), []
    for recipe in rolled:
        balance = recipe['balance']
        if not any(marker in balance for marker in UNIQUE_MARKERS):
            continue
        if not is_weapon_gun(recipe):
            skipped.append({'balance': balance, 'reason': 'no weapon gestalt'})
            continue
        gid = unique_id(SET_PREFIX['unique'], leaf(balance), picked)
        picked[gid] = (recipe, f'unique: {balance}')
    return picked, skipped


def verified_material_part(info, census_parts, mic, material_of):
    """The first part in `info['used_by_parts']` that is a Material (or unclassified) part and whose Material is `mic`."""
    for part in info.get('used_by_parts') or []:
        if (census_parts.get(part) or {}).get('slot') not in (None, 'Material'):
            continue
        if material_of(part) == mic:
            return part
    return None


def choose_base(candidates, makers):
    """(id, recipe) of the candidate whose manufacturer the skin names, else the first candidate."""
    wanted = {maker.lower() for maker in makers}
    for gid, recipe in candidates:
        if leaf(recipe.get('manufacturer') or '').lower() in wanted:
            return gid, recipe
    return candidates[0]


def select_skins(skins, census_parts, used, bases, material_of):
    """(OrderedDict id -> (base id, base recipe, part, mic), skipped).

    `used` is the set of materials the matrix and unique guns already show; `bases` maps matrix
    ids to recipes. A skin with no Material part that uses it, or no matrix gun of its weapon
    class, is skipped and listed with its reason.
    """
    picked, skipped = OrderedDict(), []
    for mic in sorted(skins):
        if mic in used:
            continue
        info = skins[mic]
        part = verified_material_part(info, census_parts, mic, material_of)
        if part is None:
            skipped.append({'skin': mic, 'reason': 'no Material part uses it'})
            continue
        weapon_class = info.get('weapon_class')
        candidates = [(gid, recipe) for gid, recipe in bases.items() if gestalt_type(recipe.get('gestalt')) == weapon_class]
        if not candidates:
            skipped.append({'skin': mic, 'reason': f'no matrix gun of class {weapon_class}'})
            continue
        base_id, base = choose_base(candidates, info.get('manufacturers') or [])
        picked[unique_id(SET_PREFIX['skins'], leaf(mic), picked)] = (base_id, base, part, mic)
    return picked, skipped


def skin_recipe(base_id, base, part, mic, fragments_of):
    """A copy of `base` whose Material part is `part`, whose material is `mic`, with its stats copied as is."""
    recipe = copy.deepcopy(base)
    recipe['parts']['Material'] = {'part': part, 'weight': None, 'candidates': []}
    fragments = {slot: fragments_of(choice['part']) for slot, choice in recipe['parts'].items()}
    recipe['gestalt_fragments'] = sorted({name for names in fragments.values() for name in names})
    recipe['material'] = mic
    recipe['stats_source'] = f'copied from {base_id}'
    return recipe


def expected_triangles(fragments, rows):
    """(triangles, fragments with no row): NumPrimitives summed over every gestalt row naming each fragment.

    `rows` is filter_gestalt_gltf.gestalt_parts() output: dicts with SkeletalMeshFragmentName and NumPrimitives.
    """
    total, missing = 0, []
    for name in fragments:
        counts = [row['NumPrimitives'] for row in rows if row['SkeletalMeshFragmentName'] == name]
        if not counts:
            missing.append(name)
        total += sum(counts)
    return total, missing


def index_entry(set_name, recipe, rows):
    """One sample_index.json record. `rows` is the gestalt table of the gun's weapon gestalt type."""
    triangles, missing = expected_triangles(recipe.get('gestalt_fragments') or [], rows)
    return {'set': set_name, 'weapon_type': recipe['weapon_type'], 'gestalt': recipe.get('gestalt'),
            'manufacturer': recipe.get('manufacturer'), 'material': recipe.get('material'),
            'fragments': recipe.get('gestalt_fragments') or [], 'expected_triangles': triangles,
            'unknown_fragments': missing}


def totals(index, possible_pairs, skins, covered_materials, roll_failures, stats_failures):
    """Counts for sample_index.json: guns per set, gestalt type, manufacturer; matrix coverage; skins."""
    present = {(gestalt_type(e['gestalt']), leaf(e['manufacturer'] or 'none'))
               for e in index.values() if e['set'] == 'matrix'}
    matrix = {}
    for gtype, maker in sorted(present):
        matrix.setdefault(gtype, {})[maker] = 1
    return {
        'guns_per_set': dict(Counter(e['set'] for e in index.values())),
        'guns_total': len(index),
        'guns_per_gestalt_type': dict(Counter(gestalt_type(e['gestalt']) for e in index.values())),
        'guns_per_manufacturer': dict(Counter(leaf(e['manufacturer'] or 'none') for e in index.values())),
        'matrix': matrix,
        'matrix_coverage': {'present': len(present & possible_pairs), 'possible': len(possible_pairs),
                            'missing': sorted(f'{g} x {m}' for g, m in possible_pairs - present)},
        'skins': {'covered': len(set(skins) & covered_materials), 'total': len(skins)},
        'roll_failures': len(roll_failures),
        'stats_failures': len(stats_failures),
    }


def output_under_local(output, local):
    """True when `output` is inside the local/ directory (game-derived output must stay there)."""
    return local in output.resolve().parents


def make_localize(game, package):
    """The localize(path, default, property) callback weapon_stats.evaluate() takes (<Package>.int overrides)."""
    files = {}

    def localize(path, default, property_name='NoConstraintText'):
        name = path.split('.', 1)[0]
        if name not in files:
            files[name] = skill_stats.localization_for(game, name)
        return files[name].get((skill_stats.localization_key(path, package.classes), property_name), default)
    return localize


def material_lookup(package):
    def material_of(part):
        try:
            return package.props(part).get('Material')
        except KeyError:
            return None
    return material_of


def run(args):
    started = time.time()
    output = Path(args.output).resolve()
    if not output_under_local(output, (REPO / 'local').resolve()):
        raise SystemExit(f'refusing to write {output}: output must be inside {REPO / "local"}')
    output.mkdir(parents=True, exist_ok=True)
    schema = output.parent / 'weapon_assembly_sample.schema'
    schema.write_text('\n'.join(weapon_stats.SCHEMA_LINES) + '\n', encoding='utf-8')
    game = Path(args.game).resolve()
    package = skill_stats.Package(str(Path(args.reader).resolve()), game / GAME_PACKAGE, str(schema.resolve()))
    balances = [path for path, cls in package.classes.items() if cls == BALANCE_CLASS]
    for start in range(0, len(balances), CRAWL_CHUNK):
        package.crawl(balances[start:start + CRAWL_CHUNK])
    print(f'balances: {len(balances)}; decoded in {time.time() - started:.0f}s', flush=True)

    rolled, roll_failures = [], []
    for balance in balances:
        try:
            recipe = weapon_recipe.roll(package, balance, SEED, GAME_STAGE)
            # The host's item type (arms clip set, ammo) is the card label, as tools/weapon_slice_gear.py writes it.
            recipe['type'] = weapon_slice_gear.card_type(package, recipe.get('weapon_type'))
            rolled.append(recipe)
        except Exception as error:  # one bad balance must not stop the sample; it is recorded
            roll_failures.append({'balance': balance, 'error': f'{type(error).__name__}: {error}'})
    print(f'rolled: {len(rolled)}; failures: {len(roll_failures)}', flush=True)

    matrix = select_matrix(rolled)
    unique, unique_skipped = select_unique(rolled)
    localize = make_localize(game, package)
    stats_failures = []

    def with_stats(picked):
        kept = OrderedDict()
        for gid, (recipe, reason) in picked.items():
            try:
                recipe['stats'] = weapon_stats.evaluate(package, recipe, int(recipe.get('game_stage') or 1), localize)
            except Exception as error:  # recorded, and the gun is left out
                stats_failures.append({'id': gid, 'balance': recipe['balance'], 'error': f'{type(error).__name__}: {error}'})
                continue
            kept[gid] = (recipe, reason)
        return kept

    matrix, unique = with_stats(matrix), with_stats(unique)
    guns = OrderedDict()
    for set_name, picked in (('matrix', matrix), ('unique', unique)):
        for gid, (recipe, reason) in picked.items():
            guns[gid] = (set_name, reason, recipe)

    census = json.loads(Path(args.census).read_text(encoding='utf-8'))
    skins_census = census['skins']
    used = {recipe.get('material') for _, _, recipe in guns.values()}
    bases = OrderedDict((gid, recipe) for gid, (recipe, _) in matrix.items())
    skin_picks, skin_skipped = select_skins(skins_census, census['parts'], used, bases, material_lookup(package))
    def fragments_of(part):
        return weapon_recipe.part_fragments(package, part)
    for sid, (base_id, base, part, mic) in skin_picks.items():
        guns[sid] = ('skins', SKIN_REASON, skin_recipe(base_id, base, part, mic, fragments_of))
    print(f'guns with stats: {len(guns)} (matrix {len(matrix)}, unique {len(unique)}); skins {len(skin_picks)}', flush=True)

    selected = OrderedDict((gid, value) for gid, value in guns.items() if value[0] in args.sets)
    rows_cache = {}

    def rows_for(gtype):
        if gtype not in rows_cache:
            rows_cache[gtype] = filter_gestalt_gltf.gestalt_parts(Path(args.gestalt) / f'{gtype}.json')
        return rows_cache[gtype]

    index = OrderedDict()
    for gid, (set_name, reason, recipe) in selected.items():
        recipe['sample_set'], recipe['sample_reason'] = set_name, reason
        (output / f'{gid}.json').write_text(json.dumps(recipe, indent=1), encoding='utf-8')
        index[gid] = index_entry(set_name, recipe, rows_for(gestalt_type(recipe.get('gestalt'))))

    # Recipes of a set that is no longer selected are stale; remove only this run's own naming pattern.
    prefixes = tuple(f'{SET_PREFIX[s]}_' for s in args.sets)
    stale = sorted(p.name for p in output.glob('*.json') if p.name.startswith(prefixes) and p.stem not in selected)
    for name in stale:
        (output / name).unlink()
    if stale:
        print(f'removed stale recipes: {stale}')

    order = [gid for s in SETS if s in args.sets for gid in index if index[gid]['set'] == s]
    (output / 'load_order.txt').write_text('\n'.join(order) + '\n', encoding='utf-8')

    possible = {(gestalt_type(r['gestalt']), leaf(r.get('manufacturer') or 'none'))
                for r in rolled if is_weapon_gun(r)}
    covered = {recipe.get('material') for _, _, recipe in guns.values()}
    report = {
        'game': str(game), 'seed': SEED, 'game_stage': GAME_STAGE, 'sets_run': list(args.sets),
        'balances': len(balances), 'elapsed_seconds': round(time.time() - started, 1),
        'totals': totals(index, possible, skins_census, covered, roll_failures, stats_failures),
        'skins_skipped': skin_skipped, 'unique_skipped': unique_skipped,
        'roll_failures': roll_failures, 'stats_failures': stats_failures,
        'guns_with_unresolved_attributes': sorted(gid for gid in index if guns[gid][2]['stats'].get('unresolved_attributes')),
        'guns': index,
    }
    (output / 'sample_index.json').write_text(json.dumps(report, indent=1), encoding='utf-8')
    print(f'wrote {len(index)} recipes to {output} in {report["elapsed_seconds"]}s')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--reader', default=str(REPO / 'build' / 'Release' / 'ow-package.exe'))
    parser.add_argument('--census', default=str(REPO / 'local' / 'wpa' / 'census' / 'visual_census.json'))
    parser.add_argument('--gestalt', default=str(REPO / 'local' / 'gestalt'), help='cooked gestalt tables (<Type>.json)')
    parser.add_argument('--output', default=str(REPO / 'local' / 'wpa' / 'sample'), help='must be inside local/')
    parser.add_argument('--sets', nargs='+', choices=SETS, default=list(SETS))
    args = parser.parse_args()
    run(args)


if __name__ == '__main__':
    main()
