//! Command-line front end of the Rust core: the port of `src/cli.cpp`. Until it is ported, the C++ `ow-package` in
//! `build/Release` is the one to use.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!(
        "ow-package (Rust) is not ported yet; use build/Release/ow-package.exe. \
         See docs/decisions/2026-11/2026-11-14-01-rust-replaces-the-ue5-host-engine-route-by-benchmark.md"
    );
    ExitCode::from(2)
}
