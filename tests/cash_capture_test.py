"""Synthetic tests of tools/real_game/cash_capture.ps1's data-safety path: the real script and the real realgame.ps1 helpers run against
invented text files in a temporary folder, with only the game itself stubbed. No game, no game files, no real lock. Needs Windows PowerShell.

Run: python tests/cash_capture_test.py
"""
import json
import os
import re
from pathlib import Path
import subprocess
import realgame_test_env  # noqa: F401  (a fresh pending-record folder for every PowerShell run)
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
CAPTURE = ROOT / 'tools/real_game/cash_capture.ps1'

# Dot-sourced by the capture in place of realgame.ps1: the real helpers, then the game's side replaced. $env:OW_SCENARIO picks a fault.
STUB = r"""
. '__REALGAME__'
$script:RealGamePy = ${function:Invoke-GamePy}   # the real transport, for the reply-file faults (review of #251/#255, T1)
$work = '__WORK__'
$scenario = $env:OW_SCENARIO
$script:RG = Join-Path $work 'rg'
$script:CmdDir = Join-Path $script:RG 'cmd'
$script:WillowDir = Join-Path $work 'willow'
$script:Game = $env:OPENWILLOW_BL2
$script:DriverDir = Join-Path $script:Game 'sdk_mods/openwillow_realgame'
$script:Lock = Join-Path $work 'run.lock'
New-Item -ItemType Directory -Force $script:CmdDir | Out-Null
# The stand-in driver of the reply-file faults: when a command is armed, the first wait of the real transport answers it with a reply file
# this process creates and holds open with no sharing for the rest of the run, so every read of it fails as a sharing violation would (not
# a timeout), however slow the machine. It touches no file the transport wrote (CI, 2026-11-13: removing the command file raced a file
# scanner on the runner), and nothing in it waits on time.
# The wall clock a scenario can step back (review round 3, item 1): from the second den-down poll of 'wall_step_back' on, it reads 60 s
# earlier than it is. Only that scenario sets the flag; every other run reads the real clock.
function Get-Date {
    $d = Microsoft.PowerShell.Utility\Get-Date @args
    if ($script:wallBack -and $d -is [datetime]) { $d.AddSeconds(-60) } else { $d }
}
function Start-Sleep {
    if ($script:fakeDriver) {
        $py = @(Get-ChildItem -LiteralPath $script:CmdDir -Filter '*.py')
        if ($py.Count) {
            $script:fakeDriver = $false
            $script:heldReply = [IO.File]::Open([IO.Path]::ChangeExtension($py[0].FullName, '.out'), 'Create', 'ReadWrite', 'None')
        }
    }
}
# An arbitrary fault inside the real transport (review of #263, S1): the command folder's path holds characters no path may hold, so the
# transport's first file call fails with the file system's own text, before anything is sent.
function Read-Faulty([string]$Code, [int]$TimeoutSeconds) {
    Add-Content -LiteralPath (Join-Path $work 'events.txt') 'lost'
    $keep = $script:CmdDir; $script:CmdDir = "$work\no<such|folder"
    try { & $script:RealGamePy $Code $TimeoutSeconds } finally { $script:CmdDir = $keep }
}
function Read-Unreadable([string]$Code, [int]$TimeoutSeconds) {
    Add-Content -LiteralPath (Join-Path $work 'events.txt') 'lost'
    $script:fakeDriver = $true
    & $script:RealGamePy $Code $TimeoutSeconds
}
function Get-GameProcess { $null }
function Get-UntrackedGames($Own) { if ($scenario -eq 'foreign' -and $Own) { [pscustomobject]@{ Id = 777 } } }
function Enter-RunLock { Set-Content -LiteralPath $script:Lock 'held' }
function Exit-RunLock { Remove-Item -LiteralPath $script:Lock -Force }
function Install-Driver { New-Item -ItemType Directory -Force $script:DriverDir | Out-Null }
function Remove-Driver { if ($scenario -ne 'driver_stuck') { Remove-Item -Recurse -Force $script:DriverDir } }
function Save-Shot { 'shot' }
function Focus-Game { $true }
$script:foreground = 0
function Assert-GameForeground {
    $script:foreground++
    if ($scenario -eq 'no_focus') { throw 'refusing to send input: not the foreground window' }
    if ($scenario -eq 'focus_lost_in_den' -and $script:foreground -ge 3) { throw 'refusing to send input: not the foreground window' }
    if ($scenario -eq 'focus_lost_in_kills' -and $script:foreground -ge 3) { throw 'refusing to send input: not the foreground window' }   # the first kill's check
}
# Every key and click is recorded on disk (review of #251, A8: keys were not); events.txt holds them in order with each stand-in reply lost.
function Send-Key([string]$Key, [int]$HoldMs = 60) { Add-Content -LiteralPath (Join-Path $work 'keys.txt') $Key; Add-Content -LiteralPath (Join-Path $work 'events.txt') "key $Key" }
function Send-Click { Add-Content -LiteralPath (Join-Path $work 'clicks.txt') 'click'; Add-Content -LiteralPath (Join-Path $work 'events.txt') 'click' }
function Write-Lost { Add-Content -LiteralPath (Join-Path $work 'events.txt') 'lost'; throw 'no reply to c-invented' }
function Invoke-GamePyFile([string]$Path) { if ($scenario -eq 'probe_traceback' -and $Path -like '*den_probe*') { "Traceback (most recent call last):`n  NameError" } else { 'ok' } }
function Stop-OwnGame($Process) { Add-Content -LiteralPath (Join-Path $work 'stopped_pid.txt') $Process.Id; if ($scenario -eq 'hung') { 'refused: stub' } else { $script:Fake.HasExited = $true; 'stopped' } }
$script:collects = 0; $script:singles = 0
$script:Fake = [pscustomobject]@{ Id = 4242; StartTime = (Get-Date); HasExited = $false }
$script:Fake | Add-Member ScriptMethod WaitForExit { param($ms) $this.HasExited } | Out-Null
$script:Fake | Add-Member ScriptMethod Refresh { } | Out-Null
function Start-Game { $script:Fake }
function Invoke-GamePy([string]$Code, [int]$TimeoutSeconds = 30) {
    Add-Content -LiteralPath (Join-Path $work 'events.txt') 'cmd'   # every driver command, in order with the keys, clicks and losses
    switch -Wildcard ($Code) {
        '*cp_immune_off*' { Add-Content -LiteralPath (Join-Path $work 'sent.txt') 'cp_immune_off'; Set-Content -LiteralPath (Join-Path $work 'immune_off_at.txt') ([Diagnostics.Stopwatch]::GetTimestamp() / [Diagnostics.Stopwatch]::Frequency).ToString([Globalization.CultureInfo]::InvariantCulture); return '{"remaining": []}' }   # before '*cp_immune*', which would match it too
        # The watch's calls (slice_watch.ps1), for the runs that pass -WatchSpec.
        '*sw_start(*' { if ($scenario -eq 'watch_start_stop_lost') { return "Traceback (most recent call last):`n  RuntimeError: invented" }; $script:watchStarted = $true; return '{"label": "cash", "resolved": 1, "missing": []}' }
        '*sw_group_names_at*' { return $(if ($scenario -eq 'group_lost') { '["g1"]' } else { '[]' }) }
        '*sw_install_group(*' { if ($scenario -eq 'group_lost') { throw 'no reply to c-invented' }; return '{"group": "g1", "resolved": 1, "missing": []}' }
        '*sw_mark(*' { $script:marks++; if ($scenario -eq 'mark_lost' -and $Code -like '*replace1*') { throw 'no reply to c-invented' }; if ($scenario -eq 'kill_traceback_mark_lost' -and $Code -like '*readings.done*') { Write-Lost }; return 'ok' }
        '*sw_settled*' { if ($scenario -eq 'settle_lost') { throw 'no reply to c-invented' }; return $(if ($scenario -eq 'kills_unsettled') { 'False' } else { 'True' }) }
        '*sw_snap(*' { if ($scenario -eq 'smoke_unsettled_lost' -and $Code -like "*sw_snap('smoke_unsettled')*") { Write-Lost }; if ($scenario -eq 'smoke_unsettled_unreadable' -and $Code -like "*sw_snap('smoke_unsettled')*") { return (Read-Unreadable $Code $TimeoutSeconds) }; if ($scenario -eq 'smoke_unsettled_fault' -and $Code -like "*sw_snap('smoke_unsettled')*") { return (Read-Faulty $Code $TimeoutSeconds) }; if ($Code -like "*sw_snap('drop')*") { Add-Content -LiteralPath (Join-Path $work 'drops.txt') 'drop' }; if ($scenario -eq 'before_down_lost' -and $Code -like "*sw_snap('before_down')*") { throw 'no reply to c-invented' }; if (($scenario -eq 'snap_lost' -and $Code -like "*sw_snap('down')*") -or ($scenario -eq 'after_lost' -and $Code -like "*sw_snap('after_respawn')*") -or ($scenario -eq 'end_snap_lost' -and $Code -like "*sw_snap('end')*")) { throw 'no reply to c-invented' }; return '{"deferred": false, "not_taken": false, "failed": []}' }
        '*sw_pawns(*' { return '{"label": "x", "pawns": 0, "failed": []}' }
        '*sw_dump(*' { Add-Content -LiteralPath (Join-Path $work 'sent.txt') 'sw_dump'; return '{}' }   # every command after the end snapshot is counted on disk
        '*sw_stop*' { Add-Content -LiteralPath (Join-Path $work 'sent.txt') 'sw_stop'; if ($scenario -eq 'watch_start_stop_lost') { Write-Lost }; return $(if ($script:watchStarted) { 'stopped' } else { 'nothing to stop' }) }   # as the game answers: nothing loaded, nothing to stop
        '*block_saves*' {
            if ($scenario -eq 'saves_open') { return 'False' }
            if ($scenario -eq 'saves_traceback') { return 'Traceback: True, block_saves' }
            return '5'
        }
        '*pc.Pawn.Class.Name*' {
            # The save's load after the menu's Enter (Wait-PlayerPawn): on the menu while its online session is created, then the pawn.
            $script:loadReads++
            if ($scenario -eq 'load_late') { return $(if ($script:loadReads -le 3) { 'menumap None' } else { 'Sanctuary_P WillowPlayerPawn' }) }
            if ($scenario -eq 'never_loads') { return 'menumap None' }
            if ($scenario -eq 'load_lost') { throw 'no reply to c-invented' }
            if ($scenario -eq 'load_stuck') { return 'Loader None' }   # the menu was left, the pawn never comes
            if ($scenario -eq 'pawn_after_silence') {   # the first read answers 2 s past its wait, after the silence bound; the next at once
                if ($script:loadReads -eq 1) { [Threading.Thread]::Sleep(($TimeoutSeconds + 2) * 1000) }
                return 'Sanctuary_P WillowPlayerPawn'
            }
            if ($scenario -eq 'late_pawn') {   # the menu is left at once; the pawn's reply takes 5 s, past a 4 s load bound
                if ($script:loadReads -eq 1) { return 'Loader None' }
                if ($TimeoutSeconds -lt 5) { [Threading.Thread]::Sleep($TimeoutSeconds * 1000); throw 'no reply to c-invented' }
                [Threading.Thread]::Sleep(5000); return 'Sanctuary_P WillowPlayerPawn'
            }
            return 'Sanctuary_P WillowPlayerPawn'
        }
        '*ow_clock_before*' { return $(if ($scenario -eq 'travel_clock') { '50.0' } else { 'None' }) }
        '*pc.Pawn is not None*' {
            if ($scenario -eq 'travel_clock') { $script:polls++; return $(if ($script:polls -lt 3) { 'Ice_P True 55.0' } else { 'Ice_P True 1.5' }) }
            return $(if ($scenario -eq 'no_travel') { 'Sanctuary_P True' } else { 'Ice_P True' })
        }
        '*cp_start*' {
            # What a session does to the player's files, as seen on the real machine: a restore must undo all of this.
            Set-Content -LiteralPath (Join-Path $script:WillowDir 'SaveData/save.sav') 'changed by the run'
            Set-Content -LiteralPath (Join-Path $script:WillowDir 'SaveData/new.sav') 'added by the run'
            Remove-Item -LiteralPath (Join-Path $script:WillowDir 'Config/game.ini')
            Set-Content -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings/python-sdk.json') 'changed by the run'
            Set-Content -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver/store.txt') 'changed by the run'
            if ($scenario -eq 'backup_changed') {
                $saved = Get-ChildItem -LiteralPath $script:RG -Directory -Filter 'save-backup-*' | Select-Object -First 1
                Set-Content -LiteralPath (Join-Path $saved.FullName 'SaveData/save.sav') 'backup damaged'
            }
            if ($scenario -eq 'run_error') { throw 'channel lost' }
            if ($scenario -eq 'not_immune') { return '{"immune": false}' }
            if ($scenario -eq 'loot_mods_error') { return '{"immune": true, "mods_off": ["Loot Collector", "Drop Chance Multiplier: AttributeError(invented)"]}' }
            if ($scenario -eq 'loot_mods_ok') { return '{"immune": true, "mods_off": ["Drop Chance Multiplier", "Loot Collector", "Be Gone Out Of Bounds Loot"]}' }
            if ($scenario -eq 'traceback') { return "Traceback (most recent call last):`n  True block" }
            return '{"immune": true}'
        }
        '*cp_level*' { return '8' }
        '*cp_cash*' { return '100' }
        '*cp_ready_kill*' { if ($scenario -eq 'kill_lost') { Write-Lost }; if ($scenario -eq 'kill_unreadable') { return (Read-Unreadable $Code $TimeoutSeconds) }; if ($scenario -in 'kill_traceback', 'kill_traceback_mark_lost') { return "Traceback (most recent call last):`n  RuntimeError: invented" }; return 'WillowAIPawn_2' }
        '*cp_collect_start*' { return $(if ($scenario -in 'no_money', 'kills', 'kills_unsettled', 'settle_lost', 'focus_lost_in_kills', 'kill_traceback', 'kill_traceback_mark_lost', 'kill_lost', 'kill_unreadable') { 'None' } else { 'WillowPickup_1' }) }
        '*cp_collect_end*' {
            $script:collects++
            if ($scenario -eq 'never_single') { return '{"value_per_item": null}' }
            $script:singles++
            return '{"value_per_item": 3}'
        }
        '*cp_up()*' { return $(if ($scenario -eq 'downed') { 'False' } else { 'True' }) }
        '*cp_immune*' { return $(if ($scenario -eq 'immunity_lost') { 'False' } else { 'True' }) }
        '*cp_target*' { return $(if ($scenario -in 'kills', 'kills_unsettled', 'settle_lost', 'focus_lost_in_kills', 'kill_traceback', 'kill_traceback_mark_lost', 'kill_lost', 'kill_unreadable') { 'WillowAIPawn_2' } else { 'None' }) }
        '*cp_died*' { Add-Content -LiteralPath (Join-Path $work 'sent.txt') 'cp_died'; $script:attempts++; return $(if ($script:attempts % 2) { 'True' } else { 'False' }) }   # every other attempt kills
        '*cp_dump*' {
            if ($scenario -eq 'game_gone') { $script:Fake.HasExited = $true }   # the game is gone right after the readings
            $real = @(for ($i = 0; $i -lt $script:collects; $i++) { @{ value_per_item = $(if ($i -lt $script:singles) { 3 } else { $null }) } })
            $written = if ($scenario -eq 'dump_short') { @() } else { $real }
            if ($scenario -ne 'dump_missing') {
                [IO.File]::WriteAllText((Join-Path $script:RG 'cash/cash.json'), (ConvertTo-Json -Depth 4 @{ kills = 0; real = @($written) }))
            }
            return (ConvertTo-Json @{ kills = 0; collected = $script:collects; single_item_rows = $script:singles })
        }
        '*dp_start*' { Add-Content -LiteralPath (Join-Path $work 'den.txt') 'dp_start'; if ($scenario -eq 'den_channel') { throw 'channel lost in the den read' }; return '[]' }
        '*dp_begin*' { return $(if ($scenario -eq 'den_no_enemy') { 'None' } else { 'WillowAIPawn_1' }) }
        '*dp_replace*' {
            # The den down's re-placement; every call is counted on disk, so a test sees one that was sent when it should not have been.
            Add-Content -LiteralPath (Join-Path $work 'replace_calls.txt') 'sent'
            $now = ([Diagnostics.Stopwatch]::GetTimestamp() / [Diagnostics.Stopwatch]::Frequency).ToString([Globalization.CultureInfo]::InvariantCulture)   # the command clock
            Add-Content -LiteralPath (Join-Path $work 'replace_waits.txt') ("$TimeoutSeconds " + $(if ($Code -match 'until=([\d.]+)') { $Matches[1] } else { 'no-until' }) + " $now")
            if ($scenario -eq 'down_at_replace') { $script:downNow = $true; return '{"result": "not_up", "state": "injured_flag"}' }   # down since the poll
            return '{"result": "placed", "enemy": "WillowAIPawn_1", "dist": 900.0}'
        }
        '*cp_down_state*' {
            # The den down's poll (invented values): up with a steady pool unless the scenario says otherwise.
            $script:polls++
            if ($scenario -eq 'wall_step_back') {
                Add-Content -LiteralPath (Join-Path $work 'poll_clock.txt') (Get-QpcSeconds).ToString([Globalization.CultureInfo]::InvariantCulture)
                if ($script:polls -eq 2) { $script:wallBack = $true }
            }
            if ($script:downNow -or ($scenario -in @('snap_lost', 'after_lost', 'close_lost') -and $script:polls -eq 1) -or ($scenario -eq 'replace_after_down' -and $script:polls -ge 2)) { return '{"state": "injured_flag", "injured_field": true, "pool": 50.0}' }
            if ($scenario -eq 'replace_hit') { return ('{"state": "up", "injured_field": true, "health": ' + (1000 - $script:polls) + ', "pool": ' + (1000 - $script:polls) + '}') }   # a hit every poll
            if ($scenario -eq 'shield_only') { return ('{"state": "up", "injured_field": true, "health": 100.0, "pool": ' + (1000 - $script:polls) + '}') }   # only the shield falls
            if ($scenario -eq 'poll_lost' -and $script:polls -eq 2) { throw 'no reply to c-invented' }   # one poll unanswered, the next ones answered
            if ($scenario -eq 'short_timeout' -and $TimeoutSeconds -lt 30) { throw 'no reply to c-invented' }   # a shortened wait runs out
            if ($scenario -eq 'late_down') { [Threading.Thread]::Sleep(($TimeoutSeconds + 2) * 1000); return '{"state": "injured_flag", "injured_field": true, "pool": 0.0}' }   # answers after its window
            if ($scenario -eq 'slow_poll') {   # each reply takes 2.5 s: a wait shorter than that gets no reply
                if ($TimeoutSeconds -lt 3) { [Threading.Thread]::Sleep($TimeoutSeconds * 1000); throw 'no reply to c-invented' }
                [Threading.Thread]::Sleep(2500); return '{"state": "up", "injured_field": true, "health": 100.0, "pool": 100.0}'
            }
            if ($scenario -eq 'health_then_null') { return $(if ($script:polls -eq 1) { '{"state": "up", "injured_field": true, "health": 100.0, "pool": 100.0}' } else { '{"state": "up", "injured_field": true, "health": null, "pool": null}' }) }
            if ($scenario -eq 'pool_none') { return '{"state": "up", "injured_field": true, "health": null, "pool": null}' }   # the health cannot be read
            return '{"state": "up", "injured_field": true, "health": 100.0, "pool": 100.0}'
        }
        '*dp_end*' {
            $script:ends++
            if ($scenario -eq 'close_lost' -and $script:ends -gt 3) { throw 'no reply to c-invented' }   # the 3 den trials' closes answer; the den down's does not
            return '{"counts": true}'
        }
        '*dp_dump*' {
            $trials = if ($scenario -eq 'den_no_enemy') { 0 } else { 3 }
            if ($scenario -ne 'den_dump_missing') {
                [IO.File]::WriteAllText((Join-Path $script:RG 'cash/den.json'), (ConvertTo-Json @{ trials = @(1..3 | Select-Object -First $trials) }))
            }
            # As the real one: an object, not a list. A list would reach ConvertFrom-Json as one item in Windows PowerShell.
            $missing = if ($scenario -eq 'den_unanswered') { '["first-blow spread: no counted trial had blows from two attackers"]' } else { '[]' }
            return '{"summaries": [{"counts": true}], "missing": ' + $missing + '}'
        }
        '*quit*' { Add-Content -LiteralPath (Join-Path $work 'sent.txt') 'quit'; if ($scenario -eq 'quit_lost') { Write-Lost }; if ($scenario -ne 'hung') { $script:Fake.HasExited = $true }; return '' }
        default { return 'None' }
    }
}
if ($scenario -eq 'bad_snapshot') {
    function Assert-SnapshotMatches([string]$Live, [string]$Snapshot) { if ($Snapshot -like '*Config') { throw 'snapshot differs' }; Get-TreeHashes $Snapshot }
}
# The -SdkModsOff faults (lane capture-prep): the driver install fails, the launch fails after mods off, or the settings write stops half way.
if ($scenario -eq 'driver_fails') { function Install-Driver { throw 'driver install failed (test)' } }
if ($scenario -eq 'launch_fails') { function Start-Game { throw 'launch failed (test)' } }
if ($scenario -eq 'partial_mods') {
    function Set-SdkModsOff {
        Add-PendingChange 'SDK mods off (test: stops half way)'
        $first = Get-ChildItem -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings') -Filter *.json | Sort-Object Name | Select-Object -First 1
        [IO.File]::WriteAllText($first.FullName, '{"enabled": false}')
        throw 'stopped half way (test)'
    }
}
if ($scenario -eq 'mismatch') { function Restore-Tree { [pscustomobject]@{ file = 'x'; status = 'MISMATCH after restore' } } }
if ($scenario -eq 'restore_throws') { function Restore-Tree { throw 'disk full' } }
"""


