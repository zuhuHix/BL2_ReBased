"""Synthetic tests for tools/weapon_part_census.py (no game data; names are invented)."""
from pathlib import Path
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_part_census as C


FRAGMENT_SETS = {
    'Pistol': {'Pistol_Body_Alpha', 'Pistol_Body_Alpha_Var1', 'Pistol_Barrel_Plain'},
    'SMG': {'SMG_Body_Beta', 'SMG_Barrel_Plain', 'Shared_Sight'},
    'Launcher': {'Launcher_Body_Gamma', 'Shared_Sight'},
}


class FakePackage:
    """Stands in for weapon_recipe.Package: props() serves dicts and raises for bad names."""

    def __init__(self, objects):
        self.objects = objects
        self.extra_names = {}

    def props(self, path):
        value = self.objects[path]
        if isinstance(value, Exception):
            raise value
        return value


class SlotTests(unittest.TestCase):
    def test_part_type_wins_over_everything(self):
        self.assertEqual(C.slot_for('GD_Weap_X.Grip.X_Body_Foo', 'WP_Grip', 'Mat.Path'), ('Grip', 'PartType'))

    def test_material_when_no_part_type(self):
        self.assertEqual(C.slot_for('GD_Weap_X.ManufacturerMaterials.Mat_A_1', None, 'Pkg.Mats.Mati_A'),
                         ('Material', 'Material'))

    def test_second_path_component(self):
        self.assertEqual(C.slot_for('GD_Weap_X.elemental.X_Elemental_Fire', None, None), ('Elemental', 'path'))
        self.assertEqual(C.slot_for('GD_Weap_X.ManufacturerMaterials.Mat_A_1', None, None), ('Material', 'path'))

    def test_leaf_name_fallback(self):
        self.assertEqual(C.slot_for('GD_Weap_X.Misc.X_Stock_Plain', None, None), ('Stock', 'leaf_name'))

    def test_unknown(self):
        self.assertEqual(C.slot_for('GD_Weap_X.Misc.Plain', None, None), ('Unknown', 'none'))


