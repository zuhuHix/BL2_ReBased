"""Imported by the tests that run tools/real_game scripts: every PowerShell run gets a fresh pending-record folder under the temp folder
(OW_TEST_PENDING_DIR with OW_TEST_MODE=synthetic-real-game, read by tools/real_game/pending.ps1, which then also requires the save and
game folders to be under Temp), so no test reads or writes the maintainer's %LOCALAPPDATA%\OpenWillow records,
and a record one run leaves cannot stop the next. A run that sets the variable itself keeps its own.

The child also gets no inherited PSModulePath, so each PowerShell builds its own default module path and no test depends on the shell
it was started from (CI on #263: a CI runner's pwsh step passed PowerShell 7's module path to Windows PowerShell children, which then
could not load New-Guid). A test that sets the module path on purpose passes OW_TEST_KEEP_PSMODULEPATH.

No OPENWILLOW_* variable reaches the child from the machine (CI on #263: a fragment test leaned on the maintainer's OPENWILLOW_BL2, which
the runner does not have): one the test did not set itself (its value is the machine's) is dropped, and OPENWILLOW_BL2 is then a fresh,
empty stand-in game folder under Temp. A test that needs no game folder at all passes OW_TEST_NO_GAME_FOLDER.

remove_tree(): a test folder's removal, retried for a moment when a file in it is still held (a file scanner on Windows)."""
import atexit
import os
import shutil
import subprocess
import sys
import tempfile
import time

_run = subprocess.run


def _run_with_pending_dir(args, *rest, **kwargs):
    if isinstance(args, (list, tuple)) and args and str(args[0]).lower().startswith(('powershell', 'pwsh')):
        env = dict(kwargs.get('env') or os.environ)
        machine = {k.upper(): v for k, v in os.environ.items()}
        env = {k: v for k, v in env.items() if not (k.upper().startswith('OPENWILLOW_') and machine.get(k.upper()) == v)}
        if 'OPENWILLOW_BL2' not in {k.upper() for k in env} and not env.get('OW_TEST_NO_GAME_FOLDER'):
            env['OPENWILLOW_BL2'] = game = tempfile.mkdtemp(prefix='ow-game-')   # a stand-in game folder, empty
            atexit.register(shutil.rmtree, game, True)
        if 'OW_TEST_PENDING_DIR' not in env:
            env['OW_TEST_PENDING_DIR'] = folder = tempfile.mkdtemp(prefix='ow-pending-')
            atexit.register(shutil.rmtree, folder, True)
        if 'OPENWILLOW_UE_LOCK' not in env and env.get('OW_TEST_PENDING_DIR'):
            env['OPENWILLOW_UE_LOCK'] = os.path.join(env['OW_TEST_PENDING_DIR'], 'ue_run.lock')   # test mode needs a synthetic run lock
        if 'OW_TEST_PENDING_DIR' in env and 'OW_TEST_MODE' not in env:
            env['OW_TEST_MODE'] = 'synthetic-real-game'   # pending.ps1 honours the folder only in this mode, on folders under Temp
        if not env.get('OW_TEST_KEEP_PSMODULEPATH'):
            env = {k: v for k, v in env.items() if k.upper() != 'PSMODULEPATH'}
        kwargs['env'] = env
    return _run(args, *rest, **kwargs)


def long_path(path):
    """The long form of an existing path on Windows (GetLongPathNameW: each 8.3 short part, such as RUNNER~1, written out in full);
    elsewhere, or if the call fails, the path unchanged. Junctions and links are not resolved. A test that builds an expected path from a
    temp folder needs this where the tool prints a path PowerShell enumerated: Get-ChildItem's FullName comes in the long form, while
    TEMP can hold the short form (GitHub's Windows runner: its user folder as RUNNER~1, PR #264)."""
    if os.name != 'nt':
        return str(path)
    import ctypes
    buffer = ctypes.create_unicode_buffer(32768)
    size = ctypes.windll.kernel32.GetLongPathNameW(str(path), buffer, len(buffer))
    return buffer.value if 0 < size < len(buffer) else str(path)


def remove_tree(path, tries=25, pause=0.2, raising=True):
    """Remove a test folder. A sharing violation (a file scanner still holding a file) is retried, at most `tries` times `pause` s apart;
    one that keeps failing is raised, so it is reported. With raising=False (a cleanup while the test's own error is already on its way)
    it is printed instead, so it cannot hide that error."""
    for attempt in range(tries):
        try:
            shutil.rmtree(path)
            return
        except FileNotFoundError:
            return
        except PermissionError as error:   # a sharing violation: retried, a bounded number of times
            if attempt == tries - 1:
                if raising:
                    raise
                print(f'test cleanup: {path} not removed: {error}', file=sys.stderr)
                return
            time.sleep(pause)
        except OSError as error:   # any other failure: not retried; raised, or reported without hiding the test's own error (#263, R4)
            if raising:
                raise
            print(f'test cleanup: {path} not removed: {error}', file=sys.stderr)
            return


if subprocess.run is _run:
    subprocess.run = _run_with_pending_dir
