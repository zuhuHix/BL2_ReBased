#!/usr/bin/env python3
"""Synthetic tests for tools/decode_slice_audio.py. The decoder is stubbed and every file is generated: no vgmstream, no game data."""
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'tools'))
import decode_slice_audio as dsa  # noqa: E402
import prepare_slice_audio as psa  # noqa: E402


def good_decoder(exe, wem, wav):
    psa.write_tone(wav, seconds=1.0)
    return True, ''


class DecodeTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.dir = self.temp.name
        self.out = os.path.join(self.dir, 'decoded')
        os.makedirs(self.out)
        self.wem = os.path.join(self.dir, 'in.wem')
        with open(self.wem, 'wb') as fh:
            fh.write(b'not used by the stub')

    def item(self, duration=1.0):
        return {'wem': self.wem, 'duration_s': duration}

    def assert_nothing_left(self):
        self.assertEqual(os.listdir(self.out), [])

    def test_a_good_decode_lands_under_its_final_name(self):
        self.assertIsNone(dsa.decode_one('exe', 7, self.item(), self.out, good_decoder))
        self.assertEqual(os.listdir(self.out), ['7.wav'])

    def test_a_decoder_failure_leaves_nothing_and_says_why(self):
        reason = dsa.decode_one('exe', 7, self.item(), self.out, lambda e, w, o: (False, 'exit 1: bad'))
        self.assertIn('exit 1', reason)
        self.assert_nothing_left()

    def test_a_wrong_length_is_refused_and_removed(self):
        reason = dsa.decode_one('exe', 7, self.item(duration=3.0), self.out, good_decoder)
        self.assertIn('header says', reason)
        self.assert_nothing_left()

    def test_a_looped_output_longer_than_the_header_is_refused(self):
        def looped(exe, wem, wav):
            psa.write_tone(wav, seconds=2.0)
            return True, ''
        self.assertIn('header says', dsa.decode_one('exe', 7, self.item(), self.out, looped))
        self.assert_nothing_left()

    def test_a_truncated_wav_is_refused_and_removed(self):
        def truncated(exe, wem, wav):
            psa.write_tone(wav, seconds=1.0)
            with open(wav, 'r+b') as fh:
                fh.truncate(1000)
            return True, ''
        self.assertIn('past the end', dsa.decode_one('exe', 7, self.item(), self.out, truncated))
        self.assert_nothing_left()

    def test_a_decoder_that_exits_0_and_writes_nothing_is_a_failure(self):
        reason = dsa.decode_one('exe', 7, self.item(), self.out, lambda e, w, o: (True, ''))
        self.assertTrue(reason)
        self.assert_nothing_left()

    def test_a_missing_wem_is_reported(self):
        self.assertIn('no .wem', dsa.decode_one('exe', 7, {'wem': os.path.join(self.dir, 'absent.wem')}, self.out, good_decoder))

    def test_a_failed_redecode_does_not_keep_the_old_file(self):
        dsa.decode_one('exe', 7, self.item(), self.out, good_decoder)
        dsa.decode_one('exe', 7, self.item(), self.out, lambda e, w, o: (False, 'bad'))
        self.assert_nothing_left()

    def test_each_source_is_decoded_once_and_counts_are_honest(self):
        calls = []

        def counting(exe, wem, wav):
            calls.append(wav)
            if '/2.' in wav.replace(os.sep, '/'):
                return False, 'bad'
            return good_decoder(exe, wem, wav)
        manifest = {'entries': [{'media': [{'source': 1, **self.item()}, {'source': 2, **self.item()}]}, {'media': [{'source': 1, **self.item()}]}]}
        report = dsa.decode_all('exe', manifest, self.out, counting)
        self.assertEqual(len(calls), 2)
        self.assertEqual((report['distinct_sources'], report['decoded'], report['failed']), (2, 1, 1))
        self.assertEqual(report['failures'], {'2': 'bad'})
        self.assertEqual(report['output_bytes'], os.path.getsize(os.path.join(self.out, '1.wav')))

    def test_a_kept_file_is_one_the_prepare_tool_maps(self):
        manifest = {'schema': 'ow-audio-v1', 'entries': [{'key': 'weapon:fire', 'media': [{'source': 1, **self.item()}]}]}
        dsa.decode_all('exe', manifest, self.out, good_decoder)
        roles, _ = psa.build_play_map(manifest, self.out)
        self.assertEqual(roles['fire'], [os.path.abspath(os.path.join(self.out, '1.wav'))])

    def test_output_inside_the_repository_must_be_under_local(self):
        self.assertTrue(dsa.inside_repo_but_not_local(os.path.join(dsa.ROOT, 'docs', 'x')))
        self.assertTrue(dsa.inside_repo_but_not_local(dsa.ROOT))
        self.assertTrue(dsa.inside_repo_but_not_local(os.path.join(dsa.ROOT, 'localish')))
        self.assertFalse(dsa.inside_repo_but_not_local(os.path.join(dsa.ROOT, 'local', 'audio', 'decoded')))
        self.assertFalse(dsa.inside_repo_but_not_local(self.out))

    def test_main_refuses_a_missing_tool_and_a_tracked_folder_and_never_downloads(self):
        with self.assertRaises(SystemExit) as caught:
            dsa.main(['--vgmstream', os.path.join(self.dir, 'absent.exe')])
        self.assertIn('does not download', str(caught.exception))
        with self.assertRaises(SystemExit) as caught:
            dsa.main(['--vgmstream', self.wem, '--out-dir', os.path.join(dsa.ROOT, 'docs', 'decoded')])
        self.assertIn('ignored local', str(caught.exception))
        with open(dsa.__file__, encoding='utf-8') as fh:
            source = fh.read()
        self.assertNotIn('urllib', source)
        self.assertNotIn('requests', source)

    def test_main_refuses_a_wrong_manifest_schema(self):
        bad = os.path.join(self.dir, 'bad.json')
        with open(bad, 'w', encoding='utf-8') as fh:
            json.dump({'schema': 'other'}, fh)
        with self.assertRaises(SystemExit):
            dsa.main(['--vgmstream', self.wem, '--manifest', bad, '--out-dir', self.out, '--report', os.path.join(self.dir, 'r.json')])


if __name__ == '__main__':
    unittest.main()
