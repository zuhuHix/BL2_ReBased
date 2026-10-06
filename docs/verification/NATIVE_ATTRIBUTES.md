# Native attributes: definitions, context and value resolvers, modifier stacks, effect application (2026-10-06)

AI-assisted (Claude), analyst lane G14. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script signatures come from the packages
(`tools/ghidra/class_layout.py` machinery), enum names from the package enums, and script behaviour from the local
`research/script_disasm.py` listings. Field offsets were named with `tools/ghidra/class_layout.py` and are given as
field names.

Already covered elsewhere and not repeated: the stack formula `(base + PreAdd) * (1 + up) / (1 - down) + PostAdd`
and the weapon effect order ([NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) section 1), the
`AttributeInitializationData` evaluator ([NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md) section 1), the skill effect value
formula, `Skill.AdjustModifiers` modes and the per-frame refresh ([NATIVE_SKILLS.md](NATIVE_SKILLS.md) section 4), item
equip hooks ([NATIVE_INVENTORY_EQUIP.md](NATIVE_INVENTORY_EQUIP.md)), damage attribute reads
([NATIVE_DAMAGE_DEATH.md](NATIVE_DAMAGE_DEATH.md)). This note adds what sits **underneath** those: how an attribute
definition finds its object and value, what a modifier add / remove / base-value set really does to the stored
numbers, what the resolver classes do, and the notification that follows a change. Where it confirms an earlier note it
says so; contradictions are listed at the end.

## The model in one page

An **attribute definition** (`AttributeDefinition`, `AttributeDefinitionMultiContext`, with subclasses
`ResourcePoolAttributeDefinition`, `InventoryAttributeDefinition`, `DesignerAttributeDefinition`,
`NestedAttributeDefinition`) holds two ordered chains and two flags:
- `ContextResolverChain`: turns a **context source** (any object the caller has: a controller, a pawn, a weapon, a skill,
  an item) into the **context** (the object that owns the stored number: a pool, a weapon, a pawn, a skill, a
  designer-attribute object ...).
- `ValueResolverChain`: given the context, reads or edits the number.
- `bIsSimpleAttribute` and `AttributeDataType` (float, int, bool).

The stored number lives on the context object as a **triple of properties**: the value (for example `MaxValue`), its
base (`MaxValueBaseValue`) and an array of `AttributeModifier` object references (`MaxValueModifierStack`). Each modifier
has a type (`MT_Scale` 0, `MT_PreAdd` 1, `MT_PostAdd` 2) and a float value. The value property is always the base
combined with the stack; nothing recomputes it lazily.

Three kinds of attribute property exist in the cooked classes (read from the property flags of `Engine`, `WillowGame`
and `GearboxFramework` classes; counts are of float / int / byte attribute properties found):
- **Stack attributes** (the value property of a triple; high flag bit 31): 226 float, 24 int and 2 byte properties, plus
  1 float and 4 int that also carry the notify flag. Base and stack are companions of the value.
- **Plain attributes** (high flag bit 30 only): the `...BaseValue` companions (227 float, 28 int, 2 byte), and a few plain
  fields marked the same way, for example `ResourcePool.CurrentValue` (which also carries the notify flag). No stack.
- **Notifying attributes** (high flag bit 29, on top of either): `Skill.Grade`, `WillowWeapon.ClipSize`,
  `WillowInventoryManager.WeaponReadyMax`, `WillowInteractiveObject.MaxHealth`, `WillowGameInfo.ShopTimerRate`,
  `ResourcePool.CurrentValue`.

