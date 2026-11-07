"""Synthetic tests for the mission screen capture (no game, no game files): the end of its run is stubbed the way
tests/realgame_snapshot_test.py does for the Phaselock capture, and the probe's game-free helpers run on invented values. Needs Windows PowerShell.

Run: python tests/mission_screen_capture_test.py
"""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tests'))
import realgame_snapshot_test as base  # noqa: E402

CAPTURE = ROOT / 'tools/real_game/mission_screen_capture.ps1'


def run_tail(body, clean=True, proc='$null'):
    """The capture's own end (its catch and finally), after a stubbed body; 'hit' is the one requested stage in the harness."""
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    tail = text[text.index('\n} catch {\n    $runError') + 1:]
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'driver.ps1'
        script.write_text(base.TAIL.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__TMP__', tmp)
                          .replace('__OUT__', tmp).replace('__SETUP__', '').replace('__LAUNCHED__', '$true').replace('__OWNED__', '$true').replace('__ATTEMPTED__', '$true').replace('__STUBS__', 'function Remove-Driver { }').replace('__BODY__', body).replace('__CAPTURE_TAIL__', tail).replace('__PROC__', proc)
                          .replace('__CLEAN__', '$true' if clean else '$false'), encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True,
                              text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp), timeout=180)
        done.hub_lock_kept = (Path(tmp) / 'hub.lock').exists()
        return done


def test_exit_0_only_for_a_completed_run_with_nothing_to_recover():
    finish = "$stagesDone.Add('hit')"
    never_exits = '[pscustomobject]@{ HasExited = $false; Id = 1 }'
    for body, clean, proc, code, status in ((finish, True, '$null', 0, 'completed'),
                                            (finish, False, '$null', 1, 'recovery needed: restore'),
                                            ("'no stage'", True, '$null', 1, 'incomplete: hit'),
                                            ("throw 'x'", True, '$null', 1, 'failed: x'),
                                            (finish, True, never_exits, 1, 'recovery needed: game exit')):
        done = run_tail(body, clean, proc)
        assert done.returncode == code and f'status: {status}' in done.stdout, (body, clean, proc, done.returncode, done.stdout, done.stderr)
        assert done.hub_lock_kept is status.startswith('recovery'), (body, clean, proc, done.stdout)
        # The real finalizer prints the one recovery text, and only when there is something to recover.
        assert ('MANUAL RECOVERY' in done.stdout) is status.startswith('recovery'), (body, clean, proc, done.stdout)


def test_the_capture_is_wired_to_the_safety_helpers():
    text = CAPTURE.read_text(encoding='utf-8')
    for call in ('$snapshots[$d] = Assert-SnapshotMatches', '$snapshots[$d] = New-VerifiedSnapshot', 'Restore-VerifiedRoots $bak $roots $snapshots',
                 'Save-VerifiedBackupRecord $bak $roots $snapshots', 'Get-RunStatus $stagesRequested', "$log.block_saves = Assert-SavesBlocked (Py 'print(block_saves())')",
                 'Get-RecoveryReason $gameGone', '$holdLocks = [bool]$recovery', 'Format-ManualRecovery $recovery $bak $HubLock $Lane',
                 'Install-Driver -BlockSavesAtStart -Exclusive', 'if ($holdLocks) { $runLock = $false; $hub = $null }', 'Format-RestoreRows $rows',
                 "if ($log.status -ne 'completed') { exit 1 }", "sdk_settings = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/settings')",
                 "SanitySaver = (Join-Path $env:OPENWILLOW_BL2 'sdk_mods/SanitySaver')"):
        assert call in text, call
    assert '-Name Borderlands2' not in text and 'taskkill' not in text
    # Nothing changes a file before the driver install and the game start: a run with neither skips the restore.
    install = text.index('Install-Driver -BlockSavesAtStart -Exclusive')
    start = text.index('$proc = Start-Game $gameArgs')
    assert install < start < text.index("$log.block_saves = Assert-SavesBlocked")
    assert text.index('$log.block_saves = Assert-SavesBlocked') < text.index('Send-Key Enter')
    body = text.index('try {\n    # Preflight')
    for change in ('Send-Key', 'ms_advance', 'ms_force_ready', 'ms_down', 'ms_start'):
        assert text.index(change, body) > start, change
    # The sends check focus themselves. Focus-Game's Alt tap would itself be a key press at an open screen, so the stages only assert.
    assert 'Focus-Game' not in text


