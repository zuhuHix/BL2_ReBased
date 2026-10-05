"""Synthetic mission-script bridge coverage (src/mission_script.*, src/progression.*, docs/verification/NATIVE_MISSION_SCRIPT_BRIDGE.md
and NATIVE_PROGRESSION.md).

Builds toy `Engine`, `GearboxFramework`, `WillowGame` and `TestMission` packages with invented classes (the names are the vocabulary
the bridge reads: WillowPlayerController, MissionTracker, WillowGameReplicationInfo, MissionDefinition, WorldInfo.GRI, Actor.Role,
the attribute classes) and hand-assembled bytecode, then runs `ow-package --mission-run ... script:accept script:turnin ...` and checks
results computed by hand. Nothing here comes from the game; the toy controller script is not the game's, it only calls what the bridge
binds and leaves a trace through the recorded ExpEarn calls (Exp 0, Source = a marker, so only the reward adds experience):

  UpdateMissionStatus(Mission, Status)  ->  ExpEarn(0, Status)
  IsMissionMoviePlaying()               ->  ExpEarn(0, 100)       (called by the tracker tick consuming the kickoff)
  AcceptMission(Mission, Director)      ->  WorldInfo.GRI.MissionTracker.ActivateMission(Mission, Self)
  ServerCompleteMission(Mission, Dir.)  ->  ...CompleteMission(Mission, Self); ...PlayTurnIn(Mission);
                                            ExpEarn(Mission.GetExperienceReward(Self, false), 5)
  ExpLevelUp(bCheated)                  ->  PRI.ExpLevel += 1; PRI.ExpPointsNextLevelAt = GetExpPointsRequiredForLevel(ExpLevel + 1)

The invented data: the experience curve f(n) = 2 x (n^2 + 1) (the level comes from a global-slot attribute), so R(n) = trunc f(n) - 4:
R(2) 6, R(3) 16, R(4) 30, R(5) 48, R(50) 4998; the reward percentage is an attribute chain, constant 0.5 times a conditional
(playthrough count == 2 gives 4, else 3), so 1.5 on the first playthrough. At game stage 4 the reward is
trunc((R(5) - R(4)) x 1.5) = trunc(18 x 1.5) = 27.
"""

import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

reader = str(Path(sys.argv[1]).resolve())


def w32(*values): return b''.join(struct.pack('<i', v) for v in values)
def u16(v): return struct.pack('<H', v)
def u32(v): return struct.pack('<I', v)
def u64(v): return struct.pack('<Q', v)


class Package:
    """Same toy package writer as tests/behavior_test.py."""
    def __init__(self):
        self.names = []
        self.imports = []
        self.exports = []

    def name(self, text):
        if text not in self.names: self.names.append(text)
        return self.names.index(text)

    def fname(self, text, number=0): return w32(self.name(text), number)

    def add_import_full(self, class_name, outer, name):
        self.imports.append((self.fname('Core'), self.fname(class_name), outer, self.fname(name)))
        return -len(self.imports)

    def add_import(self, class_name, name): return self.add_import_full(class_name, 0, name)

    def add_export(self, cls, name, payload, outer=0, super_ref=0):
        self.exports.append((cls, super_ref, outer, self.fname(name), payload))
        return len(self.exports)

    def build(self):
        self.name('None')
        table = b''.join(w32(len(n) + 1) + n.encode() + b'\0' + bytes(8) for n in self.names)
        io = 48 + len(table)
        import_data = b''.join(cp + cn + w32(outer) + nm for cp, cn, outer, nm in self.imports)
        eo = io + len(import_data)
        payload_at = eo + 68 * len(self.exports)
        export_data = b''
        payloads = b''
        for cls, sup, outer, name, payload in self.exports:
            export_data += w32(cls, sup, outer) + name + w32(0) + bytes(8) + w32(len(payload), payload_at + len(payloads)) \
                + w32(0) + w32(0) + bytes(20)
            payloads += payload
        header = w32(0x9e2a83c1 - (1 << 32), 832 | (46 << 16), 48, 0, 0, len(self.names), 48, len(self.exports), eo,
                     len(self.imports), io, 0)
        return header + table + import_data + export_data + payloads


