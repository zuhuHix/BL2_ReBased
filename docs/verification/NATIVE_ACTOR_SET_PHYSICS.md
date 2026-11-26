# Actor.SetPhysics: changing an actor's physics mode (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-ACTOR-SETPHYSICS (decision 2026-11-25-23), the
analyst half of the clean room: **another lane implements this note.** Raw output stays under ignored
`local/analysis/setphys/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED in the running game.**

**Why.** The hub's runner probe of main + #682 (the Queue port) shows slice link 9's `cast` stopping at the unbound
native `Actor.SetPhysics(byte)`. The harness record names only the native, not its caller. No NATIVE note covered it.

## 1. Evidence, in the order asked

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`, the nine code packages):
  - **a caller on link 9's path (read):** `LiftActionSkill.LiftTarget(BeginLiftTarget)` runs on the phaselocked
    target. In order, it calls `BeginLifting`, plays the phase-lock definition's lift animation on the target's
    special-move component (`Play`), queues its `LoopAnim` (`Queue`), calls **`SetPhysics(14)` on the target**, and
    clears the target's `bCollideWorld`;
  - **that this is the call link 9 stops at is an inference (UNVERIFIED).** No caller stack was recorded. It is the
    `SetPhysics` that follows `Queue` on the Phaselock cast's path, which fits the probe stopping there right after the
    Queue port landed. The same holds for `Queue`: `LiftTarget` is a `Queue` caller missing from
    NATIVE_SPECIAL_MOVE_QUEUE.md section 1, and probably the one link 9 reaches. That note's census is amended in its
    section 4;
  - the mode: `EPhysics` in `Engine.upk` lists `PHYS_None` 0, `PHYS_Walking` 1, `PHYS_Falling` 2, `PHYS_Swimming` 3,
    `PHYS_Flying` 4, `PHYS_Rotating` 5, `PHYS_Projectile` 6, `PHYS_Interpolating` 7, `PHYS_Spider` 8, `PHYS_Ladder` 9,
    `PHYS_RigidBody` 10, `PHYS_SoftBody` 11, `PHYS_NavMeshWalking` 12, `PHYS_Unused` 13, `PHYS_Custom` 14 and
    `PHYS_MAX` 15. **`LiftTarget` asks for `PHYS_Custom`**;
  - the events the native fires:
    - `WillowPawn`'s `StoppedFalling()`, `BeganFalling(PreviousPhysics)`, `BeganWalking(PreviousPhysics)` and
      `StoppedWalking()`. On `WillowPawn` they reset gravity scaling, or forward to the action skill's
      `OnActionSkillOwner…` hooks;
    - `PhysicsVolume.PhysicsChangedFor(Other)`, which is empty on `PhysicsVolume` and overridden by `LadderVolume`;
- **The native** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - the exec stub calls the actor's physics-change virtual with the new mode, no new floor, and an upward floor normal;
  - the virtual's bodies along the target's class chain: `Actor`, `Pawn` (which `GearboxPawn` keeps), `WillowPawn` and
    `WillowAIPawn`. The tables were found as in NATIVE_COMPONENT_SKIP_FLAG.md section 6;
  - field names: `tools/ghidra/class_layout.py`, with a local copy giving `MapProperty` a stand-in size. Event names are
    **by fit**: each matches its flag's name and its parameter shape.

## 2. What the native does

Each layer of the class chain wraps the next. Grouped by effect, in the order an observer would see it:

**The AI pawn's own work comes first, whatever the request.** On a `WillowAIPawn`:
- a request for `PHYS_NavMeshWalking` runs a check through the pawn's mind and navigation data, which can turn the
  request into `PHYS_Walking`. It was not read in detail;
- a request for `PHYS_RigidBody`, on a pawn whose `bTearOff` is clear and whose `Role` is not `ROLE_Authority`, fires a
  script event (not identified).

These two happen **before** the gates below. So even a request that then changes nothing can have done this work.

**Two ways for the request to change nothing further:**
- on a `WillowAIPawn` with `bHardAttach` set, the request stops after the AI work above, unless it is
  `PHYS_Interpolating`. No mode change, no base or velocity change, no volume event, no falling or walking event;
