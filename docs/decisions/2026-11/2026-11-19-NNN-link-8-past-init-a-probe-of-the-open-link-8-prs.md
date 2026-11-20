## 2026-11-19: Link 8 past init: a probe of the open link 8 PRs

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-LINK8-NEXT, an analyst lane. No real-game session was run,
and no native was read for this record. Every stop below is what the host did on a throwaway probe tree, not parity.

**The probe.** A local merge that was never pushed and is deleted: main at 4c4522d7, plus #596 (the gestalt mesh layer),
#613 (the gestalt data layer), #616 (AddAttributeModifier, which contains #615), #577 (GetExpLevelFromManufacturerData)
and #599 (SetSkeletalMesh). #572, #573 and #584 are on main already. Each later PR went in only once the run stopped at
the native it ports, as the lane brief asks. The installed Off strict `--slice-gate` ran under the machine lock, release
build. Logs are under ignored `local/analysis/link8next/`.

**Link 8 (`equip_gun`) on main 4c4522d7.** Its steps are `make`, `init`, `bases`, `equip` and `unequip`. It passes
`make` and stops in `init` at the unbound `InventoryBalanceDefinition.GetExpLevelFromManufacturerData(object,int)`
(an error), the stop the hub measured.

**The stops past it, in order**, each with the open PR that already ports it:

| # | Stop in `init` | Ported by |
|---|---|---|
| 1 | `GetExpLevelFromManufacturerData`, unbound (error) | #577, open |
| 2 | `SkeletalMeshComponent.SetSkeletalMesh(object,bool)`, unbound (error) | #599, open |
| 3 | `BuildCompositeMesh: a definition it reads is not loaded` (NotRead) | #596's own preflight; see below |
| 4 | `SetSkeletalMesh: a mesh change (bones, the animation tree's per-bone state, physics and render state)`, LayerAbsent (the component's bone transforms), after partial writes | #599's own guard; no native |

With every PR above in, link 8 still passes only `make` (1 of 5 steps), but the stop moves from an error to a NotYet.
The whole gate on the last probe was 4 ok / 10 NotYet / 0 errors of 14.

**GetExpLevelFromManufacturerData (for the hub's BIND-GETEXPLEVEL-EQUIP-WORLD question).** Main has the rule:
`ow-combat` `loot.rs`'s `exp_level_for` (NATIVE_LOOT.md), but only the loot rolls call it, and nothing binds the
script native. #577 binds it in Off through `ow_combat::hit::Pack`, which the equip world already registers. #577 also
adds what the equip world needs and main lacks: harness inputs for the definition data's `ManufacturerDefinition` and
`ManufacturerGradeIndex`, set only on link 8's and `--equip-run`'s weapon routes (`WeaponRoute`, review r1 of #577).
In the probe, with #577 in, link 8 passed that call. So the native needs more than a bind in the equip world, and #577
already does both parts; a separate BIND-GETEXPLEVEL-EQUIP-WORLD lane would duplicate it. Probe note: main now reaches
link 8 through `prepare_line_typed`, the pickup route, so #577's rebase has to give link 8 the `Link8` route again (the
probe did this with a local stand-in).

**Stop 3 is a harness gap, not a missing native.** The cold definition is the weapon type's `GestaltMesh`, a
`GearboxFramework.GestaltSkeletalMeshDefinition`. #596's preflight refuses it as unloaded, as its note says it should,
because a lazy load is a write. Nothing in the equip world's set-up preloads it. A probe-only stand-in preloaded it
(logged as a harness input), and `BuildCompositeMesh` then passed on the gestalt path. The fix belongs in #596, or in a
follow-up to #596 and #613: a logged, **UNVERIFIED** harness preload of the weapon type's gestalt definition, the way
#573 preloads the muzzle-flash collections. The hub decides which.

**Stop 4 is the next real blocker, and it is a layer.** After `BuildCompositeMesh`, the script sets the weapon's mesh
to the composite one. #599's `SetSkeletalMesh` refuses a change of mesh as `LayerAbsent` (a skeletal mesh component's
bone transforms), per NATIVE_SET_SKELETAL_MESH.md. No native is missing here, so **this lane writes no new analyst
note.** What comes next is the layer #599's note names (bones, the per-bone animation state, physics and render state
for a mesh change), or a hub ruling on how far an empty Off answer may go for a weapon nobody renders.

**Open, for the hub:**
- stop 3's preload: in #596, or a follow-up;
- stop 4: a mesh-change layer lane, or a ruling;
- the rebase order: #577 and #599 both touch `equip_run.rs`'s weapon preparation, and the probe merged them by hand.
