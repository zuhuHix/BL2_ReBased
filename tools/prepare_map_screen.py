"""Write the Map screen's local strings (local/ui/run/map.json) from the installed game.

The StatusMenu movie's Map frame shows its hint bar from the token "$<StringAliasMap:map_bar>", which the game resolves
through DefaultGame.ini's MenuInputMapArray (PC set: <Strings:WillowMenu.mapbar.ButtonBarKB>) and the localization; the
key names inside that string are <StringAliasMap:X> tokens again. StatusMenuExGFxMovie.SetTooltipsText sets nothing for
the Map screen, so this is the text the bar shows. Output is game data: it is written only inside the repository's ignored
local/ folder (as tools/prepare_mission_log.py does).

  python tools/prepare_map_screen.py --game "<Borderlands 2 folder>"
"""
import argparse
import json
import os
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO / 'tools'))
from gfx_to_swf import load_localization  # noqa: E402
from prepare_skill_tree import ALIAS, key_aliases  # noqa: E402

STRINGS = re.compile(r'<Strings:([^>]+)>')


def resolve(text, strings, aliases, depth=0):
    """Expands <StringAliasMap:X> through `aliases` and <Strings:File.Section.Key> through `strings` (lower-case keys),
    recursively; an unknown token is kept as it is."""
    if depth > 4:
        return text
    text = ALIAS.sub(lambda m: aliases.get(m.group(1), m.group(0)), text)
    text = STRINGS.sub(lambda m: strings.get(m.group(1).lower(), m.group(0)), text)
    if ALIAS.search(text) or any(m.group(1).lower() in strings for m in STRINGS.finditer(text)):
        return resolve(text, strings, aliases, depth + 1)
    return text.replace('\\n', '\n')


def build(game):
    localization = Path(game) / 'WillowGame/Localization/INT'
    strings = load_localization(localization)
    aliases = key_aliases(Path(game), localization)
    return {'strings': {'bar': resolve('<StringAliasMap:map_bar>', strings, aliases)}}


def output_file(output, repo=REPO):
    """<output>/map.json, resolved, refused unless it lies inside the repository's ignored local/ folder."""
    local = (Path(repo) / 'local').resolve()
    target = (Path(output) / 'map.json').resolve()
    if not target.is_relative_to(local):
        raise SystemExit(f'refusing to write {target}: the output must be inside {local}')
    return target


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--game', default=os.environ.get('OPENWILLOW_BL2'))
    parser.add_argument('--output', default=str(REPO / 'local/ui/run'))
    args = parser.parse_args()
    if not args.game:
        raise SystemExit('pass --game <Borderlands 2 folder> or set OPENWILLOW_BL2')
    target = output_file(args.output)   # checked before any game data is read
    data = build(args.game)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(data, indent=1, ensure_ascii=False), encoding='utf-8')
    print(f'wrote {target}')


if __name__ == '__main__':
    main()
