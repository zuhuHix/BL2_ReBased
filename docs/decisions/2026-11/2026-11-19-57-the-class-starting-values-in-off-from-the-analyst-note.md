## 2026-11-19: The class starting values in Off from the analyst note

AI-assisted (Claude Code, Opus 5.5). Lane FIX-CLASS-STARTING-VALUES, implementer. Both natives are coded from
`docs/verification/NATIVE_CLASS_STARTING_VALUES.md` alone. That note's analyst was lane ANALYST-CLASS-STARTING-VALUES
(decision 2026-11-19-50), not this thread, and this lane opened no analysis output, Ghidra or REA. There was no
real-game session: every rule is **UNVERIFIED** in the running game. No dependency was added, no bounds check was
loosened and no existing `NotYet` guard was relaxed. It replaces the player override's `NotYet` of decision
2026-11-19-46.

**What changed.**

- `crates/ow-combat/src/hit/pools.rs`: `Controller.ApplyCharacterClassStartingValues` and the
  `WillowPlayerController` override, both bound in `ParityMode::Off` only and both reaching one function. A
  `WillowPlayerController` (or a subclass) gets the override, every other controller the base, so a call through the
  base's function on a player controller runs the override, as a native virtual does. The note's rule, in order:
  - **The base:** every pool of the controller's `ResourcePoolManager` is deleted, always. Then, with a class, one pool
    for each `ResourcePools` entry in order, then `AccuracyPoolDefinition` into `AccuracyPool` and
    `OffHandAccuracyPoolDefinition` into `OffHandAccuracyPool`, each only when set. No role check.
  - **The override:** the base first, with the same class. Then only when `Role` is `ROLE_Authority`: with the
    controller's own `PlayerClass` (not the argument), pools for its `ExperiencePoolDefinition`,
    `SkillCooldownPoolDefinition` and `MeleeSkillCooldownPoolDefinition`, in that order and each only when set,
    stored as `ExpPool`, `SkillCooldownPool` and `MeleeSkillCooldownPool`. Then, with or without a `PlayerClass`, the
    controller's script event `OnPlayerClassChange()`, run through the VM.
  - **References left as they were:** no stored reference is cleared. With no class, an unset definition, a controller
    that is not the authority, or no `PlayerClass`, the field keeps what it held, and its pool is gone.
- **Checked before anything changes:**
  - the class fields, `Role` (a byte), `PlayerClass` (an object), the reference fields' struct types, and that the
    controller has `OnPlayerClassChange` when it will run. A missing field or one of the wrong kind is bad data.
  - `NotYet { NotRead }`, kept from decision 2026-11-19-26 and widened to the override's pools: pools that name one
    resource twice. The note does not give `CreatePool`'s reinitialise flag, which can matter only for a second pool of
    the same resource.
- **Left open, as the note leaves it:**
  - open question 1 (whether the argument and `PlayerClass` can differ) does not change the rule: each step reads
    what the note says it reads;
  - open question 2 (whether a left reference can read as valid again through a same-GUID pool in the same slot) is
    not decided here: the manager's existing validity rule answers it.
- **Reference is unchanged.** Neither key is bound there.
- Tests: `crates/ow-combat/src/run/fixture.rs` gains `ToyPlayerStarting`, an invented package: a player-class
  definition with the six fields, a `WillowPlayerController` subclass with `PlayerClass`, the five references and a
  native `OnPlayerClassChange` the tests count, four invented resources and their pools.

**Review r1 of #444 (the hub's ruling, the same rule as #437, #439 and #442).**

- **Decided before the first delete**, on the controller as it is when the call starts:
  - **Bad data:**
    - the base's three class fields must be declared, with `ResourcePools` an array of objects and the accuracy
      fields objects;
    - `Role` must be a byte, and `PlayerClass` and the player definition fields objects;
    - every reference field to be written must be a pool reference struct, with all four fields and their kinds
      (`ow_world::pools::check_reference_type`);
    - the controller must have `OnPlayerClassChange` when the event will run.
  - **`NotYet { NotRead }`:**
    - one resource twice;
    - any pool to make when the manager is not the authority, or when there is no manager and none would be spawned.
      What creation does there is the note's open question 3. Before, the code wrote an invalid reference over the
      held one.
  - **`NotYet { NotPorted }`:** when the event will run, an `InitPlayerSkillTree` that the event's script calls and
    nothing binds (NATIVE_SKILLS.md).
