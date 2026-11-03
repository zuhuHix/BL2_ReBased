"""Re-evaluate the item-card `stats` of existing local recipes, optionally on the values read from the running game.

Why: the six slice recipes in local/items/slice were written before the evaluator's single-precision/rounding work
(DECISIONS 2026-10-03) and from the cooked packages only. Lane D saw the Bandit shotgun and the turn-in shotgun print reload
4.4 and magazine 9 / 3.7 and 13 where the game's cards read 4.1 and 10 / 3.5 and 14. Two causes, both found here:
1. stale numbers: the stored stats came from an older evaluator (they lack the level line and carry double-precision values);
2. type data: the running game's WT_Bandit_Shotgun differs from the cooked decode (ClipSize 10 against 9, ReloadTime 4.1
   against 4.4, plus FireRate, InstantHitDamage, AttributeSlotEffects and StatusEffectDamage), the documented
   live-versus-cooked difference of `docs/verification/REALGAME_GROUND_TRUTH.md` ("Live weapon data"). With the live overlay the
   shotgun card reads 4.1 / 10 and the reward roll 3.5 / 14, as the game's cards do (inspect view of the exact shotgun parts on
   2026-10-04: reload 4.1, magazine 10).

Only `stats` is replaced (plus `stats_source`); parts, name, mesh fragments and provenance stay. Reads/writes ignored local/ files.
The live file is game data (tools/real_game/scripts/weapon_dump.py); where its values come from is still unexplained, so this is a
choice of data source, not an evaluator change.

  python tools/weapon_refresh_stats.py --game "<BL2 install>" --dir local/items/slice --live-data local/realgame/cards/live_weapon_data.json
"""
import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
sys.path.insert(0, str(ROOT / 'tools' / 'real_game'))
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--dir', type=Path, nargs='+', required=True)
    parser.add_argument('--live-data', type=Path, help='live_weapon_data.json: use the values read from the running game')
    parser.add_argument('--schema', type=Path, default=ROOT / 'local/items/slice/slice_gear.schema')
    parser.add_argument('--ids', nargs='*')
    parser.add_argument('--level', type=int, default=None, help='game stage; default the recipe\'s stored level')
    args = parser.parse_args()
    startup = args.game / 'WillowGame/CookedPCConsole/Startup.upk'
    reader, schema = str(Path(args.reader).resolve()), str(args.schema.resolve())
    if args.live_data:
        import live_overlay
        package = live_overlay.LivePackage(reader, startup, schema, args.live_data)
    else:
        package = weapon_recipe.Package(reader, startup, schema)
    for folder in args.dir:
        for path in sorted(folder.glob('*.json')):
            if args.ids and path.stem not in args.ids:
                continue
            recipe = json.loads(path.read_text(encoding='utf-8-sig'))
            if 'parts' not in recipe or 'stats' not in recipe:
                continue
            level = args.level or int((recipe.get('provenance') or {}).get('level') or recipe['stats']['card'].get('level') or 8)
            before = recipe['stats']['card'].get('display', {})
            recipe['stats'] = weapon_stats.evaluate(package, recipe, level)
            recipe['stats_source'] = 'live overlay (running game values)' if args.live_data else 'cooked packages'
            path.write_text(json.dumps(recipe, indent=1), encoding='utf-8')
            after = recipe['stats']['card']['display']
            changed = {k: (before.get(k), after.get(k)) for k in after if before.get(k) != after.get(k)}
            print(f'{path.stem}: level {level} {recipe["stats_source"]}; changed display fields {changed or "none"}')


if __name__ == '__main__':
    main()
