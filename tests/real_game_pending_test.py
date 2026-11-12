"""Synthetic tests for the pending records and guarded input of tools/real_game (pending.ps1, realgame.ps1): no game, no game files, no
Steam. Temporary folders hold invented text; the pending folder is a temporary one (OW_TEST_PENDING_DIR), never %LOCALAPPDATA%\\OpenWillow.
The native key and mouse calls are replaced by a log. Needs Windows PowerShell.

Run: python tests/real_game_pending_test.py
"""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

import realgame_test_env  # noqa: F401  (a fresh pending-record folder for every PowerShell run that does not set its own)
from real_game_capture_safety_test import STUB

ROOT = Path(__file__).resolve().parents[1]
REALGAME = ROOT / 'tools/real_game/realgame.ps1'

# Loaded first in every PowerShell body: the harness, pointed at the test's folders.
PRELUDE = r"""
$ErrorActionPreference = 'Stop'
. '__REALGAME__'
$tmp = '__TMP__'
$script:RG = Join-Path $tmp 'realgame'
$script:WillowDir = Join-Path $tmp 'willow'
$script:Game = Join-Path $tmp 'game'
$script:DriverDir = Join-Path $script:Game 'sdk_mods/openwillow_realgame'
function Get-GameProcesses { if (Test-Path -LiteralPath (Join-Path $tmp 'game_running')) { @([pscustomobject]@{ Id = 4242; Path = 'x'; StartTime = Get-Date; MainWindowHandle = [IntPtr]1 }) } else { @() } }
$out = [ordered]@{}
function Read-Text([string]$Path) { [IO.File]::ReadAllText($Path) }
function Fail-Text([scriptblock]$Block) { try { & $Block | Out-Null; '' } catch { "$_" } }
"""


def powershell(body, tmp, pending):
    script = Path(tmp) / 'body.ps1'
    script.write_text(PRELUDE.replace('__REALGAME__', str(REALGAME)).replace('__TMP__', str(tmp)) + body + "\n$out | ConvertTo-Json -Depth 6\n",
                      encoding='utf-8-sig')
    done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True,
                          timeout=180, env=dict(os.environ, OW_TEST_PENDING_DIR=str(pending), OPENWILLOW_BL2=str(Path(tmp) / 'game')))
    if done.returncode != 0:
        raise AssertionError(done.stdout + done.stderr)
    text = done.stdout
    return json.loads(text[text.index('{'):])


def make_tree(tmp):
    files = {'willow/SaveData/Save0001.sav': 'original save', 'willow/SaveData/profile.bin': 'original profile',
             'willow/Config/game.ini': 'original config', 'game/sdk_mods/settings/SomeMod.json': '{"enabled": true, "keys": {}}',
             'game/sdk_mods/settings/Other.json': '{"enabled": false}'}
    for rel, text in files.items():
        path = Path(tmp) / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding='utf-8')


class PendingBase(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix='ow-pending-test-'))
        self.pending = Path(tempfile.mkdtemp(prefix='ow-pending-dir-'))
        make_tree(self.tmp)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)
        shutil.rmtree(self.pending, ignore_errors=True)

    def run_ps(self, body):
        return powershell(body, self.tmp, self.pending)

    def records(self):
        return sorted(p.name for p in self.pending.glob('*pending*.json'))


