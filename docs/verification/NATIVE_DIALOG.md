# Native dialog: event triggering, talk selection, line end, kickoff chain (2026-10-05)

AI-assisted (Claude), analyst lane G6. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Where a rule was cross-checked against the installed package data
(a structural oracle, not a game run) the line says so; that checks the data layout the rule needs, nothing more.

Field names are those of the script classes (offsets were named with `tools/ghidra/class_layout.py`; every field the code
touched matched a declared field of the expected type, which is the only oracle for them). Script-side behaviour was read
from the disassembled script of GearboxFramework and WillowGame (`research/script_disasm.py`). The dialog natives are
almost all *virtual* natives: the registered exec function only reads its arguments and calls through the object's
vtable, so the rules below describe the implementing virtual, found through the class vtable. Overlap: G2 (behavior kernel:
`bIsInitialRunOfThisBehavior`, `NextExecutionDelayTime`, link selection), G3 (world time, ProcessEvent-style event calls),
G4 (AI hold tokens, pawn helpers), G5 (echo HUD, subtitles), C1 (mission tracker kickoff tick).

## Summary
| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| Behavior_TriggerDialogEvent.ApplyBehaviorToContext (script) | `event ApplyBehaviorToContext(...)` | Fire: kickoff, objective chatter, turn-in | high | UNVERIFIED |
| Behavior_TriggerDialogEvent.TriggerDialogEvent | `native function TriggerDialogEvent(Object Context, Object Self, Object Instigator, Object Other, out GearboxDialogEventData EventData)` | the actual trigger | medium-high | UNVERIFIED |
| GearboxDialogManager.TriggerGroupEvent | `native function GearboxDialogEventData TriggerGroupEvent(GearboxDialogGroup, GearboxDialogEventTag, Object Instigator, Object Other, ...)` | picks the event node, starts the chain | high | UNVERIFIED |
| GearboxDialogComponent.TriggerEvent / GetMatchingEvent | `native function GearboxDialogEventData TriggerEvent(GearboxDialogEventTag, Object Other, Object ObjectParameter, optional GearboxDialogEventData)` | talker-owned events (no group given) | medium | UNVERIFIED |
| GearboxDialogNode.ActivateOutput and the Act_*.Activate natives (Talk, Chance, Compare, ObjectParameterSwitch, Trigger; Willow Talk, RandomBranch, MissionSwitch) | `native function Activate()` / `ActivateOutput(int)` | the node graph that picks a line | medium-high | UNVERIFIED |
| GearboxDialogComponent.Talk | `native function Talk(GearboxDialogAct_Talk)` | starts the audio, sets the live-line state | medium-high | UNVERIFIED |
| (component per-frame update; not a registered native) | - | detects the end of the audio, applies OutputDelay, fires the act's output | high | UNVERIFIED |
| GearboxDialogComponent.StopTalking / IsTalking / TalkReplicated | `native function StopTalking(optional GearboxDialogEventTag, optional bool)` etc. | interrupt, "is the line live" | medium-high | UNVERIFIED |
| GearboxDialogManager.RegisterTalker / UnregisterTalker / EnableTalker / DisableTalker / AddGroup / SilenceGroup / Get/SetGroupEventTag / GetPriority / GetEventTagForEventInfo / Cleanup | see package | talker registry, priority arbitration | medium | UNVERIFIED |
| WillowDialogManager.PlayEchoDialog / IsMissionKickoffPlaying / GetPriorityForEchoActor | `native function PlayEchoDialog(GearboxDialogEventTag, GearboxDialogNameTag, optional bool)` | echo callers (Marcus lines are echo events) | medium | UNVERIFIED |
| WillowDialogGlobalsDefinition.Get / TriggerTemplateEvent / StaticTriggerTemplateEvent | see package | generic events (pain, kill chatter) | low-medium | UNVERIFIED |
| GearboxSeqAct_TriggerDialogName (Kismet action, no registered native) | - | Fire: `..._17_EchoX_Marcus` after the dummy is reset | medium | UNVERIFIED |

## 0. The model in one page

