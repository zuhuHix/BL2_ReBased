"""Tests tools/play_slice.ps1 with no editor: the real launcher and the helpers it calls, copied to a temporary repository with synthetic local data
(empty or minimal files; nothing game-derived). -PrintCommandLine shows the line it would start and -GradeLog grades a synthetic log, so no lock,
server or editor is ever started here. Windows only (the launcher is a PowerShell script)."""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SOURCE = REPO / 'host' / 'ue5' / 'OpenWillow' / 'Source' / 'OpenWillow'
PAGES = REPO / 'tools' / 'hud_overlay'
sys.path.insert(0, str(REPO / 'tools'))
import check_overlay_data  # noqa: E402

# The data the launcher needs, and a word of the tool its refusal must name.
DATA = {
    'local/doors/mover.json': 'prepare_mover.py',
    'local/slice/world.json': 'prepare_slice_world.py',
    'local/slice/npc_assets.json': 'seed_slice_npc_assets.ps1',
    'local/slice/audio.json': 'audio_slice_chain.py',
    'local/items/slice/slice_manifest.json': 'weapon_slice_gear.py',
    'local/character/action_skill_siren.json': 'prepare_action_skill.py',
    'local/slice/audio_play.json': 'prepare_slice_audio.py',
    'local/range/range_manifest.json': 'prepare_range.py',
    'local/slice/stand_in_shield.json': 'COMBAT_DOWN_RESPAWN.md',
}
# A range manifest the preflight accepts (tools/check_range.py): a default candidate and a scout centre, no spawns.
RANGE = {'spot_candidates': [{'x': 0}], 'default_spot_candidate': 0, 'scout_centre': {'x': 0, 'y': 0}}
# Switches and console commands that drive the game or stand in for play. None may be on the line.
FIXTURES = ('owslicegate', 'owcombattest', 'owcombatshots', 'owrangeloot', 'owrangetest', 'owrangesurvey', 'owrangekillrow', 'owrangeat',
            'owquesttest', 'owquestresume', 'ownostartshield', 'owstandinshield', 'owshieldcheck', 'owcanvashud', 'execcmds', 'ow.range', 'ow.rollgun')

# One line per smoke item, as the host and the pages write them (the source check below keeps these honest).
GOOD_LOG = """LogTemp: Display: OWQUEST READY mission=M status=0 trigger=T r=1.00 hh=1.00 walk_nodes=1 stations=1 audio_entries=1
LogTemp: Display: OWRANGE start switch: built=1
LogTemp: Display: OpenWillow Phaselock data: lift 0.70 s
LogTemp: Display: OWAUDIO role=fire files=5
LogTemp: Display: OpenWillow Maya shield Stand-in: capacity 383.0 (stand-in rule, UNVERIFIED), recharge 25.00/s after 1.000 s
LogTemp: Display: OpenWillow Skills console: OpenWillow Inventory movie ready in 6949ms (inventory.js:1)
LogTemp: Display: OpenWillow Skills console: OpenWillow Skills movie ready: 3 branches, 30 skills (skills.js:1)
LogTemp: Display: OpenWillow Skills console: OWINVTIME js_missions_ready pageMs=6131 (missions.js:1)
LogTemp: Display: OpenWillow mission page console: OWMISSIONREADY (mission_screen.js:89)
LogTemp: Display: OpenWillow HUD path: movie (page reported ready)
"""
ITEMS = ('mission', 'range', 'phaselock_data', 'audio_map', 'start_shield', 'page_inventory', 'page_skills', 'page_missions', 'page_mission_screen', 'page_hud')
BAD_LINES = {
    'mission': 'OWQUEST ERROR no mission',
    'range': 'OWRANGE start switch: built=0',
    'phaselock_data': 'OpenWillow Phaselock unavailable: cannot read x',
    'audio_map': 'OWAUDIO play map refused: x: too big',
    'start_shield': 'OpenWillow Maya starts with no shield: no rule file',
    'page_mission_screen': 'OpenWillow mission page failed to load (is tools/hud_overlay/serve.py running?)',
    'page_hud': 'OpenWillow HUD path: canvas (page failed to load)',
}


