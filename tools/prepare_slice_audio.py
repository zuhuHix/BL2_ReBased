#!/usr/bin/env python3
"""Write the host's sound map (AI-assisted, clean room). See docs/verification/SLICE_AUDIO_CHAIN.md section 5.

Reads the ow-audio-v1 manifest (tools/audio_slice_chain.py) and PCM WAV files already decoded into an ignored directory,
and writes local/slice/audio_play.json for the host's -owaudioplay=. This tool decodes nothing and calls no decoder: a
role without a checked WAV stays out of the map and is listed under "unresolved". The "timing" block holds the reload notify
offsets (clip out, clip in) per arms clip, from the same manifest, for the host to time the reload's sounds (section 5). Dialog lines
get one role each, "line:<event tag>" (section 5c).

  python tools/prepare_slice_audio.py [--manifest local/slice/audio.json] [--decoded-dir local/audio/decoded]
  python tools/prepare_slice_audio.py --tone-only     # a synthetic test tone only, no game data

A WAV is accepted only when it is 16-bit PCM and its length matches the manifest's header duration. That is a length
check: it does not show the sound is the right one, and nothing here means anyone has listened to it.

Gains (section 7): with --game (or OPENWILLOW_BL2) each file also gets the game's static level in dB, the sum of the volumes on its Wwise
sound, containers, actor-mixers and buses, read from the installed sound banks; the map's "gains" block holds one per file, in the order of
"roles", and "gain_notes" says per role where they came from. Without the install every gain is 0 dB. RTPCs, states, ducking, 3D falloff
and random volume ranges are not applied.
"""
import argparse
import json
import math
import os
import re
import struct
import sys
import wave

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import audio_census  # noqa: E402  (the bank readers; reads the install only when --game is given)

PLAY_SCHEMA = 'ow-audio-play-v1'
# Role -> manifest key. Roles with no key are not resolved from evidence yet, so they stay silent rather than guess.
# hit and phaselock_cast come from one live session and are UNVERIFIED (SLICE_AUDIO_CHAIN.md section 5). hit's many files are one event's per-surface
# sources, presumably, and the host picks at random, so a hit can sound like any surface until the switch value is known.
# equip is the lent pistol's PickupAndEquipSounds event; the host plays it when a gun is drawn (UNVERIFIED that the game does the same).
# door_open / door_close are the slice mover door's two keys (SLICE_AUDIO_CHAIN.md section 5b): which one is the opening sound is UNVERIFIED.
ROLE_KEYS = {'fire': 'weapon:fire', 'hit': 'role:hit', 'phaselock_cast': 'role:phaselock_cast', 'pickup': None, 'equip': 'weapon:equip',
             'door_open': 'door:open', 'door_close': 'door:close'}
# The lent pistol's reload sounds are not one key each: the manifest has one reload-anim key per (arms clip, notify time). The trigger's
# text gives the clip and the notify time; the AkEvent's name gives clip out or clip in. Both come from the manifest, nothing is typed in here.
RELOAD_NOTIFY = re.compile(r'^reload anim (\S+) @([0-9.]+)s$')
RELOAD_ROLES = {'reload_out': 'Clip_Out', 'reload_in': 'Clip_In'}
RELOAD_TIMING_KEY = {'reload_out': 'clip_out_s', 'reload_in': 'clip_in_s'}
# Dialog lines (SLICE_AUDIO_CHAIN.md section 5c): one role per manifest dialog entry, "line:<event tag>", from the mission's dialog group
# (dialog:) and Marcus's own group (marcus_group:). The host does not choose a line: the stock dialog in the VM does, and the host plays the
# role of the event tag it chose. A tag with no decoded file stays unresolved, and that line stays silent.
LINE_PREFIXES = ('dialog:', 'marcus_group:')
DURATION_TOLERANCE_S = 0.05
# The host's limits (OpenWillowAudio.h; a test compares them): a file past them would be mapped here and then refused there.
MIN_SAMPLE_RATE = 8000
MAX_SAMPLE_RATE = 96000
MAX_FILE_BYTES = 64 * 1024 * 1024
MAX_TOTAL_BYTES = 256 * 1024 * 1024


