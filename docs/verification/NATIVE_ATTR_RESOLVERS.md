# Native attribute resolvers: Currency, Random, AmmoDropWeight and Constraint (draft, 2026-11-16)

AI-assisted (Claude Code), analyst lane FIX-ATTR-RESOLVERS. Read from the game executable with Ghidra 12.1.4
(`tools/ghidra/run.ps1`, read-only queries of each class's get and get-base slots against the project made by
`tools/ghidra_import.ps1`), written in our own words; no decompiler output, pseudo-code or listing structure is
reproduced here, and the raw output stays outside the repository. Field names and types come from the cooked packages
(the export tables, `tools/ghidra/class_layout.py`). **Every rule is UNVERIFIED in the running game**; nothing here
was observed live.

This note is the hand-off to the implementer of the four `ParityMode::Off` rules. The analyst lane implemented only
the C++ readings (`ParityMode::Reference`, decision 2026-11-16-73); the Off rules are to be built by a fresh
implementer from this note alone (docs/LANE_BRIEF.md, "Clean room (native code)"). Section 6 lists what to build and
where the C++ stand-ins differ.

## 1. What the four classes share

- Each extends the value resolver base class, has no script function and registers no native. Its rules are its
  answers to the value resolver's operations (NATIVE_ATTRIBUTES.md section 6).
- **get-base** is the same base-class function for all four: it answers what get answers. Add, remove and set-base
  were not read for these classes; the base class's default (they answer false and change nothing) is the reading
  NATIVE_ATTRIBUTES.md section 6 gives for every value resolver but the object-property one.
- Every initialization-data field below is evaluated with `EvaluateInitializationData` on the context the attribute's
  context chain gave (the resolver's context argument), with no override source.

## 2. `WillowGame.CurrencyAttributeValueResolver`

Field: `FormOfCurrency`, a byte of `ECurrencyType`.

| The context | get answers |
|---|---|
| none | 0.0 |
| an object that is not of the replication-info class the cast names (read as `WillowPlayerReplicationInfo`, medium confidence: the cast's class is found by its package, and the call it guards is the replication info's currency query) | 0.0 |
| a `WillowPlayerReplicationInfo` (or a subclass) | that object's `GetCurrencyOnHand(FormOfCurrency)`, the same native rule the script call reaches (NATIVE_CONTROLLER_HELPERS.md: 0 for a type above 12, the owner controller's key count for golden keys, else the slot's `CurrentAmount`), the int converted to a float |

- The resolver does **not** move from a controller to its replication info. A controller context answers 0.0. Whether
  an attribute reaches the replication info is its context chain's business (for example a
  `PlayerReplicationInfoAttributeContextResolver` in the chain; the installed `TeleportCost` data is to be checked
  by the implementer, not assumed).

## 3. `GearboxFramework.RandomAttributeValueResolver`

Fields: `bUseIntegerRandomization`, `LowerBound`, `UpperBound` and `ValueLifetime` (initialization data), and
`SavedRandomValues`, an array of `SavedRandomValue` (`ContextName`, a name; `SavedValue`; `ExpirationTime`).

**The draw** (one call of the C runtime `rand()`, after both bounds are evaluated, lower first):

| `bUseIntegerRandomization` | Result |
|---|---|
| false | `low + rand() / 32767 * (high - low)`, single precision, so both ends are reachable |
| true | `span = trunc(high) + 1 - trunc(low)`; when `span` is below 1 the result is `trunc(low)` and no number is drawn; else `trunc(low) + trunc(rand() / 32768 * span)` |

`trunc` is the float-to-int conversion toward zero. The integer range is `trunc(low)` to `trunc(high)` inclusive.

**The lifetime.** `ValueLifetime` is evaluated first.

| Lifetime | get answers |
|---|---|
| at most 0.0001 | a fresh draw; nothing is saved |
| above 0.0001 | the saved value for the context, when one is still alive; else a fresh draw, saved for the context |

- **The key** is the context's object name (the None name for no context).
- **The clock** is the current world's `TimeSeconds`, or 0 with no world.
- **The saved list** is walked in order. An entry with `ExpirationTime` before the clock is removed as it is met,
  and the walk goes on at the same place. A live entry whose name equals the key answers its `SavedValue`, and the
  walk stops there; entries after it are not looked at, so expired ones after it stay.
- **With no live match**, a fresh draw is appended (name: the key; value: the draw; expiration: the clock plus the lifetime) and answered.

## 4. `WillowGame.AmmoDropWeightAttributeValueResolver`

Fields: `Resource` (a `ResourceDefinition`), and `ResourceThreshold`, `AboveThresholdWeight`, `NoPoolWeight`,
`MinBelowThresholdWeight` and `MaxBelowThresholdWeight` (initialization data).

