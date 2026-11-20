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

### Review r1 of #617 (FIX)

AI-assisted (Claude Code, Claude Opus 5.5). Four items, all in `attach_mesh/`; still from the note alone, Off only,
**UNVERIFIED** in the running game.
1. **(c) narrowed: the mixed looping-sound state refuses.** The note leaves the looping-sound test unread (sections 7.2
   and 7.4), so (c) above must not turn that unread test into a pass. A non-zero `AkPlayingId` still counts as
   recorded (the audio layer is absent), and the all-zero record as none. A `SourceComponent` set with a zero id is
   now its own `NotYet { NotRead }`, `LOOPING_UNREAD` ("the looping sound record's test ... not read"), before any
   write. The sentence above that says "the step zeroes it" no longer holds.
2. **Step 11 is tested on its own.** A new test runs with `game_thread_lod` clear and two LOD distances. With the forced
   lowest LOD, `LODLevel` 1 plans and `LODLevel` 0 is `LOD_CHANGE`. With it clear, a fixed `LODMethod` and no bias,
   no level is checked. **The LOD bias is not tested.** With more than one distance, the level for location needs the
   camera layer, and with one, every level clamps to 0, so the bias stays unreachable for now.
3. **(d) the parameter name is found first, and interned last.** The plan looks "RColor" up with `names.find`. Only
   after the last `NotYet` (`transform_refusals()`) does it intern the name, when the table has no spelling of it, and
   then it builds the value and runs `fits`. The name table keeps the first spelling it meets (decision 2026-11-15-23),
   so a refused call must not add one. The exact claim: **every refusal of the preflight changes nothing, the name
   table included.** A preflight that plans may add that one name, which none of #518's three counters counts. A test
   checks that the looping-sound refusal leaves the name absent.
4. **The template's role is in the stop-audio refusal.** `stop_audio` takes its not-loaded text as an argument:
   `OLD_NOT_LOADED` for the reset's old template, and the new `T_NOT_LOADED` for T. The preflight refuses an unloaded T
   first (#581's refusal B), so the T text is a guard.

Gates for this round: `cargo test --workspace -j 2` 2,505 passed / 0 failed / 46 ignored (48 result lines);
`ow-inventory` 244 passed. `crate_edges_test.py` OK. Clippy has no hits in the touched files. The slice gate was not
rerun: the changed paths are reached only with an audio-enabled held component or the forced lowest LOD, and links
6 and 7 stop at `touch` as before.