class PendingRecordTest(PendingBase):
    def test_the_tree_hash_sees_every_change_and_nothing_else(self):
        out = self.run_ps(r"""
$d = Join-Path $tmp 'willow/SaveData'
$a = Get-TreeDigest $d; $out.same = ($a -eq (Get-TreeDigest $d))
Set-Content (Join-Path $d 'Save0001.sav') 'changed' -NoNewline; $out.changed = ($a -ne (Get-TreeDigest $d))
Set-Content (Join-Path $d 'Save0001.sav') 'original save' -NoNewline; $out.back = ($a -eq (Get-TreeDigest $d))
Set-Content (Join-Path $d 'new.sav') 'x'; $out.added = ($a -ne (Get-TreeDigest $d)); Remove-Item (Join-Path $d 'new.sav')
Rename-Item (Join-Path $d 'profile.bin') 'profile.bak'; $out.renamed = ($a -ne (Get-TreeDigest $d)); Rename-Item (Join-Path $d 'profile.bak') 'profile.bin'
$out.again = ($a -eq (Get-TreeDigest $d))
$out.absent = Get-TreeDigest (Join-Path $tmp 'nothing')
""")
        self.assertEqual(out, {'same': True, 'changed': True, 'back': True, 'added': True, 'renamed': True, 'again': True, 'absent': 'absent'})

    def test_backup_writes_the_record_first_and_a_verified_restore_removes_it(self):
        out = self.run_ps(r"""
$bak = Backup-Saves
$out.records = @(Get-PendingFiles | ForEach-Object { $_.Name })
$r = Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json
$out.kind = $r.kind; $out.pid_ok = ($r.pid -eq $PID); $out.backup_ok = ($r.backup -eq $bak); $out.roots = @($r.roots.PSObject.Properties.Name)
$out.digest_ok = ($r.roots.SaveData.tree_sha256 -eq (Get-TreeDigest (Join-Path $tmp 'willow/SaveData')))
$out.has_start = [bool]$r.process_start
Add-PendingChange 'test change'
$out.changes = @((Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json).changes | ForEach-Object { $_ -replace '^\S+ ', '' })
# The game wrote the saves: the record stays.
Set-Content (Join-Path $tmp 'willow/SaveData/Save0001.sav') 'written by the game'
Set-Content (Join-Path $tmp 'willow/SaveData/Save0001.sav.bak') 'new file'
$out.kept = Complete-PendingRecord $true
$out.unsafe = Complete-PendingRecord $false
New-Item -ItemType File (Join-Path $tmp 'game_running') | Out-Null
$out.game_running = Complete-PendingRecord $true
Remove-Item (Join-Path $tmp 'game_running')
$out.restore = (Restore-FromVerifiedRecord $bak).summary
$out.removed = Complete-PendingRecord $true
$out.after = @(Get-PendingFiles).Count
$out.save = Read-Text (Join-Path $tmp 'willow/SaveData/Save0001.sav')
$out.bak_file_gone = !(Test-Path (Join-Path $tmp 'willow/SaveData/Save0001.sav.bak'))
""")
        self.assertEqual(len(out['records']), 1)
        self.assertRegex(out['records'][0], r'^realgame_run_pending-body-\d+\.json$')
        self.assertEqual(out['kind'], 'openwillow-realgame-pending')
        self.assertTrue(out['pid_ok'] and out['backup_ok'] and out['digest_ok'] and out['has_start'])
        self.assertEqual(out['roots'], ['SaveData', 'Config', 'sdk_settings', 'SanitySaver'])
        self.assertEqual(out['changes'], ['test change'])
        self.assertIn('KEPT', out['kept']); self.assertIn('SaveData: differs from the backup', out['kept'])
        self.assertIn('SaveData: tree hash', out['kept'])
        self.assertIn('a game may still run', out['unsafe'])
        self.assertIn('Borderlands 2 is running', out['game_running'])
        self.assertTrue(out['removed'].startswith('removed: verified by tree hash'), out['removed'])
        self.assertEqual(out['after'], 0)
        self.assertEqual(out['save'].strip(), 'original save')
        self.assertTrue(out['bak_file_gone'])

    def test_a_rewrite_whose_replace_fails_keeps_the_old_record_and_its_tmp_stops_the_next_run(self):
        # Review of #245: a delete-then-move rewrite lost the old record when the move failed, and the .tmp left was found by no run.
        out = self.run_ps(r"""
$bak = Backup-Saves
$rec = $script:PendingRecord
$before = Read-Text $rec
# A scanner holds the new copy open the moment it is written, so the rewrite cannot complete.
function Set-Content {
    param([Parameter(ValueFromPipeline = $true)]$Value, [string]$LiteralPath, $Encoding)
    begin { $all = @() } process { $all += $Value }
    end {
        Microsoft.PowerShell.Management\Set-Content -LiteralPath $LiteralPath -Value $all -Encoding $Encoding
        if ($LiteralPath -like '*.tmp') { $script:held = [IO.File]::Open($LiteralPath, 'Open', 'Read', 'Read') }
    }
}
$out.rewrite = Fail-Text { Add-PendingChange 'settings rewritten' }
$script:held.Close()
Remove-Item Function:\Set-Content
$out.json_kept = Test-Path -LiteralPath $rec
$out.old_content = $out.json_kept -and ((Read-Text $rec) -eq $before)
$out.tmp_left = Test-Path -LiteralPath "$rec.tmp"
$out.found = @(Get-PendingFiles | ForEach-Object { $_.Name })
$script:PendingRecord = $null   # as the next run sees the folder
$out.next = Fail-Text { Assert-NoPendingRecords }
""")
        self.assertTrue(out['rewrite'])   # the rewrite reports its failure
        self.assertTrue(out['json_kept'] and out['old_content'], out)   # the old record is where every run looks
        self.assertTrue(out['tmp_left'])
        self.assertTrue(any(n.endswith('.json.tmp') for n in out['found']), out['found'])
        self.assertIn('PENDING RECORD (an unfinished rewrite)', out['next'])   # fail closed, before any other record

    def test_a_backup_that_changed_keeps_the_record(self):
        out = self.run_ps(r"""
$bak = Backup-Saves
Set-Content (Join-Path $bak 'SaveData/Save0001.sav') 'tampered'
$out.kept = Complete-PendingRecord $true
""")
        self.assertIn('the backup changed after it was verified', out['kept'])

    def test_the_driver_and_the_launch_refuse_without_a_record(self):
        out = self.run_ps(r"""
New-Item -ItemType Directory -Force (Join-Path $script:Game 'sdk_mods') | Out-Null
$out.driver = Fail-Text { Install-Driver -BlockSavesAtStart -Exclusive }
$out.driver_dir = Test-Path $script:DriverDir
$out.game = Fail-Text { Start-Game @('-windowed') }
$out.mods = Fail-Text { Set-SdkModsOff (Join-Path $tmp 'game/sdk_mods/settings') }
""")
        for key in ('driver', 'game', 'mods'):
            self.assertIn('no pending record has been written', out[key])
        self.assertFalse(out['driver_dir'])

    def test_the_driver_install_is_recorded_before_the_folder_appears(self):
        out = self.run_ps(r"""
New-Item -ItemType Directory -Force (Join-Path $script:Game 'sdk_mods') | Out-Null
$bak = Backup-Saves
Install-Driver -BlockSavesAtStart -Exclusive
$r = Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json
$out.changes = @($r.changes | ForEach-Object { $_ -replace '^\S+ ', '' })
$out.kept = Complete-PendingRecord $true
Remove-Driver
$out.removed = Complete-PendingRecord $true
""")
        self.assertEqual(len(out['changes']), 1)
        self.assertIn('save block from the first frame: True', out['changes'][0])
        self.assertIn('the driver folder is still there', out['kept'])
        self.assertTrue(out['removed'].startswith('removed'), out['removed'])

    def test_mods_off_is_recorded_first_and_the_restore_puts_it_back(self):
        out = self.run_ps(r"""
$settings = Join-Path $tmp 'game/sdk_mods/settings'
$bak = Backup-Saves
$out.not_a_root = Fail-Text { Set-SdkModsOff (Join-Path $tmp 'elsewhere') }
$roots = [ordered]@{ SaveData = (Join-Path $tmp 'willow/SaveData'); Config = (Join-Path $tmp 'willow/Config'); sdk_settings = $settings; SanitySaver = (Join-Path $tmp 'game/sdk_mods/SanitySaver') }
$snapshots = @{ SaveData = (Get-TreeHashes (Join-Path $bak 'SaveData')); Config = (Get-TreeHashes (Join-Path $bak 'Config')) }
$snapshots.sdk_settings = New-VerifiedSnapshot $settings (Join-Path $bak 'sdk_settings'); $snapshots.SanitySaver = 'absent'
Save-VerifiedBackupRecord $bak $roots $snapshots
$out.off = @(Set-SdkModsOff $settings)
$out.text = Read-Text (Join-Path $settings 'SomeMod.json')
$out.changes = @((Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json).changes | ForEach-Object { $_ -replace '^\S+ ', '' })
$out.kept = Complete-PendingRecord $true
Restore-FromVerifiedRecord $bak | Out-Null
$out.back = Read-Text (Join-Path $settings 'SomeMod.json')
$out.removed = Complete-PendingRecord $true
""")
        self.assertIn('is not a backed-up root', out['not_a_root'])
        self.assertEqual(out['off'], ['SomeMod'])
        self.assertIn('"enabled": false', out['text'])
        self.assertIn('SDK mods off', out['changes'][0]); self.assertIn('SomeMod', out['changes'][0])
        self.assertIn('sdk_settings: differs from the backup', out['kept'])
        self.assertIn('"enabled": true', out['back'])
        self.assertTrue(out['removed'].startswith('removed'), out['removed'])


