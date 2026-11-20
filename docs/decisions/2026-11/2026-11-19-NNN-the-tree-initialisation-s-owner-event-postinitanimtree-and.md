## 2026-11-19: The tree initialisation's owner event PostInitAnimTree and the two lookups in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-TREE-INIT-OWNER-EVENT, implementer. The decision number is pending
from the hub (three digits); NNN is a placeholder.

- **Source:** `docs/verification/NATIVE_TREE_INIT_OWNER_EVENT.md` (#612, accepted at its final head 3691317c, analyst
  9554, not this thread), and NATIVE_ANIM_NODE_LOOKUP.md section 1 (#543) for the lookups' rules, scoped as #612 binds
  them. Clean room: this lane implemented from the notes and opened no native code or analysis output.
- **Base:** #608 (fix/rust-copy-component-pass at 193ef94f, replayed onto main 4c4522d7). Main holds #570 (squash
  030d04c), the prerequisite #612 lists for `Vm::find_loaded_method`, so this branch adds nothing there.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged:** no dependency was added and no bounds check was loosened. Off only: Reference binds neither native, and
  the copy's Reference path is untouched.

**What changed:**
- **The owner event (`crates/ow-world/src/world/anim_tree_copy.rs`).** It replaces #561's refusal "the owner's event
  after the tree initialisation" and its test-only switch. The copy's preflight (`plan`) now plans the event, write-free,
  load-free and draw-free, in the note's order:
  1. the owner: the component's `Owner` in the instance path, the actor passed by the caller in `attach_check`
     (`plan_with`, `EventOwner`). `Owner` must be declared as an object reference. No owner means no event;
  2. the owner must be resident (#543's refusal);
  3. an owner in a state: `NotRead`;
  4. `find_loaded_method(class, "PostInitAnimTree", None)`. A chain class whose code is not loaded is `NotRead`. A
     function other than the three read versions (`Engine.Actor`, `Engine.Pawn`, `WillowGame.WillowPlayerPawn`,
     compared by path) is the override refusal, `NotRead`;
  5. `Vm::check_call(f, [component])`, the call's own binding and entry checks;
  6. for `Pawn`'s and `WillowPlayerPawn`'s versions: `Mesh` declared. In the instance path, a `Mesh` that is the
     component is `NotRead` (`Pawn.CacheAnimNodes` is native and not read);
  7. for `WillowPlayerPawn`'s version:
     - `ArmBoneBlend`, `AimOffset`, `LeftHandIK` and `BodyClass` declared as object references;
     - a set `BodyClass` resident and declaring `AimOffsetNodeName` as a name;
     - each native bound, else `NotPorted` with its own text;
     - the three cast classes defined at set-up, else `NotRead`;
     - each find the script makes (a cache only while it is None) planned on the template's tick order and control
       order, with its cast's answer computed (`Vm::is_child_checked`);
  8. nothing draws a random number.

  The write is `vm.call(f, Some(owner), vec![component])`, after step 5's writes and before `bAnimTreeInitialised`, as
  `notify_owner` calls `SetAnimTreeTemplate`. It is not the world-event path. A `NotYet` the run raises is marked
  "stopped after partial writes: " by #518's counters (the existing `after_writes`); any other error passes unchanged.
  `body_composition.rs` passes the pawn to `attach_check`.
- **The two lookups (`crates/ow-world/src/world/tree_lookup.rs`, new).** `SkeletalMeshComponent.FindAnimNode(name)`
  and `FindSkelControl(name)` are bound in Off, read only, keep-existing. They are scoped to the path #543 read:
  - the component's own tree means `Animations` derives from `Engine.AnimTree` and has the component as its heap outer;
  - the node lookup takes the tick-list path only (the first entry of `AnimTickArray` is the tree). Otherwise it is
    `NotRead`, the gather path;
  - names compare as whole names (name and number);
  - controls are searched in `SkelControlLists` order, then along each `NextControl` chain.

  The binding is global in Off, so the strict slice gate below lists every link.
- **ow-vm.** `Vm::check_call` and `interp::check_entry` now take `&Vm`.
  They only read; the body decode they share with the call caches through a `RefCell`, as before. Callers holding
  `&mut Vm` are unchanged. New: `Vm::is_child_checked`, the class half of `DynamicCast` with its stand-in check. The
  three cast classes are not interfaces, so the cast's `implements` arm cannot say yes for them.

**Tests** (`anim_tree_copy_tests.rs`, synthetic toy Core / Engine / WillowGame packages with the three versions at
their installed paths, plus invented classes). The note's 19 are covered, with these limits:
- tests 6 and 7, and test 8's storing None, are the event's own script, which the toy events do not reproduce. The port
  runs whatever script the installed version holds. These tests cover only the preflight side: `BodyClass` None reads
  no body class, and a find of another class passes the preflight with its cast answer computed;
- test 5 checks the call and its argument through a toy field, and checks the three finds' answers by calling the
  natives on the instance;
- test 18 uses an invented native bound to raise a `NotYet`. An unbound native inside a run is `UnboundNative`, which
  passes unchanged.

Added beyond the 19: an undefined cast class, and the lookups' rules (no tree, a tree that is not the component's own,
an empty tick list, a whole-name match, a None control name, a looping chain).

