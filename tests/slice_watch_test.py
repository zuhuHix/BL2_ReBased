"""tools/real_game/scripts/slice_watch.py and tools/real_game/slice_watch_check.py with invented rows and no game, plus the order of the
slice-session switches in the captures that take them (lane capture-prep, 2026-11-11). Every value here is made up."""
import importlib.util
import json
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


watch = load('slice_watch', 'tools/real_game/scripts/slice_watch.py')
checker = load('slice_watch_check', 'tools/real_game/slice_watch_check.py')


class Args:
    def __init__(self, **values):
        self.__dict__.update(values)


class Func:
    """A hook's func argument: its parameters are at .func._properties() (as tests/den_probe_test.py's BoundFunction)."""
    def __init__(self, *names):
        self.func = type('F', (), {'_properties': lambda _self: [type('P', (), {'Name': n}) for n in names]})()


class RecorderTest(unittest.TestCase):
    def setUp(self):
        watch.sw_reset('t', watch.load_spec({'funcs': ['Pawn.TakeDamage'], 'per_func_cap': 2, 'row_cap': 3}))
        watch.SW['on'] = True
        watch.get_pc = lambda: (_ for _ in ()).throw(RuntimeError('no game'))   # the game clock reads as None

    def test_names(self):
        self.assertEqual(watch.parse_func('Pawn.TakeDamage'), ('decl', ('Pawn', 'TakeDamage')))
        self.assertEqual(watch.parse_func('Pkg.Thing:Fn'), ('path', 'Pkg.Thing:Fn'))
        for bad in ('TakeDamage', 'Pawn.Take Damage', 'a.b.c', '', 'Pkg.Thing:Fn:Two'):
            self.assertIsNone(watch.parse_func(bad), bad)

    def test_spec_refuses_typos_and_bad_names(self):
        with self.assertRaises(ValueError):
            watch.load_spec({'funcs': ['Pawn.TakeDamage'], 'frame': True})   # 'frames' misspelt
        with self.assertRaises(ValueError):
            watch.load_spec({'funcs': ['TakeDamage']})
        spec = watch.load_spec({'funcs': [], 'checks': {'max_errors': 0}, 'notes': 'x'})
        self.assertNotIn('checks', spec)

    def test_the_observer_never_blocks_and_rows_are_capped(self):
        hook = watch._observer('Pkg.Pawn:TakeDamage', 'pre')
        args = Args(DamageAmount=12.5, EventInstigator=None)
        results = [hook(None, args, None, Func('DamageAmount', 'EventInstigator')) for _ in range(4)]
        self.assertEqual(results, [None] * 4)
        rows = watch.SW['rows']
        self.assertEqual(len(rows), 2)   # per_func_cap 2
        self.assertEqual(rows[0]['args'], {'DamageAmount': 12.5, 'EventInstigator': None})
        self.assertEqual(watch.SW['dropped'], {'Pkg.Pawn:TakeDamage': 2})
        post = watch._observer('Pkg.Pawn:Died', 'post')
        self.assertIsNone(post(None, Args(), 7, Func()))
        self.assertEqual(watch.SW['rows'][-1]['ret'], 7)
        self.assertIsNone(post(None, Args(), 7, Func()))
        self.assertEqual(len(watch.SW['rows']), 3)   # row_cap 3

    def test_an_error_in_a_row_is_counted_not_raised(self):
        hook = watch._observer('Pkg.Pawn:TakeDamage', 'pre')
        watch.SW['spec'] = None   # breaks add_row
        self.assertIsNone(hook(None, Args(), None, Func()))
        self.assertEqual(len(watch.SW['errors']), 1)

    def test_off_records_nothing(self):
        watch.SW['on'] = False
        self.assertIsNone(watch._observer('Pkg.Pawn:TakeDamage', 'pre')(None, Args(), None, Func()))
        self.assertEqual(watch.SW['rows'], [])

    def test_the_pawn_filter(self):
        self.assertTrue(watch.pawn_wanted({'name': 'Lvl.TheWorld:PersistentLevel.WillowAIPawn_7'}, []))
        self.assertTrue(watch.pawn_wanted({'name': 'Lvl.TheWorld:PersistentLevel.WillowAIPawn_7'}, ['WillowAIPawn_7']))
        self.assertTrue(watch.pawn_wanted({'name': 'x', 'ai_class': 'CharClass_Invented'}, ['Invented']))
        self.assertFalse(watch.pawn_wanted({'name': 'x', 'cls': 'y'}, ['WillowAIPawn_7']))

    def test_automatic_reads_wait_for_a_pawn_and_their_errors_are_apart(self):
        watch.SW['spec']['snap_every'] = 0.001
        watch._tick(None, Args(DeltaTime=0.0167), None, None)   # no game: no pawn, so no automatic snapshot
        self.assertEqual(watch.SW['snaps'], [])
        self.assertEqual(watch.SW['errors'], [])
        real = watch._auto_reads_ok
        watch._auto_reads_ok = lambda: True
        try:
            watch._tick(None, Args(DeltaTime=0.0167), None, None)   # the snapshot's reads fail without a game
        finally:
            watch._auto_reads_ok = real
        self.assertEqual(len(watch.SW['snaps']), 1)
        self.assertEqual(watch.SW['errors'], [])
        self.assertGreaterEqual(watch.SW['soft_errors'], 1)

    def test_the_player_read_is_empty_without_a_game(self):
        self.assertEqual(watch._player_here(), {'loc': None})   # get_pc raises here: no exception escapes

    def test_frame_rows_carry_three_deltas(self):
        first = watch.frame_row(0.0167, 1.0, 10.0)
        second = watch.frame_row(0.0167, 1.02, 10.0167)
        self.assertIsNone(first['dt_real'])
        self.assertAlmostEqual(second['dt_real'], 0.02)
        self.assertAlmostEqual(second['dt_game'], 0.0167)
        self.assertIsNone(watch.frame_row(0.0167, 1.04, None)['dt_game'])

    def test_dump_names_and_folder(self):
        with self.assertRaises(ValueError):
            watch.sw_dump('../escape')
        with tempfile.TemporaryDirectory() as tmp:
            watch.RG_OUT = str(Path(tmp) / 'realgame' / 'x')   # no repository's local/ above it
            with self.assertRaises(ValueError):
                watch.sw_dump('watch')
            repo = Path(tmp) / 'repo'
            (repo / '.git').mkdir(parents=True)
            watch.RG_OUT = str(repo / 'local' / 'realgame' / 'x')
            counts = watch.sw_dump('watch')
            self.assertEqual(counts['rows'], 0)
            self.assertTrue((repo / 'local' / 'realgame' / 'x' / 'watch.json').exists())

    def test_the_probe_holds_no_writes(self):
        """The recorder reads only: no setter, no Block, no console command, no input. A guard on the source, for the reviewer."""
        text = (ROOT / 'tools/real_game/scripts/slice_watch.py').read_text(encoding='utf-8')
        code = '\n'.join(re.sub(r'\s#\s.*$', '', line) for line in text.splitlines() if not line.lstrip().startswith('#'))
        code = re.sub(r'"""[\s\S]*?"""', '', code)
        for word in ('Block', 'ConsoleCommand', 'SetHealth', 'TakeDamage(', 'SetLocation', 'Destroy', 'StartFire', 'keybd',
                     'ServerSetSkillSaveGameData', 'AddBackpack', '.Location =', '.Rotation =', 'setattr('):
            self.assertNotIn(word, code, word)
        self.assertNotRegex(code, r'\b\w+\.\w+\s*=[^=]', 'an attribute assignment on a game object')


