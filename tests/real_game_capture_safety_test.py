"""Runs each capture script whole with the game stubbed (no game, no game files; temporary folders hold invented text): what it does
to the saves, the driver and the locks when the run goes well and when it does not. Needs Windows PowerShell.

The real tools/real_game/realgame.ps1 is loaded, so its backup and restore helpers are the ones under test. Only the parts that touch
the game, the screen and the machine-wide lock are replaced.

Run: python tests/real_game_capture_safety_test.py
"""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import realgame_test_env  # noqa: F401  (a fresh pending-record folder for every PowerShell run)
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
REFERENCE = ROOT / 'tools/real_game/phaselock_integrity_capture.ps1'
SCRIPTS = {'loot_capture': 'loot_capture', 'vendors_capture': 'vendors'}

# Stands in for realgame.ps1 beside a copy of the capture script. Scenario: $env:OW_TEST_SCENARIO.
STUB = r"""
. '__REALGAME__'
$tmp = '__TMP__'
$script:RG = Join-Path $tmp 'realgame'
$script:CmdDir = Join-Path $script:RG 'cmd'
$script:WillowDir = Join-Path $tmp 'willow'
$script:Game = $env:OPENWILLOW_BL2
$script:DriverDir = Join-Path $script:Game 'sdk_mods/openwillow_realgame'
$script:Lock = Join-Path $tmp 'run.lock'
New-Item -ItemType Directory -Force $script:CmdDir | Out-Null
$scenario = $env:OW_TEST_SCENARIO
function Start-Sleep { }
function Enter-RunLock { Set-Content -LiteralPath $script:Lock 'test' }
function Exit-RunLock { Remove-Item -LiteralPath $script:Lock }
function Install-Driver {
    if ($scenario -eq 'install_leaves_folder') { New-Item -ItemType Directory -Force $script:DriverDir | Out-Null }
    if ($scenario -in 'install_fails', 'install_leaves_folder') { throw 'install failed' }
    New-Item -ItemType Directory -Force $script:DriverDir | Out-Null
}
function Remove-Driver { if ($scenario -ne 'driver_stuck') { Remove-Item -Recurse -Force $script:DriverDir } }
function Get-GameProcess { $null }
# Only a game this run did not start: the real helpers then decide what it means.
function Get-GameProcesses {
    if ($scenario -eq 'foreign_at_launch' -or ($scenario -eq 'foreign_later' -and $global:OwStarted)) {
        return @([pscustomobject]@{ Id = 77; Path = 'C:\elsewhere\Borderlands2.exe'; StartTime = Get-Date })
    }
    @()
}
function Focus-Game { $true }
function Assert-GameForeground { if ($scenario -eq 'focus_lost') { throw 'refusing to send input: Borderlands 2 is not the foreground window after 3 focus attempts (is someone using this PC?)' } }
function Send-Key { $global:OwUseKeySent = $true; Add-Content -LiteralPath '__TMP__/keys.log' "$args" }
function Send-KeyDown { }
function Send-KeyUp { }
function Save-Shot([string]$Name) { $f = Join-Path $script:RG $Name; New-Item -ItemType Directory -Force (Split-Path $f) | Out-Null; Set-Content -LiteralPath $f 'frame'; $f }
function Invoke-Burst { param($Folder, $Seconds, $IntervalMs, $Trigger, $Scale) Add-Content -LiteralPath '__TMP__/bursts.log' "$Folder"; if ($Trigger) { & $Trigger }; [pscustomobject]@{ frames = 3 } }
function Stop-OwnGame { if ($scenario -eq 'game_stuck') { 'refused: test' } else { $global:OwGameGone = $true; 'stopped' } }
function Invoke-GamePyFile { param($Path, $OutFolder) 'loaded' }
# What a session does to the player's files, whatever the run does about it.
function Start-Game {
    $global:OwStarted = $true
    if ($scenario -eq 'compare_unwritable') { New-Item -ItemType Directory -Force (Join-Path $Out 'save_compare.json') | Out-Null }
    if ($scenario -eq 'session_unwritable') { New-Item -ItemType Directory -Force (Join-Path $Out 'session.json') | Out-Null }
    Set-Content -LiteralPath (Join-Path $script:WillowDir 'SaveData/slot.sav') 'changed by the session'
    Set-Content -LiteralPath (Join-Path $script:WillowDir 'Config/game.ini') 'changed by the session'
    Set-Content -LiteralPath (Join-Path $script:Game 'sdk_mods/settings/python-sdk.json') 'changed by the session'
    Set-Content -LiteralPath (Join-Path $script:Game 'sdk_mods/SanitySaver/store.sav') 'changed by the session'
    if ($scenario -eq 'backup_changes') {
        Get-ChildItem (Join-Path $script:RG 'save-backup-*/SaveData/slot.sav') | Remove-Item
    }
    # Running until quit is answered or a stop is accepted; a stuck game does neither.
    $p = [pscustomobject]@{ Id = 1; StartTime = Get-Date }
    $p | Add-Member ScriptProperty HasExited { $global:OwGameGone -eq $true }
    $p | Add-Member ScriptMethod Refresh { }
    $p | Add-Member ScriptMethod WaitForExit { param($ms) $global:OwGameGone -eq $true }
    $p
}
function Invoke-GamePy([string]$Code, [int]$TimeoutSeconds = 30) {
    if ($Code -match 'print\(get_pc\(\)\)') { if ($scenario -eq 'no_answer') { throw 'no reply' }; return 'pc' }
    if ($Code -match 'block_saves') { if ($scenario -eq 'saves_not_blocked') { return 'True, but no hooks' }; return '4' }
    if ($Code -match 'GetStreamingPersistentMapName') { return 'Sanctuary_P WillowPlayerPawn' }
    if ($Code -match 'lc_discover') {
        $count = if ($scenario -eq 'type_missing') { 0 } else { 1 }
        return (@{ Skel_DahlChest_Ammo = 1; Skel_Dumpster = 1; Skel_StorageLocker = 1; Skel_StrongBox = 1; Skel_CardboardBox = 1; Skel_LaundryMachine = $count } | ConvertTo-Json -Compress)
    }
    if ($Code -match 'lc_materials') { Set-Content -LiteralPath (Join-Path $script:RG 'loot_capture/materials.json') '{}'; if ($scenario -eq 'materials_unread') { return '{"slots": {}, "problems": ["Skel_X has no readable material slot"]}' }; return '{"slots": {}, "problems": []}' }
    if ($Code -match 'lc_lower_ammo') {
        if ($scenario -eq 'ammo_not_lowered') { return '{"before": {"a": 10}, "after": {"a": 10}, "fallen": []}' }
        return '{"before": {"a": 10}, "after": {"a": 5}, "fallen": ["a"]}'
    }
    if ($Code -match 'lc_pickup_baseline') {
        $global:OwBaselines = 1 + [int]$global:OwBaselines
        $went = '{"present": false, "reason": "the chosen pickup went before this attempt"}'
        if ($scenario -eq 'chosen_pickup_went') { return $went }
        # Still there for the walk-over, gone by the use key.
        if ($scenario -eq 'pickup_went_after_the_walk_over' -and ($global:OwBaselines % 2) -eq 0) { return $went }
        return '{"present": true}'
    }
    if ($Code -match 'lc_mods') { return '[{"name": "SomeMod", "enabled": true}]' }
    if ($Code -match 'lc_choose_pickup') { $global:OwFellOn = $false; $global:OwUseKeySent = $false }
    if ($Code -match 'lc_over_pickup') { $global:OwFellOn = $true }
    if ($Code -match 'lc_choose_pickup|lc_stand_pickup|lc_over_pickup|lc_aim_pickup') { return '{"pickup": "p"}' }
    if ($Code -match 'lc_pickup_state') {
        if ($scenario -in 'pickup_gone_only', 'pickup_went_after_the_walk_over') { return '{"gone": true, "changed": false, "collected": false}' }
        # Collected only once the thing that collects it was really sent.
        $sent = if ($scenario -eq 'collected_by_use_key') { $global:OwUseKeySent } else { $global:OwFellOn }
        if (-not $sent) { return '{"gone": false, "changed": false, "collected": false}' }
        return '{"gone": true, "changed": true, "collected": true}'
    }
    if ($Code -match 'lc_opened') {
        $global:OwOpenReads = 1 + [int]$global:OwOpenReads
        if ($scenario -eq 'never_opens') { return '{"opened": false, "changed": {}}' }
        if ($scenario -eq 'opens_on_third_side' -and $global:OwOpenReads -lt 3) { return '{"opened": false, "changed": {}}' }
        return '{"opened": true, "changed": {"bCanBeUsed": [true, false]}}'
    }
    if ($Code -match 'lc_aim') { return '{}' }
    if ($Code -match 'lc_stand') {
        if ($scenario -eq 'script_error') { return "Traceback (most recent call last):`n LookupError" }
        return '{}'
    }
    if ($Code -match 'lc_side') { return '{}' }
    if ($Code -match 'vp_dump') { if ($scenario -eq 'script_error') { return "Traceback (most recent call last):`n LookupError" }; return '7 [10, 12]' }
    if ($Code -match 'quit') { if ($scenario -ne 'game_stuck') { $global:OwGameGone = $true }; return '' }
    throw "unplanned game call: $Code"
}
"""