- **Data.** A `GearboxDialogGroup` holds `DialogEvents` (tag, `bEnabled`, optional `OutputAction`), an inline `Nodes` list,
  a shared event node and a shared Talk act (`SharedDialogEvent`, `SharedTalkAct`), the `TalkActs` template array and the
  link table `OutputLinksToStructs` (FromNodeID, LinkNumber, ToNodeID). A `ParentGroup` chains groups. A
  `GearboxDialogEventTag` carries a `Priority` object and flags; the Willow subclass adds `bIsEchoEvent`,
  `bDoesNotOverrideSamePriority`, `bMultiplayerOnly`, `bOncePerSession`.
- **Flow.** Behavior (or Kismet action, or code) -> manager/component "trigger event" -> the group's event node for the
  tag -> `ActivateOutput(0)` along links -> chance/compare/switch nodes -> a Talk act -> the talker's dialog component
  starts a Wwise event -> the component notices the end of the audio on its own update, waits `OutputDelay`, then fires the
  act's output 0 -> (no further link) the chain ends and the event data becomes inactive.
- **What "the dialog is finished" means to a behavior.** The behavior polls `GearboxDialogEventData.IsActive`, which is
  true exactly while a Talk act is *live* (its `LiveTalkAction` is set). A line that never started (blocked by priority, no
  talker, no audio device, no audio event) makes the event data inactive at once, so the behavior's `Finished` fires at
  once. The line length is the audio length (+ OutputDelay), not a stored duration.

## Behavior_TriggerDialogEvent.ApplyBehaviorToContext (script)
- **Signature:** the usual behavior event; reads `KernelInfo` (G2).
- **Reads:** `EventTag`, `Group`, `NameTag`, `Other`, `bForcePlayImmediate`, `MyEventData`, `MyDataUseCount`;
  `KernelInfo.bIsInitialRunOfThisBehavior`, `KernelInfo.bHasLinkedOutputs`; `BehaviorHelpers.IsBehaviorsV2(KernelInfo)`
  (true when the kernel info carries a live kernel, i.e. the behavior runs under the thread kernel).
- **Does (in order):**
  1. "Immediate mode" = (not running under the V2 kernel) or `bForcePlayImmediate`.
  2. With an `EventTag`:
     - **initial run and not immediate:** set `NextExecutionDelayTime` to **0.001 s** (the thread becomes latent; per the
       kernel note the wait is at least 1/60 s) and do nothing else yet;
     - **otherwise** (a resumed run, or immediate mode): if `MyEventData` is empty (or immediate) call the native
       `TriggerDialogEvent` (below), which stores the returned event data in `MyEventData` and its `UseCount` in
       `MyDataUseCount`; immediate mode then clears both again. Then, if `MyEventData` is empty, or no longer active, or
       its `UseCount` differs from `MyDataUseCount` (the pooled data was reused for another event), or immediate mode,
       or the behavior has **no linked outputs**: select output **1 (Finished)**. Otherwise set `NextExecutionDelayTime`
       to **0.1 s** and wait (poll).
  3. Without an `EventTag`: select output 1 at once.
  4. At the end of every call: **if this is the initial run, select output 0 (Out)**.
- **Net timing:** `Out` fires on the first run, at once, *before* the dialog is triggered. The trigger happens about one
  kernel wake (>= 1/60 s) later. `Finished` fires when the live line ends, up to 0.1 s late (poll granularity), or
  immediately if no line started. In immediate mode both are selected in the order *Finished, then Out* (list order; the
  kernel note says the first selected link continues the current thread, later ones start new threads).
- **Calls into script:** none besides `IsBehaviorsV2` (native).
- **Edge cases:** `MyEventData`/`MyDataUseCount` are fields of the behavior *definition object*, shared by every consumer
  of the provider; two consumers running the same behavior at once overwrite each other's poll state. If `bHasLinkedOutputs`
  is false the behavior never waits (the dialog still plays).
