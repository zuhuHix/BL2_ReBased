"""Synthetic pawn-health coverage (src/combat.*, docs/verification/NATIVE_COMBAT_GROUNDWORK.md sections 1 and 2).

Builds toy `Engine`, `WillowGame` and `ToyPawns` packages with invented classes (the names are the vocabulary the unit reads: Actor.Role, the
ResourcePoolReference struct, ResourcePoolManager.ResourcePools / NextPoolGUID, ResourcePool, ResourcePoolDefinition.Resource, Pawn.HealthPool /
HealthVar / HealthMaxVar, WillowAIPawn, WillowMind) and a toy pool script, then runs `ow-package --combat-run` and checks results computed by hand.
Nothing here comes from the game. The toy pool's InitializeAuthoritative (the real one is script and runs on the VM unchanged) takes Definition and
AssociatedProvider from its arguments, sets bIsAuthoritative, MaxValue and CurrentValue from the definition's StartingValue and MinValue from its
BaseMinValue; ShutDown clears Definition, AssociatedProvider and the values.

The invented data: a pool for the resource Health (StartingValue 40.5, BaseMinValue -10) and a second definition naming the same resource (9), pools for
the resource Ammo (7, and a second definition 3), a pawn archetype Pawn_Toy with HealthVar 11 and HealthMaxVar 12 and an AI class that names the health pool.

The stock hit chain (src/combat_hit.cpp, docs/verification/NATIVE_DAMAGE_DEATH.md sections 0 and 5) is checked with a toy `TakeDamage` that has the order of the real
chain and none of its arithmetic: if the pawn is demigod (the installed IsDemigodMode asks its AI component's demigod hold) it sets health to the floor 1.0, else to 0;
it then calls NotifyTakeHit. The real script computes health - damage and its floor in script; here the checks are about what this unit adds: the demigod hold
natives (HoldDemigod / ReleaseDemigod / DemigodOnHold keep the reasons, a pawn stays demigod until every hold is released), SetHealth(1.0) staying exactly 1.0, and the
order of the events (health is set before NotifyTakeHit raises OnTakeDamage).

The death chain (src/combat_death.cpp, docs/verification/COMBAT_DEATH.md) is checked with a toy killing pawn class `WillowDoomedPawn`: its TakeDamage sets health below 0 and
calls NotifyTakeHit and HandleHealthDepleted, whose toy scripts call the mission tracker's native NotifyPawnDied and Died, which calls SetHealth(0) and the game's
script Killed through WorldInfo.Game. The checks are about what this unit adds: enableDeaths makes WorldInfo.Game and WorldInfo.GRI.MissionTracker (without them the
script's context calls land on None), the call-order trace (`order`) shows the chain in the order the scripts run it, NotifyPawnDied is a bound native, and the
post-state of the pawn (health 0, no valid pool after destroyhealth).

The kill experience (src/combat_xp.cpp, docs/verification/COMBAT_KILL_XP.md) is checked on the same toy kill: the toy game's script Killed calls the two natives of
WillowExperiencePipeline, and a toy `GD_Globals.General.Globals` (in a toy Startup package) holds invented data: the base formula is a constant 20 scaled by 1.5 (so every
kill is worth 30 for the party, whatever the victim), the party weights are (killer bonus 0, weight 1) for one player and (0.25, 0.5) for two, and the level-difference table has
two rows (1: higher 1.5, lower 0.8; 2: higher 2.0, lower 0.5). Victim level 5. The toy ExpEarn is a script that stores its argument; the amounts it was given are in the events.
  - one player, level 4 (victim above by 1): 30 x 1.5 = 45;
  - two players, the killer at level 4 and a partner at level 7: the killer (0.25 + 0.5) x 30 = 22.5 x 1.5 = 33.75 truncated to 33; the partner 0.5 x 30 = 15 x 0.5 (victim below
    by 2) = 7.5 truncated to 7;
  - one player at level 1 (difference 4, past the table's last row: the last row): 30 x 2.0 = 60; at level 5 (equal): 30.

The melee swing (src/combat_hit.cpp, docs/verification/COMBAT_MELEE.md) is checked with a toy `MeleeDefinition` (Damage = constant 40 scaled by 1.5, source class
WillowDmgSource_Melee, a toy damage type definition), a toy thrown rock (`Behavior_AIThrowProjectileAtTarget` whose ChildProjectileBaseValues carry a ProjectileDamage entry of
constant 10 scaled by 1.5, and a `Behavior_CauseDamage` with DamageFormula scale 2.0 and source WillowDmgSource_Grenade) and a toy player pawn whose TakeDamage stores its Damage argument
as the health: the step must read the damage from the data (60; 10 x 1.5 x 2.0 = 30), take the source class and the type from the definition, and refuse an object that delivers nothing.

The down state's natives (src/combat_down.cpp, docs/verification/COMBAT_DOWN_RESPAWN.md) are checked with a toy `WillowDownPawn` whose TakeDamage sets bIsInjured, calls
SetSecondWindReason(3) and SetCanTarget(false), then sets the health to 3.0, to 7.0 if IsInjured() answers true and to 1.0 if CanTarget() still answers true: the run must log the two
calls, end at health 7.0 (IsInjured read the flag the script set, CanTarget read the false SetCanTarget stored) and leave those natives out of the stubs.

Checked (the numbers are the rules of the note):
  - spawn: the pool lands in slot 0 with GUID 0, the manager's Owner is the mind, GetHealth / GetMaxHealth answer the pool (40.5), HealthVar stays 11
    (not written while the pool is valid), HealthMaxVar becomes the pool's maximum (SetHealthResourcePoolDefinition);
  - SetHealth: 0.5 becomes 0, -3 is passed on as -3 (the pool's minimum is -10), 1.5 stays, 500 clamps to the maximum; SetMaxHealth writes HealthMaxVar only;
  - one pool per resource: a second definition of the same resource is refused (invalid reference, nothing changes); another resource takes slot 1, GUID 1;
  - a reference is valid only while its slot holds a pool of the same GUID: after the pool is deleted and another made in the same slot (the lowest free
    one, the next GUID) the old copy of the reference is stale although the slot is used again;
  - DeletePoolByReference on the health pool: HealthPool becomes the invalid reference (index 255, GUID 255), GetHealth falls back to HealthVar and
    GetMaxHealth to HealthMaxVar, SetHealth writes HealthVar, and the next pool takes the freed slot 0.
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
    """Same toy package writer as tests/mission_script_test.py."""
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
    def obj(self, name, ref): return self.tag(name, 'ObjectProperty', 4, w32(ref))
    def struct_(self, name, type_, body): return self.tag(name, 'StructProperty', len(body), body, self.p.fname(type_))
    def array(self, name, count, body): return self.tag(name, 'ArrayProperty', 4 + len(body), w32(count) + body)
    def none(self): return self.p.fname('None')


class Asm:
    """One function body; object references are 4 bytes in the file and 8 in memory (as in tests/vm_test.py)."""
    def __init__(self):
        self.b = bytearray()
        self.refs = 0

    def raw(self, *bs): self.b += bytes(bs)
    def ref(self, r): self.b += struct.pack('<i', r); self.refs += 1
    def w(self, v): self.b += struct.pack('<H', v)
    def end(self): self.raw(0x53)

    def local(self, ids, name): self.raw(0x00); self.ref(ids[name])
    def instance(self, ref): self.raw(0x01); self.ref(ref)
    def context(self, obj, body):               # Context(obj, skip, return property, flags, body)
        self.raw(0x19); obj(); self.w(0); self.ref(0); self.raw(0); body()
    def float_const(self, v): self.raw(0x1E); self.b += struct.pack('<f', v)
    def return_nothing(self): self.raw(0x04, 0x0B)
    def class_context(self, class_ref, body):   # ClassContext(ObjectConst(class), skip, return property, flags, body): a static call
        self.raw(0x12, 0x20); self.ref(class_ref); self.w(0); self.ref(0); self.raw(0); body()
    def here(self): return len(self.b) + 4 * self.refs              # in-memory offset (references are 8 bytes there)
    def final(self, function, *args):                               # FinalFunction(function, args...)
        self.raw(0x1C); self.ref(function)
        for arg in args: arg()
        self.raw(0x16)


FUNC_NATIVE, FUNC_FINAL, FUNC_DEFINED, FUNC_PUBLIC, FUNC_STATIC = 0x400, 0x1, 0x2, 0x20000, 0x2000
CPF_PARM, CPF_OUT, CPF_OPT, CPF_RET = 0x80, 0x100, 0x10, 0x400
ROLES = ['ROLE_None', 'ROLE_SimulatedProxy', 'ROLE_AutonomousProxy', 'ROLE_Authority']
NATIVE = FUNC_NATIVE | FUNC_PUBLIC


class Toy:
    """Reflection helpers over one package: classes, properties, structs, functions (as in tests/mission_script_test.py)."""
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

    def struct_(self, owner, name): return self.p.add_export(self.imp['ScriptStruct'], name, w32(0) * 4, outer=owner)

    def function(self, owner, name, declared, body=None, flags=FUNC_FINAL | FUNC_DEFINED | FUNC_PUBLIC, result=None):
        """declared: [(kind, name, extra flags[, type ref])]; result: kind or (kind, type ref); `body(ids)` returns an Asm (None for a native)."""
        result_kind, result_ref = (result if isinstance(result, tuple) else (result, 0)) if result else (None, 0)
        ordered = ([(result_kind, 'ReturnValue', CPF_PARM | CPF_OUT | CPF_RET, result_ref)] if result else []) + \
                  [(d[0], d[1], CPF_PARM | d[2], d[3] if len(d) > 3 else 0) for d in reversed(declared)]
        func_index = len(self.p.exports) + 1 + len(ordered)
        ids = {}
        for kind, pname, pflags, type_ref in ordered: ids[pname] = self.prop(kind, func_index, pname, pflags, type_ref)
        asm = body(ids) if body else None
        script = bytes(asm.b) if asm else b''
        memory = len(script) + 4 * asm.refs if asm else 0
        payload = u16(0) + w32(*([0] * 10)) + w32(memory, len(script)) + script
        payload += u16(0) + bytes([0]) + u32(flags) + self.p.fname(name)
        assert self.p.add_export(self.imp['Function'], name, payload, outer=owner) == func_index
        ids['__self__'] = func_index
        return ids


def build_engine():
    T = Toy(('Byte', 'Object', 'Bool', 'Float', 'Array', 'Struct', 'Class', 'Name'))
    p = T.p
    actor = T.cls('Actor')
    T.prop('Byte', actor, 'Role')
    T.prop('Object', actor, 'WorldInfo')
    T.prop('Object', actor, 'Owner')
    T.prop('Bool', actor, 'bCanTarget')
    T.function(actor, 'SetCanTarget', [('Bool', 'bCanTargetIn', 0)], None, NATIVE)
    T.function(actor, 'CanTarget', [], None, NATIVE, 'Bool')
    p.add_export(T.imp['Enum'], 'ENetRole', w32(0) + T.none + w32(0) + w32(len(ROLES)) + b''.join(p.fname(r) for r in ROLES), outer=actor)
    reference = T.struct_(actor, 'ResourcePoolReference')
    T.prop('Object', reference, 'PoolManager')
    T.prop('Byte', reference, 'PoolIndexInManager')
    T.prop('Byte', reference, 'PoolGUID')
    T.prop('Object', reference, 'Data')
    T.function(actor, 'IsResourcePoolValid', [('Struct', 'PoolReference', CPF_OUT, reference)], None, NATIVE, 'Bool')
    T.function(actor, 'GetResourcePoolCurrentValueOrDefault', [('Struct', 'PoolReference', CPF_OUT, reference), ('Float', 'DefaultValue', 0)], None, NATIVE, 'Float')
    world = T.cls('WorldInfo', super_ref=actor)
    T.prop('Float', world, 'TimeSeconds')
    T.prop('Object', world, 'Game')
    T.prop('Object', world, 'GRI')
    controller = T.cls('Controller', super_ref=actor)
    T.prop('Object', controller, 'Pawn')
    T.prop('Object', controller, 'ResourcePoolManager')
    pawn = T.cls('Pawn', super_ref=actor)
    T.prop('Object', pawn, 'Controller')
    T.prop('Struct', pawn, 'HealthPool', type_ref=reference)
    T.prop('Float', pawn, 'HealthVar')
    T.prop('Float', pawn, 'HealthMaxVar')
    T.prop('Bool', pawn, 'bIsInjured')
    T.function(pawn, 'IsInjured', [], None, NATIVE, 'Bool')
    T.function(pawn, 'GetHealth', [], None, NATIVE, 'Float')
    T.function(pawn, 'SetHealth', [('Float', 'NewHealth', 0)], None, NATIVE)
    T.function(pawn, 'GetMaxHealth', [], None, NATIVE, 'Float')
    T.function(pawn, 'SetMaxHealth', [('Float', 'NewMaxHealth', 0)], None, NATIVE)
    T.function(pawn, 'SetHealthResourcePoolDefinition', [('Object', 'ResourcePoolManager', CPF_OUT), ('Object', 'Definition', 0)], None, NATIVE)
    T.function(pawn, 'NotifyTakeHit', [('Object', 'InstigatedBy', 0), ('Float', 'HitLocation', 0), ('Float', 'Damage', 0), ('Class', 'DamageType', 0),
                                       ('Float', 'Momentum', 0), ('Object', 'Pipeline', 0)], None, NATIVE)

    resource = T.cls('ResourceDefinition')
    T.prop('Class', resource, 'DefaultResourcePoolClass')
    T.prop('Bool', resource, 'bIntegerOnlyUpdates')
    pool_definition = T.cls('ResourcePoolDefinition')
    T.prop('Object', pool_definition, 'Resource')
    starting = T.prop('Float', pool_definition, 'StartingValue')
    base_min = T.prop('Float', pool_definition, 'BaseMinValue')
    pool = T.cls('ResourcePool')
    guid = T.prop('Byte', pool, 'PoolGUID')
    definition = T.prop('Object', pool, 'Definition')
    provider = T.prop('Object', pool, 'AssociatedProvider')
    authoritative = T.prop('Bool', pool, 'bIsAuthoritative')
    current, maximum, minimum = (T.prop('Float', pool, n) for n in ('CurrentValue', 'MaxValue', 'MinValue'))
    for name in ('RateRemainder', 'PoolIdleDelayStartTime', 'LastCurrentValue', 'LastMinValue', 'LastMaxValue'): T.prop('Float', pool, name)
    T.prop('Bool', pool, 'bCreatedAndNotModified')

    def initialize(ids):        # Definition = InDefinition; AssociatedProvider = Provider; bIsAuthoritative = true; Max = Definition.StartingValue; Min = ...BaseMinValue; Current = Max
        a = Asm()
        a.raw(0x0F); a.instance(definition); a.local(ids, 'InDefinition')
        a.raw(0x0F); a.instance(provider); a.local(ids, 'ProviderToAssociatePoolWith')
        a.raw(0x14); a.instance(authoritative); a.raw(0x27)
        a.raw(0x0F); a.instance(maximum); a.context(lambda: a.instance(definition), lambda: a.instance(starting))
        a.raw(0x0F); a.instance(minimum); a.context(lambda: a.instance(definition), lambda: a.instance(base_min))
        a.raw(0x0F); a.instance(current); a.instance(maximum)
        a.return_nothing(); a.end(); return a
    T.function(pool, 'InitializeAuthoritative', [('Object', 'InDefinition', 0), ('Object', 'ProviderToAssociatePoolWith', 0)], initialize)

    def shut_down(ids):
        a = Asm()
        a.raw(0x0F); a.instance(definition); a.raw(0x2A)
        a.raw(0x0F); a.instance(provider); a.raw(0x2A)
        a.raw(0x0F); a.instance(current); a.float_const(0.0)
        a.raw(0x0F); a.instance(maximum); a.float_const(0.0)
        a.return_nothing(); a.end(); return a
    T.function(pool, 'ShutDown', [], shut_down)
    T.function(pool, 'Reinitialize', [], lambda ids: (lambda a: (a.return_nothing(), a.end(), a)[2])(Asm()))

    manager = T.cls('ResourcePoolManager', super_ref=actor)
    T.array_of(manager, 'ResourcePools', 'Object')
    T.prop('Byte', manager, 'NextPoolGUID')
    T.function(manager, 'CreatePool', [('Object', 'Mgr', CPF_OUT), ('Object', 'ActorOwner', 0), ('Object', 'PoolDefinition', 0), ('Bool', 'bReinitializeExistingPool', CPF_OPT)],
               None, NATIVE | FUNC_STATIC, ('Struct', reference))
    T.function(manager, 'DeletePoolByReference', [('Struct', 'PoolReference', CPF_OUT, reference)], None, NATIVE)
    return p


def build_gearbox():
    """The AI component's demigod hold natives, invented body-less declarations (the real ones are native)."""
    T = Toy(('Byte', 'Object', 'Bool', 'Name'))
    component = T.cls('AIComponent')
    T.function(component, 'HoldDemigod', [('Name', 'Reason', 0), ('Object', 'Source', 0)], None, NATIVE)
    T.function(component, 'ReleaseDemigod', [('Name', 'Reason', 0), ('Object', 'Source', 0)], None, NATIVE)
    T.function(component, 'DemigodOnHold', [], None, NATIVE, 'Bool')
    return T.p


