"""Oracle tables for ow_core::num and ow_core::fmt (lane W1-NUM; architecture section 5.2).

AI-assisted (Claude Code). Every input is invented: no value here comes from the game.

Three steps, run from the repository root (the C++ half is oracle.cpp beside this file, built with the MSVC toolset
the C++ core uses; the README of crates/ow-core/src/num/gen says how):

    python crates/ow-core/src/num/gen/oracle.py inputs local/p2/W1-NUM/inputs.txt
    oracle.exe local/p2/W1-NUM/inputs.txt > local/p2/W1-NUM/cxx.txt
    python crates/ow-core/src/num/gen/oracle.py python local/p2/W1-NUM/inputs.txt local/p2/W1-NUM/py.txt
    python crates/ow-core/src/num/gen/oracle.py merge local/p2/W1-NUM/cxx.txt local/p2/W1-NUM/py.txt \
        crates/ow-core/src/num/tables.txt crates/ow-core/src/fmt/tables.txt

`merge` refuses to write the tables when the two oracles disagree on a cell both of them produce. A cell only one of
them can produce (the C++ `to_chars` form, Python's `json.dumps` form, NaN signs Python does not print, inputs where
Python raises instead of returning) is taken from the one that does, and the row says which.
"""

import decimal
import json
import math
import re
import struct
import sys

MISSING = '-'
NOT_STORED = '~'
LONG_DIGITS_RE = re.compile(r'[0-9a-fA-F]{120,}')


def bits64(x):
    return struct.unpack('<Q', struct.pack('<d', x))[0]


def from64(h):
    return struct.unpack('<d', struct.pack('<Q', int(h, 16)))[0]


def f32(x):
    """(float)x, as the C++ cast: round to nearest, infinity on overflow."""
    if math.isnan(x) or math.isinf(x):
        return x
    try:
        return struct.unpack('<f', struct.pack('<f', x))[0]
    except OverflowError:
        return math.copysign(math.inf, x)


def bits32(x):
    return struct.unpack('<I', struct.pack('<f', x))[0]


def from32(h):
    return struct.unpack('<f', struct.pack('<I', int(h, 16)))[0]


def h64(x):
    return '%016x' % bits64(x)


def h32(x):
    return '%08x' % bits32(x)


class Lcg:
    """An invented generator for invented inputs (Numerical Recipes constants); its numbers mean nothing."""

    def __init__(self, seed):
        self.state = seed & 0xFFFFFFFF

    def next(self):
        self.state = (self.state * 1664525 + 1013904223) & 0xFFFFFFFF
        return self.state

    def unit(self):
        return ((self.next() << 21) ^ self.next()) / float(1 << 53)

    def uniform(self, lo, hi):
        return lo + (hi - lo) * self.unit()


def nan_bits():
    # Positive and negative quiet NaN, a payload, a signalling NaN: the C++ printers name them apart.
    return ['7ff8000000000000', 'fff8000000000000', '7ff8000000000001', 'fff8000000000001', '7ff0000000000001', 'fff0000000000001']


