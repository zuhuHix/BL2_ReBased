# Native map load: the level actor list, bringing a world up for play, the game info (2026-11-16; second pass 2026-11-19)

AI-assisted (Claude Code). Lane W5-MAPLOAD, analyst part (clean room, docs/LANE_BRIEF.md), with a second analyst pass by
lane MAP-LOAD-B (2026-11-19, decision 2026-11-19-13). This note records in own words what the engine does when a loaded
map is brought up for play, read locally from `Borderlands2.exe` with the project's Ghidra tooling
(`tools/ghidra/run.ps1`, read-only queries, plus a small local read-only table reader; the output stays under ignored
`local/analysis/`). Nothing here was observed in the running game: every rule is **UNVERIFIED** until it is. Neither
analyst lane implements the native rules below (they stay `NotYet` in Off); a fresh implementer works from this note
alone.

Confidence: **high** where the behaviour is a direct test or store the routine makes and the field is named by the
class layout (`tools/ghidra/class_layout.py`) or an event by the engine's own name table; **medium** where a name is
matched by position, use or public UE3 convention only; **low** where a call is only inferred.

**How the second pass named things.** Fields: the byte position a routine reads or writes, checked against the class
layout computed from the packages. `WorldInfo` cannot be laid out whole (it declares map properties the tool does not
size), but both of its map properties come after every field used here, and `WorldInfo.Game` falls where the bring-up
stores the game info whatever size a map property is given: an oracle for that part of the layout. The
`WillowGameInfo` layout's total size equals the size its class registration passes. Script events: the engine fills a
table of global names from literal strings at start-up; an event a routine raises was named by matching the global it
passes to the event call with that table.

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

One routine (it logs "Bringing %s up for play (%i) at %s" and "Bringing up level for play took: %f") takes the map URL
and a reset-time flag. The map load calls the game info routine of section 3 first (only when the world's net mode is
not the client one), then sets one flag of the new game info, then calls this routine. In order:

1. **Time reset** (high). With the flag set, `WorldInfo.TimeSeconds`, `RealTimeSeconds` and `AudioTimeSeconds` are
   set to 0 (named by the layout: the three fields the first pass matched by position). Two engine calls follow (not
   read). Which value the map load passes for the flag was not separated (low); on a fresh load the three are 0 anyway
   (decision 2026-11-19-04's installed read).
2. **URL options** (medium). The URL's options are walked. When the URL has no `load` option, the world keeps its own
   copy of the URL's parts (protocol, host, port, map, options, portal and the valid flag); with `load` it keeps the
   copy it has. The world's URL is engine state, not a script field.
3. **Setup calls** (low): two of the world's strings are cleared, then several engine calls (rendering, physics and
   navigation set-up by their use; not read). None of the bring-up's direct callees uses one of the engine's script
   event names except the two named below (deeper callees were not scanned). Each streaming level entry gets two
   per-entry fields cleared.
4. **Begin play, once** (high). The routine tests `WorldInfo.bBegunPlay`. When it is clear:
   - one engine object gets one flag set (not identified; low);
   - the actor initialisation pass (section 2.2);
   - `bBegunPlay` and `bStartup` are set together;
   - when `WorldInfo.Game` is set and its `bScriptInitialized` is clear: the script event **`InitGame(Options, Error)`**
     on the game info, with the URL's options joined as text and an error string (named by the name table; the
     options text is the `?`-joined option list of section 3);
   - the level's begin-play routing (section 2.1);
   - one call on a second engine object when it exists (not read; low);
   - `bStartup` is cleared again.
5. Each level of the world then gets one more engine call (not read; low).
6. The elapsed time is logged. Then, when the world info has a game info: a `BandwidthLimit=` option is passed to it
   (a network setting); and when the game info has an `AutoTestManager` (`MyAutoTestManager`) with
   `bUsingAutomatedTestingMapList` set, the manager gets the script event `StartMenuTimer`, or with
   `bCheckingForMemLeaks` set, a memory-leak console command runs. Both are automated-testing support; neither applies
   to normal play.

### 2.1 The level's begin-play routing

