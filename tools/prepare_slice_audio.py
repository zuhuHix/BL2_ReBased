#!/usr/bin/env python3
"""Write the host's sound map (AI-assisted, clean room). See docs/verification/SLICE_AUDIO_CHAIN.md section 5.

Reads the ow-audio-v1 manifest (tools/audio_slice_chain.py) and PCM WAV files already decoded into an ignored directory,
and writes local/slice/audio_play.json for the host's -owaudioplay=. This tool decodes nothing and calls no decoder: a
role without a checked WAV stays out of the map and is listed under "unresolved".

  python tools/prepare_slice_audio.py [--manifest local/slice/audio.json] [--decoded-dir local/audio/decoded]
  python tools/prepare_slice_audio.py --tone-only     # a synthetic test tone only, no game data

A WAV is accepted only when it is 16-bit PCM and its length matches the manifest's header duration. That is a length
check: it does not show the sound is the right one, and nothing here means anyone has listened to it.
"""
import argparse
import json
import math
import os
import struct
import sys
import wave

PLAY_SCHEMA = 'ow-audio-play-v1'
# Role -> manifest key. Roles with no key are not resolved from evidence yet, so they stay silent rather than guess.
ROLE_KEYS = {'fire': 'weapon:fire', 'hit': None, 'phaselock_cast': None, 'pickup': None}
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
        if at + 8 + chunk > riff_end:
            return None, 'chunk runs past the RIFF size'
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
        at += 8 + chunk + (chunk & 1)
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
        files, reasons = [], []
        for media in entry['media']:
            path = os.path.abspath(os.path.join(decoded_dir, '%s.wav' % media['source']))
            reason = check_wav(path, media.get('duration_s'))
            if not reason and used + os.path.getsize(path) > MAX_TOTAL_BYTES:
                reason = 'the map would hold more than %d bytes' % MAX_TOTAL_BYTES
            if reason:
                reasons.append('%s: %s' % (media['source'], reason))
            else:
                used += os.path.getsize(path)
                files.append(path)
        if files:
            roles[role] = files
        else:
            unresolved[role] = '; '.join(reasons) or 'entry has no media'
    return roles, unresolved


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--manifest', default='local/slice/audio.json')
    ap.add_argument('--decoded-dir', default='local/audio/decoded')
    ap.add_argument('--out', default='local/slice/audio_play.json')
    ap.add_argument('--tone', default='local/audio/test_tone.wav', help='synthetic tone, always written, role test_tone')
    ap.add_argument('--tone-only', action='store_true', help='map only the synthetic tone; reads no game data')
    args = ap.parse_args(argv)
    roles, unresolved = {}, {}
    if not args.tone_only:
        if not os.path.isfile(args.manifest):
            sys.exit('no manifest at %s: run tools/audio_slice_chain.py first, or pass --tone-only' % args.manifest)
        with open(args.manifest, encoding='utf-8') as fh:
            manifest = json.load(fh)
        if manifest.get('schema') != 'ow-audio-v1':
            sys.exit('%s is not an ow-audio-v1 manifest' % args.manifest)
        roles, unresolved = build_play_map(manifest, args.decoded_dir)
    write_tone(args.tone)
    roles['test_tone'] = [os.path.abspath(args.tone)]
    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    with open(args.out, 'w', encoding='utf-8') as fh:
        json.dump({'schema': PLAY_SCHEMA, 'roles': roles, 'unresolved': unresolved}, fh, indent=1)
    for role, files in sorted(roles.items()):
        print('role %s: %d file(s)' % (role, len(files)))
    for role, reason in sorted(unresolved.items()):
        print('role %s: UNRESOLVED (%s)' % (role, reason))
    print('wrote %s (length checked only; nothing has been listened to)' % args.out)
    return 0


if __name__ == '__main__':
    sys.exit(main())
