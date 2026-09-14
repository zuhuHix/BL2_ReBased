"""Synthetic map discovery and selection checks; no game data or UE required."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import sys
sys.path.insert(0, str(Path(__file__).parents[1] / 'tools'))

spec = importlib.util.spec_from_file_location('viewer', Path(__file__).parents[1] / 'tools/viewer.py')
viewer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(viewer)
from installed_content import cache_directory


class ViewerTests(unittest.TestCase):
    def test_install_required(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(ValueError, 'installed Borderlands'):
                viewer.catalog(Path(folder))

    def test_catalog_filters_and_rejects_ambiguous_names(self):
        with tempfile.TemporaryDirectory() as folder:
            game = Path(folder)
            for name in ('Binaries/Win32/Borderlands2.exe',
                         'WillowGame/CookedPCConsole/Zed_P.upk',
                         'WillowGame/CookedPCConsole/Ash_P.upk',
                         'WillowGame/CookedPCConsole/Ash_Dynamic.upk',
                         'WillowGame/CookedPCConsole/Fake_P.txt',
                         'WillowGame/CookedPCConsole/sub/ASH_P.UPK'):
                path = game / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.touch()
            records = viewer.catalog(game)
            self.assertEqual([r['map'] for r in records], ['Ash_P', 'Zed_P'])
            self.assertEqual(viewer.select(records, 'zed_p'), 'Zed_P')
            with self.assertRaisesRegex(ValueError, 'Ambiguous'):
                viewer.select(records, 'ash_p')
            with self.assertRaisesRegex(ValueError, 'not installed'):
                viewer.select(records, '../Other_P')

    def test_view_rejects_wrong_manifest_before_launch(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            scene = root / 'local/ash'
            scene.mkdir(parents=True)
            (scene / 'scene.json').write_text('{"map": "Other_P"}')
            records = [{'map': 'Ash_P', 'selectable': True}]
            with patch.object(viewer, 'ROOT', root), patch.object(viewer, 'catalog', return_value=records), \
                    patch.object(viewer.sys, 'argv', ['viewer', '--game', folder, '--map', 'Ash_P',
                                                    '--engine', folder]), \
                    patch.object(viewer.subprocess, 'run') as run:
                with self.assertRaises(SystemExit) as error:
                    viewer.main()
                self.assertEqual(error.exception.code, 1)
                run.assert_not_called()

    def test_dlc_discovery_is_opt_in_and_duplicate_caches_are_explicit(self):
        with tempfile.TemporaryDirectory() as folder:
            game = Path(folder)
            for name in ('Binaries/Win32/Borderlands2.exe',
                         'WillowGame/CookedPCConsole/Base_P.upk',
                         'DLC/Example/Content/Dlc_P.upk'):
                path = game / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.touch()
            self.assertEqual([r['map'] for r in viewer.catalog(game)], ['Base_P'])
            self.assertEqual([r['map'] for r in viewer.catalog(game, True)], ['Base_P', 'Dlc_P'])
            caches = [game / 'DLC/A/Textures.tfc', game / 'DLC/B/Textures.tfc']
            fallback = game / 'WillowGame/CookedPCConsole'
            self.assertEqual(cache_directory(caches, game / 'DLC/A/Map.upk', fallback), caches[0].parent)
            self.assertEqual(cache_directory(caches[:1], game / 'DLC/B/Map.upk', fallback), caches[0].parent)
            self.assertEqual(cache_directory([], game / 'Map.upk', fallback), fallback)
            with self.assertRaisesRegex(ValueError, 'Ambiguous texture cache'):
                cache_directory(caches, game / 'DLC/C/Map.upk', fallback)


if __name__ == '__main__':
    unittest.main()