class Tags:
    def __init__(self, p): self.p = p

    def tag(self, name, kind, size, body, detail=b''):
        return self.p.fname(name) + self.p.fname(kind) + w32(size, 0) + detail + body

    def int(self, name, v): return self.tag(name, 'IntProperty', 4, w32(v))
    def float(self, name, v): return self.tag(name, 'FloatProperty', 4, struct.pack('<f', v))
    def byte(self, name, v): return self.tag(name, 'ByteProperty', 1, bytes([v]), self.p.fname('None'))
    def struct_(self, name, type_, body): return self.tag(name, 'StructProperty', len(body), body, self.p.fname(type_))
    def bool(self, name, v): return self.tag(name, 'BoolProperty', 0, bytes([1 if v else 0]))
    def obj(self, name, ref): return self.tag(name, 'ObjectProperty', 4, w32(ref))
    def array(self, name, count, body): return self.tag(name, 'ArrayProperty', 4 + len(body), w32(count) + body)
    def none(self): return self.p.fname('None')


class Asm:
    """One function body; object references are 4 bytes in the file and 8 in memory (as in tests/vm_test.py)."""
    def __init__(self):
        self.b = bytearray()
        self.refs = 0

    def raw(self, *bs): self.b += bytes(bs)
    def ref(self, r): self.b += struct.pack('<i', r); self.refs += 1
    def i32(self, v): self.b += struct.pack('<i', v)
    def w(self, v): self.b += struct.pack('<H', v)
    def end(self): self.raw(0x53)

    def local(self, ids, name): self.raw(0x00); self.ref(ids[name])
    def instance(self, ref): self.raw(0x01); self.ref(ref)
    def context(self, obj, body):               # Context(obj, skip, return property, flags, body)
        self.raw(0x19); obj(); self.w(0); self.ref(0); self.raw(0); body()
    def call(self, function, *args):            # FinalFunction
        self.raw(0x1C); self.ref(function)
        for arg in args: arg()
        self.raw(0x16)
    def int_const(self, v): self.raw(0x1D); self.i32(v)
    def byte_const(self, v): self.raw(0x24, v)
    def self_(self): self.raw(0x17)
    def false(self): self.raw(0x28)
    def return_nothing(self): self.raw(0x04, 0x0B)


FUNC_NATIVE, FUNC_FINAL, FUNC_DEFINED, FUNC_PUBLIC = 0x400, 0x1, 0x2, 0x20000
CPF_PARM, CPF_OUT, CPF_OPT, CPF_RET = 0x80, 0x100, 0x10, 0x400
ROLES = ['ROLE_None', 'ROLE_SimulatedProxy', 'ROLE_AutonomousProxy', 'ROLE_Authority']