def fmt_values():
    values = [0.0, -0.0, math.inf, -math.inf, 5e-324, -5e-324, 1e-310, 2.225073858507201e-308,
              2.2250738585072014e-308, 1.7976931348623157e308, -1.7976931348623157e308]
    for k in range(-20, 23):
        p = float('1e%d' % k)
        values += [p, math.nextafter(p, math.inf), math.nextafter(p, -math.inf)]
    # Exact binary halfway cases and near-halfway cases for every printed precision.
    values += [0.5, 1.5, 2.5, 3.5, -0.5, -2.5, 0.125, 0.375, 0.625, 0.875, 1.25, 1.75, 2.25, 0.25, 0.75, 9.5, 99.5,
               999.5, 9999.5, 99999.5, 999999.5, 12345.5, 123456.5, 1234567.5, 12345678.5, 123456789.5, 0.0625,
               0.03125, 2.675, 1.005, 1.015, 4.35, 0.15, 0.35, 0.45, 0.55, 33.35, 1.0000005, 1.00000005, 0.99999995,
               9.9999995, 999999.95, 0.000123455, 1e15 + 0.5, 4503599627370495.5, 8.5, 10.5, 0.05, 0.005, 0.0005]
    # Integers and their neighbours around the precision and conversion limits.
    for p in (24, 31, 32, 52, 53, 54, 60, 63, 64, 70):
        x = float(1 << p)
        values += [x, x + 2 ** (p - 52), x - 2 ** (p - 53)]
    values += [1.0, 7.0, 100.0, 123456789.0, 123456789012345678.0, 1e21, 1e22, 1e23, 2e22, 1.2345e20, 9007199254740993.0,
               -1.0, -123456.0, 3.0, 1e16, 1.234e16, 9.999999999999999e15]
    # Values that are single-precision floats, the most common thing the outputs print.
    rng = Lcg(20261114)
    for _ in range(110):
        mag = 10.0 ** rng.uniform(-6, 7)
        sign = -1.0 if rng.next() & 1 else 1.0
        values.append(f32(sign * mag))
    for text in ('33.35', '0.1', '1.13', '753.2157', '2.45', '1.7499999', '0.3', '1e-7', '16777215', '3.4028234e38',
                 '1.1754944e-38', '1.4e-45'):
        values.append(f32(float(text)))
    # Floats from invented bit patterns, and floats whose exact decimal value has 18 digits ending in 5: their
    # shortest form is an exact tie between two 17-digit neighbours, which to_chars and repr break to even.
    ties = 0
    while ties < 40 or len(values) < 400:
        x = from32('%08x' % (rng.next() & 0x7f7fffff))
        exact = format(decimal.Decimal(x), 'e').split('e')[0].replace('.', '')
        if len(exact) == 18 and exact.endswith('5') and ties < 40:
            values.append(x)
            ties += 1
        elif len(values) < 400:
            values.append(x)
    # Doubles across the exponent range.
    for _ in range(150):
        mag = 10.0 ** rng.uniform(-30, 30)
        sign = -1.0 if rng.next() & 1 else 1.0
        values.append(sign * mag)
    out, seen = [], set()
    for v in values:
        key = h64(v)
        if key not in seen:
            seen.add(key)
            out.append(key)
    return out + nan_bits()


def record_pairs():
    """1.13^L and n^2.8 for L, n in 1..=80, and 10^d for d in 0..=6 (architecture 5.2)."""
    pairs = [(1.13, float(l)) for l in range(1, 81)]
    pairs += [(float(n), 2.8) for n in range(1, 81)]
    pairs += [(10.0, float(d)) for d in range(0, 7)]
    return pairs


def round_inputs():
    """Floats for std::round(float) (progression.cpp restricted(), RoundingMode 1)."""
    values = [0.5, 1.5, 2.5, 3.5, -0.5, -1.5, -2.5, -3.5, 0.3, -0.3, 0.0, -0.0, 0.49999997, -0.49999997, 0.50000006,
              -0.50000006, 8388607.5, -8388607.5, 8388609.0, 16777215.0, 1e10, -1e10, 1.4e-45, -1.4e-45, 99.5, -99.5,
              math.inf, -math.inf, math.nan]
    rng = Lcg(2468)
    values += [rng.uniform(-200, 200) for _ in range(30)]
    values += [int(rng.uniform(-200, 200)) + 0.5 for _ in range(10)]
    return [h32(f32(v)) for v in values]


