"""Synthetic diagnostic grid geometry; no native flag semantics assumed."""
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from prepare_terrain_probe import grid_obj


class GridTests(unittest.TestCase):
    def test_grid_uses_all_cells_and_keeps_flag_groups_disjoint(self):
        heights = [32768, 32896, 33024, 32768, 32896, 33024]
        flags = [0, 1, 99, 99, 99, 99]
        for flag in (0, 1):
            text, triangles = grid_obj(3, 2, heights, flags, flag)
            self.assertEqual(triangles, 2)
            self.assertEqual(sum(line.startswith('v ') for line in text.splitlines()), 6)
            self.assertIn('v 2 0 2', text)
            faces = [line for line in text.splitlines() if line.startswith('f ')]
            self.assertEqual(len(faces), 2)
        self.assertEqual(grid_obj(3, 2, heights, flags, 99)[1], 0)

    def test_invalid_dimensions_rejected(self):
        for width, height, heights, flags in [(1, 2, [0, 0], [0, 0]),
                                              (2, 2, [0] * 3, [0] * 4),
                                              (2, 2, [0] * 4, [0] * 3)]:
            with self.assertRaises(ValueError):
                grid_obj(width, height, heights, flags, 0)


if __name__ == '__main__':
    unittest.main()