## Summary
| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| AttributeDefinitionBase.ResolveContext | `native final function Object ResolveContext(Object ContextSource)` | every skill / item effect add, remove and read | high | UNVERIFIED |
| AttributeDefinitionBase.ResolveContexts | `native final function ResolveContexts(Object ContextSource, out array<Object> ResolvedContexts)` | multi-target effects (not slice) | medium | UNVERIFIED |
| AttributeDefinitionBase.GetValueFromContext / GetBaseValueFromContext | `native final function float GetValueFromContext(Object Context)` / `...GetBaseValueFromContext` | reading an already-resolved context | high | UNVERIFIED |
| AttributeDefinition.GetValue / GetBaseValue | `native final function float GetValue(Object ContextSource, optional out Object ResolvedContext, optional Object OptionalOverrideContextSource)` | health, shield, damage, XP attribute reads | high | UNVERIFIED |
| AttributeDefinition.StaticGetAttributeValueOrDefault | `native static final function float StaticGetAttributeValueOrDefault(AttributeDefinition Definition, Object ContextSource, float DefaultValue, optional Object OptionalOverrideContextSource)` | formula and designer reads with a fallback | high | UNVERIFIED |
| AttributeDefinitionMultiContext.GetValues / GetBaseValues | `native final function GetValues(Object ContextSource, out array<float> Values, optional out array<Object> ResolvedContexts, optional Object OptionalOverrideContextSource)` | not slice | medium | UNVERIFIED |
| AttributeDefinitionBase.AddAttributeModifier | `native final function bool AddAttributeModifier(Object Context, AttributeModifier Modifier, optional bool bSuppressNotify)` | Maya's skill effects, item and weapon effects | high | UNVERIFIED |
| AttributeDefinitionBase.RemoveAttributeModifier | `native final function bool RemoveAttributeModifier(Object Context, AttributeModifier Modifier, optional bool bSuppressNotify)` | skill refresh, unequip, respec | high | UNVERIFIED |
| AttributeDefinitionBase.SetAttributeBaseValue | `native final function bool SetAttributeBaseValue(Object ContextSource, float BaseValue, optional bool bSuppressNotify)` | `Skill.UpdateGrade`, designer attributes | high | UNVERIFIED |
| AttributeInitializationDefinition.EvaluateInitializationData / SetBaseValue | `native static final function float EvaluateInitializationData(AttributeInitializationData InitializationData, Object ContextSource, optional Object OptionalOverrideContextSource)` / `native static final function bool SetBaseValue(AttributeDefinition DestAttribute, out AttributeInitializationData BaseValue, Object ValueContextSource, Object AttributeContextSource)` | XP, health, every balance number | high | UNVERIFIED |
| Object.AddModifier / RemoveModifier / GetAttributeValueByName / GetAttributeModiferDescriptor | `native final function bool AddModifier(AttributeModifier mod, name AttributeName, optional bool bSuppressNotify)`, `float GetAttributeValueByName(name AttributeName)` | direct by-name access | high / medium | UNVERIFIED |
| Attribute property storage (internal to the above) | n/a | what add / remove / set-base do to numbers | high | UNVERIFIED |
| Attribute change notification (virtual on the owner, internal) | n/a (a native virtual taking the property name) | pool clamp, skill grade refresh | high | UNVERIFIED |
| Context resolver classes (Controller, Pawn, Weapon, OffHandWeapon, PlayerController, PlayerReplicationInfo, NoContextNeeded, ResourcePool, WeaponResourcePool, GameInfo, BalancedActor, Inventory, EquippedInventory, StatusEffectChanceModifier, Designer*, Skill) | virtual `GetAttributeContext(AttributeDefinitionBase Attribute, Object AttributeContextSource)` | which object an attribute lands on | medium to high | UNVERIFIED |
| Value resolver classes (ObjectProperty, Constant, SimpleMath, Conditional, Global, PlayerSkill, ResourcePoolState, AttributeSlotEffect, Badass, Manufacturer, WeaponType) | virtual get / get-base / add / remove / set-base | how a number is read | medium to high | UNVERIFIED |
| GlobalAttributeValueResolver.GetGlobalAttributeValue / SetGlobalAttributeValue | `native static final function float GetGlobalAttributeValue(EGlobalAttributes Attribute)` / `SetGlobalAttributeValue(EGlobalAttributes Attribute, float Value)` | XP curve level slot | high | UNVERIFIED |
| AttributeEffect.ApplyAttributeEffects / RemoveAttributeEffects (script) | `static final function ApplyAttributeEffects(Object ContextSource, out array<AttributeEffectData> InAttributeEffects, out array<AppliedAttributeEffect> OutModifiers, optional Object OptionalOverrideContextSource)` | the loop every item and weapon effect list goes through | high | UNVERIFIED |
| ResourcePool.ClearAttributeModifierStacks | `native function ClearAttributeModifierStacks()` (virtual on the pool) | pool reset | medium | UNVERIFIED |
| ResourcePoolManager.RecalculateBaseValues | `native final function RecalculateBaseValues(ResourcePoolManager Mgr)` | pool rebase after level / upgrades | low | UNVERIFIED |
| WillowWeapon.RecomputeAttributeBaseValues | `native function RecomputeAttributeBaseValues()` | weapon (re)build resets | medium | UNVERIFIED |
| WillowInventory.GetAttributeSlotGrade / GetAttributeSlotModifierValue / GetAttributeSlotIndex / GetAttributeSlotIndexByAttributeDef | `native function int GetAttributeSlotGrade(name SlotName)`, `float GetAttributeSlotModifierValue(name SlotName)` ... | the lent Maliwan pistol's stat slots, card text | medium | UNVERIFIED |
| WillowInventory.InitializeAttributeSlots / InitializeAttributeSlotsForNameParts | `native function InitializeAttributeSlots(bool bIncludeNameParts)` | slot grade bookkeeping at weapon build | medium | UNVERIFIED |
| WillowInventory.ApplyInternalSlotEffectModifiers / ApplyExternalSlotEffectModifiers | `native function ApplyInternalSlotEffectModifiers(bool bBackupSlotEffectsApplied, int MaxSlotsActivated, out array<AppliedAttributeEffect> AttributeModifiers)` | slot modifiers land on weapon / owner | high | UNVERIFIED |
| Skill.GetAttributeContexts | `native function GetAttributeContexts(Controller EffectInstigator, out AppliedSkillEffect SkillEffect)` | where each Maya effect lands | medium | UNVERIFIED |
| WillowPawn.GetAttributeContextSource / WillowInteractiveObject.GetAttributeContextSource | `native function Object GetAttributeContextSource()` | pawn context redirect | medium | UNVERIFIED |
| AttributeExpression.EvaluateExpression / EvaluateExpressions, AttributeExpressionEvaluator.Evaluate | `native static final function bool EvaluateExpression(Object ContextSource, out AttributeExpressionData Expression, optional Object OptionalOverrideContextSource)` | attribute comparisons in constraints | high | UNVERIFIED |
| Controller.RecalculateAttributeInitializedState, WillowItem.RecomputeAttributeBaseValues | virtual thunks | HUD / item refresh hooks | low (bodies not reached) | UNVERIFIED |

## 1. Context and value chains: AttributeDefinitionBase / AttributeDefinition natives

