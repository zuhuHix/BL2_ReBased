"""Prepare Maya's action skill (Phaselock) and one skill-tree upgrade path from installed data.

Writes one JSON manifest under ignored local/ (default
local/character/action_skill_siren.json) for the UE host. Everything in it is
read from the installed packages with `ow-package --properties` (the tagged
property reader, which keeps enum names such as MT_PostAdd); nothing is
copied into the repository. The schema and what is verified are in
docs/verification/PHASELOCK_STOCK_DATA.md.

What is read, and from where:
- the skill tree root (action skill tier) and the chosen branch's tiers;
- Skill_Phaselock, its LiftActionSkill archetype (ActionSkill_Phaselock) and the
  LiftActionSkill / PhaseLockDefinition class defaults (WillowGame.upk);
- the skills the archetype's behaviors switch on and off (cooldown manager,
  diminishing returns), the cooldown pool and Cooldown_Phaselock (Startup.upk);
- the archetype's BehaviorProviderDefinition: sequences, events, behaviors with
  their own properties and the packed output links, checked structurally;
- the skill-point formulas in GD_Globals.Skills (Startup.upk);
- the upgrade path's SkillDefinitions, with per-grade numbers from
  tools/skill_stats.py;
- Skill_Phaselock's constraint evaluators (class and own properties; their
  Evaluate() is native), the CanLiftTargetIf flag chain and the player
  controller's auto-aim strategy data (WillowGlobals.AutoAimDefinition, which
  WillowPlayerController.InitInputSystem hands to its WillowAutoAimStrategy).

Format history: /1 (2026-10-01) first version; /2 (2026-10-01) adds
skill.constraintEvaluators, actionSkill.canLiftTargetIfChain and autoAim. The
UE host reads only /2, so a /1 manifest must be regenerated.

Rules that are NOT read from data and stay UNVERIFIED (see the record):
- attribute modifiers combine as (base + PreAdd) * (1 + Scale) + PostAdd, the
  rule fitted for weapons (tools/weapon_stats.py);
- the timing formulas in `timeline` restate what LiftActionSkill's script does
  (read with research/script_disasm.py, summarised in SCRIPT_FLOW below); the
  script is executed by nobody here;
- what a behavior output link's id byte selects.
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import prepare_skill_tree  # noqa: E402
import skill_stats  # noqa: E402

FORMAT = 'openwillow.action_skill/2'
SCHEMA_LINES = prepare_skill_tree.SCHEMA_LINES + [
    # LiftActionSkill / SkillDefinition / behaviors
    'PhaselockedAttributeEffects=StructProperty:AttributeEffectData',
    'LiftBodyMap=StructProperty:LiftBodyPair',
    'AttributeEffects=StructProperty:AttributeEffectData',
    'InstanceData=StructProperty:InstanceDataNameInfo',
    'SkillConstraints=StructProperty:SkillConstraintData',
    'EvaluatorDefinitions=ObjectProperty',
    'KillEvents=StructProperty:SkillKillEventData',
    'EventConstraints=ObjectProperty',
    'Conditions=StructProperty:BehaviorConditionData',
    'Behaviors=ObjectProperty',
    'FireLocationSocketNames=NameProperty',
    'FlagChain=StructProperty:FlagEvaluationData',
    # BehaviorProviderDefinition
    'BehaviorSequences=StructProperty:BehaviorSequenceData',
    'EventData2=StructProperty:BehaviorEventData',
    'BehaviorData2=StructProperty:BehaviorData',
    'ConsolidatedOutputLinkData=StructProperty:BehaviorOutputLinkData',
    'VariableData=StructProperty:BehaviorVariableData',
    'ConsolidatedVariableLinkData=StructProperty:BehaviorVariableLinkData',
    'ConsolidatedLinkedVariables=IntProperty',
    'EventData=StructProperty:OldBehaviorEventData',
    'BehaviorData=StructProperty:OldBehaviorData',
    # AttributeInitializationDefinition conditions (skill points)
    'ConditionalExpressionList=StructProperty:ConditionalAttributeInitializationExpressions',
    'Expressions=StructProperty:AttributeExpression',
]

# What the installed LiftActionSkill / WillowPlayerController script does, in our
# own words. Read from `python research/script_disasm.py WillowGame.upk
# LiftActionSkill.<Function>` (2026-10-01); listings stay under local/. Events
# are the BehaviorProviderDefinition events each step fires: every LiftActionSkill
# On* function hands its own name to the native
# BehaviorKernel.ActivateBehaviorEventFromScript with (instigator, target).
SCRIPT_FLOW = [
    {'function': 'WillowPlayerController.StartActionSkill', 'events': [],
     'summary': 'The target is the auto-aim preferred target (native WillowAutoAimStrategy.GetPreferredTarget) '
                'when it is a WillowPawn, else none; ServerStartActionSkill activates the skill through the '
                'native SkillEffectManager.'},
    {'function': 'WillowPlayerController.InitInputSystem', 'events': [],
     'summary': 'A local controller creates its WillowAutoAimStrategy and sets its DataDefinition to '
                'WillowGlobals.AutoAimDefinition (see autoAim). No range test appears in script.'},
    {'function': 'WillowPlayerController.ServerStartActionSkill', 'events': [],
     'summary': 'Uses the driver when the pawn is a vehicle; returns when that pawn is on a ladder (Physics 9, the '
                'value BeginLifting also treats as a ladder). Needs the action skill in the skill tree '
                '(GetSkillState). When IsActionSkillOnCooldown (pool value above 0) is false and the skill is not '
                'active, it returns in an attached rider seat, else calls the native ActivateSkill; '
                'SkillConstraints are not evaluated in script.'},
    {'function': 'WillowPlayerController.ActionSkillCallback', 'events': [],
     'summary': 'On activation StartActiveSkillCooldown refills the skill cooldown pool to 100%.'},
    {'function': 'ActionSkill.OnActionSkillStarted', 'events': ['OnActionSkillActivated'],
     'summary': 'Stores pawn, controller and target, then fires OnActionSkillActivated.'},
    {'function': 'LiftActionSkill.OnActionSkillStarted', 'events': [],
     'summary': 'SkillDuration = LiftDuration + LockDurationFormula (on Maya) * LockDurationScaleFormula '
                '(on the target); plays PhaselockSMD_Hit or PhaselockSMD_Miss on Maya; calls SelectTarget.'},
    {'function': 'LiftActionSkill.SelectTarget', 'events': [],
     'summary': 'If CanPhaseLockTarget (target set, not friendly, alive and well, not already phaselocked): '
                'PhaseLockTarget when the target drives no vehicle and CanLiftTargetIf passes, else '
                'TargetBlocked. Otherwise ResurrectTarget when Res can revive it, else FizzleOut.'},
    {'function': 'LiftActionSkill.PhaseLockTarget', 'events': ['OnSelectedTarget'],
     'summary': 'State Lifting for LiftDuration; fires OnSelectedTarget; applies PhaselockedAttributeEffects '
                'to the target; timers LockTarget at +LiftDuration, StartOutro at SkillDuration - '
                'LockFadeOutTime and EndSkill at SkillDuration + ReleaseBufferTime (from skill start). '
                'With Thoughtlock active the target is charmed, else LiftTarget lifts it (PhaseLockDefinition '
                'HeightFromGround and Lift/Loop anims); Helios spawns when active.'},
    {'function': 'LiftActionSkill.BeginLifting', 'events': [],
     'summary': 'Lift end: from the pawn location (GetLiftCheckLocation), trace down HeightFromGround (extent 1 uu, '
                'TRACEFLAG_Blocking, actors included). On a hit the end is the hit location raised by the pawn\'s '
                'CollisionHeight (cylinder half height) plus HeightFromGround; then a trace from the pawn location '
                'up to that end lowers it to the hit minus CollisionHeight. Without ground in reach the end is the '
                'pawn location (no lift).'},
    {'function': 'LiftActionSkill.GetLiftLocation', 'events': [],
     'summary': 'Lift curve: up to LiftDuration x LiftSnapTimePct, lerp start -> snap point (LiftSnapHeightPct of '
                'the way) by a^2; then snap point -> end by 1 - (1 - a)^2.'},
    {'function': 'LiftActionSkill.GetBobLocation', 'events': [],
     'summary': 'Bob: end + LiftBobAmplitude x sin((now - SkillStartTime) x LiftBobFrequency x pi) in Z, '
                'smoothed by VInterpTo from the previous bob location at speed 1.'},
    {'function': 'LiftActionSkill.LockTarget', 'events': ['OnCharmTarget', 'OnTargetBecomesLocked'],
     'summary': 'State Locked for SkillDuration - LiftDuration - LockFadeOutTime; fires OnCharmTarget when '
                'charmed, else spawns the bubble and fires OnTargetBecomesLocked.'},
    {'function': 'LiftActionSkill.StartOutro', 'events': ['OnTargetIsAboutToBecomeUnlocked'],
     'summary': 'State Outro for LockFadeOutTime; fires OnTargetIsAboutToBecomeUnlocked; ReleaseTarget '
                'after LockFadeOutTime.'},
    {'function': 'LiftActionSkill.ReleaseTarget', 'events': ['OnReleasedTarget'],
     'summary': 'Fires OnReleasedTarget, removes the PhaselockedAttributeEffects and, unless charmed, drops '
                'the target (DropAnim over DropTime).'},
    {'function': 'LiftActionSkill.EndSkill', 'events': ['OnActionSkillDeactivated'],
     'summary': 'Releases a still-held target, lands a dropped one, detonates a Sub-Sequence projectile and '
                'deactivates the action skill; ActionSkill.OnActionSkillEnded fires OnActionSkillDeactivated.'},
    {'function': 'LiftActionSkill.OnActionSkillTick', 'events': ['OnKilledTarget'],
     'summary': 'Moves the lifted pawn (snap lift, then a sine bob of LiftBobAmplitude at LiftBobFrequency). '
                'When the lifted pawn dies, or is staggered while not charmed, InterruptPhaseLock fires '
                'OnKilledTarget, then either spawns a Sub-Sequence projectile (chance roll) or schedules '
                'EndSkill after ReleaseBufferTime, and releases the target.'},
    {'function': 'LiftActionSkill.TargetBlocked', 'events': ['OnTargetBlocked'],
     'summary': 'Fires OnTargetBlocked; no lift and no LiftActionSkill timers.'},
    {'function': 'LiftActionSkill.FizzleOut', 'events': ['OnLiftFailed'],
     'summary': 'Fires OnLiftFailed and plays the miss impact along MissTraceDistance; after '
                'ReleaseBufferTime, Fizzled resets the skill cooldown and deactivates the skill.'},
]
# Functions on the path that have no script in this build (native).
NATIVE = [
    'WillowAutoAimStrategy.GetPreferredTarget (target choice, therefore range)',
    'SkillEffectManager.ActivateSkill / DeactivateSkill / IsSkillActive (skill effects, grades)',
    'GearboxFramework.BehaviorKernel.ActivateBehaviorEventFromScript (runs the provider events)',
    'PlayerSkillTree.GetSkillState / UpgradeSkill (bIsUnlocked: the tier rule)',
    'Engine.ResourcePool (cooldown pool consumption)',
    'GearboxFramework.SpecialMoveComponent.Play / Queue / Stop (animations)',
    'WeaponActionAvailable / HealthState / VehiclePassenger ExpressionEvaluator.Evaluate (skill constraints)',
    'GearboxFramework.FlagExpressionEvaluator.Evaluate (CanLiftTargetIf; how a FlagChain combines)',
]

# WillowAutoAimStrategyDefinition fields copied into the manifest. What the native
# GetPreferredTarget does with them is not known.
AUTO_AIM_FIELDS = ('MinTargetDistance', 'MaxTargetDistance', 'MaxSnapAngle', 'RadiusMultiplier', 'DistanceOffset',
                   'AcquireTime', 'SustainTime', 'ChangeTime')


def unpack(packed):
    """SubarrayData.ArrayIndexAndLength -> (start, length): index << 16 | length."""
    raw = (packed or {}).get('ArrayIndexAndLength', 0) & 0xFFFFFFFF
    return raw >> 16, raw & 0xFFFF


def tagged(package, path):
    """Decoded properties of one export and the names the reader could not decode."""
    data = json.loads(package.run('--properties', package.index[path], '--property-offset', 4,
                                  '--array-schema', package.schema))
    decoded = {p['name']: prepare_skill_tree.plain(p.get('value'))
               for p in data['properties'] if p.get('status') == 'decoded'}
    undecoded = sorted({p['name'] for p in data['properties'] if p.get('status') != 'decoded'})
    return decoded, undecoded


def read_provider(package, path):
    """A BehaviorProviderDefinition as sequences of events, behaviors and links.

    Structural oracle (as src/behavior.cpp): every event and behavior link range
    lies inside the sequence's link array and every link names an existing
    behavior. Also reported: whether the ranges tile the link array exactly and
    which behaviors no event reaches.
    """
    props, _ = tagged(package, path)
    sequences = []
    for data in props.get('BehaviorSequences') or []:
        links = [((l.get('LinkIdAndLinkedBehavior', 0) & 0xFFFFFFFF), l.get('ActivateDelay', 0.0))
                 for l in data.get('ConsolidatedOutputLinkData') or []]
        variables = data.get('VariableData') or []
        variable_links = data.get('ConsolidatedVariableLinkData') or []
        linked = data.get('ConsolidatedLinkedVariables') or []
        behaviors = data.get('BehaviorData2') or []
        ranges = []

        def outputs(packed):
            start, length = unpack(packed)
            if start + length > len(links):
                raise ValueError(f'{path}: link range {start}+{length} outside {len(links)} links')
            ranges.append((start, length))
            out = []
            for raw, delay in links[start:start + length]:
                target = raw & 0xFFFFFF
                if target >= len(behaviors):
                    raise ValueError(f'{path}: link to behavior {target} of {len(behaviors)}')
                out.append({'behavior': target, 'outputId': raw >> 24, 'delay': delay})
            return out

        def bindings(packed):
            start, length = unpack(packed)
            out = {}
            for entry in variable_links[start:start + length]:
                first, count = unpack(entry.get('LinkedVariables'))
                out[entry.get('PropertyName', '')] = [
                    {'variable': i, 'name': variables[i].get('Name'), 'type': variables[i].get('Type')}
                    for i in linked[first:first + count]]
            return out

        events = [{'name': e.get('UserData', {}).get('EventName'),
                   'enabled': e.get('UserData', {}).get('bEnabled', True),
                   'outputs': bindings(e.get('OutputVariables')),
                   'links': outputs(e.get('OutputLinks'))} for e in data.get('EventData2') or []]
        parsed = []
        for i, b in enumerate(behaviors):
            behavior_path = b.get('Behavior')
            own, undecoded = tagged(package, behavior_path)
            parsed.append({'index': i, 'id': behavior_path, 'class': package.classes.get(behavior_path),
                           'properties': own, 'undecodedProperties': undecoded,
                           'variables': bindings(b.get('LinkedVariables')),
                           'links': outputs(b.get('OutputLinks'))})
        covered = sorted(i for start, length in set(ranges) for i in range(start, start + length))
        reached, stack = set(), [l['behavior'] for e in events for l in e['links']]
        while stack:
            i = stack.pop()
            if i not in reached:
                reached.add(i)
                stack.extend(l['behavior'] for l in parsed[i]['links'])
        sequences.append({
            'name': data.get('BehaviorSequenceName'),
            'enabledOnSpawn': data.get('bEnabledOnSpawn', True),
            'events': events,
            'behaviors': parsed,
            'check': {'links': len(links), 'linksTiledExactly': covered == list(range(len(links))),
                      'unreachedBehaviors': [b['id'].rsplit('.', 1)[-1] for b in parsed if b['index'] not in reached]},
        })
    return {'id': path, 'sequences': sequences}


def combine(base, rows, grade_of):
    """(base + PreAdd) * (1 + Scale) + PostAdd over effect rows (UNVERIFIED rule,
    fitted for weapons). grade_of(row) gives the grade whose value applies."""
    sums = {'MT_PreAdd': 0.0, 'MT_Scale': 0.0, 'MT_PostAdd': 0.0}
    for row in rows:
        value = row['values'][grade_of(row)] if grade_of(row) < len(row['values']) else None
        if value is not None:
            sums[row['modifierType'] or 'MT_Scale'] += value
    return (base + sums['MT_PreAdd']) * (1 + sums['MT_Scale']) + sums['MT_PostAdd']


def timeline(lift, lock_seconds, time_scale, fade, release_buffer):
    """Seconds after the cast, as LiftActionSkill's script schedules them."""
    skill = lift + lock_seconds * time_scale
    return {
        'skillDuration': round(skill, 4),
        'lockedAt': round(lift, 4),
        'lockedFor': round(skill - lift - fade, 4),
        'outroAt': round(skill - fade, 4),
        'releasedAt': round(skill, 4),
        'endSkillAt': round(skill + release_buffer, 4),
    }


