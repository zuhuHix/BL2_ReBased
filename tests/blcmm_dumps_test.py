"""Parser tests for the OpenBLCMM dump reader, on synthetic dump text.

The text here is hand-written in the shape the game's ``obj dump`` prints, not
copied from a dump file: no game data lives in the repository.
"""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from blcmm_dumps import level_object, parse_dump, parse_value, split_top_level

COMPONENT = """*** Property dump for object 'TerrainComponent Level.TheWorld:PersistentLevel.Terrain_3.TerrainComponent_4' ***
=== TerrainComponent properties ===
ShadowMaps(0)=ShadowMap2D'Level.TheWorld:PersistentLevel.Terrain_3.TerrainComponent_4.ShadowMap2D_1'
TerrainObject=
SectionBaseX=0
SectionBaseY=16
SectionSizeX=16
SectionSizeY=16
BatchMaterials(0)=1
BatchMaterials(1)=0
BatchMaterials(3)=2
FullBatch=3
=== PrimitiveComponent properties ===
Bounds=(Origin=(X=-23424.000000,Y=-29696.000000,Z=1930.000000),BoxExtent=(X=3073.000000,Y=3073.000000,Z=263.000000),SphereRadius=4353.829102)
_LocalToWorld=(XPlane=(W=0.000000,X=384.000000,Y=0.000000,Z=0.000000),YPlane=(W=0.000000,X=0.000000,Y=384.000000,Z=0.000000),ZPlane=(W=0.000000,X=0.000000,Y=0.000000,Z=256.000000),WPlane=(W=1.000000,X=-26496.000000,Y=-32768.000000,Z=0.000000))
FogVolumeComponent=None
bUseAsOccluder=True
bSelectable=False
DepthPriorityGroup=SDPG_World
Scales[0]=1.000000
Scales[1]=0.500000
"""

MATERIAL = """*** Property dump for object 'MaterialInstanceConstant Pkg.Materials.Mati_House' ***
=== MaterialInstanceConstant properties ===
TextureParameterValues(0)=(ParameterValue=Texture2D'Pkg.Textures.House_Dif',ParameterName="p_Diffuse",ExpressionGUID=(A=1,B=2,C=3,D=4))
TextureParameterValues(1)=(ParameterValue=Texture2D'Pkg.Textures.House_Nrm',ParameterName="p_Normal",ExpressionGUID=(A=5,B=6,C=7,D=8))
Parent=Material'Common_Materials.Environment.Master_World'
"""


class ValueTest(unittest.TestCase):
    def test_scalars(self):
        self.assertEqual(parse_value('0'), 0)
        self.assertEqual(parse_value('-12'), -12)
        self.assertEqual(parse_value('1.500000'), 1.5)
        self.assertIs(parse_value('True'), True)
        self.assertIs(parse_value('False'), False)
        self.assertIsNone(parse_value('None'))
        self.assertIsNone(parse_value(''))
        self.assertEqual(parse_value('"p_Diffuse"'), 'p_Diffuse')
        # Enum names and other bare words stay strings rather than becoming numbers.
        self.assertEqual(parse_value('SDPG_World'), 'SDPG_World')

    def test_reference(self):
        self.assertEqual(parse_value("Texture2D'Pkg.Textures.House_Dif'"),
                         {'class': 'Texture2D', 'path': 'Pkg.Textures.House_Dif'})

    def test_nested_struct(self):
        value = parse_value('(Origin=(X=1.000000,Y=-2.000000),Radius=3.500000)')
        self.assertEqual(value, {'Origin': {'X': 1.0, 'Y': -2.0}, 'Radius': 3.5})

    def test_split_respects_parentheses_and_quotes(self):
        self.assertEqual(split_top_level('a=1,b=(c=2,d=3),e="x,y"'), ['a=1', 'b=(c=2,d=3)', 'e="x,y"'])
        self.assertEqual(split_top_level(''), [''])


class DumpTest(unittest.TestCase):
    def test_header_and_sections(self):
        dump = parse_dump(COMPONENT)
        self.assertEqual(dump['class'], 'TerrainComponent')
        self.assertEqual(dump['name'], 'Level.TheWorld:PersistentLevel.Terrain_3.TerrainComponent_4')
        self.assertNotIn('=== TerrainComponent properties ===', dump['properties'])
        # Properties from both sections land in one flat mapping.
        self.assertEqual(dump['properties']['SectionBaseY'], 16)
        self.assertIs(dump['properties']['bUseAsOccluder'], True)
        self.assertIsNone(dump['properties']['FogVolumeComponent'])
        self.assertIsNone(dump['properties']['TerrainObject'])

    def test_dynamic_and_static_arrays(self):
        properties = parse_dump(COMPONENT)['properties']
        # Index 2 is absent from the text, so the gap stays None rather than shifting later entries.
        self.assertEqual(properties['BatchMaterials'], [1, 0, None, 2])
        self.assertEqual(properties['Scales'], [1.0, 0.5])
        self.assertEqual(properties['ShadowMaps'][0]['class'], 'ShadowMap2D')

    def test_struct_properties(self):
        properties = parse_dump(COMPONENT)['properties']
        self.assertEqual(properties['Bounds']['Origin']['Z'], 1930.0)
        self.assertEqual(properties['Bounds']['SphereRadius'], 4353.829102)
        self.assertEqual(properties['_LocalToWorld']['WPlane']['X'], -26496.0)
        self.assertEqual(properties['_LocalToWorld']['XPlane']['X'], 384.0)

    def test_material_parameters(self):
        dump = parse_dump(MATERIAL)
        self.assertEqual(dump['class'], 'MaterialInstanceConstant')
        values = dump['properties']['TextureParameterValues']
        self.assertEqual([v['ParameterName'] for v in values], ['p_Diffuse', 'p_Normal'])
        self.assertEqual(values[0]['ParameterValue']['path'], 'Pkg.Textures.House_Dif')
        self.assertEqual(dump['properties']['Parent']['path'], 'Common_Materials.Environment.Master_World')

    def test_empty_text(self):
        dump = parse_dump('')
        self.assertEqual((dump['class'], dump['name'], dump['properties']), (None, None, {}))


class NameTest(unittest.TestCase):
    def test_level_object(self):
        self.assertEqual(level_object('Sanctuary_P', 'TheWorld.PersistentLevel.Terrain_0'),
                         'Sanctuary_P.TheWorld:PersistentLevel.Terrain_0')
        # Anything that is not a level path is passed through unchanged.
        self.assertEqual(level_object('Sanctuary_P', 'Pkg.Materials.Mati_House'), 'Pkg.Materials.Mati_House')


if __name__ == '__main__':
    unittest.main()
