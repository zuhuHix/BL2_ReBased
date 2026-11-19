## 2026-11-19: Read the login status as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane ANALYST-LOGIN-STATUS, analyst only. Number 89 was given by the hub. For slice link 1, NATIVE_INIT_PLAYER_SKILL_TREE.md
(#466) has `ApplyCharacterClassStartingValues` reach `UIInteraction.GetLoginStatus` through `IsInvalidName` after
`InitPlayerSkillTree`, for a local player. The new own-words note `docs/verification/NATIVE_LOGIN_STATUS.md` and three
`NATIVE_INDEX.md` rows record, as behaviour only:

- **`UIInteraction.GetLoginStatus` is script, not a native.** It answers 0 (`LS_NotLoggedIn`) for id -1, with no
  online subsystem or with no player interface. Otherwise it answers the player interface's `GetLoginStatus`.
- **`GameEngine.GetOnlineSubsystem`** (native) answers the engine's `GameEngine.OnlineSubsystem` field, or None when
  the engine is not a `GameEngine`.
- **`OnlineSubsystemSteamworks.GetLoginStatus`** (native, through a C++ virtual) answers the subsystem's
  `LoggedInStatus` when the id equals `LoggedInPlayerNum`, else 0.
- **The two `IsInvalidName` overrides** (script):
  - the Willow one goes on only for a local owner;
  - with status 0 it renames through `ServerSetPlayerNameAsCharName`;
  - otherwise the base one renames to the Steam nickname (`LoggedInPlayerName`) when the status is at least 1 and
    the name differs.
- **The next stops,** read cheaply: `SetName`, then `SetupInputDevices` and its two natives.
- **The Off answers** are given step by step, with mutually exclusive answers for:
  - an absent engine: a named `NotYet` (layer absent);
  - an engine that is not a `GameEngine`: None;
  - a `GameEngine` with an empty subsystem slot: None, as stored;
  - a populated slot: the object.

  None for an absent engine would assume status 0 (`LS_NotLoggedIn`), while the shipped offline result stays open.
  When the stock gates (authority, a Willow replication info with a local owner, an id other than -1) and the absent
  engine are known before the flow, the flow's preflight answers the `NotYet` before its writes. A path known only
  later keeps the runtime check, terminal with the partial-writes prefix. A Steamworks subsystem whose login state
  the world has not filled, and `SetName`'s URL and config writes, are named `NotYet`s (not read).
- **It supersedes #466 on one point:** `UIInteraction.GetLoginStatus` is script, and `GameEngine.GetOnlineSubsystem`
  is the first native. #466 stays as it is (hub ruling).

Method:
- the script was read with `research/script_disasm.py`;
- the native list came from `tools/ghidra/script_natives.py`, the layouts and the enum from the cooked packages
  (`tools/ghidra/class_layout.py`);
- the two natives and the Steamworks vtable entry were read with Ghidra 12.1.4 headless, read-only, under the machine
  lock.

Raw output stays under ignored `local/analysis/login-status/`. The note was scanned with the brief's pattern. No bl-sdk
session was granted and none of the recorded evidence covers the login status, so every rule is UNVERIFIED. This lane
implements nothing.

Open (the note's questions): the shipped login state for an offline Maya, under the Steam client's mode and the
game's setting recorded apart; which object the subsystem's `PlayerInterface` holds; `SetName`'s URL and config writes
and `ChangeName`; the two input natives.

Edits (review r1 of #474): the preflight of the gated absence, the four engine cases, the supersede of #466's
wording, and the live checks by return value and branch hooks.
