"""Synthetic tests for tools/check_hud_shots.py (no game or engine needed; logs and PNGs are generated here)."""
import struct
import sys
import tempfile
import unittest
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_hud_shots as chk  # noqa: E402

NB = '\\u00a0'      # the host's JSON escape for a no-break space, as written in the log
MOVIE = 'LogTemp: Display: OpenWillow HUD path: movie (page reported ready)'
WARP = 'LogTemp: Display: OpenWillow Flash HUD console: OpenWillow HUD 3D path warp: Ruffle webgl2 (http://127.0.0.1:8795/hud_warp.js:311)'
CANVAS = 'LogTemp: Display: OpenWillow HUD path: canvas (page not ready)'
AFFINE = 'LogTemp: Display: OpenWillow HUD 3D path affine: 3D turn not reported'
HEALTH_RED, AMMO_BLUE = (200, 30, 30), (80, 120, 230)


def state(magazine=7, shield='1.000', reserve=200, level='36', health_text='400'):
    return (f'LogTemp: Display: OWHUDSTATE health=1.000 healthText={health_text} shield={shield} '
            f'ammoText={magazine}{NB}/{NB}{reserve} grenadeText=3{NB}/{NB}3 level={level}')


def synthetic_log(magazines=(7, 6, 5, 4), shield=True, path=(MOVIE, WARP), states=True, done=True, level='36'):
    """A run's log as the host writes it: the path line, state lines, a shield line, three fires, the shots."""
    lines = list(path)
    if shield:
        lines.append('LogTemp: Display: OWHUDSHOTS shield The Bee')
    if states:
        lines.append(state(magazines[0], shield='1.000' if shield else 'null', level=level))
    lines.append('LogTemp: Display: OWHUDSHOTS shot rest t=12.006')
    for magazine in magazines[1:]:
        lines.append('LogTemp: Display: OWHUDSHOTS fire 1')
        if states:
            lines.append(state(magazine, level=level))
    lines.append('LogTemp: Display: OWHUDSHOTS shot fired t=15.129')
    if done:
        lines.append('LogTemp: Display: OWHUDSHOTS done')
    return '\n'.join(lines)


def png(path, size=(1280, 720), boxes=(), shade=True):
    """An RGB PNG (filter 0): a grey background, shaded row by row unless shade is False, with filled boxes [(x0, y0, x1, y1, rgb)]."""
    width, height = size
    raw = bytearray()
    for y in range(height):
        level = 20 + (y * 37) % 211 if shade else 60     # a shade per row; a prime step keeps every 4th row distinct
        row = bytearray(b'\0' + bytes((level,) * 3) * width)
        for x0, y0, x1, y1, rgb in boxes:
            if y0 <= y < y1:
                row[1 + 3 * x0:1 + 3 * x1] = bytes(rgb) * (x1 - x0)
        raw += row

    def chunk(kind, body):
        return struct.pack('>I', len(body)) + kind + body + struct.pack('>I', zlib.crc32(kind + body))
    Path(path).write_bytes(chk.PNG_SIGNATURE + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
                           + chunk(b'IDAT', zlib.compress(bytes(raw))) + chunk(b'IEND', b''))


def health_box():
    x0, y0, x1, y1 = chk.HEALTH_BOX
    return (x0 + 10, y0 + 10, x1 - 10, y1 - 10, HEALTH_RED)


def ammo_box():
    x0, y0, x1, y1 = chk.AMMO_BOX
    return (x0 + 10, y0 + 10, x1 - 10, y1 - 10, AMMO_BLUE)


def outcome(text, folder=None, expect_shield=False):
    return {name: (ok, detail) for name, ok, detail in chk.run_checks(text, folder, expect_shield)}


class CheckTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.folder = tempfile.TemporaryDirectory()
        folder = Path(cls.folder.name)
        png(folder / 'OWHud_rest.png', boxes=[health_box(), ammo_box()])
        png(folder / 'OWHud_fired.png', boxes=[health_box(), ammo_box()])

    @classmethod
    def tearDownClass(cls):
        cls.folder.cleanup()

    def copy_shots(self):
        target = tempfile.TemporaryDirectory()
        for name in chk.SHOTS:
            (Path(target.name) / name).write_bytes((Path(self.folder.name) / name).read_bytes())
        return target

    def test_matching_run_passes(self):
        found = outcome(synthetic_log(), self.folder.name, expect_shield=True)
        self.assertTrue(all(ok for ok, _ in found.values()), {k: v for k, v in found.items() if not v[0]})

    def test_canvas_path_fails(self):
        found = outcome(synthetic_log(path=(CANVAS,)))
        self.assertFalse(found['movie_path'][0])
        self.assertIn('canvas', found['movie_path'][1])
        self.assertFalse(outcome(synthetic_log(path=(MOVIE, CANVAS, WARP)))['movie_path'][0])

    def test_affine_path_fails_with_reason(self):
        found = outcome(synthetic_log(path=(MOVIE, AFFINE)))
        self.assertFalse(found['warp_path'][0])
        self.assertIn('affine', found['warp_path'][1])
        self.assertIn('3D turn not reported', found['warp_path'][1])

    def test_no_state_lines_fails(self):
        found = outcome(synthetic_log(states=False))
        self.assertFalse(found['state_lines'][0])
        self.assertFalse(found['state_format'][0])

    def test_level_not_zero_padded_fails(self):
        found = outcome(synthetic_log(level='8'))
        self.assertFalse(found['state_format'][0])
        self.assertIn('level', found['state_format'][1])

    def test_ascii_separator_in_ammo_fails(self):
        text = synthetic_log().replace(f'ammoText=6{NB}/{NB}200', 'ammoText=6 / 200')
        self.assertIn('ammoText=6 / 200', text)
        self.assertFalse(outcome(text)['state_format'][0])

    def test_shot_fired_but_magazine_unchanged_fails(self):
        found = outcome(synthetic_log(magazines=(7, 7, 5, 4)))
        self.assertFalse(found['fire_counts'][0])
        self.assertFalse(outcome(synthetic_log(magazines=(7, 6, 5, 6)))['fire_counts'][0])

    def test_missing_done_fails(self):
        self.assertFalse(outcome(synthetic_log(done=False))['done'][0])

    def test_shield_expected_but_not_shown_fails(self):
        self.assertFalse(outcome(synthetic_log(shield=False), self.folder.name, expect_shield=True)['shield_shown'][0])

    def test_shield_check_skipped_without_flag(self):
        self.assertIsNone(outcome(synthetic_log(shield=False))['shield_shown'][0])

    def test_missing_shot_fails_and_names_it(self):
        with self.copy_shots() as folder:
            (Path(folder) / 'OWHud_fired.png').unlink()
            found = outcome(synthetic_log(), folder)
            self.assertFalse(found['shots_valid'][0])
            self.assertIn('OWHud_fired.png', found['shots_valid'][1])

    def test_truncated_or_corrupt_shot_fails(self):
        with self.copy_shots() as folder:
            whole = (Path(folder) / 'OWHud_rest.png').read_bytes()
            (Path(folder) / 'OWHud_rest.png').write_bytes(whole[:len(whole) // 2])
            found = outcome(synthetic_log(), folder)
            self.assertFalse(found['shots_valid'][0])
            self.assertIn('OWHud_rest.png', found['shots_valid'][1])
        with self.copy_shots() as folder:
            corrupt = bytearray((Path(folder) / 'OWHud_fired.png').read_bytes())
            corrupt[60] ^= 0xFF                                  # a byte inside IDAT: its CRC fails
            (Path(folder) / 'OWHud_fired.png').write_bytes(bytes(corrupt))
            found = outcome(synthetic_log(), folder)
            self.assertFalse(found['shots_valid'][0])
            self.assertIn('CRC', found['shots_valid'][1])

    def test_wrong_size_or_empty_shot_fails(self):
        with self.copy_shots() as folder:
            png(Path(folder) / 'OWHud_fired.png', size=(640, 360))
            found = outcome(synthetic_log(), folder)
            self.assertFalse(found['shots_valid'][0])
            self.assertIn('640x360', found['shots_valid'][1])
        with self.copy_shots() as folder:
            (Path(folder) / 'OWHud_rest.png').write_bytes(b'')
            self.assertIn('not a PNG', outcome(synthetic_log(), folder)['shots_valid'][1])

    def test_blank_shot_fails(self):
        with self.copy_shots() as folder:
            for name in chk.SHOTS:
                png(Path(folder) / name, shade=False)            # one flat colour, nothing drawn
            found = outcome(synthetic_log(), folder)
            self.assertTrue(found['shots_valid'][0])
            for name in ('shots_not_blank', 'health_bar_drawn', 'ammo_bar_drawn'):
                self.assertFalse(found[name][0], name)

    def test_missing_red_in_health_box_fails(self):
        with self.copy_shots() as folder:
            png(Path(folder) / 'OWHud_rest.png', boxes=[ammo_box()])
            found = outcome(synthetic_log(), folder)
            self.assertFalse(found['health_bar_drawn'][0])
            self.assertTrue(found['ammo_bar_drawn'][0])
            self.assertTrue(found['shots_not_blank'][0])

    def test_exit_codes(self):
        with tempfile.TemporaryDirectory() as temp:
            log = Path(temp) / 'run.log'
            log.write_text(synthetic_log(), encoding='utf-8')
            argv = sys.argv
            try:
                sys.argv = ['check', str(log)]
                self.assertEqual(chk.main(), 2)                # no captures: PARTIAL
                sys.argv = ['check', str(log), '--shots', self.folder.name, '--expect-shield']
                self.assertEqual(chk.main(), 0)
                log.write_text(synthetic_log(done=False), encoding='utf-8')
                self.assertEqual(chk.main(), 1)                # a failed check: FAIL
            finally:
                sys.argv = argv


if __name__ == '__main__':
    unittest.main()
