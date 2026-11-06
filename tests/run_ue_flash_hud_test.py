"""End-to-end test of tools/run_ue_flash_hud.ps1 with no editor: the real launcher, lock helper and serve.py, copied to a temporary repository with
empty movie files, and a stand-in for the editor (-EditorExe): pythonw running the temporary project file, which is a small Python script. It is not named
UnrealEditor.exe: on a shared PC that name would make other lanes' suites refuse to start. Windows only. The lock is a temporary file (OPENWILLOW_UE_LOCK), but
Enter-OwUeLock also waits while a real Unreal editor runs, so with one up nothing can be tested: this prints NOT RUN and exits nonzero unless --allow-skip is given.
Stops only processes it started, by id, after checking the command line."""
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
STUB = """import os, subprocess, sys, time
open(os.environ['OW_STUB_PID'], 'w').write(str(os.getpid()))
mode = os.environ['OW_STUB_MODE']
if mode == 'stall':
    # A worker that outlives the editor; the launcher must name it and leave it. The marker is the temporary folder, for the cleanup's check.
    child = subprocess.Popen([sys.executable, '-c', 'import sys, time; time.sleep(40)', os.environ['OW_STUB_PID']])
    open(os.environ['OW_STUB_PID'] + '.child', 'w').write(str(child.pid))
    time.sleep(120)
sys.exit(3 if mode == 'crash' else 0)
"""


def powershell(script):
    return subprocess.run(['powershell', '-NoProfile', '-NonInteractive', '-Command', script], capture_output=True, text=True, timeout=120)


def real_editor_running():
    return 'UnrealEditor' in powershell('Get-Process UnrealEditor,UnrealEditor-Cmd -ErrorAction SilentlyContinue | ForEach-Object Name').stdout


def free_port():
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        return probe.getsockname()[1]


def serving(port):
    try:
        urllib.request.urlopen(f'http://127.0.0.1:{port}/', timeout=2)
        return True
    except OSError:
        return False


def alive(pid):
    return str(pid) in subprocess.run(['tasklist', '/FI', f'PID eq {pid}', '/NH'], capture_output=True, text=True).stdout