def dump(rows=(), frames=(), snaps=(), pawns=(), missing=(), errors=()):
    return {'rows': list(rows), 'frames': list(frames), 'snaps': list(snaps), 'pawns': list(pawns), 'missing': list(missing), 'errors': list(errors)}


def row(func, g, tag='pre', dt=0.0167, **args):
    return {'func': func, 'g': g, 'tag': tag, 'dt': dt, 'args': args}


class CheckerTest(unittest.TestCase):
    def results(self, data, checks):
        return {name: ok for name, ok, _ in checker.check(data, checks)}

    def test_require_order_and_gap(self):
        rows = [row('X.Pawn:ResurrectPlayer', 100.0), row('X.Pawn:NotifyTeleported', 103.0), row('X.Pawn:ShowRespawnDialog', 111.0)]
        got = self.results(dump(rows), {
            'require': [{'func': 'ResurrectPlayer', 'min': 1}, {'func': 'NoSuchCall', 'min': 1}, {'func': 'NotifyTeleported', 'max': 0}],
            'order': [['ResurrectPlayer', 'ShowRespawnDialog'], ['ShowRespawnDialog', 'NotifyTeleported']],
            'gap': [{'from': 'ResurrectPlayer', 'to': 'ShowRespawnDialog', 'min': 5, 'max': 30},
                    {'from': 'ResurrectPlayer', 'to': 'NotifyTeleported', 'min': 5, 'max': 30}]})
        self.assertTrue(got['require ResurrectPlayer'])
        self.assertFalse(got['require NoSuchCall'])
        self.assertFalse(got['require NotifyTeleported'])
        self.assertTrue(got['order ResurrectPlayer -> ShowRespawnDialog'])
        self.assertFalse(got['order ShowRespawnDialog -> NotifyTeleported'])
        self.assertTrue(got['gap ResurrectPlayer -> ShowRespawnDialog'])
        self.assertFalse(got['gap ResurrectPlayer -> NotifyTeleported'])

    def test_press_and_release_must_both_be_rows(self):
        both = [row('X.Movie:HandleInputKey', 1.0, ukey='Enter', uevent=0), row('X.Movie:HandleInputKey', 1.1, ukey='Enter', uevent=1)]
        edges = {'edges': [{'func': 'HandleInputKey', 'arg': 'uevent', 'values': [0, 1]}]}
        self.assertTrue(self.results(dump(both), edges)['edges HandleInputKey.uevent'])
        self.assertFalse(self.results(dump(both[:1]), edges)['edges HandleInputKey.uevent'])

    def test_mods_frames_snaps_pawns(self):
        start = {'label': 'start', 'player': {}, 'pools': [], 'mods': [{'name': 'Driver', 'enabled': True}, {'name': 'Other', 'enabled': False}]}
        frames = [{'dt_arg': 0.0167}] * 95 + [{'dt_arg': 0.5}] * 5
        pawns = [{'pawns': [{'cylinder': {'radius': 1.0}, 'GroundSpeed': 2.0}]}]
        checks = {'mods_allowed': ['Driver'], 'snaps': ['start', 'end'], 'snap_fields': ['player', 'pools'],
                  'frames': {'min': 50, 'dt_min': 0.005, 'dt_max': 0.1, 'share': 0.95},
                  'pawns': {'min_reads': 1, 'fields': ['cylinder', 'GroundSpeed']}}
        got = self.results(dump(snaps=[start], frames=frames, pawns=pawns), checks)
        self.assertTrue(got['mods_allowed'])
        self.assertTrue(got['snap start'])
        self.assertFalse(got['snap end'])
        self.assertTrue(got['snap_fields'])
        self.assertTrue(got['frames'])
        self.assertTrue(got['pawns'])
        start['mods'][1]['enabled'] = True
        self.assertFalse(self.results(dump(snaps=[start]), checks)['mods_allowed'])
        self.assertFalse(self.results(dump(snaps=[dict(start, mods=[{'error': 'x'}])]), checks)['mods_allowed'])
        self.assertFalse(self.results(dump(frames=frames[:40]), checks)['frames'])
        self.assertFalse(self.results(dump(pawns=[{'pawns': [{'cylinder': None, 'GroundSpeed': 2.0}]}]), checks)['pawns'])

    def test_named_mods_must_be_off(self):
        start = {'label': 'start', 'mods': [{'name': 'Better Thing', 'enabled': False}, {'name': 'Library X', 'enabled': True}]}
        checks = {'mods_must_be_off': ['better thing']}
        self.assertTrue(self.results(dump(snaps=[start]), checks)['mods_must_be_off'])
        start['mods'][0]['enabled'] = True
        self.assertFalse(self.results(dump(snaps=[start]), checks)['mods_must_be_off'])
        self.assertFalse(self.results(dump(), checks)['mods_must_be_off'])   # no start snapshot: not shown off

    def test_errors_and_missing(self):
        self.assertFalse(self.results(dump(errors=['e']), {})['max_errors'])
        self.assertTrue(self.results(dump(errors=['e']), {'max_errors': 1})['max_errors'])
        self.assertFalse(self.results(dump(missing=['A.B']), {'no_missing': True})['no_missing'])
        self.assertFalse(self.results(dump([row('A', 1.0, dt=None)]), {'rows_have_dt': True})['rows_have_dt'])

    def test_the_command_line_prints_counts_and_exits_by_result(self):
        with tempfile.TemporaryDirectory() as tmp:
            d, s = Path(tmp) / 'watch.json', Path(tmp) / 'spec.json'
            d.write_text(json.dumps(dump([row('X.Pawn:Died', 1.0, DamageAmount=12345.5)])), encoding='utf-8')
            s.write_text(json.dumps({'funcs': ['Pawn.Died'], 'checks': {'require': [{'func': 'Died', 'min': 1}]}}), encoding='utf-8')
            ok = subprocess.run([sys.executable, str(ROOT / 'tools/real_game/slice_watch_check.py'), str(d), '--spec', str(s)], capture_output=True, text=True)
            self.assertEqual(ok.returncode, 0, ok.stdout)
            self.assertIn('USABLE', ok.stdout)
            self.assertNotIn('12345', ok.stdout)   # no recorded value is printed
            s.write_text(json.dumps({'checks': {'require': [{'func': 'Missing', 'min': 1}]}}), encoding='utf-8')
            bad = subprocess.run([sys.executable, str(ROOT / 'tools/real_game/slice_watch_check.py'), str(d), '--spec', str(s)], capture_output=True, text=True)
            self.assertEqual(bad.returncode, 1)
            self.assertIn('NOT USABLE', bad.stdout)
            gone = subprocess.run([sys.executable, str(ROOT / 'tools/real_game/slice_watch_check.py'), str(Path(tmp) / 'none.json'), '--spec', str(s)], capture_output=True, text=True)
            self.assertEqual(gone.returncode, 2)