def write_tone(path, seconds=1.0, rate=48000, hz=440.0):
    """A plain sine at low level: synthetic, so the host path can be run without game data."""
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    frames = b''.join(struct.pack('<h', int(6000 * math.sin(2 * math.pi * hz * i / rate))) for i in range(int(seconds * rate)))
    with wave.open(path, 'wb') as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(rate)
        out.writeframes(frames)


def read_wav_seconds(path):
    """The length of a WAV by the same rules the host's ParseWav applies, from the bytes actually there (the `wave` module trusts the
    header, so a truncated file would pass). Returns (seconds, None) or (None, reason)."""
    try:
        size = os.path.getsize(path)
        if size > MAX_FILE_BYTES:
            return None, 'file is %d bytes, the limit is %d' % (size, MAX_FILE_BYTES)
        with open(path, 'rb') as fh:
            data = fh.read()
    except OSError as error:
        return None, str(error)
    if len(data) < 12 or data[:4] != b'RIFF' or data[8:12] != b'WAVE':
        return None, 'not a RIFF WAVE file'
    riff_end = 8 + struct.unpack_from('<I', data, 4)[0]
    if riff_end < 12:
        return None, 'RIFF size too small'
    if riff_end > len(data):
        return None, 'RIFF size runs past the end of the file'
    at, fmt, data_bytes = 12, None, None
    while at + 8 <= riff_end:
        tag, chunk = data[at:at + 4], struct.unpack_from('<I', data, at + 4)[0]
        # RIFF pads an odd chunk to a word; the pad is not in its size, but it is inside the RIFF size.
        following = at + 8 + chunk + (chunk & 1)
        if following > riff_end:
            return None, 'chunk or its pad runs past the RIFF size'
        if tag == b'fmt ':
            if fmt is not None:
                return None, 'more than one fmt chunk'
            if chunk < 16:
                return None, 'fmt chunk too short'
            fmt = struct.unpack_from('<HHIIHH', data, at + 8)
            if fmt[0] != 1 or fmt[5] != 16 or fmt[1] not in (1, 2):
                return None, 'not 16-bit mono or stereo PCM'
            if not MIN_SAMPLE_RATE <= fmt[2] <= MAX_SAMPLE_RATE:
                return None, 'unsupported sample rate %d' % fmt[2]
        elif tag == b'data':
            if fmt is None:
                return None, 'data before fmt'
            if data_bytes is not None:
                return None, 'more than one data chunk'
            if chunk == 0 or chunk % (2 * fmt[1]):
                return None, 'data is not whole sample frames'
            data_bytes = chunk
        at = following
    # Stops short of the end only for fewer than 8 bytes left: a chunk header cut off inside the declared size.
    if at < riff_end:
        return None, 'partial chunk at the end of the RIFF size'
    if data_bytes is None:
        return None, 'no data chunk'
    return data_bytes / (2 * fmt[1] * fmt[2]), None


def check_wav(path, expected_duration):
    """None when the WAV is usable, else the reason it is not."""
    seconds, reason = read_wav_seconds(path)
    if reason:
        return reason
    if expected_duration is not None and abs(seconds - expected_duration) > DURATION_TOLERANCE_S:
        return 'length %.3f s, header says %.3f s' % (seconds, expected_duration)
    return None


def entry_files(media_list, decoded_dir, used):
    """The usable WAVs among media_list (paths), the reasons the others were refused, and the bytes held with them added to `used`."""
    files, reasons = [], []
    for media in media_list:
        path = os.path.abspath(os.path.join(decoded_dir, '%s.wav' % media['source']))
        reason = check_wav(path, media.get('duration_s'))
        if not reason and used + os.path.getsize(path) > MAX_TOTAL_BYTES:
            reason = 'the map would hold more than %d bytes' % MAX_TOTAL_BYTES
        if reason:
            reasons.append('%s: %s' % (media['source'], reason))
        else:
            used += os.path.getsize(path)
            files.append(path)
    return files, reasons, used


