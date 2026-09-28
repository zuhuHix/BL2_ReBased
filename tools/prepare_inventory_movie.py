"""Work around Ruffle's nested-import preload stop in a local SWF library.

Use only on SharedWillowInventory's converted *library*, not a timeline movie.
Keeps every tag byte-for-byte and moves ImportAssets(2) after definitions,
before End. This is a local Ruffle workaround, not Scaleform equivalence.
Outputs must remain under ignored local/. See the inventory verification note.
"""
import argparse
import struct
from pathlib import Path


def defer_library_imports(raw):
    if len(raw) < 13 or raw[:3] != b'FWS':
        raise ValueError('expected an uncompressed converted SWF library')
    if struct.unpack_from('<I', raw, 4)[0] != len(raw):
        raise ValueError('SWF length mismatch')
    start = 8 + (5 + 4 * (raw[8] >> 3) + 7) // 8 + 4
    if start > len(raw):
        raise ValueError('truncated movie header')
    if struct.unpack_from('<H', raw, start - 2)[0] != 1:
        raise ValueError('only single-frame libraries are supported')
    imports, other = [], []
    pos = start
    while pos < len(raw):
        begin = pos
        if pos + 2 > len(raw):
            raise ValueError('truncated tag header')
        tag = struct.unpack_from('<H', raw, pos)[0]
        pos += 2
        code, size = tag >> 6, tag & 63
        if size == 63:
            if pos + 4 > len(raw):
                raise ValueError('truncated long tag header')
            size = struct.unpack_from('<I', raw, pos)[0]
            pos += 4
        if pos + size > len(raw):
            raise ValueError('truncated tag body')
        pos += size
        record = raw[begin:pos]
        if code == 0:
            if size or pos != len(raw):
                raise ValueError('invalid End tag or trailing bytes')
            return raw[:start] + b''.join(other + imports) + record, len(imports)
        (imports if code in (57, 71) else other).append(record)
    raise ValueError('missing End tag')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    local = Path(__file__).resolve().parents[1] / 'local'
    if not args.output.resolve().is_relative_to(local.resolve()):
        parser.error('output must be under this checkout\'s ignored local/')
    result, count = defer_library_imports(args.input.read_bytes())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(result)
    print(f'{count} import tags deferred; {len(result)} bytes; {args.output}')


if __name__ == '__main__':
    main()