def skill_points(startup):
    """GD_Globals.Skills point rules: per level-up and total for a level."""
    out = {}
    for key, path in (('perLevelUp', 'GD_Globals.Skills.INI_SkillPointsPerLevelUp'),
                      ('totalForLevel', 'GD_Globals.Skills.INI_TotalSkillPointsForCurrentLevel')):
        props, _ = tagged(startup, path)
        rule = (props.get('ConditionalInitialization') or {}).get('ConditionalExpressionList') or []
        entries = []
        for entry in rule:
            value = entry.get('BaseValueIfTrue') or {}
            formula = None
            if value.get('InitializationDefinition'):
                f = tagged(startup, value['InitializationDefinition'])[0].get('ValueFormula') or {}
                formula = {k: f.get(k) for k in ('Multiplier', 'Level', 'Power', 'Offset')}
            entries.append({'when': entry.get('Expressions'), 'constant': value.get('BaseValueConstant'),
                            'formula': formula})
        out[key] = {'id': path, 'conditions': entries}
    return out


def branch_path(package, branch_path_id, upgrade, resolver):
    branch = package.props(branch_path_id)
    tiers = []
    for number, tier in enumerate(branch['Tiers']):
        skills = []
        for skill in tier.get('Skills') or []:
            props = package.props(skill)
            entry = {'id': skill, 'name': props.get('SkillName', ''), 'maxGrade': props.get('MaxGrade', 1),
                     'onPath': skill in upgrade}
            entry['effects'] = skill_stats.effect_rows(package, props, resolver, entry['maxGrade'])
            skills.append(entry)
        tiers.append({'tier': number, 'pointsToUnlockNext': tier.get('PointsToUnlockNextTier', 0), 'skills': skills})
        if any(s['id'] == upgrade[-1] for s in skills):
            break
    return {'id': branch_path_id, 'name': branch.get('BranchName', ''), 'tiers': tiers}


