## 2026-11-19: Rust SetAnimTreeTemplate in Off

AI-assisted (Claude Code / Claude Opus 5.5). Lane IMPL-SETANIMTREETEMPLATE, implementer, for slice links 6 and 7 (the
`make` step). Every two-digit number of 2026-11-19 is taken, so `NNN` is a placeholder until the maintainer rules on
three-digit numbers; the decisions check refuses it until then.

- **The branch.** `fix/rust-set-anim-tree-template`, stacked on #487 (`fix/rust-links6-8-next-stop` c2b5a258, its r2), which
  links 6 and 7 need to reach this native.
- **The source.** NATIVE_SET_ANIM_TREE_TEMPLATE.md at `research/rust-set-anim-tree-template` c6ca2ef1 (analyst lane
  ANALYST-SETANIMTREETEMPLATE). The rules were coded from that note alone.
- **Clean room.** This lane did not read the native and opened no `local/analysis/`, Ghidra or REA output.
- **No dependency, no parsing change.** No real-game session took place, so every rule is **UNVERIFIED** in the running
  game. Reference is unchanged: nothing is bound there.

**New module `ow_inventory::anim_tree`**, registered by the inventory pack in Off. It binds
`SkeletalMeshComponent.SetAnimTreeTemplate(object)` as the note's section 4 gives it:
- **Preflight, before any write:**
  - the declarations it reads and writes (a data error names the field);
  - then, in the note's order:
    1. a template other than None;
    2. a non-empty `AnimTickArray`;
    3. a set `Animations`;
    4. the engine-wide registry walk.
  - The first three are named `NotYet` (not read). The registry walk is `NotYet` (layer absent), the note's default:
    the port holds no such registry.
- **The writes, in order** (reached only when the registry is answered "nothing"):
  - `bAnimTreeInitialised` false;
  - `Animations` None;
  - the three tick lists emptied;
  - `MorphTargetIndexMap` emptied (the VM holds a native map as None);
  - `ActiveMorphs` and `ActiveCurveMorphs` emptied;
  - `AnimTreeTemplate` None.
- **The owner's event.** With `Owner` set, `AnimTreeUpdated(component)` is dispatched as script on the owner's class
  and state. A `NotYet` from it is terminal and marked "stopped after partial writes", its kind and record kept, with
  no rollback.
  - An owner class without the event is a named `NotYet` (not read) after the writes. The note is silent there.
- **The note's section 5 option** ("nothing" for a world without the registry) is a `Registry::Nothing` answer that
  only the tests and a future hub ruling use. The bound native does not use it.
- The interface entry `IAnimBehavior.SetAnimTreeTemplate` is not bound. It is not on the slice path.

**Tests** (synthetic, `ow_package::testkit`): 7 in `anim_tree_tests.rs`. They cover:
- the bound default stopping before any write;
- the preflight's order;
- the writes and the event;
- no owner;
- a refusal after the writes, and an owner without the event;
- a data error;
- Reference binding nothing.

**Verified.** The host suites on c2b5a258 (2,329 passed / 0 failed / 46 ignored). The installed Off `--slice-gate`
strict ran under the machine lock before the rebase onto #487 r2, whose change is a path constant and a doc line:
- before (fda0e0e9): 0 ok, 5 not yet, 9 error of 14;
- after: 0 ok, 7 not yet, 7 error of 14.
- Only links 6 and 7 change. They stay in `make` (0 of 3 and 0 of 4 driver steps), now at the registry `NotYet`
  (layer absent), prefixed by the `make` step's earlier writes.
- A local, uncommitted probe that answered the registry "nothing": links 6 and 7 got past the native and stopped at
  the next statement of `AttachMeshToPickup`, the unbound `SkeletalMeshComponent.SetForceRefPose(bool)`.

**Open.**
- The hub's ruling on the registry walk (the note's section 5).
- `SetForceRefPose`, the next stop after it.
- The note's open questions 1 to 5.
