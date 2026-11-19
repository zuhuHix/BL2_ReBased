## 2026-11-19: The recorded resource pool hooks matched by their installed package

AI-assisted (Claude Code, Opus 5.5). Lane LINK-1-NEXT-STOP, implementer.

- **Sources:** `docs/verification/NATIVE_RESOURCE_POOL_IDLE.md` (analyst lane FIX-POOLS-OFF, not this thread) and the
  installed packages' class exports, read with `ow-package --exports`. No native code or analysis output was opened.
- **No real-game session:** every rule is **UNVERIFIED** in the running game.
- **Unchanged rules:** no dependency was added, no bounds check was loosened and no existing `NotYet` guard was
  relaxed.
- **The base:** stacked on `fix/rust-init-player-skill-tree` (#475), first on 772c19b4, then rebased onto its r1
  head 07017c6d; the installed stop was re-checked there and is unchanged.

**What was found.** The stop was traced on a local merge of main 17c07e2c, #475 and #458 (9e880cc6), with the
installed Off `--slice-gate`. Links 1, 3, 4, 5, 13 and 14 stopped in `maya:8`.
- On main plus #475 the first stop is `WillowGame.BlackMarketUpgradeAttributeValueResolver`, which #458 implements.
- With #458 too it is "stopped after partial writes: `WillowGame.AmmoResourcePool` native idle-reset hook/override
  (stamp already written)".

The note reads that hook. For `ResourcePool`, `HealthResourcePool`, `AmmoResourcePool` and `ExperienceResourcePool`
it does nothing; for `ShieldResourcePool` its chain is unread. The note names the classes without their package.
- `ow_combat::hit::pools::reset_idle_timer` matched all five under `Engine`.
- The installed packages declare `ResourcePool` and `HealthResourcePool` in `Engine`, but `AmmoResourcePool`,
  `ExperienceResourcePool` and `ShieldResourcePool` in `WillowGame`.
- So the game's own ammo pool met the refusal meant for an unrecorded subclass.

**What changed** (`crates/ow-combat/src/hit/pools.rs`, Off only; Reference unchanged).
- The recorded identities are now the installed ones: `Engine.ResourcePool`, `Engine.HealthResourcePool`,
  `WillowGame.AmmoResourcePool` and `WillowGame.ExperienceResourcePool` do nothing after the stamp;
  `WillowGame.ShieldResourcePool` keeps its `NotYet`. Any other class still refuses after the stamp.
- The tests' invented packages now declare the three Willow classes in a `WillowGame` package. An `Engine` class of
  the same name, `AmmoResourcePool`, is kept as a decoy that still refuses.

**Installed Off `--slice-gate`** (the same local merge, with this change):
- Links 1, 3, 4, 5, 13 and 14 get past the pool hook in `maya:8`. They now stop at the unbound native
  `GameEngine.GetOnlineSubsystem()`, which the gate grades as an error, not a `NotYet`.
- That native is read in NATIVE_LOGIN_STATUS.md (#474, decision 2026-11-19-89). This thread was its analyst, so a
  fresh implementer binds it.
