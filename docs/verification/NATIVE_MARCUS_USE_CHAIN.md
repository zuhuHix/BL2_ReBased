# Native Marcus use chain: `Behavior_IsSequenceEnabled` and the OnUsed provider chain (2026-10-06)

AI-assisted (Claude), analyst lane G18. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script behaviour was read with `research/script_disasm.py`
(GearboxFramework and WillowGame); data values were read with `ow-package --properties`, `--object-dump` and the provider
graph of `GD_Marcus.Character.AIDef_Marcus.AIBehaviorProviderDefinition_0` in `Sanctuary_Dynamic`. Related notes (reused,
not repeated): [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) ("Marcus's chain", `AIDefinition.OnUsed`),
[NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) (enable state, mission conditions, context objects),
[NATIVE_MISSION_DISPATCH.md](NATIVE_MISSION_DISPATCH.md) (event firing, output links).

## Summary

The main finding is a correction: **`Behavior_IsSequenceEnabled` is not native.** Its `ApplyBehaviorToContext` is script in
GearboxFramework. The same holds for `Behavior_HasMissions`, `Behavior_PlayAIMissionContextDialog`,
`Behavior_ShowMissionInterface` and `Behavior_RemoteCustomEvent` (the first three are WillowGame script, already
described in NATIVE_USE_INTERACTION). The VM runs all of them; what is native is what they call. So the open question
"what does the check read" is answered by one native, `BehaviorKernel.IsBehaviorSequenceEnabled`, plus the path resolver
`BehaviorHelpers.ResolveBehaviorProviderDefinitionReference`.

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| BehaviorKernel.IsBehaviorSequenceEnabled | `native static final function bool IsBehaviorSequenceEnabled(BehaviorConsumerHandle ConsumerHandle, BehaviorProviderDefinition ProviderDefinition, name BehaviorSequenceName)` | Fire: all nine checks of Marcus's chain | high | UNVERIFIED |
| BehaviorHelpers.ResolveBehaviorProviderDefinitionReference | `native static function BehaviorProviderDefinition ResolveBehaviorProviderDefinitionReference(Behavior SourceBehavior, BehaviorProviderDefinition ProviderReference, NameBasedObjectPath PathName)` | Fire: turns the nine check instances and the nine remote events into Marcus's provider | medium-high (precedence), medium (reference branch) | UNVERIFIED |
| BehaviorKernel.ActivateBehaviorEventFromScript (details added) | `native function ActivateBehaviorEventFromScript(BehaviorConsumerHandle ConsumerHandle, BehaviorProviderDefinition ProviderDefinition, name EventName, optional int EventOutputToActivate, optional array<...> Parameters)` | Fire: the remote custom events | high | UNVERIFIED |
| Behavior_AddMissionDirectives.ApplyBehaviorToContext | native virtual | none on the Fire route | not read | UNVERIFIED |

Parameter order and optionality were taken from the package declarations (parameters are stored in reverse order); the
`static` and `final` flags were not checked and do not matter for a port.

## What the whole chain does, in one page (UNVERIFIED)

1. `AIDefinition.OnUsed` (and the class twin) fire the event `OnUsed` with link filter 2 (Generic) and a two-object
   payload (instigator, used component) on Marcus's own AI-definition provider only (NATIVE_USE_INTERACTION). In the
   provider's sequence `Brain` the event `OnUsed` has **one** output link, id 2, to the first check; its payload
   binding is described in "Event payload to named variable" below.
2. The nine checks form a **cascade**, not a fan-out. Each check has exactly two output links: id 0 (enabled) leads to a
   remote custom event, id 1 (not enabled) leads to the next check. After the ninth, link 1 leads to
   `Behavior_PlayAIMissionContextDialog` -> (default link) `Behavior_HasMissions` -> (link 0) `Behavior_ShowMissionInterface`.
3. A remote custom event has no outgoing link, so the cascade stops at the first enabled sequence. The event it fires
   runs inside the same call (depth first), before anything else of the cascade could continue.

## Behavior_IsSequenceEnabled (script) with BehaviorKernel.IsBehaviorSequenceEnabled (native)

