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
                  p_DecalColor=(1, 1, 1, 1), p_AColorMidtone=(5, 5, 5, 1), p_DColor=(1, 1, 1, 1))
    scalar = {'p_DecalRotate': 0.0, 'p_UseFullColorDecal': 1.0, 'p_ReplaceDecal': 0.0,
              'p_HighlightsIntensity': 2.0, 'p_ShadowsIntensity': 3.0}
    return {'path': 'Fake.Base.Base_Gun', 'scalar': scalar, 'vector': vector,
            'texture': {'p_Decal': 'StubTexture'},
            'source': {kind: {name: f'Fake.Base.Base_Gun:{name}' for name in values}
                       for kind, values in (('scalar', scalar), ('vector', vector))}}


class FakeFacts:
    """The two lookups prepare() uses; no reader process."""
    class package:
        package = Path('Fake.upk')

    def __init__(self, address=('TA_Clamp', 'TA_Wrap'), static=None):
        self.address = address
        self.static = static

    base_defaults = staticmethod(base_defaults)

    def static_channels(self, name):
        return self.static

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
            self.assertEqual(decal['rotate'], 0.5)
            self.assertEqual(decal['address'], ['TA_Clamp', 'TA_Wrap'])
            self.assertIn('UNVERIFIED', decal['reading'])
            self.assertTrue(out['textures']['p_Decal'].endswith('FakeDecal.png'))
            self.assertEqual(out['params']['source']['vector']['p_CColorHilight'], 'Fake.Base.Base_Gun:p_CColorHilight')

    def test_decal_not_drawn_without_address_but_single_channel_is_drawn(self):
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('ABC') + [('p_DecalScalePosition', (1, 1, 0, 0))])
            unknown = paint.prepare(recipe(root), root, facts=FakeFacts(address=None))['decal']
            self.assertFalse(unknown['used'])
            self.assertIn('no unique Texture2D', unknown['not_used_because'])
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('ABC'), child_scalars=[('p_UseFullColorDecal', 0)])
            single = paint.prepare(recipe(root), root, facts=FakeFacts())
            self.assertTrue(single['decal']['used'])  # the compiled shader has a single-channel path
            self.assertEqual(single['decal']['full_color'], 0)

    def test_no_decal_parameter_means_no_layer(self):
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('ABC'), decal=False)
            self.assertIsNone(paint.prepare(recipe(root), root, facts=FakeFacts())['decal'])


class StaticChoicesTest(unittest.TestCase):
    def test_static_parameters_pick_channels_over_the_guess(self):
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('ABC'))
            facts = FakeFacts(static={'p_WeapClassSelect': 'G', 'p_PatternChannel': 'R', 'p_DecalChannel': 'RGB'})
            out = paint.prepare(recipe(root), root, facts=facts)
            self.assertEqual(out['detail_channel'], 1)  # the pistol guess would be 2 (blue)
            self.assertEqual(out['pattern_channels'], 'R')
            self.assertIn('Mati_Fake static parameters', out['channel_source'])
            self.assertIn('UNVERIFIED', out['reading'])

    def test_guess_without_static_parameters(self):
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('ABC'))
            out = paint.prepare(recipe(root), root, facts=FakeFacts(static=None))
            self.assertEqual(out['detail_channel'], 2)
            self.assertIn('guess', out['channel_source'])

    def test_more_than_one_detail_channel_refuses(self):
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('ABC'))
            with self.assertRaisesRegex(ValueError, 'p_WeapClassSelect'):
                paint.prepare(recipe(root), root, facts=FakeFacts(static={'p_WeapClassSelect': 'RG'}))


