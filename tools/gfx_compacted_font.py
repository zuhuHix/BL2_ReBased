"""Decode Scaleform DefineCompactedFont (tag 1005) and re-encode it as DefineFont3.

Layout observed in BL2's installed movies (UI_FontsEn.FontsEn, UI_HUD.HUD);
no Scaleform code or SDK was consulted. Offsets below are relative to the
byte after the tag's u16 font id ("base").

- u16 font id, NUL-terminated name, then u16 flags, u16 nominal size (256),
  u16 ascent, u16 descent, s16 leading (Chintzy CPU BRK: -26), u32 glyph count, u32 glyph-data size.
- Glyph data, then per glyph u16 code, u16 advance, u32 offset (from base),
  then a kerning table: u32-ish count prefix and (u16 code1, u16 code2,
  s16 adjust) records (the prefix is not yet understood; pairs are located
  by the record size).
- Numbers are variable length: an unsigned "ui15"/signed "si15" is 1 byte
  when bit 0 of the first byte is clear, else 2 bytes; the value is >> 1.
- A glyph: si15 bounds (x1, y1, x2, y2), ui15 contour count; a contour:
  si15 x, si15 y (move-to), ui15 edge word. Edge word bit 1 set: the edge
  list (edge word + edges) is shared, at offset (word >> 2) from base.
  Otherwise (word >> 2) edges follow.
- An edge's low 4 bits are its type; the payload fills whole bytes:
  0/1 horizontal line of 12/20 bits, 2/3 vertical line of 12/20 bits,
  4-7 lines with two 6/10/14/18-bit deltas, 8-15 quadratic curves with four
  5/7/.../19-bit deltas (control dx, dy, anchor dx, dy relative to the
  control). Checked by closure/bounds on every ASCII glyph of all three
  fonts and by rendering them.
- UNVERIFIED/unsupported: edge-word bit 0 (seen only on some accented
  glyphs, e.g. U+010C); such glyphs are emitted empty and reported.
"""
import struct

DEFINE_FONT3 = 75
EM_SCALE = 1024 * 20 / 256  # DefineFont3 EM is 1024 twentieths; nominal size is 256


class Reader:
    def __init__(self, data, pos):
        self.data, self.pos = data, pos

    def u8(self):
        value = self.data[self.pos]
        self.pos += 1
        return value

    def ui15(self):
        first = self.u8()
        return (first | self.u8() << 8) >> 1 if first & 1 else first >> 1

    def si15(self):
        first = self.u8()
        if first & 1:
            value = (first | self.u8() << 8) >> 1
            return value - 0x8000 if value & 0x4000 else value
        value = first >> 1
        return value - 0x80 if value & 0x40 else value


def signed(value, bits):
    return value - (1 << bits) if value & (1 << (bits - 1)) else value


LINE_BITS = {4: 6, 5: 10, 6: 14, 7: 18}
CURVE_BITS = {8: 5, 9: 7, 10: 9, 11: 11, 12: 13, 13: 15, 14: 17, 15: 19}


def read_edges(reader, count):
    edges = []
    for _ in range(count):
        kind = reader.data[reader.pos] & 15
        if kind < 4:
            bits = 12 if kind in (0, 2) else 20
        elif kind in LINE_BITS:
            bits = 2 * LINE_BITS[kind]
        else:
            bits = 4 * CURVE_BITS[kind]
        size = (4 + bits) // 8
        word = int.from_bytes(reader.data[reader.pos:reader.pos + size], 'little') >> 4
        reader.pos += size
        if kind < 4:
            value = signed(word, bits)
            edges.append(('L', value, 0) if kind in (0, 1) else ('L', 0, value))
        elif kind in LINE_BITS:
            b = LINE_BITS[kind]
            edges.append(('L', signed(word & ((1 << b) - 1), b), signed(word >> b, b)))
        else:
            b = CURVE_BITS[kind]
            m = (1 << b) - 1
            edges.append(('C', signed(word & m, b), signed((word >> b) & m, b),
                          signed((word >> 2 * b) & m, b), signed((word >> 3 * b) & m, b)))
    return edges


def decode(tag):
    """Tag body -> dict(id, name, ascent, descent, leading, glyphs, kerning, skipped)."""
    font_id = struct.unpack_from('<H', tag, 0)[0]
    name_end = tag.index(b'\0', 2)
    name = tag[2:name_end].decode('latin-1')
    _flags, _nominal, ascent, descent, leading, count, size = struct.unpack_from('<HHHHhII', tag, name_end + 1)
    data_start = name_end + 1 + 18
    table = data_start + size
    glyphs, skipped = [], []
    for i in range(count):
        code, advance, offset = struct.unpack_from('<HHI', tag, table + i * 8)
        reader = Reader(tag, 2 + offset)
        bounds = [reader.si15() for _ in range(4)]
        contours = []
        try:
            for _ in range(reader.ui15()):
                x, y = reader.si15(), reader.si15()
                word = reader.ui15()
                if word & 1:
                    raise ValueError('edge word bit 0 unsupported')
                if word & 2:
                    shared = Reader(tag, 2 + (word >> 2))
                    edges = read_edges(shared, shared.ui15() >> 2)
                else:
                    edges = read_edges(reader, word >> 2)
                contours.append((x, y, edges))
        except (ValueError, IndexError, KeyError):
            contours = []
            skipped.append(code)
        empty = bounds[0] > bounds[2]
        glyphs.append({'code': code, 'advance': advance, 'contours': contours,
                       'bounds': (0, 0, 0, 0) if empty else tuple(bounds)})
    kerning = []
    rest = tag[table + count * 8:]
    # Kerning records are (u16, u16, s16) aligned to the table's end.
    usable = len(rest) - len(rest) % 6
    for i in range(len(rest) - usable, len(rest), 6):
        a, b, adjust = struct.unpack_from('<HHh', rest, i)
        kerning.append((a, b, adjust))
    return {'id': font_id, 'name': name, 'ascent': ascent, 'descent': descent, 'leading': leading,
            'glyphs': glyphs, 'kerning': kerning, 'skipped': skipped}


