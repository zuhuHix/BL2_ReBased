# Native behavior context: `BehaviorBase.GetBehaviorContext`, the consumer handle of a pawn, and Marcus's on-use dialog (2026-10-06)

AI-assisted (Claude), analyst lane G21. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script behaviour was read with the WillowGame listing
(`research/script_disasm.py`); data values were read with `ow-package --object-dump` and `--properties` on `Startup` and
`Sanctuary_Dynamic`; field offsets with `tools/ghidra/class_layout.py`. Related notes (reused, not repeated):
[NATIVE_MARCUS_USE_CHAIN.md](NATIVE_MARCUS_USE_CHAIN.md) (the chain this closes, open questions 1 and 2),
[NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) (sections G1/G2: runner, context lists),
[NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) (`PlayOnUseDialog` outline, mission counting),
[NATIVE_DIALOG.md](NATIVE_DIALOG.md) (`TriggerEvent`, talk acts).

## Summary

The question as it reached us ("what does `GetBehaviorContext` return for a behavior running inside a provider thread: the
context object, the consumer handle, the provider and the sequence?") rests on a wrong picture. **`GetBehaviorContext` is a
small pure resolver.** It returns **one object** and nothing else, it reads **no kernel state at all** (no thread, no
consumer, no provider, no sequence), and its result is decided by its arguments. What makes it return something useful
inside a thread is the **thread runner**, which fills the behavior's `PlayerWhoUsedMe` property (a `BehaviorContextData`
struct) from the sequence's variables **before** the behavior runs, and sets the struct's selector to "use the context
object stored in the struct". The stub that returns `None` therefore has to be replaced by the resolver below **and** the
runner has to do that struct fill; with only one of the two, Marcus's dialog stays silent.

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| BehaviorBase.GetBehaviorContext | `native final function Object GetBehaviorContext(BehaviorContextData ContextData, Object SelfObject, Object MyInstigatorObject, Object OtherEventParticipantObject, optional BehaviorParameters EventData)` | Marcus use chain: finds the player for the on-use dialog | high (selector, plain path), medium (named instance-data path) | UNVERIFIED |
| BehaviorBase.StaticGetBehaviorContext | `native static final function Object StaticGetBehaviorContext(Object DebugCaller, BehaviorContextData ContextData, Object SelfObject, Object MyInstigatorObject, Object OtherEventParticipantObject, optional BehaviorParameters EventData)` | same resolver, no instance | high | UNVERIFIED |
| BehaviorBase.StaticGetAllBehaviorContexts | `native static final function bool StaticGetAllBehaviorContexts(Object DebugCaller, BehaviorContextData ContextData, Object SelfObject, Object MyInstigatorObject, Object OtherEventParticipantObject, optional BehaviorParameters EventData, out array<Object> Contexts)` | same resolver, all results | medium-high | UNVERIFIED |
| BehaviorBase.GetBehaviorContextInterface | `native function Interface GetBehaviorContextInterface(Class InterfaceClass, BehaviorContextData ContextData, Object SelfObject, Object MyInstigatorObject, Object OtherEventParticipantObject, optional BehaviorParameters EventData, optional out Object ContextObject)` | same resolver, then an interface of the result | medium | UNVERIFIED |
| WillowPawn.GetBehaviorConsumerHandle | `native function BehaviorConsumerHandle GetBehaviorConsumerHandle()` (the only function of `IBehaviorConsumer`) | Marcus's consumer handle for every kernel call | high | UNVERIFIED |
| WillowPawn.InitializeBehaviorProviders | `native function InitializeBehaviorProviders()` | assigns the handle (spawn); open question 2 | medium-high | UNVERIFIED |
| Kernel thread runner (not a script native): input binding of a `BehaviorContextData` property, empty context list | none (internal) | the part that makes `GetBehaviorContext` return the player | medium-high | UNVERIFIED |
| Behavior_PlayAIMissionContextDialog.ApplyBehaviorToContext, WillowAIPawn.PlayOnUseDialog | script | the dialog decision | high (script is read directly) | UNVERIFIED (as a game rule) |

