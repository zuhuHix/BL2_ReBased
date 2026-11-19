## 2026-11-19: A component's QueryInterface answer, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-LINK11-QUERY-INTERFACE, slice link 11. Coded from
docs/verification/NATIVE_PAWN_AI_INTERFACE.md section 6 alone, at its review-r1 head 17b9c330 (PR #512; clean room:
lane ANALYST-LINK11-QUERY-INTERFACE wrote it, and the hub's rulings on its review are folded into section 6.4). The
executable and the analysis output were not opened. Synthetic fixtures only.

- **What changes (`ow-combat` `pawn_interface::component_answer`, registered in Off on exactly
  `Engine.ActorComponent` and `Engine.SkeletalMeshComponent`; first-wins, so a class that already has a rule keeps
  it).** In the note's order:
  1. **The exact-class test.** The VM's interface rules are inherited, so the rule tests the receiver's class first.
     Any other class that reaches it, `SkeletalMeshComponent` subclasses and the `PrimitiveComponent` line included, is
     `NotYet { NotRead }`: "what a <class> answers to QueryInterface is not read (asked of <class> for <interface>)",
     under this note. No VM-wide dispatch change.
  2. **The plain answer** (`Vm::plain_interface`): an object is the answer. A `NotYet` or an error passes through
     unchanged, and only a successful None goes on.
  3. **The `Owner` field:**
     - undeclared: `VmError::Script`, "ActorComponent.QueryInterface: <class> declares no Owner";
     - declared as anything but an object reference to `Engine.Actor` or a subclass: `VmError::Script`, "<class>.Owner
       is not an object reference to Engine.Actor";
     - None: None.
  4. **The owner's answer** through `Vm::query_interface`, its cycle guard and depth limit unchanged, passed through
     as it is.
  - **Nothing written, nothing loaded.** A load is a store write, so a receiver or an `Owner` that is an unloaded
    definition handle is `NotYet { NotRead }` with the note's texts.
- **Retired:** the open answer for `Engine.SkeletalMeshComponent` ("what a skeletal mesh component answers to
  QueryInterface"), now the rule. Reference is unchanged and registers none of it.
- **Tests** (`ow-combat` `pawn_interface/tests.rs`, on an invented `Engine.upk`; `Pawn` and `Controller` now derive
  from an invented `Engine.Actor`):
  - pawn first: a pawn with no controller whose mesh's `Owner` is the pawn answers None through the cycle guard,
    writing nothing;
  - a direct mesh query forwards to its owner pawn, which answers through a controller listing the interface;
    `Owner` None answers None;
  - a `SkeletalMeshComponent` subclass and another component class get the open `NotRead`;
  - a plain-answer `NotYet` (an engine-native stand-in class) passes through with its own kind and record;
  - `Owner` missing and mistyped give the two Script texts;
  - an unloaded `Owner` is `NotRead` and stays unloaded;
  - Reference gives the plain answer;
  - the open-answers test now expects the new open text on a `SkeletalMeshComponent` subclass.
- **Checks.** `cargo test -p ow-combat` 15 pawn-interface tests pass; `cargo test --workspace` 2,445 passed, 0 failed,
  46 ignored; crate edges OK.
- **Installed `--slice-gate`, Off, strict, under the machine lock:**
  - **On this branch** (#512's base, main 7668d07): 0 ok / 5 not yet / 9 error. Links 10 to 12 stop in `accept` at
    `MissionTracker.GetAllMissions` (that base has neither #496 nor #497), so link 11 does not reach the rule there. The
    other rows are the base's.
  - **A local probe, never committed:** this commit merged with #496 (f972d7d3) and #497 (7af3ab76), test files left
    unmerged, release binary only. Result: 1 ok / 4 not yet / 9 error.
    - Link 10 ok, 8 of 8 steps.
    - Link 11 passes 4 of its 8 steps (`range`, `tick:1`, `spawn`, `tick:1`) and stops at `hit:fire`, marked: "a
      pawn's QueryInterface(GearboxFramework.IAIInterface) goes to its Mesh, a Engine.SkeletalMeshComponent:
      ActorComponent.QueryInterface: the component GD_TargetDummy.Character.Pawn_TargetDummy:SkeletalMeshComponent_333
      is an unloaded definition handle" (`NotRead`, this note).
    - Link 12 is blocked behind link 11.
  - So the rule is reached and refuses by its own no-load step. The port's Fire dummy holds the archetype's own,
    still unloaded mesh component, not a loaded copy. Whether that mesh should be loaded, or instanced per actor by
    the dummy's spawn, is a question for the hub. This rule may not load it.
- **UNVERIFIED.** Nothing here was checked in the running game. Still open, from the note:
  - the rule of other component classes (6.6, 1);
  - why the port's Fire dummy gives none at step 2 (2);
  - the dummy mesh's `Owner` (3).
