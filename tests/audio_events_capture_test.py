"""Synthetic tests for the sound-event capture (no game: unrealsdk is stubbed; the objects and names are invented). The capture's
cleanup is run by the same harness as tests/realgame_snapshot_test.py. Needs Windows PowerShell.

Run: python tests/audio_events_capture_test.py
"""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tests'))
import realgame_snapshot_test as snapshot  # noqa: E402

SCRIPT = ROOT / 'tools/real_game/scripts/audio_events.py'
CAPTURE = ROOT / 'tools/real_game/audio_events_capture.ps1'


class Obj:
    def __init__(self, **fields):
        self.__dict__.update(fields)

    def _path_name(self):
        return 'Pkg.' + self.__dict__.get('Name', 'Obj')


def event(name):
    return Obj(Name=name, Class=Obj(Name='AkEvent'))


def args(**values):
    props = [Obj(Name=key) for key in values]
    return Obj(_type=Obj(_properties=lambda: props), **values)


def load(tmp, functions=None):
    sdk = types.ModuleType('unrealsdk')
    sdk.make_struct = lambda name, **f: Obj(**f)

    def find_object(_cls, path):
        if functions is not None and path not in functions:
            raise ValueError(path)
        return path
    sdk.find_object = find_object
    sdk.find_all = lambda *a, **k: []
    hooks = types.ModuleType('unrealsdk.hooks')
    hooks.Type = Obj(PRE=1, POST=2)
    sdk.calls = []
    hooks.add_hook = lambda *a: sdk.calls.append(('add',) + a)
    hooks.remove_hook = lambda *a: sdk.calls.append(('remove',) + a)
    sdk.hooks = hooks
    sys.modules['unrealsdk'], sys.modules['unrealsdk.hooks'] = sdk, hooks
    namespace = {'__name__': 'probe', 'RG_OUT': str(tmp), 'sdk': sdk}
    exec(compile(SCRIPT.read_text(encoding='utf-8'), str(SCRIPT), 'exec'), namespace)
    return namespace


def bound(name):
    """The shape the SDK really passes a hook: a bound function, with the function itself under .func. It has no .Name of its own."""
    return Obj(func=Obj(Name=name), object=None)


def call(ns, fn, by, **values):
    ns['_ae_on_call'](Obj(Class=Obj(Name=by)), args(**values), None, bound(fn))


def rows_at(ns, *timed):
    """A window of (time, fn, event) rows, written directly so a test controls the clock."""
    ns['ae_begin']('w')
    ns['ae']['rows'] = [{'t': t, 'fn': fn, 'by': 'Actor', 'event': event} for t, fn, event in timed]


def test_a_hook_records_and_never_raises():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        call(ns, 'PostAkEvent', 'Actor', InAkEvent=event('Shot'))     # no window open: not recorded
        assert ns['ae']['rows'] == []
        ns['ae_begin']('w')
        ns['_ae_on_call'](None, None, None, None)                     # a read that fails is counted, not raised into the game
        call(ns, 'PostAkEvent', 'Actor', InAkEvent=event('Shot'), Other=Obj(Name='notasound'))
        result = ns['ae_end']()
        assert result['errors'] == 1 and result['rows'] == 1 and 'AttributeError' in result['first_error']
        assert result['counts'] == {'PostAkEvent|Actor|Pkg.Shot': 1}
        rows = (Path(tmp) / 'events_w.jsonl').read_text(encoding='utf-8').splitlines()
        assert json.loads(rows[0])['event'] == 'Pkg.Shot'


def test_the_first_runs_mistake_is_counted_with_its_text_and_not_in_the_source():
    # A function object with a name but no .func is what the old code wanted and the real hook never sends: the first run lost all its calls this way.
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['ae_begin']('w')
        ns['_ae_on_call'](Obj(Class=Obj(Name='Actor')), args(), None, Obj(Name='PostAkEvent'))
        result = ns['ae_end']()
        assert result['rows'] == 0 and result['errors'] == 1 and "'func'" in result['first_error']
    assert not re.search(r'(?<!func\.)func\.Name', SCRIPT.read_text(encoding='utf-8').replace('`func.Name`', ''))


