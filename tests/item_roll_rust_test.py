"""Differential gate for the Rust item roll: `ow-package --item-roll` against tools/item_roll.py and tools/weapon_recipe.py.

AI-assisted (Claude Code). Lane W4-ROLL, review r1 of PR #319. Builds an invented install (one package, named
Startup.upk so that `--item-roll --weapons` reads its weapon balances) with shields, grenade mods (two manufacturers,
a manufacturer-only weight, a weight and a stage window that need runtime state), a balance that does not interpolate
its level, a class mod with two definitions, a relic whose hook removes a part, a balance with no grade at the low
stages, and a weapon (a manufacturer name expression, a slot whose every candidate weighs 0, a weight that needs
runtime state). Then:

- `--item-roll --census` (the Rust roll in Reference mode): every line must pass `item_roll.check` against the tool's
  registry and census of the same install read with the C++ `ow-package`, and equal `item_roll.roll` for the same
  balance, stage and seed, field for field (a not-rollable line: the same reason);
- `--item-roll --weapons`: every gun must equal `weapon_recipe.roll` on the same package (C++ reader), field for field.

The comparison needs the reference stand-ins (architecture section 6.1), so it runs as a `mode=reference` line
(tests/rust_parity.d/roll.txt) or as ctest `item-roll-rust`, which sets the same mode. A Rust weapon line not in
`weapon_recipe.roll`'s shape means the binary ran in Off mode, and the test fails saying so. Every name and number is
invented. Run: python tests/item_roll_rust_test.py <C++ ow-package> <Rust ow-package>
"""
import json
import subprocess
import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'tools'))
import item_registry as ir  # noqa: E402
import item_registry_test as fx  # noqa: E402
import item_roll  # noqa: E402
import weapon_recipe  # noqa: E402

CXX = RUST = None
W = 'WillowGame.'
STARTUP = 'WillowGame/CookedPCConsole/Startup.upk'
ITEM_FIELDS = ('kind', 'stage', 'level', 'manufacturer', 'grade', 'definition', 'parts', 'name_parts', 'hook_removed',
               'notes')
RARITY_FIELDS = ('level', 'band', 'unresolved', 'name_parts_counted')
WEAPON_FIELDS = ('manufacturer', 'game_stage', 'weapon_type', 'name', 'title', 'prefix', 'parts', 'notes')


class Str(str):
    """A StrProperty value."""


class Name(str):
    """A NameProperty value."""


class Package(fx.Package):
    """item_registry_test's package, with string and name properties."""

    def value(self, name, value):
        if isinstance(value, Str):
            return self.tag(name, 'StrProperty', fx.ints(len(value) + 1) + value.encode('ascii') + b'\0')
        if isinstance(value, Name):
            return self.tag(name, 'NameProperty', self.fname(str(value)))
        return super().value(name, value)


ACME, BOLT = 'GD_RollSyn.Makers.Acme', 'GD_RollSyn.Makers.Bolt'
MIN_STAGE = 'GD_RollSyn.Attr.MinStage'
INIT = fx.Struct
# Consolidated data: 0 -> 1, 1 -> 100, 2 -> 5, 3 -> 30, 4 -> 0, 5 -> an attribute base (runtime state for the oracles).
CONSOLIDATED = ('ConsolidatedAttributeInitData', fx.Array([
    fx.const(1), fx.const(100), fx.const(5), fx.const(30), fx.const(0),
    INIT('AttributeInitializationData', [('BaseValueAttribute', fx.Ref(MIN_STAGE)), ('BaseValueScaleConstant', 1.0)])]))


def weighted(part, low=-1, high=-1, makers=(), default=None):
    return fx.weighted(part, low, high, default, makers)


def item_list(*rows):
    return [('WeightedParts', fx.Array(rows)), CONSOLIDATED]


def grade(low, high, level=1, definition=None, spawn=None):
    fields = [('GradeModifiers', INIT('Mods', [('ExpLevel', level), ('CustomInventoryDefinition', fx.Ref(definition))])),
              ('GameStageRequirement', INIT('Need', [('MinGameStage', low), ('MaxGameStage', high)]))]
    if spawn:
        fields += [('MinSpawnProbabilityModifier', fx.const(spawn[0])),
                   ('MaxSpawnProbabilityModifier', fx.const(spawn[1]))]
    return INIT('GradeData', fields)


def slots(*names):
    return fx.Array(INIT('Slot', [('SlotName', Name(n))]) for n in names)


def rarity(value):
    return ('Rarity', fx.const(value))


