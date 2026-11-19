## 2026-11-19: Read InstanceDataHelper.InitializeBodyComposition as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-BODY-COMPOSITION, analyst only. The decision number is pending from
the maintainer (three digits); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_INITIALIZE_BODY_COMPOSITION.md`, and one `NATIVE_INDEX.md`
  row. No code changes.
- **Sources:**
  - the installed script (`research/script_disasm.py`) for the callers and what follows the build;
  - the installed packages (`ow-package --object-dump`, `tools/ghidra/class_layout.py`) for Maya's body class and the
    layouts;
  - `Borderlands2.exe` read locally with the project's Ghidra tooling (read-only, under the machine lock) for the
    build, the per-type creation, the attach routes and the switch packing.

  Raw output stays under ignored `local/analysis/bodycomp/`. The note has no addresses, offsets, listing structure or
  decompiler names. No bl-sdk session took place, so **every rule is UNVERIFIED** in the running game.
- **Clean room:**
  - This lane's author implemented the caller's Off rule earlier (IMPL-INIT-PAWN-MESHES, #522), which refuses before
    the build. This note must be implemented by someone else.
- **Findings for the hub:**
  - Links 6 and 7 never call the build. They share only the component layer with it (a component from a template,
    actor attach, primitive flags).
  - The note corrects three points of NATIVE_INIT_PAWN_MESHES.md section 3:
    - there is no "base first" step;
    - the parent identifier does not pick the parent;
    - Maya's switches are 1 bit wide, and her three switched components are not made on a first build.
  - The zero-width switch fallback is the entry's `Float`.
  - The note proposes the smallest headless layer and a typed Off answer.
  - It leaves three choices to the hub: the validity check, the special parent identifier's refresh, and the head and
    skin customization apply. Maya has both customizations, so that last ruling decides whether links 1, 3, 4, 5, 13
    and 14 get past the build.

**Review r1 of #529 (Sol medium on cdf199be), folded in one commit; the hub's rulings recorded:**
1. A component made from a template counts as valid.
2. A custom head or skin appends its entry and applies nothing.
3. The `Parent` link (the old "special refresh") stays `NotRead`.

All three are port answers, UNVERIFIED.

**Settled in this round:**
- **The special identifier.** It is `Parent`, not `Mesh`, read from the engine's fixed name table (676 `Parent`, 1402
  `CylinderComponent`, 1450 `Mesh`). The step sets the new skeletal component's parent-animation link and rebuilds its
  parent-bone map, read whole. Maya's attachment 1 meets it.
- **The pawn's own hook.** `WillowPawn` implements `IBodyCompositionInstance`, so the build calls the pawn's
  `PostInitBodyComposition` for every made component. That hook:
  - replaces `Pawn.Mesh` (identifier `Mesh`) and `Pawn.CylinderComponent` (identifier `CylinderComponent`);
  - sets `MeshBodyCompositionIndex`, `HeadMesh` and `CollisionComponent`;
  - looks up the animation nodes;
  - calls `ReattachWeapons`.

  Under the rulings, Maya's build now stops first at attachment 0's animation-node lookups (`NotYet { LayerAbsent }`,
  in preflight), then at attachment 1's `Parent` link.
- **The entry contract** per type: what is copied, replaced and defaulted.
  - A component's and a customization's `Replication` is not copied.
  - A switch copies its whole `Data`, including `Object`, and only `Float` is cleared.
  - `BodyCompositionIndex` is copied from `Data`, which holds `i` + 1.
  - Existing entries are copied aside and then `Data` is emptied, so new entries start at index 0. A non-empty `Data`
    (a rebuild) is refused in preflight.
- **Collision:** the exact flags for every value.
- **The attach:**
  - the owner retry passes no socket name;
  - both attempts failing leaves the component and its entry unattached;
  - what each target writes.
- **The switch packing:** a missing entry, a zero width, a value too wide for its width and the near-zero test, each
  exact. A switch child's parent is the switch's own parent.
- **The Off contract** is limited to the pawn wrapper; it keeps the wrapper's root-motion writes.
- **The prerequisite order** is #517, then #522, at the cited revisions. The older note's points that this note
  corrects are listed as superseded.
