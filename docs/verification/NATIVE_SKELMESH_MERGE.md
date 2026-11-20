# The skeletal mesh merge behind BuildCompositeMesh (draft, 2026-11-20)

AI-assisted (Claude Code, Opus 5.5). Analyst lane ANALYST-SKELMESH-MERGE (decision
2026-11-19-NNN-skelmesh-merge-analyst). Analyst only: a different lane implements anything proposed here (clean
room).

**Sources:**
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (`tools/ghidra/run.ps1`, read-only queries,
  under the machine lock): `WillowWeapon.BuildCompositeMesh`'s merge branch, the merge routine it calls, and that
  routine's other callers;
- the script, swept with `research/script_disasm.py`'s decoder over the nine code packages for every call of
  `BuildCompositeMesh` and `StaticCreateCompositeMesh`;
- the installed packages through `ow-package --def-dump` (the slice gun's type; one field);
- the installed Off `--slice-gate` on main dba540a, run under the machine lock.

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/merge/` and
`local/analysis/merge2/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or
listing structure. **Every rule here is UNVERIFIED** in the running game.

Builds on NATIVE_BUILD_COMPOSITE_MESH.md (section 2, step 4: "two or more: a new merged skeletal mesh is created (the
merge algorithm was not read)") and NATIVE_GESTALT_DATA_LAYER.md, which covers the gestalt path and is not changed
here.

## 0. Where link 8 is: the merge is not on its path

- **The slice gun takes the gestalt path.** `WT_Bandit_AssaultRifle`'s `GestaltMesh` is
  `Weap_AssaultRifles.GestaltDef_AssaultRifle` (re-read for this lane with `--def-dump`), and that definition has a
  `GestaltSkeletalMesh` (NATIVE_BUILD_COMPOSITE_MESH.md section 3). `BuildCompositeMesh` takes the merge branch only
  when the type has **no** usable gestalt mesh, so link 8's weapon never reaches the merge.
- **Main's gate does not stop there.** Installed Off strict `--slice-gate` on main dba540a: link 8 passes `make` (1 of 5
  driver steps) and stops in `init` at "SetSkeletalMesh: a mesh change", after `BuildCompositeMesh` has answered the
  gestalt mesh. The open #645 and #651 move that stop to the new mesh's residency and then to the mesh change's LOD
  row (their PR texts), still past `BuildCompositeMesh`.
- **No recorded run shows the merge refusal.** No branch's GATES rows or decision files record a stop at "a skeletal
  mesh merge", and I found no probe that reaches it. If the hub's reading came from a local probe, it most likely
  changed the weapon or its type's `GestaltMesh`. Worth a look before a lane implements anything below.

**So link 8 needs no merge layer.** The rest of this note says what the merge is, for the weapons and items that do
take that branch (after the Phase 4 gate), and keeps the refusal honest until then.

## 1. Who calls the merge

1. **`WillowWeapon.BuildCompositeMesh`'s non-gestalt branch** (NATIVE_BUILD_COMPOSITE_MESH.md section 2, step 4). The
   part walk collects each part's `NongestaltSkeletalMesh` for parts not in gestalt mode:
   - none: the answer is None;
   - one: that mesh;
   - two or more: the collected list, in walk order, goes to the merge, and its result is the answer.
   The list is a temporary owned by the native and freed afterwards. It is never stored on the weapon.
2. **`WillowInventory.StaticCreateCompositeMesh(array<SkeletalMesh>)`**, a static native. It hands its argument list
   straight to the merge, **with no zero or one shortcut**, so even a one-element list builds a new mesh. The sweep
   found **no script caller** in the nine code packages. Callers outside them (map scripts, Kismet) were not looked
   for.
3. **A third native-only caller**, with the same part-walk shape as the weapon's. It is most likely the
   `WillowItem.BuildCompositeMesh` twin's body (UNVERIFIED: not matched to its exec function).

Script callers of `BuildCompositeMesh` (the sweep): `WillowWeapon.InitMeshes`, `WillowItem.InitMeshes`,
`PlayerStandIn.CloneWeaponMesh` and `PlayerStandIn.CloneItemMesh`, each once, at the start. This matches
NATIVE_BUILD_COMPOSITE_MESH.md section 1.

## 2. What the merge does (read in outline)