class CashCaptureTest(unittest.TestCase):
    def setUp(self):
        # Removed with bounded retries for a file still held (review of #263, S4); a removal that keeps failing is reported as an error.
        self.work = Path(tempfile.mkdtemp())
        self.addCleanup(realgame_test_env.remove_tree, self.work)
        self.game = self.work / 'game'
        for name, text in (('sdk_mods/settings/python-sdk.json', 'original settings'), ('sdk_mods/SanitySaver/store.txt', 'original store')):
            path = self.game / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding='utf-8')
        for name, text in (('SaveData/save.sav', 'original save'), ('Config/game.ini', 'original config')):
            path = self.work / 'willow' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding='utf-8')
        self.hub = self.work / 'hub.lock'

    def run_capture(self, scenario, extra=()):
        stub = self.work / 'stub.ps1'
        stub.write_text(STUB.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__WORK__', str(self.work)),
                        encoding='utf-8-sig')
        env = dict(os.environ, OPENWILLOW_BL2=str(self.game), OW_SCENARIO=scenario)
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(CAPTURE),
                               '-HubLock', str(self.hub), '-Helpers', str(stub), *extra],
                              capture_output=True, text=True, env=env, timeout=180)
        session = json.loads((self.work / 'rg/cash/session.json').read_text(encoding='utf-8-sig'))
        return done, session

    def restore_by_hand(self):
        """Runs the printed recovery command in a new PowerShell, with the real helpers, as the maintainer would."""
        script = self.work / 'by_hand.ps1'
        script.write_text(f". '{ROOT / 'tools/real_game/realgame.ps1'}'\n$script:WillowDir = [IO.Path]::GetTempPath()   # test mode: by-hand restores run only over synthetic folders\nRestore-VerifiedBackup '{self.backups()[0]}'\n", encoding='utf-8-sig')
        env = dict(os.environ, OPENWILLOW_BL2=str(self.game))
        return subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)],
                              capture_output=True, text=True, env=env, timeout=120)

    def saves(self):
        return (self.work / 'willow/SaveData/save.sav').read_text(encoding='utf-8').strip()

    def sdk_files(self):
        return ((self.game / 'sdk_mods/settings/python-sdk.json').read_text(encoding='utf-8').strip(),
                (self.game / 'sdk_mods/SanitySaver/store.txt').read_text(encoding='utf-8').strip())

    def locks_held(self):
        return self.hub.exists(), (self.work / 'run.lock').exists()

    def backups(self):
        return list((self.work / 'rg').glob('save-backup-*'))

    def test_clean_run_restores_everything_and_releases(self):
        done, session = self.run_capture('ok')
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual(session['status'], 'completed')
        self.assertTrue(session['cleanup']['saves'].startswith('identical'), session['cleanup'])
        self.assertEqual(self.saves(), 'original save')
        self.assertFalse((self.work / 'willow/SaveData/new.sav').exists())
        self.assertTrue((self.work / 'willow/Config/game.ini').exists())
        self.assertEqual(self.sdk_files(), ('original settings', 'original store'))   # the SDK's files end as they began
        self.assertFalse((self.game / 'sdk_mods/openwillow_realgame').exists())
        self.assertEqual(self.locks_held(), (False, False))

    def test_a_clean_run_names_every_stage_it_ran(self):
        _, session = self.run_capture('ok')
        self.assertEqual(session['stage_results'], {'travel': 'passed', 'readings': 'passed', 'den_read': 'passed'})
        self.assertEqual(session['completed_stages'], ['travel', 'readings', 'den_read'])

    def test_a_failed_den_read_keeps_the_cash_result_and_is_not_completed(self):
        for scenario, reason in (('den_channel', 'channel lost'), ('den_no_enemy', '0 of 0 trials')):
            with self.subTest(scenario):
                if self.backups():
                    self.setUp()
                done, session = self.run_capture(scenario)
                self.assertEqual(session['status'], 'incomplete: den_read')
                self.assertEqual(done.returncode, 1)
                self.assertEqual(session['stage_results']['readings'], 'passed')
                self.assertIn(reason, session['stage_results']['den_read'])
                self.assertEqual(self.saves(), 'original save')
                self.assertEqual(self.locks_held(), (False, False))

    def test_without_the_immunity_hook_nothing_is_read(self):
        for scenario in ('not_immune', 'traceback'):
            with self.subTest(scenario):
                if self.backups():
                    self.setUp()
                done, session = self.run_capture(scenario)
                self.assertTrue(session['status'].startswith(('failed: the immunity hook is not installed', 'failed: game python error')), session['status'])
                self.assertEqual(session['stage_results']['readings'], 'not run')
                self.assertEqual(done.returncode, 1)
                self.assertEqual(self.saves(), 'original save')

    def test_immunity_lost_before_a_den_trial_fails_only_that_stage(self):
        done, session = self.run_capture('immunity_lost')
        self.assertEqual(session['status'], 'incomplete: den_read')
        self.assertIn('immunity hook is gone', session['stage_results']['den_read'])
        self.assertEqual(session['stage_results']['readings'], 'passed')

    def test_a_dump_that_does_not_match_fails_the_stage(self):
        for scenario, stage, words in (('dump_missing', 'readings', 'cash.json was not written'), ('dump_short', 'readings', 'cash.json holds 0'),
                                       ('den_dump_missing', 'den_read', 'den.json was not written')):
            with self.subTest(scenario):
                if self.backups():
                    self.setUp()
                done, session = self.run_capture(scenario)
                self.assertEqual(session['status'], 'incomplete: ' + stage)
                self.assertIn(words, session['stage_results'][stage])
                self.assertEqual(done.returncode, 1)

    def test_zero_wanted_items_is_refused_before_anything_is_touched(self):
        stub = self.work / 'stub.ps1'
        stub.write_text(STUB.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__WORK__', str(self.work)), encoding='utf-8-sig')
        env = dict(os.environ, OPENWILLOW_BL2=str(self.game), OW_SCENARIO='ok')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(CAPTURE), '-HubLock', str(self.hub),
                               '-Helpers', str(stub), '-WantItems', '0'], capture_output=True, text=True, env=env, timeout=180)
        self.assertNotEqual(done.returncode, 0)
        self.assertFalse((self.work / 'rg/cash/session.json').exists())
        self.assertEqual(self.backups(), [])
        self.assertFalse(self.hub.exists())

    def test_too_few_readings_do_not_hide_a_good_den_read(self):
        _, session = self.run_capture('never_single')
        self.assertEqual(session['status'], 'incomplete: readings')
        self.assertEqual(session['stage_results']['den_read'], 'passed')

    def test_too_few_readings_is_incomplete_not_finished(self):
        for scenario in ('no_money', 'never_single'):
            with self.subTest(scenario):
                if self.backups():
                    self.setUp()
                done, session = self.run_capture(scenario)
                self.assertEqual(session['status'], 'incomplete: readings')
                self.assertEqual(done.returncode, 1)
                self.assertEqual(self.saves(), 'original save')
                self.assertEqual(self.locks_held(), (False, False))

    def test_a_pickup_that_cannot_be_collected_stops_the_loop(self):
        done, session = self.run_capture('never_single')
        self.assertIn('6 distinct pickups in a row', session['stopped'])
        self.assertEqual(session['status'], 'incomplete: readings')

    def test_a_failed_travel_is_not_completed(self):
        done, session = self.run_capture('no_travel')
        self.assertTrue(session['status'].startswith('failed'), session['status'])
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.locks_held(), (False, False))

    def test_trials_that_ran_are_not_enough_for_the_den_stage(self):
        done, session = self.run_capture('den_unanswered')
        self.assertEqual(session['status'], 'incomplete: den_read')
        self.assertIn('first-blow spread', session['stage_results']['den_read'])
        self.assertEqual(session['stage_results']['readings'], 'passed')
        self.assertEqual(done.returncode, 1)

    def test_a_probe_that_did_not_load_fails_its_stage(self):
        done, session = self.run_capture('probe_traceback')
        self.assertEqual(session['status'], 'incomplete: den_read')
        self.assertIn('den_probe.py did not load', session['stage_results']['den_read'])
        self.assertEqual(session['stage_results']['readings'], 'passed')

    def test_readings_with_maya_down_are_not_readings(self):
        done, session = self.run_capture('downed')
        self.assertTrue(session['status'].startswith('incomplete: readings'), session['status'])
        self.assertIn('Maya is down', session['stage_results']['readings'])

    def test_a_game_this_run_did_not_start_touches_nothing(self):
        done, session = self.run_capture('foreign')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertIn('did not start', session['status'])
        self.assertIn('NOT RESTORED', session['cleanup']['saves'])
        self.assertEqual(self.saves(), 'changed by the run')
        self.assertTrue((self.game / 'sdk_mods/openwillow_realgame').exists())
        self.assertEqual(self.locks_held(), (True, True))
        self.assertEqual(done.returncode, 1)

    def test_earlier_evidence_is_kept_under_a_new_name_and_never_read_as_this_run(self):
        out = self.work / 'rg/cash'
        out.mkdir(parents=True)
        for name in ('session', 'save_compare', 'cash', 'den'):
            (out / (name + '.json')).write_text('{"status": "completed", "kills": 0, "real": [], "trials": [1, 2, 3]}', encoding='utf-8')
        done, session = self.run_capture('dump_missing')
        self.assertEqual(session['status'], 'incomplete: readings')   # the earlier cash.json was not taken for this run's
        self.assertIn('cash.json was not written', session['stage_results']['readings'])
        for name in ('session', 'save_compare', 'cash', 'den'):
            kept = list(out.glob(name + '.*.json'))
            self.assertEqual(len(kept), 1, name)
            self.assertIn('completed', kept[0].read_text(encoding='utf-8'))
        self.assertEqual(done.returncode, 1)

    def test_without_game_focus_the_run_stops_with_the_reason(self):
        done, session = self.run_capture('no_focus')
        self.assertIn('refusing to send input', session['status'])
        self.assertEqual(session['stage_results']['travel'], 'not run')
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.saves(), 'original save')
        self.assertEqual(self.locks_held(), (False, False))

    def test_focus_lost_in_the_den_read_fails_only_that_stage(self):
        done, session = self.run_capture('focus_lost_in_den')
        self.assertEqual(session['status'], 'incomplete: den_read')
        self.assertIn('refusing to send input', session['stage_results']['den_read'])
        self.assertEqual(session['stage_results']['readings'], 'passed')
        self.assertEqual(done.returncode, 1)

    def test_the_capture_never_ignores_a_focus_result(self):
        text = CAPTURE.read_text(encoding='utf-8')
        self.assertNotIn('Focus-Game', text)
        self.assertEqual(text.count('Assert-GameForeground'), 4)

    def test_a_traceback_is_not_saves_blocked(self):
        done, session = self.run_capture('saves_traceback')
        self.assertTrue(session['status'].startswith('failed: saves are not blocked'), session['status'])
        self.assertEqual(session['stage_results']['travel'], 'not run')
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.saves(), 'original save')

    def test_saves_not_blocked_stops_the_run(self):
        done, session = self.run_capture('saves_open')
        self.assertIn('saves are not blocked', session['status'])
        self.assertEqual(done.returncode, 1)

    def test_a_run_error_is_failed_and_still_restores(self):
        done, session = self.run_capture('run_error')
        self.assertEqual(session['status'], 'failed: channel lost')
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.saves(), 'original save')
        self.assertEqual(self.sdk_files(), ('original settings', 'original store'))
        self.assertEqual(self.locks_held(), (False, False))

    def test_an_unverified_snapshot_is_never_restored_from(self):
        done, session = self.run_capture('bad_snapshot')
        self.assertTrue(session['status'].startswith('failed'), session['status'])
        # Backup-Saves proves each copy itself now, so the run stops inside it: no backup, no pending record, nothing touched.
        self.assertIn('snapshot differs', session['status'])
        self.assertNotIn('saves', session['cleanup'])
        self.assertEqual(session['cleanup']['pending'], 'none written')
        self.assertEqual(self.locks_held(), (False, False))
        self.assertTrue((self.work / 'willow/Config/game.ini').exists())

    def test_a_restore_mismatch_keeps_locks_and_backup_and_prints_recovery(self):
        done, session = self.run_capture('mismatch')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertEqual(session['run_status'], 'completed')
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.locks_held(), (True, True))
        self.assertTrue(self.backups())
        self.assertIn('MANUAL RECOVERY', done.stdout)
        self.assertIn("Restore-PendingRecord '", done.stdout); self.assertNotIn('Restore-VerifiedBackup', done.stdout)   # with a record, its one way back
        self.assertNotIn('Restore-Tree', done.stdout)
        self.assertIn('MISMATCH after restore', done.stdout)
        self.assertEqual(self.saves(), 'changed by the run')
        hand = self.restore_by_hand()
        self.assertIn('identical', hand.stdout, hand.stdout + hand.stderr)
        self.assertEqual(self.saves(), 'original save')
        self.assertEqual(self.sdk_files(), ('original settings', 'original store'))

    def test_a_restore_that_throws_keeps_locks_and_prints_recovery(self):
        done, session = self.run_capture('restore_throws')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertEqual(self.locks_held(), (True, True))
        self.assertTrue(self.backups())
        self.assertIn('MANUAL RECOVERY', done.stdout)
        self.assertEqual(done.returncode, 1)

    def test_a_backup_that_changed_is_not_restored_from(self):
        done, session = self.run_capture('backup_changed')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertEqual(self.saves(), 'changed by the run')
        self.assertTrue((self.work / 'willow/Config/game.ini').exists())   # the other roots are still restored
        self.assertEqual(self.locks_held(), (True, True))
        self.assertIn('BACKUP CHANGED', done.stdout)
        hand = self.restore_by_hand()   # the command itself refuses the changed root, by hand as well
        self.assertIn('BACKUP CHANGED', hand.stdout, hand.stdout + hand.stderr)
        self.assertEqual(self.saves(), 'changed by the run')
        self.assertEqual(done.returncode, 1)

    def test_a_driver_that_stays_is_reported_as_recovery(self):
        done, session = self.run_capture('driver_stuck')
        self.assertTrue(session['status'].startswith('recovery needed: driver not removed'), session['status'])
        self.assertEqual(session['run_status'], 'completed')
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.saves(), 'original save')

    def test_a_game_that_will_not_exit_touches_nothing(self):
        done, session = self.run_capture('hung')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertEqual(self.saves(), 'changed by the run')
        self.assertEqual(self.sdk_files(), ('changed by the run', 'changed by the run'))
        self.assertTrue((self.game / 'sdk_mods/openwillow_realgame').exists())
        self.assertIn("Restore-PendingRecord '", done.stdout); self.assertNotIn('Restore-VerifiedBackup', done.stdout)   # with a record, its one way back
        self.assertTrue((self.backups()[0] / 'verified_hashes.json').exists())   # written before the game started
        self.assertEqual(self.locks_held(), (True, True))
        self.assertIn('MANUAL RECOVERY', done.stdout)
        self.assertEqual(done.returncode, 1)

    def test_a_held_hub_lock_stops_before_any_backup(self):
        self.hub.write_text('someone 2026', encoding='utf-8')
        done, session = self.run_capture('ok')
        self.assertTrue(session['status'].startswith('failed'), session['status'])
        self.assertIn('not waiting', session['status'])
        self.assertEqual(self.backups(), [])
        self.assertEqual(self.hub.read_text(encoding='utf-8'), 'someone 2026')
        self.assertFalse((self.work / 'run.lock').exists())