def reload_notifies(manifest):
    """(role, arms clip name, notify time in seconds) for each reload notify the manifest records, with its manifest entry."""
    found = []
    for entry in manifest['entries']:
        match = RELOAD_NOTIFY.match((entry.get('trigger') or {}).get('role', ''))
        if not match:
            continue
        name = (entry.get('ak_event') or {}).get('wwise_name', '')
        for role, suffix in RELOAD_ROLES.items():
            if name.endswith(suffix):
                found.append((role, match.group(1), float(match.group(2)), entry))
    return found


def reload_timing(manifest):
    """{clip name: {clip_out_s, clip_in_s}} for each arms clip that has both notifies, in seconds from the clip's start. A clip with two
    notifies of one kind, or only one of the two, is left out (the host then schedules nothing for it)."""
    times, clashes = {}, set()
    for role, clip, seconds, _ in reload_notifies(manifest):
        slot = times.setdefault(clip, {})
        if RELOAD_TIMING_KEY[role] in slot:
            clashes.add(clip)
        slot[RELOAD_TIMING_KEY[role]] = seconds
    return {clip: slot for clip, slot in times.items() if clip not in clashes and len(slot) == 2}


def build_play_map(manifest, decoded_dir):
    entries = {entry['key']: entry for entry in manifest['entries']}
    roles, unresolved, used = {}, {}, 0
    for role, key in ROLE_KEYS.items():
        if key is None:
            unresolved[role] = 'no sound identified from evidence yet'
            continue
        entry = entries.get(key)
        if entry is None:
            unresolved[role] = 'manifest has no %s' % key
            continue
        files, reasons, used = entry_files(entry['media'], decoded_dir, used)
        if files:
            roles[role] = files
        else:
            unresolved[role] = '; '.join(reasons) or 'entry has no media'
    notifies = reload_notifies(manifest)
    for role in RELOAD_ROLES:
        media, seen = [], set()
        for found_role, _, _, entry in notifies:
            if found_role != role:
                continue
            for item in entry['media']:
                if item['source'] not in seen:   # the same sound can sit under both clips (the same source, once)
                    seen.add(item['source'])
                    media.append(item)
        if not media:
            unresolved[role] = 'manifest has no reload notify for %s' % RELOAD_ROLES[role]
            continue
        files, reasons, used = entry_files(media, decoded_dir, used)
        if files:
            roles[role] = files
        else:
            unresolved[role] = '; '.join(reasons) or 'entry has no media'
    # dialog: before marcus_group:, the order of the host's own manifest lookup, so a tag under both prefixes takes the dialog: entry.
    line_entries = [(prefix, entry) for prefix in LINE_PREFIXES for entry in manifest['entries'] if entry['key'].startswith(prefix)]
    for prefix, entry in line_entries:
        role = 'line:' + entry['key'][len(prefix):]
        if role in roles or role in unresolved:
            continue
        files, reasons, used = entry_files(entry['media'], decoded_dir, used)
        if files:
            roles[role] = files
        else:
            unresolved[role] = '; '.join(reasons) or 'entry has no media'
    return roles, unresolved


# ---- The game's static mix (SLICE_AUDIO_CHAIN.md section 7) ---------------------------------------------------------------------
# Every Wwise object a sound plays through (the sound, its containers and actor-mixers up the parent chain, then its output bus and that
# bus's parents) can carry a volume in dB. The game's level for a sound is the sum of those volumes. Layouts below were fitted on this
# install's banks (bank version 62) and checked over the whole install, not taken from a spec; the oracles are in section 7.
VOLUME_PROP = 0              # property id 0: values -96.3 .. -1 over the install, -96.3 being Wwise's volume floor
CONTAINER_KINDS = (5, 6, 7, 9)
SOUND_KIND, SWITCH_KIND, BUS_KIND = 2, 6, 8
# The weapon events' switch containers pick a child by a two-value switch. FNV-1 of these names gives those two values: the player's own
# sounds take the first-person child; anything else takes the container's default (third person in every slice case).
FIRST_PERSON = 'firstperson'
PLAYER_ROLES = ('fire', 'hit', 'phaselock_cast', 'pickup', 'equip', 'reload_out', 'reload_in')   # the 2D roles (section 5d)
# Gains the host accepts (OpenWillowAudio.h; a test compares them). Wwise allows +12 dB on one object; a sum below -144 dB is silence in 16 bits.
MIN_GAIN_DB = -144.0
MAX_GAIN_DB = 12.0


