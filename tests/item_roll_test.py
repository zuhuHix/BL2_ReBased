"""Synthetic test for tools/item_roll.py: invented registries and objects, no game files or values.

AI-assisted (Claude Code). Each case builds a small registry in the shape tools/item_registry.py writes and a dictionary
lookup standing in for the package census. Every object name and number is invented. Run: python tests/item_roll_test.py
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import item_roll as ir  # noqa: E402

GLOBALS = ir.GLOBALS
SEEDS = ir.spread_seeds(60)  # spread: consecutive small seeds all draw near 0 first (see spread_seeds)


class Lookup:
    """props / get / classes over a dictionary, as item_registry's census answers them."""

    def __init__(self, objects, classes=None):
        self.objects = objects
        self.classes = classes or {}

    def props(self, path):
        if path not in self.objects:
            raise KeyError(path)
        return self.objects[path]

    def get(self, path):
        return self.objects.get(path)


def flat(part, low=None, high=None):
    return {'part': part, 'list': 'T.List', 'weight': {'source': 'flat', 'value': 100.0}, 'min_stage': low, 'max_stage': high}


def by_maker(part, weights, default=0.0):
    return {'part': part, 'list': 'T.List', 'min_stage': None, 'max_stage': None,
            'weight': {'source': 'manufacturer', 'by_manufacturer': weights, 'default': default}}


def grade(low=0, high=100, definition=None, exp_level=None, spawn=(1.0, 1.0)):
    return {'min_stage': low, 'max_stage': high, 'definition': definition, 'exp_level': exp_level,
            'spawn_probability': {'min': spawn[0], 'max': spawn[1]}}


def balance(kind, definition, slots, makers=(('T.MakerA', [grade()]),), class_mods=(), variants=None):
    record = {'kind': kind, 'inventory_definition': definition, 'class_mod_definitions': list(class_mods),
              'manufacturers': [{'manufacturer': m, 'grades': g} for m, g in makers]}
    record['variants'] = variants or [{'definition': definition, 'slots': slots}]
    return record


def rarity_attr(value):
    """An init whose base attribute has one constant resolver (as the item rarity attributes do)."""
    return {'BaseValueAttribute': f'T.Rarity{value}', 'BaseValueScaleConstant': 1.0}


def world(extra=None, classes=None):
    """Globals with invented bands, five rarity attributes, and `extra` objects."""
    objects = {GLOBALS: {'RarityLevelColors': [
        {'MinLevel': 0, 'MaxLevel': 0, 'RarityRating': 'RARITY_Unknown'},
        {'MinLevel': 1, 'MaxLevel': 1, 'RarityRating': 'RARITY_Common'},
        {'MinLevel': 2, 'MaxLevel': 2, 'RarityRating': 'RARITY_Uncommon'},
        {'MinLevel': 3, 'MaxLevel': 5, 'RarityRating': 'RARITY_Rare'},
        {'MinLevel': 7, 'MaxLevel': 9, 'RarityRating': 'RARITY_Legendary'}]}}
    all_classes = {}
    for value in range(1, 6):
        objects[f'T.Rarity{value}'] = {'ValueResolverChain': [f'T.Rarity{value}.Const']}
        objects[f'T.Rarity{value}.Const'] = {'ConstantValue': float(value)}
        all_classes[f'T.Rarity{value}.Const'] = 'Class GearboxFramework.ConstantAttributeValueResolver'
    objects.update(extra or {})
    all_classes.update(classes or {})
    return Lookup(objects, all_classes)


def registry(balances, definitions, parts):
    return {'balances': balances, 'definitions': definitions, 'parts': parts}


