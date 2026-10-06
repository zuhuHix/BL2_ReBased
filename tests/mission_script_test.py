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

The waypoint scenario (--slice-run, scenario F) puts a toy WillowWaypoint at the stock placed-actor path of the toy `Sanctuary_Dynamic`
package; its toy script (Touch, ProcessPlayerTouch, the set-changed reaction over its Touching list, PostBeginPlay registering it as a
mission observer) mirrors the stock one in structure, over invented classes. The controller's toy UpdateMissionObjective records the
marker ExpEarn(0, 210), so the number of such records is the number of applied objective updates the controller was told about.

The director scenario (--mission-run on `DirectorMission`, scenario G) adds a toy Marcus, GD_Marcus.Character.Pawn_Marcus (a WillowAIPawn), whose
MissionDirectives table has five invented entries (another mission that begins, ToyMission that begins and ends, another that only ends,
ToyMission ending on branch 1 and on branch 2). Its three toy list functions (GetEligible/InProgress/RedeemableMissions) mirror the stock ones
in structure and call the bridge's CanStartMission / CanEndMission / GetMissionStatus / GetCompletedBranch; its OnPlayerAcceptedMission /
OnPlayerTurnedInMission leave the markers ExpEarn(0, 120) / (0, 121). The toy controller calls them when AcceptMission / ServerCompleteMission
get a director.

The dialog scenario (--mission-run, scenario E) uses a toy mission whose provider runs Behavior_TriggerDialogEvent behaviors over a toy
dialog group (invented tags, priorities, acts, a talker name tag): Out on the first run, the dialog one kernel wake later, Finished when
the live line ends, the priority arbitration with the tracked-mission floor, the last enabled entry for a tag, a template act through the
link table, a registered pawn against the echo caller, and bForcePlayImmediate.

The dummy-sequence scenario (--slice-run, last section) uses a toy `Sanctuary_Dynamic` package that holds a provider at the stock path
with sequences whose enable conditions are BehaviorSequenceEnableByMission objects (invented), and checks the remote events their
OnBehaviorSequenceEnabled / Disabled behaviors emit, in order: the registration at spawn, the objective-state verdicts with the
status gating, ObjectiveSetRestrictions, mission-level conditions and bSequenceEnabledMutex.
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
    def name_(self, name, v): return self.tag(name, 'NameProperty', 8, self.p.fname(v))
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
    def struct_member(self, prop, struct, expr): self.raw(0x35); self.ref(prop); self.ref(struct); self.raw(0, 0); expr()
    def jump(self): self.raw(0x06); at = len(self.b); self.w(0); return at     # Jump(<target>); patch like jump_if_not
    def jump_if_not(self, cond):                # JumpIfNot(<target>, cond); returns the position to patch with the target statement
        self.raw(0x07); at = len(self.b); self.w(0); cond(); return at
    def patch(self, at): struct.pack_into('<H', self.b, at, self.here())   # the target is the next statement's in-memory offset
    def here(self): return len(self.b) + 4 * self.refs


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

    def struct_(self, owner, name, defaults=None):
        return self.p.add_export(self.imp['ScriptStruct'], name, w32(0) * 4 if defaults is None else bytes(52) + defaults, outer=owner)

    def function(self, owner, name, declared, body=None, flags=FUNC_FINAL | FUNC_DEFINED | FUNC_PUBLIC, result=None, native=0, friendly=None, locals_=()):
        """declared: [(kind, name, extra flags)]; `body(ids)` returns an Asm (None for a native). Children are exported
        first, in reverse declaration order, the return value before them (as in the real packages)."""
        ordered = ([(result, 'ReturnValue', CPF_PARM | CPF_OUT | CPF_RET)] if result else []) + \
                  [(k, n, CPF_PARM | extra) for k, n, extra in reversed(declared)] + [(k, n, 0) for k, n in locals_]
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
    toy = Toy(('Byte', 'Object', 'Bool', 'Float', 'Array', 'Struct', 'Str', 'Name'))
    p, T = toy.p, toy
    t = Tags(p)
    changes = ['CHANGE_Toggle', 'CHANGE_Enable', 'CHANGE_Disable']
    p.add_export(toy.imp['Enum'], 'EChangeStatus', w32(0) + toy.none + w32(0) + w32(len(changes)) + b''.join(p.fname(c) for c in changes),
                 outer=T.cls('ITargetable'))
    T.prop('Name', T.cls('Behavior_RemoteEvent'), 'EventName')
    behavior_base = T.cls('BehaviorBase')
    T.function(behavior_base, 'GetWorldInfo', [], None, FUNC_NATIVE | FUNC_PUBLIC, 'Object')
    contexts = ['BCONTEXT_Self', 'BCONTEXT_Instigator']
    p.add_export(toy.imp['Enum'], 'EBehaviorContext', w32(0) + toy.none + w32(0) + w32(len(contexts)) + b''.join(p.fname(c) for c in contexts),
                 outer=behavior_base)
    actor = T.cls('Actor')
    T.function(T.cls('PlayerController', super_ref=actor), 'IsPrimaryPlayer', [], None, FUNC_NATIVE | FUNC_PUBLIC, 'Bool')
    T.prop('Byte', actor, 'Role')
    T.prop('Object', actor, 'WorldInfo')
    T.prop('Object', actor, 'Owner')
    T.array_of(actor, 'Touching', 'Object')
    T.function(actor, 'IsPlayerOwned', [], None, FUNC_NATIVE | FUNC_PUBLIC, 'Bool')
    T.prop('Object', T.cls('Pawn', super_ref=actor), 'Controller')
    p.add_export(toy.imp['Enum'], 'ENetRole', w32(0) + toy.none + w32(0) + w32(len(ROLES)) + b''.join(p.fname(r) for r in ROLES), outer=actor)
    world = T.cls('WorldInfo', super_ref=actor)          # an Actor (Role) like the stock one
    T.prop('Object', world, 'GRI')
    T.prop('Bool', world, 'bIsMenuLevel')
    T.function(world, 'IsMenuLevel', [('Str', 'MapName', CPF_OPT)], None, FUNC_NATIVE | FUNC_PUBLIC | 0x2000, 'Bool')
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
    toy = Toy(('Float', 'Struct', 'Byte', 'Object', 'Int', 'Bool', 'Name', 'Array'))
    p, T = toy.p, toy
    t = Tags(p)
    engine = p.add_import_full('Package', 0, 'Engine')
    aid = p.add_import_full('ScriptStruct', engine, 'AttributeInitializationData')
    globals_ = T.cls('GearboxGlobals')
    T.prop('Object', globals_, 'TheBehaviorKernel')
    T.function(globals_, 'GetGearboxGlobals', [], None, FUNC_NATIVE | FUNC_PUBLIC | 0x2000, 'Object')
    T.function(globals_, 'GetBehaviorKernel', [], None, FUNC_NATIVE | FUNC_PUBLIC | 0x2000, 'Object')
    T.cls('NoContextNeededAttributeContextResolver')
    T.prop('Float', T.cls('ConstantAttributeValueResolver'), 'ConstantValue')
    math = T.cls('SimpleMathValueResolver')
    T.prop('Struct', math, 'Argument', type_ref=aid)
    T.prop('Byte', math, 'Operand')
    p.add_export(toy.imp['Enum'], 'EMathValueResolverOperand', w32(0) + toy.none + w32(0) + w32(len(OPERANDS)) + b''.join(p.fname(o) for o in OPERANDS),
                 outer=math)
    # Dialog vocabulary (names as the dialog code reads them)
    trigger = T.cls('Behavior_TriggerDialogEvent')
    T.prop('Object', trigger, 'EventTag')
    T.prop('Object', trigger, 'Group')
    T.prop('Object', trigger, 'NameTag')
    T.prop('Bool', trigger, 'bForcePlayImmediate')
    priority = T.cls('GearboxDialogPriority')
    tag = T.cls('GearboxDialogEventTag')
    T.prop('Object', tag, 'Priority')
    T.prop('Bool', tag, 'bSoundEffect')
    T.prop('Bool', tag, 'bGroupEvent')
    node = T.cls('GearboxDialogNode')
    T.prop('Int', node, 'NodeID')
    act = T.cls('GearboxDialogAct_Talk', super_ref=node)
    talk = T.struct_(0, 'GearboxDialogTalkData')
    T.prop('Object', talk, 'NameTag')
    T.prop('Object', talk, 'TalkAkEvent')
    T.prop('Int', talk, 'AkAudioUniqueID')
    T.prop('Float', talk, 'Pitch')
    T.array_of(act, 'TalkData', 'Struct', talk)
    T.prop('Bool', act, 'bInstigatorTalker')
    T.prop('Float', act, 'OutputDelay')
    event_data = T.struct_(0, 'DialogEventData')
    T.prop('Object', event_data, 'Tag')
    T.prop('Bool', event_data, 'bEnabled')
    T.prop('Object', event_data, 'OutputAction')
    talk_act = T.struct_(0, 'TalkActData')
    T.prop('Float', talk_act, 'OutputDelay')
    T.array_of(talk_act, 'TalkData', 'Struct', talk)
    T.prop('Object', talk_act, 'TalkerVariable')
    T.prop('Object', talk_act, 'OutputAction')
    T.prop('Bool', talk_act, 'bInstigatorTalker')
    link_struct = T.struct_(0, 'OutputLinkToStruct')
    for field in ('FromNodeID', 'LinkNumber', 'ToNodeID'): T.prop('Int', link_struct, field)
    group = T.cls('GearboxDialogGroup')
    T.array_of(group, 'DialogEvents', 'Struct', event_data)
    T.array_of(group, 'TalkActs', 'Struct', talk_act)
    T.array_of(group, 'OutputLinksToStructs', 'Struct', link_struct)
    T.array_of(group, 'Nodes', 'Object')
    T.prop('Object', group, 'ParentGroup')

    # The behavior provider vocabulary (tests/behavior_test.py has the commented version): sequences, events, behaviors, links.
    bpd = T.cls('BehaviorProviderDefinition')
    var_types = ['BVAR_None', 'BVAR_Object', 'BVAR_Int', 'BVAR_Float', 'BVAR_InstanceData', 'BVAR_NamedVariable', 'BVAR_Mystery']
    link_types = ['BVARLINK_Unknown', 'BVARLINK_Input', 'BVARLINK_Output']
    p.add_export(toy.imp['Enum'], 'EBehaviorVariableType', w32(0) + toy.none + w32(0) + w32(len(var_types)) + b''.join(p.fname(v) for v in var_types), outer=bpd)
    p.add_export(toy.imp['Enum'], 'EBehaviorVariableLinkType', w32(0) + toy.none + w32(0) + w32(len(link_types)) + b''.join(p.fname(v) for v in link_types), outer=bpd)
    sub = T.struct_(0, 'SubarrayData')
    T.prop('Int', sub, 'ArrayIndexAndLength')
    user = T.struct_(0, 'BehaviorEventUserData', t.bool('bEnabled', True) + toy.none)
    T.prop('Name', user, 'EventName')
    T.prop('Bool', user, 'bEnabled')
    T.prop('Int', user, 'MaxTriggerCount')
    T.prop('Float', user, 'ReTriggerDelay')
    event = T.struct_(0, 'BehaviorEventData2')
    T.prop('Struct', event, 'UserData', type_ref=user)
    T.prop('Struct', event, 'OutputVariables', type_ref=sub)
    T.prop('Struct', event, 'OutputLinks', type_ref=sub)
    data = T.struct_(0, 'BehaviorData2')
    T.prop('Object', data, 'Behavior')
    T.prop('Struct', data, 'LinkedVariables', type_ref=sub)
    T.prop('Struct', data, 'OutputLinks', type_ref=sub)
    link = T.struct_(0, 'BehaviorOutputLinkData')
    T.prop('Int', link, 'LinkIdAndLinkedBehavior')
    T.prop('Float', link, 'ActivateDelay')
    var = T.struct_(0, 'BehaviorVariableData')
    T.prop('Name', var, 'Name')
    T.prop('Byte', var, 'Type')
    vlink = T.struct_(0, 'BehaviorVariableLinkData2')
    T.prop('Name', vlink, 'PropertyName')
    T.prop('Byte', vlink, 'VariableLinkType')
    T.prop('Byte', vlink, 'ConnectionIndex')
    T.prop('Struct', vlink, 'LinkedVariables', type_ref=sub)
    seq = T.struct_(0, 'BehaviorSequenceData')
    T.prop('Name', seq, 'BehaviorSequenceName')
    T.prop('Bool', seq, 'bEnabledOnSpawn')
    T.prop('Bool', seq, 'bSequenceEnabledMutex')
    T.prop('Object', seq, 'CustomEnableCondition')
    T.array_of(seq, 'EventData2', 'Struct', event)
    T.array_of(seq, 'BehaviorData2', 'Struct', data)
    T.array_of(seq, 'VariableData', 'Struct', var)
    T.array_of(seq, 'ConsolidatedOutputLinkData', 'Struct', link)
    T.array_of(seq, 'ConsolidatedVariableLinkData', 'Struct', vlink)
    T.array_of(seq, 'ConsolidatedLinkedVariables', 'Int')
    T.array_of(bpd, 'BehaviorSequences', 'Struct', seq)
    return p


