# The body composition's rendering hooks (2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-BODYCOMP-RENDER-HOOKS (decision 2026-11-19-NNN, number
pending). Analyst only: a different lane implements it (clean room). This thread implemented #618, the lookups that
come before these hooks; that is a different rule. It implements nothing here.

**Sources, in order:**
- the records: NATIVE_INITIALIZE_BODY_COMPOSITION.md (#529) section 4.5 steps 3, 5 and 6 and its section 7 ("the
  body's rendering hooks", the hub's rulings at 19:58Z and on the unread flags); NATIVE_ATTACH_MESH_TO_PICKUP.md and
  its implementation in `ow-inventory` `attach_mesh.rs` (#511, #520), which already answer `SetOwnerNoSee` and
  `SetLightEnvironment` in Off;
- the installed packages: WillowGame's name table (the gear-likeness interface's name) and the class layouts of
  `Pawn`, `WillowPawn` and `PrimitiveComponent` through `tools/ghidra/class_layout.py`;
- `Borderlands2.exe`, read locally with `tools/ghidra/run.ps1 -ReadOnly`, under the machine lock. The body of the
  pawn's `PostInitBodyComposition` was found as the routine that interns the name `CustomHead`, the name its step 4
  compares. Its field reads land on the pawn's computed layout exactly (`LightEnvironment`, `HeadMesh`, `BodyClass`
  and `Controller`), a structural check of the fit. The two virtual calls were identified from the execs of the
  natives they belong to: each is the slot that native's own exec calls.

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/bcrh/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here
is UNVERIFIED** in the running game.

## 1. The three hooks, in order

The pawn's `PostInitBodyComposition` runs once per made attachment (#529 section 4.5). In mode 0 (and mode 3), after
the identifier steps (`Mesh`, `CylinderComponent`), it runs these three, in this order. The custom-head step comes
between hooks 1 and 2.

### Hook 1: the gear-likeness step

