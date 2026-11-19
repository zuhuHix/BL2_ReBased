# Native rules of running behavior lists outside the kernel: RunBehaviors, ApplyBehavior and their callers (draft, 2026-11-19)

AI-assisted (Claude), analyst lane RUN-BEHAVIORS (decision 2026-11-19-47). Script first: the installed packages read
with `research/script_disasm.py`. The natives the script does not show were then read from the game executable with
Ghidra (`tools/ghidra/run.ps1 -ReadOnly`, under the machine lock). Written in our own words, as behaviour only: no
decompiler output, pseudo-code, quoted script statements, addresses, offsets or call structure is reproduced here.
Field and struct names come from the cooked packages. **Every rule is UNVERIFIED in the running game.** The last
section lists the live observations that would confirm them.

**Clean room.** This lane implements nothing. Everything here stays unbound or `NotYet` in Off until a fresh
implementer codes it from this note alone.

**Why it matters.** `BehaviorBase.RunBehaviors` is what 8 Reference checks of `tests/mission_script_test.py` stop on
during the level-up (S7b, S9, S10). The special-move start of slice link 9 runs behavior lists the same way
(NATIVE_SPECIAL_MOVE_PLAY.md). NATIVE_DOWN_TIMERS.md already gives the loop in one paragraph and calls the record empty. This note
refines that (one flag of the record is set, section 1) and adds the script it reaches and the two special-move
callers.

## Summary

| Piece | Kind | Confidence | Status |
|---|---|---|---|
| `BehaviorBase.RunBehaviors(out Behaviors, SelfObject, optional MyInstigatorObject, OtherEventParticipantObject, EventData)` | native, static | high | UNVERIFIED |
| `BehaviorBase.ApplyBehavior` (the default) and `ApplyBehaviorToContext` (the default, empty) | script | high | UNVERIFIED |
| `Behavior_RunBehaviorCollection` | script | high | UNVERIFIED |
| `InstanceDataHelper.GetFirstInstanceDataObject(ContextObj, DataName, DefaultValue)` | native | medium-high | UNVERIFIED |
| `SpecialMoveComponent.RunBehaviors(SMBehaviors)` and `CanPlayBehaviorData` | native | high | UNVERIFIED |
| `GearboxAnimDefinition.OnStart` / `OnServerStart(ConsumerHandle)` | native | medium | UNVERIFIED |

## 1. BehaviorBase.RunBehaviors

1. **One shared kernel-info record** (`BehaviorKernelInfo`) is made for the whole call. It has no running-thread state,
   all times are 0 and `bHasLinkedOutputs` is clear; only `bIsInitialRunOfThisBehavior` is set (this refines
   NATIVE_DOWN_TIMERS.md, which calls the record empty). Every behavior of the
   list gets the same record, so a later one sees what an earlier one wrote into it.
2. **The list is walked in index order.** Its length is read again at every step, so a behavior that changes the list
   changes the walk. Empty entries are skipped.
3. **For each behavior** the event data is copied, and the behavior's script event `ApplyBehavior` runs with the
   self object, the shared record, the instigator, the other participant and that copy.
4. **No result, and nothing is checked.** A behavior that does nothing, or fails inside its script, does not stop the
   walk. An empty or None list does nothing.
5. **Not a kernel thread.** No behavior consumer, provider or process is involved. A behavior that asks the kernel to
   follow an output link or to wait (`NextExecutionDelayTime`) has no thread to act on, so nothing follows from it,
   as read (UNVERIFIED; open question 1).
   Reading the record as having no thread is this read's (UNVERIFIED); NATIVE_MISSION_DISPATCH.md A2 describes the
   kernel's own threads.

## 2. ApplyBehavior and ApplyBehaviorToContext (script)

- **The default `ApplyBehavior`** resolves **every** context the behavior's `Context` data names. It uses
  `BehaviorBase.StaticGetAllBehaviorContexts` with the self object, instigator, other participant and event data;
  the context rules are NATIVE_BEHAVIOR_CONTEXT.md's. When that answers false, nothing runs. Otherwise
  `ApplyBehaviorToContext` runs once per resolved context, in the resolver's order, with the same record and
  arguments.
- **The default `ApplyBehaviorToContext`** does nothing. Each behavior class's own override is its rule.
- A behavior class may override `ApplyBehavior` instead, as `Behavior_RunBehaviorCollection` does (section 3).

## 3. Behavior_RunBehaviorCollection (the level-up's behavior)

Its `ApplyBehavior` and its `ApplyBehaviorToContext` both lead to one step:
1. **One context** is resolved from the behavior's `OverrideContext` (`StaticGetBehaviorContext`,
   NATIVE_BEHAVIOR_CONTEXT.md).
