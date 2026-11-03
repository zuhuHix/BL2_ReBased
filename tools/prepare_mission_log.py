"""Prepare the mission log's presentation data for tools/hud_overlay/missions.js.

Reads, from the installed game, through `ow-package --object-dump` and the
install's localization (tools/gfx_to_swf.load_localization):
- `UI_StatusMenu.StatusMenu_Definition` (Startup.upk): the list icon frame and
  text colour per mission status (Pres_NotStarted, Pres_Active, ...);
- `UI_StatusMenu.GFxTextList_Missions`: the list container's row count, the
  normal/highlight text colours, the category (divider) prefix/postfix and icon;
- `GD_Globals.General.Globals`: the LevelDifference_* thresholds that pick the
  difficulty bar frame;
- the strings StatusMenuExGFxMovie and the missions panel use (WillowGame.int
  [StatusMenuExGFxMovie], WillowMenu.int [missions]).

Writes `<output>/missionlog.json` (default local/ui/run). It is game data and
stays local (docs/LEGAL.md). Which values the script uses and how is recorded in
docs/verification/MENUS_PARITY.md.
"""
import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from gfx_to_swf import load_localization  # noqa: E402
from prepare_skill_tree import ALIAS, key_aliases  # noqa: E402

REPO = Path(__file__).resolve().parents[1]
OBJECTS = {
    'status': 'UI_StatusMenu.StatusMenu_Definition',
    'list': 'UI_StatusMenu.GFxTextList_Missions',
    'globals': 'GD_Globals.General.Globals',
}
# StatusMenuGFxDefinition presentation fields, by the status names missions.js uses.
PRESENTATION = {
    'notstarted': 'pres_notstarted', 'active': 'pres_active', 'activeplot': 'pres_activeplot',
    'turnin': 'pres_readytoturnin', 'complete': 'pres_complete', 'failed': 'pres_failed',
    'ineligible': 'pres_ineligible',
}
DIFFICULTY = ('impossible', 'hard', 'tough', 'normal')
STRINGS = {
    'levelString': 'willowgame.statusmenuexgfxmovie.levelstring',
    'myMissions': 'willowgame.statusmenuexgfxmovie.missionlogmymissions',
    'noMissions': 'willowgame.statusmenuexgfxmovie.nomissionsstring',
    'sortAll': 'willowgame.statusmenuexgfxmovie.sortall',
    'tooltipCancel': 'willowgame.statusmenuexgfxmovie.missionlogtooltip_cancel',
    'tooltipTrack': 'willowgame.statusmenuexgfxmovie.missionlogtooltip_track',
    'tooltipScroll': 'willowgame.statusmenuexgfxmovie.missionlogtooltip_scroll',
    'tooltipSort': 'willowgame.statusmenuexgfxmovie.missionlogtooltip_sort',
    'xpRewards': 'willowmenu.missions.xprewards',
    'missionLevel': 'willowmenu.missions.missionlevel',
    'storyMission': 'willowmenu.missions.storymission',
    'optionalMission': 'willowmenu.missions.optionalmission',
    'dividerInProgress': 'willowmenu.missions.divider_inprogress',
    'dividerTurnIn': 'willowmenu.missions.divider_turnin',
    'dividerNew': 'willowmenu.missions.divider_new',
    'dividerCompleted': 'willowmenu.missions.divider_completed',
    'dividerFailed': 'willowmenu.missions.divider_failed',
    # GetMissionDescriptionForUI's pieces.
    'colonAndSpace': 'willowgame.missions.colonandspace',
    'objectiveComplete': 'willowgame.missions.singlecountobjectivecomplete',
    'or': 'willowmenu.missions.or',
    'turnInHud': 'willowmenu.missions.divider_turninhud',
    'briefingTurnIn': 'willowmenu.missions.briefing_turnin',
}


def hex_color(color):
    return '#%02X%02X%02X' % (color['R'], color['G'], color['B'])


def dump(reader, cooked, index):
    out = subprocess.run([reader, str(cooked / 'Startup.upk'), '--object-dump', str(index), '4', '--cooked', str(cooked), '--all'],
                         check=True, capture_output=True, text=True).stdout
    return json.loads(out)['properties']


def build(reader, cooked, localization, game):
    exports = json.loads(subprocess.run([reader, str(cooked / 'Startup.upk'), '--exports'], check=True,
                                        capture_output=True, text=True).stdout)
    index = {e['path']: e['index'] for e in exports}
    missing = [path for path in OBJECTS.values() if path not in index]
    if missing:
        raise SystemExit(f'not in Startup.upk: {", ".join(missing)}')
    status, rows, globals_ = (dump(reader, cooked, index[OBJECTS[key]]) for key in ('status', 'list', 'globals'))
    strings = load_localization(localization)
    aliases = key_aliases(game, localization)
    absent = [key for key, name in STRINGS.items() if name not in strings]
    if absent:
        raise SystemExit(f'localization lacks: {", ".join(absent)}')
    return {
        'source': {'definition': OBJECTS['status'], 'list': OBJECTS['list'], 'globals': OBJECTS['globals']},
        'presentation': {name: {'frame': status[field]['IconFrame'], 'color': hex_color(status[field]['TextColor'])}
                         for name, field in PRESENTATION.items()},
        'list': {
            'container': rows['containername'], 'entries': rows['initialentrycount'],
            'normalColor': hex_color(rows['textnormalcolor']), 'highlightColor': hex_color(rows['texthighlightcolor']),
            'categoryPrefix': rows['categoryprefix'], 'categoryPostfix': rows['categorypostfix'],
            'categoryIcon': rows['categoryiconlabel'], 'moreUp': rows['moreupname'], 'moreDown': rows['moredownname'],
        },
        # Level difference (mission level minus player level) at or above which each frame applies; below 'normal' it is 'trivial'.
        'difficulty': {name: globals_[f'leveldifference_{name}'] for name in DIFFICULTY},
        # <StringAliasMap:X> key tokens resolved to the PC text (DefaultGame.ini MenuInputMapArray), as the hint bar shows them.
        'strings': {key: ALIAS.sub(lambda m: aliases.get(m.group(1), m.group(0)), strings[name]).replace('\\n', '\n')
                    for key, name in STRINGS.items()},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(REPO / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', default=os.environ.get('OPENWILLOW_BL2'))
    parser.add_argument('--output', default=str(REPO / 'local/ui/run'))
    args = parser.parse_args()
    if not args.game:
        raise SystemExit('pass --game <Borderlands 2 folder> or set OPENWILLOW_BL2')
    game = Path(args.game)
    data = build(args.reader, game / 'WillowGame/CookedPCConsole', game / 'WillowGame/Localization/INT', game)
    output = Path(args.output)
    output.mkdir(parents=True, exist_ok=True)
    (output / 'missionlog.json').write_text(json.dumps(data, indent=1, ensure_ascii=False), encoding='utf-8')
    print(f'wrote {output / "missionlog.json"}: {len(data["presentation"])} presentations, {len(data["strings"])} strings')


if __name__ == '__main__':
    main()
