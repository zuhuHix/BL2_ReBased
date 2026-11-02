"""Synthetic tests of tools/weapon_gen_parity.py's pass/fail logic (no package, no game data)."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_gen_parity as P  # noqa: E402

PASSED = {'mismatches': 0, 'python_errors': 0, 'guns_compared': 4, 'pool_rolls_compared': 2, 'output_problem': None}


class Verdict(unittest.TestCase):
    def test_clean_run_passes(self):
        self.assertEqual(P.verdict(PASSED, 4, 2), [])

    def test_planted_mismatch_fails(self):
        diff = P.Diff()
        diff.compare('balance Test.A stage 8 seed 1', 'gun', {'card': {'damage': 10.0}}, {'card': {'damage': 10.5}})
        report = dict(PASSED, mismatches=diff.mismatches)
        self.assertEqual(diff.mismatches, 1)
        self.assertTrue(P.verdict(report, 4, 2))

    def test_errors_and_short_counts_fail(self):
        self.assertTrue(P.verdict(dict(PASSED, python_errors=1), 4, 2))
        self.assertTrue(P.verdict(dict(PASSED, guns_compared=3), 4, 2))
        self.assertTrue(P.verdict(dict(PASSED, pool_rolls_compared=1), 4, 2))
        self.assertTrue(P.verdict(dict(PASSED, output_problem='too few lines'), 4, 2))


class Identity(unittest.TestCase):
    def test_echo_matching_the_request_passes_and_is_removed(self):
        diff = P.Diff()
        cpp = {'balance': 'Test.A', 'stage': 50, 'seed': 3, 'spawn_stage': 80, 'stage_cap': 50, 'name': 'X'}
        P.check_identity(diff, 'w', cpp, P.expected_identity('Test.A', 80, 3, 50))
        self.assertEqual(diff.mismatches, 0)
        self.assertEqual(cpp, {'name': 'X'})

    def test_wrong_echo_is_a_mismatch(self):
        for wrong in ({'balance': 'Test.B'}, {'seed': 4}, {'stage': 80}):
            diff = P.Diff()
            cpp = dict({'balance': 'Test.A', 'stage': 8, 'seed': 3}, **wrong)
            P.check_identity(diff, 'w', cpp, P.expected_identity('Test.A', 8, 3, None))
            self.assertEqual(diff.mismatches, 1, wrong)

    def test_uncapped_request_must_not_echo_a_cap(self):
        diff = P.Diff()
        cpp = {'balance': 'Test.A', 'stage': 8, 'seed': 3, 'spawn_stage': 8, 'stage_cap': 50}
        P.check_identity(diff, 'w', cpp, P.expected_identity('Test.A', 8, 3, None))
        self.assertEqual(diff.mismatches, 2)


if __name__ == '__main__':
    unittest.main()
