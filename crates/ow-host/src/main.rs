//! `ow-host --game <install>`: checks the install and stops, because no engine route is chosen yet.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let game = match (args.next(), args.next()) {
        (Some(flag), Some(path)) if flag == "--game" => PathBuf::from(path),
        _ => {
            eprintln!("usage: ow-host --game <Borderlands 2 install folder>");
            return ExitCode::from(2);
        }
    };
    match ow_host::find_install(&game) {
        Ok(install) => {
            println!("Borderlands 2 install found: {}", install.cooked.display());
            println!(
                "No engine route is chosen yet (Phase 0R benchmark, docs/verification/ENGINE_BENCHMARK.md); \
                 nothing to run."
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ow-host refuses to start: {e}. The original game must be installed (AGENTS.md rule 11).");
            ExitCode::from(1)
        }
    }
}
