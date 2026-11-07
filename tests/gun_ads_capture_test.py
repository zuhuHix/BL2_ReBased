"""Synthetic checks for the real-game ADS frame capture (no game: the SDK and the namespace it expects are stubbed; names are invented).

Run: python tests/gun_ads_capture_test.py
"""
import json
import os
import re
import shutil
import subprocess
import tempfile
import types
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CAPTURE = ROOT / 'tools/real_game/gun_ads_capture.ps1'
PROBE = ROOT / 'tools/real_game/scripts/gun_ads_probe.py'
TRUSTED = ROOT / 'tools/real_game/phaselock_integrity_capture.ps1'


def text_of(path):
    return path.read_text(encoding='utf-8').replace('\r\n', '\n')


def finalizer(text):
    return text[text.index('\n} catch {\n    $runError') + 1:]


def probe_namespace(repo):
    made = []
    inventory = types.SimpleNamespace(AddBackpackWeaponFromDefinitionData=made.append)
    sdk = types.SimpleNamespace(find_object=lambda _kind, path: None if path == 'P.Missing' else path, make_struct=lambda _name, **f: f)
    pc = types.SimpleNamespace(GetPawnInventoryManager=lambda: inventory, Pawn=None, CachedFOVAngle=50.0)
    namespace = {'__name__': 'probe', 'unrealsdk': sdk, 'get_pc': lambda: pc, '_repo_root': lambda: Path(repo), 'made': made, 'pc': pc,
                 'wp_state': {'recipes': {}, 'spawned': set()}, '_deep': lambda value: value,
                 'definition_fields': lambda r: {'BalanceDefinition': r['balance'], 'GameStage': r['game_stage']},
                 '_find_recipe': lambda _id: made[-1] if made else None,
                 '_matches_recipe': lambda weapon, _r: weapon.kind == 'exact', '_loose_match': lambda weapon, _r: weapon.kind == 'parts'}
    exec(compile(text_of(PROBE), str(PROBE), 'exec'), namespace)
    return namespace


class ProbeTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.repo = temporary.name
        Path(self.repo, 'gun.json').write_text(json.dumps({'balance': 'P.Balance', 'game_stage': 30, 'parts': {}}), encoding='utf-8')

    def test_a_gun_is_made_at_the_asked_level_and_is_owned_by_the_probe(self):
        ns = probe_namespace(self.repo)
        self.assertEqual(ns['ads_seed']('gun', 'gun.json', 8), {'recipe_id': 'gun', 'created': True})
        self.assertEqual(ns['made'][0], {'BalanceDefinition': 'P.Balance', 'GameStage': 8})
        self.assertEqual(ns['wp_state']['spawned'], {'gun'})
        self.assertEqual(ns['ads_seed']('gun', 'gun.json', 8)['created'], False)
        self.assertEqual(len(ns['made']), 1)

    def test_an_unresolved_part_makes_nothing(self):
        Path(self.repo, 'gun.json').write_text(json.dumps({'balance': 'P.Missing', 'game_stage': 8}), encoding='utf-8')
        ns = probe_namespace(self.repo)
        with self.assertRaises(LookupError):
            ns['ads_seed']('gun', 'gun.json')
        self.assertEqual(ns['made'], [])

    def test_the_character_read_needs_the_skill_itself(self):
        ns = probe_namespace(self.repo)
        ns['pc'].Pawn = types.SimpleNamespace(GetExpLevel=lambda: 8)
        skill = lambda path: types.SimpleNamespace(_path_name=lambda: path)
        ns['unrealsdk'].find_all = lambda _name: [skill('Pkg.Default__LiftActionSkill')]
        self.assertEqual(ns['ads_character'](), {'is_maya': False, 'level': 8})
        ns['unrealsdk'].find_all = lambda _name: [skill('Pkg.Default__LiftActionSkill'), skill('Pkg.Lift')]
        self.assertTrue(ns['ads_character']()['is_maya'])

    def test_held_says_how_the_gun_matches(self):
        ns = probe_namespace(self.repo)
        ns['wp_state']['recipes']['gun'] = {}
        self.assertEqual(ns['ads_held']('gun'), 'no', 'nothing held')
        for kind, answer in (('exact', 'exact'), ('parts', 'parts'), ('other', 'no')):
            ns['pc'].Pawn = types.SimpleNamespace(Weapon=types.SimpleNamespace(kind=kind))
            self.assertEqual(ns['ads_held']('gun'), answer)

    def test_only_the_settled_state_reads_as_zoomed(self):
        ns = probe_namespace(self.repo)
        self.assertFalse(ns['ads_read']()['zoomed'], 'no gun')
        for state, zoomed in (('ZST_Zoomed', True), ('ZoomState.ZST_Zoomed', True), ('ZST_NotZoomed', False), ('ZST_Zooming', False)):
            ns['pc'].Pawn = types.SimpleNamespace(Weapon=types.SimpleNamespace(ZoomState=state, ZoomedRate=0.0, ZoomedFOV=50.0))
            self.assertEqual(ns['ads_read']()['zoomed'], zoomed, state)


