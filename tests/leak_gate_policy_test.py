"""Synthetic repository tests for tools/leak_gate.py's policy: trusted lists from origin/main, the fallback warning,
gitlinks and unreadable objects, local/ copies, staged and whole-tree modes, exit codes and output that never carries a trigger.
Split from tests/leak_gate_test.py (which holds the shared fixture) to keep each ctest entry well under its time limit.
"""
import os
import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import leak_gate_test as common  # noqa: E402  (the shared fixture and trigger strings)

from leak_gate_test import ALLOW, EMAIL, PRIVATE, STEAM, TOKEN, TRIGGERS, USER_PATH, git, run_gate  # noqa: E402


class PolicyTests(common.RepoFixture, unittest.TestCase):
    def test_lists_come_from_origin_main(self):
        self.write('tools/leak_gate_allow.txt', ALLOW + '\nlocal-path local/* -- sneaky\n')
        self.write('.gitignore', '')
        self.commit('local/out.md', 'x\n')
        code, out = self.range_gate()
        self.assertEqual(code, 1, out)
        self.assertIn('ERROR local-path [path] loc', out)
        self.assertIn('WARN gate-change', out)
        self.assertIn('tools/leak_gate_allow.txt', out)

    def test_uncommitted_lists_never_judge(self):
        self.write('tools/leak_gate_allow.txt', ALLOW + '\nuser-path *.md -- sneaky\n')  # not committed
        self.write('staged.md', f'see {USER_PATH}\n')
        git(self.repo, 'add', 'staged.md')
        code, out = run_gate(self.repo, '--staged')
        self.assertEqual(code, 1, out)
        self.assertIn('user-path staged.md:1', out)
        code, out = run_gate(self.repo, '--staged', '--worktree-config')
        self.assertEqual(code, 2, out)

    def test_branch_that_merged_main_still_uses_main_lists(self):
        git(self.repo, 'checkout', '-q', '-b', 'topic')
        self.write('tools/leak_gate_allow.txt', ALLOW + '\nlocal-path local/* -- sneaky\n')
        self.write('.gitignore', '')
        self.commit('topic.md', 'topic\n')
        git(self.repo, 'checkout', '-q', 'main')
        self.commit('main.md', 'main moves on\n')
        git(self.repo, 'push', '-q', 'origin', 'main')
        git(self.repo, 'checkout', '-q', 'topic')
        git(self.repo, 'merge', '-q', '--no-edit', 'main')
        self.commit('local/out.md', 'x\n')
        code, out = run_gate(self.repo, '--range', 'origin/main..HEAD')
        self.assertEqual(code, 1, out)
        self.assertIn('ERROR local-path', out)

    def test_no_trusted_lists_refuses_unless_bootstrapping(self):
        git(self.repo, 'update-ref', '-d', 'refs/remotes/origin/main')
        self.commit('a.md', 'fine\n')
        code, out = self.range_gate()
        self.assertEqual(code, 2, out)
        self.assertIn('nothing can be judged', out)
        code, out = run_gate(self.repo, '--staged')
        self.assertEqual(code, 2, out)
        code, out = run_gate(self.repo, '--range', f'{self.base}..HEAD', '--config-from', self.base)
        self.assertEqual(code, 0, out)
        self.assertIn('named with --config-from', out)

    def test_replace_refs_do_not_hide_content(self):
        leak = self.commit('leak.md', f'see {USER_PATH}\n')
        bad = git(self.repo, 'rev-parse', f'{leak}:leak.md').stdout.decode().strip()
        good = self.blob('nothing to see\n')
        git(self.repo, 'replace', bad, good)
        self.assertEqual(git(self.repo, 'show', f'{leak}:leak.md').stdout, b'nothing to see\n')  # replaced view
        code, out = self.range_gate()
        self.assertEqual(code, 1, out)
        self.assertIn('user-path leak.md:1', out)

    def test_paths_starting_with_pr_and_ref_are_masked(self):
        self.commit('pr/Fake_P.upk', 'x\n')
        self.commit('refs-notes/Other_P.upk', 'x\n')
        code, out = self.range_gate()
        self.assertEqual(code, 1, out)
        self.assertNotIn('pr/Fake_P.upk', out)
        self.assertNotIn('refs-notes/Other_P.upk', out)
        self.assertIn('ERROR forbidden-type [path]', out)

    def test_gitlink_and_unreadable_blob(self):
        self.commit_tree_entry('160000', self.base, 'vendor/sub')
        code, out = self.range_gate()
        self.assertEqual(code, 1, out)
        self.assertIn('gitlink vendor/sub', out)
        git(self.repo, 'reset', '-q', '--hard', self.base)
        self.commit('lost.md', 'content that goes missing\n')
        sha = git(self.repo, 'rev-parse', 'HEAD:lost.md').stdout.decode().strip()
        loose = Path(self.repo, '.git', 'objects', sha[:2], sha[2:])
        os.chmod(loose, 0o666)  # loose objects are read-only on Windows
        os.remove(loose)
        code, out = self.range_gate()
        self.assertEqual(code, 2, out)

    def test_local_copy_is_a_warning(self):
        body = '{"rows": "' + 'generated output line ' * 5 + '"}\n'
        self.write('local/p2/out.json', body)
        self.commit('tools/copied.txt', body)
        code, out = self.range_gate()
        self.assertEqual(code, 0, out)
        self.assertIn('WARN local-copy tools/copied.txt', out)
        os.remove(Path(self.repo, 'local', 'p2', 'out.json'))
        self.write('local/review/tools/copied.txt', body)  # a mirror of the repository is not a leak
        code, out = self.range_gate()
        self.assertEqual(code, 0, out)
        self.assertNotIn('local-copy', out)

    def test_staged_and_all(self):
        self.write('x.md', f'{STEAM}\n')
        git(self.repo, 'add', 'x.md')
        code, out = run_gate(self.repo, '--staged')
        self.assertEqual(code, 1, out)
        self.assertIn('account-id x.md:1', out)
        code, out = run_gate(self.repo, '--all', 'HEAD')
        self.assertEqual(code, 1, out)  # the e-mail line in notes.md, not baselined here
        self.assertIn('email notes.md:1', out)

    def test_exit_codes(self):
        code, out = run_gate(self.repo, '--range', 'no-such-ref..HEAD')
        self.assertEqual(code, 2, out)
        code, out = run_gate(self.repo, '--text', '-', stdin='A plain PR description.\n')
        self.assertEqual(code, 0, out)
        code, out = run_gate(self.repo, '--text', '-', stdin=f'Ping {EMAIL}\n')
        self.assertEqual(code, 1, out)
        code, out = run_gate(self.repo, '--pre-push', 'origin', stdin='not a ref line\n')
        self.assertEqual(code, 2, out)
        self.write('tools/leak_gate_allow.txt', 'broken line\n')
        code, out = run_gate(self.repo, '--all', 'HEAD', '--worktree-config')
        self.assertEqual(code, 2, out)

    def test_output_never_carries_a_trigger(self):
        names = Path(self.folder, 'names.txt')
        names.write_text(PRIVATE + '\n', encoding='utf-8')
        self.commit(f'docs/{PRIVATE}/{STEAM}.md', f'{EMAIL}\n{USER_PATH}\n{TOKEN}\n{STEAM}\nthe {PRIVATE} PC\n')
        outputs = []
        for args in (('--range', f'{self.base}..HEAD'), ('--all', 'HEAD')):
            code, out = run_gate(self.repo, *args, names=str(names))
            self.assertEqual(code, 1, out)
            outputs.append(out)
        self.write('tools/leak_gate_allow.txt', f'{EMAIL} *.md -- {USER_PATH}\n')  # an invalid rule, echoed?
        code, out = run_gate(self.repo, '--all', 'HEAD', '--worktree-config', names=str(names))
        self.assertEqual(code, 2, out)
        outputs.append(out)
        for out in outputs:
            for trigger in TRIGGERS:
                self.assertNotIn(trigger, out)


    def test_deleting_an_enforcement_file_is_loud(self):
        self.commit('.gitattributes', '*.sh text eol=lf\n')
        git(self.repo, 'push', '-q', 'origin', 'main')
        before = self.head()
        git(self.repo, 'rm', '-q', '.gitattributes')
        git(self.repo, 'commit', '-q', '-m', 'drop attributes')
        code, out = self.range_gate(before)
        self.assertIn('WARN gate-change (range) [DELETES an enforcement file', out)
        self.assertIn('.gitattributes', out)

    def test_another_remotes_cache_hides_nothing(self):
        git(self.repo, 'checkout', '-q', '--orphan', 'loose')
        git(self.repo, 'rm', '-q', '-r', '--cached', '.')
        self.commit('payload.md', f'see {USER_PATH}\n')
        git(self.repo, 'update-ref', 'refs/remotes/other/cache', 'HEAD')
        code, out = run_gate(self.repo, '--range', '..HEAD')
        self.assertEqual(code, 1, out)
        self.assertIn('user-path payload.md:1', out)

    def test_arguments_and_labels_are_never_echoed(self):
        code, out = run_gate(self.repo, '--range', f'{self.base}..HEAD', f'--no-such-flag={TOKEN}')
        self.assertEqual(code, 2, out)
        self.assertNotIn(TOKEN, out)
        code, out = run_gate(self.repo, '--text', '-', '--label', TOKEN, stdin='plain\n')
        self.assertEqual(code, 0, out)
        self.assertNotIn(TOKEN, out)
        self.commit(f'refs/{TOKEN}.md', 'x\n')
        code, out = self.range_gate()
        self.assertEqual(code, 1, out)
        self.assertNotIn(TOKEN, out)

    def test_text_without_a_bom_in_utf16_is_refused(self):
        proc = subprocess.run([sys.executable, str(Path(self.repo, 'tools', 'leak_gate.py')), '--text', '-'],
                              cwd=self.repo, input=f'see {USER_PATH}\n'.encode('utf-16-le'), capture_output=True,
                              env=common.gate_env())
        self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
        self.assertIn(b'ERROR binary', proc.stdout)

    def test_commit_message_with_nul_is_refused(self):
        tree = git(self.repo, 'write-tree').stdout.decode().strip()
        ident = f'bot <{common.NOREPLY}> 1700000000 +0000'
        raw = f'tree {tree}\nparent {self.head()}\nauthor {ident}\ncommitter {ident}\n\nhidden\0{USER_PATH}\n'
        sha = git(self.repo, 'hash-object', '-t', 'commit', '-w', '--stdin', '--literally',
                  data=raw.encode()).stdout.decode().strip()
        git(self.repo, 'update-ref', 'HEAD', sha)
        code, out = self.range_gate()
        self.assertEqual(code, 1, out)
        self.assertIn('ERROR binary commit:', out)

    def test_strict_fails_on_warnings(self):
        self.commit('tools/x.toml', 'a = 1\n')  # unknown-type: a warning
        code, out = self.range_gate()
        self.assertEqual(code, 0, out)
        self.assertIn('warning(s) (unknown-type)', out)
        code, out = run_gate(self.repo, '--range', f'{self.base}..HEAD', '--strict')
        self.assertEqual(code, 3, out)
        self.assertIn('refused under --strict: unknown-type', out)
        git(self.repo, 'reset', '-q', '--hard', self.base)
        self.commit('clean.md', 'clean\n')
        code, out = run_gate(self.repo, '--range', f'{self.base}..HEAD', '--strict')
        self.assertEqual(code, 0, out)

    def test_no_trusted_exclude_for_a_push_to_main(self):
        leak = self.commit('leak.md', f'see {USER_PATH}\n')
        git(self.repo, 'push', '-q', 'origin', 'main')  # as after a push: origin/main sits at the pushed head
        code, out = self.range_gate()
        self.assertEqual(code, 0, out)
        self.assertIn('0 commit(s) scanned', out)
        code, out = run_gate(self.repo, '--range', f'{self.base}..HEAD', '--no-trusted-exclude')
        self.assertEqual(code, 1, out)
        self.assertIn(f'user-path leak.md:1 @{leak[:12]}', out)

    def test_shallow_and_grafted_histories_are_refused(self):
        self.commit('one.md', 'one\n')
        self.commit('two.md', 'two\n')
        git(self.repo, 'push', '-q', 'origin', 'main')
        shallow = os.path.join(self.folder, 'shallow')
        git(self.folder, 'clone', '-q', '--depth', '1', 'file://' + self.remote.replace(os.sep, '/'), shallow)
        for key, value in (('user.name', 'bot'), ('user.email', common.NOREPLY)):
            git(shallow, 'config', key, value)
        Path(shallow, 'three.md').write_text('three\n', encoding='utf-8')
        git(shallow, 'add', '-A')
        git(shallow, 'commit', '-q', '-m', 'three')
        code, out = run_gate(shallow, '--range', '..HEAD', '--whole-history')
        self.assertEqual(code, 2, out)
        self.assertIn('shallow', out)
        head = self.head()
        parent = git(self.repo, 'rev-parse', 'HEAD~1').stdout.decode().strip()
        grafts = Path(git(self.repo, 'rev-parse', '--path-format=absolute', '--git-path', 'info/grafts').stdout.decode().strip())
        grafts.parent.mkdir(parents=True, exist_ok=True)
        grafts.write_text(f'{parent}\n', encoding='utf-8')  # makes the parent look like a root
        self.commit('four.md', 'four\n')
        code, out = run_gate(self.repo, '--range', '..HEAD', '--whole-history')
        self.assertEqual(code, 2, out)
        self.assertIn('grafts', out)

if __name__ == '__main__':
    unittest.main()