def test_errors_and_their_text_reset_with_each_window():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['ae_begin']('a')
        ns['_ae_on_call'](None, None, None, None)
        ns['ae_end']()
        ns['ae_begin']('b')
        result = ns['ae_end']()
        assert result['errors'] == 0 and result['first_error'] is None


def test_a_bad_event_read_keeps_the_row_and_counts_the_error():
    def broken():
        raise RuntimeError('no such type')
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['ae_begin']('w')
        ns['_ae_on_call'](Obj(Class=Obj(Name='WillowWeapon')), Obj(_type=Obj(_properties=broken)), None, bound('PlayFiringSound'))
        result = ns['ae_end']()
        assert result['rows'] == 1 and result['calls'] == {'PlayFiringSound': 1} and result['errors'] == 1 and 'no such type' in result['first_error']


def test_a_call_with_no_arguments_struct_is_a_row():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['ae_begin']('w')
        ns['_ae_on_call'](Obj(Class=Obj(Name='WillowWeapon')), None, None, bound('PlayFiringSound'))
        result = ns['ae_end']()
        assert result['rows'] == 1 and result['errors'] == 0


def test_a_window_with_hook_errors_fails_its_stage_with_the_first_errors_text():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    start = text.index('function Get-HookErrors')
    function = text[start:text.index('function Send-Shots')]
    body = function + r"""
$bad = '{"errors": 42, "first_error": "AttributeError(boom)", "tied": ["Pkg.Shot"]}' | ConvertFrom-Json
$good = '{"errors": 0, "first_error": null, "tied": []}' | ConvertFrom-Json
"BAD=" + (Get-HookErrors $bad)
"GOOD=[" + (Get-HookErrors $good) + "]"
"NONE=" + (Get-HookErrors $null)
"""
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'errors.ps1'
        script.write_text(body, encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, timeout=120)
    assert 'BAD=hooks failed 42 times, first: AttributeError(boom)' in done.stdout, done.stdout + done.stderr
    assert 'GOOD=[]' in done.stdout and 'NONE=no window' in done.stdout, done.stdout
    # Every stage asks it, and a tied event cannot outvote it.
    assert text.count('(!$castErrors -and') == 1 and text.count('(!$awayErrors -and') == 1
    assert text.count('(!$hitErrors -and') == 1 and text.count('(!$pickupErrors -and') == 1 and "Set-Stage 'baseline' (!$idleErrors)" in text


def test_a_behavior_gives_its_own_event_and_a_marker_gives_none():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        behavior = Obj(Class=Obj(Name='Behavior_PostAkEvent'), Event=event('Cast'))
        assert ns['_ae_event'](behavior, args(SelfObject=Obj(Name='Maya'))) == 'Pkg.Cast'
        assert ns['_ae_event'](Obj(Class=Obj(Name='WillowWeapon')), args()) is None


def test_new_events_exclude_the_baselines_and_ignore_the_route():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['ae_begin']('fire_away')
        call(ns, 'PlayFiringSound', 'WillowWeapon')
        call(ns, 'WeaponPlayAkEvent', 'WillowWeapon', Event=event('Shot'))
        away = ns['ae_end'](['idle'])
        assert away['new'] == ['Pkg.Shot'] and away['calls']['PlayFiringSound'] == 1
        ns['ae_begin']('fire_hit')
        call(ns, 'PostAkEvent', 'Actor', InAkEvent=event('Shot'))     # the same sound by another function is not new
        call(ns, 'PlayImpactEffectSound', 'WillowPhysicalMaterialProperty', Event=event('Impact'))
        assert ns['ae_end'](['idle', 'fire_away'])['new'] == ['Pkg.Impact']


