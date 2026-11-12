"""tools/preflight.ps1: each check has a passing and a failing case.

AI-assisted (Claude Opus 5.5), 2026-11-12. Synthetic: every case runs the real script against a stub git repository
in a temporary folder with invented files and dated commits (GIT_COMMITTER_DATE). The run lock is a temporary path
(OPENWILLOW_UE_LOCK, with LOCALAPPDATA also pointed at the temporary folder), the port check uses a socket this test
opens, and the editor check is fed an invented process list through -ProcessList, so no real lock, editor or other
process is read, matched or touched. Windows only (PowerShell, and mklink /J for the link check). Every case runs under
Windows PowerShell 5.1 and under pwsh, each when present: the suites run under 5.1, where a native program's stderr
behaves differently.
"""
import datetime
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'tools' / 'preflight.ps1'
SHELLS = {name: path for name, path in (('powershell', shutil.which('powershell')), ('pwsh', shutil.which('pwsh'))) if path}
OLD = datetime.datetime(2019, 6, 1, tzinfo=datetime.timezone.utc).timestamp()     # before every commit below
COMMITTED = '2020-01-01T12:00:00+00:00'


def git(repo, *args, date=None):
    env = dict(os.environ, GIT_AUTHOR_NAME='t', GIT_AUTHOR_EMAIL='t@example.invalid', GIT_COMMITTER_NAME='t',
               GIT_COMMITTER_EMAIL='t@example.invalid')
    if date:
        env['GIT_COMMITTER_DATE'] = env['GIT_AUTHOR_DATE'] = date
    subprocess.run(['git', '-C', str(repo), *args], check=True, capture_output=True, env=env)


