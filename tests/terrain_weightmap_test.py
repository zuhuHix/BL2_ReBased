"""Synthetic PF_G8 correspondence diagnostics; no game-derived bytes."""
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import zlib

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from terrain_decode import (_png_gray, _score_field,
                            analyze_weightmap_correspondence)


def png_rgba(rows):
    height, width = len(rows), len(rows[0])
    scanlines = b''.join(b'\x00' + bytes(channel for value in row for channel in (value,) * 4)
                         for row in rows)
    def chunk(kind, body):
        return (struct.pack('>I', len(body)) + kind + body +
                struct.pack('>I', zlib.crc32(kind + body) & 0xffffffff))
    return (b'\x89PNG\r\n\x1a\n' +
            chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)) +
            chunk(b'IDAT', zlib.compress(scanlines)) + chunk(b'IEND', b''))


class TerrainWeightmapTests(unittest.TestCase):
    def test_png_preserves_dimensions_and_channel_values(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'weightmap.png'
            path.write_bytes(png_rgba([[0, 7, 255], [13, 42, 99]]))
            decoded = _png_gray(path)
        self.assertEqual((decoded['width'], decoded['height']), (3, 2))
        self.assertEqual(decoded['values'], [[0, 7, 255], [13, 42, 99]])
        self.assertEqual(len(decoded['sha256']), 64)

    def test_exact_nonconstant_crop_is_reported_with_coordinates(self):
        source = [[9, 9, 9, 9], [9, 1, 2, 3], [9, 4, 5, 6], [9, 7, 8, 9]]
        alpha = {'width': 3, 'height': 3,
                 'maps': [[1, 2, 3, 4, 5, 6, 7, 8, 9]]}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'weightmap.png'
            path.write_bytes(png_rgba(source))
            report = analyze_weightmap_correspondence(
                alpha, [{'identity': 'Synthetic:TerrainWeightMapTexture_0', 'file': path}])
        self.assertEqual(report['resolution'], 'verified_exact_nonconstant')
        exact = report['verified_exact_nonconstant']
        self.assertEqual(exact[0]['crop'], [1, 1, 3, 3])
        self.assertEqual(exact[0]['resampling'], 'none')
        self.assertEqual(exact[0]['mae'], 0)

    def test_score_rejects_shape_mismatch(self):
        with self.assertRaises(ValueError):
            _score_field([[1, 2]], [[1]])


if __name__ == '__main__':
    unittest.main()
