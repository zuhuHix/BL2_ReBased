"""
native_count.py — measure native vs script functions in Borderlands 2 code packages.

Reads fully-compressed UE3 packages straight off disk:
  1. unwraps the UE3 compressed-chunk container
  2. LZO1X-decompresses each block (pure Python, no deps; independent
     implementation, provenance in THIRD_PARTY.md)
  3. parses the package name/import/export tables (package version 832)
  4. finds every UFunction export and reads its FunctionFlags from the END of
     the object (robust to licensee-specific fields at the front)

FUNC_Native (0x400) = body lived in C++ inside Borderlands2.exe -> must be rewritten.
Anything else = UnrealScript bytecode present in the file -> inherited as-is.
"""
import struct, sys, os, math, collections, time

GAME = r"C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2\WillowGame\CookedPCConsole"

# ---------------------------------------------------------------- LZO1X decompress
class LzoError(ValueError):
    pass


def lzo1x_decompress(src, dst_len):
    """Decode one LZO1X stream into exactly dst_len bytes.

    Independent implementation of the LZO1X instruction format, written from
    the public format description and checked against lzokay (MIT, Jack
    Andersen) for instruction semantics. No code from any LZO implementation
    is copied or translated. The stream is a sequence of instructions; the
    first byte is special, and how a low opcode (< 16) is interpreted depends
    on how many literals the previous instruction ended with (`state`).
    Every read and write is bounds-checked and raises LzoError.
    """
    src = memoryview(src)
    n_in = len(src)
    out = bytearray(dst_len)
    ip = 0
    op = 0

    def take():
        nonlocal ip
        if ip >= n_in:
            raise LzoError("input overrun")
        v = src[ip]
        ip += 1
        return v

    def literals(count):
        nonlocal ip, op
        if ip + count > n_in:
            raise LzoError("input overrun in literal run")
        if op + count > dst_len:
            raise LzoError("output overrun in literal run")
        out[op:op + count] = src[ip:ip + count]
        ip += count
        op += count

    def match(distance, length):
        nonlocal op
        if distance <= 0 or distance > op:
            raise LzoError("match distance outside decoded output")
        if op + length > dst_len:
            raise LzoError("output overrun in match")
        start = op - distance
        if distance >= length:
            out[op:op + length] = out[start:start + length]
            op += length
        else:
            for _ in range(length):  # overlapping copy repeats the pattern
                out[op] = out[op - distance]
                op += 1

    def extended(base, first_zero_byte_value):
        # Run-length extension: each 0x00 adds 255, the terminating byte adds itself.
        length = base
        while True:
            b = take()
            if b != 0:
                return length + first_zero_byte_value + b
            length += 255

    # First byte: 18..255 means (byte - 17) literals precede the first instruction.
    first = take()
    if first > 17:
        count = first - 17
        literals(count)
        state = count if count < 4 else 4
    else:
        ip -= 1
        state = 0

    while True:
        t = take()
        if t < 16:
            if state == 0:
                # Literal run of (t + 3) bytes; t == 0 selects the extended form.
                count = extended(3, 15) if t == 0 else t + 3
                literals(count)
                state = 4
                continue
            h = take()
            if state == 4:
                # After a long literal run: 3-byte match, distance 2049..3072.
                match((h << 2) + (t >> 2) + 2049, 3)
            else:
                # After 1..3 literals: 2-byte match, distance 1..1024.
                match((h << 2) + (t >> 2) + 1, 2)
            trailing = t & 3
        elif t >= 64:
            # 1-byte header + 1 distance byte: length 3..8, distance 1..2048.
            h = take()
            match((h << 3) + ((t >> 2) & 7) + 1, (t >> 5) + 1)
            trailing = t & 3
        elif t >= 32:
            # Length in low 5 bits (extended when zero), then 16-bit LE distance word.
            length = extended(2, 31) if (t & 31) == 0 else (t & 31) + 2
            lo = take(); hi = take()
            word = lo | (hi << 8)
            match((word >> 2) + 1, length)
            trailing = word & 3
        else:
            # 16..31: long distance. Bit 3 adds 16384; low 3 bits are the length (extended when zero).
            high = (t & 8) << 11
            length = extended(2, 7) if (t & 7) == 0 else (t & 7) + 2
            lo = take(); hi = take()
            word = lo | (hi << 8)
            distance = high + (word >> 2)
            if distance == 0:
                break  # end-of-stream marker
            match(distance + 16384, length)
            trailing = word & 3
        literals(trailing)
        state = trailing

    if ip != n_in:
        raise LzoError(f"{n_in - ip} trailing input bytes after end marker")
    if op != dst_len:
        raise LzoError(f"decoded {op} bytes, expected {dst_len}")
    return bytes(out)

