//! UnrealScript bytecode: decoder, opcode table, disassembler text and the package check (port of `src/script.*`).
//! Execution is `ow-vm`'s.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod check;
pub mod decode;
pub mod disasm;
pub mod function;
pub mod ops;