2. **The collection** is found with `InstanceDataHelper.GetFirstInstanceDataObject`: the instance data named
   `OverrideName` on that context, when there is one, and otherwise the behavior's own `CollectionDefinition`. It must
   be a `BehaviorCollectionDefinition`. Without one, nothing runs.
3. **Its `Behaviors` run** through `RunBehaviors` (section 1), with a new record. The arguments are the **original**
   self object, instigator, other participant and event data: the resolved context only serves to find the instance
   data.

**`InstanceDataHelper.GetFirstInstanceDataObject(ContextObj, DataName, DefaultValue)`:**
- answers `DefaultValue` when either the name or the context object is missing;
- otherwise asks the context's instance-data interface for its first entry of that name. An entry that holds an
  object answers it (which entry kinds count is open question 2);
- any other case answers `DefaultValue`.

**The level-up.** The controller's level-up script runs `RunBehaviors` over the player class's `OnLevelUp` list, with
the pawn as self and the pawn's instigator as instigator, and then over `OnLevelUpNaturally` when `bNaturalLevelup` is
set. Both runs sit behind the guard of NATIVE_LEVEL_UP_ATTRIBUTES.md section 6 step 3 (`bFeedback`, and more than
1.0 second since `LastLevelUpTime`). Maya's `OnLevelUp` is one
`Behavior_RunBehaviorCollection` naming `PlayerBehavior_LevelUp`, whose four behaviors run in data order
(NATIVE_LEVEL_UP_ATTRIBUTES.md section 7). So the level-up is two nested `RunBehaviors` calls, and each inner
behavior's own `ApplyBehavior`, through the context lookup of section 2.

## 4. The special-move callers

- **`SpecialMoveComponent.RunBehaviors(SMBehaviors)`** runs a definition's `StartBehaviors` or `StopBehaviors`:
  - nothing without an owner interface;
  - otherwise, for each `SMBehavior` entry in order that passes `CanPlayBehaviorData`, its `Behaviors` run through the
    loop of section 1, with **the owner actor as self** and the other arguments left empty;
  - the walk is not affected if the definition's list changes during it (UNVERIFIED).
- **`CanPlayBehaviorData(Data)`:** true when at least one of the entry's flags allows the current side:
  - `PlayServer`, when the world is not a network client (`NetMode` is not `NM_Client`);
  - `PlayClient`, when it is one;
  - `PlayLocal`, when the owner's pawn has a controller that is locally controlled.

  False without an owner interface.
- **`GearboxAnimDefinition.OnStart` / `OnServerStart(ConsumerHandle)`:** fire the named behavior event `OnStart` or
  `OnServerStart` of the definition's behavior provider, for that consumer. This happens only when the behavior kernel
  exists, the definition has a provider, and the consumer's process is active; otherwise nothing happens. This is the
  kernel's own event activation, the one `ActivateBehaviorEventFromScript` uses (read as such: UNVERIFIED;
  NATIVE_BEHAVIOR_POPULATION.md). After the activation the kernel is asked to go on with that process's due threads
  (read as such: UNVERIFIED).

## 5. For an implementer, in the port's terms

- `RunBehaviors` is a VM-level loop. It needs no consumer, provider or process, only the behavior objects' installed
  `ApplyBehavior`.
- The game's record has no thread, so output links selected by behaviors run this way lead nowhere. A port that runs
  `ApplyBehavior` inside `ow_behavior`'s output-link collector would collect ids that the game never follows.
- The behaviors' rules are their own script (`ApplyBehaviorToContext` overrides) and the natives those reach. On the
  level-up path these are the four behaviors of NATIVE_LEVEL_UP_ATTRIBUTES.md section 7.
- `SpecialMoveComponent.RunBehaviors` is the same loop behind the side test. `OnStart` / `OnServerStart` are kernel
  event activations, not this loop.

## Open questions

1. What a behavior's wait request does when run outside a thread: ignored as read, or kept somewhere (UNVERIFIED).
2. The names of the instance-data entry kinds `GetFirstInstanceDataObject` accepts.
3. The exact role of the kernel step that follows `OnStart` / `OnServerStart`'s event activation.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Level up with Maya. Hook `BehaviorBase.RunBehaviors` and each inner behavior's `ApplyBehavior`, and record the
  order, the self and instigator objects, and that the collection's four behaviors run once each.
- Hook an `ApplyBehaviorToContext` of a behavior with two resolved contexts and count the calls.
- During a Phaselock cast, hook `SpecialMoveComponent.RunBehaviors` and record which `StartBehaviors` entries run in
  single player (`PlayServer` and `PlayLocal` expected).