Declaration order and optionality come from the package declarations (parameters are stored in reverse order); `optional`
was inferred from how the executable treats an omitted argument and from call sites that pass an empty value. Parameter
flags such as `final` and `static` were not checked and do not matter for a port.

## What the pieces are

- **`BehaviorContextData`** (struct nested in `BehaviorBase`, 16 bytes): `InstancedDataContextName` (name, 8 bytes),
  `ContextObject` (object, at 8), `BehaviorContext` (byte enum, at 12), `bSupportsDefaultOutputLink` (byte, at 13). Every
  behavior has one named `Context`; `Behavior_PlayAIMissionContextDialog` adds a second, `PlayerWhoUsedMe`.
- **`EBehaviorContext` (read from the Engine package):** 0 = Self, 1 = MyInstigator, 2 = OtherEventParticipant,
  3 = EventData, 4 = UseContextObject (5 = MAX).
- **`BehaviorConsumerHandle`** (struct inside `IBehaviorConsumer`): a single `int` field `PID`.
- **Variable link types** on a behavior (provider data and runner agree): 1 = Context, 2 = Input, 3 = Output.

## BehaviorBase.GetBehaviorContext

- **Signature:** above. Called from script as `GetBehaviorContext(Property, SelfObject, MyInstigatorObject,
  OtherEventParticipantObject, EventData)` inside every `ApplyBehaviorToContext`; the first argument is a
  `BehaviorContextData` property of the behavior.
- **Reads:** only its arguments. The first resolved object is returned. (The instance-data path below also asks the chosen
  object for its instance-data entries; no kernel table is touched.)
- **Does (the rule, UNVERIFIED):**
  1. Chooses a **base object** by the struct's `BehaviorContext` byte: 0 gives `SelfObject`, 1 gives
     `MyInstigatorObject`, 2 gives `OtherEventParticipantObject`, 4 gives the struct's own `ContextObject`. **Value 3
     (EventData) selects nothing**, and so does any other value: the result is `None`. `EventData` is never consulted by
     this resolver.
  2. If the base object is `None`: result `None`.
  3. If the struct's `InstancedDataContextName` is `None` (both name words zero): the result is the base object itself.
     This is the path of Marcus's dialog behavior.
  4. Otherwise (a name is set): the base object must support the Engine `IInstanceData` interface and is asked, through
     that interface, for its instance-data entries with that name. Every returned entry that holds an object (three of the
     entry's type tags carry one) contributes that object; the result of `GetBehaviorContext` is the **first**
     contributed object, `None` when there is none or the base object has no such interface. Medium confidence; the entry
     layout and the three type tags were read coarsely. Not on the Marcus route.
  5. The script-visible result is that first object, or `None` for an empty list. Nothing else is returned or written.
- **Outside a running thread:** identical. The function has no notion of a running thread; called with the default selector
  (Self) and a `SelfObject` it returns that object, called with nothing it returns `None`. The kernel never enters it.
- **Which kernel state it reads:** none. It does not look at the consumer handle, the provider, the sequence, the thread
  or the event payload.
- **Calls other natives:** the interface query of the base object, only on the named path.
- **Constants / formulas:** the selector values above.
- **Edge cases:** `EventData` selector returns `None`; a selector of 4 with an empty `ContextObject` returns `None`;
  nothing is logged in any of these cases.
- **Implementer checklist:** (a) a pure function of the five arguments, no host or kernel access on the plain path;
  (b) selector 0, 1, 2, 4 pick the matching argument or the struct's own `ContextObject`, 3 and anything else give
  `None`; (c) a non-empty `InstancedDataContextName` goes through the instance-data lookup, `None` when unsupported;
  (d) never return a handle, provider or sequence; (e) the runner part below is required for the result to be the player.
- **Open:** exact instance-data entry layout and the three object-bearing type tags; whether `EventData` was meant to be
  used (no code path in this native reads it).

### Siblings sharing the resolver

- **`StaticGetBehaviorContext`:** same resolution; the first parameter in declaration order is a `DebugCaller` object that
  is read and ignored by the resolver. Returns the first object or `None`.
- **`StaticGetAllBehaviorContexts`:** same resolution, writes **every** contributed object into the `Contexts` out array
  and returns true when at least one object was produced (the plain path always produces one when the base object is
  set). Whether the array is cleared first was not read; the out array arrives empty from script.
- **`GetBehaviorContextInterface`:** resolves the first object as above, writes that object into the optional out
  parameter `ContextObject`, then asks it for the interface named by `InterfaceClass`; with no object (or an object
  without that interface) both the object and the interface result are empty. Used by many behaviors (for example the
  lifting, fire-shot, impact-effect ones); none is on Marcus's route.

## Kernel thread runner: how the player gets into `PlayerWhoUsedMe` (extends NATIVE_BEHAVIOR_POPULATION G2)

This is what makes the resolver return something. Read from the runner and its input-binding helper. UNVERIFIED.

1. **Input links of a behavior are bound before it runs**, one per link whose type is Input (2) and whose property name is
   set. The runner looks the property up by name on the behavior's class and branches by the property's type. For an
   object, vector and similar properties it writes the variable's value straight in. **For a struct property whose type
   name is `BehaviorContextData` (compared case-insensitively)** it does this:
   - resolves the linked variables to a list of objects (a named-variable reference resolves as in NATIVE_MARCUS_USE_CHAIN
     "Event payload to named variable"; an object variable contributes its object when it passes the expected class
     check, which for `ContextObject` is plain `Object`);
   - writes the **first** object into the struct's `ContextObject` (None when the list is empty);
   - **sets the struct's `BehaviorContext` byte to 4 (UseContextObject)**, even when the list is empty;
   - remembers the `ContextObject` slot, and **resets it to None when the behavior's run finishes** (the cleanup runs when
     the behavior is not latent, or when it has just finished; the selector byte stays 4).
   (NATIVE_BEHAVIOR_POPULATION G2 said the `EBehaviorContext` enum is consumed only inside the behavior; the runner in
   fact also *writes* the value 4 here. See Corrections.)