def shield_case():
    slots = {'Alpha': [flat('T.SA1'), flat('T.SA2')], 'Beta': [flat('T.SB1')],
             'Gamma': [flat('T.SG_late', low=20)],  # out of window below stage 20: the slot is empty there
             'Material': [flat('T.SM1'), flat('T.SM2'), flat('T.SM3')]}
    reg = registry({'T.ShieldBal': balance('shield', 'T.ShieldDef', slots)},
                   {'T.ShieldDef': {'class': 'ShieldDefinition', 'prefix': [], 'title': ['T.TitleShield']}},
                   {p: {'prefix': [], 'title': []} for p in ('T.SA1', 'T.SA2', 'T.SB1', 'T.SG_late', 'T.SM1', 'T.SM2')}
                   | {'T.SM3': {'prefix': ['T.PrefixRare'], 'title': []}})
    look = world({'T.ShieldDef': {}, 'T.SA1': {}, 'T.SA2': {}, 'T.SB1': {}, 'T.SG_late': {},
                  'T.SM1': {'Rarity': rarity_attr(1)}, 'T.SM2': {'Rarity': rarity_attr(2)},
                  'T.SM3': {'Rarity': rarity_attr(3)},
                  'T.TitleShield': {'PartName': 'Shieldy'},
                  'T.PrefixRare': {'PartName': 'Shiny', 'Priority': 2.0, 'Rarity': rarity_attr(1)}})
    return reg, look


class ShieldRolls(unittest.TestCase):
    def test_rolls_are_legal_and_reproducible(self):
        reg, look = shield_case()
        seen = set()
        for seed in SEEDS:
            item = ir.roll(reg, 'T.ShieldBal', 8, seed, look)
            self.assertEqual(ir.check(reg, item, look), [], item)
            self.assertEqual(item, ir.roll(reg, 'T.ShieldBal', 8, seed, look))
            self.assertEqual(item['level'], 8)
            self.assertEqual(item['manufacturer'], 'T.MakerA')
            self.assertNotIn('Gamma', item['parts'])  # its only candidate starts at stage 20
            seen.add(item['parts']['Material'])
        self.assertEqual(seen, {'T.SM1', 'T.SM2', 'T.SM3'})
        self.assertIn('Gamma', ir.roll(reg, 'T.ShieldBal', 20, 1, look)['parts'])

    def test_rarity_sums_parts_and_name_parts(self):
        reg, look = shield_case()
        by_material = {}
        for seed in SEEDS:
            item = ir.roll(reg, 'T.ShieldBal', 8, seed, look)
            by_material[item['parts']['Material']] = item
        self.assertEqual(by_material['T.SM1']['rarity']['level'], 1)
        self.assertEqual(by_material['T.SM1']['rarity']['band'], 'Common')
        self.assertEqual(by_material['T.SM2']['rarity']['band'], 'Uncommon')
        # SM3 brings the prefix, whose own rarity 1 counts (the stand-in): 3 + 1.
        rare = by_material['T.SM3']
        self.assertEqual(rare['name_parts'], {'prefix': 'T.PrefixRare', 'title': 'T.TitleShield'})
        self.assertEqual(rare['rarity']['level'], 4)
        self.assertEqual(rare['rarity']['band'], 'Rare')
        self.assertTrue(rare['rarity']['name_parts_counted'])

    def test_a_level_in_no_band_has_no_band(self):
        look = world()
        self.assertIsNone(ir.rarity_band(look, 6))
        self.assertEqual(ir.rarity_band(look, 8), 'Legendary')

    def test_checker_rejects_illegal_rolls(self):
        reg, look = shield_case()
        good = ir.roll(reg, 'T.ShieldBal', 8, 3, look)

        def broken(**change):
            item = {**good, **change}
            return ir.check(reg, item, look)

        self.assertTrue(any('not a candidate' in p for p in broken(parts={**good['parts'], 'Alpha': 'T.SB1'})))
        self.assertTrue(any('not a candidate' in p for p in broken(parts={**good['parts'], 'Gamma': 'T.SG_late'})))
        emptied = dict(good['parts'])
        del emptied['Beta']
        self.assertTrue(any('empty although' in p for p in broken(parts=emptied)))
        self.assertTrue(any('level' in p for p in broken(level=9)))
        self.assertTrue(any('rarity level' in p for p in broken(rarity={**good['rarity'], 'level': 42})))
        self.assertTrue(any('no drawable grade' in p for p in broken(manufacturer='T.MakerZ')))
        # The legal roll moved to a stage outside its manufacturer's only grade window (0..100).
        self.assertTrue(any('no drawable grade at stage 101' in p for p in broken(stage=101)))
        self.assertTrue(any('name' in p for p in broken(name_parts={'prefix': 'T.PrefixRare', 'title': None})))
        self.assertTrue(any('not one this balance yields' in p for p in broken(definition='T.OtherDef')))

    def test_checker_catches_a_roll_with_a_wrong_rarity_rule(self):
        """A roll built with a rarity that ignores the parts fails check(), which sums the raw properties itself."""
        reg, look = shield_case()
        original = ir.rarity_terms
        try:
            ir.rarity_terms = lambda lookup, definition, parts, names, level: [(definition, 0)]
            item = ir.roll(reg, 'T.ShieldBal', 8, SEEDS[0], look)
        finally:
            ir.rarity_terms = original
        self.assertEqual(item['rarity']['level'], 0)
        self.assertTrue(any('rarity level 0 is not the sum' in p for p in ir.check(reg, item, look)))