**Verification:** `cargo test --workspace --locked` 3,038 passed / 0 failed / 51 ignored on 193ef94f plus this branch. The installed Off strict `--slice-gate`, run on a throwaway merge with #570, #592, #500 and #528 and against the same merge without this branch, moves links 1, 3, 4, 5, 13 and 14 from the owner's event to #557's body composition lookup refusal; the other eight links are unchanged (GATES row of 2026-11-20). No real-game run.

**Review r1 of #631 (Sol xhigh), the fixes:**
1. The initialisation's own gate comes first: a component whose `ParentAnimComponent` is set gets the copy and no
   initialisation, so no owner event, on the instance and in the attach check (`ParentAnimComponent` must be declared
   as an object reference).
2. Neither lookup answers from a tree that is not the receiving component's own. `FindSkelControl` on such a tree is
   `NotYet { NotRead }` (a new text), after its None answers (a None name, no tree, a root that is not an `AnimTree`).
   `FindAnimNode` already refused one; a tick-list entry from another tree is now its gather refusal too.
3. `Vm::check_call` runs the call's service-borrow check, so a held borrow is refused in the preflight with nothing
   written.
4. Both natives are bound under their package-qualified identity, `Engine` plus the key (new
   `Vm::register_native_in` and `Vm::is_bound_in`). Keep-existing and rule 7 ask about that identity, so a same-key
   binding in another package no longer stands for Engine's.
5. A lookup literal not interned yet (`ArmBoneBlend`, `LeftHandGrip`) matches nothing, but the preflight still reads
   every node or control the run's search reads. Before, an uninterned `ArmBoneBlend` also counted as the None name.
6. The toy Engine package declares `SkeletalMeshComponent` with `bAnimTreeInitialised` and the two natives. The toy
   `WillowPlayerPawn` event records the flag it sees, then fills `ArmBoneBlend` and `LeftHandIK` through the bound
   natives in bytecode; the test asserts both caches and the flag. Its limits: the casts and the aim-offset find
   through `BodyClass` are not in the toy bytecode (bytecode cannot name the hand-declared cast classes), and the
   caches' "only while None" guard is not either. The refusal snapshot now also holds the name table's length; the
   attach snapshot holds it and the RNG draws.

**Restacked onto #618 (3630cd59, the hub's call).** #618 changed `attach_check` to `attach_plan`, which returns the
plan so the body composition's lookups search the planned copy. The two are merged: `attach_plan(vm, template,
owner)` plans the copy with the owner's event for the given owner, and `attach_check` is that plan's check. The body
composition calls `attach_plan(.., Some(owner))` and hands the plan to #618's lookups. #618's test switch for the
owner-event refusal is gone with the refusal, and its fixture declares `ParentAnimComponent`.

`cargo test --workspace --locked` 3,047 passed / 0 failed / 51 ignored. The strict slice gate on #618 plus #592 moves
links 1, 3, 4, 5, 13 and 14 from the owner's event to the body's rendering hooks; the other eight are unchanged.
Every rule is still UNVERIFIED.
