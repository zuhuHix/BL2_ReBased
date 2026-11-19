# Native rules of the player's skill tree creation: InitPlayerSkillTree (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-INIT-PLAYER-SKILL-TREE (decision 2026-11-19-78).

**How this was read.**
- The natives were read from the game executable with Ghidra (`tools/ghidra/run.ps1 -ReadOnly`), under the machine
  lock.
- The script they reach was read from the installed `WillowGame.upk` with `research/script_disasm.py`.
- This note is in our own words and records behaviour only. It reproduces no decompiler output, pseudo-code, quoted
  script statements, addresses, offsets or call structure.
- Raw output stays under ignored `local/analysis/init-player-skill-tree/`.
- The field and function names come from the cooked packages.

**Every rule here is UNVERIFIED in the running game.** Section 7 lists the bl-sdk observations that would settle
them.

**Clean room.** This lane implements nothing. The native stays unbound in Off until a fresh implementer codes it from
this note alone.

**Why it matters.** The installed script event `OnPlayerClassChange` (NATIVE_CLASS_STARTING_VALUES.md section 3) calls
`InitPlayerSkillTree` before anything else of its own. Nothing binds that native yet, so links 1, 3, 4, 5, 13 and 14
of the slice gate stop at `maya:8`.

This note replaces the outline in NATIVE_SKILLS.md section 3.3, which stays correct but leaves out the order, the
gates and the delivery details.

## Summary

| Function | Kind (from the package) | Confidence | Status |
|---|---|---|---|
| `WillowPlayerController.InitPlayerSkillTree()` | native, final, no parameters, no return value | high | UNVERIFIED |
| `WillowPlayerPawnDataManager.LoadSkillTreeDefinitionAsync(SkillTreeDefinitionPath, PlayerContext, Payload, Instigator, IntParam, FloatParam, CallbackCustomEventName)` | native | medium | UNVERIFIED |
| `WillowPlayerController.RunStreamingDataEvent` (the `SkillTreeDefinitionLoaded` branch) | script | high (script) | UNVERIFIED |
| `WillowPlayerController.ClientResetSkillTree(bIgnoreProficiencies)` | script, client function | high (script) | UNVERIFIED |

## 1. What InitPlayerSkillTree does, in order

The native takes no arguments and reads only the controller it runs on.

1. **Is there a tree already?** It reads the controller's `PlayerSkillTree`. If that is set, it goes straight to
   step 5.
2. **Make the tree.**
   - It constructs a new object of class `WillowGame.PlayerSkillTree`, with the controller as its outer, no name and
     default flags. The class is looked up once and cached.
   - It stores the object in the controller's `PlayerSkillTree`.
   - The new tree is empty: no branches, no skills and no listeners.
3. **Find the loader.** It reads the engine globals' `ThePlayerPawnDataManager` (a `WillowGlobals` field).
4. **Ask for the definition**, only when both the controller's `PlayerClass` and that manager are set. It calls the
   manager's `LoadSkillTreeDefinitionAsync` (section 2) with these arguments:
   - the path: the player class's `SkillTreePath`, a string field of `PlayerClassDefinition`;
   - the context and the instigator: the controller itself;
   - the payload: None;
   - the int: 0;
   - the float: 0.0;
   - the event name: `SkillTreeDefinitionLoaded`.

   Without a `PlayerClass` or without a manager, nothing is asked and nothing is reported. The tree stays empty
   until some later class change.
5. **Reset the tree, always.** Whether the tree is new or already existed, the controller's own reset runs with both
   flags false. This is the routine `ResetSkillTree(bIgnoreProficiencies, bIsCharacterLoad)` reaches, described in
   NATIVE_SKILLS.md section 3.1. On a new, empty tree:
   - The refund is zero: no points were spent before or after.
   - There are no tiers to clear and no root tier to open.
   - The tree's listeners are told about the reset. A new tree has none, because the controller registers only in the
     section 3 callback.
   - The controller has the authority and this is not a character load, so the routine sends the one-bool client
     event, with false. The installed script has a client function `ClientResetSkillTree(bIgnoreProficiencies)`,
     and NATIVE_SKILLS.md section 3.1 already took it for this event. On the authority that script does nothing;
     below the authority it runs `ResetSkillTree` itself. The dispatch name inside the native is not read here, so
     the match stays UNVERIFIED.
   - Whether this reset tolerates a tree whose definition has not arrived (no branch list at all) was not read
     (open question 1). Nothing here suggests that it refuses.

