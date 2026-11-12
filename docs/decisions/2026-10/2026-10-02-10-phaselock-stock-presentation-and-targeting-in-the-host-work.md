## 2026-10-02: Phaselock stock presentation and targeting in the host (work in progress, no suite run)

AI-assisted. Host and tooling; no parsing change. Written but **not verified by any suite**: after the module was
rebuilt at 09:18, Windows Application Control (Smart App Control) blocked `UnrealEditor-OpenWillow.dll`
(`GetLastError=4551`, Code Integrity events 3033/3077). Per project rules nothing was done to get around it; UE work
stops until the maintainer clears it.

- Presentation (`OpenWillowPhaselockFx.*`, `OpenWillowCombatTarget.*`, `OpenWillowWalker.*`): the decoded emitter
  templates are played with plane/mesh components (no Niagara, no new module dependency). From data: hand orb at the
  0.25 s notify on `L_Weapon_Bone` with the socket offset and scale 0.35; bubble scaled by bounds radius / 66.7, 0.2 s
  intro, collapse 0 -> 0.75 over the last 2 s; point light radius 500, brightness 4, a blue-violet colour; tattoo glow
  curve over 1 s. Host stand-ins (UNVERIFIED): what each stripped material does with its textures, the tattoo mask
  channel, the screen effect as a full-view quad, UE3 brightness -> UE5 intensity, burst timing, the dummy's auto-aim
  radius/aim point. The dummy keeps its idle (it has no PhaseLock clips).
- Rules from `NATIVE_PHASELOCK_TARGETING.md` (UNVERIFIED): screen-space magnetism target choice replaces the view
  ray and 30 cm sweep; reloading no longer refuses the cast, a holstered weapon does; going down ends the lock; the
  bob is timed from the cast and smoothed.
- Tooling: `tools/prepare_phaselock_fx.py` (manifest from installed data), `host/ue5/import_phaselock_fx.py` with
  `tools/import_phaselock_fx.ps1` (textures and meshes from UModel output under `local/`), `tools/run_phaselock_shots.ps1`.
- One in-engine capture run (before the block) showed the lift, the smoothed bob (about +/-16 uu), the collapse
  reaching 0.749 and the light ramp; defects seen: an overexposed white-pink core where web screenshots show a
  violet sphere with a dark core, a large violet light pool, loop sprites not showing, and first-use texture
  compilation delaying the hand orb to +0.53 s. Fixes for some of these are written and not run.
- Quest suite: check 65's reload refusal is replaced by "reload does not refuse" and "holstered refuses"; new checks
  for the presentation, an off-crosshair target and going down. Not run. Expected totals once it can run: 79 first-run
  checks (75 baseline + 4) and 11 resume checks (unchanged).
- Resumed later the same day under the maintainer's one-rebuild rule. A normal rebuild with no source change did not
  relink (the 09:18 DLL stayed, still blocked). After the one requested source change (the stale targeting comment in
  `OpenWillowQuest.h`), the module was relinked at 10:01 and Smart App Control blocked that DLL as well
  (`GetLastError=4551`, Code Integrity events 3033/3077/3118 at 10:01:40). UE work stopped again; nothing was done to
  get around the block.

