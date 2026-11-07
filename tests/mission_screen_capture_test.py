"""Synthetic tests for the mission screen capture (no game, no game files): the end of its run is stubbed the way
tests/realgame_snapshot_test.py does for the Phaselock capture, and the probe's game-free helpers run on invented values. Needs Windows PowerShell.

Run: python tests/mission_screen_capture_test.py
"""
import importlib.util
import os
import re
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
    start = mine.index('    # Mod setting: the root restore above')
    mine = mine[:start] + mine[mine.index('    $recovery = Get-RecoveryReason'):]
    mod_line = '    if ($modProblem) { $recovery = (@($recovery, "mod setting $modProblem") | Where-Object { $_ }) -join \'; \' }\n'
    assert mod_line in mine
    mine = mine.replace(mod_line, '')
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
    for stage in ('start', 'offer', 'accept', 'ready', 'turnin', 'reward', 'reward_taken', 'closed', 'respawn'):
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


def test_the_stock_read_presses_e_and_wasd():
    # Better UI Controls is off in this run, so those keys are the game's. E comes last among the tests.
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    stages = text[text.index("$log.hooks = PyJson 'ms_start()'"):text.index('\n} catch {\n    $runError')]
    keys = re.findall(r"Test-Keys '\w+' @\(([^)]*)\)|Test-Key '\w+' '(\w+)'|foreach \(\$key in ('[^)]*)\) \{\n\s+if \(\$last\.open\)", stages)
    pressed = {name for group in keys for part in group for name in re.findall(r"'(\w+)'", part) + ([part] if part.isalpha() else [])}
    assert pressed == {'Mouse', 'W', 'A', 'S', 'D', 'F', 'E', 'Esc', 'Enter'}, pressed
    assert 'Send-Key Enter' not in stages and 'Send-Key Esc' not in stages
    order = ["Test-Keys 'offer' @('Mouse', 'W', 'A', 'S', 'D', 'F', 'E') 0", "$esc = Test-Key 'offer' 'Esc'", "$accept = Test-Key 'offer' 'Enter'", "ms_mark('turnin_enter')",
             "Test-Keys 'turnin' @('Mouse', 'W', 'S', 'E') 3", "$turned = Test-Key 'turnin' 'Enter'", "foreach ($key in 'Mouse', 'W', 'S', 'E', 'Esc')", "ms_mark('reward_press_Enter')", "$after = Test-Key 'after' 'Esc'"]
    places = [stages.index(needle) for needle in order]
    assert places == sorted(places), list(zip(order, places))
    # The mod's own Enter after E would fail the stage, so a mod still on cannot read as stock.
    assert "@($log.delivered_offer_E.seen) -notcontains 'Enter'" in stages


MOD_BLOCK_RE = re.compile(r'# Mod setting start\..*?# Mod setting end\.\n', re.S)


def mod_functions():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    return MOD_BLOCK_RE.search(text).group(0)


def run_ps(body):
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'mod.ps1'
        script.write_text("$ErrorActionPreference = 'Stop'\n$tmp = '" + tmp + "'\n" + mod_functions() + body, encoding='utf-8-sig')
        return subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, timeout=120)


def test_the_mod_setting_changes_only_that_value():
    done = run_ps(r"""
$prior = [Text.Encoding]::ASCII.GetBytes("{`r`n    `"enabled`": true`r`n}")
$off = Get-ModOffBytes $prior
Write-Output ([Text.Encoding]::ASCII.GetString($off).Replace("`r", 'CR').Replace("`n", 'LF'))
foreach ($bad in '{"enabled": false}', '{}', 'not json', '{"enabled": true, "x": {"enabled": true}}') {
    try { $null = Get-ModOffBytes ([Text.Encoding]::ASCII.GetBytes($bad)); Write-Output "ACCEPTED $bad" } catch { Write-Output 'refused' }
}
try { $null = Get-ModOffBytes ([byte[]](0x7B, 0xFF, 0x7D)); Write-Output 'ACCEPTED bad utf8' } catch { Write-Output 'refused' }
""")
    assert done.returncode == 0, done.stdout + done.stderr
    lines = done.stdout.splitlines()
    assert lines[0] == '{CRLF    "enabled": falseCRLF}', lines
    assert lines[1:] == ['refused'] * 5, lines


