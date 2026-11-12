## 2026-09-30: Inventory Maya displays the selected weapon

AI-assisted bounded host presentation change. Reuses UModel 1590's already
exported `GD_Siren_Streaming_SF/Rifle_Siren.Idle_Inventory`: 381 frames at 30 fps,
converted with the existing MD5-to-UE body reference and `--anchor none`, imported
as `Anim_InventoryRifle_Idle_Inventory`. No animation data is tracked. The
first commandlet attempt used a relative input path and failed; absolute input
import succeeded, zero errors/one reference-gathering warning. A representative
request for `Pistol_Siren` returned no matching export (0.274 s); package listing
contains Base, Rifle, RocketLauncher and Unarmed third-person sets, not that
requested name. No serialization or architecture change followed that failure.

The page reports selection through a dedicated preview message. UE resolves
the current stable inventory instance to its recipe ID before loading the
mesh; arbitrary page asset paths are not accepted. The display uses the armed
clip and existing `R_Weapon_Bone` attachment, with a matching ink hull/stencil
247. Gear selection clears the gun and restores unarmed idle. Cached-menu
reopen resets transient inspect/transfer state and reissues selection to the
new display actor; empty selection clears an old display weapon.

Verified: UE build (13.85 s); in-engine 35/35 actions, log
`run-20260930-012557.log`; weapon IDs/mesh/armed-idle load logged for pistol,
shotgun and SMG; gear logs mesh=0 / armedIdle=0. Open and comparison captures
show the gun attached and following the armed pose. CTest 6/6 (8.54 s), all
nine package checks match, JS syntax/diff checks pass; browser reopen callback
clears Inspect and reissues current selection. No new independent critic.

UNVERIFIED: exact weapon-specific hold definitions/launcher pose, material
paint for most display guns, all animation phases/long-gun clipping and parity
against a matched default-Maya original-game capture. Stock shader/3D-panel
projection gaps remain; 1:1 goal is not complete.