def tail_bytes(extra=b''):
    """An invented MIC tail in the observed layout: resource block, then a static parameter set."""
    import struct
    i = lambda *values: struct.pack(f'<{len(values)}i', *values)
    guid = bytes(range(16))
    resource = i(0, 0, 1) + guid + i(2) + i(2, -5, 7) + i(0, 0, 0, 0, 0, 0) + i(1) + i(0, 1) + struct.pack(
        '<2f', 1.0, 0.5) + i(0)
    static = guid + i(1) + i(0, 0) + i(0, 0) + guid  # switch name 0 = False, no override
    static += i(2) + i(1, 0) + i(0, 0, 1, 0) + i(1) + guid + i(2, 0) + i(1, 1, 1, 0) + i(0) + guid
    static += i(1) + i(3, 0) + bytes([3]) + i(1) + guid + i(0)
    return resource + static + extra


class StaticParameterDecodeTest(unittest.TestCase):
    NAMES = ['sw_Fake', 'p_FakeClass', 'p_FakePattern', 'p_FakeNormal']

    def test_exact_consumption(self):
        from material_static_parameters import decode_tail, mask_channels
        out = decode_tail(tail_bytes(), self.NAMES)
        self.assertEqual(out['resource']['textures'], [-5, 7])
        self.assertEqual(out['resource']['lookups'][0]['v_scale'], 0.5)
        masks = out['static']['component_masks']
        self.assertEqual([(m['name'], mask_channels(m), m['override']) for m in masks],
                         [('p_FakeClass', 'B', True), ('p_FakePattern', 'RGB', False)])
        self.assertEqual(out['static']['switches'][0]['name'], 'sw_Fake')
        self.assertEqual(out['static']['normals'][0]['compression'], 3)

    def test_left_over_or_truncated_bytes_refuse(self):
        from material_static_parameters import decode_tail
        with self.assertRaisesRegex(ValueError, 'left over'):
            decode_tail(tail_bytes(b'\0\0\0\0'), self.NAMES)
        with self.assertRaises(ValueError):
            decode_tail(tail_bytes()[:-3], self.NAMES)

    def test_bad_name_index_refuses(self):
        from material_static_parameters import decode_tail
        with self.assertRaisesRegex(ValueError, 'bad name'):
            decode_tail(tail_bytes(), self.NAMES[:2])


def model_params(**scalars):
    import weapon_paint_model as model
    vector = {name: (1.0, 1.0, 1.0, 1.0) for name in model.PAINT_PARAMETERS['vector']}
    vector.update({'p_AColorMidtone': (2.0, 1.0, 0.5, 1), 'p_AColorHilight': (4.0, 4.0, 4.0, 1),
                   'p_AColorShadow': (0.0, 0.0, 0.0, 1), 'p_BColorMidtone': (0.0, 1.0, 0.0, 1),
                   'p_DColor': (0.5, 0.5, 0.5, 1), 'p_PatternColor': (2.0, 2.0, 2.0, 1),
                   'p_PatternChannelScale': (1.0, 0.0, 0.0, 0), 'p_DecalColor': (1.0, 1.0, 1.0, 1),
                   'p_DecalChannel': (0.0, 1.0, 0.0, 1)})
    scalar = {'p_HighlightsIntensity': 2.0, 'p_ShadowsIntensity': 4.0, 'p_ReplacePattern': 0.0,
              'p_UseFullColorDecal': 1.0, 'p_ReplaceDecal': 0.0}
    scalar.update(scalars)
    return {'vector': vector, 'scalar': scalar, 'pattern_channels': 'RGB', 'decal_channels': 'RGB'}


