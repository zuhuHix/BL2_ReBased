"""tools/suites_due.py and tools/run_suites.ps1 against a synthetic repository and invented suite maps.

AI-assisted (Claude Opus 5.5), 2026-11-12. Synthetic: a throwaway git repository in a temporary folder with invented file
names and contents, invented maps, and a ledger in a temporary HUB_DIR. No real suite runs: the driver's commands are
one-line Write-Output stubs. The binary package() / tag() helpers of the other tests do not apply (no package is read).
Needs git; the driver cases need Windows PowerShell and are skipped without it.
"""
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / 'tools' / 'suites_due.py'
DRIVER = ROOT / 'tools' / 'run_suites.ps1'
POWERSHELL = shutil.which('powershell')
GIT_ENV = dict(GIT_AUTHOR_NAME='t', GIT_AUTHOR_EMAIL='t@example.invalid', GIT_COMMITTER_NAME='t',
               GIT_COMMITTER_EMAIL='t@example.invalid')

TREE = {
    'README.md': 'invented\n',
    'docs/notes.md': 'invented\n',
    'src/core.cpp': 'int a;\n',
    'tools/run_alpha.ps1': "Write-Output 'alpha'\n",
    'tools/beta.py': 'print(1)\n',
}
# The helper refuses a commit whose helper and lint differ from the running copy, so the fixture commits the same two files.
CODE = {rel: (ROOT / rel).read_text(encoding='utf-8').replace('\r\n', '\n')
        for rel in ('tools/suites_due.py', 'tools/suite_map_lint.py')}
MAP = ('# invented map\n'
       '*ignore* | - | *.md docs/ | -\n'
       'alpha | 3 | src/ tools/run_alpha.ps1 | powershell -File tools/run_alpha.ps1; exit $LASTEXITCODE\n'
       '#pass alpha | ALPHA OK | ALPHA FAIL\n'
       'beta  | 2 | tools/beta.py | python tools/beta.py\n'
       '#pass beta | BETA OK |\n')


class Repo:
    def __init__(self, test):
        tmp = tempfile.TemporaryDirectory()
        test.addCleanup(tmp.cleanup)
        self.base = Path(tmp.name)
        self.path = self.base / 'repo'
        self.hub = self.base / 'hub'
        self.map = self.base / 'suites.txt'
        self.map.write_text(MAP, encoding='utf-8', newline='\n')
        self.git('init', '-q')
        self.commit({**TREE, **CODE})

    def git(self, *args):
        self.path.mkdir(exist_ok=True)
        done = subprocess.run(['git', '-C', str(self.path), *args], capture_output=True, text=True,
                              env=dict(os.environ, **GIT_ENV), check=True)
        return done.stdout.strip()

    def commit(self, files):
        for rel, text in files.items():
            p = self.path / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text, encoding='utf-8', newline='\n')
        self.git('add', '-A')
        self.git('commit', '-q', '-m', 'fixture')
        return self.git('rev-parse', 'HEAD')

    def run(self, *args, map_text=None, use_map=True):
        if map_text is not None:
            self.map.write_text(map_text, encoding='utf-8', newline='\n')
        chosen = ['--map', str(self.map)] if use_map else []
        done = subprocess.run([sys.executable, str(HELPER), *chosen, *args], cwd=self.path,
                              capture_output=True, text=True, timeout=60, env=dict(os.environ, HUB_DIR=str(self.hub)))
        return done.returncode, done.stdout, done.stderr

    def due(self, rev='HEAD', map_text=None, use_map=True):
        rc, out, err = self.run('due', rev, map_text=map_text, use_map=use_map)
        names = [l.split()[0] for l in out.splitlines() if l and not l.startswith(('-- ', 'nothing due'))]
        return rc, names, out + err

    def ledger(self):
        p = self.hub / 'test-ledger.txt'
        return p.read_text(encoding='utf-8').splitlines() if p.exists() else []

    def hand_key(self, rev, paths, command):
        """The key as suites-due.sh builds it: the covered ls-tree entries, then "#cmd <command>", as a git blob."""
        entries = self.git('ls-tree', '-r', '--full-tree', rev).splitlines()
        cov = [e for e in entries if any(e.split('\t', 1)[1].startswith(p) or e.split('\t', 1)[1] == p for p in paths)]
        blob = '\n'.join(cov) + '\n#cmd ' + command + '\n'
        return subprocess.run(['git', '-C', str(self.path), 'hash-object', '--stdin'], input=blob.encode(),
                              capture_output=True, check=True).stdout.decode().strip()


