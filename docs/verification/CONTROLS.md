# The player's keys (controls lane, 2026-11-06)

AI-assisted (Claude). Host work so a person at the keyboard can do the slice's basic verbs. A passing
`tools/test_controls.ps1` says the host did this; it does not say the host matches the game.

## Where the stock keys come from

In the default keyboard layout, M and L map to the stock map and logs actions, and V and G to melee and grenade. That was read from
the game's data (the keyboard input device object, through the OpenBLCMM dump, `tools/blcmm_dumps.py`), not pressed in the running game:
**UNVERIFIED** in play. The `.ini` files do not hold these keys, and a player's rebinds live elsewhere and were not read.

## Map and missions keys

Before this lane no gameplay key opened these pages (`DefaultInput.ini` had no mapping and the Walker no binding); they opened
only through the status menu's tabs.

`AOpenWillowWalker::ToggleMapPage` and `ToggleMissionsPage` (`OpenWillowControls.cpp`) run the steps `ToggleSkills` runs (the
trigger and the aim end, a legacy inventory widget gives way), then call the HUD's existing `ToggleMap` / `ToggleMissions`.
What those already refuse stays refused: nothing opens over a vending shop, and with no page served the key does nothing and
logs `OWCONTROLS map page did not open`.

Host rules, **UNVERIFIED**:

- With every page closed, the key opens its page. The script reaches the stock menu through `ShowStatusMenu` with a chosen first
  screen (`WillowPlayerController.ShowMap`), and it refuses through `CanShowModalMenu`, whose conditions were not read. So the host
  also opens these pages while Maya is down, as its inventory and skills keys already do.
- With a page holding the keyboard, a real key goes to the page's own script, not to these bindings. `map.js` closes on M and goes to
  the Missions on L; `missions.js` closes on L but has no M route; `inventory.js` and `skills.js` have neither. Not changed here: what
  M and L do on an open page in the real game is for the real-game capture to read, and the routes follow from that (`inventory.js`
  stays with the inventory lane).

## Melee

**What the game does, in our own words** (script and data; `research/script_disasm.py`, OpenBLCMM dump; not confirmed by a live
swing, so all of it is **UNVERIFIED** until the real-game read in `tools/real_game/controls_capture.ps1` runs):

- The key runs the action, which calls the controller's melee behaviour. An action skill may take the press first, and a skill may
  replace the melee (Converge-style overrides). Neither is modelled.
- `MeleeAttack` needs the weapons to be free: not restricted, and no shared weapon action already running. So a press during a swing
  does nothing. The held weapon's own check always answers yes.
- It plays the body move, plays the first-person move locally, plays the body class's melee camera anim, and starts a shared weapon
  action for the first-person clip's length. That stops any reload and the aim and holds the weapon busy until the clip ends.
- The body move carries the hit as a timed event. Its behaviour runs the melee definition.
- The definition (`BehaviorData_PlayerMeleeAttack_Siren`) sets the cone path: a straight trace forward; if it reaches nothing, the
  best-lined candidate in range with a clear line. Its numbers are in `OpenWillowMelee.h`.
- Damage is `Init_PlayerMeleeDamage`: 2.5 x (8 x 1.13^level) plus a melee buff attribute. Normal damage type, melee damage source.

**What the host does:** V starts a swing (`OpenWillowMelee.cpp`). It stops a reload, ends the aim and blocks the trigger, reload and
Phaselock and a throw until the clip's length ends, and ends a burst in progress. 0.30 s after the press the hit lands through `ApplyShotDamage`, so targets and listeners see
it as any hit, with the new source `Melee`. Only actors that take hits (`IOpenWillowHitTarget`) can be struck. A trace that strikes
a wall ends the swing there.

**Open, host stand-ins, UNVERIFIED:**

- Two body moves exist: the Siren's hits at 0.30 s and the shared one at 0.20 s. Which Maya plays is unread; the host uses the Siren's.
- The first-person clip is **not imported**: the anim set that holds it is unread. `MeleeAnim` looks in `PistolCombat`, logs the
  usual "arms animation not found" warning, and the swing then has no visible arm and lasts 1 s (`NoClipDuration`, a host choice).
- The swing's origin and direction use the camera, since the stock source is a native interface call.
- The cone's use of the angle (not halved) is the low-confidence reading in `NATIVE_AI_MELEE.md`, and the knockback factor is from
  a bullymong's radius melee.
- While down the input set still holds melee, and the host applies the trigger's weapon-ready rule. Whether a stock swing works
  there is unread.
- No critical hits, camera anim, hit sound, view shake, third-person move, melee weapon mesh, melee damage modifiers or skill
  overrides. Cloud Kill and Immolate act only on Bullet and Rocket sources, so a swing does not proc them. Other hit handling does see
  it: Life Tap applies to any hit that is neither a skill's nor a status effect's, and a melee kill reaches the generic kill
  notification. What the game does for these is unread.
- What interrupts a swing is a host rule. Going down, a status page or a shop cancels it and clears the pending hit. Switching weapons,
  holstering, aiming, reloading, firing, a grenade and Phaselock wait for it, because the script holds the weapon in a busy state.
  The capture reads a status page opened inside a stock swing.
- A swing and a grenade throw refuse each other, as both are shared weapon actions. Enemy melee belongs to the range lane.

## Checks

- `tests/controls_keys_test.py` (ctest `controls-keys`): every `BindAction` has a mapping and every mapping a binding (UE ignores
  a mismatch silently), no key runs two actions, M and L are as above. Our own config and source only.
- `tools/test_controls.ps1` (`-owcontrolstest`, in engine): presses through the player controller's input path, as the inventory
  suite does, with every page closed, because a simulated press reaches the bindings even where a real key could not. Rows:
  `start_closed`, `m_opens_map`, `map_closed`, `l_opens_missions`, `melee_damage_rule`, `melee_cone_rule`, `melee_swing_rule`,
  `v_not_hit_before_the_delay` (the delay measured from the frames the swing started and the target lost health), `v_hits_target_ahead`
  (one melee hit event, of the expected damage), `v_misses_target_behind` and `v_misses_target_out_of_reach` (the swing did start),
  `page_cancels_swing`, `v_refused_in_grenade_lockout`, `g_refused_during_swing`, `holster_refused_during_swing`, `v_stops_reload_before_refill`,
  `v_keeps_refill_after_it`, `closed_at_end`, then `OWCONTROLS SUMMARY passed=18 failed=0`. Ending a burst is not covered by a row.
  `tools/check_controls.py` grades the run's own log: the expected count, failed=0 and each expected row exactly once as PASS.
- `tests/check_controls_test.py` (ctest `controls-check`): the grader on invented logs, including a short count, a renamed row, a
  repeated row and a missing summary.