- a request for the mode the actor already has stops at the `Actor` layer. The mode is unchanged, and so are the base,
  velocity, volume and rigid body. No falling or walking event can fire either, because each needs a change of mode.

**The mode change itself** (`Actor`): `Physics` takes the new mode.

**Effects that depend on the transition:**
- *into `PHYS_Walking` from any other mode* (`Pawn`): `bForceFloorCheck` is set, before the mode is written;
- *into `PHYS_None`, `PHYS_Walking`, `PHYS_Rotating` or `PHYS_Spider`* (`Actor`): with no new floor, which is always
  the case from script, an actor virtual is called. It is the base search: a downward floor trace of 8 units, then
  the base setter when the floor differs (NATIVE_LINK34_AFTER_MIND.md section 2);
- *into any other mode except `PHYS_Interpolating`, while a `Base` is set* (`Actor`): the base is cleared (set to None,
  with the floor normal up, notifying the actor). This is by fit to the base setter's argument shape;
- *into `PHYS_None` or `PHYS_Rotating`* (`Actor`): `Velocity` and `Acceleration` are zeroed;
- *into or out of `PHYS_RigidBody`, with a `CollisionComponent` set* (`Actor`): one or two virtuals of that component
  are called (not identified; presumably waking or settling its rigid body);
- *any change, with a `PhysicsVolume` set* (`Actor`): the volume's `PhysicsChangedFor(this actor)` event runs, after the
  mode is written.

**The falling and walking events** (`WillowPawn`), after everything above. "Walking" means `Physics` is `PHYS_Walking`
or `PHYS_NavMeshWalking` (read). Each event needs its notify flag, and is judged from the state before and after:
- `StoppedFalling()`: `bNotifyStopFalling`, the old mode `PHYS_Falling`, the new one not;
- `BeganFalling(old mode)`: `bNotifyBeginFalling`, the old mode not `PHYS_Falling`, the new one `PHYS_Falling`;
- `BeganWalking(old mode)`: `bNotifyBeginWalking`, not walking before, walking after;
- `StoppedWalking()`: `bNotifyStopWalking`, walking before, not walking after.

They fire in that order.

**For `LiftTarget`'s request** (`PHYS_Custom` on a `WillowAIPawn`):
- the AI work does not apply (neither mode is asked);
- `bHardAttach`, if set, stops it;
- otherwise `Physics` becomes 14, the base is cleared if one is set, the volume event runs if there is a volume, and
  the rigid-body step applies only if the old mode was `PHYS_RigidBody`;
- the notify flags decide the events.

**The target's class defaults** (`WillowGame.Default__WillowAIPawn`): `Physics` 0, no `Base`, no `PhysicsVolume`,
`bHardAttach` and the four notify flags clear, `Role` `ROLE_Authority`. **If the target still holds those values at
the call**, the change writes `Physics` = 14 and nothing else. In the game a live pawn always has a `PhysicsVolume`
(at least the level's default one), so its empty event runs there.

## 3. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Record: `docs/verification/NATIVE_ACTOR_SET_PHYSICS.md`. Bind
`Engine.Actor.SetPhysics` in `ParityMode::Off` with this rule, in the world's actor natives, so the cast's world has
it.

**The hit pack's stand-in becomes Reference-only.** `ow-combat`'s hit pack binds `Engine.Actor.SetPhysics` in both
modes today, as a plain `Physics` byte write. Its binder keeps an existing binding, so in a world that registers the
hit pack first, the stand-in would win in Off. So:
- the hit pack binds its stand-in only in `ParityMode::Reference`, as it already does for the special-move stand-ins;
- Off always gets this rule, whatever the pack order.

COMBAT_HIT_CHAIN.md's table lists `Actor.SetPhysics` among "bound as documented no-ops". That is inexact: the stand-in
writes `Physics`. The table row is corrected to say so and to point here.

**The layers by class.** Walk up from the actor's class to its first native class (the fitted `CLASS_NATIVE` bit, as
`ow-combat`'s `pawn_interface` walk does):
- `WillowGame.WillowAIPawn`: all four layers;
- `WillowGame.WillowPawn`: `WillowPawn`, `Pawn` and `Actor`;
- `GearboxFramework.GearboxPawn` and `Engine.Pawn`: `Pawn` and `Actor`;
- `Engine.Actor`: `Actor` only;
- `WillowGame.WillowPlayerPawn`: read since, it has `WillowPawn`'s three layers (NATIVE_LINK34_AFTER_MIND.md
  section 3);