def build_willowgame():
    T = Toy(('Byte', 'Object', 'Int', 'Float', 'Bool', 'Class', 'Struct', 'Array'))
    p = T.p
    engine = p.add_import_full('Package', 0, 'Engine')
    gearbox = p.add_import_full('Package', 0, 'GearboxFramework')
    pawn = p.add_import_full('Class', engine, 'Pawn')
    controller = p.add_import_full('Class', engine, 'Controller')
    set_health = p.add_import_full('Function', pawn, 'SetHealth')
    notify = p.add_import_full('Function', pawn, 'NotifyTakeHit')
    ai_class = T.cls('AIClassDefinition')
    T.prop('Object', ai_class, 'HealthPoolDefinition')
    T.prop('Object', ai_class, 'AIDef')
    mind = T.cls('WillowMind', super_ref=controller)
    T.prop('Object', mind, 'AIClass')
    component_property = T.prop('Object', mind, 'AIComponent')
    T.cls('WillowAIComponent', super_ref=p.add_import_full('Class', gearbox, 'AIComponent'))
    on_hold = p.add_import_full('Function', p.add_import_full('Class', gearbox, 'AIComponent'), 'DemigodOnHold')
    willow_pawn = T.cls('WillowPawn', super_ref=pawn)
    T.prop('Byte', willow_pawn, 'SecondWindReason')
    set_wind = T.function(willow_pawn, 'SetSecondWindReason', [('Byte', 'Reason', 0)], None, NATIVE)['__self__']
    clear_wind = T.function(willow_pawn, 'ClearSecondWindReason', [], None, NATIVE)['__self__']
    ai_pawn = T.cls('WillowAIPawn', super_ref=willow_pawn)
    T.prop('Object', ai_pawn, 'AIClass')
    T.prop('Int', ai_pawn, 'ExpLevel')
    T.prop('Int', ai_pawn, 'GameStage')
    mind_property = T.prop('Object', ai_pawn, 'MyWillowMind')

    # IsDemigodMode(): MyWillowMind.AIComponent.DemigodOnHold() (the installed script also asks the mind's own flag)
    def is_demigod(ids):
        a = Asm()
        a.raw(0x04)
        a.context(lambda: a.context(lambda: a.instance(mind_property), lambda: a.instance(component_property)), lambda: a.final(on_hold))
        a.end(); return a
    demigod = T.function(ai_pawn, 'IsDemigodMode', [], is_demigod, FUNC_FINAL | FUNC_DEFINED | FUNC_PUBLIC, 'Bool')['__self__']

    # TakeDamage(...): if (IsDemigodMode()) SetHealth(1.0) else SetHealth(0.0); NotifyTakeHit(InstigatedBy, HitLocation, Damage, DamageType, Momentum, Pipeline)
    def take_damage(ids):
        def build(otherwise, notify_at):
            a = Asm()
            a.raw(0x07); a.w(otherwise); a.final(demigod)
            a.final(set_health, lambda: a.float_const(1.0))
            a.raw(0x06); a.w(notify_at)
            here_else = a.here()
            a.final(set_health, lambda: a.float_const(0.0))
            here_notify = a.here()
            a.final(notify, *[(lambda n=n: a.local(ids, n)) for n in ('InstigatedBy', 'HitLocation', 'Damage', 'DamageType', 'Momentum', 'Pipeline')])
            a.return_nothing(); a.end()
            return a, here_else, here_notify
        _, here_else, here_notify = build(0, 0)
        return build(here_else, here_notify)[0]
    T.function(ai_pawn, 'TakeDamage', [('Float', 'Damage', 0), ('Object', 'InstigatedBy', 0), ('Float', 'HitLocation', 0), ('Float', 'Momentum', 0),
                                       ('Class', 'DamageType', 0), ('Float', 'HitInfo', 0), ('Object', 'DamageCauser', 0), ('Object', 'Pipeline', 0)], take_damage)

    # the death chain (WillowDoomedPawn): TakeDamage -> SetHealth(-5) -> NotifyTakeHit -> HandleHealthDepleted -> tracker.NotifyPawnDied -> Died -> SetHealth(0) -> Game.Killed
    actor = p.add_import_full('Class', engine, 'Actor')
    world_info = p.add_import_full('Class', engine, 'WorldInfo')
    world_property = p.add_import_full('ObjectProperty', actor, 'WorldInfo')
    game_property = p.add_import_full('ObjectProperty', world_info, 'Game')
    gri_property = p.add_import_full('ObjectProperty', world_info, 'GRI')
    game_info = T.cls('WillowGameInfo', super_ref=actor)
    T.prop('Int', game_info, 'EffectiveNumPlayers')
    # the kill experience data and the two natives (static, on WillowExperiencePipeline)
    globals_class = T.cls('GlobalsDefinition')
    data_struct = T.struct_(globals_class, 'AttributeInitializationData')
    for name in ('BaseValueConstant', 'BaseValueScaleConstant'): T.prop('Float', data_struct, name)
    for name in ('BaseValueAttribute', 'InitializationDefinition'): T.prop('Object', data_struct, name)
    T.prop('Struct', globals_class, 'BaseEnemyExperienceFormula', type_ref=data_struct)
    weight_struct = T.struct_(globals_class, 'ExpAwardWeight')
    T.prop('Int', weight_struct, 'Players')
    for name in ('KillerExpBonus', 'ExpWeight'): T.prop('Float', weight_struct, name)
    T.array_of(globals_class, 'ExpAwardWeights', 'Struct', weight_struct)
    scale_struct = T.struct_(globals_class, 'ExpScaleByLevel')
    T.prop('Int', scale_struct, 'LevelDifference')
    for name in ('HigherLevelEnemyExpScale', 'LowerLevelEnemyExpScale'): T.prop('Float', scale_struct, name)
    T.array_of(globals_class, 'ExpScaleByLevelDifference', 'Struct', scale_struct)
    pipeline_x = T.cls('WillowExperiencePipeline')
    calculate_function = T.function(pipeline_x, 'CalculateExperiencePointsForKill', [('Object', 'Killer', 0), ('Object', 'KilledPawn', 0), ('Bool', 'bKilledByCritical', 0),
                                    ('Class', 'DamageSource', 0), ('Object', 'DamageTypeDef', 0)], None, NATIVE | FUNC_STATIC, 'Float')['__self__']
    award_function = T.function(pipeline_x, 'AwardCombatExperienceToParty', [('Float', 'ExperiencePoints', 0), ('Object', 'Killer', 0), ('Object', 'KilledActor', 0)],
                                None, NATIVE | FUNC_STATIC)['__self__']

    def killed_body(ids):             # AwardCombatExperienceToParty(CalculateExperiencePointsForKill(Killer, KilledPawn, false, None, None), Killer, KilledPawn)
        a = Asm()
        local = lambda n: (lambda: a.local(ids, n))
        points = lambda: a.class_context(pipeline_x, lambda: a.final(calculate_function, local('Killer'), local('KilledPawn'), lambda: a.raw(0x28), lambda: a.raw(0x2A), lambda: a.raw(0x2A)))
        a.class_context(pipeline_x, lambda: a.final(award_function, points, local('Killer'), local('KilledPawn')))
        a.return_nothing(); a.end(); return a
    killed_function = T.function(game_info, 'Killed', [('Object', 'Killer', 0), ('Object', 'KilledPawn', 0)], killed_body)['__self__']
    tracker = T.cls('MissionTracker', super_ref=actor)
    notify_died = T.function(tracker, 'NotifyPawnDied', [('Object', 'KilledPawn', 0)], None, NATIVE)['__self__']
    replication = T.cls('WillowGameReplicationInfo', super_ref=actor)
    tracker_property = T.prop('Object', replication, 'MissionTracker')
    T.array_of(replication, 'PRIArray', 'Object')
    doomed = T.cls('WillowDoomedPawn', super_ref=ai_pawn)

    def died(ids):                    # SetHealth(0.0); WorldInfo.Game.Killed(Killer, Self)
        a = Asm()
        a.final(set_health, lambda: a.float_const(0.0))
        a.context(lambda: a.context(lambda: a.instance(world_property), lambda: a.instance(game_property)),
                  lambda: a.final(killed_function, lambda: a.local(ids, 'Killer'), lambda: a.raw(0x17)))
        a.return_nothing(); a.end(); return a
    died_function = T.function(doomed, 'Died', [('Object', 'Killer', 0)], died)['__self__']

    def depleted(ids):                # WorldInfo.GRI.MissionTracker.NotifyPawnDied(Self); Died(InstigatedBy)
        a = Asm()
        a.context(lambda: a.context(lambda: a.instance(world_property), lambda: a.instance(gri_property)),
                  lambda: a.context(lambda: a.instance(tracker_property), lambda: a.final(notify_died, lambda: a.raw(0x17))))
        a.final(died_function, lambda: a.local(ids, 'InstigatedBy'))
        a.return_nothing(); a.end(); return a
    depleted_function = T.function(doomed, 'HandleHealthDepleted', [('Object', 'InstigatedBy', 0)], depleted)['__self__']

    def doomed_take_damage(ids):      # SetHealth(-5.0); NotifyTakeHit(...); HandleHealthDepleted()
        a = Asm()
        a.final(set_health, lambda: a.float_const(-5.0))
        a.final(notify, *[(lambda n=n: a.local(ids, n)) for n in ('InstigatedBy', 'HitLocation', 'Damage', 'DamageType', 'Momentum', 'Pipeline')])
        a.final(depleted_function, lambda: a.local(ids, 'InstigatedBy'))
        a.return_nothing(); a.end(); return a
    T.function(doomed, 'TakeDamage', [('Float', 'Damage', 0), ('Object', 'InstigatedBy', 0), ('Float', 'HitLocation', 0), ('Float', 'Momentum', 0),
                                      ('Class', 'DamageType', 0), ('Float', 'HitInfo', 0), ('Object', 'DamageCauser', 0), ('Object', 'Pipeline', 0)], doomed_take_damage)

    # the down state's natives (WillowDownPawn): bIsInjured = true; SetSecondWindReason(3); SetCanTarget(false); SetHealth(3.0); if (IsInjured()) SetHealth(7.0); if (CanTarget()) SetHealth(1.0);
    # ClearSecondWindReason()
    pawn_import = p.add_import_full('Class', engine, 'Pawn')
    injured_property = p.add_import_full('BoolProperty', pawn_import, 'bIsInjured')
    set_can_target = p.add_import_full('Function', actor, 'SetCanTarget')
    can_target = p.add_import_full('Function', actor, 'CanTarget')
    is_injured = p.add_import_full('Function', pawn_import, 'IsInjured')
    down = T.cls('WillowDownPawn', super_ref=ai_pawn)

    def down_take_damage(ids):
        def build(after_injured, after_target):
            a = Asm()
            a.raw(0x14); a.instance(injured_property); a.raw(0x27)
            a.final(set_wind, lambda: a.raw(0x24, 3))
            a.final(set_can_target, lambda: a.raw(0x28))
            a.final(set_health, lambda: a.float_const(3.0))
            a.raw(0x07); a.w(after_injured); a.final(is_injured)
            a.final(set_health, lambda: a.float_const(7.0))
            here_injured = a.here()
            a.raw(0x07); a.w(after_target); a.final(can_target)
            a.final(set_health, lambda: a.float_const(1.0))
            here_target = a.here()
            a.final(clear_wind)
            a.return_nothing(); a.end()
            return a, here_injured, here_target
        _, here_injured, here_target = build(0, 0)
        return build(here_injured, here_target)[0]
    T.function(down, 'TakeDamage', [('Float', 'Damage', 0), ('Object', 'InstigatedBy', 0), ('Float', 'HitLocation', 0), ('Float', 'Momentum', 0),
                                    ('Class', 'DamageType', 0), ('Float', 'HitInfo', 0), ('Object', 'DamageCauser', 0), ('Object', 'Pipeline', 0)], down_take_damage)

    # the classes the hit step builds around the chain
    pipeline = T.cls('WillowDamagePipeline')
    for name in ('DamageTypeDef', 'ImpactDefinition', 'DirectHitObject'): T.prop('Object', pipeline, name)
    T.prop('Bool', pipeline, 'bCanHitFriendly')
    T.cls('WillowDmgSource_Bullet')
    # the melee swing: the definition classes and their data, the damage sources, the toy damage type definition
    T.cls('WillowDmgSource_Melee'), T.cls('WillowDmgSource_Grenade')
    T.cls('DamageTypeDefinition')
    melee = T.cls('MeleeDefinition')
    T.prop('Struct', melee, 'Damage', type_ref=data_struct)
    T.prop('Class', melee, 'DamageSource')
    T.prop('Object', melee, 'DamageTypeDefinition')
    throw = T.cls('Behavior_AIThrowProjectileAtTarget')
    projectile_value = T.struct_(throw, 'ProjectileBaseValue')
    T.prop('Object', projectile_value, 'Attribute')
    T.prop('Struct', projectile_value, 'BaseValue', type_ref=data_struct)
    T.array_of(throw, 'ChildProjectileBaseValues', 'Struct', projectile_value)
    cause = T.cls('Behavior_CauseDamage')
    T.prop('Struct', cause, 'DamageFormula', type_ref=data_struct)
    T.prop('Class', cause, 'DamageSource')
    T.prop('Object', cause, 'DamageTypeDefinition')
    player_controller = T.cls('WillowPlayerController', super_ref=controller)
    T.prop('Object', player_controller, 'PlayerReplicationInfo')
    last_exp = T.prop('Int', player_controller, 'LastExp')
    T.function(player_controller, 'ExpEarn', [('Int', 'Exp', 0), ('Byte', 'Source', 0), ('Byte', 'Type', 0)],
               lambda ids: (lambda a: (a.raw(0x0F), a.instance(last_exp), a.local(ids, 'Exp'), a.return_nothing(), a.end(), a)[5])(Asm()))
    player_pawn = T.cls('WillowPlayerPawn', super_ref=pawn)

    def player_take_damage(ids):      # SetHealth(Damage): the health is what arrived
        a = Asm()
        a.final(set_health, lambda: a.local(ids, 'Damage'))
        a.return_nothing(); a.end(); return a
    T.function(player_pawn, 'TakeDamage', [('Float', 'Damage', 0), ('Object', 'InstigatedBy', 0), ('Float', 'HitLocation', 0), ('Float', 'Momentum', 0),
                                           ('Class', 'DamageType', 0), ('Float', 'HitInfo', 0), ('Object', 'DamageCauser', 0), ('Object', 'Pipeline', 0)], player_take_damage)
    T.prop('Int', T.cls('WillowPlayerReplicationInfo', super_ref=actor), 'ExpLevel')
    return p


