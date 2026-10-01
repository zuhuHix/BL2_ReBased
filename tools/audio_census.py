#!/usr/bin/env python3
"""Read-only census of the installed Wwise audio containers (AI-assisted, clean room).

What it reads: the `.pck` files under WillowGame/CookedPCConsole (and the language folders):
a package header with a language table, a bank table and a streaming-sound table; each bank
entry is a Wwise sound bank (BKHD / DIDX / DATA / HIRC / STID chunks). Nothing is decoded; the
only audio bytes it ever copies are raw `.wem` files, and only under the repository's ignored
`local/` directory (`extract`).

Every layout assumption below is a hypothesis fitted to the installed files and is checked by a
structural oracle (sizes tile exactly, counts match, ids resolve); the `census` command prints the
oracle results so a failure is visible instead of silent. Nothing here is a Wwise specification.

  python tools/audio_census.py census  [--game DIR] [--out local/audio/census.json]
  python tools/audio_census.py resolve NAME_OR_0xID ...        # event -> actions -> sounds -> media
  python tools/audio_census.py extract NAME_OR_0xID ... [--out local/audio/wem]
"""
import argparse
import collections
import json
import os
import struct
import sys

HIRC_TYPES = {1: 'settings', 2: 'sound', 3: 'action', 4: 'event', 5: 'random_sequence', 6: 'switch',
              7: 'actor_mixer', 8: 'bus', 9: 'layer_container'}
CONTAINER_TYPES = (5, 6, 7, 9)
PLAY_ACTION = 0x0403  # UNVERIFIED name: the action type seen on every event's actions in this install
FORMAT_NAMES = {0x0001: 'pcm', 0x0002: 'ms_adpcm', 0x0011: 'ima_adpcm', 0x0069: 'ima_adpcm_wwise',
                0xFFFE: 'wave_extensible', 0xFFFF: 'wwise_vorbis'}


def fnv1_32(name):
    """32-bit FNV-1 of the lower-cased name. Verified against AkEvent.ShortId and STID bank ids."""
    h = 2166136261
    for c in name.lower().encode('utf-8'):
        h = (h * 16777619) & 0xffffffff
        h ^= c
    return h


def u32(b, o):
    return struct.unpack_from('<I', b, o)[0]


def parse_pck_header(fh, file_size):
    """Return the AKPK header, its tables and the oracle results for one .pck file."""
    fh.seek(0)
    head = fh.read(28)
    if len(head) < 28 or head[:4] != b'AKPK':
        raise ValueError('not an AKPK file')
    header_size, version, lang_size, bank_size, sound_size, ext_size = struct.unpack_from('<6I', head, 4)
    fh.seek(28)
    rest = fh.read(lang_size + bank_size + sound_size + ext_size)
    oracle = {'header_size_matches': 8 + header_size == 28 + len(rest) and len(rest) == lang_size + bank_size + sound_size + ext_size}
    langs = {}
    count = u32(rest, 0)
    for i in range(count):
        off, lang_id = struct.unpack_from('<II', rest, 4 + 8 * i)
        end = off
        while rest[end:end + 2] != b'\0\0':
            end += 2
        langs[lang_id] = rest[off:end].decode('utf-16le')
    pos = lang_size
    tables = {}
    for name, size in (('banks', bank_size), ('sounds', sound_size)):
        n = u32(rest, pos)
        oracle[name + '_count_matches_size'] = 4 + 20 * n == size
        entries = []
        for i in range(n):
            ident, block, length, start, lang = struct.unpack_from('<5I', rest, pos + 4 + 20 * i)
            entries.append({'id': ident, 'offset': start * block, 'size': length, 'lang': lang, 'block': block})
        tables[name] = entries
        pos += size
    spans = sorted((e['offset'], e['size']) for t in tables.values() for e in t)
    cursor, gaps, overlaps = 8 + header_size, 0, 0
    for off, size in spans:
        if off < cursor:
            overlaps += 1
        else:
            gaps += off - cursor
        cursor = max(cursor, off + size)
    oracle['entries_tile_file'] = gaps == 0 and overlaps == 0 and cursor == file_size
    oracle['gap_bytes'], oracle['overlaps'] = gaps, overlaps
    return {'version': version, 'header_size': header_size, 'languages': langs, **tables, 'oracle': oracle}


def read_bank(fh, base, size):
    """Walk the chunks of one bank; returns chunk offsets and whether they tile the bank exactly."""
    chunks, pos = {}, 0
    while pos + 8 <= size:
        fh.seek(base + pos)
        tag, length = struct.unpack('<4sI', fh.read(8))
        chunks[tag.decode('latin1')] = (base + pos + 8, length)
        pos += 8 + length
    return chunks, pos == size


