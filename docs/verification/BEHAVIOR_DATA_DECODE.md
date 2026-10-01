# Behavior provider data: variables, CompareObject, sequence choice, world behaviors (2026-10-01)

AI-assisted (Claude). This records what was recovered from the installed packages to replace the
behavior-side host stand-ins of the Sanctuary slice (`docs/verification/SANCTUARY_RPG_MISSION.md`), which
structural oracle checked each piece, and what stays `UNVERIFIED`. **No original-game capture exists for
anything here**: the layouts are fitted to the packages and checked structurally; the native semantics
(BehaviorKernel, the enable-condition evaluation) are hypotheses. Generated output stays under ignored
`local/behavior/`; nothing game-derived is committed.

## 1. Where the CompareObject objects live

The previous record called the compared objects "an undecoded untagged union". What the packages show:

- All 970 `Behavior_CompareObject` exports (249 unique paths) are 12 bytes: net index + `None`. `ObjectA` /
  `ObjectB` are never set on the behavior itself.
- The behavior's properties are linked to **variables** of its sequence. `BehaviorSequenceData` carries
  `VariableData` (`BehaviorVariableData`: `Name`, `Type` = `EBehaviorVariableType`), `ConsolidatedVariableLinkData`
  (`BehaviorVariableLinkData2`: `PropertyName`, `VariableLinkType` = `EBehaviorVariableLinkType`
  {Unknown, Context, Input, Output}, `ConnectionIndex`, `LinkedVariables` packed range) and
  `ConsolidatedLinkedVariables` (variable indices). A behavior's `LinkedVariables` and an event's `OutputVariables`
  are packed ranges (`index << 16 | length`) into `ConsolidatedVariableLinkData`.
- `BehaviorVariableData` serializes only `Name` and `Type` (each element is exactly 80 bytes: two tags + `None`).
  Its `Value` (`BehaviorVariableValueUnion_Mirror.Data` is a `Core.Object.Pointer` struct) is **never written in the
  tag stream**.
- The values are an **untagged block after the provider's tagged properties**. The dummy's provider
  (`GD_TargetDummy.Character.CharClass_TargetDummy.BehaviorProviderDefinition_5`, export 5500 of
  `Sanctuary_Dynamic.upk`, 28,256 bytes) has 76 bytes after its final `None`: 19 sequence variables x 4 bytes. In
  sequence `FireDamage`, variable 1 holds import `-1871` = `GD_Incendiary.DamageType.DmgType_Incendiary_Impact`.

### Layout (FITTED, checked by exact consumption)

One entry per `VariableData` element, in sequence order then variable order, no header, sized by type name:

| Type | Bytes | What the words are (UNVERIFIED beyond the checks listed) |
|---|---|---|
| Bool, Int, Float, Object | 4 | the value; Object = package-relative object reference |
| Vector | 12 | three floats |
| DirectionVector | 48 | words 0-1 an FName (`DIRECTION_*` values observed), rest unknown |
| InstanceData | 12 | word 0 (0/1 observed), then an FName (instance-data name, e.g. `Helmet`) |
| Attribute | 20 | word 0 packed (`index << 16 \| 1` observed); words 1-4 read as `AttributeInitializationData` (BaseValueConstant, BaseValueAttribute, InitializationDefinition, BaseValueScaleConstant) |
| UnaryMath / BinaryMath | 8 / 12 | packed variable ranges + an operation code (e.g. `3000005`) |
| AttachmentLocation | 32 | word 0, then an FName, rest unknown (fitted on 59 providers only) |
| Flag | 8 | word 0, then an object reference (`FlagDefinition` observed) |
| NamedVariable, NamedKismetVariable, AllPlayers | 0 | nothing; the variable's `Name` carries the reference |

Oracles, over **every** `BehaviorProviderDefinition` / `AIBehaviorProviderDefinition` export of the 123 cooked
packages that contain one (`research/behavior_census.py`, path index from `tools/export_index.py`):

- exact consumption: 32,185 providers (9,248 unique paths), **0** whose block length differs from the sum of
  its variable sizes, **0** with an unknown type, 0 tag-walk errors;
