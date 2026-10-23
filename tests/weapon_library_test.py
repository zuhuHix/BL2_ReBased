"""Synthetic tests for tools/weapon_library.py (no game data; names and values are invented).

The glTF is a hand-built two-section LOD: section 0 holds three triangles, section 1 three, and the fragment
table gives FragA a range in each section. Index values are distinct per section so a wrong slice shows up.
"""
import copy
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_library as L

S0 = [10, 11, 12, 13, 14, 15, 16, 17, 18]  # section 0 indices (3 triangles)
S1 = [20, 21, 22, 23, 24, 25, 26, 27, 28]  # section 1 indices (3 triangles)


def synthetic_mesh():
    """One mesh, one buffer, two index primitives (materials 0 and 1) sharing one POSITION accessor."""
    blob = bytearray(b'\xff' * 4) + struct.pack('<9H', *S0) + b'\xff' * 2 + struct.pack('<9H', *S1)
    gltf = {
        'asset': {'version': '2.0'},
        'buffers': [{'uri': 'mesh.bin', 'byteLength': len(blob)}],
        'bufferViews': [{'buffer': 0, 'byteOffset': 4, 'byteLength': 18, 'target': 34963},
                        {'buffer': 0, 'byteOffset': 22, 'byteLength': 20, 'target': 34963}],
        'accessors': [{'bufferView': 0, 'componentType': 5123, 'count': 9, 'type': 'SCALAR'},
                      {'bufferView': 1, 'byteOffset': 2, 'componentType': 5123, 'count': 9, 'type': 'SCALAR'},
                      {'componentType': 5126, 'count': 4, 'type': 'VEC3'}],
        'meshes': [{'primitives': [{'attributes': {'POSITION': 2}, 'indices': 0, 'material': 0},
                                   {'attributes': {'POSITION': 2}, 'indices': 1, 'material': 1}]}],
        'materials': [{'name': 'Mat_Body'}, {'name': 'Mat_Trim'}],
    }
    return gltf, blob


def synthetic_parts():
    """FragA owns one range in each section; FragB sits in section 0; FragC in section 1."""
    return [
        {'SkeletalMeshFragmentName': 'FragA', 'MaterialIndex': 0, 'FirstIndex': 0, 'NumPrimitives': 1},
        {'SkeletalMeshFragmentName': 'FragB', 'MaterialIndex': 0, 'FirstIndex': 3, 'NumPrimitives': 2},
        {'SkeletalMeshFragmentName': 'FragA', 'MaterialIndex': 1, 'FirstIndex': 15, 'NumPrimitives': 1},
        {'SkeletalMeshFragmentName': 'FragC', 'MaterialIndex': 1, 'FirstIndex': 9, 'NumPrimitives': 2},
    ]


def read_primitive_indices(gltf, blob, slot):
    primitive = gltf['meshes'][0]['primitives'][slot]
    accessor = gltf['accessors'][primitive['indices']]
    view = gltf['bufferViews'][accessor['bufferView']]
    offset = view.get('byteOffset', 0) + accessor.get('byteOffset', 0)
    return list(struct.unpack_from(f'<{accessor["count"]}H', blob, offset))


