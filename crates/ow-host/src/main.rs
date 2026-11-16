//! `ow-host`: checks the Borderlands 2 install, then (with `--window` or `--frames N`) opens the host window and runs
//! the frame loop; `--walk` runs the scripted Sanctuary walk headless (lane H4); `--play` walks a map in the window
//! and `--play-script` drives it from a script, in the window or headless (lane H4-PLAY); `--headless-probe` prints the GPU adapter without a window. Commands and exit codes in
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
use ow_host::lighting::SceneLights;
use ow_host::material::{Blend, MaterialLibrary, decode_texture};
use ow_host::render::{DrawReport, Lighting, SceneDrawer};
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
        Command::Play { game, map, script, vsync, size, screenshot } => match check(&game) {
            Ok(()) => play(&game, &map, script.as_deref(), Some(PlayWindow { vsync, size, screenshot })),
            Err(code) => code,
        },
        Command::PlayHeadless { game, map, script } => match check(&game) {
            Ok(()) => play(&game, &map, Some(&script), None),
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
    let map = &ow_host::movement::sanctuary::resolve_map(&install, map);
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
    // Lane H8: the materials' and lights' reads, then the plan with them; the named-only plan for the counts before.
    let library = MaterialLibrary::read(&mut defs, &mut space, &scene);
    let lights = SceneLights::read(&mut defs, &mut space, &scene);
    let materials_read = started.elapsed();
    let before = DrawPlan::build(&scene).stats;
    let plan = DrawPlan::build_with(&scene, Some(&library));
    let planned = started.elapsed();
    let s = plan.stats;
    println!(
        "materials and lights read in {} ms: {} materials, {} textures only cooked lists name",
        millis(materials_read - loaded),
        library.materials.len(),
        library.textures.len()
    );
    let lighting = Lighting::from_scene(&lights);
    match &lights.sun {
        Some(sun) => println!(
            "sun {}:{}, brightness {}, toward {:?}; sky light {}; exposure {}",
            sun.level,
            sun.path,
            sun.brightness,
            sun.toward(),
            lights.sky.as_ref().map_or("none".to_string(), |k| format!("{}:{}", k.level, k.path)),
            lighting.exposure
        ),
        None => println!("no directional light: the viewer's constant light"),
    }
    let l = &lights.lightmaps;
    println!(
        "lightmaps (not decoded): {} LightMapTexture2D, {} ShadowMap2D, {} ShadowMap1D; the texture reader on the first: {}",
        l.light_map_textures,
        l.shadow_maps_2d,
        l.shadow_maps_1d,
        match &l.decodable {
            Some(Ok(())) => "decoded".to_string(),
            Some(Err(e)) => e.clone(),
            None => "none to try".to_string(),
        }
    );
    println!("scene {map}: {} levels, {} placements, loaded in {} ms", scene.levels.len(), s.placements, millis(loaded));
    println!(
        "draw plan in {} ms: {} drawn, {} hidden, {} without a decoded mesh, {} refused transforms, {} mirrored",
        millis(planned - materials_read),
        s.drawn,
        s.hidden,
        s.no_mesh,
        s.bad_transform,
        s.mirrored
    );
    println!(
        "{} meshes, {} material slots, {} batches, {} section instances ({} empty sections skipped)",
        plan.meshes.len(),
        plan.slots.len(),
        plan.batches.len(),
        s.instances,
        s.empty_sections
    );
    println!(
        "untextured section instances before (named diffuse only): {} with no material, {} with a material not in \
         the scene, {} whose material names no diffuse",
        before.untextured_no_material, before.untextured_unknown_material, before.untextured_no_diffuse
    );
    println!(
        "untextured section instances after: {} with no material, {} with a material not in the scene, {} with no \
         diffuse; {} textured by a fallback rule, {} by a constant colour",
        s.untextured_no_material,
        s.untextured_unknown_material,
        s.untextured_no_diffuse,
        s.fallback_textured,
        s.constant_coloured
    );
    println!(
        "section instances by blend: {} opaque, {} masked, {} translucent, {} additive, {} modulate, {} sky",
        s.blend_count(Blend::Opaque),
        s.blend_count(Blend::Masked),
        s.blend_count(Blend::Translucent),
        s.blend_count(Blend::Additive),
        s.blend_count(Blend::Modulate),
        s.sky_instances
    );
    println!(
        "blended (translucent, additive, modulate) section instances not drawn: {} with no diffuse",
        s.blended_skipped
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
    let source = Box::new(move |path: &str| decode_texture(&defs, &scene, &library, path));
    let report = Rc::new(RefCell::new(DrawReport::default()));
    let drawer = SceneDrawer::new(
        plan,
        lighting,
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
    println!(
        "blended slots not drawn: {} translucent whose diffuse has no alpha below 255, {} whose candidates all \
         failed ({} section instances); {} blended instances drawn one by one",
        u.translucent_without_alpha, u.blended_undecoded, u.skipped_instances, r.blended_instances
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

/// The window part of a `--play` run.
struct PlayWindow {
    vsync: bool,
    size: Option<(u32, u32)>,
    screenshot: Option<PathBuf>,
}

/// `--play` (with `window`) or a headless `--play-script` (lane H4-PLAY).
fn play(game: &Path, map: &str, script: Option<&Path>, window: Option<PlayWindow>) -> ExitCode {
    use ow_host::movement::sanctuary;
    use ow_host::play::{self, InputCollector, PlayDrawReport, PlayDrawer, PlayStats, PlayWorld};
    let Ok(install) = ow_host::find_install(game) else {
        return ExitCode::from(1);
    };
    let script = match script {
        Some(path) => match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| ow_host::movement::walk::parse_script(&text))
        {
            Ok(script) => Some(script),
            Err(e) => {
                eprintln!("ow-host: the play script {}: {e}", path.display());
                return ExitCode::from(2);
            }
        },
        None => None,
    };
    let map = sanctuary::resolve_map(&install, map);
    let loaded = match sanctuary::load(&install, &map) {
        Ok(loaded) => loaded,
        Err(e) => {
            eprintln!("ow-host: loading {map}: {e}");
            return ExitCode::from(1);
        }
    };
    print!("{}", loaded.text);
    let mut spawn_text = String::new();
    let mover = match sanctuary::spawn(&loaded.scene, &loaded.collision.world, &mut spawn_text) {
        Ok(mover) => mover,
        Err(e) => {
            eprintln!("ow-host: {e}");
            return ExitCode::from(1);
        }
    };
    print!("{spawn_text}");
    let sanctuary::Loaded { scene, mut defs, mut space, collision, .. } = loaded;
    let Some(window) = window else {
        let Some(script) = script else { return ExitCode::from(2) };
        let started = Instant::now();
        return match play::run_headless(collision.world, mover, script, &play::HEADLESS_FRAMES) {
            Ok(stats) => {
                print!("{}", play::stats_text(&stats));
                println!("headless play in {} ms", millis(started.elapsed()));
                if stats.finished { ExitCode::SUCCESS } else { ExitCode::from(1) }
            }
            Err(e) => {
                eprintln!("ow-host: {e}");
                ExitCode::from(1)
            }
        };
    };
    let scripted = script.is_some();
    // Drawn with the materials and lights as `scene_window` draws them (lane H8); drawing never feeds the simulation.
    let read_started = Instant::now();
    let library = MaterialLibrary::read(&mut defs, &mut space, &scene);
    let lights = SceneLights::read(&mut defs, &mut space, &scene);
    let lighting = Lighting::from_scene(&lights);
    println!(
        "materials and lights read in {} ms: {} materials; exposure {}",
        millis(read_started.elapsed()),
        library.materials.len(),
        lighting.exposure
    );
    let plan = DrawPlan::build_with(&scene, Some(&library));
    println!(
        "draw plan: {} drawn, {} batches, {} section instances; eye {} uu above the pawn's centre, vertical FOV {} \
         degrees, step {} s, at most {} steps a frame",
        plan.stats.drawn,
        plan.batches.len(),
        plan.stats.instances,
        ow_core::fmt::fmt_f(mover.params.base_eye_height, 0),
        ow_core::fmt::fmt_f(play::vertical_fov_deg(), 2),
        ow_core::fmt::fmt_f(play::step_seconds(), 6),
        play::MAX_STEPS_PER_FRAME
    );
    let source = Box::new(move |path: &str| decode_texture(&defs, &scene, &library, path));
    let stats = Rc::new(RefCell::new(PlayStats::default()));
    let draw = Rc::new(RefCell::new(PlayDrawReport::default()));
    let end_shot = window.screenshot.as_ref().map(|p| {
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        p.with_file_name(format!("{stem}_end.png"))
    });
    let drawer = PlayDrawer::new(
        plan,
        lighting,
        source,
        window.size,
        window.screenshot.clone().map(|p| (u64::from(args::SCREENSHOT_FRAME), p)),
        end_shot,
        Rc::clone(&draw),
    );
    let sim = PlayWorld::new(collision.world, mover, script, Rc::clone(&stats));
    let options = RunOptions { frames: None, vsync: window.vsync, time_limit: None };
    let code = match app::run_play(options, sim, Some(Box::new(drawer)), InputCollector::new(!scripted)) {
        Ok(report) => print_run(&report, options),
        Err(e) => {
            eprintln!("ow-host: {e}");
            ExitCode::from(1)
        }
    };
    let d = draw.borrow();
    println!("upload in {} ms, {} instanced draws per frame", millis(d.draw.upload_time), d.draw.draw_calls);
    if let (Some(median), Some(p95), Some(max)) = (d.draw.quantile(0.5), d.draw.quantile(0.95), d.draw.quantile(1.0)) {
        println!(
            "frame time over {} intervals (nearest rank): median {} ms, p95 {} ms, max {} ms",
            d.draw.intervals.len(),
            millis(median),
            millis(p95),
            millis(max)
        );
    }
    let mut failed = false;
    for (what, shot) in [("start", &d.start_shot), ("end", &d.end_shot)] {
        match (shot, &window.screenshot) {
            (Some(Ok(path)), _) => println!("screenshot {what}: {}", path.display()),
            (Some(Err(e)), _) => {
                eprintln!("ow-host: the {what} screenshot failed: {e}");
                failed = true;
            }
            (None, Some(_)) if what == "start" || scripted => {
                eprintln!("ow-host: no {what} screenshot was taken");
                failed = true;
            }
            _ => {}
        }
    }
    let s = stats.borrow();
    print!("{}", play::stats_text(&s));
    // A scripted run passes only when its script ran to the end.
    if scripted && !s.finished {
        eprintln!("ow-host: the play script did not finish");
        failed = true;
    }
    if failed { ExitCode::from(1) } else { code }
}

fn window(options: RunOptions) -> ExitCode {
    window_with(options, None)
}

fn window_with(options: RunOptions, drawer: Option<Box<dyn ow_host::render::Drawer>>) -> ExitCode {
    match app::run_with(options, StubWorld::new(), drawer) {
        Ok(report) => print_run(&report, options),
        Err(e) => {
            eprintln!("ow-host: {e}");
            ExitCode::from(1)
        }
    }
}

/// Prints a window run's report and gives its exit code.
fn print_run(report: &app::RunReport, options: RunOptions) -> ExitCode {
    if let Some(adapter) = &report.adapter {
        println!(
            "adapter: {} ({}), backend: {}, driver: {}",
            adapter.name, adapter.device_type, adapter.backend, adapter.driver
        );
    }
    if let Some(mode) = &report.present_mode {
        println!("present mode asked for: {mode}");
    }
    for grab in &report.cursor {
        println!("cursor: {grab:?}");
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
