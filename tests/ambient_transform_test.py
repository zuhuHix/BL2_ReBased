"""Synthetic test for tools/prepare_ambient_world.py component_transform / rotator_matrix (invented numbers, no game data).

The bone frame handed to component_transform can be a reflection (the imported skeleton is a Y mirror of the UE3 one while the
static meshes keep UE3 coordinates), so the (location, quaternion, scale) it returns must rebuild the same matrix, with a negative
Z scale when the result is a reflection. Needs numpy.
"""
from pathlib import Path
import sys
import unittest

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import prepare_ambient_world as P  # noqa: E402


def quat_matrix(q):
    x, y, z, w = q
    return np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                     [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                     [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]])


def rebuild(loc, quat, scale):
    m = np.eye(4)
    m[:3, :3] = quat_matrix(quat) @ np.diag(scale)
    m[:3, 3] = loc
    return m


class ComponentTransformTest(unittest.TestCase):
    def bone(self, reflect):
        rot = P.rotator_matrix(1200, 9000, -700)
        m = np.eye(4)
        m[:3, :3] = rot @ (np.diag([1.0, 1.0, -1.0]) if reflect else np.eye(3))
        m[:3, 3] = [3.0, -2.0, 40.0]
        return m

    def att(self):
        return {'comp_translation': [10.0, -15.0, 6.5], 'comp_rotation': [-1313, 31776, -10765], 'comp_scale': 2.2, 'comp_scale3d': [1.0, 1.1, 0.9]}

    def check(self, reflect):
        bone, att = self.bone(reflect), self.att()
        loc, quat, scale = P.component_transform(bone, att)
        t3 = np.eye(4)
        t3[:3, :3] = P.rotator_matrix(*att['comp_rotation']) @ np.diag([att['comp_scale'] * v for v in att['comp_scale3d']])
        t3[:3, 3] = att['comp_translation']
        self.assertLess(np.abs(rebuild(loc, quat, scale) - bone @ t3).max(), 1e-3)
        return scale

    def test_rotation_only_bone_keeps_positive_scale(self):
        self.assertTrue(all(s > 0 for s in self.check(False)))

    def test_reflected_bone_gives_one_negative_scale(self):
        scale = self.check(True)
        self.assertLess(scale[2], 0)
        self.assertTrue(scale[0] > 0 and scale[1] > 0)

    def test_without_component_fields_only_the_bone_frame_remains(self):
        loc, quat, scale = P.component_transform(self.bone(False), {})
        self.assertLess(np.abs(rebuild(loc, quat, scale) - self.bone(False)).max(), 1e-3)

    def test_rotator_zero_is_identity_and_units_are_65536_per_turn(self):
        self.assertLess(np.abs(P.rotator_matrix(0, 0, 0) - np.eye(3)).max(), 1e-12)
        yaw90 = P.rotator_matrix(0, 16384, 0)
        self.assertLess(np.abs(yaw90 @ np.array([1.0, 0.0, 0.0]) - np.array([0.0, 1.0, 0.0])).max(), 1e-9)


if __name__ == '__main__':
    unittest.main()