# ---------------------------------------------------------------- container
def unwrap_fully_compressed(path):
    data = open(path, "rb").read()
    magic, block, csize, usize = struct.unpack_from("<IIII", data, 0)
    assert magic == 0x9E2A83C1, "not a UE3 package"
    nblocks = math.ceil(usize / block)
    table = struct.unpack_from("<" + "II"*nblocks, data, 16)
    pos = 16 + 8*nblocks
    out = bytearray()
    for i in range(nblocks):
        bc, bu = table[2*i], table[2*i+1]
        out += lzo1x_decompress(data[pos:pos+bc], bu)
        pos += bc
    assert len(out) == usize, f"size mismatch {len(out)} != {usize}"
    return bytes(out)

# ---------------------------------------------------------------- package parse
class Reader:
    def __init__(s, b): s.b = b; s.p = 0
    def i32(s): v = struct.unpack_from("<i", s.b, s.p)[0]; s.p += 4; return v
    def u32(s): v = struct.unpack_from("<I", s.b, s.p)[0]; s.p += 4; return v
    def u16(s): v = struct.unpack_from("<H", s.b, s.p)[0]; s.p += 2; return v
    def u64(s): v = struct.unpack_from("<Q", s.b, s.p)[0]; s.p += 8; return v
    def fstring(s):
        n = s.i32()
        if n == 0: return ""
        if n < 0:
            raw = s.b[s.p:s.p + (-n)*2]; s.p += (-n)*2
            return raw.decode("utf-16le", "replace").rstrip("\x00")
        raw = s.b[s.p:s.p+n]; s.p += n
        return raw.decode("latin-1").rstrip("\x00")
    def fname(s): idx = s.i32(); num = s.i32(); return idx, num

def parse_package(b):
    r = Reader(b)
    tag = r.u32(); ver = r.u16(); lic = r.u16()
    header_size = r.i32()
    folder = r.fstring()
    pkg_flags = r.u32()
    name_count, name_off = r.i32(), r.i32()
    exp_count, exp_off = r.i32(), r.i32()
    imp_count, imp_off = r.i32(), r.i32()

    # names
    r.p = name_off
    names = []
    for _ in range(name_count):
        names.append(r.fstring()); r.u64()

    # imports
    r.p = imp_off
    imports = []
    for _ in range(imp_count):
        cp = r.fname(); cn = r.fname(); outer = r.i32(); on = r.fname()
        imports.append({"classpkg": names[cp[0]], "class": names[cn[0]], "outer": outer, "name": names[on[0]]})

    # exports
    r.p = exp_off
    exports = []
    for _ in range(exp_count):
        cls = r.i32(); sup = r.i32(); outer = r.i32()
        nm = r.fname(); arch = r.i32(); oflags = r.u64()
        ssize = r.i32(); soff = r.i32(); eflags = r.i32()
        nnet = r.i32(); r.p += 4*nnet
        r.p += 16  # guid
        r.i32()    # package flags
        exports.append({"class": cls, "super": sup, "outer": outer,
                        "name": names[nm[0]] + (f"_{nm[1]-1}" if nm[1] else ""),
                        "size": ssize, "off": soff})
    return ver, lic, names, imports, exports

def objname(idx, imports, exports):
    if idx > 0: return exports[idx-1]["name"]
    if idx < 0: return imports[-idx-1]["name"]
    return None

FUNC_NET = 0x40; FUNC_NATIVE = 0x400; FUNC_EVENT = 0x800; FUNC_EXEC = 0x200

KNOWN_FLAGS = 0xFFFFFFFF  # licensee may add bits; we validate via the name index instead

