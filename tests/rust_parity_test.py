"""AI-assisted (Claude Code). Self-test of tools/rust_parity.py (docs/rust-port/ARCHITECTURE.md section 6; lane
W1-HARNESS).

Stdlib only. Run: python tests/rust_parity_test.py

Each case writes a synthetic manifest folder and two tiny stand-in binaries (Python scripts, which the runner starts
with its own Python) into a temporary folder, runs the runner on them in a child process and checks its PASS/FAIL
lines, summary and exit code. Nothing here reads the real manifest, a real binary or game data.

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


class RunnerTest(unittest.TestCase):

    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix='ow-rust-parity-'))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.manifest = self.tmp / 'manifest'
        self.manifest.mkdir()
        self.rust = self.tmp / 'rust_bin.py'
        self.cxx = self.tmp / 'cxx_bin.py'
        self.rust.write_text(STAND_IN.format(who='rust'), encoding='utf-8')
        self.cxx.write_text(STAND_IN.format(who='cxx'), encoding='utf-8')
        self.checker = self.tmp / 'checker.py'
        self.checker.write_text(CHECKER, encoding='utf-8')

    def write(self, name, text):
        (self.manifest / name).write_text(textwrap.dedent(text), encoding='utf-8')

    def check_line(self, want_parity, code=0, suffix=''):
        """A plain manifest line that runs the checker with the Rust binary."""
        checker = self.checker.as_posix()
        return f'python {checker} {{bin}} {want_parity} {code}{suffix}'

    def runner(self, *extra, env_parity=None, cxx=True):
        env = dict(os.environ)
        env.pop('OW_PARITY', None)
        if env_parity is not None:
            env['OW_PARITY'] = env_parity
        argv = [sys.executable, str(RUNNER), '--bin', str(self.rust), '--manifest-dir', str(self.manifest)]
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

    def test_diff_compares_stdout(self):
        self.write('d.txt', '''\
            diff --split x
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertIn("stdout differs: line 1: C++ 'cxx-only', Rust 'rust-only'", out)

    def test_an_expected_error_needs_the_stderr_marker(self):
        # Both stand-ins exit 3 with the same stdout. Without `stderr` the line fails (two binaries refusing the
        # arguments the same way must not pass); with it, equal stderr passes and different stderr fails.
        self.write('d.txt', '''\
            diff --exit 3 x
            diff --exit 3 x stderr
            diff --exit 3 --err boom x stderr
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.verdicts(out), {'d.txt:1': 'FAIL', 'd.txt:2': 'PASS', 'd.txt:3': 'FAIL'}, out)
        self.assertIn('a binary exited non-zero (C++ 3, Rust 3): an expected error needs the stderr marker', out)

    def test_both_binaries_refusing_the_same_way_fails(self):
        # The probe of review r1: every C++ error exits 1 with empty stdout, and so does a Rust usage error. Both
        # stand-ins here do exactly that, with different stderr text.
        for binary, who in ((self.rust, 'rust'), (self.cxx, 'cxx')):
            binary.write_text(f"import sys\nsys.stderr.write('{who}: bad command line\\n')\nsys.exit(1)\n",
                              encoding='utf-8')
        self.write('d.txt', '''\
            diff --nmaes local/not-there.upk
            diff --nmaes local/not-there.upk stderr
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.verdicts(out), {'d.txt:1': 'FAIL', 'd.txt:2': 'FAIL'}, out)
        self.assertIn('exited non-zero (C++ 1, Rust 1)', out)
        self.assertIn('stderr differs', out)

    def test_empty_cxx_stdout_needs_the_stderr_marker(self):
        # Both exit 0 and print nothing.
        for binary in (self.rust, self.cxx):
            binary.write_text('import sys\nsys.exit(0)\n', encoding='utf-8')
        self.write('d.txt', '''\
            diff --quiet x
            diff --quiet x stderr
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.verdicts(out), {'d.txt:1': 'FAIL', 'd.txt:2': 'PASS'}, out)
        self.assertIn('the C++ stdout is empty', out)

    def test_misplaced_markers_comments_and_backslashes_are_bad_lines(self):
        self.write('a.txt', f'''\
            diff mode=reference --names x.upk
            diff stderr --names x.upk
            diff --names x.upk  # inline comment
            diff --names local\\\\x.upk
            {self.check_line('<unset>')} # a comment
            {self.check_line('<unset>').replace(' {bin}', ' mode=reference {bin}')}
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.summary(out), (0, 6, 6, 1))
        self.assertEqual(out.count('must come at the end of the line'), 3, out)
        self.assertEqual(out.count("'#' inside a line"), 2, out)
        self.assertEqual(out.count('a backslash in the line'), 1, out)

    def test_a_test_line_without_bin_is_a_bad_line(self):
        self.write('a.txt', f'python {self.checker.as_posix()} somebinary <unset> 0\n')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertIn('a test command must name {bin}', out)

    def test_skipped_unittest_cases_fail_the_line(self):
        # A unittest file whose cases skip exits 0; the line still fails. One with no skip passes.
        for name, body in (('skips', 'raise unittest.SkipTest("no data")'), ('runs', 'pass')):
            (self.tmp / f'{name}_test.py').write_text(textwrap.dedent(f'''\
                import sys, unittest
                class T(unittest.TestCase):
                    def test_a(self):
                        {body}
                    def test_b(self):
                        pass
                if __name__ == '__main__':
                    sys.argv[1:] = []
                    unittest.main()
                '''), encoding='utf-8')
        self.write('a.txt', f'''\
            python {(self.tmp / 'skips_test.py').as_posix()} {{bin}}
            python {(self.tmp / 'runs_test.py').as_posix()} {{bin}}
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.verdicts(out), {'a.txt:1': 'FAIL', 'a.txt:2': 'PASS'}, out)
        self.assertIn('the output reports skipped cases (skipped=1)', out)

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

    def test_a_timeout_stops_the_whole_process_tree(self):
        # The Rust stand-in starts a grandchild that holds the inherited pipes open for 60 s, then sleeps itself.
        # With a 2 s timeout the line fails quickly: the runner stops the tree of the PID it started.
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
        self.assertLess(elapsed, 45, out)

    def test_diff_with_different_exit_codes_fails(self):
        # Same stdout, but the C++ stand-in exits one higher than asked.
        self.cxx.write_text(STAND_IN.format(who='cxx').replace('sys.exit(code)', 'sys.exit(code + 1)'),
                            encoding='utf-8')
        self.write('d.txt', 'diff --exit 3 x\n')
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

    def test_bad_lines_fail(self):
        self.write('a.txt', '''\
            diff
            diff mode=reference
            stderr
            python x.py {bin} stderr
            python "unclosed
            ''')
        code, out, _ = self.runner()
        self.assertEqual(code, 1, out)
        self.assertEqual(self.summary(out), (0, 5, 5, 1))
        self.assertEqual(out.count('bad line:'), 5, out)

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

    def test_no_cxx_needed_without_diff_lines(self):
        self.write('a.txt', self.check_line('<unset>') + '\n')
        code, out, _ = self.runner('--cxx', str(self.tmp / 'missing.exe'))
        self.assertEqual(code, 0, out)

    def test_the_real_manifest_parses(self):
        # The tracked manifest folder: every *.txt line parses (running it needs the binaries; ctest's rust-parity).
        sys.path.insert(0, str(ROOT / 'tools'))
        try:
            import rust_parity
        finally:
            sys.path.pop(0)
        folder = ROOT / 'tests' / 'rust_parity.d'
        self.assertTrue((folder / 'README.md').is_file())
        _, _, problems = rust_parity.read_manifest(folder, [])
        self.assertEqual(problems, [])


if __name__ == '__main__':
    unittest.main()