**What it refuses.** Nothing. There is no error path:
- a missing `PlayerClass` or a missing manager only skips step 4;
- the object construction is not checked;
- the native returns nothing.

**What it writes.**
- The controller's `PlayerSkillTree`, in step 2 only.
- Whatever the request in step 4 records or delivers (section 2).
- What the reset writes: the tree's grades and tier flags, which are empty on a new tree, and the client event.

## 2. What the manager does with the request (LoadSkillTreeDefinitionAsync)

Read from the manager's implementation, in order:

1. **The package part of the path.** It finds the first dot in the path; the text before it names the package to
   load.
2. **Already loaded?** It asks whether the object at that path is already loaded.
   - **If it is,** it delivers the completion at once, inside the call and so inside `InitPlayerSkillTree`, before
     that native's reset in step 5. The delivery carries:
     - the event name;
     - the payload (None here) as the event's instigator;
     - the loaded definition as its other participant;
     - the int and the float.
   - The receiver of that delivery and the gate on it are not fully read. Section 3 takes the requesting context
     (the controller) as the receiver. That fits the script handler, but it is UNVERIFIED.
3. **Not loaded, and the path has a dot:** it builds a pending request holding all seven arguments. Then:
   - If a global engine switch is set, it loads the object at once and delivers the completion as in point 2. Which
     switch this is was not read (open question 2).
   - Otherwise it queues the request against the package name for a streaming load, and the completion comes later.
4. **Not loaded, and the path has no dot:** nothing happens. No request is made, no completion comes, and no error is
   raised.

## 3. The completion (script RunStreamingDataEvent, branch SkillTreeDefinitionLoaded)

When the event name is `SkillTreeDefinitionLoaded`, the controller's script does these four things in order:
1. it casts the other participant to `SkillTreeDefinition`;
2. it calls `PlayerSkillTree.Initialize` with it (native; NATIVE_SKILLS.md, "PlayerSkillTree.Initialize"). That builds
   the branches, tiers and skills, then resets them to their starting grades;
3. it registers the controller with the tree as its skill-tree listener (`RegisterListener`, native);
4. it sets a one-shot timer of 0.0001 s, not looping, for `NotifyReadyToLoadPendingSavegame`, through the engine's
   timer native.

Saved grades are therefore applied only after this point, as NATIVE_SKILLS.md section 3.3 says.

**When the definition is already loaded, the order differs** (section 2, point 2):
- `Initialize` and the listener registration happen inside `InitPlayerSkillTree`, before its own reset.
- That reset then runs on a full tree. It returns every grade to its starting grade and tells the newly registered
  controller.
- Whether this is the usual case at a class change in the running game is open question 3.

## 4. What OnPlayerClassChange reaches after InitPlayerSkillTree (read cheaply, script only)

This continues NATIVE_CLASS_STARTING_VALUES.md section 3, for the slice stop that comes next.

1. **The gate.** Everything in this section except step 4 needs the authority and a `WillowPlayerReplicationInfo`.
2. **Name and movement type.** With the authority and that replication info:
   - The replication info's `CharacterNameIdDef` is set from the player class's `CharacterNameId`.
   - **`SetPlayerMovementType`** (script) runs with the controller's `PlayerMovementType`. It clamps the value to the
     range 0 to 2, stores it, and sets two movement flags from it:
     - 0: analog movement off, gamepad walk/run off, and the run byte cleared;
     - 1: gamepad walk/run on;
     - 2: analog movement on.

     Nothing native is reached.
   - **`GetCharacterClassDefaultName`** (script) returns the class's name identifier's `CharacterName`, or an empty
     string without one. The result is stored in `PlayerPreferredCharacterName`. Nothing native is reached.
