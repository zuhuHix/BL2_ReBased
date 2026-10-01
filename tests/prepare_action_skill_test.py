"""Synthetic tests for tools/prepare_action_skill.py (no game data).

A made-up behavior provider stands in for the tagged-property reader: two
events, four behaviors and packed links (ArrayIndexAndLength = index << 16 |
length; a link is behavior | id << 24). Names and numbers are invented.
Run: python tests/prepare_action_skill_test.py
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import prepare_action_skill as p  # noqa: E402


def packed(start, length):
    return {'ArrayIndexAndLength': start << 16 | length}


def link(behavior, link_id, delay=0.0):
    raw = behavior | link_id << 24
    # The reader returns the int32 as stored, so a high id byte is negative.
    return {'LinkIdAndLinkedBehavior': raw - (1 << 32) if raw >= 1 << 31 else raw, 'ActivateDelay': delay}


class FakePackage:
    classes = {'Pkg.Provider.B_Wait': 'Fake.Behavior_Delay', 'Pkg.Provider.B_Hit': 'Fake.Behavior_CauseDamage',
               'Pkg.Provider.B_Flag': 'Fake.Behavior_SetFlag', 'Pkg.Provider.B_Orphan': 'Fake.Behavior_Explode'}


def provider(links, behavior_links):
    behaviors = ['Pkg.Provider.B_Wait', 'Pkg.Provider.B_Hit', 'Pkg.Provider.B_Flag', 'Pkg.Provider.B_Orphan']
    return {'BehaviorSequences': [{
        'BehaviorSequenceName': 'Main',
        'bEnabledOnSpawn': True,
        'EventData2': [
            {'UserData': {'EventName': 'OnStart', 'bEnabled': True},
             'OutputVariables': packed(0, 1), 'OutputLinks': packed(0, 1)},
            {'UserData': {'EventName': 'OnStop', 'bEnabled': True},
             'OutputVariables': packed(1, 0), 'OutputLinks': packed(1, 1)},
        ],
        'BehaviorData2': [{'Behavior': b, 'LinkedVariables': packed(1, 1) if i == 1 else packed(0, 0),
                           'OutputLinks': behavior_links[i]} for i, b in enumerate(behaviors)],
        'ConsolidatedOutputLinkData': links,
        'VariableData': [{'Name': 'Caster', 'Type': 'BVAR_Object'}, {'Name': 'Victim', 'Type': 'BVAR_Object'}],
        'ConsolidatedVariableLinkData': [
            {'PropertyName': 'Instigator', 'LinkedVariables': packed(0, 1)},
            {'PropertyName': 'TargetContext', 'LinkedVariables': packed(1, 1)},
        ],
        'ConsolidatedLinkedVariables': [0, 1],
    }]}


def install(data):
    def tagged(package, path):
        if path == 'Pkg.Provider':
            return data, []
        return {'Name': path.rsplit('.', 1)[-1]}, []
    p.tagged = tagged


def test_unpack():
    assert p.unpack(packed(5439, 1)) == (5439, 1)
    assert p.unpack(None) == (0, 0)


def test_read_provider():
    # OnStart -> B_Wait -(1.5 s, id 2)-> B_Hit; OnStop -> B_Flag. B_Orphan is never linked.
    links = [link(0, 0), link(2, 0), link(1, 2, 1.5)]
    install(provider(links, [packed(2, 1), packed(3, 0), packed(3, 0), packed(3, 0)]))
    result = p.read_provider(FakePackage(), 'Pkg.Provider')
    sequence = result['sequences'][0]
    assert [e['name'] for e in sequence['events']] == ['OnStart', 'OnStop']
    assert sequence['events'][0]['links'] == [{'behavior': 0, 'outputId': 0, 'delay': 0.0}]
    assert sequence['events'][0]['outputs'] == {'Instigator': [{'variable': 0, 'name': 'Caster', 'type': 'BVAR_Object'}]}
    wait = sequence['behaviors'][0]
    assert wait['class'] == 'Fake.Behavior_Delay' and wait['links'] == [{'behavior': 1, 'outputId': 2, 'delay': 1.5}]
    assert sequence['behaviors'][1]['variables'] == {'TargetContext': [{'variable': 1, 'name': 'Victim', 'type': 'BVAR_Object'}]}
    assert sequence['check'] == {'links': 3, 'linksTiledExactly': True, 'unreachedBehaviors': ['B_Orphan']}


def test_read_provider_rejects_bad_links():
    install(provider([link(9, 0)], [packed(0, 0)] * 4))
    try:
        p.read_provider(FakePackage(), 'Pkg.Provider')
    except ValueError as error:
        assert 'link to behavior 9' in str(error)
    else:
        raise AssertionError('a link past the behavior list must be rejected')
    install(provider([link(0, 0)], [packed(0, 5)] + [packed(0, 0)] * 3))
    try:
        p.read_provider(FakePackage(), 'Pkg.Provider')
    except ValueError as error:
        assert 'outside' in str(error)
    else:
        raise AssertionError('a link range past the link array must be rejected')


def test_untiled_links_are_reported():
    # Link 3 is referenced by nobody: the ranges do not cover the array.
    install(provider([link(0, 0), link(2, 0), link(1, 0), link(3, 0)], [packed(2, 1)] + [packed(0, 0)] * 3))
    sequence = p.read_provider(FakePackage(), 'Pkg.Provider')['sequences'][0]
    assert sequence['check']['linksTiledExactly'] is False


def test_combine():
    rows = [{'modifierType': 'MT_PostAdd', 'values': [None, 2.0, 4.0]},
            {'modifierType': 'MT_Scale', 'values': [0.5, 0.5, 0.5]},
            {'modifierType': 'MT_PreAdd', 'values': [1.0, 1.0, 1.0]}]
    assert p.combine(3.0, rows, lambda r: 0) == (3.0 + 1.0) * 1.5
    assert p.combine(3.0, rows, lambda r: 2) == (3.0 + 1.0) * 1.5 + 4.0
    assert p.combine(3.0, [], lambda r: 0) == 3.0


def test_timeline():
    t = p.timeline(lift=0.5, lock_seconds=4.0, time_scale=0.5, fade=1.0, release_buffer=2.0)
    assert t == {'skillDuration': 2.5, 'lockedAt': 0.5, 'lockedFor': 1.0, 'outroAt': 1.5,
                 'releasedAt': 2.5, 'endSkillAt': 4.5}


if __name__ == '__main__':
    tests = [(name, fn) for name, fn in sorted(globals().items()) if name.startswith('test_')]
    for name, fn in tests:
        fn()
        print('ok', name)
    print(f'{len(tests)} tests passed')
