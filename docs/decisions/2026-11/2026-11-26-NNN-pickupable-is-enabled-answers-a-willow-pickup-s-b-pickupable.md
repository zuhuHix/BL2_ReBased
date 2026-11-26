## 2026-11-26: Pickupable_IsEnabled answers a WillowPickup's bPickupable

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-PICKUPABLE-ISENABLED, stacked on fix/scaled-bounds-transform (943f8f1a,
itself on #761). Clean room: implemented from docs/verification/NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 5 alone
(analyst lane F read the native); this lane did not read the executable. No session was run. Every rule is UNVERIFIED in
the running game.

- **What was decided** (section 5's Off answer), in `ow-inventory`'s `pickup` module:
  - Two keys, `DroppedPickup.Pickupable_IsEnabled()` and `WillowPickup.Pickupable_IsEnabled()`, are bound in Off only,
    to one rule. Both are declared natives in the installed packages (`--disasm` finds no script). No pack bound either
    key before: a grep of `crates/` found none.
  - A receiver of class `WillowGame.WillowPickup` or a subclass answers its `bPickupable`. The class test goes through
    the class table by path, so no code is loaded.
  - The field is read **by name** through the class layout, never by offset. The note places it after `RBState`
    under a fitted 16-byte alignment (UNVERIFIED), and a name read does not depend on that fit.
  - A class with no `bPickupable`, or one whose `bPickupable` is not a bool, is a data error (`VmError::Script`).
  - Any other receiver (a `DroppedPickup` that is not a `WillowPickup` among them) is `NotYet { NotRead }`:
    "Pickupable_IsEnabled on <class>: its own answer is not read".
  - **Added where the note is silent:** a receiver the store has not loaded is `NotYet { NotRead }`, because a
    handle's slots are not the package's values.
  - Nothing is written, loaded or drawn.
- **Reference:** unchanged; it binds neither key (no C++ stand-in).
- **The slice gate** (installed, Off, strict, release build, under the machine lock): 6 ok / 8 not ok on both. The
  step share goes from 47.0% to 51.2%.
  - The baseline is the earlier run of the scaled-bounds code (39fce848, the same code as 943f8f1a).
  - **Link 6** passes `touch`, so its steps go from 1/3 to 2/3. It stops in `key` at the unbound
    `WillowPlayerController.CanAffordToPickUpPickupable(interface)`. The strict grading counts that as an error.
  - **Link 7** passes `touch`, so its steps go from 1/4 to 2/4. It stops in `look` at the pickup targeting's HUD
    refusal (NATIVE_PICKUP_TARGETING.md section 1: the headless world spawns no HUD), a `NotYet`.
  - These are the next stops NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 4 predicts.
  - Every other row is unchanged.
- **Tests** (`pickup_tests.rs`, invented packages), the note's three plus two:
  - a toy `WillowPickup` with `bPickupable` set answers true, and cleared answers false, by either key. No code load,
    write, draw, store load or name is counted;
  - a toy `DroppedPickup` that is not a `WillowPickup` is the `NotRead`;
  - a subclass of `WillowPickup` with no field of its own reads the inherited `bPickupable`;
  - a slotless (unloaded) `WillowPickup` is the `NotRead`;
  - Reference binds neither key.

  Each was mutation-checked: an exact-class test, no unloaded check, a constant answer and no class test each fail a
  test. The all-packs wiring test still passes.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game, `DroppedPickup`'s own answer, and the field's placement.
- **Sensitive files:** none.
