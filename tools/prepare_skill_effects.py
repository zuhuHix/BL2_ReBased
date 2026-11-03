"""Prepare a class's skill effects (modifiers and constraints) from installed data for the UE host.

Writes one JSON manifest under ignored local/ (default local/ui/run/skilleffects_siren.json, next to the
skilltree_siren.json that tools/prepare_skill_tree.py writes; the host loads it beside the tree). Everything in it is
read from the installed packages with `ow-package --properties`; nothing is copied into the repository.

For every skill of the tree (action skill first, then the branches in tree order):
- `type`: SkillDefinition.SkillType without its prefix (Passive when untagged, the enum's 0);
- `effects`: one row per SkillEffectData, in order: the attribute it modifies (None for the rows that only carry a
  number for a behaviour or the info box), ModifierType, EffectTarget, GradeToStartApplyingEffect,
  PerGradeUpgradeInterval, the base and per-grade values, and BonusUpgradeList. A value is
  {"constant": x} when it reduces to a number here (constant attributes resolved through their
  ConstantAttributeValueResolver or a DesignerAttributeDefinition's BaseValue), {"attribute": A, "scale": s} when it
  is another attribute's runtime value times s, or {"unresolved": path} (an initialization definition with a formula
  or a condition, evaluated by nobody here);
- `conditions`: SkillConstraints as a small expression tree the host evaluates: attribute comparisons
  (AttributeExpressionEvaluator), And/Or of two (CompoundExpressionEvaluator), the on-foot test
  (VehiclePassengerExpressionEvaluator), the healthy test (HealthStateExpressionEvaluator) and anything else by class.

How the host turns a row into a modifier value (the grade formula of NATIVE_SKILLS.md 4.3) and combines modifiers (the
stack formula of NATIVE_WEAPON_RULES.md section 1) is in OpenWillowSkillEffects.h; both are native reads, UNVERIFIED in
game. Rows with a None attribute or TARGET_None apply no modifier.
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import prepare_skill_tree  # noqa: E402
import skill_stats  # noqa: E402

FORMAT = 'openwillow.skill_effects/1'
REPO = Path(__file__).resolve().parents[1]
SCHEMA_LINES = prepare_skill_tree.SCHEMA_LINES + [
    'SkillConstraints=StructProperty:SkillConstraintData',
    'EvaluatorDefinitions=ObjectProperty',
]


def local_output(path, repo=REPO):
    """The output folder, resolved; refuses anything not below the repository's ignored local/ folder, because the
    manifest holds game-derived names and values that must never land in a tracked file."""
    resolved = Path(path).resolve()
    local = (Path(repo) / 'local').resolve()
    # Strictly below local/: the schema file is written to the output's parent folder.
    if local not in resolved.parents:
        raise SystemExit(f'refusing --output {path}: it must be inside {local} (ignored game-derived output)')
    return resolved


def strip(value, prefix):
    return value[len(prefix):] if isinstance(value, str) and value.startswith(prefix) else value


def owner(packages, path):
    for package in packages:
        if path in package.index:
            return package
    raise KeyError(f'{path} is in none of the packages read')


def init_value(packages, resolver, init):
    """AttributeInitializationData -> {"constant"} | {"attribute", "scale"} | {"unresolved"}."""
    init = init or {}
    definition = init.get('InitializationDefinition')
    attribute = init.get('BaseValueAttribute')
    scale = init.get('BaseValueScaleConstant', 1.0)
    if definition:
        value = resolver.value(owner(packages, definition), init)
        return {'constant': round(value, 6)} if value is not None else {'unresolved': definition}
    if attribute:
        constant = resolver.attribute(attribute)
        if constant is not None:
            return {'constant': round(constant * scale, 6)}
        return {'attribute': attribute, 'scale': round(scale, 6)}
    return {'constant': round((init.get('BaseValueConstant') or 0.0) * scale, 6)}


def expression(packages, path):
    """An evaluator object as a condition node."""
    package = owner(packages, path)
    cls = package.classes[path].split('.')[-1]
    props = package.props(path)
    if cls == 'AttributeExpressionEvaluator':
        e = props.get('Expression') or {}
        return {'kind': 'attribute', 'attribute': e.get('AttributeOperand1'),
                'op': strip(e.get('ComparisonOperator') or 'OPERATOR_EqualTo', 'OPERATOR_'),
                'attribute2': e.get('AttributeOperand2'),
                'usage': strip(e.get('Operand2Usage') or 'OPERAND_PreferAttribute', 'OPERAND_'),
                'constant': e.get('ConstantOperand2', 0.0)}
    if cls == 'CompoundExpressionEvaluator':
        return {'kind': strip(props.get('Operator') or 'CEOP_And', 'CEOP_').lower(),
                'left': expression(packages, props['Expression1']), 'right': expression(packages, props['Expression2'])}
    if cls == 'VehiclePassengerExpressionEvaluator':
        return {'kind': 'onFoot', 'properties': props}
    if cls == 'HealthStateExpressionEvaluator':
        return {'kind': 'healthState', 'properties': props}
    return {'kind': 'other', 'class': package.classes[path], 'properties': props}


def conditions(packages, constraints):
    rows = []
    for constraint in constraints:
        evaluators = [constraint['Evaluator']] if constraint.get('Evaluator') else []
        for definition in constraint.get('EvaluatorDefinitions') or []:
            evaluators.append(owner(packages, definition).props(definition)['Evaluator'])
        for evaluator in evaluators:
            rows.append({'test': expression(packages, evaluator),
                         'onActivation': constraint.get('bApplyConstraintOnActivatation', False),
                         'whileActive': constraint.get('bApplyConstraintWhileActive', False),
                         'whilePaused': constraint.get('bApplyConstraintWhilePaused', False),
                         'onFailure': strip(constraint.get('OnFailure') or 'SKILL_Deactivated', 'SKILL_')})
    return rows


def skill_entry(packages, resolver, path):
    package = owner(packages, path)
    props = package.props(path)
    effects = []
    for effect in props.get('SkillEffectDefinitions') or []:
        effects.append({
            'attribute': effect.get('AttributeToModify'),
            'modifierType': strip(effect.get('ModifierType') or 'MT_Scale', 'MT_'),
            'target': strip(effect.get('EffectTarget') or 'TARGET_None', 'TARGET_'),
            # The struct default for an untagged start grade is 0 (NATIVE_SKILLS.md 4.3).
            'startGrade': effect.get('GradeToStartApplyingEffect', 0),
            'interval': effect.get('PerGradeUpgradeInterval', 0),
            'base': init_value(packages, resolver, effect.get('BaseModifierValue')),
            'perGrade': init_value(packages, resolver, effect.get('PerGradeUpgrade')),
            'bonus': [{'grade': b.get('GradeToApplyAt', 0), 'value': b.get('Modifier', 0.0)}
                      for b in effect.get('BonusUpgradeList') or []],
        })
    return {
        'id': path,
        'name': props.get('SkillName', ''),
        'type': strip(props.get('SkillType') or 'SKILL_TYPE_Passive', 'SKILL_TYPE_'),
        'maxGrade': props.get('MaxGrade', 1),
        'subjectToGradeRules': props.get('bSubjectToGradeRules', False),
        'durationType': strip(props.get('DurationType') or 'DURATION_Infinite', 'DURATION_'),
        'initialDuration': props.get('InitialDuration', 0.0),
        'effects': effects,
        'conditions': conditions(packages, props.get('SkillConstraints') or []),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--package', default='GD_Siren_Streaming_SF.upk')
    parser.add_argument('--tree', default='GD_Siren_Streaming.SkillTree.SkillTree_Siren')
    parser.add_argument('--name', default='siren')
    parser.add_argument('--output', default='local/ui/run')
    args = parser.parse_args()

    cooked = Path(args.game) / 'WillowGame' / 'CookedPCConsole'
    output = local_output(args.output)
    schema = output.parent / 'skilleffects.schema'
    schema.parent.mkdir(parents=True, exist_ok=True)
    schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
    reader = str(Path(args.reader).resolve())
    package = skill_stats.Package(reader, cooked / args.package, str(schema.resolve()))
    startup = skill_stats.Package(reader, cooked / 'Startup.upk', str(schema.resolve()))
    packages = [package, startup]
    resolver = skill_stats.Resolver(packages)

    root = package.props(package.props(args.tree)['Root'])
    paths = [root['Tiers'][0]['Skills'][0]]
    for branch in root['Children']:
        for tier in package.props(branch)['Tiers']:
            paths.extend(tier.get('Skills') or [])
    skills = [skill_entry(packages, resolver, path) for path in paths]
    manifest = {'format': FORMAT, 'tree': args.tree, 'skills': skills, 'unresolved': sorted(resolver.unresolved)}
    output.mkdir(parents=True, exist_ok=True)
    target = output / f'skilleffects_{args.name}.json'
    target.write_text(json.dumps(manifest, indent=1, ensure_ascii=False), encoding='utf-8')
    effects = sum(len(s['effects']) for s in skills)
    applied = sum(1 for s in skills for e in s['effects'] if e['attribute'] and e['target'] != 'None')
    print(f'wrote {target}: {len(skills)} skills, {effects} effects ({applied} apply a modifier), '
          f'{sum(len(s["conditions"]) for s in skills)} conditions, unresolved {len(manifest["unresolved"])}')


if __name__ == '__main__':
    main()
