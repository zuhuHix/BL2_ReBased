"""tools/listen_report.py's applied-gain fit on invented noise bursts (no game data). Needs numpy: not registered in ctest's bare-Python
group, run by hand for now (docs/verification/GATES.md, "Tests outside ctest")."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
import wave

import numpy

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'tools'))
import listen_report as lr  # noqa: E402

RATE = 48000


class FitTest(unittest.TestCase):
    """The recording holds a known scale of each file, at lags that are not a multiple of anything, under other sounds."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.rng = numpy.random.default_rng(7)

    def save(self, name, frames):
        path = self.tmp / name
        data = numpy.clip(numpy.round(frames * 32767), -32768, 32767).astype('<i2')
        with wave.open(str(path), 'wb') as w:
            w.setnchannels(frames.shape[1])
            w.setsampwidth(2)
            w.setframerate(RATE)
            w.writeframes(data.tobytes())
        return path

    def burst(self, name, seconds=0.5, amplitude=0.5):
        """White noise: broadband, so it correlates with itself only at its exact lag."""
        frames = (self.rng.standard_normal((int(seconds * RATE), 1)) * amplitude / 3).clip(-1, 1)
        return self.save(name, frames), frames

    def recording(self, placements, seconds=6.0):
        """placements: (start frame, mono frames, gain dB). Stereo, the same in both channels (a 2D sound after the upmix)."""
        mix = numpy.zeros((int(seconds * RATE), 2))
        for start, frames, db in placements:
            mix[start:start + len(frames)] += frames * 10 ** (db / 20)
        return self.save('listen.wav', mix)

    def play(self, role, file, db):
        return (f'LogTemp: Display: OWAUDIO play role={role} channels=2 rate={RATE} bytes=8000 gain_db={db:.1f} file={file} '
                'upmix=mono_to_stereo (the host called play; not heard)')

    def cue(self, role, t):
        return f'LogTemp: Display: OWAUDIOREC cue t={t:.4f} label={role} seconds=0.500 frame=-'

    def test_a_broadband_burst_fits_at_every_lag_residue(self):
        """The decimated search of the first version missed white noise one to seven samples off a multiple of 8 (review r1 on PR #221)."""
        _, burst = self.burst('a.wav')
        for residue in range(8):
            start = 6240 + residue                     # 0.13 s after the cue at 48 kHz, plus 0..7 samples
            wav = self.recording([(RATE + start, burst, -9.0)])
            rate, recording = lr.read_channels(wav)
            db, corr, lag, clipped = lr.fit_gain(recording, rate, 1.0, RATE, burst)
            self.assertAlmostEqual(db, -9.0, delta=0.05, msg=residue)
            self.assertGreater(corr, 0.99, residue)
            self.assertEqual(round(lag * RATE), start, residue)
            self.assertFalse(clipped)

    def test_the_fit_finds_the_files_gain_under_an_overlapping_sound(self):
        _, equip = self.burst('a.wav')
        _, other = self.burst('b.wav')
        wav = self.recording([(int(1.1306 * RATE), equip, -9.0), (int(1.10 * RATE), other, -3.0)])
        rate, recording = lr.read_channels(wav)
        db, corr, lag, _ = lr.fit_gain(recording, rate, 1.0, RATE, equip)
        self.assertAlmostEqual(db, -9.0, delta=0.3)
        self.assertAlmostEqual(lag, 0.1306, delta=0.0001)
        self.assertGreater(corr, lr.FIT_MIN_CORR)

    def test_the_reference_role_fits_and_carries_the_balance(self):
        """The shot (fire) is the reference: it must fit under its impact at an odd lag, or every row loses its fitted "off by"."""
        a_path, a = self.burst('a.wav')
        b_path, b = self.burst('b.wav')
        c_path, c = self.burst('c.wav')
        # fire plays b at -12 (map -12), hit plays c over it, equip plays a at -6 (map -6): each 3 dB under its map gain.
        wav = self.recording([(int(1.1306 * RATE), b, -15.0), (int(1.1353 * RATE), c, -14.0), (int(3.1311 * RATE), a, -9.0)])
        log_text = '\n'.join([self.play('fire', 'b.wav', -12), self.cue('fire', 1.0), self.play('hit', 'c.wav', -11), self.cue('hit', 1.005),
                              self.play('equip', 'a.wav', -6), self.cue('equip', 3.0)])
        cues = lr.parse_log(log_text)['cues']
        play = {'roles': {'fire': [str(a_path), str(b_path)], 'hit': [str(c_path)], 'equip': [str(a_path)]},
                'gains': {'fire': [-20.0, -12.0], 'hit': [-11.0], 'equip': [-6.0]}}
        fits = lr.fit_cues(cues, log_text, wav, play)
        self.assertEqual(Path(fits[0]['file']).name, 'b.wav')        # the logged file, not the role's first one
        self.assertTrue(fits[0]['file_logged'])
        for cue in cues:
            cue['level'] = {'rms_dbfs': -20.0}
        rows = {r['role']: r for r in lr.level_table(cues, log_text, play, fits)}
        self.assertAlmostEqual(rows['fire']['applied_minus_map_db'], -3.0, delta=0.3)
        self.assertEqual(rows['fire']['off_by_from'], 'fit')
        self.assertAlmostEqual(rows['equip']['off_by_db'], 0.0, delta=0.4)
        self.assertEqual(rows['equip']['off_by_from'], 'fit')

    def test_a_play_before_the_recording_does_not_shift_the_file(self):
        a_path, a = self.burst('a.wav')
        b_path, b = self.burst('b.wav')
        wav = self.recording([(int(1.1306 * RATE), b, -15.0)])
        log_text = '\n'.join([self.play('fire', 'a.wav', -12), 'LogTemp: Display: OWAUDIOREC start file=C:/x/listen.wav map=Sanctuary_P (test only)',
                              self.play('fire', 'b.wav', -12), self.cue('fire', 1.0)])
        cues = lr.parse_log(log_text)['cues']
        fits = lr.fit_cues(cues, log_text, wav, {'roles': {'fire': [str(a_path), str(b_path)]}})
        self.assertEqual(Path(fits[0]['file']).name, 'b.wav')
        self.assertAlmostEqual(fits[0]['fit_gain_db'], -15.0, delta=0.05)

    def test_a_guessed_file_is_marked(self):
        a_path, a = self.burst('a.wav')
        b_path, b = self.burst('b.wav')
        wav = self.recording([(int(1.1306 * RATE), b, -15.0)])
        log_text = self.cue('fire', 1.0)                         # an older log: no play line with file=
        cues = lr.parse_log(log_text)['cues']
        play = {'roles': {'fire': [str(a_path), str(b_path)]}, 'gains': {'fire': [-12.0, -12.0]}}
        fits = lr.fit_cues(cues, log_text, wav, play)
        self.assertEqual(Path(fits[0]['file']).name, 'b.wav')
        self.assertFalse(fits[0]['file_logged'])
        cues[0]['level'] = {'rms_dbfs': -20.0}
        row = lr.level_table(cues, log_text, play, fits)[0]
        self.assertTrue(row['file_guessed'])

    def test_two_cues_of_one_role_close_together_get_no_fit(self):
        a_path, a = self.burst('a.wav')
        wav = self.recording([(int(1.1306 * RATE), a, -9.0), (int(1.1346 * RATE), a, -9.0)])   # two identical sounds 4 ms apart add up
        log_text = '\n'.join([self.play('equip', 'a.wav', -6), self.cue('equip', 1.0), self.play('equip', 'a.wav', -6), self.cue('equip', 1.004)])
        cues = lr.parse_log(log_text)['cues']
        self.assertEqual(lr.fit_cues(cues, log_text, wav, {'roles': {'equip': [str(a_path)]}}), {})

    def test_a_role_without_a_fit_shows_no_off_by_when_others_fit(self):
        a_path, a = self.burst('a.wav')
        wav = self.recording([(int(1.1306 * RATE), a, -15.0)])
        log_text = '\n'.join([self.play('fire', 'a.wav', -12), self.cue('fire', 1.0), self.cue('equip', 3.0)])
        cues = lr.parse_log(log_text)['cues']
        play = {'roles': {'fire': [str(a_path)], 'equip': [str(a_path)]}, 'gains': {'fire': [-12.0], 'equip': [-6.0]}}
        fits = lr.fit_cues(cues, log_text, wav, play)
        for cue in cues:
            cue['level'] = {'rms_dbfs': -20.0}
        rows = {r['role']: r for r in lr.level_table(cues, log_text, play, fits)}
        self.assertIsNone(rows['equip']['off_by_db'])     # equip's window is silent: no fit, and no RMS number in its place

    def test_the_guide_says_why_off_by_is_empty_when_the_shot_has_no_fit(self):
        """Two shots inside one search window (automatic fire) leave the reference without a fit: the guide must say so."""
        a_path, a = self.burst('a.wav')
        b_path, b = self.burst('b.wav')
        wav = self.recording([(int(1.1306 * RATE), b, -15.0), (int(1.1346 * RATE), b, -15.0), (int(3.1311 * RATE), a, -9.0)])
        log_text = '\n'.join(['LogTemp: Display: OWAUDIOREC start file=C:/x/listen.wav map=Sanctuary_P (test only)',
                              'LogTemp: Display: OWAUDIOREC cue t=0.500 label=step Fight frame=-',
                              self.play('fire', 'b.wav', -12), self.cue('fire', 1.0), self.play('fire', 'b.wav', -12), self.cue('fire', 1.004),
                              self.play('equip', 'a.wav', -6), self.cue('equip', 3.0), 'LogTemp: Display: OWAUDIOREC stop t=5.900 file=C:/x/listen.wav'])
        play_map = self.tmp / 'play.json'
        play_map.write_text(json.dumps({'roles': {'fire': [str(b_path)], 'equip': [str(a_path)]}, 'gains': {'fire': [-12.0], 'equip': [-6.0]},
                                        'unresolved': {}}), encoding='utf-8')
        lr.build(log_text, wav, self.tmp / 'out', play_map=play_map, local_root=self.tmp)
        guide = (self.tmp / 'out' / 'LISTEN.md').read_text(encoding='utf-8')
        self.assertIn('No "Off by" in this table: the reference role `fire` gives no balance', guide)
        self.assertIn('none of its 2 cue(s) fitted', guide)
        equip = next(line for line in guide.splitlines() if line.startswith('| `equip` |') and 'dBFS' in line)
        self.assertIn('| -3.0 dB | - |', equip)                # its own applied-minus-map still shown, no off by

    def test_a_sound_that_starts_just_before_its_cue_fits(self):
        """The first equip of both listening runs started 2 and 8 ms before its cue (the clocks are not calibrated)."""
        _, burst = self.burst('a.wav')
        wav = self.recording([(RATE - 384, burst, -9.0)])     # 8 ms before the cue at 1.0 s
        rate, recording = lr.read_channels(wav)
        db, corr, lag, _ = lr.fit_gain(recording, rate, 1.0, RATE, burst)
        self.assertAlmostEqual(db, -9.0, delta=0.05)
        self.assertAlmostEqual(lag, -0.008, delta=0.0001)

    def test_a_cue_in_the_first_50_ms_searches_from_the_files_start(self):
        _, burst = self.burst('a.wav')
        wav = self.recording([(0, burst, -9.0)])                # the sound at frame 0, its cue at 0.02 s
        rate, recording = lr.read_channels(wav)
        db, corr, lag, _ = lr.fit_gain(recording, rate, 0.02, RATE, burst)
        self.assertAlmostEqual(db, -9.0, delta=0.05)
        self.assertGreater(corr, 0.99)
        self.assertAlmostEqual(lag, -0.020, delta=0.0001)

    def test_a_cue_with_a_negative_time_gets_no_fit(self):
        _, burst = self.burst('a.wav')
        wav = self.recording([(RATE, burst, -9.0)])
        rate, recording = lr.read_channels(wav)
        self.assertIsNone(lr.fit_gain(recording, rate, -0.5, RATE, burst))

    def test_the_no_off_by_line_names_a_missing_map_gain(self):
        """The shot fitted, but its file's map gain is null: the guide must say that, not that it found no fit."""
        b_path, b = self.burst('b.wav')
        wav = self.recording([(int(1.1306 * RATE), b, -15.0)])
        log_text = chr(10).join(['LogTemp: Display: OWAUDIOREC start file=C:/x/listen.wav map=Sanctuary_P (test only)',
                              'LogTemp: Display: OWAUDIOREC cue t=0.500 label=step Fight frame=-',
                              self.play('fire', 'b.wav', 0), self.cue('fire', 1.0), 'LogTemp: Display: OWAUDIOREC stop t=5.900 file=C:/x/listen.wav'])
        play_map = self.tmp / 'play.json'
        play_map.write_text(json.dumps({'roles': {'fire': [str(b_path)]}, 'gains': {'fire': [None]}, 'unresolved': {}}), encoding='utf-8')
        lr.build(log_text, wav, self.tmp / 'out', play_map=play_map, local_root=self.tmp)
        guide = (self.tmp / 'out' / 'LISTEN.md').read_text(encoding='utf-8')
        self.assertIn('its file has no game gain', guide)

    def test_clipping_is_flagged(self):
        _, burst = self.burst('a.wav', amplitude=0.9)
        wav = self.recording([(int(1.1306 * RATE), burst, 6.0)])
        rate, recording = lr.read_channels(wav)
        self.assertTrue(lr.fit_gain(recording, rate, 1.0, RATE, burst)[3])


if __name__ == '__main__':
    unittest.main()
