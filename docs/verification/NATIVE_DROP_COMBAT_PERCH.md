# `WillowAIPawn.DropCombatPerch` on link 9 (draft, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-DROPCOMBATPERCH (decision
`docs/decisions/2026-11/2026-11-19-NNN-drop-combat-perch-on-link-9.md`, number pending). Analyst only: a different lane
implements it (clean room).

**Sources:**
- the records: `tools/native_index_find.py DropCombatPerch` finds no row. COMBAT_DEATH.md lists it among the death
  path's presentation no-ops, without a read;
- the script, read with `research/script_disasm.py`: the four callers and `WillowAIPawn.HoldingCombatPerch`;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock):
  - the exec step;
  - `WillowAIPawn`'s vtable slot (the vtable found from its registration's constructor);
  - the body that slot holds;
- the installed packages: `tools/ghidra/class_layout.py` for `WillowAIPawn`, `Actor` and `ProjectileDefinition`;
- a local probe of the installed slice gate (section 4), never pushed.

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/dcp/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here
is UNVERIFIED** in the running game.

## 1. What the native does

`native final function DropCombatPerch()` is declared on `WillowAIPawn`. Its exec step reads no arguments and calls one
pawn virtual. For `WillowAIPawn` that body does this:
1. **The guard:** it acts only when all three hold, tested in this order:
   - the pawn's `Role` is `ROLE_Authority` (3);
   - its `PerchProjectile` (a `ProjectileDefinition`) is set;
   - its `bPerchAttached` is set (the flag `HoldingCombatPerch` returns in script).

   Otherwise it returns at once, with **nothing written, loaded or drawn**.
