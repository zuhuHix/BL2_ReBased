"""Synthetic tests for tools/check_hud_minimap.py (no game, engine or network; PNGs are generated here)."""
import contextlib
import io
import struct
import sys
import tempfile
import unittest
import zlib
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_hud_minimap as chk  # noqa: E402

CENTRE = (1000.0, -2000.0)
PREFIX = 'LogTemp: Display: '
CYAN, GOLD, BLACK = (60, 170, 220), (240, 160, 30), (0, 0, 0)
N_SPOTS = len(chk.SPOTS)
PNG_SIGNATURE = b'\x89PNG\r\n\x1a\n'


def page(x, y, yaw, map_dx=0.0, rot_delta=0.0, north_dx=0.0, mapw=211.2):
    """A page read-back line, with the clip values the widget would write (2 decimals, as minimap.js does)."""
    e = chk.expected(x, y, yaw, CENTRE)
    mx, my, rot, scale = e['map']
    nx, ny = e['north']
    return (f'OWMINIMAPPAGE x={x} y={y} yaw={yaw} map={mx + map_dx:.2f},{my:.2f},{rot + rot_delta:.2f},{scale:.2f} '
            f'mapw={mapw:.2f} north={nx + north_dx:.2f},{ny:.2f} icons=3')


def state(i):
    x, y, yaw = chk.SPOTS[i]
    return f'OWMINIMAP t={1 + i:.3f} x={x} y={y} yaw={yaw} icons=3'


def shot(i, dx=0):
    x, y, yaw = chk.SPOTS[i]
    return f'OWMINIMAPSHOTS shot OWMinimap_00{i}.png spot {i} at ({x + dx}, {y}, 3700) yaw {yaw}'


def default_pages():
    return [page(x, y, yaw) for x, y, yaw in chk.SPOTS]


def log_text(manifest=True, pages=None, fog=True, shots=None, done=True):
    """A run's log as the host and the page write it; every argument has a passing default."""
    lines = []
    if manifest:
        lines.append(f'{PREFIX}OWMINIMAP manifest local/slice/minimap.json: movie /x, centre (1000.0, -2000.0), 3 icons')
    lines += [f'{PREFIX}{state(i)}' for i in range(N_SPOTS)]
    lines += [f'{PREFIX}{line}' for line in (default_pages() if pages is None else pages)]
    if fog:
        lines.append(f'{PREFIX}OWMINIMAPPAGE fog {{"A_PWDA_0":false}}')
    if shots is None:
        shots = [shot(i) for i in range(N_SPOTS)]
    lines += [f'{PREFIX}{line}' for line in shots]
    if done:
        lines.append(f'{PREFIX}OWMINIMAPSHOTS done')
    return '\n'.join(lines)


def results(text, shots=None):
    return {name: (ok, detail) for name, ok, detail in chk.run_checks(text, shots)}


def failed(text, shots=None):
    return {name for name, (ok, _) in results(text, shots).items() if not ok}


def write_png(path, boxes=(), background=BLACK, size=(1280, 720)):
    """An 8-bit RGB PNG with filter 0 on every row: a background colour and filled boxes [(x0, y0, x1, y1, rgb)]."""
    width, height = size
    plain = b'\0' + bytes(background) * width
    raw = bytearray()
    for y in range(height):
        row = bytearray(plain)
        for x0, y0, x1, y1, rgb in boxes:
            if y0 <= y < y1:
                row[1 + 3 * x0:1 + 3 * x1] = bytes(rgb) * (x1 - x0)
        raw += row

    def chunk(kind, body):
        return struct.pack('>I', len(body)) + kind + body + struct.pack('>I', zlib.crc32(kind + body))
    Path(path).write_bytes(PNG_SIGNATURE + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
                           + chunk(b'IDAT', zlib.compress(bytes(raw))) + chunk(b'IEND', b''))


