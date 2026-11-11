"""tools/real_game/scripts/slice_watch.py, tools/real_game/slice_watch_check.py and tools/real_game/slice_watch.ps1 with invented values and no
game, the shared command namespace each capture loads its scripts into, and the order of the slice-session switches in the captures that take
them (lane capture-prep, 2026-11-11). Every value here is made up."""
import ast
import importlib.util
import json
import math
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / 'tools/real_game/scripts'


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


class Prop:
    """An invented property of a function: a name, an offset and a size."""
    def __init__(self, name, offset, size=4, dim=1):
        self.Name, self.Offset_Internal, self.ElementSize, self.ArrayDim = name, offset, size, dim


class Func:
    """A hook's func argument: its properties at .func._properties() (as tests/den_probe_test.py's BoundFunction), parameters first,
    then locals placed after ParamsSize, as a compiled function lays them out. `bad` puts a parameter past ParamsSize."""
    def __init__(self, *names, locals=(), bad=False):
        props = [Prop(n, 4 * i) for i, n in enumerate(names)]
        size = 4 * len(names)
        props += [Prop(n, size + 4 * j) for j, n in enumerate(locals)]
        if bad and props:
            props[0].Offset_Internal = size + 64
        path = 'Pkg.Invented:' + '_'.join(names + tuple(locals)) + ('_bad' if bad else '')
        self.func = type('F', (), {'_properties': lambda _self: props, 'NumParams': len(names), 'ParamsSize': size,
                                   '_path_name': lambda _self: path})()


def no_game():
    raise RuntimeError('no game')


class RepoDir:
    """A temporary folder that looks like a repository, with RG_OUT inside its local/."""
    def __enter__(self):
        self.tmp = tempfile.TemporaryDirectory()
        repo = Path(self.tmp.name) / 'repo'
        (repo / '.git').mkdir(parents=True)
        self.out = repo / 'local' / 'realgame' / 'x'
        watch.RG_OUT = str(self.out)
        return self

    def __exit__(self, *a):
        try:
            if watch.SW.get('fh'):
                watch.SW['fh'].close()
                watch.SW['fh'] = None
        finally:
            self.tmp.cleanup()


class SpecTest(unittest.TestCase):
    def test_names(self):
        self.assertEqual(watch.sw_parse_func('Pawn.TakeDamage'), ('decl', ('Pawn', 'TakeDamage')))
        self.assertEqual(watch.sw_parse_func('Pkg.Thing:Fn'), ('path', 'Pkg.Thing:Fn'))
        for bad in ('TakeDamage', 'Pawn.Take Damage', 'a.b.c', '', 'Pkg.Thing:Fn:Two'):
            self.assertIsNone(watch.sw_parse_func(bad), bad)

    def test_bad_specs_are_refused(self):
        for bad in ({'frame': True}, {'funcs': 'Pawn.Died'}, {'funcs': ['TakeDamage']}, {'funcs': [3]}, {'frames': 1},
                    {'snap_every': -1}, {'snap_every': 0.1}, {'snap_every': math.nan}, {'pawns_every': math.inf},
                    {'snap_every': True}, {'row_cap': 0}, {'row_cap': 10 ** 9}, {'byte_cap': 10 ** 12}, {'per_func_cap': 2.5},
                    {'frame_cap': '100'}, {'flush_every': 0}, {'pawns_paths': ['ok', '']}, {'pawns_match': 'Marcus'},
                    {'funcs': ['A.B'] * 301}, {'checks': []}, []):
            with self.assertRaises(ValueError, msg=repr(bad)[:80]):
                watch.sw_load_spec(bad)

    def test_good_spec_keeps_documents_out(self):
        spec = watch.sw_load_spec({'funcs': ['Pawn.Died'], 'snap_every': 0, 'pawns_every': 0.1, 'row_cap': 7, 'notes': 'x',
                                   'checks': {'max_errors': 0}})
        self.assertNotIn('checks', spec)
        self.assertEqual((spec['row_cap'], spec['snap_every'], spec['pawns_every']), (7, 0.0, 0.1))


class RecorderTest(unittest.TestCase):
    def setUp(self):
        watch.sw_reset('t', watch.sw_load_spec({'funcs': ['Pawn.TakeDamage'], 'per_func_cap': 2, 'row_cap': 3}))
        watch.SW['on'] = True
        watch.get_pc = no_game   # the game clock reads as None

    def test_the_observer_never_blocks_and_rows_are_capped(self):
        hook = watch._sw_observer('Pkg.Pawn:TakeDamage', 'pre')
        results = [hook(None, Args(DamageAmount=12.5, EventInstigator=None), None, Func('DamageAmount', 'EventInstigator')) for _ in range(4)]
        self.assertEqual(results, [None] * 4)
        rows = [r for r in watch.SW['buf'] if r['k'] == 'row']
        self.assertEqual(len(rows), 2)   # per_func_cap 2
        self.assertEqual(rows[0]['args'], {'DamageAmount': 12.5, 'EventInstigator': None})
        self.assertEqual(watch.SW['dropped'], {'Pkg.Pawn:TakeDamage': 2})
        post = watch._sw_observer('Pkg.Pawn:Died', 'post')
        self.assertIsNone(post(None, Args(), 7, Func()))
        self.assertEqual(watch.SW['buf'][-1]['ret'], 7)
        self.assertIsNone(post(None, Args(), 7, Func()))
        self.assertEqual(watch.SW['n']['row'], 3)   # row_cap 3
        self.assertEqual(watch.SW['dropped']['row'], 1)

    def test_locals_are_never_read_only_parameters(self):
        class Args:
            def __init__(self):
                self.read = []
            def __getattr__(self, name):
                self.read.append(name)
                if name == 'OwnerLocal':
                    raise AssertionError('a local was read')
                return {'Param': 3.0, 'Other': None, 'ReturnValue': 7}.get(name)
            def __hasattr__(self, name):
                return True
        args = Args()
        row = watch.sw_add_row('Pkg.P:Fired', 'pre', None, args, None, Func('Param', 'Other', 'ReturnValue', locals=('OwnerLocal',)))
        self.assertEqual(row['args'], {'Param': 3.0, 'Other': None})   # ReturnValue is not read before the call; the local never
        self.assertNotIn('OwnerLocal', args.read)
        self.assertNotIn('ReturnValue', args.read)
        bad = Args()
        self.assertEqual(watch.sw_add_row('Pkg.P:Odd', 'pre', None, bad, None, Func('Param', locals=('OwnerLocal',), bad=True))['args'], {})
        self.assertEqual(bad.read, [])   # a layout that does not fit the rule: nothing is read at all
        self.assertEqual(watch.sw_param_names(type('NoLayout', (), {'_properties': lambda _s: [Prop('X', 0)]})()), [])

    def test_an_error_in_a_row_is_counted_not_raised(self):
        hook = watch._sw_observer('Pkg.Pawn:TakeDamage', 'pre')
        watch.SW['spec'] = None   # breaks the row
        self.assertIsNone(hook(None, Args(), None, Func()))
        self.assertEqual(len(watch.SW['errors']), 1)

    def test_off_records_nothing(self):
        watch.SW['on'] = False
        self.assertIsNone(watch._sw_observer('Pkg.Pawn:TakeDamage', 'pre')(None, Args(), None, Func()))
        self.assertEqual(watch.SW['buf'], [])

    def test_frame_rows_carry_three_deltas(self):
        first = watch.sw_frame_row(0.0167, 1.0, 10.0)
        second = watch.sw_frame_row(0.0167, 1.02, 10.0167)
        self.assertIsNone(first['dt_real'])
        self.assertAlmostEqual(second['dt_real'], 0.02)
        self.assertAlmostEqual(second['dt_game'], 0.0167)
        self.assertIsNone(watch.sw_frame_row(0.0167, 1.04, None)['dt_game'])

    def test_the_player_read_is_empty_without_a_game(self):
        self.assertEqual(watch._sw_player_here(), {'loc': None})

    def test_the_pawn_filter(self):
        self.assertTrue(watch.sw_pawn_wanted({'name': 'Lvl.TheWorld:PersistentLevel.WillowAIPawn_7'}, []))
        self.assertTrue(watch.sw_pawn_wanted({'name': 'Lvl.TheWorld:PersistentLevel.WillowAIPawn_7'}, ['WillowAIPawn_7']))
        self.assertTrue(watch.sw_pawn_wanted({'name': 'x', 'ai_class': 'CharClass_Invented'}, ['Invented']))
        self.assertFalse(watch.sw_pawn_wanted({'name': 'x', 'cls': 'y'}, ['WillowAIPawn_7']))

    def test_the_probe_holds_no_writes(self):
        """The recorder reads only: no setter, no Block, no console command, no input. A guard on the source, for the reviewer."""
        text = (SCRIPTS / 'slice_watch.py').read_text(encoding='utf-8')
        code = '\n'.join(re.sub(r'\s#\s.*$', '', line) for line in text.splitlines() if not line.lstrip().startswith('#'))
        code = re.sub(r'"""[\s\S]*?"""', '', code)
        for word in ('Block', 'ConsoleCommand', 'SetHealth', 'TakeDamage(', 'SetLocation', 'Destroy', 'StartFire', 'keybd',
                     'ServerSetSkillSaveGameData', 'AddBackpack', '.Location =', '.Rotation =', 'setattr('):
            self.assertNotIn(word, code, word)
        self.assertNotRegex(code, r'\b(?!SW\b)\w+\.\w+\s*=[^=]', 'an attribute assignment on a game object')


