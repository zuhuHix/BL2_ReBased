## 2026-11-19: Rust special move Play and its gates in Off

AI-assisted (Claude Code). Lane FIX-SM-PLAY, implementer (slice link 9, Phaselock). Number 52 was given by the hub. The
branch is `origin/main` 964a5b61 (written on f64aeba9, which holds #427, and rebased once as main moved). The source is NATIVE_SPECIAL_MOVE_PLAY.md as on main (decision
2026-11-19-35, with #427's wording fixes). This lane did not read the natives and opened no `local/analysis/`, Ghidra or
REA output. It read the installed packages only for the native keys, parameter lists and field names it binds. No
dependency, no CLI flag, no parsing change. There was no real-game session; every rule is **UNVERIFIED** in the running
game. Every binding is Off only. Reference has no C++ stand-in and binds nothing new (R3).

- **New module `ow_skills::action_skill::special_move`**, registered by the action skill pack after `GetSMComponent`,
  bound by function path (the first binding of a key wins).
  - **`Pawn.IsAliveAndWell()`** (section 0): `bDeleteMe`, `bIsInjured` and `bIsDead` all clear.
    `LiftActionSkill.GetPlayerAnimation` is installed script; this is the native of its own that it reaches.
  - **`SpecialMoveDefinition.GetSMDToPlay(SMI)`**: the definition itself.
  - **`SpecialMoveDefinition.IsLocalAuthority(SMI)`**: the owner's `Role` is at least `ROLE_Authority`, or at least
    `ROLE_AutonomousProxy` with `bClientHasAuthority`. Roles are compared by their order in the property's enum,
    looked up by name.
  - **`SpecialMoveComponent.IsPlayingClass(SMClass)`**: the current definition is of that class or a subclass.
  - **`SpecialMoveComponent.GetSMNode()`**: None without an owner interface. A cached `SMBlendNode` on the owner's
    current mesh is reused. The hub's decision for the port question: an owner with no mesh, or a mesh with no anim
    tree, is `NotYet { LayerAbsent }` naming the missing anim tree, because the game's pawns always have one and the
    game's None would make `Play` start nothing silently. A mesh with a tree is `NotYet { NotPorted }`: the port does
    not search a tree for the `SpecialMoveBlend` node.
  - **`SpecialMoveComponent.Play(...)`** (sections 1 and 4).
    - The zero answers: no definition, no owner interface, no definition to play, a failed gate.
    - The gates: `IsLocalAuthority`, then the script event `AuthorityCanPlay`. The second is not asked when the first
      fails (UNVERIFIED).
    - Three refusals, decided before anything changes:
      - an owner below `ROLE_Authority` on a network client (`WorldInfo.NetMode` `NM_Client`): the forward to the
        server is network play, `NotYet { NotPorted }`;
      - with the gates passed, an owner at `ROLE_Authority`: `NotYet { NotRead }`. The note says the owner records
        the current move for replication (step 4.4) but does not say how. **In the single-player slice this is every
        `Play` past the gates;**
      - a definition with a `NextSpecialMove` (what the queued follow-ups carry is not read). The Phaselock
        definitions have none.
    - The start, otherwise:
      - the queue is emptied, and a current move is stopped as interrupted;
      - the request becomes `CurrentSMData`, and `StartTime` is the world time;
      - `ClientStarted(SMI, SMData)` answers the duration. A duration of 0 or less stops the move again, not
        interrupted;
      - the answer is the duration. The defaults are the note's: rate 1, duration 0.
    - The stop:
      - the queue and the current move are cleared;
      - `ClientFinished(SMI, bInterrupted)` runs;
      - then the callback runs when the old definition passes `IsLocalAuthority` and a `CallbackName` was given:
        `CallbackName` on `CallbackObject` or the owner, with `bInterrupted` set and every other parameter zero. A
        callback the target does not declare is `NotYet`.
  - **`SpecialMoveComponent.PlayLocal(...)`**: no gate, no current move. `ServerStarted`, then `ClientStarted`, whose
    answer is the answer.
  - **Left unbound**, so the stub policy answers: `RunBehaviors` (until #432), `OnStart` / `OnServerStart`, and the
    blend node's `PlayData`, all of which `ClientStarted` reaches.
  - **Shadowing.** `ow-combat`'s hit pack binds `SpecialMoveComponent.Play` as a no-op and `IsPlayingClass` as false,
    in both modes (its presentation stand-ins). In a world that registers that pack first, those stay. No current CLI
    world registers both packs; whether the combat stand-ins should be Reference only is left to the hub.
- **Verified (automated, local, pc-hummer).**
  - **Synthetic tests** (`ow-skills`, 10 new, invented classes; the script events and the callback are toy natives
    that record their calls):
    - `IsAliveAndWell`: the three flags;
    - `GetSMDToPlay` and `IsLocalAuthority`: the authority, an autonomous proxy with and without
      `bClientHasAuthority`, and a simulated proxy;
    - `IsPlayingClass`: none, the class, a parent, another class, no class;
    - `GetSMNode`:
      - None with no owner;
      - `LayerAbsent` with no mesh, and with no anim tree;
      - `NotPorted` with a tree;
      - the cached node on the owner's mesh, but not on another mesh;
    - `Play`:
      - the zero answers, and no `AuthorityCanPlay` when `IsLocalAuthority` fails;
      - a start with its world time, rate and duration 2.5;
      - a second play that stops the first as interrupted: `ClientFinished`, then the callback on the owner as
        (0, true), then `ClientStarted`;
      - a zero duration that stops again, not interrupted;
      - the three refusals, nothing changed by any of them;
    - `PlayLocal`;
    - Reference binds none of the seven.

    The tests are hand-built class toys, as the crate's other native tests are, not packages from the
    `package()`/`tag()` helpers.
  - **Installed Phaselock cast** (the W6-SKILLS run, `part_cast`, Off), on main and on this branch: the same stop,
    `GetSMComponent` on a pawn whose component is the class default's shared sub-object. The run makes its pawn from
    class defaults, not through a spawn, so SUBOBJECT-INSTANCING (decision 2026-11-19-44, not on main) would not move
    it by itself. Past it, `Play` would stop at `AuthorityCanPlay`'s `GetSMNode` (no anim tree), or, with a tree, on
    the authority's replication record.
  - **`--slice-gate`**, strict: byte-identical on main and on this branch, 0 / 11 / 3. Link 9's row is unchanged:
    `notyet`, "no headless driver: the Phaselock cast has no run step".
- **Gates** (on 964a5b61): workspace build clean; workspace tests 2,038 passed / 0 failed / 45 ignored (`ow-skills`
  88 / 0 / 1 before the rebase); `crate_edges_test.py` OK; decisions check OK (1 added); leak gate clean. The
  `--slice-gate` and cast results above were rerun on 964a5b61, the same. ctest and verify_packages were not rerun
  (no C++ change).
- **Open.**
  - For an analyst: the owner's replication record of step 4.4 (and the stop's update of it), and the anim tree node
    search behind `GetSMNode`.
  - For the hub: a link 9 driver.
  - The note's own open questions: `RunBehaviors`, `OnStart` / `OnServerStart`, `PlayData`;
    `PerformSharedWeaponActions`; the queue order of a `NextSpecialMove` chain.
- **Review r1 of #437** (fix commits on this branch). Where this section and the text above differ, this section
  holds.
  - **The combat stand-ins** (`ow-combat` `hit`): the `Play` no-op and the `IsPlayingClass` false are now bound in
    Reference only (`SPECIAL_MOVE_STAND_INS`). In Off this module's natives answer whichever pack registers first, and
    a world without the action skill pack leaves both unbound. A new test registers both packs in both orders, in both
    modes. The Off combat runs are not changed by it: `--slice-gate` is byte-identical.
  - **Refusals before any change.** `Play` now resolves, before it changes anything:
    - each stop it may run: the current move's, and the new move's own for a duration of 0 or less;
    - their `ClientFinished`;
    - their callbacks: the target must declare the function, with no parameter or one `bool bInterrupted`. Any other
      signature is `NotYet`. The note names no other argument, and the zero or default the first version passed
      would be a guess.

    A script event that fails during the start (an unbound native under `ClientStarted`, for example) cannot be
    decided in advance. Then `CurrentSMData`, `SMDQueue` and `StartTime` are put back as they were and the error is
    answered. What the events themselves changed is not undone. A new test seeds a current move, a one-entry queue
    and a `StartTime` and checks that six refusals leave all three unchanged.
  - **`GetSMNode` with a tree** no longer answers `NotYet { NotPorted }`.
    - The node named `SpecialMoveBlend` (without case) is looked for from the tree's root down: each node, then its
      `Children` entries' `Anim` in order, each node once, at most 4,096 nodes. The order is the port's, UNVERIFIED.
    - The first node of that name answers when it is an `AnimNodeSpecialMoveBlend`, and is cached in `SMBlendNode`.
      Another class, or no such node, answers the note's None.
    - The missing-tree `NotYet { LayerAbsent }` stays for no mesh or no `Animations`, the headless case.
  - **`Pawn.IsFriendly`** already has an Off binding from a note: `ow-combat`'s allegiance rule (NATIVE_COMBAT_GROUNDWORK.md
    section 6), bound in both modes. Those four natives (`GetOpinion`, `IsEnemy`, `IsNeutral`, `IsFriendly`) are now
    their own `hit::register_allegiance`. The hit pack still calls it, and the W6-SKILLS installed cast's VM now
    calls it on its own, without the combat stand-ins. The installed cast still stops first at `GetSMComponent`.
  - **The tests are rebuilt on two invented packages** (`ow_package::testkit`, the Rust side of the `package()` /
    `tag()` helpers): `Engine` and `GearboxFramework`. Every native is declared by the packages and reached by its
    path. There are 12 tests, among them:
    - a script-level regression: a toy `GetPlayerAnimation` script with the cast's shape (`IsFriendly`, then
      `IsAliveAndWell`) picks the hit move for an enemy alive and well, and the fizzle move for a friend and for a
      dead target;
    - the pack-order test;
    - the seeded-refusal test;
    - the `GetSMNode` search (none, another class, found without case, cached, a cache on another mesh not reused).
  - **Gates after r1:** `ow-skills` 90 / 0 / 1; `ow-combat` 148 / 0 / 2; workspace 2,040 / 0 / 45. `--slice-gate`
    strict is byte-identical to the #437 head, 0 / 11 / 3, and link 9 is still `notyet` (no headless driver). The
    installed cast is unchanged: it still stops at `GetSMComponent`.
