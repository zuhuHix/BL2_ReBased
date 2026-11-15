"""Differential gate for the Rust item card: `ow-package --item-card` against tools/weapon_stats.py and the shield rule.

AI-assisted (Claude Code). Lane W4-STATS, review r1 of PR #323. Builds an invented install (one package, named
Startup.upk) with a weapon balance and a mission weapon balance over one weapon type, parts and name parts that exercise
every card rule the comparison reads: a manufacturer operand, scale, pre-add and post-add effects, an integer
attribute, an activated internal slot, the part price total and a price constant, rarity bands, a spinning barrel with
its own firing mode, the zoom and ammo lines, a red flavour line and a card line, a status effect with a chance remap,
the accuracy pool with a part's external effect, and gestalt fragments with a static array. Then:

- the Rust `--item-card --requests` lines for the two balances at four stages and fifteen spread seeds must equal
  `tools/weapon_gen_parity.python_gun` (`weapon_recipe.roll`, `weapon_stats.evaluate`, `weapon_firing.accuracy_pool_of`)
  on the same package read with the C++ `ow-package`, field for field (`weapon_gen_parity.Diff`: floats bit for bit,
  the int or float kind checked), and echo the request's identity (`weapon_gen_parity.check_identity`);
- the Rust `--item-card --data` shield card must equal `tools/prepare_stock_shield.stats` bit for bit on the same
  package, and a shield whose recharge rate evaluates to 0 must be refused by both.

The comparison needs the reference stand-ins (architecture section 6.1), so it runs as a `mode=reference` line
(tests/rust_parity.d/stats.txt) or as ctest `item-stats-rust`, which sets the same mode. In Off mode the Rust card of a
balance answers `not_yet` (the Off roll needs the item's instance first), and the test fails saying so. Every name and
number is invented. Run: python tests/item_stats_rust_test.py <C++ ow-package> <Rust ow-package>
"""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'tools'))
import item_registry_test as fx  # noqa: E402
import item_roll_rust_test as rr  # noqa: E402
import loot_pools  # noqa: E402
import prepare_stock_shield as ps  # noqa: E402
import prepare_vendors as pv  # noqa: E402
import skill_stats  # noqa: E402
import weapon_firing  # noqa: E402
import weapon_gen_manifest  # noqa: E402
import weapon_gen_parity as P  # noqa: E402
import weapon_stats  # noqa: E402

CXX = RUST = None
W = 'WillowGame.'
STARTUP = 'WillowGame/CookedPCConsole/Startup.upk'
S = 'GD_StatsSyn.'
WEAPON_ATTR = 'D_Attributes.Weapon.'
RED = {'R': 171, 'G': 205, 'B': 239}
STAGES = (1, 8, 15, 30)
# Consecutive small seeds all draw near 0 first with the C runtime generator, so the seeds are spread
# (item_roll.spread_seeds).
SEEDS = [(i * 0x9E3779B1) % (1 << 32) for i in range(1, 16)]
BALANCES = (S + 'Bal.Weapon', S + 'Bal.Mission')
INIT = fx.Struct


class Names(list):
    """A static array of NameProperty elements, one tag per element with its array index."""


class Package(rr.Package):
    def value(self, name, value):
        if isinstance(value, Names):
            return b''.join(self.tag(name, 'NameProperty', self.fname(n), index=i) for i, n in value if n)
        return super().value(name, value)

    def body(self, fields):
        return b''.join(self.value(n, v) for n, v in fields) + self.fname('None')


def effect(attribute, kind, value):
    return INIT('AttributeEffectData', [('AttributeToModify', fx.Ref(attribute)),
                                        ('ModifierType', fx.Enum('EModifierType', kind)), ('BaseModifierValue', value)])


def of_attr(attribute, scale=1.0):
    return INIT('AttributeInitializationData', [('BaseValueAttribute', fx.Ref(attribute)),
                                                ('BaseValueScaleConstant', scale)])


def colour(c):
    return INIT('TestColor', [('R', c['R']), ('G', c['G']), ('B', c['B']), ('A', 255)])


def constant(p, path, value):
    p.add(path, 'Engine.AttributeDefinition')
    p.add(path + '.ConstantAttributeValueResolver_0', 'GearboxFramework.ConstantAttributeValueResolver',
          [('ConstantValue', value)])