def test_the_mod_setting_comes_back_byte_identical():
    done = run_ps(r"""
$file = Join-Path $tmp 'mod.json'
$prior = [byte[]](1, 2, 3, 200)
[IO.File]::WriteAllBytes($file, $prior)
Write-Output (Restore-ModSetting $file $prior)
[IO.File]::WriteAllBytes($file, [byte[]](9))
Write-Output (Restore-ModSetting $file $prior)
Write-Output (Test-SameBytes ([IO.File]::ReadAllBytes($file)) $prior)
Remove-Item $file
Write-Output (Restore-ModSetting $file $prior)
""")
    assert done.returncode == 0, done.stdout + done.stderr
    assert done.stdout.split() == ['identical', 'restored,', 'identical', 'True', 'restored,', 'identical'], done.stdout


def test_the_finalizer_restores_the_setting_or_holds_for_recovery():
    never_exits = '[pscustomobject]@{ HasExited = $false; Id = 1 }'
    finish = "$stagesDone.Add('hit')"
    base_setup = mod_functions() + "$modFile = '__TMP__\\mod.json'; $modWritten = $true; $modPrior = [byte[]](1, 2, 3)\n[IO.File]::WriteAllBytes($modFile, [byte[]](9))\n"
    def run(setup, proc='$null', extra=''):
        text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
        tail = text[text.index('\n} catch {\n    $runError') + 1:]
        with tempfile.TemporaryDirectory() as tmp:
            script = Path(tmp) / 'driver.ps1'
            script.write_text(base.TAIL.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__TMP__', tmp)
                              .replace('__OUT__', tmp).replace('__SETUP__', setup.replace('__TMP__', tmp) + extra).replace('__LAUNCHED__', '$true').replace('__OWNED__', '$true').replace('__ATTEMPTED__', '$true')
                              .replace('__STUBS__', 'function Remove-Driver { }').replace('__BODY__', finish).replace('__CAPTURE_TAIL__', tail).replace('__PROC__', proc).replace('__CLEAN__', '$true'), encoding='utf-8-sig')
            done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp, LOCALAPPDATA=tmp), timeout=180)
            done.file_bytes = (Path(tmp) / 'mod.json').read_bytes()
            done.hub_lock_kept = (Path(tmp) / 'hub.lock').exists()
            return done
    done = run(base_setup)
    assert done.returncode == 0 and 'status: completed' in done.stdout and 'cleanup mod_setting: restored, identical' in done.stdout, done.stdout + done.stderr
    assert done.file_bytes == bytes([1, 2, 3]) and done.hub_lock_kept is False, done.stdout
    # It cannot restore: not completed, the locks stay, and the recovery text prints.
    done = run(base_setup, extra="function Restore-ModSetting { throw 'disk full' }\n")
    assert done.returncode == 1 and 'recovery needed' in done.stdout and 'mod setting not restored' in done.stdout and 'MANUAL RECOVERY' in done.stdout and done.hub_lock_kept is True, done.stdout + done.stderr
    # A game that may still run: nothing is written under it.
    done = run(base_setup, never_exits)
    assert done.returncode == 1 and 'cleanup mod_setting: NOT RESTORED' in done.stdout and done.file_bytes == bytes([9]) and done.hub_lock_kept is True, done.stdout + done.stderr


