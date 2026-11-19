## 2026-11-19: A controller's and a pawn's destroy in Off, the kill's unlinking

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-ACTOR-DESTROY-CONTROLLER, implementer (clean room). Coded from
`docs/verification/NATIVE_ACTOR_DESTROY_CONTROLLER.md` (analyst lane ANALYST-ACTOR-DESTROY-CONTROLLER, decision
2026-11-19-83, PR #472) and `NATIVE_ACTOR_DESTROY.md` alone. No analysis output was opened.

**What changed, in `crates/ow-world/src/actors.rs`, Off only.** `ParityMode::Reference` is unchanged (the C++
stand-in for a controller; the generic routine for any other actor).
- **The controller and player case:**
  - a player controller whose `Player` is a `LocalPlayer` gets `bPendingDestroy` and answers false;
  - one whose `Player` is anything else (a network connection) is `NotYet { NotPorted }`;
  - any other controller, such as the kill's `WillowMind`, takes the generic routine. Before this change, Off refused
    every controller.
- **The world's two lists:**
  - `WorldInfo.ControllerList` / `Controller.NextController` and `WorldInfo.PawnList` / `Pawn.NextPawn` are kept on a
    `WorldInfo` whose class declares them;
  - a spawn adds a controller (unless pending delete) or a pawn at the head, newest first. A pawn already listed is
    first removed;
  - the note did not identify which native begin-play step adds them. The port adds them right after `PreBeginPlay`
    (UNVERIFIED). Map-placed actors are not added, since their begin-play pass is not modelled.
- **The preflight**, before any write:
  - the existing generic refusals;
  - a `GearboxMind` with `MyRuleEngine` set: `NotYet { NotRead }`, "the rule engine's call at its mind's destroy";
  - a controller or pawn in a world that does not hold its list: `NotYet { LayerAbsent }`;
  - every field the step uses must be declared, and the list must walk without a loop.
- **The world-drop step**, right after `Destroyed()`:
  - a controller: its `ResourcePoolManager`, when still set on the authority, destroyed and cleared; the
    `ControllerList` unlink, and `NextController` None;
  - a `WillowMind`: its `InteractionProxy`, when set, destroyed (the field kept);
  - a pawn: the `PawnList` unlink, its `NextPawn` left.
  - The engine notification and `GearboxMind`'s two lookups change no game state the note saw, and are not run.
- **Terminal errors:** a `NotYet` after the first write gets "stopped after partial writes", its kind and record kept.
  Nothing is rolled back, and other errors are unchanged.

**Verified (automated, invented packages).** `ow-world` has 148 tests, 8 of them new on the toy package:
- the lists are newest first;
- the mind's destroy unlinks it and destroys its pool manager and proxy;
- the head and middle unlinks;
- the pawn unlink;
- the rule-engine and layer refusals before any write;
- a nested refusal after the first write is marked;
- the local and network player cases;
- the kill's end as script (`PawnDied` calling `Destroy()`).

Two older tests changed with the rule: Off no longer refuses every controller. `cargo test --workspace`: 2,201 passed,
0 failed.

**Installed (Off, `--slice-gate` strict):** 0 ok / 14 `NotYet` / 0 errors, unchanged. Links 3 and 4 are blocked by link
1 (`InitPlayerSkillTree`), so the kill does not reach the destroy on the installed slice yet.

**UNVERIFIED** in the running game: every rule above. Open: the rule engine's call, the begin-play step that adds to
the lists, and whether `AllControllers` must follow the list order (the note's open questions).
