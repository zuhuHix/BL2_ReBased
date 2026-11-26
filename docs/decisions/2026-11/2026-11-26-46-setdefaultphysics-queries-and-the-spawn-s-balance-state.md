## 2026-11-26: SetDefaultPhysics' queries and the spawn's balance state

AI-assisted (Claude Code)

Lane ANALYST-SETDEFAULTPHYSICS-AND-SPAWN-BALANCE, analyst only, with Claude Opus 5.5. Docs only: no code, no
dependency, and no bounds check changed.

- **Why:** #783 predicts links 3 and 4's spawn stops at `SetDefaultPhysics` past its gates (NATIVE_MIND_ROUTE.md
  section 1's three unnamed queries), and found the port's spawn never fills `BalanceDefinitionState`.
- **What:**
  - the new note `docs/verification/NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md`;
  - four NATIVE_INDEX rows and a GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock (class-table slots of four classes, a static flag);
  - the layout tool;
  - a local probe: an installed Off `--combat-run` of links 3 and 4's steps after `player:8`, with a call trace and
    stubs. The temporary code was reverted and never committed.

  Raw output stays under ignored `local/analysis/sdp/` (review r1: `local/analysis/l34/`).
- **Findings** (UNVERIFIED in game):
  - the collision copy is skipped only for a cloaked pawn whose cloak's shield requirement is met, behind a
    process-wide can-cloak flag (initially true);
  - the nav handle is "flying" when its `FlightMode` is neither 0 nor 4;
  - the game fills `BalanceDefinitionState` in the population factory's script, after the class step and the attribute
    starting values;
  - on the probe's path:
    - the spawns stop first at `SetDefaultPhysics`' `SetPhysics` call: both ask for `PHYS_None` from `PHYS_Walking`,
      the base-search `NotRead` (review r1);
    - then at the unbound `WillowMind.RecalculateAttributeInitializedState()`. Its mind part, now read,
      evaluates `CommerceMarkup` and ten resistances on the mind, after the skill refresh and the controller part;
    - five plain reads follow;
    - link 3 then stops at `GameInfo.IsFriendlyFire` (unread), and link 4 at `SetPhysics` on the player pawn.
- **Ruled by the hub on #788:** the spawn's balance input, the archetype's pawn balance as a logged, UNVERIFIED harness
  input in Off only, written where the factory's script fills it.
- **Review r1 (#788):**
  - `SetDefaultPhysics`' `SetPhysics` call refuses on the slice and is the spawns' first stop; it is in the preflight
    and the tests;
  - the order inside the mind's recalculation is skill refresh, controller part, mind part, as NATIVE_MIND_ROUTE.md
    section 3 says, correcting this note's first version; the port's order is named for the implementer;
  - the parallel index rows are folded into the existing ones, with pointers from NATIVE_MIND_ROUTE.md;
  - the mind part's preflight (resident definitions, declared fields) and prefixes are written; the stack step is read;
  - the probe's stubs are stated, and the class-level part is not met (the mind's `CharacterClass` is None);
  - the balance input is Off only, logged, named for both spawns (picked by the balance's archetype pointer), placed
    after the starting values, with a test for a missing or unloaded balance;
  - the archetype record is new port state; citations and the `SetPhysics` prefix are fixed;
  - `IsFriendlyFire` has no script override.
- **Clean room:** a different lane implements this note.