def test_a_refusal_before_the_launch_does_not_restore_the_roots():
    # The setting was changed, or its compare refused, and no game launched: the roots must not be restored over newer files, and the one file still comes back.
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    tail = text[text.index('\n} catch {\n    $runError') + 1:]
    with tempfile.TemporaryDirectory() as tmp:
        setup = mod_functions() + f"$modFile = '{tmp}\\mod.json'; $modWritten = $true; $modPrior = [byte[]](1, 2, 3)\n[IO.File]::WriteAllBytes($modFile, [byte[]](9))\n"
        script = Path(tmp) / 'driver.ps1'
        # base.TAIL's Restore-VerifiedRoots is replaced by one that leaves a mark, which must not appear.
        stub = f"function Remove-Driver {{ }}\nfunction Restore-VerifiedRoots {{ Set-Content '{tmp}\\ROOTS_RESTORED' x; [pscustomobject]@{{ rows = @(); summary = 'x'; clean = $true }} }}"
        script.write_text(base.TAIL.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__TMP__', tmp).replace('__OUT__', tmp).replace('__SETUP__', setup)
                          .replace('__LAUNCHED__', '$false').replace('__OWNED__', '$true').replace('__ATTEMPTED__', '$true').replace('__STUBS__', stub)
                          .replace('__BODY__', "throw 'the mod setting differs from its verified backup; not changing it'").replace('__CAPTURE_TAIL__', tail).replace('__PROC__', '$null').replace('__CLEAN__', '$true'), encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp, LOCALAPPDATA=tmp), timeout=180)
        assert done.returncode == 1 and 'not touched: this run launched no game' in done.stdout, done.stdout + done.stderr
        assert not (Path(tmp) / 'ROOTS_RESTORED').exists(), done.stdout
        assert (Path(tmp) / 'mod.json').read_bytes() == bytes([1, 2, 3]) and 'cleanup mod_setting: restored, identical' in done.stdout, done.stdout


def test_a_killed_run_is_recovered_by_following_the_refusal():
    # The state a kill leaves: setting off, driver folder there, a save changed, both locks. Only the lines the refusal prints are then run, in a fresh process.
    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)
        env = dict(os.environ, OPENWILLOW_BL2=str(tmp_path / 'game'), LOCALAPPDATA=str(tmp_path / 'appdata'))
        env.pop('OPENWILLOW_UE_LOCK', None)
        (tmp_path / 'game/sdk_mods/settings').mkdir(parents=True)
        (tmp_path / 'appdata').mkdir()
        live_saves = tmp_path / 'live_saves'
        live_saves.mkdir()
        (live_saves / 'one.sav').write_bytes(b'before')
        prior = b'{\r\n    "enabled": true\r\n}'
        setting = tmp_path / 'game/sdk_mods/settings/BetterUIControls.json'
        setting.write_bytes(prior)
        kill = tmp_path / 'kill.ps1'
        kill.write_text(f""". '{ROOT / 'tools/real_game/realgame.ps1'}'
{mod_functions()}
$bak = '{tmp}\\bak'; New-Item -ItemType Directory $bak | Out-Null
$roots = [ordered]@{{ SaveData = '{live_saves}'; sdk_settings = '{setting.parent}' }}
$snapshots = @{{}}; foreach ($d in $roots.Keys) {{ $snapshots[$d] = New-VerifiedSnapshot $roots[$d] (Join-Path $bak $d) }}
Save-VerifiedBackupRecord $bak $roots $snapshots
Save-ModPending '{setting}' ([IO.File]::ReadAllBytes('{setting}')) $bak 'test' '{tmp}\\hub.lock' '{ROOT / 'tools/real_game/realgame.ps1'}'
[IO.File]::WriteAllBytes('{setting}', (Get-ModOffBytes ([IO.File]::ReadAllBytes('{setting}'))))
New-Item -ItemType Directory -Force $script:DriverDir | Out-Null
Set-Content '{live_saves}\\one.sav' 'after'
Set-Content '{tmp}\\hub.lock' 'test now'
Set-Content -LiteralPath $script:Lock '{{"pid": 999999, "process_start_utc_filetime": 1}}'
try {{ Assert-NoModPending; Write-Output 'NOT REFUSED' }} catch {{ [IO.File]::WriteAllText('{tmp}\\refusal.txt', "$_") }}
""", encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(kill)], capture_output=True, text=True, env=env, timeout=120)
        assert done.returncode == 0 and 'NOT REFUSED' not in done.stdout, done.stdout + done.stderr
        assert setting.read_bytes() != prior, 'the refusal restored by itself'
        lock = tmp_path / 'appdata/OpenWillow/ue_run.lock'
        record = tmp_path / 'appdata/OpenWillow/mission_screen_mod_pending.json'
        driver = tmp_path / 'game/sdk_mods/openwillow_realgame'
        assert lock.exists() and record.exists() and driver.exists() and (tmp_path / 'hub.lock').exists()
        refusal = (tmp_path / 'refusal.txt').read_text(encoding='utf-8')
        commands = [line[4:] for line in refusal.splitlines() if line.startswith('    ')]
        assert len(commands) == 7, refusal
        # In the right order: the restore rewrites the settings folder, so the exact bytes come after it, and the locks and the record only after both.
        order = ['OPENWILLOW_BL2', 'Remove-Item -Recurse', 'Restore-VerifiedBackup', 'WriteAllBytes', 'hub.lock', 'Test-OwUeLockStale', 'mission_screen_mod_pending']
        assert all(needle in command for needle, command in zip(order, commands)), commands
        follow = tmp_path / 'follow.ps1'
        follow.write_text('\n'.join(commands) + '\n', encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(follow)], capture_output=True, text=True, env=env, timeout=120)
        assert done.returncode == 0, done.stdout + done.stderr
        assert 'changed, restored' in done.stdout, done.stdout
        assert setting.read_bytes() == prior and (live_saves / 'one.sav').read_bytes() == b'before', done.stdout
        assert not driver.exists() and not (tmp_path / 'hub.lock').exists() and not lock.exists() and not record.exists(), done.stdout
        # The next run goes ahead.
        rerun = tmp_path / 'rerun.ps1'
        rerun.write_text(f""". '{ROOT / 'tools/real_game/realgame.ps1'}'
{mod_functions()}
Assert-NoModPending
Write-Output 'no record, no refusal'
""", encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(rerun)], capture_output=True, text=True, env=env, timeout=120)
        assert 'no record, no refusal' in done.stdout, done.stdout + done.stderr
        # A hub lock naming another lane stays.
        (tmp_path / 'hub.lock').write_text('other now')
        keep = tmp_path / 'keep.ps1'
        hub_line = next(c for c in commands if 'hub.lock' in c)
        keep.write_text(hub_line + '\n', encoding='utf-8-sig')
        subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(keep)], capture_output=True, text=True, env=env, timeout=60)
        assert (tmp_path / 'hub.lock').exists(), 'a hub lock naming another lane was removed'