class GrenadeRolls(unittest.TestCase):
    def case(self):
        slots = {'Alpha': [by_maker('T.GA_a', {'T.MakerA': 1.0}), by_maker('T.GA_b', {'T.MakerB': 1.0})],
                 'Epsilon': [flat('T.GE1')]}
        makers = (('T.MakerA', [grade(1, 10), grade(11, 100, spawn=(0.0, 0.0))]),
                  ('T.MakerB', [grade(5, 100)]))
        reg = registry({'T.GrenBal': balance('grenade_mod', 'T.GrenDef', slots, makers=makers)},
                       {'T.GrenDef': {'class': 'GrenadeModDefinition', 'prefix': [], 'title': []}},
                       {p: {'prefix': [], 'title': []} for p in ('T.GA_a', 'T.GA_b', 'T.GE1')})
        look = world({'T.GrenDef': {'BaseRarity': {'BaseValueConstant': 5.0}}, 'T.GA_a': {}, 'T.GA_b': {},
                      'T.GE1': {'Rarity': rarity_attr(4)}})
        return reg, look

    def test_manufacturer_follows_the_grade_windows_and_weights(self):
        reg, look = self.case()
        makers = {ir.roll(reg, 'T.GrenBal', 8, seed, look)['manufacturer'] for seed in SEEDS}
        self.assertEqual(makers, {'T.MakerA', 'T.MakerB'})
        for seed in SEEDS:
            low = ir.roll(reg, 'T.GrenBal', 2, seed, look)  # only maker A covers stage 2
            self.assertEqual((low['manufacturer'], low['parts']['Alpha']), ('T.MakerA', 'T.GA_a'))
            high = ir.roll(reg, 'T.GrenBal', 30, seed, look)  # A's covering grade weighs 0: only B
            self.assertEqual((high['manufacturer'], high['parts']['Alpha']), ('T.MakerB', 'T.GA_b'))
            for item in (low, high):
                self.assertEqual(ir.check(reg, item, look), [])
                self.assertEqual(item['rarity']['level'], 9)  # base 5 + epsilon 4
        with self.assertRaises(ir.NotRollable) as caught:
            ir.roll(reg, 'T.GrenBal', 0, 1, look)  # no grade covers stage 0
        self.assertEqual(caught.exception.reason, 'no_drawable_grade_at_stage')

    def test_checker_rejects_a_part_of_another_manufacturer(self):
        reg, look = self.case()
        item = ir.roll(reg, 'T.GrenBal', 2, 1, look)
        item['parts']['Alpha'] = 'T.GA_b'  # weighs the default 0 for maker A
        self.assertTrue(any('not a candidate' in p for p in ir.check(reg, item, look)))

    def test_non_interpolating_balance_takes_the_grade_level_and_definition(self):
        reg, look = self.case()
        record = reg['balances']['T.GrenBal']
        record['manufacturers'] = [{'manufacturer': 'T.MakerA', 'grades': [grade(1, 100, 'T.GrenDef2', exp_level=12)]}]
        record['variants'].append({'definition': 'T.GrenDef2', 'slots': {'Epsilon': [flat('T.GE1')]}})
        reg['definitions']['T.GrenDef2'] = {'class': 'GrenadeModDefinition', 'prefix': [], 'title': []}
        look.objects['T.GrenDef2'] = {}
        look.objects['T.GrenBal'] = {'bInterpolateExpLevel': False}
        item = ir.roll(reg, 'T.GrenBal', 8, 1, look)
        self.assertEqual((item['definition'], item['level']), ('T.GrenDef2', 12))
        self.assertEqual(ir.check(reg, item, look), [])
        look.objects['T.GrenBal'] = {}  # interpolating again: the balance's own definition, level = stage
        item = ir.roll(reg, 'T.GrenBal', 8, 1, look)
        self.assertEqual((item['definition'], item['level']), ('T.GrenDef', 8))