- **Properties of the behavior (from the package):** `SequenceName` (name), `SequenceProvider` (a
  `BehaviorProviderDefinition` object reference) and `ProviderDefinitionPathName` (`NameBasedObjectPath`: a fixed array of six
  name slots plus a one-byte `IsSubobjectMask`). Output ids come from the enum `Behavior_IsSequenceEnabled.EIsSequenceOutputLinkIds`:
  **0 = OUTPUT_SequenceIsEnabled, 1 = OUTPUT_SequenceIsDisabled.** The behavior's `Context.bSupportsDefaultOutputLink` is
  **false** (read with `--object-dump --all`), so the default link (-1) is never selected; only 0 or 1 or nothing.
- **Is it latent?** No. It selects its output and returns in the same call.
- **What the script does (outcomes, in order):**
  1. Nothing at all (no output selected, the thread ends) when `SequenceName` is `None`.
  2. Asks the context object for its `IBehaviorConsumer` interface; if it has one, takes its consumer handle. If it does not,
     the handle stays at the struct's zero value (see Edge cases).
  3. Resolves the provider with `BehaviorHelpers.ResolveBehaviorProviderDefinitionReference(Self, SequenceProvider,
     ProviderDefinitionPathName)`. If the result is `None`: nothing selected, thread ends.
  4. Calls `BehaviorKernel.IsBehaviorSequenceEnabled(handle, provider, SequenceName)` and selects output **0 if true, 1 if
     false**.
- **What the native reads:** the kernel's process record for the consumer handle; the consumer's list of registered
  provider/sequence state records (the same records NATIVE_BEHAVIOR_POPULATION section C creates, one per sequence of each
  provider registered on that consumer); the per-sequence **enabled bit** that `ChangeBehaviorSequenceActivationStatus` and
  `IntializeBehaviorProviderForConsumer` set. It looks the sequence up **by name inside the given provider only**, on the **consumer
  given by the handle** (the context object's own behavior state, not a global table). It reads state only: no event fires, no
  trigger counters change.
- **Result rule:** true exactly when that consumer has the given provider registered, the provider has a sequence with that
  name, and its enabled bit is set. False in every other case, without any error or log line:
  no kernel, `None` provider (`None` does **not** mean "any provider" here, unlike the event-firing path of NATIVE_MISSION_DISPATCH A1),
  invalid or unregistered handle, a consumer whose process is not live, provider not registered on that consumer, or **a sequence name
  that does not exist in that provider**. A sequence that exists but is disabled is also just false (outputs 1 in the script).
  The provider is matched by object identity (the resolver returns the one provider object that the kernel registered).
- **Edge cases:**
  - Three of Marcus's nine names are for sequences that **do not exist** in his provider: `Ep4_SpeakToMarcusAboutBank`,
    `Ep4_GetMarcusCrystal` and `Ep14_Rescued` (the provider's sequences are `AI`, `Brain`, `M_TheBane`, `M_BearerBadNews`,
    `M_OutOfBody`, `M_ClaptrapBirthdayBash`, `M_BearerBadNewsReaction`, `M_OutOfBody_TurnIn`, `M_SafeAndSound_BringPictures`,
    `M_SafeAndSound_TurnIn`, `Ep8_SkyisFallingMarcus`, `Patrol`, `Ep17_TalkToMarcus`). Those three checks are always "not enabled",
    so they just pass through to the next check; the remote events they would fire (`Ep4_SpeakToMarcusCrystal`,
    `Ep4_GetMarcusCrystal`, `CE_Ep14_Rescued`) have no listener in this provider either.
  - If the context object is not an `IBehaviorConsumer`, the handle is the zero value, not an invalid one. Handle 0 could in principle
    name a real consumer. Marcus's pawn is a consumer, so this does not arise on our route. UNVERIFIED which value counts as invalid (-1 is
    the native's own default for an unsupplied handle).
  - The checks have **no variable links** in Marcus's `Brain` (the provider's variable-link table has no entry for them), so the behavior's
    context is the single default context: the consumer's own object, i.e. Marcus's pawn.
