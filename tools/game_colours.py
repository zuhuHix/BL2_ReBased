"""The game's UI colours, kept out of the repository (DECISIONS.md 2026-11-08).

Exact colours taken from Borderlands 2 (rarity tiers, card and menu text tints, the
skill info box) are game-derived data, so they live in the ignored file
local/ui/colours/game_colours.json, which each machine generates or receives with
tools/private_sync.ps1 (it mirrors local/ui/colours). The code ships only neutral greys
that are plainly not the game's; they are used, with one warning, when the file is missing.

    python tools/game_colours.py --reader build/Release/ow-package.exe \
        --package "<BL2>/WillowGame/CookedPCConsole/Startup.upk" [--seed <notes.json>]

writes the file: the rarity tiers are read from the install (GlobalsDefinition
RarityLevelColors) unless --seed names them; every other key comes from --seed, a private notes file of
{"key": "#RRGGBB"} measured on or traced from the real game. Keys the run cannot fill
keep the value already in the file. Only key names and counts are printed, never values.
The overlay pages get the same file through serve.py (/game_colours.js) and tools/hud_overlay/colours.js.
"""
import argparse
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PATH = ROOT / 'local' / 'ui' / 'colours' / 'game_colours.json'
HEX = re.compile(r'^#[0-9A-Fa-f]{6}$')

# Every key the code reads, with its neutral stand-in (greys and white, chosen to look
# obviously unstyled, not taken from the game). Keep in step with tools/hud_overlay/colours.js.
NEUTRAL = {
    'rarity_1': '#FFFFFF', 'rarity_2': '#E0E0E0', 'rarity_3': '#C0C0C0', 'rarity_4': '#A0A0A0', 'rarity_5': '#808080',
    'card_text': '#D0D0D0',             # item card labels and values, menu hint text
    'card_projectile_count': '#B0B0B0', # the "xN" after a shotgun's damage
    'card_red_text': '#909090',         # a title part's flavour line; also how weapon_stats.py recognises it
    'hint_disabled': '#707070',         # a menu hint that does nothing right now
    'list_header_text': '#F0F0F0',      # inventory list headers and the storage plate
    'inventory_band': '#909090',        # the selected row's band
    'inventory_tile_fill': '#A0A0A0',   # the backpack tile wash
    'inventory_tile_frame': '#B0B0B0',  # the compare card's frame
    'skills_text': '#C8C8C8',           # skills page text without its own colour
    'skill_current': '#B0B0B0',         # info box number below max grade
    'skill_maxed': '#E0E0E0',           # info box number at max grade
    'skill_locked': '#808080',          # info box text while the tier is locked
    'skill_class_mod': '#F0F0F0',       # info box class-mod note
    'skill_tag': '#D8D8D8',             # [skill]..[-skill] words in skill descriptions
}

# GlobalsDefinition.RarityLevelColors RarityRating -> key (as weapon_stats.RARITY_NUMBER).
RARITY_KEYS = {'RARITY_Common': 'rarity_1', 'RARITY_Uncommon': 'rarity_2', 'RARITY_Rare': 'rarity_3',
               'RARITY_VeryRare': 'rarity_4', 'RARITY_Legendary': 'rarity_5'}
GLOBALS = 'GD_Globals.General.Globals'
SCHEMA_LINES = ['RarityLevelColors=StructProperty:RarityLevelColor']

_loaded = {}


def path():
    return Path(os.environ.get('OPENWILLOW_GAME_COLOURS') or DEFAULT_PATH)


def valid(colours):
    """Only known keys with #RRGGBB values (upper case)."""
    if not isinstance(colours, dict):
        return {}
    return {key: value.upper() for key, value in colours.items()
            if key in NEUTRAL and isinstance(value, str) and HEX.match(value)}


def read_file(file):
    try:
        data = json.loads(Path(file).read_text(encoding='utf-8'))
    except (OSError, ValueError):
        return None
    return valid(data.get('colours') if isinstance(data, dict) else None)


def load():
    """The local file's colours over the neutral ones; warns once on stderr when the file is missing."""
    file = path()
    key = str(file)
    if key not in _loaded:
        found = read_file(file)
        if found is None:
            print(f'game colours: {file} missing or unreadable; using neutral greys '
                  '(tools/game_colours.py, DECISIONS.md 2026-11-08)', file=sys.stderr)
        _loaded[key] = {**NEUTRAL, **(found or {})}
    return _loaded[key]


def colour(key):
    """'#RRGGBB' for one key."""
    return load()[key]


def rgb(key):
    value = colour(key)
    return {'R': int(value[1:3], 16), 'G': int(value[3:5], 16), 'B': int(value[5:7], 16)}


def rarity_from_package(package):
    """{rarity_N: '#RRGGBB'} from the first RarityLevelColors row of each rating."""
    found = {}
    for row in package.props(GLOBALS).get('RarityLevelColors') or []:
        key = RARITY_KEYS.get(row.get('RarityRating'))
        color = row.get('Color') or {}
        if key and key not in found and all(isinstance(color.get(c), int) for c in 'RGB'):
            found[key] = '#%02X%02X%02X' % tuple(color[c] for c in 'RGB')
    return found


def generate(output, package=None, seed=None):
    """Write the file; returns {key: source} for every key filled by this run."""
    current = read_file(output) or {}
    sources = {}
    merged = dict(current)
    # The package fills the rarity tiers; a seed value wins over it (the maintainer's explicit notes).
    if package is not None:
        values = rarity_from_package(package)
        merged.update(values)
        sources.update({key: f'package {GLOBALS}.RarityLevelColors' for key in values})
    if seed is not None:
        values = read_file(seed)
        if values is None:
            raise SystemExit(f'seed {seed} is missing or not {{"colours": {{...}}}} JSON')
        merged.update(values)
        sources.update({key: 'seed' for key in values})
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps({'schemaVersion': 1, 'colours': dict(sorted(merged.items())),
                                  'note': 'Game-derived; never commit (DECISIONS.md 2026-11-08).'}, indent=1) + '\n',
                      encoding='utf-8')
    return sources, sorted(set(NEUTRAL) - set(merged))


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', help='ow-package executable (with --package)')
    parser.add_argument('--package', help='the install\'s Startup.upk (holds GD_Globals)')
    parser.add_argument('--seed', type=Path, help='private {"colours": {key: "#RRGGBB"}} notes for the measured keys')
    parser.add_argument('--output', type=Path, default=DEFAULT_PATH)
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to((ROOT / 'local').resolve()):
        parser.error("output must stay under this checkout's ignored local/")
    if bool(args.reader) != bool(args.package):
        parser.error('--reader and --package go together')
    package = None
    if args.package:
        sys.path.insert(0, str(Path(__file__).parent))
        import skill_stats
        schema = args.output.parent / 'game_colours.schema'
        schema.parent.mkdir(parents=True, exist_ok=True)
        schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
        package = skill_stats.Package(str(Path(args.reader).resolve()), Path(args.package), str(schema.resolve()))
    sources, missing = generate(args.output, package, args.seed)
    print(json.dumps({'output': str(args.output), 'filled': {k: sources[k] for k in sorted(sources)},
                      'still_neutral': missing}, indent=1))


if __name__ == '__main__':
    main()