WIRED = ('mission_screen_capture.ps1', 'cash_capture.ps1', 'phaselock_integrity_capture.ps1', 'audio_events_capture.ps1')


class WiringTest(unittest.TestCase):
    """The order the switches must keep in each capture: mods off only after the backup record and before the driver and the launch; the
    watch started after the probes load and checked (its hooks off) before the quit; nothing added to the pinned finalizer."""
    def test_order(self):
        for name in WIRED:
            text = (ROOT / 'tools/real_game' / name).read_text(encoding='utf-8')
            at = {key: text.find(needle) for key, needle in (
                ('record', 'Save-VerifiedBackupRecord $bak'), ('mods', 'Invoke-SdkModsOffStep $log'), ('driver', 'Install-Driver -'),
                ('launch', 'Start-Game '), ('start', 'Start-SliceWatch $WatchSpec'), ('check', 'Complete-SliceWatch $WatchSpec'),
                ('quit', 'ConsoleCommand("quit"'), ('finally', '\n} catch {\n    $runError'))}
            for key, pos in at.items():
                self.assertGreaterEqual(pos, 0, f'{name}: {key} not found')
            self.assertTrue(at['record'] < at['mods'] < at['driver'] < at['launch'] < at['start'] < at['check'], name)
            self.assertLess(at['check'], text.find('ConsoleCommand("quit"', at['start']), name)
            # The finalizer is pinned word for word (tests/real_game_capture_safety_test.py): the watch adds nothing to it.
            self.assertNotIn('Watch', text[at['finally']:], name)
            self.assertIn("if ($SdkModsOff) { Invoke-SdkModsOffStep $log }", text, name)
            self.assertIn(". (Join-Path $PSScriptRoot 'slice_watch.ps1')", text, name)

    def test_den_down_removes_only_its_own_hook_and_sends_no_new_input(self):
        text = (ROOT / 'tools/real_game/cash_capture.ps1').read_text(encoding='utf-8')
        block = text[text.index('if ($DenDown) {\n        try {'):text.index("Set-Stage 'den_down' ($null")]
        self.assertIn("cp_immune_off()", block)
        # The only key is the Enter every den trial already sends after the map load; no click, no movement key.
        self.assertEqual(re.findall(r'Send-\w+ \w+', re.sub(r'#.*', '', block)), ['Send-Key Enter'])
        probe = (ROOT / 'tools/real_game/scripts/cash_probe.py').read_text(encoding='utf-8')
        body = probe[probe.index('def cp_immune_off'):probe.index('def cp_immune():')]
        self.assertIn('remove_hook(path, Type.PRE, CP_IMMUNE_ID)', body)
        self.assertNotIn('add_hook', body)