class ClassAndGestaltTests(unittest.TestCase):
    def test_weapon_class_from_path_words(self):
        self.assertEqual(C.weapon_class_for('Pkg.Materials.SMG.Mati_AlphaUncommonSMG'), 'SMG')
        self.assertEqual(C.weapon_class_for('Weap_Launchers.Something'), 'Launcher')
        self.assertEqual(C.weapon_class_for('Pkg.Materials.AssaultRifle.Mati_X'), 'AssaultRifle')
        self.assertEqual(C.weapon_class_for('Pkg.Materials.sniper.Mati_Y_Sniper'), 'SniperRifle')
        self.assertIsNone(C.weapon_class_for('Pkg.Misc.Plain_Thing'))

    def test_gestalt_single_ambiguous_unresolved(self):
        self.assertEqual(C.gestalt_for(['Pistol_Body_Alpha'], FRAGMENT_SETS), ('Pistol', 'fragments'))
        self.assertEqual(C.gestalt_for(['Shared_Sight'], FRAGMENT_SETS), ('ambiguous', None))
        self.assertEqual(C.gestalt_for(['SMG_Body_Beta', 'Pistol_Barrel_Plain'], FRAGMENT_SETS), ('unresolved', None))
        self.assertEqual(C.gestalt_for([], FRAGMENT_SETS), (None, None))

    def test_path_class_breaks_tie_between_candidates(self):
        self.assertEqual(C.gestalt_for(['Shared_Sight'], FRAGMENT_SETS, 'GD_Weap_SMG.elemental.SMG_Elemental_Fire'),
                         ('SMG', 'fragments+path'))
        self.assertEqual(C.gestalt_for(['Shared_Sight'], FRAGMENT_SETS, 'GD_Invented_Launchers.Sight.Sight_Plain'),
                         ('Launcher', 'fragments+path'))

    def test_path_tie_break_only_among_candidates(self):
        # the AR word names a class that is not a candidate, and two candidates named together stay ambiguous
        self.assertEqual(C.gestalt_for(['Shared_Sight'], FRAGMENT_SETS, 'GD_Weap_AssaultRifle.Accessory.AR_Accessory_X'),
                         ('ambiguous', None))
        self.assertEqual(C.gestalt_for(['Shared_Sight'], FRAGMENT_SETS, 'GD_Invented.SMG_Launcher.Sight_X'),
                         ('ambiguous', None))
        self.assertEqual(C.gestalt_for(['Shared_Sight'], FRAGMENT_SETS, 'GD_Invented.Misc.Plain'), ('ambiguous', None))

    def test_path_classes_words(self):
        self.assertEqual(C.path_classes('GD_Weap_Pistol.elemental.Pistol_Elemental_Fire'), {'Pistol'})
        self.assertEqual(C.path_classes('GD_Aster_Weapons.Pistols.Pistol_Plain'), {'Pistol'})
        self.assertEqual(C.path_classes('GD_Invented.Accessory.AR_Accessory_Bayonet_1'), {'AssaultRifle'})
        self.assertEqual(C.path_classes('GD_Invented.Snipers.Sniper_Accessory_Bayonet1'), {'SniperRifle'})
        self.assertEqual(C.path_classes('GD_Invented.Misc.Plain'), set())

    def test_resolve_parts_records_source(self):
        parts = {'GD_Weap_SMG.elemental.SMG_Elemental_Fire': {'fragments': ['Shared_Sight'], 'decode_error': None}}
        C.resolve_parts(parts, FRAGMENT_SETS)
        self.assertEqual(parts['GD_Weap_SMG.elemental.SMG_Elemental_Fire']['gestalt_type'], 'SMG')
        self.assertEqual(parts['GD_Weap_SMG.elemental.SMG_Elemental_Fire']['gestalt_source'], 'fragments+path')

    def test_unresolved_fragments_are_unknown_to_every_gestalt(self):
        self.assertEqual(C.unresolved_fragments(['SMG_Body_Beta', 'Nope_Part'], FRAGMENT_SETS), ['Nope_Part'])

    def test_resolve_parts_marks_decode_errors(self):
        parts = {'A.Body.a': {'fragments': ['SMG_Body_Beta'], 'decode_error': None},
                 'A.Body.b': {'fragments': [], 'decode_error': 'boom'}}
        C.resolve_parts(parts, FRAGMENT_SETS)
        self.assertEqual(parts['A.Body.a']['gestalt_type'], 'SMG')
        self.assertEqual(parts['A.Body.a']['gestalt_source'], 'fragments')
        self.assertEqual(parts['A.Body.a']['unresolved_fragments'], [])
        self.assertEqual(parts['A.Body.b']['gestalt_type'], 'decode_error')
        self.assertIsNone(parts['A.Body.b']['gestalt_source'])


