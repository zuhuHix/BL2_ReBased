## 2026-11-19: A mind's `QueryInterface` answer is `Controller`'s rule

AI-assisted (Claude Code). Analyst lane ANALYST-GEARBOXMIND-QUERYINTERFACE (link 9). Documentation only: no code
changed, and the lane implements none of it (clean room). **Every rule is UNVERIFIED in the running game.**

- **The note:** `docs/verification/NATIVE_GEARBOXMIND_QUERYINTERFACE.md`, with a `NATIVE_INDEX.md` row. The open
  item in `NATIVE_PAWN_AI_INTERFACE.md` now points at it.
- **What was read** (local Ghidra, read-only, under the machine lock; raw output under ignored `local/analysis/`):
  - `GearboxMind` and `WillowMind` give no answer of their own. They share `Controller`'s, as `AIController` and
    `GearboxAIController` do;
  - `Controller`'s rule: a repeat guard; `IWorldBody` and `ISoundBehavior` go to the controller's pawn first; then
    the plain answer (its own declared interfaces); then its pawn; then, only when a pawn declined,
    `PlayerReplicationInfo`. It writes, loads and draws nothing;
  - "its pawn" is `Pawn`, except that `WillowMind` reads `MyWillowPawn`. The fitted `WillowMind` layout ends at the
    class size its native registration gives, a structural check;
  - `WillowPlayerController` has its own answer, which uses `Controller`'s. It stays open, as do `PlayerController`
    and `GamePlayerController`, which were not read.
- **Link 9's answer:** asked `IAIInterface`, the target's `WillowMind` answers itself, because its class lists the
  interface.
- **Proposed Off rule for a separate implementer:** register it for `Engine.Controller` in `ow-combat`'s
  pawn-interface registration, replacing the `GearboxMind` open answer. `Engine.PlayerController` gets a NotYet
  (NotRead) answer, and `WillowPlayerController`'s open answer is kept. After review r1 of #589, the rule must be
  read-only along the whole chain it reaches:
  - a plain answer over interface-table entries resolved at set-up, with an unresolved entry as NotRead;
  - read-only forms of the delegate rules, with a delegate rule that has none as NotPorted;
  - fields checked only when a step reaches them, with an outcome table that pins Script, NotRead, LayerAbsent and
    NotPorted;
  - a controller-keyed repeat guard, as in the game, where the port's dispatch guard keys by (object, interface).
  Details and the tests are in the note's section 5. Reference is unchanged.
- **Local probe (never pushed):** a merge of #535, #579, #564, #548, #555, #560, #523 and the link 9 spawn branches,
  with the open `GearboxMind` answer changed to the plain answer for `IAIInterface` only. Installed Off strict
  `--slice-gate`: 1 ok / 3 NotYet / 10 errors (1 / 4 / 9 without the change). **In the probe's world**, link 9
  becomes an error at `cast`, on an unbound `GearboxMind.GetAIComponent()`. The probe's registrations cause that
  error: link 9's world does not register `ow_combat::holds`, which in Off binds `GetAIComponent` to a NotYet. With
  that pack, the stop would be that NotYet. Either way that native is the next stop.
- **Live check (bl-sdk; not run, no session granted):** the note's last section.
