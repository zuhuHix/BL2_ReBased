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

The use-chain scenario (--mission-run `script:use`, scenario H, packages UseChainOn / UseChainOff) gives the toy Marcus a provider at the stock path
GD_Marcus.Character.AIDef_Marcus.AIBehaviorProviderDefinition_0: sequence Brain with the event OnUsed (link id 2, its Instigator output published into an
object variable by connection index), a cascade of Behavior_IsSequenceEnabled checks (a sequence that does not exist, one that is disabled, one that is
enabled in the On package and disabled in the Off package, one whose path names no provider) and Behavior_RemoteCustomEvent / Behavior_ShowMissionInterface
toys; their scripts mirror the installed ones in structure and call the natives the bridge binds (IsBehaviorSequenceEnabled, ResolveBehaviorProvider-
DefinitionReference, ActivateBehaviorEventFromScript, ActivateBehaviorOutputLink, GetBehaviorConsumerHandle, ClientGFxPlayMovie).

The dialog scenario (--mission-run, scenario E) uses a toy mission whose provider runs Behavior_TriggerDialogEvent behaviors over a toy
dialog group (invented tags, priorities, acts, a talker name tag): Out on the first run, the dialog one kernel wake later, Finished when
the live line ends, the priority arbitration with the tracked-mission floor, the last enabled entry for a tag, a template act through the
link table, a registered pawn against the echo caller, and bForcePlayImmediate.