class GestaltReportTests(unittest.TestCase):
    def test_duplicate_names_sum_and_keep_ranges(self):
        rows = [{'SkeletalMeshFragmentName': 'Acc_X', 'MaterialIndex': 0, 'FirstIndex': 0, 'NumPrimitives': 4},
                {'SkeletalMeshFragmentName': 'Acc_X', 'MaterialIndex': 1, 'FirstIndex': 12, 'NumPrimitives': 2},
                {'SkeletalMeshFragmentName': 'Acc_Y', 'MaterialIndex': 0, 'FirstIndex': 36, 'NumPrimitives': 1}]
        report = C.gestalt_report(rows)
        self.assertEqual(report['Acc_X']['triangles'], 6)
        self.assertEqual(report['Acc_X']['ranges'], [{'material': 0, 'first_index': 0, 'triangles': 4},
                                                    {'material': 1, 'first_index': 12, 'triangles': 2}])
        self.assertEqual(report['Acc_Y']['triangles'], 1)

    def test_referenced_and_unreferenced_fragments(self):
        rows = [{'SkeletalMeshFragmentName': 'SMG_Body_Beta', 'MaterialIndex': 0, 'FirstIndex': 0, 'NumPrimitives': 1},
                {'SkeletalMeshFragmentName': 'SMG_Barrel_Plain', 'MaterialIndex': 0, 'FirstIndex': 3, 'NumPrimitives': 2}]
        parts = {'p1': {'fragments': ['SMG_Body_Beta']}, 'p2': {'fragments': ['SMG_Body_Beta']}}
        entry = C.gestalt_entries({'SMG': ('Weap_SMG.GestaltDef_SMG', rows)}, parts)['SMG']
        self.assertEqual(entry['triangles_total'], 3)
        self.assertEqual(entry['referenced_by_parts'], {'SMG_Body_Beta': 2, 'SMG_Barrel_Plain': 0})
        self.assertEqual(entry['unreferenced_fragments'], ['SMG_Barrel_Plain'])


class PartDecodeTests(unittest.TestCase):
    def test_vector_params_and_flags(self):
        path = 'GD_Weap_X.elemental.X_Elemental_Fire'
        package = FakePackage({path: {
            'PartType': 'WP_Elemental',
            'MaterialVectorParameterValues': [{'ParameterName': 'p_EmissiveColor',
                                               'ParameterValue': {'R': 1.0, 'G': 0.5, 'B': 0, 'A': 1}}],
            'FirstPersonParticle': 'GD_Fx.Particle.Fx_1',
            'ManufacturerDefinitionForScopeMovie': 'GD_Manufacturers.Hyperion',
            'GestaltModeSkeletalMeshName': 'Acc_Elemental_Fire',
        }})
        decoded = C.decode_part(package, path)
        self.assertIsNone(decoded['decode_error'])
        self.assertEqual(decoded['vector_params'], {'p_EmissiveColor': [1.0, 0.5, 0, 1]})
        self.assertTrue(decoded['first_person_particle'])
        self.assertEqual(decoded['manufacturer'], 'Hyperion')
        entry = C.part_entry(path, 'Startup.upk', ['Startup.upk'], decoded)
        self.assertEqual(entry['slot'], 'Elemental')
        self.assertFalse(entry['draws_nothing_rule'])

    def test_decode_error_is_recorded_not_raised(self):
        path = 'GD_Weap_X.Body.X_Body_Bad'
        package = FakePackage({path: RuntimeError("'str' object has no attribute 'name'")})
        decoded = C.decode_part(package, path)
        self.assertIn("'str' object has no attribute 'name'", decoded['decode_error'])
        self.assertEqual(decoded['fragments'], [])

    def test_none_part_draws_nothing(self):
        path = 'GD_Weap_X.Sight.X_Sight_None'
        package = FakePackage({path: {'GestaltModeSkeletalMeshName': 'Shared_Sight'}})
        decoded = C.decode_part(package, path)
        self.assertEqual(decoded['fragments'], [])
        self.assertTrue(C.part_entry(path, 'p', ['p'], decoded)['draws_nothing_rule'])

    def test_weapon_type_bones_and_gestalt_field(self):
        package = FakePackage({'GD_Weap_X.Type.Type_X': {
            'BoneToHideOnMesh': ['Bone_A', 'None'],
            'AdditionalBoneToHideOnMesh': 'Bone_B',
            'GestaltMesh': 'Weap_SMG.GestaltDef_SMG',
        }})
        record = C.weapon_type_record(package, 'GD_Weap_X.Type.Type_X')
        self.assertEqual(record['bones_to_hide'], ['Bone_A', 'Bone_B'])
        self.assertEqual(record['gestalt_field'], {'field': 'GestaltMesh', 'value': 'Weap_SMG.GestaltDef_SMG'})


