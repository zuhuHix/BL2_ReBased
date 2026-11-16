## 2026-11-16: The four attribute value resolvers' Off rules from the analyst note

AI-assisted (Codex/GPT-6.1 Sol). Lane FIX-ATTR-RESOLVERS-OFF: the fresh implementer of the clean-room pair begun by
FIX-ATTR-RESOLVERS (decision 2026-11-16-73, PR #372). The only input for the native rules was
`docs/verification/NATIVE_ATTR_RESOLVERS.md`; this lane opened no analysis output, Ghidra project or REA result. The unpublished child was rebased onto main after #372 merged; only the child commit was replayed. There was no real-game session: every rule below is **UNVERIFIED** in the running game.

**What changes** in `ParityMode::Off`. Reference keeps decision 73's stand-ins, and its installed rows are unchanged
(below).

- **Constraint** (note section 5). The native's clamp is the stand-in's, so one rule now serves both modes:
  - the minimum first, then the maximum;
  - a bound is evaluated only when it is enabled.
- **Currency** (note section 2).
  - A context that is not a `WillowGame.WillowPlayerReplicationInfo` (or a subclass) answers 0. That includes a
    controller: the native does not move to its replication info, unlike the stand-in.
  - On a replication info, the value is its `GetCurrencyOnHand(FormOfCurrency)` as a script call would answer it, the
    int converted to a float.
  - The call goes through a new `AttrWorld::currency_on_hand`. The VM runs it as `ow_combat::hit::call_method` does, so
    the answer is whatever `ow_combat::down` binds. Worlds that run no script answer `NotYet { LayerAbsent("ow-vm") }`.
- **Random** (note section 3).
  - The draw:
    - one 15-bit draw of `CrtStream::AttributeVariance`;
    - real: `low + r / 32767 * (high - low)` in single precision;
    - integer: `trunc(low) + trunc(r / 32768 * span)`, with `span = trunc(high) + 1 - trunc(low)`;
    - a span below 1 gives `trunc(low)`, with no draw.
  - The lifetime: `ValueLifetime` is evaluated first.
    - Above 0.0001, one saved value per context object name lives in the resolver's own `SavedRandomValues`. The list
      is walked in order. Expired entries are removed as they are met, a live match stops the walk, and a miss appends
      a fresh draw expiring at clock + lifetime.
    - The clock is a new `AttrWorld::time_seconds`: the VM's clock, or 0 with no world. The trait default is `NotYet`,
      so a world with no clock does not claim to be "no world". `ow_items::RollWorld` forwards it; it is a read.
  - `trunc` of a bound outside the int range is bad data: what the engine's conversion gives there is not read.
- **AmmoDropWeight** (note section 4).
  - Evaluation order:
    - the threshold first;
    - with no `Resource`, the no-pool weight;
    - with one, the minimum and maximum weights next, before any pool.
  - The pools: every controller of the world's list in order, and each one's pool of the resource. A pool still
    `bCreatedAndNotModified` is skipped.
  - Each counting pool is measured from its `MinValue` against `MaxValue * threshold`, as the note's table gives it.
  - The answer:
    - the sum of the adding pools;
    - with pools counted but none adding, the above-threshold weight;
    - with none counted, the no-pool weight.
  - Two additive engine queries carry the walk: `EngineQuery::WorldController { index }` and
    `EngineQuery::ControllerPool { resource }` (source: the controller). `ow-world`'s `QueryKind` gains both.
  - The network-client clearing of the never-modified flag is not modelled: these worlds are standalone.

**Left `NotYet`, with the question for a later analyst or live lane.** Each is a case the note does not settle:

1. **The controller virtual and the provider method** that give a controller's pool (the note's "Open"). No world
   answers `ControllerPool`, and none models a controller list, so today the walk stops at
   `NotYet { LayerAbsent }` on its first query. Two questions remain:
   - which object of a controller (itself, its pawn, a manager) provides the pool;
   - which method is asked.
2. **The precision of `r / 32768 * span`** for an integer Random. The note gives single precision for the real case
   only. For a span up to 512 the pick is the same in either precision: `r / 32768` is exact, and `r * span` stays
   below 2^24. A wider span is `NotYet { NotRead }`. The question is whether that product is single or double.
3. **When the bounds are evaluated** for a Random with a lifetime. The note puts the bounds' evaluation inside "the
   draw", so this lane evaluates them only when it draws, not on a saved hit. That matters only for bounds that draw
   themselves (random variance in their initialization data). It is a reading of the note, not a fact; a live read of
   `SavedRandomValues` around a second evaluation would settle it.
4. **Which C `rand()` stream** the Random draw shares. `CrtStream::AttributeVariance` is the project's stream for
   attribute randomness; whether the engine's call shares a generator with the item rolls' variance is UNVERIFIED.
5. **The cast class of Currency.** The note reads it as `WillowPlayerReplicationInfo` with medium confidence, and this
   lane uses that reading.
6. **Add, remove and set-base** of the four classes stay `NotYet` in both modes; the note did not read them.

**The installed `TeleportCost` data** (the note asks the implementer to check it):
- The installed `D_Attributes.Currency.CreditsOnHand` (read with `ow-package --properties` and the export table) has
  two context resolvers, a `ControllerAttributeContextResolver` and a `PlayerReplicationInfoAttributeContextResolver`,
  and one `CurrencyAttributeValueResolver`. The chain's element order is not decoded by that read.
- So the respawn fee's chain reaches the replication info through its own context resolver. The native's "a
  controller gives 0" does not, by itself, zero the fee.

**Proof.**

- Synthetic tests (rule 9) in `value_resolvers/attr_resolver_tests.rs`:
  - Currency: the info's call; a controller and a non-info both give 0; the unmodelled world is `NotYet`.
  - Random:
    - real and integer draws against `CrtRand`, including a truncation case where the stand-in's ceil and floor
      differ;
    - an empty span draws nothing; a wide span is `NotYet`;
    - saved values per context name across the clock: kept, expired, removed as met, and the walk stopping at a live
      match;
    - no world gives a clock of 0; a lifetime of 0.0001 or less saves nothing.
  - AmmoDropWeight:
    - an empty list, no pool, and a never-modified pool all give the no-pool weight;
    - the `MinValue` measure;
    - a pool above `M`, and `M - m` not above 0, give the above weight;
    - three controllers summed.
  - Constraint in Off.
  - The Reference tests are unchanged.
- A VM test in `ow-vm`: the clock, and the call refused on a class with no `GetCurrencyOnHand`.
- **The installed W6-LOOT run** (`ow-package --combat-run` over `Sanctuary_Dynamic.upk`, decision 73's step list,
  binaries built from #372's head and from this lane; outputs under ignored `local/`):
  - Reference: 197 rows before, 197 after, identical, with the same 300 `Actor.Destroy` errors.
  - Off: 0 rows before and 0 after, with the same 16 errors. The run stops before any resolver on other lanes' items:
    - the unbound `Actor.LocalPlayerControllers` in the mission script (the `seed` step);
    - the player's health pool, which the bridge builds only in Reference (`maya`, `mayahealth`);
    - the spawn's archetype reference `Default__WillowAIPawn.SpecialMoveComponent0`, a subobject of a class default
      object the store does not load (`spawn`, so every `rollpool` has no pawn);
    - `lootbatch` in Off, which is the installed death script whose draws are not recorded (decision 41).
  - Once those clear, the ammo and health weights will stop at item 1.