class BoundsTest(unittest.TestCase):
    """With every cap at its lowest, a long run keeps a bounded number of lines; byte_cap bounds the file."""
    def test_every_stream_is_capped(self):
        with RepoDir():
            spec = {'frames': True, 'snap_every': 0.5, 'pawns_every': 0.1, 'row_cap': 1, 'frame_cap': 1, 'snap_cap': 1, 'pawn_cap': 1,
                    'per_func_cap': 1, 'flush_every': 0.5}
            watch.sw_reset('t', watch.sw_load_spec(spec))
            watch._sw_open('watch')
            watch.SW['on'] = True
            watch.get_pc = lambda: type('PC', (), {'Pawn': object(), 'WorldInfo': type('W', (), {'TimeSeconds': 100.0})()})()   # settled; every read of the pawn fails
            watch._sw_sdk = type('SDK', (), {'find_all': staticmethod(lambda *a: [])})
            try:
                for i in range(100):
                    watch.SW['next_snap'] = watch.SW['next_pawns'] = 0.0   # every tick is due
                    watch._sw_tick(None, Args(DeltaTime=0.0167), None, None)
                    watch._sw_observer('Pkg.P:F', 'pre')(None, Args(), None, Func())
            finally:
                watch._sw_sdk = None
            n = watch.SW['n']
            self.assertEqual((n['row'], n['frame'], n['snap'], n['pawns']), (1, 1, 1, 1))
            self.assertGreaterEqual(watch.SW['auto_failures'], 1)   # the failing automatic reads are counted, not dropped silently
            self.assertEqual(watch.SW['errors'], [])
            watch.sw_flush()
            size = watch.SW['path'].stat().st_size
            watch.SW['spec']['byte_cap'] = size + 50   # the file stops growing at its cap
            watch.SW['buf'].append({'k': 'row', 'pad': 'x' * 200})
            watch.sw_flush()
            self.assertEqual(watch.SW['dropped']['bytes'], 1)
            self.assertEqual(watch.SW['path'].stat().st_size, size)
            watch.sw_stop()


class StreamTest(unittest.TestCase):
    def test_the_file_grows_as_it_goes_and_ends_with_an_end_line(self):
        with RepoDir() as d:
            watch.sw_reset('t', watch.sw_load_spec({'frames': True, 'flush_every': 0.5}))
            watch._sw_open('watch')
            watch.SW['on'] = True
            watch.get_pc = no_game
            watch.SW['next_flush'] = 0.0
            watch._sw_tick(None, Args(DeltaTime=0.0167), None, None)   # this frame flushes
            watch._sw_tick(None, Args(DeltaTime=0.0167), None, None)
            path = d.out / 'watch.jsonl'
            self.assertTrue(path.exists())
            crashed = checker.load_recording(path)   # what a crash would leave: no end line
            self.assertIsNone(crashed['end'])
            self.assertFalse(dict((n, ok) for n, ok, _ in checker.check(crashed, {}))['ended'])
            counts = watch.sw_dump('watch')
            done = checker.load_recording(path)
            self.assertIsNotNone(done['end'])
            self.assertEqual(len(done['frames']), counts['lines']['frame'])
            self.assertEqual(len(done['frames']), 2)
            self.assertIn('busy', done['frames'][0])   # the frame in which the flush ran is marked
            watch.sw_reset('t', watch.sw_load_spec({}))
            watch._sw_open('watch')   # an earlier file is kept under a new name
            self.assertEqual(len(list(d.out.glob('watch.*.jsonl'))), 1)
            watch.sw_stop()
            self.assertTrue(checker.load_recording(path)['end']['stopped'])

    def test_dump_names_and_folder(self):
        with self.assertRaises(ValueError):
            watch.sw_start('t', {}, name='../escape')
        with tempfile.TemporaryDirectory() as tmp:
            watch.RG_OUT = str(Path(tmp) / 'realgame' / 'x')   # no repository's local/ above it
            with self.assertRaises(ValueError):
                watch.sw_root()


class FakeObject:
    """An invented game object: attributes as given, a path name, a class."""
    def __init__(self, path, cls='Thing', **fields):
        self._path = path
        self.Name = path.rsplit('.', 1)[-1].rsplit(':', 1)[-1]
        self.bDeleteMe = False
        self.Class = type('C', (), {'Name': cls})()
        self.__dict__.update(fields)

    def _path_name(self):
        return self._path


