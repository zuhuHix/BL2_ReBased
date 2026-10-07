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
  the source of the difference is **open** (see "Live weapon data" below: not an online hotfix, not a package
  override, not an installed mod). The magazine, reload and most damage mismatches above follow these type-level
  differences.
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

## Live weapon data (2026-10-03)

`tools/real_game/scripts/weapon_dump.py` reads every `WeaponPartDefinition` (887), `WeaponNamePartDefinition` (877) and
`WeaponTypeDefinition` (39 including buzzaxe, turret and vehicle types) from the running game at the main menu and
writes the properties the evaluator and the name rule read (`OVERLAY_KEYS` in `tools/weapon_card_audit.py`, plus the
external and zoom attribute effects) to ignored `local/realgame/cards/live_weapon_data.json`. `tools/real_game/live_overlay.py`
turns that file into a `weapon_recipe.Package` overlay and `golden_card_compare.py --live-data` uses it. Confirmed in game
on 2026-10-03 by reading the live objects through the SDK, game at the main menu, saving blocked, saves restored:

- **Live data agrees with OpenBLCMM's static dump** for all 78 property values of the 61 objects in W's
  `runtime_changes_filtered.json` (the only differences are `BaseValueScaleConstant` 1.0, the default, which one source
  writes and the other omits). Two independent sources, same values.
- **Against the cooked decode** 1,803 live objects compared (364 are not in `Startup.upk`): 37 have genuinely different
  values (ClipSize on 7 types, ReloadTime on 5, InstantHitDamage on 2, and 24 part effect lists, mostly a
  `MT_PreAdd` modifier that is `MT_Scale` live and the other way round); every one is in W's list. The rest of the
  differences are defaults the cooked packages leave out (for example `InstantHitDamage.BaseValueConstant` 20 and
  `ProjectilesPerShot` 1, which the live game reports).
- **Effect on the golden set** (same 69 weapons, exact parts): evaluator on cooked data 52/69 weapons match the main four
  stats (damage, fire rate, reload, magazine); with the live overlay **69/69**, and every printed stat 68/69. Per
  field: damage 65 to 69/69, reload 63 to 69/69, magazine 57 to 69/69.
- **Still open after the overlay:** sale value 65/69 (four rocket launchers, model 5 to 7 points high, about 4 %); status
  chance 15/16 (33.3 % against 33.4 %, one Pyroclastic launcher); the level line (10 cards where the game prints none:
  all mission-balance weapons and game-stage-1 weapons, which a rule on the player's level could explain; UNVERIFIED);
  the stage-15 Maliwan pistol reload printed 1.8 by the host and 1.7 by the game.
- **Where the live values come from is still not explained.** They are present 0.01 s after the SDK loads (7.8 s before
  the `Micropatch` configuration exists), `Startup.upk` is the only one of the 2,010 installed packages (DLC included)
  that defines these objects, and nothing in `sdk_mods`, the zipped mods or the config files sets them, so it is not an
  online hotfix, not a package override and not an installed mod. Remaining hypotheses: something the game or its executable applies at
  load, or a gap in the cooked decode (both UNVERIFIED). The correction to NATIVE_WEAPON_RULES section 7, which
  attributes the differences to hotfixes, is for the weapon lane's owner. `tools/real_game/openwillow_valuewatch/`
  (the load-time watcher, local use only) produced the timing.
- Note for the port: the live values are what the player's game uses, so they are the oracle for stats, but they are game
  data: the file stays under `local/` and a port reading them needs a decision on where its values come from.

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

## Level-up, Marcus's use chain and interface answers (lane L1, 2026-10-07)

AI-assisted (Claude), lane L1. Second capture session, driven with the same tool (`tools/real_game/`, one game process under
`local/ue_run.lock`, saves backed up first, the driver blocking every `WillowSaveGameManager` save call from the first frame,
memory-only changes, objects looked up again in every command). The game was started with the installed startup mod's
`-Character=Save0008.sav` argument and "Continue" was pressed, which loaded the level-8 Maya (Plan B) in Sanctuary. The driver gained
`Install-Driver -BlockSavesAtStart` for this (a flag file makes the driver block saves at import, before any command can run).
New command scripts: `scripts/levelup_probe.py` (hooks, attribute snapshots) and `scripts/marcus_use_probe.py` (hooks, state reads).
Raw rows and screenshots stay under ignored `local/realgame/L1/`. Hooks were Python hooks on script-visible functions: a call that
the game makes natively from C++ (not through script) is not seen by them, which limits what an order of events can show.

### Level-up (NATIVE_PROGRESSION section 3 and 4, SANCTUARY_RPG_MISSION "Not modelled")

Three level-ups in one session, 8 to 9, 9 to 10 and 10 to 11, each by `WillowPlayerController.ExpEarn(shortfall, plot-mission source)`
with the health pool set to 50 %, 25 % and 30 % and the shield to 50 %, 10 % and 30 % beforehand. `ExpEarn` raised the experience
pool by exactly the amount and did not level up inside the call; the level changed on the next frame (every level-up call below ran in
that one frame, at one game time).

- **Health: confirmed full refill.** Current health went from 214.5 of 429.1 to 484.8 of 484.8 (8 to 9), from 121.2 of 484.8 to
  547.9 of 547.9 (9 to 10), from 164.4 of 547.9 to 619.1 of 619.1 (10 to 11). It is neither the same fraction nor the same absolute
  value. The refill happens inside `OnExpLevelChange`, after `RecalculateAttributeInitializedState` returned and before
  `ClientOnExpLevelChange` runs. It is the class's `OnLevelUp` behavior, whose stock skill definition adds `HealthMaxValue` to
  `HealthCurrentValue` (read from the live object, post-add modifier): current health counts as a gain (the controller's
  health-gained accumulator took the fractional part of the refill, 0.663 of 426.663) and the pool's impulse counter rose by one.
  Whether the effect is "add the maximum, then cap" or "set to the maximum" cannot be told apart from the end state.
- **Maximum: confirmed written by `RecalculateAttributeInitializedState`.** Before the call the pool maximum was still the old one,
  after it the new one; the call leaves current health alone. The pool's base maximum was 212.6755, 240.3233, 271.5654 (levels 8,
  9, 10), `80 x 1.13^L` to float precision; the effective maximum is that base times 2.0176 on this character (the profile's
  Badass Rank modifier), as before. The note's stand-in (re-evaluate the base maximum for the new level) gives the right value;
  that it is this native that recomputes it is now observed. What else the native does is not shown by this run.
