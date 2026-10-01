"""Synthetic checks for research/script_disasm.py. No game files: every byte string is built here.

Run: python tests/script_disasm_test.py
"""
import os
import struct
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "research"))
import script_disasm as sd  # noqa: E402

I32 = lambda v: struct.pack("<i", v)
U16 = lambda v: struct.pack("<H", v)


def decode(script, memory_size=None):
    d = sd.Decoder(script, lambda ref: f"ref{ref}", lambda index, number: f"name{index}",
                   lambda native: f"native{native}")
    return d, d.statements(memory_size)


class Expressions(unittest.TestCase):
    def test_constants_and_return(self):
        _, lines = decode(b"\x04\x1d" + I32(5) + b"\x53")
        self.assertEqual([text for _, text in lines], ["Return(IntConst(5))", "EndOfScript"])

    def test_native_calls_end_at_end_of_function_parms(self):
        _, lines = decode(b"\x70\x26\x25\x16\x53")
        self.assertEqual(lines[0][1], "native112(IntOne, IntZero)")

    def test_extended_native_index_is_two_bytes(self):
        _, lines = decode(b"\x61\x17\x16\x53")  # (0x61 - 0x60) * 256 + 0x17 = 279
        self.assertEqual(lines[0][1], "native279()")

    def test_strings(self):
        _, lines = decode(b"\x1f" + b"hi\0" + b"\x34" + "é".encode("utf-16le") + b"\0\0" + b"\x53")
        self.assertEqual([text for _, text in lines[:2]], ["'hi'", "'é'"])

    def test_unknown_opcode_is_a_decode_error(self):
        with self.assertRaises(sd.DecodeError):
            decode(b"\x2b\x53")


class MemoryOffsets(unittest.TestCase):
    """Object references are 4 bytes in the file but 8 in the in-memory script that jumps count in."""

    def test_reference_adds_four_bytes_of_memory(self):
        script = b"\x04\x00" + I32(1) + b"\x53"  # Return(LocalVariable) EndOfScript: 7 bytes in the file
        d, _ = decode(script, memory_size=11)
        self.assertEqual(d.refs, 1)
        with self.assertRaises(sd.DecodeError):
            decode(script, memory_size=7)

    def test_jump_target_must_be_a_statement_start(self):
        ok = b"\x06" + U16(5) + b"\x04\x25" + b"\x53"  # Jump to the EndOfScript at offset 5
        decode(ok, memory_size=6)
        bad = b"\x06" + U16(4) + b"\x04\x25" + b"\x53"  # offset 4 is inside Return(IntZero)
        with self.assertRaises(sd.DecodeError):
            decode(bad, memory_size=6)

    def test_jump_targets_are_measured_in_memory_offsets(self):
        # Let(LocalVariable, LocalVariable) takes 11 file bytes but 19 memory bytes, so the jump that
        # skips it targets memory offset 3 + 19 = 22, not file offset 14.
        body = b"\x0f\x00" + I32(1) + b"\x00" + I32(2)
        script = b"\x06" + U16(22) + body + b"\x53"
        decode(script, memory_size=len(script) + 8)


class FunctionHeader(unittest.TestCase):
    def package(self, blob):
        pkg = sd.Package.__new__(sd.Package)
        pkg.b = blob
        return pkg

    def export(self, script, locals_entries=2, memory_size=None, net=False):
        head = U16(locals_entries) + U16(0x10) * locals_entries + I32(7) * 10
        head += I32(memory_size if memory_size is not None else len(script)) + I32(len(script))
        tail = U16(0) + b"\x00" + struct.pack("<I", 0x2003 | (0x40 if net else 0)) + (U16(0) if net else b"") + I32(1) + I32(0)
        blob = b"\xaa" * 8 + head + script + tail
        return blob, {"off": 8, "size": len(blob) - 8}

    def test_local_variable_array_shifts_the_script_size_field(self):
        for count in (0, 2, 6):
            blob, e = self.export(b"\x04\x25\x53", locals_entries=count, memory_size=3)
            memory_size, script = self.package(blob).split(e, 0x2003)
            self.assertEqual((memory_size, script), (3, b"\x04\x25\x53"))

    def test_net_functions_have_a_longer_tail(self):
        blob, e = self.export(b"\x04\x25\x53", net=True)
        self.assertEqual(self.package(blob).split(e, 0x2043)[1], b"\x04\x25\x53")

    def test_script_size_must_end_exactly_at_the_tail(self):
        blob, e = self.export(b"\x04\x25\x53")
        e["size"] += 2
        with self.assertRaises(sd.DecodeError):
            self.package(blob + b"\0\0").split(e, 0x2003)


if __name__ == "__main__":
    unittest.main(verbosity=2)