class PaintModelTest(unittest.TestCase):
    """Invented values; checks the reading in tools/weapon_paint_model.py, not the game."""

    def setUp(self):
        sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
        import weapon_paint_model
        self.model = weapon_paint_model

    def assertClose(self, got, want):
        for g, w in zip(got, want):
            self.assertAlmostEqual(g, w, places=5)

    def test_masks_texture_halves(self):
        self.assertEqual(self.model.mask_uvs(0.25, 0.5), ((0.25, 0.25), (0.25, 0.75)))

    def test_unmasked_texel_is_dcolor_times_detail(self):
        self.assertClose(self.model.albedo((0, 0), (0, 0, 0), 0.5, model_params()), (0.25, 0.25, 0.25))

    def test_zone_tones_midtone_then_hilight_then_shadow(self):
        params = model_params()
        self.assertClose(self.model.albedo((0, 0), (1, 0, 0), 1.0, params), (2.0, 1.0, 0.5))
        # highlight amount saturate(0.25 * 2) = 0.5 -> halfway to Hilight
        self.assertClose(self.model.albedo((0.25, 0), (1, 0, 0), 1.0, params), (3.0, 2.5, 2.25))
        # shadow amount saturate(0.5 * 4) = 1 -> Shadow wins whatever the highlight
        self.assertClose(self.model.albedo((1, 0.5), (1, 0, 0), 1.0, params), (0.0, 0.0, 0.0))

    def test_zones_blend_in_order_over_dcolor(self):
        # A fully, then B by half: halfway between A's midtone and B's midtone
        self.assertClose(self.model.albedo((0, 0), (1, 0.5, 0), 1.0, model_params()), (1.0, 1.0, 0.25))

    def test_pattern_multiplies_by_squared_weight_or_replaces(self):
        texel = ((0, 0), (0.5, 0, 0), 1.0)
        base = (0.5 * 0.5 + 2.0 * 0.5, 0.5 * 0.5 + 1.0 * 0.5, 0.5 * 0.5 + 0.5 * 0.5)
        weight = 0.25  # (0.5 * 1) ** 2
        tint = 2.0 * 0.5  # PatternColor * pattern value 0.5
        multiplied = tuple(b * (1 + (tint - 1) * weight) for b in base)
        self.assertClose(self.model.albedo(*texel, model_params(), pattern=(0.5, 0.5, 0.5)), multiplied)
        replaced = tuple(b + (tint - b) * weight for b in base)
        self.assertClose(self.model.albedo(*texel, model_params(p_ReplacePattern=1.0), pattern=(0.5, 0.5, 0.5)),
                         replaced)

    def test_decal_full_colour_uses_alpha_single_channel_uses_colour(self):
        mask = (0, 1, 0)  # zone B, the decal channel
        full = self.model.albedo((0, 0), mask, 1.0, model_params(), decal=(0.5, 0.5, 0.5, 0.0))
        self.assertClose(full, (0.0, 1.0, 0.0))  # alpha 0: no decal
        single = self.model.albedo((0, 0), mask, 1.0, model_params(p_UseFullColorDecal=0.0),
                                   decal=(0.5, 0.5, 0.5, 0.0))
        self.assertClose(single, (0.0, 1.0 * (1 + (1 - 1) * 0.5), 0.0))  # tint 1: multiply leaves it

    def test_decal_uv_shift_rotate_scale_about_centre(self):
        self.assertClose(self.model.decal_uv(0.5, 0.5, (1, 1, 0, 0), 0.0), (0.5, 0.5))
        self.assertClose(self.model.decal_uv(0.75, 0.5, (1, 1, 0, 0), 0.5), (0.5, 0.75))  # quarter turn
        self.assertClose(self.model.decal_uv(0.75, 0.5, (2, 2, 0, 0), 0.0), (1.0, 0.5))
        self.assertClose(self.model.decal_uv(0.5, 0.5, (1, 1, 0.25, 0), 0.0), (0.75, 0.5))

    def test_single_static_channel_broadcasts(self):
        self.assertEqual(self.model.select((0.1, 0.2, 0.3), 'G'), (0.2, 0.2, 0.2))
        self.assertEqual(self.model.select((0.1, 0.2, 0.3), 'RB'), (0.1, 0.0, 0.3))


