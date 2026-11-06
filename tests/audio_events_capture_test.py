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
        assert away['new'] == ['Pkg.Shot'] and away['firing_calls'] == 1
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
        script.write_text(snapshot.TAIL.replace('__REALGAME__', str(ROOT / 'tools/real_game/realgame.ps1')).replace('__TMP__', tmp)
                          .replace('__SETUP__', '').replace('__BODY__', body).replace('__CAPTURE_TAIL__', tail).replace('__PROC__', proc)
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


def test_the_tail_is_the_one_that_ran_clean_and_all_four_roots_are_protected():
    def tail_of(path):
        text = path.read_text(encoding='utf-8').replace('\r\n', '\n')
        return text[text.index('\n} catch {\n    $runError'):]
    assert tail_of(CAPTURE) == tail_of(ROOT / 'tools/real_game/phaselock_integrity_capture.ps1')
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
