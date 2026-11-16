# Native noun attributes: `NounAttributeValueResolver` and the noun store of `INounAttributeProvider` (draft, 2026-11-16)

AI-assisted (Claude Code), analyst lane FIX-NOUN. Read from the game executable with Ghidra 12.1.4 (`tools/ghidra/`,
driven by `tools/ghidra/run.ps1` against a project made by `tools/ghidra_import.ps1`), written in our own words; no
decompiler output, pseudo-code or listing structure is reproduced here. Names, field types, enum values and the field
layout come from the cooked packages (`ow-package --exports` and `--payload`, `tools/ghidra/class_layout.py`,
`research/script_disasm.py`). **Every rule is UNVERIFIED in the running game**; nothing here was observed live.

This note is the hand-off to the implementer of the `ParityMode::Off` rule. The same session that read the native
also wrote a first implementation; that implementation was discarded before publication (decision 2026-11-16-37), so
the Off rule is to be built by a fresh implementer from this note alone. Section "Implementer checklist" at the end
lists everything to build.

## 1. The shape (from the packages)

- `GearboxFramework.NounAttributeValueResolver` declares two fields: `ReplicationStrategy` (a byte of
  `INounAttributeProvider.ENounReplicationStrategy`) and `DefaultValue` (`AttributeInitializationData`). It has no
  script functions and registers no natives. Its rules are the overrides of the value resolver's virtual get,
  get-base, add, remove and set-base (NATIVE_ATTRIBUTES.md section 6); everything else is the base class's default.
- `ENounReplicationStrategy`, in order: 0 `NOUNREP_NoReplication`, 1 `NOUNREP_RepToOwnerOnly`,
  2 `NOUNREP_RepToEveryone` (then `NOUNREP_MAX`).
- `GearboxFramework.INounAttributeProvider` is a native interface with two structs:
  - `NounAttributeState`, in field order: `NounName` (name), `Value` (float), `BaseValue` (float), `ModifierStack`
    (array of `AttributeModifier`), `ReplicationStrategy` (the enum), `IndexInReplicationArray` (int).
  - `ReplicatedNounAttributeState`: `NounName`, `Value` (float), `bOnlyRelevantToOwner` (bool).
- Four classes implement the interface, and they are exactly the four whose class declares a `NounState` array of
  `NounAttributeState`: `GearboxMind`, `WillowPawn`, `WillowPlayerReplicationInfo`, `WillowWeapon`. The last three also
  declare a fixed-size `ReplicatedNouns` array of `ReplicatedNounAttributeState` and an int `NumReplicatedNouns`.
- The installed data has 15 Noun resolvers. Each is the only value resolver of its attribute and a subobject of that
  attribute definition. The attributes resolve their context through the WillowMind (12, among them the
  `GD_Balance_HealthAndDamage.AIParameters` multipliers such as `Attribute_HealthMultiplier`), the weapon (2,
  `D_Attributes.Weapon.WeaponMaxReloadAnimTime` with default 999 and `WeaponMaxFastReloadAnimTime` with default -1)
  or the pawn (1, `GD_Siren_Skills.Attributes.IsPhaselocked`, default 0). Every mind and weapon noun has strategy 0;
  `IsPhaselocked` has strategy 2.
