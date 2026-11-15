## 2026-11-15: The Rust parity runner reads CRLF as LF

AI-assisted (Claude Code). The maintainer approved this on 2026-11-15, after the hub asked. The change affects
`tools/rust_parity.py` (lane W1-HARNESS's runner) and its self-test `tests/rust_parity_test.py`.

- **What changed.** A `diff` manifest line compares stdout, and stderr when it has the `stderr` marker, after every
  CRLF in both outputs is read as LF. Nothing else is normalised: a lone CR, or CR CR LF against LF, still differs.
  Exit codes and the rules against vacuous passes are unchanged.
- **Why.** The C++ `ow-package` writes through MSVC text-mode streams, so on Windows each `\n` it prints reaches
  stdout and stderr as CRLF. The Rust binary writes LF on every platform (the hub's decision for the Rust binary,
  recorded since W1-PKG). Every wave-2 and wave-3 lane matched the C++ output once line endings were normalised, and
  the only remaining differences were those endings, so without this change no `diff` line could pass on Windows CI.
- **What it does not cover.** Files a command writes (PNG, OBJ and the like) are compared by the lanes as bytes, and
  this runner does not compare them. Test-command lines are unchanged.
- **Verified.** The self-test gains `test_crlf_reads_as_lf_and_nothing_else_is_normalised`, which checks six cases:
  - CRLF against LF passes, with and without a final line end;
  - a lone CR fails;
  - CRLF against nothing fails;
  - CRLF in stderr passes;
  - CR CR LF against LF fails.

  `python tests/rust_parity_test.py` passes, 27 tests.
- **Follow-up.** The lanes kept their CRLF-only `diff` lines out of the tracked manifests and recorded them in their
  handoffs. Those lines can now go into `tests/rust_parity.d/*.txt`. Each one still has to pass on CI before it is kept.
  `cmd/tables.rs` (W1-PKG's command family) still prints CRLF on Windows. Switching it to LF is now safe.