**What is evaluated, and when.**
- `ResourceThreshold` is always evaluated first.
- With a `Resource`, `MinBelowThresholdWeight` and `MaxBelowThresholdWeight` are evaluated next, before any pool is
  looked at.
- `AboveThresholdWeight` and `NoPoolWeight` are evaluated only when they are the answer.

**The pools looked at.** Every controller of the current world's controller list, in list order (all controllers,
not only the local player's). For each:
- a controller virtual, not identified, gives the controller whose resource-pool provider interface is asked. With
  none, the controller is skipped.
- That provider is asked for its pool of `Resource`, as a pool reference. A reference that does not resolve (no
  manager, an index past the 16 slots, or a slot whose pool's `PoolGUID` differs) skips the controller.
- A resolved pool still flagged `bCreatedAndNotModified` is skipped. On a network client the flag is first cleared
  when the pool's `LastCurrentValue` differs from its `CurrentValue`.
- Every other pool **counts**.

**Each counting pool.** Let `M = MaxValue * threshold` (the pool's `MaxValue`, not its base value), `m` = the pool's
`MinValue`, `c` = its `CurrentValue`, and `f = (M - c) / (M - m)`.

| The pool | What it adds |
|---|---|
| `M - m` is not above 0, or `c` is below 0, or `c` is above `M` | nothing (it counts, but is not below the threshold) |
| otherwise, `f` below 0 | nothing (not reached in practice: `c <= M` makes `f` at least 0) |
| otherwise | `Min + (Max - Min) * min(f, 1)` to the sum, and the answer will be the sum |

So a pool at the threshold adds the minimum weight, and an empty pool (at its `MinValue`) the maximum.

**The answer.**

| Case | get answers |
|---|---|
| no `Resource`, no world, an empty controller list, or no pool counted | `NoPoolWeight` |
| pools counted, none of them adding | `AboveThresholdWeight` |
| at least one pool added | the sum of what the adding pools added (one term per controller) |

Single precision throughout.

## 5. `GearboxFramework.ConstraintAttributeValueResolver`

Field: `Constraints`, a `Range` (`bEnableMinValueRestriction` and `MinValue`, `bEnableMaxValueRestriction` and
`MaxValue`; the bounds are initialization data).

get takes the running value of the chain (the earlier resolver's result, 0.0 for the first) and:

| Range | get answers |
|---|---|
| neither bound enabled | the running value, unchanged; nothing is evaluated |
| the minimum enabled | `MinValue` is evaluated; a running value at or below it becomes it |
| then the maximum enabled | `MaxValue` is evaluated; a value above it becomes it |

So the minimum is applied first and the maximum last, so a minimum above the maximum gives the maximum. A bound is
evaluated only when it is enabled. This is the clamp the definition's range restriction applies (the same shared
routine), with the resolver's own range.

## 6. For the implementer

- **The four Off rules, from sections 2 to 5.**
  - Currency needs the replication info's `GetCurrencyOnHand`, which `ow-combat` binds (`ow_combat::down`). From
    `ow-attr` it is a native call, so the seam is the implementer's choice: an engine query, or the VM's.
  - Random needs the world clock and per-resolver saved values on the resolver object (it writes its own
    `SavedRandomValues`).
  - AmmoDropWeight needs the world's controller list and each controller's pool for a resource. That is a pool lookup
    unlike `EngineQuery::ResourcePool`, which goes through the resource's context resolver.
  - Constraint needs nothing new: the range clamp of the evaluator on the chain's running value.
- **Where the C++ stand-ins (`src/progression.cpp`, Reference) differ, so the Off tests should not copy them:**
  - Currency: the stand-in moves from a controller to its replication info; the native does not.
  - Random:
    - the stand-in's real draw divides by 32768 (the top end is never reached), and its integer range is `ceil(low)`
      to `floor(high)`, against the native's truncation;
    - the stand-in saves nothing.
  - AmmoDropWeight:
    - the stand-in reads the local player's health pool only;
    - it compares the pool's share of its maximum with the threshold and interpolates on the share;
    - it ignores `MinValue`, the never-modified flag and the other controllers;
    - it does not sum.
  - Constraint: the stand-in's rule is the native's (section 5); only the Off route through the note is missing.
- **Open** (not read):
  - the controller virtual that gives the pool provider;
  - which provider method is asked (read as "the pool for this resource");
  - add, remove and set-base of the four classes;
  - the cast class of Currency, beyond the medium reading above.
- **bl-sdk reads that would confirm the rules** (none run):
  - `TeleportCost` evaluated with the controller and with its replication info as the source;
  - the adult bullymong's `ItemGameStageOffset` over many spawns, both ends;
  - `SavedRandomValues` on a resolver with a lifetime before and after a second read;
  - a health drop weight read at a few health levels of the player's, with a second player present and absent.
