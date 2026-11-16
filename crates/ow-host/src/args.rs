//! Owner: lane H1. The host's command line.
//!
//! AI-assisted (Claude Code).
//!
//! - `ow-host --game <install>`: checks the install and stops (needs no GPU).
//! - `ow-host --game <install> --window`: checks the install, then opens the window until it is closed.
//! - `ow-host --game <install> --frames N`: the same, but exits after N presented frames and prints frame times.
//! - `--no-vsync` (with `--window` or `--frames`): asks for a present mode without vsync, for timing runs.
//! - `--time-limit S` (with `--frames`): fail the run if its frames are not presented within S seconds, instead of
//!   the default limit of `app::frames_time_limit` (60 s plus 1 s per frame).
//! - `ow-host --game <install> --walk`: checks the install, loads Sanctuary and its collision, and runs the scripted
//!   walk from its player start headless, printing positions and counts (lane H4; no window: H2 stage 2 draws).
//!   It loads the whole scene, so it is a heavy run: the caller takes the machine-wide lock first, as for window
//!   runs (`docs/verification/GATES.md`); the host takes none itself.
//! - `--map NAME` (with `--window` or `--frames`, lane H2): loads that map's scene from the install and draws it from
//!   its player start; `--size WxH` asks for a window of that many physical pixels, `--screenshot PATH` writes a PNG
//!   of an early frame.
//! - `ow-host --headless-probe`: creates a wgpu instance and adapter with no window and prints the adapter. It reads
//!   nothing from any install, so it takes no `--game`.
//!
//! Every window run needs `--game`: the host never runs without the player's installed game (AGENTS.md rule 11),
//! even while it draws nothing from it.

use std::ffi::OsString;
use std::path::PathBuf;

pub const USAGE: &str = "usage: ow-host --game <Borderlands 2 install folder> [--window | --frames N [--time-limit S]] [--no-vsync]\n       \
                         [--map NAME [--size WxH] [--screenshot PATH]]\n       \
                         ow-host --game <Borderlands 2 install folder> --walk\n       \
                         ow-host --headless-probe";

/// What to run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Command {
    /// Check the install and stop.
    Check { game: PathBuf },
    /// Check the install, then run the window: until closed (`frames: None`) or for that many presented frames,
    /// within `time_limit` seconds when given.
    Window { game: PathBuf, frames: Option<u32>, vsync: bool, time_limit: Option<u32> },
    /// Check the install, then the scripted Sanctuary walk, headless.
    Walk { game: PathBuf },
    /// Instance and adapter only, no window.
    HeadlessProbe,
    /// [`Command::Window`] drawing map `map`'s scene, in a window of `size` physical pixels when given, writing a
    /// screenshot to `screenshot` when given.
    Scene {
        game: PathBuf,
        map: String,
        frames: Option<u32>,
        vsync: bool,
        time_limit: Option<u32>,
        size: Option<(u32, u32)>,
        screenshot: Option<PathBuf>,
    },
}