- How a noun gets its number in the game (script, not native): `WillowPawn.ApplyAttributeStartingValues` and
  `WillowPawn.GetAttributeStartingValuesFromCharacterClass` call `AttributeInitializationDefinition.SetBaseValue` with
  (the entry's attribute, the entry's base value data, the pawn as context source, the pawn) for each starting value:
  first the controller's character class, walked up its `AttributeParentClass` chain with the parents applied first
  (each class's `AttributeStartingValues`, then its `AttributeStartingValuesNew` categories), then the pawn's own
  `AttributeStartingValues`. An attribute whose context resolves to the mind therefore sets the mind's noun. Who calls
  `ApplyAttributeStartingValues`, and when relative to the health pool's creation, is not read here.

## 2. Inputs

The attribute chain calls each value resolver with only two inputs: the resolved context object and the chain's
previous value. The resolver is not told which attribute it serves. The noun resolver ignores the previous value
in every operation.

## 3. The key

The key is **the name of the resolver's own outer object**: in the installed data, the attribute definition that
owns the resolver (for example `Attribute_HealthMultiplier`). Entries match by name equality, number suffix
included. A resolver with no outer has no key: it behaves as on a context that is not a provider (section 4).

## 4. Which store, and what each operation returns

Terms. A **provider** is a context that implements `INounAttributeProvider`. Its **entry** for the key is the first
`NounState` element, in array order, whose `NounName` equals the key. **Strategy** is the resolver's
`ReplicationStrategy`. **Client** means the world's net mode is the network-client mode; **standalone or server** is
every other mode (the slice is single player, so always this one). `d` is the resolver's `DefaultValue`, evaluated as
ordinary initialization data (the evaluator the script's `AttributeInitializationDefinition.EvaluateInitializationData`
uses) with the context as the source and no override source, in single precision.

| Context | Strategy | Net mode | Store used | Missing entry created? | Get / get-base | Writes (add, remove, set-base) |
|---|---|---|---|---|---|---|
| not a provider, or the resolver has no outer | any | any | none | no | 0.0 / 0.0 | false |
| provider | 0 | any | `NounState` | yes, by reads and writes alike | entry `Value` / entry `BaseValue` | applied to the entry (section 5) |
| provider with a replicated array | 1 or 2 | standalone or server | `NounState`, mirrored into `ReplicatedNouns` | yes, by reads and writes alike, with a replicated slot | entry `Value` / entry `BaseValue` | applied to the entry, then mirrored |
| provider without a replicated array | 1 or 2 | any | none | no | 0.0 / 0.0 | false |
| provider with a replicated array | 1 or 2 | client | `ReplicatedNouns` only (its first `NumReplicatedNouns` slots, by `NounName`) | no | slot `Value`, or 0.0 when no slot matches / 0.0 | false |

A provider that hands over no `NounState` array behaves as "not a provider" for strategy 0. Which arrays each of the
four providers hands over was not read; expected: all four hand over `NounState`, and the three that declare
`ReplicatedNouns` hand that over too (so `GearboxMind` would be "without a replicated array"). Every installed mind
noun has strategy 0, so the data never meets the row "provider without a replicated array".

**A created entry** holds: `NounName` = the key, `Value` = `BaseValue` = `d`, an empty `ModifierStack`,
`ReplicationStrategy` = the resolver's, and `IndexInReplicationArray` = the replicated slot it got, or -1 when it got
none (strategy 0, no replicated array, or the array was full, which logs an error). A slot it gets is the next free
one (index `NumReplicatedNouns`, which then grows by one), filled with the key, `d`, and `bOnlyRelevantToOwner` true
exactly for strategy 1 (`NOUNREP_RepToOwnerOnly`); the provider is then told its replicated copy changed (an
interface call whose body was not read).

Two orderings change behaviour:

- `d` is evaluated **before** the entry is appended, so a `DefaultValue` that reads other nouns of the same context
  sees `NounState` without the new entry, and an entry those reads create comes before it.
- The replicated slot is assigned **when the entry is created**, and only then; an existing entry never gains one
  later.

## 5. The operations on an entry

Each operation below works on the entry the table gives (created first where the table says so). Where the table
says "none", reads return 0.0 and writes return false, and nothing changes.

- **Get:** the entry's `Value`. The chain's previous value is ignored. So the first get of a noun on a provider
  returns `d` and leaves the new entry behind.
- **Get-base:** the entry's `BaseValue`.
- **Add a modifier:** a null modifier returns false and nothing is looked up or created. If the modifier is already
  in the entry's `ModifierStack`, that is, the same `AttributeModifier` object is there (identity, not equal values):
  false, nothing changes. Otherwise it is appended, the value recomputed and mirrored: true.
- **Remove a modifier:** a null modifier returns false and nothing is looked up or created. Every occurrence of that
  same modifier object (identity, as for add) is removed from the entry's stack. None was there: false, nothing
  changes. Otherwise the value is recomputed and mirrored: true.
- **Set-base(x):** `BaseValue` = x, then the value is recomputed and mirrored: true. On a provider without the entry,
  the entry is created from `d` first, then its base is overwritten.
- **Recompute:** `Value` = `(BaseValue + sum PreAdd) * ((1 + sum of positive Scale) / (1 - sum of zero-or-negative
  Scale)) + sum PostAdd`, single precision, sums taken in stack order, over the entry's own `ModifierStack`. Each
  `AttributeModifier`'s type selects its sum (Scale, PreAdd, PostAdd; any other type is skipped). This is the same
  arithmetic as the property stacks (NATIVE_ATTRIBUTES.md section 3), without the integer truncation (a noun is a
  float). No clamp.
- **Mirror:** when the entry has a replicated slot (`IndexInReplicationArray` is not -1) and the provider has a
  replicated array, that slot's `Value` becomes the new `Value` and the provider is told its replicated copy changed.
- No operation raises the attribute change notification of NATIVE_ATTRIBUTES.md section 4 (a noun is not a
  property). The writes do not touch `DefaultValue`, and once the entry exists `d` is never evaluated again.

## 6. Edge cases

- Two resolvers whose outers share a name share one entry on a given context (the key is the name only). Not seen in
  the installed data.
- A noun entry persists on its context for that object's lifetime; a fresh context (a new mind) starts empty.
- A context that is not a provider (for example the player controller) reads 0.0, not `DefaultValue`.

## 7. What the port does now (`ow-attr`, UNVERIFIED)

- `ParityMode::Off`: get, get-base and the writes answer `NotYet` (`NotPorted`), "native rule in
  NATIVE_NOUN_ATTRIBUTES.md, awaiting clean-room implementation".
- `ParityMode::Reference`: the C++ reading (`src/progression.cpp` `nounValue`): the `BaseValue` of the context's
  `AIClass.AttributeStartingValues` entry for the attribute, else `DefaultValue`, both evaluated on the context; no
  noun is written, and the writes answer `NotYet` (`NotPorted`) (the C++ has none). The attribute is taken as the
  resolver's outer. This stands in for the starting values having been applied through set-base.

## 8. Implementer checklist (Off mode)

1. Key = the resolver's outer's name; no outer: reads 0.0, writes false.
2. Provider = the context implements `INounAttributeProvider` (in the installed reflection, the same as "its class
   declares `NounState`"); not a provider: reads 0.0, writes false.
3. Find by `NounName` in `NounState` order; create on a miss for reads and writes (section 4, with its two
   orderings).
4. Get = `Value`, get-base = `BaseValue`, ignoring the previous chain value.
5. Add (dedupe), remove (all copies), set-base, each followed by the recompute and the mirror; true on success.
6. Single player: the client row of the table is not needed; the replicated copy only matters for `IsPhaselocked`
   on a pawn and is read only by clients.
7. `ow-attr`'s `Storage` has no form for a noun entry (its own base and modifier stack): the writes need one.
8. **How the class values arrive.** In the game the AI class's starting values reach a noun only through the script's
   `SetBaseValue` (section 1, `WillowPawn.ApplyAttributeStartingValues`). Until that has run on a context, the first
   get returns `d`, not the class value: 1.0 for `Attribute_HealthMultiplier`, whatever the AI class lists. So the
   Off rule gives class-specific numbers only once set-base works and the starting values are applied. Who calls
   `ApplyAttributeStartingValues`, and when relative to the health pool's creation, is not read; it is a dependency
   of this implementation.

## 9. What would confirm it (live, bl-sdk)

- On a spawned enemy, read its mind's `NounState` right after the spawn. If an entry named
  `Attribute_HealthMultiplier` is there, expect value = base = the AI class's starting value (the script's
  `SetBaseValue` made it, section 8). If it is not there, expect the first reading of the attribute to create it at
  the default (1.0).
- On a weapon with no `WeaponMaxReloadAnimTime` noun, read the attribute: 999 (its default) and a new entry.
- Read `IsPhaselocked` on a Phaselocked enemy pawn and on a free one; check its `ReplicatedNouns` entry and
  `NumReplicatedNouns` (strategy 2).
- On a context that is not a provider (the player controller), any noun attribute whose context resolves there
  reads 0.
