## 2026-11-16: Host held-weapon fire in the play mode (H4-WEAPON)

AI-assisted (Claude Code). Lane H4-WEAPON, host track. The hub named 61 as the next free number, and the decisions
log's tail confirmed it. In `ow-host --play` the left mouse button now fires a held weapon. A ray against the H4
collision gives the impact point, and the log gives the weapon's damage number. A placeholder first-person overlay
shows a crosshair, a weapon block, a muzzle flash and the impact marker. No new weapon maths, no dependency, no
reader or bounds change. There was no real-game session: every rule used stays **UNVERIFIED** in the running game.

- **What changed.**
  - New `crates/ow-host/src/weapon.rs` (and `weapon/tests.rs`) and `crates/ow-host/src/overlay.rs`, plus two `pub mod`
    lines in `lib.rs`.
  - `play.rs`: the gun in `PlayWorld` (`with_gun`), `run_headless_with`, the shot log and digest, the left button
    in the input collector, and the overlay in the drawer.
  - `sim.rs` (lane H1's message types, additions only): `PlayInput::fire` and the snapshot's `ShotView`.
  - `movement/walk.rs`: a script segment's trigger (`Segment::fire`, the `f` and `jf` actions in the script text).
  - `args.rs` and `main.rs`: `--weapon FILE`.
  - The H4-WEAPON check in `docs/verification/GATES.md`.
- **Which weapon.**
  - The player's equipped weapon would come from the save, whose format waits on D9, or from a starting loadout,
    which is not ported. So it answers `NotYet` (`weapon::equipped_weapon`).
  - `--weapon FILE` names one instead, in a `key value` file: definition, balance, manufacturer, parts by slot,
    prefix, title, level. These are the fields of `ow-package --item-stats --data`. Such a file names game objects,
    so it stays under ignored `local/`.
  - With no weapon a shot still traces, with a labelled host probe range (100,000 uu), and its damage is `NotYet`.
    It fires once per press, since there is no fire interval.
- **Its numbers come from the existing rules.**
  - `ow_items::stats::instantiate_stats` (W4-STATS) makes the instance. Then `instance_stat` reads
    `WeaponDamage`, `WeaponFireInterval` and `WeaponProjectilesPerShot`.
  - The instance's `WeaponRange` property is the hit-scan range (`Weapon.GetTraceRange`,
    NATIVE_WEAPON_TRIGGER_AMMO.md).
  - The refire time goes through `ow_combat::firing::fire_interval` (W1-FIRING). Spin-up is not applied: the firing
    mode's spin-up fields are not read here.
  - **The world it is evaluated in.** The Off-mode route `ow-package --item-stats --data` takes, `ow-balance`'s
    `OfflineWorld`, does not work for a weapon. It answers every attribute change notification `NotYet`, and
    instantiating the installed Tediore pistol notifies a `WillowWeapon` property. Seen on the installed game: "the
    change notification of WillowGame.WillowWeapon ... in the offline world: needs ow-world".
  - So the weapon is evaluated in an `ow_world::World`: the VM over the install's store, the core natives, and only
    the world's own packs. The VM dispatches a change to the owner class's registered override and does nothing
    without one (NATIVE_ATTRIBUTES.md section 4; the weapon `ClipSize` override is still open there). No rule for
    the weapon's override is registered, so a notification changes nothing. This is UNVERIFIED as the game's
    behaviour, and the offline world's refusal is reported here for the owners of `ow-balance` and `ow-cli`.
  - **The skip is visible** (review r1 of PR #366). `evaluate` registers a `WillowGame.WillowWeapon` override that only
    records the notified properties, and the weapon line names them: "notifications not applied (NotYet,
    NATIVE_ATTRIBUTES section 4): <names>". The numbers do not change.
  - **Cross-check.** For the same Tediore pistol, the Python tool's firing record under ignored `local/items/firing/`
    gives damage 34.2408, fire interval 0.21 s and trace range 16384. The Rust evaluation gives 34.24, 0.2100 s, 1
    projectile and 16384 uu. They agree, but this is the tool's reading, not the game's.
