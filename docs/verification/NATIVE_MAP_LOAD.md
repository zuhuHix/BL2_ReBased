# Native map load: the level actor list, bringing a world up for play, the game info (2026-11-16)

AI-assisted (Claude Code). Lane W5-MAPLOAD, analyst part (clean room, docs/LANE_BRIEF.md). This note records in own
words what the engine does when a loaded map is brought up for play, read locally from `Borderlands2.exe` with the
project's Ghidra tooling (`tools/ghidra/run.ps1`, read-only queries; the output stays under ignored `local/analysis/`).
Nothing here was observed in the running game: every rule is **UNVERIFIED** until it is. The lane that wrote this note
implements none of the native rules below (the rules that are not read stay `NotYet` in Off); a fresh implementer
works from this note alone.

Confidence: **high** where the behaviour is a direct test or store the routine makes; **medium** where a field's
public UE3 name is matched by its position and use, not by the class layout; **low** where a call is only inferred.

## 1. The level's actor list (read from the packages, not from the executable)

This part is data, recovered from the packages with structural oracles (AGENTS.md rule 4), and is what `ow-world`'s
map load implements (decision 2026-11-16-76).

- A level is the export `TheWorld.PersistentLevel` (class `Engine.Level`) of a map file. Its tags (two counts) end
  with `None`; the untagged tail starts with the actor list, serialized as an owner reference (the level itself),
  then a count, then that many object references.
- Oracles, all ten Sanctuary level files: the owner is the level in every file; item 0 is that file's `WorldInfo`;
  item 1 is a null reference (the one null in each list); every other item is an export whose outer is the level;
  the bytes after the list are the level URL, whose first string is the protocol `unreal`. The exports under a level
  that the list leaves out are the model, its polys, the Kismet sequence, the model components, and 8 blocking-mesh
  actors and one wiring mesh in two files (the list, not the export table, is the level's).
- The list is the order `AllActors` walks (NATIVE_ENGINE_CORE.md, "Iterators").

## 2. Bringing the world up for play

One routine (it logs "Bringing %s up for play" and "Bringing up level for play took") takes the map URL and a
reset-time flag. In order:

1. **Time reset** (high for the stores, medium for the names). With the flag set, three consecutive float fields of
   the persistent level's world info are set to 0. By position and use they are, in public UE3 terms, `TimeSeconds`,
   `RealTimeSeconds` and `AudioTimeSeconds`; the mapping was not checked against the class layout. Two engine calls
   follow (not read).
2. **URL options** (low). The URL's options are walked; a `load` option is looked up, and when it is absent four fields
   of the world are taken from the URL (not read further).
3. **Setup calls** (low): several engine calls, one of them the game info routine of section 3.
4. **Begin play, once** (high for the test, medium for the names). The world tests a flag of the world info (the
   public name is `bBegunPlay`; the test is a single bit). When the flag is clear:
   - for every level of the world, an actor initialisation pass over its actor list, in list order, skipping null
     entries: an engine call per actor that refreshes its components; the role pair (`Role`, `RemoteRole`) swapped for
     an actor marked as owned remotely on a client; a few per-actor fields reset; one per-actor engine call (not
     identified);
   - the begun-play flag is set together with a second world info flag that stays set only while begin play is being
     routed and is cleared after (public name `bStartup`; medium);
   - when the world info has a game info and a flag of the game info is clear, one engine call with two outputs (not
     read; low);
   - the level's begin-play routing (section 2.1);
   - an engine call on a second object when it exists (not read).
5. Each level of the world then gets one more engine call (not read; low).
6. The elapsed time is logged; then, when the world info has a game info, a `BandwidthLimit=` URL option is passed to
   it and a memory-check option is handled (debug, not behaviour).

### 2.1 The level's begin-play routing

Two passes over the level's actor list, in list order, skipping null entries.

- **Pass 1.** For each actor that is not pending deletion and that is either not static or carries a second flag, one
  per-actor event (a virtual of the actor class; by public UE3 convention `PreBeginPlay`, **not confirmed**: the
  virtual's identity is the open item below). Then, for every actor, a query of its physics volume; the volumes found
  (excluding one flagged kind) are collected into a list.
- **Pass 2.** For each actor that is not pending deletion: a per-actor call with the collected list (by convention the
  zone or volume assignment; not confirmed); for an actor that is not static or carries the same second flag, each
  attached component flagged as attached gets one engine call (component begin play; low); then a second per-actor
  event (by convention `PostBeginPlay`; not confirmed) and class tests against two engine classes (not read).

Whether `SetInitialState` runs here, as it does in `Actor.Spawn` (NATIVE_ENGINE_CORE.md), was not read.

## 3. The game info

One routine (it holds the text "GameInfo class '%s' not found (or could not be loaded)"):

- It does nothing when the persistent level's world info already holds a game info (public name `WorldInfo.Game`;
  high, the same field the begin-play routine reads), or when a network or demo driver exists (medium).
- Otherwise it collects the URL options, then picks a class (medium):
  - when a world info field is zero and an engine check passes: the configured default game class, the game ini
    setting `Engine.GameInfo.DefaultGame`, loaded once and cached;
  - otherwise a class named through the URL options (the parse was not read), with the warning "GameInfo class '%s'
    not found (or could not be loaded). Default will be used." where it fails.
- It spawns the chosen class through the world spawn (NATIVE_ENGINE_CORE.md, "Actor.Spawn") and stores the new actor
  in the world info's game field. A failed spawn logs an error and leaves the field empty.

Which class Sanctuary gets in the running game, and what the class's script `InitGame` and login do, are script and
data, not part of this note.

## 4. The local player's play actor

`LocalPlayer.SpawnPlayActor(URL, out Error)` reads the URL text and the error output and calls the engine's routine
that makes the local player's play actor (not read); it returns that routine's result. The engine's list of local
players (`GamePlayers`, which `Actor.LocalPlayerControllers` and `GetALocalPlayerController` walk) is filled by the
engine before this call; where was not read.

## Open (for the implementer, or a follow-up analyst pass)

- The identities of the per-actor virtuals in section 2.1 (pass 1, pass 2's first call and second event). The query
  tool's `virtual:` form names a vtable slot of a script class; confirm them before implementing.
- The time-field mapping of section 2 step 1 against the `WorldInfo` class layout (`class_layout.py`).
- The game info's class choice (the world info field it tests, the URL parse) and the engine call after the begun-play
  flag.
- The local player routine of section 4, and where `GamePlayers` is filled.
- What the running game shows: the order of `PreBeginPlay` and `PostBeginPlay` across Sanctuary's actors, the game
  info's class, and `TimeSeconds` right after load (bl-sdk reads).
