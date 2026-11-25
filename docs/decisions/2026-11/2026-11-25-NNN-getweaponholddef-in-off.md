## 2026-11-25: BodyClassDefinition.GetWeaponHoldDef in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-GETWEAPONHOLDDEF, stacked on `fix/rust-link1-head-mesh` (#727,
15cd21c0, its review fix). It implements docs/verification/NATIVE_GET_WEAPON_HOLD_DEF.md section 3 (lane F's note, branch
`docs/native-getweaponholddef` at 420b03e6, its review pending). Clean room: this lane did not read the executable. No
real-game session was run. **Every rule is UNVERIFIED in the running game.** Reference keeps the stub policy.

**Where.** `ow-world` `world::weapon_hold`, bound keep-existing by key in the world pack's Off block.

**The rule**, a pure read (no load, no class-code load, nothing interned, nothing written):
1. the receiver is resident (`DefStore::is_unloaded` / `is_failed`, or a slotless stand-in, is `NotYet { NotRead }`
   before any field read). Its class declares `WeaponHoldDefs` (an array of object references),
   `DefaultWeaponHoldDef` and `WeaponActionParentClass`; otherwise `VmError::Script`;
2. the own list in order, None skipped, each entry resident (the same `NotRead`, for a weapon hold definition) and
   declaring `HoldName` (a name) and `EnvironmentTag` (an object reference). An entry with the name and the tag is the
   answer at once. Otherwise the first name match is the candidate, replaced by a later name match only when that
   entry has no tag;
3. with `bSearchParentClass` and a parent: the same lookup on `WeaponActionParentClass` (by fit), the fallback passed
   on only when this class has no default. With a candidate, the parent's answer stands only when it is an exact match;
   with none, the parent's answer when it is not None. **"Exact" is the match kind the parent's lookup reports**
   (exact / candidate / default), not a re-read of the returned hold's fields: the hub's interim ruling while #728's
   review asks the analyst which the routine does (UNVERIFIED). It reads nothing more and stays load-free. If the note
   settles on a re-read, the port switches and adds the not-loaded `NotRead` on the returned object;
4. then the candidate; then `DefaultWeaponHoldDef` when `bFallbackToDefaultDef`, else None.

A parent chain that loops is a `VmError::Script`, a port-only guard.

**The set-up walk (the hub's ruling).** With the native bound, link 1 stopped at the honest `NotRead`: Maya's hold
definitions were store handles. They, `BodyClass_PlayerShared` and its default are exports of `GD_Siren_Streaming_SF`,
her pawn's streaming package file. So #727's body-composition walk (ow-cli `combat.rs`) goes on to
`walk_weapon_holds`: from the body class along `WeaponActionParentClass`, each class, its `WeaponHoldDefs` entries and
its `DefaultWeaponHoldDef`, by the same rule (only exports of the pawn's file, or that file's own copy of the path),
each load logged as an UNVERIFIED stand-in. The installed run logs 22 loaded, 26 visited: the seven Siren holds, her
default, `BodyClass_PlayerShared` and `WeaponHold_Player_Default` (both from the Siren file's own copies).

**Tests.** `weapon_hold_tests.rs`: the note's section 4, 1 to 12, in 11 tests, on invented classes, with #728's review cases: a parent default that looks like an exact match
when the fallback was passed on (the own candidate wins), the fallback passed on with no match (the parent's default)
and with an own candidate (the candidate), and (`A`, None) asked against [(`A`, X)] (the tagged entry, ahead of the
default). Every call asserts
that #518's counters, `class_code_loads` and the name table are unchanged. Lazy objects are slotless stand-ins.
`combat/tests.rs`: the walk's weapon-hold part over invented packages (a home hold, the default, the parent and its
default load; another package's hold is skipped with a logged line).

**The slice gate** (installed Off strict, release build, under the machine lock): 5 ok / 2 NotYet / 7 errors, `links
ok 5/14`, `step share X = 5.783/14 = 41.3%`. Links 1, 3, 4, 5, 13 and 14 are past `GetWeaponHoldDef` and stop at
`maya:8` with an **error at the unbound `AnimNodeAimOffset.SetActiveProfileByName(name)`** (the note's predicted next
stop). Before the walk's extension they stopped at `NotRead`, "GetWeaponHoldDef over a weapon hold definition not
loaded at set-up".

**Gates.** `cargo test --workspace -j 8`: 3,659 passed / 0 failed / 57 ignored. `crate_edges_test.py`: OK.
