# Actor.SetBase, and link 6's take after it (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-ACTOR-SETBASE (decision 2026-11-26-NNN), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/setbase/`. This note
is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here is
UNVERIFIED in the running game.**

**Why.** NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md section 5 predicted, and #789's installed slice gate measured, that link 6
(`pick_up_gun`) stops at `Actor.SetBase` inside `WillowInventory.GivenTo`: NATIVE_GO_DOWN_PATH.md section 3.2's
`NotRead` ("the class's own implementation is not resolved"), after partial writes. This note resolves that
implementation, reads it, and reads link 6's `key` step on to its next stop.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`): `WillowInventory.GivenTo`, `Inventory.GivenTo`, `WillowWeapon.GivenTo`,
  `InventoryManager.AddInventory`, `WillowInventoryManager.AddInventory`, `WillowInventory.PlayPickupSound`,
  `DroppedPickup.GiveTo`, `DroppedPickup.PickedUpBy`, `WillowPickup.PickedUpBy`, `WillowPickup.GiveTo`, the `Pickup`
  states' `EndState` of `DroppedPickup` and `WillowPickup`, and the `Destroyed` events of both. The parameter and event
  names come from the package's own exports.
- **The natives** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - `Actor.SetBase`'s exec, then the class implementation it hands to. That implementation was found in the class tables
    of `Actor`, `Inventory`, `Weapon`, `WillowWeapon`, `WillowInventory`, `DroppedPickup` and `WillowPickup`. Each
    table came from the class's registration routine and default constructor, by the documented method
    (`docs/NATIVE_ANALYSIS.md`, "When `virtual:` refuses a class"), with a throwaway helper under
    `local/analysis/setbase/` (not tracked). **All seven tables hold the same function in that slot**, so the slice's
    weapon runs `Actor`'s own `SetBase`;
  - `WillowWeapon.GetAmmoCount`'s exec and the implementation in `WillowWeapon`'s table, and `GetMaxAmmo` beside it.
    They agree with NATIVE_WEAPON_TRIGGER_AMMO.md section 2, read earlier from elsewhere. That agreement is the
    "second entry" check that the method asks for before a table found this way is trusted;
  - `AttributeExpression.SelectAkEvent` and `Actor.PlayAkEvent`'s execs, and the expression walk `SelectAkEvent` uses.
    The walk is the same And walk as `EvaluateExpressions` (NATIVE_ATTRIBUTES.md section 11).
- **Field places** (the local copy of `tools/ghidra/class_layout.py`). Every field the routines read lands exactly on a
  declared property, with no shift:
  - on `Actor`: `Location`, `Rotation`, `Role`, `Base`, `bStatic`, `bDeleteMe`, `bWorldGeometry`, `bMovable`,
    `bHardAttach`, `BaseSkelComponent`, `BaseBoneName`, `Attached`, `RelativeLocation`, `RelativeRotation`;
  - on the component: `ActorComponent.Owner`; on `SkeletalMeshSocket`: `BoneName`, `RelativeLocation`,
    `RelativeRotation`;
  - on the weapon: `InvManager`, `StoredAmmo`, `AmmoPool`, `bUseOverheatBehavior`, `ClipSize`, `OverheatAmmo`;
    `InventoryManager.bInfiniteAmmo`; `ConditionalSoundData`'s `Event`, `Cue` and `Expressions`.

  One skeletal-component field that `SetBase` tests could not be placed: the layout tool does not read
  `SkeletalMeshComponent`, because of a map property. It is not on the slice's path (3.6).
