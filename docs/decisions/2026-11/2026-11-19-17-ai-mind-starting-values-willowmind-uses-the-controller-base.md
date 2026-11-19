## 2026-11-19: AI mind starting values: WillowMind uses the Controller base (analyst note)

AI-assisted (Claude Code, Opus 5.5). Lane MIND-STARTING-VALUES, **analyst only** (clean room, docs/LANE_BRIEF.md). No crate
code changed. The rules read here stay `NotYet` in Off until a different thread implements them from the note. There was no
real-game session: everything is **UNVERIFIED** in the running game.

**Why.** Lane FIX-OFF-STOCK traced the Off spawn health factor (decision 2026-11-19-06's 5 / 2 / 6 / 4 / 0.5 against
Reference) to Off skipping the AI class's starting values. The game's route is `WillowMind.Possess` →
`InitializeCharacterClass` → `ApplyCharacterClassStartingValues`, then `ApplyCharacterClassDefaults`. Routing Off through it
stopped on what NATIVE_COMBAT_GROUNDWORK.md listed as an unread `WillowMind` override of
`ApplyCharacterClassStartingValues`.

**Found.**

- **There is no `WillowMind` override.** `WillowMind` declares no script function of that name. In its C++ function table,
  read from its class registration's constructor, the slot `Controller`'s script native calls holds the same routine as 8
  other function tables: the inherited `Controller` base.
- That routine is the groundwork note's step 1, now with every field named by the class layout. It deletes all pools of
  `ResourcePoolManager`, then creates the class's `ResourcePools` in order, then `AccuracyPoolDefinition` into
  `AccuracyPool`, then `OffHandAccuracyPoolDefinition` into `OffHandAccuracyPool`, each only when set. Its two callees
  are the routines behind the script natives `DeleteAllPools` and `CreatePool`.
- **The rest of the route is installed script**, read with `research/script_disasm.py`.
  - `InitializeCharacterClass` runs once, guarded by `bCharacterClassInitialized`.
  - `WillowMind.ApplyCharacterClassDefaults` calls the `Controller` base first: the speeds, then the **health pool**
    through the native `Pawn.SetHealthResourcePoolDefinition`, then the accuracy pools reinitialised.
  - Then the pawn's movement fields and `SetDefaultPhysics`, then the **shield pool**.
  - The interaction proxy is spawned only when the AI class has an `InteractionDefinition`.
  - The passive skills: each `Passive` entry of `PassiveSkills` is activated through `GetSkillManager().ActivateSkill`,
    at the pawn's `GetExpLevel`.
  - Then the pawn's script **`ApplyAttributeStartingValues`** (where the class's starting values reach the pawn and
    mind), then the mind's `RecalculateAttributeInitializedState`, the sight cones and `bUsable`.
- **Pool order for an AI mind:** delete all; the class's `ResourcePools`; accuracy; off-hand accuracy; then health; then
  shield.
- **The natives the route needs** are listed in the note, each with its note or "not read".

**What changed (docs only).**

- `docs/verification/NATIVE_COMBAT_GROUNDWORK.md`: new section 1.11, the summary row, step 1.10's last line and the "Not read
  yet" entry.
- `docs/verification/NATIVE_INDEX.md`: the `ApplyCharacterClassStartingValues` row updated, plus one row for the script
  route.

**Tools.** `research/script_disasm.py`. Ghidra 12.1.4, read-only, under the machine lock and one run at a time; twice I
waited for another lane's run to release the project. Four `tools/ghidra/run.ps1` query runs and two runs of a local
read-only table reader (lane MAP-LOAD-B's, untracked under `local/analysis/`). `tools/ghidra/class_layout.py` for the
fields. Raw output only under ignored `local/analysis/mind-sv/`.

**Open.**

- The natives on the route that have no note yet: `WillowPawn.SetDefaultPhysics`, `WillowAIPawn.GetExpLevel` and
  `WillowMind.RecalculateAttributeInitializedState`; the pawn native `GearboxMind.Possess` calls; the rule engine.
- A bl-sdk read of a fresh bullymong's pool slots and health multiplier would confirm the order and the factor.