- **Implementer checklist:** (a) return false for a `None` provider or an unregistered or non-live consumer; (b) look up by provider
  object **and** sequence name; unknown name gives false; (c) the answer is the sequence's current enabled flag, nothing else; (d) the
  script behavior selects 0 for true, 1 for false, selects nothing for an empty `SequenceName` or an unresolved provider; (e) never
  select the default link.
- **Open:** the exact layout of the consumer's record list (only the outcome matters); whether the process "live" flag equals the "running"
  state of NATIVE_BEHAVIOR_POPULATION G1 (assumed so).

### The nine instances (data, `GD_Marcus.Character.AIDef_Marcus.AIBehaviorProviderDefinition_0`, sequence `Brain`)

All nine have `ProviderDefinitionPathName` = `GD_Marcus.Character.AIDef_Marcus.AIBehaviorProviderDefinition_0` (name slots 2 to 5, slots 0
and 1 empty, `IsSubobjectMask` = 16) and `SequenceProvider` unset, i.e. each names **Marcus's own provider**. Cascade order, starting from
the target of the event's link; "0" is the enabled output, "1" the not-enabled output; names are the instance names.

| # | Instance | `SequenceName` | Sequence in provider? | Output 0 (enabled) fires remote event | Output 1 goes to |
|---|---|---|---|---|---|
| 1 | `Behavior_IsSequenceEnabled_197` | `Ep4_SpeakToMarcusAboutBank` | no | `Ep4_SpeakToMarcusCrystal` (`Behavior_RemoteCustomEvent_184`) | #2 |
| 2 | `..._191` | `Ep4_GetMarcusCrystal` | no | `Ep4_GetMarcusCrystal` (`..._183`) | #3 |
| 3 | `..._196` | `Ep14_Rescued` | no | `CE_Ep14_Rescued` (`..._185`) | #4 |
| 4 | `..._194` | `M_TheBane` | yes | `M_TheBane_TalkToMarcus` (`..._186`) | #5 |
| 5 | `..._193` | `M_BearerBadNews` | yes | `M_BearerBadNews_TalkToMarcus` (`..._188`) | #6 |
| 6 | `..._195` | `M_ClaptrapBirthdayBash` | yes | `ClapTrapBirthday_TalkToMarcus` (`..._182`) | #7 |
| 7 | `..._192` | `M_OutOfBody` | yes | `M_OutOfBody_TurnInMarcus` (`..._187`) | #8 |
| 8 | `..._199` | `M_SafeAndSound_BringPictures` | yes | `M_SafeAndSound_BringPictures` (`..._190`) | #9 |
| 9 | `..._198` | `Ep17_TalkToMarcus` | yes | `Ep17_Use` (`..._189`) | `Behavior_PlayAIMissionContextDialog_37` |

(This confirms the order in NATIVE_USE_INTERACTION. The event named by an enabled check is the same on every row except the
Ep4/Ep14 and `Ep17` rows, whose names differ from the sequence name.)

Each remote event is fired on this same provider. In the provider the matching events are, per sequence:
`M_TheBane` has `M_TheBane_TalkToMarcus` (output id 0 -> `Behavior_UpdateMissionObjective_1`); `M_BearerBadNews` has
`M_BearerBadNews_TalkToMarcus` (id 0 -> a second remote custom event `BearerBadNews_ReactionMarcus` -> `Behavior_UpdateMissionObjective_26`);
`M_OutOfBody` has `M_OutOfBody_TurnInMarcus` (id 0 -> `Behavior_UpdateMissionObjective_3`); `M_ClaptrapBirthdayBash` has
`ClapTrapBirthday_TalkToMarcus` (id 0 -> `Behavior_UpdateMissionObjective_4`); `M_SafeAndSound_BringPictures` has an event of the same
name (id 0 -> `Behavior_UpdateMissionObjective_5`); `Ep17_TalkToMarcus` has `Ep17_Use` (id 0 -> `Behavior_AIHold_20` and
`Behavior_ChangeUsability_24`, the latter chaining remote sequence-state changes and dialog). `M_OutOfBody_TurnIn` and
`M_SafeAndSound_TurnIn` are different sequences that react to their own `OnBehaviorSequenceEnabled` by calling
`Behavior_ShowMissionInterface` (instances `_1` and `_2`); they are not reached from the cascade.