2. **When it acts** (not on link 9's path; read only to its edge):
   - it spawns an actor of the definition's `ProjectileClass` at the pawn's `Location` and `Rotation`;
   - when that spawn gives an actor:
     - it clears two leading members of `PerchState` (its `AttachmentName`);
     - it calls a pawn virtual with false (by its place, the perch mesh's detach; not read);
     - it hands the definition to the new actor through one of the actor's virtuals;
     - it gives the actor a velocity of 100 along the pawn's facing;
     - it calls one more pawn virtual with the actor (not read);
     - it sets `PerchProjectile` to None.

**Field names.** They come from the packages' layouts:
- `Role` and `Location` need no shift;
- `bPerchAttached` takes the known 4-byte `WillowPawn` shortfall (NATIVE_PLAYER_MOVEMENT.md, line 118);
- `PerchProjectile` and `PerchState` take a 16-byte shift. That shift is consistent with `RBState`, a
  `RigidBodyState` holding a quaternion, being aligned more strictly than the layout tool assumes.

The structural oracle for `PerchProjectile`: the spawned class is read at the place where `ProjectileDefinition`
declares `ProjectileClass`. By fit, UNVERIFIED. The port reads every field by name (section 3), never by offset.

**The script callers:**
- `LiftActionSkill.BeginLifting`, on the lifted pawn (link 9);
- `WillowAIPawn.Died`;
- `WillowAIPawn.Staggered.BeginState`;
- `Behavior_CombatPerchThrow.ApplyBehaviorToContext`.

## 2. Link 9's call

`LiftActionSkill.BeginLifting` first clears the lifted pawn's `bCanUse`, then calls
`WillowAIPawn(LiftedPawn).DropCombatPerch()`, then records `LiftStartLocation`. On link 9 the lifted pawn is the freshly
spawned target. The probe read it as `Role` 3, `PerchProjectile` None and `bPerchAttached` false, so the guard fails
and the native does nothing.

## 3. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.**
- **Bind in Off** the key `WillowAIPawn.DropCombatPerch()` in `ow-combat`'s pawn natives (`hit/pools.rs`), beside
  `IsStaggered` (#611), through the keep-existing `bind_fn`.
- **Reference** keeps the stub policy and is unchanged.

**Preflight** (write-free, load-free, RNG-free), in this order; the first failure is the answer:
1. **The receiver** is set, and its class is `WillowAIPawn` or a child (`by_path` and `is_child_of` only). Otherwise a
   `VmError::Script`.
2. **D1, an unresident receiver**, `NotYet { NotRead }`, record this note, with two texts:
   - an unloaded handle: "WillowAIPawn.DropCombatPerch: the pawn <path> is an unloaded handle (a load is a write)";
   - a slotless stand-in: "WillowAIPawn.DropCombatPerch: the pawn <path> is a stand-in with no properties (an object
     the store cannot load)".
3. **D2, a native descendant** of `WillowAIPawn` (any class `research/native_by_class.json` lists under it, today
   `WillowAICranePawn`; its slot was not read): `NotYet { NotRead }`, "WillowAIPawn.DropCombatPerch: the perch rule
   of <class> (its virtual was not read)". Script-only children take `WillowAIPawn`'s rule.
4. **The declarations**, by name: `Role` a byte, `PerchProjectile` an object, and `bPerchAttached` a bool. A missing
   field or a wrong kind is a `VmError::Script` naming it.
5. **The guard**, in section 1's order: `Role` not 3, or `PerchProjectile` None, or `bPerchAttached` clear: answer
   with **nothing written**.
6. **D3, the drop itself** (the guard holds): `NotYet { NotRead }`, "WillowAIPawn.DropCombatPerch: the perch
   projectile's spawn and launch (not read)", before any write. Its engine steps were read only to their edge, and two
   of its virtuals not at all.

`Role` is compared as the raw byte 3, `ROLE_Authority` in the stock enum (UNVERIFIED). That enum is exempt from the
tests' reordering rule.

**The write:** none on the answered path. D3 refuses before anything, so no refusal is marked "stopped after partial
writes: " from inside the native.

**Tests** (synthetic classes built with `package()` / `tag()`, every value invented):
1. the slice shape: `Role` 3, `PerchProjectile` None and `bPerchAttached` clear give nothing, with #518's three
   counters unchanged;
2. each guard failing alone answers nothing: `Role` 2 with the other two set; the projectile None with the flag set;
   the flag clear with the projectile set;
3. all three holding: D3, nothing written;
4. a script-only child of `WillowAIPawn` takes the rule; an invented native child gives D2;
5. a receiver outside `WillowAIPawn`: `VmError::Script`;
6. an unloaded handle and a slotless stand-in: D1, each with its own text, nothing loaded;
7. a missing `PerchProjectile` and a non-bool `bPerchAttached`: `VmError::Script` each;
8. a key bound first keeps its binding; under `ParityMode::Reference` nothing new is bound.

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
- `IsStaggered` as #611 proposes;
- **this lane's:** `DropCombatPerch` as section 3's guard (it would have refused at D3), logged.

**Installed Off `--slice-gate`, strict:** ok 2, notyet 9, error 3 of 14.
- The one call was on the target `WillowAIPawn`: `Role` 3, `PerchProjectile` None and `bPerchAttached` false, so
  nothing was done.
- **Link 9** passes 1 of its 2 driver steps (`spawn`) and stops at `cast` with an `error`: the unbound native
  `Actor.Trace(vector,vector,vector,vector,bool,vector,tracehitinfo,int,bool,object)`.

So link 9's next stop is `Actor.Trace`.

## Open questions

1. **Not read:**
   - the drop's two pawn virtuals and the actor's two virtuals (section 1, step 2);
   - the slots of `WillowAICranePawn` (D2) and of `WillowPawn`'s other C++ children (the native is declared on
     `WillowAIPawn` only).
2. **UNVERIFIED:** the field fits under the 4-byte and 16-byte shifts; the raw `Role` value of `ROLE_Authority`.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Phaselock an enemy that is not on a perch: expect `PerchProjectile` None before and after, and no projectile
   spawned.
2. Phaselock a perched thrower (holding a rock): expect a projectile actor of the definition's `ProjectileClass` at
   the pawn, `PerchProjectile` None and `bPerchAttached` cleared afterwards.