Two passes over the level's actor list, in list order, skipping null entries. **Correction to the first pass:** the
per-actor flag tested first is `bScriptInitialized` (an actor already script-initialised gets none of the events), not
"pending deletion". The routing takes a mode argument; when it is set, static actors (`bStatic`) are skipped entirely.
Which value the bring-up passes was not separated (low).

An actor **routes** when it is not `bStatic`, or when it is `bStatic` and has **`bRouteBeginPlayEvenIfStatic`** set
(high: both bits named by the `Actor` layout; this is the "second flag" of the first pass). The installed class
default of `WillowWeapon` has it set, for example.

- **Pass 1.** For each actor not yet script-initialised that routes: the actor's "pre" virtual (below). Then, for every
  actor: a query of its physics volume; each volume found that does not have `bBlockActors` set is added to a list.
- **Pass 2.** For each actor:
  - not yet script-initialised: the volume assignment, with the list (an engine virtual that works through collision
    overlap and touch lists; no script event; not read further);
  - that routes: each of its `Components` with `bAttached` set gets one engine call (component begin play; low);
  - not yet script-initialised and routes: the actor's "post" virtual (below); then, when the actor is a `Pawn` or a
    `SkeletalMeshActor` (or a subclass; the two classes named by their registrations), one engine registration call
    (hands it to an engine subsystem when that exists; no script event);
  - not yet script-initialised and does not route (a static actor without the flag): `bScriptInitialized` is set, and
    nothing else happens to it.

**The "pre" and "post" virtuals** (high for the events, medium for the slot identity in general). In the
`WillowGameInfo` function table (read from its constructor), the slot the routing calls first holds a `WillowGameInfo`
override, and the slot it calls second holds the shared `Actor` routine; across the executable 262 function tables hold
the shared "pre" routine directly followed by the shared "post" routine, as the two adjacent slots the routing uses.
The shared routines, in own words:

- **"Pre"**: raises the script event **`PreBeginPlay`** (subject to the state probe below). Then, unless `bDeleteMe`,
  one more virtual with a world-derived flag (not identified), and for an actor with `Physics` `PHYS_RigidBody` whose
  `TickGroup` is not the ticking group's third value, one engine call with that value (by convention moving it to that
  tick group; low).
- **"Post"**: raises **`PostBeginPlay`** (subject to the state probe), then, unless `bDeleteMe`, raises
  **`SetInitialState`**, then, when `Base` is None, `bCollideWorld` and `bShouldBaseAtStartup` are set and `Physics`
  is `PHYS_None` or `PHYS_Rotating`, one more virtual (by stock UE3 convention `FindBase`; medium). This answers the
  first pass's question: **`SetInitialState` runs here, right after `PostBeginPlay`.**
- **The state probe** (medium). Each of the two events is raised when the actor has no state frame, or when a bit of its
  state frame's probe word is set (a different bit for each event): by public UE3 convention, an event a state
  ignores is not raised. On a freshly loaded actor outside any state, the events are raised.
- **`WillowGameInfo`'s "pre" override**: the shared "pre" routine, then one engine call with the text `Hardcore`
  (not read further).

### 2.2 The actor initialisation pass

For every level, over its actor list in order, skipping null entries (and static actors when the same mode argument is
set):

- **On a client only** (not the server), for an actor not yet script-initialised: one that is neither `bStatic` nor
  `bNoDelete` is destroyed; any other has `Role` and `RemoteRole` swapped once, guarded by `bExchangedRoles`, which is
  then set. On the server (the single-player case) nothing here happens.