def test_no_new_event_gives_an_empty_list_for_the_stage_to_refuse():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['ae_begin']('idle')
        call(ns, 'PostAkEvent', 'Actor', InAkEvent=event('Wind'))
        ns['ae_end']()
        ns['ae_begin']('pickup')
        call(ns, 'PostAkEvent', 'Actor', InAkEvent=event('Wind'))
        assert ns['ae_end'](['idle'])['new'] == []


def test_an_ambient_event_in_the_window_is_new_but_not_tied():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        # Shots at 10, 11, 12. The gun sound follows each; the wind blows once, 0.7 s after a shot.
        rows_at(ns, (10.0, 'PlayFiringSound', None), (10.01, 'PostAkEvent', 'Shot'), (11.0, 'PlayFiringSound', None), (11.01, 'PostAkEvent', 'Shot'),
                (11.7, 'PostAkEvent', 'Wind'), (12.0, 'PlayFiringSound', None), (12.01, 'PostAkEvent', 'Shot'))
        result = ns['ae_end'](['idle'], [], 'PlayFiringSound', 0.4, 2)
        assert result['new'] == ['Shot', 'Wind'] and result['tied'] == ['Shot'] and result['anchored_by'] == 'marker'


def test_an_event_near_only_one_of_several_shots_is_not_tied():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        rows_at(ns, (10.0, 'PlayFiringSound', None), (10.1, 'PostAkEvent', 'Chirp'), (11.0, 'PlayFiringSound', None), (12.0, 'PlayFiringSound', None))
        assert ns['ae_end'](['idle'], [], 'PlayFiringSound', 0.4, 2)['tied'] == []


def test_an_event_before_the_action_is_not_tied():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        rows_at(ns, (9.9, 'PostAkEvent', 'Early'))
        assert ns['ae_end']([], [10.0], '', 1.0, 1)['tied'] == []


def test_one_event_firing_many_times_after_one_shot_counts_once():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        rows_at(ns, (10.0, 'PlayFiringSound', None), (10.1, 'PostAkEvent', 'Burst'), (10.2, 'PostAkEvent', 'Burst'), (11.0, 'PlayFiringSound', None))
        assert ns['ae_end']([], [], 'PlayFiringSound', 0.4, 2)['tied'] == []


def test_presses_are_used_when_the_marker_is_too_rare_and_the_result_says_so():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        rows_at(ns, (10.0, 'PlayFiringSound', None), (10.1, 'PostAkEvent', 'Shot'), (11.1, 'PostAkEvent', 'Shot'))
        result = ns['ae_end']([], [10.0, 11.0], 'PlayFiringSound', 0.4, 2)
        assert result['anchored_by'] == 'press' and result['anchors'] == 2 and result['tied'] == ['Shot']


def test_no_moments_means_nothing_is_tied():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        rows_at(ns, (10.0, 'PostAkEvent', 'Wind'))
        result = ns['ae_end']()
        assert result['new'] == ['Wind'] and result['tied'] == []


def test_overlap_names_the_other_windows_that_saw_a_tied_event():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['ae_begin']('a')
        ns['ae']['rows'] = [{'t': 10.1, 'fn': 'PostAkEvent', 'by': 'Actor', 'event': 'Shared'}]
        ns['ae_end']([], [10.0], '', 0.5, 1)
        ns['ae_begin']('b')
        ns['ae']['rows'] = [{'t': 20.1, 'fn': 'PostAkEvent', 'by': 'Actor', 'event': 'Shared'}, {'t': 20.1, 'fn': 'PostAkEvent', 'by': 'Actor', 'event': 'Only'}]
        ns['ae_end']([], [20.0], '', 0.5, 1)
        overlap = ns['ae_overlap']()
        assert overlap['b'] == {'Only': [], 'Shared': ['a']} and overlap['a'] == {'Shared': ['b']}


