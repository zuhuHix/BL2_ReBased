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

    def run_hook(self, stdin, root=None, cwd=None):
        env = dict(os.environ, CLAUDE_PROJECT_DIR=str(root or self.root))
        done = subprocess.run([sys.executable, '-I', str(HOOK)], input=stdin, capture_output=True, env=env, cwd=cwd,
                              timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(done.stderr, b'')
        return done.stdout.decode('utf-8').strip()

    def answer(self, file_path, root=None, cwd=None):
        """The hook's answer for a path as the tool would pass it: None when it says nothing."""
        out = self.run_hook(json.dumps({'tool_input': {'file_path': file_path}}).encode('utf-8'), root=root, cwd=cwd)
        return json.loads(out) if out else None

    def decide(self, rel, root=None):
        """The hook's answer for a repository-relative path, joined to the root with the platform's separator."""
        return self.answer(os.path.join(str(root or self.root), *rel.split('/')), root=root)

    def decision(self, answer):
        return None if answer is None else answer['hookSpecificOutput']['permissionDecision']

    def test_ordinary_files_pass_in_silence(self):
        for rel in ('docs/notes.md', 'src/vm.cpp', 'crates/ow-vm/src/lib.rs', 'tools/package.cpp.md',
                    'crates/ow-packages/src/lib.rs', 'other/src/package.cpp'):
            self.assertIsNone(self.decide(rel), rel)

    def test_only_the_root_build_files_are_sensitive(self):
        # AGENTS.md lists the root CMakeLists.txt and the workspace's Cargo.toml and rust-toolchain.toml.
        for rel in ('tests/CMakeLists.txt', 'docs/examples/CMakeLists.txt', 'third_party/lzokay/CMakeLists.txt',
                    'crates/ow-vm/Cargo.toml', 'crates/ow-host/rust-toolchain.toml', 'tools/Cargo.toml'):
            self.assertIsNone(self.decide(rel), rel)
        for rel in ('CMakeLists.txt', 'Cargo.toml', 'rust-toolchain.toml'):
            self.assertEqual(self.decision(self.decide(rel)), 'allow', rel)

    def test_sensitive_files_are_allowed_with_the_reminder(self):
        for rel in ('src/package.cpp', 'src/container.cpp', 'src/container.hpp', 'CMakeLists.txt',
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
        for rel in ('THIRD_PARTY.md', 'LICENSE', 'COPYING.txt', 'crates/ow-package/LICENSE-MIT', 'docs/THIRD_PARTY.md'):
            self.assertEqual(self.decision(self.decide(rel)), 'ask', rel)

    def test_game_files_and_analysis_databases_are_refused(self):
        for rel in ('invented.upk', 'local/invented.gzf', 'crates/ow-package/invented.tfc', 'src/invented.i64'):
            self.assertEqual(self.decision(self.decide(rel)), 'deny', rel)

    def test_a_path_outside_the_project_is_not_the_workspace_file(self):
        other = Path(self.tmp.name) / 'elsewhere'
        for rel in ('Cargo.toml', 'rust-toolchain.toml', 'CMakeLists.txt', 'crates/ow-package/src/lib.rs'):
            self.assertIsNone(self.answer(os.path.join(str(other), *rel.split('/'))), rel)
        # A game file is refused wherever it lies, and a licence file asks.
        self.assertEqual(self.decision(self.answer(os.path.join(str(other), 'invented.upk'))), 'deny')
        self.assertEqual(self.decision(self.answer(os.path.join(str(other), 'LICENSE'))), 'ask')

    def test_dot_dot_is_resolved_after_the_separators(self):
        # Invented roots in both spellings: the hook must give the same answer on every platform.
        for root, sep in (('C:\\invented\\repo', '\\'), ('/invented/repo', '/')):
            def at(rel):
                return root + sep + rel.replace('/', sep)
            # Leaves a sensitive folder through `..`: not sensitive.
            self.assertIsNone(self.answer(at('crates/ow-package/../ow-vm/src/lib.rs'), root=root), root)
            self.assertIsNone(self.answer(at('src/../docs/package.cpp'), root=root), root)
            # Arrives in one through `..`: sensitive.
            self.assertEqual(self.decision(self.answer(at('crates/ow-vm/../ow-package/src/lib.rs'), root=root)), 'allow', root)
            self.assertEqual(self.decision(self.answer(at('src/../Cargo.toml'), root=root)), 'allow', root)
            self.assertEqual(self.decision(self.answer(at('docs/./../CMakeLists.txt'), root=root)), 'allow', root)
            # Climbs out of the repository: the workspace file's name elsewhere is not the workspace file.
            self.assertIsNone(self.answer(at('../Cargo.toml'), root=root), root)

    def test_windows_long_path_prefixes_are_stripped(self):
        root = 'C:\\invented\\repo'
        self.assertEqual(self.decision(self.answer('\\\\?\\C:\\invented\\repo\\Cargo.toml', root=root)), 'allow')
        self.assertEqual(self.decision(self.answer('\\\\?\\C:\\invented\\repo\\crates\\ow-package\\src\\lib.rs', root=root)),
                         'allow')
        self.assertIsNone(self.answer('\\\\?\\C:\\invented\\repo\\crates\\ow-vm\\Cargo.toml', root=root))
        # The prefix on the root as well, and a drive letter in the other case.
        self.assertEqual(self.decision(self.answer('c:/invented/repo/src/package.cpp', root='\\\\?\\C:\\invented\\repo')),
                         'allow')
        # The UNC form of the prefix.
        unc = '\\\\invented-server\\share\\repo'
        self.assertEqual(self.decision(self.answer('\\\\?\\UNC\\invented-server\\share\\repo\\rust-toolchain.toml', root=unc)),
                         'allow')
        self.assertIsNone(self.answer('\\\\?\\UNC\\invented-server\\share\\elsewhere\\Cargo.toml', root=unc))
        # With the real temporary root, in the platform's own spelling behind the prefix.
        real = '\\\\?\\' + os.path.join(str(self.root), 'Cargo.toml').replace('/', '\\')
        self.assertEqual(self.decision(self.answer(real)), 'allow')

    def test_a_relative_path_is_read_from_the_working_directory(self):
        # The root as a child process names it: a temporary folder can have a second spelling (a short name, a link).
        seen = subprocess.run([sys.executable, '-I', '-c', 'import os; print(os.getcwd())'], capture_output=True,
                              cwd=str(self.root), timeout=60).stdout.decode('utf-8').strip()
        self.assertEqual(self.decision(self.answer('Cargo.toml', root=seen, cwd=str(self.root))), 'allow')
        self.assertIsNone(self.answer(os.path.join('crates', 'ow-vm', 'Cargo.toml'), root=seen, cwd=str(self.root)))

    def test_input_it_cannot_read_passes_in_silence(self):
        # No decision and exit 0 (run_hook checks the exit code and that nothing was written to stderr).
        for stdin in (b'not json', b'', b'{}', b'null', b'5', b'[]', b'"text"', b'{"tool_input": null}',
                      b'{"tool_input": 5}', b'{"tool_input": []}', b'{"tool_input": {}}',
                      b'{"tool_input": {"file_path": null}}', b'{"tool_input": {"file_path": 5}}',
                      b'{"tool_input": {"file_path": ["Cargo.toml"]}}', b'{"tool_input": {"file_path": ""}}'):
            self.assertEqual(self.run_hook(stdin), '', stdin)


if __name__ == '__main__':
    unittest.main()