def build_willowgame():
    toy = Toy(('Int', 'Float', 'Bool', 'Byte', 'Object', 'Array', 'Struct', 'Name'))
    p, T = toy.p, toy
    engine = p.add_import_full('Package', 0, 'Engine')
    aid = p.add_import_full('ScriptStruct', engine, 'AttributeInitializationData')
    actor = p.add_import_full('Class', engine, 'Actor')
    player_controller = p.add_import_full('Class', engine, 'PlayerController')
    gearbox = p.add_import_full('Package', 0, 'GearboxFramework')
    gearbox_globals = p.add_import_full('Class', gearbox, 'GearboxGlobals')
    world_info, gri = (p.add_import_full('ObjectProperty', actor, 'WorldInfo'),
                       p.add_import_full('ObjectProperty', p.add_import_full('Class', engine, 'WorldInfo'), 'GRI'))
    native = FUNC_NATIVE | FUNC_PUBLIC

    # Operators the toy ExpLevelUp uses: Object.+(int,int), registered by the core natives under native number 146.
    object_cls = T.cls('Object')
    T.function(object_cls, 'Add_IntInt', [('Int', 'P0', 0), ('Int', 'P1', 0)], None, FUNC_NATIVE | 0x1000 | 0x23000, 'Int', native=146, friendly='+')
    T.function(object_cls, 'EqualEqual_IntInt', [('Int', 'P0', 0), ('Int', 'P1', 0)], None, FUNC_NATIVE | 0x1000 | 0x23000, 'Bool', native=154, friendly='==')
    T.function(object_cls, 'NotEqual_ObjectObject', [('Object', 'P0', 0), ('Object', 'P1', 0)], None, FUNC_NATIVE | 0x1000 | 0x23000, 'Bool', native=119, friendly='!=')
    T.cls('GlobalAttributeValueResolver')
    reward = T.struct_(0, 'MissionRewardData')
    for field in ('ExperienceRewardPercentage', 'CreditRewardMultiplier', 'OtherCurrencyReward'): T.prop('Struct', reward, field, type_ref=aid)
    T.prop('Byte', reward, 'CurrencyRewardType')
    T.array_of(reward, 'RewardItems', 'Object')
    T.array_of(reward, 'RewardItemPools', 'Object')
    # what FireMissionSlice reads at construction, and the dummy's enable condition class
    transforms = ['AIT_None', 'AIT_Transformed']
    p.add_export(toy.imp['Enum'], 'EAITransformed', w32(0) + toy.none + w32(0) + w32(len(transforms)) + b''.join(p.fname(x) for x in transforms),
                 outer=T.cls('AIPawnBalanceDefinition'))
    maths = ['MATH_Add', 'MATH_Sub']
    p.add_export(toy.imp['Enum'], 'EBinaryMathOperation', w32(0) + toy.none + w32(0) + w32(len(maths)) + b''.join(p.fname(x) for x in maths),
                 outer=T.cls('Behavior_SimpleMath'))
    mission_states = T.struct_(0, 'MissionStatesToLinkTo')
    for field in ('bNotStarted', 'bActive', 'bRequiredObjectivesComplete', 'bReadyToTurnIn', 'bComplete', 'bFailed'): T.prop('Bool', mission_states, field)
    objective_states = T.struct_(0, 'ObjectiveStatesToLinkTo')
    for field in ('bNotStarted', 'bActive', 'bComplete'): T.prop('Bool', objective_states, field)
    condition = T.cls('BehaviorSequenceEnableByMission')
    T.prop('Object', condition, 'LinkedMission')
    T.prop('Struct', condition, 'MissionStatesToLinkTo', type_ref=mission_states)
    T.prop('Bool', condition, 'bIsObjectiveSpecific')
    T.prop('Object', condition, 'LinkedObjective')
    T.prop('Struct', condition, 'ObjectiveStatesToLinkTo', type_ref=objective_states)
    T.array_of(condition, 'ObjectiveSetRestrictions', 'Object')
    gearbox_tag = p.add_import_full('Class', gearbox, 'GearboxDialogEventTag')
    gearbox_act = p.add_import_full('Class', gearbox, 'GearboxDialogAct_Talk')
    willow_tag = T.cls('WillowDialogEventTag', super_ref=gearbox_tag)
    for field in ('bOncePerSession', 'bMultiplayerOnly', 'bDoesNotOverrideSamePriority', 'bIsEchoEvent'): T.prop('Bool', willow_tag, field)
    T.prop('Object', T.cls('WillowDialogAct_Talk', super_ref=gearbox_act), 'Emote')
    globals_definition = T.cls('WillowDialogGlobalsDefinition')
    T.array_of(globals_definition, 'Priorities', 'Object')
    for field in ('ActiveMissionMinPriorityStart', 'ActiveSideMissionMinPriority', 'ActivePlotMissionMinPriority'): T.prop('Object', globals_definition, field)
    T.cls('WillowDialogNameTag')
    T.prop('Name', T.cls('Behavior_MissionRemoteEvent'), 'EventName')
    mission = T.cls('MissionDefinition')
    T.prop('Object', mission, 'MissionDialogGroup')
    T.prop('Bool', mission, 'bPlotCritical')
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

    engine_pawn = p.add_import_full('Class', engine, 'Pawn')
    T.cls('WillowPlayerPawn', super_ref=engine_pawn)
    ai_pawn = T.cls('WillowAIPawn', super_ref=engine_pawn)
    reactions = ['MissionReactionLevelLoad', 'MissionReactionStatusChanged', 'MissionReactionObjectiveSetChanged',
                 'MissionReactionObjectiveUpdated', 'MissionReactionObjectiveCleared', 'MissionReactionObjectiveComplete']
    imission = T.cls('IMission')
    # LevelLoad also names the mission (as the stock observers' does); the others take the tracker here
    def reaction_params(name): return [('Object', 'Tracker', 0)] + ([('Object', 'Mission', 0)] if name == 'MissionReactionLevelLoad' else [])
    for name in reactions: T.function(imission, name, reaction_params(name), None, native)
    tracker = T.cls('MissionTracker')
    for name, result in (('IsMissionObjectiveActive', 'Bool'), ('IsMissionObjectiveComplete', 'Bool'), ('IsObjectiveSetActive', 'Bool')):
        T.function(tracker, name, [('Object', 'Objective', 0)], None, native, result)
    update_objective = T.function(tracker, 'UpdateObjective', [('Object', 'Objective', 0), ('Int', 'ObjectiveBit', CPF_OPT)], None, native)['__self__']
    register_observer = T.function(tracker, 'RegisterMissionObserver', [('Object', 'Observer', 0), ('Object', 'Mission', 0)], None, native)['__self__']
    T.function(tracker, 'TriggerMissionObjectivesChangedDelegates', [('Object', 'Mission', 0)],
               lambda ids: (lambda a: (a.return_nothing(), a.end(), a)[2])(Asm()))
    pair = [('Object', 'InMission', 0), ('Object', 'WillowPC', CPF_OPT)]
    activate = T.function(tracker, 'ActivateMission', pair, None, native)['__self__']
    complete = T.function(tracker, 'CompleteMission', pair, None, native)['__self__']
    play_turn_in = T.function(tracker, 'PlayTurnIn', [('Object', 'InMission', 0)], None, native)['__self__']
    get_status = T.function(tracker, 'GetMissionStatus', [('Object', 'InMission', 0)], None, native, 'Byte')['__self__']
    can_start = T.function(tracker, 'CanStartMission', [('Object', 'InMission', 0)], None, native, 'Bool')['__self__']
    can_end = T.function(tracker, 'CanEndMission', [('Object', 'InMission', 0)], None, native, 'Bool')['__self__']
    completed_branch = T.function(tracker, 'GetCompletedBranch', [('Object', 'InMission', 0)], None, native, 'Byte')['__self__']
    is_valid = T.function(tracker, 'IsDataValid', [], None, native, 'Bool')['__self__']
    validate = T.function(tracker, 'ValidateData', [], None, native)['__self__']
    T.prop('Bool', tracker, 'bDataValidated')
    def no_delegates(ids):
        a = Asm()
        a.return_nothing(); a.end(); return a
    T.function(tracker, 'TriggerMissionStatusChangedDelegates', [], no_delegates)

    replication = T.cls('WillowGameReplicationInfo')
    tracker_prop = T.prop('Object', replication, 'MissionTracker')
    T.prop('Int', replication, 'CurrentPlaythrough')
    globals_ = T.cls('WillowGlobals', super_ref=gearbox_globals)
    T.function(globals_, 'GetWillowGlobals', [], None, native | 0x2000, 'Object')
    T.function(globals_, 'GetGlobalsDefinition', [], None, native, 'Object')
    globals_definition = T.cls('GlobalsDefinition')
    T.prop('Float', globals_definition, 'PlayerInteractionDistance')

    status_owner = T.cls('IMission')
    status = T.struct_(status_owner, 'MissionStatusPlayerData')
    T.prop('Object', status, 'MissionDef')
    T.prop('Byte', status, 'Status')
    T.prop('Bool', status, 'bNeedsRewards')
    pri = T.cls('WillowPlayerReplicationInfo')
    exp_level = T.prop('Int', pri, 'ExpLevel')
    next_at = T.prop('Int', pri, 'ExpPointsNextLevelAt')
    controller = T.cls('WillowPlayerController', super_ref=player_controller)
    T.function(controller, 'GetCurrentPlaythrough', [], None, native, 'Int')
    T.function(controller, 'GetHUDMovie', [], None, native, 'Object')
    T.function(controller, 'UpdateLcdMissionStatus', [], None, native)
    T.function(controller, 'PlayUIAkEvent', [('Object', 'Event', 0)], None, native)
    pri_prop = T.prop('Object', controller, 'PlayerReplicationInfo')
    T.function(controller, 'GetMaxExpLevel', [], None, native, 'Int')
    required = T.function(controller, 'GetExpPointsRequiredForLevel', [('Int', 'Level', 0)], None, native, 'Int')['__self__']
    playthrough = T.struct_(controller, 'MissionPlaythroughData')
    T.array_of(playthrough, 'MissionList', 'Struct', status)
    T.array_of(controller, 'MissionPlaythroughs', 'Struct', playthrough)
    T.function(controller, 'NativeGetMissionIndex', [('Object', 'InMission', 0)], None, native, 'Int')
    is_primary = p.add_import_full('Function', player_controller, 'IsPrimaryPlayer')
    is_menu = p.add_import_full('Function', p.add_import_full('Class', engine, 'WorldInfo'), 'IsMenuLevel')
    exp_earn = T.function(controller, 'ExpEarn', [('Int', 'Exp', 0), ('Byte', 'Source', 0), ('Byte', 'ExpType', CPF_OPT)], None, native)['__self__']

    # The toy director (a WillowAIPawn): the two callbacks leave markers (ExpEarn on the accepting controller), the three list functions mirror
    # the stock ones in structure (iterate the directive table, test each entry against the tracker), over the invented classes.
    director_calls = {}
    imd = T.cls('IMissionDirector')
    director_data = T.struct_(imd, 'MissionDirectorData')
    d_mission = T.prop('Object', director_data, 'MissionDefinition')
    d_begins = T.prop('Bool', director_data, 'bBeginsMission')
    d_ends = T.prop('Bool', director_data, 'bEndsMission')
    d_branch = T.prop('Byte', director_data, 'BranchEnding')
    directives = T.cls('MissionDirectivesDefinition')
    directives_array = T.array_of(directives, 'MissionDirectives', 'Struct', director_data)
    directives_prop = T.prop('Object', ai_pawn, 'MissionDirectives')

    def callback(marker):
        def body(ids):
            a = Asm()
            a.context(lambda: a.local(ids, 'PlayerAccepting'), lambda: a.call(exp_earn, lambda: a.int_const(0), lambda: a.byte_const(marker)))
            a.return_nothing(); a.end(); return a
        return body
    director_calls['OnPlayerAcceptedMission'] = T.function(ai_pawn, 'OnPlayerAcceptedMission', [('Object', 'PlayerAccepting', 0), ('Object', 'MissionAccepted', 0)], callback(120))['__self__']
    director_calls['OnPlayerTurnedInMission'] = T.function(ai_pawn, 'OnPlayerTurnedInMission', [('Object', 'PlayerAccepting', 0), ('Object', 'MissionTurnedIn', 0)], callback(121))['__self__']

    def director_list(name, test):
        def body(ids):
            a = Asm()
            member = lambda prop: (lambda: a.struct_member(prop, director_data, lambda: a.local(ids, 'Data')))
            tracker_call = lambda function: a.context(lambda: a.local(ids, 'Tracker'), lambda: a.call(function, member(d_mission)))
            def add():
                a.raw(0x55); a.raw(0x48); a.ref(ids['Out']); a.w(0); member(d_mission)(); a.raw(0x16)
                a.raw(0x0F); a.local(ids, 'Count'); a.raw(146); a.local(ids, 'Count'); a.int_const(1); a.raw(0x16)
            def int_equal(left, right): a.raw(154); left(); right(); a.raw(0x16)
            cast = lambda inner: (lambda: (a.raw(0x38, 58), inner()))
            a.raw(0x0F); a.local(ids, 'Count'); a.int_const(0)
            a.raw(0x0F); a.local(ids, 'Tracker')
            a.context(lambda: a.context(lambda: a.instance(world_info), lambda: a.instance(gri)), lambda: a.instance(tracker_prop))
            a.raw(0x58); a.context(lambda: a.instance(directives_prop), lambda: a.instance(directives_array)); a.local(ids, 'Data'); a.raw(0); a.raw(0x4A)
            end_at = len(a.b); a.w(0)
            test(a, ids, member, tracker_call, add, int_equal, cast)
            a.raw(0x31)
            struct.pack_into('<H', a.b, end_at, a.here())
            a.raw(0x30)
            a.raw(0x04); a.local(ids, 'Count'); a.end(); return a
        T.function(ai_pawn, name, [('Array', 'Out', CPF_OUT)], body, result='Int', locals_=[('Int', 'Count'), ('Struct', 'Data'), ('Object', 'Tracker')])

    def eligible(a, ids, member, tracker_call, add, int_equal, cast):     # begins and CanStartMission
        at1 = a.jump_if_not(lambda: (a.raw(0x2D), member(d_begins)()))
        at2 = a.jump_if_not(lambda: tracker_call(can_start))
        add()
        a.patch(at2); a.patch(at1)

    def redeemable(a, ids, member, tracker_call, add, int_equal, cast):   # ends, CanEndMission, branch None or the completed branch
        at1 = a.jump_if_not(lambda: (a.raw(0x2D), member(d_ends)()))
        at2 = a.jump_if_not(lambda: tracker_call(can_end))
        at3 = a.jump_if_not(lambda: int_equal(cast(member(d_branch)), lambda: (a.raw(0x38, 58), a.byte_const(0))))    # branch is None: add
        add()
        done = a.jump()
        a.patch(at3)
        at4 = a.jump_if_not(lambda: int_equal(cast(lambda: tracker_call(completed_branch)), cast(member(d_branch))))
        add()
        a.patch(at4); a.patch(done); a.patch(at2); a.patch(at1)

    def in_progress(a, ids, member, tracker_call, add, int_equal, cast):   # begins or ends, and the status is Active
        at1 = a.jump_if_not(lambda: (a.raw(0x2D), member(d_begins)()))
        check_status = a.jump()
        a.patch(at1)
        at2 = a.jump_if_not(lambda: (a.raw(0x2D), member(d_ends)()))
        a.patch(check_status)
        at3 = a.jump_if_not(lambda: int_equal(cast(lambda: tracker_call(get_status)), lambda: (a.raw(0x38, 58), a.byte_const(1))))
        add()
        a.patch(at3); a.patch(at2)

    director_list('GetEligibleMissions', eligible)
    director_list('GetRedeemableMissions', redeemable)
    director_list('GetInProgressMissions', in_progress)

    def update_hook(ids):
        a = Asm()
        a.call(exp_earn, lambda: a.int_const(0), lambda: a.byte_const(210))
        a.return_nothing(); a.end(); return a
    T.function(controller, 'UpdateMissionObjective', [('Object', 'MissionObjective', 0), ('Int', 'ObjectiveBit', 0)], update_hook)

    def on_tracker(a, body):         # Self.WorldInfo.GRI.MissionTracker.<body>
        a.context(lambda: a.context(lambda: a.context(lambda: a.instance(world_info), lambda: a.instance(gri)),
                                    lambda: a.instance(tracker_prop)), body)

    def with_director(a, ids, name):     # if (MissionDirector != None) MissionDirector.<name>(Self, Mission)
        at = a.jump_if_not(lambda: (a.raw(119), a.local(ids, 'MissionDirector'), a.raw(0x2A), a.raw(0x16)))
        a.context(lambda: a.local(ids, 'MissionDirector'), lambda: a.call(director_calls[name], a.self_, lambda: a.local(ids, 'Mission')))
        a.patch(at)

    def accept(ids):
        a = Asm()
        on_tracker(a, lambda: a.call(activate, lambda: a.local(ids, 'Mission'), a.self_))
        with_director(a, ids, 'OnPlayerAcceptedMission')
        def marker(condition, yes, no):       # ExpEarn(0, condition ? yes : no): the recorded Source says what the native answered
            a.call(exp_earn, lambda: a.int_const(0),
                   lambda: (a.raw(0x45), condition(), a.w(0), a.byte_const(yes), a.w(0), a.byte_const(no)))
        marker(lambda: on_tracker(a, lambda: a.call(is_valid)), 101, 102)
        marker(lambda: a.call(is_primary), 103, 104)
        marker(lambda: a.context(lambda: a.instance(world_info), lambda: a.call(is_menu)), 105, 106)
        a.return_nothing(); a.end(); return a
    T.function(controller, 'AcceptMission', [('Object', 'Mission', 0), ('Object', 'MissionDirector', 0)], accept)

    def complete_mission(ids):
        a = Asm()
        on_tracker(a, lambda: a.call(complete, lambda: a.local(ids, 'Mission'), a.self_))
        with_director(a, ids, 'OnPlayerTurnedInMission')
        on_tracker(a, lambda: a.call(play_turn_in, lambda: a.local(ids, 'Mission')))
        a.call(exp_earn,
               lambda: a.context(lambda: a.local(ids, 'Mission'), lambda: a.call(get_xp, a.self_, a.false)),
               lambda: a.byte_const(5))
        a.return_nothing(); a.end(); return a
    T.function(controller, 'ServerCompleteMission', [('Object', 'Mission', 0), ('Object', 'MissionDirector', 0)], complete_mission)

    # The toy waypoint: Touch -> ProcessPlayerTouch; the set-changed reaction re-checks every actor in Touching; PostBeginPlay registers it.
    waypoint = T.cls('WillowWaypoint', super_ref=actor)
    linked = T.prop('Object', waypoint, 'LinkedObjective')
    T.prop('Bool', waypoint, 'bUpdateObjectiveOnPlayerTouch')
    touching_prop = p.add_import_full('ArrayProperty', actor, 'Touching')
    is_owned = p.add_import_full('Function', actor, 'IsPlayerOwned')

    def process(ids):
        a = Asm()
        at = a.jump_if_not(lambda: on_tracker(a, lambda: a.call(active_fn, lambda: a.instance(linked))))
        on_tracker(a, lambda: a.call(update_objective, lambda: a.instance(linked)))
        a.patch(at)
        a.return_nothing(); a.end(); return a
    active_fn = [e for e in p.exports if e[3] == p.fname('IsMissionObjectiveActive')]
    active_fn = p.exports.index(active_fn[0]) + 1
    process_ids = T.function(waypoint, 'ProcessPlayerTouch', [], process)['__self__']

    def touch(ids):
        a = Asm()
        at = a.jump_if_not(lambda: a.context(lambda: a.local(ids, 'Other'), lambda: a.call(is_owned)))
        a.call(process_ids)
        a.patch(at)
        a.return_nothing(); a.end(); return a
    T.function(waypoint, 'Touch', [('Object', 'Other', 0)], touch)

    def set_changed(ids):
        a = Asm()
        a.raw(0x58); a.instance(touching_prop); a.local(ids, 'Item'); a.raw(0); a.raw(0x4A); end_at = len(a.b); a.w(0)
        at = a.jump_if_not(lambda: a.context(lambda: a.local(ids, 'Item'), lambda: a.call(is_owned)))
        a.call(process_ids)
        a.patch(at)
        a.raw(0x31)                                  # IteratorNext
        struct.pack_into('<H', a.b, end_at, a.here())
        a.raw(0x30)                                  # IteratorPop
        a.return_nothing(); a.end(); return a
    for name in reactions:
        if name == 'MissionReactionObjectiveSetChanged':
            T.function(waypoint, name, [('Object', 'Tracker', 0)], set_changed, locals_=[('Object', 'Item')])
        else:
            T.function(waypoint, name, reaction_params(name), lambda ids: (lambda a: (a.return_nothing(), a.end(), a)[2])(Asm()))

    def begin_play(ids):
        a = Asm()
        on_tracker(a, lambda: a.call(register_observer, lambda: (a.raw(0x52), a.ref(imission), a.self_()), lambda: a.raw(0x2A)))
        a.return_nothing(); a.end(); return a
    T.function(waypoint, 'PostBeginPlay', [], begin_play)

    def validate_reply(ids):
        a = Asm()
        on_tracker(a, lambda: a.call(validate))
        a.return_nothing(); a.end(); return a
    T.function(controller, 'ClientValidateMissionData', [], validate_reply)

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


