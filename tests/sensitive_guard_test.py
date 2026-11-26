"""Checks of .claude/hooks/sensitive_guard.py on invented paths: what it blocks, asks about, lets through with the
reminder, and lets through in silence. No file is written; the hook only reads the path in its input.
AI-assisted (Claude Code)."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

HOOK = Path(__file__).resolve().parents[1] / '.claude' / 'hooks' / 'sensitive_guard.py'


class GuardTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.tmp.name) / 'repo'
        cls.root.mkdir()

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def run_hook(self, stdin):
        env = dict(os.environ, CLAUDE_PROJECT_DIR=str(self.root))
        done = subprocess.run([sys.executable, '-I', str(HOOK)], input=stdin, capture_output=True, env=env, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr)
        return done.stdout.decode('utf-8').strip()

    def decide(self, rel, root=None):
        """The hook's answer for a repository-relative path: None when it says nothing."""
        path = os.path.join(str(root or self.root), *rel.split('/'))
        out = self.run_hook(json.dumps({'tool_input': {'file_path': path}}).encode('utf-8'))
        return json.loads(out) if out else None

    def test_ordinary_files_pass_in_silence(self):
        for rel in ('docs/notes.md', 'src/vm.cpp', 'crates/ow-vm/src/lib.rs', 'crates/ow-vm/Cargo.toml',
                    'crates/ow-host/rust-toolchain.toml', 'tools/package.cpp.md', 'crates/ow-packages/src/lib.rs'):
            self.assertIsNone(self.decide(rel), rel)

    def test_sensitive_files_are_allowed_with_the_reminder(self):
        for rel in ('src/package.cpp', 'src/container.cpp', 'src/container.hpp', 'CMakeLists.txt', 'tests/CMakeLists.txt',
                    'crates/ow-container/src/lzo.rs', 'crates/ow-container/Cargo.toml', 'crates/ow-package/src/lib.rs',
                    'crates/ow-package/src/reader/tests.rs', 'Cargo.toml', 'rust-toolchain.toml'):
            answer = self.decide(rel)
            self.assertIsNotNone(answer, rel)
            hook = answer['hookSpecificOutput']
            self.assertEqual(hook['permissionDecision'], 'allow', rel)
            reason = hook['permissionDecisionReason']
            self.assertEqual(hook['additionalContext'], reason, rel)
            self.assertIn(f"'{rel}'", reason)
            self.assertIn('no loosened bounds check', reason)
            self.assertIn('decision file', reason)
            self.assertIn('WARN THE MAINTAINER in your final message', reason)
            self.assertIn('owed even when this hook is silent', reason)
            # The reminder enters the session on every such edit: keep it short.
            self.assertLess(len(reason), 360, rel)

    def test_licence_and_provenance_files_ask(self):
        for rel in ('THIRD_PARTY.md', 'LICENSE', 'COPYING.txt', 'crates/ow-package/LICENSE-MIT'):
            answer = self.decide(rel)
            self.assertIsNotNone(answer, rel)
            self.assertEqual(answer['hookSpecificOutput']['permissionDecision'], 'ask', rel)

    def test_game_files_and_analysis_databases_are_refused(self):
        for rel in ('invented.upk', 'local/invented.gzf', 'crates/ow-package/invented.tfc', 'src/invented.i64'):
            answer = self.decide(rel)
            self.assertIsNotNone(answer, rel)
            self.assertEqual(answer['hookSpecificOutput']['permissionDecision'], 'deny', rel)

    def test_a_path_outside_the_project_is_not_the_workspace_file(self):
        other = Path(self.tmp.name) / 'elsewhere'
        for rel in ('Cargo.toml', 'rust-toolchain.toml', 'crates/ow-package/src/lib.rs'):
            self.assertIsNone(self.decide(rel, root=other), rel)

    def test_input_that_is_not_json_passes_in_silence(self):
        self.assertEqual(self.run_hook(b'not json'), '')
        self.assertEqual(self.run_hook(b'{}'), '')


if __name__ == '__main__':
    unittest.main()
