"""Prepare selected per-recipe weapon reload sequences from the local UModel MD5 exports.

Run after `import_weapon_items.py` has written target reference poses under
`local/laneE/weapon-refs`. The conversion output stays in ignored local/ and is
then consumed by `host/ue5/import_weapon_reload_anims.py`.
"""
import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import prepare_character_anims as converter  # noqa: E402


FAMILIES = {
    'Pistol': ('umodel-benchmark-r1-pistol', 'umodel-pistol-preview-r1'),
    'AssaultRifle': ('umodel-assault-rifle-r1', 'umodel-assault-rifle-preview-r1'),
    'SMG': ('umodel-smg-r1', 'umodel-smg-preview-r1'),
}
DEFAULT_IDS = ('slice_pistol', 'slice_mission_pistol_fire', 'slice_assault_rifle', 'slice_smg')


def prepare(items_dir, references_dir, lane_dir, output_dir, recipe_ids=DEFAULT_IDS):
    view_path = Path(items_dir) / 'weapon_view.json'
    view = json.loads(view_path.read_text(encoding='utf-8'))
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    summaries = []
    for recipe_id in recipe_ids:
        entry = view.get(recipe_id)
        if not entry:
            raise ValueError(f'{view_path}: no view data for {recipe_id}')
        clip = entry.get('reload_animation')
        anim_set_path = entry.get('weapon_anim_set') or ''
        family = anim_set_path.rsplit('.', 1)[-1]
        if not clip or family not in FAMILIES:
            raise ValueError(f'{recipe_id}: missing or unsupported reload set/clip ({anim_set_path}, {clip})')
        anim_root, mesh_root = FAMILIES[family]
        anim_matches = list((Path(lane_dir) / anim_root).rglob(f'{clip}.md5anim'))
        mesh_matches = list((Path(lane_dir) / mesh_root).rglob('*.md5mesh'))
        if len(anim_matches) != 1 or len(mesh_matches) != 1:
            raise ValueError(f'{recipe_id}: expected one {clip}.md5anim and one family MD5 mesh; '
                             f'found {len(anim_matches)} and {len(mesh_matches)}')
        reference = Path(references_dir) / f'{recipe_id}.json'
        if not reference.is_file():
            raise FileNotFoundError(f'{recipe_id}: target Unreal reference pose missing: {reference}')
        result = converter.convert(mesh_matches[0], reference, {clip: anim_matches[0]}, anchor=None,
                                   allow_subset=True)
        output = output_dir / f'{recipe_id}.json'
        output.write_text(json.dumps(result), encoding='utf-8')
        summaries.append({'recipe': recipe_id, 'clip': clip, 'frames': result[clip]['frames'],
                          'source_rate': result[clip]['rate'], 'output': str(output)})
    return summaries


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--items-dir', type=Path, default=ROOT / 'local/items/slice')
    parser.add_argument('--references-dir', type=Path, default=ROOT / 'local/laneE/weapon-refs')
    parser.add_argument('--lane-dir', type=Path, default=ROOT / 'local/laneE')
    parser.add_argument('--output-dir', type=Path, default=ROOT / 'local/laneE/prepared-reloads')
    parser.add_argument('--ids', nargs='+', default=DEFAULT_IDS)
    args = parser.parse_args()
    print(json.dumps(prepare(args.items_dir, args.references_dir, args.lane_dir,
                             args.output_dir, args.ids), indent=2))


if __name__ == '__main__':
    main()