### AttributeDefinitionBase.ResolveContext(ContextSource)
- **Reads:** the definition's `ContextResolverChain`.
- **Does (in order):** runs the chain left to right, each resolver receiving what the previous returned (the first
  receives `ContextSource`). A null chain entry is skipped. If a resolver returns None the walk stops and the result is
  None; an **empty chain returns None** (it does not pass the source through). The result is the last resolver's answer.
- **Calls other natives:** each resolver's `GetAttributeContext` (section 5). Script callers: `AttributeEffect.ApplyAttributeEffects`
  and every `GetAttributeContexts` consumer.
- **Edge cases:** None source: most resolvers return None; `NoContextNeededAttributeContextResolver` returns the engine
  globals object instead (it is how constant attributes always resolve).
- **Implementer checklist:** chain of functions object -> object; first None ends it; empty chain = None.
- **Open:** none.

### AttributeDefinitionBase.ResolveContexts(ContextSource, out ResolvedContexts)
Runs the definition's own multi-result resolution (`AttributeDefinition`: the single `ResolveContext` answer is appended
to the output array if it is not None; `AttributeDefinitionMultiContext`: the chain gives one context, which the definition's
`MultiContextResolver` object expands to a list). Used by `SetAttributeBaseValue` below. **Confidence medium** for the
multi-context expansion (the resolver class was not read).

### AttributeDefinitionBase.GetValueFromContext(Context) and GetBaseValueFromContext(Context)
- **Does:** 0 for a None context or an empty chain. Otherwise runs the `ValueResolverChain` in order, each resolver
  receiving `(Context, previous result)` (previous starts at 0) and the **last result is returned**. The base variant
  uses each resolver's base-value method instead of its value method (default: the same as value, section 6).
- **Implementer checklist:** one fold over the chain; the usual chain has one resolver (`ObjectPropertyAttributeValueResolver`
  or `ConstantAttributeValueResolver`); a chain of several (for example an ObjectProperty read followed by `SimpleMath`)
  feeds the previous number forward.

### AttributeDefinition.GetValue(ContextSource, out ResolvedContext, OptionalOverrideContextSource) and GetBaseValue
- **Does (in order):** if an override source is given and the context chain resolves it to a context, that context is
  used; otherwise `ContextSource` goes through the chain. The resolved context is written to the optional out argument
  (None when it did not resolve). With a context, the result is `GetValueFromContext` (or the base variant); with none,
  **0.0**.
- **Edge cases:** the override is tried first; falling back to the source is silent.

### AttributeDefinition.StaticGetAttributeValueOrDefault(Definition, ContextSource, DefaultValue, OptionalOverrideContextSource)
Definition None -> `DefaultValue`. Otherwise the same override-first resolution as `GetValue`; if no context resolves ->
`DefaultValue`, else the value from the chain. (A resolved context with a value of 0 returns 0, not the default.) This is
what formulas use for "attribute or constant" reads.

### AttributeDefinitionMultiContext.GetValues / GetBaseValues(ContextSource, out Values, out ResolvedContexts, Override)
Resolves the list of contexts (override tried first, then the source), runs the value chain on each, appends one float per
context in order; the optional second array receives the contexts. Not used in the slice. `StaticGetAttributeValues` and
`GetDescriptors` not read.

### AttributeDefinitionBase.AddAttributeModifier(Context, Modifier, bSuppressNotify) / RemoveAttributeModifier
- **Note the argument:** these take the **already resolved context** (the object that owns the number), not a source.
  `AttributeEffect.ApplyAttributeEffects` resolves first (section 10), then adds.
- **Does:** 0 resolvers or a None context -> false. Otherwise it asks the value resolvers in order, each only while the
  previous reported success; a null entry counts as failure. The result is the success of the chain. In practice only
  `ObjectPropertyAttributeValueResolver` supports add / remove / set-base; **every other resolver class answers false** (a
  shared refuse stub), so an effect on a constant or derived attribute (`ConstantAttributeValueResolver`, `SimpleMath`,
  `Conditional`, `Global`, `Manufacturer`, `WeaponType`, `PlayerSkill`, `Badass`, `ResourcePoolState`, `AttributeSlotEffect`) is
  refused and nothing changes.
- **Calls other natives:** the property-level add / remove of section 3.
- **Edge cases:** a second add of the same modifier object: false; removing a modifier that is not there: false.
- **Implementer checklist:** return false for non-property attributes; apply section 3 for property attributes; fire the
  notification (section 4) after a successful change unless `bSuppressNotify`.

### AttributeDefinitionBase.SetAttributeBaseValue(ContextSource, BaseValue, bSuppressNotify)
Resolves the context list (as `ResolveContexts`), then for each value resolver and each context sets the base value
through the resolver (section 3). Returns **true only if there was at least one context and one resolver and every set
succeeded**; after the first failure it stops calling. The stack and value are recomputed by the property (section 3).
Script users: `Skill.UpdateGrade`, `InstancedDesignerAttribute`, `AttributeInitializationDefinition.SetBaseValue`.