class CaptureTests(unittest.TestCase):
    def test_the_finalizer_is_the_tested_one_less_its_hook_stop(self):
        trusted = finalizer(text_of(TRUSTED))
        start, end = trusted.index('    # A throw between pi_start and pi_stop'), trusted.index('    $gameGone = $true')
        self.assertEqual(finalizer(text_of(CAPTURE)), trusted[:start] + trusted[end:])

    def test_the_safety_helpers_are_called(self):
        text = text_of(CAPTURE)
        for call in ('Install-Driver -BlockSavesAtStart -Exclusive', "Assert-SavesBlocked (Py 'print(block_saves())')", 'Save-VerifiedBackupRecord $bak $roots $snapshots',
                     'Get-ForeignGamesSeen $null', 'Assert-SnapshotMatches', 'New-VerifiedSnapshot'):
            self.assertIn(call, text)
        self.assertLess(text.index('Backup-Saves'), text.index('Install-Driver'))
        self.assertLess(text.index('Install-Driver'), text.index('Start-Game'))
        self.assertLess(text.index('wp_cleanup'), text.index('ConsoleCommand("quit"'), 'spawned guns go before the quit')

    def test_input_is_guarded_and_the_zoom_press_always_released(self):
        text = text_of(CAPTURE)
        self.assertNotIn('mouse_event', text)
        self.assertNotIn('Focus-Game', text)
        self.assertEqual(len(re.findall(r'Send-MouseEvent 0x8\b', text)), 1)
        self.assertEqual(len(re.findall(r'Send-MouseEvent 0x10\b', text)), 1)
        block = text[text.index('function Save-AimedFrame'):text.index('$extraRoots = ')]
        self.assertLess(block.index('Assert-GameForeground'), block.index('Send-MouseEvent 0x8'))
        self.assertLess(block.index('Send-MouseEvent 0x8'), block.index('try {'))
        self.assertRegex(block, r'\} finally \{ Send-MouseEvent 0x10 \}')

    def test_a_bad_recipe_stops_before_any_lock_and_leaves_earlier_evidence(self):
        # A copy of the scripts in a temporary repository: output goes by the script's own place, so a real gun_ads folder is out of reach.
        with tempfile.TemporaryDirectory() as folder:
            repo = Path(folder) / 'repo'
            (repo / 'tools/real_game').mkdir(parents=True)
            for name in ('tools/ue_lock.ps1', 'tools/real_game/realgame.ps1', 'tools/real_game/gun_ads_capture.ps1'):
                shutil.copy(ROOT / name, repo / name)
            evidence = repo / 'local/realgame/gun_ads'
            evidence.mkdir(parents=True)
            for name in ('session.json', 'old_ads.png'):
                (evidence / name).write_text('earlier', encoding='utf-8')
            lock = Path(folder) / 'hub.lock'
            done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(repo / 'tools/real_game/gun_ads_capture.ps1'),
                                   '-HubLock', str(lock), '-Recipes', 'local/no/such_recipe.json'], capture_output=True, text=True, timeout=120,
                                  env=dict(os.environ, OPENWILLOW_BL2=folder))
            self.assertNotEqual(done.returncode, 0)
            self.assertIn('no recipe', done.stdout + done.stderr)
            self.assertFalse(lock.exists())
            self.assertEqual(sorted(p.name for p in evidence.iterdir()), ['old_ads.png', 'session.json'])

    def test_a_gun_frame_checks_the_foreground_and_a_failed_cleanup_fails_the_run(self):
        text = text_of(CAPTURE)
        self.assertEqual(len(re.findall(r'Save-Shot "gun_ads/\$', text)), 0, 'gun frames go through Save-GunFrame')
        self.assertEqual(len(re.findall(r'= Save-GunFrame "gun_ads/', text)), 2)
        frame = text[text.index('function Save-GunFrame'):text.index('function Wait-Unzoomed')]
        self.assertLess(frame.index('Assert-GameForeground'), frame.index('Save-Shot'))
        self.assertLess(frame.index('Save-Shot'), frame.index('Get-ForegroundWindowHandle'))
        self.assertIn("+ 'gun_cleanup'", text)
        self.assertIn("PyJson 'wp_cleanup()'", text)
        self.assertIn("Set-Stage 'gun_cleanup' $false", text)
        self.assertIn("$g.released = Wait-Unzoomed", text)
        self.assertLess(text.index('ads_character()'), text.index('foreach ($id in $guns.Keys)'))


if __name__ == '__main__':
    unittest.main()
