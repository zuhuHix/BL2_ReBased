## 2026-11-16: Rust playthrough count resolver: the native Off rule from the clean-room note

AI-assisted (Claude Code). Lane FIX-PLAYTHROUGH-OFF fills `ParityMode::Off` of
`WillowGame.PlayThroughCountAttributeValueResolver` in `crates/ow-attr/src/value_resolvers/playthrough_count.rs`
(tests in `playthrough_count_tests.rs`). It adds one engine query for the world's game replication info, and the
offline world's answer to it. No parsing behaviour changes. **Every rule here is UNVERIFIED in the running game**; no real-game session ran.

**Clean room, plainly.** This lane is the implementer of the two-agent rule of docs/LANE_BRIEF.md ("Clean room
(native code)"). It worked from `docs/verification/NATIVE_PLAYTHROUGH_ATTRIBUTES.md` alone, as landed with PR #341
(752ffb2), and never saw the analysis: no `local/analysis/`, no Ghidra project, no REA output, no decompiler or
disassembler run, and no other worktree's `local/`. Reference mode (the C++ reading of decision 2026-11-16-40,
"always 1") is unchanged; only its write message now says the C++ has no write for this class.

**Shipped (Off mode).** CP is the replication info's `CurrentPlaythrough`, as stored.

- **Get** follows the note's section 3 table:
  - no replication info, or one that is not a `WillowGameReplicationInfo` (a subclass counts): 1.0;
  - CP < 0: 1.0;
  - CP >= 0 and `IncludePlaythroughThree` 0: CP + 1, clamped to 1..2;
  - CP >= 0 and the field not 0: CP + 1.
- The context and the chain's previous value are not read (checklist item 1). The field is read only for the last
  two rows.
