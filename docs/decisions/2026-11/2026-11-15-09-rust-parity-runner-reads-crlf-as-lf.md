## 2026-11-15: The Rust parity runner reads CRLF as LF

AI-assisted (Claude Code). The maintainer approved this on 2026-11-15, after the hub asked. The change affects
`tools/rust_parity.py` (lane W1-HARNESS's runner) and its self-test `tests/rust_parity_test.py`.

- **What changed.** A `diff` manifest line compares stdout, and stderr when it has the `stderr` marker, after every
  CRLF in both outputs is read as LF. Nothing else is normalised: a lone CR, or CR CR LF against LF, still differs.
  Exit codes and the rules against vacuous passes are unchanged.
- **Why.** The C++ `ow-package` writes through MSVC text-mode streams, so on Windows each `\n` it prints reaches
  stdout and stderr as CRLF. The Rust binary writes LF on every platform (the hub's decision for the Rust binary,
  recorded since W1-PKG). The wave-2 and wave-3 lanes reported in their handoffs that their outputs matched the C++
  once line endings were normalised. Without this change, no `diff` line could pass on Windows CI.
- **What it does not cover.**
  - Files a command writes (PNG, OBJ and the like) are compared by the lanes as bytes; this runner does not compare
    them.
  - Test-command lines are unchanged.
  - The mapping is not a full inverse of MSVC text mode. A literal CR LF printed by the C++ reaches stdout as CR CR LF
    and maps to CR LF, while a Rust binary printing the same CR LF maps to LF, so such a line fails although the two
    agree. That errs only towards failing; do not loosen `lf` to cover it.
  - Rule R7 in `docs/rust-port/ARCHITECTURE.md` now names this mapping.
- **Verified.** The self-test gains `test_crlf_reads_as_lf_and_nothing_else_is_normalised`, which checks seven cases:
  - CRLF against LF passes, with and without a final line end;
  - a lone CR fails;
  - a CRLF line end against no line end fails;
  - a C++ output of only CRLF against an empty Rust output fails;
  - CRLF in stderr passes;
  - CR CR LF against LF fails.

  Each failing case also checks the reason the runner gives. `python tests/rust_parity_test.py` passes, 27 tests.
- **Follow-up.** The lanes kept their CRLF-only `diff` lines out of the tracked manifests and recorded them in their
  handoffs. Those lines can now go into `tests/rust_parity.d/*.txt`. Each one still has to pass on CI before it is kept.
  `cmd/tables.rs` (W1-PKG's command family) still prints CRLF on Windows. Switching it to LF is now safe.