- **Shield: the level-up does not set current or maximum shield** (maximum 180.589 and base 0 unchanged, current unchanged through
  the level-up frame). It does start the shield recharge at once: the recharge rate attribute is raised by half the maximum shield per
  second (90.295 per second) for 4.0 s (observed 4.008 s), then back to 0. Without a level-up (control, also with a 10 XP
  `ExpEarn`) the same shield waits 1.61 s and then recharges at 25.9 per second. The rate and the 4.0 s duration are in the stock
  `PlayerBehavior_LevelUp` skill definition; the shield reached full in about 1.9 s.
- **Skill points: confirmed +1 per level** at 8 to 9, 9 to 10 and 10 to 11 (unspent 0 to 1 to 2, `LevelUpCount` up by one each time).
  The point is added at the start of `ExpLevelUp`, before `OnExpLevelChange`. The "0 below level 5" half was not observed.
- **Other attributes: nothing else changed.** A before/after snapshot of every numeric attribute and its base value on the
  controller, pawn, replication info, health, shield and experience pools (about 1,640 values, run on two level-ups) changed only in
  these places: experience pool and `ExpPointsNextLevelAt` (28,126 to 37,798 to 49,377), health pool base and maximum, current health,
  the pawn's game stage (8 to 9 to 10, set inside `OnExpLevelChange` after `RecalculateAttributeInitializedState`), level, skill
  points, level-up counters and timestamps, the shield recharge rate above. Outside that list: a natural level-up also applies the
  stock `PlayerBehavior_LevelUpNaturally` skill, a weapon-damage modifier of the scale kind that **raised the equipped weapon's
  damage from 41.40 to 61.92 for 30 s** (the scale sum went from 2.017 to 3.017, i.e. +1.0 added to the existing sum) and was gone
  at 30.3 s. The action-skill cooldown part of `PlayerBehavior_LevelUp` was **not shown to do anything**: a cooldown pool set to
  its maximum drained at the same speed with and without a level-up (inconclusive, the set-up was artificial).
- Incidental: `GetExperienceReward` for the Fire mission still answers 395 at level 11 (it was 395 at level 8), and Marcus's offer
  screen printed "Level 8, 395 XP" for this level-11 character.

### Marcus's use chain (NATIVE_MARCUS_USE_CHAIN, NATIVE_BEHAVIOR_CONTEXT, NATIVE_DIALOG_GROUPS)

The player was put 110 units in front of Marcus (`Sanctuary_Dynamic` `WillowAIPawn_13`), made his current usable object, and the real
use key was pressed once. The character's mission state: two active missions (the Sanctuary welcome mission at its fuel cell objective
and "Handsome Jack Here") and five complete; none of the six missions whose sequences Marcus's chain tests, and not the Fire mission.

- **The nine checks: confirmed in order and all answered "not enabled".** `BehaviorKernel.IsBehaviorSequenceEnabled` was called nine
  times, in the note's order (Ep4_SpeakToMarcusAboutBank, Ep4_GetMarcusCrystal, Ep14_Rescued, M_TheBane, M_BearerBadNews,
  M_ClaptrapBirthdayBash, M_OutOfBody, M_SafeAndSound_BringPictures, Ep17_TalkToMarcus), each with consumer handle 49 and Marcus's
  own AI provider, each answering false. The three names that have no sequence in the provider answered false as well. After the
  ninth the chain went to `Behavior_PlayAIMissionContextDialog`, `Behavior_HasMissions` and `Behavior_ShowMissionInterface`, in
  that order; nothing else of the cascade ran. What the note predicts for this state (none of the six missions started) is exactly
  this; a Fire-only save gives the same cascade.
