"""Convert an installed Scaleform GFX movie into a standard SWF for Flash players.

Input: the GFX bytes of a `SwfMovie` (its `RawData`, e.g. UI_HUD.HUD) and a
directory of that movie's textures decoded to PNG (`ow-package --texture`).
Output: an uncompressed `FWS` SWF under ignored local/ that ordinary players
(Ruffle, gameswf) can open. Nothing here goes into the repository.

The container follows Adobe's public SWF specification. Scaleform tags are
handled from layouts observed in the files (see DECISIONS.md 2026-09-26):
- DefineExternalImage2 (1009): u32 id, u16 format, u16 target width/height,
  u8-length export name, u8-length file name. Emitted as DefineBitsLossless2
  (36) from `<file stem>.png`, cropped to the target size. Missing files
  (the `-nopack` weapon icons the game fills at runtime) become transparent.
- Packed atlases are DefineExternalImage2 records whose bytes 2-3 are
  `09 00` (real character ids are u16, so those bytes are otherwise 0);
  bytes 0-1 are then a u16 atlas index (UI_HUD: index 1 `texture1`;
  SharedWillowComponents: 0 and 1; ConsoleComponents: 0 and 1).
- DefineSubImage (1008): u16 id, u16 atlas index, u16 x1, y1, x2, y2.
  Emitted as DefineBitsLossless2 cut from that atlas. The atlas marker and
  index are UNVERIFIED readings, consistent across the three movies checked.
- DefineCompactedFont (1005) is decoded by gfx_compacted_font.py and written
  as DefineFont3 (75). A font library's sample text fields read
  "$Alias = Font Name" (e.g. "$WillowBody = WillowBody"); each such font is
  also exported under its alias so importing movies find it.
- With --localization, "$File.Section.Key" tokens in DefineEditText initial
  text (e.g. "$WillowMenu.HUD.EnemyLevelAbbreviation") are replaced from the
  install's Localization/<lang>/*.int files, as Scaleform's translator does
  in game; Patched*.int files override their base file. The same tokens in
  ActionScript ConstantPool/Push strings are replaced too, but only in action
  streams with no branches or nested code (all 40 in UI_HUD are); others are
  counted as script_skipped.
- With --inline-font-imports, a font-only ImportAssets(2) whose URL resolves
  (next to --output) to an already converted SWF is replaced by the fonts
  themselves; see Converter.inline_fonts for the Ruffle limitation this avoids.
- Other Scaleform tags (>= 1000) are dropped and counted.
"""
import argparse
import json
import re
import struct
import zlib
from collections import Counter
from pathlib import Path

from PIL import Image

import gfx_compacted_font

EXTERNAL_IMAGE2 = 1009
COMPACTED_FONT = 1005
EXPORT_ASSETS = 56
IMPORT_ASSETS = 57
IMPORT_ASSETS2 = 71
DEFINE_FONT3 = 75
EDIT_TEXT = 37
DO_ACTION = 12
DO_INIT_ACTION = 59
SUB_IMAGE = 1008
DEFINE_SPRITE = 39
BITS_LOSSLESS2 = 36
ATLAS_MARKER = bytes((9, 0))  # bytes 2-3 of a packed-atlas DefineExternalImage2


def read_tags(data, pos, end):
    """Yield (code, body) for tags in data[pos:end], stopping after End."""
    while pos < end:
        code_len = struct.unpack_from('<H', data, pos)[0]
        pos += 2
        code, length = code_len >> 6, code_len & 0x3F
        if length == 0x3F:
            length = struct.unpack_from('<I', data, pos)[0]
            pos += 4
        yield code, data[pos:pos + length]
        pos += length
        if code == 0:
            return


def write_tag(code, body):
    # Long form always works; short form only for short bodies.
    if len(body) < 0x3F and code not in (BITS_LOSSLESS2,):
        return struct.pack('<H', (code << 6) | len(body)) + body
    return struct.pack('<HI', (code << 6) | 0x3F, len(body)) + body


