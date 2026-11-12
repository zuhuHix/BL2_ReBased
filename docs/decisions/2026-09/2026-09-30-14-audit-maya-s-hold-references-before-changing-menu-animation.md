## 2026-09-30: Audit Maya's hold references before changing menu animation

AI-assisted read-only audit using the existing bounded package CLI. No parser,
serialization layout or engine change. WillowGame's reflected
BodyWeaponHoldDefinition.AnimSetList inner property is Core.ObjectProperty;
the audit supplies that metadata through the existing --array-schema option
and checks each reference against the actual local Engine.AnimSet export.
Nine third-person Maya holds found, zero trailing bytes in their property
streams, eight unsupported WeaponActions retained explicitly. Pistol, rifle,
shotgun, SMG and sniper reference Rifle_Siren. Launcher references
RocketLauncher_Siren; unarmed references Unarmed_Siren. Default and Blizzard
contain no own AnimSetList. This supports sharing the Rifle set across five
classes, not the complete menu action/clip selection, inheritance or IK.

Representative existing-backend benchmark: UModel 1590 from official
https://github.com/gildor2/UEViewer, local external binary. Command
`umodel -path=<CookedPCConsole> -game=border -export -md5
-out=local/external/umodel/maya-launcher-anims-20260930 GD_Siren_Streaming_SF
RocketLauncher_Siren AnimSet`: .086 s, exit 0, 9 MD5 clips / 357936 bytes,
zero duplicate files or export failures, no Idle_Inventory. A separate
`-dump ... WeaponHold_Siren_Pistol BodyWeaponHoldDefinition` benchmark took
.114 s and reported Unknown class/no supported objects despite exit 0;
that class is unsupported by UModel. No new tool or architecture change.
Do not substitute a launcher Draw or additive clip for the inventory idle.

Reproducible own audit tool writes ignored local output, .914 s for nine holds.
CTest 6/6 (8.57 s), all nine package checks match, Python syntax/diff pass.
No runtime changes, new engine run or independent critic. Native computer-use
pipe remains explicitly unavailable on recheck, so no fresh game observation.
Full projection, action selection, Inspect and visual parity remain open.