ALPHA_CMD = 'powershell -File tools/run_alpha.ps1; exit $LASTEXITCODE'


class DueRecordTest(unittest.TestCase):
    def setUp(self):
        self.repo = Repo(self)

    def test_everything_due_then_nothing_after_record(self):
        rc, names, out = self.repo.due()
        self.assertEqual((rc, names), (0, ['alpha', 'beta']), out)
        self.assertRegex(out, r'-- 2 due, about 5 min, on [0-9a-f]+ \(tree [0-9a-f]+\)')
        rc, out, err = self.repo.run('record', 'HEAD', 'alpha', 'beta')
        self.assertEqual(rc, 0, out + err)
        self.assertEqual(out.splitlines()[0][:26], 'recorded: alpha passed on ')
        rc, names, out = self.repo.due()
        self.assertEqual((rc, names), (0, []), out)
        self.assertIn('nothing due on', out)
        # Recording the same content again adds no line.
        self.repo.run('record', 'HEAD', 'alpha')
        self.assertEqual(len(self.repo.ledger()), 2)

    def test_ledger_line_and_key_match_the_shell_helper(self):
        head = self.repo.git('rev-parse', 'HEAD')
        self.repo.run('record', head, 'alpha')
        [line] = self.repo.ledger()
        name, k, commit, when = line.split('|')
        self.assertEqual((name, commit), ('alpha', head))
        self.assertEqual(k, self.repo.hand_key(head, ['src/', 'tools/run_alpha.ps1'], ALPHA_CMD))
        self.assertRegex(when, r'^\d{4}-\d\d-\d\dT\d\d:\d\dZ$')

    def test_a_pass_recorded_by_the_shell_helper_still_counts(self):
        # A line written by suites-due.sh (key computed here by hand, CRLF line end, older timestamp) keeps alpha off the list.
        head = self.repo.git('rev-parse', 'HEAD')
        self.repo.hub.mkdir()
        key = self.repo.hand_key(head, ['src/', 'tools/run_alpha.ps1'], ALPHA_CMD)
        (self.repo.hub / 'test-ledger.txt').write_bytes(f'alpha|{key}|{head}|2026-11-11T15:56Z\r\n'.encode())
        rc, names, out = self.repo.due()
        self.assertEqual(names, ['beta'], out)

    def test_only_a_covered_change_makes_a_suite_due(self):
        self.repo.run('record', 'HEAD', 'alpha', 'beta')
        self.repo.commit({'docs/notes.md': 'changed\n', 'README.md': 'changed\n'})
        self.assertEqual(self.repo.due()[1], [])                       # ignored paths only
        self.repo.commit({'src/core.cpp': 'int b;\n'})
        self.assertEqual(self.repo.due()[1], ['alpha'])
        self.repo.commit({'src/core.cpp': 'int a;\n'})                 # back to content alpha passed on
        self.assertEqual(self.repo.due()[1], [])

    def test_a_changed_line_makes_that_suite_due_once(self):
        self.repo.run('record', 'HEAD', 'alpha', 'beta')
        changed = MAP.replace('python tools/beta.py', 'python tools/beta.py --quiet')
        self.assertEqual(self.repo.due(map_text=changed)[1], ['beta'])
        self.repo.run('record', 'HEAD', 'beta')
        self.assertEqual(self.repo.due()[1], [])
        # Comments, #pass lines and the minutes are not part of the key.
        quiet = changed.replace('# invented map', '# edited comment').replace('| BETA OK |', '| BETA PASSED |')
        self.assertEqual(self.repo.due(map_text=quiet.replace('beta  | 2 |', 'beta | 9 |'))[1], [])
        # Paths that cover the same files give the same key; paths that cover more do not.
        self.assertEqual(self.repo.due(map_text=quiet.replace('src/ tools/run_alpha.ps1', 'src tools/run_alpha.ps1'))[1], [])
        self.assertEqual(self.repo.due(map_text=quiet.replace('src/ tools/run_alpha.ps1', 'src/ tools/'))[1], ['alpha'])

    def test_unknown_suite_name(self):
        rc, out, err = self.repo.run('record', 'HEAD', 'gamma')
        self.assertEqual(rc, 2, out + err)
        self.assertIn("no suite named 'gamma'", err)
        self.assertEqual(self.repo.ledger(), [])
        rc, out, err = self.repo.run('why', 'HEAD', 'gamma')
        self.assertEqual(rc, 2)

    def test_path_that_matches_nothing(self):
        # One path of a suite matching nothing: a warning, the suite still works on the rest.
        rc, names, out = self.repo.due(map_text=MAP.replace('tools/beta.py |', 'tools/beta.py tools/gone.py |'))
        self.assertEqual((rc, names), (0, ['alpha', 'beta']), out)
        self.assertIn('warning: "tools/gone.py" of suite beta matches no file at this commit', out)
        # A suite covering no file at all: listed with "!!", exit 1, and it cannot be recorded.
        rc, names, out = self.repo.due(map_text=MAP.replace('tools/beta.py |', 'tools/gone.py |'))
        self.assertEqual(rc, 1, out)
        self.assertRegex(out, r'beta +2 min   !! covers no file at this commit')
        rc, out, err = self.repo.run('record', 'HEAD', 'beta')
        self.assertEqual(rc, 2)
        self.assertIn("suite 'beta' covers no file", err)

    def test_paths_split_on_spaces_and_tabs_only(self):
        name = 'tools/a' + chr(0xa0) + 'b.py'
        self.repo.commit({name: 'x\n'})
        rc, names, out = self.repo.due(map_text=MAP.replace('tools/beta.py |', name + '\ttools/beta.py |'))
        self.assertEqual((rc, names), (0, ['alpha', 'beta']), out)
        self.assertNotIn('matches no file', out)

    def test_a_covered_symlink_is_refused(self):
        blob = subprocess.run(['git', '-C', str(self.repo.path), 'hash-object', '-w', '--stdin'], input=b'core.cpp',
                              capture_output=True, check=True).stdout.decode().strip()
        self.repo.git('update-index', '--add', '--cacheinfo', f'120000,{blob},src/link')
        self.repo.git('commit', '-q', '-m', 'link')
        for args in (('due', 'HEAD'), ('record', 'HEAD', 'alpha')):
            rc, out, err = self.repo.run(*args)
            self.assertEqual(rc, 2, out + err)
            self.assertIn('suite alpha: src/link is a symbolic link', err)
        self.assertEqual(self.repo.ledger(), [])
        self.assertEqual(self.repo.run('record', 'HEAD', 'beta')[0], 0)   # beta covers no link

    def test_refused_map_and_commit(self):
        rc, out, err = self.repo.run('due', 'HEAD', map_text=MAP + 'bad name | 1 | src/ | x\n')
        self.assertEqual(rc, 2)
        self.assertIn('may hold only letters', err)
        rc, out, err = self.repo.run('due', 'HEAD', map_text=MAP + 'alpha | 1 | src/ | x\n')
        self.assertEqual(rc, 2)
        self.assertIn('"alpha" is listed twice', err)
        rc, out, err = self.repo.run('due', 'HEAD', map_text=MAP.replace('python tools/beta.py', 'python a.py | more'))
        self.assertEqual(rc, 2)
        self.assertIn('suite "beta" has more than 4 fields', err)
        rc, out, err = self.repo.run('due', 'HEAD', map_text=MAP + 'Alpha | 1 | src/ | x\n')
        self.assertEqual(rc, 2)
        self.assertIn('"Alpha" differs from another suite only in case', err)
        rc, out, err = self.repo.run('due', 'no-such-rev', map_text=MAP)
        self.assertEqual((rc, err.strip()), (2, 'suites-due: unknown commit: no-such-rev'))

    def test_why_lists_the_covered_files_that_changed(self):
        self.repo.run('record', 'HEAD', 'alpha')
        self.repo.commit({'src/core.cpp': 'int c;\n', 'src/new.cpp': '\n', 'docs/notes.md': 'x\n', 'tools/beta.py': 'x\n'})
        rc, out, err = self.repo.run('why', 'HEAD', 'alpha')
        self.assertEqual(rc, 0, err)
        self.assertEqual([l.strip() for l in out.splitlines()[1:]], ['src/core.cpp', 'src/new.cpp'])
        self.assertEqual(self.repo.run('why', 'HEAD', 'beta')[1].strip(), 'beta has no recorded pass yet')

    def test_ledger_defaults_to_local_hub_of_the_main_checkout(self):
        env = {k: v for k, v in os.environ.items() if k != 'HUB_DIR'}
        wt = self.repo.base / 'wt'
        self.repo.git('worktree', 'add', '-q', '--detach', str(wt))
        done = subprocess.run([sys.executable, str(HELPER), '--map', str(self.repo.map), 'record', 'HEAD', 'beta'], cwd=wt,
                              capture_output=True, text=True, env=env, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertTrue((self.repo.path / 'local' / 'hub' / 'test-ledger.txt').is_file())


class MapOfTheCommitTest(unittest.TestCase):
    """Without --map the map is tools/suites.txt as committed in the commit being judged, not the working tree's."""

    def setUp(self):
        self.repo = Repo(self)

    def test_no_map_in_the_commit_is_refused(self):
        rc, out, err = self.repo.run('due', 'HEAD', use_map=False)
        self.assertEqual(rc, 2, out + err)
        self.assertIn('has no tools/suites.txt; pass --map <file>', err)

    def test_other_helper_code_in_the_commit_is_refused(self):
        for rel in CODE:
            with self.subTest(rel=rel):
                self.repo.commit({**CODE, rel: CODE[rel] + '# changed\n'})
                for args in (('due', 'HEAD'), ('record', 'HEAD', 'beta'), ('why', 'HEAD', 'beta')):
                    rc, out, err = self.repo.run(*args)
                    self.assertEqual(rc, 2, out + err)
                    self.assertIn(f'has a different {rel} than this copy; run the helper from a checkout of that commit', err)
        self.assertEqual(self.repo.ledger(), [])
        self.repo.commit(CODE)
        self.assertEqual(self.repo.run('due', 'HEAD')[0], 0)

    def test_a_commit_that_widens_a_suite_is_judged_by_its_own_map(self):
        a = self.repo.commit({'tools/suites.txt': MAP})
        self.assertEqual(self.repo.run('record', a, 'alpha', 'beta', use_map=False)[0], 0)
        self.assertEqual(self.repo.due(a, use_map=False)[1], [])
        # B widens beta to lib/ and adds a file there; the working tree goes back to A, whose map does not know lib/.
        b = self.repo.commit({'tools/suites.txt': MAP.replace('tools/beta.py |', 'tools/beta.py lib/ |'), 'lib/x.py': '\n'})
        self.repo.git('checkout', '-q', '--detach', a)
        rc, names, out = self.repo.due(b, use_map=False)
        self.assertEqual((rc, names), (0, ['beta']), out)
        # The working tree's map (A's) would have missed it: the bug this guards against.
        self.repo.map.write_text((self.repo.path / 'tools' / 'suites.txt').read_text(encoding='utf-8'), encoding='utf-8')
        self.assertEqual(self.repo.due(b)[1], [])
        # record and why read B's map too.
        self.assertEqual(self.repo.run('record', b, 'beta', use_map=False)[0], 0)
        self.assertEqual(self.repo.due(b, use_map=False)[1], [])
        rc, out, err = self.repo.run('why', b, 'beta', use_map=False)
        self.assertEqual(rc, 0, err)


class TrackedMapTest(unittest.TestCase):
    def test_tracked_map_is_machine_neutral(self):
        text = (ROOT / 'tools' / 'suites.txt').read_text(encoding='utf-8')
        self.assertIsNone(re.search(r'\b[A-Za-z]:[\\/]|/Users/|/home/|\\Users\\', text))

    def test_tracked_map_reads(self):
        done = subprocess.run([sys.executable, str(HELPER), 'list'], cwd=ROOT, capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn('slice_gate ', done.stdout)


@unittest.skipUnless(POWERSHELL, 'needs Windows PowerShell')
class DriverTest(unittest.TestCase):
    def setUp(self):
        self.repo = Repo(self)
        self.head = self.repo.git('rev-parse', 'HEAD')
        self.repo.map.write_text(
            '*ignore* | - | *.md docs/ | -\n'
            "alpha | 3 | src/ | Write-Output 'ALPHA OK'; exit 0\n"
            '#pass alpha | ALPHA OK | ALPHA FAIL\n'
            "beta | 2 | tools/beta.py | Write-Output 'BETA NO'; exit 3\n"
            '#pass beta | BETA OK |\n', encoding='utf-8', newline='\n')

    def drive(self, *extra):
        out = self.repo.base / 'out'
        done = subprocess.run([POWERSHELL, '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(DRIVER), '-Worktree',
                               str(self.repo.path), '-Commit', self.head, '-OutDir', str(out), '-Map', str(self.repo.map),
                               *extra], capture_output=True, text=True, timeout=300,
                              env=dict(os.environ, HUB_DIR=str(self.repo.hub)))
        return done.returncode, done.stdout + done.stderr, out

    def test_runs_the_due_suites_records_passes_and_counts_fails(self):
        rc, text, out = self.drive()
        self.assertEqual(rc, 1, text)   # one suite ended FAIL
        rows = [l.split('\t')[:4] for l in (out / 'results.tsv').read_text(encoding='utf-8-sig').splitlines()[1:]]
        self.assertEqual(rows, [['alpha', 'run1', 'PASS', '0'], ['beta', 'run1', 'FAIL-RERUN', '3'],
                                ['beta', 'run1.rerun', 'FAIL', '3']])
        self.assertEqual([l.split('|')[0] for l in self.repo.ledger()], ['alpha'])
        self.assertIn('ALPHA OK', (out / 'alpha.log').read_text(encoding='utf-8-sig'))
        self.assertEqual(self.repo.due(map_text=None)[1], ['beta'])

    def test_a_suite_that_changes_the_checkout_is_not_recorded(self):
        self.repo.map.write_text('*ignore* | - | *.md docs/ | -\n'
                                 "alpha | 3 | src/ | Add-Content src/core.cpp 'x'; Write-Output 'ALPHA OK'; exit 0\n"
                                 '#pass alpha | ALPHA OK |\n', encoding='utf-8', newline='\n')
        rc, text, out = self.drive()
        self.assertEqual(rc, 1, text)
        self.assertIn('alpha: not recorded: the checkout changed during the run', text)
        self.assertEqual(self.repo.ledger(), [])

    def test_a_change_before_the_rerun_fails_it_without_running(self):
        self.repo.map.write_text('*ignore* | - | *.md docs/ | -\n'
                                 "alpha | 3 | src/ | Add-Content src/core.cpp 'x'; exit 1\n"
                                 '#pass alpha | ALPHA OK |\n', encoding='utf-8', newline='\n')
        rc, text, out = self.drive()
        self.assertEqual(rc, 1, text)
        self.assertIn('FAILED: the checkout changed during the run', text)
        rows = [l.split('\t')[:4] for l in (out / 'results.tsv').read_text(encoding='utf-8-sig').splitlines()[1:]]
        self.assertEqual(rows[:2], [['alpha', 'run1', 'FAIL-RERUN', '1'], ['alpha', 'run1.rerun', 'FAIL', '-']])
        # The rerun never ran: the file holds the one line the first attempt added.
        self.assertEqual((self.repo.path / 'src' / 'core.cpp').read_text().splitlines(), ['int a;', 'x'])

    def test_a_refused_record_counts_as_a_failed_suite(self):
        self.repo.map.write_text('*ignore* | - | *.md docs/ | -\n'
                                 "alpha | 3 | nothing/ | Write-Output 'ALPHA OK'; exit 0\n"
                                 '#pass alpha | ALPHA OK |\n', encoding='utf-8', newline='\n')
        rc, text, out = self.drive('-Suites', 'alpha')
        self.assertEqual(rc, 1, text)
        self.assertIn('recording alpha failed (exit 2); it counts as a failed suite', text)
        self.assertEqual(self.repo.ledger(), [])

    def test_default_map_is_the_one_in_the_worktree(self):
        committed = self.repo.map.read_text(encoding='utf-8').replace('exit 3', 'exit 0').replace('BETA NO', 'BETA OK')
        self.head = self.repo.commit({'tools/suites.txt': committed})
        out = self.repo.base / 'out'
        done = subprocess.run([POWERSHELL, '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(DRIVER), '-Worktree',
                               str(self.repo.path), '-Commit', self.head, '-OutDir', str(out)], capture_output=True,
                              text=True, timeout=300, env=dict(os.environ, HUB_DIR=str(self.repo.hub)))
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual(sorted(l.split('|')[0] for l in self.repo.ledger()), ['alpha', 'beta'])

    def test_refuses_an_unknown_suite_and_a_wrong_commit(self):
        rc, text, _ = self.drive('-Suites', 'gamma')
        self.assertEqual(rc, 100, text)
        self.assertIn("no suite named 'gamma'", text)
        self.head = '0' * 40
        rc, text, _ = self.drive()
        self.assertEqual(rc, 100, text)
        self.assertEqual(self.repo.ledger(), [])


if __name__ == '__main__':
    unittest.main()
