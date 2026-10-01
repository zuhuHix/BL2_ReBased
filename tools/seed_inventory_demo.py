"""Roll the local inventory demo weapons in a fresh worktree.

Runs, for each pick below, the three existing steps in order:
tools/weapon_recipe.py -> tools/weapon_stats.py -> tools/filter_gestalt_gltf.py
and leaves <id>.json / <id>.gltf under local/items (ignored). Nothing is copied
into the repository. The pick list is a host demo set (weapon types x
manufacturers x rarity); it is not a claim about any real drop table.

Prerequisites (docs/TOOLING.md): a built ow-package, UModel glTF exports of the
GestaltDef_*_GestaltSkeletalMesh objects and their decoded GestaltDef part
tables under --gestalt (<Type>.json, ow-package --properties with
local/infinity/gestalt.schema).
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / 'tools'))
from filter_gestalt_gltf import gestalt_parts  # noqa: E402

# (balance leaf name, seed). Leaf names are resolved to full object paths from
# the package's export list, so a wrong name fails loudly.
PICKS = [
    ('Pistol_Vladof_5_Infinity', 3), ('Pistol_Jakobs_3_Rare', 1), ('Pistol_Hyperion_3_Rare', 2), ('Pistol_Bandit', 1),
    ('SMG_Hyperion_5_Bitch', 1), ('SMG_Dahl_3_Rare', 1), ('SMG_Maliwan', 1),
    ('AR_Jakobs_5_HammerBuster', 1), ('AR_Vladof_3_Rare', 1), ('AR_Dahl', 1),
    ('SG_Torgue_5_Flakker', 1), ('SG_Bandit_3_Rare', 1), ('SG_Hyperion', 1),
    ('Sniper_Jakobs_5_Skullmasher', 1), ('Sniper_Dahl_3_Rare', 1),
    ('RL_Torgue_5_Nukem', 1), ('RL_Vladof_3_Rare', 1), ('RL_Bandit', 1),
]


def run(*command):
    result = subprocess.run([str(part) for part in command], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'{command[1]} failed: {result.stderr.strip() or result.stdout.strip()}')
    return result.stdout


def balance_paths(reader, package):
    text = run(reader, package, '--exports')
    exports = json.loads(text[text.index('['):])
    return {e['path'].split('.')[-1]: e['path'] for e in exports
            if e['class'] == 'WillowGame.WeaponBalanceDefinition'}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', type=Path, default=ROOT / 'build/Release/ow-package.exe')
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install folder')
    parser.add_argument('--gestalt', type=Path, default=ROOT / 'local/gestalt',
                        help='folder holding <Type>.json part tables')
    parser.add_argument('--gltf', type=Path, default=ROOT / 'local/external/umodel/gestalt/Startup/SkeletalMesh3',
                        help='folder holding GestaltDef_<Type>_GestaltSkeletalMesh.gltf')
    parser.add_argument('--output', type=Path, default=ROOT / 'local/items')
    args = parser.parse_args()

    package = args.game / 'WillowGame/CookedPCConsole/Startup.upk'
    paths = balance_paths(args.reader, package)
    args.output.mkdir(parents=True, exist_ok=True)
    tools = ROOT / 'tools'
    for leaf, seed in PICKS:
        if leaf not in paths:
            raise SystemExit(f'Unknown balance {leaf}')
        recipe_id = f'{leaf.lower()}_{seed}'
        recipe = args.output / f'{recipe_id}.json'
        run(sys.executable, tools / 'weapon_recipe.py', '--reader', args.reader, '--package', package,
            '--balance', paths[leaf], '--seed', seed, '--output', recipe)
        run(sys.executable, tools / 'weapon_stats.py', '--reader', args.reader, '--package', package,
            '--recipe', recipe, '--game', args.game)
        data = json.loads(recipe.read_text(encoding='utf-8'))
        gestalt = data['gestalt']
        kind = (gestalt['path'] if isinstance(gestalt, dict) else str(gestalt)).split('.')[-1].removeprefix('GestaltDef_')
        # Some rolled parts (typically *_None) decode to a fragment name the
        # gestalt does not contain. Keep the mesh of every fragment that does
        # exist and record the rest, instead of inventing a substitute.
        known = {part['SkeletalMeshFragmentName'] for part in gestalt_parts(args.gestalt / f'{kind}.json')}
        missing = sorted(set(data['gestalt_fragments']) - known)
        if missing:
            data['unresolved_fragments'] = missing
            recipe.write_text(json.dumps(data, indent=2), encoding='utf-8')
        filtered = args.output / f'{recipe_id}.filter.tmp'
        filtered.write_text(json.dumps({**data, 'gestalt_fragments': sorted(set(data['gestalt_fragments']) & known)}),
                            encoding='utf-8')
        try:
            run(sys.executable, tools / 'filter_gestalt_gltf.py',
                '--gltf', args.gltf / f'GestaltDef_{kind}_GestaltSkeletalMesh.gltf',
                '--gestalt', args.gestalt / f'{kind}.json', '--recipe', filtered,
                '--output', args.output / f'{recipe_id}.gltf')
        finally:
            filtered.unlink(missing_ok=True)
        print(f'{recipe_id}: recipe, stats, {kind} mesh' + (f', unresolved {missing}' if missing else ''))


if __name__ == '__main__':
    main()
