"""Synthetic repository tests for tools/leak_gate.py's range judging: added lines only, copies of baselined lines,
intermediate commits, awkward file names, git attributes, commit messages and identities.
Split from tests/leak_gate_test.py (which holds the shared fixture) to keep each ctest entry well under its time limit.
"""
import os
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import leak_gate_test as common  # noqa: E402  (the shared fixture and trigger strings)

from leak_gate_test import EMAIL, FUN, NOREPLY, PRIVATE, USER_PATH, git, run_gate  # noqa: E402


class RangeTests(common.RepoFixture, unittest.TestCase):
    def test_range_scans_added_lines_only(self):
        self.commit('notes.md', f'old line {EMAIL}\nsecond line\nnew clean line\n')
        code, out = self.range_gate()
        self.assertEqual(code, 0, out)
        self.commit('notes.md', f'second line\nold line {EMAIL}\nnew clean line\n')  # a move is not an addition
        code, out = self.range_gate()
        self.assertEqual(code, 0, out)
        self.commit('notes.md', f'second line\nold line {EMAIL}\nnew clean line\nnew {USER_PATH}\n')
        code, out = self.range_gate()
        self.assertEqual(code, 1, out)
        self.assertIn('user-path notes.md:4', out)
        self.assertNotIn('notes.md:2', out)

    def test_a_copy_of_a_baselined_line_fails(self):
        code, out = run_gate(self.repo, '--all', 'HEAD', '--worktree-config', '--write-baseline',
                             'tools/leak_gate_baseline.txt')
        self.assertEqual(code, 0, out)
        self.commit('tools/leak_gate_baseline.txt', Path(self.repo, 'tools/leak_gate_baseline.txt').read_text())
        git(self.repo, 'push', '-q', 'origin', 'main')
        base = self.head()
        self.commit('notes.md', f'old line {EMAIL}\nsecond line\nold line {EMAIL}\n')
        code, out = self.range_gate(base)
        self.assertEqual(code, 1, out)
        self.assertIn('ERROR email notes.md', out)

    def test_intermediate_commits_are_judged(self):
        added = self.commit('docs/tmp.md', f'see {USER_PATH}\n')
        git(self.repo, 'rm', '-q', 'docs/tmp.md')
        git(self.repo, 'commit', '-q', '-m', 'remove it again')
        code, out = self.range_gate()
        self.assertEqual(code, 1, out)
        self.assertIn(f'user-path docs/tmp.md:1 @{added[:12]}', out)

    def test_awkward_file_names(self):
        self.commit('dir x/a b.md', f'see {USER_PATH}\n')
        code, out = self.range_gate()
        self.assertEqual(code, 1, out)
        self.assertIn('user-path dir x/a b.md:1', out)
        for name in ('odd"name.md', 'tab\tname.md', 'new\nline.md'):
            before = self.head()
            self.commit_tree_entry('100644', self.blob(f'see {USER_PATH}\n'), name)
            code, out = self.range_gate(before)
            self.assertEqual(code, 1, (name, out))
            self.assertIn('ERROR user-path', out)

    def test_git_attributes_do_not_hide_lines(self):
        self.commit('.gitattributes', '*.md -diff\n*.txt diff=nothing binary\n')
        before = self.head()
        self.commit('hidden.md', f'see {USER_PATH}\n')
        self.commit('hidden.txt', f'see {FUN}\n')
        code, out = self.range_gate(before)
        self.assertEqual(code, 1, out)
        self.assertIn('user-path hidden.md:1', out)
        self.assertIn('decompiler-mark hidden.txt:1', out)
        code, out = self.range_gate()
        self.assertIn('WARN gate-change', out)
        self.assertIn('.gitattributes', out)

    def test_commit_messages_identities_and_names(self):
        self.commit('a.md', 'fine\n', message=f'note {FUN}')
        code, out = self.range_gate()
        self.assertEqual(code, 1, out)
        self.assertIn('decompiler-mark commit:', out)
        git(self.repo, 'config', 'user.email', EMAIL)
        self.commit('b.md', 'fine\n')
        code, out = run_gate(self.repo, '--range', 'HEAD~1..HEAD')
        self.assertEqual(code, 1, out)
        self.assertIn(':author', out)
        git(self.repo, 'config', 'user.email', NOREPLY)
        names = Path(self.folder, 'names.txt')
        names.write_text(PRIVATE + '\n', encoding='utf-8')
        git(self.repo, 'config', 'user.name', f'{PRIVATE} person')
        self.commit('c.md', 'fine\n')
        code, out = run_gate(self.repo, '--range', 'HEAD~1..HEAD', names=str(names))
        self.assertEqual(code, 1, out)
        self.assertIn('private-name commit:', out)
        self.assertIn('author-name', out)


    def test_merge_second_parent_is_judged(self):
        git(self.repo, 'checkout', '-q', '-b', 'side')
        side = self.commit('side.md', f'see {USER_PATH}\n')
        git(self.repo, 'checkout', '-q', '-b', 'topic', self.base)
        self.commit('topic.md', 'topic\n')
        git(self.repo, 'merge', '-q', '--no-ff', '--no-edit', 'side')
        code, out = self.range_gate()
        self.assertEqual(code, 1, out)
        self.assertIn(f'user-path side.md:1 @{side[:12]}', out)

    def test_main_history_is_not_judged_again(self):
        git(self.repo, 'checkout', '-q', '-b', 'topic')
        self.commit('topic.md', 'topic\n')
        git(self.repo, 'checkout', '-q', 'main')
        self.commit('old-main.md', f'see {USER_PATH}\n')  # main's own history, judged when it landed
        git(self.repo, 'push', '-q', 'origin', 'main')
        git(self.repo, 'checkout', '-q', 'topic')
        git(self.repo, 'merge', '-q', '--no-edit', 'main')
        self.commit('topic2.md', 'more\n')
        code, out = self.range_gate()
        self.assertEqual(code, 0, out)
        self.assertIn('3 commit(s) scanned', out)  # topic, the merge and topic2; main's commit is not one

    def test_mergetag_is_judged(self):
        git(self.repo, 'checkout', '-q', '-b', 'side')
        self.commit('side.md', 'clean\n')
        git(self.repo, 'tag', '-a', 'v-side', '-m', f'notes {USER_PATH}')
        git(self.repo, 'checkout', '-q', '-b', 'topic', self.base)
        self.commit('topic.md', 'topic\n')
        git(self.repo, 'merge', '-q', '--no-ff', '--no-edit', 'v-side')
        raw = git(self.repo, 'cat-file', 'commit', 'HEAD').stdout
        if b'\nmergetag ' not in raw:
            self.skipTest('this git did not record a mergetag header')
        code, out = run_gate(self.repo, '--range', 'HEAD^1..HEAD')
        self.assertEqual(code, 1, out)
        self.assertIn('mergetag', out)

    def baselined_table(self):
        rows = ''.join((f'{n * 3}\n' if n % 4 else '# block\n') for n in range(1, 200))
        self.commit('data/table.json', rows)
        code, out = run_gate(self.repo, '--all', 'HEAD', '--worktree-config', '--write-baseline',
                             'tools/leak_gate_baseline.txt')
        self.assertEqual(code, 0, out)
        self.commit('tools/leak_gate_baseline.txt', Path(self.repo, 'tools/leak_gate_baseline.txt').read_text())
        git(self.repo, 'push', '-q', 'origin', 'main')
        return rows, self.head()

    def test_a_baselined_table_is_bound_to_its_content(self):
        rows, base = self.baselined_table()
        code, out = run_gate(self.repo, '--all', 'HEAD')
        self.assertEqual(code, 0, out)
        changed = ''.join((f'{n * 5}\n' if n % 4 else '# block\n') for n in range(1, 200))  # same counts
        self.commit('data/table.json', changed)
        code, out = self.range_gate(base)
        self.assertEqual(code, 1, out)
        self.assertIn('ERROR dump data/table.json', out)
        code, out = run_gate(self.repo, '--all', 'HEAD')
        self.assertEqual(code, 1, out)
        self.assertIn('stale-baseline', out)

    def test_a_baselined_table_removed_and_added_again_is_judged(self):
        rows, base = self.baselined_table()
        git(self.repo, 'rm', '-q', 'data/table.json')
        git(self.repo, 'commit', '-q', '-m', 'remove')
        self.commit('data/table.json', rows, message='add it back')
        code, out = self.range_gate(base)
        self.assertEqual(code, 1, out)
        self.assertIn('ERROR dump data/table.json', out)

if __name__ == '__main__':
    unittest.main()
