"""Write weapon_view.json: each local recipe's first-person view-model numbers, read from its weapon type (lane C).

For every recipe in the given folders this reads the recipe's WeaponTypeDefinition from the installed Startup.upk
and records PlayerViewOffset (forward, right, up in game units) and FirstPersonMeshFOV (the foreground FOV the game
sets while that weapon is held). Output: <folder>/weapon_view.json, keyed by recipe id, read by the host's
AOpenWillowWalker::SelectSlot. Ignored local/ files only.

Why: the running game places the arms mesh at the camera plus the held weapon type's PlayerViewOffset (SDK probe of
2026-10-04: arms origin minus view point = (12.5, 4, 2) with a shotgun held, equal to WT_Bandit_Shotgun's value) and sets
ForegroundFOV to FirstPersonMeshFOV (45; 50 with the Hyperion SMG in hand). Native-code attribution of the offset is
UNVERIFIED; the equality with the live probe is the evidence (docs/verification/WEAPON_VISUALS.md section 10).

  python tools/weapon_view_model.py --game "<BL2 install>" --dir local/items/slice local/items
"""
import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import weapon_recipe  # noqa: E402

SCHEMA = 'FirstPersonMeshFOV=FloatProperty\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--dir', type=Path, nargs='+', required=True)
    parser.add_argument('--schema', type=Path, default=ROOT / 'local/items/slice/slice_gear.schema')
    args = parser.parse_args()
    import tempfile
    schema = Path(tempfile.mkdtemp()) / 'view.schema'
    lines = [l for l in args.schema.read_text(encoding='utf-8').splitlines() if l.strip() and not l.startswith('FirstPersonMeshFOV')]
    schema.write_text(chr(10).join(lines + ['FirstPersonMeshFOV=FloatProperty']) + chr(10), encoding='utf-8')
    package = weapon_recipe.Package(str(Path(args.reader).resolve()),
                                    args.game / 'WillowGame/CookedPCConsole/Startup.upk', str(schema.resolve()))
    for folder in args.dir:
        view = {}
        for path in sorted(folder.glob('*.json')):
            if path.name == 'weapon_view.json':
                continue
            try:
                recipe = json.loads(path.read_text(encoding='utf-8-sig'))
            except ValueError:
                continue
            weapon_type = recipe.get('weapon_type') if isinstance(recipe, dict) else None
            if not weapon_type or 'parts' not in recipe:
                continue
            props = package.props(weapon_type)
            offset = props.get('PlayerViewOffset')
            if isinstance(offset, dict) and {'X', 'Y', 'Z'} <= set(offset):
                entry = {'weapon_type': weapon_type,
                         'player_view_offset': [float(offset['X']), float(offset['Y']), float(offset['Z'])]}
                if props.get('FirstPersonMeshFOV') is not None:
                    entry['first_person_fov'] = float(props['FirstPersonMeshFOV'])
                view[path.stem] = entry
            else:
                print(f'{path.stem}: no PlayerViewOffset on {weapon_type}; host default')
        out = folder / 'weapon_view.json'
        out.write_text(json.dumps(view, indent=1), encoding='utf-8')
        print(f'{out}: {len(view)} recipes')
        for key, entry in view.items():
            print(f'  {key}: offset {entry["player_view_offset"]} fov {entry.get("first_person_fov")}')


if __name__ == '__main__':
    main()
