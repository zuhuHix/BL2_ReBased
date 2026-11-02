"""Synthetic test of tools/weapon_refresh_firing.py: invented firing modes and recipes, no game data."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import weapon_refresh_firing as refresh  # noqa: E402

ZERO = {'Pitch': 0, 'Yaw': 0, 'Roll': 0}


def mode_props(**changes):
    props = {'FireType': 'EWWFT_Bullet', 'Speed': 1000.0, 'AirFriction': -2.0, 'NumRicochets': 1,
             'TraceExtent': {'X': 1.0, 'Y': 2.0, 'Z': 3.0}, 'bSuppressWeaponSpread': True, 'bFireRandomlyFromPattern': False,
             'bScalePatternByWeaponSpread': True, 'bResetPatternAfterEachShot': True, 'BasePatternSpread': 4.0,
             'MinPatternScale': 0.5, 'PartSysTemplate': 'Fx.Tracer', 'ProjectileDefinition': None,
             'FiringPatternLines': [
                 {'StartPoint': {'Pitch': 0, 'Yaw': 182, 'Roll': 0}, 'EndPoint': ZERO, 'bUseStartPointOnly': True},
                 {'StartPoint': {'Pitch': -364, 'Yaw': 0, 'Roll': 0}, 'EndPoint': {'Pitch': 364, 'Yaw': 1820, 'Roll': 0},
                  'bUseStartPointOnly': False}]}
    props.update(changes)
    return props


class WeaponRefreshFiringTest(unittest.TestCase):
    def test_block_converts_rotator_units_and_kinds(self):
        block = refresh.build_block('Mode.A', mode_props(), 3, 7)
        self.assertEqual(block['fire_type'], 'Bullet')
        self.assertEqual(block['trace_extent'], [1.0, 2.0, 3.0])
        self.assertEqual(block['projectiles_per_shot'], 3)
        self.assertEqual(block['instant_hit_momentum'], 7)
        point, rectangle = block['pattern_lines']
        self.assertEqual(point, {'kind': 'point', 'start': [0.0, round(182 / (65536 / 360), 6), 0.0]})
        self.assertEqual(rectangle['kind'], 'rectangle')
        self.assertAlmostEqual(rectangle['start'][0], -1.999512, places=5)   # -364 units / 182.0444 per degree
        self.assertAlmostEqual(rectangle['end'][1], 9.99756, places=4)

    def test_type_momentum_reads_base_value_or_none(self):
        cooked = {'GD.Type': {'InstantHitMomentum': {'BaseValueConstant': 8}}, 'GD.Other': {}}
        self.assertEqual(refresh.type_momentum(cooked, 'GD.Type'), 8)
        self.assertEqual(refresh.type_momentum(cooked, {'path': 'GD.Type'}), 8)
        self.assertIsNone(refresh.type_momentum(cooked, 'GD.Other'))
        self.assertIsNone(refresh.type_momentum(cooked, 'GD.Missing'))

    def test_command_adds_block_keeps_backup_and_skips_unknown_modes(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            live, items, backup = folder / 'live', folder / 'items', folder / 'backup'
            live.mkdir()
            items.mkdir()
            (live / 'firing_modes.json').write_text(json.dumps({'FiringModeDefinition': {'Mode.A': {'props': mode_props()}}}))
            for name, mode in (('gun_a', 'Mode.A'), ('gun_b', 'Mode.Unknown')):
                (items / f'{name}.json').write_text(json.dumps({'weapon_type': 'GD.Type', 'stats': {'card': {
                    'firing_mode': mode, 'projectiles': 2, 'projectile_speed': 2000.0, 'spread': 1.5}}}))
            command = [sys.executable, str(ROOT / 'tools/weapon_refresh_firing.py'), '--dir', str(items), '--live', str(live),
                       '--backup', str(backup)]
            self.assertEqual(subprocess.run(command, capture_output=True, text=True).returncode, 0)
            changed = json.loads((items / 'gun_a.json').read_text())['stats']['card']
            self.assertEqual(changed['firing']['projectiles_per_shot'], 2)
            self.assertEqual(changed['spread'], 1.5)                                  # nothing else in the card changes
            self.assertNotIn('firing', json.loads((items / 'gun_b.json').read_text())['stats']['card'])
            self.assertNotIn('firing', json.loads((backup / 'gun_a.json').read_text())['stats']['card'])
            # a second run keeps the first backup, not the already-changed file
            subprocess.run(command, capture_output=True, text=True)
            self.assertNotIn('firing', json.loads((backup / 'gun_a.json').read_text())['stats']['card'])


if __name__ == '__main__':
    unittest.main()