def weapon(p):
    acme = S + 'Makers.Acme'
    p.add(acme, 'Engine.ManufacturerDefinition')
    constant(p, S + 'Price.Double', 2.0)
    constant(p, S + 'Price.Half', 1.5)
    p.add(S + 'Attr.Weapon_Is_Acme', 'Engine.AttributeDefinition')
    p.add(S + 'Names.Title', W + 'WeaponNamePartDefinition', [('PartName', rr.Str('Blaster'))])
    p.add(S + 'Names.Prefix', W + 'WeaponNamePartDefinition',
          [('PartName', rr.Str('Sharp')), ('MonetaryValueMod', fx.Ref(S + 'Price.Half'))])
    p.add(S + 'Pres.Red', W + 'AttributePresentationDefinition', [
        ('Description', rr.Str('Hot [shock]stuff[-shock]; now &amp; then')), ('TextColor', colour(RED))])
    p.add(S + 'Pres.Card', W + 'AttributePresentationDefinition',
          [('Description', rr.Str('A card line')), ('bDontDisplayNumber', True)])
    for field, path, rounding in (('damage', 'AttrPresent_WeaponDamage', 'ATTRROUNDING_IntCeil'),
                                  ('magazine', 'AttrPresent_WeaponClipSize', 'ATTRROUNDING_IntFloor'),
                                  ('fire_rate', 'AttrPresent_WeaponFireRate', 'ATTRROUNDING_Float'),
                                  ('reload_time', 'AttrPresent_WeaponReloadSpeed', 'ATTRROUNDING_Float'),
                                  ('weapon_zoom', 'AttrPresent_WeaponZoomEndFOV', 'ATTRROUNDING_Float')):
        p.add('GD_AttributePresentation.Weapons.' + path, W + 'AttributePresentationDefinition',
              [('RoundingMode', fx.Enum('EAttributeRounding', rounding))])
    p.add(weapon_stats.ACCURACY_PRESENTATION, W + 'AttributePresentationDefinition', [
        ('RoundingMode', fx.Enum('EAttributeRounding', 'ATTRROUNDING_Float')),
        ('RemappingData', INIT('AttributeRemapping', [('InputValueMx', fx.const(15)), ('OutputValueMn', fx.const(100))]))])
    p.add(weapon_stats.STATUS_CHANCE_MAPPING, W + 'WeaponStatusEffectAttributePresentationDefinition', [
        ('MappingData', fx.Array([INIT('WeaponStatusEffectPresentationMapping', [
            ('DamageType', fx.Ref(S + 'Dmg.Zap')), ('Presentation', fx.Ref(S + 'Pres.Zap'))])]))])
    p.add(S + 'Pres.Zap', W + 'AttributePresentationDefinition', [
        ('bValueRemappingEnabled', True),
        ('RemappingData', INIT('AttributeRemapping', [('InputValueMx', fx.const(3.33)), ('OutputValueMx', fx.const(100))]))])
    p.add(S + 'Dmg.Zap', W + 'WillowDamageTypeDefinition',
          [('StatusEffect', fx.Ref(S + 'Status.Zap')), ('DamageType', fx.Enum('EDamageType', 'DAMAGE_TYPE_Shock'))])
    p.add(S + 'Status.Zap', W + 'StatusEffectDefinition', [
        ('StatusEffectType', fx.Enum('EStatusEffectType', 'STATUS_EFFECT_Electrocute')),
        ('DamageSurfaceChanceModifiers', fx.Array([INIT('StatusEffectChanceModifier', [
            ('SurfaceType', fx.Enum('EDamageSurfaceType', 'DMGSURFACE_Generic')), ('BaseChance', fx.const(30))]),
            INIT('StatusEffectChanceModifier', [
                ('SurfaceType', fx.Enum('EDamageSurfaceType', 'DMGSURFACE_Armor')), ('BaseChance', fx.const(12))])])),
        ('bDoesDamageOverTime', True), ('BaseDuration', fx.const(4))])
    p.add(S + 'Firing.Mode', W + 'FiringModeDefinition', [('Speed', 1000.0)])
    p.add('GD_Globals.General.Globals', W + 'GlobalsDefinition', [('RarityLevelColors', fx.Array([
        INIT('RarityLevelColor', [('MinLevel', lo), ('MaxLevel', hi), ('RarityRating', fx.Enum('EItemRarity', r)),
                                  ('Color', colour(c))])
        for lo, hi, r, c in ((1, 2, 'RARITY_Common', {'R': 250, 'G': 250, 'B': 250}),
                             (3, 9, 'RARITY_Uncommon', {'R': 18, 'G': 52, 'B': 86}))]))])
    p.add(weapon_firing.ACCURACY_POOL, 'Engine.ResourcePoolDefinition',
          [('BaseMinValue', fx.const(2)), ('BaseMaxValue', fx.const(10))])
    zoom = WEAPON_ATTR + 'WeaponZoomEndFOV'
    p.add(S + 'Weapon.Type', W + 'WeaponTypeDefinition', [
        ('InstantHitDamage', fx.const(10)), ('FireRate', 0.4), ('ClipSize', 12.0), ('ReloadTime', 33.35),
        ('Spread', 1.5), ('ProjectilesPerShot', 1), ('StatusEffectDamage', fx.const(7)),
        ('BaseStatusEffectChanceModifier', fx.const(2)),
        ('WeaponAttributeEffects', fx.Array([
            effect(WEAPON_ATTR + 'WeaponDamage', 'MT_PostAdd', of_attr(S + 'Attr.Weapon_Is_Acme', 3.0)),
            effect(WEAPON_ATTR + 'WeaponShotCost', 'MT_PreAdd', fx.const(1))])),
        ('BaseRarity', fx.const(1)), ('MonetaryValue', of_attr(weapon_stats.PART_PRICE_TOTAL, 50.0)),
        ('BaseMonetaryValueModifier', 2.0), ('bUsesPlayerLevelRequirement', True),
        ('GestaltMesh', fx.Ref(S + 'Gestalt.Mesh')), ('DefaultDamageTypeDefinition', fx.Ref(S + 'Dmg.Zap')),
        ('ZoomedEndFOV', 40.0),
        ('WeaponCardAttributes', fx.Array([INIT('WeaponCardAttributeData', [('Attribute', fx.Ref(zoom))])])),
        ('WeaponCardPresentations', fx.Array([fx.Ref(S + 'Pres.Card')])),
        ('AttributeSlotEffects', fx.Array([INIT('AttributeSlotEffectData', [
            ('SlotName', rr.Name('Sp')), ('AttributeToModify', fx.Ref(WEAPON_ATTR + 'WeaponSpread')),
            ('ModifierType', fx.Enum('EModifierType', 'MT_PreAdd')), ('BaseModifierValue', fx.const(0)),
            ('PerGradeUpgrade', fx.const(0.25)), ('bEnforceMaximumGrade', True), ('MaximumGrade', 2)])]))])
    p.add(S + 'Weapon.B1', W + 'WeaponPartDefinition', [
        ('WeaponAttributeEffects', fx.Array([effect(WEAPON_ATTR + 'WeaponDamage', 'MT_Scale', fx.const(-0.2)),
                                             effect(WEAPON_ATTR + 'WeaponClipSize', 'MT_Scale', fx.const(-0.2))])),
        ('ExternalAttributeEffects', fx.Array([
            effect('D_Attributes.AccuracyResourcePool.AccuracyMinValue', 'MT_PreAdd', fx.const(0.5))])),
        ('Rarity', fx.const(1.9)), ('MonetaryValueMod', fx.Ref(S + 'Price.Double')),
        ('CustomPresentations', fx.Array([fx.Ref(S + 'Pres.Red')])),
        ('TitleList', fx.Array([fx.Ref(S + 'Names.Title')])),
        ('AttributeSlotUpgrades', fx.Array([INIT('AttributeSlotUpgradeData', [
            ('SlotName', rr.Name('Sp')), ('GradeIncrease', 3), ('bActivateSlot', True)])])),
        ('GestaltModeSkeletalMeshName', rr.Name('Frag_Body')),
        ('AdditionalGestaltModeSkeletalMeshNames', Names([(0, 'Frag_Var0'), (2, 'Frag_Var2')]))])
    p.add(S + 'Weapon.B2', W + 'WeaponPartDefinition', [('Rarity', fx.const(4))])
    p.add(S + 'Weapon.G1', W + 'WeaponPartDefinition', [
        ('WeaponAttributeEffects', fx.Array([effect(WEAPON_ATTR + 'WeaponDamage', 'MT_Scale', fx.const(0.5)),
                                             effect(WEAPON_ATTR + 'WeaponDamage', 'MT_PreAdd', fx.const(2))])),
        ('PrefixList', fx.Array([fx.Ref(S + 'Names.Prefix')])), ('GestaltModeSkeletalMeshName', rr.Name('Frag_Grip'))])
    p.add(S + 'Weapon.G2', W + 'WeaponPartDefinition')
    p.add(S + 'Weapon.R1', W + 'WeaponPartDefinition', [
        ('bIsSpinningEnabled', True), ('SpinUpDuration', fx.const(0.5)), ('StartingSpinUpFireIntervalMultiplier', 2.0),
        ('CustomFiringModeDefinition', fx.Ref(S + 'Firing.Mode'))])
    p.add(S + 'Weapon.R2', W + 'WeaponPartDefinition')
    p.add(S + 'Weapon.M1', W + 'WeaponPartDefinition', [('Material', fx.Ref(S + 'Mat.Skin'))])
    p.add(S + 'Weapon.Coll', W + 'WeaponPartListCollectionDefinition', [
        ('BodyPartData', fx.part_data([rr.weighted(S + 'Weapon.B1'), rr.weighted(S + 'Weapon.B2')])),
        ('GripPartData', fx.part_data([rr.weighted(S + 'Weapon.G1'), rr.weighted(S + 'Weapon.G2', low=2)])),
        ('BarrelPartData', fx.part_data([rr.weighted(S + 'Weapon.R1'), rr.weighted(S + 'Weapon.R2')])),
        ('MaterialPartData', fx.part_data([rr.weighted(S + 'Weapon.M1')])),
        rr.CONSOLIDATED])
    for balance, cls in zip(BALANCES, ('WeaponBalanceDefinition', 'MissionWeaponBalanceDefinition')):
        p.add(balance, W + cls, [
            ('InventoryDefinition', fx.Ref(S + 'Weapon.Type')),
            ('Manufacturers', fx.makers((acme, [rr.grade(1, 100)]))),
            ('WeaponPartListCollection', fx.Ref(S + 'Weapon.Coll'))])


