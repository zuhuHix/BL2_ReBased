## 2026-11-19: The rest of the made-Mesh step, read as analyst

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-MESH-REST, links 1, 3, 4, 5, 13 and 14. Analyst note only
(clean room): docs/verification/NATIVE_MESH_REST.md. This lane's author implemented #542 and #557, so a separate lane
implements the note. Sources:
- the script, read with `research/script_disasm.py`, plus a local scan of every script function;
- the installed packages, through `ow-package --def-dump` and the layout tool;
- local, read-only Ghidra analysis of the executable under the machine lock.

Raw output stays under ignored `local/analysis/mesh-rest/`. No live session was run.

- **Read (native):** for identifier `Mesh`, `WillowPawn`'s `PostInitBodyComposition` does the following, in order:
  1. releases the old `Pawn.Mesh`;
  2. sets `Pawn.Mesh` to the made object as a skeletal mesh component;
  3. when that is set: `MeshBodyCompositionIndex` = the attachment index as a byte, the mesh's flag set, #543's five
     fields, `SlotNodes` emptied and rebuilt, and on authority `RepMeshScale` = the mesh's `Scale`;
  4. sets `CollisionComponent` to the mesh when the body class uses the mesh for collision;
  5. raises the `ReattachWeapons` script event by name, whether or not step 3 ran.
- **The flag** #529 left unnamed is `SkeletalMeshComponent.bSkipAllUpdateWhenPhysicsAsleep`, set to true. The name is a
  layout fit, labelled UNVERIFIED. No script reads it, and Maya's template already holds it true.
- **Read (script):** `WillowPawn.ReattachWeapons` is the only definition, and only the native raises it. With
  `Weapon` and `OffHandWeapon` both None it writes nothing. With a held weapon it runs
  `AttachThirdPersonMeshToComponent`, whose many natives the port lacks.
- **For the implementer:** this replaces #557's `MESH_REST` refusal with plain writes, decided in the write-free
  preflight. The event is answered as writing nothing when both weapons are None. A held weapon, or an override of
  the event, is `NotYet { NotRead }`.
- **Maya:** her links still stop at #529's animation-node `LayerAbsent` (her tree has found nodes and slot nodes), so
  this note clears the stop after that one.
- **Open:** whether Maya holds a weapon when the body composition runs, the flag's native readers, and the modes 1 and
  2 foot-IK step.
- **UNVERIFIED.** Every rule is unconfirmed in the running game; the note lists the live checks.
