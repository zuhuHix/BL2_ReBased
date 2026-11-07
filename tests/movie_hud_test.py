"""Tests the -MovieHud switch of the capture tools with no editor: the real tools/movie_hud.ps1 and serve.py, copied to temporary folders, and synthetic log lines.
The tools themselves launch an editor, so for them this checks the text: each still passes what it passed without the switch. Windows only. Stops only the
servers it started, through their own handles."""
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SOURCE = REPO / 'host' / 'ue5' / 'OpenWillow' / 'Source' / 'OpenWillow'
CANVAS_TOOLS = ('run_phaselock_shots', 'test_range', 'play_range', 'test_mission_page', 'test_mission_log')


def free_port():
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        return probe.getsockname()[1]


def serving(port):
    try:
        urllib.request.urlopen(f'http://127.0.0.1:{port}/', timeout=2)
        return True
    except OSError:
        return False


@unittest.skipUnless(os.name == 'nt', 'the helper is a Windows script')
class Helper(unittest.TestCase):
    def setUp(self):
        self.folder = Path(tempfile.mkdtemp(prefix='ow_moviehud_'))
        self.addCleanup(shutil.rmtree, self.folder, ignore_errors=True)
        self.tools = self.copy_tools(self.folder / 'a')
        self.port = free_port()

    def copy_tools(self, root):
        tools = root / 'tools'
        shutil.copytree(REPO / 'tools' / 'hud_overlay', tools / 'hud_overlay', ignore=shutil.ignore_patterns('__pycache__'))
        for name in ('movie_hud.ps1', 'run_limit.ps1'):
            shutil.copy(REPO / 'tools' / name, tools / name)
        # serve.py will not start without these; empty ones do, as in tests/run_ue_flash_hud_test.py.
        for name in ('ruffle/ruffle.js', 'UI_HUD/harness.swf', 'UI_HUD/HUD.swf'):
            movie = root / 'local' / 'ui' / 'run' / name
            movie.parent.mkdir(parents=True, exist_ok=True)
            movie.write_bytes(b'')
        return tools

    def powershell(self, body):
        script = self.folder / 'body.ps1'
        script.write_text(f"$ErrorActionPreference = 'Stop'\n. '{self.tools / 'movie_hud.ps1'}'\n{body}\n", encoding='utf-8')
        return subprocess.run(['powershell', '-NoProfile', '-NonInteractive', '-File', str(script)], capture_output=True, text=True, timeout=60)

    def verdict(self, log_text, arguments=''):
        log = self.folder / 'run.log'
        log.write_text(log_text, encoding='utf-8')
        done = self.powershell(f"if (Test-OwMovieHudLog -Log '{log}' {arguments}) {{ 'PROVED' }} else {{ 'REFUSED' }}")
        self.assertEqual(done.returncode, 0, done.stderr)
        return done.stdout

    def start_server(self, tools):
        server = subprocess.Popen([sys.executable, str(tools / 'hud_overlay' / 'serve.py'), '--port', str(self.port)],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.addCleanup(server.wait)
        self.addCleanup(server.kill)
        for _ in range(50):
            if serving(self.port):
                return server
            time.sleep(.1)
        self.fail('the test server did not start')

    def test_log_with_the_movie_is_proved(self):
        self.assertIn('PROVED', self.verdict('OpenWillow HUD path: movie (page reported ready)\nOWCAPTUREHUD ready driver=phaselock after 12.0 s\n'))

    def test_logs_without_it_are_refused_and_say_why(self):
        for text, why in (('nothing about a HUD\n', 'no "OpenWillow HUD path: movie" line'),
                          ('OpenWillow HUD path: canvas (page failed to load)\n', 'canvas (page failed to load)'),
                          ('OpenWillow HUD path: movie (page reported ready)\nOpenWillow HUD path: canvas (the movie was lost)\n', 'the movie was lost'),
                          ('OpenWillow HUD path: movie (page reported ready)\nLogTemp: Error: OWCAPTUREHUD FAIL driver=gunshots after 120.1 s: not ready in time; quitting\n', 'not ready in time')):
            output = self.verdict(text)
            self.assertIn('REFUSED', output, text)
            self.assertIn(why, output, text)

    def test_a_frame_must_follow_its_own_drivers_ready_line(self):
        movie = 'OpenWillow HUD path: movie (page reported ready)\n'
        ready = 'OWCAPTUREHUD ready driver=range after 12.0 s\n'
        shot = 'OWCAPTUREHUD shot driver=range file=a.png\n'
        self.assertIn('PROVED', self.verdict(movie + ready + shot, "-Driver 'range' -RequireShot"))
        output = self.verdict(movie + shot + ready, "-Driver 'range' -RequireShot")
        self.assertIn('REFUSED', output)
        self.assertIn('a frame before', output)
        # Another driver's ready line does not count, even with no -Driver named.
        output = self.verdict(movie + 'OWCAPTUREHUD ready driver=lid after 12.0 s\n' + shot)
        self.assertIn('REFUSED', output)
        self.assertIn('a frame before', output)

    def test_a_movie_lost_after_ready_is_refused(self):
        output = self.verdict('OpenWillow HUD path: movie (page reported ready)\nOWCAPTUREHUD ready driver=vendor after 12.0 s\n'
                              'LogTemp: Error: OWCAPTUREHUD FAIL driver=vendor movie lost: no frame a.png; quitting\n', "-Driver 'vendor'")
        self.assertIn('REFUSED', output)
        self.assertIn('movie lost', output)

    def test_a_named_driver_must_have_reached_the_gate(self):
        movie = 'OpenWillow HUD path: movie (page reported ready)\n'
        ready = 'OWCAPTUREHUD ready driver=range after 12.0 s\n'
        output = self.verdict(movie, "-Driver 'range'")
        self.assertIn('REFUSED', output)
        self.assertIn('OWCAPTUREHUD ready driver=range', output)
        self.assertIn('PROVED', self.verdict(movie + ready, "-Driver 'range'"))
        output = self.verdict(movie + ready, "-Driver 'range' -RequireShot")
        self.assertIn('REFUSED', output)
        self.assertIn('OWCAPTUREHUD shot driver=range', output)

    def test_a_missing_log_is_refused(self):
        done = self.powershell(f"if (Test-OwMovieHudLog -Log '{self.folder / 'none.log'}') {{ 'PROVED' }} else {{ 'REFUSED' }}")
        self.assertIn('REFUSED', done.stdout, done.stderr)

    def test_it_starts_a_server_and_stops_only_that(self):
        done = self.powershell(f"$m = Start-OwMovieHud -Port {self.port}\n$m.Url\nInvoke-WebRequest -UseBasicParsing -TimeoutSec 5 $m.Url | Out-Null\n'SERVED'\nStop-OwMovieHud $m\nStop-OwMovieHud $null\n'STOPPED'")
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn(f'http://127.0.0.1:{self.port}/', done.stdout)
        self.assertIn('SERVED', done.stdout)
        self.assertIn('STOPPED', done.stdout)
        self.assertFalse(serving(self.port), 'the server it started must be stopped')

    def test_a_server_of_these_pages_is_reused_and_left_running(self):
        server = self.start_server(self.tools)
        done = self.powershell(f"$m = Start-OwMovieHud -Port {self.port}\nif ($m.Server) {{ 'STARTED' }} else {{ 'REUSED' }}\nStop-OwMovieHud $m")
        self.assertIn('REUSED', done.stdout, done.stderr)
        self.assertIsNone(server.poll(), 'a server this run did not start is not its to stop')

    def test_another_checkouts_server_is_refused(self):
        server = self.start_server(self.copy_tools(self.folder / 'b'))
        done = self.powershell(f"Start-OwMovieHud -Port {self.port}")
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('serves other pages', ' '.join((done.stdout + done.stderr).split()))
        self.assertIsNone(server.poll())


class ToolText(unittest.TestCase):
    """The tools launch an editor, so these read them: the default run must keep what it passed before."""

    def text(self, name):
        return (REPO / 'tools' / f'{name}.ps1').read_text(encoding='utf-8')

    def test_the_canvas_stays_the_default(self):
        for name in CANVAS_TOOLS:
            text = self.text(name)
            self.assertIn('[switch]$MovieHud', text, name)
            self.assertEqual(len(re.findall(r"'-owcanvashud'", text)), 1, name)
            self.assertRegex(text, r"if \(\$MovieHud\).*-owflashhud=|'-owcanvashud'\s*\n\s*if \(\$MovieHud\)", name)
            self.assertIn(f"Test-OwMovieHudLog -Log $log -Driver ", text, name)
            self.assertIn("'-owmaya', $hudSwitch,", text, name)

    def test_the_kill_rows_time_limit_grows_only_with_the_switch(self):
        self.assertIn('$CheckSeconds = if ($MovieHud) { 305 } else { 180 }', self.text('play_range'))

    def test_the_weapon_tool_keeps_its_level_and_passes_no_hud_switch_by_default(self):
        text = self.text('run_weapon_assembly_shots')
        self.assertIn('[int]$Level = 40', text)
        self.assertIn('"-owlevel=$Level"', text)
        self.assertNotIn('-owcanvashud', text)
        self.assertRegex(text, r'if \(\$MovieHud\) \{ \$movie = Start-OwMovieHud')

    def test_every_tool_that_starts_a_server_stops_it(self):
        for name in ('run_phaselock_shots', 'test_range', 'play_range', 'run_weapon_assembly_shots'):
            self.assertIn('Stop-OwMovieHud $movie', self.text(name), name)


class HostText(unittest.TestCase):
    """The module has no test harness: what must hold for a capture's frames, read from the source. The in-engine runs prove the rest."""

    def source(self, name):
        return (SOURCE / name).read_text(encoding='utf-8')

    def test_each_driver_asks_the_gate_and_logs_its_frames(self):
        for name, driver in (('OpenWillowWalker.cpp', 'phaselock'), ('OpenWillowWalker.cpp', 'gunshots'), ('OpenWillowWalker.cpp', 'combat'),
                             ('OpenWillowWalkerFiringTests.cpp', 'firing'), ('OpenWillowInteractive.cpp', 'lid'), ('OpenWillowRange.cpp', 'range'),
                             ('OpenWillowVendor.cpp', 'vendor'), ('OpenWillowMissionPageTest.cpp', 'missionpage'),
                             ('OpenWillowMayaHUD.cpp', 'missionlog'), ('OpenWillowMayaHUD.cpp', 'inv')):
            source = self.source(name)
            self.assertRegex(source, rf'CaptureHudGate\([^;]*TEXT\("{driver}"\)\)', driver)
            self.assertIn(f'CaptureShot(TEXT("{driver}"), ', source, driver)

    def test_a_gated_drivers_frames_all_go_through_the_shot_line(self):
        for name, frames in (('OpenWillowWalker.cpp', ('OWGun_', 'OWCombat_')), ('OpenWillowWalkerFiringTests.cpp', ('OWFiring_',)), ('OpenWillowInteractive.cpp', ('OWLootLid_',)),
                             ('OpenWillowVendor.cpp', ('OWVendor_',)), ('OpenWillowMissionPageTest.cpp', ('OWMissionPage_',)),
                             ('OpenWillowMayaHUD.cpp', ('OWMission_', 'OWCombat_'))):
            for frame in frames:
                self.assertNotRegex(self.source(name), rf'RequestScreenshot\([^;]*{frame}', frame)
        self.assertNotIn('RequestScreenshot(Name, ', self.source('OpenWillowWalker.cpp').split('void AOpenWillowWalker::RunPhaselockShots')[1].split('float AOpenWillowWalker::PhaselockRemaining')[0])
        self.assertNotIn('RequestScreenshot', self.source('OpenWillowRange.cpp'))
        self.assertNotIn('RequestScreenshot(File', self.source('OpenWillowInteractive.cpp'))

    def test_a_shot_takes_no_frame_once_the_movie_is_lost(self):
        body = self.source('OpenWillowMayaHUD.cpp').split('void AOpenWillowMayaHUD::CaptureShot', 1)[1].split('\n}\n', 1)[0]
        check, shot = body.index('!Hud->IsFlashHudReady()'), body.index('RequestScreenshot')
        lost = body.index('OWCAPTUREHUD FAIL driver=%s movie lost')
        self.assertLess(check, lost)
        self.assertLess(lost, shot)
        self.assertLess(body.index('return;'), shot)
        self.assertLess(body.index('CaptureAskedForMovie()'), check, 'without -owflashhud nothing changes')

    def test_the_gate_only_applies_when_a_movie_was_asked_for(self):
        body = self.source('OpenWillowMayaHUD.cpp').split('AOpenWillowMayaHUD::CaptureHudGate', 1)[1]
        self.assertIn('if (!CaptureAskedForMovie()) return EOpenWillowCaptureHud::Go;', body.split('static constexpr', 1)[0])

    def test_the_shield_function_is_shared(self):
        self.assertEqual(self.source('OpenWillowShieldCheck.cpp').count('EquipGearById'), 1)
        self.assertNotIn('Shield Check Stand-in', self.source('OpenWillowMayaHUD.cpp'))
        self.assertIn('OpenWillowEquipStandInShield(*Walker)', self.source('OpenWillowMayaHUD.cpp'))
        self.assertIn('OpenWillowEquipStandInShield(Maya)', self.source('OpenWillowShieldCheck.cpp'))


if __name__ == '__main__':
    unittest.main()