def pow_pairs():
    pairs = record_pairs()
    rng = Lcg(4242)
    for _ in range(180):
        pairs.append((rng.uniform(0.0, 100.0), rng.uniform(-4.0, 4.0)))
    for _ in range(60):
        pairs.append((rng.uniform(0.5, 2.0), rng.uniform(-60.0, 60.0)))
    for _ in range(30):
        pairs.append((float(int(rng.uniform(-50, 50))), float(int(rng.uniform(-6, 7)))))
    pairs += [(0.0, 0.0), (0.0, -1.0), (-0.0, -1.0), (-0.0, -2.0), (0.0, 1.0), (-2.0, 3.0), (-2.0, 0.5), (-8.0, 1 / 3),
              (math.inf, 1.0), (math.inf, -1.0), (-math.inf, 3.0), (-math.inf, 2.0), (1.0, math.nan), (math.nan, 0.0),
              (2.0, 1024.0), (2.0, -1075.0), (2.0, -1074.0), (0.5, 1074.0), (5e-324, 0.5), (1e-310, 2.0), (1.0, math.inf),
              (-1.0, math.inf), (0.5, math.inf), (2.0, -math.inf), (10.0, 308.0), (10.0, 309.0), (1.0000001, 1e9),
              (2.0, 0.5), (3.0, 2.0), (7.0, 0.0), (-7.0, 0.0), (1.5, -0.0)]
    return pairs


def transcendental_inputs():
    rng = Lcg(777)
    common = [0.0, -0.0, 1.0, -1.0, 0.5, 2.0, math.pi, math.pi / 2, math.pi / 4, 1e-300, 5e-324, 1e300, math.inf,
              -math.inf, math.nan, 710.0, 709.78, -745.2, -746.0, 1e22, 0.1, 100.0]
    common += [f32(rng.uniform(-10, 10)) for _ in range(30)]
    common += [rng.uniform(-1000, 1000) for _ in range(10)]
    common += [rng.uniform(-1.2, 1.2) for _ in range(12)]
    pairs = [(rng.uniform(-10, 10), rng.uniform(-10, 10)) for _ in range(30)]
    pairs += [(0.0, 0.0), (-0.0, 0.0), (0.0, -0.0), (-0.0, -0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, -1.0), (math.inf, math.inf),
              (-math.inf, math.inf), (math.inf, -math.inf), (1.0, math.inf), (1.0, -math.inf), (math.nan, 1.0)]
    return common, pairs


def trunc_inputs():
    values = [0.0, -0.0, 0.5, -0.5, 0.9999999, -0.9999999, 1.5, -1.5, 2147483647.0, 2147483647.9, 2147483648.0,
              -2147483648.0, -2147483648.9, -2147483649.0, 4294967295.0, 4294967296.0, 4294967297.5, -4294967297.5,
              9.2233720368547748e18, 2.0 ** 63, -9.2233720368547758e18, -9.223372036854778e18, 1e19, -1e19,
              1e300, math.inf, -math.inf, math.nan, 123456.789, -98765.4321, 3e9, -3e9, 6442450943.0]
    return [h64(v) for v in values] + ['fff8000000000000']


def card_inputs():
    rng = Lcg(31337)
    rows = []
    for text in ('33.35', '33.349998', '1.7499998807907104', '1.75', '1.25', '2.45', '2.04', '87.36', '87.35',
                 '94.00000016', '9.0909', '0.05', '0.15', '1234.5', '0.5', '-1.25', '-0.05', '0'):
        for digits in (0, 1, 2):
            rows.append((f32(float(text)), digits))
    for _ in range(80):
        rows.append((f32(rng.uniform(-500, 500)), int(rng.uniform(0, 4))))
    return rows


