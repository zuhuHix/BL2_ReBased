"""Decode the bytes a cooked MaterialInstanceConstant keeps after its tagged properties.

When bHasStaticPermutationResource is set, those bytes hold the instance's own compiled
material resource followed by its static parameter set (static switch, static component
mask, normal and terrain-layer parameters). The layout below was recovered from Startup.upk
(version 832/46) and is accepted only when it consumes the trailing bytes exactly; the
census mode applies that oracle to every such MIC in a package. The resource's leading
words are not given meanings beyond what the bytes show.

Output is game-derived and belongs under ignored local/. Everything here is structural:
what a static parameter does inside the stripped Master_Gun graph is not proven by it.
"""
import argparse
import json
from pathlib import Path
import struct
import subprocess
import tempfile


class Cursor:
    def __init__(self, data):
        self.data, self.pos = data, 0

    def take(self, size):
        if size < 0 or self.pos + size > len(self.data):
            raise ValueError(f'truncated at {self.pos} (+{size} of {len(self.data)})')
        chunk = self.data[self.pos:self.pos + size]
        self.pos += size
        return chunk

    def i32(self):
        return struct.unpack('<i', self.take(4))[0]

    def f32(self):
        return struct.unpack('<f', self.take(4))[0]

    def u8(self):
        return self.take(1)[0]

    def count(self, record_size, limit=4096):
        value = self.i32()
        if value < 0 or value > limit or self.pos + value * record_size > len(self.data):
            raise ValueError(f'implausible count {value} at {self.pos - 4}')
        return value

    def boolean(self):
        value = self.i32()
        if value not in (0, 1):
            raise ValueError(f'non-boolean {value} at {self.pos - 4}')
        return bool(value)

    def guid(self):
        return self.take(16).hex()

    def name(self, names):
        index, number = self.i32(), self.i32()
        if not 0 <= index < len(names) or number < 0:
            raise ValueError(f'bad name ({index}, {number}) at {self.pos - 8}')
        return names[index] + (f'_{number - 1}' if number else '')


def material_resource(cur):
    """The leading compiled-resource block. Only counts and references are interpreted."""
    if cur.count(4):
        raise ValueError('compile error strings present; not observed, not decoded')
    if cur.count(8):
        raise ValueError('texture dependency map entries present; not observed, not decoded')
    out = {'max_texture_dependency': cur.i32(), 'id': cur.guid(), 'user_texcoords': cur.i32()}
    out['textures'] = [cur.i32() for _ in range(cur.count(4))]
    out['flag_words'] = [cur.i32() for _ in range(6)]
    out['lookups'] = [{'texcoord': cur.i32(), 'texture': cur.i32(), 'u_scale': cur.f32(), 'v_scale': cur.f32()}
                      for _ in range(cur.count(16))]
    out['trailing_word'] = cur.i32()
    return out


def static_parameters(cur, names):
    out = {'base_material_id': cur.guid(), 'switches': [], 'component_masks': [], 'normals': [], 'terrain_layers': []}
    for _ in range(cur.count(32)):
        out['switches'].append({'name': cur.name(names), 'value': cur.boolean(), 'override': cur.boolean(),
                                'guid': cur.guid()})
    for _ in range(cur.count(44)):
        entry = {'name': cur.name(names)}
        entry.update({channel: cur.boolean() for channel in 'RGBA'})
        entry.update({'override': cur.boolean(), 'guid': cur.guid()})
        out['component_masks'].append(entry)
    for _ in range(cur.count(29)):
        out['normals'].append({'name': cur.name(names), 'compression': cur.u8(), 'override': cur.boolean(),
                               'guid': cur.guid()})
    for _ in range(cur.count(32)):
        out['terrain_layers'].append({'name': cur.name(names), 'override': cur.boolean(), 'guid': cur.guid(),
                                      'weightmap_index': cur.i32()})
    return out


def decode_tail(tail, names):
    """Resource + static parameter set; raises unless the bytes are consumed exactly."""
    cur = Cursor(tail)
    result = {'resource': material_resource(cur), 'static': static_parameters(cur, names)}
    if cur.pos != len(tail):
        raise ValueError(f'{len(tail) - cur.pos} bytes left over')
    return result


def mask_channels(entry):
    return ''.join(channel for channel in 'RGBA' if entry[channel])


class Reader:
    def __init__(self, exe, package):
        self.exe, self.package = str(exe), str(package)
        self.names = json.loads(self.run('--names'))
        self.exports = json.loads(self.run('--exports'))

    def run(self, *args):
        result = subprocess.run([self.exe, self.package, *args], capture_output=True, encoding='utf-8')
        if result.returncode:
            raise ValueError(f'ow-package {args[0]} failed: {result.stderr.strip()}')
        return result.stdout

    def tails(self, indices):
        """export index -> (property record, trailing bytes) for MICs with a static permutation."""
        with tempfile.TemporaryDirectory() as folder:
            listing = Path(folder) / 'indices.txt'
            listing.write_text('\n'.join(map(str, indices)))
            records = [json.loads(line) for line in
                       self.run('--properties-batch', str(listing), '--property-offset', '4').splitlines()
                       if line.strip()]
        out = {}
        for start in range(0, len(records), 256):
            batch = records[start:start + 256]
            payloads = json.loads(self.run('--payloads', *[str(r['index']) for r in batch]))
            for record, payload in zip(batch, payloads):
                flags = {p['name']: p.get('value') for p in record['properties']}
                if not flags.get('bHasStaticPermutationResource'):
                    continue
                begin = record['property_offset'] + record['consumed_bytes']
                out[record['index']] = (record, bytes(payload['payload'])[begin:])
        return out


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--package', type=Path, required=True)
    parser.add_argument('--mic', nargs='*', default=[], help='MIC object names to print (default: census only)')
    parser.add_argument('--output', type=Path, help='JSON report; must be under local/')
    args = parser.parse_args()
    reader = Reader(args.reader, args.package)
    mics = [e for e in reader.exports if e['class'].split('.')[-1] == 'MaterialInstanceConstant']
    tails = reader.tails([e['index'] for e in mics])
    decoded, failures = {}, []
    for record, tail in tails.values():
        try:
            decoded[record['path']] = decode_tail(tail, reader.names)
        except ValueError as error:
            failures.append({'path': record['path'], 'bytes': len(tail), 'error': str(error)})
    print(f'{len(mics)} MICs, {len(tails)} with a static permutation, {len(decoded)} consumed exactly, '
          f'{len(failures)} failed')
    for failure in failures[:10]:
        print('  FAILED', failure)
    for path, value in decoded.items():
        if path.split('.')[-1] not in args.mic:
            continue
        static = value['static']
        print(path)
        for entry in static['switches']:
            print(f"  switch {entry['name']} = {entry['value']} (override {entry['override']})")
        for entry in static['component_masks']:
            print(f"  mask {entry['name']} = {mask_channels(entry) or '-'} (override {entry['override']})")
    if args.output:
        local = Path(__file__).resolve().parents[1] / 'local'
        if not args.output.resolve().is_relative_to(local.resolve()):
            parser.error('Output must remain under local/')
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps({'decoded': decoded, 'failures': failures}, indent=1))
    return 1 if failures else 0


if __name__ == '__main__':
    raise SystemExit(main())
