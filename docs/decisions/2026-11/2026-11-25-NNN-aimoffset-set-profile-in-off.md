## 2026-11-25: AnimNodeAimOffset.SetActiveProfileByName / ByIndex in Off, and the store's skipped-property record

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-AIMOFFSET-SET-PROFILE, stacked on `fix/rust-getweaponholddef`
(2910cde2). It implements docs/verification/NATIVE_AIMOFFSET_SET_PROFILE.md section 3 (lane F's note, branch
`docs/native-link1-after-hold-aimoffset-profile` at 46a470ec, not yet reviewed). Clean room: this lane did not read
the executable. No real-game session was run. **Every rule is UNVERIFIED in the running game.** Reference keeps the
stub policy.

**Where.** `ow-world` `world::aim_offset`, both keys bound keep-existing in the world pack's Off block.

**The rule**, preflight first, load-free:
1. the receiver is resident and an `Engine.AnimNodeAimOffset` or below it, declaring `TemplateNode`, `Profiles` (an
   array of a struct) and `CurrentProfileIndex` (an int); otherwise `VmError::Script`;
2. the list's owner is `TemplateNode` when set (not loaded: `NotYet { NotRead }`), else the receiver;
3. an owner whose `Profiles` tag the store skipped: `NotYet { LayerAbsent }`, "AnimNodeAimOffset: the profiles (a
   binary-serialized struct array)", layer "a reader for AnimNodeAimOffset.Profiles". A `Profiles` that was read,
   empty included, answers by the rules;
4. by name, the first `ProfileName` match (none: nothing); by index, in range only; the current index is nothing;
5. the rebuild: with no `SkelComponent` or no mesh on it, `RequiredBones` and `AimCpntIndexLUT` are left empty; with
   a mesh, `NotYet { NotRead }`, "AnimNodeAimOffset: the required-bones rebuild's sort and parent pass", before any
   write.

Then `CurrentProfileIndex` := the index, and the two arrays emptied where they are not empty already.

**The skipped-property record (the note leaves its shape to the hub).** The refusal in step 3 must know that the
owner's `Profiles` was skipped, not empty. The decoder now records each top-level property whose tag
`ValueCx::apply_tags` skipped, keyed by (package, export index, property): `DecodeState::skipped`, read through
`DefStore::skipped_property(space, object, prop)`. It is keyed by the export, not the object handle, so a freed and
reused handle never inherits an entry. **Decoding is unchanged**: the same values, objects and log lines. It is only
a record. This changes the decoder's state in `ow-object` (`decode.rs`) and adds a read accessor in `ow-defs`
(`store.rs`). Neither is on the sensitive list, but the hub may want another shape.

**Tests.** `aim_offset_tests.rs`, over an invented `Engine.upk`: the note's section 4, 1 to 9, in 10 tests, plus the
declarations' keys. Test 5 is a package export whose `Profiles` tag the store genuinely skips. Test 8 runs through a
toy pawn script `SetAimOffsetProfile(N) { AimOffset.SetActiveProfileByName(N); }`. `ow-defs` store tests: the record
reports a skipped tag and not a read one, nor a runtime object.

**The slice gate** (installed Off strict, release build, under the machine lock): 5 ok / 8 NotYet / 1 error, `links
ok 5/14`, `step share X = 5.783/14 = 41.3%`. Links 1, 3, 4, 5, 13 and 14 stop at `maya:8` with the profiles'
**`NotYet { LayerAbsent }`** (the note's predicted stop): Maya's template node's `Profiles` was skipped by the store.
They were errors at the unbound native before.

**Gates.** `cargo test --workspace -j 8`: 3,679 passed / 0 failed / 57 ignored. `crate_edges_test.py`: OK. `ctest` and
`tools/verify_packages.py --reader` were not run (no C++ build in this worktree); the decoder change adds a record
only and changes no decoded value.
