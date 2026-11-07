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
  screen (`WillowPlayerController.ShowMap`), and it refuses through `CanShowModalMenu`, whose conditions were not read.
- While Maya is down M and L refuse. In the real game neither opened anything (twice each), and the set of actions allowed while down holds no menu. A page
  already open when she went down cannot route to the Map or Missions either, the same rule (a host choice: a stock page over going down was not read). The host's
  inventory and skills keys still open while down; those belong to the inventory lane.
- With a page holding the keyboard, a real key goes to the page's own script, not to these bindings. In the real game M stayed on the Map (twice) and went to the Map
  from the log (twice) and from Skills (once); L went to the log from the Map, the Inventory and Skills (once each) and stayed on the log (once). So `map.js`, `missions.js`
  and `skills.js` follow that. `inventory.js` has no L route yet, which belongs to the inventory lane; M on the Inventory was not read, as a dialog took the key.

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

**What the real-game reads say** (`tools/real_game/controls_capture.ps1`, two sessions, 2026-11-07; data under ignored `local/`; counts are over both):

- V reaches the melee behaviour (15 swings). The hit ran 0.29 to 0.30 s after the press, on all 15, through the Siren's definition. A press about 0.4 s into a
  swing started nothing and was not queued (twice), and one just after a swing ended started another (twice). A swing ran about 6 s after Maya went down (twice).
- A swing holds the weapons for 0.70 s, on all 15, which is the length of the `Melee` clip in the Siren's first-person anim set.
- A status menu opened about 0.18 s into a swing did not cancel it, and the hit behaviour still ran at 0.30 s (twice). The game clock then nearly stopped under the
  menu, and the swing ended once it had closed.
- A swing at a cardboard box from 120 uu hit it once: 53.17 damage at level 8, which is the formula above with no buff; melee damage source, the definition as the
  causer, and a horizontal momentum of 10,000 away from Maya, which is the knockback factor (one hit). A swing at a friendly pawn from the same distance made no
  damage call on it (one swing). Reach, the cone and the trace's order against a target were not tested.
- Not read: whether a weapon key is refused in a swing.

**What the host does:** V starts a swing (`OpenWillowMelee.cpp`). It stops a reload, ends the aim and blocks the trigger, reload and
Phaselock and a throw until the clip's length ends (they are refused, not queued), and ends a burst in progress. 0.30 s after the press the hit lands through `ApplyShotDamage`, so targets and listeners see
it as any hit, with the new source `Melee`. Only actors that take hits (`IOpenWillowHitTarget`) can be struck. A trace that strikes
a wall ends the swing there.

**Open, host stand-ins, UNVERIFIED:**

- The first-person clip is **not imported** yet: `tools/import_maya_combat_anims.ps1` now names it, and running that (an editor run) is the step left.
  Until then `MeleeAnim` logs the usual "arms animation not found" warning and the swing has no visible arm, for the 0.70 s the game holds the weapons.
- The swing's origin and direction use the camera, since the stock source is a native interface call.
- The cone's use of the angle (not halved) is the low-confidence reading in `NATIVE_AI_MELEE.md`.
- While down the host applies the trigger's weapon-ready rule. A stock swing does work down (twice, about 6 s after going down); how soon is unread.
- No critical hits, camera anim, hit sound, view shake, third-person move, melee weapon mesh, melee damage modifiers or skill
  overrides. Cloud Kill and Immolate act only on Bullet and Rocket sources, so a swing does not proc them. Other hit handling does see
  it: Life Tap applies to any hit that is neither a skill's nor a status effect's, and a melee kill reaches the generic kill
  notification. What the game does for these is unread.
- What interrupts a swing: going down cancels it and clears the pending hit (host rule). A status page does not, per the one read above. The game
  pauses under its menu and the host does not, so a hit due after the page opened lands under it here: **UNVERIFIED** against the game. A shop is
  treated the same, unread. Switching weapons,
  holstering, aiming, reloading, firing, a grenade and Phaselock are refused during it, not queued: a press in the swing is dropped,
  and only a trigger still held fires afterwards. The script does hold the weapon in a busy state, but what the game does with these
  presses is **UNVERIFIED**.
- A swing and a grenade throw refuse each other, as both are shared weapon actions. Enemy melee belongs to the range lane.

## Use prompt

**Confirmed from the code first, not seen in play:** nothing fed the use-target choice to the HUD. A vending machine had an always-on
world label with its title and no key hint, a loose item a label with its amount, and a door nothing. The only key hint was a debug line
for Marcus. The movie's prompt clip `p1.tools` was not driven.

**What the host does:** `OpenWillowUsePrompt::StateField` reads `NearestUseTarget` (and whether Marcus would take the press, as he
outranks it) and sends the `usePrompt` field; the page moves `p1.tools` to that frame and fills its `desc` field. It only reads: what the
use key does is unchanged. Nothing shows when a press would do nothing (`AOpenWillowWalker::CanUseNow`, the key's own check: Maya not
active, down, or under a menu) or while the reward page is up. The placement is the movie's own,
since the host only drives the clip.

