//! The UnrealScript interpreter: frames, states, latent code, delegates, iterators, the native registry, the stub
//! policy, Core natives, the step limit, call census and trace, `override_script`, and the VM sweep and native census
//! (port of the run half of `src/vm.cpp`, `src/interp.*`, `src/natives*.cpp`, `src/vm_sweep.cpp`, `src/census.*`).
//!
//! Done when the VM's synthetic tests and the Fire event-order check (`tools/verify_fire_event_order.py`) give the
//! same results with the Rust binary as with the C++ build.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod api;
pub mod census;
pub mod error;
pub mod interp;
pub mod latent;
pub mod natives_core;
pub mod path_name;
pub mod services;
pub mod states;
pub mod sweep;