- any other first native class: a `NotYet` of kind `NotRead`,
  "Actor.SetPhysics: the physics change of <class>".

**Preflight, write-free and loaded-only.** Reads go by name lookup on resident objects. The mode is the argument's
low byte. Each field read must have its declared kind (`Physics` a byte, the flags bools, `Role` a byte), else a
`VmError::Script` naming it. **A referenced object is checked for residency only on a path that uses it:** a same-mode
request, or one stopped by `bHardAttach`, succeeds even with a lazy `PhysicsVolume`, `Base` or `CollisionComponent`.
A non-resident object on a path that uses it is a `NotYet` of kind `NotRead`, "Actor.SetPhysics: an object the store
has not loaded (a load is a write)". In order:
1. the `WillowAIPawn` work:
   - `PHYS_NavMeshWalking`: a `NotYet` of kind `NotRead`, "Actor.SetPhysics: a WillowAIPawn's nav-mesh walking check";
   - `PHYS_RigidBody` with `bTearOff` clear and `Role` not `ROLE_Authority`: a `NotYet` of kind `NotRead`,
     "Actor.SetPhysics: a WillowAIPawn's rigid-body event";
   - `bHardAttach` set and the mode not `PHYS_Interpolating`: answer at once, with nothing changed;
2. the same mode as now: answer at once, with nothing changed;
3. the transition refusals:
   - into `PHYS_None`, `PHYS_Walking`, `PHYS_Rotating` or `PHYS_Spider` (read since: NATIVE_LINK34_AFTER_MIND.md
     section 2, with its proposed answer there): a `NotYet` of kind `NotRead`,
     "Actor.SetPhysics: the base search for a walking-type mode";
   - into any other mode except `PHYS_Interpolating`, with a `Base` set (resident): a `NotYet` of kind `NotRead`,
     "Actor.SetPhysics: clearing the base (the base setter is not ported)";
   - into or out of `PHYS_RigidBody` with a `CollisionComponent` set: a `NotYet` of kind `LayerAbsent`,
     "Actor.SetPhysics: the collision component's rigid body", layer "rigid-body physics (the headless world has
     none)";
4. **every script event the commit will call is resolved now, on its receiver's class**, by a state-aware,
   loaded-only lookup:
   - with a `PhysicsVolume` set: the volume must be resident (else the "not loaded" `NotYet`), and
     `PhysicsChangedFor` must resolve on its class;
   - on a `WillowPawn` layer: each of the four events whose condition will hold after the change must resolve on the
     actor's class.

   A method whose code is not loaded is a `NotYet` of kind `NotRead`, "Actor.SetPhysics: the code of <event> on
   <class> is not loaded". A missing event, or one declared with other parameters (one actor for `PhysicsChangedFor`;
   none, or one byte, for the four events), is a `VmError::Script` naming it.

Every refusal in this list is a preflight refusal: nothing is written, loaded or counted.

**Commit, in the game's order:**
1. `bForceFloorCheck` = true on a transition into `PHYS_Walking` from another mode;
2. `Physics` = the mode;
3. with a `PhysicsVolume`, its `PhysicsChangedFor(actor)` script event, run as script;
4. the four `WillowPawn` events, in section 2's order, each fired only when its condition holds.

A refusal raised inside a script event after step 2 is terminal and carries "stopped after partial writes".

**What link 9 would meet** (a prediction, not a run; the caller is an inference, section 1). If the call is
`LiftTarget`'s, and the target still holds its class defaults, the native writes `Physics` = 14 and nothing else.
`LiftTarget` then clears `bCollideWorld` (a script write), and the cast goes on past `LiftTarget`. Anything the spawn
or the cast changed on the target first would change this:
- a `Physics` of 2 with `bNotifyStopFalling` set fires `StoppedFalling`;
- a `Base` gives the base-clearing `NotRead`;
- a `PhysicsVolume` runs its event.

