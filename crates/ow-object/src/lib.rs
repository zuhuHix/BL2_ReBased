//! The object model: ids, `Value`, the heap, class layouts, reflection loading from the code packages, class-body
//! decoding and typed values (the loading half of `src/vm.cpp`, `decodeClassBody`, `applyTaggedDefaults`).
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod cdo;
pub mod class_body;
pub mod decode;
pub mod heap;
pub mod ids;
pub mod layout;
pub mod names;
pub mod prefix;
pub mod reflect;
pub mod value;
