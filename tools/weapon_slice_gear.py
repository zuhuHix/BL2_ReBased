"""Build the Sanctuary slice's guns and loot table from stock data (local/items/slice).

Writes, under ignored local/:
- <id>.json: weapon recipes in the existing host format (tools/weapon_recipe.py
  output + `stats` from tools/weapon_stats.py), so `-owitems=<dir>` /
  UOpenWillowInventory::LoadRecipes reads them unchanged. Additive fields:
  `type` (card type label), `legal_parts` (per slot: fixed or rolled, weights) and
  `provenance` (where the gun comes from: mission weapon or a pool roll, seeds,
  level rule).
- load_order.txt: mission pistol first.
- slice_manifest.json: index of the above plus the slice loot table
  (tools/loot_pools.py) for the chosen loot source. It has no `stats` key, so the
  host's recipe loader skips it.

Picks:
- the lent mission pistol GD_Z1_RockPaperGenocideData.MW_RockPaper_Fire;
- one gun per type pool (Pool_Weapons_<Type>) rolled from a seed through the
  pool tree (tools/loot_pools.py), then its parts rolled from a seed.
Level: 8 by default, inside both Sanctuary's playthrough-1 default band (7-9) and
the M_Ep4_WelcomeToSanctuary override band (8-11) in
GD_GameStages.Balance.Balance_P1_Zone1. How the game picks a level inside a band,
and which level a lent mission weapon gets, is native code: UNVERIFIED.
"""
import argparse
import itertools
import json
import random
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import loot_pools  # noqa: E402
import skill_stats  # noqa: E402
import weapon_balance  # noqa: E402
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
MISSION_WEAPON = 'GD_Z1_RockPaperGenocideData.MW_RockPaper_Fire'
MISSION = 'GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire'
TYPE_POOLS = [('slice_pistol', 'GD_Itempools.WeaponPools.Pool_Weapons_Pistols'),
              ('slice_smg', 'GD_Itempools.WeaponPools.Pool_Weapons_SMG'),
              ('slice_assault_rifle', 'GD_Itempools.WeaponPools.Pool_Weapons_AssaultRifles'),
              ('slice_shotgun', 'GD_Itempools.WeaponPools.Pool_Weapons_Shotguns')]
LOOT_SOURCE = 'GD_Itempools.ListDefs.StandardEnemyGunsAndGear'
LEVEL_RULE = ('UNVERIFIED slice choice: 8 lies in Sanctuary P1 default band 7-9 and the Ep4 override band '
              '8-11 (GD_GameStages.Balance.Balance_P1_Zone1); the native level pick is not decoded')


def card_type(package, weapon_type):
    """The card's type label (WeaponTypeDefinition.Typename, e.g. 'Pistol')."""
    return package.props(weapon_type).get('Typename') if weapon_type else None


def summarize_parts(parts):
    return {slot: {'fixed': data['fixed'], 'all_zero_weight': data['all_zero_weight'],
                   'candidates': [{'part': c['part'], 'weight': c['weight'], 'share': c['share']}
                                  for c in data['candidates']]}
            for slot, data in parts['slots'].items()}


def build_recipe(package, balance, seed, level, localize, provenance):
    recipe = weapon_recipe.roll(package, balance, seed, level)
    recipe['stats'] = weapon_stats.evaluate(package, recipe, level, localize)
    recipe['type'] = card_type(package, recipe['weapon_type'])
    legal = weapon_balance.legal_parts(package, balance, level)
    recipe['legal_parts'] = {'stage': level, 'combinations': legal['combinations'],
                             'slots': summarize_parts(legal)}
    recipe['provenance'] = provenance
    return recipe


def stat_ranges(package, recipe, level):
    """Card numbers (as printed) over every legal part combination at `level`, unweighted."""
    merged = weapon_recipe.merge(package, recipe['balance'], level)
    slots = {slot: list(dict.fromkeys(part for part, _ in entries)) for slot, entries in merged['merged'].items()
             if entries}
    ranges, count = {}, 0
    for combo in itertools.product(*slots.values()):
        count += 1
        candidate = {'manufacturer': merged['manufacturer'], 'weapon_type': merged['weapon_type'],
                     'parts': {slot: {'part': part} for slot, part in zip(slots, combo)}}
        card = weapon_stats.evaluate(package, candidate, level)['card']
        for field, number in {**card['display'], 'projectiles': card['projectiles']}.items():
            low, high = ranges.get(field, (number, number))
            ranges[field] = (min(low, number), max(high, number))
    return {'combinations': count, 'stats': {f: {'min': lo, 'max': hi} for f, (lo, hi) in sorted(ranges.items())}}