class SplitTests(unittest.TestCase):
    def setUp(self):
        self.gltf, self.blob = synthetic_mesh()

    def test_one_primitive_per_fragment_with_slot_material(self):
        records = L.split(self.gltf, self.blob, synthetic_parts())
        primitives = self.gltf['meshes'][0]['primitives']
        self.assertEqual(len(primitives), 3)
        self.assertEqual([p['material'] for p in primitives], [0, 1, 2])
        self.assertEqual([r['slot'] for r in records], [0, 1, 2])
        self.assertEqual([r['name'] for r in records], ['FragA', 'FragB', 'FragC'])
        self.assertEqual([m['name'] for m in self.gltf['materials']], ['FragA', 'FragB', 'FragC'])
        for primitive in primitives:
            self.assertEqual(primitive['attributes'], {'POSITION': 2})

    def test_index_data_is_the_concatenated_source_ranges(self):
        L.split(self.gltf, self.blob, synthetic_parts())
        self.assertEqual(read_primitive_indices(self.gltf, self.blob, 0), [10, 11, 12, 26, 27, 28])
        self.assertEqual(read_primitive_indices(self.gltf, self.blob, 1), [13, 14, 15, 16, 17, 18])
        self.assertEqual(read_primitive_indices(self.gltf, self.blob, 2), [20, 21, 22, 23, 24, 25])

    def test_records_report_triangles_and_source_sections(self):
        records = L.split(self.gltf, self.blob, synthetic_parts())
        self.assertEqual(records[0], {'name': 'FragA', 'slot': 0, 'triangles': 2, 'source_sections': [0, 1],
                                      'source_materials': ['Mat_Body', 'Mat_Trim']})
        self.assertEqual(records[1], {'name': 'FragB', 'slot': 1, 'triangles': 2, 'source_sections': [0],
                                      'source_materials': ['Mat_Body']})
        self.assertEqual(records[2], {'name': 'FragC', 'slot': 2, 'triangles': 2, 'source_sections': [1],
                                      'source_materials': ['Mat_Trim']})
        self.assertEqual(sum(r['triangles'] for r in records), sum(p['NumPrimitives'] for p in synthetic_parts()))

    def test_appended_views_stay_inside_the_buffer(self):
        L.split(self.gltf, self.blob, synthetic_parts())
        end = max(v['byteOffset'] + v['byteLength'] for v in self.gltf['bufferViews'])
        self.assertLessEqual(end, len(self.blob))
        self.assertTrue(all(v['byteOffset'] % 2 == 0 for v in self.gltf['bufferViews']))

    def test_section_total_mismatch_raises(self):
        parts = synthetic_parts()
        parts[1]['NumPrimitives'] = 1  # section 0 now needs 6 indices, the export has 9
        with self.assertRaisesRegex(ValueError, 'section 0'):
            L.split(self.gltf, self.blob, parts)

    def test_range_past_its_section_end_raises(self):
        parts = synthetic_parts()
        parts[1]['FirstIndex'] = 9  # FragB's range starts at section 1's first index but claims section 0
        with self.assertRaisesRegex(ValueError, 'FragB: range outside section 0'):
            L.split(self.gltf, self.blob, parts)

    def test_range_before_its_section_start_raises(self):
        parts = synthetic_parts()
        parts[3]['FirstIndex'] = 3  # local start would be -6 inside section 1
        with self.assertRaisesRegex(ValueError, 'FragC: range outside section 1'):
            L.split(self.gltf, self.blob, parts)

    def test_fragment_ranges_with_different_attributes_raise(self):
        self.gltf['meshes'][0]['primitives'][1]['attributes'] = {'POSITION': 2, 'NORMAL': 2}
        with self.assertRaisesRegex(ValueError, 'FragA: ranges use different vertex attributes'):
            L.split(self.gltf, self.blob, synthetic_parts())

    def test_non_16_bit_indices_raise(self):
        self.gltf['accessors'][0]['componentType'] = 5125
        with self.assertRaisesRegex(ValueError, '16-bit'):
            L.split(self.gltf, self.blob, synthetic_parts())

    def test_split_does_not_change_the_fixture_when_copied(self):
        original = copy.deepcopy(self.gltf)
        L.split(copy.deepcopy(self.gltf), bytearray(self.blob), synthetic_parts())
        self.assertEqual(self.gltf, original)


class GroupFragmentsTests(unittest.TestCase):
    def test_first_seen_order_and_multiple_ranges(self):
        grouped = L.group_fragments(synthetic_parts())
        self.assertEqual(list(grouped), ['FragA', 'FragB', 'FragC'])
        self.assertEqual(grouped['FragA'], [{'material': 0, 'first_index': 0, 'triangles': 1},
                                            {'material': 1, 'first_index': 15, 'triangles': 1}])
        self.assertEqual(grouped['FragB'], [{'material': 0, 'first_index': 3, 'triangles': 2}])
        self.assertEqual(grouped['FragC'], [{'material': 1, 'first_index': 9, 'triangles': 2}])

    def test_order_follows_the_table_not_the_name(self):
        parts = list(reversed(synthetic_parts()))
        self.assertEqual(list(L.group_fragments(parts)), ['FragC', 'FragA', 'FragB'])


