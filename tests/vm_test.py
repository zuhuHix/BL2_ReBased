"""Synthetic VM coverage (Phase 2): builds a tiny "Core" package with classes, properties, functions and
hand-assembled bytecode, runs it through `ow-package --run`, and checks results computed by hand.

Nothing here comes from the game. The function layout is the one recovered from the real packages
(see docs/verification/SCRIPT_BYTECODE_DISASM.md): u16 local-array count + entries, 10 i32, in-memory
size, file size, script bytes, function tail.
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


class Asm:
    """Assembles one function body. Object references are 4 bytes in the file and 8 in memory."""
    def __init__(self):
        self.b = bytearray()
        self.refs = 0
        self.statements = []

    def here(self): return len(self.b) + 4 * self.refs        # in-memory offset

    def stmt(self): self.statements.append(self.here())

    def raw(self, *bs): self.b += bytes(bs)
    def ref(self, r): self.b += struct.pack('<i', r); self.refs += 1
    def i32(self, v): self.b += struct.pack('<i', v)
    def w(self, v): self.b += struct.pack('<H', v)
    def end(self): self.stmt(); self.raw(0x53)


FUNC_NATIVE, FUNC_FINAL, FUNC_DEFINED, FUNC_STATIC, FUNC_OPERATOR, FUNC_PUBLIC = 0x400, 0x1, 0x2, 0x2000, 0x1000, 0x20000
CPF_PARM, CPF_OUT, CPF_OPT, CPF_RET = 0x80, 0x100, 0x10, 0x400


def build_package():
    p = Package()
    imp = {n: p.add_import('Class', n) for n in
           ('Class', 'Function', 'IntProperty', 'FloatProperty', 'BoolProperty', 'StrProperty', 'ArrayProperty')}
    none = p.fname('None')

    def prop(kind, owner, name, flags=0):
        payload = w32(0) + none + w32(0) + u32(1) + u64(flags) + none + w32(0)
        return p.add_export(imp[kind], name, payload, outer=owner)

    def func(owner, name, params, body, flags=FUNC_FINAL | FUNC_DEFINED | FUNC_PUBLIC, native=0, friendly=None, ret='Int'):
        """params: [(kind, name, flags)]; children are exported in reverse declaration order."""
        index = len(p.exports) + 1 + len(params) + 1    # reserve: children first, then the function itself
        ids = {}
        script = body.b if body else b''
        payload = u16(0) + w32(*([0] * 10)) + w32(len(script) + 4 * (body.refs if body else 0), len(script)) + script
        payload += u16(native) + bytes([0]) + u32(flags) + p.fname(friendly or name)
        # children are created before the function so the function's index is known only afterwards; refs to
        # them are filled by the caller through `ids`
        return payload, ids

    return p, imp, none, prop, func


def make():
    p, imp, none, prop, _ = build_package()

    def make_function(owner, name, declared, asm_fn, flags=FUNC_FINAL | FUNC_DEFINED | FUNC_PUBLIC, native=0, friendly=None,
                      result='Int', locals_=()):
        """declared: [(kind, name, extraflags)] in declaration order. asm_fn(ids) -> Asm (or None for natives)."""
        # Children first, in reverse declaration order (the real packages export them that way).
        first_child = len(p.exports) + 1
        children = []
        for kind, pname, extra in reversed([*locals_, *declared]):
            is_param = any(pname == d[1] for d in declared)
            children.append((kind, pname, extra | (CPF_PARM if is_param else 0)))
        ids = {}
        # Return value is declared last, so it is exported first.
        ordered = [(result_kind, 'ReturnValue', CPF_PARM | CPF_OUT | CPF_RET) for result_kind in ([result] if result else [])] + children
        func_index = first_child + len(ordered)
        for kind, pname, flags_ in ordered:
            ids[pname] = prop(kind + 'Property', func_index, pname, flags_)
        asm = asm_fn(ids) if asm_fn else None
        script = bytes(asm.b) if asm else b''
        memory = (len(script) + 4 * asm.refs) if asm else 0
        payload = u16(0) + w32(*([0] * 10)) + w32(memory, len(script)) + script
        payload += u16(native) + bytes([0]) + u32(flags) + p.fname(friendly or name)
        index = p.add_export(imp['Function'], name, payload, outer=owner)
        assert index == func_index, (index, func_index)
        ids['__self__'] = index
        return ids

    class_ids = {}
    class_ids['Object'] = p.add_export(imp['Class'], 'Object', w32(0) * 4)
    obj = class_ids['Object']

    def native(name, friendly, number, params, result):
        declared = [(k, f'P{i}', 0) for i, k in enumerate(params)]
        return make_function(obj, name, declared, None, flags=FUNC_NATIVE | FUNC_OPERATOR | 0x23000, native=number,
                             friendly=friendly, result=result)

    native('Add_IntInt', '+', 146, ['Int', 'Int'], 'Int')
    native('LessEq_IntInt', '<=', 152, ['Int', 'Int'], 'Bool')
    native('Concat_StrStr', '$', 112, ['Str', 'Str'], 'Str')
    make_function(obj, 'Mystery', [('Int', 'P0', 0)], None, flags=FUNC_NATIVE | 0x20000, native=999, result='Int')

    foo = class_ids['Foo'] = p.add_export(imp['Class'], 'Foo', w32(0) * 4, super_ref=obj)
    bar = class_ids['Bar'] = p.add_export(imp['Class'], 'Bar', w32(0) * 4, super_ref=foo)
    count = prop('IntProperty', foo, 'Count')

    # Expression helpers (operate on an Asm) -------------------------------------------------------------
    def local(a, ids, name): a.raw(0x00); a.ref(ids[name])
    def intc(a, v): a.raw(0x1D); a.i32(v)
    def call(a, index, *args):              # numbered native call: opcode byte = index (>= 0x70)
        a.raw(index)
        for arg in args: arg()
        a.raw(0x16)
    L = lambda a, ids, n: (lambda: local(a, ids, n))
    C = lambda a, v: (lambda: intc(a, v))

    # Foo.Answer() -> 42
    def answer(ids):
        a = Asm(); a.stmt(); a.raw(0x04); intc(a, 42); a.end(); return a
    make_function(foo, 'Answer', [], answer)

    # Foo.Add2(int a, int b) -> a + b
    def add2(ids):
        a = Asm(); a.stmt(); a.raw(0x04); call(a, 146, L(a, ids, 'A'), L(a, ids, 'B')); a.end(); return a
    make_function(foo, 'Add2', [('Int', 'A', 0), ('Int', 'B', 0)], add2)

    # Foo.SumTo(int n): i = 1; total = 0; while (i <= n) { total += i; i++ } return total
    def sum_to(ids):
        def build(start_target, end_target):
            a = Asm()
            a.stmt(); a.raw(0x0F); local(a, ids, 'I'); a.raw(0x26)
            a.stmt(); a.raw(0x0F); local(a, ids, 'Total'); a.raw(0x25)
            loop = a.here()
            a.stmt(); a.raw(0x07); a.w(end_target); call(a, 152, L(a, ids, 'I'), L(a, ids, 'N'))
            a.stmt(); a.raw(0x0F); local(a, ids, 'Total'); call(a, 146, L(a, ids, 'Total'), L(a, ids, 'I'))
            a.stmt(); a.raw(0x0F); local(a, ids, 'I'); call(a, 146, L(a, ids, 'I'), C(a, 1))
            a.stmt(); a.raw(0x06); a.w(start_target)
            end = a.here()
            a.stmt(); a.raw(0x04); local(a, ids, 'Total')
            a.end()
            return a, loop, end
        _, loop, end = build(0, 0)
        return build(loop, end)[0]
    make_function(foo, 'SumTo', [('Int', 'N', 0)], sum_to, locals_=[('Int', 'I', 0), ('Int', 'Total', 0)])

    # Foo.Pick(int n): switch (n) { case 1: return 10; case 2: return 20; default: return 30 }
    def pick(ids):
        def build(case2, default):
            a = Asm()
            a.stmt(); a.raw(0x05); a.ref(ids['N']); a.raw(1); local(a, ids, 'N')
            a.stmt(); a.raw(0x0A); case_fix = len(a.b); a.w(case2); intc(a, 1)
            a.stmt(); a.raw(0x04); intc(a, 10)
            here2 = a.here()
            a.stmt(); a.raw(0x0A); fix2 = len(a.b); a.w(default); intc(a, 2)
            a.stmt(); a.raw(0x04); intc(a, 20)
            here3 = a.here()
            a.stmt(); a.raw(0x0A); a.w(0xFFFF)
            a.stmt(); a.raw(0x04); intc(a, 30)
            a.end()
            return a, here2, here3
        _, here2, here3 = build(0, 0)
        return build(here2, here3)[0]
    make_function(foo, 'Pick', [('Int', 'N', 0)], pick)

    # Virtual dispatch: Foo.Get() = 1, Bar.Get() = 2, Foo.CallGet() = VirtualFunction Get
    def get(value):
        def f(ids):
            a = Asm(); a.stmt(); a.raw(0x04); intc(a, value); a.end(); return a
        return f
    make_function(foo, 'Get', [], get(1), flags=FUNC_DEFINED | FUNC_PUBLIC)
    make_function(bar, 'Get', [], get(2), flags=FUNC_DEFINED | FUNC_PUBLIC)

    def call_get(ids):
        a = Asm(); a.stmt(); a.raw(0x04); a.raw(0x1B); a.i32(p.name('Get')); a.i32(0); a.raw(0x16); a.end(); return a
    make_function(foo, 'CallGet', [], call_get)

    # Instance variable: Foo.GetCount() returns Count (defaults: Foo 7, Bar 9)
    def get_count(ids):
        a = Asm(); a.stmt(); a.raw(0x04); a.raw(0x01); a.ref(count); a.end(); return a
    make_function(foo, 'GetCount', [], get_count)

    def tagged_int(name, value):
        return p.fname(name) + p.fname('IntProperty') + w32(4, 0) + w32(value)
    p.add_export(foo, 'Default__Foo', w32(0) + tagged_int('Count', 7) + none)
    p.add_export(bar, 'Default__Bar', w32(0) + tagged_int('Count', 9) + none)

    # Optional parameter with a default: Foo.Opt(int a, optional int b = 5) = a + b
    def opt(ids):
        a = Asm()
        a.stmt(); a.raw(0x49); fix = len(a.b); a.w(0); start = a.here(); intc(a, 5); a.b[fix:fix + 2] = struct.pack('<H', a.here() - start)
        a.stmt(); a.raw(0x15)
        a.stmt(); a.raw(0x04); call(a, 146, L(a, ids, 'A'), L(a, ids, 'B'))
        a.end(); return a
    make_function(foo, 'Opt', [('Int', 'A', 0), ('Int', 'B', CPF_OPT)], opt)

    # Out parameter: Foo.Bump(out int x) { x = x + 1 }
    def bump(ids):
        a = Asm()
        a.stmt(); a.raw(0x0F); a.raw(0x48); a.ref(ids['X']); call(a, 146, L(a, ids, 'X'), C(a, 1))
        a.stmt(); a.raw(0x04); a.raw(0x3A); a.ref(ids['ReturnValue'])
        a.end(); return a
    make_function(foo, 'Bump', [('Int', 'X', CPF_OUT)], bump, result='Int')

    # Strings: Foo.Join(string a, string b) = a $ b
    def join(ids):
        a = Asm(); a.stmt(); a.raw(0x04); call(a, 112, L(a, ids, 'A'), L(a, ids, 'B')); a.end(); return a
    make_function(foo, 'Join', [('Str', 'A', 0), ('Str', 'B', 0)], join, result='Str')

    # Unimplemented native: Foo.CallMystery() = Mystery(5), logs UNIMPLEMENTED and returns 0
    def call_mystery(ids):
        a = Asm(); a.stmt(); a.raw(0x04); a.raw(0x1C); a.ref(MYSTERY);
        intc(a, 5); a.raw(0x16); a.end(); return a
    MYSTERY = p.exports.index(next(e for e in p.exports if e[3] == p.fname('Mystery'))) + 1
    make_function(foo, 'CallMystery', [], call_mystery)

    # Dynamic array: Foo.Arr() adds 3 and 4 to a local array and returns its length
    def arr(ids):
        a = Asm()
        for v in (3, 4):
            a.stmt(); a.raw(0x55); local(a, ids, 'Items'); a.w(0); intc(a, v); a.raw(0x16)
        a.stmt(); a.raw(0x04); a.raw(0x36); local(a, ids, 'Items')
        a.end(); return a
    make_function(foo, 'Arr', [], arr, locals_=[('Array', 'Items', 0)])
    return p


def run(root, function, *args, self_class=None):
    cmd = [reader, str(root / 'Core.upk'), '--run', function, '--cooked', str(root)]
    if self_class: cmd += ['--self', self_class]
    for a in args: cmd += ['--arg', a]
    out = subprocess.run(cmd, capture_output=True, text=True, encoding='utf-8')
    assert out.returncode == 0, (function, out.stdout, out.stderr)
    return json.loads(out.stdout)


with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    (root / 'Core.upk').write_bytes(make().build())
    # sanity: the package loads and the functions are recognised by the loader
    check = subprocess.run([reader, str(root / 'Core.upk'), '--script-check', '--failures'], capture_output=True, text=True, encoding='utf-8')
    assert check.returncode == 0, check.stderr
    summary = json.loads(check.stdout)
    assert summary['failed'] == 0, summary

    assert run(root, 'Core.Foo.Answer')['result'] == '42'
    assert run(root, 'Core.Foo.Add2', 'i:40', 'i:2')['result'] == '42'
    assert run(root, 'Core.Foo.Add2', 'i:2147483647', 'i:1')['result'] == '-2147483648'      # 32-bit wrap
    assert run(root, 'Core.Foo.SumTo', 'i:10')['result'] == '55'
    assert run(root, 'Core.Foo.SumTo', 'i:0')['result'] == '0'
    assert [run(root, 'Core.Foo.Pick', f'i:{n}')['result'] for n in (1, 2, 3)] == ['10', '20', '30']
    assert run(root, 'Core.Foo.CallGet', self_class='Core.Foo')['result'] == '1'
    assert run(root, 'Core.Foo.CallGet', self_class='Core.Bar')['result'] == '2'                # virtual dispatch
    assert run(root, 'Core.Foo.GetCount', self_class='Core.Foo')['result'] == '7'               # class default
    assert run(root, 'Core.Foo.GetCount', self_class='Core.Bar')['result'] == '9'               # subclass default
    assert run(root, 'Core.Foo.Opt', 'i:1')['result'] == '6'                                     # default b = 5
    assert run(root, 'Core.Foo.Opt', 'i:1', 'i:10')['result'] == '11'
    bumped = run(root, 'Core.Foo.Bump', 'i:41')
    assert bumped['args'] == ['42'], bumped                                                      # out parameter
    assert run(root, 'Core.Foo.Join', 's:ab', 's:cd')['result'] == '"abcd"'
    mystery = run(root, 'Core.Foo.CallMystery')
    assert mystery['result'] == '0' and any('UNIMPLEMENTED Object.Mystery(int)' in l for l in mystery['log']), mystery
    assert run(root, 'Core.Foo.Arr')['result'] == '2'
    # a runaway script stops at the step limit instead of hanging is covered by the C++ limit, not here
print('vm synthetic coverage passed.')