def build_mesh(recipe_path, recipe, gestalt_dir, gltf_dir):
    """<id>.gltf through tools/filter_gestalt_gltf.py, as tools/seed_inventory_demo.py does."""
    from filter_gestalt_gltf import gestalt_parts
    gestalt = recipe['gestalt']
    kind = (gestalt['path'] if isinstance(gestalt, dict) else str(gestalt)).split('.')[-1].removeprefix('GestaltDef_')
    table = gestalt_dir / f'{kind}.json'
    mesh = gltf_dir / f'GestaltDef_{kind}_GestaltSkeletalMesh.gltf'
    if not table.exists() or not mesh.exists():
        return f'skipped: {table.name} or {mesh.name} missing'
    known = {part['SkeletalMeshFragmentName'] for part in gestalt_parts(table)}
    missing = sorted(set(recipe['gestalt_fragments']) - known)
    if missing:
        recipe['unresolved_fragments'] = missing
    filtered = recipe_path.with_suffix('.filter.tmp')
    filtered.write_text(json.dumps({**recipe, 'gestalt_fragments': sorted(set(recipe['gestalt_fragments']) & known)}),
                        encoding='utf-8')
    try:
        result = subprocess.run([sys.executable, str(ROOT / 'tools/filter_gestalt_gltf.py'), '--gltf', str(mesh),
                                 '--gestalt', str(table), '--recipe', str(filtered),
                                 '--output', str(recipe_path.with_suffix('.gltf'))], capture_output=True, text=True)
    finally:
        filtered.unlink(missing_ok=True)
    return 'ok' if result.returncode == 0 else f'failed: {result.stderr.strip()[:200]}'


def pick_from_pool(package, values, pool, level, seed):
    """Seeded roll down a pool tree to one item balance (UNVERIFIED native selection rule)."""
    tree = loot_pools.expand(package, values, pool, level)
    drops = loot_pools.roll_pool(tree, random.Random(seed))
    weapons = [d for d in drops if package.classes.get(d['item'], '').endswith('WeaponBalanceDefinition')]
    return (weapons[0], tree) if weapons else (None, tree)


REWARD_ID = 'slice_reward_roll'
REWARD_NOTE = ('HOST STAND-IN, not stock: the mission RewardData carries only ExperienceRewardPercentage and '
               'CreditRewardMultiplier 0, and PawnBalance_TargetDummy has no item pools, so nothing in the data drops '
               'an item for this mission. This rolls the slice fallback list (loot.source) so the pickup path can be '
               'exercised at turn-in. The seed is the first one >= 1 whose roll drops a weapon (most single rolls drop '
               'nothing or money); the roll and part rules are seeded stand-ins for native code (UNVERIFIED).')


