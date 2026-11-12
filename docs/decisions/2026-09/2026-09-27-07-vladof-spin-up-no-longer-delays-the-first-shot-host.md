## 2026-09-27: Vladof spin-up no longer delays the first shot (host)

The maintainer found the Infinity's wait before firing wrong in play. The
host held every first shot for the evaluated spin-up (0.55 s). Installed data:
`WeaponType_Vladof_Pistol.BarrelSpinMode` is `BSM_SpinUpToFullFireRate`, and
`WeaponPartDefinition` has `StartingSpinUpFireIntervalMultiplier`, 1 on
`Default__WeaponPartDefinition` and not overridden by
`Pistol_Barrel_Vladof_Infinity`. Read together, the names suggest the gun
fires at once and the barrel's spin ramps the fire interval from
multiplier x interval down to the interval, which is flat at 1. The host now
skips the wait in that mode; other modes keep the old wait. `weapon_stats.py`
adds `spin_mode` and `spin_start_interval_scale` to the card. The native
behavior, the other `EBarrelSpinMode` values and the ramp shape are
UNVERIFIED; no real-game timing was measured.

Automated: `tests/weapon_stats_test.py` 3/3 (synthetic). The 8 local Infinity
recipes re-evaluate to `BSM_SpinUpToFullFireRate`, scale 1.

