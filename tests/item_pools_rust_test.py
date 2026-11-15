"""Differential gate for the Rust item pool roll: `ow-package --item-pool` against tools/loot_pools.py's CrtRand path.

AI-assisted (Claude Code). Lane W4-POOLS. Builds an invented install (one package, named Startup.upk, the package
`--item-pool` reads by default) with pools below `.WeaponPools.`, pool lists and a pawn balance: formula, designer,
constant and conditional values, a disabled and an enabled range restriction, int and float weights in one pool, stage
gates by a resolver chain and by a resolver subobject, a gate with no value, a maximum gate, an empty and an
ineligible sub-pool, half-way quantities (2.5, 3.5), a zero and a negative quantity, a cycle, and a listed pool with an
unresolved probability. Then:

- `--item-pool --parity` (the Rust roll in Reference mode): every line must equal what `loot_pools.roll_crt` (a list)
  or `loot_pools.roll_pool_crt` (a pool) drops for the same pool, stage and seed, called as
  `tools/weapon_gen_parity.py` `pool_drops` calls them, on the C++ reader;
- `--item-pool <pawn>` and `--item-pool <pool>` (the single form, 20 seeds each, every parity stage): the same, for a
  pawn balance and for pools outside `.WeaponPools.`.

The comparison needs the oracle's rules (round-half-even quantities, every listed pool fired), which the Rust roll
reproduces only in Reference mode, so it runs as a `mode=reference` line (tests/rust_parity.d/pools.txt) or as ctest
`item-pools-rust`, which sets the same mode. A Rust line whose drops are objects means the binary ran in Off mode, and
the test fails saying so. Every name and number is invented. Run:
python tests/item_pools_rust_test.py <C++ ow-package> <Rust ow-package>
"""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'tools'))
import item_registry_test as fx  # noqa: E402
import loot_pools  # noqa: E402
import weapon_recipe  # noqa: E402

CXX = RUST = None
STARTUP = 'WillowGame/CookedPCConsole/Startup.upk'
STAGES = (1, 8, 15, 30, 50, 80)
SEEDS = 20
SOURCES = ('ItemPoolListDefinition', 'AIPawnBalanceDefinition')
W = 'WillowGame.'
IP = W + 'ItemPoolDefinition'
ATTR = 'Engine.AttributeDefinition'
CONSTANT = 'GearboxFramework.ConstantAttributeValueResolver'
FDEF = 'Engine.AttributeInitializationDefinition'
INIT = fx.Struct


def init(constant=0.0, attribute=None, definition=None):
    fields = [('BaseValueConstant', float(constant))]
    if attribute:
        fields.append(('BaseValueAttribute', fx.Ref(attribute)))
    if definition:
        fields.append(('InitializationDefinition', fx.Ref(definition)))
    return INIT('AttributeInitializationData', fields + [('BaseValueScaleConstant', 1.0)])


def formula(m, level, p, offset=None):
    fields = [('bEnabled', True), ('Multiplier', m), ('Level', level), ('Power', p)]
    if offset is not None:
        fields.append(('Offset', offset))
    return INIT('AttributeInitializationValueFormula', fields)


def entry(weight, pool=None, balance=None):
    fields = [('ItmPoolDefinition', fx.Ref(pool))] if pool else []
    fields += [('InvBalanceDefinition', fx.Ref(balance))] if balance else []
    return INIT('BalancedInventoryData', fields + [('Probability', weight)])


def items(*entries):
    return ('BalancedItems', fx.Array(entries))


def listed(pool, probability):
    return INIT('ItemPoolInfo', [('ItemPool', fx.Ref(pool)), ('PoolProbability', probability)])


def condition(value, operator):
    expression = INIT('AttributeExpressionData', [
        ('AttributeOperand1', fx.Ref(loot_pools.NUMBER_OF_PLAYERS)),
        ('ComparisonOperator', fx.Enum('EComparisonOperator', operator)), ('ConstantOperand2', 1.0)])
    return INIT('ConditionalExpression', [('BaseValueIfTrue', fx.const(value)), ('Expressions', fx.Array([expression]))])


