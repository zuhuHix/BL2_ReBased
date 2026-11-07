"""Reload clips and arms clip sets for every weapon type, from local/weapon_library/view_types.json.

One data step for a machine: exports with UModel, converts onto the library skeletons and Maya's arms, and prints the two imports to
run (both start the editor). Everything it writes stays under ignored local/. Each type's default reload clip is taken: the fast
variants depend on a live threshold no table holds.

  python tools/weapon_reload_library.py --game "<BL2 install>" --umodel <umodel.exe>
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import prepare_character_anims as converter  # noqa: E402

# Library folder of each game clip set, where the names differ.
KINDS = {'RocketLauncher': 'Launcher'}
# The clips the Walker plays for a set of its own. The pistol's are imported with the character.
BASE_CLIPS = ('Idle', 'Run_F', 'Sprint', 'Jump_Start', 'Jump_Idle', 'Jump_End', 'Draw', 'ADD_Fire_Recoil')
BASE_DONE = ('Pistol',)
ARMS_PACKAGE = 'GD_Siren_Streaming_SF'


def reload_plan(types):
    """{clip set: {'kind', 'mesh', 'clips'}} from the table's weapon types; types with no set or clip are left out."""
    plan = {}
    for entry in types.values():
        clip_set = str(entry.get('weapon_anim_set') or '').rsplit('.', 1)[-1]
        if not clip_set:
            continue
        row = plan.setdefault(clip_set, {'kind': KINDS.get(clip_set, clip_set), 'mesh': None, 'clips': set()})
        row['mesh'] = row['mesh'] or str(entry.get('socket_table') or '').rsplit('.', 1)[-1] or None
        if entry.get('reload_animation'):
            row['clips'].add(entry['reload_animation'])
    return {name: dict(row, clips=sorted(row['clips'])) for name, row in sorted(plan.items())}


def arms_clips(clip_set, reloads):
    return ([] if clip_set in BASE_DONE else list(BASE_CLIPS)) + list(reloads)


def found_clips(folder, clips):
    """({clip: path} for those present, [clips missing]) in an exported AnimSet folder."""
    present, missing = {}, []
    for clip in clips:
        path = Path(folder) / f'{clip}.md5anim'
        if path.is_file():
            present[clip] = path
        else:
            missing.append(clip)
    return present, missing


def export(umodel, game, out, package, name, kind):
    subprocess.run([str(umodel), f'-path={game}', '-game=border', '-export', '-md5', f'-out={out}', package, name, kind],
                   check=True, capture_output=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--umodel', type=Path, required=True)
    parser.add_argument('--view-types', type=Path, default=ROOT / 'local/weapon_library/view_types.json')
    parser.add_argument('--references', type=Path, required=True,
                        help='Folder of UE reference poses per library type: <kind>.json, the weapon-refs of a recipe of that type')
    parser.add_argument('--arms-reference', type=Path, default=ROOT / 'local/character/anim/ref_pose.json')
    parser.add_argument('--out', type=Path, default=ROOT / 'local/weapon_library/reload')
    args = parser.parse_args()

    plan = reload_plan(json.loads(args.view_types.read_text(encoding='utf-8'))['types'])
    exports = args.out / 'umodel'
    weapons, arms = args.out / 'weapons', args.out / 'arms'
    weapons.mkdir(parents=True, exist_ok=True)
    arms.mkdir(parents=True, exist_ok=True)
    export(args.umodel, args.game, exports, ARMS_PACKAGE, 'Hands_Siren', 'SkeletalMesh')
    arms_mesh = exports / ARMS_PACKAGE / 'SkeletalMesh3' / 'Hands_Siren.md5mesh'
    missing_any = False
    weapon_pairs, arms_pairs = [], []
    for clip_set, row in plan.items():
        kind = row['kind']
        export(args.umodel, args.game, exports, 'Startup', clip_set, 'AnimSet')
        export(args.umodel, args.game, exports, 'Startup', row['mesh'], 'SkeletalMesh')
        export(args.umodel, args.game, exports, ARMS_PACKAGE, f'1st_Person_{clip_set}', 'AnimSet')

        present, missing = found_clips(exports / 'Startup' / 'AnimSet' / clip_set, row['clips'])
        mesh = exports / 'Startup' / 'SkeletalMesh3' / f"{row['mesh']}.md5mesh"
        result = converter.convert(mesh, args.references / f'{kind}.json', present, anchor=None, allow_subset=True)
        output = weapons / f'{kind}.json'
        output.write_text(json.dumps(result), encoding='utf-8')
        weapon_pairs.append(f'{kind}={output}')
        print(f'{kind}: weapon clips {sorted(present)}' + (f'; MISSING {missing}' if missing else ''))
        missing_any = missing_any or bool(missing)

        present, missing = found_clips(exports / ARMS_PACKAGE / 'AnimSet' / f'1st_Person_{clip_set}', arms_clips(clip_set, row['clips']))
        result = converter.convert(arms_mesh, args.arms_reference, present, anchor='Camera')
        output = arms / f'{clip_set}.json'
        output.write_text(json.dumps(result), encoding='utf-8')
        arms_pairs.append(f'{clip_set}={output}')
        print(f'{clip_set}: arms clips {len(present)}' + (f'; MISSING {missing}' if missing else ''))
        missing_any = missing_any or bool(missing)

    print('Import the arms: OPENWILLOW_CHARACTER_ANIMS=' + ';'.join(arms_pairs) + ' host/ue5/import_character_anims.py')
    print("Import the weapon clips: tools/import_weapon_library.ps1 -Only none -Reloads '" + ','.join(weapon_pairs) + "'")
    return 1 if missing_any else 0


if __name__ == '__main__':
    sys.exit(main())
