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
        self.pc = Thing('Map.PC', Pawn=Thing('Map.Pawn', SetPhysics=lambda _physics: None), PlayerReplicationInfo=state,
                        FastTrace=lambda _end, _start: True, ClientSetRotation=lambda _rotation: None)
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