- **The native's result rule: confirmed by direct calls.** Of Marcus's 13 provider sequences exactly `AI`, `Brain` and `Patrol` read
  as enabled; the other ten, an unknown name, a `None` provider and consumer handles -1 and 0 all read false.
- **Use event: confirmed.** `AIClassDefinition.OnUsed` ran with link filter 2 (generic) first and filter 0 right after the interface
  call; the second raise reached none of the checks.
- **The on-use tag: `VO_NPC_OnUse_MissionsAvailable`.** `PlayOnUseDialog(player pawn)` triggered it on Marcus's dialog component
  with the player as the other object. The mission counts for this player (`CountMyMissionsByState`) were 1 eligible, 0 in progress,
  0 redeemable, so the note's rule (redeemable, else eligible, else in progress, else none) predicts it. The screen shown by
  `Behavior_ShowMissionInterface` was the offer for the Fire mission ("Rock, Paper, Genocide: Fire Weapons!", level 8, 395 XP,
  optional); it was declined, not accepted. After it closed the component was asked for `VO_NPC_PlayerLingeringInMenu` and, on the
  decline, `VO_NPC_GenericDismissal`.
- **Which group answers.** A read-only `GetMatchingEvent` call on Marcus's dialog component with the player's name tag answered
  `DialogGroup_NPC` (the generic group, second in his list) for all four `VO_NPC_OnUse_*` tags and `DialogGroup_NPC_Marcus` for
  `DET_NPC_OnUse_MissionsAvailable`, as the note says. The event object it returned was `GearboxDialogEvent_0` for all four `VO_` tags,
  which looks like an artefact of the out parameter, so only the group is claimed. The second, native step (the Trigger act
  re-firing the `DET_` tag) is not visible to script hooks and was **not observed**; the `TriggerEvent` post hook did not report.
- **Dialog group list: confirmed size and order.** `GetDialogGroups` on Marcus returned 127 entries: his own group first,
  `DialogGroup_NPC` second, the default template group last; his body class has 1 group, `bNPCDialog` is true, the globals have 125
  NPC groups, his name tag has no DLC expansion; `DialogGroups_Side_ThisJustIn` appears twice (positions 105 and 119 of the 127, which
  are 104 and 118 of the globals' list). His consumer handle is 49 (the player's is 2).

### Interface answers (NATIVE_CLASS_SERIAL_LAYOUT) and `Skill.UpdateGrade` (NATIVE_BYTECODE_OPCODES)

- **Interface table versus stand-in: confirmed on all disputed pairs.** Class-level interface answers read from the running game
  through the SDK (the engine's own `ImplementsInterface` on the loaded class) agree with the real interface table on **all 66**
  pairs where the stand-in said yes and the table no (59 IGFxMenuScreenTickable, 3 IInstanceData, one each IResourcePoolProvider,
  IStorageDevice, ISkillTreeListener, OnlineAccountInterface; so WillowPlayerController is not an IInstanceData nor an
  IGFxMenuScreenTickable) and on **all 203** pairs where the table said yes and the stand-in no because the interface declares no
  function (for example IConstructObject, IAttributeEffectBehavior). The other 772 of the 975 "table yes" pairs have the interface
  `Core.Interface`, for which the engine answers false for every class: consistent with the note's "excluding Core.Interface". 15 more
  hand-picked pairs, including the slice's IUsable on WillowAIPawn and IMission on WillowWaypoint, agree with the table (two
  more pairs, IConstructObject and IMissionDirector on WillowPlayerController, were my own guesses for "yes", not table entries; the
  game says no, as the full sweep implies). Caveat: this is the class check, not a run of the script cast opcode on an object. (The note says 771 pairs are `Core.Interface`; the list holds
  772.)
- **Let-attribute rule, partly confirmed.** `Skill.UpdateGrade(N)` on a live skill set both `Grade` and `GradeBaseValue` to N (N = 2,
  5, 3: base and value together, empty modifier stack) and set `bForceRefreshModifiersNextTick`. **Correction:** the stored grade is
  `max(N, 1)` (0, -3 and 1 all give 1), not "N plus one"; the script's second operand is the constant 1 of a native two-argument
  call, which reads as a maximum. Not shown: "no change notification" (a native virtual, invisible to script hooks) and the recompute
  from a non-empty modifier stack.

### Save safety (this session)

`Backup-Saves` copied `SaveData` and `Config` first; 33 files were hashed before and after. With saves blocked from the first
frame, **all 33 files are byte-identical after the session** (no restore was needed, profile.bin included), the startup mod's
settings folder is unchanged, and the driver was removed. The game was closed through its window. The run lock changed hands
before this lane released it (see the lane's handoff).
