"""Synthetic tests for the sound-event capture (no game: unrealsdk is stubbed; the objects and names are invented). The capture's
cleanup is run by the same harness as tests/realgame_snapshot_test.py. Needs Windows PowerShell.

Run: python tests/audio_events_capture_test.py
"""
import json
import os
from pathlib import Path
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


def call(ns, fn, by, **values):
    ns['_ae_on_call'](Obj(Class=Obj(Name=by)), args(**values), None, Obj(Name=fn))


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
        assert result['errors'] == 1 and result['rows'] == 1
        assert result['counts'] == {'PostAkEvent|Actor|Pkg.Shot': 1}
        rows = (Path(tmp) / 'events_w.jsonl').read_text(encoding='utf-8').splitlines()
        assert json.loads(rows[0])['event'] == 'Pkg.Shot'


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
    stages = [line for line in body.splitlines() if line.strip().startswith('Set-Stage ') and "'hooks'" not in line]
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


def test_health_falls_back_to_the_field():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['ai_pawn'] = lambda name: Obj(Health=7)
        assert ns['ae_health']('x') == 7.0
        ns['ai_pawn'] = lambda name: Obj(GetHealth=lambda: 3)
        assert ns['ae_health']('x') == 3.0


def test_a_target_that_is_gone_counts_as_health_zero():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)

        def gone(name):
            raise LookupError(name)
        ns['ai_pawn'] = gone
        assert ns['ae_health']('x') == 0.0


def test_throw_with_an_empty_backpack_throws_nothing():
    with tempfile.TemporaryDirectory() as tmp:
        ns = load(tmp)
        ns['get_pc'] = lambda: Obj(GetPawnInventoryManager=lambda: Obj(Backpack=[]))
        assert ns['ae_throw']() == {'thrown': None}


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


def run_tail(body, clean=True, proc='$null'):
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    tail = text[text.index('\n} catch {\n    $runError') + 1:]
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / 'driver.ps1'
        script.write_text(snapshot.TAIL.replace('__OUT__', tmp).replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__TMP__', tmp)
                          .replace('__SETUP__', '').replace('__OWNED__', '$true').replace('__STUBS__', 'function Remove-Driver { }')
                          .replace('__BODY__', body).replace('__CAPTURE_TAIL__', tail).replace('__PROC__', proc)
                          .replace('__CLEAN__', '$true' if clean else '$false'), encoding='utf-8-sig')
        done = subprocess.run(['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(script)], capture_output=True,
                              text=True, env=dict(os.environ, OPENWILLOW_BL2=tmp), timeout=180)
        done.hub_lock_kept = (Path(tmp) / 'hub.lock').exists()
        return done


def test_exit_0_only_for_a_completed_run_with_nothing_to_recover():
    finish = "$stagesDone.Add('hit')"
    never_exits = '[pscustomobject]@{ HasExited = $false; Id = 1 }'
    for body, clean, proc, code, status in ((finish, True, '$null', 0, 'completed'),
                                            (finish + '; return', True, '$null', 0, 'completed'),
                                            (finish, False, '$null', 1, 'recovery needed: restore'),
                                            ("'no stage'", True, '$null', 1, 'incomplete: hit'),
                                            ("throw 'x'", True, '$null', 1, 'failed: x'),
                                            (finish, True, never_exits, 1, 'recovery needed: game exit')):
        done = run_tail(body, clean, proc)
        assert done.returncode == code and f'status: {status}' in done.stdout, (body, clean, done.returncode, done.stdout, done.stderr)
        assert done.hub_lock_kept is status.startswith('recovery'), (body, done.stdout)


def test_a_bad_restore_prints_the_rows_and_the_one_recovery_command():
    done = run_tail("'body finished'", clean=False)
    order = [done.stdout.find(text) for text in ('status: recovery needed', 'MISMATCH after restore', 'MANUAL RECOVERY', 'Restore-VerifiedBackup')]
    assert -1 not in order and order == sorted(order), done.stdout


def test_the_tail_is_the_helper_branch_one_but_for_the_hooks_it_stops():
    def tail_of(path):
        text = path.read_text(encoding='utf-8').replace('\r\n', '\n')
        return text[text.index('\n} catch {\n    $runError'):]
    ours, theirs = tail_of(CAPTURE), tail_of(ROOT / 'tools/real_game/phaselock_integrity_capture.ps1')
    # The one block that differs: this capture has no recorder to stop. If the other script's tail changes, this fails: re-copy it.
    start, end = '    # A throw between pi_start and pi_stop', '    $gameGone = $true\n'
    theirs_rest = theirs[:theirs.index(start)] + theirs[theirs.index(end):]
    our_start = '    # Stop this run\'s logging hooks'
    ours_rest = ours[:ours.index(our_start)] + ours[ours.index(end):]
    assert ours_rest == theirs_rest
    assert 'pi_stop' not in ours and 'ae_remove(); protect_player(False)' in ours
    for needed in ('$recovery = Get-RecoveryReason', '$holdLocks = [bool]$recovery', 'Format-ManualRecovery'):
        assert needed in ours, needed


def test_saves_are_proved_blocked_before_any_input_and_nothing_bypasses_the_focus_guard():
    text = CAPTURE.read_text(encoding='utf-8').replace('\r\n', '\n')
    body = text[:text.index('\n} catch {\n    $runError')]
    order = [body.index(step) for step in ('Backup-Saves', 'Save-VerifiedBackupRecord', 'Install-Driver -BlockSavesAtStart -Exclusive', 'Start-Game $gameArgs',
                                           "Assert-SavesBlocked (Py 'print(block_saves())')", 'Send-Key Enter', 'ae_throw()')]
    assert order == sorted(order), order
    assert 'Focus-Game' not in body


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