def test_the_stages_accept_only_tied_events_and_no_pawn_is_held():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    body = text[:text.index('\n} catch {\n    $runError')]
    stages = [line for line in body.splitlines() if line.strip().startswith('Set-Stage ') and "'hooks'" not in line and "'baseline'" not in line]
    assert len(stages) == 4 and all('.tied' in line or 'castTied' in line for line in stages), stages
    assert '.new).Count' not in body
    # The recorder holds a target pawn on every frame; this capture only needs the skill's own lock call.
    assert 'pi_start' not in body and 'pi_result' not in body and 'mark_lock' not in body


def test_rows_stop_at_the_limit_and_say_so():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['AE_MAX_ROWS'] = 2
        ns['ae_begin']('w')
        for _ in range(5):
            call(ns, 'PostAkEvent', 'Actor', InAkEvent=event('Shot'))
        result = ns['ae_end']()
        assert result['rows'] == 2 and result['dropped'] == 3


def test_install_reports_what_is_missing_and_remove_undoes_it():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp, functions={'Engine.Actor:PostAkEvent'})
        result = ns['ae_install']()
        assert result['hooked'] == ['Engine.Actor:PostAkEvent'] and 'Engine.Actor:PlayAkEvent' in result['missing']
        ns['ae_remove']()
        assert [c[0] for c in ns['sdk'].calls] == ['add', 'remove'] and ns['ae']['hooks'] == []


def test_a_hook_has_no_way_to_change_the_call():
    source = SCRIPT.read_text(encoding='utf-8')
    assert 'Block' not in source and 'Type.POST' not in source


def test_target_reads_health_with_a_fallback():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['ai_pawn'] = lambda name: Obj(Health=7)
        assert ns['ae_target']('x') == {'gone': False, 'health': 7.0}
        ns['ai_pawn'] = lambda name: Obj(GetHealth=lambda: 3)
        assert ns['ae_target']('x') == {'gone': False, 'health': 3.0}


def test_gone_and_zero_health_are_different_answers():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)

        def gone(name):
            raise LookupError(name)
        ns['ai_pawn'] = gone
        assert ns['ae_target']('x') == {'gone': True, 'health': None}
        ns['ai_pawn'] = lambda name: Obj(bDeleteMe=True, Health=5)
        assert ns['ae_target']('x') == {'gone': True, 'health': None}
        ns['ai_pawn'] = lambda name: Obj(bDeleteMe=False, Health=0)
        assert ns['ae_target']('x') == {'gone': False, 'health': 0.0}


def test_throw_with_an_empty_backpack_throws_nothing():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['get_pc'] = lambda: Obj(GetPawnInventoryManager=lambda: Obj(Backpack=[]))
        assert ns['ae_throw']()['thrown'] is None


def named(cls, name):
    return Obj(Class=Obj(Name=cls), GetShortHumanReadableName=lambda: name, Name=name)


def throw_setup(ns, items):
    thrown = []
    manager = Obj(Backpack=items, ThrowBackpackInventory=thrown.append)
    ns['get_pc'] = lambda: Obj(GetPawnInventoryManager=lambda: manager, Pawn=Obj(Location=Obj(X=0.0, Y=0.0, Z=0.0), BaseEyeHeight=60.0))
    return thrown


def test_throw_prefers_a_gun_and_never_a_usable_item():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        diary, shield, gun = named('WillowUsableItem', 'Diary'), named('WillowShield', 'Shield'), named('WillowWeapon', 'Gun')
        thrown = throw_setup(ns, [diary, shield, gun])
        assert ns['ae_throw']()['thrown'] == 'WillowWeapon:Gun' and thrown == [gun] and ns['ae']['thrown'] == 'WillowWeapon:Gun'
        thrown = throw_setup(ns, [diary, shield])
        result = ns['ae_throw']()
        assert result['thrown'] == 'WillowShield:Shield' and result['is_gun'] is False and thrown == [shield]
        thrown = throw_setup(ns, [diary])
        assert ns['ae_throw']()['thrown'] is None and thrown == [] and ns['ae']['thrown'] is None


