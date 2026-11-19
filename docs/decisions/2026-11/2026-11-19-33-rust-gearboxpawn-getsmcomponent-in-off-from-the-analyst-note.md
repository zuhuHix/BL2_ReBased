## 2026-11-19: Rust GearboxPawn.GetSMComponent in Off, from the analyst note

AI-assisted (Claude Code). Lane FIX-GETSMCOMPONENT, the fresh implementer of `NATIVE_SPECIAL_MOVE_COMPONENT.md`
(written by the analyst lane GETSMCOMPONENT, decision 2026-11-19-28; read on `research/rust-getsmcomponent` at
5faa593c and again at d81818d after its review rewording, which left the rule unchanged; it had not landed when this
lane started). This lane coded from the note alone and opened no analysis output. There was no real-game session, so
the rule is **UNVERIFIED** in the running game. No dependency was added, no bounds check was loosened and no existing
`NotYet` guard was relaxed.

- **What was added** (`crates/ow-skills/src/action_skill.rs`, the action-skill pack, Off only):
  - `GearboxPawn.GetSMComponent()` is bound under its native key (the key the Off cast's stop named).
  - It answers the pawn's `SMComponent` as it is: nothing is created, checked or registered. A None field answers None.
  - A class that declares no `SMComponent`, or holds a non-object there, is a typed error.
- **The open question it keeps open.** The game's pawn is expected to hold its own instance of the component: a
  default sub-object, copied per actor at spawn (stock UE3, UNVERIFIED in this game; the note's bl-sdk check settles
  it). A pawn made from class defaults in this port shares the class default's sub-object,
  because `instantiate_from_defaults` copies the slots as they are.
  - Whether the port should answer that shared object is the note's open question 1. Answering it would let a `Play`
    on one pawn write the template.
  - So a pawn whose component's outer is not the pawn itself (a class-default or archetype sub-object) answers
    `NotYet { NotRead }` naming that question. The pawn's own instance is answered, the same on every call.
- **Reference** is unbound: the C++ has no stand-in (R3).
- **Tests** (2, `action_skill/sm_component_tests.rs`, invented classes):
  - the own component, the same on every call; None; a shared template's component refused with the question named;
  - unbound under Reference, bound in Off.
- **Installed, before and after** (pc-hummer; `phaselock::installed::installed_game_maya_skills_duration_and_lift`;
  outputs under ignored `local/p2/fix-getsmcomponent/`):
  - Off: one line changes. The cast (`LiftActionSkill.OnActionSkillStarted`) stopped at the unbound
    `GearboxPawn.GetSMComponent()`. It now stops at this native's `NotYet`: the test's Maya pawn is made from
    `WillowPlayerPawn`'s class defaults, so its `SMComponent` is the class default's shared sub-object.
  - Reference: byte-identical (it stops earlier, in the parent `OnActionSkillStarted`, as the note says).
  - So the cast's next stop is open question 1 itself: giving a class-default pawn its own component instance (how
    instanced sub-objects are copied at spawn, which `actors::spawn` also refuses for a template other than the class
    default). It is a port decision before it is a native.
  - After it, by the note, the cast goes on in script to `GetPlayerAnimation`, and the first unbound native among
    `Pawn.IsFriendly`, `IsAliveAndWell`, `IsPhaselocked`, the skill manager's `IsSkillActive`, `IsInjured`,
    `IsInjuredDead` and `SpecialMoveComponent.Play` (not run: the cast does not get there yet).
- **`--slice-gate`** (main aaa93bb3's binary against this branch's): the rows are byte-identical, strict 0/5/9 and
  past-errors 0/13/1. Link 9 stays `notyet`, because it has no headless driver (the cast has no run step).
- **Not changed:** the note and its index row are not on main, so this branch leaves them alone; the note's status
  lines are for when it lands.
