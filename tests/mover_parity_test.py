"""Differential gate for the Rust mover: `ow-package --mover-probe` and `--mover-event` against the C++ binary.

AI-assisted (Claude Code). Lane W5-DIALOG (decision 2026-11-15-32). Builds an invented install: an Engine.upk that
declares a toy InterpActor (its two notifications and a timer callback in synthetic bytecode, the scoped SetTimer /
ClearTimer natives, an ambient AudioComponent with Stop) together with toy Kismet declarations (the public UE3
property names the executor reads, as tests/kismet_test.py has them), and a Doors.upk holding a placed mover, a
sequence whose remote events play and reverse its Matinee action and switch an invented world action, and an action
with no owning sequence. Every name, value and bytecode body is invented (start sets a flag, completion schedules a
callback, the callback clears it); nothing is transcribed from the game.

Each command line runs on both binaries; exit code, stdout (CRLF read as LF) and stderr must be equal. Two kinds of
case differ on purpose and are checked as such: a notification that reaches an unbound native (the C++ runtime logs it
and runs on; the Rust --mover-event refuses it, architecture section 3.8, "Stub policy per command", so both fail the
notification and only its error text and steps differ), and an export that is not an actor bound as the placed actor
(both fail the binding; the error text differs, see the comment at the case).

Run: python tests/mover_parity_test.py <C++ ow-package> <Rust ow-package>
"""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile


def w32(*values): return b''.join(struct.pack('<i', v) for v in values)
def u16(v): return struct.pack('<H', v)
def u32(v): return struct.pack('<I', v)
def u64(v): return struct.pack('<Q', v)


FUNC_NATIVE, FUNC_FINAL, FUNC_DEFINED, FUNC_PUBLIC = 0x400, 0x1, 0x2, 0x20000
CPF_PARM, CPF_OPT, CPF_OUT, CPF_RET = 0x80, 0x10, 0x100, 0x400


class Package:
    """The toy package writer of tests/kismet_test.py (imports with an explicit class package and outer)."""
    def __init__(self):
        self.names, self.imports, self.exports = [], [], []

    def name(self, text):
        if text not in self.names: self.names.append(text)
        return self.names.index(text)

    def fname(self, text, number=0): return w32(self.name(text), number)

    def add_import(self, class_name, outer, name):
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
        payload_at = eo + 68 * len(self.exports)
        export_data, payloads = b'', b''
        for cls, sup, outer, name, payload in self.exports:
            export_data += w32(cls, sup, outer) + name + w32(0) + bytes(8) + w32(len(payload), payload_at + len(payloads)) \
                + w32(0) + w32(0) + bytes(20)
            payloads += payload
        header = w32(0x9e2a83c1 - (1 << 32), 832 | (46 << 16), 48, 0, 0, len(self.names), 48, len(self.exports), eo,
                     len(self.imports), io, 0)
        return header + table + import_data + export_data + payloads


class Asm:
    """One function body; object references are 4 bytes in the file and 8 in memory."""
    def __init__(self): self.b, self.refs = bytearray(), 0
    def raw(self, *bs): self.b += bytes(bs)
    def ref(self, r): self.b += struct.pack('<i', r); self.refs += 1


