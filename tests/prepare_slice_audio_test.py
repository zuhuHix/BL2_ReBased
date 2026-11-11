#!/usr/bin/env python3
"""Synthetic tests for tools/prepare_slice_audio.py: every WAV and manifest here is generated, no game data."""
import json
import math
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

# Synthetic only: main() must never find the installed game through the environment.
os.environ.pop('OPENWILLOW_BL2', None)


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

    # Invented names and times throughout: the real manifest's values stay out of tracked files.
    def reload_entry(self, clip, seconds, suffix, source):
        return {'key': 'weapon:reload_anim_%s_at_%ss' % (clip, seconds), 'trigger': {'role': 'reload anim %s @%ss' % (clip, seconds)},
                'ak_event': {'wwise_name': 'Play_Synth_%s' % suffix}, 'media': [{'source': source, 'duration_s': 1.0}]}

    def test_equip_maps_from_its_key(self):
        self.tone('1.wav')
        data = {'schema': 'ow-audio-v1', 'entries': [{'key': 'weapon:equip', 'media': [{'source': 1, 'duration_s': 1.0}]}]}
        roles, unresolved = psa.build_play_map(data, self.dir)
        self.assertEqual(roles['equip'], [os.path.abspath(os.path.join(self.dir, '1.wav'))])
        self.assertNotIn('equip', unresolved)

    def test_door_roles_map_from_their_keys(self):
        self.tone('4.wav')
        self.tone('5.wav')
        data = {'schema': 'ow-audio-v1', 'entries': [
            {'key': 'door:open', 'media': [{'source': 4, 'duration_s': 1.0}]},
            {'key': 'door:close', 'media': [{'source': 5, 'duration_s': 1.0}]}]}
        roles, unresolved = psa.build_play_map(data, self.dir)
        self.assertEqual(roles['door_open'], [os.path.abspath(os.path.join(self.dir, '4.wav'))])
        self.assertEqual(roles['door_close'], [os.path.abspath(os.path.join(self.dir, '5.wav'))])
        self.assertNotIn('door_open', unresolved)
        self.assertNotIn('door_close', unresolved)

    def test_a_door_without_a_decoded_file_stays_unresolved_and_is_not_guessed(self):
        self.tone('4.wav')
        data = {'schema': 'ow-audio-v1', 'entries': [
            {'key': 'door:open', 'media': [{'source': 4, 'duration_s': 1.0}]},
            {'key': 'door:close', 'media': [{'source': 9, 'duration_s': 1.0}]}]}
        roles, unresolved = psa.build_play_map(data, self.dir)
        self.assertIn('door_open', roles)
        self.assertNotIn('door_close', roles)
        self.assertIn('9', unresolved['door_close'])
        self.assertNotIn('door_open', unresolved)

    def test_missing_door_keys_are_unresolved_with_reasons(self):
        _, unresolved = psa.build_play_map(manifest([]), self.dir)
        self.assertEqual(unresolved['door_open'], 'manifest has no door:open')
        self.assertEqual(unresolved['door_close'], 'manifest has no door:close')

    def test_each_dialog_line_gets_its_own_role_from_its_key(self):
        self.tone('6.wav')
        self.tone('7.wav')
        self.tone('8.wav')
        data = {'schema': 'ow-audio-v1', 'entries': [
            {'key': 'dialog:Pkg.Events.Invented_Line_A', 'media': [{'source': 6, 'duration_s': 1.0}]},
            {'key': 'marcus_group:Pkg.Events.Invented_Bark', 'media': [{'source': 7, 'duration_s': 1.0}, {'source': 8, 'duration_s': 1.0}]}]}
        roles, unresolved = psa.build_play_map(data, self.dir)
        self.assertEqual(roles['line:Pkg.Events.Invented_Line_A'], [os.path.abspath(os.path.join(self.dir, '6.wav'))])
        self.assertEqual(len(roles['line:Pkg.Events.Invented_Bark']), 2)   # a random container: every source, the host picks
        self.assertFalse([r for r in unresolved if r.startswith('line:')])

    def test_a_dialog_line_without_a_decoded_file_stays_silent(self):
        data = {'schema': 'ow-audio-v1', 'entries': [{'key': 'dialog:Pkg.Events.Invented_Line_B', 'media': [{'source': 9, 'duration_s': 1.0}]}]}
        roles, unresolved = psa.build_play_map(data, self.dir)
        self.assertNotIn('line:Pkg.Events.Invented_Line_B', roles)
        self.assertIn('9', unresolved['line:Pkg.Events.Invented_Line_B'])

    def test_a_tag_under_both_prefixes_takes_the_dialog_entry(self):
        self.tone('6.wav')
        self.tone('7.wav')
        data = {'schema': 'ow-audio-v1', 'entries': [
            {'key': 'marcus_group:Pkg.Events.Invented_Both', 'media': [{'source': 7, 'duration_s': 1.0}]},
            {'key': 'dialog:Pkg.Events.Invented_Both', 'media': [{'source': 6, 'duration_s': 1.0}]}]}
        roles, _ = psa.build_play_map(data, self.dir)
        self.assertEqual(roles['line:Pkg.Events.Invented_Both'], [os.path.abspath(os.path.join(self.dir, '6.wav'))])

    def test_other_keys_make_no_line_role(self):
        self.tone('6.wav')
        data = {'schema': 'ow-audio-v1', 'entries': [{'key': 'door:open', 'media': [{'source': 6, 'duration_s': 1.0}]}]}
        roles, unresolved = psa.build_play_map(data, self.dir)
        self.assertFalse([r for r in list(roles) + list(unresolved) if r.startswith('line:')])

    def test_reload_roles_come_from_the_clip_out_and_clip_in_notifies(self):
        for name in ('1.wav', '2.wav', '3.wav'):
            self.tone(name)
        data = {'schema': 'ow-audio-v1', 'entries': [
            self.reload_entry('Synth_Reload', 0.25, 'Clip_Out', 2), self.reload_entry('Synth_Reload', 0.75, 'Clip_In', 3),
            # A second clip under the same two sounds: the shared source is mapped once, not twice.
            self.reload_entry('Synth_Fast', 0.1, 'Clip_Out', 2), self.reload_entry('Synth_Fast', 0.4, 'Clip_In', 3)]}
        roles, unresolved = psa.build_play_map(data, self.dir)
        self.assertEqual(roles['reload_out'], [os.path.abspath(os.path.join(self.dir, '2.wav'))])
        self.assertEqual(roles['reload_in'], [os.path.abspath(os.path.join(self.dir, '3.wav'))])
        self.assertNotIn('reload_out', unresolved)
        self.assertNotIn('reload_in', unresolved)

    def test_missing_equip_and_reload_notifies_are_unresolved_with_reasons(self):
        _, unresolved = psa.build_play_map(manifest([]), self.dir)
        self.assertEqual(unresolved['equip'], 'manifest has no weapon:equip')
        self.assertEqual(unresolved['reload_out'], 'manifest has no reload notify for Clip_Out')
        self.assertEqual(unresolved['reload_in'], 'manifest has no reload notify for Clip_In')

    def test_timing_pairs_each_clip_with_both_notifies(self):
        data = {'schema': 'ow-audio-v1', 'entries': [self.reload_entry('Synth_Reload', 0.25, 'Clip_Out', 2),
                                                     self.reload_entry('Synth_Reload', 0.75, 'Clip_In', 3),
                                                     self.reload_entry('Synth_Half', 0.5, 'Clip_Out', 2)]}
        self.assertEqual(psa.reload_timing(data), {'Synth_Reload': {'clip_out_s': 0.25, 'clip_in_s': 0.75}})

    def test_a_clip_with_two_notifies_of_one_kind_is_left_out_of_timing(self):
        data = {'schema': 'ow-audio-v1', 'entries': [self.reload_entry('Synth_Reload', 0.25, 'Clip_Out', 2),
                                                     self.reload_entry('Synth_Reload', 0.5, 'Clip_Out', 2),
                                                     self.reload_entry('Synth_Reload', 0.75, 'Clip_In', 3)]}
        self.assertEqual(psa.reload_timing(data), {})

    def test_main_writes_roles_and_timing(self):
        for name in ('1.wav', '2.wav', '3.wav'):
            self.tone(name)
        data = {'schema': 'ow-audio-v1', 'entries': [
            self.reload_entry('Synth_Reload', 0.25, 'Clip_Out', 2), self.reload_entry('Synth_Reload', 0.75, 'Clip_In', 3)]}
        manifest_path = os.path.join(self.dir, 'audio.json')
        with open(manifest_path, 'w', encoding='utf-8') as fh:
            json.dump(data, fh)
        out = os.path.join(self.dir, 'play.json')
        tone = os.path.join(self.dir, 'tone.wav')
        self.assertEqual(psa.main(['--manifest', manifest_path, '--decoded-dir', self.dir, '--out', out, '--tone', tone]), 0)
        with open(out, encoding='utf-8') as fh:
            played = json.load(fh)
        self.assertEqual(played['timing'], {'reload': {'Synth_Reload': {'clip_out_s': 0.25, 'clip_in_s': 0.75}}})
        self.assertIn('reload_out', played['roles'])
        self.assertIn('reload_in', played['roles'])
        self.assertIn('test_tone', played['roles'])

    def test_tone_only_writes_an_empty_timing_block(self):
        out = os.path.join(self.dir, 'play.json')
        psa.main(['--tone-only', '--out', out, '--tone', os.path.join(self.dir, 'tone.wav')])
        with open(out, encoding='utf-8') as fh:
            self.assertEqual(json.load(fh)['timing'], {})



