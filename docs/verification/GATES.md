# Gates and the current baseline

Update this file in the commit that changes a number. Paste real output in reports; do not copy these numbers.

**ANALYST-UNLOCKPLAYERCLASSACHIEVEMENT (2026-11-20, pc-hummer, docs only, on docs/native-unlock-class-achievement-2,
rebased onto main d3c7a13):** AI-assisted (Claude Code, Opus 5.5). NATIVE_UNLOCK_CLASS_ACHIEVEMENT.md, link 9's next
stop after the trace amendment. No code changed, so no measured number moved; the slice gate was not run for this lane.
UNVERIFIED in game.

**LINK9-NEXT-STOP (2026-11-20, pc-hummer, on fix/rust-link9-pawn-natives, stacked on #650):** AI-assisted (Claude
Code, Opus 5.5). Link 9's set-up registers the combat pack's pawn natives (`IsStaggered`, `DropCombatPerch`).
`cargo test --workspace`: 3202 passed / 0 failed / 51 ignored; `crate_edges_test.py` OK. Installed Off
`--slice-gate` strict (release builds, under the machine lock): this branch 4/9/1, link 9 at #616's
`AddAttributeModifier`, not in its base. A local throwaway with #616 gives 4/10/0, link 9 1 of 2 steps, `cast` at
`Actor.Trace`'s missing component bounds (a NotYet). UNVERIFIED in game.

