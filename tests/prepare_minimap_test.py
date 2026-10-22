"""Synthetic tests for the pure helpers of tools/prepare_minimap.py (no packages, game or ow-package needed)."""
import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import prepare_minimap as pm  # noqa: E402


def bits(fields):
    """Pack (value, width) pairs most significant bit first, padded to a byte boundary (SWF bit fields)."""
    total = sum(width for _, width in fields)
    value = 0
    for v, width in fields:
        value = (value << width) | (v & ((1 << width) - 1))
    nbytes = (total + 7) // 8
    return (value << (nbytes * 8 - total)).to_bytes(nbytes, 'big')


def matrix(scale=None, translate=(1440, 720)):
    """MATRIX: optional HasScale (17-bit 16.16 factors), no rotate, then NTranslateBits(12) translate."""
    fields = []
    if scale is None:
        fields.append((0, 1))
    else:
        fields += [(1, 1), (17, 5), (scale[0], 17), (scale[1], 17)]
    fields.append((0, 1))                                  # HasRotate
    fields += [(12, 5), (translate[0], 12), (translate[1], 12)]
    return bits(fields)


def place_object2(depth=1, char=None, matrix_bytes=None, name=None, code=26):
    """PlaceObject2 (26) body; PlaceObject3 (70) adds a flags byte with no class name or image."""
    flags = (0x02 if char is not None else 0) | (0x04 if matrix_bytes is not None else 0) | (0x20 if name else 0)
    body = bytes([flags])
    if code == 70:
        body += b'\0'                                      # PlaceFlagsThree: no class name, image or blend mode
    body += struct.pack('<H', depth)
    if char is not None:
        body += struct.pack('<H', char)
    if matrix_bytes is not None:
        body += matrix_bytes
    if name is not None:
        body += name.encode('latin-1') + b'\0'
    return body


def record(code, body):
    """A tag header (code << 6 | length, or 0x3F then a u32 length) followed by the body."""
    if len(body) < 0x3F:
        return struct.pack('<H', (code << 6) | len(body)) + body
    return struct.pack('<HI', (code << 6) | 0x3F, len(body)) + body


def swf(tags):
    """An uncompressed FWS movie: 1-bit frame rect of zeros, 24 fps, one frame, then the tag records."""
    rect = b'\x08\x00'                                     # nbits = 1, four zero bits, padded
    body = rect + struct.pack('<HH', 24 << 8, 1) + b''.join(tags)
    return b'FWS' + bytes([10]) + struct.pack('<I', 8 + len(body)) + body


class FakeReader:
    """Stands in for prepare_slice_world.Reader: props by path, and the package's exports by path."""

    def __init__(self, props=None, exports=None):
        self._props = props or {}
        self._exports = exports or {}

    def props(self, pkg, path, allow_tail=False):
        return self._props[path]

    def exports(self, pkg):
        return {'by_path': self._exports}


class PlacedNamesTest(unittest.TestCase):
    def test_name_only(self):
        self.assertEqual(pm.placed_names(place_object2(name='MAP_PWDA_0'), 26), 'MAP_PWDA_0')

    def test_character_matrix_scale_and_translate_then_name(self):
        body = place_object2(depth=7, char=42, matrix_bytes=matrix(scale=(98304, 32768), translate=(1440, 720)),
                             name='MAP_PWDA_1')
        self.assertEqual(pm.placed_names(body, 26), 'MAP_PWDA_1')

    def test_matrix_translate_only_then_name(self):
        body = place_object2(char=3, matrix_bytes=matrix(translate=(600, 90)), name='Hero')
        self.assertEqual(pm.placed_names(body, 26), 'Hero')

    def test_no_name_returns_none(self):
        self.assertIsNone(pm.placed_names(place_object2(char=42, matrix_bytes=matrix(translate=(100, 200))), 26))

    def test_place_object3_with_name(self):
        body = place_object2(depth=2, char=9, name='MAP_PWDA_3', code=70)
        self.assertEqual(pm.placed_names(body, 70), 'MAP_PWDA_3')
        self.assertIsNone(pm.placed_names(place_object2(depth=2, char=9, code=70), 70))


