## 2026-11-25: Actor.SetPhysics in Off; the hit pack's byte-write stand-in becomes Reference-only

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-ACTOR-SETPHYSICS, slice link 9's current stop (the runner: after
`Queue`, `cast` stops at the unbound `Actor.SetPhysics(byte)`). Branched from main 84b6d99. Coded from
docs/verification/NATIVE_ACTOR_SET_PHYSICS.md section 3 alone (#688 at f361007, decision 2026-11-25-23). Clean room:
the analyst lane read the native; this lane did not open the executable, Ghidra, REA or the analysis output.
Synthetic fixtures only.

**What changes:**
- `ow-world` `world::set_physics` binds `Actor.SetPhysics(byte)` in the world pack's Off block, keep-existing by key.
  `World::new` registers that pack before any other.
- `ow-combat`'s hit pack binds its plain `Physics` byte-write stand-in only in `ParityMode::Reference` now. In Off a
  world that registered the hit pack first would otherwise keep the stand-in.
- COMBAT_HIT_CHAIN.md's table row no longer calls the stand-in a no-op: it writes `Physics`, in Reference only, and
  the row points to the note.

**The rule** (each part UNVERIFIED in game):
- **Layers** come from the actor's first native class: `WillowAIPawn` has all four, `WillowPawn` three, `GearboxPawn`
  and `Pawn` two, `Actor` one. Any other first native class, `WillowPlayerPawn` included, is `NotRead`.
- **The preflight** is write-free and load-free. It reads by name on resident objects, with declared kinds checked
  (else `VmError::Script`), and checks a referenced object's residency only on a path that uses it. In order:
  1. the AI layer's nav-mesh `NotRead`, its rigid-body-event `NotRead`, and the `bHardAttach` stop;
  2. the same mode;
  3. the base-search `NotRead`, the base-clearing `NotRead`, and the rigid-body `LayerAbsent`;
  4. the events the commit will call, resolved state-aware in loaded code only: `PhysicsChangedFor(actor)` on a
     resident volume, and each `WillowPawn` falling or walking event whose condition will hold. Code not loaded is
     `NotRead`; a missing event or another parameter shape is `VmError::Script`.
- **The commit:** `Physics` = the mode, then the volume's event, then the pawn events in the note's order
  (`BeganFalling` and `BeganWalking` get the old mode). A refusal afterwards is marked "stopped after partial writes"
  when #518's counters moved. `Vm::class_code_loads` (#685) is not on this base.
- **Not written:** the note's commit step 1 (`bForceFloorCheck` on a transition into `PHYS_Walking`). Every transition
  into `PHYS_Walking` is refused at the base search, so the step cannot be reached in the port, and this lane writes no
  dead code for it.

**Effect on link 9** (predicted, not run; the caller is the note's inference): if `LiftTarget`'s call reaches a target
still at its class defaults, `Physics` becomes 14 and nothing else is written, and the cast goes on past `LiftTarget`.
A `Base` set on the target would stop it at the base-clearing `NotRead`. A `PhysicsVolume` runs its event, whose code
must be loaded (else `NotRead`).

**Tests** (synthetic; each preflight refusal leaves #518's counters, the name table, the heap and the event list
unchanged): the note's tests 1 to 14 and 16 in `world/set_physics_tests.rs`, and test 15 in `ow-combat`
`hit/set_physics_pack_tests.rs` (the hit pack registered before or after the world's natives: Off gets the rule,
Reference the stand-in).

**Not verified:** anything in the running game; the event names; that `LiftTarget`'s call is link 9's stop.

**Review r1** (Sol high, on #696):
1. **Events are looked up as Off event dispatch finds them.** `ow_vm::states::find_loaded_event` walks the active
   state, its parent states, then the class chain, loading nothing, and the commit calls the `FuncId` it returns. The
   old lookup by the current state's name alone missed an override in a parent state. Test: a pawn in state `Landing`
   (extends `Falling`, which overrides `StoppedFalling`) gets `Falling`'s function. The test fails with the old
   lookup.
2. **Parameters must be scalars by value.** No static array and no `out`. The volume event's object parameter must be
   of a class that accepts the actor. A `BeganFalling(byte[2])` and a `PhysicsChangedFor` of an unrelated object class
   are script errors before any write.
3. **Bound for `Engine`'s function only**, by its package-qualified identity (`is_bound_in` and `register_native_in`
   for `Engine`). Another package's same-keyed function is neither bound nor shadowing.
4. **`Physics` is read after the AI layer's answers**, so a malformed `Physics` does not turn a nav-mesh request or a
   `bHardAttach` stop into a script error.