- every non-zero Object value is an in-range reference (0 out of range); classes of the 905 non-zero object
  constants: WillowDamageTypeDefinition 465, MaterialInstanceConstant 272, Texture2D 111, WillowAnimDefinition 42,
  BodyHitRegionDefinition 8, SpecialMoveRandom 3, ItemPoolDefinition 3, AkEvent 1;
- Attribute word 2 always resolves to an attribute definition class (AttributeDefinition 1,735,
  ResourcePoolAttributeDefinition 1,160, DesignerAttributeDefinition 90, InventoryAttributeDefinition 22) and word 3 to
  `AttributeInitializationDefinition` (209); Flag word 1 to `FlagDefinition` (94 of 94);
- 0 non-finite Float values, 0 Bool values other than 0/1;
- copies of one provider path in different packages decode to the same values once package-relative references and
  names are resolved: **0** inconsistent (this check first flagged package-relative names and references in DirectionVector, Attribute, AttachmentLocation and Flag values, which is how those words were identified);
- independent cross-check on the dummy: `Behavior_IntMath_2.B` and `_3.B` are linked to Int variables whose decoded
  words are 9 and 4, equal to the behaviors' own tagged `B` (9 and 4);
- the type sizes were fitted on `Sanctuary_Dynamic` (462 providers, then 0 mismatches) and the two rare types
  (AttachmentLocation, Flag) on their 59 unique providers (59/59); the rest of the corpus is the check.

### The dummy's CompareObject inputs

`--behavior-dump` on the dummy provider (all from data):

| Sequence | Condition (`BehaviorSequenceEnableByMission`) | `ObjectA` | `ObjectB` (constant) |
|---|---|---|---|
| FireDamage | M_RockPaperGenocide_Fire, objective `Fire` | OnTakeDamage output `DamageType` (ConnectionIndex 4) | `GD_Incendiary.DamageType.DmgType_Incendiary_Impact` |
| AmpDamage | M_RockPaperGenocide_Amp, objective `amp` | OnTakeDamage `DamageType` | `GD_Amp.DamageType.DmgType_Amp_Impact` |
| Slagged | M_RockPaperGenocide_Amp, objective `KillCompetitor` | OnTakeDamage `DamageType` | `GD_Amp.DamageType.DmgType_Amp_Impact` |

The sibling pawns agree: `GD_TargetDummy_Shield` FireDamage/ShockDamage/CorrosiveDamage/AmpDamage/Slagged compare
against Incendiary/Shock/Corrosive/Amp/Amp `_Impact`, `GD_TargetDummyBot` CorrosiveDamage against Corrosive. Over the
whole census (249 unique CompareObject behaviors, sources listed in ignored `local/behavior/`): ObjectA comes from an
event output 168 times, a named variable 81, another behavior's output 1, and is unwritten (None) 72 times; ObjectB is a
constant from the value block 111 times, a named variable 28, unwritten 91. Unwritten inputs read None, so those
behaviors test "is None"; that reading is UNVERIFIED.

### What CompareObject does with them

`Behavior_CompareObject.ApplyBehaviorToContext` **has script** (installed bytecode, read with `--disasm`):
`if (ObjectA == ObjectB)` (Core native 114, `Object.EqualEqual_ObjectObject`) then
`BehaviorKernel.ActivateBehaviorOutputLink(KernelInfo, 0)` else `(..., 1)`; the class enum is
`ECompareObjectOutputLinkIds {OUTPUT_Same, OUTPUT_Different}`. `BehaviorBase.LINK_ID_RESERVED_FOR_DEFAULT_BEHAVIOR_OUTPUT`
is the const `-1` (byte 255). So the high byte of `LinkIdAndLinkedBehavior` is the output link id the behavior activates.
How the native kernel follows those links (only the activated id? default 255 after any behavior?) is `UNVERIFIED`.

The executor now runs CompareObject from data (`BehaviorProvider` built-in handler): incendiary damage on the dummy
follows `OUTPUT_Same` (AttemptStatusEffect `Status_Incendiary`, AIHold, ChangeRemoteBehaviorSequenceState,
UpdateMissionObjective `Fire`); shock or None damage follows `OUTPUT_Different` (no links in FireDamage).

