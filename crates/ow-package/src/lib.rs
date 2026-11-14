//! The UE3 version 832 / licensee 46 package reader: the port of `src/package.cpp` / `src/package.hpp` (header,
//! names, imports, exports, the tagged-property tag walker) and the package store.
//!
//! The C++ `Reader` is the reference: its `require()` and limit checks become `Result` errors here, never panics and
//! never wider bounds (AGENTS.md rule 5). Offsets come from the C++ code, which recovered and checked them; nothing
//! about the format is re-derived or invented (rule 4). This crate knows no reflection: it never depends on
//! `ow-object`.
//!
//! Done when: `python tools/verify_packages.py --reader <the Rust ow-package>` reports the same packages, counts
//! and exact class bodies as the C++ reader, and the synthetic fixtures pass in both.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod header;
pub mod package;
pub mod props;
pub mod reader;
pub mod schema;
pub mod store;
pub mod tables;

#[cfg(any(test, feature = "test-support"))]
pub mod testkit;

pub use store::PackageStore;
