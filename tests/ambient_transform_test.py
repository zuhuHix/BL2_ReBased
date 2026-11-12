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


class AnnotateLooksTest(unittest.TestCase):
    """annotate_looks on an invented capture: one citizen wearing a hat on Head and a pack on Spine3 (made-up names)."""

    def setUp(self):
        saved = P.attachment_transforms
        P.attachment_transforms = lambda kind, bones: {bone: np.eye(4) for bone in bones}
        self.addCleanup(setattr, P, 'attachment_transforms', saved)

    def run_looks(self, report_meshes, known_missing=()):
        import json
        import tempfile
        compose = [{'path': 'P.Citizen_1', 'materials': [], 'attachments': [
            {'static_mesh': 'Pkg.Mesh.Hat_A', 'bone': 'Head', 'materials': []},
            {'static_mesh': 'Pkg.Mesh.Pack_A', 'bone': 'Spine3', 'materials': []},
            {'static_mesh': 'Pkg.Mesh.Ring_A', 'bone': 'Finger', 'materials': []}]}]   # a bone the host does not use
        report = {'meshes': {name: {'asset': f'/Game/X/{name}.{name}'} for name in report_meshes}, 'materials': {}, 'heads': {}, 'zone': {}}
        spawns = [{'pawn_path': 'P.Citizen_1', 'kind': 'CitizenMale'}]
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'compose.json'
            path.write_text(json.dumps(compose), encoding='utf-8')
            P.annotate_looks(spawns, str(path), ['CitizenMale'], report, set(known_missing))
        return spawns[0]['attachments']

    def test_every_piece_in_the_report_is_placed(self):
        self.assertEqual([a['bone'] for a in self.run_looks(['Hat_A', 'Pack_A'])], ['Head', 'Spine3'])

    def test_a_piece_missing_from_the_attach_report_stops_the_tool(self):
        # 2026-11-12: a generated input that lost data must not become "nothing to place".
        with self.assertRaises(SystemExit) as stop:
            self.run_looks(['Hat_A'])
        self.assertIn('lacks 1 worn meshes', str(stop.exception))
        self.assertIn('Pack_A', str(stop.exception))
        self.assertIn('-Steps attach', str(stop.exception))
        self.assertIn('previous ambient_world.json is left as it was and is now out of date', str(stop.exception))

    def test_a_mesh_the_attach_job_recorded_as_unavailable_is_left_out(self):
        self.assertEqual([a['bone'] for a in self.run_looks(['Hat_A'], ['Pkg.Mesh.Pack_A'])], ['Head'])


if __name__ == '__main__':
    unittest.main()