class ClassModRolls(unittest.TestCase):
    def test_definition_is_one_of_the_balance_class_mods(self):
        defs = ['T.CM1', 'T.CM2', 'T.CM3']
        variants = [{'definition': d, 'slots': {'Alpha': [flat(d + '_alpha')], 'Material': [flat('T.CMmat')]}}
                    for d in defs]
        reg = registry({'T.CMBal': balance('class_mod', None, None, class_mods=defs, variants=variants)},
                       {d: {'class': 'ClassModDefinition', 'prefix': [], 'title': []} for d in defs},
                       {p: {'prefix': [], 'title': []} for p in [d + '_alpha' for d in defs] + ['T.CMmat']})
        look = world({**{d: {} for d in defs}, **{d + '_alpha': {'Rarity': rarity_attr(2)} for d in defs},
                      'T.CMmat': {}})
        chosen = set()
        for seed in SEEDS:
            item = ir.roll(reg, 'T.CMBal', 15, seed, look)
            self.assertEqual(item['parts']['Alpha'], item['definition'] + '_alpha')  # the variant of that definition
            self.assertEqual(ir.check(reg, item, look), [])
            self.assertEqual(item['rarity']['band'], 'Uncommon')
            chosen.add(item['definition'])
        self.assertEqual(chosen, set(defs))
        item['parts']['Alpha'] = 'T.CM1_alpha' if item['definition'] != 'T.CM1' else 'T.CM2_alpha'
        self.assertTrue(any('not a candidate' in p for p in ir.check(reg, item, look)))
        reg['balances']['T.CMBal']['class_mod_definitions'] = []
        with self.assertRaises(ir.NotRollable):
            ir.roll(reg, 'T.CMBal', 15, 1, look)


