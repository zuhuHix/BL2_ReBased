## 2026-11-16: Rust behavior kernel natives: one pack, the shared collector and the consumer table

AI-assisted (Claude Code). Lane FIX-KERNEL-NATIVES of the Rust port (a foundation gap). Number 50 was reserved for this
lane by the hub. It adds `crates/ow-behavior/src/kernel/natives.rs` (and its `natives/tests.rs`), one `pub mod` line and
the `Provider::consumer` accessor in `crates/ow-behavior/src/kernel.rs`, and the granted seam edits in
`crates/ow-mission/src/bridge.rs` and `crates/ow-combat/src/{hit,death}.rs`, with their test modules. There was no
real-game session: every rule below is read from the records and the C++ reference and is **UNVERIFIED** in the running
game. No parsing behaviour changes.

- **The problem.** The behavior kernel's script-facing natives had no owner. The mission bridge bound
  `ActivateBehaviorOutputLink`, `IsBehaviorSequenceEnabled`, `ActivateBehaviorEventFromScript` and
  `WillowPawn.GetBehaviorConsumerHandle` with `register_native`, which refuses a key that is already bound. The combat
  pack bound `ActivateBehaviorOutputLink`, `BroadcastBehaviorEventFromScript` and `GetBehaviorConsumerHandle` through
  `hit::bind`, which skips one. On one VM the order decided the outcome. Combat first: the bridge failed when the
  director was placed (`duplicate native registration`). Bridge first: combat's script behaviors' output links went
  to the bridge's list, so combat read empty lists. Nothing bound `RegisterBehaviorConsumer`. (Read from the code; no
  CLI command puts the two on one VM yet, so this was not a visible failure.)

