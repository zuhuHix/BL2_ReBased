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
function Get-GameProcesses { if (Test-Path -LiteralPath (Join-Path $tmp 'game_running')) { @([pscustomobject]@{ Id = 4242; Path = 'x'; StartTime = Get-Date }) } else { @() } }
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
$out.restore = Restore-VerifiedBackup $bak
$out.removed = Complete-PendingRecord $true
$out.after = @(Get-PendingFiles).Count
$out.save = Read-Text (Join-Path $tmp 'willow/SaveData/Save0001.sav')
$out.bak_file_gone = !(Test-Path (Join-Path $tmp 'willow/SaveData/Save0001.sav.bak'))
""")
        self.assertEqual(len(out['records']), 1)
        self.assertRegex(out['records'][0], r'^realgame_run_pending-body-\d+\.json$')
        self.assertEqual(out['kind'], 'openwillow-realgame-pending')
        self.assertTrue(out['pid_ok'] and out['backup_ok'] and out['digest_ok'] and out['has_start'])
        self.assertEqual(out['roots'], ['SaveData', 'Config'])
        self.assertEqual(out['changes'], ['test change'])
        self.assertIn('KEPT', out['kept']); self.assertIn('SaveData: differs from the backup', out['kept'])
        self.assertIn('SaveData: tree hash', out['kept'])
        self.assertIn('a game may still run', out['unsafe'])
        self.assertIn('Borderlands 2 is running', out['game_running'])
        self.assertTrue(out['removed'].startswith('removed: verified by tree hash'), out['removed'])
        self.assertEqual(out['after'], 0)
        self.assertEqual(out['save'].strip(), 'original save')
        self.assertTrue(out['bak_file_gone'])

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
$out.not_a_root = Fail-Text { Set-SdkModsOff $settings }
$roots = [ordered]@{ SaveData = (Join-Path $tmp 'willow/SaveData'); Config = (Join-Path $tmp 'willow/Config'); sdk_settings = $settings }
$snapshots = @{ SaveData = (Get-TreeHashes (Join-Path $bak 'SaveData')); Config = (Get-TreeHashes (Join-Path $bak 'Config')) }
$snapshots.sdk_settings = New-VerifiedSnapshot $settings (Join-Path $bak 'sdk_settings')
Save-VerifiedBackupRecord $bak $roots $snapshots
$out.off = @(Set-SdkModsOff $settings)
$out.text = Read-Text (Join-Path $settings 'SomeMod.json')
$out.changes = @((Get-Content -Raw -LiteralPath $script:PendingRecord | ConvertFrom-Json).changes | ForEach-Object { $_ -replace '^\S+ ', '' })
$out.kept = Complete-PendingRecord $true
Restore-VerifiedBackup $bak | Out-Null
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
        self.assertEqual(out['save'], 'original save')
        self.assertEqual(out['next'], '')
        self.assertFalse(record.exists())

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

    def test_a_game_running_or_a_live_writer_refuses_without_checking(self):
        self.leave_record()
        (self.tmp / 'game_running').write_text('')
        out = self.run_ps(r"""$out.refusal = Fail-Text { Assert-NoPendingRecords }""")
        self.assertIn('Borderlands 2 is running', out['refusal'])
        (self.tmp / 'game_running').unlink()
        # A record naming this very PowerShell's parent-free pid as a live writer: its own pid is skipped, so use a live process whose
        # command line carries the script name.
        name = self.records()[0]
        record = json.loads((self.pending / name).read_text(encoding='utf-8-sig'))
        live = subprocess.Popen(['powershell', '-NoProfile', '-Command', 'Start-Sleep 30 # body'])
        try:
            record['pid'] = live.pid
            (self.pending / name).write_text(json.dumps(record), encoding='utf-8')
            out = self.run_ps(r"""$out.refusal = Fail-Text { Assert-NoPendingRecords }""")
            self.assertIn('still looks like the run (body)', out['refusal'])
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
        (self.pending / 'showcase_run_pending.json').write_text(json.dumps({'backup': ours['backup'], 'pid': 1, 'script': 'showcase_real_capture'}), encoding='utf-8')
        (self.tmp / 'willow/Config/game.ini').write_text('changed', encoding='utf-8')
        out = self.run_ps(r"""$out.refusal = Fail-Text { Assert-NoPendingRecords }""")
        self.assertIn('Config: differs from the backup', out['refusal'])
        (self.tmp / 'willow/Config/game.ini').write_text('original config', encoding='utf-8')
        out = self.run_ps(r"""$out.lines = @(Assert-NoPendingRecords)""")
        self.assertIn('Record removed', out['lines'][-1])
        self.assertEqual(self.records(), [])


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
    SCRIPTS = sorted(p for p in (ROOT / 'tools/real_game').glob('*.ps1') if p.name not in ('realgame.ps1', 'pending.ps1'))

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