class SkinAssetsTests(unittest.TestCase):
    def test_asset_path_format(self):
        self.assertEqual(L.skin_asset('MIC_Solo'),
                         '/Game/OpenWillow/Weapons/Library/Skins/MI_MIC_Solo.MI_MIC_Solo')

    def test_duplicate_leaf_names_get_numeric_suffixes_in_sorted_order(self):
        mics = ['Pkg_B.Other.MIC_Synthetic_Red', 'Pkg_A.Gun.MIC_Synthetic_Red', 'Pkg_C.Thing.MIC_Solo']
        assets = L.skin_assets(mics)
        self.assertEqual(assets['Pkg_A.Gun.MIC_Synthetic_Red'],
                         '/Game/OpenWillow/Weapons/Library/Skins/MI_MIC_Synthetic_Red.MI_MIC_Synthetic_Red')
        self.assertEqual(assets['Pkg_B.Other.MIC_Synthetic_Red'],
                         '/Game/OpenWillow/Weapons/Library/Skins/MI_MIC_Synthetic_Red_2.MI_MIC_Synthetic_Red_2')
        self.assertEqual(assets['Pkg_C.Thing.MIC_Solo'],
                         '/Game/OpenWillow/Weapons/Library/Skins/MI_MIC_Solo.MI_MIC_Solo')

    def test_suffixes_do_not_depend_on_input_order(self):
        mics = ['Pkg_B.Other.MIC_Dup', 'Pkg_A.Gun.MIC_Dup', 'Pkg_D.Last.MIC_Dup']
        forward = L.skin_assets(mics)
        backward = L.skin_assets(list(reversed(mics)))
        self.assertEqual(forward, backward)
        self.assertTrue(forward['Pkg_D.Last.MIC_Dup'].endswith('MI_MIC_Dup_3.MI_MIC_Dup_3'))


class LiveVectorsTests(unittest.TestCase):
    def test_converts_complete_entries_to_rgba_floats(self):
        live = {'vectors': [{'ParameterName': 'p_EmissiveColor',
                             'ParameterValue': {'R': 1, 'G': 0.5, 'B': 0, 'A': 1}}]}
        out = L.live_vectors(live)
        self.assertEqual(out, {'p_EmissiveColor': [1.0, 0.5, 0.0, 1.0]})
        self.assertTrue(all(isinstance(c, float) for c in out['p_EmissiveColor']))

    def test_ignores_incomplete_or_unnamed_entries(self):
        live = {'vectors': [
            {'ParameterName': 'Incomplete', 'ParameterValue': {'R': 1, 'G': 0}},
            {'ParameterValue': {'R': 1, 'G': 1, 'B': 1, 'A': 1}},
            {'ParameterName': 'NoValue'},
            {'ParameterName': 'p_Kept', 'ParameterValue': {'R': 0.1, 'G': 0.2, 'B': 0.3, 'A': 0.4}},
        ]}
        self.assertEqual(L.live_vectors(live), {'p_Kept': [0.1, 0.2, 0.3, 0.4]})

    def test_missing_part_or_vectors_gives_empty(self):
        self.assertEqual(L.live_vectors(None), {})
        self.assertEqual(L.live_vectors({}), {})
        self.assertEqual(L.live_vectors({'vectors': None}), {})


class BuildIndexTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.output = Path(self.temp.name)
        library = {'types': {'Pistol': {'mesh': '/Game/Synth/SK_Gestalt_Pistol.SK_Gestalt_Pistol', 'fragments': 2,
                                        'triangles': 6},
                             'Mystery': {'mesh': '/Game/Synth/SK_Gestalt_Mystery.SK_Gestalt_Mystery'}}}
        (self.output / 'library.json').write_text(json.dumps(library), encoding='utf-8')
        self.census = {
            'parts': {
                '/Game/Synth/PartA.PartA': {'fragments': ['FragA', 'FragB'], 'slot': 'Body', 'gestalt_type': 'Pistol',
                                            'material': 'Pkg.X.MIC_Red',
                                            'vector_params': [{'ParameterName': 'p_Tint', 'ParameterValue':
                                                               {'R': 0, 'G': 0, 'B': 0, 'A': 1}}]},
                '/Game/Synth/PartB.PartB': {'fragments': ['FragC'], 'slot': 'Barrel', 'gestalt_type': 'SMG',
                                            'material': None,
                                            'vector_params': [{'ParameterName': 'p_Glow', 'ParameterValue':
                                                               {'R': 0.2, 'G': 0.2, 'B': 0.2, 'A': 1}}]},
                '/Game/Synth/PartC.PartC': {'fragments': [], 'slot': 'Grip', 'gestalt_type': None, 'material': None},
                '/Game/Synth/PartD.PartD': {'fragments': ['FragD'], 'slot': 'Material',
                                            'gestalt_type': 'decode_error', 'material': None},
            },
            'skins': {'Pkg.X.MIC_Red': {'weapon_class': 'Pistol'}, 'Pkg.Y.MIC_Other': {'weapon_class': 'SMG'}},
            'weapon_types': {'/Game/Synth/Type_Pistol': {'bones_to_hide': ['None', 'Bone_Hip', '', 'Bone_Ear']},
                             '/Game/Synth/Type_Empty': {'bones_to_hide': ['None']}},
        }
        # PartA has a live vector on a differently-cased path; PartC has an empty live entry (no vectors at all).
        self.live = {'parts': {
            '/GAME/SYNTH/PARTA.PARTA': {'vectors': [{'ParameterName': 'p_EmissiveColor',
                                                     'ParameterValue': {'R': 1.0, 'G': 0.5, 'B': 0.25, 'A': 1.0}}]},
            '/Game/Synth/PartC.PartC': {'vectors': []},
        }}

    def tearDown(self):
        self.temp.cleanup()

    def test_types_get_gestalt_path_from_the_table_and_keep_their_fields(self):
        L.build_index(self.census, self.output, self.live)
        types = json.loads((self.output / 'library.json').read_text(encoding='utf-8'))['types']
        self.assertEqual(types['Pistol']['gestalt'], 'Weap_Pistol.GestaltDef_Pistol')
        self.assertEqual(types['Pistol']['triangles'], 6)
        self.assertEqual(types['Pistol']['mesh'], '/Game/Synth/SK_Gestalt_Pistol.SK_Gestalt_Pistol')
        self.assertIsNone(types['Mystery']['gestalt'])

    def test_live_vectors_beat_cooked_and_cooked_is_used_otherwise(self):
        index = L.build_index(self.census, self.output, self.live)
        parts = index['parts']
        self.assertEqual(parts['/Game/Synth/PartA.PartA']['vector_source'], 'live')
        self.assertEqual(parts['/Game/Synth/PartA.PartA']['vectors'],
                         {'p_EmissiveColor': [1.0, 0.5, 0.25, 1.0]})
        self.assertEqual(parts['/Game/Synth/PartB.PartB']['vector_source'], 'cooked')
        self.assertEqual(parts['/Game/Synth/PartB.PartB']['vectors'], self.census['parts']['/Game/Synth/PartB.PartB']['vector_params'])

    def test_part_without_any_vectors_has_no_vector_fields(self):
        parts = L.build_index(self.census, self.output, self.live)['parts']
        self.assertNotIn('vectors', parts['/Game/Synth/PartC.PartC'])
        self.assertNotIn('vector_source', parts['/Game/Synth/PartC.PartC'])

    def test_part_records_carry_fragments_slot_gestalt_and_skin(self):
        parts = L.build_index(self.census, self.output, self.live)['parts']
        self.assertEqual(parts['/Game/Synth/PartA.PartA'],
                         {'fragments': ['FragA', 'FragB'], 'slot': 'Body', 'gestalt': 'Pistol', 'skin': 'Pkg.X.MIC_Red',
                          'vectors': {'p_EmissiveColor': [1.0, 0.5, 0.25, 1.0]}, 'vector_source': 'live'})
        # Unknown gestalt types (decode_error) get no gestalt key; the census gestalt type itself is stored, not the path.
        self.assertNotIn('gestalt', parts['/Game/Synth/PartD.PartD'])
        self.assertEqual(parts['/Game/Synth/PartB.PartB']['gestalt'], 'SMG')

    def test_none_and_empty_bones_are_dropped(self):
        index = L.build_index(self.census, self.output, self.live)
        self.assertEqual(index['weapon_types'], {'/Game/Synth/Type_Pistol': {'bones_to_hide': ['Bone_Hip', 'Bone_Ear']},
                                                 '/Game/Synth/Type_Empty': {'bones_to_hide': []}})

    def test_skin_entries_carry_asset_prepared_and_imported_status(self):
        (self.output / 'skins.json').write_text(json.dumps({'Pkg.X.MIC_Red': {'status': 'prepared', 'asset': 'x'}}),
                                                encoding='utf-8')
        (self.output / 'import_report.json').write_text(
            json.dumps({'skins': {'Pkg.X.MIC_Red': {'status': 'imported'}}}), encoding='utf-8')
        index = L.build_index(self.census, self.output, self.live)
        self.assertEqual(index['skins']['Pkg.X.MIC_Red'],
                         {'asset': '/Game/OpenWillow/Weapons/Library/Skins/MI_MIC_Red.MI_MIC_Red',
                          'prepared': 'prepared', 'imported': 'imported'})
        self.assertEqual(index['skins']['Pkg.Y.MIC_Other'],
                         {'asset': '/Game/OpenWillow/Weapons/Library/Skins/MI_MIC_Other.MI_MIC_Other',
                          'prepared': None, 'imported': None})

    def test_skin_status_is_none_without_the_optional_reports(self):
        index = L.build_index(self.census, self.output, None)
        self.assertEqual(index['skins']['Pkg.X.MIC_Red']['prepared'], None)
        self.assertEqual(index['skins']['Pkg.X.MIC_Red']['imported'], None)
        # With no live census, PartA falls back to its cooked vectors.
        self.assertEqual(index['parts']['/Game/Synth/PartA.PartA']['vector_source'], 'cooked')

    def test_index_is_written_back_to_library_json(self):
        L.build_index(self.census, self.output, self.live)
        written = json.loads((self.output / 'library.json').read_text(encoding='utf-8'))
        self.assertEqual(set(written), {'types', 'parts', 'skins', 'weapon_types'})


