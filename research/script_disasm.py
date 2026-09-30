"""
script_disasm.py: read-only UnrealScript bytecode disassembler for the nine BL2 code packages.

Phase 2, step 2 ("bytecode loader") prototype, in the same spirit as native_count.py: a Python
oracle that is easy to change while the layout is still being established. Nothing is executed.
Output is pseudo-code for reading only; generated listings belong under the ignored local/.

How a function's bytes are laid out, as RECOVERED FROM THE PACKAGES (2026-09-30), not from any
source: the export data of a script UFunction is

    u16 N                       Gearbox local-variable array: N entries of u16 (the quirk flagged in
                                ENGINE_PLAN.md; what the entries mean is not needed here)
    u16 * N
    i32 * 10                    object references and line/position fields (meaning UNVERIFIED)
    i32 ScriptBytecodeSize      size of the script IN MEMORY, where every object reference is 8 bytes
                                (Jump/Case/Skip operands are offsets into that in-memory form)
    i32 ScriptSize              size in the file
    u8  Script[ScriptSize]      ends with EX_EndOfScript (0x53)
    function tail               iNative u16, OperPrecedence u8, FunctionFlags u32,
                                [RepOffset u16 if FUNC_Net], FriendlyName FName (8 bytes)

The check that makes this trustworthy is structural, not a spot check: for every non-native
function in a package, (header end + ScriptSize) must equal the tail start exactly, the last script
byte must be 0x53, the expression grammar below must consume exactly ScriptSize bytes, and every
Jump/JumpIfNot/Case target must be the start of a statement. A wrong operand size desynchronises
the stream and fails one of those within a few instructions.

The opcode table is the public UE3 expression-token set written from the general knowledge of that
format, NOT copied from UE Explorer or any GPL tool. Each opcode's operand layout is UNVERIFIED
until `--check` reports zero failures for that opcode; the report lists what failed and why.

Usage:
    python research/script_disasm.py --check [Package.upk ...]     # structural validation
    python research/script_disasm.py Package.upk Class.Function    # disassemble one function
Set OPENWILLOW_BL2 (game folder) or pass --game <CookedPCConsole dir>.
"""
import argparse
import collections
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import native_count as nc  # the package reader this prototype builds on

EX_END_OF_SCRIPT = 0x53
ALL_PACKAGES = ["Core.upk", "Engine.upk", "GameFramework.upk", "GearboxFramework.upk", "WillowGame.upk",
                "GFxUI.upk", "IpDrv.upk", "OnlineSubsystemSteamworks.upk", "AkAudio.upk"]


class DecodeError(Exception):
    pass


