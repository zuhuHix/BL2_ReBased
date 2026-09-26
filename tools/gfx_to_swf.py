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
- DefineSubImage (1008): u16 id, u16 image id, u16 x1, y1, x2, y2. Emitted as
  DefineBitsLossless2 cut from the packed atlas. UNVERIFIED: every observed
  sub-image references image 1; the atlas record's id bytes are ambiguous, so
  the atlas is chosen as the external image whose file is the pack texture.
- Other Scaleform tags (>= 1000) are dropped and counted.
"""
import argparse
import json
import struct
import zlib
from collections import Counter
from pathlib import Path

from PIL import Image

EXTERNAL_IMAGE2 = 1009
SUB_IMAGE = 1008
DEFINE_SPRITE = 39
BITS_LOSSLESS2 = 36


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


def parse_external(body):
    character, fmt, width, height = struct.unpack_from('<IHHH', body, 0)
    pos = 10
    export = body[pos + 1:pos + 1 + body[pos]].decode('latin-1')
    pos += 1 + body[pos]
    file = body[pos + 1:pos + 1 + body[pos]].decode('latin-1')
    return {'id': character, 'raw_id': body[:4].hex(' '), 'format': fmt, 'width': width, 'height': height,
            'export': export, 'file': file}


class Converter:
    def __init__(self, textures, pack_texture):
        self.textures = Path(textures)
        self.pack_texture = pack_texture
        self.atlas = None
        self.dropped = Counter()
        self.images = []
        self.sub_images = 0

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
                if Path(info['file']).stem == self.pack_texture:
                    # The pack atlas is only a source for sub-images.
                    path = self.textures / (self.pack_texture + '.png')
                    self.atlas = Image.open(path).convert('RGBA')
                    info['role'] = 'atlas'
                    self.images.append(info)
                    continue
                image, found = self.load(info['file'], info['width'], info['height'])
                info['found'] = found
                self.images.append(info)
                out += write_tag(BITS_LOSSLESS2, lossless2(info['id'] & 0xFFFF, image))
            elif code == SUB_IMAGE:
                character, image_id, x1, y1, x2, y2 = struct.unpack_from('<6H', body, 0)
                if self.atlas is None:
                    raise RuntimeError('DefineSubImage before the pack atlas')
                out += write_tag(BITS_LOSSLESS2, lossless2(character, self.atlas.crop((x1, y1, x2, y2))))
                self.sub_images += 1
            elif code == DEFINE_SPRITE:
                header = body[:4]
                inner = self.convert(read_tags(body, 4, len(body)))
                out += write_tag(DEFINE_SPRITE, header + inner)
            elif code >= 1000:
                self.dropped[code] += 1
            else:
                out += write_tag(code, body)
        return bytes(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--gfx', required=True, help='CFX/GFX bytes of the movie')
    parser.add_argument('--textures', required=True, help='directory of the movie textures as PNG')
    parser.add_argument('--pack-texture', default='texture1', help='name of the packed atlas texture')
    parser.add_argument('--output', required=True, help='output .swf (keep under local/)')
    args = parser.parse_args()
    raw = Path(args.gfx).read_bytes()
    if raw[:3] not in (b'CFX', b'GFX'):
        raise SystemExit(f'not a Scaleform movie: {raw[:3]!r}')
    version = raw[3]
    body = zlib.decompress(raw[8:]) if raw[:1] == b'C' else raw[8:]
    nbits = body[0] >> 3
    header_end = (5 + 4 * nbits + 7) // 8 + 4  # frame rect, frame rate, frame count
    converter = Converter(args.textures, args.pack_texture)
    tags = converter.convert(read_tags(body, header_end, len(body)))
    swf_body = body[:header_end] + tags
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(b'FWS' + bytes([version]) + struct.pack('<I', 8 + len(swf_body)) + swf_body)
    report = {'output': str(output), 'version': version, 'bytes': 8 + len(swf_body),
              'external_images': converter.images, 'sub_images': converter.sub_images,
              'dropped_scaleform_tags': dict(converter.dropped)}
    output.with_suffix('.json').write_text(json.dumps(report, indent=1), encoding='utf-8')
    missing = [i['file'] for i in converter.images if i.get('found') is False]
    print(json.dumps({k: v for k, v in report.items() if k != 'external_images'}))
    print('placeholders for runtime-supplied images:', missing)


if __name__ == '__main__':
    main()