def build_pawns():
    p = Package()
    t = Tags(p)
    none = t.none()
    engine = p.add_import_full('Package', 0, 'Engine')
    willow = p.add_import_full('Package', 0, 'WillowGame')
    resource_class = p.add_import_full('Class', engine, 'ResourceDefinition')
    definition_class = p.add_import_full('Class', engine, 'ResourcePoolDefinition')
    pool_class = p.add_import_full('Class', engine, 'ResourcePool')

    def resource(name): return p.add_export(resource_class, name, w32(0) + t.tag('DefaultResourcePoolClass', 'ClassProperty', 4, w32(pool_class)) + none)

    def pool_definition(name, resource_ref, starting, minimum=0.0):
        return p.add_export(definition_class, name, w32(0) + t.obj('Resource', resource_ref) + t.float('StartingValue', starting) + t.float('BaseMinValue', minimum) + none)
    health, ammo = resource('Health'), resource('Ammo')
    health_definition = pool_definition('Pool_Health', health, 40.5, -10.0)
    pool_definition('Pool_Health_Copy', health, 9.0)
    pool_definition('Pool_Ammo', ammo, 7.0)
    pool_definition('Pool_Ammo_Copy', ammo, 3.0)
    ai_class = p.add_export(p.add_import_full('Class', willow, 'AIClassDefinition'), 'AIClass_Toy',
                            w32(0) + t.obj('HealthPoolDefinition', health_definition) + t.obj('AIDef', health_definition) + none)
    p.add_export(p.add_import_full('Class', willow, 'WillowAIPawn'), 'Pawn_Toy',
                 w32(0) + t.obj('AIClass', ai_class) + t.float('HealthVar', 11.0) + t.float('HealthMaxVar', 12.0) + none)
    p.add_export(p.add_import_full('Class', willow, 'WillowDoomedPawn'), 'Pawn_Doomed',
                 w32(0) + t.obj('AIClass', ai_class) + t.float('HealthVar', 11.0) + t.float('HealthMaxVar', 12.0) + none)
    p.add_export(p.add_import_full('Class', willow, 'WillowDownPawn'), 'Pawn_Down',
                 w32(0) + t.obj('AIClass', ai_class) + t.float('HealthVar', 11.0) + t.float('HealthMaxVar', 12.0) + none)
    # the melee swing's data: a damage type definition, a melee definition, a thrown rock and its Behavior_CauseDamage, the ProjectileDamage attribute under a Projectile group
    core = p.add_import_full('Package', 0, 'Core')
    package_class = p.add_import_full('Class', core, 'Package')
    type_definition = p.add_export(p.add_import_full('Class', willow, 'DamageTypeDefinition'), 'DamageType_Toy', w32(0) + none)
    group = p.add_export(package_class, 'Projectile', w32(0) + none)
    attribute = p.add_export(resource_class, 'ProjectileDamage', w32(0) + none, outer=group)

    def data(name, constant, scale):
        return t.struct_(name, 'AttributeInitializationData', t.float('BaseValueConstant', constant) + t.float('BaseValueScaleConstant', scale) + none)
    p.add_export(p.add_import_full('Class', willow, 'MeleeDefinition'), 'Melee_Toy',
                 w32(0) + data('Damage', 40.0, 1.5) + t.tag('DamageSource', 'ClassProperty', 4, w32(p.add_import_full('Class', willow, 'WillowDmgSource_Melee')))
                 + t.obj('DamageTypeDefinition', type_definition) + none)
    p.add_export(p.add_import_full('Class', willow, 'Behavior_AIThrowProjectileAtTarget'), 'Throw_Toy',
                 w32(0) + t.array('ChildProjectileBaseValues', 1, t.obj('Attribute', attribute) + data('BaseValue', 10.0, 1.5) + none) + none)
    p.add_export(p.add_import_full('Class', willow, 'Behavior_CauseDamage'), 'Cause_Toy',
                 w32(0) + data('DamageFormula', 0.0, 2.0) + t.tag('DamageSource', 'ClassProperty', 4, w32(p.add_import_full('Class', willow, 'WillowDmgSource_Grenade')))
                 + t.obj('DamageTypeDefinition', type_definition) + none)
    return p


