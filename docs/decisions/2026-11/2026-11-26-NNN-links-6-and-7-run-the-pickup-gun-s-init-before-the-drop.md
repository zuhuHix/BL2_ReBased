## 2026-11-26: Links 6 and 7 run the pickup gun's init before the drop

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-LINK67-PICKUP-GUN-INIT (queue 0ab), on main 457c46e4. Clean room:
implemented from docs/verification/NATIVE_LINK67_PICKUP_GUN_INIT.md alone (#756 with its r1 fix, decision 2026-11-26-15);
this lane did not read the executable. No session was run. Every rule is UNVERIFIED in the running game; this is
harness work, with no new native rule.

- **What was decided** (the note's section 3 and the hub's ruling in it):
  - **the world:** `pickup_world` is `equip_world` plus the pickup set-up:
    - `equip_world` brings the card's class set-up at build, `ow_skills::action_skill::Pack`, the `WorldInfo`, and
      the world's one `WillowGlobals` holding the installed `GD_Globals.General.Globals`;
    - the pickup set-up uses those globals (checking their declared schemas, as before) instead of making its own;
    - then it adds the weapon class default, the empty registry and scene layers, the particle preload and the game
      info, as before;
  - **the preflight:** `run_pickup` runs `card::preflight` before `make`, as link 8's driver does. Its refusal is
    unprefixed, since nothing has been written;
  - **the routes:** `WeaponRoute::manufacturer_inputs()` is true for `Pickup` and `PickupDecoy` as well, so links 6
    and 7's guns (the decoy included) get the definition data's manufacturer inputs, as link 8's does;
  - **the gun's init** (`equip_run::pickup_gun_init`), on each gun after it is made and before its drop, in link 8's
    order:
    1. the five preloads (muzzle flashes, gestalt definition, gestalt skeletal mesh, the Material part's material,
       the material classes);
    2. the skeletal mesh layer, installed once per world;
    3. the threshold preload;
    4. the installed `WillowWeapon.InitializeInternal(false)`.

    Each preload is a logged UNVERIFIED stand-in, and `run_pickup` keeps `make`'s harness lines in the row.
  - **the prefix:** the init runs inside `make`'s mark, so its refusal carries "stopped after partial writes: ".
    Link 8 runs the threshold preload after its own mark, so the same refusal is unprefixed there, as the note says.
  - The decoy's init runs with no owner, which link 8 never does: not exercised before, UNVERIFIED.
- **Reference:** unchanged. The pickup links are Off-only slice-gate drivers, and the manufacturer inputs stay Off-only
  (`weapon_of` sets them in Off only; the route test checks Reference sets none).
- **The slice gate** (installed, Off, strict, under the machine lock; base 457c46e4 and this change): 6 ok / 8 NotYet,
  47.0% on both.
  - Links 6 and 7 move in `touch`: from "Actor.GetBoundingCylinder: the component's template (option B) is not read"
    to "Actor.GetBoundingCylinder: the component's world transform" (`NotRead`).
  - That is the note's section 5, item 5 (the whole recipe): the copied component now holds the shared gestalt mesh,
    the mesh layer answers its stored bounds, and its `Scale` 0.92 is a transform the bounding cylinder does not
    answer.
  - Link 8's row and every other row are unchanged.
- **Tests:** the note's tests 1 to 5:
  - the init runs after each gun is made and before its drop;
  - its refusal is prefixed and nothing is dropped;
  - the decoy's gun gets the same init. Its place after the first drop is `build`'s order; the toy world spawns
    nothing, so no drop is run there;
  - the mesh layer is installed once per world;
  - the copied component's source f: the `LayerAbsent` with no layer, `WORLD_TRANSFORM` at `Scale` 0.92, the box at
    `Scale` 1.

  The two route tests now pin the new rule: every route sets the inputs in Off.
- **The hub's call** (the note's open question 3): whether `skeletal_bounds` should answer a scaled world transform,
  the next stop of links 6 and 7.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game.
- **Sensitive files:** none.