@unittest.skipUnless(os.name == 'nt', 'the launcher is a Windows script')
class PlaySlice(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix='ow_playslice_'))
        self.addCleanup(shutil.rmtree, self.root, ignore_errors=True)
        tools = self.root / 'tools'
        tools.mkdir()
        for name in ('play_slice.ps1', 'movie_hud.ps1', 'run_limit.ps1', 'ue_lock.ps1', 'check_range.py', 'check_overlay_data.py'):
            shutil.copy(REPO / 'tools' / name, tools / name)
        for relative in DATA:
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('{}')
        (self.root / 'local/items/slice/slice_manifest.json').write_text(json.dumps({'level': 8}))
        (self.root / 'local/range/range_manifest.json').write_text(json.dumps(RANGE))
        for name, _, _ in check_overlay_data.REQUIRED:
            movie = self.root / 'local/ui/run' / name
            movie.parent.mkdir(parents=True, exist_ok=True)
            movie.write_bytes(b'')
        self.game = self.root / 'game'
        self.game.mkdir()

    def run_script(self, *arguments):
        return subprocess.run(['powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', str(self.root / 'tools/play_slice.ps1'),
                               '-Game', str(self.game), *arguments], capture_output=True, text=True, timeout=120)

    def command_line(self, *arguments):
        result = self.run_script('-PrintCommandLine', *arguments)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        lines = result.stdout.splitlines()
        start = next(i for i, line in enumerate(lines) if line.startswith('PLAY SLICE COMMAND LINE'))
        return lines[start], [line.strip() for line in lines[start + 1:] if line.startswith('  ')]

    def assert_no_fixture(self, line):
        for argument in line:
            name = argument.lstrip('-').split('=')[0].strip('"').lower()
            for fixture in FIXTURES:
                self.assertFalse(name.startswith(fixture), f'{argument} is a test switch or fixture')
        # Every switch tools/run_limit.ps1 lists as one that drives a run and quits.
        listed = re.search(r'\$script:OwScriptedSwitches = @\((.*?)\)', (REPO / 'tools/run_limit.ps1').read_text(), re.S).group(1)
        drivers = set(re.findall(r"'(\w+)'", listed))
        self.assertGreater(len(drivers), 10)
        names = {argument.lstrip('-').split('=')[0].lower() for argument in line}
        self.assertEqual(names & drivers, set())

    def test_play_line(self):
        title, line = self.command_line()
        self.assertIn('(play)', title)
        for switch in ('-owwalk', '-owmaya', '-owquest', '-owrangestart', '-owrangemode=normal', '-owlevel=8', '-game', '-d3d11'):
            self.assertIn(switch, line)
        for prefix in ('-owmover=', '-owslice=', '-ownpcs=', '-owaudio=', '-owitems=', '-owactionskill=', '-owrange=', '-owaudioplay=', '-owquestsave=', '-abslog='):
            self.assertEqual(len([a for a in line if a.startswith(prefix)]), 1, prefix)
        self.assertIn('-owflashhud=http://127.0.0.1:8798/', line)
        self.assertIn('-owflashskills=http://127.0.0.1:8798/skills.html', line)
        self.assertIn('-owflashinventory=http://127.0.0.1:8798/inventory.html', line)
        self.assertTrue(any(a.startswith('-owaudioplay=') and 'audio_play.json' in a for a in line))
        self.assertTrue(any(a.startswith('-owquestsave=') and a.rstrip('"').endswith('save.json') and 'smoke' not in a for a in line))
        self.assertNotIn('-unattended', line)
        self.assert_no_fixture(line)

    def test_smoke_line(self):
        title, line = self.command_line('-CheckSeconds', '180', '-Level', '5', '-Port', '8800')
        self.assertIn('(smoke)', title)
        self.assertIn('-unattended', line)
        self.assertIn('-owlevel=5', line)
        self.assertIn('-owflashskills=http://127.0.0.1:8800/skills.html', line)
        self.assertTrue(any(a.startswith('-owquestsave=') and 'smoke-save.json' in a for a in line))
        self.assertTrue(any(a.startswith('-abslog=') and 'smoke-' in a for a in line))
        self.assert_no_fixture(line)

    def test_each_missing_file_is_refused_by_name(self):
        for relative, tool in DATA.items():
            with self.subTest(relative):
                path = self.root / relative
                saved = path.read_bytes()
                path.unlink()
                try:
                    result = self.run_script('-PrintCommandLine')
                finally:
                    path.write_bytes(saved)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                refused = [line for line in result.stdout.splitlines() if line.startswith('PLAY SLICE REFUSED')]
                self.assertEqual(len(refused), 1, result.stdout)
                self.assertIn(Path(relative).name, refused[0])
                self.assertIn(tool, refused[0])
                self.assertNotIn('PLAY SLICE COMMAND LINE', result.stdout)

    def test_every_missing_piece_is_named_at_once(self):
        for relative in ('local/slice/audio_play.json', 'local/range/range_manifest.json'):
            (self.root / relative).unlink()
        result = self.run_script('-PrintCommandLine')
        self.assertEqual(result.returncode, 2)
        self.assertEqual(len([line for line in result.stdout.splitlines() if line.startswith('PLAY SLICE REFUSED')]), 2, result.stdout)

    def test_unusable_range_manifest_is_refused(self):
        (self.root / 'local/range/range_manifest.json').write_text(json.dumps({'spot_candidates': []}))
        result = self.run_script('-PrintCommandLine')
        self.assertEqual(result.returncode, 2)
        self.assertRegex(result.stdout, r'PLAY SLICE REFUSED: range manifest not usable.*prepare_range\.py')

    def test_missing_movie_is_refused(self):
        (self.root / 'local/ui/run/UI_StatusMenu/StatusMenu.swf').unlink()
        result = self.run_script('-PrintCommandLine')
        self.assertEqual(result.returncode, 2)
        self.assertRegex(result.stdout, r'PLAY SLICE REFUSED: overlay data missing:.*StatusMenu\.swf')

    def test_missing_optional_movie_is_only_noted(self):
        (self.root / 'local/ui/run/UI_Mission/MissionInterface.swf').unlink()
        result = self.run_script('-PrintCommandLine')
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn('Note: OVERLAY DATA MISSING: UI_Mission/MissionInterface.swf', result.stdout)

    def test_missing_game_is_refused(self):
        result = subprocess.run(['powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', str(self.root / 'tools/play_slice.ps1'),
                                 '-PrintCommandLine', '-Game', str(self.root / 'nowhere')], capture_output=True, text=True, timeout=120)
        self.assertEqual(result.returncode, 2)
        self.assertIn('PLAY SLICE REFUSED: no Borderlands 2 folder', result.stdout)

    def grade(self, text):
        log = self.root / 'smoke.log'
        log.write_text(text)
        result = self.run_script('-GradeLog', str(log))
        return result, {name: verdict for verdict, name in re.findall(r'^(PASS|FAIL) (\w+)', result.stdout, re.M)}

    def test_good_log_passes_every_item(self):
        result, verdicts = self.grade(GOOD_LOG)
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(verdicts, {name: 'PASS' for name in ITEMS})

    def test_each_missing_line_fails_only_its_item(self):
        lines = GOOD_LOG.splitlines()
        for index, name in enumerate(ITEMS):
            with self.subTest(name):
                result, verdicts = self.grade('\n'.join(lines[:index] + lines[index + 1:]) + '\n')
                self.assertEqual(result.returncode, 1)
                self.assertEqual({item for item, verdict in verdicts.items() if verdict == 'FAIL'}, {name})
                self.assertEqual(len(verdicts), len(ITEMS))

    def test_a_failure_line_fails_its_item(self):
        for name, line in BAD_LINES.items():
            with self.subTest(name):
                result, verdicts = self.grade(GOOD_LOG + 'LogTemp: Warning: ' + line + '\n')
                self.assertEqual(result.returncode, 1)
                self.assertEqual(verdicts[name], 'FAIL', result.stdout)

    def test_empty_shield_and_audio_do_not_pass(self):
        text = GOOD_LOG.replace('files=5', 'files=0').replace('capacity 383.0', 'capacity 0.0')
        result, verdicts = self.grade(text)
        self.assertEqual(verdicts['audio_map'], 'FAIL')
        self.assertEqual(verdicts['start_shield'], 'FAIL')


