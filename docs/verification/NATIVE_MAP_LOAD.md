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
   - the persistent level's navigation, cover and pylon join (section 5.2), right before the initialisation pass;
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
work once the engine calls them. The engine's local-player list (`GamePlayers`) is filled before this, at engine
start-up (section 6.1). The map load calls the local-player spawn directly for each local player after the bring-up
(section 6.2, which corrects this section's earlier reading).

## 5. The navigation list at load (lane ANALYST-MAPLOAD-NAVLIST-PLAYACTOR, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst only (decision 2026-11-19-147). Read from the packages (the level's
untagged tail and the points' tags, through `ow-package` and the definition store), then from the executable
(read-only Ghidra queries under the machine lock) for what the data does not settle. Raw output stays under ignored
`local/analysis/navlist/`. **UNVERIFIED** in the running game.

**The answer to Q1: the third candidate.**
- Each level carries its own list, and when the level is added to the world that list is joined onto the world's.
- `WorldInfo.NavigationPointList` is not saved data: it is None in `Sanctuary_P`'s tags and is built at run time.
- There is no rebuild over the placed points.

### 5.1 Each level's own list (data)

**What the level holds** (high for the two sampled files, medium as a general rule):
- A level holds six native references in its untagged tail: the start and the end of its navigation list, of its
  cover list and of its pylon list. They are serialized as three adjacent pairs in that order, with a null reference
  for an empty list. Stock UE3 names them `NavListStart`, `NavListEnd`, `CoverListStart`, `CoverListEnd`,
  `PylonListStart` and `PylonListEnd`.
- **The chain inside a level** is the points' own tagged `nextNavigationPoint`, from the start to the end, whose
  `nextNavigationPoint` is None. For covers the link is `CoverLink.NextCoverLink`, and for pylons `Pylon.NextPylon`
  (named by the join's field use and the layout tool; medium for those two names).

**The samples** (high), read with the store:
- `Sanctuary_P`: the navigation start and end are both `WillowCoopPlayerStart_0`, whose `nextNavigationPoint` is None
  (a one-point list); the cover and pylon pairs are null.
- `Sanctuary_Dynamic`: the navigation start is `Pylon_2` and the end `Pylon_1`. Walking from the start along
  `nextNavigationPoint` meets 7 points, all `Pylon`, and ends exactly at `Pylon_1` with None after it. The cover pair
  is null, and the pylon pair names the same two pylons.

**Which classes are in the list:** the points the editor's path build linked, which are `NavigationPoint` subclasses
(player starts, pylons, and so on). Not every placed point need be in it: `Sanctuary_Dynamic`'s `WillowAIMoveNode`s
are not in its chain. The rule is "the saved chain", not "every placed point".

**Where the pairs sit in the tail: not established.**
- The tail holds other native data between the URL and the pairs. That data was not decoded, so no structural locator
  for the pairs is known.
- The two samples were found by a search: a pair of references to that level's own navigation-point exports, followed
  by the cover and pylon pairs. That works only for a level with a non-empty list.
- It cannot find an empty level's pairs among unrelated zero fields. It is evidence for the shape, not a decoder.

**So decoding the pairs stays behind a typed refusal:** a not-read refusal (`NotYet`, kind `NotRead`), "the level
tail's navigation, cover and pylon list pairs (their position after the URL is not decoded)", until a structural
decoder of the level tail exists.

### 5.2 The join (executable)

**The join of one level onto the world** (stock UE3 `AddLevelNavList`). It runs for a level whose navigation start
and end are both set:
- each point of the level's navigation chain gets one engine call first (by stock UE3 convention its navigation
  set-up; not read; low);
- then the level's chain is put at the front of the world's list: the level's end point's `nextNavigationPoint`
  becomes the world's current `NavigationPointList`, and `WorldInfo.NavigationPointList` becomes the level's start
  (high);
- covers and pylons are spliced the same way, each only when both of its own ends are set: `NextCoverLink` and
  `WorldInfo.CoverList`; `NextPylon` and `WorldInfo.PylonList` (high).

The `WorldInfo` written is the persistent level's, the world's one world info. A matching routine takes a level's
chains out again when a level is removed (not on the load path).

**When it runs: the port's join schedule** (UNVERIFIED as a whole):
1. **The persistent level:** once, inside the bring-up (section 2), in its begin-play-once step. That is after the
   game info routine of section 3 has spawned the game info, and before the actor initialisation pass. High for the
   call, medium for the receiver being the persistent level.
2. **Each streamed level:** when it is added to the world. This is inferred: the streaming add calls the same
   per-level routine, which is one of only two routines that join, but the mode it passes was not pinned.
3. **A navigation point spawned during play** is appended at its level's end. If it is the level's first point during
   play, the level is joined then (low relevance to the load).

**The order across levels.**
- **The rule** (high): every join puts that level's chain in front. So the world's list is the levels' chains in the
  reverse of the order they joined, each keeping its own order.
- **The actual order at load: unknown.** When the streamed levels visible at load are added, relative to the
  persistent level's join, was not read. Some may come before it and some after. So `Sanctuary_P`'s chain may sit at
  either end of the list, or in the middle.

### 5.3 For the implementer (proposed rule)

1. **The level's pairs.** Decoding them is the typed refusal of 5.1 until the tail is decoded. A test may supply them
   directly.
2. **A write-free preflight of all three chain kinds before any splice.** For each of navigation, cover and pylon:
   - **a null pair** (start and end both null) is an independent no-op for that kind;
   - **a half-null pair** (one end null) is rejected;
   - **already joined, checked first for a set pair:** when the level's start is already in that kind's world list,
     that kind is a no-op (item 4), and the checks below are not run for it;
   - **a set pair not yet joined:** walk from the start along that kind's link. The walk must reach the end and find None after it.
     It must not revisit a node (no cycle), and it must not pass through a node already in the world's list of the
     same kind;
   - a failed check is a data error, and nothing is spliced, for any kind.
3. **The same actor in different kinds is allowed:** one actor may be in the navigation list and the pylon list, as
   `Sanctuary_Dynamic`'s pylons are. The "already in the world's list" test is per kind.
4. **A level joined twice:** the second join is a no-op, kind by kind. It is detected by item 2's first test, the
   level's start already in the world's list of that kind, before the shared-node, cycle and end checks. A port answer: the game guards this with a per-level done flag on the streaming path.
5. **The splices**, in the order navigation, cover, pylon, each `end.link = head`, then `head = start`. These write the
   end points' link fields and `WorldInfo`'s three heads.
6. **The schedule** of 5.2, labelled UNVERIFIED. The relative order of the persistent join and the streamed joins at
   load is open: the implementer picks one order and labels it UNVERIFIED, or refuses with a not-read refusal.
7. **The per-point engine call** stays a not-read refusal if a later stop needs it.

## 6. The local player and the play actor (lane ANALYST-MAPLOAD-NAVLIST-PLAYACTOR, 2026-11-19)

Read from the installed script (`research/script_disasm.py`) and the executable (read-only). **UNVERIFIED** in the
running game.

**The answer to Q3.**

### 6.1 Where `GamePlayers` is filled

**The script body** (high):
- `GameViewportClient.Init` (which `WillowGameViewportClient.Init` calls first, then sets up its login state) ends
  with `CreateInitialPlayer`.
- `CreateInitialPlayer` calls `CreatePlayer(ControllerId, out Error, bSpawnActor = false)` once, for the first
  controller id in 0 to 3 that is logged in. With none, it uses the first with a gamepad connected, and with none of
  those, id 0.
- **`CreatePlayer`:**
  1. makes a new local player of the engine's `LocalPlayerClass` and sets its `ViewportClient` and `ControllerId`;
  2. `AddLocalPlayer` appends it to `Engine.GamePlayers`;
  3. only when `bSpawnActor` is set and the net mode is not the client mode, it calls `SpawnPlayActor('', out Error)`.
     A false answer removes the player again.

  `NotifyPlayerAdded` follows.

**The start-up invocation** (inferred): the engine calls `GameViewportClient.Init` at start-up, before any map. That
caller was not read. So the slice's one local player is in `GamePlayers` before the map loads, with no play actor.

### 6.2 Where the map load calls `SpawnPlayActor`

**The observed call site** (high): the map load calls the engine's local-player spawn directly, once for each local
player in `GamePlayers` and in their order. That is the same routine the script's `LocalPlayer.SpawnPlayActor`
reaches (section 4). This corrects section 4's earlier reading.
- **When:** after the game info routine (section 3) and the bring-up (section 2), so after `InitGame`, the begin-play
  routing and the persistent level's navigation join.
- **The URL** passed is the map URL as text.
- **A failed spawn** (no actor) with a non-empty error text leads to an error path that was not read. By stock UE3
  convention the load fails with that error.

### 6.3 For the implementer (proposed rule, lane 3)

**Before the map:** one local player in `GamePlayers`, as `CreateInitialPlayer` makes it, with no actor. Controller id 0
is a port answer for the slice, which has no login or gamepad layer (UNVERIFIED).

**After the bring-up, for each local player, the local-player spawn of section 4 with the map URL's text.** The Off
failure contract:
1. **Preflight, before any write.** Refuse the unsupported inputs first:
   - a `GetUniqueNetId` answer the port does not model (anything but the empty id): a not-read refusal;
   - a remote role other than `ROLE_SimulatedProxy`: a not-read refusal. The value the engine's local-player routine
     passes was not read; `ROLE_SimulatedProxy` is the stock UE3 value for a local player's play actor, taken as the
     port's answer, UNVERIFIED;
   - a missing game info: a not-read refusal.

   Nothing is written before these checks.
2. **`Login`** runs (script) with the portal, the joined options, the id and the error string.
   - **A controller comes back:** in this order, the net player index is set, the local player is attached to the
     controller, `Role` and `RemoteRole` are set, then `PostLogin(NewPlayer)` runs (script), then the controller is
     stored as the local player's actor. **Last, on success only,** the port adds it to its local-player list
     (`add_local_player`).
   - **None comes back with a non-empty error:** the load's failure. The `Login` error text is kept unchanged in the
     outcome, and nothing after `Login` runs.
   - **None comes back with an empty error:** its own typed outcome, a not-read refusal, "the map load after a
     play-actor spawn that gave no actor and no error".
3. **Refusals after script has run** (a refusal raised inside `Login` or `PostLogin`, or after them) are marked
   "stopped after partial writes" by the caller, keeping their kind and record. A preflight refusal is never marked.
4. **The two engine calls** between the bring-up and the player loop stay unread.

### 6.4 Open (UNVERIFIED)

1. Whether the streamed levels visible at load are joined before or after the persistent level, or some of each (5.2).
2. The mode the streaming add passes to the per-level routine (the streaming join is inferred).
3. The per-point engine call the join makes.
4. Where the level tail's six list references sit (5.1): no structural locator yet.
5. The map load's two calls between the bring-up and the player loop, and its error path.
6. Who calls `GameViewportClient.Init` at start-up (the start-up invocation is inferred).

### 6.5 Live checks (bl-sdk, the RTSE way)

1. Right after Sanctuary loads, walk `WorldInfo.NavigationPointList` along `nextNavigationPoint`. Record each point's
   level and the order. Expect each level's chain to be contiguous. `WillowCoopPlayerStart_0`'s chain may be first,
   last or in the middle; its place answers open question 1.
2. Read `Engine.GamePlayers` before and after the load. Expect one entry both times, with its `Actor` None before and
   the player controller after.

## Open (for the implementer, or a follow-up analyst pass)

- The value of the routing's and the initialisation pass's mode argument as the bring-up passes it, and the time-reset
  flag the map load passes (stock UE3 routes every actor and resets the time on a load; not confirmed).
- The receiver of `SetGameType` in each branch of section 3.
- The unread engine calls: the setup calls of step 3, the flagged object and the second object of step 4, step 5, the
  "pre" routine's extra virtual, the static hidden actor's second call, the object flag of section 2.2, the client
  controller class of section 4.
- Where the map load calls `SpawnPlayActor`, and where `GamePlayers` is filled: answered in section 6.
- Where `WorldInfo.NavigationPointList` comes from: answered in section 5.
- What the running game shows (bl-sdk reads): the order of `PreBeginPlay`, `PostBeginPlay` and `SetInitialState` across
  Sanctuary's actors; which static actors route; the game info's class and whether `SetGameType` ran; `Login` /
  `PostLogin` with their arguments for the local player; `TimeSeconds` right after load.
