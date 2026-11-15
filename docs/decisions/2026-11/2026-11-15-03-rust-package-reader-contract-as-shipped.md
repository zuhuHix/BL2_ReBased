## 2026-11-15: Rust package reader contract as shipped

AI-assisted (Claude Code). Lane W1-PKG of the Rust port fills `crates/ow-package/src/{reader,header,tables,package}.rs`
and the table commands of `crates/ow-cli/src/cmd/tables.rs`, a port of the C++ reader (`src/package.cpp`) and of
those branches of `src/cli.cpp`. No parsing behaviour changes: every offset, entry size and check is the C++ one. This
file records where the shipped interface differs from revision 3 of `docs/rust-port/ARCHITECTURE.md` section 3.2
(that section now lists the shipped shapes). The hub accepted all five after review r1 of PR #293, because each
mirrors the C++ reader.

- **`Package.name` is the raw file stem (`String`), not a `PackageName`.** The C++ `packageName` is `path.stem()`
  with no check, and a stem such as `a.b` loads there; `PackageName::parse` refuses a `.`. `Package::package_name()`
  gives the typed name and fails where the stem is not a valid package name. Case-insensitive package identity is
  W2-STORE's, through that method.
- **`find_export` returns `Result<Option<i32>, PackageError>`.** The C++ `findExport` builds each export's path in
  turn and throws when a chain is cyclic or deeper than 256, before any later match; the Rust search keeps that
  error instead of turning it into "not found".
- **`Package::from_decoded` is new.** It reads bytes that are already decoded (synthetic fixtures, tools that decode
  themselves). `load` and `from_bytes` always go through `ow_container::decode_package`; nothing bypasses it.
- **`Reader::name(count: i32)`** replaces `name(&mut self, names: usize)`: the C++ `Reader::name(int32_t count)`
  compares the index with a signed count, and the table code passes the name count it already holds.
- **`PackageError` stays `Clone + PartialEq + Eq`.** A future cause that is neither (an `io::Error`, a boxed error)
  is carried as text, never as the value. The container error is wrapped as `source()` and not re-printed.

- **Verified (automated).** `tools/verify_packages.py`'s decoded-bytes, counts and export-field checks pass on the
  Rust binary for the 9 code packages (its class-body step waits for W2-REFLECT, run with that step removed in a
  local copy); the summary, `--names`, `--imports`, `--exports` and `--census` output is byte-identical to the C++
  binary on those packages; `tests/package_test.py` passes with and without `--compressed`.
- **Not verified.** Nothing here is checked against the running game: parity is with the C++ reader, which
  `verify_packages.py` checks against the independent Python decoder.
