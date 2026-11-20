## 2026-11-19: Controller's QueryInterface rule in Off, read-only along the chain

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-GEARBOXMIND-QUERYINTERFACE, implementer. The decision number is pending
from the hub (three digits); NNN is a placeholder.

- **Source:** `docs/verification/NATIVE_GEARBOXMIND_QUERYINTERFACE.md` at 53daae12 (#589, accepted; analyst lane
  ANALYST-GEARBOXMIND-QUERYINTERFACE), section 5. This lane did not analyse it, and no native code or analysis output
  was opened (clean room).
- **Base:** origin/main at 66378ec4, rebased onto c687c13 for review r2 (onto #524). The installed `--slice-gate`
  reading below was taken on the 66378ec4 base and not repeated after the rebase.
- **No real-game session:** every rule is **UNVERIFIED** in the running game. The note's bl-sdk check settles it.
- **Unchanged:** Reference registers none of this. `WillowPlayerController`'s own answer is unchanged, and so is
  `Vm::query_interface`'s own code. No dependency was added and no bounds check was loosened.
- **What changes for Off callers** (review r1 of #602, item 4): every `Engine.Controller` subclass now answers by the
  read-only rule, through the ordinary dispatch too. A controller whose interface table was not resolved at set-up is
  `NotRead` ("the interface table of `<class>` has an entry not resolved at set-up"), where `plain_interface` used to
  resolve it. Any `PlayerController`-line class other than `WillowPlayerController` refuses every interface (`NotRead`).
  So the Fire AI hold and the awareness callers, which ask a pawn's controller, now stop at that "not resolved at
  set-up" refusal unless their set-up resolves the tables (as link 9's does, `prepare`).
- **Also for Off callers** (review r2 of #602, finding 1): a controller (or a mind) asked an interface its class does
  not list goes on to its pawn read-only, and the pawn reaches its `Mesh`. Every installed pawn's mesh is a
  `SkeletalMeshComponent`, whose rule (#524) has no read-only form, so that question is now `NotYet { NotPorted }`
  ("not ported read-only"). On main before this PR, `Controller` had no rule and the same question got the plain
  answer (none).

**What changed:**
- `crates/ow-object/src/reflect.rs`:
  - `CodeIndex::implements_resolved` reads only entries resolved already. It answers yes when a resolved entry is `I`
    or derives from it, and no when every entry is resolved and none is. Otherwise it names the first class with an
    unresolved entry, so the answer does not depend on table order. Undecoded chains keep `implements`' structural
    fallback.
  - `resolve_interfaces` and `unresolved_interfaces` are the set-up step and its read-only count.
- `crates/ow-vm/src/interface_read_only.rs` (a child of `api`):
  - `ReadOnlyInterfaceRuleFn` takes `&Vm`, so a rule writes, loads and draws nothing except through a service or the
    world hooks (`Vm::service` and the world hooks hand out shared cells), which these rules do not touch (review r1
    of #602, item 5).
    The questions travel in `InterfaceAsking`: the (object, interface) guard and a rule's own marks.
  - `Vm::query_interface_read_only` is the dispatch:
    - the guard and the depth cap, as `query_interface` has them;
    - an unloaded stand-in is `NotRead` before it is asked;
    - the nearest rule on the class chain runs if it is read-only; a rule with no read-only form is `NotPorted`, "`<class>`'s QueryInterface rule is not ported read-only";
    - with no rule, the plain answer.
  - `Vm::plain_interface_read_only` answers:
    - `NotRead` for an entry not resolved at set-up, "the interface table of `<class>` has an entry not resolved at set-up (resolving it is a load)";
    - `LayerAbsent` for a world whose code layer holds no class;
    - none for a class defined by hand, which implements nothing, as `Vm::implements` already answers;
    - after a "no", the engine-native stand-in check, as `plain_interface` does.
  - `register_read_only_interface_rule` binds a rule in both forms; the ordinary form runs the read-only rule with the
    running chain as its guard. `register_read_only_form` adds a read-only twin beside an existing ordinary rule.
- `crates/ow-combat/src/pawn_interface/read_only.rs`:
  - **`Controller`'s rule**, the note's section 2, steps 1 to 5. It is registered for `Engine.Controller` with
    `Pawn` and for `WillowGame.WillowMind` with `MyWillowPawn` (section 3).
  - Step 1's guard is keyed by the controller alone, on top of the dispatch guard, so it matches the game rather
    than approximating it.
  - Step 2 compares the asked class's name, case-insensitively, with `IWorldBody` and `ISoundBehavior`.
  - Fields are read only when a step reaches them. A missing or non-object field is `VmError::Script`; a field read
    of an unloaded object is `NotRead`. A refusal from further down keeps its kind and record, with the step named ahead.
  - **`Engine.PlayerController`** answers `NotRead`, so the rule never answers that line by inheritance.
  - **Read-only forms** were added for `Pawn`, `WillowAIPawn`, `WillowVehicle` and the open answers.
    `WillowAIPawn`'s `IPlayerMaster` step compares the asked class's path, so it needs no lookup and none can be
    refused. A `None` interface passed from Rust (`query_read_only`) is `VmError::Script`.
  - `GearboxMind` leaves `OPEN_ANSWERS`. The ordinary pawn rules are unchanged.
  - `prepare` is the set-up: it resolves `SETUP_CLASSES` (`IAIInterface`, `IPlayerMaster`, `IRuleEngineOwner` as
    #586 recommends, and the controller classes). It then resolves the interface entries of every class reachable
    from the roots through `Controller`, `Pawn`, `MyWillowPawn`, `MyWillowMind`, `DrivenVehicle`, `Mesh` and
    `PlayerReplicationInfo`. It loads no object: an unloaded one is noted and left for the rule to refuse. Each step
    gets a `harness input:` line.
  - `IWorldBody` and `ISoundBehavior` are compared by name, so they need no lookup; their packages are not identified.
- `crates/ow-cli/src/cmd/slice_gate.rs`: link 9's cast runs `prepare` on its three actors before the script call. The
  lines go to the VM log, because main's link 9 has no set-up list yet (#579 adds one).
- Tests that used `GearboxMind`'s open NotYet as "an unread answer" now use a still-open one: a `WillowPlayerController`
  asked `IAIInterface`, in ow-ai (the AI pawn's `Controller`; review r1 of #602, item 2) and in ow-mission. The ow-ai
  shared fixture's mind gains `MyWillowPawn`.

**Tests** (invented values and classes; `pawn_interface/read_only_tests.rs`, 10, and one in `reflect/tests.rs`):
- the branches:
  - itself;
  - its pawn;
  - no pawn (the `PlayerReplicationInfo` is not asked);
  - the `PlayerReplicationInfo`;
  - `IWorldBody` with a declining pawn gives none although the controller lists it; with no pawn, step 3; with a pawn that lists it, the pawn;
  - the mind reads `MyWillowPawn`, not `Pawn`;
  - the ordinary dispatch gives the same answers;
- lazy fields:
  - undeclared and malformed fields at step 3 are never read;
  - at steps 4 and 5 they are `Script`;
- the cold entry is `NotRead`, with the entry still unresolved;
- nested refusals:
  - an unloaded `DrivenVehicle` reached through step 4;
  - `IPlayerMaster`;
  - an unloaded pawn;
  - `NotPorted`;
- the guard: the same interface back gives none, and a cross-interface cycle gives none although the controller
  answers that interface when asked directly;
- `PlayerController` `NotRead`, `None` is `Script`, no code layer is `LayerAbsent`, and Reference registers none;
- the set-up resolves the cold entry, loads no object and logs each step.

Every question in the tests, an answer or a refusal, checks #518's counters (`Heap::committed_writes`,
`GameRng::draws`, `DefStore::load_transitions`) and the class table's length unchanged around it (review r1 of #602,
item 3); the cold-entry test also checks the unresolved-entry count.

**Review r1 of #602** (rebased onto main c687c135): main's #524 gave `SkeletalMeshComponent` its ordinary-only
component rule (it left `OPEN_ANSWERS`). By the hub's ruling the read-only chain reaching it answers `NotPorted`, with
no read-only form added now. The ow-ai hold tests use a `WillowPlayerController` asked `IAIInterface` as their still-open
answer. `prepare` drains the store's log after its loads, and `IRuleEngineOwner` is marked to move to the
evaluator's set-up list.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on a throwaway local merge with #579
(2a428257), #586 (eec4026a, on #535), #564, #548, #555, #560, #523 (as its recipe: our heap/store, the counters'
names) and #595, deleted after:
- ok 2, NotYet 3, error 9 of 14;
- link 9 runs 2 of its driver's 2 steps (spawn, cast). The mind answers `IAIInterface` itself by step 3, and the cast
  goes on to its first stop, the unbound native `GearboxMind.GetAIComponent()` (graded error). Before this lane the
  stop was the NotYet at GearboxMind's own QueryInterface. The note's section 6 predicts exactly this world's stop:
  link 9's world does not register `ow_combat::holds`, which binds that native to a `NotRead` in Off. With an
  answer for GetAIComponent (#594), the next stop is the AI component's own QueryInterface (b6d8's probe).

Landing (hub ruling, 2026-11-20): #575 landed first with `CodeIndex::implements_resolved` (`Option<bool>`), so this lane's whole-chain read is named `implements_resolved_chain` (reflect.rs, its test, ow-vm `interface_read_only.rs`); and #614's `an_owned_ai_component_answers_its_owner_s_answer` resolves the controller's interface tables first (`resolve_interface_tables`), since the controller now answers by this lane's read-only rule.