def _u32(body, at):
    return struct.unpack_from('<I', body, at)[0]


def prop_bundle(body, at):
    """{prop id: value} and {prop id: (min, max)} of the two property bundles at `at`: u8 count, the ids, a float per id; then the same with
    a (min, max) pair per id (random ranges)."""
    n = body[at]
    ids = body[at + 1:at + 1 + n]
    values = struct.unpack_from('<%df' % n, body, at + 1 + n)
    at += 1 + 5 * n
    m = body[at]
    ranged_ids = body[at + 1:at + 1 + m]
    pairs = struct.unpack_from('<%df' % (2 * m), body, at + 1 + m)
    return dict(zip(ids, values)), {ranged_ids[j]: (pairs[2 * j], pairs[2 * j + 1]) for j in range(m)}


def node_links(kind, body, index, buses):
    """(output bus, parent, props, ranged, shifted) of a sound or container. The parent id follows the output bus id; for a sound the pair
    sits after the source description, whose length depends on the stream type; for a container right after its own id and two effect
    bytes. An object with effects has a longer effect block: then the pair is searched for, and `shifted` says so."""
    base = {0: 35, 1: 27, 2: 35}.get(_u32(body, 8)) if kind == SOUND_KIND else 10
    if base is None:
        raise ValueError('unknown stream type %d' % _u32(body, 8))
    for at in range(base, min(base + 64, len(body) - 4)):
        bus, parent = _u32(body, at - 4), _u32(body, at)
        parent_ok = parent == 0 or index.get(parent, (None,))[0] in CONTAINER_KINDS
        if parent_ok and (bus == 0 or bus in buses) and (parent or bus):
            props, ranged = prop_bundle(body, at + 6)    # two flag bytes, then the bundles
            return bus, parent, props, ranged, at != base
    raise ValueError('no parent or bus found')


def bus_links(body):
    """(parent bus, props) of a bus: its id, its parent's id, then one property bundle."""
    n = body[8]
    return _u32(body, 4), dict(zip(body[9:9 + n], struct.unpack_from('<%df' % n, body, 9 + n)))


def switch_values(body, index):
    """{child id: switch value} of a switch container, and its default value. Children are the container's child list; after it a u32
    count of switch values, each a u32 value, a u32 count and that many child ids. The group and default value sit just before the list."""
    children = audio_census.container_children(body, set(index))
    for at in range(4, len(body) - 4):
        if _u32(body, at) == len(children) and list(struct.unpack_from('<%dI' % len(children), body, at + 4)) == children:
            break
    else:
        return {}, None
    default = _u32(body, at - 5)
    at += 4 + 4 * len(children)
    found = {}
    for _ in range(_u32(body, at)):
        value, n = struct.unpack_from('<II', body, at + 4)
        for child in struct.unpack_from('<%dI' % n, body, at + 12):
            found[child] = value
        at += 8 + 4 * n
    return found, default


def volume_chain(sound, index, global_index, buses):
    """The objects a sound plays through, sound first, as dicts (kind, id, volume_db, ranged, shifted, switch value it was reached by)."""
    chain, bus, current, below = [], 0, sound, None
    while current:
        kind, body = index[current]
        out_bus, parent, props, ranged, shifted = node_links(kind, body, index, buses)
        if below is not None and kind == SWITCH_KIND:
            values, default = switch_values(body, index)
            chain[-1]['switch'] = (values.get(below), default)
        chain.append({'kind': kind, 'id': current, 'volume_db': props.get(VOLUME_PROP, 0.0), 'ranged': VOLUME_PROP in ranged, 'shifted': shifted, 'switch': None})
        bus = bus or out_bus
        below, current = current, parent
    while bus:
        parent, props = bus_links((index.get(bus) or global_index[bus])[1])
        chain.append({'kind': BUS_KIND, 'id': bus, 'volume_db': props.get(VOLUME_PROP, 0.0), 'ranged': False, 'shifted': False, 'switch': None})
        bus = parent
    return chain


