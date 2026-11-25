# Actor.SetPhysics: changing an actor's physics mode (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-ACTOR-SETPHYSICS (decision 2026-11-25-23), the
analyst half of the clean room: **another lane implements this note.** Raw output stays under ignored
`local/analysis/setphys/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED in the running game.**

**Why.** The hub's runner probe of main + #682 (the Queue port) shows slice link 9's `cast` stopping at the unbound
native `Actor.SetPhysics(byte)`. No NATIVE note covered it. COMBAT_HIT_CHAIN.md lists it among the hit chain's stand-ins.

## 1. Evidence, in the order asked

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`, the nine code packages):
  - **link 9's caller** is `LiftActionSkill.LiftTarget(BeginLiftTarget)`, on the phaselocked target. In order, it:
    1. calls `BeginLifting`;
    2. plays the phase-lock definition's lift animation on the target's special-move component (`Play`);
    3. queues its `LoopAnim` (`Queue`);
    4. calls **`SetPhysics(14)` on the target**;
    5. clears the target's `bCollideWorld`.

    This is read, not inferred. It also corrects NATIVE_SPECIAL_MOVE_QUEUE.md section 4: that note inferred
    `Action_FollowPath.SetSpecialMoves` as link 9's `Queue` caller, but the caller is `LiftTarget`, right after its own
    `Play`;
  - the mode: `EPhysics` in `Engine.upk` lists `PHYS_None` 0, `PHYS_Walking` 1, `PHYS_Falling` 2, `PHYS_Swimming` 3,
    `PHYS_Flying` 4, `PHYS_Rotating` 5, `PHYS_Projectile` 6, `PHYS_Interpolating` 7, `PHYS_Spider` 8, `PHYS_Ladder` 9,
    `PHYS_RigidBody` 10, `PHYS_SoftBody` 11, `PHYS_NavMeshWalking` 12, `PHYS_Unused` 13, `PHYS_Custom` 14 and
    `PHYS_MAX` 15. **So link 9 asks for `PHYS_Custom`**;
  - the events the native fires:
    - `WillowPawn`'s `StoppedFalling()`, `BeganFalling(PreviousPhysics)`, `BeganWalking(PreviousPhysics)` and
      `StoppedWalking()`. On `WillowPawn` they reset gravity scaling, or forward to the action skill's
      `OnActionSkillOwner…` hooks;
    - `PhysicsVolume.PhysicsChangedFor(Other)`, which is empty on `PhysicsVolume` and overridden by `LadderVolume`;
- **The native** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - the exec stub calls the actor's physics-change virtual with the new mode, no new floor, and an upward floor normal;
  - the virtual's bodies along link 9's target's class chain: `Actor`, `Pawn` (which `GearboxPawn` keeps),
    `WillowPawn` and `WillowAIPawn`. The tables were found as in NATIVE_COMPONENT_SKIP_FLAG.md section 6;
  - field names: `tools/ghidra/class_layout.py`, with a local copy giving `MapProperty` a stand-in size. Event names are
    **by fit**: each matches its flag's name and its parameter shape.

## 2. What the native does, layer by layer (outermost first)

**`WillowAIPawn`:**
1. a request for `PHYS_NavMeshWalking` runs a check through the pawn's mind and navigation data that can turn the
   request into `PHYS_Walking`. It was not read in detail;
2. a request for `PHYS_RigidBody`, on a pawn whose `bTearOff` is clear and whose `Role` is not `ROLE_Authority`, fires
   a script event first (not identified);
3. **with `bHardAttach` set, the request is dropped entirely, unless it is `PHYS_Interpolating`.** Nothing changes, and
   no event fires;
4. otherwise the request goes on to `WillowPawn`.

**`WillowPawn`:** before passing the request on, it notes four things. "Walking" means `Physics` is `PHYS_Walking` or
`PHYS_NavMeshWalking` (read). The four:
- `bNotifyStopFalling` set and the old mode `PHYS_Falling`;
- `bNotifyBeginFalling` set and the old mode not `PHYS_Falling`;
- `bNotifyBeginWalking` set and the actor not walking;
- `bNotifyStopWalking` set and the actor walking.

Then it passes the request on (`Pawn`, then `Actor`). Afterwards it fires, in this order:
1. `StoppedFalling()`, when the first was noted and the mode is no longer `PHYS_Falling`;
2. `BeganFalling(old mode)`, when the second was noted and the mode is now `PHYS_Falling`;
3. `BeganWalking(old mode)`, when the third was noted and the actor is now walking;
4. `StoppedWalking()`, when the fourth was noted and the actor is no longer walking.