**The prompt and the key agree by construction, with one exception.** The prompt asks `NearestUseTarget`, which applies the same order as
`PickupNearby`: Marcus, then an aimed pickup, then the nearest target. The exception: when the named target refuses a press (a full ammo
pool, say) the press passes to the next target, which the prompt did not name.

**Which target should win stays open.** `NATIVE_USE_INTERACTION.md` (read from native code) says the game casts one
ray along the view and takes the first usable thing it hits, with no distance rule and no priority between kinds, and that a current
pickupable wins the key. The host's nearest-target rule is a stand-in that also reaches things Maya is not looking at. This work does not
change it. The real-game read supports the note: standing 120 uu from a usable thing and turning 15 to 30 degrees away, the usable went to none on 8 of 11 targets,
with the thing still in reach and nothing else taking over. The other 3 stayed usable, and why was not checked (the ray may still have hit them). So the game appears to follow the view; the host's rule also reaches things Maya is not
looking at. Aiming at a pickup with an interactive in reach was not read, since Sanctuary has no loose pickup to stand at: **UNVERIFIED**.

**The stock prompt, read live** (the clip, its frame and its text, with screenshots; one target per kind): `[E] SHOP` on a vending machine (frame `shop`), `[E] TALK` on a pawn
(`talk`), `[E] USE` on the character customization machine and the bounty board (`use`), `[E] OPEN` on a golden chest (`open`), with its key cost after it in a red `(1)`. So the
host's frames and words hold for those. Others have their own: a radio's `[E] CHANGE STATION` with a second prompt `[Q] TURN OFF` on `toolsSecondary` below it, a mission object's
own words on `objective`. Not read: a door, a strongbox (no usable from the spot tried), a loose pickup or gun. Costs, the secondary prompt and per-object words are not driven.

**The reward page** takes the press before anything else (`UOpenWillowQuest::TryUse`), so no world prompt shows over it. It should carry its
own wording, the stock reward screen's confirmation in the movie, not `p1.tools`: **UNVERIFIED**, and not built here. In the host the use key
stands in for the movie's Enter.

**Read from data:** the clip's frame labels and `desc` field (the HUD movie), and the words of the default icons (`GD_InteractionIcons`); the
words are short constants in `OpenWillowUsePrompt.cpp`, to change if the real-game read shows others. Which icon goes with which target is a
host choice.

**Writing the text:** `desc` is the field's instance name and also its bound variable. In the Ruffle bench a dot path to it read nothing, and
writing through the path only took the first time, so a later prompt kept the first one's words. The page now writes and reads the variable on
the clip, and a prompt counts as applied only when the label's frame and exactly the asked text read back. Ruffle's behaviour here, not
Scaleform's.

**Open, UNVERIFIED until the real-game read:** which icon the game gives each target (a host choice per target), how the stock prompt
writes the key (`[E]` is a host choice), whether a lootable says something other than open, costs on the prompt, and the secondary prompt.

## Checks

- `tests/controls_keys_test.py` (ctest `controls-keys`): every `BindAction` has a mapping and every mapping a binding (UE ignores
  a mismatch silently), no key runs two actions, M and L are as above. Our own config and source only.
- `tools/test_controls.ps1` (`-owcontrolstest`, in engine): presses through the player controller's input path, as the inventory
  suite does, with every page closed, because a simulated press reaches the bindings even where a real key could not. Rows:
  `start_closed`, `m_opens_map`, `map_closed`, `l_opens_missions`, `melee_damage_rule`, `melee_cone_rule`, `melee_swing_rule`, `use_prompt_rule`,
  `v_not_hit_before_the_delay` (the delay measured from the frames the swing started and the target lost health), `v_hits_target_ahead`
  (one melee hit event, of the expected damage), `v_misses_target_behind` and `v_misses_target_out_of_reach` (the swing did start),
  `page_does_not_cancel_swing`, `v_refused_in_grenade_lockout`, `g_refused_during_swing`, `holster_refused_during_swing`, `v_stops_reload_before_refill`,
  `v_keeps_refill_after_it`, `burst_ends_on_swing`, `prompt_nothing_in_open_air`, `prompt_marcus_over_world_target`, `prompt_aimed_pickup`,
  `e_takes_the_named_pickup`, `down_cancels_swing`, `prompt_nothing_while_down`, `m_refused_while_down`, `l_refused_while_down`, `route_refused_while_down`, `closed_at_end`, then `OWCONTROLS SUMMARY passed=29 failed=0`.
  Refusal rows count the refusal itself, so a press that never arrived fails them; the reload rows require the reload to have started.
  A shop during a swing has no row.
  `tools/check_controls.py` grades the run's own log: the expected count, failed=0 and each expected row exactly once as PASS.
- `tests/check_controls_test.py` (ctest `controls-check`): the grader on invented logs, including a short count, a renamed row, a
  repeated row and a missing summary.
- `tests/hud_fields_test.js`: the use prompt's calls on the movie.
- `tests/hud_use_prompt_page_test.js`: `index.html`'s real script with a stub player: a prompt reaches the clip, and a throwing clip does not stop
  the rest of the HUD. No reward page is reached in the suite: only `use_prompt_rule` covers it.
