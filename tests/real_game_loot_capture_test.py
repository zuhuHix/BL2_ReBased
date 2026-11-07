"""Synthetic tests for the lootable capture (no game or mod SDK needed): its output guard, its keeping to the safety pattern,
and its tracked files holding no placed position or object id."""
import importlib.util
from pathlib import Path
import re
import sys
import tempfile
import types
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'tools/real_game/scripts/loot_capture.py'
CAPTURE = ROOT / 'tools/real_game/loot_capture.ps1'


def load(rg_out):
    sdk = types.ModuleType('unrealsdk')
    sys.modules['unrealsdk'] = sdk
    spec = importlib.util.spec_from_file_location('ow_loot_capture', SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.RG_OUT = str(rg_out)   # the command channel's prelude sets this in the shared namespace
    return module


class Dead:
    """A destroyed game object: touching it is what crashed the game."""

    def __getattr__(self, name):
        raise AssertionError(f'touched a destroyed object: {name}')


class Thing:
    def __init__(self, path, **fields):
        self.path = path
        self.__dict__.update(fields)
        self.Location = types.SimpleNamespace(X=0.0, Y=0.0, Z=0.0)
        self.Rotation = types.SimpleNamespace(Yaw=0)
        self.Class = types.SimpleNamespace(Name='Thing', _fields=lambda: [], SuperField=None)

    def _path_name(self):
        return self.path


class FakeWorld:
    """A loaded script with an invented world behind unrealsdk."""

    def __init__(self):
        with tempfile.TemporaryDirectory() as folder:
            self.lc = load(Path(folder))
        self.money = 100
        self.pool = Thing('Pools.Ammo', Definition=Thing('D.Ammo_Pool'), AssociatedProvider=Thing('Map.PC.Manager'), CurrentValue=5.0)
        self.pickups = [self.pickup('Map.Pick_1', 'GD.Ammo_Item'), self.pickup('Map.Pick_2', 'GD.Cash_Money')]
        self.objects = []
        state = types.SimpleNamespace(GetCurrencyOnHand=lambda _kind: self.money)
        self.aims = []
        self.pc = Thing('Map.PC', Pawn=Thing('Map.Pawn', SetPhysics=lambda _physics: None), PlayerReplicationInfo=state,
                        CachedUsableObject=None, ClientSetRotation=self.aims.append)
        self.lc.get_pc = lambda: self.pc
        sdk = sys.modules['unrealsdk']
        sdk.make_struct = lambda _name, **fields: types.SimpleNamespace(**fields)
        sdk.find_all = lambda name, _exact: {'WillowPickup': self.pickups, 'ResourcePool': [self.pool],
                                             'WillowInteractiveObject': self.objects}.get(name, [])

    def pickup(self, path, definition):
        item = types.SimpleNamespace(DefinitionData=types.SimpleNamespace(ItemDefinition=Thing(definition)))
        return Thing(path, Inventory=item, bDeleteMe=False)


class PickupStateTest(unittest.TestCase):
    def setUp(self):
        self.world = FakeWorld()
        self.lc = self.world.lc

    def collect(self, chosen=0):
        """The pickup goes, and leaves a destroyed object behind for anything still holding it."""
        self.world.pickups = [p for i, p in enumerate(self.world.pickups) if i != chosen]

    def test_a_pickup_in_the_open_is_chosen_over_a_nearer_one_inside_a_lootable(self):
        bounds = types.SimpleNamespace(Origin=types.SimpleNamespace(X=0.0, Y=0.0, Z=0.0), SphereRadius=50.0)
        self.world.objects.append(Thing('Map.Box', Mesh=Thing('c', SkeletalMesh=Thing('Pkg.Skel_Box'), Bounds=bounds), bDeleteMe=False))
        self.assertEqual(self.lc.lc_choose_pickup('ammo')['in_the_open'], False)   # the only one: taken, and the record says so
        far = self.world.pickup('Map.Pick_3', 'GD.Ammo_Item')
        far.Location = types.SimpleNamespace(X=500.0, Y=0.0, Z=0.0)
        self.world.pickups.append(far)
        chosen = self.lc.lc_choose_pickup('ammo')
        self.assertEqual((chosen['pickup'], chosen['in_the_open']), ('Map.Pick_3', True))

    def test_the_walk_over_lands_on_the_pickup_and_the_chosen_one_is_judged(self):
        self.lc.lc_choose_pickup('ammo')
        self.world.pickups[0].Location = types.SimpleNamespace(X=7.0, Y=8.0, Z=9.0)
        self.assertEqual(self.lc.lc_over_pickup()['pickup'], 'Map.Pick_1')
        self.assertEqual((self.world.pc.Pawn.Location.X, self.world.pc.Pawn.Location.Y), (7.0, 8.0))

    def test_a_chosen_pickup_that_went_has_nothing_to_stand_by(self):
        self.lc.lc_choose_pickup('ammo')
        self.collect()
        self.assertEqual(self.lc.lc_over_pickup(), {'pickup': None})
        self.assertEqual(self.lc.lc_aim_pickup(), {'pickup': None})

    def test_nothing_kept_between_commands_is_a_game_object(self):
        self.lc.lc_stand_pickup('ammo')
        self.assertEqual(self.lc.LC['pickup']['path'], 'Map.Pick_1')
        self.assertIsInstance(self.lc.LC['pickup']['path'], str)

    def test_a_collected_pickup_is_not_touched_again(self):
        self.lc.lc_stand_pickup('ammo')
        self.lc.LC['pickup']['object'] = Dead()   # were anything to use a kept reference, this would raise
        self.collect()
        self.world.pool.CurrentValue = 9.0
        self.assertEqual(self.lc.lc_pickup_state()['collected'], True)

    def test_a_pickup_gone_without_the_value_rising_is_not_collected(self):
        self.lc.lc_stand_pickup('ammo')
        self.collect()
        state = self.lc.lc_pickup_state()
        self.assertEqual((state['gone'], state['changed'], state['collected']), (True, False, False))

    def test_the_value_rising_with_the_pickup_still_there_is_not_collected(self):
        self.lc.lc_stand_pickup('cash')
        self.world.money = 500   # another pickup, say
        state = self.lc.lc_pickup_state()
        self.assertEqual((state['gone'], state['changed'], state['collected']), (False, True, False))

    def test_cash_counts_money_and_ammo_counts_pools(self):
        self.lc.lc_stand_pickup('cash')
        self.collect(1)
        self.world.pool.CurrentValue = 9.0
        self.assertEqual(self.lc.lc_pickup_state()['collected'], False)   # ammo rose, not money
        self.world.money = 150
        self.assertEqual(self.lc.lc_pickup_state()['collected'], True)

    def test_a_failed_read_never_reports_collected(self):
        # The old check read an error string as "gone".
        def unreadable(_kind):
            raise RuntimeError('read failed')
        self.lc.lc_stand_pickup('cash')
        self.collect(1)
        self.world.pc.PlayerReplicationInfo.GetCurrencyOnHand = unreadable
        state = self.lc.lc_pickup_state()
        self.assertEqual((state['gone'], state['collected']), (True, False))

    def test_a_pickup_whose_state_cannot_be_read_is_not_gone(self):
        self.lc.lc_stand_pickup('ammo')
        self.world.pool.CurrentValue = 9.0
        chosen = self.world.pickups[0]
        delete_flag = property(lambda _self: (_ for _ in ()).throw(RuntimeError('read failed')))
        chosen.__class__ = type('Unreadable', (Thing,), {'bDeleteMe': delete_flag})
        del chosen.__dict__['bDeleteMe']
        state = self.lc.lc_pickup_state()
        self.assertEqual((state['gone'], state['collected']), (False, False))

    def test_no_pickup_chosen_is_not_collected(self):
        self.assertEqual(self.lc.lc_pickup_state()['collected'], False)


class OpenedTest(unittest.TestCase):
    def setUp(self):
        self.world = FakeWorld()
        self.lc = self.world.lc
        mesh = Thing('Pkg.Meshes.Skel_Box')
        bounds = types.SimpleNamespace(Origin=types.SimpleNamespace(X=10.0, Y=20.0, Z=30.0), SphereRadius=25.0)
        self.box = Thing('Map.Box', Mesh=Thing('c', SkeletalMesh=mesh, Bounds=bounds), bDeleteMe=False, SimpleAnimState=0, RepSimpleAnimState=0, bCanBeUsed=True)
        self.world.objects.append(self.box)

    def test_a_state_field_that_changed_is_opened(self):
        self.lc.lc_stand('Skel_Box')
        self.assertEqual(self.lc.lc_opened('Skel_Box')['opened'], False)
        self.box.bCanBeUsed = False
        result = self.lc.lc_opened('Skel_Box')
        self.assertEqual((result['opened'], result['changed']), (True, {'bCanBeUsed': [True, False]}))

    def test_loot_nearby_alone_is_not_opened(self):
        # A stray press on a neighbour drops loot here too. The old stage counted frames, and loot is no better.
        self.lc.lc_stand('Skel_Box')
        self.world.pickups.append(self.world.pickup('Map.Pick_9', 'GD.Ammo_Item'))
        result = self.lc.lc_opened('Skel_Box')
        self.assertEqual(result['opened'], False)
        self.assertEqual(len(result['pickups']), 3)

    def test_an_unreadable_field_is_not_evidence(self):
        self.lc.lc_stand('Skel_Box')
        del self.box.__dict__['SimpleAnimState']   # now it reads as an error string
        result = self.lc.lc_opened('Skel_Box')
        self.assertEqual((result['opened'], result['changed']), (False, {}))

    def test_a_later_side_compares_with_the_first_state(self):
        self.lc.lc_stand('Skel_Box', 0)
        self.box.SimpleAnimState = 2     # opened from the first side; only read after a later stand
        self.lc.lc_stand('Skel_Box', 4)
        self.assertEqual(self.lc.lc_opened('Skel_Box')['opened'], True)

    def test_it_aims_at_the_mesh_not_a_guess_above_the_origin(self):
        # The strong box: a small mesh under a shelf, missed by looking at a height above the actor.
        self.world.pc.Pawn.BaseEyeHeight = 64.0
        self.world.pc.Pawn.Location = types.SimpleNamespace(X=10.0, Y=-200.0, Z=30.0)
        self.lc.lc_aim('Skel_Box')
        yaw = self.world.aims[-1].Yaw * 360.0 / 65536.0
        self.assertAlmostEqual(yaw, 90.0, delta=0.1)   # straight at Y=20 from Y=-200, at X=10: the bounds' origin

    def test_each_side_stands_around_the_mesh(self):
        spots = {side: self.lc._stand_at(self.box, side)[0] for side in (0, 4)}
        self.assertAlmostEqual(spots[0][0], 180.0)
        self.assertAlmostEqual(spots[4][0], -160.0)
        self.assertAlmostEqual(self.lc._stand_at(self.box, 0)[1][2], 30.0)

    def test_a_read_of_the_mods_changes_nothing_and_survives_failing(self):
        self.assertTrue(str(self.lc.lc_mods()).startswith('<error'))   # no mods_base here: recorded, not raised


class MaterialsTest(unittest.TestCase):
    def setUp(self):
        self.world = FakeWorld()
        self.lc = self.world.lc
        self.lc.lc_root = lambda: self.folder
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.folder = Path(folder.name)

    def place(self, leaf, material='Pkg.Mat', slots=('Pkg.Mat',)):
        mesh = Thing(f'Pkg.Meshes.{leaf}', Materials=[Thing(s) if s else None for s in slots])
        component = Thing('c', SkeletalMesh=mesh, GetMaterial=lambda _slot: Thing(material) if material else None)
        self.world.objects.append(Thing(f'Map.{leaf}', Mesh=component, bDeleteMe=False))
        sys.modules['unrealsdk'].find_all = (lambda real, m: lambda name, exact: [m] if name == 'SkeletalMesh' else real(name, exact))(sys.modules['unrealsdk'].find_all, mesh)

    def test_read_values_have_no_problems(self):
        self.place('Skel_StrongBox')
        self.assertEqual(self.lc.lc_materials('materials')['problems'], [])

    def test_an_empty_read_is_a_problem_though_the_file_is_written(self):
        self.place('Skel_StrongBox', material=None, slots=(None,))
        problems = self.lc.lc_materials('materials')['problems']
        self.assertEqual(len(problems), 2)
        self.assertTrue((self.folder / 'materials.json').is_file())

    def test_no_placed_lootable_is_a_problem(self):
        self.assertEqual(self.lc.lc_materials('materials')['problems'], ['no placed lootable to read'])


class OutputGuardTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name)
        (self.repo / '.git').write_text('gitdir: elsewhere\n')   # a worktree's .git is a file
        self.out = self.repo / 'local/realgame/loot_capture'
        self.lc = load(self.out)

    def test_a_plain_label_lands_in_rg_out(self):
        self.assertEqual(self.lc.lc_out('materials'), (self.out / 'materials.json').resolve())

    def test_escaping_labels_are_refused(self):
        for label in ('../../../docs/x', '..', 'a/b', 'a\\b', '/abs', 'C:x', '', '.hidden', None, 3):
            with self.subTest(label=label), self.assertRaises(ValueError):
                self.lc.lc_out(label)

    def test_rg_out_outside_local_is_refused(self):
        for rg_out in (self.repo / 'docs', self.repo / 'local', self.repo, self.repo / 'local/../docs'):
            self.lc.RG_OUT = str(rg_out)
            with self.subTest(rg_out=rg_out), self.assertRaises(ValueError):
                self.lc.lc_out('materials')
        self.assertFalse((self.repo / 'docs').exists())


