"""Synthetic tests for tools/prepare_weapon_paint.py and resolve_material's base-default fallback.

Invented MIC chains in UModel's props.txt layout and invented reader records; no game data.
"""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from PIL import Image
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import prepare_weapon_paint as paint
from render_weapon_previews import resolve_material

DETAIL = 'Weap_LauncherShotgunPistol_Comp'  # a DETAIL_CHANNELS key; the PNG itself is synthetic


def props(parent, scalars=(), vectors=(), textures=()):
    def block(header, entries, show):
        lines = [f'{header}[{len(entries)}] =', '{']
        for i, (name, value) in enumerate(entries):
            lines += [f'    {header}[{i}] =', '    {', '        ParameterInfo = ',
                      f'        ParameterValue = {show(value)}', f'        ParameterName = {name}', '    }']
        return lines + ['}']
    text = [f'Parent = {parent}']
    text += block('ScalarParameterValues', list(scalars), str)
    text += block('TextureParameterValues', list(textures), lambda leaf: f"Texture2D'Fake.Tex.{leaf}'")
    text += block('VectorParameterValues', list(vectors),
                  lambda v: '{ ' + ', '.join(f'{c}={x}' for c, x in zip('RGBA', v)) + ' }')
    return '\n'.join(text + ['BasePropertyOverrides = ', ''])


def colours(zones):
    return [(f'p_{zone}Color{tone}', (0.1 * n, 0.2, 0.3, 1)) for zone in zones
            for n, tone in enumerate(['Shadow', 'Midtone', 'Hilight'], 1)]


def chain(root, parent_vectors, child_scalars=(), decal=True):
    mic = root / 'FakePkg' / 'MaterialInstanceConstant'
    tex = root / 'FakePkg' / 'Texture2D'
    mic.mkdir(parents=True)
    tex.mkdir(parents=True)
    textures = [('p_Masks', 'FakeMasks'), ('p_Diffuse', DETAIL), ('p_NormalScopesEmissive', 'FakeNormal')]
    if decal:
        textures.append(('p_Decal', 'FakeDecal'))
    for leaf in [leaf for _, leaf in textures]:
        Image.new('RGB', (4, 4), (128, 128, 128)).save(tex / f'{leaf}.png')
    (mic / 'Mati_Fake.props.txt').write_text(props("MaterialInstanceConstant'Fake.Master.MasterMati_Fake'",
                                                   scalars=child_scalars, vectors=[('p_AColorMidtone', (9, 9, 9, 1))]))
    (mic / 'MasterMati_Fake.props.txt').write_text(props("Material3'Fake.Base.Base_Gun'", textures=textures,
                                                         vectors=parent_vectors))
    return root


def base_defaults(name):
    if name != 'Base_Gun':
        return None
    vector = dict(colours('ABC'), p_DecalScalePosition=(1, 1, 0, 0), p_DecalChannel=(1, 1, 1, 1),
                  p_DecalColor=(1, 1, 1, 1), p_AColorMidtone=(5, 5, 5, 1))
    scalar = {'p_DecalRotate': 0.0, 'p_UseFullColorDecal': 1.0, 'p_ReplaceDecal': 0.0}
    return {'path': 'Fake.Base.Base_Gun', 'scalar': scalar, 'vector': vector,
            'texture': {'p_Decal': 'StubTexture'},
            'source': {kind: {name: f'Fake.Base.Base_Gun:{name}' for name in values}
                       for kind, values in (('scalar', scalar), ('vector', vector))}}


class FakeFacts:
    """The two lookups prepare() uses; no reader process."""
    class package:
        package = Path('Fake.upk')

    def __init__(self, address=('TA_Clamp', 'TA_Wrap')):
        self.address = address

    base_defaults = staticmethod(base_defaults)

    def texture_address(self, name):
        return list(self.address) if self.address else None


def recipe(root):
    path = root / 'fake_gun.json'
    path.write_text(json.dumps({'material': 'Fake.Mati_Fake', 'weapon_type': 'Fake.pistol'}))
    return path


class ResolveMaterialTest(unittest.TestCase):
    def test_child_overrides_parent_and_sources_name_the_mic(self):
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('A'))
            params, _, names = resolve_material('Mati_Fake', [root])
            self.assertEqual(names, ['Mati_Fake', 'MasterMati_Fake'])
            self.assertEqual(params['vector']['p_AColorMidtone'], (9, 9, 9, 1))
            self.assertEqual(params['source']['vector']['p_AColorMidtone'], 'Mati_Fake')
            self.assertEqual(params['source']['vector']['p_AColorShadow'], 'MasterMati_Fake')
            self.assertNotIn('p_BColorShadow', params['vector'])

    def test_base_defaults_fill_only_what_every_mic_left_unset(self):
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('A'))
            params, _, _ = resolve_material('Mati_Fake', [root], base_defaults)
            self.assertEqual(params['vector']['p_AColorMidtone'], (9, 9, 9, 1))  # MIC wins over base default
            self.assertEqual(params['vector']['p_BColorShadow'], (0.1, 0.2, 0.3, 1))
            self.assertEqual(params['source']['vector']['p_BColorShadow'], 'Fake.Base.Base_Gun:p_BColorShadow')
            self.assertEqual(params['texture']['p_Decal'], 'FakeDecal')  # texture defaults are never merged
            self.assertEqual(params['scalar']['p_ReplaceDecal'], 0.0)

    def test_lookup_is_asked_for_the_chain_end_only(self):
        asked = []
        with tempfile.TemporaryDirectory() as folder:
            resolve_material('Mati_Fake', [chain(Path(folder), colours('A'))], lambda name: asked.append(name))
        self.assertEqual(asked, ['Base_Gun'])


