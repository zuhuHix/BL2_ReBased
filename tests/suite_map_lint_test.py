"""tools/suite_map_lint.py against synthetic suite maps and a synthetic tree.

AI-assisted (Claude Opus 5.5), 2026-11-12. Synthetic: a stub git repository in a temporary folder with invented file
names and contents, and invented maps in the hub's format. The binary package() / tag() helpers of the other tests do
not apply here (no package is read). Needs git.
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import suite_map_lint as lint  # noqa: E402

TREE = {
    'README.md': '',
    'docs/notes.md': '',
    'CMakeLists.txt': '',
    'src/core.cpp': '',
    'src/extra.cpp': '',
    'host/ue5/OpenWillow/Source/OpenWillow/Thing.cpp': '',
    'tools/run_suite.ps1': "param()\n. (Join-Path $PSScriptRoot 'helper_a.ps1')\n"
                           "# . (Join-Path $PSScriptRoot 'commented.ps1')\n"
                           "$data = Join-Path $PSScriptRoot '..\\local\\x.json'\n",
    'tools/helper_a.ps1': "& python (Join-Path $PSScriptRoot \"check_x.py\")\n. (Join-Path $PSScriptRoot 'sub\\helper_b.ps1')\n",
    'tools/sub/helper_b.ps1': "$up = Join-Path $PSScriptRoot '..\\shared.py'\n",
    'tools/shared.py': '',
    'tools/check_x.py': '',
    'tools/commented.ps1': '',
}
COVERED = 'tools/run_suite.ps1 tools/helper_a.ps1 tools/sub/ tools/shared.py tools/check_x.py'
HEADER = '# invented map\n*ignore* | - | *.md docs/ | -\n'


class Repo:
    def __init__(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.path = Path(self.tmp.name) / 'repo'
        for rel, text in TREE.items():
            p = self.path / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
        env = dict(os.environ, GIT_AUTHOR_NAME='t', GIT_AUTHOR_EMAIL='t@example.invalid', GIT_COMMITTER_NAME='t',
                   GIT_COMMITTER_EMAIL='t@example.invalid')
        for args in (['init', '-q'], ['add', '.'], ['commit', '-q', '-m', 'fixture']):
            subprocess.run(['git', '-C', str(self.path), *args], check=True, capture_output=True, env=env)

    def write(self, name, text):
        p = Path(self.tmp.name) / name
        p.write_text(text)
        return str(p)


class MatchTest(unittest.TestCase):
    """The matching rules of the hub helper (suites-due.sh)."""

    def test_patterns(self):
        self.assertTrue(lint.match1('src/a.cpp', '.'))
        self.assertTrue(lint.match1('src/a.cpp', '*.cpp'))
        self.assertTrue(lint.match1('src/a.cpp', 'src/'))
        self.assertTrue(lint.match1('src/a.cpp', 'src'))
        self.assertTrue(lint.match1('src/a.cpp', 'src/a.cpp'))
        self.assertFalse(lint.match1('srcx/a.cpp', 'src'))
        self.assertFalse(lint.match1('src/a.cpp', 'src/a'))
        self.assertFalse(lint.match1('src/a.cpp', '*.py'))

    def test_ignore_and_claim(self):
        ignore = ['*.md', 'docs/']
        self.assertFalse(lint.covers('docs/a.txt', ['.'], ignore))            # ignored, "." does not claim it
        self.assertFalse(lint.covers('README.md', ['.'], ignore))
        self.assertTrue(lint.covers('README.md', ['README.md'], ignore))       # the file itself
        self.assertTrue(lint.covers('docs/a.md', ['*.md'], ignore))            # a suffix
        self.assertTrue(lint.covers('docs/sub/a.txt', ['docs/sub/'], ignore))  # a folder inside an ignored one
        self.assertTrue(lint.covers('docs/sub/a.txt', ['docs/'], ignore))      # naming the ignored folder claims it too
        self.assertFalse(lint.covers('docs/sub/a.txt', ['docs/other/', '.'], ignore))
        self.assertTrue(lint.covers('src/a.cpp', ['src/'], ignore))

    def test_map_trimming(self):
        repo = Repo()
        self.addCleanup(repo.tmp.cleanup)
        m = repo.write('m.txt', '  # comment\n\nalpha|1|  src/  tools/x.ps1 |cmd one\r\n beta | 2 | . | cmd two \n#off gamma | 1 | src/ | x\n')
        self.assertEqual(lint.read_map(m), [('alpha', '1', ['src/', 'tools/x.ps1'], 'cmd one'), ('beta', '2', ['.'], 'cmd two')])


class LintTest(unittest.TestCase):
    def setUp(self):
        self.repo = Repo()
        self.addCleanup(self.repo.tmp.cleanup)

    def run_lint(self, body, *extra):
        m = self.repo.write('suites.txt', HEADER + body)
        done = subprocess.run([sys.executable, str(ROOT / 'tools' / 'suite_map_lint.py'), '--map', m, '--repo', str(self.repo.path),
                               *extra], capture_output=True, text=True, timeout=60)
        return done.returncode, [l for l in done.stdout.splitlines() if not l.startswith('suite_map_lint:')], done.stdout + done.stderr

    CLEAN = (f'whole | 4 | . | powershell -File tools/run_suite.ps1; exit $LASTEXITCODE\n'
             f'narrow | 2 | CMakeLists.txt src/ host/ue5/OpenWillow/Source/ {COVERED} | '
             f'powershell -NoProfile -File tools/run_suite.ps1 -Switch; exit $LASTEXITCODE\n')

    def test_clean_map(self):
        rc, errors, out = self.run_lint(self.CLEAN)
        self.assertEqual((rc, errors), (0, []), out)
        self.assertIn('2 suites, 0 error(s)', out)

    def test_pattern_that_matches_nothing(self):
        rc, errors, out = self.run_lint(self.CLEAN.replace('CMakeLists.txt src/', 'CMakeLists.txt src/ tools/gone.py *.nothing'))
        self.assertEqual(rc, 1, out)
        self.assertEqual(errors, ['unmatched: suite narrow: "tools/gone.py" matches no tracked file',
                                  'unmatched: suite narrow: "*.nothing" matches no tracked file'])

    def test_ignored_pattern_still_counts_as_matching(self):
        # As the helper warns: a path matching only ignored files is not "unmatched" (it matched before *ignore*).
        rc, errors, out = self.run_lint(self.CLEAN.replace('CMakeLists.txt src/', 'CMakeLists.txt src/ docs/'))
        self.assertEqual((rc, errors), (0, []), out)

    def test_loaded_file_not_covered(self):
        # helper_b.ps1 is reached only through helper_a.ps1, and loads ../shared.py from its own subfolder.
        body = self.CLEAN.replace(' tools/sub/ tools/shared.py', '')
        rc, errors, out = self.run_lint(body)
        self.assertEqual(rc, 1, out)
        # Both are reported: an uncovered helper is still followed, so its own loads count.
        self.assertEqual(sorted(errors), [
            'not-covered: suite narrow: tools/run_suite.ps1 loads tools/shared.py (in tools/sub/helper_b.ps1), '
            'which its paths do not cover',
            'not-covered: suite narrow: tools/run_suite.ps1 loads tools/sub/helper_b.ps1 (in tools/helper_a.ps1), '
            'which its paths do not cover'])
        rc, errors, out = self.run_lint(self.CLEAN.replace(' tools/shared.py', ''))
        self.assertEqual(errors, ['not-covered: suite narrow: tools/run_suite.ps1 loads tools/shared.py (in tools/sub/helper_b.ps1), '
                                  'which its paths do not cover'], out)

    def test_switch_only_accepts_a_pair(self):
        accept = self.repo.write('switch.txt', '# behind -Other\nnarrow tools/check_x.py\n')
        body = self.CLEAN.replace(' tools/check_x.py', '')
        rc, errors, out = self.run_lint(body)
        self.assertEqual(rc, 1, out)
        self.assertEqual(len(errors), 1)
        rc, errors, out = self.run_lint(body, '--switch-only', accept)
        self.assertEqual((rc, errors), (0, []), out)
        bad = self.repo.write('bad.txt', 'narrow\n')
        rc, _, out = self.run_lint(body, '--switch-only', bad)
        self.assertEqual(rc, 2, out)

    def test_comments_and_local_paths_are_not_loads(self):
        # run_suite.ps1 names commented.ps1 only in a comment and ..\local\x.json (untracked): neither is reported.
        rc, errors, _ = self.run_lint(self.CLEAN)
        self.assertFalse([e for e in errors if 'commented' in e or 'local' in e])

    def test_core_file_only_in_the_whole_tree_suite(self):
        body = self.CLEAN.replace(' src/ ', ' src/core.cpp ').replace('host/ue5/OpenWillow/Source/ ', '')
        rc, errors, out = self.run_lint(body)
        self.assertEqual(rc, 1, out)
        self.assertEqual(errors, [
            'uncovered-core: host/ue5/OpenWillow/Source/OpenWillow/Thing.cpp is covered only by a whole-tree suite '
            '(add it to a suite or to --smoke-only)',
            'uncovered-core: src/extra.cpp is covered only by a whole-tree suite (add it to a suite or to --smoke-only)'])
        smoke = self.repo.write('smoke.txt', '# smoke-only\nsrc/extra.cpp\nhost/ue5/OpenWillow/Source/OpenWillow/Thing.cpp\n')
        rc, errors, out = self.run_lint(body, '--smoke-only', smoke)
        self.assertEqual((rc, errors), (0, []), out)

    def test_unreadable_inputs(self):
        done = subprocess.run([sys.executable, str(ROOT / 'tools' / 'suite_map_lint.py'), '--map', str(self.repo.path / 'none.txt'),
                               '--repo', str(self.repo.path)], capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 2, done.stderr)
        m = self.repo.write('ok.txt', self.CLEAN)
        done = subprocess.run([sys.executable, str(ROOT / 'tools' / 'suite_map_lint.py'), '--map', m,
                               '--repo', str(Path(self.repo.tmp.name) / 'not-a-repo')], capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 2, done.stderr)


if __name__ == '__main__':
    unittest.main()
