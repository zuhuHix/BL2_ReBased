"""Synthetic tests for tools/prepare_stock_shield.py: invented packages and values, no game data, no ow-package needed."""
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import prepare_stock_shield as ps  # noqa: E402
import prepare_vendors as pv  # noqa: E402
import weapon_recipe  # noqa: E402

NONE = {'index': 0, 'path': None}
CAP, RATE, DELAY = (ps.POOL + 'ShieldMaxValue', ps.POOL + 'ShieldOnIdleRegenerationRate',
                    ps.POOL + 'ShieldOnIdleRegenerationDelay')


def ref(path, index=1):
    return {'index': index, 'path': path}


def const(value):
    return {'BaseValueConstant': value, 'BaseValueAttribute': NONE, 'InitializationDefinition': NONE, 'BaseValueScaleConstant': 1}


def attr_init(path):
    return {'BaseValueConstant': 1, 'BaseValueAttribute': ref(path), 'InitializationDefinition': NONE, 'BaseValueScaleConstant': 1}


def def_init(path):
    return {'BaseValueConstant': 0, 'BaseValueAttribute': NONE, 'InitializationDefinition': ref(path), 'BaseValueScaleConstant': 1}


class FakeReader:
    """Stands in for prepare_slice_world.Reader: {path: (class, props)} for the one package 'Startup'.

    A path in `failing` raises the ValueError the real reader raises for an export that does not decode."""

    def __init__(self, objects, failing=()):
        self.objects, self.failing = objects, set(failing)

    def exports(self, pkg):
        by_path = {p: {'index': i, 'path': p, 'class': c} for i, (p, (c, _)) in enumerate(self.objects.items(), start=1)}
        return {'by_path': by_path, 'by_index': {x['index']: x for x in by_path.values()}}

    def props(self, pkg, path):
        if path in self.failing:
            raise ValueError(f'{path}: invented decode failure')
        return self.objects[path][1]


def effect(slot, attribute, per_grade, external=True, skill=False, base=0.0):
    return {'SlotName': slot, 'AttributeToModify': ref(attribute), 'ModifierType': 'MT_Scale', 'BaseModifierValue': const(base),
            'PerGradeUpgrade': const(per_grade), 'bExternalSlot': external, 'bRunEffectsAsSkill': skill}


def upgrade(slot, increase, activate=False):
    return {'SlotName': slot, 'GradeIncrease': increase, 'bActivateSlot': activate}


def weighted(part, weight_index=1, makers=()):
    return {'Part': ref(part), 'Manufacturers': [{'Manufacturer': ref(m), 'DefaultWeightIndex': i} for m, i in makers],
            'MinGameStageIndex': 0, 'MaxGameStageIndex': 2, 'DefaultWeightIndex': weight_index}