## 4. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each preflight refusal leaves #518's three counters and the name table
unchanged:
1. a toy `WillowAIPawn` subclass with defaults, asked for `PHYS_Custom` (14): `Physics` 14, nothing else written, no
   event;
2. the same mode as now: nothing changes, even with notify flags set. This holds for modes other than 10 and 12 on a
   `WillowAIPawn`, whose AI work comes first;
3. `bHardAttach` set: 14 is dropped, and `Physics` is unchanged. `PHYS_Interpolating` (7) goes through;
4. old `PHYS_Falling` with `bNotifyStopFalling`, asked for 14: `Physics` 14, then `StoppedFalling` (a spy event).
   Asked for `PHYS_Falling` from 14 with `bNotifyBeginFalling`: `BeganFalling(14)`;
5. walking is 1 or 12: from 12 to 14 with `bNotifyStopWalking`, `StoppedWalking` fires;
6. a toy `Engine.Pawn` subclass asked for `PHYS_Walking` from 2: the base-search `NotRead`, before any write. Neither
   `bForceFloorCheck` nor `Physics` is written;
7. 14 with a resident `Base` set: the base-clearing `NotRead`, before any write;
8. `PHYS_RigidBody` with a `CollisionComponent`: the rigid-body `LayerAbsent`. A `WillowAIPawn` below authority asked
   for 10: the event `NotRead`. Asked for 12: the nav-mesh `NotRead`;
9. a resident `PhysicsVolume` with a spy `PhysicsChangedFor`: called with the actor, after `Physics` is written;
10. **the event boundaries:**
    - a `PhysicsVolume` whose class lacks `PhysicsChangedFor`, or declares it with another parameter: `VmError::Script`,
      with `Physics` unchanged;
    - a `PhysicsVolume` whose `PhysicsChangedFor` code is not loaded: the "code not loaded" `NotRead`, with `Physics`
      unchanged;
    - the same two cases for `StoppedFalling` on a `WillowPawn` subclass whose condition holds;
11. **residency boundaries:**
    - a lazy `PhysicsVolume` on a changing request: the "not loaded" `NotRead`, nothing loaded;
    - a same-mode request, and a `bHardAttach` stop, with a lazy `PhysicsVolume` and `Base`: success, nothing loaded;
12. an actor whose first native class is not listed (a toy native class): the class `NotRead`;
13. `Physics` not declared as a byte: `VmError::Script` naming it;
14. **a post-commit refusal:** a spy `PhysicsChangedFor` that calls a test native **bound to answer a typed `NotYet`**
    (an unbound native would not exercise this path). The refusal carries "stopped after partial writes", and
    `Physics` keeps the new mode. This case has its own assertion, not the "unchanged" one;
15. **both packs:** a world that registers `ow-combat`'s hit pack before the actor natives, and one that registers it
    after. In Off both get this rule (test 1's answer). In Reference both get the stand-in's plain byte write;
16. Reference: the hit pack's stand-in unchanged.

## Open questions

1. Not read:
   - the `WillowAIPawn` nav-mesh walking check and its rigid-body event;
   - the `Actor` base-search virtual for walking-type modes: read since (NATIVE_LINK34_AFTER_MIND.md section 2);
   - the two rigid-body component virtuals;
   - `WillowPlayerPawn` and other classes' overrides: `WillowPlayerPawn` read since, it has none
     (NATIVE_LINK34_AFTER_MIND.md section 3); other classes not read;
2. UNVERIFIED:
   - that `LiftTarget`'s call is the one link 9 stops at (no caller stack was recorded);
   - the event names (by fit);
   - "the base is cleared" (by the setter's argument shape);
   - everything in game.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Phaselock an enemy and read its `Physics` right after the lift starts: expect 14 (`PHYS_Custom`), and
   `bCollideWorld` false.
2. Hook `PhysicsVolume.PhysicsChangedFor` and `WillowPawn.StoppedFalling` during the lift: expect the volume event once.
   Expect `StoppedFalling` only if the target was falling with `bNotifyStopFalling` set.
3. On a pawn that is not an AI pawn, or with a mode other than 10 and 12, call `SetPhysics` with the current mode:
   expect no events.