2. **What a thread run passes to `ApplyBehaviorToContext`:** the argument list is (ContextObject, KernelInfo, SelfObject,
   MyInstigatorObject, OtherEventParticipantObject, EventData). The runner supplies the context object of the current
   iteration, the call-information struct (NATIVE_BEHAVIOR_POPULATION G2 item 3), **`SelfObject` = the consumer's own
   object (Marcus's pawn)**, and **`MyInstigatorObject`, `OtherEventParticipantObject` both None and an empty
   `EventData`**. So in a kernel-run behavior the Self selector (0) gives the consumer, and selectors 1, 2 and 3 give
   nothing; only selector 4 (written by the input binding) gives the event's player. Medium confidence on the last three
   being always empty here; the call goes through one wrapper whose only caller is the runner.
3. **Empty context list (NATIVE_MARCUS_USE_CHAIN open question 1, answered):** a behavior with a Context-type link that
   resolves to no object, and with no `Context.ContextObject` set in its own data, is **not run at all** for that pass: the
   per-context loop executes zero times, no output link is recorded, and the thread treats the behavior as finished (no
   latent wait). If the behavior declares `bSupportsDefaultOutputLink`, the **default link (-1) is still selected** after
   the empty pass, so its default-linked successor runs and the thread continues; otherwise the thread ends there. A
   behavior with **no** Context link and no `Context.ContextObject` runs once with the consumer's own object as context.
   A behavior with `Context.ContextObject` set (and no Context link) runs once with that object. Medium-high.
- **Implementer checklist:** (a) when binding input links, treat a property of struct type `BehaviorContextData` as above:
  first resolved object into `ContextObject`, selector := 4, reset `ContextObject` after the run; (b) pass `SelfObject` =
  consumer object, the two participants None and an empty `EventData` into `ApplyBehaviorToContext` for kernel-run
  behaviors; (c) an empty context list skips the behavior, then selects the default link when the behavior supports one.
- **Open:** several resolved objects for a `BehaviorContextData` property (only the first is stored; the array-property
  form of the binding was not read); the latent copy's stored contexts; the exact point at which the slot reset happens
  for latent behaviors.

## Behavior_PlayAIMissionContextDialog and WillowAIPawn.PlayOnUseDialog (script)

Both are script, run on the VM. Instances: Marcus's provider has `Behavior_PlayAIMissionContextDialog_37` in sequence
`Brain`, with **no property overrides** (all defaults) and the variable links described in NATIVE_MARCUS_USE_CHAIN: its
`PlayerWhoUsedMe` property is an **Input** link (type 2) to the named reference, which resolves to the object variable
`PlayerWhoUsedMe` that the `OnUsed` event filled with the instigator pawn. The behavior has no Context link and its own
`Context.ContextObject` is unset, so it runs **once, with Marcus's pawn as context object**; its
`bSupportsDefaultOutputLink` is true, so the default link to `Behavior_HasMissions` is always taken afterwards.

**`Behavior_PlayAIMissionContextDialog.ApplyBehaviorToContext(ContextObject, ...)` does, in order (UNVERIFIED as a game rule,
read directly from script):**
1. `user` := `GetBehaviorContext(PlayerWhoUsedMe, SelfObject, MyInstigatorObject, OtherEventParticipantObject, EventData)`.
   With the runner fill above this is the **player pawn** (the instigator of `OnUsed`).
2. `userPawn` := `user` cast to `Pawn`. If that cast is `None` **and** `user` is a Controller, `user` is replaced by that
   controller's `Pawn`, but `userPawn` is **not** refreshed: the stock script would then pass `None` on. (A quirk of the
   script; irrelevant on our route because the payload instigator is already a pawn.)
