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
//! - `ow-host --game <install> --map NAME --play` (lane H4-PLAY): walks the map as the pawn in the window, first
//!   person, from its player start: WASD, the mouse, space to jump, Escape to let go of the cursor (a click takes it
//!   again), F1 for the fly camera. Runs until the window is closed. `--play-script FILE` drives the pawn from a
//!   script instead (`movement::walk::parse_script`) and ends when it does; `--size` and `--no-vsync` work as for
//!   `--map`, and `--screenshot PATH` writes the start frame there (presented frame 30, or the last frame of a
//!   scripted run that ends sooner) and, for a scripted run only, its last frame beside it as `<stem>_end.png`; an
//!   interactive run never finishes, so it writes no end frame. `--arms DIR` (lane H6-SKIN) also draws Maya's
//!   first-person arms, skinned on the GPU and posed by her hands AnimTree's idle; `DIR` is a UModel export folder
//!   of `GD_Siren_Streaming_SF` under `local/` (`anim::arms`).
//!   `NAME` may leave off `_P` (`Sanctuary` is `Sanctuary_P`). A heavy run: the caller takes the machine lock.
//! - `--weapon FILE` (with `--play` or `--play-script`, lane H4-WEAPON): the weapon the left mouse button (or a script
//!   segment with the `f` action) fires, named by a file (`weapon::parse_spec`); without it the weapon is the
//!   player's equipped one, which is `NotYet`, and a shot still traces its ray but has no damage number.
//! - `ow-host --game <install> --map NAME --play-script FILE` (no `--play`): the same script through the same
//!   simulation with no window, at uneven frame times (`play::HEADLESS_FRAMES`), printing the path digests.
//! - The play mode (window or headless) runs on the map loaded as a world (lane H-WORLD, `crate::world`): its placed
//!   actors in an `ow_world` world, ticked once per step. `--no-world` runs on the scene alone, as before H-WORLD.
//!   `--world-pawns DIR` (with `--play`, repeatable) draws the world's placed pawns from the UModel exports under
//!   `DIR`; a pawn whose mesh has no export there is reported and not drawn.
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
                         ow-host --game <Borderlands 2 install folder> --map NAME --play [--play-script FILE] [--weapon FILE] [--arms DIR] [--pawn NAME DIR X Y Z YAW]... [--world-pawns DIR]... [--no-world] [--size WxH] [--screenshot PATH] [--no-vsync]\n       \
                         ow-host --game <Borderlands 2 install folder> --map NAME --play-script FILE [--weapon FILE] [--no-world]\n       \
                         ow-host --headless-probe";

/// One `--pawn NAME DIR X Y Z YAW` (lane H-PAWNS): a pawn kind (`anim::pawns::KINDS`), the UModel export folder of
/// its package, its location (uu) and its yaw (degrees).
#[derive(Debug, Clone, PartialEq)]
pub struct PawnArg {
    pub kind: String,
    pub dir: PathBuf,
    pub location: [f32; 3],
    pub yaw_degrees: f64,
}

