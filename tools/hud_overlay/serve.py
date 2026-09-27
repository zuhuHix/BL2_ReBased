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
import http.server
import urllib.parse
from pathlib import Path

HERE = Path(__file__).parent
PAGES = {'/': 'index.html', '/index.html': 'index.html', '/skills.html': 'skills.html', '/skills.js': 'skills.js',
         '/skill_info.js': 'skill_info.js'}
TYPES = {'.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8'}
PACKAGE_PREFIX = '/ package/'


class Handler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        path = urllib.parse.unquote(self.path.split('?')[0])
        if path in PAGES:
            page = HERE / PAGES[path]
            body = page.read_bytes()
            self.send_response(200)
            self.send_header('Content-Type', TYPES[page.suffix])
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if path.startswith(PACKAGE_PREFIX):
            package, _, movie = path[len(PACKAGE_PREFIX):].partition('/')
            self.path = '/' + urllib.parse.quote(f'{package}/{movie}.swf')
        super().do_GET()

    def end_headers(self):
        self.send_header('Cache-Control', 'no-store')
        super().end_headers()

    def log_message(self, *args):
        pass


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--movies', default=str(Path(__file__).parents[2] / 'local' / 'ui' / 'run'))
    parser.add_argument('--port', type=int, default=8767)
    args = parser.parse_args()
    movies = Path(args.movies).resolve()
    for need in ('ruffle/ruffle.js', 'UI_HUD/harness.swf', 'UI_HUD/HUD.swf'):
        if not (movies / need).is_file():
            raise SystemExit(f'missing {movies / need}; see DECISIONS.md 2026-09-26 for the conversion steps')
    handler = functools.partial(Handler, directory=str(movies))
    print(f'UI overlay on http://127.0.0.1:{args.port}/ serving {movies}')
    http.server.ThreadingHTTPServer(('127.0.0.1', args.port), handler).serve_forever()


if __name__ == '__main__':
    main()
