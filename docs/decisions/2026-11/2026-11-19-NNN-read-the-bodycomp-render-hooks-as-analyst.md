## 2026-11-19: The body composition's rendering hooks are two ported natives and a gear-likeness interface test

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-BODYCOMP-RENDER-HOOKS, analyst only (clean room: a different lane
implements it). The decision number is pending from the hub (three digits); NNN is a placeholder.

- **What was decided:** `docs/verification/NATIVE_BODYCOMP_RENDER_HOOKS.md` names the three hooks #542's ruling 4 left
  `LayerAbsent`, in order, in the pawn's `PostInitBodyComposition`:
  1. a gear-likeness step on the made object through `WillowGame.IGearLikenessConsumer` (by fit), which does nothing
     for a class that does not implement it;
  2. in mode 0, on a primitive, behind a pawn test and a local-player-controller test (by fit):
     `PrimitiveComponent.SetOwnerNoSee(true)`;
  3. with the pawn's `LightEnvironment` and a mesh component that accepts lights (fitted):
     `PrimitiveComponent.SetLightEnvironment`.

  Hooks 2 and 3 follow the natives' own rules, already ported in `attach_mesh.rs`. **Proposed:**
  - real answers for hook 1's "no" and for hooks 2 and 3;
  - `LayerAbsent` kept for a gear-likeness implementer (render-side, not read);
  - `NotRead` for the unidentified gate parts, or a hub ruling taking them as allowing.
- **Sources:**
  - the records;
  - the packages (WillowGame's name table, class layouts);
  - `Borderlands2.exe` through `tools/ghidra/run.ps1 -ReadOnly` under the machine lock. The body was found by the name
    `CustomHead` it interns, and the two virtuals by the slots their natives' execs call. Raw output stays under
    ignored `local/analysis/bcrh/`.

  No addresses, decompiler output or listing structure is in tracked files. No bl-sdk session was granted.
- **Verified:** nothing in the running game. Every rule is **UNVERIFIED**.
- **Probe:** with local stand-ins for #612's event and these hooks, the six links (1, 3, 4, 5, 13, 14) stop next at
  attachment 1's socket attach ("the socket and bone tables of Char_Siren.Mesh.Skel_SirenBody", `LayerAbsent`).
- **Index:** a NATIVE_INDEX row was added.
