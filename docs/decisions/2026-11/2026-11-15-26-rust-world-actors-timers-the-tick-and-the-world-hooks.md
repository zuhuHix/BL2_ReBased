## 2026-11-15: Rust world: actors, timers, the tick and the world hooks

AI-assisted (Claude Code). Lane W5-WORLD of the Rust port fills `crates/ow-world/src/{world,query,actors,timers}.rs`
and adds `crates/ow-world/src/world/tests.rs` (architecture sections 3.8, 3.9, N9, D13). No parsing behaviour changes.
The rules come from `docs/verification/NATIVE_ENGINE_CORE.md` ("Actor timers", "Spawning and lifetime",
"Iterators", "Lookups") and NATIVE_ATTRIBUTES.md section 1. None was checked in the running game; there was no
session in this lane.

- **Seam edit outside the lane's files** (granted: "the `AllActors`/`DynamicActors` iterator source"), one line per
  file:
  - `crates/ow-vm/src/natives_core.rs`: an additive `ActorWalk` enum (`#[non_exhaustive]`), `ActorWalkFn` and an
    `ActorSource` service type; the two iterators ask that service for their candidates when it exists and keep the
    C++ list (the heap's live instances) when it does not. No frozen contract changed. With a source, a None
    BaseClass keeps every candidate (the record's "None means Actor"), where the C++ list matches nothing.

  No `WorldHooks` implementor needed an `ow-vm` edit, and `EvaluateInitializationData` is registered from
  `ow-world`'s own `Pack`, so the other two grants were not used.
- **Shipped shapes** (all public structs and enums that may grow are `#[non_exhaustive]`).
  - `world::World { pub vm, .. }`. `World::new(vm, BalanceState, &[&dyn NativePack])` installs the services below
    on the VM and registers `world::Pack`, then the given packs in order. `World::tick(real_dt) -> Step`.
    `set_state_step` and `set_physics_step` take an `ActorStep` (`Rc<dyn Fn(&mut Vm, ObjRef, f32)>`), each set once.
    `WorldError { Vm, Clock, BadDelta }` has `source()`.
  - Services, each reachable from `&mut Vm`: `query::WorldQuery` (globals, game info, engine-query handlers keyed by
    `QueryKind`), `world::BalanceState { snapshot, region_stages }`, `actors::Actors` (the level list),
    `timers::Timers`, and the `ow_vm::natives_core::ActorSource` over the actor list.
  - Free functions for packs above: `actors::{spawn, destroy, set_owner, send_event, is_destroyed, actors_of}`,
    `timers::{update_pass, timers_of}`, `query::install`.
  - `world::Pack` registers `Actor.Spawn`, `Actor.Destroy`, the nine timer natives,
    `AttributeInitializationDefinition.EvaluateInitializationData` and `WorldInfo.`, `SequenceObject.` and
    `BehaviorBase.GetWorldInfo`. They find the services at call time; on a VM without a world they are `NotYet`.
- **The world hooks.** `globals` and `game_info` answer the objects the map load set, else `NotYet { LayerAbsent }`
  (never None: the running game always has both). An engine query goes to the handler registered for its kind
  (W5-POOLS, W5-EQUIP and W6-SKILLS register theirs); a kind with no handler is `NotYet { LayerAbsent }` naming that
  lane. The world answers none itself: how the game finds the player controller from a source, or a pawn's attribute
  context redirect, is not read.
- **Timers** (the record's "Actor timers", as the lane row lists them). Entries keyed by (name, object), the object
  defaulting to the actor and the name to `Timer`; replace-not-add; rate 0 stores only the rate; `SetTimer` clears
  the pause and resets `Count`; static actors ignore it; negative rates stored. The clear natives only zero the rate.
  The update pass: `Count += TimerTimeDilation * delta` unless paused; walk by index with the length re-read; rate 0
  or a gone target removed; due when `Count > Rate` strictly; a loop fires `floor(Count / Rate)` times with `Count`
  reduced first; the function found by name in the target's state, then class chain (missing: removed silently);
  zeroed parameters; after each callback the pass stops on actor destruction, a cleared entry is removed, a `Count`
  of exactly 0 keeps a non-loop entry; a due non-loop entry is removed afterwards. **Rust-only refusals** (`NotYet {
  NotRead }`, the record leaves them untraced): a timer with a negative rate (refused up front, due or not), and a
  loop due more than `i32::MAX` times in one pass. Both are refused before the pass changes anything (review r1 of
  PR #325), for each entry whose target is still there (the loop case judged on what step 1 would make of it); an
  entry a callback appends or re-sets meets them at its own check.
  The entries live in the `Timers` service, not in the heap's `Timers` property: script never reads that array.
  A destroyed actor's entries are dropped by its destroy.
- **The tick.** `WorldInfo.TimeDilation` is copied into the clock first; a value the clock refuses stops the tick with
  `WorldError::Clock`, the clock keeping its last good value (the lane decision `ow_core::sim` left open). A bad
  delta is `WorldError::BadDelta`, not the clock's panic. `TimeSeconds` and `RealTimeSeconds` are written from the
  clock. Actors are ticked by tick group 0 to 4. Each actor's group is read once per tick, before the clock
  advances (review r1): an actor gets at most one tick and keeps that group if its `TickGroup` changes meanwhile,
  and an actor spawned during a tick is first ticked in the next one. The record sets a spawned actor's `bTicked`
  from the level's tick state (spawn step 5); reading that as "waits for the next tick" is UNVERIFIED, as is the
  order across actors. The unread alternative is spawn step 11's list of newly spawned actors: in public UE3 that
  list ticks actors spawned during a tick in the same frame (open item). One actor's tick is the record's authoritative path: `Tick(DeltaTime)`, the state-code step,
  the timer pass, `LifeSpan` (destroyed at 0.0001 or below, and the tick ends), physics when `Physics` is not
  `PHYS_None` and `Role` is not 2 (simulated proxy). Physics of an actor with a `Base` is `NotYet { NotRead }`:
  whether the base is being moved by a rigid-body step is the record's condition and the world cannot see it, so
  the physics hook is not handed that rule. `Role` 0 gets only a physics step in the four modes the record names;
  `RemoteRole` 2 is `NotYet { NotRead }`. **Note for W6:** nothing in this world sets `RemoteRole` to none. The
  record says the single-player game keeps local actors at remote role none, but where it does so was not read.
  Until then, a class whose defaults carry `RemoteRole` 2 stops the tick with this `NotYet`. The
  state-code step (W5-STATES) and the physics step (the host) are hooks; without one, an actor that needs it is
  `NotYet { LayerAbsent }`. Physics modes are compared by their enum names, read from the property's enum.
- **Spawn and destroy**, in the record's order, with these choices:
  - Spawn returns None for the record's refusals it can evaluate. A template other than the class default object is
    `NotYet` (how instanced subobjects are copied is not read). Everything that needs collision is `NotYet {
    LayerAbsent }`, checked on the class defaults before anything is made: the placement spot search and, after play
    began, the encroachment check or relocation of an actor with `bCollideActors` and the base search (which
    review r1 found ran only after the events; now checked first too). An actor that does not collide with actors is
    taken to pass the encroachment check (UNVERIFIED: its internals are not read). The pre-checks are conservative:
    they refuse with `NotYet` even when the begin-play events would have made the check unnecessary. The begin-play events can change
    those fields, so `bCollideActors` is checked again after `PreBeginPlay` and the base search after
    `SetInitialState`; a `NotYet` from those post-event checks leaves the actor made, listed and past its events, and
    the run is meant to stop there. Before play begins no begin-play event, encroachment check or base search runs
    (the later begin-play pass is not read). The abstract and "not instantiable" class-flag
    refusals are **not applied**: the class table holds no class flags and the record does not identify the bits
    (open item).
  - Destroy returns the record's results. It is `NotYet`, before anything changes, for a `Controller`, an actor with
    `GeneratedEvents` (W5-KISMET), an actor in a state (`EndState`, W5-STATES), an actor with a `Base`, attached
    actors based on it or live `Touching` entries (unbase and untouch routines not read), and a nested `Destroy` of
    an actor whose destroy routine is running (Rust-only: the record does not say what the game does, and a
    `Destroyed()` that destroys itself would otherwise recurse). The actor is marked destroyed, never freed here.
  - `SetOwner` refuses an ownership loop; a pre-existing owner loop that does not pass through the actor is a typed
    error (the game's walk would not end).
  - Script events are sent when the object sees a function of that name. The probe mask (`Disable`) belongs to the
    state frame of W5-STATES and is not applied yet.
- **Iterators.** `AllActors` walks the level list from index 0 (the `WorldInfo` included), `DynamicActors` from the
  first-dynamic index skipping `bStatic` actors, both skipping destroyed actors, then the class and interface filters
  of W4-INTERP2. One persistent level only. The first-dynamic index is stated by the map load (how the game computes
  it is not in the record). The list is taken when the loop starts (the `foreach` contract takes a list).
  - **Known divergence, kept by hub decision; a live walk is a follow-up.** The record reads the game's walk from
    the executable as live: an actor added during the body is visited, and removing an earlier actor shifts the scan
    so the next actor is skipped. The snapshot reproduces neither: actors spawned during the body are missed, and no
    actor is skipped by a removal.
- **`EvaluateInitializationData`** calls `ow-attr`'s evaluator with both context sources. The C++ refuses an override
  source; the Rust follows the record, which tries it first.
- **Where the output differs from the C++, and why.** The C++ binary has no world command (the mover adapter's
  scoped `SetTimer`/`ClearTimer` sits behind `--mover-event`, W5-DIALOG's), so `tests/rust_parity.d/world.txt` holds
  no line. Differences in behaviour: the iterators with a world source (None BaseClass, level list), the override
  context source above, and every rule the C++ never had.
- **Verified (automated, local, pc-hummer).** 32 `ow-world` unit tests on an invented `Engine.upk` built with the
  `ScriptToy` helpers, whose script event functions call a test native that records the order: each timer rule of
  the row, the tick order and groups, `Role` 0, world time and dilation, a replay of one delta list twice (identical
  clock and timer counts), the hooks, spawn and destroy orders and refusals, `SetOwner`, both iterators through the
  `ActorSource` seam, `EvaluateInitializationData` and `GetWorldInfo`; after review r1 also the base-search
  refusal before anything is made, one tick per actor per tick, the timer refusals before any change, the physics
  conditions and the dropped timer entries of a destroyed actor. The `ow-vm` tests are unchanged and pass.
  **Native keys against the installed game:** each of the 15 keys `world::Pack` registers equals the key the Rust
  loader derives from the installed `Engine.upk` for that function (`--run <function>` under `LogAndZero` logs the
  key of the unbound native; output kept under `local/`). That checks the binding, not the behaviour.
- **Open items.**
  - The abstract and not-instantiable spawn refusals (class flags in the class table; the second bit's identity).
  - Live iterator walks: the `foreach` contract (`IteratorResult` takes a list) cannot follow the game's live walk
    (above). The hub kept the list for now; a live walk is a contract follow-up.
  - The probe mask, `EndState` on destroy and the state-code step: W5-STATES. Kismet destroyed events: W5-KISMET.
    Collision, physics and the unbase and untouch routines: the host track.
  - Where the world's `TimeDilation` and an actor's `CustomTimeDilation` enter the tick delta (NATIVE_ENGINE_CORE.md,
    Open); the order across actors.
  - Actors spawned during a tick: this world first ticks them in the next tick (the `bTicked` reading above). Spawn
    step 11's newly-spawned list is the unread alternative; in public UE3 it ticks them in the same frame. Read
    which one the game does.
  - Where the game sets a local actor's `RemoteRole` to none (above).
- **UNVERIFIED.** Every rule above, as read from the executable by the record, none checked in the running game; the
  delta every timer and tick gets in particular.
