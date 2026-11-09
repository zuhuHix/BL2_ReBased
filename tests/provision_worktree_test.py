"""Synthetic tests for tools/provision_worktree_assets.ps1 and tools/install_worktree_assets_hook.ps1: throwaway git
repositories and fake seeds, no game data.

Run: python tests/provision_worktree_test.py   (Windows only; it runs Windows PowerShell, as the post-checkout hook does,
and the installer tests also need Git for Windows' bash.exe)
"""
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / 'tools'
SCRIPT = TOOLS / 'provision_worktree_assets.ps1'
INSTALLER = TOOLS / 'install_worktree_assets_hook.ps1'
POWERSHELL = shutil.which('powershell') or shutil.which('pwsh')
GIT_IDENTITY = ['-c', 'user.name=test', '-c', 'user.email=test@example.invalid']
HUMMER_MESSAGE = 'local\\ is a junction (shared data root); not provisioning local'


def git_bash_path():
    """The bash.exe the installer looks for: Program Files first, then the Git install that git.exe comes from."""
    candidate = Path(os.environ.get('ProgramFiles', r'C:\Program Files')) / 'Git' / 'bin' / 'bash.exe'
    if candidate.is_file():
        return candidate
    git = shutil.which('git')
    if git:
        candidate = Path(git).parent.parent / 'bin' / 'bash.exe'
        if candidate.is_file():
            return candidate
    return None


def write(path, text='x'):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding='utf-8')


def make_junction(link, target):
    target.mkdir(parents=True, exist_ok=True)
    link.parent.mkdir(parents=True, exist_ok=True)
    # shell=True so the quotes reach cmd.exe as written; list arguments would escape them and mklink would fail.
    subprocess.run(f'mklink /J "{link}" "{target}"', shell=True, check=True, capture_output=True)


def powershell_file(script, *args, cwd=None):
    return subprocess.run([POWERSHELL, '-NoLogo', '-NoProfile', '-NonInteractive', '-File', str(script), *args],
                          capture_output=True, text=True, encoding='utf-8', errors='replace', cwd=cwd)


