"""Write the HUD movie's own localized strings for the HUD overlay page.

The game's HUD script fills some of the HUD movie's text fields from properties of WillowHUDGFxMovie and of the mission
widget, which the install's English localization file holds (`WillowGame/Localization/INT/WillowGame.int`, UTF-16):
the down-state title and hint, the second-wind banner, the tracker's turn-in label. Which field each one fills is
native and was not observed in the running game (UNVERIFIED; docs/verification/HUD_PARITY.md, "Fight For Your Life").

This copies those values into `<output>/UI_HUD/hud_strings.json`, which tools/hud_overlay/serve.py serves next to the
movie and tools/hud_overlay/index.html loads. Game text: the output stays under ignored local/ (docs/LEGAL.md); a page
without the file leaves those fields as the movie has them.
"""
import argparse
import json
import sys
from pathlib import Path

# (json key, .int section, .int key)
STRINGS = (
    ('injured', 'WillowHUDGFxMovie', 'PlayerInjuredString'),
    ('reviving', 'WillowHUDGFxMovie', 'PlayerRevivingString'),
    ('secondWind', 'WillowHUDGFxMovie', 'SecondWindString'),
    ('injuredTip', 'WillowHUDGFxMovie', 'InjuredTooltipString1'),
    ('turnIn', 'HUDWidget_Missions', 'MissionTurnInString'),
)


def read_sections(path):
    """{section: {key: value}} of a UTF-16 .int file; the first value of a key wins."""
    sections, current = {}, None
    for line in Path(path).read_text(encoding='utf-16').splitlines():
        if line.startswith('['):
            current = sections.setdefault(line.strip()[1:-1], {})
        elif current is not None and '=' in line:
            key, value = line.split('=', 1)
            current.setdefault(key.strip(), value.strip())
    return sections


def hud_strings(sections):
    out, missing = {}, []
    for name, section, key in STRINGS:
        value = sections.get(section, {}).get(key)
        if value is None:
            missing.append(f'[{section}] {key}')
        else:
            out[name] = value
    return out, missing


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--output', default='local/ui/run')
    args = parser.parse_args()
    source = Path(args.game) / 'WillowGame' / 'Localization' / 'INT' / 'WillowGame.int'
    strings, missing = hud_strings(read_sections(source))
    if missing:
        print('missing in ' + str(source) + ': ' + ', '.join(missing), file=sys.stderr)
        return 1
    target = Path(args.output) / 'UI_HUD' / 'hud_strings.json'
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(strings, ensure_ascii=False, indent=1), encoding='utf-8')
    print(f'wrote {target} ({len(strings)} strings)')
    return 0


if __name__ == '__main__':
    sys.exit(main())