- **Implementer checklist:**
  - first run: select Out, mark latent for one wake; second run: trigger; poll every 0.1 s while the event data is active;
  - select Finished on the first poll where the event data is inactive (or the use count changed);
  - Out must not wait for the dialog; Finished must wait for the audio.
- **Open:** whether `bIsInitialRunOfThisBehavior` is false on resumed runs (G2).

## Behavior_TriggerDialogEvent.TriggerDialogEvent (native virtual)
- **Signature:** `(Object Context, Object Self, Object Instigator, Object Other, out GearboxDialogEventData EventData)`; the
  out parameter is not written; the result is stored in the behavior's own `MyEventData`.
- **Does (in order):**
  1. Returns without effect if `EventTag` is None or the tag reports itself invalid (see "Tag validity").
  2. Finds the world's dialog manager (a field of the engine's globals object); none -> return.
  3. Resolves a talker: the `Context` object if it implements the dialog interface; if not and `NameTag` is set, the
     manager's talker for that name tag, **created as an echo caller if absent** (see "Name tag to pawn").
  4. If `Group` is set: calls the manager's `TriggerGroupEvent(Group, EventTag, Instigator := Context, Other := Other)`.
     **The `Instigator` argument of the behavior is not used; the Context object is the dialog's instigator.** The talker
     resolved in step 3 is not used on this path. (A DLC special case matching one group/tag/name tag triple of an
     unrelated episode takes a different route; irrelevant here.)
  5. If `Group` is None: needs the step-3 talker; calls that talker's component `TriggerEvent(EventTag, Other)`.
  6. Stores the result in `MyEventData` and its `UseCount` in `MyDataUseCount`.
- **Consequence for the Fire data:** `Behavior_TriggerDialogEvent_1184` names `DialogName_HYP_Engineer` while the group lists
  `DialogName_HypEngineer`. The behavior's `NameTag` plays no part when `Group` is set (all Fire dialog behaviors set it), so
  the mismatch noted in SLICE_AUDIO_CHAIN has no effect; the line's talker comes from the act's TalkData (below).
- **Implementer checklist:** with a group: one manager call, instigator = the behavior's context object; `NameTag` ignored.

## GearboxDialogManager.TriggerGroupEvent
- **Signature:** `(GearboxDialogGroup Group, GearboxDialogEventTag Tag, Object Instigator, Object Other, Object ObjectParameter, optional GearboxDialogEventData Reuse)`; returns the event data or None.
- **Does (in order):**
  1. None if the manager, group or tag is None or the tag is invalid.
  2. For a tag that is not a sound-effect tag (`bSoundEffect` false) on a **network client**: returns None (clients do not
     start non-effect events; the server replicates). Single-player/standalone passes.
  3. Asks the group for the event node of the tag (`FindEvent`, below). None -> return None.
  4. Takes an event data object: the caller's `Reuse` if given, else one from the **pool** (below); sets its
     `EventInfo` (the event node and its `NodeID`).
  5. Makes that event data the manager's `CurrentEventContext`, sets `Instigator`, `Other`, `ObjectParameter`, clears
     `LastTalker`.
  6. Activates output 0 of the event node (graph flow, below). The whole chain up to the first Talk act, and that act's
     start, runs **synchronously inside this call**.
  7. Returns the event data (active or not).
- **FindEvent (group):** walks `DialogEvents` in order; an entry counts if its `Tag` is the requested tag (identity
  comparison) and it is enabled (or disabled entries were explicitly allowed). The **last** matching entry wins. The
  group's shared event node is then bound to that entry: its `bDisabled`, `Tag`, and its single output link are copied
  from the entry (`OutputAction`), and its `NodeID` is the entry's 1-based index.
- **Pool:** the manager keeps `EventDataPool` (initial size 5, from the ini key `DialogEventDataPoolSize` in section
  `GearboxDialog`). The first pooled data that is not active is reset, bound to the event, and its `UseCount` incremented
  (20-bit wrap); if all are active a new one is created and appended.
- **Implementer checklist:** the last enabled matching entry wins; the chain runs inside the call; return value is the data
  even when nothing started; `UseCount` increments on reuse.

