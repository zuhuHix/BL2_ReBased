"""AI-assisted (Claude Code). Synthetic `ow-package --object-dump` coverage: builds a toy `Core` package with
hand-assembled reflection exports and tagged data (the package writer and tag helpers of tests/behavior_test.py),
runs only `--object-dump`, and checks the output against values computed by hand. It runs on the C++ and the Rust
binary alike (tests/rust_parity.d/decode.txt).

Nothing here comes from the game: every class, struct, property and value is invented.

Pinned here (the C++ VM's loading half, `src/vm.cpp`; UNVERIFIED in the running game):
  * a class default object applies each class's `Default__` tags from the root class down, each adding only what it
    changes;
  * an export starts from its class default object and applies its own tags after the property prefix: 4 bytes for an
    ordinary object, 8 for a component, 26 for a placed actor;
  * a struct field the tags omit takes the struct's own default tag;
  * without `--all` only what differs from the class default object prints;
  * the refusals: an export index that is not positive or a prefix other than 4, 8 and 26 ("unsupported exported
    object request"), a Class export ("cannot instantiate a null class"), an export too small for its prefix
    ("object property prefix outside export"), a malformed option list (the usage text).
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
def f32(v): return struct.pack('<f', v)


class Package:
    """The toy package writer of tests/behavior_test.py."""
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
    """The tag writer of tests/behavior_test.py (names are package-relative)."""
    def __init__(self, p): self.p = p

    def tag(self, name, kind, size, body, detail=b''):
        return self.p.fname(name) + self.p.fname(kind) + w32(size, 0) + detail + body

    def int(self, name, v): return self.tag(name, 'IntProperty', 4, w32(v))
    def float(self, name, v): return self.tag(name, 'FloatProperty', 4, f32(v))
    def bool(self, name, v): return self.tag(name, 'BoolProperty', 0, bytes([1 if v else 0]))
    def name_(self, name, v): return self.tag(name, 'NameProperty', 8, self.p.fname(v))
    def struct_(self, name, type_, body): return self.tag(name, 'StructProperty', len(body), body, self.p.fname(type_))
    def none(self): return self.p.fname('None')


def build():
    """Core.upk: struct Scaled { int Base; int Scale } (default Scale = 1); class Base { int A; float B; bool C;
    Scaled Holder }; class Leaf extends Base { int D; name N }; their default objects; exports Item (prefix 4),
    Comp (prefix 8), Placed (prefix 26) and Tiny (4 bytes), all of class Leaf."""
    p = Package()
    t = Tags(p)
    imp = {n: p.add_import('Class', n) for n in ('Class', 'ScriptStruct', 'IntProperty', 'FloatProperty',
                                                  'BoolProperty', 'NameProperty', 'StructProperty')}
    none = p.fname('None')

    def prop(kind, owner, name, type_ref=0):
        # The 40-byte property header of src/vm.cpp readProperty, then the type reference.
        payload = w32(0) + none + w32(0) + u32(1) + u64(0) + none + w32(0) + w32(type_ref)
        return p.add_export(imp[kind + 'Property'], name, payload, outer=owner)

    def cls(name, super_ref=0): return p.add_export(imp['Class'], name, w32(0) * 4, super_ref=super_ref)

    obj = cls('Object')
    p.add_export(obj, 'Default__Object', w32(0) + t.none())
    # A ScriptStruct export: the 52-byte header (word at +44 zero), then its default tags.
    scaled = p.add_export(imp['ScriptStruct'], 'Scaled', bytes(52) + t.int('Scale', 1) + t.none())
    prop('Int', scaled, 'Scale')
    prop('Int', scaled, 'Base')
    base = cls('Base', obj)
    prop('Struct', base, 'Holder', scaled)
    prop('Bool', base, 'C')
    prop('Float', base, 'B')
    prop('Int', base, 'A')
    leaf = cls('Leaf', base)
    prop('Name', leaf, 'N')
    prop('Int', leaf, 'D')
    p.add_export(base, 'Default__Base', w32(0) + t.int('A', 3) + t.float('B', 1.5) + t.none())
    p.add_export(leaf, 'Default__Leaf', w32(0) + t.int('A', 7) + t.name_('N', 'Foo')
                 + t.struct_('Holder', 'Scaled', t.int('Base', 2) + t.none()) + t.none())
    p.add_export(leaf, 'Item', w32(0) + t.int('D', 2) + t.bool('C', True) + t.none())
    p.add_export(leaf, 'Comp', bytes(8) + t.int('D', 3) + t.none())
    p.add_export(leaf, 'Placed', bytes(26) + t.int('D', 4) + t.float('B', -0.25) + t.none())
    p.add_export(leaf, 'Tiny', w32(0))
    return p


failures = []


def check(label, condition, detail=''):
    if not condition: failures.append(f'{label}: {detail}')


def run(root, *args):
    proc = subprocess.run([reader, str(root / 'Core.upk'), '--object-dump', *args], capture_output=True)
    return proc.returncode, proc.stdout.decode('utf-8').replace('\r\n', '\n'), proc.stderr.decode('utf-8').replace('\r\n', '\n')


def dump(root, index, prefix, everything=True):
    code, out, err = run(root, str(index), str(prefix), '--cooked', str(root), *(['--all'] if everything else []))
    assert code == 0 and out.endswith('}\n') and out.count('\n') == 1, (index, prefix, code, out, err)
    return json.loads(out)


with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    p = build()
    (root / 'Core.upk').write_bytes(p.build())
    index = {p.names[name_ref]: i + 1 for i, (_, _, _, nm, _) in enumerate(p.exports)
             for name_ref in [struct.unpack('<i', nm[:4])[0]]}
    leaf_all = {'a': 7, 'b': 1.5, 'c': False, 'd': 0, 'holder': {'Scale': 1, 'Base': 2}, 'n': 'Foo'}

    # (1) Class default objects: root class first, each default object adding what it changes.
    got = dump(root, index['Default__Base'], 4)
    check('1 base default', got == {'path': 'Default__Base', 'class': 'Core.Base', 'log': [],
                                    'properties': {'a': 3, 'b': 1.5, 'c': False, 'holder': {'Scale': 1, 'Base': 0}}}, got)
    got = dump(root, index['Default__Leaf'], 4)
    check('1 leaf default', got == {'path': 'Default__Leaf', 'class': 'Core.Leaf', 'log': [], 'properties': leaf_all}, got)
    got = dump(root, index['Default__Leaf'], 4, everything=False)
    check('1 a default object differs from itself in nothing', got['properties'] == {}, got)

    # (2) Exports over the class default, after their prefix: 4, 8 and 26 bytes.
    got = dump(root, index['Item'], 4)
    check('2 prefix 4', got == {'path': 'Item', 'class': 'Core.Leaf', 'log': [],
                                'properties': {**leaf_all, 'c': True, 'd': 2}}, got)
    got = dump(root, index['Item'], 4, everything=False)
    check('2 only the changes', got['properties'] == {'c': True, 'd': 2}, got)
    got = dump(root, index['Comp'], 8)
    check('2 prefix 8', got['properties'] == {**leaf_all, 'd': 3}, got)
    got = dump(root, index['Placed'], 26, everything=False)
    check('2 prefix 26', got == {'path': 'Placed', 'class': 'Core.Leaf', 'log': [],
                                 'properties': {'b': -0.25, 'd': 4}}, got)

    # (3) The refusals, their texts and exit code 1.
    cooked = ['--cooked', str(root)]
    for label, args, text in (
            ('index 0', ['0', '4', *cooked], 'ow-package: unsupported exported object request\n'),
            ('prefix 5', [str(index['Item']), '5', *cooked], 'ow-package: unsupported exported object request\n'),
            ('a Class export', [str(index['Leaf']), '4', *cooked], 'ow-package: cannot instantiate a null class\n'),
            ('too small', [str(index['Tiny']), '4', *cooked], 'ow-package: object property prefix outside export\n')):
        code, out, err = run(root, *args)
        check(f'3 {label}', code == 1 and out == '' and err == text, (code, out, err))
    code, out, err = run(root, str(index['Item']), '4', '--cooked')
    check('3 usage', code == 1 and out == '' and err.startswith('ow-package: usage: ow-package <package>'), (code, err))

if failures:
    print('\n'.join(failures))
    sys.exit(1)
print('object dump: class defaults, prefixes 4/8/26, struct defaults and refusals passed.')
