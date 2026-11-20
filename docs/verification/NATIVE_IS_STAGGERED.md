# `WillowPawn.IsStaggered` and `WillowAIPawn.IsStaggered` on link 9 (draft, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-ISSTAGGERED (decision
`docs/decisions/2026-11/2026-11-19-NNN-is-staggered-on-link-9.md`, number pending). Analyst only: a different lane
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
- **`WillowAIPawn`:** `StaggerState.bIsStaggered`, the first bool of the `StaggerStateData` struct. The read falls on
  that member once the layout tool's known 4-byte shortfall for `WillowPawn` is added (the same shortfall other notes
  record). By fit, UNVERIFIED.
- **`WillowPawn` itself:** false. Its slot holds a shared stub that answers zero.
- **`WillowPlayerPawn`, and any other C++ subclass:** not read. Whether a subclass of `WillowAIPawn` overrides the
  slot was not checked either.

**It writes, loads and draws nothing.**

**The state it reads, from the script.** `WillowAIPawn.Stagger` sets `StaggerState.bIsStaggered` true (with `bForced`
false, the instigator, and `RestaggeredCount` 0). `WillowAIPawn.UnStagger` sets it false (with `bForced` true), under
an authority test. Nothing else in the nine code packages writes the bool.

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
1. **The receiver** is set, otherwise a `VmError::Script`.
2. **S1, an unloaded receiver:** a handle the store has not loaded (`vm.defs.is_unloaded`), or a stand-in with no
   slots, is `NotYet { NotRead }`, "WillowPawn.IsStaggered: the pawn <path> is not loaded (a load is a write)".
3. **The class**, by the class table's `by_path` and `is_child_of` only:
   - a `WillowAIPawn` or a child: the AI rule (step 4). A child is taken to keep `WillowAIPawn`'s slot (UNVERIFIED,
     open question 1);
   - exactly `WillowPawn`: answer **false**;
   - **S2, any other class** (`WillowPlayerPawn` included): `NotYet { NotRead }`, "WillowPawn.IsStaggered: the
     stagger rule of <class> (its virtual was not read)", record this note.
4. **The AI rule:** `StaggerState` declared as a struct with a bool `bIsStaggered`, otherwise a `VmError::Script` naming
   the field. The answer is that bool, read from the resident pawn's slot with `vm.space.read`.

**The write:** none. The answer is returned, so no refusal is ever marked "stopped after partial writes: " from
inside it.

**Tests** (synthetic classes built with `package()` / `tag()`, every value invented):
1. a `WillowAIPawn` with `bIsStaggered` clear: false; set: true;
2. a script subclass of `WillowAIPawn`: the AI rule;
3. a plain `WillowPawn`, even with a `StaggerState`-like field declared: false;
4. a `WillowPlayerPawn`, and another `WillowPawn` subclass: S2;
5. an unloaded pawn handle and a slotless stand-in: S1, nothing loaded (#518's three counters unchanged);
6. a `StaggerState` of the wrong shape: `VmError::Script`;
7. a key bound first keeps its binding; under `ParityMode::Reference` nothing new is bound.

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

1. **Not read:**
   - `WillowPlayerPawn`'s slot, and whether any C++ subclass of `WillowAIPawn` overrides it;
   - the authority test's place in `UnStagger` (script, outside this native).
2. **UNVERIFIED:** the `StaggerState` fit (the 4-byte `WillowPawn` shortfall).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On a staggered enemy (hit by a vehicle, or by a stagger skill), call `IsStaggered()` and read
   `StaggerState.bIsStaggered`. Expect both true, and both false after `UnStagger`.
2. Call `IsStaggered()` on the player pawn. Expect false (not read).