- **A local probe** (an installed Off strict `--slice-gate` on #789's head a9f7187a, which landed as main c79c2f8; under the machine lock): temporary
  stand-ins, each answering by the rule read here and logging its inputs, stubbed each stop in turn. They were reverted
  and never committed. Section 5 gives the order.

## 2. Where link 6 reaches it

`WillowInventoryManager.AddInventory` → `InventoryManager.AddInventory` → `SetOwner` →
`WillowWeapon.GivenTo(NewOwner, bReady, bDoNotActivate)`, which:
1. associates the ammo pool (NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md 3.8);
2. calls **`WillowInventory.GivenTo`**:
   - `AdditionalQueryInterfaceSource` := the new owner;
   - **`SetBase(None)`**, with the other three arguments omitted;
   - `Inventory.GivenTo`: `Instigator` := the owner, `Ready()` when `bReady`, then `ClientGivenTo(owner,
     bDoNotActivate)`;
3. then, when the weapon is `bReadied`: `ReloadCnt` := min(`ClipSize`, **`GetAmmoCount()`**), `LastReloadCnt`,
   `LastClipSize`, and `LoadedWeapon(GetAmmoCount())`.

## 3. `Actor.SetBase(NewBase, optional NewFloor, optional SkelComp, optional AttachName)`

**The exec.** It reads the four arguments. An omitted `NewFloor` becomes (0, 0, 1); an omitted `SkelComp` and
`AttachName` are None. It then calls the class's own implementation with them, and with a fifth input, "notify", that
is always true from script. `NewFloor` is passed on but the implementation never reads it.

**The implementation** (`Actor`'s; section 1: every class on the slice's chain uses it), in order. "Nothing" means
return with no write and no event:
1. **A collection actor as the new base.** A `NewBase` whose class is `StaticMeshCollectionActor` or
   `BlockingMeshCollectionActor` (or below them) is replaced by another actor that the world supplies. By fit that
   actor is the world's info. Its lookup was not read.
2. **A base loop.** Walk from `NewBase` along each actor's `Base`. If the walk reaches the receiver: nothing.
3. **A fixed receiver.** When the receiver is `bStatic`, or not `bMovable`:
   - a `NewBase` that is not `bStatic` and is `bMovable` (a moving base): nothing;
   - a `NewBase` with `bDeleteMe`: nothing.
4. **A component of another actor.** A `SkelComp` whose `Owner` is not `NewBase`: nothing.
5. **No change.** `NewBase` equal to `Base`, `SkelComp` equal to `BaseSkelComponent`, and `AttachName` equal to
   `BaseBoneName`: nothing. Steps 1 to 5 all return before the notify (step 10), so `BaseChange` is not raised.
6. **Leave the old base.** When `Base` is set and is not `bWorldGeometry`:
   - every entry equal to the receiver is removed from the old base's `Attached`;
   - then the old base gets the script event **`Detach(receiver)`**.
7. `Base` := `NewBase`; `BaseSkelComponent` := None; `BaseBoneName` := None.
8. **The relative pose.** When `NewBase` is set and is not `bWorldGeometry`:
   - when the receiver is not `bHardAttach`, or its `Role` is `ROLE_Authority`:
     - `RelativeLocation` := `Location` − `NewBase.Location`;
     - `RelativeRotation` := the receiver's rotation expressed in the base's frame. It is computed through the two
       rotation matrices, and the result is converted back into a rotator. This happens only when no bone name is set,
       which step 7 has just ensured;
   - with a `SkelComp`, see 3.6;
   - then, when the receiver is `bHardAttach` and its `Role` is not `ROLE_SimulatedProxy`, the relative location and
     rotation are recomputed from the full inverse of the base's transform. A hard-attached authority gets both
     computations, the second overwriting the first; a hard-attached simulated proxy gets neither.
9. **Join the new base.** When `Base` (now `NewBase`) is set and is not `bWorldGeometry`:
   - the receiver is appended to `NewBase.Attached`, with no duplicate check;
   - then `NewBase` gets **`Attach(receiver)`**.
10. **Notify.** With "notify" true, the receiver gets **`BaseChange()`**.

**The three events** run through the engine's event call. Each is skipped when the receiver's current state ignores it
(the state's probe mask). Which event goes where is by fit. The old base gets the event that carries the receiver;
the new base gets the other one that carries the receiver; and the receiver gets the one with no argument. Those are
`Detach(Other)`, `Attach(Other)` and `BaseChange()`, the only three of those shapes the class declares. The event names
in the binary were not read as text.

### 3.6 With a skeletal component (not the slice's case)

When `SkelComp` is given, `AttachName` is first looked up as one of the component's mesh sockets. A socket's `BoneName`
replaces `AttachName`. When that name is set, the component passes a readiness test (on the unplaced field of
section 1), the bone exists, and the component has an `Owner`:
- **from a socket:** `RelativeLocation` is the socket's `RelativeLocation`, scaled by the largest axis scale of the
  bone's transform, and `RelativeRotation` is the socket's `RelativeRotation`;
- **from a bare bone:** both come from the actor's transform expressed in the bone's frame. A degenerate bone matrix
  gives the zero matrix there;
- then `BaseSkelComponent` := `SkelComp` and `BaseBoneName` := the bone name, and step 9 follows.

When any of those tests fails, the step 8 rules above apply instead.

### 3.7 The slice's call

The probe logged the receiver's state. The receiver is the `WillowWeapon`; its `Base`, `BaseSkelComponent` and
`BaseBoneName` are all None, and `SetBase(None)` is called with the other three omitted. **Step 5 applies: nothing.**
No write and no event.

### 3.8 Proposed Off answer

Bind the script native `Actor.SetBase(object,vector,object,name)` in Off. It replaces NATIVE_GO_DOWN_PATH.md 3.2's
`NotRead`, which is a placeholder under `ow-combat`'s `go_down` unresolved list. Check for existing bindings and
placeholders first.

**The preflight** writes and loads nothing. It checks:
- the declarations, each else `VmError::Script`:
  - on the receiver: `Base`, `BaseSkelComponent`, `BaseBoneName`, `Attached` (an array of objects), the flags
    `bStatic`, `bMovable`, `bDeleteMe`, `bWorldGeometry` and `bHardAttach`, `Role`, and `Location`, `Rotation`,
    `RelativeLocation` and `RelativeRotation`;
  - on `NewBase` and the old base, the subset each step reads;
- a `NewBase` or old base the store has not loaded: `NotRead` (a load is a write);
- the walk of step 2 is bounded: a `Base` chain that loops without reaching the receiver is `VmError::Script`;
- steps 1 to 5 in order. Step 1's replacement is `NotRead` ("SetBase: a collection actor as the base"). A `SkelComp`
  that passes step 4 is `NotRead` ("SetBase: the bone and socket placement"), because 3.6 needs the skeleton's bone
  transforms;
- the step 8 cases that need math the port does not have, each `NotRead` ("SetBase: the relative rotation to a turned
  base" and "SetBase: the hard-attach relative transform"):
  - the full inverse transform, for a hard-attached receiver that is not a simulated proxy;
  - the relative rotation to a base whose `Rotation` is not zero.

  With a base `Rotation` of zero, the relative rotation is the receiver's `Rotation`. That holds only when the
  receiver's `Rotation` is already normalised: each component within the 16-bit signed range, and pitch within a
  quarter turn. Otherwise it is `NotRead` too, because the conversion back renormalises. UNVERIFIED.

**The commit**, steps 6 to 10 as read:
- the old base's `Attached` removal, then `Detach`;
- the three fields;
- the relative pose;
- the append, then `Attach`;
- `BaseChange`.

The events go through the VM's event call, as `SetOwner`'s `LostChild` and `GainedChild` do. The port's event call has
no probe masks; record that as a gap.

A refusal after the first write carries the "stopped after partial writes" prefix (#518's counters).

**The slice needs only step 5** (nothing). An implementer may bind steps 1 to 5 and step 7's no-base cases first, and
answer the rest `NotRead` as named above.

## 4. Link 6 after SetBase

### 4.1 `WillowWeapon.GetAmmoCount()`

Its exec hands to `WillowWeapon`'s implementation, which is NATIVE_WEAPON_TRIGGER_AMMO.md section 2's rule, re-read
here:
- the owner's `InvManager.bInfiniteAmmo`: `ClipSize`;
- else, with a valid `AmmoPool`, the pool's current value truncated to an int. When the reference's cached object is
  empty, it is refreshed first: the game writes that cache;
- else, with `bUseOverheatBehavior`, `OverheatAmmo`;
- else `StoredAmmo`.

`GetMaxAmmo` follows the same shape: the pool's maximum, else the weapon type's `MaxStoredAmmo` (`ClipSize` for an
overheat weapon).

**Off answer:** bind `WillowWeapon.GetAmmoCount()` with that rule, reading only:
- the pool reference is resolved on a copy, and the cache is not written back. That is the port's convention, as for
  the pawn's `HealthPool`;
- `InvManager`, `AmmoPool`, `bUseOverheatBehavior`, `StoredAmmo`, `OverheatAmmo` and `ClipSize` must be declared, else
  `VmError::Script`;
- a manager or pool the store has not loaded is `NotRead`.

**The slice:** no pool is associated (NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md 3.8: the gun has no `Instigator` at
`AssociateAmmoPool`), and the weapon is not overheat, so the answer is `StoredAmmo`, 30. It is called twice. Today the
key is unbound, so this is an error row, not a `NotYet`.

### 4.2 `WillowInventory.PlayPickupSound(Other, bEquipped)`

`WillowInventoryManager.AddInventory` calls it on the new item when its `bPlayPickupSound` is set:
- with `bEquipped`, an AkEvent is chosen by **`AttributeExpression.SelectAkEvent(Other.Controller, the definition's
  PickupAndEquipSounds)`**;
- with none chosen, or without `bEquipped`, from `PickupSounds`;
- a chosen event is played by **`Other.PlayAkEvent(event)`**.

**`SelectAkEvent(ContextSource, SoundList)`** (static) walks the list in order:
- an entry whose `Expressions` are non-empty and hold (And, with `ContextSource` as the source and no override) is the
  answer, and the walk stops;
- an entry with an empty `Expressions` list is taken as the fallback, and the walk goes on. A later empty-list entry
  replaces it;
- an entry whose expressions fail changes nothing;
- the answer is the last fallback, or None. `Cue` is not read.

**Off answer:** bind it with `ow-attr`'s expression walk, the same core as `EvaluateExpressions`. It reads only. Each
entry's `Event` (an object) and `Expressions` (an array of the expression struct) must be declared, else
`VmError::Script`.

**The slice's list:** one entry, `Ak_Play_Obj_Pickup_Equip_Rifle`, with no expressions. So that event is chosen.

**`PlayAkEvent(InSoundCue, ...)`:**
- with a None event: nothing, and the return value stays at its default;
- with an event: the audio engine plays it on the actor, with the four replication and lifetime flags, and returns a
  playing handle.

**Off answer:** a None event is nothing. A set event is `LayerAbsent` ("an audio layer"), by default. That stops
link 6 here. **The hub's call:** whether to answer it with a presentation stand-in instead: nothing played, the
default handle returned, and a logged UNVERIFIED line. The probe did that to read on. Its result is unused on this path.

### 4.3 The pickup's destroy

After the item is added, `DroppedPickup.GiveTo` clears the pickup's `Inventory` and calls `PickedUpBy(P)`.
`WillowPickup.PickedUpBy` records `PickedUpByActor` := P, then `DroppedPickup.PickedUpBy` calls **`Destroy()` on the
pickup, which is in its script state `Pickup`**.

The port's destroy refuses any actor that is in a state: "EndState when an actor in a state is destroyed",
`LayerAbsent`, W5-STATES. NATIVE_ACTOR_DESTROY_CONTROLLER.md 5.3 records this refusal as broader than the game's
routine. NATIVE_ENGINE_CORE.md's destroy routine sends `EndState(None)` only behind its own condition, and that
condition is the least certain part of the routine.

The scripts that would run:
- `WillowPickup.Pickup.EndState`, which is empty. It replaces `DroppedPickup.Pickup.EndState`, which would remove the
  pickup from navigation;
- then `WillowPickup.Destroyed`:
  - the item cast of `Inventory`, now None;
  - `WillowGlobals.RemovePickup(self)`;
  - `DroppedPickup.Destroyed`, with nothing to destroy;
  - each local player controller's HUD told to forget the pickup;
  - the Gearbox globals' spawned-actor removal.

**Off answer: the hub's call.** One option is a narrow lift for an actor whose current state's `EndState` is empty
(the slice's case): destroy it as if stateless. The other is to keep the refusal until W5-STATES models the state frame.

### 4.4 The next stop

The probe lifted 4.3 for the pickup only. Link 6 then stops at `WillowGlobals.RemovePickup`'s existing `NotRead`
("the pickup events of RemovePickup", NATIVE_WILLOW_GLOBALS.md), inside `WillowPickup.Destroyed`, after partial writes.

`PickupPickupable`'s remaining steps were not reached: the touched and seen slots cleared, `UnTouchedPickupable`, and
the second `UpdateAmmoCounts` (NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md section 2, step 9). `key` is link 6's last step;
after it, the gate checks that the gun was taken.

## 5. Predicted stops

Once `SetBase` is bound (3.8), link 6 meets, in order:
1. `WillowWeapon.GetAmmoCount()` unbound: an error row (4.1);
2. `AttributeExpression.SelectAkEvent` unbound: an error row (4.2);
3. `Actor.PlayAkEvent` unbound: an error row. Bound by 4.2's default, it is the audio `LayerAbsent` (the hub's call);
4. the pickup's stateful destroy, the existing W5-STATES `LayerAbsent` (4.3; the hub's call);
5. `WillowGlobals.RemovePickup`'s `NotRead` (4.4).

The probe answered 1 to 3 and lifted 4 to reach 5. Link 7 is unchanged: the HUD `LayerAbsent` in `look`.

## 6. Tests (synthetic fixtures)

1. `SetBase`, no change: a toy actor with `Base`, `BaseSkelComponent` and `BaseBoneName` None, and `SetBase(None)`.
   Nothing written, no event; the counters, `class_code_loads` and the name table are unchanged.
2. `SetBase` onto a base:
   - an unrotated base at (10, 0, 0), and a receiver at (12, 3, 4) with a normalised rotation;
   - expect `Base` set, `BaseSkelComponent` and `BaseBoneName` None, `RelativeLocation` (2, 3, 4), `RelativeRotation`
     the receiver's;
   - the receiver appended to the base's `Attached`;
   - events in the order `Attach(receiver)` on the base, then `BaseChange()` on the receiver.
3. Changing bases: from base A to B, A's `Attached` loses every copy of the receiver and A gets `Detach` before B gets
   `Attach`. Base None leaves A, and no `Attach` is sent.
4. A `bWorldGeometry` base: `Base` set, but no relative pose, no `Attached` change and no `Attach`; `BaseChange` still
   runs.
5. Refusals that do nothing (no event, no `BaseChange`):
   - a base loop;
   - a `bStatic` receiver onto a movable base;
   - a fixed receiver onto a `bDeleteMe` base;
   - a `SkelComp` owned by another actor.
6. Off `NotRead`s, raised before any write:
   - a collection-actor base;
   - a `SkelComp` from the base;
   - a hard-attached receiver that is not a simulated proxy;
   - a turned base;
   - an unloaded base.
7. A refusal after the old base's `Detach` (a toy `Detach` that writes) carries the prefix.
8. `GetAmmoCount`:
   - infinite ammo: `ClipSize`;
   - a pool at 42.9: 42;
   - no pool: `StoredAmmo`;
   - overheat: `OverheatAmmo`;
   - the pool's cache is not written back.
9. `SelectAkEvent`:
   - an empty list: None;
   - one empty-expression entry: its event;
   - an entry that fails, then one that holds: the second;
   - two empty-expression entries: the last;
   - an empty entry, then one that holds: the one that holds.
10. `PlayAkEvent`: a None event writes nothing. A set event is the audio `LayerAbsent` (or, under a ruling, the
    stand-in line).
11. Reference binds none of the keys.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **By fit:**
   - which of the three events goes where (section 3);
   - step 1's replacement actor;
   - the readiness test of 3.6.
3. **Not read:**
   - the audio engine's play routine (4.2);
   - the world's info lookup (step 1);
   - the destroy routine's `EndState` condition. It is the same open point as NATIVE_ENGINE_CORE.md's.
4. **The hub's calls:**
   - `PlayAkEvent`'s stand-in (4.2);
   - the pickup's stateful destroy (4.3).
5. **Gaps the implementer should record:**
   - the port's event call has no probe masks (section 3);
   - `GetAmmoCount`'s cache refresh is not written back (4.1).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Pick up a dropped gun. Read the weapon's `Base`, `BaseSkelComponent` and `BaseBoneName` before and after
   `WillowInventory.GivenTo`: expect None throughout, and no `BaseChange` hook on the weapon.
2. Hook `Actor.Attach`, `Actor.Detach` and `Actor.BaseChange` while a pawn steps onto and off a mover. Expect `Detach`
   on the old base before `Attach` on the new one, and `BaseChange` last.
3. Hook `PlayAkEvent` on the player pawn at pickup. Expect the pickup-and-equip event chosen by the definition's list.
