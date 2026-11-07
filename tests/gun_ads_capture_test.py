"""Synthetic checks for the real-game ADS frame capture (no game: the SDK and the namespace it expects are stubbed; names are invented).

Run: python tests/gun_ads_capture_test.py
"""
import json
import os
import math
import re
import shutil
import subprocess
import sys
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

    def test_the_fov_fit_reads_the_level_row_and_refuses_too_little(self):
        ns = probe_namespace(self.repo)
        for hfov in (77.55, 50.84, 110.0):
            focal = 640.0 / math.tan(math.radians(hfov) / 2.0)
            rows = [{'yaw': yaw, 'pitch': 0.0, 'x': 640.0 + focal * math.tan(math.radians(yaw)), 'y': 360.0} for yaw in (-20, -10, 0, 10, 20)]
            rows += [{'yaw': 0.0, 'pitch': 15, 'x': 640.0, 'y': 100.0}, {'yaw': 0.0, 'pitch': -15, 'x': 640.0, 'y': 620.0}]
            self.assertAlmostEqual(ns['ads_fit_fov'](rows, 1280.0), hfov, places=6)
        self.assertIsNone(ns['ads_fit_fov']([], 1280.0))
        self.assertIsNone(ns['ads_fit_fov']([{'yaw': 0.0, 'pitch': 0.0, 'error': 'x'}], 1280.0))
        flipped = [{'yaw': yaw, 'pitch': 0.0, 'x': 640.0 - yaw} for yaw in (-20, 0, 20)]
        self.assertIsNone(ns['ads_fit_fov'](flipped, 1280.0))

    def test_the_fov_hook_measures_once_never_raises_and_goes_away(self):
        ns = probe_namespace(self.repo)
        added, removed = [], []
        hooks = types.ModuleType('unrealsdk.hooks')
        hooks.Type = types.SimpleNamespace(POST=1)
        hooks.add_hook = lambda path, _type, _name, function: added.append((path, function))
        hooks.remove_hook = lambda path, _type, _name: removed.append(path)
        sys.modules['unrealsdk.hooks'] = hooks
        self.addCleanup(sys.modules.pop, 'unrealsdk.hooks', None)
        self.assertEqual(len(ns['ads_fov_arm']()['hooked']), 2)
        callback = added[0][1]
        callback(types.SimpleNamespace(Canvas=None), None, None, None)
        self.assertIsNone(ns['ads_fov_state']['result'], 'no canvas: waits for a frame that has one')
        callback(types.SimpleNamespace(Canvas=object()), None, None, None)   # the stub controller has no camera
        self.assertIn('error', ns['ads_fov_state']['result'])
        first = ns['ads_fov_state']['result']
        callback(types.SimpleNamespace(Canvas=object()), None, None, None)
        self.assertIs(ns['ads_fov_state']['result'], first, 'one frame only')
        self.assertEqual(ns['ads_fov_read'](), dict(first, fired=3))
        self.assertEqual(len(removed), 2)
        self.assertEqual(ns['ads_fov_state']['hooks'], [])

    def test_a_hook_that_never_fires_says_so(self):
        ns = probe_namespace(self.repo)
        hooks = types.ModuleType('unrealsdk.hooks')
        hooks.Type = types.SimpleNamespace(POST=1)
        hooks.add_hook = hooks.remove_hook = lambda *a: None
        sys.modules['unrealsdk.hooks'] = hooks
        self.addCleanup(sys.modules.pop, 'unrealsdk.hooks', None)
        ns['ads_fov_arm']()
        self.assertEqual(ns['ads_fov_read'](), {'fired': 0})

    def test_the_turn_is_read_back_from_the_controller(self):
        ns = probe_namespace(self.repo)
        pov = ns['pc']
        pov.Rotation = types.SimpleNamespace(Yaw=16384)
        pov.PlayerCamera = None   # as in the game: the read must not go through it
        turns = []
        ns['pc'].ClientSetRotation = lambda rotator: turns.append(rotator)
        ns['wp_state']['view'] = {'yaw': 359.0}
        ns['ads_turn'](4)
        self.assertEqual(turns[-1], {'Pitch': 0, 'Yaw': int(363.0 * 65536 / 360) % 65536, 'Roll': 0}, 'wraps into 16 bits')
        self.assertEqual(ns['ads_view'](), {'yaw_units': 16384})
        four = int(4 * 65536 / 360)
        self.assertFalse(ns['ads_turned_by'](16384, 4)['moved'], 'the camera did not move')
        pov.Rotation.Yaw = 16384 + four
        self.assertTrue(ns['ads_turned_by'](16384, 4)['moved'])
        pov.Rotation.Yaw = (65000 + four) % 65536
        self.assertTrue(ns['ads_turned_by'](65000, 4)['moved'], 'across the wrap')
        pov.Rotation.Yaw = 16384 + 3 * four
        self.assertFalse(ns['ads_turned_by'](16384, 4)['moved'], 'too far is not the measured turn')

    def test_a_getter_script_cannot_call_reads_as_none(self):
        ns = probe_namespace(self.repo)
        self.assertEqual(ns['ads_foreground'](), {'foreground_fov': None, 'callable': False})
        ns['pc'].GetFOVAngleForeground = lambda: 45.0
        self.assertEqual(ns['ads_foreground'](), {'foreground_fov': 45.0, 'callable': True})

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
        block = text[text.index('function Invoke-WhileZoomed'):text.index('function Save-AimedFrame')]
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
        self.assertEqual(len(re.findall(r'= Save-GunFrame "gun_ads/', text)), 4)
        self.assertNotIn('Save-Shot "gun_ads/fov', text)
        frame = text[text.index('function Save-GunFrame'):text.index('function Wait-Unzoomed')]
        self.assertLess(frame.index('Assert-GameForeground'), frame.index('Save-Shot'))
        self.assertLess(frame.index('Save-Shot'), frame.index('Get-ForegroundWindowHandle'))
        self.assertIn("+ 'gun_cleanup'", text)
        self.assertIn("PyJson 'wp_cleanup()'", text)
        self.assertIn("Set-Stage 'gun_cleanup' $false", text)
        self.assertIn("$g.released = Wait-Unzoomed", text)
        self.assertLess(text.index('ads_character()'), text.index('foreach ($id in $guns.Keys)'))

    def test_the_fov_stage_is_asked_for_only_with_its_switch_and_cleans_up_after_itself(self):
        text = text_of(CAPTURE)
        self.assertIn('$(if ($MeasureFov) { @($guns.Keys | ForEach-Object { "${_}_fov" }) } else { @() })', text)
        self.assertIn("if ($reply -match 'Traceback') { throw", text[text.index('function PyJson'):text.index('function Q(')])
        self.assertIn('$turned = ($g.fov_hip.turned.moved -eq $true) -and ($g.fov_zoom.measured.turned.moved -eq $true)', text)
        self.assertIn('Set-Stage "${id}_fov" ($states -and $turned)', text)
        measure = text[text.index('function Measure-ViewFov'):text.index('$Recipes') if text.index('$Recipes') > text.index('function Measure-ViewFov') else text.index('$extraRoots = ')]
        self.assertRegex(measure, r"try \{ Start-Sleep -Milliseconds 500 \} finally \{ \$result\.canvas = PyJson 'ads_fov_read\(\)' \}")
        self.assertRegex(measure, r"finally \{ PyJson 'ads_turn\(0\)' \| Out-Null \}")


if __name__ == '__main__':
    unittest.main()
