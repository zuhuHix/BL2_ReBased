"""Synthetic tests for tools/game_colours.py and serve.py's /game_colours.js (made-up colours only)."""
import contextlib
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
sys.path.insert(0, str(ROOT / 'tools' / 'hud_overlay'))
import game_colours  # noqa: E402
import serve  # noqa: E402

GLOBALS = game_colours.GLOBALS


class FakePackage:
    def __init__(self, rows):
        self.rows = rows

    def props(self, path):
        assert path == GLOBALS
        return {'RarityLevelColors': self.rows}


def ensure_local(case):
    """This checkout's local/; a clean checkout has none yet, so make it and remove it after the test if it is empty."""
    local = ROOT / 'local'
    if not local.is_dir():
        local.mkdir()
        case.addCleanup(lambda: local.rmdir() if not any(local.iterdir()) else None)
    return local


def make_outside_junction(case, target):
    """A junction inside this checkout's local/ pointing at `target` (outside it), removed after the test; None off Windows."""
    try:
        import _winapi
    except ImportError:
        return None
    link = ensure_local(case) / f'ow_test_junction_{os.getpid()}'
    _winapi.CreateJunction(str(target), str(link))
    case.addCleanup(os.rmdir, link)  # removes the junction only, never the target's contents
    return link


def row(rating, r, g, b):
    return {'RarityRating': rating, 'Color': {'R': r, 'G': g, 'B': b, 'A': 255}}


class GameColoursTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.saved = os.environ.get('OPENWILLOW_GAME_COLOURS')

    def tearDown(self):
        if self.saved is None:
            os.environ.pop('OPENWILLOW_GAME_COLOURS', None)
        else:
            os.environ['OPENWILLOW_GAME_COLOURS'] = self.saved
        self.tmp.cleanup()

    def test_rarity_from_package_takes_the_first_row_of_each_rating(self):
        package = FakePackage([row('RARITY_Common', 1, 2, 3), row('RARITY_Common', 9, 9, 9),
                               row('RARITY_Uncommon', 16, 32, 48), row('RARITY_Bogus', 7, 7, 7), {'RarityRating': 'RARITY_Rare'}])
        self.assertEqual(game_colours.rarity_from_package(package), {'rarity_1': '#010203', 'rarity_2': '#102030'})

    def test_generate_merges_seed_package_and_keeps_earlier_values(self):
        out = self.dir / 'game_colours.json'
        out.write_text(json.dumps({'colours': {'skill_tag': '#ABCDEF', 'rarity_1': '#000001'}}), encoding='utf-8')
        seed = self.dir / 'seed.json'
        seed.write_text(json.dumps({'colours': {'card_text': '#123456', 'rarity_2': '#000002', 'not_a_key': '#111111',
                                                'skill_maxed': 'green'}}), encoding='utf-8')
        sources, missing = game_colours.generate(out, FakePackage([row('RARITY_Uncommon', 0, 0, 3), row('RARITY_Rare', 0, 0, 4)]), seed)
        written = json.loads(out.read_text(encoding='utf-8'))['colours']
        self.assertEqual(written, {'card_text': '#123456', 'rarity_1': '#000001', 'rarity_2': '#000002', 'rarity_3': '#000004',
                                   'skill_tag': '#ABCDEF'})
        self.assertEqual(sources['card_text'], 'seed')
        self.assertEqual(sources['rarity_2'], 'seed')  # a seed value wins over the package
        self.assertTrue(sources['rarity_3'].startswith('package '))
        self.assertIn('skill_maxed', missing)
        self.assertNotIn('not_a_key', written)

    def test_load_falls_back_to_neutral_and_warns_once(self):
        os.environ['OPENWILLOW_GAME_COLOURS'] = str(self.dir / 'absent.json')
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.assertEqual(game_colours.colour('card_red_text'), game_colours.NEUTRAL['card_red_text'])
            game_colours.colour('skill_tag')
        self.assertEqual(err.getvalue().count('neutral greys'), 1)

    def test_a_json_file_of_the_wrong_shape_is_unreadable_and_warns_once(self):
        for index, body in enumerate(([], {'colours': ['#010101']}, {'not_colours': {}}, {'colours': 'x'})):
            file = self.dir / f'shape{index}.json'
            file.write_text(json.dumps(body), encoding='utf-8')
            self.assertIsNone(game_colours.read_file(file), body)
            os.environ['OPENWILLOW_GAME_COLOURS'] = str(file)
            err = io.StringIO()
            with contextlib.redirect_stderr(err):
                self.assertEqual(game_colours.colour('card_text'), game_colours.NEUTRAL['card_text'])
                game_colours.colour('skill_tag')
            self.assertEqual(err.getvalue().count('missing or unreadable'), 1, body)

    def test_rarity_fallbacks_are_one_grey_unlike_a_made_up_file(self):
        tiers = {game_colours.NEUTRAL[f'rarity_{n}'] for n in range(1, 6)}
        self.assertEqual(len(tiers), 1)
        made_up = {f'rarity_{n}': '#0%d0%d0%d' % (n, n, n) for n in range(1, 6)}
        self.assertTrue(tiers.isdisjoint(made_up.values()))

    def test_under_local_follows_links(self):
        self.assertTrue(game_colours.under_local(ROOT / 'local' / 'ui' / 'colours' / 'x.json'))
        self.assertFalse(game_colours.under_local(self.dir / 'x.json'))
        self.assertFalse(game_colours.under_local(ROOT / 'local' / '..' / 'x.json'))
        link = make_outside_junction(self, self.dir)
        if link:
            self.assertFalse(game_colours.under_local(link / 'x.json'))

    def test_load_reads_the_file_over_neutral(self):
        file = self.dir / 'present.json'
        file.write_text(json.dumps({'colours': {'card_red_text': '#0F1E2D'}}), encoding='utf-8')
        os.environ['OPENWILLOW_GAME_COLOURS'] = str(file)
        self.assertEqual(game_colours.rgb('card_red_text'), {'R': 15, 'G': 30, 'B': 45})
        self.assertEqual(game_colours.colour('skill_tag'), game_colours.NEUTRAL['skill_tag'])

    def test_neutral_values_are_greys(self):
        for key, value in game_colours.NEUTRAL.items():
            self.assertTrue(value[1:3] == value[3:5] == value[5:7], f'{key} {value} is not a grey')

    def test_js_and_python_share_keys_and_neutral_values(self):
        js = (ROOT / 'tools' / 'hud_overlay' / 'colours.js').read_text(encoding='utf-8')
        block = js[js.index('const NEUTRAL = {'):js.index('};', js.index('const NEUTRAL = {'))]
        found = dict(re.findall(r"(\w+): '(#[0-9A-Fa-f]{6})'", block))
        self.assertEqual(found, game_colours.NEUTRAL)

    def test_serve_script_answers_null_or_the_valid_values(self):
        self.assertEqual(serve.colours_script(self.dir / 'absent.json'), b'window.OW_GAME_COLOURS = null;\n')
        self.assertEqual(serve.colours_script(None), b'window.OW_GAME_COLOURS = null;\n')
        file = self.dir / 'c.json'
        file.write_text(json.dumps({'colours': {'card_text': '#010101', 'bad': '</script>'}}), encoding='utf-8')
        self.assertEqual(serve.colours_script(file), b'window.OW_GAME_COLOURS = {"card_text": "#010101"};\n')

    def test_prepare_skill_tree_output_must_stay_under_local(self):
        script = [sys.executable, str(ROOT / 'tools' / 'prepare_skill_tree.py'), '--reader', 'none.exe', '--game', str(self.dir / 'no_game')]
        outside = subprocess.run(script + ['--output', str(self.dir / 'run')], capture_output=True, text=True, cwd=ROOT)
        self.assertEqual(outside.returncode, 2)
        self.assertIn("outside this checkout's ignored local/", outside.stderr)
        self.assertFalse((self.dir / 'run').exists())
        link = make_outside_junction(self, self.dir)
        if link:
            through = subprocess.run(script + ['--output', str(link / 'run')], capture_output=True, text=True, cwd=ROOT)
            self.assertEqual(through.returncode, 2)
            self.assertIn("outside this checkout's ignored local/", through.stderr)
        scratch = Path(tempfile.mkdtemp(prefix='ow_test_', dir=ensure_local(self)))  # the tool writes a schema next to --output
        self.addCleanup(shutil.rmtree, scratch, True)
        inside = subprocess.run(script + ['--output', str(scratch / 'run')], capture_output=True, text=True, cwd=ROOT)
        self.assertNotIn("outside this checkout's ignored local/", inside.stderr)  # past the guard; fails later on the fake game

    def test_output_must_stay_under_local(self):
        result = subprocess.run([sys.executable, str(ROOT / 'tools' / 'game_colours.py'), '--output', str(self.dir / 'x.json')],
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('local/', result.stderr)


if __name__ == '__main__':
    unittest.main()
