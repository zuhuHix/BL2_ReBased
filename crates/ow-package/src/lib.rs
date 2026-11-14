//! The UE3 version 832 / licensee 46 package reader: the port of `src/package.cpp` / `src/package.hpp` (header,
//! names, imports, exports, tagged properties) and `src/assets.*` / `src/census.*`.
//!
//! Not started. The C++ `Reader` is the reference: its `require()` and limit checks become `Result` errors here, never
//! panics and never wider bounds (AGENTS.md rule 5). Offsets come from the C++ code, which recovered and checked them;
//! nothing about the format is re-derived or invented (rule 4).
//!
//! Done when: `python tools/verify_packages.py --reader <this crate's ow-package>` reports the same packages, counts
//! and exact class bodies as the C++ reader, and the synthetic fixtures pass in both.
