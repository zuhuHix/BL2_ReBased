"""tools/listen_report.py on an invented log and a synthetic WAV (tones and silence written here). No game data."""
from pathlib import Path
import array
import json
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

    def test_equip_and_reload_cues_are_steps_sounds(self):
        rows = [
            'OWAUDIOREC start file=C:/x/listen.wav map=Sanctuary_P (test only; records what the host sent to the mixer)',
            'OWAUDIOREC cue t=0.100 label=step Equip frame=frame-001.png',
            'OWAUDIOREC cue t=0.200 label=equip seconds=0.400 frame=-',
            'OWAUDIOREC cue t=2.000 label=step Fight frame=frame-002.png',
            'OWAUDIOREC cue t=2.500 label=reload_out seconds=0.200 frame=-',
            'OWAUDIOREC cue t=3.200 label=reload_in seconds=0.500 frame=-',
            'OWAUDIOREC stop t=6.000 file=C:/x/listen.wav',
            'OWAUDIOREC written=1 bytes=1000 after 1.0 s',
        ]
        text = '\n'.join('[2026.11.08-12.00.00:000][  1]LogTemp: Display: ' + r for r in rows)
        out, problems, steps, _ = self.run_build(text=text, tones=((0.2, 0.4), (2.5, 0.2), (3.2, 0.5)))
        self.assertEqual(problems, [])
        self.assertEqual([s['name'] for s in steps], ['Equip', 'Fight'])
        self.assertEqual([s['name'] for s in steps[0]['sounds']], ['equip'])
        self.assertEqual([s['name'] for s in steps[1]['sounds']], ['reload_out', 'reload_in'])
        listen = (out / 'LISTEN.md').read_text(encoding='utf-8')
        self.assertIn('**equip** x1', listen)
        self.assertIn('**reload_out** x1', listen)
        self.assertIn('**reload_in** x1', listen)
        self.assertNotIn('Equip / draw a gun', listen)
        self.assertNotIn('Reload (clip out / clip in)', listen)

    def test_equip_and_reload_names_come_from_the_manifest(self):
        tmp = Path(tempfile.mkdtemp())
        manifest = {'schema': 'ow-audio-v1', 'entries': [
            {'key': 'weapon:fire', 'ak_event': {'wwise_name': 'Play_Synth_Shot'}},
            {'key': 'weapon:equip', 'ak_event': {'wwise_name': 'Play_Synth_Equip'}},
            {'key': 'weapon:reload_anim_Synth_at_0.5s', 'ak_event': {'wwise_name': 'Play_Synth_Clip_Out'}},
            {'key': 'weapon:reload_anim_Synth_at_1.5s', 'ak_event': {'wwise_name': 'Play_Synth_Clip_In'}},
            {'key': 'door:open', 'ak_event': {'wwise_name': 'Play_Synth_Door_Open'}},
            {'key': 'door:close', 'ak_event': {'wwise_name': 'Play_Synth_Door_Close'}}]}
        path = tmp / 'audio.json'
        path.write_text(json.dumps(manifest), encoding='utf-8')
        self.assertEqual(lr.load_names(path), {'fire': 'Play_Synth_Shot', 'equip': 'Play_Synth_Equip',
                                               'reload_out': 'Play_Synth_Clip_Out', 'reload_in': 'Play_Synth_Clip_In',
                                               'door_open': 'Play_Synth_Door_Open', 'door_close': 'Play_Synth_Door_Close'})
        self.assertTrue({'equip', 'reload_out', 'reload_in', 'door_open', 'door_close'} <= set(lr.ROLES))
        self.assertNotIn('Door open / close', [what for what, _ in lr.NOT_PLAYED])

    def test_door_cues_are_steps_sounds(self):
        rows = [
            'OWAUDIOREC start file=C:/x/listen.wav map=Sanctuary_P (test only; records what the host sent to the mixer)',
            'OWAUDIOREC cue t=0.100 label=step Fight frame=frame-001.png',
            'OWAUDIOREC cue t=0.500 label=door_open seconds=1.000 frame=-',
            'OWAUDIOREC cue t=2.000 label=step Recover frame=frame-002.png',
            'OWAUDIOREC cue t=2.300 label=door_close seconds=1.400 frame=-',
            'OWAUDIOREC stop t=6.000 file=C:/x/listen.wav',
            'OWAUDIOREC written=1 bytes=1000 after 1.0 s',
        ]
        text = '\n'.join('[2026.11.09-12.00.00:000][  1]LogTemp: Display: ' + r for r in rows)
        out, problems, steps, _ = self.run_build(text=text, tones=((0.5, 1.0), (2.3, 1.4)))
        self.assertEqual(problems, [])
        self.assertEqual([s['name'] for s in steps[0]['sounds']], ['door_open'])
        self.assertEqual([s['name'] for s in steps[1]['sounds']], ['door_close'])
        listen = (out / 'LISTEN.md').read_text(encoding='utf-8')
        self.assertIn('**door_open** x1', listen)
        self.assertIn('**door_close** x1', listen)
        self.assertNotIn('Door open / close', listen)

    def test_dialog_line_cues_are_steps_sounds_with_one_description(self):
        rows = [
            'OWAUDIOREC start file=C:/x/listen.wav map=Sanctuary_P (test only; records what the host sent to the mixer)',
            'OWAUDIOREC cue t=0.100 label=step Accept frame=frame-001.png',
            'OWAUDIOREC cue t=0.500 label=line:Pkg.Events.Invented_Line_A seconds=1.000 frame=-',
            'OWAUDIOREC cue t=2.000 label=step WaitWalk frame=frame-002.png',
            'OWAUDIOREC cue t=2.300 label=line:Pkg.Events.Invented_Line_B seconds=1.400 frame=-',
            'OWAUDIOREC stop t=6.000 file=C:/x/listen.wav',
            'OWAUDIOREC written=1 bytes=1000 after 1.0 s',
        ]
        text = '\n'.join('[2026.11.09-12.00.00:000][  1]LogTemp: Display: ' + r for r in rows)
        out, problems, steps, _ = self.run_build(text=text, tones=((0.5, 1.0), (2.3, 1.4)))
        self.assertEqual(problems, [])
        self.assertEqual([s['name'] for s in steps[0]['sounds']], ['line:Pkg.Events.Invented_Line_A'])
        self.assertEqual([s['name'] for s in steps[1]['sounds']], ['line:Pkg.Events.Invented_Line_B'])
        listen = (out / 'LISTEN.md').read_text(encoding='utf-8')
        self.assertIn('**line:Pkg.Events.Invented_Line_A** x1', listen)
        self.assertNotIn('a sound the guide does not describe', listen)
        self.assertEqual(lr.describe('line:Anything'), lr.ROLES[lr.LINE_ROLE][2])
        self.assertNotIn('dialog is not played by the host', listen)


    def display_text(self, rows):
        return '\n'.join('[2026.11.09-12.00.00:000][  1]LogTemp: Display: ' + r for r in rows)

    def test_magazine_fixture_is_stated_and_its_reload_is_expected(self):
        text = self.display_text([
            'OWAUDIOREC start file=C:/x/listen.wav map=Sanctuary_P (test only; records what the host sent to the mixer)',
            'OWSLICEGATE note test fixture: magazine of Synth_Gun set to 1 rounds (-owslicegatemag=1, size 12)',
            'OWAUDIOREC cue t=0.100 label=step ShootDummy frame=frame-001.png',
            'OWAUDIOREC cue t=2.000 label=reload_out seconds=0.200 frame=-',
            'OWAUDIOREC cue t=3.000 label=reload_in seconds=0.500 frame=-',
            'OWAUDIOREC stop t=6.000 file=C:/x/listen.wav',
            'OWAUDIOREC written=1 bytes=1000 after 1.0 s',
        ])
        out, problems, steps, run = self.run_build(text=text, tones=((2.0, 0.2), (3.0, 0.5)))
        self.assertEqual(problems, [])
        self.assertEqual(run['magazine_fixture'], ('Synth_Gun', 1))
        listen = (out / 'LISTEN.md').read_text(encoding='utf-8')
        self.assertIn('Test fixture: the held gun (Synth_Gun) had its magazine set to 1 round(s)', listen)

    def test_magazine_fixture_without_a_logged_reload_is_a_problem(self):
        text = self.display_text([
            'OWAUDIOREC start file=C:/x/listen.wav map=Sanctuary_P (test only; records what the host sent to the mixer)',
            'OWSLICEGATE note test fixture: magazine of Synth_Gun set to 1 rounds (-owslicegatemag=1, size 12)',
            'OWAUDIOREC cue t=0.100 label=step ShootDummy frame=frame-001.png',
            'OWAUDIOREC stop t=6.000 file=C:/x/listen.wav',
            'OWAUDIOREC written=1 bytes=1000 after 1.0 s',
        ])
        _, problems, _, _ = self.run_build(text=text, tones=((2.0, 0.5),))
        self.assertTrue(any('reload' in p and 'reload started' in p for p in problems), problems)

    def test_no_fixture_says_so(self):
        out, _, _, run = self.run_build()
        self.assertIsNone(run['magazine_fixture'])
        self.assertIn('No magazine fixture in this run', (out / 'LISTEN.md').read_text(encoding='utf-8'))

    def test_refused_magazine_fixture_is_a_problem(self):
        text = self.display_text([
            'OWAUDIOREC start file=C:/x/listen.wav map=Sanctuary_P (test only; records what the host sent to the mixer)',
            'OWSLICEGATE note test fixture: -owslicegatemag=1 not applied, no magazine-fed gun in hand',
            'OWAUDIOREC cue t=0.100 label=step ShootDummy frame=frame-001.png',
            'OWAUDIOREC stop t=6.000 file=C:/x/listen.wav',
            'OWAUDIOREC written=1 bytes=1000 after 1.0 s',
        ])
        out, problems, steps, run = self.run_build(text=text, tones=((2.0, 0.5),))
        self.assertEqual(run['magazine_refused'], (1, 'no magazine-fed gun in hand'))
        self.assertIsNone(run['magazine_fixture'])
        self.assertTrue(any('not applied' in p and 'no magazine-fed gun in hand' in p for p in problems), problems)
        # Not applied: no reload problem on top of it, and the step keeps its plain text.
        self.assertFalse(any('reload sound' in p for p in problems), problems)
        listen = (out / 'LISTEN.md').read_text(encoding='utf-8')
        self.assertIn('On screen: Maya shoots the dummy with the lent pistol.', listen)
        self.assertNotIn('she reloads before turning in', listen)

    def test_shootdummy_text_says_reload_only_when_the_fixture_applied(self):
        applied = self.display_text([
            'OWAUDIOREC start file=C:/x/listen.wav map=Sanctuary_P (test only; records what the host sent to the mixer)',
            'OWSLICEGATE note test fixture: magazine of Synth_Gun set to 1 rounds (-owslicegatemag=1, size 12)',
            'OWAUDIOREC cue t=0.100 label=step ShootDummy frame=frame-001.png',
            'OWAUDIOREC cue t=2.000 label=reload_out seconds=0.200 frame=-',
            'OWAUDIOREC cue t=3.000 label=reload_in seconds=0.500 frame=-',
            'OWAUDIOREC stop t=6.000 file=C:/x/listen.wav',
            'OWAUDIOREC written=1 bytes=1000 after 1.0 s',
        ])
        out, _, _, _ = self.run_build(text=applied, tones=((2.0, 0.2), (3.0, 0.5)))
        self.assertIn('she reloads before turning in', (out / 'LISTEN.md').read_text(encoding='utf-8'))