class BaseDefaultsFromReaderTest(unittest.TestCase):
    @staticmethod
    def record(path, name, value=None, status='decoded'):
        properties = [{'name': 'ParameterName', 'value': name, 'status': 'decoded'}]
        if value is not None:
            properties.insert(0, {'name': 'DefaultValue', 'value': value, 'status': status})
        return {'path': path, 'properties': properties}

    def test_values_class_defaults_and_conflicts(self):
        vector, scalar = 'Engine.MaterialExpressionVectorParameter', 'Engine.MaterialExpressionScalarParameter'
        children = [
            (vector, self.record('M.Base.VP_1', 'p_Colour', {'R': 0.5, 'G': 1, 'B': 2, 'A': 1})),
            (vector, self.record('M.Base.VP_2', 'p_Missing')),
            (scalar, self.record('M.Base.SP_1', 'p_Amount', 3)),
            (scalar, self.record('M.Base.SP_2', 'p_Unset')),
            (scalar, self.record('M.Base.SP_3', 'p_Twice', 1)),
            (scalar, self.record('M.Base.SP_4', 'p_Twice', 2)),
            ('Engine.MaterialExpressionStaticSwitchParameter', self.record('M.Base.SW_1', 'sw_Static', True)),
            ('Engine.MaterialExpressionTextureSampleParameter2D', self.record('M.Base.TP_1', 'p_Tex')),
        ]
        found = paint.base_defaults_from('M.Base', children, {'scalar': 0.0, 'vector': (0.0, 0.0, 0.0, 1.0)})
        self.assertEqual(found['vector'], {'p_Colour': (0.5, 1.0, 2.0, 1.0), 'p_Missing': (0.0, 0.0, 0.0, 1.0)})
        self.assertEqual(found['scalar'], {'p_Amount': 3.0, 'p_Unset': 0.0})
        self.assertEqual(found['source']['vector']['p_Colour'], 'M.Base:VP_1')
        self.assertEqual(found['source']['scalar']['p_Unset'], 'M.Base:SP_2 (class default)')
        self.assertEqual(found['conflicts'], ['p_Twice'])

    def test_undecoded_default_refuses(self):
        children = [('MaterialExpressionScalarParameter', self.record('M.Base.SP_1', 'p_Amount', 0, 'unsupported'))]
        with self.assertRaises(ValueError):
            paint.base_defaults_from('M.Base', children, {'scalar': 0.0, 'vector': (0, 0, 0, 1)})


class PrepareTest(unittest.TestCase):
    def test_missing_zone_colours_still_refuse_without_reader(self):
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('A'))
            with self.assertRaisesRegex(ValueError, 'p_BColorShadow'):
                paint.prepare(recipe(root), root)

    def test_reader_facts_fill_zones_and_draw_the_decal(self):
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('A') + [('p_DecalScalePosition', (4, 2, 0.25, 0))],
                         child_scalars=[('p_DecalRotate', 0.5)])
            out = paint.prepare(recipe(root), root, facts=FakeFacts())
            decal = out['decal']
            self.assertTrue(decal['used'])
            self.assertEqual(decal['texture'], 'FakeDecal')
            self.assertEqual(list(decal['scale_position']), [4, 2, 0.25, 0])
            self.assertEqual(decal['rotate'], 0.5)  # emitted; the importer does not apply it
            self.assertEqual(decal['address'], ['TA_Clamp', 'TA_Wrap'])
            self.assertIn('UNVERIFIED', decal['reading'])
            self.assertTrue(out['textures']['p_Decal'].endswith('FakeDecal.png'))
            self.assertEqual(out['params']['source']['vector']['p_CColorHilight'], 'Fake.Base.Base_Gun:p_CColorHilight')

    def test_decal_not_drawn_without_address_or_full_colour(self):
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('ABC') + [('p_DecalScalePosition', (1, 1, 0, 0))])
            unknown = paint.prepare(recipe(root), root, facts=FakeFacts(address=None))['decal']
            self.assertFalse(unknown['used'])
            self.assertIn('no unique Texture2D', unknown['not_used_because'])
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('ABC'), child_scalars=[('p_UseFullColorDecal', 0)])
            single = paint.prepare(recipe(root), root, facts=FakeFacts())
            self.assertFalse(single['decal']['used'])
            self.assertIn('p_UseFullColorDecal', single['decal']['not_used_because'])
            self.assertNotIn('p_Decal', single['textures'])

    def test_no_decal_parameter_means_no_layer(self):
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('ABC'), decal=False)
            self.assertIsNone(paint.prepare(recipe(root), root)['decal'])


if __name__ == '__main__':
    unittest.main()