class Toy:
    """Reflection helpers over one package: classes, properties, structs, functions."""
    def __init__(self, kinds):
        self.p = Package()
        self.none = self.p.fname('None')
        self.imp = {n: self.p.add_import('Class', n) for n in
                    ('Class', 'Function', 'ScriptStruct', 'Enum') + tuple(k + 'Property' for k in kinds)}

    def cls(self, name, super_ref=0): return self.p.add_export(self.imp['Class'], name, w32(0) * 4, super_ref=super_ref)

    def prop(self, kind, owner, name, flags=0, type_ref=0):
        payload = w32(0) + self.none + w32(0) + u32(1) + u64(flags) + self.none + w32(0) + w32(type_ref)
        return self.p.add_export(self.imp[kind + 'Property'], name, payload, outer=owner)

    def array_of(self, owner, name, inner_kind, inner_ref=0):
        index = len(self.p.exports) + 1
        self.prop('Array', owner, name, type_ref=index + 1)
        self.prop(inner_kind, index, name, type_ref=inner_ref)
        return index

    def struct_(self, owner, name):
        return self.p.add_export(self.imp['ScriptStruct'], name, w32(0) * 4, outer=owner)

    def function(self, owner, name, declared, body=None, flags=FUNC_FINAL | FUNC_DEFINED | FUNC_PUBLIC, result=None, native=0, friendly=None):
        """declared: [(kind, name, extra flags)]; `body(ids)` returns an Asm (None for a native). Children are exported
        first, in reverse declaration order, the return value before them (as in the real packages)."""
        ordered = ([(result, 'ReturnValue', CPF_PARM | CPF_OUT | CPF_RET)] if result else []) + \
                  [(k, n, CPF_PARM | extra) for k, n, extra in reversed(declared)]
        func_index = len(self.p.exports) + 1 + len(ordered)
        ids = {}
        for kind, pname, pflags in ordered: ids[pname] = self.prop(kind, func_index, pname, pflags)
        asm = body(ids) if body else None
        script = bytes(asm.b) if asm else b''
        memory = len(script) + 4 * asm.refs if asm else 0
        payload = u16(0) + w32(*([0] * 10)) + w32(memory, len(script)) + script
        payload += u16(native) + bytes([0]) + u32(flags) + self.p.fname(friendly or name)
        assert self.p.add_export(self.imp['Function'], name, payload, outer=owner) == func_index
        ids['__self__'] = func_index
        return ids


ADD, SUB, MUL, DIV = 0, 1, 2, 3
OPERANDS = ['MATHRESOLVEROPERAND_Add', 'MATHRESOLVEROPERAND_Sub', 'MATHRESOLVEROPERAND_Mul', 'MATHRESOLVEROPERAND_Div']
COMPARISONS = ['OPERATOR_EqualTo', 'OPERATOR_NotEqualTo', 'OPERATOR_GreaterThan', 'OPERATOR_GreaterThanOrEqual',
               'OPERATOR_LessThan', 'OPERATOR_LessThanOrEqual']


def build_engine():
    toy = Toy(('Byte', 'Object', 'Bool', 'Float', 'Array', 'Struct'))
    p, T = toy.p, toy
    t = Tags(p)
    actor = T.cls('Actor')
    T.prop('Byte', actor, 'Role')
    T.prop('Object', actor, 'WorldInfo')
    p.add_export(toy.imp['Enum'], 'ENetRole', w32(0) + toy.none + w32(0) + w32(len(ROLES)) + b''.join(p.fname(r) for r in ROLES), outer=actor)
    T.prop('Object', T.cls('WorldInfo'), 'GRI')
    # AttributeInitializationData: its own default makes BaseValueScaleConstant 1 (a field the data omits keeps that).
    aid = p.add_export(toy.imp['ScriptStruct'], 'AttributeInitializationData', bytes(52) + t.float('BaseValueScaleConstant', 1.0) + toy.none)
    T.prop('Float', aid, 'BaseValueConstant')
    T.prop('Object', aid, 'BaseValueAttribute')
    T.prop('Object', aid, 'InitializationDefinition')
    T.prop('Float', aid, 'BaseValueScaleConstant')
    formula = T.struct_(0, 'ValueFormula')
    T.prop('Bool', formula, 'bEnabled')
    for term in ('Multiplier', 'Level', 'Power', 'Offset'): T.prop('Struct', formula, term, type_ref=aid)
    expression = T.struct_(0, 'AttributeExpressionData')
    T.prop('Object', expression, 'AttributeOperand1')
    T.prop('Byte', expression, 'ComparisonOperator')
    T.prop('Byte', expression, 'Operand2Usage')
    T.prop('Object', expression, 'AttributeOperand2')
    T.prop('Float', expression, 'ConstantOperand2')
    entry = T.struct_(0, 'ConditionalAttributeInitializationData')
    T.prop('Struct', entry, 'BaseValueIfTrue', type_ref=aid)
    T.array_of(entry, 'Expressions', 'Struct', expression)
    conditional = T.struct_(0, 'ConditionalInitialization')
    T.prop('Bool', conditional, 'bEnabled')
    T.array_of(conditional, 'ConditionalExpressionList', 'Struct', entry)
    T.prop('Struct', conditional, 'DefaultBaseValue', type_ref=aid)
    definition = T.cls('AttributeInitializationDefinition')
    T.prop('Struct', definition, 'ValueFormula', type_ref=formula)
    T.prop('Struct', definition, 'ConditionalInitialization', type_ref=conditional)
    T.prop('Byte', definition, 'BaseValueMode')
    attribute = T.cls('AttributeDefinition')
    T.array_of(attribute, 'ContextResolverChain', 'Object')
    T.array_of(attribute, 'ValueResolverChain', 'Object')
    p.add_export(toy.imp['Enum'], 'EComparisonOperator', w32(0) + toy.none + w32(0) + w32(len(COMPARISONS)) + b''.join(p.fname(c) for c in COMPARISONS),
                 outer=T.cls('AttributeExpression'))
    return p