class SkinAndMatrixTests(unittest.TestCase):
    def setUp(self):
        self.parts = {
            'GD_Weap_SMG.Body.SMG_Body_Beta_Var1': {'fragments': ['SMG_Body_Beta_Var1'], 'slot': 'Body',
                                                    'gestalt_type': 'SMG', 'manufacturer': None, 'material': None},
            'GD_Weap_SMG.Body.SMG_Body_Gamma': {'fragments': ['SMG_Body_Gamma'], 'slot': 'Body',
                                                'gestalt_type': 'SMG', 'manufacturer': 'Gamma', 'material': None},
            'GD_Weap_SMG.ManufacturerMaterials.Mat_Beta_1': {'fragments': [], 'slot': 'Material',
                                                             'gestalt_type': None, 'manufacturer': 'Beta',
                                                             'material': 'Pkg.Materials.SMG.Mati_BetaSMG'},
            'GD_Weap_SMG.ManufacturerMaterials.Mat_Beta_2': {'fragments': [], 'slot': 'Material',
                                                             'gestalt_type': None, 'manufacturer': 'Beta',
                                                             'material': 'Pkg.Materials.SMG.Mati_BetaSMG'},
        }
        self.skins = C.build_skins(self.parts, None)

    def test_matrix_manufacturer_prefers_body_fragment(self):
        self.assertEqual(C.matrix_manufacturer(self.parts['GD_Weap_SMG.Body.SMG_Body_Beta_Var1']), 'Beta')
        self.assertEqual(C.matrix_manufacturer(self.parts['GD_Weap_SMG.Body.SMG_Body_Gamma']), 'Gamma')
        self.assertEqual(C.matrix_manufacturer({'fragments': [], 'manufacturer': None}), 'Unknown')

    def test_skins_group_parts_and_class(self):
        skin = self.skins['Pkg.Materials.SMG.Mati_BetaSMG']
        self.assertEqual(skin['used_by_parts'], ['GD_Weap_SMG.ManufacturerMaterials.Mat_Beta_1',
                                                 'GD_Weap_SMG.ManufacturerMaterials.Mat_Beta_2'])
        self.assertEqual(skin['weapon_class'], 'SMG')
        self.assertIsNone(skin['resolvable'])

    def test_matrix_counts(self):
        matrix = C.build_matrix(self.parts, self.skins)
        self.assertEqual(matrix['SMG']['Beta'], {'body_parts': 1, 'skins': 1})
        self.assertEqual(matrix['SMG']['Gamma'], {'body_parts': 1, 'skins': 0})

    def test_skin_resolvable_against_roots(self):
        with tempfile.TemporaryDirectory() as root:
            folder = Path(root) / 'Materials' / 'MaterialInstanceConstant'
            folder.mkdir(parents=True)
            (folder / 'Mati_BetaSMG.props.txt').write_text('', encoding='utf-8')
            skins = C.build_skins(self.parts, [Path(root)])
        self.assertTrue(skins['Pkg.Materials.SMG.Mati_BetaSMG']['resolvable'])
        self.assertFalse(C.build_skins(self.parts, [Path(root) / 'missing'])['Pkg.Materials.SMG.Mati_BetaSMG']['resolvable'])


