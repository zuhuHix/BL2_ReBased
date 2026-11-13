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


class ManifestGuardTest(unittest.TestCase):
    """Keep or refuse, never degrade: invented manifests (made-up counts), no game data. 2026-11-12: the bare call replaced a
    capture-based manifest (observed, worn pieces) with a census-only one without a word."""

    CAPTURED = {'observed': True, 'spawns': [{'id': 'a', 'attachments': [{}, {}]}, {'id': 'b', 'attachments': [{}]}]}
    CENSUS = {'observed': False, 'spawns': [{'id': 'c'}, {'id': 'd'}, {'id': 'e'}]}

    def folder(self):
        import tempfile
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        return Path(tmp.name)

    def existing(self, value):
        import json
        out = self.folder() / 'ambient_world.json'
        out.write_text(value if isinstance(value, str) else json.dumps(value), encoding='utf-8')
        return out

    def test_an_observed_manifest_is_not_replaced_by_a_census_one(self):
        problem = P.replace_problem(self.existing(self.CAPTURED), self.CENSUS, False)
        self.assertIn('observed=true spawns=2 worn=3', problem)
        self.assertIn('observed=false spawns=3 worn=0', problem)
        self.assertIn('--observed', problem)
        self.assertIn('--replace-observed', problem)
        self.assertIn('ambient_npcs.py', problem)

    def test_fewer_worn_pieces_from_a_capture_are_refused_too(self):
        fewer = {'observed': True, 'spawns': [{'id': 'a', 'attachments': [{}]}]}
        self.assertIn('worn=1', P.replace_problem(self.existing(self.CAPTURED), fewer, False))
        self.assertEqual(P.replace_problem(self.existing(self.CAPTURED), self.CAPTURED, False), '')   # the same capture again

    def test_worn_pieces_alone_make_a_manifest_capture_based(self):
        worn_only = {'spawns': [{'id': 'a', 'attachments': [{}]}]}
        self.assertIn('worn=1', P.replace_problem(self.existing(worn_only), self.CENSUS, False))

    def test_the_flag_writes_over_it(self):
        self.assertEqual(P.replace_problem(self.existing(self.CAPTURED), self.CENSUS, True), '')

    def test_no_file_or_a_census_file_is_written(self):
        self.assertEqual(P.replace_problem(self.folder() / 'ambient_world.json', self.CENSUS, False), '')
        self.assertEqual(P.replace_problem(self.existing(self.CENSUS), self.CENSUS, False), '')

    def test_an_unreadable_file_is_not_overwritten(self):
        for text in ('{"spawns": [', '[1, 2]', '{"observed": true}'):
            problem = P.replace_problem(self.existing(text), self.CENSUS, False)
            self.assertIn('cannot be read as an ambient manifest', problem, text)
            self.assertEqual(P.replace_problem(self.existing(text), self.CENSUS, True), '')

    def test_compose_without_observed_is_refused_before_any_work(self):
        with self.assertRaises(SystemExit) as stop:
            P.main(['--game', str(self.folder()), '--compose', 'x.json', '--output', str(P.ROOT / 'local' / 'unused.json')])
        self.assertIn('pass --observed and --samples', str(stop.exception))

    def check(self, value, census_ok=False):
        import contextlib
        import io
        printed = io.StringIO()
        with contextlib.redirect_stdout(printed):
            code = P.check_manifest(self.existing(value), census_ok)
        return code, printed.getvalue()

    def test_the_suite_preflight_names_the_look_and_fails_a_census_world(self):
        code, line = self.check(self.CAPTURED)
        self.assertEqual((code, line.strip()), (0, 'manifest observed=true spawns=2 worn=3'))
        code, line = self.check(self.CENSUS)
        self.assertEqual(code, 1)
        self.assertIn('observed=false spawns=3 worn=0: not capture-based', line)
        self.assertIn('-CensusWorld', line)
        code, line = self.check(self.CENSUS, census_ok=True)
        self.assertEqual(code, 0)
        self.assertIn('census-only world, intended', line)
        self.assertEqual(self.check('{')[0], 1)


if __name__ == '__main__':
    unittest.main()
