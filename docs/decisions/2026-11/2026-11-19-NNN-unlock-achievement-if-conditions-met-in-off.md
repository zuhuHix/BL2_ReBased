# UnlockAchievementIfConditionsMet in Off (link 12's turn-in)

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-LINK12-UNLOCK-ACHIEVEMENT, 2026-11-19. Number pending (NNN).

**What changed.** `WillowPlayerController.UnlockAchievementIfConditionsMet` is bound in `ParityMode::Off`, from
NATIVE_UNLOCK_ACHIEVEMENT_IF_CONDITIONS_MET.md (#541, fd8c7ed3) alone: the clean room holds, as this lane did not
analyse it. The code is `ow-inventory` `globals/achievement.rs`, registered from `globals::register`. It is first-wins:
a key already bound is kept. Reference is unchanged, so the VM's stub policy answers.

**The rule** (the note's section 4 in full, by the hub's ruling, not its smaller alternative):
1. **No globals object recorded:** false, and nothing is done.
2. **The globals definition** is read through the existing `globals::globals_definition` helper, as it is (the hub's
   ruling on #541's r1 review). A None `TheGlobalsDefinition` is its first-load `NotYet { NotRead }`. A handle not
   loaded yet is its lazy-load `NotYet`. Both are raised before any write and are unmarked. The native does not
   shortcut "no definition" to false.
3. **The declared field types** are checked after that refusal and before any value is read: `AchievementData` is a
   dynamic array of `WillowGame.AchievementUtility.AchievementUnlockData`, with a byte `UnlockType` and `Achievement`,
   a string `StringData` and an int `IntData` (read from the installed declaration). A mismatch is `VmError::Script`
   naming the field.
4. **The selection** is one function, `achievements_to_unlock`, taking the native's six inputs. It is shaped so
   `AchievementUtility.ShouldUnlockAchievementFromUnlockType` could share it; that native is not bound. The rules:
   - `Achievement` not 0, and the same `UnlockType`;
   - the string match (non-empty, compared without case by an ASCII fold, UNVERIFIED for text outside ASCII) or the
     int match (the entry's int at least 1, then equal, above with `bCanBeGreater`, or below with `bCanBeLess`);
   - both matches when `bCheckStringAndIntData` is set.
5. **None selected:** false, and nothing is written.
6. **Some selected:** the receiver's `ClientUnlockAchievement(id)` is found by name on its class and state and called
   through the VM once per entry, in order, with the byte id passed as the declared int. The answer is then true. Its
   refusals propagate. A `NotYet` is marked "stopped after partial writes: " if and only if #518's counters moved since
   the native's entry (committed writes, draws, load transitions).

**Omitted arguments.** An argument that is omitted, or arrives as `Value::None` (how the VM passes an omitted
optional parameter; the turn-in omits `StringData` and the last two flags), reads as its type's zero: 0, the empty
string, false. This is `ow-world`'s `arg_bool` convention. The first installed gate run showed it: without it, link 12
stopped at "StringData is None, not a string".

**#518's counters.** They are not on main. The counter files were checked out byte-identical from
`fix/rust-483-followup` 1f1a2bc: `ow-core` `rand.rs`, `ow-object` `heap.rs`, `ow-defs` `store.rs`, and `ow-vm`
`api.rs`, `interp/aggregate.rs`, `interp/eval.rs`, `interp/mod.rs`, `natives_core.rs` and `states.rs`. Main had not
changed any of them since the merge base, so the copy reverts nothing. They fold away when #518 lands.

**The driver (link 12).** The mission bridge's world makes its `WillowGlobals` from class defaults, so the helper would
refuse there. In Off, the Fire driver (`ow-cli` `cmd/slice.rs`, `unlock_setup`) binds the native, then preloads the
installed `GD_Globals.General.Globals` into the bridge's globals object. It does this at set-up, before any step, the
way the pickup world does for links 6 and 7. The shared helper is `equip_run::preload_globals_definition`; the pickup
world now calls it too, with the same steps in the same order, except that a read moved after the load.
- The preload is logged as "harness input: preload …". Its load goes through the definition store, so
  `load_transitions` counts it.
- The first load's challenge numbering is not run (NATIVE_WILLOW_GLOBALS.md section 6.1).
- An UNVERIFIED stand-in for the game's own first load.

**The slice gate.** `--slice-gate` builds its Fire world itself, so it calls the same set-up after
`FireMissionSlice::new`. Installed, in Off and strict, on a throwaway local merge with #533, #496, #497, #509, #512,
#524 and #526: 3 ok / 2 NotYet / 9 errors. Links 10 and 11 are ok. Link 12 passes 1 of 6 steps (`use`) and now stops
in `turnin` at the port's own existing refusal, "stopped after partial writes:
MissionTracker.AllExpansionSideMissionsComplete with a controller …" (`NotYet { NotRead }`,
NATIVE_MISSION_OBJECTIVES.md), the stop the note's section 5 predicted. The run got past the native; its answer on that run was not
printed, so that it answered false is the note's expectation, not observed.

**Verified:** synthetic tests only (`globals/achievement_tests.rs`, 12 tests on invented packages): the turn-in's
omitted arguments, no match, the
string match, the int match with both flags, both matches together, `Achievement` 0 and another type skipped, the call
order, no globals, the two definition refusals (unmarked, nothing loaded), a refusal marked only after a write, a wrong
`AchievementData`, Reference, and first-wins.

**UNVERIFIED:**
- every rule, in the running game;
- whether the definition is already loaded at a turn-in in game (the note's open question 2);
- the preload as a stand-in;
- the ASCII case fold.

The live check is the note's: hook both functions during the first side mission's turn-in, and expect type 16, int 1,
`bCanBeGreater`, false, and no `ClientUnlockAchievement`.
