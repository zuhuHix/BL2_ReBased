"""Synthetic map discovery and selection checks; no game data or UE required."""
import json
import importlib.util
from contextlib import redirect_stderr
from io import StringIO
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

    def test_prepare_without_opt_in_runs_only_the_level_preparation(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            records = [{'map': 'Sanctuary_P', 'selectable': True}]
            with patch.object(viewer, 'ROOT', root), patch.object(viewer, 'catalog', return_value=records), \
                    patch.object(viewer.sys, 'argv', ['viewer', '--game', folder, '--map', 'Sanctuary_P',
                                                    '--action', 'prepare']), \
                    patch.object(viewer.subprocess, 'run') as run:
                viewer.main()
                self.assertEqual(run.call_count, 1)
                self.assertEqual(Path(run.call_args.args[0][1]).name, 'prepare_level.py')
                self.assertNotIn('--outer-shell', run.call_args.args[0])

    def test_outer_shell_opt_in_is_passed_to_the_level_preparation_only(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            records = [{'map': 'Sanctuary_P', 'selectable': True}]
            with patch.object(viewer, 'ROOT', root), patch.object(viewer, 'catalog', return_value=records), \
                    patch.object(viewer.sys, 'argv', ['viewer', '--game', folder, '--map', 'Sanctuary_P',
                                                    '--action', 'prepare', '--outer-shell',
                                                    '--sanctuary-geometry']), \
                    patch.object(viewer.subprocess, 'run') as run:
                viewer.main()
                commands = [entry.args[0] for entry in run.call_args_list]
                self.assertEqual(len(commands), 3)
                self.assertEqual(commands[0][-1], '--outer-shell')
                self.assertTrue(all('--outer-shell' not in command for command in commands[1:]))

    def test_sanctuary_geometry_preparation_runs_in_order_with_collision(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            game = root / 'game'
            reader = root / 'reader.exe'
            records = [{'map': 'Sanctuary_P', 'selectable': True}]
            with patch.object(viewer, 'ROOT', root), patch.object(viewer, 'catalog', return_value=records), \
                    patch.object(viewer.sys, 'argv', ['viewer', '--game', str(game), '--map', 'Sanctuary_P',
                                                    '--reader', str(reader), '--action', 'prepare',
                                                    '--sanctuary-geometry']), \
                    patch.object(viewer.subprocess, 'run') as run:
                viewer.main()

                commands = [entry.args[0] for entry in run.call_args_list]
                scene = root / 'local' / 'sanctuary'
                common = ['--game', str(game.resolve()), '--reader', str(reader.resolve())]
                self.assertEqual(
                    commands,
                    [[sys.executable, str(root / 'tools/prepare_level.py'), *common,
                      '--map', 'Sanctuary_P', '--output', str(scene)],
                     [sys.executable, str(root / 'tools/prepare_terrain.py'), *common,
                      '--scene', str(scene), '--collision'],
                     [sys.executable, str(root / 'tools/prepare_bsp.py'), *common,
                      '--scene', str(scene), '--collision']])
                for entry in run.call_args_list:
                    self.assertTrue(entry.kwargs['check'])
                    self.assertEqual(entry.kwargs['cwd'], root)

    def test_sanctuary_geometry_rejects_unsupported_map_before_launch(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            records = [{'map': 'Ash_P', 'selectable': True}]
            with patch.object(viewer, 'ROOT', root), patch.object(viewer, 'catalog', return_value=records), \
                    patch.object(viewer.sys, 'argv', ['viewer', '--game', folder, '--map', 'Ash_P',
                                                    '--action', 'prepare', '--sanctuary-geometry']), \
                    patch.object(viewer.subprocess, 'run') as run:
                with self.assertRaises(SystemExit) as error:
                    viewer.main()
                self.assertEqual(error.exception.code, 1)
                run.assert_not_called()

    def test_sanctuary_geometry_rejects_unsupported_action_before_launch(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            records = [{'map': 'Sanctuary_P', 'selectable': True}]
            with patch.object(viewer, 'ROOT', root), patch.object(viewer, 'catalog', return_value=records), \
                    patch.object(viewer.sys, 'argv', ['viewer', '--game', folder, '--map', 'Sanctuary_P',
                                                    '--action', 'import', '--sanctuary-geometry']), \
                    patch.object(viewer.subprocess, 'run') as run:
                with self.assertRaises(SystemExit) as error:
                    viewer.main()
                self.assertEqual(error.exception.code, 1)
                run.assert_not_called()

    def test_plain_sanctuary_prepare_rejects_existing_recovered_geometry(self):
        for policy in ('terrain_policy', 'bsp_policy'):
            with self.subTest(policy=policy), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                scene = root / 'local' / 'sanctuary'
                scene.mkdir(parents=True)
                (scene / 'scene.json').write_text(
                    json.dumps({'map': 'Sanctuary_P', policy: {}}), encoding='utf-8')
                records = [{'map': 'Sanctuary_P', 'selectable': True}]
                with patch.object(viewer, 'ROOT', root), patch.object(viewer, 'catalog', return_value=records), \
                        patch.object(viewer.sys, 'argv', ['viewer', '--game', folder, '--map', 'Sanctuary_P',
                                                        '--action', 'prepare']), \
                        patch.object(viewer.subprocess, 'run') as run:
                    stderr = StringIO()
                    with redirect_stderr(stderr):
                        with self.assertRaises(SystemExit) as error:
                            viewer.main()
                    self.assertEqual(error.exception.code, 1)
                    self.assertIn('--sanctuary-geometry', stderr.getvalue())
                    run.assert_not_called()

    def test_sanctuary_geometry_stops_and_propagates_preparation_failure(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            records = [{'map': 'Sanctuary_P', 'selectable': True}]
            failure = viewer.subprocess.CalledProcessError(17, ['prepare_terrain.py'])
            with patch.object(viewer, 'ROOT', root), patch.object(viewer, 'catalog', return_value=records), \
                    patch.object(viewer.sys, 'argv', ['viewer', '--game', folder, '--map', 'Sanctuary_P',
                                                    '--action', 'prepare', '--sanctuary-geometry']), \
                    patch.object(viewer.subprocess, 'run', side_effect=[None, failure]) as run:
                with self.assertRaises(SystemExit) as error:
                    viewer.main()
                self.assertEqual(error.exception.code, 1)
                self.assertEqual(run.call_count, 2)
                self.assertEqual(Path(run.call_args_list[0].args[0][1]).name, 'prepare_level.py')
                self.assertEqual(Path(run.call_args_list[1].args[0][1]).name, 'prepare_terrain.py')

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