def package():
    p = Package()
    ib = W + 'InventoryBalanceDefinition'
    lst = W + 'ItemPartListDefinition'
    p.add(ACME, 'Engine.ManufacturerDefinition')
    p.add(BOLT, 'Engine.ManufacturerDefinition')

    def band(low, high, rating):
        return INIT('RarityLevelColor', [('MinLevel', low), ('MaxLevel', high),
                                         ('RarityRating', fx.Enum('EItemRarity', rating))])
    p.add('GD_Globals.General.Globals', W + 'GlobalsDefinition', [('RarityLevelColors', fx.Array([
        band(0, 0, 'RARITY_Unknown'), band(1, 3, 'RARITY_Common'), band(4, 6, 'RARITY_Uncommon'),
        band(7, 20, 'RARITY_Rare')]))])
    p.add(MIN_STAGE, 'Engine.AttributeDefinition',
          [('ValueResolverChain', fx.Array([fx.Ref(MIN_STAGE + '_Constant')]))])
    p.add(MIN_STAGE + '_Constant', 'GearboxFramework.ConstantAttributeValueResolver', [('ConstantValue', 20.0)])
    p.add('GD_RollSyn.Names.Title_Shield', W + 'ItemNamePartDefinition',
          [('PartName', Str('Bulwark')), ('Priority', 2.0), rarity(1)])
    p.add('GD_RollSyn.Names.Prefix_Low', W + 'ItemNamePartDefinition',
          [('PartName', Str('Low')), ('MaxExpLevelRequirement', 10)])

    # Shield.
    p.add('GD_RollSyn.Shield.A1', W + 'ShieldPartDefinition', [rarity(1)])
    p.add('GD_RollSyn.Shield.A2', W + 'ShieldPartDefinition', [rarity(2)])
    p.add('GD_RollSyn.Shield.M1', W + 'ShieldPartDefinition',
          [rarity(2), ('TitleList', fx.Array([fx.Ref('GD_RollSyn.Names.Title_Shield')]))])
    p.add('GD_RollSyn.Shield.List_A', lst, item_list(weighted('GD_RollSyn.Shield.A1'), weighted('GD_RollSyn.Shield.A2')))
    p.add('GD_RollSyn.Shield.List_M', lst, item_list(weighted('GD_RollSyn.Shield.M1')))
    p.add('GD_RollSyn.Shield.Def', W + 'ShieldDefinition', [
        ('AlphaParts', fx.Ref('GD_RollSyn.Shield.List_A')), ('MaterialParts', fx.Ref('GD_RollSyn.Shield.List_M')),
        ('BaseRarity', fx.const(1)), ('PrefixList', fx.Array([fx.Ref('GD_RollSyn.Names.Prefix_Low')]))])
    p.add('GD_RollSyn.Bal.Shield', ib, [('InventoryDefinition', fx.Ref('GD_RollSyn.Shield.Def')),
                                        ('Manufacturers', fx.makers((ACME, [grade(1, 100)])))])
    p.add('GD_RollSyn.Bal.Late', ib, [('InventoryDefinition', fx.Ref('GD_RollSyn.Shield.Def')),
                                      ('Manufacturers', fx.makers((ACME, [grade(20, 100)])))])

    # Grenade mods.
    for name in ('D1', 'D2', 'D3', 'E1', 'E2'):
        p.add(f'GD_RollSyn.Grenade.{name}', W + 'GrenadeModPartDefinition')
    p.add('GD_RollSyn.Grenade.List_D', lst, item_list(
        weighted('GD_RollSyn.Grenade.D1', makers=[(ACME, 2)], default=4), weighted('GD_RollSyn.Grenade.D2'),
        weighted('GD_RollSyn.Grenade.D3', low=5)))
    p.add('GD_RollSyn.Grenade.List_E', lst, item_list(
        weighted('GD_RollSyn.Grenade.E1', makers=[(BOLT, 5)], default=1), weighted('GD_RollSyn.Grenade.E2')))
    p.add('GD_RollSyn.Grenade.Def', W + 'GrenadeModDefinition', [
        ('DeltaParts', fx.Ref('GD_RollSyn.Grenade.List_D')), ('EpsilonParts', fx.Ref('GD_RollSyn.Grenade.List_E')),
        ('BaseRarity', fx.const(5))])
    p.add('GD_RollSyn.Bal.Grenade', ib, [
        ('InventoryDefinition', fx.Ref('GD_RollSyn.Grenade.Def')),
        ('Manufacturers', fx.makers((ACME, [grade(1, 100)]), (BOLT, [grade(1, 11, spawn=(0.5, 1.0))])))])
    p.add('GD_RollSyn.Grenade.Def_Fixed', W + 'GrenadeModDefinition',
          [('DeltaParts', fx.Ref('GD_RollSyn.Grenade.List_D'))])
    p.add('GD_RollSyn.Bal.Fixed', ib, [
        ('InventoryDefinition', fx.Ref('GD_RollSyn.Grenade.Def')), ('bInterpolateExpLevel', False),
        ('Manufacturers', fx.makers((ACME, [grade(1, 100, 12, 'GD_RollSyn.Grenade.Def_Fixed')])))])

    # Class mod.
    p.add('GD_RollSyn.ClassMod.A', W + 'ClassModPartDefinition', [rarity(3)])
    p.add('GD_RollSyn.ClassMod.List_A', lst, item_list(weighted('GD_RollSyn.ClassMod.A')))
    for d in ('Def1', 'Def2'):
        p.add(f'GD_RollSyn.ClassMod.{d}', W + 'ClassModDefinition', [('AlphaParts', fx.Ref('GD_RollSyn.ClassMod.List_A'))])
    p.add('GD_RollSyn.Bal.ClassMod', W + 'ClassModBalanceDefinition', [
        ('ClassModDefinitions', fx.Array([fx.Ref('GD_RollSyn.ClassMod.Def1'), fx.Ref('GD_RollSyn.ClassMod.Def2')])),
        ('Manufacturers', fx.makers((ACME, [grade(1, 100)])))])

    # Relic: Effect1 defined; E1 upgrades Effect1 (kept), E2 Effect2 (removed), E3 nothing (kept).
    p.add('GD_RollSyn.Relic.E1', W + 'ArtifactPartDefinition', [('AttributeSlotUpgrades', slots('Effect1'))])
    p.add('GD_RollSyn.Relic.E2', W + 'ArtifactPartDefinition', [('AttributeSlotUpgrades', slots('Effect2'))])
    p.add('GD_RollSyn.Relic.E3', W + 'ArtifactPartDefinition')
    p.add('GD_RollSyn.Relic.List_A', lst, item_list(
        weighted('GD_RollSyn.Relic.E1'), weighted('GD_RollSyn.Relic.E2'), weighted('GD_RollSyn.Relic.E3')))
    p.add('GD_RollSyn.Relic.Def', W + 'ArtifactDefinition', [
        ('AlphaParts', fx.Ref('GD_RollSyn.Relic.List_A')), ('AttributeSlotEffects', slots('Effect1'))])
    p.add('GD_RollSyn.Bal.Relic', ib, [('InventoryDefinition', fx.Ref('GD_RollSyn.Relic.Def')),
                                       ('Manufacturers', fx.makers((ACME, [grade(1, 100)])))])

    # Weapon.
    p.add('GD_RollSyn.Names.Title_Acme', W + 'WeaponNamePartDefinition', [
        ('PartName', Str('Acme Special')),
        ('Expressions', fx.Array([INIT('AttributeExpressionData', [
            ('AttributeOperand1', fx.Ref('GD_RollSyn.Attr.Weapon_Is_Acme')), ('ConstantOperand2', 1.0)])]))])
    for name in ('B1', 'E1', 'X1', 'X2'):
        p.add(f'GD_RollSyn.Weapon.{name}', W + 'WeaponPartDefinition')
    p.add('GD_RollSyn.Weapon.B2', W + 'WeaponPartDefinition',
          [('TitleList', fx.Array([fx.Ref('GD_RollSyn.Names.Title_Acme')]))])
    p.add('GD_RollSyn.Weapon.Coll', W + 'WeaponPartListCollectionDefinition', [
        ('BodyPartData', fx.part_data([weighted('GD_RollSyn.Weapon.B1'), weighted('GD_RollSyn.Weapon.B2')])),
        ('ElementalPartData', fx.part_data([weighted('GD_RollSyn.Weapon.E1', makers=[(BOLT, 1)], default=4)])),
        ('Accessory1PartData', fx.part_data([weighted('GD_RollSyn.Weapon.X1', makers=[(ACME, 5)]),
                                             weighted('GD_RollSyn.Weapon.X2')])),
        CONSOLIDATED])
    p.add('GD_RollSyn.Weapon.Type', W + 'WeaponTypeDefinition')
    p.add('GD_RollSyn.Bal.Weapon', W + 'WeaponBalanceDefinition', [
        ('InventoryDefinition', fx.Ref('GD_RollSyn.Weapon.Type')),
        ('Manufacturers', fx.makers((ACME, [grade(1, 100)]))),
        ('WeaponPartListCollection', fx.Ref('GD_RollSyn.Weapon.Coll'))])
    return p


