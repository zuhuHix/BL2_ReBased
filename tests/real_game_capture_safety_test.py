"""Runs each capture script whole with the game stubbed (no game, no game files; temporary folders hold invented text): what it does
to the saves, the driver and the locks when the run goes well and when it does not. Needs Windows PowerShell.

The real tools/real_game/realgame.ps1 is loaded, so its backup and restore helpers are the ones under test. Only the parts that touch
the game, the screen and the machine-wide lock are replaced.

Run: python tests/real_game_capture_safety_test.py
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
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
function Install-Driver { New-Item -ItemType Directory -Force $script:DriverDir | Out-Null }
function Remove-Driver { Remove-Item -Recurse -Force $script:DriverDir }
function Get-GameProcess { $null }
function Focus-Game { $env:OW_TEST_SCENARIO -ne 'focus_lost' }
function Send-Key { }
function Send-KeyDown { }
function Send-KeyUp { }
function Save-Shot([string]$Name) { $f = Join-Path $script:RG $Name; New-Item -ItemType Directory -Force (Split-Path $f) | Out-Null; Set-Content -LiteralPath $f 'frame'; $f }
function Invoke-Burst { param($Folder, $Seconds, $IntervalMs, $Trigger, $Scale) if ($Trigger) { & $Trigger }; [pscustomobject]@{ frames = 3 } }
function Stop-OwnGame { if ($scenario -eq 'game_stuck') { 'refused: test' } else { $global:OwGameGone = $true; 'stopped' } }
function Invoke-GamePyFile { param($Path, $OutFolder) 'loaded' }
# What a session does to the player's files, whatever the run does about it.
function Start-Game {
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
    if ($Code -match 'block_saves') { return 'True' }
    if ($Code -match 'GetStreamingPersistentMapName') { return 'Sanctuary_P WillowPlayerPawn' }
    if ($Code -match 'lc_discover') {
        $count = if ($scenario -eq 'type_missing') { 0 } else { 1 }
        return (@{ Skel_DahlChest_Ammo = 1; Skel_Dumpster = 1; Skel_StorageLocker = 1; Skel_StrongBox = 1; Skel_CardboardBox = 1; Skel_LaundryMachine = $count } | ConvertTo-Json -Compress)
    }
    if ($Code -match 'lc_materials') { Set-Content -LiteralPath (Join-Path $script:RG 'loot_capture/materials.json') '{}'; return '{"slots": {}}' }
    if ($Code -match 'lc_lower_ammo') { return '{"before": {"a": 10}, "after": {"a": 5}}' }
    if ($Code -match 'lc_stand_pickup') { return '{"pickup": "p"}' }
    if ($Code -match 'lc_pickup_state') { return '{"gone": true}' }
    if ($Code -match 'lc_stand') {
        if ($scenario -eq 'script_error') { return "Traceback (most recent call last):`n LookupError" }
        return '{}'
    }
    if ($Code -match 'lc_after|lc_side') { return '{}' }
    if ($Code -match 'vp_dump') { if ($scenario -eq 'script_error') { return "Traceback (most recent call last):`n LookupError" }; return '7 [10, 12]' }
    if ($Code -match 'quit') { if ($scenario -ne 'game_stuck') { $global:OwGameGone = $true }; return '' }
    throw "unplanned game call: $Code"
}
"""


def run_script(script, scenario):
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
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(tools / f'{script}.ps1'), '-HubLock', str(hub),
                               '-Character', 'test'], capture_output=True, text=True, timeout=180,
                              env=dict(os.environ, OPENWILLOW_BL2=str(game), OW_TEST_SCENARIO=scenario))
        session_path = tmp / 'realgame' / SCRIPTS[script] / 'session.json'
        session = json.loads(session_path.read_text(encoding='utf-8-sig')) if session_path.exists() else {}
        contents = {path.name: path.read_text(encoding='utf-8-sig').strip() if path.exists() else None for path in files}
        backups = sorted(p.name for p in (tmp / 'realgame').glob('save-backup-*'))
        return {'code': done.returncode, 'out': done.stdout + done.stderr, 'session': session, 'files': contents, 'hub': hub.exists(),
                'run_lock': (tmp / 'run.lock').exists(), 'driver': (game / 'sdk_mods/openwillow_realgame').exists(), 'backups': backups}


ORIGINAL = {'slot.sav': 'original save', 'game.ini': 'original config', 'python-sdk.json': 'original settings', 'store.sav': 'original store'}
CHANGED = {name: 'changed by the session' for name in ORIGINAL}


class CaptureSafetyBase:
    script = None

    def run_it(self, scenario):
        return run_script(self.script, scenario)

    def test_the_cleanup_tail_is_the_reviewed_one_word_for_word(self):
        def tail(path):
            text = path.read_text(encoding='utf-8').replace('\r\n', '\n')
            return text[text.index('\n} catch {\n    $runError'):]
        self.assertEqual(tail(ROOT / f'tools/real_game/{self.script}.ps1'), tail(REFERENCE))

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
        self.assertEqual(r['session']['status'], 'incomplete: Skel_LaundryMachine', r['out'])
        self.assertEqual(r['files'], ORIGINAL)
        self.assertEqual((r['hub'], r['run_lock']), (False, False), r['out'])

    def test_a_failed_game_script_stops_that_type_before_its_use_key(self):
        r = self.run_it('script_error')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('incomplete: Skel_'), r['out'])
        for row in r['session']['types'].values():   # a string: Capture-Type stopped at lc_stand, before any burst or key
            self.assertTrue(str(row).startswith('failed: game script failed'), r['out'])
        self.assertEqual(r['files'], ORIGINAL)

    def test_a_lost_focus_stops_the_run_and_still_restores(self):
        r = self.run_it('focus_lost')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertTrue(r['session']['status'].startswith('failed:') and 'focus' in r['session']['status'], r['out'])
        self.assertEqual(r['files'], ORIGINAL)
        self.assertEqual((r['hub'], r['run_lock'], r['driver']), (False, False, False), r['out'])


class VendorsCaptureSafetyTest(CaptureSafetyBase, unittest.TestCase):
    script = 'vendors_capture'

    def test_an_exception_as_the_dumps_reply_is_not_a_dump(self):
        r = self.run_it('script_error')
        self.assertEqual(r['code'], 1, r['out'])
        self.assertEqual(r['session']['status'], 'incomplete: dump', r['out'])
        self.assertEqual(r['files'], ORIGINAL)


if __name__ == '__main__':
    unittest.main()