class SdkModsOffRollbackTest(unittest.TestCase):
    """-SdkModsOff (lane capture-prep, 2026-11-11): whatever stops the run after the backup, the settings end byte-identical to before.
    The mods-off step runs after $launchAttempted is set, so the unchanged finalizer restores every root; a failure before it changes nothing."""
    # The harness of CashCaptureTest, without inheriting its tests.
    run_capture = CashCaptureTest.run_capture
    locks_held = CashCaptureTest.locks_held

    def setUp(self):
        CashCaptureTest.setUp(self)
        self.settings = self.game / 'sdk_mods/settings'
        (self.settings / 'python-sdk.json').write_text('{"enabled": true}', encoding='utf-8')
        (self.settings / 'other.json').write_text('{"keybinds": {}, "enabled": true}', encoding='utf-8')
        self.before = {f.name: f.read_bytes() for f in self.settings.iterdir()}

    def assert_settings_identical(self, session):
        after = {f.name: f.read_bytes() for f in self.settings.iterdir()}
        self.assertEqual(after, self.before, session.get('cleanup'))
        self.assertTrue(str(session['cleanup']['pending']).startswith('removed'), session['cleanup'])
        self.assertEqual(self.locks_held(), (False, False))

    def test_a_failed_driver_install_leaves_the_settings_untouched(self):
        done, session = self.run_capture('driver_fails', ['-SdkModsOff'])
        self.assertEqual(done.returncode, 1)
        self.assertNotIn('sdk_mods_off', session)   # the step never ran: it comes after the driver
        self.assert_settings_identical(session)

    def test_a_failed_launch_after_mods_off_restores_the_settings(self):
        done, session = self.run_capture('launch_fails', ['-SdkModsOff'])
        self.assertEqual(done.returncode, 1)
        self.assertEqual(sorted(session['sdk_mods_off']), ['other', 'python-sdk'])   # they were really turned off
        self.assertTrue(session['cleanup']['saves'].startswith('identical'), session['cleanup'])
        self.assert_settings_identical(session)

    def test_a_settings_write_stopped_half_way_is_restored(self):
        done, session = self.run_capture('partial_mods', ['-SdkModsOff'])
        self.assertEqual(done.returncode, 1)
        self.assertIn('stopped half way', session['status'])
        self.assert_settings_identical(session)

    def test_a_loot_mod_left_on_in_memory_stops_the_run_before_any_kill(self):
        done, session = self.run_capture('loot_mods_error')
        self.assertEqual(done.returncode, 1)
        self.assertIn('was not switched off in memory', session['status'])
        self.assertIn('loot mods off in memory: Loot Collector; Drop Chance Multiplier: AttributeError', done.stdout)
        self.assertEqual(session.get('kills', 0), 0)
        self.assert_settings_identical(session)

    def test_the_loot_mods_list_is_printed_and_kept(self):
        done, session = self.run_capture('loot_mods_ok', ['-SkipDen'])
        self.assertEqual(session['loot_mods_off'], ['Drop Chance Multiplier', 'Loot Collector', 'Be Gone Out Of Bounds Loot'])
        self.assertIn('loot mods off in memory: Drop Chance Multiplier; Loot Collector; Be Gone Out Of Bounds Loot', done.stdout)

    def test_an_unexpected_set_of_mods_is_refused_before_any_write(self):
        done, session = self.run_capture('launch_fails', ['-SdkModsOff', '-SdkModsExpected', 'python-sdk'])
        self.assertEqual(done.returncode, 1)
        self.assertIn('refusing SDK mods off', session['status'])
        self.assertNotIn('sdk_mods_off', session)
        self.assert_settings_identical(session)


