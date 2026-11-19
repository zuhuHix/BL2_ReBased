## 2026-11-19: Rust SetAnimTreeTemplate in Off

AI-assisted (Claude Code / Claude Opus 5.5). Lane IMPL-SETANIMTREETEMPLATE, implementer, for slice links 6 and 7 (the
`make` step). Every two-digit number of 2026-11-19 is taken, so `NNN` is a placeholder until the maintainer rules on
three-digit numbers; the decisions check refuses it until then.

- **The branch.** `fix/rust-set-anim-tree-template`, first stacked on #487, which links 6 and 7 need to reach this
  native. After #487 landed it was rebased onto main d72b96d4.
- **The source.** NATIVE_SET_ANIM_TREE_TEMPLATE.md at `research/rust-set-anim-tree-template` c6ca2ef1 (analyst lane
  ANALYST-SETANIMTREETEMPLATE). The rules were coded from that note alone.
- **Clean room.** This lane did not read the native and opened no `local/analysis/`, Ghidra or REA output.
- **No dependency, no parsing change.** No real-game session took place, so every rule is **UNVERIFIED** in the running
  game. Reference is unchanged: nothing is bound there.

**New module `ow_inventory::anim_tree`**, registered by the inventory pack in Off. It binds
`SkeletalMeshComponent.SetAnimTreeTemplate(object)` as the note's section 4 gives it, with the hub's rulings from the
review of #492 (the note at df863aff):
- **Items 1 to 3**, in the note's order, are `NotYet` (not read):
  - a template other than None;
  - a non-empty `AnimTickArray`;
  - a set `Animations`.
  - The two fields read are checked against their declarations first; a mismatch is a data error naming the field.
- **Item 4**, the engine-wide registry walk, has no condition. It is `NotYet` (layer absent), with the note's exact
  what, layer and record. So in Off the native always ends there, before any write of its own.
- **The `NotYet` is returned unmarked.** The `make` step's write tracking adds "stopped after partial writes: ",
  because its `new` already wrote.
- **Not coded:** the teardown's writes and the owner's `AnimTreeUpdated` event (the note's steps 2 and 3). They stay
  unreachable until the hub rules on the note's section 5.
- **The slice's component** holds class defaults (`Owner` None, empty lists), because the port's `new` ignores the
  template. That is expected (the note's open question 5).
- **Not bound:** the interface entry `IAnimBehavior.SetAnimTreeTemplate`, which is not on the slice path.

**Tests** (synthetic, `ow_package::testkit`), 4 in `anim_tree_tests.rs`, each graded by `NotYet` kind:
- the slice's call ends at the registry, unmarked, and writes nothing;
- items 1 to 3 in order, none of them writing;
- a data error;
- Reference binding nothing.

**Verified.** The host suites. The installed Off `--slice-gate` strict, under the machine lock (the first revision
before the rebase onto #487 r2; the review-r1 revision on c2b5a258 gave the same rows):
- before (fda0e0e9): 0 ok, 5 not yet, 9 error of 14;
- after: 0 ok, 7 not yet, 7 error of 14.
- Only links 6 and 7 change. They stay in `make` (0 of 3 and 0 of 4 driver steps), now at the registry `NotYet`
  (layer absent), prefixed by the `make` step's earlier writes.
- The first revision also had a local, uncommitted probe that answered the registry "nothing". With it, links 6 and 7
  got past the native and stopped at the next statement of `AttachMeshToPickup`, the unbound
  `SkeletalMeshComponent.SetForceRefPose(bool)`.

**The hub's ruling on the note's section 5 (the registry walk).** Off answers "nothing" for a world without that
registry: a port Off answer, UNVERIFIED, in the spirit of the empty OnlineSubsystem ruling. The ruling's commit:
- **An explicit, empty registry layer.** `ComponentRegistry`, a VM service, is added empty by the pickup world's
  set-up (`equip_run::pickup_world`, links 6 and 7).
  - A world without it still answers `NotYet` (layer absent), as before.
  - An entry naming the component is `NotYet` (not read), because the handlers were not read.
  - A lane that binds a native which adds entries to the game's registry must add them to this layer; the walk then
    reads them.
- **Steps 2 and 3 are coded from the note:**
  - the writes, in order: `bAnimTreeInitialised` false, `Animations` None, the five lists emptied, `MorphTargetIndexMap`
    left as it is (the port's layout gives a native map no script value, so its write is a no-op), then
    `AnimTreeTemplate` None;
  - then `AnimTreeUpdated(component)`, sent only when `Owner` is set. On the slice path `Owner` is None, because the
    port's `new` ignores the template, so no event fires.
- **After the first write**, any `NotYet` is terminal: marked once, kind and record kept, no rollback. The preflight's
  `NotYet`s stay unmarked, and the declarations of every field written are checked before any write.
- **Tests:** 8 in `anim_tree_tests.rs`, graded by kind:
  - the registry absent, holding this component, and holding another one;
  - the preflight order;
  - the writes with no owner;
  - the event on an owner;
  - the after-write marking, for an event refusal and an owner without the event;
  - a data error;
  - Reference binding nothing.
- **Verified** on main d72b96d4 with this branch: `cargo test --workspace` 2,413 passed / 0 failed / 46 ignored.
  Installed Off `--slice-gate` strict: 0 ok / 5 NotYet / 9 errors of 14.
  - Links 6 and 7 get past this native and stop in `make` (0 of 3 and 0 of 4 driver steps) at the next statement of
    `AttachMeshToPickup`, the unbound `SkeletalMeshComponent.SetForceRefPose(bool)` (graded error), as the probe
    predicted.

**Open.**
- `SetForceRefPose`, the next stop.
- The note's open questions 1 to 5.
