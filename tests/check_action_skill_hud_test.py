"""Synthetic tests for tools/check_action_skill_hud.py (no game or engine needed)."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_action_skill_hud as chk  # noqa: E402


def synthetic_log(cast=14.0, lock=0.70, end=5.70, pool=13.0, drain=1.0, step=1 / 60, held_drain=0.0):
    lines = [f'LogTemp: Display: OWSKILLHUD t=2.000 cast=-100.000 active=-1.000 cooldown=0.000']
    lines.append(f'LogTemp: Display: OWSKILLHUDSHOTS cast t={cast:.3f} hit=1')
    t = 0.0
    while t < 22:
        active = 1 - (t - lock) / (end - lock) if lock <= t < end else -1
        left = pool - t * held_drain if t < end else max(0.0, pool - end * held_drain - (t - end) * drain)
        lines.append(f'LogTemp: Display: OWSKILLHUD t={cast + t:.3f} cast={cast:.3f} active={active:.3f} cooldown={left / pool:.3f}')
        t += step
    return '\n'.join(lines)


def run(text):
    rows, (cast_at, hit) = chk.parse(text)
    return {name: ok for name, ok, _ in chk.check(rows, cast_at, hit)}


class CheckTest(unittest.TestCase):
    def test_matching_run_passes(self):
        results = run(synthetic_log())
        self.assertTrue(all(results.values()), results)

    def test_emblem_through_release_buffer_fails(self):
        results = run(synthetic_log(end=6.70))
        self.assertFalse(results['emblem_off_at_end'])

    def test_pool_draining_while_held_fails(self):
        self.assertFalse(run(synthetic_log(held_drain=1.0))['pool_held_while_active'])

    def test_wrong_drain_rate_fails(self):
        results = run(synthetic_log(drain=1.2))
        self.assertFalse(results['pool_drain_rate'])
        self.assertFalse(results['ready_again'])

    def test_no_cast_fails(self):
        self.assertFalse(run('nothing here')['cast_logged'])


if __name__ == '__main__':
    unittest.main()