class FogBlobsTest(unittest.TestCase):
    def test_returns_only_pwda_blobs_on_the_root_timeline(self):
        tags = [
            record(26, place_object2(depth=1, char=3, name='MAP_PWDA_0')),
            record(26, place_object2(depth=2, char=4, name='MAP_PWDA_1')),
            record(26, place_object2(depth=3, char=5, name='other')),
            record(1, b''),                                # ShowFrame
            record(0, b''),                                # End
        ]
        self.assertEqual(pm.fog_blobs(swf(tags)), ['MAP_PWDA_0', 'MAP_PWDA_1'])

    def test_long_form_tag_header_is_read(self):
        long_name = 'other' + 'x' * 80                     # body over 62 bytes, so the long header form
        tags = [record(26, place_object2(depth=1, char=3, name=long_name)), record(26, place_object2(depth=2, name='MAP_PWDA_2')),
                record(0, b'')]
        self.assertEqual(pm.fog_blobs(swf(tags)), ['MAP_PWDA_2'])


class VolumeCentreTest(unittest.TestCase):
    def setUp(self):
        self.component = {'BrushAggGeom': {'ConvexElems': [{'ElemBox': {'Min': {'X': -100, 'Y': -50, 'Z': 0},
                                                                     'Max': {'X': 300, 'Y': 150, 'Z': 0}}}]}}

    def test_centre_is_box_middle_moved_by_location(self):
        actor = {'Location': {'X': 10, 'Y': 20, 'Z': 0}, 'BrushComponent': {'index': 5, 'path': 'Comp'}}
        reader = FakeReader({'Vol': actor, 'Comp': self.component})
        self.assertEqual(pm.volume_centre(reader, 'pkg', 'Vol'), [110.0, 70.0])

    def test_rotation_is_refused(self):
        actor = {'Location': {'X': 0, 'Y': 0, 'Z': 0}, 'Rotation': {'Yaw': 16384},
                 'BrushComponent': {'index': 5, 'path': 'Comp'}}
        reader = FakeReader({'Vol': actor, 'Comp': self.component})
        with self.assertRaises(ValueError):
            pm.volume_centre(reader, 'pkg', 'Vol')

    def test_no_convex_elements_is_refused(self):
        actor = {'Location': {'X': 0, 'Y': 0, 'Z': 0}, 'BrushComponent': {'index': 5, 'path': 'Comp'}}
        reader = FakeReader({'Vol': actor, 'Comp': {'BrushAggGeom': {'ConvexElems': []}}})
        with self.assertRaises(ValueError):
            pm.volume_centre(reader, 'pkg', 'Vol')


class PopulationDefinitionTest(unittest.TestCase):
    def test_factory_balance_leads_to_default_interactive_object(self):
        exports = {
            'PopDef': {'class': 'WillowGame.PopulationDef'},
            'PopDef.PopulationFactoryVendingMachine_0': {'class': 'WillowGame.PopulationFactoryVendingMachine'},
            'Bal': {'class': 'WillowGame.ObjectBalanceDefinition'},
        }
        props = {
            'PopDef.PopulationFactoryVendingMachine_0': {'ObjectBalanceDefinition': {'index': 3, 'path': 'Bal'}},
            'Bal': {'DefaultInteractiveObject': {'index': 4, 'path': 'Def'}},
        }
        reader = FakeReader(props, exports)
        self.assertEqual(pm.population_definition(reader, 'pkg', 'PopDef'), 'Def')

    def test_population_without_factory_gives_none(self):
        exports = {'PopNo': {'class': 'WillowGame.PopulationDef'}, 'Other.Thing': {'class': 'Engine.Actor'}}
        self.assertIsNone(pm.population_definition(FakeReader({}, exports), 'pkg', 'PopNo'))



class LocalPathTest(unittest.TestCase):
    """Review 1: outputs are refused outside the repository's ignored local/."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name)
        (self.repo / 'local').mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def test_defaults_resolve_under_local(self):
        self.assertEqual(pm.local_path('local/slice/minimap.json', repo=self.repo),
                         (self.repo / 'local/slice/minimap.json').resolve())
        self.assertEqual(pm.local_path('local/ui/run', repo=self.repo),
                         (self.repo / 'local/ui/run').resolve())

    def test_outside_local_is_refused(self):
        for value in ('docs/x.json', 'local/../docs/x.json', '../elsewhere.json', str(self.repo / 'tools' / 'x.json'),
                      'local'):
            with self.assertRaises(ValueError, msg=value):
                pm.local_path(value, repo=self.repo)

    def test_movie_folder_one_level_down_is_enough(self):
        # Its parent also receives the extracted movies and textures, which must stay in local/ too.
        self.assertEqual(pm.local_path('local/run', repo=self.repo), (self.repo / 'local/run').resolve())

if __name__ == '__main__':
    unittest.main()
