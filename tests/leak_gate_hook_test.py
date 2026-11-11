"""Synthetic tests for the leak gate's pre-push hook (.githooks/pre-push): it refuses a leak, judges every ref
and tag on its own, runs origin/main's gate even on an old branch or over a replaced working-tree copy, and fails
closed without Python. Split from tests/leak_gate_test.py to keep each ctest entry well under its time limit.
"""
import os
import shutil
import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import leak_gate_test as common  # noqa: E402  (the shared fixture and trigger strings)

from leak_gate_test import ALLOW, FUN, PRIVATE, USER_PATH, gate_env, git, run_gate  # noqa: E402


class HookTests(common.RepoFixture, unittest.TestCase):
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

    def test_pre_push_hook_refuses(self):
        self.install_hook()
        self.commit('clean.md', 'a clean change\n')
        proc = self.push()
        self.assertEqual(proc.returncode, 0, proc.text)
        self.commit('leak.md', f'see {USER_PATH}\n')
        proc = self.push()
        self.assertNotEqual(proc.returncode, 0, proc.text)
        self.assertIn('user-path leak.md:1', proc.text)
        self.assertIn('push refused', proc.text)
        git(self.repo, 'checkout', '-q', '-b', 'topic', 'HEAD~1')
        self.commit('topic.md', f'{FUN}\n')
        proc = self.push('topic')
        self.assertNotEqual(proc.returncode, 0, proc.text)
        self.assertIn('decompiler-mark topic.md:1', proc.text)

    def test_hook_ignores_a_replaced_working_tree_gate(self):
        self.install_hook()
        self.commit('leak.md', f'see {USER_PATH}\n', add_all=False)
        self.write('tools/leak_gate.py', 'import sys\nsys.exit(0)\n')  # uncommitted replacement
        proc = self.push()
        self.assertNotEqual(proc.returncode, 0, proc.text)
        self.assertIn('user-path leak.md:1', proc.text)

    def test_hook_judges_every_ref_and_tags(self):
        self.install_hook()
        git(self.repo, 'checkout', '-q', '-b', 'clean')
        self.commit('clean.md', 'clean\n')
        git(self.repo, 'checkout', '-q', '-b', 'dirty', 'main')
        self.commit('dirty.md', f'see {USER_PATH}\n')
        for order in (('clean', 'dirty'), ('dirty', 'clean')):
            proc = self.push(*order)
            self.assertNotEqual(proc.returncode, 0, (order, proc.text))
            self.assertIn('user-path dirty.md:1', proc.text)
        git(self.repo, 'tag', '-a', 'v1', 'clean', '-m', f'release notes {USER_PATH}')
        proc = self.push('v1')
        self.assertNotEqual(proc.returncode, 0, proc.text)
        self.assertIn('user-path ref:tag-', proc.text)
        names = Path(self.folder, 'names.txt')
        names.write_text(PRIVATE + '\n', encoding='utf-8')
        git(self.repo, 'branch', f'feature-{PRIVATE}', 'clean')
        proc = self.push(f'feature-{PRIVATE}', OPENWILLOW_LEAK_NAMES=str(names))
        self.assertNotEqual(proc.returncode, 0, proc.text)
        self.assertIn('private-name ref-name', proc.text)

    def test_hook_runs_main_gate_on_an_old_branch(self):
        hook = self.install_hook()
        git(self.repo, 'checkout', '-q', '--orphan', 'old')
        git(self.repo, 'rm', '-q', '-r', '--cached', '.')
        for name in ('tools', '.githooks'):
            shutil.rmtree(Path(self.repo, name))
        os.remove(Path(self.repo, 'notes.md'))
        self.write('tools/leak_gate_allow.txt', ALLOW + '\nuser-path *.md -- the branch exempts itself\n')
        self.commit('old.md', 'an old branch\n')
        self.assertFalse(Path(self.repo, 'tools', 'leak_gate.py').exists())
        self.assertTrue(hook.exists())
        proc = self.push('old')
        self.assertEqual(proc.returncode, 0, proc.text)
        self.commit('old2.md', f'see {USER_PATH}\n')
        proc = self.push('old')
        self.assertNotEqual(proc.returncode, 0, proc.text)
        self.assertIn('user-path old2.md:1', proc.text)

    def test_hook_without_python_refuses(self):
        hook = self.install_hook()
        sh = shutil.which('sh')
        gitexe = shutil.which('git')
        if not sh or not gitexe:
            self.skipTest('no sh or git on PATH')
        path = os.pathsep.join(sorted({os.path.dirname(sh), os.path.dirname(gitexe)}))
        if any(shutil.which(name, path=path) for name in ('python3', 'python', 'py')):
            self.skipTest('a Python sits next to sh or git here; cannot hide it')
        env = dict(os.environ, PATH=path, OPENWILLOW_PYTHON='')
        proc = subprocess.run([sh, str(hook), 'origin', self.remote], cwd=self.repo, input=b'', capture_output=True,
                              env=env)
        self.assertEqual(proc.returncode, 1, proc.stderr)
        self.assertIn(b'no Python 3 outside the checkout found', proc.stderr)


    def test_push_to_a_url_still_uses_origin_main(self):
        self.install_hook()
        self.commit('leak.md', f'see {USER_PATH}\n', add_all=False)
        self.write('tools/leak_gate.py', 'import sys\nsys.exit(0)\n')  # a neutered working-tree gate
        proc = git(self.repo, 'push', self.remote, 'main', env=gate_env(), check=False)
        self.assertNotEqual(proc.returncode, 0, proc.text)
        self.assertIn('user-path leak.md:1', proc.text)
        ours = [line for line in proc.text.splitlines() if line.startswith(('leak', 'ERROR', 'WARN'))]
        self.assertFalse([line for line in ours if self.remote in line], ours)  # git's own error line may name it


if __name__ == '__main__':
    unittest.main()
