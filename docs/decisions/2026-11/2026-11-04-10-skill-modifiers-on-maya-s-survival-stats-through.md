## 2026-11-04: Skill modifiers on Maya's survival stats through FOpenWillowMayaState::Effective (Skills lane PR 6)

AI-assisted (Claude). Record: `docs/verification/NATIVE_SKILLS.md` section 4.8 "Survival stats". Behaviour change;
modifier rules are native reads, UNVERIFIED in game.

- `FOpenWillowMayaState` (maya lane's file) gains a `Modifiers` callback that `Effective()` applies after `Base`; the
  skills component installs it at BeginPlay and clears it at EndPlay. It sets max health (Restoration), health
  regeneration (Sustenance, Elated), shield capacity and recharge delay (Ward) and ground speed (Fleet) with the
  modifier stack formula. Evaluated on every read (the game refreshes every 0.5 s).
- Condition and formula attributes read the pools' current values and their `Base` maxima, so Fleet's shield-empty
  constraint now resolves, and nothing recurses into `Effective()`. Regeneration moved from the skills tick (`Heal`) to
  the pool's own `HealthRegenRate`.
- Review fixes: formulas on max health (Sustenance's missing health, Elated) read the effective maximum just computed;
  `Effective()` is computed once per frame (and on a `Base` change or a grade change), so reads within a frame agree; a
  level-up refill and a full new shield fill to the effective maxima; `Tick` clamps health and shield when a maximum
  drops (host rule, UNVERIFIED as stock behaviour). New `OpenWillow.MayaState` cases.

