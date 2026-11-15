"""AI-assisted (Claude Code). The Rust parity runner (docs/rust-port/ARCHITECTURE.md section 6; lane W1-HARNESS).

Runs every line of the parity manifest, tests/rust_parity.d/*.txt (one file per lane), against the Rust ow-package
binary and prints PASS or FAIL per line, then a summary. Stdlib only.

    python tools/rust_parity.py [--bin target/release/ow-package.exe] [--cxx build/Release/ow-package.exe]
                                [--manifest-dir tests/rust_parity.d] [--file NAME.txt ...] [--timeout SECONDS]

Manifest lines (files are read in name order; blank lines and lines starting with '#' are skipped):

  <command>          A test command, split like a POSIX shell line (shlex). `{bin}` is replaced by the Rust binary's
                     path, and a first word `python` by the Python running this script. It runs from the repository
                     root and passes when it exits 0.
  diff <args>        `ow-package` arguments. They run on the C++ binary (--cxx) and on the Rust binary (--bin); the
                     line passes when stdout (bytes) and the exit code are equal.

Trailing markers, in any order, after the command or the arguments:

  stderr             (diff lines only) stderr must be equal too.
  mode=reference     the line runs with OW_PARITY=reference (both binaries, for a diff line). Every other line runs
                     with OW_PARITY removed from the environment, whatever the caller's environment holds. This
                     script is the only place that sets the variable (docs/rust-port/ARCHITECTURE.md rule R3).

There is no expected-failure marker: a line is in the manifest when it passes. Exit code 0 when every line passes
(also when the manifest holds no line, which the summary says), 1 when any line fails, 2 for a usage error (a
binary that is not there, or diff lines without a C++ binary).

A binary path ending in `.py` is run with this Python. Only the self-test (tests/rust_parity_test.py) uses that, for
its stand-in binaries.
"""

import argparse
import os
import pathlib
import shlex
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EXE = '.exe' if os.name == 'nt' else ''
DEFAULT_BIN = ROOT / 'target' / 'release' / f'ow-package{EXE}'
DEFAULT_CXX = ROOT / 'build' / 'Release' / f'ow-package{EXE}'
DEFAULT_MANIFEST_DIR = ROOT / 'tests' / 'rust_parity.d'
PARITY_VAR = 'OW_PARITY'
MARKERS = ('stderr', 'mode=reference')
SHOWN_CHARS = 160   # longest stretch of one output line a failure message prints
SHOWN_TAIL = 20     # lines of a failing test command's output printed


class BadLine(Exception):
    pass


class Line:
    """One manifest line: where it is, its kind, its words and its markers."""

    def __init__(self, path, number, text):
        self.where = f'{path.name}:{number}'
        self.text = text
        try:
            words = shlex.split(text, posix=True)
        except ValueError as e:
            raise BadLine(f'cannot split the line: {e}') from None
        self.reference = False
        self.stderr = False
        while words and words[-1] in MARKERS:
            marker = words.pop()
            if marker == 'mode=reference':
                self.reference = True
            else:
                self.stderr = True
        self.diff = bool(words) and words[0] == 'diff'
        if self.diff:
            words = words[1:]
            if not words:
                raise BadLine('a diff line needs ow-package arguments')
        elif not words:
            raise BadLine('the line holds only markers')
        elif self.stderr:
            raise BadLine('the stderr marker is for diff lines only')
        self.words = words


def read_manifest(manifest_dir, only):
    """Returns (files, lines, problems): the manifest files read, their parsed lines in file and line order, and
    (where, text, message) for each line that failed to parse. Only *.txt files are manifests (the folder's
    README.md is not). A --file name that is not there raises BadLine."""
    files = sorted(p for p in manifest_dir.glob('*.txt') if p.is_file())
    if only:
        wanted = set(only)
        missing = wanted - {p.name for p in files}
        if missing:
            raise BadLine(f'no manifest file {", ".join(sorted(missing))} in {manifest_dir}')
        files = [p for p in files if p.name in wanted]
    lines, problems = [], []
    for path in files:
        for number, raw in enumerate(path.read_text(encoding='utf-8').splitlines(), 1):
            text = raw.strip()
            if not text or text.startswith('#'):
                continue
            try:
                lines.append(Line(path, number, text))
            except BadLine as e:
                problems.append((f'{path.name}:{number}', text, str(e)))
    return files, lines, problems


def command_for(binary):
    """The argv prefix that runs a binary (a .py stand-in runs with this Python)."""
    binary = str(binary)
    return [sys.executable, binary] if binary.endswith('.py') else [binary]


def environment(reference):
    env = dict(os.environ)
    env.pop(PARITY_VAR, None)
    if reference:
        env[PARITY_VAR] = 'reference'
    return env


