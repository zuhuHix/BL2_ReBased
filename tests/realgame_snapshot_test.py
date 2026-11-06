"""Synthetic tests for the real-game backup and status helpers of tools/real_game/realgame.ps1 (no game, no game files; temporary
folders hold invented text). Also parses the Phaselock capture script. Needs Windows PowerShell.

Run: python tests/realgame_snapshot_test.py
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BODY = r"""
$ErrorActionPreference = 'Stop'
. '__REALGAME__'
$out = [ordered]@{}
$tmp = '__TMP__'
function Tree($Name, $Files) {
    $d = Join-Path $tmp $Name; New-Item -ItemType Directory -Force $d | Out-Null
    foreach ($f in $Files) { Set-Content -LiteralPath (Join-Path $d $f) "text of $f" }
    $d
}
$rootsOf = { param($names) $r = [ordered]@{}; foreach ($n in $names) { $r[$n] = Join-Path $tmp "live_$n" }; $r }

# A good snapshot passes; a copy that stops partway throws, and a partial tree is not marked usable.
$live = Tree 'live_a' @('one.txt', 'two.txt')
New-VerifiedSnapshot $live (Join-Path $tmp 'snap_ok')
$out.good_snapshot = Test-Path (Join-Path $tmp 'snap_ok/two.txt')
function Copy-Item { param([switch]$Recurse, [switch]$Force, [string]$LiteralPath, [Parameter(Position = 0)][string]$Destination)
    New-Item -ItemType Directory -Force $Destination | Out-Null; Set-Content (Join-Path $Destination 'one.txt') 'text of one.txt' }
$snapshots = @{}
try { New-VerifiedSnapshot $live (Join-Path $tmp 'snap_partial'); $snapshots.a = 'verified' } catch { $out.partial_threw = "$_" }
Remove-Item Function:\Copy-Item
$out.partial_marked = $snapshots.ContainsKey('a')

# A root with no verified snapshot keeps its live files, even with a partial tree in the backup.
$bak = Join-Path $tmp 'bak'
Tree 'bak/a' @('one.txt') | Out-Null
$r = Restore-VerifiedRoots $bak (& $rootsOf @('a')) $snapshots
$out.unverified_kept = (Test-Path (Join-Path $live 'two.txt'))
$out.unverified_clean = $r.clean
$out.unverified_summary = $r.summary

# A verified root is restored and compared.
$out.flag_is_not_enough = (Restore-VerifiedRoots $bak (& $rootsOf @('a')) @{ a = 'verified' }).summary
$hashes = Get-TreeHashes (Join-Path $bak 'a')
$r = Restore-VerifiedRoots $bak (& $rootsOf @('a')) @{ a = $hashes }
$out.verified_summary = $r.summary; $out.verified_clean = $r.clean; $out.verified_removed = !(Test-Path (Join-Path $live 'two.txt'))

# A backup file that vanished after verification: the live file stays, the root is not restored, and it is not clean.
$liveB = Tree 'live_b' @('keep.txt', 'also.txt')
$hashesB = New-VerifiedSnapshot $liveB (Join-Path $bak 'b')
Remove-Item (Join-Path $bak 'b/keep.txt')
$r = Restore-VerifiedRoots $bak (& $rootsOf @('b')) @{ b = $hashesB }
$out.vanished_kept = Test-Path (Join-Path $liveB 'keep.txt'); $out.vanished_clean = $r.clean; $out.vanished_changed = @($r.changed)
$out.vanished_row = @($r.rows)[0].status

# A root absent at the backup and created during the run stays, is named, and the summary does not say identical.
Tree 'live_new' @('store.txt') | Out-Null
$r = Restore-VerifiedRoots $bak (& $rootsOf @('new')) @{ new = 'absent' }
$out.created_kept = Test-Path (Join-Path $tmp 'live_new/store.txt'); $out.created_summary = $r.summary; $out.created_clean = $r.clean
Remove-Item -Recurse (Join-Path $tmp 'live_new')
$r = Restore-VerifiedRoots $bak (& $rootsOf @('new')) @{ new = 'absent' }
$out.still_absent_summary = $r.summary; $out.still_absent_clean = $r.clean

