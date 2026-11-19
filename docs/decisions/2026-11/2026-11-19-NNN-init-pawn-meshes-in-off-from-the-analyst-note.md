## 2026-11-19: WillowPawn.InitializePawnMeshes in Off from the analyst note

AI-assisted (Claude Code). Lane IMPL-INIT-PAWN-MESHES, implementer (clean room). This lane coded
`WillowPawn.InitializePawnMeshes()` and `WillowPlayerPawn.InitializePawnMeshes()` from
`docs/verification/NATIVE_INIT_PAWN_MESHES.md` alone, the analyst lane ANALYST-INIT-PAWN-MESHES, PR #517 at
5d284e27. This lane did not read the executable. There was no real-game session, so every rule stays **UNVERIFIED** in
the running game. No dependency or crate edge was added, no bounds check was loosened, and Reference is unchanged. The
decision number is a placeholder until the hub assigns one.

- **`ow-world` `world/pawn_meshes.rs` (new), Off only.** It binds one rule under both keys, first-wins: a key another
  pack bound first keeps its binding, and no other binding of these keys exists today. In order, writing nothing:
  1. the receiver must be a `WillowGame.WillowPawn`, or a `VmError::Script`;
  2. `BodyClass` must be declared, as an object reference;
  3. no `Engine.InstanceDataHelper` class: nothing;
  4. `BodyClass` None: nothing;
  5. **an unloaded (lazy) `BodyClass`:** `NotYet { NotRead }`, naming the unloaded body class's attachment list
     (the hub's ruling on #517 r1: a load is a store write, so nothing is loaded);
  6. `BodyComposition.Attachments` must be declared, and an empty list gives nothing;
  7. a non-empty list: the receiver's `InstanceDataState`, `ReplicatedInstanceDataState` and `Mesh`, and the body
     class's `UseRootMotion` and `UseRootRotation`, are checked, then `NotYet { LayerAbsent }` with the note's text,
     layer and record.
- **Nothing is written before the `LayerAbsent`** (the hub's ruling on 9554's open question 4): no switch entry, no
  network flag, and no root-motion write, which comes after the composition build in the note's order. So there is
  no "stopped after partial writes" prefix of the native's own.
- **The declaration errors** are `VmError::Script` with the texts "WillowPawn.InitializePawnMeshes: <class path> has
  no <field>" and "WillowPawn.InitializePawnMeshes: <class path>.<field> is not <kind>". The class path is the
  receiver's (or the body class's) class.
- **Tests:** five in `world/pawn_meshes_tests.rs`, on invented classes and a toy Core. They cover:
  - the three empty cases, under both keys;
  - the `LayerAbsent`, with the heap length and both objects' slots unchanged;
  - a mistyped `Mesh`'s exact error;
  - a receiver that is not a pawn;
  - first-wins.
- **Pending:** 9554's revision of #517 brings the helper predicate and the receiver error. This lane rebases onto it
  when it lands.
- **The gate** (installed, Off, strict, with `Engine.IsSplitScreen()` bound false locally only): 0 ok / 11 NotYet / 3
  errors. Link 1 stops in `maya:8` at step 5's `NotRead`, because Maya's `BodyClass_Siren` is an unloaded handle at
  that point. So under the lazy ruling the `LayerAbsent` of step 7 is not reached on the slice. Reaching it needs the
  body class loaded earlier, by the caller or the world's set-up, which is a question for the hub.

**Rebased onto #517's review-r1 head (3217b58b).** The rule now follows that note's section 6:
- the receiver texts are "called with no pawn" and "<class path> is not a WillowPawn";
- the helper class is looked up in the class table alone, so nothing can load. When it is absent, the answer is
  `NotYet { NotRead }`, "WillowPawn.InitializePawnMeshes: the Engine.InstanceDataHelper class is not loaded", no longer
  "nothing";
- the body-class texts are "<class path> declares no BodyClass" and "BodyClass holds <kind>, not an object";
- the lazy text is "WillowPawn.InitializePawnMeshes: the attachment list of the unloaded body class <object path>";
- any malformed composition gives "<class path> has no BodyComposition.Attachments array";
- the extra field preflight before the `LayerAbsent` is dropped, as the note no longer names it.

So two cases now answer "nothing": `BodyClass` None and an empty list. Six tests.

**Review r1 of #522 (five findings, fixed).**
1. **Non-loading lookups (hub ruling):** `WillowPawn` and `Engine.InstanceDataHelper` are looked up with
   `vm.space.classes.by_path`, never `find_class`, so no class code is loaded and no function registered before a
   refusal.
2. **The helper first:** checked right after the receiver, before `BodyClass`. A missing helper is the named
   `NotRead` whatever the body class holds.
3. **No masking:** the `LayerAbsent` comes as soon as the non-empty array is established. The field checks the note
   does not name are gone, and the mistyped-`Mesh` test is replaced by a malformed-`Attachments` one.
4. **The note's templates:** the receiver, `BodyClass` and attachment texts are the note's. The lazy refusal names the
   handle's full object path, read from its package's export table without loading (`handle_path`).
5. **The slice's case:** a test builds a genuinely unloaded store handle and checks the `NotRead`, its record, the full
   path, no prefix, and that the handle is still unloaded. Seven tests.
