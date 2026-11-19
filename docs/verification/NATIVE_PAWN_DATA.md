# Native rules of the player pawn data manager: GetPlayerPawnDataManager and FindAlreadyLoadedObject (draft, 2026-11-19)

AI-assisted (Claude), analyst lane PAWN-DATA-MANAGER (decision 2026-11-19-18). Read from the game executable with Ghidra
(`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock) and from the installed packages with
`research/script_disasm.py`, written in our own words. No decompiler output, pseudo-code, addresses or listing
structure is reproduced here. Field names come from the cooked packages (`tools/ghidra/class_layout.py`). **Every rule
is UNVERIFIED in the running game.** The last section lists the live observations that would confirm them.

**Clean room.** This lane implements nothing. In the Rust port both natives stay unbound in Off, so the gameplay stub
policy refuses them, until a fresh implementer codes them from this note alone.

**Why it matters.** With the skill manager natives in (decision 2026-11-19-14), Off `--combat-run maya:8` stops on
`WillowGlobals.GetPlayerPawnDataManager()` (scratch build of `fix/rust-skill-manager` at 73a340d3, installed game). That
is link 1 (`spawn_maya`) of the slice gate, and links 3, 4, 5, 13 and 14 wait behind it.

## Summary

| Native | Script signature (from the package) | Reached on the slice | Confidence | Status |
|---|---|---|---|---|
| `WillowGlobals.GetPlayerPawnDataManager` | `()` returning `WillowPlayerPawnDataManager` (final) | `ApplyCharacterClassDefaults`, right after the skill manager statement; also about ten other controller sites (login, class switch, stand-ins, save apply) | high (getter), medium (when the field is filled) | UNVERIFIED |
| `WillowPlayerPawnDataManager.FindAlreadyLoadedObject` | `(string)` returning `Object` | the same statement, on the manager it returns | high | UNVERIFIED |

## Where the maya path calls them

`WillowPlayerController.ApplyCharacterClassDefaults(P, CharClass)` runs the parent's version, then the skill manager
statement, then (with `P` a `WillowPawn` and `CharClass` a `PlayerClassDefinition`) this one:

it gets the globals' pawn data manager, looks up the player class's `PawnArchetypePath` with
`FindAlreadyLoadedObject`, and casts the result to `WillowPlayerPawn`. Its `BodyClass` goes to the pawn's `BodySelectClass`. The statements after that are script calls:
`SetShieldResourcePoolDefinition`, the FOV and split profile calls, the FOV and eye-height speeds,
`ApplyAttributeStartingValues`, `SetUpTradeInteraction`, and below `NetMode` 3
`BalanceModifierDefinition.ApplyPlayThroughBasedPlayerAttributeEffects` and `RecalculateAttributeInitializedState`.
Which native the path reaches next, once these two answer, was not run: it needs these two bound.

`GetWillowGlobals()` (a static native) answers the engine's one globals object, held by the running game engine; with no
engine object it answers None. This was read only far enough to say that.

## WillowGlobals.GetPlayerPawnDataManager

- **What it returns:** the globals object's `ThePlayerPawnDataManager` (an object property of `WillowGlobals`, class
  `WillowPlayerPawnDataManager`), as it is. The getter creates nothing and checks nothing.
- **Who fills the field:** a native routine of `WillowGlobals` (one of its C++ virtual functions, not a script function).
  It fills several of the globals' subobjects in turn, and this field only when it is None:
  1. it first looks for an existing object of class `WillowPlayerPawnDataManager` and takes the first one an engine
     check accepts (the check is a per-object engine virtual, read as "not marked for destruction": UNVERIFIED);
  2. if there is none, it constructs a new `WillowPlayerPawnDataManager`: no name, the outer read as the engine's
     transient package (UNVERIFIED), and then adds it to the root set, so garbage collection never frees it.
- **When that routine runs:** not found. No direct caller exists; the routine is reached only through the globals' C++
  vtable, and the call site was not located in this lane's time-box (open question 1). It is expected to run when the
  engine sets up its globals, before any map or controller script runs (UNVERIFIED).
- **Null cases:** the getter answers None only while the field is None, which is before that routine has run, or if the
  construction failed. There is no lazy creation here, unlike the skill manager.
- **What a script sees:** one `WillowPlayerPawnDataManager` object (it extends `Object`, not `Actor`), the same on every
  call, for the whole session. Its class has no script functions; every function is native (the `LoadPlayerPawnData`,
  `LoadSkillTreeDefinition`, `LoadVSSVehicleDefinition` and `LoadDataPackage` families with async variants, the
  client-notification and reference natives, `FindAlreadyLoadedObject` and `ClearReferencers`). Only
  `FindAlreadyLoadedObject` is read here.
- **For an implementer, in the port's terms:** the globals object needs the manager in `ThePlayerPawnDataManager` before
  controller script runs. That is a construction at globals set-up, not at the first call. The getter is a plain field
  read.
- **Live confirmation:** with bl-sdk, at the main menu and again after loading Sanctuary, read
  `WillowGlobals.ThePlayerPawnDataManager` (expected: set in both, the same object), its `Outer` and its object flags.

## WillowPlayerPawnDataManager.FindAlreadyLoadedObject(Path)

- **Inputs:** a string, read as a full object path (`Package.Group.Name`). On the maya path it is
  `PlayerClass.PawnArchetypePath`, the player class's pawn archetype.
- **Does:** looks the path up among the objects **already in memory**, accepting any class (the base `Object` class is
  the class filter, and no outer is given, so the path must name the object fully: UNVERIFIED). It never loads a
  package. An empty string is looked up as an empty name.
- **Returns:** the object found, or None. The manager's own fields are not read or changed.
- **On the maya path:** the answer is cast to `WillowPlayerPawn`. When the pawn archetype's package is not loaded, the
  answer is None, so `BodyClass` reads None and `BodySelectClass(None)` runs; what that does is script and is not read
  here. In the game the archetype is expected to be loaded already (the `LoadPlayerPawnData` natives exist for that), so
  this is a lookup, not a load (UNVERIFIED).
- **For an implementer, in the port's terms:** the answer is the object store's object at that path if it is already
  made, None otherwise. Whether the port should count an archetype that the definition store *can* load, but has not, as
  "in memory" is a port decision. Answering None there would make `BodySelectClass(None)` run, which the game is not
  expected to do on this path.
- **Live confirmation:** call `FindAlreadyLoadedObject` through pyunrealsdk with Maya's `PawnArchetypePath` in Sanctuary
  (expected: the archetype), and with the path of an object in a package that is not loaded (expected: None).

## Open questions

1. When the globals routine that fills `ThePlayerPawnDataManager` runs: its call site through the globals vtable was
   not found.
2. Which engine check filters the existing manager objects, and what the outer of a new one is.
3. Whether `FindAlreadyLoadedObject` accepts a partial path. The read suggests not, since no outer is given.
4. The next native on the maya path after these two (needs them bound; run `--combat-run` Off `maya:8` then).
