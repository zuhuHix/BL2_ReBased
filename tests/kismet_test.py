"""Synthetic Kismet executor coverage (Phase 2/4): builds toy `Engine`, `WillowGame`, `Missions` and `Other`
packages plus a toy sequence package with hand-assembled tagged property data, runs them through
`ow-package --kismet-run` / `--kismet-census`, and checks results computed by hand.

Nothing here comes from the game: every class, property, struct, op and event name is either the public UE3
vocabulary the executor reads by name (SequenceOp, SeqOpOutputLink, ...) or an invented toy name.

Key regression pinned here: arrays inside structs (SeqOpOutputLink.Links) must decode through the declaring
struct's reflection even when the sequence package does not import that struct by name. If they decode as empty,
nothing propagates and every trace below collapses to the event line.
"""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

reader = str(Path(sys.argv[1]).resolve())


def w32(*values): return b''.join(struct.pack('<i', v) for v in values)
def u32(v): return struct.pack('<I', v)
def u64(v): return struct.pack('<Q', v)


class Package:
    """Same toy package writer as tests/vm_test.py, plus imports with an explicit class package and outer."""
    def __init__(self):
        self.names = []
        self.imports = []
        self.exports = []

    def name(self, text):
        if text not in self.names: self.names.append(text)
        return self.names.index(text)

    def fname(self, text, number=0): return w32(self.name(text), number)

    def add_import(self, class_name, name):
        self.imports.append((self.fname('Core'), self.fname(class_name), 0, self.fname(name)))
        return -len(self.imports)

    def add_import_full(self, class_name, outer, name):
        self.imports.append((self.fname('Core'), self.fname(class_name), outer, self.fname(name)))
        return -len(self.imports)

    def add_export(self, cls, name, payload, outer=0, super_ref=0):
        self.exports.append((cls, super_ref, outer, self.fname(name), payload))
        return len(self.exports)

    def build(self):
        self.name('None')
        table = b''.join(w32(len(n) + 1) + n.encode() + b'\0' + bytes(8) for n in self.names)
        io = 48 + len(table)
        import_data = b''.join(cp + cn + w32(outer) + nm for cp, cn, outer, nm in self.imports)
        eo = io + len(import_data)
        entry_size = 68
        payload_at = eo + entry_size * len(self.exports)
        export_data = b''
        payloads = b''
        for cls, sup, outer, name, payload in self.exports:
            export_data += w32(cls, sup, outer) + name + w32(0) + bytes(8) + w32(len(payload), payload_at + len(payloads)) \
                + w32(0) + w32(0) + bytes(20)
            payloads += payload
        header = w32(0x9e2a83c1 - (1 << 32), 832 | (46 << 16), 48, 0, 0, len(self.names), 48, len(self.exports), eo,
                     len(self.imports), io, 0)
        return header + table + import_data + export_data + payloads