# ---- The game's static mix (SLICE_AUDIO_CHAIN.md section 7): an invented bank, built byte by byte -----------------------------------
def bundle(props=None, ranged=None):
    props, ranged = props or {}, ranged or {}
    out = bytes([len(props)]) + bytes(props) + b''.join(struct.pack('<f', v) for v in props.values())
    return out + bytes([len(ranged)]) + bytes(ranged) + b''.join(struct.pack('<ff', *v) for v in ranged.values())


def sound_body(ident, source, parent, bus=0, props=None, ranged=None):
    """A streamed sound (stream type 1): id, plugin, stream type, source, file id, a bits byte, two effect bytes, bus, parent, two flag bytes, bundles."""
    return struct.pack('<IIIII', ident, 0x00040001, 1, source, 0) + b'\x00' + b'\x00\x00' + struct.pack('<II', bus, parent) + b'\x00\x00' + bundle(props, ranged)


def mixer_body(ident, parent, bus=0, props=None, tail=b''):
    return struct.pack('<I', ident) + b'\x00\x00' + struct.pack('<II', bus, parent) + b'\x00\x00' + bundle(props) + tail


def switch_tail(children, values, default):
    """The part of a switch container after its bundles that the reader uses: group, default, a flag, children, then each value's children."""
    out = b'\x00' + struct.pack('<II', 0x1234, default) + b'\x00' + struct.pack('<I', len(children)) + struct.pack('<%dI' % len(children), *children)
    out += struct.pack('<I', len(values))
    for value, kids in values:
        out += struct.pack('<II', value, len(kids)) + struct.pack('<%dI' % len(kids), *kids)
    return out