**`Pawn`:** with an old mode other than `PHYS_Walking` and a request for `PHYS_Walking`, it sets `bForceFloorCheck`.
Then `Actor`.

**`Actor`** (the base):
1. **the same mode as now: nothing at all.**
2. `Physics` = the new mode;
3. for `PHYS_None`, `PHYS_Walking`, `PHYS_Rotating` or `PHYS_Spider`: with no new floor (always the case from script),
   an actor virtual is called. It was not identified, and is presumably the base search;
4. for any other mode: with a `Base` set and a mode other than `PHYS_Interpolating`, the base is cleared (set to None,
   with the floor normal up, notifying the actor). This is by fit to the base setter's argument shape;
5. for `PHYS_None` or `PHYS_Rotating`: `Velocity` and `Acceleration` are zeroed;
6. with a `PhysicsVolume` set: the volume's `PhysicsChangedFor(this actor)` event;
7. rigid body:
   - a new mode of `PHYS_RigidBody` with a `CollisionComponent` set calls two virtuals of that component (not
     identified; presumably waking its rigid body);
   - an old mode of `PHYS_RigidBody` leaving, with a `CollisionComponent`, calls one of them with the opposite flag.

**For link 9's request (`PHYS_Custom` on a `WillowAIPawn`):**
- nothing in the `WillowAIPawn` layer applies, unless `bHardAttach` is set;
- in the `WillowPawn` layer, only the notify flags can fire events;
- the `Pawn` layer does nothing;
- in `Actor`: `Physics` = 14. The base is cleared if one is set. The volume event fires if there is a volume. There is
  no rigid-body step, unless the old mode was `PHYS_RigidBody`.

