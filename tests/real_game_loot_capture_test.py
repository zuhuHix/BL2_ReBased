"""Synthetic tests for the lootable capture (no game or mod SDK needed): its output guard, its keeping to the safety pattern,
and its tracked files holding no placed position or object id."""
import importlib.util
import math
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
        self.Rotation = types.SimpleNamespace(Yaw=0, Pitch=0, Roll=0)
        self.Class = types.SimpleNamespace(Name='Thing', _fields=lambda: [], SuperField=None)

    def _path_name(self):
        return self.path


class FakeWorld:
    """A loaded script with an invented world behind unrealsdk."""

    def __init__(self):
        with tempfile.TemporaryDirectory() as folder:
            self.lc = load(Path(folder))
        self.money = 100
        self.pool = self.ammo_pool('Pools.Shotgun', 'D.Ammo_Combat_Shotgun_Pool')
        self.other_pool = self.ammo_pool('Pools.Sniper', 'D.Ammo_Sniper_Rifle_Pool')
        self.pickups = [self.pickup('Map.Pick_1', 'GD.ammodrop_shotgun_shells'), self.pickup('Map.Pick_2', 'GD.Cash_Money')]
        self.objects = []
        state = types.SimpleNamespace(GetCurrencyOnHand=lambda _kind: self.money)
        self.aims = []
        self.pc = Thing('Map.PC', Pawn=Thing('Map.Pawn', SetPhysics=lambda _physics: None), PlayerReplicationInfo=state,
                        CachedUsableObject=None, ClientSetRotation=self.aims.append)
        self.lc.get_pc = lambda: self.pc
        sdk = sys.modules['unrealsdk']
        sdk.make_struct = lambda _name, **fields: types.SimpleNamespace(**fields)
        sdk.find_all = lambda name, _exact: {'WillowPickup': self.pickups, 'ResourcePool': [self.pool, self.other_pool],
                                             'WillowInteractiveObject': self.objects}.get(name, [])

    @staticmethod
    def ammo_pool(path, definition):
        pool = Thing(path, Definition=Thing(definition), AssociatedProvider=Thing('Map.PC.Manager'), CurrentValue=5.0)
        pool.SetCurrentValue = lambda value: setattr(pool, 'CurrentValue', value)
        return pool

    def pickup(self, path, definition):
        item = types.SimpleNamespace(DefinitionData=types.SimpleNamespace(ItemDefinition=Thing(definition)))
        return Thing(path, Inventory=item, bDeleteMe=False)


