# Real-game ground truth: first capture session (2026-10-02)

AI-assisted. Observations of the running original game (Borderlands 2, Steam, 32-bit DX9, `-windowed` 1280x720,
one PC), driven by `tools/real_game/` (docs/TOOLING.md "Driving the real game"). Everything the session recorded
(screenshots, burst frames, traces, card records, save backups) stays under ignored `local/realgame/`; this file holds
only rules, counts and pass/fail. Where a rule below says **confirmed in game**, it names how. Anything not listed
here keeps its earlier label.

## Method and safety

- One game process at a time under `local/ue_run.lock` (the tool's `Enter-RunLock`). Steam logged in as the save
  owner. The maintainer allowed unattended launches; the machine was idle (12 min without input) at the start.
- `Backup-Saves` copied `SaveData` and `Config` first. `block_saves()` (driver) refuses script calls to
  `WillowSaveGameManager:SaveGame/Save/SaveRawData/SaveGraveyard`; with it working, no save was written after
  10:09. Before that, one load-time write of the loaded character went through because the first hook version
  raised an exception before returning `Block`; game start and character selection also wrote `profile.bin` and a
  `.bak`. After the session those four files were restored from the backup; **all 22 save files are byte-identical
  to the pre-session copies**. The driver mod was removed after the session.
- Spawned weapons lived in memory only (backpack and weapon-slot limits raised in memory while saving was blocked)
  and were removed before any travel. One game crash (`pyunrealsdk` fatal error) came from a command reusing a weapon
  reference after an inventory swap had invalidated it; the driver now documents "look objects up again in every
  command". Nothing was written by the crash.
- Character used for slice checks: a level-8 Maya at plot mission Plan B in Sanctuary (skills: Phaselock, Mind's Eye
  3); Phaselock tests in Three Horns Divide (`Ice_P`) against bullymongs, the player made immune to damage by a hook
  (it changes nothing else). Saves of level 2, 17 and 70 Mayas were loaded read-only for progression values.

## Progression ([NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md))

- **Level curve: confirmed in game** by calling `WillowPlayerController.GetExpPointsRequiredForLevel(L)` for
  L = 1..80 (DLC cap 80 in this install): `max(0, trunc(60 × (L^2.8 + 7.33)) − 499)` evaluated in **single
  precision** (float pow, add, multiply, subtract 499, truncate) reproduces **all levels 1..59** exactly (so all of
  the host's 1..50); double precision is one point high at 17, 22, 33, 42, 45, 47, 49 and most levels above 50. Four
  levels above the base cap (60, 68, 74, 79) are one point off the float form as well (likely the C runtime's float
  pow; not needed for the slice).
- **"Next level at": confirmed** (`ExpPointsNextLevelAt`): 1,241 at level 2, 28,126 at level 8, 196,238 at level 17,
  9,155,282 at level 70, each the curve's value for L + 1.
- **Skill points: confirmed** `max(0, L − 4)` as spent + unspent: 0 at level 2, 4 at level 8 (Phaselock 1, Mind's
  Eye 3), 13 at level 17 (12 + 1), 66 at level 70.
- **Max health base: confirmed** by reading the health pool's `MaxValueBaseValue`: 102.152 (L2), 212.6755 (L8),
  638.886 (L17), 415,509.44 (L70, UVHM) = `80 × 1.13^L` to float precision. The HUD shows more (429 at L8) because the
  profile's Badass Rank skill (`GD_Challenges.BadassSkill`) adds a modifier: the HUD number is not the base.
- **Fire mission XP: confirmed** by calling `MissionDefinition.GetExperienceReward` on
  `M_RockPaperGenocide_Fire` for the level-8 Maya: **395** (mission game stage 8, `XPReward_02_Small`); the Shock,
  Corrosive and Amp variants give the same. This is the truncated value the note predicted (the host gave 396 before
  2026-10-02). Not observed: a real turn-in, the stage at levels 7 and 10.

## Phaselock ([NATIVE_PHASELOCK_TARGETING.md](NATIVE_PHASELOCK_TARGETING.md), [PHASELOCK_STOCK_DATA.md](PHASELOCK_STOCK_DATA.md))

Per-frame SDK samples of `LiftActionSkill` (state, `SkillStartTime`, `LiftStartTime`, lift locations,
`PrevBobLocation`, the lifted pawn's location) on the same QPC clock as burst screenshots; 12 key presses, 8 lifts.

- **Bob: confirmed in game exactly.** Simulating the note's rule (target `LiftEndLocation.Z + 30 × sin((now −
  SkillStartTime) × 0.5 × π)`, smoothed by `VInterpTo(prev, target, dt, 1.0)`, seeded at the end of the lift, using
  the game's own frame times) reproduces the lifted pawn's height over 435 frames with **RMS error 0.001 units**
  (worst 0.0025). Observed: peaks +15.6/+16.2, trough −16.3, peaks 4.0 s apart, first peak 1.63 s after the cast
  (0.63 s behind the unsmoothed sine; the unsmoothed sine is 17.7 units RMS off). The bob continues through the
  release state.
- **Timing at level 8 (no duration skills):** skill duration 5.7 s = lift (state 1) 0.7 s + hold (state 3) 3.9 s +
  release (state 2) 1.1 s. The lift raised the target 163 units (`LiftEndLocation`) along an ease-in/out curve
  (fraction 0.04 / 0.30 / 0.66 / 0.91 at 15 / 39 / 59 / 79 % of the lift time); the exact curve is
  `GetLiftLocation`'s (script, with `LiftSnapTimePct` / `LiftSnapHeightPct`), not fitted here.
- **Cast during a reload: confirmed it casts.** With a manual reload running (`OnAbortReload` and `StopReloading`
  marked right after `StartActionSkill`), the cast started and lifted the target; **the cast aborts the reload**
  (new detail). An earlier attempt with the target off the crosshair played the cast animation and fizzled without
  using the cooldown (a targeting result, not a reload refusal).
- **Cast during a weapon swap put-down: confirmed refused**, 2 of 2 (`PutDownWeapon` marked, `IsPuttingDown()` true at
  the key press, the skill stayed idle, no cooldown used); a control cast at the same spot right after each started
  normally.
- **Target choice:** with nothing near the crosshair the first cast took a bullymong about 98 m away (screen-space
  choice, consistent with the note's magnetism score; the radius thresholds were not measured).
- **Not tested:** going down while a target is held (Maya was immune), casting while injured.

**Presentation (burst frames, times after the key press; the skill starts about 25-50 ms after it):**

| time | real game |
|---|---|
| 0.25 s | arm raised, no orb yet |
| 0.30-0.40 s | full-screen dark blue-black radial vignette, clear centre; white/blue swirl lines around the hand |
| ~0.45-0.49 s | **hand orb appears**: a saturated blue sphere in the palm with blue arcs; tattoo stripes bright cyan-blue |
| ~0.58 s | orb turns into a swirl with a violet/green core; brief brightening of the screen |
| 0.75-1.0 s | orb thrown; a large blue-white burst with vertical streaks at the target; strong blue screen tint |
| 1.5-4.5 s | bubble: a **dark violet sphere with a near-black core** and blue-violet wisps (not a bright core) |
| ~4.7-5.2 s | release: the bubble turns into a bright cyan-white ring and shrinks |
| ~5.3-5.5 s | a faint wider violet halo, then the target drops |

Compared with the host notes (DECISIONS 2026-10-02 "Phaselock stock presentation"): the host's hand orb at +0.25 s is
early (real ≈ +0.45 s from the key), the host's white-pink core is wrong (real core is dark), and the cast vignette,
the target burst and the cyan release ring have no host equivalent yet. Frames: `local/realgame/phaselock/`.

## Inventory open time ([INVENTORY_MOVIE_PROTOTYPE.md](INVENTORY_MOVIE_PROTOTYPE.md))

Key press to the inventory page in burst captures (quarter scale, about 20 ms per frame): **first open in the
process ~156 ms** to the first page frame and 272 ms to 90 % of the opening animation (the page was covered by the
game's one-time "Unlocked!" notices on that open); **repeat opens 126-150 ms** (3 opens) and 236-258 ms to 90 %. The
capture adds its own latency (screen copy of a composed frame), so these are upper bounds for the game.

## Weapon cards (golden set for the weapon lane)

`tools/real_game/scripts/weapon_cards.py` recorded 69 weapons (13 from the save's inventory; 6 built from the host
slice guns' exact parts; 33 one-per type × manufacturer at stage 8; 6 Maliwan/Jakobs pistol rarities; 8 level-sweep
pistols at 2/15/30/50; 2 Fire mission-balance rolls; 1 extra), each with balance, manufacturer, grade, game stage,
every part by slot, material, prefix and title parts, the game's own names and sale value, plus the card exactly as
the game filled it (`ItemCardGFxObject` calls) and a screenshot. `tools/real_game/golden_cards.py` joins them;
`tools/real_game/golden_card_compare.py` evaluates `tools/weapon_stats.py` on the **exact** parts (first on `3cc5449`, the evaluator before the weapon lane's change; re-run below).

- Exact-parts comparison (as printed): damage 64/69, projectiles 69/69, accuracy 69/69, fire rate 67/69, reload
  64/69, magazine 57/69, element dps 16/16, status chance 15/16, sale value 65/69, name from the parts 69/69; 53/69
  match every printed stat. By stratum: level sweep 8/8 and rarities 6/6 on every stat.
- The existing inferred-parts audit (`weapon_card_audit.py`) reproduces 44/59 cards on the main four stats and 42/59
  on every printed stat (10 cards without a level line excluded; the tool fails on them).
- Damage on the card is **rounded up** (49.13 → 50, 22.26 → 23, 36.11 → 37 on slice guns); consistent over 64 cards.
- **Runtime type values differ from the cooked `Startup.upk` decode:** reading the live objects gave
  `WeaponType_Bandit_Pistol.ClipSize` 36 (host decode 30), `WeaponType_Dahl_Pistol` 16 (12), `WT_Bandit_Shotgun`
  10 (9) and its `ReloadTime` 4.1 (4.4). None of the 23 live `Micropatch` (hotfix) entries touches a weapon type, so
  the source of the difference is **open** (another package overriding, or a decode gap). The magazine, reload and
  most damage mismatches above follow these type-level differences.
- Fire rate: 1.25 printed as 1.3; the host's 1.2499999 (double) prints 1.2. Single-precision evaluation fixes it.
- Name: when no part title applies the game uses the weapon type's title (the host's title-less slice SMG
  "Inspiring" is "Inspiring Projectile Convergence" in the game).
- Level line: absent on all mission-balance weapons and on every game-stage-1 weapon.
- Launchers: sale value about 4 % high in the host for Maliwan/Torgue/Tediore/Vladof (the launcher price was never
  checked).

Re-run on the weapon lane's commit `bb2a444` (single-precision stack, display rounding from presentation data):
damage 65/69, fire rate 69/69, reload 63/69, magazine 57/69, every printed stat 52/69. The one new miss is a stage-15
Maliwan pistol's reload: exactly 1.75 in the new evaluation, printed 1.8 by the host and 1.7 by the game (while the
game prints a 1.25 fire rate as 1.3), so the game's operation order matters at the half. The weapon lane (W) owns the
evaluator; these are inputs for it, not changes to it.

## Gun skins

Real inspect-view and first-person captures of the slice Maliwan fire pistol (exact parts) and the slice Jakobs
pistol (exact parts) plus a save Jakobs revolver. An independent critic agent compared them with the host stills
(an agent's judgement, not a measurement of parity): **Maliwan 4.5/10, Jakobs 3/10** (inspect vs host thumbnail),
**4/10 and 4/10** first person; the earlier 6.5 and 5.0 were scored against wiki screenshots. Region medians (sRGB,
real inspect vs host thumbnail): Maliwan white body (241,239,228) vs (141,136,116), Maliwan orange (236,128,0) hue 32
vs (166,32,0) hue 11, Jakobs wood (142,130,108) vs (62,54,42), Jakobs frame (210,209,207) vs (72,76,88). Its main
findings: the host is far too dark even in unlit albedo (a colour-space hypothesis), the Maliwan orange hue is wrong
regardless of lighting, the Jakobs frame is near-white with sparse rust in the game, inserts are light grey mesh on the
Maliwan, and the game shows element glow (red barrel slot and windows) the host lacks. The real first-person shots
were taken under the HQ's cool fluorescent light, which itself gives metal a cyan cast. Report:
`local/realgame/skins/critic_report.md`.