class FakeGameTest(unittest.TestCase):
    """sw_start to sw_stop against a stand-in SDK: the hooks it adds, a recorded call, a frame, a snapshot, a pawn read and the file."""
    def setUp(self):
        self.hooks = {}
        fn = lambda name, outer: type('F', (), {'Name': name, 'Outer': outer, 'Class': type('K', (), {'Name': 'Function'})(),
                                               '_path_name': lambda _s: f'Pkg.{outer.Name}:{name}'})()
        cls = type('Cls', (), {'Name': 'InventedPawn'})()
        state = type('S', (), {'Name': 'Down', 'Class': type('K', (), {'Name': 'State'})(),
                                '_fields': lambda _s: [type('F', (), {'Name': 'Hit', 'Class': type('K', (), {'Name': 'Function'})(),
                                                                      '_path_name': lambda _t: 'Pkg.InventedPawn.Down:Hit'})()]})()
        cls._fields = lambda: [fn('Hit', cls), state]
        loc = type('V', (), {'X': 1.0, 'Y': 2.0, 'Z': 3.0})()
        pawn = FakeObject('Lvl.PersistentLevel.Player_0', 'InventedPlayer', Location=loc, bHidden=False, GetHealth=lambda: 50.0,
                          CylinderComponent=type('Cy', (), {'CollisionRadius': 1.5, 'CollisionHeight': 2.5})())
        enemy = FakeObject('Lvl.PersistentLevel.WillowAIPawn_1', 'WillowAIPawn', Location=loc, GroundSpeed=4.0, AIClass=None,
                           CylinderComponent=type('Cy', (), {'CollisionRadius': 1.0, 'CollisionHeight': 2.0})())
        world = type('W', (), {'TimeSeconds': 30.0, 'GRI': FakeObject('Lvl.GRI', ActiveRespawnCheckpointTeleportActor=None),
                               'GetStreamingPersistentMapName': lambda _s=None: 'Invented_P'})()
        pc = FakeObject('Lvl.PC', 'InventedPC', Pawn=pawn, WorldInfo=world,
                        PlayerReplicationInfo=FakeObject('Lvl.PRI', GetCurrencyOnHand=lambda _k: 7, ExpLevel=3))
        objects = {'WillowAIPawn': [enemy, FakeObject('Pkg.Default__WillowAIPawn', 'WillowAIPawn')],
                   'ResourcePool': [FakeObject('Lvl.Pool_0', 'ResourcePool', CurrentValue=1.0, AssociatedProvider=enemy)],
                   'TravelStation': [FakeObject('Lvl.Station_0', 'TravelStation', bShouldBeActive=True)], 'GameEngine': [], 'WillowPickup': []}
        functions = {'Pkg.Other:Call'}
        self.objects, self.world, self.enemy = objects, world, enemy
        self.scans = []
        hooks = self.hooks

        class Type:
            PRE, POST, POST_UNCONDITIONAL = 'pre', 'post', 'post_unconditional'
        watch._sw_sdk = type('SDK', (), {'find_class': staticmethod(lambda name: cls),
                                         'find_all': staticmethod(lambda name, exact=True: self.scan(name, exact)),
                                         'find_object': staticmethod(lambda kind, path: (enemy if path == enemy._path else None) if kind == 'WillowAIPawn'
                                                                     else (object() if path in functions else None))})
        watch._SwType = Type
        watch._sw_add_hook = lambda path, kind, hid, f: hooks.__setitem__((path, kind, hid), f)
        watch._sw_remove_hook = lambda path, kind, hid: hooks.pop((path, kind, hid), None)
        watch._sw_has_hook = lambda path, kind, hid: (path, kind, hid) in hooks
        watch.get_pc = lambda: pc

    def scan(self, name, exact=True):
        """find_all as the SDK does it: exact=True returns objects of that class only; exact=False adds its subclasses (here: objects
        whose class carries base=<name>)."""
        self.scans.append((name, exact))
        found = self.objects.get(name, [])
        if isinstance(found, Exception):
            raise found
        return [o for o in found if o.Class.Name == name or (not exact and getattr(o.Class, 'base', None) == name)]

    def tearDown(self):
        watch._sw_sdk = None

    def test_a_whole_recording(self):
        """Restored after the review of 465232c: start to stop, every hook gone, declarations in states resolved, missing names
        listed, the read-by-path branch session 1 uses."""
        with RepoDir() as d:
            spec = {'funcs': ['InventedPawn.Hit', 'InventedPawn.Missing', 'Pkg.Other:Call', 'Pkg.Nowhere:Call'], 'frames': True,
                    'frame_player': True, 'pickups': True, 'pawns_paths': ['Lvl.PersistentLevel.WillowAIPawn_1', 'Lvl.Gone_2']}
            started = watch.sw_start('t', spec)
            self.assertEqual(started['resolved'], 3)   # Hit, Down:Hit (a state's declaration) and Other:Call, each hooked PRE and POST
            self.assertEqual(started['missing'], ['InventedPawn.Missing', 'Pkg.Nowhere:Call'])   # a full path the game lacks is missing
            self.assertEqual(started['start_failed'], ['mods'])   # no mods_base outside the game: named, not hidden
            self.assertIn(('Pkg.InventedPawn.Down:Hit', 'pre', 'ow_slice_watch_pre'), self.hooks)
            self.assertIn(('Pkg.InventedPawn.Down:Hit', 'post', 'ow_slice_watch_post'), self.hooks)
            tick = self.hooks[(watch.SW_TICK, 'post_unconditional', 'ow_slice_watch_tick')]
            self.assertIsNone(tick(None, Args(DeltaTime=0.0167), None, None))
            pre = self.hooks[('Pkg.InventedPawn:Hit', 'pre', 'ow_slice_watch_pre')]
            self.assertIsNone(pre(None, Args(Amount=3.0), None, Func('Amount')))
            post = self.hooks[('Pkg.InventedPawn:Hit', 'post', 'ow_slice_watch_post')]
            self.assertIsNone(post(None, Args(Amount=3.0), 5, Func('Amount')))
            snap = watch.sw_snap('before_down')
            self.assertEqual(snap['cash'], 7)
            self.assertEqual(snap['player']['cylinder'], {'radius': 1.5, 'height': 2.5})
            self.assertEqual(snap['player']['GetHealth'], 50.0)   # the player's own pawn: one of the listed native calls
            self.assertEqual(len(snap['stations']), 1)
            self.assertEqual(snap['pickups'], [])
            self.assertEqual(snap['failed'], ['mods'])   # switched off after the start snapshot's failure, and named
            pawns = watch.sw_pawns('arrival')
            self.assertEqual((pawns['pawns'], pawns['failed']), (1, ['Lvl.Gone_2']))   # looked up by path; a missing one is named
            watch.sw_dump('watch')
            data = checker.load_recording(d.out / 'watch.jsonl')
            self.assertEqual(data['rows'][0]['args'], {'Amount': 3.0})
            self.assertEqual(data['rows'][1]['ret'], 5)
            self.assertEqual(data['rows'][0]['dt'], 0.0167)
            self.assertEqual(data['frames'][0]['loc'], [1.0, 2.0, 3.0])
            self.assertEqual([s['label'] for s in data['snaps']], ['start', 'before_down'])
            self.assertEqual(data['missing'], ['InventedPawn.Missing', 'Pkg.Nowhere:Call'])
            arrival = next(p for p in data['pawns'] if p['label'] == 'arrival')
            self.assertEqual(arrival['map'], 'Invented_P')   # a labelled pawn read carries its map
            self.assertEqual(watch.sw_stop(), 'stopped')
            self.assertEqual(self.hooks, {})   # every hook it added is gone

    def test_stale_pool_references_are_flagged(self):
        real = watch._SwUObject
        watch._SwUObject = FakeObject   # stand-in objects are recorded by path name, as the SDK's are
        self.addCleanup(setattr, watch, '_SwUObject', real)
        manager = FakeObject('Lvl.Manager_0', 'ResourcePoolManager')
        good_pool = FakeObject('Lvl.Pool_7', 'ResourcePool', PoolGUID=4, Definition=object(), CurrentValue=10.0, MaxValue=20.0)
        manager.ResourcePools = [None] * 3 + [good_pool] + [None] * 12
        ref = lambda pool, guid=4, index=3: type('Ref', (), {'PoolManager': manager, 'PoolIndexInManager': index, 'PoolGUID': guid, 'Data': pool})()
        self.assertIsNone(watch._sw_pool_of(ref(good_pool))['stale'])
        self.assertEqual(watch._sw_pool_of(ref(None))['stale'], 'no pool')
        self.assertEqual(watch._sw_pool_of(ref(good_pool, guid=5))['stale'], 'guid differs')
        shut = FakeObject('Lvl.Pool_8', 'ResourcePool', PoolGUID=4, Definition=None, CurrentValue=0.0)
        manager.ResourcePools[4] = shut
        self.assertEqual(watch._sw_pool_of(ref(shut, index=4))['stale'], 'shut down')
        other = FakeObject('Lvl.Pool_9', 'ResourcePool', PoolGUID=4, Definition=object())
        self.assertEqual(watch._sw_pool_of(ref(other, index=3))['stale'], 'not in its slot')
        row = watch._sw_pool_of(ref(good_pool))
        self.assertEqual((row['ref_index'], row['ref_guid'], row['PoolGUID'], row['CurrentValue'], row['manager']), (3, 4, 4, 10.0, 'Lvl.Manager_0'))

    def test_nothing_is_scanned_before_the_map_settles_and_labelled_reads_wait(self):
        with RepoDir():
            self.world.TimeSeconds = 2.0   # just after a map load
            started = watch.sw_start('t', {'snap_every': 0.5, 'pawns_every': 0.1})
            self.assertTrue(watch.SW['deferred'])   # the start snapshot waits
            self.assertEqual(watch.sw_pawns('arrival')['deferred'], True)
            tick = self.hooks[(watch.SW_TICK, 'post_unconditional', 'ow_slice_watch_tick')]
            for _ in range(5):
                tick(None, Args(DeltaTime=0.0167), None, None)
            self.assertEqual(self.scans, [])   # no scan of any kind before the map settled
            self.world.TimeSeconds = 20.0
            tick(None, Args(DeltaTime=0.0167), None, None)
            self.assertIn(('WillowAIPawn', True), self.scans)
            self.assertEqual(watch.SW['deferred'], [])
            watch.sw_dump('watch')
            data = checker.load_recording(watch.SW['path'])
            start = next(x for x in data['snaps'] if x['label'] == 'start')
            self.assertIn('asked_t', start)   # a deferred snapshot says when it was asked for
            self.assertTrue(any(p['label'] == 'arrival' for p in data['pawns']))
            self.assertEqual(started['resolved'], 0)

    def test_a_scan_that_fails_after_travel_is_switched_off_not_retried(self):
        with RepoDir():
            self.objects['WillowAIPawn'] = RuntimeError('invented: objects pending destruction')
            watch.sw_start('t', {'pawns_every': 0.1})
            tick = self.hooks[(watch.SW_TICK, 'post_unconditional', 'ow_slice_watch_tick')]
            for _ in range(20):
                watch.SW['next_pawns'] = 0.0
                tick(None, Args(DeltaTime=0.0167), None, None)
            self.assertEqual(self.scans.count(('WillowAIPawn', True)), 1)   # tried once (the exact class), then off
            self.assertIn('pawns', watch.SW['off'])
            self.assertEqual(watch.sw_pawns('later')['failed'], ['off'])
            self.objects['ResourcePool'] = RuntimeError('invented')
            snap = watch.sw_snap('before_down')
            self.assertIn('pools', snap['failed'])
            self.assertIn('pools', watch.sw_snap('after')['off'])
            counts = watch.sw_dump('watch')
            self.assertEqual(sorted(counts['switched_off']), ['mods', 'pawns', 'pools'])   # no mods_base outside the game: off too

    def test_nothing_is_called_on_any_scanned_object(self):
        called = []
        def getter(name):
            def get():
                called.append(name)
                return 1.0
            return get
        here = self.enemy.Location
        pool = FakeObject('Lvl.HealthPool_3', 'ResourcePool', CurrentValue=40.0, MaxValue=50.0, GetMaxValue=getter('pool'))
        shield = FakeObject('Lvl.ShieldPool_3', 'ResourcePool', CurrentValue=9.0, MaxValue=10.0)
        ref = lambda p: type('Ref', (), {'Data': p})()
        template = FakeObject('GD_Invented.Population:Pawn_Invented', 'WillowAIPawn', GetHealth=getter('template'), Location=here)
        dying = FakeObject('Lvl.PersistentLevel.WillowAIPawn_9', 'WillowAIPawn', GetHealth=getter('dying'), Location=here)
        dying.bDeleteMe = True
        live = FakeObject('Lvl.PersistentLevel.WillowAIPawn_3', 'WillowAIPawn', GetHealth=getter('live'), GetShieldStrength=getter('live shield'),
                          Location=here, Health=40, HealthPool=ref(pool), ShieldArmor=ref(shield), AIClass=getter('ai class'))
        subclass = FakeObject('Lvl.PersistentLevel.WillowAIPawn_5', 'WillowVehiclePawn', GetHealth=getter('subclass'), Location=here)
        subclass.Class.base = 'WillowAIPawn'
        far = FakeObject('Lvl.PersistentLevel.WillowAIPawn_7', 'WillowAIPawn', GetHealth=getter('far'),
                         Location=type('V', (), {'X': 99999.0, 'Y': 2.0, 'Z': 3.0})())
        self.objects['WillowAIPawn'] = [template, dying, live, subclass, far]
        self.objects['ResourcePool'] = [FakeObject('Lvl.Pool_1', 'ResourcePool', CurrentValue=2.0, AssociatedProvider=live, GetMaxValue=getter('pool'))]
        with RepoDir() as d:
            watch.sw_start('t', {})
            self.assertEqual(watch.sw_pawns('arrival')['pawns'], 1)   # only the live, exact-class pawn within the radius
            snap = watch.sw_snap('x')
            self.assertEqual(called, [])   # no getter, on the live pawn, a template, a dying pawn, a subclass, a far pawn or a pool
            self.assertEqual(snap['pools'][0]['CurrentValue'], 2.0)
            watch.sw_dump('watch')
            line = next(p for p in checker.load_recording(d.out / 'watch.jsonl')['pawns'] if p['label'] == 'arrival')
            row = line['pawns'][0]
            self.assertEqual((row['health_pool']['CurrentValue'], row['shield_pool']['MaxValue'], row['Health']), (40.0, 10.0, 40))
            self.assertIn('dist', row)
            self.assertEqual(line['beyond_radius'], 1)   # the far one is counted, not read
            self.assertIsNone(row.get('ai_class'))   # a callable is never called, even where a field was expected

    def test_a_second_travel_stops_the_scans_and_early_labels_are_not_taken(self):
        with RepoDir() as d:
            watch.sw_start('t', {'snap_every': 0.5, 'pawns_every': 0.1})
            tick = self.hooks[(watch.SW_TICK, 'post_unconditional', 'ow_slice_watch_tick')]
            tick(None, Args(DeltaTime=0.0167), None, None)
            before = len(self.scans)
            self.world.TimeSeconds = 2.0   # a second map load: its clock starts again
            for _ in range(10):
                watch.SW['next_snap'] = watch.SW['next_pawns'] = 0.0
                tick(None, Args(DeltaTime=0.0167), None, None)
            self.assertEqual(len(self.scans), before)   # nothing scanned on the new map before it settled
            self.assertTrue(watch.sw_snap('before_down')['not_taken'])   # never taken late
            self.assertTrue(watch.sw_pawns('before_down')['not_taken'])
            self.world.TimeSeconds = 30.0
            watch.SW['next_pawns'] = 0.0
            tick(None, Args(DeltaTime=0.0167), None, None)
            self.assertGreater(len(self.scans), before)   # the settled map is scanned again
            watch.sw_dump('watch')
            data = checker.load_recording(d.out / 'watch.jsonl')
            early = [x for x in data['snaps'] + data['pawns'] if x.get('label') == 'before_down']
            self.assertEqual(len(early), 2)
            self.assertTrue(all(x['not_taken'] for x in early))
            got = {n: ok for n, ok, _ in checker.check(data, {})}
            self.assertFalse(got['not_taken'])   # unexpected: the check fails
            self.assertTrue({n: ok for n, ok, _ in checker.check(data, {'expect_not_taken': ['before_down']})}['not_taken'])

