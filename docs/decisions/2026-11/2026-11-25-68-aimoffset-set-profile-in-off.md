## 2026-11-25: AnimNodeAimOffset.SetActiveProfileByName / ByIndex in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-AIMOFFSET-SET-PROFILE, stacked on `fix/rust-getweaponholddef`
(2910cde2). It implements docs/verification/NATIVE_AIMOFFSET_SET_PROFILE.md section 3 (lane F's note, #730 at its
review r1 fix bff35b46). Clean room: this lane did not read the executable. No real-game session was run. **Every
rule is UNVERIFIED in the running game.** Reference keeps the stub policy.

**Where.** `ow-world` `world::aim_offset`, both keys bound keep-existing in the world pack's Off block.

**The rule**, preflight first, load-free, all or nothing:
1. the receiver is resident and an `Engine.AnimNodeAimOffset` or below it, declaring `TemplateNode` and
   `SkelComponent` (object references), `Profiles` (an array of a struct), `CurrentProfileIndex` (an int), and
   `RequiredBones` and `AimCpntIndexLUT` (arrays of bytes); otherwise `VmError::Script`, all checked before anything
   else;
2. the list's owner is `TemplateNode` when set (not loaded: `NotYet { NotRead }`), else the receiver;
3. **the interim rule (#730's review):** the store leaves a skipped `Profiles` as the class default, an empty array,
   so an empty list cannot be told from a skipped one. An empty `Profiles` on the list's owner is
   `NotYet { LayerAbsent }`, "AnimNodeAimOffset: the profiles (a binary-serialized struct array)", layer "a reader for
   AnimNodeAimOffset.Profiles", never "no match";
4. by name, the first `ProfileName` match (none: nothing); by index, in range only; the current index is nothing;
5. the rebuild empties the two arrays first (the note's fix). A `SkelComponent` not loaded is `NotYet { NotRead }`;
   any `SkeletalMesh` set, not resolved, is `NotYet { NotRead }`, "AnimNodeAimOffset: the required-bones rebuild's
   sort and parent pass"; both before any write.

Then `CurrentProfileIndex` := the index, and `RequiredBones` and `AimCpntIndexLUT` emptied.

The first push of this lane (91287e8f) also added a store record of skipped properties, so that an empty list could be
told from a skipped one. Under the interim rule it is not needed, and it was taken out again: decoding and the store
are as on main. The accessor that would report "decoded and empty" stays the hub's call.

**Tests.** `aim_offset_tests.rs`, over an invented `Engine.upk`: the note's section 4, 1 to 9, in 10 tests, plus the
declarations' keys. Test 5 is both an export whose `Profiles` tag the store skips and a node with a simply empty list,
as receiver and as a copy's template. Test 7 covers every declaration and a lazy `SkelComponent`. Test 8 runs through
a toy pawn script `SetAimOffsetProfile(N) { AimOffset.SetActiveProfileByName(N); }`.

**The slice gate** (installed Off strict, release build, under the machine lock): 5 ok / 8 NotYet / 1 error, `links
ok 5/14`, `step share X = 5.783/14 = 41.3%`. Links 1, 3, 4, 5, 13 and 14 stop at `maya:8` with the profiles'
**`NotYet { LayerAbsent }`** (the note's expected stop): Maya's template node's `Profiles` slot is empty. They were
errors at the unbound native before.

**Gates.** `cargo test --workspace -j 8`: 3,678 passed / 0 failed / 57 ignored. `crate_edges_test.py`: OK.
