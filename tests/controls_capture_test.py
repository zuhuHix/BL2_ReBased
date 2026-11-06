"""Synthetic tests of tools/real_game/controls_capture.ps1's data-safety path: the real script and the real realgame.ps1 helpers run
against invented text files in a temporary folder, with only the game itself stubbed. No game, no game files, no real lock.
Needs Windows PowerShell.

Run: python tests/controls_capture_test.py
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
CAPTURE = ROOT / 'tools/real_game/controls_capture.ps1'

# Dot-sourced by the capture in place of realgame.ps1: the real helpers, then the game's side replaced. $env:OW_SCENARIO picks a fault.
STUB = r"""
. '__REALGAME__'
$work = '__WORK__'
$scenario = $env:OW_SCENARIO
$script:StubPageAsks = 0
$script:StubMeleeCalls = 0
$script:RG = Join-Path $work 'rg'
$script:CmdDir = Join-Path $script:RG 'cmd'
$script:WillowDir = Join-Path $work 'willow'
$script:Game = $env:OPENWILLOW_BL2
$script:DriverDir = Join-Path $script:Game 'sdk_mods/openwillow_realgame'
$script:Lock = Join-Path $work 'run.lock'
New-Item -ItemType Directory -Force $script:CmdDir | Out-Null
function Start-Sleep { }
function Get-GameProcess { $null }
function Get-GameProcesses {
    if ($scenario -eq 'foreign_before' -or ($scenario -eq 'foreign_during' -and $script:Fake.HasExited)) { @([pscustomobject]@{ Id = 999 }) } else { @() }
}
function ConvertTo-Json {
    param([Parameter(ValueFromPipeline)]$InputObject, [int]$Depth = 2)
    process {
        if ($scenario -eq 'evidence_fails' -and $InputObject -is [hashtable] -and $InputObject.ContainsKey('compared')) { throw 'disk full' }
        $InputObject | Microsoft.PowerShell.Utility\ConvertTo-Json -Depth $Depth
    }
}
function Enter-RunLock { Set-Content -LiteralPath $script:Lock 'held' }
function Exit-RunLock { Remove-Item -LiteralPath $script:Lock -Force }
function Install-Driver { if ($scenario -eq 'install_fails') { throw 'install broke' }; New-Item -ItemType Directory -Force $script:DriverDir | Out-Null }
function Remove-Driver { if ($scenario -ne 'driver_stuck') { Remove-Item -Recurse -Force $script:DriverDir } }
function Save-Shot { 'shot' }
function Focus-Game { $true }
function Assert-GameForeground { if ($scenario -eq 'no_focus') { throw 'refusing to send input: stub' } }
function Send-Key { }
function Invoke-GamePyFile { 'ok' }
function Stop-OwnGame { if ($scenario -eq 'hung') { 'refused: stub' } else { $script:Fake.HasExited = $true; 'stopped' } }
$script:Fake = [pscustomobject]@{ Id = 4242; StartTime = (Get-Date); HasExited = $false }
$script:Fake | Add-Member ScriptMethod WaitForExit { param($ms) $this.HasExited } | Out-Null
$script:Fake | Add-Member ScriptMethod Refresh { } | Out-Null
function Start-Game { $script:Fake }
function Invoke-GamePy([string]$Code, [int]$TimeoutSeconds = 30) {
    switch -Wildcard ($Code) {
        '*print("stopped")*' { return $(if ($scenario -eq 'hooks_traceback') { 'Traceback: no cp_unhook' } else { 'stopped' }) }
        '*block_saves*' { return $(if ($scenario -eq 'saves_traceback') { 'Traceback: block failed' } elseif ($scenario -eq 'saves_none') { '0' } else { '3' }) }
        '*GetStreamingPersistentMapName*' { return $(if ($scenario -eq 'wrong_map') { 'Ice_P WillowPlayerPawn' } else { 'Sanctuary_P WillowPlayerPawn' }) }
        '*cp_bindings*' {
            # The run writes to the player's files: a restore must undo all of this.
            Set-Content -LiteralPath (Join-Path $script:WillowDir 'SaveData/save.sav') 'changed by the run'
            Set-Content -LiteralPath (Join-Path $script:WillowDir 'SaveData/new.sav') 'added by the run'
            Remove-Item -LiteralPath (Join-Path $script:WillowDir 'Config/game.ini')
            if ($scenario -eq 'created_root') {
                New-Item -ItemType Directory -Force (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver') | Out-Null
                Set-Content -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver/store.txt') 'made during the run'
            }
            if ($scenario -eq 'backup_changed') {
                $saved = Get-ChildItem -LiteralPath $script:RG -Directory -Filter 'save-backup-*' | Select-Object -First 1
                Set-Content -LiteralPath (Join-Path $saved.FullName 'SaveData/save.sav') 'backup damaged'
            }
            if ($scenario -eq 'run_error' -or $scenario -eq 'hooks_traceback') { throw 'channel lost' }
            return '[{"key": "V"}]'
        }
        '*cp_melee_facts*' { return '{"definition": {}}' }
        '*cp_page*' {
            # An old stage passed on the key "movie" alone, or on eight rows. The real read: open, and the screen the key left it on.
            $script:StubPageAsks++
            if ($scenario -eq 'page_closed') { return '{"movie": "m", "open": false, "quick_screen": 0}' }
            if ($scenario -eq 'keys_no_effect') { return '{"movie": "m", "open": true, "quick_screen": 0}' }
            return "{`"movie`": `"m`", `"open`": true, `"quick_screen`": $script:StubPageAsks}"
        }
        '*cp_count*' {
            # Each ask after a V sees one more call, unless the press never reached the game.
            if ($scenario -eq 'no_melee' -or ($scenario -eq 'no_npc_melee' -and $script:StubMeleeCalls -ge 3)) { return "$script:StubMeleeCalls" }
            $script:StubMeleeCalls++
            return "$script:StubMeleeCalls"
        }
        '*cp_injure*' { return $(if ($scenario -eq 'not_down') { '{"injured": false}' } else { '{"injured": true}' }) }
        '*cp_melee_hooks*' { return '{}' }
        '*cp_targets*' { return '[{"class": "WillowAIPawn", "path": "a.b"}]' }
        '*cp_prompt_read*' { return '{}' }
        '*quit*' { if ($scenario -ne 'hung') { $script:Fake.HasExited = $true }; return '' }
        default { return 'None' }
    }
}
if ($scenario -eq 'bad_snapshot') {
    function Assert-SnapshotMatches([string]$Live, [string]$Snapshot) { if ($Snapshot -like '*Config') { throw 'snapshot differs' }; Get-TreeHashes $Snapshot }
}
if ($scenario -eq 'mismatch') { function Restore-Tree { [pscustomobject]@{ file = 'x'; status = 'MISMATCH after restore' } } }
if ($scenario -eq 'restore_throws') { function Restore-Tree { throw 'disk full' } }
"""


class ControlsCaptureTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.work = Path(self.tmp.name)
        self.game = self.work / 'game'
        (self.game / 'sdk_mods/settings').mkdir(parents=True)
        (self.game / 'sdk_mods/settings/mods.json').write_text('settings', encoding='utf-8')
        for name, text in (('SaveData/save.sav', 'original save'), ('Config/game.ini', 'original config')):
            path = self.work / 'willow' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding='utf-8')
        self.hub = self.work / 'hub.lock'

    def run_capture(self, scenario):
        stub = self.work / 'stub.ps1'
        stub.write_text(STUB.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__WORK__', str(self.work)),
                        encoding='utf-8-sig')
        env = dict(os.environ, OPENWILLOW_BL2=str(self.game), OW_SCENARIO=scenario)
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(CAPTURE),
                               '-HubLock', str(self.hub), '-Helpers', str(stub)],
                              capture_output=True, text=True, env=env, timeout=120)
        session = json.loads((self.work / 'rg/controls/session.json').read_text(encoding='utf-8-sig'))
        return done, session

    def restore_by_hand(self):
        """Runs the printed recovery command in a new PowerShell, with the real helpers, as the maintainer would."""
        script = self.work / 'by_hand.ps1'
        script.write_text(f". '{ROOT / 'tools/real_game/realgame.ps1'}'\nRestore-VerifiedBackup '{self.backups()[0]}'\n", encoding='utf-8-sig')
        env = dict(os.environ, OPENWILLOW_BL2=str(self.game))
        return subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)],
                              capture_output=True, text=True, env=env, timeout=120)

    def saves(self):
        return (self.work / 'willow/SaveData/save.sav').read_text(encoding='utf-8').strip()

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
        self.assertFalse((self.game / 'sdk_mods/openwillow_realgame').exists())
        self.assertEqual(self.locks_held(), (False, False))

    def test_a_missing_stage_is_incomplete_not_finished(self):
        done, session = self.run_capture('no_melee')
        self.assertTrue(session['status'].startswith('incomplete: melee_air'), session['status'])
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.saves(), 'original save')

    def test_a_page_that_never_opened_fails_its_stages(self):
        done, session = self.run_capture('page_closed')
        for stage in ('melee_interrupted', 'page_keys'):
            self.assertIn(stage, session['status'])
        self.assertEqual(done.returncode, 1)

    def test_keys_that_change_nothing_fail_page_keys(self):
        done, session = self.run_capture('keys_no_effect')
        self.assertEqual(session['status'], 'incomplete: page_keys')

    def test_a_press_that_never_reached_a_pawn_fails_melee_npc(self):
        done, session = self.run_capture('no_npc_melee')
        self.assertIn('melee_npc', session['status'])
        self.assertTrue(any('melee_npc' in p and 'melee calls: 0' in p for p in session['stage_problems']), session['stage_problems'])

    def test_another_map_stops_the_run(self):
        done, session = self.run_capture('wrong_map')
        self.assertTrue(session['status'].startswith('failed: not in Sanctuary'), session['status'])
        self.assertEqual(self.saves(), 'original save')

    def test_saves_not_blocked_stops_the_run(self):
        for scenario in ('saves_traceback', 'saves_none'):
            done, session = self.run_capture(scenario)
            self.assertTrue(session['status'].startswith('failed: saves are not blocked'), session['status'])
            self.setUp()

    def test_no_focus_stops_the_run_and_still_restores(self):
        done, session = self.run_capture('no_focus')
        self.assertTrue(session['status'].startswith('failed: refusing to send input'), session['status'])
        self.assertEqual(session['cleanup']['driver'], 'removed')
        self.assertEqual(self.saves(), 'original save')
        self.assertEqual(self.locks_held(), (False, False))

    def test_a_game_already_running_stops_the_launch_and_touches_nothing(self):
        done, session = self.run_capture('foreign_before')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertIn('did not start', session['run_error'])
        self.assertIn('NOT RESTORED', session['cleanup']['saves'])
        self.assertTrue((self.game / 'sdk_mods/openwillow_realgame').exists())
        self.assertEqual(self.locks_held(), (True, True))
        self.assertIn('MANUAL RECOVERY', done.stdout)

    def test_a_game_seen_during_the_run_blocks_the_restore(self):
        done, session = self.run_capture('foreign_during')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertEqual(self.saves(), 'changed by the run')
        self.assertEqual(self.locks_held(), (True, True))

    def test_a_failed_install_restores_nothing_and_reports_the_folder(self):
        done, session = self.run_capture('install_fails')
        self.assertTrue(session['status'].startswith('failed: install broke'), session['status'])
        self.assertEqual(session['cleanup']['driver'], 'install failed; nothing left')
        self.assertEqual(session['cleanup']['saves'], 'not touched: this run launched no game')
        self.assertEqual(self.locks_held(), (False, False))

    def test_hooks_answer_must_be_exact(self):
        done, session = self.run_capture('hooks_traceback')
        self.assertIn('not stopped: Traceback', session['cleanup'].get('hooks', ''), session['cleanup'])

    def test_hooks_are_stopped_when_a_run_fails_with_the_game_alive(self):
        done, session = self.run_capture('run_error')
        self.assertEqual(session['cleanup']['hooks'], 'stopped')

    def test_missing_evidence_cannot_read_as_completed(self):
        done, session = self.run_capture('evidence_fails')
        self.assertTrue(session['status'].startswith('failed: evidence not written'), session['status'])
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.saves(), 'original save')
        self.assertEqual(self.locks_held(), (False, False))

    def test_an_earlier_runs_evidence_is_kept_under_a_new_name(self):
        out = self.work / 'rg/controls'
        out.mkdir(parents=True)
        (out / 'session.json').write_text('{"status": "completed", "earlier": true}', encoding='utf-8')
        done, session = self.run_capture('no_melee')
        self.assertNotIn('earlier', session)
        kept = list(out.glob('session.*.json'))
        self.assertEqual(len(kept), 1, kept)
        self.assertIn('earlier', kept[0].read_text(encoding='utf-8'))

    def test_the_tail_starts_as_the_shared_test_expects_and_matches_the_pattern(self):
        text = CAPTURE.read_text(encoding='utf-8')
        self.assertEqual(text.count('\n} catch {\n    $runError'.replace('\n', '\r\n' if '\r\n' in text else '\n')), 1)
        self.assertIn('Get-RecoveryReason $gameGone $cleanup.game $restored $restoreError $cleanup.driver $untracked', text)
        self.assertNotIn('Focus-Game', text)
        self.assertNotIn('Start-Process', text)   # a game only comes from Start-Game

    def test_maya_not_going_down_is_incomplete(self):
        done, session = self.run_capture('not_down')
        self.assertEqual(session['status'], 'incomplete: page_down')
        self.assertEqual(done.returncode, 1)

    def test_a_run_error_is_failed_and_still_restores(self):
        done, session = self.run_capture('run_error')
        self.assertEqual(session['status'], 'failed: channel lost')
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.saves(), 'original save')
        self.assertEqual(self.locks_held(), (False, False))

    def test_an_unverified_snapshot_is_never_restored_from(self):
        done, session = self.run_capture('bad_snapshot')
        self.assertTrue(session['status'].startswith('failed'), session['status'])
        self.assertEqual(session['cleanup']['saves'], 'not touched: this run launched no game')
        self.assertTrue((self.work / 'willow/Config/game.ini').exists())
        self.assertEqual(self.saves(), 'original save')
        self.assertEqual(self.locks_held(), (False, False))

    def test_a_root_created_during_the_run_is_named_and_kept(self):
        done, session = self.run_capture('created_root')
        self.assertIn('NOT IDENTICAL', session['cleanup']['saves'])
        self.assertIn('SanitySaver created during the run', session['cleanup']['saves'])
        self.assertTrue((self.game / 'sdk_mods/SanitySaver/store.txt').exists())
        self.assertEqual(self.saves(), 'original save')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertEqual(self.locks_held(), (True, True))
        self.assertIn('MANUAL RECOVERY', done.stdout)

    def test_a_restore_mismatch_keeps_locks_and_backup_and_prints_recovery(self):
        done, session = self.run_capture('mismatch')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertEqual(session['run_error'], '')
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.locks_held(), (True, True))
        self.assertTrue(self.backups())
        self.assertIn('MANUAL RECOVERY', done.stdout)
        self.assertIn(f"Restore-VerifiedBackup '{self.backups()[0]}'", done.stdout)
        self.assertNotIn('Restore-Tree', done.stdout)
        self.assertIn('MISMATCH after restore', done.stdout)   # the rows reach the screen too (they vanish as objects before `exit`)
        self.assertEqual(self.saves(), 'changed by the run')
        hand = self.restore_by_hand()
        self.assertIn('identical', hand.stdout, hand.stdout + hand.stderr)
        self.assertEqual(self.saves(), 'original save')
        self.assertTrue((self.work / 'willow/Config/game.ini').exists())

    def test_a_restore_that_throws_keeps_locks_and_prints_recovery(self):
        done, session = self.run_capture('restore_throws')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertEqual(self.locks_held(), (True, True))
        self.assertTrue(self.backups())
        self.assertIn('MANUAL RECOVERY', done.stdout)

    def test_a_backup_that_changed_is_not_restored_from(self):
        done, session = self.run_capture('backup_changed')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertEqual(self.saves(), 'changed by the run')
        self.assertTrue((self.work / 'willow/Config/game.ini').exists())   # the other roots are still restored
        self.assertEqual(self.locks_held(), (True, True))
        self.assertTrue(self.backups())
        self.assertIn('BACKUP CHANGED', done.stdout)
        self.assertIn('Restore-VerifiedBackup', done.stdout)
        hand = self.restore_by_hand()   # the command itself refuses the changed root, by hand as well
        self.assertIn('BACKUP CHANGED', hand.stdout, hand.stdout + hand.stderr)
        self.assertEqual(self.saves(), 'changed by the run')
        self.assertEqual(done.returncode, 1)

    def test_a_driver_that_stays_is_reported_as_recovery(self):
        done, session = self.run_capture('driver_stuck')
        self.assertTrue(session['status'].startswith('recovery needed: driver not removed'), session['status'])
        self.assertEqual(self.locks_held(), (True, True))
        self.assertIn('MANUAL RECOVERY', done.stdout)
        self.assertEqual(session['run_error'], '')
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.saves(), 'original save')

    def test_a_game_that_will_not_exit_touches_nothing(self):
        done, session = self.run_capture('hung')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertEqual(self.saves(), 'changed by the run')
        self.assertTrue((self.game / 'sdk_mods/openwillow_realgame').exists())
        self.assertIn(f"Restore-VerifiedBackup '{self.backups()[0]}'", done.stdout)
        self.assertTrue((self.backups()[0] / 'verified_hashes.json').exists())   # written before the game started
        self.assertEqual(self.locks_held(), (True, True))
        self.assertTrue(self.backups())
        self.assertIn('MANUAL RECOVERY', done.stdout)

    def test_a_held_hub_lock_stops_before_any_backup(self):
        self.hub.write_text('someone 2026', encoding='utf-8')
        done, session = self.run_capture('ok')
        self.assertTrue(session['status'].startswith('failed'), session['status'])
        self.assertIn('not waiting', session['status'])
        self.assertEqual(self.backups(), [])
        self.assertEqual(self.hub.read_text(encoding='utf-8'), 'someone 2026')
        self.assertFalse((self.work / 'run.lock').exists())


if __name__ == '__main__':
    unittest.main()