class HelperTest(unittest.TestCase):
    """tools/real_game/slice_watch.ps1 with the game calls stubbed: the dump is checked before the game would close, a traceback is never an
    answer, and a bad name or spec path is refused before anything is sent."""
    def run_ps(self, body):
        shell = 'pwsh' if subprocess.run(['where', 'pwsh'], capture_output=True, shell=True).returncode == 0 else 'powershell'
        script = "$ErrorActionPreference = 'Stop'\n. '" + str(ROOT / 'tools/real_game/slice_watch.ps1') + "'\n" + body
        return subprocess.run([shell, '-NoProfile', '-NonInteractive', '-Command', script], capture_output=True, text=True, timeout=120)

    @unittest.skipUnless(sys.platform == 'win32', 'the capture helpers run on Windows')
    def test_complete_checks_the_dump(self):
        with tempfile.TemporaryDirectory() as tmp:
            spec = Path(tmp) / 'spec.json'
            spec.write_text(json.dumps({'funcs': ['Pawn.Died'], 'checks': {'require': [{'func': 'Died', 'min': 1}]}}), encoding='utf-8')
            (Path(tmp) / 'watch.json').write_text(json.dumps(dump([row('X.Pawn:Died', 1.0)])), encoding='utf-8')
            body = ("function Invoke-GamePy([string]$Code, [int]$T = 30) { $script:sent += @($Code); if ($Code -like '*sw_dump*') { '{\"rows\": 1}' } elseif ($Code -like '*sw_stop*') { 'stopped' } else { '3' } }\n"
                    f"$w = Complete-SliceWatch '{spec}' '{tmp}'\n"
                    "'usable=' + $w.usable; 'sent=' + $script:sent.Count; $w.check")
            done = self.run_ps(body)
            self.assertIn('usable=True', done.stdout, done.stdout + done.stderr)
            self.assertIn('sent=3', done.stdout)   # end snapshot, dump, stop
            (Path(tmp) / 'watch.json').write_text(json.dumps(dump()), encoding='utf-8')
            self.assertIn('usable=False', self.run_ps(body).stdout)

    @unittest.skipUnless(sys.platform == 'win32', 'the capture helpers run on Windows')
    def test_refusals(self):
        body = ("function Invoke-GamePy([string]$Code, [int]$T = 30) { 'Traceback (most recent call last): boom' }\n"
                "try { Save-SliceWatchSnap 'a b' } catch { 'refused name' }\n"
                "try { Save-SliceWatchSnap 'ok' } catch { 'refused traceback' }\n"
                "try { Start-SliceWatch 'C:\\no\\such\\spec.json' 'x' 'x' } catch { 'refused spec' }")
        out = self.run_ps(body).stdout
        for word in ('refused name', 'refused traceback', 'refused spec'):
            self.assertIn(word, out)


if __name__ == '__main__':
    unittest.main()