class StageGateTest(unittest.TestCase):
    """The travel waits for the map clock to restart; once the game is gone no later stage is attempted; every stage is stamped."""
    run_capture = CashCaptureTest.run_capture
    locks_held = CashCaptureTest.locks_held

    def setUp(self):
        CashCaptureTest.setUp(self)

    def test_the_travel_waits_for_the_new_map(self):
        done, session = self.run_capture('travel_clock', ['-SkipDen'])
        self.assertTrue(session['map'].endswith(' 1.5'), session['map'])   # the replies of the old map (55.0 > 50.0) were not taken

    def test_once_the_game_is_gone_the_remaining_stages_are_skipped(self):
        done, session = self.run_capture('game_gone', ['-DenDown'])
        self.assertIn('skipped: the game has exited', session['stage_results']['den_read'])
        self.assertIn('skipped: the game has exited', session['stage_results']['den_down'])
        marks = [(m['stage'], m['event']) for m in session['stage_marks']]
        self.assertEqual(marks, [('readings', 'enter'), ('readings', 'done'), ('den_read', 'enter'), ('den_read', 'done'),
                                 ('den_down', 'enter'), ('den_down', 'done')])
        self.assertTrue(all(m['utc'] for m in session['stage_marks']))

    def test_the_smoke_switch_needs_a_watch(self):
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(CAPTURE), '-WatchSmoke'],
                              capture_output=True, text=True, timeout=120, env=dict(os.environ, OPENWILLOW_BL2=str(self.game)))
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('-WatchSmoke needs -WatchSpec', done.stdout + done.stderr)


class WatchStopPolicyTest(unittest.TestCase):
    """-WatchSmoke with the game gone at the start of stage 2, and a lost reply during a stage-1 group install (review of c001181)."""
    run_capture = CashCaptureTest.run_capture
    locks_held = CashCaptureTest.locks_held

    def setUp(self):
        CashCaptureTest.setUp(self)
        self.spec = self.work / 'spec.json'
        self.spec.write_text(json.dumps({'funcs': ['Pawn.Died'], 'groups': [{'name': 'g1', 'at': 'stage1', 'funcs': ['Pawn.Hit']}]}), encoding='utf-8')

    def test_the_game_gone_at_stage_2_fails_the_hold_and_keeps_stage_3_and_its_stamps(self):
        done, session = self.run_capture('game_gone', ['-WatchSpec', str(self.spec), '-WatchSmoke', '-DenDown'])
        results = session['stage_results']
        self.assertIn('failed', results['smoke_hold'])   # a failed hold, not "not run" from a Set-Stage that threw
        self.assertIn('skipped: the game has exited', results['den_read'])
        self.assertIn('skipped: the game has exited', results['den_down'])
        marks = [(m['stage'], m['event']) for m in session['stage_marks']]
        self.assertIn(('den_down', 'enter'), marks)
        self.assertIn(('den_down', 'done'), marks)
        stamps = Path(session['stages_file']).read_text(encoding='utf-8')   # on disk, not only in session.json
        self.assertIn('den_down done', stamps)
        self.assertIn('watch start', stamps)   # the stamp before the watch starts

    def test_the_watch_hooks_are_stopped_in_the_cleanup_while_the_game_lives(self):
        done, session = self.run_capture('hung', ['-WatchSpec', str(self.spec)])   # the game does not quit
        self.assertEqual(session['cleanup']['watch'], 'stopped')
        self.assertIn('hooks', session['cleanup'])   # the den stop's slot is unchanged beside it

    def test_a_lost_reply_in_a_group_install_skips_every_later_stage_and_the_check(self):
        done, session = self.run_capture('group_lost', ['-WatchSpec', str(self.spec), '-WatchSmoke', '-DenDown'])
        results = session['stage_results']
        self.assertIn('the driver stopped answering', results['den_read'])
        self.assertIn('the driver stopped answering', results['den_down'])
        self.assertIn('the driver stopped answering', results['watch'])
        # No quit is sent to a lost driver (review of #245, round 2): the game is still there at the cleanup, is sent nothing, and is
        # stopped by its PID. The first lost reply is the reason everywhere.
        self.assertEqual(session['cleanup']['watch'], 'not sent: the driver stopped answering (no reply to c-invented)')
        self.assertEqual(session['cleanup']['game'], 'stopped')


