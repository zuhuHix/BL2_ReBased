//! Owner: lane W0-SCAFFOLD (complete). The parity mode (architecture rule R3).
//!
//! A stand-in of the reference implementation (C++ or Python) that a parity test pins, such as the VM's `Rand`/`FRand`
//! LCGs, is reproduced only under [`ParityMode::Reference`]. Exactly one thing selects it: the environment variable
//! `OW_PARITY=reference`. `tools/rust_parity.py` sets it for the manifest lines marked `mode=reference` and for no
//! others. Every stand-in is listed in its lane's handoff and deleted in W7-RETIRE after a Rust-native test of its
//! replacement exists.
//!
//! **Only binaries read the environment** (`ow-cli/src/main.rs`, later `ow-host/src/main.rs`): they call
//! [`init_parity_mode`] first thing in `main`, report a bad value and exit, and pass the resulting [`ParityMode`] value
//! down. Library crates never read it: they take a `ParityMode` argument (`GameRng::from_world_seed(seed, mode)`,
//! `Vm::new(.., mode)`), so one test process can build both modes with [`mode_from_value`].
//! `tests/crate_edges_test.py` refuses `parity_mode(`, `init_parity_mode`, `mode_from_env`, `ENV_VAR` and the string
//! `"OW_PARITY"` anywhere else.
//!
//! It lives in `ow-core` because `ow-core` and `ow-vm` need the type and may not depend on `ow-cli`.

use std::sync::OnceLock;

/// The environment variable that selects the parity mode.
pub const ENV_VAR: &str = "OW_PARITY";

/// Which behaviour a run reproduces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParityMode {
    /// The Rust rules, with no reference stand-in. The default.
    #[default]
    Off,
    /// Reproduce the reference implementation's stand-ins, for differential runs only.
    Reference,
}

/// Reads a value of `OW_PARITY`: unset or empty is [`ParityMode::Off`], `reference` is [`ParityMode::Reference`],
/// anything else is an error (a misspelt value must not quietly turn the stand-ins off).
pub fn mode_from_value(value: Option<&str>) -> Result<ParityMode, String> {
    match value {
        None | Some("") => Ok(ParityMode::Off),
        Some("reference") => Ok(ParityMode::Reference),
        Some(other) => Err(format!(
            "{ENV_VAR} must be \"reference\" or unset, not {other:?}"
        )),
    }
}

/// Reads `OW_PARITY` from the environment now (not cached).
pub fn mode_from_env() -> Result<ParityMode, String> {
    let raw = std::env::var_os(ENV_VAR);
    let text = raw.as_ref().map(|v| v.to_string_lossy());
    mode_from_value(text.as_deref())
}

static MODE: OnceLock<ParityMode> = OnceLock::new();

/// For a binary's `main`, first thing: reads `OW_PARITY` and fixes the mode for the rest of the process. A bad value
/// is returned as the error and fixes nothing, so `main` can print it and exit with its own code.
pub fn init_parity_mode() -> Result<ParityMode, String> {
    if let Some(mode) = MODE.get() {
        return Ok(*mode);
    }
    let mode = mode_from_env()?;
    Ok(*MODE.get_or_init(|| mode))
}

/// The process's parity mode, for a binary. After [`init_parity_mode`] succeeded this never fails. Called without it,
/// it reads `OW_PARITY` itself and panics on a bad value (a run with an unknown mode cannot say which behaviour it
/// reproduced).
pub fn parity_mode() -> ParityMode {
    init_parity_mode().unwrap_or_else(|e| panic!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values() {
        assert_eq!(mode_from_value(None), Ok(ParityMode::Off));
        assert_eq!(mode_from_value(Some("")), Ok(ParityMode::Off));
        assert_eq!(
            mode_from_value(Some("reference")),
            Ok(ParityMode::Reference)
        );
        assert!(mode_from_value(Some("Reference")).is_err());
        assert!(mode_from_value(Some("off")).is_err());
        assert_eq!(ParityMode::default(), ParityMode::Off);
    }

    #[test]
    fn fixed_once() {
        // Whatever the test environment holds, a good value is fixed and later calls agree.
        if let Ok(mode) = mode_from_env() {
            assert_eq!(init_parity_mode(), Ok(mode));
            assert_eq!(parity_mode(), mode);
            assert_eq!(init_parity_mode(), Ok(mode));
        }
    }
}
