"""Synthetic tests for tools/check_action_skill_hud.py (no game or engine needed)."""
import math
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_action_skill_hud as chk  # noqa: E402


def synthetic_log(cast=14.0, lock=0.70, end=5.70, pool=13.0, drain=1.0, step=1 / 60, held_drain=0.0,
                  page=True, stuck_icon=False, bar_offset=0, shots=True, done=True):
    """A run's log: state lines every frame, the page's read-back line after each change, scheduled shots."""
    lines = ['LogTemp: Display: OWSKILLHUD t=2.000 cast=-100.000 active=-1.000 cooldown=0.000']
    lines.append(f'LogTemp: Display: OWSKILLHUDSHOTS cast t={cast:.3f} hit=1')
    t, last_page, shot = 0.0, None, 0
    while t < 22:
        active = 1 - (t - lock) / (end - lock) if lock <= t < end else -1
        left = pool - t * held_drain if t < end else max(0.0, pool - end * held_drain - (t - end) * drain)
        lines.append(f'LogTemp: Display: OWSKILLHUD t={cast + t:.3f} cast={cast:.3f} active={active:.3f} cooldown={left / pool:.3f}')
        emblem, bar, icon = chk.expected_frames(float(f'{active:.3f}'), float(f'{left / pool:.3f}'))
        if stuck_icon:
            icon = 2
        if emblem == 3:
            bar = min(100, bar + bar_offset)
        if page and (emblem, bar, icon) != last_page:
            lines.append(f'LogTemp: Display: OpenWillow Flash HUD console: OWSKILLPAGE emblem={emblem} bar={bar} cooldown={icon} (x:1)')
            last_page = (emblem, bar, icon)
        if shots and shot < chk.EXPECTED_SHOTS and t >= -1 + shot * 0.5 + 1:
            lines.append(f'LogTemp: Display: OWSKILLHUDSHOTS shot OWSkillHud_{shot:03d}.png scheduled x actual x t=x')
            shot += 1
        t += step
    while shots and shot < chk.EXPECTED_SHOTS:
        lines.append(f'LogTemp: Display: OWSKILLHUDSHOTS shot OWSkillHud_{shot:03d}.png scheduled x actual x t=x')
        shot += 1
    if done:
        lines.append('LogTemp: Display: OWSKILLHUDSHOTS done')
    return '\n'.join(lines)


def run(text, shots=None):
    return {name: ok for name, ok, _ in chk.run_checks(text, shots)}


class CheckTest(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        for i in range(chk.EXPECTED_SHOTS):
            (Path(self.folder.name) / f'OWSkillHud_{i:03d}.png').write_bytes(b'')

    def tearDown(self):
        self.folder.cleanup()

    def test_matching_run_passes(self):
        results = run(synthetic_log(), self.folder.name)
        self.assertTrue(all(results.values()), results)

    def test_emblem_through_release_buffer_fails(self):
        self.assertFalse(run(synthetic_log(end=6.70))['emblem_off_at_end'])

    def test_pool_draining_while_held_fails(self):
        self.assertFalse(run(synthetic_log(held_drain=1.0))['pool_held_while_active'])

    def test_wrong_drain_rate_fails(self):
        results = run(synthetic_log(drain=1.2))
        self.assertFalse(results['pool_drain_rate'])
        self.assertFalse(results['ready_again'])

    def test_no_cast_fails(self):
        self.assertFalse(run('nothing here')['cast_logged'])

    def test_no_page_read_back_fails(self):
        results = run(synthetic_log(page=False))
        self.assertTrue(results['ready_again'])           # the host state alone still looks right
        self.assertFalse(results['page_lines'])
        self.assertFalse(results['page_emblem_drawn'])
        self.assertFalse(results['page_cooldown_drawn'])

    def test_icon_not_following_state_fails(self):
        self.assertFalse(run(synthetic_log(stuck_icon=True))['page_cooldown_drawn'])

    def test_bar_frames_off_fails(self):
        self.assertFalse(run(synthetic_log(bar_offset=5))['page_agrees_with_state'])

    def test_missing_shot_fails_and_names_it(self):
        (Path(self.folder.name) / 'OWSkillHud_010.png').unlink()
        found = {name: (ok, detail) for name, ok, detail in chk.run_checks(synthetic_log(), self.folder.name)}
        self.assertFalse(found['shots_written'][0])
        self.assertIn('OWSkillHud_010.png', found['shots_written'][1])

    def test_run_without_done_or_shots_fails(self):
        self.assertFalse(run(synthetic_log(done=False), self.folder.name)['shots_scheduled'])
        results = run(synthetic_log(shots=False), self.folder.name)
        self.assertFalse(results['shots_scheduled'])
        self.assertFalse(results['shots_written'])

    def test_expected_frames(self):
        self.assertEqual(chk.expected_frames(-1, 0), (1, 0, 100))
        self.assertEqual(chk.expected_frames(1, 1), (3, 1, 2))
        self.assertEqual(chk.expected_frames(0.5, 0.5), (3, math.floor(1 + 99 * 0.5), math.floor(2 + 98 * 0.5)))


if __name__ == '__main__':
    unittest.main()
