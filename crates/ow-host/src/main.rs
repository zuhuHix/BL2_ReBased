//! `ow-host --game <install>`: checks the install and stops, because the host on the chosen engine route (own engine on
//! wgpu + winit) is not written yet.

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
                "Engine route chosen: own engine on wgpu + winit (docs/verification/ENGINE_BENCHMARK.md); \
                 the host is not written yet, nothing to run."
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ow-host refuses to start: {e}. The original game must be installed (AGENTS.md rule 11).");
            ExitCode::from(1)
        }
    }
}
