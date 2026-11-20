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
  - `ow-combat` `go_down.rs` binds `Engine.Actor.Trace` as a layer-absent refusal, "Actor.Trace: the world's
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

**What was read and what was not.** The exec step was read: it fills the defaults, builds the flag word, calls the
world check and writes the answers. In that exec work nothing is written in the world, loaded or drawn. The world
check's own body and the bullet-listener path were not read, so the note makes no write, load or draw claim for them.

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

**Every rule below is UNVERIFIED.** The existing trace filter cannot answer link 9's query as it stands (section 3.4),
so this answer is narrowed: **Off answers only a miss it can prove, and refuses everything else** with a typed
refusal. A source-aware filter and an actor-hit query are new work, listed in section 3.5, not part of this answer.

### 3.1 Where it is bound

- **One implementation**, a function in `ow-world` keyed `Engine.Actor.Trace` (the native key
  `Actor.Trace(vector,vector,vector,vector,bool,vector,tracehitinfo,int,bool,object)`).
- **Two binding sites**, each checking `ParityMode::Off` before it registers:
  - **the Phaselock pack set:** `ow_skills::action_skill::Pack::register`. `phaselock_world` (`slice_gate.rs`)
    registers only the Core, skill-tree and action-skill packs, never `ow_combat::hit::Pack`, so this is the site that
    reaches link 9;
  - **the go-down worlds:** `go_down.rs` (the `bind_fn` of `Engine.Actor.Trace`). Its bare layer-absent closure is
    **replaced at its source** by the shared function. Leaving it in place is not enough: in a world with both packs,
    whichever binds first wins under keep-existing, so a world where the combat pack binds first would still get the
    old refusal.
- **Reference** keeps the stub policy and the go-down refusal, and is unchanged.

### 3.2 Preflight

The preflight is write-free, load-free and RNG-free. It is resident-only for the receiver, for every candidate actor
and for every owner it reads, and it uses only the existing class and name lookups (no interning, no load). Its checks
run in this order, and the first failure is the answer:

1. **T1, the receiver** is set and resident. An unloaded handle or a slotless stand-in is a not-read refusal,
   "Actor.Trace: the actor <path> is not loaded (a load is a write)".
2. **The arguments:** `TraceEnd`, `TraceStart` and `Extent` are vectors (`Vector` structs), `bTraceActors` a bool, and
   `ExtraTraceFlags` an int or a byte. Another shape is a `VmError::Script`. Omitted ones take section 1's defaults.
3. **T2, an unread option**, each a not-read refusal, "Actor.Trace: <option> (its effect on the query was not read)":
   - a `HitInfo` argument;
   - `bTraceBulletListeners` set;
   - a `BulletListenerSource`;
   - any `ExtraTraceFlags` bit other than 8.
