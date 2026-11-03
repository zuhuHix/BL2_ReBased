"""Synthetic checks for tools/audio_slice_chain.py dialog coverage. No game data: every object is made up here.

The slice host reaches different mission dialog lines from run to run, so the manifest must hold every
DialogEvents entry of the mission's dialog group(s), not only the ones one scripted run reaches.

Run: python tests/audio_slice_chain_test.py
"""
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'tools'))
import audio_slice_chain as chain  # noqa: E402

GROUP = 'Pkg.Groups.MissionGroup'
OTHER = 'Pkg.Groups.OtherGroup'


def talk(name):
    return [{'NameTag': 'Pkg.Names.' + name, 'TalkAkEvent': 'Ak.Play_' + name}]


class FakeUe3:
    def __init__(self):
        self.objects = {
            chain.MISSION: {'missiondialoggroup': GROUP},
            GROUP: {'dialogevents': [
                {'Tag': 'Ev.Line01', 'OutputAction': GROUP + '.Talk_1'},
                {'Tag': 'Ev.Line02_Alt', 'OutputAction': None},
                {'Tag': 'Ev.Line03', 'OutputAction': GROUP + '.Talk_3'},
                {'Tag': 'Ev.Line04_Alt', 'OutputAction': None},
                {'Tag': 'Ev.Line05_Orphan', 'OutputAction': None},
            ], 'talkacts': [{'TalkData': talk('Line02')}, {'TalkData': talk('Line04')}]},
            GROUP + '.Talk_1': {'talkdata': talk('Line01')},
            GROUP + '.Talk_3': {'talkdata': talk('Line03')},
            OTHER: {'dialogevents': [{'Tag': 'Ev.Other', 'OutputAction': OTHER + '.Talk_9'}], 'talkacts': []},
            OTHER + '.Talk_9': {'talkdata': talk('Other')},
        }

    def dump(self, pkg, path):
        return self.objects[path]

    def ak_event(self, pkg, path):
        return {'ue3_path': path, 'wwise_name': path.split('.')[-1], 'short_id': hash(path) & 0xffffffff,
                'fnv1_matches_short_id': True, 'min_duration': 1.0, 'max_duration': 2.0}


class DialogCoverageTest(unittest.TestCase):
    def rows(self, effects):
        return {(r['trigger']['group'], r['trigger']['event_tag']): r for r in chain.dialog_chain(FakeUe3(), effects)}

    def test_every_group_event_is_resolved_not_only_the_reached_ones(self):
        rows = self.rows([{'a': 'Ev.Line01', 'b': GROUP, 'c': 'Pkg.Names.Marcus'}])
        self.assertEqual([t for g, t in rows if g == GROUP],
                         ['Ev.Line01', 'Ev.Line02_Alt', 'Ev.Line03', 'Ev.Line04_Alt', 'Ev.Line05_Orphan'])
        self.assertTrue(rows[(GROUP, 'Ev.Line01')]['reached_in_scripted_run'])
        self.assertFalse(rows[(GROUP, 'Ev.Line03')]['reached_in_scripted_run'])
        self.assertEqual(rows[(GROUP, 'Ev.Line01')]['trigger']['name_tag'], 'Pkg.Names.Marcus')
        self.assertEqual(rows[(GROUP, 'Ev.Line03')]['ak_events'][0]['ue3_path'], 'Ak.Play_Line03')

    def test_null_output_action_pairs_with_kth_inline_talk_act(self):
        rows = self.rows([])
        self.assertEqual(rows[(GROUP, 'Ev.Line02_Alt')]['ak_events'][0]['ue3_path'], 'Ak.Play_Line02')
        self.assertEqual(rows[(GROUP, 'Ev.Line04_Alt')]['ak_events'][0]['ue3_path'], 'Ak.Play_Line04')
        # a third null-OutputAction event has no inline act: kept, with no AkEvent, and says why
        orphan = rows[(GROUP, 'Ev.Line05_Orphan')]
        self.assertEqual(orphan['ak_events'], [])
        self.assertIn('no k-th inline', orphan['how'])

    def test_groups_named_by_the_run_are_added_once(self):
        rows = self.rows([{'a': 'Ev.Other', 'b': OTHER, 'c': None}, {'a': 'Ev.Line01', 'b': GROUP, 'c': None}])
        self.assertIn((OTHER, 'Ev.Other'), rows)
        self.assertEqual(sum(1 for g, _ in rows if g == GROUP), 5)

    def test_reached_tag_missing_from_its_group_stays_visible(self):
        rows = self.rows([{'a': 'Ev.Gone', 'b': GROUP, 'c': None}])
        self.assertEqual(rows[(GROUP, 'Ev.Gone')]['how'], 'event tag not in group')

    def test_manifest_has_a_dialog_entry_per_resolved_line(self):
        rows = chain.dialog_chain(FakeUe3(), [])
        manifest = chain.build_manifest({'dialog': rows, 'marcus_group': [], 'door': [], 'weapon': []}, 'no-such-dir')
        keys = [e['key'] for e in manifest['entries']]
        self.assertEqual(keys, ['dialog:Ev.Line01', 'dialog:Ev.Line02_Alt', 'dialog:Ev.Line03', 'dialog:Ev.Line04_Alt'])
        self.assertEqual({e['reached_in_scripted_run'] for e in manifest['entries']}, {False})


if __name__ == '__main__':
    unittest.main()
