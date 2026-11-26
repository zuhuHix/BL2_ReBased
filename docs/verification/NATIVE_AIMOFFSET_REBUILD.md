# The aim-offset node's required-bones rebuild (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-AIMOFFSET-REBUILD (decision 2026-11-25-76), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/aimrebuild/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here
is UNVERIFIED in the running game.**

**Why.** With the profiles read (#737), links 1, 3, 4, 5, 13 and 14 stop in `maya:8` at "AnimNodeAimOffset: the
required-bones rebuild's sort and parent pass" (`NotRead`, NATIVE_AIMOFFSET_SET_PROFILE.md step 5). That rule refuses
any profile change on a node whose `SkelComponent` holds a mesh. This note reads the rebuild whole.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The executable** (Ghidra 12.1.4 headless, read-only, under the machine lock):
  - `SetActiveProfileByIndex`'s and `SetActiveProfileByName`'s execs each call one slot of the node's class table. The
    class tables of `AnimNodeAimOffset` and `WillowAnimNodeAimOffset` were found from their registrations and
    constructors (NATIVE_ANALYSIS.md, "When `virtual:` refuses a class"). Both classes hold the same two bodies;
  - the index body is lane F's outline, confirmed: the index check, the `CurrentProfileIndex` write, then one call to
    the rebuild. The rebuild is a plain routine (by fit the engine's `UpdateListOfRequiredBones`), with three callers
    in all. It calls a current-profile getter, the mesh's bone-name lookup, a byte sort and a parent pass, all read;
  - the field reads land, at one constant shift, where `tools/ghidra/class_layout.py` places `TemplateNode`,
    `Profiles`, `CurrentProfileIndex`, `RequiredBones`, `AimCpntIndexLUT` and `SkelComponent` (the shift NATIVE_
    AIMOFFSET_SET_PROFILE.md records). The mesh's reference skeleton and its name table are native-only fields,
    placed by fit with how the routines use them (a count, an element stride and a parent field), **UNVERIFIED**.
- **The slice's data** (a local probe through the port's own readers, `ow_assets::skeletal_lods` and
  `ow_assets::aim_profiles`, on `GD_Siren_Streaming_SF`; counts only, not kept):
  - Maya's mesh `Char_Siren.Mesh.Skel_SirenBody` has 27 bones. Every bone's parent comes before it, and no two bones
    share a name;
  - her `Unarmed` profile (index 3 of her template node's 7) has 11 aim components. All 11 bone names are bones of
    her mesh.

## 2. The rebuild, in order

On the node, once `CurrentProfileIndex` is written:
1. **Both arrays are emptied:** `RequiredBones` and `AimCpntIndexLUT`.
2. **The gate.** It goes on only when all three hold:
   - the current profile exists: the list (the `TemplateNode`'s when set, else the node's own) has an entry at
     `CurrentProfileIndex`;
   - `SkelComponent` is set;
   - its `SkeletalMesh` is set.

   Otherwise it stops here, with both arrays empty.
3. **The lookup table.** `AimCpntIndexLUT` gets one entry per bone of the mesh's reference skeleton, each the "none"
   byte 255.
4. **The components,** in the profile's `AimComponents` order, with their position `i`:
   - the component's `BoneName` is looked up in the mesh's bone-name table (an exact name match: a `None` name, or a
     name the mesh lacks, is no bone);
   - no bone: the component is skipped;
   - bone `b`: `b` is appended to `RequiredBones`, and `AimCpntIndexLUT[b]` := `i`.

   Both are stored as **bytes**. Two components on one bone append it twice, and the table keeps the later position.
5. **The sort:** `RequiredBones` in ascending order (equal bytes are interchangeable, so the algorithm does not matter).
6. **The parent pass,** a walk with a position `k` from 0 while `k` is below the list's current length:
   - the bone `b` at `k`, if it is the root (0) or not below the bone count: `k` moves on;
   - otherwise its parent `p` (from the reference skeleton). When `p` is already anywhere in the list, `k` moves on;
   - otherwise `p` is **inserted at `k`**, before `b`, and `k` stays, so `p` is looked at next.

   So each missing ancestor goes just before the first bone that needs it. **The result is not always sorted:** with
   bones 3 and 4 required, 3 and 1 children of the root and 4 a child of 1, the list ends as 0, 3, 1, 4.
7. Both arrays are trimmed to their lengths, a storage step with no effect on their values.

**Nothing else is written:** not the profile, the component, the mesh or the tree.

**For Maya** (a prediction from the data, not a run): `CurrentProfileIndex` := 3; all 11 components match. The parent
pass inserts 2 ancestors, so `RequiredBones` holds 13 bones, and here it ends in ascending order. `AimCpntIndexLUT` has
27 entries, 11 of them set.

## 3. Proposed Off answer (for the implementer)

This replaces the `REBUILD` refusal in `ow_world::world::aim_offset` (NATIVE_AIMOFFSET_SET_PROFILE.md step 5). It stays
in the preflight, **write-free and load-free** (no store load, no class-code load, nothing interned). The writes come
after it: `CurrentProfileIndex`, then `RequiredBones`, then `AimCpntIndexLUT`.

1. **The gate,** as section 2 step 2. A `SkelComponent` the store has not loaded stays `COMPONENT_NOT_LOADED`
   (`NotRead`). With no component, or a resident one with `SkeletalMesh` None, both arrays are written empty (as
   today).
2. **The mesh's reference skeleton:** `ow_world::world::skeletal::reference_skeleton` on the `SkeletalMesh`. Its own
   refusals stand, with their kinds: no skeleton source installed (`LayerAbsent`), a mesh the store has not loaded
   (the layer's `LayerAbsent`, nothing loaded), and a skeleton that does not parse (`VmError::Script`). On the slice,
   the port reads Maya's skeleton from the package (row SLICE-GATE-READING in GATES.md).
3. **A skeleton of more than 256 bones:** `NotYet { NotRead }`, "AnimNodeAimOffset: a required-bones rebuild past a
   byte". The game stores bone indices and component positions as bytes, and what it does past 255 was not read. The
   same holds for a profile of more than 256 components. Neither is reached on the slice (27 bones, 11 components).
4. **The components' bone names:**
   - a decoded `Profiles` list: each entry's `AimComponents`, each a struct with a `BoneName` (a name), else
     `VmError::Script` naming the field;
   - a list the profiles layer read (`world::aim_profiles`, #737): that layer gives only the profile names today. The
     implementer extends its `ProfileSource` answer with each profile's component bone names, which
     `ow_assets::aim_profiles` already reads (`AimProfile::components[..].bone_name`).
5. **The match:** a bone name against the reference skeleton's bone names, compared as names are (case-insensitive,
   the reader's `Name_N` text form for a numbered name). The skeleton layer's `find_bone` takes the first of two equal
   names, an **UNVERIFIED** choice, as the `Parent` link records; Maya's mesh has none.
6. **The sort and the parent pass,** exactly as section 2 steps 5 and 6, with the skeleton's parent of each bone. The
   root (`parent` None) is bone 0.
7. **The writes:** `RequiredBones` := the list, and `AimCpntIndexLUT` := the table, as byte arrays.

## 4. maya:8 after SetActiveProfileByName

The path past this native is already read: NATIVE_LINK1_TAIL_SETTERS.md section 1 (#734) walks
`ApplyBodyClassStartingValues` to its end, and NATIVE_LINK1_RBPHYSICS_AND_AFTER.md reads on from the collision
setters. Its first item is `WillowPawn.SetUpperBodyMovementBlendState(true)`: `FindAllAnimNodes(Nodes, 'UBMoveBlend')`,
then `SetBlendTarget` on each of Maya's four `UBMoveBlend` nodes. **Predicted next stop** (not a run): whichever of
those notes' natives is still unbound when the rebuild lands.

## 5. Tests for the implementer

Synthetic, with `package()` / `tag()` and a toy skeleton source. Each refusal leaves #518's three counters, the
class-code loads and the name table unchanged:
1. a profile with components on two bones of a toy skeleton: `RequiredBones` holds both and their missing ancestors,
   and `AimCpntIndexLUT` has one 255 per bone, except the two components' positions;
2. a component whose bone the mesh lacks, and one whose name is `None`: skipped, with no list entry and no table
   change;
3. the parent pass's order: bones 3 and 4 required, with 1 and 3 children of the root and 4 a child of 1: the list is
   0, 3, 1, 4;
4. two components on one bone: the bone twice in the list, and the table holds the later position;
5. the gate: no `SkelComponent`, or one with no mesh: both arrays written empty, `CurrentProfileIndex` written;
6. an unloaded `SkelComponent`: `COMPONENT_NOT_LOADED`. An unloaded mesh, or no skeleton source: the skeletal layer's
   `LayerAbsent`. All write-free, with nothing loaded;
7. a skeleton of 257 bones: the byte `NotRead`, write-free;
8. profiles from the profiles layer: the components' bone names come through the source (a toy source), and give the
   same lists as the decoded form;
9. malformed declarations (`AimComponents` not an array of a struct with a name `BoneName`): `VmError::Script`, nothing
   written;
10. link 1's shape: a template node with 7 profiles and `Unarmed` at index 3, with 11 components on a 27-bone toy
    skeleton: `CurrentProfileIndex` 3, the lists the rule gives;
11. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **By fit:** the mesh's native fields (its reference skeleton, the bone count, each bone's parent, the bone-name
   table).
3. **Not read:** what the game does with bone indices or component positions past 255 (the port refuses); which bone a
   duplicated bone name maps to (Maya has none).
