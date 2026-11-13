"""Imported by the tests that run tools/real_game scripts: every PowerShell run gets a fresh pending-record folder under the temp folder
(OW_TEST_PENDING_DIR with OW_TEST_MODE=synthetic-real-game, read by tools/real_game/pending.ps1, which then also requires the save and
game folders to be under Temp), so no test reads or writes the maintainer's %LOCALAPPDATA%\OpenWillow records,
and a record one run leaves cannot stop the next. A run that sets the variable itself keeps its own.

The child also gets no inherited PSModulePath, so each PowerShell builds its own default module path and no test depends on the shell
it was started from (CI on #263: a CI runner's pwsh step passed PowerShell 7's module path to Windows PowerShell children, which then
could not load New-Guid). A test that sets the module path on purpose passes OW_TEST_KEEP_PSMODULEPATH.

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
        except PermissionError as error:
            if attempt == tries - 1:
                if raising:
                    raise
                print(f'test cleanup: {path} not removed: {error}', file=sys.stderr)
                return
            time.sleep(pause)


if subprocess.run is _run:
    subprocess.run = _run_with_pending_dir
