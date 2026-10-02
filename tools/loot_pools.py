"""Expand BL2 item pools into weighted drop tables and roll them from a seed.

Sources, all decoded from the installed packages with `ow-package --properties`:
- AIPawnBalanceDefinition: DefaultItemPoolList, DefaultItemPoolIncludedLists and
  the first PlayThroughs entry's CustomItemPoolList / CustomItemPoolIncludedLists.
- ItemPoolListDefinition: ItemPools[] {ItemPool, PoolProbability}.
- ItemPoolDefinition (and KeyedItemPoolDefinition): BalancedItems[]
  {ItmPoolDefinition | InvBalanceDefinition, Probability}, Quantity,
  MinGameStageRequirement / MaxGameStageRequirement.

Weights are AttributeInitializationData. They are evaluated here (Values): constants,
ValueFormula Multiplier * (Level ^ Power + Offset) with enabled range restrictions,
ConstantAttributeValueResolver and ConditionalAttributeValueResolver attributes
(conditions evaluated against a small context: NumberOfPlayers = 1 by default) and
DesignerAttributeDefinition BaseValue. Anything else is reported as unresolved and
weighs nothing, so it never silently enters a roll.

Oracle: every cooked BalancedItems entry carries ProbabilityDisplayString, the share
the editor computed when the pool was saved. `check` recomputes every share in an
editor-like context (designer attributes 0, NumberOfPlayers 1) and counts agreement.

UNVERIFIED (ItemPool.SpawnBalancedInventoryFromPool is native code):
- a list entry fires independently with probability min(1, PoolProbability);
- a pool picks Quantity entries, each by weight among its eligible entries;
- a sub-pool is expanded the same way; game-stage requirements gate a whole pool.
Nothing game-derived is written into the repository; tables go under local/.
"""
import argparse
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import weapon_recipe  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_LINES = weapon_recipe.SCHEMA_LINES + [
    'BalancedItems=StructProperty:BalancedInventoryData',
    'ItemPools=StructProperty:ItemPoolInfo',
    'DefaultItemPoolList=StructProperty:ItemPoolInfo',
    'DefaultItemPoolIncludedLists=ObjectProperty',
    'CustomItemPoolList=StructProperty:ItemPoolInfo',
    'CustomItemPoolIncludedLists=ObjectProperty',
    'PlayThroughs=StructProperty:PlayThroughData',
    'ConditionalExpressionList=StructProperty:ConditionalExpression',
    'ValueResolverChain=ObjectProperty',
]
NUMBER_OF_PLAYERS = 'D_Attributes.GameProperties.NumberOfPlayers'
COMPARE = {
    'OPERATOR_EqualTo': lambda a, b: a == b, 'OPERATOR_NotEqualTo': lambda a, b: a != b,
    'OPERATOR_GreaterThan': lambda a, b: a > b, 'OPERATOR_GreaterThanOrEqualTo': lambda a, b: a >= b,
    'OPERATOR_LessThan': lambda a, b: a < b, 'OPERATOR_LessThanOrEqualTo': lambda a, b: a <= b,
}


