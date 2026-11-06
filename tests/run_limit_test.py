"""Synthetic test of tools/run_limit.ps1: the rule for which runs get a limit, and stopping a stalled process that the run started.
A sleeping Python stands in for the editor, so nothing heavy starts. Windows only (the launchers are)."""
import os
import subprocess
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
HELPER = REPO / 'tools' / 'run_limit.ps1'
LAUNCHER = REPO / 'tools' / 'run_ue_flash_hud.ps1'


def powershell(script):
    done = subprocess.run(['powershell', '-NoProfile', '-NonInteractive', '-Command', f". '{HELPER}'; {script}"],
                          capture_output=True, text=True, timeout=120)
    return done.stdout.strip(), done.returncode


def start_sleeper(marker):
    """Starts the stand-in editor from PowerShell (as the launcher does) and returns the script text naming it $p."""
    code = f'import time; time.sleep(60)  # {marker}'
    return (f"$p = Start-Process -PassThru -WindowStyle Hidden -FilePath '{sys.executable}' -ArgumentList @('-c', '\"{code}\"'); "
            "Start-Sleep -Milliseconds 800; ")


@unittest.skipUnless(os.name == 'nt', 'the launchers are Windows scripts')
class RunLimit(unittest.TestCase):
    def limit(self, arguments):
        return int(powershell(f'Get-OwRunLimitSeconds {arguments}')[0])

    def test_scripted_game_runs_get_the_default(self):
        for switch in ('-owinvshots', '-owcombatshots', '-owstatusshots=map', '-owfiringtest', '-owinventoryactions', '-owinvopenbench=3', '-owslicegate'):
            self.assertEqual(self.limit(f"-GameWindow -Extra '-owlevel=36','{switch}'"), 1200, switch)

    def test_a_gun_list_of_any_length_gets_longer(self):
        self.assertEqual(self.limit("-GameWindow -Extra '-owgunshots','-owgunids=a,b'"), 3600)
        self.assertEqual(self.limit("-TimeoutSeconds 90 -GameWindow -Extra '-owgunshots'"), 90)

    def test_people_are_not_stopped_by_a_clock(self):
        self.assertEqual(self.limit(''), 0)                                    # the editor
        self.assertEqual(self.limit('-Extra -owinvshots'), 0)                  # the editor waits for Play
        self.assertEqual(self.limit('-GameWindow'), 0)                         # playing
        self.assertEqual(self.limit("-GameWindow -Extra '-owlevel=36'"), 0)    # playing with a setup switch
        self.assertEqual(self.limit("-GameWindow -Extra '-owcombattest'"), 0)  # spawns a target for play, and does not quit
        self.assertEqual(self.limit("-GameWindow -Extra '-owinventoryselftest'"), 0)  # only runs checks, and does not quit

    def test_an_explicit_limit_wins(self):
        self.assertEqual(self.limit('-TimeoutSeconds 60 -GameWindow'), 60)
        self.assertEqual(self.limit('-TimeoutSeconds 0 -GameWindow -Extra -owinvshots'), 0)
        self.assertEqual(self.limit('-TimeoutSeconds 60'), 0)                  # never without -GameWindow: that is the maintainer's editor

    def scripted(self, extra):
        return powershell(f'Test-OwScriptedRun -Extra {extra}')[0] == 'True'

    def test_which_switches_count_as_a_scripted_run(self):
        for extra in ("'-owpickupcardshots'", "'-owinvshots'", "'-owcombatshots'", "'-owlevel=36','-owinvshots'", "'-owlevel=36,-owinvshots'", "'-owrangesurvey'", "'-owslicegate'"):
            self.assertTrue(self.scripted(extra), extra)
        for extra in ("'-owlevel=36'", "'-owitems=x','-owslots=2'", "'-owquest'", "''", "'-owcombattest'", "'-owinventoryselftest'", "'-owrangeshots'",
                      "'-owslicegate','-owslicegatestay'"):  # the gate does not quit with -stay; a person is watching it
            self.assertFalse(self.scripted(extra), extra)

    def run_launcher(self, *arguments):
        # Real launcher, fake folders: it must stop at the refusal, before the engine, the server or the lock matter.
        return subprocess.run(['powershell', '-NoProfile', '-NonInteractive', '-File', str(LAUNCHER), '-Engine', 'no-such-engine', '-Game', 'no-such-game', *arguments],
                              capture_output=True, text=True, timeout=120)

    def test_the_launcher_refuses_a_capture_without_a_game(self):
        done = self.run_launcher('-Extra', '-owpickupcardshots')
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('Add -GameWindow', ' '.join((done.stdout + done.stderr).split()))

    def test_a_stalled_process_is_reported_and_stopped_by_its_id(self):
        out, code = powershell(start_sleeper('owlimittest') +
            "[void]$p.Handle; $exited = Wait-OwStartedProcess -Process $p -LimitSeconds 1; "
            "$stopped = Stop-OwStartedProcess -Process $p -Expected @('owlimittest'); "
            "Write-Output \"$exited $stopped $($p.HasExited)\"")
        self.assertEqual(code, 0, out)
        self.assertEqual(out.split()[-3:], ['False', 'True', 'True'])

    def test_a_process_that_exits_in_time_is_reported_as_exited(self):
        out, _ = powershell(f"$p = Start-Process -PassThru -WindowStyle Hidden -FilePath '{sys.executable}' -ArgumentList @('-c', 'pass'); "
                            "Wait-OwStartedProcess -Process $p -LimitSeconds 30")
        self.assertEqual(out.split()[-1], 'True')

    def test_an_exit_code_can_be_read_after_the_process_has_gone(self):
        # The launcher tells a crash from a clean exit by this; PowerShell 5 reads it empty without the handle Wait-OwStartedProcess opens.
        out, _ = powershell(f"$p = Start-Process -PassThru -WindowStyle Hidden -FilePath '{sys.executable}' -ArgumentList @('-c', '\"import time, sys; time.sleep(1); sys.exit(3)\"'); "
                            "$exited = Wait-OwStartedProcess -Process $p -LimitSeconds 30; Write-Output \"$exited $($p.ExitCode)\"")
        self.assertEqual(out.split()[-2:], ['True', '3'])

    def test_the_wrong_command_line_is_left_running(self):
        out, _ = powershell(start_sleeper('owlimittest2') +
            "$stopped = Stop-OwStartedProcess -Process $p -Expected @('owlimittest2', 'not-this-process') -WarningAction SilentlyContinue; "
            "$alive = !$p.HasExited; Stop-Process -Id $p.Id -Force; Write-Output \"$stopped $alive\"")
        self.assertEqual(out.split()[-2:], ['False', 'True'])

    def test_the_launcher_uses_the_helper_and_parses(self):
        out, code = powershell(f"$e = $null; [void][System.Management.Automation.Language.Parser]::ParseFile('{LAUNCHER}', [ref]$null, [ref]$e); "
                               "Write-Output $e.Count")
        self.assertEqual((out, code), ('0', 0))
        source = LAUNCHER.read_text(encoding='utf-8')
        for piece in ('run_limit.ps1', 'Wait-OwStartedProcess', 'Stop-OwStartedProcess', '[int]$TimeoutSeconds = -1', '$process.ExitCode -eq 0'):
            self.assertIn(piece, source)
        self.assertNotRegex(source, r'Stop-Process\s+-Name|taskkill|\bpkill\b|\bkillall\b')


if __name__ == '__main__':
    unittest.main()