class ColourSpaceAndReflectionTest(unittest.TestCase):
    """Invented values for pass 2: SRGB flags, the environment term, sRGB decoding in the renderer."""

    class Engine:
        def __init__(self, cdo_props):
            self.cdo_props = cdo_props

        def find(self, name, klass):
            return {'index': 1} if name == 'Default__Texture' else None

        def records(self, indices):
            return {1: {'properties': [{'name': k, 'value': v} for k, v in self.cdo_props.items()]}}

    class Package(Engine):
        def find(self, name, klass):
            return {'index': 1} if name in ('FakeLinear', 'FakeDefault') else None

        def records(self, indices):
            return {1: {'properties': [{'name': 'SRGB', 'value': False}] if self.cdo_props == 'linear' else []}}

    def facts(self, texture, cdo):
        facts = paint.InstalledFacts.__new__(paint.InstalledFacts)
        facts.package, facts.engine, facts.cache = self.Package(texture), self.Engine(cdo), {}
        return facts

    def test_srgb_flag_or_class_default(self):
        self.assertFalse(self.facts('linear', {'SRGB': True}).texture_srgb('FakeLinear'))
        self.assertTrue(self.facts('absent', {'SRGB': True}).texture_srgb('FakeDefault'))
        self.assertIsNone(self.facts('absent', {'SRGB': True}).texture_srgb('Missing'))
        with self.assertRaisesRegex(ValueError, 'Default__Texture'):
            self.facts('absent', {}).texture_srgb('FakeDefault')

    def test_reflection_adds_and_scales(self):
        import weapon_paint_model as model
        params = {'vector': {'p_ReflectColor': (2.0, 1.0, 0.5, 1), 'p_ReflectionChannelScale': (0.0, 1.0, 0.0, 1)},
                  'scalar': {'p_ReflectColorScale': 0.0}}
        # zone B fully: R = env * ReflectColor; scale 0 -> plain add
        self.assertEqual(model.add_reflection((0.1, 0.1, 0.1), (0.5, 0.5, 0.5), (0, 1, 0), params), (1.1, 0.6, 0.35))
        params['scalar']['p_ReflectColorScale'] = 1.0  # -> c + R * c
        got = model.add_reflection((0.5, 0.5, 0.5), (0.5, 0.5, 0.5), (0, 1, 0), params)
        for g, w in zip(got, (1.0, 0.75, 0.625)):
            self.assertAlmostEqual(g, w)
        self.assertEqual(model.add_reflection((0.2, 0.2, 0.2), (1, 1, 1), (1, 0, 0), params), (0.2, 0.2, 0.2))

    def test_prepare_emits_srgb_flags_and_reflection(self):
        class Facts(FakeFacts):
            def texture_srgb(self, name):
                return name != 'FakeMasks'
        with tempfile.TemporaryDirectory() as folder:
            root = chain(Path(folder), colours('ABC') + [('p_ReflectColor', (1, 1, 1, 1)),
                                                         ('p_ReflectionChannelScale', (1, 1, 1, 1))],
                         child_scalars=[('p_ReflectColorScale', 1.0)])
            out = paint.prepare(recipe(root), root, facts=Facts())
            self.assertFalse(out['srgb']['p_Masks'])
            self.assertTrue(out['srgb']['p_Diffuse'])
            self.assertFalse(out['reflection']['used'])
            self.assertIn('no P_SimpleReflect', out['reflection']['not_used_because'])

    def test_renderer_decodes_srgb_texels(self):
        from render_weapon_previews import Texture
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'grey.png'
            Image.new('RGB', (2, 2), (128, 128, 128)).save(path)
            self.assertAlmostEqual(Texture(path).sample(0, 0)[0], 128 / 255)
            self.assertAlmostEqual(Texture(path, srgb=True).sample(0, 0)[0], 0.2158605, places=5)