class Values:
    """AttributeInitializationData -> float, or None (recorded in `unresolved`)."""

    def __init__(self, package, context=None, editor=False):
        self.package = package
        self.context = {NUMBER_OF_PLAYERS: 1.0, **(context or {})}
        self.editor = editor  # editor-like: designer attributes read 0 (see module docstring)
        self.unresolved = set()

    def init(self, data):
        data = data or {}
        definition, attribute = data.get('InitializationDefinition'), data.get('BaseValueAttribute')
        if definition:
            base = self.definition(definition)
        elif attribute:
            base = self.attribute(attribute)
        else:
            base = data.get('BaseValueConstant') or 0.0
        if base is None:
            return None
        scale = data.get('BaseValueScaleConstant')
        return base * (1.0 if scale is None else scale)

    def definition(self, path):
        props = self.package.props(path)
        if (props.get('BaseValueMode') or 'BASEVALUE_InitializationDefSetsBaseValue') != \
                'BASEVALUE_InitializationDefSetsBaseValue' or (props.get('ConditionalInitialization') or {}).get('bEnabled'):
            self.unresolved.add(path)
            return None
        formula = props.get('ValueFormula') or {}
        if not formula.get('bEnabled'):
            self.unresolved.add(path)
            return None
        terms = {k: self.init(formula.get(k)) for k in ('Multiplier', 'Level', 'Power', 'Offset')}
        if None in terms.values():
            return None
        value = weapon_recipe.formula_value(terms['Multiplier'], terms['Level'], terms['Power'], terms['Offset'])
        clamp = props.get('RangeRestriction') or {}
        if clamp.get('bEnableMinValueRestriction'):
            low = self.init(clamp.get('MinValue'))
            value = value if low is None else max(value, low)
        if clamp.get('bEnableMaxValueRestriction'):
            high = self.init(clamp.get('MaxValue'))
            value = value if high is None else min(value, high)
        return value

    def attribute(self, path):
        if path in self.context:
            return self.context[path]
        cls = self.package.classes.get(path, '')
        if cls.endswith('DesignerAttributeDefinition'):
            # The runtime may set a designer attribute; its default is BaseValue.
            return 0.0 if self.editor else self.init(self.package.props(path).get('BaseValue'))
        for resolver in self.package.props(path).get('ValueResolverChain') or self.subobjects(path):
            kind = self.package.classes.get(resolver, '')
            props = self.package.props(resolver)
            if kind.endswith('.ConstantAttributeValueResolver'):
                return props.get('ConstantValue')
            if kind.endswith('.ConditionalAttributeValueResolver'):
                value = self.conditional(props.get('ValueExpressions') or {})
                if value is not None:
                    return value
        self.unresolved.add(path)
        return None

    def subobjects(self, path):
        # Cooked attributes often omit ValueResolverChain; their resolvers are subobjects.
        if not hasattr(self, '_resolvers'):
            self._resolvers = {}
            for p, cls in self.package.classes.items():
                if 'ValueResolver' in cls:
                    self._resolvers.setdefault(p.rpartition('.')[0], []).append(p)
        return self._resolvers.get(path, [])

    def conditional(self, expressions):
        for case in expressions.get('ConditionalExpressionList') or []:
            verdicts = [self.expression(e) for e in case.get('Expressions') or []]
            if None in verdicts:
                return None
            if all(verdicts):
                return self.init(case.get('BaseValueIfTrue'))
        default = expressions.get('DefaultBaseValue')
        return self.init(default) if default else None

    def expression(self, expression):
        left = self.attribute(expression.get('AttributeOperand1')) if expression.get('AttributeOperand1') else None
        if expression.get('AttributeOperand2'):
            right = self.attribute(expression['AttributeOperand2'])
        else:
            right = expression.get('ConstantOperand2') or 0.0
        test = COMPARE.get(expression.get('ComparisonOperator'))
        if left is None or right is None or test is None:
            return None
        return test(left, right)


def pool_entries(package, values, pool):
    """[{kind: 'pool'|'item', path, weight, display}] for an ItemPoolDefinition."""
    result = []
    for entry in package.props(pool).get('BalancedItems') or []:
        child = entry.get('ItmPoolDefinition') or entry.get('InvBalanceDefinition')
        if not child:
            continue
        weight = values.init(entry.get('Probability'))
        result.append({'kind': 'pool' if entry.get('ItmPoolDefinition') else 'item', 'path': child,
                       'weight': weight, 'display': entry.get('ProbabilityDisplayString')})
    return result


def pool_gate(package, values, pool, stage):
    """True when `stage` meets the pool's game-stage requirements (None = unknown)."""
    props = package.props(pool)
    for key, test in (('MinGameStageRequirement', lambda s, v: s >= v), ('MaxGameStageRequirement', lambda s, v: s <= v)):
        if props.get(key):
            bound = values.attribute(props[key])
            if bound is None:
                return None
            if not test(stage, bound):
                return False
    return True


