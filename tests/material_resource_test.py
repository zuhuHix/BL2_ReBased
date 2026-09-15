"""Synthetic boundary checks for native resource observation tooling."""
import hashlib
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from material_resource_census import census, inspect_resource, observed_tail_layout


class ResourceTests(unittest.TestCase):
    def test_observed_layout_requires_exact_consumption(self):
        tail = struct.pack('<7I4II', 1, 2, 3, 4, 5, 6, 1, 7, 8, 9, 10, 11)
        result = observed_tail_layout(tail)
        self.assertEqual(result['records_u32'], [[7, 8, 9, 10]])
        self.assertEqual(result['prefix_words_u32'], [1, 2, 3, 4, 5, 6])
        self.assertEqual(result['final_word_u32'], 11)
        for end in range(len(tail)):
            with self.assertRaises(ValueError):
                observed_tail_layout(tail[:end])
        for count in (-1, 0, 2, 0x7fffffff):
            corrupt = bytearray(tail)
            struct.pack_into('<i', corrupt, 24, count)
            with self.assertRaises(ValueError):
                observed_tail_layout(corrupt)
        with self.assertRaises(ValueError):
            observed_tail_layout(tail + bytes(4))

    def fixture(self, tail=b''):
        payload = bytes(8) + struct.pack('<3i16s2i2i', 0, 0, 1, bytes(16), 1, 2, -3, 7) + tail
        return payload, dict(property_offset=4, consumed_bytes=4, trailing_bytes=len(payload)-8)

    def test_preserves_unaligned_opaque_bytes(self):
        tail = struct.pack('<2I', 0xffffffff, 0x12345678) + b'xyz'
        payload, data = self.fixture(tail)
        result = inspect_resource(payload, data)
        self.assertEqual(result['texture_references'], [-3, 7])
        self.assertEqual(result['opaque_words_u32'], [0xffffffff, 0x12345678])
        self.assertEqual(result['opaque_remainder_hex'], '78797a')
        self.assertEqual(result['opaque_sha256'], hashlib.sha256(tail).hexdigest())
        self.assertEqual(payload[result['opaque_offset']:], tail)
        self.assertEqual(result['graph_status'], 'unreconstructed')

    def test_empty_tail_and_invalid_boundary(self):
        payload, data = self.fixture()
        self.assertEqual(inspect_resource(payload, data)['opaque_bytes'], 0)
        with self.assertRaises(ValueError):
            inspect_resource(payload, dict(data, consumed_bytes=5))
        for end in range(8, len(payload)):
            with self.assertRaises(ValueError):
                inspect_resource(payload[:end], dict(data, trailing_bytes=end-8))

    def test_census_keeps_rejections_and_absent_expressions_distinct(self):
        payload, data = self.fixture()
        rows = {i: {'class': 'Engine.Material', 'data': dict(data, properties=[])}
                for i in range(1, 4)}
        rows[2]['data']['properties'] = [dict(name='Expressions', status='decoded', value=[{'index': 0}])]
        rows[3]['data']['trailing_bytes'] += 1

        class FakeScene:
            def load(self, package):
                return rows

            def identity(self, key):
                return f'{key[0]}:{key[1]}'

            def call(self, *args):
                return [{'index': i, 'payload': list(payload)} for i in args[2:]]

        report = census(FakeScene(), ['Synthetic'])
        self.assertEqual(report['summary']['materials'], 2)
        self.assertEqual(report['summary']['errors'], 1)
        self.assertIsNone(report['materials'][0]['expression_slots'])
        self.assertEqual(report['materials'][1]['non_null_expression_slots'], 0)
        self.assertEqual(report['summary']['materials_with_no_surviving_expression_slots'], 1)
        self.assertEqual(report['errors'][0]['source'], 'Synthetic:3')


if __name__ == '__main__':
    unittest.main()
