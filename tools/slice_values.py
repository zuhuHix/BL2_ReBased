"""Stock values for the Sanctuary Fire-mission slice: XP reward and Maya's health.

AI-assisted. Clean-room: no game data in this file. Values are decoded from the
installed packages with the project's reader and written under ignored local/.

What is decoded (data) and what is not (native code):

- Mission XP. `MissionDefinition.Reward.ExperienceRewardPercentage` names an
  AttributeDefinition whose ValueResolverChain is evaluated here: a constant
  resolver, then a SimpleMathValueResolver whose argument is an
  AttributeInitializationData. Conditional initialisations are evaluated only
  for their default branch (playthrough 1) and, separately, their single
  conditional branch (reported as the playthrough-2 value; which expression
  selects it is not decoded). The experience-required-per-level formula is a
  plain AttributeInitializationDefinition ValueFormula.
  `MissionDefinition.GetExperienceReward` is native; the amount follows the
  reading in docs/verification/NATIVE_PROGRESSION.md section 2 (UNVERIFIED in
  game): trunc(span x percentage) with span = R(L+1) - R(L) on the integer
  curve R(n) = max(0, trunc(f(n)) - trunc(f(1))), f evaluated in single
  precision (section 3). The playthrough multiplier is 1 for playthrough 1
  below level 50, so it is left out.
- Mission level. The mission's level is its GameStageRegion's game stage
  (NATIVE_PROGRESSION.md section 2): the region's RegionBalanceData entry in
  GlobalsDefinition.RegionBalanceData (playthrough 1) gives
  clamp(player level + boost, MinDefaultGameStage, MaxDefaultGameStage), or a
  completed mission override's bounds. The entry is decoded here
  (`region_stage`); applying it is the host's job.
- Maya's health. `PlayerClassDefinition.HealthPoolDefinition` ->
  `ResourcePoolDefinition.BaseMaxValue` -> an AttributeInitializationDefinition
  ValueFormula, evaluated with tools/weapon_recipe.py `attribute_value`
  (Multiplier * (Level ^ Power + Offset), min/max restriction). When an
  AttributeInitializationData names both a BaseValueAttribute and a
  BaseValueConstant, the attribute is used (the existing convention in
  weapon_recipe.py; UNVERIFIED). The shield pool has no BaseMaxValue: base
  shield capacity is 0 and capacity comes from an equipped shield item.
"""
import argparse
import json
import math
from pathlib import Path
import struct
import tempfile

import weapon_recipe

ROOT = Path(__file__).resolve().parents[1]
MISSION = 'GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire'
PLAYER_CLASS = 'GD_Siren.Character.CharClass_Siren'
XP_REQUIRED = 'GD_Balance_Experience.Formulas.Init_ExperienceRequiredForLevel'
GLOBALS = 'GD_Globals.General.Globals'
SCHEMA_LINES = [
    'RegionBalanceData=StructProperty:PlayThroughData',
    'BalanceDefinitions=ObjectProperty',
    'BalanceByRegion=StructProperty:RegionBalanceData',
    'MissionOverrides=StructProperty:MissionStateBalanceAdjustment',
    'ValueResolverChain=ObjectProperty',
    'ContextResolverChain=ObjectProperty',
    'ConditionalExpressionList=StructProperty:ConditionalAttributeInitializationData',
    'Dependencies=ObjectProperty',
    'ObjectiveDefs=ObjectProperty',
    'ObjectiveSetDefs=ObjectProperty',
    'MissionDependencies=StructProperty:MissionStatusPlayerData',
]
MATH = {'MATHRESOLVEROPERAND_Mul': lambda a, b: a * b, 'MATHRESOLVEROPERAND_Add': lambda a, b: a + b,
        'MATHRESOLVEROPERAND_Sub': lambda a, b: a - b, 'MATHRESOLVEROPERAND_Div': lambda a, b: a / b}


class Unresolved(Exception):
    pass


