## 2026-10-06: native notes on the action skill runtime (Phaselock activation, lifetime, cooldown) (UNVERIFIED)

AI-assisted (Claude), analyst lane G17, Ghidra; own-words note `docs/verification/NATIVE_ACTION_SKILL.md`, no listing text. Key
press → `StartActionSkill` → `ServerStartActionSkill` (refusals: ladder, cooldown pool above 0 or skill active, vehicle seat; a
second press during the lock or cooldown does nothing); injured and weapon state are the skill's constraints inside
`Skill.Activate`, no target is not a refusal (the skill fizzles). Activate order: constraints, spawn the action-skill actor, the
skill's `OnActivated`, then the state delegate that refills the 13 s cooldown pool; the drain starts at `OnReleasedTarget`. The 120 s
`InitialDuration` is a backstop that asks once to deactivate. Base first-lock timeline (cast, lock 0.7 s, release 5.7 s, end 6.7 s,
ready 18.7 s) and every early end are listed. The reload abort comes from the cast special move's `bBlocksWeaponActions`, which
corrects NATIVE_WEAPON_TRIGGER_AMMO; other corrections (NATIVE_SKILLS, PHASELOCK_STOCK_DATA, NATIVE_PHASELOCK_PRESENTATION) are in the note.
All UNVERIFIED in game.

