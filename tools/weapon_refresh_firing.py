"""Add the live firing-mode data (a `firing` block) to the item cards of existing local recipes.

Since the 2026-11-02 merge of main the UE host no longer reads this card block: it fires from the recipe-level `firing` block
that tools/weapon_firing.py writes (OpenWillowFiring, src/weapon_firing.*). The card block stays a record of the live dump and a
cross-check for that tool; whether to retire this script is a maintainer decision.

Why: the host fired every gun as one instant ray with a made-up wobble. Lane L7 read the real firing modes from the running game
(docs/verification/PROJECTILE_FIRING_PATTERNS_LIVE.md): the mode's speed, pattern lines and flags decide how many bullets a press
makes, where they go and how fast they fly. The cooked reader cannot decode `FiringPatternLines`, so the lines come from the live
dump, not from guessing (the dump is game data under ignored local/realgame/L7/).

Reads (all ignored game data):
  firing_modes.json   every FiringModeDefinition, props as the SDK read them (tools/real_game/scripts/firing_probe.py)
  cooked_raw.json     (optional) WeaponTypeDefinition.InstantHitMomentum, which the live dump does not hold
For each recipe with `stats.card.firing_mode` it writes `stats.card.firing` (and mirrors it into slice_manifest.json when that is in the
folder). Nothing else in the recipe changes. A recipe whose mode is not in the dump is reported and left alone.

The block (units: uu = UE cm, degrees, the game's rotator units 65,536 per turn are converted here):
  mode, fire_type (Bullet | Rocket | Beam), speed (the mode's own, before the weapon multiplier), air_friction, num_ricochets,
  trace_extent [x, y, z], suppress_weapon_spread, fire_randomly_from_pattern, scale_pattern_by_weapon_spread,
  reset_pattern_after_each_shot, base_pattern_spread, min_pattern_scale, tracer (PartSysTemplate), projectiles_per_shot (the card's),
  pattern_lines [{kind: point | rectangle, start: [pitch, yaw, roll], end: [...]}] in degrees (end only for a rectangle),
  instant_hit_momentum (type value, null when the type has none stored), projectile_definition (rocket modes, else null).
The pellet speed the host flies at is the card's `projectile_speed` (mode speed x the weapon's projectile speed multiplier).

  python tools/weapon_refresh_firing.py --dir local/items/slice --live local/realgame/L7 --backup local/p2/Hf/backup
Re-run it after tools/weapon_refresh_stats.py or tools/weapon_slice_gear.py (those rewrite the card).
"""
import argparse
import json
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
UNITS_PER_DEGREE = 65536 / 360


def degrees(rotator):
    """[pitch, yaw, roll] in degrees from a live rotator dict in rotator units."""
    return [round(rotator[axis] / UNITS_PER_DEGREE, 6) for axis in ('Pitch', 'Yaw', 'Roll')]


def pattern_line(line):
    if line['bUseStartPointOnly']:
        return {'kind': 'point', 'start': degrees(line['StartPoint'])}
    return {'kind': 'rectangle', 'start': degrees(line['StartPoint']), 'end': degrees(line['EndPoint'])}


def build_block(mode_path, props, projectiles, momentum):
    """The `firing` block for one mode (props = the live property dict of that FiringModeDefinition)."""
    return {
        'mode': mode_path,
        'fire_type': props['FireType'].removeprefix('EWWFT_'),
        'speed': props['Speed'],
        'air_friction': props['AirFriction'],
        'num_ricochets': props['NumRicochets'],
        'trace_extent': [props['TraceExtent'][axis] for axis in 'XYZ'],
        'suppress_weapon_spread': props['bSuppressWeaponSpread'],
        'fire_randomly_from_pattern': props['bFireRandomlyFromPattern'],
        'scale_pattern_by_weapon_spread': props['bScalePatternByWeaponSpread'],
        'reset_pattern_after_each_shot': props['bResetPatternAfterEachShot'],
        'base_pattern_spread': props['BasePatternSpread'],
        'min_pattern_scale': props['MinPatternScale'],
        'tracer': props['PartSysTemplate'],
        'projectiles_per_shot': projectiles,
        'pattern_lines': [pattern_line(line) for line in props['FiringPatternLines']],
        'instant_hit_momentum': momentum,
        'projectile_definition': props['ProjectileDefinition'],
    }


def type_momentum(cooked, weapon_type):
    """WeaponTypeDefinition.InstantHitMomentum from the cooked dump, or None when the type does not store one."""
    path = weapon_type['path'] if isinstance(weapon_type, dict) else weapon_type
    value = (cooked.get(path) or {}).get('InstantHitMomentum')
    return value.get('BaseValueConstant') if isinstance(value, dict) else None


def keep_original(path, backup):
    """Copy `path` into `backup` once; a second run keeps the first copy (the original)."""
    if backup and not (backup / path.name).exists():
        backup.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, backup / path.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--dir', type=Path, required=True, help='folder of recipe JSONs (local/items/slice)')
    parser.add_argument('--live', type=Path, default=ROOT / 'local/realgame/L7', help='folder with firing_modes.json (L7 dump)')
    parser.add_argument('--backup', type=Path, help='copy each JSON here before its first change (an existing copy is kept)')
    parser.add_argument('--ids', nargs='*')
    args = parser.parse_args()
    modes = json.loads((args.live / 'firing_modes.json').read_text(encoding='utf-8'))['FiringModeDefinition']
    cooked_file = args.live / 'cooked_raw.json'
    cooked = (json.loads(cooked_file.read_text(encoding='utf-8'))['cooked'].get('WillowGame.WeaponTypeDefinition', {})
              if cooked_file.exists() else {})
    blocks = {}
    for path in sorted(args.dir.glob('*.json')):
        if args.ids and path.stem not in args.ids:
            continue
        recipe = json.loads(path.read_text(encoding='utf-8-sig'))
        card = (recipe.get('stats') or {}).get('card')
        if not card or not card.get('firing_mode'):
            continue
        entry = modes.get(card['firing_mode'])
        if not entry:
            print(f'{path.stem}: {card["firing_mode"]} is not in the live dump; left alone')
            continue
        block = build_block(card['firing_mode'], entry['props'], int(card.get('projectiles') or 1),
                            type_momentum(cooked, recipe.get('weapon_type')))
        # Oracle against the card: for a bullet mode the card's projectile_speed is mode speed x the weapon multiplier.
        multiplier = card['projectile_speed'] / block['speed'] if block['speed'] and card.get('projectile_speed') else None
        keep_original(path, args.backup)
        card['firing'] = block
        path.write_text(json.dumps(recipe, indent=1), encoding='utf-8')
        blocks[path.stem] = block
        print(f'{path.stem}: {block["fire_type"]} {block["mode"].split(".")[-1]}: {block["projectiles_per_shot"]} pellet(s), '
              f'speed {block["speed"]:g} x {multiplier or 0:.3g} = {card.get("projectile_speed") or 0:g}, '
              f'{len(block["pattern_lines"])} pattern line(s), spread cone {"suppressed" if block["suppress_weapon_spread"] else card.get("spread")}, '
              f'momentum {block["instant_hit_momentum"]}')
    manifest_path = args.dir / 'slice_manifest.json'
    if blocks and manifest_path.exists():
        manifest = json.loads(manifest_path.read_text(encoding='utf-8-sig'))
        keep_original(manifest_path, args.backup)
        for item in manifest.get('items', []):
            if item['id'] in blocks:
                item['card']['firing'] = blocks[item['id']]
        manifest_path.write_text(json.dumps(manifest, indent=1), encoding='utf-8')
    if not blocks:
        print('no recipe changed', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
