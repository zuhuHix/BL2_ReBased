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

**Revised to the note's review r1 of #472 (20c516c4).**
- **The controller's refusals, in the note's table order** (section 5.2):
  1. the actor step's engine notification, `NotYet { NotRead }`;
  2. a `GearboxMind`'s two engine lookups, `NotYet { NotRead }`;
  3. a world without `ControllerList`, `NotYet { LayerAbsent }`;
  4. a set `MyRuleEngine`, `NotYet { NotRead }`.

  The first found is the answer, before any write. Since the notification runs on every controller's destroy, every
  Off controller destroy, the kill's mind included, answers it until it is read. The later rows and the world-drop
  step (section 5.4) are kept and tested directly, so their order holds once the first rows are read.
- **Pawns:**
  - a world without `PawnList` is `NotYet { LayerAbsent }`;
  - a modelled list without the pawn is not a refusal: the list is left as it is;
  - the generic path still omits the actor step's notification for non-controllers. That is the note's open
    question 6, a hub decision, kept as a documented UNVERIFIED gap.
- **Not done here** (section 5.1):
  - **The earliest certain check**, at the entry of the pawn's `Dying.BeginState`. `main` has no hook that runs before
    a script function's body; that is `Vm::wrap_script` from #464. Until it lands, the kill refuses at the mind's
    nested `Destroy()`, after the kill's own writes.
  - **The kill-boundary prefix.** The kill's driver would add it to that refusal. The destroy marks only the refusals
    that come after its own first write.
- Tests: `ow-world` has 149, 9 of them this lane's, with the toy's `DoomedPawn` for the terminal mark. `cargo test
  --workspace`: 2,202 passed, 0 failed. The installed gate is unchanged: 0/14/0, links 3 and 4 blocked by link 1.
