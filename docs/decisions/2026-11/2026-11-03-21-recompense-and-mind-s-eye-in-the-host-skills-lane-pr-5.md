## 2026-11-03: Recompense and Mind's Eye in the host (Skills lane PR 5)

AI-assisted (Claude). Record: `docs/verification/NATIVE_SKILL_BEHAVIOURS.md` section 7. Behaviour change; read from the
behaviour graph and effect data, UNVERIFIED in game.

- **Recompense.** When an enemy takes health from Maya, with chance 0.1 x grade the attacker takes the same damage back
  (DmgType_Normal, non-critical skill hit); not while Maya is down. Hook (hub-approved): one line in
  `AOpenWillowWalker::ApplyDamageToPlayer` (health damage only, after the shield); the reflected hit goes to the attacker only, never to Maya.
- **Mind's Eye.** For gun hits that can crit, the shared hit dispatcher multiplies the event's crit bonus (base 2.0, live)
  by `PlayerCriticalHitBonus` / 2.0 = 1 + 0.05 x grade, cached until a grade changes; skill hits are not scaled. Hook
  (hub-approved): one line in `AOpenWillowWalker::ApplyShotDamage`. It has no effect until a target applies the bonus
  (range PR #76 adds that for head hits).
- Recompense rolls from the skills component's own seeded random stream (`-owskillseed=<n>`), so tests replay.
- Open, recorded: Sub-Sequence, Thoughtlock, Res, Chain Reaction, Kinetic Reflection, Reaper, Scorn, Backdraft.

