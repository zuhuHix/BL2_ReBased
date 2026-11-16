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
owns the resolver (for example `Attribute_HealthMultiplier`). It is a name comparison, number suffix included. A
resolver with no outer has no key and finds nothing (section 4, step 1). If the outer's name is invalid the native
uses the text `<uninitialized>` as the key; that cannot happen with loaded objects.

## 4. Finding the entry (every operation starts here)

The lookup takes the resolver, the context and one flag, "this is a read" (get and get-base) or "this is a write"
(add, remove, set-base). In order:

1. No outer: nothing found.
2. The context is asked for the `INounAttributeProvider` interface. Not a provider: nothing found.
3. The provider hands over its `NounState` array and, when it has one, its replicated array with its fill count and
   capacity. Nothing found when the resolver's strategy is not 0 and the provider handed over no replicated array,
   or when the strategy is 0 and it handed over no `NounState` array. Which arrays each of the four providers hands
   over was not read (expected: all four hand over `NounState`, the three with `ReplicatedNouns` hand that over too).
4. Network client only (the world's net mode is the client mode) and a strategy that is not 0: a write finds
   nothing; a read searches only the replicated array (step 7). A standalone game or a server skips this step.
5. Search the `NounState` entries in array order for `NounName` equal to the key. The first match is the result.
6. **No match: create the entry, for a read and a write alike.** In this order:
   1. Evaluate `DefaultValue` as ordinary initialization data (the same evaluator the script's
      `AttributeInitializationDefinition.EvaluateInitializationData` uses) with the context as the source and no
      override source. Call the result `d` (single-precision float).
   2. If the strategy is not 0 and the provider handed over a replicated array: when its count is below its capacity,
      write a replicated entry at index `count` (key, `Value` = `d`, `bOnlyRelevantToOwner` true exactly when the
      strategy is 1), add 1 to the count, tell the provider its replicated copy changed (an interface call whose
      body was not read), and remember that index. When it is full, log an error ("Too many
      ReplicatedNouns! No room for ..."; which names fill it was not read), write nothing to the replicated array and
      remember -1. Otherwise (strategy
      0, or no replicated array) remember -1.
   3. Append a `NounAttributeState` to `NounState`: `NounName` = key, `Value` = `d`, `BaseValue` = `d`,
      `ModifierStack` empty, `ReplicationStrategy` = the resolver's, `IndexInReplicationArray` = the remembered index.
   4. The result is the new entry.
7. (Client read of a replicated noun only.) Search the replicated array's first `count` entries for the key; the first
   match is the result, else nothing found. No entry is created.

## 5. The operations

Outputs are floats for the reads and true / false for the writes.

- **Get:** the found entry's `Value` (on a client, a replicated entry's `Value`). Nothing found: 0.0. Because of
  step 6, the first get of a noun on a provider creates it and returns `d`.
- **Get-base:** the found entry's `BaseValue`. Nothing found: 0.0. A client reading a replicated noun gets 0.0 (the
  replicated entry has no base).
- **Add a modifier:** a null modifier answers false before the lookup. Otherwise look up (a write). Nothing found:
  false. If the modifier is already in the entry's `ModifierStack`: false, nothing changes. Otherwise append it,
  recompute (below), mirror (below), true.
- **Remove a modifier:** a null modifier answers false before the lookup. Otherwise look up (a write). Nothing found:
  false. Remove **every** occurrence of the modifier from the entry's stack; none was there: false, nothing changes.
  Otherwise recompute, mirror, true.
- **Set-base(x):** look up (a write); nothing found: false. Set `BaseValue` = x, recompute, mirror, true. On a
  provider without the entry, the lookup first creates it from `d` (step 6), then the base is overwritten.
- **Recompute:** `Value` = `(BaseValue + sum PreAdd) * ((1 + sum of positive Scale) / (1 - sum of zero-or-negative
  Scale)) + sum PostAdd`, single precision, sums taken in stack order, over the entry's own `ModifierStack`. The
  modifier type is read from each `AttributeModifier` (Scale, PreAdd, PostAdd; any other type is skipped). This is the
  same arithmetic as the property stacks (NATIVE_ATTRIBUTES.md section 3), without the integer truncation (a noun is a
  float). No clamp.
- **Mirror:** when the entry's `IndexInReplicationArray` is not -1 and the provider handed over a replicated array,
  write the new `Value` into that replicated entry and tell the provider its replicated copy changed.
- No operation raises the attribute change notification of NATIVE_ATTRIBUTES.md section 4 (a noun is not a
  property). The writes do not touch `DefaultValue`, and a later read never re-evaluates it once the entry exists.

## 6. Edge cases

- Two resolvers whose outers share a name share one entry on a given context (the key is the name only). Not seen in
  the installed data.
- A noun entry persists on its context for that object's lifetime; a fresh context (a new mind) starts empty.
- The order of the creation step matters only if evaluating `DefaultValue` reads other nouns on the same context:
  the new entry is appended after that evaluation.
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
3. Find by `NounName` in `NounState` order; create on a miss for reads and writes (section 4, step 6, in its order).
4. Get = `Value`, get-base = `BaseValue`, ignoring the previous chain value.
5. Add (dedupe), remove (all copies), set-base, each followed by the recompute and the mirror; true on success.
6. Single player: the client branch (section 4, steps 4 and 7) is not needed; the replicated copy only matters for
   `IsPhaselocked` on a pawn and is read only by clients.
7. `ow-attr`'s `Storage` has no form for a noun entry (its own base and modifier stack): the writes need one.

## 9. What would confirm it (live, bl-sdk)

- On a spawned enemy, read its mind's `NounState` before and after the first reading of
  `Attribute_HealthMultiplier`: an entry named `Attribute_HealthMultiplier` with value = base = the AI class's
  starting value (or the default 1.0 when the class lists none).
- On a weapon with no `WeaponMaxReloadAnimTime` noun, read the attribute: 999 (its default) and a new entry.
- Read `IsPhaselocked` on a Phaselocked enemy pawn and on a free one; check its `ReplicatedNouns` entry and
  `NumReplicatedNouns` (strategy 2).
- On a context that is not a provider (the player controller), any noun attribute whose context resolves there
  reads 0.