# A restore that throws, or a compare that still differs, is not clean.
function Restore-Tree { throw 'disk full' }
$r = Restore-VerifiedRoots $bak (& $rootsOf @('a')) @{ a = $hashes }
$out.failed_clean = $r.clean; $out.failed_summary = $r.summary
function Restore-Tree { [pscustomobject]@{ file = 'a\x'; status = 'MISMATCH after restore' } }
$r = Restore-VerifiedRoots $bak (& $rootsOf @('a')) @{ a = $hashes }
$out.mismatch_clean = $r.clean
Remove-Item Function:\Restore-Tree

# Status and frame checks.
$out.status_all = Get-RunStatus @('hit', 'miss') @('miss', 'hit') ''
$out.status_missing = Get-RunStatus @('hit', 'miss') @('miss') ''
$out.status_none = Get-RunStatus @('hit') @() ''
$out.status_error = Get-RunStatus @('hit') @('hit') 'cooldown did not end'
$out.status_recovery = Get-RunStatus @('hit') @('hit') '' 'restore did not compare clean'
$out.status_recovery_first = Get-RunStatus @('hit') @() 'x' 'game exit not confirmed'
$out.capture_ok = Test-CaptureComplete ([pscustomobject]@{ frames = 3; shots = @{ a = 1.0 } }) @{ a = 1.0 }
$out.capture_shot_missing = Test-CaptureComplete ([pscustomobject]@{ frames = 3; shots = @{} }) @{ a = 1.0 }
$out.capture_no_frames = Test-CaptureComplete ([pscustomobject]@{ frames = 0; shots = @{} })
$out.capture_null = Test-CaptureComplete $null

