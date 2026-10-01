"""Per-grade stat lines for a skill's info box, from installed skill data.

A SkillDefinition lists SkillEffectDefinitions (attribute, modifier type, base
value, per-grade upgrade) and SkillEffectPresentations (AttributePresentation-
Definition objects: label text with $NUMBER$ and number-format flags). The
install's `<Package>.int` overrides the English strings. This module turns
them into the text the game's info box shows for each grade. The rules below
were fitted to the info-box HTML the game sent in a local UI trace
(DECISIONS.md 2026-09-27); they are observations, not Gearbox's code:

- An effect applies from GradeToStartApplyingEffect; at grade g its value is
  base + per-grade * floor((g - start) / interval).
- A presentation shows the effect on the same attribute (or on no attribute,
  when both name none), in order. A value comes from its
  AttributeInitializationData (tools/weapon_recipe.py attribute_value), with
  referenced attributes resolved through their ConstantAttributeValueResolver.
- bDisplayAsPercentage (default true) multiplies by 100 and adds "%".
  Numbers round to integers unless RoundingMode is ATTRROUNDING_Float or
  bDisplayPercentAsFloat is set, which keep up to one decimal.
- SignStyle Positive shows "+" and the magnitude, Negative "-" and the
  magnitude; otherwise the value's own sign, with "+" for non-negative values.
  bDontDisplayPlusSign drops a "+". Without bUseCustomNumberPlacement the
  line is the number, a space and the text; bDontDisplayNumber leaves the
  number empty but keeps the space (traced for Ruin).
- A DesignerAttributeDefinition resolves to its own BaseValue.
Values that need runtime state (for example a formula on the player's level)
are reported as unresolved and their line is left out.
"""
import json
import math
from pathlib import Path

import weapon_recipe

SCHEMA_LINES = [
    'SkillEffectDefinitions=StructProperty:SkillEffectData',
    'SkillEffectPresentations=ObjectProperty',
    'BonusUpgradeList=StructProperty:SkillBonusUpgrade',
]


def read_localization(path):
    """`<Package>.int` -> {(section, key): value}; quotes around values removed."""
    if not path.exists():
        return {}
    entries, section = {}, None
    for line in path.read_text(encoding='utf-16').splitlines():
        if line.startswith('['):
            section = line.strip()[1:-1].rsplit(' ', 1)[0]
        elif section and '=' in line:
            key, value = line.split('=', 1)
            value = value.strip()
            if len(value) >= 2 and value[0] == value[-1] == '"':
                value = value[1:-1]
            entries[(section, key.strip())] = value
    return entries


def localization_key(path, classes):
    """Object path -> .int section name: the package name is dropped and the
    first object inside a non-package outer is joined with ':'."""
    parts = path.split('.')
    key, joined = parts[1], False
    for i in range(2, len(parts)):
        outer = '.'.join(parts[:i])
        sep = ':' if not joined and classes.get(outer, 'Core.Package') != 'Core.Package' else '.'
        joined = joined or sep == ':'
        key += sep + parts[i]
    return key


class Package(weapon_recipe.Package):
    """weapon_recipe.Package plus each export's class (for localization keys)."""

    def __init__(self, reader, path, schema):
        super().__init__(reader, Path(path), schema)
        self.classes = {e['path']: e['class'] for e in json.loads(self.run('--exports'))}


class Resolver:
    """Constant attribute values, looked up in the given packages in order."""

    def __init__(self, packages):
        self.packages = packages
        self.known = {}
        self.unresolved = set()

    def attribute(self, path):
        if path not in self.known:
            prefix = path + '.ConstantAttributeValueResolver_'
            value = None
            for package in self.packages:
                if package.classes.get(path) == 'WillowGame.DesignerAttributeDefinition':
                    # A designer attribute carries its own BaseValue.
                    value = self.value(package, package.props(path).get('BaseValue'))
                    break
                for candidate in package.index:
                    if candidate.startswith(prefix):
                        value = package.props(candidate).get('ConstantValue')
                        break
                if value is not None:
                    break
            if value is None:
                self.unresolved.add(path)
                return None
            self.known[path] = float(value)
        return self.known[path]

    def value(self, package, init):
        init = init or {}
        attribute = init.get('BaseValueAttribute')
        if attribute and attribute not in self.known and self.attribute(attribute) is None:
            return None
        try:
            return weapon_recipe.attribute_value(package, init, 1, self.known)
        except KeyError as error:  # a definition in another package
            self.unresolved.add(str(error))
            return None