class RefusalTest(PendingBase):
    """Records an earlier run left: every start refuses with the steps unless what is left can be completed and verified here."""

    def leave_record(self, body=''):
        """A run that backed up, launched a game that wrote the saves, and was killed before its finally (here: the process just ends)."""
        self.run_ps(r"""
New-Item -ItemType Directory -Force (Join-Path $script:Game 'sdk_mods') | Out-Null
$bak = Backup-Saves
""" + body)
        records = self.records()
        self.assertEqual(len(records), 1)
        return self.pending / records[0]

    def test_a_killed_run_stops_the_next_with_exact_steps_and_changes_nothing(self):
        record = self.leave_record("Set-Content (Join-Path $tmp 'willow/SaveData/Save0001.sav') 'written by the killed run'")
        out = self.run_ps(r"""
$out.refusal = Fail-Text { Assert-NoPendingRecords }
$out.backup_refusal = Fail-Text { Backup-Saves }
$out.backups = @(Get-ChildItem (Join-Path $tmp 'realgame') -Directory -Filter 'save-backup-*').Count
$out.save = (Read-Text (Join-Path $tmp 'willow/SaveData/Save0001.sav')).Trim()
""")
        for key in ('refusal', 'backup_refusal'):
            self.assertIn('PENDING RECORD', out[key])
            self.assertIn(f"Restore-PendingRecord '{record}'", out[key])
            self.assertIn('SaveData: differs from the backup', out[key])
            self.assertIn('This run changed nothing', out[key])
            self.assertIn('Make sure Borderlands 2 is closed', out[key])
        self.assertEqual(out['backups'], 1)   # only the killed run's
        self.assertEqual(out['save'], 'written by the killed run')
        self.assertTrue(record.exists())
        # The printed steps work: restore, verify, remove; then a run may start.
        out = self.run_ps(rf"""
$out.restore = @(Restore-PendingRecord '{record}')
$out.save = (Read-Text (Join-Path $tmp 'willow/SaveData/Save0001.sav')).Trim()
$out.next = Fail-Text {{ Assert-NoPendingRecords }}
""")
        self.assertIn('record', out['restore'][-1]); self.assertIn('removed', out['restore'][-1])
        self.assertIn('verified by tree hash, to', out['restore'][0])
        self.assertEqual(out['save'], 'original save')
        # What the killed run (or a person playing since) left is set aside first, not lost.
        aside = sorted((self.tmp / 'realgame').glob('pre-restore-*'))
        self.assertEqual(len(aside), 1)
        self.assertEqual((aside[0] / 'SaveData/Save0001.sav').read_text(encoding='utf-8-sig').strip(), 'written by the killed run')
        self.assertTrue((aside[0] / 'pre_restore.json').exists())
        self.assertEqual(out['next'], '')
        self.assertFalse(record.exists())

    def test_a_backup_and_manifest_changed_to_agree_are_refused_against_the_record(self):
        """Restore-PendingRecord does not trust verified_hashes.json alone: a backup file and its manifest hash both rewritten (so they agree)
        no longer give the record's tree hash for that root, and the root is refused. Review F2 on PR #200."""
        import hashlib
        record = self.leave_record("Set-Content (Join-Path $tmp 'willow/SaveData/Save0001.sav') 'written by the killed run'")
        backup = Path(json.loads(record.read_text(encoding='utf-8-sig'))['backup'])
        (backup / 'SaveData/Save0001.sav').write_bytes(b'tampered')
        manifest = backup / 'verified_hashes.json'
        data = json.loads(manifest.read_text(encoding='utf-8-sig'))
        data['SaveData']['snapshot']['Save0001.sav'] = hashlib.sha256(b'tampered').hexdigest().upper()
        manifest.write_text(json.dumps(data), encoding='utf-8')
        out = self.run_ps(rf"""$out.lines = @(Fail-Text {{ Restore-PendingRecord '{record}' -WriterGone }})""")
        text = '\n'.join(out['lines'])
        self.assertIn('record kept at', text)
        self.assertIn('SaveData', text)
        self.assertEqual((self.tmp / 'willow/SaveData/Save0001.sav').read_text(encoding='utf-8-sig').strip(), 'written by the killed run')
        self.assertTrue(record.exists())

    def test_a_manifest_naming_another_live_folder_is_refused_against_the_record(self):
        record = self.leave_record()
        backup = Path(json.loads(record.read_text(encoding='utf-8-sig'))['backup'])
        decoy = self.tmp / 'decoy'
        decoy.mkdir()
        (decoy / 'mine.txt').write_text('not a save', encoding='utf-8')
        manifest = backup / 'verified_hashes.json'
        data = json.loads(manifest.read_text(encoding='utf-8-sig'))
        data['Config']['live'] = str(decoy)
        manifest.write_text(json.dumps(data), encoding='utf-8')
        out = self.run_ps(rf"""$out.lines = @(Fail-Text {{ Restore-PendingRecord '{record}' -WriterGone }})""")
        self.assertIn('record kept at', out['lines'][0])
        self.assertIn('Config', out['lines'][0])
        self.assertEqual((decoy / 'mine.txt').read_text(encoding='utf-8'), 'not a save')   # the manifest's destination was not written
        self.assertTrue(record.exists())
        # The restore rows say why (Restore-FromVerifiedRecord with the record).
        out = self.run_ps(rf"""
$r = Restore-FromVerifiedRecord '{backup}' (Get-Content -Raw -LiteralPath '{record}' | ConvertFrom-Json)
$out.rows = @($r.rows | ForEach-Object {{ "$($_.file): $($_.status)" }}); $out.clean = $r.clean; $out.summary = $r.summary
""")
        self.assertTrue(any(row.startswith("Config: REFUSED: the manifest's live folder") for row in out['rows']), out)
        self.assertFalse(out['clean'])
        self.assertIn('roots refused', out['summary'])

    def test_a_missing_backup_root_counts_as_changed_not_empty(self):
        """A root verified empty has an empty snapshot; its backup folder gone must still keep the record (review F3 on PR #200)."""
        out = self.run_ps(r"""
New-Item -ItemType Directory -Force (Join-Path $script:Game 'sdk_mods/SanitySaver') | Out-Null   # an empty root
$bak = Backup-Saves
Remove-Item -LiteralPath (Join-Path $bak 'SanitySaver') -Recurse -Force
$out.pending = Complete-PendingRecord $true
$r = Restore-FromVerifiedRecord $bak
$out.rows = @($r.rows | ForEach-Object { "$($_.file): $($_.status)" }); $out.clean = $r.clean
$out.live = Test-Path -LiteralPath (Join-Path $script:Game 'sdk_mods/SanitySaver')
""")
        self.assertIn('KEPT', out['pending'])
        self.assertIn('SanitySaver: the backup changed after it was verified (its folder', out['pending'])
        self.assertTrue(any(row.startswith('SanitySaver: BACKUP CHANGED / MISSING') for row in out['rows']), out)
        self.assertFalse(out['clean'])
        self.assertTrue(out['live'])
        self.assertEqual(len(self.records()), 1)

    def test_a_record_whose_roots_already_match_is_completed_and_removed(self):
        record = self.leave_record()
        out = self.run_ps(r"""$out.lines = @(Assert-NoPendingRecords)""")
        self.assertIn('matches the live roots by tree hash. Record removed.', out['lines'][-1])
        self.assertFalse(record.exists())

    def test_only_our_untouched_driver_copy_is_removed_automatically(self):
        record = self.leave_record("Install-Driver -BlockSavesAtStart -Exclusive")
        out = self.run_ps(r"""$out.lines = @(Assert-NoPendingRecords); $out.driver = Test-Path $script:DriverDir""")
        self.assertFalse(out['driver'])
        self.assertIn('removed the driver copy', out['lines'][0])
        self.assertFalse(record.exists())
        # A driver folder someone added to is theirs: refused, left alone.
        record = self.leave_record("Install-Driver -BlockSavesAtStart -Exclusive; Set-Content (Join-Path $script:DriverDir 'notes.txt') 'mine'")
        out = self.run_ps(r"""$out.refusal = Fail-Text { Assert-NoPendingRecords }; $out.driver = Test-Path $script:DriverDir""")
        self.assertIn('the driver folder is still there', out['refusal'])
        self.assertTrue(out['driver'])
        self.assertTrue(record.exists())

    def test_a_game_running_refuses_without_checking(self):
        self.leave_record()
        (self.tmp / 'game_running').write_text('')
        out = self.run_ps(r"""$out.refusal = Fail-Text { Assert-NoPendingRecords }""")
        self.assertIn('Borderlands 2 is running', out['refusal'])

    def test_the_writer_is_known_by_pid_and_start_time_not_by_its_command_line(self):
        """A live process with no script name on its command line (an ISE, an interactive shell) still counts as the writer."""
        record = self.leave_record()
        live = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])
        try:
            def rewrite(start):
                data = json.loads(record.read_text(encoding='utf-8-sig'))
                data['pid'] = live.pid
                if start is None:
                    data.pop('process_start_utc', None)
                else:
                    data['process_start_utc'] = start
                record.write_text(json.dumps(data), encoding='utf-8')
            started = self.run_ps(rf"""$out.start = (Get-Process -Id {live.pid}).StartTime.ToUniversalTime().ToString('o')""")['start']
            # Same PID, same start time: the writer is alive. Nothing is completed or removed.
            rewrite(started)
            out = self.run_ps(rf"""
$out.refusal = Fail-Text {{ Assert-NoPendingRecords }}
$out.restore = Fail-Text {{ Restore-PendingRecord '{record}' -WriterGone }}
""")
            self.assertIn('is still the run that wrote', out['refusal'])
            self.assertIn('is still the run that wrote', out['restore'])
            self.assertTrue(record.exists())
            # Same PID, no recorded start time: cannot be shown gone, so nothing is completed (fail closed).
            rewrite(None)
            out = self.run_ps(rf"""
$out.refusal = Fail-Text {{ Assert-NoPendingRecords }}
$out.restore = Fail-Text {{ Restore-PendingRecord '{record}' }}
""")
            self.assertIn('cannot be shown to have ended', out['refusal'])
            self.assertIn('-WriterGone', out['restore'])
            self.assertTrue(record.exists())
            # Same PID, another start time: Windows gave the PID to another process; the writer has gone.
            rewrite('2001-01-01T00:00:00.0000000Z')
            out = self.run_ps(r"""$out.lines = @(Assert-NoPendingRecords)""")
            self.assertIn('Record removed', out['lines'][-1])
            self.assertFalse(record.exists())
        finally:
            live.kill()   # the PID this test started

    def test_other_scripts_records_are_honoured(self):
        (self.pending / 'mission_screen_mod_pending.json').write_text('{"written": "2026-11-07", "mods": ["X"]}', encoding='utf-8')
        out = self.run_ps(r"""$out.refusal = Fail-Text { Assert-NoPendingRecords }""")
        self.assertIn('names no backup', out['refusal']); self.assertIn('"mods": ["X"]', out['refusal'])
        (self.pending / 'mission_screen_mod_pending.json').unlink()
        (self.pending / 'showcase_run_pending.json').write_text('not json {', encoding='utf-8')
        out = self.run_ps(r"""$out.refusal = Fail-Text { Assert-NoPendingRecords }""")
        self.assertIn('names no backup', out['refusal']); self.assertIn('not json {', out['refusal'])
        (self.pending / 'showcase_run_pending.json').unlink()
        # The showcase's own format: a backup with verified_hashes.json, checked the same way.
        record = self.leave_record()
        ours = json.loads(record.read_text(encoding='utf-8-sig'))
        record.unlink()
        # Its pid names no live process, so its writer has gone. (A record with no pid at all cannot be shown gone and is never completed.)
        gone = subprocess.Popen([sys.executable, '-c', 'pass']); gone.wait()
        (self.pending / 'showcase_run_pending.json').write_text(json.dumps({'backup': ours['backup'], 'pid': gone.pid, 'script': 'showcase_real_capture'}), encoding='utf-8')
        (self.tmp / 'willow/Config/game.ini').write_text('changed', encoding='utf-8')
        out = self.run_ps(r"""$out.refusal = Fail-Text { Assert-NoPendingRecords }""")
        self.assertIn('Config: differs from the backup', out['refusal'])
        (self.tmp / 'willow/Config/game.ini').write_text('original config', encoding='utf-8')
        out = self.run_ps(r"""$out.lines = @(Assert-NoPendingRecords)""")
        self.assertIn('Record removed', out['lines'][-1])
        self.assertEqual(self.records(), [])


