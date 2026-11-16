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
  `IncludePlaythroughThree` (an int). It has no script functions and registers no natives. Its rules are the
  overrides of the value resolver's virtual get, get-base, add, remove and set-base (NATIVE_ATTRIBUTES.md section 6).
- The installed data has exactly one resolver of this class. It is the only value resolver of
  `D_Attributes.Balance.PlayThroughCount` (an `Engine.AttributeDefinition` whose context chain is one
  `NoContextNeededAttributeContextResolver`), and its `IncludePlaythroughThree` is **0**.
- The rule reads `WillowGameReplicationInfo.CurrentPlaythrough` (an int; 0 Normal, 1 TVHM, 2 UVHM, as
  NATIVE_CONTROLLER_HELPERS.md reads the script). `PlaythroughOverride`, the field declared right after it, is not
  read.

## 2. Which slots the class overrides

Of the five virtual operations, the class overrides **get** only. Get-base, add, remove and set-base are the base
class's own entries, the same ones `ConstantAttributeValueResolver` uses: get-base calls get with the same arguments,
and add, remove and set-base change nothing and answer false. (This is the class default that the value resolver
module already describes for every class but `ObjectPropertyAttributeValueResolver`.)

## 3. Get

Inputs: the resolver object, the resolved context and the chain's previous value. **The context and the previous
value are ignored.** The only inputs are the world's replication info and the resolver's `IncludePlaythroughThree`.

1. Start from a count of 1.
2. Find the replication info the same way `WillowPlayerController.GetCurrentPlaythrough` does
   (NATIVE_CONTROLLER_HELPERS.md): the engine's single global world, its `WorldInfo` (the persistent level's, without
   the streaming-level variant), that object's `GRI`, which must be a `WillowGameReplicationInfo` (a class test that
   walks the super-class chain). If any link is missing, or `GRI` is not a `WillowGameReplicationInfo`, the count
   stays 1.
3. Read `CurrentPlaythrough` from it (through a virtual accessor of the replication info that returns the field as
   stored: no override, no clamp). If it is 0 or more, the count is `CurrentPlaythrough + 1`. A negative value leaves
   the count at 1.
4. If `IncludePlaythroughThree` is 0: clamp the count to the range 1 to 2 (below 1 gives 1, above 2 gives 2).
   If it is any other value: no clamp; the count from step 3 stands, so a third playthrough gives 3 (and a larger
   stored value a larger count).
5. Return the count as a float.

What this gives with the installed field (0): Normal 1, TVHM 2, **UVHM 2**. With a non-zero field: 1, 2, 3.

No side effects: nothing is written, no notification, no random draw.

## 4. Edge cases

- No world, no world info or no replication info (a front-end or offline evaluation): 1.
- A replication info of another class: 1.
- Get-base is the same number as get (section 2).
- The context chain still runs before the resolver (NATIVE_ATTRIBUTES.md section 1); for the installed attribute it
  is NoContextNeeded, which always resolves. The resolved context does not change the answer.
- A network client reads its local copy of `CurrentPlaythrough`; single player does not need the distinction.

## 5. How it was read (for a reviewer)

The class's vtable was found from its registration, and the get, get-base and write slots were compared with the
Constant resolver's: the get entry differs and the other four are identical. The replication-info class used in the
class test was identified from its registration (name `WillowGameReplicationInfo`, package `WillowGame`). Its
registered instance size equals the size `class_layout.py` computes for that class, a structural check of the layout.
The accessor was located in that class's vtable through its constructor; it returns the field that `class_layout.py`
places as `CurrentPlaythrough`. The field read in step 4 is the resolver's only declared field. One link is not
checked by layout: `class_layout.py` cannot lay out `Engine.WorldInfo` (it has a map property the tool does not
handle), so that the field read there is `GRI` rests on the same lookup in the `GetCurrentPlaythrough` note.

## 6. Implementer checklist (Off mode)

1. Get ignores the context and the previous value.
2. Count = 1; with a `WillowGameReplicationInfo` reachable as the world's `GRI` and `CurrentPlaythrough >= 0`,
   count = `CurrentPlaythrough + 1`. `ow-attr` has no query for the world's replication info yet: the
   `AttrWorld::game_info` / `EngineQuery` contract would need a way to reach it (`ow_balance::OfflineWorld` already
   keeps one with `CurrentPlaythrough` set from its snapshot). A world that models no replication info reads 1, per
   section 4.
3. `IncludePlaythroughThree == 0`: clamp to 1..2. Otherwise no clamp.
4. Get-base = get.
5. Add, remove and set-base: false, nothing written (the module's class default, `storage` = `None`).

## 7. What would confirm it (live, bl-sdk)

Real-game question 1 (docs/rust-port/RESEARCH_SYSTEMS_MAP.md section 8). With a character in each mode (saves backed
up):

- In Normal, TVHM and UVHM, read `GetCurrentPlaythrough()` on the player controller and
  `D_Attributes.Balance.PlayThroughCount.GetValue(<player controller>)`: expected 0 / 1, 1 / 2 and 2 / **2**.
- In UVHM, also read it after setting the resolver's `IncludePlaythroughThree` to 1 on the live object (and set it
  back): expected 3.
- A visible check: a value the data keys on `PlayThroughCount == 2` (for example an ammo pickup amount) should take
  the same value in TVHM and UVHM.
