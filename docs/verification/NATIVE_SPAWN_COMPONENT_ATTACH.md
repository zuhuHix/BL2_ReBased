# The spawn's component attach: how a spawned actor's components get their `Owner` (link 9, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-SPAWN-COMPONENT-OWNER, decision
`docs/decisions/2026-11/2026-11-19-215-spawn-attaches-the-actor-s-components.md`. Read from the game executable with
Ghidra (`tools/ghidra/run.ps1 -ReadOnly` and a local read-only table reader, under the machine lock). Field names come
from the cooked packages (`tools/ghidra/class_layout.py`). It is written in our own words: no decompiler output,
pseudo-code, addresses, offsets or listing structure is reproduced here, and raw output stays under ignored
`local/analysis/`. **Every rule is UNVERIFIED in the running game.** The last section is the bl-sdk check that would
confirm it.

**Clean room.** The lane that wrote this note implements none of it. A fresh implementer codes the rules from this
note alone.

**Where it fits.** This is a new file. It fills a gap in NATIVE_ENGINE_CORE.md's spawn routine: between its steps 5
and 6 there is one more engine call, which that record does not list (a pointer was added there). It builds on
NATIVE_ATTACH_MESH_TO_PICKUP.md section 3.4 (`Actor.AttachComponent`) and the hub's empty-scene ruling there. It uses
the same per-component attach, and the same layer and answers, and contradicts neither.

**What it answers.** NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md section 3, link 9's first stop once #600 is in. The
port's spawn never runs the component attach step, so a spawned mind's `WillowAIComponent` keeps `Owner` None, and
under #600's ruling that component's `QueryInterface` refuses with NotRead.

## Summary

| Question | Answer | Confidence | Status |
|---|---|---|---|
| Which step sets a spawned actor's components' `Owner` | the spawn's component update, an engine call right after `Location` and `Rotation` are set, before `PhysicsVolume`, the owner and `Instigator`, and so before every script event | high | UNVERIFIED |
| Whether it needs play to have begun | no: it runs on every spawn | high | UNVERIFIED |
| Which components | each non-null entry of the actor's `Components` array, in array order (the instanced copies of the template's components) | high | UNVERIFIED |
| What it writes, per component | `bNeedsReattach` and `bNeedsUpdateTransform` false, `Scene` the world's scene, `Owner` the actor, the transform; then the component's own attach entry (for an AI component: `bAttached` true and the component appended to the owner's `AllComponents`) | high (the AI component case), low (other classes' own entries) | UNVERIFIED |
| Loads, draws | none | high | UNVERIFIED |
| The case that matters | a `WillowMind`'s `Components` holds its own `WillowAIComponent` (seen in the probe), so the step gives it the mind as `Owner` | high | UNVERIFIED |

## 1. Where it runs in the spawn

The spawn routine (NATIVE_ENGINE_CORE.md, "Spawn routine, outcomes in order") was read again around its step 5. After
the new actor's `Location` and `Rotation` are written, and before `PhysicsVolume`, `SetOwner` and `Instigator`, the
spawn calls the actor's component-update virtual with both of its options off (a full update, not "collision only",
not "transform only"). Nothing before it tests whether play has begun, so it runs before map load too.

The virtual is the same body for `Actor`, `Controller`, `GearboxMind` and `WillowMind` (their class tables hold one
entry), and so is the inner pass it calls. On the line to the mind, nobody overrides it.

**The update, in order:**
1. **An optional pre-pass** over the actor's `AllComponents`, run only when one engine-wide setting is on. It was not
   identified. For a freshly spawned actor `AllComponents` is expected to be empty, so the pass would do nothing
   (UNVERIFIED).
2. **Guards.** The pass runs only when the actor's `bDeleteMe` is clear, the actor is not pending kill, and it carries
   neither of two object flags that by fit mark a class default object or an archetype (UNVERIFIED, fitted). A fresh
   spawn passes all three.
3. **The attach pass.**
   - The actor's local-to-world transform is built once, as `Actor.AttachComponent` builds it (section 3.4 there).
   - For each non-null entry of `Components`, in array order, the **conditional attach** runs (section 2), with the
     world's scene, the actor and that transform.