def dump(rows=(), frames=(), snaps=(), pawns=(), missing=(), errors=(), ended=True, auto_failures=0):
    return {'rows': list(rows), 'frames': list(frames), 'snaps': list(snaps), 'pawns': list(pawns), 'missing': list(missing),
            'end': {'errors': list(errors), 'auto_failures': auto_failures, 'stopped': False} if ended else None}


def row(func, g, tag='pre', dt=0.0167, **args):
    return {'func': func, 'g': g, 'tag': tag, 'dt': dt, 'args': args}


GOOD_SNAP = {'label': 'start', 'failed': [], 'player': {'Location': {}}, 'pools': [{'pool': 'p'}], 'cash': 7, 'checkpoint': None,
             'stations': [{'name': 's'}], 'mods': [{'name': 'Better Thing', 'enabled': False}]}
CONTENT = {'player': 'dict', 'pools': 'nonempty', 'cash': 'number', 'checkpoint': 'present', 'stations': 'nonempty', 'mods': 'nonempty'}


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

    def test_content_fails_on_each_kind_of_failed_read(self):
        checks = {'snap_content': CONTENT, 'snaps': ['start']}
        self.assertTrue(self.results(dump(snaps=[GOOD_SNAP]), checks)['snap_content'])
        for broken in (dict(GOOD_SNAP, failed=['cash']), dict(GOOD_SNAP, player={}), dict(GOOD_SNAP, pools=[]), dict(GOOD_SNAP, cash='err'),
                       dict(GOOD_SNAP, stations=[]), {k: v for k, v in GOOD_SNAP.items() if k != 'checkpoint'},
                       dict(GOOD_SNAP, failed=['checkpoint']), dict(GOOD_SNAP, mods=[]), dict(GOOD_SNAP, cash=True)):
            self.assertFalse(self.results(dump(snaps=[broken]), checks)['snap_content'], broken)
        auto_short = {'label': 'auto', 'failed': ['player']}   # automatic snapshots are counted by max_auto_failures, not here
        self.assertTrue(self.results(dump(snaps=[GOOD_SNAP, auto_short]), checks)['snap_content'])
        self.assertFalse(self.results(dump(snaps=[]), checks)['snap_content'])
        self.assertFalse(self.results(dump(auto_failures=3), {'max_auto_failures': 2})['max_auto_failures'])
        self.assertTrue(self.results(dump(auto_failures=2), {'max_auto_failures': 2})['max_auto_failures'])

    def test_named_mods_must_be_off(self):
        checks = {'mods_must_be_off': ['better thing']}
        self.assertTrue(self.results(dump(snaps=[GOOD_SNAP]), checks)['mods_must_be_off'])
        on = dict(GOOD_SNAP, mods=[{'name': 'Better Thing', 'enabled': True}])
        self.assertFalse(self.results(dump(snaps=[on]), checks)['mods_must_be_off'])
        self.assertFalse(self.results(dump(snaps=[dict(GOOD_SNAP, failed=['mods'])]), checks)['mods_must_be_off'])
        self.assertFalse(self.results(dump(), checks)['mods_must_be_off'])   # no start snapshot: not shown off

    def test_frames_leave_out_busy_ones_and_pawn_reads_must_be_clean(self):
        frames = [{'dt_arg': 0.0167}] * 95 + [{'dt_arg': 0.5, 'busy': [['snap', 40.0]]}] * 5
        checks = {'frames': {'min': 50, 'dt_min': 0.005, 'dt_max': 0.1, 'share': 1.0}}
        self.assertTrue(self.results(dump(frames=frames), checks)['frames'])
        self.assertFalse(self.results(dump(frames=frames + [{'dt_arg': 0.5}]), checks)['frames'])
        self.assertFalse(self.results(dump(frames=frames[:40]), checks)['frames'])
        p = {'pawns': {'min_reads': 1, 'fields': ['cylinder', 'GroundSpeed']}}
        ok = {'failed': [], 'pawns': [{'cylinder': {'radius': 1.0}, 'GroundSpeed': 2.0}]}
        self.assertTrue(self.results(dump(pawns=[ok]), p)['pawns'])
        self.assertFalse(self.results(dump(pawns=[dict(ok, failed=['read'])]), p)['pawns'])
        self.assertFalse(self.results(dump(pawns=[{'failed': [], 'pawns': [{'cylinder': None, 'GroundSpeed': 2.0}]}]), p)['pawns'])

    def test_anything_a_cap_dropped_fails_unless_allowed(self):
        capped = dump()
        capped['end']['dropped'] = {'frame': 3}
        self.assertFalse(self.results(capped, {})['max_dropped'])
        self.assertTrue(self.results(capped, {'max_dropped': {'frame': 3}})['max_dropped'])
        per_func = dump()
        per_func['end']['dropped'] = {'Pkg.P:F': 10, 'Pkg.P:G': 5}
        self.assertFalse(self.results(per_func, {'max_dropped': {'per_func': 14}})['max_dropped'])
        self.assertTrue(self.results(per_func, {'max_dropped': {'per_func': 15}})['max_dropped'])
        self.assertTrue(self.results(dump(), {})['max_dropped'])
        self.assertFalse(self.results(dump(ended=False), {})['max_dropped'])

    def test_expected_not_taken_alternatives_and_the_defer_bound(self):
        nt = {'label': 'smoke_unsettled', 'not_taken': 'asked before the map settled', 'asked_t': 1.0}
        both = dump(snaps=[nt, dict(GOOD_SNAP, label='smoke_unsettled')])
        self.assertFalse(self.results(both, {'expect_not_taken': ['smoke_unsettled']})['not_taken'])   # one instance was taken
        alt = {'snaps': [['down', 'down_seen_no_health', 'down_seen_no_pawn']], 'allow_not_taken': ['down_seen_no_pawn']}
        self.assertTrue(self.results(dump(snaps=[dict(GOOD_SNAP, label='down_seen_no_health')]), alt)['snap down/down_seen_no_health/down_seen_no_pawn'])
        no_pawn = dump(snaps=[{'label': 'down_seen_no_pawn', 'not_taken': 'asked before the map settled', 'asked_t': 1.0}])
        got = self.results(no_pawn, alt)
        self.assertTrue(got['snap down/down_seen_no_health/down_seen_no_pawn'])
        self.assertTrue(got['not_taken'])   # allowed not taken
        self.assertFalse(self.results(dump(snaps=[dict(GOOD_SNAP, label='other')]), alt)['snap down/down_seen_no_health/down_seen_no_pawn'])
        slow = dump(snaps=[dict(GOOD_SNAP, asked_t=1.0, t=100.0)])
        self.assertFalse(self.results(slow, {})['defer_bound'])
        self.assertTrue(self.results(slow, {'max_defer_seconds': 120})['defer_bound'])

    def test_stale_health_pools_do_not_count(self):
        p = {'pawns': {'min_reads': 1, 'min_pawns': 1, 'fields': ['cylinder', 'health_pool']}}
        ok = {'failed': [], 'pawns': [{'cylinder': {'radius': 1.0}, 'health_pool': {'stale': None}}]}
        self.assertTrue(self.results(dump(pawns=[ok]), p)['pawns'])
        stale = {'failed': [], 'pawns': [{'cylinder': {'radius': 1.0}, 'health_pool': {'stale': 'guid differs'}}]}
        self.assertFalse(self.results(dump(pawns=[stale]), p)['pawns'])
        missing = {'failed': [], 'pawns': [{'cylinder': {'radius': 1.0}, 'health_pool': None}]}
        self.assertFalse(self.results(dump(pawns=[missing]), p)['pawns'])
        mixed = {'failed': [], 'pawns': stale['pawns'] + ok['pawns']}   # one dying pawn does not spoil a read with a good one
        self.assertTrue(self.results(dump(pawns=[mixed]), p)['pawns'])

    def test_late_switched_off_and_never_taken_fail(self):
        late = dump(snaps=[dict(GOOD_SNAP, label='before_down', asked_t=3.0)])
        self.assertFalse(self.results(late, {})['taken_late'])
        self.assertTrue(self.results(dump(snaps=[dict(GOOD_SNAP, asked_t=3.0)]), {})['taken_late'])   # 'start' may be deferred
        off = dump()
        off['end']['switched_off'] = {'pools': 'RuntimeError()'}
        self.assertFalse(self.results(off, {})['switched_off'])
        self.assertTrue(self.results(off, {'allow_switched_off': ['pools']})['switched_off'])
        never = dump()
        never['end']['never_taken'] = [['snap', 'start', 1.0]]
        self.assertFalse(self.results(never, {})['never_taken'])
        expected = dump(snaps=[{'label': 'smoke_unsettled', 'not_taken': 'asked before the map settled', 'asked_t': 1.0}])
        self.assertTrue(self.results(expected, {'expect_not_taken': ['smoke_unsettled']})['not_taken'])
        self.assertFalse(self.results(dump(), {'expect_not_taken': ['smoke_unsettled']})['not_taken'])   # expected, never recorded

    def test_errors_missing_and_the_end(self):
        self.assertFalse(self.results(dump(errors=['e']), {})['max_errors'])
        self.assertTrue(self.results(dump(errors=['e']), {'max_errors': 1})['max_errors'])
        self.assertFalse(self.results(dump(missing=['A.B']), {'no_missing': True})['no_missing'])
        self.assertFalse(self.results(dump([row('A', 1.0, dt=None)]), {'rows_have_dt': True})['rows_have_dt'])
        self.assertFalse(self.results(dump(ended=False), {})['ended'])
        self.assertFalse(self.results(dump(ended=False), {})['max_errors'])

    def write_recording(self, folder, records):
        path = Path(folder) / 'watch.jsonl'
        path.write_text(''.join(json.dumps(r) + '\n' for r in records) + '{"k": "row", "cut sh', encoding='utf-8')   # a crash-cut last line
        return path

    def test_the_command_line(self):
        tool = str(ROOT / 'tools/real_game/slice_watch_check.py')
        with tempfile.TemporaryDirectory() as tmp:
            d = self.write_recording(tmp, [{'k': 'meta', 'missing': []}, dict(row('X.Pawn:Died', 1.0, DamageAmount=12345.5), k='row'),
                                           {'k': 'end', 'errors': [], 'auto_failures': 0, 'stopped': False}])
            s = Path(tmp) / 'spec.json'
            s.write_text(json.dumps({'funcs': ['Pawn.Died'], 'checks': {'require': [{'func': 'Died', 'min': 1}]}}), encoding='utf-8')
            ok = subprocess.run([sys.executable, tool, str(d), '--spec', str(s)], capture_output=True, text=True)
            self.assertEqual(ok.returncode, 0, ok.stdout)
            self.assertIn('USABLE', ok.stdout)
            self.assertNotIn('12345', ok.stdout)   # no recorded value is printed
            s.write_text(json.dumps({'checks': {'require': [{'func': 'Missing', 'min': 1}]}}), encoding='utf-8')
            bad = subprocess.run([sys.executable, tool, str(d), '--spec', str(s)], capture_output=True, text=True)
            self.assertEqual(bad.returncode, 1)
            self.assertIn('NOT USABLE', bad.stdout)
            gone = subprocess.run([sys.executable, tool, str(Path(tmp) / 'none.jsonl'), '--spec', str(s)], capture_output=True, text=True)
            self.assertEqual(gone.returncode, 2)
            for refused in ({'snap_every': -1}, {'checks': {'snap_feilds': []}}, {'checks': {'snap_content': {'cash': 'anything'}}}):
                s.write_text(json.dumps(refused), encoding='utf-8')
                v = subprocess.run([sys.executable, tool, '--validate-spec', str(s)], capture_output=True, text=True)
                self.assertEqual(v.returncode, 2, refused)
            s.write_text(json.dumps({'funcs': ['Pawn.Died'], 'checks': {'snap_content': {'cash': 'number'}}}), encoding='utf-8')
            self.assertEqual(subprocess.run([sys.executable, tool, '--validate-spec', str(s)], capture_output=True).returncode, 0)


