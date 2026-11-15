AI-assisted (Claude Code).

# The Rust parity manifest

Each `*.txt` file in this folder is one lane's list of checks the Rust `ow-package` binary must pass
(`docs/rust-port/ARCHITECTURE.md` section 6; one file per lane, so lanes do not collide). `tools/rust_parity.py` runs
every line and prints `PASS` or `FAIL` per line and a summary; ctest runs it as `rust-parity` when
`target/release/ow-package.exe` existed at configure time (build it with `cargo build --release -p ow-cli --locked`,
then configure again). The entry judges whatever release binary was built last, so rebuild it
(`cargo build --release -p ow-cli --locked`) before every ctest run. This README is not a manifest: only `*.txt`
files are read, and any other entry in this folder (a file with another extension, a subfolder) fails the run.

    python tools/rust_parity.py --bin target/release/ow-package.exe --cxx build/Release/ow-package.exe [--file pkg.txt]

## Line format

Files are read in name order. Blank lines and lines starting with `#` are skipped. A line is split like a POSIX shell
line (so quote an argument with spaces, and write paths with forward slashes) and runs from the repository root.

| Line | What runs | Passes when |
|---|---|---|
| `python tests/package_test.py {bin}` | the test script, with `{bin}` (required) replaced by the Rust binary's path and `python` by the runner's Python | it exits 0 and reports no skipped unittest case (`skipped=N`, `OK (skipped`) |
| `diff --names <package>` | the arguments on the C++ binary (`--cxx`) and on the Rust binary (`--bin`) | stdout (bytes) and the exit code are equal, and the rules below hold |

A test-command line starts with `python`, and the next word is a tracked `tests/*.py` file (`git ls-files`). No
interpreter option goes before it: `-O` would strip the test's asserts, and the runner removes `PYTHONOPTIMIZE` from
every command's environment for the same reason.

A diff line must not pass just because both binaries did nothing, or failed the same way:

- if the C++ binary wrote nothing (empty stdout and stderr), the line fails whatever its markers. Commands that print
  nothing belong in a test-command line;
- if the C++ stdout is empty, the line needs `stderr`;
- if the C++ binary exits non-zero, the line needs `exit=N` with N its exit code, and the C++ stderr must not be
  empty.

Trailing markers, in any order:

- `exit=N` (diff lines only): the C++ binary is expected to exit with N; required for every non-zero exit, and a
  different C++ exit code fails the line. It says the error is expected. It compares nothing by itself.
- `stderr` (diff lines only): stderr must be equal too. Add it where a C++ test asserts stderr text, and on expected
  errors whose message matters (`diff --nmaes x exit=1` passes on any shared usage text; `stderr` pins the text).
- `mode=reference`: the line runs with `OW_PARITY=reference`, which turns on the reference stand-ins
  (`docs/rust-port/ARCHITECTURE.md` section 6.1). Every other line runs with `OW_PARITY` removed, whatever the
  caller's environment holds. Mark only the lines that pin a stand-in, and list each in your handoff with the
  Rust-native test of its replacement.

A marker word anywhere but at the end of a line, a `#` inside a line (no inline comments) and a backslash (write paths
with forward slashes) make the line a bad line, which fails. There is no expected-failure marker: a line goes into the
manifest when it passes.

## What a line may hold

- Synthetic inputs only in lines that run on CI (AGENTS.md rule 9). CI has no game install: a line that needs the
  installed packages cannot be in a manifest file CI runs. Real-package comparisons are run by hand with
  `--manifest-dir` pointing at a manifest under ignored `local/`, and their outputs stay there.
- No game-derived text in a line: names of game objects are allowed, dumps, listings and values from a save are not
  (AGENTS.md rule 1). A failing diff prints the first differing line; do not paste a real-package failure into a PR.
- The C++ binary in CI is built twice, with LZO on and off; a diff line whose input is compressed fails against the
  LZO-off build. Prefer a test-command line (it runs the Rust binary only) for compressed inputs.
