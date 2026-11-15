"""AI-assisted (Claude Code). The Rust parity runner (docs/rust-port/ARCHITECTURE.md section 6; lane W1-HARNESS).

Runs every line of the parity manifest, tests/rust_parity.d/*.txt (one file per lane), against the Rust ow-package
binary and prints PASS or FAIL per line, then a summary. Stdlib only.

    python tools/rust_parity.py [--bin target/release/ow-package.exe] [--cxx build/Release/ow-package.exe]
                                [--manifest-dir tests/rust_parity.d] [--file NAME.txt ...] [--timeout SECONDS]

Manifest lines (files are read in name order; blank lines and lines starting with '#' are skipped):

  <command>          A test command, split like a POSIX shell line (shlex). It must name `{bin}`, which is replaced by
                     the Rust binary's path; a first word `python` is replaced by the Python running this script. It
                     runs from the repository root and passes when it exits 0 and its output reports no skipped
                     unittest case (`skipped=N`, `OK (skipped`).
  diff <args>        `ow-package` arguments. They run on the C++ binary (--cxx) and on the Rust binary (--bin); the
                     line passes when stdout (bytes) and the exit code are equal. So that two binaries failing the
                     same way cannot pass, a line where either binary exits non-zero or the C++ stdout is empty
                     passes only with the `stderr` marker, which compares stderr too.

Trailing markers, in any order, after the command or the arguments:

  stderr             (diff lines only) stderr must be equal too; needed for an expected error.
  mode=reference     the line runs with OW_PARITY=reference (both binaries, for a diff line). Every other line runs
                     with OW_PARITY removed from the environment, whatever the caller's environment holds. This
                     script is the only place that sets the variable (docs/rust-port/ARCHITECTURE.md rule R3).

A line holding a marker word anywhere but at its end, a `#` (no inline comments) or a backslash (write paths with
forward slashes) is a bad line and fails, as does any entry of the manifest folder other than README.md and *.txt
files (a lane's file with a wrong name would otherwise never run).

There is no expected-failure marker: a line is in the manifest when it passes. Exit code 0 when every line passes
(also when the manifest holds no line, which the summary says), 1 when any line fails, 2 for a usage error (a
binary that is not there, or diff lines without a C++ binary). A command that outruns --timeout is stopped with its
child processes (the process tree of the PID this script started) and its line fails.

A binary path ending in `.py` is run with this Python. Only the self-test (tests/rust_parity_test.py) uses that, for
its stand-in binaries.
"""

import argparse
import os
import pathlib
import re
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
        if '#' in text:
            raise BadLine("'#' inside a line (only whole-line comments are allowed)")
        if '\\' in text:
            raise BadLine('a backslash in the line (write paths with forward slashes)')
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
        misplaced = [w for w in words if w in MARKERS]
        if misplaced:
            raise BadLine(f'the marker {misplaced[0]} must come at the end of the line')
        self.diff = bool(words) and words[0] == 'diff'
        if self.diff:
            words = words[1:]
            if not words:
                raise BadLine('a diff line needs ow-package arguments')
        elif not words:
            raise BadLine('the line holds only markers')
        elif self.stderr:
            raise BadLine('the stderr marker is for diff lines only')
        elif not any('{bin}' in w for w in words):
            raise BadLine('a test command must name {bin} (else it tests some other binary)')
        self.words = words


def read_manifest(manifest_dir, only):
    """Returns (files, lines, problems): the manifest files read, their parsed lines in file and line order, and
    (where, text, message) for each line that failed to parse. Only *.txt files are manifests (the folder's
    README.md is not); any other entry of the folder is a problem. A --file name that is not there raises BadLine."""
    files, problems = [], []
    for p in sorted(manifest_dir.iterdir()):
        if p.is_file() and p.suffix == '.txt':
            files.append(p)
        elif not (p.is_file() and p.name == 'README.md'):
            problems.append((p.name, '', 'not a manifest file (only README.md and *.txt files belong here)'))
    if only:
        wanted = set(only)
        missing = wanted - {p.name for p in files}
        if missing:
            raise BadLine(f'no manifest file {", ".join(sorted(missing))} in {manifest_dir}')
        files = [p for p in files if p.name in wanted]
    lines = []
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
        proc = subprocess.Popen(argv, cwd=ROOT, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, start_new_session=(os.name != 'nt'))
    except OSError as e:
        return None, b'', f'cannot start: {e}'.encode()
    try:
        out, err = proc.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        stop_tree(proc)
        return None, b'', f'timed out after {timeout} s; stopped it and its child processes'.encode()
    return proc.returncode, out, err


def stop_tree(proc):
    """Stops the process this script started and its children (a child ow-package can hold the pipes open). Only
    that PID's tree, never a process by image name (AGENTS.md rule 10)."""
    if os.name == 'nt':
        subprocess.run(['taskkill', '/PID', str(proc.pid), '/T', '/F'], stdin=subprocess.DEVNULL,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        import signal
        try:
            os.killpg(proc.pid, signal.SIGKILL)   # its own session, started above
        except ProcessLookupError:
            pass
    try:
        proc.communicate(timeout=30)
    except subprocess.TimeoutExpired:
        proc.kill()


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
    text = (out + err).decode('utf-8', 'replace')
    if code == 0:
        skipped = re.search(r'skipped=\d+', text) or re.search(r'OK \(skipped', text)
        if not skipped:
            return []
        reason = f'exit code 0, but the output reports skipped cases ({skipped.group(0)})'
    else:
        reason = f'exit code {code}'
    return [reason] + [f'  | {t}' for t in text.splitlines()[-SHOWN_TAIL:]]


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
    # Two binaries that both refuse the arguments agree on exit code and (empty) stdout; without a stderr comparison
    # that would pass whatever the cause. Such a line must say what error it expects.
    if not line.stderr:
        if c_code != 0 or r_code != 0:
            reasons.append(f'a binary exited non-zero (C++ {c_code}, Rust {r_code}): '
                           'an expected error needs the stderr marker')
        elif not c_out:
            reasons.append('the C++ stdout is empty: a line with no output needs the stderr marker')
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