class LogLinesExist(unittest.TestCase):
    """Each line the smoke waits for is one the host or a page really writes: a renamed log line fails here, not in an unattended run."""
    def test_lines_are_in_the_source(self):
        host = ''.join(path.read_text(encoding='utf-8', errors='replace') for path in SOURCE.glob('*.cpp'))
        pages = ''.join(path.read_text(encoding='utf-8', errors='replace') for path in PAGES.glob('*.js'))
        for fragment in ('OWQUEST READY mission=', 'OWQUEST ERROR', 'OWRANGE start switch: built=', 'OpenWillow Phaselock data: ',
                         'OpenWillow Phaselock unavailable', 'OWAUDIO role=%s files=%d', 'OWAUDIO play map refused', 'OWAUDIO play map unreadable',
                         'OpenWillow Maya shield %s: capacity', 'OpenWillow Maya starts with no shield', 'OpenWillow Skills console: %s',
                         'OpenWillow mission page console: %s', 'OpenWillow mission page failed to load', 'OpenWillow HUD path: movie (page reported ready)'):
            self.assertIn(fragment, host)
        for fragment in ('OpenWillow Inventory movie ready', 'OpenWillow Skills movie ready', 'OWINVTIME js_missions_ready', "'OWMISSIONREADY'"):
            self.assertIn(fragment, pages)
        script = (REPO / 'tools/play_slice.ps1').read_text(encoding='utf-8')
        for good in ('OWQUEST READY mission=', 'OWRANGE start switch: built=1', 'OpenWillow Phaselock data: ', 'OWAUDIO role=', 'OpenWillow Maya shield ',
                     'OpenWillow Inventory movie ready', 'OpenWillow Skills movie ready', 'OWINVTIME js_missions_ready', 'OpenWillow mission page console: OWMISSIONREADY'):
            self.assertIn(f"Good = '{good}", script)


if __name__ == '__main__':
    unittest.main()
