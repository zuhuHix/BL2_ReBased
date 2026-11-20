# `Actor.Trace` on link 9 (draft, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-ACTOR-TRACE (decision
`docs/decisions/2026-11/2026-11-19-NNN-actor-trace-on-link-9.md`, number pending). Analyst only: a different lane
implements it (clean room).

**Sources:**
- the records:
  - `tools/native_index_find.py Trace` finds no `Actor.Trace` row;
  - NATIVE_GO_DOWN_PATH.md section 4.5 left the native unread and answered it `LayerAbsent`;
  - NATIVE_SPAWN_PLACEMENT.md section 7 reads the world's trace filters;
- the code:
  - `ow-combat` `go_down.rs` binds `Engine.Actor.Trace` as `NotYet { LayerAbsent }`, "Actor.Trace: the world's
    collision", in the go-down worlds only;
  - `ow-world` `placement.rs` holds the collision service (`SpawnCollision`: `box_test`, `box_sweep`, `point_sweep`,
    `actor_bounds`) and its no-level stand-in `NoLevel`;
  - `ow-world` `trace_filter.rs` holds the trace flag bits and the per-actor filters;
- the script, read with `research/script_disasm.py`: `LiftActionSkill.BeginLifting` and `GetLiftCheckLocation`;
- `Engine.upk`'s export table, for the parameter names;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock): the
  exec step, and one `WillowAIPawn` vtable slot;
- a local probe of the installed slice gate (section 4), never pushed.

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/trc/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here
is UNVERIFIED** in the running game.

## 1. What the native does

`native(277) final function Actor Trace(out vector HitLocation, out vector HitNormal, vector TraceEnd, optional vector
TraceStart, optional bool bTraceActors, optional vector Extent, optional out TraceHitInfo HitInfo, optional int
ExtraTraceFlags, optional bool bTraceBulletListeners, optional Object BulletListenerSource)`.

**The defaults:**
- `TraceStart` omitted: the actor's `Location`;
- `bTraceActors` omitted: the actor's own `bCollideActors`;
- `Extent` omitted: zero.

**The query.** One world single-line check from `TraceStart` to `TraceEnd` with `Extent`. The trace flags are built
from the `trace_filter.rs` bits:
- `bTraceActors` false: movers, the level BSP, world geometry and terrain. That is the same word as `trace_filter`'s
  `SWEEP`;
- `bTraceActors` true: pawns, movers, the level BSP, volumes, other actors, world geometry and terrain, plus
  projectile targets. When `ExtraTraceFlags` has the blocking bit (8), the projectile-targets bit gives way to
  blocking-only;
- a `HitInfo` argument adds the material request;
- the other `ExtraTraceFlags` bits each add one more world bit. That mapping was read, but not what each added bit
  means.

**The source actor.** The actor itself, unless a virtual of the actor names another object to trace as. For
`WillowAIPawn` that virtual is the shared stub that answers none, so a `WillowAIPawn` traces as itself. It was not read
for other classes.

**The answers:**
- the hit actor, or None;
- `HitLocation` and `HitNormal` from the hit record, which starts zeroed, so a miss gives zero vectors;
- `HitInfo`, when given, from the hit's material, item, bone and component;
- bullet listeners are told when `bTraceBulletListeners` is set or the matching extra bit is.

It writes nothing in the world, loads nothing and draws nothing of its own.

## 2. Link 9's call

`LiftActionSkill.BeginLifting`, on the lifted target:
1. `CheckLoc` is `GetLiftCheckLocation()`: the target's `Location`, adjusted for an interpolating or spawn-lerping
   pawn.
2. **First trace:** `Target.Trace(HitLocation, HitNormal, CheckLoc + (0, 0, 1) × HeightFromGround, CheckLoc, true,
   (1, 1, 1), , 8)`. That is a box of extent 1 traced through actors, with the blocking bit. In the probe the end lay
   200 units straight below the start, so `HeightFromGround` was negative and the trace looks for the floor.
3. **On a hit:** `LiftEndLocation` is raised from the hit location by the target's collision height and
   `HeightFromGround`. A **second trace** then runs from `LiftEndLocation` back to `CheckLoc` with the same
   arguments, and on a hit `LiftEndLocation` is set from that hit lowered by the collision height.
4. **On a miss** (the headless world has no floor): `LiftEndLocation` stays `CheckLoc` and the script skips to the
   light attach.

## 3. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.**
- **Bind in Off** the key `Actor.Trace(vector,vector,vector,vector,bool,vector,tracehitinfo,int,bool,object)`, as one
  shared function in `ow-world` with two bindings: the skill worlds (`ow-combat`, keep-existing `bind_fn`) and
  `go_down.rs`. Once the shared function exists, it replaces `go_down.rs`'s bare `LayerAbsent` binding.
- **Reference** keeps the stub policy and is unchanged.

**Preflight** (write-free, load-free, RNG-free), in this order; the first failure is the answer:
1. **The receiver** is set and resident. An unloaded handle or a slotless stand-in is T1, `NotYet { NotRead }`,
   "Actor.Trace: the actor <path> is not loaded (a load is a write)".