# ---------------------------------------------------------------- the shared namespace
DRIVER_SCOPE = {'unrealsdk', 'find', 'find_all', 'get_pc', 'plain', 'mark', 'Type', 'add_hook', 'block_saves', 'RG_OUT'}
# Each capture's command scripts, in the order it loads them (Invoke-GamePyFile), then the watch.
CAPTURE_SCRIPTS = {
    'mission_screen_capture.ps1': ['fire_mission_probe', 'mission_screen_probe'],
    'cash_capture.ps1': ['combat_probe', 'cash_probe', 'den_probe'],
    'phaselock_integrity_capture.ps1': ['phaselock', 'phaselock_matched', 'phaselock_integrity', 'action_skill_hud'],
    'audio_events_capture.ps1': ['phaselock', 'phaselock_matched', 'phaselock_integrity', 'audio_events'],
}
# Already there before the watch: phaselock.py and action_skill_hud.py both define these two helpers, and phaselock_integrity_capture loads
# both (the later one wins). Reported to the hub on 2026-11-11; not the watch's, and left as reviewed.
KNOWN_COLLISIONS = {('phaselock_integrity_capture.ps1', '_sample'), ('phaselock_integrity_capture.ps1', '_on_start_skill')}


def top_level(stem):
    """Every name a script binds at its top level: (name, how), how being 'def', 'assign' or 'import'."""
    tree = ast.parse((SCRIPTS / f'{stem}.py').read_text(encoding='utf-8'))
    out = {}

    def walk(nodes):
        for node in nodes:
            if isinstance(node, (ast.FunctionDef, ast.ClassDef)):
                out[node.name] = 'def'
            elif isinstance(node, (ast.Assign, ast.AnnAssign, ast.AugAssign)):
                targets = node.targets if isinstance(node, ast.Assign) else [node.target]
                for t in targets:
                    for n in ast.walk(t):
                        if isinstance(n, ast.Name):
                            out.setdefault(n.id, 'assign')
            elif isinstance(node, (ast.Import, ast.ImportFrom)):
                for a in node.names:
                    out.setdefault((a.asname or a.name).split('.')[0], 'import')
            elif isinstance(node, (ast.Try, ast.If)):
                walk(node.body)
                for h in getattr(node, 'handlers', []):
                    walk(h.body)
                walk(node.orelse)
    walk(tree.body)
    return out