# ---------------------------------------------------------------------------------------------- Engine.upk
def build_engine():
    """Toy declarations carrying the real property NAMES the executor reads, with toy layouts."""
    p = Package()
    imp = {n: p.add_import('Class', n) for n in
           ('Class', 'ScriptStruct', 'IntProperty', 'FloatProperty', 'BoolProperty', 'StrProperty', 'NameProperty',
            'ObjectProperty', 'ArrayProperty', 'StructProperty')}
    none = p.fname('None')

    def prop(kind, owner, name, type_ref=0):
        # 40-byte property header, then the type reference (struct / array inner) at +40.
        payload = w32(0) + none + w32(0) + u32(1) + u64(0) + none + w32(0) + w32(type_ref)
        return p.add_export(imp[kind + 'Property'], name, payload, outer=owner)

    def array_of(owner, name, inner_kind, inner_ref=0):
        index = len(p.exports) + 1
        prop('Array', owner, name, index + 1)          # the inner declaration is exported right after it
        prop(inner_kind, index, name, inner_ref)
        return index

    def struct_(name): return p.add_export(imp['ScriptStruct'], name, w32(0) * 4)
    def cls(name, super_ref=0): return p.add_export(imp['Class'], name, w32(0) * 4, super_ref=super_ref)

    s_in = struct_('SeqOpInputLink')
    prop('Str', s_in, 'LinkDesc')
    s_link = struct_('SeqOpOutputInputLink')
    prop('Object', s_link, 'LinkedOp')
    prop('Int', s_link, 'InputLinkIdx')
    s_out = struct_('SeqOpOutputLink')
    array_of(s_out, 'Links', 'Struct', s_link)
    prop('Str', s_out, 'LinkDesc')
    prop('Bool', s_out, 'bDisabled')
    prop('Float', s_out, 'ActivateDelay')
    s_var = struct_('SeqVarLink')
    prop('Str', s_var, 'LinkDesc')
    array_of(s_var, 'LinkedVariables', 'Object')

    op = cls('SequenceOp')
    array_of(op, 'InputLinks', 'Struct', s_in)
    array_of(op, 'OutputLinks', 'Struct', s_out)
    array_of(op, 'VariableLinks', 'Struct', s_var)
    prop('Int', op, 'ActivateCount')
    cls('Sequence', op)
    event = cls('SequenceEvent', op)
    prop('Int', event, 'TriggerCount')
    prop('Object', event, 'Originator')
    remote = cls('SeqEvent_RemoteEvent', event)
    prop('Name', remote, 'EventName')
    action = cls('SequenceAction', op)
    activate = cls('SeqAct_ActivateRemoteEvent', action)
    prop('Name', activate, 'EventName')
    gate = cls('SeqAct_Gate', action)
    prop('Bool', gate, 'bOpen')
    prop('Int', gate, 'AutoCloseCount')
    prop('Int', gate, 'CurrentCloseCount')
    set_bool = cls('SeqAct_SetBool', action)
    prop('Bool', set_bool, 'DefaultValue')
    compare = cls('SeqCond_CompareBool', op)
    prop('Bool', compare, 'bResult')
    delay = cls('SeqAct_Delay', action)
    prop('Float', delay, 'Duration')
    cls('SeqAct_ToyWorldAction', action)              # invented: an action no handler knows, so host_boundary gets it
    cls('SeqCond_ToyUnhandled', op)                   # invented: a non-action op no handler knows -> unsupported
    var = cls('SeqVar_Bool')
    prop('Int', var, 'bValue')
    return p


def build_willowgame():
    """Toy stand-in for the mission-aware remote event class (derives the plain remote event class)."""
    p = Package()
    imp_class = p.add_import('Class', 'Class')
    imp_obj = p.add_import('Class', 'ObjectProperty')
    engine = p.add_import_full('Package', 0, 'Engine')
    remote = p.add_import_full('Class', engine, 'SeqEvent_RemoteEvent')
    none = p.fname('None')
    mission = p.add_export(imp_class, 'WillowSeqEvent_MissionRemoteEvent', w32(0) * 4, super_ref=remote)
    p.add_export(imp_obj, 'AssociatedMissionDefinition', w32(0) + none + w32(0) + u32(1) + u64(0) + none + w32(0) + w32(0),
                 outer=mission)
    return p


def build_plain(group=None, leaves=()):
    """A package of trivially named objects; only their identity is ever used."""
    p = Package()
    imp = p.add_import('Class', 'Object')
    none = p.fname('None')
    outer = p.add_export(imp, group, w32(0) + none) if group else 0
    for leaf in leaves: p.add_export(imp, leaf, w32(0) + none, outer=outer)
    return p


# ------------------------------------------------------------------------------------------ sequence builder
def op(name, cls, inputs=(), outs=(), vars_=(), scalars=()):
    """One sequence op. outs: out(...) dicts; vars_: [(LinkDesc, [variable names])]; scalars: [(kind, name, value)]."""
    return dict(name=name, cls=cls, inputs=list(inputs), outs=list(outs), vars=list(vars_), scalars=list(scalars))


def out(desc, *to, disabled=False, delay=0.0):
    """One output link; each target is (op name | ('ext', path...) | None, input index)."""
    return dict(desc=desc, to=list(to), disabled=disabled, delay=delay)


def event(name, event_name, *to, desc='Out'):
    return op(name, 'SeqEvent_RemoteEvent', [], [out(desc, *to)], scalars=[('name', 'EventName', event_name)])


def gate(name, outs, open_=False, auto=0):
    scalars = [('bool', 'bOpen', open_)] + ([('int', 'AutoCloseCount', auto)] if auto else [])
    return op(name, 'SeqAct_Gate', ['In', 'Open', 'Close', 'Toggle'], outs, scalars=scalars)


