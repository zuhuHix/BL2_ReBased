## 2026-11-15: Rust container decoder: own LZO1X, full and partial forms

AI-assisted (Claude Code). Lane W1-LZO of the Rust port (`docs/rust-port/ARCHITECTURE.md`, wave 1), branch
`feature/rust-w1-lzo`.

- **What changed.** `crates/ow-container` now decodes BL2 package files: `decode_package` returns a fully compressed
  container's decoded bytes, a partially compressed package's bytes (the summary up to the first chunk, then the decoded
  chunks), and an uncompressed 832/46 package unchanged, as `src/container.cpp` does. The C++ reader is unchanged and
  stays the reference.
- **The LZO1X decoder is the project's own** (`crates/ow-container/src/lzo.rs`), written from the LZO1X instruction format
  and the pure-Python decoder in `research/native_count.py`, which it follows instruction by instruction. No code of
  lzokay (`third_party/lzokay`) or any other LZO implementation was copied or translated; lzokay runs only as the C++
  side of the comparison below. No third-party crate was added.
- **Checks kept, none loosened (rule 5).** The container checks and limits are the C++ code's, in the same order: block
  size a power of two from 4 KiB to 1 MiB, decoded size 1 byte to 512 MiB, the compressed size equal to the bytes after
  the block table, each block's sizes and their totals, every block an exact LZO stream (it ends with the end marker as
  its last byte and fills exactly its decoded size). The partial table is read at the C++ offsets
  (`container.cpp:28-34`, not re-derived): generation count bounded by the file, codec 2, a non-zero chunk count that
  fits the file, each chunk's bounds against the file, the 512 MiB limit and the previous chunk (first destination at
  least 48 and not past its source), each chunk a container (not a package) whose decoded size matches the table, and
  no bytes after the last chunk. A failed check is an error; nothing is clamped.
- **Errors.** The C++ code throws messages only; the Rust errors are the frozen `ContainerError` kinds. Header-level
  errors of a fully compressed container report `BadChunk { index: 0 }`, because the frozen enum has no header variant
  (listed for the hub as a possible additive contract change; not changed here).
- **Verified, and how.** Unit tests re-express `tests/package_test.py`'s container cases (valid, every truncation, the
  corrupt header and table fields, trailing bytes, a broken end marker, the two bad LZO streams, padded multi-block
  containers) and its partial case (valid; the six bad table fields; trailing bytes), and they give the accept/refuse
  results the C++ binary gives in that test; more unit tests cover each LZO instruction form on invented streams. On
  the zuhu desktop's install (2026-11-15), all 2,010 `.upk` packages under `WillowGame/CookedPCConsole` and `DLC/`
  were decoded and compared with the C++ reader's `--verify-decoded`: 2,008 byte-identical (1,973 of them partially
  compressed, 11 fully compressed, 24 uncompressed), and the 2 Mancana remaster texture packages refused by both
  (bad tag). The per-package hash list is under ignored `local/w1-lzo/`. This shows the Rust decoder reproduces the
  C++ reader's bytes; it says nothing new about the game beyond what the C++ reader already showed.
- **Not covered.** `tests/package_test.py {bin} --compressed` on the Rust binary needs the package reader and CLI
  (lane W1-PKG); it is not run here.
