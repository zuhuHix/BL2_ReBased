# Native actor destroy: the controller and player case (draft, 2026-11-16)

AI-assisted (Claude Code), analyst lane FIX-ACTOR-DESTROY. Read from the game executable with Ghidra 12.1.4
(`tools/ghidra/run.ps1`, a read-only query of `Actor.Destroy` and two levels of its callees against the project made
by `tools/ghidra_import.ps1`), written in our own words; no decompiler output, pseudo-code or listing structure is
reproduced here, and the raw output stays outside the repository. Field names come from the cooked packages
(`tools/ghidra/class_layout.py`). **Every rule is UNVERIFIED in the running game**; nothing here was observed live.

This note completes step 1 of the destroy routine in NATIVE_ENGINE_CORE.md ("Actor.Destroy": "Controller special
case ... not read in detail"). It is the hand-off to the implementer of the `ParityMode::Off` rule. The analyst lane
left Off as `NotYet` for this case and gave Reference the C++ stand-in (decision 2026-11-16-79); the Off rule is to be
built by a fresh implementer from this note and NATIVE_ENGINE_CORE.md alone (docs/LANE_BRIEF.md, "Clean room").

## 1. Where the case sits

`Actor.Destroy` asks the world's destroy routine to destroy the actor, with no forced network destroy and level
modification allowed, and answers its result. The checks NATIVE_ENGINE_CORE.md lists come first: not in play (the
editor path), `bStatic` or `bNoDelete` (false), already destroyed (true), not authority and not net-temporary (false).
The controller and player case comes right after them and before anything is changed (before `bPendingDelete`).

## 2. The case

The actor is asked, through a virtual every actor has, for **the player controller it is**. Read as the engine's
"this actor as a player controller" query: it gives the actor itself for a player controller and nothing for any
other actor. Medium-high confidence: the same virtual is how the ammo drop weight picks the controllers whose pools
count (NATIVE_ATTR_RESOLVERS.md section 4), and both uses fit that reading.

| The destroyed actor | What happens | `Destroy` answers |
|---|---|---|
| not a player controller (a pawn, an AI controller such as a `WillowMind`, any other actor) | nothing here; the generic routine runs (NATIVE_ENGINE_CORE.md steps 2 to 7) | the generic routine's answer |
| a player controller whose `Player` is a **local player** (an object of an `Engine` class the cast names; read as `LocalPlayer`) | its `bPendingDestroy` is set; the engine object a second cast gives (read as the game engine) is asked, through a virtual not identified, to drop that player (read as removing the local player from the viewport), and a follow-up call is made on it. The controller is **not** destroyed: no `bPendingDelete`, no script event, nothing is removed | false |
| the same, but the second cast gives no engine object | `bPendingDestroy` is set and a log line is written; nothing else | false |
| a player controller whose `Player` is not a local player (read as a network connection) | each child connection of that connection whose actor is this controller is told, through a virtual not identified, to close (read as closing the split-screen child connection); then the generic routine runs | the generic routine's answer |
| a player controller with no `Player` | the generic routine runs | the generic routine's answer |

## 3. For the implementer

- **The Off rule.** For a standalone game (the slice) this reduces to two rows:
  - an actor that is not a player controller goes through the generic routine the port already has. An AI
    controller is such an actor, so the kill's destroy of the victim's mind takes the generic path;
  - a local player's controller answers false after setting `bPendingDestroy`. The viewport removal is the
    host's.
- **The child-connection row** is network play; `NotYet` is a fair answer for it in the slice.
- **What the generic path needs for a controller** is NATIVE_ENGINE_CORE.md's: the state, Kismet, base and touch
  conditions the port already refuses with `NotYet`, and the script `Destroyed()` (the installed
  `Controller.Destroyed`, which the VM runs).
- **The C++ stand-in** (`src/combat_death.cpp`, combat runs only) marks every destroyed actor `bDeleteMe` and
  answers true; it has no controller case. The Off tests should not copy it.
- **Open** (not read):
  - the names of the three `Engine` classes the casts compare with (read by role above);
  - the two engine calls of the local-player row;
  - the child-connection call.
- **bl-sdk reads that would confirm the rules** (none run):
  - `Destroy()` on a spawned AI controller (its answer, its `bDeleteMe`, and whether `Destroyed` fired);
  - `Destroy()` on the local player's controller (the answer, `bPendingDestroy` after it, and whether the controller
    is still in the world's controller list).