def build_startup():
    p = Package()
    t = Tags(p)
    none = t.none()
    core = p.add_import_full('Package', 0, 'Core')
    willow = p.add_import_full('Package', 0, 'WillowGame')
    package_class = p.add_import_full('Class', core, 'Package')
    globals_class = p.add_import_full('Class', willow, 'GlobalsDefinition')
    group = p.add_export(package_class, 'GD_Globals', w32(0) + none)
    general = p.add_export(package_class, 'General', w32(0) + none, outer=group)
    formula = t.struct_('BaseEnemyExperienceFormula', 'AttributeInitializationData', t.float('BaseValueConstant', 20.0) + t.float('BaseValueScaleConstant', 1.5) + none)

    def weight(players, bonus, share): return t.int('Players', players) + t.float('KillerExpBonus', bonus) + t.float('ExpWeight', share) + none

    def scale(difference, higher, lower): return t.int('LevelDifference', difference) + t.float('HigherLevelEnemyExpScale', higher) + t.float('LowerLevelEnemyExpScale', lower) + none
    p.add_export(globals_class, 'Globals', w32(0) + formula + t.array('ExpAwardWeights', 2, weight(1, 0.0, 1.0) + weight(2, 0.25, 0.5))
                 + t.array('ExpScaleByLevelDifference', 2, scale(1, 1.5, 0.8) + scale(2, 2.0, 0.5)) + none, outer=general)
    return p


