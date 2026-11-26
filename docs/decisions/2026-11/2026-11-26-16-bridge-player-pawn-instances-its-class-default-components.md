## 2026-11-26: the mission bridge's player pawn gets its own copies of its class default's components (Off)

AI-assisted (Claude Code, Claude Opus 5.5). Lane LINK1-RBPHYSICS-UNLOADED-COMPONENT, on main 5ce85058. No native was
read; no bl-sdk session was granted. **UNVERIFIED in the running game.**

**The stop.** On main plus #754 (merged locally only), links 1, 3, 4, 5, 13 and 14 stopped in `maya:8` at
`WillowPawn.InitializeRBPhysics`' entry check, "a component the store has not loaded (a load is a write)". The refusal
now names the entry's class and path. It was `GearboxFramework.SpecialMoveComponent`
`WillowGame.Default__WillowPlayerPawn.SpecialMoveComponent0`, the first of five such entries in the pawn's
`Components`. The others are the class default's `GearboxDialogComponent0`, `StatComp`, `MyLightEnvironment` and
`MyArmsLightEnvironment`. Every one was a store handle owned by the class default. The two resident entries were the
`Mesh` and `CylinderComponent` that body composition attaches.

**Why: a port bug (a).** The bridge makes the player's pawn from the class defaults (decision 2026-11-16-54) with
`instantiate_from_defaults`, a plain slot copy. So its `Components` shared the class default's own sub-objects, which
nothing loads. The world's spawn gives every actor its own copy of each component its template owns (public UE3
component instancing, decision 2026-11-19-44), and the bridge's target dummy already does the same in Off. The player
pawn was the one path left out. Neither a harness preload (b) nor an unread rule (c) fits: what to copy is the
existing instancing rule, and loading the class default's sub-objects is that rule's own pass 2.

**The fix.** In Off, `ow-mission`'s bridge makes the player pawn this way:
- the instancing plan on the class default (`ow_world::actors::plan_components`), before the pawn is made;
- the pawn from the class defaults, as before;
- `apply_plan`: the copies, outered to the pawn, with the references re-pointed;
- each copy whose class declares `Owner` gets the pawn there, as the dummy's copies do.

A refusal after a write is marked "stopped after partial writes" by #518's counters. Reference is unchanged.
The spawn's `load_own_definitions` step (the actor's own definition references loaded before it is made) is not done
here: the bridge pawn is not spawned, and its definitions load where the script reads them (review of #757, LOW 4).
`player_pawn_tail.rs`' not-loaded refusal now names its entry (`not_loaded`), as `tree_ticks` names its object. The
pawn's origin stays the class defaults: spawning it from Maya's archetype (`Pawn_Siren`) instead is the hub's call
(decision 2026-11-16-54).

**Measured** (installed Off strict `--slice-gate`, release build, under the machine lock, on the local merge of main
5ce85058, #754 and this change): 6 ok / 8 NotYet, 42.9%. Links 1, 3, 4, 5, 13 and 14 pass the entry check and now stop
in the same native's second pass at "Actor.GetBoundingCylinder: the colliding components' bounds" (`LayerAbsent`,
"component bounds (the headless world has none)"). The prediction was 2.A's rigid-body `LayerAbsent`. The cause: the
copies are resident, but none is in the spawn's attach record, because the bridge pawn is not spawned. The note's
answer for a live entry it cannot place as attached is that bounds refusal (NATIVE_LINK1_RBPHYSICS_AND_AFTER.md 2.A).
Whether the bridge pawn should also get the spawn's component attach (`component_attach`) is the hub's call.
