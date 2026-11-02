"""Synthetic tests for tools/gestalt_sockets.py (invented values; no package bytes)."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import gestalt_sockets as g


def prop(name, value, **extra):
    return {'name': name, 'status': 'decoded', 'value': value, **extra}


def vec(x, y, z):
    return {'X': x, 'Y': y, 'Z': z}


class SocketTest(unittest.TestCase):
    def test_socket_defaults(self):
        # the cooked stream omits a zero rotation and a unit scale
        name, socket = g.parse_socket({'properties': [prop('SocketName', 'A_Muzzle'), prop('BoneName', 'Barrel'),
                                                      prop('RelativeLocation', vec(12, 0, 2))]})
        self.assertEqual(name, 'A_Muzzle')
        self.assertEqual(socket, {'bone': 'Barrel', 'location': [12, 0, 2], 'rotation': [0, 0, 0], 'scale': [1.0, 1.0, 1.0]})

    def test_socket_rotation_and_scale_are_kept(self):
        _, socket = g.parse_socket({'properties': [
            prop('SocketName', 'S'), prop('BoneName', 'Root'),
            prop('RelativeRotation', {'Pitch': 0, 'Yaw': -16384, 'Roll': 0}), prop('RelativeScale', vec(2, 2, 2))]})
        self.assertEqual(socket['rotation'], [0, -16384, 0])
        self.assertEqual(socket['scale'], [2, 2, 2])
        self.assertEqual(socket['location'], [0.0, 0.0, 0.0])

    def table(self):
        def mapping(fragment, original, mangled):
            return [prop('SkeletalMeshFragmentName', fragment), prop('OriginalSocketName', original),
                    prop('MangledSocketName', mangled)]
        box = [prop('Origin', vec(0, -16, 6)), prop('BoxExtent', vec(2, 9, 4)), prop('SphereRadius', 11.0)]
        return {'properties': [
            {'name': 'GestaltPartBounds', 'status': 'decoded', 'value': [
                [prop('SkeletalMeshFragmentName', 'Barrel_X'), prop('ReferencePoseBounds', box)]]},
            {'name': 'GestaltSocketMappings', 'status': 'decoded', 'value': [
                mapping('Barrel_X', 'Muzzle', 'Barrel_X_Muzzle'), mapping('Barrel_X', 'Sight', 'Barrel_X_Sight')]}]}

    def test_fragment_sockets_and_missing_mapping(self):
        sockets = {'Barrel_X_Muzzle': {'bone': 'Barrel', 'location': [1, 0, 0], 'rotation': [0, 0, 0], 'scale': [1, 1, 1]}}
        fragments, missing = g.fragment_sockets(self.table(), sockets)
        self.assertEqual(fragments['Barrel_X']['bounds'], {'origin': [0, -16, 6], 'extent': [2, 9, 4], 'radius': 11.0})
        self.assertEqual(list(fragments['Barrel_X']['sockets']), ['Muzzle'])
        self.assertEqual(fragments['Barrel_X']['sockets']['Muzzle']['socket'], 'Barrel_X_Muzzle')
        self.assertEqual(missing, [{'fragment': 'Barrel_X', 'original': 'Sight', 'mangled': 'Barrel_X_Sight'}])

    def test_same_original_name_twice_is_an_error(self):
        table = self.table()
        table['properties'][1]['value'].append([prop('SkeletalMeshFragmentName', 'Barrel_X'),
                                                prop('OriginalSocketName', 'Muzzle'), prop('MangledSocketName', 'Other')])
        sockets = {'Barrel_X_Muzzle': {}, 'Other': {}, 'Barrel_X_Sight': {}}
        with self.assertRaises(RuntimeError):
            g.fragment_sockets(table, sockets)


class ReferencePoseTest(unittest.TestCase):
    def gltf(self):
        # Root (identity) -> Barrel: 0.1 m along glTF z, then a 90 degree turn about glTF y
        s = 2 ** -0.5
        return {'nodes': [{'name': 'Root', 'children': [1]},
                          {'name': 'Barrel', 'translation': [0, 0, 0.1], 'rotation': [0, s, 0, s]}]}

    def test_bone_pose_composes_the_chain(self):
        rotation, translation = g.bone_pose(self.gltf(), 'Barrel')
        self.assertEqual([round(v, 6) for v in translation], [0.0, 0.0, 0.1])
        # a quarter turn about y sends the local z axis onto x: the matrix' third column is (1, 0, 0)
        self.assertEqual([round(rotation[r][2], 6) for r in range(3)], [1.0, 0.0, 0.0])

    def test_mesh_location_swaps_y_and_z_and_scales_to_cm(self):
        # glTF z (0.1 m) is the cooked y axis (10 cm); a bone-local offset keeps its length
        self.assertEqual(g.mesh_location(self.gltf(), 'Barrel', [0, 0, 0]), [0.0, 10.0, 0.0])
        moved = g.mesh_location(self.gltf(), 'Barrel', [5, 0, 0])
        self.assertAlmostEqual((moved[0] ** 2 + (moved[1] - 10.0) ** 2 + moved[2] ** 2) ** 0.5, 5.0, places=3)

    def test_unknown_bone(self):
        with self.assertRaises(KeyError):
            g.bone_pose(self.gltf(), 'Nope')

    def test_inside_bounds(self):
        bounds = {'origin': [0, -16, 6], 'extent': [2, 9, 4], 'radius': 11}
        self.assertTrue(g.check_inside_bounds([0, -27, 6], bounds))   # 2 cm past the front face, inside the margin
        self.assertFalse(g.check_inside_bounds([0, -40, 6], bounds))


class RecipeSocketsTest(unittest.TestCase):
    def test_attach_sockets_keeps_first_fragment_and_lists_the_rest(self):
        import json
        import tempfile
        import weapon_slice_gear
        socket = {'socket': 'x', 'bone': 'Root', 'location': [0, 1, 2], 'rotation': [0, 0, 0], 'scale': [1, 1, 1]}
        data = {'fragments': {'Body': {'bounds': None, 'sockets': {'EjectPort': socket, 'RearSight': socket}},
                              'Body_Var1': {'bounds': None, 'sockets': {'RearSight': socket}},
                              'Barrel': {'bounds': None, 'sockets': {'Muzzle': socket}}}}
        with tempfile.TemporaryDirectory() as name:
            folder = Path(name)
            (folder / 'Pistol.sockets.json').write_text(json.dumps(data), encoding='utf-8')
            recipe = {'gestalt': {'path': 'Weap_Pistol.GestaltDef_Pistol'}, 'sockets': {'stale': {}},
                      'gestalt_fragments': ['Body_Var1', 'Body', 'Barrel', 'Unknown']}
            self.assertEqual(weapon_slice_gear.attach_sockets(recipe, folder), 'ok')
            self.assertEqual(sorted(recipe['sockets']), ['EjectPort', 'Muzzle', 'RearSight'])
            self.assertEqual(recipe['sockets']['RearSight']['fragment'], 'Body')
            self.assertEqual(recipe['sockets']['RearSight']['also_from'], ['Body_Var1'])
            self.assertEqual(recipe['sockets']['Muzzle']['fragment'], 'Barrel')
            # no file for the family: the recipe loses its old sockets and says why
            recipe['gestalt'] = 'Weap_SMG.GestaltDef_SMG'
            self.assertTrue(weapon_slice_gear.attach_sockets(recipe, folder).startswith('skipped'))
            self.assertNotIn('sockets', recipe)


if __name__ == '__main__':
    unittest.main()