**Why the cascade ends at the default for the Fire mission (UNVERIFIED, from data):** the tested sequences are enabled by
`BehaviorSequenceEnableByMission` conditions on other missions only. Read from the package: `M_TheBane` (mission `GD_Z3_Bane.M_Bane`,
objective `AskMarcusAboutBane`), `M_BearerBadNews` (`GD_Z1_BearerBadNews.M_BearerBadNews`, objective `TalkMarcus`), `M_ClaptrapBirthdayBash`
(`GD_Z2_ClaptrapBirthdayBash.M_ClaptrapBirthdayBash`, objective `ClapTrapBirthday_MarcusInv`), `M_OutOfBody` (`GD_Z3_OutOfBody.M_OutOfBody`,
objective `TurnInMarcus`), `M_SafeAndSound_BringPictures` (`GD_Z2_SafeAndSound.M_SafeAndSound`, objective `TurnInMarcus`), `Ep17_TalkToMarcus`
(`GD_Episode17.M_Ep17_KillJack`, objective `TalkToMarcus`). All are objective-specific with the default objective state Active, so they
are enabled only while that objective is active in an Active mission. None of these missions is the Fire mission, so in a Fire-only
session every check answers "not enabled". (Where these conditions are applied is NATIVE_BEHAVIOR_POPULATION sections B and C.)

## BehaviorHelpers.ResolveBehaviorProviderDefinitionReference

- **Signature:** `(Behavior SourceBehavior, BehaviorProviderDefinition ProviderReference, NameBasedObjectPath PathName)`, returns a
  `BehaviorProviderDefinition` or None. Used by `Behavior_IsSequenceEnabled` and `Behavior_RemoteCustomEvent` (the script passes `Self`
  as the source behavior). The three arguments are read in declaration order in the native.
- **Reads:** the path name's six name slots and the subobject mask; the source behavior's `Outer`; the reference object.
- **Does (precedence, UNVERIFIED):**
  1. If the path name is **non-empty** (at least one name slot set): builds one dotted path string from the set slots in slot order and looks
     for an object with that full path among the loaded objects; the object must be a `BehaviorProviderDefinition` (otherwise None). Where
     the mask bit of a slot is set, the separator written before that slot is a different character (the subobject separator); the bit for
     slot k is bit k-1. For all eighteen Marcus instances this builds the path of his AI-definition provider.
  2. Otherwise, if `ProviderReference` is set: the provider that reference stands for (the reference is read through a GearboxFramework
     interface and asked for its provider; for a plain provider object this is the object itself; not read further).
  3. Otherwise, if `SourceBehavior` is set: its `Outer` when that is a `BehaviorProviderDefinition`, else None. (So a check with no path and no
     reference means "the provider this behavior sits in".)
  4. Otherwise None.
- **Edge cases:** an unknown path gives None (the check behavior ends silently, the remote event does nothing). The path wins over the reference
  when both are set.
- **Implementer checklist:** for Marcus, resolve the path to his AI-definition provider (the same provider object the kernel registered for
  his consumer), or, equivalently for this data, use the provider of the owning behavior.
- **Open:** the reference branch; whether the mask changes more than the separator; case rules of the path match.

## Behavior_RemoteCustomEvent (script) and BehaviorKernel.ActivateBehaviorEventFromScript