def pickup(name, inventory, x):
    return Obj(Name=name, Inventory=inventory, bDeleteMe=False, Location=Obj(X=x, Y=0.0, Z=0.0))


def test_face_goes_to_the_thrown_items_pickup_not_the_nearest():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        throw_setup(ns, [named('WillowWeapon', 'Gun')])
        ns['ae_throw']()
        near, own = pickup('Near', named('WillowWeapon', 'Loot'), 10.0), pickup('Own', named('WillowWeapon', 'Gun'), 500.0)
        sys.modules['unrealsdk'].find_all = lambda *a, **k: [near, own]
        assert ns['ae_face_pickup']() == 'Own'
        sys.modules['unrealsdk'].find_all = lambda *a, **k: [near]     # another item's pickup answers another question
        assert ns['ae_face_pickup']() is None


def test_a_failed_read_costs_a_stage_not_the_session():
    def broken():
        raise RuntimeError('no such field')
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['get_pc'] = broken
        assert ns['ae_weapon']() == {'error': 'no such field'}
        assert ns['ae_throw']() == {'thrown': None, 'error': 'no such field'}
        assert ns['ae_face_pickup']() is None


def test_an_empty_json_list_counts_as_zero_in_windows_powershell():
    # The stage checks are @($log.window_x.new).Count -gt 0: an empty `new` must not count as one item.
    command = "$r = '{\"new\": []}' | ConvertFrom-Json; exit @($r.new).Count"
    assert subprocess.run(['powershell', '-NoProfile', '-Command', command], timeout=60).returncode == 0


def test_no_pickup_means_no_target():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['get_pc'] = lambda: Obj(Pawn=Obj(Location=Obj(X=0.0, Y=0.0, Z=0.0)))
        assert ns['ae_face_pickup']() is None
        assert ns['ae_pickup_gone']('anything') is True


def run_tail(body, clean=True, proc='$null', owned='$true', attempted=None, launched='$true', stubs='function Remove-Driver { }', out=None, setup=''):
    """This script's real tail, from its `} catch {` on, after a stub body: the harness of tests/realgame_snapshot_test.py."""
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    tail = text[text.index('\n} catch {\n    $runError') + 1:]
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'driver.ps1'
        script.write_text(snapshot.TAIL.replace('__OUT__', out or tmp).replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__TMP__', tmp)
                          .replace('__SETUP__', setup.replace('__TMP__', tmp)).replace('__LAUNCHED__', launched).replace('__OWNED__', owned)
                          .replace('__ATTEMPTED__', attempted or owned).replace('__STUBS__', stubs).replace('__BODY__', body)
                          .replace('__CAPTURE_TAIL__', tail).replace('__PROC__', proc)
                          .replace('__CLEAN__', '$true' if clean else '$false'), encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True,
                              text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp), timeout=180)
        done.hub_lock_kept = (Path(tmp) / 'hub.lock').exists()
        return done


FINISH = "$stagesDone.Add('hit')"
ALIVE = '[pscustomobject]@{ HasExited = $false; Id = 1 }'
NO_RESTORE = "function Remove-Driver { Write-Host 'driver removed' }\nfunction Restore-VerifiedRoots { throw 'must not run' }\n"


def test_exit_0_only_for_a_completed_run_with_nothing_to_recover():
    for body, clean, proc, code, status in ((FINISH, True, '$null', 0, 'completed'),
                                            (FINISH + '; return', True, '$null', 0, 'completed'),
                                            (FINISH, False, '$null', 1, 'recovery needed: restore'),
                                            ("'no stage'", True, '$null', 1, 'incomplete: hit'),
                                            ("throw 'x'", True, '$null', 1, 'failed: x'),
                                            (FINISH, True, ALIVE, 1, 'recovery needed: game exit')):
        done = run_tail(body, clean, proc)
        assert done.returncode == code and f'status: {status}' in done.stdout, (body, clean, done.returncode, done.stdout, done.stderr)
        assert done.hub_lock_kept is status.startswith('recovery'), (body, done.stdout)


