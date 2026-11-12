## 2026-09-18: Vertical slice adopted as the development priority

An outside software engineer reviewed the project's public roadmap and left
feedback recommending, in essence, that map porting (Phase 1's breadth) not
be finished before proving the harder, unproven phases: the script VM
(Phase 2), stock UE3 natives (Phase 3) and Gearbox's undocumented natives
(Phase 4). Their argument: porting maps is likely the easiest part of this
project, and discovering in Phase 2-4 that the approach doesn't hold after 80
maps are already ported would waste most of the project's calendar time on
the easy part. zuhu agreed and adopted a vertical-slice-first priority.

What changed: no code, no serialization, no parsing behavior. ROADMAP.md and
`docs/OPENWILLOW_ENGINE_PLAN.md` now scope Phases 1-4's gates to one map
(Sanctuary, already the most complete) and one Vault Hunter, plus one
hand-picked simple mission and a small set of guns, working end-to-end
(spawn, fight, loot, equip a gun, use a skill, complete the mission, die,
respawn), instead of requiring all 82 maps before gameplay work starts. Broad
map coverage (the remaining ~79 maps) and the remaining Vault Hunters are
deferred to Phase 5, after the slice gate is met. `CLAUDE.md` now records
this priority order for AI assistants so map or character breadth work isn't
picked up out of order without asking first.

Also adopted from the same feedback: an AI-usage norm recorded in
`CLAUDE.md` and `docs/OPENWILLOW_ENGINE_PLAN.md` §7, debug errors yourself
first (read the message, check documentation, Stack Overflow, GitHub issues)
before handing them to an AI assistant, and be able to explain generated code
before merging it. This is a working-norm change, not a parsing or
verification claim.

Not done and not claimed: which mission is still an open question (the
Vault Hunter was chosen the same day; see the entry above). No phase gate has
actually been met differently than before this entry. This is a
reprioritization of already-planned work, not new verified functionality.