class PickupStateTest(unittest.TestCase):
    def setUp(self):
        self.world = FakeWorld()
        self.lc = self.world.lc

    def begin(self, kind):
        """As the capture does: stand by the pickup, then take the attempt's baseline."""
        self.lc.lc_stand_pickup(kind)
        return self.lc.lc_pickup_baseline()

    def collect(self, chosen=0):
        """The pickup goes, and leaves a destroyed object behind for anything still holding it."""
        self.world.pickups = [p for i, p in enumerate(self.world.pickups) if i != chosen]

    def test_a_pickup_inside_a_lootable_is_skipped_and_one_in_the_open_is_chosen(self):
        bounds = types.SimpleNamespace(Origin=types.SimpleNamespace(X=0.0, Y=0.0, Z=0.0), SphereRadius=50.0)
        self.world.objects.append(Thing('Map.Box', Mesh=Thing('c', SkeletalMesh=Thing('Pkg.Skel_CardboardBox'), Bounds=bounds), bDeleteMe=False))
        # The only ammo (Pick_1) lies inside the lootable: it is skipped, and the record says why, not "not collected" later.
        chosen = self.lc.lc_choose_pickup('ammo')
        self.assertEqual((chosen['pickup'], chosen['reason'], chosen['skipped']['inside_container']), (None, 'no reachable ammo pickup', 1))
        self.assertEqual(self.lc.lc_stand_pickup('ammo'), {'kind': 'ammo', 'pickup': None})
        far = self.world.pickup('Map.Pick_3', 'GD.Ammo_Item')
        far.Location = types.SimpleNamespace(X=500.0, Y=0.0, Z=0.0)
        self.world.pickups.append(far)
        chosen = self.lc.lc_choose_pickup('ammo')
        self.assertEqual((chosen['pickup'], chosen['reach'], chosen['container'], chosen['pos']), ('Map.Pick_3', 'loose in the open', None, [500.0, 0.0, 0.0]))

    def test_a_thing_that_is_not_a_known_lootable_does_not_hold_a_pickup_out_of_the_open(self):
        bounds = types.SimpleNamespace(Origin=types.SimpleNamespace(X=500.0, Y=0.0, Z=0.0), SphereRadius=50.0)
        vending = Thing('Map.Vending', Mesh=Thing('c', SkeletalMesh=Thing('Pkg.Vending.Skel_Vending'), Bounds=bounds), bDeleteMe=False)
        self.world.objects.append(vending)
        far = self.world.pickup('Map.Pick_3', 'GD.Ammo_Item')
        far.Location = types.SimpleNamespace(X=500.0, Y=0.0, Z=0.0)
        self.world.pickups = [far]
        self.assertEqual(self.lc.lc_choose_pickup('ammo')['reach'], 'loose in the open')   # a vending machine is not a container here
        vending.Mesh.SkeletalMesh = Thing('Pkg.Meshes.Skel_LaundryMachine')                 # the same sphere, of a lootable type, holds it
        self.assertEqual((self.lc.lc_choose_pickup('ammo')['pickup'], self.lc.lc_choose_pickup('ammo')['skipped']['inside_container']), (None, 1))

    def test_a_pickup_above_the_standing_reach_or_too_far_is_skipped(self):
        self.world.pickups[0].Location = types.SimpleNamespace(X=300.0, Y=0.0, Z=500.0)   # above a standing player's floor
        skipped = self.lc.lc_choose_pickup('ammo')['skipped']
        self.assertEqual(skipped['above_reach'], 1)
        self.world.pickups[0].Location = types.SimpleNamespace(X=3000.0, Y=0.0, Z=0.0)     # beyond the search radius
        self.assertEqual(self.lc.lc_choose_pickup('ammo')['skipped']['too_far'], 1)
        self.assertIsNone(self.lc.lc_choose_pickup('ammo')['pickup'])

    def test_another_pool_rising_with_the_chosen_pickup_gone_is_not_collected(self):
        self.begin('ammo')
        self.collect()
        self.world.other_pool.CurrentValue = 9.0   # another pickup, touched on the way
        state = self.lc.lc_pickup_state()
        self.assertEqual((state['gone'], state['changed'], state['collected']), (True, False, False))
        self.world.pool.CurrentValue = 9.0         # its own pool
        self.assertEqual(self.lc.lc_pickup_state()['collected'], True)

    def test_what_moved_before_the_attempt_is_not_credited_to_it(self):
        self.lc.lc_stand_pickup('ammo')
        self.world.pool.CurrentValue = 9.0         # on the way here, before the baseline
        self.lc.lc_pickup_baseline()
        self.collect()
        self.assertEqual(self.lc.lc_pickup_state()['collected'], False)

    def test_a_chosen_pickup_gone_before_the_attempt_is_not_present(self):
        self.lc.lc_stand_pickup('ammo')
        self.collect()
        self.assertEqual(self.lc.lc_pickup_baseline()['present'], False)
        self.world.pool.CurrentValue = 9.0
        self.assertEqual(self.lc.lc_pickup_state()['collected'], False)

    def test_no_baseline_for_the_attempt_is_not_collected(self):
        self.begin('ammo')
        self.lc.lc_stand_pickup('ammo')            # the second try's walk clears the first's baseline
        self.collect()
        self.world.pool.CurrentValue = 9.0
        self.assertEqual(self.lc.lc_pickup_state()['collected'], False)

    def test_no_baseline_for_a_cash_attempt_is_not_collected(self):
        self.lc.lc_stand_pickup('cash')
        self.collect(1)
        self.world.money += 50
        self.assertEqual(self.lc.lc_pickup_state()['collected'], False)

    def test_an_ammo_type_with_no_known_pool_credits_nothing(self):
        self.world.pickups[0] = self.world.pickup('Map.Pick_1', 'GD.ammodrop_unknown')
        self.begin('ammo')
        self.collect()
        self.world.pool.CurrentValue = 9.0
        self.assertEqual(self.lc.lc_pickup_state()['collected'], False)

    def test_the_ammo_stage_lists_only_pools_that_fell(self):
        self.world.other_pool.SetCurrentValue = lambda _value: (_ for _ in ()).throw(RuntimeError('no'))
        result = self.lc.lc_lower_ammo()
        self.assertEqual(result['fallen'], ['Ammo_Combat_Shotgun_Pool'])
        self.world.pool.SetCurrentValue = self.world.other_pool.SetCurrentValue
        self.assertEqual(self.lc.lc_lower_ammo()['fallen'], [])

    def test_a_walk_over_onto_a_loose_pickup_is_skipped_and_the_record_says_so(self):
        # No floor under a loose pickup was read, so Maya is not dropped onto it; the use key still runs.
        self.lc.lc_choose_pickup('ammo')
        self.world.pickups[0].Location = types.SimpleNamespace(X=7.0, Y=8.0, Z=9.0)
        result = self.lc.lc_over_pickup()
        self.assertEqual(result['pickup'], 'Map.Pick_1')
        self.assertTrue(result['walk_over'].startswith('skipped: no floor under the target was read'), result)
        self.assertEqual((self.world.pc.Pawn.Location.X, self.world.pc.Pawn.Location.Y), (0.0, 0.0))
        self.lc.lc_pickup_baseline()   # the state is judged against a baseline, as in an attempt
        self.assertTrue(self.lc.lc_pickup_state()['walk_over'].startswith('skipped'))

    def test_a_chosen_pickup_that_went_has_nothing_to_stand_by(self):
        self.lc.lc_choose_pickup('ammo')
        self.collect()
        self.assertEqual(self.lc.lc_over_pickup(), {'pickup': None})
        self.assertEqual(self.lc.lc_aim_pickup(), {'pickup': None})

    def test_nothing_kept_between_commands_is_a_game_object(self):
        self.begin('ammo')
        self.assertEqual(self.lc.LC['pickup']['path'], 'Map.Pick_1')
        self.assertIsInstance(self.lc.LC['pickup']['path'], str)

    def test_a_collected_pickup_is_not_touched_again(self):
        self.begin('ammo')
        self.lc.LC['pickup']['object'] = Dead()   # were anything to use a kept reference, this would raise
        self.collect()
        self.world.pool.CurrentValue = 9.0
        self.assertEqual(self.lc.lc_pickup_state()['collected'], True)

    def test_a_pickup_gone_without_the_value_rising_is_not_collected(self):
        self.begin('ammo')
        self.collect()
        state = self.lc.lc_pickup_state()
        self.assertEqual((state['gone'], state['changed'], state['collected']), (True, False, False))

    def test_the_value_rising_with_the_pickup_still_there_is_not_collected(self):
        self.begin('cash')
        self.world.money = 500   # another pickup, say
        state = self.lc.lc_pickup_state()
        self.assertEqual((state['gone'], state['changed'], state['collected']), (False, True, False))

    def test_cash_counts_money_and_ammo_counts_pools(self):
        self.begin('cash')
        self.collect(1)
        self.world.pool.CurrentValue = 9.0
        self.assertEqual(self.lc.lc_pickup_state()['collected'], False)   # ammo rose, not money
        self.world.money = 150
        self.assertEqual(self.lc.lc_pickup_state()['collected'], True)

    def test_a_failed_read_never_reports_collected(self):
        # The old check read an error string as "gone".
        def unreadable(_kind):
            raise RuntimeError('read failed')
        self.begin('cash')
        self.collect(1)
        self.world.pc.PlayerReplicationInfo.GetCurrencyOnHand = unreadable
        state = self.lc.lc_pickup_state()
        self.assertEqual((state['gone'], state['collected']), (True, False))

    def test_a_pickup_whose_state_cannot_be_read_is_not_gone(self):
        self.begin('ammo')
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

    def test_ready_only_when_the_usable_object_is_this_one_and_nudges_otherwise(self):
        self.world.pc.Pawn.Location = types.SimpleNamespace(X=210.0, Y=20.0, Z=30.0)
        self.world.pc.CachedUsableObject = Thing('Map.Other')
        first = self.lc.lc_ready('Skel_Box', 0)
        self.assertEqual((first['usable'], first['use_target'], first['next']), (False, 'Map.Other', 'low'))
        self.assertEqual(len(self.world.aims), 1)   # re-aimed for the next read
        step = self.lc.lc_ready('Skel_Box', 1)
        self.assertEqual(step['next'], 'step')
        # Stepped closer on the same side: 90 uu from the centre at X=10, with the pawn put above the floor to fall.
        self.assertAlmostEqual(self.world.pc.Pawn.Location.X, 100.0)
        self.world.pc.CachedUsableObject = self.box
        self.assertEqual(self.lc.lc_ready('Skel_Box', 2), {'object': 'Map.Box', 'attempt': 2, 'use_target': 'Map.Box', 'usable': True})

    def test_next_object_moves_on_by_path_with_a_fresh_closed_state(self):
        self.world.objects.append(Thing('Map.Box2', Mesh=self.box.Mesh, bDeleteMe=False, SimpleAnimState=0, RepSimpleAnimState=0, bCanBeUsed=True))
        self.lc.lc_stand('Skel_Box')
        self.assertIn('Skel_Box', self.lc.LC['before'])
        self.assertEqual(self.lc.lc_next_object('Skel_Box'), {'object': 'Map.Box2', 'count': 2})
        self.assertNotIn('Skel_Box', self.lc.LC['before'])
        self.assertEqual(self.lc.lc_stand('Skel_Box')['object'], 'Map.Box2')
        self.assertEqual(self.lc.lc_next_object('Skel_Box'), {'object': None, 'count': 2})

    def test_pickups_have_world_positions_and_an_offset_that_shows_a_mirror(self):
        self.box.Rotation = types.SimpleNamespace(Yaw=16384, Pitch=0, Roll=0)   # a quarter turn: forward is +Y
        left, right = self.world.pickups
        left.Location = types.SimpleNamespace(X=-10.0, Y=5.0, Z=2.0)
        right.Location = types.SimpleNamespace(X=10.0, Y=5.0, Z=2.0)
        self.lc.lc_stand('Skel_Box')
        rows = {p['path']: p for p in self.lc.lc_opened('Skel_Box')['pickups']}
        self.assertEqual(rows['Map.Pick_1']['pos'], [-10.0, 5.0, 2.0])
        # The same distance on both, so a distance alone cannot tell them apart. The offset's right component can.
        self.assertEqual(rows['Map.Pick_1']['distance'], rows['Map.Pick_2']['distance'])
        self.assertEqual(rows['Map.Pick_1']['local'], [5.0, 10.0, 2.0])
        self.assertEqual(rows['Map.Pick_2']['local'], [5.0, -10.0, 2.0])

    def ready_before(self, leaf='Skel_Box', box=None):
        """The usable read just before the press, as the capture does it: the pickups lying there now are not this container's drop."""
        self.world.pc.CachedUsableObject = box or self.box
        self.lc.lc_ready(leaf, 0)

    def make_container(self, path, mesh, x):
        bounds = types.SimpleNamespace(Origin=types.SimpleNamespace(X=x, Y=0.0, Z=0.0), SphereRadius=25.0)
        box = Thing(path, Mesh=Thing('c', SkeletalMesh=Thing('Pkg.Meshes.' + mesh), Bounds=bounds), bDeleteMe=False,
                    SimpleAnimState=0, RepSimpleAnimState=0, bCanBeUsed=True)
        box.Location = types.SimpleNamespace(X=x, Y=0.0, Z=0.0)
        self.world.objects.append(box)
        return box

    def test_a_drop_on_the_floor_of_an_opened_box_is_chosen_and_one_inside_it_is_skipped(self):
        self.lc.lc_stand('Skel_Box')
        self.ready_before()
        floor = self.world.pickup('Map.Floor', 'GD.ammodrop_patrol_smg_clip')
        floor.Location = types.SimpleNamespace(X=5.0, Y=5.0, Z=2.0)       # 2 uu above the box's origin, as the Z53 clip lay
        inside = self.world.pickup('Map.Inside', 'GD.ammodrop_sniper_rifle_cartridges')
        inside.Location = types.SimpleNamespace(X=0.0, Y=0.0, Z=44.0)     # 44 uu up in the box's frame, inside it, and nearer the player
        self.world.pickups = [floor, inside]
        self.box.bCanBeUsed = False
        self.lc.lc_opened('Skel_Box')
        self.world.pc.Pawn.Location = types.SimpleNamespace(X=10.0, Y=-200.0, Z=70.0)
        chosen = self.lc.lc_choose_pickup('ammo')
        self.assertEqual((chosen['pickup'], chosen['reach'], chosen['container']), ('Map.Floor', 'dropped on the floor', 'Map.Box'))
        self.assertEqual((chosen['container_local'], chosen['pos'], chosen['skipped']['inside_container']), ([5.0, 5.0, 2.0], [5.0, 5.0, 2.0], 1))
        self.assertEqual(chosen['pool'], 'Patrol_SMG')
        self.assertEqual(self.lc.LC['pickup']['path'], 'Map.Floor')

    def test_a_drop_is_judged_in_its_containers_frame_not_from_where_the_player_stands(self):
        # Run Z54: the cardboard box's clip, 2.4 uu above its box, was refused while the player stood on another level. Stands teleport the
        # player beside the pickup, so only a loose pickup is measured from her.
        self.lc.lc_stand('Skel_Box')
        self.ready_before()
        floor = self.world.pickup('Map.Floor', 'GD.ammodrop_patrol_smg_clip')
        floor.Location = types.SimpleNamespace(X=5.0, Y=5.0, Z=2.0)
        self.world.pickups = [floor]
        self.box.bCanBeUsed = False
        self.lc.lc_opened('Skel_Box')
        self.world.pc.Pawn.Location = types.SimpleNamespace(X=9000.0, Y=9000.0, Z=3000.0)
        chosen = self.lc.lc_choose_pickup('ammo')
        self.assertEqual((chosen['pickup'], chosen['reach']), ('Map.Floor', 'dropped on the floor'))

    def test_a_pickup_by_a_box_that_did_not_open_is_not_a_drop(self):
        self.lc.lc_stand('Skel_Box')
        floor = self.world.pickup('Map.Floor', 'GD.ammodrop_patrol_smg_clip')
        floor.Location = types.SimpleNamespace(X=5.0, Y=5.0, Z=2.0)
        self.world.pickups = [floor]
        self.assertFalse(self.lc.lc_opened('Skel_Box')['opened'])
        self.assertNotIn('Map.Floor', self.lc.LC['drops'])
        self.world.pc.Pawn.Location = types.SimpleNamespace(X=10.0, Y=-200.0, Z=70.0)
        self.assertEqual(self.lc.lc_choose_pickup('ammo')['reach'], 'loose in the open')   # outside the box's bounds, so loose

    def test_a_drop_with_nothing_else_reachable_reports_no_reachable_pickup(self):
        self.lc.lc_stand('Skel_Box')
        self.ready_before()
        inside = self.world.pickup('Map.Inside', 'GD.ammodrop_sniper_rifle_cartridges')
        inside.Location = types.SimpleNamespace(X=0.0, Y=0.0, Z=87.0)     # the Z53 laundry machine's ammo: 87 uu up
        self.world.pickups = [inside]
        self.box.bCanBeUsed = False
        self.lc.lc_opened('Skel_Box')
        self.world.pc.Pawn.Location = types.SimpleNamespace(X=10.0, Y=-200.0, Z=70.0)
        chosen = self.lc.lc_choose_pickup('ammo')
        self.assertEqual((chosen['pickup'], chosen['reason'], chosen['skipped']['inside_container']), (None, 'no reachable ammo pickup', 1))
        self.assertIsNone(self.lc.LC['pickup'])

    def test_each_drop_is_tagged_once_by_the_container_that_dropped_it(self):
        # B's usable read comes first, so B's pre-press list does not hold the drop A makes; B's open then sees it nearby and must not take it.
        a = self.make_container('Map.BoxA', 'Skel_CardboardBox', 0.0)
        b = self.make_container('Map.BoxB', 'Skel_Dumpster', 100.0)
        old = self.world.pickup('Map.Old', 'GD.ammodrop_patrol_smg_clip')
        old.Location = types.SimpleNamespace(X=50.0, Y=0.0, Z=2.0)
        self.world.pickups = [old]
        self.lc.lc_stand('Skel_Dumpster')
        self.ready_before('Skel_Dumpster', box=b)
        self.lc.lc_stand('Skel_CardboardBox')
        self.ready_before('Skel_CardboardBox', box=a)
        a.bCanBeUsed = False
        drop_a = self.world.pickup('Map.DropA', 'GD.ammodrop_patrol_smg_clip')
        drop_a.Location = types.SimpleNamespace(X=5.0, Y=0.0, Z=2.0)
        self.world.pickups.append(drop_a)
        self.assertEqual(self.lc.lc_opened('Skel_CardboardBox')['drops_tagged'], ['Map.DropA'])
        b.bCanBeUsed = False
        drop_b = self.world.pickup('Map.DropB', 'GD.ammodrop_patrol_smg_clip')
        drop_b.Location = types.SimpleNamespace(X=95.0, Y=0.0, Z=2.0)
        self.world.pickups.append(drop_b)
        self.assertEqual(self.lc.lc_opened('Skel_Dumpster')['drops_tagged'], ['Map.DropB'])
        self.assertEqual({path: d['container'] for path, d in self.lc.LC['drops'].items()}, {'Map.DropA': 'Map.BoxA', 'Map.DropB': 'Map.BoxB'})

    def test_a_drop_from_a_tilted_container_is_not_judged_a_floor_drop(self):
        self.box.Rotation = types.SimpleNamespace(Yaw=0, Pitch=0, Roll=-4544)   # about 25 degrees, as the Z53d cardboard box read
        self.lc.lc_stand('Skel_Box')
        self.ready_before()
        floor = self.world.pickup('Map.Floor', 'GD.ammodrop_patrol_smg_clip')
        floor.Location = types.SimpleNamespace(X=5.0, Y=5.0, Z=2.0)
        self.world.pickups = [floor]
        self.box.bCanBeUsed = False
        result = self.lc.lc_opened('Skel_Box')
        self.assertTrue(result['drop_frame'].startswith('tilted'), result)
        self.assertEqual((result['drops_tagged'], self.lc.LC['drops']), ([], {}))

    def test_an_unreadable_rotation_is_not_level_either(self):
        self.box.Rotation = types.SimpleNamespace(Yaw=0)   # no pitch or roll to read
        self.lc.lc_stand('Skel_Box')
        self.ready_before()
        self.world.pickups = [self.world.pickup('Map.Floor', 'GD.ammodrop_patrol_smg_clip')]
        self.box.bCanBeUsed = False
        result = self.lc.lc_opened('Skel_Box')
        self.assertTrue(result['drop_frame'].startswith('rotation unreadable'), result)
        self.assertEqual(self.lc.LC['drops'], {})

    def test_the_nearest_eligible_pickup_wins_whether_it_is_a_drop_or_not(self):
        self.lc.lc_stand('Skel_Box')
        self.ready_before()
        drop = self.world.pickup('Map.Floor', 'GD.ammodrop_patrol_smg_clip')
        drop.Location = types.SimpleNamespace(X=5.0, Y=5.0, Z=2.0)
        self.world.pickups = [drop]
        self.box.bCanBeUsed = False
        self.lc.lc_opened('Skel_Box')
        loose = self.world.pickup('Map.Loose', 'GD.ammodrop_patrol_smg_clip')
        loose.Location = types.SimpleNamespace(X=10.0, Y=-380.0, Z=2.0)
        self.world.pickups.append(loose)   # after the open: not the box's drop
        self.world.pc.Pawn.Location = types.SimpleNamespace(X=10.0, Y=-200.0, Z=70.0)   # the drop is about 216 uu away, the loose one 192
        chosen = self.lc.lc_choose_pickup('ammo')
        self.assertEqual((chosen['pickup'], chosen['reach']), ('Map.Loose', 'loose in the open'))

    def test_a_walk_over_falls_only_onto_a_drops_floor_when_it_is_within_reach(self):
        self.lc.lc_stand('Skel_Box')
        self.ready_before()
        floor = self.world.pickup('Map.Floor', 'GD.ammodrop_patrol_smg_clip')
        floor.Location = types.SimpleNamespace(X=5.0, Y=5.0, Z=2.0)
        self.world.pickups = [floor]
        self.box.bCanBeUsed = False
        self.lc.lc_opened('Skel_Box')
        self.lc.lc_choose_pickup('ammo')
        self.assertEqual(self.lc.lc_over_pickup()['walk_over'], 'fall')   # the box's origin, 0 uu, is within the pickup's reach
        self.assertEqual((self.world.pc.Pawn.Location.X, self.world.pc.Pawn.Location.Y), (5.0, 5.0))
        self.lc.LC['drops']['Map.Floor']['floor_z'] = 50.0   # a floor 48 uu above the pickup: outside its reach
        self.world.pc.Pawn.Location = types.SimpleNamespace(X=-300.0, Y=-300.0, Z=70.0)
        self.lc.lc_choose_pickup('ammo')
        result = self.lc.lc_over_pickup()
        self.assertTrue(result['walk_over'].startswith('skipped: the floor under the target is outside'), result)
        self.assertEqual((self.world.pc.Pawn.Location.X, self.world.pc.Pawn.Location.Y), (-300.0, -300.0))

    def bones(self, door_turn_degrees, base_turn_degrees=0.0, hinge=(-20.0, 25.0, 6.0)):
        """A root bone and a door bone on the hinge at (-20, 25): only the door bone's rotation changes, about the vertical, as in run Z53."""
        names = ['Base', 'Door']
        spots = {'Base': (0.0, 0.0, 0.0), 'Door': hinge}

        def about_z(degrees):
            half = math.radians(degrees) / 2.0
            return (0.0, 0.0, math.sin(half), math.cos(half))
        quats = {'Base': about_z(base_turn_degrees), 'Door': about_z(door_turn_degrees)}
        component = self.box.Mesh
        component.GetBoneName = lambda i: names[i] if i < len(names) else 'None'
        component.GetBoneLocation = lambda name, _space: types.SimpleNamespace(X=spots[name][0], Y=spots[name][1], Z=spots[name][2])
        component.GetBoneQuaternion = lambda name, _space: types.SimpleNamespace(X=quats[name][0], Y=quats[name][1], Z=quats[name][2], W=quats[name][3])

    def test_the_door_view_stands_on_the_side_the_door_turned_to(self):
        self.bones(0.0)
        self.lc.lc_stand('Skel_Box')
        self.bones(90.0)   # a hinge on the +Y face's corner, turned a quarter: the box's origin swings out to +Y
        self.box.bCanBeUsed = False
        result = self.lc.lc_opened('Skel_Box')
        door = result['door']
        self.assertEqual((door['bone'], door['turned'], door['yaw']), ('Door', 90.0, 90.0))
        self.assertAlmostEqual(door['raw_yaw'], 83.7, delta=0.1)   # before the snap to a quarter turn of the facing
        self.assertEqual(result['bones_closed']['Door']['pos'], result['bones']['Door']['pos'])   # the position did not move
        view = self.lc.lc_view('Skel_Box', 'door')
        self.assertEqual((view['from'], view['yaw']), ('lid bone', 90.0))
        pawn = self.world.pc.Pawn.Location
        self.assertAlmostEqual(pawn.X, 10.0)
        self.assertAlmostEqual(pawn.Y, 20.0 + self.lc.VIEW_DISTANCE)   # on that side of the mesh's centre, at the fixed distance
        self.assertEqual(self.lc.lc_view('Skel_Box', 'opposite')['yaw'], 270.0)

    def test_the_door_side_follows_the_objects_facing_and_a_wider_swing(self):
        self.box.Rotation = types.SimpleNamespace(Yaw=384, Pitch=0, Roll=0)   # a little off the axes, as the Z53 box was
        self.bones(0.0)
        self.lc.lc_stand('Skel_Box')
        self.bones(125.0)
        self.box.bCanBeUsed = False
        self.assertAlmostEqual(self.lc.lc_opened('Skel_Box')['door']['yaw'], 92.1, delta=0.1)   # facing + a quarter turn

    def test_a_bone_that_turned_too_little_marks_no_door(self):
        self.bones(0.0)
        self.lc.lc_stand('Skel_Box')
        self.bones(5.0)
        self.box.bCanBeUsed = False
        self.assertIsNone(self.lc.lc_opened('Skel_Box')['door'])

    def open_with(self, **after):
        self.bones(0.0, hinge=after.get('hinge', (-20.0, 25.0, 6.0)))
        self.lc.lc_stand('Skel_Box')
        self.bones(**after)
        self.box.bCanBeUsed = False
        return self.lc.lc_opened('Skel_Box')['door']

    def test_two_bones_turning_alike_mark_no_door_and_the_view_falls_back(self):
        self.assertIsNone(self.open_with(door_turn_degrees=90.0, base_turn_degrees=89.5))   # within 1 degree: no single door
        self.assertEqual(self.lc.lc_view('Skel_Box', 'door')['from'], 'object facing')

    def test_bones_exactly_one_degree_apart_mark_no_door(self):
        # On _door directly, with exact quaternions: the record rounds them to 4 places, about 0.01 degree.
        def bone(degrees, spot):
            half = math.radians(degrees) / 2.0
            return {'pos': list(spot), 'quat': [0.0, 0.0, math.sin(half), math.cos(half)]}
        before = {'Base': bone(0.0, (0, 0, 0)), 'Door': bone(0.0, (-20, 25, 6))}

        def door(base_degrees):
            return self.lc._door(before, {'Base': bone(base_degrees, (0, 0, 0)), 'Door': bone(90.0, (-20, 25, 6))}, (0.0, 0.0, 0.0), 0.0)
        self.assertIsNone(door(89.0))                    # the boundary counts as a tie
        self.assertEqual(door(88.9)['bone'], 'Door')     # just past it does not

    def test_eridium_is_not_cash(self):
        # Z53c chose an eridium stick as cash: its definition sits under gd_currency, but it moves no money.
        self.world.pickups = [self.world.pickup('Map.Pick_E', 'gd_currency.a_item.eridiumstick'), self.world.pickup('Map.Pick_C', 'gd_currency.a_item.currency')]
        self.assertEqual(self.lc._kind(self.world.pickups[0])[0], 'eridium')
        self.assertEqual(self.lc.lc_choose_pickup('cash')['pickup'], 'Map.Pick_C')
        self.assertEqual(self.lc.lc_view('Skel_Box', 'door')['from'], 'object facing')

    def test_a_clear_strongest_bone_still_marks_the_door(self):
        self.assertEqual(self.open_with(door_turn_degrees=90.0, base_turn_degrees=40.0)['bone'], 'Door')

    def test_an_origin_that_barely_moves_marks_no_door(self):
        # A hinge 0.4 uu from the origin: a quarter turn moves the origin well under 1 uu across the floor.
        self.assertIsNone(self.open_with(door_turn_degrees=90.0, hinge=(-0.4, 0.0, 6.0)))
        self.assertEqual(self.lc.lc_view('Skel_Box', 'door')['from'], 'object facing')

    def test_where_reports_the_press_stand_distance(self):
        self.world.pc.Pawn.Location = types.SimpleNamespace(X=10.0, Y=-180.0, Z=30.0)
        self.assertEqual((self.lc.lc_where('Skel_Box')['distance'], self.lc.lc_where('Skel_Box')['floor_distance']), (200, 200))

    def test_an_empty_read_records_the_objects_state_and_distance(self):
        self.world.pc.Pawn.Location = types.SimpleNamespace(X=10.0, Y=-180.0, Z=30.0)
        row = self.lc.lc_ready('Skel_Box', 0)
        self.assertEqual((row['use_target'], row['state'], row['distance']), (None, {'SimpleAnimState': 0, 'RepSimpleAnimState': 0, 'bCanBeUsed': True}, 200))

    def test_without_a_bone_that_moved_the_facing_stands_in(self):
        self.lc.lc_stand('Skel_Box')
        self.box.bCanBeUsed = False
        result = self.lc.lc_opened('Skel_Box')
        self.assertIsNone(result['door'])
        self.assertEqual(list(result['bones']), ['<read>'])   # this fake component has no bone calls: the failure is recorded
        self.assertEqual(self.lc.lc_view('Skel_Box', 'door')['from'], 'object facing')

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
                # A world coordinate is a number of four or more digits; the window size, timeouts, rotator units and date are named here,
                # and so is the pickup search radius (2000 uu, a host choice, not a position).
                numbers = [n for n in re.findall(r'-?\b\d{4,}(?:\.\d+)?\b', text)
                           if n not in ('1280', '1800', '15000', '65536', '65536.0', '2026', '2000.0')]
                self.assertEqual(numbers, [])


if __name__ == '__main__':
    unittest.main()
