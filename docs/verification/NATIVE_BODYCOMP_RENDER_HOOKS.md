# The body composition's rendering hooks (2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-BODYCOMP-RENDER-HOOKS (decision 2026-11-19-235, number
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
- **Hook 2** (the hub's ruling on #622: the unidentified pawn test is `NotRead` whenever it would decide). Her
  controller is the local player's, so the known parts of the gate pass by fit, and the outcome turns on her `Mesh`
  template's `bOwnerNoSee`, which was not read for this note:
  - true: the setter would do nothing, and so would a failed pawn test. The test does not decide, so hook 2 is
    nothing;
  - false: a passing test would run the setter (its deferred-reattach refusal, since the attach sets the component's
    `Owner`), and a failing one would do nothing. The test decides, so hook 2 is `NotYet { NotRead }`, the pawn test.
- **Hook 3:** `MyLightEnvironment` is set. If her template's `bAcceptsLights` (fitted) is clear, hook 3 is nothing.
  If it is set, the helper's unidentified array check is reached, and it is `NotYet { NotRead }` (no passing answer is
  authorised, review r1 of #622). That check is the **current stop on this path**. The one `LightEnvironment` write is
  reachable only once that check is read, or under an explicit harness input the hub grants.

**After the hooks** (section 5's probe, where all three hooks were answered with nothing as a stand-in), the six links
stop at attachment 1's socket attach on her mesh: "InitializeBodyComposition: the socket and bone tables of
Char_Siren.Mesh.Skel_SirenBody", `LayerAbsent`, the skeletal mesh data reader. With the rules of section 4 instead of
the stand-in, they may stop earlier, at hook 2's pawn test or hook 3's array check, depending on her template's values.
#569's layer now has `sockets()` and `reference_skeleton()`, so the socket refusal is the next lane after these.

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
- **Hook 2: a real answer where the pawn test does not decide, `NotRead` where it does.** `SetOwnerNoSee`'s rule is
  already ported. The controller test can use the port's Off `IsLocalPlayerController` (decision 2026-11-19-15), by
  fit. The pawn virtual is not identified. Under the hub's ruling it is `NotYet { NotRead }` exactly when its unknown
  result would change the outcome (section 4, rule 2).
- **Hook 3: a real answer up to the array check.** `SetLightEnvironment`'s rule is already ported. The mesh-component
  test's fitted `bAcceptsLights` is answered; once it passes, the unidentified array check is `NotYet { NotRead }`
  (no passing answer is authorised), and so is the lighting-channel branch when it would act.

**Where the shared logic lives** (review r1 of #622, item 1). The two setters' Off rules are in `ow-inventory`
(`attach_mesh.rs`), and `ow-inventory` already depends on `ow-world`. So the body composition (`ow-world`) cannot call
them: that would be a crate cycle. The proposal:
- move the setters' **planning and apply logic** into `ow-world` (a module beside the body composition, or another
  crate both already depend on). The planning functions read only; the apply functions take the plan and write;
- the existing Off natives in `ow-inventory` and the body composition then both call that one implementation;
- the bindings stay as they are, `SetLightEnvironment`'s keep-existing registration included. Only the code behind
  them moves, so each native rule keeps one implementation;
- `crate_edges_test.py` confirms the new edge set.

## 4. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces `body_composition.rs`'s `RENDER_HOOKS` refusal (ruling 4) and its
test-only switch, per made component. **Reference is unchanged.** Every new `NotYet` carries this note as its record:
`docs/verification/NATIVE_BODYCOMP_RENDER_HOOKS.md`.

**Where, in the preflight** (review r1 of #622, item 4). Per made attachment, the game runs hook 1, then the
custom-head step, then hooks 2 and 3. Today's `RENDER_HOOKS` site comes after the `CustomHead` validation (the
`HeadMesh` declaration check), so the hooks go in two places:
1. **hook 1 before the `CustomHead` validation**, right after the identifier steps. Otherwise a malformed `HeadMesh`
   declaration would mask hook 1's earlier refusal;
2. **hooks 2 and 3 after it**, at today's `RENDER_HOOKS` site.

The writes go in the walk's writes for that attachment, after its identifier writes, in the same order: hook 1 (none
in Off), the custom head, hook 2, hook 3.

**The values the preflight may plan with** (review r1 of #622, item 3):
- **Settled template values only.** The walk accepts lazy component templates and loads them in `make_component`
  before it copies their slots. A preflight that read an unloaded template could plan a no-op (`bAcceptsLights`
  clear) for a component whose loaded value turns out true. Loading in the preflight is itself a write. So on every
  path that needs a template value (hook 2's `bOwnerNoSee`; hook 3's `bAcceptsLights`, `LightingChannels`,
  `LightEnvironment` and `bAttached`), an unloaded or failed template is `NotYet { NotRead }`, "the rendering hooks
  on a template not loaded at set-up (its values are not settled)", before any write.
- **The slots the real attach produces.** The made component's values are its template's, with what
  `Actor.AttachComponent` writes in the port's empty scene: `Owner` = the pawn, the component appended to the pawn's
  `Components`, and `bNeedsReattach` and `bNeedsUpdateTransform` cleared. `bAttached` stays as copied from the
  template, and so does `Scene`. The planners read those values, never the template's `Owner` (None).

**The setters' planners** (review r1 of #622, item 4). Each setter's own validation order is kept, so a malformed
component cannot pass the preflight and then fail after earlier writes:
- `set_flag` (behind `SetOwnerNoSee`) checks the deferred-reattach declarations, then the flag's declaration, and only
  then the equal-value shortcut, then the reattach's refusals;
- `set_light_environment` checks `LightEnvironment`, then the equal value, then `bAttached`, and on each path the
  destination's compatibility and the slots it writes, before its first write.

These checks are extracted into immutable planners (reads only, the same order, the same errors). The preflight calls
the planners; the writes call the apply side with the plan. The existing natives become planner then apply, with no
behaviour change.

**Preflight, in this order** (write-free, load-free, RNG-free; reads with `vm.space.prop` / `vm.space.read`):
1. **Hook 1** (before the `CustomHead` validation):
   - the engine flags: taken as allowing (the hub's ruling on #529);
   - `WillowGame.IGearLikenessConsumer` resolved read-only (`classes.by_path`, a defined class), else
     `NotYet { NotRead }`, "the gear-likeness interface not resolved at set-up";
   - whether the made template's class implements it, read-only: the interface tables resolved at set-up, as #602's
     `plain_interface_read_only` reads them, never the caching `implements`. An unresolved entry is
     `NotYet { NotRead }`;
   - no: nothing. Yes: `NotYet { LayerAbsent }`, "InitializeBodyComposition: the body's rendering hooks (the
     gear-likeness refresh)", layer "a rendering layer".
2. **Hook 2** (after it):
   - the known parts of the gate, in the game's order. Not mode 0, not a `PrimitiveComponent`, or a pawn
     `Controller` that is None or not a local `PlayerController` (its Off `IsLocalPlayerController` answer,
     read-only): the gate fails whatever the pawn test says, so nothing;
   - the known parts pass: the settled template must be loaded (above), and `set_flag`'s planner runs on the slots the
     attach produces. If it plans nothing (`bOwnerNoSee` already true), the pawn test cannot change the outcome, so
     nothing;
   - otherwise the pawn test decides (pass: the setter's plan, here the deferred reattach's `REATTACH_OWNER`, since
     `Owner` is set; fail: nothing), so `NotYet { NotRead }`, "the owner-no-see gate's pawn test (not identified)".
     It is raised before the setter's own refusals, because the gate comes first;
   - each field read declared with its kind, else `VmError::Script` naming it.
3. **Hook 3** (after hook 2), with the pawn's `LightEnvironment` set (declared as an object, else `VmError::Script`):
   - the made class is not a `MeshComponent`: nothing;
   - the settled template must be loaded (above); its `bAcceptsLights` (fitted) clear: nothing;
   - set: the helper's array check, `NotYet { NotRead }`, "the light-environment step's array check (not
     identified)". This is the current stop on that path;
   - **only once that check is read, or under an explicit harness input:**
     - a set `LightingChannels` bit (fitted) is `NotYet { NotRead }`, "the light-environment step's lighting-channel
       refresh";
     - then `set_light_environment`'s planner on the slots the attach produces: the same value is nothing; not
       attached is one write; attached runs the deferred reattach's refusals.

**Writes** (only after the whole preflight passed, in the hooks' order): the setters' apply side with their plans.
Hook 2's writes are never reached under today's rules: where the setter would write, the pawn test refuses first.
Hook 3's writes come only once the array check is resolved.

**After writes started:** the walk's writes precede these. A `NotYet` raised in a hook's writes despite the preflight is
marked "stopped after partial writes: " by #518's counters, its kind and record kept.

**Tests for the implementer** (synthetic packages through `package()`/`tag()`; each refusal with the heap writes, draws
and load transitions unchanged):
1. hook 1, a made component whose class does not implement the gear-likeness interface: nothing written;
2. hook 1, one whose class implements it: the gear-likeness `LayerAbsent`, before any write;
3. the interface class not resolved at set-up, and an unresolved table entry: each `NotRead`;
4. hook 1's refusal and a malformed `HeadMesh` declaration together: hook 1's refusal is the one reported;
5. hook 2 with a local player controller and `bOwnerNoSee` already true: nothing (the pawn test does not decide);
6. hook 2 with a local player controller and `bOwnerNoSee` clear: the pawn test's `NotRead`, before any write, ahead
   of `REATTACH_OWNER`;
7. hook 2 with no controller, a non-player controller, a non-local one, mode 3, or a non-primitive made object:
   nothing;
8. hook 2 with a malformed reattach declaration on a component whose flag already equals the value: the setter
   planner's `VmError::Script` (its order kept), before any write;
9. hook 3 with no pawn `LightEnvironment`, on a non-mesh component, or with `bAcceptsLights` clear: nothing;
10. hook 3 with `bAcceptsLights` set: the array check's `NotRead`, before any write;
11. an unloaded or failed template on a path that needs its values (hooks 2 and 3): the not-settled `NotRead`, with
    nothing loaded;
12. a malformed pawn or component field: `VmError::Script` naming it, before any write;
13. **conditional on the array check being read, or on an explicit harness input:** hook 3 on an unattached mesh
    component gets the one `LightEnvironment` write after the walk's own writes; the lighting-channel bit set is
    `NotRead`;
14. the existing Off natives (`SetOwnerNoSee`, `SetLightEnvironment`) keep their tests unchanged after the move to the
    shared planners, and `crate_edges_test.py` passes;
15. Reference: unchanged.

## 5. Local probe (never pushed)

A throwaway merge of #618 (6bf10dd3), #592 (5ab85711), #570 (d910fa13), #500 (c1ed82a9) and #528 (0e8e2312). Two
local stand-ins: #612's owner event, and the rendering hooks, each answered with nothing. The installed Off strict
`--slice-gate`, under the machine lock, gave ok 1, NotYet 10, error 3 of 14. Links 1, 3, 4, 5, 13 and 14 each run 0
of their driver's 1 step (`maya:8`). The first stop moved from "InitializeBodyComposition: the body's rendering
hooks" to "InitializeBodyComposition: the socket and bone tables of Char_Siren.Mesh.Skel_SirenBody" (`LayerAbsent`,
the skeletal mesh data reader), attachment 1's socket attach. The branch was deleted after.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not identified:** the two engine flags of hook 1, hook 2's pawn virtual (`NotRead` where it decides, the hub's ruling on #622), and hook 3's array check (the current stop on its path).
3. **By fit:** the interface name (`IGearLikenessConsumer`), hook 2's controller test (`IsLocalPlayerController`),
   and hook 3's `bAcceptsLights` and `LightingChannels` fields (the constant shift of #598).
4. **Not read:** the gear-likeness refresh, and hook 3's lighting-channel virtual.
5. **Not read for Maya:** her `Mesh` template's `bOwnerNoSee`, `bAcceptsLights` and `LightingChannels`.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, her `Mesh.bOwnerNoSee` is true (hook 2) and `Mesh.LightEnvironment` is `MyLightEnvironment`
   (hook 3); the pawn test's answer on her pawn (hook 2's gate) and the array check's input (hook 3).
2. Her `HeadMesh` (attachment 1) gets the same light environment, and a gear-likeness component on her gets its
   refreshed look; an enemy pawn's `Mesh.bOwnerNoSee` stays as its template has it.
