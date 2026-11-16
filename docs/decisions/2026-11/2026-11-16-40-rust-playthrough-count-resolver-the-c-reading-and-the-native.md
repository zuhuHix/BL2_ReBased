## 2026-11-16: Rust playthrough count resolver: the C++ reading and the native rule noted

AI-assisted (Claude Code). Lane FIX-PLAYTHROUGH adds `WillowGame.PlayThroughCountAttributeValueResolver` to `ow-attr`
(`crates/ow-attr/src/value_resolvers/playthrough_count.rs`, registered in `value_resolvers::register`; tests in
`playthrough_count_tests.rs`; the class added to the synthetic world in `testworld.rs`). Before it, `ow-attr` answered
`NotYet { NotRead }` for this class in both parity modes, which stopped W6-COMBAT's installed spawns, the installed Fire
turn-in's experience and FIX-NOUN's init definitions. No parsing behaviour changes. **Every rule here is UNVERIFIED in
the running game**; no real-game session ran in this lane.

**The class (installed packages).** `WillowGame.PlayThroughCountAttributeValueResolver` declares one int,
`IncludePlaythroughThree`, and no script functions or natives. The installed data has one resolver of the class: the
only value resolver of `D_Attributes.Balance.PlayThroughCount` (context chain NoContextNeeded), with
`IncludePlaythroughThree` 0.

**Clean room.** The records did not read this class, and it has no script, so its rule is native. This lane read it
with Ghidra 12.1.4 (`tools/ghidra/run.ps1`, read-only queries against the existing project of `tools/ghidra_import.ps1`;
this PC's JDK is Temurin 21.0.13+9, passed with `-Jdk`) and wrote the own-words note
`docs/verification/NATIVE_PLAYTHROUGH_ATTRIBUTES.md` (scanned with the clean-room regex of docs/LANE_BRIEF.md: no
hits). Raw output stays under ignored `local/analysis/`. **This lane implemented nothing it read in Ghidra**: the Off
rule waits for a fresh implementer working from the note alone.

**Shipped.**

- `ParityMode::Reference`: the C++ reading, **1** (`src/progression.cpp` `operandValue` answers a `PlayThroughCount`
  operand with `AttributeContext::playThroughCount`, 1 by default in `src/progression.hpp`, never set otherwise; the
  mission bridge refuses any other count). The context, the previous value, `IncludePlaythroughThree` and the world's
  playthrough are not read. Get and get-base are the same read. The C++ matches the attribute by path suffix and skips
  its chain; the port runs the chain and answers at the resolver. On the installed data these are the same attribute,
  and its chain always resolves: of the 662,212 census entries, exactly 1 path ends in `PlayThroughCount`
  (`D_Attributes.Balance.PlayThroughCount`, an `Engine.AttributeDefinition`), and exactly 1 object is of this class
  (that attribute's only value resolver). Where the C++ refuses (the attribute read outside an expression), the port
  answers 1.
- **"Always 1" is a new Reference stand-in** (architecture R3; the hub adds its ARCHITECTURE.md section 6.1 row).
  Pinned by the installed Fire reward percentage and the init-definition sweeps below, and by
  `playthrough_count_tests.rs`. Replaced by FIX-PLAYTHROUGH-OFF (the Off rule, from the note). Where it differs from
  the native rule: TVHM and UVHM read 2 natively and 1 here; the C++ refuses the attribute outside an expression,
  where the port answers 1.
- `ParityMode::Off`: get and get-base are `NotYet { NotPorted }`, "native rule in NATIVE_PLAYTHROUGH_ATTRIBUTES.md,
  awaiting clean-room implementation".
- Writes (add, remove, set-base) are the same `NotYet { NotPorted }` in both modes (the C++ has none), as FIX-NOUN did.
- The mode is the world's store's (`DefStore::mode`); a world without a store runs `Off`.

**The native rule in one line (the note has it in full).** Count = `CurrentPlaythrough + 1` of the world's
`WillowGameReplicationInfo` (1 with none, or for a negative value), clamped to 1..2 when `IncludePlaythroughThree` is
0. With the installed field, UVHM reads **2**, not 3. The context and the previous value are ignored. Get-base calls
get, and the writes are the class default (false). Live checks that would settle it are in the note's section 7
(real-game question 1).

**Installed game (local output under `local/p2/fix-playthrough/`, no game data here).** W3-ATTREVAL's sweep, before
(`origin/main` 413eafd0) and after, run by this lane:

- Controller as the source, Reference: attribute definitions with a value 260 to 273 of 858 (13 stops gone), init
  definitions 95 to 117 of 300 (30 stops gone; 22 evaluate, the other 8 stop at the next unported class:
  `GearboxFramework.ActorAttributeContextResolver` 7, `BalancedActorAttributeContextResolver` 1). The Fire mission's
  reward percentage evaluates to 0.05: `trunc(7918 x 0.05)` = 395 at stage 8, the amount SANCTUARY_RPG_MISSION.md
  records as confirmed on 2026-10-02. That was a Normal playthrough, where the count is 1 by every reading, so it
  does not exercise the count rule.
- A WillowMind as the source (FIX-NOUN's sweep, without the Noun resolver), Reference: 225 to 238 attributes, 88 to
  109 init definitions (29 stops gone; 21 evaluate, the same 7 + 1 stop next).
- Off: every count unchanged in both sweeps (260 / 95, 225 / 88); the 13 and 30 (29) stops now name the note
  (`NotPorted`) instead of `NotRead`.
- W6-COMBAT's installed steps (W6-COMBAT's own 15-step list) on a local merge of `origin/main` 413eafd0,
  `feature/rust-w6-combat` 177a00d5, `fix/rust-noun-resolver` 68c27226 and this lane. Reference: all 5 spawns get
  past the Noun and PlayThroughCount resolvers and stop at the next layer, the Vm world's game info ("needs the map
  load that makes it (WorldQuery::set_game_info), which is not built yet"). Still 0 rows against the C++'s 29;
  `player:8` succeeds, and every step that needs a pawn fails for lack of one. Off: the spawns stop before any
  attribute, at the archetype reference `Default__WillowAIPawn.SpecialMoveComponent0` ("inside a class default
  object"), as FIX-NOUN found; this lane does not change that.

**Open.** The Off rule (from the note; it needs a way for `ow-attr` to reach the world's replication info). A
frozen-contract wording conflict for the hub's docs pass (not edited here): `Playthrough::count()`
(`crates/ow-balance/src/types.rs:131`) and its line in ARCHITECTURE.md section 3.6
(`docs/rust-port/ARCHITECTURE.md:898`) call `index + 1` "the PlayThroughCount reading" (UVHM 3). By the note the
attribute is that count clamped to 1..2 when `IncludePlaythroughThree` is 0, as installed (UVHM 2); the docs should call
it the 1-based index only and point at the note. The next blockers named above: `ActorAttributeContextResolver` and
`BalancedActorAttributeContextResolver` for init definitions, and the Vm world's game info for the spawns. Real-game
question 1.