- **CP + 1 is the wrapping 32-bit sum** (section 4; checklist item 4 allows wrapping or refusing, and this lane took
  wrapping because it is the note's reading of the arithmetic). So CP = `i32::MAX` reads 1.0 with the field at 0 and
  -2147483648.0 otherwise. The int becomes an `f32` rounded to nearest (`eval::number`).
- **Get-base is get** (section 2).
- **Writes** (add, remove, set-base): the class default, `storage` = `Ok(None)`. Each answers false and nothing
  changes (section 2, checklist item 6). This lane built them as the note says rather than leaving them `NotYet`.
- **Value kinds.** `CurrentPlaythrough` and `IncludePlaythroughThree` must be `Int`, as note section 1 reads them.
  Any other kind, a byte included, is `BadData` (review r1 of PR #347; the offline world's synthetic fixture now
  declares `CurrentPlaythrough` as an int).
- **A replication info whose ancestry ends at an engine-native stand-in** cannot answer the class test, so the read
  is `NotYet { NotRead }` (the shared `is_a` rule of the context resolvers), not the first row's 1.0.
- **Reference's write refusal** keeps its kind and record (`NotYet { NotPorted }`, the note); only its text changed,
  to "the C++ has no write for this class" (accepted in review r1 of PR #347).
- **Not in the note (this lane's choice):** a world that cannot answer the query (`NotYet`, for example the trait's
  default `LayerAbsent("ow-world")`) stops the read with that `NotYet`. Only `Ok(None)` is the first row. "This
  world does not say" is not "this world has no replication info".

**The query seam** (granted to this lane; additive). `ow_attr::api::EngineQuery::GameReplicationInfo` (a unit
variant; the enum was already `#[non_exhaustive]`) asks for the world's GRI. `Ok(None)` means the world has none;
the source is passed but not read, and the reader checks the class. In `ow-world/src/query.rs`,
`QueryKind::GameReplicationInfo` maps to it. With no handler registered it is `NotYet { LayerAbsent }`, naming "the
map load that makes the game replication info". No handler exists yet. `ow_balance::OfflineWorld::engine_query`
answers the kind with its own replication info, whatever the source (one match arm, granted by the hub in review).
No existing kind or method changed.

**Tests (synthetic, rule 9).**

- 18 Off tests in `playthrough_count_tests.rs`, each naming its note section. They cover:
  - no replication info, a replication info of another class, and a subclass;
  - CP -1, -7 and `i32::MIN`;
  - the clamped and unclamped rows at CP 0, 1, 2 and 5;
  - CP = `i32::MAX` with the field 0 and 1;
  - rounding to the nearest `f32`;
  - the table as a pure function;
  - the previous value ignored, and the context ignored (the query's source is checked);
  - a `== 2` conditional, the same in TVHM and UVHM;
  - get-base = get;
  - writes false and unchanged;
  - a world that cannot answer, a byte and a float playthrough (bad data), and a native stand-in ancestry
    (`NotRead`).
- 1 new Reference test: the world's playthrough is not read.
- 1 new `ow-world` test: the kind's `LayerAbsent` and a registered handler.
- 1 new `ow-balance` test: the offline world answers the query for any source, and the count over its synthetic
  fixture reads 1 / 2 / 2 (field 0) and 1 / 2 / 3 (field 1) in Normal / TVHM / UVHM. The fixture gained the
  resolver class and a `playthrough_count_attr` helper.
- The test world answers the query through a local adapter, so `testworld.rs` is untouched.

**Installed game** (a local harness under ignored `local/p2/fix-playthrough-off/`; no game data here). Neither
earlier lane's harness is in the repository, so this one is new. It counts every exact-class
`Engine.AttributeDefinition` (708) and `Engine.AttributeInitializationDefinition` (416) export, as FIX-NOUN-OFF's
did, so its totals are not decision 40's 858 / 300. Sources: the `OfflineWorld` player controller, and a bare
`WillowMind` (no AI class set). It runs playthroughs 0, 1 and 2. "Before" is the harness built on `origin/main`
752ffb2; "after" is the same harness on this change.

- **Reference:** unchanged before and after, in every playthrough. Controller: 328 attributes, 170 init
  definitions. Mind: 306 and 168. The count reads 1.
- **Off, before:** controller 316 and 143; mind 282 and 133. The count stops (`NotPorted`, "awaiting clean-room
  implementation").
- **Off, after.** The plain `OfflineWorld`, which now answers the query, gives exactly the same counts as the
  harness's adapter that answered it before the arm was added (rerun after the arm, under `after2/`):
  - The count reads 1 / 2 / 2 in Normal / TVHM / UVHM, the note's section 3 with the installed field 0.
  - Controller attributes: 316 to 328 (12 stops gone), equal to Reference.
  - Controller init definitions: 143 to 170 in Normal, equal to Reference. In TVHM and UVHM it is 169: one
    definition takes a different branch there and stops at the offline world's missing random source
    (`AttributeVariance`).
  - Mind: attributes 282 to 294, init definitions 133 to 158 (157 in TVHM and UVHM). The rest of the gap to
    Reference is the Noun resolver's Off stops (FIX-NOUN-OFF, decision 2026-11-16-43, which landed after 752ffb2,
    the base these sweeps ran on).
  - 3 stops whose text mentions the count's name are left. They are present in Reference too, so they do not stop
    at this resolver.
- **Rerun after review r1** (byte arm dropped; tree on main 20f40c27, which holds FIX-NOUN-OFF; under `after3/`).
  - The count still reads 1 / 2 / 2, so the installed reflection declares `CurrentPlaythrough` as an int.
  - Controller numbers: unchanged.
  - Mind: Off attributes 306, init definitions 168 (167 in TVHM and UVHM), now equal to Reference.

**Open.**

- The Vm world needs the map load to register a handler for the kind.
- The note says nothing on a world that cannot answer (handled as `NotYet` above), or on a field or playthrough of a
  kind other than int.
- Real-game question 1: the live reads are in the note's section 7.
- The `Playthrough::count()` wording conflict that decision 40 hands to the hub's docs pass stands.