def objects(rate=5.0):
    """An invented shield: capacity base 10 x 2^level, delay base 3 (a constant attribute), rate base `rate`.

    Slots: Capacity +0.1 per grade, RechargeDelay +0.1 per grade, an internal slot and a skill-run slot on the capacity that
    must not reach the pool. Two bodies (Up: capacity +2, delay -2; Down: capacity -1, delay +1), two batteries from the
    balance's collection (no upgrades), one material: 4 combinations.
    """
    o = {}
    o['Attr.Level'] = ('Engine.AttributeDefinition', {'ValueResolverChain': [ref('Attr.Level.Prop_0')]})
    o['Attr.Level.Prop_0'] = ('Engine.ObjectPropertyAttributeValueResolver', {'PropertyName': 'ExpLevel'})
    for path, value in [('Attr.Ten', 10.0), ('Attr.Two', 2.0), ('Attr.Delay', 3.0)]:
        o[path] = ('Engine.AttributeDefinition', {})
        o[path + '.ConstantAttributeValueResolver_0'] = ('Engine.ConstantAttributeValueResolver', {'ConstantValue': value})
    o['Init.Cap'] = ('Engine.AttributeInitializationDefinition', {'ValueFormula': {
        'bEnabled': True, 'Multiplier': attr_init('Attr.Ten'), 'Level': attr_init('Attr.Two'), 'Power': attr_init('Attr.Level')}})
    o['Parts.Up'] = ('WillowGame.ShieldPartDefinition', {'AttributeSlotUpgrades': [upgrade('Capacity', 2), upgrade('RechargeDelay', -2)]})
    o['Parts.Down'] = ('WillowGame.ShieldPartDefinition', {'AttributeSlotUpgrades': [upgrade('Capacity', -1), upgrade('RechargeDelay', 1)]})
    o['Parts.Mat'] = ('WillowGame.ShieldPartDefinition', {})
    o['Parts.Late'] = ('WillowGame.ShieldPartDefinition', {'AttributeSlotUpgrades': [upgrade('Capacity', 50)]})
    o['Parts.BatA'] = ('WillowGame.ShieldPartDefinition', {})
    o['Parts.BatB'] = ('WillowGame.ShieldPartDefinition', {})
    o['Lists.Body'] = ('WillowGame.ItemPartListDefinition', {
        'WeightedParts': [weighted('Parts.Up'), weighted('Parts.Down', makers=[('Makers.A', 3)]),
                          {**weighted('Parts.Late'), 'MinGameStageIndex': 4}],
        'ConsolidatedAttributeInitData': [const(1), const(100), const(10), const(25), const(9)]})
    o['Lists.Mat'] = ('WillowGame.ItemPartListDefinition', {
        'WeightedParts': [weighted('Parts.Mat')], 'ConsolidatedAttributeInitData': [const(1), const(100), const(10)]})
    o['Lists.Coll'] = ('WillowGame.ItemPartListCollectionDefinition', {
        'BetaPartData': {'bEnabled': True, 'WeightedParts': [weighted('Parts.BatA'), weighted('Parts.BatB'),
                                                              {**weighted('Parts.Late'), 'MinGameStageIndex': 3}]},
        'ConsolidatedAttributeInitData': [const(1), const(100), const(10), const(9)], 'PartReplacementMode': 'EPRM_Additive'})
    o['Names.Title'] = ('WillowGame.ItemNamePartDefinition', {'PartName': 'Test Shield'})
    o['Items.Def'] = ('WillowGame.ShieldDefinition', {
        'ItemName': 'Fallback Name', 'AlphaParts': ref('Lists.Body'), 'MaterialParts': ref('Lists.Mat'),
        'TitleList': [ref('Names.Title')],
        'ExternalAttributeEffects': [
            {'AttributeToModify': ref(CAP), 'ModifierType': 'MT_PreAdd', 'BaseModifierValue': def_init('Init.Cap')},
            {'AttributeToModify': ref(DELAY), 'ModifierType': 'MT_PreAdd', 'BaseModifierValue': attr_init('Attr.Delay')},
            {'AttributeToModify': ref(RATE), 'ModifierType': 'MT_PreAdd', 'BaseModifierValue': const(rate)}],
        'AttributeSlotEffects': [effect('Capacity', CAP, 0.1), effect('RechargeDelay', DELAY, 0.1),
                                 effect('Inside', CAP, 1.0, external=False, base=1.0),
                                 effect('AsSkill', CAP, 1.0, skill=True, base=1.0)],
        'AttributeSlotUpgrades': [upgrade('Capacity', 0, True), upgrade('RechargeDelay', 0, True), upgrade('Inside', 0, True),
                                  upgrade('AsSkill', 0, True)]})
    o['Grades.Shield'] = ('WillowGame.InventoryBalanceDefinition', {
        'InventoryDefinition': ref('Items.Def'), 'Manufacturers': [{'Manufacturer': ref('Makers.A')}],
        'PartListCollection': ref('Lists.Coll')})
    o['Makers.A'] = ('WillowGame.ManufacturerDefinition', {})
    o['Grades.Gun'] = ('WillowGame.InventoryBalanceDefinition', {'InventoryDefinition': ref('Items.Gun')})
    o['Items.Gun'] = ('WillowGame.WeaponTypeDefinition', {})
    return o


def chain(objs=None, failing=()):
    return pv.Chain(FakeReader(objects() if objs is None else objs, failing), ['Startup'])


