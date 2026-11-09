"""The two capture finalizers (projectiles_capture.ps1, weapon_parity2_capture.ps1) re-check each backup before they restore from it.

No game, no game files: invented text in temporary folders. The test runs each script's own backup section and its own finally block (the
cleanup), lifted out of the script text, with the game and the locks stubbed. Between the backup and the cleanup a backup folder is changed
(or not). Needs Windows PowerShell.

Run: python tests/real_game_finalizer_recheck_test.py
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import realgame_test_env  # noqa: F401  (a fresh pending-record folder for every PowerShell run that does not set its own)

ROOT = Path(__file__).resolve().parents[1]
R = ROOT / 'tools' / 'real_game'
REALGAME = R / 'realgame.ps1'
SCRIPTS = ('projectiles_capture', 'weapon_parity2_capture')

FILES = {'willow/SaveData/Save0001.sav': 'original save', 'willow/Config/game.ini': 'original config',
         'game/sdk_mods/settings/SomeMod.json': '{"enabled": false}'}

BODY = r"""
$ErrorActionPreference = 'Stop'
. '__REALGAME__'
$tmp = '__TMP__'
$script:RG = Join-Path $tmp 'realgame'
$script:WillowDir = Join-Path $tmp 'willow'   # synthetic: test mode refuses the real save folders
$script:Game = Join-Path $tmp 'game'
$script:DriverDir = Join-Path $script:Game 'sdk_mods/openwillow_realgame'
function Get-GameProcesses { @() }   # no game runs in this test
function Get-GameProcess { $null }
$result = [ordered]@{}
$runId = 'recheck-test'
$Out = Join-Path $tmp 'out'; New-Item -ItemType Directory -Force $Out | Out-Null
$HubLock = Join-Path $tmp 'hub.lock'; Set-Content -LiteralPath $HubLock 'test run'
$Lane = 'test'; $log = [ordered]@{}
__ROOTS__
$rootState = [ordered]@{}; $backupHashes = @{}; $proc = $null; $ownerFile = $null; $runLock = $false; $hub = 'held'
# The script's own backup section: every root copied, verified, and the backup's record written.
__BACKUP__
# The game played on after the backup: its progress is in the live saves now, and the backup is not that.
Set-Content -LiteralPath (Join-Path $script:WillowDir 'SaveData/Save0001.sav') 'played since' -NoNewline
Set-Content -LiteralPath (Join-Path $script:WillowDir 'Config/game.ini') 'played config' -NoNewline
__TAMPER__
# The script's own finally block: the cleanup that restores from the backup.
__FINAL__
$result['saves'] = $cleanup.saves; $result['pending'] = $cleanup.pending
$result['rows'] = @($rows | ForEach-Object { "$($_.file): $($_.status)" })
$result['save'] = [IO.File]::ReadAllText((Join-Path $script:WillowDir 'SaveData/Save0001.sav'))
$result['config'] = [IO.File]::ReadAllText((Join-Path $script:WillowDir 'Config/game.ini'))
$result['hub_lock_kept'] = Test-Path -LiteralPath $HubLock
$result['records'] = @(Get-ChildItem -LiteralPath $env:OW_TEST_PENDING_DIR -Filter '*pending*.json' -File).Count
$result | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $tmp 'out.json') -Encoding UTF8
"""

TAMPER = "Set-Content -LiteralPath (Join-Path $bak 'SaveData/Save0001.sav') 'tampered' -NoNewline   # the backup changed after it was verified"


def lifted(script):
    """The roots, the backup section and the finally block of one capture script, as its own text."""
    text = (R / f'{script}.ps1').read_text(encoding='utf-8').replace('\r\n', '\n')
    roots_at = text.index('$roots = [ordered]@{')
    roots = text[roots_at:text.index('\n', text.index('sdk_settings = ', roots_at)) + 1]
    start = text.index('    $bak = Join-Path $script:RG "save-backup-$runId"')
    end_marker = '    Save-VerifiedBackupRecord $bak $roots $snapshots\n'
    backup = text[start:text.index(end_marker, start) + len(end_marker)]
    marker = '\n} finally {\n'
    final = text[text.index(marker) + len(marker):].rstrip()
    assert final.endswith('}'), script
    return roots, backup, final[:-1]


def run_cleanup(script, tamper):
    roots, backup, final = lifted(script)
    with tempfile.TemporaryDirectory(prefix='ow-recheck-') as tmp, tempfile.TemporaryDirectory(prefix='ow-recheck-pending-') as pending:
        tmp_path = Path(tmp)
        for rel, text in FILES.items():
            path = tmp_path / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding='utf-8')
        body = (BODY.replace('__REALGAME__', str(REALGAME)).replace('__TMP__', tmp).replace('__ROOTS__', roots)
                .replace('__BACKUP__', backup).replace('__TAMPER__', tamper).replace('__FINAL__', final))
        path = tmp_path / 'body.ps1'
        path.write_text(body, encoding='utf-8-sig')
        env = dict(os.environ, OW_TEST_PENDING_DIR=pending, OW_TEST_MODE='synthetic-real-game', OPENWILLOW_BL2=str(tmp_path / 'game'))
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(path)], capture_output=True, text=True,
                              timeout=300, env=env)
        out_file = tmp_path / 'out.json'
        if done.returncode != 0 or not out_file.exists():
            raise AssertionError(done.stdout + done.stderr)
        return json.loads(out_file.read_text(encoding='utf-8-sig'))


class FinalizerRecheckTest(unittest.TestCase):
    def test_a_backup_changed_since_verified_is_not_restored_and_the_record_and_locks_stay(self):
        for script in SCRIPTS:
            with self.subTest(script=script):
                out = run_cleanup(script, TAMPER)
                self.assertEqual(out['save'], 'played since', out)          # the live saves are untouched
                self.assertTrue(any(row.startswith('SaveData: BACKUP CHANGED after verification') for row in out['rows']), out)
                self.assertTrue(out['saves'].startswith('NOT IDENTICAL'), out)
                self.assertTrue(out['pending'].startswith('KEPT'), out)
                self.assertIn('SaveData: the backup changed after it was verified', out['pending'], out)
                self.assertEqual(out['records'], 1)                          # the record is kept
                self.assertTrue(out['hub_lock_kept'])                        # and the lock with it
                self.assertEqual(out['config'], 'original config')           # a root whose backup is intact is still restored

    def test_an_unchanged_backup_is_restored_and_the_record_is_removed(self):
        for script in SCRIPTS:
            with self.subTest(script=script):
                out = run_cleanup(script, '')
                self.assertEqual(out['save'], 'original save', out)
                self.assertEqual(out['config'], 'original config', out)
                self.assertTrue(out['saves'].startswith('identical'), out)
                self.assertTrue(out['pending'].startswith('removed'), out)
                self.assertEqual(out['records'], 0)
                self.assertFalse(out['hub_lock_kept'])

    def test_neither_finalizer_restores_with_restore_tree_directly(self):
        for script in SCRIPTS:
            text = (R / f'{script}.ps1').read_text(encoding='utf-8')
            self.assertNotIn('Restore-Tree $b $live $sub', text, script)
            self.assertIn('Restore-VerifiedRoots $bak @{ $sub = $live } @{ $sub = $backupHashes[$sub] }', text, script)


if __name__ == '__main__':
    unittest.main()