CAP, RATE, DELAY = (ps.POOL + 'ShieldMaxValue', ps.POOL + 'ShieldOnIdleRegenerationRate',
                    ps.POOL + 'ShieldOnIdleRegenerationDelay')


def shield(p):
    """prepare_stock_shield_test's shield (capacity 10 x 2^level, delay 3, rate 5; two slots on the pool, one internal
    slot and one skill-run slot that do not reach it), and the same with a recharge rate of 0."""
    p.add(S + 'Attr.Level', 'Engine.AttributeDefinition', [('ValueResolverChain', fx.Array([fx.Ref(S + 'Attr.Level.Prop_0')]))])
    p.add(S + 'Attr.Level.Prop_0', 'Engine.ObjectPropertyAttributeValueResolver', [('PropertyName', rr.Name('ExpLevel'))])
    for name, value in (('Ten', 10.0), ('Two', 2.0), ('Delay', 3.0)):
        constant(p, S + 'Attr.' + name, value)
    p.add(S + 'Init.Cap', 'Engine.AttributeInitializationDefinition', [('ValueFormula', INIT('BalanceFormula', [
        ('bEnabled', True), ('Multiplier', of_attr(S + 'Attr.Ten')), ('Level', of_attr(S + 'Attr.Two')),
        ('Power', of_attr(S + 'Attr.Level'))]))])

    def upgrade(slot, increase, activate=False):
        return INIT('AttributeSlotUpgradeData', [('SlotName', rr.Name(slot)), ('GradeIncrease', increase),
                                                 ('bActivateSlot', activate)])

    def slot(name, attribute, per_grade, external=True, skill=False, base=0.0):
        return INIT('AttributeSlotEffectData', [
            ('SlotName', rr.Name(name)), ('AttributeToModify', fx.Ref(attribute)),
            ('ModifierType', fx.Enum('EModifierType', 'MT_Scale')), ('BaseModifierValue', fx.const(base)),
            ('PerGradeUpgrade', fx.const(per_grade)), ('bExternalSlot', external), ('bRunEffectsAsSkill', skill)])
    p.add(S + 'Shield.Up', W + 'ShieldPartDefinition',
          [('AttributeSlotUpgrades', fx.Array([upgrade('Capacity', 2), upgrade('RechargeDelay', -2)]))])
    p.add(S + 'Shield.Mat', W + 'ShieldPartDefinition')
    p.add(S + 'Shield.Title', W + 'ItemNamePartDefinition',
          [('PartName', rr.Str('Test Shield')), ('AttributeSlotUpgrades', fx.Array([upgrade('Capacity', 1)]))])
    for name, rate in (('Def', 5.0), ('Zero', 0.0)):
        p.add(S + 'Shield.' + name, W + 'ShieldDefinition', [
            ('ExternalAttributeEffects', fx.Array([
                effect(CAP, 'MT_PreAdd', INIT('AttributeInitializationData', [
                    ('InitializationDefinition', fx.Ref(S + 'Init.Cap')), ('BaseValueScaleConstant', 1.0)])),
                effect(DELAY, 'MT_PreAdd', of_attr(S + 'Attr.Delay')), effect(RATE, 'MT_PreAdd', fx.const(rate))])),
            ('AttributeSlotEffects', fx.Array([slot('Capacity', CAP, 0.1), slot('RechargeDelay', DELAY, 0.1),
                                               slot('Inside', CAP, 1.0, external=False, base=1.0),
                                               slot('AsSkill', CAP, 1.0, skill=True, base=1.0)])),
            ('AttributeSlotUpgrades', fx.Array([upgrade('Capacity', 0, True), upgrade('RechargeDelay', 0, True),
                                                upgrade('Inside', 0, True), upgrade('AsSkill', 0, True)]))])


