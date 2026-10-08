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

    def array_of(self, owner, name, inner_kind):
        index = len(self.p.exports) + 1
        self.prop('Array', owner, name, type_ref=index + 1)
        self.prop(inner_kind, index, name)
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
    T = Toy(('Byte', 'Object', 'Int', 'Float', 'Bool', 'Class', 'Struct'))
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
    ai_pawn = T.cls('WillowAIPawn', super_ref=pawn)
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
    killed_function = T.function(game_info, 'Killed', [('Object', 'KilledPawn', 0)], lambda ids: (lambda a: (a.return_nothing(), a.end(), a)[2])(Asm()))['__self__']
    tracker = T.cls('MissionTracker', super_ref=actor)
    notify_died = T.function(tracker, 'NotifyPawnDied', [('Object', 'KilledPawn', 0)], None, NATIVE)['__self__']
    replication = T.cls('WillowGameReplicationInfo', super_ref=actor)
    tracker_property = T.prop('Object', replication, 'MissionTracker')
    doomed = T.cls('WillowDoomedPawn', super_ref=ai_pawn)

    def died(ids):                    # SetHealth(0.0); WorldInfo.Game.Killed(Self)
        a = Asm()
        a.final(set_health, lambda: a.float_const(0.0))
        a.context(lambda: a.context(lambda: a.instance(world_property), lambda: a.instance(game_property)), lambda: a.final(killed_function, lambda: a.raw(0x17)))
        a.return_nothing(); a.end(); return a
    died_function = T.function(doomed, 'Died', [], died)['__self__']

    def depleted(ids):                # WorldInfo.GRI.MissionTracker.NotifyPawnDied(Self); Died()
        a = Asm()
        a.context(lambda: a.context(lambda: a.instance(world_property), lambda: a.instance(gri_property)),
                  lambda: a.context(lambda: a.instance(tracker_property), lambda: a.final(notify_died, lambda: a.raw(0x17))))
        a.final(died_function)
        a.return_nothing(); a.end(); return a
    depleted_function = T.function(doomed, 'HandleHealthDepleted', [], depleted)['__self__']

    def doomed_take_damage(ids):      # SetHealth(-5.0); NotifyTakeHit(...); HandleHealthDepleted()
        a = Asm()
        a.final(set_health, lambda: a.float_const(-5.0))
        a.final(notify, *[(lambda n=n: a.local(ids, n)) for n in ('InstigatedBy', 'HitLocation', 'Damage', 'DamageType', 'Momentum', 'Pipeline')])
        a.final(depleted_function)
        a.return_nothing(); a.end(); return a
    T.function(doomed, 'TakeDamage', [('Float', 'Damage', 0), ('Object', 'InstigatedBy', 0), ('Float', 'HitLocation', 0), ('Float', 'Momentum', 0),
                                      ('Class', 'DamageType', 0), ('Float', 'HitInfo', 0), ('Object', 'DamageCauser', 0), ('Object', 'Pipeline', 0)], doomed_take_damage)

    # the classes the hit step builds around the chain
    pipeline = T.cls('WillowDamagePipeline')
    for name in ('DamageTypeDef', 'ImpactDefinition', 'DirectHitObject'): T.prop('Object', pipeline, name)
    T.prop('Bool', pipeline, 'bCanHitFriendly')
    T.cls('WillowDmgSource_Bullet')
    player_controller = T.cls('WillowPlayerController', super_ref=controller)
    T.prop('Object', player_controller, 'PlayerReplicationInfo')
    T.cls('WillowPlayerPawn', super_ref=pawn)
    T.prop('Int', T.cls('WillowPlayerReplicationInfo'), 'ExpLevel')
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
    code, got = combat_run(root, 'spawn:ToyPawns/Pawn_Doomed:4', 'player:8', 'hit:0:10::Bullet', 'destroyhealth:0')
    check('death: exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    rows = got['steps']
    kill = rows[2]['result']
    expected_order = ['WillowGame.WillowDoomedPawn.TakeDamage', 'Engine.Pawn.SetHealth', 'Engine.Pawn.NotifyTakeHit', 'WillowGame.WillowDoomedPawn.HandleHealthDepleted',
                      'WillowGame.MissionTracker.NotifyPawnDied', 'WillowGame.WillowDoomedPawn.Died', 'Engine.Pawn.SetHealth', 'WillowGame.WillowGameInfo.Killed']
    check('death: the call order of the chain (TakeDamage, SetHealth, NotifyTakeHit, HandleHealthDepleted, NotifyPawnDied, Died, SetHealth, Killed)', kill.get('order') == expected_order, kill.get('order'))
    check('death: the tracker native ran (it is bound, not a stub)', any(e.startswith('NotifyPawnDied') for e in kill['events']) and 'MissionTracker.NotifyPawnDied(object)' not in got['stubs'], (kill['events'], got['stubs']))
    check('death: the game script ran (WorldInfo.Game is not None): no call on None', 'WillowGame.WillowGameInfo.Killed' in kill.get('order', []), kill.get('order'))
    check('death: SetHealth(0) came after the lethal -5 (the events list the writes in order)', [e for e in kill['events'] if e.startswith('SetHealth')] == ['SetHealth -5.000000', 'SetHealth 0.000000'], kill['events'])
    check('death: health is 0 after the chain', kill['health_after'] == 0, kill)
    check('death: the pool is gone after destroyhealth', rows[3]['result']['state']['pool_valid'] is False and rows[3]['result']['state']['pool_slot'] == 255, rows[3])

if failures:
    print(f'{len(failures)} combat check(s) failed:')
    for failure in failures: print('  - ' + failure)
    sys.exit(1)
print('combat synthetic coverage passed.')
