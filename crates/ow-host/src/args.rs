//! Owner: lane H1. The host's command line.
//!
//! AI-assisted (Claude Code).
//!
//! - `ow-host --game <install>`: checks the install and stops (needs no GPU).
//! - `ow-host --game <install> --window`: checks the install, then opens the window until it is closed.
//! - `ow-host --game <install> --frames N`: the same, but exits after N presented frames and prints frame times.
//! - `--no-vsync` (with `--window` or `--frames`): asks for a present mode without vsync, for timing runs.
//! - `ow-host --headless-probe`: creates a wgpu instance and adapter with no window and prints the adapter. It reads
//!   nothing from any install, so it takes no `--game`.
//!
//! Every window run needs `--game`: the host never runs without the player's installed game (AGENTS.md rule 11),
//! even while it draws nothing from it.

use std::ffi::OsString;
use std::path::PathBuf;

pub const USAGE: &str = "usage: ow-host --game <Borderlands 2 install folder> [--window | --frames N] [--no-vsync]\n       \
                         ow-host --headless-probe";

/// What to run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Command {
    /// Check the install and stop.
    Check { game: PathBuf },
    /// Check the install, then run the window: until closed (`frames: None`) or for that many presented frames.
    Window { game: PathBuf, frames: Option<u32>, vsync: bool },
    /// Instance and adapter only, no window.
    HeadlessProbe,
}

/// Parses the arguments after the program name. The error is a one-line reason; print [`USAGE`] after it.
pub fn parse(args: &[OsString]) -> Result<Command, String> {
    let mut game: Option<PathBuf> = None;
    let mut window = false;
    let mut frames: Option<u32> = None;
    let mut no_vsync = false;
    let mut probe = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let Some(flag) = arg.to_str() else {
            return Err(format!("unknown argument {}", arg.to_string_lossy()));
        };
        match flag {
            "--game" => {
                let path = it.next().ok_or("--game needs a folder")?;
                if game.replace(PathBuf::from(path)).is_some() {
                    return Err("--game given twice".into());
                }
            }
            "--frames" => {
                let text = it.next().ok_or("--frames needs a count")?;
                let count = text
                    .to_str()
                    .and_then(|t| t.parse::<u32>().ok())
                    .filter(|&n| n > 0)
                    .ok_or_else(|| format!("--frames needs a positive whole number, not {}", text.to_string_lossy()))?;
                if frames.replace(count).is_some() {
                    return Err("--frames given twice".into());
                }
            }
            "--window" => window = set_once(window, "--window")?,
            "--no-vsync" => no_vsync = set_once(no_vsync, "--no-vsync")?,
            "--headless-probe" => probe = set_once(probe, "--headless-probe")?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if probe {
        if game.is_some() || window || frames.is_some() || no_vsync {
            return Err("--headless-probe takes no other argument".into());
        }
        return Ok(Command::HeadlessProbe);
    }
    let game = game.ok_or("--game is required")?;
    if window && frames.is_some() {
        return Err("--window and --frames exclude each other".into());
    }
    if window || frames.is_some() {
        return Ok(Command::Window { game, frames, vsync: !no_vsync });
    }
    if no_vsync {
        return Err("--no-vsync needs --window or --frames".into());
    }
    Ok(Command::Check { game })
}

fn set_once(already: bool, flag: &str) -> Result<bool, String> {
    if already { Err(format!("{flag} given twice")) } else { Ok(true) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Command, String> {
        parse(&args.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn the_install_check_is_unchanged() {
        assert_eq!(p(&["--game", "X"]), Ok(Command::Check { game: "X".into() }));
    }

    #[test]
    fn window_and_frames() {
        assert_eq!(p(&["--game", "X", "--window"]), Ok(Command::Window { game: "X".into(), frames: None, vsync: true }));
        assert_eq!(
            p(&["--frames", "300", "--game", "X", "--no-vsync"]),
            Ok(Command::Window { game: "X".into(), frames: Some(300), vsync: false })
        );
    }

    #[test]
    fn the_probe_stands_alone() {
        assert_eq!(p(&["--headless-probe"]), Ok(Command::HeadlessProbe));
        assert!(p(&["--headless-probe", "--game", "X"]).is_err());
    }

    #[test]
    fn bad_command_lines_are_refused() {
        for bad in [
            &[][..],
            &["--frames", "10"],
            &["--game"],
            &["--game", "X", "--frames"],
            &["--game", "X", "--frames", "0"],
            &["--game", "X", "--frames", "-3"],
            &["--game", "X", "--frames", "ten"],
            &["--game", "X", "--window", "--frames", "5"],
            &["--game", "X", "--no-vsync"],
            &["--game", "X", "--game", "Y"],
            &["--game", "X", "--window", "--window"],
            &["--game", "X", "--fast"],
        ] {
            assert!(p(bad).is_err(), "{bad:?} was accepted");
        }
    }
}
