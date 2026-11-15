"""AI-assisted (Claude Code). Self-test of tools/rust_parity.py (docs/rust-port/ARCHITECTURE.md section 6; lane
W1-HARNESS).

Stdlib only (and git, which the runner uses to list tracked test scripts). Run: python tests/rust_parity_test.py

Each case makes a throwaway git repository in a temporary folder (test scripts under its tests/, added to its index
so the runner sees them as tracked), a synthetic manifest folder and two tiny stand-in binaries (Python scripts, which
the runner starts with its own Python), runs the runner on them with --repo in a child process and checks its
PASS/FAIL lines, summary and exit code. Nothing here reads the real manifest, a real binary or game data.

The stand-ins print their arguments and the OW_PARITY value they see. `--exit N` makes them exit with N, `--err TEXT`
writes TEXT to stderr; the Rust stand-in prints `rust-only` where the C++ one prints `cxx-only` when given `--split`,
and writes its stderr with a `rust:` prefix (the C++ one with `cxx:`), so stderr always differs between the two.
"""

import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import textwrap
import time
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
RUNNER = ROOT / 'tools' / 'rust_parity.py'

STAND_IN = textwrap.dedent('''\
    import os, sys
    WHO = {who!r}
    args = sys.argv[1:]
    code = 0
    if '--exit' in args:
        code = int(args[args.index('--exit') + 1])
    if '--err' in args:
        sys.stderr.write(WHO + ':' + args[args.index('--err') + 1] + '\\n')
    if '--split' in args:
        print(WHO + '-only')
    print('args=' + ' '.join(args))
    print('parity=' + os.environ.get('OW_PARITY', '<unset>'))
    sys.exit(code)
''')

# A test command for plain lines: checks the binary path it was given and the parity value it sees.
CHECKER = textwrap.dedent('''\
    import os, sys
    binary, want_parity, code = sys.argv[1], sys.argv[2], int(sys.argv[3])
    if not binary.endswith('rust_bin.py'):
        print('wrong binary: ' + binary); sys.exit(9)
    seen = os.environ.get('OW_PARITY', '<unset>')
    if seen != want_parity:
        print('parity ' + seen + ', wanted ' + want_parity); sys.exit(8)
    print('checker ran')
    sys.exit(code)
''')

# Stand-in pair that write only `text` to the given stream and exit with `code` (the same for both binaries).
SAME_OUTPUT = 'import sys\nsys.{stream}.write({text!r})\nsys.exit({code})\n'