def read_function_flags(b, exp, names):
    """UFunction tail: [iNative u16][OperPrecedence u8][FunctionFlags u32][RepOffset u16 if FUNC_Net][FriendlyName FName]
    FriendlyName is the export name for normal functions, or an operator symbol ('+', '==', ...) for operators."""
    end = exp["off"] + exp["size"]
    if exp["size"] < 40: return None
    fn_idx, fn_num = struct.unpack_from("<ii", b, end-8)
    if not (0 <= fn_idx < len(names)) or fn_num < 0 or fn_num > 100000:
        return None
    fname = names[fn_idx]
    base = exp["name"].rsplit("_", 1)[0] if "_" in exp["name"] else exp["name"]
    if not (fname == exp["name"] or fname == base or not fname[:1].isalpha()):
        return None
    flags_b = struct.unpack_from("<I", b, end-14)[0]   # layout if RepOffset present
    flags_a = struct.unpack_from("<I", b, end-12)[0]   # layout if not
    if flags_b & FUNC_NET:
        return flags_b
    return flags_a

# ---------------------------------------------------------------- main
def analyze(pkg):
    path = os.path.join(GAME, pkg)
    t0 = time.time()
    b = unwrap_fully_compressed(path)
    ver, lic, names, imports, exports = parse_package(b)
    # find the "Function" class import
    func_import = None
    for i, im in enumerate(imports):
        if im["name"] == "Function" and im["class"] == "Class":
            func_import = -(i+1); break
    per_class = collections.defaultdict(lambda: [0, 0, 0])  # [native, script, event]
    total = collections.Counter(); bad = 0
    for e in exports:
        if e["class"] != func_import: continue
        fl = read_function_flags(b, e, names)
        if fl is None: bad += 1; continue
        cls = objname(e["outer"], imports, exports) or "?"
        if fl & FUNC_NATIVE:
            per_class[cls][0] += 1; total["native"] += 1
        else:
            per_class[cls][1] += 1; total["script"] += 1
            if fl & FUNC_EVENT: per_class[cls][2] += 1; total["event"] += 1
    return dict(pkg=pkg, ver=ver, lic=lic, names=len(names), imports=len(imports), exports=len(exports),
                total=total, per_class=per_class, bad=bad, secs=time.time()-t0)

if __name__ == "__main__":
    pkgs = sys.argv[1:] or ["Core.upk", "Engine.upk", "GameFramework.upk", "GearboxFramework.upk",
                            "WillowGame.upk", "GFxUI.upk", "IpDrv.upk", "OnlineSubsystemSteamworks.upk", "AkAudio.upk"]
    grand = collections.Counter(); all_classes = {}
    print(f"{'package':<30}{'ver/lic':>9}{'exports':>9}{'functions':>11}{'NATIVE':>9}{'script':>9}{'native%':>9}{'events':>8}{'unparsed':>9}{'time':>7}")
    print("-"*110)
    for p in pkgs:
        try:
            r = analyze(p)
        except Exception as ex:
            print(f"{p:<30} ERROR: {ex}"); continue
        n, s = r["total"]["native"], r["total"]["script"]
        tot = n + s
        pct = (n / tot * 100) if tot else 0
        print(f"{p:<30}{str(r['ver'])+'/'+str(r['lic']):>9}{r['exports']:>9}{tot:>11}{n:>9}{s:>9}{pct:>8.1f}%{r['total']['event']:>8}{r['bad']:>9}{r['secs']:>6.1f}s")
        grand.update(r["total"])
        for c, v in r["per_class"].items():
            all_classes[(p, c)] = v
    n, s = grand["native"], grand["script"]
    print("-"*110)
    print(f"{'TOTAL':<30}{'':>9}{'':>9}{n+s:>11}{n:>9}{s:>9}{n/(n+s)*100:>8.1f}%{grand['event']:>8}")

    # where the natives live
    print("\n=== Top 40 classes by NATIVE function count (this is the rewrite list) ===")
    rows = sorted(all_classes.items(), key=lambda kv: -kv[1][0])[:40]
    print(f"{'package':<22}{'class':<40}{'native':>7}{'script':>7}{'events':>7}")
    for (p, c), (nn, ss, ev) in rows:
        print(f"{p:<22}{c:<40}{nn:>7}{ss:>7}{ev:>7}")

    # how concentrated are they?
    counts = sorted((v[0] for v in all_classes.values()), reverse=True)
    cum = 0
    print("\n=== Concentration: how many classes hold what share of all native functions ===")
    for k in (10, 25, 50, 100, 200):
        share = sum(counts[:k]) / max(1, n) * 100
        print(f"  top {k:>3} classes -> {share:5.1f}% of all natives")
    print(f"  classes with >=1 native: {sum(1 for c in counts if c>0)} of {len(counts)} classes with functions")
