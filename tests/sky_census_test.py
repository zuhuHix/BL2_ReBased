"""Sky census classification and policy restatement on synthetic records."""
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from sky_census import SkyCensus, diffuse_policy, sky_named


def tags(**kwargs):
    return [{'name': k, 'value': v, 'status': 'decoded'} for k, v in kwargs.items()]


class SkyCensusTest(unittest.TestCase):
    def test_sky_named_uses_package_prefix_or_object_name_only(self):
        self.assertTrue(sky_named('Prop_Skybox.MoveMe.SlateGravel_Dif'))
        self.assertTrue(sky_named('FX_ENV_Sanctuary.Meshes.Sanc_Building01_Skybox'))
        self.assertTrue(sky_named('Common_Materials.Sky.Mat_SkyTimeOfDay_Master'))
        self.assertFalse(sky_named('Common_Materials.Sky.Master_ArauraBorealis'))
        self.assertFalse(sky_named('Prop_Glacier.Materials.Mat_MountainDistant'))

    def test_policy_restatement_matches_material_v1_precedence(self):
        identity = lambda key: key
        texture_class = lambda key: 'Texture2D'
        # p_diffuse beats an alias; a stub loses to a concrete alias.
        named = diffuse_policy({'p_snowdiffuse': 'Snow_Dif', 'p_diffuse': 'Wall_Dif'},
                               None, identity, texture_class, 'BLEND_Opaque', None)
        self.assertEqual((named['selected'], named['parameter']), ('Wall_Dif', 'p_diffuse'))
        stub = diffuse_policy({'p_diffuse': 'Common_Textures.Stub.StubGray_Gray',
                               'p_snowdiffuse': 'Snow_Dif'},
                              None, identity, texture_class, 'BLEND_Opaque', None)
        self.assertEqual(stub['selected'], 'Snow_Dif')
        null = diffuse_policy({'p_diffuse': None}, ['Sky_Dif'], identity, texture_class,
                              'BLEND_Opaque', None)
        self.assertIsNone(null['selected'])
        self.assertIn('explicitly null', null['reason'])
        unique = diffuse_policy({}, ['Shapes_Gray', 'Sky_Dif'], identity, texture_class,
                                'BLEND_Masked', 'Sky_Dif')
        self.assertEqual((unique['selected'], unique['method']),
                         ('Sky_Dif', 'sole_cooked_resource_dif_texture'))
        ambiguous = diffuse_policy({}, ['Sky_Dif', 'Smoke_Dif'], identity, texture_class,
                                   'BLEND_Masked', 'Sky_Dif')
        self.assertIsNone(ambiguous['selected'])
        self.assertIn('sky-named _Dif texture is among the candidates', ambiguous['reason'])
        self.assertEqual(ambiguous['dif_candidates'], ['Sky_Dif', 'Smoke_Dif'])
        translucent = diffuse_policy({}, ['Cloud_Gray'], identity, texture_class,
                                     'BLEND_Translucent', None)
        self.assertIn('non-opaque', translucent['reason'])
        self.assertEqual(diffuse_policy({}, None, identity, texture_class, 'BLEND_Opaque', None)['reason'],
                         'no cooked base material')

    def test_chain_parameters_skip_undecoded_expressions_and_prefer_child_overrides(self):
        rows = {1: {'class': 'Engine.MaterialInstanceConstant', 'path': 'Prop_Skybox.Materials.Mati_Sky',
                    'data': {'properties': tags(Parent={'index': 2}, TextureParameterValues=[
                        tags(ParameterName='Masks', ParameterValue={'index': 6})])}},
                2: {'class': 'Engine.Material', 'path': 'Common_Materials.Sky.Mat_Sky_Master',
                    'data': {'properties': tags(Expressions=[{'index': 3}, {'index': 4}, {'index': 5}, {'index': 8}])}},
                3: {'class': 'Engine.MaterialExpressionTextureSampleParameter2D', 'path': 'x.Sampler',
                    'data': {'properties': tags(ParameterName='Masks', Texture={'index': 7})}},
                4: {'class': 'Engine.MaterialExpressionScalarParameter', 'path': 'x.Scalar',
                    'data': {'properties': tags(ParameterName='Time_of_Day', DefaultValue=170)}},
                5: {'class': 'Engine.MaterialExpressionVectorParameter', 'path': 'x.Vector', 'error': 'truncated package'},
                6: {'class': 'Engine.Texture2D', 'path': 'Prop_Skybox.Textures.Sky_Multi2'},
                7: {'class': 'Engine.Texture2D', 'path': 'Prop_Skybox.Textures.Sky_Multi'},
                8: {'class': 'Engine.MaterialExpressionConstant', 'path': 'x.Constant', 'error': 'truncated package'}}

        class FakeScene:
            issues = []

            def load(self, package):
                return rows

            def identity(self, key):
                return 'Synthetic:' + rows[key[1]]['path']

            def resolve(self, package, ref):
                index = ref.get('index', 0) if isinstance(ref, dict) else ref
                return ('Synthetic', index) if index else None

            def call(self, package, *args):
                return {'properties': tags(Format='PF_DXT1', SizeX=256, SizeY=256)}

        census = SkyCensus(FakeScene())
        chain = census.parent_chain(('Synthetic', 1))
        self.assertEqual([k[1] for k in chain], [1, 2])
        parameters = census.expression_parameters(chain)
        self.assertEqual(parameters['samplers'], {'Masks': 'Synthetic:Prop_Skybox.Textures.Sky_Multi2'})
        self.assertEqual(parameters['scalars'], {'Time_of_Day': 170})
        self.assertEqual(parameters['undecoded_parameter_expressions'], 1)
        self.assertEqual(census.textures['Synthetic:Prop_Skybox.Textures.Sky_Multi']['Format'], 'PF_DXT1')


if __name__ == '__main__':
    unittest.main()
