## 2026-11-02: merge of main into `t3/ai-melee-and-bleedout` (host firing, muzzle, turn-in loot)

AI-assisted (Claude). One merge commit; where both lines of work implemented the same host behaviour, main's implementation
is kept and the branch's callers adapted. Nothing here was re-checked in the running game. The paragraph as first written said
the in-engine suites were not run for this merge; that was before the hub ran them. Recorded afterwards (PR #59 body):
`tools/test_quest.ps1` at 5be3581 (the merge) and again at a8a3dd5 (after review fixes 1), each first run PASS 111/111 and
resume PASS 11/11 (host behaviour only, not parity with the original game); CTest 19/19 and `verify_packages` 9/9 at 5be3581,
with the UE5 module compiling. `tools/test_weapon_firing.ps1` was not run (no generated firing recipes in that worktree).
683a083 merges a8a3dd5 into an unmoved main, so it is the same tree.

- **Gun firing.** The branch's in-walker bullets (lane Hf, 2026-10-08: card `firing` block from `tools/weapon_refresh_firing.py`,
  `FOpenWillowFiringMode`, `AdvanceBullets`, `FShotRecord`) are replaced by main's firing from the parts (`OpenWillowFiring`,
  `OpenWillowProjectile`, `src/weapon_firing.*`, recipe-level `firing` from `tools/weapon_firing.py`). Kept from the branch: the
  level-up weapon damage scale (`ShotDamage`, script swap 9) now sets main's per-pellet damage for light bullets, rockets and
  traces. Not carried over: the per-bullet momentum (type `InstantHitMomentum` x 100; main passes none) and the branch's
  `TraceExtent` box sweep.
- **Quest suite, shot cases 14/15.** They keep the branch's structure (fire, wait for the landing, multi-pellet shotgun
  fixture) and now read main's tally (`OpenWillowFiring::Counts`) and the live `AOpenWillowProjectile` count. Re-expressed:
  `bullet_has_not_landed_in_the_pressing_frame` (trace guns exempt), `wrong_element_shot_...`, `multi_pellet_gun_fires_its_pellet_count_in_one_press`,
  `damage_called_once_per_pellet_that_hits`; `ammo_taken_once_per_press_not_per_pellet` unchanged. Dropped because they tested
  the replaced implementation's internals: `pellet_directions_inside_pattern_and_cone_bounds`, `shot_momentum_is_type_momentum_times_100`,
  `*_bullet_lands_after_its_flight_time`, `firing_pattern_lines_sequential_scaled_random_rules` (main's pattern rules and the flight
  step (speed, gravity, own-motion friction) are unit-tested by CTest `weapon-firing-math`; no in-engine check now compares a
  projectile's impact delay with distance and speed, so runtime flight timing is OPEN).
- **Muzzle point.** Main's view-model `MuzzleSocket` (`weapon_view.json`) first, then the branch's recipe `sockets` Muzzle
  (`GunSocketLocation`, WEAPON_VISUALS.md section 17, renumbered from 11), then main's camera-space fallback (logged once per gun).
  The base's Barrel + 27 cm estimate, which main had dropped, stays dropped. Whether the recipe socket's bone frame holds on
  main's assembled library meshes is UNVERIFIED.
- **Turn-in loot.** The branch's maintainer decision (2026-10-07: no item reward) wins over main's generator-rolled reward:
  the quest rolls and drops nothing; the generator itself and `-owweapongentest` are unchanged.
- **Lent weapon.** The branch's rule (grant to the backpack, not equipped; kept after the objective) is kept; main's
  `PreLendSlot` is dropped with it.
- **Notes.** The branch's `NATIVE_WEAPON_FIRING.md` (lane G13) is now `NATIVE_WEAPON_TRIGGER_AMMO.md`; main's note keeps the
  name. Links in NATIVE_INDEX, NATIVE_ACTION_SKILL, NATIVE_PLAYER_MOVEMENT and PROJECTILE_FIRING_PATTERNS_LIVE point to the new name.

