"""Prepare Marcus's mission screen for the overlay page (tools/hud_overlay/mission_screen.js).

Converts the installed movie `UI_Mission.MissionInterface` (Startup.upk) into `<output>/UI_Mission/MissionInterface.swf` and the
library its reward page imports into `<output>/SharedWillowMission/SharedWillowMission.swf`, writes the wrapper `<output>/UI_Mission/harness.swf` and `<output>/missionscreen.json`: the strings the screen's script uses,
from the install's localization, with the key names of this PC's bindings filled in.

All of it is game data and stays under ignored local/ (docs/LEGAL.md, "UI movies"). Which strings the screen uses, and where, is
recorded in docs/verification/MISSION_SCREEN.md.
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from extract_swfmovie import extract  # noqa: E402
from gfx_to_swf import load_localization  # noqa: E402
from prepare_skill_tree import Package, key_aliases  # noqa: E402
from prepare_slice_world import Reader  # noqa: E402
from prepare_vending_movie import decode_textures, local_path  # noqa: E402

MOVIE = 'UI_Mission.MissionInterface'
LIBRARY = 'SharedWillowMission.SharedWillowMission'   # the reward page's clip, imported by the movie
TOOLS = Path(__file__).parent
STRINGS = {
    'accept': 'willowmenu.board.acceptaccept',
    'decline': 'willowmenu.board.acceptdecline',
    'scroll': 'willowmenu.board.acceptscroll',
    'acceptTooltips': 'willowmenu.board.accepttooltips',
    'complete': 'willowmenu.board.completebuttonbarkb',
    'noChoice': 'willowmenu.board.nochoicebuttonbarkb',
    'acceptReward': 'willowmenu.missionreward.acceptreward',
    'xpRewards': 'willowmenu.missions.xprewards',
    'missionLevel': 'willowmenu.missions.missionlevel',
    'storyMission': 'willowmenu.missions.storymission',
    'optionalMission': 'willowmenu.missions.optionalmission',
    'missionComplete': 'willowmenu.missions.missioncomplete',
    'rewardsTitle': 'willowmenu.missions.rewardstitle',
}


def strings_from(table, aliases):
    """The named strings with `<StringAliasMap:NAME>` replaced by the key's label. A string or alias the install lacks is an error:
    a prompt with a hole in it is worse than a failed preparation."""
    out = {}
    for name, key in STRINGS.items():
        if key not in table:
            raise KeyError(f'{key} is not in the localization')
        text = table[key]
        for alias in set(part.split('>', 1)[0] for part in text.split('<StringAliasMap:')[1:]):
            if alias not in aliases:
                raise KeyError(f'{name}: no key label for {alias}')
            text = text.replace(f'<StringAliasMap:{alias}>', aliases[alias])
        out[name] = text
    return out


def convert(gfx, textures, swf, localization):
    subprocess.run([sys.executable, str(TOOLS / 'gfx_to_swf.py'), '--gfx', str(gfx), '--textures', str(textures), '--localization', str(localization),
                    '--inline-font-imports', '--output', str(swf)], check=True, capture_output=True, text=True)
    return json.loads(swf.with_suffix('.json').read_text(encoding='utf-8'))


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--output', default='local/ui/run', help='overlay movie folder (serve.py --movies)')
    args = parser.parse_args()
    try:
        output = local_path(args.output)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    reader = Reader(args.reader, args.game, None)
    package = Package(str(Path(args.reader).resolve()), reader.cooked / 'Startup.upk')
    localization = Path(args.game) / 'WillowGame' / 'Localization' / 'INT'
    unresolved = {}
    for movie in (MOVIE, LIBRARY):
        owner, name = movie.split('.', 1)
        gfx, textures, swf = output.parent / 'gfx' / owner / f'{name}.gfx', output.parent / 'tex' / owner, output / owner / f'{name}.swf'
        for folder in (gfx.parent, textures, swf.parent):
            folder.mkdir(parents=True, exist_ok=True)
        data, _ = extract(package.reader, package.path, movie)
        gfx.write_bytes(data)
        report = convert(gfx, textures, swf, localization)
        unresolved[movie] = decode_textures(package, movie, report, textures)
        report = convert(gfx, textures, swf, localization)
    # Ruffle's nested-import stop (docs/verification/INVENTORY_MOVIE_PROTOTYPE.md): without this the reward clip never appears.
    library = output / 'SharedWillowMission' / 'SharedWillowMission.swf'
    subprocess.run([sys.executable, str(TOOLS / 'prepare_inventory_movie.py'), '--input', str(library), '--output', str(library)], check=True)
    swf = output / 'UI_Mission' / 'MissionInterface.swf'
    subprocess.run([sys.executable, str(TOOLS / 'hud_harness_swf.py'), '--movie', swf.name, '--output', str(swf.with_name('harness.swf'))], check=True)
    table = load_localization(localization)
    strings = strings_from(table, key_aliases(args.game, localization))
    (output / 'missionscreen.json').write_text(json.dumps({'strings': strings}, indent=1), encoding='utf-8')
    print(json.dumps({'swf': str(swf), 'bytes': swf.stat().st_size, 'images_missing_after_decode': unresolved}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