class TestModeTest(unittest.TestCase):
    """OW_TEST_PENDING_DIR is honoured only in the explicit synthetic test mode, on synthetic folders; anything else refuses to start."""

    def child(self, env, body=''):
        with tempfile.TemporaryDirectory() as tmp:
            script = Path(tmp) / 'mode.ps1'
            script.write_text(f". '{REALGAME}'\n{body}\n'loaded'\n", encoding='utf-8-sig')
            base = {k: v for k, v in os.environ.items() if k not in ('OW_TEST_PENDING_DIR', 'OW_TEST_MODE')}
            return subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True,
                                  timeout=120, env=dict(base, OPENWILLOW_BL2=tmp, **env))

    def test_an_inherited_test_folder_without_the_mode_refuses_to_start(self):
        with tempfile.TemporaryDirectory() as pending:
            done = self.child({'OW_TEST_PENDING_DIR': pending, 'OW_TEST_MODE': ''})
            self.assertNotEqual(done.returncode, 0)
            self.assertIn('refusing to start', done.stdout + done.stderr)
            self.assertNotIn('loaded', done.stdout)

    def test_the_mode_alone_or_a_folder_outside_temp_refuses(self):
        done = self.child({'OW_TEST_MODE': 'synthetic-real-game', 'OW_TEST_PENDING_DIR': ''})
        self.assertIn('refusing to start', done.stdout + done.stderr)
        done = self.child({'OW_TEST_MODE': 'synthetic-real-game', 'OW_TEST_PENDING_DIR': str(ROOT / 'local' / 'pending-test')})
        self.assertIn('must be under', done.stdout + done.stderr)
        self.assertNotIn('loaded', done.stdout)

    def test_test_mode_on_the_real_save_folders_refuses_at_first_use(self):
        # The harness's own default WillowDir is the maintainer's Documents folder: test mode must refuse it before reading or writing records.
        with tempfile.TemporaryDirectory() as pending:
            done = self.child({'OW_TEST_PENDING_DIR': pending, 'OW_TEST_MODE': 'synthetic-real-game'},
                              "try { Get-PendingFiles | Out-Null; 'used' } catch { \"refused: $_\" }")
            self.assertIn('refused: refusing: test mode', done.stdout, done.stdout + done.stderr)
            self.assertNotIn('used', done.stdout.replace('refused', ''))


class DriverOwnershipTest(PendingBase):
    """The driver folder is this run's only when this run created it (or claimed it); nothing else is overwritten or removed."""

    def test_an_existing_folder_is_never_overwritten_or_removed(self):
        out = self.run_ps(r"""
New-Item -ItemType Directory -Force $script:DriverDir | Out-Null
Set-Content (Join-Path $script:DriverDir '__init__.py') 'someone else'
$bak = Backup-Saves
$out.plain = Fail-Text { Install-Driver }
$out.exclusive = Fail-Text { Install-Driver -BlockSavesAtStart -Exclusive }
$out.claim = Fail-Text { Install-Driver -ClaimedBy 'run-1' }
Remove-Driver 3>$null
$out.kept = (Read-Text (Join-Path $script:DriverDir '__init__.py')).Trim()
$out.changes = @((Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json).changes).Count
""")
        for key in ('plain', 'exclusive', 'claim'):
            self.assertIn('already exists and is not this run', out[key])
        self.assertEqual(out['kept'], 'someone else')
        self.assertEqual(out['changes'], 0)   # refused before anything was recorded or touched

    def test_a_claimed_folder_is_filled_and_only_this_runs_files_are_removed(self):
        out = self.run_ps(r"""
New-Item -ItemType Directory -Force (Join-Path $script:Game 'sdk_mods') | Out-Null
$bak = Backup-Saves
New-Item -ItemType Directory -Path $script:DriverDir | Out-Null
New-Item -ItemType File -Path (Join-Path $script:DriverDir '.owrun') -Value 'run-1' | Out-Null
$out.wrong = Fail-Text { Install-Driver -ClaimedBy 'run-2' }
Install-Driver -ClaimedBy 'run-1'
$out.installed = Test-Path (Join-Path $script:DriverDir '__init__.py')
Set-Content (Join-Path $script:DriverDir 'notes.txt') 'added by a person'
Remove-Driver 3>$null
$out.left_with_extra = Test-Path $script:DriverDir
Remove-Item (Join-Path $script:DriverDir 'notes.txt')
Remove-Driver
$out.removed = !(Test-Path $script:DriverDir)
""")
        self.assertIn('already exists and is not this run', out['wrong'])
        self.assertTrue(out['installed'])
        self.assertTrue(out['left_with_extra'])
        self.assertTrue(out['removed'])


