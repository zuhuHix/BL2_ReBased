"""tools/private_sync.ps1 refuses a store inside the repository, also when it is reached through a junction.

Windows only (junctions); runs the real script with -WhatIf on temporary folders and never copies anything.
"""
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'tools' / 'private_sync.ps1'
SHELLS = [s for s in (shutil.which('pwsh'), shutil.which('powershell')) if s]


@unittest.skipUnless(sys.platform == 'win32' and SHELLS, 'needs Windows junctions and PowerShell')
class PrivateSyncStoreTests(unittest.TestCase):
    def setUp(self):
        import _winapi
        self.junction = _winapi.CreateJunction
        self.tmp = Path(tempfile.mkdtemp(prefix='ow_sync_'))
        self.addCleanup(shutil.rmtree, self.tmp, True)

    def link(self, name, target):
        path = self.tmp / name
        self.junction(str(target), str(path))
        self.addCleanup(os.rmdir, path)  # removes the junction itself, before rmtree could walk into it
        return path

    def run_sync(self, shell, store):
        return subprocess.run([shell, '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(SCRIPT), '-Direction', 'Push',
                               '-Store', str(store), '-Folders', 'local/ow_sync_test_missing', '-WhatIf'],
                              capture_output=True, text=True)

    def test_each_shell(self):
        name = f'ow_sync_test_store_{os.getpid()}'
        into_repo = self.link('into_repo', ROOT)
        outside = self.tmp / 'real'
        outside.mkdir()
        elsewhere = self.link('elsewhere', outside)
        for shell in SHELLS:
            with self.subTest(shell=Path(shell).name):
                plain = self.run_sync(shell, ROOT / name)
                self.assertNotEqual(plain.returncode, 0)
                self.assertIn('inside the repository', plain.stdout + plain.stderr)
                through = self.run_sync(shell, into_repo / name)
                self.assertNotEqual(through.returncode, 0, through.stdout)
                self.assertIn('inside the repository', through.stdout + through.stderr)
                self.assertFalse((ROOT / name).exists(), 'a refused store must not be created')
                fine = self.run_sync(shell, elsewhere / 'store')
                self.assertEqual(fine.returncode, 0, fine.stdout + fine.stderr)
                self.assertIn('skip (missing)', fine.stdout)


if __name__ == '__main__':
    unittest.main()