def build_engine(broken=False):
    p = Package()
    imp = {n: p.add_import('Class', 0, n) for n in
           ('Class', 'Function', 'ScriptStruct', 'IntProperty', 'FloatProperty', 'BoolProperty', 'StrProperty',
            'NameProperty', 'ObjectProperty', 'ArrayProperty', 'StructProperty')}
    none = p.fname('None')

    def prop(kind, owner, name, type_ref=0, flags=0):
        payload = w32(0) + none + w32(0) + u32(1) + u64(flags) + none + w32(0) + w32(type_ref)
        return p.add_export(imp[kind + 'Property'], name, payload, outer=owner)

    def array_of(owner, name, inner_kind, inner_ref=0):
        index = len(p.exports) + 1
        prop('Array', owner, name, index + 1)          # the inner declaration is exported right after it
        prop(inner_kind, index, name, inner_ref)
        return index

    def struct_(name): return p.add_export(imp['ScriptStruct'], name, w32(0) * 4)
    root = []
    def cls(name, super_ref=0):
        return p.add_export(imp['Class'], name, w32(0) * 4, super_ref=super_ref or (root[0] if root else 0))

    def function(owner, name, params, body=None, flags=FUNC_FINAL | FUNC_DEFINED | FUNC_PUBLIC, native=0, result=None):
        """params: [(kind, name, extra flags)] in declaration order; children are exported in reverse, then the function."""
        ordered = ([(result, 'ReturnValue', CPF_PARM | CPF_OUT | CPF_RET)] if result else []) + \
            [(k, n, f | CPF_PARM) for k, n, f in reversed(params)]
        index = len(p.exports) + 1 + len(ordered)
        for kind, pname, f in ordered:
            prop(kind, index, pname, obj if kind == 'Object' else 0, f)
        script = bytes(body.b) if body else b''
        memory = len(script) + 4 * body.refs if body else 0
        payload = u16(0) + w32(*([0] * 10)) + w32(memory, len(script)) + script + u16(native) + bytes([0]) + u32(flags) \
            + p.fname(name)
        assert p.add_export(imp['Function'], name, payload, outer=owner) == index
        return index

    root.append(cls('Object'))
    obj = root[0]
    mystery = function(obj, 'Mystery', [('Int', 'P0', 0)], flags=FUNC_NATIVE | 0x20000, native=999, result='Int')
    # Toy Kismet declarations (tests/kismet_test.py's).
    s_in = struct_('SeqOpInputLink')
    prop('Str', s_in, 'LinkDesc'); prop('Bool', s_in, 'bDisabled'); prop('Float', s_in, 'ActivateDelay')
    s_link = struct_('SeqOpOutputInputLink')
    prop('Object', s_link, 'LinkedOp', obj); prop('Int', s_link, 'InputLinkIdx')
    s_out = struct_('SeqOpOutputLink')
    array_of(s_out, 'Links', 'Struct', s_link)
    prop('Str', s_out, 'LinkDesc'); prop('Bool', s_out, 'bDisabled'); prop('Float', s_out, 'ActivateDelay')
    s_var = struct_('SeqVarLink')
    prop('Str', s_var, 'LinkDesc'); array_of(s_var, 'LinkedVariables', 'Object', obj)
    op = cls('SequenceOp')
    array_of(op, 'InputLinks', 'Struct', s_in); array_of(op, 'OutputLinks', 'Struct', s_out)
    array_of(op, 'VariableLinks', 'Struct', s_var); prop('Int', op, 'ActivateCount')
    cls('Sequence', op)
    event = cls('SequenceEvent', op)
    prop('Int', event, 'TriggerCount'); prop('Object', event, 'Originator', obj)
    remote = cls('SeqEvent_RemoteEvent', event)
    prop('Name', remote, 'EventName')
    action = cls('SequenceAction', op)
    cls('SeqAct_ToyToggle', action)                   # invented: a world action, listed at the host boundary
    # The toy mover.
    actor = cls('Actor')
    clear = function(actor, 'ClearTimer', [('Name', 'inTimerFunc', CPF_OPT), ('Object', 'inObj', CPF_OPT)],
                     flags=FUNC_NATIVE | 0x20000)
    timer = function(actor, 'SetTimer', [('Float', 'InRate', 0), ('Bool', 'inbLoop', CPF_OPT),
                                         ('Name', 'inTimerFunc', CPF_OPT), ('Object', 'inObj', CPF_OPT)],
                     flags=FUNC_NATIVE | 0x20000)
    audio = cls('AudioComponent')
    prop('Object', audio, 'SoundCue', obj)
    function(audio, 'Stop', [], flags=FUNC_NATIVE | 0x20000)
    interp = cls('SeqAct_Interp', action)
    prop('Bool', interp, 'bReversePlayback')
    mover = cls('InterpActor', actor)
    checkpoint = prop('Bool', mover, 'bShouldSaveForCheckpoint')
    delay = prop('Float', mover, 'Delay')
    prop('Object', mover, 'AmbientSoundComponent', audio)
    callback = p.fname('Callback')

    started = Asm()   # bShouldSaveForCheckpoint = true; ClearTimer('Callback'); return;
    started.raw(0x14, 0x01); started.ref(checkpoint); started.raw(0x27)
    started.raw(0x1C); started.ref(clear); started.raw(0x21); started.b += callback; started.raw(0x16)
    started.raw(0x04, 0x0B, 0x53)
    finish = Asm()    # SetTimer(Delay, false, 'Callback'); [Mystery(0);] return;
    finish.raw(0x1C); finish.ref(timer); finish.raw(0x01); finish.ref(delay); finish.raw(0x28, 0x21)
    finish.b += callback; finish.raw(0x16)
    if broken: finish.raw(0x1C); finish.ref(mystery); finish.raw(0x25, 0x16)
    finish.raw(0x04, 0x0B, 0x53)
    back = Asm()      # bShouldSaveForCheckpoint = false; return;
    back.raw(0x14, 0x01); back.ref(checkpoint); back.raw(0x28, 0x04, 0x0B, 0x53)
    function(mover, 'InterpolationStarted', [('Object', 'InterpAction', 0), ('Object', 'GroupInst', 0)], started)
    function(mover, 'InterpolationFinished', [('Object', 'InterpAction', 0)], finish)
    function(mover, 'Callback', [], back)
    return p