def test_the_finalizer_is_the_hardened_one_with_only_the_hook_stop_changed():
    def tail_of(path):
        text = path.read_text(encoding='utf-8').replace('\r\n', '\n')
        return text[text.index('\n} catch {\n    $runError') + 1:]
    mine = tail_of(CAPTURE)
    theirs = tail_of(ROOT / 'tools/real_game/phaselock_integrity_capture.ps1')
    for a, b in (('pi_stop(); unmark_lock(); print("stopped")', 'ms_stop(); print("stopped")'),
                 ('A throw between pi_start and pi_stop leaves the per-frame hook reading a held pawn into shutdown: stop it while the game can still answer.',
                  'A throw between ms_start and ms_stop leaves the hooks and the per-frame down job running into shutdown: stop them while the game can still answer.'),
                 ("so -ProbeOnly's return passes through it too", 'so an early return passes through it too')):
        theirs = theirs.replace(a, b)
    assert mine == theirs
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    # The order in the body and at the top.
    order = ['New-Item -ItemType Directory -Force $Out', '# Stale evidence', '# End of stale evidence', '$driverAttempted = $false', '$driverAttempted = $true', 'Install-Driver -BlockSavesAtStart -Exclusive',
             '$foreign = Get-ForeignGamesSeen $null', '$launchAttempted = $true', '$proc = Start-Game $gameArgs', "Assert-SavesBlocked (Py 'print(block_saves())')"]
    places = [text.index(needle) for needle in order]
    assert places == sorted(places), list(zip(order, places))


def test_the_capture_parses_and_every_stage_is_marked():
    command = ("$e = $null; [void][Management.Automation.Language.Parser]::ParseFile('" + str(CAPTURE) + "', [ref]$null, [ref]$e); "
               "if ($e) { $e | ForEach-Object { $_.Message }; exit 1 }")
    done = subprocess.run(['powershell', '-NoProfile', '-Command', command], capture_output=True, text=True, timeout=60)
    assert done.returncode == 0, done.stdout + done.stderr
    text = CAPTURE.read_text(encoding='utf-8')
    requested = text[text.index('$stagesRequested = @('):].split('\n')[0]
    for stage in ('start', 'offer', 'offer_esc', 'accept', 'ready', 'turnin', 'reward', 'reward_taken', 'closed', 'respawn'):
        assert f"'{stage}'" in requested, stage
        assert f"Set-Stage '{stage}'" in text, stage


def load_probe():
    """The probe with its game modules stubbed; only the helpers that need no game are called."""
    sdk = types.ModuleType('unrealsdk')
    hooks = types.ModuleType('unrealsdk.hooks')
    hooks.Type = types.SimpleNamespace(PRE=0, POST=1)
    hooks.add_hook = hooks.remove_hook = lambda *a: None
    sys.modules.update({'unrealsdk': sdk, 'unrealsdk.hooks': hooks})
    spec = importlib.util.spec_from_file_location('mission_screen_probe', ROOT / 'tools/real_game/scripts/mission_screen_probe.py')
    probe = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(probe)
    return probe


class Obj:
    def __init__(self, path='Transient.Movie_1'):
        self.path = path

    def _path_name(self):
        return self.path


FUNC = types.SimpleNamespace(func=types.SimpleNamespace(_properties=lambda: []))


def fresh(probe):
    probe.MS.update(rows=[], t0=0.0, on=True, movie_path=None, down=None, missing=[], cash_before=None)


def hook(probe, name, tag, ret=None, obj=None, cls='WillowGame.QuestAcceptGFxMovie'):
    probe._ms_mk(f'{cls}:{name}', tag)(obj or Obj(), types.SimpleNamespace(), ret, FUNC)