2. **The arguments:** `TraceEnd`, `TraceStart` and `Extent` are vectors (`Vector` structs), `bTraceActors` a bool, and
   `ExtraTraceFlags` an int or a byte. Another shape is a `VmError::Script`. Omitted ones take section 1's defaults.
3. **T2, an unread option:**
   - a `HitInfo` argument;
   - `bTraceBulletListeners` set;
   - a `BulletListenerSource`;
   - any `ExtraTraceFlags` bit other than 8.

   Each is `NotYet { NotRead }`, "Actor.Trace: <option> (its effect on the query was not read)".
4. **T3, the source actor:** the receiver's class must be one whose owner virtual was read, today `WillowAIPawn` and
   its script-only children (native descendants as #611 names them refuse). Otherwise `NotYet { NotRead }`,
   "Actor.Trace: the trace source of <class> (its virtual was not read)".
5. **T4, the collision world:** no collision service (`placement::collision_model` None) is `NotYet { LayerAbsent }`,
   "Actor.Trace: the world's collision", layer "the world's collision", go_down's text.
6. **The level part:** the service's `box_sweep(Extent, TraceStart, TraceEnd)`, or `point_sweep` for a zero extent.
   It reads only.
7. **The actor part,** only with `bTraceActors`:
   - the walk over live actors with `trace_filter`'s per-actor filters, for section 1's flag word, leaving out the
     source actor;
   - a candidate the filters keep whose primitive bounds the service cannot give (`actor_bounds` None) is T5,
     `NotYet { LayerAbsent }`, "Actor.Trace: the colliding components' bounds", layer "component bounds (the
     headless world has none)" (the pickup targeting's layer text). So a headless world answers only when no kept
     actor is near the line.

   Whether a candidate is near the line before its bounds are asked is the implementer's to settle from the existing
   walk (open question 2).

**The answer:** the nearest of the level and actor hits, with its actor (None for the level) and its location and
normal. A miss answers None with both vectors zero. The out arguments are checked to exist and be declared `out`
before either is written, as #595 does for `GetBoundingCylinder`. They are the only writes, so no refusal is ever
marked "stopped after partial writes: " from inside the native.

**Tests** (synthetic classes built with `package()` / `tag()`, every value invented; a test collision service that
answers what the test sets):
1. link 9's shape on a no-level world with no other actors: None, both outs zero;
2. a level hit from the service: its location and normal, with None as the actor;
3. an actor hit nearer than the level hit: that actor; the source actor itself is never hit;
4. `bTraceActors` false: actors are not asked;
5. omitted `TraceStart` (from `Location`), `bTraceActors` (from `bCollideActors`) and `Extent` (zero, a point sweep);
6. T1 to T5, each before any write, with #518's three counters unchanged;
7. both bindings in either order, keep-existing; under `ParityMode::Reference` nothing new is bound.

## 4. The probe (local, never pushed)

A local tree at the resolve-context lane's probe commit (d51aa46a, with its uncommitted patch). It merges
`origin/main` 3ad58b0 with #579, #586, #564, #548, #555, #560, #523 (counter conflicts on main's side, except its noun
resolver), #595, #601 and `fix/rust-gearboxmind-queryinterface`.

**Probe-only stand-ins:**
- `GearboxMind.GetAIComponent` bound where link 9 runs;
- the AI components' `QueryInterface` answered by main's component rule;
- the mind's AI component `Owner` set to the mind;
- `WillowAIComponent.PhaseLockOnHold` answering false;
- `ResolveContext` and `AddAttributeModifier` (#604, #607);
- `IsStaggered` (#611) and `DropCombatPerch` (#620);
- **this lane's:** `Actor.Trace` answering no hit, with its arguments logged.

**Installed Off `--slice-gate`, strict:** ok 2, notyet 9, error 3 of 14.
- The one trace ran on the target `WillowAIPawn`: from (0, 0, 0) to (0, 0, −200), `bTraceActors` true, extent
  (1, 1, 1), extra flags 8, no `HitInfo` and no bullet listeners. Its miss skipped the second trace.
- **Link 9** passes 1 of its 2 driver steps (`spawn`) and stops at `cast` with an `error`: the unbound native
  `Actor.AttachComponent(object)`. That is the script's attach of the phaselock light to the target. It is bound only
  in the pickup worlds (#511), where the empty scene layer is installed.

So link 9's next stop is `Actor.AttachComponent`, which already has a record (NATIVE_ATTACH_MESH_TO_PICKUP.md part 1)
and a binding; the question is only where it is bound.

## Open questions

1. **Not read:**
   - the world check's own body (how level and actor hits are merged, and what each added extra bit means);
   - the source virtual for classes other than `WillowAIPawn`;
   - the `HitInfo` fill;
   - the bullet listeners.
2. **To settle (implementer):** whether `trace_filter`'s walk can rule out a far actor before asking the service for
   its bounds. If it cannot, T5 stops link 9 in the headless world whenever another colliding actor exists.
3. **UNVERIFIED:** that the blocking bit's effect is the swap in section 1. The bit names are `trace_filter.rs`'s,
   which NATIVE_SPAWN_PLACEMENT.md section 7 recorded.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `Actor.Trace` during a phaselock on a grounded enemy and record its arguments and answer. Expect a downward
   trace that hits the floor, then a second trace back.
2. Trace from a point in empty air with no geometry below. Expect None and zero vectors.