def build_gearbox():
    toy = Toy(('Float', 'Struct', 'Byte'))
    p, T = toy.p, toy
    engine = p.add_import_full('Package', 0, 'Engine')
    aid = p.add_import_full('ScriptStruct', engine, 'AttributeInitializationData')
    T.cls('NoContextNeededAttributeContextResolver')
    T.prop('Float', T.cls('ConstantAttributeValueResolver'), 'ConstantValue')
    math = T.cls('SimpleMathValueResolver')
    T.prop('Struct', math, 'Argument', type_ref=aid)
    T.prop('Byte', math, 'Operand')
    p.add_export(toy.imp['Enum'], 'EMathValueResolverOperand', w32(0) + toy.none + w32(0) + w32(len(OPERANDS)) + b''.join(p.fname(o) for o in OPERANDS),
                 outer=math)
    return p


def build_willowgame():
    toy = Toy(('Int', 'Bool', 'Byte', 'Object', 'Array', 'Struct'))
    p, T = toy.p, toy
    engine = p.add_import_full('Package', 0, 'Engine')
    aid = p.add_import_full('ScriptStruct', engine, 'AttributeInitializationData')
    actor = p.add_import_full('Class', engine, 'Actor')
    world_info, gri = (p.add_import_full('ObjectProperty', actor, 'WorldInfo'),
                       p.add_import_full('ObjectProperty', p.add_import_full('Class', engine, 'WorldInfo'), 'GRI'))
    native = FUNC_NATIVE | FUNC_PUBLIC

    # Operators the toy ExpLevelUp uses: Object.+(int,int), registered by the core natives under native number 146.
    T.function(T.cls('Object'), 'Add_IntInt', [('Int', 'P0', 0), ('Int', 'P1', 0)], None, FUNC_NATIVE | 0x1000 | 0x23000, 'Int', native=146, friendly='+')
    T.cls('GlobalAttributeValueResolver')
    reward = T.struct_(0, 'MissionRewardData')
    for field in ('ExperienceRewardPercentage', 'CreditRewardMultiplier', 'OtherCurrencyReward'): T.prop('Struct', reward, field, type_ref=aid)
    T.prop('Byte', reward, 'CurrencyRewardType')
    T.array_of(reward, 'RewardItems', 'Object')
    T.array_of(reward, 'RewardItemPools', 'Object')
    mission = T.cls('MissionDefinition')
    T.prop('Struct', mission, 'Reward', type_ref=reward)
    T.prop('Struct', mission, 'AlternativeReward', type_ref=reward)
    T.array_of(mission, 'ObjectiveSetDefs', 'Object')
    T.prop('Object', mission, 'InitialObjectiveSet')
    T.prop('Object', mission, 'BehaviorProvider')
    T.prop('Bool', mission, 'bActivateInitialObjectiveSet')
    get_xp = T.function(mission, 'GetExperienceReward', [('Object', 'InWPC', 0), ('Bool', 'bGetAltReward', 0)], None, native, 'Int')['__self__']
    T.function(mission, 'GetGameStage', [], None, native, 'Int')
    T.function(mission, 'GetCurrencyRewardType', [('Bool', 'bGetAltReward', 0)], None, native, 'Byte')
    T.function(mission, 'GetCurrencyReward', [('Object', 'InWPC', 0), ('Bool', 'bGetAltReward', 0)], None, native, 'Int')
    T.function(mission, 'ShouldGrantAlternateReward', [('Array', 'ObjectivesProgress', CPF_OUT)], None, native, 'Bool')
    T.function(mission, 'GetItemRewardsForPlayer', [('Object', 'WillowPC', 0), ('Object', 'MissionReward', CPF_OUT)], None, native)
    sets = T.cls('MissionObjectiveSetDefinition')
    T.array_of(sets, 'ObjectiveDefinitions', 'Object')
    T.prop('Object', sets, 'NextSet')
    T.prop('Bool', sets, 'bCanCompleteMission')
    T.prop('Bool', sets, 'bAutoEnableNextSet')
    objective = T.cls('MissionObjectiveDefinition')
    T.prop('Int', objective, 'ObjectiveCount')
    T.prop('Bool', objective, 'bRememberItemsWithinObjective')

    tracker = T.cls('MissionTracker')
    pair = [('Object', 'InMission', 0), ('Object', 'WillowPC', CPF_OPT)]
    activate = T.function(tracker, 'ActivateMission', pair, None, native)['__self__']
    complete = T.function(tracker, 'CompleteMission', pair, None, native)['__self__']
    play_turn_in = T.function(tracker, 'PlayTurnIn', [('Object', 'InMission', 0)], None, native)['__self__']
    T.function(tracker, 'GetMissionStatus', [('Object', 'InMission', 0)], None, native, 'Byte')
    T.function(tracker, 'IsDataValid', [], None, native, 'Bool')
    def no_delegates(ids):
        a = Asm()
        a.return_nothing(); a.end(); return a
    T.function(tracker, 'TriggerMissionStatusChangedDelegates', [], no_delegates)

    replication = T.cls('WillowGameReplicationInfo')
    tracker_prop = T.prop('Object', replication, 'MissionTracker')

    status_owner = T.cls('IMission')
    status = T.struct_(status_owner, 'MissionStatusPlayerData')
    T.prop('Object', status, 'MissionDef')
    T.prop('Byte', status, 'Status')
    T.prop('Bool', status, 'bNeedsRewards')
    pri = T.cls('WillowPlayerReplicationInfo')
    exp_level = T.prop('Int', pri, 'ExpLevel')
    next_at = T.prop('Int', pri, 'ExpPointsNextLevelAt')
    controller = T.cls('WillowPlayerController', super_ref=actor)
    pri_prop = T.prop('Object', controller, 'PlayerReplicationInfo')
    T.function(controller, 'GetMaxExpLevel', [], None, native, 'Int')
    required = T.function(controller, 'GetExpPointsRequiredForLevel', [('Int', 'Level', 0)], None, native, 'Int')['__self__']
    playthrough = T.struct_(controller, 'MissionPlaythroughData')
    T.array_of(playthrough, 'MissionList', 'Struct', status)
    T.array_of(controller, 'MissionPlaythroughs', 'Struct', playthrough)
    T.function(controller, 'NativeGetMissionIndex', [('Object', 'InMission', 0)], None, native, 'Int')
    exp_earn = T.function(controller, 'ExpEarn', [('Int', 'Exp', 0), ('Byte', 'Source', 0), ('Byte', 'ExpType', CPF_OPT)], None, native)['__self__']

    def on_tracker(a, body):         # Self.WorldInfo.GRI.MissionTracker.<body>
        a.context(lambda: a.context(lambda: a.context(lambda: a.instance(world_info), lambda: a.instance(gri)),
                                    lambda: a.instance(tracker_prop)), body)

    def accept(ids):
        a = Asm()
        on_tracker(a, lambda: a.call(activate, lambda: a.local(ids, 'Mission'), a.self_))
        a.return_nothing(); a.end(); return a
    T.function(controller, 'AcceptMission', [('Object', 'Mission', 0), ('Object', 'MissionDirector', 0)], accept)

    def complete_mission(ids):
        a = Asm()
        on_tracker(a, lambda: a.call(complete, lambda: a.local(ids, 'Mission'), a.self_))
        on_tracker(a, lambda: a.call(play_turn_in, lambda: a.local(ids, 'Mission')))
        a.call(exp_earn,
               lambda: a.context(lambda: a.local(ids, 'Mission'), lambda: a.call(get_xp, a.self_, a.false)),
               lambda: a.byte_const(5))
        a.return_nothing(); a.end(); return a
    T.function(controller, 'ServerCompleteMission', [('Object', 'Mission', 0), ('Object', 'MissionDirector', 0)], complete_mission)

    def update(ids):
        a = Asm()
        a.call(exp_earn, lambda: a.int_const(0), lambda: a.local(ids, 'NewMissionStatus'))
        a.return_nothing(); a.end(); return a
    T.function(controller, 'UpdateMissionStatus', [('Object', 'Mission', 0), ('Byte', 'NewMissionStatus', 0)], update)

    def movie(ids):
        a = Asm()
        a.call(exp_earn, lambda: a.int_const(0), lambda: a.byte_const(100))
        a.raw(0x04); a.false(); a.end(); return a
    T.function(controller, 'IsMissionMoviePlaying', [], movie, result='Bool')

    def level_of_pri(a, member):     # Self.PlayerReplicationInfo.<member>
        a.context(lambda: a.instance(pri_prop), lambda: a.instance(member))

    def level_up(ids):
        a = Asm()
        a.raw(0x0F); level_of_pri(a, exp_level)
        a.raw(146); level_of_pri(a, exp_level); a.int_const(1); a.raw(0x16)
        a.raw(0x0F); level_of_pri(a, next_at)
        a.call(required, lambda: (a.raw(146), level_of_pri(a, exp_level), a.int_const(1), a.raw(0x16)))
        a.return_nothing(); a.end(); return a
    T.function(controller, 'ExpLevelUp', [('Bool', 'bCheated', 0)], level_up)
    return p


