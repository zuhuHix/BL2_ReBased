# The gestalt component data layer: what fills and copies `GestaltData` (draft, 2026-11-19)

AI-assisted (Claude Code, Claude Opus 5.5), analyst lane ANALYST-GESTALT-DATA-LAYER (decision
`docs/decisions/2026-11/2026-11-19-NNN-gestalt-data-layer-note.md`, number pending). Analyst only: a different lane
implements it (clean room).

**A new note, not an extension.** NATIVE_BUILD_COMPOSITE_MESH.md (#588) and NATIVE_INIT_GESTALT_MESH_DATA.md (#591)
are not on main yet. This note reads the four things they left open and leaves their texts as they are:
- (a) which step writes `PartMeshNames`;
- (b) the socket fill;
- (c) the bounds growth;
- (d) which `GestaltDataContainer` member each write lands in.

Where this note reads further than those two, this note wins. #596 (`fix/rust-gestalt-mesh-layer`) refuses the
layer as `LayerAbsent`. This note is what an implementer needs to build it.

**Sources:**
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock):
  - the script natives `WillowWeapon.BuildCompositeMesh`, `SkeletalMeshComponent.InitGestaltMeshDataFromOther` and
    `GestaltSkeletalMeshDefinition.InitGestaltMeshDataFromParts`, and the routines they reach;
  - for one call, the instructions themselves, to settle which object it writes;
- the installed packages: `tools/ghidra/class_layout.py` for the struct `SkeletalMeshComponent.GestaltDataContainer`,
  its member structs, and `GearboxFramework.GestaltSkeletalMeshDefinition` with its structs;
- the script, read with `research/script_disasm.py` (every function of the nine code packages searched for the
  member names; section 4).

There is no bl-sdk session and no recorded live evidence beyond the read WEAPON_VISUALS.md section 2 records. Raw
output stays under ignored `local/analysis/gdl/`. This note is in our own words: no decompiler output, pseudo-code,
addresses, offsets or listing structure. **Every rule here is UNVERIFIED** in the running game.

## 1. The container, and where each write lands (question d)

The class layout tool cannot lay out `SkeletalMeshComponent`, but it does lay out the struct `GestaltDataContainer`
itself. Its members, in declared order:

| Member | Type |
|---|---|
| `PartMeshNames` | array of names |
| `SkeletalMesh` | `SkeletalMesh` reference |
| `LODModelData` | array of `GestaltLODModel`; each holds `GestaltFragments`, an array of `GestaltLODModelFragment` (`MaterialIndex`, `FirstIndex`, `NumPrimitives`, all ints, in that order) |
| `SocketMappings` | array of `SocketRemapEntry` (`OriginalSocketName`, `MangledSocketName`, names) |
| `GestaltAccessoryList` | array of `GestaltAccessoryEntry` (`TransformIndex`, `OriginalBoneIndex`, `GPUBoneIndex`) |
| `GestaltPartMatricesContainer` | `GestaltPartMatrices` reference |
| `ReferencePoseBounds` | `BoxSphereBounds` (`Origin`, `BoxExtent`, `SphereRadius`) |

**The match is by offset, not by position.**
- Both natives address the container from one fixed base in the component. Every write and every copied member
  falls at the offset the tool computes for one member, with that member's size.
- The copy (section 3) touches all seven, and nothing between them.
- The fill (section 2) touches five of them.

This settles (d) for the layout the packages declare. The component's base offset is a single fit, and it is
consistent with every access.

## 2. The fill (questions a, b and c)

`BuildCompositeMesh` reaches one routine that writes the component. The script native
`GestaltSkeletalMeshDefinition.InitGestaltMeshDataFromParts(component, names)` reaches the same routine, but no
script calls that native (section 4). `BuildCompositeMesh` builds the name list as NATIVE_BUILD_COMPOSITE_MESH.md
section 2 says, calls the routine with the component C and that list, and **writes nothing else to C**.

The routine, with G the gestalt definition and N the name list, in order:
1. **`PartMeshNames` := N.** This is the routine's **first** write: a whole copy of the list, in list order,
   duplicates kept (settles a; the instructions show the copy's target is C's container).
2. **`SkeletalMesh` := G's `GestaltSkeletalMesh`.**
3. **`LODModelData` := empty**, nested fragment arrays included.
4. **`SocketMappings` := empty.**
5. **G's `GestaltSkeletalMesh` None: stop here.** The bounds, the accessory list and the matrices object are left as
   they were.
6. **`LODModelData` := one empty `GestaltLODModel` per entry of G's `GestaltInfos`**, the same count and order.
7. **`ReferencePoseBounds` := a constant start.**
   - `Origin` and `BoxExtent` both take one engine constant vector. It is held in data that is zero in the image and
     set at start-up; by fit it is the zero vector, UNVERIFIED.
   - `SphereRadius` := 0.
