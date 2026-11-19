## 2026-11-19: Read what a component answers to QueryInterface as analyst

AI-assisted (Claude Code). Lane ANALYST-LINK11-QUERY-INTERFACE, analyst only (clean room). A new section 6 of
`docs/verification/NATIVE_PAWN_AI_INTERFACE.md`, its pointers there, and a NATIVE_INDEX row. The decision number is a
placeholder until the numbering question is settled.

- **Sources:**
  - `Borderlands2.exe`, through `tools/ghidra/run.ps1` read-only queries and the local table reader, one headless run
    at a time under the machine lock;
  - field names from `tools/ghidra/class_layout.py`;
  - two local, uncommitted slice-gate probes on a throwaway merge of #496 (9b591ad) and #497 (7af3ab7) on main 7668d07.

  Raw output stays under ignored `local/analysis/link11qi/`. There is no bl-sdk session, so every rule is
  **UNVERIFIED**.
- **Read:** `ActorComponent` replaces `Object`'s `QueryInterface` answer: the plain answer first, then the
  component's `Owner`, under a cycle guard, with no write. `SkeletalMeshComponent` shares that body; the classes
  between them were not compared.
- **For slice link 11:** the port's Fire dummy has no `Controller`, so its question reaches its `Mesh`. The mesh
  answers none (its owner is the dummy, which is still answering), so `Behavior_AIHold` makes no hold.
  - The game's dummy has a mind, and the golden trace shows it demigod. So this is the game's rule on the port's
    input, not the game's outcome, and the missing controller is recorded as open.
- **Off answer proposed:** register the rule for exactly `ActorComponent` and `SkeletalMeshComponent`. Other
  component classes keep the existing open `NotRead`.
- **Probes:**
  - with that rule and R1 to R3 (NATIVE_MISSION_TEXT_LIST.md), link 11 next met the unbound
    `RecordMissionObjectiveChangedEventForPlayer`. It shares the statistics submit routine, so R3 covers it;
  - with that answered, links 10 and 11 pass (8 / 8 each), and link 12 stops at `turnin` (1 / 6) on the unbound
    `WillowPlayerController.UnlockAchievementIfConditionsMet`.
- Not implemented here: an implementer lane codes it from the note.

**Review r1 of #512 (four findings and two hub rulings, fixed in section 6).**
1. **The classes (hub ruling):** an exact-class test inside the native, for `ActorComponent` and
   `SkeletalMeshComponent` only. Every other component class, `SkeletalMeshComponent`'s subclasses included, keeps the
   open `NotRead`. The VM's dispatch is not changed.
2. **What the stop proves:** that the pawn's steps 1 and 2 gave none, not that the dummy has no `Controller`. The game
   dummy's first-hit `Controller` and route stay UNVERIFIED.
3. **The contract (hub ruling):**
   - only a successful None from the plain answer reaches `Owner`, and a `NotYet` or error passes through;
   - `Owner` None answers None;
   - a missing or mistyped `Owner` is a `VmError::Script`, with the exact texts given;
   - nothing is lazy-loaded, and an unloaded receiver or `Owner` is a named `NotRead`;
   - the VM's cycle and depth behaviour is kept.
4. **The call paths:** pawn first (the cycle gives none) and a direct mesh query (owner forwarding), each with a test
   the implementer owes.