class BytecodeReaderTest(unittest.TestCase):
    """An invented ps_3_0 token stream built from the documented token layout."""

    @staticmethod
    def stream(end=True):
        import struct
        dest = lambda kind, n, mask=0xF: 0x80000000 | ((kind & 7) << 28) | ((kind & 0x18) << 8) | (mask << 16) | n
        src = lambda kind, n, swizzle=0xE4, modifier=0: (0x80000000 | ((kind & 7) << 28) | ((kind & 0x18) << 8)
                                                         | (modifier << 24) | (swizzle << 16) | n)
        tokens = [0xFFFF0300]
        ctab = b'CTAB' + struct.pack('<7I', 28, 0, 0, 1, 28, 0, 0) + struct.pack('<IHHHHII', 48, 2, 9, 1, 0, 0, 0) + b'Fake\0\0\0\0'
        tokens += [0xFFFE | (len(ctab) // 4) << 16] + list(struct.unpack(f'<{len(ctab) // 4}I', ctab))
        tokens += [0x04000004, dest(0, 1, 0x7), src(2, 9), src(1, 0, 0x00), src(0, 2, 0xE4, 6)]  # mad r1.xyz
        tokens += [0x03000042, dest(0, 0), src(1, 0), src(10, 3)]  # texld r0, v0, s3
        if end:
            tokens.append(0x0000FFFF)
        return struct.pack(f'<{len(tokens)}I', *tokens)

    def test_instructions_and_constant_table(self):
        sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'research'))
        from d3d9_bytecode import disassemble
        shader, constants, lines = disassemble(self.stream())
        self.assertEqual(shader, 'ps_3_0')
        self.assertEqual(constants, [('Fake', 'c', 9, 1)])
        self.assertEqual(lines, ['mad r1.xyz, c9, v0.x, (1-r2)', 'texld r0, v0, s3'])

    def test_missing_end_token_refuses(self):
        sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'research'))
        from d3d9_bytecode import disassemble
        with self.assertRaises(ValueError):
            disassemble(self.stream(end=False))


class FragmentTests(unittest.TestCase):
    """tools/weapon_recipe.part_fragments and static_arrays on invented reader records."""

    class FakePackage:
        def __init__(self, props, extra):
            self._props, self.extra_names = props, extra

        def props(self, path):
            return self._props[path]

    def test_static_array_elements_keep_order_and_all_values(self):
        import weapon_recipe
        data = {'properties': [
            {'name': 'Names', 'array_index': 1, 'value': 'B'}, {'name': 'Names', 'array_index': 0, 'value': 'A'},
            {'name': 'Gone', 'status': 'unsupported', 'value': None}, {'name': 'One', 'value': 5}]}
        self.assertEqual(weapon_recipe.static_arrays(data), {'Names': ['A', 'B'], 'One': [5]})

    def test_part_draws_its_fragment_and_the_additional_ones(self):
        import weapon_recipe
        package = self.FakePackage({'GD.Body.Body_X': {'GestaltModeSkeletalMeshName': 'Body_X'}},
                                   {'GD.Body.Body_X': {'AdditionalGestaltModeSkeletalMeshNames': ['Body_X_Var1', 'None']}})
        self.assertEqual(weapon_recipe.part_fragments(package, 'GD.Body.Body_X'), ['Body_X', 'Body_X_Var1'])

    def test_none_parts_draw_nothing(self):
        import weapon_recipe
        package = self.FakePackage({'GD.Sight.Pistol_Sight_None': {'GestaltModeSkeletalMeshName': 'Pistol_Scope_Made'}}, {})
        self.assertEqual(weapon_recipe.part_fragments(package, 'GD.Sight.Pistol_Sight_None'), [])


class PartVectorTests(unittest.TestCase):
    class FakeFacts:
        def __init__(self, table):
            self.table = table

        def part_vectors(self, part):
            return self.table.get(part, {})

    def test_later_part_wins_and_order_is_the_native_slot_order(self):
        recipe = {'parts': {'Material': {'part': 'M'}, 'Elemental': {'part': 'E'}, 'Body': {'part': 'B'}}}
        facts = self.FakeFacts({'B': {'p_EmissiveColor': (1, 1, 1, 1)}, 'E': {'p_EmissiveColor': (4, 0, 0, 1)},
                                'M': {'p_Other': (2, 2, 2, 1)}})
        self.assertEqual(paint.part_vector_overrides(recipe, facts),
                         {'p_EmissiveColor': (4, 0, 0, 1), 'p_Other': (2, 2, 2, 1)})

    def test_without_reader_nothing_is_overridden(self):
        self.assertEqual(paint.part_vector_overrides({'parts': {'Body': {'part': 'B'}}}, None), {})


if __name__ == '__main__':
    unittest.main()