class NamespaceTest(unittest.TestCase):
    def test_every_watch_name_is_its_own(self):
        names = top_level('slice_watch')
        self.assertTrue(names)
        for name in names:
            self.assertRegex(name, r'^(sw_|_sw_|SW|_Sw)', name)

    def test_no_script_replaces_another_in_each_capture(self):
        for capture, stems in CAPTURE_SCRIPTS.items():
            text = (ROOT / 'tools/real_game' / capture).read_text(encoding='utf-8')
            for stem in stems:
                self.assertIn(f"'{stem}.py'", text, f'{capture} no longer loads {stem}.py: update this list')
            owner = {n: 'driver' for n in DRIVER_SCOPE}
            for stem in stems + ['slice_watch']:
                for name, how in top_level(stem).items():
                    if name in owner and owner[name] != stem and how != 'import' and (capture, name) not in KNOWN_COLLISIONS:
                        self.fail(f'{capture}: {stem}.py redefines {name} from {owner[name]}')
                    owner.setdefault(name, stem)


# ---------------------------------------------------------------- the helpers and the captures
def ps(body, env=None):
    shell = 'pwsh' if subprocess.run(['where', 'pwsh'], capture_output=True, shell=True).returncode == 0 else 'powershell'
    script = "$ErrorActionPreference = 'Stop'\n. '" + str(ROOT / 'tools/real_game/slice_watch.ps1') + "'\n" + body
    return subprocess.run([shell, '-NoProfile', '-NonInteractive', '-Command', script], capture_output=True, text=True, timeout=120, env=env)


