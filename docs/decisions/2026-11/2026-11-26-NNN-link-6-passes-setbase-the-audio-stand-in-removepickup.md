## 2026-11-26: link 6 passes: SetBase, the audio stand-in, the destroy lift, RemovePickup and AllControllers

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-LINK6-AFTER-TAKE, on main 7a67b9f. Clean room: implemented from
docs/verification/NATIVE_ACTOR_SETBASE.md (#795 as landed) and NATIVE_LINK6_AFTER_TAKE.md (#797 as landed) alone; this
lane did not read the executable. No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided** (Off only; Reference binds none of it and keeps W5-STATES' destroy refusal):
  - **`Actor.SetBase`** (`ow-world` `world::set_base`) for a receiver whose first native class is one of the seven
    the note read (Actor, Inventory, Weapon, WillowWeapon, WillowInventory, DroppedPickup, WillowPickup); any other
    receiver is `NotRead`, and `SetPhysics`' base-setter stays `NotRead`. A plan step runs the note's steps 1 to 5 and
    step 8's checks with no write (a collection-actor base replaced, the four do-nothing cases, the socket and bone
    placement, a turned base and a hard attach `NotRead`); the commit runs steps 6 to 10 (the old base's `Attached`
    removal and `Detach`, a recheck after `Detach`, the new base's `Attached`, `Attach` and `BaseChange`), the events
    sent through the ordinary event path. go_down's `UNRESOLVED` placeholder for the key is removed at the source.
  - **`WillowWeapon.GetAmmoCount`**, **`AttributeExpression.SelectAkEvent`** and **`Actor.PlayAkEvent`**
    (`ow-inventory` `link6`): the ammo count by the note's rule; the sound entry selection (the first entry whose
    non-empty expressions hold, else the last entry with none, else None); `PlayAkEvent` with a None event returns
    the default handle. A set event is `LayerAbsent` (an audio layer) unless the **audio stand-in** is on: a VM
    harness setting, Off only (enabling it in Reference is a registration error), turned on by `ow-cli`'s equip
    harness and logged as a harness input; with it the call logs "audio is not modelled" and returns the default
    handle. The hub's ruling, UNVERIFIED: the game plays the sound.
  - **The empty-EndState destroy lift** (`ow-world` `actors`): in Off, destroying an actor in a state whose
    `EndState` (looked up without a load: the state's own, else the class's) is a bare return in the decoded script
    no longer refuses; every caller of the destroy gets it, and the destroy logs "harness input: hub ruling: destroy in state …,
    its EndState a bare return; EndState not sent (UNVERIFIED)". A native `EndState`, one with a body, one the lookup
    cannot settle without a load, or a script that does not decode keeps W5-STATES' `LayerAbsent`.
  - **`WillowGlobals.RemovePickup`**: the globals' `PickupList` declared, the pickup and its `Base` resident, the
    base's `Attached` an array of objects, then a pre-write `NotRead` when the base provides `IBehaviorConsumer`
    (its two events not read), or when the interface cannot be named from the tables already loaded (nothing is
    interned, no class code loaded). Then every copy of the pickup is removed from `PickupList` in one write, order
    kept. The older blanket `NotRead` entry in `pickup.rs` is gone.
  - **`WorldInfo.AllControllers`** (`ow-world` `world::all_controllers`): the world info's `ControllerList`, then
    each `NextController` (both declared as objects, else `VmError::Script`); a None class or exactly `Controller`
    visits all, any other class its subclasses; an unloaded controller `NotRead`; a loop `VmError::Script`. #789's
    guard: the chain copied at the call and compared after each body, a change `NotRead` with the partial-writes
    prefix. Stricter than the game, which tolerates unlinking the current controller.
- **Tests** (invented packages): `ow-world` `tests.rs` `mod link6` (SetBase: no change, onto a base and onto
  another with the event order, a world-geometry base, the refusals with nothing written, the `NotRead`s before any
  write; AllControllers by class; the bare `EndState` lifted and the busy, fallback and native ones refused);
  `ow-inventory` `link6_tests.rs` (GetAmmoCount, SelectAkEvent's last fallback, PlayAkEvent with and without the
  stand-in, RemovePickup's removal and its refusals before any write, Reference binding none of them). One older test
  changed with the rule: `pickup_tests.rs`' RemovePickup `NotRead` now checks the list first (that fixture declares
  no `PickupList`). **Not tested:** GetAmmoCount's live-pool case; SelectAkEvent's entry whose expressions hold;
  RemovePickup with a base that provides `IBehaviorConsumer`; the AllControllers guard firing; SetBase's hard attach
  (the toy `Role` has no enum); the destroy lift's lookup that would need a load. No probe masks were added and the
  SetBase plan writes no cache.
- **Measured:** see the GATES row of this lane.
- **Verified:** the tests on invented data, and link 6 passing on the installed Off slice gate (the harness, not
  parity). **Not verified:** everything in the running game.
- **Sensitive files:** none.
