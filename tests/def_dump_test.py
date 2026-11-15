"""AI-assisted (Claude Code). Synthetic `ow-package --def-dump` coverage (lane W3-DEFS, Rust-only): builds a toy `Core`
package with the package writer and tag helpers of tests/object_dump_test.py, runs `--def-dump` with and without
`--overlay` (the plain test format and a live capture in the shape tools/real_game/scripts/weapon_dump.py writes), and
checks the output against values written by hand.

Usage: `def_dump_test.py <ow-package>`, the Rust binary (the C++ one has no `--def-dump`). ctest runs it as
`def-dump-synthetic` on the Rust release binary; tests/rust_parity.d/defs.txt runs it through the parity runner.

Nothing here comes from the game: every class, property, object and value is invented.

Pinned here (the definition store, decision 2026-11-15-16; UNVERIFIED in the running game):
  * a definition prints as `--object-dump --all` prints an export: every property, keys lower-cased in byte order,
    references by their paths; the definition starts from its class default object;
  * a reference inside a definition is the store's object for that path and prints by its path;
  * a text overlay patches a property before the definition loads; a patch that does not type is reported on stderr
    and leaves the cooked value;
  * a live capture sets values, an object path written `Class'path'` names the object, and a struct starts from its
    defaults with the listed fields set;
  * the refusals: a path no file holds, a code export, a bad command line.
"""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

reader = str(Path(sys.argv[1]).resolve())
assert len(sys.argv) == 2, 'usage: def_dump_test.py <ow-package>'


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
    def obj(self, name, ref): return self.tag(name, 'ObjectProperty', 4, w32(ref))
    def none(self): return self.p.fname('None')


def build():
    """Core.upk: struct Scaled { int Base; int Scale } (default Scale = 1); class Thing { int Count; float Rate; Thing
    Next; Scaled Holder }; Default__Thing (Count 1); definitions Item (Count 5, Next -> Other) and Other (Rate 2)."""
    p = Package()
    t = Tags(p)
    imp = {n: p.add_import('Class', n) for n in ('Class', 'ScriptStruct', 'IntProperty', 'FloatProperty',
                                                  'ObjectProperty', 'StructProperty')}
    none = p.fname('None')

    def prop(kind, owner, name, type_ref=0):
        # The 40-byte property header of src/vm.cpp readProperty, then the type reference.
        payload = w32(0) + none + w32(0) + u32(1) + u64(0) + none + w32(0) + w32(type_ref)
        return p.add_export(imp[kind + 'Property'], name, payload, outer=owner)

    def cls(name, super_ref=0): return p.add_export(imp['Class'], name, w32(0) * 4, super_ref=super_ref)

    obj = cls('Object')
    p.add_export(obj, 'Default__Object', w32(0) + t.none())
    scaled = p.add_export(imp['ScriptStruct'], 'Scaled', bytes(52) + t.int('Scale', 1) + t.none())
    prop('Int', scaled, 'Scale')
    prop('Int', scaled, 'Base')
    thing = cls('Thing', obj)
    prop('Struct', thing, 'Holder', scaled)
    prop('Object', thing, 'Next', thing)
    prop('Float', thing, 'Rate')
    prop('Int', thing, 'Count')
    p.add_export(thing, 'Default__Thing', w32(0) + t.int('Count', 1) + t.none())
    other = len(p.exports) + 2
    p.add_export(thing, 'Item', w32(0) + t.int('Count', 5) + t.obj('Next', other) + t.none())
    assert p.add_export(thing, 'Other', w32(0) + t.float('Rate', 2.0) + t.none()) == other
    return p


failures = []


def check(label, condition, detail=''):
    if not condition: failures.append(f'{label}: {detail}')


def run(*args):
    proc = subprocess.run([reader, '--def-dump', *args], capture_output=True)
    return proc.returncode, proc.stdout.decode('utf-8').replace('\r\n', '\n'), proc.stderr.decode('utf-8').replace('\r\n', '\n')


def expect(label, got, properties):
    code, out, err = got
    want = {'path': label, 'class': 'Core.Thing', 'properties': properties, 'log': []}
    check(label, code == 0 and out.endswith('}\n') and out.count('\n') == 1 and json.loads(out) == want, (code, out, err))
    return err


with tempfile.TemporaryDirectory() as folder:
    root = Path(folder)
    (root / 'Core.upk').write_bytes(build().build())
    cooked = ['--cooked', str(root)]
    holder = {'Base': 0, 'Scale': 1}

    # (1) No overlay: the class default first (Count 1, Holder's default Scale), then the definition's own tags.
    expect('Item', run('Core.Item', *cooked), {'count': 5, 'holder': holder, 'next': 'Other', 'rate': 0})
    expect('Other', run('Core.Other', *cooked), {'count': 1, 'holder': holder, 'next': None, 'rate': 2})
    # The printed text, byte for byte (the --object-dump format).
    code, out, _ = run('Core.Item', *cooked)
    check('1 text', out == '{"path":"Item","class":"Core.Thing","properties":{"count":5,"holder":{"Base":0,"Scale":1},'
          '"next":"Other","rate":0},"log":[]}\n', out)

    # (2) A text overlay: one patch applies, one does not type and is reported; the cooked value stays there.
    text = root / 'patch.txt'
    text.write_text('# invented\nCore.Item Count = 8\nCore.Item Holder.Base = 3\nCore.Item Count = "eight"\n',
                    encoding='utf-8')
    err = expect('Item', run('Core.Item', *cooked, '--overlay', str(text)),
                 {'count': 8, 'holder': {'Base': 3, 'Scale': 1}, 'next': 'Other', 'rate': 0})
    check('2 report', 'defs: overlay 0: 2 applied, 0 pending, 1 failed\n' in err, err)
    check('2 failed patch', 'defs: overlay 0 patch 2: patch Core.Item Count:' in err, err)

    # (3) A live capture (weapon_dump.py's shape) after the text overlay: the later overlay wins.
    live = root / 'live_weapon_data.json'
    live.write_text(json.dumps({'Core.Other': {'class': 'Thing', 'properties': {
        'Next': "Thing'Core.Item'", 'Rate': 0.5, 'Holder': {'Base': 4, 'Scale': None}}},
        'Core.Item': {'class': 'Thing', 'properties': {'Count': 9}}}), encoding='utf-8')
    expect('Other', run('Core.Other', *cooked, '--overlay', str(text), '--overlay', str(live)),
           {'count': 1, 'holder': {'Base': 4, 'Scale': 1}, 'next': 'Item', 'rate': 0.5})
    expect('Item', run('Core.Item', *cooked, '--overlay', str(text), '--overlay', str(live)),
           {'count': 9, 'holder': {'Base': 3, 'Scale': 1}, 'next': 'Other', 'rate': 0})

    # (4) The refusals: messages on stderr after `ow-package: `, exit code 1, nothing on stdout.
    for label, args, text_ in (
            ('missing', ['Core.Nothing', *cooked], 'ow-package: object not found: Core.Nothing\n'),
            ('code', ['Core.Thing', *cooked], 'ow-package: not a definition: Core.Thing: a code export\n'),
            ('usage', ['Core.Item'], 'ow-package: usage: ow-package --def-dump <object path> --cooked <dir> '
                                     '[--overlay <file>]...\n')):
        code, out, err = run(*args)
        check(f'4 {label}', code == 1 and out == '' and err == text_, (code, out, err))

if failures:
    print('\n'.join(failures))
    sys.exit(1)
print('def dump: a definition, references by path, a text overlay, a live capture and refusals passed.')
