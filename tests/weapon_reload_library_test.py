"""Synthetic checks for the per-type reload and arms clip plan (invented names, no game data)."""
import contextlib
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_reload_library as library


class ReloadPlanTests(unittest.TestCase):
    TYPES = {
        'WT.A': {'weapon_anim_set': 'Anim.RocketLauncher', 'socket_table': 'Pkg.Mesh_Rocket', 'reload_animation': 'Reload_B'},
        'WT.B': {'weapon_anim_set': 'Anim.RocketLauncher', 'socket_table': 'Pkg.Mesh_Rocket', 'reload_animation': 'Reload_A'},
        'WT.C': {'weapon_anim_set': 'Anim.Pistol', 'socket_table': 'Pkg.Mesh_Pistol', 'reload_animation': None},
        'WT.D': {'socket_table': 'Pkg.Mesh_None', 'reload_animation': 'Reload_C'},
    }

    def test_clips_group_by_set_with_the_library_folder_and_mesh(self):
        plan = library.reload_plan(self.TYPES)
        self.assertEqual(list(plan), ['Pistol', 'RocketLauncher'])
        self.assertEqual(plan['RocketLauncher'], {'kind': 'Launcher', 'mesh': 'Mesh_Rocket', 'clips': ['Reload_A', 'Reload_B']})
        self.assertEqual(plan['Pistol'], {'kind': 'Pistol', 'mesh': 'Mesh_Pistol', 'clips': []})

    def test_a_set_of_its_own_gets_the_base_clips_and_the_pistol_only_its_reloads(self):
        self.assertEqual(library.arms_clips('Pistol', ['Reload_A']), ['Reload_A'])
        clips = library.arms_clips('Shotgun', ['Reload_A'])
        self.assertIn('Idle', clips)
        self.assertEqual(clips[-1], 'Reload_A')

    def test_a_clip_not_exported_is_named_and_not_converted(self):
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / 'Reload_A.md5anim').write_text('', encoding='utf-8')
            present, missing = library.found_clips(folder, ['Reload_A', 'Reload_B'])
        self.assertEqual(list(present), ['Reload_A'])
        self.assertEqual(missing, ['Reload_B'])

    def test_import_lines_keep_spaces_and_quotes_inside_one_argument(self):
        arms, weapons = library.import_lines(["A=C:\\my dir\\it's.json"], ['K=C:\\my dir\\k.json'])
        self.assertIn("= 'A=C:\\my dir\\it''s.json' then", arms)
        self.assertTrue(weapons.endswith("-Reloads 'K=C:\\my dir\\k.json'"))


class MainTests(unittest.TestCase):
    """The step on invented tables, with UModel and the converter replaced."""

    TYPES = {'WT.A': {'weapon_anim_set': 'Anim.Shotgun', 'socket_table': 'Pkg.Mesh_S', 'reload_animation': 'Reload_A'}}

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.repo = Path(temporary.name) / 'repo'
        (self.repo / 'docs').mkdir(parents=True)
        (self.repo / 'local').mkdir()
        self.table = self.repo / 'local/view_types.json'
        self.refs = self.repo / 'local/refs'
        self.refs.mkdir()
        (self.refs / 'Shotgun.json').write_text('{}', encoding='utf-8')
        self.arms_ref = self.repo / 'local/ref_pose.json'
        self.arms_ref.write_text('{}', encoding='utf-8')
        self.out = self.repo / 'local/reload'

    def run_main(self, types, exported=('Reload_A',), out=None):
        self.table.write_text(json.dumps({'types': types}), encoding='utf-8')
        umodel = self.out / 'umodel'

        def export(_umodel, _game, _out, package, name, kind):
            if kind != 'AnimSet':
                return
            folder = umodel / package / 'AnimSet' / name
            folder.mkdir(parents=True, exist_ok=True)
            for clip in exported:
                (folder / f'{clip}.md5anim').write_text('', encoding='utf-8')
        argv = ['--game', 'g', '--umodel', 'u', '--view-types', str(self.table), '--references', str(self.refs),
                '--arms-reference', str(self.arms_ref), '--out', str(out or self.out)]
        stdout = io.StringIO()
        with mock.patch.object(library, 'export', export), \
                mock.patch.object(library.converter, 'convert', lambda _m, _r, clips, **_k: {c: {} for c in clips}), \
                contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stdout):
            code = library.main(argv, repo=self.repo)
        return code, stdout.getvalue()

    def test_a_good_run_writes_both_results_and_names_the_imports(self):
        code, text = self.run_main(self.TYPES, exported=('Reload_A',) + library.BASE_CLIPS)
        self.assertEqual(code, 0, text)
        self.assertEqual(list(json.loads((self.out / 'weapons/Shotgun.json').read_text(encoding='utf-8'))), ['Reload_A'])
        self.assertTrue((self.out / 'arms/Shotgun.json').is_file())
        self.assertIn('OPENWILLOW_CHARACTER_ANIMS', text)

    def test_a_table_without_reload_clips_fails_and_writes_nothing(self):
        for types in ({}, {'WT.A': dict(self.TYPES['WT.A'], reload_animation=None)}):
            code, text = self.run_main(types)
            self.assertEqual(code, 1, text)
            self.assertFalse(self.out.exists())

    def test_a_missing_reference_fails_before_anything_is_written(self):
        (self.refs / 'Shotgun.json').unlink()
        code, text = self.run_main(self.TYPES)
        self.assertEqual(code, 1)
        self.assertIn('no reference pose', text)
        self.assertFalse(self.out.exists())

    def test_an_old_export_does_not_count_for_this_run(self):
        old = self.out / 'umodel/Startup/AnimSet/Shotgun/Reload_A.md5anim'
        old.parent.mkdir(parents=True)
        old.write_text('', encoding='utf-8')
        code, text = self.run_main(self.TYPES, exported=())
        self.assertEqual(code, 1)
        self.assertIn("MISSING ['Reload_A']", text)
        self.assertFalse(old.exists())
        self.assertFalse((self.out / 'weapons/Shotgun.json').exists(), 'no empty result')

    def test_output_outside_local_is_refused(self):
        with self.assertRaises(SystemExit):
            self.run_main(self.TYPES, out=self.repo / 'docs/reload')
        self.assertFalse((self.repo / 'docs/reload').exists())

    def test_output_through_a_junction_into_a_tracked_folder_is_refused(self):
        link = self.repo / 'local/link'
        made = subprocess.run(['cmd', '/c', 'mklink', '/J', str(link), str(self.repo / 'docs')], capture_output=True)
        if made.returncode:
            self.skipTest('could not make a junction here')
        try:
            with self.assertRaises(SystemExit):
                self.run_main(self.TYPES, out=link / 'reload')
            self.assertEqual(list((self.repo / 'docs').iterdir()), [])
        finally:
            link.rmdir()   # removes the junction, not its target

    def test_a_list_separator_in_the_output_path_is_refused(self):
        with self.assertRaises(SystemExit):
            self.run_main(self.TYPES, out=self.repo / 'local/re,load')


if __name__ == '__main__':
    unittest.main()