def write_inputs(path):
    lines = []
    for a, b in pow_pairs():
        lines.append(f'pow64 {h64(a)} {h64(b)}')
        lines.append(f'pow32 {h32(f32(a))} {h32(f32(b))}')
    # The record constants again with float-widened inputs, as weapon_gen FormulaValue and the VM's ** receive them
    # (1.13 and 2.8 stored as floats); the pow32 rows above already hold those float inputs.
    for a, b in record_pairs():
        if (f32(a), f32(b)) != (a, b):
            lines.append(f'pow64 {h64(f32(a))} {h64(f32(b))}')
    common, pairs = transcendental_inputs()
    for name in ('exp', 'ln', 'sqrt', 'sin', 'cos', 'tan', 'asin', 'acos', 'atan'):
        for x in common:
            lines.append(f'{name} {h64(x)}')
    for y, x in pairs:
        lines.append(f'atan2 {h64(y)} {h64(x)}')
    for h in trunc_inputs():
        lines.append(f'trunc {h}')
    for h in round_inputs():
        lines.append(f'round32 {h}')
    for x, digits in card_inputs():
        lines.append(f'card {h32(x)} {digits}')
    for h in fmt_values():
        lines.append(f'fmt {h}')
    with open(path, 'w', encoding='ascii', newline='\n') as out:
        out.write('\n'.join(lines) + '\n')
    print(f'{len(lines)} inputs written to {path}')


def py_or_missing(fn):
    try:
        return fn()
    except (ValueError, OverflowError):
        return None


FMT_COLUMNS = ['g1', 'g5', 'g6', 'g9', 'g17', 'f0', 'f1', 'f3', 'f6', 'ts32', 'os6', 'os9', 'shortest', 'pyjson']


def python_fmt(x, bits):
    if math.isnan(x) and bits not in ('7ff8000000000000',):
        # Python prints every NaN as 'nan' without its sign or payload: the C++ forms of these come from C++ only.
        return [MISSING] * 12 + [MISSING, json.dumps(x)]
    cols = ['%.1g' % x, '%.5g' % x, '%g' % x, '%.9g' % x, '%.17g' % x, '%.0f' % x, '%.1f' % x, '%.3f' % x, '%f' % x,
            '%f' % f32(x), '%g' % x, '%.9g' % x]
    return cols + [MISSING, json.dumps(x)]


def run_python(inputs, path):
    out = []
    for line in open(inputs, encoding='ascii'):
        parts = line.split()
        kind, args = parts[0], parts[1:]
        if kind == 'pow64':
            r = py_or_missing(lambda: math.pow(from64(args[0]), from64(args[1])))
            res = [MISSING if r is None else h64(r)]
        elif kind == 'pow32':
            # Python has no single-precision pow: the double pow rounded once (equal to powf unless it double-rounds).
            r = py_or_missing(lambda: math.pow(from32(args[0]), from32(args[1])))
            res = [MISSING if r is None else h32(f32(r))]
        elif kind in ('exp', 'ln', 'sqrt', 'sin', 'cos', 'tan', 'asin', 'acos', 'atan'):
            fn = {'ln': math.log}.get(kind) or getattr(math, kind)
            r = py_or_missing(lambda: fn(from64(args[0])))
            if r is not None and math.isnan(r):
                r = None   # the C++ NaN's sign is the CRT's; Python cannot report it
            res = [MISSING if r is None else h64(r)]
        elif kind == 'atan2':
            r = py_or_missing(lambda: math.atan2(from64(args[0]), from64(args[1])))
            if r is not None and math.isnan(r):
                r = None
            res = [MISSING if r is None else h64(r)]
        elif kind == 'trunc':
            x = from64(args[0])
            if math.isfinite(x) and -2.0 ** 63 <= x < 2.0 ** 63:
                wrapped = int(x) & 0xFFFFFFFF
                wrapped = wrapped - (1 << 32) if wrapped >= 1 << 31 else wrapped
                res = [str(wrapped), MISSING]
            else:
                res = [MISSING, MISSING]
        elif kind == 'round32':
            # std::round: halves away from zero, the sign kept. Exact in double for a float input.
            x = from32(args[0])
            r = py_or_missing(lambda: math.copysign(float(math.floor(abs(x) + 0.5)), x))
            res = [MISSING if r is None else h32(f32(r))]
        elif kind == 'card':
            x, digits = from32(args[0]), int(args[1])
            scale = f32(10.0 ** digits)
            res = [h64(math.floor(f32(f32(x * scale) + 0.5)) / scale)]
        elif kind == 'fmt':
            res = python_fmt(from64(args[0]), args[0])
        else:
            raise SystemExit(f'unknown input kind {kind}')
        out.append(line.rstrip('\n') + '\t' + '\t'.join(res))
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.write('\n'.join(out) + '\n')
    print(f'{len(out)} python rows written to {path}')