- **One shot** (`weapon::Gun::step`, once per simulation step, so a script's shots depend on its steps only).
  - A shot fires when the trigger is held and the refire time is up. Ready means within 1e-9 s of zero: a numeric
    margin, so an interval of whole steps does not fire a step late on rounding.
  - One ray from the eye along the view after the step's look, as long as the weapon's range (or the probe range),
    traced against the H4 collision as a shape of no size.
  - The damage is the weapon's `WeaponDamage` per projectile, the number its card shows. Against what the ray hits it
    is `NotYet` (`weapon::target_damage`): the play mode has no pawns, and the hit chain and damage pipeline
    (`ow-combat` `hit`, `damage`) run on a world's pawns.
  - Not applied, each open: spread, the firing pattern, accuracy, kick and recoil, one ray per pellet, ammunition and
    reload, the burst, and bullets in flight (most guns are Bullet, NATIVE_WEAPON_FIRING.md section 1.2). Their rules
    are in `ow-combat::firing`; wiring them needs the firing mode's data and a `GameRng` stream.
  - Two limits of the stepping (review r1 of PR #366): at most one shot per step, so a fire interval under 1/60 s is
    capped at one step; and every gun is treated as automatic, so a semi-automatic firing mode's one shot per press is
    not modelled.
- **Input and determinism.**
  - The trigger is a field of the per-frame `PlayInput` message. The left button fires only while the cursor is held,
    so the click that takes the cursor does not fire; a release lets go. Losing focus or letting go of the cursor
    (Escape) lets go of the trigger too, and the fly camera never fires (review r1).
  - A press is latched like the jump (review r1): the collector reports a click that began and ended between two
    messages, and the simulation keeps a trigger message until a step uses it, so a click shorter than a frame or a
    step still fires one shot.
  - A script holds the trigger with the `f` action (`jf` for a jump too). `--walk` ignores it: firing moves nothing,
    so a firing script's path digest equals a non-firing run's.
  - `PlayStats` keeps every shot with the label of the placement it hit, and a shot digest over each shot's step, end
    point, damage and range.
- **Overlay** (`overlay.rs`; sizes and colours are the host's, not the game's HUD).
  - A crosshair, a grey placeholder block where the weapon mesh will go (lane H6 stage 2), a flash on a frame that
    fires, and a marker on the last impact for 2 s. The marker is red with a damage number and orange when `NotYet`;
    the impact is projected through the `frame.rs` matrix.
  - It is drawn in one pass over the frame. The scene capture draws the scene only, so screenshots paint the same
    rectangles onto it.
- **Verified (automated, invented values and geometry).**
  - Four `weapon` tests: the weapon file and its refusals; refire at the interval and no banked time; the range and
    the aim; no weapon, one probe-range shot per press with `NotYet` damage.
  - Three `overlay` tests: the always-there rectangles, the flash and marker with their colours and expiry, and
    painting clipped to the image.
  - Two `play` tests: a firing script gives the same path as `walk::run` and the same 20 shots at three frame-time
    patterns, and the script text keeps the trigger; the left button and fly rules.
  - One `args` test.
  - Review r1 added three: the notification recorder and the weapon line's text; a press shorter than a step fires
    once; and, in the left-button test, a click within a frame and Escape letting go of the trigger.
  - Workspace: 1,712 passed, 0 failed, 25 ignored (main e002de5 plus this lane with review r1; 1,705, 0 failed, 25 ignored on 1075dc2 before
    it; the installed-game runs below were made on main f5cd6a4 plus this lane).
- **Verified (local, installed game, zuhu desktop, release build, every run under the machine lock and released
  afterwards; output under ignored `local/p2/h4-weapon/`).** The script `fire20.txt` (20 s, 1,200 steps) fires while
  standing, walking, jumping and turning.
  - **Headless, no weapon:** 2 shots on 2 presses, damage `NotYet`, path digest `3413832365d19721`.
  - **Headless, the Tediore pistol:** 53 shots, the same path digest, shot digest `797e931e95ce7bb6`.
  - **Window, `--no-vsync`:** exit 0, 21,101 frames, median 0.780 ms, p95 1.661 ms. The same path digest and shot
    digest as headless.
  - **A 2.55 s window run ending while firing** (`fire3.txt`): 5 shots at steps 91, 104, 117, 129 and 142. Each hit
    `Sanctuary_Outer` `InterpActor_34`'s BoxLrg 715 uu ahead of the spawn, for damage 34.24 per projectile. The end
    screenshot shows the red marker under the crosshair on that box, and the placeholder weapon.
  - So the first thing a player shoots at in Sanctuary is the box decision 52 left open.
- **Open.**
  - The player's equipped weapon, which needs a save format (D9) or a starting loadout.
  - Damage to targets, which needs pawns in the play mode.
  - Spread, pattern, accuracy, kick, pellets, ammunition, reload, burst, bullets, spin-up.
  - Semi-automatic per-press firing modes, and fire intervals under one step (1/60 s).
  - A shared weapon builder in `ow-items`: `weapon.rs`'s definition data and balance snapshot duplicate `ow-cli`'s
    `items_stats.rs` (which evaluates at level 1 unless told otherwise), a drift risk.
  - The rule behind each notified weapon property (the `ClipSize` override and any other the weapon line names).
  - The weapon mesh and the game's HUD.
  - `OfflineWorld`'s refusal of change notifications, which keeps `ow-package --item-stats --data` from evaluating a
    weapon like this one.
  - A real-game check of the damage number (a bl-sdk live read of the pistol's `WeaponDamage`).
  - `InterpActor_34`.
