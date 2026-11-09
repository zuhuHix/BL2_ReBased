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
# The run lock is held, as after Enter-RunLock; its release is stubbed and counted, so the test sees whether the finalizer releases it.
$global:runLockReleases = 0
function Exit-RunLock { $global:runLockReleases++ }
$rootState = [ordered]@{}; $backupHashes = @{}; $proc = $null; $ownerFile = $null; $runLock = $true; $hub = 'held'
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
$result['run_lock_releases'] = $global:runLockReleases
$result['sdk_setting_exists'] = Test-Path -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings/SomeMod.json')
$result['other_writer_file'] = if (Test-Path -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings/Other.json')) { [IO.File]::ReadAllText((Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings/Other.json')) } else { 'MISSING' }
$result['records'] = @(Get-ChildItem -LiteralPath $env:OW_TEST_PENDING_DIR -Filter '*pending*.json' -File).Count
$result | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $tmp 'out.json') -Encoding UTF8
"""

# The backup folder of a verified root is gone after verification. sdk_settings is used because the missing folder is the worst case for any
# root: Get-TreeHashes reads it as empty, and the restore would then delete every live file of it. Its live file is changed too, so a restore
# (or a delete) would show.
MISSING = ("Set-Content -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings/SomeMod.json') 'played since' -NoNewline; "
           "Remove-Item -LiteralPath (Join-Path $bak 'sdk_settings') -Recurse -Force   # the backup root is gone")
# A verified root that was empty: its snapshot is an empty table, so only the missing-folder check can tell gone from empty.
EMPTY_THEN_MISSING = ("Get-ChildItem -LiteralPath (Join-Path $bak 'sdk_settings') -File | Remove-Item -Force; "
                      "$backupHashes['sdk_settings'] = @{}; "
                      "Remove-Item -LiteralPath (Join-Path $bak 'sdk_settings') -Recurse -Force")

# The sdk_settings root did not exist at the backup (no file of it in FILES), and another writer creates it after the backup, before the cleanup.
ABSENT_THEN_CREATED = ("New-Item -ItemType Directory -Force (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings') | Out-Null; "
                       "Set-Content -LiteralPath (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings/Other.json') 'another writer' -NoNewline")
FILES_WITHOUT_SDK_SETTINGS = {k: v for k, v in FILES.items() if not k.startswith('game/sdk_mods/settings/')}

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


def run_cleanup(script, tamper, files=FILES):
    roots, backup, final = lifted(script)
    with tempfile.TemporaryDirectory(prefix='ow-recheck-') as tmp, tempfile.TemporaryDirectory(prefix='ow-recheck-pending-') as pending:
        tmp_path = Path(tmp)
        for rel, text in files.items():
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
                self.assertEqual(out['run_lock_releases'], 0, out)           # and the run lock: its release is skipped
                self.assertEqual(out['config'], 'original config')           # a root whose backup is intact is still restored

    def test_a_missing_backup_root_leaves_the_live_root_untouched_and_keeps_the_record(self):
        for script in SCRIPTS:
            for name, tamper in (('missing', MISSING), ('empty-then-missing', EMPTY_THEN_MISSING)):
                with self.subTest(script=script, case=name):
                    out = run_cleanup(script, tamper)
                    self.assertTrue(out['sdk_setting_exists'], out)               # nothing in the live root was deleted
                    self.assertTrue(any(row.startswith('sdk_settings: BACKUP CHANGED / MISSING') for row in out['rows']), out)
                    self.assertTrue(out['saves'].startswith('NOT IDENTICAL'), out)
                    self.assertTrue(out['pending'].startswith('KEPT'), out)
                    self.assertIn('sdk_settings: the backup changed after it was verified', out['pending'], out)
                    self.assertEqual(out['records'], 1)
                    self.assertTrue(out['hub_lock_kept'])
                    self.assertEqual(out['run_lock_releases'], 0, out)
                    self.assertEqual(out['save'], 'original save', out)        # the intact roots are still restored

    def test_an_absent_root_created_during_the_run_is_left_in_place_and_reported_like_the_shared_path(self):
        for script in SCRIPTS:
            with self.subTest(script=script):
                out = run_cleanup(script, ABSENT_THEN_CREATED, files=FILES_WITHOUT_SDK_SETTINGS)
                self.assertEqual(out['other_writer_file'], 'another writer', out)   # never removed: another writer's file stays
                self.assertIn('sdk_settings: created during the run; left for the maintainer', out['rows'], out)   # the shared helper's row
                self.assertNotIn('created during the run, removed', ' '.join(out['rows']), out)
                self.assertTrue(out['saves'].startswith('NOT IDENTICAL'), out)     # as Restore-VerifiedRoots' summary reads
                self.assertTrue(out['pending'].startswith('KEPT'), out)            # the record stays, as on the shared path
                self.assertEqual(out['records'], 1)
                self.assertTrue(out['hub_lock_kept'])
                self.assertEqual(out['run_lock_releases'], 0, out)
                self.assertEqual(out['save'], 'original save', out)                # the intact roots are still restored, as on the shared path
                self.assertEqual(out['config'], 'original config', out)

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
                self.assertEqual(out['run_lock_releases'], 1, out)           # a clean restore releases the run lock

    def test_neither_finalizer_restores_with_restore_tree_directly(self):
        for script in SCRIPTS:
            text = (R / f'{script}.ps1').read_text(encoding='utf-8')
            self.assertNotIn('Restore-Tree $b $live $sub', text, script)
            self.assertIn('Restore-VerifiedRoots $bak @{ $sub = $live } @{ $sub = $backupHashes[$sub] }', text, script)
            # The backup is hashed once, and that one table is both the comparison with the live tree and the baseline (review F1).
            self.assertIn('$diff = @(Compare-Hashes (Get-TreeHashes $live) $staged)', text, script)
            self.assertIn('$backupHashes[$sub] = $staged', text, script)
            self.assertNotIn('$backupHashes[$sub] = Get-TreeHashes', text, script)


class RestoreTreeTest(unittest.TestCase):
    def test_restore_tree_refuses_a_missing_backup_before_touching_the_live_folder(self):
        with tempfile.TemporaryDirectory(prefix='ow-recheck-') as tmp, tempfile.TemporaryDirectory(prefix='ow-recheck-pending-') as pending:
            live = Path(tmp) / 'live'
            live.mkdir()
            (live / 'keep.sav').write_text('mine', encoding='utf-8')
            body = Path(tmp) / 'body.ps1'
            body.write_text(f"""$ErrorActionPreference = 'Stop'
. '{REALGAME}'
try {{ Restore-Tree '{Path(tmp) / 'gone'}' '{live}' 'x' | Out-Null; 'NO THROW' }} catch {{ "threw: $_" }}
""", encoding='utf-8-sig')
            env = dict(os.environ, OW_TEST_PENDING_DIR=pending, OW_TEST_MODE='synthetic-real-game', OPENWILLOW_BL2=str(Path(tmp) / 'game'))
            done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(body)], capture_output=True, text=True,
                                  timeout=120, env=env)
            self.assertIn('threw: backup folder', done.stdout, done.stdout + done.stderr)
            self.assertEqual((live / 'keep.sav').read_text(encoding='utf-8'), 'mine')


if __name__ == '__main__':
    unittest.main()
