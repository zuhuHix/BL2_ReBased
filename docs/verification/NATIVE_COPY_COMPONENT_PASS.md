# The tree initialisation's last steps: the component pass and the per-bone control arrays (2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-COPY-COMPONENT-PASS (decision 2026-11-19-211, number
pending). Analyst only: a different lane implements it (clean room). This thread implemented earlier, unrelated slice
lanes (#573 GetParticleEffect, #584 the damage-type resolver). It does not implement this note, and it read no native
code for those lanes.

**Sources:**
- the script: `SkeletalMeshComponent` declares the native `InitSkelControls`, whose exec calls the same control-array
  builder this note reads. Neither step is script;
- the installed packages through `ow-package --def-dump`: Maya's tree template `GD_PlayerShared.Character.AnimTree_Player`
  and its five controls. A local copy of `tools/ghidra/class_layout.py` gave the field names and order. The copy
  differs only in giving a `MapProperty` a fitted size; the tracked tool stops at one;
- `ow-package --names` over Maya's 107 packages (`CD_Siren_*`, `GD_Siren_Streaming_SF`, `Startup`) for the name
  `MorphSets`;
- Maya's reference skeleton (`Skel_SirenBody`), from the same local UModel `.psk` export NATIVE_PER_BONE_WEIGHTS.md
  used. Only bone names and indices appear here;
- `Borderlands2.exe`, read locally with `tools/ghidra/run.ps1` (read-only queries, under the machine lock). The
  initialisation and the control-array builder were found from the script native `InitSkelControls`. Where the
  decompiler hid which array a helper call works on, a few call sites' bytes were read.

There is no bl-sdk session and no recorded live evidence on these steps. Raw output stays under ignored
`local/analysis/ccp/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

**Builds on:**
- NATIVE_COPY_ANIM_TREE.md (#551, `research/rust-copy-anim-tree` 4a363c0c), section 2, whose step 5 this note reads;
- NATIVE_ANIM_NODE_INIT.md (#566, 07b2dd68), the per-node initialisation;
- NATIVE_PER_BONE_WEIGHTS.md (#580, 5b1a5675), whose section 7 names this step as Maya's next stop;
- #569's skeletal mesh data layer (`fix/rust-skeletal-mesh-layer` 10be0643): `reference_skeleton` and `bone_index`.

**How the fields were tied to names (fitted, UNVERIFIED).** The executable's use of the component's, the tree's and
the controls' data was fitted to the packages' computed class layout. The component's computed layout falls short by
the same constant amount at every field this step touches, and its other fields agree with that shift. The details of
the fit stay under ignored `local/analysis/`. The names and kinds are:
- **the component** (`SkeletalMeshComponent`):
  - `SkelControlIndex` and `PostPhysSkelControlIndex`: arrays of bytes;
  - `SkelControlTickArray`: an array of `SkelControlBase`;
  - `TickTag` and `InitTag`: ints;
  - `MorphSets`: an array of `MorphTargetSet`;
  - `MorphTargetIndexMap`: a native map from names to morph targets, with no script value;
  - `AnimTickRelevancyArray` and `AnimTickWeightsArray`: arrays of ints and of floats;
  - `ParentAnimComponent`, `SkeletalMesh`, `Animations` and `bAnimTreeInitialised`;
  - the owner, `ActorComponent.Owner`;
- **the tree** (`AnimTree`): `SkelControlLists`, an array of structs whose members are `BoneName` (a name) and
  `ControlHead` (a `SkelControlBase`);
- **a control** (`SkelControlBase`): `ControlTickTag` (an int), `NextControl` (a `SkelControlBase`) and
  `bPostPhysicsController` (a bool);
- **a morph target set** (`MorphTargetSet`): `Targets` (an array of `MorphTarget`) and `BaseSkelMesh`;
- **a node** (`AnimNode`): `NodeInitTag`.

The mesh side is read through two things:
- its bone count;
- its bone-name lookup, the same one the script native `MatchRefBone` uses. A None name, or a name not in the mesh,
  finds no bone.

## 1. Where the steps sit

#551 section 2 lists the initialisation in seven steps. Its step 5 ("what follows was not read") is, in order:
1. **the component pass** (section 2 below): the component's morph-target name map is rebuilt from its `MorphSets`;
2. **the morph nodes' initialisation**, only when the component's `Animations` is a tree (section 3);
3. **the per-bone control arrays** (section 4): the same builder the script native `InitSkelControls` runs;
4. then #551's step 6, the owner's event (only with a tree and an owner), and step 7, `bAnimTreeInitialised` set.

**Found on the way, outside step 5.** #551 and #566 do not have these (all UNVERIFIED):
- **The gate field.** The component field that must be clear for the initialisation to run (#551 open question 6)
  fits `ParentAnimComponent`. The initialisation runs only when it is None, `Animations` is set, and the call forces
  it or `bAnimTreeInitialised` is clear.
- **The init tag.** After #551's steps 1 and 2 and before the tick list, the component's `InitTag` becomes the root
  node's `NodeInitTag` plus one. The root is `Animations`, a tree or not.
- **Two per-node arrays.** Between the tick list (#551 step 3) and the per-node loop (#551 step 4), with n the tick
  list's length:
  - `AnimTickRelevancyArray` is emptied and then holds n zeros;
  - `AnimTickWeightsArray` is emptied and then holds n entries whose values are not set there. Whatever writes them
    later was not read.
- **The tick counter.** The counter #551 step 3 raises is the component's `TickTag`. Section 4 raises it once more.

## 2. The component pass (the morph-target map)

1. The component's `MorphTargetIndexMap` is emptied.
2. `MorphSets` is walked **from its last entry to its first**. Empty entries are skipped.
3. In the editor only, a set with no `BaseSkelMesh` gets a prompt that can set it to the component's mesh. The flag
   that gates it fits the engine's editor flag, so by fit the game never takes this branch (UNVERIFIED).
4. A set whose `BaseSkelMesh` is set and is not the component's `SkeletalMesh` is skipped.
5. Otherwise each of the set's `Targets`, in order, is added under its object name, unless the map already holds that
   name. Empty entries are skipped. So among targets of one name, the one in the **latest** set wins, and within a set
   the first.

**Maya:** none of her 107 packages holds the name `MorphSets`, so no component there sets it, and the class default is
taken. By fit that default is empty: UE3 declares no default sets (UNVERIFIED). With no sets, the pass only empties
the map.

## 3. The morph nodes' initialisation

The tree's morph nodes are gathered, by the gather step #551 section 1.4 names, which was not read here either. Each
one gathered gets one per-class initialisation call with the component. **By fit**, a tree with no `RootMorphNodes`
gathers none (UNVERIFIED). #551's preflight rule 2 already refuses any morph node, so a tree that reaches this step
has none.

**Maya:** her template's `RootMorphNodes` is empty (#551 section 3).

## 4. The per-bone control arrays

1. **Emptied first, always:** `SkelControlIndex`, `PostPhysSkelControlIndex` and `SkelControlTickArray` lose every
   entry.
2. **The tree** is the component's `Animations` when that is an `AnimTree` (the instance #551 made), else none. The
   tree is found through one per-class call on `Animations`, read by fit as "this node as a tree" (UNVERIFIED).
3. **The build runs only** with a `SkeletalMesh`, a tree, and at least one `SkelControlLists` entry. Otherwise the
   three arrays stay empty and `TickTag` is not raised.
4. **`SkelControlIndex`** gets one byte per bone of the mesh, every one 255 ("no list").
5. **`TickTag`** goes up by one.
6. **Each list, in array order** (index i):
   1. its `BoneName` is looked up in the mesh. A list whose name finds no bone is skipped;
   2. a list whose bone already holds a list index (not 255) is skipped whole: **the first list for a bone wins**, and
      the later list's controls are neither tagged nor listed;
   3. otherwise `SkelControlIndex[bone]` = i, stored as a byte;
   4. its controls are walked from `ControlHead` along `NextControl` until an empty link. For each control:
      - **listed once:** if its `ControlTickTag` is not the component's `TickTag`, it is set to it and the control is
        appended to `SkelControlTickArray`. A control reached a second time (shared by two chains) is not appended
        again, and a control whose tag already equals the new `TickTag` is not appended either;
      - **post-physics:** the tag-and-list step above comes first, so the post-physics control itself is tagged and
        listed (once, as any other control). Then, if its `bPostPhysicsController` is set, `PostPhysSkelControlIndex` is
        first given one byte per bone, every one 255, unless it already has entries, and
        `PostPhysSkelControlIndex[bone]` = i. **The walk of this chain then stops**, whether or not that control was
        newly listed (a post-physics control shared with an earlier chain stops this one too), so the controls after
        it are not tagged or listed.
7. The bone index is checked against the array's length (an engine assertion). The lookup never gives a bone outside
   the mesh, so this cannot fail on consistent data.

**What each control gets:** `ControlTickTag` and a place in the tick list. The component gets its three arrays and
one `TickTag` step. Nothing is loaded. No script runs, nothing random is drawn, and no other object is written.

**Byte limits.** The list index is a byte, and 255 means no list. With 256 lists or more, a list's index would read as
"no list" or wrap. The engine does not guard against this; the port refuses it (section 6).

## 5. Maya's attachment 0 (names and counts only)

- Her body mesh has 27 bones. Her tree has **4** `SkelControlLists`: `L_Hand` (bone 18), `L_Foot` (4), `R_Foot` (8) and
  `R_Hand` (24), every name found once.
- Her **5 controls**, by `ControlName`:
  - list 0 is `LeftHandGrip`, whose `NextControl` is `LeftHandIK`;
  - lists 1 and 2 are a `WillowFootIK` each;
  - list 3 is `RightHandIK`.
  None of them sets `bPostPhysicsController`.
- **The rule's result** (a structural reading of the packages, not a game run; the controls are #551's copies):
  - `SkelControlIndex`: 27 entries, 255 except bone 18 = 0, bone 4 = 1, bone 8 = 2 and bone 24 = 3;
  - `PostPhysSkelControlIndex`: empty;
  - `SkelControlTickArray`: the 5 copies in the order `LeftHandGrip`, `LeftHandIK`, `WillowFootIK` (L),
    `WillowFootIK` (R), `RightHandIK`, each with `ControlTickTag` = the component's new `TickTag`;
  - `TickTag`: two above its value before the initialisation (#551 step 3, then step 4 above), given the port's
    always-built tick list.
- **The component pass:** no `MorphSets`, so only the map is emptied.

## 6. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces the two refusals #575's implementation raises for #551 section 2
step 5, and the implementer removes both: `COMPONENT_PASS` (`NotYet { NotRead }`) and `BONE_ARRAYS` (`NotYet
{ LayerAbsent }` with `body_composition::MESH_LAYER`).
It runs after #566's per-node initialisation and before #551's rule 4 (the owner's event). Like the rest of #551's
answer, it is planned in the preflight, which writes and loads nothing, and written only after the whole preflight
passed.

**Preflight additions, in this order:**
1. **The component pass.** `MorphSets` must be a dynamic array of object references, else `VmError::Script` naming the
   field. A non-empty `MorphSets` with any set entry refuses `NotYet { NotPorted }`, "the tree initialisation's
   component pass: the morph-target map of MorphSets". The port has no value for the native map, and no slice
   component sets one. An empty list (Maya's) passes and writes nothing.
2. **The morph nodes.** Nothing new: #551 rule 2 has refused every morph node, and an empty `RootMorphNodes` gathers
   none (by fit).
3. **The control arrays:**
   - the three array fields and `TickTag` must exist with the kinds above, else `VmError::Script` naming the field;
   - no mesh, no tree, or no lists: the build is skipped (writes 1 below only);
   - otherwise the bone count comes from #569's `reference_skeleton(vm, mesh)`, called directly. **Its refusals pass
     through unchanged:** the layer's `NotYet` texts and its `VmError::Script`, with nothing loaded;
   - more than 255 lists: `NotYet { NotRead }`, "the per-bone control arrays: a tree with more than 255 control lists
     (the list index is a byte and 255 means none)";
   - each list's `BoneName` must be a name and its `ControlHead` an object reference or None, else `VmError::Script`.
     Names are looked up with `ReferenceSkeleton::bone_index`, as NATIVE_PER_BONE_WEIGHTS.md section 6 does. A None
     name finds nothing;
   - **when it runs:** the copies and the new `Animations` exist only in #561's write phase, so the preflight walks
     the **template's** `SkelControlLists` and controls, read through the copy plan. The plan maps each template
     control to its copy one to one (`NextControl` included), and a copy keeps its template's slot values, so the
     template's `BoneName`s, chains, `bPostPhysicsController`s and saved `ControlTickTag`s are what the copies will
     hold. The simulated tags start from those saved values. The new control tag is the component's `TickTag` + 2:
     #551 step 3 raises it once and section 4 step 5 once more. The writes then go to the copies;
   - each template control reached must have the three control fields with their kinds, else `VmError::Script`.
     Residency is #561's: its gather already checks every control it reaches;
   - **a template control whose `ControlTickTag` equals the component's next control tag** (`TickTag` + 2):
     `NotYet { NotRead }`, "the per-bone control arrays: a template control whose ControlTickTag equals the
     component's next control tag (how the game keeps the copies' tags apart was not read)". It parallels #561's
     `TAG_COLLISION` for nodes: the copy keeps the saved tag, so the tag rule would skip the control and it would be
     dropped from the tick list silently;
   - **a looping chain needs no refusal here.** #561's copy gather (`controls()` in `anim_tree_copy.rs`, #557's chain
     rule) already walks every chain of every `SkelControlLists` entry before the first write, and refuses a control
     reached twice within one chain as `VmError::Script`. A tree that reaches this step has no loop. A control shared
     by two different chains is fine.

   The preflight walks the lists with the rule of section 4, simulating the tags, so it knows every write before the
   first one.

**Writes, in order** (only when the whole preflight passed). #561 and #575 write neither `InitTag` nor the two
per-node arrays, so they are this lane's writes too, at the places section 1 found them:
1. **before #575's per-node loop** (#551 step 4):
   - `InitTag` = the root's `NodeInitTag` + 1, the root being the new `Animations`. In the engine it comes before the
     tick list (#551 step 3). Neither the tick list nor the per-node loop reads `InitTag` (by fit, UNVERIFIED), so
     writing it after the tick list and before the loop gives the same values;
   - `AnimTickRelevancyArray` = n zeros and `AnimTickWeightsArray` = n entries of 0.0, n being the tick list's
     length (each emptied first);
2. after the per-node loop: `SkelControlIndex`, `PostPhysSkelControlIndex` and `SkelControlTickArray` emptied;
3. when the build runs:
   - `SkelControlIndex` = bone count × 255;
   - `TickTag` + 1;
   - per list as section 4 says: the index byte, each newly tagged control's `ControlTickTag` and its append, and
     `PostPhysSkelControlIndex` (sized to bone count × 255 on its first use, then the entry).

The component pass writes nothing for an empty `MorphSets` (the port keeps no map). The preflight checks that
`InitTag`, `AnimTickRelevancyArray` and `AnimTickWeightsArray` exist with the kinds of the field list above, else
`VmError::Script`, and that the root has a `NodeInitTag`. **Port convention, UNVERIFIED:** `AnimTickWeightsArray`'s n
entries hold 0.0, because the engine does not set them at that point.

**The counters.** These are heap writes under #518. Every refusal above is raised in the preflight, before the
initialisation's first write, so none of them takes the "stopped after partial writes: " prefix.

**Reference** keeps the stub policy.

**Tests for the implementer** (invented skeletons and trees, rule 9; each refusal with the heap, the draws and the load
counter unchanged):
1. no mesh, no tree (`Animations` not a tree), or no lists: the three arrays emptied (stale entries removed) and
   `TickTag` unchanged;
2. a 5-bone invented skeleton with 2 lists on bones 1 and 3: indices 0 and 1 there, 255 elsewhere; `TickTag` + 1;
   the chain's controls listed in walk order with the new tag;
3. two lists on one bone: the first list's index; the second list's controls neither tagged nor listed;
4. a None `BoneName` and a name not in the skeleton: those lists skipped, the later lists keep their own indices;
5. one control shared by two chains: listed once;
6. a control whose tag was set earlier in the same build (a control shared with an earlier chain): walked but not
   listed again;
7. a post-physics control in the middle of a chain: it is tagged and listed, `PostPhysSkelControlIndex` = bone
   count × 255 with that bone's entry = the list index, and the controls after it not listed. A second post-physics
   list keeps the earlier entries. A post-physics control shared with an earlier chain: not listed again, the later
   list's bone still gets its `PostPhysSkelControlIndex` entry, and that chain's walk still stops there;
8. 256 lists: the byte-limit refusal, nothing written;
9. a looping chain is still refused by the copy's gather (`VmError::Script`), nothing written;
10. a non-empty `MorphSets`: the NotPorted refusal before any write; an empty one: no write;
11. #569's refusals (a cold mesh, a mesh that is not a `SkeletalMesh`) passed through unchanged;
12. Maya-shaped: 27 invented bone names, 4 lists and 5 controls arranged as section 5 describes, giving section 5's
    three arrays;
13. Reference binds nothing new;
14. a template control whose `ControlTickTag` equals the component's `TickTag` + 2: the collision refusal, nothing
    written;
15. `InitTag` = the root's `NodeInitTag` + 1, and the two per-node arrays at the tick list's length (zeros, and 0.0),
    stale entries removed, written before the per-node loop.

## 7. Maya's next stop

With these rules, her step 5 passes: no `MorphSets`, no morph nodes, and section 5's arrays. Her next stop is **#551
rule 4, the owner's event** after the tree initialisation, whose name was not read ("by fit `PostInitAnimTree`"). It is
sent only with a tree and an owner, as one event call carrying the component, and it runs before
`bAnimTreeInitialised` is set. Reading that event's name is the next analyst step, unless the hub rules it as
`PostInitAnimTree`.

## Open questions

1. **UNVERIFIED:** every rule above. Section 5's arrays are a structural reading of the packages, not a saved oracle:
   the template saves no component.
2. **Not read:** the morph nodes' gather step and per-class initialisation (section 3); what later writes
   `AnimTickWeightsArray`'s values.
3. **UNVERIFIED:** that `MorphSets`' class default is empty (section 2).
4. **UNVERIFIED:** that #569's `bone_index` matches the game's name lookup for `L_Hand`, `L_Foot`, `R_Foot` and
   `R_Hand` (plain names, each once in her skeleton).
5. **Not read:** the other two callers of the control-array builder (by fit `SetSkeletalMesh`'s path, which
   NATIVE_SET_SKELETAL_MESH.md, #583, says runs part of this step).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. On Maya after spawn, `Mesh.SkelControlIndex` has 27 entries, 255 except 18 = 0, 4 = 1, 8 = 2 and 24 = 3, and
   `Mesh.PostPhysSkelControlIndex` is empty.
2. `Mesh.SkelControlTickArray` holds 5 controls of the instanced tree, `LeftHandGrip` first and `RightHandIK` last,
   each with `ControlTickTag` = `Mesh.TickTag`.
3. `Mesh.AnimTickRelevancyArray` and `Mesh.AnimTickWeightsArray` have 66 entries each (the tick list's length).
4. `Mesh.MorphSets` is empty and `Mesh.ParentAnimComponent` is None.
