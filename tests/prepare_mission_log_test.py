"""Synthetic test for tools/prepare_mission_log.py's output guard (review 2 of PR #64): the extracted presentation data may
only be written inside the repository's ignored local/, also when the path goes through a symlink or junction."""
import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1] / 'tools'
sys.path.insert(0, str(TOOLS))
spec = importlib.util.spec_from_file_location('ow_prepare_mission_log', TOOLS / 'prepare_mission_log.py')
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)


def link(link_path, target, directory=True):
    """A symlink when allowed, else (Windows without the privilege) a junction for a directory."""
    try:
        os.symlink(target, link_path, target_is_directory=directory)
        return True
    except OSError:
        if os.name != 'nt' or not directory:
            return False
        subprocess.run(['cmd', '/c', 'mklink', '/J', str(link_path), str(target)], check=True, capture_output=True)
        return True


class OutputGuardTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name) / 'repo'
        (self.repo / 'local' / 'ui' / 'run').mkdir(parents=True)
        (self.repo / 'docs').mkdir()
        self.links = []

    def tearDown(self):
        for path in self.links:
            if path.is_symlink() or path.exists():
                os.rmdir(path) if path.is_dir() and os.name == 'nt' else path.unlink()
        self.tmp.cleanup()

    def test_inside_local_is_accepted(self):
        target = prepare.output_file(self.repo / 'local' / 'ui' / 'run', self.repo)
        self.assertEqual(target, (self.repo / 'local' / 'ui' / 'run' / 'missionlog.json').resolve())
        self.assertEqual(prepare.output_file(self.repo / 'local' / 'new' / 'dir', self.repo).parent.name, 'dir')

    def test_outside_local_is_refused(self):
        for output in (self.repo / 'docs', self.repo, self.repo / 'local' / '..' / 'docs', Path(self.tmp.name) / 'elsewhere'):
            with self.assertRaises(SystemExit, msg=str(output)):
                prepare.output_file(output, self.repo)

    def test_link_out_of_local_is_refused(self):
        escape = self.repo / 'local' / 'escape'
        self.assertTrue(link(escape, self.repo / 'docs'))
        self.links.append(escape)
        with self.assertRaises(SystemExit):
            prepare.output_file(escape, self.repo)

    def test_file_symlink_out_of_local_is_refused(self):
        run = self.repo / 'local' / 'ui' / 'run'
        if not link(run / 'missionlog.json', self.repo / 'docs' / 'missionlog.json', directory=False):
            self.skipTest('file symlinks need a privilege on this machine')
        self.links.append(run / 'missionlog.json')
        with self.assertRaises(SystemExit):
            prepare.output_file(run, self.repo)


if __name__ == '__main__':
    unittest.main()
