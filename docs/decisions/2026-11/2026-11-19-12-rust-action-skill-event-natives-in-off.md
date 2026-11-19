## 2026-11-19: `ActionSkill.OnActionSkillActivated` and `OnActionSkillDeactivated` in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane FIX-PHASELOCK-CAST part B (implementer; slice plan step 4,
section 3.5). Implemented from `docs/verification/NATIVE_ACTION_SKILL.md` section 9 and its summary row only. This
lane opened no raw analysis output, no Ghidra project and no REA, and it does not implement the two natives its
author read in part A (`BeginNondeterministicProviderRegistration`, `ForceUnregisterBehaviorConsumer`).

- **The rule (Off, `crates/ow-skills/src/action_skill.rs`).** Each native fires a behavior event named after the
  function on the provider of the consumer its handle names, with every link followed. The object parameters go in
  order as the event's positional payload: pawn and target for `OnActionSkillActivated`, the pawn for
  `OnActionSkillDeactivated`. The payload rule is the kernel's existing one (NATIVE_MARCUS_USE_CHAIN.md, "Event
  payload to named variable"). Both natives return nothing. A handle nobody owns (-1 among them) fires nothing
  (NATIVE_BEHAVIOR_CONTEXT.md, "kernel entry points given -1").
- **`NotYet { NotRead }`, naming the question.** The note says "the action skill's provider" and does not say which
  providers the delivery reaches, so a kernel consumer with no provider, or with several, answers NotYet. So does a
  handle another crate gave out, because the note does not say how such a consumer receives the event.
- **Reference.** Unbound: `src/` has no stand-in, so the VM's stub policy answers, as before.
- **Seams.** `ow-behavior`'s kernel pack gains two read-only queries, `provider_paths` and `consumer_pid`. The action
  skill pack now registers the kernel pack in Off, as `ow-combat`'s hit pack does, so the consumer table exists when
  these natives run. In the installed run that registration changed no other line of the Off output.
- **Verified (automated, local).** Four synthetic tests on invented providers (`action_skill/events_tests.rs`, with
  `ow-behavior`'s invented package builder included by path, as `ow-combat` does):
  - delivery with the payload in order, and a None target passed on;
  - nothing fires for -1 or an unknown `PID`;
  - NotYet for zero or two providers and for a crate's handle;
  - Reference refuses both as unbound.
- **The installed cast (ignored test, `OW_BL2_ROOT`).** Before, `LiftActionSkill.OnActionSkillStarted` stopped at
  the unbound `ActionSkill.OnActionSkillActivated` in both modes. After, in Off, it gets past it (the archetype's
  handle belongs to nobody, so nothing fires) and stops at the unbound `GearboxPawn.GetSMComponent()`. No record reads
  that native, so it needs an analyst first. Reference is unchanged.
- **UNVERIFIED.** Every rule above. Nothing was checked in the running game. The Phaselock provider's behaviors on
  `OnActionSkillDeactivated` did not run in any of these runs.

Edits: docs/decisions/2026-11/2026-11-19-02-rust-phaselock-cast-vlerp-and-the-provider-natives-read.md
Review r1 of PR #390 (two low doc fixes, folded into this change): the 2026-11-19-02 VLerp bullet now says Reference
differs from the C++ there (the C++ VM has no VLerp and logs a stub where Rust answers, as for `VInterpTo`), and
NATIVE_BEHAVIOR_PROVIDERS.md section 1 now points to section 3, step 4 for the -1 handle write.
