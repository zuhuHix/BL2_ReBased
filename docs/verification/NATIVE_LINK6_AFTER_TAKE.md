# Link 6 after the take: RemovePickup, and PickupPickupable to its end (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK6-REMOVEPICKUP-AND-AFTER (decision 2026-11-26-54), the analyst
half of the clean room: **another lane implements this note.** Raw output stays under ignored
`local/analysis/link6after/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED in the running game.**

**Why.** NATIVE_ACTOR_SETBASE.md (#795) found link 6 (`pick_up_gun`) stops at `WillowGlobals.RemovePickup`'s `NotRead`
("the pickup events of RemovePickup") inside `WillowPickup.Destroyed`. That was with its own stops answered:
`SetBase`, `GetAmmoCount`, `SelectAkEvent`, `PlayAkEvent` and the empty-`EndState` destroy. This note reads
`RemovePickup`, then the rest of the `key` step to the end of link 6. It does not redo #795's reads.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`): `WillowPlayerController.PickupPickupable` (the part after `GiveTo`),
  `WillowPlayerController.UnTouchedPickupable`, `Controller.UnTouchedPickupable`, `WillowPickup.Destroyed` and
  `DroppedPickup.Destroyed`.
- **The natives** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - `WillowGlobals.RemovePickup`'s exec and the routine it calls. To settle which object the dispatch asks, the
    instruction that loads that object was checked in the raw bytes: it is the pickup's `Base`;
  - the interface lookup and the event call that routine uses;
  - the `WillowPickup` class test on the base's attached actors;
  - `WorldInfo.AllControllers`' exec.
- **Field places** (the local copy of `tools/ghidra/class_layout.py`): the pickup's `Base`, the base's `Attached`,
  `WillowGlobals.PickupList`, and `Controller.NextController`. Each lands exactly on its declared property.
  - `WorldInfo`'s list head is placed by fit as `ControllerList`. The layout tool does not read `WorldInfo`, because of
    a map property. The iterator walks it through `NextController`.
  - The second event argument is a pickup field beyond the script layout. It is native-only, not a script property, and
    was not read further.
- **A local probe** (an installed Off strict `--slice-gate` on main 54c9cf0, under the machine lock): temporary
  stand-ins, reverted and never committed. They answered #795's stops by that note's rules and rulings, then
  `RemovePickup` by section 2 and `AllControllers` by section 4, logging their inputs. Section 5 gives the result.

## 2. `WillowGlobals.RemovePickup(Pickup)`

NATIVE_WILLOW_GLOBALS.md section 6.3, step 7 read this with medium confidence for the events. The routine, in order:
1. **The consumer.** The pickup's `Base` is asked for its behavior-consumer interface (`IBehaviorConsumer`, from the
   `GearboxFramework` package), through the object's interface query. A pickup with no `Base`, or a `Base` that does
   not provide the interface, has no consumer: **steps 2 and 3 are skipped.**
2. **`OnPickupDetached`.** With a consumer, the event named `OnPickupDetached` is raised on it. It carries two object
   arguments: the pickup, and the pickup's native-only field of section 1.
3. **`OnAllPickupsDetached`.** Still with a consumer, the base's `Attached` list is walked in order. An entry whose
   class is `WillowPickup` or below (a class-chain test), other than this pickup, ends the walk with no further event.
   When no such entry is found, the event `OnAllPickupsDetached` is raised on the same consumer, with the same two
   arguments.
4. **The removal.** Every entry equal to the pickup is removed from the globals' `PickupList`, keeping the others in
   order. This runs whether or not anything was dispatched.

**What this corrects in NATIVE_WILLOW_GLOBALS.md 6.3, step 7:**
- the events are not global: they go to the pickup's base, as a behavior consumer;
- the "list held by that actor" is the base's `Attached`;
- the "engine helper" is the `WillowPickup` class test.

The removal there stands as written.

The event names also appear as natives of `InteractiveObjectDefinition` and `AIClassDefinition`: two kinds of
definition that can receive them. Their bodies were not read.