class CaseSensitiveHashTest(PendingBase):
    def test_paths_differing_only_in_case_are_different_entries(self):
        out = self.run_ps(r"""
$a = [hashtable]::new([StringComparer]::Ordinal); $a['Save.sav'] = 'H1'
$b = [hashtable]::new([StringComparer]::Ordinal); $b['save.sav'] = 'H1'
$out.diff = @(Compare-Hashes $a $b)
$both = [hashtable]::new([StringComparer]::Ordinal); $both['Save.sav'] = 'H1'; $both['save.sav'] = 'H2'
$out.collision = @(Compare-Hashes $both $a)
$out.same = @(Compare-Hashes $both $both).Count
$out.hash_case = @(Compare-Hashes @{ 'x' = 'ABC' } @{ 'x' = 'abc' })
$h = Get-TreeHashes (Join-Path $tmp 'willow/SaveData')
$out.exact = $h.ContainsKey('Save0001.sav'); $out.other_case = $h.ContainsKey('SAVE0001.SAV')
""")
        self.assertEqual(out['diff'], ['Save.sav', 'save.sav'])
        self.assertEqual(out['collision'], ['save.sav'])
        self.assertEqual(out['same'], 0)
        self.assertEqual(out['hash_case'], ['x'])
        self.assertTrue(out['exact']); self.assertFalse(out['other_case'])

    def test_a_rename_that_changes_only_case_is_seen_and_restored(self):
        out = self.run_ps(r"""
$bak = Backup-Saves
$d = Join-Path $tmp 'willow/SaveData'
$before = Get-TreeDigest $d
Rename-Item (Join-Path $d 'Save0001.sav') 'tmp.sav'; Rename-Item (Join-Path $d 'tmp.sav') 'SAVE0001.sav'
$out.digest_changed = ($before -ne (Get-TreeDigest $d))
$out.kept = Complete-PendingRecord $true
$out.restore = (Restore-FromVerifiedRecord $bak).summary
$out.names = @(Get-ChildItem $d -File | ForEach-Object { $_.Name } | Sort-Object)
$out.removed = Complete-PendingRecord $true
""")
        self.assertTrue(out['digest_changed'])
        self.assertIn('SaveData: differs from the backup', out['kept'])
        self.assertIn('Save0001.sav', out['names']); self.assertNotIn('SAVE0001.sav', out['names'])
        self.assertTrue(out['removed'].startswith('removed'), out['removed'])