def read_chunk(fh, chunks, tag):
    if tag not in chunks:
        return b''
    fh.seek(chunks[tag][0])
    return fh.read(chunks[tag][1])


def parse_hirc(data):
    """Return (objects, tiles). Each object is (type, id, body); body excludes the id."""
    objects, pos = [], 4
    if len(data) < 4:
        return objects, len(data) == 0
    for _ in range(u32(data, 0)):
        kind, length = struct.unpack_from('<BI', data, pos)
        body = data[pos + 5:pos + 5 + length]
        objects.append((kind, u32(body, 0), body))
        pos += 5 + length
    return objects, pos == len(data)


def decode_event(body):
    n = u32(body, 4)
    return list(struct.unpack_from('<%dI' % n, body, 8)), len(body) == 8 + 4 * n


def decode_action(body):
    # id, u16 type, u32 target, ...; a play action is 17 bytes: id type target 00 00 curve bank_id
    kind, target = struct.unpack_from('<HI', body, 4)
    bank = u32(body, 13) if kind == PLAY_ACTION and len(body) == 17 else None
    return {'type': kind, 'target': target, 'bank': bank, 'play_layout_ok': kind != PLAY_ACTION or len(body) == 17}


def decode_sound(body):
    plugin, stream, source = struct.unpack_from('<3I', body, 4)
    return {'plugin': plugin, 'stream': stream, 'source': source}


def container_children(body, known_ids):
    """UNVERIFIED method: a container's child list is a u32 count followed by that many ids; rather than
    parse the variable-length node parameters, take the longest such run whose ids all exist in the bank."""
    best = []
    for pos in range(4, len(body) - 4):
        n = u32(body, pos)
        if not 1 <= n <= 256 or pos + 4 + 4 * n > len(body):
            continue
        ids = struct.unpack_from('<%dI' % n, body, pos + 4)
        if all(i in known_ids for i in ids) and len(ids) > len(best):
            best = list(ids)
    return best


def riff_info(head):
    """Header facts of a RIFF/WAVE blob (`head` = its first bytes). Wwise Vorbis keeps its sample count
    at +0x18 of the fmt data (UNVERIFIED: checked only against AkEvent durations)."""
    if head[:4] != b'RIFF' or head[8:12] != b'WAVE':
        return {'riff': False}
    info = {'riff': True}
    pos = 12
    while pos + 8 <= len(head):
        tag, length = struct.unpack_from('<4sI', head, pos)
        if tag == b'fmt ' and pos + 8 + min(length, 16) <= len(head):
            tagid, channels, rate, avg, align, bits = struct.unpack_from('<HHIIHH', head, pos + 8)
            info.update(format_tag=tagid, codec=FORMAT_NAMES.get(tagid, 'unknown'), channels=channels,
                        sample_rate=rate, avg_bytes_per_sec=avg, fmt_size=length)
            if tagid == 0xFFFF and length >= 0x1c and pos + 8 + 0x1c <= len(head):
                info['vorbis_samples'] = u32(head, pos + 8 + 0x18)
                if rate:
                    info['duration_s'] = round(info['vorbis_samples'] / rate, 4)
        elif tag == b'data':
            info['data_size'] = length
            break
        pos += 8 + length + (length & 1)
    return info


def scan_pcks(root):
    """rel path -> parsed AKPK header (plus path and size) for every .pck under `root`."""
    found = {}
    for folder, _, files in os.walk(root):
        for f in sorted(files):
            if f.lower().endswith('.pck'):
                path = os.path.join(folder, f)
                size = os.path.getsize(path)
                with open(path, 'rb') as fh:
                    found[os.path.relpath(path, root).replace(os.sep, '/')] = {'path': path, 'size': size, **parse_pck_header(fh, size)}
    return found