def run(argv, env, timeout):
    """(exit code, stdout bytes, stderr bytes); a timeout or a program that cannot start gives exit code None."""
    try:
        done = subprocess.run(argv, cwd=ROOT, env=env, stdin=subprocess.DEVNULL, capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return None, b'', f'timed out after {timeout} s'.encode()
    except OSError as e:
        return None, b'', f'cannot start: {e}'.encode()
    return done.returncode, done.stdout, done.stderr


def shorten(data):
    text = data.decode('utf-8', 'replace')
    return text if len(text) <= SHOWN_CHARS else text[:SHOWN_CHARS] + '...'


def first_difference(cxx, rust):
    """Says where two byte strings first differ, by line (line endings count; they are not shown)."""
    a, b = cxx.splitlines(keepends=True), rust.splitlines(keepends=True)
    for i, (x, y) in enumerate(zip(a, b), 1):
        if x != y:
            x, y = x.rstrip(b'\r\n'), y.rstrip(b'\r\n')
            if x == y:
                return f'line {i}: the line endings differ'
            return f'line {i}: C++ {shorten(x)!r}, Rust {shorten(y)!r}'
    return f'C++ {len(a)} lines, Rust {len(b)} lines'


def run_test_line(line, binary, timeout):
    argv = [w.replace('{bin}', str(binary)) for w in line.words]
    if argv[0] == 'python':
        argv[0] = sys.executable
    code, out, err = run(argv, environment(line.reference), timeout)
    if code == 0:
        return []
    tail = (out + err).decode('utf-8', 'replace').splitlines()[-SHOWN_TAIL:]
    return [f'exit code {code}'] + [f'  | {t}' for t in tail]


def run_diff_line(line, binary, cxx, timeout):
    env = environment(line.reference)
    c_code, c_out, c_err = run(command_for(cxx) + line.words, env, timeout)
    r_code, r_out, r_err = run(command_for(binary) + line.words, env, timeout)
    reasons = []
    if c_code is None:
        reasons.append(f'C++ binary: {c_err.decode()}')
    if r_code is None:
        reasons.append(f'Rust binary: {r_err.decode()}')
    if reasons:
        return reasons
    if c_code != r_code:
        reasons.append(f'exit code: C++ {c_code}, Rust {r_code}')
    if c_out != r_out:
        reasons.append(f'stdout differs: {first_difference(c_out, r_out)}')
    if line.stderr and c_err != r_err:
        reasons.append(f'stderr differs: {first_difference(c_err, r_err)}')
    return reasons


def main(argv=None):
    parser = argparse.ArgumentParser(description='Run the Rust parity manifest (tests/rust_parity.d/*.txt).')
    parser.add_argument('--bin', type=pathlib.Path, default=DEFAULT_BIN, help='the Rust ow-package binary')
    parser.add_argument('--cxx', type=pathlib.Path, default=DEFAULT_CXX,
                        help='the C++ ow-package binary (needed only by diff lines)')
    parser.add_argument('--manifest-dir', type=pathlib.Path, default=DEFAULT_MANIFEST_DIR)
    parser.add_argument('--file', action='append', default=[], metavar='NAME',
                        help='run only this manifest file (by name, e.g. pkg.txt); may be repeated')
    parser.add_argument('--timeout', type=float, default=600, help='seconds allowed per command (default 600)')
    args = parser.parse_args(argv)
    # Commands run from the repository root, so paths given relative to the caller's folder are made absolute first.
    args.bin, args.cxx, args.manifest_dir = (p.resolve() for p in (args.bin, args.cxx, args.manifest_dir))

    if not args.manifest_dir.is_dir():
        print(f'rust_parity: no manifest folder {args.manifest_dir}', file=sys.stderr)
        return 2
    try:
        files, lines, problems = read_manifest(args.manifest_dir, args.file)
    except BadLine as e:
        print(f'rust_parity: {e}', file=sys.stderr)
        return 2
    if not args.bin.is_file():
        print(f'rust_parity: no Rust binary at {args.bin} (cargo build --release -p ow-cli --locked)', file=sys.stderr)
        return 2
    if any(line.diff for line in lines) and not args.cxx.is_file():
        print(f'rust_parity: the manifest has diff lines and there is no C++ binary at {args.cxx}', file=sys.stderr)
        return 2

    passed = failed = 0
    for where, text, message in problems:
        failed += 1
        print(f'FAIL {where}  {text}\n  bad line: {message}')
    for line in lines:
        if line.diff:
            reasons = run_diff_line(line, args.bin, args.cxx, args.timeout)
        else:
            reasons = run_test_line(line, args.bin, args.timeout)
        if reasons:
            failed += 1
            print(f'FAIL {line.where}  {line.text}')
            for reason in reasons:
                print(f'  {reason}')
        else:
            passed += 1
            print(f'PASS {line.where}  {line.text}')
        sys.stdout.flush()

    total = passed + failed
    note = ' (the manifest holds no line yet)' if total == 0 else ''
    print(f'rust_parity: {passed} passed, {failed} failed, of {total} lines in {len(files)} files{note}')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