# ---------------------------------------------------------------- opcode operand layouts
# Operand letters: r = object reference (i32), n = FName (two i32), i = i32, f = f32, b = u8, w = u16.
# Sub-expressions are written as 'E'. A trailing 'P' means "call parameters: expressions until 0x16".
# Anything needing logic (strings, label table, conditionals) is handled in Decoder.expression().
SIMPLE = {
    0x00: ("LocalVariable", "r"), 0x01: ("InstanceVariable", "r"), 0x02: ("DefaultVariable", "r"),
    0x03: ("StateVariable", "r"), 0x48: ("LocalOutVariable", "r"),
    0x04: ("Return", "E"), 0x05: ("Switch", "rbE"),  # property reference of the switched value, u8, then the value (trial-fitted)
    0x06: ("Jump", "w"), 0x07: ("JumpIfNot", "wE"), 0x08: ("Stop", ""), 0x09: ("Assert", "wbE"),
    0x0B: ("Nothing", ""), 0x0D: ("GotoLabel", "E"), 0x0E: ("EatReturnValue", "r"),
    0x0F: ("Let", "EE"), 0x10: ("DynArrayElement", "EE"), 0x11: ("New", "EEEE"),
    0x13: ("MetaCast", "rE"), 0x14: ("LetBool", "EE"), 0x15: ("EndParmValue", ""),
    0x16: ("EndFunctionParms", ""), 0x17: ("Self", ""), 0x18: ("Skip", "wE"),
    0x1A: ("ArrayElement", "EE"), 0x1D: ("IntConst", "i"), 0x1E: ("FloatConst", "f"),
    0x20: ("ObjectConst", "r"), 0x21: ("NameConst", "n"), 0x22: ("RotationConst", "iii"),
    0x23: ("VectorConst", "fff"), 0x24: ("ByteConst", "b"), 0x25: ("IntZero", ""), 0x26: ("IntOne", ""),
    0x27: ("True", ""), 0x28: ("False", ""), 0x29: ("NativeParm", "r"), 0x2A: ("NoObject", ""),
    0x2C: ("IntConstByte", "b"), 0x2D: ("BoolVariable", "E"), 0x2E: ("DynamicCast", "rE"),
    0x2F: ("Iterator", "Ew"), 0x30: ("IteratorPop", ""), 0x31: ("IteratorNext", ""),
    0x32: ("StructCmpEq", "rEE"), 0x33: ("StructCmpNe", "rEE"), 0x35: ("StructMember", "rrbbE"),
    0x36: ("DynArrayLength", "E"), 0x38: ("PrimitiveCast", "bE"), 0x39: ("DynArrayInsert", "EEE"),
    0x3A: ("ReturnNothing", "r"), 0x3B: ("EqualEqual_DelDel", "EE"), 0x3C: ("NotEqual_DelDel", "EE"),
    0x3D: ("EqualEqual_DelFunc", "EE"), 0x3E: ("NotEqual_DelFunc", "EE"), 0x3F: ("EmptyDelegate", ""),
    0x40: ("DynArrayRemove", "EEE"), 0x41: ("DebugInfo", "iiib"), 0x43: ("DelegateProperty", "nr"),
    0x44: ("LetDelegate", "EE"), 0x46: ("DynArrayFind", "EwE"), 0x47: ("DynArrayFindStruct", "EwP"),
    0x49: ("DefaultParmValue", "wE"), 0x4A: ("EmptyParmValue", ""), 0x4B: ("InstanceDelegate", "n"),
    0x51: ("InterfaceContext", "E"), 0x52: ("InterfaceCast", "rE"), 0x53: ("EndOfScript", ""),
    0x54: ("DynArrayAdd", "EE"), 0x55: ("DynArrayAddItem", "EwP"), 0x56: ("DynArrayRemoveItem", "EwP"),
    0x57: ("DynArrayInsertItem", "EEE"), 0x58: ("DynArrayIterator", "EEbEw"), 0x59: ("DynArraySort", "EE"),
    0x5A: ("FilterEditorOnly", "w"),
    # Not in the set the format's public descriptions list; present in this build. Each carries one i32 that is
    # NOT an object reference (a least-squares fit of the header's in-memory size against reference counts,
    # local/phase2/regress.py, gives exactly +0 bytes per reference opcode and +4 for these). Meaning UNVERIFIED.
    0x5E: ("Op5E", "r"), 0x5F: ("Op5F", "EE"),  # 0x5F sits where Let does (lhs, rhs): a typed Let, UNVERIFIED; 0x5E: one object reference (fits 94 of the 100 functions that use it)
    0x4C: ("Op4C", "i"), 0x4D: ("Op4D", "i"), 0x4E: ("Op4E", "i"), 0x4F: ("Op4F", "i"), 0x50: ("Op50", "i"),
    # Calls: a target, then parameter expressions up to EndFunctionParms (0x16).
    0x1B: ("VirtualFunction", "nP"), 0x1C: ("FinalFunction", "rP"), 0x37: ("GlobalFunction", "nP"),
    0x42: ("DelegateFunction", "brnP"),
    # Context: object expression, u16 skip, property reference, u8 size, then the member expression.
    0x19: ("Context", "EwrbE"), 0x12: ("ClassContext", "EwrbE"),
}