class Install:
    """The base-game .pck files under WillowGame/CookedPCConsole, with lazy bank access."""

    def __init__(self, game):
        self.game = game
        self.cooked = os.path.join(game, 'WillowGame', 'CookedPCConsole')
        self.pcks = scan_pcks(self.cooked)
        self._banks = None

    def banks(self):
        """List of dicts: pck, id, lang, chunk oracle, DIDX ids, HIRC objects and decoded events/actions/sounds."""
        if self._banks is not None:
            return self._banks
        out = []
        for rel, pck in self.pcks.items():
            if not pck['banks']:
                continue
            with open(pck['path'], 'rb') as fh:
                for entry in pck['banks']:
                    chunks, tiles = read_chunk_table(fh, entry)
                    hirc, hirc_tiles = parse_hirc(read_chunk(fh, chunks, 'HIRC'))
                    bkhd = read_chunk(fh, chunks, 'BKHD')
                    didx_raw = read_chunk(fh, chunks, 'DIDX')
                    didx = {u32(didx_raw, i): (u32(didx_raw, i + 4), u32(didx_raw, i + 8)) for i in range(0, len(didx_raw) - 11, 12)}
                    stid = read_chunk(fh, chunks, 'STID')
                    names = []
                    if len(stid) >= 8:
                        pos = 8
                        for _ in range(u32(stid, 4)):
                            ident, n = struct.unpack_from('<IB', stid, pos)
                            names.append((ident, stid[pos + 5:pos + 5 + n].decode('latin1')))
                            pos += 5 + n
                    out.append({'pck': rel, 'entry': entry, 'chunks': chunks, 'chunks_tile': tiles, 'hirc': hirc,
                                'hirc_tiles': hirc_tiles, 'bkhd_version': u32(bkhd, 0) if bkhd else None,
                                'bkhd_id': u32(bkhd, 4) if bkhd else None, 'didx': didx,
                                'didx_exact': len(didx_raw) == 12 * len(didx), 'names': names,
                                'data_len': chunks.get('DATA', (0, 0))[1]})
        self._banks = out
        return out

    def streaming(self):
        """(lang id, source id) -> (pck, entry) for the streaming tables."""
        res = {}
        for rel, pck in self.pcks.items():
            for e in pck['sounds']:
                res[(e['lang'], e['id'])] = (rel, e)
        return res


def read_chunk_table(fh, entry):
    return read_bank(fh, entry['offset'], entry['size'])


def census(inst):
    """Counts and oracle results over every container. Pure read; deterministic."""
    result = {'pck_files': {}, 'oracles': {}}
    for rel, pck in inst.pcks.items():
        result['pck_files'][rel] = {'bytes': pck['size'], 'languages': pck['languages'], 'banks': len(pck['banks']),
                                    'streaming_sounds': len(pck['sounds']), 'oracle': pck['oracle']}
    dlc_root = os.path.join(inst.game, 'DLC')
    if os.path.isdir(dlc_root):
        dlc = scan_pcks(dlc_root)
        result['dlc_pck_files'] = {'count': len(dlc), 'bytes': sum(p['size'] for p in dlc.values()),
                                   'all_entries_tile_file': all(p['oracle']['entries_tile_file'] for p in dlc.values()),
                                   'languages': sorted({n for p in dlc.values() for n in p['languages'].values()}),
                                   'banks': sum(len(p['banks']) for p in dlc.values()),
                                   'streaming_sounds': sum(len(p['sounds']) for p in dlc.values())}
    banks = inst.banks()
    stream = inst.streaming()
    types = collections.Counter()
    ok = collections.Counter()
    events = {}
    for b in banks:
        ok['banks'] += 1
        ok['bank_chunks_tile'] += b['chunks_tile']
        ok['bkhd_id_equals_table_id'] += b['bkhd_id'] == b['entry']['id']
        ok['didx_whole_entries'] += b['didx_exact']
        ok['hirc_tiles'] += b['hirc_tiles']
        for ident, name in b['names']:
            ok['stid_names'] += 1
            ok['stid_fnv_matches_bank_id'] += fnv1_32(name) == ident and ident == b['bkhd_id']
        ids = {i for _, i, _ in b['hirc']}
        for kind, ident, body in b['hirc']:
            types[HIRC_TYPES.get(kind, kind)] += 1
            if kind == 4:
                acts, size_ok = decode_event(body)
                ok['events'] += 1
                ok['event_size_matches_action_count'] += size_ok
                events.setdefault(ident, []).append(b['pck'])
                for a in acts:
                    ok['event_actions'] += 1
                    ok['event_actions_resolve_in_bank'] += a in ids
            elif kind == 3:
                act = decode_action(body)
                ok['actions'] += 1
                ok['action_play_layout'] += act['play_layout_ok']
                if act['type'] == PLAY_ACTION:
                    ok['play_actions'] += 1
                    ok['play_target_resolves_in_bank'] += act['target'] in ids
                    ok['play_bank_equals_owner'] += act['bank'] == b['bkhd_id']
            elif kind == 2:
                snd = decode_sound(body)
                ok['sounds'] += 1
                ok['sound_stream_%d' % snd['stream']] += 1
                if snd['stream'] == 0:
                    ok['sound_source_in_didx'] += snd['source'] in b['didx']
                else:
                    ok['sound_source_in_stream_table'] += (b['entry']['lang'], snd['source']) in stream or (0, snd['source']) in stream
    result['bank_count'] = len(banks)
    result['hirc_object_types'] = dict(types)
    result['distinct_event_ids'] = len(events)
    result['oracles'] = dict(ok)
    return result


