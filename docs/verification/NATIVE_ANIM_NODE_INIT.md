# The aim-offset link fix-up and the per-node initialisation (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-AIMOFFSET-REFRESH-AND-NODE-INIT (decision 2026-11-19-NNN,
number pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed packages through `ow-package --def-dump` (Maya's tree template `GD_PlayerShared.Character.AnimTree_Player`
  and its 66 nodes: classes, saved values) and `tools/ghidra/class_layout.py` (field names and order);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock), plus
  a local read of the node classes' C++ interface tables from the image, to see which classes share an implementation;
- none of it is script: the fix-up and the initialisation are C++ virtuals.

There is no bl-sdk session and no recorded live evidence on these steps. Raw output stays under ignored
`local/analysis/aimoff/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

**Builds on** NATIVE_COPY_ANIM_TREE.md (#551, `research/rust-copy-anim-tree` 4a363c0c, accepted). Section numbers
"#551 §x" point there.

**How the fields were tied to names.** Every field named below is named by the computed layout of
`class_layout.py`, checked against the executable's use of it:
- the fields before the first 16-byte-aligned struct (`CachedRootMotionDelta`, a `BoneAtom`) agree exactly, for
  example `SkelComponent`, the `AnimNode` flag word (its bit `bCallScriptEventOnInit` guards the script event),
  `NodeTotalWeight` and `CachedNumDesiredBones`;
- the fields after it sit one word later in the executable than the computed layout says, uniformly. That fits the
  struct's 16-byte alignment, which the layout tool does not model. With that shift, the struct sizes agree
  (`AnimBlendChild` 28 bytes, `AimOffsetProfile` 124, `SlotAnimParameters` 68), and so do the places of the arrays
  inside them and the flag bits read (`bIsAdditive`, `bIsIssuingNotifies`, `bPlayActiveChild`);
- one component field and one component flag are **not identified** (named as such below).

## 1. The aim-offset link fix-up and its refresh (question a)

**Whose it is (correction to #551 §1.2 step 4).** `WillowAnimNodeAimOffset` does not override the fix-up. It inherits
`AnimNodeAimOffset`'s unchanged (the two classes' interface tables hold the same method there). The rule is the same.
Only the owner of the code differs: it is the engine class's.

**The fix-up, in order:**
1. **The gate:** only when the copy's `Outer` differs from its source node's `Outer`. For a copy outered to the mesh
   component this always holds (#551 §1.2).
2. **`TemplateNode`** := the source node (the template's node), through a checked cast to `AnimNodeAimOffset`. The
   cast cannot fail here, since the copy has the source node's class. A failed cast would be the engine's cast
   error, which no Maya case reaches.
3. **The refresh** (the call #551 left unread): **the copy's own `Profiles` array is emptied.** Every entry is
   destroyed, its two inner arrays (`AimComponents`, `AimTransformsCache`) included. Then the count goes to 0 and
   the storage is released. Nothing else is read or written. It does not rebuild or look anything up.

**What it means, by fit (UNVERIFIED):** after the copy, an aim-offset instance keeps no profiles of its own and
points at its template node. The natives that read profiles would then read them through `TemplateNode`. Those
natives were not read here.

**On Maya's tree:** her one aim-offset node (`WillowAnimNodeAimOffset_1`) has an empty `Profiles` in the template,
and so does its class default. So the refresh changes no value on her copy. `TemplateNode` goes from None to the
template node.

## 2. The per-node initialisation (question b)

**The call.** #551 §2 step 4: for each node in tick order, the initialisation sets `SkelComponent` and the tick tag,
then calls the node's per-class initialisation with the component (and a parent argument none of these bodies uses).
It is one virtual. Maya's 17 node classes, and the tree root's class, share **eight** implementations among them:

| Implementation | Maya's classes that use it (inherited where not their own) |
|---|---|
| A. `AnimNode`'s | (only through the others) |
| B. `AnimNodeBlendBase`'s | `AnimNodeScaleRateBySpeed`, `AnimNodeBlendDirectional`, `WillowAnimNodeAimOffset`, `WillowAnimNode_Condition`, `WillowAnimNode_ClimbLadder` |
| C. `AnimNodeBlendList`'s | `AnimNodeBlendBySpeed`, `AnimNodeBlendByPhysics`, `AnimNodeBlendByPosture`, `WillowAnimNodeBlendInjured`, `WillowAnimNodeBlendThirdPersonMenu` |
| D. `AnimNodeSequence`'s | `AnimNodeSequence`, `WillowAnimNodeSequenceAdditiveBlend` |
| E. `AnimTree`'s | `WillowAnimTree` (the root) |
| F. `AnimNodeBlendPerBone`'s | `AnimNodeBlendPerBone`, `WillowAnimNodeFeatherBlend` |
| G. `AnimNodeAdditiveBlending`'s | `AnimNodeAdditiveBlending` |
| H. `AnimNodeSpecialMoveBlend`'s, on top of `AnimNodeSlot`'s | `AnimNodeSpecialMoveBlend` |

Each step lists what it writes, in order, and what it reads.

### A. `AnimNode` (the base every other one runs first)

1. `bRelevant` and `bJustBecameRelevant` := false.
2. `NodeTickTag` := a component counter, by fit the tick counter that #551 §2 step 3 already wrote there, so the
   value does not change (UNVERIFIED: the counter was not identified by name).
3. `NodeTotalWeight` := 0.
4. If `bCallScriptEventOnInit` is set: one script event on the node. By the flag's name it is `OnInit` (UNVERIFIED,
   the name was not read). **No node in Maya's tree sets the flag** (template and class defaults).

Reads: the component's counter and the node's flag. No bones, sequences or mesh.

### B. `AnimNodeBlendBase`

1. A.
2. For each child slot: if its `AdditiveWeightScale` is exactly 0, it becomes 1.0. **No child in Maya's tree has 0.**

Reads: `Children`. Nothing else.

### C. `AnimNodeBlendList`

1. B.
2. If `TargetWeight` does not have one entry per child slot: it is emptied, given one zeroed entry per child, and
   `TargetWeight[0]` := 1.0 (when there is at least one child). **All of Maya's list nodes already match.**
3. If the children's weights, summed over the slots not marked `bIsAdditive`, are at most 1e-5: the list's own
   "set active child" with `ActiveChildIndex` and a blend time of 0 (sketched in section 4). **Every list node of
   Maya's sums to 1**, so this does not run.

Reads: `Children`, `TargetWeight`, `ActiveChildIndex`.

### D. `AnimNodeSequence`

1. A.
2. `AnimSeqName` is kept aside. Then the node's own "set animation" runs with the name None:
   - **unless** `bIsIssuingNotifies` is set (then it does nothing at all);
   - `AnimSeqName` := None, `AnimSeq` := None, `AnimLinkupIndex` := -1;
   - with the name None no sequence is looked up, so the mesh and the anim sets are not read;
   - only if `AnimSeq` was set before does the change run more steps: a call on the node and a pass over the old
     sequence's meta-data objects. Those were not read.
3. `CachedNumDesiredBones` := 0.
4. `AnimSeqName` := the kept name.
5. If `ActiveCameraAnimInstance` is set: that instance is stopped (a call, a flag on it), and the field := None.

**Net effect:** the sequence keeps its name but holds no sequence and linkup index -1. So the initialisation does
not bind sequences to the mesh. Where that happens later (by fit the component's pass over its anim sets, #551 §2
step 5) was not read. On Maya's 31 sequence nodes `AnimSeq` and `ActiveCameraAnimInstance` are None and
`bIsIssuingNotifies` is clear. So only the plain writes above happen.

### E. `AnimTree` (and `WillowAnimTree`)

1. B.
2. When a component is given, one flag bit in a component bit field is set. It is **not identified**: the layout
   tool cannot lay out `SkeletalMeshComponent` (it has a map field).

### F. `AnimNodeBlendPerBone` (and `WillowAnimNodeFeatherBlend`)

1. B.
2. **The per-bone weight list** (each class has its own builder; both read the mesh):
   - it runs only when the node's `SkelComponent` and that component's `SkeletalMesh` are both set;
   - it reads the mesh's reference skeleton (the bone count, and each bone's parent);
   - it reads the node's branch names, looked up as bone names: `BranchStartBoneName` for `AnimNodeBlendPerBone`,
     `BoneBlendTargets` (start bone and blend weight) for the feather blend;
   - it rewrites `Child2PerBoneWeight` with one weight per bone, and the bone lists that follow (not read in detail).
3. `Children[0].Weight` := 1.0.

**Reads the mesh.** Maya has 1 per-bone node and 5 feather blends, and her mesh component has a mesh.

### G. `AnimNodeAdditiveBlending`

1. B.
2. `Children[0].Weight` := 1.0 if it is not already. **All five of Maya's are 1.**

### H. `AnimNodeSpecialMoveBlend` (on top of `AnimNodeSlot`)

**`AnimNodeSlot`'s part:**
1. B.
2. **The children past the first are initialised again:** for each child slot from the second on with a node, that
   node's `SkelComponent` := the component, and its own per-class initialisation runs (with no parent). So those
   nodes run their initialisation twice: here, and in tick order.
3. If `TargetWeight` does not have one entry per child slot: it is emptied, given one zeroed entry per child, and
   `TargetWeight[0]` := 1.0. **Maya's three already match.**
4. `Children[0].Weight` := 1 − (the sum of the non-additive weights of the other slots, clamped to 0..1).
5. If the non-additive weights sum to at most 1e-5: the slot's own "set active child" with `TargetChildIndex` and
   a blend time of 0. It can send the owner script events (section 4). **Maya's sum to 1**, so this does not run.
6. `SynchNode` := None.

**`AnimNodeSpecialMoveBlend`'s part:**
7. If `AnimParameters` does not have one entry per child slot: it is emptied and given one zeroed entry per child.
   **Maya's three hold 0 entries for 3 children,** so each gets 3 zeroed entries.
8. For each child slot from the second on, whose entry names a `CustomAnimDef`: a per-bone weight build for that
   entry (it reads the mesh). After step 7, every entry's `CustomAnimDef` is None on Maya's nodes, so this does not
   run.
9. If the component's `Owner` is set and implements `GearboxFramework.SpecialMoveInterface`: `SMI` := that owner
   (as the interface). Maya's pawn class implements it (through `GearboxPawn`), so `SMI` := her pawn. If the owner
   does not implement it, `SMI` is left as it is.

### What Maya's tree runs, in sum

- A, B, C, D, E and G write only node fields from section 2, with no mesh read and no script event.
- H writes `AnimParameters` (3 zeroed entries) and `SMI` (her pawn) on each of the 3 special-move nodes, and
  re-runs the initialisation of their children past the first.
- **F needs the mesh's reference skeleton** on 6 nodes.

## 3. Proposed ParityMode Off rules (question c; for a separate implementer)

These replace #551 §5 rule 3's first item ("the per-node initialisation of every node class: `NotYet { NotRead }`")
and refine its rule 2 for the aim offset. **Every rule is UNVERIFIED.** As in #551, everything is planned in a
preflight that writes nothing, loads nothing and draws nothing. Every refusal is raised before the first write, so
none takes the "stopped after partial writes: " prefix.

**The fix-up (replaces #551 rule 2's aim-offset refusal).** For a node of class `AnimNodeAimOffset` or a subclass that
does not override it (`WillowAnimNodeAimOffset`):
- when the copy's outer differs from the source node's, write `TemplateNode` := the source node, then write the copy's
  `Profiles` := an empty array;
- a source node whose class is not `AnimNodeAimOffset` or a subclass cannot occur (the copy has its class): a data
  error if it does.

**The per-node initialisation: a class table, exact classes only.** The implementer cannot see C++ inheritance, so
the preflight maps each node's exact class to one of A to H by the table in section 2 (plus `AnimNodeSlot` → its
part of H, and `AnimNodeBlendBase`, `AnimNodeBlendList`, `AnimNodeSequence`, `AnimTree` and their implementations
for themselves). A class not in the table refuses `NotYet { NotRead }`, "the per-node initialisation of <class>".

**Preflight refusals** (checked over every node in tick order, including the children that H initialises again):
1. A node with `bCallScriptEventOnInit` set: `NotYet { NotRead }`, "the node's OnInit script event (the event's name
   was not read)".
2. D with `AnimSeq` set before the init: `NotYet { NotRead }`, "AnimNodeSequence: the steps after its sequence
   changes". D with `ActiveCameraAnimInstance` set: `NotYet { NotRead }`, "AnimNodeSequence: stopping its camera
   animation".
3. C or H whose non-additive weights sum to at most 1e-5: `NotYet { NotRead }`, "<class>: set active child during
   initialisation".
4. E (the root): the component flag is not identified. `NotYet { NotRead }`, "the animation tree's flag on the mesh
   component", unless the hub rules that the port leaves it out, labelled UNVERIFIED (it is not a field the port
   models).
5. F with `SkelComponent` and its `SkeletalMesh` both set: `NotYet { LayerAbsent("a skeletal mesh data reader (sockets
   and the reference skeleton)") }`, the layer #529 already names. With no mesh, F skips the build and is written
   as read.
6. H with an `AnimParameters` entry past the first naming a `CustomAnimDef` after step 7: the same `LayerAbsent`.
7. H's owner test needs the owner's class interfaces. If the port cannot tell whether a class implements
   `SpecialMoveInterface` from what is loaded, refuse `NotYet { LayerAbsent("the class's implemented interfaces") }`
   rather than guess.

**Writes** (only after the whole preflight passes), in tick order: each node's writes as section 2 lists them, with
H's re-initialised children written at that point too (their writes are idempotent the second time).

**Tests the implementer can build** (synthetic packages): one per implementation A to H, each showing its writes; the
AimOffset fix-up (`TemplateNode` set, `Profiles` emptied, nothing when the outers match); each refusal above, with the
heap, the draws and the load counter unchanged; an unknown node class refusing by name; H's double initialisation;
F with no mesh doing no build; Reference unbound.

**Reference** keeps the stub policy.

## 4. What is still unread

- C's and H's "set active child" in full: read for a blend time of 0, apart from C's tail when `bPlayActiveChild` is
  set and H's owner script events. Neither runs on Maya's tree.
- D's steps when `AnimSeq` changes, and stopping a camera animation.
- F's bone lists after the weights.
- H's per-bone weight build per entry (step 8).
- The component flag of E.

## 5. Maya's next stop (question d)

With #551's copy in place and these rules, her tree passes the fix-up and implementations A, B, C, D, G and H.
**Her next stop is refusal 5:** `NotYet { LayerAbsent("a skeletal mesh data reader (sockets and the reference
skeleton)") }`, for `AnimNodeBlendPerBone_24` and the five feather blends, because her component has a mesh. It is
the same layer #529 names for the per-bone control arrays (#551 §2 step 5), so one reader serves both.

**Refusal 4 comes earlier**, since the root is first in tick order and the preflight checks in that order, unless the
hub rules to leave the component flag out. So the order of her stops is: refusal 4 (needs a hub ruling), then refusal
5 (needs the reader).

**For the hub:** the host side already has a skeletal mesh reader (`ow-assets` `skeletal.rs`). Whether the VM path
may use it is a lane decision, not read here. After the reader come #551 §2's unread step 5 (the component pass and
the per-bone control arrays) and the owner's event (#551 rule 4).

No local probe was run in this lane: #551's implementation is not on main.

## Open questions

1. **UNVERIFIED:** that the natives reading aim profiles use the template node's (section 1).
2. **UNVERIFIED:** the name of A's script event (by fit `OnInit`), and the component counter A writes.
3. **Not identified:** the component flag of E.
4. **Not read:** section 4's list.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On Maya after spawn: her mesh component's `Animations` holds an aim-offset node whose `TemplateNode` is the
   template's `WillowAnimNodeAimOffset_1` and whose `Profiles` is empty.
2. On the instance's sequence nodes: `AnimSeqName` kept, `AnimLinkupIndex` and `AnimSeq` as the component pass left
   them (they show whether the sequences are bound after the init).
3. The three special-move nodes: `SMI` is the pawn, `AnimParameters` has 3 entries.
4. `Child2PerBoneWeight` of the per-bone and feather nodes: one weight per bone of her mesh.
