## 2026-11-25: The aim-offset node's required-bones rebuild

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-AIMOFFSET-REBUILD, analyst only, on main 59f0d2ea: another lane
implements it. Raw output stays under ignored `local/analysis/aimrebuild/`. No session was run. Every rule is UNVERIFIED
in the running game.

- **What was decided:** the own-words note NATIVE_AIMOFFSET_REBUILD.md records the rebuild that
  `SetActiveProfileByIndex` runs after writing `CurrentProfileIndex`:
  1. both arrays are emptied;
  2. the gate: a current profile, a `SkelComponent` and its `SkeletalMesh`;
  3. `AimCpntIndexLUT` is one 255 per bone of the mesh;
  4. each aim component's bone, matched by exact name in the mesh's bone-name table, is appended to `RequiredBones`,
     and the table takes the component's position;
  5. an ascending byte sort;
  6. a parent pass that inserts each missing ancestor before its child, so the list is not always sorted.

  Indices are stored as bytes. Nothing else is written.
- **The proposed Off answer:** it replaces the `REBUILD` refusal of NATIVE_AIMOFFSET_SET_PROFILE.md step 5 and stays
  write-free and load-free in the preflight:
  - it reads the mesh's reference skeleton through the skeletal layer, whose refusals and kinds stand;
  - a skeleton or profile past 256 entries is a typed `NotRead`;
  - the profiles layer's source is extended with the component bone names its reader already decodes.
- **For Maya** (a prediction): `Unarmed`'s 11 components all match her 27-bone mesh. The parent pass adds 2 ancestors,
  for 13 required bones. The path past the native is already read by NATIVE_LINK1_TAIL_SETTERS.md.
- **Evidence:** Ghidra 12.1.4 headless read-only under the machine lock: the class tables of both node classes from
  their registrations and constructors, and the rebuild and its four helpers. `tools/ghidra/class_layout.py` gives the
  node's field names at the recorded shift; the mesh's native fields are by fit. A local probe through the port's own
  readers gave the counts, which were not kept.
- **Sensitive files:** none. No code changed.
- **Verified:** nothing in the running game.