3. `aiPawn` := `ContextObject` cast to `WillowAIPawn`; if that is `None` and `ContextObject` is a Controller, the
   controller's `Pawn` cast to `WillowAIPawn`.
4. If `aiPawn` is set: `aiPawn.PlayOnUseDialog(userPawn)`. Nothing else happens. The behavior never calls an output-link
   native; the default link comes from the runner.
- **If `GetBehaviorContext` returns `None`** (stub, empty variable): `userPawn` is `None`, `PlayOnUseDialog(None)` returns at
  its first guard (below), the dialog is silent, and the chain still continues to `Behavior_HasMissions` through the
  default link. This is exactly the behaviour seen with the stub.

**`WillowAIPawn.PlayOnUseDialog(PlayerEnteringMenu)` decision rules (script):**
1. `pc` := `PlayerEnteringMenu.Controller` cast to `WillowPlayerController`.
2. **Silent return** when any of: `pc` is None; the pawn's `MyWillowMind` is None; `MyWillowMind.AIClass` is None;
   `AIClass.AIDef` is None; `PawnsUsingMe` has **more than one** entry (so one user is the supported case; zero entries
   does not stop it).
3. `CountMyMissionsByState(pc, eligible, inProgress, redeemable)`: script that calls the director's
   `GetEligibleMissions`, `GetInProgressMissions` and `GetRedeemableMissions` on the pawn itself, one shared array, only
   the returned counts used (natives and counts: NATIVE_USE_INTERACTION and NATIVE_MISSION_DISPATCH).
4. **First matching branch wins**, in this order: redeemable above 0 gives tag `DET_OnUse_MissionComplete`; else eligible
   above 0 gives `DET_OnUse_MissionsAvailable`; else in-progress above 0 gives `DET_OnUse_AllMissionsInProgress`; else
   `DET_OnUse_NoMissions`. The tags are fields of the global `WillowDialogGlobalsDefinition` (`GD_Globals.Dialog.DialogGlobals`,
   in `Startup`), fetched through its static `Get`.
5. `DialogComponent.TriggerEvent(tag, PlayerEnteringMenu)` on the pawn's own `GearboxDialogComponent` (native, NATIVE_DIALOG
   "Component TriggerEvent": instigator becomes the component's owner, Marcus; the other-object argument is the player).
   The result is discarded.
- **There is no "first time talked" flag, no context-dialog data on the AI definition and no per-session gate in this
  script.** The only inputs are the five guards, the three counts, and the tag data below. (Cross-checked: the four tags
  and the three `DET_NPC_OnUse_*` tags all have `bOncePerSession` and `bMultiplayerOnly` false.)

**The tag data behind the four tags (read from `Startup`, UNVERIFIED chain):**
- The four global tags are `GD_Dialog_NPC.Events.VO_NPC_OnUse_MissionComplete`, `..._MissionsAvailable`,
  `..._AllMissionsInProgress`, `..._NoMissions`. Priorities: MissionComplete has none; the other three `DialogPriority_40`.
