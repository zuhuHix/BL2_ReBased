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
# For the one case that runs the stamp script of CMakeLists.txt in script mode: cmake on PATH, else the Build Tools copy GATES.md names.
CMAKE = shutil.which('cmake') or next((str(p) for p in [Path(os.environ.get('ProgramFiles(x86)', r'C:\Program Files (x86)'))
        / 'Microsoft Visual Studio/18/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe'] if p.is_file()), None)


def long_path(path):
    r"""The path with every 8.3 short name expanded (GetLongPathNameW), so it can be compared as text with what the script
    prints: a temporary folder can come back with a short 8.3 name such as RUNNER~1 for a folder the script prints by its
    long name.
    The path must exist. Not Path.resolve(): that also follows junctions, which the link check needs left alone."""
    if sys.platform != 'win32':
        return str(path)
    import ctypes
    size = ctypes.windll.kernel32.GetLongPathNameW(str(path), None, 0)
    buf = ctypes.create_unicode_buffer(size)
    if not size or not ctypes.windll.kernel32.GetLongPathNameW(str(path), buf, size):
        raise OSError(f'GetLongPathNameW failed for {path}')
    return buf.value


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
        # Every path below is built from the long form, so paths the script prints compare as text with the test's.
        self.base = base = Path(long_path(self.tmp.name))
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
        e['LOCALAPPDATA'] = str(self.base)
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
        bare = self.base / 'no repo'
        bare.mkdir()
        e = dict(os.environ, OPENWILLOW_UE_LOCK=str(self.lock), LOCALAPPDATA=str(self.base), GIT_CEILING_DIRECTORIES=str(self.base))
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
        procs = self.base / 'procs.txt'
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
        # A text check only: no handle is held against a real delete here, it asserts that the script opens the lock with
        # delete sharing instead of through Get-OwUeLockOwner.
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

    # -CheckBuild. The module DLL is judged by file times against what it is built from; the core library by the input stamp
    # the CMake build writes next to it. stamp() writes that stamp as target ow-build-stamp does (test_stamp_writer_matches
    # checks that the two agree, line endings aside).
    STAMP_PATHS = ['src', 'third_party/lzokay', 'CMakeLists.txt', 'tests/CMakeLists.txt']
    BUILD = ['-CheckBuild', '-ModuleLibrary', 'bin/module.dll', '-CoreLibrary', 'bin/core.lib']

    def git_out(self, *args):
        return subprocess.run(['git', '--no-optional-locks', '-C', str(self.wt), *args], check=True, capture_output=True,
                              text=True).stdout

    def stamp(self, lib='bin/core.lib'):
        status = self.git_out('status', '--porcelain', '--', *self.STAMP_PATHS)
        text = (f"ow-build-stamp 1\npaths {' '.join(self.STAMP_PATHS)}\ncommit {self.git_out('rev-parse', 'HEAD').strip()}\n"
                f"dirty {len([l for l in status.splitlines() if l])}\n"
                + self.git_out('ls-files', '-s', '--', *self.STAMP_PATHS))
        path = self.wt / (lib + '.inputs')
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(text.encode())
        return path

    def at(self, rel, when):
        os.utime(self.wt / rel, (when, when))

    def commit(self, rel, text, message='change'):
        (self.wt / rel).parent.mkdir(parents=True, exist_ok=True)
        (self.wt / rel).write_text(text)
        git(self.wt, 'add', rel)
        git(self.wt, 'commit', '-q', '-m', message)

    def core_line(self, lines):
        return [l for l in lines if 'bin/core.lib' in l][0]

    def test_core_library_against_its_stamp(self):
        self.put('bin/module.dll')
        self.put('bin/core.lib', old=True)
        rc, lines, out = self.run_ps(*self.BUILD)              # no stamp: a checkout built before the stamp existed
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'build', ['ok', 'UNKNOWN'])
        self.assertIn('no input stamp', self.core_line(lines))
        self.assertIn('cmake --build build --config Release', self.core_line(lines))
        self.assertIn('unknown=1', out)
        self.stamp()
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'build', ['ok', 'ok'])
        # The #228 case: CMakeLists.txt changes in a way the library does not depend on; the build runs, rebuilds nothing
        # and rewrites the stamp. The library stays older than CMakeLists.txt and its commit, and is still fresh.
        self.commit('CMakeLists.txt', 'invented, test registration moved\n')
        self.stamp()
        self.assertLess((self.wt / 'bin/core.lib').stat().st_mtime, (self.wt / 'CMakeLists.txt').stat().st_mtime)
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'build', ['ok', 'ok'])
        # A real input changes and is committed, with no build after it: stale, and the file is named.
        self.commit('src/core.cpp', 'invented, edited\n')
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'build', ['ok', 'FAIL'])
        self.assertIn('src/core.cpp', self.core_line(lines))
        # Built after that; then the input changes again, not committed, with a file time older than the library: stale.
        self.stamp()
        (self.wt / 'src' / 'core.cpp').write_text('invented, edited again\n')
        self.at('src/core.cpp', OLD)
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'build', ['ok', 'FAIL'])
        self.assertIn('src/core.cpp', self.core_line(lines))
        # Built with that uncommitted change: the stamp cannot say what was built, so unknown, never ok.
        self.stamp()
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'build', ['ok', 'UNKNOWN'])
        self.assertIn('uncommitted', self.core_line(lines))

    def test_core_stamp_from_another_commit(self):
        self.put('bin/module.dll')
        self.put('bin/core.lib', old=True)
        stamp = self.stamp()
        built = self.git_out('rev-parse', 'HEAD').strip()
        start = self.git_out('rev-parse', '--abbrev-ref', 'HEAD').strip()
        self.commit('docs/notes.md', 'invented\n', 'docs only')   # not an input: the commit differs, the inputs do not
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'build', ['ok', 'ok'])
        self.assertIn(built[:7], self.core_line(lines))
        mine = stamp.read_bytes()
        git(self.wt, 'checkout', '-q', '-b', 'other', built)     # another branch, where an input differs
        self.commit('third_party/lzokay/lzokay.cpp', 'invented\n')
        other = self.stamp().read_bytes()                       # the library was last built there
        git(self.wt, 'checkout', '-q', start)
        stamp.write_bytes(other)
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'build', ['ok', 'FAIL'])
        self.assertIn('third_party/lzokay/lzokay.cpp', self.core_line(lines))
        stamp.write_bytes(mine)
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'build', ['ok', 'ok'])

    def test_core_stamp_that_cannot_be_trusted(self):
        self.put('bin/module.dll')
        self.put('bin/core.lib', old=True)
        stamp = self.stamp()
        good = stamp.read_bytes()
        for damaged in (b'', b'ow-build-stamp 2\n' + good.split(b'\n', 1)[1], good.replace(b'paths src ', b'paths ')):
            stamp.write_bytes(damaged)
            rc, lines, out = self.run_ps(*self.BUILD)
            self.assertEqual(rc, 0, out)
            self.assertLines(lines, 'build', ['ok', 'UNKNOWN'])
            self.assertIn('damaged or from another version', self.core_line(lines))
        stamp.write_bytes(good)
        os.utime(stamp, (OLD - 10, OLD - 10))                   # the library was built after the stamp, without the stamp step
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'build', ['ok', 'UNKNOWN'])
        self.assertIn('newer than its input stamp', self.core_line(lines))
        (self.wt / 'bin' / 'core.lib').unlink()
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'build', ['ok', 'FAIL'])
        self.assertIn('bin/core.lib is missing', out)

    def test_module_against_the_files_it_is_built_from(self):
        now = datetime.datetime.now().timestamp()
        for rel in ('host/ue5/OpenWillow/Source/OpenWillow/OpenWillow.Build.cs', 'host/ue5/OpenWillow/OpenWillow.uproject',
                    'src/core.hpp', 'bin/core.lib', 'bin/ow-lzokay.lib'):
            self.put(rel)
        git(self.wt, 'add', '.')
        git(self.wt, 'commit', '-q', '-m', 'module inputs')
        for p in [p for p in self.wt.rglob('*') if p.is_file() and '.git' not in p.parts]:
            os.utime(p, (now - 100, now - 100))
        self.put('bin/module.dll')
        self.stamp()
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'build', ['ok', 'ok'])
        # The #228 case for the module: CMakeLists.txt and a .cpp under src are not what the DLL is built from.
        self.at('CMakeLists.txt', now + 50)
        self.at('src/core.cpp', now + 50)
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'build', ['ok', 'ok'])
        for rel in ('host/ue5/OpenWillow/Source/OpenWillow/Thing.cpp', 'src/core.hpp', 'bin/ow-lzokay.lib'):
            self.at(rel, now + 50)
            rc, lines, out = self.run_ps(*self.BUILD)
            self.assertEqual(rc, 1, out)
            self.assertLines(lines, 'build', ['FAIL', 'ok'])
            self.assertIn(rel.replace('/', '\\'), lines[0])
            self.at(rel, now - 100)
        self.put('host/ue5/OpenWillow/Source/OpenWillow/New.cpp')   # untracked: the build tool compiles it all the same
        self.at('host/ue5/OpenWillow/Source/OpenWillow/New.cpp', now + 50)
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'build', ['FAIL', 'ok'])
        self.at('host/ue5/OpenWillow/Source/OpenWillow/New.cpp', now - 100)
        for rel in ('host/ue5/OpenWillow/Source/OpenWillow/OpenWillow.Build.cs', 'host/ue5/OpenWillow/OpenWillow.uproject'):
            self.at(rel, now + 50)                              # a build description: may or may not change the DLL
            rc, lines, out = self.run_ps(*self.BUILD)
            self.assertEqual(rc, 0, out)
            self.assertLines(lines, 'build', ['UNKNOWN', 'ok'])
            self.at(rel, now - 100)
        (self.wt / 'bin' / 'module.dll').unlink()
        rc, lines, out = self.run_ps(*self.BUILD)
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'build', ['FAIL', 'ok'])

    @unittest.skipUnless(CMAKE, 'no cmake found (PATH, or the Visual Studio Build Tools copy)')
    def test_stamp_writer_matches(self):
        # The stamp script in CMakeLists.txt, which the build runs, and stamp() above write the same bytes. It is taken from
        # between the brackets, as configure does, and run in script mode only (-P): it runs git and writes the one file; no
        # configure, no build.
        cmakelists = (ROOT / 'CMakeLists.txt').read_text(encoding='utf-8')
        script = self.base / 'ow_build_stamp.cmake'
        script.write_text(cmakelists.split('ow_build_stamp.cmake" [==[\n', 1)[1].split(']==]', 1)[0])
        (self.wt / 'src' / 'core.cpp').write_text('dirty\n')    # so the dirty count is not 0
        out = self.base / 'from cmake.inputs'
        writer = [CMAKE, f'-DGIT={shutil.which("git")}', f'-DOUT={out}', '-P', str(script)]
        subprocess.run(writer[:2] + [f'-DREPO={self.wt}'] + writer[2:], check=True, capture_output=True, timeout=120)
        # CMake's file(WRITE) ends lines with CRLF on Windows; the preflight reads either.
        written = out.read_bytes().replace(b'\r\n', b'\n')
        self.assertEqual(written, self.stamp().read_bytes())
        self.assertIn(b'\ndirty 1\n', written)
        self.assertFalse((self.base / 'from cmake.inputs.tmp').exists())
        bare = self.base / 'no repo'
        bare.mkdir()
        subprocess.run(writer[:2] + [f'-DREPO={bare}'] + writer[2:], check=True, capture_output=True, timeout=120,
                       env=dict(os.environ, GIT_CEILING_DIRECTORIES=str(self.base)))
        self.assertFalse(out.exists())                          # git failed: the old stamp is gone and none is written

    def test_engine(self):
        engine = self.base / 'Invented UE'
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

    def test_git_leaves_the_index_alone(self):
        # Behavioural. A tracked file is rewritten with the same bytes and a new time, so a plain git status would refresh
        # the index (take index.lock, rewrite the index); with --no-optional-locks the preflight's git calls must not.
        # The last step runs a plain status to show that the setup really would have changed the index.
        index = self.wt / '.git' / 'index'
        tracked = self.wt / 'src' / 'core.cpp'
        tracked.write_text('invented\n')
        later = datetime.datetime.now().timestamp() + 5
        os.utime(tracked, (later, later))
        before = (index.read_bytes(), index.stat().st_mtime_ns)
        self.put('local/fresh/one.json')
        self.put('bin/core.lib', old=True)
        self.put('bin/module.dll')
        self.stamp()                                            # its git calls use --no-optional-locks too
        self.assertEqual((index.read_bytes(), index.stat().st_mtime_ns), before)
        rc, lines, out = self.run_ps('-Generators', 'local/fresh=tools/make_data.py', '-CheckBuild',
                                     '-ModuleLibrary', 'bin/module.dll', '-CoreLibrary', 'bin/core.lib')
        self.assertEqual(rc, 0, out)
        self.assertLines(lines, 'build', ['ok', 'ok'])          # same bytes, new time: the stamp's comparison still matches
        self.assertEqual((index.read_bytes(), index.stat().st_mtime_ns), before)
        self.assertFalse((self.wt / '.git' / 'index.lock').exists())
        git(self.wt, 'status', '--porcelain')
        self.assertNotEqual((index.read_bytes(), index.stat().st_mtime_ns), before, 'the setup would not refresh the index')

    def test_one_failing_check_does_not_stop_the_others(self):
        # -ProcessList names a file that does not exist, so the editor check throws; the checks after it still run.
        self.put('local/a/data.json')
        rc, lines, out = self.run_ps('-Data', 'local/a/data.json', '-NoEditor', '-ProcessList', str(self.base / 'none.txt'),
                                     '-ReportHold', 'local/batch.hold')
        self.assertEqual(rc, 1, out)
        self.assertLines(lines, 'data', ['ok'])
        self.assertLines(lines, 'editor', ['FAIL'])
        self.assertIn('editor: stopped by an unexpected error', out)
        self.assertLines(lines, 'hold', ['ok'])
        self.assertIn('PREFLIGHT SUMMARY failed=1 checks=3', out)

    def test_report_file(self):
        report = self.base / 'report.txt'
        rc, _, out = self.run_ps('-Data', 'local/missing.json', '-Report', str(report))
        self.assertEqual(rc, 1, out)
        text = report.read_text()
        self.assertIn('PREFLIGHT FAIL data', text)
        self.assertIn('PREFLIGHT SUMMARY failed=1', text)


for _name, _path in SHELLS.items():
    globals()[f'Preflight_{_name}'] = unittest.skipUnless(sys.platform == 'win32', 'Windows only')(
        type(f'Preflight_{_name}', (Cases, unittest.TestCase), {'POWERSHELL': _path}))


class LongPathTest(unittest.TestCase):
    @unittest.skipUnless(sys.platform == 'win32', 'Windows only')
    def test_short_names_expand(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(long_path(tmp)) / 'a folder with a long name'
            folder.mkdir()
            import ctypes
            size = ctypes.windll.kernel32.GetShortPathNameW(str(folder), None, 0)
            buf = ctypes.create_unicode_buffer(size)
            ctypes.windll.kernel32.GetShortPathNameW(str(folder), buf, size)
            if buf.value == str(folder):
                self.skipTest('this volume makes no 8.3 short names')
            self.assertNotEqual(buf.value, str(folder))
            self.assertEqual(long_path(buf.value), str(folder))


class ShellsTest(unittest.TestCase):
    @unittest.skipUnless(sys.platform == 'win32', 'Windows only')
    def test_windows_powershell_is_among_them(self):
        self.assertIn('powershell', SHELLS)


if __name__ == '__main__':
    unittest.main()
