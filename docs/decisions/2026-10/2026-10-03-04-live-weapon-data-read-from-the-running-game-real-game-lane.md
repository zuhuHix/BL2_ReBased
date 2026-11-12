## 2026-10-03: Live weapon data read from the running game (real-game lane)

- **What.** `tools/real_game/scripts/weapon_dump.py` reads the stat properties of every weapon part, name part and type from
  the running game (SDK, main menu, saving blocked, saves restored byte for byte) into ignored `local/realgame/cards/`.
  `tools/real_game/live_overlay.py` and `golden_card_compare.py --live-data` use it as an overlay;
  `tools/real_game/openwillow_valuewatch/` logs chosen values from SDK load (local developer tools).
- **Confirmed in game (2026-10-03, method above).** The live values equal OpenBLCMM's static dump on all 78 values of W's
  61 changed objects; 37 objects carry real value differences against the cooked decode (ClipSize 7 types, ReloadTime 5,
  InstantHitDamage 2, 24 part effect lists), all in W's list. On the 69 golden weapons with exact parts the main four
  stats match 52/69 on cooked data and 69/69 with the live overlay.
- **Correction.** The source is not an online hotfix: the values exist 0.01 s after SDK load, before the `Micropatch`
  configuration, and its 23 entries touch no weapon. Not a package override (only `Startup.upk` defines the objects among 2,010
  packages) and not an installed mod. The origin is UNVERIFIED (load-time change by the game, or a decode gap). The wording in
  NATIVE_WEAPON_RULES section 7 still says "hotfix"; the weapon lane's owner should change it.
- **Open after the overlay.** Rocket launcher sale value about 4 % high (4 weapons), one status chance 33.3 vs 33.4, the
  level line rule, the stage-15 Maliwan pistol reload rounding (1.8 vs 1.7).
- **Not done / needs the maintainer.** The dump is game data and stays in `local/`; whether the port may read live values or
  must carry them some other way is the maintainer's call. No sensitive files touched; no tests rerun (no code under src/ or host/).

