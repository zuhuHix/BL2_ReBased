# The tree initialisation's owner event: `PostInitAnimTree` (2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-TREE-INIT-OWNER-EVENT (decision 2026-11-19-222, number
pending). Analyst only: a different lane implements it (clean room). This thread implemented the step before it
(#608, the component pass and the control arrays). It implements nothing here.

**Sources, in order:**
- the records: NATIVE_COPY_ANIM_TREE.md (#551) section 2 step 6 and rule 4, NATIVE_COPY_COMPONENT_PASS.md (#598)
  section 7, NATIVE_ANIM_NODE_LOOKUP.md (#543) and NATIVE_INITIALIZE_BODY_COMPOSITION.md (#529);
- the script, through `research/script_disasm.py`: `Actor.PostInitAnimTree`, `Pawn.PostInitAnimTree`,
  `WillowPlayerPawn.PostInitAnimTree` and `Pawn.ClearAnimNodes`. `GamePawn`, `GearboxPawn`, `WillowPawn`,
  `WillowAIPawn` and `WillowVehicle` were asked too and declare no version;
- `Borderlands2.exe`, read locally with `tools/ghidra/run.ps1 -ReadOnly`, under the machine lock. This settled only
  the event's name and where it is sent: the engine's start-up name table holds the name `PostInitAnimTree`, and the
  one engine routine that sends it is the tree initialisation #598 read. That routine was found from the script native
  `InitSkelControls`, as #598 found it.

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/toie/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here
is UNVERIFIED** in the running game.

## 1. Which event, on which owner, and when

**The event is `PostInitAnimTree`.** #551 and #598 had it by fit. The executable settles it: the name the tree
initialisation sends is the one the start-up name table registers as `PostInitAnimTree`, and no other engine routine
sends that name.

**When it is sent:**
- at the end of the tree initialisation (#551 section 2): after the tick list, the per-node initialisation, the init
  tag and the per-node arrays, and step 5 (the component pass and the per-bone control arrays, #598);
- only when the component's `Animations` is a tree (the same "this node as a tree" call #598 section 4 step 2 reads)
  **and** the component's owner (`ActorComponent.Owner`) is set. Otherwise it is skipped;
- it is sent before `bAnimTreeInitialised` is set. The initialisation's own gate (#598 section 1: `ParentAnimComponent`
  None, `Animations` set, forced or not yet initialised) comes first.

**How it is sent:** as an ordinary script event to the owner. The function is found on the owner by that name (by fit,
the owner's current state first, then its class chain, as any script event is), and it is called with one argument.
The event's script signature is one `SkeletalMeshComponent` (`SkelComp`), and the only component the routine holds is
the one being initialised, so the argument is that component. The decompiler hid the store of the argument, so this
is by the signature (high, UNVERIFIED).

**Order within Maya's attach** (from #529 section 4.5 and NATIVE_MESH_REST.md):
1. the made component is attached to the pawn (by fit, its `Owner` set);
2. the attach instances its tree (#561, #575, #608), and this event is sent to the pawn;
3. only later, in the pawn's `PostInitBodyComposition`, does `Pawn.Mesh` become the made component (#529 section 4.5,
   NATIVE_MESH_REST.md section 2).

So when the event runs, the pawn's `Mesh` is not the made component. **In the port** (review r1 of #612, item 5): on
the #608 head, `body_composition.rs` refuses a pawn that already holds a `Mesh` (the old-`Mesh` release, `NotRead`)
before it reaches `attach_check`, so the copy's preflight only ever runs with `Pawn.Mesh` unset; and #529 records
#542's measurement of Maya's case. **In the game,** the archetype's `Mesh` is not the new component whichever way the
order goes, so `Pawn`'s `SkelComp == Mesh` test is false for this attach. The game's order itself stays by fit from
those records, UNVERIFIED.

### What the three versions do

- **`Actor.PostInitAnimTree`:** nothing.
- **`Pawn.PostInitAnimTree`:** runs `Actor`'s. Then, only when the argument is the pawn's own `Mesh`, it calls the
  virtual `ClearAnimNodes` and then the native `Pawn.CacheAnimNodes`. `Pawn.ClearAnimNodes` empties `Pawn.SlotNodes`.
  No class on the slice's lines overrides it among those asked. `CacheAnimNodes` is native and was not read.
- **`WillowPlayerPawn.PostInitAnimTree`:** runs `Pawn`'s. Then three caches, each **only while it is still None**:
  1. `ArmBoneBlend` = the component's animation node named `ArmBoneBlend`, kept only as an `AnimNodeBlendPerBone`;
  2. `AimOffset`, only when `BodyClass` is also set, = the component's animation node named
     `BodyClass.AimOffsetNodeName`, kept only as a `WillowAnimNodeAimOffset`;
  3. `LeftHandIK` = the component's skeletal control named `LeftHandGrip`, kept only as a
     `SkelControlLeftHandGripWeapon`.

  The lookups are the script natives `SkeletalMeshComponent.FindAnimNode` and `FindSkelControl`, whose rules
  NATIVE_ANIM_NODE_LOOKUP.md section 1 reads. A result of another class is stored as None (the cast). Each
  assignment writes the field even when the result is None.
- **The enemy's pawn** (`WillowAIPawn`) declares no version, so it takes `Pawn`'s.

**What it reads:** the owner's `Mesh`, `ArmBoneBlend`, `AimOffset`, `LeftHandIK` and `BodyClass` fields, the body
class's `AimOffsetNodeName`, and the instanced tree, read through the two lookups. **What it writes:** on the owner
only, those three fields; and, on `Pawn`'s branch, `SlotNodes` plus whatever `CacheAnimNodes` writes (not read).
Nothing is loaded by the event itself. No random number is drawn on these paths.

## 2. What each link needs

Links 1, 3, 4, 5, 13 and 14 share one first stop, the `maya:8` step: Maya's spawn, then her body composition's
attachment 0 (her `Mesh`). Links 3, 4, 5, 13 and 14 are blocked behind link 1, so each needs exactly what link 1
needs:
- the event sent to Maya's pawn (a `WillowPlayerPawn`) with her made `Mesh`;
- `Pawn`'s branch skipped (her `Mesh` is not yet the made component; section 1's order);
- the three lookups, on the instanced tree:
  - `ArmBoneBlend`: no node of that name on her tree, so None (#543 section 4);
  - `AimOffset`: her body class's `AimOffsetNodeName` is `AimOffset`, so the instance's `WillowAnimNodeAimOffset`
    (saved tick index 5);
  - `LeftHandIK`: the instance's `LeftHandGrip` control (a `SkelControlLeftHandGripWeapon`).

**After the event**, by a local probe with a stand-in (section 5), the six links stop at the next step: the body
composition's own lookups in `PostInitBodyComposition` (#557's refusal "needs an animation-tree instance for the
skeletal mesh, which is not built yet"). With the copy in place that instance now exists, so that refusal is the next
lane. NATIVE_ANIM_NODE_LOOKUP.md section 1 already says one lookup can serve both callers.

## 3. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces #561's `OWNER_EVENT` refusal ("the owner's event after the tree
initialisation"). **Reference is unchanged.** Every new `NotYet` below carries this note as its record:
`docs/verification/NATIVE_TREE_INIT_OWNER_EVENT.md`.

**Prerequisites, in landing order** (review r1 of #612, item 6):
1. #570 (`fix/rust-mesh-rest` at d910fa13): `Vm::find_loaded_method` exists only there;
2. #608 (`fix/rust-copy-component-pass` at 383c929c): the copy's step 5, after which this event runs;
3. the probe's other branches, which the slice needs but this rule does not: #592 (5ab85711, the skeleton source and
   Maya's mesh preload), #500 (c1ed82a9) and #528 (0e8e2312).

`anim_tree_copy::plan` and `attach_check` need a new parameter, the owner (below, preflight rule 1).

**Where it runs.** In `anim_tree_copy::initialise`, after step 5's writes and before `bAnimTreeInitialised` is set, as
section 1 orders it. It is planned in the copy's preflight (`anim_tree_copy::plan`), which already runs write-free
before the copy's first write. **The write path must call `instance` after the attach has written the component's
`Owner`** (`attach_to_actor`, #529's attach step). Called before, "no owner: no event" would skip the event without a
word.

**The call** (review r1 of #612, item 1). It is `vm.call(f, Some(owner), vec![Value::Object(Some(component))])`,
with the `f` the preflight found, as `ow-inventory/src/anim_tree.rs` `notify_owner` does for `SetAnimTreeTemplate`.
It is **not** the world-event path (`states::send_world_event` / `find_event`). That path writes: `ensure_frame`
writes the heap's state name (counted by `note_write`) and builds a probe mask that can load class code. It also walks
parent states, which `find_loaded_method` does not.

**Preflight additions, in this order** (each refusal before any write, nothing loaded, nothing drawn):
1. **The owner:**
   - in the instance path it is the component's `Owner`. In the attach's check (`attach_check`, which runs on the
     template component), it is the actor being attached to, passed by the caller; the template's own `Owner` is not
     it;
   - `Owner` must be declared as an object reference, else `VmError::Script`;
   - no owner: no event.
2. **The owner must be resident:** an unloaded handle is #543's refusal (`anim_node_lookup::refusal`), as #561 refuses
   any unloaded object.
3. **The owner's state.** `find_loaded_method(class, name, state)` reads only the heap state's own table, not its
   parent states, and the game's lookup takes the state chain (by fit). So:
   - an owner in no state: the class chain, through `find_loaded_method(class, "PostInitAnimTree", None)`;
   - an owner in a state: `NotYet { NotRead }`, "the owner's PostInitAnimTree while the owner is in a state (the
     state chain lookup is not planned read-only)". An implementer may replace this refusal with a read-only lookup
     over the state and its parent states, as `crate::states` resolves them, reading class code only where it is
     loaded (a chain class whose code is not loaded is then rule 4's refusal). Whether Maya's pawn holds a state at
     this point was not read; the strict `--slice-gate` re-run below shows it.
4. **The event, found without loading:**
   - `find_loaded_method` answering None (a chain class whose code is not loaded): `NotYet { NotRead }`, "the owner's
     PostInitAnimTree on a class whose code is not loaded" (finding it would be a load);
   - a function that is none of `Engine.Actor.PostInitAnimTree`, `Engine.Pawn.PostInitAnimTree` and
     `WillowGame.WillowPlayerPawn.PostInitAnimTree` (compared by path): `NotYet { NotRead }`, "an override of
     PostInitAnimTree that was not read (`<path>`)". A state function of that name meets rule 3 first.
5. **The call's entry** (review r1 of #612, item 4): `Vm::call` refuses on a borrowed service, on the argument binding
   and on its entry checks (`check_entry`). The preflight plans these with `Vm::check_call(f, args)`, which makes the
   same binding and entry checks without making the call or changing anything. The caller holds no service borrow
   across the call (the copy's `instance` holds none).
6. **`Pawn`'s branch** (for `Pawn`'s and `WillowPlayerPawn`'s versions):
   - the owner must declare `Mesh` (`VmError::Script` if not);
   - when the owner's `Mesh` is the component (in the instance path; never in the attach, whose component is new),
     the branch would run `ClearAnimNodes` and the native `CacheAnimNodes`, so it refuses: `NotYet { NotRead }`, "the
     owner's PostInitAnimTree on its own Mesh: Pawn.CacheAnimNodes (native) is not read". The `ClearAnimNodes`
     override question does not arise before that.
7. **`WillowPlayerPawn`'s lookups** (for its version only):
   - the owner must declare `ArmBoneBlend`, `AimOffset`, `LeftHandIK` and `BodyClass` as object references, else
     `VmError::Script`;
   - a set `BodyClass` must be resident (#543's refusal if not), and declare `AimOffsetNodeName` as a name;
   - **the natives.** `SkeletalMeshComponent.FindAnimNode` and `SkeletalMeshComponent.FindSkelControl` must be bound
     in Off. An unbound one is `NotYet { NotPorted }`, each with its own text:
     - "SkeletalMeshComponent.FindAnimNode in Off";
     - "SkeletalMeshComponent.FindSkelControl in Off";
   - **the casts** (review r1 of #612, item 2). The script casts each find, and the VM's dynamic cast calls `class_at`,
     which can define a class layout on first use, and `is_child`, which raises `NotYet` through an engine-native
     stand-in chain. So the preflight:
     - resolves the three cast classes, `Engine.AnimNodeBlendPerBone`, `WillowGame.WillowAnimNodeAimOffset` and
       `WillowGame.SkelControlLeftHandGripWeapon`, read-only (`classes.by_path`, a defined layout), else
       `NotYet { NotRead }`, "the cast class `<path>` is not defined at set-up";
     - computes each cast's answer on the template object the lookup plans (below), with the same rule as the cast
       (`is_child`, its stand-in check included), so any `NotYet` of it is raised before the first write;
     - at run time the copy has the template's class, so the cast meets a defined layout and the answer the preflight
       computed;
   - **the lookups planned:** each find is planned on the template (#543 rule 3: the copy keeps each node's class and
     name), over the copy's planned tick order and control order. The template's nodes and controls are the copy's
     sources, already checked resident by #561's gather.
8. **Random numbers:** none of the three versions draws one.

**The two natives' Off binding** (review r1 of #612, item 3). The binding is global in Off: every script caller gets
it, not only this event. #543 read only the tick-list path, so the binding is scoped to it:
- **the component's own tree:** `Animations` is set, its class derives from `Engine.AnimTree`, and its heap outer is
  the component (the copy's outer, #561). Anything else is not the component's own tree;
- **the tick-list path:** with the component's own tree and a non-empty `AnimTickArray` whose first entry is
  `Animations`, `FindAnimNode(Name)` answers the first node of `AnimTickArray`, in order, whose `NodeName` equals
  `Name`, compared as a whole name (name and number); a None `Name` matches the first node whose `NodeName` is None;
- **the gather path** (no `Animations` that is the own tree, or an empty tick list): `NotYet { NotRead }`, "the
  animation-node lookup's gather path (no tick list, or not the component's own tree)". With no `Animations` at all
  the answer is None (#543 section 1);
- **`FindSkelControl(Name)`:** None at once for a None `Name`; None when the component has no tree or its tree is not
  an `AnimTree`; else the tree's `SkelControlLists` in array order, each list's `ControlHead` then its `NextControl`
  chain, the first control whose `ControlName` equals `Name` (a whole name);
- both read only and load nothing. A target that is an unloaded handle is #543's refusal;
- **a strict re-run.** Because the binding is global, the implementer re-runs the installed Off strict
  `--slice-gate` and reports every link whose stop changes, not only the six below.

**The writes** (only after the whole preflight passed): the call above. Its script writes what section 1 lists. On
Maya's path that is `ArmBoneBlend` = None, `AimOffset` = the instance's aim-offset node, and `LeftHandIK` = the
instance's `LeftHandGrip` control (the fields that are None before; Maya's are, at spawn).

**After writes started:** the copy's writes precede the event. So any `NotYet` the event's run raises despite the
preflight is marked "stopped after partial writes: " by #518's counters, as `anim_tree_copy::instance` already does
(`after_writes`), its kind and record kept. Any other error passes unchanged.

**Tests for the implementer** (synthetic packages through `package()`/`tag()` or the copy's invented fixture; each
refusal with the heap writes, draws and load transitions unchanged):
1. no owner: no event, and the initialisation still sets `bAnimTreeInitialised`;
2. an owner of a class with only `Actor`'s version: the event runs and writes nothing;
3. a `Pawn`-line owner whose `Mesh` is another component: `Pawn`'s branch skipped, nothing written;
4. the same owner whose `Mesh` is the component: the `CacheAnimNodes` refusal, before any write;
5. a `WillowPlayerPawn`-shaped owner (an invented class whose event is the `WillowPlayerPawn` path, or the installed
   path in a toy package) with all three caches None: `ArmBoneBlend` None when no node has that name, `AimOffset` the
   copy's aim-offset node named by `AimOffsetNodeName`, `LeftHandIK` the copy's `LeftHandGrip` control;
6. the same with a cache already set: that field is left as it was;
7. with `BodyClass` None: `AimOffset` is not looked up and stays None;
8. a lookup result of another class (a node named `ArmBoneBlend` that is not an `AnimNodeBlendPerBone`): stored as
   None, the cast's answer computed in the preflight (no `NotYet` after the writes);
9. an override of `PostInitAnimTree` on an invented subclass: the override refusal, before any write;
10. an owner whose class code is not loaded: the not-loaded refusal, with nothing loaded;
11. an unloaded owner handle: #543's refusal;
12. a malformed owner (no `Mesh`, or a cache field of another kind): `VmError::Script`, before any write;
13. the attach check with a passed owner: the same refusals on the template, nothing written;
14. Reference: nothing new registered, and the copy's Reference path unchanged;
15. an owner in a state: the state refusal, before any write; and a state function named `PostInitAnimTree` on the
    owner's class, reached by that state: the same refusal (rule 3 before rule 4);
16. an unloaded `BodyClass`: #543's refusal, before any write;
17. each of the two natives unbound: its own `NotPorted` text, before any write;
18. a `NotYet` raised during the run (an invented event body that reaches an unread native after the copy's writes):
    marked "stopped after partial writes: " once, its kind and record kept;
19. the binding's gather path: `FindAnimNode` on a component whose tick list is empty, and on one whose `Animations`
    is not its own tree, each `NotRead`; and a whole-name match (`Name_1` does not match `Name_2`).

## 4. Maya's attachment 0 (names and counts only)

- Owner: her pawn, a `WillowPlayerPawn`; `Pawn.Mesh` at that point is not the made component (section 1).
- `BodyClass`: `BodyClass_Siren`, whose `AimOffsetNodeName` is `AimOffset` (#543 section 4).
- The event's result: `ArmBoneBlend` None; `AimOffset` the aim-offset node with saved tick index 5; `LeftHandIK` the
  `LeftHandGrip` control. These are the same three values #543 gives for the body composition's later lookups, which
  find the fields set and set them again.

## 5. Local probe (never pushed)

A throwaway local merge of #608 (383c929c), #592 (5ab85711), #570 (d910fa13), #500 (c1ed82a9) and #528 (0e8e2312) was
run with one local stand-in: the owner's event answered with nothing. The installed Off strict `--slice-gate`, under
the machine lock, gave ok 1, NotYet 10, error 3 of 14. Links 1, 3, 4, 5, 13 and 14 each run 0 of their driver's 1 step
(`maya:8`), and the first stop moved from "the owner's event after the tree initialisation" to the body composition's
lookup refusal: "InitializeBodyComposition: the new Mesh's animation nodes (ArmBoneBlend, AimOffset, LeftHandGrip,
the head look-at, WillowFootIK and the slot nodes): needs an animation-tree instance for the skeletal mesh, which is
not built yet" (NATIVE_INITIALIZE_BODY_COMPOSITION.md section 7, NATIVE_ANIM_NODE_LOOKUP.md). The branch was deleted
after.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **By signature, not read:** that the event's argument is the initialising component (the decompiler hid the store).
3. **By fit:** the order in Maya's attach, that her `Owner` is set before the tree is instanced and `Pawn.Mesh`
   afterwards (#529, NATIVE_MESH_REST.md).
4. **Not read:** `Pawn.CacheAnimNodes` (native), and whether any class outside those asked overrides
   `PostInitAnimTree` or `ClearAnimNodes`. The implementer's override refusal covers the first for any owner.
5. **By fit:** that the event lookup takes the owner's state first, as a script event does.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. A hook on `PostInitAnimTree` fires once on Maya's pawn per mesh component during her spawn, with her made `Mesh` as
   the argument, before `PostInitBodyComposition`; at that moment `Pawn.Mesh` is not the argument.
2. After the event, `AimOffset` is the instanced tree's `WillowAnimNodeAimOffset` (its outer the `Mesh`),
   `LeftHandIK` the instanced `LeftHandGrip` control, and `ArmBoneBlend` None.
3. On a spawned enemy, the event fires once per animated mesh with `Pawn`'s version only.
