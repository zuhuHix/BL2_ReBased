## 2026-11-03: gun parity, host side: muzzle socket mirror removed, ADS refit, slice SMG ammo pool, reload clips (Weapons)

AI-assisted (Claude). Record and numbers: `docs/verification/WEAPON_VISUALS.md` (open list after section 14) and
`WEAPON_PROJECTILES.md` section 4. Host behaviour checked in the host only; nothing here was compared with new real-game frames.

- **Muzzle flash at the rear of the SMG and shotguns: fixed.** `WeaponSocketWorld` mirrored the cooked socket offset across Y;
  the decoded-socket path and the mesh-space reference do not. The gunprobe showed the mirrored SMG muzzle 33 cm behind the gun's
  root and the unmirrored point equal to the decoded socket (SMG 116.9 cm, shotgun 162.2 cm forward of the camera); the pistol and
  AR moved less than 0.3 cm. The mirror is removed. Firing suite after it: 14 PASS / 0 FAIL.
- **ADS refit (UNVERIFIED host rules).** The eye sockets went through the same mirror. The fit now projects the eye socket onto the
  plane through the gun's `Root` across the barrel axis (only the sight's height and side offset count), and converts the 18 px
  drop at the gun body's depth. SMG, shotgun and AR offsets change by at most 1 cm; the pistol's changes and its fit still does
  not converge (3.2 cm side error, as before the change): open.
- **Slice SMG ammo pool.** `UOpenWillowInventory::ResolveAmmoType` matched only the type `SMG`; the recipes carry the game's
  long SMG type label (the one `tools/weapon_stats.py` writes), so the slice SMG had no ammo pool and could not reload. One alias row; inventory self-test check added.
- **Reload clips.** Regenerated on a fresh machine with the existing tools (commands in WEAPON_VISUALS.md); the pistol, AR and SMG
  reloads now play with their arms clips in the host. The shotgun has no reload clip (no shotgun family in
  `tools/weapon_reload_anims.py`).
- **Tools.** `tools/run_weapon_assembly_shots.ps1` no longer defaults its hub lock to another machine's user folder
  (`OPENWILLOW_HUB_LOCK`, else none; a missing lock folder fails fast).
- **Waiting on real-game frames:** reload mid-poses, SMG hip size (the old comparison used a different SMG roll), ADS widths and the
  pistol ADS position. Elemental impact explosions wait on a maintainer decision (native read or live hook).