# Cyan: 30 x 20 = 600 pixels in the minimap box (1110, 25)-(1265, 150), away from the arrow box (1168, 70)-(1202, 110).
# Gold: 10 x 6 = 60 pixels wholly inside the arrow box (the checker needs >= 40 there).
MINIMAP_BOXES = [(1120, 40, 1150, 60, CYAN), (1175, 80, 1185, 86, GOLD)]


def write_set(folder, boxes=None):
    """All eight captures of a run, each either drawn with the minimap or (boxes=None) blank black."""
    for i in range(N_SPOTS):
        write_png(Path(folder) / f'OWMinimap_00{i}.png', boxes=MINIMAP_BOXES if boxes is None else boxes)


class CheckHudMinimapTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Captures are slow to decode (pure-Python PNG filters), so the drawn and blank sets are made once.
        cls.good = tempfile.TemporaryDirectory()
        write_set(cls.good.name)
        cls.blank = tempfile.TemporaryDirectory()
        write_set(cls.blank.name, boxes=[])

    @classmethod
    def tearDownClass(cls):
        cls.good.cleanup()
        cls.blank.cleanup()

    def test_expected_hand_values(self):
        north = {yaw: chk.expected(8109, 2930, yaw, CENTRE)['north'] for yaw in (0, 90, 45)}
        for yaw, want in {0: (-16, -66.8), 90: (-74, -18.8), 45: (-64, -66.8)}.items():
            self.assertAlmostEqual(north[yaw][0], want[0], places=6, msg=f'yaw {yaw}')
            self.assertAlmostEqual(north[yaw][1], want[1], places=6, msg=f'yaw {yaw}')
        # Map: (y - cy) and -(x - cx) in map units at 1.024 scale over 128 uu per unit: 4930 * 0.008, -7109 * 0.008.
        mx, my, rot, scale = chk.expected(8109, 2930, 0, CENTRE)['map']
        self.assertAlmostEqual(mx, -39.44, places=6)
        self.assertAlmostEqual(my, 56.872, places=6)
        self.assertAlmostEqual(rot, 0.0, places=6)
        self.assertAlmostEqual(scale, 102.4, places=6)

    def test_rotation_is_minus_yaw_wrapped_to_half_turn(self):
        self.assertAlmostEqual(chk.expected(8109, 2930, 0, CENTRE)['map'][2], 0.0, places=6)
        self.assertAlmostEqual(chk.expected(8109, 2930, 90, CENTRE)['map'][2], -90.0, places=6)
        self.assertAlmostEqual(chk.expected(8109, 2930, 180, CENTRE)['map'][2], -180.0, places=6)
        self.assertAlmostEqual(chk.expected(8109, 2930, 270, CENTRE)['map'][2], 90.0, places=6)

    def test_passing_log_passes_every_check_without_shots(self):
        found = results(log_text())
        self.assertEqual(len(found), 13)
        self.assertEqual({name for name, (ok, _) in found.items() if not ok}, set())

    def test_no_manifest_fails_manifest_loaded(self):
        self.assertIn('manifest_loaded', failed(log_text(manifest=False)))

    def test_map_off_by_one_unit_fails_map_follows_player(self):
        pages = default_pages()
        pages[3] = page(*chk.SPOTS[3], map_dx=1.0)
        self.assertIn('map_follows_player', failed(log_text(pages=pages)))

    def test_wrong_rotation_fails_map_turns_with_view(self):
        pages = default_pages()
        pages[2] = page(*chk.SPOTS[2], rot_delta=5.0)
        self.assertIn('map_turns_with_view', failed(log_text(pages=pages)))

    def test_north_marker_off_fails_north_on_box_edge(self):
        pages = default_pages()
        pages[0] = page(*chk.SPOTS[0], north_dx=5.0)
        self.assertIn('north_on_box_edge', failed(log_text(pages=pages)))

    def test_readback_of_unlogged_state_fails_echo(self):
        pages = default_pages()
        pages[0] = page(8110, 2930, 0)          # internally consistent, but the host never logged x=8110
        self.assertIn('readbacks_echo_host_state', failed(log_text(pages=pages)))

    def test_map_never_drawn_fails_map_movie_loaded(self):
        pages = [page(x, y, yaw, mapw=0) for x, y, yaw in chk.SPOTS]
        found = failed(log_text(pages=pages))
        self.assertIn('map_movie_loaded', found)
        self.assertIn('icons_shown', found)

    def test_missing_fog_line_fails_fog_initialised(self):
        self.assertIn('fog_initialised', failed(log_text(fog=False)))

    def test_missing_done_fails_shots_logged(self):
        self.assertIn('shots_logged', failed(log_text(done=False)))

    def test_shot_200_uu_away_fails_shots_at_capture_spots(self):
        shots = [shot(i, dx=200 if i == 0 else 0) for i in range(N_SPOTS)]
        self.assertIn('shots_at_capture_spots', failed(log_text(shots=shots)))

    def test_every_shot_of_one_spot_fails_shots_logged(self):
        # Review 1 regression: eight shots, all of spot 0 under distinct files, used to pass both shot checks.
        x, y, yaw = chk.SPOTS[0]
        shots = [f'OWMINIMAPSHOTS shot OWMinimap_00{i}.png spot 0 at ({x}, {y}, 3700) yaw {yaw}' for i in range(N_SPOTS)]
        bad = failed(log_text(shots=shots))
        self.assertIn('shots_logged', bad)
        self.assertNotIn('shots_at_capture_spots', bad)   # each row is at its own spot; the coverage check catches it

    def test_repeated_spot_or_file_fails_shots_logged(self):
        repeated = [shot(i) for i in range(N_SPOTS - 1)] + [shot(0)]
        self.assertIn('shots_logged', failed(log_text(shots=repeated)))
        same_file = [shot(i).replace(f'OWMinimap_00{i}.png', 'OWMinimap_000.png') for i in range(N_SPOTS)]
        self.assertIn('shots_logged', failed(log_text(shots=same_file)))
        self.assertIn('shots_logged', failed(log_text(shots=[shot(i) for i in range(N_SPOTS - 1)])))

    def test_case_alias_file_names_fail_shots_logged(self):
        # Review 2: OWMinimap_000.png and owminimap_000.png are one file on Windows.
        shots = [shot(i) for i in range(N_SPOTS)]
        shots[1] = shots[1].replace('OWMinimap_001.png', 'owminimap_000.png')
        self.assertIn('shots_logged', failed(log_text(shots=shots)))

    def test_missing_pngs_fail_captures_valid(self):
        with tempfile.TemporaryDirectory() as folder:
            found = results(log_text(), folder)
            self.assertFalse(found['captures_valid'][0])
            self.assertFalse(found['captures_show_minimap'][0])
            self.assertIn('0 of 8 complete', found['captures_valid'][1])

    def test_drawn_captures_pass_with_shots(self):
        self.assertEqual(failed(log_text(), self.good.name), set())

    def test_blank_captures_fail_captures_show_minimap(self):
        found = results(log_text(), self.blank.name)
        self.assertTrue(found['captures_valid'][0])
        self.assertFalse(found['captures_show_minimap'][0])

    def test_main_exit_codes(self):
        with tempfile.TemporaryDirectory() as tmp:
            passing = Path(tmp) / 'pass.log'
            passing.write_text(log_text(), encoding='utf-8')
            failing = Path(tmp) / 'fail.log'
            failing.write_text(log_text(manifest=False), encoding='utf-8')
            cases = [
                (['check', str(passing)], 2),                                  # no shots: partial
                (['check', str(passing), '--shots', self.good.name], 0),       # pass
                (['check', str(passing), '--shots', self.blank.name], 1),      # blank captures fail
                (['check', str(failing), '--shots', self.good.name], 1),       # log fails
                (['check', str(failing)], 1),
            ]
            for argv, code in cases:
                with self.subTest(argv=argv), mock.patch.object(sys, 'argv', argv):
                    with contextlib.redirect_stdout(io.StringIO()):
                        self.assertEqual(chk.main(), code)


if __name__ == '__main__':
    unittest.main()
