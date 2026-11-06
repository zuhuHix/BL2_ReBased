"""tools/lootables_editor.py writes only under the repository's ignored local/ (synthetic paths, nothing game-derived)."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import lootables_editor as editor  # noqa: E402


class LocalOutputTest(unittest.TestCase):
    def test_a_path_under_local_is_accepted(self):
        self.assertEqual(editor.local_output('local/lootables/plan.json'),
                         (editor.ROOT / 'local/lootables/plan.json').resolve())

    def test_tracked_and_outside_paths_are_refused(self):
        for value in ('docs/plan.json', 'tools/plan.json', '../plan.json', 'local', 'local/../docs/plan.json',
                      str(Path(tempfile.gettempdir()) / 'local' / 'plan.json')):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, 'must be inside'):
                editor.local_output(value)

    def test_a_loaded_plan_cannot_point_its_output_at_a_tracked_folder(self):
        with tempfile.TemporaryDirectory() as folder:
            plan = Path(folder) / 'plan.json'
            plan.write_text(json.dumps({'out_dir': str(editor.ROOT / 'docs')}), encoding='utf-8')
            with self.assertRaisesRegex(ValueError, 'must be inside'):
                editor.load_plan(plan)

    def test_a_loaded_plan_inside_local_is_kept(self):
        with tempfile.TemporaryDirectory() as folder:
            plan = Path(folder) / 'plan.json'
            plan.write_text(json.dumps({'out_dir': str(editor.OUT)}), encoding='utf-8-sig')
            self.assertEqual(Path(editor.load_plan(plan)['out_dir']), editor.OUT.resolve())

    def test_the_plan_and_report_commands_refuse_a_tracked_output(self):
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / 'interactives.json'
            source.write_text(json.dumps({'objects': [], 'balances': {}, 'items': {}, 'assets': {}}), encoding='utf-8')
            with self.assertRaises(ValueError):
                editor.main(['plan', '--interactives', str(source), '--out', 'docs/plan.json'])
            self.assertFalse((editor.ROOT / 'docs/plan.json').exists())


if __name__ == '__main__':
    unittest.main()