def with_props(objs, path, **changes):
    cls, props = objs[path]
    objs[path] = (cls, {**props, **changes})
    return objs


class StockShieldTest(unittest.TestCase):
    def values(self, body, level=2, objs=None):
        c = chain(objs)
        parts = {'Alpha': body, 'Beta': 'Parts.BatA', 'Material': 'Parts.Mat'}
        names = ps.name_parts(c, 'Items.Def', parts, 'Makers.A', level)
        return ps.stats(c, 'Items.Def', parts, names, level)

    def test_base_formula_and_slots(self):
        # Level 2: capacity base 10 x 2^2 = 40. Up: capacity grade 1 + 2 = 3 -> +0.2 scale; delay grade -1 -> -0.2 (divides).
        values, reached = self.values('Parts.Up')
        self.assertAlmostEqual(values['Capacity'], 48.0, places=4)
        self.assertAlmostEqual(values['Recharge Delay'], 3.0 / 1.2, places=5)
        self.assertAlmostEqual(values['Recharge Rate'], 5.0, places=5)
        self.assertEqual({r['slot']: r['grade'] for r in reached}, {'Capacity': 3, 'RechargeDelay': -1})

    def test_internal_and_skill_slots_do_not_reach_the_pool(self):
        values, reached = self.values('Parts.Down')
        self.assertAlmostEqual(values['Capacity'], 40.0 / 1.1, places=4)  # grade 0 -> -0.1; Inside and AsSkill would add +1
        self.assertAlmostEqual(values['Recharge Delay'], 3.3, places=5)
        self.assertNotIn('Inside', [r['slot'] for r in reached])
        self.assertNotIn('AsSkill', [r['slot'] for r in reached])

    def test_every_effect_of_an_active_slot_applies(self):
        # Two effects named Capacity: the second scales the rate by +0.2 at grade 3, as weapon_stats applies every entry.
        objs = objects()
        cls, props = objs['Items.Def']
        props['AttributeSlotEffects'] = props['AttributeSlotEffects'] + [effect('Capacity', RATE, 0.1)]
        values, reached = self.values('Parts.Up', objs=objs)
        self.assertAlmostEqual(values['Capacity'], 48.0, places=4)
        self.assertAlmostEqual(values['Recharge Rate'], 6.0, places=5)
        self.assertEqual([r['slot'] for r in reached].count('Capacity'), 2)

    def test_name_part_upgrades_count(self):
        # The title name part adds +1 to Capacity (InitializeAttributeSlotsForNameParts): Up's grade 3 becomes 4 -> +0.3.
        objs = with_props(objects(), 'Names.Title', AttributeSlotUpgrades=[upgrade('Capacity', 1)])
        values, _ = self.values('Parts.Up', objs=objs)
        self.assertAlmostEqual(values['Capacity'], 52.0, places=4)

    def test_activation_limit(self):
        # AttributeSlotMaxActivated 1: only the first activated slot (Capacity) applies; the delay keeps its base.
        objs = with_props(objects(), 'Items.Def', AttributeSlotMaxActivated=1)
        values, reached = self.values('Parts.Up', objs=objs)
        self.assertEqual([r['slot'] for r in reached], ['Capacity'])
        self.assertAlmostEqual(values['Recharge Delay'], 3.0, places=5)

    def test_zero_or_negative_values_are_refused(self):
        for rate in (0.0, -1.0):
            with self.subTest(rate=rate), self.assertRaises(ps.RefusedError) as raised:
                ps.card(chain(objects(rate=rate)), 'Grades.Shield', 2, 1)
            self.assertIn('Recharge Rate', str(raised.exception))

    def test_decode_failure_is_refused(self):
        with self.assertRaises(ps.RefusedError) as raised:
            ps.card(chain(failing={'Parts.Up'}), 'Grades.Shield', 2, 1)
        self.assertIn('Parts.Up', str(raised.exception))
        self.assertIn('did not decode', str(raised.exception))

    def test_missing_part_is_refused(self):
        objs = objects()
        del objs['Parts.Down']
        with self.assertRaises(ps.RefusedError) as raised:
            ps.card(chain(objs), 'Grades.Shield', 2, 1)
        self.assertIn('Parts.Down', str(raised.exception))
        self.assertIn('in no package', str(raised.exception))

    def test_weights_read_and_stage_window(self):
        cands = ps.candidates(chain(), 'Grades.Shield', 2)
        # Up: empty Manufacturers list -> flat 100; Down: the entry for Makers.A -> index 3 (25); Late: outside its stage window.
        self.assertEqual(cands['slots']['Alpha'], {'Parts.Up': 100.0, 'Parts.Down': 25.0})
        self.assertEqual(cands['slots']['Beta'], {'Parts.BatA': 100.0, 'Parts.BatB': 100.0})
        self.assertEqual(cands['manufacturer'], 'Makers.A')

    def test_candidates_match_prepare_vendors(self):
        # Drift guard: the weighted copy and prepare_vendors.item_parts give the same parts in the same order.
        for stage in (1, 2, 3, 5):
            c = chain()
            ours = ps.candidates(c, 'Grades.Shield', stage)
            theirs = pv.item_parts(c, pv.base_chain(c, 'Grades.Shield'), 'Items.Def', stage)
            theirs = {slot: list(dict.fromkeys(parts)) for slot, parts in theirs.items() if parts}
            self.assertEqual({slot: list(parts) for slot, parts in ours['slots'].items()}, theirs, stage)

    def test_name_parts_match_weapon_recipe(self):
        # Drift guard: the item-slot copy and weapon_recipe.choose_name_parts pick the same name parts. The weapon slots are
        # mapped onto the item slots in order; no part names a damage type, so the weapon's damage-type rule has no effect.
        objs = objects()
        objs['Names.PrefixHigh'] = ('WillowGame.ItemNamePartDefinition', {'PartName': 'High', 'Priority': 3})
        objs['Names.PrefixLow'] = ('WillowGame.ItemNamePartDefinition', {'PartName': 'Low', 'Priority': 1})
        objs['Names.PrefixLate'] = ('WillowGame.ItemNamePartDefinition', {'PartName': 'Late', 'MinExpLevelRequirement': 9})
        with_props(objs, 'Parts.Up', PrefixList=[ref('Names.PrefixHigh'), ref('Names.PrefixLate')])
        with_props(objs, 'Parts.BatA', PrefixList=[ref('Names.PrefixLow')])
        with_props(objs, 'Parts.Mat', TitleList=[ref('Names.Title')])
        c = chain(objs)
        mapping = dict(zip(ps.ITEM_SLOTS, weapon_recipe.SLOTS))
        for body in ('Parts.Up', 'Parts.Down'):
            for battery in ('Parts.BatA', 'Parts.BatB'):
                parts = {'Alpha': body, 'Beta': battery, 'Material': 'Parts.Mat'}
                weapon_parts = {mapping[s]: p for s, p in parts.items()}
                for level in (2, 10):
                    self.assertEqual(ps.name_parts(c, 'Items.Def', parts, 'Makers.A', level),
                                     weapon_recipe.choose_name_parts(c, 'Items.Def', weapon_parts, 'Makers.A', level),
                                     (body, battery, level))

    def test_roll_is_seeded_and_inside_the_range(self):
        first = ps.card(chain(), 'Grades.Shield', 2, 1)
        self.assertEqual(first, ps.card(chain(), 'Grades.Shield', 2, 1))
        item = first['items'][0]
        prov = item['provenance']
        self.assertEqual(prov['combinations'], 4)
        self.assertEqual(prov['status'], ps.STATUS)
        self.assertIn('UNVERIFIED', prov['status'])
        for stat in item['stats']:
            low, high = prov['range'][stat['label']]
            self.assertLessEqual(low - 1e-3, float(stat['value']))
            self.assertLessEqual(float(stat['value']), high + 1e-3)
            self.assertTrue(0.0 <= prov['position_in_range'][stat['label']] <= 1.0)
        self.assertEqual(item['assetId'], '')
        self.assertEqual(item['name'], 'Test Shield')
        self.assertEqual([s['label'] for s in item['stats']], ['Capacity', 'Recharge Rate', 'Recharge Delay'])
        self.assertAlmostEqual(prov['range']['Capacity'][0], 40.0 / 1.1, places=3)
        self.assertAlmostEqual(prov['range']['Capacity'][1], 48.0, places=3)

    def test_every_seed_lands_inside_the_range(self):
        for seed in range(1, 40):
            prov = ps.card(chain(), 'Grades.Shield', 2, seed)['items'][0]['provenance']
            for value in prov['position_in_range'].values():
                self.assertTrue(0.0 <= value <= 1.0, (seed, prov['position_in_range']))

    def test_too_many_combinations_are_refused(self):
        with mock.patch.object(ps, 'MAX_COMBINATIONS', 3), self.assertRaises(ps.RefusedError) as raised:
            ps.card(chain(), 'Grades.Shield', 2, 1)
        self.assertIn('4 part combinations', str(raised.exception))

    def test_not_a_shield_is_refused(self):
        with self.assertRaises(ps.RefusedError):
            ps.card(chain(), 'Grades.Gun', 2, 1)

    def test_level_above_the_slice_is_refused(self):
        with self.assertRaises(SystemExit) as raised:
            ps.main(['--game', 'nowhere', '--level', str(ps.SLICE_LEVEL + 1)])
        self.assertEqual(raised.exception.code, 2)

    def test_output_outside_local_is_refused(self):
        with tempfile.TemporaryDirectory() as folder:
            self.assertEqual(ps.main(['--game', 'nowhere', '--output', str(Path(folder) / 'card.json')]), 2)

    def test_keep_or_refuse(self):
        document = ps.card(chain(), 'Grades.Shield', 2, 1)
        other = ps.card(chain(), 'Grades.Shield', 1, 1)
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / 'card.json'
            self.assertEqual(ps.replace_problem(out, document, False), '')
            ps.write(out, document)
            self.assertEqual(ps.replace_problem(out, document, False), '')
            self.assertIn('different card', ps.replace_problem(out, other, False))
            self.assertEqual(ps.replace_problem(out, other, True), '')
            out.write_text('{not json', encoding='utf-8')
            self.assertIn('cannot be read', ps.replace_problem(out, other, False))

    def run_main(self, out, *extra):
        argv = ['--game', 'nowhere', '--level', '2', '--output', str(out), '--balance', 'Grades.Shield', *extra]
        with mock.patch.object(ps, 'open_chain', lambda reader, game: chain()), \
                mock.patch.object(ps.pv, 'local_path', lambda value: Path(value)):
            return ps.main(argv)

    def test_replace_through_main(self):
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / 'card.json'
            out.write_text(json.dumps({'schemaVersion': 1, 'items': []}), encoding='utf-8')
            self.assertEqual(self.run_main(out), 1)  # a different card is kept
            self.assertEqual(json.loads(out.read_text(encoding='utf-8'))['items'], [])
            self.assertEqual(self.run_main(out, '--replace'), 0)
            self.assertEqual(json.loads(out.read_text(encoding='utf-8')), ps.card(chain(), 'Grades.Shield', 2, 1))
            self.assertEqual(self.run_main(out), 0)  # the same card again: nothing to refuse

    def test_failed_write_leaves_the_card_untouched(self):
        document = ps.card(chain(), 'Grades.Shield', 2, 1)
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / 'card.json'
            out.write_text('old card', encoding='utf-8')
            with mock.patch.object(Path, 'replace', side_effect=OSError('invented failure')), self.assertRaises(OSError):
                ps.write(out, document)
            self.assertEqual(out.read_text(encoding='utf-8'), 'old card')
            self.assertFalse(out.with_name(out.name + '.tmp').exists())
            with self.assertRaises(TypeError):
                ps.write(out, {'not json': {1, 2}})
            self.assertEqual(out.read_text(encoding='utf-8'), 'old card')
            self.assertFalse(out.with_name(out.name + '.tmp').exists())


if __name__ == '__main__':
    unittest.main()
