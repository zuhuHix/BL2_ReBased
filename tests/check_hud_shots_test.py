"""Synthetic tests for tools/check_hud_shots.py (no game or engine needed; logs and PNGs are generated here).

Every value is invented: a pistol with a 12-round magazine and 340 in reserve, grenades 2 of 4, a health text of 250,
a shield of 1200 named "Test Shield". The frames the page should show are worked out by hand in the comments below
(1 + round((1 - f) * 99), round half up), not computed by the checker.
"""
import struct
import sys
import tempfile
import unittest
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_hud_shots as chk  # noqa: E402

NB = '\\u00a0'      # the host's JSON escape for a no-break space, as written in the log
PREFIX = 'LogTemp: Display: '
CONSOLE = 'LogTemp: Display: OpenWillow Flash HUD console: '
READY = CONSOLE + 'OpenWillow HUD movie ready pages=C:\\Test\\tools\\hud_overlay (http://127.0.0.1:8795/:152)'
MOVIE = PREFIX + 'OpenWillow HUD path: movie (page reported ready)'
WARP = CONSOLE + 'OpenWillow HUD 3D path warp: Ruffle webgl2 (http://127.0.0.1:8795/hud_warp.js:311)'
CANVAS = PREFIX + "OpenWillow HUD path: canvas (the page is served from 'C:\\Test\\served', not this checkout's tools/hud_overlay)"
AFFINE = PREFIX + 'OpenWillow HUD 3D path affine: 3D turn not reported'
HEALTH_RED, AMMO_BLUE = (200, 30, 30), (80, 120, 230)
# Magazine 12 of 12 down to 9: the ammo fraction, and the bullets frame 1 + round((1 - f) * 99) worked out by hand.
MAGAZINE_FRACTION = {12: '1.000', 11: '0.917', 10: '0.833', 9: '0.750'}
MAGAZINE_FRAME = {12: 1, 11: 9, 10: 18, 9: 26}
# Grenades 2 of 4 is the fraction 0.500: 1 + round(0.5 * 99) = 1 + round(49.5) = 51 (round half up).
GRENADE_FRAME = '51'
PAGE_SHAPE = 'outer=3 spread=9 icon=3'                         # the pistol's frames
ENDING = ' (http://127.0.0.1:8795/:112)'


def host_fields(magazine=12, shielded=True, level='24'):
    """The 11 OWHUDSTATE fields, in the host's order, for a pistol out with the given magazine."""
    return ('1.000', '1.000' if shielded else 'null', MAGAZINE_FRACTION[magazine], '0.500', '1', 'pistol',
            '250', '1200' if shielded else '', f'{magazine}{NB}/{NB}340', f'2{NB}/{NB}4', level)


def state(magazine=12, shielded=True, level='24'):
    keys = chk.STATE_FIELDS
    return PREFIX + 'OWHUDSTATE ' + ' '.join(
        f'{key}={value}' for key, value in zip(keys, host_fields(magazine, shielded, level)))


def page(magazine=12, shielded=True, level='24'):
    """The OWHUDPAGE line the page writes for that state: its echo, then the frames and texts it shows."""
    echo = state(magazine, shielded, level).split('OWHUDSTATE ', 1)[1]
    fields = host_fields(magazine, shielded, level)
    shield_frame = '1' if shielded else '101'
    shown = (f'fHealth=1 fShield={shield_frame} fBullets={MAGAZINE_FRAME[magazine]} fGrenades={GRENADE_FRAME} '
             f'tHealth=250 tShield={fields[7]} tAmmo={fields[8]} tGrenade={fields[9]} tLevel={level} {PAGE_SHAPE}')
    return CONSOLE + 'OWHUDPAGE ' + echo + ' ' + shown + ENDING


def synthetic_log(magazines=(12, 11, 10, 9), shield=True, path=(READY, MOVIE, WARP), states=True, pages=True,
                  done=True, level='24'):
    """A run's log as the host writes it: the path lines, a shield line, the rest shot, three fires, the fired shot.
    Each state is followed by its page read-back."""
    lines = list(path)
    if shield:
        lines.append(PREFIX + 'OWHUDSHOTS shield Test Shield')

    def read(magazine):
        if states:
            lines.append(state(magazine, shield, level))
            if pages:
                lines.append(page(magazine, shield, level))

    read(magazines[0])
    lines.append(PREFIX + 'OWHUDSHOTS shot rest t=12.006')
    for magazine in magazines[1:]:
        lines.append(PREFIX + 'OWHUDSHOTS fire 1')
        read(magazine)
    lines.append(PREFIX + 'OWHUDSHOTS shot fired t=15.129')
    if done:
        lines.append(PREFIX + 'OWHUDSHOTS done')
    return '\n'.join(lines)


def edit_page(text, index, old, new):
    """Replace old by new in the index-th OWHUDPAGE line only (index -1 is the last one)."""
    lines = text.split('\n')
    pages = [i for i, line in enumerate(lines) if 'OWHUDPAGE' in line]
    target = pages[index]
    assert old in lines[target], (old, lines[target])
    lines[target] = lines[target].replace(old, new)
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