With `Sources` the list of meshes:
1. **A new mesh object.** It constructs a new `Engine.SkeletalMesh` object with the transient flag, in the transient
   package, with no name of its own (an engine-generated one), then checks that it is a `SkeletalMesh`. Every call
   makes a new object: nothing is cached or shared between calls, so two weapons with the same parts get two meshes.
2. **The merge job.** It sets up a merge of `Sources` into the new mesh, with **no forced section mapping** and **no
   LODs stripped**, then runs it.
3. **The merge itself** (the stock engine's skeletal mesh merge). Read in outline only; the per-LOD steps are
   recognised by shape, not traced through:
   - it resets the target mesh's own data;
   - the LOD count is the smallest LOD count among the sources. A source with fewer LODs caps the result, and a source
     list with no LOD at all **fails** the merge;
   - it builds one reference skeleton from the sources' skeletons. Bones are matched by name, so a bone the sources
     share appears once (UNVERIFIED: the match key was not traced);
   - for each LOD, it gathers the sources' sections, groups them by material, copies their vertices with bone
     indices remapped to the merged skeleton, and builds the merged index data (UNVERIFIED in detail);
   - it initialises the new mesh's render resources (the vertex and index buffers the renderer draws from).
4. **The answer.** A successful merge returns the new mesh. A failed merge **destroys** the new object (it starts
   its destruction at once) and returns **None**.

**Loads:** none of its own. Every source mesh must be resident **with its native data** (skeleton, LOD models, vertex
data), since the merge reads all of it.

**Writes:**
- a new heap object, together with its whole native payload;
- no write to the weapon, the component or any definition. The caller's `SetSkeletalMesh` later puts the answer on
  the components (NATIVE_SET_SKELETAL_MESH.md).

**Not read:**
- whether the merged mesh gets the sources' `Sockets`, physics asset, morph sets or material list. The mesh's
  script-visible properties other than its data were not traced;
- the bone-match key;
- the exact section grouping;
- what a merge with conflicting bone hierarchies does.

## 3. Is it a renderer-side layer? Mostly, yes

The merge builds a **whole new skeletal mesh**: its reference skeleton, its per-LOD models, its vertex and index data,
and its render resources. The port has none of these for any mesh:
- **No native mesh decode.** The skeletal mesh data layer (decision 2026-11-19-184, `ow-world` `world::skeletal`)
  answers a loaded mesh's reference skeleton and sockets read-only. The skeleton comes from a harness-installed
  `SkeletonSource` (UModel `.psk` exports) and the sockets from the tagged `Sockets` property. Nothing in the tree
  decodes `SkeletalMesh`'s native serialization (rule 4: no public spec, no recovered layout), so a source's LOD
  models and vertex data cannot be read.
- **Nothing to put a merged mesh's data in.** The heap can hold a new `SkeletalMesh` object's tagged properties, but
  not its native payload.
- **The renderer's half** (vertex buffers, sections, materials) is the host's draw path, not the VM's.

So a faithful port of the merge is a mesh-building layer on top of a native mesh decode, which the project does not
have. It belongs with the host's skeletal draw path (decision 2026-11-16-53 used UModel exports for that), not with
the VM.

## 4. The smallest layer, and what can stay absent

**For link 8: nothing.** Section 0: the slice gun never takes this branch.

**If a slice link ever reaches it** (another weapon, or an item through the twin), the merged mesh's later readers
decide what a stand-in must answer:
- **The script and VM side** needs a mesh **object** to hand to `SetSkeletalMesh`, and later its **skeleton and
  sockets** (socket lookups for effects and muzzle locations, the animation tree's bone lookups,
  NATIVE_SET_SKELETAL_MESH.md's mesh change reading the new mesh's skeleton and LOD count).
- **The draw side** needs the LOD models, sections and buffers. That stays `LayerAbsent` in the VM.

**The narrowest stand-in the gate could use honestly** (proposal; **UNVERIFIED**; only if a link needs it):
- a **merged-skeleton record**: a new transient `SkeletalMesh` heap object, plus an entry in the skeletal mesh data
  layer keyed by that object (a `DetMap`). The entry gives the reference skeleton built from the sources' skeletons
  (bones matched by name, in source order, parents remapped), and no sockets until the socket merge is read;
- its LOD count, vertex data and render state stay `LayerAbsent` ("a merged skeletal mesh's LOD models"), so any
  reader of those still refuses;
- the stand-in would be labelled in the harness lines, as the gestalt preloads are.

I don't recommend building it now: no slice link needs it, and the bone-match key and socket rules are not read.

## 5. Proposed Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** It refines what `crates/ow-inventory/src/gestalt.rs` already does. Reference keeps
the stub policy.

**`BuildCompositeMesh`, the non-gestalt branch,** keeps today's walk and refusal, with two precisions:
1. **The walk runs to the end first.** Every part's residency and field shapes are checked (NATIVE_BUILD_COMPOSITE_MESH.md
   section 4, check 4) before the count of collected meshes is looked at. So an unloaded later part is `NotRead`,
   not the merge refusal. Today's code already does this.