The guards and the transform are the ones `Actor.AttachComponent` uses. The difference is that the spawn walks the
`Components` the actor already holds, and appends nothing to it.

## 2. The conditional attach, per component

For one component, with a scene, an owner and a transform:
1. **A child-update bit.** `bSkipChildComponentUpdate` (a bit of `ActorComponent`'s bool word, identified by the
   layout tool) is set for the duration of a full update and cleared at the end. Its net value is false.
2. **Already attached** (`bAttached` set):
   - with `bNeedsReattach` set: the component is detached, `Scene` and `Owner` are cleared, and it is attached again
     as in step 3;
   - otherwise, with `bNeedsUpdateTransform` set: only its transform is refreshed. Nothing else is written.
3. **Not attached:** the **attach** (the same routine `Actor.AttachComponent` hands a component to, section 3.4 there):
   1. a component that is still attached is detached first (not reached from step 2's "not attached" branch);
   2. `bNeedsReattach` = false and `bNeedsUpdateTransform` = false;
   3. `Scene` = the world's scene, and `Owner` = the actor;
   4. the transform and one more value are handed to it (two class-table entries, not read);
   5. when its validity entry says yes (for an AI component, a plain object-state test), its **own attach entry**
      runs;
   6. the engine-wide notifier is told (section 2 of NATIVE_ATTACH_MESH_TO_PICKUP.md).
4. After a full update, one more entry of the component runs (a child-component update, not read).

**The own attach entry.**
- For `AIComponent` and `WillowAIComponent` (one shared entry, read): `bAttached` = true, and the component is
  appended to its `Owner`'s `AllComponents`. That is all.
- For other classes it was not read. A primitive component's entry is its scene registration (rendering, collision,
  physics), as section 3.4 of NATIVE_ATTACH_MESH_TO_PICKUP.md says.

**Loads and draws.** None. The only other effect is the memory for `AllComponents` growing.

## 3. Which classes it covers, on the slice

In the probe (section 5), the spawn's `Components` held, per spawned actor:
- **the target mind** (`WillowMind`): one entry, its own `WillowAIComponent`. This is link 9's case;
- `WillowAIPawn` and `WillowPlayerPawn`: six entries each (their meshes, cylinder and the like, primitive components);
- `WillowPlayerController`: one entry;
- a pickup: three entries.

So the step reaches every spawned actor. For link 9 only the mind's AI component matters, since that is the
`Owner` #600's rule asks.

## 4. A proposed Off answer, for a separate implementer (UNVERIFIED)

Reference registers none of this and is unchanged. Review r1 of #605 reshaped this section: the preflight runs before
the actor is made, so a refusal leaves no actor, and only the writes come after it.

**Where.** `ow-world`'s spawn (`actors.rs`, `spawn_from`), in Off only, in two parts:
- **the preflight**, in the spawn's pre-make block: after `instancing::plan_components` and `load_own_definitions`, and
  before `vm.instantiate`. It reads the source's `Components` mapped through the plan (each template component to the
  copy the plan will make). A refusal there leaves no actor, as the spawn's other pre-make refusals do;
- **the writes**, in `after_insertion`, which is where the spawn writes `Location` and `Rotation`: between the
  `Rotation` write and `set_owner` (`GainedChild`). They run both before and after play began. The actor is already in
  the level list by then, so an error there goes through the spawn's post-insertion boundary, as the spawn's other
  late refusals do. The preflight leaves the writes nothing to refuse except a failed heap write.

