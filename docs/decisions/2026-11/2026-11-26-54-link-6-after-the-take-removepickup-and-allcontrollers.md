## 2026-11-26: Link 6 after the take: RemovePickup and AllControllers

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK6-REMOVEPICKUP-AND-AFTER, docs only, on main 54c9cf0. The analyst
half of the clean room: another lane implements NATIVE_LINK6_AFTER_TAKE.md. No session was run. Raw output stays under
ignored `local/analysis/link6after/`. UNVERIFIED in the running game.

- **`WillowGlobals.RemovePickup` is read past NATIVE_WILLOW_GLOBALS.md's medium-confidence events:**
  - the pickup's `Base` is asked for `IBehaviorConsumer`. Without one there is no event;
  - with one, `OnPickupDetached` is raised on it with two object arguments. Then `OnAllPickupsDetached` follows unless
    another `WillowPickup` is in the base's `Attached`;
  - then every entry of the pickup is removed from `PickupList`.

  Off answer: the removal, plus a pre-write `NotRead` when the base provides the interface. The consumer's event call is
  not ported. The slice's pickup has no `Base`.
- **`WorldInfo.AllControllers` is read.** It walks the world info's controller list through `NextController`. The class
  filter is the class chain (None or `Controller`: every entry), and the next link is read before the body. Off answer:
  an iterator with #789's guard. The pickup world's list is empty, so the slice's `UnTouchedPickupable` loop does not
  run. In the game it would change nothing either, because the touched slot is already cleared.
- **The rest of `PickupPickupable`:** the slot clears, the loop, and the second `UpdateAmmoCounts`, which is bound since
  #789.
- **The probe** (main 54c9cf0; #795's stops and these two answered by their rules; reverted): **link 6 passes**, 8 ok
  / 14, X 60.7%, against 7 ok, 58.3% on main.
- **The hub's ruling (2026-11-26):** two known gaps stay as they are, in Off only, with Reference unchanged:
  - the pickup world never calls `AddPickup` during `make`, so `PickupList` is empty;
  - its controller is missing from `ControllerList`.

  Neither changes link 6's result.
- **Review r1 of #797:**
  - the interface test interns no name and loads no class code: an absent name or class is a pre-write `NotRead`;
  - the `AllControllers` guard is documented as stricter than the game;
  - NATIVE_PICKUP.md's "global events" lines point here;
  - the merged `RemovePickup` row keeps #795's 4.4 pointer.
- **Paperwork:**
  - `NATIVE_INDEX.md`: both `RemovePickup` rows edited, and a new `AllControllers` row;
  - pointers in NATIVE_WILLOW_GLOBALS.md 6.3 and NATIVE_ENGINE_CORE.md;
  - a GATES row.
- **Verified:** the reads and the probe's rows. **Not verified:** everything in the running game.
- **Sensitive files:** none.
