"""The arithmetic of tools/real_game/scripts/den_probe.py, with made-up rows and no game."""
import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location('den_probe', ROOT / 'tools/real_game/scripts/den_probe.py')
den_probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(den_probe)


def sample(t, name, to_player, speed):
    return {'t': t, 'name': name, 'to_player': to_player, 'speed': speed, 'target': True}


CASH_SPEC = importlib.util.spec_from_file_location('cash_probe', ROOT / 'tools/real_game/scripts/cash_probe.py')
cash_probe = importlib.util.module_from_spec(CASH_SPEC)
CASH_SPEC.loader.exec_module(cash_probe)


class BoundFunction:
    """A hook's func argument as the SDK gives it: a bound function, whose own .Name raises and whose function is at .func (the audio lane's lost events)."""
    def __init__(self, *names):
        self.func = type('Func', (), {'_properties': lambda _self: [type('Prop', (), {'Name': n}) for n in names], 'Name': 'MeleeAttack'})()

    @property
    def Name(self):
        raise AttributeError('Name')


class HookShapeTest(unittest.TestCase):
    def test_the_attacker_comes_through_a_bound_function(self):
        pawn = type('Pawn', (), {'Location': object(), 'Name': 'WillowAIPawn_1'})()
        args = type('Args', (), {'ContextObject': pawn})()
        self.assertIs(den_probe._attacker_of_call(args, BoundFunction('ContextObject')), pawn)
        self.assertIsNone(den_probe._attacker_of_call(type('Args', (), {'Other': 3})(), BoundFunction('Other')))

    def test_an_error_in_a_hook_is_counted_and_spoils_the_trial(self):
        den_probe.DP['hook_errors'] = []

        def broken(obj, args, ret, func):
            return func.Name
        self.assertIsNone(den_probe._guarded(broken)(None, None, None, BoundFunction()))
        self.assertEqual(len(den_probe.DP['hook_errors']), 1)
        summary = den_probe.summarise({'samples': [sample(1.0, 'A', 900.0, 500.0)], 'calls': [{'t': 1.0, 'attacker': 'A'}], 'blows': [], 'hp_start': 9, 'hp_end': 9, 'hook_errors': 1})
        self.assertFalse(den_probe.trial_counts(summary))
        summary = den_probe.summarise({'samples': [sample(1.0, 'A', 900.0, 500.0)], 'calls': [{'t': 1.0, 'attacker': 'A'}], 'blows': [], 'hp_start': 9, 'hp_end': 9})
        self.assertTrue(den_probe.trial_counts(summary))
        summary = den_probe.summarise({'samples': [sample(1.0, 'A', 900.0, 500.0)], 'calls': [{'t': 1.0, 'attacker': 'A'}], 'blows': [], 'hp_start': 9, 'hp_end': 3})
        self.assertFalse(den_probe.trial_counts(summary))   # Maya lost health: the immunity did not hold

    def test_the_stage_names_what_it_never_saw(self):
        whole = {'counts': True, 'first_blow_spread': 0.2, 'chase_speed_median': {'A': 500.0}, 'swing_starts': [{'attacker': 'A'}]}
        self.assertEqual(den_probe.den_answers([whole]), [])
        for key, empty, words in (('first_blow_spread', None, 'first-blow'), ('chase_speed_median', {}, 'chase'), ('swing_starts', [], 'swing')):
            missing = den_probe.den_answers([dict(whole, **{key: empty})])
            self.assertEqual(len(missing), 1, key)
            self.assertIn(words, missing[0])
        self.assertEqual(len(den_probe.den_answers([dict(whole, counts=False)])), 3)   # only trials that count
        self.assertEqual(len(den_probe.den_answers([])), 3)


class ImmunityPathsTest(unittest.TestCase):
    def test_every_declaration_down_from_the_player_is_hooked(self):
        declared = {'WillowPlayerPawn': ['W.WillowPlayerPawn:TakeDamage'], 'WillowPawn': ['W.WillowPawn:TakeDamage'], 'Pawn': ['E.Pawn:TakeDamage']}
        self.assertEqual(cash_probe.immune_paths(lambda cls, name: declared[cls]),
                         ['W.WillowPlayerPawn:TakeDamage', 'W.WillowPawn:TakeDamage', 'E.Pawn:TakeDamage'])

    def test_no_player_side_declaration_is_no_immunity(self):
        declared = {'WillowPlayerPawn': [], 'WillowPawn': [], 'Pawn': ['E.Pawn:TakeDamage']}
        self.assertEqual(cash_probe.immune_paths(lambda cls, name: declared[cls]), [])