def pick_by_switch(chains, perspective):
    """Of several chains for one source under one play action, those reached through the wanted switch value (`perspective`, a value id),
    or through each switch's default when `perspective` is None. Chains with no switch on the way are kept either way."""
    kept = []
    for chain in chains:
        choices = [node['switch'] for node in chain if node['switch']]
        if all(value == (perspective if perspective is not None else default) for value, default in choices):
            kept.append(chain)
    return kept


def coherent_sum_db(levels):
    """Two play actions on one source start the same signal together, so their amplitudes add."""
    total = sum(10 ** (db / 20) for db in levels)
    return 20 * math.log10(total) if total > 0 else MIN_GAIN_DB


def source_gain(rows, source, perspective):
    """(gain in dB, notes) for one source from resolved rows: {bank: {action: [chain, ...]}}. Banks must agree; actions add up."""
    per_bank, notes = {}, set()
    for bank, actions in sorted(rows.items()):
        levels = []
        for action, chains in sorted(actions.items()):
            kept = pick_by_switch(chains, perspective) or chains
            values = sorted({round(sum(node['volume_db'] for node in chain), 2) for chain in kept})
            if len(values) > 1:
                notes.add('several levels under one action (%s dB); the highest is used' % ', '.join('%.1f' % v for v in values))
            if any(node['ranged'] for chain in kept for node in chain):
                notes.add('a random volume range is not applied')
            if any(node['shifted'] for chain in kept for node in chain):
                notes.add('an object with effects (its layout was searched, UNVERIFIED)')
            levels.append(values[-1])
        if len(levels) > 1:
            notes.add('%d play actions on this source, summed' % len(levels))
        per_bank[bank] = coherent_sum_db(levels)
    if not per_bank:
        return None, ['source %s not reached from the event' % source]
    if len({round(v, 2) for v in per_bank.values()}) > 1:
        notes.add('banks disagree (%s); the first bank by id is used' % ', '.join('%.1f' % v for v in per_bank.values()))
    return round(next(iter(per_bank.values())), 2), sorted(notes)


class GameMix:
    """The installed game's banks, read with tools/audio_census.py's readers. Reads only; writes nothing."""

    def __init__(self, game):
        self.install = audio_census.Install(game)
        self.indexes = {bank['bkhd_id']: {i: (kind, body) for kind, i, body in bank['hirc']} for bank in self.install.banks()}
        self.global_index = {}
        for index in self.indexes.values():
            for i, item in index.items():
                self.global_index.setdefault(i, item)
        self.buses = {i for i, (kind, _) in self.global_index.items() if kind == BUS_KIND}

    def entry_gains(self, entry, role):
        """{source: (gain dB or None, notes)} for a manifest entry, as the role hears it (first person for the player's own roles)."""
        perspective = audio_census.fnv1_32(FIRST_PERSON) if role in PLAYER_ROLES else None
        short_id = (entry.get('ak_event') or {}).get('short_id')
        if short_id is None:
            return {media['source']: (None, ['the manifest entry names no AkEvent']) for media in entry['media']}
        resolved = audio_census.resolve(self.install, '0x%x' % short_id)
        rows = {}
        for row in resolved['rows']:
            for sound in row.get('sounds', []):
                index = self.indexes[row['bank']]
                chain = volume_chain(sound['sound'], index, self.global_index, self.buses)
                rows.setdefault(sound['source'], {}).setdefault(row['bank'], {}).setdefault(row['action'], []).append(chain)
        return {media['source']: source_gain(rows.get(media['source'], {}), media['source'], perspective) for media in entry['media']}


def role_entries(manifest):
    """role -> the manifest entries its files come from, in the order build_play_map takes them (fixed roles, reload roles, lines)."""
    entries = {entry['key']: entry for entry in manifest['entries']}
    found = {role: [entries[key]] for role, key in ROLE_KEYS.items() if key and key in entries}
    for role, _, _, entry in reload_notifies(manifest):
        found.setdefault(role, []).append(entry)
    for prefix in LINE_PREFIXES:
        for entry in manifest['entries']:
            if entry['key'].startswith(prefix):
                found.setdefault('line:' + entry['key'][len(prefix):], [entry])
    return found