def tail_driver(tmp, setup, body, proc='$null', launched='$false', attempted='$false', owned='$false', stubs='function Remove-Driver { }'):
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    tail = text[text.index('\n} catch {\n    $runError') + 1:]
    script = Path(tmp) / 'driver.ps1'
    script.write_text(base.TAIL.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__TMP__', tmp).replace('__OUT__', tmp).replace('__SETUP__', setup)
                      .replace('__LAUNCHED__', launched).replace('__OWNED__', owned).replace('__ATTEMPTED__', attempted).replace('__STUBS__', stubs)
                      .replace('__BODY__', body).replace('__CAPTURE_TAIL__', tail).replace('__PROC__', proc).replace('__CLEAN__', '$true'), encoding='utf-8-sig')
    return subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp, LOCALAPPDATA=tmp), timeout=180)


def test_the_record_is_the_refusal_even_with_the_driver_folder_there():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    preflight = text[text.index('    # Preflight before any lock'):text.index('    # What was loaded:')]
    with tempfile.TemporaryDirectory() as tmp:
        (Path(tmp) / 'sdk_mods/openwillow_realgame').mkdir(parents=True)
        setup = mod_functions() + f"$env:LOCALAPPDATA = '{tmp}'; Save-ModPending '{tmp}\\mod.json' ([byte[]](1, 2, 3)) 'bak' 'test' 'hub' 'realgame'\n"
        done = tail_driver(tmp, setup, preflight)
        assert done.returncode == 1 and 'Recovery, in this order' in done.stdout and 'already exists' not in done.stdout, done.stdout + done.stderr


def launch_lines():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    return text[text.index('    $launchAttempted = $true\n'):text.index('    $ok = $false\n')]