def build_reward(args, package, localize):
    """The turn-in loot stand-in: roll the fallback list from seed 1 up until a weapon drops; write its recipe."""
    manifest_path = args.output / 'slice_manifest.json'
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    loot = manifest['loot']
    tried = []
    for seed in range(1, args.reward_seed_limit + 1):
        drops = loot_pools.roll(loot, seed)
        tried.append({'seed': seed, 'drops': [d['item'] for d in drops]})
        weapons = [d for d in drops if package.classes.get(d['item'], '').endswith('WeaponBalanceDefinition')]
        if weapons:
            break
    else:
        raise SystemExit(f'no weapon in {args.reward_seed_limit} seeded rolls of {loot["source"]}')
    drop = weapons[0]
    # Parts use the roll seed too, so the reward is not a part-for-part copy of a pool-rolled slice gun.
    recipe = build_recipe(package, drop['item'], seed, manifest['level'], localize, {
        'kind': 'reward_roll', 'source': loot['source'], 'pool_chain': drop['chain'], 'roll_seed': seed,
        'part_seed': seed, 'level': manifest['level'], 'level_rule': manifest['level_rule'], 'note': REWARD_NOTE})
    path = args.output / f'{REWARD_ID}.json'
    mesh = build_mesh(path, recipe, args.gestalt, args.gltf) if args.gestalt and args.gltf else 'not built'
    path.write_text(json.dumps(recipe, indent=1), encoding='utf-8')
    manifest['reward_roll'] = {'id': REWARD_ID, 'recipe': path.name, 'balance': recipe['balance'], 'name': recipe['name'],
                               'type': recipe['type'], 'roll_seed': seed, 'seeds_tried': tried, 'mesh': mesh,
                               'note': REWARD_NOTE}
    manifest_path.write_text(json.dumps(manifest, indent=1), encoding='utf-8')
    print(f"{REWARD_ID}: seed {seed} of {len(tried)} -> {recipe['name']} ({recipe['balance']}) via {' > '.join(drop['chain'])}; mesh {mesh}")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install folder')
    parser.add_argument('--level', type=int, default=8)
    parser.add_argument('--seed', type=int, default=1, help='part-roll seed (pool picks use seed + index)')
    parser.add_argument('--output', type=Path, default=ROOT / 'local/items/slice')
    parser.add_argument('--gestalt', type=Path, help='folder of <Type>.json gestalt part tables (optional meshes)')
    parser.add_argument('--gltf', type=Path, help='folder of GestaltDef_<Type>_GestaltSkeletalMesh.gltf (optional)')
    parser.add_argument('--reward-only', action='store_true',
                        help='only (re)build the turn-in loot stand-in (slice_reward_roll) from an existing slice_manifest.json')
    parser.add_argument('--reward-seed-limit', type=int, default=200)
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to((ROOT / 'local').resolve()):
        parser.error('--output must stay under local/')
    args.output.mkdir(parents=True, exist_ok=True)
    startup = args.game / 'WillowGame/CookedPCConsole/Startup.upk'
    schema = args.output / 'slice_gear.schema'
    schema.write_text('\n'.join(dict.fromkeys(weapon_stats.SCHEMA_LINES + loot_pools.SCHEMA_LINES)) + '\n',
                      encoding='utf-8')
    package = skill_stats.Package(str(Path(args.reader).resolve()), startup, str(schema.resolve()))
    package.crawl([MISSION_WEAPON, LOOT_SOURCE] + [p for _, p in TYPE_POOLS],
                  follow=lambda path, cls: cls.endswith('Definition') or 'ItemPool' in cls)
    files = {}

    def localize(path, default):
        name = path.split('.', 1)[0]
        if name not in files:
            files[name] = skill_stats.localization_for(args.game, name)
        return files[name].get((skill_stats.localization_key(path, package.classes), 'NoConstraintText'), default)

    if args.reward_only:
        build_reward(args, package, localize)
        return
    values = loot_pools.Values(package)
    items = []
    recipe = build_recipe(package, MISSION_WEAPON, args.seed, args.level, localize, {
        'kind': 'mission_weapon', 'mission': MISSION, 'objective': f'{MISSION}.Fire',
        'part_seed': args.seed, 'level': args.level, 'level_rule': LEVEL_RULE,
        'note': 'MissionWeaponBalanceDefinition over GD_Weap_Pistol.A_Weapons_Elemental.Pistol_Maliwan_2_Fire; '
                'the part pick inside rolled slots is a seeded stand-in for native code (UNVERIFIED)'})
    recipe['stat_ranges'] = stat_ranges(package, recipe, args.level)
    items.append(('slice_mission_pistol_fire', recipe))
    for index, (recipe_id, pool) in enumerate(TYPE_POOLS):
        pool_seed = args.seed + index
        drop, _ = pick_from_pool(package, values, pool, args.level, pool_seed)
        if drop is None:
            print(f'{recipe_id}: {pool} gave no weapon at stage {args.level}', file=sys.stderr)
            continue
        recipe = build_recipe(package, drop['item'], args.seed, args.level, localize, {
            'kind': 'pool_roll', 'pool_chain': drop['chain'], 'pool_seed': pool_seed, 'part_seed': args.seed,
            'level': args.level, 'level_rule': LEVEL_RULE,
            'note': 'pool and part selection are seeded stand-ins for native code (UNVERIFIED)'})
        items.append((recipe_id, recipe))

    meshes = {}
    for recipe_id, recipe in items:
        path = args.output / f'{recipe_id}.json'
        if args.gestalt and args.gltf:
            meshes[recipe_id] = build_mesh(path, recipe, args.gestalt, args.gltf)
        path.write_text(json.dumps(recipe, indent=1), encoding='utf-8')
    (args.output / 'load_order.txt').write_text('\n'.join(i for i, _ in items) + '\n', encoding='utf-8')
    loot = loot_pools.table(package, loot_pools.Values(package), LOOT_SOURCE, args.level)
    manifest = {
        'schemaVersion': 1, 'kind': 'openwillow.slice_gear', 'level': args.level, 'level_rule': LEVEL_RULE,
        'items': [{'id': i, 'recipe': f'{i}.json', 'balance': r['balance'], 'name': r['name'], 'type': r['type'],
                   'provenance': r['provenance'], 'mesh': meshes.get(i, 'not built'), 'card': {k: r['stats']['card'].get(k) for k in (
                       'damage', 'fire_rate', 'reload_time', 'magazine', 'shot_cost', 'projectiles', 'element',
                       'status_effect', 'status_chance', 'status_dps', 'status_duration', 'accuracy', 'display')}}
                  for i, r in items],
        'loot': {'source': LOOT_SOURCE, 'note': 'GD_Population_Psycho.Balance.PawnBalance_TargetDummy has no item pools; '
                 'this is the standard enemy list (e.g. PawnBalance_Psycho)', **loot},
    }
    (args.output / 'slice_manifest.json').write_text(json.dumps(manifest, indent=1), encoding='utf-8')
    for i, r in items:
        card = r['stats']['card']
        print(f"{i}: {r['name']} ({r['balance']}) dmg {card['display'].get('damage')} "
              f"rate {card['display'].get('fire_rate')} mag {card['display'].get('magazine')} "
              f"element {card['element']} chance {card.get('status_chance')}")


if __name__ == '__main__':
    main()
