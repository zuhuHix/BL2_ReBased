//! `ow-host`: checks the Borderlands 2 install, then (with `--window` or `--frames N`) opens the host window and runs
//! the frame loop; `--headless-probe` prints the GPU adapter without a window. Commands and exit codes in
//! `args.rs` and below.
//!
//! Exit codes: 0 done; 1 refused or failed (no install, a GPU or window error, a `--frames` run closed early);
//! 2 bad command line; 4 `--headless-probe` found no adapter (no GPU or driver: not a fault of the build).

use std::process::ExitCode;

use ow_host::app::{self, Outcome, RunOptions};
use ow_host::args::{self, Command};
use ow_host::gpu;
use ow_host::sim::StubWorld;
use ow_host::stats::millis;
use std::time::Duration;

fn main() -> ExitCode {
    let raw: Vec<_> = std::env::args_os().skip(1).collect();
    let command = match args::parse(&raw) {
        Ok(command) => command,
        Err(reason) => {
            eprintln!("ow-host: {reason}\n{}", args::USAGE);
            return ExitCode::from(2);
        }
    };
    match command {
        Command::HeadlessProbe => probe(),
        Command::Check { game } => match check(&game) {
            Ok(()) => {
                println!("Install check only; add --window or --frames N to open the host window.");
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },
        Command::Window { game, frames, vsync, time_limit } => match check(&game) {
            Ok(()) => window(RunOptions {
                frames,
                vsync,
                time_limit: time_limit.map(|s| Duration::from_secs(u64::from(s))),
            }),
            Err(code) => code,
        },
        _ => ExitCode::from(2),
    }
}

fn check(game: &std::path::Path) -> Result<(), ExitCode> {
    match ow_host::find_install(game) {
        Ok(install) => {
            println!("Borderlands 2 install found: {}", install.cooked.display());
            Ok(())
        }
        Err(e) => {
            eprintln!("ow-host refuses to start: {e}. The original game must be installed (AGENTS.md rule 11).");
            Err(ExitCode::from(1))
        }
    }
}

fn probe() -> ExitCode {
    match gpu::headless_probe() {
        Ok(adapter) => {
            println!("adapter: {}", adapter.name);
            println!("backend: {}", adapter.backend);
            println!("device type: {}", adapter.device_type);
            println!("driver: {}", adapter.driver);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ow-host --headless-probe: {e}");
            ExitCode::from(4)
        }
    }
}

fn window(options: RunOptions) -> ExitCode {
    let report = match app::run(options, StubWorld::new()) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("ow-host: {e}");
            return ExitCode::from(1);
        }
    };
    if let Some(adapter) = &report.adapter {
        println!(
            "adapter: {} ({}), backend: {}, driver: {}",
            adapter.name, adapter.device_type, adapter.backend, adapter.driver
        );
    }
    if let Some(mode) = &report.present_mode {
        println!("present mode asked for: {mode}");
    }
    println!(
        "frames presented: {}, skipped: {}, first to last present: {} ms",
        report.presented,
        report.skipped,
        millis(report.elapsed)
    );
    if let Some(summary) = report.timer.summary() {
        let median = match summary.median {
            Some(median) if summary.sampled == summary.intervals => format!("median {} ms, ", millis(median)),
            Some(median) => format!("median {} ms (of the first {}), ", millis(median), summary.sampled),
            None => String::new(),
        };
        println!(
            "frame time over {} intervals: {median}min {} ms, max {} ms",
            summary.intervals,
            millis(summary.min),
            millis(summary.max)
        );
    }
    match (&report.outcome, options.frames) {
        (Outcome::Completed, _) | (Outcome::Closed, None) => ExitCode::SUCCESS,
        (Outcome::Closed, Some(n)) => {
            eprintln!("ow-host: the window was closed after {} of {n} frames", report.presented);
            ExitCode::from(1)
        }
        (Outcome::Failed(reason), _) => {
            eprintln!("ow-host: {reason}");
            ExitCode::from(1)
        }
        (_, _) => ExitCode::from(1),
    }
}
