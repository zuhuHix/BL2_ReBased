## 2026-11-19: Rust particle activation follow-up: #609's four review lows

AI-assisted (Claude Code, Claude Opus 5.5). Lane PARTICLE-ACTIVATION-FOLLOWUP, stacked on #609
(`fix/rust-particle-activation` @ 1d33bd29). Coded from `docs/verification/NATIVE_PARTICLE_ACTIVATION.md` (#597, at
3df9cd88) alone. This lane did not analyse it, so the clean room holds. No real-game session was run. **Every rule is
UNVERIFIED in the running game.** Off only. Reference is unchanged and binds nothing.

It fixes the four items #609's decision lists under "A third guard, and two known gaps":
- **(a) The old template is read only inside the stop-audio step's gate.** The reset no longer checks the old
  `Template` up front. `stop_audio` checks the template it reads, and only once its gate has passed (`AudioEnabled`
  set, a game session or no `Owner`, not being destroyed). An old template that is unloaded or a stand-in is now its
  own refusal, `OLD_NOT_LOADED`, `NotYet { NotRead }`, record NATIVE_PARTICLE_ACTIVATION.md: "the old template's stop
  audio events read a template that is not loaded (a load is a write)". The step also type-checks the template's three
  stop fields. A held component with `AudioEnabled` clear now plans, as the note says, and its old template is never
  read or loaded.
- **(b) The engine-flags guard covers only what is unknown.** `OUTSIDE_SESSION` is now the editor, or a session that is
  not a game. The forced lowest LOD and the LOD bias are ported from the note's section 3:
  - step 11, with the forced lowest LOD: `bOverrideLODMethod` := true, `LODMethod` := 1, and the level T's LOD count
    − 1;
  - step 16, applying a level when T has LOD distances: level plus bias, clamped to 0 .. count − 1.

  The LOD count is read as the number of `LODDistances` (one per level; UNVERIFIED). A level that differs from
  `LODLevel` is still the implementer's guard `LOD_CHANGE`, because the reattach and the instances' LOD are not
  modelled.
- **(c) "A looping sound recorded" is `AkPlayingId` set.** It was any non-zero member of `LoopingAkPlayingInfo`. The
  note leaves this test unread ("by its role") and names the struct's two members, `SourceComponent` and
  `AkPlayingId`. The playing id is the one that names a playing sound, so it is the test (UNVERIFIED; **for the hub**,
  since the note does not single out a member). A `SourceComponent` set with a zero id is not a recorded sound, and the
  step zeroes it.
- **(d) The plan builds the whole `InstanceParameters` value.** The "RColor" entry is updated or appended in the plan,
  and the value is checked against the heap's write rules there. The apply phase only writes it.
  - The parameter name is interned at the start of the preflight, which the plan needs in order to build the value.
  - That changes the name table only. #518's three counters (heap committed writes, generator draws, store load
    transitions) do not move, and the refusal tests still see no change.
- **Tests** (`attach_mesh/pickup_particles_tests.rs`, 24, 4 new):
  - an unloaded old template with `AudioEnabled` clear plans, and stays unloaded;
  - the unloaded old template under the gate is `OLD_NOT_LOADED`;
  - the LOD bias and the forced lowest LOD pass with one LOD distance, `bOverrideLODMethod` and `LODMethod` are
    written, and a level change is `LOD_CHANGE`;
  - a `SourceComponent` without a playing id is not a recorded sound;
  - the plan holds the built parameter value;
  - the engine-flags test now uses the editor for `OUTSIDE_SESSION`.
- **Gates:**
  - `cargo test --workspace -j 2`: 2,504 passed / 0 failed / 46 ignored. `crate_edges_test.py` OK. Clippy has no hits
    in the touched files;
  - the installed Off strict `--slice-gate`, under the machine lock: 0 ok / 5 NotYet / 9 errors. Links 6 and 7 pass
    `make` and stop at `touch` on the unbound `Actor.GetBoundingCylinder(float,float)`, the same as #609's run. This is
    what the host did, not parity.