def build_doors():
    p = Package()
    none = p.fname('None')
    engine = p.add_import('Package', 0, 'Engine')
    classes = {}

    def cls(name):
        if name not in classes: classes[name] = p.add_import('Class', engine, name)
        return classes[name]

    def tag(name, kind, size, body, detail=b''): return p.fname(name) + p.fname(kind) + w32(size, 0) + detail + body
    def t_int(name, v): return tag(name, 'IntProperty', 4, w32(v))
    def t_float(name, v): return tag(name, 'FloatProperty', 4, struct.pack('<f', v))
    def t_name(name, v): return tag(name, 'NameProperty', 8, p.fname(v))
    def t_obj(name, ref): return tag(name, 'ObjectProperty', 4, w32(ref))
    def t_str(name, text):
        data = text.encode() + b'\0'
        return tag(name, 'StrProperty', 4 + len(data), w32(len(data)) + data)
    def t_array(name, count, elements): return tag(name, 'ArrayProperty', 4 + len(elements), w32(count) + elements)
    def inputs(*descs): return t_array('InputLinks', len(descs), b''.join(t_str('LinkDesc', d) + none for d in descs))
    def outputs(*outs):
        elements = b''
        for desc, to in outs:
            targets = b''.join(t_obj('LinkedOp', r) + t_int('InputLinkIdx', i) + none for r, i in to)
            elements += t_array('Links', len(to), targets) + t_str('LinkDesc', desc) + none
        return t_array('OutputLinks', len(outs), elements)

    seq, open_, close, inner, lamp = 1, 2, 3, 4, 5
    assert p.add_export(cls('Sequence'), 'Seq', w32(0) + none) == seq
    ops = [
        ('SeqEvent_RemoteEvent', 'OpenEvent', t_name('EventName', 'OpenDoor') + outputs(('Out', [(inner, 0), (lamp, 0)]))),
        ('SeqEvent_RemoteEvent', 'CloseEvent', t_name('EventName', 'CloseDoor') + outputs(('Out', [(inner, 1)]))),
        ('SeqAct_Interp', 'InnerAction', inputs('Play', 'Reverse', 'Stop')
         + outputs(('Completed', [(lamp, 1)]), ('Reversed', []))),
        ('SeqAct_ToyToggle', 'Lamp', inputs('Turn On', 'Turn Off')),
    ]
    for (c, name, body), index in zip(ops, (open_, close, inner, lamp)):
        assert p.add_export(cls(c), name, w32(0) + body + none, outer=seq) == index
    p.add_export(cls('InterpActor'), 'PlacedMover', bytes(26) + t_float('Delay', 0.5) + none)
    p.add_export(cls('SeqAct_Interp'), 'LoneAction', w32(0) + none)
    return p


def run(binary, args):
    done = subprocess.run([binary, *map(str, args)], capture_output=True)
    lf = lambda b: b.replace(b'\r\n', b'\n')
    return done.returncode, lf(done.stdout), lf(done.stderr)


