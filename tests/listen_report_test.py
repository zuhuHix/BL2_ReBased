"""tools/listen_report.py on an invented log and a synthetic WAV (tones and silence written here). No game data."""
from pathlib import Path
import array
import math
import sys
import tempfile
import unittest
import wave

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'tools'))
import listen_report as lr  # noqa: E402

RATE = 8000


def write_wav(path, seconds, tones, channels=2, amplitude=0.5):
    """tones: (start s, length s) spans of a 440 Hz tone at the amplitude, on the left channel only; silence elsewhere."""
    frames = int(seconds * RATE)
    data = array.array('h', [0] * frames * channels)
    for start, length in tones:
        for i in range(int(start * RATE), min(frames, int((start + length) * RATE))):
            data[i * channels] = int(amplitude * 32767 * math.sin(2 * math.pi * 440 * i / RATE))
    with wave.open(str(path), 'wb') as w:
        w.setnchannels(channels)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(data.tobytes())


def log(stop=9.0):
    rows = [
        'OWAUDIOREC start file=C:/x/listen.wav map=Sanctuary_P (test only; records what the host sent to the mixer)',
        'OWAUDIOREC cue t=0.100 label=step Start frame=frame-001.png',
        'OWAUDIOREC cue t=2.000 label=step Fight frame=frame-002.png',
        'OWAUDIOREC cue t=2.500 label=fire seconds=0.500 frame=-',
        'OWAUDIOREC cue t=2.520 label=hit seconds=0.300 frame=frame-003.png',
        'OWAUDIOREC cue t=6.000 label=step Down frame=frame-004.png',
        'OWSLICEGATE link=go_down result=PASS fixtures=[] detail=down (t=6.0s)',
        'OWSLICEGATE SUMMARY pass=1 fail=0 na=0',
        f'OWAUDIOREC stop t={stop:.3f} file=C:/x/listen.wav',
        'OWAUDIOREC written=1 bytes=1000 after 1.0 s',
    ]
    return '\n'.join('[2026.11.08-12.00.00:000][  1]LogTemp: Display: ' + r for r in rows)