def test_a_bad_restore_prints_the_rows_and_the_one_recovery_command():
    done = run_tail("'body finished'", clean=False)
    order = [done.stdout.find(text) for text in ('status: recovery needed', 'MISMATCH after restore', 'MANUAL RECOVERY', 'Restore-VerifiedBackup')]
    assert -1 not in order and order == sorted(order), done.stdout


def test_a_failed_run_stops_this_captures_hooks_on_the_exact_answer_only():
    def reply(answer):
        return "function Remove-Driver { }\nfunction Py([string]$Code, [int]$Timeout = 30) { 'py: ' + $Code | Write-Host; " + answer + ' }'
    done = run_tail("throw 'boom'", True, ALIVE, stubs=reply("'stopped'"))
    assert 'py: ae_remove(); protect_player(False)' in done.stdout and 'cleanup hooks: stopped' in done.stdout, done.stdout
    done = run_tail("throw 'boom'", True, ALIVE, stubs=reply("'Traceback: stopped'"))
    assert 'cleanup hooks: not stopped: Traceback: stopped' in done.stdout, done.stdout
    done = run_tail("throw 'boom'", True, ALIVE, stubs=reply("throw 'no reply'"))
    assert 'cleanup hooks: not stopped' in done.stdout and 'cleanup game:' in done.stdout, done.stdout


def test_a_game_this_run_did_not_start_stops_the_restore_and_holds_the_locks():
    foreign = NO_RESTORE + "function Get-GameProcesses { [pscustomobject]@{ Id = 77; StartTime = Get-Date; Path = 'x' } }"
    done = run_tail(FINISH, stubs=foreign)
    assert done.returncode == 1 and 'a game this run did not start is running (pid 77)' in done.stdout, done.stdout
    assert done.hub_lock_kept is True and 'MANUAL RECOVERY' in done.stdout and 'NOT RESTORED' in done.stdout and 'driver removed' not in done.stdout, done.stdout


def test_a_run_that_touched_nothing_has_nothing_to_recover():
    done = run_tail("throw 'copy failed'", owned='$false', launched='$false', stubs=NO_RESTORE)
    assert 'status: failed: copy failed' in done.stdout and 'cleanup saves: not touched' in done.stdout and done.hub_lock_kept is False, done.stdout
    assert done.returncode == 1 and 'MANUAL RECOVERY' not in done.stdout, done.stdout


def test_missing_evidence_cannot_exit_0():
    done = run_tail(FINISH, setup="New-Item -ItemType Directory -Force '__TMP__\\session.json' | Out-Null")
    assert done.returncode == 1 and 'status: failed: evidence not written' in done.stdout, done.stdout


def test_the_tail_is_the_helper_branch_one_but_for_the_hooks_it_stops():
    def tail_of(path):
        text = path.read_text(encoding='utf-8').replace('\r\n', '\n')
        return text[text.index('\n} catch {\n    $runError'):]
    ours, theirs = tail_of(CAPTURE), tail_of(ROOT / 'tools/real_game/phaselock_integrity_capture.ps1')
    # The one difference: this capture has no recorder, so it stops its own hooks. If the other script's tail changes, this fails: re-copy it.
    comment = ("    # A throw between pi_start and pi_stop leaves the per-frame hook reading a held pawn into shutdown: stop it while the game can still answer.\n",
               "    # A throw can leave this run's logging hooks and the player's protection on: stop them while the game can still answer.\n")
    call = ("""Py 'pi_stop(); unmark_lock(); print("stopped")' 5""", """Py 'ae_remove(); protect_player(False); print("stopped")' 5""")
    assert theirs.count(comment[0]) == 1 and theirs.count(call[0]) == 1
    assert ours == theirs.replace(*comment).replace(*call)
    assert 'pi_stop' not in ours and 'ae_remove(); protect_player(False)' in ours
    for needed in ('$recovery = Get-RecoveryReason $gameGone $cleanup.game $restored $restoreError $cleanup.driver $untracked', '$holdLocks = [bool]$recovery',
                   'Format-ManualRecovery', "$untracked = if ($touched) { Get-ForeignGamesSeen $proc } else { '' }", 'if ($restored) {'):
        assert needed in ours, needed