### 2.1 Proposed Off answer

Bind `WillowGlobals.RemovePickup(object)` in Off. It replaces `ow_inventory::pickup`'s `NotRead`; check for other
bindings and placeholders first.

**The preflight** writes and loads nothing. It checks:
- the declarations, each else `VmError::Script`:
  - the globals' `PickupList`, an array of `WillowPickup` (the existing `globals::pickup_list` checks);
  - the pickup's `Base` (an object);
  - with a `Base`, the base's `Attached` (an array of objects);
- the argument: a None pickup is `VmError::Script`, because the game reads its `Base` without a test; a pickup the
  store has not loaded is `NotRead` (a load is a write);
- with a `Base`:
  - a base the store has not loaded is `NotRead`;
  - whether the base's class provides `IBehaviorConsumer`, by the class table's interface list. That is the same
    question the port's interface query answers, and it loads nothing;
  - a base that provides it is `NotRead` ("RemovePickup: the base's behavior-consumer events"), decided before any
    write. The consumer's event call is not ported;
- no `Base`, or a base without the interface: no event.

**The commit:** the removal (step 4), one write of `PickupList`. A refusal after it carries the partial-writes prefix.

**On the slice:** the pickup has no `Base`, so there is no event, only the removal.

**A harness observation (for the hub).** In the pickup world, `PickupList` is empty at the take: the probe saw no
`AddPickup` call during `make`. NATIVE_WILLOW_GLOBALS.md section 1 says `WillowPickup.PostBeginPlay` adds the pickup.
So the removal changes nothing here. Either the harness spawn skips that script, or it runs on another path. This was
not followed; it is not a rule, and it does not change link 6's result.

## 3. The rest of `WillowPickup.Destroyed`

After `RemovePickup` (NATIVE_WILLOW_GLOBALS.md section 1, item 4):
- `DroppedPickup.Destroyed` finds `Inventory` None, which `GiveTo` cleared, and destroys nothing;
- `LocalPlayerControllers` is bound; the pickup world has no HUD, so no `RemovePostRenderedActor` is called;
- the population master is None in the supported fixture, so `RemoveSpawnedActor` is skipped.

The probe passed through all three with no stop.

## 4. `PickupPickupable` after `GiveTo` (its step 9)

In the script, after the pickup's `GiveTo`:
1. `CurrentTouchedPickupable` := None when it is this pickupable; then the same for `CurrentSeenPickupable`;
2. **`foreach WorldInfo.AllControllers(class'WillowPlayerController', PC)`**, each `PC.UnTouchedPickupable(this
   pickupable)`:
   - `WillowPlayerController.UnTouchedPickupable` clears `CurrentTouchedPickupable` when it equals the argument;
   - then it calls `Controller.UnTouchedPickupable`, which is empty;
3. the bulk loop over the gathered nearby pickups. The slice's gun gathers none (NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md
   section 2, step 1);
4. **`UpdateAmmoCounts`**, silent exactly when the inventory count changed. This is #789's binding, already in the
   port;
5. return.

`key` is link 6's last step. After it, the gate checks that the gun was taken.

### 4.1 `WorldInfo.AllControllers(BaseClass, out C)` (iterator)

The exec walks the world info's controller list from its head, through each controller's `NextController`, in list
order:
- a None class, or exactly `Controller`, visits every entry;
- any other class visits the entries whose class is that class or below it (a class-chain test);
- the next link is read **before** the body runs, so a body that unlinks the current controller does not cut the walk.

NATIVE_ENGINE_CORE.md listed this iterator as not read.

**Proposed Off answer:** an iterator in Off (`WorldInfo.AllControllers(class,object)`):
- `ControllerList` on the world info and `NextController` on each controller must be declared as objects, else
  `VmError::Script`;