@unittest.skipUnless(sys.platform == 'win32' and POWERSHELL, 'needs Windows PowerShell')
class ProvisionTest(unittest.TestCase):
    def setUp(self):
        self._folder = tempfile.TemporaryDirectory()
        self.addCleanup(self._folder.cleanup)
        self.base = Path(self._folder.name)
        self.worktree = self.make_worktree(self.base / 'worktree')
        self.seed = self.base / 'seed'
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

    def make_worktree(self, path):
        write(path / 'host' / 'ue5' / 'OpenWillow' / 'OpenWillow.uproject', '{}')
        subprocess.run(['git', 'init', '-q', str(path)], check=True)
        return path

    def run_script(self, *extra, worktree=None, seed=None):
        command = [POWERSHELL, '-NoLogo', '-NoProfile', '-NonInteractive', '-File', str(SCRIPT),
                   '-WorktreeRoot', str(worktree or self.worktree), '-SeedDirectory', str(seed or self.seed), *extra]
        return subprocess.run(command, capture_output=True, text=True, encoding='utf-8', errors='replace')

    def run_command(self, call):
        return subprocess.run([POWERSHELL, '-NoLogo', '-NoProfile', '-NonInteractive', '-Command', call],
                              capture_output=True, text=True, encoding='utf-8', errors='replace')

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
        result = self.run_command(call)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('local/nosuchfolder: 0 missing files added (not in the seed)', result.stdout)
        self.assertIn('local/slice: 0 missing files added', result.stdout)

    def test_unsafe_folder_name_is_refused(self):
        result = self.run_script('-ExtraSeedFolders', '..\\escape')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.worktree.parent / 'escape').exists())

    def test_seed_folder_named_for_an_excluded_name_is_skipped(self):
        write(self.seed / 'logs' / 'kept.txt', 'log')
        call = f"& '{SCRIPT}' -WorktreeRoot '{self.worktree}' -SeedDirectory '{self.seed}' -SeedFolders 'slice','logs'"
        result = self.run_command(call)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertFalse(self.local('logs').exists())
        self.assertIn('local/logs: skipped (excluded name)', result.stdout)

    def test_top_level_excluded_folder_in_a_source_worktree_is_skipped(self):
        source = self.base / 'source'
        write(source / 'host' / 'ue5' / 'OpenWillow' / 'Content' / 'a.umap', 'map')
        write(source / 'local' / 'logs' / 'run.log', 'log')
        write(source / 'local' / 'slice' / 'x.json', 'kept')
        write(source / 'local' / 'range' / 'shots' / 'frame.png', 'shot')
        result = self.run_script('-SourceWorktree', str(source))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertFalse(self.local('logs').exists())
        self.assertFalse(self.local('range', 'shots').exists())
        self.assertEqual(self.local('slice', 'x.json').read_text(encoding='utf-8'), 'kept')

    def test_junction_on_a_folder_destination_is_refused(self):
        elsewhere = self.base / 'elsewhere'
        elsewhere.mkdir()
        make_junction(self.local('doors'), elsewhere)
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('junction or symlink', result.stdout + result.stderr)
        self.assertEqual(list(elsewhere.iterdir()), [])     # nothing was written through the junction

    def test_refusal_leaves_earlier_destinations_untouched(self):
        # The list order is Content, items, character, ui, slice, doors: slice and Content are planned and checked before doors refuses.
        write(self.seed / 'slice' / 'extra.json', 'extra')
        elsewhere = self.base / 'elsewhere'
        elsewhere.mkdir()
        make_junction(self.local('doors'), elsewhere)
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.local('slice', 'extra.json').exists())
        self.assertFalse((self.worktree / 'host' / 'ue5' / 'OpenWillow' / 'Content' / 'Maps' / 'a.umap').exists())

    def test_junctioned_local_is_skipped_with_one_line(self):
        hummer = self.make_worktree(self.base / 'hummer')
        shared = self.base / 'shared-data'
        write(shared / 'slice' / 'shared.json', 'shared')
        make_junction(hummer / 'local', shared)
        result = self.run_script(worktree=hummer)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout.count(HUMMER_MESSAGE), 1)
        self.assertNotIn('local/slice', result.stdout)                 # the local pairs are not planned at all
        self.assertTrue((hummer / 'host' / 'ue5' / 'OpenWillow' / 'Content' / 'Maps' / 'a.umap').is_file())
        self.assertFalse((shared / 'doors').exists())                  # nothing written into the shared root
        self.assertEqual(sorted(p.name for p in shared.iterdir()), ['slice'])

    def test_junction_in_the_seed_is_refused(self):
        seed = self.base / 'seed-junction'
        write(seed / 'Content' / 'a.umap', 'map')
        write(seed / 'slice' / 'manifest.json', 'seed')
        real = self.base / 'real-doors'
        write(real / 'mover.json', 'door')
        make_junction(seed / 'doors', real)
        result = self.run_script(seed=seed)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('junction or symlink', result.stdout + result.stderr)
        self.assertFalse(self.local('doors').exists())


@unittest.skipUnless(sys.platform == 'win32' and POWERSHELL and git_bash_path(),
                     'needs Windows PowerShell and Git for Windows bash.exe (the installer runs the hook with it)')
