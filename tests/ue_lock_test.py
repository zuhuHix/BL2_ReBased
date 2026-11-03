"""The Unreal run lock is one file per machine, every Unreal launch holds it, and only a dead owner's lock is stale.

AI-assisted. Synthetic: reads the tracked scripts and drives tools/ue_lock.ps1 against temporary lock files; no game
data, and no editor is started.

Static (always run):
1. Every script that names UnrealEditor.exe or UnrealEditor-Cmd.exe dot-sources tools/ue_lock.ps1 and takes the lock
   (Enter-OwUeLock / Invoke-OwUeLocked), and every script that takes it releases it (Exit-OwUeLock).
2. No script builds a per-checkout lock path or opens a lock with CreateNew itself.
Runtime (PowerShell needed). Each test's PowerShell process redefines Get-OwUeBusyProcess after loading the helper
(no process running, or python itself to check the wait), so a real editor elsewhere on the machine does not matter and
the production helper has no test switch:
3. The default path and the absolute override; a relative override is rejected.
4. Enter writes the owner (pid, process start time, script); Exit removes only the caller's own lock.
5. A lock is stale only when its owner process is gone or its start time differs; an old-format lock never is.
6. Stale recovery is atomic: several processes recovering the same stale lock at once end with exactly one owner.
7. A lock taken while an editor starts in the meantime is given back.
8. The real-game helpers take the lock and also wait for Borderlands2; every real-game script that starts the game takes it.
"""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / 'tools' / 'ue_lock.ps1'
POWERSHELL = shutil.which('pwsh') or shutil.which('powershell')
LAUNCH = re.compile(r'UnrealEditor(-Cmd)?\.exe')


def scripts():
    return sorted(p for p in list((ROOT / 'tools').glob('*.ps1')) + list((ROOT / 'tools' / 'real_game').glob('*.ps1'))
                  if p != HELPER)


def text(path):
    return path.read_text(encoding='utf-8-sig', errors='replace')


def code(path):
    """Script text without full-line comments."""
    return '\n'.join(line for line in text(path).splitlines() if not line.lstrip().startswith('#'))


class StaticTest(unittest.TestCase):
    def test_every_unreal_launcher_holds_the_lock(self):
        launchers = [p for p in scripts() if LAUNCH.search(code(p))]
        self.assertGreater(len(launchers), 20)
        for path in launchers:
            body = code(path)
            with self.subTest(script=path.name):
                self.assertRegex(body, r"(?m)^\. \(Join-Path \$PSScriptRoot '(\.\./)?ue_lock\.ps1'\)")
                self.assertRegex(body, r'\b(Enter-OwUeLock|Invoke-OwUeLocked)\b')

    def test_lock_takers_release_it(self):
        for path in scripts():
            body = code(path)
            if re.search(r'\bEnter-OwUeLock\b', body):
                with self.subTest(script=path.name):
                    self.assertRegex(body, r'\bExit-OwUeLock\b')

    def test_no_script_builds_or_opens_its_own_lock(self):
        for path in scripts():
            body = code(path)
            with self.subTest(script=path.name):
                self.assertNotRegex(body, r"Join-Path[^\n]*'(local/)?ue_run\.lock'")
                self.assertNotRegex(body, r'CreateNew')

    def test_real_game_sessions_hold_the_lock_and_wait_for_the_game(self):
        helper = code(ROOT / 'tools' / 'real_game' / 'realgame.ps1')
        self.assertRegex(helper, r"Enter-OwUeLock[^\n]*-AlsoWaitFor\s+'?Borderlands2")
        for path in sorted((ROOT / 'tools' / 'real_game').glob('*.ps1')):
            body = code(path)
            if path.name != 'realgame.ps1' and re.search(r'\bStart-Game\b', body):
                with self.subTest(script=path.name):
                    self.assertRegex(body, r'\b(Enter-RunLock|Enter-OwUeLock)\b')

    def test_helper_has_no_test_switch(self):
        self.assertNotIn('TEST_PROCESS', text(HELPER))

    def test_gate_is_global_only(self):
        # A session-local fallback would be a second gate that does not serialize against the global one.
        helper = code(HELPER)
        self.assertIn(r"'Global\OpenWillowUeLockGate'", helper)
        self.assertNotIn(r'Local\OpenWillowUeLockGate', helper)

    def test_helper_is_loaded_at_script_level_not_invoked(self):
        # `& ue_lock.ps1` would run it in a child scope: the functions would not exist afterwards.
        for path in scripts():
            with self.subTest(script=path.name):
                self.assertNotRegex(code(path), r"&\s*\(Join-Path \$PSScriptRoot '(\.\./)?ue_lock\.ps1'\)")