class ListenReportTest(unittest.TestCase):
    def run_build(self, seconds=9.0, tones=((2.5, 0.5),), text=None, amplitude=0.5):
        tmp = Path(tempfile.mkdtemp())
        wav = tmp / 'listen.wav'
        write_wav(wav, seconds, tones, amplitude=amplitude)
        problems, steps, run = lr.build(text or log(), wav, tmp / 'out', local_root=tmp)
        return tmp / 'out', problems, steps, run

    def test_steps_sounds_and_levels(self):
        out, problems, steps, run = self.run_build()
        self.assertEqual(problems, [])
        self.assertEqual([s['name'] for s in steps], ['Start', 'Fight', 'Down'])
        fight = steps[1]
        self.assertEqual([s['name'] for s in fight['sounds']], ['fire', 'hit'])
        fire = fight['sounds'][0]
        # A 0.5 amplitude tone is about -6 dBFS at its peak; the cue sits on the tone's first sample.
        self.assertAlmostEqual(fire['level']['peak_dbfs'], -6.0, delta=0.3)
        self.assertEqual(fire['level']['onset_s'], 0.0)
        self.assertFalse(fire['level']['clipped'])
        self.assertEqual(steps[0]['sounds'], [])
        # The gap after the fight's sounds is silence.
        self.assertEqual(fight['gap']['peak_dbfs'], float('-inf'))
        listen = (out / 'LISTEN.md').read_text(encoding='utf-8')
        self.assertIn('### 2. 0:02.000 to 0:06.000: Fight', listen)
        self.assertIn('**fire** x1', listen)
        self.assertIn('You should hear silence', listen)
        answers = (out / 'ANSWERS.md').read_text(encoding='utf-8')
        self.assertIn('| 2 | 0:02.000 | Fight | fire, hit |', answers)
        self.assertTrue((out / 'cues.json').exists())

    def test_sound_left_over_in_a_gap_is_reported(self):
        _, problems, steps, _ = self.run_build(tones=((2.5, 0.5), (5.0, 0.5)))
        self.assertEqual(problems, [])
        self.assertGreater(steps[1]['gap']['peak_dbfs'], lr.SILENT_DBFS)

    def test_onset_delay_measured(self):
        _, _, steps, _ = self.run_build(tones=((2.6, 0.5),))
        self.assertAlmostEqual(steps[1]['sounds'][0]['level']['onset_s'], 0.1, delta=0.01)

    def test_silent_recording_is_a_problem(self):
        _, problems, _, _ = self.run_build(tones=())
        self.assertTrue(any('silent' in p for p in problems), problems)

    def test_short_recording_is_a_problem(self):
        _, problems, _, _ = self.run_build(seconds=4.0)
        self.assertTrue(any('cut short' in p for p in problems), problems)

    def test_missing_wav_and_no_start(self):
        tmp = Path(tempfile.mkdtemp())
        problems, _, _ = lr.build('nothing here', tmp / 'none.wav', tmp / 'out', local_root=tmp)
        self.assertTrue(any('no WAV' in p for p in problems), problems)
        self.assertTrue(any('never started recording' in p for p in problems), problems)
        self.assertTrue((tmp / 'out' / 'LISTEN.md').exists())

    def test_clipping_flagged(self):
        _, _, steps, _ = self.run_build(amplitude=1.0)
        self.assertTrue(steps[1]['sounds'][0]['level']['clipped'])

    def test_repeated_step_cue_is_one_step(self):
        text = log().replace('label=step Down frame=frame-004.png', 'label=step Fight frame=frame-004.png')
        _, _, steps, _ = self.run_build(text=text)
        self.assertEqual([s['name'] for s in steps], ['Start', 'Fight'])

    def test_onset_not_measured_over_another_sound(self):
        # The hit cue at 2.52 s starts inside the fire tone: no onset for it, while the fire cue (in silence) has one.
        _, _, steps, _ = self.run_build()
        fire, hit = steps[1]['sounds']
        self.assertIsNotNone(fire['level']['onset_s'])
        self.assertIsNone(hit['level']['onset_s'])

    def test_out_outside_local_refused(self):
        root = Path(tempfile.mkdtemp())
        local = root / 'local'
        local.mkdir()
        self.assertEqual(lr.checked_out(local / 'a' / 'b', local), (local / 'a' / 'b').resolve())
        for outside in (root / 'elsewhere', local / '..' / 'escape', root):
            with self.assertRaises(ValueError):
                lr.checked_out(outside, local)
        with self.assertRaises(ValueError):
            lr.build('nothing here', root / 'none.wav', root / 'out', local_root=local)
        self.assertFalse((root / 'out').exists())

    def test_link_inside_local_pointing_outside_refused(self):
        root = Path(tempfile.mkdtemp())
        local, target = root / 'local', root / 'target'
        local.mkdir()
        target.mkdir()
        link = local / 'link'
        try:
            if sys.platform == 'win32':
                import _winapi
                _winapi.CreateJunction(str(target), str(link))
            else:
                link.symlink_to(target, target_is_directory=True)
        except OSError as error:
            self.skipTest(f'cannot make a link here: {error}')
        with self.assertRaises(ValueError):
            lr.checked_out(link / 'run', local)

    def test_main_refuses_outside_out(self):
        root = Path(tempfile.mkdtemp())
        self.assertEqual(lr.main([str(root / 'x.log'), str(root / 'x.wav'), str(root / 'out')]), 2)
        self.assertFalse((root / 'out').exists())

    def test_labels_parse(self):
        run = lr.parse_log(log())
        fire = [c for c in run['cues'] if c['name'] == 'fire'][0]
        self.assertEqual((fire['kind'], fire['seconds'], fire['frame']), ('sound', 0.5, None))
        self.assertEqual(run['summary'], ('1', '0', '0'))
        self.assertEqual(run['written'], (True, 1000))


if __name__ == '__main__':
    unittest.main()