class CaptureScriptTest(unittest.TestCase):
    def setUp(self):
        self.text = CAPTURE.read_text(encoding='utf-8')

    def test_a_failed_game_script_stops_the_step(self):
        # The command channel prints an exception as the reply instead of raising it.
        self.assertIn("-match 'Traceback (most recent call last)'".replace('(', r'\(').replace(')', r'\)'), self.text)
        self.assertIn('throw "game script failed', self.text)

    def test_it_never_kills_by_image_name(self):
        self.assertIsNone(re.search(r'taskkill|Stop-Process\s+-Name|pkill|killall', self.text, re.I))

    def test_no_key_is_sent_before_a_focus_check(self):
        # Each function or stage that sends a key checks focus first.
        for start in ('function Capture-Type', 'function Capture-Pickup'):
            body = self.text[self.text.index(start):]
            first_key = min(i for i in (body.find('Send-Key'),) if i >= 0)
            self.assertLess(body.index('Assert-GameForeground'), first_key)
        for line in self.text.splitlines():
            if line.lstrip().startswith(('Start-Sleep 5;', 'Assert-GameForeground;')) and 'Send-Key Enter' in line:
                self.assertIn('Assert-GameForeground', line)

    def test_the_script_saves_nothing(self):
        code = SCRIPT.read_text(encoding='utf-8')
        self.assertIsNone(re.search(r'SaveGame|SaveRawData|\bsavegame\b', code + self.text, re.I))


class NoPlacedDataTest(unittest.TestCase):
    def test_no_object_id_or_coordinate_is_tracked(self):
        for path in (SCRIPT, CAPTURE):
            text = path.read_text(encoding='utf-8')
            with self.subTest(path=path.name):
                self.assertNotIn('OpportunityPoint', text)
                self.assertNotIn('Sanctuary_Dynamic', text)
                # A world coordinate is a number of four or more digits; the window size, timeouts, rotator units and date are named here.
                numbers = [n for n in re.findall(r'-?\b\d{4,}(?:\.\d+)?\b', text) if n not in ('1280', '1800', '15000', '65536', '65536.0', '2026')]
                self.assertEqual(numbers, [])


if __name__ == '__main__':
    unittest.main()
