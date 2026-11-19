# Native rules of SkeletalMeshComponent.SetAnimTreeTemplate (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-SETANIMTREETEMPLATE (decision number pending from the hub).

**Sources:**
- the installed script, read with `research/script_disasm.py`;
- the class layout, from the cooked packages through the project's layout tool;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries under the machine lock).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/`. This note is in our own words: no decompiler output, pseudo-code, addresses or listing structure.
Field names come from the cooked packages. **Every rule here is UNVERIFIED** in the running game; the live checks at
the end would confirm them. This lane does not implement them: a fresh implementer codes the Off rules from this note
alone.

**Why it matters.** Slice links 6 and 7 stop in their `make` step at this native, unbound, once the pickup's
inventory accessors are in (decision 2026-11-19-40, PR #487).

## 1. Who calls it on the slice path (script)

`WillowWeapon.AttachMeshToPickup(Pickup)`, when a weapon is dropped as a pickup:
1. makes a new `SkeletalMeshComponent`, with the pickup as its outer and the weapon's `ThirdPersonMesh` as its
   template;
2. calls `SetAnimTreeTemplate(None)` on it;
3. then `SetForceRefPose(true)`, `SetHidden(false)`, `SetOwnerNoSee(false)`, `HideBonesInMesh`, the pickup's
   `AttachComponent`, and sets the pickup's `CollisionComponent` to the new component.

So on the slice path the argument is always **None**, and the component was made one statement earlier and is not
attached yet.

The same native also implements `IAnimBehavior.SetAnimTreeTemplate`, which `Behavior_SetAnimTree` calls with its own
template. The script entry and the interface entry run one body.

## 2. What the native does

`SetAnimTreeTemplate(NewTemplate)`, in order:
1. **The tree teardown** (section 3), always.
2. **With a template:**
   - one of the component's flags is cleared;
   - the template is copied into a new tree instance, outered to the component, or to an engine-wide outer when the
     template carries a certain flag of its own;
   - when the copy succeeds, `AnimTreeTemplate` becomes the template, and the new tree is initialised (one call, not
     read).
   - Which flag is cleared was not settled: the project's layout tool does not place this class's fields exactly,
     and two neighbouring flags fit (open question 3).
3. **With None**, or when the copy fails: `AnimTreeTemplate` becomes None.
4. **The owner's event:** when the component's `Owner` is set, the native sends `Owner` the script event
   `AnimTreeUpdated(SkelMesh)`, with the component as its argument. `Actor.AnimTreeUpdated` is declared with an empty
   body, and no class overrides it in the decoded script (open question 4: the undecoded functions).

It returns nothing.

## 3. The tree teardown

It runs on the component, in order:
1. **The tick list.** For each node in `AnimTickArray` that is of one Engine class (not identified here) and has one
   of its own fields set, one virtual of the node is called (not read).
2. **An engine-wide registry.** A list held outside the component is walked: each entry of two kinds whose subject is
   this component is handled. One kind is removed; the other is removed and then, when the component has a tree,
   checked against it and handed on. The registry's owner and both handlers were not identified (open question 1).
3. **The current tree.** When `Animations` is set, it is asked one question (a virtual, not read). When the answer is
   yes, one more call is made on it (not read).
4. **The writes:**
   - `bAnimTreeInitialised` is cleared;
   - `Animations` becomes None;
   - `AnimTickArray` is emptied;
   - `AnimAlwaysTickArray` is emptied;
   - `SkelControlTickArray` is emptied;
   - `MorphTargetIndexMap` is emptied;
   - `ActiveMorphs` is emptied;
   - `ActiveCurveMorphs` is emptied.

The tick relevancy and weight lists, `AnimSets` and the skeletal mesh are not touched.

## 4. Off answers, step by step (for an implementer)

The hub rules apply:
- preflight what is cheap and certain before the first write;
- a typed `VmError::NotYet` after a write is terminal: prefixed "stopped after partial writes: ", its kind and record
  kept, nothing rolled back;
- a lazy definition load is a write;
- other errors propagate unchanged.

1. **Preflight, before any write, in this order:**
   1. A template argument other than None: `NotYet { NotRead }`, "SetAnimTreeTemplate with a template: the tree copy
      and its initialisation". The copy, the initialisation and the cleared flag were not read.
   2. `AnimTickArray` not empty: `NotYet { NotRead }`, "the tree teardown's per-node call". The node class and the
      call were not read.
   3. `Animations` set: `NotYet { NotRead }`, "the tree teardown's calls on the current tree".
   4. **The registry walk** (section 3, step 2): the port holds no such registry. Default Off answer:
      `NotYet { LayerAbsent }`, "the engine's registry of entries that name a skeletal mesh component". Each is
      certain from the arguments and the component's fields alone, so all four are checked before the first write.
      For the hub: see section 5.
2. **The writes, in section 3's order:** `bAnimTreeInitialised` false; `Animations` None (already None after the
   preflight); the six lists and the map emptied; then `AnimTreeTemplate` None.
   - None of them needs a definition load: the component is a live object, and the argument is None.
3. **The owner's event:** when `Owner` is set, `AnimTreeUpdated(component)` is called on it as a script event through
   the VM (the installed empty body, or an override, run as script). A `NotYet` from it comes after the writes:
   terminal, prefixed. `Owner` None: no call.
4. **Reference:** unchanged (the C++ binds nothing here, so the VM's stub policy answers).

## 5. For the hub: the registry walk

- **The case for "nothing".** In the slice's call the component was made one statement earlier and is not attached.
  The port has no such registry, and no native the port binds adds entries to one. So the walk would find nothing.
- **Why the note does not take it.** The registry's adders were not identified (open question 1). A native that adds
  an entry could be one the port binds later without knowing it.
- **The two options.**
  - Keep the named `LayerAbsent`. Links 6 and 7 then stop here, after the `make` step's writes, so the stop is
    prefixed.
  - Rule "nothing" as a port Off answer for a world without that registry, labelled UNVERIFIED. The next stop would
    then be the next unbound call of `AttachMeshToPickup`.

## Open questions

1. **UNVERIFIED:** the engine-wide registry: who owns it, who adds its two kinds of entries, and what its two
   handlers do.
2. **UNVERIFIED:** the tree teardown's per-node call (its node class and virtual) and its two calls on the current
   tree.
3. **UNVERIFIED:** which flag the template path clears, and the tree copy and initialisation.
4. **UNVERIFIED:** whether any undecoded function overrides `AnimTreeUpdated`.
5. **UNVERIFIED:** what a component made from a template holds in `Animations` and the tick lists in the game (the
   port's `new` decides it at the call).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

1. **The slice's call.** Drop a weapon (or kill an enemy that drops one), and hook
   `WillowWeapon.AttachMeshToPickup`. On the new component, record before and after `SetAnimTreeTemplate(None)`:
   - `AnimTreeTemplate`, `Animations` and `bAnimTreeInitialised`;
   - the lengths of `AnimTickArray`, `AnimAlwaysTickArray`, `SkelControlTickArray`, `ActiveMorphs` and
     `ActiveCurveMorphs`;
   - `Owner`.
2. **The event.** Hook `Actor.AnimTreeUpdated` during that drop. Record whether it fires, on which actor, and with
   which component.
3. **A template.** Hook `Behavior_SetAnimTree.ApplyBehaviorToContext` where the game uses it, and record the same
   fields around the native with a real template: the template set, a new `Animations` whose outer is the component,
   and `bAnimTreeInitialised` afterwards.
