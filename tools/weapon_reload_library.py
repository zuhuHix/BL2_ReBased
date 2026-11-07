"""Reload clips and arms clip sets for the weapon types in local/weapon_library/view_types.json (types in DLC packages are not in it).

One data step for a machine: exports with UModel, converts onto the library skeletons and Maya's arms, and prints the two imports to
run (both start the editor). Everything it writes stays under ignored local/; --out elsewhere is refused. Its export and result folders
are emptied first, so a file from an older run cannot count as this run's. Each type's default reload clip is taken: the fast variants
depend on a live threshold no table holds.

  python tools/weapon_reload_library.py --game "<BL2 install>" --umodel <umodel.exe>
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import prepare_character_anims as converter  # noqa: E402
from lootables_editor import local_output  # noqa: E402

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


def unusable_sets(plan):
    """The sets the step cannot finish, each with its reason: a set with no reload clip would write an empty file and look done."""
    return [f'{name}: no reload clip' if not row['clips'] else f'{name}: no mesh'
            for name, row in plan.items() if not row['clips'] or not row['mesh']]


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


def powershell_quoted(text):
    return "'" + str(text).replace("'", "''") + "'"


def import_lines(arms_pairs, weapon_pairs):
    """The two imports, quoted for PowerShell. The lists themselves split on ; and , so main refuses a path holding those."""
    return ['Import the arms: $env:OPENWILLOW_CHARACTER_ANIMS = ' + powershell_quoted(';'.join(arms_pairs))
            + ' then run host/ue5/import_character_anims.py',
            'Import the weapon clips: tools/import_weapon_library.ps1 -Only none -Reloads ' + powershell_quoted(','.join(weapon_pairs))]


def main(argv=None, repo=ROOT):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--umodel', type=Path, required=True)
    parser.add_argument('--view-types', type=Path, default=ROOT / 'local/weapon_library/view_types.json')
    parser.add_argument('--references', type=Path, required=True,
                        help='Folder of UE reference poses per library type: <kind>.json, the weapon-refs of a recipe of that type')
    parser.add_argument('--arms-reference', type=Path, default=ROOT / 'local/character/anim/ref_pose.json',
                        help="Maya's arms reference pose; importing Maya writes it (OPENWILLOW_CHARACTER_REFERENCE)")
    parser.add_argument('--out', type=Path, default=Path('local/weapon_library/reload'), help="Inside the repository's ignored local/")
    args = parser.parse_args(argv)
    try:
        out = local_output(args.out, repo=repo)
        exports, weapons, arms = (local_output(out / name, repo=repo) for name in ('umodel', 'weapons', 'arms'))
    except ValueError as error:
        parser.error(str(error))
    if set(';,') & set(str(out)):
        parser.error(f'{out}: the import lists split on ; and , so the path cannot hold them')

    plan = reload_plan(json.loads(args.view_types.read_text(encoding='utf-8'))['types'])
    problems = unusable_sets(plan) if plan else ['the table has no clip set']
    problems += [f'no reference pose {path}' for path in
                 [args.references / f"{row['kind']}.json" for row in plan.values()] + [args.arms_reference] if not path.is_file()]
    if problems:
        print('Nothing written: ' + '; '.join(problems), file=sys.stderr)
        return 1
    for folder in (exports, weapons, arms):
        if folder.exists():
            shutil.rmtree(folder)
    weapons.mkdir(parents=True)
    arms.mkdir(parents=True)
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
        print(f'{kind}: weapon clips {sorted(present)}' + (f'; MISSING {missing}' if missing else ''))
        missing_any = missing_any or bool(missing)
        if present:
            mesh = exports / 'Startup' / 'SkeletalMesh3' / f"{row['mesh']}.md5mesh"
            result = converter.convert(mesh, args.references / f'{kind}.json', present, anchor=None, allow_subset=True)
            output = weapons / f'{kind}.json'
            output.write_text(json.dumps(result), encoding='utf-8')
            weapon_pairs.append(f'{kind}={output}')

        present, missing = found_clips(exports / ARMS_PACKAGE / 'AnimSet' / f'1st_Person_{clip_set}', arms_clips(clip_set, row['clips']))
        print(f'{clip_set}: arms clips {len(present)}' + (f'; MISSING {missing}' if missing else ''))
        missing_any = missing_any or bool(missing)
        if present:
            result = converter.convert(arms_mesh, args.arms_reference, present, anchor='Camera')
            output = arms / f'{clip_set}.json'
            output.write_text(json.dumps(result), encoding='utf-8')
            arms_pairs.append(f'{clip_set}={output}')

    for line in import_lines(arms_pairs, weapon_pairs):
        print(line)
    return 1 if missing_any else 0


if __name__ == '__main__':
    sys.exit(main())