class RecoveryTest(PendingBase):
    """Review r2: one way back while a record exists, every launch-written root covered, the lock and the game checks around a recovery,
    the driver owner, and keys a killed run left held."""

    def leave(self, body=''):
        self.run_ps("New-Item -ItemType Directory -Force (Join-Path $script:Game 'sdk_mods') | Out-Null\n$bak = Backup-Saves\n" + body)
        return self.pending / self.records()[0]

    def test_the_recovery_text_names_only_restore_pending_record(self):
        out = self.run_ps(r"""
$bak = Backup-Saves
$out.text = Format-ManualRecovery 'test' $bak (Join-Path $tmp 'hub.lock') 'lane'
$out.refused = Fail-Text { Restore-VerifiedBackup $bak }
""")
        self.assertIn('Restore-PendingRecord', out['text'])
        self.assertNotIn('Restore-VerifiedBackup', out['text'])
        self.assertIn('use Restore-PendingRecord', out['refused'])

    def test_restore_verified_backup_without_a_record_sets_the_state_aside_first(self):
        record = self.leave("Set-Content (Join-Path $tmp 'willow/SaveData/Save0001.sav') 'played since'")
        backup = json.loads(record.read_text(encoding='utf-8-sig'))['backup']
        record.unlink()   # as if no record named this backup
        out = self.run_ps(rf"""$out.lines = @(Restore-VerifiedBackup '{backup}'); $out.save = (Read-Text (Join-Path $tmp 'willow/SaveData/Save0001.sav')).Trim()""")
        self.assertIn('verified by tree hash, to', out['lines'][0])
        self.assertEqual(out['save'], 'original save')
        aside = sorted((self.tmp / 'realgame').glob('pre-restore-*'))
        self.assertEqual((aside[0] / 'SaveData/Save0001.sav').read_text(encoding='utf-8-sig').strip(), 'played since')

    def test_the_sdk_settings_and_sanity_saver_store_are_backed_up_and_restored(self):
        (self.tmp / 'game/sdk_mods/SanitySaver').mkdir(parents=True)
        (self.tmp / 'game/sdk_mods/SanitySaver/store.sav').write_text('original store', encoding='utf-8')
        record = self.leave("""Set-Content (Join-Path $tmp 'game/sdk_mods/settings/SomeMod.json') 'rewritten by the game'
Set-Content (Join-Path $tmp 'game/sdk_mods/SanitySaver/store.sav') 'rewritten by the game'""")
        data = json.loads(record.read_text(encoding='utf-8-sig'))
        self.assertEqual(sorted(data['roots']), sorted(['SaveData', 'Config', 'sdk_settings', 'SanitySaver']))
        out = self.run_ps(rf"""$out.refusal = Fail-Text {{ Assert-NoPendingRecords }}; $out.lines = @(Restore-PendingRecord '{record}')""")
        self.assertIn('sdk_settings: differs from the backup', out['refusal'])
        self.assertIn('SanitySaver: differs from the backup', out['refusal'])
        self.assertEqual((self.tmp / 'game/sdk_mods/settings/SomeMod.json').read_text(encoding='utf-8-sig'), '{"enabled": true, "keys": {}}')
        self.assertEqual((self.tmp / 'game/sdk_mods/SanitySaver/store.sav').read_text(encoding='utf-8-sig'), 'original store')
        self.assertFalse(record.exists())

    def test_a_root_absent_at_the_backup_and_created_by_the_game_keeps_the_record(self):
        record = self.leave("New-Item -ItemType Directory -Force (Join-Path $tmp 'game/sdk_mods/SanitySaver') | Out-Null; Set-Content (Join-Path $tmp 'game/sdk_mods/SanitySaver/new.sav') 'x'")
        out = self.run_ps(r"""$out.refusal = Fail-Text { Assert-NoPendingRecords }""")
        self.assertIn('SanitySaver: absent at the backup, present now', out['refusal'])
        self.assertTrue(record.exists())

    def test_recovery_holds_the_lock_and_stops_when_a_game_appears(self):
        record = self.leave("Set-Content (Join-Path $tmp 'willow/SaveData/Save0001.sav') 'written by the killed run'")
        out = self.run_ps(rf"""
$global:calls = 0
function Get-GameProcesses {{ $global:calls++; if ($global:calls -ge 2) {{ @([pscustomobject]@{{ Id = 77; MainWindowHandle = [IntPtr]::Zero }}) }} else {{ @() }} }}
function Save-PreRestoreSnapshot([string]$Backup) {{ $global:lockDuring = Test-Path -LiteralPath (Get-OwUeLockPath); 'aside' }}
$out.error = Fail-Text {{ Restore-PendingRecord '{record}' }}
$out.lock_during = $global:lockDuring
$out.lock_after = Test-Path -LiteralPath (Get-OwUeLockPath)
$out.save = (Read-Text (Join-Path $tmp 'willow/SaveData/Save0001.sav')).Trim()
""")
        self.assertIn('Borderlands 2 is running (pid 77) after the copy', out['error'])
        self.assertTrue(out['lock_during'])
        self.assertFalse(out['lock_after'])
        self.assertEqual(out['save'], 'written by the killed run')   # nothing restored
        self.assertTrue(record.exists())

    def test_restore_entry_points_refuse_real_folders_in_test_mode(self):
        record = self.leave()
        data = json.loads(record.read_text(encoding='utf-8-sig'))
        data['backup'] = str(ROOT / 'local' / 'realgame' / 'save-backup-not-temp')
        record.write_text(json.dumps(data), encoding='utf-8')
        out = self.run_ps(rf"""
$out.pending = Fail-Text {{ Restore-PendingRecord '{record}' }}
$out.verified = Fail-Text {{ Restore-VerifiedBackup '{data['backup']}' }}
$out.roots = Fail-Text {{ Restore-VerifiedRoots (Join-Path $tmp 'bak') ([ordered]@{{ SaveData = '{ROOT / 'local' / 'x'}' }}) @{{}} }}
""")
        for key in ('pending', 'verified', 'roots'):
            self.assertIn('test mode', out[key], key)

    def test_a_driver_folder_with_another_owner_is_never_removed(self):
        record = self.leave("Install-Driver -BlockSavesAtStart -Exclusive")
        data = json.loads(record.read_text(encoding='utf-8-sig'))
        self.assertTrue(data['driver_owner'])
        owner = self.tmp / 'game/sdk_mods/openwillow_realgame/.owrun'
        self.assertEqual(owner.read_text(encoding='utf-8'), data['driver_owner'])
        owner.write_text('someone-else', encoding='utf-8')
        out = self.run_ps(rf"""$out.refusal = Fail-Text {{ Assert-NoPendingRecords }}; $out.lines = @(Fail-Text {{ Restore-PendingRecord '{record}' }}); $out.driver = Test-Path $script:DriverDir""")
        self.assertIn('the driver folder is still there', out['refusal'])
        self.assertIn('the driver folder is still there', out['lines'][0])
        self.assertTrue(out['driver'])
        self.assertTrue(record.exists())

    def test_keys_a_killed_run_held_are_recorded_and_released_by_the_next_run(self):
        mock = r"""
$global:native = [Collections.Generic.List[string]]::new()
function Invoke-NativeKey([byte]$Vk, [byte]$Scan, [uint32]$Flags) { $global:native.Add("key $Scan $Flags") }
function Invoke-NativeMouse([uint32]$Flags, [int]$Dx = 0, [int]$Dy = 0, [uint32]$Data = 0) { $global:native.Add("mouse $Flags") }
function Assert-GameForeground { }
"""
        # The run presses W and the right button, then dies without its finally.
        record = self.leave(mock + "Send-KeyDown W; Send-MouseEvent 0x8")
        self.assertEqual(json.loads(record.read_text(encoding='utf-8-sig'))['held_inputs'], ['key W', 'button right'])
        # The game is still open: the next run sends the releases first, then refuses because a game runs.
        (self.tmp / 'game_running').write_text('')
        out = self.run_ps(mock + r"""$out.refusal = Fail-Text { Assert-NoPendingRecords }; $out.native = @($global:native)""")
        self.assertEqual(out['native'], ['key 17 10', 'mouse 16'])
        self.assertIn('Borderlands 2 is running', out['refusal'])
        self.assertEqual(json.loads(record.read_text(encoding='utf-8-sig'))['held_inputs'], [])
        # A killed run whose game has closed too: the keys were synthesised system-wide, and the game's exit does not release them, so
        # the releases are still sent, the entries cleared only after that, and the record completes as usual.
        (self.tmp / 'game_running').unlink()
        record.unlink()
        record = self.leave(mock + "Send-KeyDown W; Send-MouseEvent 0x8")
        out = self.run_ps(mock + r"""$out.lines = @(Assert-NoPendingRecords); $out.native = @($global:native)""")
        self.assertEqual(out['native'], ['key 17 10', 'mouse 16'])
        self.assertIn('released what the run that wrote', out['lines'][0])
        self.assertFalse(record.exists())

    def test_an_earlier_record_is_completed_only_under_the_run_lock_and_read_again_there(self):
        """Review of #245: two starters can read the same dead writer's record. The one that waited must not send the recorded releases into the
        session of a run that holds the run lock, nor write back a record that run already completed."""
        mock = r"""
$global:native = [Collections.Generic.List[string]]::new()
function Invoke-NativeKey([byte]$Vk, [byte]$Scan, [uint32]$Flags) { $global:native.Add("key $Scan $Flags") }
function Invoke-NativeMouse([uint32]$Flags, [int]$Dx = 0, [int]$Dy = 0, [uint32]$Data = 0) { $global:native.Add("mouse $Flags") }
function Assert-GameForeground { }
"""
        record = self.leave(mock + "Send-KeyDown W; Send-MouseEvent 0x8")
        # Another run holds the run lock: nothing is released, the record is untouched, and this run refuses.
        out = self.run_ps(mock + r"""
$script:PendingLockSeconds = 2
Enter-OwUeLock -Owner 'another capture' -TimeoutSeconds 5
$out.refusal = Fail-Text { Assert-NoPendingRecords }
Exit-OwUeLock
$out.native = @($global:native)
""")
        self.assertEqual(out['native'], [])
        self.assertIn('UE run lock', out['refusal'])
        self.assertEqual(json.loads(record.read_text(encoding='utf-8-sig'))['held_inputs'], ['key W', 'button right'])
        # The other run completed the record while this one waited for the lock: nothing is released, and the record is not written back.
        out = self.run_ps(mock + r"""
function Enter-OwUeLock { param($Owner, $TimeoutSeconds, $AlsoWaitFor) Remove-Item -LiteralPath '__RECORD__' }
function Exit-OwUeLock { }
$out.lines = @(Assert-NoPendingRecords)
$out.native = @($global:native)
""".replace('__RECORD__', str(record)))
        self.assertEqual(out['native'], [])
        self.assertFalse(record.exists())
        self.assertIn('completed by another run', out['lines'][0])

    def test_a_release_that_fails_stays_recorded(self):
        mock = r"""
$global:native = [Collections.Generic.List[string]]::new()
function Invoke-NativeKey([byte]$Vk, [byte]$Scan, [uint32]$Flags) { if ($Scan -eq 17) { throw 'invented: the release failed' }; $global:native.Add("key $Scan $Flags") }
function Invoke-NativeMouse([uint32]$Flags, [int]$Dx = 0, [int]$Dy = 0, [uint32]$Data = 0) { $global:native.Add("mouse $Flags") }
function Assert-GameForeground { }
"""
        record = self.leave(mock.replace("if ($Scan -eq 17) { throw 'invented: the release failed' }; ", '') + "Send-KeyDown W; Send-MouseEvent 0x8")
        out = self.run_ps(mock + r"""$out.text = Fail-Text { Restore-PendingRecord '__RECORD__' }; $out.native = @($global:native)""".replace('__RECORD__', str(record)))
        self.assertEqual(out['native'], ['mouse 16'])   # the button's release went out; the key's failed
        self.assertEqual(json.loads(record.read_text(encoding='utf-8-sig'))['held_inputs'], ['key W'])   # kept, not cleared

    def test_a_game_running_without_a_window_still_gets_every_release(self):
        """A hung or closing game has no window: the synthesised keys are still held system-wide, so every release is sent and each entry is
        cleared once its own release went out (review of c001181). The record itself stays: a game runs."""
        mock = r"""
$global:native = [Collections.Generic.List[string]]::new()
function Invoke-NativeKey([byte]$Vk, [byte]$Scan, [uint32]$Flags) { $global:native.Add("key $Scan $Flags") }
function Invoke-NativeMouse([uint32]$Flags, [int]$Dx = 0, [int]$Dy = 0, [uint32]$Data = 0) { $global:native.Add("mouse $Flags") }
function Assert-GameForeground { }
"""
        record = self.leave(mock + "Send-KeyDown W; Send-MouseEvent 0x8")
        out = self.run_ps(mock + r"""
function Get-GameProcesses { @([pscustomobject]@{ Id = 55; MainWindowHandle = [IntPtr]::Zero }) }
$out.restore = Fail-Text { Restore-PendingRecord '__RECORD__' }
$out.native = @($global:native)
""".replace('__RECORD__', str(record)))
        self.assertEqual(out['native'], ['key 17 10', 'mouse 16'])
        self.assertIn('Borderlands 2 is running', out['restore'])
        self.assertEqual(json.loads(record.read_text(encoding='utf-8-sig'))['held_inputs'], [])
        self.assertTrue(record.exists())

    def test_a_failed_release_write_does_not_keep_a_clean_run_s_record(self):
        """A release whose journal write failed leaves the key named in the record; the clean run's empty list is written at completion, so the
        record goes (review of c001181)."""
        out = self.run_ps(r"""
function Invoke-NativeKey([byte]$Vk, [byte]$Scan, [uint32]$Flags) { }
function Assert-GameForeground { }
$bak = Backup-Saves
Send-KeyDown W
$real = ${function:Set-PendingField}
function Set-PendingField([string]$Name, $Value) { throw 'invented: the journal write failed' }
Send-KeyUp W 3>$null
$out.named = @((Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json).held_inputs)
Set-Item function:Set-PendingField $real
$out.done = Complete-PendingRecord $true
""")
        self.assertEqual(out['named'], ['key W'])   # the failed write left it named
        self.assertTrue(out['done'].startswith('removed'), out['done'])

    def test_the_recovery_text_says_how_to_clear_an_entry_that_never_releases(self):
        out = self.run_ps(r"""
$record = [pscustomobject]@{ script = 's'; pid = 1; written = 'w'; backup = 'b'; changes = @(); held_inputs = @('key NotAKey'); game = $env:OPENWILLOW_BL2 }
$out.text = Format-PendingRecovery 'C:\invented\record.json' $record @('inputs recorded as held were not released: key NotAKey')
""")
        self.assertIn('key NotAKey', out['text'])
        self.assertIn('remove that entry from the record\'s "held_inputs" list', out['text'])

    def test_an_older_record_without_every_root_is_never_completed_automatically(self):
        record = self.leave()
        data = json.loads(record.read_text(encoding='utf-8-sig'))
        manifest = Path(data['backup']) / 'verified_hashes.json'
        roots = json.loads(manifest.read_text(encoding='utf-8-sig'))
        manifest.write_text(json.dumps({k: roots[k] for k in ('SaveData', 'Config')}), encoding='utf-8')   # the older two-root format
        data['roots'] = {k: data['roots'][k] for k in ('SaveData', 'Config')}
        record.write_text(json.dumps(data), encoding='utf-8')
        (self.tmp / 'game/sdk_mods/settings/SomeMod.json').write_text('rewritten by the game', encoding='utf-8')
        # SaveData and Config match, but the record cannot vouch for the SDK roots: refused, never auto-completed.
        out = self.run_ps(r"""$out.refusal = Fail-Text { Assert-NoPendingRecords }""")
        self.assertIn('does not cover sdk_settings, SanitySaver', out['refusal'])
        self.assertTrue(record.exists())
        # Restore-PendingRecord restores what it covers, says what it cannot vouch for, keeps a note, and removes the record.
        out = self.run_ps(rf"""$out.lines = @(Restore-PendingRecord '{record}')""")
        text = '\n'.join(out['lines'])
        self.assertIn('NOT VOUCHED FOR: sdk_settings, SanitySaver', text)
        self.assertTrue((Path(data['backup']) / 'recovery_note.txt').exists())
        self.assertFalse(record.exists())
        self.assertEqual((self.tmp / 'game/sdk_mods/settings/SomeMod.json').read_text(encoding='utf-8-sig'), 'rewritten by the game')   # not touched

    def test_a_game_that_starts_during_the_restore_keeps_the_record_and_says_so(self):
        record = self.leave("Set-Content (Join-Path $tmp 'willow/SaveData/Save0001.sav') 'written by the killed run'")
        out = self.run_ps(rf"""
function Restore-FromVerifiedRecord([string]$Backup) {{ New-Item -ItemType File (Join-Path $tmp 'game_running') | Out-Null; [pscustomobject]@{{ rows = @(); summary = 'restored' }} }}
$out.error = Fail-Text {{ Restore-PendingRecord '{record}' }}
""")
        self.assertIn('A GAME STARTED DURING RECOVERY', out['error'])
        self.assertIn('pre-restore-', out['error'])
        self.assertTrue(record.exists())

    def test_a_press_without_a_record_is_refused_and_a_release_clears_the_list(self):
        out = self.run_ps(r"""
function Invoke-NativeKey([byte]$Vk, [byte]$Scan, [uint32]$Flags) { }
function Assert-GameForeground { }
$out.refused = Fail-Text { Send-KeyDown W }
$out.held = @($script:HeldInputs).Count
$bak = Backup-Saves
Send-KeyDown W
$out.during = @((Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json).held_inputs)
Send-KeyUp W
$out.after = @((Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json).held_inputs).Count
""")
        self.assertIn('refusing to press W', out['refused'])
        self.assertEqual(out['held'], 0)
        self.assertEqual(out['during'], ['key W'])
        self.assertEqual(out['after'], 0)