class Bits:
    def __init__(self):
        self.out, self.acc, self.n = bytearray(), 0, 0

    def put(self, value, bits):
        for i in range(bits - 1, -1, -1):
            self.acc = (self.acc << 1) | ((value >> i) & 1)
            self.n += 1
            if self.n == 8:
                self.out.append(self.acc)
                self.acc, self.n = 0, 0

    def flush(self):
        if self.n:
            self.out.append(self.acc << (8 - self.n))
            self.acc, self.n = 0, 0
        return bytes(self.out)


def sbits_needed(*values):
    return max(2, max((abs(v) * 2 + (1 if v < 0 else 0)).bit_length() + 1 for v in values))


def put_signed(bits, value, n):
    bits.put(value & ((1 << n) - 1), n)


def shape(contours):
    """SHAPE for DefineFont3: NumFillBits 1, NumLineBits 0, fill style 1."""
    bits = Bits()
    bits.put(1, 4)
    bits.put(0, 4)
    first = True
    for x, y, edges in contours:
        mx, my = round(x * EM_SCALE), round(y * EM_SCALE)
        n = sbits_needed(mx, my)
        bits.put(0, 1)                      # style change record
        bits.put(0, 1); bits.put(0, 1)      # new styles, line style
        bits.put(1 if first else 0, 1)      # fill style 1
        bits.put(0, 1)                      # fill style 0
        bits.put(1, 1)                      # move to
        bits.put(n, 5)
        put_signed(bits, mx, n)
        put_signed(bits, my, n)
        if first:
            bits.put(1, 1)                  # fill style 1 index
        first = False
        for edge in edges:
            if edge[0] == 'L':
                dx, dy = round(edge[1] * EM_SCALE), round(edge[2] * EM_SCALE)
                n = sbits_needed(dx, dy)
                bits.put(1, 1); bits.put(1, 1); bits.put(n - 2, 4)
                bits.put(1, 1)              # general line
                put_signed(bits, dx, n); put_signed(bits, dy, n)
            else:
                values = [round(v * EM_SCALE) for v in edge[1:]]
                n = sbits_needed(*values)
                bits.put(1, 1); bits.put(0, 1); bits.put(n - 2, 4)
                for v in values:
                    put_signed(bits, v, n)
    bits.put(0, 6)                          # end shape record
    return bits.flush()


def rect(x1, y1, x2, y2):
    values = [round(v) for v in (x1, x2, y1, y2)]
    n = max(1, max((abs(v) * 2 + (1 if v < 0 else 0)).bit_length() + 1 for v in values))
    bits = Bits()
    bits.put(n, 5)
    for v in values:
        put_signed(bits, v, n)
    return bits.flush()


def define_font3(font):
    glyphs = sorted(font['glyphs'], key=lambda g: g['code'])
    shapes = [shape(g['contours']) for g in glyphs]
    count = len(glyphs)
    offsets, pos = [], 4 * count + 4
    for s in shapes:
        offsets.append(pos)
        pos += len(s)
    body = bytearray(struct.pack('<HBB', font['id'], 0x8C, 1))   # HasLayout, WideOffsets, WideCodes
    name = font['name'].encode('latin-1')
    body += bytes([len(name)]) + name + struct.pack('<H', count)
    body += b''.join(struct.pack('<I', o) for o in offsets) + struct.pack('<I', pos)
    body += b''.join(shapes)
    body += b''.join(struct.pack('<H', g['code']) for g in glyphs)
    s = EM_SCALE
    body += struct.pack('<HHh', round(font['ascent'] * s), round(font['descent'] * s), round(font['leading'] * s))
    body += b''.join(struct.pack('<h', max(-32768, min(32767, round(g['advance'] * s)))) for g in glyphs)
    body += b''.join(rect(*(v * s for v in g['bounds'])) for g in glyphs)
    codes = {g['code'] for g in glyphs}
    kerning = [k for k in font['kerning'] if k[0] in codes and k[1] in codes]
    body += struct.pack('<H', len(kerning))
    body += b''.join(struct.pack('<HHh', a, b, round(adj * s)) for a, b, adj in kerning)
    return bytes(body)