- They are listed by the generic group `GD_Dialog_NPC.Groups.DialogGroup_NPC`, whose name tags include
  `DialogName_Marcus`. Its events for the three barking tags point to Talk acts whose speaker table has **no entry for
  Marcus** and whose `bInstigatorTalker` and `bEnableNoMatch` are set; per NATIVE_DIALOG (act chooses a talker, no valid
  talker plus `bEnableNoMatch` gives output 1) the **no-match output (1)** runs. That output links to a Trigger act that fires
  the implementation tag `GD_Dialog_NPCImplementation.Events_MissionGiver.DET_NPC_OnUse_AllMissionsInProgress` /
  `..._MissionsAvailable` (the NoMissions act was not followed; assumed the same). Marcus's own group
  `GD_Dialog_NPCImplementation.Groups.DialogGroup_NPC_Marcus` (name tag `DialogName_Marcus`) holds events for exactly those
  `DET_NPC_OnUse_*` tags (priority `DialogPriority_20`) and for `DET_NPC_PlayerLingeringInMenu`, each with a Talk act
  (barks listed in SLICE_AUDIO_CHAIN). So the on-use bark is a **two-step dispatch**: global tag, generic group no-match,
  Trigger act, implementation tag, Marcus's group. A flat "find the tag in Marcus's group" lookup would find nothing,
  because the tags are different objects.
- `VO_NPC_OnUse_MissionComplete` has an event in the generic group with **no output action**, and Marcus's group has no
  tag for it. Whether SLICE_AUDIO_CHAIN's "events with a null output action pair in order with the group's inline talk
  acts" rule gives it a line was not checked; treat the MissionComplete branch as possibly silent.
- **Natives this path reaches:** `GearboxDialogComponent.TriggerEvent` and
  what the talk and trigger acts call (NATIVE_DIALOG), the mission-director list functions (NATIVE_USE_INTERACTION), the
  Core array natives, `Object.IsA` and the casts. `GetBehaviorContext` is the only native newly needed.
- **Implementer checklist:** (a) fill `PlayerWhoUsedMe` as described, run `GetBehaviorContext`; (b) apply the five guards
  of step 2; (c) pick the tag by the redeemable / eligible / in-progress / none order; (d) call the component's
  `TriggerEvent` with the player as the other object; (e) expect the two-step group dispatch rather than a direct hit in
  Marcus's group; (f) keep taking the default link to `Behavior_HasMissions` whatever the dialog does.
- **Open:** the Talk act that serves `DET_OnUse_NoMissions` (assumed symmetric); the null-output-action pairing for
  MissionComplete; Marcus's `MyWillowMind.AIClass.AIDef` chain needs to exist on the VM pawn or the guard in step 2 makes the
  dialog silent (a data requirement, not read for the VM pawn here).

## WillowPawn.GetBehaviorConsumerHandle (IBehaviorConsumer)

- **Signature:** `native function BehaviorConsumerHandle GetBehaviorConsumerHandle()`; `IBehaviorConsumer` declares **no other
  function** (its other members are the handle struct and two replication structs), so there are no further natives of the
  interface on `WillowPawn`.
- **Reads:** the pawn's own `ConsumerHandle` field (class layout offset 3588 computed, 3592 observed in the code, the
  same one-word shift NATIVE_BEHAVIOR_POPULATION noted). The call goes through the pawn's `IBehaviorConsumer` dispatch
  table (slot 2), whose implementation for the pawn class copies that one field out and returns it.
- **Does:** a **plain field read**, no kernel call, no lookup, no registration, no logging. The result is the pawn's current
  handle value whatever it is, including the invalid one.