def build_mission():
    """ToyMission: one set SetA {Only (count 1)} with bCanCompleteMission, activated at acceptance; a reward percentage that is an
    attribute chain (constant 0.5 x a conditional on the playthrough count) and the experience curve definition at its stock path."""
    p = Package()
    t = Tags(p)
    chains = {}

    def chain(*path):
        if path in chains: return chains[path]
        outer = chain(*path[:-1]) if len(path) > 1 else 0
        chains[path] = p.add_import_full('Package' if len(path) == 1 else 'Class', outer, path[-1])
        return chains[path]

    none = t.none()
    mission = p.add_export(chain('WillowGame', 'MissionDefinition'), 'ToyMission', b'')
    only = p.add_export(chain('WillowGame', 'MissionObjectiveDefinition'), 'Only', w32(0) + t.int('ObjectiveCount', 1) + none, outer=mission)
    set_a = p.add_export(chain('WillowGame', 'MissionObjectiveSetDefinition'), 'SetA',
                         w32(0) + t.array('ObjectiveDefinitions', 1, w32(only)) + t.bool('bCanCompleteMission', True) + none, outer=mission)

    def aid(**fields):               # AttributeInitializationData tag body: BaseValueConstant / BaseValueAttribute / InitializationDefinition
        body = b''
        if 'constant' in fields: body += t.float('BaseValueConstant', fields['constant'])
        if 'attribute' in fields: body += t.obj('BaseValueAttribute', fields['attribute'])
        if 'definition' in fields: body += t.obj('InitializationDefinition', fields['definition'])
        return body + none

    def data(name, **fields): return t.struct_(name, 'AttributeInitializationData', aid(**fields))
    context = p.add_export(chain('GearboxFramework', 'NoContextNeededAttributeContextResolver'), 'ContextResolver', w32(0) + none)
    # the experience curve: Level is read from the global slot through an attribute
    global_resolver = p.add_export(chain('WillowGame', 'GlobalAttributeValueResolver'), 'GlobalResolver', w32(0) + none)
    level_attribute = p.add_export(chain('Engine', 'AttributeDefinition'), 'LevelAttribute',
                                   w32(0) + t.array('ContextResolverChain', 1, w32(context)) + t.array('ValueResolverChain', 1, w32(global_resolver)) + none)
    formula = (t.bool('bEnabled', True) + data('Multiplier', constant=2.0) + data('Level', attribute=level_attribute)
               + data('Power', constant=2.0) + data('Offset', constant=1.0) + none)
    curve_package = p.add_export(chain('Core', 'Package'), 'GD_Balance_Experience', w32(0) + none)
    curve_formulas = p.add_export(chain('Core', 'Package'), 'Formulas', w32(0) + none, outer=curve_package)
    p.add_export(chain('Engine', 'AttributeInitializationDefinition'), 'Init_ExperienceRequiredForLevel',
                 w32(0) + t.struct_('ValueFormula', 'ValueFormula', formula) + none, outer=curve_formulas)
    # the reward percentage: 0.5, then x (4 when PlayThroughCount == 2, else 3)
    play_through = p.add_export(chain('Engine', 'AttributeDefinition'), 'PlayThroughCount', w32(0) + none)
    expression = (t.obj('AttributeOperand1', play_through) + t.byte('ComparisonOperator', COMPARISONS.index('OPERATOR_EqualTo'))
                  + t.float('ConstantOperand2', 2.0) + none)
    entry = (data('BaseValueIfTrue', constant=4.0)
             + t.array('Expressions', 1, expression))
    conditional = (t.bool('bEnabled', True)
                   + t.array('ConditionalExpressionList', 1, entry)
                   + data('DefaultBaseValue', constant=3.0) + none)
    multiplier = p.add_export(chain('Engine', 'AttributeInitializationDefinition'), 'PlaythroughMultiplier',
                              w32(0) + t.struct_('ConditionalInitialization', 'ConditionalInitialization', conditional) + none)
    constant_resolver = p.add_export(chain('GearboxFramework', 'ConstantAttributeValueResolver'), 'ConstantResolver',
                                     w32(0) + t.float('ConstantValue', 0.5) + none)
    math_resolver = p.add_export(chain('GearboxFramework', 'SimpleMathValueResolver'), 'MathResolver',
                                 w32(0) + data('Argument', definition=multiplier) + t.byte('Operand', MUL) + none)
    percentage = p.add_export(chain('Engine', 'AttributeDefinition'), 'XPReward',
                              w32(0) + t.array('ContextResolverChain', 1, w32(context))
                              + t.array('ValueResolverChain', 2, w32(constant_resolver, math_resolver)) + none)
    reward = t.struct_('Reward', 'MissionRewardData', data('ExperienceRewardPercentage', attribute=percentage) + none)
    cls_, sup, outer, name, _ = p.exports[mission - 1]
    p.exports[mission - 1] = (cls_, sup, outer, name, w32(0) + t.array('ObjectiveSetDefs', 1, w32(set_a))
                              + t.obj('InitialObjectiveSet', set_a) + t.bool('bActivateInitialObjectiveSet', True) + reward + none)
    return p