def with_class_defaults(own, defaults, names):
    """Each name from the object's own tags, else from its class default object, else 0.

    A name tagged in neither was never set in defaultproperties, so UE3 leaves it
    zero-initialised. Those names are returned separately so the manifest says so."""
    values, zero = {}, []
    for name in names:
        if name in own:
            values[name] = own[name]
        elif name in defaults:
            values[name] = defaults[name]
        else:
            values[name] = 0
            zero.append(name)
    return values, zero


def owner(packages, path):
    """The first package that exports path."""
    for package in packages:
        if path in package.index:
            return package
    raise KeyError(f'{path} is in none of {[p.path.name for p in packages]}')


def constraint_evaluators(constraints, packages):
    """SkillConstraints as one row per evaluator: class, own properties and when it applies.

    An entry names its evaluator directly (Evaluator) or through
    SkillExpressionEvaluatorDefinitions (EvaluatorDefinitions). Evaluate() is
    native for every evaluator class used here, so the rows say what is tested
    only through the class name and properties."""
    rows = []
    for constraint in constraints:
        evaluators = [constraint['Evaluator']] if constraint.get('Evaluator') else []
        for definition in constraint.get('EvaluatorDefinitions') or []:
            evaluator = owner(packages, definition).props(definition).get('Evaluator')
            if not evaluator:
                raise ValueError(f'{definition} names no Evaluator')
            evaluators.append(evaluator)
        for evaluator in evaluators:
            package = owner(packages, evaluator)
            rows.append({'evaluator': evaluator, 'class': package.classes[evaluator],
                         'properties': package.props(evaluator),
                         'onActivation': constraint.get('bApplyConstraintOnActivatation'),
                         'whileActive': constraint.get('bApplyConstraintWhileActive'),
                         'whilePaused': constraint.get('bApplyConstraintWhilePaused'),
                         'onFailure': constraint.get('OnFailure')})
    return rows