8. **The fragments.** For each LOD i, its `GestaltFragments`:
   - every `GestaltPart` of `GestaltInfos[i].Parts` whose `SkeletalMeshFragmentName` equals a name in N (an exact
     name match, number included);
   - in the parts' order, not N's;
   - one fragment per part, even when the name appears twice in N;
   - each fragment gets `MaterialIndex`, `FirstIndex` and `NumPrimitives`, copied from that part.

   The part's `SkeletalMeshFragment` object is not read.
9. **For each name n of N, in list order** (duplicates included, so a duplicate repeats this step):
   1. **The sockets** (settles b). Each entry of G's `GestaltSocketMappings`, in its order, whose
      `SkeletalMeshFragmentName` equals n appends one `SocketRemapEntry` to `SocketMappings`: its
      `OriginalSocketName` and `MangledSocketName`. The entry's `SkeletalMeshFragment` object is not read. So the
      mappings come grouped by name, in N's order.
   2. **The bounds** (settles c). Each entry of G's `GestaltPartBounds`, in its order, whose
      `SkeletalMeshFragmentName` equals n:
      - **the current `SphereRadius` not above 0:** `ReferencePoseBounds` := that entry's `ReferencePoseBounds`,
        whole;
      - **otherwise, the union:**
        - each box is taken as the per-axis minimum and maximum of `Origin − BoxExtent` and `Origin + BoxExtent`, so
          a negative extent still gives a box. The new box is their union;
        - the new `Origin` is its centre, and the new `BoxExtent` its half size;
        - the new `SphereRadius` is the smaller of the new box's half diagonal (the length of the new `BoxExtent`)
          and the larger of (the old origin's distance to the new origin + the old radius) and (the entry origin's
          distance to the new origin + the entry radius).

        All of it is single-precision float with square roots. This is the stock UE3 bounds sum.

**Untouched by the fill:** `GestaltAccessoryList` and `GestaltPartMatricesContainer` (the accessory natives' state; not
read here). **Loads:** none. G is read as it is in memory, and only allocations happen. **Draws:** none.

## 3. The copy (`InitGestaltMeshDataFromOther`)

The copy is as NATIVE_INIT_GESTALT_MESH_DATA.md section 3 says, now confirmed member for member. The four arrays are
copied as values (`LODModelData` with its nested fragment arrays), the two references as references, and the bounds as
seven words, in declared order. Nothing else is written.

## 4. Script readers

Every script function of the nine code packages was disassembled with `research/script_disasm.py` (12,968 of 12,978
decode; the 10 that do not were not searched). The search was for the names `GestaltData`, `PartMeshNames`,
`LODModelData`, `SocketMappings`, `GestaltAccessoryList`, `ReferencePoseBounds`, `GestaltPartMatricesContainer` and
`InitGestaltMeshDataFromParts`. There are **zero hits** for each. Check on the same search: `BuildCompositeMesh` gives
4 functions and `InitGestaltMeshDataFromOther` 2, which matches the callers in #588 and #591. So script reaches the
state only through natives. Whether the socket natives read `SocketMappings` stays open (#588's open question 3).

## 5. The slice

Link 8's gun gives N = 4 names, each a gestalt part with no additional names (NATIVE_BUILD_COMPOSITE_MESH.md section
3). Its definition has 47 part bounds. Its `GestaltInfos`, socket-mapping and part-bounds contents were not read
here; the implementer's fixture uses invented ones.

On the slice, the fill makes:
- `PartMeshNames` the 4 names;
- `SkeletalMesh` the shared gestalt mesh;
- one `GestaltLODModel` per gestalt info, holding the 4 parts' fragments where they are listed;
- the socket mappings of those 4 fragments;
- bounds grown over their part bounds.

The copies to the third-person mesh (and to any extra slot) then hand that value on whole.

## 6. A proposed Off answer, for the implementer

It replaces #596's `LayerAbsent` for both natives. Reference keeps the stub policy. Every rule is UNVERIFIED.

**The layer is the component's `GestaltData` slot in the heap**, one struct value, as #591 section 4 proposes. A slot
is "held whole" when the heap holds it as a decoded `GestaltDataContainer` value with all seven members, the nested
arrays included. **To check first:** whether the store decodes a resident `GestaltSkeletalMeshDefinition`'s
`GestaltInfos`, `GestaltSocketMappings` and `GestaltPartBounds` into heap values. The tool-side schema
(`tools/gestalt-arrays.schema`) is not the heap. Where they are not decoded, refusal F2 applies.

**The fill's preflight** (the gestalt path of #588's preflight, after its checks 1 to 5). It is write-free, load-free
and draw-free, in this order; the first failure is the answer:
1. **C's slot:**
   - C's class declares `GestaltData` as a `GestaltDataContainer`, with the seven members of section 1 at their
     declared types, nested structs included. Otherwise a `VmError::Script` naming the member;
   - the slot is held whole, otherwise `NotYet { NotRead }`, "WillowWeapon.BuildCompositeMesh: the component's
     GestaltData is not held whole (a decode is a write)" (refusal F1).