- **When the field is assigned:**
  1. The pawn's constructor sets it to **-1 (all bits set)**: this is the invalid value.
  2. `WillowPawn.InitializeBehaviorProviders` (native; the AI-pawn version runs it only when the pawn has a controller or
     is not on the authority role, per NATIVE_BEHAVIOR_POPULATION) assigns it **once**: while the field is -1 it asks the
     kernel to register the pawn as a consumer and stores the answer; afterwards (field not -1) it does nothing more
     with the handle. Then it initializes providers for the pawn's `BodyClass` (offset 2348 in the pawn) and parts,
     the stance providers and so on (NATIVE_BEHAVIOR_POPULATION section E). For the AI pawn, where it is called from at spawn is described in
     NATIVE_BEHAVIOR_POPULATION.
  3. The kernel registration (the same routine `BehaviorKernel.RegisterBehaviorConsumer` uses) returns **-1** when there
     is no kernel, the consumer object is None, or no process slot can be allocated; otherwise a **non-negative slot
     index** of the process table. The first handle is probably 0 (the allocator was not read to prove a zero base).
- **Answer to NATIVE_MARCUS_USE_CHAIN open question 2:** **the invalid handle is -1, not 0.** The zero value a script local
  gets when `Behavior_IsSequenceEnabled` finds no `IBehaviorConsumer` is **not** invalid: it can name the first real
  consumer. The VM should therefore initialise a pawn's field to -1, assign a real value at `InitializeBehaviorProviders`,
  and compare against -1 for "unregistered".
- **Implementer checklist:** (a) `GetBehaviorConsumerHandle` returns the pawn's field unchanged, no side effects;
  (b) the field starts at -1 and is set exactly once, at consumer registration; (c) kernel entry points given -1 behave as
  in NATIVE_MARCUS_USE_CHAIN (false / nothing fires); (d) the VM's bound accessor is therefore correct as a plain field
  read, provided the field starts at -1 and is not left at 0 for an unregistered pawn.
- **Open:** the process-table allocator's base (is the first handle 0); behaviour of the same call on non-pawn consumers
  (projectiles, items, weapons, vehicles, skills have their own tables; the Perch, interactive-object and tracker ones
  share the same shape or a shared stub, not read).

## Not read yet

- The instance-data entry layout used by the named-context path; the array-property form of the `BehaviorContextData` input
  binding (more than one object); `Behavior_AddMissionDirectives`; the process-table allocator.
- The Talk act of `VO_NPC_OnUse_NoMissions` in the generic group and the null-output-action pairing for MissionComplete.
- Replication of `GetBehaviorConsumerHandle` and of the consumer state (single player ignores it).

## Corrections to earlier notes (listed, not applied)

- **NATIVE_BEHAVIOR_POPULATION G2, item 2 (last parenthesis):** "the `EBehaviorContext` enum ... is consumed inside the behavior's
  own path, not by the runner". The runner **writes** the value 4 (UseContextObject) and the first resolved object into every
  `BehaviorContextData` property that has an Input link; `GetBehaviorContext` (native, not the behavior's own code) consumes it.
  Item 1 of G2 ("each Input link is copied ... handled by type") is right and this is the struct case it did not name.
- **NATIVE_MARCUS_USE_CHAIN open questions:** question 1 (empty context list) and question 2 (invalid handle) are answered
  here (empty list: behavior skipped, default link still taken if supported; invalid handle: -1). Its edge-case remark that
  "-1 is the native's own default for an unsupplied handle" is consistent with this.
- **SANCTUARY_RPG_MISSION.md ("Needed beyond the three" and "Stubs newly reached"):** `WillowPawn.GetBehaviorConsumerHandle` now has a
  note (this file): a plain field read is right, but the field must start at -1 (invalid) and be assigned at consumer
  registration; `GetBehaviorContext` is not "BehaviorBase context resolution" in the kernel sense but the pure resolver above,
  and the stub also needs the runner's struct fill.
- **NATIVE_USE_INTERACTION.md ("Behavior_PlayAIMissionContextDialog"):** add that `PlayOnUseDialog` is also silent when the
  pawn has no `MyWillowMind`, `AIClass` or `AIClass.AIDef`, and that its tags are the `VO_NPC_OnUse_*` ones reaching Marcus
  only through the generic NPC group's no-match output and a Trigger act (two-step dispatch above).
- **SLICE_AUDIO_CHAIN.md ("Marcus's own group"):** the three `DET_NPC_OnUse_*` barks are not triggered directly by the
  on-use script; the script triggers the global `VO_NPC_OnUse_*` tags.