def source_lists(package, source):
    """(pool, probability init) pairs a source contributes, in data order."""
    cls = package.classes.get(source, '')
    props = package.props(source)
    pairs = []
    if cls.endswith('ItemPoolListDefinition'):
        return [(e.get('ItemPool'), e.get('PoolProbability')) for e in props.get('ItemPools') or []]
    if cls.endswith('AIPawnBalanceDefinition'):
        pairs += [(e.get('ItemPool'), e.get('PoolProbability')) for e in props.get('DefaultItemPoolList') or []]
        for listed in props.get('DefaultItemPoolIncludedLists') or []:
            pairs += source_lists(package, listed)
        first = (props.get('PlayThroughs') or [{}])[0] or {}
        pairs += [(e.get('ItemPool'), e.get('PoolProbability')) for e in first.get('CustomItemPoolList') or []]
        for listed in first.get('CustomItemPoolIncludedLists') or []:
            pairs += source_lists(package, listed)
        return pairs
    if 'ItemPoolDefinition' in cls:
        return [(source, {'BaseValueConstant': 1.0})]
    raise ValueError(f'{source}: unsupported loot source class {cls}')


def expand(package, values, pool, stage, depth=0, seen=()):
    """Pool -> {path, quantity, gate, entries: [{..., share, pool?}]} (shares among eligible entries)."""
    if pool in seen or depth > 12:
        return {'path': pool, 'cycle': True, 'entries': []}
    gate = pool_gate(package, values, pool, stage)
    quantity = values.init(package.props(pool).get('Quantity') or {'BaseValueConstant': 1.0})
    entries = pool_entries(package, values, pool)
    children = []
    for entry in entries:
        node = dict(entry)
        if entry['kind'] == 'pool':
            node['pool'] = expand(package, values, entry['path'], stage, depth + 1, seen + (pool,))
            if node['pool'].get('gate') is False:
                node['eligible'] = False
        children.append(node)
    total = sum(c['weight'] for c in children if c['weight'] and c.get('eligible', True))
    for c in children:
        c['share'] = (c['weight'] / total) if total > 0 and c['weight'] and c.get('eligible', True) else 0.0
    return {'path': pool, 'quantity': quantity, 'gate': gate, 'entries': children}


def flatten(tree, factor=1.0, out=None):
    """{item balance: expected count} for one firing of the pool tree (shares multiplied down)."""
    out = {} if out is None else out
    quantity = tree.get('quantity') or 1.0
    for entry in tree['entries']:
        p = factor * quantity * entry['share']
        if not p:
            continue
        if entry['kind'] == 'item':
            out[entry['path']] = out.get(entry['path'], 0.0) + p
        else:
            flatten(entry['pool'], p, out)
    return out


def roll_pool(tree, rng, chain=()):
    """Seeded roll of one firing: [{'item', 'chain'}] (UNVERIFIED native selection rule)."""
    drops = []
    eligible = [e for e in tree['entries'] if e['share'] > 0]
    if not eligible:
        return drops
    for _ in range(max(0, int(round(tree.get('quantity') or 1)))):
        entry = rng.choices(eligible, [e['share'] for e in eligible])[0]
        if entry['kind'] == 'item':
            drops.append({'item': entry['path'], 'chain': list(chain + (tree['path'],))})
        else:
            drops.extend(roll_pool(entry['pool'], rng, chain + (tree['path'],)))
    return drops


def table(package, values, source, stage):
    """Source -> {source, pools: [{pool, probability, tree, items}]}."""
    pools = []
    for pool, probability in source_lists(package, source):
        if not pool:
            continue
        chance = values.init(probability)
        tree = expand(package, values, pool, stage)
        items = flatten(tree) if tree.get('gate') is not False else {}
        by_class = {}
        for item, count in items.items():
            kind = package.classes.get(item, '?').rsplit('.', 1)[-1]
            by_class[kind] = by_class.get(kind, 0.0) + count
        pools.append({'pool': pool, 'probability': chance, 'by_class': by_class, 'tree': tree,
                      'items': dict(sorted(items.items(), key=lambda kv: -kv[1]))})
    return {'source': source, 'stage': stage, 'pools': pools, 'unresolved': sorted(values.unresolved)}


