"""Synthetic tests for tools/provision_worktree_assets.ps1: a throwaway git repository and a fake seed, no game data.

Run: python tests/provision_worktree_test.py   (Windows only; it runs Windows PowerShell, as the post-checkout hook does)
"""
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'tools' / 'provision_worktree_assets.ps1'
POWERSHELL = shutil.which('powershell') or shutil.which('pwsh')


def write(path, text='x'):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding='utf-8')


@unittest.skipUnless(sys.platform == 'win32' and POWERSHELL, 'needs Windows PowerShell')
class ProvisionTest(unittest.TestCase):
    def setUp(self):
        self._folder = tempfile.TemporaryDirectory()
        self.addCleanup(self._folder.cleanup)
        base = Path(self._folder.name)
        self.worktree = base / 'worktree'
        self.seed = base / 'seed'
        write(self.worktree / 'host' / 'ue5' / 'OpenWillow' / 'OpenWillow.uproject', '{}')
        subprocess.run(['git', 'init', '-q', str(self.worktree)], check=True)
        # The seed: one file per folder the default list names, plus output that must never be copied.
        write(self.seed / 'Content' / 'Maps' / 'a.umap', 'map')
        write(self.seed / 'slice' / 'manifest.json', 'seed')
        write(self.seed / 'doors' / 'mover.json', 'door')
        write(self.seed / 'ui' / 'run' / 'previews' / 'p.png', 'preview')
        write(self.seed / 'range' / 'shots' / 'frame.png', 'shot')
        write(self.seed / 'range' / 'logs' / 'run.log', 'log')
        write(self.seed / 'realgame' / 'capture.json', 'capture')
        write(self.seed / 'sanctuary' / 'big.bin', 'big')          # not in the default list
        write(self.seed / 'unlisted' / 'x.txt', 'not listed')
        # A worktree that already has one of the files keeps its own copy.
        write(self.worktree / 'local' / 'slice' / 'manifest.json', 'own')

    def run_script(self, *extra):
        command = [POWERSHELL, '-NoLogo', '-NoProfile', '-NonInteractive', '-File', str(SCRIPT),
                   '-WorktreeRoot', str(self.worktree), '-SeedDirectory', str(self.seed), *extra]
        return subprocess.run(command, capture_output=True, text=True, encoding='utf-8', errors='replace')

    def local(self, *parts):
        return self.worktree.joinpath('local', *parts)

    def test_default_list_copies_missing_only(self):
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.local('slice', 'manifest.json').read_text(encoding='utf-8'), 'own')   # never overwritten
        self.assertEqual(self.local('doors', 'mover.json').read_text(encoding='utf-8'), 'door')
        self.assertTrue(self.local('ui', 'run', 'previews', 'p.png').is_file())                      # per-folder subpaths kept
        self.assertTrue((self.worktree / 'host' / 'ue5' / 'OpenWillow' / 'Content' / 'Maps' / 'a.umap').is_file())

    def test_output_folders_are_never_copied(self):
        self.run_script()
        self.assertFalse(self.local('range', 'shots').exists())
        self.assertFalse(self.local('range', 'logs').exists())
        self.assertFalse(self.local('realgame').exists())

    def test_big_and_unlisted_folders_stay_out(self):
        self.run_script()
        self.assertFalse(self.local('sanctuary').exists())
        self.assertFalse(self.local('unlisted').exists())

    def test_extra_folder_by_name(self):
        result = self.run_script('-ExtraSeedFolders', 'sanctuary')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.local('sanctuary', 'big.bin').read_text(encoding='utf-8'), 'big')

    def test_summary_names_folders_missing_from_the_seed(self):
        # -File passes an array as one string, so this case calls the script the way a PowerShell caller does.
        call = f"& '{SCRIPT}' -WorktreeRoot '{self.worktree}' -SeedDirectory '{self.seed}' -SeedFolders 'slice','nosuchfolder'"
        result = subprocess.run([POWERSHELL, '-NoLogo', '-NoProfile', '-NonInteractive', '-Command', call],
                                capture_output=True, text=True, encoding='utf-8', errors='replace')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('local/nosuchfolder: 0 missing files added (not in the seed)', result.stdout)
        self.assertIn('local/slice: 0 missing files added', result.stdout)

    def test_unsafe_folder_name_is_refused(self):
        result = self.run_script('-SeedFolders', '..\\escape')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.worktree.parent / 'escape').exists())


if __name__ == '__main__':
    unittest.main()