3. **`IsInvalidName`** (script, the `WillowPlayerReplicationInfo` override) runs on the replication info. Its result
   is ignored.
   - Only when the info's owner is a player controller that is local does it go further. It then asks the UI layer
     for the login status (`UIInteraction.GetLoginStatus`, a native of the UI layer).
   - With a login status of 0 it asks the server to set the player name to the preferred character name.
   - Otherwise it falls back to the base `PlayerReplicationInfo.IsInvalidName`.
   - For a non-local owner, or no owner, the answer is false and nothing is reached.

   **The likely next stop, but only on that path** (the owner a local player controller): `GetLoginStatus` needs
   the UI layer, as `GetDataStoreClient` does (decision 2026-11-19-19). Section 5 gives the Off disposition.
4. **`SetupInputDevices`** (script) runs, without any gate. It asks the controller's `PlayerInput`, cast to
   `WillowPlayerInput`, to set up its devices. That is a native of `WillowPlayerInput` and is not read here. Without a
   player input object the step is skipped. Then it clears `InputContextSelector` and calls `UpdateInputContext`,
   which was not read here.

## 5. The Off answer, step by step (for an implementer, under the hub's write rule)

Review r1 of #466 set the rule this section follows: every case has exactly one Off answer. Where the native is
unread, the answer is marked **Off assumption**, and that is UNVERIFIED like everything else here.

**The hub's write rule.**
- Everything that can refuse is decided before the first write, here before the tree is allocated.
- After a write there is no rollback.
- A `NotYet` raised after a write is terminal and keeps its kind (`NotRead`, `LayerAbsent` and so on). Its text gets
  the prefix "stopped after partial writes: ".
- Every other error propagates unchanged, before a write or after one.

**The Off assumptions** (each one is a single answer for something the native or the game leaves open):

- **A1, residency.** Off treats the skill tree definition as **never already loaded** when the request is made, so
  nothing is delivered inside the call (section 2, point 2 never applies in Off). The port has no notion of
  "already resident" that matches the game's (open question 3).
- **A2, the immediate-load switch.** Off treats the switch as **off**, so a request is never loaded and delivered at
  once (section 2, point 3; open question 2).
- **A3, delivery.** This lane delivers nothing. A recorded request stays pending until a later lane delivers it. That
  lane delivers it to the request's `PlayerContext` (the controller), the receiver assumed in section 2 (open
  question 4), by running its `RunStreamingDataEvent`.
- **A4, the reset of a new tree.** On a tree this call has just made, with no definition and no listener, the reset
  changes nothing and its refund is 0. The reset's own code is unread (open question 1).
- **A5, the client event.** Off **does not send** the client event. The native sends it only on the authority
  (NATIVE_SKILLS.md section 3.1). On the authority the installed `ClientResetSkillTree` script does nothing, so it
  cannot re-enter the reset, and the headless world has no remote client to send it to. Below the authority the
  native does not send it at all.

**The four request cases.** A request is made only when both the manager and the player class are set (P4 and P5).

| Path shape | Game, already loaded | Game, not loaded | Off (A1, A2) |
|---|---|---|---|
| has a dot | delivered inside the call | queued, delivered later | pending request recorded, nothing delivered |
| no dot (the empty string included) | delivered inside the call | dropped: nothing recorded, nothing delivered | nothing recorded, nothing delivered |

Off's row is the game's "not loaded" column, by A1. Off does not deliver for a dotted path even where the game
would deliver at once.

**Preflight.** These reads are cheap and certain, and all of them come before the allocation, in this order:

