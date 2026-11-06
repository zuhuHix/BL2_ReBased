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
$script:RG = Join-Path $work 'rg'
$script:CmdDir = Join-Path $script:RG 'cmd'
$script:WillowDir = Join-Path $work 'willow'
$script:Game = $env:OPENWILLOW_BL2
$script:DriverDir = Join-Path $script:Game 'sdk_mods/openwillow_realgame'
$script:Lock = Join-Path $work 'run.lock'
New-Item -ItemType Directory -Force $script:CmdDir | Out-Null
function Start-Sleep { }
function Get-GameProcess { $null }
function Enter-RunLock { Set-Content -LiteralPath $script:Lock 'held' }
function Exit-RunLock { Remove-Item -LiteralPath $script:Lock -Force }
function Install-Driver { New-Item -ItemType Directory -Force $script:DriverDir | Out-Null }
function Remove-Driver { if ($scenario -ne 'driver_stuck') { Remove-Item -Recurse -Force $script:DriverDir } }
function Save-Shot { 'shot' }
function Focus-Game { $true }
function Send-Key { }
function Invoke-GamePyFile { 'ok' }
function Stop-OwnGame { if ($scenario -eq 'hung') { 'refused: stub' } else { $script:Fake.HasExited = $true; 'stopped' } }
$script:Fake = [pscustomobject]@{ Id = 4242; StartTime = (Get-Date); HasExited = $false }
$script:Fake | Add-Member ScriptMethod WaitForExit { param($ms) $this.HasExited } | Out-Null
$script:Fake | Add-Member ScriptMethod Refresh { } | Out-Null
function Start-Game { $script:Fake }
function Invoke-GamePy([string]$Code, [int]$TimeoutSeconds = 30) {
    switch -Wildcard ($Code) {
        '*block_saves*' { return 'True' }
        '*GetStreamingPersistentMapName*' { return 'Sanctuary_P WillowPlayerPawn' }
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
            if ($scenario -eq 'run_error') { throw 'channel lost' }
            return '[{"key": "V"}]'
        }
        '*cp_melee_facts*' { return '{"definition": {}}' }
        '*cp_page*' { return '{"movie": "m"}' }
        '*cp_injure*' { return $(if ($scenario -eq 'not_down') { '{"injured": false}' } else { '{"injured": true}' }) }
        '*cp_melee_hooks*' { return '{}' }
        '*endswith*' { return $(if ($scenario -eq 'no_melee') { '0' } else { '2' }) }
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
        self.assertEqual(session['status'], 'incomplete: melee_air')
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.saves(), 'original save')

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
        self.assertIn('NOT IDENTICAL', session['cleanup']['saves'])
        self.assertIn('Config not backed up', session['cleanup']['saves'])
        self.assertTrue((self.work / 'willow/Config/game.ini').exists())

    def test_a_root_created_during_the_run_is_named_and_kept(self):
        done, session = self.run_capture('created_root')
        self.assertIn('NOT IDENTICAL', session['cleanup']['saves'])
        self.assertIn('SanitySaver created during the run', session['cleanup']['saves'])
        self.assertTrue((self.game / 'sdk_mods/SanitySaver/store.txt').exists())
        self.assertEqual(self.saves(), 'original save')

    def test_a_restore_mismatch_keeps_locks_and_backup_and_prints_recovery(self):
        done, session = self.run_capture('mismatch')
        self.assertTrue(session['status'].startswith('recovery needed'), session['status'])
        self.assertEqual(session['run_status'], 'completed')
        self.assertEqual(done.returncode, 1)
        self.assertEqual(self.locks_held(), (True, True))
        self.assertTrue(self.backups())
        self.assertIn('MANUAL RECOVERY', done.stdout)
        self.assertIn('Restore-Tree', done.stdout)
        self.assertIn('MISMATCH after restore', done.stdout)   # the rows reach the screen too (they vanish as objects before `exit`)

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
        self.assertIn('Do NOT restore SaveData', done.stdout)
        commands = [line for line in done.stdout.splitlines() if 'Restore-Tree' in line]
        self.assertFalse([line for line in commands if line.rstrip().endswith("'SaveData'")], commands)   # recovery must not offer the changed root
        self.assertTrue([line for line in commands if line.rstrip().endswith("'Config'")], commands)
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
        self.assertTrue((self.game / 'sdk_mods/openwillow_realgame').exists())
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
