"""Imported by the tests that run tools/real_game scripts: every PowerShell run gets a fresh pending-record folder under the temp folder
(OW_TEST_PENDING_DIR with OW_TEST_MODE=synthetic-real-game, read by tools/real_game/pending.ps1, which then also requires the save and
game folders to be under Temp), so no test reads or writes the maintainer's %LOCALAPPDATA%\OpenWillow records,
and a record one run leaves cannot stop the next. A run that sets the variable itself keeps its own."""
import atexit
import os
import shutil
import subprocess
import tempfile

_run = subprocess.run


def _run_with_pending_dir(args, *rest, **kwargs):
    if isinstance(args, (list, tuple)) and args and str(args[0]).lower().startswith(('powershell', 'pwsh')):
        env = dict(kwargs.get('env') or os.environ)
        if 'OW_TEST_PENDING_DIR' not in env:
            env['OW_TEST_PENDING_DIR'] = folder = tempfile.mkdtemp(prefix='ow-pending-')
            atexit.register(shutil.rmtree, folder, True)
        if 'OW_TEST_PENDING_DIR' in env and 'OW_TEST_MODE' not in env:
            env['OW_TEST_MODE'] = 'synthetic-real-game'   # pending.ps1 honours the folder only in this mode, on folders under Temp
        kwargs['env'] = env
    return _run(args, *rest, **kwargs)


if subprocess.run is _run:
    subprocess.run = _run_with_pending_dir
