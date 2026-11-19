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
  `NounAttributeState`: `GearboxMind`, `WillowPawn`, `WillowPlayerReplicationInfo`, `WillowWeapon`. Only `WillowPawn`
  and `WillowPlayerReplicationInfo` also declare a fixed-size `ReplicatedNouns` array of
  `ReplicatedNounAttributeState` (20 entries) and an int `NumReplicatedNouns` (the export tables; section 10.2).
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

**Which store** each case uses is settled in **section 10.5's storage-selection table**, which puts the net mode
together with the arrays each provider hands over (section 10.2). The rows that used to stand here are replaced by it.
In short:
- no outer, or not a provider: no store;
- strategy 0: `NounState` when the provider hands it over;
- strategies 1 and 2: `NounState` mirrored into `ReplicatedNouns`, or `ReplicatedNouns` read alone, or no store,
  depending on the arrays and the net mode.

A provider that hands over no `NounState` array behaves as "not a provider" for strategy 0. **Which arrays each
provider hands over is read in section 10.2:**
- pawns and the replication info hand over `NounState` on authority only, and their `ReplicatedNouns` always;
- `GearboxMind` hands over `NounState` on authority only, and no replicated array;
- `WillowWeapon` hands over `NounState` always, and no replicated array.

Every installed mind and weapon noun has strategy 0, so the data never meets the row "provider without a replicated
array".

**A created entry** holds: `NounName` = the key, `Value` = `BaseValue` = `d`, an empty `ModifierStack`,
`ReplicationStrategy` = the resolver's, and `IndexInReplicationArray` = the replicated slot it got, or -1 when it got
none (strategy 0, no replicated array, or the array was full, which logs an error). A slot it gets is the next free
one (index `NumReplicatedNouns`, which then grows by one), filled with the key, `d`, and `bOnlyRelevantToOwner` true
exactly for strategy 1 (`NOUNREP_RepToOwnerOnly`); the provider is then told its replicated copy changed. That call
sets the actor's `bNetDirty` (section 10.3, which also gives the order: the slot comes before the `NounState`
append).

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