def test_saves_are_proved_blocked_before_any_input_and_nothing_bypasses_the_focus_guard():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    body = text[:text.index('\n} catch {\n    $runError')]
    order = [body.index(step) for step in ('# Stale evidence:', 'Enter-RunLock', 'Backup-Saves', 'Save-VerifiedBackupRecord', '$driverAttempted = $true', 'Install-Driver -BlockSavesAtStart -Exclusive',
                                           '$foreign = Get-ForeignGamesSeen $null', '$launchAttempted = $true', 'Start-Game $gameArgs',
                                           "Assert-SavesBlocked (Py 'print(block_saves())')", 'Send-Key Enter', 'ae_throw()')]
    assert order == sorted(order), order
    assert 'Focus-Game' not in body and 'OWRG.Native' not in body     # all input goes through the guarded sends
    reference = (ROOT / 'tools/real_game/phaselock_integrity_capture.ps1').read_text(encoding='utf-8').replace('\r\n', '\n')
    block = reference[reference.index('# Stale evidence:'):reference.index('# End of stale evidence.')]
    assert block in body


def test_stale_event_files_move_aside_before_the_run_and_keep_their_contents():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    start = text.index('# Stale evidence:')
    block = text[start:text.index('# End of stale event files.')]
    assert start < text.index('Enter-RunLock')
    with tempfile.TemporaryDirectory() as tmp:
        (Path(tmp) / 'events_phaselock_try2.jsonl').write_text('{"old": 1}', encoding='utf-8')
        (Path(tmp) / 'other.txt').write_text('stays', encoding='utf-8')
        script = Path(tmp) / 'stale.ps1'
        script.write_text(f"$ErrorActionPreference = 'Stop'\n$Out = '{tmp}'\n{block}", encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, timeout=120)
        assert done.returncode == 0, done.stdout + done.stderr
        assert not list(Path(tmp).glob('events_*.jsonl')) and (Path(tmp) / 'other.txt').exists()
        kept = list(Path(tmp).glob('earlier-*/events_phaselock_try2.jsonl'))
        assert len(kept) == 1 and 'old' in kept[0].read_text(encoding='utf-8')
        # A second pass with nothing stale makes no empty folder.
        before = sorted(p.name for p in Path(tmp).iterdir())
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True, text=True, timeout=120)
        assert done.returncode == 0 and sorted(p.name for p in Path(tmp).iterdir()) == before


def test_all_four_roots_are_protected():
    text = CAPTURE.read_text(encoding='utf-8')
    for needed in ('$snapshots[$d] = Assert-SnapshotMatches', '$snapshots[$d] = New-VerifiedSnapshot', 'Save-VerifiedBackupRecord $bak $roots $snapshots',
                   "'SaveData', 'Config'", 'sdk_mods/settings', 'sdk_mods/SanitySaver', 'Install-Driver -BlockSavesAtStart -Exclusive', 'Enter-RunLock'):
        assert needed in text, needed


def test_the_capture_parses():
    command = "$e = $null; [void][Management.Automation.Language.Parser]::ParseFile('%s', [ref]$null, [ref]$e); exit @($e).Count" % CAPTURE
    assert subprocess.run(['powershell', '-NoProfile', '-Command', command], timeout=60).returncode == 0


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