## Graph flow: node outputs and the Activate natives
- **ActivateOutput(n)** (node): if output `n` has a direct link in the node's own link list, its first target is activated.
  Otherwise the group's link table is searched for (this node's `NodeID`, `n`); the found `ToNodeID` is resolved by asking
  the shared event node, then the shared Talk act, whether they can *represent* that id (see "Shared template nodes"); the
  matching one is activated. Activating means running the target's `Activate` (invoked as a script-callable event, so a
  script override would run).
- **Shared template nodes:** the event node represents ids 1..(number of DialogEvents); the Talk act represents ids
  (number of DialogEvents)+1 .. +(number of TalkActs), taking `OutputDelay`, `bInstigatorTalker`, `TalkData`, the talker
  variable link and the output link from `TalkActs[id - Events - 1]`.
  **Structural check (data, not game):** in `GD_VOSQ_RockPaperGeno.Groups.DialogGroups_Side_RockPaperGeno` there are 35
  events; 28 have an inline `OutputAction` Talk act (28 inline nodes exist); the other 7 are events 11, 18, 21, 26, 28, 32,
  34 (1-based), exactly the 7 `FromNodeID`s of the link table, whose `ToNodeID`s are 36..42, i.e. `TalkActs[0..6]` in order.
  So the earlier "pair in order" heuristic (SLICE_AUDIO_CHAIN) matches the native rule here, but the rule is the link table.
- **Act_Chance.Activate:** fields `Chance`, `QuietTimeMin`, `QuietTimeMax`, `NextFireTime`. If now >= `NextFireTime` and a
  uniform draw in [0,1) is below `Chance`: set `NextFireTime = now + Min + (Max - Min) * draw2` and activate output 0.
  Otherwise (including during the quiet time) output 1.
- **Act_Compare.Activate:** needs two variable links; asks each variable for its talkers; output 0 if the two sets share an
  element, else 1 (also 1 when a variable is missing).
- **Act_ObjectParameterSwitch.Activate:** compares the current event data's `ObjectParameter` with each entry of `Outputs`;
  activates output *i* for **every** equal entry; if none, the **last** output.
- **Act_Trigger.Activate** (template call): guarded against re-entry. Needs `DialogEvent` and a talker variable; collects the
  variable's talkers (for a non-effect tag only those that can talk it), picks one at random, stores the current event info as
  `TemplateEventInfo` and itself as `LiveTriggerAction`, then triggers `DialogEvent` on that talker (component `TriggerEvent`
  reusing the same event data; for effect tags the group's simple-event path). If the result has a live talk act the act
  returns and its output 0 fires **later, when that line finishes** (the end-of-line rule below); otherwise output 0 now.
  Its `ActivateOutput` first restores the saved event info.
- **WillowDialogAct_MissionSwitch.Activate:** needs the local player's mission tracker; `TrackedState` 1 requires the
  tracked mission to be this act's `MissionDefinition`, 2 requires it not to be, otherwise no check; if the filter passes it
  activates the output equal to the mission's status number (NotStarted 0 .. Failed 5); if it fails nothing is activated.
  Low confidence on the early-exit gate.
- **WillowDialogAct_RandomBranch.Activate:** fields `Chances`, `QuietTimeMin/Max`, `Mode`, `AvoidRepeatingLastNPlayed`,
  `NextFireTime`, runtime history list. Only acts when now >= `NextFireTime`; trims the history to the last N entries
  (mode- and N-dependent), picks an index by weighted draw over `Chances` excluding indices in the history, activates it,
  sets `NextFireTime = now + Min + (Max - Min) * draw`, and records the index in the history for the modes that avoid
  repeats. The exact weighting and the three `Mode` values were not decoded. Not on the Fire route.
- **Act_Talk.Activate** (base and Willow): if none of the act's TalkData entries has an `AkEvent`, **skip: output 0 at
  once** (a line without audio is a pass-through). Otherwise choose a talker (below). Base: no talker -> the chain stops.
  Willow: no talker and `bEnableNoMatch` -> activate output 1 instead; no talker otherwise -> stop. With a talker, call
  the talker's dialog component `Talk(act)`. (The Willow override chooses once to test and the base routine chooses again;
  with several candidates the two draws can differ. Low impact.)