## 2. How the Fire variant picks `FireDamage`

Each damage sequence has a `CustomEnableCondition` of class `WillowGame.BehaviorSequenceEnableByMission`
(`LinkedMission`, `bIsObjectiveSpecific`, `LinkedObjective`, `MissionStatesToLinkTo`, `ObjectiveStatesToLinkTo`,
`ObjectiveSetRestrictions`; class defaults: mission states {bActive}, objective states {bActive}). For the dummy:
FireDamage <- Fire mission objective `Fire` active; AmpDamage <- Amp mission objective `amp`; Slagged <- Amp mission
objective `KillCompetitor`; TargetDummy <- Amp mission Complete. The class's `MissionReaction*` functions have **no
script**: the evaluation is native. `FireMissionSlice` now evaluates these conditions after every mission change with
this rule (`UNVERIFIED`): mission state in `MissionStatesToLinkTo` and, when objective-specific, the objective state
(NotStarted / Active = in the active set and not complete / Complete) in `ObjectiveStatesToLinkTo`; a mission other
than the slice's is Complete if in the completed set, else NotStarted; non-empty `ObjectiveSetRestrictions` is
reported as unsupported (none on this provider).

The "instance-data switch" of the earlier record is something else: the `Default` sequence's `OnSpawned` runs native
`Behavior_IntMath` (`BINARYMATH_Rand`, B = 9 and 4) into `Behavior_ChangeInstanceDataSwitch` (`RatHead`, `RatMasks`),
whose script calls `IBodyCompositionInstance.ChangeInstanceDataSwitch`: a cosmetic body-composition choice. Not the
damage-sequence choice. Where the chain goes native: the condition evaluation, IntMath, and spawning (the world record
`SLICE_WORLD_PLACEMENT.md` on the integration branch traces the population den that spawns this dummy).

Also recovered while doing this: `Behavior_ChangeRemoteBehaviorSequenceState.Action` (`ITargetable.EChangeStatus`
{Toggle, Enable, Disable}, class default Enable). Two of the dummy's behaviors **disable** (`ObjectiveComplete`
disables itself after `RocksPaper_SendTargetBack`; `ResetTarget` likewise); the old handler always enabled.

## 3. World behaviors the slice reaches (decoded fields, host boundary)

Both have script; they act on the context object, so they stay at the host boundary with structured fields
(`BehaviorProvider::boundaryCalls`):

- `Behavior_Transform` (`Transform` = `AIPawnBalanceDefinition.EAITransformed`): sets `WillowAIPawn.TransformType`
  on the context's `IBodyPawn`. No amount, no duration. FireDamage `Behavior_Transform_12` = `EAIT_Fire` (on
  `OnBehaviorSequenceEnabled`), AmpDamage/Slagged `_6`/`_7` = `EAIT_Slagged`. Context `BCONTEXT_Self`.
- `Behavior_RegisterTargetable` (`bUnregister`): calls `ITargetable.Behavior_RegisterTargetable(bUnregister)` on the
  context. `Targetable` enables register (`_36`, false) / disable unregister (`_35`, true); Idle `_77`/`_78` on the
  slag custom events.
- Also reported on `OnSpawned`: `Behavior_IntMath` (Operation, A, B) and `Behavior_ChangeInstanceDataSwitch`
  (SwitchName, NewValue source).

## 4. Link-id census (goal 4; all conclusions UNVERIFIED)

Over the 9,248 unique providers: 238 behavior classes have outgoing links; 209 only ever use id 255, 29 use small ids.
Selected classes (instances; id counts; most frequent id sets):

| Class | Instances | Ids seen |
|---|---|---|
| Behavior_CompareObject | 249 | 0: 211, 1: 210 (sets {0,1} 160, {1} 48, {0} 40) |
| Behavior_CompareBool | 560 | 0: 418, 1: 344 |
| Behavior_CompareFloat / CompareInt | 792 / 192 | 0, 1, 2 |
| Behavior_IsSequenceEnabled | 227 | 0, 1 |
| Behavior_Conditional | 456 | 1-6 |
| Behavior_RandomBranch | 321 | 0-21 |
| Behavior_IntSwitchRange / DamageSourceSwitch / Switch | 149 / 47 / 28 | up to 17 / 15 / 5 |
| Behavior_Metronome, InterpolateFloatOverTime | 91, 33 | 1 and 255 |