def run_script(script, scenario, earlier_session=None):
    """Runs one copy of the script. Returns what a test needs, with the temporary tree still readable through a copy of its state."""
    with tempfile.TemporaryDirectory() as folder:
        tmp = Path(folder)
        tools = tmp / 'tools/real_game'
        tools.mkdir(parents=True)
        shutil.copy(ROOT / f'tools/real_game/{script}.ps1', tools)
        (tools / 'realgame.ps1').write_text(STUB.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__TMP__', folder), encoding='utf-8-sig')
        game = tmp / 'game'
        files = {tmp / 'willow/SaveData/slot.sav': 'original save', tmp / 'willow/Config/game.ini': 'original config',
                 game / 'sdk_mods/settings/python-sdk.json': 'original settings', game / 'sdk_mods/SanitySaver/store.sav': 'original store'}
        for path, text in files.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding='utf-8')
        hub = tmp / 'hub.lock'
        if earlier_session:
            (tmp / 'realgame' / SCRIPTS[script]).mkdir(parents=True)
            (tmp / 'realgame' / SCRIPTS[script] / 'session.json').write_text(earlier_session, encoding='utf-8')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(tools / f'{script}.ps1'), '-HubLock', str(hub),
                               '-Character', 'test'], capture_output=True, text=True, timeout=180,
                              env=dict(os.environ, OPENWILLOW_BL2=str(game), OW_TEST_SCENARIO=scenario))
        session_path = tmp / 'realgame' / SCRIPTS[script] / 'session.json'
        session = json.loads(session_path.read_text(encoding='utf-8-sig')) if session_path.is_file() else {}
        kept = sorted(p.name for p in (tmp / 'realgame' / SCRIPTS[script]).glob('session.*.json'))
        contents = {path.name: path.read_text(encoding='utf-8-sig').strip() if path.exists() else None for path in files}
        backups = sorted(p.name for p in (tmp / 'realgame').glob('save-backup-*'))
        def logged(name):
            path = tmp / name
            return path.read_text(encoding='utf-8').split() if path.exists() else []
        return {'code': done.returncode, 'out': done.stdout + done.stderr, 'session': session, 'files': contents, 'hub': hub.exists(),
                'keys': logged('keys.log'), 'bursts': logged('bursts.log'),
                'run_lock': (tmp / 'run.lock').exists(), 'driver': (game / 'sdk_mods/openwillow_realgame').exists(), 'backups': backups,
                'kept': kept, 'kept_text': [(tmp / 'realgame' / SCRIPTS[script] / name).read_text(encoding='utf-8') for name in kept]}