- **Choosing the talker (Talk act):**
  - If the act has a talker variable linked: the variable's talkers, filtered to those that pass the validity test, one at
    random.
  - Otherwise, if `bInstigatorTalker` and an event context exists: the event's `Instigator` if it passes the validity test;
    else none.
  - Otherwise: a random entry of `TalkData`, resolved by its `NameTag` through the manager (below).
  - **Echo events** (Willow, `bIsEchoEvent`): the random-TalkData branch resolves with "create if absent" (an echo caller is
    synthesised); the other branches are as above.
  - **Validity test:** needs an actor; accepted at once for a sound-effect event; otherwise needs TalkData for the
    candidate's name tag (exact tag, or an entry whose tag is an ancestor of the candidate's tag through `ParentTag`), a
    positive can-talk answer from the interface, and the candidate not already in the act's own exclusion list.
- **Implementer checklist:** no-audio acts pass through; link-table lookup; last enabled entry wins; chance rules as above;
  talker choice order variable > instigator > random TalkData.

## GearboxDialogComponent.Talk
- **Signature:** `Talk(GearboxDialogAct_Talk Act)`, virtual; reads the live event context from the manager.
- **Does (in order):**
  1. Does nothing unless: the component has a dialog interface, **the engine's audio device is available**, a manager and an
     act exist, and there is a current event tag/info. **(No audio device means no line plays and no live-line state is set,
     so the behavior's `Finished` follows immediately; reading, UNVERIFIED.)**
  2. **Priority gate** (manager arbitration, below). Fails -> nothing starts; the event data stays inactive.
  3. Sets the current event data's `LiveTalkAction = Act` and `LiveTalkActionDataID = Act.NodeID`.
  4. If the tag is a **group event** (`bGroupEvent`; the Willow override also returns true for `bIsEchoEvent`): the manager
     silences the group (stops every registered talker that owns that group) and records the tag as the group's current
     event (`SetGroupEventTag`).
  5. Interrupts whatever this component was saying (a plain `StopTalking`), binds its `EventData`, resets
     `TalkFinishTime` to 0.
  6. Looks up the act's TalkData for this talker's name tag. If it has an `AkEvent`: posts it on the talker's audio object
     and stores the returned playing info in the event data; sets the Wwise voice priority to
     **255 - clamp(priority index, 0..254)** (a more important dialog priority gives a higher voice priority).
  7. If neither an `AkEvent` nor `AkAudioUniqueID` exists: stop here (the line is "live" until the next component update
     ends it, see below; neither `TalkStarted` nor the manager start hook run).
  8. Sets the globals' `PitchRTPC` to the TalkData `Pitch` on the talker (even when 0), notifies the talker's dialog
     interface, calls the manager's talk-started hook, then the **script event `TalkStarted(owner)` on the act**.
- **Calls into script:** `GearboxDialogAct_Talk.TalkStarted(Actor)` (Willow override starts the emote stance on the talker
  through its name tag's `BeginStance`, when `Emote` is set and the talker has a name tag).
- **Manager talk-started hook (Willow):** for echo tags records the talker as the current `EchoActor` (and its emote); for
  `bOncePerSession` tags records the tag as played (see "Tag validity"); also touches the echo HUD (G5).
- **Open:** how an `AkAudioUniqueID` without an `AkEvent` is played (not seen); the interface notification's callee.

## Line end: the dialog component's per-frame update
This is the answer to "what decides the duration". It is a component update, not a registered native.
- **Authority (single-player, standalone, server):** each frame, if the component is attached and its event data has a live
  talk act:
  1. **Audio finished** = the event data's playing info has an id and the audio object no longer reports it playing
     (`AkComponent.IsPlayingId`). Then: `delay = act.OutputDelay`, or, if the act is the group's shared Talk act and the
     data id is set, `TalkActs[id - Events - 1].OutputDelay`; clear the playing info; `TalkFinishTime = now + delay`.
  2. **Finish:** if the data has a live act, **no playing id**, and now >= `TalkFinishTime`: `StopTalking` (which clears the
     live state), set `LastTalker` to the component's owner, **activate output 0 of the finished act**; then, if the data
     has a valid template event info and a live trigger action and no new live talk act, activate that trigger action's
     output 0 (the template-call return).
  3. **Cleanup:** a data with no live act and no playing id, or whose sound ended without being collected, is stopped, reset
     and released from the component.
  So the line length is **the Wwise event's actual playing time** (the streamed source length, e.g. SLICE_AUDIO_CHAIN's
  header durations) **plus OutputDelay** (0.0 in all seven `TalkActs` and in the inline Marcus acts of the Fire group; the
  AkEvent's Min/Max duration fields are not consulted). A live act with **no playing id** (no audio started) finishes on
  the next update with **no OutputDelay**.
- **Client (network mode 3):** only clears its local playing info and tells the manager when the sound ended. Irrelevant
  to single-player.
- **Implementer checklist:** end = audio end + OutputDelay; no audio -> next frame; output 0 of the act fires on the end;
  the behavior's poll sees `IsActive` fall when the live act is cleared.

## GearboxDialogComponent.StopTalking / IsTalking
- **StopTalking(optional Tag, optional bool):** acts if the component has a live line and (no tag was given, or the live
  event's tag equals the given tag): stops the audio if still playing, clears the playing info, clears the group's current
  event tag, clears the event data's live fields (`LastTalker`, `LiveTalkAction`, `LiveTalkActionDataID`), drops the
  component's `EventData`, notifies the talker's interface and the manager's talk-ended hook (Willow: forgets the echo actor
  if it was this one), then calls the **script event `TalkFinished(owner)` on the act** (Willow override ends the emote
  stance). Without a live line and with the bool set (and not a client) it still notifies interface and manager.
- **IsTalking:** on a client, whether the playing info is valid; otherwise whether the event data has a live act.
- **TalkReplicated:** client-side counterpart (starts the sound from replicated data, sets the voice priority, pitch RTPC).

## Manager rules (GearboxDialogManager / WillowDialogManager)
- **Priority index:** the position of the tag's `Priority` object in the dialog globals' `Priorities` array
  (`GD_Globals.Dialog...DialogGlobals`): installed order is 100, 90, 80, 70, 60, 50, 40, 35, 30, 25, 20, 10, 05 (indices
  0..12). A tag without a priority, or one not in the array, is `INT_MAX` (least important); no globals gives -1. **A lower
  index is more important.**
- **Playback arbitration (before every Talk):** compare the new event's index N with (a) the index of the group's current
  event tag (group state keyed by the *root* group, found through `ParentGroup`; none = no constraint) and (b) the index of
  the talker component's own live event. The line is **blocked** if either exists and is `<= N`; if the tag's "may override
  the same priority" answer is true the test is strict (`< N`). The Willow answer is true only for echo events without
  `bDoesNotOverrideSamePriority`; the base answer is false. A *more* important (smaller index) event interrupts.
- **Willow priority floor:** if the event's group is the **tracked mission's `MissionDialogGroup`** (checked: the field exists on `MissionDefinition`; the Fire mission's is
  `DialogGroups_Side_RockPaperGeno`), the index is capped so that it is at least as important as
  `ActiveSideMissionMinPriority` (installed: `DialogPriority_35`) or, for a `bPlotCritical` mission, `ActivePlotMissionMinPriority`
  (`DialogPriority_05`), unless the base is already at least as important as `ActiveMissionMinPriorityStart`
  (`DialogPriority_20`). Installed Fire tags: `01` `DialogPriority_70`, `02`/`03a`/`05` `DialogPriority_30`, `17`
  `DialogPriority_10`; with the numbers above, an event of the same talker queued while another is live is dropped unless it
  is strictly more important.
- **Name tag to pawn (`FindTalker`):** among the registered talkers in registration order, skip pending-destroy actors, the
  first whose interface name tag equals the requested tag (**exact tag**, no ancestor test here). Willow: if none and
  creation is allowed, look at `PureEchoActors` for one with that name tag, else spawn a `WillowDialogEchoActor`, give it
  the tag, mark it, add it to `PureEchoActors`; its location is a far-away fixed point (Z = -262144 in one call; which axis
  was not confirmed). `PlayEchoDialog(Tag, NameTag, bOnlyPure)` does exactly: find the real talker (unless `bOnlyPure`),
  else find/create the echo actor, then `TriggerEvent(Tag)` on its component.
- **RegisterTalker(actor):** takes the interface's `DialogGroups`, calls `AddGroup` for each (unique add), and (reading) adds
  the actor to `Talkers`. **UnregisterTalker:** removes from `Talkers`/`DisabledTalkers`, tells every group node to drop the
  actor, and clears it from pooled event data (`Instigator`, `Other`, `LastTalker`). **DisableTalker:** if registered,
  stops its line and adds it to `DisabledTalkers`; **EnableTalker** removes it. **SilenceGroup(group):** stops every talker
  that owns the group. **Get/SetGroupEventTag:** map keyed by the root group. **GetEventTagForEventInfo:** the event's tag
  (for a shared event node the tag of the bound `DialogEvents` entry). **Cleanup:** clears the current context, event
  data and lists (Willow also destroys the echo actors).
- **Component events are gated by the manager's `bEnabled`** (component `TriggerEvent` only); `TriggerGroupEvent` is not.
- **`IsMissionKickoffPlaying`:** true if the echo actor currently has a live event whose priority index is `<=` the index of
  `SideMissionKickoffPriority` (installed `DialogPriority_70`). `GetPriorityForEchoActor`: that live event's index, or INT_MAX.

## Component TriggerEvent / GetMatchingEvent (no group given)
- **GetMatchingEvent(Tag, out Event, out Group, bAllowTemplates, ...):** over the talker interface's `DialogGroups`, in order,
  the first group with a matching event; a group without a match appends its `ParentGroup` to the search; template groups are
  skipped unless allowed. As a side effect it registers the talker with the manager when it had groups.
- **TriggerEvent(Tag, Other, ObjectParameter, Reuse):** needs an enabled manager, an interface, a valid tag. On a client with a
  non-effect tag it forwards to the server (`ServerDialog_TriggerEvent`) once (re-entry guarded) and returns None. Otherwise as
  `TriggerGroupEvent` with **Instigator := the component's owner**. Effect tags use the group's `SimpleEvent`: play the
  entry's AkEvent once on the owner (plus pitch RTPC), no live-line state, no end detection.

## Tag validity (WillowDialogEventTag)
The tag answers "valid" except: `bOncePerSession` and already recorded as played in the session list -> invalid;
`bMultiplayerOnly` and fewer than two players -> invalid. Checked by the behavior, the group trigger and the component.

## GearboxSeqAct_TriggerDialogName (Kismet)
The class is a latent action (`SeqAct_Latent`) with fields `Other`, `EventTag`, `NameTag`, `EventData`, `MyDataUseCount`,
`Group`. On activation: with `EventTag` and `Group` set it calls the manager's `TriggerGroupEvent(Group, EventTag, instigator,
...)` where the instigator is the action's first target, or, with no target, the talker for `NameTag` (created as an echo
caller if absent), and keeps the result in `EventData`/`MyDataUseCount` (how `Other` is passed was not confirmed); without
`Group` it triggers `EventTag` on the instigator's dialog component with `Other`. The action stays **active** while its
event data is active and its use count unchanged (its update reports "not done"), and finishes when the line ends or at
once if nothing started (so its output fires on the end, like `Finished`). The Fire data's
`GearboxSeqAct_TriggerDialogName_1` (event `..._17_EchoX_Marcus`, name tag Marcus) plays an echo line this way.

## The kickoff chain in native terms (confirmation)
C1: the tracker tick calls `PlayKickoff` after acceptance, which fires `Default` id 12. Fire data: id 12 reaches
`Behavior_TriggerDialogEvent_1185` (tag `..._01_EchoX_Marcus`, Marcus, group `DialogGroups_Side_RockPaperGeno`).
1. Kernel runs the behavior: initial run -> **Out selected now**, thread latent 0.001 s.
2. Resumed run: `TriggerGroupEvent(group, tag 01, instigator = context object, other = None)`.
3. Group: last enabled entry for tag 01 = `DialogEvents[0]`, whose `OutputAction` is the inline Talk act (Marcus,
   `Ak_Play_VOSQ_RockPaperGeno_01_EchoX_Marcus`, no `OutputDelay`, no instigator flag).
4. Talk act: has an AkEvent; echo event -> Marcus resolved by name tag, an echo actor is created if no Marcus pawn is
   registered; `Talk`: audio device must be available; priority index of tag 01 is 3 (`DialogPriority_70`), nothing live ->
   allowed; live state set; group/echo group event recorded; Wwise event posted; `TalkStarted` runs.
5. Behavior polls every 0.1 s: event data active.
6. The audio ends (header duration 17.666 s for this source, SLICE_AUDIO_CHAIN); within a frame the component update sets
   the finish time (= now, `OutputDelay` 0), then ends the line, `LiveTalkAction` clears; the act's output 0 has no link.
7. Next poll: inactive -> **Finished (id 1) selected**; the data's `Finished` links (per NATIVE_MISSION_DISPATCH B9:
   `RocksPaper_MoveMarcusToRange`, AdvanceObjectiveSet to `GoToRange_ObjSet`) run. So the first objective set starts
   **after the audio length** (about 17.7 s plus <= 0.1 s), not at acceptance. Without an audio device it would follow within
   one poll. Mission-level `DialogEvent`/`DialogTalker` (C1) are unset for Fire and not read here.
- Not re-decoded here: which behaviors hang off Out (id 0) of `_1185`.

## Subtitles and echo HUD (brief, presentation)
Echo events set the manager's `EchoActor` (above) and drive the HUD's echo caller display through the name tag's echo portrait
natives (`StaticShowEchoPortrait` / `StaticHideEchoPortrait`) and `WillowHUDGFxMovie.ShowEchoCaller` / `HideEchoCaller`;
`WillowPlayerController.DisplaySubtitle` is native. None of these decide timing; they were not read (G5).

## Not read yet
- `GearboxDialogVariable.ResolveToArgumentValue` and the concrete variable subclasses (only the name-tag variable's talker
  match, "interface name tag equals the variable's tag", was seen).
- `AIComponent.HoldDialog` / `ReleaseDialog` / `DialogOnHold` (hold tokens), `AIPawn.CanTalk`, pawn `SetDialogNameTag`
  (G4); `WillowDialogNameTag.BeginStance`/`EndStance` internals; the emote/stance data.
- The audio-device availability predicate behind the Talk gate; `AkAudioUniqueID`-only playback; the interface notification
  hook; `bOncePerSession` storage details; RandomBranch weighting and modes; `WillowDialogGlobalsDefinition.TriggerTemplateEvent`
  contents (pain/kill chatter tables); replication helpers (`TalkReplicated`, `ServerDialog_TriggerEvent`, replicated data).
- Echo actor placement axis; `DrawDialogDebug`.

## Corrections to earlier notes
- SLICE_AUDIO_CHAIN: "events with null OutputAction pair in order with the inline TalkActs" is a consequence of the link
  table (checked on the data); the "trigger name tag differs from the group's" item is explained (NameTag unused when Group is
  set). Durations: the line ends on the real audio end, not on the AkEvent Min/Max.
- NATIVE_MISSION_DISPATCH A2/B9 and SANCTUARY_RPG_MISSION: `Behavior_TriggerDialogEvent` does not select `Out` and `Finished`
  together. `Out` is selected on the first run; `Finished` only when the line ends (or at once if no line started). The
  host's stand-in (both selected at once) therefore starts `GoToRange_ObjSet` immediately instead of after the Marcus line.