failures = []


def check(label, condition, detail=''):
    if not condition: failures.append(f'{label}: {detail}')


def combat_run(root, *steps):
    proc = subprocess.run([reader, str(root / 'ToyPawns.upk'), '--combat-run', '--cooked', str(root), *steps], capture_output=True, text=True, encoding='utf-8')
    assert proc.stdout.strip(), ('no JSON output', proc.returncode, proc.stderr)
    return proc.returncode, json.loads(proc.stdout)


def near(a, b): return abs(a - b) < 1e-4


with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    (root / 'Engine.upk').write_bytes(build_engine().build())
    (root / 'GearboxFramework.upk').write_bytes(build_gearbox().build())
    (root / 'WillowGame.upk').write_bytes(build_willowgame().build())
    (root / 'ToyPawns.upk').write_bytes(build_pawns().build())
    (root / 'Startup.upk').write_bytes(build_startup().build())

    code, got = combat_run(root, 'spawn:ToyPawns/Pawn_Toy:4',
                           'sethealth:0:0.5', 'sethealth:0:-3', 'sethealth:0:1.5', 'sethealth:0:500', 'setmax:0:99',
                           'pool:0:Pool_Health_Copy', 'pool:0:Pool_Ammo', 'pool:0:Pool_Ammo_Copy', 'copyref:0:1', 'valid:0:3',
                           'unpool:0:1', 'valid:0:3', 'pool:0:Pool_Ammo', 'valid:0:3', 'valid:0:4',
                           'destroyhealth:0', 'sethealth:0:7', 'pool:0:Pool_Health')
    check('exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    steps = {s['step']: s['result'] for s in got['steps']}
    rows = got['steps']

    def state(n): return rows[n]['result']['state']

    # step indices: 0 spawn, 1-4 sethealth, 5 setmax, 6 pool(copy) 7 pool(ammo) 8 pool(ammo copy) 9 copyref 10 valid 11 unpool 12 valid 13 pool 14 valid 15 valid
    #               16 destroyhealth 17 sethealth 18 pool(health)
    spawn = got['pawns'][0] if got['pawns'] else {}
    check('spawn: GetHealth and GetMaxHealth answer the pool', near(got['rows'].get('health.Toy.L4', -1), 40.5) and near(got['rows'].get('max_health.Toy.L4', -1), 40.5), got['rows'])
    # the first state report is after sethealth 0.5: the rows above are the spawn values; the other fields come from the first step that reports state
    check('spawn: the pool is in slot 0 with GUID 0 and the mind as its provider',
          state(1)['pool_valid'] and state(1)['pool_slot'] == 0 and state(1)['pool_guid'] == 0 and len(state(1)['slots']) == 1
          and state(1)['slots'][0]['slot'] == 0 and state(1)['slots'][0]['provider_is_mind'], state(1))
    check('spawn: HealthVar is not written while the pool is valid, HealthMaxVar follows the pool', state(1)['health_var'] == 11 and near(state(1)['health_max_var'], 40.5), state(1))
    check('SetHealth(0.5) is exactly 0', state(1)['health'] == 0 and state(1)['health_var'] == 11, state(1))
    check('SetHealth(-3) is passed on as -3', near(state(2)['health'], -3), state(2))
    check('SetHealth(1.5) stays 1.5', near(state(3)['health'], 1.5), state(3))
    check('SetHealth(500) clamps to the pool maximum', near(state(4)['health'], 40.5), state(4))
    check('SetMaxHealth writes HealthMaxVar only', near(state(5)['health_max_var'], 99) and near(state(5)['max_health'], 40.5), state(5))

    # one pool per resource
    check('a second definition of the same resource is refused', rows[6]['result'].get('valid') is False and rows[6]['result'].get('slot') == 255
          and rows[6]['result'].get('guid') == 255 and len(state(6)['slots']) == 1, rows[6])
    check('another resource takes slot 1 with GUID 1', rows[7]['result'].get('valid') is True and rows[7]['result'].get('slot') == 1 and rows[7]['result'].get('guid') == 1
          and len(state(7)['slots']) == 2, rows[7])
    check('a second definition of that resource is refused too', rows[8]['result'].get('valid') is False and len(state(8)['slots']) == 2, rows[8])

    # a reference is valid only while its slot holds a pool of the same GUID
    check('the copy is valid while its pool lives', rows[10]['result'].get('valid') is True, rows[10])
    check('DeletePoolByReference empties the slot', len(state(11)['slots']) == 1 and state(11)['slots'][0]['slot'] == 0, rows[11])
    check('the copy is stale when its slot is empty', rows[12]['result'].get('valid') is False, rows[12])
    check('the next pool takes the lowest free slot with the next GUID', rows[13]['result'].get('slot') == 1 and rows[13]['result'].get('guid') == 2
          and rows[13]['result'].get('valid') is True and len(state(13)['slots']) == 2, rows[13])
    check('the copy stays stale although the slot is used again', rows[14]['result'].get('valid') is False, rows[14])
    check('the new reference is valid', rows[15]['result'].get('valid') is True, rows[15])

    # destroying the health pool
    after = state(16)
    check('destroying the health pool invalidates HealthPool (index 255, GUID 255)', after['pool_valid'] is False and after['pool_slot'] == 255 and after['pool_guid'] == 255, after)
    check('GetHealth and GetMaxHealth fall back to the plain variables', after['health'] == 11 and near(after['max_health'], 99), after)
    check('the health pool left its slot', [s['slot'] for s in after['slots']] == [1], after)
    check('SetHealth writes HealthVar without a pool', state(17)['health_var'] == 7 and state(17)['health'] == 7, state(17))
    check('the next pool takes the freed slot 0', rows[18]['result'].get('slot') == 0 and rows[18]['result'].get('guid') == 3 and [s['slot'] for s in state(18)['slots']] == [0, 1], rows[18])

    # ---- the hit chain: demigod holds, the floor, the order of the events
    code, got = combat_run(root, 'spawn:ToyPawns/Pawn_Toy:4', 'player:8',
                           'hit:0:10::Bullet',
                           'sethealth:0:40.5', 'hold:0:Safe', 'hit:0:10::Bullet',
                           'sethealth:0:40.5', 'hold:0:Other', 'release:0:Safe', 'hit:0:10::Bullet',
                           'release:0:Other', 'sethealth:0:40.5', 'hit:0:10::Bullet')
    check('hit chain: exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    rows = got['steps']
    mortal, floor, one_hold, released = (rows[i]['result'] for i in (2, 5, 9, 12))

    def position(result, prefix):
        return next((i for i, e in enumerate(result['events']) if e.startswith(prefix)), -1)

    check('a pawn with no hold is not demigod and the mortal toy chain drops it to 0', mortal['demigod_before'] is False and mortal['demigod_after'] is False
          and near(mortal['health_before'], 40.5) and mortal['health_after'] == 0, mortal)
    check('state: no demigod before a hold', rows[3]['result']['state']['demigod'] is False, rows[3])
    check('state: a hold makes the pawn demigod (the installed script asks the AI component)', rows[4]['result']['state']['demigod'] is True, rows[4])
    check('demigod: the chain sets health to the floor 1.0 and SetHealth(1.0) leaves exactly 1.0', floor['demigod_before'] is True and floor['demigod_after'] is True
          and floor['health_after'] == 1.0, floor)
    check('two holds, one released: still demigod (a hold per reason)', rows[8]['result']['state']['demigod'] is True and one_hold['health_after'] == 1.0, (rows[8], one_hold))
    check('every hold released: no longer demigod', rows[10]['result']['state']['demigod'] is False and released['demigod_before'] is False and released['health_after'] == 0,
          (rows[10], released))
    check('the rows of the hit: arriving damage and health after', near(got['rows'].get('hit.Toy.L4.2.arriving', -1), 10) and got['rows'].get('hit.Toy.L4.2.health_after') == 1.0, got['rows'])
    order = [position(floor, 'TakeDamage'), position(floor, 'SetHealth'), position(floor, 'NotifyTakeHit'), position(floor, 'OnTakeDamage raised')]
    check('event order: health is set before NotifyTakeHit, which raises OnTakeDamage', all(i >= 0 for i in order) and order == sorted(order), floor['events'])
    check('a class with no behavior provider has nothing to raise the event to', any('no behavior provider' in e for e in floor['events']), floor['events'])

    # ---- the death chain: enableDeaths gives the world a game and a tracker, the scripts run in order, NotifyPawnDied is a bound native, the post-state
    code, got = combat_run(root, 'spawn:ToyPawns/Pawn_Doomed:4', 'player:8', 'hit:0:10::Bullet', 'destroyhealth:0')   # (the toy game's Killed also awards experience: logged, not asserted here)
    check('death: exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    rows = got['steps']
    kill = rows[2]['result']
    expected_order = ['WillowGame.WillowDoomedPawn.TakeDamage', 'Engine.Pawn.SetHealth', 'Engine.Pawn.NotifyTakeHit', 'WillowGame.WillowDoomedPawn.HandleHealthDepleted',
                      'WillowGame.MissionTracker.NotifyPawnDied', 'WillowGame.WillowDoomedPawn.Died', 'Engine.Pawn.SetHealth', 'WillowGame.WillowGameInfo.Killed']
    check('death: the call order of the chain (TakeDamage, SetHealth, NotifyTakeHit, HandleHealthDepleted, NotifyPawnDied, Died, SetHealth, Killed)',
          kill.get('order', [])[:len(expected_order)] == expected_order and kill.get('order', [])[len(expected_order):][:2] == ['WillowGame.WillowExperiencePipeline.CalculateExperiencePointsForKill',
          'WillowGame.WillowExperiencePipeline.AwardCombatExperienceToParty'], kill.get('order'))
    check('death: the tracker native ran (it is bound, not a stub)', any(e.startswith('NotifyPawnDied') for e in kill['events']) and 'MissionTracker.NotifyPawnDied(object)' not in got['stubs'], (kill['events'], got['stubs']))
    check('death: the game script ran (WorldInfo.Game is not None): no call on None', 'WillowGame.WillowGameInfo.Killed' in kill.get('order', []), kill.get('order'))
    check('death: SetHealth(0) came after the lethal -5 (the events list the writes in order)', [e for e in kill['events'] if e.startswith('SetHealth')] == ['SetHealth -5.000000', 'SetHealth 0.000000'], kill['events'])
    check('death: health is 0 after the chain', kill['health_after'] == 0, kill)
    check('death: the pool is gone after destroyhealth', rows[3]['result']['state']['pool_valid'] is False and rows[3]['result']['state']['pool_slot'] == 255, rows[3])

    # ---- kill experience: the base formula and the two tables from the data, the split by player count, the level scale, truncation, the ExpEarn call
    def kill(*steps):
        code, got = combat_run(root, 'spawn:ToyPawns/Pawn_Doomed:5', *steps)
        check('xp: exit and errors', code == 0 and got['errors'] == [], (steps, code, got['errors']))
        hit = next(r['result'] for r in got['steps'] if r['step'].startswith('hit'))
        credits = [e for e in hit['events'] if e.startswith('AwardCombatExperienceToParty')]
        return got, hit, credits
    got, hit, credits = kill('player:4', 'players:1', 'hit:0:10::Bullet')
    check('xp: one player below the victim by 1 gets 30 x 1.5 = 45', credits == ['AwardCombatExperienceToParty player_level=4 victim_level=5 ExpEarn(45, 1, 1)'], credits)
    check('xp: the award of the kill is the base formula of the data (20 x 1.5)', near(got['rows'].get('kill.Doomed.L5.p4', {}).get('award', -1), 30.0), got['rows'])
    check('xp: no stub for the experience natives (they are bound) and ExpEarn is the toy script', not any('Experience' in k or 'ExpEarn' in k for k in got['stubs']), got['stubs'])
    got, hit, credits = kill('player:4', 'player:7', 'players:2', 'hit:0:10::Bullet')
    check('xp: two players: the killer (0.25 + 0.5) x 30 x 1.5 = 33.75 -> 33, the partner 0.5 x 30 x 0.5 = 7.5 -> 7, in list order',
          credits == ['AwardCombatExperienceToParty player_level=4 victim_level=5 ExpEarn(33, 1, 1)', 'AwardCombatExperienceToParty player_level=7 victim_level=5 ExpEarn(7, 1, 1)'], credits)
    got, hit, credits = kill('player:1', 'players:1', 'hit:0:10::Bullet')
    check('xp: a difference past the table uses its last row: 30 x 2.0 = 60', credits == ['AwardCombatExperienceToParty player_level=1 victim_level=5 ExpEarn(60, 1, 1)'], credits)
    got, hit, credits = kill('player:5', 'players:1', 'hit:0:10::Bullet')
    check('xp: equal levels leave the share unscaled: 30', credits == ['AwardCombatExperienceToParty player_level=5 victim_level=5 ExpEarn(30, 1, 1)'], credits)
    got, hit, credits = kill('player:4', 'hit:0:10::Bullet')
    check('xp: a game with no player count (0) is outside the table: bonus 0, weight 1', credits == ['AwardCombatExperienceToParty player_level=4 victim_level=5 ExpEarn(45, 1, 1)'], credits)

    # ---- the melee swing: the damage from the data (the attacker's mind as the source), the definition's source and type, delivered to the player with no hit volume
    code, got = combat_run(root, 'spawn:ToyPawns/Pawn_Toy:4', 'player:8', 'swing:0:Melee_Toy', 'swing:0:Throw_Toy:Cause_Toy', 'swing:0:Throw_Toy', 'swing:0:Pawn_Toy', 'swing:0:NoSuchObject')
    rows = got['steps']
    swing, rock = rows[2]['result'], rows[3]['result']
    check('swing: the toy player pawn takes 40 x 1.5 = 60 from the definition (health, arrival and the hit report agree)', near(swing.get('arrival', -1), 60) and near(swing['hit']['arriving'], 60)
          and near(swing['hit']['health_after'], 60) and near(got['rows'].get('arrival.Toy.L4', -1), 60), (swing, got['rows']))
    check('swing: the damage source class and the damage type come from the definition', swing.get('source') == 'WillowDmgSource_Melee' and swing.get('damage_type', '').endswith('DamageType_Toy'), swing)
    check("swing: the event list says the fixture always connects and the chain ran with the definition's source", 'always connects' in swing['hit']['events'][0]
          and any('source=WillowDmgSource_Melee' in e for e in swing['hit']['events']), swing['hit']['events'])
    check('swing: a thrown rock is the ProjectileDamage entry (10 x 1.5) scaled by the cause-damage formula (2.0) = 30, source from the Behavior_CauseDamage',
          near(rock.get('arrival', -1), 30) and rock.get('source') == 'WillowDmgSource_Grenade' and near(rock['hit']['health_after'], 30) and near(got['rows'].get('arrival.Toy.L4#2', -1), 30), (rock, got['rows']))
    check('swing: a throw without its Behavior_CauseDamage, an object that delivers nothing and a missing object are refused with a reason',
          len(got['errors']) == 3 and 'needs the projectile' in got['errors'][0] and 'neither a MeleeDefinition' in got['errors'][1] and 'has no NoSuchObject' in got['errors'][2] and code != 0, (code, got['errors']))
    code, got = combat_run(root, 'spawn:ToyPawns/Pawn_Toy:4', 'swing:0:Melee_Toy')
    check('swing: with no player to hit the step is refused', code != 0 and 'needs a player' in got['errors'][0], got['errors'])

    # ---- the down state's natives: bound (not stubs), their effects visible to the script that calls them
    code, got = combat_run(root, 'spawn:ToyPawns/Pawn_Down:4', 'player:8', 'hit:0:10::Bullet')
    down = got['steps'][2]['result']
    check('down: exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    check('down: SetSecondWindReason and SetCanTarget log what they were given', 'SetSecondWindReason 3' in down['events'] and 'SetCanTarget false' in down['events'], down['events'])
    check('down: IsInjured answered the flag the script set and CanTarget the false SetCanTarget stored (health 7)', near(down['health_after'], 7.0), down)
    check('down: none of the down natives is a stub', not [k for k in got['stubs'] if k.split('(')[0] in ('Pawn.IsInjured', 'Actor.SetCanTarget', 'Actor.CanTarget', 'WillowPawn.SetSecondWindReason', 'WillowPawn.ClearSecondWindReason')], got['stubs'])
    code, got = combat_run(root, 'spawn:ToyPawns/Pawn_Down:4', 'bleed:1')
    check('down: bleed without the mission bridge is refused with a reason', code != 0 and 'needs the mission bridge' in got['errors'][0], got['errors'])
    code, got = combat_run(root, 'spawn:ToyPawns/Pawn_Down:4', 'cash:100')
    check('down: cash without the mission bridge is refused with a reason', code != 0 and 'needs the mission bridge' in got['errors'][0], got['errors'])

if failures:
    print(f'{len(failures)} combat check(s) failed:')
    for failure in failures: print('  - ' + failure)
    sys.exit(1)
print('combat synthetic coverage passed.')
