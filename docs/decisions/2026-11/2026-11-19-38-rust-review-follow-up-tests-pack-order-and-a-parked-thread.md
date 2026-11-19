## 2026-11-19: Review follow-up tests: pack order for the action skill events, Force with a parked thread

AI-assisted (Claude Code, Claude Opus 5.5). Lane TEST-FOLLOWUPS. Tests only: no rule, binding or behaviour changes.

- **#407 follow-up (`ow-skills`, `action_skill/events_tests.rs`).** The event fixture can now register packs before
  and after the action skill pack. One new test runs the two `ActionSkill` event natives' delivery check in five
  orders:
  - ow-combat's pack before, or after, the action skill pack;
  - the kernel pack before and after it (so three registrations, with the action skill pack's own);
  - the kernel pack and ow-combat's pack mixed on both sides.

  Delivery and payload are the same in every order.
- **Found, not fixed (reported to the hub).** ow-combat's pack cannot be registered twice on one VM. Its `CombatState`
  service is guarded, but its pools service (`ow_world::pools::ResetIdleTimerOff`) is added unguarded, so a second
  `register` fails with "a service of this type is already added". No caller registers it twice, and the kernel pack
  is the one documented as idempotent. So the test registers ow-combat's pack once per world, and the comment says so.
- **#405 follow-up (`ow-behavior`, `kernel/natives/tests.rs`).** A delayed toy provider (`Go` runs one behavior after a
  1 s link delay) is initialized on two consumers, and `Go` parks a thread on each. One consumer is forced out
  (`ForceUnregisterBehaviorConsumer`). The other, as the control, is ticked past the delay, and its thread runs. The
  forced process has left the table, so no tick reaches its provider, and its parked thread never runs, on that tick
  or on a later one.
- **#413 follow-up: not done.** The `foreach AllParts(...)` test needs the `AllParts` iterator, which is in #413. #413
  was still open on `origin/main` (0a23f13) when this lane ran, so the test belongs after it lands.
- **UNVERIFIED.** These tests say what the port does on invented data, not what the game does.
