"""The Unreal run lock is one file per machine, not one per checkout.

AI-assisted. Synthetic: reads the tracked scripts and runs tools/ue_lock.ps1 with a temporary path; no game data.

1. Every script that takes the lock gets its path from tools/ue_lock.ps1 (no script builds a
   per-checkout local/ue_run.lock path any more).
2. tools/ue_lock.ps1 prints %LOCALAPPDATA%\\OpenWillow\\ue_run.lock by default, honours
   OPENWILLOW_UE_LOCK, and creates the folder. Skipped when no PowerShell is installed.
"""
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / 'tools' / 'ue_lock.ps1'
POWERSHELL = shutil.which('pwsh') or shutil.which('powershell')


def scripts():
    return sorted(list((ROOT / 'tools').glob('*.ps1')) + list((ROOT / 'tools' / 'real_game').glob('*.ps1')))


class UeLockTest(unittest.TestCase):
    def test_no_script_builds_a_per_checkout_lock(self):
        for path in scripts():
            if path == HELPER:
                continue
            text = path.read_text(encoding='utf-8-sig', errors='replace')
            with self.subTest(script=path.name):
                self.assertNotRegex(text, r"Join-Path[^\n]*'(local/)?ue_run\.lock'")

    def test_lock_takers_use_the_helper(self):
        takers = []
        for path in scripts():
            if path == HELPER:
                continue
            text = path.read_text(encoding='utf-8-sig', errors='replace')
            if re.search(r'^\s*\$(script:)?[Ll]ock(Path)?\s*=', text, re.M):
                takers.append(path.name)
                with self.subTest(script=path.name):
                    self.assertRegex(text, r"\$(script:)?[Ll]ock(Path)?\s*=\s*& \(Join-Path \$PSScriptRoot '(\.\./)?ue_lock\.ps1'\)")
        self.assertGreater(len(takers), 15)

    def run_helper(self, env):
        done = subprocess.run([POWERSHELL, '-NoProfile', '-NonInteractive', '-File', str(HELPER)],
                              capture_output=True, text=True, env=env, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr)
        return done.stdout.strip()

    @unittest.skipUnless(POWERSHELL, 'PowerShell not installed')
    def test_helper_default_and_override(self):
        with tempfile.TemporaryDirectory() as tmp:
            env = dict(os.environ)
            env.pop('OPENWILLOW_UE_LOCK', None)
            env['LOCALAPPDATA'] = tmp
            default = Path(self.run_helper(env))
            self.assertEqual(default, Path(tmp) / 'OpenWillow' / 'ue_run.lock')
            self.assertTrue(default.parent.is_dir())
            override = Path(tmp) / 'shared' / 'deeper' / 'my.lock'
            env['OPENWILLOW_UE_LOCK'] = str(override)
            self.assertEqual(Path(self.run_helper(env)), override)
            self.assertTrue(override.parent.is_dir())
            self.assertFalse(override.exists())   # the helper names the lock; it never takes it


if __name__ == '__main__':
    unittest.main()