class CashReadingTest(unittest.TestCase):
    def test_a_reading_needs_the_target_alone_gone_and_cash_paid(self):
        self.assertEqual(cash_probe.value_per_item(3, ['Target'], 'Target'), 3)
        self.assertIsNone(cash_probe.value_per_item(3, ['Neighbour'], 'Target'))   # one pickup went, but not the one stood on
        self.assertIsNone(cash_probe.value_per_item(6, ['Target', 'Neighbour'], 'Target'))
        self.assertIsNone(cash_probe.value_per_item(0, ['Target'], 'Target'))
        self.assertIsNone(cash_probe.value_per_item(3, [], 'Target'))
        self.assertIsNone(cash_probe.value_per_item(6, ['Target'], 'Target', 2))   # two touches in the walk (review of #251, A7)
        self.assertEqual(cash_probe.value_per_item(3, ['Target'], 'Target', 1), 3)


class SummariseTest(unittest.TestCase):
    def trial(self):
        return {'kinds': {'A': 'made', 'B': 'made'}, 'hp_start': 9, 'hp_end': 9,
                'samples': [sample(0.0, 'A', 1600.0, 0.0), sample(1.0, 'A', 1000.0, 500.0), sample(2.0, 'A', 500.0, 450.0), sample(2.6, 'A', 300.0, 300.0),
                            sample(1.0, 'B', 1500.0, 100.0), sample(3.0, 'B', 350.0, 620.0)],
                'calls': [{'t': 3.0, 'attacker': 'A', 'definition': 'Made_Melee', 'to_player': 280.0},
                          {'t': 3.4, 'attacker': 'B', 'definition': 'Made_Melee', 'to_player': 300.0},
                          {'t': 3.6, 'attacker': 'A', 'definition': 'Made_Melee', 'to_player': 280.0}],
                'blows': [{'t': 3.0, 'attacker': 'A', 'damage': 9}, {'t': 3.4, 'attacker': 'B', 'damage': 9}, {'t': 4.0, 'attacker': 'A', 'damage': 9}]}

    def test_first_blows_and_their_spread(self):
        s = den_probe.summarise(self.trial())
        self.assertEqual(s['first_blow'], {'A': 3.0, 'B': 3.4})
        self.assertEqual(s['first_blow_spread'], 0.4)
        self.assertEqual(s['attackers'], 2)

    def test_chase_speed_leaves_out_the_walk_and_standing(self):
        s = den_probe.summarise(self.trial())
        self.assertEqual(s['chase_speed_median'], {'A': 450.0, 'B': 620.0})

    def test_swing_start_is_the_sample_before_the_call(self):
        s = den_probe.summarise(self.trial())
        self.assertEqual(s['swing_starts'][0]['distance_at_start'], 300)   # A at 2.6, the sample nearest 3.0 - 0.41
        self.assertEqual(s['swing_starts'][0]['distance_at_call'], 280.0)

    def test_no_sample_near_the_start_gives_no_answer(self):
        trial = self.trial()
        trial['samples'] = [sample(0.0, 'A', 1600.0, 500.0)]
        trial['calls'] = [{'t': 3.0, 'attacker': 'A', 'definition': 'Made_Melee'}]
        self.assertEqual(den_probe.summarise(trial)['swing_starts'], [])

    def test_one_attacker_has_no_spread(self):
        trial = self.trial()
        trial['blows'] = [{'t': 3.0, 'attacker': 'A', 'damage': 9}, {'t': 3.4, 'attacker': None, 'damage': 9}]
        s = den_probe.summarise(trial)
        self.assertIsNone(s['first_blow_spread'])
        self.assertEqual(s['attackers'], 1)

    def test_a_quiet_trial_does_not_count(self):
        self.assertFalse(den_probe.trial_counts(den_probe.summarise({'samples': [sample(0.0, 'A', 1600.0, 0.0)], 'calls': [], 'blows': [], 'hp_start': 9, 'hp_end': 9})))
        self.assertTrue(den_probe.trial_counts(den_probe.summarise(self.trial())))


if __name__ == '__main__':
    unittest.main()
