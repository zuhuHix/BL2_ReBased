"""Synthetic tests for tools/prepare_slice_world.py, tools/slice_values.py and the
binding helpers added to tools/prepare_mover.py (no game data).

Every path, number and name below is invented; the dictionaries only have the
shape the project's reader returns. Run: python tests/slice_world_test.py
"""
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import prepare_mover as pm  # noqa: E402
import prepare_slice_world as w  # noqa: E402
import slice_values as v  # noqa: E402


def tag(name, value, status='decoded'):
    return {'name': name, 'value': value, 'status': status}


def raises(fn, text):
    try:
        fn()
    except ValueError as error:
        assert text in str(error), error
        return
    raise AssertionError('expected ValueError: ' + text)


# ---------------------------------------------------------------- reader JSON -> values
def test_fields_flattens_nested_structs_and_arrays():
    props = [tag('Location', {'X': 1, 'Y': 2, 'Z': 3}),
             tag('Info', [tag('Linked', {'index': -5, 'path': 'Pkg.Obj'})]),
             tag('Nodes', [[tag('Node', {'index': 7, 'path': 'Level.NodeB'}), tag('Weight', 1)]]),
             tag('Refs', [{'index': 3, 'path': 'Level.A'}]),
             tag('Skipped', None, 'unsupported')]
    f = w.fields(props)
    assert f['Location'] == {'X': 1, 'Y': 2, 'Z': 3}
    assert w.ref(f['Info']['Linked']) == 'Pkg.Obj'
    assert w.ref(f['Nodes'][0]['Node']) == 'Level.NodeB'
    assert w.ref(f['Refs'][0]) == 'Level.A'
    assert 'Skipped' not in f
    assert w.ref({'index': 0, 'path': None}) is None


def test_placement_uses_scene_pipeline_conversion():
    p = w.placement({'Location': {'X': 10, 'Y': -20, 'Z': 30}, 'Rotation': {'Yaw': -16384, 'Pitch': 8192}})
    assert p['ue3'] == {'location': [10.0, -20.0, 30.0], 'rotation_units': [8192, -16384, 0]}
    assert p['host']['location'] == [10, -20, 30]
    assert p['host']['rotation'] == [45.0, -90.0, 0.0]


# ---------------------------------------------------------------- geometry oracles
def test_cylinder_containment():
    assert w.in_cylinder([3, 4, 9], [0, 0, 0], 5, 10)
    assert not w.in_cylinder([3, 4.1, 0], [0, 0, 0], 5, 10)
    assert not w.in_cylinder([0, 0, 10.5], [0, 0, 0], 5, 10)


def test_point_segment_distance():
    assert w.point_segment_distance([5, 3, 0], [0, 0, 0], [10, 0, 0]) == 3
    assert w.point_segment_distance([-4, 3, 0], [0, 0, 0], [10, 0, 0]) == 5
    assert w.point_segment_distance([1, 1, 1], [0, 0, 0], [0, 0, 0]) == math.sqrt(3)


def test_follow_chain_linear_branch_loop():
    nodes = {'A': {'next': ['B']}, 'B': {'next': ['C']}, 'C': {'next': []}}
    assert w.follow_chain(nodes, 'A') == ['A', 'B', 'C']
    nodes['C']['next'] = ['A']
    raises(lambda: w.follow_chain(nodes, 'A'), 'loops')
    nodes['C']['next'] = ['D', 'E']
    raises(lambda: w.follow_chain(nodes, 'A'), 'Branching')


def test_scene_nearest_placement():
    locations = [[0, 0, 0], [100, 0, 500], [1e6, 1e6, 1e6]]
    result = w.scene_bounds_check(locations, {'near': [100, 30, 0], 'origin': [0, 0, 99]})
    checks = {c['name']: c['nearest_placement_planar'] for c in result['checks']}
    assert checks == {'near': 30.0, 'origin': 0.0}
    assert result['placements'] == 3


# ---------------------------------------------------------------- attachment socket and target names
def close(a, b):
    return all(abs(x - y) < 1e-6 for x, y in zip(a, b))