def test_a_launch_that_throws_is_judged_by_whether_a_game_exists():
    throwing = "function Start-Game { throw 'launch failed' }\n"
    marks = "function Remove-Driver { }\nfunction Restore-VerifiedRoots { Set-Content '__TMP__\\ROOTS' x; [pscustomobject]@{ rows = @(); summary = 'identical'; clean = $true } }\nfunction Stop-OwnGame { Set-Content '__TMP__\\STOPPED' $args[2]; 'refused' }\n"
    gone = "[pscustomobject]@{ HasExited = $true; Id = 1; StartTime = (Get-Date) } | Add-Member ScriptMethod Refresh { } -PassThru"
    alive = "[pscustomobject]@{ HasExited = $false; Id = 1; StartTime = (Get-Date) } | Add-Member ScriptMethod Refresh { } -PassThru"
    def run(find, expect_roots, expect_stop):
        with tempfile.TemporaryDirectory() as tmp:
            setup = mod_functions() + f"$modFile = '{tmp}\\mod.json'; $modWritten = $true; $modPrior = [byte[]](1, 2, 3)\n[IO.File]::WriteAllBytes($modFile, [byte[]](9))\n"
            done = tail_driver(tmp, setup, launch_lines(), attempted='$true', stubs=(marks + throwing + find).replace('__TMP__', tmp))
            assert done.returncode == 1 and 'launch failed' in done.stdout, done.stdout + done.stderr
            assert (Path(tmp) / 'ROOTS').exists() is expect_roots, done.stdout
            assert (Path(tmp) / 'STOPPED').exists() is expect_stop, done.stdout
            return done, (Path(tmp) / 'mod.json').read_bytes()
    # None found: nothing launched, the roots stay, the one file comes back.
    done, data = run("function Find-GameByMarker { }\n", False, False)
    assert data == bytes([1, 2, 3]) and 'not touched: this run launched no game' in done.stdout, done.stdout
    # Found and already gone: treated as launched.
    done, data = run("function Find-GameByMarker { " + gone + " }\n", True, False)
    assert data == bytes([1, 2, 3]), done.stdout
    # Found and alive: the finalizer tries to stop it, and holds when that is refused.
    done, data = run("function Find-GameByMarker { " + alive + " }\n", False, True)
    assert data == bytes([9]) and 'MANUAL RECOVERY' in done.stdout, done.stdout
    # The search failed: the launch's own error still reports, and the flag stays set.
    done, data = run("function Find-GameByMarker { throw 'no cim' }\n", True, False)
    assert data == bytes([1, 2, 3]), done.stdout
    # The marker must match whole: this run's pid must not match a longer one.
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    finder = text[text.index('function Find-GameByMarker('):text.index('function Py(')]
    done = run_ps(finder + r"""
function Get-CimInstance { [pscustomobject]@{ ProcessId = 7; CommandLine = 'game.exe -windowed -owrun=a-123 -x' } }
function Get-Process { param($Id) "pid $Id" }
Write-Output ('short=[' + (Find-GameByMarker '-owrun=a-12') + ']')
Write-Output ('whole=[' + (Find-GameByMarker '-owrun=a-123') + ']')
""")
    assert 'short=[]' in done.stdout and 'whole=[pid 7]' in done.stdout, done.stdout + done.stderr


def test_a_record_that_will_not_go_is_a_cleanup_problem():
    with tempfile.TemporaryDirectory() as tmp:
        setup = mod_functions() + f"$env:LOCALAPPDATA = '{tmp}'; $modFile = '{tmp}\\mod.json'; $modWritten = $true; $modPrior = [byte[]](1, 2, 3)\n[IO.File]::WriteAllBytes($modFile, [byte[]](9))\nSave-ModPending $modFile $modPrior 'bak' 'test' 'hub' 'realgame'\n(Get-Item (Get-ModPendingFile)).IsReadOnly = $true\n"
        done = tail_driver(tmp, setup, "$stagesDone.Add('hit')", launched='$true', attempted='$true', owned='$true')
        assert done.returncode == 1 and 'status: completed' not in done.stdout, done.stdout + done.stderr
        assert 'could not be removed' in done.stdout and 'MANUAL RECOVERY' in done.stdout and (Path(tmp) / 'hub.lock').exists(), done.stdout
        assert (Path(tmp) / 'mod.json').read_bytes() == bytes([1, 2, 3]), done.stdout


