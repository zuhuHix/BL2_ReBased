# `WillowPawn.IsStaggered` and `WillowAIPawn.IsStaggered` on link 9 (draft, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-ISSTAGGERED (decision
`docs/decisions/2026-11/2026-11-19-220-is-staggered-on-link-9.md`, number pending). Analyst only: a different lane
implements it (clean room).

**Sources:**
- the records: `tools/native_index_find.py IsStaggered` finds no row. NATIVE_ACTION_SKILL.md section 11 names it a
  read-only helper of the lift;
- the script, read with `research/script_disasm.py`: the callers and the stagger functions of `WillowAIPawn`;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock):
  - the exec step;
  - the two classes' vtables, found from their registrations' constructors;
  - the slot each one holds;
- the installed packages: `tools/ghidra/class_layout.py` for `WillowAIPawn` and `StaggerStateData`;
- a local probe of the installed slice gate (section 4), never pushed.

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/stg/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here
is UNVERIFIED** in the running game.

## 1. What the native answers

`native function bool IsStaggered()` is declared on `WillowPawn`, and `WillowAIPawn` declares it native again. Both
register the same exec step. That step reads no arguments and returns what one pawn virtual answers, so the answer
depends on the receiver's C++ class:
- **`WillowAIPawn`:** `StaggerState.bIsStaggered`, the first bool of the struct `WillowAIPawn.StaggerStateData`.
  - **The structural oracle:** the read lands on the struct's first bool, at the place where the layout tool puts
    `StaggerState` plus 4 bytes. That 4-byte shortfall for `WillowPawn` is the one NATIVE_PLAYER_MOVEMENT.md (line 118,
    the "four bytes off" fields) records.
  - The fit is evidence about the native's body only, and the port uses no offset (section 3, step 5). By fit,
    UNVERIFIED.
- **`WillowPawn` itself:** false. Its slot holds a shared stub that answers zero.
- **`WillowPlayerPawn`, and any other C++ subclass:** not read. That includes the two children of `WillowAIPawn` in
  `WillowGame`'s export super chain, `WillowAICranePawn` and `DeployableTurretActor`. The layout tool reports both as
  extending it, and neither one's slot was read.

**It writes, loads and draws nothing.**

**The state it reads, from the script.**
- `WillowAIPawn.Stagger` sets `StaggerState.bIsStaggered` true, with `bForced` false, the instigator, and
  `RestaggeredCount` 0.
- `WillowAIPawn.UnStagger` sets it false, with `bForced` true. Both writes sit under the script's own test
  `Role == ROLE_Authority && StaggerState.bIsStaggered`.
- Those are the only member writes of the bool that the name search found (every function of the nine code packages
  that mentions `StaggerState`). The search did not separate whole-struct assignments, and `StaggerState` is also
  handled in `ReplicatedEvent`, so a client's copy can change by replication. "Only `Stagger` and `UnStagger`" holds for
  the member writes alone.

## 2. Link 9's call

In `LiftActionSkill.PhaseLockTarget`, when the target cannot be charmed, the script asks `NewTarget.IsStaggered()`
(a virtual call on a `WillowPawn` reference). When it answers true, the script calls
`WillowAIPawn(NewTarget).UnStagger()` before `LiftTarget(NewTarget)`; otherwise it goes straight to `LiftTarget`.

On link 9 the target is a freshly spawned `WillowAIPawn` that was never staggered. Its `bIsStaggered` is the class
default's, false in the probe, so the script goes straight to `LiftTarget`. The other script callers (`OnRanOver`,
`HitByVehicle`, the stagger functions, the `Staggered` state, `IsLiftedPawnIncapacitated`) are not on link 9's path.

## 3. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.**
- **Bind in Off** the two keys `WillowPawn.IsStaggered()` and `WillowAIPawn.IsStaggered()`, by their declarations'
  native keys.
- **Where:** `ow-combat`'s pawn natives, next to `WillowPawn.SetDefaultPhysics` and `GetExpLevel` in
  `hit/pools.rs`, through the keep-existing `bind_fn`. That pack already runs in link 9's world.
- **Reference** keeps the stub policy and is unchanged.

**Preflight** (write-free, load-free, RNG-free), in this order; the first failure is the answer:
1. **The receiver** is set, and its class is `WillowPawn` or a child (the class table's `by_path` and `is_child_of`
   only), as #604's step 1 does. Otherwise a `VmError::Script`.
2. **S1, an unresident receiver**, with two texts, both `NotYet { NotRead }` on this note's record:
   - an unloaded handle (`vm.defs.is_unloaded`): "WillowPawn.IsStaggered: the pawn <path> is an unloaded handle (a load
     is a write)";
   - a slotless stand-in that the store does not load: "WillowPawn.IsStaggered: the pawn <path> is a stand-in with no
     properties (an object the store cannot load)", worded as `ow-attr`'s `eval.rs` words that case.

   `StaggerState` is an inline struct of the pawn, so reading it loads nothing. Any read helper that can load
   (`access::get` and the like) may be reached only after S1.