def chain_value(package, attribute, known=None):
    """Value of an AttributeDefinition whose ValueResolverChain needs no runtime context."""
    known = known or {}
    if attribute in known:
        return known[attribute]
    value = None
    for resolver in package.props(attribute).get('ValueResolverChain') or []:
        cls = package.classes.get(resolver)
        props = package.props(resolver)
        if cls == 'GearboxFramework.ConstantAttributeValueResolver':
            value = float(props.get('ConstantValue') or 0.0)
        elif cls == 'GearboxFramework.SimpleMathValueResolver':
            if value is None or props.get('Operand') not in MATH:
                raise Unresolved(f'{resolver}: unsupported math step')
            value = MATH[props['Operand']](value, init_value(package, props.get('Argument') or {}, known)['default'])
        else:
            raise Unresolved(f'{attribute}: runtime resolver {cls}')
    if value is None:
        raise Unresolved(f'{attribute}: empty resolver chain')
    return value


def init_value(package, init, known, branch='default'):
    """AttributeInitializationData -> {'default': value, 'conditional': value or None}.

    Only constants, constant attributes and conditional initialisations with
    constant branches are handled here; formulas go through weapon_recipe.
    """
    definition = init.get('InitializationDefinition')
    scale = init.get('BaseValueScaleConstant')
    scale = 1.0 if scale is None else scale
    if definition:
        props = package.props(definition)
        conditional = props.get('ConditionalInitialization') or {}
        if conditional.get('bEnabled'):
            default = init_value(package, conditional.get('DefaultBaseValue') or {}, known)['default']
            branches = conditional.get('ConditionalExpressionList') or []
            other = init_value(package, branches[0].get('BaseValueIfTrue') or {}, known)['default'] if len(branches) == 1 else None
            return {'default': default * scale, 'conditional': None if other is None else other * scale}
        value = weapon_recipe.attribute_value(package, init, 1, known)
        if value is None:
            raise Unresolved(f'{definition}: needs runtime state')
        return {'default': value, 'conditional': None}
    attribute = init.get('BaseValueAttribute')
    base = chain_value(package, attribute, known) if attribute else (init.get('BaseValueConstant') or 0.0)
    return {'default': base * scale, 'conditional': None}


def formula_at(package, definition, level_attributes, level, known):
    """Evaluate a ValueFormula definition with the named level attributes set to `level`."""
    values = dict(known)
    values.update({a: float(level) for a in level_attributes})
    value = weapon_recipe.attribute_value(package, {'InitializationDefinition': definition}, level, values)
    if value is None:
        raise Unresolved(definition)
    return value


def formula_attributes(package, definition):
    """Every BaseValueAttribute named by a ValueFormula's terms and restriction."""
    props = package.props(definition)
    terms = list((props.get('ValueFormula') or {}).values()) + list((props.get('RangeRestriction') or {}).values())
    return sorted({t['BaseValueAttribute'] for t in terms if isinstance(t, dict) and t.get('BaseValueAttribute')})


def f32(value):
    """Round a Python float to IEEE single precision (the native evaluator's float)."""
    return struct.unpack('<f', struct.pack('<f', value))[0]


def required_points(multiplier, power, offset, level):
    """R(n) = max(0, trunc(f(n)) - trunc(f(1))), f(n) = Multiplier x (n ^ Power + Offset) in single precision.

    NATIVE_PROGRESSION.md section 3 (UNVERIFIED in game). Each step is rounded to
    float as the native evaluator stores it; how its pow rounds is not known, so
    a level where the float and double results straddle an integer may be one off.
    """
    def f(n):
        return math.trunc(f32(f32(multiplier) * f32(f32(f32(n) ** f32(power)) + f32(offset))))
    return max(0, f(level) - f(1))


def mission_xp(percentage, span, multiplier=1.0):
    """trunc(span x percentage x playthrough multiplier), stored as float first (NATIVE_PROGRESSION.md section 2)."""
    return math.trunc(f32(span * f32(percentage) * multiplier))


def xp_candidate(percentage, required, level):
    """Mission XP at mission level L from an integer curve `required` (UNVERIFIED in game; the rule is native)."""
    return mission_xp(percentage, required(level + 1) - required(level))


def constant_init(package, init, known):
    """A bound such as MinDefaultGameStage: an AttributeInitializationData with no runtime input."""
    return init_value(package, init or {}, known)['default']


