"""Synthetic tests for the mission screen capture (no game, no game files): the end of its run is stubbed the way
tests/realgame_snapshot_test.py does for the Phaselock capture, and the probe's game-free helpers run on invented values. Needs Windows PowerShell.

Run: python tests/mission_screen_capture_test.py
"""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tests'))
import realgame_snapshot_test as base  # noqa: E402

CAPTURE = ROOT / 'tools/real_game/mission_screen_capture.ps1'


def run_tail(body, clean=True, proc='$null'):
    """The capture's own end (its catch and finally), after a stubbed body; 'hit' is the one requested stage in the harness."""
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    tail = text[text.index('\n} catch {\n    $runError') + 1:]
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'driver.ps1'
        script.write_text(base.TAIL.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__TMP__', tmp)
                          .replace('__SETUP__', '').replace('__BODY__', body).replace('__CAPTURE_TAIL__', tail).replace('__PROC__', proc)
                          .replace('__CLEAN__', '$true' if clean else '$false'), encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True,
                              text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp), timeout=180)
        done.hub_lock_kept = (Path(tmp) / 'hub.lock').exists()
        return done


def test_exit_0_only_for_a_completed_run_with_nothing_to_recover():
    finish = "$stagesDone.Add('hit')"
    never_exits = '[pscustomobject]@{ HasExited = $false; Id = 1 }'
    for body, clean, proc, code, status in ((finish, True, '$null', 0, 'completed'),
                                            (finish, False, '$null', 1, 'recovery needed: restore'),
                                            ("'no stage'", True, '$null', 1, 'incomplete: hit'),
                                            ("throw 'x'", True, '$null', 1, 'failed: x'),
                                            (finish, True, never_exits, 1, 'recovery needed: game exit')):
        done = run_tail(body, clean, proc)
        assert done.returncode == code and f'status: {status}' in done.stdout, (body, clean, proc, done.returncode, done.stdout, done.stderr)
        assert done.hub_lock_kept is status.startswith('recovery'), (body, clean, proc, done.stdout)


def test_the_capture_is_wired_to_the_safety_helpers():
    text = CAPTURE.read_text(encoding='utf-8')
    for call in ('$snapshots[$d] = Assert-SnapshotMatches', '$snapshots[$d] = New-VerifiedSnapshot', 'Restore-VerifiedRoots $bak $roots $snapshots',
                 'Save-VerifiedBackupRecord $bak $roots $snapshots', "$restoreCommand = \"Restore-VerifiedBackup '$bak'\"", 'Get-RunStatus $stagesRequested',
                 'Install-Driver -BlockSavesAtStart -Exclusive', 'if ($holdLocks) { $runLock = $false; $hub = $null }', 'Format-RestoreRows $rows',
                 "if ($log.status -ne 'completed') { exit 1 }", "sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings')",
                 "SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver')"):
        assert call in text, call
    assert '-Name Borderlands2' not in text and 'taskkill' not in text


def test_the_capture_parses_and_every_stage_is_marked():
    command = ("$e = $null; [void][Management.Automation.Language.Parser]::ParseFile('" + str(CAPTURE) + "', [ref]$null, [ref]$e); "
               "if ($e) { $e | ForEach-Object { $_.Message }; exit 1 }")
    done = subprocess.run(['powershell', '-NoProfile', '-Command', command], capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stdout + done.stderr
    text = CAPTURE.read_text(encoding='utf-8')
    requested = text[text.index('$stagesRequested = @('):].split('\n')[0]
    for stage in ('start', 'offer', 'offer_esc', 'accept', 'ready', 'turnin', 'reward', 'reward_taken', 'closed', 'respawn'):
        assert f"'{stage}'" in requested, stage
        assert f"Set-Stage '{stage}'" in text, stage


def load_probe():
    """The probe with its game modules stubbed; only the helpers that need no game are called."""
    sdk = types.ModuleType('unrealsdk')
    hooks = types.ModuleType('unrealsdk.hooks')
    hooks.Type = types.SimpleNamespace(PRE=0, POST=1)
    hooks.add_hook = hooks.remove_hook = lambda *a: None
    sys.modules.update({'unrealsdk': sdk, 'unrealsdk.hooks': hooks})
    spec = importlib.util.spec_from_file_location('mission_screen_probe', ROOT / 'tools/real_game/scripts/mission_screen_probe.py')
    probe = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(probe)
    return probe


def test_probe_helpers():
    probe = load_probe()
    first, second = probe.MS_TEXT[0], probe.MS_TEXT[1]

    def read(path):
        if path == first:
            return 'invented title'
        if path == second:
            raise RuntimeError('no such field')
        return ''
    picked = probe.ms_pick_text(read)
    assert picked[first] == 'invented title' and picked[second].startswith('error') and len(picked) == 2, picked
    assert probe.ms_turnin_ready(3) and not probe.ms_turnin_ready(1) and not probe.ms_turnin_ready(4)
    # Every hooked function is named once, so a row always says which call it was.
    names = [(cls, name) for cls, name, _ in probe.MS_FUNCS]
    assert len(names) == len(set(names))


if __name__ == '__main__':
    test_probe_helpers()
    test_the_capture_parses_and_every_stage_is_marked()
    test_the_capture_is_wired_to_the_safety_helpers()
    test_exit_0_only_for_a_completed_run_with_nothing_to_recover()
    print('ok')
