//! Owner: lane W0-SCAFFOLD (complete). The parity mode (architecture rule R3).
//!
//! A stand-in of the reference implementation (C++ or Python) that a parity test pins, such as the VM's `Rand`/`FRand`
//! LCGs, is reproduced only under [`ParityMode::Reference`]. Exactly one thing selects it: the environment variable
//! `OW_PARITY=reference`, read once, the first time [`parity_mode`] is called. `tools/rust_parity.py` sets it for the
//! manifest lines marked `mode=reference` and for no others. Every stand-in is listed in its lane's handoff and
//! deleted in W7-RETIRE after a Rust-native test of its replacement exists.
//!
//! It lives in `ow-core` because `ow-core` and `ow-vm` need it and may not depend on `ow-cli`.

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

/// The run's parity mode, read from `OW_PARITY` on the first call and fixed for the rest of the process.
///
/// Panics on a value other than `reference`, unset or empty: a run with an unknown mode cannot say which behaviour it
/// reproduced.
pub fn parity_mode() -> ParityMode {
    static MODE: OnceLock<ParityMode> = OnceLock::new();
    *MODE.get_or_init(|| {
        let raw = std::env::var_os(ENV_VAR);
        let text = raw.as_ref().map(|v| v.to_string_lossy());
        mode_from_value(text.as_deref()).unwrap_or_else(|e| panic!("{e}"))
    })
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
    fn read_once() {
        // Whatever the test environment holds, two calls agree.
        if mode_from_value(std::env::var(ENV_VAR).ok().as_deref()).is_ok() {
            assert_eq!(parity_mode(), parity_mode());
        }
    }
}
