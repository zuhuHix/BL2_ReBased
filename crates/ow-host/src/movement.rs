//! Player movement and collision (lane H4-MOVEMENT, decision 2026-11-16-52). The renderer-free code moved to
//! `ow_assets::movement` (lane LINK-2-HEADLESS-MOVER, decision 2026-11-19-66), so that `ow-package --slice-gate`
//! walks Sanctuary headless through the same mover the host uses. It is re-exported whole here, so every
//! `crate::movement::...` path in the host is unchanged.

pub use ow_assets::movement::*;
