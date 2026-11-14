//! The shared foundation of every crate: the typed "not yet" ([`NotYet`]), object paths and package names, the
//! key-ordered and insertion-ordered maps, the parity mode (`OW_PARITY`), and (from the wave 1 lanes) numerics,
//! units, printf-style formatting, game randomness and the simulation clock. Depends on nothing.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their own
//! files and never edit this `lib.rs` (architecture section 2.3). `notyet`, `ident`, `detmap`, `ordmap` and
//! `parity` are complete.

pub mod detmap;
pub mod fmt;
pub mod ident;
pub mod notyet;
pub mod num;
pub mod ordmap;
pub mod parity;
pub mod rand;
pub mod sim;
pub mod units;

pub use detmap::{DetMap, DetSet, FixedHashMap, FixedHashSet};
pub use ident::{IdentError, ObjectPath, PackageName};
pub use notyet::{Missing, NotYet};
pub use ordmap::OrderedMap;
// `parity::init_parity_mode` and `parity::parity_mode` are for binaries only and are not re-exported here.
pub use parity::{ParityMode, mode_from_value};