def roll(loot, seed):
    """Seeded roll of a whole source: each listed pool fires with min(1, probability)."""
    rng = random.Random(seed)
    drops = []
    for pool in loot['pools']:
        chance = pool['probability']
        if chance is None or pool['tree'].get('gate') is False:
            continue
        if rng.random() < min(1.0, chance):
            drops.extend(roll_pool(pool['tree'], rng))
    return drops


def display_check(package, pools):
    """Compare our shares (editor-like context) with every cooked ProbabilityDisplayString."""
    values = Values(package, editor=True)
    report = {'pools': 0, 'entries': 0, 'agree': 0, 'differ': 0, 'unresolved_entries': 0, 'no_display': 0,
              'differences': []}
    for pool in pools:
        entries = pool_entries(package, values, pool)
        if not entries:
            continue
        report['pools'] += 1
        total = sum(e['weight'] for e in entries if e['weight'])
        for e in entries:
            report['entries'] += 1
            shown = e['display']
            if shown is None:
                report['no_display'] += 1
                continue
            if e['weight'] is None:
                report['unresolved_entries'] += 1
                continue
            ours = 100.0 * e['weight'] / total if total > 0 else 0.0
            # The editor prints two decimals; '%' suffix.
            if abs(round(ours, 2) - float(shown.rstrip('%'))) <= 0.011:
                report['agree'] += 1
            else:
                report['differ'] += 1
                if len(report['differences']) < 40:
                    report['differences'].append({'pool': pool, 'entry': e['path'], 'ours': round(ours, 3), 'shown': shown})
    report['unresolved'] = sorted(values.unresolved)
    return report


def open_package(reader, path):
    schema = ROOT / 'local/loot/loot_pools.schema'
    schema.parent.mkdir(parents=True, exist_ok=True)
    schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
    package = weapon_recipe.Package(str(Path(reader).resolve()), Path(path), str(schema.resolve()))
    pools = [p for p, c in package.classes.items() if 'ItemPool' in c or 'Attribute' in c or 'Weighting' in p]
    package.preload(pools)
    return package


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True)
    parser.add_argument('--package', required=True, help='package holding the source, e.g. Startup.upk')
    sub = parser.add_subparsers(dest='command', required=True)
    show = sub.add_parser('table', help='weighted drop table of a source')
    show.add_argument('source', help='AIPawnBalanceDefinition, ItemPoolListDefinition or ItemPoolDefinition')
    show.add_argument('--stage', type=float, default=8, help='game stage for pool gates')
    show.add_argument('--players', type=float, default=1)
    show.add_argument('--seed', type=int, action='append', help='also roll with these seeds')
    sub.add_parser('check', help='compare every pool entry share with its cooked ProbabilityDisplayString')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    package = open_package(args.reader, args.package)
    if args.command == 'check':
        pools = sorted(p for p, c in package.classes.items() if c.endswith('ItemPoolDefinition'))
        result = display_check(package, pools)
    else:
        values = Values(package, {NUMBER_OF_PLAYERS: args.players})
        result = table(package, values, args.source, args.stage)
        result['rolls'] = {seed: roll(result, seed) for seed in args.seed or []}
    text = json.dumps(result, indent=1)
    if args.output:
        if not args.output.resolve().is_relative_to(ROOT / 'local'):
            parser.error('--output must stay under local/')
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding='utf-8')
        print(json.dumps({k: v for k, v in result.items() if k not in ('pools', 'differences', 'rolls')}, indent=1))
    else:
        print(text)


if __name__ == '__main__':
    main()
