## 2026-11-16: Rust noun attribute value resolver: the native Off rule from the clean-room note

AI-assisted (Claude Code). Lane FIX-NOUN-OFF fills `ParityMode::Off` of `GearboxFramework.NounAttributeValueResolver`
in `crates/ow-attr/src/value_resolvers/noun.rs` (tests in `noun_tests.rs`). No parsing behaviour changes. **Every rule
here is UNVERIFIED in the running game**; no real-game session ran.

**Clean room, plainly.** This lane is the implementer of the two-agent rule of docs/LANE_BRIEF.md ("Clean room
(native code)"). It worked from `docs/verification/NATIVE_NOUN_ATTRIBUTES.md` alone, as rewritten in review r1 and r2
of PR #338 (commit e68a4dd), and never saw the analysis: no `local/analysis/`, no Ghidra project, no REA output, no
decompiler or disassembler run, and no other worktree's `local/`. An earlier start was halted by the hub before any
code was written, when the note was being rewritten as outcome rules. Where the note says a fact was not read, this
lane leaves that part `NotYet` and asks the question in its handoff instead of choosing. Reference mode (the C++
reading of decision 2026-11-16-37) is unchanged; only its write message now says the C++ has no noun write.

**Shipped (Off mode).**

- **Key:** the name of the resolver's outer object, compared as `NameId` (the number included). No outer: reads
  0.0, writes false. The outer need not be an attribute definition here (Reference still requires one).
- **Provider:** a context whose class declares `NounState`; `ow-object` models no interfaces, and checklist item 2
  makes the two the same. Anything else reads 0.0 and refuses writes (`Refusal::NotWritable`), whatever the
  strategy.
- **Strategy 0:** the entry is the first `NounState` element whose `NounName` is the key. Reads and writes both
  create a missing entry. `DefaultValue` is evaluated on the context first, then the element goes on the end of the
  array as it stands after that evaluation, with: the key; `Value` = `BaseValue` = `d`; an empty stack; strategy 0;
  slot -1.
  - Get gives `Value` and get-base gives `BaseValue`; the chain's previous value is ignored.
  - Add refuses the same modifier object (identity) with `Duplicate`. Remove takes every copy, or answers
    `NotPresent` and changes nothing.
  - Set-base writes the base. Each write then recomputes `(base + pre) * ((1 + up) / (1 - down)) + post` in single
    precision.
  - That is `stack::combine` with `CombineForm::Vm5F` and `NumKind::Float`, used whatever combine form the call
    carries, because the note fixes single precision. Other modifier types are skipped; no truncation, no clamp.
  - No change notification is raised.
- **Strategy 1 or 2 on a provider** (only `IsPhaselocked`, on a pawn, in the installed data): `NotYet { NotPorted }`.
  The note's rows for it depend on two facts it marks as not read: which arrays each provider hands over (only
  "expected"), and what the "replicated copy changed" call does. A strategy outside 0..=2 is `NotYet { NotRead }`.
- **An entry that already holds a replicated slot** (`IndexInReplicationArray` not -1) reads normally. Writing it is
  `NotYet { NotPorted }` before anything changes, because the mirror needs the same two facts.
- **Net mode:** the attribute worlds are standalone (single player), so the client row of the table is not built
  (checklist item 6). Every row built here is the same in all net modes.
- **A null modifier** cannot reach the resolver: `AttributeSystem::add_modifier` and `remove_modifier` take an
  object reference. The note's "a null modifier returns false" is therefore the caller's (the VM's).

**The `Storage` addition** (`crates/ow-attr/src/api.rs`, additive): `Storage::Noun { owner, state, index }` is element
`index` of the `NounState` array property `state` on the provider `owner`. The variant itself is `#[non_exhaustive]`,
because the replicated slot may need a field once its mirror is read. `storage.rs` sends a noun storage to
`value_resolvers::noun` (add, remove, set-base and the by-name read) and gives it no notification.
`effects.rs` counts a noun storage as holding the modifier object, so `ApplyAttributeEffects` does not free a
modifier a noun stack holds. `storage::stack_value` and the `noun` module became `pub(crate)`. No file outside
`crates/ow-attr/` changed.

**Tests (synthetic, rule 9):** 23 Off tests in `noun_tests.rs`, each naming its note section. They cover:

- the created entry;
- get and get-base, the default not re-evaluated, the previous value ignored;
- the key: first match, number included, any outer, shared names;
- no outer, and a context that is not a provider;
- the evaluation-before-append ordering, and a fresh context;
- add (dedupe by identity), remove (every copy; none there), remove and set-base on a fresh provider;
- the recompute, bit for bit, on inputs where WeaponGen differs; no clamp, no truncation;
- no notification;
- a bad modifier leaving the entry unchanged;
- checklist item 8;
- applied effects staying on a noun stack;
- strategies 1, 2 and 3;
- a slotted entry.

The test world counts notifications (`World::notified`).

**Installed game** (a local harness under ignored `local/p2/fix-noun-off/`; no game data here). The harness is new:
the FIX-NOUN lane's own harness is not in the repository. It counts every `Engine.AttributeDefinition` (708),
`Engine.AttributeInitializationDefinition` (416) and `WillowGame.AIClassDefinition` (849) export of the installed
game, so its totals are not the 858 / 300 / 328 of decision 2026-11-16-37. Before (main 32fc9ae, PR #338 as landed;
the same numbers on e68a4dd) and after (this change rebased on it) were run with
the same harness.

- **Mind-sourced sweep.** One WillowMind, whose AI class is the first by path that lists
  `Attribute_HealthMultiplier`.
  - Off attributes with a value: 282 before, 294 after. The 12 noun stops are gone.
  - Off init definitions that evaluate: 133 before, 141 after. Of the 11 noun stops, 8 now evaluate and 3 stop at
    the next unported step.
  - Off now gives the same tallies as Reference, which is unchanged (294 and 141, before and after).
- **Noun sweep.** The 15 noun attributes on a fresh mind for each of the 849 AI classes.
  - In Off, the 10,188 mind reads (12 nouns × 849) were all `NotYet { NotPorted }` before. All 10,188 evaluate now.
    The other 2,547 reads (the weapon and pawn nouns) resolve no context on a mind, before and after.
  - Without starting values, 8,245 of the 10,188 equal the Reference reading, as checklist item 8 predicts: the first
    get gives `d`.
  - The harness then applied each AI class's starting values through set-base first, a stand-in for the script's
    `ApplyAttributeStartingValues` (not its order). 10,185 of 10,188 then equal Reference.
  - The 3 that differ are on AI classes that list the same noun twice. The C++ reading takes the first entry; the
    stand-in's sets in list order leave the last.
- **Provider cross-check.** In the installed reflection, the 12 classes that declare `NounState` are exactly the 12
  that implement `INounAttributeProvider`, subclasses included (all 3,339 code classes laid out).

**Open.**

- The strategy-1/2 rows and the mirror wait for two answers from the analyst:
  - which arrays each provider hands over;
  - what the replicated-copy-changed call does.
- Who calls `ApplyAttributeStartingValues`, and when (checklist item 8): until that script runs, Off reads give the
  default, not the class value.
- A class that lists a noun twice: whether the game's script leaves the first or the last value is not read (the
  C++ reading takes the first).
- The live checks are in note section 9.
