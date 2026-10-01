"""Synthetic tests for tools/skill_stats.py (no game data).

A fake package stands in for ow-package: it maps made-up object paths to the
property dictionaries the real reader would return. The expected strings are
shaped like the traced info-box lines, with invented labels and values.
Run: python tests/skill_stats_test.py
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import skill_stats as s  # noqa: E402


class FakePackage:
    def __init__(self, objects, classes=None):
        self.objects = objects
        self.index = {path: i for i, path in enumerate(objects)}
        self.classes = classes or {}

    def props(self, path):
        return self.objects[path]


def constant(value):
    return {'BaseValueConstant': value, 'BaseValueScaleConstant': 1.0}


def test_number_format():
    assert s.format_number(0.05, {}) == '+5%'
    assert s.format_number(-0.08, {'SignStyle': 'SIGNSTYLE_Negative'}) == '-8%'
    assert s.format_number(-0.1, {'SignStyle': 'SIGNSTYLE_Positive'}) == '+10%'
    assert s.format_number(0.1, {'bDontDisplayPlusSign': True}) == '10%'
    assert s.format_number(0.012, {'bDisplayPercentAsFloat': True}) == '+1.2%'
    assert s.format_number(0.5, {'bDisplayAsPercentage': False, 'RoundingMode': 'ATTRROUNDING_Float'}) == '+0.5'
    assert s.format_number(13, {'bDisplayAsPercentage': False, 'bDontDisplayPlusSign': True}) == '13'


def test_localization_key():
    classes = {'Pkg': 'Core.Package', 'Pkg.Group': 'Core.Package', 'Pkg.Group.Skill': 'WillowGame.SkillDefinition'}
    assert s.localization_key('Pkg.Group.Skill', classes) == 'Group.Skill'
    assert s.localization_key('Pkg.Group.Skill.Presentation_1', classes) == 'Group.Skill:Presentation_1'


def test_effect_value():
    effect = {'GradeToStartApplyingEffect': 1, 'PerGradeUpgradeInterval': 1}
    assert s.effect_value(effect, 0, 0.1, 0.1) is None
    assert abs(s.effect_value(effect, 3, 0.1, 0.1) - 0.3) < 1e-9
    every_other = {'GradeToStartApplyingEffect': 1, 'PerGradeUpgradeInterval': 2}
    assert abs(s.effect_value(every_other, 4, 1.0, 1.0) - 2.0) < 1e-9


def test_skill_lines():
    objects = {
        'Pkg.Group.Skill.PresA': {'Attribute': 'Attr.Speed', 'Description': 'Speed: $NUMBER$',
                                  'bUseCustomNumberPlacement': True},
        'Pkg.Group.Skill.PresB': {'Attribute': 'Attr.Quote', 'Description': 'Quoted text', 'bDontDisplayNumber': True},
        'Pkg.Misc.Designer': {'BaseValue': constant(0.25)},
        'Pkg.Group.Skill.PresC': {'Attribute': 'Attr.Chance', 'Description': '$NUMBER$ chance.',
                                  'bUseCustomNumberPlacement': True, 'bDontDisplayPlusSign': True},
    }
    classes = {'Pkg.Group.Skill': 'WillowGame.SkillDefinition',
               'Pkg.Misc.Designer': 'WillowGame.DesignerAttributeDefinition'}
    package = FakePackage(objects, classes)
    props = {
        'SkillEffectDefinitions': [
            {'AttributeToModify': 'Attr.Speed', 'BaseModifierValue': constant(0.1),
             'PerGradeUpgrade': constant(0.1), 'GradeToStartApplyingEffect': 1, 'PerGradeUpgradeInterval': 1},
            {'AttributeToModify': 'Attr.Quote', 'BaseModifierValue': constant(1.0),
             'PerGradeUpgrade': constant(0.0), 'GradeToStartApplyingEffect': 1, 'PerGradeUpgradeInterval': 1},
            {'AttributeToModify': 'Attr.Chance',
             'BaseModifierValue': {'BaseValueConstant': 0.0, 'BaseValueAttribute': 'Pkg.Misc.Designer',
                                   'BaseValueScaleConstant': 1.0},
             'PerGradeUpgrade': constant(0.0), 'GradeToStartApplyingEffect': 1, 'PerGradeUpgradeInterval': 1},
        ],
        'SkillEffectPresentations': ['Pkg.Group.Skill.PresA', 'Pkg.Group.Skill.PresB', 'Pkg.Group.Skill.PresC'],
    }
    localization = {('Group.Skill:PresA', 'Description'): 'Localized speed: $NUMBER$'}
    lines, notes = s.skill_lines(package, 'Pkg.Group.Skill', props, s.Resolver([package]), localization, 2)
    assert notes == []
    assert lines[0] == []
    assert lines[2] == [('Localized speed: ', '+20%', ''), ('', '', ' Quoted text'), ('', '25%', ' chance.')]


def test_effect_rows():
    package = FakePackage({})
    props = {'SkillEffectDefinitions': [
        {'AttributeToModify': 'Attr.Time', 'ModifierType': 'MT_PostAdd', 'EffectTarget': 'TARGET_Self',
         'BaseModifierValue': constant(0.25), 'PerGradeUpgrade': constant(0.25),
         'GradeToStartApplyingEffect': 1, 'PerGradeUpgradeInterval': 1},
        {'AttributeToModify': 'Attr.Rate', 'ModifierType': 'MT_PreAdd',
         'BaseModifierValue': constant(-1.0), 'PerGradeUpgrade': constant(0.0),
         'GradeToStartApplyingEffect': 0, 'PerGradeUpgradeInterval': 1},
    ]}
    rows = s.effect_rows(package, props, s.Resolver([package]), 3)
    assert rows[0] == {'attribute': 'Attr.Time', 'modifierType': 'MT_PostAdd', 'target': 'TARGET_Self',
                       'startGrade': 1, 'values': [None, 0.25, 0.5, 0.75]}
    assert rows[1]['values'] == [-1.0, -1.0, -1.0, -1.0] and rows[1]['target'] is None


if __name__ == '__main__':
    tests = [(name, fn) for name, fn in sorted(globals().items()) if name.startswith('test_')]
    for name, fn in tests:
        fn()
        print('ok', name)
    print(f'{len(tests)} tests passed')
