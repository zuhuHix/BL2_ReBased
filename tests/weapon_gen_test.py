"""Synthetic test for ow-weapongen (src/weapon_gen.cpp): a hand-written manifest, expected values by hand.

Usage: weapon_gen_test.py <ow-weapongen executable>. No game data: every object, number and name
below is invented. The manifest format is documented in tools/weapon_gen_manifest.py.

CrtRand (MSVC rand) draws used below: srand(1) -> 41, 18467, 6334; srand(6000) -> 19632, ...;
srand(8000) -> 26163, 11738.

The tests at the end add one rule each to a copy of the manifest: the sale cap, the attribute-slot
grade (base grade, clamps, the constant 1), the stage cap and the slag chance remap.
"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

EXE = None
W = 'D_Attributes.Weapon.'

RECORDS = [
    ('V', 'weapon_gen_manifest', '2'),
    # Attributes: one runtime manufacturer flag, the part price total, two price constants.
    ('A', 0, 'Test.Attr.Weapon_Is_Acme', '-'),
    ('A', 1, 'D_Attributes.Inventory.InventoryPartMonetaryValueModifierTotal', '-'),
    ('A', 2, 'Test.Price.Double', '2.0'),
    ('A', 3, 'Test.Price.Half', '1.5'),
    # Initializers: constant, attribute, definition, scale.
    ('I', 0, '0.0', -1, -1, '-'),
    ('I', 1, '1.0', -1, -1, '-'),      # min stage 1
    ('I', 2, '5.0', -1, -1, '-'),      # min stage 5
    ('I', 3, '4.9', -1, -1, '-'),      # max stage 4.9: truncated to 4
    ('I', 4, '300.0', -1, -1, '-'),    # a manufacturer weight
    ('I', 5, '10.0', -1, -1, '-'),     # base damage
    ('I', 6, '-0.2', -1, -1, '-'),     # a negative scale
    ('I', 7, '0.5', -1, -1, '-'),
    ('I', 8, '2.0', -1, -1, '-'),
    ('I', 9, '0.0', 0, -1, '-'),       # weight needing Weapon_Is_Acme: unknown while rolling
    ('I', 10, '0.0', 0, -1, '3.0'),    # Weapon_Is_Acme * 3: 3 when evaluating an Acme gun
    ('I', 11, '1.0', -1, -1, '-'),
    ('I', 12, '1.9', -1, -1, '-'),     # rarity 1.9 counts 1
    ('I', 13, '0.0', 1, -1, '50.0'),   # price = part total * 50
    ('O', 0, 'WillowGame.WeaponBalanceDefinition', 'Test.Bal'),
    ('O', 1, 'WillowGame.WeaponTypeDefinition', 'Test.Type'),
    ('O', 2, 'WillowGame.WeaponPartDefinition', 'Body.A'),
    ('O', 3, 'WillowGame.WeaponPartDefinition', 'Body.B'),
    ('O', 4, 'WillowGame.WeaponPartDefinition', 'Grip.Early'),
    ('O', 5, 'WillowGame.WeaponPartDefinition', 'Grip.Late'),
    ('O', 6, 'WillowGame.WeaponPartDefinition', 'Barrel.A'),
    ('O', 7, 'WillowGame.WeaponPartDefinition', 'Barrel.B'),
    ('O', 8, 'WillowGame.WeaponNamePartDefinition', 'Name.TypeTitle'),
    ('O', 9, 'WillowGame.WeaponNamePartDefinition', 'Name.BodyTitle'),
    ('O', 10, 'WillowGame.WeaponNamePartDefinition', 'Name.P1'),
    ('O', 11, 'WillowGame.WeaponNamePartDefinition', 'Name.P2'),
    ('O', 12, 'WillowGame.WeaponNamePartDefinition', 'Name.P3'),
    ('O', 13, 'WillowGame.WeaponNamePartDefinition', 'Name.P4'),
    ('O', 14, 'WillowGame.AttributePresentationDefinition', 'Pres.Red'),
    ('O', 15, 'WillowGame.ItemPoolDefinition', 'Pool.Main'),
    ('O', 16, 'WillowGame.ItemPoolDefinition', 'Pool.Sub'),
    ('O', 17, 'WillowGame.ItemPoolDefinition', 'Pool.Q'),
    ('O', 18, 'WillowGame.InventoryBalanceDefinition', 'Gun.Item'),
    # Weapon type: bases (33.35 is the float 33.349998), the Acme +3 damage, a title, rarity, price.
    ('F', 1, 'Base', 'InstantHitDamage', 'init', 5),
    ('F', 1, 'Base', 'FireRate', 'num', '0.4'),
    ('F', 1, 'Base', 'ClipSize', 'num', '12.0'),
    ('F', 1, 'Base', 'ReloadTime', 'num', '33.35'),
    ('F', 1, 'Base', 'Spread', 'num', '1.5'),
    ('F', 1, 'Base', 'ProjectilesPerShot', 'num', '1.0'),
    ('F', 1, 'Effect', W + 'WeaponDamage', 'MT_PostAdd', 10),
    ('F', 1, 'TitleList', '8'),
    ('F', 1, 'BaseRarity', 11),
    ('F', 1, 'SlotBaseGrade', 11),
    ('F', 1, 'MonetaryValue', 13),
    ('F', 1, 'MonetaryBase', '2.0'),   # BaseMonetaryValueModifier: the part total starts at 2
    ('F', 1, 'UsesLevelRequirement'),
    ('F', 1, 'Gestalt', 'Test.Gestalt'),
    ('F', 2, 'Effect', W + 'WeaponDamage', 'MT_Scale', 6),
    ('F', 2, 'Effect', W + 'WeaponClipSize', 'MT_Scale', 6),
    ('F', 2, 'TitleList', '9'),
    ('F', 2, 'PrefixList', '10,11,12,13'),
    ('F', 2, 'Rarity', 12),
    ('F', 2, 'MonetaryValueMod', 2),
    ('F', 2, 'CustomPresentations', '14'),
    ('F', 2, 'Fragments', 'Frag_Body,Frag_Body_Var1'),
    ('F', 4, 'Effect', W + 'WeaponDamage', 'MT_Scale', 7),
    ('F', 4, 'Effect', W + 'WeaponDamage', 'MT_PreAdd', 8),
    ('F', 4, 'Fragments', 'Frag_Grip'),
    ('F', 8, 'Name', 'Pistol', '1.0', '1.0', '100.0', 'any'),
    ('F', 9, 'Name', 'Blaster', '1.0', '1.0', '100.0', 'any'),   # ties the type's title: the part wins
    ('F', 10, 'Name', 'Plain', '0.5', '1.0', '100.0', 'any'),
    ('F', 11, 'Name', 'Sharp', '0.5', '1.0', '100.0', 'maker:Acme'),  # ties Plain: the later one wins
    ('F', 11, 'MonetaryValueMod', 3),
    ('F', 12, 'Name', 'Wrong', '5.0', '1.0', '100.0', 'maker:Other'),
    ('F', 13, 'Name', 'Late', '0.9', '5.0', '100.0', 'any'),       # only from level 5
    ('F', 14, 'Line', 'Hot stuff', '#DC4646', '1', '• <font color="#DC4646">Hot stuff</font>'),
    ('B', 0, 1, 'Test.Makers.Acme', '0'),
    ('S', 0, 'Body', 2, 'flat', -1, -1),
    ('S', 0, 'Body', 3, 4, -1, 3),
    ('S', 0, 'Grip', 4, 'flat', 1, -1),
    ('S', 0, 'Grip', 5, 'flat', 2, -1),
    ('S', 0, 'Barrel', 6, 'flat', -1, -1),
    ('S', 0, 'Barrel', 7, 9, -1, -1),
    ('G', 'rarity', '1.0', '1.0', 'RARITY_Common', '#FFFFFF'),
    ('G', 'rarity', '2.0', '2.0', 'RARITY_Uncommon', '#3DD20B'),
    ('G', 'round', 'damage', 'ATTRROUNDING_IntCeil', '1.0'),
    ('G', 'round', 'magazine', 'ATTRROUNDING_IntFloor', '1.0'),
    ('G', 'round', 'fire_rate', 'ATTRROUNDING_Float', '1.0'),
    ('G', 'round', 'reload_time', 'ATTRROUNDING_Float', '1.0'),
    ('G', 'round', 'accuracy', 'ATTRROUNDING_Float', '1.0'),
    ('G', 'accuracy', '0.0', '15.0', '100.0', '0.0'),
    # Pools: Main = item 3 : sub-pool 1, the sub-pool gated to stage >= 10; Q draws round-half-even(2.5) = 2.
    ('F', 15, 'Quantity', '1.0'), ('F', 15, 'Gate', '-', '-'),
    ('F', 15, 'Entry', 'item', 18, '3.0'), ('F', 15, 'Entry', 'pool', 16, '1.0'),
    ('F', 16, 'Quantity', '-'), ('F', 16, 'Gate', '10.0', '-'), ('F', 16, 'Entry', 'item', 0, '1.0'),
    ('F', 17, 'Quantity', '2.5'), ('F', 17, 'Gate', '-', '-'), ('F', 17, 'Entry', 'item', 18, '1.0'),
]


class WeaponGenTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.TemporaryDirectory()
        cls.manifest = Path(cls.dir.name) / 'manifest.txt'
        cls.manifest.write_text('\n'.join('\t'.join(map(str, r)) for r in RECORDS) + '\n', encoding='utf-8')

    @classmethod
    def tearDownClass(cls):
        cls.dir.cleanup()

    def run_gen(self, *args, manifest=None):
        result = subprocess.run([EXE, str(manifest or self.manifest), *map(str, args)], capture_output=True,
                                text=True, encoding='utf-8')
        self.assertEqual(result.returncode, 0, result.stderr)
        return [json.loads(line) for line in result.stdout.splitlines()]

    def gun(self, stage, seed, manifest=None):
        return self.run_gen('--balance', 'Test.Bal', '--stage', stage, '--seed', seed, manifest=manifest)[0]

    def variant(self, name, extra, drop=()):
        """A copy of the manifest without the records in `drop`, plus `extra` (a V header in it goes first)."""
        path = Path(self.dir.name) / f'{name}.txt'
        headers = [r for r in extra if r[0] == 'V']
        records = headers + [r for r in RECORDS if r not in drop] + [r for r in extra if r[0] != 'V']
        path.write_text('\n'.join('\t'.join(map(str, r)) for r in records) + '\n', encoding='utf-8')
        return path

    def test_pick_weights_and_stage_window(self):
        gun = self.gun(1, 1)
        # Body: A flat 100 + B 300; draw 41 -> r = 400 * 41 / 32767 = 0.50, inside A's [0, 100].
        self.assertEqual(gun['parts']['Body'], {'part': 'Body.A', 'weight': 100.0, 'candidates': 2})
        # Grip at stage 1: Late needs stage >= 5, so Early is the only candidate (draw 18467 used anyway).
        self.assertEqual(gun['parts']['Grip']['part'], 'Grip.Early')
        self.assertEqual(gun['parts']['Grip']['candidates'], 1)
        # Barrel: B's weight needs Weapon_Is_Acme, unknown while rolling: treated as 1 -> total 101,
        # draw 6334 -> r = 19.5 -> A.
        self.assertEqual(gun['parts']['Barrel'], {'part': 'Barrel.A', 'weight': 100.0, 'candidates': 2})
        self.assertEqual(gun['notes'], ['Barrel: some weights need runtime attributes; treated as 1'])
        self.assertEqual(gun['gestalt_fragments'], ['Frag_Body', 'Frag_Body_Var1', 'Frag_Grip'])
        # Stage 8: Body.B's max stage trunc(4.9) = 4 drops it; Grip total 200, r = 112.7 -> Late.
        late = self.gun(8, 1)
        self.assertEqual(late['parts']['Body']['candidates'], 1)
        self.assertEqual(late['parts']['Grip']['part'], 'Grip.Late')
        # srand(6000) draws 19632 first: r = 239.7, inside B's [100, 400].
        self.assertEqual(self.gun(1, 6000)['parts']['Body'], {'part': 'Body.B', 'weight': 300.0, 'candidates': 2})

    def test_name_priority_ties_maker_and_level_window(self):
        gun = self.gun(1, 1)
        # Title: type 'Pistol' 1 vs part 'Blaster' 1 -> the later list wins. Prefix: Plain 0.5 and
        # Sharp 0.5 (Acme) tie -> Sharp; Wrong needs Other; Late needs level 5.
        self.assertEqual(gun['name'], 'Sharp Blaster')
        self.assertEqual(gun['prefix'], {'part': 'Name.P2', 'text': 'Sharp', 'priority': 0.5})
        self.assertEqual(gun['title']['part'], 'Name.BodyTitle')
        self.assertEqual(self.gun(8, 1)['name'], 'Late Blaster')  # Late 0.9 beats Sharp from level 5

    def test_modifier_stack_and_card_rounding(self):
        card = self.gun(1, 1)['card']
        # Damage: (10 + 2) * (1 + 0.5) / (1 + 0.2f) + 3 (Weapon_Is_Acme * 3) = 17.99999996 -> float 18.
        self.assertEqual(card['damage'], 18.0)
        # Clip: 12 / (1 + 0.2f) = 9.99999998, an integer attribute: truncated to 9.
        self.assertEqual(card['magazine'], 9.0)
        # Reload 33.35 is the float 33.349998474; x10 = 333.49998474 is a float tie that rounds to the even
        # 333.5, + 0.5 = 334 -> the card prints 33.4 (double arithmetic would print 33.3).
        self.assertEqual(card['reload_time'], 33.349998474121094)
        self.assertEqual(card['display'], {'damage': 18, 'magazine': 9, 'fire_rate': 2.5, 'reload_time': 33.4,
                                           'accuracy': 90.0})
        # Fire rate 1 / float(0.4) = 2.49999996 -> float 2.5; accuracy 100 - 1.5 * 6.6666665 = 90.
        self.assertEqual((card['fire_rate'], card['accuracy']), (2.5, 90.0))
        # Stage 8 (Grip.Late, no grip effects): 10 / (1 + 0.2f) + 3 = 11.3333333 -> float 11.333333015;
        # IntCeil prints 12.
        late = self.gun(8, 1)['card']
        self.assertEqual((late['damage'], late['display']['damage']), (11.333333015441895, 12))

    def test_rarity_price_level_and_text(self):
        card = self.gun(1, 1)['card']
        # Rarity: trunc(1.0) + trunc(1.9) = 2 -> the Uncommon row.
        self.assertEqual((card['rarity_level'], card['rarity'], card['rarity_color']), (2, 2, '#3DD20B'))
        # Price: part total 2 (type) * 2 (Body.A) * 1.5 (prefix Sharp) = 6, * 50 = 300; not a checked
        # calculator.
        self.assertEqual((card['sale_value'], card['sale_value_known']), (300, False))
        self.assertEqual((card['level_requirement'], card['level_line']), (1, False))
        self.assertEqual(card['fun_stats'], 'Hot stuff')
        self.assertEqual(card['fun_stats_markup'], '• <font color="#DC4646">Hot stuff</font>')
        self.assertEqual((card['manufacturer'], card['projectiles'], card['shot_cost']), ('Acme', 1, 1.0))
        late = self.gun(8, 1)['card']
        # Stage 8: prefix Late has no modifier: 2 * 2 * 50 = 200.
        self.assertEqual((late['sale_value'], late['level_requirement'], late['level_line']), (200, 8, True))

    def test_pools(self):
        # Stage 1: the sub-pool's gate (>= 10) is false, so the item has the whole share.
        lines = self.run_gen('--pool', 'Pool.Main', '--stage', 1, '--seed', 8000)
        self.assertEqual(lines[0]['drops'], ['Gun.Item'])
        # Stage 20: shares 0.75 / 0.25; draw 26163 -> 0.798 -> the sub-pool, whose one draw gives the gun,
        # rolled with seed 8000 + drop index 0.
        lines = self.run_gen('--pool', 'Pool.Main', '--stage', 20, '--seed', 8000)
        self.assertEqual(lines[0]['drops'], ['Test.Bal'])
        self.assertEqual((lines[1]['balance'], lines[1]['seed']), ('Test.Bal', 8000))
        # Quantity 2.5 rounds half to even: 2 draws.
        self.assertEqual(self.run_gen('--pool', 'Pool.Q', '--stage', 1, '--seed', 1)[0]['drops'], ['Gun.Item'] * 2)

    def test_sale_value_currency_cap(self):
        # SaleCap is the FormOfCurrency cap the builder writes (Eridium would be 500): 300 -> 250.
        manifest = self.variant('cap', [('F', 1, 'SaleCap', '250')])
        self.assertEqual(self.gun(1, 1, manifest)['card']['sale_value'], 250)
        # A version 1 manifest has no MonetaryBase: the part total starts at 1 (2 * 1.5 * 50 = 150).
        old = self.variant('v1', [], drop=[('F', 1, 'MonetaryBase', '2.0')])
        old.write_text(old.read_text(encoding='utf-8').replace('weapon_gen_manifest\t2', 'weapon_gen_manifest\t1'),
                       encoding='utf-8')
        self.assertEqual(self.gun(1, 1, old)['card']['sale_value'], 150)

    def test_slot_grade_base_clamps_and_constant_one(self):
        # Slot 'Sp' adds (grade - 1) * 0.5 (init 7) to the spread (base 1.5); grade clamped to [0, 4], max first.
        effect = ('F', 1, 'SlotEffect', 'Sp', W + 'WeaponSpread', 'MT_PreAdd', 0, 7, '0', '1', '0.0', '1', '4.0')
        # Base grade 1 + Body.A's -3 = -2 -> minimum 0 -> (0 - 1) * 0.5 = -0.5: spread 1.0, accuracy
        # 100 + 1.0 * f32(-100 / 15) = 93.333336. (The old rule clamped the sum -3 to 0: spread 1.5.)
        low = self.variant('slot_low', [effect, ('F', 2, 'SlotUpgrade', 'Sp', '-3.0', '1')])
        card = self.gun(1, 1, low)['card']
        self.assertEqual((card['spread'], card['accuracy']), (1.0, 93.33333587646484))
        # Base grade 2 (init 8) + 3 = 5 -> maximum 4 -> (4 - 1) * 0.5 = 1.5: spread 3.0, accuracy 80.
        high = self.variant('slot_high', [effect, ('F', 2, 'SlotUpgrade', 'Sp', '3.0', '1'),
                                          ('F', 1, 'SlotBaseGrade', 8)], drop=[('F', 1, 'SlotBaseGrade', 11)])
        card = self.gun(1, 1, high)['card']
        self.assertEqual((card['spread'], card['accuracy']), (3.0, 80.0))

    def test_stage_cap(self):
        # Stage 8 capped at 1 builds the stage-1 gun: Grip.Early, 'Sharp Blaster', level 1, damage 18.
        capped = self.run_gen('--balance', 'Test.Bal', '--stage', 8, '--seed', 1, '--stage-cap', 1)[0]
        plain = self.gun(1, 1)
        self.assertEqual((capped['stage'], capped['spawn_stage'], capped['stage_cap']), (1, 8, 1))
        for key in ('parts', 'name', 'card'):
            self.assertEqual(capped[key], plain[key])
        # A cap above the stage changes nothing; a batch line's fifth field is the cap.
        uncapped = self.run_gen('--balance', 'Test.Bal', '--stage', 8, '--seed', 1, '--stage-cap', 50)[0]
        self.assertEqual(uncapped['name'], 'Late Blaster')
        batch = Path(self.dir.name) / 'batch.txt'
        batch.write_text('balance Test.Bal 8 1 1\nbalance Test.Bal 8 1\n', encoding='utf-8')
        first, second = self.run_gen('--batch', batch)
        self.assertEqual((first['card']['level'], second['card']['level']), (1, 8))
        self.assertNotIn('spawn_stage', second)

    def test_slag_chance_remap(self):
        status = [
            ('O', 19, 'WillowGame.WillowDamageTypeDefinition', 'Dmg.Amp'),
            ('O', 20, 'WillowGame.StatusEffectDefinition', 'Status.Amp'),
            ('I', 14, '30.0', -1, -1, '-'),
            ('I', 15, '5.0', -1, -1, '-'),
            ('F', 1, 'DefaultDamageType', 19),
            ('F', 19, 'StatusEffect', 20),
            ('F', 20, 'StatusEffectType', 'STATUS_EFFECT_Amp'),
            ('F', 20, 'Surface', 'generic', 14),
        ]
        two = ('F', 1, 'Base', 'BaseStatusEffectChanceModifier', 'init', 8)   # combined modifier 2.0
        card = self.gun(1, 1, self.variant('slag_plain', status + [two]))['card']
        # No remap for the damage type: Generic BaseChance 30 * 2.0 = 60.
        self.assertEqual((card['status_chance'], card['display']['status_chance']), (60.0, 60.0))
        remap = ('G', 'status_remap', 'Dmg.Amp', '0.0', '3.3299999237060547', '0.0', '100.0')
        card = self.gun(1, 1, self.variant('slag', status + [two, remap]))['card']
        # Remap [0, 3.33] -> [0, 100]: slope f32(100 / 3.33f) = 30.030031, * 2.0 = 60.060062 -> prints 60.1.
        self.assertEqual((card['status_chance'], card['display']['status_chance']), (60.060062408447266, 60.1))
        self.assertEqual(card['status_chance_by_surface'], {'generic': 60.0})  # gameplay chance unchanged
        # The input clamps at 3.33: a combined modifier of 5 (init 15) shows 100.
        five = ('F', 1, 'Base', 'BaseStatusEffectChanceModifier', 'init', 15)
        self.assertEqual(self.gun(1, 1, self.variant('slag_five', status + [five, remap]))['card']['status_chance'],
                         100.0)

    def test_damage_type_name_expressions(self):
        """Version 3 NameExpr: an element-gated title (priority 2) on the elemental part beats the body's
        'Blaster' (1) on a Zap gun; a '!= 1' prefix (0.6, beats Sharp 0.5) shows only without Zap."""
        records = [
            ('O', 19, 'WillowGame.WillowDamageTypeDefinition', 'Dmg.DmgType_Zap_NoDoT'),
            ('O', 20, 'WillowGame.WeaponPartDefinition', 'Elem.Zap'),
            ('O', 21, 'WillowGame.WeaponPartDefinition', 'Elem.Plain'),
            ('O', 22, 'WillowGame.WeaponNamePartDefinition', 'Name.ZapTitle'),
            ('O', 23, 'WillowGame.WeaponNamePartDefinition', 'Name.Dull'),
            ('F', 20, 'CustomDamageType', 19),
            ('F', 20, 'TitleList', '22'), ('F', 20, 'PrefixList', '23'),
            ('F', 21, 'TitleList', '22'), ('F', 21, 'PrefixList', '23'),
            ('F', 22, 'Name', 'Zapper', '2.0', '1.0', '100.0', 'expr'),
            ('F', 22, 'NameExpr', 'maker', 'Acme', '-', 'OPERATOR_EqualTo', '1.0'),
            ('F', 22, 'NameExpr', 'damage', 'DmgType_Zap:1.0,DmgType_Zap_NoDoT:1.0', '0.0', 'OPERATOR_EqualTo', '1.0'),
            ('F', 23, 'Name', 'Dull', '0.6', '1.0', '100.0', 'expr'),
            ('F', 23, 'NameExpr', 'damage', 'DmgType_Zap:1.0,DmgType_Zap_NoDoT:1.0', '0.0', 'OPERATOR_NotEqualTo', '1.0'),
        ]
        version = ('V', 'weapon_gen_manifest', '3')
        drop = [('V', 'weapon_gen_manifest', '2')]
        zap = self.variant('expr_zap', [version] + records + [('S', 0, 'Elemental', 20, 'flat', -1, -1)], drop)
        plain = self.variant('expr_plain', [version] + records + [('S', 0, 'Elemental', 21, 'flat', -1, -1)], drop)
        gun = self.gun(1, 1, zap)
        self.assertEqual(gun['parts']['Elemental']['part'], 'Elem.Zap')
        self.assertEqual((gun['name'], gun['title']['part']), ('Sharp Zapper', 'Name.ZapTitle'))
        self.assertEqual(self.gun(1, 1, plain)['name'], 'Dull Blaster')
        # The name rule reads only the elemental part's custom damage type (then the type's default): a body
        # with one does not make the plain gun a Zap gun for its name, though the card's any-part rule shows it.
        body = self.variant('expr_body', [version] + records + [('S', 0, 'Elemental', 21, 'flat', -1, -1),
                                                               ('F', 2, 'CustomDamageType', 19)], drop)
        gun = self.gun(1, 1, body)
        self.assertEqual((gun['parts']['Body']['part'], gun['name']), ('Body.A', 'Dull Blaster'))
        self.assertEqual(gun['card']['damage_type'], 'Dmg.DmgType_Zap_NoDoT')
        typed = self.variant('expr_type', [version] + records + [('S', 0, 'Elemental', 21, 'flat', -1, -1),
                                                                ('F', 1, 'DefaultDamageType', 19)], drop)
        self.assertEqual(self.gun(1, 1, typed)['name'], 'Sharp Zapper')
        # Both expressions must hold: another maker's Zap gun keeps the body's title.
        other = Path(self.dir.name) / 'expr_other.txt'
        other.write_text(zap.read_text(encoding='utf-8').replace('Test.Makers.Acme', 'Test.Makers.Other'),
                         encoding='utf-8')
        self.assertEqual(self.gun(1, 1, other)['title']['part'], 'Name.BodyTitle')

    def test_count_and_bad_manifest(self):
        guns = self.run_gen('--balance', 'Test.Bal', '--stage', 1, '--seed', 1, '--count', 3)
        self.assertEqual([g['seed'] for g in guns], [1, 2, 3])
        bad = Path(self.dir.name) / 'bad.txt'
        bad.write_text('V\tweapon_gen_manifest\t1\nI\t0\tnot-a-number\t-1\t-1\t-\n', encoding='utf-8')
        result = subprocess.run([EXE, str(bad), '--balance', 'X', '--stage', '1', '--seed', '1'], capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn('line 2', result.stderr)

    def load_error(self, name, records):
        """ow-weapongen's error for a manifest of `records` (or raw text): exit 1, nothing on stdout."""
        path = Path(self.dir.name) / f'{name}.txt'
        text = records if isinstance(records, str) else '\n'.join('\t'.join(map(str, r)) for r in records) + '\n'
        path.write_text(text, encoding='utf-8')
        result = subprocess.run([EXE, str(path), '--balance', 'Test.Bal', '--stage', '1', '--seed', '1'],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertEqual(result.stdout, '')
        return result.stderr

    def test_parser_rejects_bad_headers_ids_and_references(self):
        body = [r for r in RECORDS if r[0] != 'V']
        self.assertIn('missing', self.load_error('no_header', body))
        self.assertIn('header', self.load_error('empty', ''))
        self.assertIn('unsupported', self.load_error('v4', [('V', 'weapon_gen_manifest', '4')] + body))
        self.assertIn('unsupported', self.load_error('other', [('V', 'other_manifest', '3')] + body))
        # A huge id is rejected before any table grows; a sparse one (a hole in a table) after parsing.
        self.assertIn('bad init id 9999999', self.load_error('huge', RECORDS + [('I', 9999999, '1.0', -1, -1, '-')]))
        self.assertIn('bad object id', self.load_error('huge_field', RECORDS + [('F', 2000000000, 'Rarity', 1)]))
        self.assertIn('init id 14 is used but has no I record',
                      self.load_error('sparse', RECORDS + [('I', 15, '1.0', -1, -1, '-')]))
        # Dangling references: an init past the table, an object past the table, a negative attribute.
        self.assertIn('dangling init id 14', self.load_error(
            'dangling_init', [r for r in RECORDS if r != ('F', 1, 'SlotBaseGrade', 11)] + [('F', 1, 'SlotBaseGrade', 14)]))
        self.assertIn('dangling object id 19',
                      self.load_error('dangling_object', RECORDS + [('F', 1, 'DefaultDamageType', 19)]))
        self.assertIn('dangling attribute id -2',
                      self.load_error('negative_attribute', RECORDS + [('I', 14, '1.0', -2, -1, '-')]))
        # An initializer whose definition refers back to it would recurse forever.
        cycle = RECORDS + [('I', 14, '1.0', -1, 0, '-'),
                           ('D', 0, 'Def.Loop', '-', '0', '1', 'Multiplier:14', '0', '0', '-', '-')]
        self.assertIn('refers back', self.load_error('cycle', cycle))

    def test_missing_slot_base_grade_defaults_to_one(self):
        # Without SlotBaseGrade the class default 1 applies (weapon_stats: the constant 1), as with init 11 (1.0).
        effect = ('F', 1, 'SlotEffect', 'Sp', W + 'WeaponSpread', 'MT_PreAdd', 0, 7, '0', '1', '0.0', '1', '4.0')
        upgrade = ('F', 2, 'SlotUpgrade', 'Sp', '-3.0', '1')
        with_grade = self.gun(1, 1, self.variant('grade_one', [effect, upgrade]))['card']
        without = self.gun(1, 1, self.variant('grade_none', [effect, upgrade], drop=[('F', 1, 'SlotBaseGrade', 11)]))
        self.assertEqual(without['card']['spread'], 1.0)
        self.assertEqual(without['card'], with_grade)


if __name__ == '__main__':
    EXE = str(Path(sys.argv.pop(1)).resolve())
    unittest.main()