def read(probe, **fields):
    probe.MS['rows'].append({'read': fields})


def called(probe, name, cls='WillowPlayerController', **args):
    probe.MS['rows'].append({'t': 0, 'tag': 'pre', 'func': f'{cls}:{name}', 'args': args})


def test_the_movie_is_a_path_cleared_on_both_close_paths():
    probe = load_probe()
    for closer, tag in (('OnClose', 'post'), ('OnClosedOnDeath', 'pre')):
        fresh(probe)
        hook(probe, 'Start', 'post', True)
        assert probe.MS['movie_path'] == 'Transient.Movie_1' and 'movie' not in {k for k, v in probe.MS.items() if isinstance(v, Obj)}
        hook(probe, closer, tag)
        assert probe.MS['movie_path'] is None, closer
    # A Start that refused does not count as open.
    fresh(probe)
    hook(probe, 'Start', 'post', False)
    assert probe.MS['movie_path'] is None
    # The object is found again for each use, and a movie that is gone reads as closed instead of being used.
    fresh(probe)
    probe.MS['movie_path'] = 'Transient.Movie_1'

    def gone(*a):
        raise ValueError('no such object')
    sys.modules['unrealsdk'].find_object = gone
    probe.unrealsdk = sys.modules['unrealsdk']
    assert probe._ms_movie() is None and probe.MS['movie_path'] is None
    assert probe.ms_settings() == {'error': 'the screen is not open'}
    text = (ROOT / 'tools/real_game/scripts/mission_screen_probe.py').read_text(encoding='utf-8')
    assert 'MS["movie"]' not in text


def test_a_mission_status_alone_is_not_a_reward_page():
    probe = load_probe()
    page = {'reward.rewardsHeader.text': 'H', 'reward.missionTitle.text': 'T'}
    fresh(probe)
    probe.ms_mark('turnin_enter')
    read(probe, status=4, open=True, text={})     # complete, screen open, nothing drawn: the old check passed this
    assert not probe.ms_reward_shown()['ok']
    called(probe, 'DisplayRewardsPage', 'QuestAcceptGFxMovie')
    read(probe, status=4, open=False, text=page)
    assert not probe.ms_reward_shown()['ok']
    read(probe, status=4, open=True, text=page)
    assert probe.ms_reward_shown()['ok']
    # A page call before the Enter does not count.
    fresh(probe)
    called(probe, 'DisplayRewardsPage', 'QuestAcceptGFxMovie')
    probe.ms_mark('turnin_enter')
    read(probe, status=4, open=True, text=page)
    assert not probe.ms_reward_shown()['ok']
    # A reward taken already is not a page waiting.
    fresh(probe)
    probe.ms_mark('turnin_enter')
    called(probe, 'DisplayRewardsPage', 'QuestAcceptGFxMovie')
    called(probe, 'MissionRewardsReceived')
    read(probe, status=4, open=True, text=page)
    assert not probe.ms_reward_shown()['ok']


def test_the_reward_is_taken_once_by_a_marked_press_after_the_page():
    probe = load_probe()
    page = {'reward.rewardsHeader.text': 'H'}
    fresh(probe)
    called(probe, 'MissionRewardsReceived')       # earlier in the recording: the old check passed on this row
    probe.ms_mark('reward_shown')
    probe.ms_mark('reward_press_Enter')
    read(probe, open=False)
    assert not probe.ms_reward_taken()['ok']
    # The call came, but the page is still up afterwards.
    fresh(probe)
    probe.ms_mark('reward_shown')
    probe.ms_mark('reward_press_Enter')
    called(probe, 'MissionRewardsReceived')
    read(probe, open=True, text=page)
    assert not probe.ms_reward_taken()['ok']
    # Taken and gone: the verdict names the press, also when it was a key test, which is a finding.
    for key in ('Enter', 'E'):
        fresh(probe)
        probe.ms_mark('reward_shown')
        probe.ms_mark('reward_press_W')
        probe.ms_mark(f'reward_press_{key}')
        called(probe, 'MissionRewardsReceived')
        read(probe, open=False)
        probe.ms_mark('reward_press_F')
        verdict = probe.ms_reward_taken()
        assert verdict['ok'] and verdict['press'] == key, verdict
    # Twice is not once.
    called(probe, 'MissionRewardsReceived')
    assert not probe.ms_reward_taken()['ok']