### AttributeInitializationDefinition.EvaluateInitializationData and SetBaseValue
`EvaluateInitializationData` is the evaluator of NATIVE_PROGRESSION section 1, re-read here: the override context is
**tried first** for the `BaseValueAttribute` read; the order after that is: base (constant or attribute), definition combined
by `BaseValueMode`, enabled random variance **added**, times `BaseValueScaleConstant`, then (definition set) minimum, maximum,
rounding. Enum names now fixed from `Engine.upk` (closing the open item of NATIVE_PROGRESSION): `BaseValueMode` 0
`InitializationDefSetsBaseValue`, 1 `AddsToBaseValue`, 2 `ScalesBaseValue`, 3 `OffsetByBaseValue` (value is the definition
result **minus** the base); `RoundingMode` 0 `Float`, 1 `IntRound`, 2 `IntFloor`, 3 `IntCeil`. The value formula is
`Multiplier * (Level ^ Power + Offset)` (Power only applied when it is not 1), confirming section 1's "offset before
multiplier". The conditional form takes the first entry of its list whose expressions all hold (the same "all" evaluation as
`AttributeExpression.EvaluateExpressions` in And mode), else the default.
`SetBaseValue(DestAttribute, BaseValue, ValueContextSource, AttributeContextSource)`: evaluates `BaseValue` with
`ValueContextSource`, then `DestAttribute.SetAttributeBaseValue(AttributeContextSource, result)`; false when `DestAttribute` is None.

### Object.AddModifier / RemoveModifier(mod, AttributeName, bSuppressNotify), GetAttributeValueByName, GetAttributeModiferDescriptor
They look the named property up on **the object's own class** and apply the property-level operation of section 3 directly
(no definition, no chains). `GetAttributeValueByName` returns the property's stored value (0 if the name is not an attribute).
`GetAttributeModiferDescriptor` returns a string describing the property (formatting not read). The lookup helper as
read walks the object's own class member list; the data uses inherited properties through definitions (for example
`ExpLevel` on weapons), so a superclass walk happens somewhere (see Open).

## 2. Notes the other lanes should reuse

- Confirmed: `Skill.UpdateGrade` -> `SetAttributeBaseValue` -> notification forces the skill refresh (section 4).
- Slice reads: `PlayerExperienceLevel` is a **simple** attribute reading `ExpLevel` from the player replication info;
  `Att_UniversalBalanceScaler` and `Att_UniversalBalanceMultiplier_HealthShields` are simple constant attributes (1.13 and
  80, as NATIVE_PROGRESSION says).

## 3. The property-level rules (what add / remove / set-base do)

Applied by the `ObjectPropertyAttributeValueResolver` to the property it names. It looks the property up by name on the
context's class, caches it, and reuses the cache for any context whose class derives from the property's owner.

**Read value:** a stack attribute or plain attribute returns its stored number (float; int and byte converted to float;
anything that is neither returns 0). **Read base:** a stack attribute reads its base companion; a plain attribute reads
itself.

**Add a modifier, stack attribute:** fails (false) if that modifier object is already in the stack; otherwise appends it,
recomputes the value, optionally notifies, returns true. The recompute is the formula of NATIVE_WEAPON_RULES section 1 over
the base companion and the whole stack: `(base + sum PreAdd) * ((1 + sum of positive Scale) / (1 - sum of negative-or-zero
Scale)) + sum PostAdd`, single precision; integer attributes use the same arithmetic in `float` and **truncate toward zero**
when storing. Confirmed by reading both the float and the int routines (they differ only in the final conversion).

**Remove, stack attribute:** removes **every** occurrence of the modifier from the stack; false if none was present;
otherwise recomputes the value from the base and the remaining stack and notifies.

