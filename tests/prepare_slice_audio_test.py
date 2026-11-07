#!/usr/bin/env python3
"""Synthetic tests for tools/prepare_slice_audio.py: every WAV and manifest here is generated, no game data."""
import json
import os
from pathlib import Path
import re
import struct
import sys
import tempfile
import unittest
import wave

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'tools'))
import prepare_slice_audio as psa  # noqa: E402


def wav_bytes(rate=48000, channels=1, frames=48000, data=None, riff_size=None, before_fmt=b'', after_data=b''):
    """A WAV built by hand, so a test can make the header lie."""
    payload = bytes(2 * channels * frames) if data is None else data
    fmt = struct.pack('<4sIHHIIHH', b'fmt ', 16, 1, channels, rate, rate * 2 * channels & 0xFFFFFFFF, 2 * channels, 16)
    body = b'WAVE' + before_fmt + fmt + struct.pack('<4sI', b'data', len(payload)) + payload + after_data
    return b'RIFF' + struct.pack('<I', len(body) if riff_size is None else riff_size) + body


def manifest(media):
    return {'schema': 'ow-audio-v1', 'entries': [{'key': 'weapon:fire', 'media': media}]}


class PrepareSliceAudioTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.dir = self.temp.name

    def tone(self, name, seconds=1.0):
        path = os.path.join(self.dir, name)
        psa.write_tone(path, seconds=seconds)
        return path

    def write(self, name, data):
        path = os.path.join(self.dir, name)
        with open(path, 'wb') as fh:
            fh.write(data)
        return path

    def test_a_truncated_file_with_a_good_header_is_refused(self):
        whole = wav_bytes()
        reason = psa.check_wav(self.write('1.wav', whole[:len(whole) // 2]), 1.0)
        self.assertIn('past the end', reason)

    def test_the_data_chunk_may_not_run_past_the_riff_size(self):
        whole = wav_bytes()
        self.assertIn('past the RIFF size', psa.check_wav(self.write('1.wav', wav_bytes(riff_size=len(whole) - 8 - 2)), None))

    def test_riff_size_zero_is_refused(self):
        self.assertIn('too small', psa.check_wav(self.write('1.wav', wav_bytes(riff_size=0)), None))

    def test_partial_frames_are_refused(self):
        self.assertIn('whole sample frames', psa.check_wav(self.write('1.wav', wav_bytes(channels=2, data=bytes(6))), None))

    def test_a_zero_rate_is_refused_not_a_crash(self):
        self.assertIn('sample rate', psa.check_wav(self.write('1.wav', wav_bytes(rate=0)), None))

    def test_a_rate_outside_the_host_range_is_refused(self):
        for rate in (psa.MIN_SAMPLE_RATE - 1, psa.MAX_SAMPLE_RATE + 1, 0x7FFFFFFF):
            self.assertIn('sample rate', psa.check_wav(self.write('1.wav', wav_bytes(rate=rate)), None))

    def test_a_padded_odd_chunk_is_stepped_over(self):
        odd = struct.pack('<4sI', b'LIST', 3) + b'abc' + b'\0'
        self.assertIsNone(psa.check_wav(self.write('1.wav', wav_bytes(before_fmt=odd)), 1.0))

    def test_a_second_fmt_is_refused(self):
        second = struct.pack('<4sIHHIIHH', b'fmt ', 16, 1, 1, 8000, 16000, 2, 16)
        self.assertIn('more than one fmt', psa.check_wav(self.write('1.wav', wav_bytes(before_fmt=second)), None))

    def test_a_second_data_is_refused(self):
        second = struct.pack('<4sI', b'data', 2) + b'\0\0'
        self.assertIn('more than one data', psa.check_wav(self.write('1.wav', wav_bytes(after_data=second)), None))

    def test_a_chunk_after_data_may_not_run_past_the_riff_size(self):
        late = struct.pack('<4sI', b'LIST', 100)
        self.assertIn('past the RIFF size', psa.check_wav(self.write('1.wav', wav_bytes(after_data=late)), None))

    def test_a_partial_chunk_at_the_end_is_refused(self):
        self.assertIn('partial chunk', psa.check_wav(self.write('1.wav', wav_bytes(after_data=b'LIST')), None))

    def test_an_odd_final_chunk_needs_its_pad_inside_the_riff_size(self):
        chunk = struct.pack('<4sI', b'LIST', 1) + b'a'
        self.assertIsNone(psa.check_wav(self.write('1.wav', wav_bytes(after_data=chunk + b'\0')), 1.0))
        self.assertIn('pad', psa.check_wav(self.write('2.wav', wav_bytes(after_data=chunk)), 1.0))
        # The pad byte is in the file but outside the declared size: still refused.
        whole = wav_bytes(after_data=chunk + b'\0')
        self.assertIn('pad', psa.check_wav(self.write('3.wav', wav_bytes(after_data=chunk + b'\0', riff_size=len(whole) - 8 - 1)), 1.0))

    def test_a_good_chunk_after_data_is_accepted(self):
        late = struct.pack('<4sI', b'LIST', 2) + b'ab'
        self.assertIsNone(psa.check_wav(self.write('1.wav', wav_bytes(after_data=late)), 1.0))

    def test_a_file_over_the_limit_is_refused_before_reading(self):
        path = self.write('1.wav', b'x')
        with open(path, 'r+b') as fh:
            fh.truncate(psa.MAX_FILE_BYTES + 1)
        self.assertIn('limit', psa.check_wav(path, None))

    def test_the_map_stops_at_the_total_limit_and_says_so(self):
        self.tone('1.wav')
        self.tone('2.wav')
        limit, psa.MAX_TOTAL_BYTES = psa.MAX_TOTAL_BYTES, os.path.getsize(os.path.join(self.dir, '1.wav'))
        self.addCleanup(setattr, psa, 'MAX_TOTAL_BYTES', limit)
        roles, unresolved = psa.build_play_map(manifest([{'source': 1, 'duration_s': 1.0}, {'source': 2, 'duration_s': 1.0}]), self.dir)
        self.assertEqual(len(roles['fire']), 1)
        self.assertNotIn('fire', unresolved)     # one file still plays
        psa.MAX_TOTAL_BYTES = 1                  # nothing fits: the role is listed, not silently absent
        roles, unresolved = psa.build_play_map(manifest([{'source': 1, 'duration_s': 1.0}]), self.dir)
        self.assertNotIn('fire', roles)
        self.assertIn('more than', unresolved['fire'])

    def test_the_limits_match_the_host(self):
        header = (Path(__file__).resolve().parent.parent / 'host/ue5/OpenWillow/Source/OpenWillow/OpenWillowAudio.h').read_text(encoding='utf-8')
        for name, value in (('MinSampleRate', psa.MIN_SAMPLE_RATE), ('MaxSampleRate', psa.MAX_SAMPLE_RATE)):
            self.assertEqual(int(re.search(name + r' = (\d+);', header).group(1)), value)
        for name, value in (('MaxFileBytes', psa.MAX_FILE_BYTES), ('MaxTotalBytes', psa.MAX_TOTAL_BYTES)):
            self.assertEqual(int(re.search(name + r' = (\d+) \* 1024 \* 1024;', header).group(1)) * 1024 * 1024, value)

    def test_tone_is_16_bit_mono_of_the_asked_length(self):
        with wave.open(self.tone('t.wav', seconds=0.5), 'rb') as wav:
            self.assertEqual((wav.getnchannels(), wav.getsampwidth()), (1, 2))
            self.assertAlmostEqual(wav.getnframes() / wav.getframerate(), 0.5)

    def test_matching_length_is_mapped(self):
        self.tone('1.wav')
        roles, unresolved = psa.build_play_map(manifest([{'source': 1, 'duration_s': 1.0}]), self.dir)
        self.assertEqual(roles['fire'], [os.path.abspath(os.path.join(self.dir, '1.wav'))])
        self.assertNotIn('fire', unresolved)

    def test_wrong_length_is_refused(self):
        self.tone('1.wav')
        roles, unresolved = psa.build_play_map(manifest([{'source': 1, 'duration_s': 2.0}]), self.dir)
        self.assertNotIn('fire', roles)
        self.assertIn('header says', unresolved['fire'])

    def test_missing_file_leaves_the_role_silent(self):
        roles, unresolved = psa.build_play_map(manifest([{'source': 1, 'duration_s': 1.0}]), self.dir)
        self.assertNotIn('fire', roles)
        self.assertIn('fire', unresolved)

    def test_a_bad_file_does_not_drop_the_good_one(self):
        self.tone('1.wav')
        with open(os.path.join(self.dir, '2.wav'), 'wb') as fh:
            fh.write(b'RIFFnot a wave')
        roles, _ = psa.build_play_map(manifest([{'source': 1, 'duration_s': 1.0}, {'source': 2, 'duration_s': 1.0}]), self.dir)
        self.assertEqual(len(roles['fire']), 1)

    def test_eight_bit_is_refused(self):
        path = os.path.join(self.dir, '1.wav')
        with wave.open(path, 'wb') as wav:
            wav.setnchannels(1)
            wav.setsampwidth(1)
            wav.setframerate(8000)
            wav.writeframes(b'\x80' * 8000)
        self.assertIn('16-bit', psa.check_wav(path, None))

    def test_a_role_without_evidence_stays_unresolved(self):
        _, unresolved = psa.build_play_map(manifest([]), self.dir)
        self.assertEqual(unresolved['pickup'], 'no sound identified from evidence yet')

    def test_a_manifest_older_than_the_roles_leaves_them_unresolved_with_a_reason(self):
        _, unresolved = psa.build_play_map(manifest([]), self.dir)
        for role, key in (('hit', 'role:hit'), ('phaselock_cast', 'role:phaselock_cast')):
            self.assertEqual(unresolved[role], 'manifest has no ' + key)

    def test_the_live_roles_map_when_the_manifest_has_them(self):
        self.tone('1.wav')
        self.tone('2.wav')
        data = {'schema': 'ow-audio-v1', 'entries': [{'key': 'role:hit', 'media': [{'source': 1, 'duration_s': 1.0}]},
                                                     {'key': 'role:phaselock_cast', 'media': [{'source': 2, 'duration_s': 1.0}]}]}
        roles, unresolved = psa.build_play_map(data, self.dir)
        self.assertEqual(roles['hit'], [os.path.abspath(os.path.join(self.dir, '1.wav'))])
        self.assertEqual(roles['phaselock_cast'], [os.path.abspath(os.path.join(self.dir, '2.wav'))])
        self.assertNotIn('hit', unresolved)
        self.assertNotIn('phaselock_cast', unresolved)

    def test_tone_only_writes_the_map_without_a_manifest(self):
        out = os.path.join(self.dir, 'play.json')
        self.assertEqual(psa.main(['--tone-only', '--out', out, '--tone', os.path.join(self.dir, 'tone.wav'), '--manifest', os.path.join(self.dir, 'absent.json')]), 0)
        with open(out, encoding='utf-8') as fh:
            data = json.load(fh)
        self.assertEqual(data['schema'], psa.PLAY_SCHEMA)
        self.assertEqual(list(data['roles']), ['test_tone'])

    def test_wrong_manifest_schema_is_refused(self):
        bad = os.path.join(self.dir, 'bad.json')
        with open(bad, 'w', encoding='utf-8') as fh:
            json.dump({'schema': 'other', 'entries': []}, fh)
        with self.assertRaises(SystemExit):
            psa.main(['--manifest', bad, '--out', os.path.join(self.dir, 'p.json'), '--tone', os.path.join(self.dir, 'tone.wav')])


if __name__ == '__main__':
    unittest.main()