**The scene layer (hub ruling, review r1 of #605).**
- `SceneLayer` and `add_empty_scene` move from `ow-inventory` (`attach_mesh.rs`) into `ow-world`. `crate_edges_test.py`
  allows it with no edge change. `ow-inventory` keeps a `pub use` re-export, so its callers are unchanged.
- **Option (a) stands, with two conditions:**
  1. the layer is asked for only when at least one non-None `Components` entry is to be attached. An actor with no
     components spawns with no layer;
  2. `ow-world`'s standard Off world set-up installs the empty layer as a logged, **UNVERIFIED** harness input, so the
     map load and the slice, combat and `phaselock_world` worlds have it. A bare VM with no layer keeps
     `NotYet { LayerAbsent }`, with `Actor.AttachComponent`'s existing text.

**The preflight, in the pre-make block.** It writes, loads and draws nothing. Every read uses `vm.space.prop` and
`vm.space.read`, never `hit::access::get` or anything that calls `loaded`. In order, the first failure is the answer:
1. the source's class declares `Components` and `AllComponents` as arrays: else `VmError::Script`;
2. the source's `AllComponents` is empty: else `NotYet { NotRead }`, "the spawn's component update: the pre-pass over
   a non-empty AllComponents is not read";
3. for each non-None entry of the source's `Components`, in order:
   - **it is one of this spawn's copies:** the plan maps it to a copy that `apply_plan` will make for this actor. An
     entry the plan does not map (a shared component, a definition's component, any object that is not the template's
     own) is `NotYet { NotRead }`, "the spawn's component update: the component `<path>` is not this spawn's own copy
     (writing Owner onto it would write a shared object)". Otherwise `Owner`, the two flags, `bAttached` and
     `AllComponents` would be written onto a shared object;
   - it is not an unloaded definition handle: else `NotYet { NotRead }`, naming it;
   - its class declares `Owner`, `Scene`, `bAttached`, `bNeedsReattach`, `bNeedsUpdateTransform` and
     `bSkipChildComponentUpdate` (all on `ActorComponent`): else `VmError::Script`;
   - `bAttached` is false on the template component, which the copy takes. Otherwise `NotYet { NotRead }`, "the
     spawn's component update on an attached component: the reattach and transform refresh", the same caution as
     `Actor.AttachComponent`'s preflight;
   - its class is an `AIComponent` (`GearboxFramework.AIComponent` or a subclass) but not exactly
     `GearboxFramework.AIComponent` or `WillowGame.WillowAIComponent`: `NotYet { NotRead }`, "the spawn's component
     update: the own attach entry of `<class>` is not read" (the exact-class convention of #600);
4. when at least one entry passed step 3: the scene layer is present, else `NotYet { LayerAbsent }` with
   `Actor.AttachComponent`'s text.

**The writes, in `after_insertion`, for each copy in `Components` order (hub ruling on the empty-scene scope):**
1. **the generic attach, on every copy:** `bNeedsReattach` = false and `bNeedsUpdateTransform` = false; `Owner` = the
   actor. `Scene` stays at its zero value, as the ruling says for the empty layer;
2. **the own attach entry:**
   - **for exactly `GearboxFramework.AIComponent` and `WillowGame.WillowAIComponent`** (read): `bAttached` = true, then
     the copy appended to the actor's `AllComponents`;
   - **for every other class, nothing more.** That leaves the copy with **`Owner` set and `bAttached` false**, a state
     the game would not show for a primitive component once registered. This is labelled the port's answer,
     **UNVERIFIED**: for a primitive component the own entry is the scene registration the empty layer does not model;
     for the non-primitive classes whose own entry was not read (a light environment, a controller's components) the
     generic attach itself is UNVERIFIED;
3. `bSkipChildComponentUpdate` ends false (its net value). The child-component update entry is not modelled.

**What moves elsewhere.** Setting `Owner` on a spawned actor's own components changes the answer of other Off rules
that read it. **The implementing PR reports a row-by-row installed Off strict `--slice-gate` diff against main.**
- `ow-combat` `holds.rs` (the hold kinds): `HoldMovement` on a component whose `Owner` is set is a terminal NotRead
  ("owner controller virtual and motion vector constant unresolved"). Before this step an AI component's `Owner` was
  always None there. Whether link 9's lift reaches a Type 2 (movement) hold was **not checked here**: the probes so far
  stop before the lift's success path. The implementer's slice diff shows it.
- `ow-inventory` `attach_mesh.rs`: the deferred reattach's preflight (`SetForceRefPose`, `SetHidden`,
  `SetOwnerNoSee`) and `SetLightEnvironment` on an attached component refuse with `REATTACH_OWNER` when `Owner` is set.
  On a spawned actor's own components that refusal becomes reachable (the pickup's make step on links 6 and 7).