- **The physics level filter** (high for the fields): `bLoadIfPhysXLevel0`, `1` and `2` are compared with the engine's
  physics level (0, 1 or 2). An actor whose flag for the current level is clear is removed: destroyed when it is not
  `bStatic`; when static, hidden (`bHidden` set through the actor's hide virtual, which notifies on a change) plus one
  more engine call (not read). Which level the engine runs at was not read; class defaults set all three flags.
- For an actor that is not `bDeleteMe` and does not carry one object flag (not named): `LastRenderTime` is set to the
  most negative float, `Touching` is emptied, `PhysicsVolume` is set to the world's default physics volume; and when
  the actor is not script-initialised or has no state frame, one more virtual (by stock UE3 convention the script
  execution set-up; low).

## 3. The game info

One routine (it holds the text "GameInfo class '%s' not found (or could not be loaded)"):

- It does nothing when the persistent level's world info already holds a game info (`WorldInfo.Game`; high), or when a
  network or demo driver exists (medium).
- Otherwise it joins the URL options into one text (`?` before each) and reads the value of a **`GAME=`** option from
  them (high for the key: the routine's own string; medium for the parse).
- **The class choice** (high for the field and the event, medium for the branch order):
  - When `WorldInfo.DefaultGameType` (the map's own class reference; named by the layout) is None: the class named by
    the `GAME=` option is loaded; if that fails, the class is the configured default, the game ini setting
    `Engine.GameInfo.DefaultGame`, loaded once and cached, and the choice ends there.
  - When `DefaultGameType` is set, or the `GAME=` class loaded: the script event **`SetGameType(MapName, Options,
    Portal)`** is raised on a class default object, and the class it returns is the choice (named by the name table:
    the event's three strings are the URL's map, the joined options and the portal). Which class's default object is
    the receiver in each branch was not separated (by stock UE3 convention, the class chosen so far).
  - `SetGameType` is UnrealScript: `GameInfo` and its subclasses define it in the installed script.
- It spawns the chosen class through the world spawn (NATIVE_ENGINE_CORE.md, "Actor.Spawn") and stores the new actor
  in `WorldInfo.Game`. A failed spawn logs the not-found text and leaves the field empty.

Which class Sanctuary gets in the running game depends on `Sanctuary_P`'s `DefaultGameType`, the URL and the script's
`SetGameType`: data and script, not native code.

## 4. The local player's play actor

`LocalPlayer.SpawnPlayActor(URL, out Error)` passes the URL text and the error string to the engine's local-player
routine and returns its result (high). That routine (medium for the event names' roles, high for the names):

- **On the server** (single player): it parses the URL text; when the local player's name is known, a `Name=<name>`
  option is added; it finds the player's index in the engine's local-player list; it asks the local player's script
  event **`GetUniqueNetId`** for the player's id; then it calls the world's play-actor spawn with the local player, the
  remote role, the URL, the id, the error string and the index, and stores the result as the local player's actor.
- **The world's play-actor spawn**: joins the URL options; asks the game info's script event **`Login(Portal,
  Options, UniqueId, out ErrorMessage)`** for a controller; when one comes back, sets its net player index, attaches the
  local player to it, sets its `Role` to the authority value and its `RemoteRole` to the role passed in, and raises the
  game info's script event **`PostLogin(NewPlayer)`**; it returns the controller (or None).
- **On a client**: a controller of a configured class is spawned directly (not read further; not used by single
  player).

`Login` and `PostLogin` are UnrealScript (`GameInfo` and its subclasses), so the play actor is the installed script's
work once the engine calls them. The engine's local-player list (`GamePlayers`) is filled before this; where was not
read, nor where the map load calls `SpawnPlayActor` (its loop over the local players after the bring-up does not call
it directly).

## Open (for the implementer, or a follow-up analyst pass)

- The value of the routing's and the initialisation pass's mode argument as the bring-up passes it, and the time-reset
  flag the map load passes (stock UE3 routes every actor and resets the time on a load; not confirmed).
- The receiver of `SetGameType` in each branch of section 3.
- The unread engine calls: the setup calls of step 3, the flagged object and the second object of step 4, step 5, the
  "pre" routine's extra virtual, the static hidden actor's second call, the object flag of section 2.2, the client
  controller class of section 4.
- Where the map load calls `SpawnPlayActor`, and where `GamePlayers` is filled.
- What the running game shows (bl-sdk reads): the order of `PreBeginPlay`, `PostBeginPlay` and `SetInitialState` across
  Sanctuary's actors; which static actors route; the game info's class and whether `SetGameType` ran; `Login` /
  `PostLogin` with their arguments for the local player; `TimeSeconds` right after load.
