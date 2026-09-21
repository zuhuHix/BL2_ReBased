"""Select an installed map and prepare, import or open its UE5 scene."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
from installed_content import PACKAGE_SUFFIXES, content_files

ROOT = Path(__file__).resolve().parents[1]
SANCTUARY_MAP = 'Sanctuary_P'


def catalog(game, include_dlc=False):
    """Filename discovery only; presence does not mean viewer compatibility."""
    if not (game / 'Binaries/Win32/Borderlands2.exe').is_file():
        raise ValueError('An installed Borderlands 2 is required')
    found = {}
    for path in content_files(game, include_dlc):
        if path.suffix.lower() in PACKAGE_SUFFIXES and re.fullmatch(
                r'[A-Za-z0-9_]+_P', path.stem, re.IGNORECASE):
            found.setdefault(path.stem.casefold(), []).append(path)
    return [{'map': paths[0].stem,
             'packages': [p.relative_to(game).as_posix() for p in paths],
             'selectable': len(paths) == 1}
            for _, paths in sorted(found.items())]


def select(records, name):
    matches = [r for r in records if r['map'].casefold() == name.casefold()]
    if not matches:
        raise ValueError(f'Map is not installed: {name}')
    if not matches[0]['selectable']:
        raise ValueError(f'Ambiguous package name: {name}')
    return matches[0]['map']


def validate_options(args, selected_map=None):
    if not args.sanctuary_geometry:
        return
    if args.action != 'prepare':
        raise ValueError('--sanctuary-geometry requires --action prepare')
    if args.list:
        raise ValueError('--sanctuary-geometry requires a selected map, not --list')
    if selected_map is not None and selected_map.casefold() != SANCTUARY_MAP.casefold():
        raise ValueError('--sanctuary-geometry is only supported for Sanctuary_P')


def reject_plain_sanctuary_rebuild(scene, name, sanctuary_geometry):
    if sanctuary_geometry or name.casefold() != SANCTUARY_MAP.casefold():
        return
    manifest = scene / 'scene.json'
    if not manifest.is_file():
        return
    data = json.loads(manifest.read_text(encoding='utf-8'))
    if 'terrain_policy' in data or 'bsp_policy' in data:
        raise ValueError(
            'Existing Sanctuary scene contains recovered terrain or BSP; '
            'rerun with --sanctuary-geometry to preserve it')


def preparation_commands(args, name, scene):
    game = str(args.game.resolve())
    reader = str(args.reader.resolve())
    level = [sys.executable, str(ROOT / 'tools/prepare_level.py'),
             '--game', game, '--reader', reader, '--map', name,
             '--output', str(scene)]
    if args.include_dlc:
        level.append('--include-dlc')
    if args.outer_shell:
        level.append('--outer-shell')
    commands = [level]
    if args.sanctuary_geometry:
        commands.extend([
            [sys.executable, str(ROOT / 'tools/prepare_terrain.py'),
             '--game', game, '--reader', reader, '--scene', str(scene),
             '--collision'],
            [sys.executable, str(ROOT / 'tools/prepare_bsp.py'),
             '--game', game, '--reader', reader, '--scene', str(scene),
             '--collision'],
        ])
    return commands


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--map', help='Installed package name, e.g. Ash_P')
    parser.add_argument('--list', action='store_true', help='List installed maps and exit')
    parser.add_argument('--json', action='store_true', help='Machine-readable map catalog (with --list)')
    parser.add_argument('--action', choices=('prepare', 'import', 'view'), default='view')
    parser.add_argument('--reader', type=Path, default=ROOT / 'build/Release/ow-package.exe')
    parser.add_argument('--engine', type=Path)
    parser.add_argument('--skip-build', action='store_true')
    parser.add_argument('--include-dlc', action='store_true', help='Include installed DLC maps and texture caches')
    parser.add_argument('--sanctuary-geometry', action='store_true',
                        help='Also prepare Sanctuary terrain and BSP with triangle collision')
    parser.add_argument('--outer-shell', action='store_true',
                        help='Prepare the outer hull meshes with mesh-default materials (labeled approximation)')
    args = parser.parse_args()
    try:
        if args.json and not args.list:
            raise ValueError('--json requires --list')
        validate_options(args)
        records = catalog(args.game.resolve(), args.include_dlc)
        if args.list:
            if args.json:
                print(json.dumps({'schema': 1, 'maps': records}, indent=2))
            else:
                for i, record in enumerate(records, 1):
                    suffix = '' if record['selectable'] else ' [ambiguous; unavailable]'
                    print(f"{i:3}. {record['map']}{suffix}")
                scope = 'installed' if args.include_dlc else 'base-game'
                print(f'{len(records)} {scope} map names; compatibility is not implied.')
            return
        name = args.map
        if not name:
            if not sys.stdin.isatty():
                raise ValueError('Use --map or --list in non-interactive sessions')
            for i, record in enumerate(records, 1):
                print(f"{i:3}. {record['map']}" + ('' if record['selectable'] else ' [ambiguous]'))
            choice = input('Map number or package name: ').strip()
            if choice.isdecimal():
                index = int(choice) - 1
                if not 0 <= index < len(records):
                    raise ValueError('Map number is out of range')
                name = records[index]['map']
            else:
                name = choice
        name = select(records, name)
        validate_options(args, name)
        scene = ROOT / 'local' / name[:-2].lower()
        if args.action == 'prepare':
            reject_plain_sanctuary_rebuild(scene, name, args.sanctuary_geometry)
            for command in preparation_commands(args, name, scene):
                subprocess.run(command, check=True, cwd=ROOT)
        else:
            if not args.engine:
                raise ValueError('--engine is required for import and view')
            manifest = scene / 'scene.json'
            if not manifest.is_file():
                raise ValueError(f'Prepare {name} first with --action prepare')
            if json.loads(manifest.read_text(encoding='utf-8')).get('map') != name:
                raise ValueError('Prepared manifest does not match the selected map')
            command = ['powershell.exe', '-NoProfile', '-File', str(ROOT / 'tools/run_ue_level.ps1'),
                       '-Engine', str(args.engine.resolve()), '-Game', str(args.game.resolve()),
                       '-Scene', str(scene), '-ImportOnly' if args.action == 'import' else '-ViewOnly']
            if args.skip_build:
                command.append('-SkipBuild')
            subprocess.run(command, check=True, cwd=ROOT)
    except (ValueError, OSError, subprocess.CalledProcessError, EOFError) as error:
        parser.exit(1, f'viewer: {error}\n')


if __name__ == '__main__':
    main()