/// `WxH` with both parts positive whole numbers.
fn parse_size(text: &str) -> Option<(u32, u32)> {
    let (w, h) = text.split_once(['x', 'X'])?;
    let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

/// Parses the arguments after the program name. The error is a one-line reason; print [`USAGE`] after it.
pub fn parse(args: &[OsString]) -> Result<Command, String> {
    let mut game: Option<PathBuf> = None;
    let mut window = false;
    let mut frames: Option<u32> = None;
    let mut time_limit: Option<u32> = None;
    let mut no_vsync = false;
    let mut probe = false;
    let mut walk = false;
    let mut map: Option<String> = None;
    let mut size: Option<(u32, u32)> = None;
    let mut screenshot: Option<PathBuf> = None;
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
            "--frames" | "--time-limit" => {
                let text = it.next().ok_or_else(|| format!("{flag} needs a number"))?;
                let count = text
                    .to_str()
                    .and_then(|t| t.parse::<u32>().ok())
                    .filter(|&n| n > 0)
                    .ok_or_else(|| format!("{flag} needs a positive whole number, not {}", text.to_string_lossy()))?;
                let slot = if flag == "--frames" { &mut frames } else { &mut time_limit };
                if slot.replace(count).is_some() {
                    return Err(format!("{flag} given twice"));
                }
            }
            "--map" => {
                let name = it.next().and_then(|n| n.to_str()).ok_or("--map needs a package name")?;
                if map.replace(name.to_string()).is_some() {
                    return Err("--map given twice".into());
                }
            }
            "--size" => {
                let text = it.next().ok_or("--size needs WxH")?;
                let parsed = text
                    .to_str()
                    .and_then(parse_size)
                    .ok_or_else(|| format!("--size needs WxH in pixels, not {}", text.to_string_lossy()))?;
                if size.replace(parsed).is_some() {
                    return Err("--size given twice".into());
                }
            }
            "--screenshot" => {
                let path = it.next().ok_or("--screenshot needs a file")?;
                if screenshot.replace(PathBuf::from(path)).is_some() {
                    return Err("--screenshot given twice".into());
                }
            }
            "--window" => window = set_once(window, "--window")?,
            "--no-vsync" => no_vsync = set_once(no_vsync, "--no-vsync")?,
            "--headless-probe" => probe = set_once(probe, "--headless-probe")?,
            "--walk" => walk = set_once(walk, "--walk")?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if map.is_none() && (size.is_some() || screenshot.is_some()) {
        return Err("--size and --screenshot need --map".into());
    }
    if probe {
        if game.is_some() || window || frames.is_some() || time_limit.is_some() || no_vsync || walk || map.is_some() {
            return Err("--headless-probe takes no other argument".into());
        }
        return Ok(Command::HeadlessProbe);
    }
    let game = game.ok_or("--game is required")?;
    if walk {
        if window || frames.is_some() || time_limit.is_some() || no_vsync || map.is_some() {
            return Err("--walk takes only --game".into());
        }
        return Ok(Command::Walk { game });
    }
    if window && frames.is_some() {
        return Err("--window and --frames exclude each other".into());
    }
    if time_limit.is_some() && frames.is_none() {
        return Err("--time-limit needs --frames".into());
    }
    if let Some(map) = map {
        if !(window || frames.is_some()) {
            return Err("--map needs --window or --frames".into());
        }
        if screenshot.is_some() && frames.is_some_and(|n| n <= SCREENSHOT_FRAME) {
            return Err(format!(
                "--screenshot is taken after frame {SCREENSHOT_FRAME}: --frames must be more than that"
            ));
        }
        return Ok(Command::Scene { game, map, frames, vsync: !no_vsync, time_limit, size, screenshot });
    }
    if window || frames.is_some() {
        return Ok(Command::Window { game, frames, vsync: !no_vsync, time_limit });
    }
    if no_vsync {
        return Err("--no-vsync needs --window or --frames".into());
    }
    Ok(Command::Check { game })
}

/// The presented frame a `--screenshot` is taken after (lane H2): a `--frames` run must present more than this.
pub const SCREENSHOT_FRAME: u32 = 30;

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
        assert_eq!(
            p(&["--game", "X", "--window"]),
            Ok(Command::Window { game: "X".into(), frames: None, vsync: true, time_limit: None })
        );
        assert_eq!(
            p(&["--frames", "300", "--game", "X", "--no-vsync"]),
            Ok(Command::Window { game: "X".into(), frames: Some(300), vsync: false, time_limit: None })
        );
        assert_eq!(
            p(&["--game", "X", "--frames", "300", "--time-limit", "15"]),
            Ok(Command::Window { game: "X".into(), frames: Some(300), vsync: true, time_limit: Some(15) })
        );
    }

    #[test]
    fn the_walk_takes_only_the_game() {
        assert_eq!(p(&["--game", "X", "--walk"]), Ok(Command::Walk { game: "X".into() }));
        assert_eq!(p(&["--walk", "--game", "X"]), Ok(Command::Walk { game: "X".into() }));
        for bad in [
            &["--walk"][..],
            &["--game", "X", "--walk", "--window"],
            &["--game", "X", "--walk", "--frames", "5"],
            &["--game", "X", "--walk", "--no-vsync"],
            &["--game", "X", "--walk", "--walk"],
            &["--headless-probe", "--walk"],
        ] {
            assert!(p(bad).is_err(), "{bad:?} was accepted");
        }
    }

    #[test]
    fn the_probe_stands_alone() {
        assert_eq!(p(&["--headless-probe"]), Ok(Command::HeadlessProbe));
        assert!(p(&["--headless-probe", "--game", "X"]).is_err());
    }

    #[test]
    fn a_scene_run_names_its_map() {
        assert_eq!(
            p(&["--game", "X", "--frames", "60", "--map", "Test_P", "--size", "1920x1080", "--screenshot", "a.png"]),
            Ok(Command::Scene {
                game: "X".into(),
                map: "Test_P".into(),
                frames: Some(60),
                vsync: true,
                time_limit: None,
                size: Some((1920, 1080)),
                screenshot: Some("a.png".into()),
            })
        );
        for bad in [
            &["--game", "X", "--map", "Test_P"][..],
            &["--game", "X", "--window", "--size", "10x10"],
            &["--game", "X", "--window", "--map", "A", "--map", "B"],
            &["--game", "X", "--window", "--map", "A", "--size", "0x10"],
            &["--game", "X", "--window", "--map", "A", "--size", "wide"],
            &["--headless-probe", "--map", "A"],
            &["--game", "X", "--frames", "30", "--map", "A", "--screenshot", "a.png"],
        ] {
            assert!(p(bad).is_err(), "{bad:?} was accepted");
        }
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
            &["--game", "X", "--window", "--time-limit", "5"],
            &["--game", "X", "--frames", "5", "--time-limit", "0"],
            &["--game", "X", "--frames", "5", "--time-limit", "5", "--time-limit", "6"],
            &["--headless-probe", "--time-limit", "5"],
        ] {
            assert!(p(bad).is_err(), "{bad:?} was accepted");
        }
    }
}
