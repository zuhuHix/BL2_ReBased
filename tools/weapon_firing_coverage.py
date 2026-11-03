"""Regenerate the firing-test recipes under ignored local/ (WEAPON_PROJECTILES.md section 2).

tools/test_weapon_firing.ps1 reads three recipe folders: local/items (Infinity rolls), local/items/slice (the slice
guns) and local/items/firing/coverage (one gun per manufacturer, type and element). This script

1. rolls the coverage guns below at stage 8, seed 1 from their A_Weapons balances in Startup.upk
   (tools/weapon_recipe.py), evaluates their cards (tools/weapon_stats.py --level 8) and writes them as
   local/items/firing/coverage/<id>.json;
2. adds or refreshes the `firing` block (tools/weapon_firing.py) on every coverage, slice and Infinity recipe it finds.

Run (after `cmake --build build --config Release`, with the slice folder made by tools/weapon_slice_gear.py):

    python tools/weapon_firing_coverage.py --game "<Borderlands 2 folder>"

Everything written is game-derived and stays in the ignored folders. The list of ids is ours; the 2026-10-23 set was
not recorded by id, so this list reconstructs it from the record (every manufacturer, type and element; the ids the
firing test and the frame captures name).
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
TOOLS = REPO / 'tools'
STAGE, SEED = 8, 1
COVERAGE = {
    'pistol_bandit': 'GD_Weap_Pistol.A_Weapons.Pistol_Bandit',
    'pistol_tediore': 'GD_Weap_Pistol.A_Weapons.Pistol_Tediore',
    'pistol_maliwan_shock': 'GD_Weap_Pistol.A_Weapons_Elemental.Pistol_Maliwan_2_Shock',
    'pistol_maliwan_corrosive': 'GD_Weap_Pistol.A_Weapons_Elemental.Pistol_Maliwan_2_Corrosive',
    'pistol_maliwan_slag': 'GD_Weap_Pistol.A_Weapons_Elemental.Pistol_Maliwan_2_Slag',
    'ar_bandit_fire': 'GD_Weap_AssaultRifle.A_Weapons_Elemental.AR_Bandit_2_Fire',
    'ar_dahl': 'GD_Weap_AssaultRifle.A_Weapons.AR_Dahl',
    'ar_torgue_gyrojet': 'GD_Weap_AssaultRifle.A_Weapons.AR_Torgue',
    'ar_vladof': 'GD_Weap_AssaultRifle.A_Weapons.AR_Vladof',
    'smg_hyperion': 'GD_Weap_SMG.A_Weapons.SMG_Hyperion',
    'sg_jakobs': 'GD_Weap_Shotgun.A_Weapons.SG_Jakobs',
    'sg_torgue': 'GD_Weap_Shotgun.A_Weapons.SG_Torgue',
    'sr_vladof': 'GD_Weap_SniperRifles.A_Weapons.Sniper_Vladof',
    'rl_bandit': 'GD_Weap_Launchers.A_Weapons.RL_Bandit',
    'rl_maliwan': 'GD_Weap_Launchers.A_Weapons.RL_Maliwan',
    'rl_tediore': 'GD_Weap_Launchers.A_Weapons.RL_Tediore',
}


def run(*args):
    result = subprocess.run([sys.executable, *map(str, args)], capture_output=True, text=True)
    if result.returncode != 0:
        raise SystemExit(f'failed: {" ".join(map(str, args))}\n{result.stdout}\n{result.stderr}')
    return result.stdout.strip()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install folder')
    parser.add_argument('--reader', type=Path, default=REPO / 'build/Release/ow-package.exe')
    parser.add_argument('--items', type=Path, default=REPO / 'local/items')
    parser.add_argument('--only-firing', action='store_true', help='do not reroll; only refresh the firing blocks')
    args = parser.parse_args(argv)
    cooked = args.game / 'WillowGame/CookedPCConsole'
    startup = cooked / 'Startup.upk'
    items = args.items.resolve()
    if REPO / 'local' not in items.parents and items != REPO / 'local':
        raise SystemExit(f'{items} is not under the ignored local/ folder')
    coverage = items / 'firing/coverage'
    coverage.mkdir(parents=True, exist_ok=True)
    if not args.only_firing:
        for gun, balance in COVERAGE.items():
            out = coverage / f'{gun}.json'
            run(TOOLS / 'weapon_recipe.py', '--reader', args.reader, '--package', startup, '--balance', balance,
                '--seed', SEED, '--stage', STAGE, '--output', out)
            run(TOOLS / 'weapon_stats.py', '--reader', args.reader, '--package', startup, '--recipe', out,
                '--game', args.game, '--level', STAGE)
            print(f'rolled {gun}: {json.loads(out.read_text(encoding="utf-8")).get("name")}')
    recipes = sorted(coverage.glob('*.json')) + sorted((items / 'slice').glob('slice_*.json')) \
        + sorted(items.glob('infinity_*.json'))
    for path in recipes:
        recipe = json.loads(path.read_text(encoding='utf-8'))
        if 'balance' not in recipe or 'parts' not in recipe:
            continue   # manifests and view data
        line = run(TOOLS / 'weapon_firing.py', '--reader', args.reader, '--package', startup, '--recipe', path)
        print(f'firing {path.relative_to(items)}: {line}')
    print(f'{len(COVERAGE)} coverage guns in {coverage}; firing blocks on {len(recipes)} recipes')


if __name__ == '__main__':
    main()
