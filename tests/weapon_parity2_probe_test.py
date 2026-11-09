"""Synthetic test for the weapons parity 2 probe's hook rows (no game): every hook call must reach the run file.

Run: python tests/weapon_parity2_probe_test.py
"""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools" / "real_game" / "scripts"))
import weapon_parity2_probe as probe


class _Named:
    def __init__(self, name):
        self.Name = name


class _Func:
    def __init__(self, name):
        self.func = _Named(name)


class Wp2HookRowTests(unittest.TestCase):
    def setUp(self):
        self.rows = []

        # Same signature as weapon_firing_probe._log: the row kind is the first argument.
        def log(kind, **data):
            self.rows.append(dict(data, kind=kind))

        probe._log = log
        probe.fp_state = {"run": {"name": "test"}}
        probe._path = lambda obj: "Fake.Obj"
        probe._args = lambda args: {}

    def test_an_emitter_hook_call_is_logged_with_its_hook_group(self):
        # Run Z47: the row's own "kind" key collided with _log's kind argument, so every call raised and became a hook_error.
        probe._wp2_cb("emitter")(object(), object(), None, _Func("SpawnEmitter"))
        self.assertEqual(self.rows, [{"kind": "wp2", "hook": "emitter", "func": "SpawnEmitter", "obj": "Fake.Obj", "args": {}}])

    def test_nothing_is_logged_outside_a_run(self):
        probe.fp_state = {"run": None}
        probe._wp2_cb("hurt")(object(), object(), None, _Func("HurtRadius"))
        self.assertEqual(self.rows, [])


if __name__ == "__main__":
    unittest.main()