$errors = $null
[void][Management.Automation.Language.Parser]::ParseFile('__CAPTURE__', [ref]$null, [ref]$errors)
$out.capture_parse_errors = @($errors).Count
$out | ConvertTo-Json -Depth 3
"""


def run():
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'driver.ps1'
        work = Path(tmp) / 'work'
        work.mkdir()
        script.write_text(BODY.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1'))
                          .replace('__CAPTURE__', str(ROOT / 'tools/real_game/phaselock_integrity_capture.ps1'))
                          .replace('__TMP__', str(work)), encoding='utf-8-sig')
        env = dict(os.environ, OPENWILLOW_BL2=str(work))
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)],
                              capture_output=True, text=True, env=env, timeout=180)
        assert done.returncode == 0, done.stdout + done.stderr
        return json.loads(done.stdout[done.stdout.index('{'):])


def test_helpers():
    r = run()
    assert r['good_snapshot'] is True
    assert 'differs' in r['partial_threw'] and r['partial_marked'] is False
    assert r['unverified_kept'] is True and r['unverified_clean'] is False and 'NOT IDENTICAL' in r['unverified_summary'] and 'a not backed up' in r['unverified_summary']
    assert 'not backed up' in r['flag_is_not_enough']       # only the recorded hashes make a root restorable
    assert r['vanished_kept'] is True and r['vanished_clean'] is False and r['vanished_changed'] == ['b'] and r['vanished_row'].startswith('BACKUP CHANGED')
    assert r['status_recovery'] == 'recovery needed: restore did not compare clean' and r['status_recovery_first'] == 'recovery needed: game exit not confirmed'
    assert r['verified_clean'] is True and r['verified_removed'] is True and r['verified_summary'].startswith('identical')
    assert r['created_kept'] is True and r['created_clean'] is False and 'NOT IDENTICAL' in r['created_summary'] and 'new created during the run' in r['created_summary']
    assert r['still_absent_summary'].startswith('identical') and r['still_absent_clean'] is True
    assert r['failed_clean'] is False and 'NOT IDENTICAL' in r['failed_summary'] and r['mismatch_clean'] is False
    assert r['status_all'] == 'completed' and r['status_missing'] == 'incomplete: hit' and r['status_none'] == 'incomplete: hit'
    assert r['status_error'] == 'failed: cooldown did not end'
    assert r['capture_ok'] is True and r['capture_shot_missing'] is False and r['capture_no_frames'] is False and r['capture_null'] is False
    assert r['capture_parse_errors'] == 0


# The capture script's own tail, from its catch to its end, after a body that fails or finishes, with a restore that reports a mismatch.
TAIL = r"""
$ErrorActionPreference = 'Stop'
. '__REALGAME__'
function Restore-VerifiedRoots {
    if (__CLEAN__) { [pscustomobject]@{ rows = @(); summary = 'identical to bak after 0 restores'; clean = $true } }
    else { [pscustomobject]@{ rows = @([pscustomobject]@{ file = 'SaveData\x'; status = 'MISMATCH after restore' });
                              summary = 'NOT IDENTICAL to bak (1 problems)'; clean = $false } }
}
$roots = [ordered]@{ SaveData = '__TMP__\live' }
$Out = '__OUT__'; $bak = '__TMP__'; $HubLock = '__TMP__\hub.lock'; $Lane = 'test'
Set-Content -LiteralPath $HubLock 'test now'
$log = [ordered]@{}; $snapshots = @{ SaveData = @{} }; $extraRoots = [ordered]@{}; $stagesRequested = @('hit'); $runError = ''
$stagesDone = New-Object Collections.Generic.List[string]; $stageProblems = New-Object Collections.Generic.List[string]
__SETUP__
$hub = 'held'; $proc = __PROC__; $procStart = $null; $runLock = $false; $driverOwned = __OWNED__; $marker = 'x'
__STUBS__
try {
    __BODY__
__CAPTURE_TAIL__
"""


def run_tail(body, clean=False, proc='$null', setup='', after=None, owned='$true', stubs="function Remove-Driver { }", out='__TMP__'):
    text = (ROOT / 'tools/real_game/phaselock_integrity_capture.ps1').read_text(encoding='utf-8').replace('\r\n', '\n')
    tail = text[text.index('\n} catch {\n    $runError') + 1:]
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'driver.ps1'
        script.write_text(TAIL.replace('__OUT__', out).replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__TMP__', tmp)
                          .replace('__SETUP__', setup.replace('__TMP__', tmp)).replace('__OWNED__', owned).replace('__STUBS__', stubs.replace('__TMP__', tmp)).replace('__BODY__', body).replace('__CAPTURE_TAIL__', tail).replace('__PROC__', proc)
                          .replace('__CLEAN__', '$true' if clean else '$false'), encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True,
                              text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp), timeout=180)
        done.hub_lock_kept = (Path(tmp) / 'hub.lock').exists()
        if after:
            after(tmp, done)
        return done


def test_a_failed_run_with_a_bad_restore_still_prints_the_rows_and_the_recovery():
    for body in ("throw 'cooldown did not end'", "'body finished'"):
        status = 'status: recovery needed: restore did not compare clean'
        done = run_tail(body)
        assert done.returncode == 1, done.stdout + done.stderr
        assert done.hub_lock_kept is True       # the lock stays while the restore needs recovery
        order = [done.stdout.find(text) for text in (status, 'MISMATCH after restore', 'MANUAL RECOVERY', 'Restore-VerifiedBackup')]
        assert -1 not in order and order == sorted(order), done.stdout


def test_exit_0_only_for_a_completed_run_with_nothing_to_recover():
    finish = "$stagesDone.Add('hit')"
    # (body, restore clean, game process, expected exit, expected status)
    never_exits = '[pscustomobject]@{ HasExited = $false; Id = 1 }'
    for body, clean, proc, code, status in ((finish, True, '$null', 0, 'completed'),
                                            (finish + '; return', True, '$null', 0, 'completed'),   # -ProbeOnly's return passes through the finally
                                            (finish, False, '$null', 1, 'recovery needed: restore'),   # stages done, bad restore
                                            ("'no stage'", True, '$null', 1, 'incomplete: hit'),
                                            ("'no stage'; return", True, '$null', 1, 'incomplete: hit'),
                                            ("throw 'x'", True, '$null', 1, 'failed: x'),
                                            (finish, True, never_exits, 1, 'recovery needed: game exit')):
        done = run_tail(body, clean, proc)
        assert done.returncode == code and f'status: {status}' in done.stdout, (body, clean, proc, done.returncode, done.stdout, done.stderr)
        # A clean restore releases the hub lock; recovery keeps it.
        assert done.hub_lock_kept is status.startswith('recovery'), (body, clean, proc, done.stdout)


def test_a_leftover_driver_holds_the_locks_and_says_what_to_do():
    # Remove-Driver does nothing here, and the folder stays. The restore is clean.
    make = "New-Item -ItemType Directory -Force (Join-Path $script:DriverDir 'x') | Out-Null"
    done = run_tail("$stagesDone.Add('hit')", True, '$null', make)
    assert done.returncode == 1 and 'status: recovery needed: driver not removed' in done.stdout and done.hub_lock_kept is True, done.stdout
    assert 'MANUAL RECOVERY' in done.stdout and 'Remove-Item -Recurse -Force' in done.stdout, done.stdout
    # Remove-Driver throws.
    done = run_tail("$stagesDone.Add('hit')", True, stubs="function Remove-Driver { throw 'in use' }")
    assert done.returncode == 1 and 'driver not removed (error: in use)' in done.stdout and done.hub_lock_kept is True, done.stdout


def test_a_missing_evidence_file_cannot_exit_0_and_the_restore_still_runs():
    marker = "function Restore-VerifiedRoots { Set-Content '__TMP__\\restored.txt' 'x'; [pscustomobject]@{ rows = @(); summary = 'identical'; clean = $true } }"
    done = run_tail("$stagesDone.Add('hit')", True, stubs='function Remove-Driver { }\n' + marker, out='__TMP__\\no\\such\\folder',
                    after=lambda tmp, d: setattr(d, 'restored', (Path(tmp) / 'restored.txt').exists()))
    assert done.returncode == 1 and 'status: failed: evidence not written' in done.stdout and done.restored is True, done.stdout


def test_a_failed_run_stops_the_hooks_while_the_game_is_alive():
    alive = '[pscustomobject]@{ HasExited = $false; Id = 1 }'
    calls = "function Remove-Driver { }\nfunction Py([string]$Code, [int]$Timeout = 30) { 'py: ' + $Code | Write-Host; if (__FAILS__) { throw 'no reply' } }"
    done = run_tail("throw 'boom'", True, alive, stubs=calls.replace('__FAILS__', '$false'))
    assert 'py: pi_stop(); unmark_lock()' in done.stdout and 'cleanup hooks: stopped' in done.stdout, done.stdout
    done = run_tail("throw 'boom'", True, alive, stubs=calls.replace('__FAILS__', '$true'))
    assert 'cleanup hooks: not stopped' in done.stdout and 'cleanup game:' in done.stdout, done.stdout    # the rest of the cleanup still runs
    done = run_tail("$stagesDone.Add('hit')", True, '[pscustomobject]@{ HasExited = $true; Id = 1 }', stubs=calls.replace('__FAILS__', '$true'))
    assert 'py:' not in done.stdout and 'cleanup hooks' not in done.stdout, done.stdout   # an exited game is not asked


def test_a_run_that_touched_nothing_has_nothing_to_recover():
    # The backup failed partway, before the driver or the game: no restore, no recovery, the locks released, and still not completed.
    done = run_tail("throw 'copy failed'", True, owned='$false', stubs="function Restore-VerifiedRoots { throw 'must not run' }")
    assert 'status: failed: copy failed' in done.stdout and 'cleanup saves: not touched' in done.stdout and done.hub_lock_kept is False, done.stdout
    assert done.returncode == 1 and 'MANUAL RECOVERY' not in done.stdout, done.stdout


GUARDS = r"""
$ErrorActionPreference = 'Stop'
. '__REALGAME__'
$out = [ordered]@{}
foreach ($reply in '4', ' 4', "4`n", 'True', 'block', '0', '-1', '', '4 saves', "4`n5", 'Traceback: block_saves failed') {
    try { $out["saves [$reply]"] = Assert-SavesBlocked $reply } catch { $out["saves [$reply]"] = 'refused' }
}
try { $out['saves null'] = Assert-SavesBlocked $null } catch { $out['saves null'] = 'refused' }

$script:focus = 0
function Get-GameProcess { [pscustomobject]@{ MainWindowHandle = 123 } }
function Focus-Game { $script:focus++; $false }
function Get-ForegroundWindowHandle { 999 }
foreach ($call in @{ n = 'KeyDown'; s = { Send-KeyDown F } }, @{ n = 'Key'; s = { Send-Key F } }, @{ n = 'ClickAt'; s = { Send-ClickAt 1 1 } }, @{ n = 'Wheel'; s = { Send-Wheel 1 } },
                  @{ n = 'Drag'; s = { Send-Drag 1 1 } }, @{ n = 'Mouse'; s = { Send-Mouse 1 1 } }, @{ n = 'Click'; s = { Send-Click } }) {
    try { & $call.s; $out["send $($call.n)"] = 'sent' } catch { $out["send $($call.n)"] = "$_" }
}
$out.focus_attempts_each = $script:focus / 7
$script:focus = 0
function Focus-Game { $script:focus++; $script:fg = 123; $true }
$script:fg = 999
function Get-ForegroundWindowHandle { $script:fg }
try { Assert-GameForeground; $out.refocus = "ok after $script:focus" } catch { $out.refocus = "$_" }
function Get-GameProcess { $null }
try { Assert-GameForeground; $out.no_game = 'ok' } catch { $out.no_game = "$_" }
$out | ConvertTo-Json -Depth 3
"""


def test_the_guards_refuse():
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'guards.ps1'
        script.write_text(GUARDS.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')), encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True,
                              env=dict(os.environ, OPENWILLOW_BL2=tmp), timeout=180)
    assert done.returncode == 0, done.stdout + done.stderr
    r = json.loads(done.stdout[done.stdout.index('{'):])
    assert r['saves [4]'] == 4 and r['saves [ 4]'] == 4 and r['saves [4\n]'] == 4, r
    for reply in ('True', 'block', '0', '-1', '', '4 saves', '4\n5', 'Traceback: block_saves failed'):
        assert r[f'saves [{reply}]'] == 'refused', reply
    assert r['saves null'] == 'refused'
    for name in ('KeyDown', 'Key', 'ClickAt', 'Wheel', 'Drag', 'Mouse', 'Click'):
        assert 'not the foreground window' in r[f'send {name}'], (name, r[f'send {name}'])
    assert r['focus_attempts_each'] == 3 and r['refocus'] == 'ok after 1' and 'not running' in r['no_game'], r


def test_the_printed_restore_refuses_a_backup_that_changed_after_an_unconfirmed_exit():
    # A real record and backup, a backup file that vanishes, and the game's exit unconfirmed. Then the printed command, in a new
    # process as a person would run it: the in-memory hashes are gone by then.
    setup = r"""
New-Item -ItemType Directory -Force '__TMP__\live' | Out-Null
Set-Content '__TMP__\live\keep.txt' 'kept'; Set-Content '__TMP__\live\other.txt' 'other'
$snapshots = @{ SaveData = New-VerifiedSnapshot '__TMP__\live' '__TMP__\SaveData' }
Save-VerifiedBackupRecord '__TMP__' $roots $snapshots
Remove-Item '__TMP__\SaveData\keep.txt'
"""

    def run_printed(tmp, done):
        line = next(l.strip() for l in done.stdout.splitlines() if l.strip().startswith('Restore-VerifiedBackup'))
        script = Path(tmp) / 'by_hand.ps1'
        script.write_text(f". '{ROOT / 'tools/real_game/realgame.ps1'}'\n{line}\n", encoding='utf-8-sig')
        hand = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True,
                              text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp), timeout=180)
        assert 'BACKUP CHANGED' in hand.stdout and 'NOT IDENTICAL' in hand.stdout, hand.stdout + hand.stderr
        assert (Path(tmp) / 'live/keep.txt').exists(), 'the live file was deleted'
        assert (Path(tmp) / 'live/other.txt').exists()

    done = run_tail("$stagesDone.Add('hit')", True, '[pscustomobject]@{ HasExited = $false; Id = 1 }', setup, run_printed)
    assert 'status: recovery needed: game exit' in done.stdout and 'Restore-Tree' not in done.stdout, done.stdout


def test_the_printed_restore_restores_an_intact_backup():
    setup = r"""
New-Item -ItemType Directory -Force '__TMP__\live' | Out-Null
Set-Content '__TMP__\live\keep.txt' 'kept'
$snapshots = @{ SaveData = New-VerifiedSnapshot '__TMP__\live' '__TMP__\SaveData' }
Save-VerifiedBackupRecord '__TMP__' $roots $snapshots
Set-Content '__TMP__\live\keep.txt' 'changed by the run'
"""

    def run_printed(tmp, done):
        script = Path(tmp) / 'by_hand.ps1'
        script.write_text(f". '{ROOT / 'tools/real_game/realgame.ps1'}'\nRestore-VerifiedBackup '{tmp}'\n", encoding='utf-8-sig')
        hand = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True,
                              text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp), timeout=180)
        assert 'changed, restored' in hand.stdout and 'identical to' in hand.stdout, hand.stdout + hand.stderr
        assert (Path(tmp) / 'live/keep.txt').read_text(encoding='utf-8-sig').strip() == 'kept'

    run_tail("$stagesDone.Add('hit')", True, '[pscustomobject]@{ HasExited = $false; Id = 1 }', setup, run_printed)


def test_the_capture_is_wired_to_the_safety_helpers():
    text = (ROOT / 'tools/real_game/phaselock_integrity_capture.ps1').read_text(encoding='utf-8')
    for call in ('$snapshots[$d] = Assert-SnapshotMatches', '$snapshots[$d] = New-VerifiedSnapshot', 'Restore-VerifiedRoots $bak $roots $snapshots', 'Save-VerifiedBackupRecord $bak $roots $snapshots',
                 'Format-ManualRecovery $recovery $bak $HubLock $Lane', '$recovery = Get-RecoveryReason', '$holdLocks = [bool]$recovery',
                 "Assert-SavesBlocked (Py 'print(block_saves())')",
                 'if ($holdLocks) { $runLock = $false; $hub = $null }', 'Format-RestoreRows $rows', "if ($log.status -ne 'completed') { exit 1 }"):
        assert call in text, call
    assert 'Focus-Game' not in text     # Assert-GameForeground checks what Focus-Game's answer only reported


if __name__ == '__main__':
    test_helpers()
    test_a_failed_run_with_a_bad_restore_still_prints_the_rows_and_the_recovery()
    test_exit_0_only_for_a_completed_run_with_nothing_to_recover()
    test_the_printed_restore_refuses_a_backup_that_changed_after_an_unconfirmed_exit()
    test_the_printed_restore_restores_an_intact_backup()
    test_a_leftover_driver_holds_the_locks_and_says_what_to_do()
    test_a_missing_evidence_file_cannot_exit_0_and_the_restore_still_runs()
    test_a_failed_run_stops_the_hooks_while_the_game_is_alive()
    test_a_run_that_touched_nothing_has_nothing_to_recover()
    test_the_guards_refuse()
    test_the_capture_is_wired_to_the_safety_helpers()
    print('ok')