def open_screen_run(body):
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    start = text.index('function Open-Screen(')
    function = text[start:text.index('# Which of these keys never reached')]
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'open.ps1'
        script.write_text(r"""$ErrorActionPreference = 'Stop'
$log = @{}; $script:calls = New-Object Collections.Generic.List[string]; $script:open = $false; $script:opensAt = -1; $script:polls = 0
function Start-Sleep { }
function Assert-GameForeground { }
function Py($Code) { $script:calls.Add('mark') }
function Send-Key($Key) { $script:calls.Add("key $Key") }
function Read-After($Label, $Wait) { $script:calls.Add("read $Label"); [pscustomobject]@{ open = $script:open; status = 0 } }
function PyJson($Expr) {
    if ($Expr -like 'fm_stand*') { $script:calls.Add('stand'); return [pscustomobject]@{} }
    if ($Expr -like 'ms_read*') { $script:calls.Add('baseline'); return [pscustomobject]@{ open = $script:open } }
    $script:polls++
    if ($script:opensAt -ge 0 -and $script:polls -ge $script:opensAt) { $script:open = $true; return [pscustomobject]@{ opened = $true } }
    [pscustomobject]@{ opened = $false }
}
""" + function + body, encoding='utf-8-sig')
        return subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, timeout=60)


def test_a_slow_open_is_a_read_and_moves_nothing():
    # Opens on the third poll: one press, one stand, and a read after it.
    done = open_screen_run(r"""
$script:opensAt = 3
$row = Open-Screen 'x'
Write-Output ('open=' + $row.open); Write-Output ('calls=' + ($script:calls -join ','))
Write-Output ('late=' + $log['x_opened_late'])
""")
    assert done.returncode == 0, done.stdout + done.stderr
    assert 'open=True' in done.stdout and 'late=1' in done.stdout, done.stdout
    assert 'calls=baseline,stand,mark,key E,read x,read x_late_1' in done.stdout, done.stdout
    # Opens after the wait, seen at the next baseline: still this press, and no second stand.
    done = open_screen_run(r"""
$script:opensAt = 999
function Start-Sleep { if ($script:polls -eq 10) { $script:open = $true; $script:opensAt = 11 } }
$row = Open-Screen 'x'
Write-Output ('open=' + $row.open); Write-Output ('calls=' + ($script:calls -join ','))
""")
    assert done.returncode == 0, done.stdout + done.stderr
    assert 'open=True' in done.stdout and 'calls=baseline,stand,mark,key E,read x,baseline,read x_late_1' in done.stdout, done.stdout
    # Open with no press of ours: refused before anything moves.
    done = open_screen_run(r"""
$script:open = $true
try { Open-Screen 'x' | Out-Null; Write-Output 'NOT REFUSED' } catch { Write-Output "refused: $_" }
Write-Output ('calls=' + ($script:calls -join ','))
""")
    assert 'refused: the screen is already open' in done.stdout and 'calls=baseline' in done.stdout and 'stand' not in done.stdout, done.stdout + done.stderr
    # Never opens: four presses, reported closed.
    done = open_screen_run(r"""
$row = Open-Screen 'x'
Write-Output ('open=' + $row.open); Write-Output ('presses=' + @($script:calls | Where-Object { $_ -eq 'key E' }).Count)
""")
    assert 'open=False' in done.stdout and 'presses=4' in done.stdout, done.stdout + done.stderr


def test_the_finalizer_removes_the_record_only_when_the_bytes_are_back():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    tail = text[text.index('\n} catch {\n    $runError') + 1:]
    for extra, proc, kept in (('', '$null', False), ("function Restore-ModSetting { throw 'disk full' }\n", '$null', True), ('', '[pscustomobject]@{ HasExited = $false; Id = 1 }', True)):
        with tempfile.TemporaryDirectory() as tmp:
            setup = mod_functions() + f"$env:LOCALAPPDATA = '{tmp}'; $modFile = '{tmp}\\mod.json'; $modWritten = $true; $modPrior = [byte[]](1, 2, 3)\n[IO.File]::WriteAllBytes($modFile, [byte[]](9))\nSave-ModPending $modFile $modPrior 'bak' 'test'\n" + extra
            script = Path(tmp) / 'driver.ps1'
            script.write_text(base.TAIL.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__TMP__', tmp).replace('__OUT__', tmp).replace('__SETUP__', setup)
                              .replace('__LAUNCHED__', '$true').replace('__OWNED__', '$true').replace('__ATTEMPTED__', '$true').replace('__STUBS__', 'function Remove-Driver { }')
                              .replace('__BODY__', "$stagesDone.Add('hit')").replace('__CAPTURE_TAIL__', tail).replace('__PROC__', proc).replace('__CLEAN__', '$true'), encoding='utf-8-sig')
            subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp), timeout=180)
            assert (Path(tmp) / 'OpenWillow/mission_screen_mod_pending.json').exists() is kept, (extra, proc)