def region_stage(package, region, known, playthrough=1):
    """The region's RegionBalanceData entry for one playthrough, from GlobalsDefinition.RegionBalanceData.

    Searches that playthrough's BalanceDefinitions in order and returns the first
    entry naming `region`, with its bounds evaluated (they are constants in this
    data). The DLC-expansion table and playthrough 3 are not handled.
    """
    settings = package.props(GLOBALS)
    entry_set = next((e for e in settings.get('RegionBalanceData') or [] if e.get('PlayThroughNumber') == playthrough), None)
    if entry_set is None:
        raise Unresolved(f'no RegionBalanceData for playthrough {playthrough}')
    globals_boost = constant_init(package, settings.get('GameStageIncreaseAbovePlayer'), known)
    for definition in entry_set.get('BalanceDefinitions') or []:
        for entry in package.props(definition).get('BalanceByRegion') or []:
            if entry.get('Region') != region:
                continue
            boost = (constant_init(package, entry.get('GameStageIncreaseAbovePlayer'), known)
                     if entry.get('bSpecifyBoostAbovePlayer') else globals_boost)
            return {'region': region, 'playthrough': playthrough, 'balance_definition': definition,
                    'boost_above_player': boost,
                    'default': {'min': constant_init(package, entry.get('MinDefaultGameStage'), known),
                                'max': constant_init(package, entry.get('MaxDefaultGameStage'), known)},
                    'overrides': [{'mission': o.get('Mission'),
                                   'min': constant_init(package, o.get('MinGameStage'), known),
                                   'max': constant_init(package, o.get('MaxGameStage'), known)}
                                  for o in entry.get('MissionOverrides') or []],
                    'rule': 'clamp(player level + boost, min, max) of the largest completed override, else the '
                            'default bounds; fixed per player and playthrough the first time it is asked '
                            '(NATIVE_PROGRESSION.md section 2, UNVERIFIED in game)'}
    raise Unresolved(f'{region}: no RegionBalanceData entry for playthrough {playthrough}')