failures = []


def check(label, condition, detail=''):
    if not condition: failures.append(f'{label}: {detail}')


def mission_run(root, *steps):
    proc = subprocess.run([reader, str(root / 'TestMission.upk'), '--mission-run', 'ToyMission', '--cooked', str(root), *steps],
                          capture_output=True, text=True, encoding='utf-8')
    assert proc.stdout.strip(), ('no JSON output', proc.returncode, proc.stderr)
    return proc.returncode, json.loads(proc.stdout)


def earned_of(script): return [(e['amount'], e['source'], e['type']) for e in script['exp_earned']]


with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    (root / 'Engine.upk').write_bytes(build_engine().build())
    (root / 'GearboxFramework.upk').write_bytes(build_gearbox().build())
    (root / 'WillowGame.upk').write_bytes(build_willowgame().build())
    (root / 'TestMission.upk').write_bytes(build_mission().build())

    # Scenario A: stage 4 at acceptance (locked then: the later stage:9 changes nothing), a level-3 player at the curve point R(3) = 16.
    code, got = mission_run(root, 'stage:4', 'player:3:16', 'script:accept', 'script:accept', 'stage:9', 'tick:0.5', 'tick:0.5',
                            'obj:Only', 'script:turnin', 'pool')
    check('A exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    steps = [step for step in got['steps'] if not step['step'].startswith(('stage:', 'player:'))]
    check('A steps reported', len(steps) == 7, len(steps))
    if len(steps) == 7:
        def kinds(step): return [e['kind'] for e in steps[step]['effects']]
        # AcceptMission -> ActivateMission: Active, the initial set; a pending kickoff record is written (not played yet)
        check('A accept', steps[0]['ok'] and steps[0]['status'] == 1 and steps[0]['kickoff_pending']
              and kinds(0) == ['status', 'objective_set_active'], steps[0])
        # a second accept is refused by the native itself (status is not NotStarted): no new status, no hook
        check('A second accept refused', not steps[1]['ok'] and steps[1]['status'] == 1 and kinds(1) == [], steps[1])
        # the tracker tick consumes the record once
        check('A tick consumes kickoff', not steps[2]['kickoff_pending'] and not steps[3]['kickoff_pending'], (steps[2], steps[3]))
        check('A objective makes the mission ready', steps[4]['status'] == 2, steps[4])
        # ServerCompleteMission -> CompleteMission (Complete), PlayTurnIn, then the script's own reward call
        check('A turn-in', steps[5]['ok'] and steps[5]['status'] == 3 and kinds(5) == ['status', 'reward'], steps[5])
    script = got.get('script')
    check('A script report present', script is not None, got)
    if script:
        # hook Active (1), kickoff tick (IsMissionMoviePlaying, once), hook ReadyToTurnIn (3), hook Complete (4), then the reward:
        # trunc(18 x 1.5) = 27 at the stage locked at acceptance (4), not the later 9
        want = [(0, 1, -1), (0, 100, -1), (0, 3, -1), (0, 4, -1), (27, 5, -1)]
        check('A recorded ExpEarn calls in order', earned_of(script) == want, earned_of(script))
        # 16 + 27 = 43 in the pool; the pool update ran ExpLevelUp from level 3 to 4 (R(4) = 30 <= 43 < R(5) = 48) and stopped
        check('A pool and level', script['experience_pool'] == 43 and script['player_level'] == 4, (script['experience_pool'], script['player_level']))
        # every native the toy script reaches is bound
        check('A no stubs', script['stubs'] == [], script['stubs'])
        check('A no VM notes', script['notes'] == [], script['notes'])

    # Scenario B: the pool never passes the experience of the maximum level (R(50) = 4998); the level-up loop stops at 50.
    code, got = mission_run(root, 'stage:4', 'player:3:4990', 'script:accept', 'obj:Only', 'script:turnin', 'pool')
    check('B exit', code == 0 and got['errors'] == [], (code, got['errors']))
    check('B clamp and cap', got['script']['experience_pool'] == 4998 and got['script']['player_level'] == 50,
          (got['script']['experience_pool'], got['script']['player_level']))

    # Scenario C: no region stage supplied: stage 0, so the span R(1) - R(0) is 0 and the reward is 0: the pool stays put.
    code, got = mission_run(root, 'player:3:16', 'script:accept', 'obj:Only', 'script:turnin', 'pool')
    check('C exit', code == 0 and got['errors'] == [], (code, got['errors']))
    check('C no region stage', got['script']['experience_pool'] == 16 and got['script']['player_level'] == 3,
          (got['script']['experience_pool'], got['script']['player_level']))

if failures:
    print(f'{len(failures)} mission script check(s) failed:')
    for failure in failures: print('  - ' + failure)
    sys.exit(1)
print('mission script synthetic coverage passed.')
