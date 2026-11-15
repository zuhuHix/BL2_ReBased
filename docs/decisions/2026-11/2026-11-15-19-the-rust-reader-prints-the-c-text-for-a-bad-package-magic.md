## 2026-11-15: The Rust reader prints the C++ text for a bad package magic

AI-assisted (Claude Code). Lane FOLLOWUPS-2 of the Rust port, rule R7 (`docs/rust-port/ARCHITECTURE.md`: the C++ CLI
is the contract, including stderr where a test asserts it). No parsing behaviour changes: the same files are refused,
with the same error kind and exit code 1. Only the message text changes.

- **What changed.** For a file whose first word is not the UE3 package tag, the C++ reader throws
  `invalid package magic` from `decode_package` in `src/container.cpp` (and again from `src/package.cpp` for a
  decoded package without the tag). The Rust reader printed `cannot decode the package container: not a UE3 package
  (bad tag)`. Now `ow_container::ContainerError::BadMagic` displays as `invalid package magic`, and
  `ow_package::package::PackageError::Container(BadMagic)` displays that text alone, with no `source()`, so every CLI
  command that joins an error with its causes prints `ow-package: invalid package magic`, as the C++ binary does.
  The error stays typed; the other container errors keep the "cannot decode the package container: ..." prefix and
  their own texts.
- **Why it matters.** `tools/item_registry.py` keeps the reader's stderr line as a file's scan error in
  `census.json`, so the two readers wrote different census files for the same install (two such files in the installed
  game). `tests/item_registry_rust_test.py` found the gap; its two expected-failure markers for the text are removed.
  The registry comparison stays skipped while `DefStore::census()` has no body (W3-DEFS).
- **Verified.** Rust unit tests in `ow-container` and `ow-package` assert the text and the empty `source()`;
  `tests/package_test.py` passes on both binaries (its corrupt-magic case included); `tests/item_registry_rust_test.py`
  compares the two binaries' stderr on an invented non-package file and passes. Only the synthetic files were run;
  the two installed files were not re-run in this lane.