| # | Check | Outcome |
|---|---|---|
| P1 | the controller's class declares `PlayerSkillTree` as an object property | otherwise a data error |
| P2 | read `PlayerSkillTree` | None: a new tree (go on); set: **an existing tree**, the named `NotYet` (`NotRead`) "InitPlayerSkillTree: the reset of an existing skill tree (NATIVE_SKILLS.md section 3.1 is not ported)", returned before any write; any other kind: a data error |
| P3 | find the class `WillowGame.PlayerSkillTree` in the code store | found: go on; otherwise the store's own error, propagated unchanged (never a `NotYet` made here, never a None) |
| P4 | the engine globals | from the world; a world without them gives its own `NotYet` (`LayerAbsent`), unchanged. The globals' class must declare `ThePlayerPawnDataManager` as an object property, otherwise a data error; None: **no request**; an object: go on; any other kind: a data error |
| P5 | the controller's `PlayerClass` (declared as an object property, otherwise a data error) | None: **no request**; set: its class must declare `SkillTreePath` as a string property, otherwise a data error; the string is the request path |
| P6 | the request decision | a request exactly when P4 and P5 both gave one; then its row in the table above (a dot or not) |

The existing-tree refusal in P2 is the one answer for that state. The only installed caller, `OnPlayerClassChange`,
clears `PlayerSkillTree` first, so the slice never meets it. Its reset is the full NATIVE_SKILLS.md section 3.1
reset, which this note does not port. Reading the globals before the allocation (P4 before W1) changes nothing
observable, because in the game nothing runs between the two.

**Writes**, for a new tree (P2 gave None), in this order:

| # | Write | Off answer |
|---|---|---|
| W1 | allocate a `PlayerSkillTree`, outer the controller, no name | the object; a heap error propagates unchanged |
| W2 | the controller's `PlayerSkillTree` | the new object; a heap error propagates unchanged |
| W3 | the request, only when P6 made one with a dotted path | the port records one pending request with all seven arguments (section 1, step 4). Nothing is delivered (A3) |
| W4 | the reset with both flags false | nothing changes, the refund is 0 (A4), and no client event is sent (A5). Nothing is written |

The native returns nothing.

**After a write.** With A1 to A5, no step after W1 can raise a `NotYet`. No script runs inside the call, and W3 and W4
cannot refuse. If a later change makes one possible anyway, the rule above applies: the `NotYet` is terminal, keeps
its kind and gets the "stopped after partial writes: " prefix, nothing is rolled back, and other errors propagate
unchanged.

**What `OnPlayerClassChange` reaches next (section 4) in Off.** None of these calls needs a disposition of its own in
this note:
- **`SetPlayerMovementType` and `GetCharacterClassDefaultName`** are script and reach no native. They run through the
  VM's existing dispatch.
- **`IsInvalidName`** is script and runs through the VM's existing dispatch.
  - It reaches `UIInteraction.GetLoginStatus` only when the replication info's owner is a player controller and
    `IsLocalPlayerController` answers true.
  - On that path the VM's own answer for that native is the stop: its binding, or the stub policy's answer for an
    unbound native. No named refusal is added here.
  - If the owner is not local, or there is none, the function answers false and reaches nothing.
- **`SetupInputDevices`** is script and runs through the VM's existing dispatch.
  - It calls `WillowPlayerInput.SetupInputDevices` (native, unread) only when the controller's `PlayerInput` is a
    `WillowPlayerInput`. When it does, the VM's answer for that native is the stop, as above.
  - `UpdateInputContext` is then called as any script function is. What it reaches answers as the VM answers.

**Slice minimum** (so that `OnPlayerClassChange` goes on and `maya:8` can reach the ammo maxima). It is exactly the
rows above, nothing more:
- P1 to P6;
- W1 and W2;
- W3 as one pending record for a dotted path, and nothing for a dotless one;
- W4 as the no-op of A4 and A5.

Nothing in sections 1 to 4 reads the tree afterwards, so the pools and the ammo maxima do not need it.

**Delivery is a later lane's.** Delivering the request means running `RunStreamingDataEvent` and, through it,
`PlayerSkillTree.Initialize`, `RegisterListener` and the timer. Where the game has the definition already loaded
(A1), it fills the tree inside the call; Off fills it later or never. That changes when the tree fills, not what
`OnPlayerClassChange` does next.