def build(reader, game, levels=range(1, 16)):
    startup = Path(game) / 'WillowGame' / 'CookedPCConsole' / 'Startup.upk'
    with tempfile.TemporaryDirectory() as tmp:
        schema = Path(tmp) / 'values.schema'
        schema.write_text('\n'.join(SCHEMA_LINES + weapon_recipe.SCHEMA_LINES) + '\n')
        package = weapon_recipe.Package(str(reader), startup, str(schema))
        package.classes = {e['path']: e['class'] for e in json.loads(package.run('--exports'))}
        mission = package.props(MISSION)
        reward = (mission.get('Reward') or {}).get('ExperienceRewardPercentage') or {}
        percentage_attribute = reward.get('BaseValueAttribute')
        known = {}
        percentage = {'attribute': percentage_attribute, 'playthrough1': None, 'playthrough2': None}
        chain = package.props(percentage_attribute).get('ValueResolverChain') or []
        steps = []
        for resolver in chain:
            steps.append({'resolver': resolver, 'class': package.classes.get(resolver), 'props': package.props(resolver)})
        constant = next(s['props'].get('ConstantValue') for s in steps if s['class'].endswith('ConstantAttributeValueResolver'))
        multiplier = [init_value(package, s['props'].get('Argument') or {}, known)
                      for s in steps if s['class'].endswith('SimpleMathValueResolver')]
        if len(steps) != 2 or len(multiplier) != 1 or steps[1]['props'].get('Operand') != 'MATHRESOLVEROPERAND_Mul':
            raise Unresolved('Unexpected XP reward resolver chain shape')
        percentage['playthrough1'] = constant * multiplier[0]['default']
        if multiplier[0]['conditional'] is not None:
            percentage['playthrough2'] = constant * multiplier[0]['conditional']
        level_attrs = formula_attributes(package, XP_REQUIRED)
        required = lambda level: formula_at(package, XP_REQUIRED, level_attrs, level, known)
        terms = package.props(XP_REQUIRED).get('ValueFormula') or {}
        curve = [constant_init(package, terms.get(k), known) for k in ('Multiplier', 'Power', 'Offset')]
        points = lambda level: required_points(*curve, level)
        xp = {'mission': MISSION, 'reward_percentage': percentage,
              'resolver_chain': [{'resolver': s['resolver'], 'class': s['class']} for s in steps],
              'required_formula': {'definition': XP_REQUIRED, 'level_attributes': level_attrs,
                                   'formula': package.props(XP_REQUIRED).get('ValueFormula')},
              'required_by_level': {str(l): required(l) for l in levels},
              'required_points_by_level': {str(l): points(l) for l in levels},
              'candidate_amount_by_mission_level': {str(l): xp_candidate(percentage['playthrough1'], points, l)
                                                    for l in levels},
              'amount_rule': 'trunc(percentage x (R(L+1) - R(L))) on the integer curve R '
                             '(docs/verification/NATIVE_PROGRESSION.md sections 2-3, read from native code): UNVERIFIED',
              'mission_level': "the GameStageRegion's game stage (region_stage), NATIVE_PROGRESSION.md section 2: "
                               'UNVERIFIED',
              'game_stage_region': mission.get('GameStageRegion'),
              'region_stage': region_stage(package, mission.get('GameStageRegion'), known)}
        siren = package.props(PLAYER_CLASS)
        pool = package.props(siren['HealthPoolDefinition'])
        base = pool.get('BaseMaxValue') or {}
        definition = base.get('InitializationDefinition')
        attributes = formula_attributes(package, definition)
        level_attribute = [a for a in attributes if a.endswith('PlayerExperienceLevel')]
        if len(level_attribute) != 1:
            raise Unresolved('Expected one player-level attribute in the health formula')
        constants = {}
        for a in attributes:
            if a not in level_attribute:
                constants[a] = chain_value(package, a)
        health = {'player_class': PLAYER_CLASS, 'health_pool': siren['HealthPoolDefinition'], 'definition': definition,
                  'formula': package.props(definition).get('ValueFormula'),
                  'restriction': package.props(definition).get('RangeRestriction'),
                  'constant_attributes': constants, 'level_attribute': level_attribute[0],
                  'by_level': {str(l): formula_at(package, definition, level_attribute, l, constants) for l in levels},
                  'start_with_max': bool(pool.get('StartWithMaxValue')),
                  'attribute_over_constant': 'BaseValueAttribute used when both are set; the native evaluator does so '
                                             'whenever the attribute resolves (NATIVE_PROGRESSION.md sections 1 and 4): '
                                             'UNVERIFIED in game'}
        shield_pool = package.props(siren['ShieldPoolDefinition'])
        shield = {'shield_pool': siren['ShieldPoolDefinition'], 'base_max_value': shield_pool.get('BaseMaxValue'),
                  'note': 'no BaseMaxValue in data: base capacity 0, capacity from an equipped shield item'}
        dependencies = []
        for dep in mission.get('Dependencies') or []:
            d = package.props(dep)
            dependencies.append({'mission': dep, 'name': d.get('MissionName'), 'plot_critical': bool(d.get('bPlotCritical')),
                                 'dependencies': d.get('Dependencies') or [], 'next_in_chain': d.get('NextMissionInChain'),
                                 'game_stage_region': d.get('GameStageRegion'), 'turn_in_station': d.get('TurnInStation'),
                                 'objectives': d.get('ObjectiveDefs') or []})
        station = package.props(mission['TravelStation']) if mission.get('TravelStation') else {}
        return {'xp': xp, 'health': health, 'shield': shield,
                'dependency': {'mission_dependencies': dependencies,
                               'mission_travel_station': mission.get('TravelStation'),
                               'travel_station_dependencies': station.get('MissionDependencies') or [],
                               'status_rule': 'which dependency status unlocks the mission is MissionTracker (native): UNVERIFIED'}}


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument('--reader', type=Path, default=ROOT / 'build/Release/ow-package.exe')
    p.add_argument('--game', type=Path, required=True)
    p.add_argument('--output', type=Path, default=ROOT / 'local/slice/values.json')
    args = p.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / 'local'):
        raise SystemExit('Game-derived output must stay under repository local/')
    result = build(args.reader, args.game)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=2) + '\n')
    print('health L1..3', [round(result['health']['by_level'][str(l)], 1) for l in (1, 2, 3)],
          'xp percentage', result['xp']['reward_percentage'])


if __name__ == '__main__':
    main()