class DenDownReplacementTest(unittest.TestCase):
    """The den down's re-placement (S0 of 2026-11-12): up and unhit for -ReplaceAfterSeconds puts Maya at the pack again with the stage's own
    placement, at most -MaxReplacements times, never after a down or once the driver is lost; each one stamped and counted. Invented values;
    the stub's Start-Sleep returns at once, so the poll loop runs on real time for -DownSeconds."""
    run_capture = CashCaptureTest.run_capture
    locks_held = CashCaptureTest.locks_held

    def setUp(self):
        CashCaptureTest.setUp(self)

    def run_down(self, scenario, *extra):
        return self.run_capture(scenario, ['-DenDown', '-RespawnSeconds', '8', *extra])

    def test_unhit_for_the_wait_means_one_replacement(self):
        done, session = self.run_down('replace_one', '-DownSeconds', '9', '-ReplaceAfterSeconds', '2')
        self.assertEqual(session['den_down_replacements'], 1)
        self.assertEqual(len(session['den_down_replace_results']), 1)
        self.assertIn(('den_down', 'replace1'), [(m['stage'], m['event']) for m in session['stage_marks']])
        self.assertIn('den_down replace1', Path(session['stages_file']).read_text(encoding='utf-8'))   # stamped on disk
        self.assertIn('failed', session['stage_results']['den_down'])   # still no down: the stage fails as before

    def test_the_cap_is_four(self):
        done, session = self.run_down('replace_cap', '-DownSeconds', '8', '-ReplaceAfterSeconds', '0')
        self.assertEqual(session['den_down_replacements'], 4)
        self.assertEqual(sum(1 for m in session['stage_marks'] if m['event'].startswith('replace')), 4)

    def test_a_wall_clock_stepped_back_mid_window_does_not_lengthen_it(self):
        # Review round 3, item 1: each re-placement put its end on the performance counter from the WALL-clock time left, and the loop, the
        # polls and the pauses counted wall-clock time, so a clock stepped back mid-window lengthened it. The window's end is now taken
        # once on the command clock. The stub's wall clock reads 60 s earlier from the second poll on; the polls must still stop at the
        # window's 8 s (none starts in its last 6 s), measured on the performance counter.
        done, session = self.run_down('wall_step_back', '-DownSeconds', '8', '-ReplaceAfterSeconds', '999')
        clocks = [float(v) for v in (self.work / 'poll_clock.txt').read_text(encoding='utf-8').split()]
        self.assertGreater(len(clocks), 2, done.stdout)   # the step happened inside the window
        self.assertLess(clocks[-1] - clocks[0], 8.0, done.stdout)   # 60 s more on the old code
        self.assertIn('no down in 8 s', session['stage_results']['den_down'])

    def test_none_after_a_down(self):
        done, session = self.run_down('replace_after_down', '-DownSeconds', '9', '-ReplaceAfterSeconds', '0')
        self.assertEqual(session['den_down_reason'], 'injured_flag')
        self.assertEqual(session['den_down_replacements'], 1)   # the one before the down; none after it

    def test_none_while_hits_keep_coming(self):
        done, session = self.run_down('replace_hit', '-DownSeconds', '8', '-ReplaceAfterSeconds', '1')
        self.assertEqual(session['den_down_replacements'], 0)

    def replace_calls(self):
        f = self.work / 'replace_calls.txt'
        return len(f.read_text(encoding='utf-8').split()) if f.exists() else 0

    def test_none_once_the_driver_is_lost(self):
        # Poll 2 gets no reply and the polls after it would answer: without the rethrow the loop would go on placing her.
        done, session = self.run_down('poll_lost', '-DownSeconds', '40', '-ReplaceAfterSeconds', '0')
        self.assertEqual(session['den_down_replacements'], 1)   # the one after poll 1; none after the lost reply
        self.assertEqual(self.replace_calls(), 1)
        self.assertIn('no reply', session['stage_results']['den_down'])
        self.assertNotIn('den_down_poll_errors', session)   # the stage ended at the lost reply, before the respawn polls and dp_end
        self.assertEqual(sum(1 for m in session['stage_marks'] if m['event'].startswith('replace')), 1)

    def test_a_lost_driver_found_by_the_stamp_sends_no_placement(self):
        spec = self.work / 'spec.json'
        spec.write_text(json.dumps({'funcs': ['Pawn.Died']}), encoding='utf-8')
        done, session = self.run_down('mark_lost', '-WatchSpec', str(spec), '-DownSeconds', '9', '-ReplaceAfterSeconds', '0')
        self.assertEqual(self.replace_calls(), 0)   # the stamp's watch mark got no reply: dp_replace is not sent
        self.assertEqual(session['den_down_replacements'], 0)
        self.assertIn(('den_down', 'replace1'), [(m['stage'], m['event']) for m in session['stage_marks']])   # the stamp itself is on disk
        self.assertIn('the driver stopped answering during the den down', session['stage_results']['den_down'])
        self.assertIn('the driver stopped answering', session['stage_results']['watch'])

    def test_a_lost_driver_at_the_down_snapshot_sends_no_respawn_poll_and_no_trial_close(self):
        spec = self.work / 'spec.json'
        spec.write_text(json.dumps({'funcs': ['Pawn.Died']}), encoding='utf-8')
        done, session = self.run_down('snap_lost', '-WatchSpec', str(spec), '-DownSeconds', '9')
        self.assertEqual(session['den_down_reason'], 'injured_flag')
        self.assertIsNone(session['den_down_up_after_s'])   # a respawn poll would have answered 'up': none was sent
        self.assertTrue(session['den_down_trial_close'].startswith('not sent: the driver stopped answering'))
        self.assertIn('the driver stopped answering during the den down', session['stage_results']['den_down'])

    def test_a_lost_driver_at_after_respawn_fails_the_stage_although_she_stood_again(self):
        spec = self.work / 'spec.json'
        spec.write_text(json.dumps({'funcs': ['Pawn.Died']}), encoding='utf-8')
        done, session = self.run_down('after_lost', '-WatchSpec', str(spec), '-DownSeconds', '9')
        self.assertIsNotNone(session['den_down_up_after_s'])   # down and up were both seen
        self.assertNotIn('den_down', session['completed_stages'])
        self.assertIn('the driver stopped answering during the den down', session['stage_results']['den_down'])
        self.assertTrue(session['den_down_trial_close'].startswith('not sent'))
        self.assertEqual(session['den_down_health_back_s'], session['den_down_up_after_s'])   # named for what it is: health back, not standing

    def sent(self):
        f = self.work / 'sent.txt'
        return f.read_text(encoding='utf-8').split() if f.exists() else []

    def test_a_lost_reply_at_the_end_snapshot_sends_no_dump_and_no_stop(self):
        # Review of #245: the end snapshot's timeout latched the flag, and the dump and the stop were sent anyway.
        spec = self.work / 'spec.json'
        spec.write_text(json.dumps({'funcs': ['Pawn.Died']}), encoding='utf-8')
        done, session = self.run_capture('end_snap_lost', ['-WatchSpec', str(spec)])
        self.assertEqual(self.sent(), [])   # neither sw_dump nor sw_stop nor quit, not even in the finalizer
        self.assertEqual(session['cleanup']['game'], 'stopped')   # the finalizer stopped this run's own game by its PID
        self.assertEqual(session['cleanup']['saves'].split(' to ')[0], 'identical')   # and the restore did not need the driver
        self.assertIn('the driver stopped answering at the end snapshot', session['stage_results']['watch'])

    def test_a_lost_reply_to_the_trial_close_fails_the_den_down(self):
        # Review of #245: a timeout of dp_end was only logged, so den_down could pass after the driver was lost.
        spec = self.work / 'spec.json'
        spec.write_text(json.dumps({'funcs': ['Pawn.Died']}), encoding='utf-8')
        done, session = self.run_down('close_lost', '-WatchSpec', str(spec), '-DownSeconds', '9')
        self.assertIsNotNone(session['den_down_up_after_s'])   # down and up were both seen
        self.assertNotIn('den_down', session['completed_stages'])
        self.assertIn('the driver stopped answering during the den down', session['stage_results']['den_down'])
        self.assertNotIn('sw_dump', self.sent())   # and the watch's check sent nothing afterwards

    def test_no_placement_after_the_window_even_when_a_poll_answers_late(self):
        # Review of #245: a poll started near the end of the 150 s window could return 'up' after it and still bring a re-placement.
        done, session = self.run_down('slow_poll', '-DownSeconds', '9', '-ReplaceAfterSeconds', '0')
        times = [float(r.split(' s:')[0]) for r in session['den_down_replace_results']]
        self.assertTrue(times and all(t < 9 for t in times), times)

    def test_a_shortened_wait_that_runs_out_is_driver_loss(self):
        # Review of #245, round 2: a poll whose wait the window had shortened, and that got no reply, left both flags empty and the stage
        # went on as if nothing had happened. It is driver loss: latched, the stage fails, and nothing more is sent.
        done, session = self.run_down('short_timeout', '-DownSeconds', '12', '-ReplaceAfterSeconds', '0')
        self.assertIn('no reply', session['stage_results']['den_down'])   # the stage fails on the lost reply, not on a window that ran out
        self.assertNotIn('no down in', session['stage_results']['den_down'])
        self.assertNotIn('den_down_trial_close', session)   # no trial close was sent
        self.assertNotIn('quit', self.sent())

    def test_a_down_that_answers_after_the_window_is_not_taken(self):
        # Review of #245, round 2: a reply that came back after the window was returned without a test of its deadline.
        done, session = self.run_down('late_down', '-DownSeconds', '8')
        self.assertNotIn('den_down_reason', session)
        self.assertIn('no down in 8 s', session['stage_results']['den_down'])

    def test_a_falling_shield_alone_is_not_a_hit(self):
        # Session 2 analysis, A6: her shield refilled between engagements, so a health-plus-shield rule saw hits where only the shield
        # fell. A re-placement now waits on her health only: a falling shield does not hold it back.
        done, session = self.run_down('shield_only', '-DownSeconds', '9', '-ReplaceAfterSeconds', '1')
        self.assertGreaterEqual(session['den_down_replacements'], 1)

    def test_the_den_down_defaults_are_240_s_and_20_s_of_health_unhit(self):
        text = CAPTURE.read_text(encoding='utf-8')
        self.assertIn('[int]$DownSeconds = 240,', text)
        self.assertIn('[int]$ReplaceAfterSeconds = 20,', text)

    def test_a_placement_gets_the_window_s_end_and_a_wait_clamped_to_it(self):
        # Review of #255, B1: the window limited when a re-placement was sent, not when it ran; its command waited the default 30 s.
        done, session = self.run_down('replace_one', '-DownSeconds', '9', '-ReplaceAfterSeconds', '0')
        wait, until, now = self.placement_waits()[0]
        # Review of #255, B8: the VALUE of until is the window's end, on the command clock (the performance counter in seconds) since the
        # session gate's second review: the window starts right after the immunity goes.
        off = float((self.work / 'immune_off_at.txt').read_text(encoding='utf-8'))
        self.assertAlmostEqual(until, off + 9, delta=1.5)
        self.assertLess(wait, until - now)   # and its reply wait fits in what is left of the window

    def placement_waits(self):
        rows = [line.split() for line in (self.work / 'replace_waits.txt').read_text(encoding='utf-8').splitlines() if line.strip()]
        return [(int(w), float(u), float(n)) for w, u, n in rows]

    def test_no_re_placement_with_less_than_6_s_of_the_window_left(self):
        # Review of #255, B7: a re-placement could go out with a 1 s reply wait, and a healthy slower reply then read as a lost driver.
        # Each poll takes 2.5 s here, so the window's later re-placements would come with under 6 s left.
        done, session = self.run_down('slow_poll', '-DownSeconds', '14', '-ReplaceAfterSeconds', '0')
        calls = self.placement_waits()
        self.assertTrue(calls, session)
        for wait, until, now in calls:
            self.assertGreaterEqual(until - now, 5.9, calls)
            self.assertGreaterEqual(wait, 4, calls)
        self.assertNotIn('stopped answering', session['stage_results']['den_down'])

    def test_health_read_once_then_unreadable_gives_no_second_placement(self):
        # Review of #255, B4: after a placement the health must be read again before another; here it never is.
        done, session = self.run_down('health_then_null', '-DownSeconds', '9', '-ReplaceAfterSeconds', '0')
        self.assertEqual(session['den_down_replacements'], 1)

    def test_an_unreadable_health_never_reads_as_unhit(self):
        done, session = self.run_down('pool_none', '-DownSeconds', '8', '-ReplaceAfterSeconds', '0')
        self.assertEqual(session['den_down_replacements'], 0)
        self.assertEqual(self.replace_calls(), 0)
        self.assertIn('no down in 8 s after 0 re-placements', session['stage_results']['den_down'])

    def test_a_down_between_the_poll_and_the_call_is_left_to_the_next_poll(self):
        done, session = self.run_down('down_at_replace', '-DownSeconds', '9', '-ReplaceAfterSeconds', '0')
        self.assertEqual(self.replace_calls(), 1)
        self.assertEqual(session['den_down_replacements'], 1)
        self.assertIn('not_up', session['den_down_replace_results'][0])   # logged, no placement
        self.assertEqual(session['den_down_reason'], 'injured_flag')   # the next poll saw the down