CompareObject never uses an id other than 0/1; compare-like behaviors use 0..2; switch-like behaviors use as many
ids as they have cases; 209 classes only use 255. This is consistent with "id = output link" but proves nothing about
the kernel. **Event links differ:** their id byte is mostly 0 (13,601 of 17,271; then 1: 1,306, 2: 1,081, 4: 413, 3: 315), with values up to 33, so for links that
leave an event the byte is not a behavior output id; its meaning is unknown (`UNVERIFIED`).

Non-zero event-link ids concentrate on mission-style targets (MissionRemoteEvent 1/2/4, TriggerDialogEvent 2/12/13,
AdvanceObjectiveSet 1/2, ChangeRemoteBehaviorSequenceState 1/2/4).

Duplicate links (same source, same target): 853 of 17,271 event links and 399 of 24,594 behavior links, and **in
every one of them the duplicates carry different id bytes** (0 duplicates with the same id). The executor's current
rule "a behavior runs at most once per fired event" therefore collapses links that the data distinguishes. **This is
reached on the slice route**: the Fire mission's `Default` event links `Behavior_MissionRemoteEvent_319` twice (ids 3
and 9) and `Behavior_TriggerDialogEvent_1185` twice (ids 12 and 13); the executor runs each once. Whether the game runs
them once or twice (or whether the id selects something else, e.g. a target input) is unknown; this needs the game
trace. The dummy's provider has no duplicate links. Not changed in this pass.

## Code and API (this branch)

- `src/vm.cpp`: enum bytes whose enum is declared in another package now resolve through the declaring
  `ByteProperty` (the tag's enum name must match the referenced Enum export); before, they read 0. Struct values now
  start from the struct's own default tags (ScriptStruct: 52-byte header with word +44 = 0, StructFlags, then tags;
  FITTED, 1,275 of 1,275 structs of Core/Engine/GameFramework/GearboxFramework/WillowGame end exactly at the export
  end, `research/struct_defaults_census.py`), applied only when that stream is consumed exactly.
- `src/behavior.*`: variables, variable links, value block (exact consumption or nothing), `objectInput`/`intInput`,
  built-in CompareObject, `enableCondition`, structured `boundaryCalls`, `fireEvent(event, outputs)`.
- `src/slice.*`: `damageDummy(damageTypePath, damageSourcePath)`, `spawnDummy()`, enable conditions, `Action`.
- `src/mission.*`: `objectiveState(path)`, SetSequence effects carry the action.
- CLI: `--behavior-dump`, `--behavior-run`, `--slice-run` steps `damage:<path>` and `spawn`.
- Tests: `tests/behavior_test.py` (synthetic, CTest `behavior-synthetic`).

## Reproduce

```powershell
$G = "$env:OPENWILLOW_BL2\WillowGame\CookedPCConsole"
$P = "GD_TargetDummy.Character.CharClass_TargetDummy.BehaviorProviderDefinition_5"
build\Release\ow-package.exe "$G\Sanctuary_Dynamic.upk" --behavior-dump $P --cooked $G
build\Release\ow-package.exe "$G\Sanctuary_Dynamic.upk" --behavior-run $P --cooked $G enable:FireDamage "event:OnTakeDamage:DamageType=GD_Incendiary.DamageType.DmgType_Incendiary_Impact"
build\Release\ow-package.exe "$G\Sanctuary_Dynamic.upk" --slice-run GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire --cooked $G accept tick:5 range spawn tick:6 "damage:GD_Shock.DamageType.DmgType_Shock_Impact" "damage:GD_Incendiary.DamageType.DmgType_Incendiary_Impact" tick:5 turnin
python research/behavior_census.py          # ~8 min, writes local/behavior/behavior_census.json
python research/struct_defaults_census.py
```
