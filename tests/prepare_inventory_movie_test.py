"""Synthetic tag-order and malformed-input checks; no game data."""
import struct
import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from prepare_inventory_movie import defer_library_imports


def tag(code, body=b''):
    return struct.pack('<H', (code << 6) | len(body)) + body


def movie(tags, frames=1):
    # RECT with 1-bit coordinates, 24 fps, frame count.
    body = b'\x08\x00\x00\x18' + struct.pack('<H', frames) + tags
    return b'FWS\x09' + struct.pack('<I', 8 + len(body)) + body


class InventoryMovieTest(unittest.TestCase):
    def test_preserves_definitions_and_import_order(self):
        first, second = tag(71, b'one'), tag(57, b'two')
        define, export, end = tag(39, b'opaque'), tag(56, b'export'), tag(0)
        raw = movie(first + define + second + export + end)
        actual, count = defer_library_imports(raw)
        self.assertEqual(actual, movie(define + export + first + second + end))
        self.assertEqual(count, 2)
        self.assertEqual(defer_library_imports(actual)[0], actual)

    def test_rejects_timeline(self):
        with self.assertRaises(ValueError): defer_library_imports(movie(tag(0), 2))

    def test_rejects_truncated_and_missing_end(self):
        for raw in [b'', movie(tag(39, b'abc')),
                    movie(struct.pack('<H', (39 << 6) | 10) + b'a'),
                    movie(struct.pack('<H', (39 << 6) | 63) + b'a'),
                    movie(tag(0)+b'extra')]:
            with self.subTest(raw=raw), self.assertRaises(ValueError): defer_library_imports(raw)


if __name__ == '__main__': unittest.main()