PAGE_CHECKS = ('page_lines', 'page_state_from_host', 'page_frames_match_state', 'page_follows_fire')


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
        text = synthetic_log().replace(f'ammoText=11{NB}/{NB}340', 'ammoText=11 / 340')
        self.assertIn('ammoText=11 / 340', text)
        self.assertFalse(outcome(text)['state_format'][0])

    def test_fraction_without_three_decimals_fails(self):
        text = synthetic_log().replace('ammo=0.917', 'ammo=0.92', 1)
        self.assertIn('ammo=0.92 ', text)
        self.assertFalse(outcome(text)['state_format'][0])

    def test_shot_fired_but_magazine_unchanged_fails(self):
        found = outcome(synthetic_log(magazines=(12, 12, 10, 9)))
        self.assertFalse(found['fire_counts'][0])
        self.assertFalse(outcome(synthetic_log(magazines=(12, 11, 10, 10)))['fire_counts'][0])

    def test_missing_done_fails(self):
        self.assertFalse(outcome(synthetic_log(done=False))['done'][0])

    def test_shield_expected_but_not_shown_fails(self):
        self.assertFalse(outcome(synthetic_log(shield=False), self.folder.name, expect_shield=True)['shield_shown'][0])

    def test_shield_check_skipped_without_flag(self):
        self.assertIsNone(outcome(synthetic_log(shield=False))['shield_shown'][0])

    def test_page_lines_need_three(self):
        self.assertTrue(outcome(synthetic_log())['page_lines'][0])
        self.assertFalse(outcome(synthetic_log(magazines=(12, 11)))['page_lines'][0])

    def test_no_page_lines_fails(self):
        found = outcome(synthetic_log(pages=False))
        self.assertFalse(found['page_lines'][0])
        for name in PAGE_CHECKS[1:]:
            self.assertFalse(found[name][0], name)
        self.assertTrue(found['state_format'][0])

    def test_page_echo_matching_no_state_fails(self):
        text = edit_page(synthetic_log(), 0, f'healthText=250 ', 'healthText=251 ')
        found = outcome(text)
        self.assertFalse(found['page_state_from_host'][0])
        self.assertIn('line', found['page_state_from_host'][1])

    def test_bullets_frame_off_by_two_fails(self):
        text = edit_page(synthetic_log(), 1, f'fBullets={MAGAZINE_FRAME[11]} ', f'fBullets={MAGAZINE_FRAME[11] + 2} ')
        found = outcome(text)
        self.assertTrue(found['page_state_from_host'][0])
        self.assertFalse(found['page_frames_match_state'][0])
        self.assertIn('fBullets', found['page_frames_match_state'][1])

    def test_grenade_frame_rounds_half_up(self):
        text = edit_page(synthetic_log(), 0, f'fGrenades={GRENADE_FRAME} ', 'fGrenades=50 ')
        found = outcome(text)
        self.assertFalse(found['page_frames_match_state'][0])
        self.assertIn('fGrenades', found['page_frames_match_state'][1])

    def test_tammo_with_ascii_separator_fails(self):
        text = edit_page(synthetic_log(), 1, f'tAmmo=11{NB}/{NB}340 ', 'tAmmo=11 / 340 ')
        found = outcome(text)
        self.assertTrue(found['page_state_from_host'][0])
        self.assertFalse(found['page_frames_match_state'][0])
        self.assertIn('tAmmo', found['page_frames_match_state'][1])

    def test_shield_null_with_frame_50_fails(self):
        text = synthetic_log(shield=False).replace('fShield=101 ', 'fShield=50 ')
        self.assertIn('fShield=50 ', text)
        found = outcome(text)
        self.assertFalse(found['page_frames_match_state'][0])
        self.assertIn('fShield', found['page_frames_match_state'][1])

    def test_pistol_with_outer_1_fails(self):
        text = synthetic_log().replace('outer=3 ', 'outer=1 ')
        found = outcome(text)
        self.assertFalse(found['page_frames_match_state'][0])
        self.assertIn('outer', found['page_frames_match_state'][1])

    def test_shotgun_frames_pass(self):
        text = synthetic_log().replace('weaponType=pistol', 'weaponType=shotgun').replace(
            PAGE_SHAPE, 'outer=7 spread=17 icon=5')
        self.assertTrue(outcome(text)['page_frames_match_state'][0])

    def test_final_page_behind_host_fails(self):
        lines = synthetic_log().split('\n')
        pages = [i for i, line in enumerate(lines) if 'OWHUDPAGE' in line]
        lines[pages[-1]] = lines[pages[-2]]                 # the last page still shows the magazine before
        found = outcome('\n'.join(lines))
        self.assertTrue(found['page_state_from_host'][0])
        self.assertTrue(found['page_frames_match_state'][0])
        self.assertFalse(found['page_follows_fire'][0])
        self.assertIn('"10', found['page_follows_fire'][1])

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
