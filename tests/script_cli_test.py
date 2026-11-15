"""AI-assisted (Claude Code). Synthetic coverage of the script commands of `ow-package`: `--script-check`,
`--script-check --failures` and `--disasm <index|Class.Function>`.

    python tests/script_cli_test.py <ow-package binary>

Runs on the C++ binary in ctest (`script-cli-synthetic`) and on the Rust binary through `tests/rust_parity.d/script.txt`.
The package is invented (AGENTS.md rule 9): a toy `Core` with one class `Foo` and five functions, built with the package writer
of `tests/vm_test.py` and the function payload of `Toy.function` in `tests/mission_script_test.py` (no child properties, since
none of the bodies names one). The function layout is the one recovered from the real packages
(docs/verification/SCRIPT_BYTECODE_DISASM.md). The bodies:

  Answer   return 42;                      04 | 1D 2A000000 | 53
  Nothing  return;                         04 0B | 53
  Skip     if (true) return; (jump lands)  07 0600 27 | 04 0B | 53       the JumpIfNot target 6 is the EndOfScript statement
  Native   a native function, no script    (counted as native, not decoded)
  Broken   a jump into an instruction      06 0100 | 04 0B | 53          target 1 is inside the jump, not a statement start

Line ends are compared after reading CRLF as LF: the C++ binary's text-mode streams print CRLF on Windows (decision 2026-11-15-09).
"""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

reader = str(Path(sys.argv[1]).resolve())


def w32(*values): return b''.join(struct.pack('<i', v) for v in values)
def u16(v): return struct.pack('<H', v)
def u32(v): return struct.pack('<I', v)


class Package:
    """The toy package writer of tests/vm_test.py."""
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


FUNC_NATIVE, FUNC_FINAL, FUNC_DEFINED, FUNC_PUBLIC = 0x400, 0x1, 0x2, 0x20000
SCRIPTED = FUNC_FINAL | FUNC_DEFINED | FUNC_PUBLIC


def function_payload(p, name, script, refs=0, flags=SCRIPTED, native=0):
    """Toy.function's payload: no local-array entries, ten words, in-memory and stored size, script, then the tail
    [iNative u16][OperPrecedence u8][FunctionFlags u32][FriendlyName FName]."""
    payload = u16(0) + w32(*([0] * 10)) + w32(len(script) + 4 * refs, len(script)) + script
    return payload + u16(native) + bytes([0]) + u32(flags) + p.fname(name)


def make():
    p = Package()
    cls = p.add_import('Class', 'Class')
    fn = p.add_import('Class', 'Function')
    foo = p.add_export(cls, 'Foo', w32(0) * 4)                                       # 1
    bodies = [
        ('Answer', bytes([0x04, 0x1D]) + w32(42) + bytes([0x53]), SCRIPTED),        # 2
        ('Nothing', bytes([0x04, 0x0B, 0x53]), SCRIPTED),                            # 3
        ('Skip', bytes([0x07]) + u16(6) + bytes([0x27, 0x04, 0x0B, 0x53]), SCRIPTED),  # 4
        ('Native', b'', FUNC_NATIVE | FUNC_FINAL | FUNC_PUBLIC),                      # 5
        ('Broken', bytes([0x06]) + u16(1) + bytes([0x04, 0x0B, 0x53]), SCRIPTED),   # 6
    ]
    for name, script, flags in bodies:
        p.add_export(fn, name, function_payload(p, name, script, flags=flags), outer=foo)
    return p.build()


def run(*args):
    out = subprocess.run([reader, *map(str, args)], capture_output=True)
    return out.returncode, out.stdout.decode('utf-8').replace('\r\n', '\n'), out.stderr.decode('utf-8').replace('\r\n', '\n')


class ScriptCli(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.folder = tempfile.TemporaryDirectory()
        cls.path = Path(cls.folder.name) / 'Core.upk'
        cls.path.write_bytes(make())

    @classmethod
    def tearDownClass(cls):
        cls.folder.cleanup()

    def test_script_check_counts(self):
        code, out, err = run(self.path, '--script-check')
        self.assertEqual((code, err), (0, ''))
        self.assertEqual(out, '{"package":"Core","functions":5,"native":1,"script":4,"decoded":3,"failed":1}\n')

    def test_script_check_lists_the_failing_function(self):
        code, out, err = run(self.path, '--script-check', '--failures')
        self.assertEqual((code, err), (0, ''))
        summary = json.loads(out)
        self.assertEqual({k: summary[k] for k in ('functions', 'native', 'script', 'decoded', 'failed')},
                         {'functions': 5, 'native': 1, 'script': 4, 'decoded': 3, 'failed': 1})
        self.assertEqual(len(summary['failures']), 1)
        self.assertTrue(summary['failures'][0].startswith('Foo.Broken: '), summary['failures'])
        self.assertEqual(out, '{"package":"Core","functions":5,"native":1,"script":4,"decoded":3,"failed":1,'
                              '"failures":["Foo.Broken: ' + BROKEN + '"]}\n')

    def test_disasm_by_path_and_by_index(self):
        expected = {
            'Foo.Answer': (2, ANSWER),
            'Foo.Nothing': (3, NOTHING),
            'Foo.Skip': (4, SKIP),
        }
        for target, (index, text) in expected.items():
            for argument in (target, str(index)):
                with self.subTest(argument=argument):
                    self.assertEqual(run(self.path, '--disasm', argument), (0, text, ''))

    def test_errors(self):
        # A name that matches no export.
        self.assertEqual(run(self.path, '--disasm', 'Foo.Missing'), (1, '', 'ow-package: function not found: Foo.Missing\n'))
        # The function --script-check reports as failing does not disassemble either.
        self.assertEqual(run(self.path, '--disasm', 'Foo.Broken'), (1, '', 'ow-package: ' + BROKEN + '\n'))
        # --failures is the only option --script-check takes.
        code, out, err = run(self.path, '--script-check', '--all')
        self.assertEqual((code, out), (1, ''))
        self.assertTrue(err.startswith('ow-package: usage: ow-package <package> '), err)


# What the decoder prints, worked out from the bodies above: statement offsets count each byte once (no object references here),
# so Answer's EndOfScript sits after 1 (Return) + 1 (IntConst) + 4 (the value) bytes. The flags are SCRIPTED (0x20003).
ANSWER = '// Foo.Answer  flags 0x20003\n0: Return(IntConst(42))\n6: EndOfScript\n'
NOTHING = '// Foo.Nothing  flags 0x20003\n0: Return(Nothing)\n2: EndOfScript\n'
SKIP = '// Foo.Skip  flags 0x20003\n0: JumpIfNot(6, True)\n4: Return(Nothing)\n6: EndOfScript\n'
BROKEN = 'jump target 1 is not a statement start'

if __name__ == '__main__':
    unittest.main(argv=sys.argv[:1], verbosity=2)