def main():
    if len(sys.argv) != 3:
        sys.exit('usage: python tests/mover_parity_test.py <C++ ow-package> <Rust ow-package>')
    cxx, rust = (str(Path(a).resolve()) for a in sys.argv[1:3])
    failures, compared = [], 0
    with tempfile.TemporaryDirectory() as folder:
        root = Path(folder)
        good, broken = root / 'good', root / 'broken'
        for directory, engine in ((good, build_engine()), (broken, build_engine(broken=True))):
            directory.mkdir()
            (directory / 'Engine.upk').write_bytes(engine.build())
            (directory / 'Doors.upk').write_bytes(build_doors().build())

        def both(directory, *args):
            line = [directory / 'Doors.upk', *args]
            line = [a if a != '<cooked>' else directory for a in line]
            return run(cxx, line), run(rust, line), ' '.join(map(str, args))

        def same(directory, *args, expect=None):
            nonlocal compared
            a, b, label = both(directory, *args)
            compared += 1
            if a != b:
                failures.append(f'{label}:\n  C++  {a!r}\n  Rust {b!r}')
            elif not a[1] and not a[2]:
                failures.append(f'{label}: both binaries printed nothing')
            elif expect:
                problem = expect(a)
                if problem: failures.append(f'{label}: {problem}')

        def doc(result): return json.loads(result[1])

        def field(name, wanted):
            return lambda r: None if doc(r)[name] == wanted else f'{name} is {doc(r)[name]!r}, expected {wanted!r}'

        probe = ('--mover-probe', 'PlacedMover', 'Seq.InnerAction', '--cooked', '<cooked>')
        same(good, *probe, expect=lambda r: None if r[0] == 0 else f'exit {r[0]}')
        same(good, '--mover-probe', 'PlacedMover', 'LoneAction', '--cooked', '<cooked>')
        same(broken, *probe, expect=lambda r: None if r[0] == 1 else f'exit {r[0]}')
        event = lambda name, action='Seq.InnerAction': ('--mover-event', 'PlacedMover', action, '--cooked', '<cooked>', name)
        same(good, *event('OpenDoor'), expect=field('motion', 1))
        same(good, *event('CloseDoor'), expect=field('motion', -1))
        same(good, *event('Nobody'), expect=field('matched', 0))
        same(good, *event('OpenDoor', 'LoneAction'), expect=field('errors', ['mover action path has no owning sequence']))
        # Binding failures and bad command lines: the C++ error text on stderr.
        same(good, '--mover-event', 'PlacedMover', 'Ghost.InnerAction', '--cooked', '<cooked>', 'OpenDoor')
        same(good, '--mover-event', 'PlacedMover', 'Seq.InnerAction', '--cooked', '<cooked>')
        same(good, '--mover-event', 'PlacedMover', 'Seq.InnerAction', '--wrong', '<cooked>', 'OpenDoor')

        # A second difference on purpose: an export that is not an actor, asked for as the placed actor. The C++ decodes
        # it from the 26-byte prefix anyway and fails on what it reads there; the Rust definition store derives the
        # export's own prefix and refuses the mismatch (DefError::PrefixMismatch, decision 2026-11-15-32). Both fail the
        # binding with nothing on stdout.
        for args in (('--mover-event', 'Seq.InnerAction', 'PlacedMover', '--cooked', '<cooked>', 'OpenDoor'),
                     ('--mover-probe', 'Seq.Lamp', 'Seq.InnerAction', '--cooked', '<cooked>')):
            a, b, label = both(good, *args)
            compared += 1
            if not (a[0] == b[0] == 1 and a[1] == b[1] == b'' and a[2] and b'property prefix, not 26' in b[2]):
                failures.append(f'{label} (prefix): {a!r} / {b!r}')

        # The one difference on purpose: a finish notification that reaches an unbound native.
        a, b, label = both(broken, *event('OpenDoor'))
        compared += 1
        da, db = doc(a), doc(b)
        rest = lambda d: {k: v for k, v in d.items() if k not in ('finish_steps', 'finish_error')}
        if a[0] != 1 or b[0] != 1 or rest(da) != rest(db):
            failures.append(f'{label} (stub policy): {a!r} / {b!r}')
        elif not da['finish_error'].startswith('UNIMPLEMENTED ') or 'unbound native' not in db['finish_error']:
            failures.append(f'{label} (stub policy): finish errors {da["finish_error"]!r} / {db["finish_error"]!r}')

    for f in failures: print('FAIL', f)
    print(f'mover parity: {compared - len(failures)} of {compared} cases as expected')
    sys.exit(1 if failures else 0)


if __name__ == '__main__':
    main()