def test_rotate_and_child_location_follow_unreal_rotators():
    assert close(w.rotate([0, 90, 0], [1, 0, 0]), [0, 1, 0])      # yaw +90: X -> Y
    assert close(w.rotate([90, 0, 0], [1, 0, 0]), [0, 0, 1])      # pitch +90: X -> Z
    assert close(w.rotate([0, 0, 90], [0, 1, 0]), [0, 0, -1])     # roll +90: Y -> -Z (FRotationMatrix's Y row)
    parent = {'location': [100, 200, 300], 'rotation': [0, -90, 0]}
    assert close(w.child_location(parent, {'location': [10, 0, 5], 'rotation': [0, 0, 0]}), [100, 190, 305])


def attachments(*components):
    return {'Attachments': [{'Data': {'ComponentData': {'Component': {'index': i + 1, 'path': path}, 'bAttachToMesh': False,
                                                         'MeshSocketName': 'None'}}} for i, path in enumerate(components)]}


def test_attach_socket_picks_the_named_socket_component():
    components = {'Def.Mesh_1': ('Engine.StaticMeshComponent', {}),
                  'Def.Socket_1': ('Engine.SocketComponent', {'SocketName': 'Grip', 'Translation': {'X': 1, 'Y': 2, 'Z': 3}}),
                  'Def.Socket_2': ('Engine.SocketComponent', {'SocketName': 'Hook', 'Translation': {'X': 4, 'Y': 5, 'Z': 6},
                                                              'Rotation': {'Pitch': 16384}})}
    body = attachments('Def.Mesh_1', 'Def.Socket_1', 'Def.Socket_2')
    path, pose = w.attach_socket(body, components, 'Hook')
    assert path == 'Def.Socket_2'
    assert pose['location'] == [4, 5, 6] and pose['rotation'] == [90.0, 0.0, 0.0] and pose['scale'] == [1, 1, 1]
    raises(lambda: w.attach_socket(body, components, 'Missing'), 'found 0')
    raises(lambda: w.attach_socket(attachments('Def.Socket_1', 'Def.Socket_1'), components, 'Grip'), 'found 2')
    on_mesh = attachments('Def.Socket_1')
    on_mesh['Attachments'][0]['Data']['ComponentData'].update(bAttachToMesh=True, MeshSocketName='Bone')
    raises(lambda: w.attach_socket(on_mesh, components, 'Grip'), 'mesh socket')


def test_playthrough_names_table():
    balance = {'PlayThroughs': [{'PlayThrough': 1, 'DisplayName': 'Plain',
                                 'TransformedNames': [{'Type': 'EAIT_A', 'TransformedName': 'Alpha'},
                                                      {'Type': 'EAIT_B', 'TransformedName': 'Beta'}]}]}
    assert w.playthrough_names(balance) == [{'playthrough': 1, 'display_name': 'Plain',
                                             'transformed_names': {'EAIT_A': 'Alpha', 'EAIT_B': 'Beta'}}]
    assert w.playthrough_names({}) == []


# ---------------------------------------------------------------- respawn rule
def station(name, location, can, active=False):
    return {'teleport_destination': name + '.Dest', 'location': location, 'can_resurrect': can, 'runtime_active': active}


def test_respawn_prefers_checkpoint_then_active_then_nearest_capable():
    stations = [station('Far', [1000, 0, 0], True), station('NearLevelTravel', [10, 0, 0], False),
                station('Mid', [500, 0, 0], True)]
    assert w.choose_respawn(stations, [0, 0, 0], active='Check.Dest') == ('Check.Dest', 'active checkpoint')
    assert w.choose_respawn(stations, [0, 0, 0]) == ('Mid.Dest', 'nearest resurrect-capable station')
    stations[0]['runtime_active'] = True
    assert w.choose_respawn(stations, [0, 0, 0]) == ('Far.Dest', 'active station')


def test_respawn_falls_back_to_nearest_other_station():
    stations = [station('A', [300, 0, 0], False), station('B', [100, 0, 0], False)]
    assert w.choose_respawn(stations, [0, 0, 0]) == ('B.Dest', 'nearest station')
    assert w.choose_respawn([], [0, 0, 0]) == (None, 'no station')


