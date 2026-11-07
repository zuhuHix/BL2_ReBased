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

    def test_same_tag_in_two_groups_fails_loudly(self):
        ue3 = FakeUe3()
        ue3.objects[OTHER]['dialogevents'].append({'Tag': 'Ev.Line03', 'OutputAction': OTHER + '.Talk_9'})
        # Ev.Line03 is now in both groups: refused, because manifest keys carry no group
        with self.assertRaisesRegex(ValueError, 'Ev.Line03 in Pkg.Groups.MissionGroup, Pkg.Groups.OtherGroup'):
            chain.dialog_chain(ue3, [{'a': 'Ev.Line03', 'b': OTHER, 'c': None}])

    def test_reached_flag_follows_the_group_of_the_effect(self):
        rows = self.rows([{'a': 'Ev.Other', 'b': OTHER, 'c': None}])
        self.assertTrue(rows[(OTHER, 'Ev.Other')]['reached_in_scripted_run'])
        self.assertFalse(rows[(GROUP, 'Ev.Line01')]['reached_in_scripted_run'])

    def test_manifest_has_a_dialog_entry_per_resolved_line(self):
        rows = chain.dialog_chain(FakeUe3(), [])
        manifest = chain.build_manifest({'dialog': rows, 'marcus_group': [], 'door': [], 'weapon': []}, 'no-such-dir')
        keys = [e['key'] for e in manifest['entries']]
        self.assertEqual(keys, ['dialog:Ev.Line01', 'dialog:Ev.Line02_Alt', 'dialog:Ev.Line03', 'dialog:Ev.Line04_Alt'])
        self.assertEqual({e['reached_in_scripted_run'] for e in manifest['entries']}, {False})


class DurationCheckTest(unittest.TestCase):
    def test_true_only_when_every_duration_is_inside(self):
        self.assertIs(chain.duration_check([{'duration_s': 1.0}, {'duration_s': 2.0}], 1.0, 2.0), True)

    def test_false_only_when_a_comparison_failed(self):
        self.assertIs(chain.duration_check([{'duration_s': 1.0}, {'duration_s': 9.0}], 1.0, 2.0), False)
        self.assertIs(chain.duration_check([{'duration_s': None}, {'duration_s': 9.0}], 1.0, 2.0), False)

    def test_none_when_no_full_comparison_ran(self):
        self.assertIsNone(chain.duration_check([], 1.0, 2.0))
        self.assertIsNone(chain.duration_check([{'duration_s': 1.0}], None, None))
        self.assertIsNone(chain.duration_check([{'duration_s': None}, {'duration_s': 1.5}], 1.0, 2.0))

    def test_a_live_role_writes_null_not_false(self):
        sound = {'source': 1, 'wem': 'x', 'bytes': 1, 'codec': 'c', 'channels': 1, 'sample_rate': 48000, 'duration_s': 1.0}
        ak = {**chain.live_role_ak({'event': 'Pkg.Ak_Play_Thing'}), 'wwise': [{'bank_names': ['B'], 'sounds': [sound]}]}
        manifest = chain.build_manifest({'dialog': [], 'marcus_group': [], 'door': [], 'weapon': [],
                                         'live_roles': [{'key': 'role:hit', 'seen': 'once', 'ak_event': ak}]}, 'no-such-dir')
        entry = manifest['entries'][0]
        self.assertEqual(entry['state'], 'extracted_undecoded')
        self.assertIsNone(entry['checks']['wem_header_duration_within_akevent_range'])


class LiveRoleTest(unittest.TestCase):
    def test_each_role_gets_an_entry_marked_unverified_with_the_check_not_claimed(self):
        ak = {**chain.live_role_ak({'event': 'Pkg.Ak_Play_Thing'}), 'wwise': []}
        manifest = chain.build_manifest({'dialog': [], 'marcus_group': [], 'door': [], 'weapon': [],
                                         'live_roles': [{'key': 'role:hit', 'seen': 'once', 'ak_event': ak}]}, 'no-such-dir')
        entry = manifest['entries'][0]
        self.assertEqual((entry['key'], entry['kind'], entry['state']), ('role:hit', 'live_role', 'unresolved'))
        self.assertIn('UNVERIFIED', entry['evidence'])
        self.assertIsNone(entry['checks']['fnv1_matches_short_id'])     # not read from a package, so not checked and not claimed

    def test_the_wwise_name_and_id_come_from_the_event_name(self):
        ak = chain.live_role_ak({'event': 'Pkg.Group.Ak_Play_Thing'})
        self.assertEqual(ak['wwise_name'], 'Play_Thing')
        self.assertEqual(ak['short_id'], chain.ac.fnv1_32('Play_Thing'))

    def test_a_name_that_breaks_the_rule_fails_loudly(self):
        with self.assertRaises(ValueError):
            chain.live_role_ak({'event': 'Pkg.Play_Thing'})

    def test_every_role_is_listed_with_what_was_seen_and_no_pickup_guess(self):
        keys = [row['key'] for row in chain.live_role_chain()]
        self.assertEqual(keys, ['role:phaselock_cast', 'role:hit'])
        self.assertTrue(all(row['seen'] for row in chain.live_role_chain()))


if __name__ == '__main__':
    unittest.main()