def flag_chain(packages, evaluator):
    """A FlagExpressionEvaluator's FlagChain as [{flag, test, derivedFrom}].

    derivedFrom is the flag's own EvaluationExpression chain, one level down
    (Flag_Skills_CanPhaseLock is computed from other flags). How a chain
    combines its tests is native."""
    def chain(path):
        props = owner(packages, path).props(path)
        return [{'flag': link.get('FlagDefinition'), 'test': link.get('FlagEvalType')} for link in props.get('FlagChain') or []]

    rows = chain(evaluator)
    for row in rows:
        expression = owner(packages, row['flag']).props(row['flag']).get('EvaluationExpression') if row['flag'] else None
        row['derivedFrom'] = chain(expression) if expression else []
    return {'evaluator': evaluator, 'class': owner(packages, evaluator).classes[evaluator], 'chain': rows}


def auto_aim(willow):
    """The player controller's auto-aim strategy data.

    WillowPlayerController.InitInputSystem (script) creates a WillowAutoAimStrategy
    and sets its DataDefinition to WillowGlobals.AutoAimDefinition. That the globals
    object carries its class default value is an inference (no instance found in
    the installed data). GetPreferredTarget, which picks the action-skill target, is native."""
    definition = willow.props('Default__WillowGlobals').get('AutoAimDefinition')
    settings, zero = with_class_defaults(willow.props(definition), willow.props('Default__WillowAutoAimStrategyDefinition'),
                                         AUTO_AIM_FIELDS)
    return {'definition': definition, 'class': willow.classes.get(definition), 'settings': settings,
            'untaggedReadAsZero': zero,
            'actionSkillCall': 'WillowPlayerController.StartActionSkill: GetPreferredTarget(Self, , '
                               'bGetInstantaneousTarget=True, ); the result is the target when it is a WillowPawn'}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--package', default='GD_Siren_Streaming_SF.upk')
    parser.add_argument('--tree', default='GD_Siren_Streaming.SkillTree.SkillTree_Siren')
    parser.add_argument('--class-definition', default='GD_Siren.Character.CharClass_Siren')
    parser.add_argument('--branch', default='GD_Siren_Skills.SkillTree.Branch_Motion')
    parser.add_argument('--upgrade', nargs='+', default=['GD_Siren_Skills.Motion.Ward', 'GD_Siren_Skills.Motion.Suspension'],
                        help='skills of the chosen path, in spend order; the last one is the Phaselock modifier')
    parser.add_argument('--output', default='local/character/action_skill_siren.json')
    args = parser.parse_args()

    cooked = Path(args.game) / 'WillowGame' / 'CookedPCConsole'
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    schema = output.parent / 'action_skill.schema'
    schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
    reader = str(Path(args.reader).resolve())
    packages = {name: prepare_skill_tree.Package(reader, cooked / name, str(schema.resolve()))
                for name in (args.package, 'Startup.upk', 'WillowGame.upk')}
    package, startup, willow = packages[args.package], packages['Startup.upk'], packages['WillowGame.upk']
    resolver = skill_stats.Resolver([package, startup])

    root = package.props(package.props(args.tree)['Root'])
    skill_id = root['Tiers'][0]['Skills'][0]
    skill = package.props(skill_id)
    archetype_id = skill['ActionSkillArchetype']
    archetype, _ = tagged(package, archetype_id)
    lift_defaults = willow.props('Default__LiftActionSkill')
    phaselock_def_defaults = willow.props('Default__PhaseLockDefinition')

    def setting(name):
        """Archetype value, else the LiftActionSkill class default (32-bit floats rounded)."""
        value = archetype.get(name, lift_defaults.get(name))
        return round(value, 6) if isinstance(value, float) else value

    # Cooldown: the class's skill cooldown pool and the attribute it reads.
    class_def = startup.props(args.class_definition)
    pool_id = class_def.get('SkillCooldownPoolDefinition')
    pool = startup.props(pool_id) if pool_id else {}
    cooldown_attribute = (pool.get('BaseMaxValue') or {}).get('BaseValueAttribute')
    cooldown = resolver.attribute(cooldown_attribute) if cooldown_attribute else None

    # Lock duration: LockDurationFormula on Maya, LockDurationScaleFormula on the target.
    duration_attribute = (archetype.get('LockDurationFormula') or {}).get('BaseValueAttribute')
    base_duration = resolver.attribute(duration_attribute)
    scale_attribute = (archetype.get('LockDurationScaleFormula') or {}).get('BaseValueAttribute')
    scale_default = None
    for candidate in package.index:
        if candidate.startswith(scale_attribute + '.') and package.classes[candidate].endswith('NounAttributeValueResolver'):
            scale_default = resolver.value(package, package.props(candidate).get('DefaultValue'))

    provider = read_provider(package, archetype['BehaviorProviderDefinition'])
    switched = sorted({b['properties'].get(k) for s in provider['sequences'] for b in s['behaviors']
                       for k in ('SkillToActivate', 'SkillToDeactivate') if b['properties'].get(k)})
    helpers = {}
    for helper in switched:
        if helper in package.index and helper.startswith(skill_id.rsplit('.', 1)[0] + '.'):
            props = package.props(helper)
            helpers[helper] = {'name': props.get('SkillName', ''), 'initialDuration': props.get('InitialDuration'),
                               'effects': skill_stats.effect_rows(package, props, resolver, 1)}

    # Event names must be functions of the action skill's class chain (script wrappers).
    functions = {p for p, c in willow.classes.items() if c == 'Core.Function'}
    event_check = {e['name']: any(f'{cls}.{e["name"]}' in functions for cls in ('LiftActionSkill', 'ActionSkill'))
                   for s in provider['sequences'] for e in s['events']}

    branch = branch_path(package, args.branch, args.upgrade, resolver)
    modifier = next(s for t in branch['tiers'] for s in t['skills'] if s['id'] == args.upgrade[-1])
    duration_rows = [r for r in modifier['effects'] if r['attribute'] == duration_attribute]
    diminishing = [r for h in helpers.values() for r in h['effects'] if r['attribute'] == scale_attribute]
    per_grade = []
    for grade in range(modifier['maxGrade'] + 1):
        lock = combine(base_duration, duration_rows, lambda r: grade)
        per_grade.append({
            'grade': grade,
            'lockDurationAttribute': round(lock, 4),
            'firstLock': timeline(setting('LiftDuration'), lock, scale_default, setting('LockFadeOutTime'),
                                  setting('ReleaseBufferTime')),
            'relockWithinDiminishingReturns': timeline(
                setting('LiftDuration'), lock, combine(scale_default, diminishing, lambda r: 0),
                setting('LockFadeOutTime'), setting('ReleaseBufferTime')),
        })
    tier_points = [t['pointsToUnlockNext'] for t in branch['tiers']]
    # Oracle: the modifier's own info-box line for the duration attribute must
    # show the same per-grade change as the lock duration computed above.
    modifier_props = package.props(modifier['id'])
    lines, _ = skill_stats.skill_lines(package, modifier['id'], modifier_props, resolver, {}, modifier['maxGrade'])
    shown = [float(line[1]) for g in range(1, modifier['maxGrade'] + 1) for line in lines[g] if line[1]]
    computed = [round(p['lockDurationAttribute'] - base_duration, 4) for p in per_grade[1:]]
    presentation_agrees = len(shown) == len(computed) and all(abs(a - b) < 1e-3 for a, b in zip(shown, computed))

    manifest = {
        'format': FORMAT,
        'packages': sorted(packages),
        'skill': {
            'id': skill_id, 'name': skill.get('SkillName', ''), 'maxGrade': skill.get('MaxGrade', 1),
            'playerLevelRequirement': skill.get('PlayerLevelRequirement'),
            'initialDuration': skill.get('InitialDuration'),
            'constraints': skill.get('SkillConstraints') or [],
            'constraintEvaluators': constraint_evaluators(skill.get('SkillConstraints') or [], [package, startup]),
            'effects': skill_stats.effect_rows(package, skill, resolver, skill.get('MaxGrade', 1)),
            'behaviorProvider': read_provider(package, skill['BehaviorProviderDefinition'])
            if skill.get('BehaviorProviderDefinition') else None,
        },
        'cooldown': {'pool': pool_id, 'attribute': cooldown_attribute, 'seconds': cooldown,
                     'baseConsumptionRate': pool.get('BaseConsumptionRate')},
        'actionSkill': {
            'id': archetype_id, 'class': package.classes.get(archetype_id),
            'settings': {name: setting(name) for name in (
                'LiftDuration', 'LockFadeOutTime', 'ReleaseBufferTime', 'RuinHoldTime', 'LiftSnapTimePct',
                'LiftSnapHeightPct', 'LiftBobAmplitude', 'LiftBobFrequency', 'MissTraceDistance',
                'BubbleFXIntroTime', 'BubbleFXOutroOverlapTime')},
            'lockDuration': {'attribute': duration_attribute, 'base': base_duration},
            'lockDurationScale': {'attribute': scale_attribute, 'default': scale_default},
            'phaselockedAttributeEffects': archetype.get('PhaselockedAttributeEffects') or [],
            'canLiftTargetIf': archetype.get('CanLiftTargetIf'),
            'canLiftTargetIfChain': flag_chain([package, startup, willow], archetype['CanLiftTargetIf'])
            if archetype.get('CanLiftTargetIf') else None,
            'phaseLockDefinitions': {'default': archetype.get('DefaultPhaseLockDef'),
                                     'byBodyTag': archetype.get('LiftBodyMap') or [],
                                     'classDefaults': {k: phaselock_def_defaults.get(k) for k in ('HeightFromGround', 'DropTime')}},
            'linkedSkills': {k: archetype.get(k) for k in ('CharmSkill', 'HeliosSkill', 'ResurrectSkill', 'SubsequenceSkill', 'RuinSkill')},
            'animations': {'hit': archetype.get('PhaselockSMD_Hit'), 'miss': archetype.get('PhaselockSMD_Miss')},
        },
        'helperSkills': helpers,
        'autoAim': auto_aim(willow),
        'behaviorProvider': provider,
        'eventNamesAreScriptFunctions': event_check,
        'scriptFlow': SCRIPT_FLOW,
        'native': NATIVE,
        'upgradePath': {
            'branch': branch,
            'spendOrder': args.upgrade,
            'actionSkillPointsToUnlockTrees': root['Tiers'][0].get('PointsToUnlockNextTier'),
            'branchPointsBeforeTier': [sum(tier_points[:i]) for i in range(len(tier_points))],
            # SkillTreeGFxObject.CanUpgradeSkill compares the player level with 5
            # (script constant); the tier unlock itself is native (bIsUnlocked).
            'minimumPlayerLevelToTrainFromScript': 5,
            'phaselockByModifierGrade': per_grade,
            'presentationAgreesWithDuration': presentation_agrees,
        },
        'skillPoints': skill_points(startup),
        'unresolved': sorted(resolver.unresolved),
    }
    output.write_text(json.dumps(manifest, indent=1, ensure_ascii=False), encoding='utf-8')
    sequence = provider['sequences'][0] if provider['sequences'] else {'events': [], 'behaviors': [], 'check': {}}
    print(f'wrote {output}: {len(sequence["events"])} events, {len(sequence["behaviors"])} behaviors, '
          f'links tiled={sequence["check"].get("linksTiledExactly")}, '
          f'unmatched event names={[n for n, ok in event_check.items() if not ok]}, '
          f'presentation agrees={presentation_agrees}')


if __name__ == '__main__':
    main()
