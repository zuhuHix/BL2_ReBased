//! `ow-host`: checks the Borderlands 2 install, then (with `--window` or `--frames N`) opens the host window and runs
//! the frame loop; `--walk` runs the scripted Sanctuary walk headless (lane H4); `--headless-probe` prints the GPU adapter without a window. Commands and exit codes in
//! `args.rs` and below.
//!
//! Exit codes: 0 done; 1 refused or failed (no install, a GPU or window error, a `--frames` run closed early, a
//! `--walk` that could not load or did not pass its checks);
//! 2 bad command line; 4 `--headless-probe` found no adapter (no GPU or driver: not a fault of the build).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Instant;

use ow_host::app::{self, Outcome, RunOptions};
use ow_host::args::{self, Command};
use ow_host::gpu;
use ow_host::sim::StubWorld;
use ow_host::stats::millis;

use ow_core::{PackageName, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_host::camera::FlyCamera;
use ow_host::draw::DrawPlan;
use ow_host::render::{DrawReport, SceneDrawer};
use ow_host::scene::{Scene, SceneAssets};

/// How far above a player start the camera starts, uu (`tools/prepare_level.py` raises its camera by 100).
const EYE_HEIGHT: f32 = 100.0;
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
        Command::Walk { game } => match check(&game) {
            Ok(()) => walk(&game),
            Err(code) => code,
        },
        Command::Scene { game, map, frames, vsync, time_limit, size, screenshot } => match check(&game) {
            Ok(()) => scene_window(
                &game,
                &map,
                RunOptions { frames, vsync, time_limit: time_limit.map(|s| Duration::from_secs(u64::from(s))) },
                size,
                screenshot,
            ),
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

fn walk(game: &std::path::Path) -> ExitCode {
    let install = match ow_host::find_install(game) {
        Ok(install) => install,
        Err(e) => {
            eprintln!("ow-host --walk: {e}");
            return ExitCode::from(1);
        }
    };
    match ow_host::movement::sanctuary::run(&install) {
        Ok(run) => {
            print!("{}", run.text);
            if run.passed { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        Err(e) => {
            eprintln!("ow-host --walk: {e}");
            ExitCode::from(1)
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

/// `--map`: loads the scene, builds its draw plan and runs the window with it (lane H2).
fn scene_window(
    game: &Path,
    map: &str,
    options: RunOptions,
    size: Option<(u32, u32)>,
    screenshot: Option<PathBuf>,
) -> ExitCode {
    let Ok(install) = ow_host::find_install(game) else {
        return ExitCode::from(1);
    };
    let Ok(name) = PackageName::parse(map) else {
        eprintln!("ow-host: {map:?} is not a package name");
        return ExitCode::from(2);
    };
    let started = Instant::now();
    // The host runs no parity stand-in: the scene is read in ParityMode::Off.
    let mut defs = match DefStore::open(install.cooked.clone(), IdentityPolicy::FirstInScanOrder, ParityMode::Off) {
        Ok(defs) => defs,
        Err(e) => {
            eprintln!("ow-host: {e}");
            return ExitCode::from(1);
        }
    };
    let mut space = ow_object::heap::ObjectSpace::new();
    let scene = match Scene::load(&mut defs, &mut space, &SceneAssets::new(install.cooked.clone()), &name) {
        Ok(scene) => scene,
        Err(e) => {
            eprintln!("ow-host: loading {map}: {e}");
            return ExitCode::from(1);
        }
    };
    let loaded = started.elapsed();
    let plan = DrawPlan::build(&scene);
    let planned = started.elapsed();
    let s = plan.stats;
    println!("scene {map}: {} levels, {} placements, loaded in {} ms", scene.levels.len(), s.placements, millis(loaded));
    println!(
        "draw plan in {} ms: {} drawn, {} hidden, {} without a decoded mesh, {} refused transforms, {} mirrored",
        millis(planned - loaded),
        s.drawn,
        s.hidden,
        s.no_mesh,
        s.bad_transform,
        s.mirrored
    );
    println!(
        "{} meshes, {} materials with diffuse candidates, {} batches, {} section instances ({} empty sections skipped)",
        plan.meshes.len(),
        plan.diffuse.len(),
        plan.batches.len(),
        s.instances,
        s.empty_sections
    );
    println!(
        "untextured section instances: {} with no material, {} with a material not in the scene, {} whose material \
         names no diffuse",
        s.untextured_no_material, s.untextured_unknown_material, s.untextured_no_diffuse
    );
    let start = scene.player_starts.values().next();
    let camera = match start {
        Some(p) => {
            let [x, y, z] = p.pose.location;
            FlyCamera::at([x, y, z + EYE_HEIGHT], p.pose.rotation)
        }
        None => FlyCamera::at([0.0; 3], Default::default()),
    };
    match start {
        Some(p) => println!("camera at player start {} (+{} uu)", p.path, EYE_HEIGHT),
        None => println!("no player start: camera at the origin"),
    }
    // The texture source owns the scene and the store, and decodes one texture at a time while the renderer uploads.
    let source = Box::new(move |path: &str| scene.decode_texture(&defs, path).map_err(|e| e.to_string()));
    let report = Rc::new(RefCell::new(DrawReport::default()));
    let drawer = SceneDrawer::new(
        plan,
        source,
        camera,
        size,
        screenshot.clone().map(|path| (u64::from(args::SCREENSHOT_FRAME), path)),
        Rc::clone(&report),
    );
    let code = window_with(options, Some(Box::new(drawer)));
    let r = report.borrow();
    let u = r.upload;
    println!(
        "upload in {} ms: {} meshes ({} vertex and {} index bytes), {} instance bytes, {} textures ({} bytes), \
         {} candidate textures failed, {} materials on a later candidate, {} materials grey; {} instanced draws \
         per frame",
        millis(r.upload_time),
        u.meshes,
        u.vertex_bytes,
        u.index_bytes,
        u.instance_bytes,
        u.textures,
        u.texture_bytes,
        u.textures_failed,
        u.fallbacks,
        u.grey_materials,
        r.draw_calls
    );
    if let Some((w, h)) = r.size {
        println!("drawable size {w}x{h}");
    }
    if let (Some(median), Some(p95)) = (r.quantile(0.5), r.quantile(0.95)) {
        println!(
            "frame time over {} intervals (nearest rank): median {} ms, p95 {} ms",
            r.intervals.len(),
            millis(median),
            millis(p95)
        );
    }
    // A screenshot asked for and not written fails the run, whatever the window did.
    let shot_failed = match (&screenshot, &r.screenshot) {
        (_, Some(Ok(path))) => {
            println!("screenshot: {}", path.display());
            false
        }
        (_, Some(Err(e))) => {
            eprintln!("ow-host: screenshot failed: {e}");
            true
        }
        (Some(_), None) => {
            eprintln!("ow-host: no screenshot: the run ended before frame {}", args::SCREENSHOT_FRAME);
            true
        }
        (None, None) => false,
    };
    if shot_failed { ExitCode::from(1) } else { code }
}

fn window(options: RunOptions) -> ExitCode {
    window_with(options, None)
}

fn window_with(options: RunOptions, drawer: Option<Box<dyn ow_host::render::Drawer>>) -> ExitCode {
    let report = match app::run_with(options, StubWorld::new(), drawer) {
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
