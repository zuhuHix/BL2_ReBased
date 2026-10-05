"""Synthetic mission-script bridge coverage (src/mission_script.*, docs/verification/NATIVE_MISSION_SCRIPT_BRIDGE.md).

Builds toy `Engine`, `WillowGame` and `TestMission` packages with invented classes (the names are the vocabulary the bridge
reads: WillowPlayerController, MissionTracker, WillowGameReplicationInfo, MissionDefinition, WorldInfo.GRI, Actor.Role)
and hand-assembled bytecode, then runs `ow-package --mission-run ... script:accept script:turnin xp:<n>` and checks results
computed by hand. Nothing here comes from the game; the toy controller script is not the game's, it only calls what the
bridge binds and leaves a trace through the recorded ExpEarn calls (amount = what the script passed):

  UpdateMissionStatus(Mission, Status)  ->  ExpEarn(Status, 7)
  IsMissionMoviePlaying()               ->  ExpEarn(100, 8)       (called by the tracker tick consuming the kickoff)
  AcceptMission(Mission, Director)      ->  WorldInfo.GRI.MissionTracker.ActivateMission(Mission, Self)
  ServerCompleteMission(Mission, Dir.)  ->  ...CompleteMission(Mission, Self); ...PlayTurnIn(Mission);
                                            ExpEarn(Mission.GetExperienceReward(Self, false), 5)

So the recorded amounts spell the order of events: the status hook for every status (Active 1, ReadyToTurnIn 3, Complete 4)
in order, the kickoff tick, and the reward call with the amount the host supplied (the xp: step).
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

    def function(self, owner, name, declared, body=None, flags=FUNC_FINAL | FUNC_DEFINED | FUNC_PUBLIC, result=None):
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
        payload += u16(0) + bytes([0]) + u32(flags) + self.p.fname(name)
        assert self.p.add_export(self.imp['Function'], name, payload, outer=owner) == func_index
        ids['__self__'] = func_index
        return ids


def build_engine():
    toy = Toy(('Byte', 'Object'))
    actor = toy.cls('Actor')
    toy.prop('Byte', actor, 'Role')
    toy.prop('Object', actor, 'WorldInfo')
    toy.p.add_export(toy.imp['Enum'], 'ENetRole', w32(0) + toy.none + w32(0) + w32(len(ROLES)) + b''.join(toy.p.fname(r) for r in ROLES),
                     outer=actor)
    toy.prop('Object', toy.cls('WorldInfo'), 'GRI')
    return toy.p


def build_willowgame():
    toy = Toy(('Int', 'Bool', 'Byte', 'Object', 'Array', 'Struct'))
    p, T = toy.p, toy
    engine = p.add_import_full('Package', 0, 'Engine')
    actor = p.add_import_full('Class', engine, 'Actor')
    world_info, gri = (p.add_import_full('ObjectProperty', actor, 'WorldInfo'),
                       p.add_import_full('ObjectProperty', p.add_import_full('Class', engine, 'WorldInfo'), 'GRI'))
    native = FUNC_NATIVE | FUNC_PUBLIC

    mission = T.cls('MissionDefinition')
    T.array_of(mission, 'ObjectiveSetDefs', 'Object')
    T.prop('Object', mission, 'InitialObjectiveSet')
    T.prop('Object', mission, 'BehaviorProvider')
    T.prop('Bool', mission, 'bActivateInitialObjectiveSet')
    get_xp = T.function(mission, 'GetExperienceReward', [('Object', 'InWPC', 0), ('Bool', 'bGetAltReward', 0)], None, native, 'Int')['__self__']
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
    controller = T.cls('WillowPlayerController', super_ref=actor)
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
        a.call(exp_earn, lambda: a.local(ids, 'NewMissionStatus'), lambda: a.byte_const(7))
        a.return_nothing(); a.end(); return a
    T.function(controller, 'UpdateMissionStatus', [('Object', 'Mission', 0), ('Byte', 'NewMissionStatus', 0)], update)

    def movie(ids):
        a = Asm()
        a.call(exp_earn, lambda: a.int_const(100), lambda: a.byte_const(8))
        a.raw(0x04); a.false(); a.end(); return a
    T.function(controller, 'IsMissionMoviePlaying', [], movie, result='Bool')
    return p


def build_mission():
    """ToyMission: one set SetA {Only (count 1)} with bCanCompleteMission, activated at acceptance."""
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
    cls_, sup, outer, name, _ = p.exports[mission - 1]
    p.exports[mission - 1] = (cls_, sup, outer, name, w32(0) + t.array('ObjectiveSetDefs', 1, w32(set_a))
                              + t.obj('InitialObjectiveSet', set_a) + t.bool('bActivateInitialObjectiveSet', True) + none)
    return p


failures = []


def check(label, condition, detail=''):
    if not condition: failures.append(f'{label}: {detail}')


def mission_run(root, *steps):
    proc = subprocess.run([reader, str(root / 'TestMission.upk'), '--mission-run', 'ToyMission', '--cooked', str(root), *steps],
                          capture_output=True, text=True, encoding='utf-8')
    assert proc.stdout.strip(), ('no JSON output', proc.returncode, proc.stderr)
    return proc.returncode, json.loads(proc.stdout)


with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    (root / 'Engine.upk').write_bytes(build_engine().build())
    (root / 'WillowGame.upk').write_bytes(build_willowgame().build())
    (root / 'TestMission.upk').write_bytes(build_mission().build())

    code, got = mission_run(root, 'script:accept', 'script:accept', 'tick:0.5', 'tick:0.5', 'obj:Only', 'xp:395', 'script:turnin')
    check('exit and errors', code == 0 and got['errors'] == [], (code, got['errors']))
    steps = got['steps']
    check('steps reported', len(steps) == 7, len(steps))
    if len(steps) == 7:
        def kinds(step): return [e['kind'] for e in steps[step]['effects']]
        # AcceptMission -> ActivateMission: Active, the initial set; a pending kickoff record is written (not played yet)
        check('accept', steps[0]['ok'] and steps[0]['status'] == 1 and steps[0]['kickoff_pending']
              and kinds(0) == ['status', 'objective_set_active'], steps[0])
        # a second accept is refused by the native itself (status is not NotStarted): no new status, no hook
        check('second accept refused', not steps[1]['ok'] and steps[1]['status'] == 1 and kinds(1) == [], steps[1])
        # the tracker tick consumes the record once
        check('tick consumes kickoff', not steps[2]['kickoff_pending'] and not steps[3]['kickoff_pending'], (steps[2], steps[3]))
        check('objective makes the mission ready', steps[4]['status'] == 2, steps[4])
        # ServerCompleteMission -> CompleteMission (Complete), PlayTurnIn, then the script's own reward call
        check('turn-in', steps[6]['ok'] and steps[6]['status'] == 3 and kinds(6) == ['status', 'reward'], steps[6])
    script = got.get('script')
    check('script report present', script is not None, got)
    if script:
        earned = [(e['amount'], e['source'], e['type']) for e in script['exp_earned']]
        # hook Active (1), kickoff tick (IsMissionMoviePlaying, once), hook ReadyToTurnIn (3), hook Complete (4), reward amount
        want = [(1, 7, -1), (100, 8, -1), (3, 7, -1), (4, 7, -1), (395, 5, -1)]
        check('recorded ExpEarn calls in order', earned == want, earned)
        # every native the toy script reaches is bound: ActivateMission, CompleteMission, PlayTurnIn, ExpEarn, GetExperienceReward
        check('no stubs', script['stubs'] == [], script['stubs'])
        check('no VM notes', script['notes'] == [], script['notes'])

    # without the stand-in amount GetExperienceReward returns 0 and says so in the stub list (the script still calls ExpEarn here)
    code, got = mission_run(root, 'script:accept', 'obj:Only', 'script:turnin')
    check('no-amount exit', code == 0, (code, got['errors']))
    stubs = got['script']['stubs']
    check('no-amount stub listed', len(stubs) == 1 and stubs[0].startswith('MissionDefinition.GetExperienceReward'), stubs)

if failures:
    print(f'{len(failures)} mission script check(s) failed:')
    for failure in failures: print('  - ' + failure)
    sys.exit(1)
print('mission script synthetic coverage passed.')