def lossless2(character, image):
    """DefineBitsLossless2 format 5: zlib of premultiplied A, R, G, B bytes."""
    rgba = image.convert('RGBA')
    source = rgba.tobytes()
    pixels = bytearray(len(source))
    for i in range(0, len(source), 4):
        r, g, b, a = source[i:i + 4]
        pixels[i:i + 4] = (a, r * a // 255, g * a // 255, b * a // 255)
    return struct.pack('<HBHH', character, 5, rgba.width, rgba.height) + zlib.compress(bytes(pixels), 9)


def movie_tags(raw):
    """Top-level (code, body) tags of SWF/GFX bytes (FWS/CWS/GFX/CFX)."""
    body = zlib.decompress(raw[8:]) if raw[:1] in (b'C',) else raw[8:]
    nbits = body[0] >> 3
    header_end = (5 + 4 * nbits + 7) // 8 + 4  # frame rect, frame rate, frame count
    return body, header_end, read_tags(body, header_end, len(body))


def parse_assets(body, code):
    """(url or None, [(id, name)]) of an ExportAssets/ImportAssets(2) tag."""
    url, pos = None, 0
    if code != EXPORT_ASSETS:
        pos = body.index(b'\0') + 1
        url = body[:pos - 1].decode('latin-1')
        pos += 2 if code == IMPORT_ASSETS2 else 0
    count = struct.unpack_from('<H', body, pos)[0]
    pos += 2
    items = []
    for _ in range(count):
        character = struct.unpack_from('<H', body, pos)[0]
        end = body.index(b'\0', pos + 2)
        items.append((character, body[pos + 2:end].decode('latin-1')))
        pos = end + 1
    return url, items


def normalize_import_url(body):
    r"""ImportAssets(2) body with '\' in its URL replaced by '/'.

    Some movies (UI_StatusMenu) import `..\SharedWillowInventory\...`;
    Scaleform on Windows accepts that, a web player requests a literal
    backslash path and fails.
    """
    end = body.index(b'\0')
    return body[:end].replace(b'\\', b'/') + body[end:]


def exported_fonts(path):
    """{export name: DefineFont3 body} of an already converted SWF."""
    fonts, exports = {}, []
    for code, body in movie_tags(Path(path).read_bytes())[2]:
        if code == DEFINE_FONT3:
            fonts[struct.unpack_from('<H', body)[0]] = body
        elif code == EXPORT_ASSETS:
            exports += parse_assets(body, code)[1]
    return {name: fonts[i] for i, name in exports if i in fonts}


def parse_external(body):
    character, fmt, width, height = struct.unpack_from('<IHHH', body, 0)
    pos = 10
    export = body[pos + 1:pos + 1 + body[pos]].decode('latin-1')
    pos += 1 + body[pos]
    file = body[pos + 1:pos + 1 + body[pos]].decode('latin-1')
    return {'id': character, 'raw_id': body[:4].hex(' '), 'format': fmt, 'width': width, 'height': height,
            'export': export, 'file': file}


def load_localization(directory):
    """{"File.Section.Key": text} from UE3 .int files (UTF-16 or Latin-1)."""
    table = {}
    files = sorted(Path(directory).glob('*.int'), key=lambda p: p.name.lower().startswith('patched'))
    for path in files:
        raw = path.read_bytes()
        text = raw.decode('utf-16') if raw[:2] in (b'\xff\xfe', b'\xfe\xff') else raw.decode('latin-1')
        stem = path.stem[len('Patched'):] if path.stem.lower().startswith('patched') else path.stem
        section = None
        for line in text.splitlines():
            line = line.strip()
            if line.startswith('[') and line.endswith(']'):
                section = line[1:-1]
            elif section and '=' in line and not line.startswith(';'):
                key, value = line.split('=', 1)
                table[f'{stem}.{section}.{key.strip()}'.lower()] = value.strip().strip('"')
    return table


def localize_edit_text(body, table, counts):
    """Replace $File.Section.Key tokens in a DefineEditText's initial text."""
    if not table or not body[len(body) - 1:] == b'\0':
        return body
    rect_bits = body[2] >> 3
    flags_at = 2 + (5 + 4 * rect_bits + 7) // 8
    if not body[flags_at] & 0x80:             # HasText
        return body
    start = body.rindex(b'\0', 0, len(body) - 1) + 1
    return body[:start] + translate(body[start:-1], table, counts) + b'\0'


TOKEN = re.compile(r'\$([A-Za-z0-9_]+\.[A-Za-z0-9_]+\.[A-Za-z0-9_]+)')


def translate(raw, table, counts):
    """Replace $File.Section.Key tokens in UTF-8 bytes."""
    def swap(match):
        value = table.get(match.group(1).lower())
        counts['translated' if value is not None else 'untranslated'] += 1
        return match.group(0) if value is None else value
    text = raw.decode('utf-8', 'replace')
    return TOKEN.sub(swap, text).encode('utf-8')


# Actions that measure a byte distance over later code. Changing a string's
# length moves every later byte, so these are re-measured on the new layout.
BRANCHES = {0x99, 0x9D}                    # Jump, If: s16 offset from the action's end
FUNCTIONS = {0x9B, 0x8E}                   # DefineFunction(2): u16 body size, last field
WITH, TRY = 0x94, 0x8F                     # With: u16 size; Try: three u16 block sizes
PUSH_SIZES = {1: 4, 2: 0, 3: 0, 4: 1, 5: 1, 6: 8, 7: 4, 8: 1, 9: 2}  # non-string Push value sizes


def translate_strings(op, data, table, counts):
    """ConstantPool (0x88) or Push (0x96) payload with tokens translated."""
    if op == 0x88:
        strings = data[2:].split(b'\0')[:-1]
        return data[:2] + b''.join(translate(s, table, counts) + b'\0' for s in strings)
    new, i = bytearray(), 0
    while i < len(data):
        kind = data[i]
        if kind == 0:
            end = data.index(b'\0', i + 1)
            new += b'\0' + translate(data[i + 1:end], table, counts) + b'\0'
            i = end + 1
        else:
            new += data[i:i + 1 + PUSH_SIZES[kind]]
            i += 1 + PUSH_SIZES[kind]
    return bytes(new)


def localize_actions(code, table, counts):
    """Translate $File.Section.Key strings in an AVM1 action stream.

    Scaleform's translator swaps such strings when script assigns them to a
    text field (the HUD shows "$WillowMenu.HUD.EnemyLevelAbbreviation" as "LV"
    in game); players without a translator show the raw token. ConstantPool
    and Push strings are rewritten, then every branch offset and function,
    With and Try size is recomputed from old and new action positions. If a
    distance does not start and end on an action boundary the stream is left
    unchanged and its tokens are counted as script_skipped.
    """
    if not table or b'$' not in code:
        return code
    records, pos = [], 0                   # (old position, op, payload or None)
    while pos < len(code) and code[pos]:
        op = code[pos]
        size = struct.unpack_from('<H', code, pos + 1)[0] if op >= 0x80 else 0
        records.append((pos, op, code[pos + 3:pos + 3 + size] if op >= 0x80 else None))
        pos += 3 + size if op >= 0x80 else 1
    tail = pos                             # ActionEnd (or end of data)

    local = Counter()
    payloads = [translate_strings(op, data, table, local) if op in (0x88, 0x96) else data
                for _, op, data in records]
    if not local:
        return code
    moved, new_pos = {}, 0                 # old action start -> new start
    for (old, op, _), data in zip(records, payloads):
        moved[old] = new_pos
        new_pos += 1 if data is None else 3 + len(data)
    moved[tail] = new_pos

    def distance(old_from, old_to, new_from):
        if old_to not in moved:
            raise ValueError('distance does not end on an action boundary')
        return moved[old_to] - new_from

    try:
        for i, ((old, op, data), new) in enumerate(zip(records, payloads)):
            end, new_end = old + 3 + len(data or b''), moved[old] + 3 + len(new or b'')
            if op in BRANCHES:
                target = end + struct.unpack_from('<h', data)[0]
                payloads[i] = struct.pack('<h', distance(end, target, new_end))
            elif op in FUNCTIONS or op == WITH:
                size = struct.unpack_from('<H', new, len(new) - 2)[0]
                payloads[i] = new[:-2] + struct.pack('<H', distance(end, end + size, new_end))
            elif op == TRY:
                sizes = struct.unpack_from('<3H', new, 1)
                bounds, at = [], end
                for size in sizes:
                    bounds.append(at + size)
                    at += size
                starts = [end] + bounds[:2]
                news = [new_end] + [moved.get(b, -1) for b in bounds[:2]]
                fixed = [distance(s, b, n) for s, b, n in zip(starts, bounds, news)]
                payloads[i] = new[:1] + struct.pack('<3H', *fixed) + new[7:]
    except (ValueError, struct.error):
        counts['script_skipped'] += len(TOKEN.findall(code.decode('latin-1')))
        return code
    counts.update(local)
    out = bytearray()
    for (_, op, _), data in zip(records, payloads):
        out.append(op)
        if data is not None:
            out += struct.pack('<H', len(data)) + data
    return bytes(out) + code[tail:]


def font_aliases(tags):
    """{font name: [alias]} from "$Alias = Font Name" sample edit texts."""
    aliases = {}
    for code, body in tags:
        if code != EDIT_TEXT:
            continue
        text = re.sub(r'<[^>]+>', '', body.decode('latin-1')).replace('&nbsp;', ' ')
        for alias, name in re.findall(r'(\$\w+)\s*=\s*([^\x00<]+)', text):
            aliases.setdefault(name.strip(), []).append(alias)
    return aliases


class Converter:
    def __init__(self, textures, aliases=None, localization=None, font_base=None):
        self.textures = Path(textures)
        self.aliases = aliases or {}
        self.localization = localization or {}
        self.text = Counter()
        self.font_base = font_base      # directory import URLs resolve against, or None
        self.inlined_fonts = []
        self.atlases = {}
        self.dropped = Counter()
        self.images = []
        self.sub_images = 0
        self.fonts = []

    def inline_fonts(self, code, body):
        """Replace a font-only import with the fonts themselves, else keep it.

        Ruffle (nightly 2026-09-26) preloads an imported movie once; a library
        that itself imports (SharedWillowComponents imports gfxfontlib) stops
        at that import and never registers its later exports. Embedding the
        fonts removes the nested import. Scaleform resolves imports itself, so
        this only changes the locally converted copy.
        """
        url, items = parse_assets(body, code)
        source = self.font_base / url
        fonts = exported_fonts(source) if source.is_file() else {}
        if not items or any(name not in fonts for _, name in items):
            return write_tag(code, body)
        out = bytearray()
        for character, name in items:
            font = fonts[name]
            out += write_tag(DEFINE_FONT3, struct.pack('<H', character) + font[2:])
            self.inlined_fonts.append({'id': character, 'name': name, 'from': url})
        # HTML text selects fonts by the import name (<font face="$WillowBody">),
        # which an import registers; keep that name as an export.
        exports = b''.join(struct.pack('<H', i) + n.encode('latin-1') + b'\0' for i, n in items)
        return bytes(out) + write_tag(EXPORT_ASSETS, struct.pack('<H', len(items)) + exports)

    def load(self, file, width, height):
        path = self.textures / (Path(file).stem + '.png')
        if not path.is_file():
            return Image.new('RGBA', (max(1, width), max(1, height)), (0, 0, 0, 0)), False
        return Image.open(path).convert('RGBA').crop((0, 0, width, height)), True

    def convert(self, tags):
        out = bytearray()
        for code, body in tags:
            if code == EXTERNAL_IMAGE2:
                info = parse_external(body)
                if body[2:4] == ATLAS_MARKER:
                    # A packed atlas is only a source for sub-images.
                    index = struct.unpack_from('<H', body, 0)[0]
                    image, found = self.load(info['file'], info['width'], info['height'])
                    self.atlases[index] = image
                    info.update(role='atlas', atlas_index=index, found=found)
                    self.images.append(info)
                    continue
                image, found = self.load(info['file'], info['width'], info['height'])
                info['found'] = found
                self.images.append(info)
                out += write_tag(BITS_LOSSLESS2, lossless2(info['id'] & 0xFFFF, image))
            elif code == SUB_IMAGE:
                character, atlas, x1, y1, x2, y2 = struct.unpack_from('<6H', body, 0)
                if atlas not in self.atlases:
                    raise RuntimeError(f'DefineSubImage {character} references unknown atlas {atlas}')
                out += write_tag(BITS_LOSSLESS2, lossless2(character, self.atlases[atlas].crop((x1, y1, x2, y2))))
                self.sub_images += 1
            elif code == DEFINE_SPRITE:
                header = body[:4]
                inner = self.convert(read_tags(body, 4, len(body)))
                out += write_tag(DEFINE_SPRITE, header + inner)
            elif code == COMPACTED_FONT:
                font = gfx_compacted_font.decode(body)
                out += write_tag(gfx_compacted_font.DEFINE_FONT3, gfx_compacted_font.define_font3(font))
                aliases = self.aliases.get(font['name'], [])
                if aliases:
                    exports = b''.join(struct.pack('<H', font['id']) + a.encode('latin-1') + b'\0' for a in aliases)
                    out += write_tag(EXPORT_ASSETS, struct.pack('<H', len(aliases)) + exports)
                self.fonts.append({'id': font['id'], 'name': font['name'], 'glyphs': len(font['glyphs']),
                                   'skipped_glyphs': [f'U+{c:04X}' for c in font['skipped']], 'exported_as': aliases})
            elif code == EDIT_TEXT:
                out += write_tag(code, localize_edit_text(body, self.localization, self.text))
            elif code == DO_ACTION:
                out += write_tag(code, localize_actions(body, self.localization, self.text))
            elif code == DO_INIT_ACTION:
                out += write_tag(code, body[:2] + localize_actions(body[2:], self.localization, self.text))
            elif code in (IMPORT_ASSETS, IMPORT_ASSETS2):
                body = normalize_import_url(body)
                out += self.inline_fonts(code, body) if self.font_base is not None else write_tag(code, body)
            elif code >= 1000:
                self.dropped[code] += 1
            else:
                out += write_tag(code, body)
        return bytes(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--gfx', required=True, help='CFX/GFX bytes of the movie')
    parser.add_argument('--textures', required=True, help='directory of the movie textures as PNG')
    parser.add_argument('--localization', help='install Localization/<lang> directory of .int files')
    parser.add_argument('--inline-font-imports', action='store_true',
                        help='embed fonts imported from already converted SWFs next to --output')
    parser.add_argument('--output', required=True, help='output .swf (keep under local/)')
    args = parser.parse_args()
    raw = Path(args.gfx).read_bytes()
    if raw[:3] not in (b'CFX', b'GFX'):
        raise SystemExit(f'not a Scaleform movie: {raw[:3]!r}')
    version = raw[3]
    body, header_end, tags = movie_tags(raw)
    table = load_localization(args.localization) if args.localization else {}
    font_base = Path(args.output).parent if args.inline_font_imports else None
    converter = Converter(args.textures, font_aliases(tags), table, font_base)
    tags = converter.convert(read_tags(body, header_end, len(body)))
    swf_body = body[:header_end] + tags
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(b'FWS' + bytes([version]) + struct.pack('<I', 8 + len(swf_body)) + swf_body)
    report = {'output': str(output), 'version': version, 'bytes': 8 + len(swf_body),
              'external_images': converter.images, 'sub_images': converter.sub_images,
              'fonts': converter.fonts, 'inlined_font_imports': converter.inlined_fonts,
              'localized_tokens': dict(converter.text),
              'dropped_scaleform_tags': dict(converter.dropped)}
    output.with_suffix('.json').write_text(json.dumps(report, indent=1), encoding='utf-8')
    missing = [i['file'] for i in converter.images if i.get('found') is False]
    print(json.dumps({k: v for k, v in report.items() if k != 'external_images'}))
    print('placeholders for runtime-supplied images:', missing)


if __name__ == '__main__':
    main()