**Set base, stack attribute:** writes the base companion, recomputes the value from the stack, notifies. (The "simple
attribute reset" is a plain set of the base.)

**Plain attribute (no stack):** add **changes the stored value in place and permanently**: Scale multiplies by the raw
modifier value (no `1 +`), PreAdd and PostAdd add it, any other type does nothing; integers truncate. **Remove is a successful
no-op** (it returns true and changes nothing). Set base writes the number. This is the **resolution of the open item in
NATIVE_SKILLS**: effects aimed at the plain attributes marked simple (a pool's `CurrentValue`, `ReloadCnt`, ...) act as
one-shot edits and are not undone by deactivating the skill, which is why `Skill.AdjustModifiers` modes 1 and 2 skip simple
attributes (the 208 simple definitions in `Startup.upk` are almost all constants, derived reads and plain fields).

A property that is neither a stack nor a plain attribute: every operation returns false.

**Missing stack:** adding or removing on a stack attribute whose companion stack property cannot be found raises an engine
error ("Modifier stack not found for attribute"); treat as a data bug, not a gameplay path.

**Implementer checklist**
1. Store value, base and modifier list per attribute; recompute value on every add, remove and set-base.
2. Never store a computed value as the base; a modifier add on a stack attribute leaves the base alone.
3. Distinguish stack attributes from plain ones; keep "remove is a no-op" for the latter.
4. Dedupe on add by modifier identity; remove deletes all copies.
5. Notify only if requested and the property has the notify flag.

## 4. Attribute change notification

After a successful add, remove or set-base without `bSuppressNotify`, if the property carries the notify flag, the owner
object's native virtual "attribute value changed" is called with the property's name. The base implementation does nothing.
Two overrides were read:
- **`ResourcePool`**: when the changed name is `CurrentValue`, the current value is clamped: below `MinValue` it becomes
  `MinValue`; at or above `MaxValue` it becomes `MaxValue`; otherwise unchanged (the min test is first, so with a min above
  the max the min wins).
- **`Skill`**: when the changed name is `Grade`, the skill's `bForceRefreshModifiersNextTick` is set. This is the link that
  makes class-mod skill-level bonuses (modifiers on `Skill.Grade` through the `SkillGradeModifiers.*` definitions) and
  `Skill.UpdateGrade` re-apply the skill's effects on the next manager tick (NATIVE_SKILLS 4.5).
Other owners (weapon `ClipSize`, inventory manager `WeaponReadyMax`, interactive object `MaxHealth`, game info
`ShopTimerRate`) have the flag; whether their classes override the virtual was not resolved (see Open). No script event is
raised by this path.

## 5. Context resolvers (class `AttributeContextResolver`, virtual `GetAttributeContext`)
Each takes the context from the previous step (or the source) and returns an object or None.

| Resolver | Result | Confidence |
|---|---|---|
| `NoContextNeededAttributeContextResolver` | the given object, or the engine globals object when None is given (never None) | high |
| `ControllerAttributeContextResolver` | a controller source -> itself; a pawn or other actor with an Instigator -> the instigator's controller; a pawn without one -> its controller, or its vehicle's controller when driving | medium |
| `PawnAttributeContextResolver` | a pawn -> itself; a controller -> its pawn; another actor -> its Instigator | medium |
| `WeaponAttributeContextResolver` | a weapon -> itself; a controller -> its pawn's `Weapon`; a pawn -> its `Weapon` | high |
| `OffHandWeaponAttributeContextResolver` | a weapon -> itself; controller or pawn -> the pawn's `OffHandWeapon` | high |
| `PlayerControllerAttributeContextResolver` | the player controller reachable from the source (itself, or through its owner interface) | medium-low |
| `PlayerReplicationInfoAttributeContextResolver` | the source's replication info: controller -> `PlayerReplicationInfo`; pawn -> `PlayerReplicationInfo`; a replication info -> itself | medium |
| `ResourcePoolAttributeContextResolver` | runs the `Resource` definition's own owner resolver on the source to find the owner, then returns that owner's pool for the resource (None when no pool) | medium |
| `WeaponResourcePoolAttributeContextResolver` | as above with `PrimaryHandResource`, or `OffHandResource` when the source weapon has `bOffHand` set | medium |
| `GameInfoAttributeContextResolver` | the game info (when the engine globals exist) | medium |
| `BalancedActorAttributeContextResolver`, `CurrentProficiencySkillAttributeContextResolver`, `StatusEffectChanceModifierResolver` | native implementations; the script-callable `GetAttributeContext` of these three shares one thunk that calls the virtual. BalancedActor: the source if it implements the balanced-actor interface, else None. Status-effect resolver: the source if it is a status-effect proxy, otherwise through the proxy's interface; not read in detail | medium-low |
| `InventoryAttributeContextResolver` | a lookup through the source's interface for the item matching `InventoryDefinition` (optionally only equipped items) | low |
| `EquippedInventoryAttributeContextResolver` | the item the source's inventory manager holds at `EquipmentLocation` | medium |
| `SkillAttributeContextResolver` (script-only), `DesignerAttributeContextResolver`, `DesignerAttributeContextResolverByName` (script) | Designer: asks the source (or its owner) for its `IDesignerAttributeProvider` and returns that provider's `InstancedDesignerAttribute` for the definition's `ValueName`; the ByName form first checks `HasDesignerAttribute` | medium |

The designer object (`InstancedDesignerAttribute`) holds three triples (float `Value`, int `IntValue`, bool-as-int
`BoolValue`) each with base and stack; its `SetBaseValue` evaluates the initialization data into the float base and copies
the integer truncation to the others. **Maya's Phaselock duration (`Att_Phaselock_Duration`) is such an attribute**: Suspension's
PostAdd +0.5 per grade lands on the `Value` stack, so the lock time is `(base + PreAdd) * Scale + PostAdd` = 5 + 0.5 g, which
confirms the assumption in PHASELOCK_STOCK_DATA ("UNVERIFIED") at the level of the engine rule.

## 6. Value resolvers (class `AttributeValueResolver`; virtual get, get-base, add, remove, set-base)
Default get-base calls get; default add / remove / set-base answer false.

| Resolver | Get | Supports add / remove / set | Confidence |
|---|---|---|---|
| `ObjectPropertyAttributeValueResolver` | property value on the context (`PropertyName`) | yes (section 3) | high |
| `ReadOnlyObjectPropertyAttributeValueResolver` | same read | none (not read in detail) | low |
| `ConstantAttributeValueResolver` | `ConstantValue` | no | high |
| `SimpleMathValueResolver` | the previous value, or the `Arg1Attribute` init data when `Arg1Option` is `FromAttribute`, combined with `Argument` by `Operand`: Add `arg + prev`, Sub `prev - arg`, Mul `arg * prev`, Div `prev / arg` (0 when `abs(arg)` is below 1e-8) | no | high |
| `ConditionalAttributeValueResolver` | the conditional-initialization result of `ValueExpressions`, ignoring the previous value | no | medium |
| `GlobalAttributeValueResolver` | global slot `GlobalAttribute` (0 `ExperiencePointTestLevel`, 1 `BadassTokenTestRank`; other values 0) | no | high |
| `PlayerSkillAttributeValueResolver` | the grade of `AssociatedSkill` in the player's skill tree (0 when the player or tree is missing) | no | medium |
| `ResourcePoolStateAttributeValueResolver` | 1.0 if the pool is in `PoolState` (Depleted, Filled, Regenerating), else 0 | no | high |
| `AttributeSlotEffectAttributeValueResolver` | the item's slot named `SlotName`: grade (`SlotProperty` Grade) or its computed modifier value (`ComputedModifierValue`); needs the item's slot-provider interface, else 0 | no | high |
| `BadassAttributeValueResolver` | reads the profile's badass reward rank / token state for `AssociatedBadassReward`; **its set-base writes the rank (rounded half up)** | set only | low |
| `ManufacturerAttributeValueResolver` | `ValueIfNotMatched` unless the item's manufacturer matches a `Manufacturers` entry, then that entry's init data | no | medium |
| `WeaponTypeAttributeValueResolver` | as above keyed by the weapon type byte | no | medium |

Natives on these classes: `GlobalAttributeValueResolver.GetGlobalAttributeValue(Attribute)` returns the slot (0 for an
index at or beyond 2, 0.0 default); `SetGlobalAttributeValue(Attribute, Value)` stores it; the XP curve sets slot 0 to the
level it evaluates (NATIVE_PROGRESSION 3). `PlayerClassCountAttributeValueResolver.SetPlayerClassCountOverride /
ResetPlayerClassCountOverride`: bodies not read (a test override of the per-class player count).

## 7. AttributeEffect.ApplyAttributeEffects / RemoveAttributeEffects (script, `Engine.upk`)
`ApplyAttributeEffects(ContextSource, InAttributeEffects, out OutModifiers, OptionalOverrideContextSource)`. For each effect
data entry, in order:
1. create a new `AttributeModifier` object (outer: the context source);
2. `Type` = the entry's `ModifierType`; `Value` = `EvaluateInitializationData(BaseModifierValue, ContextSource, Override)`,
   so the value is **computed once, at apply time** (a later change of the input attribute does not retarget it);
3. resolve the context: the entry's attribute `ResolveContext(Override)` if an override was given, and if that gave None,
   `ResolveContext(ContextSource)`;
4. if a context was found and `AddAttributeModifier(Context, Modifier)` returned true, append the applied record (context,
   attribute, modifier) to `OutModifiers`; failed entries leave no record.
`RemoveAttributeEffects(EffectModifiers)`: for every record with a context, `RemoveAttributeModifier(Context, Modifier)`, then the
array is emptied. Equipped item effects pass the controller as source and the item as the override
(`WillowItem.ApplyAllExternalAttributeEffects`), so for an attribute whose chain starts with an item-based resolver the
item wins and otherwise the controller's chain is used. A native twin of step 1 to 4 exists for
`BalanceModifierDefinition.ApplyPlayThroughBasedPlayerAttributeEffects` / `UpdateSpawnedPlayerEnemyAIPawn`; both appear to act
only when a playthrough number read from the game state is above 1, so they should be **no-ops for the slice**
(playthrough 1; the exact value tested was not confirmed).

## 8. Reset and bookkeeping natives
- **ResourcePool.ClearAttributeModifierStacks** (virtual on the pool): empties the modifier arrays of `MinValue`, `MaxValue`,
  `ConsumptionRate`, `ActiveRegenerationRate`, `OnIdleRegenerationRate`, `OnIdleRegenerationDelay` and
  `PassiveRegenerationRate`. It does **not** recompute the values and does not touch `RegenerationDisabled`. Used when a pool is
  rebuilt; after it the caller must re-add its modifiers (confidence medium).
- **WillowWeapon.RecomputeAttributeBaseValues**: for every stack attribute declared on the weapon class, the modifier stack
  is emptied and the value is reset to its base; afterwards the weapon's `WeaponAttributeModifiers` bookkeeping array is
  emptied. A weapon rebuild therefore starts from the bases and reapplies all effects (NATIVE_WEAPON_RULES section 1 order).
  The reset covers the properties of the weapon class as declared (inherited ones were not confirmed).
- **WillowItem.RecomputeAttributeBaseValues, Controller / WillowPlayerController / WillowMind / WillowVehicle
  RecalculateAttributeInitializedState**: thin native thunks that call a virtual method; the bodies were not reached
  (the class vtables could not be resolved by name). Script calls them after level or class changes.
- **ResourcePoolManager.RecalculateBaseValues(Mgr)**: for each of the manager's 16 pool slots that holds a pool, calls a named
  script function on the pool with a true argument (name not decoded). Confidence low.

## 9. Attribute slots on items (WillowInventory natives)
Slot record layout (`WillowInventory.AttributeSlotData`, 19 per item): `SlotName`, flags (`bExternalSlot`,
`bRunEffectsAsSkill`, `bActivated`, `bIncludeAlliesAsTarget`, `bIncludeInModifierText`, `bEnforceMinimumGrade`,
`bEnforceMaximumGrade`), `MinimumGrade`, `MaximumGrade`, `TargetInstanceDataName`, `EffectGrade`, `AttributeToModify`,
`ConstraintAttribute`, `ModifierType`, `BaseModifierValue`, `PerGradeUpgrade`, `ComputedModifierValue`.
- **GetAttributeSlotGrade(SlotName) / GetAttributeSlotModifierValue(SlotName)**: forwards to the item's
  `IIAttributeSlotEffectProvider` interface (the same interface the slot value resolver uses); values are the slot's
  `EffectGrade` and `ComputedModifierValue`. The interface bodies were **not** resolved, so the computation of
  `ComputedModifierValue` is still unread (narrowed from NATIVE_WEAPON_RULES: it is a native step run once the active slot
  list has been rebuilt; script never assigns it).
- **GetAttributeSlotIndex(SlotName)**: index of the first activated slot with that name among the first
  `AttributeSlotMaxActivated` slots (clamped to 1..19, default 19), else -1. **...ByAttributeDef(Def)**: first slot (up to the
  same limit) whose `AttributeToModify` is `Def`, else -1.
- **InitializeAttributeSlots(bIncludeNameParts) / ...ForNameParts**: builds the slot list from the type's and then each
  part's `AttributeSlotUpgrades` (and the name parts' for the second form). Grade accumulation as in NATIVE_WEAPON_RULES
  (activation adds the type's `AttributeSlotBaseGrade`, rounded half up, once; every entry adds its `GradeIncrease`). Added here:
  after a slot's data is copied, **`MaximumGrade` is enforced first (when `bEnforceMaximumGrade`), then `MinimumGrade` (when
  `bEnforceMinimumGrade`)** on `EffectGrade`, so the minimum wins when the two conflict. An activation count above the limit is
  ignored.
- **ApplyInternalSlotEffectModifiers(bBackup, MaxSlotsActivated, out AttributeModifiers)**: unless `bBackup`, first removes
  everything recorded in the item's `AppliedAttributeSlotEffects` backup list and empties it. Then, for each of the first
  `MaxSlotsActivated` slots that is **activated, not external and has an `AttributeToModify`**: creates a modifier with the slot's
  `ModifierType` and `ComputedModifierValue`, resolves the attribute's context **from the item itself**, adds it, and on success
  appends a record to the out array (and to the backup list when `bBackup`). A failed add leaves nothing recorded.
- **ApplyExternalSlotEffectModifiers(ContextSource, MaxSlotsActivated, out AttributeModifiers, OverrideContextSource)**: same
  loop for slots that are activated, **external, and not `bRunEffectsAsSkill`** (those are applied by the skill machinery); the
  context is resolved from the override first, then from `ContextSource` (the owning controller).
- **Implementer checklist:** one modifier object per slot; type and value copied from the slot (not recomputed at apply
  time); internal slots land on the item's own context (for weapons, the weapon), external ones on the owner; removal goes
  through the recorded list.
- **Open:** the `ComputedModifierValue` computation (NATIVE_WEAPON_RULES gives the working fit `BaseModifierValue +
  PerGradeUpgrade * grade`, still the best available rule); how `ReplicatedAttributeSlotModifierValues` (a 19-float array) is
  filled on clients.

## 10. Skill.GetAttributeContexts(EffectInstigator, SkillEffect)
Fills the applied-effect record's context list. If the effect's attribute names, as its first context resolver, the
**skill context resolver**, the single context is the skill itself (this is how the `SkillGradeModifiers.*` attributes reach
`Skill.Grade`). Otherwise a target search runs (self / allies / enemies per the effect data and the instigator's pawn) and
each target found is passed through the attribute's own context resolution to produce the contexts. Afterwards the skill's
`NextContextUpdateTime` is set to now plus the definition's `SkillEffectUpdateIterval`. **Confidence medium** for the self
case, low for the target search. Consistent with NATIVE_SKILLS 4.6.
`WillowPawn.GetAttributeContextSource` / `WillowInteractiveObject.GetAttributeContextSource`: forward to a held helper
object's equivalent method (the pawn's and the object's balance-data owner) and return its answer; used to redirect a pawn's
context to the object that carries the attributes. Confidence low (the helper's answer was not read).

## 11. AttributeExpression.EvaluateExpression(ContextSource, Expression, OptionalOverrideContextSource) / EvaluateExpressions / AttributeExpressionEvaluator.Evaluate
- **Struct** `AttributeExpressionData`: `AttributeOperand1`, `ComparisonOperator` (0 `EqualTo`, 1 `NotEqualTo`, 2 `LessThan`,
  3 `LessThanOrEqual`, 4 `GreaterThan`, 5 `GreaterThanOrEqual`), `Operand2Usage` (0 `PreferAttribute`, 1 `Multiply`),
  `AttributeOperand2`, `ConstantOperand2`.
- **Does:** left = `AttributeOperand1.GetValue(source, override)`; if the attribute is None or its context does not resolve
  the expression is **false**. Right: if `AttributeOperand2` is None, `ConstantOperand2`; otherwise its attribute value, and when
  `Operand2Usage` is `Multiply` the constant times that value; if the right attribute is set but does not resolve, the
  expression is false. Then the comparison in single-precision floats (exact equality for `EqualTo` / `NotEqualTo`).
- **EvaluateExpressions(Mode, ContextSource, Expressions, Override)**: Mode 0 (And): true when all are true, stops at the first
  false, **an empty list is true**; Mode 1 (Or): true when any is true, stops at the first true, an empty list is true (the
  fold starts true). `AttributeExpressionEvaluator.Evaluate(ContextSource)` evaluates its single `Expression` the same way
  (virtual, called by the constraint machinery of NATIVE_PHASELOCK_TARGETING / NATIVE_SKILLS).

## Maya, Phaselock, the lent pistol: what this means for the slice
- Maya's Phaselock **duration** (`Att_Phaselock_Duration`, designer attribute) and cooldown (`ActiveSkillCooldown*` pool
  attributes via the controller's cooldown pool) are stack attributes on non-simple contexts: effects are added at activation, removed on
  deactivation / pause, and re-added on refresh, with the stack formula of section 3.
- Ward (`ShieldMaxValue` Scale) and the health pool: the shield pool and health pool are found through `ResourcePoolAttributeContextResolver`
  (owner -> pool for the resource); `HealthMaxValue_Player` reaches the pool from the player controller. A Scale of +0.05 per grade
  multiplies the shield `MaxValue` base by `1 + 0.05 g`; the pool then clamps `CurrentValue` through the notification (section 4)
  only when `CurrentValue` itself changes.
- Accelerate (`WeaponDamage` Scale, `WeaponProjectileSpeedMultiplier`): the context is the **currently held weapon**
  (controller -> pawn -> `Weapon`), resolved when the effect is applied or refreshed; the skill refresh re-resolves, so a weapon swap
  moves the effect at the next refresh (the auto-update interval), not at the swap.
- The lent Maliwan pistol: weapon stats are stack attributes on the weapon (`InstantHitDamage`, `ClipSize`, ...), reset by
  `RecomputeAttributeBaseValues`, rebuilt by the order in NATIVE_WEAPON_RULES, with item-slot effects applied by section 9.
- XP and level: `PlayerExperienceLevel` and `ExperienceCurrentValue` are simple (plain) attributes; adding an
  experience modifier through an effect would apply once, in place.

## Not read yet
- Bodies of `AttributeDefinitionMultiContext.StaticGetAttributeValues`, `GetDescriptors`, `GetDescriptorFromContext`, `UObject.GetAttributeModiferDescriptor`
  formatting; `AttributeMultiContextResolver` subclasses.
- Context resolvers: `ProjectileAttributeContextResolver`, `ActorAttributeContextResolver`, `VehicleAttributeContextResolver`,
  `WillowInventoryManagerAttributeContextResolver`, `WillowInteractiveObjectAttributeContextResolver`,
  `ObjectPropertyContextResolver`, `BestTarget...`, `PopulationMaster...` (their registration could not be resolved by name).
- Value resolvers: `TargetableAttributeValueResolver`, `StateAttributeResolver`, `RandomAttributeValueResolver`,
  `NounAttributeValueResolver`, `TimeValueResolver`, `CurrencyAttributeValueResolver`, `LevelNameAttributeValueResolver`,
  `ShopTimerRateValueResolver`, `AmmoDropWeightAttributeValueResolver`, `AmmoResourceUpgradeAttributeValueResolver`,
  `BlackMarketUpgradeAttributeValueResolver`, `ClassDropWeightValueResolver`, `DamageTypeAttributeValueResolver`, `PlayerStat...`,
  `AIResourceAttributeValueResolver`.
- `AWillowInventory` interface bodies (slot computed value), `PopulationFactoryBalancedAIPawn.ApplyAttributeStartingValues`,
  `BalanceModifierDefinition.*` body detail (other lanes), `AttributePresentationDefinition.*` natives (display rules, see
  NATIVE_WEAPON_RULES section 2), `WillowGameInfo` startup-teleporter attribute natives.

## Corrections to earlier notes
- **NATIVE_SKILLS (Open: "what `bIsSimpleAttribute` means for removal")**: answered in section 3. A plain (non-stack) attribute
  takes a modifier by editing its stored value in place and its remove is a no-op that reports success, so skipping simple
  attributes in modes 1 and 2 is consistent. Note this also means skill effects aimed at simple attributes are one-shot at
  activation (mode 0) and never withdrawn.
- **NATIVE_SKILLS 4.4**: "adding refuses a modifier already on the stack" confirmed; removal deletes every copy; and the
  notification is a native virtual on the owner that fires **only for properties carrying the notify flag**, not "the owner of
  the attribute change" in general (section 4). Only `Skill.Grade` and `ResourcePool.CurrentValue` overrides were read.
- **NATIVE_SKILLS 4.5**: the force-refresh after a grade change from class-mod bonuses is driven by the `Skill` override of this
  notification, not only by `Skill.UpdateGrade`'s script.
- **NATIVE_PROGRESSION section 1** (open item): `BaseValueMode` 1..3 are `AddsToBaseValue`, `ScalesBaseValue`,
  `OffsetByBaseValue` (definition result minus base). No contradiction with the evaluator order; the optional override context
  is tried first (NATIVE_PROGRESSION says "tried first": matches).
- **NATIVE_WEAPON_RULES section 1**: the stack formula is confirmed from the property code (float and integer variants). Added: a
  plain attribute does **not** use `(1 + Scale)` semantics: Scale multiplies directly (section 3); slot grade clamps apply max
  first, then min (section 9); the one remaining unread piece (`ComputedModifierValue`) is native and not script-assigned.
- **PHASELOCK_STOCK_DATA ("`(base + PreAdd) * (1 + Scale) + PostAdd` is UNVERIFIED")**: the engine rule is the one read here;
  still not confirmed in game.

## Open
- Whether `ObjectPropertyAttributeValueResolver`'s property lookup walks superclasses (the data relies on inherited properties
  such as `ExpLevel`); behaviourally a name lookup over the whole class chain is what the data needs.
- How the script VM's typed assignment to an attribute property (the Gearbox typed-let opcodes used by
  `InstancedDesignerAttribute.SetBaseValue` and `WillowWeapon.CalculateWeaponBaseValues`) is executed. Read here as "set the
  base and recompute from the stack, with notification" because nothing else in the packages writes a stack attribute's base;
  not read in the VM. This matters for `src/vm.cpp`.
- Which classes besides `ResourcePool` and `Skill` override the change notification.
- Context-resolver chain order is not decodable from the packages (object arrays are not decoded); the table in section 5
  states each resolver's behaviour, and the chains seen in `Startup.upk` are consistent with "first resolver takes the source,
  later ones refine" (for example `HealthMaxValue_Player`: player controller, then pool).