- **No change expected:** the trace filter's `owner_of`, the encroachment check's `ATTACH_STEP` (it keys on
  `bAttached`, which stays false for primitives), and the body composition's `attach_to_actor`.

**Tests on invented classes** (synthetic fixtures, the existing `package()` / `tag()` helpers, through the world's own
spawn). Each checks heap committed writes, load transitions and generator draws against what the step should do.
- An actor whose template holds one toy `AIComponent`: after the spawn the copy's `Owner` is the actor, `bAttached` is
  true, it is in the actor's `AllComponents` once, and `bNeedsReattach` and `bNeedsUpdateTransform` are false.
- **The mind's `AIComponent` slot holds the same copy that got `Owner`** (a toy mind with an `AIComponent` field that
  the template points at its component).
- The same with a toy primitive component: `Owner` set, `bAttached` false (unchanged), `AllComponents` unchanged.
- Two components: both handled, in `Components` order. A None entry is skipped.
- Before play began: the step still runs. After play began it runs before `PreBeginPlay`, and a script `GainedChild` or
  `PreBeginPlay` that reads the component's `Owner` sees the actor.
- **An actor with no components spawns in a world with no scene layer** (condition 1).
- Refusals, each **leaving no actor** (the level list and the heap as before the spawn) and with nothing written by
  the step:
  - an entry that is not this spawn's own copy (item 2 of the review);
  - a component that is an unloaded handle;
  - an attached template component;
  - a non-empty `AllComponents`;
  - an `AIComponent` subclass that is not one of the two exact classes;
  - a class without `Owner` (`VmError::Script`);
  - no scene layer with one component (LayerAbsent).
- **`HoldMovement` after the step** (the change listed above): a toy AI component spawned by the world, then a hold of
  the movement kind on it: the terminal NotRead, where before the step it answered as for Owner None.
- Reference: the spawn is unchanged.

## 5. Local probe (never pushed)

A throwaway local merge:
- #535 at its current head, #579, #564, #548, #555, #560, the link 9 spawn branches, #524, and #523 merged by hand;
- probe-only stubs of #589 (reduced to its plain step), #594, #600 (both AI component rules), and this note's step in
  `ow-world`'s spawn (Owner set, the two flags cleared, `bAttached` and `AllComponents` left as they were).

Installed Off strict `--slice-gate`, under the machine lock: 3 ok / 8 NotYet / 3 errors.
- The spawned `WillowMind`'s `Components` held its `WillowAIComponent`, which got the mind as `Owner`.
- **Link 9** ran both of the driver's steps (`spawn`, `cast`) and is an **error** at the unbound native
  `AttributeDefinitionBase.ResolveContext(object)`, the next stop past this note. It is the same stop the earlier
  probe reached with a hand-set `Owner`.
- The other rows moved too, but the merge base was not the earlier probe's, so their moves are not attributed here.
  An implementer reruns the gate on main.

## Open (each UNVERIFIED)

- The engine-wide setting that turns on the pre-pass over `AllComponents`.
- The two object flags of the guard (fitted as the class-default and archetype marks).
- The own attach entry of every class other than the two AI component classes, and the child-component update entry.
- Whether `AllComponents` is empty on every freshly spawned actor.
- **Map-placed actors** never go through `actors::spawn`, so this step does not reach them: a placed mind's AI component
  keeps `Owner` None, and #600's NotRead stays reachable for it. How the map load attaches placed actors' components
  was not read.

## Live confirmation (bl-sdk, the RTSE way; not run, no session granted)

On an enemy spawned in Sanctuary or the Fire mission's map, in a hook of its mind's `PreBeginPlay` (or `GainedChild`
on its owner, which runs earlier):
- read the mind's `AIComponent`, and that component's `Owner` (expected the mind), `bAttached` (expected true) and the
  mind's `AllComponents` (expected to hold the component);
- on the enemy pawn, read each `Components` entry's `Owner` (expected the pawn) and `bAttached`.
