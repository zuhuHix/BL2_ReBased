## 2026-11-19: InstanceDataHelper.InitializeBodyComposition in Off from the analyst note

AI-assisted (Claude Code). Lane IMPL-BODY-COMPOSITION, implementer (clean room). This lane coded
`InstanceDataHelper.InitializeBodyComposition` from `docs/verification/NATIVE_INITIALIZE_BODY_COMPOSITION.md` alone:
analyst lane ANALYST-BODY-COMPOSITION, PR #529 at cdf199be. It also used the hub's three rulings on that note's
section 7 choices. This lane did not read the executable. There was no real-game session, so every rule stays
**UNVERIFIED** in the running game. No dependency or crate edge was added, no bounds check was loosened, and
Reference is unchanged. The lane is stacked on #522. The decision number is a placeholder.

- **The write tracker:** #518's counters (`Heap::committed_writes` and `note_write`, `GameRng::draws`,
  `DefStore::load_transitions`, and the VM's `note_write` calls) are copied unchanged from
  `fix/rust-483-followup` @ 211484a5 in one commit, so the rebase folds.
- **`ow-world` `world/body_composition.rs`, Off only:**
  - **The plan** (write-free, load-free). It runs in walk order and decides what the walk makes: the base and switch
    rules (no "base first"), removed attachments, a missing template.
  - **The refusals**, raised before the first write, with the note's texts:
    - call-wide, first: menu modes, and the client-side apply;
    - then per attachment that would be made: object and template types, the teardown of a removed attachment, a
      collision type, an actor's own named socket components, the socket and bone tables of a skeletal parent
      (`LayerAbsent`), and the special parent identifier's refresh.
  - **The writes**, in the note's order:
    - each made attachment's entry: a component is made from its template, owned by the owner, its properties the
      template's; its entry is appended, then it is attached to the actor (its `Components`, zero offset);
    - switch entries (`Data` with `Float` 0) and custom entries (`Name`, `Replication`, `Type`, index);
    - then, on authority, `SwitchStateBitField` and `ActiveSwitchValues` (zeroed, then packed), `bNetDirty` and
      `bForceNetUpdate`.
  - A `NotYet` after any counter moved is marked "stopped after partial writes", its kind kept.
  - The script native writes its two structs to its out arguments. The pawn wrapper writes them to the pawn's own
    fields.
- **The hub's rulings:**
  1. a component made from a template counts as valid;
  2. the special parent identifier's refresh stays `NotYet { NotRead }`;
  3. a custom head or skin entry is appended and nothing is applied.
- **This lane's narrower answers**, each a named `NotYet { NotRead }` where the note does not give the rule:
  - every collision type other than 0: the note names the flags for types 1, 2, 3, 6 and 9 "by fit only";
  - a component made under a switch: its parent is not given;
  - a component parent that is not a skeletal component;
  - an instance-data type outside the note's list.
- **`InitializePawnMeshes`** (#522) now runs the build instead of its `LayerAbsent`, after one more write-free check:
  a body class that sets `UseRootMotion` or `UseRootRotation` is `NotYet { NotRead }`, because the two mesh fields
  are named by fit only.
- **Tests:** nine new and two updated, on invented classes and structs. They cover:
  - a component made, entered and attached;
  - a switch entered, with its component waiting;
  - a custom head entry;
  - a missing template;
  - the socket-table `LayerAbsent`, write-free;
  - the special-name, collision and actor-socket refusals, write-free;
  - an object-template refusal;
  - the tracker's marking;
  - the script native's out arguments;
  - the pawn wrapper's malformed list and its root-motion `NotRead`.
- **The slice** (installed, Off, strict, with #528 merged and `Engine.IsSplitScreen()` bound false, both locally
  only): 1 ok / 10 NotYet / 3 errors.
  - Links 1, 3, 4, 5, 13 and 14 now stop in `maya:8` at the build's preflight, "InitializeBodyComposition: the special
    parent identifier's refresh" (`NotRead`, ruling 2), on Maya's attachment 0 (parent identifier `Mesh`). Nothing is
    written.
  - The note's data puts the next stop after it at attachment 1's socket on the new skeletal mesh: the socket-table
    `LayerAbsent`.