class ReadingsLoopTest(unittest.TestCase):
    """The readings loop (session 2 analysis, A2): deaths are counted apart from kill attempts, and a pickup that gave no reading is
    skipped from then on instead of retried. Invented values."""
    run_capture = CashCaptureTest.run_capture

    def setUp(self):
        CashCaptureTest.setUp(self)

    def test_deaths_are_counted_apart_from_kill_attempts(self):
        done, session = self.run_capture('kills', ['-MaxKills', '4'])
        self.assertEqual(session['kill_attempts'], 4)
        self.assertEqual(session['deaths'], 2)   # every other attempt killed

    def test_each_kill_attempt_is_followed_by_one_drop_snapshot(self):
        # Session 2 analysis, A3: a labelled 'drop' snapshot after each kill's wait, so a drop's lifetime can be read from the recording.
        spec = self.work / 'spec.json'
        spec.write_text(json.dumps({'funcs': ['Pawn.Died']}), encoding='utf-8')
        done, session = self.run_capture('kills', ['-MaxKills', '3', '-WatchSpec', str(spec)])
        drops = self.work / 'drops.txt'
        self.assertEqual(len(drops.read_text(encoding='utf-8').split()) if drops.exists() else 0, session['kill_attempts'])

    def test_kills_are_capped_so_drops_leave_room_for_the_other_labels(self):
        # Review of #251, note 7: with about 190 kills the per-kill drop snapshots would fill the 200 labelled reads and drop before_down,
        # down and after_respawn. -MaxKills is at most 150; 151 is refused before anything is touched.
        # Always with the stub helpers and this test's own hub lock, so a missing range check runs the stubbed capture, never the real one.
        stub = self.work / 'stub.ps1'
        stub.write_text(STUB.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__WORK__', str(self.work)), encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(CAPTURE), '-HubLock', str(self.hub),
                               '-Helpers', str(stub), '-MaxKills', '151'], capture_output=True, text=True, timeout=180,
                              env=dict(os.environ, OPENWILLOW_BL2=str(self.game), OW_SCENARIO='ok'))
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('MaxKills', done.stdout + done.stderr)
        self.assertFalse((self.work / 'rg/cash/session.json').exists())   # refused before anything ran
        watch = (ROOT / 'tools/real_game/scripts/slice_watch.py').read_text(encoding='utf-8')
        self.assertIn('SW_LABELLED_CAP = 200', watch)   # the room this range is set against

    def test_no_drop_snapshot_is_asked_for_before_the_map_settles(self):
        # Review of #251, point 3: a drop snapshot asked for before the map settled is recorded as not taken and fails the watch's check.
        # One read of the settle state comes first (no wait); an unsettled drop is counted, not asked for.
        spec = self.work / 'spec.json'
        spec.write_text(json.dumps({'funcs': ['Pawn.Died']}), encoding='utf-8')
        done, session = self.run_capture('kills_unsettled', ['-MaxKills', '3', '-WatchSpec', str(spec)])
        self.assertFalse((self.work / 'drops.txt').exists())
        self.assertEqual(session['drop_snapshots_unsettled'], session['kill_attempts'])

    def test_a_pickup_that_gave_no_reading_is_skipped_not_retried(self):
        class Obj:
            def __init__(self, **kw):
                self.__dict__.update(kw)
        ns = {}
        path = ROOT / 'tools/real_game/scripts/cash_probe.py'
        exec(compile(path.read_text(encoding='utf-8'), str(path), 'exec'), ns)
        at = lambda x: Obj(X=x, Y=0.0, Z=0.0)
        money = Obj(DefinitionData=Obj(BalanceDefinition='GD_Invented.Currency_Money_Little'), ExpLevel=7, GameStage=7)
        near = Obj(Name='WillowPickup_1', _path_name=lambda: 'Ice_P.TheWorld:PersistentLevel.WillowPickup_1', Location=at(100.0), Inventory=money, Physics=2, bCollideActors=True, bHidden=False, bDeleteMe=False)
        far = Obj(Name='WillowPickup_2', _path_name=lambda: 'Ice_P.TheWorld:PersistentLevel.WillowPickup_2', Location=at(900.0), Inventory=money, Physics=2, bCollideActors=True, bHidden=False, bDeleteMe=False)
        player = Obj(Location=at(0.0), GetExpLevel=lambda: 8)
        ns.update(unrealsdk=Obj(find_all=lambda cls: [near, far], make_struct=lambda kind, **kw: Obj(**kw)),
                  get_pc=lambda: Obj(Pawn=player, PlayerReplicationInfo=Obj(GetCurrencyOnHand=lambda n: 100)),
                  _p=lambda v, depth=0: v, _g=lambda o, n, d=None: getattr(o, n, d))
        self.assertEqual(ns['cp_collect_start'](), 'WillowPickup_1')
        ns['_count_touch'](None, None, None, None)   # the touch event fired, but no cash came and the pickup stayed
        row = ns['cp_collect_end']()
        self.assertIsNone(row['value_per_item'])
        self.assertEqual(row['touches'], 1)
        self.assertEqual(row['where_before']['location'], [100, 0, 0])
        self.assertEqual(row['where_before']['Physics'], 2)
        self.assertTrue(row['skipped_from_now'])
        self.assertEqual(ns['cp_collect_start'](), 'WillowPickup_2')   # the failed one is not retried
        # Session 2 analysis, A4: the item's part definitions are in the row (here the stand-in struct as _p returned it).
        self.assertIs(row['target']['parts'], money.DefinitionData)


    def probe(self, **extra):
        """cash_probe.py in a namespace of stand-ins: two money pickups, the nearer first."""
        class Obj:
            def __init__(self, **kw):
                self.__dict__.update(kw)
        self.Obj = Obj
        ns = {}
        path = ROOT / 'tools/real_game/scripts/cash_probe.py'
        exec(compile(path.read_text(encoding='utf-8'), str(path), 'exec'), ns)
        at = lambda x: Obj(X=x, Y=0.0, Z=0.0)
        money = Obj(DefinitionData=Obj(BalanceDefinition='GD_Invented.Currency_Money_Little'), ExpLevel=7, GameStage=7)
        pickups = [Obj(Name=f'WillowPickup_{n}', _path_name=lambda n=n: f'Ice_P.TheWorld:PersistentLevel.WillowPickup_{n}', Location=at(x),
                       Inventory=money, Physics=2, bCollideActors=True, bHidden=False, bDeleteMe=False) for n, x in ((1, 100.0), (2, 900.0))]
        self.cash = 100
        player = Obj(Location=at(0.0), GetExpLevel=lambda: 8)
        pri = Obj(GetCurrencyOnHand=lambda n: self.cash)
        ns.update(unrealsdk=Obj(find_all=lambda cls: list(pickups), make_struct=lambda kind, **kw: Obj(**kw)),
                  get_pc=lambda: Obj(Pawn=player, PlayerReplicationInfo=pri), _p=lambda v, depth=0: v, _g=lambda o, n, d=None: getattr(o, n, d))
        ns.update(extra)
        return ns, pri

    def test_a_try_that_raises_before_its_end_does_not_bring_the_pickup_back(self):
        # Review of #251, note 8: the skip was recorded at the end of a try, so a try that raised before it retried the same pickup and
        # "6 distinct pickups" could be untrue. It is recorded when the try starts.
        ns, pri = self.probe()
        self.assertEqual(ns['cp_collect_start'](), 'WillowPickup_1')
        pri.GetCurrencyOnHand = lambda n: (_ for _ in ()).throw(RuntimeError('invented: no cash read'))
        with self.assertRaises(RuntimeError):
            ns['cp_collect_end']()
        pri.GetCurrencyOnHand = lambda n: self.cash
        self.assertEqual(ns['cp_collect_start'](), 'WillowPickup_2')

    def test_a_skipped_pickup_that_goes_in_the_same_walk_spoils_the_reading(self):
        # Review of #251/#255, A2: present and levels came from the pickups not skipped, so a skipped pickup taken in the same walk was not
        # seen as gone and the cash of both was read as the target's value. They are recorded from all money pickups.
        ns, pri = self.probe()
        ns['CP']['skip'].append('WillowPickup_1')   # skipped earlier, still on the ground
        self.assertEqual(ns['cp_collect_start'](), 'WillowPickup_2')
        everything = ns['unrealsdk'].find_all('WillowPickup')
        ns['unrealsdk'].find_all = lambda cls: []   # the walk took both
        self.cash = 106
        row = ns['cp_collect_end']()
        self.assertEqual(sorted(row['gone']), ['WillowPickup_1', 'WillowPickup_2'])
        self.assertIsNone(row['value_per_item'])
        self.assertEqual(len(everything), 2)

    def test_a_walk_with_more_than_one_touch_gives_no_reading(self):
        # Review of #251, A7: a pickup that spawns AND is collected between the two censuses never shows in `gone`; its payment was read as
        # the target's. With the counter installed, more than one touch in the walk is no reading; without it, the census alone, recorded.
        for hook, touches, cash, value, rule in (('installed', 2, 106, None, 'more than one touch: not a reading'),
                                                 ('installed', 1, 103, 3, 'one touch'),
                                                 ('installed', 0, 103, 3, 'no touch counted: the census alone'),
                                                 ('not installed: invented', 2, 106, 6, 'touch counter unavailable: the census alone')):
            ns, pri = self.probe()
            ns['CP']['touch_hook'] = hook
            self.assertEqual(ns['cp_collect_start'](), 'WillowPickup_1')
            for _ in range(touches):
                ns['_count_touch'](None, None, None, None)
            left = [p for p in ns['unrealsdk'].find_all('WillowPickup') if p.Name != 'WillowPickup_1']
            ns['unrealsdk'].find_all = lambda cls: list(left)   # the target went; the one that spawned and went is in neither census
            self.cash = cash
            row = ns['cp_collect_end']()
            self.assertEqual((row['gone'], row['touches'], row['value_per_item'], row['touch_rule']), (['WillowPickup_1'], touches, value, rule))

    def test_the_touch_counter_is_an_observer_and_a_missing_function_is_not_fatal(self):
        # Review of #251, note 6: installed in every cash run (with or without a watch), outside the watch's groups. It counts the calls
        # before them, reads nothing, never blocks or changes them; a build without the function reports it and the run goes on.
        added = []

        class Untouchable:
            def __getattr__(self, name):
                raise AssertionError(f'the observer read {name}')
        ns, _ = self.probe()
        Obj = self.Obj
        ns.update(Type=Obj(PRE='pre'), has_hook=lambda *a: False, add_hook=lambda *a: added.append(a), _declared_paths=lambda c, f: [],
                  cb_mods_off=lambda: [])
        ns['unrealsdk'].find_object = lambda kind, path: (_ for _ in ()).throw(ValueError('invented: no such function'))
        started = ns['cp_start']()
        self.assertIn('not installed', ns['CP']['touch_hook'])
        self.assertEqual(added, [])
        self.assertIn('mods_off', started)   # the start went on
        ns['unrealsdk'].find_object = lambda kind, path: object()
        ns['cp_start']()
        self.assertEqual(ns['CP']['touch_hook'], 'installed')
        self.assertEqual([(a[0], a[1], a[2]) for a in added], [(ns['CP_TOUCH'], 'pre', ns['CP_TOUCH_ID'])])
        before = ns['CP']['touches']
        self.assertIsNone(added[0][3](Untouchable(), Untouchable(), None, Untouchable()))   # returns None: the call goes on unchanged
        self.assertEqual(ns['CP']['touches'], before + 1)

    def test_cp_died_reads_fields_and_never_calls_one(self):
        # Review of #251, note 6: a field that turns out to be a function is skipped, never called.
        ns, _ = self.probe()
        called = []
        pawn = self.Obj(bDeleteMe=False, bPlayedDeath=lambda: called.append('bPlayedDeath') or True)
        ns['_find_ai'] = lambda name: pawn
        self.assertFalse(ns['cp_died']('WillowAIPawn_2'))
        self.assertEqual(called, [])
        pawn.bPlayedDeath = True
        self.assertTrue(ns['cp_died']('WillowAIPawn_2'))
        ns['_find_ai'] = lambda name: None   # gone
        self.assertTrue(ns['cp_died']('WillowAIPawn_2'))