def _event_id(token):
    return int(token, 16) if token.lower().startswith('0x') else fnv1_32(token)


def resolve(inst, token):
    """Event name/id -> every bank that holds it -> play actions -> leaf sounds -> media location."""
    eid = _event_id(token)
    stream = inst.streaming()
    rows = []
    for b in inst.banks():
        index = {i: (k, body) for k, i, body in b['hirc']}
        if eid not in index or index[eid][0] != 4:
            continue
        acts, _ = decode_event(index[eid][1])
        for a in acts:
            if a not in index or index[a][0] != 3:
                rows.append({'bank': b['bkhd_id'], 'pck': b['pck'], 'action': a, 'unresolved': True})
                continue
            act = decode_action(index[a][1])
            row = {'bank': b['bkhd_id'], 'pck': b['pck'], 'action': a, 'action_type': act['type'], 'target': act['target'],
                   'target_kind': HIRC_TYPES.get(index.get(act['target'], (None,))[0], 'missing'), 'sounds': []}
            for sid in leaf_sounds(index, act['target']):
                snd = decode_sound(index[sid][1])
                loc = media_location(b, snd, stream)
                row['sounds'].append({'sound': sid, **snd, **loc})
            rows.append(row)
    return {'event_id': eid, 'rows': rows}


def leaf_sounds(index, target, depth=0):
    kind, body = index.get(target, (None, b''))
    if kind == 2:
        return [target]
    if kind in CONTAINER_TYPES and depth < 8:
        found = []
        for child in container_children(body, set(index)):
            found += leaf_sounds(index, child, depth + 1)
        return found
    return []


def media_location(bank, snd, stream):
    if snd['stream'] == 0:
        if snd['source'] in bank['didx'] and 'DATA' in bank['chunks']:
            off, size = bank['didx'][snd['source']]
            # chunk offsets are absolute file offsets; DIDX offsets are relative to the DATA chunk body
            return {'where': 'bank', 'pck': bank['pck'], 'offset': bank['chunks']['DATA'][0] + off, 'size': size}
        return {'where': 'missing'}
    hit = stream.get((bank['entry']['lang'], snd['source'])) or stream.get((0, snd['source']))
    if hit:
        return {'where': 'stream', 'pck': hit[0], 'offset': hit[1]['offset'], 'size': hit[1]['size']}
    return {'where': 'missing'}


def read_media(inst, loc):
    with open(inst.pcks[loc['pck']]['path'], 'rb') as fh:
        fh.seek(loc['offset'])
        return fh.read(loc['size'])


def main(argv=None):
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('command', choices=['census', 'resolve', 'extract'])
    ap.add_argument('events', nargs='*')
    ap.add_argument('--game', default=os.environ.get('OPENWILLOW_BL2'))
    ap.add_argument('--out')
    args = ap.parse_args(argv)
    if not args.game:
        sys.exit('set OPENWILLOW_BL2 or pass --game')
    out_default = os.path.join(root, 'local', 'audio', 'wem' if args.command == 'extract' else 'census.json')
    out = os.path.abspath(args.out or out_default)
    if not out.startswith(os.path.join(root, 'local') + os.sep):
        sys.exit('output must stay under the repository local/ directory')
    inst = Install(args.game)
    if args.command == 'census':
        os.makedirs(os.path.dirname(out), exist_ok=True)
        res = census(inst)
        with open(out, 'w') as f:
            json.dump(res, f, indent=1)
        print(json.dumps({k: v for k, v in res.items() if k != 'pck_files'}, indent=1))
        return
    for token in args.events:
        res = resolve(inst, token)
        if args.command == 'resolve':
            print(json.dumps(res, indent=1))
            continue
        os.makedirs(out, exist_ok=True)
        for row in res['rows']:
            for s in row.get('sounds', []):
                if s.get('where') in ('bank', 'stream'):
                    path = os.path.join(out, '%d.wem' % s['source'])
                    with open(path, 'wb') as f:
                        f.write(read_media(inst, s))
                    print(path, s['size'])


if __name__ == '__main__':
    main()