def read_rows(path):
    rows = []
    for line in open(path, encoding='utf-8'):
        line = line.rstrip('\n')
        if line:
            key, _, rest = line.partition('\t')
            rows.append((key, rest.split('\t')))
    return rows


def merge(cxx_path, py_path, num_table, fmt_table):
    cxx, py = read_rows(cxx_path), read_rows(py_path)
    if [k for k, _ in cxx] != [k for k, _ in py]:
        raise SystemExit('the two oracles saw different inputs')
    disagreements, num_lines, fmt_lines = [], [], []
    counts = {}
    for (key, c), (_, p) in zip(cxx, py):
        kind = key.split()[0]
        if kind == 'fmt':
            # C++ columns: everything but pyjson. Python columns: everything but shortest.
            p = p[:12] + [MISSING, p[13]]
            c = c[:12] + [c[12], MISSING]
        if len(c) != len(p):
            raise SystemExit(f'column count differs on {key}')
        merged, source = [], []
        for i, (a, b) in enumerate(zip(c, p)):
            if a != MISSING and b != MISSING and a != b:
                disagreements.append(f'{key} column {i}: C++ {a!r} Python {b!r}')
            cell = a if a != MISSING else b
            letter = 'b' if a != MISSING and b != MISSING else ('c' if a != MISSING else ('p' if b != MISSING else '-'))
            if LONG_DIGITS_RE.search(cell):
                # tools/leak_gate.py refuses any run of 160 or more hex digits, and the %f text of a double above
                # about 1e159 is one. Such a cell is checked by the oracles here but not stored: '~', letter 's'.
                cell, letter = NOT_STORED, 's'
            merged.append(cell)
            source.append(letter)
        counts.setdefault(kind, {'both': 0, 'one': 0})
        counts[kind]['both' if all(s in 'b-' for s in source) else 'one'] += 1
        line = key.replace(' ', '\t') + '\t' + '\t'.join(merged) + '\t' + ''.join(source)
        (fmt_lines if kind == 'fmt' else num_lines).append(line)
    if disagreements:
        print(f'{len(disagreements)} disagreements; tables not written:')
        for d in disagreements[:60]:
            print('  ' + d)
        raise SystemExit(1)
    header = ('# Generated by crates/ow-core/src/num/gen/oracle.py merge (AI-assisted, Claude Code) from oracle.cpp '
              '(MSVC) and Python on the zuhu desktop. Invented inputs only.\n'
              '# Tab-separated: kind, inputs (hex bits), outputs, then one letter per output: b = both oracles '
              'printed it and agreed, c = C++ only, p = Python only, s = not stored (~: a run of 120 or more '
              'digits, which the leak gate refuses).\n')
    with open(num_table, 'w', encoding='utf-8', newline='\n') as f:
        f.write(header + '\n'.join(num_lines) + '\n')
    with open(fmt_table, 'w', encoding='utf-8', newline='\n') as f:
        f.write(header + '# fmt columns: ' + ' '.join(FMT_COLUMNS) + '\n' + '\n'.join(fmt_lines) + '\n')
    for kind, n in counts.items():
        print(f'{kind}: {n["both"]} rows from both oracles, {n["one"]} with a cell from one only')


def main(argv):
    if argv[:1] == ['inputs'] and len(argv) == 2:
        write_inputs(argv[1])
    elif argv[:1] == ['python'] and len(argv) == 3:
        run_python(argv[1], argv[2])
    elif argv[:1] == ['merge'] and len(argv) == 5:
        merge(*argv[1:])
    else:
        raise SystemExit(__doc__)


if __name__ == '__main__':
    main(sys.argv[1:])
