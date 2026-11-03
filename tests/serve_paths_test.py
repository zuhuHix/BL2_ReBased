"""Synthetic test for tools/hud_overlay/serve.py: files served by the stock fallback stay inside the movies root, also
through a symlink or junction (review 1 of PR #64). No server is started; translate_path is called directly."""
import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / 'tools' / 'hud_overlay' / 'serve.py'
spec = importlib.util.spec_from_file_location('ow_serve', SCRIPT)
serve = importlib.util.module_from_spec(spec)
spec.loader.exec_module(serve)


def link_dir(link, target):
    """A directory link: a symlink when allowed, else (Windows without the privilege) a junction."""
    try:
        os.symlink(target, link, target_is_directory=True)
    except OSError:
        if os.name != 'nt':
            raise
        subprocess.run(['cmd', '/c', 'mklink', '/J', str(link), str(target)], check=True, capture_output=True)


class ServePathsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name)
        self.root, self.outside = base / 'movies', base / 'outside'
        (self.root / 'UI_HUD').mkdir(parents=True)
        (self.root / 'UI_HUD' / 'HUD.swf').write_bytes(b'movie')
        self.outside.mkdir()
        (self.outside / 'secret.txt').write_text('tracked file')
        link_dir(self.root / 'escape', self.outside)
        self.handler = serve.Handler.__new__(serve.Handler)
        self.handler.directory = str(self.root)

    def tearDown(self):
        link = self.root / 'escape'
        if link.is_symlink() or link.exists():
            os.rmdir(link) if os.name == 'nt' else link.unlink()
        self.tmp.cleanup()

    def test_files_inside_the_root_are_served(self):
        self.assertEqual(Path(self.handler.translate_path('/UI_HUD/HUD.swf')), (self.root / 'UI_HUD' / 'HUD.swf').resolve())

    def test_links_and_dot_dot_out_of_the_root_are_refused(self):
        for url in ('/escape/secret.txt', '/escape/', '/../outside/secret.txt', '/UI_HUD/../../outside/secret.txt'):
            translated = Path(self.handler.translate_path(url))
            self.assertTrue(translated.is_relative_to(self.root.resolve()), url)
            self.assertFalse(translated.exists(), url)


if __name__ == '__main__':
    unittest.main()