- **Inventory** (the nine code packages, `research/script_disasm.py` over every script function; 10 functions do not
  decode, so the counts are lower bounds; the counts are generated locally, under ignored `local/`). `BehaviorKernel`
  declares 28 natives. 18 of them have script call sites, 223 direct calls in all. `GetBehaviorConsumerHandle`
  (`IBehaviorConsumer`) has 21 interface calls.

  | Native | Calls | C++ reference | Here |
  |---|---|---|---|
  | `ActivateBehaviorOutputLink` | 86 | `mission_script.cpp`, `combat_death.cpp` | the shared collector, both modes |
  | `ActivateBehaviorEventFromScript` | 24 | `mission_script.cpp` | routed by the handle's owner, both modes |
  | `IntializeBehaviorProviderForConsumer` | 23 | as `BehaviorProvider::registerConsumer` (pass 1), not as a native | Off: the kernel's own processes |
  | `ChangeBehaviorSequenceActivationStatus` | 15 | as `setSequenceEnabled` (the slice's handler), not as a native | Off: the kernel's own processes |
  | `PublishFloatOutputVariable` / `Object` / `Vector` / `Int` / `Bool` | 13 / 12 / 5 / 4 / 2 | no | not bound |
  | `RegisterBehaviorConsumer` | 7 | no | Off: the kernel's own processes |
  | `BeginNondeterministicProviderRegistration` | 7 | no | not bound |
  | `ProcessReplicatedBehaviorConsumerState` / `ProcessReplicatedBehaviorEvent` | 7 / 7 | no | not bound |
  | `BroadcastBehaviorEventFromScript` | 4 | `combat_death.cpp` | routed, both modes |
  | `ForceUnregisterBehaviorConsumer` | 3 | no | not bound |
  | `IsBehaviorSequenceEnabled` | 2 | `mission_script.cpp` | routed, both modes |
  | `RemoveBehaviorProviderFromConsumer`, `ChangeBehaviorConsumerSuspensionStatus` | 1 each | no | not bound |
  | 10 others (`IsBehaviorKernelWatchingConsumer` and the debug listers) | 0 | no | not bound |
  | `WillowPawn.GetBehaviorConsumerHandle` | 21 (interface) | `mission_script.cpp` (the director only), `combat_death.cpp` | Off: a plain field read; Reference: routed |

  "Not bound" means the VM's stub policy answers the call, as before this lane. With `LogAndZero`, that is the
  `UNIMPLEMENTED <key>` line and a zero value; with `Refuse`, it is `UnboundNative`. No record reads those rules.

- **Interface** (`ow_behavior::kernel::natives`).
  - `Pack` (a `NativePack`) is the only binder of these keys. `ow-combat`'s `hit::Pack` and the mission bridge's use
    chain register it themselves. A second registration only binds keys declared since the first, and a key that
    another pack bound first is refused.
  - `collect(vm, run) -> (T, Vec<i32>)`: runs one script behavior and returns the output ids its script selected,
    in call order with duplicates kept. Runs nest. Without the pack it is `NotYet { LayerAbsent }`.
  - The consumer table. `claim(vm, pid, owner)` and `allocate(vm, owner, from)` let a crate own the handles it gives
    out. A handle another owner holds is refused, and so is the invalid -1. `route(vm, owner, native, body)` gives
    the owner's body for one of the five `ROUTED` natives. `owner_of`, `process_of` and `with_provider` read the
    table and lend out a kernel provider.
  - `Provider::consumer()` / `set_consumer()`: the additive route of decision 2026-11-15-30. A handler can read the
    consumer without a change to `HandlerFn`.

- **Routing.** A routed native goes to the owner of its handle's `PID`.
  - A crate's handle runs that crate's body. If that crate gave no body for the native, the call gets the VM's unbound
    behaviour (the stub policy). On a VM that holds that crate alone, this is what the call got before the pack. Where
    another crate had bound the native, the call used to reach that crate's body.
  - A kernel handle runs the kernel rule.
  - A handle nobody owns (the invalid -1 among them):
    - **Off:** answers false and changes nothing (NATIVE_BEHAVIOR_CONTEXT.md; NATIVE_BEHAVIOR_POPULATION.md section C).
    - **Reference (review r1):** gets what the C++ does. That is the body of the one crate that binds the native in the
      C++, given with `natives::fallback`: `ow-mission` for `IsBehaviorSequenceEnabled` and
      `ActivateBehaviorEventFromScript`, `ow-combat` for `BroadcastBehaviorEventFromScript`, whose note is written
      for every `PID`, as `combat_death.cpp` writes it. When no crate binds it, the stub policy answers. Two crates'
      fallbacks for one native have no C++ VM behind them: `NotYet { NotPorted }`.
  - A None provider given to `ActivateBehaviorEventFromScript` fires nothing, whoever owns the handle (Off;
    NATIVE_MARCUS_USE_CHAIN.md). In Reference the body the call reaches decides, as in the C++.

- **The kernel rules, Off only.** Reference binds only what the C++ binds; these three stay unbound there.
  - `RegisterBehaviorConsumer`: a None consumer gets the handle -1. Otherwise the kernel makes a new process and
    claims the lowest free non-negative `PID`. **Fitted, UNVERIFIED:** the record reads "a non-negative slot index",
    the first "probably 0"; the allocator itself is not read. Registering the same consumer twice is `NotYet`.
  - `IntializeBehaviorProviderForConsumer` (section C): decodes the provider, sets its consumer and runs pass 1
    (`Provider::register_consumer`). Four cases are refused with `NotYet`: a None provider (not read; `Skill.Activate`
    passes its definition's provider unchecked), a provider registered twice on one consumer (not read), and a
    sequence with a `CustomEnableCondition`. That last one is pass 2, which the condition's owner links (the mission
    tracker), and it is refused before any effect. A handle with no process changes nothing.
  - `ChangeBehaviorSequenceActivationStatus` (section C): Enable, Disable or Toggle through
    `Provider::set_sequence_enabled` (the mutex and the event order are the kernel's). The action is named from the
    declaration's enum, never by position. A None provider is refused with `NotYet` (the record: "the lookup was not
    read"), and so is a provider the consumer has not registered.
  - `IsBehaviorSequenceEnabled` / `ActivateBehaviorEventFromScript` / `BroadcastBehaviorEventFromScript` on a kernel
    process: the registered provider's enabled bit (false for one not registered); the event fired on that provider
    (an omitted filter is -1; given `Parameters` are `NotYet`); every provider of the process, in registration order.
  - A native that reaches a kernel provider while one of its behaviors is running is `NotYet`. Re-entrant use is not
    modelled.

- **The seam edits.**
  - `ow-mission`: the bridge's own `selected` list and its `n_output_link` are gone. The use chain allocates the
    director's `PID` in the table as `"ow-mission"`: the lowest free one from 0, kept on the bridge. Its sequence check
    compares against that `PID`, so a consumer the kernel registered first keeps its own (review r1). The bridge gives
    its three bodies, which are unchanged, as routes and as Reference fallbacks: the sequence query, the event, and the
    director-only `GetBehaviorConsumerHandle`. Its script behaviors collect their links through `collect`. In
    Reference no kernel consumer exists, so the director still gets 0, as in the C++.
  - `ow-combat`: `CombatState::selected` and its bindings of `ActivateBehaviorOutputLink` and
    `GetBehaviorConsumerHandle` are gone. `prepare_for_death` allocates handles through the table as `"ow-combat"`
    (from 100, as before). The broadcast body and the handle read are routes and Reference fallbacks. **One edit outside the grant:** `run.rs`'s
    `run_script_behavior`, the reader of combat's old list, now wraps the run in `collect` (same calls, same order).
  - `GetBehaviorConsumerHandle`:
    - **Off:** follows the record for every object, a plain read of its `ConsumerHandle`. A class without the field
      is an error (`... has no property ConsumerHandle`).
    - **Reference:** routed by the `PID` in the object's own `ConsumerHandle`, so it reaches the C++ bridge's
      director-only body or combat's read (review r1).
  - **Off only:** a routed call on a handle nobody owns no longer writes combat's
    `BroadcastBehaviorEventFromScript <name>` note. Reference writes it, as the C++ does.
  - A failed pass 1 of `IntializeBehaviorProviderForConsumer` removes the provider's slot, so the call can be made
    again (review r1).

- **How `ow-skills` should use it** (W6-SKILLS landed on main without an output-link binding: its two skill behaviors
  select no link). If a skill behavior that selects links is added, wrap its `ApplyBehaviorToContext` calls in
  `natives::collect` rather than binding the native. `Skill.Activate` reaches `RegisterBehaviorConsumer` and
  `IntializeBehaviorProviderForConsumer`, which the kernel answers in Off. The skill's provider is then a kernel
  provider: its handlers can read the skill through `Provider::consumer()` instead of being set per provider, and
  `natives::with_provider` reaches it. The next call of `Skill.Activate`, `BeginNondeterministicProviderRegistration`,
  is unbound (rule not read), so under `Refuse` the cast stops there with `UnboundNative`. Do not bind any of these
  keys in `ow-skills`.

- **Verified (automated, local, pc-hummer).**
  - 13 new `ow-behavior` tests on invented declarations and objects (`natives/tests.rs`), both modes where both bind:
    the collector, idempotent registration, a refused second binder, late declaration, Reference's bindings, the
    plain handle read, routing in both setup orders, the stub fallback under both policies, exclusive claims, the
    None-provider rule, `RegisterBehaviorConsumer`, a kernel provider's events, sequences and broadcast with its
    consumer, and every `NotYet` above.
  - The order test (`ow-mission` `bridge/tests.rs`): the world handlers with the kernel pack, the combat pack and the
    bridge's use chain, registered in all 6 orders, in both modes, give one world. That world holds the handle, the
    combat events, the director's route, the collected links and the owners. The bridge's toy packages gained the
    declarations this needs. With the old bindings, read from the code and not run: the three orders that register
    combat before the bridge fail at registration, and the other three send combat's links to the bridge's list.
  - Before / after on invented packages. `tests/behavior_test.py`, `tests/combat_test.py` and
    `tests/mission_script_test.py` were run with every run command replayed on the Rust release binary of main
    2a0666d and of this branch, in Off and Reference; the tests themselves got the C++ answers and passed. The local
    harness is in a scratch folder. All 188 run commands give the same stdout and exit code: 18 `--behavior-run`,
    21 `--combat-run`, 35 `--mission-run` and 20 `--slice-run`, each in both modes. The `milliseconds` and
    `elapsed_ms` timing fields were masked.

- **Open items.**
  - Analyst questions (clean room; this lane implemented from the records, so it read no listing for them):
    `RegisterBehaviorConsumer`'s allocator (its base, reuse, a second registration);
    `IntializeBehaviorProviderForConsumer` with a None provider or a provider registered twice;
    `ChangeBehaviorSequenceActivationStatus`'s provider lookup (None, unregistered); `ActivateBehaviorEventFromScript`'s
    `Parameters`; and the unbound natives above, `BeginNondeterministicProviderRegistration` and
    `ForceUnregisterBehaviorConsumer` first, because `Skill.Activate` and `Skill.Deactivate` call them.
  - Pass 2 for a kernel provider. Linking a `CustomEnableCondition` belongs to the condition's owner, and a route or
    hook for it is a follow-up.
  - `GetBehaviorContext` and `ResolveBehaviorProviderDefinitionReference` stay in the mission bridge. No other pack
    binds them, so they do not clash; moving them is a follow-up.
  - The VM census counts a routed call that falls back to the stub as a native call. The stub log line is the same.
- **UNVERIFIED.** Every rule above. None was checked in the running game.