class NameParseTests(unittest.TestCase):
    def test_material_leaf(self):
        self.assertEqual(C.material_name('Mat_BanditMade_1'), ('Bandit', '1'))
        self.assertEqual(C.material_name('Mat_Hyperion_4_Diamond_SG'), ('Hyperion', '4'))
        self.assertEqual(C.material_name('Mat_Invented_3_Named'), ('Invented', '3'))
        self.assertEqual(C.material_name('BuzzAxe_BaseMat'), (None, None))

    def test_mic_leaf(self):
        self.assertEqual(C.mic_name('Mati_HyperionUncommonSMG'), ('Hyperion', 'Uncommon'))
        self.assertEqual(C.mic_name('MasterMati_DahlEpic'), ('Dahl', 'Epic'))
        self.assertEqual(C.mic_name('Mati_Dahl_Emerald_AR'), ('Dahl', 'Emerald'))
        self.assertEqual(C.mic_name('Mati_BanditSG_3_Dog'), ('Bandit', None))
        self.assertEqual(C.mic_name('BuzzAxe_BaseMat'), (None, None))

    def test_skin_manufacturers_and_rarity_from_names(self):
        parts = {'GD_Invented_SMG.ManufacturerMaterials.Mat_Zeta_2': {
            'fragments': [], 'slot': 'Material', 'gestalt_type': None, 'manufacturer': None,
            'material': 'Pkg.Materials.SMG.Mati_ZetaUncommonSMG'}}
        skin = C.build_skins(parts, None)['Pkg.Materials.SMG.Mati_ZetaUncommonSMG']
        self.assertEqual(skin['manufacturers'], ['Zeta'])
        self.assertEqual(skin['rarity_tokens'], ['Uncommon', 'index:2'])
        matrix = C.build_matrix(parts, {'Pkg.Materials.SMG.Mati_ZetaUncommonSMG': dict(skin, weapon_class='SMG')})
        self.assertEqual(matrix, {'SMG': {'Zeta': {'body_parts': 0, 'skins': 1}}})
        self.assertEqual(C.build_summary({}, {'m': {'weapon_class': 'SMG', 'resolvable': None, 'manufacturers': []}},
                                         {}, 0, 0)['skins_per_manufacturer'], {'Unknown': 1})