ORIGINAL = {'slot.sav': 'original save', 'game.ini': 'original config', 'python-sdk.json': 'original settings', 'store.sav': 'original store'}
CHANGED = {name: 'changed by the session' for name in ORIGINAL}


class CaptureSafetyBase:
    script = None

    def run_it(self, scenario, earlier_session=None):
        return run_script(self.script, scenario, earlier_session)

    def test_the_cleanup_tail_is_the_reviewed_one_word_for_word(self):
        def tail(path):
            text = path.read_text(encoding='utf-8').replace('\r\n', '\n')
            return text[text.index('\n} catch {\n    $runError'):]
        # All but phaselock's own per-frame hook, which these captures do not install.
        reviewed, found = re.subn(r'    # A throw between pi_start.*?\n    if \(\$proc -and !\$proc\.HasExited\) \{\n.*?\n    \}\n', '', tail(REFERENCE), flags=re.S)
        self.assertEqual(found, 1)
        self.assertEqual(tail(ROOT / f'tools/real_game/{self.script}.ps1'), reviewed)

    def test_saves_not_blocked_stops_before_any_gameplay_and_restores(self):
        r = self.run_it('saves_not_blocked')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('failed: saves are not blocked'), r['out'])
        self.assertEqual(r['files'], ORIGINAL)
        self.assertEqual((r['hub'], r['run_lock'], r['driver']), (False, False, False), r['out'])

    def test_a_driver_left_behind_holds_the_locks(self):
        r = self.run_it('driver_stuck')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('recovery needed: driver not removed'), r['out'])
        self.assertEqual(r['files'], ORIGINAL)   # the restore itself still ran
        self.assertEqual((r['hub'], r['run_lock'], r['driver']), (True, True, True), r['out'])
        self.assertIn('MANUAL RECOVERY', r['out'])

    def test_an_install_that_leaves_a_folder_holds_the_locks_and_does_not_remove_it(self):
        r = self.run_it('install_leaves_folder')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('recovery needed: driver not removed'), r['out'])
        self.assertEqual(r['session']['cleanup']['saves'], 'not touched: this run launched no game', r['out'])
        self.assertEqual((r['hub'], r['run_lock'], r['driver']), (True, True, True), r['out'])

    def test_a_game_already_running_stops_the_launch_and_nothing_is_removed_or_restored(self):
        r = self.run_it('foreign_at_launch')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertIn('did not start', r['session']['status'], r['out'])
        self.assertIn('NOT RESTORED', r['session']['cleanup']['saves'], r['out'])
        self.assertEqual(r['files'], ORIGINAL)   # no game of ours ever ran
        self.assertEqual((r['hub'], r['run_lock'], r['driver']), (True, True, True), r['out'])

    def test_a_game_that_appears_during_the_run_stops_the_restore(self):
        r = self.run_it('foreign_later')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertIn('did not start', r['session']['status'], r['out'])
        self.assertEqual(r['files'], CHANGED, r['out'])   # restoring could erase what that game saved
        self.assertEqual((r['hub'], r['run_lock'], r['driver']), (True, True, True), r['out'])
        self.assertIn('MANUAL RECOVERY', r['out'])

    def test_earlier_evidence_is_kept_under_a_new_name_not_overwritten_or_lost(self):
        r = self.run_it('no_answer', '{"status": "completed"}')
        self.assertEqual(r['session']['status'][:7], 'failed:', r['out'])
        self.assertEqual(r['kept_text'], ['{"status": "completed"}'], r['out'])

    def test_missing_evidence_cannot_read_as_completed(self):
        for scenario in ('compare_unwritable', 'session_unwritable'):
            with self.subTest(scenario):
                r = self.run_it(scenario)
                self.assertEqual(r['code'], 1, r['out'])
                self.assertIn('status: failed: evidence not written', r['out'])
                self.assertEqual(r['files'], ORIGINAL)   # the restore had already run
                self.assertEqual((r['hub'], r['run_lock'], r['driver']), (False, False, False), r['out'])

    def test_the_capture_declares_its_state_and_keeps_stale_evidence(self):
        text = (ROOT / f'tools/real_game/{self.script}.ps1').read_text(encoding='utf-8').replace('\r\n', '\n')
        order = [text.index(part) for part in ('# Stale evidence:', '$driverAttempted = $false', '$driverAttempted = $true', 'Install-Driver -BlockSavesAtStart',
                                               '$foreign = Get-ForeignGamesSeen $null', '$launchAttempted = $true', '$proc = Start-Game @(', 'Assert-SavesBlocked (Py')]
        self.assertEqual(order, sorted(order))
        self.assertNotIn('Focus-Game', text)

    def test_a_run_that_stops_before_the_driver_touches_nothing_and_releases(self):
        r = self.run_it('install_fails')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('failed: install failed'), r['out'])
        self.assertEqual(r['session']['cleanup']['saves'], 'not touched: this run launched no game', r['out'])
        self.assertEqual(r['session']['cleanup']['driver'], 'install failed; nothing left', r['out'])
        self.assertEqual(r['files'], ORIGINAL)
        self.assertEqual((r['hub'], r['run_lock'], r['driver']), (False, False, False), r['out'])

    def test_a_good_run_restores_everything_and_exits_0(self):
        r = self.run_it('good')
        self.assertEqual(r['code'], 0, r['out'])
        self.assertEqual(r['session']['status'], 'completed', r['out'])
        self.assertEqual(r['files'], ORIGINAL)   # the SDK's settings and Sanity Saver's store too
        self.assertEqual((r['hub'], r['run_lock'], r['driver']), (False, False, False), r['out'])

    def test_a_game_exit_not_confirmed_restores_nothing_and_keeps_the_locks(self):
        r = self.run_it('game_stuck')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('recovery needed: game exit'), r['out'])
        self.assertEqual(r['files'], CHANGED, r['out'])
        self.assertEqual((r['hub'], r['run_lock'], r['driver']), (True, True, True), r['out'])
        self.assertEqual(len(r['backups']), 1)
        self.assertIn('Restore-VerifiedBackup', r['out'])

    def test_a_restore_that_does_not_compare_clean_keeps_the_locks_and_the_backup(self):
        r = self.run_it('backup_changes')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('recovery needed: restore'), r['out'])
        self.assertEqual(r['files']['slot.sav'], 'changed by the session', r['out'])   # never restored from a backup that changed
        self.assertEqual((r['hub'], r['run_lock']), (True, True), r['out'])
        self.assertEqual(len(r['backups']), 1)
        self.assertIn('BACKUP CHANGED', r['out'])
        self.assertIn('Restore-VerifiedBackup', r['out'])

    def test_a_run_that_fails_restores_and_exits_1_without_finishing_as_completed(self):
        r = self.run_it('no_answer')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('failed: driver did not answer'), r['out'])
        self.assertEqual(r['files'], ORIGINAL)
        self.assertEqual((r['hub'], r['run_lock'], r['driver']), (False, False, False), r['out'])


