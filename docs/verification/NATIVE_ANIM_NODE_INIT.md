# The aim-offset link fix-up and the per-node initialisation (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-AIMOFFSET-REFRESH-AND-NODE-INIT (decision 2026-11-19-180,
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
- **the mesh component** (hub ruling on #566, fit as #563 named `bSkipAllUpdateWhenPhysicsAsleep`): the layout
  tool does not size the component's map field, so #563's local copy of the tool gives it the engine's usual map
  size. Then the code sits a constant 16 bytes past the computed layout. That holds at `SkeletalMesh`, `Animations`
  and `ClothSim` (#563) and is **fitted**. With that shift:
  - the counter A reads is `SkeletalMeshComponent.TickTag`;
  - E's flag is bit 18 of the bool word that holds the cloth flags: `bUpdateComposeSkeletonPasses`. The same word's
    bit 13 is `bRecentlyRendered`, and that is the bit C's and H's "set active child" test for their "skip the blend
    when not rendered" flag. The two uses agree, which supports the fit.

## 1. The aim-offset link fix-up and its refresh (question a)

**Whose it is (correction to #551 §1.2 step 4).** `WillowGame.WillowAnimNodeAimOffset` does not override the fix-up.
It inherits `Engine.AnimNodeAimOffset`'s unchanged (the two classes' interface tables hold the same method there).
The rule is the same. Only the owner of the code differs: it is the engine class's.

**When it runs:** at #551 §1.2 step 4, in gather order, for that node right after its `ParentNodes` rebuild.

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
*Corrected 2026-11-25 by NATIVE_AIMOFFSET_SET_PROFILE.md section 1: the template's `Profiles` holds 7 binary-serialized
entries that the decoders skip, so it is not empty. The copy's own `Profiles` is still emptied, and `TemplateNode`
still points at the template node.*

## 2. The per-node initialisation (question b)

**The call.** #551 §2 step 4: for each node in tick order, the initialisation sets `SkelComponent` and the tick tag,
then calls the node's per-class initialisation with the component (and a parent argument none of these bodies uses).
It is one virtual. Maya's tree has 16 node classes and the root (17 classes), which share **eight** implementations
among them:

| Implementation | Maya's classes that use it (inherited where not their own) |
|---|---|
| A. `AnimNode`'s | (only through the others) |
| B. `AnimNodeBlendBase`'s | `AnimNodeScaleRateBySpeed`, `AnimNodeBlendDirectional`, `Engine.AnimNodeAimOffset` (not in her tree), `WillowAnimNodeAimOffset`, `WillowAnimNode_Condition`, `WillowAnimNode_ClimbLadder` |
| C. `AnimNodeBlendList`'s | `AnimNodeBlendBySpeed`, `AnimNodeBlendByPhysics`, `AnimNodeBlendByPosture`, `WillowAnimNodeBlendInjured`, `WillowAnimNodeBlendThirdPersonMenu` |
| D. `AnimNodeSequence`'s | `AnimNodeSequence`, `WillowAnimNodeSequenceAdditiveBlend` |
| E. `AnimTree`'s | `WillowAnimTree` (the root) |
| F. `AnimNodeBlendPerBone`'s | `AnimNodeBlendPerBone`, `WillowAnimNodeFeatherBlend` |
| G. `AnimNodeAdditiveBlending`'s | `AnimNodeAdditiveBlending` |
| H. `AnimNodeSpecialMoveBlend`'s, on top of `AnimNodeSlot`'s | `AnimNodeSpecialMoveBlend` |

Each step lists what it writes, in order, and what it reads.

**The write order, per node in tick order:** #551's two writes (`SkelComponent`, `NodeTickTag`), then the class's
initialisation below, then the next node. H also initialises some children again, at that point (H step 2). On Maya's
tree no node's initialisation reads a field another node's initialisation writes, so the order does not change her
result.

### A. `AnimNode` (the base every other one runs first)

1. `bRelevant` and `bJustBecameRelevant` := false.
2. `NodeTickTag` := the component's `TickTag` (named by the fitted component layout above). By fit it is the counter
   #551 §2 step 3 already wrote there, so the value does not change. Port answer: the port's tick counter, UNVERIFIED.
3. `NodeTotalWeight` := 0.
4. If `bCallScriptEventOnInit` is set: one script event on the node. By the flag's name it is `OnInit` (UNVERIFIED,
   the name was not read). **No node in Maya's tree sets the flag** (template and class defaults).

Reads: the component's `TickTag` and the node's flag. No bones, sequences or mesh.

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

*Correction 2026-11-26 (NATIVE_UPDATEANIMATIONS_TREE_TICKS.md section 3, #747; lane ANIM-NODE-INIT-D-CORRECTIONS,
decision 2026-11-26-NNN). Two points in step 2 above were wrong:*
- *the "unless" guard needs both conditions: the reset is skipped only when `bIsIssuingNotifies` is set **and**
  `AnimSeqName` is not None. A node issuing notifies with no name is reset;*
- *for `WillowAnimNodeSequenceAdditiveBlend`, the "set animation" is the additive class's override, through the class
  table. After the base step, and whatever the guard:*
  - *with `bAutoGenerateAdditiveSeqName` set, `AddAnimSeqName` := `Add_None`, a name intern;*
  - *then `AddAnimSeqName` := that name (or the node's own value), `AddAnimSeq` := None and `AddAnimLinkupIndex` :=
    -1;*
  - *then, with a non-None name, a `SkeletalMesh` and a non-empty `AnimSets` on the component, the component's
    sequence lookup.*

  *The port's kind D follows these from this date. A lookup match, and the auto-name while issuing notifies under a
  name, are refused (that note's section 5 texts). The intern happens only at the write: the preflight matches by
  text.*

### E. `AnimTree` (and `WillowAnimTree`)

1. B.
2. When a component is given: the component's `bUpdateComposeSkeletonPasses` := true. The name is a layout fit
   (**UNVERIFIED, fitted**: see "How the fields were tied to names"). **No script function reads or writes it** (a
   local scan of every script function, which found no use of `bRecentlyRendered` either). By its name it concerns
   the engine's compose-skeleton passes, render side. On Maya it changes a value: her mesh template
   (`GD_Siren_Streaming.Pawn_Siren.SkeletalMeshComponent_7134`) holds it false.

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
   `TargetWeight[0]` := 1.0 (when there is at least one child). **Maya's three already match.**
4. `Children[0].Weight` := 1 − (the sum of the non-additive weights of the other slots, clamped to 0..1).
5. If the non-additive weights sum to at most 1e-5: the slot's own "set active child" with `TargetChildIndex` and
   a blend time of 0. It can send the owner script events (section 4). **Maya's sum to 1**, so this does not run.
6. `SynchNode` := None.

**`AnimNodeSpecialMoveBlend`'s part:**
7. If `AnimParameters` does not have one entry per child slot: it is emptied and given one zeroed entry per child.
   **Maya's three hold 0 entries for 3 children,** so each gets 3 zeroed entries.
8. For each child slot from the second on, whose entry names a `CustomAnimDef`: a per-bone weight build for that
   entry. It was **not read**, beyond its reading the mesh. After step 7, every entry's `CustomAnimDef` is None on
   Maya's nodes, so this does not run.
9. If the component's `Owner` is set and implements `GearboxFramework.SpecialMoveInterface`: `SMI` := that owner
   (as the interface). Maya's pawn class implements it (through `GearboxPawn`), so `SMI` := her pawn. If the owner
   does not implement it, `SMI` is left as it is.

### What Maya's tree runs, in sum

- A, B, C, D and G write only node fields from section 2, with no mesh read and no script event. E also writes
  the component's fitted flag.
- H writes `AnimParameters` (3 zeroed entries) and `SMI` (her pawn) on each of the 3 special-move nodes, and
  re-runs the initialisation of their children past the first.
- **F needs the mesh's reference skeleton** on 6 nodes.

## 3. Proposed ParityMode Off rules (question c; for a separate implementer)

These replace #551 §5 rule 3's first item ("the per-node initialisation of every node class: `NotYet { NotRead }`")
and refine its rule 2 for the aim offset. **Every rule is UNVERIFIED.** As in #551, everything is planned in a
preflight that writes nothing, loads nothing and draws nothing. Every refusal is raised before the first write, so
none takes the "stopped after partial writes: " prefix.

**The fix-up (replaces #551 rule 2's aim-offset refusal, `AIM_OFFSET` in #561).** It runs at #551 §1.2 step 4, in
gather order, after that node's `ParentNodes` rebuild. It applies to a node of exact class `Engine.AnimNodeAimOffset`
or `WillowGame.WillowAnimNodeAimOffset` (the two classes checked):
- when the copy's outer differs from the source node's, write `TemplateNode` := the source node, then write the copy's
  `Profiles` := an empty array;
- a source node whose class is not `AnimNodeAimOffset` or a subclass cannot occur (the copy has its class): a data
  error if it does.

**The per-node initialisation: a class table, exact classes only.** The implementer cannot see C++ inheritance, so
the preflight maps each node's exact class to one of A to H by the table in section 2 (plus `AnimNodeSlot` → its
part of H, and `AnimNodeBlendBase`, `AnimNodeBlendList`, `AnimNodeSequence`, `AnimTree` to their own
implementations). A class not in the table refuses `NotYet { NotRead }`, "the per-node initialisation of <class>".

**Preflight refusals** (checked over every node in tick order, including the children that H initialises again):
1. A node with `bCallScriptEventOnInit` set: `NotYet { NotRead }`, "the node's OnInit script event (the event's name
   was not read)".
2. D with `AnimSeq` set before the init where the reset runs (`bIsIssuingNotifies` clear, or `AnimSeqName` None;
   corrected 2026-11-26, see D): `NotYet { NotRead }`, "AnimNodeSequence: the steps after its sequence changes". D
   with `ActiveCameraAnimInstance` set: `NotYet { NotRead }`, "AnimNodeSequence: stopping its camera animation".
3. C or H whose non-additive weights sum to at most 1e-5: `NotYet { NotRead }`, "<class>: set active child during
   initialisation".
4. E (the root), hub ruling on #566: the flag has a fit (section "How the fields were tied to names") and no script
   reads it, so **the implementer writes `bUpdateComposeSkeletonPasses` := true, labelled UNVERIFIED (fitted)**. The
   preflight checks that the component's class declares it as a bool; otherwise `VmError::Script` naming the class.
   Without the fit this would have stayed `NotYet { NotRead }`.
5. F with `SkelComponent` and its `SkeletalMesh` both set. While the mesh layer is absent: #529's exact `MESH_LAYER`
   refusal, `NotYet { LayerAbsent("a skeletal mesh data reader (sockets and the reference skeleton)") }`. With no mesh,
   F skips the build and is written as read.
   - **Hub ruling on #566:** the VM path may use the project's own `ow-assets` skeletal reader. It is our code, not a
     dependency, if `crate_edges_test.py` allows the edge or a minimal allowed route exists. That layer is being built
     (IMPL-SKELETAL-MESH-LAYER).
   - **Once the layer lands,** F's two weight builders become `NotYet { NotRead }`, "the per-bone weight build of
     <class>", because the weight rule and the bone lists were not read. **Reading those two builders is the next
     analyst read.**
6. H with an `AnimParameters` entry past the first naming a `CustomAnimDef` after step 7: `NotYet { NotRead }`,
   "AnimNodeSpecialMoveBlend: the per-bone weight build of an entry" (H step 8 was not read).
7. H's owner test (step 9) has three outcomes:
   - **yes:** `SMI` := the owner, as the interface;
   - **no:** `NotYet { NotRead }`, "AnimNodeSpecialMoveBlend: an owner that does not implement SpecialMoveInterface".
     A "no" is not settled in this port, because a class chain may run through an engine stand-in class (decisions
     2026-11-15-08 and 2026-11-15-20);
   - **a check that would resolve interfaces or load anything:** refuse in the preflight, as a 'no' would be.

   `Vm::implements` (`ow-vm` `api.rs`) takes `&mut self` and may resolve class code, so the preflight must plan the
   check without that side effect. It may answer only when the owner class's interfaces are already resolved.

**Writes** (only after the whole preflight passes), in tick order, per node: #551's two writes, then that node's
writes as section 2 lists them, then the next node. H's re-initialised children are written at that point too (their
writes are idempotent the second time). On Maya's tree the order does not change the result.

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
- What E's flag does natively (its name is fitted).

## 5. Maya's next stop (question d)

With #551's copy in place and these rules, her tree passes the fix-up and implementations A, B, C, D, E, G and H.
E writes its fitted flag (refusal 4's ruling). H's owner test says yes for her pawn, provided the pawn class's
interfaces are already resolved.

**Her next stop is refusal 5, the reader, then reading F's two builders:**
- first #529's `MESH_LAYER` refusal, for `AnimNodeBlendPerBone_24` and the five feather blends, because her component
  has a mesh. It is the same layer #529 names for the per-bone control arrays (#551 §2 step 5), so one reader serves
  both;
- once IMPL-SKELETAL-MESH-LAYER lands, F's `NotYet { NotRead }`, "the per-bone weight build of <class>", until an
  analyst reads the two builders.

After that come #551 §2's unread step 5 (the component pass and the per-bone control arrays) and the owner's event
(#551 rule 4).

No local probe was run in this lane: #551's implementation is not on main.

## Open questions

1. **UNVERIFIED:** that the natives reading aim profiles use the template node's (section 1).
2. **UNVERIFIED:** the name of A's script event (by fit `OnInit`); the names `TickTag` and
   `bUpdateComposeSkeletonPasses` (fitted).
3. **Not read:** F's two weight builders and H's per-entry weight build.
4. **Not read:** section 4's list.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On Maya after spawn: her mesh component's `Animations` holds an aim-offset node whose `TemplateNode` is the
   template's `WillowAnimNodeAimOffset_1` and whose `Profiles` is empty.
2. On the instance's sequence nodes: `AnimSeqName` kept, `AnimLinkupIndex` and `AnimSeq` as the component pass left
   them (they show whether the sequences are bound after the init).
3. The three special-move nodes: `SMI` is the pawn, `AnimParameters` has 3 entries.
4. `Child2PerBoneWeight` of the per-bone and feather nodes: one weight per bone of her mesh.

**The refusal texts, ready as constants:** "the per-node initialisation of <class>" · "the node's OnInit script event
(the event's name was not read)" · "AnimNodeSequence: the steps after its sequence changes" · "AnimNodeSequence:
stopping its camera animation" · "<class>: set active child during initialisation" · #529's `MESH_LAYER` "a skeletal
mesh data reader (sockets and the reference skeleton)" · "the per-bone weight build of <class>" ·
"AnimNodeSpecialMoveBlend: the per-bone weight build of an entry" · "AnimNodeSpecialMoveBlend: an owner that does not
implement SpecialMoveInterface".
