//! LZO1X container of BL2's packages: the port of `src/container.cpp` / `src/container.hpp`.
//!
//! Not started. The C++ code is the reference and uses the vendored lzokay decoder (`third_party/lzokay`, MIT). This
//! crate gets its own decoder, written from the LZO1X format and from the project's own pure-Python decoder in
//! `research/native_count.py`, so no C code is translated. The container's bounds checks move over unchanged in
//! meaning (AGENTS.md rule 5): every size and offset is checked before it is used, and a failed check is an error,
//! never a clamp.
//!
//! Done when: the synthetic container fixtures that `tests/` builds give the same results here as in the C++ build,
//! and the real packages decompress to the same bytes as `ow-package` from `build/Release`.
