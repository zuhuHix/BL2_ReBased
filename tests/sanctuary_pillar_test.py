"""Synthetic checks for the Sanctuary spire Luminosity x Color bake."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from prepare_sanctuary_pillar import bake_spire_diffuse

from PIL import Image


class SpireDiffuseTest(unittest.TestCase):
    def bake(self, channel, atlas):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            # Color: three vertical strips (red, green, blue tints) at 50%.
            color = Image.new('RGB', (30, 4))
            for x in range(30):
                for y in range(4):
                    tint = [64, 64, 64]
                    tint[x // 10] = 128
                    color.putpixel((x, y), tuple(tint))
            color.save(root / 'color.png')
            # Luminosity: R=255, G=128, B=0 everywhere.
            Image.new('RGB', (8, 8), (255, 128, 0)).save(root / 'lum.png')
            bake_spire_diffuse(root / 'color.png', root / 'lum.png', channel, atlas, root / 'out.png')
            out = Image.open(root / 'out.png')
            return out.size, out.convert('RGB').getpixel((4, 4))

    def test_uses_luminosity_size_channel_and_strip(self):
        size, pixel = self.bake(0, (1/3, 1.0, 1/3, 0.0))
        self.assertEqual(size, (8, 8))
        # 2 x 1.0 x (64|128)/255, clamped: middle (green-tinted) strip.
        self.assertEqual(pixel, (128, 255, 128))

    def test_darker_channel_scales_tint(self):
        _, pixel = self.bake(2, (1/3, 1.0, 0.0, 0.0))
        self.assertEqual(pixel, (0, 0, 0))

    def test_rejects_vertical_strip(self):
        with self.assertRaises(ValueError):
            self.bake(0, (1.0, 0.5, 0.0, 0.5))


if __name__ == '__main__':
    unittest.main()