def build_gains(roles, manifest, mix):
    """gains: {role: [dB per file, in the order of roles[role]]}; notes: {role: where the numbers came from}. Without game data (`mix` None)
    every gain is 0 dB and the note says so. A file whose source the banks do not give a level for gets 0 dB, also with a note."""
    gains, notes = {}, {}
    sources = role_entries(manifest) if manifest else {}
    for role, files in roles.items():
        if mix is None or role not in sources:
            gains[role] = [0.0] * len(files)
            notes[role] = 'unity: no game data given (host default, UNVERIFIED)' if mix is None else 'unity: not a game sound (host default)'
            continue
        levels, why = {}, set()
        for entry in sources[role]:
            for source, (db, entry_notes) in mix.entry_gains(entry, role).items():
                if source not in levels and db is not None:
                    levels[source] = db
                why.update(entry_notes)
        role_gains = []
        for path in files:
            source = int(os.path.splitext(os.path.basename(path))[0])
            db = levels.get(source)
            if db is None:
                why.add('source %d: no level in the banks, unity used' % source)
                db = 0.0
            role_gains.append(min(MAX_GAIN_DB, max(MIN_GAIN_DB, db)))
        gains[role] = role_gains
        perspective = ' (first person where a switch picks)' if role in PLAYER_ROLES else ''
        notes[role] = 'game data: summed object and bus volumes%s' % perspective + ('; ' + '; '.join(sorted(why)) if why else '')
    return gains, notes


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--manifest', default='local/slice/audio.json')
    ap.add_argument('--decoded-dir', default='local/audio/decoded')
    ap.add_argument('--game', default=os.environ.get('OPENWILLOW_BL2'),
                    help='the installed game (default OPENWILLOW_BL2): its sound banks give each file its gain; without it every gain is 0 dB')
    ap.add_argument('--out', default='local/slice/audio_play.json')
    ap.add_argument('--tone', default='local/audio/test_tone.wav', help='synthetic tone, always written, role test_tone')
    ap.add_argument('--tone-only', action='store_true', help='map only the synthetic tone; reads no game data')
    args = ap.parse_args(argv)
    roles, unresolved, timing, manifest, mix = {}, {}, {}, None, None
    if not args.tone_only:
        if not os.path.isfile(args.manifest):
            sys.exit('no manifest at %s: run tools/audio_slice_chain.py first, or pass --tone-only' % args.manifest)
        with open(args.manifest, encoding='utf-8') as fh:
            manifest = json.load(fh)
        if manifest.get('schema') != 'ow-audio-v1':
            sys.exit('%s is not an ow-audio-v1 manifest' % args.manifest)
        roles, unresolved = build_play_map(manifest, args.decoded_dir)
        timing = {'reload': reload_timing(manifest)}
        if args.game and os.path.isdir(args.game):
            mix = GameMix(args.game)
        else:
            print('no game install given (--game or OPENWILLOW_BL2): every gain is 0 dB, not the game mix')
    write_tone(args.tone)
    roles['test_tone'] = [os.path.abspath(args.tone)]
    gains, gain_notes = build_gains(roles, manifest, mix)
    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    with open(args.out, 'w', encoding='utf-8') as fh:
        json.dump({'schema': PLAY_SCHEMA, 'roles': roles, 'unresolved': unresolved, 'timing': timing, 'gains': gains, 'gain_notes': gain_notes}, fh, indent=1)
    for role, files in sorted(roles.items()):
        levels = sorted(set(gains[role]))
        print('role %s: %d file(s), gain %s dB (%s)' % (role, len(files), ' / '.join('%.1f' % g for g in levels), gain_notes[role]))
    for role, reason in sorted(unresolved.items()):
        print('role %s: UNRESOLVED (%s)' % (role, reason))
    for clip, slot in sorted(timing.get('reload', {}).items()):
        print('reload timing %s: clip out %.3f s, clip in %.3f s (notify offsets in the arms clip)' % (clip, slot['clip_out_s'], slot['clip_in_s']))
    print('wrote %s (length checked only; nothing has been listened to)' % args.out)
    return 0


if __name__ == '__main__':
    sys.exit(main())