4. **T3, the source actor.** The receiver's class must be one whose owner virtual was read: today `WillowAIPawn` and
   its script-only children (the native descendants #611 names refuse). Otherwise a not-read refusal, "Actor.Trace:
   the trace source of <class> (its virtual was not read)".
5. **T4, the collision world.** No collision service (`placement::collision_model` None) is a layer-absent refusal,
   "Actor.Trace: the world's collision", layer "the world's collision" (go_down's text).
6. **The out destinations.** `HitLocation` and `HitNormal` are checked to exist and be declared `out`, and both answer
   values are prepared, **before the first `set_out`**. `NativeCall::set_out` writes through the caller's alias at
   once (`api.rs`), so a failure between the two writes would leave one written. With the checks first, the two
   `set_out` calls are the only writes and nothing can fail between them. No refusal is marked "stopped after partial
   writes" from inside the native.

### 3.3 The query, driven from the flag word

The flag word from section 1 decides what is asked. With `bTraceActors` false the word still covers movers, world
geometry and terrain, which are actors, so **the actor walk runs for both values of `bTraceActors`**. The collision
service covers the level's BSP only.

1. **The level part:** the service's `box_sweep(Extent, TraceStart, TraceEnd)`, or `point_sweep` for a zero extent.
   It reads only.
   - `Sweep::Unsupported`: a not-read refusal, "Actor.Trace: the level sweep".
   - `Sweep::Hit`: **T6**, a not-read refusal, "Actor.Trace: the actor answered for a level hit (not read)". The return
     value for a BSP hit was not read. Answering None would look like a miss to `BeginLifting`, whose hit branch raises
     the target and traces again. So Off does not answer a floor hit until a supported hit identity is read.
   - `Sweep::Clear`: go on.
2. **The actor part:** every live actor's primitive components, except the source actor's own (step 3 of this list),
   in the walk order `placement.rs` already uses. For each primitive:
   - its owner, and the primitive itself, must be resident (a not-read refusal as T1, naming the object);
   - **missing bounds** (`actor_bounds` None) is **T5**, a layer-absent refusal, "Actor.Trace: the colliding
     components' bounds", layer "component bounds (the headless world has none)" (the pickup targeting's layer text);
   - **bounds known to be disjoint** from the swept box (the segment's box grown by `Extent`): the primitive cannot be
     hit, and it is left out **without reading any filter field**;
   - **bounds that meet the swept box:** **T7**, a not-read refusal naming the first unread rule that would have to
     decide it: the terrain handling (`trace_filter::TERRAIN_HANDLING`) for a terrain owner, then the owner filters
     with a source actor, then the collision hash's admission (`trace_filter::HASH_ADMISSION`). The walk does not try
     to decide a near primitive, so there is no nearer-actor test.
3. **The source actor's own primitives are left out** of the walk. The exec passes the source to the world check, but
   the check's body was not read, so this exclusion is UNVERIFIED (open question 1).

**The answer:** with no refusal, the trace is a **provable miss**: None, with both out vectors zero.

### 3.4 Why the existing filter cannot answer the query

- `Query::flags()` gives only the spawn's masks;
- the owner filters assume a query with no source actor (`trace_filter.rs`, the owner and row filters);
- `Candidate` has only `NotTested` and `Unsupported`, so a primitive is never answered "tested";
- the placement walk records no actor hits.

### 3.5 Not in this answer (new work, for a later note or lane)

- a source-aware filter (the owner filters with a source actor, and blocking-only through the source's test);
- an actor-hit query that answers the hit actor, location and normal;
- the hit identity for a level hit (T6);
- the comparator between a BSP hit and an actor hit (`trace_filter::BSP_ACTOR_COMPARATOR`). A mixed BSP and actor hit
  takes a typed refusal naming it until that comparator is read. Under this answer the BSP hit refuses first (T6).

### 3.6 Tests

The tests use synthetic classes built with `package()` / `tag()`, every value invented, and a test collision service
that answers what the test sets:

1. link 9's shape on a no-level world with no other actors: None, both outs zero;
2. a level hit from the service: T6, with no out written;
3. a primitive whose bounds meet the swept box: T7; the source actor's own primitives never refuse;
4. `bTraceActors` false: the walk still runs from the flag word, so a meeting primitive still gives T7 (no false
   miss);
5. a primitive with **known-disjoint bounds and unloaded filter fields**: left out, so the answer is the miss;
6. outcome order: missing bounds gives T5; a meeting primitive gives T7; `Sweep::Unsupported` gives its not-read
   refusal;
7. omitted `TraceStart` (from `Location`), `bTraceActors` (from `bCollideActors`) and `Extent` (zero, a point sweep);
8. T1 to T7, each before any write, with #518's three counters unchanged; an unresident candidate and an unresident
   owner each refuse;
9. the binding sites:
   - the Phaselock-only pack set (Core, skill tree, action skill) gets the shared function;
   - both combined orders (the combat pack first, and the action-skill pack first) get the shared function, never the
     old refusal;
   - under `ParityMode::Reference` neither site binds anything new.

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

So **under the probe's stand-ins**, link 9's next stop is `Actor.AttachComponent`. It already has a record
(NATIVE_ATTACH_MESH_TO_PICKUP.md part 1) and a binding, so the question is only where it is bound. Without those
stand-ins the stop would be earlier. Note too that the probe's trace stand-in answered a plain miss, which section 3's
answer gives only when every primitive is known to be clear.

## Open questions

1. **Not read:**
   - the world check's own body (how level and actor hits are merged, what actor a level hit returns, whether the
     source actor's primitives are skipped, and what each added extra bit means);
   - the source virtual for classes other than `WillowAIPawn`;
   - the `HitInfo` fill;
   - the bullet listeners.
2. **The headless world:** `NoLevel` holds no bounds, so T5 stops link 9 whenever another live actor has a
   primitive. Whether the link 9 world has one was not checked.
3. **UNVERIFIED:** that the blocking bit's effect is the swap in section 1. The bit names are `trace_filter.rs`'s,
   which NATIVE_SPAWN_PLACEMENT.md section 7 recorded.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `Actor.Trace` during a phaselock on a grounded enemy and record its arguments and answer. Expect a downward
   trace that hits the floor, then a second trace back.
2. Trace from a point in empty air with no geometry below. Expect None and zero vectors.