class LatchTest(unittest.TestCase):
    """Review of #251/#255, A1: after a watch helper has latched the driver-lost condition (it does not throw), the capture's own Py sends
    nothing more: no cp_died, no further kill and no click, no cp_immune_off. The cleanup and the restore still run."""
    run_capture = CashCaptureTest.run_capture
    saves = CashCaptureTest.saves
    locks_held = CashCaptureTest.locks_held

    def setUp(self):
        CashCaptureTest.setUp(self)
        self.spec = self.work / 'spec.json'
        self.spec.write_text(json.dumps({'funcs': ['Pawn.Died']}), encoding='utf-8')

    def lines(self, name):
        f = self.work / name
        return f.read_text(encoding='utf-8').split() if f.exists() else []

    def test_a_settle_read_that_times_out_sends_nothing_after_it_in_the_kill_loop(self):
        done, session = self.run_capture('settle_lost', ['-MaxKills', '4', '-WatchSpec', str(self.spec)])
        self.assertEqual(len(self.lines('clicks.txt')), 3)   # the one kill's three shots, then nothing
        self.assertNotIn('cp_died', self.lines('sent.txt'))
        self.assertIn('the driver stopped answering', session['stage_results']['readings'])
        self.assertIn('the driver stopped answering', session['stage_results']['den_read'])   # the later stages skip themselves
        self.assertEqual(self.saves(), 'original save')   # the cleanup and the restore still ran
        self.assertTrue(session['cleanup']['saves'].startswith('identical'), session['cleanup'])
        self.assertTrue((self.work / 'rg/cash/save_compare.json').exists())
        self.assertEqual(self.locks_held(), (False, False))

    def nothing_pressed_after_the_loss(self):
        events = self.lines('events.txt')
        self.assertIn('lost', events)
        after = events[events.index('lost') + 1:]
        self.assertEqual([e for e in after if e in ('key', 'click')], [], events)

    def ended_without_den_or_input(self, session):
        self.assertEqual(self.lines('keys.txt'), ['Enter', 'Enter'])   # the boot's and the travel's, and no key after the failure
        self.assertEqual(self.lines('den.txt'), [])   # no den command
        self.assertNotIn('den_read', session.get('completed_stages', []))
        self.assertEqual(self.saves(), 'original save')   # the outer cleanup and the restore ran
        self.assertTrue(session['cleanup']['saves'].startswith('identical'), session['cleanup'])
        self.assertTrue((self.work / 'rg/cash/save_compare.json').exists())
        self.assertEqual(self.locks_held(), (False, False))
        marks = [(m['stage'], m['event']) for m in session['stage_marks']]
        self.assertEqual(marks[-2:], [('readings', 'enter'), ('readings', 'done')])   # the stage closed on its own stamp, nothing after it

    def test_lost_focus_in_the_kill_loop_ends_the_run(self):
        # Hub, 2026-11-13: a readings failure other than a lost driver ends the run, as at e7b5fa3; no den placement follows.
        done, session = self.run_capture('focus_lost_in_kills', ['-MaxKills', '4', '-DenDown'])
        self.assertEqual(self.lines('clicks.txt'), [])   # the focus check came before the shots
        self.assertIn('not the foreground window', session['stage_results']['readings'])
        self.assertIn('not the foreground window', session['status'])
        self.ended_without_den_or_input(session)

    def test_a_game_side_traceback_in_the_readings_ends_the_run(self):
        done, session = self.run_capture('kill_traceback', ['-MaxKills', '4', '-DenDown'])
        self.assertEqual(self.lines('clicks.txt'), [])
        self.assertIn('game python error', session['stage_results']['readings'])
        self.ended_without_den_or_input(session)

    def test_a_failed_closing_mark_still_ends_the_run_through_the_outer_handler(self):
        # Review of #251, A8: the readings' closing mark itself fails (its reply never comes, which latches the condition); the rethrow
        # that follows it still ends the run with the readings' own error, through the cleanup and the restore.
        done, session = self.run_capture('kill_traceback_mark_lost', ['-MaxKills', '4', '-DenDown', '-WatchSpec', str(self.spec)])
        self.assertIn('game python error', session['status'])   # the readings' error, not the mark's
        self.assertIn('the driver stopped answering', session['cleanup']['watch'])   # the mark's loss: nothing more sent
        self.nothing_pressed_after_the_loss()
        self.ended_without_den_or_input(session)

    def test_a_timed_out_smoke_snapshot_is_followed_by_no_key(self):
        # Review of #251, A5: Save-SliceWatchSnap 'smoke_unsettled' latched the loss without throwing, and the Enter after it still went out.
        done, session = self.run_capture('smoke_unsettled_lost', ['-WatchSpec', str(self.spec), '-WatchSmoke', '-DenDown'])
        self.nothing_pressed_after_the_loss()
        self.assertIn('the driver stopped answering', session['stage_results']['den_read'])
        self.assertIn('the driver stopped answering', session['stage_results']['den_down'])
        self.assertEqual(self.saves(), 'original save')
        self.assertEqual(self.locks_held(), (False, False))

    def test_a_watch_start_whose_recovery_stop_times_out_latches_the_loss(self):
        # Review of #251, A6 (a): the start failed for another reason, and the recovery sw_stop's timeout was only written into the result.
        done, session = self.run_capture('watch_start_stop_lost', ['-WatchSpec', str(self.spec), '-DenDown'])
        self.assertTrue(session['watch_start']['stop_after_error'].startswith('no reply'), session['watch_start'])
        self.nothing_pressed_after_the_loss()
        self.assertEqual(self.lines('keys.txt'), ['Enter', 'Enter'])   # the boot's and the travel's, both before the watch started
        self.assertIn('the driver stopped answering', session['stage_results']['readings'])
        self.assertIn('the driver stopped answering', session['stage_results']['den_read'])
        self.assertEqual(self.lines('sent.txt').count('sw_stop'), 1)   # the finalizer sent no second stop
        self.assertIn('not sent: the driver stopped answering', session['cleanup']['watch'])
        self.assertEqual(self.saves(), 'original save')

    def test_a_quit_that_gets_no_reply_is_followed_by_no_driver_command(self):
        # Review of #251, A6 (c): the quit's catch printed "no reply" and the finalizer's stops still went to the driver.
        done, session = self.run_capture('quit_lost', ['-WatchSpec', str(self.spec)])
        sent = self.lines('sent.txt')
        self.assertEqual(sent[sent.index('quit') + 1:], [], sent)
        self.assertIn('not sent: the driver stopped answering', session['cleanup']['hooks'])
        self.assertIn('not sent: the driver stopped answering', session['cleanup']['watch'])
        self.assertEqual(session['cleanup']['game'], 'stopped')   # stopped by its PID
        self.assertEqual(self.saves(), 'original save')
        self.assertTrue(session['cleanup']['saves'].startswith('identical'), session['cleanup'])
        self.assertEqual(self.locks_held(), (False, False))

    def same_as_a_timeout(self, unreadable, lost, extra, reason='could not be read'):
        """Review of #251/#255, T1: a reply file that cannot be read sets the condition exactly as a reply that never came. Run both;
        after the loss nothing goes to the game, the game is stopped by this run's own PID, and the cleanup, the restore, the saves
        compare and the pending record come out as on the timeout."""
        seen = {}
        for scenario in (lost, unreadable):
            done, session = self.run_capture(scenario, extra)
            events = self.lines('events.txt')
            self.assertIn('lost', events, scenario)
            after = events[events.index('lost') + 1:]
            self.assertEqual([e for e in after if e in ('key', 'click', 'cmd')], [], (scenario, events))   # no input, no driver command
            self.assertEqual(self.lines('stopped_pid.txt'), ['4242'], scenario)   # this run's own game, by its PID
            self.assertEqual(self.saves(), 'original save', scenario)
            self.assertTrue((self.work / 'rg/cash/save_compare.json').exists(), scenario)
            self.assertEqual(self.locks_held(), (False, False), scenario)
            pending = session['cleanup'].get('pending')
            self.assertTrue(str(pending).startswith('removed'), (scenario, pending))   # the pending record is completed as on any clean restore
            norm = lambda v: re.sub(r'save-backup-[\d-]+', 'BACKUP', re.sub(r'ow-pending-[^"]*?\.json', 'PENDING',
                                    re.sub(r'no reply to .*', 'no reply', json.dumps(v, sort_keys=True))))
            seen[scenario] = (norm(session['cleanup']), norm(session['stage_results']), norm(session['status']), norm(pending))
            for name in ('events.txt', 'stopped_pid.txt', 'keys.txt', 'clicks.txt', 'den.txt', 'sent.txt'):
                (self.work / name).unlink(missing_ok=True)
        self.assertIn(reason, json.dumps(session['stage_results']))   # the transport's own reason is kept
        self.assertEqual(seen[unreadable], seen[lost])
        return session

    def test_an_unreadable_reply_at_the_smoke_snapshot_is_a_lost_driver(self):
        # T2 (a): the snapshot swallows its error; before, a read failure there did not latch, and the Enter after it still went out.
        session = self.same_as_a_timeout('smoke_unsettled_unreadable', 'smoke_unsettled_lost', ['-WatchSpec', str(self.spec), '-WatchSmoke', '-DenDown'])
        self.assertIn('the driver stopped answering', session['stage_results']['den_down'])
        self.assertIn('not sent: the driver stopped answering', session['cleanup']['watch'])

    def test_an_arbitrary_transport_fault_at_the_smoke_snapshot_is_a_lost_driver(self):
        # Review of #263, S1: any failure inside the transport, whatever its text, leaves in the no-reply form; at a helper that swallows
        # its errors it sets the condition and nothing follows, exactly as a timeout there.
        self.same_as_a_timeout('smoke_unsettled_fault', 'smoke_unsettled_lost', ['-WatchSpec', str(self.spec), '-WatchSmoke', '-DenDown'],
                               reason='transport error')

    def test_an_unreadable_reply_to_a_kill_loop_command_is_a_lost_driver(self):
        # T2 (b): a plain Py in the kill loop; before, the read error ended the run but the finalizer still sent its stops.
        session = self.same_as_a_timeout('kill_unreadable', 'kill_lost', ['-MaxKills', '4', '-WatchSpec', str(self.spec), '-DenDown'])
        self.assertEqual(self.lines('clicks.txt'), [])
        self.assertIn('the driver stopped answering', session['stage_results']['den_read'])
        self.assertIn('not sent: the driver stopped answering', session['cleanup']['watch'])

    def test_a_timed_out_before_down_is_not_followed_by_removing_the_immunity(self):
        done, session = self.run_capture('before_down_lost', ['-DenDown', '-RespawnSeconds', '8', '-WatchSpec', str(self.spec)])
        self.assertNotIn('cp_immune_off', self.lines('sent.txt'))
        self.assertIn('driver stopped answering', session['stage_results']['den_down'])
        self.assertEqual(self.saves(), 'original save')
        self.assertTrue(session['cleanup']['saves'].startswith('identical'), session['cleanup'])


class LoadWaitTest(unittest.TestCase):
    """The save's load after the menu's Enter (Wait-PlayerPawn in realgame.ps1, session 2 of 2026-11-12): the game may sit on the menu creating
    its online session; the capture waits for the pawn within named bounds, and a run that fails there stops nothing it never loaded."""
    run_capture = CashCaptureTest.run_capture
    locks_held = CashCaptureTest.locks_held
    saves = CashCaptureTest.saves

    def setUp(self):
        CashCaptureTest.setUp(self)

    def test_a_load_that_comes_late_is_waited_for(self):
        done, session = self.run_capture('load_late', ['-MenuWaitSeconds', '30', '-LoadWaitSeconds', '30'])
        self.assertTrue(session['load_wait']['ok'], session.get('load_wait'))
        self.assertEqual(session['status'], 'completed', done.stdout + done.stderr)

    def test_a_menu_that_never_loads_fails_naming_the_online_session_wait_and_restores(self):
        spec = self.work / 'spec.json'
        spec.write_text(json.dumps({'funcs': ['Pawn.Died']}), encoding='utf-8')
        done, session = self.run_capture('never_loads', ['-WatchSpec', str(spec), '-MenuWaitSeconds', '4', '-LoadWaitSeconds', '4'])
        self.assertFalse(session['load_wait']['ok'])
        self.assertIn('no player pawn: still on the main menu', session['status'])
        self.assertIn('online session', session['status'])
        self.assertIn('ran out at 4 s', session['status'])
        self.assertEqual(self.saves(), 'original save')   # the finalizer restored as always
        self.assertEqual(self.locks_held(), (False, False))
        # The watch was never loaded: nothing to stop, which is not an error.
        self.assertEqual(session['cleanup']['watch'], 'nothing to stop: never loaded')
        self.assertNotIn('NameError', done.stdout + done.stderr)

    def test_a_driver_lost_while_loading_fails_within_the_bound(self):
        done, session = self.run_capture('load_lost', ['-MenuWaitSeconds', '4', '-LoadWaitSeconds', '4'])
        self.assertFalse(session['load_wait']['ok'])
        self.assertIn('the driver did not answer for', session['status'])
        self.assertEqual(self.saves(), 'original save')

    def test_a_pawn_reply_after_the_load_bound_is_not_taken(self):
        # Review of #245: a read that started before the load bound and answered after it was taken as a success.
        done, session = self.run_capture('late_pawn', ['-MenuWaitSeconds', '4', '-LoadWaitSeconds', '4'])
        self.assertFalse(session['load_wait']['ok'], session['load_wait'])
        self.assertLess(session['load_wait']['after_enter_s'], 6)
        self.assertIn('no player pawn', session['status'])

    def test_a_reply_after_the_silence_bound_does_not_reopen_it(self):
        # Review of #245, round 2 (and the note that the late-pawn test only exercised the clamp): a read that ignores its wait and answers
        # with the pawn after the silence bound is not taken, and it does not refresh the time of the last reply.
        done, session = self.run_capture('pawn_after_silence', ['-MenuWaitSeconds', '30', '-LoadWaitSeconds', '4'])
        self.assertFalse(session['load_wait']['ok'], session['load_wait'])
        self.assertIn('the driver did not answer for', session['status'])
        self.assertIn('a reply after the bound, not taken', session['status'])

    def test_a_load_that_never_brings_the_pawn_fails_on_the_load_bound(self):
        done, session = self.run_capture('load_stuck', ['-MenuWaitSeconds', '4', '-LoadWaitSeconds', '4'])
        self.assertIn('the menu was left, but no player pawn', session['status'])


class FinalizerStopTextTest(unittest.TestCase):
    """The finalizers' stop commands as the game runs them: each stops only what was loaded, and says 'nothing to stop' when nothing was
    (a run that failed at the load printed a NameError there on 2026-11-12). The Python is taken from the scripts' own text."""

    def command(self, script, needle):
        text = (ROOT / 'tools/real_game' / script).read_text(encoding='utf-8')
        line = next(l for l in text.splitlines() if needle in l and "'nothing to stop'" in l and 'Py "' in l)
        return line.split('Py "', 1)[1].split('" 5;', 1)[0].replace('`n', '\n')

    def run_in(self, code, defined):
        import contextlib
        import io
        calls = []
        scope = {name: (lambda name=name: calls.append(name) or 'stopped') for name in defined}
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            exec(code, scope)
        return out.getvalue().strip(), calls

    def test_each_stop_is_quiet_when_nothing_was_loaded_and_stops_what_was(self):
        for script, needle, names in (('cash_capture.ps1', 'sw_stop', ['sw_stop']),
                                      ('phaselock_integrity_capture.ps1', 'pi_stop', ['pi_stop', 'unmark_lock']),
                                      ('mission_screen_capture.ps1', 'ms_stop', ['ms_stop'])):
            code = self.command(script, needle)
            self.assertEqual(self.run_in(code, []), ('nothing to stop', []), script)
            self.assertEqual(self.run_in(code, names), ('stopped', names), script)
        code = self.command('phaselock_integrity_capture.ps1', 'pi_stop')
        self.assertEqual(self.run_in(code, ['unmark_lock']), ('stopped', ['unmark_lock']))   # loaded half way: what is there is stopped


