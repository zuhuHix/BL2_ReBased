"""Stage named weapon recipes for the ignored first-person capture harness.

The host's normal inventory loader intentionally omits mission-lent weapons.
This helper makes local capture-only copies, leaving the source recipes untouched.
All output stays under ignored local/.
"""
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_IDS = ('slice_pistol', 'slice_mission_pistol_fire', 'slice_assault_rifle', 'slice_smg')


def stage_capture_items(source_dir, destination_dir, recipe_ids=DEFAULT_IDS):
    source_dir = Path(source_dir).resolve()
    destination_dir = Path(destination_dir).resolve()
    local_root = (ROOT / 'local').resolve()
    if local_root not in destination_dir.parents:
        raise ValueError(f'capture output must be inside ignored local/: {destination_dir}')
    if not source_dir.is_dir():
        raise FileNotFoundError(source_dir)

    destination_dir.mkdir(parents=True, exist_ok=True)
    staged = []
    for recipe_id in recipe_ids:
        if not recipe_id or Path(recipe_id).name != recipe_id:
            raise ValueError(f'invalid recipe id: {recipe_id!r}')
        source = source_dir / f'{recipe_id}.json'
        data = json.loads(source.read_text(encoding='utf-8-sig'))
        if not isinstance(data, dict):
            raise ValueError(f'{source}: expected a recipe object')
        provenance = data.get('provenance')
        if isinstance(provenance, dict) and provenance.get('kind') == 'mission_weapon':
            provenance['kind'] = 'host_capture_fixture'
        # The inventory ammo resolver accepts the host category key "SMG";
        # the recipe's display label is "Sub-Machine Gun". Normalize only this
        # local capture copy so its reload phase can exercise the existing path.
        item_type = data.get('type')
        if isinstance(item_type, str) and item_type.casefold() == 'sub-machine gun':
            data['type'] = 'SMG'
        target = destination_dir / source.name
        target.write_text(json.dumps(data, indent=1), encoding='utf-8')
        staged.append(target)

    view_manifest = source_dir / 'weapon_view.json'
    if view_manifest.is_file():
        target = destination_dir / view_manifest.name
        target.write_bytes(view_manifest.read_bytes())
        staged.append(target)
    (destination_dir / 'load_order.txt').write_text('\n'.join(recipe_ids) + '\n', encoding='utf-8')
    return staged


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--source', type=Path, default=ROOT / 'local/items/slice')
    parser.add_argument('--destination', type=Path, default=ROOT / 'local/laneE/host-r2-items')
    parser.add_argument('--ids', nargs='+', default=DEFAULT_IDS)
    args = parser.parse_args()
    outputs = stage_capture_items(args.source, args.destination, args.ids)
    print(f'staged {len(outputs)} recipe/manifest files under {args.destination}')
    for output in outputs:
        print(f'  {output.name}')


if __name__ == '__main__':
    main()
