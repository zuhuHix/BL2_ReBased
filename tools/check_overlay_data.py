#!/usr/bin/env python3
"""Check that the converted movies and data the overlay pages load are present under local/ui/run.

The pages carry on without a missing movie (tools/hud_overlay/movie_guard.js), and a host suite can pass without
one, so a run says what it is missing here. Exit 1 when something is missing. Prints file names only.
"""
import argparse
import sys
from pathlib import Path

# File, relative to the movies folder; what needs it; how to get it.
UI_STEPS = 'DECISIONS.md 2026-09-26 (tools/gfx_to_swf.py, tools/hud_harness_swf.py)'
PICKUP_STEPS = 'docs/verification/PICKUP_CARD.md "Local data"'
REQUIRED = [
    ('ruffle/ruffle.js', 'every page', UI_STEPS),
    ('UI_HUD/harness.swf', 'HUD', UI_STEPS),
    ('UI_HUD/HUD.swf', 'HUD', UI_STEPS),
    ('UI_StatusMenu/harness.swf', 'Inventory, Skills, Missions, Map', UI_STEPS),
    ('UI_StatusMenu/StatusMenu.swf', 'Inventory, Skills, Missions, Map', UI_STEPS),
    ('UI_Inworld/harness.swf', 'world pickup card', PICKUP_STEPS),
    ('UI_Inworld/Weapon_Card.swf', 'world pickup card', PICKUP_STEPS),
    ('UI_VendingMachine/VendingMachineInterface.swf', 'vending frame art (optional: the page works without it)', UI_STEPS),
    ('map.json', 'Map names', 'python tools/prepare_map_screen.py'),
]


def missing(movies):
    return [entry for entry in REQUIRED if not (Path(movies) / entry[0]).is_file()]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--movies', default=str(Path(__file__).parents[1] / 'local' / 'ui' / 'run'))
    parser.add_argument('--fail-on', action='append', default=None, metavar='PREFIX',
                        help='exit 1 only for missing files under this prefix (a suite names what it depends on); default: any')
    parser.add_argument('--warn-only', action='store_true', help='print what is missing, exit 0')
    args = parser.parse_args(argv)
    absent = missing(args.movies)
    for name, needed_by, how in absent:
        print(f'OVERLAY DATA MISSING: {name} (needed by: {needed_by}; get it: {how})')
    if not absent:
        print(f'Overlay data: all {len(REQUIRED)} files present')
    fatal = [name for name, _, _ in absent if args.fail_on is None or any(name.startswith(prefix) for prefix in args.fail_on)]
    return 1 if fatal and not args.warn_only else 0


if __name__ == '__main__':
    sys.exit(main())