class GuardedInputTest(unittest.TestCase):
    """The native layer is a log; every press must be matched by a release on every exit path."""

    def run_ps(self, body):
        with tempfile.TemporaryDirectory() as tmp, tempfile.TemporaryDirectory() as pending:
            return powershell(r"""
$global:native = [Collections.Generic.List[string]]::new()
function Invoke-NativeKey([byte]$Vk, [byte]$Scan, [uint32]$Flags) { $global:native.Add("key $Scan $Flags") }
function Invoke-NativeMouse([uint32]$Flags, [int]$Dx = 0, [int]$Dy = 0, [uint32]$Data = 0) { $global:native.Add("mouse $Flags") }
$global:front = $true
function Assert-GameForeground { if (!$global:front) { throw 'refusing to send input: not in front' } }
New-Item -ItemType Directory -Force (Join-Path $tmp 'bak') | Out-Null
if (!$global:NoRecord) { Write-PendingRecord (Join-Path $tmp 'bak') ([ordered]@{}) | Out-Null }
""" + body + "\n$out.native = @($global:native); $out.held = @($script:HeldInputs)", tmp, pending)

    def test_a_key_held_across_an_error_is_released_by_the_finally(self):
        out = self.run_ps(r"""
try { Send-KeyDown W; Send-KeyDown Up; Send-MouseEvent 0x8; throw 'timeout' } catch { $out.error = "$_" } finally { $out.release = Release-HeldInputs }
""")
        self.assertEqual(out['error'], 'timeout')
        self.assertEqual(out['native'], ['key 17 8', 'key 72 9', 'mouse 8', 'key 17 10', 'key 72 11', 'mouse 16'])
        self.assertEqual(out['release'], 'released key W, key Up, button right')
        self.assertEqual(out['held'], [])

    def test_send_key_releases_when_the_hold_throws(self):
        out = self.run_ps(r"""
function Start-Sleep { throw 'Ctrl+C' }
try { Send-Key E 500 } catch { $out.error = "$_" }
Remove-Item Function:\Start-Sleep
$out.release = Release-HeldInputs
""")
        self.assertEqual(out['error'], 'Ctrl+C')
        self.assertEqual(out['native'], ['key 18 8', 'key 18 10'])
        self.assertEqual(out['release'], 'none held')

    def test_a_drag_and_a_click_let_the_button_go_when_focus_is_lost(self):
        out = self.run_ps(r"""
$global:calls = 0
function Assert-GameForeground { $global:calls++; if ($global:calls -gt 2) { throw 'refusing to send input: not in front' } }
try { Send-Drag 100 0 5 } catch { $out.drag = "$_" }
$global:calls = 0
function Start-Sleep { throw 'stopped' }
try { Send-Click -Right } catch { $out.click = "$_" }
""")
        self.assertIn('not in front', out['drag'])
        self.assertEqual(out['native'][0], 'mouse 2'); self.assertIn('mouse 4', out['native'])
        self.assertEqual(out['native'][-2:], ['mouse 8', 'mouse 16'])
        self.assertEqual(out['held'], [])

    def test_a_refused_press_sends_nothing_and_a_release_is_never_refused(self):
        out = self.run_ps(r"""
$global:front = $false
try { Send-KeyDown W } catch { $out.refused = "$_" }
$out.release_while_away = Release-HeldInputs
$global:front = $true; Send-KeyDown A; $global:front = $false
$out.release = Release-HeldInputs
""")
        self.assertIn('not in front', out['refused'])
        self.assertEqual(out['release_while_away'], 'none held')
        self.assertEqual(out['native'], ['key 30 8', 'key 30 10'])
        self.assertEqual(out['release'], 'released key A')