def lines(path):
    return [json.loads(line) for line in Path(path).read_text(encoding='utf-8').splitlines() if line]


class RollRustTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.folder = tempfile.TemporaryDirectory()
        root = Path(cls.folder.name)
        cls.game = (root / 'Game').absolute()
        package().save(cls.game / STARTUP)
        out = root / 'tool'
        ir.run(CXX, cls.game, out, jobs=2, checks=False)
        cls.registry = json.loads((out / 'registry.json').read_text(encoding='utf-8'))
        schema = out / 'item_roll.schema'
        schema.write_text('\n'.join(item_roll.schema_lines()) + '\n', encoding='utf-8')
        cls.census, _, errors = ir.open_census(CXX, cls.game, schema.resolve(), 2)
        assert not errors, errors
        wschema = out / 'weapon_recipe.schema'
        wschema.write_text('\n'.join(weapon_recipe.SCHEMA_LINES) + '\n', encoding='utf-8')
        cls.package = weapon_recipe.Package(CXX, cls.game / STARTUP, str(wschema.resolve()))
        cls.items = cls.rust(root, '--census')
        cls.weapons = cls.rust(root, '--weapons')

    @classmethod
    def rust(cls, root, flag):
        target = root / (flag.strip('-') + '.jsonl')
        run = subprocess.run([RUST, '--item-roll', flag, str(target), '--cooked', str(cls.game)], capture_output=True)
        assert run.returncode == 0, run.stderr.decode('utf-8', 'replace')
        return lines(target)

    @classmethod
    def tearDownClass(cls):
        cls.folder.cleanup()

    def test_the_rust_ran_in_reference_mode(self):
        self.assertTrue(self.weapons, 'no weapon line')
        self.assertTrue(all('game_stage' in w for w in self.weapons),
                        'the Rust binary rolled in Off mode: run this test as a mode=reference line')

    def test_the_census_covers_the_fixture(self):
        kinds = Counter(i.get('kind', 'not_rollable:' + i.get('not_rollable', '?')) for i in self.items)
        # 6 item balances x 6 stages x 20 seeds; the late balance has no grade at stages 1, 8 and 15.
        self.assertEqual(sum(kinds.values()), 6 * 6 * 20)
        self.assertEqual(kinds['not_rollable:no_drawable_grade_at_stage'], 3 * 20)
        self.assertEqual(set(kinds) - {'not_rollable:no_drawable_grade_at_stage'},
                         {'shield', 'grenade_mod', 'class_mod', 'relic'})
        self.assertTrue(any(i.get('hook_removed') for i in self.items))
        self.assertTrue(any(i.get('notes') for i in self.items))
        self.assertEqual(len(self.weapons), 6 * 20)
        self.assertTrue(any(w['notes'] for w in self.weapons))

    def test_every_rust_item_roll_passes_the_check_and_equals_the_tool(self):
        failures, differing = [], []
        for ours in self.items:
            balance, stage, seed = ours['balance'], ours['stage'], ours['seed']
            try:
                theirs = item_roll.roll(self.registry, balance, stage, seed, self.census)
            except item_roll.NotRollable as error:
                if ours.get('not_rollable') != error.reason:
                    differing.append((balance, stage, seed, 'not_rollable'))
                continue
            if 'not_rollable' in ours:
                differing.append((balance, stage, seed, 'not_rollable'))
                continue
            problems = item_roll.check(self.registry, ours, self.census)
            if problems:
                failures.append((balance, stage, seed, problems))
            bad = [f for f in ITEM_FIELDS if ours.get(f) != theirs.get(f)]
            bad += ['rarity.' + f for f in RARITY_FIELDS if ours['rarity'].get(f) != theirs['rarity'].get(f)]
            if bad:
                differing.append((balance, stage, seed, bad))
        self.assertEqual(failures, [])
        self.assertEqual(differing, [])

    def test_every_rust_gun_equals_weapon_recipe(self):
        differing = []
        for ours in self.weapons:
            theirs = weapon_recipe.roll(self.package, ours['balance'], ours['seed'], ours['game_stage'],
                                        rng=weapon_recipe.CrtRand(ours['seed']))
            bad = [f for f in WEAPON_FIELDS if ours.get(f) != theirs.get(f)]
            if bad:
                differing.append((ours['balance'], ours['game_stage'], ours['seed'], bad))
        self.assertEqual(differing, [])


if __name__ == '__main__':
    if len(sys.argv) < 3:
        sys.exit('usage: python tests/item_roll_rust_test.py <C++ ow-package> <Rust ow-package>')
    RUST = str(Path(sys.argv.pop(2)).resolve())
    CXX = str(Path(sys.argv.pop(1)).resolve())
    unittest.main()