- **Gate:** two engine-wide flags, not identified (#529 "two flags"). It runs when the first is clear or the second
  is set. The hub's ruling on #529 takes them as allowing.
- **What it does:** the made object is asked for the interface `WillowGame.IGearLikenessConsumer`, through the
  object's own interface lookup. The interface's package is read; its name is WillowGame's only gear-likeness
  interface name (by fit, high). Then one routine runs on that interface.
- **When the object does not implement it,** the routine returns at once, and nothing is read or written.
- **When it does** (by fit, `GearLikenessMeshComponent` and its kind), the routine calls the interface's own methods
  to refresh the gear likeness: render-side, not read.
- **Reads:** the made object's class (its interfaces). **Writes:** nothing for a non-implementer; for an
  implementer, not read.

### Hook 2: the mode-0 owner-no-see setting

- **Gate,** all of:
  - mode 0 only (not mode 3);
  - the made object is a `PrimitiveComponent` (its class chain holds Engine's `PrimitiveComponent`);
  - a pawn virtual answers true. It is not identified: by fit, a test that the pawn is viewed by its own player;
  - the pawn's `Controller`, cast to an Engine controller class, answers true to a controller virtual. By fit, that
    is a `PlayerController` whose `IsLocalPlayerController` is true.
- **What it does:** it calls `PrimitiveComponent.SetOwnerNoSee(true)` on the made object. The call is the virtual slot
  that `SetOwnerNoSee`'s own exec calls, so it follows that native's rule: NATIVE_ATTACH_MESH_TO_PICKUP.md section 3,
  implemented in `attach_mesh.rs`.
  - If `bOwnerNoSee` is already true, nothing happens.
  - Otherwise comes the deferred reattach: with the component's `Owner` set, that is `NotYet { NotRead }`
    (`REATTACH_OWNER`, "the deferred reattach: the owner's component update"). Then `bOwnerNoSee` and
    `bNeedsReattach`.
- **Why it exists** (purpose by fit): a locally viewed player's own third-person body is hidden from that player.

### Hook 3: the light environment

- **Gate:** the pawn's `LightEnvironment` is set, and the made object passes a mesh-component test. Maya's archetype
  sets `LightEnvironment`: `MyLightEnvironment`, a `DynamicLightEnvironmentComponent`.
- **The mesh-component test** (the test's own helper, read by fit):
  - the object must be a mesh component whose `bAcceptsLights` is set. This is fitted: the component's computed layout
    with the constant shift #598 found for components;
  - the helper also checks an array of a second object it reaches, not identified. When that array holds entries and
    its first entry's field is set, the step is skipped;
  - when a `LightingChannels` bit is set (fitted), the helper clears `bUsePrecomputedShadows` and calls a component
    virtual (by fit, a lighting-channel refresh; not read).
- **What it does:** `PrimitiveComponent.SetLightEnvironment(Pawn.LightEnvironment)`, again through the slot that
  native's own exec calls, so it follows NATIVE_ATTACH_MESH_TO_PICKUP.md section 6.2 as `attach_mesh.rs` implements
  it:
  - the same value is nothing;
  - a component that is not attached (`bAttached` false) gets one write, `LightEnvironment`;
  - an attached one runs the deferred reattach's refusals, then `PreviousLightEnvironment`, `LightEnvironment` and
    `bNeedsReattach`.

## 2. What the six links need

Links 1, 3, 4, 5, 13 and 14 meet the hooks first on Maya's attachment 0, her made `Mesh` (a `SkeletalMeshComponent`,
mode 0). Links 3, 4, 5, 13 and 14 are blocked behind link 1, so each needs what link 1 needs:
- **Hook 1:** an Engine `SkeletalMeshComponent` does not implement the WillowGame interface (by its class tables;
  checkable read-only), so nothing happens.
- **Hook 2:** her pawn is the local player's, so the gate passes by fit. The outcome then depends on her `Mesh`
  template's `bOwnerNoSee`: true is nothing; false meets `REATTACH_OWNER`, since the made component's `Owner` is set
  by the attach. Her template's value was not read for this note.
- **Hook 3:** `MyLightEnvironment` is set. A mesh component that accepts lights, in the port's empty scene layer
  (`bAttached` left false by `Actor.AttachComponent`, NATIVE_ATTACH_MESH_TO_PICKUP.md section 5), gets the one write
  `LightEnvironment` = `MyLightEnvironment`.

**After the hooks** (section 5's probe), the six links stop at attachment 1's socket attach on her mesh:
"InitializeBodyComposition: the socket and bone tables of Char_Siren.Mesh.Skel_SirenBody", `LayerAbsent`, the
skeletal mesh data reader. #569's layer now has `sockets()` and `reference_skeleton()`, so that refusal is the next
lane.

## 3. Real answer or harness input, hook by hook

The hub's rule (19:58Z, #529): a hook answers "nothing" in Off only when it writes no field the script or a slice step
reads. #605's pattern keeps a `LayerAbsent` behind a harness input (an empty `SceneLayer` service) where the port has
no layer.
- **Hook 1: a real answer for non-implementers; `LayerAbsent` for implementers.** The interface test is read and
  checkable, and its "no" writes nothing, so Off answers it. An implementer's refresh is not read and is render-side.
  It stays `NotYet { LayerAbsent }`, "InitializeBodyComposition: the body's rendering hooks (the gear-likeness
  refresh)", layer "a rendering layer". An empty render-layer harness input (as #605's `SceneLayer`) could answer it
  only under a hub ruling, because what it writes was not read. Maya's gear-likeness components meet their socket
  attach first, so the slice does not need that ruling now.
- **Hook 2: a real answer.** `SetOwnerNoSee`'s rule is already ported. Only the two gate virtuals are new:
  - the controller test can use the port's Off `IsLocalPlayerController` (decision 2026-11-19-15), by fit;
  - the pawn virtual is not identified. The proposed answer takes it as allowing under the same ruling as #529's
    unread flags (hub to confirm), else `NotYet { NotRead }`.
- **Hook 3: a real answer.** `SetLightEnvironment`'s rule is already ported. The mesh-component test's fitted parts:
  - the `bAcceptsLights` test is answered;
  - the unidentified array check and the lighting-channel branch are each `NotYet { NotRead }` when they would act.
  - On Maya's path that depends on her template's values, which the implementer's strict re-run shows.

## 4. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces `body_composition.rs`'s `RENDER_HOOKS` refusal (ruling 4) and its
test-only switch, per made component. **Reference is unchanged.** Every new `NotYet` carries this note as its record:
`docs/verification/NATIVE_BODYCOMP_RENDER_HOOKS.md`.

**Where:** in the body composition's preflight, per made attachment, where `RENDER_HOOKS` is raised today. The writes
go in the walk's writes for that attachment, after its identifier writes and in the hooks' order (1, the custom
head, 2, 3).

**Preflight, in this order** (write-free, load-free, RNG-free; reads with `vm.space.prop` / `vm.space.read`):
1. **Hook 1:**
   - the engine flags: taken as allowing (the hub's ruling on #529);
   - `WillowGame.IGearLikenessConsumer` resolved read-only (`classes.by_path`, a defined class), else
     `NotYet { NotRead }`, "the gear-likeness interface not resolved at set-up";
   - whether the made template's class implements it, read-only: the interface tables resolved at set-up, as #602's
     `plain_interface_read_only` reads them, never the caching `implements`. An unresolved entry is
     `NotYet { NotRead }`;
   - no: nothing. Yes: `NotYet { LayerAbsent }`, "InitializeBodyComposition: the body's rendering hooks (the
     gear-likeness refresh)", layer "a rendering layer".
2. **Hook 2,** in mode 0 and for a `PrimitiveComponent`:
   - the pawn virtual: taken as allowing (hub to confirm), else `NotYet { NotRead }`, "the owner-no-see gate's pawn
     test (not identified)";
   - the pawn's `Controller`: declared as an object, else `VmError::Script`. None, or not a `PlayerController`: the
     gate fails and nothing happens. A `PlayerController`: its Off `IsLocalPlayerController` answer, read-only (the
     same rule as the native, without calling it);
   - gate passed: `SetOwnerNoSee(true)`'s own preflight, planned on the made component's template values. If
     `bOwnerNoSee` is true, nothing. Otherwise the deferred reattach's refusals in their order: an `Owner` that the
     attach will set is `REATTACH_OWNER`, `NotYet { NotRead }`. The plan must use the value the attach writes, not
     the template's None;
   - each field read declared with its kind, else `VmError::Script` naming it.
3. **Hook 3,** with the pawn's `LightEnvironment` set (declared as an object, else `VmError::Script`):
   - the made class is a `MeshComponent`, and its `bAcceptsLights` (fitted) is set: else nothing;
   - the helper's array check: `NotYet { NotRead }`, "the light-environment step's array check (not identified)",
     unless the hub rules it as passing;
   - a set `LightingChannels` bit (fitted): `NotYet { NotRead }`, "the light-environment step's lighting-channel
     refresh";
   - then `SetLightEnvironment`'s own preflight (`attach_mesh::set_light_environment`'s checks, write-free), planned
     with the made component's values after the attach: the same value is nothing; not attached is one write;
     attached runs the deferred reattach's refusals.

**Writes** (only after the whole preflight passed, in the hooks' order): hook 2's `bOwnerNoSee` and `bNeedsReattach`
when they apply; hook 3's `LightEnvironment` (one write, or the attached path's three). The writes call the same
functions `attach_mesh.rs` exposes, so the native rules have one implementation.

**After writes started:** the walk's writes precede these. A `NotYet` raised in a hook's writes despite the preflight is
marked "stopped after partial writes: " by #518's counters, its kind and record kept.

**Tests for the implementer** (synthetic packages through `package()`/`tag()`; each refusal with the heap writes, draws
and load transitions unchanged):
1. a made component whose class does not implement the gear-likeness interface: hook 1 writes nothing;
2. one whose class implements it: the gear-likeness `LayerAbsent`, before any write;
3. the interface class not resolved at set-up, and an unresolved table entry: each `NotRead`;
4. hook 2 in mode 0 with a local player controller: `bOwnerNoSee` already true writes nothing; false meets
   `REATTACH_OWNER`;
5. hook 2 with no controller, a non-player controller, and mode 3: nothing;
6. hook 2 on a non-primitive made object: nothing;
7. hook 3 with no pawn `LightEnvironment`, and on a non-mesh component: nothing;
8. hook 3 with `bAcceptsLights` clear: nothing;
9. hook 3 on an unattached mesh component: the one `LightEnvironment` write, after the walk's own writes;
10. hook 3 with the lighting-channel bit set, and with the array check acting: each `NotRead`, before any write;
11. a malformed pawn or component field: `VmError::Script` naming it, before any write;
12. Reference: unchanged.

## 5. Local probe (never pushed)

A throwaway merge of #618 (6bf10dd3), #592 (5ab85711), #570 (d910fa13), #500 (c1ed82a9) and #528 (0e8e2312). Two
local stand-ins: #612's owner event, and the rendering hooks, each answered with nothing. The installed Off strict
`--slice-gate`, under the machine lock, gave ok 1, NotYet 10, error 3 of 14. Links 1, 3, 4, 5, 13 and 14 each run 0
of their driver's 1 step (`maya:8`). The first stop moved from "InitializeBodyComposition: the body's rendering
hooks" to "InitializeBodyComposition: the socket and bone tables of Char_Siren.Mesh.Skel_SirenBody" (`LayerAbsent`,
the skeletal mesh data reader), attachment 1's socket attach. The branch was deleted after.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not identified:** the two engine flags of hook 1, hook 2's pawn virtual, and hook 3's array check.
3. **By fit:** the interface name (`IGearLikenessConsumer`), hook 2's controller test (`IsLocalPlayerController`),
   and hook 3's `bAcceptsLights` and `LightingChannels` fields (the constant shift of #598).
4. **Not read:** the gear-likeness refresh, and hook 3's lighting-channel virtual.
5. **Not read for Maya:** her `Mesh` template's `bOwnerNoSee`, `bAcceptsLights` and `LightingChannels`.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, her `Mesh.bOwnerNoSee` is true (hook 2) and `Mesh.LightEnvironment` is `MyLightEnvironment`
   (hook 3).
2. Her `HeadMesh` (attachment 1) gets the same light environment, and a gear-likeness component on her gets its
   refreshed look; an enemy pawn's `Mesh.bOwnerNoSee` stays as its template has it.