class InstallerTest(unittest.TestCase):
    """installer -> seed -> post-checkout hook -> a new worktree, in a synthetic repository."""

    def setUp(self):
        self._folder = tempfile.TemporaryDirectory()
        self.addCleanup(self._folder.cleanup)
        self.base = Path(self._folder.name)
        self.repo = self.base / 'repo'
        for name in ('install_worktree_assets_hook.ps1', 'provision_worktree_assets.ps1', 'worktree_seed_folders.txt'):
            write(self.repo / 'tools' / name, (TOOLS / name).read_text(encoding='utf-8'))
        write(self.repo / '.githooks' / 'post-checkout', (ROOT / '.githooks' / 'post-checkout').read_text(encoding='utf-8'))
        write(self.repo / 'host' / 'ue5' / 'OpenWillow' / 'OpenWillow.uproject', '{}')
        write(self.repo / '.gitignore', 'local/\nhost/ue5/OpenWillow/Content/\n')
        write(self.repo / 'host' / 'ue5' / 'OpenWillow' / 'Content' / 'Maps' / 'a.umap', 'map')   # ignored, so the seed supplies it
        write(self.repo / 'local' / 'slice' / 'x.json', 'slice')
        write(self.repo / 'local' / 'doors' / 'mover.json', 'door')
        write(self.repo / 'local' / 'ui' / 'run' / 'previews' / 'p.png', 'preview')
        write(self.repo / 'local' / 'range' / 'shots' / 'frame.png', 'shot')
        write(self.repo / 'local' / 'sanctuary' / 'big.bin', 'big')
        subprocess.run(['git', 'init', '-q', str(self.repo)], check=True)
        subprocess.run(['git', *GIT_IDENTITY, '-C', str(self.repo), 'add', 'tools', '.githooks', '.gitignore',
                        'host/ue5/OpenWillow/OpenWillow.uproject'], check=True)
        subprocess.run(['git', *GIT_IDENTITY, '-C', str(self.repo), 'commit', '-q', '-m', 'synthetic'], check=True)

    def install(self):
        return powershell_file(self.repo / 'tools' / 'install_worktree_assets_hook.ps1', cwd=str(self.repo))

    def add_worktree(self, name):
        return subprocess.run(['git', *GIT_IDENTITY, '-C', str(self.repo), 'worktree', 'add', '-q', str(self.base / name), '-b', name],
                              capture_output=True, text=True, encoding='utf-8', errors='replace')

    def test_installer_seeds_the_list_and_the_hook_provisions_a_new_worktree(self):
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        seed = self.repo / 'local' / 'worktree-seed'
        self.assertEqual((seed / 'slice' / 'x.json').read_text(encoding='utf-8'), 'slice')
        self.assertTrue((seed / 'doors' / 'mover.json').is_file())
        self.assertTrue((seed / 'ui' / 'run' / 'previews' / 'p.png').is_file())
        self.assertTrue((seed / 'Content' / 'Maps' / 'a.umap').is_file())
        self.assertTrue((seed / 'worktree_seed_folders.txt').is_file())
        self.assertTrue((seed / 'provision_worktree_assets.ps1').is_file())
        self.assertFalse((seed / 'range' / 'shots').exists())     # per-run output is never seeded
        self.assertFalse((seed / 'sanctuary').exists())           # not in the default list

        added = self.add_worktree('w1')
        self.assertEqual(added.returncode, 0, added.stdout + added.stderr)
        new = self.base / 'w1'
        self.assertEqual((new / 'local' / 'slice' / 'x.json').read_text(encoding='utf-8'), 'slice')
        self.assertTrue((new / 'local' / 'doors' / 'mover.json').is_file())
        self.assertTrue((new / 'host' / 'ue5' / 'OpenWillow' / 'Content' / 'Maps' / 'a.umap').is_file())
        self.assertFalse((new / 'local' / 'range' / 'shots').exists())
        self.assertFalse((new / 'local' / 'sanctuary').exists())

    def test_failing_hook_leaves_the_checkout_in_place_but_fails_the_command(self):
        self.assertEqual(self.install().returncode, 0)
        (self.repo / 'local' / 'worktree-seed' / 'worktree_seed_folders.txt').unlink()   # an old or broken seed
        added = self.add_worktree('w2')
        self.assertNotEqual(added.returncode, 0)            # git reports the hook's failure...
        self.assertTrue((self.base / 'w2' / 'host').is_dir())   # ...after the checkout already happened

    def test_reinstall_refreshes_the_cached_provisioner(self):
        self.assertEqual(self.install().returncode, 0)
        cached = self.repo / 'local' / 'worktree-seed' / 'provision_worktree_assets.ps1'
        cached.write_text('# an old copy\n', encoding='utf-8')
        self.assertEqual(self.install().returncode, 0)
        self.assertEqual(cached.read_text(encoding='utf-8'), (self.repo / 'tools' / 'provision_worktree_assets.ps1').read_text(encoding='utf-8'))

    def git_in(self, worktree, *args, env=None):
        return subprocess.run(['git', *GIT_IDENTITY, '-C', str(worktree), *args], capture_output=True, text=True,
                              encoding='utf-8', errors='replace', env=env)

    def test_hook_provisions_a_new_worktree_but_not_a_later_branch_switch(self):
        self.assertEqual(self.install().returncode, 0)
        self.git_in(self.repo, 'branch', 'other')
        added = self.add_worktree('w3')
        self.assertEqual(added.returncode, 0, added.stdout + added.stderr)
        new = self.base / 'w3'
        slice_file = new / 'local' / 'slice' / 'x.json'
        self.assertTrue(slice_file.is_file())                     # the new worktree was provisioned
        slice_file.unlink()
        switched = self.git_in(new, 'checkout', '-q', 'other')    # an existing worktree: the hook must skip
        self.assertEqual(switched.returncode, 0, switched.stdout + switched.stderr)
        self.assertFalse(slice_file.exists())

    def test_hook_skips_a_file_checkout(self):
        self.assertEqual(self.install().returncode, 0)
        added = self.add_worktree('w4')
        self.assertEqual(added.returncode, 0, added.stdout + added.stderr)
        new = self.base / 'w4'
        slice_file = new / 'local' / 'slice' / 'x.json'
        slice_file.unlink()
        restored = self.git_in(new, 'checkout', '--', 'host/ue5/OpenWillow/OpenWillow.uproject')
        self.assertEqual(restored.returncode, 0, restored.stdout + restored.stderr)
        self.assertFalse(slice_file.exists())

    def test_env_var_forces_a_provision_on_a_branch_switch(self):
        self.assertEqual(self.install().returncode, 0)
        self.git_in(self.repo, 'branch', 'forced')
        added = self.add_worktree('w5')
        self.assertEqual(added.returncode, 0, added.stdout + added.stderr)
        new = self.base / 'w5'
        slice_file = new / 'local' / 'slice' / 'x.json'
        slice_file.unlink()
        env = dict(os.environ, OPENWILLOW_PROVISION='1')
        switched = self.git_in(new, 'checkout', '-q', 'forced', env=env)
        self.assertEqual(switched.returncode, 0, switched.stdout + switched.stderr)
        self.assertTrue(slice_file.is_file())                     # forced: the missing file came back
        self.assertEqual(slice_file.read_text(encoding='utf-8'), 'slice')

    def test_no_checkout_worktree_provisions_on_its_first_real_checkout(self):
        self.assertEqual(self.install().returncode, 0)
        self.git_in(self.repo, 'branch', 'nc-a')
        self.git_in(self.repo, 'branch', 'nc-b')
        added = subprocess.run(['git', *GIT_IDENTITY, '-C', str(self.repo), 'worktree', 'add', '-q', '--no-checkout',
                                '--detach', str(self.base / 'nc'), 'nc-a'], capture_output=True, text=True, encoding='utf-8', errors='replace')
        self.assertEqual(added.returncode, 0, added.stdout + added.stderr)
        new = self.base / 'nc'
        slice_file = new / 'local' / 'slice' / 'x.json'
        self.assertFalse(slice_file.exists())                   # --no-checkout: the add did not provision
        first = self.git_in(new, 'checkout', '-q', 'nc-a')      # a real previous HEAD, not zeros
        self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
        self.assertTrue(slice_file.is_file())                   # no marker yet, so the first real checkout provisioned
        git_dir = self.git_in(new, 'rev-parse', '--path-format=absolute', '--git-dir').stdout.strip()
        self.assertTrue((Path(git_dir) / 'openwillow-provisioned').is_file())
        slice_file.unlink()
        switched = self.git_in(new, 'checkout', '-q', 'nc-b')   # marked now: a branch switch is skipped
        self.assertEqual(switched.returncode, 0, switched.stdout + switched.stderr)
        self.assertFalse(slice_file.exists())

    def test_junctioned_seed_is_refused_before_anything_is_written(self):
        elsewhere = self.base / 'elsewhere-seed'
        elsewhere.mkdir()
        make_junction(self.repo / 'local' / 'worktree-seed', elsewhere)
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('junction or symlink', result.stdout + result.stderr)
        self.assertEqual(list(elsewhere.iterdir()), [])             # no cached copy, no fill, no manifest


if __name__ == '__main__':
    unittest.main()
