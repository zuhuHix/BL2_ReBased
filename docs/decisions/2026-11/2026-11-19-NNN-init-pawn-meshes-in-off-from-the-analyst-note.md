## 2026-11-19: WillowPawn.InitializePawnMeshes in Off from the analyst note

AI-assisted (Claude Code). Lane IMPL-INIT-PAWN-MESHES, implementer (clean room). This lane coded
`WillowPawn.InitializePawnMeshes()` and `WillowPlayerPawn.InitializePawnMeshes()` from
`docs/verification/NATIVE_INIT_PAWN_MESHES.md` alone: analyst lane ANALYST-INIT-PAWN-MESHES, PR #517, at its
review-r1 head 3217b58b. This lane did not read the executable. There was no real-game session, so every rule stays
**UNVERIFIED** in the running game. No dependency or crate edge was added, no bounds check was loosened, and Reference
is unchanged. The decision number is a placeholder until the hub assigns one.

- **`ow-world` `world/pawn_meshes.rs` (new), Off only.** It binds one rule under both keys, first-wins: a key another
  pack bound first keeps its binding, and no other binding of these keys exists today.
- **The note's section 6 preflight**, in five steps. It writes nothing and loads nothing: class lookups use
  `vm.space.classes.by_path` alone, never `find_class` (hub ruling, review r1 of #522).
  1. **The receiver:** none is `VmError::Script` "called with no pawn". A class that is not `WillowGame.WillowPawn` or
     a subclass is "<class path> is not a WillowPawn". The function is `WillowPawn.InitializePawnMeshes`.
  2. **The helper class:** `Engine.InstanceDataHelper` not in the class table is `NotYet { NotRead }`,
     "WillowPawn.InitializePawnMeshes: the Engine.InstanceDataHelper class is not loaded". This is checked before the
     body class, whatever the body class holds. A class a package holds but the table has not loaded counts as absent.
  3. **The body class:**
     - a class with no `BodyClass` is "<class path> declares no BodyClass";
     - a non-object value is "BodyClass holds <kind>, not an object";
     - None does nothing;
     - an unloaded or failed definition handle is `NotYet { NotRead }`, "WillowPawn.InitializePawnMeshes: the
       attachment list of the unloaded body class <object path>". The path is the store's name for the export
       (`ow_defs::identity::full_path`). Nothing is loaded (hub ruling, review r1 of #517: a load is a store write).
  4. **The attachments:** a malformed composition is "<class path> has no BodyComposition.Attachments array". An empty
     list does nothing; that also covers the root-motion writes inside the gate.
  5. **A non-empty list:** `NotYet { LayerAbsent }` with the note's text, layer and record, as soon as the non-empty
     array is established. No other field is checked.
- **Nothing is written in Off** (the note's hub rulings 1 and 2): no switch entry, no network flag and no root-motion
  write. So there is no "stopped after partial writes" prefix of the native's own; a caller that wrote before adds it.
- **Tests:** ten in `world/pawn_meshes_tests.rs`, on invented classes and toy packages. They cover:
  - each answer above, under both keys;
  - the `LayerAbsent` with the heap unchanged, and with the unrelated fields left out of the classes;
  - a missing helper on a pawn with no `BodyClass`;
  - a helper only in a package, which the lookup does not load;
  - a genuinely unloaded store handle, in a plain file and in a package cooked into another file;
  - first-wins.
- **The gate** (installed, Off, strict, with `Engine.IsSplitScreen()` bound false locally only): 0 ok / 11 NotYet / 3
  errors.
  - Link 1 stops in `maya:8` at step 3's `NotRead`, because Maya's `BodyClass_Siren` is an unloaded handle at that
    point.
  - Reaching the `LayerAbsent` needs the body class loaded before the native runs. The hub ruled that the spawn loads
    it (lane SPAWN-COMPLETE).

**History.**
- The first version (5d284e27) answered a missing helper with "nothing" and checked fields the note does not name.
- Rebasing onto 3217b58b and review r1 of #522 replaced both with the steps above.
- **Review r2 of #522:**
  - `handle_path` uses `ow_defs::identity::full_path`, so a body class in a package cooked into `Startup.upk` is
    `GD_ToyBody.Target`, not `Startup.GD_ToyBody.Target`;
  - three tests were added: the nested package, the missing helper with no `BodyClass`, and the bare-class
    `LayerAbsent` together with a helper held only by a package.
