## 2026-11-26: the mission bridge's player pawn takes the spawn's component attach (Off)

AI-assisted (Claude Code, Claude Opus 5.5). Lane BRIDGE-PAWN-ATTACH, the hub's ruling (1) on #757, stacked on #757
(decision 2026-11-26-16) over main 457c46e4. No native was read; no bl-sdk session was granted. **UNVERIFIED in the
running game.**

Edits: docs/decisions/2026-11/2026-11-26-16-bridge-player-pawn-instances-its-class-default-components.md

That entry gains a line saying the spawn's `load_own_definitions` is not done for the bridge pawn (review of #757, LOW
4).

**What changed.** In Off, the bridge's player pawn (made from the class defaults, decision 2026-11-16-54, with its own
component copies since decision 2026-11-26-16) now also takes the component attach step every spawned actor takes
(`ow_world::actors::component_attach`, NATIVE_SPAWN_COMPONENT_ATTACH.md, #605/#610):
- its preflight (`plan`) runs on the class default with the instancing plan, before the pawn is made, so its refusals
  leave no pawn;
- after the copies are made and each gets the pawn as `Owner` (as before), `apply` writes the generic attach fields,
  appends the two exact AI component classes to `AllComponents`, and publishes the attach record last. An `Owner`
  write after publication would make an entry stale, which is why it comes after the `Owner` writes.

The copies in the pawn's `Components` thus land in the attach record under the pawn. Reference is unchanged. The
pawn's source stays the class defaults: the hub queued an analyst read of what the game spawns Maya's pawn from
(ruling 2 on #757).

**Test fixtures.** The step is as strict as for a spawned actor: the class must declare `Components` and
`AllComponents`, and each component the attach fields. The bridge's toy `WillowPlayerPawn` and its component declare
them. The slice tests' toy `WillowPlayerPawn` declares the two arrays; its class default holds no component, so the
step has nothing to do there.

**Measured** (installed Off strict `--slice-gate`, release build, under the machine lock): 6 ok / 8 NotYet, 47.0%, as
on main 457c46e4. Links 1, 3, 4, 5, 13 and 14 move from "Actor.GetBoundingCylinder: the colliding components' bounds"
to NATIVE_LINK1_RBPHYSICS_AND_AFTER.md 2.A's "WillowPawn.InitializeRBPhysics: a component's rigid body"
(`LayerAbsent`, a physics layer), as predicted. No step count moves, so X is unchanged. Links 8 to 12 (equip,
phaselock, the three mission links) stay ok; links 6 and 7 are unchanged at the template `NotRead` in touch.

**Review of #757's LOWs** (a separate commit): `PLAYER_PAWN_CLASS` and `player_pawn_with_components` sit above
`new_object`'s documentation; a failure of `apply_plan` or of an `Owner` write frees the pawn (and the copies), as
the spawn frees its actor; a test of a plan refusal (a non-component sub-object of the class default in its
`Components`: `NotRead`, no pawn made or cached); decision 16's line above. The toy component's `Owner` and its
assertion came with this lane's first commit.