class LiveCompareTests(unittest.TestCase):
    def setUp(self):
        rows = [{'SkeletalMeshFragmentName': 'SMG_Body_Beta', 'MaterialIndex': 0, 'FirstIndex': 0, 'NumPrimitives': 1},
                {'SkeletalMeshFragmentName': 'SMG_Barrel_Plain', 'MaterialIndex': 0, 'FirstIndex': 3, 'NumPrimitives': 2}]
        self.tables = {'SMG': ('Weap_SMG.GestaltDef_SMG', rows)}
        self.parts = {
            'GD_Weap_SMG.Body.SMG_Body_Beta': {'fragments': ['SMG_Body_Beta'], 'material': None, 'vector_params': {}},
            'GD_Weap_SMG.Barrel.SMG_Barrel_Plain': {
                'fragments': ['SMG_Barrel_Plain'], 'material': 'Pkg.Materials.SMG.Mati_ZetaUncommonSMG',
                'vector_params': {'p_EmissiveColor': [1.0, 0.5, 0.0, 1.0]}},
            'GD_Weap_SMG.Sight.SMG_Sight_None': {'fragments': [], 'material': None, 'vector_params': {}},
            'GD_Weap_SMG.Grip.SMG_Grip_Cooked': {'fragments': ['SMG_Grip_X'], 'material': None, 'vector_params': {}},
            'GD_Weap_SMG.Grip.Gun_Plain': {'fragments': [], 'material': None, 'vector_params': {}},
        }
        self.skins = {'Pkg.Materials.SMG.Mati_ZetaUncommonSMG': {}, 'Pkg.Materials.SMG.Mati_CookedOnly': {}}
        self.types = {'GD_Weap_SMG.Type.Type_SMG': {'bones_to_hide': ['Bone_A']},
                      'GD_Weap_SMG.Type.Type_Other': {'bones_to_hide': []}}
        self.live = {
            'gestalts': {'weap_smg.gestaltdef_smg': {'lods': [[
                {'name': 'SMG_Body_Beta', 'material': 0, 'first_index': 0, 'triangles': 1},
                {'name': 'SMG_Barrel_Plain', 'material': 0, 'first_index': 3, 'triangles': 2}]]}},
            'parts': {
                'GD_Weap_SMG.Body.SMG_Body_Beta': {'gestalt': 'SMG_Body_Beta', 'additional': ['None', 'None'],
                                                   'material': None, 'vectors': []},
                'GD_Weap_SMG.Barrel.SMG_Barrel_Plain': {
                    'gestalt': 'SMG_Barrel_Plain', 'additional': ['None'],
                    'material': 'pkg.materials.smg.mati_zetauncommonsmg',
                    'vectors': [{'ParameterName': 'p_EmissiveColor',
                                 'ParameterValue': {'R': 1.0, 'G': 0.50001, 'B': 0, 'A': 1}}]},
                'GD_Weap_SMG.Sight.SMG_Sight_None': {'gestalt': 'Shared_Sight', 'additional': ['None'],
                                                     'material': None, 'vectors': []},
                'GD_Weap_SMG.Grip.SMG_Grip_Cooked': {'gestalt': 'SMG_Grip_Y', 'additional': ['None'],
                                                     'material': None, 'vectors': []},
                'GD_Weap_SMG.Grip.Gun_Plain': {'gestalt': 'None', 'additional': ['None'],
                                                 'material': None, 'vectors': []},
                'GD_Weap_SMG.Accessory.Live_Only': {'gestalt': 'SMG_Acc_Live', 'additional': ['None'],
                                                    'material': None, 'vectors': []},
            },
            'mics': {
                'Pkg.Materials.SMG.Mati_ZetaUncommonSMG': [
                    {'path': 'Pkg.Materials.SMG.Mati_ZetaUncommonSMG', 'class': 'MaterialInstanceConstant'},
                    {'path': 'Pkg.Master.Master_Gun', 'class': 'Material'}],
                'Pkg.Materials.SMG.Mati_LiveOnly': [{'path': 'Pkg.Materials.SMG.Mati_LiveOnly', 'class': 'M'}],
            },
            'weapon_types': {
                'gd_weap_smg.type.type_smg': {'bones_to_hide': [['Bone_A', 'None'], None]},
                'GD_Weap_SMG.Type.Type_Other': {'bones_to_hide': [None, ['Bone_B']]},
            },
        }

    def compare(self, live=None):
        return C.live_compare(live or self.live, self.tables, self.parts, self.skins, self.types)

    def test_counts_and_mismatches(self):
        report = self.compare()
        self.assertTrue(report['gestalts']['SMG']['equal'])
        self.assertEqual((report['parts']['in_both'], report['parts']['cooked_only'], report['parts']['live_only']),
                         (5, 0, 1))
        self.assertEqual(report['parts']['fragments_differ'], 1)
        self.assertEqual(report['parts']['material_differ'], 0)  # case-insensitive path match
        self.assertEqual(report['parts']['vectors_differ'], 0)  # 0.50001 is within 1e-4 of 0.5
        self.assertEqual((report['skins']['in_both'], report['skins']['cooked_only'], report['skins']['live_only']),
                         (1, 1, 1))
        self.assertEqual(report['skins']['parent_chains']['Pkg.Materials.SMG.Mati_ZetaUncommonSMG'],
                         ['Pkg.Materials.SMG.Mati_ZetaUncommonSMG', 'Pkg.Master.Master_Gun'])
        self.assertEqual(report['weapon_types']['bones_equal'], 1)
        self.assertEqual(report['weapon_types']['bones_differ'], 1)
        kinds = sorted(m['kind'] for m in report['mismatches'])
        self.assertEqual(kinds, ['bones_to_hide', 'fragments'])

    def test_gestalt_table_and_vector_mismatches(self):
        self.live['gestalts']['weap_smg.gestaltdef_smg']['lods'][0][1]['triangles'] = 3
        self.live['parts']['GD_Weap_SMG.Barrel.SMG_Barrel_Plain']['vectors'][0]['ParameterValue']['G'] = 0.6
        report = self.compare()
        self.assertFalse(report['gestalts']['SMG']['equal'])
        self.assertEqual(report['gestalts']['SMG']['first_difference']['index'], 1)
        self.assertEqual(report['parts']['vectors_differ'], 1)
        self.assertIn({'kind': 'vectors', 'path': 'GD_Weap_SMG.Barrel.SMG_Barrel_Plain', 'names': ['p_EmissiveColor']},
                      report['mismatches'])

    def test_live_fragments_follow_the_none_rules(self):
        self.assertEqual(C.live_fragments('GD_Weap_SMG.Sight.SMG_Sight_None',
                                          {'gestalt': 'Shared_Sight', 'additional': ['None']}), [])
        self.assertEqual(C.live_fragments('GD_Weap_SMG.Body.SMG_Body_Beta',
                                          {'gestalt': 'SMG_Body_Beta', 'additional': ['SMG_Body_Beta_Var1', 'None']}),
                         ['SMG_Body_Beta', 'SMG_Body_Beta_Var1'])

    def test_first_difference_pads_short_lists(self):
        self.assertIsNone(C.first_difference([1, 2], [1, 2]))
        self.assertEqual(C.first_difference([1, 2], [1]), {'index': 1, 'cooked': 2, 'live': None})

    def test_vectors_differing(self):
        self.assertEqual(C.vectors_differing({'a': [1.0, None, 0, 1]}, {'a': [1.00005, None, 0, 1]}), [])
        self.assertEqual(C.vectors_differing({'a': [1.0, 0, 0, 1]}, {'a': [1.0, 0.1, 0, 1]}), ['a'])
        self.assertEqual(C.vectors_differing({'a': [0, 0, 0, 0]}, {}), ['a'])