**The target's class defaults** (`WillowGame.Default__WillowAIPawn`): `Physics` 0, no `Base`, no `PhysicsVolume`,
`bHardAttach` and the four notify flags clear, `Role` `ROLE_Authority`. With those, the change writes `Physics` = 14 and
nothing else. In the game a live pawn always has a `PhysicsVolume` (at least the level's default one), so its empty
event runs there.

## 3. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `Engine.Actor.SetPhysics` in `ParityMode::Off` with this rule, in the
world's actor natives, so the cast's world has it. Record: `docs/verification/NATIVE_ACTOR_SET_PHYSICS.md`. In
Reference, `ow-combat`'s hit pack keeps its stand-in (a plain `Physics` byte write, as the C++ binds it). In Off that
stand-in must not be the binding that wins, so this rule is bound first wherever both packs register. The hub decides
the registration order.

**The layers by class.** Walk up from the actor's class to its first native class (the fitted `CLASS_NATIVE` bit, as
`ow-combat`'s `pawn_interface` walk does):
- `WillowGame.WillowAIPawn`: all four layers;
- `WillowGame.WillowPawn`: `WillowPawn`, `Pawn` and `Actor`;
- `GearboxFramework.GearboxPawn` and `Engine.Pawn`: `Pawn` and `Actor`;
- `Engine.Actor`: `Actor` only;
- any other first native class (`WillowPlayerPawn` included, not read): a `NotYet` of kind `NotRead`,
  "Actor.SetPhysics: the physics change of <class>".

**Preflight, write-free and loaded-only** (by name lookup on resident objects; a non-resident actor, `Base`,
`PhysicsVolume` or `CollisionComponent` is a `NotYet` of kind `NotRead`, "Actor.SetPhysics: an object the store has
not loaded (a load is a write)"). The mode is the argument's low byte. Each field must have its declared kind
(`Physics` a byte, the flags bools, `Role` a byte), else a `VmError::Script` naming it. In order:
1. the `WillowAIPawn` layer:
   - `PHYS_NavMeshWalking`: a `NotYet` of kind `NotRead`, "Actor.SetPhysics: a WillowAIPawn's nav-mesh walking check";
   - `PHYS_RigidBody` with `bTearOff` clear and `Role` not `ROLE_Authority`: a `NotYet` of kind `NotRead`,
     "Actor.SetPhysics: a WillowAIPawn's rigid-body event";
   - `bHardAttach` set and the mode not `PHYS_Interpolating`: answer at once, with nothing changed;
2. the same mode as now: answer at once, with nothing changed. The outer layers' events cannot fire then, and
   `bForceFloorCheck` cannot be set;
3. the `Actor` layer's refusals:
   - a mode of `PHYS_None`, `PHYS_Walking`, `PHYS_Rotating` or `PHYS_Spider`: a `NotYet` of kind `NotRead`,
     "Actor.SetPhysics: the base search for a walking-type mode";
   - any other mode with a `Base` set (and not `PHYS_Interpolating`): a `NotYet` of kind `NotRead`, "Actor.SetPhysics:
     clearing the base (the base setter is not ported)";
   - `PHYS_RigidBody` entered or left with a `CollisionComponent` set: a `NotYet` of kind `LayerAbsent`,
     "Actor.SetPhysics: the collision component's rigid body", layer "rigid-body physics (the headless world has
     none)";
4. the `WillowPawn` layer: note the four conditions from the old state. Each event that could fire must resolve on
   the class, else a `VmError::Script`.

**Commit, in the game's order:**
1. `Pawn`: `bForceFloorCheck` = true when the request is `PHYS_Walking` and the old mode was not;
2. `Actor`: `Physics` = the mode;
3. `Actor`: with a `PhysicsVolume`, its `PhysicsChangedFor(actor)` script event, run as script;
4. `WillowPawn`: the four events in section 2's order, each fired only if its condition still holds after the change.

A refusal raised by a script event after step 2 is terminal and carries "stopped after partial writes".

**What link 9 would meet** (a prediction, not a run): the target is a `WillowAIPawn` with its class defaults. The
request is `PHYS_Custom` from `PHYS_None`, with no base, no volume and no notify flag. So the native writes `Physics` =
14 and nothing else. `LiftTarget` then clears `bCollideWorld` (a script write), and the cast goes on past
`LiftTarget`. Anything the spawn changed on the target (a `Physics` of 1 or 2, a `Base`) would change this:
- an old mode of 2 with `bNotifyStopFalling` set would fire `StoppedFalling`;
- a `Base` would give the base-clearing `NotRead`.

## 4. Tests for the implementer

Synthetic, with `package()` / `tag()`. Each preflight refusal leaves #518's three counters and the name table
unchanged:
1. a toy `WillowAIPawn` subclass with defaults, asked for `PHYS_Custom` (14): `Physics` 14, nothing else written, no
   event;
2. the same mode as now: nothing at all, even with notify flags set;
3. `bHardAttach` set: 14 is dropped (`Physics` unchanged). `PHYS_Interpolating` (7) goes through;
4. old `PHYS_Falling` with `bNotifyStopFalling`, asked for 14: `Physics` 14, then `StoppedFalling` (a spy event).
   Asked for `PHYS_Falling` from 14 with `bNotifyBeginFalling`: `BeganFalling(14)`;
5. walking is 1 or 12: from 12 to 14 with `bNotifyStopWalking`, `StoppedWalking` fires;
6. a `Pawn` (toy subclass of `Engine.Pawn`) asked for `PHYS_Walking` from 2: the base-search `NotRead`, before any
   write. Neither `bForceFloorCheck` nor `Physics` is written;
7. 14 with a resident `Base` set: the base-clearing `NotRead`, before any write;
8. `PHYS_RigidBody` with a `CollisionComponent`: the rigid-body `LayerAbsent`. A `WillowAIPawn` below authority asked for
   10: the event `NotRead`. Asked for 12: the nav-mesh `NotRead`;
9. a resident `PhysicsVolume` with a spy `PhysicsChangedFor`: called with the actor, after `Physics` is written;
10. an actor whose first native class is not listed (a toy native class): the class `NotRead`;
11. a lazy `Base` or `PhysicsVolume`: the "not loaded" `NotRead`, nothing loaded;
12. `Physics` not declared as a byte: `VmError::Script` naming it;
13. a post-commit failure: a `PhysicsChangedFor` that reaches an unbound native. The refusal carries "stopped after
    partial writes", and `Physics` keeps the new mode;
14. Reference: the hit pack's stand-in unchanged.

## Open questions

1. Not read:
   - the `WillowAIPawn` nav-mesh walking check and its rigid-body event;
   - the `Actor` base-search virtual for walking-type modes;
   - the two rigid-body component virtuals;
   - `WillowPlayerPawn` and other classes' overrides;
2. UNVERIFIED: the event names (by fit); "the base is cleared" (by the setter's argument shape); everything in game.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Phaselock an enemy and read its `Physics` right after the lift starts: expect 14 (`PHYS_Custom`), and
   `bCollideWorld` false.
2. Hook `PhysicsVolume.PhysicsChangedFor` and `WillowPawn.StoppedFalling` during the lift: expect the volume event once.
   Expect `StoppedFalling` only if the target was falling with `bNotifyStopFalling` set.
3. Call `SetPhysics` with the current mode: expect no events.
