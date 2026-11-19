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

**Every rule here is UNVERIFIED in the running game.** Section 7 lists the bl-sdk observations that would confirm
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

   **This is the likely next stop for a headless local player:** `GetLoginStatus` needs the UI layer, as
   `GetDataStoreClient` does (decision 2026-11-19-19).
4. **`SetupInputDevices`** (script) runs, without any gate. It asks the controller's `PlayerInput`, cast to
   `WillowPlayerInput`, to set up its devices. That is a native of `WillowPlayerInput` and is not read here. Without a
   player input object the step is skipped. Then it clears `InputContextSelector` and calls `UpdateInputContext`,
   which was not read here.

## 5. The Off answer, step by step (for an implementer, under the hub's write rule)

The hub rule: everything that can refuse is decided before the first write. A refusal that is possible only after a
write is terminal, and its text starts "stopped after partial writes".

**Preflight (no writes).**

| # | Read | Off answer |
|---|---|---|
| P1 | the controller's `PlayerSkillTree` | set: skip to P5; None: continue; the field missing from the class: a data error |
| P2 | the class `WillowGame.PlayerSkillTree` in the code store | found: continue; missing: the store's own error or `NotYet` (not a None) |
| P3 | the engine globals and their `ThePlayerPawnDataManager` | no globals: the world's `NotYet` (`LayerAbsent`); the field None: no request (step W3 is skipped) |
| P4 | the controller's `PlayerClass`, and with it `SkillTreePath` | None: no request; a string: the request path |
| P4a | the port's loader for the request (section 2) | when a request will be made and the port has no loader that can take it: the named `NotYet` (`NotRead`) "LoadSkillTreeDefinitionAsync: the skill tree definition's streaming load", before any write |
| P5 | the reset (NATIVE_SKILLS.md section 3.1) on the tree it will run on | when the port has no Off reset: the named `NotYet` (`NotRead`) "ResetSkillTree: the controller's tree reset", before any write |
| P6 | the client event `ClientResetSkillTree` | on the authority, the script does nothing, so the port may run it through the VM or skip it; below the authority it is the VM's own call |

Reading the globals before creating the tree (P3 before W1) changes nothing observable: in the game nothing runs
between the two.

**Writes, in order.**

| # | Write | Off answer |
|---|---|---|
| W1 | a new `PlayerSkillTree`, outer the controller | the object |
| W2 | the controller's `PlayerSkillTree` | the new object |
| W3 | the request, only when P3 and P4 both gave one | the port records the request with all seven arguments (section 2). A path without a dot records nothing. Delivery: see the slice minimum |
| W4 | the reset with both flags false | refund 0 on an empty tree; no listener; the client event with false |

The native returns nothing. Nothing after W1 can refuse once P4a and P5 have passed. If the port delivers a completion
inside W3 (the already-loaded case), the completion's script runs in the VM, and whatever it reaches answers as the VM
answers any call. A `NotYet` raised there falls after W2, so it is terminal and must start "stopped after partial
writes".

**Slice minimum** (so that `OnPlayerClassChange` goes on and `maya:8` can reach the ammo maxima). The tree is not
needed for the pools or the ammo maxima: nothing in sections 1 to 4 reads it afterwards. The smallest honest port:
- Do P1 to P6.
- Make the tree (W1, W2).
- Record the request (W3) as pending, with no delivery in this lane. This is the state the game is in right after the
  call when the definition is not yet loaded, and that is the normal case for an asynchronous load. The recorded
  request must stay visible, so that a later lane can deliver it. Delivery means running `RunStreamingDataEvent`, and
  through it `PlayerSkillTree.Initialize`, `RegisterListener` and the timer.
- Reset the empty tree (W4). That writes nothing on an empty tree, and the client script does nothing on the authority.

The fork must be labelled UNVERIFIED and stated in the decision file: whether Maya's definition is already loaded at
the class change (open question 3). If it is, the game initialises the tree inside the call, and the minimum above
differs from it in when the tree fills, not in what `OnPlayerClassChange` does next.

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

## 7. Live checks that would confirm these rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `WillowPlayerController.InitPlayerSkillTree` before and after, at a class change and at a save load. Before:
   `PlayerSkillTree` is None. After: it is set, its class is `PlayerSkillTree` and its outer is the controller.
2. Hook `WillowPlayerController.RunStreamingDataEvent`. Record:
   - the event name (`SkillTreeDefinitionLoaded`);
   - the other participant, whose path should equal the player class's `SkillTreePath`;
   - the instigator (None);
   - the int and the float (0);
   - whether it fires inside the `InitPlayerSkillTree` hook (open questions 3 and 4) or how many frames later
     (open question 5).
3. Hook `PlayerSkillTree.Initialize` and `PlayerSkillTree.RegisterListener`, and check that the order is as in
   section 3.
4. Hook `WillowPlayerController.ClientResetSkillTree` and check whether it is called during `InitPlayerSkillTree`, and
   with false. Its arrival on a local authority controller is what P6 assumes.
5. Read Maya's `PlayerClass.SkillTreePath` live, and check that it holds a dot (section 2, point 4).
6. Hook `WillowPlayerReplicationInfo.IsInvalidName` and `UIInteraction.GetLoginStatus` during `OnPlayerClassChange`,
   to confirm the section 4 path and the next stop.

The internal reset (step 5) is a virtual call, not a script event, so a hook does not see it. Read the tree's grades
and tier flags before and after the call instead.

## Cross-references

- NATIVE_CLASS_STARTING_VALUES.md (#444), sections 2 and 3: the override and the event that call this native.
- NATIVE_SKILLS.md, sections 3.1 (the reset), 3.3 (the outline this note details) and "PlayerSkillTree.Initialize".
- ROADMAP.md, Phase 4: Maya's skill trees (Motion, Harmony, Cataclysm) need the delivered definition (section 3).
  The slice minimum in section 5 does not deliver it.