# ---------------------------------------------------------------- prepare_mover binding helpers
def test_event_keys_and_directions():
    keys = [[tag('Time', 0), tag('EventName', 'Opened')], [tag('Time', 1.5), tag('EventName', 'Done')]]
    assert pm.event_keys(keys, 1.5) == [{'time': 0, 'name': 'Opened'}, {'time': 1.5, 'name': 'Done'}]
    try:
        pm.event_keys([[tag('Time', 3), tag('EventName', 'Late')]], 1.5)
        raise AssertionError('late key accepted')
    except ValueError:
        pass
    assert pm.track_direction({}) == {'play_direction': 'ETPD_Both', 'fires_forward': True, 'fires_backward': True}
    back = pm.track_direction({'TrackPlayDirection': 'ETPD_PlayOnlyReverse', 'bFireEventsWhenForwards': False})
    assert back == {'play_direction': 'ETPD_PlayOnlyReverse', 'fires_forward': False, 'fires_backward': True}


# ---------------------------------------------------------------- slice_values
class FakePackage:
    def __init__(self, objects, classes):
        self.objects, self.classes = objects, classes

    def props(self, path):
        return self.objects[path]


def values_package():
    objects = {
        'Bal.Reward': {'ValueResolverChain': ['Bal.Reward.Const', 'Bal.Reward.Math']},
        'Bal.Reward.Const': {'ConstantValue': 0.25},
        'Bal.Reward.Math': {'Operand': 'MATHRESOLVEROPERAND_Mul',
                            'Argument': {'InitializationDefinition': 'Bal.PlaythroughInit'}},
        'Bal.PlaythroughInit': {'ConditionalInitialization': {
            'bEnabled': True, 'DefaultBaseValue': {'BaseValueConstant': 1},
            'ConditionalExpressionList': [{'BaseValueIfTrue': {'BaseValueAttribute': 'Bal.Second'}}]}},
        'Bal.Second': {'ValueResolverChain': ['Bal.Second.Const']},
        'Bal.Second.Const': {'ConstantValue': 2.0},
        'Bal.Mult': {'ValueResolverChain': ['Bal.Mult.Const']},
        'Bal.Mult.Const': {'ConstantValue': 10.0},
        'Bal.HealthInit': {'ValueFormula': {'bEnabled': True, 'Multiplier': {'BaseValueAttribute': 'Bal.Mult'},
                                            'Level': {'BaseValueConstant': 2},
                                            'Power': {'BaseValueAttribute': 'Bal.PlayerLevel'}},
                           'RangeRestriction': {'bEnableMinValueRestriction': True, 'MinValue': {'BaseValueConstant': 25}}},
        'Bal.Runtime': {'ValueResolverChain': ['Bal.Runtime.Object']},
        'Bal.Runtime.Object': {},
    }
    classes = {'Bal.Reward.Const': 'GearboxFramework.ConstantAttributeValueResolver',
               'Bal.Second.Const': 'GearboxFramework.ConstantAttributeValueResolver',
               'Bal.Mult.Const': 'GearboxFramework.ConstantAttributeValueResolver',
               'Bal.Reward.Math': 'GearboxFramework.SimpleMathValueResolver',
               'Bal.Runtime.Object': 'Engine.ObjectPropertyAttributeValueResolver'}
    return FakePackage(objects, classes)


def test_resolver_chain_and_conditional_branches():
    package = values_package()
    assert v.chain_value(package, 'Bal.Reward') == 0.25
    branches = v.init_value(package, {'InitializationDefinition': 'Bal.PlaythroughInit'}, {})
    assert branches == {'default': 1.0, 'conditional': 2.0}
    try:
        v.chain_value(package, 'Bal.Runtime')
        raise AssertionError('runtime resolver evaluated')
    except v.Unresolved:
        pass


