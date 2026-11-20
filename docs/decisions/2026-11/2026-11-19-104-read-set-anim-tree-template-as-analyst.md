## 2026-11-19: SkeletalMeshComponent.SetAnimTreeTemplate read as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SETANIMTREETEMPLATE, analyst only (clean room: this thread does not
implement the note). The decision number is pending from the hub; NNN is a placeholder.

- **What:** `docs/verification/NATIVE_SET_ANIM_TREE_TEMPLATE.md`, the stock native links 6 and 7 reach in `make` once
  the pickup's inventory accessors are in (#487), and two NATIVE_INDEX rows.
- **How:**
  - the installed script, read with `research/script_disasm.py`;
  - the executable, read with the project's Ghidra tooling (read-only queries under the machine lock);
  - field names from the cooked packages.

  Raw output stays under ignored `local/analysis/setanim/`. No bl-sdk evidence is on record for this native, and no
  real-game session was run.
- **Found:**
  - `WillowWeapon.AttachMeshToPickup` calls it with None, on a component made one statement earlier.
  - The native first tears the tree down: the tick lists, the morph state and `Animations` are cleared, and
    `bAnimTreeInitialised` is set false. The teardown also walks an engine-wide registry and makes three calls that
    were not read.
  - A template is copied and initialised; None sets `AnimTreeTemplate` to None.
  - The component's `Owner` then gets the script event `AnimTreeUpdated`, whose installed body is empty.
- **Off answers:**
  - named `NotYet`s, checked before any write:
    - a template argument;
    - a non-empty tick list;
    - a set `Animations`;
    - the registry walk, as `LayerAbsent` ("nothing" there is left to the hub);
  - then the writes in order, and the owner's event run as script.
- Every rule is **UNVERIFIED** in the running game. The note lists five open questions and three bl-sdk checks.

**Review r1 of #492, folded in.**
- In default Off the native always ends at the preflight's fourth item (the unconditional registry walk, `LayerAbsent`),
  with its `what`, layer and record given; steps 2 and 3 wait for the hub's section 5 ruling.
- The preflight `NotYet` is returned unmarked; the caller's write tracking adds the prefix.
- The port's `new` ignores the template, so the slice's component holds class defaults (open question 5 now asks whether
  the game's copy carries `Owner`).
- The five lists and the map are named; emptying the map is a no-op or a zero write where the layout gives it no script
  value.
