"""Synthetic tests for the leak gate's pre-push hook, the trust side: a push whose trusted copy cannot be refreshed
is refused (only a bootstrap naming a commit id passes), a stale tracking ref is refreshed first, an interpreter
or module planted in the checkout is never used, and malformed hook input exits 2. Split from
tests/leak_gate_hook_test.py to keep each ctest entry well under its time limit.
"""
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import leak_gate_test as common  # noqa: E402  (the shared fixture and trigger strings)

from leak_gate_test import ALLOW, FUN, PRIVATE, USER_PATH, gate_env, git, run_gate  # noqa: E402


class PushTests(common.RepoFixture, unittest.TestCase):
    def install_hook(self):
        code, out = run_gate(self.repo, '--install-hook')
        self.assertEqual(code, 0, out)
        hook = Path(git(self.repo, 'rev-parse', '--git-path', 'hooks').stdout.decode().strip())
        if not hook.is_absolute():
            hook = Path(self.repo, hook)
        self.assertNotIn(b'\r\n', (hook / 'pre-push').read_bytes())
        return hook / 'pre-push'

    def push(self, *refs, **env):
        return git(self.repo, 'push', 'origin', *(refs or ('main',)), env=gate_env(**env), check=False)

    def test_no_trusted_copy_is_refused(self):
        self.install_hook()
        git(self.repo, 'update-ref', '-d', 'refs/remotes/origin/main')
        git(self.repo, 'remote', 'set-url', 'origin', os.path.join(self.folder, 'missing.git'))
        self.commit('clean.md', 'clean\n')
        proc = git(self.repo, 'push', self.remote, 'main', env=gate_env(), check=False)
        self.assertNotEqual(proc.returncode, 0, proc.text)
        self.assertIn('could not be refreshed from origin', proc.text)
        proc = git(self.repo, 'push', self.remote, 'main', env=gate_env(OPENWILLOW_LEAK_GATE_BOOTSTRAP='1'), check=False)
        self.assertNotEqual(proc.returncode, 0, proc.text)  # a bare flag is not a commit id
        head = git(self.repo, 'rev-parse', 'HEAD').stdout.decode().strip()
        proc = git(self.repo, 'push', self.remote, 'main', env=gate_env(OPENWILLOW_LEAK_GATE_BOOTSTRAP=head), check=False)
        self.assertEqual(proc.returncode, 0, proc.text)
        self.assertIn(f'BOOTSTRAP run: judging this push with the gate and lists of commit {head}', proc.text)
        ours = [line for line in proc.text.splitlines() if line.startswith(('leak', 'ERROR', 'WARN'))]
        self.assertFalse([line for line in ours if 'missing.git' in line or self.remote in line], ours)

    def test_stale_trusted_ref_is_refreshed(self):
        self.install_hook()
        self.commit('leak.md', f'see {USER_PATH}\n')
        # Main on origin revokes nothing here, but the local tracking ref is moved back to a commit without lists:
        git(self.repo, 'update-ref', 'refs/remotes/origin/main', self.base)
        git(self.repo, 'commit', '-q', '--allow-empty', '-m', 'x')
        proc = self.push()
        self.assertNotEqual(proc.returncode, 0, proc.text)  # refreshed first, then judged by origin's main
        self.assertIn('user-path leak.md:1', proc.text)

    def test_interpreter_and_modules_from_the_checkout_are_not_used(self):
        self.install_hook()
        self.commit('leak.md', f'see {USER_PATH}\n')
        fake = Path(self.repo, 'bin')
        fake.mkdir()
        for name in ('python3', 'python', 'py'):
            (fake / name).write_text('#!/bin/sh\nexit 0\n', encoding='utf-8')
            os.chmod(fake / name, 0o755)
        Path(self.repo, 'argparse.py').write_text('raise SystemExit(0)\n', encoding='utf-8')
        env = gate_env(PATH=str(fake) + os.pathsep + os.environ.get('PATH', ''), PYTHONPATH=self.repo)
        env.pop('OPENWILLOW_PYTHON', None)
        proc = git(self.repo, 'push', 'origin', 'main', env=env, check=False)
        self.assertNotEqual(proc.returncode, 0, proc.text)
        self.assertIn('user-path leak.md:1', proc.text)

    def test_malformed_hook_input_is_refused(self):
        sha = git(self.repo, 'rev-parse', 'HEAD').stdout.decode().strip()
        for line in (f'  {sha}  {sha}\n', f'refs/heads/main {sha} {sha}\n', 'x y z w\n'):
            code, out = run_gate(self.repo, '--pre-push', 'origin', stdin=line)
            self.assertEqual(code, 2, (line, out))


    def test_head_and_raw_id_sources_are_accepted(self):
        self.install_hook()
        self.commit('clean.md', 'clean\n')
        sha = git(self.repo, 'rev-parse', 'HEAD').stdout.decode().strip()
        for spec in ('HEAD:refs/heads/x', f'{sha}:refs/heads/y'):
            proc = self.push(spec)
            self.assertEqual(proc.returncode, 0, (spec, proc.text))
        self.commit('leak.md', f'see {USER_PATH}\n')
        sha = git(self.repo, 'rev-parse', 'HEAD').stdout.decode().strip()
        for spec in ('HEAD:refs/heads/x2', f'{sha}:refs/heads/y2'):
            proc = self.push(spec)
            self.assertNotEqual(proc.returncode, 0, (spec, proc.text))
            self.assertIn('user-path leak.md:1', proc.text)

    def test_interpreter_in_a_tmp_spelt_checkout_is_skipped(self):
        if os.name != 'nt':
            self.skipTest('the /tmp spelling of the temporary folder is a Git Bash case')
        temp = os.path.realpath(tempfile.gettempdir())
        repo = os.path.realpath(self.repo)
        if not repo.lower().startswith(temp.lower() + os.sep):
            self.skipTest('the test checkout is not under the temporary folder')
        self.install_hook()
        self.commit('leak.md', f'see {USER_PATH}\n')
        fake = Path(self.repo, 'bin')
        fake.mkdir()
        for name in ('python3', 'python', 'py'):
            (fake / name).write_text('#!/bin/sh\nexit 0\n', encoding='utf-8')
        tmp_spelt = '/tmp/' + os.path.relpath(str(fake), temp).replace(os.sep, '/')
        env = gate_env(PATH=tmp_spelt + os.pathsep + os.environ.get('PATH', ''))
        env.pop('OPENWILLOW_PYTHON', None)
        proc = git(self.repo, 'push', 'origin', 'main', env=env, check=False)
        self.assertNotEqual(proc.returncode, 0, proc.text)
        self.assertIn('user-path leak.md:1', proc.text)

if __name__ == '__main__':
    unittest.main()