## 6. Open questions

1. Whether the controller's reset tolerates a tree with no branch list (a definition that has not arrived). The
   reset's implementation is reached only through the controller's virtual table, which the read-only query could not
   locate for `WillowPlayerController`. Not read.
2. Which engine switch makes the manager load at once instead of queueing the request.
3. Whether the skill tree definition is already loaded when a class change runs: always, never, or depending on the
   path (a new game against a save load). This decides whether the completion runs inside `InitPlayerSkillTree` or
   later.
4. The receiver of an immediate delivery: the requesting context, as assumed here, or another object.
5. The frame on which a queued completion arrives (the streaming load's latency).
6. Whether `WillowPlayerInput.SetupInputDevices` and `UpdateInputContext` reach anything the headless slice cannot
   answer (section 4, step 4). Not read.

## 7. Live checks that would settle these rules (bl-sdk, the RTSE way; none run, no session granted)

Run each one twice:
- at a class change on a fresh character, where `OnPlayerClassChange` clears the tree first, so the native makes a
  new one;
- with the native called directly on a controller whose tree already exists, the existing-tree case of P2.

Run them at a new game and at a save load as well. The **state snapshot** read at each boundary below is:
- `PlayerSkillTree` and its outer;
- for each skill of the tree, its grade;
- for each tier, its unlocked flag;
- the tree's listener count, where readable;
- the replication info's `GeneralSkillPoints`.

1. **Residency before the request** (A1, open question 3). In a pre-hook of `InitPlayerSkillTree`, read the player
   class's `SkillTreePath`. Look the object at that path up with the SDK's object finder, without loading anything,
   and record whether it is resident and whether the path holds a dot (section 2, point 4).
2. **The native's frame.** Take the state snapshot in a pre-hook and in a post-hook of `InitPlayerSkillTree`, and
   record the frame number at each.
3. **The callback.** Hook `WillowPlayerController.RunStreamingDataEvent`, and record:
   - **the receiver** (the hooked `self`, and whether it is the requesting controller, open question 4);
   - the event name;
   - the other participant and its path;
   - the instigator;
   - the int and the float;
   - the frame;
   - whether the native's pre-hook has fired and its post-hook has not, which means the callback ran inside the call
     (A1 and A2, open questions 2, 3 and 5).
   Take the state snapshot at the callback's entry and at its exit.
4. **The completion's order.** Hook `PlayerSkillTree.Initialize` and `PlayerSkillTree.RegisterListener`, and record
   their order and frames against points 2 and 3.
5. **The reset's client event** (A5). Hook `WillowPlayerController.ClientResetSkillTree` and record:
   - whether it is called inside the native's frame;
   - its argument;
   - the controller's `Role`;
   - whether the controller is local.
6. **The reset on each tree** (A4, P2, open question 1). The internal reset is a virtual call, not a script event, so
   no hook sees it. Compare the post-hook snapshot of point 2 with the pre-hook one, and with the callback's exit
   snapshot when the callback ran inside the call. The difference is what the reset did, on a new tree and on an
   existing one.
7. **What comes next** (section 4). Hook `WillowPlayerReplicationInfo.IsInvalidName`, `UIInteraction.GetLoginStatus`,
   `WillowPlayerInput.SetupInputDevices` and `WillowPlayerController.UpdateInputContext` during
   `OnPlayerClassChange`. Record which of them fire, and the owner's `IsLocalPlayerController` answer.

## Cross-references

- NATIVE_CLASS_STARTING_VALUES.md (#444), sections 2 and 3: the override and the event that call this native.
- NATIVE_SKILLS.md, sections 3.1 (the reset), 3.3 (the outline this note details) and "PlayerSkillTree.Initialize".
- ROADMAP.md, Phase 4: Maya's skill trees (Motion, Harmony, Cataclysm) need the delivered definition (section 3).
  The slice minimum in section 5 does not deliver it.