@unittest.skipUnless(sys.platform == 'win32', 'the capture helpers run on Windows')
class HelperTest(unittest.TestCase):
    def test_complete_checks_the_recording(self):
        with tempfile.TemporaryDirectory() as tmp:
            spec = Path(tmp) / 'spec.json'
            spec.write_text(json.dumps({'funcs': ['Pawn.Died'], 'checks': {'require': [{'func': 'Died', 'min': 1}]}}), encoding='utf-8')
            rec = Path(tmp) / 'watch.jsonl'
            rec.write_text(json.dumps(dict(row('X.Pawn:Died', 1.0), k='row')) + '\n' + json.dumps({'k': 'end', 'errors': []}) + '\n', encoding='utf-8')
            body = ("function Invoke-GamePy([string]$Code, [int]$T = 30) { $script:sent += @($Code); if ($Code -like '*sw_dump*') { '{\"lines\": {}}' } elseif ($Code -like '*sw_stop*') { 'stopped' } else { '3' } }\n"
                    "$script:SliceWatchOn = $true\n"
                    f"$w = Complete-SliceWatch '{spec}' '{tmp}'\n"
                    "'usable=' + $w.usable; 'sent=' + $script:sent.Count; $w.check")
            done = ps(body)
            self.assertIn('usable=True', done.stdout, done.stdout + done.stderr)
            self.assertIn('sent=3', done.stdout)   # end snapshot, end of recording, stop
            rec.write_text(json.dumps(dict(row('X.Pawn:Died', 1.0), k='row')) + '\n', encoding='utf-8')   # no end line
            self.assertIn('usable=False', ps(body).stdout)

    def test_a_watch_that_does_not_start_never_ends_the_capture(self):
        body = ("function Invoke-GamePy([string]$Code, [int]$T = 30) { $script:sent += @($Code); if ($Code -like '*sw_stop*') { 'stopped' } else { throw 'no reply to c1' } }\n"
                "function Invoke-GamePyFile([string]$P, [string]$O) { 'loaded' }\n"
                f"$spec = '{ROOT / 'tests' / 'slice_watch_test.py'}'\n"
                "try { Save-SliceWatchSnap 'a b' } catch { 'refused name' }\n"
                "$r = Start-SliceWatch $spec 'x' 'x' 3>$null; if ($r.error) { 'not started' }; 'stop after error: ' + $r.stop_after_error\n"
                "Save-SliceWatchSnap 'ok'; 'a snapshot while off does nothing'\n"
                "try { Complete-SliceWatch $spec 'C:\\' } catch { 'complete refused: ' + (\"$_\" -like '*did not start*') }\n"
                "$script:SliceWatchOn = $true; Save-SliceWatchSnap 'ok' 3>$null; 'a failed snapshot is only a warning'")
        done = ps(body)
        for word in ('refused name', 'not started', 'stop after error: stopped', 'a snapshot while off does nothing',
                     'complete refused: True', 'a failed snapshot is only a warning'):
            self.assertIn(word, done.stdout, done.stdout + done.stderr)

    def test_a_snapshot_says_what_happened(self):
        body = ("$script:SliceWatchOn = $true\n"
                "$script:reply = '{\"deferred\": false, \"not_taken\": true, \"failed\": []}'\n"
                "function Invoke-GamePy([string]$Code, [int]$T = 30) { $script:reply }\n"
                "Save-SliceWatchSnap 'before_down'\n"
                "$script:reply = '{\"deferred\": true, \"not_taken\": false, \"failed\": []}'; Save-SliceWatchSnap 'start'\n"
                "$script:reply = '{\"deferred\": false, \"not_taken\": false, \"failed\": [\"pools\"]}'; Save-SliceWatchSnap 'x'\n"
                "$script:reply = '{\"deferred\": false, \"not_taken\": false, \"failed\": []}'; Save-SliceWatchSnap 'y'")
        out = ps(body).stdout
        for line in ('before_down: NOT TAKEN (asked before the map settled)', 'start: deferred until the map settles',
                     'x: taken, failed parts: pools', 'y: taken'):
            self.assertIn(line, out, out)

    def test_the_settle_wait(self):
        body = ("function Start-Sleep { }\n"
                "$script:n = 0; function Invoke-GamePy([string]$Code, [int]$T = 30) { $script:n++; if ($script:n -ge 3) { 'True' } else { 'False' } }\n"
                "'off: ' + (Wait-SliceWatchSettled 5)\n"
                "$script:SliceWatchOn = $true; 'on: ' + (Wait-SliceWatchSettled 5) + ' after ' + $script:n")
        out = ps(body).stdout
        self.assertIn('off: False', out)   # with the watch off it asks nothing
        self.assertIn('on: True after 3', out)

    def test_a_bad_spec_and_too_little_disk_stop_before_anything(self):
        with tempfile.TemporaryDirectory() as tmp:
            bad = Path(tmp) / 'bad.json'
            bad.write_text(json.dumps({'snap_every': -1}), encoding='utf-8')
            good = Path(tmp) / 'good.json'
            good.write_text(json.dumps({'funcs': ['Pawn.Died']}), encoding='utf-8')
            body = (f"try {{ Assert-WatchSpec '{bad}'; 'accepted' }} catch {{ 'refused spec' }}\n"
                    f"Assert-WatchSpec '{good}'; 'good spec'\n"
                    f"$script:RG = '{tmp}'\n"
                    f"function Get-RunRoots {{ [ordered]@{{ SaveData = '{tmp}' }} }}\n"
                    "$r = Assert-DiskReserve 1 1; 'reserve ok'\n"
                    "try { Assert-DiskReserve ([long]::MaxValue / 4) 1 | Out-Null; 'no check' } catch { 'refused disk' }")
            out = ps(body).stdout
            for word in ('refused spec', 'good spec', 'reserve ok', 'refused disk'):
                self.assertIn(word, out)

    def test_mods_off_refuses_another_set_before_writing(self):
        with tempfile.TemporaryDirectory() as tmp:
            settings = Path(tmp) / 'settings'
            settings.mkdir()
            (settings / 'A.json').write_text('{"enabled": true}', encoding='utf-8')
            (settings / 'B.json').write_text('{"enabled": true}', encoding='utf-8')
            (settings / 'C.json').write_text('{"enabled": false}', encoding='utf-8')
            body = (f"function Set-SdkModsOff {{ 'WROTE' }}\n"
                    f"$env:OPENWILLOW_BL2 = '{tmp}'; New-Item -ItemType Directory -Force '{tmp}\\sdk_mods' | Out-Null; Move-Item '{settings}' '{tmp}\\sdk_mods\\settings'\n"
                    "(Get-SdkModsEnabled) -join ','\n"
                    "$log = [ordered]@{}\n"
                    "try { Invoke-SdkModsOffStep $log @('A') } catch { 'refused: ' + (\"$_\" -like '*nothing was changed*') }\n"
                    "try { Invoke-SdkModsOffStep $log 'A,C' } catch { 'refused joined' }\n"
                    "Invoke-SdkModsOffStep $log 'B, A'")
            out = ps(body).stdout
            self.assertIn('A,B', out)
            self.assertIn('refused: True', out)
            self.assertIn('refused joined', out)   # a comma-joined list, as pwsh -File passes it, is split
            self.assertIn('SDK mods off for this run: WROTE', out)
            self.assertEqual(out.count('WROTE'), 1)   # the refused call wrote nothing