def test_formula_attributes_and_level_formula():
    package = values_package()
    assert v.formula_attributes(package, 'Bal.HealthInit') == ['Bal.Mult', 'Bal.PlayerLevel']
    constants = {'Bal.Mult': v.chain_value(package, 'Bal.Mult')}
    assert v.formula_at(package, 'Bal.HealthInit', ['Bal.PlayerLevel'], 1, constants) == 25  # min restriction
    assert v.formula_at(package, 'Bal.HealthInit', ['Bal.PlayerLevel'], 3, constants) == 80


def test_xp_amount_is_truncated_fraction_of_level_gap():
    required = lambda level: 333 * level * level
    # 0.1 x (2997 - 1332) = 166.5: truncated to 166 (rounding would give 167).
    assert v.xp_candidate(0.1, required, 2) == 166
    assert v.mission_xp(0.1, 999) == 99
    assert v.mission_xp(0.5, 7, multiplier=2.0) == 7


def test_required_points_truncate_each_point_on_the_native_order():
    # Invented constants. 1.5 x (n^2 + 0.4): f(1) = 2.1 -> 2, f(2) = 6.6 -> 6, so R(2) = 4; the offset outside the
    # multiplier would give 1.9 -> 1 and 6.4 -> 6, i.e. 5.
    assert v.required_points(1.5, 2, 0.4, 1) == 0
    assert v.required_points(1.5, 2, 0.4, 2) == 4
    # 1.5 x (n^2 + 0.2): 1.8 -> 1, 6.3 -> 6, R(2) = 5, where flooring the unrounded difference (4.5) gives 4.
    assert v.required_points(1.5, 2, 0.2, 2) == 5
    # Single precision: 0.1 is not exact in float, 10 x (n + 0.1) at n = 1 is 11.000000149 -> 11.
    assert v.required_points(10, 1, 0.1, 3) == 20
    assert v.f32(0.1) != 0.1


def region_package():
    def bound(value):
        return {'BaseValueConstant': value, 'BaseValueAttribute': None, 'InitializationDefinition': None,
                'BaseValueScaleConstant': 1}
    objects = {
        v.GLOBALS: {'GameStageIncreaseAbovePlayer': bound(0), 'RegionBalanceData': [
            {'PlayThroughNumber': 1, 'BalanceDefinitions': ['Bal.P1_A', 'Bal.P1_B']},
            {'PlayThroughNumber': 2, 'BalanceDefinitions': ['Bal.P2']}]},
        'Bal.P1_A': {'BalanceByRegion': [{'Region': 'Region.Other', 'MinDefaultGameStage': bound(1),
                                          'MaxDefaultGameStage': bound(2)}]},
        'Bal.P1_B': {'BalanceByRegion': [{'Region': 'Region.Town', 'MinDefaultGameStage': bound(3),
                                          'MaxDefaultGameStage': bound(5), 'bSpecifyBoostAbovePlayer': True,
                                          'GameStageIncreaseAbovePlayer': bound(1),
                                          'MissionOverrides': [{'Mission': 'M.First', 'MinGameStage': bound(4),
                                                                'MaxGameStage': bound(6)}]}]},
        'Bal.P2': {'BalanceByRegion': [{'Region': 'Region.Town', 'MinDefaultGameStage': bound(30),
                                        'MaxDefaultGameStage': bound(32)}]},
    }
    return FakePackage(objects, {})


def test_region_stage_entry_for_playthrough_one():
    stage = v.region_stage(region_package(), 'Region.Town', {})
    assert stage['balance_definition'] == 'Bal.P1_B'
    assert stage['default'] == {'min': 3, 'max': 5}
    assert stage['boost_above_player'] == 1          # the entry's own boost, as it is flagged
    assert stage['overrides'] == [{'mission': 'M.First', 'min': 4, 'max': 6}]
    assert v.region_stage(region_package(), 'Region.Town', {}, playthrough=2)['default'] == {'min': 30, 'max': 32}
    try:
        v.region_stage(region_package(), 'Region.Missing', {})
        raise AssertionError('missing region resolved')
    except v.Unresolved:
        pass


if __name__ == '__main__':
    tests = [(name, fn) for name, fn in sorted(globals().items()) if name.startswith('test_')]
    for name, fn in tests:
        fn()
        print('ok', name)
    print(f'{len(tests)} tests passed')