def effect_value(effect, grade, base, per):
    start = effect.get('GradeToStartApplyingEffect', 1)
    if grade < start or base is None:
        return None
    interval = max(1, effect.get('PerGradeUpgradeInterval') or 1)
    return base + (per or 0.0) * math.floor((grade - start) / interval)


def effect_rows(package, props, resolver, grades):
    """A skill's effects as numbers: one dict per SkillEffectData with its
    attribute, ModifierType, EffectTarget and the value at grades 0..grades
    (None where the effect does not apply yet or its value is unresolved).
    The grade rule is effect_value's; how the game combines the modifier types
    is not decided here (see tools/prepare_action_skill.py)."""
    rows = []
    for effect in props.get('SkillEffectDefinitions') or []:
        base = resolver.value(package, effect.get('BaseModifierValue'))
        per = resolver.value(package, effect.get('PerGradeUpgrade'))
        rows.append({
            'attribute': effect.get('AttributeToModify'),
            'modifierType': effect.get('ModifierType'),
            'target': effect.get('EffectTarget'),
            'startGrade': effect.get('GradeToStartApplyingEffect', 1),
            # Rounded to 6 places: the data are 32-bit floats (0.05 is stored as 0.0500000007).
            'values': [None if v is None else round(v, 6)
                       for v in (effect_value(effect, g, base, per) for g in range(grades + 1))],
        })
    return rows


def format_number(value, flags):
    percent = flags.get('bDisplayAsPercentage', True)
    shown = abs(value) * (100 if percent else 1)
    keep_float = flags.get('RoundingMode') == 'ATTRROUNDING_Float' or flags.get('bDisplayPercentAsFloat')
    shown = round(shown, 1) if keep_float else round(shown)
    text = f'{shown:.1f}'.rstrip('0').rstrip('.') if keep_float else str(int(shown))
    style = flags.get('SignStyle')
    sign = '+' if style == 'SIGNSTYLE_Positive' else '-' if style == 'SIGNSTYLE_Negative' else '-' if value < 0 else '+'
    if sign == '+' and flags.get('bDontDisplayPlusSign'):
        sign = ''
    return sign + text + ('%' if percent else '')


def skill_lines(package, skill_path, props, resolver, localization, grades):
    """[(grade -> [(before, number, after)])] for grades 1..grades, and notes."""
    effects = props.get('SkillEffectDefinitions') or []
    values = [(e, resolver.value(package, e.get('BaseModifierValue')), resolver.value(package, e.get('PerGradeUpgrade')))
              for e in effects]
    lines, notes = {g: [] for g in range(0, grades + 1)}, []
    used = set()
    for presentation in props.get('SkillEffectPresentations') or []:
        flags = package.props(presentation)
        key = localization_key(presentation, package.classes)
        text = localization.get((key, 'Description'), flags.get('Description', ''))
        match = next((i for i, (e, _, _) in enumerate(values)
                      if i not in used and e.get('AttributeToModify') == flags.get('Attribute')), None)
        if match is None:
            notes.append(f'{presentation}: no effect on {flags.get("Attribute")}')
            continue
        used.add(match)
        effect, base, per = values[match]
        for grade in lines:
            if grade < effect.get('GradeToStartApplyingEffect', 1):
                continue
            if flags.get('bDontDisplayNumber'):
                number = ''
            else:
                value = effect_value(effect, grade, base, per)
                if value is None:
                    continue
                number = format_number(value, flags)
            if flags.get('bUseCustomNumberPlacement'):
                # Custom placement without $NUMBER$ (Converge) shows no number;
                # that case is not traced (UNVERIFIED).
                before, after = text.split('$NUMBER$', 1) if '$NUMBER$' in text else (text, '')
                number = number if '$NUMBER$' in text else ''
            else:  # the number, a space, then the text (traced for Ruin)
                before, after = '', ' ' + text
            lines[grade].append((before, number, after))
        if base is None:
            notes.append(f'{presentation}: unresolved value')
    return lines, notes


def localized_description(skill_path, props, localization, classes):
    return localization.get((localization_key(skill_path, classes), 'SkillDescription'), props.get('SkillDescription', ''))


def localization_for(game, package_name):
    return read_localization(Path(game) / 'WillowGame' / 'Localization' / 'INT' / f'{package_name}.int')
