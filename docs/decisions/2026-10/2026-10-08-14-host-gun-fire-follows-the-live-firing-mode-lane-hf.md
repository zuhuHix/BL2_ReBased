## 2026-10-08: host gun fire follows the live firing mode (lane Hf)

 AI-assisted (Claude). The host fired one instant ray per press along a
made-up figure-eight. It now follows the observed rules of PROJECTILE_FIRING_PATTERNS_LIVE.md (R1, R3 to R6; section 7 items 1 to 6 and 8): the
item card carries a `firing` block written by `tools/weapon_refresh_firing.py` from the L7 live dump (the cooked reader still cannot decode
`FiringPatternLines`; no offsets were invented); a press makes `ProjectilesPerShot` bullets, each with the pattern offset then the weapon spread cone,
flying at the card's `projectile_speed`, 16,384 uu range, swept per tick, damage applied per pellet that hits. **Verified (host suite, not the real
game):** 10-pellet shotgun press fires 10 bullets and takes the ammo once; pellet directions stay inside pattern + cone (6.89 of 7.91 deg); 8 of 10
pellets landed, one damage call each; landing is never before length/speed; the pattern rules on an invented weapon. Quest 117/117 + resume 11/11,
mover 16/16, inventory 49/0. **UNVERIFIED:** accuracy-pool cone (none in the host), random line draw, two-axis rectangle shape, scale above 1, spread
density, positive friction/Acceleration, ricochets, rocket and beam modes (carried, fired as bullets with a warning), the Jakobs rifle's type momentum,
tracer look; the in-game parity of any of it. The damage factor (open item 5 of the record) is NOT applied: the host damage is still the card
damage.

Orchestrator rerun on the combined tree: UE build succeeded, test_quest 117/117 + resume 11/11, test_mover 16/16, test_inventory_actions 49/0; ctest 13/13 (the new `weapon-refresh-firing-synthetic` registered in `CMakeLists.txt`, a sensitive file: one `add_test` line).