class SummaryTests(unittest.TestCase):
    def test_summary_counts(self):
        parts = {'a': {'slot': 'Body', 'gestalt_type': 'SMG', 'unresolved_fragments': []},
                 'b': {'slot': 'Material', 'gestalt_type': None, 'unresolved_fragments': ['X']},
                 'c': {'slot': 'Body', 'gestalt_type': 'decode_error', 'unresolved_fragments': []}}
        skins = {'m1': {'weapon_class': 'SMG', 'resolvable': True}, 'm2': {'weapon_class': None, 'resolvable': False}}
        gestalts = {'SMG': {'unreferenced_fragments': ['Old_Frag']}}
        summary = C.build_summary(parts, skins, gestalts, decode_errors=1, package_errors=0)
        self.assertEqual(summary['parts'], 3)
        self.assertEqual(summary['parts_per_slot'], {'Body': 2, 'Material': 1})
        self.assertEqual(summary['parts_per_gestalt_type'], {'SMG': 1, 'decode_error': 1, 'none': 1})
        self.assertEqual(summary['parts_with_unresolved_fragments'], ['b'])
        self.assertEqual(summary['fragments_never_referenced'], {'SMG': ['Old_Frag']})
        self.assertEqual(summary['skins_per_weapon_class'], {'SMG': 1, 'unknown': 1})
        self.assertEqual((summary['skins_resolvable'], summary['skins_unresolvable']), (1, 1))
        self.assertEqual(summary['decode_errors'], 1)

    def test_summary_without_materials_leaves_resolvable_unset(self):
        skins = {'m1': {'weapon_class': 'SMG', 'resolvable': None}}
        summary = C.build_summary({}, skins, {}, decode_errors=0, package_errors=0)
        self.assertIsNone(summary['skins_resolvable'])
        self.assertIsNone(summary['skins_unresolvable'])


class OutputGuardTests(unittest.TestCase):
    def test_output_must_be_under_local(self):
        local = (Path(__file__).resolve().parents[1] / 'local').resolve()
        self.assertTrue(C.output_under_local(local / 'wpa' / 'census' / 'visual_census.json', local))
        self.assertFalse(C.output_under_local(local.parent / 'visual_census.json', local))


if __name__ == '__main__':
    unittest.main()
