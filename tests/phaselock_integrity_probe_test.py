"""Synthetic tests for the new reads of tools/real_game/scripts/phaselock_integrity.py (no game: unrealsdk and the namespace it
expects are stubbed; the objects and numbers are invented). They catch mistakes that would otherwise only show in a real-game session.

Run: python tests/phaselock_integrity_probe_test.py
"""
import sys
import tempfile
import types
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / 'tools' / 'real_game' / 'scripts' / 'phaselock_integrity.py'


class Obj:
    def __init__(self, **fields):
        self.__dict__.update(fields)

    def _path_name(self):
        return 'Pkg.' + self.__dict__.get('Name', 'Obj')


def load():
    sdk = types.ModuleType('unrealsdk')
    sdk.make_struct = lambda name, **f: Obj(**f)
    sdk.find_class = lambda name: name
    sdk.find_all = lambda *a, **k: []
    hooks = types.ModuleType('unrealsdk.hooks')
    hooks.Type = Obj(PRE=1, POST=2, POST_UNCONDITIONAL=3)
    hooks.add_hook = hooks.remove_hook = lambda *a, **k: None
    sdk.hooks = hooks
    sys.modules['unrealsdk'], sys.modules['unrealsdk.hooks'] = sdk, hooks
    namespace = {'__name__': 'probe', 'plain': lambda v: v, 'mark': lambda *a, **k: None, 'RG_OUT': tempfile.gettempdir()}
    exec(compile(SCRIPT.read_text(encoding='utf-8'), str(SCRIPT), 'exec'), namespace)
    return namespace


def test_bones_read_location_and_scale_and_survive_a_missing_bone():
    ns = load()
    axis = Obj(X=2.0, Y=0.0, Z=0.0)
    def location(name):
        if name == 'Head':
            raise ValueError('no such bone')
        return Obj(X=11.0, Y=22.0, Z=33.0)
    mesh = Obj(GetBoneLocation=location, MatchRefBone=lambda n: 3, GetBoneMatrix=lambda i: Obj(XPlane=axis, YPlane=axis, ZPlane=axis))
    rows = ns['_bone_rows'](mesh, Obj(Location=Obj(X=1.0, Y=2.0, Z=3.0)))
    assert rows['Hips'] == {'rel': [10.0, 20.0, 30.0], 'scale': [2.0, 2.0, 2.0]}
    assert rows['Head'] is None
    assert set(rows) == set(ns['PI_BONES'])


def test_assets_list_the_mesh_materials_lod_and_what_is_attached():
    ns = load()
    static = Obj(Class=Obj(Name='StaticMeshComponent'), Name='Spikes', StaticMesh=Obj(Name='SM_Spikes'), HiddenGame=False)
    attachment = Obj(Component=static, SocketName='Back', BoneName='Spine2')
    mesh = Obj(SkeletalMesh=Obj(Name='Skel'), Materials=[Obj(Name='Mat'), None], PredictedLODLevel=1, ForcedLodModel=0, MinLodModel=0,
               Attachments=[attachment])
    row = ns['_asset_row'](Obj(Attached=[Obj(Class=Obj(Name='Actor'), Name='A_1')], Components=[static]), mesh)
    assert row['skeletal_mesh'] == 'Pkg.Skel' and row['materials'] == ['Pkg.Mat', None]
    assert row['lod']['PredictedLODLevel'] == 1
    assert row['attachments'][0]['socket'] == 'Back' and row['attachments'][0]['StaticMesh'] == 'Pkg.SM_Spikes'
    assert row['components'][0]['name'] == 'Spikes' and row['attached_actors'] == ['Actor:A_1']


def test_assets_survive_a_mesh_without_those_fields():
    row = load()['_asset_row'](Obj(), Obj())
    assert row['attachments'] == [] and row['components'] == []


def injure_setup(ns):
    source = Obj(Name='Target', Controller='ctrl')
    ns['ai_pawn'] = lambda name: source
    me = Obj(bIsInjured=False, Location=Obj(), hits=0)
    def take(*args):
        me.hits += 1
    me.TakeDamage = take
    ns['get_pc'] = lambda: Obj(Pawn=me, WorldInfo=Obj(TimeSeconds=10.0))
    ns['_player'] = lambda pawn: {'injured': pawn.bIsInjured}
    return me


def test_injure_refuses_when_the_target_is_not_lifted():
    ns = load()
    me = injure_setup(ns)
    assert 'skipped' in ns['pi_injure']('Target')
    assert me.hits == 0 and ns['pi']['injure'] is None


def test_injure_repeats_until_injured_and_stops():
    ns = load()
    me = injure_setup(ns)
    ns['pi']['seen_lifted'] = {'Target'}
    ns['pi_injure']('Target')
    assert me.hits == 1
    assert ns['_tick_injure'](me, 10.2) is None             # not yet time
    assert ns['_tick_injure'](me, 10.5)['hit'] == 2
    me.bIsInjured = True
    assert ns['_tick_injure'](me, 11.0) == {'injured_after_hits': 2}
    assert ns['_tick_injure'](me, 12.0) is None and me.hits == 2


def test_injure_gives_up_after_eight_hits():
    ns = load()
    me = injure_setup(ns)
    ns['pi']['seen_lifted'] = {'Target'}
    ns['pi_injure']('Target')
    now, last = 10.0, None
    for _ in range(12):
        now += 0.6
        last = ns['_tick_injure'](me, now) or last
    assert 'gave_up_after_hits' in last and me.hits == 8


def test_result_says_whether_the_target_was_lifted():
    ns = load()
    ns['pi']['pawn'] = Obj(Name='Target', Controller='ctrl')
    ns['pi']['seen_lifted'] = {'Other'}
    result = ns['pi_result']()
    assert result['lifted'] == ['Other'] and result['target_lifted'] is False and result['target_has_controller'] is True
    ns['pi']['seen_lifted'] = {'Target', 'Other'}
    assert ns['pi_result']()['target_lifted'] is True


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