def test_the_respawn_needs_the_order_and_the_money():
    probe = load_probe()
    pawn = 'WillowPlayerPawn'

    def run(steps, cash_before=1000):
        fresh(probe)
        called(probe, 'ClientShowRespawnDialog', pawn, AmountCharged=70)   # before the mark: must not count
        probe.ms_mark('down_requested')
        probe.MS['cash_before'] = cash_before
        for step in steps:
            step()
        return probe.ms_respawn()
    down = lambda: probe.ms_mark('down')
    back = lambda: called(probe, 'ResurrectAtLocation', pawn)
    dialog = lambda: called(probe, 'ClientShowRespawnDialog', pawn, AmountCharged=70)
    alive = lambda: read(probe, health=100.0, injured=False, cash=930)
    assert run([down, back, dialog, alive])['ok']
    assert not run([dialog, alive])['ok']                 # the dialog function alone: the old check passed on this
    assert not run([back, dialog, alive])['ok']           # never went down
    assert not run([down, dialog, back, alive])['ok']     # the dialog before the respawn
    assert not run([down, back, dialog])['ok']            # no read after it
    assert not run([down, back, dialog, lambda: read(probe, health=100.0, injured=False, cash=1000)])['ok']   # no money taken
    assert not run([down, back, dialog, lambda: read(probe, health=0.0, injured=True, cash=930)])['ok']       # not alive


def test_the_capture_stops_before_the_respawn_unless_the_screen_closed():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    gate, respawn = text.index('not going on to the respawn'), text.index("$down = PyJson 'ms_down()'")
    assert gate < respawn
    for needle in ("'closed' -notin $stagesDone", '$closed.open -ne $false', '$closed.movie_playing -eq $true'):
        assert needle in text, needle
    # The stages use the verdicts, not a bare status or any row since the start.
    for needle in ("Set-Stage 'reward' ($log.reward_shown.ok -eq $true)", "Set-Stage 'reward_taken' ($log.reward_taken.ok -eq $true)", "Set-Stage 'respawn' ($log.respawn.ok -eq $true)"):
        assert needle in text, needle
    assert "for r in MS['rows']" not in text
    # ms_down refuses by itself too.
    probe = load_probe()
    fresh(probe)
    probe.ms_read = lambda label: {'open': True, 'cash': 5}
    assert probe.ms_down()['refused'] and probe.MS['down'] is None and probe.MS['rows'] == []
    probe.ms_read = lambda label: {'open': False, 'movie_playing': True, 'cash': 5}
    assert probe.ms_down()['refused'] and probe.MS['down'] is None
    probe.ms_read = lambda label: {'open': False, 'movie_playing': False, 'cash': 5}
    assert not probe.ms_down()['refused'] and probe.MS['down'] and probe.MS['cash_before'] == 5