- a controller the store has not loaded is `NotRead` (a load is a write);
- the walk is bounded: a loop in the list is `VmError::Script`;
- the VM's iterators take their list at the call, but the game reads the links live. So use the iterator guard that
  #789 added for `ComponentList`: keep a copy of the chain, and compare it after each body. A changed chain is
  `NotRead` ("AllControllers: the loop body changed ControllerList"), with the partial-writes prefix. Equal chains
  give the game's answer.

**On the slice:** the pickup world's `ControllerList` is empty (the probe walked 0 entries), so the body never runs.
In the game the player's controller is on the list; when and by what it joins is NATIVE_ACTOR_DESTROY_CONTROLLER.md's
open question 3. Its `UnTouchedPickupable` would then find `CurrentTouchedPickupable` already None (step 1), so the
result is the same. **For the hub:** whether to link the pickup world's controller into the list as a harness input,
as #789 gave it the level's `WorldInfo`. It changes no state on this path.

## 5. Predicted stops

The probe answered #795's stops by that note, then `RemovePickup` by section 2 and `AllControllers` by section 4.1.
**Link 6 then passes: all three steps, `ok`.** The gate's take check holds.

The installed rows were 8 ok / 14, X = 8.500/14 = 60.7%. On main 54c9cf0 they are 7 ok, X 58.3%, with link 6 at
`SetBase`. Every other row is unchanged.

In order, after #795 is implemented, link 6 meets:
1. `RemovePickup`'s current `NotRead`. Section 2's answer removes it;
2. `WorldInfo.AllControllers`, unbound, which is an error row. Section 4.1's answer removes it;
3. nothing else: the second `UpdateAmmoCounts` is bound and answers on the slice.

Link 7 is unchanged: the HUD `LayerAbsent` in `look`.

## 6. Tests (synthetic fixtures)

1. `RemovePickup`, no `Base`:
   - a list of [A, P, B, P] becomes [A, B];
   - no event is raised;
   - one write.
2. `RemovePickup` with a `Base` that does not provide `IBehaviorConsumer`: the same removal, no event.
3. `RemovePickup` with a `Base` that provides it: `NotRead`, raised before any write, so the list is unchanged.
   The counters, `class_code_loads` and the name table are unchanged too.
4. `RemovePickup` refusals, each before any write:
   - a None pickup: `VmError::Script`;
   - a pickup or base the store has not loaded: `NotRead`;
   - `PickupList` declared as other than an array of `WillowPickup`: `VmError::Script`.
5. `AllControllers` over a toy list C1 → C2 → C3:
   - a None class: all three, in order;
   - `Controller`: all three;
   - a subclass that only C2 is: [C2];
   - an empty list: no body.
6. `AllControllers`' guard: a body that unlinks C2 is the prefixed `NotRead`. A body that writes something else is
   not refused.
7. `AllControllers`' refusals:
   - a loop in the list: `VmError::Script`;
   - an unloaded controller: `NotRead`;
   - an undeclared `NextController`: `VmError::Script`.
8. `UnTouchedPickupable` through the VM: the matching `CurrentTouchedPickupable` is cleared; any other value is kept.
9. Reference binds neither key.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the consumer's event call: how a behavior consumer routes a named event and its two arguments;
   - the `OnPickupDetached` natives of `InteractiveObjectDefinition` and `AIClassDefinition`;
   - the pickup's native-only field passed as the second argument.
3. **By fit:** the world info's list head as `ControllerList`.
4. **For the hub:**
   - the empty `PickupList` in the pickup world (section 2.1);
   - linking the pickup world's controller into `ControllerList` (section 4.1).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Pick up a dropped gun lying on the ground. Read `WillowGlobals.PickupList` before and after: expect the pickup gone
   and the others kept in order. Expect no `OnPickupDetached` behavior event, since there is no base.
2. Take a gun from a chest or vending machine (a pickup with a `Base`). Hook the base's behavior events: expect
   `OnPickupDetached`, then `OnAllPickupsDetached` when it was the last pickup attached there.
3. Hook `WillowPlayerController.UnTouchedPickupable` at a pickup. Expect one call per player controller, the local one
   included.