def world(name): return op(name, 'SeqAct_ToyWorldAction', ['In'])


IN, OPEN, CLOSE, TOGGLE = 0, 1, 2, 3


def build_sequence(ops, variables=(), sequence_class=('Engine', 'Sequence')):
    p = Package()
    none = p.fname('None')
    chains = {}

    def chain(*path):
        """Import reference for Package.Group.Object, creating the outer chain once."""
        if path in chains: return chains[path]
        outer = chain(*path[:-1]) if len(path) > 1 else 0
        chains[path] = p.add_import_full('Package' if len(path) == 1 else 'Class', outer, path[-1])
        return chains[path]

    def tag(name, kind, size, body, detail=b''):
        return p.fname(name) + p.fname(kind) + w32(size, 0) + detail + body

    def t_int(name, v): return tag(name, 'IntProperty', 4, w32(v))
    def t_float(name, v): return tag(name, 'FloatProperty', 4, struct.pack('<f', v))
    def t_bool(name, v): return tag(name, 'BoolProperty', 0, bytes([1 if v else 0]))
    def t_name(name, v): return tag(name, 'NameProperty', 8, p.fname(v))
    def t_obj(name, ref): return tag(name, 'ObjectProperty', 4, w32(ref))
    def t_str(name, text):
        data = text.encode() + b'\0'
        return tag(name, 'StrProperty', 4 + len(data), w32(len(data)) + data)
    def t_array(name, count, elements): return tag(name, 'ArrayProperty', 4 + len(elements), w32(count) + elements)

    scalar = dict(int=t_int, float=t_float, bool=t_bool, name=t_name, obj=lambda name, target: t_obj(name, ref(target)))
    index = {'Seq': 1}
    for i, o in enumerate(ops): index[o['name']] = 2 + i
    for j, (name, _) in enumerate(variables): index[name] = 2 + len(ops) + j

    def ref(target):
        if target is None: return 0
        if isinstance(target, tuple): return chain(*target[1:])
        return index[target]

    def class_ref(cls):
        return chain(*(cls.split('.') if '.' in cls else ('Engine', cls)))

    p.add_export(class_ref('.'.join(sequence_class)), 'Seq', w32(0) + none)
    for o in ops:
        body = w32(0)                                                        # net-index prefix, as the executor expects
        for kind, name, value in o['scalars']: body += scalar[kind](name, value)
        if o['inputs']:
            body += t_array('InputLinks', len(o['inputs']), b''.join(t_str('LinkDesc', d) + none for d in o['inputs']))
        if o['outs']:
            elements = b''
            for link in o['outs']:
                targets = b''.join(t_obj('LinkedOp', ref(t)) + t_int('InputLinkIdx', i) + none for t, i in link['to'])
                elements += t_array('Links', len(link['to']), targets) + t_str('LinkDesc', link['desc'])
                if link['disabled']: elements += t_bool('bDisabled', True)
                if link['delay']: elements += t_float('ActivateDelay', link['delay'])
                elements += none
            body += t_array('OutputLinks', len(o['outs']), elements)
        if o['vars']:
            elements = b''
            for desc, names in o['vars']:
                elements += t_str('LinkDesc', desc) + t_array('LinkedVariables', len(names), b''.join(w32(ref(n)) for n in names)) + none
            body += t_array('VariableLinks', len(o['vars']), elements)
        cls = class_ref(o['cls'])
        assert p.add_export(cls, o['name'], body + none, outer=1) == index[o['name']]
    for name, value in variables:
        assert p.add_export(class_ref('SeqVar_Bool'), name, w32(0) + t_int('bValue', value) + none, outer=1) == index[name]
    return p


# ------------------------------------------------------------------------------------------------- harness
failures = []


def check(label, condition, detail=''):
    if not condition: failures.append(f'{label}: {detail}')


def kismet(root, ops, *entry, variables=(), seq='Seq'):
    (root / 'TestSeq.upk').write_bytes(build_sequence(ops, variables).build())
    proc = subprocess.run([reader, str(root / 'TestSeq.upk'), '--kismet-run', seq, '--cooked', str(root), *entry],
                          capture_output=True, text=True, encoding='utf-8')
    assert proc.stdout.strip(), ('no JSON output', proc.returncode, proc.stderr)
    return proc.returncode, json.loads(proc.stdout), proc