def build_mission(director=False):
    """With director=True the package also holds a toy Marcus: GD_Marcus.Character.Pawn_Marcus (a WillowAIPawn) whose MissionDirectives reference
    a table of five invented entries (below). ToyMission: one set SetA {RockPaper_GoToRange (count 1)} (and an unused SetB) with bCanCompleteMission, activated at acceptance; a reward percentage that is an
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
    only = p.add_export(chain('WillowGame', 'MissionObjectiveDefinition'), 'RockPaper_GoToRange', w32(0) + t.int('ObjectiveCount', 1) + none, outer=mission)
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
    set_b = p.add_export(chain('WillowGame', 'MissionObjectiveSetDefinition'), 'SetB', w32(0) + none, outer=mission)
    # GD_Globals.General.Globals with an invented interaction distance (the use ray's length)
    globals_package = p.add_export(chain('Core', 'Package'), 'GD_Globals', w32(0) + none)
    general_package = p.add_export(chain('Core', 'Package'), 'General', w32(0) + none, outer=globals_package)
    p.add_export(chain('WillowGame', 'GlobalsDefinition'), 'Globals', w32(0) + t.float('PlayerInteractionDistance', 420.0) + none, outer=general_package)
    if director:
        other = p.add_export(chain('WillowGame', 'MissionDefinition'), 'OtherMission', b'')
        end_only = p.add_export(chain('WillowGame', 'MissionDefinition'), 'EndOnlyMission', b'')

        def entry(ref, begins, ends, branch):
            return t.obj('MissionDefinition', ref) + t.bool('bBeginsMission', begins) + t.bool('bEndsMission', ends) + t.byte('BranchEnding', branch) + none
        # e1 another mission that begins; e2 ToyMission begins and ends (no branch); e3 another mission that only ends; e4 / e5 ToyMission ends on
        # branch 1 / 2. Nothing in the tracker knows the other two missions, so they are never offered.
        entries = [entry(other, True, False, 0), entry(mission, True, True, 0), entry(end_only, False, True, 0),
                   entry(mission, False, True, 1), entry(mission, False, True, 2)]
        table = p.add_export(chain('WillowGame', 'MissionDirectivesDefinition'), 'MissionDirectivesDefinition_1',
                             w32(0) + t.array('MissionDirectives', len(entries), b''.join(entries)) + none)
        gd = p.add_export(chain('Core', 'Package'), 'GD_Marcus', w32(0) + none)
        character = p.add_export(chain('Core', 'Package'), 'Character', w32(0) + none, outer=gd)
        p.add_export(chain('WillowGame', 'WillowAIPawn'), 'Pawn_Marcus', w32(0) + t.obj('MissionDirectives', table) + none, outer=character)
    cls_, sup, outer, name, _ = p.exports[mission - 1]
    p.exports[mission - 1] = (cls_, sup, outer, name, w32(0) + t.array('ObjectiveSetDefs', 2, w32(set_a, set_b))
                              + t.obj('InitialObjectiveSet', set_a) + t.bool('bActivateInitialObjectiveSet', True) + reward + none)
    return p


def build_dynamic():
    """Sanctuary_Dynamic: a provider at the stock path whose sequences exercise the enable conditions. Each sequence has an
    OnBehaviorSequenceEnabled and OnBehaviorSequenceDisabled event with a Behavior_RemoteEvent that names the transition."""
    p = Package()
    t = Tags(p)
    chains = {}

    def chain(*path):
        if path in chains: return chains[path]
        outer = chain(*path[:-1]) if len(path) > 1 else 0
        chains[path] = p.add_import_full('Package' if len(path) == 1 else 'Class', outer, path[-1])
        return chains[path]

    none = t.none()
    package_class = chain('Core', 'Package')
    outer = 0
    for part in ('GD_TargetDummy', 'Character', 'CharClass_TargetDummy'):
        outer = p.add_export(package_class, part, w32(0) + none, outer=outer)
    provider = p.add_export(chain('GearboxFramework', 'BehaviorProviderDefinition'), 'BehaviorProviderDefinition_5', b'', outer=outer)
    # the placed waypoint at the stock path (a placed actor: 26 bytes before its tags)
    world = p.add_export(package_class, 'TheWorld', w32(0) + none)
    level = p.add_export(package_class, 'PersistentLevel', w32(0) + none, outer=world)
    p.add_export(chain('WillowGame', 'WillowWaypoint'), 'WillowWaypoint_9',
                 bytes(26) + t.obj('LinkedObjective', chain('Startup', 'ToyMission', 'RockPaper_GoToRange')) + t.bool('bUpdateObjectiveOnPlayerTouch', True) + none, outer=level)
    mission, objective = chain('Startup', 'ToyMission'), chain('Startup', 'ToyMission', 'RockPaper_GoToRange')
    set_a, set_b = chain('Startup', 'ToyMission', 'SetA'), chain('Startup', 'ToyMission', 'SetB')

    def states(name, type_, **bits): return t.struct_(name, type_, b''.join(t.bool(k, v) for k, v in bits.items()) + none)

    def condition(name, specific, mission_states=None, objective_states=None, restrictions=()):
        body = t.obj('LinkedMission', mission) + t.bool('bIsObjectiveSpecific', specific)
        if mission_states: body += states('MissionStatesToLinkTo', 'MissionStatesToLinkTo', **mission_states)
        if specific: body += t.obj('LinkedObjective', objective) + states('ObjectiveStatesToLinkTo', 'ObjectiveStatesToLinkTo', **objective_states)
        if restrictions: body += t.array('ObjectiveSetRestrictions', len(restrictions), w32(*restrictions))
        return p.add_export(chain('WillowGame', 'BehaviorSequenceEnableByMission'), name, w32(0) + body + none, outer=provider)

    def sub(name, first, length): return t.struct_(name, 'SubarrayData', t.int('ArrayIndexAndLength', (first << 16) | length) + none)

    def sequence(name, on_spawn=False, mutex=False, cond=None):
        behaviors, links, events = [], [], b''
        for event, remote in (('OnBehaviorSequenceEnabled', name + 'On'), ('OnBehaviorSequenceDisabled', name + 'Off')):
            behaviors.append(p.add_export(chain('Engine', 'Behavior_RemoteEvent'), f'{name}_{remote}', w32(0) + t.name_('EventName', remote) + none, outer=provider))
            events += (t.struct_('UserData', 'BehaviorEventUserData', t.name_('EventName', event) + none)
                       + sub('OutputVariables', 0, 0) + sub('OutputLinks', len(links), 1) + none)
            links.append(len(behaviors) - 1)
        behavior_data = b''.join(t.obj('Behavior', ref) + sub('LinkedVariables', 0, 0) + sub('OutputLinks', 0, 0) + none for ref in behaviors)
        link_data = b''.join(t.int('LinkIdAndLinkedBehavior', index | (0 << 24)) + t.float('ActivateDelay', 0.0) + none for index in links)
        return (t.name_('BehaviorSequenceName', name) + t.bool('bEnabledOnSpawn', on_spawn) + t.bool('bSequenceEnabledMutex', mutex)
                + (t.obj('CustomEnableCondition', cond) if cond else b'')
                + t.array('EventData2', 2, events) + t.array('BehaviorData2', len(behaviors), behavior_data)
                + t.array('ConsolidatedOutputLinkData', len(links), link_data) + none)
    active = dict(bActive=True)
    sequences = [
        sequence('Idle', on_spawn=True),
        # the objective's state: Active while it can be progressed, Complete once its count is reached (status gating included)
        sequence('ObjSeq', cond=condition('ObjCondition', True, active, active)),
        sequence('RestrictA', cond=condition('RestrictACondition', True, active, active, [set_a])),
        sequence('RestrictB', cond=condition('RestrictBCondition', True, active, active, [set_b])),
        sequence('MissionLevel', cond=condition('MissionLevelCondition', False, dict(bComplete=True))),
        # two sequences of one mutex group: enabling the second disables the first
        sequence('MutexA', mutex=True, cond=condition('MutexACondition', False, dict(bActive=True, bReadyToTurnIn=True))),
        sequence('MutexB', mutex=True, cond=condition('MutexBCondition', False, dict(bReadyToTurnIn=True))),
    ]
    cls_, sup, outer_, name_, _ = p.exports[provider - 1]
    p.exports[provider - 1] = (cls_, sup, outer_, name_, w32(0) + t.array('BehaviorSequences', len(sequences), b''.join(sequences)) + none)
    return p


def build_dialog_mission():
    """DialogMission: ToyMission's shape (SetA {RockPaper_GoToRange}) plus a behavior provider and a dialog group. Default id 12 (the
    kickoff) -> K1 (tag A); custom events Chatter, Template and Immediate -> K2 (tag B), K3 (tag C), K4 (tag D, bForcePlayImmediate).
    Every K has Out (id 0) -> <name>Out and Finished (id 1) -> <name>Done remote events."""
    p = Package()
    t = Tags(p)
    chains = {}

    def chain(*path):
        if path in chains: return chains[path]
        outer = chain(*path[:-1]) if len(path) > 1 else 0
        chains[path] = p.add_import_full('Package' if len(path) == 1 else 'Class', outer, path[-1])
        return chains[path]

    none = t.none()
    mission = p.add_export(chain('WillowGame', 'MissionDefinition'), 'DialogMission', b'')
    only = p.add_export(chain('WillowGame', 'MissionObjectiveDefinition'), 'RockPaper_GoToRange', w32(0) + t.int('ObjectiveCount', 1) + none, outer=mission)
    set_a = p.add_export(chain('WillowGame', 'MissionObjectiveSetDefinition'), 'SetA',
                         w32(0) + t.array('ObjectiveDefinitions', 1, w32(only)) + t.bool('bCanCompleteMission', True) + none, outer=mission)
    package_class = chain('Core', 'Package')
    # priorities: index 0 is the most important
    names = ['P100', 'P70', 'P35', 'P30', 'P20', 'P10']
    priority_package = p.add_export(package_class, 'GD_Globals', w32(0) + none)
    dialog_package = p.add_export(package_class, 'Dialog', w32(0) + none, outer=priority_package)
    prio = {n: p.add_export(chain('GearboxFramework', 'GearboxDialogPriority'), 'DialogPriority_' + n[1:], w32(0) + none, outer=dialog_package) for n in names}
    p.add_export(chain('WillowGame', 'WillowDialogGlobalsDefinition'), 'DialogGlobals',
                 w32(0) + t.array('Priorities', len(names), w32(*[prio[n] for n in names])) + t.obj('ActiveMissionMinPriorityStart', prio['P20'])
                 + t.obj('ActiveSideMissionMinPriority', prio['P35']) + t.obj('ActivePlotMissionMinPriority', prio['P100']) + none, outer=dialog_package)
    marcus = p.add_export(chain('WillowGame', 'WillowDialogNameTag'), 'DialogName_Marcus', w32(0) + none)
    ak = {n: p.add_export(chain('Engine', 'AkEvent'), 'Ak_' + n, w32(0) + none) for n in 'ABCD'}

    def event_tag(name, priority_name, echo):
        return p.add_export(chain('WillowGame', 'WillowDialogEventTag'), 'Tag' + name,
                            w32(0) + t.obj('Priority', prio[priority_name]) + t.bool('bIsEchoEvent', echo) + none)
    tags = {'A': event_tag('A', 'P70', True), 'B': event_tag('B', 'P30', False), 'C': event_tag('C', 'P10', False), 'D': event_tag('D', 'P70', True)}
    group = p.add_export(chain('GearboxFramework', 'GearboxDialogGroup'), 'DialogGroup', b'')

    def talk_data(ak_name):
        return t.array('TalkData', 1, t.obj('NameTag', marcus) + t.obj('TalkAkEvent', ak[ak_name]) + none)

    def act(name, node_id, ak_name):
        return p.add_export(chain('WillowGame', 'WillowDialogAct_Talk'), name, w32(0) + t.int('NodeID', node_id) + talk_data(ak_name) + none, outer=group)
    acts = {'A': act('ActA', 41, 'A'), 'B1': act('ActB1', 42, 'A'), 'B2': act('ActB2', 43, 'B'), 'B3': act('ActB3', 44, 'A'), 'D': act('ActD', 45, 'D')}
    # events (1-based ids): 1 A, 2 B (first), 3 C (no inline act: the link table), 4 B (last enabled wins), 5 B (disabled), 6 D
    entries = [('A', True, acts['A']), ('B', True, acts['B1']), ('C', True, 0), ('B', True, acts['B2']), ('B', False, acts['B3']), ('D', True, acts['D'])]
    event_bodies = b''.join(t.obj('Tag', tags[n]) + t.bool('bEnabled', on) + (t.obj('OutputAction', a) if a else b'') + none for n, on, a in entries)
    template = t.array('TalkActs', 1, talk_data('C') + none)
    links = t.array('OutputLinksToStructs', 1, t.int('FromNodeID', 3) + t.int('LinkNumber', 0) + t.int('ToNodeID', 7) + none)
    cls_, sup, outer, name, _ = p.exports[group - 1]
    p.exports[group - 1] = (cls_, sup, outer, name, w32(0) + t.array('DialogEvents', len(entries), event_bodies) + template + links + none)

    provider = p.add_export(chain('GearboxFramework', 'BehaviorProviderDefinition'), 'DialogBpd', b'', outer=mission)
    behaviors, refs = [], {}

    def behavior(name, cls_name, tags_):
        refs[name] = len(behaviors)
        behaviors.append(p.add_export(chain(*cls_name), name, w32(0) + tags_ + none, outer=provider))
    for k, tag_name, immediate in (('Kick', 'A', False), ('Chat', 'B', False), ('Tmpl', 'C', False), ('Imm', 'D', True)):
        behavior('K' + k, ('GearboxFramework', 'Behavior_TriggerDialogEvent'),
                 t.obj('EventTag', tags[tag_name]) + t.obj('Group', group) + (t.bool('bForcePlayImmediate', True) if immediate else b''))
        behavior(k + 'Out', ('WillowGame', 'Behavior_MissionRemoteEvent'), t.name_('EventName', k + 'Out'))
        behavior(k + 'Done', ('WillowGame', 'Behavior_MissionRemoteEvent'), t.name_('EventName', k + 'Done'))
    # consolidated links: the events' links first, then each K behavior's Out (id 0) and Finished (id 1) links to its two remote events
    link_list = []

    def sub(name, first, length): return t.struct_(name, 'SubarrayData', t.int('ArrayIndexAndLength', (first << 16) | length) + none)
    events = [('Default', [(12, refs['KKick'])]), ('Chatter', [(0, refs['KChat'])]), ('Template', [(0, refs['KTmpl'])]), ('Immediate', [(0, refs['KImm'])])]
    event_data_bytes = b''
    for name, targets in events:
        event_data_bytes += (t.struct_('UserData', 'BehaviorEventUserData', t.name_('EventName', name) + none) + sub('OutputVariables', 0, 0)
                             + sub('OutputLinks', len(link_list), len(targets)) + none)
        link_list.extend(targets)
    behavior_bytes = b''
    for index, ref in enumerate(behaviors):
        is_trigger = index % 3 == 0
        behavior_bytes += (t.obj('Behavior', ref) + sub('LinkedVariables', 0, 0)
                           + (sub('OutputLinks', len(link_list), 2) if is_trigger else sub('OutputLinks', 0, 0)) + none)
        if is_trigger: link_list.extend([(0, index + 1), (1, index + 2)])
    link_bytes = b''.join(t.int('LinkIdAndLinkedBehavior', index | (link_id << 24)) + t.float('ActivateDelay', 0.0) + none for link_id, index in link_list)
    seq = (t.name_('BehaviorSequenceName', 'Main') + t.bool('bEnabledOnSpawn', True)
           + t.array('EventData2', len(events), event_data_bytes) + t.array('BehaviorData2', len(behaviors), behavior_bytes)
           + t.array('ConsolidatedOutputLinkData', len(link_list), link_bytes) + none)
    cls_, sup, outer, name, _ = p.exports[provider - 1]
    p.exports[provider - 1] = (cls_, sup, outer, name, w32(0) + t.array('BehaviorSequences', 1, seq) + none)
    cls_, sup, outer, name, _ = p.exports[mission - 1]
    p.exports[mission - 1] = (cls_, sup, outer, name, w32(0) + t.array('ObjectiveSetDefs', 1, w32(set_a)) + t.obj('InitialObjectiveSet', set_a)
                              + t.obj('BehaviorProvider', provider) + t.obj('MissionDialogGroup', group)
                              + t.bool('bActivateInitialObjectiveSet', True) + none)
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
                            'obj:RockPaper_GoToRange', 'script:turnin', 'pool')
    check('A exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    # Swap 6a: the use ray's length is read from the globals data (invented 420), not a host constant
    check('A interaction distance from the globals data', got['script']['interaction_distance'] == 420.0, got['script'].get('interaction_distance'))
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
        # trunc(18 x 1.5) = 27 at the stage locked at acceptance (4), not the later 9; the objective update tells the controller
        # UpdateMissionObjective (210) before the ReadyToTurnIn hook
        # AcceptMission (twice: the second is refused by the native but the script still runs its probes) probes IsDataValid (the
        # flag the validation reply set at construction: 101), IsPrimaryPlayer (true: 103) and IsMenuLevel (false: 106)
        probes = [(0, 101, -1), (0, 103, -1), (0, 106, -1)]
        want = [(0, 1, -1)] + probes + probes + [(0, 100, -1), (0, 210, -1), (0, 3, -1), (0, 4, -1), (27, 5, -1)]
        check('A recorded ExpEarn calls in order', earned_of(script) == want, earned_of(script))
        # 16 + 27 = 43 in the pool; the pool update ran ExpLevelUp from level 3 to 4 (R(4) = 30 <= 43 < R(5) = 48) and stopped
        check('A pool and level', script['experience_pool'] == 43 and script['player_level'] == 4, (script['experience_pool'], script['player_level']))
        # every native the toy script reaches is bound
        check('A no stubs', script['stubs'] == [], script['stubs'])
        check('A no VM notes', script['notes'] == [], script['notes'])

    # Scenario B: the pool never passes the experience of the maximum level (R(50) = 4998); the level-up loop stops at 50.
    code, got = mission_run(root, 'stage:4', 'player:3:4990', 'script:accept', 'obj:RockPaper_GoToRange', 'script:turnin', 'pool')
    check('B exit', code == 0 and got['errors'] == [], (code, got['errors']))
    check('B clamp and cap', got['script']['experience_pool'] == 4998 and got['script']['player_level'] == 50,
          (got['script']['experience_pool'], got['script']['player_level']))

    # Scenario C: no region stage supplied: stage 0, so the span R(1) - R(0) is 0 and the reward is 0: the pool stays put.
    code, got = mission_run(root, 'player:3:16', 'script:accept', 'obj:RockPaper_GoToRange', 'script:turnin', 'pool')
    check('C exit', code == 0 and got['errors'] == [], (code, got['errors']))
    check('C no region stage', got['script']['experience_pool'] == 16 and got['script']['player_level'] == 3,
          (got['script']['experience_pool'], got['script']['player_level']))

    # Scenario G (swap 6c): Marcus's mission screen and the director callbacks (NATIVE_USE_INTERACTION.md, NATIVE_MISSION_SCRIPT_BRIDGE.md).
    (root / 'DirectorMission.upk').write_bytes(build_mission(director=True).build())

    def director_run(*steps):
        proc = subprocess.run([reader, str(root / 'DirectorMission.upk'), '--mission-run', 'ToyMission', '--cooked', str(root), *steps],
                              capture_output=True, text=True, encoding='utf-8')
        assert proc.stdout.strip(), ('no JSON output', proc.returncode, proc.stderr)
        return proc.returncode, json.loads(proc.stdout)

    code, got = director_run('script:screen', 'script:accept', 'script:screen', 'script:accept', 'obj:RockPaper_GoToRange', 'script:screen',
                             'script:turnin', 'script:screen')
    check('G exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    screens = [step['screen'] for step in got['steps'] if 'screen' in step]
    toy = 'ToyMission'
    check('G screens reported', len(screens) == 4, len(screens))
    if len(screens) == 4:
        # NotStarted: only e2 begins and CanStartMission holds (the other missions have no record and are never offered); status 0 is not in progress
        check('G not started: eligible only', screens[0] == {'eligible': [toy], 'in_progress': [], 'redeemable': []}, screens[0])
        # Active: not startable; every entry of ToyMission that begins or ends is in progress (no deduplication); CanEndMission is false
        check('G active: in progress', screens[1] == {'eligible': [], 'in_progress': [toy, toy, toy], 'redeemable': []}, screens[1])
        # ReadyToTurnIn (the set completed): in progress is Active only, so none; redeemable: e2 (no branch) and e4 (branch 1 = the completed
        # branch, every objective of the last set complete); not e5 (branch 2) and not e3 (another mission)
        check('G ready: redeemable by branch', screens[2] == {'eligible': [], 'in_progress': [], 'redeemable': [toy, toy]}, screens[2])
        # Complete: neither CanStartMission (not repeatable) nor CanEndMission holds
        check('G complete: nothing offered', screens[3] == {'eligible': [], 'in_progress': [], 'redeemable': []}, screens[3])
    steps = [step for step in got['steps'] if step['step'] in ('script:accept', 'script:turnin')]
    check('G accept applied, then refused', [step['ok'] for step in steps] == [True, False, True] and [step['status'] for step in steps] == [1, 1, 3], steps)
    sources = [e['source'] for e in got['script']['exp_earned']]
    # AcceptMission runs the director's OnPlayerAcceptedMission after the native (marker 120, the toy script does not skip a refused call);
    # ServerCompleteMission runs OnPlayerTurnedInMission (121) after CompleteMission and before PlayTurnIn
    check('G director callbacks ran', sources.count(120) == 2 and sources.count(121) == 1, sources)
    check('G turn-in callback before the reward', sources.index(121) < sources.index(5) if 121 in sources and 5 in sources else False, sources)
    # a mission without a director table behaves as before (scenario A runs without Marcus: no callback markers)

    # Scenario D: the dummy's enable conditions through --slice-run (src/slice.cpp), NATIVE_BEHAVIOR_POPULATION.md sections A-C.
    (root / 'Startup.upk').write_bytes((root / 'TestMission.upk').read_bytes())
    (root / 'Sanctuary_Dynamic.upk').write_bytes(build_dynamic().build())

    def slice_run(*steps):
        proc = subprocess.run([reader, str(root / 'Sanctuary_Dynamic.upk'), '--slice-run', 'ToyMission', '--cooked', str(root), *steps],
                              capture_output=True, text=True, encoding='utf-8')
        assert proc.stdout.strip(), ('no JSON output', proc.returncode, proc.stderr)
        return proc.returncode, json.loads(proc.stdout)

    def remote(step): return [e['a'] for e in step['events'] if e['kind'] == 'remote_event']
    code, got = slice_run('accept', 'spawn', 'range', 'turnin')
    check('D exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    steps = {step['step']: step for step in got['steps']}
    # before the spawn the provider is not a consumer: the accept (Active) notifications reach no condition
    check('D accept: no sequence events', remote(steps['accept']) == [], remote(steps['accept']))
    # registration: pass 1 enables Idle, then each condition's verdict is applied in sequence order (objective Active in the active set,
    # RestrictA's set is active, RestrictB's is not, mission not Complete, MutexA's {Active} holds, MutexB's {ReadyToTurnIn} does not)
    check('D spawn registration', remote(steps['spawn']) == ['IdleOn', 'ObjSeqOn', 'RestrictAOn', 'MutexAOn'], remote(steps['spawn']))
    # the objective's progress write already makes it Complete (status Active, progress = count): ObjSeq and RestrictA turn off at the
    # progress notification; the set completion makes the mission ReadyToTurnIn: MutexB enables and disables MutexA first (the mutex)
    check('D range', remote(steps['range']) == ['ObjSeqOff', 'RestrictAOff', 'MutexAOff', 'MutexBOn'], remote(steps['range']))
    # Complete: the mission-level condition turns on, MutexB's {ReadyToTurnIn} no longer holds
    check('D turn-in', remote(steps['turnin']) == ['MissionLevelOn', 'MutexBOff'], remote(steps['turnin']))
    # transitions only: the sequences end in the state the last verdicts left
    check('D final enabled sequences', sorted(got['dummy_enabled_sequences']) == ['Idle', 'MissionLevel'], got['dummy_enabled_sequences'])

    # Scenario E: Behavior_TriggerDialogEvent through --mission-run (src/dialog.*), NATIVE_DIALOG.md.
    (root / 'DialogMission.upk').write_bytes(build_dialog_mission().build())

    def dialog_run(*steps):
        proc = subprocess.run([reader, str(root / 'DialogMission.upk'), '--mission-run', 'DialogMission', '--cooked', str(root), *steps],
                              capture_output=True, text=True, encoding='utf-8')
        assert proc.stdout.strip(), ('no JSON output', proc.returncode, proc.stderr)
        return proc.returncode, json.loads(proc.stdout)

    def seen(step): return [e['a'] for e in step['effects'] if e['kind'] == 'remote_event']
    def lines(step): return [dict(item.split('=', 1) for item in e['detail'].split(';')) | {'tag': e['a'].split('.')[-1], 'talker_tag': e['c']}
                             for e in step['effects'] if e['kind'] == 'dialog']

    # E1: no line player (no audio device): Out on the first run, the dialog one wake later, Finished in the same run (nothing started)
    code, got = dialog_run('accept', 'tick:0', 'tick:0.02')
    check('E1 exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    kick, wake = got['steps'][1], got['steps'][2]
    check('E1 Out first, before any dialog', seen(kick) == ['KickOut'] and lines(kick) == [], (seen(kick), lines(kick)))
    check('E1 dialog on the next wake, Finished at once', [l['outcome'] for l in lines(wake)] == ['no audio device'] and seen(wake) == ['KickDone'], (lines(wake), seen(wake)))
    check('E1 the line names its act, AkEvent and an echo talker', lines(wake)[0]['ak'].endswith('Ak_A') and lines(wake)[0]['talker'] == 'echo'
          and lines(wake)[0]['act'].endswith('ActA') and lines(wake)[0]['tag'] == 'TagA', lines(wake))

    # E2: a line player (every line lasts 0.55 s) and a registered pawn: Finished waits for the end of the line (polls every 0.1 s);
    # the chatter event B (non-echo, priority index 3) is blocked while A (index 1) is live and finishes at once
    code, got = dialog_run('lines:0.55', 'talker:DialogName_Marcus', 'accept', 'tick:0', 'tick:0.02', 'custom:Chatter',
                           'tick:0.02', *(['tick:0.1'] * 6))
    check('E2 exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    steps = got['steps'][2:]          # after lines: / talker:
    check('E2 line A starts with a pawn talker, no Finished yet', [l['outcome'] for l in lines(steps[2])] == ['started'] and lines(steps[2])[0]['talker'] == 'pawn'
          and seen(steps[2]) == [], (lines(steps[2]), seen(steps[2])))
    check('E2 chatter: Out at once, blocked by priority on its wake, Finished at once', seen(steps[3]) == ['ChatOut']
          and [l['outcome'] for l in lines(steps[4])] == ['blocked by priority'] and seen(steps[4]) == ['ChatDone'], (steps[3], steps[4]))
    done_at = [i for i, step in enumerate(steps[5:]) if 'KickDone' in seen(step)]
    # A starts at about 0.0167 s (the first wake) and lasts 0.55 s (ends at 0.5667 s); the polls are 0.1 s apart from 0.1167 s: the first one
    # after the end is 0.6167 s, reached by the sixth 0.1 s tick (t = 0.64 s), and not by the fifth (t = 0.54 s), whose last poll (0.5167 s)
    # still sees the line live
    check('E2 Finished at the first poll after the line ended', done_at == [5], done_at)

    # E3: after the line ended, the same chatter event plays; a template event (no inline act) resolves through the link table;
    # a forced-immediate trigger selects Finished and Out in its first run
    code, got = dialog_run('lines:0.55', 'talker:DialogName_Marcus', 'accept', 'tick:1', 'custom:Chatter', 'tick:0.1', 'tick:0.7', 'custom:Template',
                           'tick:0.1', 'custom:Immediate')
    check('E3 exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    steps = got['steps'][2:]
    chat = lines(steps[3])
    # tag B: the last enabled entry (ActB2, AkEvent B) wins over the first and the disabled one
    check('E3 chatter uses the last enabled entry', len(chat) == 1 and chat[0]['outcome'] == 'started' and chat[0]['act'].endswith('ActB2') and chat[0]['ak'].endswith('Ak_B'), chat)
    template = lines(steps[6])
    check('E3 template act through the link table', len(template) == 1 and template[0]['act'].endswith('TalkActs[0]') and template[0]['ak'].endswith('Ak_C'), template)
    immediate = steps[7]
    check('E3 immediate: Finished and Out in the first run (Out thread first)', seen(immediate) == ['ImmOut', 'ImmDone'] and len(lines(immediate)) == 1, (seen(immediate), lines(immediate)))

    # Scenario F: the GoToRange waypoint script through --slice-run (src/mission_script.*, src/slice.*), NATIVE_OBJECTIVE_TRIGGERS.md.
    def updates(got): return sum(1 for e in got['script']['exp_earned'] if e['source'] == 210)

    # F1: Marcus's touch is ignored; the player's completes the objective (the toy set can complete the mission) with exactly one update;
    # touching again after it completed changes nothing
    code, got = slice_run('touch:marcus', 'accept', 'touch:marcus', 'touch:player', 'untouch:player', 'touch:player')
    check('F1 exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    status = [step['status'] for step in got['steps']]
    check('F1 Marcus ignored, the player completes it once', status == [0, 1, 1, 2, 2, 2] and updates(got) == 1, (status, updates(got)))

    # F2: not updatable (the mission is not started): a touch changes nothing; the player already inside when the set activates completes the
    # objective at that moment, without leaving and re-entering
    code, got = slice_run('touch:player', 'accept')
    check('F2 exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    status = [step['status'] for step in got['steps']]
    check('F2 no update while not updatable, then the set-changed reaction completes it', status == [0, 2] and updates(got) == 1
          and [e['kind'] for e in got['steps'][0]['events']] == [], (status, updates(got)))

    # F3: the Fire-style direct wrapper (range) goes through the waypoint too
    code, got = slice_run('accept', 'range')
    check('F3 range wrapper', code == 0 and [step['status'] for step in got['steps']] == [1, 2] and updates(got) == 1, [step['status'] for step in got['steps']])

if failures:
    print(f'{len(failures)} mission script check(s) failed:')
    for failure in failures: print('  - ' + failure)
    sys.exit(1)
print('mission script synthetic coverage passed.')
