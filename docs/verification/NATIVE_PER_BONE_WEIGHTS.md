# The per-bone weight builders of `AnimNodeBlendPerBone` and `WillowAnimNodeFeatherBlend` (2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-PER-BONE-WEIGHT-BUILDERS (decision 2026-11-19-NNN, number
pending). Analyst only: a different lane implements it (clean room). This thread implemented earlier, unrelated slice
lanes (#570, the body composition's made-Mesh step). It does not implement this note, and it read no native code for
those lanes.

**Sources:**
- the script, through `research/script_disasm.py` and the packages' function tables. Neither class has a script
  function apart from a native `SetBlendTarget`, so both builders are C++;
- the installed packages through `ow-package --def-dump`: Maya's tree template
  `GD_PlayerShared.Character.AnimTree_Player`, its per-bone node `AnimNodeBlendPerBone_24` and its five feather blends
  (`WillowAnimNodeFeatherBlend_0`, `_1`, `_2`, `_4`, `_5`). `tools/ghidra/class_layout.py` gave the field names and
  order;
- Maya's reference skeleton (`Char_Siren.Mesh.Skel_SirenBody`), read locally from a UModel `.psk` export under
  ignored `local/`, the source #569's layer reads. Only bone names and counts appear here;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (`tools/ghidra/run.ps1`, read-only queries, under
  the machine lock). The two classes' C++ interface tables were compared slot by slot with `AnimNodeBlend`'s to find
  what they override.