class ScriptWiringTest(unittest.TestCase):
    """Every capture uses the shared pending records and guarded input."""
    # The helpers the captures dot-source are not captures: realgame.ps1, pending.ps1 and slice_watch.ps1 (the -WatchSpec helpers).
    SCRIPTS = sorted(p for p in (ROOT / 'tools/real_game').glob('*.ps1') if p.name not in ('realgame.ps1', 'pending.ps1', 'slice_watch.ps1'))

    def test_every_capture_refuses_on_a_record_first_and_releases_input_in_its_finally(self):
        self.assertGreaterEqual(len(self.SCRIPTS), 18)
        for path in self.SCRIPTS:
            text = path.read_text(encoding='utf-8')
            with self.subTest(script=path.name):
                first = text.index('Assert-NoPendingRecords')
                for later in ('Enter-RunLock', 'Backup-Saves', 'Install-Driver', 'Start-Game', 'New-Item -ItemType File -Path $HubLock'):
                    if later in text:
                        self.assertLess(first, text.index(later), later)
                self.assertIn('Release-HeldInputs', text[text.rindex('finally {'):])
                self.assertTrue('Complete-PendingRecord' in text or 'Complete-BackupRun' in text)
                self.assertNotIn('[OWRG.Native]::keybd_event', text); self.assertNotIn('[OWRG.Native]::mouse_event', text)
                for kill in ('taskkill', 'Stop-Process -Name', '-Name Borderlands2 | Stop'):
                    self.assertNotIn(kill, text)

    def test_every_press_held_across_steps_is_released_in_a_finally(self):
        for path in self.SCRIPTS:
            lines = path.read_text(encoding='utf-8').splitlines()
            for i, line in enumerate(lines):
                if re.search(r'Send-KeyDown|Send-MouseEvent 0x(2|8|20)\b', line) and 'function ' not in line:
                    window = '\n'.join(lines[i:i + 12])
                    with self.subTest(script=path.name, line=i + 1):
                        self.assertIn('finally', window)

    def test_the_harness_sends_input_only_through_the_mockable_layer(self):
        text = REALGAME.read_text(encoding='utf-8')
        self.assertEqual(len(re.findall(r'\[OWRG\.Native\]::(keybd_event|mouse_event)\(', text)), 2)


class WholeRunTest(unittest.TestCase):
    """A whole capture (loot_capture, the game stubbed as in real_game_capture_safety_test): a run left for recovery stops the next one."""

    def run_loot(self, tmp, pending, scenario):
        tools = tmp / 'tools/real_game'
        tools.mkdir(parents=True, exist_ok=True)
        shutil.copy(ROOT / 'tools/real_game/loot_capture.ps1', tools)
        (tools / 'realgame.ps1').write_text(STUB.replace('__REALGAME__', str(REALGAME)).replace('__TMP__', str(tmp)), encoding='utf-8-sig')
        return subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(tools / 'loot_capture.ps1'), '-HubLock',
                               str(tmp / 'hub.lock'), '-Character', 'test'], capture_output=True, text=True, timeout=180,
                              env=dict(os.environ, OPENWILLOW_BL2=str(tmp / 'game'), OW_TEST_SCENARIO=scenario, OW_TEST_PENDING_DIR=str(pending)))

    def test_a_good_run_leaves_no_record_and_a_stuck_one_stops_the_next(self):
        with tempfile.TemporaryDirectory() as folder, tempfile.TemporaryDirectory() as pending_folder:
            tmp, pending = Path(folder), Path(pending_folder)
            for rel, text in {'willow/SaveData/slot.sav': 'original save', 'willow/Config/game.ini': 'original config',
                              'game/sdk_mods/settings/python-sdk.json': 'original settings', 'game/sdk_mods/SanitySaver/store.sav': 'original store'}.items():
                (tmp / rel).parent.mkdir(parents=True, exist_ok=True)
                (tmp / rel).write_text(text, encoding='utf-8')
            good = self.run_loot(tmp, pending, 'good')
            self.assertEqual(good.returncode, 0, good.stdout + good.stderr)
            self.assertIn('cleanup pending: removed: verified by tree hash', good.stdout)
            self.assertIn('cleanup inputs: none held', good.stdout)
            self.assertEqual(list(pending.glob('*.json')), [])
            stuck = self.run_loot(tmp, pending, 'game_stuck')
            self.assertEqual(stuck.returncode, 1)
            self.assertIn('cleanup pending: KEPT', stuck.stdout)
            self.assertEqual(len(list(pending.glob('*pending*.json'))), 1)
            # The locks it kept are the maintainer's to remove; here they go, so only the record stops the next run.
            for lock in (tmp / 'hub.lock', tmp / 'run.lock'):
                lock.unlink(missing_ok=True)
            backups = sorted((tmp / 'realgame').glob('save-backup-*'))
            nxt = self.run_loot(tmp, pending, 'good')
            self.assertEqual(nxt.returncode, 1)
            self.assertIn('PENDING RECORD', nxt.stdout + nxt.stderr)
            self.assertIn('Restore-PendingRecord', nxt.stdout + nxt.stderr)
            self.assertEqual(sorted((tmp / 'realgame').glob('save-backup-*')), backups)   # no new backup, no lock, no launch
            self.assertFalse((tmp / 'hub.lock').exists())
            self.assertEqual((tmp / 'willow/SaveData/slot.sav').read_text(encoding='utf-8-sig').strip(), 'changed by the session')


if __name__ == '__main__':
    unittest.main()