def expect(label, got, code, expected_code, **fields):
    """Whole-document comparison: unspecified list fields must be empty, so surprises show up."""
    want = dict(sequence='Seq', ops=None, entry_matches=1, executed=None, trace=[], host_boundary=[], errors=[], log=[])
    want.update(fields)
    for key, value in want.items():
        if value is None: continue
        check(f'{label} [{key}]', got[key] == value, f'expected {value!r}, got {got[key]!r}')
    check(f'{label} [exit code]', code == expected_code, f'expected {expected_code}, got {code}')
    check(f'{label} [fields]', set(got) == set(want), f'unexpected fields {sorted(set(got) ^ set(want))}')


with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    (root / 'Engine.upk').write_bytes(build_engine().build())
    (root / 'WillowGame.upk').write_bytes(build_willowgame().build())
    (root / 'Other.upk').write_bytes(build_plain(leaves=['Elsewhere']).build())
    (root / 'Missions.upk').write_bytes(build_plain('ToyGroup', ['ToyMission', 'OtherMission']).build())
    HOST = 'Engine.SeqAct_ToyWorldAction:'

    # (1a) remote event -> gate -> two actions: propagation and ordering come from decoded Links.
    chain_ops = [event('Ev', 'Ping', ('G1', IN)),
                 gate('G1', [out('Out', ('Act1', 0), ('Act2', 0))], open_=True),
                 world('Act1'), world('Act2'),
                 event('Idle', 'Nope', ('Act3', 0)), world('Act3')]
    code, got, _ = kismet(root, chain_ops, '--remote', 'Ping')
    expect('1a chain', got, code, 0, ops=6, executed=4,
           trace=['event Ev', 'Ev output 0 -> 1 link(s)', 'G1 <- In', 'G1 output 0 -> 2 link(s)',
                  'Act1 <- In', 'Act2 <- In'],
           host_boundary=[HOST + 'Act1 <- In', HOST + 'Act2 <- In'])

    # (1b) ActivateRemoteEvent re-enters the sequence: the matched event runs before the activator's own Out.
    code, got, _ = kismet(root, [event('Ev', 'Go', ('Hop', 0)),
                                 op('Hop', 'SeqAct_ActivateRemoteEvent', ['In'], [out('Out', ('Act1', 0))],
                                    scalars=[('name', 'EventName', 'Pong')]),
                                 event('Ev2', 'Pong', ('Act2', 0)), world('Act1'), world('Act2')], '--remote', 'Go')
    expect('1b remote re-entry', got, code, 0, ops=5, executed=5,
           trace=['event Ev', 'Ev output 0 -> 1 link(s)', 'Hop <- In', "Hop remote 'Pong'", 'event Ev2',
                  'Ev2 output 0 -> 1 link(s)', 'Hop output 0 -> 1 link(s)', 'Act2 <- In', 'Act1 <- In'],
           host_boundary=[HOST + 'Act2 <- In', HOST + 'Act1 <- In'])

    # --op enters the same event directly, and the full sequence path (with package prefix) resolves too.
    code, got, _ = kismet(root, chain_ops, '--op', 'Ev', seq='TestSeq.Seq')
    expect('1c --op', got, code, 0, sequence='TestSeq.Seq', ops=6, executed=4, trace=[
        'event Ev', 'Ev output 0 -> 1 link(s)', 'G1 <- In', 'G1 output 0 -> 2 link(s)',
        'Act1 <- In', 'Act2 <- In'], host_boundary=[HOST + 'Act1 <- In', HOST + 'Act2 <- In'])
    code, got, _ = kismet(root, chain_ops, '--op', 'Missing')
    expect('1d --op unknown', got, code, 1, ops=6, entry_matches=0, executed=0)

    # (2) delays: the CLI runs run() once at time 0, so delayed impulses are queued but not executed.
    # (CLI cannot call tick(); tick coverage is not possible from here.)
    code, got, _ = kismet(root, [event('Ev', 'Ping', ('Del', 0), ('G', IN)),
                                 op('Del', 'SeqAct_Delay', ['Start', 'Stop'], [out('Finished', ('ActDone', 0))],
                                    scalars=[('float', 'Duration', 2.5)]),
                                 gate('G', [out('Out', ('ActLate', 0), delay=1.5)], open_=True),
                                 world('ActDone'), world('ActLate')], '--remote', 'Ping')
    expect('2a delayed impulses not run at t=0', got, code, 0, ops=5, executed=3,
           trace=['event Ev', 'Ev output 0 -> 2 link(s)', 'Del <- Start', 'G <- In', 'G output 0 -> 1 link(s)'])
    code, got, _ = kismet(root, [event('Ev', 'Ping', ('D0', 0)),
                                 op('D0', 'SeqAct_Delay', ['Start', 'Stop'], [out('Finished', ('Act', 0))]),
                                 world('Act')], '--remote', 'Ping')
    expect('2b zero-duration delay completes in the same run', got, code, 0, ops=3, executed=4,
           trace=['event Ev', 'Ev output 0 -> 1 link(s)', 'D0 <- Start', 'D0 finished', 'D0 output 0 -> 1 link(s)',
                  'Act <- In'], host_boundary=[HOST + 'Act <- In'])

    # (3) gate inputs, one run each. Gate inputs: In=0 Open=1 Close=2 Toggle=3.
    def gate_case(label, gate_op, sequence_of_inputs, trace_tail, host, executed, errors=(), code_expected=0):
        code, got, _ = kismet(root, [event('Ev', 'Ping', *[('G', i) for i in sequence_of_inputs]), gate_op, world('Act')],
                              '--remote', 'Ping')
        head = ['event Ev', f'Ev output 0 -> {len(sequence_of_inputs)} link(s)']
        expect(label, got, code, code_expected, ops=3, executed=executed, trace=head + trace_tail,
               host_boundary=[HOST + 'Act <- In'] * host, errors=list(errors))

    through = [out('Out', ('Act', 0))]
    gate_case('3a closed gate blocks In', gate('G', through), [IN], ['G <- In'], 0, 2)
    gate_case('3b Open then In passes', gate('G', through), [OPEN, IN],
              ['G <- Open', 'G <- In', 'G output 0 -> 1 link(s)', 'Act <- In'], 1, 4)
    gate_case('3c In then Open blocks (order matters)', gate('G', through), [IN, OPEN], ['G <- In', 'G <- Open'], 0, 3)
    gate_case('3d Toggle opens a closed gate', gate('G', through), [TOGGLE, IN],
              ['G <- Toggle', 'G <- In', 'G output 0 -> 1 link(s)', 'Act <- In'], 1, 4)
    gate_case('3e Close shuts an open gate', gate('G', through, open_=True), [CLOSE, IN], ['G <- Close', 'G <- In'], 0, 3)
    gate_case('3f Toggle closes an open gate', gate('G', through, open_=True), [TOGGLE, IN],
              ['G <- Toggle', 'G <- In'], 0, 3)
    gate_case('3f2 Toggle twice reopens it', gate('G', through, open_=True), [TOGGLE, TOGGLE, IN],
              ['G <- Toggle', 'G <- Toggle', 'G <- In', 'G output 0 -> 1 link(s)', 'Act <- In'], 1, 5)
    # AutoCloseCount (UNVERIFIED against the game in src/kismet.cpp): closes after N passes.
    gate_case('3g AutoCloseCount=2 closes after two passes', gate('G', through, open_=True, auto=2), [IN, IN, IN],
              ['G <- In', 'G output 0 -> 1 link(s)', 'G <- In', 'G output 0 -> 1 link(s)', 'G <- In',
               'Act <- In', 'Act <- In'], 2, 6)

    # (4) SetBool writes its Target (from a linked Value, else DefaultValue); CompareBool routes by that value.
    def setbool_case(label, link_value, default, target_initial, expected_true):
        variables = [('VarSrc', 1 if link_value else 0), ('VarDst', 1 if target_initial else 0)]
        sb_vars = ([('Value', ['VarSrc'])] if link_value is not None else []) + [('Target', ['VarDst'])]
        ops = [event('Ev', 'Ping', ('SB', 0)),
               op('SB', 'SeqAct_SetBool', ['In'], [out('Out', ('Cmp', 0))], sb_vars, [('bool', 'DefaultValue', default)]),
               op('Cmp', 'SeqCond_CompareBool', ['In'], [out('True', ('ActT', 0)), out('False', ('ActF', 0))],
                  [('Bool', ['VarDst'])]),
               world('ActT'), world('ActF')]
        code, got, _ = kismet(root, ops, '--remote', 'Ping', variables=variables)
        branch = ['Cmp output 0 -> 1 link(s)', 'ActT <- In'] if expected_true else ['Cmp output 1 -> 1 link(s)', 'ActF <- In']
        expect(label, got, code, 0, ops=5, executed=4,
               trace=['event Ev', 'Ev output 0 -> 1 link(s)', 'SB <- In', 'SB output 0 -> 1 link(s)', 'Cmp <- In'] + branch,
               host_boundary=[HOST + ('ActT' if expected_true else 'ActF') + ' <- In'])

    setbool_case('4a linked Value true -> Target true -> True', True, False, False, True)
    setbool_case('4b linked Value false overrides DefaultValue true -> False', False, True, True, False)
    setbool_case('4c no Value linked: DefaultValue true -> True', None, True, False, True)
    setbool_case('4d no Value linked: DefaultValue false clears Target -> False', None, False, True, False)

    code, got, _ = kismet(root, [event('Ev', 'Ping', ('SB', 0)),
                                 op('SB', 'SeqAct_SetBool', ['In'], [out('Out', ('Act', 0))]), world('Act')], '--remote', 'Ping')
    expect('4e SetBool without Target is an error', got, code, 1, ops=3, executed=2,
           trace=['event Ev', 'Ev output 0 -> 1 link(s)', 'SB <- In'], errors=['SB: SetBool without a Target'])
    code, got, _ = kismet(root, [event('Ev', 'Ping', ('Cmp', 0)),
                                 op('Cmp', 'SeqCond_CompareBool', ['In'], [out('True', ('Act', 0))]), world('Act')], '--remote', 'Ping')
    expect('4f CompareBool without Bool is an error', got, code, 1, ops=3, executed=2,
           trace=['event Ev', 'Ev output 0 -> 1 link(s)', 'Cmp <- In'], errors=['Cmp: CompareBool without a Bool variable'])

    # (5) unsupported op class: reported, nothing downstream runs, exit 1; actions go to host_boundary.
    code, got, _ = kismet(root, [event('Ev', 'Ping', ('W', 0), ('U', 0)), world('W'),
                                 op('U', 'SeqCond_ToyUnhandled', ['In'], [out('Out', ('After', 0))]), world('After')],
                          '--remote', 'Ping')
    expect('5 unsupported op', got, code, 1, ops=4, executed=3,
           trace=['event Ev', 'Ev output 0 -> 2 link(s)', 'W <- In', 'U <- In'],
           host_boundary=[HOST + 'W <- In'],
           errors=['unsupported op class Engine.SeqCond_ToyUnhandled (U)'])

    # (6) a disabled output link is traced but does not propagate.
    code, got, _ = kismet(root, [event('Ev', 'Ping', ('Act', 0)), world('Act')], '--remote', 'Ping')
    check('6 control (enabled link propagates)', got['trace'][-1] == 'Act <- In', got)
    ops6 = [op('Ev', 'SeqEvent_RemoteEvent', [], [out('Out', ('Act', 0), disabled=True)], scalars=[('name', 'EventName', 'Ping')]),
            world('Act')]
    code, got, _ = kismet(root, ops6, '--remote', 'Ping')
    expect('6 disabled link', got, code, 0, ops=2, executed=1, trace=['event Ev', 'Ev output 0 disabled'])

    # (7) links that do not land on an op of this sequence: errors, no crash, the other targets still run.
    code, got, _ = kismet(root, [event('Ev', 'Ping', (('ext', 'Other', 'Elsewhere'), 0), ('Act', 0)), world('Act')],
                          '--remote', 'Ping')
    expect('7a link into another package', got, code, 1, ops=2, executed=2,
           trace=['event Ev', 'Ev output 0 -> 2 link(s)', 'Act <- In'], host_boundary=[HOST + 'Act <- In'],
           errors=['Ev: linked op is outside this sequence: Elsewhere'])
    code, got, _ = kismet(root, [event('Ev', 'Ping', ('Var', 0)), world('Act')], '--remote', 'Ping', variables=[('Var', 0)])
    expect('7b link to a local non-op export', got, code, 1, ops=2, executed=1,
           trace=['event Ev', 'Ev output 0 -> 1 link(s)'], errors=['Ev: linked op is outside this sequence: Var'])
    code, got, _ = kismet(root, [event('Ev', 'Ping', (None, 0)), world('Act')], '--remote', 'Ping')
    expect('7c null LinkedOp is skipped silently', got, code, 0, ops=2, executed=1, trace=['event Ev', 'Ev output 0 -> 1 link(s)'])

    # (8) unknown remote event.
    code, got, _ = kismet(root, chain_ops, '--remote', 'Nobody')
    expect('8 unknown remote event', got, code, 1, ops=6, entry_matches=0, executed=0)

    # (9) a two-op ring stops at the execution limit instead of hanging.
    ring = [event('Ev', 'Ping', ('GA', IN)),
            gate('GA', [out('Out', ('GB', IN))], open_=True), gate('GB', [out('Out', ('GA', IN))], open_=True)]
    code, got, _ = kismet(root, ring, '--remote', 'Ping')
    check('9 ring: exit code', code == 1, code)
    check('9 ring: error', got['errors'] == ['kismet execution limit exceeded'], got['errors'])
    # event(1) + 9,999 completed impulses, then the 10,000th pop makes `executed` 10,001 and aborts unexecuted.
    check('9 ring: executed', got['executed'] == 10001, got['executed'])
    check('9 ring: trace length', len(got['trace']) == 2 + 2 * 9999, len(got['trace']))
    check('9 ring: trace shape', got['trace'][:6] == ['event Ev', 'Ev output 0 -> 1 link(s)', 'GA <- In', 'GA output 0 -> 1 link(s)',
                                                      'GB <- In', 'GB output 0 -> 1 link(s)'], got['trace'][:6])
    check('9 ring: log clean', got['log'] == [], got['log'])

    # (10) mission remote events: a different class family, matched by mission path + event name.
    mission = ('ext', 'Missions', 'ToyGroup', 'ToyMission')
    other_mission = ('ext', 'Missions', 'ToyGroup', 'OtherMission')

    def mission_event(name, mission_ref, *to):
        # AssociatedMissionDefinition lives on the WillowGame class, so it is written as an object tag here.
        return op(name, 'WillowGame.WillowSeqEvent_MissionRemoteEvent', [], [out('Out', *to)],
                  scalars=[('name', 'EventName', 'Go'), ('obj', 'AssociatedMissionDefinition', mission_ref)])

    mission_ops = [mission_event('MEv', mission, ('ActM', 0)), mission_event('OEv', other_mission, ('ActO', 0)),
                   event('REv', 'Go', ('ActR', 0)), world('ActM'), world('ActO'), world('ActR')]
    code, got, _ = kismet(root, mission_ops, '--mission', 'ToyGroup.ToyMission', 'Go')
    expect('10a mission event matches its mission only', got, code, 0, ops=6, executed=2,
           trace=['event MEv', 'MEv output 0 -> 1 link(s)', 'ActM <- In'], host_boundary=[HOST + 'ActM <- In'])
    code, got, _ = kismet(root, mission_ops, '--mission', 'ToyGroup.OtherMission', 'Go')
    expect('10b other mission', got, code, 0, ops=6, executed=2,
           trace=['event OEv', 'OEv output 0 -> 1 link(s)', 'ActO <- In'], host_boundary=[HOST + 'ActO <- In'])
    code, got, _ = kismet(root, mission_ops, '--mission', 'ToyGroup.NoSuchMission', 'Go')
    expect('10c unknown mission', got, code, 1, ops=6, entry_matches=0, executed=0)
    code, got, _ = kismet(root, mission_ops, '--mission', 'ToyGroup.ToyMission', 'Stop')
    expect('10d wrong event name for the mission', got, code, 1, ops=6, entry_matches=0, executed=0)
    # a plain --remote activation must not fire mission events that share the event name
    code, got, _ = kismet(root, mission_ops, '--remote', 'Go')
    expect('10e plain remote ignores mission events', got, code, 0, ops=6, executed=2,
           trace=['event REv', 'REv output 0 -> 1 link(s)', 'ActR <- In'], host_boundary=[HOST + 'ActR <- In'])

    # (11) --kismet-census over the loaded graph: arrays-in-structs regression, counted without running anything.
    variables = [('VarSrc', 1), ('VarDst', 0)]
    ops = [event('Ev', 'Ping', ('SB', 0)),
           op('SB', 'SeqAct_SetBool', ['In'], [out('Out', ('Cmp', 0))], [('Value', ['VarSrc']), ('Target', ['VarDst'])]),
           op('Cmp', 'SeqCond_CompareBool', ['In'], [out('True', ('ActT', 0)), out('False', ('ActF', 0), (('ext', 'Other', 'Elsewhere'), 0))],
              [('Bool', ['VarDst'])]),
           world('ActT'), world('ActF')]
    (root / 'TestSeq.upk').write_bytes(build_sequence(ops, variables).build())
    census = subprocess.run([reader, str(root / 'TestSeq.upk'), '--kismet-census', '--cooked', str(root)],
                            capture_output=True, text=True, encoding='utf-8')
    census_json = json.loads(census.stdout)
    # outputs: Ev 1 + SB 1 + Cmp 2 = 4; links: Ev->SB, SB->Cmp, Cmp.True->ActT, Cmp.False->ActF + Elsewhere = 5, 1 unresolved.
    check('11 census', census.returncode == 0 and census_json['sequences'] == [
        dict(path='Seq', ops=5, outputs=4, links=5, unresolved=1, variable_links=3)] and census_json['totals'] == dict(
        sequences=1, failed=0, ops=5, outputs=4, links=5, unresolved=1) and census_json['log_entries'] == 0,
          (census.returncode, census.stdout, census.stderr))

    # (13) --originator enters every event whose Originator is that placed object (host: a population den/point
    # spawned something); events of other originators and non-event ops carrying the property are not entered.
    elsewhere = ('ext', 'Other', 'Elsewhere')
    origin_ops = [op('P1', 'SequenceEvent', [], [out('Out', ('Act1', 0))], scalars=[('obj', 'Originator', elsewhere)]),
                  op('P2', 'SequenceEvent', [], [out('Out', ('Act2', 0))], scalars=[('obj', 'Originator', elsewhere)]),
                  op('P3', 'SequenceEvent', [], [out('Out', ('Act3', 0))], scalars=[('obj', 'Originator', mission)]),
                  op('NotEvent', 'SeqAct_ToyWorldAction', ['In'], [], scalars=[('obj', 'Originator', elsewhere)]),
                  world('Act1'), world('Act2'), world('Act3')]
    code, got, _ = kismet(root, origin_ops, '--originator', 'Elsewhere')
    expect('13a originator events', got, code, 0, ops=7, entry_matches=2, executed=4,
           trace=['event P1', 'P1 output 0 -> 1 link(s)', 'event P2', 'P2 output 0 -> 1 link(s)', 'Act1 <- In', 'Act2 <- In'],
           host_boundary=[HOST + 'Act1 <- In', HOST + 'Act2 <- In'])
    code, got, _ = kismet(root, origin_ops, '--originator', 'ToyGroup.ToyMission')
    expect('13b other originator', got, code, 0, ops=7, executed=2,
           trace=['event P3', 'P3 output 0 -> 1 link(s)', 'Act3 <- In'], host_boundary=[HOST + 'Act3 <- In'])
    code, got, _ = kismet(root, origin_ops, '--originator', 'Nowhere')
    expect('13c unknown originator', got, code, 1, ops=7, entry_matches=0, executed=0)

    # (12) a sequence path that does not exist is a hard error, not an empty run.
    missing = subprocess.run([reader, str(root / 'TestSeq.upk'), '--kismet-run', 'NoSuchSeq', '--cooked', str(root),
                              '--remote', 'Ping'], capture_output=True, text=True, encoding='utf-8')
    check('12 missing sequence', missing.returncode != 0 and 'kismet sequence not found' in missing.stderr,
          (missing.returncode, missing.stderr))

if failures:
    print(f'{len(failures)} kismet check(s) failed:')
    for failure in failures: print('  - ' + failure)
    sys.exit(1)
print('kismet synthetic coverage passed.')