def package():
    p = Package()
    weapon(p)
    shield(p)
    return p


class StatsRustTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.folder = tempfile.TemporaryDirectory()
        root = Path(cls.folder.name)
        cls.root = root
        cls.game = (root / 'Game').absolute()
        package().save(cls.game / STARTUP)
        # The red flavour colour for both sides: Python reads OPENWILLOW_GAME_COLOURS, Rust --colours.
        cls.colours = root / 'game_colours.json'
        cls.colours.write_text(json.dumps({'schemaVersion': 1, 'colours': {
            'card_red_text': '#%02X%02X%02X' % (RED['R'], RED['G'], RED['B'])}}), encoding='utf-8')
        os.environ['OPENWILLOW_GAME_COLOURS'] = str(cls.colours)
        schema = root / 'stats.schema'
        schema.write_text('\n'.join(dict.fromkeys(weapon_stats.SCHEMA_LINES + loot_pools.SCHEMA_LINES
                                                  + weapon_firing.POOL_SCHEMA_LINES)) + '\n', encoding='utf-8')
        cls.package = skill_stats.Package(CXX, cls.game / STARTUP, str(schema.resolve()))
        cls.localize = staticmethod(weapon_gen_manifest.make_localize(cls.package, cls.game))
        cls.requests = [(b, stage, seed) for b in BALANCES for stage in STAGES for seed in SEEDS]
        batch = root / 'requests.txt'
        batch.write_text(''.join(f'balance {b} {s} {seed}\n' for b, s, seed in cls.requests), encoding='utf-8')
        out = root / 'guns.jsonl'
        cls.rust_run = subprocess.run([RUST, '--item-card', '--requests', str(batch), '--output', str(out),
                                       '--cooked', str(cls.game), '--colours', str(cls.colours)], capture_output=True)
        assert cls.rust_run.returncode == 0, cls.rust_run.stderr.decode('utf-8', 'replace')
        cls.guns = [json.loads(line) for line in out.read_text(encoding='utf-8').splitlines() if line]
        sschema = root / 'shield.schema'
        sschema.write_text('\n'.join(pv.SCHEMA_EXTRA + loot_pools.SCHEMA_LINES + ps.SCHEMA_EXTRA) + '\n',
                           encoding='utf-8')
        from prepare_slice_world import Reader
        cls.chain = pv.Chain(Reader(CXX, cls.game, str(sschema.resolve())), ['Startup'])

    @classmethod
    def tearDownClass(cls):
        cls.folder.cleanup()

    def shield_card(self, definition):
        data = self.root / 'shield.json'
        data.write_text(json.dumps({'kind': 'shield', 'definition': definition, 'game_stage': 2, 'parts': {
            'Alpha': S + 'Shield.Up', 'Material': S + 'Shield.Mat'}, 'title': S + 'Shield.Title'}), encoding='utf-8')
        return subprocess.run([RUST, '--item-card', '--data', str(data), '--cooked', str(self.game)], capture_output=True)

    def test_the_rust_ran_in_reference_mode(self):
        self.assertEqual(len(self.guns), len(self.requests))
        # A failed evaluation is its own line (`error`): report it as that, not as a mode problem.
        errors = [g for g in self.guns if 'error' in g]
        self.assertEqual(errors[:3], [], 'the Rust card failed on a request')
        off = [g for g in self.guns if 'card' not in g]
        self.assertEqual(off[:3], [], 'the Rust card answered without a gun: run this test as a mode=reference line')

    def test_every_rust_gun_equals_the_python_card(self):
        diff, errors = P.Diff(), []
        for (balance, stage, seed), rust in zip(self.requests, self.guns):
            where = f'{balance} stage {stage} seed {seed}'
            try:
                ours = P.python_gun(self.package, self.localize, balance, stage, seed)
            except Exception as error:  # noqa: BLE001 - a Python failure is reported, not hidden
                errors.append((where, f'{type(error).__name__}: {error}'))
                continue
            P.check_identity(diff, where, rust, P.expected_identity(balance, stage, seed, None))
            diff.compare(where, 'gun', ours, rust)
        self.assertEqual(errors, [])
        self.assertEqual(diff.mismatches, 0, json.dumps(diff.examples[:5], default=str))
        # Every gun compares its whole card (about 85 leaves in this fixture), so a missing block shows here.
        self.assertGreater(diff.fields, 80 * len(self.requests))

    def test_the_fixture_reaches_every_rule(self):
        cards = [g['card'] for g in self.guns if 'card' in g]
        self.assertTrue(any(c['status_chance'] and c['status_dps'] for c in cards))
        self.assertTrue(any(c['spin_up'] for c in cards))
        self.assertTrue(any(c['weapon_zoom'] for c in cards))
        self.assertTrue(any(c['accuracy_pool'] and c['accuracy_pool']['min'] != 2.0 for c in cards))
        self.assertTrue(any('Consumes 2' in (c['fun_stats'] or '') for c in cards))
        self.assertTrue(any(c['level_requirement'] == 0 for c in cards))
        self.assertTrue(any(c['level_line'] for c in cards))
        self.assertTrue(any(g['gestalt_fragments'] == ['Frag_Body', 'Frag_Grip', 'Frag_Var0', 'Frag_Var2']
                            for g in self.guns))
        self.assertEqual({c['rarity'] for c in cards}, {1, 2})

    def test_the_shield_card_equals_prepare_stock_shield(self):
        run = self.shield_card(S + 'Shield.Def')
        self.assertEqual(run.returncode, 0, run.stderr.decode('utf-8', 'replace'))
        rust = {row['label']: row for row in json.loads(run.stdout)['stats']}
        names = (None, S + 'Shield.Title')
        parts = {'Alpha': S + 'Shield.Up', 'Material': S + 'Shield.Mat'}
        ours, _ = ps.stats(self.chain, S + 'Shield.Def', parts, names, 2)
        self.assertEqual({label: rust[label]['raw'] for label in ours}, ours)
        self.assertEqual(rust['Capacity']['value'], '52')

    def test_a_shield_that_evaluates_to_zero_is_refused_by_both(self):
        run = self.shield_card(S + 'Shield.Zero')
        self.assertEqual(run.returncode, 1)
        self.assertIn('must be above 0', run.stderr.decode('utf-8', 'replace'))
        with self.assertRaises(ps.RefusedError):
            ps.stats(self.chain, S + 'Shield.Zero', {'Alpha': S + 'Shield.Up', 'Material': S + 'Shield.Mat'},
                     (None, S + 'Shield.Title'), 2)


if __name__ == '__main__':
    if len(sys.argv) < 3:
        sys.exit('usage: python tests/item_stats_rust_test.py <C++ ow-package> <Rust ow-package>')
    RUST = str(Path(sys.argv.pop(2)).resolve())
    CXX = str(Path(sys.argv.pop(1)).resolve())
    unittest.main()