def bus_body(ident, parent, volume=None):
    props = {0: volume} if volume is not None else {}
    return struct.pack('<II', ident, parent) + bytes([len(props)]) + bytes(props) + b''.join(struct.pack('<f', v) for v in props.values()) + b'\x00' * 8


FIRST, THIRD = psa.audio_census.fnv1_32('firstperson'), psa.audio_census.fnv1_32('thirdperson')


def invented_bank():
    """Master bus (-2) <- sfx bus (-5) <- mixer (-1) <- switch (0) <- two sounds of source 77: first person (-4), third person (-9)."""
    index = {
        900: (8, bus_body(900, 0, -2.0)),
        901: (8, bus_body(901, 900, -5.0)),
        10: (7, mixer_body(10, 0, bus=901, props={0: -1.0, 4: 50.0})),
        20: (6, mixer_body(20, 10, tail=switch_tail([30, 31], [(FIRST, [30]), (THIRD, [31])], THIRD))),
        30: (2, sound_body(30, 77, 20, props={0: -4.0})),
        31: (2, sound_body(31, 77, 20, props={0: -9.0})),
    }
    return index, {900, 901}


class GameMixTest(unittest.TestCase):
    def test_a_chain_sums_the_object_and_bus_volumes(self):
        index, buses = invented_bank()
        chain = psa.volume_chain(30, index, index, buses)
        self.assertEqual([node['id'] for node in chain], [30, 20, 10, 901, 900])
        self.assertAlmostEqual(sum(node['volume_db'] for node in chain), -4.0 - 1.0 - 5.0 - 2.0)

    def test_a_property_other_than_volume_is_not_a_volume(self):
        index, buses = invented_bank()
        self.assertEqual(psa.volume_chain(10, index, index, buses)[0]['volume_db'], -1.0)   # prop 4 = 50 is ignored

    def test_the_player_takes_the_first_person_child_and_others_the_default(self):
        index, buses = invented_bank()
        chains = [psa.volume_chain(s, index, index, buses) for s in (30, 31)]
        rows = {1: {5: chains}}
        self.assertEqual(psa.source_gain(rows, 77, FIRST)[0], -12.0)
        self.assertEqual(psa.source_gain(rows, 77, None)[0], -17.0)

    def test_two_play_actions_on_one_source_add_as_amplitudes(self):
        index, buses = invented_bank()
        chain = psa.volume_chain(30, index, index, buses)
        db, notes = psa.source_gain({1: {5: [chain], 6: [chain]}}, 77, FIRST)
        self.assertAlmostEqual(db, -12.0 + 20 * math.log10(2), places=2)
        self.assertTrue(any('2 play actions' in n for n in notes))

    def test_banks_that_disagree_are_noted(self):
        index, buses = invented_bank()
        first, third = (psa.volume_chain(s, index, index, buses) for s in (30, 31))
        _, notes = psa.source_gain({1: {5: [first]}, 2: {5: [third]}}, 77, FIRST)
        self.assertTrue(any('banks disagree' in n for n in notes))

    def test_a_random_volume_range_is_noted_not_applied(self):
        index, buses = invented_bank()
        index[30] = (2, sound_body(30, 77, 20, props={0: -4.0}, ranged={0: (-3.0, 0.0)}))
        db, notes = psa.source_gain({1: {5: [psa.volume_chain(30, index, index, buses)]}}, 77, FIRST)
        self.assertEqual(db, -12.0)
        self.assertTrue(any('random volume range' in n for n in notes))

    def test_a_source_not_reached_has_no_level(self):
        self.assertIsNone(psa.source_gain({}, 77, FIRST)[0])

    def test_without_game_data_every_gain_is_unity_and_says_so(self):
        gains, notes = psa.build_gains({'fire': ['a/1.wav', 'a/2.wav'], 'test_tone': ['t.wav']}, None, None)
        self.assertEqual(gains, {'fire': [0.0, 0.0], 'test_tone': [0.0]})
        self.assertIn('no game data', notes['fire'])

    def test_gains_follow_the_files_order_and_a_missing_level_is_unity(self):
        class FakeMix:
            def entry_gains(self, entry, role):
                self.role = role
                return {1: (-12.0, []), 2: (None, ['source 2 not reached from the event']), 3: (-200.0, [])}
        mix = FakeMix()
        man = manifest([{'source': 1}, {'source': 2}, {'source': 3}])
        gains, notes = psa.build_gains({'fire': [os.path.join('d', '3.wav'), os.path.join('d', '1.wav'), os.path.join('d', '2.wav')]}, man, mix)
        self.assertEqual(gains['fire'], [psa.MIN_GAIN_DB, -12.0, 0.0])   # clamped to the host's floor; no level means 0 dB
        self.assertIn('source 2', notes['fire'])
        self.assertTrue(notes['fire'].startswith('game data'))

    def test_the_gain_limits_match_the_host(self):
        header = (Path(__file__).resolve().parent.parent / 'host/ue5/OpenWillow/Source/OpenWillow/OpenWillowAudio.h').read_text(encoding='utf-8')
        for name, value in (('MinGainDb', psa.MIN_GAIN_DB), ('MaxGainDb', psa.MAX_GAIN_DB)):
            self.assertEqual(float(re.search(name + r' = (-?[\d.]+);', header).group(1)), value)

    def test_tone_only_map_has_a_unity_gain_per_file(self):
        out = os.path.join(tempfile.mkdtemp(), 'play.json')
        psa.main(['--tone-only', '--out', out, '--tone', os.path.join(os.path.dirname(out), 't.wav')])
        with open(out, encoding='utf-8') as fh:
            data = json.load(fh)
        self.assertEqual(data['gains'], {'test_tone': [0.0]})


if __name__ == '__main__':
    unittest.main()
