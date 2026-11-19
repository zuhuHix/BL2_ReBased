## 2026-11-19: Read the controller's and the pawn's destroy as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane ANALYST-ACTOR-DESTROY-CONTROLLER, analyst only (SLICE_RUN_PLAN.md section 3.1, lane 4). The kill in slice links 3
and 4 stops at the port's Off refusal of every controller's destroy. The new own-words note
`docs/verification/NATIVE_ACTOR_DESTROY_CONTROLLER.md` and three `NATIVE_INDEX.md` rows record, as behaviour only:

- How the kill reaches the mind's destroy: `WillowPawn`'s own `Dying.BeginState` ends with `Controller.PawnDied`,
  which (through `WillowMind` and `GearboxMind`) unpossesses the pawn and makes a non-player controller destroy itself.
- What the controller's destroy runs: the script `Destroyed()` chain (no `Logout` and no replication info for an AI
  mind; the pool manager destroyed; the perches cleared), then each class's world-drop step, which NATIVE_ENGINE_CORE.md
  step 6 left unidentified. For a controller that is the `WorldInfo.ControllerList` unlink; a mind also drops its rule
  engine and destroys its interaction proxy. A pawn's own later destroy unlinks it from `WorldInfo.PawnList`.
- What the world and the game info drop, and the Off answers step by step, with the writes in order and the named
  `NotYet`s: the rule engine's unread call, and a world that does not hold the controller or pawn list.

NATIVE_ENGINE_CORE.md step 6 now points at the note. Raw output stays under ignored
`local/analysis/destroy-controller/`; the Ghidra runs were read-only and took the machine lock. Nothing is
implemented. Every rule is UNVERIFIED in the running game; the note lists the bl-sdk checks and four open questions.

**Review r1 of #472, folded in.** Section 5 now:
- names two boundaries: the nested `Destroy()` (a refusal before `bPendingDelete` is clean there) and the kill (already
  written, so any `NotYet` is prefixed "stopped after partial writes", kind and record kept);
- checks every certain refusal at the earliest certain point, the pawn's `Dying.BeginState` entry with a non-player
  controller that holds this pawn;
- tables each call of the modelled path as supported or a typed `NotYet`: the actor step's notification and
  `GearboxMind`'s two lookups are `NotRead`, and they are no longer skipped when `MyRuleEngine` is empty. Whether the
  generic path's own omission of the notification becomes a refusal is left to the hub;
- tells an absent `PawnList` (`LayerAbsent`) from a modelled list without the pawn (unchanged);
- keeps the port's broader W5-STATES refusal for a mind or a corpse in a state, and keeps `Destroyed()` behind the
  `EndState` probe if stateful destroy comes later.

The live checks now take snapshots at hook entry and exit, at the pool manager's and the proxy's destroy, and around the
corpse's removal. The unnamed native calls stay open.
