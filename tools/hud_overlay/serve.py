"""Serve the HUD and menu overlay pages and the locally converted UI movies.

Pages (`index.html` for the HUD, `skills.html` for the skills screen, and
their scripts) come from this directory (our code). Every other path comes
from --movies, the ignored local directory that holds Ruffle (`ruffle/`), the
converted movies (`UI_HUD/HUD.swf`, `UI_StatusMenu/StatusMenu.swf`, their
`harness.swf`, `gfxfontlib.swf`, `SharedWillowComponents/...`) produced by
tools/gfx_to_swf.py and tools/hud_harness_swf.py, and the skill tree data from
tools/prepare_skill_tree.py.

Scaleform movies name other movies as "/ package/<Package>/<Movie>" (seen in a
UI trace for the class portrait); such requests are answered with
`<Package>/<Movie>.swf` from --movies. Binds to localhost only.
"""
import argparse
import functools
import json
import re
import http.server
import urllib.parse
from pathlib import Path

HERE = Path(__file__).parent
PAGES = {'/': 'index.html', '/index.html': 'index.html', '/action_skill.js': 'action_skill.js', '/hud3d.js': 'hud3d.js', '/hud_fields.js': 'hud_fields.js', '/hud_warp.js': 'hud_warp.js', '/minimap.js': 'minimap.js', '/skills.html': 'skills.html', '/skills.js': 'skills.js',
         '/skill_info.js': 'skill_info.js', '/skill_nav.js': 'skill_nav.js', '/skill_rules.js': 'skill_rules.js', '/inventory.html': 'inventory.html',
         '/inventory.js': 'inventory.js', '/menu3d.js': 'menu3d.js', '/glass_sheet.js': 'glass_sheet.js',
         '/missions.html': 'missions.html', '/missions.js': 'missions.js', '/mission_log.js': 'mission_log.js',
         '/item_card.js': 'item_card.js', '/pickup_card.js': 'pickup_card.js', '/pickup_card.html': 'pickup_card.html',
         '/map.html': 'map.html', '/map.js': 'map.js',
         '/mission_screen.html': 'mission_screen.html', '/mission_screen.js': 'mission_screen.js', '/mission_screen_model.js': 'mission_screen_model.js',
         '/vending.html': 'vending.html', '/vending.js': 'vending.js', '/vending_model.js': 'vending_model.js', '/vending_movie.js': 'vending_movie.js', '/movie_guard.js': 'movie_guard.js',
         '/colours.js': 'colours.js'}
TYPES = {'.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8'}
PACKAGE_PREFIX = '/ package/'
# The game's UI colours stay in an ignored local file (tools/game_colours.py, DECISIONS.md 2026-11-08); the pages read
# them as a script so they are there before the page's own scripts run. A missing file answers null (neutral greys).
COLOURS_PATH = '/game_colours.js'
HEX_COLOUR = re.compile(r'^#[0-9A-Fa-f]{6}$')


def colours_script(colours_file):
    """`window.OW_GAME_COLOURS = {key: '#RRGGBB'}` from the file, or null. Values that are not #RRGGBB are dropped
    (colours.js then uses its neutral grey for them); serve.py runs alone in tests, so it does not import tools/."""
    try:
        data = json.loads(Path(colours_file).read_text(encoding='utf-8'))
        found = {k: v for k, v in data['colours'].items() if isinstance(v, str) and HEX_COLOUR.match(v)}
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        found = None
    return ('window.OW_GAME_COLOURS = ' + (json.dumps(found, sort_keys=True) if found else 'null') + ';\n').encode('utf-8')


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, colours=None, **kwargs):
        self.colours = colours
        super().__init__(*args, **kwargs)

    def do_GET(self):
        path = urllib.parse.unquote(self.path.split('?')[0])
        if path in PAGES or path == COLOURS_PATH:
            if path == COLOURS_PATH:
                body, kind = colours_script(self.colours), TYPES['.js']
            else:
                page = HERE / PAGES[path]
                body, kind = page.read_bytes(), TYPES[page.suffix]
            self.send_response(200)
            self.send_header('Content-Type', kind)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if path.startswith(PACKAGE_PREFIX):
            package, _, movie = path[len(PACKAGE_PREFIX):].partition('/')
            self.path = '/' + urllib.parse.quote(f'{package}/{movie}.swf')
        super().do_GET()

    def translate_path(self, path):
        # The stock handler drops '..' but follows symlinks; resolve the file and refuse anything outside the movies root
        # (a name that does not exist there, so the handler answers 404).
        root = Path(self.directory).resolve()
        target = Path(super().translate_path(path)).resolve()
        return str(target) if target == root or target.is_relative_to(root) else str(root / '__ow_outside_the_movies_root__')

    def end_headers(self):
        self.send_header('Cache-Control', 'no-store')
        # Launchers reuse a running server; this tells them whose pages it serves.
        self.send_header('X-OpenWillow-Pages', str(HERE.resolve()))
        super().end_headers()

    def log_message(self, *args):
        pass


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--movies', default=str(Path(__file__).parents[2] / 'local' / 'ui' / 'run'))
    parser.add_argument('--port', type=int, default=8767)
    parser.add_argument('--colours', default=str(Path(__file__).parents[2] / 'local' / 'ui' / 'colours' / 'game_colours.json'),
                        help='the ignored game colours file (tools/game_colours.py); missing means neutral greys')
    args = parser.parse_args()
    movies = Path(args.movies).resolve()
    for need in ('ruffle/ruffle.js', 'UI_HUD/harness.swf', 'UI_HUD/HUD.swf'):
        if not (movies / need).is_file():
            raise SystemExit(f'missing {movies / need}; see DECISIONS.md 2026-09-26 for the conversion steps')
    handler = functools.partial(Handler, directory=str(movies), colours=args.colours)
    if not Path(args.colours).is_file():
        print(f'game colours: {args.colours} missing; the pages use neutral greys')
    print(f'UI overlay on http://127.0.0.1:{args.port}/ serving {movies}')
    http.server.ThreadingHTTPServer(('127.0.0.1', args.port), handler).serve_forever()


if __name__ == '__main__':
    main()
