"""Synthetic malformed-input coverage for diagnostic-only terrain prefixes."""
import math
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from terrain_decode import decode_terrain, decode_component


def fixture(prefix, tail, properties=()):
    payload = bytes(prefix + 8) + tail
    return payload, {'property_offset': prefix, 'consumed_bytes': 8,
                     'trailing_bytes': len(tail), 'properties': list(properties)}


def prop(name, value):
    return {'name': name, 'value': value, 'status': 'decoded'}


class TerrainTests(unittest.TestCase):
    def terrain(self, tail=None):
        if tail is None:
            tail = struct.pack('<I4HI', 4, 32768, 32769, 32770, 32771, 4) + bytes([0, 1, 2, 255]) + b'opaque'
        return fixture(26, tail, [prop('NumPatchesX', 1), prop('NumPatchesY', 1)])

    def test_height_prefix_retains_unknown_flags_and_tail(self):
        result = decode_terrain(*self.terrain())
        self.assertEqual(result['heights'], [32768, 32769, 32770, 32771])
        self.assertEqual(result['flags'], [0, 1, 2, 255])
        self.assertEqual(result['opaque_tail_bytes'], 6)
        self.assertEqual(result['topology'], 'UNVERIFIED')

    def test_truncation_at_every_array_byte(self):
        payload, data = self.terrain()
        for length in range(34, 54):
            with self.subTest(length=length), self.assertRaises(ValueError):
                decode_terrain(payload[:length], {**data, 'trailing_bytes': length - 34})

    def test_bad_counts(self):
        payload, data = self.terrain()
        for offset in (34, 46):
            damaged = bytearray(payload)
            struct.pack_into('<I', damaged, offset, 5)
            with self.assertRaises(ValueError):
                decode_terrain(damaged, data)

    def test_no_offset_scan_or_extent_guess(self):
        payload, data = self.terrain()
        for update in ({'property_offset': 4}, {'consumed_bytes': 9}, {'trailing_bytes': 0}):
            with self.assertRaises(ValueError):
                decode_terrain(payload, {**data, **update})

    def test_budget_and_nonfinite_transform(self):
        payload, data = self.terrain()
        for properties in ([prop('NumPatchesX', 1_000_000), prop('NumPatchesY', 1_000_000)],
                           data['properties'] + [prop('DrawScale', math.inf)]):
            with self.assertRaises(ValueError):
                decode_terrain(payload, {**data, 'properties': properties})

    def test_sparse_tree_prefix(self):
        root = struct.pack('<6fBI4H', 0, 0, 0, 1, 1, 1, 1, 0, 1, 65535, 65535, 65535)
        leaf = struct.pack('<6fBI4H', 0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1)
        payload, data = fixture(8, struct.pack('<I', 2) + root + leaf + b'opaque')
        result = decode_component(payload, data)
        self.assertEqual(len(result['nodes']), 2)
        self.assertEqual(result['opaque_tail_bytes'], 6)
        for length in range(16, 94):
            with self.subTest(length=length), self.assertRaises(ValueError):
                decode_component(payload[:length], {**data, 'trailing_bytes': length - 16})
        for offset, fmt, value in [(49, '<I', 2), (53, '<H', 0), (20, '<f', math.nan)]:
            damaged = bytearray(payload)
            struct.pack_into(fmt, damaged, offset, value)
            with self.assertRaises(ValueError):
                decode_component(damaged, data)


if __name__ == '__main__':
    unittest.main()