/// What to run.
#[derive(Debug, Clone, PartialEq)]
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
    /// Walk map `map` in the window (lane H4-PLAY), from the player or from `script`.
    Play {
        game: PathBuf,
        map: String,
        script: Option<PathBuf>,
        vsync: bool,
        size: Option<(u32, u32)>,
        screenshot: Option<PathBuf>,
        weapon: Option<PathBuf>,
        /// A UModel export folder to draw the first-person arms from (lane H6-SKIN).
        arms: Option<PathBuf>,
        /// Third-person pawns to draw (lane H-PAWNS), in the order given.
        pawns: Vec<PawnArg>,
        /// Run on the map loaded as a world (lane H-WORLD); `--no-world` clears it.
        world: bool,
        /// UModel export folders to draw the world's placed pawns from (lane H-WORLD).
        world_pawns: Vec<PathBuf>,
    },
    /// The script through the play simulation with no window.
    PlayHeadless { game: PathBuf, map: String, script: PathBuf, weapon: Option<PathBuf>, world: bool },
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
    let mut play = false;
    let mut play_script: Option<PathBuf> = None;
    let mut weapon: Option<PathBuf> = None;
    let mut arms: Option<PathBuf> = None;
    let mut pawns: Vec<PawnArg> = Vec::new();
    let mut no_world = false;
    let mut world_pawns: Vec<PathBuf> = Vec::new();
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
            "--play-script" => {
                let path = it.next().ok_or("--play-script needs a file")?;
                if play_script.replace(PathBuf::from(path)).is_some() {
                    return Err("--play-script given twice".into());
                }
            }
            "--weapon" => {
                let path = it.next().ok_or("--weapon needs a file")?;
                if weapon.replace(PathBuf::from(path)).is_some() {
                    return Err("--weapon given twice".into());
                }
            }
            "--pawn" => {
                let mut next = |what: &str| {
                    it.next()
                        .and_then(|a| a.to_str())
                        .map(str::to_string)
                        .ok_or_else(|| format!("--pawn needs NAME DIR X Y Z YAW (missing {what})"))
                };
                let kind = next("NAME")?;
                let dir = PathBuf::from(next("DIR")?);
                let mut number = |what: &str| -> Result<f64, String> {
                    let text = next(what)?;
                    text.parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite() && v.abs() <= 1.0e7)
                        .ok_or_else(|| format!("--pawn {what} must be a finite number, not {text}"))
                };
                let (x, y, z, yaw) = (number("X")?, number("Y")?, number("Z")?, number("YAW")?);
                // Checked: each is finite and within 1e7 uu, so the f64-to-f32 `as` rounds and stays finite.
                let location = [x as f32, y as f32, z as f32];
                pawns.push(PawnArg { kind, dir, location, yaw_degrees: yaw });
            }
            "--arms" => {
                let path = it.next().ok_or("--arms needs a folder")?;
                if arms.replace(PathBuf::from(path)).is_some() {
                    return Err("--arms given twice".into());
                }
            }
            "--world-pawns" => world_pawns.push(PathBuf::from(it.next().ok_or("--world-pawns needs a folder")?)),
            "--no-world" => no_world = set_once(no_world, "--no-world")?,
            "--play" => play = set_once(play, "--play")?,
            "--window" => window = set_once(window, "--window")?,
            "--no-vsync" => no_vsync = set_once(no_vsync, "--no-vsync")?,
            "--headless-probe" => probe = set_once(probe, "--headless-probe")?,
            "--walk" => walk = set_once(walk, "--walk")?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if weapon.is_some() && !(play || play_script.is_some()) {
        return Err("--weapon needs --play or --play-script".into());
    }
    if (arms.is_some() || !pawns.is_empty()) && !play {
        return Err("--arms and --pawn need --play".into());
    }
    if no_world && !(play || play_script.is_some()) {
        return Err("--no-world needs --play or --play-script".into());
    }
    if !world_pawns.is_empty() && (!play || no_world) {
        return Err("--world-pawns needs --play on the world (not with --no-world)".into());
    }
    if map.is_none() && (size.is_some() || screenshot.is_some()) {
        return Err("--size and --screenshot need --map".into());
    }
    if probe {
        if game.is_some()
            || window
            || frames.is_some()
            || time_limit.is_some()
            || no_vsync
            || walk
            || map.is_some()
            || play
            || play_script.is_some()
        {
            return Err("--headless-probe takes no other argument".into());
        }
        return Ok(Command::HeadlessProbe);
    }
    let game = game.ok_or("--game is required")?;
    if walk {
        if window || frames.is_some() || time_limit.is_some() || no_vsync || map.is_some() || play || play_script.is_some() {
            return Err("--walk takes only --game".into());
        }
        return Ok(Command::Walk { game });
    }
    if play || play_script.is_some() {
        let map = map.ok_or("--play and --play-script need --map")?;
        if window || frames.is_some() || time_limit.is_some() {
            return Err("--play runs until it is closed or its script ends: no --window, --frames or --time-limit".into());
        }
        if play {
            return Ok(Command::Play {
                game,
                map,
                script: play_script,
                vsync: !no_vsync,
                size,
                screenshot,
                weapon,
                arms,
                pawns,
                world: !no_world,
                world_pawns,
            });
        }
        if no_vsync || size.is_some() || screenshot.is_some() {
            return Err("--play-script without --play opens no window: no --no-vsync, --size or --screenshot".into());
        }
        let script = play_script.ok_or("--play-script needs a file")?;
        return Ok(Command::PlayHeadless { game, map, script, weapon, world: !no_world });
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
        if screenshot.is_some() && frames.is_some_and(|n| n < SCREENSHOT_FRAME) {
            return Err(format!(
                "--screenshot is taken on frame {SCREENSHOT_FRAME}: --frames must be at least that"
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

/// The presented frame a `--screenshot` is taken on (lane H2): a `--frames` run must present at least this many (the
/// shot is written as that frame is presented, before the run counts its frames and stops).
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
            &["--game", "X", "--frames", "29", "--map", "A", "--screenshot", "a.png"],
        ] {
            assert!(p(bad).is_err(), "{bad:?} was accepted");
        }
        // The shot is taken on frame 30, so 30 frames are enough (H2-DRAW review r2).
        assert!(p(&["--game", "X", "--frames", "30", "--map", "A", "--screenshot", "a.png"]).is_ok());
    }

    #[test]
    fn play_needs_a_map_and_runs_with_or_without_a_window() {
        assert_eq!(
            p(&["--game", "X", "--map", "Sanctuary", "--play"]),
            Ok(Command::Play {
                game: "X".into(),
                map: "Sanctuary".into(),
                script: None,
                vsync: true,
                size: None,
                screenshot: None,
                weapon: None,
                arms: None,
                pawns: Vec::new(),
                world: true,
                world_pawns: Vec::new(),
            })
        );
        assert_eq!(
            p(&["--game", "X", "--map", "S", "--play", "--play-script", "w.txt", "--size", "1280x720", "--screenshot", "a.png", "--no-vsync"]),
            Ok(Command::Play {
                game: "X".into(),
                map: "S".into(),
                script: Some("w.txt".into()),
                vsync: false,
                size: Some((1280, 720)),
                screenshot: Some("a.png".into()),
                weapon: None,
                arms: None,
                pawns: Vec::new(),
                world: true,
                world_pawns: Vec::new(),
            })
        );
        assert_eq!(
            p(&["--game", "X", "--map", "S", "--play", "--arms", "exports"]),
            Ok(Command::Play {
                game: "X".into(),
                map: "S".into(),
                script: None,
                vsync: true,
                size: None,
                screenshot: None,
                weapon: None,
                arms: Some("exports".into()),
                pawns: Vec::new(),
                world: true,
                world_pawns: Vec::new(),
            })
        );
        assert_eq!(
            p(&["--game", "X", "--map", "S", "--play-script", "w.txt"]),
            Ok(Command::PlayHeadless { game: "X".into(), map: "S".into(), script: "w.txt".into(), weapon: None, world: true })
        );
        assert_eq!(
            p(&["--game", "X", "--map", "S", "--play-script", "w.txt", "--weapon", "gun.txt"]),
            Ok(Command::PlayHeadless {
                game: "X".into(),
                map: "S".into(),
                script: "w.txt".into(),
                weapon: Some("gun.txt".into()),
                world: true,
            })
        );
        assert!(matches!(
            p(&["--game", "X", "--map", "S", "--play", "--weapon", "gun.txt"]),
            Ok(Command::Play { weapon: Some(_), .. })
        ));
        for bad in [
            &["--game", "X", "--weapon", "gun.txt"][..],
            &["--game", "X", "--map", "S", "--window", "--weapon", "gun.txt"],
            &["--game", "X", "--map", "S", "--play", "--weapon", "a", "--weapon", "b"],
            &["--game", "X", "--map", "S", "--play", "--weapon"],
        ] {
            assert!(p(bad).is_err(), "{bad:?} was accepted");
        }
        assert_eq!(
            p(&["--game", "X", "--map", "S", "--play", "--pawn", "bullymong", "d", "1", "-2", "3.5", "225", "--pawn",
                "maya", "e", "0", "0", "0", "0"]),
            Ok(Command::Play {
                game: "X".into(),
                map: "S".into(),
                script: None,
                vsync: true,
                size: None,
                screenshot: None,
                weapon: None,
                arms: None,
                pawns: vec![
                    PawnArg { kind: "bullymong".into(), dir: "d".into(), location: [1.0, -2.0, 3.5], yaw_degrees: 225.0 },
                    PawnArg { kind: "maya".into(), dir: "e".into(), location: [0.0; 3], yaw_degrees: 0.0 },
                ],
                world: true,
                world_pawns: Vec::new(),
            })
        );
        // Lane H-WORLD: the world is on unless --no-world; --world-pawns needs --play on the world.
        assert!(matches!(
            p(&["--game", "X", "--map", "S", "--play-script", "w.txt", "--no-world"]),
            Ok(Command::PlayHeadless { world: false, .. })
        ));
        assert!(matches!(
            p(&["--game", "X", "--map", "S", "--play", "--world-pawns", "a", "--world-pawns", "b"]),
            Ok(Command::Play { world: true, ref world_pawns, .. }) if world_pawns.len() == 2
        ));
        for bad in [
            &["--game", "X", "--map", "S", "--play", "--no-world", "--world-pawns", "a"][..],
            &["--game", "X", "--map", "S", "--play-script", "w.txt", "--world-pawns", "a"],
            &["--game", "X", "--map", "S", "--play", "--world-pawns"],
            &["--game", "X", "--map", "S", "--play", "--no-world", "--no-world"],
            &["--game", "X", "--map", "S", "--window", "--no-world"],
        ] {
            assert!(p(bad).is_err(), "{bad:?} was accepted");
        }
        for bad in [
            &["--game", "X", "--play"][..],
            &["--game", "X", "--play-script", "w.txt"],
            &["--game", "X", "--map", "S", "--play", "--window"],
            &["--game", "X", "--map", "S", "--play", "--frames", "60"],
            &["--game", "X", "--map", "S", "--play", "--play"],
            &["--game", "X", "--map", "S", "--play-script", "a", "--play-script", "b"],
            &["--game", "X", "--map", "S", "--play-script", "w.txt", "--size", "10x10"],
            &["--game", "X", "--map", "S", "--play-script", "w.txt", "--no-vsync"],
            &["--game", "X", "--walk", "--play"],
            &["--headless-probe", "--play"],
            &["--game", "X", "--map", "S", "--play-script"],
            &["--game", "X", "--map", "S", "--play-script", "w.txt", "--arms", "e"],
            &["--game", "X", "--map", "S", "--play", "--arms", "a", "--arms", "b"],
            &["--game", "X", "--map", "S", "--play", "--arms"],
            &["--game", "X", "--map", "S", "--play-script", "w.txt", "--pawn", "maya", "e", "0", "0", "0", "0"],
            &["--game", "X", "--map", "S", "--play", "--pawn", "maya", "e", "0", "0", "0"],
            &["--game", "X", "--map", "S", "--play", "--pawn", "maya", "e", "0", "nan", "0", "0"],
            &["--game", "X", "--map", "S", "--play", "--pawn", "maya", "e", "0", "0", "1e9", "0"],
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
