## 2026-11-16: Host play mode: walk Sanctuary in the window (H4-PLAY)

AI-assisted (Claude Code). Lane H4-PLAY, host track; number 57 was reserved for this lane by the hub. With
`ow-host --game <install> --map Sanctuary --play` the pawn of lane H4 (decision 2026-11-16-52) walks the scene of
lane H2 (decisions 2026-11-16-46 and -51) in the window, first person, with WASD, the mouse and space. No dependency,
no new tool, no reader or bounds change. There was no real-game session: the movement and the view stay
**UNVERIFIED** in the running game. What is verified is that the window, the headless runner and `--walk` put the
pawn at the same bits on every step for the same inputs.

- **What changed.**
  - New `crates/ow-host/src/play.rs` and `play/tests.rs`, plus one `pub mod play;` line in `lib.rs`.
  - `app.rs`: `run_play`, input routing, the cursor grab.
  - `args.rs`: `--play` and `--play-script`, and H2-DRAW's review r2 nit (`--frames 30 --screenshot` is accepted;
    the shot is written on frame 30, before the run counts its frames).
  - `main.rs`: the play and headless handlers. `--map` also takes a name without `_P` (`Sanctuary` is
    `Sanctuary_P`), for scene runs too.
  - `sim.rs` (lane H1's message types, additions only): `HostCommand::Play(PlayInput)` and the snapshot's `view` and
    `finished`. It also gains H2-DRAW's other nit: a note that an sRGB surface encodes the linear clear, so the plain
    clear looks lighter.
  - `render.rs` (lane H2's): one default method on the `Drawer` trait, `snapshot`, so a drawer sees the frame's
    snapshot. `SceneDrawer` is unchanged.
  - `movement/walk.rs`: owned segment names, `segment_ticks` and `segment_input` (the one script-to-input path),
    `PathDigest`, `ScriptCursor`, and the script text format (`parse_script`, `script_text`).
  - `movement/sanctuary.rs`: `load`, `spawn` and `resolve_map`, shared by `--walk` and play; the digests in its
    report.
  - The H4-PLAY check in `docs/verification/GATES.md`.
- **Along the host boundary (D18).**
  - Host side: `play::InputCollector` turns winit's keys and raw mouse motion into one `PlayInput` message per frame.
    The message holds booleans and whole rotator units only: pixels become rotator units at 20 per pixel, a host
    choice, since the game's sensitivity is not read; the fraction is carried to the next frame.
  - Simulation: `play::PlayWorld` implements `sim::Simulation`. It runs on the event-loop thread at the fixed step
    `--walk` uses (D13's 1/60 s), from an accumulator of real frame time. A frame takes at most 8 steps; time beyond
    that is dropped and counted, so a stall never fast-forwards the pawn.
  - Renderer: `play::PlayDrawer` draws H2's `SceneRenderer` from the snapshot's view, with every matrix built in
    `frame.rs` (`view_matrix`, `perspective_reverse_z_infinite`).
- **Render interpolation.** The eye is interpolated between the last two steps by the accumulator's leftover. The
  rotation is the last step's plus the turn received but not yet stepped, so the mouse answers at the frame rate. It
  is not interpolated: it would only lag the mouse.
- **The view.**
  - The eye is the pawn's centre plus `BaseEyeHeight` 70, read live on 2026-10-21. It is the same `Mover::eye` as
    `--walk`'s. The game's eye-height easing, its starting `EyeHeight` 77 and the bob (section 15) are not applied.
  - The field of view is the record's default: 70 degrees horizontal at 16:9, which the game stores as the vertical
    `2 atan(tan(35 deg) x 0.5625)`, 42.996 degrees, 43.00 to two places (`NATIVE_PLAYER_MOVEMENT.md` sections
    18-19; the record rounds it to 43.0). Review r1 asked whether the figure is 43.07: it is not, and the play module's
    own doc, which said "about 43.07", is corrected. It is kept vertical
    at other aspects. UNVERIFIED in the game, as is the profile FOV setting's effect.
  - The near plane (10 uu) is the host's.
- **Bindings, all host choices.**
  - W, A, S and D move; the mouse turns, within the pitch limits of `MovementParams`; space jumps on the press, not
    on auto-repeat.
  - Escape lets go of the cursor, and a left click takes it again.
  - F1 toggles the fly camera (`camera::FlyCamera`, with E and Q for up and down and Shift for fast). While it flies,
    the pawn gets no input.
  - Losing focus releases every key and the cursor. The presses winit makes up for keys held when the window gains
    focus again (`is_synthetic`) are ignored, so coming back with W or space held neither walks nor jumps; a made-up
    release still releases (review r1).
- **Cursor grab, with a typed fallback.** The host locks the cursor; if the platform refuses, it confines it to the
  window. If both are refused, the cursor stays free and visible, and the raw mouse motion still turns the view
  (`app::Grab::Refused`, which carries the platform's reason). Every change is listed in the run report. A scripted run
  never grabs the cursor.
- **Scripts and determinism.**
  - `--play-script FILE` drives the pawn from a script, one input per step, whatever the frame times. The format is
    one segment per line: seconds, forward, strafe, jump 0 or 1, yaw per tick, then a name.
  - With `--play` it runs in the window and ends when the script does. Without `--play` it runs headless through the
    same `PlayWorld::step`, at the frame times of `play::HEADLESS_FRAMES`. Those are uneven on purpose (4 ms to 250 ms),
    so frames take one step, none, or the most, and time is dropped.
  - `--walk` and play turn a script into inputs through the same two functions. Both print a path digest (FNV-1a over
    every step's location bits) at each segment's end, so equal digests mean equal paths, bit for bit.
  - Every script segment is reported by its script index, a segment that rounds to no tick included, with the digest
    `walk::run` gives it (the one before the next step). Before review r1 play skipped such a segment, so every later
    "play segment N" line was compared with the wrong `--walk` line.
  - A scripted window run ends on the frame its script finishes on. If that frame cannot be presented (a minimised
    or hidden window), the run ends after 100 frames in a row that could not be presented (`app::FINISHED_SKIP_LIMIT`,
    about a second), without its end screenshot, instead of waiting for the window to come back. The start screenshot
    is taken on presented frame 30, or on the last frame of a script that finishes sooner (review r1).
- **Verified (automated, synthetic geometry).** Twelve `play` tests, one `app` test and one argument test (with the
  `--frames 30` nit among its cases):
  - one script through `walk::run` and through the play simulation at three frame-time patterns (uneven, steady 60
    Hz, 3 ms) gives the same digests at every segment end, the same step count and the same end;
  - a 1 s frame takes 8 steps and drops the rest;
  - the view is the eye interpolated at the accumulator's leftover;
  - the player's turn shows at once and reaches the pawn on its next step; W moves along the new yaw; one space
    press, one jump;
  - F1 flies the camera and leaves the pawn still;
  - the collector turns pixels into whole units and keeps the fraction, ignores a NaN motion and clamps a wild one;
  - the keys: jump only on a press, Escape, a click, focus loss, and a scripted run ignoring WASD;
  - the field of view is 42.996 degrees;
  - the Sanctuary script reads back from its text;
  - review r1: segments of no tick at the start, middle and end keep their index and `walk::run`'s digests at two
    frame-time patterns; made-up presses on focus are ignored and a made-up release releases; the start shot is due
    on a finishing frame before frame 30; a finished run ends on a present or after the bounded number of skips.

  Workspace: 1,603 passed, 0 failed, 21 ignored (main 7f73940 plus this lane); 1,607 after review r1.
- **Verified (local, installed game, zuhu desktop, RTX 4070, Vulkan, release build, every run under the machine
  lock; output under ignored `local/p2/h4-play/`).** The script is `--walk`'s nine segments (17.5 s), then eleven more
  (42.5 s): 60 s and 3,600 steps.
  - `--walk`: PASS as before (decision 52), with digest `fd6551c9598c8ec7` after its 1,050 ticks.
  - **Headless `--map Sanctuary --play-script`:** 3,600 steps over 1,715 uneven frames, 1,995 steps' time dropped,
    finished. The first nine segment digests equal `--walk`'s; the end digest is `d7b95ebfe14df774`, at
    (3594.03, -5880.78, 2664.95).
  - **Window, vsync on:** exit 0, 456 frames, all twenty segment digests and the end digest equal the headless run's.
    The frame times are meaningless: median 252 ms, p95 266 ms. That is the figure H1 recorded with this desktop's
    display asleep (GATES.md, H1 baseline). The 8-step cap dropped 3,310 steps' time, so the 60 s script took 115 s
    of wall time; the path did not change.
  - **Window, `--no-vsync`:** exit 0, 87,516 frames in 60.3 s, median 0.680 ms, p95 0.711 ms, max 436 ms (one hitch,
    which cost the only 18 dropped steps). The same twenty digests again.
  - **Screenshots** at frame 30 and at the script's end, at 1280x720: `shot.png`, `shot_end.png`, `novsync.png`,
    `novsync_end.png`.
    - The end frame shows the pawn's view over grey ground (untextured sections, H2's grey) toward props.
    - The start frame is filled by a brown, blurred texture. This is very probably the face of `Sanctuary_Outer`
      `InterpActor_34`'s BoxLrg at 20x scale: about 440 uu ahead of the spawn, across the eye's height. H2 draws it
      because it is not serialized hidden; the retired pipeline hid its visual (`prepare_level.py`
      `HIDDEN_BOX_LRG_SOURCES`). It is the same box decision 52 left as an open question for collision. So the first
      thing a player sees here is that question.
  - **After review r1** (same desktop, under the lock, released afterwards): `--walk` again PASS with digest
    `fd6551c9598c8ec7`. A scripted window run of exactly the `--walk` path (17.5 s, `--no-vsync`): exit 0, 24,297
    frames in 17.8 s, median 0.688 ms, p95 0.821 ms, both screenshots written. Its nine segment digests equal
    `--walk`'s, and its end digest is `--walk`'s whole-path digest.
  - Not run: an interactive session (keys and mouse by hand) and the cursor grab. The grab would take the
    maintainer's cursor on a shared screen, so it is left to the maintainer. The key and grab paths are covered by
    the unit tests above, not by a window run.
- **Open.**
  - A maintainer check of `--play` by hand: feel, the grab on this desktop, Escape, the click to grab again, F1.
  - Whether `_Outer`'s BoxLrg blocks or shows in landed play (decision 52's real-game ask), now also a drawing
    question for H2.
  - The game's eye height easing and bob, mouse sensitivity, FOV setting and near plane.
  - A script that finishes while the window is minimised ends without its end screenshot (above), and with
    `--screenshot` that fails the run.
  - Crouch, sprint and the other moves decision 52 left open.
