## 2026-11-25: SetMaterial and the glow read in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-SETMATERIAL-AND-GLOW, stacked on
`fix/rust-link8-startup-material` (#712, b257456f). It implements `MeshComponent.SetMaterial` and
`MaterialInterface.GetScalarParameterValue` in Off from docs/verification/NATIVE_INITMESHES_MATERIAL_GLOW.md section 4
(#705, lane F's note). Clean room: this lane did not read the executable. No real-game session was run. **Every rule is
UNVERIFIED in the running game.** Reference is unchanged: neither key is bound there.

**Where.** `ow-inventory` `equip::material_natives`, bound by key with the equip natives (`register_equip_keys`,
first-wins).

**`SetMaterial`,** in the note's order:
1. the declarations first: `component_setters::schemas` on `REATTACH_FIELDS`, and `Materials` an array of objects.
   A mismatch is a `VmError::Script` naming the field, even for a negative index;
2. a negative index: nothing;
3. the entry already holds the material: nothing;
4. the deferred reattach's preflight through `component_setters::reattach_check`. Since the rebase onto
   integration/2026-11-25-c21 (#714), an `Owner` goes through #714's owner rule: `REATTACH_OWNER` for a static or
   tick-disabled owner that is not deleted, its not-loaded refusal for an owner handle; a dynamic owner passes. With
   no `Owner`, a `Scene` set is `REATTACH_SCENE`.

The commit grows `Materials` to reach the index, with the grown entries None (**UNVERIFIED (fitted)**), sets the
entry, then `bNeedsReattach` = true. The second write goes through `component_setters::after_writes`. A receiver the
store has not loaded is a `NotRead`.

**`GetScalarParameterValue`,** by the receiver's first native class:
- `MaterialInterface` and `MaterialInstance`: false;
- `MaterialInstanceConstant`: false with `ReentrantFlag` set; else its own first matching `ScalarParameterValues`
  entry; else its `Parent`; else false. The game's flag set-and-clear is kept without writing: the walk carries the
  instances it is asking, and one met again answers false;
- `Material`: its first `MaterialExpressionScalarParameter` expression with the name. A None entry is skipped. Other
  entries must be resident, and the class must pass `ready_class`, which is checked only when a non-None entry is
  tested;
- any other class: a `NotRead` naming it.

The walk is read-only, and `OutValue` is written only on true.

**Tests** (`material_natives_tests.rs`, invented packages): section 5's 1 to 14, with 5a (link 8's call shape: two
meshes, then the glow read on the same instance) and test 5's four malformed declarations. Each refusal leaves #518's
counters, the class-code loads and the name table unchanged. A key test checks both natives bind on the keys the
package's declarations give.

**The slice gate** (installed Off strict, release build, under the machine lock), on this branch stacked on #712's
head b257456f. That head loads the material's Startup graph (the parent chain and a Material's expressions) and warms
the scalar-expression class. Result: 5 ok / 8 NotYet / 1 error, `links ok 5/14`, `step share X = 5.783/14 = 41.3%`.
Link 8 is 1 of 5 at `init`:
- `SetMaterial` runs on both meshes, and the glow read `GetScalarParameterValue` now answers: no refusal of either
  native;
- the stop is past both: an **error at the unbound `ParticleSystemComponent.SetColorParameter(name,color)`**,
  `InitMeshes`' next step (not part of this lane).

On the way here, link 8 stopped in turn at:
- the parent `MasterMati_BanditCommon` (Startup 23841), not loaded;
- `Master_Gun`'s expression `MaterialExpressionVectorParameter_11` (Startup 23744), not loaded;
- the scalar class check.

Reverted probes named the first two; #712's set-up loads lifted each.

**Gates** (on the stacked head): `cargo test --workspace -j 8`: 3,581 passed / 0 failed / 61 ignored. `crate_edges_test.py`: 19 OK. This is
what the host did, not parity.

**Review of #718 (LOWs) and the rebase onto integration/2026-11-25-c21:**
- the toy owner declares #714's three owner flags; the owner tests use a static owner for `REATTACH_OWNER`, and test 4
  adds a dynamic owner, whose reattach passes and whose entry is written;
- test 5a(ii) says what it shows: the first call's refusal stops the script, so the `ThirdPersonMesh` is untouched;
- a `ParameterName` that is not a name is a `VmError::Script` naming the field (a new test, through an expression
  subclass that shadows it with an int).
