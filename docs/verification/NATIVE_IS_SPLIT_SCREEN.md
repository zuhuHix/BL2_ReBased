# Native rules of Engine.IsSplitScreen (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-LINK1-IS-SPLIT-SCREEN (decision 2026-11-19-122, number
pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py`;
- the class layout, from the cooked packages through the project's layout tool;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (one read-only query, under the machine lock).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/splitscreen/`. This note is in our own words: no decompiler output, pseudo-code, addresses or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** With #508 (`WillowGameInfo.IsEditor` false in Off), slice link 1 stops in `maya:8` at this
native, unbound. Links 3, 4, 5, 13 and 14 sit behind the same world set-up.

## 1. Script or native, and the caller on link 1's path

- `Engine.IsSplitScreen()` is a **native**: static and final, declared on `Engine`, returning a bool. Its key is
  `Engine.IsSplitScreen()`.
- On link 1's path it is reached from `WillowGameInfo.ShouldSpawnAtStartSpot(Player)`. That function returns false when
  `IsSplitScreen()` is true; otherwise it returns the base `GameInfo.ShouldSpawnAtStartSpot(Player)`.
- The decoded script calls it from about twenty other places: menu tooltips, the inventory inspect panels, camera
  shakes, impact effects, the injured state's set-up and cinematic mode. None of those is on link 1's recorded path.
- Two other natives share the name and are not this one: `WillowPawn.IsSplitScreen` and
  `PlayerController.IsSplitscreenPlayer`. Neither is on link 1's path, and neither was read.

## 2. What it does

It reads one value: how many entries the global engine object's `GamePlayers` list holds (the engine's local players).
- It answers true when that list holds more than one player, else false.
- It writes nothing, calls nothing and loads nothing.
- The field is named by fit with the layout tool, which places `Engine.GamePlayers` (an array of `LocalPlayer`) where
  the native reads its element count. That is medium-high confidence, since the layout tool drifts in some classes.

## 3. Off answer

The hub's pre-ruling applies: the answer depends only on the local player count, so the port's Off answer is **false**.
- The port has one local player.
- This is a port answer, **UNVERIFIED** in game, of the same kind as the empty online-subsystem slot.
- No preflight is needed: it cannot refuse, and it writes nothing.
- The port's offline engine object (#501) is made from `GameEngine`'s class defaults. Its `GamePlayers` list is empty,
  so a literal read would also answer false. The answer does not rest on that list being filled.
- Reference keeps the stub policy.

## 4. What comes next (a local probe only)

With `Engine.IsSplitScreen()` bound false on a local, uncommitted build of this branch, the installed Off `--slice-gate`
moves:
- link 1 to `maya:8`'s next stop, the unbound iterator `WorldInfo.AllNavigationPoints(class,object)`;
- links 3, 4, 5, 13 and 14 to the same stop, behind it.

The iterator is reached from `GameInfo.FindPlayerStart`, which walks the level's navigation points for a `Teleporter`
before it asks `ChoosePlayerStart`. `ChoosePlayerStart` then walks them for a `PlayerStart`.

## Open questions

1. **UNVERIFIED:** that the counted list is `Engine.GamePlayers` (named by layout fit).
2. **UNVERIFIED:** that the global the native reads is the engine object `GameEngine` instances fill (the usual engine
   global).

## Live check that would confirm the rule (bl-sdk, the RTSE way)

In a single-player session, read `Engine.GamePlayers`' length (expect 1) and call `Engine.IsSplitScreen()` (expect
false). Then repeat with a second local player joined in split screen (expect 2 and true).