def package():
    p = fx.Package()
    p.add('GD_PoolSyn.W.Common', FDEF, [('ValueFormula', formula(fx.const(100), fx.const(1), fx.const(1)))])
    p.add('GD_PoolSyn.W.Rare', FDEF, [('ValueFormula', formula(fx.const(1), fx.const(1), fx.const(1))),
                                      ('RangeRestriction', INIT('Restriction', [('MinValue', fx.const(100))]))])
    p.add('GD_PoolSyn.W.Mod', FDEF, [('ValueFormula', formula(init(0, attribute='GD_PoolSyn.D.CommonMod'),
                                                              init(0, definition='GD_PoolSyn.W.Common'), fx.const(1)))])
    p.add('GD_PoolSyn.D.CommonMod', W + 'DesignerAttributeDefinition', [('BaseValue', fx.const(1))])
    # 2 x (3^2 + 1) = 20, clamped to 7 by an enabled maximum.
    p.add('GD_PoolSyn.W.Capped', FDEF, [
        ('ValueFormula', formula(fx.const(2), fx.const(3), fx.const(2), fx.const(1))),
        ('RangeRestriction', INIT('Restriction', [('bEnableMaxValueRestriction', True), ('MaxValue', fx.const(7))]))])
    p.add('GD_PoolSyn.Odds', ATTR)
    p.add('GD_PoolSyn.Odds.Cond_0', 'GearboxFramework.ConditionalAttributeValueResolver', [
        ('ValueExpressions', INIT('ConditionalExpressionList', [('ConditionalExpressionList', fx.Array([
            condition(0.5, 'OPERATOR_EqualTo'), condition(0.25, 'OPERATOR_GreaterThan')]))]))])
    p.add('GD_PoolSyn.Stage9', ATTR)
    p.add('GD_PoolSyn.Stage9.Const_0', CONSTANT, [('ConstantValue', 9.0)])
    p.add('GD_PoolSyn.Stage20', ATTR, [('ValueResolverChain', fx.Array([fx.Ref('GD_PoolSyn.Stage20_Const')]))])
    p.add('GD_PoolSyn.Stage20_Const', CONSTANT, [('ConstantValue', 20.0)])
    p.add('GD_PoolSyn.Unknown', ATTR)
    for gun in 'ABCDLRX':
        p.add(f'GD_PoolSyn.Guns.{gun}', W + 'WeaponBalanceDefinition')
    pools = 'GD_PoolSyn.WeaponPools.'
    p.add(pools + 'Mixed', IP, [items(entry(fx.const(3), balance='GD_PoolSyn.Guns.A'),
                                      entry(fx.const(0.1), balance='GD_PoolSyn.Guns.B'),
                                      entry(fx.const(1), balance='GD_PoolSyn.Guns.C'),
                                      entry(fx.const(0.7), balance='GD_PoolSyn.Guns.D'))])
    p.add(pools + 'Late', IP, [('MinGameStageRequirement', fx.Ref('GD_PoolSyn.Stage9')),
                               items(entry(fx.const(1), balance='GD_PoolSyn.Guns.L'))])
    p.add(pools + 'Early', IP, [('MaxGameStageRequirement', fx.Ref('GD_PoolSyn.Stage20')),
                                ('Quantity', fx.const(2.5)),
                                items(entry(fx.const(1), balance='GD_PoolSyn.Guns.A'),
                                      entry(fx.const(2), balance='GD_PoolSyn.Guns.B'))])
    p.add(pools + 'Unknown', IP, [('MinGameStageRequirement', fx.Ref('GD_PoolSyn.Unknown')),
                                  items(entry(fx.const(1), balance='GD_PoolSyn.Guns.X'))])
    p.add(pools + 'Wrap', IP, [items(entry(fx.const(1), pool=pools + 'Late'))])
    p.add(pools + 'Empty', IP)
    p.add(pools + 'Outer', IP, [('Quantity', fx.const(3.5)), items(
        entry(init(0, definition='GD_PoolSyn.W.Mod'), pool=pools + 'Mixed'),
        entry(init(0, definition='GD_PoolSyn.W.Rare'), pool=pools + 'Wrap'),
        entry(fx.const(5), pool=pools + 'Empty'),
        entry(init(0, definition='GD_PoolSyn.W.Capped'), pool=pools + 'Early'),
        entry(init(0, attribute='GD_PoolSyn.Odds'), pool=pools + 'Unknown'))])
    p.add(pools + 'Zero', IP, [('Quantity', fx.const(0)), items(entry(fx.const(1), balance='GD_PoolSyn.Guns.R'))])
    p.add(pools + 'Negative', IP, [('Quantity', fx.const(-2)),
                                   items(entry(fx.const(1), balance='GD_PoolSyn.Guns.R'))])
    p.add(pools + 'CycA', IP, [items(entry(fx.const(1), pool=pools + 'CycB'),
                                     entry(fx.const(1), balance='GD_PoolSyn.Guns.C'))])
    p.add(pools + 'CycB', IP, [items(entry(fx.const(1), pool=pools + 'CycA'))])
    p.add('GD_PoolSyn.Lists.Main', W + 'ItemPoolListDefinition', [('ItemPools', fx.Array([
        listed(pools + 'Outer', init(0, attribute='GD_PoolSyn.Odds')),
        listed(pools + 'Late', fx.const(1)),
        listed(pools + 'Mixed', init(0, attribute='GD_PoolSyn.Unknown')),
        listed(pools + 'Early', fx.const(0.3))]))])
    p.add('GD_PoolSyn.Other.Cash', IP, [('Quantity', init(0, definition='GD_PoolSyn.W.Capped')),
                                        items(entry(fx.const(1), balance='GD_PoolSyn.Guns.R'))])
    p.add('GD_PoolSyn.Pawns.Bandit', W + 'AIPawnBalanceDefinition', [
        ('DefaultItemPoolList', fx.Array([listed(pools + 'Mixed', fx.const(0.5))])),
        ('DefaultItemPoolIncludedLists', fx.Array([fx.Ref('GD_PoolSyn.Lists.Main')])),
        ('PlayThroughs', fx.Array([INIT('PlayThroughData', [
            ('CustomItemPoolList', fx.Array([listed('GD_PoolSyn.Other.Cash', fx.const(1))]))])]))])
    return p