class RunnerTest(unittest.TestCase):

    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix='ow-rust-parity-'))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.repo = self.tmp / 'repo'
        (self.repo / 'tests').mkdir(parents=True)
        subprocess.run(['git', 'init', '-q', str(self.repo)], check=True, capture_output=True)
        self.manifest = self.tmp / 'manifest'
        self.manifest.mkdir()
        self.rust = self.tmp / 'rust_bin.py'
        self.cxx = self.tmp / 'cxx_bin.py'
        self.rust.write_text(STAND_IN.format(who='rust'), encoding='utf-8')
        self.cxx.write_text(STAND_IN.format(who='cxx'), encoding='utf-8')
        self.add_script('checker_test.py', CHECKER)

    def add_script(self, name, text, track=True):
        """Writes tests/<name> in the throwaway repository and, unless track is False, adds it to the index."""
        (self.repo / 'tests' / name).write_text(text, encoding='utf-8')
        if track:
            subprocess.run(['git', '-C', str(self.repo), 'add', f'tests/{name}'], check=True, capture_output=True)

    def same_output(self, stream, text, code=0):
        for binary in (self.rust, self.cxx):
            binary.write_text(SAME_OUTPUT.format(stream=stream, text=text, code=code), encoding='utf-8')

    def write(self, name, text):
        (self.manifest / name).write_text(textwrap.dedent(text), encoding='utf-8')

    def check_line(self, want_parity, code=0, suffix=''):
        """A plain manifest line that runs the checker with the Rust binary."""
        return f'python tests/checker_test.py {{bin}} {want_parity} {code}{suffix}'

    def runner(self, *extra, env_parity=None, cxx=True, env_extra=None):
        env = dict(os.environ)
        env.pop('OW_PARITY', None)
        if env_parity is not None:
            env['OW_PARITY'] = env_parity
        env.update(env_extra or {})
        argv = [sys.executable, str(RUNNER), '--bin', str(self.rust), '--manifest-dir', str(self.manifest),
                '--repo', str(self.repo)]
        if cxx:
            argv += ['--cxx', str(self.cxx)]
        done = subprocess.run(argv + list(extra), capture_output=True, text=True, env=env, timeout=120)
        return done.returncode, done.stdout, done.stderr

    def verdicts(self, out):
        """{'file:line': 'PASS'|'FAIL'} from the runner's output."""
        return {m.group(2): m.group(1) for m in re.finditer(r'^(PASS|FAIL) (\S+:\d+) ', out, re.M)}

    def summary(self, out):
        m = re.search(r'^rust_parity: (\d+) passed, (\d+) failed, of (\d+) lines in (\d+) files', out, re.M)
        self.assertIsNotNone(m, out)
        return tuple(int(g) for g in m.groups())

    # Plain test-command lines.

    def test_passing_failing_reference_and_diff_lines(self):
        # The case the lane entry names: a passing, a failing, a mode=reference and a diff line, in two files.
        self.write('a.txt', f'''\
            # a comment and a blank line are skipped

            {self.check_line('<unset>')}
            {self.check_line('<unset>', code=1)}
            {self.check_line('reference', suffix=' mode=reference')}
            ''')
        self.write('b.txt', '''\
            diff --names some/path.upk
            ''')
        code, out, err = self.runner()
        self.assertEqual(code, 1, out + err)
        self.assertEqual(self.verdicts(out), {'a.txt:3': 'PASS', 'a.txt:4': 'FAIL', 'a.txt:5': 'PASS',
                                              'b.txt:1': 'PASS'}, out)
        self.assertEqual(self.summary(out), (3, 1, 4, 2))
        self.assertIn('exit code 1', out)
        self.assertIn('| checker ran', out)   # a failing command's output is shown

    def test_all_passing_exits_zero(self):
        self.write('a.txt', self.check_line('<unset>') + '\n')
        code, out, _ = self.runner()
        self.assertEqual(code, 0, out)
        self.assertEqual(self.summary(out), (1, 0, 1, 1))

    def test_the_callers_parity_variable_is_cleared(self):
        # A caller with OW_PARITY=reference set: plain lines still see it unset, reference lines see it set.
        self.write('a.txt', f'''\
            {self.check_line('<unset>')}
            {self.check_line('reference', suffix=' mode=reference')}
            ''')
        code, out, _ = self.runner(env_parity='reference')
        self.assertEqual(code, 0, out)
        self.assertEqual(self.summary(out), (2, 0, 2, 1))
        code, out, _ = self.runner(env_parity='somethingelse')
        self.assertEqual(code, 0, out)

    def test_a_line_wanting_reference_fails_without_the_marker(self):
        self.write('a.txt', self.check_line('reference') + '\n')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertIn('parity <unset>, wanted reference', out)

    def test_asserts_cannot_be_optimised_away(self):
        # Review r2's probe: an assert-based test passed under PYTHONOPTIMIZE=1 and with python -O. The runner removes
        # PYTHONOPTIMIZE, so the assert still fails; a -O option before the script is a bad line.
        self.add_script('asserts_test.py', 'import sys\nassert False, "an assert ran"\n')
        self.write('a.txt', '''\
            python tests/asserts_test.py {bin}
            python -O tests/asserts_test.py {bin}
            python -OO tests/asserts_test.py {bin}
            ''')
        code, out, _ = self.runner(env_extra={'PYTHONOPTIMIZE': '1'})
        self.assertEqual(code, 1, out)
        self.assertEqual(self.summary(out), (0, 3, 3, 1))
        self.assertIn('an assert ran', out)
        self.assertEqual(out.count('python -O strips the asserts'), 2, out)

    def test_a_test_line_must_run_a_tracked_test_script(self):
        # Review r2: the {bin} check alone let `python -c "print('{bin}')"` through.
        self.add_script('untracked_test.py', 'pass\n', track=False)
        self.write('a.txt', '''\
            python -c "print('{bin}')"
            python tests/untracked_test.py {bin}
            python tests/missing_test.py {bin}
            python tools/checker_test.py {bin}
            node tests/checker_test.py {bin}
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.summary(out), (0, 5, 5, 1))
        self.assertEqual(out.count('is not a tracked tests/*.py file'), 4, out)
        self.assertIn('a test command starts with python', out)

    def test_a_test_line_without_bin_is_a_bad_line(self):
        self.write('a.txt', 'python tests/checker_test.py somebinary <unset> 0\n')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertIn('a test command must name {bin}', out)

    def test_skipped_unittest_cases_fail_the_line(self):
        # A unittest file whose cases skip exits 0; the line still fails. One with no skip passes.
        for name, body in (('skips', 'raise unittest.SkipTest("no data")'), ('runs', 'pass')):
            self.add_script(f'{name}_test.py', textwrap.dedent(f'''\
                import sys, unittest
                class T(unittest.TestCase):
                    def test_a(self):
                        {body}
                    def test_b(self):
                        pass
                if __name__ == '__main__':
                    sys.argv[1:] = []
                    unittest.main()
                '''))
        self.write('a.txt', '''\
            python tests/skips_test.py {bin}
            python tests/runs_test.py {bin}
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.verdicts(out), {'a.txt:1': 'FAIL', 'a.txt:2': 'PASS'}, out)
        self.assertIn('the output reports skipped cases (skipped=1)', out)

    # Diff lines.

    def test_diff_compares_stdout(self):
        self.write('d.txt', '''\
            diff --split x
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertIn("stdout differs: line 1: C++ 'cxx-only', Rust 'rust-only'", out)

    def test_crlf_reads_as_lf_and_nothing_else_is_normalised(self):
        # The MSVC build writes text-mode CRLF, the Rust binary LF (maintainer's decision, 2026-11-15). A lone CR,
        # or CRLF against nothing, still differs.
        def pair(cxx_bytes, rust_bytes, stream='stdout'):
            for binary, data in ((self.cxx, cxx_bytes), (self.rust, rust_bytes)):
                binary.write_text('import sys\n'
                                  f'sys.stdout.buffer.write({(data if stream == "stdout" else b"out")!r})\n'
                                  f'sys.stderr.buffer.write({(data if stream == "stderr" else b"")!r})\n',
                                  encoding='utf-8')
        cases = [(b'a\r\nb\r\n', b'a\nb\n', 'stdout', 'PASS'),
                 (b'a\r\nb', b'a\nb', 'stdout', 'PASS'),
                 (b'a\rb\n', b'a\nb\n', 'stdout', 'FAIL'),
                 (b'a\r\n', b'a', 'stdout', 'FAIL'),
                 (b'e\r\n', b'e\n', 'stderr', 'PASS'),
                 (b'e\r\r\n', b'e\n', 'stderr', 'FAIL')]
        for cxx_bytes, rust_bytes, stream, want in cases:
            with self.subTest(cxx=cxx_bytes, rust=rust_bytes, stream=stream):
                pair(cxx_bytes, rust_bytes, stream)
                self.write('d.txt', 'diff x stderr\n' if stream == 'stderr' else 'diff x\n')
                code, out, _ = self.runner()
                self.assertEqual(self.verdicts(out), {'d.txt:1': want}, out)
                self.assertEqual(code, 0 if want == 'PASS' else 1, out)

    def test_an_expected_error_needs_the_exit_marker(self):
        # Review r2's probe: once the usage text is ported, both binaries exit 1 with the same usage on stderr, so
        # `stderr` alone must not let a mistyped flag pass. Only exit=N with the C++ exit code does.
        self.same_output('stderr', 'usage: ow-package <flag> <package>\n', code=1)
        self.write('d.txt', '''\
            diff --nmaes local/not-there.upk
            diff --nmaes local/not-there.upk stderr
            diff --nmaes local/not-there.upk exit=1 stderr
            diff --nmaes local/not-there.upk stderr exit=1
            diff --nmaes local/not-there.upk exit=1
            diff --nmaes local/not-there.upk exit=2 stderr
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.verdicts(out), {'d.txt:1': 'FAIL', 'd.txt:2': 'FAIL', 'd.txt:3': 'PASS',
                                              'd.txt:4': 'PASS', 'd.txt:5': 'FAIL', 'd.txt:6': 'FAIL'}, out)
        self.assertEqual(out.count('an expected error needs exit=1'), 2, out)
        self.assertIn('the C++ stdout is empty: a line with no output needs the stderr marker', out)
        self.assertIn('exit=2 expected, the C++ binary exited 1', out)

    def test_an_expected_error_needs_cxx_stderr(self):
        # The stand-ins exit 3 with the same stdout and no stderr: an error that says nothing fails even with exit=3.
        self.write('d.txt', '''\
            diff --exit 3 x exit=3
            diff --exit 3 --err boom x exit=3
            diff --exit 3 --err boom x exit=3 stderr
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.verdicts(out), {'d.txt:1': 'FAIL', 'd.txt:2': 'PASS', 'd.txt:3': 'FAIL'}, out)
        self.assertIn('the C++ binary exited 3 with empty stderr', out)
        self.assertIn("stderr differs: line 1: C++ 'cxx:boom', Rust 'rust:boom'", out)

    def test_both_binaries_refusing_the_same_way_fails(self):
        # Review r1's probe: every C++ error exits 1 with empty stdout, and so does a Rust usage error.
        for binary, who in ((self.rust, 'rust'), (self.cxx, 'cxx')):
            binary.write_text(f"import sys\nsys.stderr.write('{who}: bad command line\\n')\nsys.exit(1)\n",
                              encoding='utf-8')
        self.write('d.txt', '''\
            diff --nmaes local/not-there.upk
            diff --nmaes local/not-there.upk exit=1 stderr
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.verdicts(out), {'d.txt:1': 'FAIL', 'd.txt:2': 'FAIL'}, out)
        self.assertIn('an expected error needs exit=1', out)
        self.assertIn('stderr differs', out)

    def test_a_cxx_binary_that_writes_nothing_fails_whatever_the_markers(self):
        # Review r2: with `stderr`, two silent binaries agreed on everything (a Rust `fn main() {}` passed).
        self.same_output('stdout', '')
        self.write('d.txt', '''\
            diff --quiet x
            diff --quiet x stderr
            diff --quiet x exit=0 stderr
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.summary(out), (0, 3, 3, 1))
        self.assertEqual(out.count('the C++ binary wrote nothing'), 3, out)

    def test_empty_cxx_stdout_needs_the_stderr_marker(self):
        # Both exit 0 and write the same text to stderr only.
        self.same_output('stderr', 'note: nothing to list\n')
        self.write('d.txt', '''\
            diff --quiet x
            diff --quiet x stderr
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.verdicts(out), {'d.txt:1': 'FAIL', 'd.txt:2': 'PASS'}, out)
        self.assertIn('the C++ stdout is empty', out)

    def test_diff_with_different_exit_codes_fails(self):
        # Same stdout, but the C++ stand-in exits one higher than asked.
        self.cxx.write_text(STAND_IN.format(who='cxx').replace('sys.exit(code)', 'sys.exit(code + 1)'),
                            encoding='utf-8')
        self.write('d.txt', 'diff --exit 3 --err e x exit=4\n')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertIn('exit code: C++ 4, Rust 3', out)

    def test_stderr_is_compared_only_on_request(self):
        self.write('d.txt', '''\
            diff --err boom x
            diff --err boom x stderr
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.verdicts(out), {'d.txt:1': 'PASS', 'd.txt:2': 'FAIL'}, out)
        self.assertIn("stderr differs: line 1: C++ 'cxx:boom', Rust 'rust:boom'", out)

    def test_diff_reference_line_sets_the_variable_for_both_binaries(self):
        # Each stand-in exits 0 only when it sees OW_PARITY=reference (else the C++ one exits 7, the Rust one 6), so a
        # diff line passes only when both binaries got the variable. The markers may come in either order. Without
        # the marker, a caller's OW_PARITY=reference does not reach either binary.
        for binary, who in ((self.rust, 'rust'), (self.cxx, 'cxx')):
            other = '7' if who == 'cxx' else '6'
            binary.write_text(STAND_IN.format(who=who).replace(
                'sys.exit(code)', f"sys.exit(0 if os.environ.get('OW_PARITY') == 'reference' else {other})"),
                encoding='utf-8')
        self.write('d.txt', '''\
            diff x mode=reference
            diff x stderr mode=reference
            diff x mode=reference stderr
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 0, out)
        self.assertEqual(self.summary(out), (3, 0, 3, 1))
        self.write('d.txt', 'diff x\n')
        code, out, _ = self.runner(env_parity='reference')
        self.assertEqual(code, 1, out)
        self.assertIn('exit code: C++ 7, Rust 6', out)

    def test_a_timeout_stops_the_whole_process_tree(self):
        # The Rust stand-in starts a grandchild that holds the inherited pipes open for 60 s, then sleeps itself.
        # With a 2 s timeout the line fails quickly: the runner stops the tree of the PID it started. The bound sits
        # below stop_tree's 30 s fallback, so a tree stop that left the grandchild holding the pipes would fail it.
        self.rust.write_text(textwrap.dedent('''\
            import subprocess, sys, time
            subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])
            time.sleep(60)
            '''), encoding='utf-8')
        self.write('d.txt', 'diff x\n')
        started = time.monotonic()
        code, out, _ = self.runner('--timeout', '2')
        elapsed = time.monotonic() - started
        self.assertEqual(code, 1, out)
        self.assertIn('Rust binary: timed out after 2.0 s; stopped it and its child processes', out)
        self.assertLess(elapsed, 20, out)

    # Bad lines and the manifest folder.

    def test_misplaced_markers_comments_and_backslashes_are_bad_lines(self):
        self.write('a.txt', f'''\
            diff mode=reference --names x.upk
            diff stderr --names x.upk
            diff exit=1 --names x.upk
            diff --names x.upk  # inline comment
            diff --names local\\\\x.upk
            {self.check_line('<unset>')} # a comment
            {self.check_line('<unset>').replace(' {bin}', ' mode=reference {bin}')}
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.summary(out), (0, 7, 7, 1))
        self.assertEqual(out.count('must come at the end of the line'), 4, out)
        self.assertEqual(out.count("'#' inside a line"), 2, out)
        self.assertEqual(out.count('a backslash in the line'), 1, out)

    def test_bad_lines_fail(self):
        self.write('a.txt', '''\
            diff
            diff mode=reference
            stderr
            python tests/checker_test.py {bin} stderr
            python tests/checker_test.py {bin} exit=1
            diff x exit=1 exit=1
            python "unclosed
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.summary(out), (0, 7, 7, 1))
        self.assertEqual(out.count('bad line:'), 7, out)
        self.assertEqual(out.count('the stderr and exit= markers are for diff lines only'), 2, out)
        self.assertIn('more than one exit= marker', out)

    def test_other_entries_in_the_manifest_folder_fail(self):
        self.write('a.txt', self.check_line('<unset>') + '\n')
        self.write('README.md', 'not a manifest\n')
        self.write('pkg.text', self.check_line('<unset>') + '\n')
        self.write('pkg.md', self.check_line('<unset>') + '\n')
        (self.manifest / 'sub').mkdir()
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.summary(out), (1, 3, 4, 1))
        self.assertEqual(out.count('not a manifest file'), 3, out)
        for name in ('pkg.text', 'pkg.md', 'sub'):
            self.assertIn(f'FAIL {name} ', out)

    def test_an_empty_manifest_passes_and_says_so(self):
        (self.manifest / 'README.md').write_text('diff not a manifest line\n', encoding='utf-8')
        code, out, _ = self.runner()
        self.assertEqual(code, 0, out)
        self.assertEqual(self.summary(out), (0, 0, 0, 0))
        self.assertIn('the manifest holds no line yet', out)

    def test_file_selects_one_manifest(self):
        self.write('a.txt', self.check_line('<unset>', code=1) + '\n')
        self.write('b.txt', self.check_line('<unset>') + '\n')
        code, out, _ = self.runner('--file', 'b.txt')
        self.assertEqual(code, 0, out)
        self.assertEqual(self.summary(out), (1, 0, 1, 1))
        code, out, err = self.runner('--file', 'nope.txt')
        self.assertEqual(code, 2, out + err)
        self.assertIn('no manifest file nope.txt', err)

    def test_usage_errors(self):
        self.write('d.txt', 'diff x\n')
        code, _, err = self.runner('--cxx', str(self.tmp / 'missing.exe'))
        self.assertEqual(code, 2)
        self.assertIn('there is no C++ binary', err)
        code, _, err = self.runner('--bin', str(self.tmp / 'missing.exe'))
        self.assertEqual(code, 2)
        self.assertIn('no Rust binary', err)
        code, _, err = self.runner('--manifest-dir', str(self.tmp / 'nowhere'))
        self.assertEqual(code, 2)
        self.assertIn('no manifest folder', err)
        not_a_repo = self.tmp / 'not-a-repo'
        not_a_repo.mkdir()
        code, _, err = self.runner('--repo', str(not_a_repo))
        self.assertEqual(code, 2)
        self.assertIn('git ls-files failed', err)

    def test_no_cxx_needed_without_diff_lines(self):
        self.write('a.txt', self.check_line('<unset>') + '\n')
        code, out, _ = self.runner('--cxx', str(self.tmp / 'missing.exe'))
        self.assertEqual(code, 0, out)

    def test_the_real_manifest_parses(self):
        # The tracked manifest folder: every *.txt line parses against this repository's tracked tests (running it
        # needs the binaries; that is ctest's rust-parity).
        sys.path.insert(0, str(ROOT / 'tools'))
        try:
            import rust_parity
        finally:
            sys.path.pop(0)
        folder = ROOT / 'tests' / 'rust_parity.d'
        self.assertTrue((folder / 'README.md').is_file())
        tracked = rust_parity.tracked_test_scripts(ROOT)
        self.assertIn('tests/rust_parity_test.py', tracked)
        _, _, problems = rust_parity.read_manifest(folder, [], tracked)
        self.assertEqual(problems, [])


if __name__ == '__main__':
    unittest.main()