def test_a_hook_error_is_counted_and_a_key_must_really_arrive():
    probe = load_probe()
    fresh(probe)
    probe.MS.update(hook_errors=0, hook_error_samples=[])

    def broken(obj, args, ret, func):
        raise AttributeError('bound function has no Name')
    probe._ms_guard(broken)(None, None, None, None)   # the bound-function mistake: counted, not lost
    assert probe.ms_errors()['hook_errors'] == 1 and 'bound function' in probe.ms_errors()['samples'][0]

    class FName:   # not a str, as the SDK's name values are not
        def __init__(self, text):
            self.text = text

        def __str__(self):
            return self.text
    assert probe._ms_p(FName('E')) == 'E'
    fresh(probe)
    probe.ms_mark('key_offer_E')
    assert probe.ms_key_seen('key_offer_E')['delivered'] is False      # pressed, never arrived
    called(probe, 'HandleInputKey', 'QuestAcceptGFxMovie', ukey='W')
    assert probe.ms_key_seen('key_offer_E')['delivered'] is False      # another key
    called(probe, 'HandleInputKey', 'QuestAcceptGFxMovie', ukey='E')
    assert probe.ms_key_seen('key_offer_E')['delivered'] is True
    # Before the mark does not count, and the capture's key names map to the game's.
    probe.ms_mark('key_reward_Esc')
    assert probe.ms_key_seen('key_reward_Esc')['delivered'] is False
    called(probe, 'HandleRewardInputKey', 'QuestAcceptGFxMovie', ukey='Escape')
    assert probe.ms_key_seen('key_reward_Esc')['delivered'] is True
    # A class that lacks a named setting costs nothing; every property is not walked.
    assert probe._ms_named(types.SimpleNamespace(bCaptureInput=True), ('bCaptureInput', 'bAbsent')) == {'bCaptureInput': True}
    # The respawn functions are hooked on the class that has them.
    names = {(cls, name) for cls, name, _ in probe.MS_FUNCS}
    assert ('WillowGame.WillowPawn', 'InjuredRespawn') in names and ('WillowGame.WillowPlayerPawn', 'InjuredRespawn') not in names
    # add_hook takes any string: a name that does not resolve is reported, not hooked.

    def nowhere(*a):
        raise ValueError('no such function')
    sys.modules['unrealsdk'].find_object = nowhere
    probe.unrealsdk = sys.modules['unrealsdk']
    started = probe.ms_start()
    assert started['hooks'] == len(probe.MS_WRITES) + 1 and len(started['missing']) == len(probe.MS_FUNCS), started
    probe.ms_stop()


def stubbed_test_keys(rows):
    """The capture's real Test-Keys against stubbed reads: one (open, status) per key; returns what it did."""
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    start = text.index('function Test-Keys(')
    body = text[start:text.index('\n}\n', start) + 3]
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'keys.ps1'
        fake = ', '.join(f"[pscustomobject]@{{ open = ${str(o).lower()}; status = {s} }}" for o, s in rows)
        script.write_text('$log = [ordered]@{}\n$script:rows = @(' + fake + ')\n$script:i = 0\n$script:keys = @()\n'
                          + 'function Test-Key($Stage, $Key) { $script:keys += $Key; $row = $script:rows[$script:i]; $script:i++; $row }\n'
                          + 'function Open-Screen($Label) { $script:keys += "reopen"; [pscustomobject]@{ open = $true; status = 0 } }\n' + body
                          + "\n$last = Test-Keys 'offer' @('W', 'Mouse', 'F') 0\n"
                          + "Write-Output ('keys=' + ($script:keys -join ',')); Write-Output ('log=' + (($log.Keys | Sort-Object) -join ',')); Write-Output ('count=' + @($last).Count + ' status=' + $last.status)\n",
                          encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, timeout=120)
        assert done.returncode == 0, done.stdout + done.stderr
        return done.stdout


def test_a_key_that_changes_the_status_ends_the_key_tests():
    # The first run: a key accepted, and reopening threw with nothing left to read.
    out = stubbed_test_keys([(False, 1), (True, 1), (True, 1)])
    assert 'keys=W\n' in out.replace('\r', '') and 'offer_status_changed_by_W' in out and 'reopen' not in out and 'count=1 status=1' in out, out
    # A key that only closes the screen is logged, and it is opened again for the next key.
    out = stubbed_test_keys([(True, 0), (False, 0), (True, 0)])
    assert 'keys=W,Mouse,reopen,F' in out and 'offer_closed_by_Mouse' in out and 'count=1 status=0' in out, out
    # All open: one row back, not a list.
    out = stubbed_test_keys([(True, 0), (True, 0), (True, 0)])
    assert 'keys=W,Mouse,F' in out and 'log=\n' in out.replace('\r', '') and 'count=1 status=0' in out, out


