## 2026-11-19: Review follow-up tests: pack order, Force with a parked thread, a script AllParts loop

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
- **#413 follow-up (`ow-inventory`, `equip/natives/script_tests.rs`).** An invented `WillowGame.upk` holds a weapon
  class with the eleven-slot definition data, the native iterator `AllParts` and one script function, assembled with
  `ow_package::testkit::script::Asm`: `foreach AllParts(DefinitionData, Cursor, false) { Seen = Cursor; } return
  Cursor;`. The loop is called by reference (`FinalFunction`), so it runs through `ow_vm::interp::iter`'s `engine`.
  - In Off, the body runs once per visited part (Body, then Barrel; the Title name part is skipped without the flag),
    and the out argument is None after the loop runs to its end (NATIVE_EQUIP_PARTS.md section 2).
  - With no slot set, the body never runs, and the out argument is None as well.
  - In Reference the same loop is refused at the unbound `AllParts` (`StubPolicy::refuse_all`).

  The out argument is an instance variable, because the toy package's locals carry no class reference. #413 landed
  on `origin/main` (5554ead) during this lane, and the branch was rebased onto it before this test was written.
- **UNVERIFIED.** These tests say what the port does on invented data, not what the game does.
