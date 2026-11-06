"""Synthetic test of tools/check_overlay_data.py: empty files in a temporary folder, no game data."""
import importlib.util
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location('check_overlay_data', Path(__file__).parents[1] / 'tools' / 'check_overlay_data.py')
tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tool)


class CheckOverlayData(unittest.TestCase):
    def test_reports_exactly_the_absent_file(self):
        with tempfile.TemporaryDirectory() as folder:
            for name, _, _ in tool.REQUIRED:
                target = Path(folder) / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(b'')
            self.assertEqual(tool.main(['--movies', folder]), 0)
            (Path(folder) / 'UI_Inworld' / 'harness.swf').unlink()
            self.assertEqual([entry[0] for entry in tool.missing(folder)], ['UI_Inworld/harness.swf'])
            self.assertEqual(tool.main(['--movies', folder]), 1)
            self.assertEqual(tool.main(['--movies', folder, '--fail-on', 'UI_Inworld']), 1)
            self.assertEqual(tool.main(['--movies', folder, '--fail-on', 'UI_HUD']), 0)  # still printed, not fatal
            self.assertEqual(tool.main(['--movies', folder, '--warn-only']), 0)


if __name__ == '__main__':
    unittest.main()