2. **Two or more meshes:** `NotYet { LayerAbsent("a skeletal mesh merge") }`, "BuildCompositeMesh: merging the
   parts' meshes", before any write, **without loading or inspecting the source meshes**. Their residency does not
   change the answer, because the layer is absent either way, so the mesh handles stay cold. Zero meshes answer None
   and one answers that mesh, as today: those paths never reach the merge.

**`WillowInventory.StaticCreateCompositeMesh(Meshes)`** (unbound today; bind it only if a link calls it):
1. a `Meshes` argument that is not an array of object references: `VmError::Script`;
2. **any length, including 0 and 1:** `NotYet { LayerAbsent("a skeletal mesh merge") }`,
   "StaticCreateCompositeMesh: merging the meshes", before any write. There is no zero or one shortcut to copy
   (section 1, item 2). With an empty list the game's merge would fail (no LOD) and answer None. The port could
   answer None for an empty list, but that rests on the LOD-count reading alone, so the proposal refuses instead.

**The counters:** every refusal above comes before the first write, so #518's three counters and the name table stay
unchanged, and no "stopped after partial writes: " prefix appears inside the native.

## 6. Test list (synthetic fixtures)

For the implementer, on invented packages (`package()` / `tag()`), each call asserting #518's three counters
(`Heap::committed_writes`, `GameRng::draws`, `DefStore::load_transitions`) and `names.len()` unchanged:
1. A type with no gestalt mesh and **two parts with distinct non-gestalt meshes**: `LayerAbsent("a skeletal mesh
   merge")`. Both mesh handles are still unloaded afterwards.
2. The same with **three** parts, one of them in gestalt mode (not collected): still `LayerAbsent`. With two in
   gestalt mode (one collected): that one mesh is the answer and nothing is written.
3. **Two collected meshes, and an unloaded later part**: `NotRead` (the walk), not the merge refusal.
4. **Two parts naming the same mesh handle**: still `LayerAbsent`, since the collection is a list, not a set
   (UNVERIFIED: the native does not dedupe; nothing in the read collapses duplicates).
5. **The gestalt path** with parts that also carry non-gestalt meshes: never refuses on the merge (the gestalt fill
   as today).
6. `StaticCreateCompositeMesh`, if bound: an empty list, a one-element list and a two-element list each refuse
   `LayerAbsent`. A non-array argument is `VmError::Script`. Reference leaves it unbound.

## Open questions

1. **Not read:** the merge's bone-match key and section grouping, and whether sockets, the physics asset and the
   material list carry over to the merged mesh.
2. **Not counted:** how many weapon types and item definitions take the non-gestalt branch with two or more meshes. A
   `--def-dump` census of every `WeaponTypeDefinition`'s `GestaltMesh`, and of the item definitions behind the twin,
   would say how soon a link could meet the merge.
3. **UNVERIFIED:** that the third native caller is `WillowItem.BuildCompositeMesh`'s body.
4. **Section 0:** which probe, if any, saw link 8 stop at the merge.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `BuildCompositeMesh` (post) for a weapon whose type has no `GestaltMesh` and two meshed parts. Expect a new
   transient `SkeletalMesh` with a generated name, and a different object on each call.
2. Call `WillowInventory.StaticCreateCompositeMesh` with one mesh. Expect a new mesh, not the argument.
3. Read the merged mesh's `Sockets` and bone names against its sources'. That settles open question 1's socket and
   bone parts.