- **Properties:** `ProviderDefinitionPathName` (as above), `SequenceProvider` (unset in Marcus's nine), `CustomEventName` (names above).
  `Context.bSupportsDefaultOutputLink` is **true**, but no remote-event instance has an outgoing link.
- **Script does:** resolves the provider (same resolver; None means return); asks the context object for `IBehaviorConsumer` (not a consumer
  means return); takes the handle; calls `BehaviorKernel.ActivateBehaviorEventFromScript(handle, provider, CustomEventName)` with the last
  two arguments omitted.
- **Native (added to NATIVE_MISSION_DISPATCH A1):** with the optional `EventOutputToActivate` omitted the native presets it to **-1**, i.e.
  every output link of the matching event entries is taken (the filter of A1). The payload is empty. **A `None` provider fires nothing**; the
  earlier wording "or none = any provider" in A1 does not apply to this route (see Corrections). Otherwise it is the event activation of A1
  restricted to that provider: every **enabled** sequence of that provider on the consumer that has an event entry with that name runs it
  (`bEnabled`, trigger count and re-trigger gates as in A1; no filter callback is supplied from script). Depth first: the event's whole
  downstream that is due at once runs before the calling thread continues.
- **Implementer checklist:** event name exact; provider required; sequences that are disabled or lack the event do nothing; ids of the event's
  own output links do not matter (filter -1 takes all).

## Behavior_HasMissions, Behavior_PlayAIMissionContextDialog, Behavior_ShowMissionInterface (all script; pointer plus data)

Behaviour is as in NATIVE_USE_INTERACTION; what that note did not give, from the data and listings:

- **Link ids.** `Behavior_PlayAIMissionContextDialog` and `Behavior_ShowMissionInterface` have `bSupportsDefaultOutputLink` true: the dialog behavior's
  default link goes to `Behavior_HasMissions`. `Behavior_HasMissions` has it **false** and selects output **0** when the sum of eligible, in-progress
  and redeemable mission counts is above 0, else output **1**. With a context object that is not an `IMissionDirector` it still ends
  in output 1 (counts stay 0). In Marcus's provider output 1 has no link: with no missions the thread simply ends and no screen opens.
  The three `Get*Missions` calls reuse one array; only the returned counts are summed.
- **Context objects of the two WillowGame behaviors.** Marcus's `Brain` declares four variables: `MarcusVendingProxy` (named Kismet), `PlayerWhoUsedMe` (a
  named-variable reference), `PlayerWhoUsedMe` (an object variable) and `MarcusStoreUser` (named Kismet). `Behavior_PlayAIMissionContextDialog` has its
  `PlayerWhoUsedMe` property linked as an input to the named reference, and `Behavior_ShowMissionInterface_38` has its `Context` linked to the same named
  reference. So both run with the **player who used Marcus** as context object (a pawn is turned into its controller by the behavior's script, the dialog
  behavior needs the pawn, the interface behavior the controller), and the pawn that owns the behavior (`SelfObject`, Marcus) is the second object. The dialog
  behavior's own context object is Marcus's `WillowAIPawn` (its default context).
  (`Behavior_CustomEvent_11`, the `OnSecondaryUsed` handler, is linked to the Kismet variable `MarcusVendingProxy`; it is the vending interaction and not part of
  the Fire route.)

### Event payload to named variable (what NATIVE_USE_INTERACTION left open)

- The event entry `OnUsed` lists **one** output-variable link: property name `Instigator`, link type output, connection index 0, target variable index 2
  (the object variable `PlayerWhoUsedMe`). When the event fires, the kernel walks the entry's output-variable links; a link of output type whose connection
  index is below the payload length takes the **payload element with that index**, and writes it into the target variable, if the element's type matches the
  variable's type (object element into object variable). The property name is not used for matching. `AIDefinition.OnUsed` builds the payload with two object
  elements (the instigator first, the used component second), so connection index 0 is the instigator, i.e. the player pawn. The used component is published
  nowhere in Marcus's provider. `OnSecondaryUsed` publishes its instigator into `MarcusStoreUser` the same way.
- A **named-variable reference** (variable type named variable) is resolved when a link uses it: the kernel returns the **first variable of the sequence,
  in declaration order, with the same name that is not itself a named-variable or named-Kismet-variable reference**. Here `PlayerWhoUsedMe` (the reference)
  resolves to the object variable with that name, which the event just filled. If no such variable exists the link resolves to nothing.