@unittest.skipUnless(os.name == 'nt', 'the launcher is a Windows script')
class Launcher(unittest.TestCase):
    def setUp(self):
        if real_editor_running():
            self.fail('NOT RUN: an editor is running (the launcher would wait for it)')
        self.folder = Path(tempfile.mkdtemp(prefix='ow_launcher_'))
        self.addCleanup(shutil.rmtree, self.folder, ignore_errors=True)
        tools = self.folder / 'tools'
        shutil.copytree(REPO / 'tools' / 'hud_overlay', tools / 'hud_overlay', ignore=shutil.ignore_patterns('__pycache__'))
        for name in ('run_ue_flash_hud.ps1', 'run_limit.ps1', 'ue_lock.ps1', 'check_overlay_data.py'):
            if (REPO / 'tools' / name).exists():
                shutil.copy(REPO / 'tools' / name, tools / name)
        project = self.folder / 'host' / 'ue5' / 'OpenWillow' / 'OpenWillow.uproject'
        project.parent.mkdir(parents=True)
        project.write_text(STUB)
        movies = self.folder / 'local' / 'ui' / 'run'
        for name in ('ruffle/ruffle.js', 'UI_HUD/harness.swf', 'UI_HUD/HUD.swf'):
            (movies / name).parent.mkdir(parents=True, exist_ok=True)
            (movies / name).write_bytes(b'')
        (self.folder / 'game').mkdir()
        self.lock = self.folder / 'run.lock'
        self.pid_file = self.folder / 'stub.pid'
        self.port = free_port()
        # Before anything starts, so a launcher that hangs or a test that times out leaves nothing behind.
        self.addCleanup(self.stop_stubs)
        self.addCleanup(self.stop_server_on_port)

    def run_launcher(self, mode, *arguments, timeout=120, check_lock=True):
        environment = dict(os.environ, OPENWILLOW_UE_LOCK=str(self.lock), OW_STUB_MODE=mode, OW_STUB_PID=str(self.pid_file))
        stand_in = Path(sys.executable).with_name('pythonw.exe')
        done = subprocess.run(['powershell', '-NoProfile', '-NonInteractive', '-File', str(self.folder / 'tools' / 'run_ue_flash_hud.ps1'),
                               '-Engine', 'not-used', '-Game', str(self.folder / 'game'), '-Port', str(self.port), '-EditorExe', str(stand_in),
                               '-GameWindow', *arguments], capture_output=True, text=True, timeout=timeout, env=environment)
        if check_lock:
            self.assertFalse(self.lock.exists(), 'the lock must be released')
        return done

    def stop_stubs(self):
        # The stand-in editor and its child, by id, each only if its command line names this temporary folder.
        for file in (self.pid_file, Path(str(self.pid_file) + '.child')):
            if file.exists():
                powershell(f"$p = Get-CimInstance Win32_Process -Filter 'ProcessId={file.read_text()}'; "
                           f"if ($p -and $p.CommandLine -like '*{self.folder.name}*') {{ Stop-Process -Id $p.ProcessId -Force }}")

    def stop_server_on_port(self):
        # Only a server of this temporary repository, by id. A server this test started and handed to the launcher is its own business.
        powershell(f"$c = Get-NetTCPConnection -LocalPort {self.port} -State Listen -ErrorAction SilentlyContinue | Select -First 1; "
                   f"if ($c) {{ $p = Get-CimInstance Win32_Process -Filter \"ProcessId=$($c.OwningProcess)\"; "
                   f"if ($p.CommandLine -like '*{self.folder.name}*serve.py*') {{ Stop-Process -Id $p.ProcessId -Force }} }}")

    def test_stall_times_out_fails_and_stops_both(self):
        done = self.run_launcher('stall', '-Extra', '-owinvshots', '-TimeoutSeconds', '4')
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('stalled, stopped', ' '.join((done.stdout + done.stderr).split()))
        self.assertFalse(alive(self.pid_file.read_text()), 'the stand-in editor must be stopped')
        self.assertFalse(serving(self.port), 'the page server it started must be stopped')
        child = Path(str(self.pid_file) + '.child').read_text()
        self.assertIn(f'pid {child} (not stopped)', ' '.join((done.stdout + done.stderr).split()))
        self.assertTrue(alive(child), 'a child is named, not stopped')

    def test_cleanup_works_when_the_launcher_itself_hangs(self):
        with self.assertRaises(subprocess.TimeoutExpired):  # no limit on a stalled stand-in: the harness times out, and the launcher is gone
            self.run_launcher('stall', '-Extra', '-owinvshots', '-TimeoutSeconds', '0', timeout=8, check_lock=False)
        stand_in = self.pid_file.read_text()
        self.assertTrue(alive(stand_in), 'the leak this guards against')
        self.doCleanups()
        self.assertFalse(alive(stand_in))
        self.assertFalse(serving(self.port))

    def test_crash_fails_and_stops_the_server(self):
        done = self.run_launcher('crash', '-Extra', '-owinvshots')
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('exited with code 3', ' '.join((done.stdout + done.stderr).split()))
        self.assertFalse(serving(self.port))

    def test_clean_exit_is_zero_and_the_server_stays(self):
        done = self.run_launcher('clean', '-Extra', '-owinvshots')
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertTrue(serving(self.port), 'a later run reuses it')

    def test_a_server_it_did_not_start_is_left_alone(self):
        # Started by this test from the temporary tools, as an earlier run would have.
        server = subprocess.Popen([sys.executable, str(self.folder / 'tools' / 'hud_overlay' / 'serve.py'), '--port', str(self.port)],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.addCleanup(server.wait)
        self.addCleanup(server.kill)
        for _ in range(50):
            if serving(self.port):
                break
            time.sleep(.1)
        done = self.run_launcher('stall', '-Extra', '-owinvshots', '-TimeoutSeconds', '4')
        self.assertNotEqual(done.returncode, 0)
        self.assertFalse(alive(self.pid_file.read_text()))
        self.assertIsNone(server.poll(), 'a server this run did not start is not its to stop')
        self.assertTrue(serving(self.port))


if __name__ == '__main__':
    allow_skip = '--allow-skip' in sys.argv
    if allow_skip:
        sys.argv.remove('--allow-skip')
    if os.name == 'nt' and real_editor_running():
        print('NOT RUN: an editor is running')
        sys.exit(0 if allow_skip else 2)
    unittest.main()
