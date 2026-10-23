"""Synthetic tests for tools/weapon_assembly_sample.py selection logic (no game data; names are invented)."""
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_assembly_sample as S


def gun(balance, weapon_type, gestalt, maker, parts=None, material='Mat.Alpha', fragments=None):
    return {'balance': balance, 'weapon_type': weapon_type, 'gestalt': gestalt, 'manufacturer': maker,
            'parts': parts or {}, 'material': material, 'gestalt_fragments': fragments or [],
            'game_stage': 30.0, 'name': 'x', 'stats': {'card': {'damage': 1.0}}}


PISTOL = 'Weap_Pistol.GestaltDef_Pistol'
SMG = 'Weap_SMG.GestaltDef_SMG'
NO_GESTALT = None


class NamingTests(unittest.TestCase):
    def test_gestalt_type_is_the_leaf_without_prefix(self):
        self.assertEqual(S.gestalt_type(PISTOL), 'Pistol')
        self.assertIsNone(S.gestalt_type(None))

    def test_make_id_is_lowercase_and_only_sample_characters(self):
        self.assertEqual(S.make_id('m', 'WT_Hyperion-SMG.v2'), 'm_wt_hyperion_smg_v2')

    def test_unique_id_suffixes_collisions_in_order(self):
        taken = {'u_a': 1, 'u_a_2': 1}
        self.assertEqual(S.unique_id('u', 'A', taken), 'u_a_3')
        self.assertEqual(S.unique_id('u', 'B', taken), 'u_b')


class MatrixTests(unittest.TestCase):
    def test_one_gun_per_type_and_manufacturer_and_non_legendary_preferred(self):
        rolled = [
            gun('GD.Legendary.Pistol_Alpha_Legendary', 'GD.A.WT_Alpha_Pistol', PISTOL, 'GD.Man.Alpha'),
            gun('GD.Plain.Pistol_Alpha_1', 'GD.A.WT_Alpha_Pistol', PISTOL, 'GD.Man.Alpha'),
            gun('GD.Plain.Pistol_Alpha_2', 'GD.A.WT_Alpha_Pistol', PISTOL, 'GD.Man.Alpha'),
        ]
        picked = S.select_matrix(rolled)
        self.assertEqual(list(picked), ['m_wt_alpha_pistol'])
        self.assertEqual(picked['m_wt_alpha_pistol'][0]['balance'], 'GD.Plain.Pistol_Alpha_1')

    def test_only_the_first_legendary_is_kept_when_no_plain_balance_exists(self):
        rolled = [gun('GD.Unique.Only', 'GD.A.WT_Solo_SMG', SMG, 'GD.Man.Beta'),
                  gun('GD.Etech.Other', 'GD.A.WT_Solo_SMG', SMG, 'GD.Man.Beta')]
        picked = S.select_matrix(rolled)
        self.assertEqual(picked['m_wt_solo_smg'][0]['balance'], 'GD.Unique.Only')

    def test_guns_without_a_weapon_gestalt_are_left_out(self):
        rolled = [gun('GD.Shield.One', 'GD.A.WT_Shield', NO_GESTALT, 'GD.Man.Alpha'),
                  gun('GD.Other.Gadget', 'GD.A.WT_Gadget', 'Weap_Gadget.GestaltDef_Gadget', 'GD.Man.Alpha')]
        self.assertEqual(S.select_matrix(rolled), {})

    def test_same_type_under_two_manufacturers_gets_two_ids(self):
        rolled = [gun('GD.P.A', 'GD.A.WT_Twin_Pistol', PISTOL, 'GD.Man.Alpha'),
                  gun('GD.P.B', 'GD.A.WT_Twin_Pistol', PISTOL, 'GD.Man.Beta')]
        picked = S.select_matrix(rolled)
        self.assertEqual(sorted(picked), ['m_wt_twin_pistol', 'm_wt_twin_pistol_2'])