class DpReplaceTest(unittest.TestCase):
    """den_probe.dp_replace with invented objects: one call that checks she is up, then picks a living enemy by fields only: the first
    placement's enemy at bearing 180 while it lives, else the nearest on the side she is on now."""

    class Obj:
        def __init__(self, **kw):
            self.__dict__.update(kw)

    def pawn(self, n, dist, health, live=True, controller=True, at=None, kind=None):
        return self.Obj(Name=f'WillowAIPawn_{n}', live=live, dist=dist, health=health, controller=self.Obj() if controller else None,
                        Location=self.Obj(X=at[0], Y=at[1], Z=0.0) if at else self.Obj(X=dist, Y=0.0, Z=0.0), kind=kind)

    def setUp(self):
        self.ns = {}
        path = ROOT / 'tools/real_game/scripts/den_probe.py'
        exec(compile(path.read_text(encoding='utf-8'), str(path), 'exec'), self.ns)
        self.placed = []
        here = self.Obj(X=0.0, Y=0.0, Z=0.0)
        self.state = 'up'
        fresh = {'stale': None, 'CurrentValue': 50.0}
        self.pawns = [self.pawn(1, 3000.0, fresh),
                      self.pawn(2, 1000.0, {'stale': 'guid differs', 'CurrentValue': 50.0}),
                      self.pawn(3, 900.0, {'stale': None, 'CurrentValue': 0.0}),
                      self.pawn(4, 800.0, fresh, live=False),
                      self.pawn(5, 7000.0, fresh),
                      self.pawn(7, 1500.0, fresh, controller=False),   # still spawning or being destroyed
                      self.pawn(6, 2000.0, {'stale': None, 'CurrentValue': 10.0}, at=(0.0, 2000.0))]
        self.ns.update(
            cp_down_state=lambda: {'state': self.state},
            get_pc=lambda: self.Obj(Pawn=self.Obj(Location=here)),
            unrealsdk=self.Obj(find_all=lambda cls: list(self.pawns)),
            sw_live_pawn=lambda p: p.live, _sw_pool_refs=lambda p: {'health_pool': p.health},
            _sw_dist=lambda p, h: round(((p.Location.X - h.X) ** 2 + (p.Location.Y - h.Y) ** 2 + (p.Location.Z - h.Z) ** 2) ** 0.5, 1),
            _sw_fields=lambda p, names: {'Controller': p.controller, 'AIClass': p.kind},
            cb_place=lambda name, distance, bearing: self.placed.append((name, distance, round(bearing, 1))))

    def test_without_the_first_enemy_the_nearest_living_one_on_her_side(self):
        got = self.ns['dp_replace']()
        self.assertEqual((got['result'], got['rule']), ('placed', 'kind_my_side'))
        # stale, empty, not live, too far and without a Controller are passed over; she stays on her side of it (it is at +Y, she at 0)
        self.assertEqual(self.placed, [('WillowAIPawn_6', 1600.0, -90.0)])

    def test_the_first_enemy_while_it_lives_at_bearing_180(self):
        self.ns['DP']['trial'] = {'first': 'WillowAIPawn_1'}
        got = self.ns['dp_replace']()
        self.assertEqual((got['rule'], got['enemy']), ('first', 'WillowAIPawn_1'))   # chosen over the nearer one
        self.assertEqual(self.placed, [('WillowAIPawn_1', 1600.0, 180.0)])
        self.placed.clear()
        self.ns['DP']['trial'] = {'first': 'WillowAIPawn_3'}   # the first enemy dead: the nearest on her side
        self.assertEqual(self.ns['dp_replace']()['rule'], 'kind_my_side')
        self.assertEqual(self.placed, [('WillowAIPawn_6', 1600.0, -90.0)])

    def test_the_strongest_kind_in_reach_comes_first(self):
        # Session 2 analysis, A6: the nearest enemy could be a lone baby or slinger that never downed her. The adult comes first, then the
        # regular, the slinger, any other kind and the baby last, read from the AIClass field; the first placement's enemy only wins
        # within its own kind.
        fresh = {'stale': None, 'CurrentValue': 50.0}
        gd = 'GD_Invented.Character.'
        self.pawns = [self.pawn(11, 200.0, fresh, kind=gd + 'CharClass_PrimalBeastBaby'),
                      self.pawn(12, 500.0, fresh, kind=gd + 'CharClass_PrimalBeast'),
                      self.pawn(13, 800.0, fresh, kind=gd + 'CharClass_PrimalBeast_Ranged'),
                      self.pawn(14, 3000.0, fresh, kind=gd + 'CharClass_PrimalBeastAdult', at=(0.0, 3000.0))]
        self.ns['DP']['trial'] = {'first': 'WillowAIPawn_12'}   # the first placement was at a regular
        got = self.ns['dp_replace']()
        self.assertEqual((got['rule'], got['enemy'], got['kind']), ('kind_my_side', 'WillowAIPawn_14', gd + 'CharClass_PrimalBeastAdult'))
        self.assertEqual(self.placed, [('WillowAIPawn_14', 1600.0, -90.0)])
        self.placed.clear()
        self.pawns = self.pawns[:3]   # no adult: the regular, which is also the first placement's enemy, at bearing 180
        self.assertEqual(self.ns['dp_replace']()['rule'], 'first')
        self.assertEqual(self.placed, [('WillowAIPawn_12', 1600.0, 180.0)])
        self.assertEqual([self.ns['dp_kind_rank'](gd + k) for k in ('CharClass_PrimalBeastAdult', 'CharClass_PrimalBeast',
                          'CharClass_PrimalBeast_Ranged', 'CharClass_Invented', 'CharClass_PrimalBeastBaby')], [0, 1, 2, 3, 4])
        self.assertEqual(self.ns['dp_kind_rank'](None), 3)

    def test_a_late_call_moves_nothing(self):
        # Review of #255, B1: a call at or after the window's end answers 'late' and moves nothing, also when the end passes during it.
        # The probe reads the command clock (qpc_seconds, from the driver's scope), never the wall clock.
        self.ns['qpc_seconds'] = lambda: 2000.0
        self.ns['time'] = self.Obj(time=lambda: 0.0)   # a wall clock stepped far back must not matter
        self.assertEqual(self.ns['dp_replace'](until=1000.0), {'result': 'late'})
        self.ns['qpc_seconds'] = lambda: 1000.0   # at the end exactly: late as well
        self.assertEqual(self.ns['dp_replace'](until=1000.0), {'result': 'late'})
        ticks = iter([999.0, 1001.0])   # in time at the start, past the end right before the placement
        self.ns['qpc_seconds'] = lambda: next(ticks)
        self.assertEqual(self.ns['dp_replace'](until=1000.0), {'result': 'late'})
        self.assertEqual(self.placed, [])

    def test_the_first_enemy_far_from_her_is_still_with_its_pack(self):
        # Review of #255, B6: its distance of 0 from its own location was read as unknown, so with her more than 3500 from it, it fell out.
        fresh = {'stale': None, 'CurrentValue': 50.0}
        gd = 'GD_Invented.Character.'
        self.pawns = [self.pawn(12, 500.0, fresh, kind=gd + 'CharClass_PrimalBeast'),
                      self.pawn(14, 4000.0, fresh, kind=gd + 'CharClass_PrimalBeastAdult', at=(0.0, 4000.0))]
        self.ns['DP']['trial'] = {'first': 'WillowAIPawn_14'}   # the first placement's enemy, the only adult, 4000 from her
        got = self.ns['dp_replace']()
        self.assertEqual((got['result'], got['rule'], got['enemy']), ('placed', 'first', 'WillowAIPawn_14'))
        self.assertEqual(self.placed, [('WillowAIPawn_14', 1600.0, 180.0)])

    def test_only_the_pack_she_fought_is_ranked(self):
        # Review of #255, B2: an adult of another group, beyond 3500 of her and of the first placement's enemy, does not outrank the pack.
        fresh = {'stale': None, 'CurrentValue': 50.0}
        gd = 'GD_Invented.Character.'
        self.pawns = [self.pawn(12, 500.0, fresh, kind=gd + 'CharClass_PrimalBeast'),
                      self.pawn(14, 5000.0, fresh, kind=gd + 'CharClass_PrimalBeastAdult', at=(0.0, 5000.0))]
        self.ns['DP']['trial'] = {'first': 'WillowAIPawn_12'}
        got = self.ns['dp_replace']()
        self.assertEqual((got['rule'], got['enemy']), ('first', 'WillowAIPawn_12'))

    def test_a_candidate_on_her_own_spot_is_passed_over(self):
        # Review of #255, B3: with her horizontal position equal to the candidate's there is no side to keep; the next one is taken.
        fresh = {'stale': None, 'CurrentValue': 50.0}
        gd = 'GD_Invented.Character.'
        above = self.pawn(15, 300.0, fresh, kind=gd + 'CharClass_PrimalBeastAdult')
        above.Location = self.Obj(X=0.0, Y=0.0, Z=300.0)
        self.pawns = [above, self.pawn(16, 1000.0, fresh, kind=gd + 'CharClass_PrimalBeast')]
        got = self.ns['dp_replace']()
        self.assertEqual((got['rule'], got['enemy']), ('kind_my_side', 'WillowAIPawn_16'))

    def test_not_up_places_nothing(self):
        for state in ('injured_flag', 'no_health', 'no_pawn'):
            self.state = state
            self.assertEqual(self.ns['dp_replace'](), {'result': 'not_up', 'state': state})
        self.assertEqual(self.placed, [])

    def test_nobody_in_reach_and_errors_are_results_not_tracebacks(self):
        self.pawns = self.pawns[1:6]
        self.assertEqual(self.ns['dp_replace'](), {'result': 'no_enemy'})
        self.ns['get_pc'] = lambda: self.Obj(Pawn=None)   # the pawn gone in that moment
        self.assertEqual(self.ns['dp_replace']()['result'], 'error')
        del self.ns['sw_live_pawn']
        self.assertEqual(self.ns['dp_replace'](), {'result': 'no_watch'})
        self.assertEqual(self.placed, [])


class DpReplaceWatchHelpersTest(unittest.TestCase):
    """dp_replace with the watch's real helpers (slice_watch.py, loaded first into the same namespace, as the driver runs them) and invented
    pawns and pools, so the shape of a real _sw_pool_refs record is what dp_replace reads."""

    class Obj:
        def __init__(self, **kw):
            self.__dict__.update(kw)

    def pawn(self, n, x, value=50.0, guid=7, slot_guid=7, controller=True, kind=None):
        pool = self.Obj(CurrentValue=value, MaxValue=100.0, MinValue=0.0, MaxValueBaseValue=100.0, PoolGUID=slot_guid,
                        Definition=self.Obj(), AssociatedProvider=self.Obj())
        ref = self.Obj(PoolIndexInManager=0, PoolGUID=guid, PoolManager=self.Obj(ResourcePools=[pool]), Data=pool)
        path = f'Ice_P.TheWorld:PersistentLevel.WillowAIPawn_{n}'
        return self.Obj(Name=f'WillowAIPawn_{n}', _path_name=lambda: path, bDeleteMe=False, Location=self.Obj(X=x, Y=0.0, Z=0.0),
                        Controller=self.Obj() if controller else None, HealthPool=ref, ShieldArmor=None, AIClass=kind)

    def setUp(self):
        self.ns = {}
        for name in ('slice_watch.py', 'den_probe.py'):
            path = ROOT / 'tools/real_game/scripts' / name
            exec(compile(path.read_text(encoding='utf-8'), str(path), 'exec'), self.ns)
        self.placed = []
        here = self.Obj(X=0.0, Y=0.0, Z=0.0)
        self.pawns = []
        self.ns.update(
            cp_down_state=lambda: {'state': 'up'},
            get_pc=lambda: self.Obj(Pawn=self.Obj(Location=here)),
            unrealsdk=self.Obj(find_all=lambda cls: list(self.pawns)),
            cb_place=lambda name, distance, bearing: self.placed.append((name, round(bearing, 1))))

    def test_a_real_pool_record_is_read_and_a_bad_one_skipped(self):
        self.pawns = [self.pawn(1, 500.0, guid=7, slot_guid=8),   # the slot holds another pool: stale
                      self.pawn(2, 600.0, value=0.0),             # dead
                      self.pawn(3, 700.0, controller=False),      # no Controller
                      self.pawn(4, 900.0)]
        got = self.ns['dp_replace']()
        self.assertEqual((got['result'], got['enemy']), ('placed', 'WillowAIPawn_4'), got)
        self.assertEqual(self.placed, [('WillowAIPawn_4', 180.0)])   # she is at -X of it: her own side is bearing 180
        self.pawns = self.pawns[:3]
        self.assertEqual(self.ns['dp_replace'](), {'result': 'no_enemy'})

    def test_the_kind_is_read_as_the_path_name_and_without_the_object_type_every_kind_is_other(self):
        # Review of #255, B4: AIClass reaches dp_kind_rank as the path name _sw_fields makes of an object.
        class Kind:
            def __init__(self, path):
                self.path = path

            def _path_name(self):
                return self.path
        gd = 'GD_Invented.Character.'
        self.ns['_SwUObject'] = Kind   # the SDK's object type, as the watch's _sw_p tests for it
        self.pawns = [self.pawn(21, 500.0, kind=Kind(gd + 'CharClass_PrimalBeast')), self.pawn(22, 900.0, kind=Kind(gd + 'CharClass_PrimalBeastAdult'))]
        got = self.ns['dp_replace']()
        self.assertEqual((got['enemy'], got['kind']), ('WillowAIPawn_22', gd + 'CharClass_PrimalBeastAdult'))   # the adult, by its path name
        self.ns['_SwUObject'] = ()   # the object type unavailable: every kind ranks "other", and the reply shows what was read
        got = self.ns['dp_replace']()
        self.assertEqual(got['enemy'], 'WillowAIPawn_21')   # the nearest
        self.assertTrue(str(got['kind']).startswith('<'), got)


if __name__ == '__main__':
    unittest.main()