class LevelTableTest(unittest.TestCase):
    """The per-role level table against the game's mix, on synthetic files and an invented log."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())

    def source(self, name, seconds, amplitude, channels=1):
        path = self.tmp / name
        write_wav(path, seconds, ((0.0, seconds),), channels=channels, amplitude=amplitude)
        return str(path)

    def cue(self, name, rms):
        return {'kind': 'sound', 'name': name, 'level': {'rms_dbfs': rms}}

    def test_balance_against_the_shot_matches_when_the_mix_is_the_games(self):
        shot = self.source('1.wav', 0.5, 0.5)
        equip = self.source('2.wav', 0.5, 0.125)    # 12 dB under the shot's file
        play = {'roles': {'fire': [shot], 'equip': [equip]}, 'gains': {'fire': [-12.0], 'equip': [-6.0]}}
        shot_file, equip_file = lr.file_level(Path(shot)), lr.file_level(Path(equip))
        self.assertAlmostEqual(shot_file - equip_file, 20 * math.log10(4), places=1)
        # Recorded as the game would mix it, plus a 3 dB offset shared by the whole mix.
        cues = [self.cue('fire', shot_file - 12 - 3), self.cue('equip', equip_file - 6 - 3)]
        rows = {r['role']: r for r in lr.level_table(cues, '', play)}
        self.assertAlmostEqual(rows['equip']['game_vs_shot_db'], -12 + 6, delta=0.15)   # the files are 12.04 dB apart; levels round to 0.1
        self.assertAlmostEqual(rows['equip']['off_by_db'], 0.0, delta=0.15)
        self.assertEqual(rows['fire']['off_by_db'], 0.0)

    def test_a_role_quieter_than_the_games_balance_shows_how_far(self):
        shot = self.source('1.wav', 0.5, 0.5)
        equip = self.source('2.wav', 0.5, 0.5)
        play = {'roles': {'fire': [shot], 'equip': [equip]}, 'gains': {'fire': [0.0], 'equip': [0.0]}}
        level = lr.file_level(Path(shot))
        rows = {r['role']: r for r in lr.level_table([self.cue('fire', level), self.cue('equip', level - 9)], '', play)}
        self.assertEqual(rows['equip']['off_by_db'], -9.0)

    def test_a_positioned_role_gets_no_prediction(self):
        line = self.source('3.wav', 0.5, 0.5)
        text = 'LogTemp: Display: OWAUDIO play role=line:x channels=1 rate=8000 bytes=8000 gain_db=-3.0 at=Marcus_0 (the host called play; not heard)'
        rows = lr.level_table([self.cue('line:x', -30.0)], text, {'roles': {'line:x': [line]}, 'gains': {'line:x': [-3.0]}})
        self.assertTrue(rows[0]['positioned'])
        self.assertIsNone(rows[0]['expected_rms_dbfs'])

    def test_without_a_game_gain_there_is_no_game_prediction(self):
        """A logged gain_db is what the host applied, which is 0 dB for a fallback: it is not the game's balance."""
        shot = self.source('1.wav', 0.5, 0.5)
        text = 'LogTemp: Display: OWAUDIO play role=fire channels=2 rate=8000 bytes=8000 gain_db=0.0 upmix=mono_to_stereo (the host called play; not heard)'
        for play in ({'roles': {'fire': [shot]}}, {'roles': {'fire': [shot]}, 'gains': {'fire': [None]}, 'gain_notes': {'fire': 'no game value: no game data given'}}):
            rows = lr.level_table([self.cue('fire', -20.0)], text, play)
            self.assertIsNone(rows[0]['expected_rms_dbfs'])
            self.assertEqual(rows[0]['prediction'], 'no game prediction')
        self.assertIn('no game data', rows[0]['gain_note'])

    def test_each_gain_goes_with_its_own_file_before_the_median(self):
        files = [self.source('%d.wav' % n, 0.5, amplitude) for n, amplitude in ((1, 0.9), (2, 0.285), (3, 0.09))]   # about 10 dB apart
        levels = [lr.file_level(Path(f)) for f in files]
        play = {'roles': {'hit': files}, 'gains': {'hit': [-20.0, 0.0, 0.0]}}
        row = lr.level_table([self.cue('hit', -30.0)], '', play)[0]
        paired = sorted([levels[0] - 20, levels[1], levels[2]])[1]
        unpaired = sorted(levels)[1] + 0.0          # the median level plus the median gain: what the table must not do
        self.assertAlmostEqual(row['expected_rms_dbfs'], round(paired, 1), places=1)
        self.assertGreater(abs(row['expected_rms_dbfs'] - unpaired), 5)

    def test_a_role_with_some_null_gains_predicts_from_the_others_and_says_so(self):
        files = [self.source('1.wav', 0.5, 0.5), self.source('2.wav', 0.5, 0.05)]
        row = lr.level_table([self.cue('hit', -30.0)], '', {'roles': {'hit': files}, 'gains': {'hit': [-6.0, None]}})[0]
        self.assertAlmostEqual(row['expected_rms_dbfs'], round(lr.file_level(Path(files[0])) - 6, 1), places=1)
        self.assertEqual(row['prediction'], 'from 1 of 2 file(s)')

    def test_the_guide_shows_partial_coverage_and_counts_every_cue(self):
        files = [self.source('1.wav', 0.5, 0.5), self.source('2.wav', 0.5, 0.05)]
        play_map = self.tmp / 'play.json'
        play_map.write_text(json.dumps({'roles': {'fire': files, 'hit': files}, 'gains': {'fire': [-12.0, None], 'hit': [-11.0, -11.0]}, 'unresolved': {}}),
                            encoding='utf-8')
        wav = self.tmp / 'listen.wav'
        write_wav(wav, 9.0, ((2.5, 0.5),))
        # A second fire cue past the recording's end has no level: it is still counted.
        extra = '\n[x]LogTemp: Display: OWAUDIOREC cue t=50.000 label=fire seconds=0.500 frame=-'
        text = log().replace('label=fire seconds=0.500 frame=-', 'label=fire seconds=0.500 frame=-' + extra)
        lr.build(text, wav, self.tmp / 'out', play_map=play_map, local_root=self.tmp)
        guide = (self.tmp / 'out' / 'LISTEN.md').read_text(encoding='utf-8')
        fire = next(line for line in guide.splitlines() if line.startswith('| `fire` |') and 'dBFS' in line)
        self.assertIn('(from 1 of 2 file(s))', fire)
        self.assertIn('| 2 (1 measured) |', fire)
        hit = next(line for line in guide.splitlines() if line.startswith('| `hit` |') and 'dBFS' in line)
        self.assertNotIn('file(s))', hit)

    def test_the_guide_has_the_table_and_cues_json_the_rows(self):
        shot = self.source('1.wav', 0.5, 0.5)
        play_map = self.tmp / 'play.json'
        play_map.write_text(json.dumps({'roles': {'fire': [shot], 'hit': [shot]}, 'gains': {'fire': [-12.0], 'hit': [-11.0]}, 'unresolved': {}}), encoding='utf-8')
        wav = self.tmp / 'listen.wav'
        write_wav(wav, 9.0, ((2.5, 0.5),))
        lr.build(log(), wav, self.tmp / 'out', play_map=play_map, local_root=self.tmp)
        text = (self.tmp / 'out' / 'LISTEN.md').read_text(encoding='utf-8')
        self.assertIn("## Levels against the game's mix", text)
        self.assertIn('| `fire` | 1 |', text)
        levels = json.loads((self.tmp / 'out' / 'cues.json').read_text(encoding='utf-8'))['levels']
        self.assertEqual(sorted(r['role'] for r in levels), ['fire', 'hit'])
        self.assertEqual(next(r for r in levels if r['role'] == 'hit')['game_vs_shot_db'], 1.0)


if __name__ == '__main__':
    unittest.main()