**LINK9-RESOLVER-PRELOAD (2026-11-20, pc-hummer, on fix/rust-link9-resolver-preload from #644 905f3ece):** AI-assisted
(Claude Code, Opus 5.5). Link 9's set-up preloads the phaselock attribute's context resolver, a labelled UNVERIFIED
stand-in. `cargo test --workspace`: 3201 passed / 0 failed / 51 ignored; `crate_edges_test.py` OK. Installed Off
`--slice-gate` strict (release build, under the machine lock): ok 4, notyet 9, error 1 of 14. Link 9 passes 1 of 2
steps; `cast` gets past the resolver and stops with an error at the unbound
`AttributeDefinitionBase.AddAttributeModifier(object,object,bool)` (IMPL-ADD-ATTRIBUTE-MODIFIER, not on main).
UNVERIFIED in game.

**LINK9-NEXT-STOP-2 (2026-11-20, pc-hummer, docs only, on docs/native-actor-trace-hash-admission from main
b9dfca8b):** AI-assisted (Claude Code, Opus 5.5). No code changed, so no measured number moved. On a local throwaway
(main + #650 + #655 + #616's own commits; installed Off `--slice-gate` strict): ok 4, notyet 10, error 0, link 9 1 of 2
steps, `cast` at `Actor.Trace`'s T5 on the controller's non-colliding cylinder. With NATIVE_ACTOR_TRACE.md section 5's
skip stood in locally: ok 4, notyet 9, error 1, `cast` at the unbound
`WillowPlayerController.UnlockPlayerClassAchievementIfConditionsMet`. UNVERIFIED in game.

**IMPL-SKIP-FLAG (2026-11-20, zuhu desktop, on fix/rust-trace-skip-flag from main b9dfca8b):** AI-assisted (Claude
Code, Opus 5.5). Refusal S is lifted in `Actor.GetBoundingCylinder` (live and template) and in the spawn extent's
`Actor` rule, from NATIVE_COMPONENT_SKIP_FLAG.md (#653 at 4fd8587d). `BulletListener` set skips an entry; clear goes
on to the per-component check (passes for the primitive, cylinder and bullet-listener classes, `NotRead` for the
rest), then the box. `cargo test --workspace -j 2`: 3,285 passed / 0 failed / 51 ignored. Clippy flags nothing in the
changed files. Installed Off `--slice-gate` strict, under the machine lock: 4 ok / 10 NotYet / 0 errors. Links 6 and
7 pass `make` (1 of 3 and 1 of 4 driver steps) and stop in `touch` at "Actor.GetBoundingCylinder: the component
check of Engine.SkeletalMeshComponent (not read)", the stop the note predicts; the pickup's cylinder now gives its
box. No real-game run; `UNVERIFIED` in game. Logs under ignored `local/p2/skip/`.

**LINK9-GETAICOMPONENT-BINDING (2026-11-20, pc-hummer, on fix/rust-link9-holds-binding from main 241a6c47):**
AI-assisted (Claude Code, Opus 5.5). Link 9's set-up registers the AI holds' natives (`ow_combat::holds::register`),
`GearboxMind.GetAIComponent` among them. `cargo test --workspace`: 3200 passed / 0 failed / 51 ignored;
`crate_edges_test.py` OK. Installed Off `--slice-gate` strict (release build, under the machine lock): ok 4, notyet
10, error 0 of 14. Link 9 passes 1 of 2 steps; `cast` stops with a NotYet, no longer an error: `ResolveContext`'s
unloaded context resolver `GD_Siren_Skills.Attributes.IsPhaselocked.PawnAttributeContextResolver_0`. UNVERIFIED in
game.

**IMPL-BODYCOMP-SOCKET-BONE-TABLES (2026-11-20, pc-hummer, on fix/rust-bodycomp-socket-bone-tables from main
92c79308):** AI-assisted (Claude Code, Opus 5.5). The body composition's socket attach in Off
(NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md section 4), and the harness's socket preload beside #592's mesh preload.
`cargo test --workspace`: 3192 passed / 0 failed / 51 ignored (13 new in `ow-world`
`world/body_composition_socket_tests.rs`, 4 in `ow-cli` `cmd/mesh_sockets_tests.rs`); `crate_edges_test.py` OK.
Installed Off `--slice-gate` strict (release build, under the machine lock): ok 4, notyet 10, error 0 of 14. The
preload loads all 13 of `Skel_SirenBody`'s sockets.
- With `OPENWILLOW_PSK_EXPORTS` at a local UModel export: link 1 stops in `maya:8` at "the owner's event after the
  tree initialisation" (#612, a note only).
- Without the export: link 1 stops at "no reference skeleton for Char_Siren.Mesh.Skel_SirenBody".

Links 3, 4, 5, 13 and 14 are blocked behind link 1. Both stops come before the socket attaches, so the new rule is
not reached on installed data yet. UNVERIFIED in game.

**FOLLOWUP-602-CLASS-POLICY (2026-11-20, pc-hummer, on main 241a6c47 after review r1 of #640):** AI-assisted (Claude
Code, Opus 5.5). Controller's QueryInterface rule stops at a native class whose answer was not read (a fitted
native-class bit), the pawn accessor is checked apart, and a reserved interface is refused. `cargo test -p ow-combat
read_only` 16 passed; `cargo test --workspace --locked -j 2 --no-fail-fast` 3,205 passed / 0 failed / 51 ignored;
`crate_edges_test.py` OK. Installed Off `--slice-gate` strict, under the machine lock (run on the pre-rebase head
e315d48c and its base 5e1ca57a, not rerun after r1's test-and-doc changes): 4 ok / 9 NotYet / 1 error on both, row for
row. Link 9 runs `spawn` (1 of 2 steps) and `cast` stops at the unbound `GearboxMind.GetAIComponent()`. No real-game
run; `UNVERIFIED` in game. Decision 2026-11-19-251.

**LINK8-NEWMESH-PRELOAD (2026-11-20, pc-hummer, on #645 44992baa):** AI-assisted (Claude Code, Opus 5.5). Link 8's
set-up preloads the gestalt definition's `GestaltSkeletalMesh` (an UNVERIFIED stand-in, Off only). `cargo test
--workspace` 3,211 passed / 0 failed / 51 ignored; `crate_edges_test.py` OK. Installed Off `--slice-gate` strict: 4 ok /
9 NotYet / 1 error; only row 8 differs from #645: link 8 passes `make` and first stops in `init` at SetSkeletalMesh's
LOD row (`LayerAbsent`, the skeletal mesh data layer's LOD models) instead of the unloaded new mesh.

**IMPL-SKELMESH-MESH-CHANGE (2026-11-20, pc-hummer, on main 241a6c47):** AI-assisted (Claude Code, Opus 5.5).
SetSkeletalMesh's mesh change in Off from NATIVE_SKELMESH_MESH_CHANGE.md. `cargo test --workspace -j 2` 3,209 passed / 0
failed / 51 ignored (10 new in `skeletal_mesh_change_tests.rs`); `crate_edges_test.py` OK. Installed Off strict
`--slice-gate`: 4 ok / 9 NotYet / 1 error, as main (link 9: unbound `GearboxMind.GetAIComponent()`); link 8 passes
`make` and stops in `init` at "SetSkeletalMesh: the new mesh is an unloaded handle" (NotRead). No real-game run.

**IMPL-COMPONENT-BOUNDS (2026-11-20, pc-hummer, on main 241a6c47 with #639's note at r1):** AI-assisted (Claude Code,
Opus 5.5). An attach record on the scene layer. `GetBoundingCylinder`'s live path now skips non-colliding entries and
refuses L for unrecorded ones, then S unconditionally. `cargo test --workspace --locked -j 2 --no-fail-fast` 3,204
passed / 0 failed / 51 ignored; `crate_edges_test.py` OK. Installed Off `--slice-gate` strict, under the machine lock:
4 ok / 9 NotYet / 1 error. Links 6 and 7 pass `make` (1 of 3, 1 of 4) and stop in `touch` at refusal S (they stopped
at L before). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-253.

**IMPL-BODYCOMP-RENDER-HOOKS (2026-11-20, zuhu desktop, on #631 at 36334758):** AI-assisted (Claude Code, Opus 5.5).
The body composition's three rendering hooks in Off, from NATIVE_BODYCOMP_RENDER_HOOKS.md section 4 (#622), replacing
ruling 4's refusal of every made component. `cargo build --workspace --locked` clean; `cargo test --workspace --locked -j
4` 3,212 passed / 0 failed / 51 ignored (the note's tests 1 to 12 in `body_composition_tests.rs`; five old tests now
stop at hook 3's array check; `ow-inventory`'s setter tests unchanged); `python tests/crate_edges_test.py` OK (19);
`cargo clippy -p ow-world -p ow-inventory --all-targets` adds no warning in the touched files. Installed Off
`--slice-gate` (strict, under the lock) against #631's own run: ok 4 / NotYet 10 / error 0 of 14 on both. Links 1, 3, 4,
5, 13 and 14 now stop at hook 1, "the gear-likeness interface not resolved at set-up" (`NotRead`: the harness never
defines `WillowGame.IGearLikenessConsumer`); link 1 runs 0 of its 1 step, the other five 0 of 0. The other eight links
are unchanged. Two local probes (never pushed) under the lock: with the interface loaded at set-up, the stop is hook 1's
unresolved interface entry on `Engine.SkeletalMeshComponent`'s chain; with that entry also resolved at set-up (answer:
no), it is hook 2's pawn test (`NotRead`), since Maya's `Mesh` template has `bOwnerNoSee` clear. Both inputs would be
harness changes, not made here. ctest and verify_packages not run (no C++ change). No real-game run; every rule
UNVERIFIED.

**IMPL-TREE-INIT-OWNER-EVENT (2026-11-20, zuhu desktop, on main 92c79308, with reviews r1 and r2 of #631):** AI-assisted
(Claude Code, Opus 5.5). The owner's `PostInitAnimTree` after the tree initialisation, and
`SkeletalMeshComponent.FindAnimNode` / `FindSkelControl` in Off (bound for Engine by their package-qualified identity),
from NATIVE_TREE_INIT_OWNER_EVENT.md (#612 at 3691317c), replacing #561's owner-event refusal. `cargo build --workspace
--locked` clean; `cargo test --workspace --locked -j 4` 3,199 passed / 0 failed / 51 ignored (26 new in
`anim_tree_copy`, 3 of them for review r2; the 2 owner-event refusal tests removed); `cargo clippy -p ow-world -p ow-vm
--all-targets` adds no warning in the touched files. Installed Off `--slice-gate` (strict, under the lock) on this
branch at its r2 code commit, against main 92c79308: ok 4 / NotYet 10 / error 0 of 14 on both. Links 1, 3, 4, 5, 13
and 14 now stop at the body's rendering hooks ("InitializeBodyComposition: the body's rendering hooks"); before, they
stopped at the owner's event. The other eight links' stops are unchanged. Maya's `.psk` was exported locally with
UModel 1590 for the run, as #592 records. ctest and verify_packages not run (no C++ change). No real-game run; every
rule UNVERIFIED.

**IMPL-SETSCALE (2026-11-20, zuhu desktop, on fix/rust-primitive-setscale from main 92c79308):** AI-assisted (Claude
Code, Opus 5.5). `PrimitiveComponent.SetScale` in Off from NATIVE_PRIMITIVE_SETSCALE.md. The same scale does nothing.
A change writes `Scale` and `bNeedsUpdateTransform`, or refuses `NotRead` before any write for the two unread
transform-update virtuals and for an unloaded owner. `cargo test --workspace -j 2`: 3,180 passed / 0 failed / 51
ignored (5 new in `ow-world`). Clippy flags nothing in the changed files. Installed Off `--slice-gate` strict, under
the machine lock: 4 ok / 10 NotYet / 0 errors. Link 8 passes 1 of 5 driver steps (`make`) and stops in `init` at
"stopped after partial writes: BuildCompositeMesh: a definition it reads is not loaded", which comes before
`SetScale`, so main's gate does not reach the new binding yet. No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-256-primitive-setscale-in-off; logs under ignored `local/p2/setscale/`.

**IMPL-BONE-TRANSFORM-LAYER (2026-11-20, pc-hummer, on main dba540a3):** AI-assisted (Claude Code, Opus 5.5). The
bind pose, the composed reference pose, `stored_space_bases`, `pose_update` and `ForceSkelUpdate` / `UpdateAnimations`
in Off, from NATIVE_BONE_TRANSFORM_LAYER.md at 94ab4339. `cargo test --workspace -j 2` 3,226 passed / 0 failed / 51
ignored (8 new in `bone_transform_tests.rs`); `crate_edges_test.py` OK. Installed Off strict `--slice-gate`: 4 ok / 9
NotYet / 1 error, every row as main's (no link reaches the layer). No real-game run.

**ANALYST-BONE-TRANSFORM-LAYER (2026-11-20, pc-hummer, on main 241a6c47):** AI-assisted (Claude Code, Opus 5.5).
Analyst note NATIVE_BONE_TRANSFORM_LAYER.md, no code. Installed Off `--slice-gate` strict on main: 4 ok / 9 NotYet / 1
error; with local stand-ins (never pushed) for #612, #622, the socket attach and the Parent link, still 4 / 9 / 1, the
six Maya links next at AnimNodeSpecialMoveBlend, and the parent's `bAttached` clear at all eight socket attaches: no
slice link reaches the layer today.

**ANALYST-SPECIALMOVE-OWNER (2026-11-20, pc-hummer, docs only, on docs/native-specialmove-owner from main 241a6c47):**
AI-assisted (Claude Code, Opus 5.5). NATIVE_SPECIALMOVE_OWNER.md: what `AnimNodeSpecialMoveBlend` asks its owner
during and after the tree initialisation. No code changed, so no measured number moved; the slice gate was not run
for this lane. UNVERIFIED in game.

**ANALYST-SKELMESH-MERGE (2026-11-20, zuhu desktop, on docs/native-skelmesh-merge from main dba540a):** AI-assisted
(Claude Code, Opus 5.5). Docs only: NATIVE_SKELMESH_MERGE.md, an index row and a decision. No code changed, so no
build or test run. Installed Off `--slice-gate` strict on main dba540a, under the machine lock, for section 0: 4 ok /
9 NotYet / 1 error. Link 8 passes 1 of 5 driver steps (`make`) and stops in `init` at "SetSkeletalMesh: a mesh
change", after `BuildCompositeMesh` answered the gestalt mesh. The slice gun's type has a `GestaltMesh`, so link 8
does not reach the merge. No real-game run; `UNVERIFIED` in game. Logs under ignored `local/p2/merge/`.

**ANALYST-POSTINITBODYCOMP-SLOTNODE (2026-11-20, pc-hummer, on main dba540a3):** AI-assisted (Claude Code, Opus
5.5). Analyst note NATIVE_POSTINIT_BODYCOMP_SLOTNODE.md, no code; slice gate not run (docs only). The six Maya links'
stop after the hooks is the recorded AnimNodeSpecialMoveBlend owner test (NATIVE_BONE_TRANSFORM_LAYER.md section 5,
#646); after it, attachment 13's attached CylinderComponent release is the expected stop (UNVERIFIED, not measured).

**PREFLIGHT-ORDER-LINK9-CAST (2026-11-20, zuhu desktop, on fix/rust-link9-cast-preflight-order from main 241a6c4):**
AI-assisted (Claude Code, Opus 5.5). `FlagExpressionEvaluator.Evaluate` asks the AI component resolver's
`QueryInterface(IAIInterface)` through #602's read-only dispatch before its next load, so a question with no bound
answer refuses before the evaluator's first write, not after it. `cargo test -p ow-skills --lib flag_expression` 22
passed / 0 failed; `cargo build --workspace -j 2` OK; `cargo test --workspace -j 2` 3,201 passed / 0 failed / 51
ignored; `crate_edges_test.py` 19 OK; clippy: no warning in the touched files. Installed Off `--slice-gate` strict
(release build, under the machine lock): 4 ok / 9 NotYet / 1 error. Link 9 errors in `cast` on the unbound
`GearboxMind.GetAIComponent()`, which is batch 9's own error (lane d2b6). With #602 the mind answers `IAIInterface`
read-only, so the preflight passes and the cast reaches that call. The old "after partial writes" stop is not reached
on this base. ctest and verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-266; logs under ignored `local/p2/link9-preflight/`. **Review r1 of #648:** the preflight now asks only for a resident
definition whose first set resolver is the AI component resolver, and passes on only a rule's own `NotYet` (not one
with the read-only dispatch's own record). `cargo test -p ow-skills --lib flag_expression` 23 passed / 0
failed; `cargo test -p ow-vm -p ow-skills -j 2` 371 passed / 0 failed / 2 ignored; clippy: no warning in the touched files. Off `--slice-gate`
strict: 4 ok / 9 NotYet / 1 error, each of the 14 links the same result and first stop as main 241a6c4 built the same
way. **Review r2 of #648:** the dispatch's own refusals get `ow_vm::api::DISPATCH_RECORD` (`ow-vm`
`interface_read_only.rs`, and two read-only-only refusals in `ow-combat` `pawn_interface/read_only.rs`), since the
rules share `READ_ONLY_RECORD`; the filter now keeps the rules' refusals. `cargo test -p ow-vm -p ow-combat -p
ow-skills -j 2 --no-fail-fast` 642 passed / 0 failed / 4 ignored; clippy: no warning in the touched files. Off `--slice-gate` strict, release build under the lock, on main b9dfca8b
with this branch merged in: 4 ok / 10 NotYet / 0 error, each of the 14 links the same result and first stop as
b9dfca8b alone built the same way (link 9: `cast`, the unloaded `PawnAttributeContextResolver_0` handle).

**BIND-ATTACHCOMPONENT-SKILL-WORLDS (2026-11-20, pc-hummer, on fix/rust-attach-component-skill-worlds from main
b0d9ca08):** AI-assisted (Claude Code, Opus 5.5). `Actor.AttachComponent(object)` is bound in the action-skill pack with
the pickup worlds' existing function (keep-existing, Off only). `cargo test --workspace`: 2878 passed / 0 failed / 50
ignored (5 new in `ow-skills` `action_skill/attach_component_tests.rs`); `crate_edges_test.py` OK. Installed Off
`--slice-gate` strict (release build, under the machine lock): ok 3, notyet 9, error 2 of 14. Link 9 passes 1 of 2
steps and `cast` stops before the attach, at `FlagExpressionEvaluator.Evaluate`'s not-loaded interface class
`GearboxFramework.IRuleEngineOwner` (NATIVE_FLAG_EXPRESSION_EVALUATE.md). The attach is reached only under the probe
stand-ins of NATIVE_ACTOR_TRACE.md section 4, and `phaselock_world` installs no scene layer, so there it would refuse
layer-absent. UNVERIFIED in game.

**HARNESS-LINK9-SETUP (2026-11-20, pc-hummer, on fix/rust-link9-harness-setup from main 4c4522d7):** AI-assisted
(Claude Code, Opus 5.5). Link 9's set-up resolves `GearboxFramework.IRuleEngineOwner` and installs the pickup worlds'
empty scene layer, both labelled UNVERIFIED harness inputs. `cargo test --workspace`: 2978 passed / 0 failed / 51
ignored; `crate_edges_test.py` OK. Installed Off `--slice-gate` strict (release builds, under the machine lock): main
and branch both ok 4, notyet 9, error 1 of 14. Only link 9 differs: still 1 of 2 steps, with `cast` now stopping at
GearboxMind's unread QueryInterface answer for `IAIInterface` (marked after partial writes), past the
IRuleEngineOwner load.

**FOLLOWUP-FLAG-PROVIDER-ORDER (2026-11-20, pc-hummer, on main at 4c4522d7):** AI-assisted (Claude Code, Opus
5.5). The flag evaluator's IFlagProvider guard before any resolver side effect, a non-loading rule engine search
(`Vm::implements_read_only`) and the store path before the guard. `cargo test --workspace --locked -j 2` 2,982 passed
/ 0 failed / 51 ignored (4 new); `cargo clippy` adds no warning in the touched files; `crate_edges_test.py` OK.
Installed Off `--slice-gate` (strict, under the lock) on the branch and its base: ok 4 / NotYet 9 / error 1 of 14 on
both, every link the same (link 9: 2 of 2 steps, stopped at the IRuleEngineOwner class not loaded). ctest and
verify_packages not run (no C++ change). No real-game run; every rule UNVERIFIED.

**HARNESS-LINK8-GESTALT-PRELOAD (2026-11-20, pc-hummer, on main 4c4522d7):** AI-assisted (Claude Code, Opus 5.5).
The equip world's logged, UNVERIFIED preload of the weapon type's `GestaltMesh` (hub ruling on stop 3 of
ANALYST-LINK8-NEXT). `cargo test --workspace -j 2` 2,981 passed / 0 failed / 51 ignored (3 new in
`inventory/equip_run_gestalt_tests.rs`); `crate_edges_test.py` OK. Installed Off strict `--slice-gate` (release, under
the machine lock): on this branch 4 ok / 9 NotYet / 1 error, link 8 at `GetExpLevelFromManufacturerData` as on main; on
a probe merge with #596, #613, #577 and #599, 4 ok / 10 NotYet / 0 errors, link 8 passes `make` and stops in `init` at
#599's `SetSkeletalMesh` mesh-change LayerAbsent. ctest and verify_packages not run (no C++ change). No real-game run. Rebased onto main df0970ec (batch 7): `cargo test --workspace -j 2` 3,134 passed / 0 failed / 51 ignored; the branch alone is now 4 ok / 10 NotYet / 0 errors, link 8 at the same mesh-change stop.

**FOLLOWUP-GESTALT-OPERAND-ORDER (2026-11-20, pc-hummer, on main df0970ec):** AI-assisted (Claude Code, Opus 5.5).
InitGestaltMeshDataFromOther's one operand order (None, unloaded, class and field). `cargo test --workspace -j 2`
3,131 passed / 0 failed / 51 ignored; `crate_edges_test.py` OK. Installed Off strict `--slice-gate`: 4 ok / 10 NotYet /
0 errors, the same as main; link 8 passes `make` and stops at `BuildCompositeMesh`'s cold gestalt definition (#632).
ctest and verify_packages not run (no C++ change). No real-game run.

**ANALYST-SKELMESH-MESH-CHANGE (2026-11-20, pc-hummer, on main df0970ec):** AI-assisted (Claude Code, Opus 5.5).
Analyst note only (NATIVE_SKELMESH_MESH_CHANGE.md); no code changed, so no test numbers move. Probe (local throwaway
merge of main 4c4522d7 with #577, #599, #596 and #613, deleted after), installed Off `--slice-gate` strict under the
machine lock: 4 ok / 10 NotYet / 0 errors without stand-ins (link 8 at `BuildCompositeMesh`'s unloaded definition);
4 ok / 9 NotYet / 1 error with a gestalt preload and SetSkeletalMesh's mesh-change refusal stubbed (link 8 runs
`make`, passes both mesh changes in `init` and stops at the unbound `PrimitiveComponent.SetScale(float)`). No real-game run;
`UNVERIFIED` in game. Decision 2026-11-19-242; raw output under ignored `local/analysis/smmc/`.

**FOLLOWUP-PICKUP-RB-VALIDATION r1 (2026-11-20, zuhu desktop, on main b0d9ca08, #625):** AI-assisted (Claude Code,
Opus 5.5). Under the hub's ruling, `InitializePickupForRBPhysics` checks only the chosen path's declarations again
(#587's branch-scoped checks and tests restored). It also checks G's four impact fields, and on the spin branch its
range fields, before that path's first write. The globals' reference checks compare the declared class through the
class table's read-only `by_path`, by id, with no load.
- `cargo test --workspace -j 2`: 2,876 passed / 0 failed / 50 ignored. With r2's fixes (exact float checks for
  `MaxRBAwakeTime` and `PhysicsWeight`, `Scene` only on the ownerless reattach): 2,878 / 0 / 50, and the slice gate
  below is unchanged.
- Clippy on `ow-inventory`: no new warning.
- Installed Off `--slice-gate` strict (release build, under the machine lock): 3 ok / 9 NotYet / 2 errors. Links 6 and
  7 pass the native (`make`) and stop at "stopped after partial writes: ParticleSystemComponent.ActivateSystem", as
  before.
- Not run: ctest and verify_packages (no C++ change).

No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-243 (placeholder).

**IMPL-ACTOR-TRACE (2026-11-20, zuhu desktop, on fix/rust-actor-trace from main 4c4522d):** AI-assisted (Claude Code,
Opus 5.5). `Actor.Trace` in Off from NATIVE_ACTOR_TRACE.md (#621 at 860e5492): one shared `ow-world` function, bound
by the action skill pack and by go_down in place of its bare `LayerAbsent` closure. `cargo test -p ow-world --lib
actor_trace` 10 passed / 0 failed; `cargo test -p ow-skills --lib` 133 passed / 0 failed / 1 ignored (the binding-site
test included); `cargo build --workspace -j 2` OK; `cargo test --workspace -j 2` 2,989 passed / 0 failed / 51
ignored; `crate_edges_test.py` 19 OK; clippy: no warning in the new files. Installed Off `--slice-gate` strict (release
build, under the machine lock): 4 ok / 9 NotYet / 1 error. Link 9 passes `spawn` and stops in `cast` at the unloaded
`IRuleEngineOwner` (1 of 2 steps), before the trace. A throwaway probe with #628 and #623 gives the same counts, with
link 9 stopping at #602's GearboxMind `QueryInterface` question, still before the trace. ctest and verify_packages
not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-245; logs under ignored
`local/p2/impl-actor-trace/`. **Review r1 of #633 (rebased on main df0970e as 441827b, then fixed on top):** both
outs and their alias writes are checked before either is written (`NativeCall::check_out`), non-finite or negative
query values and unsound primitive bounds refuse as `NotRead` (UNVERIFIED policy), and every vector must be
`Core.Object.Vector` by type. `cargo test -p ow-world --lib actor_trace` 13 passed / 0 failed; `cargo build
--workspace -j 2` OK; `cargo test --workspace -j 2` 3,144 passed / 0 failed / 51 ignored; `crate_edges_test.py` 19 OK;
clippy (ow-object, ow-vm, ow-world): no new warning. Slice gate not rerun for the fix round.

**ANALYST-COMPONENT-BOUNDS (2026-11-20, pc-hummer, on main df0970ec):** AI-assisted (Claude Code, Opus 5.5). Docs
only: NATIVE_COMPONENT_BOUNDS.md. Installed Off strict `--slice-gate` (release, under the machine lock): 4 ok / 10
NotYet / 0 errors; links 6 and 7 pass `make` and stop in `touch` at `GetBoundingCylinder`'s refusal L. A throwaway
probe with a stand-in past L stopped both at `DroppedPickup.TouchPickupTrace`. No real-game run.

**IMPL-TOUCH-PICKUP-TRACE (2026-11-20, zuhu desktop, on main 4c4522d):** AI-assisted (Claude Code, Opus 5.5).
`DroppedPickup.TouchPickupTrace` in Off, from NATIVE_TOUCH_PICKUP_TRACE.md (#624, 6dbdea1e). The pickup run's world
gets link 9's `NoLevel` collision service (UNVERIFIED stand-in).
- `cargo test --workspace -j 2`: 2,993 passed / 0 failed / 51 ignored. The 15 note cases are in `pickup_touch_tests.rs`.
  With r1 (the Base chain before the owner's flags, the declared Vector id, a kept None, the harness line): 2,995 / 0 /
  51; the probe readings below are unchanged, and link 6's `harness` names the NoLevel stand-in.
- Clippy on `ow-world`, `ow-inventory` and `ow-cli`: no warning on a changed line.
- Installed Off `--slice-gate` strict, on a local probe that is never pushed (under the machine lock):
  - **main df0970e + this branch:** 4 ok / 10 NotYet / 0 errors. Links 6 and 7 pass `make` and stop in `touch` at
    `Actor.GetBoundingCylinder` refusal L ("the colliding components' bounds"), before this native.
  - **The same plus a local stand-in for that refusal** (an invented cylinder): 4 ok / 9 NotYet / 1 error.
    - Link 6 passes `touch` and `key`, then fails the driver's own evidence check: the weapon is not the pawn's
      afterwards.
    - Link 7 stops in `touch` in this native, at "the trace filter of WillowGame.WillowPickup": the other pickup's
      `CollisionCylinder`, a class with no read row, which is the note's refusal.
- Not run: ctest and verify_packages (no C++ change).

No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-250 (placeholder).

**IMPL-NOUN-REPLICATION (2026-11-19, pc-hummer, on fix/rust-link9-attr-get-value b0cd5f4f, #515):** AI-assisted
(Claude Code, Opus 5.5). The noun providers' arrays and replicated copy in Off, from section 10 of
NATIVE_NOUN_ATTRIBUTES.md (#516 r1, 94663dbe). `cargo test -p ow-attr -p ow-inventory -p ow-combat -p ow-skills -p
ow-world -p ow-cli --locked -j 4` 1,092 passed / 0 failed / 10 ignored; `cargo test --workspace --locked -j 4` 2,475
passed / 0 failed / 46 ignored; `crate_edges_test.py` OK. Installed Off `--slice-gate` strict (release build, under
the machine lock): 1 ok / 3 NotYet / 10 errors. Link 9 passes `spawn` and stops in `cast` at the unbound
`FlagExpressionEvaluator.Evaluate(object)` (1 of 2 steps). ctest and verify_packages not run (no C++ change). No
real-game run; `UNVERIFIED` in game. Decision 2026-11-19-212; logs under ignored `local/analysis/nounrep-impl/`.
Rebased onto #518's counters (squash cf9cfd31 on main 3ad58b0, desktop): `cargo test -p ow-attr -p ow-inventory` 537
passed / 0 failed / 1 ignored; `cargo test --workspace -j 2` 2,682 passed / 0 failed / 48 ignored; clippy clean on both
crates; installed Off `--slice-gate` strict 2 ok / 9 NotYet / 3 errors, link 9 still 1 of 2 steps, `cast` stopping at
`FlagExpressionEvaluator.Evaluate(object)`.

**PARTICLE-ACTIVATION-FOLLOWUP (2026-11-20, pc-hummer, on #609 @ 1d33bd29):** AI-assisted (Claude Code, Opus 5.5).
This lane fixes #609's four review lows:
- the old template is read only inside the stop-audio gate;
- the forced lowest LOD and the LOD bias are ported;
- a looping sound counts as recorded by `AkPlayingId`;
- the `InstanceParameters` value is built in the plan.

`cargo test --workspace -j 2`: 2,504 passed / 0 failed / 46 ignored (4 new). The installed Off `--slice-gate` strict:
0 ok / 5 NotYet / 9 errors. Links 6 and 7 pass `make` and stop at `touch` (`Actor.GetBoundingCylinder`), unchanged
from #609. No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-229 (particle activation follow-up).

**IMPL-PARTICLE-ACTIVATION (2026-11-20, zuhu desktop, on fix/rust-spawn-pickup-particles 9777c2cc, #593):**
AI-assisted (Claude Code, Opus 5.5). `SetTemplate`'s activation and initialisation in Off from
NATIVE_PARTICLE_ACTIVATION.md (#597, 3df9cd88), with the hub's rulings: `SpawnPickupParticles` plans every write, then
applies it; #581's refusals E and F are gone; engine flags are a logged harness input (UNVERIFIED in game). `cargo build
--workspace --locked -j 4` clean; `cargo test --workspace --locked -j 4`: 2,500 passed / 0 failed / 46 ignored
(`ow-inventory` 239; `attach_mesh/pickup_particles_tests.rs` 20, 10 net new). Installed `--slice-gate` (Off, strict,
release build, under the machine lock): ok 0, notyet 5, error 9 of 14. Links 6 and 7 pass `make` and stop at `touch`
on the unbound `Actor.GetBoundingCylinder(float,float)` (1 of 3 and 1 of 4 driver steps), from #593's refusal E (so
notyet 7 / error 7 becomes 5 / 9); the other 12 rows read as on #593. ctest and verify_packages not run (no C++ change). No
real-game run. Logs under ignored `local/p2/ppa/gate/`. This is what the host did, not parity.

**IMPL-SPAWN-PICKUP-PARTICLES (2026-11-20, zuhu desktop, on fix/rust-pickup-rb 7ae53972, #587):** AI-assisted
(Claude Code, Opus 5.5). `SpawnPickupParticles` in Off from NATIVE_SPAWN_PICKUP_PARTICLES.md: the ordered preflight to
refusal E or F, which always applies today, so the native writes nothing; a harness preload of `PartPickup` and the
particle component class (UNVERIFIED in game). `cargo test --workspace -j 2`: 2,490 passed, 0 failed (48 result lines; 2,489 before review r1 of #593's count fix). `ow-inventory` 228 passed (10 new in
`attach_mesh/pickup_particles_tests.rs`). Installed `--slice-gate` (Off, strict): ok 0, notyet 7, error 7 of 14.
Links 6 and 7 move from "WillowPickup.SpawnPickupParticles: the pickup's particle effect" (`LayerAbsent`) to refusal
E, "stopped after partial writes: ParticleSystemComponent.ActivateSystem: the system's activation ..." in `make`
(0 of 3 and 0 of 4 driver steps; the prefix is the driver's, for `make`'s earlier writes). The other 12 rows read as
before. This is what the host did, not parity.

**IMPL-PICKUP-RB (2026-11-20, zuhu desktop, on fix/rust-attach-mesh-to-pickup-2 4d0539b2, #520):** AI-assisted
(Claude Code, Opus 5.5). `InitializePickupForRBPhysics` in Off from NATIVE_PICKUP_RB_MINIMAL.md, with the asset
branch at rest and a seeded stand-in stream (hub rulings; UNVERIFIED in game). `cargo test --workspace -j 2`: 2,476
passed, 0 failed (48 result lines); `ow-inventory` 215 passed (10 new in `attach_mesh/pickup_rb_tests.rs`);
`ow-core` rand 14 passed (1 new). Clippy: no warning in the touched files. `tests/crate_edges_test.py`: 19 OK.
Installed `--slice-gate` (Off, strict): ok 0, notyet 7, error 7 of 14. Links 6 and 7 move from "the pickup's
rigid-body set-up" (`LayerAbsent`) to "stopped after partial writes: WillowPickup.SpawnPickupParticles: the pickup's
particle effect" in `make`, as #574 foresaw. The other 12 rows read as before. This is what the host did, not parity.

**IMPL-GESTALT-DATA-LAYER (2026-11-20, zuhu desktop, on #596's fix/rust-gestalt-mesh-layer 491a1ba over main
c687c13):** AI-assisted (Claude Code, Opus 5.5). The gestalt component data layer in Off from
NATIVE_GESTALT_DATA_LAYER.md (#603, 9e5113c0): `BuildCompositeMesh`'s fill and `InitGestaltMeshDataFromOther`'s copy,
each one whole-slot write after a write-free preflight (F1, F2). `cargo build --workspace -j 2` clean (0 warnings);
`cargo test --workspace -j 2` 2,761 passed / 0 failed / 49 ignored (10 new in `ow-inventory`); clippy on
`ow-inventory` clean in the gestalt files. Review r1 of #613 (4 tests on the declaration checks; the F2 prefix named):
`cargo test --workspace -j 2` 2,765 passed / 0 failed / 49 ignored. Installed Off `--slice-gate` strict (release build, under the machine lock), base 491a1ba and branch each built:
3 ok / 10 NotYet / 1 error both times, every row byte-identical. Link 8 stops in `init` (1 of 5 driver steps,
`make`) at the unbound `WillowWeapon.RecomputeAttributeBaseValues()`, before either native, so the gate does not
reach the layer. ctest and verify_packages not run (no C++ change). No real-game run;
`UNVERIFIED` in game. Decision 2026-11-19-224 (placeholder).

**IMPL-GESTALT-MESH-LAYER (2026-11-20, zuhu desktop, on main c687c13):** AI-assisted (Claude Code, Opus 5.5).
`WillowWeapon.BuildCompositeMesh` and `SkeletalMeshComponent.InitGestaltMeshDataFromOther` in Off, from
NATIVE_BUILD_COMPOSITE_MESH.md (ce0ec917) and NATIVE_INIT_GESTALT_MESH_DATA.md (67170f22). They run the notes'
preflights, refuse the gestalt data layer as `LayerAbsent`, give the non-gestalt answers, and answer a self-copy as
success; the layer itself is not built. `cargo build --workspace --locked -j 4` clean (0 warnings); `cargo test
--workspace --locked -j 4` 2,574 passed / 0 failed / 48 ignored (10 new); after review r1 of #596, 2,577 passed / 0 failed / 48 ignored (13 new); final pass rebased on c687c13, 2,751 passed / 0 failed / 49 ignored. Installed Off `--slice-gate` strict (release
build, under the machine lock): 2 ok / 4 NotYet / 8 errors. Link 8 is `notyet` in `make` at `WillowWeapon.CacheWeaponCard`,
before either native, so the gate does not reach them (the base was not rerun). ctest and verify_packages not run (no
C++ change). No real-game run; `UNVERIFIED` in game. Logs under ignored `local/p2/gestalt/`.

**IMPL-GEARBOXMIND-GETAICOMPONENT (2026-11-20, pc-hummer, on main 3ad58b08):** AI-assisted (Claude Code, Opus 5.5).
`GearboxMind.GetAIComponent` in Off from NATIVE_GEARBOXMIND_GETAICOMPONENT.md as final at 660d0b32 (`ow-combat`
`holds::get_ai_component`): the mind's own `AIComponent` by a plain heap read, an unloaded mind `NotYet { NotRead }`, a
template component (outer `Origin::Default` or `Default__...`) `NotYet { NotRead }`, both also in the Off hold preflight;
Reference unchanged. `cargo build --workspace` clean; `cargo test --workspace` 2,670 passed / 0 failed / 48 ignored (6 new
hold tests, 5 in `ow-combat` and 1 in `ow-ai`, and 1 renamed; #518's counters checked); `crate_edges_test.py` OK. Installed Off
`--slice-gate` strict: 2 ok / 10 NotYet / 2 errors, all 14 rows byte-identical with and without the binding. Link 9
passes 1 of its driver's 2 steps (`spawn`) and stops at `cast` on main's noun `ReplicationStrategy` 2 refusal ("stopped
after partial writes: NounAttributeValueResolver with ReplicationStrategy 2 on a provider..."), before the flag
evaluator reaches this native. A probe stacked on #523 was not run: #523 conflicts with main in 10 files (its counter
names against #518's), which is #523's rebase. No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-210; logs
under ignored `local/p2/gac/`.

**IMPL-SPAWN-COMPONENT-ATTACH (2026-11-20, pc-hummer, on main c687c135):** AI-assisted (Claude Code, Opus 5.5).
The spawn's component update in Off from NATIVE_SPAWN_COMPONENT_ATTACH.md as final at bc83e3b6 (`ow-world`
`actors::component_attach`): the preflight before the actor is made, the writes between `Rotation` and `set_owner`; the
empty `SceneLayer` moved to `ow-world` and installed by `World::new` in Off (a logged harness input). `cargo build
--workspace` clean; `cargo test --workspace` 2,748 passed / 0 failed / 49 ignored (7 `component_attach` unit tests, 2
spawn-level tests, 1 holds test; 124 existing tests fixed in six fixture builders, no assertion changed);
`crate_edges_test.py` OK. Installed Off `--slice-gate` strict: 3 ok / 10 NotYet / 1 error, all 14 rows byte-identical
to main's. Link 9 passes 1 of its driver's 2 steps (`spawn`) and stops at `cast` on main's noun `ReplicationStrategy`
2 refusal, before the mind's AI component is asked. No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-221;
logs under ignored `local/p2/sca/`.

**PLACEMENT-WILLOWPICKUP-EXTENT (2026-11-20, zuhu desktop, on main c687c135):** AI-assisted (Claude Code, Opus 5.5).
The spawn extent's `Actor` rule now also covers `WillowGame.WillowPickup` (by exact class), as #595's
`GetBoundingCylinder` does (UNVERIFIED in game). `cargo test --workspace -j 2`: 2,741 passed, 0 failed (48 result
lines); `ow-world` gains 3 tests (`placement_pickup_tests.rs`). Clippy: no warning on the changed lines.
`tests/crate_edges_test.py`: OK. Installed `--slice-gate` (Off, strict): ok 3, notyet 10, error 1 of 14. Links 6 and 7
stop in `make` at "stopped after partial writes: WillowPickup.InitializePickupForRBPhysics: the pickup's rigid-body
set-up" (0 of 3 and 0 of 4 driver steps; #587 not on main yet). They spawn with `bNoCollisionFail` and never ask for
the extent. This is what the host did, not parity.

**IMPL-SET-SKELETAL-MESH r1 (2026-11-20, zuhu desktop, review r1 of #599, on main 3ad58b0):** AI-assisted (Claude Code,
Opus 5.5). The tests check #518's three counters around every call; the harness logs its input before the
instancing and follows the spawn's order; the receiver check comes before the arguments. `cargo build --workspace -j
2` clean; `cargo test --workspace -j 2` 2,668 passed / 0 failed / 48 ignored (1 new test in `ow-cli`); clippy flags
nothing in the changed files. Installed Off `--slice-gate` strict on this branch alone, under the machine lock: 2 ok / 10
NotYet / 2 errors; link 8 is 1 of 5 driver steps, at "stopped after partial writes: WillowWeapon.CacheWeaponCard: the
card's slot count"; links 6 and 7 stay at 0 of 1 step (`make`) at main's `InitializePickupForRBPhysics` stop. No
real-game run; `UNVERIFIED` in game.

**IMPL-SET-SKELETAL-MESH (2026-11-20, zuhu desktop, from main 97a3e39):** AI-assisted (Claude Code, Opus 5.5).
`SkeletalMeshComponent.SetSkeletalMesh` in Off from NATIVE_SET_SKELETAL_MESH.md (#583 at 231bbaed): the same mesh does
nothing, a change is `LayerAbsent` before any write; plus the equip harness's own weapon components (an UNVERIFIED
harness input). `cargo build --workspace -j 2` clean; `cargo test --workspace -j 2` 2,447 passed / 0 failed / 46
ignored (3 new in `ow-world`); clippy flags nothing in the changed files. Installed Off `--slice-gate` strict, under
the machine lock, on a throwaway merge with #572 (3d948c43), #534 (8fc0420b) and #573 (144a73f5; conflicts resolved
by hand) — #552 conflicts with #534 in `card.rs` and was left out: 1 ok / 5 NotYet / 8 errors. Link 8 is 1 of 5 driver
steps (`make` passes, 6 components instanced), stopping in `init` at "stopped after partial writes: value resolver
class WillowGame.DamageTypeAttributeValueResolver". ctest and verify_packages not run (no C++ change). No real-game
run; `UNVERIFIED` in game. Decision 2026-11-19-218; logs under the throwaway worktree's ignored `local/probe/`.
Rebased onto main c687c13 (98f22f6, desktop): `cargo test -p ow-world -p ow-cli` 461 passed / 0 failed / 9 ignored;
`cargo test --workspace -j 2` 2,742 passed / 0 failed / 49 ignored; clippy flags nothing in the changed files.

**IMPL-WILLOWAICOMPONENT-QUERYINTERFACE (2026-11-20, zuhu desktop, from main c687c13):** AI-assisted (Claude Code,
Opus 5.5).
- **The change** (from NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md, #600 at e7dcdde): `AIComponent` and
  `WillowAIComponent` take the component `QueryInterface` rule. Owner None with `bAttached` false is NotRead. Also
  `WillowAIComponent.PhaseLockOnHold` in Off.
- **Gates:**
  - `cargo test -p ow-combat -j 2`: 220 passed / 0 failed / 2 ignored, 7 of them new;
  - `cargo build --workspace -j 2`: clean;
  - `cargo test --workspace -j 2`: 2,745 passed / 0 failed / 49 ignored;
  - clippy flags nothing in the changed files.
- **Slice probe** (installed Off `--slice-gate`, strict, under the machine lock, on a throwaway merge with #601
  e1c9f0e4 and #602 3e7c1294; one test patched there for #602): 3 ok / 10 NotYet / 1 error. Link 9 runs both of its
  driver's 2 steps: `spawn` passes, and `cast` stops at "stopped after partial writes: NounAttributeValueResolver with
  ReplicationStrategy 2 on a provider". The same merge without this branch gives byte-identical output.
- **Not run:** ctest and verify_packages (no C++ change), and no real-game run. Every rule is `UNVERIFIED` in game.

**IMPL-ADD-ATTRIBUTE-MODIFIER (2026-11-19, pc-hummer, on fix/rust-resolve-context over main c687c135):** AI-assisted
(Claude Code, Opus 5.5). `AttributeDefinitionBase.AddAttributeModifier` as one shared Off rule: A1, A0, A2, A3, the
load-free storage probes (`eval::load_free`), then a terminal write. It is bound by the action-skill and equip packs in
either order. Results:
- `cargo test --workspace -j 2`: 2,762 passed / 0 failed / 50 ignored (12 new; the replicated test is ignored, needs
  #523); `crate_edges_test.py` OK;
- the installed equip run (50 weapons, Off): identical to main;
- the installed Off `--slice-gate` strict, under the machine lock: 3 ok / 10 NotYet / 1 error, identical to main. Link 8
  is at the unbound `RecomputeAttributeBaseValues`, and link 9 at the noun strategy-2 refusal (#523).

ctest and verify_packages were not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-255 (add attribute modifier).

**IMPL-RESOLVE-CONTEXT (2026-11-19, pc-hummer, on main c687c135):** AI-assisted (Claude Code, Opus 5.5).
`AttributeDefinitionBase.ResolveContext` as one shared Off rule (R1, R2, R3, then a terminal walk), bound by the
action-skill and equip packs in either order. Results:
- `cargo test --workspace -j 2`: 2,750 passed / 0 failed / 49 ignored (12 new); `crate_edges_test.py` OK;
- the installed equip run (50 weapons, Off): identical to main;
- the installed Off `--slice-gate` strict, under the machine lock: 3 ok / 10 NotYet / 1 error, identical to main. Link 8
  is at the unbound `RecomputeAttributeBaseValues`, and link 9 at the noun strategy-2 refusal (#523).

ctest and verify_packages were not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-226 (resolve context).

**IMPL-DROPCOMBATPERCH (2026-11-20, pc-hummer, on fix/rust-is-staggered 11097cef, main c687c135):** AI-assisted (Claude
Code, Opus 5.5). `WillowAIPawn.DropCombatPerch` in Off from NATIVE_DROP_COMBAT_PERCH.md at 08edad6d (`ow-combat`
`hit/pools.rs`): D1 (two texts), D2 for native descendants, the three fields by name, the guard answering nothing,
D3 when it holds; Reference unchanged. `cargo build --workspace` clean; `cargo test --workspace` 2,758 passed / 0
failed / 49 ignored (12 new tests); `crate_edges_test.py` OK. Installed Off `--slice-gate` strict: 3 ok / 10 NotYet / 1
error, all 14 rows byte-identical to main's. Link 9 passes 1 of its driver's 2 steps (`spawn`) and stops at `cast` on
main's noun `ReplicationStrategy` 2 refusal, before the lift asks this native. No real-game run; `UNVERIFIED` in game.
Decision 2026-11-19-232; logs under ignored `local/p2/is/`.

**IMPL-IS-STAGGERED (2026-11-20, pc-hummer, on main c687c135):** AI-assisted (Claude Code, Opus 5.5).
`WillowPawn`/`WillowAIPawn.IsStaggered` in Off from NATIVE_IS_STAGGERED.md as final at b8b22422 (`ow-combat`
`hit/pools.rs`): S1 (two texts), S2 for native descendants and other pawn classes, `StaggerState.bIsStaggered` read by
name; Reference unchanged. `cargo build --workspace` clean; `cargo test --workspace` 2,745 passed / 0 failed / 49
ignored (7 new tests); `crate_edges_test.py` OK. Installed Off `--slice-gate` strict: 3 ok / 10 NotYet / 1 error, all
14 rows byte-identical to main's. Link 9 passes 1 of its driver's 2 steps (`spawn`) and stops at `cast` on main's noun
`ReplicationStrategy` 2 refusal, before the lift asks this native. No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-227; logs under ignored `local/p2/is/`.

**IMPL-ATTACH-MESH-TO-PICKUP part 2 (2026-11-19, zuhu desktop, on fix/rust-attach-mesh-to-pickup fdef74e, #511):**
AI-assisted (Claude Code, Opus 5.5). `SetLightEnvironment`, `InitializePickupForRBPhysics` and `SpawnPickupParticles`
in Off from NATIVE_ATTACH_MESH_TO_PICKUP.md section 6. `cargo build --workspace -j 2` clean; `cargo test --workspace
-j 2` 2,459 passed / 0 failed / 46 ignored. Installed Off `--slice-gate` strict, under the machine lock, base and branch
each built: 0 ok / 5 NotYet / 9 errors before, 0 ok / 7 NotYet / 7 errors after. Links 6 and 7 stay at 0 of 1 step
(`make`) and move from the unbound `PrimitiveComponent.SetLightEnvironment(object)` to "stopped after partial writes:
WillowPickup.InitializePickupForRBPhysics: the pickup's rigid-body set-up" (`NotYet { LayerAbsent }`). The other 12
rows are byte-identical. ctest and verify_packages not run (no C++ change). No real-game session. Review r2 of #520
(the destinations' slots read before the first write; 2 tests): `cargo test --workspace -j 2` 2,463 passed / 0 failed
/ 46 ignored; no slice-gate rerun (the slice's path has full slot vectors).

**IMPL-GEARBOXMIND-QUERYINTERFACE (2026-11-20, pc-hummer, on main at 66378ec4):** AI-assisted (Claude Code, Opus
5.5). `Controller`'s QueryInterface rule in Off from NATIVE_GEARBOXMIND_QUERYINTERFACE.md (#589 at 53daae12),
read-only along the chain (ow-vm read-only dispatch and plain answer over entries resolved at set-up), with link 9's
interface set-up as a harness input. `cargo test --workspace --locked -j 2` 2,575 passed / 0 failed / 48 ignored (10
new in `pawn_interface::read_only`, 1 in `reflect`); `cargo clippy` adds no warning in the touched files;
`crate_edges_test.py` OK. Installed Off `--slice-gate` (strict, under the lock) on a throwaway merge with #579, #586,
#564, #548, #555, #560, #523 and #595: ok 2 / NotYet 3 / error 9 of 14; link 9 runs 2 of 2 steps and stops at the
unbound `GearboxMind.GetAIComponent()` (was the NotYet at GearboxMind's own QueryInterface). ctest and
verify_packages not run (no C++ change). No real-game run; every rule UNVERIFIED.

**FOLLOWUP-BOUNDING-CYLINDER-TEMPLATE (2026-11-20, pc-hummer, on #595 9de51c43):** AI-assisted (Claude Code, Opus
5.5). `GetBoundingCylinder`'s template test walks the receiver's outer chain (a class default anywhere: template; an
export or resource on it: T; only run-time objects: live), read-only. `cargo build --workspace` clean; `cargo test
--workspace` 2,578 passed / 0 failed / 48 ignored (4 new); `crate_edges_test.py` OK. Installed Off `--slice-gate` strict: 2 ok / 4 NotYet / 8
errors, all 14 rows byte-identical to #595 9de51c43; links 6 and 7 still stop at `make` (unbound
`SetLightEnvironment`), so this native is proved only by tests.

**IMPL-GET-BOUNDING-CYLINDER (2026-11-20, pc-hummer, on main 66378ec4):** AI-assisted (Claude Code, Opus 5.5).
`Actor`/`Pawn`/`NavigationPoint.GetBoundingCylinder` in Off from NATIVE_GET_BOUNDING_CYLINDER.md at 6800f603
(`ow-world` `world::bounding_cylinder`): body by exact class, refusals C/T/N/L/S, both out arguments written last;
`placement::extent` unchanged. `cargo build --workspace` clean; `cargo test --workspace` 2,572 passed / 0 failed / 48
ignored (8 new); `crate_edges_test.py` OK. On the pre-rebase base 93ce3dbf: `ctest` 165 / 166, the one failure
`py-real_game_pending` (2 checks that need a folder outside the Temp directory, which a worktree under Temp cannot
give; no Python or real-game helper changed); `verify_packages.py --reader` exit 0. Installed Off `--slice-gate`
strict: 2 ok / 4 NotYet / 8 errors, all 14 rows byte-identical with and without the binding. Links 6 and 7 pass 0
steps and stop in `make` at the unbound `PrimitiveComponent.SetLightEnvironment(object)`, before `touch` reaches this
native, so it is proved by tests only. No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-203; logs under
ignored `local/p2/gbc/`.

**IMPL-ATTACH-MESH-TO-PICKUP part 1 (2026-11-19, zuhu desktop, on fix/rust-set-anim-tree-template c1c8a73, #495):**
AI-assisted (Claude Code, Opus 5.5). `SetForceRefPose`, `SetHidden`, `SetOwnerNoSee`, `HideBoneByName` and
`Actor.AttachComponent` in Off from NATIVE_ATTACH_MESH_TO_PICKUP.md part 1, with an empty scene layer in the pickup
world (the hub's ruling, a port answer, UNVERIFIED). `cargo build --workspace -j 2` clean; `cargo test --workspace -j 2`
2,454 passed / 0 failed / 46 ignored. Installed Off `--slice-gate` strict, under the machine lock, base and branch each
built: 0 ok / 5 NotYet / 9 errors both times. Links 6 and 7 stay at 0 of 1 step (`make`) and move from the unbound
`SkeletalMeshComponent.SetForceRefPose(bool)` to the unbound `PrimitiveComponent.SetLightEnvironment(object)`. The other
12 rows are byte-identical. ctest and verify_packages not run (no C++ change). No real-game session.

**IMPL-SETANIMTREETEMPLATE (2026-11-19, pc-hummer, on main d72b96d4 after #487 landed; the hub's section 5 ruling):**
AI-assisted (Claude Code, Opus 5.5). `SkeletalMeshComponent.SetAnimTreeTemplate` in Off from
NATIVE_SET_ANIM_TREE_TEMPLATE.md (df863aff; the section 5 ruling at 466350b): the preflight, an empty `ComponentRegistry` layer on the pickup world
(walk answers "nothing"), the teardown writes, and `AnimTreeUpdated` only with an `Owner`. `cargo build --workspace
--locked -j 4` clean; `cargo test --workspace --locked -j 4` 2,413 passed / 0 failed / 46 ignored (8 new);
`crate_edges_test.py` OK. Installed Off `--slice-gate` strict: 0 ok / 5 NotYet / 9 errors; links 6 and 7 get past the
native and stop in `make` (0 of 3 and 0 of 4 steps) at the unbound `SkeletalMeshComponent.SetForceRefPose(bool)`.
ctest and verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-107
(placeholder); logs under ignored `local/p2/anim-tree/`.

**IMPL-MESH-REST (2026-11-19, zuhu desktop, on #561 at 07d53556 with d83f2fee merged):** AI-assisted (Claude Code,
Opus 5.5). The rest of the made-Mesh step in Off from NATIVE_MESH_REST.md (#563 at 9ebde2bb), replacing #557's
`MESH_REST` refusal, and Maya's animation tree as a harness input in `apply_class_inputs`. `cargo test --workspace
--locked -j 2` 2,505 passed / 0 failed / 48 ignored after review r1 of #570 (9 new in `body_composition`, 3 in ow-vm); `cargo clippy -p
ow-world -p ow-vm -p ow-cli --all-targets` adds no warning in the touched files; `crate_edges_test.py` OK. Installed
Off `--slice-gate` (strict, under the lock) on a throwaway merge with #500 and #528: ok 1 / NotYet 10 / error 3 of 14;
the tree preload loads 71 objects, and link 1 runs 0 of 1 step, stopped unmarked at the aim-offset refusal (links 3, 4,
5, 13 and 14 blocked behind it). ctest and verify_packages not run (no C++ change). No real-game run; every rule
UNVERIFIED.

**SLICE-HARNESS-MAYA-MESH (2026-11-20, pc-hummer, on #585 r1 at 9dfff984):** AI-assisted (Claude Code, Opus 5.5).
Maya's skeletal mesh preload and ow-cli's skeleton source (UModel `.psk` exports under ignored `local/external/psk`) as
UNVERIFIED Off harness inputs in the `maya` step; natives and Reference unchanged. `cargo test --workspace --locked -j
2` 2,578 passed / 0 failed / 49 ignored (4 new in `combat/mesh_harness_tests`); `cargo clippy -p ow-cli --all-targets`
adds no warning in the touched files; `crate_edges_test.py` OK. Installed Off `--slice-gate` (strict, under the lock)
on a throwaway merge with #570, #500 and #528: ok 1 / NotYet 10 / error 3 of 14; the mesh preload logs "loaded", and
link 1 runs 0 of 1 step, its first stop moved from #569's unloaded-mesh `LayerAbsent` to NATIVE_COPY_ANIM_TREE.md
section 2 step 5 (the initialisation's pass over the component; links 3, 4, 5, 13 and 14 blocked behind it). ctest and
verify_packages not run (no C++ change). No real-game run; every rule UNVERIFIED.

**FIX-BODY-COMPOSITION-LOOKUP-STALE (2026-11-20, pc-hummer, on #608 at 383c929c):** AI-assisted (Claude Code, Opus
5.5). The body composition's animation-node lookups over the planned copy (NATIVE_ANIM_NODE_LOOKUP.md sections 1 and
2) instead of #557's stale no-instance refusal. `cargo test --workspace --locked -j 2` 2,589 passed / 0 failed / 49
ignored (5 new in `anim_tree_copy`, 3 template-era tests dropped in `body_composition`); `cargo clippy -p ow-world
--all-targets` adds no warning in the touched files; `crate_edges_test.py` OK. Installed Off `--slice-gate` (strict,
under the lock) on a throwaway merge with #592, #570, #500 and #528, the owner's event (#612) a local stand-in: ok 1 /
NotYet 10 / error 3 of 14; links 1, 3, 4, 5, 13 and 14 run 0 of 1 step and stop at the body's rendering hooks
(LayerAbsent), past the lookups. ctest and verify_packages not run (no C++ change). No real-game run; every rule
UNVERIFIED.

**IMPL-COPY-COMPONENT-PASS (2026-11-20, pc-hummer, on #585 at ce3be0ac):** AI-assisted (Claude Code, Opus 5.5). The
tree initialisation's component pass and per-bone control arrays in Off from NATIVE_COPY_COMPONENT_PASS.md (#598 at
5196fcec), replacing #575's two step-5 refusals, plus InitTag and the two per-node arrays. `cargo test --workspace
--locked -j 2` 2,587 passed / 0 failed / 49 ignored (14 new in `anim_tree_copy`); `cargo clippy -p ow-world --all-targets` adds no warning in the touched files;
`crate_edges_test.py` OK. Installed Off `--slice-gate` (strict, under the lock) on a throwaway merge with #592, #570,
#500 and #528: ok 1 / NotYet 10 / error 3 of 14; links 1, 3, 4, 5, 13 and 14 run 0 of 1 step and stop at the owner's
event after the tree initialisation (was the component pass). ctest and verify_packages not run (no C++ change). No
real-game run; every rule UNVERIFIED.

**IMPL-PER-BONE-WEIGHTS (2026-11-19, pc-hummer, on #575 at e30b7fa9 with #569 at 10be0643 merged):** AI-assisted
(Claude Code, Opus 5.5). The per-bone weight builders in Off from NATIVE_PER_BONE_WEIGHTS.md (#580 at 5b1a567). `cargo
test -p ow-world` 337 passed / 0 failed / 5 ignored; `cargo test -p ow-object` 138 / 0; `cargo test --workspace --locked
-j 2` 2,570 passed / 0 failed / 49 ignored; `cargo clippy -p ow-world --all-targets` adds no warning in the touched
files; `crate_edges_test.py` OK. Installed Off `--slice-gate` (strict, under the lock) on a throwaway merge with #570,
#500 and #528: ok 1 / NotYet 10 / error 3 of 14. Links 1, 3, 4, 5, 13 and 14 moved from the F `MESH_LAYER` refusal to
#569's "the unloaded skeletal mesh Skel_SirenBody", unmarked. ctest and verify_packages not run (no C++ change). No
real-game run; `UNVERIFIED` in game. Decision 2026-11-19-194.

**IMPL-ANIM-NODE-INIT (2026-11-19, pc-hummer, on #561 at d83f2fee):** AI-assisted (Claude Code, Opus 5.5). The aim-offset
fix-up and the per-node initialisation in Off from NATIVE_ANIM_NODE_INIT.md (#566 at 07b2dd68). `cargo test -p
ow-world` 314 passed / 0 failed / 5 ignored after review r1 of #575; `cargo test --workspace --locked -j 2` 2,511 passed
/ 0 failed / 48 ignored; `cargo clippy -p ow-world -p ow-object --all-targets` adds no warning in the touched files;
`crate_edges_test.py` OK. Installed Off `--slice-gate` (strict, under the lock) on a throwaway merge with #570, #500 and
#528: ok 1 / NotYet 10 / error 3 of 14. Links 1, 3, 4, 5, 13 and 14 run 0 steps (link 1 0 of 1) and stop unmarked at the
per-bone weight build's `MESH_LAYER` `LayerAbsent`. ctest and verify_packages not run (no C++ change). No real-game
run; `UNVERIFIED` in game. Decision 2026-11-19-193.

**IMPL-COPY-ANIM-TREE (2026-11-19, pc-hummer, on #542 at b2b4de8a, with #557 at 9694cc30 merged):** AI-assisted
(Claude Code, Opus 5.5). The animation tree's copy and initialisation in Off from NATIVE_COPY_ANIM_TREE.md (#551 at
4a363c0c), wired before #557's lookups. `cargo test -p ow-world` 298 passed / 0 failed / 5 ignored after review r1 of #561 (18 in
`anim_tree_copy`); `cargo test --workspace --locked -j 2` 2,486 passed / 0 failed / 48 ignored; `cargo clippy -p ow-world
--all-targets` adds no warning in the touched files; `crate_edges_test.py` OK. Installed Off `--slice-gate` (strict,
under the lock) on a throwaway merge with #500 and #528: ok 1 / NotYet 10 / error 3 of 14. Link 1 runs 0 of 1 step,
stopped at #543's `LayerAbsent` for the unloaded tree template `AnimTree_Player`. With the tree preloaded (local probe
only), link 1 stops at the aim-offset refusal. ctest and verify_packages not run (no C++ change). No real-game run;
`UNVERIFIED` in game. Decision 2026-11-19-172.

**IMPL-BODY-COMPOSITION (2026-11-19, pc-hummer, on fix/rust-init-pawn-meshes 1551f039, #522; note #529 at 87c8b9cd):**
AI-assisted (Claude Code, Opus 5.5). `InstanceDataHelper.InitializeBodyComposition` in Off for the pawn wrapper: a
write-free preflight, then the writes in the note's order under #518's counters. `InitializePawnMeshes` runs the build.

Results:
- `cargo test -p ow-world`: 271 passed / 0 failed / 5 ignored (after review r1 of #542; 264 before it).
- `cargo test --workspace`: 2,466 passed / 0 failed / 48 ignored after review r1 of #542 (2,459 before); one existing ow-defs test warning.
- Installed Off `--slice-gate` strict (release build; #528 @ 9cd49c14 merged and `Engine.IsSplitScreen()` bound false,
  both locally only; under the machine lock): 1 ok / 10 NotYet / 3 errors.
  - Links 1, 3, 4, 5, 13 and 14: 0 of their driver steps, stopping in `maya:8` at the preflight's `LayerAbsent` "the
    new Mesh's animation nodes" (Maya's attachment 0), unmarked.
  - After review r1 of #542 (#528 @ 0e8e2312 merged instead): the same, link 1 0 of 1 at the same stop.

ctest and verify_packages were not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-153.

**IMPL-INIT-PAWN-MESHES (2026-11-19, pc-hummer, on research/rust-init-pawn-meshes 5d284e27, #517 on #513):**
AI-assisted (Claude Code, Opus 5.5). `WillowPawn` / `WillowPlayerPawn.InitializePawnMeshes()` in Off:
- `BodyClass` None and an empty attachment list do nothing;
- a missing helper class and an unloaded `BodyClass` are named `NotYet { NotRead }`s, and nothing is loaded (hub
  ruling, #517 r1);
- a non-empty attachment list is `NotYet { LayerAbsent }`;
- nothing is written.

Results:
- `cargo test -p ow-world`: 245 passed / 0 failed / 5 ignored.
- `cargo test --workspace`: 2,684 passed / 0 failed / 54 ignored.
- Installed Off `--slice-gate` strict (release build; `Engine.IsSplitScreen()` bound false locally only, uncommitted;
  under the machine lock): 0 ok / 11 NotYet / 3 errors.
  - Links 1, 3, 4, 13 and 14 stop in `maya:8` at the native's lazy-`BodyClass` `NotRead` (`BodyClass_Siren` is an
    unloaded handle).
  - Link 1: 0 of 1 driver step.

ctest and verify_packages were not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-134; logs under ignored `local/analysis/`.

**IMPL-ALL-NAVIGATION-POINTS (2026-11-19, pc-hummer, on research/rust-is-split-screen 368f3f0f, #510 on #508):**
AI-assisted (Claude Code, Opus 5.5). `WorldInfo.AllNavigationPoints` in Off: the serialized list walked with a class
filter, six open cases `NotYet { NotRead }`. `cargo test -p ow-world -p ow-cli --locked -j 4` 343 passed / 0 failed / 8
ignored; `crate_edges_test.py` 19 OK. Installed Off `--slice-gate` strict (release build, `Engine.IsSplitScreen()` bound
false locally only, uncommitted, under the machine lock): 0 ok / 5 NotYet / 9 errors. Links 1, 3, 4, 5, 13 and 14 get
past the iterator and stop in `maya:8` at the unbound `WillowPlayerPawn.InitializePawnMeshes()` (link 1: 0 of 1 driver
step). ctest and verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-125; logs under ignored `local/analysis/navpoints/`.

**IMPL-IS-EDITOR (2026-11-19, pc-hummer, on fix/rust-link1-after-login c42f2e35, #499):** AI-assisted (Claude Code, Opus
5.5). `WillowGameInfo.IsEditor` answers false in Off (the hub's ruling). `cargo build --workspace --locked -j 4` clean;
`cargo test --workspace --locked -j 4` 2,422 passed / 0 failed / 48 ignored; `crate_edges_test.py` 19 OK. Installed Off
`--slice-gate` strict (release build, under the machine lock): 0 ok / 5 NotYet / 9 errors; links 1, 3, 4, 5, 13 and 14
get past `IsEditor` and stop in `maya:8` at the unbound `Engine.IsSplitScreen()` (link 1: 0 of 1 driver step). ctest and
verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-120; logs under
ignored `local/p2/is-editor/`.

**IMPL-LINK1-AFTER-LOGIN (2026-11-19, pc-hummer, on fix/rust-engine-object-offline aca14fa0, #501 on #479):**
AI-assisted (Claude Code, Opus 5.5). `UpdateURL` and `SaveConfig` (nothing, under the read-back census) and
`UpdateInputContext` in Off from NATIVE_LOGIN_URL_CONFIG_INPUT.md, review r1 of #499 folded in. `cargo build --workspace
--locked -j 4` clean; `cargo test --workspace --locked -j 4` 2,421 passed / 0 failed / 48 ignored (after the rebase onto
#501 aca14fa0; the PR touches ow-vm); the installed `off_url_and_config_have_no_readers` and
`the_slice_calls_update_input_context_without_a_player_input` pass with `--ignored` (one call, no `PlayerInput`).
`crate_edges_test.py` 19 OK. Installed Off `--slice-gate` strict: 0 ok / 5 NotYet / 9 errors; links 1, 3, 4, 5, 13 and
14 get past the login natives and stop in `maya:8` at the unbound `WillowGameInfo.IsEditor()` (link 1: 0 of 1 driver
step). ctest and verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-113; logs under ignored `local/p2/link1login/`.

**IMPL-ENGINE-OBJECT-OFFLINE (2026-11-19, zuhu desktop, on fix/rust-login-status afb32d8, #479 on main d72b96d):**
AI-assisted (Claude Code, Opus 5.5). The combat-run world registers, in Off only, a `GameEngine` with an empty
`OnlineSubsystem` slot (port Off answer: the port has no online subsystem; UNVERIFIED against a logged-in game).
`cargo build --workspace -j 2` clean; `cargo test --workspace -j 2` 2,418 passed / 0 failed / 46 ignored. Installed
Off `--slice-gate` strict, same base with and without the call: 0 ok / 11 NotYet / 3 errors both times. Links 1, 3, 4, 5,
13 and 14 move from `GameEngine.GetOnlineSubsystem` to "stopped after partial writes: `PlayerController.UpdateURL`"
in `maya:8` (link 1 records 1 step, `maya:8`; the rest are blocked by link 1). The other 8 rows are byte-identical.
ctest and verify_packages not run (no C++ change). No real-game session.

**MAP-ROUTE-FOLLOWUP (2026-11-19, pc-hummer, on #556 at 07b1ffda):** AI-assisted (Claude Code, Opus 5.5). Review r1 of
#556's five parked findings: the adopted globals' own definition, adopt's mark test, `SetGameStage` on the possessed
pawn, no director or waypoint made on the map route, `--map` parse tests. `cargo test -p ow-mission` 213 passed / 0
failed / 1 ignored; `cargo test --workspace --locked -j 2` 2,474 passed / 0 failed / 48 ignored; `cargo clippy -p
ow-mission -p ow-cli --all-targets` adds no warning in the touched files; `crate_edges_test.py` OK. Installed Off
`--slice-gate` (strict, under the lock), head against base: default route ok 1 / NotYet 4 / error 9 of 14 and
`--map Sanctuary_P` ok 1 / NotYet 4 / error 9 of 14, every row identical; on the map route link 1 stops at setup,
before adopt. ctest and verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-186.

**IMPL-SLICE-MAP-WORLD (2026-11-19, zuhu desktop, on #547 at 15d01ca):** AI-assisted (Claude Code, Opus 5.5). The
bridge's adopt path (`MissionScript::adopt`) and the combat group on the map route behind `--map`
(`cmd/combat/map_route.rs`), lane 4's first PR. `cargo test --workspace -j 2` 2,468 passed / 0 failed / 48 ignored;
clippy flags nothing in the changed files. Installed Off `--slice-gate` strict, under the machine lock, base and branch
each built: 1 ok / 4 NotYet / 9 errors both, the default output byte-identical. With `--map Sanctuary_P`: 1 ok / 4
NotYet / 9 errors; link 1 runs 0 of its 2 steps and stops at `setup`, at the unbound `WillowGameInfo.IsEditor()` inside
`Login` (the default route: 1 step run, stopped at `maya:8` on `GameEngine.GetOnlineSubsystem()`). On a throwaway local
merge with #508 (13992c2f), both routes stop link 1 at the unbound `Engine.IsSplitScreen()` (the map route at `setup`,
the default at `maya:8`). ctest and verify_packages not run (no C++ change). No real-game session; `UNVERIFIED` in
game. Logs under ignored `local/p2/slice-map-world/`.

**IMPL-MAPLOAD-PLAYER (2026-11-19, zuhu desktop, on #537 at 3d25b4a):** AI-assisted (Claude Code, Opus 5.5). The
map load's local-player spawn in Off (`mapload::player`, NATIVE_MAP_LOAD.md sections 4 and 6): Login, the attachment,
PostLogin, the local player. `cargo test --workspace -j 2` 2,462 passed / 0 failed / 48 ignored; clippy flags nothing in
the changed files. Installed Off `--slice-gate` strict, under the machine lock, base and branch each built: 1 ok / 4
NotYet / 9 errors both, the output byte-identical. Installed ignored test (Sanctuary, through the map route then the
spawn): it gets into `Login` and stops at the unbound native `WillowGameInfo.IsEditor()` (a plain error, unmarked); no
play actor. ctest and verify_packages not run (no C++ change). No real-game session; `UNVERIFIED` in game. Logs under
ignored `local/p2/mapload-player/`.

**IMPL-MAPLOAD-GAMEINFO (2026-11-19, zuhu desktop, on main d52bcc7):** AI-assisted (Claude Code, Opus 5.5). The map
load's game info routine in Off (`mapload::game_info`, NATIVE_MAP_LOAD.md section 3) and the map route
`bring_up_map_for_play`. `cargo test --workspace -j 2` 2,453 passed / 0 failed / 47 ignored. Installed Off
`--slice-gate` strict, under the machine lock, base and branch each built: 1 ok / 4 NotYet / 9 errors both, every row
byte-identical. Installed ignored test (Sanctuary through the map route): `WillowGame.WillowCoopGameInfo` spawned into
`WorldInfo.Game`; its `InitGame` and `PreBeginPlay` stop, marked, at `GotoState('Auto')`. ctest and verify_packages not
run (no C++ change). No real-game session; `UNVERIFIED` in game. Logs under ignored `local/p2/mapload-gameinfo/`.
Review r1 of #537: `cargo test -p ow-world` 231 passed / 0 failed / 6 ignored, `-p ow-core -p ow-vm` 269 passed / 0
failed; clippy flags nothing in the changed files; the installed map-route test passes under the lock, the game info's two
stops now asserted marked.

**IMPL-MAPLOAD-NAVLIST (2026-11-19, pc-hummer, on main d52bcc79):** AI-assisted (Claude Code, Opus 5.5). The level
navigation join in Off from NATIVE_MAP_LOAD.md sections 5 and 6 (`ow-world` `world::nav_join`, nothing calls it yet):
a write-free preflight of the three list kinds, then the splices; the level tail's pairs are `NotYet { NotRead }`.
`cargo build --workspace -j 4` clean; `cargo test --workspace -j 4` 2,451 passed / 0 failed / 46 ignored (ow-world 227
passed, nav_join 7); clippy no new warnings; `crate_edges_test.py` OK. Installed Off `--slice-gate` strict (release
build, under the machine lock): 1 ok / 4 NotYet / 9 errors, as expected with nothing calling the join (the stops are
main's: `maya:8` `GetOnlineSubsystem`, `SetAnimTreeTemplate`, `CacheWeaponCard`, the bounding-cylinder rule,
`GetAllMissions`). ctest and verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-155; logs under ignored `local/analysis/navjoin/`.

**IMPL-AUTO-STATE (2026-11-19, pc-hummer, on main d52bcc79):** AI-assisted (Claude Code, Opus 5.5). `GotoState('Auto')`
in Off from NATIVE_AUTO_STATE.md section 5 (#540). `cargo build --workspace --locked -j 4` clean; `cargo test -p ow-vm`
207 passed / 0 failed / 1 ignored; `cargo test --workspace --locked -j 4` 2,446 passed / 0 failed / 46 ignored;
`cargo clippy -p ow-vm --all-targets` adds no warning in `states.rs`; `crate_edges_test.py` OK. The installed Off
`--slice-gate` (strict, under the lock): ok 1 / NotYet 4 / error 9 of 14 on the head, and the same rows on a throwaway
merge with #537 at 3d25b4a; no row reaches `GotoState('Auto')`. ctest and verify_packages not run (no C++ change). No real-game run;
`UNVERIFIED` in game. Decision 2026-11-19-156.

**IMPL-LINK12-UNLOCK-ACHIEVEMENT (2026-11-19, zuhu desktop, on origin/main d52bcc7):** AI-assisted (Claude Code, Opus
5.5). `WillowPlayerController.UnlockAchievementIfConditionsMet` in Off, from
NATIVE_UNLOCK_ACHIEVEMENT_IF_CONDITIONS_MET.md (#541); the Fire driver preloads the installed globals definition (a
harness input); #518's counter files copied from 1f1a2bc. `cargo build --workspace --locked` clean; `cargo test
--workspace --locked` 2,457 passed / 0 failed / 46 ignored (run before the last two commits; `ow-inventory` 192 and
`ow-cli` re-run after them, all passing); `crate_edges_test.py` 19 OK. Installed Off `--slice-gate` strict, on a
local merge of this branch with #533, #496, #497, #509, #512, #524 and #526 (never pushed): 3 ok / 2 NotYet / 9
errors. Links 10 and 11 ok; link 12 passes 1 of 6 driver steps (`use`) and gets past the native to the port's own
"stopped after partial writes: MissionTracker.AllExpansionSideMissionsComplete with a controller …" (`NotYet
{ NotRead }`, NATIVE_MISSION_OBJECTIVES.md). ctest and verify_packages not run (no C++ change). No real-game run;
`UNVERIFIED` in game. Decision 2026-11-19-157 (unlock achievement); logs under ignored `local/p2/link12-unlock/`.

**IMPL-SET-OBJECT-BEHAVIOR-VARIABLE (2026-11-19, desktop, on main d52bcc7):** AI-assisted (Claude Code, Opus 5.5). The
five `BehaviorProviderDefinition.Set<Kind>BehaviorVariable` setters in Off, from NATIVE_SET_BEHAVIOR_VARIABLE.md (#538,
94aa293), with #518's counters carried from 1f1a2bc. `cargo build --workspace -j 2` clean; `cargo test -p ow-behavior
-j 2` 100 passed / 0 failed (9 new in `kernel::variables`); `cargo test --workspace -j 2` 2,455 passed / 0 failed / 46
ignored; clippy flags nothing in the changed files. Installed Off `--slice-gate` strict on a local merge of this branch
with #523 and #535 (never pushed; #523's two counter call sites renamed to #518's names for the merge): 1 ok / 3
NotYet / 10 errors. Link 9 passes `spawn` and stops in `cast` at the unbound `ActionSkill.GetBehaviorProviderDefinition()`,
the argument of `OnSelectedTarget`'s event fire, after the two setter calls. No baseline run without this change. ctest and
verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-159; logs under
ignored `local/p2/sbv/`.

**IMPL-CARD-BODY-REST (2026-11-19, pc-hummer, on #521 at 83d4e909):** AI-assisted (Claude Code, Opus 5.5).
`CacheWeaponCard`'s card order and displayed value in Off from NATIVE_CARD_BODY_REST.md (#532). `cargo test -p
ow-inventory` card tests 39 passed / 0 failed (after the hub's band ruling on #532); `cargo test --workspace --locked -j 4` 2,494 passed / 0 failed / 46
ignored; `cargo clippy -p ow-inventory --all-targets` adds no warning in `equip/card`; `crate_edges_test.py` OK.
Installed Off `--slice-gate` (strict, under the lock) on a throwaway merge with #534 at 8fc0420b: ok 0 / NotYet 5 /
error 9 of 14, the same rows as #534 alone. Link 8 passes 1 of 5 steps and stops at `init` on the unbound
`WillowWeapon.RecomputeAttributeBaseValues()`, after the card. ctest and verify_packages not run (no C++ change). No
real-game run; `UNVERIFIED` in game. Decision 2026-11-19-163.

**IMPL-ACTIVATE-EVENT-PARAMETERS (2026-11-19, pc-hummer, on main d52bcc79):** AI-assisted (Claude Code, Opus 5.5).
`ActivateBehaviorEventFromScript` with `Parameters` in Off from NATIVE_ACTIVATE_BEHAVIOR_EVENT_PARAMETERS.md (#549):
a typed payload, object bindings by connection index on a type match, a write-free preflight over every sequence with
the event. `cargo build --workspace -j 4` clean; `cargo test --workspace -j 4` 2,450 passed / 0 failed / 46 ignored
(ow-behavior 97); clippy no new warnings; `crate_edges_test.py` OK. Installed Off `--slice-gate` strict (release build,
under the machine lock) on a throwaway local merge with #523 (d2227301), #535 (7f8fc7d7) and #548 (22b29bfb), with
#518's counter files and a local rename of #523's two counter calls, never pushed: 1 ok / 3 NotYet / 10 errors. Link
9 passes `spawn` (1 of 2) and stops in `cast` at the unbound `ActionSkill.GetBehaviorProviderDefinition()`, the
argument `OnSelectedTarget` evaluates just before this native, so the new rule is not reached yet. ctest and
verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-166; logs under
ignored `local/analysis/aep-impl/`.

**IMPL-ANIM-NODE-LOOKUP (2026-11-19, desktop, on fix/rust-body-composition b2b4de8a, #542):** AI-assisted (Claude
Code, Opus 5.5). The new Mesh's animation-node lookups from the template, write-free and loading nothing, from
NATIVE_ANIM_NODE_LOOKUP.md (#543, b09720f6). `cargo build --workspace -j 2` clean; `cargo test -p ow-world -j 2` 279
passed / 0 failed / 5 ignored (8 new); `cargo test --workspace -j 2` 2,474 passed / 0 failed / 48 ignored; clippy flags
nothing in the changed files. Installed Off `--slice-gate` strict on a local merge with #528 (0e8e2312; never pushed),
under the machine lock: 1 ok / 4 NotYet / 9 errors. Link 1 is 0 of its 2 driver steps, stopping in `maya:8` at the
unbound `Engine.IsSplitScreen()`, as are links 3, 4, 5, 13 and 14. With `IsSplitScreen` also bound false, locally only
as #542's row did: 1 ok / 10 NotYet / 3 errors, and those six links stop in `maya:8` at #529's unmarked `LayerAbsent`
"the new Mesh's animation nodes", now raised by this check (its record names both notes). ctest and verify_packages not
run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-168; logs under ignored
`local/p2/anl/`.

**IMPL-DAMAGE-TYPE-RESOLVER (2026-11-19, zuhu desktop, on 144a73f5 / #573):** AI-assisted (Claude Code, Opus 5.5).
`WillowGame.DamageTypeAttributeValueResolver` in `ow-attr`, from NATIVE_WEAPON_RULES.md section 5 ("Name-part
expressions") with the weapon's damage type by NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md (#576 at 3225f887), reading
resident objects only; Off only, Reference keeps the unregistered refusal, and only constant values are evaluated
(reviews r1 and r2 of #584). `cargo test --workspace -j 2` 2,483 passed / 0 failed / 46 ignored (12 new in `ow-attr`); clippy
flags nothing in the changed files; `crate_edges_test.py` 19 OK. Installed Off `--slice-gate` strict on a local,
unpushed merge with #534 (8fc0420b) and #552 (0438f6b3): 1 ok / 4 notyet / 9 error. Link 8 is 1 of 5, past the
evaluators, and stops at `init` on the unbound `InventoryBalanceDefinition.GetExpLevelFromManufacturerData(object,int)`
(an error, the expected next stop). Links 6 and 7 are unchanged (`make`, unbound
`SkeletalMeshComponent.SetAnimTreeTemplate`). ctest and `verify_packages.py` were not run (no C++ or reader change).

**IMPL-GET-PARTICLE-EFFECT (2026-11-19, zuhu desktop, on 3a598e4c / #558):** AI-assisted (Claude Code, Opus 5.5).
`EffectCollectionDefinition.GetParticleEffect(object)` in Off from NATIVE_GET_PARTICLE_EFFECT.md (#562 at 865ba911);
the equip run's world registers `ow_skills::action_skill::Pack` in Off and preloads the muzzle-flash collections and
their evaluators as a logged harness input. `cargo test --workspace -j 2` 2,466 passed / 0 failed / 46 ignored (14 new
in `ow-inventory`); clippy flags nothing in the changed files; `crate_edges_test.py` OK. Installed Off `--slice-gate`
strict on a local, unpushed merge with #534 (8fc0420b) and #552 (0438f6b3): 1 ok / 5 notyet / 8 error. Link 8 is 1 of
5 at `init`, now `notyet`, past `GetParticleEffect` and stopped in the first evaluator at
`DamageTypeAttributeValueResolver` (NATIVE_ATTRIBUTES.md section 6, marked after partial writes). ctest and
verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-188.

**IMPL-RECOMPUTE-ATTRIBUTE-BASE-VALUES (2026-11-19, pc-hummer, on main d52bcc79):** AI-assisted (Claude Code, Opus
5.5). `WillowWeapon.RecomputeAttributeBaseValues()` in Off from NATIVE_RECOMPUTE_ATTRIBUTE_BASE_VALUES.md (#539).
`cargo test --workspace -j 2` 2,450 passed / 0 failed / 46 ignored (6 new in `ow-inventory`); clippy flags nothing in
the changed files (`-D warnings` stops in `ow-package`, unchanged); `crate_edges_test.py` OK. Installed Off
`--slice-gate` strict on a local, unpushed merge of this change with #534 (8fc0420b) and #552 (f17a9d60): 1 ok / 4
notyet / 9 error. Link 8 is 1 of 5 at `init`, now past `RecomputeAttributeBaseValues` and stopped at the unbound
`EffectCollectionDefinition.GetParticleEffect(object)`. ctest and verify_packages not run (no C++ change). No
real-game run; `UNVERIFIED` in game. Decision 2026-11-19-169.

**IMPL-GET-BEHAVIOR-PROVIDER-DEFINITION (2026-11-19, pc-hummer, on main d52bcc79):** AI-assisted (Claude Code, Opus
5.5). `ActionSkill.GetBehaviorProviderDefinition` in Off from NATIVE_GET_BEHAVIOR_PROVIDER_DEFINITION.md (#553): the
receiver's own field as stored, no load (low, a fitted step, UNVERIFIED). `cargo build --workspace -j 2` clean; `cargo test
--workspace -j 2` 2,449 passed / 0 failed / 46 ignored before review r1 (ow-skills provider_definition 6 after r1's added test); clippy no new warnings;
`crate_edges_test.py` OK. Installed Off `--slice-gate` strict (release build, under the machine lock) on a throwaway
local merge with #523, #535, #548 (22b29bfb) and #555 (0e9092a8), never pushed: 1 ok / 3 NotYet / 10 errors. Link 9
passes `spawn` (1 of 2) and gets past this native in `cast`, then stops at the unbound `Object.GetFuncName()`, the next
argument of the same event call. ctest and verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in
game. Decision 2026-11-19-171; logs under ignored `local/analysis/gbpd-impl/`.

**FIX-FLAG-IMPLEMENTS-NOTYET (2026-11-20, zuhu desktop, stacked on #535 fix/rust-flag-expression-evaluate 7f8fc7d):**
AI-assisted (Claude Code, Opus 5.5). In Off, `FlagExpressionEvaluator.Evaluate` with a context refuses an unloaded
`IRuleEngineOwner` or `IFlagProvider` class as `NotYet { NotRead }` before any write, instead of reading "no".
`cargo build --workspace -j 2` clean; `cargo test --workspace -j 2` 2,460 passed / 0 failed / 46 ignored (1 new test in
`ow-skills`); clippy flags nothing in ow-skills. Slice not probed in this lane. ctest and verify_packages not run (no C++
change). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-175.

**IMPL-GET-FUNC-NAME (2026-11-19, pc-hummer, on origin/main d52bcc79):** AI-assisted (Claude Code, Opus 5.5).
`Object.GetFuncName()` in Off: the innermost running script function's name; `NotRead` with no script frame.

Results:
- `cargo test -p ow-vm`, round 1: 208 passed / 0 failed / 1 ignored (3 new). Review r1 of #564 added 2 tests (state
  functions, state code and unknown frames; the binding from bytecode and a native calling back): 210 passed / 0
  failed / 1 ignored. Review r2 of #564 (state code only through the interpreter's flag, built by
  `declare_state_code`; the `:` split covered; tests changed, not added): 210 passed / 0 failed / 1 ignored.
- `cargo test --workspace -j 2`, round 1: 2,447 passed / 0 failed / 46 ignored; clippy clean in the new files;
  `crate_edges_test.py` OK.
- Installed Off `--slice-gate` strict (a local merge with #535, #548, #555, #560, #523 and the link 9 spawn branches,
  never pushed; under the machine lock): 1 ok / 3 NotYet / 10 errors. Link 9 passes `spawn` and `cast` (2 of 2); the
  cast returns, graded error only for the unchosen evidence check. Without this binding the same merge stops at the
  unbound `Object.GetFuncName()`.

ctest and verify_packages were not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-178.

**IMPL-LINK9-TARGET-MIND (2026-11-19, pc-hummer, on #568 @ 350fb829 over origin/main d52bcc79):** AI-assisted (Claude
Code, Opus 5.5). Link 9's target gets a mind (a harness input, UNVERIFIED): `WillowGame.WillowMind` or the target's
`ControllerClass`, spawned by the world and linked as `adopt` links an AI pawn, after all four links are checked. Link
9's set-up resolves and reports the classes the harness needs (`IFlagProvider` among them), then registers the pawn
`QueryInterface` rules. Reviews r1 and r2 of #579 are applied (the fallback mind is resolved at set-up).

Results:
- `cargo test -p ow-cli`: 124 + 2 passed / 0 failed / 1 ignored (8 new).
- `cargo test --workspace -j 2`: 2,453 passed / 0 failed / 46 ignored; `crate_edges_test.py` OK;
  `check_slice_run_off_test.py` OK; clippy has no hits in the touched files.
- Installed Off `--slice-gate` strict (a local merge with #535, #564, #548, #555, #560, #523 and the link 9 spawn
  branches, never pushed; under the machine lock): 1 ok / 4 NotYet / 9 errors. Without the lane's harness steps, the
  same merge gives 1 ok / 3 NotYet / 10 errors. Link 9's `spawn` completed, and its `cast` (recorded before it
  runs) reached the registered `GearboxMind` rule, which answers NotYet: `GearboxMind`'s own `QueryInterface` answer
  for `IAIInterface` is not read (NATIVE_PAWN_AI_INTERFACE.md). So link 9 moves from error (no evidence) to NotYet.
  The other rows are unchanged. B was re-run after review r1, with the same result.

ctest and verify_packages were not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-183 (link 9 target mind).

**LINK9-EVIDENCE-CHECK (2026-11-19, pc-hummer, on origin/main d52bcc79):** AI-assisted (Claude Code, Opus 5.5). Link
9's evidence: after the cast, the Phaselock skill's `LiftedPawn` is the target and `CurrentState` is `EPLS_Intro` (the
harness's choice, UNVERIFIED in game).

Results:
- `cargo test -p ow-cli`: 115 + 2 passed / 0 failed / 1 ignored (1 new, 1 changed).
- `cargo test --workspace -j 2`: 2,445 passed / 0 failed / 46 ignored; `crate_edges_test.py` OK.
- Installed Off `--slice-gate` strict (a local merge with #564, #535, #548, #555, #560, #523 and the link 9 spawn
  branches, never pushed; under the machine lock): 1 ok / 3 NotYet / 10 errors. Link 9 passes `spawn` and `cast` but
  is not ok: no evidence, because the cast takes the blocked path: `CanLiftTargetIf` answers false for a target the
  harness spawned with no mind (`IsPhaselocked` reads 0; corrected after FIX-ISPHASELOCKED-UNLOCKED).

ctest and verify_packages were not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-182.

**IMPL-SKELETAL-MESH-LAYER (2026-11-19, pc-hummer, on main d52bcc79):** AI-assisted (Claude Code, Opus 5.5). The
read-only skeletal mesh data layer (`ow-world` `world::skeletal`: reference skeleton from a harness-installed source,
`ow-cli`'s over ow-assets' ActorX reader; sockets from the store's objects). `cargo build --workspace -j 2` clean;
`cargo test --workspace -j 2` 2,449 passed / 0 failed / 47 ignored (ow-world skeletal 5; one new ignored installed
probe); clippy no new warnings; `crate_edges_test.py` OK. Installed probe (UModel 1590 export of Maya's body mesh,
counts only): 27 bones, parse pass; 13 socket objects unloaded after the mesh's load (refused, nothing loaded), all 13
read after a harness load. No slice-gate change (nothing installs the source). ctest and verify_packages not run (no C++
change). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-184; probe output under ignored
`local/analysis/skel/`.

**IMPL-SET-BASE-VALUE (2026-11-19, pc-hummer, on main d52bcc79, after review r1 of #572):** AI-assisted (Claude Code,
Opus 5.5). `AttributeInitializationDefinition.SetBaseValue` in Off from NATIVE_ATTRIBUTES.md section 1, with the
write-free, load-free preflight of review r1 and the hub's harness preload of the destination attribute.
- `cargo build --workspace` clean.
- `cargo test --workspace -j 2`: 2,455 passed / 0 failed / 46 ignored (9 new in `ow-world`).
- clippy flags nothing in the changed lines; `crate_edges_test.py` OK.
- `ctest --test-dir build -C Release --output-on-failure`: 164 of 166 passed (fresh configure and Release build in
  this worktree). The 2 failures are local-environment blockers, not this change:
  - `py-real_game_capture_safety`: C: had 1.2 GB free, 2.3 GB needed;
  - `py-real_game_pending`: its path check, with the worktree under the user's Temp folder.
- `python tools/verify_packages.py --reader build/Release/ow-package.exe`: 9 packages, all class bodies exact.
- Installed Off `--slice-gate` strict, on a local, unpushed merge with #558, #534 and #552, plus throwaway stand-ins
  for `GetParticleEffect` and `GetExpLevelFromManufacturerData`: 1 ok / 4 notyet / 9 error. The harness preloads 3
  definitions (the threshold attribute and its two resolvers), logged as UNVERIFIED harness inputs. Link 8 is 1 of 5
  at `init`: past `SetBaseValue`, stopped at the unbound `WillowWeapon.StaticGetWeaponDamageType(weapondefinitiondata)`.
- No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-187.

**IMPL-STATIC-GET-WEAPON-DAMAGE-TYPE (2026-11-19, pc-hummer, on main d52bcc79):** AI-assisted (Claude Code, Opus
5.5). `WillowWeapon.StaticGetWeaponDamageType` in Off from NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md (#576 at 3225f887).
`cargo test -p ow-inventory` 187 passed / 0 failed; `cargo test --workspace --locked -j 2` 2,451 passed / 0 failed / 46
ignored; `cargo clippy -p ow-inventory --all-targets` adds no warning in the touched files; `crate_edges_test.py` OK.
Installed Off `--slice-gate` (strict, under the lock) on a throwaway merge with #572: ok 1 / NotYet 4 / error 9 of 14.
Link 8 passes 1 of 5 steps and stops at `init` on the unbound `WillowWeapon.CacheWeaponCard()`, before this native
(#573, #534 and #552 conflict with #572 and were left out). ctest and verify_packages not run (no C++ change). No
real-game run; `UNVERIFIED` in game. Decision 2026-11-19-192.

**IMPL-GET-EXP-LEVEL (2026-11-19, pc-hummer, on main d52bcc79):** AI-assisted (Claude Code, Opus 5.5).
`InventoryBalanceDefinition.GetExpLevelFromManufacturerData` in Off from the accepted
NATIVE_GET_EXP_LEVEL_FROM_MANUFACTURER_DATA.md (#567 at 11798cc1): one rule shared with the loot roll, non-loading
reads; the equip harness sets `ManufacturerDefinition` and `ManufacturerGradeIndex` (UNVERIFIED inputs). `cargo build
--workspace -j 2` clean; `cargo test --workspace -j 2` 2,450 passed / 0 failed / 46 ignored (ow-combat 193, exp_level
6); clippy no new warnings; `crate_edges_test.py` OK. Installed Off `--slice-gate` strict (release build, under the
machine lock) on a throwaway local merge with #534, #552, #558 and the particle implementation, never pushed: 1 ok / 5
NotYet / 8 errors. Link 8 runs `make` and stops in `init` at the unread `DamageTypeAttributeValueResolver` (marked),
before this native (A/B without its registration: the same stop). ctest and verify_packages not run (no C++ change).
No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-196; logs under ignored `local/analysis/gel-impl/`.

**LINKS-6-8-NEXT (2026-11-19, pc-hummer, rebased onto main a0d40f5c after #476 landed):** AI-assisted (Claude Code, Opus
5.5). `DroppedPickup.GetPickupableInventory` and `GetPickupableInventoryDefinition` in Off from
NATIVE_INVENTORY_EQUIP.md. `cargo build --workspace --locked -j 4` clean; `cargo test --workspace --locked -j 4` 2,322
passed / 0 failed / 46 ignored; `crate_edges_test.py` 19 OK. Installed Off `--slice-gate` on a local integration of main
a629f56f, #455, #462, #468, -97 and #476 with this change: links 6 and 7 stay in `make` (0 of 3 and 0 of 4 driver steps)
but get past the accessor to the unbound `SkeletalMeshComponent.SetAnimTreeTemplate(object)`; link 8 is 1 of 5 at
`init`, the unbound `WillowWeapon.CacheWeaponCard()`. ctest and verify_packages not run (no C++ change). No real-game
run; `UNVERIFIED` in game. Decision 2026-11-19-40 (assigned -103); logs under ignored `local/p2/links68/`.

**IMPL-LINK10-NEXT (2026-11-19, pc-hummer, on #496 9b591ada; gate on a local merge with #497 7af3ab76):** AI-assisted
(Claude Code, Opus 5.5). The accept screen's calls in Off from NATIVE_MISSION_TEXT_LIST.md (#504 cb42b97a) and the
hub's rulings R1 to R3: `UpdateMissionTextList` nothing with a None list; `GetVariableObject` None; `FocusOn(None)`
writes `FocusObject` None, then `FocusOffset` the zero vector; `RecordMissionStatusChangedEventForPlayer` nothing. `cargo build
--workspace --locked -j 4` clean; `cargo test --workspace --locked -j 4` 2,382 passed / 0 failed / 46 ignored;
`crate_edges_test.py` OK. Installed Off `--slice-gate` strict on the local merge: 0 ok / 5 NotYet / 9 errors before,
1 / 4 / 9 after. Link 10 passes (8 of 8 steps); link 11 reaches 4 of 8 and stops at `hit:fire` on a pawn's
`QueryInterface(IAIInterface)` routed to its `SkeletalMeshComponent` (NotYet); link 12 is blocked behind it (0 of 6).
ctest and verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-121
(placeholder); logs under ignored `local/p2/link10-next/`.

**IMPL-REFRESH-BALANCE-DATA (2026-11-19, pc-hummer, on #554 6571aabb):** AI-assisted (Claude Code, Opus 5.5).
`WillowPlayerController.RefreshBalanceDataFromMissionCompletion` in Off from the accepted
NATIVE_REFRESH_BALANCE_DATA_FROM_MISSION_COMPLETION.md (#559 at be877676), with the harness preload of the region
balance definitions. `cargo build --workspace -j 2` clean; `cargo test --workspace -j 2` 2,395 passed / 0 failed / 46
ignored (ow-mission 231); clippy no new warnings; `crate_edges_test.py` OK. Installed Off `--slice-gate` strict
(release build, under the machine lock) on a throwaway local merge with #546, #533, #497, #509, #512, #524 and #526,
never pushed: 4 ok / 1 NotYet / 9 errors; **link 12 ok, 6 of 6 steps**. ctest and verify_packages not run (no C++
change). No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-179; logs under ignored `local/analysis/rbd-impl/`.

**IMPL-ALL-EXPANSION-SIDE-MISSIONS (2026-11-19, pc-hummer, on fix/rust-mission-list-build 9b591ada, #496):**
AI-assisted (Claude Code, Opus 5.5). `MissionTracker.AllExpansionSideMissionsComplete` in Off from #550's note: the
list follows `GetAllMissions` (refusal replayed, `LayerAbsent` while unbuilt), then the side-mission walk.

Results:
- `cargo test -p ow-mission`: 222 passed / 0 failed (7 new rule tests, 1 new bridge test, 1 changed).
- `cargo test --workspace`: 2,386 passed / 0 failed / 46 ignored; `crate_edges_test.py` OK.
- Installed Off `--slice-gate` strict (release build; a local merge with #546, #533, #497, #509, #512, #524 and #526,
  never pushed; under the machine lock): 3 ok / 1 NotYet / 10 errors.
  - Links 10 and 11 ok; link 12 passes 1 of 6 driver steps (`use`) and stops in `turnin` at the unbound
    `WillowPlayerController.RefreshBalanceDataFromMissionCompletion(object)`, past this native.

ctest and verify_packages were not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-165.

**IMPL-MISSIONLIST-BUILD (2026-11-19, pc-hummer, on the note #493 r1 5b880555, reviews r1 and r2 of #496 folded in):**
AI-assisted (Claude Code, Opus 5.5). The tracker's MissionList build and the mission numbering in Off from
NATIVE_MISSION_LIST_BUILD.md and the hub's rulings. `cargo build --workspace --locked -j 4` clean; `cargo test --workspace
--locked -j 4` 2,379 passed / 0 failed / 46 ignored (10 tests in `mission_list_tests.rs`, 1 new and 1 changed in the
bridge). Installed Off `--slice-gate` strict (release build, under the machine lock): 0 ok / 5 NotYet / 9 errors; links
10 to 12 pass 3 of 4 driver steps and stop in `accept` at "stopped after partial writes: the controller's
current-playthrough virtual" (links 11 and 12 blocked behind link 10). ctest and verify_packages not run (no C++ change).
No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-108 (number pending); logs under ignored `local/p2/mlist/`.

**IMPL-CARD-SLOT-COUNT (2026-11-19, desktop, on #521 at 83d4e90):** AI-assisted (Claude Code, Claude Opus 5.5). The
card's slot count is the weapon type's clamped `AttributeSlotMaxActivated`, and slots 0 to count - 1 run one step; an
unloaded weapon type is `NotRead`. `cargo test --workspace -j 2` 2,489 passed / 0 failed / 46 ignored (33 in
`equip::card::tests`); logs `local/p2/sc2/`. Installed Off `--slice-gate` strict, under the lock: 0 ok / 5 NotYet / 9
errors. Link 8 `error`, 1 of 5 steps, stop `init` at the unbound native `WillowWeapon.RecomputeAttributeBaseValues()`
(before: `notyet` at the card's slot count). The other 13 rows are identical.

**IMPL-CACHE-WEAPON-CARD-BODY r2 (2026-11-19, desktop, on #521 at d479a55):** AI-assisted (Claude Code, Claude Opus
5.5). #518's three write counters (copied unchanged from 211484a) mark the card's refusals, random draws included.
The preflight's globals check loads nothing, the setup resolves the name-part ancestry, and `equip_world` runs the
setup before the world exists. `cargo test --workspace -j 2`: 2,486 passed / 0 failed / 46 ignored (30 in
`equip::card::tests`). Logs are in `local/p2/cwcb-r2/`. Installed Off `--slice-gate` strict, under the lock: 0 ok /
6 NotYet / 8 errors. Link 8 is unchanged at 1 of 5 (stop `init` at the card's slot count), and the other rows match
r1.

**IMPL-CACHE-WEAPON-CARD-BODY r1 (2026-11-19, desktop, on #506 at 848ffc6):** AI-assisted (Claude Code, Claude Opus
5.5). The preflight is read-only, with an explicit card setup; the slot count comes first; loads are tracked; base values
are read; first-wins binding; `Role` is checked as an enum. `cargo test --workspace -j 2` 2,481 passed / 0 failed / 46
ignored (27 in `equip::card::tests`); logs in `local/p2/cwcb-r1/`. Installed Off `--slice-gate` strict, under the lock:
0 ok / 6 NotYet / 8 errors, link 8 unchanged at 1 of 5, stop `init` at the card's slot count.

**IMPL-CACHE-WEAPON-CARD-BODY (2026-11-19, desktop, on #506 at 848ffc6):** AI-assisted (Claude Code, Claude Opus 5.5).
`CacheWeaponCard`'s body in Off (`ow-inventory` `equip::card`) and link 8's globals world. `cargo test --workspace -j
2` 2,475 passed / 0 failed / 46 ignored (21 new in `equip::card::tests`); clippy flags nothing in the changed files;
logs `local/p2/cwcb/`. Installed Off `--slice-gate` strict, under the machine run lock: 0 ok / 6 NotYet / 8 errors.
Link 8 `notyet`, 1 of 5 steps, stop `init` at "stopped after partial writes: WillowWeapon.CacheWeaponCard: the card's
slot count" (before: 0 of 5, stop `make`). The other 13 rows are identical to the before run.

**IMPL-CACHE-WEAPON-CARD r2 (2026-11-19, desktop, on main 7668d07):** AI-assisted (Claude Code, Claude Opus 5.5).
Link 8's `make` keeps its typed error and is marked only after the weapon's allocation; struct own names by
`ObjectPath::name()`. `cargo test -j 2 -p ow-inventory -p ow-cli`: ow-inventory 196 passed / 0 failed, ow-cli 115
passed / 1 ignored and 2 passed (3 new tests); log `local/p2/cwc-r2/test.log`. No installed gate re-run: the preflight
still stops `make` before any write, so link 8's row is the r1 row's.

**IMPL-CACHE-WEAPON-CARD r1 (2026-11-19, desktop, on main 7668d07):** AI-assisted (Claude Code, Claude Opus 5.5).
`WillowWeapon.ChooseRandomNameParts` in full and `CacheWeaponCard`'s `NotRead` in Off (`ow-inventory`
`equip::name_parts`), and link 8's preflight before the weapon is made. `cargo build --workspace -j 2` clean; `cargo
test --workspace -j 2` 2,453 passed / 0 failed / 46 ignored (15 new in `equip::name_parts::tests`); clippy flags
nothing in the changed files. The natives return unprefixed `NotYet`s; link 8's driver marks phase errors after
`make` (review r1). Installed Off `--slice-gate` strict, under the machine run lock: 0 ok / 6 NotYet / 8 errors. Link
8 `notyet`, 0 of 5 steps, stop `make` at the unprefixed `CacheWeaponCard` `NotRead`. The same build with the preflight
call disabled reaches `init` and stops at the call-time `CacheWeaponCard` `NotRead`, prefixed "stopped after partial
writes" by the driver (1 of 5 steps); the other 13 rows are identical in the two runs and come from main. ctest and
verify_packages not run (no C++ change). `UNVERIFIED` in game. Decision 2026-11-19-118 (placeholder); logs under
ignored `local/p2/cwc-r1/`.

**LINK-1-NEXT-STOP (2026-11-19, pc-hummer, rebased onto fix/rust-init-player-skill-tree 07017c6d, #475 r1):**
AI-assisted (Claude Code, Opus 5.5). The recorded resource pool idle-reset hooks are matched by their installed package
(`AmmoResourcePool`, `ExperienceResourcePool` and `ShieldResourcePool` are `WillowGame` classes). `cargo build
--workspace --locked -j 4` clean; `cargo test --workspace --locked -j 4` 2,204 passed / 0 failed / 46 ignored;
`crate_edges_test.py` 19 OK. Installed Off `--slice-gate` strict, on a local merge of this branch with main and #458
(9e880cc6): 0 ok / 6 NotYet / 8 errors; links 1, 3, 4, 5, 13 and 14 get past the pool hook in `maya:8` and stop at the
unbound native `GameEngine.GetOnlineSubsystem()` (graded error; NATIVE_LOGIN_STATUS.md, #474). Without #458 they stop
earlier, at `BlackMarketUpgradeAttributeValueResolver`. ctest and verify_packages not run (no C++ change). No real-game
run; `UNVERIFIED` in game. Decision 2026-11-19-98; logs under ignored `local/p2/link1/`.

**FIX-483-FOLLOWUP (2026-11-19, pc-hummer, on main 7668d075):** AI-assisted (Claude Code, Opus 5.5). The hub's F1-F6
follow-up of #483 (`ApplyInternalSlotEffectModifiers` in Off): a write-free preflight (the noun probe validates
`DefaultValue` without evaluating it; a replicated slot refused only for an operation that rewrites the entry), P0 before
the independent checks, P1's out check, records appended to the live out argument, and the W1 mark only after an
actual write (after review r2 of #518: three monotonic counters, committed heap writes, draws and load transitions).
`cargo build --workspace --locked -j 4` clean; `cargo test --workspace --locked -j 4` 2,461 passed / 0 failed / 46 ignored
(r2); `crate_edges_test.py` OK. Installed Off `--slice-gate` strict (release build,
under the machine lock): 0 ok / 5 NotYet / 9 errors; link 8 1 of 2 driver steps, stop `WillowWeapon.CacheWeaponCard()`
in `init`. ctest and verify_packages not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-130; logs under ignored `local/p2/483-followup/`.

**SPAWN-COMPLETE part B (2026-11-19, pc-hummer, on main d52bcc79):** AI-assisted (Claude Code, Opus 5.5). An Off
spawn loads the unloaded definitions its new actor's own object properties point to, one level deep, before the actor
joins the level list.

Results:
- `cargo test -p ow-world`: 222 passed / 0 failed / 5 ignored, with 2 new tests.
- `cargo build --workspace`: clean.
- `cargo test --workspace`: 2,446 passed / 0 failed / 46 ignored.
- Installed Off `--slice-gate` strict (release build; #522 merged and `Engine.IsSplitScreen()` bound false, both
  locally only, uncommitted; under the machine lock): 1 ok / 10 NotYet / 3 errors.
  - The JSONL is byte-identical to the same build without this change.
  - Link 1 is 0 of 1 driver step, still at `InitializePawnMeshes`' lazy-`BodyClass` `NotRead`: the body class arrives
    through `BodySelectClass`'s script assignment, after the spawn.
- With the hub's option (a) (the harness stand-in for the streaming load also loads the archetype's same-package
  references): `cargo test -p ow-cli` 114 + 2 passed / 1 ignored; `cargo test --workspace` 2,447 passed / 0 failed /
  46 ignored.
  - The same installed gate gave 1 ok / 10 NotYet / 3 errors.
  - Link 1 is 0 of 1, now at `InitializePawnMeshes`' `LayerAbsent` (the body composition build), past the lazy
    `BodyClass`. Links 3, 4, 5, 13 and 14 stop at the same place.

- Review r1 of #528 (typed load failures, no leftover copies, the marker from #518's counters): `cargo test -p
  ow-world` 225 passed / 0 failed / 5 ignored; `cargo test -p ow-cli` 116 + 2 passed / 1 ignored; `cargo test
  --workspace` 2,454 passed / 0 failed / 46 ignored. The same installed gate gave 1 ok / 10 NotYet / 3 errors, link 1
  0 of 1 at the same `LayerAbsent`, links 3, 4, 5, 13 and 14 with it.
- Review r2 of #528 (the harness retries a cached failure; the unreachable copy cleanup removed): `cargo test -p
  ow-world` 225 / 0 / 5; `cargo test -p ow-cli` 116 + 2 / 0 / 1; `cargo test --workspace` 2,454 passed / 0 failed /
  46 ignored. The gate was not re-run (no change on the installed path: Maya's same-package loads succeed).

ctest and verify_packages were not run (no C++ change). No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-139.

**IMPL-MISSION-NUMBERS (2026-11-19, pc-hummer, stacked on fix/rust-settings-fill 223a2ced):** AI-assisted (Claude Code,
Opus 5.5). `GetActiveMissionNumber`, `GetActivePlotCriticalMissionNumber` and the mission screen's three calls in Off,
from NATIVE_MISSION_NUMBERS.md (#473 at 0a32108b), in the hub's conservative form: the engine-wide condition is not
guessed (both branches, `NotRead` when they differ), the playthrough virtual is `NotRead`, and the screen calls are a
marked `LayerAbsent`. `cargo build --workspace --locked
-j 4` clean; `cargo test --workspace --locked -j 4` 2,198 passed / 0 failed / 46 ignored (12 new tests in `ow-world`);
`crate_edges_test.py` 19 OK. Installed Off `--slice-gate` strict (release build): 0 ok / 14 NotYet / 0 errors. Links 10
to 12 pass 3 of 4 driver steps and stop in `accept` at "stopped after partial writes: MissionTracker.GetAllMissions",
the bridge's own `LayerAbsent`; the other links are unchanged. ctest and verify_packages not run (no C++ change). No
real-game run; every rule stays `UNVERIFIED` in game. Decision 2026-11-19-93; logs under ignored
`local/p2/mission-numbers/`.
**IMPL-MISSION-NUMBERS (2026-11-19, pc-hummer, rebased onto fix/rust-settings-fill 8ea5f6d5, #464 accepted):**
AI-assisted (Claude Code, Opus 5.5). `GetActiveMissionNumber`, `GetActivePlotCriticalMissionNumber` and the mission
screen's three calls in Off, from NATIVE_MISSION_NUMBERS.md (#473 at f586ab9c), in the hub's conservative form: the
engine-wide condition is not guessed (both branches, `NotRead` when they differ), the playthrough virtual is `NotRead`,
and the screen calls are a marked `LayerAbsent`. `cargo build --workspace --locked -j 4` clean; `cargo test --workspace
--locked -j 4` 2,254 passed / 0 failed / 46 ignored after the rebase (12 new tests in `ow-world`); `crate_edges_test.py`
19 OK. Installed Off `--slice-gate` strict (release build): 0 ok / 12 NotYet / 2 errors (links 6 and 7 at the unbound
`GetInventoryDefinition`, as on main). Links 10 to 12 pass 3 of 4 driver steps and stop in `accept` at "stopped after
partial writes: MissionTracker.GetAllMissions", the bridge's own `LayerAbsent`; the other links are unchanged. ctest and
verify_packages not run (no C++ change). No real-game run; every rule stays `UNVERIFIED` in game. Decision
2026-11-19-93; logs under ignored `local/p2/mission-numbers/`.
**IMPL-MISSION-NUMBERS (2026-11-19, pc-hummer, rebased onto main d3d76fed after #464 landed; review r1 of #480 folded
in):** AI-assisted (Claude Code, Opus 5.5). `GetActiveMissionNumber`, `GetActivePlotCriticalMissionNumber` and the
mission screen's three calls in Off, from NATIVE_MISSION_NUMBERS.md (#473 at f586ab9c), in the hub's conservative form:
the engine-wide condition is not guessed (both branches, `NotRead` when they differ), the playthrough virtual is
`NotRead`, and the screen calls are a marked `LayerAbsent`. `cargo build --workspace --locked -j 4` clean; `cargo test
--workspace --locked -j 4` 2,284 passed / 0 failed / 46 ignored (15 new tests in `ow-world`, 1 in `ow-mission`);
`crate_edges_test.py` 19 OK. Installed Off `--slice-gate` strict (release build): 0 ok / 12 NotYet / 2 errors (links 6
and 7 at the unbound `GetInventoryDefinition`, as on main). Links 10 to 12 pass 3 of 4 driver steps and stop in `accept`
at "stopped after partial writes: MissionTracker.GetAllMissions", the bridge's own `LayerAbsent`; the other links are
unchanged. ctest and verify_packages not run (no C++ change). No real-game run; every rule stays `UNVERIFIED` in game.
Decision 2026-11-19-93; logs under ignored `local/p2/mission-numbers/`.
**IMPL-MISSION-NUMBERS (2026-11-19, pc-hummer, rebased onto main d3d76fed after #464 landed; reviews r1 and r2 of #480
folded in):** AI-assisted (Claude Code, Opus 5.5). `GetActiveMissionNumber`, `GetActivePlotCriticalMissionNumber` and
the mission screen's three calls in Off, from NATIVE_MISSION_NUMBERS.md (#473 at f586ab9c), in the hub's conservative
form: the engine-wide condition is not guessed (both branches, `NotRead` when they differ), the playthrough virtual is
`NotRead`, and the screen calls are a named `LayerAbsent`, marked at the bridge's status hook (review r2 of #480).
`cargo build --workspace --locked -j 4` clean; `cargo test --workspace --locked -j 4` 2,286 passed / 0 failed / 46
ignored (15 new tests in `ow-world`, 2 in `ow-mission`); `crate_edges_test.py` 19 OK. Installed Off `--slice-gate`
strict (release build): 0 ok / 12 NotYet / 2 errors (links 6 and 7 at the unbound `GetInventoryDefinition`, as on main).
Links 10 to 12 pass 3 of 4 driver steps and stop in `accept` at "stopped after partial writes:
MissionTracker.GetAllMissions", the bridge's own `LayerAbsent`; the other links are unchanged. ctest and verify_packages
not run (no C++ change). No real-game run; every rule stays `UNVERIFIED` in game. Decision 2026-11-19-93; logs under
ignored `local/p2/mission-numbers/`.

**INHERITED-COMPONENT-INSTANCING r1 (2026-11-19, desktop, stacked on #470 r2, main 0235937):** AI-assisted (Claude
Code, Claude Opus 5.5). Review r1 of #478: a unique stand-in, its export identity checked, an object inside an
inherited component refused, the Off commit boundary after registration, and the driver's reads typed and ordered.
`cargo build --workspace -j 2` clean; `cargo test --workspace -j 2` 2,310 passed / 0 failed / 46 ignored (3 new in
`ow-world`); clippy flags nothing new in the changed files. Installed Off `--slice-gate` strict, under the machine run
lock: link 2 **`ok`** 4/4. Every other row is byte-identical to #470 r2's run on the same main: 1 ok / 11 notyet / 2
errors. Rows 6 and 7 come from main's pickup work. Decision 2026-11-19-91; logs under ignored `local/p2/inh-r1/`.

**INHERITED-COMPONENT-INSTANCING (2026-11-19, desktop, stacked on #470 6268e6d):** AI-assisted (Claude Code, Claude
Opus 5.5). A spawn's template slot naming a parent class default's component stands for the template's own
sub-object of that export archetype, which is copied per actor; link 2's driver spawns from the store's holder of the
dummy archetype and checks that the dummy owns its components. `cargo build --workspace -j 2` clean; `cargo test
--workspace -j 2` 2,190 passed / 0 failed / 46 ignored (2 new in `ow-world`); clippy flags nothing new in the changed
files; `crate_edges_test.py` 19 OK; `check_slice_run_off_test.py` OK; decisions check OK (1 added). Installed Off
`--slice-gate` strict, under the machine run lock: link 2 `notyet` 3/4 before, **`ok` 4/4** after; link 3 `notyet`, 0
steps, stop `maya:8`, unchanged; the other rows byte-identical; 1 ok / 13 NotYet / 0 errors. ctest and verify_packages
not run (no C++ change). `UNVERIFIED` in game. Decision 2026-11-19-91; logs under ignored `local/p2/inherited/`.

**IMPL-LINK11-QUERY-INTERFACE (2026-11-19, desktop, on #512 17b9c33):** AI-assisted (Claude Code, Claude Opus 5.5).
A component's `QueryInterface` answer in Off, for exactly `ActorComponent` and `SkeletalMeshComponent` (NATIVE_PAWN_AI_INTERFACE.md
section 6.4). `cargo build --workspace -j 4` clean; `cargo test --workspace -j 4` 2,445 passed / 0 failed / 46 ignored
(8 new or reworked component tests in `ow-combat` `pawn_interface`); `crate_edges_test.py` 19 OK. Installed Off
`--slice-gate` strict under the run lock: 0 ok / 5 notyet / 9 errors on this base (links 10 to 12 at `GetAllMissions`, no #496/#497); a local probe merged with #496 and #497: 1 / 4 / 9, link 10 ok 8/8, link 11 4 of 8 steps (as before), stop `hit:fire` now at the rule's unloaded-handle `NotRead` on the dummy's archetype mesh `SkeletalMeshComponent_333`; logs under ignored `local/p2/impl-qi/`. ctest and verify_packages not run (no C++ change). No real-game run; every
rule stays `UNVERIFIED` in game. Decision 2026-11-19-135.

**LINK-2-FIGHT-AREA r2 (2026-11-19, desktop, on main 0235937):** AI-assisted (Claude Code, Claude Opus 5.5). Review r2
of #470: the cylinder read from the package before any write, a grounded reach walk, the partial-writes prefix after the
first write, typed world setup. `cargo build -p ow-cli -j 2` clean; `cargo test -p ow-cli -j 2` 110 passed / 0 failed / 1
ignored (4 new in `fight_area::tests`, 1 reworded); clippy on ow-cli clean; `check_slice_run_off_test.py` OK. Installed
Off `--slice-gate` strict, under the machine run lock, rebased onto main 0235937: 0 ok / 12 notyet / 2 errors;
link 2 3 of 4 steps, stop `fight_area` at the inherited-component `NotYet`, now prefixed "stopped after partial
writes". Rows 6 and 7 are errors at `make` (the unbound `WillowGlobals.GetWillowGlobals`), from main's pickup work,
not this branch; the other rows come from main. Decision 2026-11-19-76; logs under ignored `local/p2/fa-r2/`.

**LINK-2-FIGHT-AREA (2026-11-19, desktop, on main 203c1aa):** AI-assisted (Claude Code, Claude Opus 5.5). Link 2's
`fight_area` step checks Maya's walked end against a labelled port fixture area, then places the target dummy there
through `ow_world`'s spawn path. `cargo build --workspace -j 2` clean; `cargo test --workspace -j 2` 2,179 passed / 0
failed / 46 ignored (5 new in `fight_area::tests`, 1 reworked link 2 test); clippy flags nothing in the changed files;
`crate_edges_test.py` 19 OK; `check_slice_run_off_test.py` OK (1 new test); `check_slice_gate_test.py` OK; decisions
check OK (1 added). Installed Off `--slice-gate` strict, under the machine run lock: link 2 `notyet`, 3 of 4 steps
passed, stop `fight_area`, before and after. Before, the stop was "needs a fight area placed in the map world"; after,
Maya's walk ends 9.5 uu from the area's centre (inside its 200 uu radius), and the dummy's spawn stops at decision
2026-11-19-44's inherited-component `NotYet` (`SpecialMoveComponent0` of `Default__WillowAIPawn`). The other 13 rows
are byte-identical; 0 ok / 14 NotYet / 0 errors before and after. ctest and verify_packages not run (no C++ change). A
fixture, not parity: `UNVERIFIED` in game. Decision 2026-11-19-76; logs under ignored `local/p2/fight-area/`. **Review r1 of #470** (on aaa83b7): typed grading, the placement checked against the loaded Sanctuary collision
(floor, solid, same floor, straight reach), the vector built before the template, and the checker's exact steps and
driver. `cargo test --workspace -j 2` 2,188 passed / 0 failed / 46 ignored; clippy flags nothing in slice_gate;
`crate_edges_test.py` 19 OK; `check_slice_run_off_test.py` 16 OK. Installed Off `--slice-gate`: link 2 `notyet`, 3 of 4
steps, stop `fight_area` at decision 44's inherited-component `NotYet`, reached after the floor and reach checks
pass; 0 / 14 / 0.

**IMPL-PAWN-AI-INTERFACE (2026-11-19, desktop, on main bb46e11):** AI-assisted (Claude Code, Claude Opus 5.5).
`Pawn`'s `QueryInterface` rule for `IAIInterface` in Off (a per-class rule registry in `ow-vm`, `ow-combat`
`pawn_interface`); the open answers are named `NotYet`s; Reference unchanged. `cargo build --workspace` clean; `cargo
test --workspace` 2,099 passed / 0 failed / 45 ignored; ctest at `-j 2` 173/173, 0 failed, 538.78 s; verify_packages 9
packages, all class bodies exact; `crate_edges_test.py` 19 OK. Installed, Off: `--slice-gate` strict 0/14/0,
byte-identical before and after (no row reaches `Behavior_AIHold`; link 11 stops at `accept`, `GetDataStoreClient`);
`--mission-run` Fire unchanged. **Rebased onto main 74c7b0a** (`ow-vm` `api.rs` kept both sides): `cargo build
--workspace` clean; `cargo test -p ow-vm -p ow-combat -p ow-ai -p ow-mission -p ow-cli` 753 passed / 0 failed / 7
ignored. **Review r1 of #456:** `cargo test --workspace` 2,167 passed / 0 failed / 46 ignored; clippy flags nothing in
the changed files; `crate_edges_test.py` OK. Installed Off `--slice-gate` strict 0/14/0; no row names a QueryInterface
rule (links 10 to 12 at `accept`, `GetDataStoreClient`; the combat links at `maya:8`). Decision 2026-11-19-62; logs
under ignored `local/p2/pawn-ai/`.
**IMPL-BLACKMARKET-RESOLVER (2026-11-19, pc-hummer, on main 74c7b0ac with #451's note stacked):** AI-assisted
(Claude Code, Opus 5.5). `WillowGame.BlackMarketUpgradeAttributeValueResolver` in Off from NATIVE_BLACKMARKET_UPGRADES.md
alone. `cargo build --workspace` clean; `cargo test --workspace` 2,169 passed / 0 failed / 46 ignored (14 resolver tests);
`crate_edges_test.py` 19 OK. Installed Off
`--slice-gate` strict: 0/14/0, byte-identical before and after on the same base. Links 1, 3, 4, 5, 13 and 14 stop at
`maya:8` on the stock `OnPlayerClassChange`'s unbound `WillowPlayerController.InitPlayerSkillTree` (#444), which runs
before the ammo maxima, so the resolver is not reached on main. No real-game run; `UNVERIFIED` in game. Decision
2026-11-19-72; logs under ignored `local/p2/bm-resolver/`.
**IMPL-INVENTORY-CONTEXT (2026-11-19, desktop, rebased onto main 9e53789):** AI-assisted (Claude Code, Opus 5.5).
`cargo build --workspace -j 2` clean; `cargo test --workspace -j 2` 2,128 passed / 0 failed / 45 ignored (6 new in `ow-attr`, 3 in
`ow-inventory`). `crate_edges_test.py` OK. ctest `-j 2` 173/173, 388.70 s. verify_packages 9 packages, all class bodies exact.
Installed Off, main and this branch each built: `--slice-gate` strict 0/14/0 before, 0/13/1 after (link 8 moves from the
resolver's `NotYet` at `init` to the unbound native `WillowInventory.InitializeAttributeSlots(bool)`; the other 13 rows identical).
No host or real-game run; the rules stay `UNVERIFIED` in game. Decision 2026-11-19-67.
Review r1 of #455 (same day, desktop): `cargo build --workspace -j 2` clean; `cargo test --workspace -j 2` 2,130 passed / 0 failed /
45 ignored (2 new `ItemActors` script tests in `ow-inventory`). `crate_edges_test.py` OK. ctest `-j 2` 173/173, 444.60 s. verify_packages
9 packages, all class bodies exact. Installed Off `--slice-gate` strict 0/13/1, unchanged (link 8 still stops at `InitializeAttributeSlots`).

**IMPL-SETTINGS-FILL (2026-11-19, pc-hummer, rebased onto main 7556c254, which has #448 and #450):** AI-assisted (Claude Code, Opus
5.5). The online game settings entry filled at store creation, `SetIntProperty`, `GetCurrPlaythrough` and `GetDLCManager`
(None) in Off, from NATIVE_SETTINGS_FILL.md. `cargo build --workspace --locked -j 4` clean; `cargo test --workspace
--locked -j 4` 2,178 passed / 0 failed / 46 ignored after the rebase (5 new data store tests in `ow-world`); `crate_edges_test.py` 19 OK.
Installed Off `--slice-gate` strict (release build and run under the machine lock): 0 ok / 14 NotYet / 0 errors; links
10 to 12 pass 3 of 4 driver steps and stop in `accept` at `GetActiveMissionNumber` ("stopped after partial writes"),
past the settings entry. ctest and verify_packages not run (no C++ change). No real-game run; every rule stays
`UNVERIFIED` in game. Decision 2026-11-19-80; logs under ignored `local/p2/settings-fill/`.
**IMPL-SPAWN-PLACEMENT (2026-11-19, pc-hummer, stacked on #454 5fb71e2b, the note at its final head 1a83fcbd):** AI-assisted (Claude Code, Opus 5.5). The
spawn's spot search and encroachment rules in Off from NATIVE_SPAWN_PLACEMENT.md alone (`ow-world` `placement.rs`,
a `SpawnCollision` seam). `cargo build --workspace` clean; `cargo test --workspace` 2,153 passed / 0 failed / 46
ignored (6 spawn and 7 search unit tests; every search query answers the trace-filter `NotYet` until it is read); `crate_edges_test.py` 19 OK. Installed Off `--slice-gate` strict 0/14/0. Link 9 is 0 of
the driver's 1 step: it still stops at `spawn` on "the spawn placement spot search", now this crate's `LayerAbsent`
for a world with no collision service, since the slice world installs none. The other 13 rows are byte-identical before
and after. No real-game run; `UNVERIFIED` in game. Decision 2026-11-19-79; logs under ignored
`local/p2/spawn-placement/`.
**IMPL-LOGIN-STATUS (2026-11-19, zuhu, on main ebccbca):** AI-assisted (Claude Code, Opus 5.5).

**IMPL-LOGIN-STATUS (2026-11-19, zuhu, rebased onto main 0235937):** AI-assisted (Claude Code, Opus 5.5).
`GameEngine.GetOnlineSubsystem` (exclusive answers; a missing engine object is `NotYet { LayerAbsent }`, a subsystem
no set-up initialised `NotYet { NotRead }`), `OnlineSubsystemSteamworks.GetLoginStatus` (section 4's rule with a
set-up record, otherwise `NotYet { NotRead }`) and the four natives after it (each a named `NotYet { NotRead }`) bound
by key in Off, from `NATIVE_LOGIN_STATUS.md` at a2d70e4; no preflight (open question 5). `cargo build --workspace`
clean; `cargo test --workspace` 2,187 passed / 0 failed / 46 ignored (7 `world::online` tests, 1 new `ow-combat` run
test). Installed Off `--slice-gate` strict 0/14/0, rows byte-identical before and after the four new bindings; link 1
still stops at `maya:8` on the unbound `InitPlayerSkillTree` (0 of 2 steps), so the login path is proved by tests until decision 2026-11-19-88 lands. **Review r1 of #479:** `cargo test --workspace`
2,191 passed / 0 failed / 46 ignored (11 `world::online` tests); installed Off `--slice-gate` strict 0/14/0, rows
byte-identical. Decision 2026-11-19-95; logs under ignored `local/p2/login-status/`.

**FIX-ATTR-GETVALUE-NATIVE (2026-11-19, pc-hummer, review r1 folded in, rebased onto main 7e80d7b6):** AI-assisted
(Claude Code, Opus 5.5). `AttributeDefinition.GetValue` bound as a script native in Off on `ow-attr`'s shared steps
(preflight, context, out write, value chain). After the rebase: `cargo test -p ow-combat -p ow-attr -p ow-inventory
--locked -j 4` 445 passed / 0 failed / 3 ignored (5 `GetValue` tests); clippy flags nothing in the changed files. Before
review r1, on main b985dcbf: `cargo test --workspace` 2,044 / 0 / 45 and `crate_edges_test.py` 19 OK. Installed Off
`--slice-gate` strict on this branch: 0/14/0, with links 1, 3, 4, 5, 13 and 14 at the player controller's starting-values
`NotYet` (#444 is not on main yet, so the native is not reached). On a local unpushed merge with
`fix/rust-class-starting-values` before review r1, the six links moved to the library's `NotYet` on
`WillowGame.BlackMarketUpgradeAttributeValueResolver`, still inside `maya:8`. No real-game run; `UNVERIFIED` in game.
Decision 2026-11-19-58; logs under ignored `local/p2/fix-attr-getvalue/`.
**FIX-CLASS-STARTING-VALUES (2026-11-19, pc-hummer, rebased onto main 7de9171a, which has #429, #431 and #434):**
AI-assisted (Claude Code, Opus 5.5). `ApplyCharacterClassStartingValues` (the `Controller` base and the
`WillowPlayerController` override) in Off from NATIVE_CLASS_STARTING_VALUES.md. `cargo build --workspace --locked -j 4`
clean; `cargo test --workspace --locked -j 4` 2,053 passed / 0 failed / 45 ignored (3 new, 1 updated in `ow-combat`);
`crate_edges_test.py` 19 OK. Installed Off `--slice-gate` strict: links 1, 3, 4, 5, 13 and 14 move from the override's
NotYet to an error at `maya:8`, the unbound script native `AttributeDefinition.GetValue(object,object,object)`; rows 10
to 12 stop at `UIInteraction.GetDataStoreClient` (`accept`); 0 ok / 8 NotYet / 6 errors. **Review r1:** `cargo test -p
ow-combat -p ow-world` 273 passed / 0 failed / 7 ignored, ow-cli 89 passed (combat digests unchanged); clippy flags
nothing in the changed files; installed Off `--slice-gate` 0/14/0, the six links now at the `InitPlayerSkillTree`
`NotYet { NotPorted }` before any delete. **Review r2:** ow-combat + ow-world 295 / 0 / 7, ow-cli 89 passed; after rebasing onto main c7f08814 (#442
landed), ow-combat + ow-world 301 / 0 / 7. ctest
and verify_packages not run (no C++ change). No host or real-game run; every rule stays `UNVERIFIED` in game. Decision
2026-11-19-57; logs under ignored `local/p2/fix-class-starting-values/`.

**FIX-PLAYER-CLASS-B (2026-11-19, desktop, on main df99b25):** AI-assisted (Claude Code, Claude Opus 5.5). The pawn
archetype preload (a harness input) and the player controller's starting values NotYet in Off; review r1 of #426 folded
in. `cargo build --workspace` clean; `cargo test --workspace` 2,017 passed / 0 failed / 45 ignored; ctest at `-j 2`
173/173, 0 failed, 535.32 s; verify_packages 9 packages, all class bodies exact. Installed, Off: `--combat-run maya:8`
and slice-gate links 1, 3, 4, 5, 13 and 14 move from `FindAlreadyLoadedObject` of `GD_Siren_Streaming.Pawn_Siren` to
`WillowPlayerController.ApplyCharacterClassStartingValues`'s NotYet (the unnamed script call of NATIVE_COMBAT_GROUNDWORK.md
1.10 step 2); both harness inputs are in the run's log; links 10 to 12 unchanged.

**FIX-ACTIVATE-EVENT (2026-11-19, pc-hummer, rebased onto main f64aeba9; installed runs on main cb01e96f):** AI-assisted (Claude Code, Opus 5.5).
`cargo build --workspace --locked -j 4` clean; `cargo test --workspace --locked -j 4` 2,021 passed / 0 failed / 45 ignored after the rebase
(3 new in `ow-ai`, 76/76 there). Release CLI build clean. ctest `-j 4` 173/173 passed, 113.12 s (C++ unchanged; existing
build). `crate_edges_test.py` 19 OK; Fire checker self-test passes and, under Reference with this branch's binary, 33 of 33
rules hold. Installed Off: `--slice-gate` strict 0/11/3 before, 0/14/0 after (link 10 moves from the `ActivateEvent` error
at `use` to the `GetDataStoreClient` `NotYet` at `accept`); `--slice-run` and `--mission-run` Fire unchanged. No host or
real-game run; the rule stays `UNVERIFIED` in game. Decision 2026-11-19-42; logs under ignored
`local/p2/fix-activate-event/`. **Reviews r1 to r3:** ow-ai 84/84 after r3 (10 ActivateEvent tests); r2: ow-cli 82/82;
`cargo test --workspace` 2,027 passed / 0 failed / 45 ignored; installed Off `--slice-gate` strict still 0/14/0, rows 10
to 12 unchanged at `GetDataStoreClient` (`accept`).

**FIX-PLAYER-CLASS (2026-11-19, desktop, on main e59bff0):** AI-assisted (Claude Code, Claude Opus 5.5).
`GBXDefinition.GetDefinition` and `LoadPlayerPawnDataAsync` in Off; the `maya` step's class select in Off.
`cargo build --workspace` clean; `cargo test --workspace` 2,010 passed / 0 failed / 45 ignored; ctest at `-j 2`
173/173, 0 failed, 357.40 s; verify_packages 9 packages, all class bodies exact. Installed, Off: `--combat-run
maya:8` and slice-gate links 1, 3, 4, 5, 13 and 14 move from `FindAlreadyLoadedObject` with an empty path to
`FindAlreadyLoadedObject` of `GD_Siren_Streaming.Pawn_Siren` (held by a package, not loaded); links 10 to 12
unchanged (`AIComponent.ActivateEvent`).

**FIX-POOL-SPAWN-B (2026-11-19, desktop, on main 79036dd):** AI-assisted (Claude Code, Claude Opus 5.5). The three
`WillowInventory` natives bound in Off; `GetItemTossVelocity` NotYet in Off; Reference unbound. `cargo build
--workspace` clean; `cargo test --workspace` 1,972 passed / 0 failed / 43 ignored (7 new `ow-inventory`, 7 new
`ow-combat` tests). C++ Release build clean; ctest at `-j 2` 173/173, 0 failed, 472.69 s; verify_packages 9 packages,
all class bodies exact. Installed, Off: `--slice-gate` 14 rows identical before and after (links 1, 3, 4, 5, 13 and 14
stop at the unbound `WillowGlobals.GetPlayerPawnDataManager()`); `--combat-run` `player:8 spawn:baby:6` stops the
spawn at `TargetableAttributeValueResolver`, before and after. No installed Off run reaches the death roll.
**TEST-FOLLOWUPS (2026-11-19, desktop, on main 0a23f13):** AI-assisted (Claude Code, Claude Opus 5.5). Tests only:
two review follow-ups (#407 pack order, #405 Force with a parked thread). `cargo build --workspace` clean;
`cargo test --workspace` 1,977 passed / 0 failed / 45 ignored; ctest at `-j 2` 173/173, 0 failed, 488.16 s;
verify_packages 9 packages, all class bodies exact.
**TEST-FOLLOWUPS (2026-11-19, desktop, on main 5554ead):** AI-assisted (Claude Code, Claude Opus 5.5). Tests only:
three review follow-ups (#407 pack order, #405 Force with a parked thread, #413 a script `AllParts` loop through the
engine iterator). `cargo build --workspace` clean; `cargo test --workspace` 1,987 passed / 0 failed / 45 ignored;
ctest at `-j 2` 173/173, 0 failed, 432.30 s; verify_packages 9 packages, all class bodies exact.

**FIX-PHASELOCK-CAST part B (2026-11-19, desktop):** AI-assisted (Claude Code, Claude Opus 5.5).
`ActionSkill.OnActionSkillActivated` and `OnActionSkillDeactivated` bound in Off (Reference unbound). `cargo build
--workspace` clean; `cargo test --workspace` 1,880 passed / 0 failed / 42 ignored before the rebase (four new
`ow-skills` tests), 1,889 / 0 / 42 after it (base f35ae69). C++ Release build clean; ctest at `-j 2` 173/173, 0 failed,
606.01 s before the rebase; after it 170/173 on a loaded machine, the 3 misses leak-gate Python suites timing out at
120 s (two passed on rerun; `py-leak_gate_policy` passes 19/19 run directly). verify_packages 9 packages, all class
bodies exact. Installed Phaselock test passes in both modes: in Off `OnActionSkillStarted` now stops at the unbound
`GearboxPawn.GetSMComponent()`; Reference still stops at `ActionSkill.OnActionSkillActivated`.

**FIX-PHASELOCK-CAST part A (2026-11-19, desktop):** AI-assisted (Claude Code, Claude Opus 5.5). `Object.VLerp` in
`ow-vm` (both modes), the two provider natives read into NATIVE_BEHAVIOR_PROVIDERS.md (not bound). `cargo build
--workspace` clean; `cargo test --workspace` 1,876 passed / 0 failed / 42 ignored (one new `ow-vm` test). Fresh C++
Release build (VS 18, `-T version=14.50`, `--parallel 2`) clean; ctest at `-j 2` 173/173, 0 failed, 466.46 s;
verify_packages 9 packages, decoded bytes, counts and export fields match, all class bodies exact. Installed
Phaselock test (ignored, `OW_BL2_ROOT`) passes in Off and Reference: the lift runs 43 frames, Z 0 to 200, never down;
`OnActionSkillStarted` stops at the unbound `ActionSkill.OnActionSkillActivated`.
**FIX-FIRE-TO-HIT (2026-11-19, pc-hummer, main 86c28d00 plus this lane):** AI-assisted (Claude Code, Opus 5.5).
`cargo build --workspace --locked -j 4` clean; `cargo test --workspace --locked -j 4` 1,882 passed / 0 failed / 42
ignored (7 new: 6 `ow-combat` shot tests, 1 `ow-world` replication-info fallback). Release CLI build clean.
`decisions_index.py --check` OK (1 added); `crate_edges_test.py` 19 tests OK; Fire checker and combat checker
self-tests pass. `verify_packages.py`: 9 packages, 3,339/3,339 class bodies exact with the C++ reader (same sources)
and with this branch's Rust reader. Fresh C++ Release build (`-T version=14.50`, `OW_REQUIRE_TEST_TOOLS=ON`, `--parallel 4`)
clean; `ctest -j 4` 173/173 passed, 0 failed, 126.12 s (the 7 Rust CTests included).
Installed, Off: `--combat-run` (W6-COMBAT's 15 steps) 0 rows on main, 13 after (spawns pass; hits stop at the
`WillowAIComponent` hold override; swings at unbound `WillowGameInfo.TravelPostCountdownInProcess`); Reference 29
rows, identical before and after. `--slice-run` Off byte-identical before and after (exit 3 at the Pawn
`IAIInterface` refusal, 12 steps; the checker cannot parse the partial report). New `weapon:` / `fire:` steps: Reference
4 shots land (18 rows); Off each shot stops at the same hold override. No host or real-game run; every rule stays
`UNVERIFIED` in game. Decision 2026-11-19-06; logs under ignored `local/p2/fix-fire-to-hit/`.

**Object.Localize path correction (2026-11-18, desktop):** AI-assisted (Codex/GPT-6.1 Sol).
The CLI now derives localization roots from the package store's absolute, lexically normalized root.
A child test process covers `--cooked .`, relative parent components and an absolute parent-component path
without changing the parent test process's working directory. The regression failed before the fix (0 passed /
1 failed) and passes within the workspace: 1,870 passed / 0 failed / 42 ignored. Workspace build and release
CLI build pass; Rust CTests 7/7, 0 failed, 62.58 s. Fresh Reference capture matches saved main exactly
(22/22 OK, 0 errors, 39 events); Off still exits 3 with the same partial output and Pawn IAIInterface refusal
(12 emitted OK steps, 14 events; final errors array unavailable). Earlier C++/Python and Fire evidence is
reused, not rerun. Logs are under ignored `local/p2/fix-localize-off/review-r2-*`. No host or real-game run;
native/gameplay parity remains `UNVERIFIED`.

**Object.Localize INT / Off (2026-11-17, desktop; operations recovery 2026-11-18):** AI-assisted
(Codex/GPT-6.1 Sol). Workspace build and release CLI build pass (`--locked -j 2`); workspace tests
1,870 passed / 0 failed / 42 ignored. Focused tests: 9 passed / 0 failed / 1 installed test ignored;
the first run's one path-separator assertion failure is preserved. Explicit installed test: 1 passed,
3/3 INT lookups, no localized payload printed. Rust CTests: 7/7, 0 failed; Fire self-test passes and
Reference installed checker: 33/33 rules. Saved-main Reference capture matches exactly: 22/22 OK,
0 errors, 39 events. Off remains exit 3 after 12 complete emitted OK steps / 14 events at the earlier
Pawn IAIInterface refusal; partial stdout and fatal stderr match saved main, final errors array unavailable.
Unchanged C++/Python coverage is reused separately: 166/166 and 9 packages / 3,339 exact class bodies,
not rerun here. The interrupted original workspace test had no completed exit; recovery ran the outstanding
test/release gates only. Logs and binaries: ignored `local/p2/fix-localize-off/`. No host, visual or
real-game validation; native behavior and gameplay parity remain `UNVERIFIED`. Decision 2026-11-17-11.

**NAV-MESH review r2 (2026-11-17, zuhu desktop):** AI-assisted (Codex / GPT-6.1 Sol). One additional synthetic
refusal-table case reproduces a neighbour edge with repeated endpoints: it failed before the sorted-pair check and
passes after it. Workspace build and release CLI build clean; workspace tests 1,784 passed / 0 failed / 30 ignored;
crate edges 19/19. After building the release Rust reader, configure with `OW_REQUIRE_TEST_TOOLS=ON` registered the
seven Rust CTests absent from the earlier build: all 7 passed, 0 failed, 74.47 s. The prior 166 C++/Python checks
apply to byte-identical inputs; they were not repeated. Fresh C++ reader verification: 9 packages, 3,339/3,339 class
bodies exact. These are automated checks; no host, visual or running-game validation was performed.

**NAV-MESH review r1 (2026-11-17, zuhu desktop, research/rust-navmesh):** AI-assisted (Codex / GPT-6.1 Sol),
continuing partial Claude Code fixes. `cargo build --workspace --locked -j 2` clean; `cargo test --workspace --locked
-j 2`: 1,784 passed / 0 failed / 30 ignored (1,782 passed before this fix round). The four synthetic navmesh tests
pass; the ignored installed-package driver run separately passes 1 / 0, reading all four Sanctuary meshes with the
stricter structural checks (12,092 / 191 / 148 / 46 triangles, unchanged). `crate_edges_test.py`: 19 tests OK.
Fresh C++ Release build at `--parallel 2` clean; CTest at `-j 2`: 166/166, 0 failed, 404.61 s. Package verification:
9 packages, decoded bytes/counts/export fields match, 3,339/3,339 class bodies exact. The optional `rust-parity`
entry was not registered because the Rust release CLI is absent; the separate `py-rust_parity` entry passed.
Installed-package checks are automated parsing evidence; no running-game, host or visual validation was performed.

**FIX-POOLS-OFF recovery (2026-11-17, pc-hummer, main 03df52d9 plus this lane):** AI-assisted
(Claude Code; recovery: Codex / GPT-6.1 Sol). Workspace build passes (`--locked -j 2`), workspace tests
1789 passed / 0 failed / 35 ignored. Recovered prior ctest: 173/173 (C++ unchanged); reader verification:
9 packages, 3,339/3,339 class bodies exact (recovered Rust log and fresh C++ reader run). Fire checker self-test
passes and 33/33 installed rules hold; combat checker self-test passes; crate edges 19 tests OK. Legacy Python
combat suite: 3 identical failures on the saved main binary and this lane in Reference; not a pass. Saved installed
combat trace checker: 8 PASS / 0 FAIL / 51 MISSING of 59, 43 loot rows not listed. Off CLI combat reaches unbound
`ResourcePoolManager.ShutDown`; isolated slice still stops at #370's missing template holder. No host or real-game
validation in this recovery. Decision 2026-11-16-82; full logs under ignored `local/p2/fix-pools-off/`.

**Rust switch (2026-11-14, branch rust-switch):** on main 08bcf3e plus the switch, zuhu desktop: ctest 160/160 (177 entries before; the 17 removed covered only the UE5 host or its scripts: `anim-import-frames`, `controls-keys`, `movie-hud-switch`, `play-slice-launcher`, `suite-map-tracked` and 12 `py-*`), 189 s with `-j 4`; verify_packages 9 packages, decoded bytes, counts and export fields match, all class bodies exact; `cargo build --workspace --locked` clean and `cargo test --workspace --locked` 4 passed / 0 failed (`ow-host`'s install check; the other crates hold no tests yet); `python tools/decisions_index.py --check --baseline origin/main` OK, 1 added; `viewer_test.py` and `level_test.py` pass; `tools/leak_gate.py --staged` 0 errors (warnings: the `ci.yml` gate change, and `.rs`/`.toml`/`.lock` as file types the gate does not know yet).

**Rust scaffold (2026-11-14, zuhu desktop, feature/rust-w0-scaffold on main 74d7dda, lane W0-SCAFFOLD after review r1):** 22 crates; ctest 161/161 (160 before; the new entry is `py-crate_edges`), 185 s with `-j 4`; verify_packages 9 packages, decoded bytes, counts and export fields match, 3,339/3,339 class bodies exact (the C++ reader, unchanged); `cargo build --workspace --locked` clean, `cargo test --workspace --locked` 28 passed / 0 failed (`ow-core` 17, `ow-cli` 6, `ow-host` 4, `ow-container` 1), `cargo build --release -p ow-cli --locked` clean; `python tests/crate_edges_test.py` 13 tests OK (same numbers after review r2); `python tools/decisions_index.py --check --baseline origin/main` OK, 0 added. The Rust `ow-package` exits 3 (not ported) for every command.

The UE5-host suite numbers below (`test_quest`, `test_mover`, `test_inventory_actions`, `test_weapon_firing`, the HUD
shots, the slice gate and `ambient`) are history; those suites left the tree with the host on 2026-11-14. The ctest,
verify_packages, Fire checker and combat checker numbers below were the baseline of their day.

**Baseline (2026-10-08, commit c33ed0c, second machine):** ctest 15/15; verify_packages 9 packages, 3,339/3,339 class
bodies exact; Fire event order self-test and 32 of 32 rules; test_quest 117/117 plus resume 11/11; test_mover 16/16;
test_inventory_actions 49 pass / 0 fail; verify_combat_pawn `--vm` 25 pass / 0 fail / 4 missing (the Defense Boost rows).

**After lane Sf (2026-10-09, S-f down state and respawn):** ctest 15/15 (combat-synthetic extended); verify_combat_pawn `--vm` 55 pass / 0 fail / 4 missing (the 25 earlier passes plus 30 down rows).

**After lane Sg (2026-10-09, S-g loot on death):** ctest 15/15 (combat-synthetic extended); verify_combat_pawn `--vm` on the S-a to S-f run still 55 pass / 0 fail / 4 missing (of 59); with the loot run's JSON added (`--vm all_rows.json loot.json`, [COMBAT_LOOT.md](COMBAT_LOOT.md)) 97 pass / 0 fail / 4 missing / 1 open (the live 3-item money drop) of 102.

**After the Weapons momentum/sweep change (2026-11-03, pc-hummer, on main 95e48fb):** ctest 18/18 (one test fewer:
`weapon_refresh_firing` retired); verify_packages 9 packages, all class bodies exact; `tools/test_weapon_firing.ps1` 14 PASS /
0 FAIL with the new push, wide and flight-timing checks (WEAPON_PROJECTILES.md section 5).

**After the HUD lane (2026-11-03, pc-hummer, main 95e48fb + feature/hud-integrity, Fight For Your Life in the UE host):** ctest 19/19;
verify_packages 9 packages, all class bodies exact. `test_quest` now has 112 checks; the new `player_death_goes_down_first` and the respawn
after the 20 s bleed-out pass. `run_hud_shots.ps1 -Down` 10/10, `-Tracker` 4/4, default 16/16; `test_action_skill_hud` 19/19;
`test_hud_minimap` 15/15. Rebased on main 490661e (PR #61) with a regenerated audio manifest: ctest 21/21, `test_quest` first run
112/112 and resume 11/11. On main 542d0c6 (PR #62): ctest 20/20, `test_quest` 112/112 + resume 11/11, `-Down` 10/10, `-Tracker` 4/4,
`test_weapon_firing` 14/14 (with the local slice recipes' `firing` block regenerated).

**Slice gate (2026-11-04, pc-hummer, main be94f4a + feature/slice-gate):** `tools/test_slice_gate.ps1` 9 PASS / 0 FAIL /
4 NOT_AVAILABLE of 13 links (the gaps are pieces other lanes own; [SLICE_GATE.md](SLICE_GATE.md)); ctest 21/21.
On main 8bb74a0 (after #88): 10 PASS / 0 FAIL / 3 NOT_AVAILABLE (the pickup link now checks the pickup card); ctest 22/22.
On main 195a016: 10 PASS / 0 FAIL / 4 NOT_AVAILABLE of 14 (new decoy-pickup link); ctest 22/22.
On main 7eee9ee (2026-11-07, zuhu, Z23): 14 PASS / 0 FAIL / 0 NOT_AVAILABLE of 14 in four runs (no switch twice, `-Shield`, `-DownAfterFight`); ctest 28/28; verify_packages exact; range 36 / 0; firing 14 of 14; interactives 11 / 0; mission page 21 of 21 on its rerun (the first run failed 4 rows with the movie unavailable). `-Tracker` 3 of 4 (`tracker_page` saw 6 of 7 states) on this PC.

**Ungated tests registered (2026-11-11, zuhu, main c66f24e + fix/ungated-tests):** ctest 119/119 (34 before; the 85 new `py-*` entries took 76 s of the 211 s run); after review r1, 118 (84 `py-*`, `slice_npc_assets` moved out); verify_packages 9 packages, all class bodies exact. The tests left out are in "Tests outside ctest" below.

**Node and numpy groups (2026-11-11, zuhu, main 969307f + feature/ci-test-deps, Node 26.4, numpy 2.5.4, Python 3.14.6):** ctest 145/145
(119 before; `needs-node` 18 entries in 1.8 s, `needs-numpy` 8 in 2.5 s). Configured with a Python without numpy and no Node: 119
entries and one message per group left out.

**With PR #221 (2026-11-11, zuhu, feature/ci-test-deps + feature/audio-mix-2d-level, numpy 2.5.4):** ctest 154/154 (`needs-numpy` 9 entries in 5.7 s,
`listen_report_fit` among them, 10 cases, none skipped; `needs-node` 18, `needs-pillow` 8).

**Real-game tool tests registered (2026-11-13, zuhu, lane C, main 8ee59bb + test/register-realgame-tests):** ctest 174/174 (170 before).
New entries `py-controls_capture`, `py-real_game_capture_safety` and `py-real_game_pending` (43, 58 and 70 s; three runs each, spread
under 1.5 s) and the guard `py-realgame_registration`. All six real-game test files that were outside ctest also passed in a shell with
no `OPENWILLOW_*` or `OW_*` variable, PowerShell 7's module path and a working directory outside the checkout. See "Tests outside ctest".

**On main 81d87f9 (2026-11-13, CI job "Windows / MSVC / LZO=ON" on PR #264's merge ref):** `100% tests passed out of 174`. The desktop runner kept that job's log (`local/z58/cand33/ci_job.log` in its worktree). On the zuhu desktop the same tree registers 174 entries and passes 174 of 174 (LZO on, 361 s); a configure with `-DOPENWILLOW_LZO=OFF` registers 172 (`ctest -N`).

**Fire checker level rule (2026-11-12, zuhu, lane R3):** `verify_fire_event_order.py` self-test passes and 33 of 33 rules hold on the real packages (32 before; the new one: no `level` event in the default run).

**Ambient gate (2026-11-12, zuhu, lane R3):** the `ambient` suite's PASS now also needs a capture-based manifest (`observed`, or worn pieces): `tools/test_ambient.ps1` names the manifest's look in its result line and fails a census-only one before launch unless `-CensusWorld` is passed. First run in the engine on main a3daacf (2026-11-13, zuhu desktop, hub candidate 26), with the capture-based file: `OWAMBIENT SUMMARY result=PASS pawns=35 reached_a_node=35 walked_50cm=3 at_perch=24 seconds=60 manifest observed=true spawns=35 worn=89` (log: the desktop runner's `local/z58/cand26/suites/ambient.log`; the same line again on candidate 30, tree of main 0d2405e). The citizens' movement is a host stand-in, UNVERIFIED.

## Every change
- Build (Windows; CMake ships with VS Build Tools):
  `export PATH="/c/Program Files (x86)/Microsoft Visual Studio/18/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"`,
  then `cmake --build build --config Release --parallel`. A fresh configure pins the project's MSVC toolset:
  `cmake -S . -B build -T version=14.50`.
- `ctest --test-dir build -C Release --output-on-failure`
- `python tools/verify_packages.py --reader build/Release/ow-package.exe`
- `cargo build --workspace` and `cargo test --workspace` (Rust toolchain pinned by `rust-toolchain.toml`).

## When the VM, missions or the slice change
- `python tools/verify_fire_event_order.py --self-test` and `python tools/verify_fire_event_order.py --reader build/Release/ow-package.exe`
- Compare `--slice-run` output before and after against a copy of the previous build, and explain every difference.

- The slice gate's `-Shield` run was a UE5-host run and left the tree with it; its last numbers are history
  (`git show 08bcf3e:docs/verification/GATES.md`).

## When combat changes
- `python tools/verify_combat_pawn.py --self-test`, and `--vm <json>` on a fresh `--combat-run`.
- `python tools/verify_combat_pawn.py --live` checks the formulas against ignored live dumps when they exist.

## When the Rust host changes

### Host (Rust, H1): proposed by lane H1, 2026-11-16
The host's first checks (decision `docs/decisions/2026-11/2026-11-16-44-engine-crates-wgpu-and-winit-in-ow-host-host-skeleton-h1.md`).
CI already runs most of them: the "Windows / Rust workspace" job runs the build, the tests and the refusal check, and
the Windows / MSVC LZO=ON job runs ctest, which includes `crate_edges_test.py` (`py-crate_edges`). The one new CI step
proposed here is `--headless-probe`, which waits for the maintainer's yes on a workflow change
(`.github/workflows/ci.yml` has not gained it; "CI legs" below says how the jobs split the suite).
- **CI, and every change to `ow-host`, `Cargo.lock` or `tests/crate_edges_test.py`:** `cargo build --workspace
  --locked`, `cargo test --workspace --locked` (the `ow-host` unit tests among them), `python tests/crate_edges_test.py`
  (also ctest `py-crate_edges`), and the existing refusal check (`ow-host --game <not an install>` must exit non-zero).
  Proposed, not yet in CI: `ow-host --headless-probe`: exit 0 prints the adapter; exit 4 means no adapter on that machine and counts as
  "not run", not as a failure; any other exit fails. With the engine crates, the build downloads them from crates.io
  (or restores them from the cargo cache); the CI job's comment says so since 2026-11-19.
- **A machine with a GPU and a screen** (the zuhu desktop, pc-hummer), after a change to the window, device or frame
  loop: a release build, then `ow-host --game <install> --frames 300`, and again with `--no-vsync`, timed. Pass is
  exit 0 with 300 frames presented. Record the adapter, the backend, the median frame time and the wall time. The
  vsync median should be close to the monitor's refresh interval; the `--no-vsync` median is the loop's throughput.
  A `--frames N` run fails by itself after 60 s plus 1 s per frame (`--time-limit S` sets another limit), also with
  the window minimised (runs below).
- **One run per machine:** every window run (`--window`, `--frames`) is a heavy run. Take the machine-wide lock first
  and release it after the process has exited: `. ./tools/ue_lock.ps1; Enter-OwUeLock -Owner <lane>; try { ... }
  finally { Exit-OwUeLock }`. `--headless-probe` and the install check open no window and need no lock. The
  maintainer works on the desktop's screen, so keep runs short.

**Baseline (2026-11-16, zuhu desktop, i9-9900K, RTX 4070, driver 621.01, release build, lane H1):** probe "NVIDIA
GeForce RTX 4070", Vulkan, exit 0. `--frames 300 --no-vsync`: 300 presented, 0 skipped, median 0.171 ms (min 0.164,
max 0.508) over 299 intervals, 642 ms wall. `--frames 300` with vsync: 300 presented, exit 0, median 252 ms. Dx12 gave
the same. The screen had been idle for about 18 hours, so the displays were very likely asleep; the vsync figure is
`UNVERIFIED` until it is repeated with the screen on (165 Hz monitor, 6.1 ms expected). `crate_edges_test.py` 19
tests OK.

**Minimised-window runs (2026-11-16, same desktop and build, after review r1 of PR #344, under the lock).** The
window was minimised and restored with `ShowWindow` on the window of the process the script had started. Wall times
are from process start to exit.
- `ow-host --game <install> --frames 300`: exit 0, 300 presented, 0 skipped, median 252.371 ms, 76.8 s wall.
- `ow-host --game <install> --frames 48`, minimised at 2.0 s, restored at 6.1 s: exit 0, 48 presented, 319 skipped,
  13.8 s wall.
- `ow-host --game <install> --frames 100000 --time-limit 15`, minimised at 2.0 s and kept minimised: exit 1 with
  "15 of 100000 frames presented within the 15 s limit", 15 presented, 1176 skipped, 15.8 s wall.
- In these runs the minimised window still got redraws (the skips), so the case of a minimised window that gets no
  redraw at all did not happen. That the deadline's own wake-up ends such a run is `UNVERIFIED`: the code covers it
  (the deadline is checked on every wake-up, and the loop always waits on it), but no run showed it.

### Host movement (Rust, H4): lane H4, 2026-11-16
Movement and collision (decision `docs/decisions/2026-11/2026-11-16-52-host-movement-and-collision-h4.md`).
- **Every change to `ow-host/src/movement*` or `ow-assets/src/collision.rs`:** the workspace build and tests above
  (the synthetic movement and body tests are among them).
- **On a machine with the game installed**, after a change to the mover, the sweep or the collision build: a release
  build, then `ow-host --game <install> --walk`. It opens no window, but it loads the whole scene (40-52 s on the
  desktop), so it is a heavy run: take the machine-wide lock first, as for window runs (`. ./tools/ue_lock.ps1;
  Enter-OwUeLock -Owner <lane>; try { ... } finally { Exit-OwUeLock }`). `--walk` does not take it itself (H1 has
  no Rust lock helper). Pass is exit 0 with `result: PASS`, and the collision counts unchanged unless the change
  explains them. A stall at a wall is reported by name and is not a failure; a stall with nothing in the way is.

**Baseline (2026-11-16, zuhu desktop, release build, lane H4):** placements 4,446; blocking 3,855, of them 3,124
with hulls; 299 meshes with a body (494 hulls); 4,219 brushes, 0 refused. Walk: PASS, 1,050 ticks, 3,562.2 uu
travelled, no start-solid tick; two named wall stalls. Segment 4's (a `Sanctuary_P` chain-link fence, taller than
the jump) is expected. Segment 5's, on `Sanctuary_Outer` `InterpActor_34`, is an **open question**, not settled
expected output: `prepare_level.py` drops that box's collision (decision 2026-09-23-03), and whether it blocks in
landed play is not observed (decision 2026-11-16-52). Workspace tests 1,505 passed, 0 failed, 17
ignored (on main 54880df plus this lane and its review r1); the walk run under the lock.

### Host play mode (Rust, H4-PLAY): lane H4-PLAY, 2026-11-16
Walking a map in the window (decision `docs/decisions/2026-11/2026-11-16-57-host-play-mode-walk-sanctuary-in-the-window-h4-play.md`).
- **Every change to `ow-host/src/play*`, the input or step code in `app.rs`, or `movement/walk.rs`:** the workspace build
  and tests (the `play` tests check one script's path against `walk::run` at three frame-time patterns).
- **On a machine with the game installed**, under the machine lock (every run below loads the whole scene), with a
  60 s script whose first nine segments are `--walk`'s (`local/p2/h4-play/walk60.txt` on the desktop):
  `ow-host --game <install> --walk`, then `--map Sanctuary --play-script <file>` (headless), then `--map Sanctuary
  --play --play-script <file> --size 1280x720 --screenshot <png> --no-vsync` (the window). Pass is exit 0 for all
  three, the headless run's first nine `play segment` digests equal to `--walk`'s, and the window run's twenty digests
  and end digest equal to the headless run's. Report the window's frame times and look at both screenshots. A
  run with vsync on a sleeping display shows about 252 ms frames (H1); its path must not change.

**Baseline (2026-11-16, zuhu desktop, RTX 4070, Vulkan, release build, lane H4-PLAY):** `--walk` digest
`fd6551c9598c8ec7` (1,050 ticks); headless 3,600 steps, end digest `d7b95ebfe14df774`; the window with vsync (display
asleep: median 252 ms) and with `--no-vsync` (87,516 frames in 60.3 s, median 0.680 ms, p95 0.711 ms) both gave that
digest and all twenty segment digests. After review r1 of PR #355: a scripted window run of exactly the `--walk` path
(17.5 s, `--no-vsync`, median 0.688 ms) gave `--walk`'s nine segment digests and its end digest `fd6551c9598c8ec7`;
workspace tests 1,607 passed, 0 failed, 21 ignored. Workspace tests 1,603 passed, 0 failed, 21 ignored (main 7f73940 plus this
lane; the installed-game runs were made on main e126214 plus this lane).

### Host held weapon (Rust, H4-WEAPON): lane H4-WEAPON, 2026-11-16
Firing in the play mode (decision `docs/decisions/2026-11/2026-11-16-61-host-held-weapon-fire-in-the-play-mode-h4-weapon.md`).
- **Every change to `ow-host/src/weapon*`, `overlay.rs`, or the play mode's trigger:** the workspace build and tests
  (the `weapon`, `overlay` and `play` tests).
- **On a machine with the game installed**, under the machine lock, with a firing script (`local/p2/h4-weapon/fire20.txt`
  on the desktop) and a weapon file under ignored `local/`: `--map Sanctuary --play-script <script>` without and with
  `--weapon <file>` (headless), then `--play --play-script <script> --weapon <file> --size 1280x720 --screenshot
  <png> --no-vsync` (the window). Pass is exit 0 for all three; the same path digest for all three (firing moves
  nothing); and the window's shot count and shot digest equal to the headless weapon run's. Look at the screenshots
  for the crosshair, the placeholder and, on a frame within 2 s of a hit, the impact marker.

**Baseline (2026-11-16, zuhu desktop, release build, lane H4-WEAPON, the Tediore pistol of the firing coverage
recipes):** weapon damage 34.24, fire interval 0.2100 s, range 16384 uu; path digest `3413832365d19721`; no weapon 2
shots, with the weapon 53 shots and shot digest `797e931e95ce7bb6`, headless and in the window (median frame
0.780 ms). Workspace tests 1,712 passed, 0 failed, 25 ignored (main e002de5 plus this lane with review r1 of PR #366; the installed-game
runs on main f5cd6a4 plus this lane).

### Host play mode on the loaded world (Rust, H-WORLD): lane H-WORLD, 2026-11-19
The play mode's world (decision `docs/decisions/2026-11/2026-11-19-01-host-play-mode-on-the-loaded-world-h-world.md`).
- **Every change to `ow-host/src/world*`, the play mode's world tick, or `anim/pawns.rs`' world placement:** the
  workspace build and tests (the `world`, `play` and `args` tests).
- **On a machine with the game installed**, under the machine lock, with H4-PLAY's 60 s script: `--map Sanctuary
  --play-script <file>` twice and once with `--no-world` (headless), then `--play --play-script <file> --size 1280x720
  --screenshot <png> --no-vsync --world-pawns <dir>...` (the window; the folders are UModel `-export -psk -notex
  -nostat` exports of Sanctuary's level packages under ignored `local/`). Pass is exit 0 for all four; the same path
  digest and segment digests for all four; the same world digest for both headless runs and the window. Report the
  world's load line, its halt line and how many pawns were drawn.

**Baseline (2026-11-19, zuhu desktop, RTX 4070, Vulkan, release, lane H-WORLD):** world Sanctuary_P (Off) 2,363
actors, 0 refused, 11 pawns; halted at tick 1 (`NotYet`: the host's `PHYS_Interpolating` physics for a placed
`Engine.InterpActor`), `TimeSeconds` 0.016667; path digest `d7b95ebfe14df774`, world digest `c233c6faac880f05`; the
window drew 1 of 11 pawns (median frame 1.724 ms).

**After H-PHYSICS-MODES (2026-11-19, same machine; decision `2026-11-19-22`):** the same path and world digests; still
halted at tick 1, now on the Matinee playback of a bound interpolating `WillowGame.WillowSpectatorPoint`. A change to
the physics step reruns the census (`world::tests::sanctuary_physics_census`, ignored, `OW_BL2_ROOT`) and reports its
first stop and outcome counts (today 57 pass, 23 at the world's `Base` guard, 66 at Matinee playback, 13 at the
walking, nav-mesh walking and rotating modes).

**After H-MATINEE (2026-11-19, same machine; decision `2026-11-19-53`):** the same path and world digests; still halted
at tick 1, now on the level-load event that starts the Matinee op binding a `WillowSpectatorPoint`. A change to the
Matinee scan or tracks reruns `world::tests::sanctuary_matinee_census` (ignored, `OW_BL2_ROOT`) and reports the ops by
start (today, with a complete graph: 20 at load, 53 from outside the graph, 8 with an unknown startup) and the movement tracks read (today 168 of 169,
every one ending within its op's `InterpLength`), with the physics census (today 57 pass, all unbound; no op is answered "not started at load").

The retired UE5 host's suites and their baselines are in `git show 08bcf3e:docs/verification/GATES.md`.

## Records
- `python tools/decisions_index.py --check --baseline origin/main` after adding, editing or moving decisions (with the
  default baseline, `HEAD`, it passes trivially on committed work). Add `--at HEAD` to check the committed blobs.

## Tests outside ctest
Since 2026-11-11 every `tests/*_test.py` that passes in a clean export (no `local/`, no build, no installed game, no network) under
a Python with no third-party packages, with none of its cases skipped, runs in ctest as `py-<name>` (the list in `tests/CMakeLists.txt`, which `CMakeLists.txt` includes). CI also runs
`level_test.py` and `viewer_test.py` directly (in the LZO=ON leg). The rest run only by hand (`python tests/<name>` or `node tests/<name>`); run those
that cover what you touched. Times are serial on the zuhu desktop, 2026-11-11. The list of what stays out, each with its reason, is `OW_OUTSIDE_CTEST` in `tests/CMakeLists.txt`;
`tests/realgame_registration_test.py` (ctest `py-realgame_registration`, since 2026-11-13) fails when a test of the real-game tools (one
that imports `tests/realgame_test_env.py` or names a `tools/real_game/` path) is neither registered nor in that list with a reason.

Since 2026-11-11 (lane ci-test-deps) CI also installs Node 26.4.0 and numpy 2.5.4 for tests only. The 18 `tests/*_test.js` run in
ctest as `js-<name>` (`OW_NODE_TESTS`) and the 9 Python tests that need numpy as `py-<name>` (`OW_NUMPY_PY_TESTS`: `ambient_transform`,
`creature_zone`, `listen_report_fit` (added by PR #221), `lootable_tracks`, `phaselock_target_attachments`, `prepare_phaselock_target`, `slice_npc_assets`, `weapon_reload_anims`,
`weapon_reload_library`). Each group is registered only when configure finds Node, or a `${Python3_EXECUTABLE}` that imports numpy;
otherwise configure prints which group it left out, and everything else still runs. CI configures with
`-DOW_REQUIRE_TEST_TOOLS=ON`, which turns a group left out into a configure error. CI's step "Case counts of the tests labelled
needs-*" prints each such entry's case count and fails the job if any of them skipped a case.

**CI legs (since 2026-11-19, decision `docs/decisions/2026-11/2026-11-19-100-ci-parallel-ctest-split-lzo-legs-cargo-cache-docs-only-skip.md`):**
ctest runs in parallel (`--parallel` at the runner's core count). The LZO=ON job runs every entry; the LZO=OFF job runs only the
entries labelled `cxx-binary` (`ctest -L cxx-binary`): those whose command runs `ow-package`, `ow-weapongen` or
`ow-weapon-firing-test`, found by `tests/CMakeLists.txt` from its own `add_test` calls. The `needs-*` case-count step checks the
`needs-*` entries of its job's share, so all of them in LZO=ON. CI creates the ignored `local/` folder before ctest, as a working
checkout has it. A pull request whose every changed path is under `docs/` (except `docs/TOOLING.md`, which
`real_game_migration_test.py` reads) or is a top-level `*.md` skips the three Windows jobs; the leak gate always runs. Locally,
`ctest -j 8` is the same suite as serial ctest.

Pillow (commit 2 of that lane, landed only with the maintainer's yes): CI installs `pillow==12.2.0`, and the 8 tests in
`OW_PILLOW_PY_TESTS` (`gfx_to_swf`, `prepare_map_screen`, `prepare_minimap`, `prepare_mission_log`, `prepare_mission_screen`,
`sanctuary_pillar`, `weapon_paint`, `weapon_part_census`) run as `py-<name>` where Pillow and numpy both import (`sanctuary_pillar`
needs both). `prepare_mission_log` skips one of its 4 cases where file symlinks need a privilege (this PC).

| Tests | Why not in ctest | Status on 2026-11-12 |
|---|---|---|
| `cash_capture` (236-237 s), `realgame_snapshot` (147-148 s), `skills_live_probe` (60-61 s) | over the budget: a file may add about 90 s and all additions together about 3 minutes (lane C, 2026-11-13). The three registered real-game files take that 3 minutes. `cash_capture` could be split (its 74 cases outside `DenDownReplacementTest` and `LoadWaitTest` take about 85 s), but that subset would not fit beside them, and a subset named by class would leave out any class added later | pass, also in the sanitised shell (2026-11-13) |
| `check_hud_minimap` (30 s), `provision_worktree` (36 s) | too slow for every PR (2026-11-11 times, not re-measured) | pass |

Notes (as of 2026-11-12; `lootables_editor` and `run_limit` left with the UE5 host on 2026-11-14): a test that skips cases there, or skips all of them, does not count as passing. Four registered tests skipped cases off Windows
(`check_overlay_data`, `lootables_editor`, `private_sync`, `run_limit`: junctions, PowerShell launchers); CI runs
on Windows, so they run there. `serve_paths` skips nothing: it makes a directory symlink, and on Windows falls back to a junction when that is refused. `private_sync` also skips its UNC alias case without an administrative share. `game_colours` and
`prepare_hud_strings` create `local/` in a clean checkout (the second leaves it empty); it is git-ignored and harmless. The slowest registered entry
that started processes then, `run_limit` (about 40 PowerShell runs), took 11.2 s on CI (PR #219, run 43062806521) and 15.4 s here, well under
its 120 s timeout. On that CI run (Python 3.12) the new entries took 66 s of 172 s with LZO on. `weapon_reload_library`
skips its junction case when `mklink /J` fails; on CI that skip fails the job.

**H-PAWNS review correction (2026-11-17, desktop, PR #378 after 7c7c387):** AI-assisted
(Codex/GPT-6.1 Sol). An empty static scene now still invokes the attached-world draw callback.
The invented offscreen GPU regression checks exactly one callback with and without static instances:
old code 0 passed / 1 failed; correction 1 passed / 0 failed, 191 filtered out, 0.46 s.
The test is ignored by default because it needs an adapter; it was explicitly run locally.
No new window, real-game session or visual/gameplay parity claim. Prior reported workspace and C++
results were not rerun for this one-line rendering correction; final-head branch CI remains required.

**FIX-BEHAVIOR-CONTEXT-OBJECT (2026-11-18, pc-hummer, base 2fcae040):** AI-assisted
(Codex / GPT-6.1 Sol). Off runner selection now uses the explicit context object when there is no Context link;
a Context link still wins even when empty. Native/gameplay semantics remain UNVERIFIED. Corrected synthetic
regression RED: 2 passed / 3 failed; GREEN: 5 passed / 0 failed (included in the workspace run).
`cargo build --workspace --locked -j2`, `cargo test --workspace --locked -j2 --no-fail-fast` and the release CLI
build exit 0; workspace totals 1,865 passed / 0 failed / 41 ignored. Seven Rust CTests: 7/7 passed, 13.46 s.
Reused non-Rust CTests: 166/166; reused C++ reader verification: 9 packages, 3,339/3,339 class bodies exact.
These were not rerun: the C++/Python input blobs match the earlier local evidence (506 paths checked against
its source dadca792), and no src/tools/tests/CMakeLists input path changed. The earlier full 173-entry run had
two Rust failures; those are superseded here only by the fresh seven-entry Rust run.
Fresh parent/child Reference slice captures: both exit 0, 22 steps, 39 events, 0 errors, equal after excluding
only timing fields. Off captures: both exit 3, identical truncated JSON with 12 complete steps and 14 events;
the Pawn IAIInterface routing refusal remains. This does not establish an improvement in the installed Off run.
Captures and failed regression attempts are ignored under `local/p2/fix-behavior-context-object/`.
Fire checker self-test passes; installed C++ Fire check: 33 of 33 rules hold; crate edges: 19 tests OK.
No host, visual, GPU or running-game validation; decision 2026-11-18-01.
