# The aim-offset node's profile switch: AnimNodeAimOffset.SetActiveProfileByName (2026-11-25)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK1-AFTER-HOLD (decision 2026-11-25-65), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/aimprof/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule
here is UNVERIFIED in the running game.**

**Why.** A look-ahead for link 1 past `GetWeaponHoldDef` (NATIVE_GET_WEAPON_HOLD_DEF.md, #728). In `maya:8`'s
`WillowPawn.UpdateAnimSets`:
- `BodyWeaponHoldDefinition.GetAnimSets` and the appends to `Mesh.AnimSets` are script;
- the first native with no note is `AnimNodeAimOffset.SetActiveProfileByName`, from
  `WillowPawn.SetAimOffsetProfile(GetAimOffsetProfileForHoldName('Unarmed', None))`, that is, `'Unarmed'`.

`native_index_find.py` has no row for it.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`):
  - `WillowPawn.SetAimOffsetProfile(name)` does nothing without an `AimOffset`. With one, it calls
    `AimOffset.SetActiveProfileByName(name)`, or `('Default')` when the name is None;
  - `AimOffset` is the node the body composition's `PostInitBodyComposition` set from `BodyClass.AimOffsetNodeName`
    (NATIVE_ANIM_NODE_LOOKUP.md). Maya's is her tree's one `WillowAnimNodeAimOffset`.
- **The native:** Ghidra 12.1.4 headless, read-only, under the machine lock:
  - the exec of `SetActiveProfileByName`, and of `SetActiveProfileByIndex` beside it, each calling one virtual;
  - both slots read in the class tables of `AnimNodeAimOffset` and `WillowAnimNodeAimOffset`, found as in
    NATIVE_COMPONENT_SKIP_FLAG.md section 6. **`WillowAnimNodeAimOffset` keeps both of `AnimNodeAimOffset`'s bodies**;
  - the routine the index switch calls after its write was read in outline.
- **Field names:** the local copy of `tools/ghidra/class_layout.py` places `TemplateNode`, `Profiles`,
  `CurrentProfileIndex`, `RequiredBones` and `AimCpntIndexLUT` at one constant shift, where the routines read them.
  An `AimOffsetProfile` entry is 0x7c bytes, with `ProfileName` first.
- **Maya's node**, the template `GD_PlayerShared.Character.AnimTree_Player.WillowAnimNodeAimOffset_1` (export 4522 of
  `GD_Siren_Streaming_SF.upk`):
  - `ow-package --properties 4522 --property-offset 4` lists `Profiles` with **7 elements** (44,644 bytes), with
    status "unsupported". `--def-dump` skips it ("defaults: skipped ... Profiles");
  - the package's name table holds no `ProfileName`, so the elements are serialized as binary structs, not tagged;
  - read from the payload's raw names (local, by fit to the struct's field order), the 7 profiles are **`Default`,
    `Soldier`, `Merc`, `Unarmed`, `Siren`, `Assassin`, `Mechro`**, in that order. `Unarmed` is index 3. Each opens
    with ranges of (-1, 1), and their component counts (27, 27, 26, 11, 26, 26, 26) follow;
  - `CurrentProfileIndex` 0 and `TemplateNode` None on the template.

**A correction to NATIVE_ANIM_NODE_INIT.md section 1** ("her one aim-offset node has an empty `Profiles` in the
template"): **the template's `Profiles` is not empty.** It holds 7 binary-serialized entries that the decoders skip.
The copy fix-up's emptying of the **copy's** own `Profiles` still stands, and so does `TemplateNode` := the template
node.

## 2. What the two natives do

**`SetActiveProfileByName(ProfileName)`:**
1. **The list.** When the node's `TemplateNode` is set, its template's `Profiles`; otherwise the node's own.
2. **The match.** The first entry whose `ProfileName` is the name. With none, nothing happens.
3. **Then `SetActiveProfileByIndex(index)`** of that entry.

**`SetActiveProfileByIndex(ProfileIndex)`** (the same list choice):
1. the index equals `CurrentProfileIndex`, is negative, or is not below the list's length: nothing;
2. otherwise **`CurrentProfileIndex` := the index**;
3. then **the required-bones rebuild** (read in outline; by fit the engine's `UpdateListOfRequiredBones`). With the
   current profile and the node's `SkelComponent` holding a mesh:
   - `AimCpntIndexLUT` is resized to the mesh's bone count and filled with "none";
   - `RequiredBones` is rebuilt from the profile's aim components: each component's bone name is matched in the mesh's
     name-to-bone table, and each matched bone index is added, with its slot in the LUT;
   - the list is then sorted, and its parent bones added (two engine helpers, not read).

   Both arrays are the node's own run-time state. No script reads them on the slice.

**Nothing else is written.** Neither native touches the tree, the component or the pawn.

**What Maya's `maya:8` does here** (with the copy fix-up's `TemplateNode` set):
- the list is the template node's 7 profiles;
- `'Unarmed'` matches index 3, and `CurrentProfileIndex` is 0;
- so **`CurrentProfileIndex` := 3**, and the required bones are rebuilt from `Unarmed`'s 11 aim components against her
  mesh (`Skel_SirenBody`).

## 3. The Off answer (for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `AnimNodeAimOffset.SetActiveProfileByName(name)` and
`SetActiveProfileByIndex(int)` in Off, one rule for both, for `AnimNodeAimOffset` and `WillowAnimNodeAimOffset`.
Reference keeps the stub policy. **Preflight first, then the writes;** load-free (no load, no class-code load,
nothing interned):
1. **The receiver** is resident and an `AnimNodeAimOffset` or below it. It declares `TemplateNode` (an object
   reference), `Profiles` (an array of the profile struct) and `CurrentProfileIndex` (an int). Otherwise
   `VmError::Script` naming the field.
2. **The list's owner:** `TemplateNode` when set, else the receiver. A `TemplateNode` that is a store handle not loaded
   yet is `NotYet { NotRead }`, "SetActiveProfileByName: the aim-offset template node not loaded at set-up (a load is a
   write)".
3. **The profiles.** The port does not decode the binary-serialized `Profiles` elements, so the list's owner holds
   them as not read. While that is so, the answer is **`NotYet { LayerAbsent }`**, "AnimNodeAimOffset: the profiles
   (a binary-serialized struct array)", with layer "a reader for AnimNodeAimOffset.Profiles". The refusal needs to
   know the owner's `Profiles` were skipped, not empty. That needs a store accessor that reports a skipped property
   (the record `--def-dump` logs as "skipped"). **Its shape is the hub's call.** An owner whose `Profiles` was read and
   is empty answers by the rules below (no match, nothing).
4. **With the profiles read:**
   - the name is matched by id, first match wins. No match: nothing (by name); out of range: nothing (by index);
   - the same index as `CurrentProfileIndex`: nothing;
   - otherwise the write is `CurrentProfileIndex` := the index, then the rebuild.
5. **The rebuild,** planned before the write:
   - with no `SkelComponent`, or no mesh on it, the rebuild leaves the two arrays empty, by the routine's own gate
     (read). That is answerable;
   - otherwise it needs the profile's aim-component bone names (the same reader as step 3) and the mesh's reference
     skeleton (`skeletal::reference_skeleton`, with its own refusals);
   - the sort and the parent-adding helpers are not read: **`NotYet { NotRead }`**, "AnimNodeAimOffset: the
     required-bones rebuild's sort and parent pass", raised in the preflight, before `CurrentProfileIndex` is
     written.

So, until the profiles reader and the rebuild's helpers are read, a call that would change the profile on a node
whose `SkelComponent` holds a mesh is a typed `NotYet` with nothing written. A call that changes nothing (the same
index, no match) answers "nothing" once the profiles are read.

**What link 1 would meet** (a prediction, not a run): step 3's `LayerAbsent` on Maya's aim-offset node. Her template
node's profiles are not decoded.

## 4. Tests for the implementer

Synthetic, with `package()` / `tag()`, and profiles given as the port's own values (a toy class, or the reader's
output). Each refusal leaves #518's three counters, the name table and `class_code_loads` unchanged:
1. a node with own profiles `A`, `B`, `C`, no `TemplateNode`, `CurrentProfileIndex` 0, no `SkelComponent`:
   - by name `C`: `CurrentProfileIndex` 2, and the two arrays empty;
   - by name `Z`: nothing;
2. the same with `TemplateNode` set to a node with profiles `X`, `Y`: by name `Y` gives 1, and by name `A` gives
   nothing (the template's list is used);
3. by index:
   - the current index: nothing;
   - -1, or the list's length: nothing;
   - 1: `CurrentProfileIndex` 1;
4. a change on a node whose `SkelComponent` holds a mesh: the rebuild's `NotRead`, before any write;
5. a list owner whose `Profiles` was skipped by the store: the profiles `LayerAbsent`, nothing written;
6. a lazy `TemplateNode`: the not-loaded `NotRead`, nothing loaded;
7. a malformed declaration (`CurrentProfileIndex` not an int, `Profiles` not an array): `VmError::Script`;
8. **maya:8's shape:** a toy `WillowAnimNodeAimOffset` copy with `TemplateNode` set to a template whose profiles are
   `Default`, `Soldier`, `Merc`, `Unarmed`, and `CurrentProfileIndex` 0, called through
   `WillowPawn.SetAimOffsetProfile('Unarmed')`:
   - with no mesh: `CurrentProfileIndex` 3;
   - with a mesh: the rebuild's `NotRead`;
9. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above. The two natives are read, and the rebuild in outline. Maya's profile names are
   read from the payload by fit to the struct's field order.
2. **Not read:** the binary layout of `AimOffsetProfile` past its first fields (a reader is the layer step 3 names),
   and the rebuild's sort and parent pass.
3. **The hub's call:** the skipped-property accessor and the profiles reader.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns unarmed, read her `AimOffset`'s `CurrentProfileIndex`. Expect 3, and the template node's
   `Profiles[3].ProfileName` to be `Unarmed`.
2. Read `Profiles.Length` on the template node and on her copy. Expect 7 and 0.
