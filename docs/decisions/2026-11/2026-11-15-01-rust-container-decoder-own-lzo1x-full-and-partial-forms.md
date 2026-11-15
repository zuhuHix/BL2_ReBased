## 2026-11-15: Rust container decoder: own LZO1X, full and partial forms

AI-assisted (Claude Code). Lane W1-LZO of the Rust port (`docs/rust-port/ARCHITECTURE.md`, wave 1), branch
`feature/rust-w1-lzo`.

- **What changed.** `crates/ow-container` now decodes BL2 package files: `decode_package` returns a fully compressed
  container's decoded bytes, a partially compressed package's bytes (the summary up to the first chunk, then the decoded
  chunks), and an uncompressed 832/46 package unchanged, as `src/container.cpp` does. The C++ reader is unchanged and
  stays the reference.
- **The LZO1X decoder is the project's own** (`crates/ow-container/src/lzo.rs`), written from the LZO1X instruction format
  and the pure-Python decoder in `research/native_count.py`, which it follows instruction by instruction. No code of
  lzokay (`third_party/lzokay`) or any other LZO implementation was copied or translated. The Python decoder's
  instruction semantics were checked against lzokay (MIT) when it was written (its docstring says so); here lzokay runs
  as the C++ side of the comparison below. No third-party crate was added.
- **Checks kept, none loosened (rule 5).** The container checks and limits are the C++ code's, in the same order: block
  size a power of two from 4 KiB to 1 MiB, decoded size 1 byte to 512 MiB, the compressed size equal to the bytes after
  the block table, each block's sizes and their totals, every block an exact LZO stream (it ends with the end marker as
  its last byte and fills exactly its decoded size). Only the 3-byte form `0x11 0x00 0x00` is an end marker: a zero
  distance with any other length is refused, as lzokay refuses it (review r1 of PR #283 found the first version
  accepting it; `research/native_count.py`'s decoder still accepts it, and is not changed by this lane). The partial table is read at the C++ offsets
  (`container.cpp:28-34`, not re-derived): generation count bounded by the file, codec 2, a non-zero chunk count that
  fits the file, each chunk's bounds against the file, the 512 MiB limit and the previous chunk (first destination at
  least 48 and not past its source), each chunk a container (not a package) whose decoded size matches the table, and
  no bytes after the last chunk. A failed check is an error; nothing is clamped.
- **Errors.** The C++ code throws messages only; the Rust errors are `ContainerError` kinds. Two variants were added
  (additive, on the hub's decision in review r1; `docs/rust-port/ARCHITECTURE.md` section 3.2 updated):
  `BadHeader { reason }` for the header of a fully compressed container, and `InPartialChunk { index, at, error }`,
  which wraps an error inside a partial chunk with its table entry and its file offset (the inner error stays in the
  chunk's own coordinates).
- **Verified, and how.** Unit tests re-express `tests/package_test.py`'s container cases (valid, every truncation, the
  corrupt header and table fields, trailing bytes, a broken end marker, the two bad LZO streams, padded multi-block
  containers) and its partial case (valid; the six bad table fields; trailing bytes), and they give the accept/refuse
  results the C++ binary gives in that test; more unit tests cover each LZO instruction form on invented streams. On
  pc-hummer's install (2026-11-15), all 2,010 `.upk` packages under `WillowGame/CookedPCConsole` and `DLC/`
  were decoded and compared with the C++ reader's `--verify-decoded`: 2,008 byte-identical (1,973 of them partially
  compressed, 11 fully compressed, 24 uncompressed), and the 2 Mancana remaster texture packages refused by both
  with the same error kind (bad tag: `BadMagic` here, "invalid package magic" in C++; the gate test asserts the kind). The per-package hash list is under ignored `local/w1-lzo/`. This shows the Rust decoder reproduces the
  C++ reader's bytes; it says nothing new about the game beyond what the C++ reader already showed.
- **Not covered.** `tests/package_test.py {bin} --compressed` on the Rust binary needs the package reader and CLI
  (lane W1-PKG); it is not run here.