def test_the_mod_proof_runs_against_the_real_shape():
    # As installed: the package's attribute named mod_list is its submodule, not a list, and get_ordered_mod_list() is the public way. A form that iterates mod_list would fail here.
    probe = load_probe()
    mod = lambda name, enabled, settings: types.SimpleNamespace(name=name, is_enabled=enabled, settings_file=None if settings is None else Path(settings))
    fake = types.ModuleType('mods_base')
    fake.mod_list = types.ModuleType('mods_base.mod_list')
    fake.get_ordered_mod_list = lambda: [mod('Base', True, 'python-sdk.json'), mod('NoFile', True, None), mod('BUIC', state, 'x/BetterUIControls.json')]
    sys.modules['mods_base'] = fake
    for state, ok in ((False, True), (True, False)):
        result = probe.ms_mod_state('BetterUIControls.json')
        assert result['ok'] is ok and result['names'] == ['BUIC'], result
    # Not there at all, or two with that file: not proof.
    fake.get_ordered_mod_list = lambda: [mod('Base', True, 'python-sdk.json')]
    assert probe.ms_mod_state('BetterUIControls.json')['ok'] is False
    fake.get_ordered_mod_list = lambda: [mod('A', False, 'BetterUIControls.json'), mod('B', False, 'BetterUIControls.json')]
    assert probe.ms_mod_state('BetterUIControls.json')['ok'] is False
    text = CAPTURE.read_text(encoding='utf-8')
    assert "ms_mod_state('BetterUIControls.json')" in text and '$log.mod_off.ok -ne $true' in text and ".mod_list" not in text


def test_an_open_needs_a_closed_baseline_and_its_own_mark():
    probe = load_probe()
    fresh(probe)
    hook(probe, 'Start', 'post', True)            # opened before the mark
    probe.ms_mark('open_x_1')
    assert probe.ms_opened('open_x_1')['opened'] is False
    hook(probe, 'Start', 'post', False)           # refused after it
    assert probe.ms_opened('open_x_1')['opened'] is False
    hook(probe, 'Start', 'post', True)
    assert probe.ms_opened('open_x_1')['opened'] is True
    assert probe.ms_opened('no_such_mark')['opened'] is False
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    opening = text[text.index('function Open-Screen('):text.index('function Get-KeysNotDelivered')]
    places = [opening.index(n) for n in ("ms_read('${Label}_baseline_$try')", 'if ($base.open -ne $false) {', "fm_stand_in_front", '$mark = "open_', "ms_mark('$mark')", 'Send-Key E')]
    assert places == sorted(places), places


def test_every_keyboard_key_needs_its_delivery():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    for needle in ("Get-KeysNotDelivered 'offer' @('W', 'A', 'S', 'D', 'F', 'E')", "Get-KeysNotDelivered 'turnin' @('W', 'S', 'E')", "Get-KeysNotDelivered 'reward' @('W', 'S', 'E', 'Esc')", "Set-Stage 'turnin_keys'", "Set-Stage 'reward_keys' ($missing.Count -eq 0)"):
        assert needle in text, needle
    start = text.index('function Get-KeysNotDelivered')
    body = text[start:text.index('\n}\n', start) + 3]
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'keys.ps1'
        script.write_text(body + """
$log = @{ delivered_x_W = [pscustomobject]@{ delivered = $true }; delivered_x_S = [pscustomobject]@{ delivered = $false } }
Write-Output ('missing=' + ((Get-KeysNotDelivered 'x' @('W', 'S', 'E')) -join ','))
Write-Output ('count=' + @(Get-KeysNotDelivered 'x' @('W')).Count)
""", encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, timeout=60)
    assert 'missing=S,E' in done.stdout and 'count=0' in done.stdout, done.stdout + done.stderr


