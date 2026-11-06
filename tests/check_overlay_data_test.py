"""Synthetic test of tools/check_overlay_data.py: empty files in a temporary folder, no game data."""
import importlib.util
import os
import re
import subprocess
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

    def fill(self, folder):
        for name, _, _ in tool.REQUIRED:
            target = Path(folder) / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(b'')

    @unittest.skipUnless(os.name == 'nt', 'junctions are a Windows feature')
    def test_junctions_follow_the_servers_rule(self):
        with tempfile.TemporaryDirectory() as folder:
            real = Path(folder) / 'real'
            self.fill(real)
            # A junction AT the movies folder (or above it): both sides resolve, so everything is served.
            link = Path(folder) / 'linked'
            subprocess.run(['cmd', '/c', 'mklink', '/J', str(link), str(real)], check=True, capture_output=True)
            self.assertEqual(tool.missing(link), [])
            # A junction INSIDE the movies folder, pointing out of it: serve.py answers 404, and the check must say so.
            outside = Path(folder) / 'outside'
            outside.mkdir()
            inner = real / 'UI_Inworld'
            for child in inner.iterdir():
                child.replace(outside / child.name)
            inner.rmdir()
            subprocess.run(['cmd', '/c', 'mklink', '/J', str(inner), str(outside)], check=True, capture_output=True)
            self.assertTrue((inner / 'harness.swf').is_file())  # the plain test would have said it is there
            self.assertEqual(sorted(entry[0] for entry in tool.missing(real)), ['UI_Inworld/Weapon_Card.swf', 'UI_Inworld/harness.swf'])
            subprocess.run(['cmd', '/c', 'rmdir', str(inner)], check=True, capture_output=True)
            subprocess.run(['cmd', '/c', 'rmdir', str(link)], check=True, capture_output=True)

    def test_every_import_the_guard_requires_is_checked(self):
        guard = (Path(__file__).parents[1] / 'tools' / 'hud_overlay' / 'movie_guard.js').read_text(encoding='utf-8')
        for name in re.findall(r"'([\w/]+\.swf)'", guard):
            self.assertIn(name, [entry[0] for entry in tool.REQUIRED])


if __name__ == '__main__':
    unittest.main()