class Cases:
    POWERSHELL = None   # set per shell by the classes at the end
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name)
        self.wt = base / 'work tree'
        self.outside = base / 'elsewhere'
        self.outside.mkdir()
        self.lock = base / 'locks' / 'ue_run.lock'
        for rel in ('tools/make_data.py', 'src/core.cpp', 'host/ue5/OpenWillow/Source/OpenWillow/Thing.cpp', 'CMakeLists.txt'):
            path = self.wt / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('invented\n')
        git(self.wt, 'init', '-q')
        git(self.wt, 'add', '.')
        git(self.wt, 'commit', '-q', '-m', 'fixture', date=COMMITTED)

    def tearDown(self):
        self.tmp.cleanup()

    def put(self, rel, old=False):
        path = self.wt / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('x')
        if old:
            os.utime(path, (OLD, OLD))
        return path

    def run_ps(self, *args, env=None):
        e = dict(os.environ)
        e.pop('OPENWILLOW_UE', None)
        e['OPENWILLOW_UE_LOCK'] = str(self.lock)
        e['LOCALAPPDATA'] = self.tmp.name
        e.update(env or {})
        done = subprocess.run([self.POWERSHELL, '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', str(SCRIPT),
                               '-Worktree', str(self.wt), *args], capture_output=True, text=True, env=e, timeout=120)
        lines = [l for l in done.stdout.splitlines() if l.startswith('PREFLIGHT ') and 'SUMMARY' not in l]
        return done.returncode, lines, done.stdout + done.stderr

    def assertLines(self, lines, check, kinds):
        got = [l.split(' ', 2)[1] for l in lines if l.split(' ', 2)[2].startswith(check + ':')]
        self.assertEqual(got, kinds, '\n'.join(lines))

    def test_no_check_is_a_usage_error(self):
        rc, _, out = self.run_ps()
        self.assertEqual(rc, 2, out)

    def test_data(self):
        self.put('local/a/data.json')
        (self.wt / 'local' / 'empty' / 'sub').mkdir(parents=True)
        rc, lines, out = self.run_ps('-Data', 'local/a/data.json,local/a')
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'data', ['ok', 'ok'])
        rc, lines, out = self.run_ps('-Data', 'local/a/data.json,local/missing.json,local/empty')
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'data', ['ok', 'FAIL', 'FAIL'])
        self.assertIn('local/missing.json is missing', out)
        self.assertIn('local/empty is a folder that holds no file', out)

    def test_not_a_git_checkout_still_reports(self):
        # The reviewer's reproduction: under 5.1, git's stderr used to end the run with no PREFLIGHT line and no summary.
        bare = Path(self.tmp.name) / 'no repo'
        bare.mkdir()
        e = dict(os.environ, OPENWILLOW_UE_LOCK=str(self.lock), LOCALAPPDATA=self.tmp.name, GIT_CEILING_DIRECTORIES=self.tmp.name)
        done = subprocess.run([self.POWERSHELL, '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', str(SCRIPT),
                               '-Worktree', str(bare), '-Generators', 'a=b.py', '-CheckBuild'],
                              capture_output=True, text=True, env=e, timeout=120)
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn('PREFLIGHT FAIL fresh: b.py has no commit', done.stdout)
        self.assertIn('PREFLIGHT FAIL build:', done.stdout)
        self.assertRegex(done.stdout, r'PREFLIGHT SUMMARY failed=\d+ checks=\d+')

    def test_data_newer_than_its_generator(self):
        self.put('local/fresh/one.json')                     # written now, after the 2020 commit
        self.put('local/stale/one.json', old=True)
        os.utime(self.wt / 'local' / 'stale', (OLD, OLD))
        rc, lines, out = self.run_ps('-Generators', 'local/fresh=tools/make_data.py')
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'fresh', ['ok'])
        rc, lines, out = self.run_ps('-Generators', 'local/stale=tools/make_data.py')
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'fresh', ['FAIL'])
        self.assertIn('older than the last commit of tools/make_data.py', out)
        rc, _, out = self.run_ps('-Generators', 'local/fresh')
        self.assertEqual(rc, 2, out)
        (self.wt / 'tools' / 'make_data.py').write_text('edited, not committed\n')
        rc, lines, out = self.run_ps('-Generators', 'local/fresh=tools/make_data.py')
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'fresh', ['FAIL'])
        self.assertIn('git status is not clean for tools/make_data.py', out)

    def test_links_stay_inside_the_worktree(self):
        (self.wt / 'local' / 'deep').mkdir(parents=True)
        (self.wt / 'data').mkdir()

        def junction(link, target):
            subprocess.run(['cmd', '/c', 'mklink', '/J', str(link), str(target)], check=True, capture_output=True)
        junction(self.wt / 'local' / 'deep' / 'inside', self.wt / 'data')
        rc, lines, out = self.run_ps('-CheckLinks')
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'links', ['ok'])
        self.assertIn('1 link(s)', out)
        junction(self.wt / 'local' / 'deep' / 'away', self.outside)
        rc, lines, out = self.run_ps('-CheckLinks')
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'links', ['FAIL'])
        self.assertIn('away', out)

    def test_port(self):
        with socket.socket() as held:
            held.bind(('127.0.0.1', 0))
            held.listen(1)
            port = held.getsockname()[1]
            rc, lines, out = self.run_ps('-Port', str(port))
            self.assertEqual(rc, 1, out)
            self.assertLines(lines, 'port', ['FAIL'])
        with socket.socket() as probe:                         # a port nobody listens on: bound and closed again
            probe.bind(('127.0.0.1', 0))
            free = probe.getsockname()[1]
        rc, lines, out = self.run_ps('-Port', str(free))
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'port', ['ok'])

    def test_editor(self):
        procs = Path(self.tmp.name) / 'procs.txt'
        procs.write_text('notepad 11\nUnrealEditorHelper 12\n')
        rc, lines, out = self.run_ps('-NoEditor', '-ProcessList', str(procs))
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'editor', ['ok'])
        procs.write_text('notepad 11\nUnrealEditor-Cmd 13\n')
        rc, lines, out = self.run_ps('-NoEditor', '-ProcessList', str(procs))
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'editor', ['FAIL'])

    def test_run_lock_and_extra_lock(self):
        rc, lines, out = self.run_ps('-CheckLock', '-AlsoLock', 'local/hub-ue.lock')
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'lock', ['ok', 'ok'])
        self.assertIn(str(self.lock), out)
        self.lock.parent.mkdir(parents=True, exist_ok=True)
        self.lock.write_text('{"owner":"someone","pid":1,"process_start_utc_filetime":1}')
        rc, lines, out = self.run_ps('-CheckLock')
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'lock', ['FAIL'])
        self.assertIn('owner someone', out)
        self.assertTrue(self.lock.exists())                     # looked at, never removed
        script = SCRIPT.read_text(encoding='utf-8')
        self.assertNotIn('Get-OwUeLockOwner $lock', script)     # File.ReadAllText would block the owner's delete
        self.assertIn('[IO.FileShare]::Delete', script)
        self.lock.unlink()
        self.put('local/hub-ue.lock')
        rc, lines, out = self.run_ps('-CheckLock', '-AlsoLock', 'local/hub-ue.lock')
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'lock', ['ok', 'FAIL'])

    def test_hold_is_reported_never_failed(self):
        rc, lines, out = self.run_ps('-ReportHold', 'local/batch.hold')
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'hold', ['ok'])
        self.put('local/batch.hold')
        rc, lines, out = self.run_ps('-ReportHold', 'local/batch.hold')
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'hold', ['NOTE'])

    def test_build_newer_than_the_code(self):
        self.put('bin/module.dll')
        self.put('bin/core.lib')
        args = ['-CheckBuild', '-ModuleLibrary', 'bin/module.dll', '-CoreLibrary', 'bin/core.lib']
        rc, lines, out = self.run_ps(*args)
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'build', ['ok', 'ok'])
        self.put('bin/core.lib', old=True)
        rc, lines, out = self.run_ps(*args)
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'build', ['ok', 'FAIL'])
        (self.wt / 'bin' / 'module.dll').unlink()
        rc, lines, out = self.run_ps(*args)
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'build', ['FAIL', 'FAIL'])
        self.put('bin/module.dll')
        self.put('bin/core.lib')
        self.put('src/new_file.cpp')                            # untracked source: the build may not hold it
        rc, lines, out = self.run_ps(*args)
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'build', ['FAIL', 'ok', 'ok'])
        self.assertIn('src/new_file.cpp', out)

    def test_engine(self):
        engine = Path(self.tmp.name) / 'Invented UE'
        rc, lines, out = self.run_ps('-CheckEngine', env={'OPENWILLOW_UE': str(engine)})
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'engine', ['FAIL'])
        self.assertIn(str(engine), out)
        binary = engine / 'Engine' / 'Binaries' / 'Win64' / ('UnrealEditor' + '.exe')
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b'')
        rc, lines, out = self.run_ps('-CheckEngine', env={'OPENWILLOW_UE': str(engine)})
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'engine', ['ok'])

    def test_report_file(self):
        report = Path(self.tmp.name) / 'report.txt'
        rc, _, out = self.run_ps('-Data', 'local/missing.json', '-Report', str(report))
        self.assertEqual(rc, 1, out)
        text = report.read_text()
        self.assertIn('PREFLIGHT FAIL data', text)
        self.assertIn('PREFLIGHT SUMMARY failed=1', text)


for _name, _path in SHELLS.items():
    globals()[f'Preflight_{_name}'] = unittest.skipUnless(sys.platform == 'win32', 'Windows only')(
        type(f'Preflight_{_name}', (Cases, unittest.TestCase), {'POWERSHELL': _path}))


class ShellsTest(unittest.TestCase):
    @unittest.skipUnless(sys.platform == 'win32', 'Windows only')
    def test_windows_powershell_is_among_them(self):
        self.assertIn('powershell', SHELLS)


if __name__ == '__main__':
    unittest.main()