def test_the_capture_refuses_and_proves_before_any_key():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    # Refused before any lock or backup; written after the launch flag, so the restore covers it, and last before the launch; proved in the game before any key.
    order = ['Assert-NoModPending', 'bl-sdk is not installed', 'already exists; this run', '$null = Get-ModOffBytes', '$hubDeadline = (Get-Date)', 'Backup-Saves', "$driverAttempted = $true", "sdk_settings\\BetterUIControls.json", '$log.mod_setting_prior_base64', 'Save-ModPending $modFile $modPrior $bak $Lane $HubLock', '$modWritten = $true',
             '[IO.File]::WriteAllBytes($modFile, $modOff)', '$launchAttempted = $true', '$proc = Start-Game $gameArgs', 'Assert-SavesBlocked (Py', '$log.mod_off = PyJson', 'Send-Key Enter; Start-Sleep 45']
    body = text.index('try {\n    # Preflight')
    places = [text.index(needle, body) for needle in order]
    assert places == sorted(places), list(zip(order, places))
    # Only this file: no other settings file is written.
    assert len(re.findall(r'\[IO\.File\]::WriteAllBytes\(\$(?:File|modFile),', text)) == 2 and text.count('[IO.File]::WriteAllBytes(') == 3   # the third is in the recovery text, for a person to run


def test_the_session_records_the_mods():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    record, lock = text.index('$log.sdk_mods = @('), text.index('$hubDeadline = (Get-Date)')
    assert record < lock < text.index('Install-Driver -BlockSavesAtStart -Exclusive'), 'recorded before the run changes anything'
    assert '$log.sdk_mods_settings[$file.BaseName]' in text


def test_the_capture_sees_what_each_stage_claims():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    for needle in ("Set-Stage 'hooks' (@($log.hooks.missing).Count -eq 0)", "Set-Stage 'offer_keys' ($missing.Count -eq 0", "Set-Stage 'turnin_keys' ($missing.Count -eq 0", "Set-Stage 'reward_keys' ($missing.Count -eq 0",
                   "Set-Stage 'hook_errors' ($log.hook_errors.hook_errors -eq 0)", 'if ($log.before.status -ne 0) { throw', "if ($map -notmatch 'Sanctuary')",
                   '$null = switch ($Key)', "$after = Test-Key 'after' 'Esc'"):
        assert needle in text, needle
    # Esc never goes out by itself in a stage: Test-Keys reopens a screen a key closed, and the reward's loop checks it is open.
    stages = text[text.index("$log.hooks = PyJson 'ms_start()'"):text.index('\n} catch {\n    $runError')]
    assert 'Send-Key Esc' not in stages
    requested = text[text.index('$stagesRequested = @('):].split('\n')[0]
    for stage in ('hooks', 'mod_off', 'offer_keys', 'offer_esc', 'turnin_keys', 'reward_keys', 'hook_errors'):
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
    test_the_stock_read_presses_e_and_wasd()
    test_the_mod_setting_changes_only_that_value()
    test_the_mod_setting_comes_back_byte_identical()
    test_the_finalizer_restores_the_setting_or_holds_for_recovery()
    test_the_capture_refuses_and_proves_before_any_key()
    test_a_refusal_before_the_launch_does_not_restore_the_roots()
    test_a_killed_run_is_recovered_by_following_the_refusal()
    test_the_record_is_the_refusal_even_with_the_driver_folder_there()
    test_a_launch_that_throws_is_judged_by_whether_a_game_exists()
    test_a_record_that_will_not_go_is_a_cleanup_problem()
    test_a_slow_open_is_a_read_and_moves_nothing()
    test_the_finalizer_removes_the_record_only_when_the_bytes_are_back()
    test_the_mod_proof_runs_against_the_real_shape()
    test_an_open_needs_a_closed_baseline_and_its_own_mark()
    test_every_keyboard_key_needs_its_delivery()
    test_the_session_records_the_mods()
    test_the_capture_parses_and_every_stage_is_marked()
    test_the_capture_is_wired_to_the_safety_helpers()
    test_the_finalizer_is_the_hardened_one_with_only_the_hook_stop_changed()
    test_exit_0_only_for_a_completed_run_with_nothing_to_recover()
    print('ok')