2. **G's arrays:**
   - `GestaltSkeletalMesh` (an object);
   - `GestaltInfos` (an array of `GestaltInfo`, each `Parts`, an array of `GestaltPart` with a name
     `SkeletalMeshFragmentName` and the ints `MaterialIndex`, `FirstIndex` and `NumPrimitives`);
   - `GestaltSocketMappings` (entries with the names `SkeletalMeshFragmentName`, `OriginalSocketName` and
     `MangledSocketName`);
   - `GestaltPartBounds` (entries with a name `SkeletalMeshFragmentName` and a `BoxSphereBounds`).

   A wrong declaration is a `VmError::Script` naming the field. Arrays the heap does not hold decoded are
   `NotYet { NotRead }`, "WillowWeapon.BuildCompositeMesh: the gestalt definition's arrays are not decoded (a decode
   is a write)" (refusal F2).
3. **The start constant:** the zero vector, as a port input labelled UNVERIFIED (section 2, step 7). No refusal.
4. **The new value:** section 2's steps worked out on a copy of C's current value. `GestaltAccessoryList` and
   `GestaltPartMatricesContainer` are kept; when G's `GestaltSkeletalMesh` is None, so are the bounds. Floats go
   through `ow_core::num` (rule N3). Then the whole value is checked against the declaration with the heap's own
   write rules (`check_value`), which settles that the write cannot fail.

**The fill's write:** one heap write of the whole `GestaltData` slot. The game writes member by member, but no script
runs between those writes, so one atomic write leaves the same state. Then #588's answer (the shared mesh) is
returned.

**The copy** (`InitGestaltMeshDataFromOther`): #591 section 5 as it stands. Its check 4 ("both slots held whole") uses
the same test as refusal F1, with the text "SkeletalMeshComponent.InitGestaltMeshDataFromOther: a GestaltData slot
is not held whole (a decode is a write)". Its write is one whole-slot copy.

**Records:** F1, F2 and the copy's text use the record `docs/verification/NATIVE_GESTALT_DATA_LAYER.md`. #588's and
#591's own refusals keep their texts and records. #596's two `LayerAbsent` refusals are removed once the layer
exists.

**The counters:** both natives refuse only before their single write, so they never mark a refusal "stopped after
partial writes: "; the caller's earlier writes can (#518's counters).

**Tests, on invented classes and an invented gestalt definition** (rule 9):
1. **The fill's order:**
   - N with two names, one of them twice;
   - `PartMeshNames` equal to N, duplicates kept;
   - per LOD, the fragments in the parts' order, one per part;
   - the socket mappings grouped in N's order, a duplicate name's mappings appearing twice;
   - the accessory list and matrices object kept.
2. **The bounds:**
   - a first match copied whole;
   - a second unioned, with invented boxes whose half diagonal is the smaller candidate and others where the
     distance sum is;
   - a negative extent handled;
   - exact single-precision results.
3. **`GestaltSkeletalMesh` None:** names and mesh written, both arrays emptied, bounds and the other two members
   kept.
4. **No match at all:** empty fragment lists per LOD, no mappings, bounds at the start constant with radius 0.
5. **The refusals:** F1, F2 and a wrong member declaration, each before any write, with #518's three counters
   unchanged.
6. **The copy:** after a fill, `InitGestaltMeshDataFromOther` gives an equal value, the two references the same
   objects; and F1's copy text for a slot not held whole.
7. **Reference** binds nothing.

## 7. The probe

Installed Off `--slice-gate`, strict, on origin/main 3ad58b08, under the machine lock. No stand-ins: this lane changes
no code.

**Result:** ok 2, notyet 10, error 2 of 14.

**Link 8** is `notyet` at `init`, passing 1 of its 5 driver steps (`make`). It stops before these natives, at
"stopped after partial writes: WillowWeapon.CacheWeaponCard: the card's slot count" (NATIVE_CACHE_WEAPON_CARD_BODY.md).
The landing run's card PRs move it. The probes of #588 and #591 show what follows: `BuildCompositeMesh`, then
`InitGestaltMeshDataFromOther`, then `SetSkeletalMesh`'s mesh change.

## Open questions

1. **Not read:**
   - the start constant's value at run time (zero by fit);
   - the accessory natives (`InitGestaltAccessoryMeshDataFromParts`);
   - whether the socket natives read `SocketMappings`;
   - where the game draws from `LODModelData`.
2. **UNVERIFIED:** the component's base offset (one fit, consistent with every access); whether duplicate names occur
   in real name lists.
3. **To check first (implementer):** whether the heap holds `GestaltData` and the definition's three arrays decoded
   (section 6).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After `InitMeshes` on link 8's Bandit assault rifle, read `FirstPersonMesh.GestaltData`. Expect:
   - `PartMeshNames` the 4 names;
   - per LOD, the fragments of those parts in the definition's order;
   - `SocketMappings` grouped by name;
   - `ReferencePoseBounds` equal to section 2's union of the 4 parts' bounds.
2. Call `GestaltDefinition.InitGestaltMeshDataFromParts(Component, Names)` from script with a name twice. Expect its
   socket mappings twice and its fragments once.