class UniqueTests(unittest.TestCase):
    def test_markers_select_balances_and_non_weapon_ones_are_skipped(self):
        rolled = [gun('GD.Pearl.Gun_Pearl', 'GD.A.WT_Pearl', PISTOL, 'GD.Man.Alpha'),
                  gun('GD.Mission.Gun_Mission', 'GD.A.WT_Mission', NO_GESTALT, 'GD.Man.Alpha'),
                  gun('GD.Plain.Gun_Plain', 'GD.A.WT_Plain', PISTOL, 'GD.Man.Alpha')]
        picked, skipped = S.select_unique(rolled)
        self.assertEqual(list(picked), ['u_gun_pearl'])
        self.assertEqual(skipped, [{'balance': 'GD.Mission.Gun_Mission', 'reason': 'no weapon gestalt'}])


class SkinTests(unittest.TestCase):
    def test_skin_uses_a_verified_material_part_and_the_matching_class_base(self):
        skins = {'Common.Mats.Mati_AlphaRed': {'used_by_parts': ['GD.M.Mat_Alpha_1', 'GD.M.Mat_Other'],
                                               'weapon_class': 'Pistol', 'manufacturers': ['Alpha']}}
        census_parts = {'GD.M.Mat_Alpha_1': {'slot': 'Material'}, 'GD.M.Mat_Other': {'slot': 'Material'}}
        materials = {'GD.M.Mat_Alpha_1': 'Common.Mats.Mati_AlphaRed', 'GD.M.Mat_Other': 'Common.Mats.Mati_Else'}
        bases = {'m_wt_smg': gun('GD.P.A', 'GD.A.WT_SMG', SMG, 'GD.Man.Beta'),
                 'm_wt_alpha_pistol': gun('GD.P.B', 'GD.A.WT_Alpha_Pistol', PISTOL, 'GD.Man.Alpha')}
        picked, skipped = S.select_skins(skins, census_parts, set(), bases, materials.get)
        self.assertEqual(skipped, [])
        sid, (base_id, base, part, mic) = next(iter(picked.items()))
        self.assertEqual((sid, base_id, part, mic),
                         ('s_mati_alphared', 'm_wt_alpha_pistol', 'GD.M.Mat_Alpha_1', 'Common.Mats.Mati_AlphaRed'))

    def test_used_skins_and_unverified_or_classless_skins_are_skipped(self):
        skins = {'M.Used': {'used_by_parts': ['P.1'], 'weapon_class': 'Pistol'},
                 'M.Unverified': {'used_by_parts': ['P.2'], 'weapon_class': 'Pistol'},
                 'M.NoClass': {'used_by_parts': ['P.3'], 'weapon_class': 'unknown'}}
        census_parts = {'P.1': {'slot': 'Material'}, 'P.2': {'slot': 'Material'}, 'P.3': {'slot': 'Material'}}
        materials = {'P.2': 'M.Other', 'P.3': 'M.NoClass'}
        bases = {'m_wt_alpha_pistol': gun('GD.P.B', 'GD.A.WT_Alpha_Pistol', PISTOL, 'GD.Man.Alpha')}
        picked, skipped = S.select_skins(skins, census_parts, {'M.Used'}, bases, materials.get)
        self.assertEqual(picked, {})
        self.assertEqual(skipped, [{'skin': 'M.NoClass', 'reason': 'no matrix gun of class unknown'},
                                   {'skin': 'M.Unverified', 'reason': 'no Material part uses it'}])

    def test_a_non_material_part_is_not_used_for_a_skin(self):
        info = {'used_by_parts': ['P.Barrel', 'P.Mat']}
        census_parts = {'P.Barrel': {'slot': 'Barrel'}, 'P.Mat': {'slot': 'Material'}}
        self.assertEqual(S.verified_material_part(info, census_parts, 'M', lambda p: 'M'), 'P.Mat')

    def test_skin_recipe_replaces_material_part_and_copies_stats_without_touching_base(self):
        base = gun('GD.P.A', 'GD.A.WT_Alpha_Pistol', PISTOL, 'GD.Man.Alpha',
                   parts={'Body': {'part': 'P.Body', 'weight': 5.0, 'candidates': []},
                          'Material': {'part': 'P.OldMat', 'weight': 1.0, 'candidates': []}},
                   fragments=['Pistol_Body_A'])
        fragments = {'P.Body': ['Pistol_Body_A'], 'P.NewMat': ['Pistol_Mat_New']}
        recipe = S.skin_recipe('m_wt_alpha_pistol', base, 'P.NewMat', 'M.New', lambda p: fragments[p])
        self.assertEqual(recipe['parts']['Material']['part'], 'P.NewMat')
        self.assertEqual(recipe['parts']['Body']['part'], 'P.Body')
        self.assertEqual(recipe['material'], 'M.New')
        self.assertEqual(recipe['gestalt_fragments'], ['Pistol_Body_A', 'Pistol_Mat_New'])
        self.assertEqual(recipe['stats_source'], 'copied from m_wt_alpha_pistol')
        self.assertEqual(recipe['stats'], base['stats'])
        self.assertEqual(base['parts']['Material']['part'], 'P.OldMat')
        self.assertEqual(base['material'], 'Mat.Alpha')