- **The override is decided from the controller after the base**, in the note's order. If the base changed `Role` or
  `PlayerClass`, the override's checks run again.
- **After the first delete, a `NotYet` is terminal.** It may come from a pool's `ShutDown` or `InitializeAuthoritative`,
  the event, or the override's re-check. Nothing is put back; the text starts "stopped after partial writes" and the
  kind is kept. Any other error is returned unchanged (the hub's ruling in review r2 of #442).
- **`ow-world`:** the per-pool rollback of review r1 of #329 is dropped in favour of that terminal stop. A refusal inside
  `InitializeAuthoritative` now leaves the slot filled and the GUID spent. A manager the call itself spawned is still
  destroyed. Its test is rewritten to match.
- **The combat fixture's toy `AIClassDefinition`** now declares the three base fields, unset. The ow-cli combat digests
  are unchanged (89 passed).

**Verified (automated, invented classes).** `ow-combat`: 3 new tests and 1 updated.

- On the authority with a `PlayerClass`: six pools in the note's order (two listed, accuracy, experience, skill
  cooldown, melee cooldown), each reference naming its slot, the unset off-hand reference left as it was, and the
  event run once after the last pool. The same through the base's function.
- Off the authority: the base only, the three player references left, and no event. On the authority without a
  `PlayerClass`: the base, then the event. A `PlayerClass` that differs from the argument: the base from the argument,
  the player pools from the field. No class on the authority: the delete, the player pools, then the event.
- One resource twice across the base and the override is `NotYet` with nothing changed and no event; off the authority
  the same class runs. A player controller without `PlayerClass`, on the authority, is an error before anything
  changes. Under Reference neither native is bound.
- The base test now runs a player controller off the authority through the base. Without a manager, `CreatePool`
  makes no pool off the authority (the pool creation rule).
- After review r1, 6 more tests on the extended `ToyPlayerStarting` package:
  - an unbound skill-tree native the event's script calls is NotYet before any delete;
  - a refusal in a later pool, and one in the event, are terminal;
  - creation on a non-authoritative manager is NotYet, with a live pool and a held reference kept;
  - a callback in the base changes `PlayerClass`, and the override follows it;
  - four malformed inputs refuse against a populated manager;
  - the off-hand pool comes after accuracy, and a no-class call keeps both references.
  Removing the skill-tree preflight or the after-base re-read fails these tests (checked).
- Gate numbers are in `docs/verification/GATES.md`.

**Installed (Off; this branch rebased onto main 7de9171a, which has #429, #431 and #434; outputs under ignored
`local/p2/fix-class-starting-values/`).**

- `--slice-gate` strict: links 1, 3, 4, 5, 13 and 14 stopped at the override's `NotYet` (decision 2026-11-19-46's
  GATES entry). They now stop at `maya:8` as **errors** on the unbound script native
  `AttributeDefinition.GetValue(object,object,object)`. `ow-attr` has the library rule (`AttributeSystem::value_of`),
  but nothing binds it as a script native in this world. A local diagnostic run with the event skipped gave the same
  stop, so it is reached after the override at least. Whether `OnPlayerClassChange` itself reaches it is not traced.
- Rows 10 to 12 stop at `accept` on `UIInteraction.GetDataStoreClient` (decision 2026-11-19-19's open item).
- Row totals: 6 errors, 8 `NotYet`, 0 ok.
- **After review r1:** links 1, 3, 4, 5, 13 and 14 stop at `maya:8` on the new preflight's `NotYet { NotPorted }`:
  `OnPlayerClassChange`'s script calls `WillowPlayerController.InitPlayerSkillTree()`, which nothing binds. That is
  before the `GetValue` error they reached before. Totals: 0 ok / 14 `NotYet` / 0 errors.

**Open.** The script binding of `AttributeDefinition.GetValue`, now the six links' stop; the note's open questions 1
and 2 (bl-sdk reads, as the note's live checks say).