class LootCaptureSafetyTest(CaptureSafetyBase, unittest.TestCase):
    script = 'loot_capture'

    def test_a_lootable_type_not_in_the_world_is_incomplete(self):
        r = self.run_it('type_missing')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('incomplete: Skel_LaundryMachine ('), r['out'])
        self.assertEqual(r['files'], ORIGINAL)
        self.assertEqual((r['hub'], r['run_lock']), (False, False), r['out'])

    def test_a_failed_game_script_stops_that_type_before_its_use_key(self):
        r = self.run_it('script_error')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('incomplete: Skel_'), r['out'])
        for row in r['session']['types'].values():   # a string: Capture-Type stopped at lc_stand, before any burst or key
            self.assertTrue(str(row).startswith('failed: game script failed'), r['out'])
        self.assertEqual([b for b in r['bursts'] if 'Skel_' in b], [], r['out'])
        self.assertNotIn('E', r['keys'], r['out'])
        self.assertEqual(r['files'], ORIGINAL)

    def test_a_setter_that_lowered_nothing_is_incomplete(self):
        r = self.run_it('ammo_not_lowered')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(any(p.startswith('ammo: no ammo pool fell') for p in r['session']['stage_problems']), r['out'])

    def test_a_chosen_pickup_gone_before_the_attempt_gets_no_attempt(self):
        r = self.run_it('chosen_pickup_went')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('incomplete: ammo_pickup, cash_pickup ('), r['out'])
        self.assertEqual(r['session']['pickups']['ammo']['method'], 'none', r['out'])
        self.assertEqual([b for b in r['bursts'] if 'ammo_' in b or 'cash_' in b], [], r['out'])

    def test_a_pickup_gone_after_the_walk_over_gets_no_use_key(self):
        r = self.run_it('pickup_went_after_the_walk_over')
        self.assertEqual(r['code'], 1, r['out'])
        row = r['session']['pickups']['ammo']
        self.assertEqual(row['method'], 'none', r['out'])
        self.assertTrue(row['result'].startswith('no use key'), r['out'])
        self.assertEqual([b for b in r['bursts'] if b.endswith('_use')], [], r['out'])

    def test_a_pickup_that_went_without_the_value_rising_is_incomplete(self):
        r = self.run_it('pickup_gone_only')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('incomplete: ammo_pickup, cash_pickup ('), r['out'])

    def test_frames_without_an_opening_do_not_complete_a_lootable(self):
        # The false pass of the first real run: all frames taken, three lootables never opened.
        r = self.run_it('never_opens')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertIn('incomplete: Skel_DahlChest_Ammo, Skel_Dumpster, Skel_StorageLocker, Skel_StrongBox, Skel_CardboardBox, Skel_LaundryMachine', r['session']['status'], r['out'])
        self.assertEqual(len(r['session']['types']['Skel_StrongBox']['tries']), 4, r['out'])   # every side was tried
        self.assertTrue(any('did not open from 4 side(s)' in p for p in r['session']['stage_problems']), r['out'])

    def test_it_stops_at_the_side_that_opens(self):
        r = self.run_it('opens_on_third_side')
        self.assertEqual(r['session']['status'], 'completed', r['out'])
        self.assertEqual([t['side'] for t in r['session']['types']['Skel_DahlChest_Ammo']['tries']], [0, 4, 2], r['out'])
        self.assertEqual(len(r['session']['types']['Skel_Dumpster']['tries']), 1, r['out'])

    def test_a_pickup_is_tried_by_touch_then_by_the_use_key(self):
        r = self.run_it('good')
        self.assertEqual(r['session']['pickups']['ammo']['method'], 'touch', r['out'])
        self.assertNotIn('use_burst', r['session']['pickups']['ammo'], r['out'])
        r = self.run_it('collected_by_use_key')
        self.assertEqual(r['code'], 0, r['out'])
        self.assertEqual(r['session']['pickups']['cash']['method'], 'use key', r['out'])

    def test_the_loaded_mods_are_in_the_record(self):
        r = self.run_it('good')
        self.assertIn('SomeMod', r['session']['sdk_mods'], r['out'])

    def test_a_materials_file_without_values_is_incomplete(self):
        r = self.run_it('materials_unread')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('incomplete: materials ('), r['out'])
        self.assertTrue(any('no readable material slot' in p for p in r['session']['stage_problems']), r['out'])

    def test_a_lost_focus_stops_the_run_and_still_restores(self):
        r = self.run_it('focus_lost')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('failed: refusing to send input'), r['out'])
        self.assertEqual(r['files'], ORIGINAL)
        self.assertEqual((r['hub'], r['run_lock'], r['driver']), (False, False, False), r['out'])


class VendorsCaptureSafetyTest(CaptureSafetyBase, unittest.TestCase):
    script = 'vendors_capture'

    def test_an_exception_as_the_dumps_reply_is_not_a_dump(self):
        r = self.run_it('script_error')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('incomplete: dump ('), r['out'])
        self.assertEqual(r['files'], ORIGINAL)


if __name__ == '__main__':
    unittest.main()