def test_e_comes_last_on_each_page():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    stages = text[text.index("$log.hooks = PyJson 'ms_start()'"):text.index('\n} catch {\n    $runError')]
    order = ["Test-Keys 'offer' @('W', 'Mouse', 'F', 'Space') 0", "$esc = Test-Key 'offer' 'Esc'", "$accept = Test-Key 'offer' 'E'", "$accept = Test-Key 'offer' 'Enter'",
             "Test-Keys 'turnin' @('W', 'Mouse') 3", "ms_mark('turnin_enter')", "$turned = Test-Key 'turnin' 'E'", "$turned = Test-Key 'turnin' 'Enter'",
             "foreach ($key in 'W', 'Mouse', 'F', 'Esc', 'E')", "ms_mark('reward_press_Enter')"]
    places = [stages.index(needle) for needle in order]
    assert places == sorted(places), list(zip(order, places))
    # Enter only when E did not do it; after the reward, Esc only: E could accept the next mission.
    assert 'if ($accept.status -eq 0) {' in stages and 'if ($turned.status -eq 3) {' in stages and '$log.reward_after_keys.count -eq 0' in stages
    assert "foreach ($key in 'E', 'Esc')" not in stages and "$after = Test-Key 'after' 'Esc'" in stages
    assert 'Send-Key Enter' not in stages[stages.index("$esc = Test-Key"):stages.index('# 4. The respawn')], 'every Enter goes through Test-Key, so it is marked and its delivery read'


def test_the_capture_sees_what_each_stage_claims():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    for needle in ("Set-Stage 'hooks' (@($log.hooks.missing).Count -eq 0)", "Set-Stage 'offer_keys' ($log.delivered_offer_E.delivered -eq $true)", "Set-Stage 'reward_keys' ($log.delivered_reward_E.delivered -eq $true)",
                   "Set-Stage 'hook_errors' ($log.hook_errors.hook_errors -eq 0)", 'if ($log.before.status -ne 0) { throw', "if ($map -notmatch 'Sanctuary')",
                   '$null = switch ($Key)', '$esc = Test-Key'):
        assert needle in text, needle
    # Esc never goes out by itself in a stage: Test-Keys reopens a screen a key closed, and the reward's loop checks it is open.
    stages = text[text.index("$log.hooks = PyJson 'ms_start()'"):text.index('\n} catch {\n    $runError')]
    assert 'Send-Key Esc' not in stages
    requested = text[text.index('$stagesRequested = @('):].split('\n')[0]
    for stage in ('hooks', 'offer_keys', 'reward_keys', 'hook_errors'):
        assert f"'{stage}'" in requested, stage


def test_probe_helpers():
    probe = load_probe()
    first, second = probe.MS_TEXT[0], probe.MS_TEXT[1]

    def read(path):
        if path == first:
            return 'invented title'
        if path == second:
            raise RuntimeError('no such field')
        return ''
    picked = probe.ms_pick_text(read)
    assert picked[first] == 'invented title' and picked[second].startswith('error') and len(picked) == 2, picked
    assert probe.ms_turnin_ready(3) and not probe.ms_turnin_ready(1) and not probe.ms_turnin_ready(4)
    # Every hooked function is named once, so a row always says which call it was.
    names = [(cls, name) for cls, name, _ in probe.MS_FUNCS]
    assert len(names) == len(set(names))


if __name__ == '__main__':
    test_probe_helpers()
    test_the_movie_is_a_path_cleared_on_both_close_paths()
    test_a_mission_status_alone_is_not_a_reward_page()
    test_the_reward_is_taken_once_by_a_marked_press_after_the_page()
    test_the_respawn_needs_the_order_and_the_money()
    test_the_capture_stops_before_the_respawn_unless_the_screen_closed()
    test_a_hook_error_is_counted_and_a_key_must_really_arrive()
    test_the_capture_sees_what_each_stage_claims()
    test_a_key_that_changes_the_status_ends_the_key_tests()
    test_e_comes_last_on_each_page()
    test_the_capture_parses_and_every_stage_is_marked()
    test_the_capture_is_wired_to_the_safety_helpers()
    test_the_finalizer_is_the_hardened_one_with_only_the_hook_stop_changed()
    test_exit_0_only_for_a_completed_run_with_nothing_to_recover()
    print('ok')
