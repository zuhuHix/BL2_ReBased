"""Synthetic tests for tools/prepare_skill_effects.py (no game data).

A fake package maps invented object paths to the property dictionaries ow-package would return.
Run: python tests/skill_effects_test.py
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import prepare_skill_effects as p  # noqa: E402
import skill_stats  # noqa: E402


class FakePackage:
    def __init__(self, objects, classes):
        self.objects, self.classes = objects, classes
        self.index = {path: i for i, path in enumerate(objects)}

    def props(self, path):
        return self.objects[path]


def constant(value, attribute=None, scale=1.0, definition=None):
    return {'BaseValueConstant': value, 'BaseValueAttribute': attribute, 'InitializationDefinition': definition,
            'BaseValueScaleConstant': scale}


OBJECTS = {
    'Pkg.Skill': {
        'SkillName': 'Invented', 'MaxGrade': 5, 'bSubjectToGradeRules': True, 'SkillType': 'SKILL_TYPE_Kill',
        'SkillEffectDefinitions': [
            {'AttributeToModify': 'Pkg.AttrA', 'EffectTarget': 'TARGET_Self', 'ModifierType': 'MT_Scale',
             'BaseModifierValue': constant(0.25), 'PerGradeUpgrade': constant(0.25), 'GradeToStartApplyingEffect': 1,
             'PerGradeUpgradeInterval': 1, 'BonusUpgradeList': [{'GradeToApplyAt': 3, 'Modifier': 0.5}]},
            {'AttributeToModify': 'Pkg.AttrB', 'EffectTarget': 'TARGET_Self', 'ModifierType': 'MT_PostAdd',
             'BaseModifierValue': constant(1, 'Pkg.Runtime', 0.5), 'PerGradeUpgrade': constant(1, 'Pkg.Constant', 2.0)},
            {'EffectTarget': 'TARGET_None', 'BaseModifierValue': constant(0, definition='Pkg.Formula'), 'PerGradeUpgrade': {}},
        ],
        'SkillConstraints': [
            {'Evaluator': 'Pkg.Skill.Compound', 'bApplyConstraintWhileActive': True, 'OnFailure': 'SKILL_Paused'},
            {'EvaluatorDefinitions': ['Pkg.Def'], 'bApplyConstraintOnActivatation': True},
        ],
    },
    'Pkg.Skill.Compound': {'Expression1': 'Pkg.Skill.Left', 'Operator': 'CEOP_Or', 'Expression2': 'Pkg.Skill.Right'},
    'Pkg.Skill.Left': {'Expression': {'AttributeOperand1': 'Pkg.AttrC', 'ComparisonOperator': 'OPERATOR_LessThan',
                                      'ConstantOperand2': 1}},
    'Pkg.Skill.Right': {'bNotInVehicle': True},
    'Pkg.Def': {'Evaluator': 'Pkg.Def.Health'},
    'Pkg.Def.Health': {'bHealthy': True},
    'Pkg.Constant': {},
    'Pkg.Constant.ConstantAttributeValueResolver_0': {'ConstantValue': 3.0},
    'Pkg.Formula': {'ConditionalInitialization': {'bEnabled': True}},
}
CLASSES = {
    'Pkg.Skill': 'WillowGame.SkillDefinition',
    'Pkg.Skill.Compound': 'GearboxFramework.CompoundExpressionEvaluator',
    'Pkg.Skill.Left': 'GearboxFramework.AttributeExpressionEvaluator',
    'Pkg.Skill.Right': 'WillowGame.VehiclePassengerExpressionEvaluator',
    'Pkg.Def': 'WillowGame.SkillExpressionEvaluatorDefinition',
    'Pkg.Def.Health': 'WillowGame.HealthStateExpressionEvaluator',
    'Pkg.Constant': 'Engine.AttributeDefinition',
    'Pkg.Constant.ConstantAttributeValueResolver_0': 'GearboxFramework.ConstantAttributeValueResolver',
    'Pkg.Formula': 'Engine.AttributeInitializationDefinition',
}


def entry():
    package = FakePackage(OBJECTS, CLASSES)
    return p.skill_entry([package], skill_stats.Resolver([package]), 'Pkg.Skill')


def test_skill_fields():
    s = entry()
    assert (s['name'], s['type'], s['maxGrade'], s['subjectToGradeRules']) == ('Invented', 'Kill', 5, True)


def test_effect_values():
    a, b, c = entry()['effects']
    assert a['modifierType'] == 'Scale' and a['target'] == 'Self' and a['base'] == {'constant': 0.25}
    assert a['bonus'] == [{'grade': 3, 'value': 0.5}]
    assert b['base'] == {'attribute': 'Pkg.Runtime', 'scale': 0.5}, b['base']
    assert b['perGrade'] == {'constant': 6.0}, 'a constant attribute resolves, times its scale'
    assert b['startGrade'] == 0 and b['interval'] == 0, 'untagged start grade is the struct default 0'
    assert c['attribute'] is None and c['target'] == 'None' and c['base'] == {'unresolved': 'Pkg.Formula'}


def test_conditions():
    first, second = entry()['conditions']
    assert first['onFailure'] == 'Paused' and first['whileActive'] is True
    test = first['test']
    assert test['kind'] == 'or' and test['right']['kind'] == 'onFoot'
    assert test['left'] == {'kind': 'attribute', 'attribute': 'Pkg.AttrC', 'op': 'LessThan', 'attribute2': None,
                            'usage': 'PreferAttribute', 'constant': 1}
    assert second['test']['kind'] == 'healthState' and second['onActivation'] is True and second['whileActive'] is False
    assert second['onFailure'] == 'Deactivated'


def test_output_must_be_inside_local():
    repo = Path(__file__).resolve().parents[1]
    assert p.local_output(repo / 'local' / 'ui' / 'run') == (repo / 'local' / 'ui' / 'run').resolve()
    for bad in (repo / 'docs' / 'verification', repo, repo / 'local', repo / 'local' / '..' / 'docs', repo / 'localx'):
        try:
            p.local_output(bad)
        except SystemExit:
            continue
        raise AssertionError(f'{bad} was accepted')


if __name__ == '__main__':
    tests = [v for k, v in sorted(globals().items()) if k.startswith('test_')]
    for t in tests:
        t()
    print(f'{len(tests)} skill effect tests passed')