class FragmentBoundsTests(unittest.TestCase):
    """to_ue maps glTF metres (Y up) to the UE component space (cm, Z up); each fragment's bounds cover only its vertices."""

    def test_to_ue_axes_and_scale(self):
        self.assertEqual(L.to_ue((0.01, 0.02, 0.03)), (1.0, 3.0, 2.0))

    def test_bounds_per_fragment_primitive(self):
        points = [(0.0, 0.0, 0.0), (0.1, 0.0, 0.0), (0.0, 0.2, 0.0), (0.0, 0.0, 0.5), (0.3, 0.3, 0.3)]
        positions = b''.join(struct.pack('<3f', *p) for p in points)
        pad = bytes(2)
        blob = bytearray(positions + struct.pack('<3H', 0, 1, 2) + pad + struct.pack('<3H', 0, 3, 4) + pad)
        gltf = {'bufferViews': [{'buffer': 0, 'byteOffset': 0, 'byteLength': len(positions)},
                                {'buffer': 0, 'byteOffset': len(positions), 'byteLength': 6},
                                {'buffer': 0, 'byteOffset': len(positions) + 8, 'byteLength': 6}],
                'accessors': [{'bufferView': 0, 'componentType': 5126, 'count': 5, 'type': 'VEC3'},
                              {'bufferView': 1, 'componentType': 5123, 'count': 3, 'type': 'SCALAR'},
                              {'bufferView': 2, 'componentType': 5123, 'count': 3, 'type': 'SCALAR'}],
                'meshes': [{'primitives': [{'attributes': {'POSITION': 0}, 'indices': 1, 'material': 0},
                                           {'attributes': {'POSITION': 0}, 'indices': 2, 'material': 1}]}],
                'materials': [{'name': 'FragA'}, {'name': 'FragB'}]}
        bounds = L.fragment_bounds(gltf, blob)
        self.assertEqual(bounds['FragA'], [0.0, 0.0, 0.0, 10.0, 0.0, 20.0])
        self.assertEqual(bounds['FragB'], [0.0, 0.0, 0.0, 30.0, 50.0, 30.0])


if __name__ == '__main__':
    unittest.main()