The level-up scenario (--slice-run, scenario S7, swap 7) gives the toy ExpLevelUp the installed script's two further steps, in miniature: it adds
int(EvaluateInitializationData(GlobalsDefinition.GeneralSkillPointsPerLevelUp, Self)) to PRI.GeneralSkillPoints (invented data: a conditional on the
attribute PlayerExperienceLevel, whose chain is the player replication info context resolver and an object-property resolver of ExpLevel: 1 from level 5,
else 0, the note's rule) and calls RecalculateAttributeInitializedState, which the bridge answers with the health pool's base maximum (invented
Init_PlayerHealth: 10 x 1.5 ^ level, so 50.625 at level 4, 75.9375 at 5, 113.90625 at 6). The slice reports them as skill_points and max_health events.

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
        """declared: [(kind, name, extra flags[, type ref])]; `body(ids)` returns an Asm (None for a native). Children are exported
        first, in reverse declaration order, the return value before them (as in the real packages)."""
        ordered = ([(result, 'ReturnValue', CPF_PARM | CPF_OUT | CPF_RET, 0)] if result else []) + \
                  [(d[0], d[1], CPF_PARM | d[2], d[3] if len(d) > 3 else 0) for d in reversed(declared)] + [(k, n, 0, 0) for k, n in locals_]
        func_index = len(self.p.exports) + 1 + len(ordered)
        ids = {}
        for kind, pname, pflags, type_ref in ordered: ids[pname] = self.prop(kind, func_index, pname, pflags, type_ref)
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
    context_data = T.struct_(behavior_base, 'BehaviorContextData')
    T.prop('Name', context_data, 'InstancedDataContextName')
    T.prop('Object', context_data, 'ContextObject')
    T.prop('Byte', context_data, 'BehaviorContext')
    T.prop('Byte', context_data, 'bSupportsDefaultOutputLink')
    T.function(behavior_base, 'GetBehaviorContext', [('Struct', 'ContextData', 0), ('Object', 'SelfObject', 0), ('Object', 'MyInstigatorObject', 0),
                                                     ('Object', 'OtherEventParticipantObject', 0), ('Struct', 'EventData', CPF_OPT)],
               None, FUNC_NATIVE | FUNC_PUBLIC, 'Object')
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
    T.function(definition, 'EvaluateInitializationData', [('Struct', 'InitializationData', 0, aid), ('Object', 'ContextSource', 0),
                                                         ('Object', 'OptionalOverrideContextSource', CPF_OPT)], None,
               FUNC_NATIVE | FUNC_PUBLIC | 0x2000, 'Float')
    T.prop('Name', T.cls('ObjectPropertyAttributeValueResolver'), 'PropertyName')
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
    T.prop('Bool', act, 'bEnableNoMatch')
    link_list = T.struct_(0, 'DialogOutputLink')
    T.array_of(link_list, 'Links', 'Object')
    T.array_of(act, 'OutputLinks', 'Struct', link_list)
    T.cls('GearboxDialogVar_Instigator', super_ref=node)
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
    T.cls('GearboxDialogTemplateGroup', super_ref=group)

    # The behavior provider vocabulary (tests/behavior_test.py has the commented version): sequences, events, behaviors, links.
    bpd = T.cls('BehaviorProviderDefinition')
    var_types = ['BVAR_None', 'BVAR_Object', 'BVAR_Int', 'BVAR_Float', 'BVAR_InstanceData', 'BVAR_NamedVariable', 'BVAR_Mystery', 'BVAR_NamedKismetVariable']
    link_types = ['BVARLINK_Unknown', 'BVARLINK_Input', 'BVARLINK_Output', 'BVARLINK_Context']
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

    # Swap 6b: the use chain's natives and the script behavior Behavior_IsSequenceEnabled (same structure as the installed one, over invented
    # classes; the toy skips the QueryInterface step and asks the context object for its handle directly).
    native = FUNC_NATIVE | FUNC_PUBLIC
    kernel = T.cls('BehaviorKernel')
    helpers = T.cls('BehaviorHelpers')
    path_struct = T.struct_(0, 'NameBasedObjectPath')
    T.array_of(path_struct, 'PathComponentNames', 'Name')
    T.prop('Byte', path_struct, 'IsSubobjectMask')
    activate_link = T.function(kernel, 'ActivateBehaviorOutputLink', [('Struct', 'KernelInfo', CPF_OUT), ('Int', 'OutputLinkId', 0)], None, native)['__self__']
    is_enabled = T.function(kernel, 'IsBehaviorSequenceEnabled', [('Struct', 'ConsumerHandle', 0), ('Object', 'ProviderDefinition', 0), ('Name', 'BehaviorSequenceName', 0)],
                            None, native, 'Bool')['__self__']
    T.function(kernel, 'ActivateBehaviorEventFromScript', [('Struct', 'ConsumerHandle', 0), ('Object', 'ProviderDefinition', 0), ('Name', 'EventName', 0),
                                                           ('Int', 'EventOutputToActivate', CPF_OPT), ('Array', 'Parameters', CPF_OPT)], None, native)
    resolve = T.function(helpers, 'ResolveBehaviorProviderDefinitionReference', [('Object', 'SourceBehavior', 0), ('Object', 'ProviderReference', 0),
                                                                                    ('Struct', 'PathName', 0)], None, native, 'Object')['__self__']
    willow = p.add_import_full('Package', 0, 'WillowGame')
    get_handle = p.add_import_full('Function', p.add_import_full('Class', willow, 'WillowPawn'), 'GetBehaviorConsumerHandle')
    is_seq = T.cls('Behavior_IsSequenceEnabled')
    seq_name = T.prop('Name', is_seq, 'SequenceName')
    seq_provider = T.prop('Object', is_seq, 'SequenceProvider')
    seq_path = T.prop('Struct', is_seq, 'ProviderDefinitionPathName', type_ref=path_struct)

    def is_sequence_enabled(ids):
        a = Asm()
        a.raw(0x0F); a.local(ids, 'Handle'); a.context(lambda: a.local(ids, 'ContextObject'), lambda: a.call(get_handle))
        a.raw(0x0F); a.local(ids, 'Provider'); a.call(resolve, a.self_, lambda: a.instance(seq_provider), lambda: a.instance(seq_path))
        none_at = a.jump_if_not(lambda: (a.raw(119), a.local(ids, 'Provider'), a.raw(0x2A), a.raw(0x16)))     # Provider != None, else return
        no_at = a.jump_if_not(lambda: a.call(is_enabled, lambda: a.local(ids, 'Handle'), lambda: a.local(ids, 'Provider'), lambda: a.instance(seq_name)))
        a.call(activate_link, lambda: (a.raw(0x48), a.ref(ids['KernelInfo'])), lambda: a.int_const(0))
        done = a.jump()
        a.patch(no_at)
        a.call(activate_link, lambda: (a.raw(0x48), a.ref(ids['KernelInfo'])), lambda: a.int_const(1))
        a.patch(done); a.patch(none_at)
        a.return_nothing(); a.end(); return a
    T.function(is_seq, 'ApplyBehaviorToContext', [('Object', 'ContextObject', 0), ('Object', 'SelfObject', 0), ('Object', 'MyInstigatorObject', 0),
                                                  ('Object', 'OtherEventParticipantObject', 0), ('Struct', 'EventData', 0), ('Struct', 'KernelInfo', CPF_OUT)],
               is_sequence_enabled, locals_=[('Struct', 'Handle'), ('Object', 'Provider')])
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
    evaluate = p.add_import_full('Function', p.add_import_full('Class', engine, 'AttributeInitializationDefinition'), 'EvaluateInitializationData')
    world_info, gri = (p.add_import_full('ObjectProperty', actor, 'WorldInfo'),
                       p.add_import_full('ObjectProperty', p.add_import_full('Class', engine, 'WorldInfo'), 'GRI'))
    native = FUNC_NATIVE | FUNC_PUBLIC

    # Operators the toy ExpLevelUp uses: Object.+(int,int), registered by the core natives under native number 146.
    object_cls = T.cls('Object')
    T.function(object_cls, 'Add_IntInt', [('Int', 'P0', 0), ('Int', 'P1', 0)], None, FUNC_NATIVE | 0x1000 | 0x23000, 'Int', native=146, friendly='+')
    T.function(object_cls, 'EqualEqual_IntInt', [('Int', 'P0', 0), ('Int', 'P1', 0)], None, FUNC_NATIVE | 0x1000 | 0x23000, 'Bool', native=154, friendly='==')
    T.function(object_cls, 'EqualEqual_ObjectObject', [('Object', 'P0', 0), ('Object', 'P1', 0)], None, FUNC_NATIVE | 0x1000 | 0x23000, 'Bool', native=114, friendly='==')
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
    trigger_act = T.cls('WillowDialogAct_Trigger', super_ref=p.add_import_full('Class', gearbox, 'GearboxDialogNode'))
    output_link = p.add_import_full('ScriptStruct', gearbox, 'DialogOutputLink')
    T.prop('Object', trigger_act, 'DialogEvent')
    T.array_of(trigger_act, 'VariableLinks', 'Struct', output_link)
    T.array_of(trigger_act, 'OutputLinks', 'Struct', output_link)
    globals_definition = T.cls('WillowDialogGlobalsDefinition')
    T.array_of(globals_definition, 'Priorities', 'Object')
    for field in ('ActiveMissionMinPriorityStart', 'ActiveSideMissionMinPriority', 'ActivePlotMissionMinPriority'): T.prop('Object', globals_definition, field)
    # a pawn's dialog groups (NATIVE_DIALOG_GROUPS.md): the globals' NPC groups and default template group, the body class, the name tag's expansion
    T.array_of(globals_definition, 'NPCDialogGroups', 'Object')
    T.prop('Object', globals_definition, 'DefaultTemplateGroup')
    T.prop('Object', T.cls('WillowDialogNameTag'), 'DlcExpansion')
    T.array_of(T.cls('DlcExpansionDefinition'), 'NPCDialogGroups', 'Object')       # synthetic class name
    body_class = T.cls('BodyClassDefinition')
    T.array_of(body_class, 'DialogGroups', 'Object')
    T.prop('Bool', body_class, 'bNPCDialog')
    T.prop('Object', body_class, 'DialogName')
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
    willow_pawn = T.cls('WillowPawn', super_ref=engine_pawn)
    consumer_handle = T.struct_(0, 'BehaviorConsumerHandle')
    T.prop('Int', consumer_handle, 'PID')
    T.prop('Struct', willow_pawn, 'ConsumerHandle', type_ref=consumer_handle)
    T.function(willow_pawn, 'GetBehaviorConsumerHandle', [], None, native, 'Struct')
    ai_pawn = T.cls('WillowAIPawn', super_ref=willow_pawn)
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
    get_willow_globals = T.function(globals_, 'GetWillowGlobals', [], None, native | 0x2000, 'Object')['__self__']
    get_globals_definition = T.function(globals_, 'GetGlobalsDefinition', [], None, native, 'Object')['__self__']
    globals_definition = T.cls('GlobalsDefinition')
    T.prop('Float', globals_definition, 'PlayerInteractionDistance')
    points_per_level = T.prop('Struct', globals_definition, 'GeneralSkillPointsPerLevelUp', type_ref=aid)
    # the player's class and its health pool (the data the RecalculateAttributeInitializedState stand-in reads)
    T.prop('Object', T.cls('PlayerClassDefinition'), 'HealthPoolDefinition')
    T.prop('Struct', T.cls('ResourcePoolDefinition'), 'BaseMaxValue', type_ref=aid)
    T.cls('PlayerReplicationInfoAttributeContextResolver')

    status_owner = T.cls('IMission')
    status = T.struct_(status_owner, 'MissionStatusPlayerData')
    T.prop('Object', status, 'MissionDef')
    T.prop('Byte', status, 'Status')
    T.prop('Bool', status, 'bNeedsRewards')
    pri = T.cls('WillowPlayerReplicationInfo')
    exp_level = T.prop('Int', pri, 'ExpLevel')
    next_at = T.prop('Int', pri, 'ExpPointsNextLevelAt')
    general_points = T.prop('Int', pri, 'GeneralSkillPoints')
    controller = T.cls('WillowPlayerController', super_ref=player_controller)
    T.function(controller, 'GetCurrentPlaythrough', [], None, native, 'Int')
    recalculate = T.function(controller, 'RecalculateAttributeInitializedState', [], None, native)['__self__']
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

    # ClientGFxPlayMovie (script in the game; the bridge replaces it with a report): the toy body leaves the marker 140 so a run shows it was not used
    def play_movie_body(ids):
        a = Asm()
        a.call(exp_earn, lambda: a.int_const(0), lambda: a.byte_const(140))
        a.return_nothing(); a.end(); return a
    play_movie = T.function(controller, 'ClientGFxPlayMovie', [('Object', 'MovieDefinition', 0), ('Object', 'OtherObject', 0), ('Name', 'MovieTag', CPF_OPT)],
                            play_movie_body)['__self__']
    pawn_controller = p.add_import_full('ObjectProperty', engine_pawn, 'Controller')
    gearbox_pkg = p.add_import_full('Package', 0, 'GearboxFramework')
    helpers_cls = p.add_import_full('Class', gearbox_pkg, 'BehaviorHelpers')
    kernel_cls = p.add_import_full('Class', gearbox_pkg, 'BehaviorKernel')
    resolve_fn = p.add_import_full('Function', helpers_cls, 'ResolveBehaviorProviderDefinitionReference')
    fire_fn = p.add_import_full('Function', kernel_cls, 'ActivateBehaviorEventFromScript')
    path_struct_ref = p.add_import_full('ScriptStruct', gearbox_pkg, 'NameBasedObjectPath')
    remote = T.cls('Behavior_RemoteCustomEvent')
    event_name = T.prop('Name', remote, 'CustomEventName')
    remote_provider = T.prop('Object', remote, 'SequenceProvider')
    remote_path = T.prop('Struct', remote, 'ProviderDefinitionPathName', type_ref=path_struct_ref)

    def remote_body(ids):      # Provider = Resolve(Self, SequenceProvider, Path); if (Provider != None) ActivateBehaviorEventFromScript(Handle, Provider, Name)
        a = Asm()
        a.raw(0x0F); a.local(ids, 'Provider'); a.call(resolve_fn, a.self_, lambda: a.instance(remote_provider), lambda: a.instance(remote_path))
        none_at = a.jump_if_not(lambda: (a.raw(119), a.local(ids, 'Provider'), a.raw(0x2A), a.raw(0x16)))
        a.raw(0x0F); a.local(ids, 'Handle'); a.context(lambda: a.local(ids, 'ContextObject'), lambda: a.call(get_handle_local))
        a.call(fire_fn, lambda: a.local(ids, 'Handle'), lambda: a.local(ids, 'Provider'), lambda: a.instance(event_name), lambda: a.raw(0x4A), lambda: a.raw(0x4A))
        a.patch(none_at)
        a.return_nothing(); a.end(); return a
    get_handle_local = [e for e in p.exports if e[3] == p.fname('GetBehaviorConsumerHandle')]
    get_handle_local = p.exports.index(get_handle_local[0]) + 1
    T.function(remote, 'ApplyBehaviorToContext', [('Object', 'ContextObject', 0), ('Object', 'SelfObject', 0), ('Object', 'MyInstigatorObject', 0),
                                                  ('Object', 'OtherEventParticipantObject', 0), ('Struct', 'EventData', 0), ('Struct', 'KernelInfo', CPF_OUT)],
               remote_body, locals_=[('Struct', 'Handle'), ('Object', 'Provider')])
    # Behavior_PlayAIMissionContextDialog (the class the bridge's handler covers) as a probe of GetBehaviorContext: it resolves its PlayerWhoUsedMe
    # struct and selects output 0 for None, 2 for SelfObject and 1 for any other object, so the cascade line says what the resolver returned.
    engine_pkg = p.add_import_full('Package', 0, 'Engine')
    base_cls = p.add_import_full('Class', engine_pkg, 'BehaviorBase')
    get_context = p.add_import_full('Function', base_cls, 'GetBehaviorContext')
    context_struct = p.add_import_full('ScriptStruct', base_cls, 'BehaviorContextData')
    activate_fn = p.add_import_full('Function', kernel_cls, 'ActivateBehaviorOutputLink')
    probe = T.cls('Behavior_PlayAIMissionContextDialog')
    who_prop = T.prop('Struct', probe, 'PlayerWhoUsedMe', type_ref=context_struct)

    def probe_body(ids):
        a = Asm()
        a.raw(0x0F); a.local(ids, 'Resolved')
        a.call(get_context, lambda: a.instance(who_prop), lambda: a.local(ids, 'SelfObject'), lambda: a.local(ids, 'MyInstigatorObject'),
               lambda: a.local(ids, 'OtherEventParticipantObject'), lambda: a.raw(0x4A))
        def select(n): a.call(activate_fn, lambda: (a.raw(0x48), a.ref(ids['KernelInfo'])), lambda: a.int_const(n))
        none_at = a.jump_if_not(lambda: (a.raw(119), a.local(ids, 'Resolved'), a.raw(0x2A), a.raw(0x16)))       # Resolved != None
        self_at = a.jump_if_not(lambda: (a.raw(114), a.local(ids, 'Resolved'), a.local(ids, 'SelfObject'), a.raw(0x16)))   # Resolved == SelfObject
        select(2)
        end_a = a.jump()
        a.patch(self_at)
        select(1)
        end_b = a.jump()
        a.patch(none_at)
        select(0)
        a.patch(end_a); a.patch(end_b)
        a.return_nothing(); a.end(); return a
    T.function(probe, 'ApplyBehaviorToContext', [('Object', 'ContextObject', 0), ('Object', 'SelfObject', 0), ('Object', 'MyInstigatorObject', 0),
                                                 ('Object', 'OtherEventParticipantObject', 0), ('Struct', 'EventData', 0), ('Struct', 'KernelInfo', CPF_OUT)],
               probe_body, locals_=[('Object', 'Resolved')])
    show = T.cls('Behavior_ShowMissionInterface')

    def show_body(ids):        # ContextObject.Controller.ClientGFxPlayMovie(SelfObject, SelfObject)
        a = Asm()
        a.context(lambda: a.context(lambda: a.local(ids, 'ContextObject'), lambda: a.instance(pawn_controller)),
                  lambda: a.call(play_movie, lambda: a.local(ids, 'SelfObject'), lambda: a.local(ids, 'SelfObject')))
        a.return_nothing(); a.end(); return a
    T.function(show, 'ApplyBehaviorToContext', [('Object', 'ContextObject', 0), ('Object', 'SelfObject', 0), ('Object', 'MyInstigatorObject', 0),
                                                ('Object', 'OtherEventParticipantObject', 0), ('Struct', 'EventData', 0), ('Struct', 'KernelInfo', CPF_OUT)],
               show_body)

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
        # GeneralSkillPoints += int(EvaluateInitializationData(GetWillowGlobals().GetGlobalsDefinition().GeneralSkillPointsPerLevelUp, Self)), after the level rose
        a.raw(0x0F); level_of_pri(a, general_points)
        a.raw(146); level_of_pri(a, general_points)
        a.raw(0x38, 68)                                            # PrimitiveCast FloatToInt
        a.call(evaluate, lambda: a.context(lambda: a.context(lambda: a.call(get_willow_globals), lambda: a.call(get_globals_definition)),
                                           lambda: a.instance(points_per_level)), a.self_, lambda: a.raw(0x4A))
        a.raw(0x16)
        a.call(recalculate)
        a.return_nothing(); a.end(); return a
    T.function(controller, 'ExpLevelUp', [('Bool', 'bCheated', 0)], level_up)
    return p


def build_mission(director=False, on_enabled=True):
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
             + t.array('Expressions', 1, expression) + none)
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
    # the player's level as an attribute (swap 7): the replication info context, the object property ExpLevel of it
    pri_context = p.add_export(chain('WillowGame', 'PlayerReplicationInfoAttributeContextResolver'), 'PriContextResolver', w32(0) + none)
    exp_level_property = p.add_export(chain('Engine', 'ObjectPropertyAttributeValueResolver'), 'ExpLevelResolver', w32(0) + t.name_('PropertyName', 'ExpLevel') + none)
    player_level = p.add_export(chain('Engine', 'AttributeDefinition'), 'PlayerExperienceLevel',
                                w32(0) + t.array('ContextResolverChain', 1, w32(pri_context)) + t.array('ValueResolverChain', 1, w32(exp_level_property)) + none)
    # the skill points per level (the note's rule in miniature): 1 when PlayerExperienceLevel >= 5, else the default 0
    level_five = (t.obj('AttributeOperand1', player_level) + t.byte('ComparisonOperator', COMPARISONS.index('OPERATOR_GreaterThanOrEqual'))
                  + t.float('ConstantOperand2', 5.0) + none)
    from_five = data('BaseValueIfTrue', constant=1.0) + t.array('Expressions', 1, level_five) + none
    points_conditional = t.bool('bEnabled', True) + t.array('ConditionalExpressionList', 1, from_five) + data('DefaultBaseValue', constant=0.0) + none
    points_definition = p.add_export(chain('Engine', 'AttributeInitializationDefinition'), 'INI_SkillPointsPerLevelUp',
                                     w32(0) + t.struct_('ConditionalInitialization', 'ConditionalInitialization', points_conditional) + none)
    # the toy player class with a health pool whose base maximum is 10 x 1.5 ^ PlayerExperienceLevel (a value formula, the power from the attribute)
    health_formula = t.bool('bEnabled', True) + data('Multiplier', constant=10.0) + data('Level', constant=1.5) + data('Power', attribute=player_level) + none
    health_definition = p.add_export(chain('Engine', 'AttributeInitializationDefinition'), 'Init_PlayerHealth',
                                     w32(0) + t.struct_('ValueFormula', 'ValueFormula', health_formula) + none)
    health_pool = p.add_export(chain('WillowGame', 'ResourcePoolDefinition'), 'HealthPool', w32(0) + data('BaseMaxValue', definition=health_definition) + none)
    siren = p.add_export(chain('Core', 'Package'), 'GD_Siren', w32(0) + none)
    siren_character = p.add_export(chain('Core', 'Package'), 'Character', w32(0) + none, outer=siren)
    p.add_export(chain('WillowGame', 'PlayerClassDefinition'), 'CharClass_Siren', w32(0) + t.obj('HealthPoolDefinition', health_pool) + none, outer=siren_character)
    p.add_export(chain('WillowGame', 'GlobalsDefinition'), 'Globals',
                 w32(0) + t.float('PlayerInteractionDistance', 420.0) + data('GeneralSkillPointsPerLevelUp', definition=points_definition) + none, outer=general_package)
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
        package_class_import = lambda c: c('Core', 'Package')
        p.add_export(chain('WillowGame', 'WillowAIPawn'), 'Pawn_Marcus', w32(0) + t.obj('MissionDirectives', table) + none, outer=character)
        # his AI-definition provider (NATIVE_MARCUS_USE_CHAIN.md in miniature, invented names). Brain: OnUsed (link id 2) -> check A "Missing" (no such
        # sequence) -> B "Off" (exists, disabled) -> C "On" (exists; enabled when on_enabled) -> the remote event "Fired" when enabled, else X, the
        # mission interface. A's not-enabled output also starts D, a check whose path names no provider. Reactor: "Fired" -> Y (a mission interface behavior
        # on Marcus, which cannot open it: Marcus has no controller). C names a SequenceProvider too: the path wins over that reference.
        aidef = p.add_export(package_class_import(chain), 'AIDef_Marcus', w32(0) + none, outer=character)
        provider = p.add_export(chain('GearboxFramework', 'BehaviorProviderDefinition'), 'AIBehaviorProviderDefinition_0', b'', outer=aidef)
        other_provider = p.add_export(chain('GearboxFramework', 'BehaviorProviderDefinition'), 'OtherProvider', w32(0) + none)

        def path(*names):
            slots = [p.fname('None')] * (6 - len(names)) + [p.fname(n) for n in names]
            return t.struct_('ProviderDefinitionPathName', 'NameBasedObjectPath', t.array('PathComponentNames', 6, b''.join(slots)) + t.byte('IsSubobjectMask', 16) + none)
        marcus_path = path('GD_Marcus', 'Character', 'AIDef_Marcus', 'AIBehaviorProviderDefinition_0')

        def check(name, sequence, path_tags, reference=0):
            body = w32(0) + t.name_('SequenceName', sequence) + path_tags + (t.obj('SequenceProvider', reference) if reference else b'') + none
            return p.add_export(chain('GearboxFramework', 'Behavior_IsSequenceEnabled'), name, body, outer=provider)
        a_ = check('Behavior_IsSequenceEnabled_A', 'Missing', marcus_path)
        b_ = check('Behavior_IsSequenceEnabled_B', 'Off', marcus_path)
        c_ = check('Behavior_IsSequenceEnabled_C', 'On', marcus_path, reference=other_provider)
        d_ = check('Behavior_IsSequenceEnabled_D', 'On', path('Nope', 'Nothing'))
        r_ = p.add_export(chain('WillowGame', 'Behavior_RemoteCustomEvent'), 'Behavior_RemoteCustomEvent_R',
                          w32(0) + t.name_('CustomEventName', 'Fired') + marcus_path + none, outer=provider)
        x_ = p.add_export(chain('WillowGame', 'Behavior_ShowMissionInterface'), 'Behavior_ShowMissionInterface_X', w32(0) + none, outer=provider)
        y_ = p.add_export(chain('WillowGame', 'Behavior_ShowMissionInterface'), 'Behavior_ShowMissionInterface_Y', w32(0) + none, outer=provider)

        def sub(name, first, length): return t.struct_(name, 'SubarrayData', t.int('ArrayIndexAndLength', (first << 16) | length) + none)

        def link(behavior, link_id): return t.int('LinkIdAndLinkedBehavior', behavior | (link_id << 24)) + t.float('ActivateDelay', 0.0) + none

        def var(name, kind): return t.name_('Name', name) + t.byte('Type', kind) + none

        def vlink(prop, kind, first, count):
            return t.name_('PropertyName', prop) + t.byte('VariableLinkType', kind) + t.byte('ConnectionIndex', 0) + sub('LinkedVariables', first, count) + none
        BVAR_OBJECT, BVAR_NAMED, LINK_OUTPUT, LINK_CONTEXT, LINK_INPUT = 1, 5, 2, 3, 1
        brain_behaviors = [(a_, 1, 3), (b_, 4, 2), (c_, 6, 2), (d_, 0, 0), (r_, 0, 0), (x_, 0, 0)]    # (object, first link, link count); X has the Context link
        brain_links = [link(0, 2), link(4, 0), link(1, 1), link(3, 1), link(4, 0), link(2, 1), link(4, 0), link(5, 1)]
        brain = (t.name_('BehaviorSequenceName', 'Brain') + t.bool('bEnabledOnSpawn', True)
                 + t.array('EventData2', 1, t.struct_('UserData', 'BehaviorEventUserData', t.name_('EventName', 'OnUsed') + none)
                           + sub('OutputVariables', 0, 1) + sub('OutputLinks', 0, 1) + none)
                 + t.array('BehaviorData2', len(brain_behaviors), b''.join(
                     t.obj('Behavior', obj) + sub('LinkedVariables', 1 if obj == x_ else 0, 1 if obj == x_ else 0) + sub('OutputLinks', first, count) + none
                     for obj, first, count in brain_behaviors))
                 + t.array('VariableData', 2, var('Who', BVAR_NAMED) + var('Who', BVAR_OBJECT))
                 + t.array('ConsolidatedOutputLinkData', len(brain_links), b''.join(brain_links))
                 + t.array('ConsolidatedVariableLinkData', 2, vlink('Instigator', LINK_OUTPUT, 0, 1) + vlink('Context', LINK_CONTEXT, 1, 1))
                 + t.array('ConsolidatedLinkedVariables', 2, w32(1, 0)) + none)
        reactor = (t.name_('BehaviorSequenceName', 'Reactor') + t.bool('bEnabledOnSpawn', True)
                   + t.array('EventData2', 1, t.struct_('UserData', 'BehaviorEventUserData', t.name_('EventName', 'Fired') + none)
                             + sub('OutputVariables', 0, 0) + sub('OutputLinks', 0, 1) + none)
                   + t.array('BehaviorData2', 1, t.obj('Behavior', y_) + sub('LinkedVariables', 0, 0) + sub('OutputLinks', 0, 0) + none)
                   + t.array('ConsolidatedOutputLinkData', 1, link(0, 0)) + none)
        # Probes (enabled on spawn, its own OnUsed event so the same press runs it): probe behaviors whose PlayerWhoUsedMe carries a selector, two
        # Input links onto the runner's struct fill, and E (a mission interface behavior) whose Context link resolves to nothing, with a default link to Z.
        def context_tags(selector, object_ref=0):
            return t.struct_('PlayerWhoUsedMe', 'BehaviorContextData', (t.obj('ContextObject', object_ref) if object_ref else b'') + t.byte('BehaviorContext', selector) + none)

        def probe(name, selector, object_ref=0):
            return p.add_export(chain('WillowGame', 'Behavior_PlayAIMissionContextDialog'), name, w32(0) + context_tags(selector, object_ref) + none, outer=provider)
        probes = [probe('Probe_Self', 0), probe('Probe_Instigator', 1), probe('Probe_Other', 2), probe('Probe_EventData', 3), probe('Probe_Object', 4, other_provider),
                  probe('Probe_EmptyObject', 4), probe('Probe_Unknown', 5),
                  probe('Probe_Filled', 0, other_provider),        # selector 0 and an object in the data: the Input link below must overwrite both
                  probe('Probe_EmptyLink', 0, other_provider),     # the Input link resolves to nothing: ContextObject becomes None and the selector 4
                  probe('Probe_AfterEmpty', 0)]                    # Z: reached only by the default link of the skipped behavior
        skipped = p.add_export(chain('WillowGame', 'Behavior_ShowMissionInterface'), 'Behavior_ShowMissionInterface_E', w32(0) + none, outer=provider)
        order = probes + [skipped]
        z_index = len(probes) - 1
        event_targets = [i for i in range(len(order)) if i != z_index]
        default_link = t.int('LinkIdAndLinkedBehavior', z_index + (255 << 24) - (1 << 32)) + t.float('ActivateDelay', 0.0) + none      # id 255 = -1
        behavior_rows = b''
        for i, obj in enumerate(order):
            first, count, vfirst, vcount = 0, 0, 0, 0
            if obj == probes[7]: vfirst, vcount = 1, 1          # Probe_Filled: Input link (property PlayerWhoUsedMe) -> variable 0 (named "Who")
            if obj == probes[8]: vfirst, vcount = 2, 1          # Probe_EmptyLink: Input link -> variable 2 (named "Nobody")
            if obj == skipped: vfirst, vcount, first, count = 3, 1, len(event_targets), 1     # E: Context link -> "Nobody"; default link to Z
            behavior_rows += t.obj('Behavior', obj) + sub('LinkedVariables', vfirst, vcount) + sub('OutputLinks', first, count) + none
        probe_links = b''.join(link(i, 2) for i in event_targets) + default_link
        probes_seq = (t.name_('BehaviorSequenceName', 'Probes') + t.bool('bEnabledOnSpawn', True)
                      + t.array('EventData2', 1, t.struct_('UserData', 'BehaviorEventUserData', t.name_('EventName', 'OnUsed') + none)
                                + sub('OutputVariables', 0, 1) + sub('OutputLinks', 0, len(event_targets)) + none)
                      + t.array('BehaviorData2', len(order), behavior_rows)
                      + t.array('VariableData', 3, var('Who', BVAR_NAMED) + var('Who', BVAR_OBJECT) + var('Nobody', BVAR_NAMED))
                      + t.array('ConsolidatedOutputLinkData', len(event_targets) + 1, probe_links)
                      + t.array('ConsolidatedVariableLinkData', 4, vlink('Instigator', LINK_OUTPUT, 0, 1) + vlink('PlayerWhoUsedMe', LINK_INPUT, 1, 1)
                                + vlink('PlayerWhoUsedMe', LINK_INPUT, 2, 1) + vlink('Context', LINK_CONTEXT, 3, 1))
                      + t.array('ConsolidatedLinkedVariables', 4, w32(1, 0, 2, 2)) + none)
        sequences = [brain, reactor, t.name_('BehaviorSequenceName', 'On') + t.bool('bEnabledOnSpawn', on_enabled) + none,
                     t.name_('BehaviorSequenceName', 'Off') + t.bool('bEnabledOnSpawn', False) + none, probes_seq]
        cls_, sup, outer_, name_, _ = p.exports[provider - 1]
        p.exports[provider - 1] = (cls_, sup, outer_, name_, w32(0) + t.array('BehaviorSequences', len(sequences), b''.join(sequences)) + none + w32(0, 0))
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
    globals_export = p.add_export(chain('WillowGame', 'WillowDialogGlobalsDefinition'), 'DialogGlobals',
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

    # Swap 6e (a pawn's component TriggerEvent, NATIVE_DIALOG.md): tag G lives in GenericGroup, whose talk act has no entry for the speaker (Marcus)
    # and bEnableNoMatch, so its output 1 reaches a Trigger act that fires tag S on the instigator; S lives in SpeakerGroup (Marcus's own group), whose
    # ParentGroup is GenericGroup. Tag Z (generic) has no inline act: the link table points to a template talk act with no audio event (silent).
    other = p.add_export(chain('WillowGame', 'WillowDialogNameTag'), 'DialogName_Other', w32(0) + none)
    gtag = event_tag('G', 'P30', False)
    stag = event_tag('S', 'P20', False)
    ztag = event_tag('Z', 'P30', False)
    qtag = event_tag('Q', 'P30', False)
    generic = p.add_export(chain('GearboxFramework', 'GearboxDialogGroup'), 'GenericGroup', b'')
    speaker = p.add_export(chain('GearboxFramework', 'GearboxDialogGroup'), 'SpeakerGroup', b'')
    generic_act = p.add_export(chain('WillowGame', 'WillowDialogAct_Talk'), 'GenericAct', b'', outer=generic)
    trigger_act = p.add_export(chain('WillowGame', 'WillowDialogAct_Trigger'), 'TriggerAct', b'', outer=generic)
    instigator_var = p.add_export(chain('GearboxFramework', 'GearboxDialogVar_Instigator'), 'InstigatorVar', w32(0) + none, outer=generic)
    speaker_act = p.add_export(chain('WillowGame', 'WillowDialogAct_Talk'), 'SpeakerAct', b'', outer=speaker)

    def link_struct(*targets): return t.array('Links', len(targets), w32(*targets)) + none     # one element of an array of DialogOutputLink

    def set_payload(index, payload):
        cls_, sup, outer_, name_, _ = p.exports[index - 1]
        p.exports[index - 1] = (cls_, sup, outer_, name_, payload)
    set_payload(generic_act, w32(0) + t.int('NodeID', 11) + t.array('TalkData', 1, t.obj('NameTag', other) + t.obj('TalkAkEvent', ak['A']) + none)
                + t.bool('bInstigatorTalker', True) + t.bool('bEnableNoMatch', True)
                + t.array('OutputLinks', 2, link_struct() + link_struct(trigger_act)) + none)
    set_payload(trigger_act, w32(0) + t.int('NodeID', 12) + t.obj('DialogEvent', stag) + t.array('VariableLinks', 1, link_struct(instigator_var)) + none)
    set_payload(speaker_act, w32(0) + t.int('NodeID', 13) + t.array('TalkData', 1, t.obj('NameTag', marcus) + t.obj('TalkAkEvent', ak['B']) + none)
                + t.bool('bInstigatorTalker', True) + none)
    generic_events = [(gtag, generic_act), (ztag, 0)]
    set_payload(generic, w32(0) + t.array('DialogEvents', 2, b''.join(t.obj('Tag', tg) + t.bool('bEnabled', True) + (t.obj('OutputAction', a) if a else b'') + none
                                                                      for tg, a in generic_events))
                + t.array('TalkActs', 1, t.array('TalkData', 1, t.obj('NameTag', marcus) + none) + t.bool('bInstigatorTalker', True) + none)
                + t.array('OutputLinksToStructs', 1, t.int('FromNodeID', 2) + t.int('LinkNumber', 0) + t.int('ToNodeID', 3) + none) + none)
    set_payload(speaker, w32(0) + t.array('DialogEvents', 1, t.obj('Tag', stag) + t.bool('bEnabled', True) + t.obj('OutputAction', speaker_act) + none)
                + t.obj('ParentGroup', generic) + none)

    # A pawn's dialog groups (NATIVE_DIALOG_GROUPS.md; every name and value here is invented). Groups: OwnGroup (the body classes' own), NpcGroup (the
    # globals' generic NPC group), DlcGroup (an expansion's NPC group), TemplateGroup (the globals' default template group, a GearboxDialogTemplateGroup),
    # ParentOnly (the parent of ChildA and ChildB, in no talker's list). Each event holds an inline talk act for a speaker name tag and an audio event.
    dlc_name = p.add_export(chain('WillowGame', 'WillowDialogNameTag'), 'DialogName_Dlc', b'')
    group_class, template_class = ('GearboxFramework', 'GearboxDialogGroup'), ('GearboxFramework', 'GearboxDialogTemplateGroup')
    pg = {name: p.add_export(chain(*(template_class if name == 'TemplateGroup' else group_class)), name, b'')
          for name in ('OwnGroup', 'NpcGroup', 'DlcGroup', 'TemplateGroup', 'ParentOnly', 'ChildA', 'ChildB')}
    ptags = {n: event_tag(n, 'P30', False) for n in 'NOKTHPU'}

    def pawn_act(group_name, tag_name, speaker, ak_name):
        return p.add_export(chain('WillowGame', 'WillowDialogAct_Talk'), group_name + '_' + tag_name, w32(0) + t.int('NodeID', 1)
                            + t.array('TalkData', 1, t.obj('NameTag', speaker) + t.obj('TalkAkEvent', ak[ak_name]) + none) + none, outer=pg[group_name])

    def pawn_group(group_name, entries, parent=''):
        body = b''.join(t.obj('Tag', ptags[tag_name]) + t.bool('bEnabled', True) + t.obj('OutputAction', act_) + none for tag_name, act_ in entries)
        set_payload(pg[group_name], w32(0) + t.array('DialogEvents', len(entries), body) + (t.obj('ParentGroup', pg[parent]) if parent else b'') + none)
    # TagH: a generic NPC event whose talk act has no entry for the speaker; its no-match output fires TagT (a template event) on the instigator
    h_act = p.add_export(chain('WillowGame', 'WillowDialogAct_Talk'), 'NpcGroup_H', b'', outer=pg['NpcGroup'])
    h_trigger = p.add_export(chain('WillowGame', 'WillowDialogAct_Trigger'), 'NpcGroup_HTrigger', b'', outer=pg['NpcGroup'])
    set_payload(h_act, w32(0) + t.int('NodeID', 2) + t.array('TalkData', 1, t.obj('NameTag', other) + t.obj('TalkAkEvent', ak['A']) + none)
                + t.bool('bInstigatorTalker', True) + t.bool('bEnableNoMatch', True) + t.array('OutputLinks', 2, link_struct() + link_struct(h_trigger)) + none)
    set_payload(h_trigger, w32(0) + t.int('NodeID', 3) + t.obj('DialogEvent', ptags['T']) + t.array('VariableLinks', 1, link_struct(instigator_var)) + none)
    pawn_group('OwnGroup', [('O', pawn_act('OwnGroup', 'O', marcus, 'A'))])
    pawn_group('NpcGroup', [('N', pawn_act('NpcGroup', 'N', marcus, 'A')), ('K', pawn_act('NpcGroup', 'K', marcus, 'A')), ('H', h_act)])
    pawn_group('DlcGroup', [('K', pawn_act('DlcGroup', 'K', dlc_name, 'B'))])
    pawn_group('TemplateGroup', [('T', pawn_act('TemplateGroup', 'T', marcus, 'C')), ('P', pawn_act('TemplateGroup', 'P', marcus, 'C'))])
    pawn_group('ParentOnly', [('P', pawn_act('ParentOnly', 'P', marcus, 'D')), ('U', pawn_act('ParentOnly', 'U', marcus, 'D'))])
    pawn_group('ChildA', [], 'ParentOnly')
    pawn_group('ChildB', [], 'ParentOnly')
    expansion = p.add_export(chain('WillowGame', 'DlcExpansionDefinition'), 'DlcExpansion', w32(0) + t.array('NPCDialogGroups', 1, w32(pg['DlcGroup'])) + none)
    set_payload(dlc_name, w32(0) + t.obj('DlcExpansion', expansion) + none)
    for body_name, own, npc, speaker in (('BodyNpc', ['OwnGroup'], True, marcus), ('BodyPlain', ['OwnGroup'], False, marcus), ('BodyDlc', ['OwnGroup'], True, dlc_name),
                                         ('BodyDlcPlain', ['OwnGroup'], False, dlc_name), ('BodyParent', ['ChildA', 'ChildB'], False, marcus)):
        p.add_export(chain('WillowGame', 'BodyClassDefinition'), body_name,
                     w32(0) + t.array('DialogGroups', len(own), w32(*[pg[g] for g in own])) + t.bool('bNPCDialog', npc) + t.obj('DialogName', speaker) + none)
    set_payload(globals_export, w32(0) + t.array('Priorities', len(names), w32(*[prio[n] for n in names])) + t.obj('ActiveMissionMinPriorityStart', prio['P20'])
                + t.obj('ActiveSideMissionMinPriority', prio['P35']) + t.obj('ActivePlotMissionMinPriority', prio['P100'])
                + t.array('NPCDialogGroups', 1, w32(pg['NpcGroup'])) + t.obj('DefaultTemplateGroup', pg['TemplateGroup']) + none)

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

    # Scenario H (swap 6b): the use key's press on Marcus runs his OnUsed chain on the VM (NATIVE_MARCUS_USE_CHAIN.md).
    for name, enabled in (('UseChainOn', True), ('UseChainOff', False)):
        (root / f'{name}.upk').write_bytes(build_mission(director=True, on_enabled=enabled).build())

    def use_run(name, *steps):
        proc = subprocess.run([reader, str(root / f'{name}.upk'), '--mission-run', 'ToyMission', '--cooked', str(root), *steps],
                              capture_output=True, text=True, encoding='utf-8')
        assert proc.stdout.strip(), ('no JSON output', proc.returncode, proc.stderr)
        return proc.returncode, json.loads(proc.stdout)

    def use_of(step): return step['use']
    for name, enabled in (('UseChainOn', True), ('UseChainOff', False)):
        code, got = use_run(name, 'script:use')
        check(f'H {name} exit and errors', code == 0 and got['errors'] == [], (code, got['errors'], got['script']['notes'][-3:]))
        use = got['steps'][0]['use']
        lines = use['cascade']
        # A: the sequence does not exist; B: it exists and is disabled; both answer "not enabled" (output 1); D's path names no provider: no output.
        check(f'H {name} A and B not enabled', 'Behavior_IsSequenceEnabled_A(Missing) -> 1' in lines and 'Behavior_IsSequenceEnabled_B(Off) -> 1' in lines, lines)
        check(f'H {name} D: unresolved provider selects nothing', 'Behavior_IsSequenceEnabled_D(On) ->' in lines, lines)
        if enabled:
            # The resolver (NATIVE_BEHAVIOR_CONTEXT.md): selector 0 gives SelfObject (output 2); 1, 2, 3 and unknown give None (0: a kernel-run behavior has no
            # instigator or participant, and EventData is never consulted); 4 gives the struct's own ContextObject (1), or None when it is empty. The runner
            # overwrote the selector and ContextObject of Probe_Filled with the player (output 1, not Self's 2); the empty Input link left None (0) although
            # the data held an object; the behavior with an empty Context link did not run, and its default link reached the last probe (2).
            for probe_name, expected in (('Probe_Self', 2), ('Probe_Instigator', 0), ('Probe_Other', 0), ('Probe_EventData', 0), ('Probe_Object', 1),
                                         ('Probe_EmptyObject', 0), ('Probe_Unknown', 0), ('Probe_Filled', 1), ('Probe_EmptyLink', 0), ('Probe_AfterEmpty', 2)):
                check(f'H probe {probe_name}', f'{probe_name} -> {expected}' in lines, [l for l in lines if l.startswith(probe_name)])
            check('H empty Context link: behavior skipped', not any('Behavior_ShowMissionInterface_E' in line for line in lines), lines)
            # C: the path (Marcus's provider, where "On" is enabled) wins over the SequenceProvider reference (another provider): output 0; the remote
            # event fires and the cascade stops there: Y runs in the Reactor sequence, X (the interface) is never reached
            check('H on: C enabled, path over reference', 'Behavior_IsSequenceEnabled_C(On) -> 0' in lines, lines)
            check('H on: remote event fired, cascade stops', 'Behavior_RemoteCustomEvent_R ->' in lines and 'Behavior_ShowMissionInterface_Y ->' in lines
                  and not any('_X' in line for line in lines) and not use['interface_opened'], use)
        else:
            # C disabled: output 1 reaches X, whose Context is the named variable that the OnUsed payload filled: the player pawn, so its Controller
            # gets the ClientGFxPlayMovie call, which is reported (the toy body, marker 140, is not run)
            check('H off: C not enabled', 'Behavior_IsSequenceEnabled_C(On) -> 1' in lines, lines)
            check('H off: interface opened through the payload variable', use['interface_opened'] and 'Behavior_ShowMissionInterface_X ->' in lines
                  and not any('_Y' in line for line in lines) and not any('Behavior_RemoteCustomEvent' in line for line in lines), use)
            check('H off: the script RPC was replaced', 140 not in [e['source'] for e in got['script']['exp_earned']], got['script']['exp_earned'])

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

    # Scenario S7 (swap 7): the level-up's skill points come from the globals' formula and the new maximum health from the health pool's data, both
    # as host events after the Level event. The reward at stage 4 is 27; R(4) 30, R(5) 48, R(6) 70.
    def level_events(*steps):
        code, got = slice_run(*steps)
        return code, got, [(e['kind'], e['a']) for step in got['steps'] for e in step['events']
                           if e['kind'] in ('experience', 'level', 'skill_points', 'max_health')]

    def leveled(events, level, points, health):
        return ([kind for kind, _ in events] == ['experience', 'level', 'skill_points', 'max_health'] and events[0][1] == '27' and events[1][1] == str(level)
                and events[2][1] == str(points) and abs(float(events[3][1]) - health) < 1e-3)
    flow = ('accept', 'range', 'turnin', 'tick:0.5')
    # 4 -> 5 (pool 30 + 27 = 57): the first level with a point
    code, got, events = level_events('stage:4', 'player:4:30', *flow)
    check('S7 4 -> 5 exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    check('S7 4 -> 5 awards 1 point, health 10 x 1.5^5', leveled(events, 5, 1, 75.9375), events)
    check('S7 no stubs and no notes', got['script']['stubs'] == [] and got['script']['notes'] == [], (got['script']['stubs'], got['script']['notes']))
    # 5 -> 6 (pool 48 + 27 = 75)
    code, got, events = level_events('stage:4', 'player:5:48', *flow)
    check('S7 5 -> 6 awards 1 point, health 10 x 1.5^6', code == 0 and got['errors'] == [] and leveled(events, 6, 1, 113.90625), (events, got['errors']))
    # 3 -> 4 (pool 16 + 27 = 43): below level 5 the formula gives 0, and the event still says so
    code, got, events = level_events('stage:4', 'player:3:16', *flow)
    check('S7 3 -> 4 awards no point', code == 0 and got['errors'] == [] and leveled(events, 4, 0, 50.625), (events, got['errors']))
    # two levels from one reward (pool 44 + 27 = 71 >= R(6)): one event with both points, the health of the last level
    code, got, events = level_events('stage:4', 'player:4:44', *flow)
    check('S7 4 -> 6 awards 2 points once', code == 0 and got['errors'] == [] and leveled(events, 6, 2, 113.90625), (events, got['errors']))
    # no level change (27 < R(5)): neither event is sent
    code, got, events = level_events('stage:4', 'player:4:0', *flow)
    check('S7 no level change, no points or health event', code == 0 and [kind for kind, _ in events] == ['experience'], events)

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

    # Scenario E2 (swap 6e): a pawn's component TriggerEvent through the stock dialog data (NATIVE_DIALOG.md "Component TriggerEvent")
    def component(*steps):
        proc = subprocess.run([reader, str(root / 'DialogMission.upk'), '--mission-run', 'DialogMission', '--cooked', str(root), 'lines:5', *steps],
                              capture_output=True, text=True, encoding='utf-8')
        assert proc.stdout.strip(), ('no JSON output', proc.returncode, proc.stderr)
        return proc.returncode, json.loads(proc.stdout)
    code, got = component('component:DialogName_Marcus|TagG|SpeakerGroup', 'tick:6', 'component:DialogName_Marcus|TagS|SpeakerGroup', 'tick:6',
                          'component:DialogName_Marcus|TagG|GenericGroup', 'tick:6', 'component:DialogName_Marcus|TagZ|GenericGroup', 'tick:6',
                          'component:DialogName_Marcus|TagQ|SpeakerGroup')
    check('E2 exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    rows = [step for step in got['steps'] if step['step'].startswith('component:')]

    def lines_of(step): return [(e['a'], e['c'], e['detail']) for e in step['effects'] if e['kind'] == 'dialog']
    if len(rows) == 5:
        # G through the speaker's ParentGroup: the generic act has no entry for Marcus (no-match output 1), the Trigger act fires S on the instigator
        # (Marcus), whose own group answers with his Talk act: one line, tag S, his audio event B, a pawn talker
        got_lines = lines_of(rows[0])
        check('E2 no-match output reaches the speaker group', len(got_lines) == 1 and got_lines[0][0] == 'TagS' and 'ak=Ak_B' in got_lines[0][2]
              and 'talker=pawn' in got_lines[0][2] and 'outcome=started' in got_lines[0][2] and rows[0]['ok'], rows[0])
        # S directly: the speaker's own group
        check('E2 own tag plays directly', len(lines_of(rows[1])) == 1 and 'ak=Ak_B' in lines_of(rows[1])[0][2], rows[1])
        # G with only the generic group: the Trigger act's talker cannot talk S (no group of his has it): nothing plays
        check('E2 a talker that cannot talk the event stays silent', lines_of(rows[2]) == [] and not rows[2]['ok'], rows[2])
        # Z: the template act has no audio event: a pass-through, no line, no error
        check('E2 an act without audio is silent', lines_of(rows[3]) == [], rows[3])
        # Q: no group has an enabled event for the tag
        check('E2 no matching event, nothing happens', lines_of(rows[4]) == [] and not rows[4]['ok'], rows[4])

    # Scenario E3 (a pawn's dialog groups, NATIVE_DIALOG_GROUPS.md): the body class decides the list and the search runs through it
    steps_e3 = []
    for body, tag_name in (('BodyNpc', 'TagN'), ('BodyPlain', 'TagN'), ('BodyDlc', 'TagK'), ('BodyDlcPlain', 'TagK'), ('BodyNpc', 'TagT'), ('BodyNpc', 'TagH'),
                           ('BodyParent', 'TagP'), ('BodyParent', 'TagU')):
        steps_e3 += [f'pawn:{body}|{tag_name}', 'tick:6']
    code, got = component(*steps_e3)
    check('E3 exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    rows = [step for step in got['steps'] if step['step'].startswith('pawn:')]
    if len(rows) == 8:
        def info(row): return row['pawn']['groups'], row['pawn']['searched']

        def played(row, ak_name): return len(lines_of(row)) == 1 and 'ak=' + ak_name in lines_of(row)[0][2] and 'outcome=started' in lines_of(row)[0][2]
        # an NPC body: its own group, the globals' NPC group, then the default template group; a non-NPC body: its own group and the template group only
        check('E3 NPC body list', info(rows[0])[0] == ['OwnGroup', 'NpcGroup', 'TemplateGroup'], info(rows[0]))
        check('E3 NPC body plays a generic NPC event', played(rows[0], 'Ak_A') and rows[0]['ok'] and info(rows[0])[1] == ['OwnGroup', 'NpcGroup'], rows[0])
        check('E3 non-NPC body has no NPC groups', info(rows[1])[0] == ['OwnGroup', 'TemplateGroup'] and lines_of(rows[1]) == [] and not rows[1]['ok'], rows[1])
        # a name tag with an expansion: its NPC groups between the body's groups and the globals' (only for an NPC body)
        check('E3 expansion list', info(rows[2])[0] == ['OwnGroup', 'DlcGroup', 'NpcGroup', 'TemplateGroup'], info(rows[2]))
        check('E3 expansion group answers first', played(rows[2], 'Ak_B') and 'talker=pawn' in lines_of(rows[2])[0][2] and info(rows[2])[1] == ['OwnGroup', 'DlcGroup'], rows[2])
        check('E3 expansion needs an NPC body', info(rows[3])[0] == ['OwnGroup', 'TemplateGroup'] and lines_of(rows[3]) == [], rows[3])
        # the template group is last, searched by a fresh trigger and skipped when a Trigger act reuses the event data
        check('E3 template group searched by a fresh trigger', played(rows[4], 'Ak_C') and info(rows[4])[1] == ['OwnGroup', 'NpcGroup', 'TemplateGroup'], rows[4])
        check('E3 template group skipped when event data is reused', lines_of(rows[5]) == [] and 'TemplateGroup' not in info(rows[5])[1] and 'NpcGroup' in info(rows[5])[1], rows[5])
        # a parent group is appended at the end of the search (after the template group), once although two groups name it
        check('E3 parent searched after every listed group', info(rows[6])[0] == ['ChildA', 'ChildB', 'TemplateGroup'] and info(rows[6])[1] == ['ChildA', 'ChildB', 'TemplateGroup']
              and played(rows[6], 'Ak_C'), rows[6])
        check('E3 parent appended once at the end', info(rows[7])[1] == ['ChildA', 'ChildB', 'TemplateGroup', 'ParentOnly'] and played(rows[7], 'Ak_D'), rows[7])
    else:
        check('E3 rows', False, len(rows))

if failures:
    print(f'{len(failures)} mission script check(s) failed:')
    for failure in failures: print('  - ' + failure)
    sys.exit(1)
print('mission script synthetic coverage passed.')
