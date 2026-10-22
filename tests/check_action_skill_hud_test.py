"""Synthetic tests for tools/check_action_skill_hud.py (no game or engine needed; PNGs are generated here)."""
import math
import struct
import sys
import tempfile
import unittest
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_action_skill_hud as chk  # noqa: E402

GREEN, GOLD, RED, MAGENTA, GREY = (60, 220, 60), (240, 170, 40), (170, 30, 30), (220, 60, 220), (90, 90, 90)


def png(path, size=(1280, 720), boxes=(), background=GREY):
    """An RGB PNG (filter 0) of one colour with filled boxes [(x0, y0, x1, y1, rgb)]."""
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
    Path(path).write_bytes(chk.PNG_SIGNATURE + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
                           + chunk(b'IDAT', zlib.compress(bytes(raw))) + chunk(b'IEND', b''))


def icon(rgb):
    x0, y0, x1, y1 = chk.ICON_BOX
    return (x0 + 20, y0 + 20, x0 + 60, y0 + 60, rgb)          # 1600 pixels


def emblem():
    x0, y0, x1, y1 = chk.EMBLEM_BOX
    return (x0 + 50, y0 + 20, x0 + 150, y0 + 60, MAGENTA)     # 4000 pixels


def synthetic_log(cast=14.0, lock=0.70, end=5.70, pool=13.0, drain=1.0, step=1 / 60, held_drain=0.0,
                  page=True, stuck_icon=False, bar_late=0, shots=True, done=True):
    """A run's log as the host writes it: quantized state lines on change, the page's read-back after each, shots."""
    lines = ['LogTemp: Display: OWSKILLHUD t=2.000 cast=-100.000 active=-1.00 cooldown=0.00']
    lines.append(f'LogTemp: Display: OWSKILLHUDSHOTS cast t={cast:.3f} hit=1')
    t, last, shot, bars = 0.0, None, 0, []
    while t < 22:
        active = math.ceil((1 - (t - lock) / (end - lock)) * 100) / 100 if lock <= t < end else -1
        left = pool - t * held_drain if t < end else max(0.0, pool - end * held_drain - (t - end) * drain)
        state = (f'{active:.2f}', f'{math.ceil(left / pool * 100) / 100:.2f}')
        if state != last:
            last = state
            lines.append(f'LogTemp: Display: OWSKILLHUD t={cast + t:.3f} cast={cast:.3f} active={state[0]} cooldown={state[1]}')
            emblem_frame, bar, icon_frame = chk.expected_frames(float(state[0]), float(state[1]))
            bars.append(bar)
            if bar_late and emblem_frame == 3 and len(bars) > bar_late:
                bar = bars[-1 - bar_late] or bar       # the movie showing an older bar frame
            if stuck_icon:
                icon_frame = 2
            if page:
                lines.append(f'LogTemp: Display: OpenWillow Flash HUD console: OWSKILLPAGE active={state[0]} '
                             f'cooldown={state[1]} emblem={emblem_frame} bar={bar} icon={icon_frame} (x:1)')
        while shots and shot < chk.EXPECTED_SHOTS and t >= shot * 0.5:
            lines.append(f'LogTemp: Display: OWSKILLHUDSHOTS shot OWSkillHud_{shot:03d}.png scheduled {shot * 0.5 - 1:+.2f} '
                         f'actual x t=x')
            shot += 1
        t += step
    while shots and shot < chk.EXPECTED_SHOTS:
        lines.append(f'LogTemp: Display: OWSKILLHUDSHOTS shot OWSkillHud_{shot:03d}.png scheduled {shot * 0.5 - 1:+.2f} actual x t=x')
        shot += 1
    if done:
        lines.append('LogTemp: Display: OWSKILLHUDSHOTS done')
    return '\n'.join(lines)


def results(text, shots=None):
    return {name: ok for name, ok, _ in chk.run_checks(text, shots)}


class CheckTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Shots: the three the content check reads carry the indicator; the rest only need to be valid PNG files.
        cls.folder = tempfile.TemporaryDirectory()
        folder = Path(cls.folder.name)
        png(folder / 'plain.png')
        for i in range(chk.EXPECTED_SHOTS):
            name = folder / f'OWSkillHud_{i:03d}.png'
            if i == 0:
                png(name, boxes=[icon(GREEN)])
            elif i == 7:
                png(name, boxes=[icon(GOLD), emblem()])
            elif i == 26:
                png(name, boxes=[icon(RED)])
            else:
                name.write_bytes((folder / 'plain.png').read_bytes())   # complete PNGs with nothing drawn

    @classmethod
    def tearDownClass(cls):
        cls.folder.cleanup()

    def copy_shots(self):
        target = tempfile.TemporaryDirectory()
        for f in Path(self.folder.name).glob('OWSkillHud_*.png'):
            (Path(target.name) / f.name).write_bytes(f.read_bytes())
        return target

    def test_matching_run_passes(self):
        found = results(synthetic_log(), self.folder.name)
        self.assertTrue(all(found.values()), {k: v for k, v in found.items() if not v})

    def test_state_timing_failures(self):
        self.assertFalse(results(synthetic_log(end=6.70))['emblem_off_at_end'])
        self.assertFalse(results(synthetic_log(held_drain=1.0))['pool_held_while_active'])
        drift = results(synthetic_log(drain=1.2))
        self.assertFalse(drift['pool_drain_rate'])
        self.assertFalse(drift['ready_again'])
        self.assertFalse(results('nothing here')['cast_logged'])

    def test_quantized_samples_keep_timing_checks(self):
        found = results(synthetic_log())
        for name in ('emblem_on_at_lock', 'emblem_off_at_end', 'pool_drain_rate', 'ready_again', 'pool_held_while_active'):
            self.assertTrue(found[name], name)

    def test_no_page_read_back_fails(self):
        found = results(synthetic_log(page=False))
        self.assertTrue(found['ready_again'])                 # the host state alone still looks right
        self.assertFalse(found['page_lines'])
        self.assertFalse(found['page_emblem_drawn'])

    def test_icon_not_following_state_fails(self):
        found = results(synthetic_log(stuck_icon=True))
        self.assertFalse(found['page_frames_match_state'])
        self.assertFalse(found['page_cooldown_drawn'])

    def test_bar_two_frames_late_fails(self):
        self.assertFalse(results(synthetic_log(bar_late=2))['page_frames_match_state'])

    def test_page_state_not_from_host_fails(self):
        text = synthetic_log().replace('OWSKILLPAGE active=0.50 cooldown=1.00', 'OWSKILLPAGE active=0.50 cooldown=0.77')
        self.assertIn('cooldown=0.77', text)
        self.assertFalse(results(text)['page_state_from_host'])

    def test_missing_or_invalid_shot_fails_and_names_it(self):
        with self.copy_shots() as folder:
            (Path(folder) / 'OWSkillHud_010.png').unlink()
            (Path(folder) / 'OWSkillHud_011.png').write_bytes(b'')
            png(Path(folder) / 'OWSkillHud_012.png', size=(640, 360))
            found = {n: (ok, d) for n, ok, d in chk.run_checks(synthetic_log(), folder)}
            self.assertFalse(found['shots_written'][0])
            for name in ('OWSkillHud_010.png', 'OWSkillHud_011.png', 'OWSkillHud_012.png'):
                self.assertIn(name, found['shots_written'][1])

    def test_truncated_or_corrupt_shot_fails_and_names_it(self):
        with self.copy_shots() as folder:
            whole = (Path(folder) / 'OWSkillHud_020.png').read_bytes()
            (Path(folder) / 'OWSkillHud_020.png').write_bytes(whole[:len(whole) // 2])     # header intact, IDAT cut off
            corrupt = bytearray(whole)
            corrupt[60] ^= 0xFF                                                          # a byte inside IDAT: CRC fails
            (Path(folder) / 'OWSkillHud_021.png').write_bytes(bytes(corrupt))
            self.assertTrue(chk.valid_shot(Path(folder) / 'OWSkillHud_022.png'))
            found = {n: (ok, d) for n, ok, d in chk.run_checks(synthetic_log(), folder)}
            self.assertFalse(found['shots_written'][0])
            self.assertIn('OWSkillHud_020.png', found['shots_written'][1])
            self.assertIn('OWSkillHud_021.png', found['shots_written'][1])

    def test_blank_or_uniform_captures_fail(self):
        with self.copy_shots() as folder:
            for i in (0, 7, 26):
                png(Path(folder) / f'OWSkillHud_{i:03d}.png')
            found = results(synthetic_log(), folder)
            self.assertTrue(found['shots_written'])           # valid files, nothing drawn
            for name in ('shot_ready_drawn', 'shot_active_drawn', 'shot_cooldown_drawn'):
                self.assertFalse(found[name], name)

    def test_indicator_in_wrong_state_fails(self):
        with self.copy_shots() as folder:
            png(Path(folder) / 'OWSkillHud_026.png', boxes=[icon(GOLD), emblem()])   # still active in the cooldown
            found = results(synthetic_log(), folder)
            self.assertTrue(found['shot_active_drawn'])
            self.assertFalse(found['shot_cooldown_drawn'])

    def test_run_without_done_or_shots_fails(self):
        self.assertFalse(results(synthetic_log(done=False), self.folder.name)['shots_scheduled'])
        found = results(synthetic_log(shots=False), self.folder.name)
        self.assertFalse(found['shots_scheduled'])
        self.assertFalse(found['shots_written'])

    def test_without_shots_folder_is_partial(self):
        log = Path(self.folder.name) / 'run.log'
        log.write_text(synthetic_log(), encoding='utf-8')
        argv = sys.argv
        try:
            sys.argv = ['check', str(log)]
            self.assertEqual(chk.main(), 2)
            sys.argv = ['check', str(log), '--shots', self.folder.name]
            self.assertEqual(chk.main(), 0)
        finally:
            sys.argv = argv

    def test_expected_frames(self):
        self.assertEqual(chk.expected_frames(-1, 0), (1, 0, 100))
        self.assertEqual(chk.expected_frames(1, 1), (3, 1, 2))
        self.assertEqual(chk.expected_frames(0.5, 0.5), (3, math.floor(1 + 99 * 0.5), math.floor(2 + 98 * 0.5)))


if __name__ == '__main__':
    unittest.main()