There is no bl-sdk session and no recorded live evidence on these builders. Raw output stays under ignored
`local/analysis/perbone/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

**Builds on** NATIVE_ANIM_NODE_INIT.md (#566, `research/rust-aimoffset-refresh-and-node-init` 07b2dd68), implementation
F, and on #569's skeletal mesh data layer (`fix/rust-skeletal-mesh-layer` 10be0643, `reference_skeleton`).

**How the fields were tied to names (fitted, UNVERIFIED).** The fields below are named by fitting the executable's
use of each node's data to the packages' computed class layout, with the same correction #566 describes. The details
of that fit stay under ignored `local/analysis/`. The names and kinds are:
- `AnimNodeBlendPerBone`: `BranchStartBoneName` (an array of names), `Child2PerBoneWeight` (an array of floats) and
  `LocalToCompReqBones` (an array of bytes);
- `WillowAnimNodeFeatherBlend`: `BoneBlendTargets` (an array of structs whose members are `StartBoneName`, a name, and
  `BlendWeight`, a float), `bForceLocalSpaceBlend` (a bool), and the same two arrays;
- the component's `SkeletalMesh`, as #566 fitted it.

The mesh side is read through two things:
- the mesh's reference skeleton: its bone count, and each bone's parent index;
- the mesh's bone-name lookup, the same one the component's script native `MatchRefBone` uses.

## 1. When the builders run

Both classes share one initialisation override, the `InitAnim` of #566's implementation F. In order it does:
1. `AnimNodeBlend`'s initialisation (#566, implementation B);
2. the class's own weight builder (sections 2 and 3);
3. `Children[0].Weight` := 1.0.

The builder is called every time, and checks its own condition first:
- **it builds only when the node's `SkelComponent` is set and that component's `SkeletalMesh` is set;**
- otherwise it writes nothing, and both arrays keep the values they had. After #551's copy, those are the template's
  saved values.

The only other callers found are the two classes' editor property-change hooks. They rebuild when one of the node's
branch fields is edited: `BranchStartBoneName` for the per-bone node, and `BoneBlendTargets`, `StartBoneName`,
`BlendWeight` or `bForceLocalSpaceBlend` for the feather blend. They are editor-only and not on the Off path.

## 2. `AnimNodeBlendPerBone`'s builder

**Reads:** `BranchStartBoneName`, the mesh's bone count `n`, and each bone's parent. It does not read
`bForceLocalSpaceBlend`, `Child2Weight` or the children.

**Writes, in order:**
1. `Child2PerBoneWeight` is emptied and given `n` entries of 0.0.
2. Each name in `BranchStartBoneName` is looked up as a bone name. A None name, or a name the mesh does not have,
   gives no bone.
3. For each bone `i` from 0 to `n - 1`, in index order:
   - if `i` is one of the branch bones, its weight is 1.0;
   - otherwise, if `i` is not the root, its weight is its parent's weight. The parent comes earlier in the skeleton,
     so its weight is already final;
   - the root (bone 0), when it is not a branch bone, stays 0.0.
4. `LocalToCompReqBones` is emptied. Then, for each bone `i` in index order, `i` is appended (as a byte) when its
   weight differs from its parent's weight. The comparison is exact. The root counts as its own parent, so the root is
   never appended by this rule.
5. The parents pass (section 4) runs on `LocalToCompReqBones`.

## 3. `WillowAnimNodeFeatherBlend`'s builder

**Reads:** `BoneBlendTargets` (each entry's `StartBoneName` and `BlendWeight`), `bForceLocalSpaceBlend`, the mesh's
bone count `n`, and each bone's parent.

**Writes, in order:**
1. `Child2PerBoneWeight` is emptied and given `n` entries of 0.0.
2. Each entry's `StartBoneName` is looked up as a bone name, as in section 2.
3. For each bone `i` in index order:
   - if an entry names `i`, its weight is that entry's `BlendWeight`, clamped to 0..1. The first such entry in list
     order is the one used. The clamp sends a weight below 0 to 0 and one at 1 or above to 1. A value that is not a
     number becomes 0;
   - otherwise, if `i` is not the root, its weight is its parent's weight;
   - otherwise (the root) it stays 0.0.
4. `LocalToCompReqBones` is emptied.
5. **If `bForceLocalSpaceBlend` is set,** the builder stops here: the list stays empty, and the parents pass does not
   run.
6. Otherwise, for each bone `i` in index order, `i` is appended (as a byte) when either holds:
   - its weight differs from its parent's;
   - the two weights are equal, `i`'s weight is above 0, and the parent's weight is at most 0.99999.

   The root counts as its own parent. So, besides the bones where the weight changes, every bone that has a partial
   weight is listed (a weight above 0 and at most 0.99999).
7. The parents pass (section 4) runs on `LocalToCompReqBones`.

## 4. The parents pass (shared by both builders)

It walks `LocalToCompReqBones` from the front. For the entry at the current place, naming bone `b`:
- if `b` is the root (0), or `b` is not below the bone count, the walk moves on;
- otherwise, if `b`'s parent is already somewhere in the list, the walk moves on;
- otherwise, `b`'s parent is inserted just before `b`, at the current place, and the walk looks at the inserted entry
  next. So the parent's own parent is handled the same way.

**Result, for bone indices a byte can hold (a mesh of at most 256 bones):** every listed bone's ancestors are in the
list, each ahead of its descendants, and the root is in the list whenever any other bone is. The list holds bytes: a
bone index is stored, and compared with the parents, by its low 8 bits. Above that, the guarantee does not hold: a
truncated index names a lower bone, whose ancestors are the ones visited, and the real bone's ancestors may never be.
That is why section 6 refuses such a mesh.

## 5. The rule against Maya's data

**Her skeleton:** 27 bones, every parent before its child. For the examples below: `Root` is bone 0, `Hips` bone 1,
and `Spine1` bone 10.

**`AnimNodeBlendPerBone_24` (`UpperBodySplit`, one branch bone, `Spine1`)** saved both arrays, which gives a
structural oracle:
- **the rule's weights:** 27 entries, 10 at 0.0 and 17 at 1.0. For example, `Hips` (1) is 0.0, and `Spine1` (10) is
  1.0, as is every bone below it;
- **the rule's bone list:** 3 entries, bones 0, 1 and 10. Only `Spine1` differs from its parent, and the parents pass
  adds its two ancestors ahead of it;
- **the saved arrays match exactly:** all 27 weights, value for value, and the 3-entry list. The saved values are what
  the editor's build produced, so this supports the read. It is not a game check.

**The five feather blends** saved both arrays empty, so the packages give no oracle for their builder. What follows
is the rule's prediction only. `bForceLocalSpaceBlend` is false on all five:
- **the four `UBMoveBlend` nodes** have 3 targets each (for example `Spine1` at 0.8 and `Spine2` at 0.9). The rule
  predicts 27 weights, 10 of them 0.0, and a 5-entry list, starting with bones 0, 1 and 10;
- **`UpperBodyFB` (`_1`)** has 7 targets (for example `Spine1` at 0.2 and `Head` at 1.0). The rule predicts 27
  weights, 10 of them 0.0, and a 9-entry list, starting with bones 0, 1 and 10.

## 6. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It replaces the refusal "the per-bone weight build of <class>", which #566 refusal
5 proposed and #575 raises for these two classes. The rest of #566's Off contract is unchanged. As there, everything is
decided in the write-free preflight, before any write. Nothing is loaded there, and nothing is drawn (#518: a lazy load
counts as a write).

**Where it runs.** At every F initialisation, in #566's order: each F node's own place in tick order, and also each
time an H node initialises its children again (#566 H), which can initialise an F node in a later child slot of an H
node a second time. Each of those initialisations that builds is planned on its own.

**The component.** The initialisation's component is the one the caller hands it, not one implementation A assigns:
for an ordinary call, #551 section 2 step 4 supplies it; for H's nested calls, #566 H step 2 does.

**Preflight**, per F initialisation, in this order:
1. **The mesh field.** The component's class declares `SkeletalMesh` as an object reference, and its value is one;
   otherwise `VmError::Script`, naming the class and the field. Only a real null reference (`Value::Object(None)`)
   skips the build: then there is no build and no refusal, both arrays keep their values, and F's other steps still
   run, `Children[0].Weight` included. Any other value (a missing field, or a value of another kind) is that error,
   never a skip.
2. **The skeleton.** For a non-null mesh, #569's `reference_skeleton(vm, mesh)` is called directly. Its errors pass
   through unchanged: `NotYet` with kind `LayerAbsent` (no source installed, or no export for the mesh), `NotYet` for a mesh
   the store has not loaded, and `VmError::Script` (not a `SkeletalMesh`, or an export that fails to parse). #569
   checks the class before the load state, so no separate unloaded-handle check is added here; one would change which
   error wins.
3. **The byte lists.** A mesh with more than 256 bones is `NotYet` with kind `NotRead`, "the per-bone weight build of
   <class>: a mesh with more than 256 bones (the bone lists hold bytes)". The game's low-8-bit storage is not
   reproduced. Maya's mesh has 27 bones.
4. **The builder's fields and their values.**
   - The node's class declares the fields the builder reads and writes, with these kinds:
     - `BranchStartBoneName`: an array of names;
     - `BoneBlendTargets`: an array of structs with the members `StartBoneName` (a name) and `BlendWeight` (a float);
     - `bForceLocalSpaceBlend`: a bool (feather blend only);
     - `Child2PerBoneWeight`: an array of floats;
     - `LocalToCompReqBones`: an array of bytes.
   - The values read are of those kinds too: every element of `BranchStartBoneName` a name, and every element of
     `BoneBlendTargets` a struct whose two members hold a name and a float.
   - Otherwise `VmError::Script`, naming the class and the field (or the member).
5. **The rules.** Both arrays are computed in the preflight from sections 2 to 4:
   - names are looked up with `ReferenceSkeleton::bone_index`, case-insensitively as names compare. A None name finds
     no bone;
   - the root's parent (`None` in #569) counts as the root itself;
   - the clamp and the 0.99999 bound are as in section 3.

**Writes:** two plain property writes per F initialisation that builds, H's nested ones included, at F's place in
#566's order: after implementation B's writes, and before `Children[0].Weight`.
1. `Child2PerBoneWeight` := the computed weights (`n` floats).
2. `LocalToCompReqBones` := the computed list (bytes).

**Tests the implementer can build** (invented skeletons and nodes):
- a chain with one branch bone, and the weights it inherits down the chain;
- the root as a branch bone, which gives an empty list;
- a name the mesh lacks, and a None name;
- two branch names for one bone;
- for the feather blend:
  - the clamp, including a negative weight, a weight above 1 and one that is not a number;
  - two entries naming one bone (the first wins);
  - a partial weight inherited unchanged, which is still listed;
  - `bForceLocalSpaceBlend` set, which leaves the list empty;
- the parents pass inserting a missing ancestor chain ahead of its bone;
- a null mesh, with both arrays left untouched; a mesh field that is missing or holds another kind, refused as
  `VmError::Script`;
- a `BoneBlendTargets` element whose members hold the wrong kinds, refused;
- an F node under an H node, built at H's nested initialisation and again at its own;
- more than 256 bones, refused;
- the layer's own refusal passing through;
- each refusal with the heap, the draws and the load counter unchanged.

**Reference** keeps the stub policy.

## 7. Maya's next stop

With these rules, her six F nodes build from her 27-bone skeleton, once #569's layer has her mesh's export. Then
#566's preflight goes on through her tree in tick order:
- H's owner test says yes for her pawn, provided the pawn class's interfaces are already resolved;
- H's per-entry build (step 8) does not run on her nodes, because every entry's `CustomAnimDef` is None after step 7.

**Her next stop** is the unread work #566 section 5 names after F:
- #551 section 2's step 5: the component pass and the per-bone control arrays. These need the same mesh layer;
- then the owner's event (#551 rule 4).

The first of these is the refusal the implementation raises for #551 section 2 step 5. Its text is that lane's, not
this note's.

## 8. What is still unread

- What `LocalToCompReqBones` and `Child2PerBoneWeight` do at blend time, and what the per-bone node's
  `bForceLocalSpaceBlend` does (its builder does not read it).
- The two classes' native `SetBlendTarget`.
- `AnimNodeSpecialMoveBlend`'s per-entry build (#566 implementation H, step 8). Maya does not run it.

## Open questions

1. **UNVERIFIED:** every rule above. The per-bone builder is supported by Maya's saved arrays (an editor build, not a
   game run). The feather builder has no oracle in the packages.
2. **UNVERIFIED:** that the game's bone-name lookup matches `bone_index` for every name Maya's nodes use. Her names
   are plain, with no number suffix, and each exists once in her skeleton.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, on her mesh component's animation tree instance, the `UpperBodySplit` per-bone node:
   `Child2PerBoneWeight` has 27 entries (10 at 0.0, 17 at 1.0), and `LocalToCompReqBones` has 3 (bones 0, 1 and 10).
2. Each `UBMoveBlend` feather node: 27 weights, with `Spine1` (10) at 0.8; a 5-entry list.
3. The `UpperBodyFB` feather node: 27 weights, with `Spine1` (10) at 0.2; a 9-entry list.
4. Optional: set one feather node's `bForceLocalSpaceBlend` in a session and reinitialise the tree; its list is then
   empty.

**The refusal text, ready as a constant:** "the per-bone weight build of <class>: a mesh with more than 256 bones (the
bone lists hold bytes)". The layer's own refusals keep #569's texts.