class Decoder:
    def __init__(self, data, resolve_ref, resolve_name, native_name):
        self.d = data
        self.p = 0
        self.ref = resolve_ref
        self.name = resolve_name
        self.native = native_name
        self.refs = 0  # object references seen so far: 4 bytes in the file, 8 in the in-memory script
        self.statement_starts = set()  # in-memory offsets (what Jump/Case/Skip operands are measured in)
        self.jump_targets = []
        self.ops = collections.Counter()

    def take(self, n):
        if self.p + n > len(self.d):
            raise DecodeError(f"read past end at {self.p}")
        v = self.d[self.p:self.p + n]
        self.p += n
        return v

    def u8(self): return self.take(1)[0]
    def u16(self): return struct.unpack("<H", self.take(2))[0]
    def i32(self): return struct.unpack("<i", self.take(4))[0]
    def f32(self): return struct.unpack("<f", self.take(4))[0]
    def fname(self): return self.name(self.i32(), self.i32())

    def params(self):
        out = []
        while True:
            if self.p >= len(self.d):
                raise DecodeError("call parameters run past the script end")
            if self.d[self.p] == 0x16:
                self.p += 1
                return out
            out.append(self.expression())

    def operand(self, letter, op_name):
        if letter == "r":
            self.refs += 1
            return self.ref(self.i32())
        if letter == "n": return self.fname()
        if letter == "i": return str(self.i32())
        if letter == "f": return repr(round(self.f32(), 6))
        if letter == "b": return str(self.u8())
        if letter == "w": return str(self.u16())
        if letter == "E": return self.expression()
        raise DecodeError(f"bad operand letter {letter}")

    def expression(self):
        start = self.p
        op = self.u8()
        self.ops[op] += 1
        if op >= 0x70:
            return f"{self.native(op)}({', '.join(self.params())})"
        if 0x60 <= op < 0x70:
            index = (op - 0x60) * 256 + self.u8()
            return f"{self.native(index)}({', '.join(self.params())})"
        if op == 0x1F:  # StringConst: ASCII, NUL-terminated
            end = self.d.index(b"\0", self.p)
            text = self.d[self.p:end].decode("latin-1")
            self.p = end + 1
            return repr(text)
        if op == 0x34:  # UnicodeStringConst: UTF-16, NUL-terminated
            chars = []
            while True:
                c = self.u16()
                if c == 0: break
                chars.append(chr(c))
            return repr("".join(chars))
        if op == 0x0C:  # LabelTable: (FName, u32 offset) pairs until the name "None"
            labels = []
            while True:
                n = self.fname()
                offset = self.i32()
                if n == "None": break
                labels.append(f"{n}@{offset}")
                if len(labels) > 4096: raise DecodeError("runaway label table")
            return f"LabelTable[{', '.join(labels)}]"
        if op == 0x0A:  # Case: u16 next-case offset (0xFFFF = default), then the value expression
            offset = self.u16()
            if offset == 0xFFFF: return "Default"
            self.jump_targets.append(offset)
            return f"Case[{offset}] {self.expression()}"
        if op == 0x45:  # Conditional: cond, u16 skip, true-expr, u16 skip, false-expr
            cond = self.expression(); self.u16()
            a = self.expression(); self.u16()
            return f"({cond} ? {a} : {self.expression()})"
        entry = SIMPLE.get(op)
        if entry is None:
            raise DecodeError(f"unknown opcode 0x{op:02x} at {start}")
        name, layout = entry
        parts = []
        for letter in layout:
            if letter == "P":
                parts.append("(" + ", ".join(self.params()) + ")")
            else:
                value = self.operand(letter, name)
                if name in ("Jump", "JumpIfNot") and letter == "w":
                    self.jump_targets.append(int(value))
                parts.append(value)
        return f"{name}({', '.join(parts)})" if parts else name

    def memory_offset(self, file_offset=None):
        return (self.p if file_offset is None else file_offset) + 4 * self.refs

    def statements(self, memory_size=None):
        lines = []
        while self.p < len(self.d):
            start = self.p
            self.statement_starts.add(self.memory_offset())
            text = self.expression()
            lines.append((start, text))
            if self.d[start] == EX_END_OF_SCRIPT:
                break
        if self.p != len(self.d):
            raise DecodeError(f"stopped at {self.p} of {len(self.d)}")
        self.statement_starts.add(self.memory_offset())
        if memory_size is not None and self.memory_offset() != memory_size:
            raise DecodeError(f"memory size {self.memory_offset()} != header ScriptBytecodeSize {memory_size}")
        for target in self.jump_targets:
            if target not in self.statement_starts:
                raise DecodeError(f"jump target {target} is not a statement start")
        return lines