class TriangleTests(unittest.TestCase):
    def test_every_range_of_a_fragment_is_summed(self):
        rows = [{'SkeletalMeshFragmentName': 'Pistol_Body_A', 'NumPrimitives': 100},
                {'SkeletalMeshFragmentName': 'Pistol_Body_A', 'NumPrimitives': 25},
                {'SkeletalMeshFragmentName': 'Pistol_Grip_B', 'NumPrimitives': 7}]
        self.assertEqual(S.expected_triangles(['Pistol_Body_A', 'Pistol_Grip_B'], rows), (132, []))

    def test_fragments_without_a_row_are_reported(self):
        rows = [{'SkeletalMeshFragmentName': 'Pistol_Body_A', 'NumPrimitives': 100}]
        self.assertEqual(S.expected_triangles(['Pistol_Body_A', 'Pistol_Ghost'], rows), (100, ['Pistol_Ghost']))

    def test_index_entry_carries_the_gun_fields(self):
        recipe = gun('GD.P.A', 'GD.A.WT_Alpha_Pistol', PISTOL, 'GD.Man.Alpha', fragments=['Pistol_Body_A'])
        entry = S.index_entry('matrix', recipe, [{'SkeletalMeshFragmentName': 'Pistol_Body_A', 'NumPrimitives': 3}])
        self.assertEqual(entry['expected_triangles'], 3)
        self.assertEqual((entry['set'], entry['fragments'], entry['unknown_fragments']), ('matrix', ['Pistol_Body_A'], []))


class TotalsTests(unittest.TestCase):
    def test_counts_matrix_coverage_and_skins(self):
        index = {
            'm_a': {'set': 'matrix', 'gestalt': PISTOL, 'manufacturer': 'GD.Man.Alpha'},
            'u_b': {'set': 'unique', 'gestalt': SMG, 'manufacturer': 'GD.Man.Beta'},
            's_c': {'set': 'skins', 'gestalt': PISTOL, 'manufacturer': 'GD.Man.Alpha'},
        }
        possible = {('Pistol', 'Alpha'), ('SMG', 'Beta')}
        out = S.totals(index, possible, {'M.1': 1, 'M.2': 1}, {'M.1'}, [{'balance': 'x'}], [])
        self.assertEqual(out['guns_per_set'], {'matrix': 1, 'unique': 1, 'skins': 1})
        self.assertEqual(out['matrix_coverage'], {'present': 1, 'possible': 2, 'missing': ['SMG x Beta']})
        self.assertEqual(out['skins'], {'covered': 1, 'total': 2})
        self.assertEqual(out['roll_failures'], 1)


class OutputTests(unittest.TestCase):
    def test_output_must_sit_inside_local(self):
        local = Path('/repo/local').resolve()
        self.assertTrue(S.output_under_local(local / 'wpa' / 'sample', local))
        self.assertFalse(S.output_under_local(Path('/repo/tools/out').resolve(), local))


if __name__ == '__main__':
    unittest.main()