- **Implementer checklist:** publish the instigator into the object variable `PlayerWhoUsedMe` of `Brain` before the first link of the event is followed;
  resolve the two named-variable links by name to that variable; if the instigator is missing the context list is empty (what the runner does with an empty
  context list was not read; NATIVE_BEHAVIOR_POPULATION G2 open).
- **Confidence:** medium-high for the matching rule, medium for the exact element-type check (the type tags were read coarsely). UNVERIFIED.

## Answer to question 3: `OnUsed` event raise as it applies here

NATIVE_USE_INTERACTION already covers `AIClassDefinition.OnUsed` and `AIDefinition.OnUsed`; nothing there needs correcting. Added for this chain: (a) the event
`OnUsed` of Marcus's provider has exactly one output link and its id is **2** (Generic), so a raise with filter 2 reaches the first check while a raise with
filter 0 or 1 (HasMissions / NoMissions, fired by `UseObject` after the Generic one) **reaches nothing**: `Brain.OnUsed` has no link with id 0 or 1; (b) the
event entry has `bEnabled` true, `MaxTriggerCount` 0, `ReTriggerDelay` 0 and no `FilterObject`, so no gate rejects any press; (c) the class provider
(`BehaviorProviderDefinition_5`, only `Ep17_GiveItem`) has no `OnUsed` listener, so only the AI-definition raise does anything; (d) the payload binding is the
section above. The two later raises with filters 0/1 therefore do nothing for Marcus.

## Behavior_AddMissionDirectives (question 4)

Not read in depth. It is a registered native (virtual `ApplyBehaviorToContext`, sharing the folded exec thunk of the other native behaviors; the real body is a C++
virtual of the class). There are ten instances in the game (NATIVE_USE_INTERACTION) and none in Marcus's provider or the Fire mission; Marcus's directive table
is archetype data on the pawn. Left for a later lane.

## Not read yet

- `Behavior_AddMissionDirectives` body; the consumer's internal record layout; what the thread runner does with an empty context list; the reference
  branch of the provider resolver; whether the kernel's "process live" flag is exactly the "running" state; the `Ep8_SkyisFallingMarcus` and `Patrol` sequences.
- Replication of remote events (single player ignores it).

## Corrections to earlier notes

- **SANCTUARY_RPG_MISSION.md ("Why 6b and 6c were not started") and NATIVE_USE_INTERACTION.md ("Marcus's chain", index rows):** `Behavior_IsSequenceEnabled` is script,
  not native; its native dependency is `BehaviorKernel.IsBehaviorSequenceEnabled` (returns the sequence's enabled flag, false when the sequence does not exist);
  the data's link ids are 0 = enabled, 1 = not enabled (not "just 0 and 1").
- **NATIVE_USE_INTERACTION.md:** the `--behavior-run ... fire:2:OnUsed` stop at the first check is the executor's boundary behavior, not a property of the game; the
  cascade is described above.
- **NATIVE_MISSION_DISPATCH.md A1:** the script route `ActivateBehaviorEventFromScript` requires a provider (`None` fires nothing), and an omitted link filter means -1.
  Whether "none = any provider" holds for the tracker's own event raises was not re-read here; it does not hold for this native.
- **NATIVE_USE_INTERACTION.md "Marcus's chain":** three of the nine names (`Ep4_SpeakToMarcusAboutBank`, `Ep4_GetMarcusCrystal`, `Ep14_Rescued`) have no sequence in the
  provider, so their checks are constant "not enabled".

**Confirmed in game 2026-10-07 (lane L1):** pressing use on Marcus in Sanctuary (level-8 Maya, none of the six tested missions started) made the game call
`BehaviorKernel.IsBehaviorSequenceEnabled` nine times in exactly the order of the cascade table, each with Marcus's consumer handle (49) and his AI provider, each answering
false; then `Behavior_PlayAIMissionContextDialog`, `Behavior_HasMissions` and `Behavior_ShowMissionInterface` ran (the Fire mission offer appeared). Direct calls confirmed the result rule
(only `AI`, `Brain`, `Patrol` enabled; unknown name, `None` provider and handles -1 and 0 false). `OnUsed` ran with filter 2, then with filter 0, the second reaching no check
([REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md), "lane L1").
