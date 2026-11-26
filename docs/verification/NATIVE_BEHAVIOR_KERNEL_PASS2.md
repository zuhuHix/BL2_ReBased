# The behavior kernel's pass 2: a conditioned sequence at registration (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-BEHAVIOR-KERNEL-PASS2 (decision 2026-11-26-NNN), the analyst half of
the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/pass2/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule here
is UNVERIFIED in the running game.**

**Why.** Once `InitializeRBPhysics` and `GetPhysXLevel` are answered, link 1 is predicted to stop at
`InitializeBehaviorProviders`' pass-2 `LayerAbsent` for `PlayerEvents_Siren` (NATIVE_GETPHYSXLEVEL_AND_AFTER.md section
4, #767). Its sequences carry a `CustomEnableCondition`. This note reads what the kernel's pass 2 does with such a
sequence in the game, and what the port needs to answer it.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **Earlier records**, which this note builds on and re-checks:
  - NATIVE_BEHAVIOR_POPULATION.md, sections A to C (lane G2, read from native code). It gives the observer plumbing,
    the `BehaviorSequenceEnableByMission` verdict and its lifecycle, and the registration order;
  - NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md (#740), the pawn's provider list and gate.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read:
  - `IntializeBehaviorProviderForConsumer`'s exec and its registration routine;
  - the condition class's link and after-link virtuals, each found in `BehaviorSequenceEnableByMission`'s primary
    class table (the NATIVE_COMPONENT_SKIP_FLAG.md section 6 method);
  - the base class's link step;
  - the tracker's register routine.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. All land exactly where the routines read them:
  - the condition's `LinkedMission`, `bInstanced` and the base class's `LinkedBehaviorSequences` and
    `BehaviorKernelInstanceTagForTransientState`;
  - `WorldInfo.GRI` and `WillowGameReplicationInfo.MissionTracker`;
  - the definition's `BehaviorSequences`.
- **Maya's data** (`ow-package --def-dump`, local): `PlayerEvents_Siren`'s provider definition and its four
  conditions.
- **The port** on main 0147a0b: `ow-behavior` (the kernel natives and the pawn's providers) and `ow-mission` (the
  bridge's tracker natives, the Fire slice's condition verdict).

## 2. What the game does

### 2.1 The registration (re-checked)

`IntializeBehaviorProviderForConsumer`'s routine does the following for a provider on a consumer:
1. it makes one state record per sequence, all disabled;
2. **pass 1:** each record whose sequence has `bEnabledOnSpawn` is enabled through the shared enable routine (its
   enabled event fires);
3. **pass 2:** for each of the consumer's records of this provider, in sequence order, whose sequence has a
   `CustomEnableCondition`:
   - it asks the condition to **link** the record;
   - when the link succeeds, it runs the condition's **after-link** step.

Both are virtuals of the condition, so what they do is the condition class's. All of pass 1 comes before any of pass
2. NATIVE_BEHAVIOR_POPULATION.md section C says the same.

### 2.2 `BehaviorSequenceEnableByMission`: link and after-link

**Link:**
- **nothing, and failure,** when the world's net mode is the client's (the value 3). Section B left this "run-mode
  value" unidentified; it is the world's net mode, by fit with the engine's net-mode numbering;
- **nothing, and failure,** when `bInstanced` is set;
- **otherwise the base class's link**, which adds the pair (consumer handle, record) to `LinkedBehaviorSequences`
  unless it is already there, and succeeds:
  - `LinkedBehaviorSequences` is a native-only list (a mirror struct that script cannot read), kept **on the condition
    object**. The condition is a subobject of the shared provider definition, so the list is shared by every consumer
    of that definition;
  - before adding, the list is emptied if the condition's `BehaviorKernelInstanceTagForTransientState` differs from
    the running kernel's tag. Then the tag is updated. A list left from an earlier kernel instance is not reused.

**After-link:**
- nothing on a client;
- otherwise it finds the tracker: the world's `WorldInfo`, its `GRI` as a `WillowGameReplicationInfo`, then that
  object's `MissionTracker`. A missing link anywhere means nothing. The condition never registers, and its records
  stay as pass 1 left them;
- with a tracker, it registers the condition as a mission observer of `LinkedMission`.

### 2.3 The tracker's register (re-checked against section A)

- **nothing** unless the tracker's `Role` is the authority's and `LinkedMission` is set;
- **at once**, the condition's level-load reaction (the interface call, with the tracker and the mission). This is
  **the initial verdict**: there is no separate "initial state" step;
- **then**, the condition is stored in the tracker's per-mission observer list, unless the mission is Complete and not
  `bRepeatable`. If the mission has no observer entry yet, one is added.

### 2.4 The reaction and the verdict (section B, unchanged)

Every reaction, the level-load one included, does the same:
- it recomputes the verdict from the tracker's current state;
- it applies the verdict to **every** record in `LinkedBehaviorSequences`. True gives an enable, false a disable;
- an enable of an enabled record and a disable of a disabled one do nothing. So the enabled and disabled events fire
  only on a real change.

The verdict reads, in order:
1. `LinkedObjective` and its `bRememberItemsWithinObjective`, to choose between one verdict for all records and one
   per record (per-instance objectives);
2. not objective-specific: the mission's status (no record: NotStarted), tested against `MissionStatesToLinkTo`;
3. objective-specific: `LinkedObjective` None gives false. Otherwise the objective's state (Complete, Active or
   NotStarted, by the tracker's objective rules) is tested against `ObjectiveStatesToLinkTo`. While the mission is
   NotStarted or Failed, the objective's state is NotStarted;
4. a true verdict with a non-empty `ObjectiveSetRestrictions` stays true only while one listed set is active.

**The enable and disable hooks:** `bCreateWaypoint` set means a HUD waypoint on enable, removed on disable. Neither
hook vetoes.

### 2.5 When the condition is evaluated

- **At registration**, at once (2.3), when the world has a tracker with authority. It is not deferred;
- **again on every tracker notification** for `LinkedMission`: level load, status change, objective set change, and
  objective update, clear and complete (section A's six kinds);
- **never**, when the world has no tracker at registration. The records keep pass 1's state, and the condition is not
  in the observer list, so later notifications do not reach it. It is not registered later on its own; nothing that
  would retry was found.

### 2.6 `PlayerEvents_Siren`

Its provider definition has five sequences. Each has one `OnPlayerEvent` event, and none is a mutex sequence.

| Sequence | `bEnabledOnSpawn` | Condition (all `BehaviorSequenceEnableByMission`) |
|---|---|---|
| `Ep10_MordecaiChatter` | true | `GD_Episode10.M_Ep10_BirdISTheWord`, not objective-specific, mission states {Active} |
| `Ep11_FactoryBattleCommentary` | false | `GD_Episode11.M_Ep11_LikeATonOf`, objective `SurviveInitiation`, objective states {Active} |
| `Ep11_MortarBattleCommentary` | false | the same mission, objective `DestroyMortarBeacons`, {Active} |
| `Ep17_BrickCommentary` | false | `GD_Episode17.M_Ep17_KillJack`, objective `FindJack`, {Active} |
| `CraterLake_ElevatorDownCheck` | true | none |

Every condition has `bInstanced`, `bCreateWaypoint` and `ObjectiveSetRestrictions` empty or clear, and the class
default states. **At a fresh character's spawn**, the three missions are NotStarted, so all four verdicts are false
(this depends on the save and was not observed). Then:
- **pass 1** enables `Ep10_MordecaiChatter` and `CraterLake_ElevatorDownCheck`;
- **pass 2:**
  - `Ep10_MordecaiChatter` is linked, registered and found false, so it is **disabled**: its disabled event fires
    first, and the sequence has no behavior on that event;
  - the three Ep11/Ep17 sequences are linked, registered and found false. They are already disabled, so nothing
    changes;
- **the writes:**
  - four (handle, record) pairs, one in each condition's list;
  - four observer entries in the tracker, under three missions;
  - one sequence bit cleared.
  
  No VM-visible field of the pawn or the definition changes. The conditions' lists and the tracker's observer list are
  native state.

## 3. The port today, and what each piece needs

| Piece | On main 0147a0b | Status |
|---|---|---|
| Pass 1, the records, the enable routine | the kernel rule (`provider_into`, `Provider::register_consumer`) | covered |
| Enable and disable of a record, events only on a change | `ChangeBehaviorSequenceActivationStatus`' kernel rule | covered |
| Pass 2 | refused before pass 1 (`conditioned`: `LayerAbsent`, "the condition's owner"); the pawn's preflight 7 the same | **to replace** |
| Link: net-mode test, `bInstanced`, the (handle, record) list, the instance tag | none | unread before this note; rule in 2.2 |
| After-link: `WorldInfo` → `GRI` → `MissionTracker` | the bridge's graph holds these objects (`MissionScript`, both routes) | covered (the objects); the walk is new |
| The tracker's register: authority, immediate level-load, the store rule | `RegisterMissionObserver` bound by the bridge for **script** observers: it calls `MissionReactionLevelLoad` by name and always stores | partly: the Complete-and-not-repeatable rule is missing, and a condition's reaction is a native the VM does not bind |
| The verdict's mission status | the bridge's `GetMissionStatus` rule: its own mission's status, any other NotStarted | covered |
| The objective state, the set restrictions | the bridge's `IsMissionObjectiveActive` / `IsMissionObjectiveComplete` / `IsObjectiveSetActive` rules; the Fire slice's `condition_holds` (`ow-mission` `slice.rs`) computes this same verdict for the dummy | covered (for the bridge's own mission) |
| Per-instance objectives (`bRememberItemsWithinObjective`) | none | **unread** (the key derivation) |
| Waypoint hooks (`bCreateWaypoint`) | none | presentation, not read further |
| Later notifications reaching a kernel-registered condition | the bridge notifies by script call | not needed for the slice (2.6's missions never change there) |
| `IntializeBehaviorProviderForConsumer`'s own gate: an empty `BehaviorSequences` registers nothing | the pawn's list applies it; the **direct native does not** (it registers an empty provider) | a gap: `SetDeathDefinition` reaches it with `Death_Siren` (section 5) |

**Crate edges:** `ow-mission` depends on `ow-behavior`, never the reverse. So the kernel cannot call the bridge. It
reads the tracker's answers through a VM service that the bridge registers (section 4, step C). The service's shape
is the implementer's choice.

## 4. Proposed Off answer: register in full, evaluate at once, as the game does

**No deferral:** the game evaluates at registration. "Recorded but not evaluated" would match the game only in a world
without a tracker. That case is answered below by its own rule, not as a stand-in.

**The smallest evaluator** is section 2.4's verdict over the bridge's existing rules:
- mission status by the one-record rule;
- the objective state and the set test for the bridge's own mission;
- NotStarted (so an objective NotStarted) for every other mission.

Only per-instance objectives are refused. For Maya's four conditions the verdict needs:
- the status rule;
- the objective's `bRememberItemsWithinObjective`;
- the NotStarted shortcut.

It needs no objective progress.

**Preflight** (write-free and load-free, nothing interned). It runs in `provider_into` before the decode, and in the
pawn's preflight 7 in place of today's refusal. For each kept definition, each sequence with a `CustomEnableCondition`,
in order:
1. the condition resident, else `NotYet { NotRead }` (a load is a write);
2. its class `WillowGame.BehaviorSequenceEnableByMission` (by the loaded class chain). Any other condition class,
   `BehaviorSequenceEnableByMultipleConditions` among them, is `NotYet { NotRead }` naming the class;
3. its fields declared with their kinds, else `VmError::Script` naming the field:
   - `LinkedMission` and `LinkedObjective`, objects;
   - `bInstanced`, `bIsObjectiveSpecific` and `bCreateWaypoint`, bools;
   - `MissionStatesToLinkTo` (six bools) and `ObjectiveStatesToLinkTo` (three bools), structs;
   - `ObjectiveSetRestrictions`, an array of objects;
4. `bInstanced` set: no link. That sequence's pass 2 is nothing, and the rest of this list is skipped for it;
5. the tracker walk (2.2). Any link missing means no after-link. This rule is the game's, and nothing is refused;
6. with a tracker:
   - its `Role` not the authority's, or `LinkedMission` None: no register;
   - `LinkedObjective` set and not resident: `NotYet { NotRead }`;
   - its `bRememberItemsWithinObjective` set: `NotYet { NotRead }`, "per-instance objectives (the key is not read)";
7. `bCreateWaypoint` set on a condition whose verdict would change its record's state: `NotYet { NotRead }`, "the
   waypoint hooks";
8. the verdict computed now, read-only, from the tracker service. With a tracker but no service:
   `NotYet { LayerAbsent }`, "the mission tracker's answers".

Every refusal above comes before the first write.

**The commit**, after pass 1, in the game's order. Records are in sequence order:
- **A. link:** add (handle, record) to the condition's list. The port keeps the list in the kernel's state, keyed by
  the condition object: it is native state, so no VM field is written. A kernel's lists start empty, and one kernel
  per world plays the instance tag's part;
- **B. register:** store the condition under `LinkedMission` in the service's observer list, unless the mission is
  Complete and not `bRepeatable`;
- **C. react:** apply the preflight's verdict to every record in the condition's list, by the
  `ChangeBehaviorSequenceActivationStatus` kernel rule. The game reacts before it stores; the port's order of A, C, B
  writes the same result, since C reads nothing B writes;
- a refusal from a behavior run by a fired enabled or disabled event comes after writes. It carries the
  "stopped after partial writes: " prefix (#518's counters), with its kind and record kept. Nothing before pass 1's
  first write has the prefix.

**Without a tracker** (a world that has none): pass 1, and the links, and nothing else. This is the game's answer for
that world. Maya's `maya:8` world has the bridge's tracker.

**Reference:** unchanged (the C++ binds none of this).

## 5. Side finding: the direct native's gate

`WillowPawn.SetDeathDefinition` (script, called right after `InitializeBehaviorProviders` in
`ApplyBodyClassStartingValues`) calls `IntializeBehaviorProviderForConsumer` with the death definition.
`Death_Siren`'s provider has **no sequences**. The game's gate then registers nothing (#740's section 2.2: a non-empty
`BehaviorSequences` is required).

The port's direct native has no such gate: it decodes and registers an empty provider. Proposed:
- the direct native applies the same gate, a None or empty definition doing nothing, in the preflight;
- test 9 below.

## 6. Tests (synthetic fixtures)

1. A provider with one conditioned sequence (`bEnabledOnSpawn` false), a tracker with authority, and a mission with no
   record (verdict false):
   - registered; the sequence still disabled;
   - the condition linked once and stored once under the mission;
   - no event fired.
2. The same with mission states {NotStarted} (verdict true): the sequence enabled, and its enabled event fired once,
   after pass 1.
3. `bEnabledOnSpawn` true with a false verdict (Maya's `Ep10_MordecaiChatter`): enabled in pass 1, then disabled in
   pass 2. The enabled event fires, then the disabled event.
4. `bInstanced` set: no link, no register, and the sequence as pass 1 left it.
5. No tracker (`GRI` None, or not a `WillowGameReplicationInfo`, or `MissionTracker` None): links only. The sequences
   as pass 1 left them, and nothing stored.
6. A tracker that is not the authority, or a None `LinkedMission`: linked, not registered, no verdict applied.
7. Two consumers of one definition: the second registration re-applies the verdict to the first consumer's record too.
   With a false verdict, nothing changes for it.
8. Refusals, each before any write (the consumer's handle still -1 through `InitializeBehaviorProviders`):
   - an unloaded condition: `NotRead`;
   - a `BehaviorSequenceEnableByMultipleConditions`: `NotRead` naming it;
   - a mistyped field: `VmError::Script`;
   - an unloaded `LinkedObjective`: `NotRead`;
   - a per-instance objective: `NotRead`;
   - a waypoint condition whose verdict changes the record: `NotRead`;
   - a tracker with no service: `LayerAbsent`.
9. The direct native with an empty definition: nothing registered (section 5).
10. A Complete, non-repeatable mission: the verdict applied, the condition not stored.

## 7. Predicted next stop (a prediction, not a run)

With this rule, Maya's `InitializeBehaviorProviders` completes:
- the handle is set;
- the crouch stance's provider and `PlayerEvents_Siren`'s are registered;
- `Ep10_MordecaiChatter` is disabled.

Two caveats:
- **if** the store holds a `LinkedObjective` (in `GD_Episode11` or `GD_Episode17`) as an unloaded handle at that
  point, the stop is preflight step 6's `NotRead`. That is open question 2, the first thing a run will show;
- `SetDeathDefinition`'s empty provider (section 5) does not stop the run, but differs from the game until fixed.

After that, `ApplyBodyClassStartingValues` reaches the natives that NATIVE_LINK1_RBPHYSICS_AND_AFTER.md section 3
lists. Each is bound on main:
- `BeginNondeterministicProviderRegistration`;
- `GetFirstPersonWeaponHoldDef`;
- `GetLinearColor`;
- `GetInstancedMaterialInstanceConstant`;
- `OnNativeApplyBodyClassStartingValuesHook`.

So no stop is predicted inside it. Which native after it stops maya:8 was not read.

## Open questions

1. **UNVERIFIED:** every rule above, and the missions' state at the slice's spawn (it depends on the save).
2. **Not checked:** whether the port's store holds the four conditions, their missions and objectives as loaded
   objects when `InitializeBehaviorProviders` runs. In the game, the packages that `GD_Siren_Streaming` references are
   loaded with it.
3. **Not read:**
   - the per-instance objective key;
   - the waypoint hooks' own work;
   - the kernel's deferred-registration branch (#740's second branch);
   - `BehaviorSequenceEnableByMultipleConditions`;
   - whether anything re-registers a condition that met no tracker.
4. **The hub's call:**
   - the service's shape between `ow-mission` and `ow-behavior`;
   - whether the Fire slice's `condition_holds` should become the one verdict both use.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns on a fresh character, ask the kernel whether `Ep10_MordecaiChatter` is enabled. Expect false,
   and `CraterLake_ElevatorDownCheck` true.
2. With Ep10 active (a save in that mission), the same check: expect `Ep10_MordecaiChatter` true.
3. Accept a mission that one of the four conditions links, and check that its sequence's state follows the verdict
   without a respawn (the stored observer).