@unittest.skipUnless(POWERSHELL, 'PowerShell not installed')
class RuntimeTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.lock = Path(self.tmp.name) / 'sub' / 'ue_run.lock'

    def tearDown(self):
        self.tmp.cleanup()

    def command(self, script, busy=None, prelude=''):
        # busy=None: no Unreal process is running; busy='python': this test's own interpreter counts as one.
        mock = ('function Get-OwUeBusyProcess { param([string[]]$AlsoWaitFor) }' if busy is None else
                f"function Get-OwUeBusyProcess {{ param([string[]]$AlsoWaitFor) Get-Process -Name '{busy}' -ErrorAction SilentlyContinue }}")
        return [POWERSHELL, '-NoProfile', '-NonInteractive', '-Command',
                f". '{HELPER}'; {mock}; {prelude}; $ErrorActionPreference = 'Stop'; try {{ {script} }} "
                "catch { [Console]::Error.WriteLine($_.Exception.Message); exit 1 }"]

    def env(self, override=None):
        env = dict(os.environ)
        env['OPENWILLOW_UE_LOCK'] = str(self.lock) if override is None else override
        env['LOCALAPPDATA'] = self.tmp.name
        return env

    def ps(self, script, override=None, busy=None, prelude=''):
        done = subprocess.run(self.command(script, busy, prelude), capture_output=True, text=True,
                              env=self.env(override), timeout=120)
        return done.returncode, done.stdout.strip(), done.stderr.strip()

    def test_paths(self):
        rc, out, _ = self.ps('Get-OwUeLockPath', override='')
        self.assertEqual((rc, Path(out)), (0, Path(self.tmp.name) / 'OpenWillow' / 'ue_run.lock'))
        rc, out, _ = self.ps('Get-OwUeLockPath')
        self.assertEqual((rc, Path(out)), (0, self.lock))
        self.assertTrue(self.lock.parent.is_dir())
        self.assertFalse(self.lock.exists())            # naming the lock never takes it
        for relative in ('rel\\a.lock', '.\\a.lock', '\\a.lock', 'C:a.lock'):
            rc, _, err = self.ps('Get-OwUeLockPath', override=relative)
            self.assertNotEqual(rc, 0, relative)
            self.assertIn('absolute', err)

    def write_lock(self, record):
        self.lock.parent.mkdir(parents=True, exist_ok=True)
        self.lock.write_text(record if isinstance(record, str) else json.dumps(record), encoding='utf-8')

    def owner_of(self, pid):
        rc, out, err = self.ps(f'(Get-Process -Id {pid}).StartTime.ToFileTimeUtc()')
        self.assertEqual(rc, 0, err)
        return int(out)

    def test_staleness_follows_the_owner_process(self):
        # A live process with its own start time: not stale. Same pid, other start time: stale.
        live = os.getpid()
        start = self.owner_of(live)
        self.write_lock({'owner': 't', 'pid': live, 'process_start_utc_filetime': start})
        self.assertEqual(self.ps('Test-OwUeLockStale')[1], 'False')
        self.write_lock({'owner': 't', 'pid': live, 'process_start_utc_filetime': start + 1})
        self.assertEqual(self.ps('Test-OwUeLockStale')[1], 'True')
        # A finished process: stale.
        gone = subprocess.Popen([sys.executable, '-c', 'pass'])
        gone.wait()
        self.write_lock({'owner': 't', 'pid': gone.pid, 'process_start_utc_filetime': 1})
        self.assertEqual(self.ps('Test-OwUeLockStale')[1], 'True')
        # Old one-line format, empty or garbage: never stale, whatever its age.
        for record in ('test_quest 2026-11-01T00:00:00', '', '{"pid": 1}'):
            self.write_lock(record)
            os.utime(self.lock, (0, 0))
            self.assertEqual(self.ps('Test-OwUeLockStale')[1], 'False', record)

    def test_exit_removes_only_the_callers_lock(self):
        self.write_lock({'owner': 'other', 'pid': os.getpid(), 'process_start_utc_filetime': self.owner_of(os.getpid())})
        rc, _, err = self.ps('Exit-OwUeLock')
        self.assertEqual(rc, 0, err)
        self.assertTrue(self.lock.exists())
        self.write_lock('test_quest 2026-11-01T00:00:00')
        self.ps('Exit-OwUeLock')
        self.assertTrue(self.lock.exists())

    def test_enter_records_the_owner_and_exit_releases(self):
        rc, out, err = self.ps("Enter-OwUeLock -Owner 'unit' -TimeoutSeconds 0; Get-Content -Raw (Get-OwUeLockPath); "
                               "Exit-OwUeLock; Test-Path (Get-OwUeLockPath)")
        self.assertEqual(rc, 0, err)
        record, exists = out.rsplit('\n', 1)
        record = json.loads(record)
        self.assertEqual(record['owner'], 'unit')
        self.assertIsInstance(record['pid'], int)
        self.assertIn('process_start_utc_filetime', record)
        self.assertEqual(exists.strip(), 'False')

    def test_enter_waits_while_an_editor_runs(self):
        rc, _, err = self.ps("Enter-OwUeLock -Owner 'unit' -TimeoutSeconds 0", busy='python')   # this test runs in python
        self.assertNotEqual(rc, 0)
        self.assertIn('still present', err)
        self.assertFalse(self.lock.exists())

    def test_enter_waits_for_a_live_owner_and_recovers_a_dead_ones(self):
        self.write_lock({'owner': 'live', 'pid': os.getpid(), 'process_start_utc_filetime': self.owner_of(os.getpid())})
        rc, _, err = self.ps("Enter-OwUeLock -Owner 'unit' -TimeoutSeconds 0")
        self.assertNotEqual(rc, 0)
        self.assertIn('still present', err)
        self.assertIn('"owner":"live"', self.lock.read_text(encoding='utf-8').replace(' ', ''))
        gone = subprocess.Popen([sys.executable, '-c', 'pass'])
        gone.wait()
        self.write_lock({'owner': 'dead', 'pid': gone.pid, 'process_start_utc_filetime': 1})
        rc, out, err = self.ps("Enter-OwUeLock -Owner 'unit' -TimeoutSeconds 0; (Get-OwUeLockOwner).owner; Exit-OwUeLock")
        self.assertEqual(rc, 0, err)
        self.assertTrue(out.endswith('unit'))
        self.assertFalse(self.lock.exists())

    def test_concurrent_stale_recovery_has_one_owner(self):
        # Every competitor judges the dead owner stale and then pauses before acting on it, the widest window for the
        # race. Without the gate, a later competitor would delete the lock an earlier one had just created.
        gone = subprocess.Popen([sys.executable, '-c', 'pass'])
        gone.wait()
        self.write_lock({'owner': 'dead', 'pid': gone.pid, 'process_start_utc_filetime': 1})
        go = time.time() + 8
        slow = ("$orig = ${function:Test-OwUeLockStale}; "
                "function Test-OwUeLockStale([string]$Path) { $r = & $orig $Path; Start-Sleep -Milliseconds 1500; $r }; "
                f"while ([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -lt {int(go * 1000)}) {{ Start-Sleep -Milliseconds 20 }}")
        body = ("Enter-OwUeLock -Owner \"racer$PID\" -TimeoutSeconds 0; "
                "'OWNED ' + (Get-OwUeLockOwner).owner; Start-Sleep -Seconds 8; Exit-OwUeLock")
        racers = [subprocess.Popen(self.command(body, prelude=slow), stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   text=True, env=self.env()) for _ in range(3)]
        outputs = [r.communicate(timeout=180) for r in racers]
        owners = [out for out, _ in outputs if 'OWNED racer' in out]
        refused = [err for _, err in outputs if 'still present' in err]
        self.assertEqual(len(owners), 1, outputs)
        self.assertEqual(len(refused), 2, outputs)
        self.assertFalse(self.lock.exists())

    def test_lock_is_given_back_when_an_editor_appeared(self):
        # The editor shows up only on the second look (after the lock was created): the lock must not be kept.
        prelude = ("$script:looks = 0; function Get-OwUeBusyProcess { param([string[]]$AlsoWaitFor) "
                   "$script:looks++; if ($script:looks -ge 2) { Get-Process -Id $PID } }")
        rc, _, err = self.ps("Enter-OwUeLock -Owner 'unit' -TimeoutSeconds 0", prelude=prelude)
        self.assertNotEqual(rc, 0)
        self.assertIn('still present', err)
        self.assertFalse(self.lock.exists())

    def test_invoke_locked_releases_when_the_body_throws(self):
        rc, _, err = self.ps("try { Invoke-OwUeLocked -Owner 'unit' -TimeoutSeconds 0 -Body { throw 'boom' } } catch { }; "
                             "if (Test-Path (Get-OwUeLockPath)) { exit 3 }")
        self.assertEqual(rc, 0, err)


if __name__ == '__main__':
    unittest.main()