# ---------------------------------------------------------------- package glue
class Package:
    def __init__(self, path):
        self.name = os.path.basename(path)
        self.b = nc.unwrap_fully_compressed(path)
        self.ver, self.lic, self.names, self.imports, self.exports = nc.parse_package(self.b)
        self.function_import = next((-(i + 1) for i, im in enumerate(self.imports)
                                     if im["name"] == "Function" and im["class"] == "Class"), None)

    def qualified(self, index):
        parts = []
        if not -len(self.imports) <= index <= len(self.exports):
            raise DecodeError(f"object reference {index} out of range")
        while index != 0 and len(parts) < 8:
            if index > 0:
                e = self.exports[index - 1]; parts.append(e["name"]); index = e["outer"]
            else:
                e = self.imports[-index - 1]; parts.append(e["name"]); index = e["outer"]
        return ".".join(reversed(parts)) if parts else "None"

    def fname(self, index, number):
        if not 0 <= index < len(self.names): raise DecodeError(f"FName index {index} out of range")
        return self.names[index] + (f"_{number - 1}" if number else "")

    def functions(self):
        """Yield (export index, export, flags) for every function export with readable flags."""
        for i, e in enumerate(self.exports):
            if e["class"] != self.function_import: continue
            flags = nc.read_function_flags(self.b, e, self.names)
            if flags is not None: yield i + 1, e, flags

    def split(self, e, flags):
        """Return (ScriptBytecodeSize, script bytes) or raise DecodeError. Checked against the tail."""
        start, end = e["off"], e["off"] + e["size"]
        tail = 15 + (2 if flags & nc.FUNC_NET else 0)
        tail_start = end - tail
        if tail_start <= start + 2: raise DecodeError("export too small")
        count = struct.unpack_from("<H", self.b, start)[0]
        size_at = start + 2 + 2 * count + 44
        if size_at + 4 > tail_start: raise DecodeError("header runs into the tail")
        size = struct.unpack_from("<i", self.b, size_at)[0]
        if size < 1 or size_at + 4 + size != tail_start:
            raise DecodeError(f"ScriptSize {size} does not end at the tail (gap {tail_start - (size_at + 4 + size)})")
        memory_size = struct.unpack_from("<i", self.b, size_at - 4)[0]
        return memory_size, self.b[size_at + 4:tail_start]

    def native_table(self):
        table = {}
        for _, e, flags in self.functions():
            if flags & nc.FUNC_NATIVE:
                end = e["off"] + e["size"]
                tail = 15 + (2 if flags & nc.FUNC_NET else 0)
                index = struct.unpack_from("<H", self.b, end - tail)[0]
                if index: table[index] = e["name"]
        return table


def decode_function(pkg, e, flags, natives):
    memory_size, script = pkg.split(e, flags)
    d = Decoder(script, pkg.qualified, pkg.fname, lambda n: natives.get(n, f"native_{n}"))
    lines = d.statements(memory_size)
    if script[-1] != EX_END_OF_SCRIPT: raise DecodeError("last byte is not EndOfScript")
    return d, lines


def check(pkg, natives, show):
    total = ok = 0
    reasons = collections.Counter()
    samples = {}
    ops = collections.Counter()
    for _, e, flags in pkg.functions():
        if flags & nc.FUNC_NATIVE: continue
        total += 1
        try:
            d, _ = decode_function(pkg, e, flags, natives)
            ok += 1
            ops.update(d.ops)
        except (DecodeError, struct.error, ValueError, IndexError) as ex:
            key = str(ex).split(" at ")[0][:60]
            reasons[key] += 1
            samples.setdefault(key, f"{pkg.qualified(e['outer'])}.{e['name']}: {ex}")
    print(f"{pkg.name:<32}{total:>8} script functions{ok:>8} decode exactly{total - ok:>7} fail")
    for reason, n in reasons.most_common(show):
        print(f"    {n:>5}  {reason}   e.g. {samples[reason]}")
    return total, ok, ops


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--game", default=None)
    parser.add_argument("--show", type=int, default=6, help="failure reasons listed per package")
    parser.add_argument("items", nargs="*")
    args = parser.parse_args()
    game = args.game or (os.path.join(os.environ["OPENWILLOW_BL2"], "WillowGame", "CookedPCConsole")
                         if os.environ.get("OPENWILLOW_BL2") else nc.GAME)
    if args.check:
        names = args.items or ALL_PACKAGES
        native = {}
        pkgs = [Package(os.path.join(game, n)) for n in names]
        for p in pkgs: native.update(p.native_table())
        grand = collections.Counter(); opcount = collections.Counter()
        for p in pkgs:
            t, o, ops = check(p, native, args.show)
            grand["total"] += t; grand["ok"] += o; opcount.update(ops)
        print(f"{'TOTAL':<32}{grand['total']:>8} script functions{grand['ok']:>8} decode exactly{grand['total'] - grand['ok']:>7} fail")
        print("opcodes seen:", ", ".join(f"0x{k:02x}:{v}" for k, v in sorted(opcount.items()) if k < 0x60))
        return 0 if grand["total"] == grand["ok"] else 1
    if len(args.items) != 2:
        parser.error("give Package.upk and Function (Class.Function or bare name), or --check")
    pkg = Package(os.path.join(game, args.items[0]))
    natives = pkg.native_table()
    for index, e, flags in pkg.functions():
        full = f"{pkg.qualified(e['outer'])}.{e['name']}"
        if args.items[1] in (full, e["name"]) and not flags & nc.FUNC_NATIVE:
            d, lines = decode_function(pkg, e, flags, natives)
            print(f"// {full}  flags 0x{flags:x}")
            for offset, text in lines: print(f"{offset:5}: {text}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
