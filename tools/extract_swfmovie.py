"""Write a GFxUI.SwfMovie's RawData (its Scaleform GFX/CFX bytes) to a local file.

Uses `ow-package --properties` to find the RawData ArrayProperty and
`--payload` for the export bytes. The movie is game data: write it only under
ignored local/ (see docs/LEGAL.md, "UI movies").
"""
import argparse
import json
import subprocess
from pathlib import Path


def run(reader, package, *args):
    result = subprocess.run([reader, str(package), *map(str, args)], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'ow-package {args}: {result.stderr.strip()}')
    return result.stdout


def extract(reader, package, path):
    exports = json.loads(run(reader, package, '--exports'))
    index = next((e['index'] for e in exports if e['path'] == path), None)
    if index is None:
        raise KeyError(f'{path} is not an export of {Path(package).name}')
    props = json.loads(run(reader, package, '--properties', index, '--property-offset', 4))
    raw = next(p for p in props['properties'] if p['name'] == 'RawData')
    payload = bytes(json.loads(run(reader, package, '--payload', index)))
    # `offset` is the property tag's start: a UE3 tag is name (8) + type (8)
    # + size (4) + array index (4) bytes, then the value is a u32 element
    # count followed by the bytes. Checked on UI_HUD.HUD: CFX at byte 256.
    start = raw['offset'] + 24 + 4
    data = payload[start:start + raw['element_count']]
    if data[:3] not in (b'CFX', b'GFX', b'FWS', b'CWS'):
        raise RuntimeError(f'{path}: RawData does not start with a movie signature: {data[:4]!r}')
    source = next((p['value'] for p in props['properties'] if p['name'] == 'SourceFile'), None)
    return data, source


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True)
    parser.add_argument('--package', required=True)
    parser.add_argument('--movie', required=True, help='e.g. UI_HUD.HUD')
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    data, source = extract(str(Path(args.reader).resolve()), Path(args.package), args.movie)
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    Path(args.output).write_bytes(data)
    print(json.dumps({'movie': args.movie, 'source_file': source, 'signature': data[:3].decode(),
                      'version': data[3], 'bytes': len(data), 'output': args.output}))


if __name__ == '__main__':
    main()