- `ParityMode::Off` (lane FIX-NOUN-OFF, decision 2026-11-16-43, `ow-attr`'s `value_resolvers/noun.rs`):
  - strategy 0 is implemented as sections 3 to 5 say, on any context whose class declares `NounState`;
  - strategies 1 and 2 on a provider answer `NotYet { NotPorted }`, "NounAttributeValueResolver with
    ReplicationStrategy <n> on a provider: whether the provider hands over a replicated array, and what the
    replicated-copy-changed call does, are not read";
  - a write to an entry that holds a replicated slot answers `NotYet { NotPorted }`, "NounAttributeValueResolver write
    to an entry with replicated slot <i>: the mirror needs the provider's replicated array and its change call, which
    are not read".

  Section 10.5 says what replaces those two refusals.
- `ParityMode::Reference`: the C++ reading (`src/progression.cpp` `nounValue`). It returns the `BaseValue` of the
  context's `AIClass.AttributeStartingValues` entry for the attribute, else `DefaultValue`, both evaluated on the
  context.
  - No noun is written, and the writes answer `NotYet` (`NotPorted`), because the C++ has none.
  - The attribute is taken as the resolver's outer. This stands in for the starting values having been applied
    through set-base.

## 8. Implementer checklist (Off mode)

1. Key = the resolver's outer's name; no outer: reads 0.0, writes false.
2. Provider = the context implements `INounAttributeProvider` (in the installed reflection, the same as "its class
   declares `NounState`"); not a provider: reads 0.0, writes false.
3. Find by `NounName` in `NounState` order; create on a miss for reads and writes (section 4, with its two
   orderings).
4. Get = `Value`, get-base = `BaseValue`, ignoring the previous chain value.
5. Add (dedupe), remove (all copies), set-base, each followed by the recompute and the mirror; true on success.
6. **The net mode and the replicated copy:** section 10.5's table. The replicated copy matters on the slice too:
   `IsPhaselocked` (strategy 2) on a pawn creates a replicated slot and sets `bNetDirty` (section 10.3).
7. `ow-attr` has a noun form of `Storage` (`Storage::Noun`, FIX-NOUN-OFF); the mirror of section 10.3 writes the slot
   and `bNetDirty` beside it.
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

## 10. The providers' arrays and the replicated copy (lane ANALYST-NOUN-REPLICATION, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Analyst lane ANALYST-NOUN-REPLICATION (decision 2026-11-19-NNN).
- **Sources:**
  - the game executable, read with Ghidra (`tools/ghidra/run.ps1` read-only queries and the local table reader, one
    headless run at a time under the machine lock);
  - field names from `tools/ghidra/class_layout.py` and the export tables.
- Raw output stays under ignored `local/analysis/nounrep/`. There is no bl-sdk session and no recorded live evidence
  for these rules, so **every rule here is UNVERIFIED in the running game**. This lane implements nothing.

This section answers section 4's two "not read" points: which arrays each provider hands over, and what "the
provider is told its replicated copy changed" does.

### 10.1 What the resolver asks a provider

The noun resolver asks the provider interface two things, both answered by each provider class's own implementation:
1. **Its arrays.** It hands back four things:
   - its `NounState` array, or none;
   - its `ReplicatedNouns` array, or none;
   - where its replicated count lives;
   - the replicated array's capacity.
2. **"The replicated copy changed."** A call with no arguments and no answer.

### 10.2 Each provider's answer

The provider implementations were found through each class's registration and constructor, by the interface's table
in each class. `WillowAIPawn` and `WillowPlayerPawn` hold the same two functions as `WillowPawn`.

| Provider class | `NounState` handed over | `ReplicatedNouns` handed over | Count / capacity | "Replicated copy changed" |
|---|---|---|---|---|
| `WillowPawn`, `WillowAIPawn`, `WillowPlayerPawn` | only when the pawn's `Role` is 3 (`ROLE_Authority`) | always | `NumReplicatedNouns` / 20 | sets the pawn's `bNetDirty` |
| `WillowPlayerReplicationInfo` | only when its `Role` is 3 | always | `NumReplicatedNouns` / 20 | sets its `bNetDirty` |
| `GearboxMind` | only when its `Role` is 3 | none | none / 0 | does nothing |
| `WillowWeapon` | always | none | none / 0 | sets the weapon's `bNetDirty` (never reached: there is no replicated array) |

**Corrections to sections 1 and 4.**
- `WillowWeapon` declares no `ReplicatedNouns` and no `NumReplicatedNouns` (the export table). Only `WillowPawn` and
  `WillowPlayerReplicationInfo` do, so the weapon is "without a replicated array".
- Three providers hand over `NounState` only on authority. Off the authority role, a pawn, a mind or a replication
  info is a provider with no `NounState` (section 10.4).

**The names.** `Role` and `bNetDirty` are the `Actor` fields the layout tool places exactly. The pawn's three noun
fields are named by fit with `NumReplicatedNouns`, `NounState` and `ReplicatedNouns`, in that order: high confidence,
UNVERIFIED by name.
The capacity 20 is the declared size of `ReplicatedNouns[20]`.

**The slice's pawn.** `GD_Siren_Skills.Attributes.IsPhaselocked` (strategy 2) resolves its context to a pawn. In the
slice that is a `WillowAIPawn` (the cast's target) or Maya's `WillowPlayerPawn`; both take the first row. So the row
"provider with a replicated array" of section 4 applies to strategy 2 there, **when the pawn's `Role` is 3**.

### 10.3 Creating an entry with a replicated slot, in order

On a provider that hands over both arrays (a pawn or a replication info on authority), with strategy 1 or 2, and no
entry for the key:
1. **`d` is evaluated** (section 4: before anything is appended; it may itself create other entries).
2. **The replicated slot**, when `NumReplicatedNouns` is less than 20:
   1. `NumReplicatedNouns` grows by one; the slot is the old count;
   2. that slot of `ReplicatedNouns` gets `NounName` = the key, `Value` = `d`, and `bOnlyRelevantToOwner` = true
      exactly for strategy 1;
   3. the provider is told its replicated copy changed: the actor's `bNetDirty` is set true.

   When the count is already 20, there is no slot: an error naming the provider and the key is logged ("too many
   replicated nouns"), and nothing above is written.
3. **The `NounState` entry** is appended: `NounName` = the key, `Value` = `BaseValue` = `d`, an empty
   `ModifierStack`, `ReplicationStrategy` = the resolver's, and `IndexInReplicationArray` = the slot, or -1 when there
   was none.

So the slot and `bNetDirty` are written **before** the `NounState` entry.

**The mirror** (section 5, after an add, a remove or a set-base on an entry with a slot): the slot's `Value` becomes
the entry's new `Value`, then `bNetDirty` is set true. No other notification is sent, and nothing else is written by
the replicated-copy call.

### 10.4 A provider whose `NounState` is not handed over

A pawn, a mind or a replication info whose `Role` is not 3:
- **Strategy 0:** no store. Reads give 0.0, writes give false, nothing is created.
- **Strategy 1 or 2** (a pawn or a replication info): `ReplicatedNouns` is read alone. Reads give the `Value` of the
  first slot, among the first `NumReplicatedNouns`, whose `NounName` is the key, or 0.0 when none matches; get-base
  gives 0.0. Writes give false, and nothing is created. This holds in any net mode.

On a **client** (the world's net mode is the network-client mode), a strategy 1 or 2 noun on a pawn or a replication
info also reads `ReplicatedNouns` alone, as above, whatever its `Role`. Writes give false.

### 10.5 Proposed `ParityMode::Off` answer (for an implementer)

The hub's rulings apply:
- preflight what is cheap and certain;
- the preflight writes nothing;
- grade by the `VmError` variant and the `NotYet` kind, never by text;
- **Off only; Reference is unchanged.**

**The write signal and the prefix** (hub ruling, review r1 of #516):
- The resolver signals its first committed write through `EvalCx::note_writes()`. That includes a creation during get
  or get-base (an appended entry, a slot, `NumReplicatedNouns` or `bNetDirty`).
- The resolver returns its own errors unprefixed. The caller prefixes only a `NotYet`, with "stopped after partial
  writes: ", keeping its kind and record.
- A preflight failure is raised before any write, so the write flag is left untouched.

**The error boundary** (hub ruling, review r1 of #516): the existing one is kept. A resolver method returns
`AttrError`. Malformed data is `AttrError::BadData { reason }`, which reaches the VM as `VmError::Attr`; it is never
promised as `VmError::Script`.

**The classes read:**
- `WillowPawn` and its read subclasses `WillowAIPawn` and `WillowPlayerPawn`;
- `WillowPlayerReplicationInfo`, `GearboxMind` and `WillowWeapon`;
- each by its exact class.

Any other class whose class declares `NounState` (for example `WillowMind`, which the combat port instantiates and
the noun context resolver uses) is an **unread class**.

**The storage-selection table.** `NS` is "the provider hands over `NounState`" and `RN` is "it hands over
`ReplicatedNouns`" (section 10.2). For a read class, `NS` holds unless the class's row needs `Role` 3 and `Role` is
not 3. For an unread class, `NS` is "the class declares `NounState`", today's rule, and `RN` is not known.

| Case | Strategy | `NS` | `RN` | Net mode | Store | Reads (get / get-base) | Writes (add, remove, set-base) |
|---|---|---|---|---|---|---|---|
| no resolver outer, or a context that is not a provider | any | | | any | none (today's) | 0.0 / 0.0 | false |
| read or unread class | 0 | yes | any | any | `NounState`, created on a miss (today's path) | entry `Value` / `BaseValue` | applied to the entry |
| read class | 0 | no | any | any | none | 0.0 / 0.0 | false |
| pawn or replication info | 1 or 2 | yes | yes | not client | `NounState` mirrored into `ReplicatedNouns`, created on a miss with a slot (section 10.3) | entry `Value` / `BaseValue` | applied to the entry, then mirrored |
| pawn or replication info | 1 or 2 | no, or any on a client | yes | any | `ReplicatedNouns` alone, never created | slot `Value` or 0.0 / 0.0 | false |
| mind or weapon | 1 or 2 | any | no | any | none | 0.0 / 0.0 | false |
| unread class | 1 or 2 | yes | unknown | any | refused | `NotYet { NotRead }` (below) | `NotYet { NotRead }` |

Notes on the table:
- An unread class keeps today's strategy-0 path in full: no `Role` test. Its refusal is scoped to the operations that
  need unread replication behaviour, strategies 1 and 2.
- A write to an existing entry with a slot, on an unread class, is refused the same way.

**Preflight**, before `d` is evaluated or anything is written, in this order. Each failure is returned as it is:
1. **The key and the provider:** no outer, or a context whose class does not declare `NounState`, gives today's
   0.0 / false. Nothing is refused.
2. **The strategy:** outside 0 to 2 stays the existing `NotYet { NotRead }`, "NounAttributeValueResolver with
   ReplicationStrategy <n>, outside the note's table".
3. **An unread class with strategy 1 or 2,** or a write on an unread class to an entry whose `IndexInReplicationArray`
   is not -1: `NotYet { NotRead }`, "NounAttributeValueResolver: the noun arrays a <class path> hands over, and its
   replicated-copy-changed call, are not read (ReplicationStrategy <n>)", record `NATIVE_NOUN_ATTRIBUTES.md`.
4. **The fields of a read class.** The `Role` byte is read (`Role` 3 is the authority role). Then:
   - `Role` must be a byte, and `NounState` an array of `NounAttributeState`;
   - for a pawn or a replication info, `ReplicatedNouns` must be a fixed array of 20 `ReplicatedNounAttributeState`,
     `NumReplicatedNouns` an int, and `bNetDirty` a bool.
5. **The count:** `NumReplicatedNouns` must lie between 0 and 20.
6. **An existing entry's slot** (writes, and reads of an entry), before any mutation:
   - `IndexInReplicationArray` must be an int;
   - it must be -1, or within the occupied slots `0 .. NumReplicatedNouns - 1`;
   - the strategy 1 and 2 rows of a read class with `RN` need it there for the mirror;
   - for the mind and weapon rows, any value other than -1 is malformed, since they hand over no replicated array.

**The diagnostics**, each an `AttrError::BadData` with this `reason`:
- "NounAttributeValueResolver: <class path> has no <field>" (a missing declaration);
- "NounAttributeValueResolver: <class path>.<field> is not <the declaration of step 4>" (a mistyped one);
- "NounAttributeValueResolver: <object path> holds NumReplicatedNouns <n>, outside 0..=20";
- "NounAttributeValueResolver: the entry <key> on <object path> holds IndexInReplicationArray <i>, outside -1 and the
  occupied slots 0..<n>" (with `n` = `NumReplicatedNouns`, or 0 for the mind and weapon rows);
- the existing "IndexInReplicationArray holds a <kind>, not an int" stays.

**Creation with a slot,** in order, as section 10.3 says:
1. `d` is evaluated. It may create other entries, take slots and set `bNetDirty`.
2. **`NumReplicatedNouns` is read again**, after `d`. A nested creation may have taken the next slot or filled the
   array, so the slot is the count as it stands now.
   - If the count is below 20: the count grows by one, the slot gets `NounName`, `Value` = `d` and
     `bOnlyRelevantToOwner`, and `bNetDirty` is set true.
   - If it is 20: no slot, nothing written for it, and slot -1.
3. The `NounState` entry is appended.
4. `EvalCx::note_writes()` is called at the first of these writes.

**A successful mirror,** in order:
1. the entry's stack or base is changed;
2. its `Value` is recomputed (section 5);
3. `ReplicatedNouns[slot].Value` = that `Value`;
4. `bNetDirty` is set true.

`note_writes()` is called at the first of these.

**Which refusals each rule replaces:**
- The strategy 1 and 2 `NotYet { NotPorted }` (section 7) goes for every read class. For an unread class, it is
  replaced by preflight step 3's `NotYet { NotRead }`.
- The mirror's `NotYet { NotPorted }` (section 7) goes for every read class, which mirrors as above. For an unread
  class, it is replaced by step 3's `NotYet { NotRead }`.
- The existing "outside the note's table" `NotRead` and the existing `BadData`s stay.

### 10.6 Open (UNVERIFIED)

1. `WillowMind`'s noun provider, and that of any other subclass of the four classes, not compared.
2. The error log's exact text and channel in the "too many" case. It is not needed.
3. What `bNetDirty` causes beyond marking the actor for replication. The script and the natives were not searched for
   its readers in this lane.

### 10.7 Live checks (bl-sdk, the RTSE way)

1. Phaselock an enemy, then read its `NumReplicatedNouns`, `ReplicatedNouns[0]` and `NounState`. Expect one slot named
   `IsPhaselocked` with `bOnlyRelevantToOwner` false, and an entry whose `IndexInReplicationArray` is that slot.
2. Read a weapon's `NounState` after reading `WeaponMaxReloadAnimTime`. Expect an entry with slot -1.