3. **The class**, by the class table's `by_path` and `is_child_of` only (hub ruling on #611):
   - **exactly `WillowAIPawn`, or a script-only child of it:** the AI rule (step 5);
   - **a recorded native descendant of `WillowAIPawn`**, `WillowGame.WillowAICranePawn` or
     `WillowGame.DeployableTurretActor` (checked with `is_child_of`, no load): S2, since its slot was not read;
   - **exactly `WillowPawn`:** answer **false**;
   - **S2, any other child of `WillowPawn`** (`WillowPlayerPawn` included): `NotYet { NotRead }`,
     "WillowPawn.IsStaggered: the stagger rule of <class> (its virtual was not read)", record this note. A script-only
     child of `WillowPawn` is S2 too; it is not assumed to answer false.
4. **Which classes are native** comes from the list recorded in step 3, carried with the binding, so the check stays
   load-free. The list was taken from the super chain; `research/native_by_class.json` holds function counts only and
   cannot decide which class is native.
5. **The AI rule, read by name:**
   - the field through `vm.space.prop(class, "StaggerState")`;
   - its declaration a struct of type `WillowAIPawn.StaggerStateData`;
   - the member through that struct layout's `field("bIsStaggered")`, a bool, as `hit/pools.rs` reads struct members.

   A field that is not declared, a field that is not that struct, or a member that is not a bool is a
   `VmError::Script` naming it. The answer is that bool, read from the resident pawn's slot. No offset is used.

**The write:** none. The answer is returned, so no refusal is ever marked "stopped after partial writes: " from
inside it.

**Tests** (synthetic classes built with `package()` / `tag()`, every value invented):
1. a `WillowAIPawn` with `bIsStaggered` clear: false; set: true;
2. a script subclass of `WillowAIPawn`: the AI rule;
3. a plain `WillowPawn`, even with a `StaggerState`-like field declared: false;
4. a `WillowPlayerPawn`, another `WillowPawn` subclass, and a script-only child of `WillowPawn`: S2;
5. **an invented child of `WillowAIPawn` for each listed class** (`WillowAICranePawn` and `DeployableTurretActor`): S2;
6. a receiver outside `WillowPawn`: `VmError::Script`;
7. an unloaded pawn handle and a slotless stand-in: S1, each with its own text, nothing loaded (#518's three counters
   unchanged);
8. the wrong shape, as three cases: `StaggerState` missing, `StaggerState` not a struct, and `bIsStaggered` not a
   bool, each a `VmError::Script`;
9. a key bound first keeps its binding; under `ParityMode::Reference` nothing new is bound.

## 4. The probe (local, never pushed)

A local tree at the resolve-context lane's probe commit (d51aa46a, with its uncommitted patch). It merges
`origin/main` 3ad58b0 with #579, #586, #564, #548, #555, #560, #523 (counter conflicts on main's side, except its noun
resolver), #595, #601 and `fix/rust-gearboxmind-queryinterface`.

**Probe-only stand-ins:**
- `GearboxMind.GetAIComponent` bound where link 9 runs;
- the AI components' `QueryInterface` answered by main's component rule;
- the mind's AI component `Owner` set to the mind;
- `WillowAIComponent.PhaseLockOnHold` answering false;
- `ResolveContext` as `ow-attr`'s `resolve_context`;
- `AddAttributeModifier` as the equip pack's body (#607);
- **this lane's:** `IsStaggered` as section 3 proposes, logged.

**Installed Off `--slice-gate`, strict:** ok 2, notyet 9, error 3 of 14.
- The one call was on the target `WillowGame.WillowAIPawn`, which took the AI rule and answered **false**.
- **Link 9** passes 1 of its 2 driver steps (`spawn`) and stops at `cast` with an `error`: the unbound native
  `WillowAIPawn.DropCombatPerch()`.

So link 9's next stop is `WillowAIPawn.DropCombatPerch()`.

## Open questions

1. **Not read:** the slots of `WillowPlayerPawn`, `WillowAICranePawn` and `DeployableTurretActor` (S2 for all three).
2. **UNVERIFIED:** the `StaggerState` fit (the 4-byte `WillowPawn` shortfall).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On a staggered enemy (hit by a vehicle, or by a stagger skill), call `IsStaggered()` and read
   `StaggerState.bIsStaggered`. Expect both true, and both false after `UnStagger`.
2. Call `IsStaggered()` on the player pawn. Expect false (not read).