class RelicHook(unittest.TestCase):
    def case(self, cls='ArtifactDefinition', flag=None):
        slots = {'Beta': [flat('T.Enable1'), flat('T.Enable3'), flat('T.Plain')]}
        reg = registry({'T.RelicBal': balance('relic', 'T.RelicDef', slots)},
                       {'T.RelicDef': {'class': cls, 'prefix': [], 'title': []}},
                       {p: {'prefix': [], 'title': []} for p in ('T.Enable1', 'T.Enable3', 'T.Plain')})
        definition = {'AttributeSlotEffects': [{'SlotName': 'Effect1'}, {'SlotName': 'Effect2'}]}
        if flag is not None:
            definition['bAllowInventoryDefToModifyPartWeight'] = flag
        look = world({'T.RelicDef': definition,
                      'T.Enable1': {'AttributeSlotUpgrades': [{'SlotName': 'Effect1'}]},
                      'T.Enable3': {'AttributeSlotUpgrades': [{'SlotName': 'Effect3'}]},  # a slot the relic lacks
                      'T.Plain': {}})
        return reg, look

    def test_hook_drops_parts_upgrading_only_slots_the_relic_lacks(self):
        reg, look = self.case()
        drawn = set()
        for seed in SEEDS:
            item = ir.roll(reg, 'T.RelicBal', 8, seed, look)
            self.assertEqual(ir.check(reg, item, look), [])
            self.assertEqual(item['hook_removed'], 1)
            drawn.add(item['parts']['Beta'])
        self.assertEqual(drawn, {'T.Enable1', 'T.Plain'})
        item['parts']['Beta'] = 'T.Enable3'
        self.assertTrue(any('not a candidate' in p for p in ir.check(reg, item, look)))

    def test_checker_catches_a_roll_with_a_wrong_hook(self):
        """Rolls built with a mutated hook fail check(), which restates the hook from the raw slot properties."""
        reg, look = self.case()
        original = ir.hook_keeps
        try:
            # Too strict: a part with no slot upgrades is dropped too. With T.Plain alone in a slot, the slot goes empty.
            ir.hook_keeps = lambda lookup, defined, part: bool(ir.slot_names(
                ir.props_of(lookup, part).get('AttributeSlotUpgrades')) & defined)
            reg['balances']['T.RelicBal']['variants'][0]['slots']['Alpha'] = [flat('T.Plain')]
            strict = ir.roll(reg, 'T.RelicBal', 8, SEEDS[0], look)
            # Too lax: every part keeps its weight, so T.Enable3 can be drawn.
            ir.hook_keeps = lambda lookup, defined, part: True
            lax = [ir.roll(reg, 'T.RelicBal', 8, seed, look) for seed in SEEDS]
        finally:
            ir.hook_keeps = original
        self.assertNotIn('Alpha', strict['parts'])
        self.assertTrue(any('Alpha: empty although' in p for p in ir.check(reg, strict, look)))
        drew_enable3 = [item for item in lax if item['parts']['Beta'] == 'T.Enable3']
        self.assertTrue(drew_enable3)
        for item in drew_enable3:
            self.assertTrue(any('T.Enable3 is not a candidate' in p for p in ir.check(reg, item, look)))

    def test_a_slot_named_none_names_no_slot(self):
        """An upgrade row whose slot name is literally None counts as no upgrade, in the roll and in the checker."""
        reg, look = self.case()
        look.objects['T.Enable3'] = {'AttributeSlotUpgrades': [{'SlotName': 'None'}]}
        drawn = set()
        for seed in SEEDS:
            item = ir.roll(reg, 'T.RelicBal', 8, seed, look)
            self.assertEqual(ir.check(reg, item, look), [])
            drawn.add(item['parts']['Beta'])
        self.assertIn('T.Enable3', drawn)
        counts = ir.none_slot_names(reg, look, ['T.RelicBal'])
        self.assertEqual(counts['AttributeSlotUpgrades rows naming no slot'], 1)

    def test_hook_only_where_the_flag_is_set(self):
        for cls, flag in (('ShieldDefinition', None), ('ArtifactDefinition', False)):
            reg, look = self.case(cls, flag)
            drawn = {ir.roll(reg, 'T.RelicBal', 8, seed, look)['parts']['Beta'] for seed in SEEDS}
            self.assertEqual(drawn, {'T.Enable1', 'T.Enable3', 'T.Plain'}, cls)


class Refusals(unittest.TestCase):
    def test_other_kinds_and_unknown_balances_are_not_rolled(self):
        reg, look = shield_case()
        reg['balances']['T.Gun'] = balance('weapon', 'T.GunDef', {})
        for path, reason in (('T.Gun', 'other_kind'), ('T.Missing', 'unknown_balance')):
            with self.assertRaises(ir.NotRollable) as caught:
                ir.roll(reg, path, 8, 1, look)
            self.assertEqual(caught.exception.reason, reason)

    def test_balance_without_manufacturers_rolls_flat(self):
        reg, look = shield_case()
        reg['balances']['T.ShieldBal']['manufacturers'] = []
        item = ir.roll(reg, 'T.ShieldBal', 8, 1, look)
        self.assertIsNone(item['manufacturer'])
        self.assertEqual(ir.check(reg, item, look), [])


if __name__ == '__main__':
    unittest.main()