WIRED = tuple(CAPTURE_SCRIPTS)


class WiringTest(unittest.TestCase):
    """The order the switches keep in each capture: the spec and the disk checked before anything; mods off only after $launchAttempted is set
    (so the unchanged finalizer restores the settings even if the launch never happens) and before the launch; the watch started after the
    probes load and checked before the quit; nothing added to the pinned finalizer."""
    def test_order(self):
        for name in WIRED:
            text = (ROOT / 'tools/real_game' / name).read_text(encoding='utf-8')
            at = {key: text.find(needle) for key, needle in (
                ('spec', 'if ($WatchSpec) { Assert-WatchSpec $WatchSpec }'), ('disk', '$null = Assert-DiskReserve'),
                ('body', '\ntry {\n'), ('record', 'Save-VerifiedBackupRecord $bak'), ('driver', 'Install-Driver -'),
                ('flag', '$launchAttempted = $true'), ('mods', 'Invoke-SdkModsOffStep $log $SdkModsExpected'), ('launch', 'Start-Game '),
                ('start', 'Start-SliceWatch $WatchSpec'), ('check', 'Complete-SliceWatch $WatchSpec'), ('finally', '\n} catch {\n    $runError'))}
            for key, pos in at.items():
                self.assertGreaterEqual(pos, 0, f'{name}: {key} not found')
            self.assertTrue(at['spec'] < at['disk'] < at['body'] < at['record'] < at['driver'] < at['flag'] < at['mods'] < at['launch']
                            < at['start'] < at['check'] < at['finally'], name)
            self.assertLess(at['check'], text.find('ConsoleCommand("quit"', at['start']), name)
            self.assertEqual(text.count('Invoke-SdkModsOffStep'), 1, name)
            self.assertNotIn('Watch', text[at['finally']:], name)   # the finalizer is pinned (tests/real_game_capture_safety_test.py)

    def test_den_down_removes_only_its_own_hook_and_sends_no_new_input(self):
        text = (ROOT / 'tools/real_game/cash_capture.ps1').read_text(encoding='utf-8')
        block = text[text.index('if ($DenDown) {\n        try {'):text.index("Set-Stage 'den_down' ($null")]
        self.assertIn('cp_immune_off()', block)
        self.assertIn('@($off.remaining).Count', block)   # every path is asked, and any one left stops the trial
        self.assertIn("-in 'up', 'no_pawn', 'no_health', 'injured_flag'", block)   # only an exact state counts
        self.assertIn("if (!$log.den_down_settled) { throw", block)
        self.assertIn("'injured_flag' { 'down' } 'no_health' { 'down_seen_no_health' } default { 'down_seen_no_pawn' }", block)
        self.assertNotIn("Save-SliceWatchSnap 'down'", block)   # 'down' only through the label rule   # no settle, no den down: before_down must come before the immunity goes
        smoke = [line for line in text.splitlines() if '$WatchSmoke -and $WatchSpec' in line]
        self.assertEqual(len(smoke), 2)
        self.assertFalse(any('Send-' in line for line in smoke))   # the smoke switch adds reads, never input
        self.assertEqual(re.findall(r'Send-\w+ \w+', re.sub(r'#.*', '', block)), ['Send-Key Enter'])
        probe = (SCRIPTS / 'cash_probe.py').read_text(encoding='utf-8')
        body = probe[probe.index('def cp_immune_off'):probe.index('def immune_remaining')]
        self.assertIn('remove_hook(path, Type.PRE, CP_IMMUNE_ID)', body)
        self.assertNotIn('add_hook', body)


class DownStateTest(unittest.TestCase):
    """cash_probe's down rule and the remaining-hook check, with invented values."""
    @classmethod
    def setUpClass(cls):
        cls.cash = load('cash_probe', 'tools/real_game/scripts/cash_probe.py')

    def test_down_state(self):
        ds = self.cash.down_state
        self.assertEqual(ds(None, None, None), 'no_pawn')
        self.assertEqual(ds(object(), 0.0, None), 'no_health')
        self.assertEqual(ds(object(), 5.0, True), 'injured_flag')
        self.assertEqual(ds(object(), 5.0, False), 'up')
        self.assertEqual(ds(object(), 5.0, None), 'up')   # no injured field: a down shows only at bleed-out, as no_health or no_pawn

    def test_immune_remaining(self):
        self.assertEqual(self.cash.immune_remaining(['a', 'b', 'c'], lambda p: p == 'b'), ['b'])
        self.assertEqual(self.cash.immune_remaining(['a'], lambda p: False), [])


if __name__ == '__main__':
    unittest.main()
