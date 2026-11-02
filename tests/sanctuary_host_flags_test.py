"""Synthetic guards for the Sanctuary host profile and opt-in flags; no package assets used."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'host' / 'ue5'))
from scene_import_rules import (SANCTUARY_OPT_IN_ENV, SANCTUARY_OPT_IN_NAMES,
                                SANCTUARY_XMAS_BULB_METHOD,
                                effective_scene_recipe, host_sky_cloud_opacity,
                                sanctuary_opt_ins, sanctuary_profile,
                                sanctuary_source_lighting, sanctuary_verifier_checks)

SANCTUARY_SCENE = {'map': 'Sanctuary_P', 'lighting': {'sky_light': {'light_brightness': 1.0}}}
OTHER_SCENE = {'map': 'Other_P', 'lighting': {'sky_light': {'light_brightness': 1.0}}}
OTHER_SCENE_NO_LIGHTING = {'map': 'Other_P'}
SANCTUARY_SCENE_NO_LIGHTING = {'map': 'Sanctuary_P'}
XMAS_DEFINITION = {'scene_recipe': {'method': SANCTUARY_XMAS_BULB_METHOD,
                                    'emissive_layers': [{'texture': 'Pkg.Tex.Bulbs', 'channel': 'RGB'}]}}


class SanctuaryHostFlagsTest(unittest.TestCase):
    def test_environment_name_and_flag_set_are_fixed(self):
        self.assertEqual(SANCTUARY_OPT_IN_ENV, 'OPENWILLOW_SANCTUARY_OPT_IN')
        self.assertEqual(SANCTUARY_OPT_IN_NAMES, frozenset(
            ('grade', 'diffuse_tint', 'xmas_bulbs', 'static_lights', 'graffiti_decals')))

    def test_default_empty_environment_has_no_opt_ins(self):
        for value in (None, '', '   ', ' , ,'):
            self.assertEqual(sanctuary_opt_ins(value, SANCTUARY_SCENE), frozenset(), repr(value))

    def test_each_name_parses_case_insensitively(self):
        for name in sorted(SANCTUARY_OPT_IN_NAMES):
            self.assertEqual(sanctuary_opt_ins(name.upper(), SANCTUARY_SCENE), frozenset((name,)))
            self.assertEqual(sanctuary_opt_ins('  ' + name.title() + ' ', SANCTUARY_SCENE),
                             frozenset((name,)))

    def test_comma_list_ignores_whitespace_empty_items_and_duplicates(self):
        self.assertEqual(sanctuary_opt_ins('Grade, DIFFUSE_TINT ,grade,', SANCTUARY_SCENE),
                         frozenset(('grade', 'diffuse_tint')))

    def test_unknown_name_is_rejected_with_the_name_in_the_message(self):
        with self.assertRaises(ValueError) as caught:
            sanctuary_opt_ins('grade,gradee', SANCTUARY_SCENE)
        self.assertIn("'gradee'", str(caught.exception))
        self.assertIn(SANCTUARY_OPT_IN_ENV, str(caught.exception))

    def test_unknown_name_is_rejected_even_on_other_maps(self):
        with self.assertRaises(ValueError):
            sanctuary_opt_ins('not_a_flag', OTHER_SCENE)

    def test_sanctuary_profile_is_only_the_sanctuary_p_map(self):
        self.assertTrue(sanctuary_profile(SANCTUARY_SCENE))
        self.assertFalse(sanctuary_profile(OTHER_SCENE))
        self.assertFalse(sanctuary_profile({}))
        self.assertFalse(sanctuary_profile({'map': 'sanctuary_p'}))

    def test_non_sanctuary_map_ignores_opt_ins_and_source_lighting(self):
        self.assertEqual(sanctuary_opt_ins('grade,graffiti_decals,static_lights', OTHER_SCENE),
                         frozenset())
        self.assertEqual(sanctuary_source_lighting(OTHER_SCENE), {})
        self.assertEqual(sanctuary_source_lighting(SANCTUARY_SCENE), SANCTUARY_SCENE['lighting'])

    def test_verifier_runs_sanctuary_checks_only_for_sanctuary_p_with_lighting(self):
        self.assertTrue(sanctuary_verifier_checks(SANCTUARY_SCENE))
        self.assertFalse(sanctuary_verifier_checks(OTHER_SCENE))
        self.assertFalse(sanctuary_verifier_checks(OTHER_SCENE_NO_LIGHTING))
        self.assertFalse(sanctuary_verifier_checks({}))

    def test_other_map_without_lighting_has_no_source_lighting_for_import_or_verify(self):
        self.assertEqual(sanctuary_source_lighting(OTHER_SCENE_NO_LIGHTING), {})
        self.assertEqual(sanctuary_opt_ins('static_lights,graffiti_decals', OTHER_SCENE_NO_LIGHTING),
                         frozenset())

    def test_sanctuary_p_without_lighting_runs_checks_but_has_no_source_lighting(self):
        # Edge case: the Sanctuary checks stay on, but the importer and the verifier both
        # see an empty source-lighting record, so no selection policy is asserted.
        self.assertTrue(sanctuary_verifier_checks(SANCTUARY_SCENE_NO_LIGHTING))
        self.assertEqual(sanctuary_source_lighting(SANCTUARY_SCENE_NO_LIGHTING), {})

    def test_non_sanctuary_map_keeps_the_raw_cloud_cap(self):
        self.assertEqual(host_sky_cloud_opacity(OTHER_SCENE, 2.0), 2.0)
        self.assertAlmostEqual(host_sky_cloud_opacity(SANCTUARY_SCENE, 2.0), 0.28)

    def test_bulb_recipe_is_dropped_unless_xmas_bulbs_is_opted_in(self):
        self.assertEqual(effective_scene_recipe(XMAS_DEFINITION, frozenset()), {})
        self.assertEqual(effective_scene_recipe(XMAS_DEFINITION, frozenset(('grade',))), {})
        self.assertEqual(effective_scene_recipe(XMAS_DEFINITION, frozenset(('xmas_bulbs',))),
                         XMAS_DEFINITION['scene_recipe'])

    def test_other_recipes_are_not_affected_by_the_bulb_flag(self):
        definition = {'scene_recipe': {'method': 'world_map_mask_colorization_v2'}}
        self.assertEqual(effective_scene_recipe(definition, frozenset()), definition['scene_recipe'])
        self.assertEqual(effective_scene_recipe({}, frozenset()), {})

    def test_xmas_bulb_recipe_is_dropped_on_other_maps_even_if_requested(self):
        opt_ins = sanctuary_opt_ins('xmas_bulbs', OTHER_SCENE)
        self.assertEqual(effective_scene_recipe(XMAS_DEFINITION, opt_ins), {})


if __name__ == '__main__':
    unittest.main()