def lines(path):
    return [json.loads(line) for line in Path(path).read_text(encoding='utf-8').splitlines() if line.strip()]


class PoolsRustTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.folder = tempfile.TemporaryDirectory()
        root = Path(cls.folder.name)
        cls.game = (root / 'Game').absolute()
        package().save(cls.game / STARTUP)
        schema = root / 'loot_pools.schema'
        schema.write_text('\n'.join(loot_pools.SCHEMA_LINES) + '\n', encoding='utf-8')
        cls.package = weapon_recipe.Package(CXX, cls.game / STARTUP, str(schema.resolve()))
        cls.values = loot_pools.Values(cls.package)
        target = root / 'parity.jsonl'
        run = subprocess.run([RUST, '--item-pool', '--parity', str(target), '--cooked', str(cls.game)],
                             capture_output=True)
        assert run.returncode == 0, run.stderr.decode('utf-8', 'replace')
        cls.parity = lines(target)

    @classmethod
    def tearDownClass(cls):
        cls.folder.cleanup()

    def oracle(self, pool, stage, seed):
        """weapon_gen_parity.pool_drops."""
        rng = weapon_recipe.CrtRand(seed)
        if self.package.classes.get(pool, '').rsplit('.', 1)[-1] in SOURCES:
            loot = loot_pools.table(self.package, self.values, pool, stage)
            return [d['item'] for d in loot_pools.roll_crt(loot, rng)]
        tree = loot_pools.expand(self.package, self.values, pool, stage)
        return [d['item'] for d in loot_pools.roll_pool_crt(tree, rng)]

    def single(self, pool, stage):
        run = subprocess.run([RUST, '--item-pool', pool, '--cooked', str(self.game), '--stage', str(stage),
                              '--seed', '1', '--count', str(SEEDS)], capture_output=True)
        self.assertEqual(run.returncode, 0, run.stderr.decode('utf-8', 'replace'))
        return [json.loads(line) for line in run.stdout.decode('utf-8').splitlines()]

    def test_the_rust_ran_in_reference_mode(self):
        self.assertTrue(self.parity, 'no parity line')
        self.assertTrue(all(isinstance(d, str) for line in self.parity for d in line.get('drops', [None])),
                        'the Rust binary rolled in Off mode: run this test as a mode=reference line')

    def test_the_parity_batch_covers_the_fixture(self):
        pools = sorted(p for p, c in self.package.classes.items()
                       if ('ItemPoolDefinition' in c and '.WeaponPools.' in p) or c.endswith('ItemPoolListDefinition'))
        self.assertEqual(len(pools), 12)
        requests = [(p, s, seed) for p in pools for s in STAGES for seed in range(1, SEEDS + 1)]
        self.assertEqual([(line['pool'], line['stage'], line['seed']) for line in self.parity], requests)
        # Not a vacuous comparison: most requests drop something, and some drop nothing.
        dropping = sum(1 for line in self.parity if line['drops'])
        self.assertGreater(dropping, len(requests) // 2)
        self.assertLess(dropping, len(requests))

    def test_every_parity_roll_equals_the_oracle(self):
        for line in self.parity:
            with self.subTest(pool=line['pool'], stage=line['stage'], seed=line['seed']):
                self.assertEqual(line['drops'], self.oracle(line['pool'], line['stage'], line['seed']))

    def test_the_pawn_and_other_pools_equal_the_oracle(self):
        for pool in ('GD_PoolSyn.Pawns.Bandit', 'GD_PoolSyn.Other.Cash'):
            for stage in STAGES:
                got = self.single(pool, stage)
                self.assertEqual(len(got), SEEDS)
                for line in got:
                    with self.subTest(pool=pool, stage=stage, seed=line['seed']):
                        self.assertEqual((line['pool'], line['stage']), (pool, stage))
                        self.assertEqual(line['drops'], self.oracle(pool, stage, line['seed']))


if __name__ == '__main__':
    if len(sys.argv) < 3:
        sys.exit('usage: python tests/item_pools_rust_test.py <C++ ow-package> <Rust ow-package>')
    RUST = str(Path(sys.argv.pop(2)).resolve())
    CXX = str(Path(sys.argv.pop(1)).resolve())
    unittest.main()
