# Native playthrough attribute: `PlayThroughCountAttributeValueResolver` (draft, 2026-11-16)

AI-assisted (Claude Code), analyst lane FIX-PLAYTHROUGH. Read from the game executable with Ghidra 12.1.4
(`tools/ghidra/run.ps1`, read-only queries against the project made by `tools/ghidra_import.ps1`), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. Names, field types and the field
layout come from the cooked packages (the export tables, `tools/ghidra/class_layout.py`). **Every rule is UNVERIFIED
in the running game**; nothing here was observed live.

This note is the hand-off to the implementer of the `ParityMode::Off` rule. The analyst lane implemented only the
C++ reading (`ParityMode::Reference`); the Off rule is to be built by a fresh implementer from this note alone
(docs/LANE_BRIEF.md, "Clean room (native code)"). Section 6 lists everything to build.

## 1. The shape (from the packages)

- `WillowGame.PlayThroughCountAttributeValueResolver` extends the value resolver base class and declares one field,
  `IncludePlaythroughThree` (an int). It has no script functions and registers no natives. Its rules are its answers
  to the value resolver's five operations: get, get-base, add, remove and set-base (NATIVE_ATTRIBUTES.md section 6).
- The installed data has exactly one resolver of this class. It is the only value resolver of
  `D_Attributes.Balance.PlayThroughCount` (an `Engine.AttributeDefinition` whose context chain is one
  `NoContextNeededAttributeContextResolver`), and its `IncludePlaythroughThree` is **0**.
- The rule reads `WillowGameReplicationInfo.CurrentPlaythrough` (an int; 0 Normal, 1 TVHM, 2 UVHM, as
  NATIVE_CONTROLLER_HELPERS.md reads the script). `PlaythroughOverride`, the field declared right after it, is not
  read.

## 2. What the class defines

The class defines its own **get** only. The other four operations behave as the base class's do (the same as
`ConstantAttributeValueResolver`): get-base gives the same number as get, and add, remove and set-base change nothing
and answer false. That is the class default the value resolver module already describes for every class but
`ObjectPropertyAttributeValueResolver`.

## 3. Get

The answer depends only on the world's replication info (the `GRI` of the world's `WorldInfo`, the object
`WillowPlayerController.GetCurrentPlaythrough` reads, NATIVE_CONTROLLER_HELPERS.md) and on the resolver's
`IncludePlaythroughThree`. **The resolved context and the chain's previous value do not change it.** Nothing is
written and nothing is drawn.

Writing CP for that replication info's `CurrentPlaythrough`, as stored:

| Case | Result |
|---|---|
| No world, no `WorldInfo` or no `GRI`, or a `GRI` that is not a `WillowGameReplicationInfo` (a subclass counts) | 1.0 |
| CP < 0 | 1.0 |
| CP >= 0, `IncludePlaythroughThree` = 0 | CP + 1, clamped to 1..2 (`max(1, min(CP + 1, 2))`) |
| CP >= 0, `IncludePlaythroughThree` not 0 | CP + 1 |

`PlaythroughOverride` plays no part. With the installed field (0): Normal 1, TVHM 2, **UVHM 2**. With a non-zero
field: Normal 1, TVHM 2, UVHM 3.

## 4. Edge cases

- A front-end or offline evaluation with no replication info: 1.0 (the first row).
- Get-base gives the same number as get (section 2).
- The context chain still runs before the resolver (NATIVE_ATTRIBUTES.md section 1); for the installed attribute it
  is NoContextNeeded, which always resolves.
- CP = `i32::MAX`: CP + 1 is a 32-bit signed sum and wraps to `i32::MIN`. With a non-zero field the result is
  -2147483648.0; with the field 0 the wrapped value is below 1, so the result is 1.0, not 2. Large counts are
  rounded to the nearest `f32`. Both are readings of the arithmetic only (UNVERIFIED); no shipped value comes near.
- A network client reads its local copy of `CurrentPlaythrough`; single player does not need the distinction.

## 5. How it was read (for a reviewer)

- Only get is the class's own; get-base and the three writes are the base class's, shared with the Constant
  resolver.
- The registered instance size of `WillowGameReplicationInfo` equals the size `class_layout.py` computes for it, and
  the field the rule reads is the one `class_layout.py` places as `CurrentPlaythrough`. The `IncludePlaythroughThree`
  read is the resolver's only declared field.
- Not checked by layout: `class_layout.py` cannot lay out `Engine.WorldInfo` (it has a map property the tool does
  not handle), so that the world info's field is `GRI` rests on the `GetCurrentPlaythrough` note.

## 6. Implementer checklist (Off mode)

1. Get ignores the context and the previous value.
2. The table in section 3. `ow-attr` has no query for the world's replication info yet: the `AttrWorld::game_info` /
   `EngineQuery` contract would need a way to reach it (`ow_balance::OfflineWorld` already keeps one with
   `CurrentPlaythrough` set from its snapshot). A world that models no replication info reads 1.0.
3. **`ow_balance::types::Playthrough::count()` is not this attribute's value.** It is `index + 1` (UVHM 3); the
   attribute applies the clamp of section 3 on top of that when `IncludePlaythroughThree` is 0 (UVHM 2), as it is
   in the installed data. Its documentation (`types.rs`, ARCHITECTURE.md section 3.6) calls it "the PlayThroughCount
   reading"; that wording is to be corrected in the hub's docs pass.
4. Do not overflow on CP + 1 (Rust's default `+` panics in a debug build): use the wrapping sum of section 4, or
   refuse the value as bad data, and say which.
5. Get-base = get.
6. Add, remove and set-base: false, nothing written (the module's class default, `storage` = `None`).

## 7. What would confirm it (live, bl-sdk)

Real-game question 1 (docs/rust-port/RESEARCH_SYSTEMS_MAP.md section 8). With a character in each mode (saves backed
up):

- In Normal, TVHM and UVHM, read `GetCurrentPlaythrough()` on the player controller and
  `D_Attributes.Balance.PlayThroughCount.GetValue(<player controller>)`: expected 0 / 1, 1 / 2 and 2 / **2**.
- In UVHM, also read it after setting the resolver's `IncludePlaythroughThree` to 1 on the live object (and set it
  back): expected 3.
- A visible check: a value the data keys on `PlayThroughCount == 2` (for example an ammo pickup amount) should take
  the same value in TVHM and UVHM.
