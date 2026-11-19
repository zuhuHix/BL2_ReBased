## 2026-11-19: Rust fire to hit: a held weapon's shot into the hit chain, and the replication info from the placed WorldInfo

AI-assisted (Claude Code, Opus 5.5). Lane FIX-FIRE-TO-HIT (slice plan step 4, link 3; owner `ow-combat`). Implementer of
script, stock UE3 and recorded rules. For one Gearbox question (the `WillowAIComponent` hold override) this lane acted
as analyst only, and leaves the answer `NotYet`. Nothing here was checked in the running game, and every rule stays
**UNVERIFIED** there unless a line says how it was seen.

**The first Off stop, confirmed fresh.** On main 86c28d00, the installed `--combat-run` in Off (W6-COMBAT's 15 steps)
gave 0 rows. Every spawn now stops at the engine query `GameReplicationInfo` ("needs the map load"), not at the
template archetype the slice plan's section 2 names (that was settled by later lanes). Reference gave 29 rows.

**What changed.**

- **`ow-world` `query.rs`: the replication info from the placed WorldInfo.** With no handler registered, a query that
  follows the actor list now answers the `GRI` of the `WorldInfo` placed there. That is the object the playthrough
  native reads, by NATIVE_PLAYTHROUGH_ATTRIBUTES.md section 3; in stock UE3, `GameInfo` spawns it into
  `WorldInfo.GRI`. With no `WorldInfo` placed, or with a None `GRI`, the answer stays `NotYet { LayerAbsent }`, so it
  is not read as "no replication info" (the note's 1.0 row). A registered handler still wins. The combat world's
  `enable_deaths` already makes that `GRI` from class defaults (`CurrentPlaythrough` 0, so the count is 1).
- **`ow-items` `data.rs`: the weapon file reader, moved.** `WeaponSpec`, `parse_weapon_spec` and
  `weapon_definition_data` move from `ow-host/src/weapon.rs` with their behaviour unchanged, which is decision 61's
  "shared weapon builder" follow-up. The host re-exports them under its old names (`parse_spec`, `definition_data`).
  `ow-cli`'s `items_stats.rs` copy is still separate.
- **`ow-combat` `hit.rs`: `hit_with` and `ShotParts`.** These are `hit_with_type` plus an impact definition for the
  pipeline and a damage causer for a declared `DamageCauser`. With `ShotParts::default()` the call is
  `hit_with_type`, byte for byte; a shot adds one event line (`TakeDamage shot impact=... causer=...`).
- **`ow-combat` `hit/shot.rs`, new: one shot of a held weapon.**
  - Fire mode 0's setup follows the installed script `WillowWeapon.ClientInitializeFromWeaponTypeDefinition`, read
    with `ow-package --disasm`. It is a reading of the script, not a native. `InstantHitDamageTypes[0]` takes the
    type's `InstantHitDamageType`. `InstantHitDamageTypeDefinitions[0]` takes `StaticGetWeaponDamageType`.
    `InstantHitImpactDefinitions[0]` takes the type's `WeaponImpact`, or the elemental part's when it has one.
  - `StaticGetWeaponDamageType` is native, but its rule is already recorded: the elemental part's
    `CustomDamageTypeDefinition`, else the type's `DefaultDamageTypeDefinition` (read from native code 2026-10-23,
    `tools/weapon_recipe.py`, NATIVE_WEAPON_RULES.md section 5). The name rule built on it was confirmed live once.
  - `GetFiringModeDefinition`: the barrel's `CustomFiringModeDefinition`, else the type's
    `DefaultFiringModeDefinition` (NATIVE_WEAPON_FIRING.md section 1.1).
  - The shot (NATIVE_WEAPON_TRIGGER_AMMO.md, NATIVE_WEAPON_FIRING.md section 6). The damage is the weapon's
    `InstantHitDamage`, the instigator the shooter's controller, the class `InstantHitDamageTypes[0]`, the causer the
    weapon. With a firing mode, the impact definition is `InstantHitImpactDefinitions[0]` unless the mode sets
    `bSuppressImpactEffects`.
  - **Seen live** (PROJECTILE_FIRING_PATTERNS_LIVE.md R6, 59 `TakeDamage` rows): the damage equals the stored bullet
    damage, one call per pellet, and the class is the weapon's source class.
  - The damage type definition the pipeline carries is read as `InstantHitDamageTypeDefinitions[0]`, the array the
    script fills; that the native passes this element is not separated in the notes (UNVERIFIED). It agrees with the
    one live shotgun row of COMBAT_HIT_CHAIN.md (`DmgType_Normal`).
  - **`NotYet` in both modes** (the C++ has no shot, so there is no Reference stand-in): Beam and Rocket fire types
    (`NotPorted`); more than one projectile (`NotRead`: `InstantHitDamage` or `GetMultiProjectileDamage`'s share,
    behind an unidentified shooter check); a fire mode other than 0; a firing mode with `ImpactResponses` (the
    surface lookup `GetDamageSurfaceTypeFromImpact` is not read), with `OnAnyImpact` behaviors, or with its own
    `DamageTypeDefinition` (each `NotRead`).
  - Not modelled, and said so in the module: the bullet's flight, momentum, hit location and hit information, spread,
    pattern, range and ammunition. A run's shot always reaches its pawn.
- **`ow-combat` `run.rs` and `run/shots.rs`, new: the steps `weapon:<file>` and `fire:<pawn>`.**
  - `weapon:<file>` builds the file's weapon with W4-STATS (`instantiate_stats`) in the run's own VM and holds it
    (`Run::hold_weapon`, which runs the fire mode 0 setup).
  - `fire:<pawn>` sends one shot through the `hit` step's chain and bookkeeping, now shared as `Run::player_hit`. Its
    rows are `fire.<class>.L<level>.<n>.*`, counted apart from the `hit` rows, so existing reports keep their numbers.
  - The `hit` step's output is unchanged: it resolves its damage type as before and calls the same chain with
    `ShotParts::default()`.

**`Trigger.TriggerDetachSprites` stays `NotYet`.** It is declared in the installed `Engine` package, which Gearbox
extends (it holds `Engine.WillowInventory`, for one). It is not among the stock UE3 `Trigger` natives public
documentation describes, so there are no public semantics to implement. The earlier analysis (NATIVE_SLICE_OFF_STOPS.md,
10 queries) was not repeated. In Off, `--slice-run` does not reach it any more: the run stops earlier, below.

**Analyst only: `WillowAIComponent`'s hold virtuals.** Every installed pawn's component is a `WillowAIComponent`, and
the hit's `IsDemigodMode` reaches `DemigodOnHold` on it. NATIVE_AI_HOLDS.md leaves that class's overrides unread, so
Off answers `NotYet` there. This lane ran two read-only Ghidra queries, under the machine lock and with the project
read-only (raw output only under ignored `local/analysis/aihold-override/`). The base class's script natives each call
one slot of the component's function table, and the query tool resolves the base class's table. For
`WillowAIComponent` it finds no registration or constructor pattern, the same wall the note records, so whether the
class replaces those slots remains unread. The fix is in `ow-combat`'s refusal for a non-base class, and it needs a
fresh reading. No rule was written from this attempt.

**Verified (automated, invented packages).**

- `ow-combat`: 6 new tests over two invented packages beside the combat fixture (`ToyWeaponGame`, `ToyWeapons`). The
  fixture itself is unchanged, since `ow-cli` pins its output against the C++. In both modes they cover:
  - the fire mode 0 setup with no elemental part, with one that overrides, and with one that does not;
  - the shot's inputs;
  - each `NotYet` and its missing kind;
  - a suppressed impact;
  - the step errors;
  - a shot equals the scripted `hit` with the same inputs (the same rows under their own prefix, and the same
    result apart from the shot's own event line). On the toy pawn this holds in Off and in Reference, where the
    victim is told of a 10-damage hit. On the armored toy (its `TakeDamage` runs `AdjustDamage`) it holds in
    Reference. In Off the armored toy's mind declares none of the `Received*` attributes, so the shot and the hit
    stop alike.
  - The Off end-to-end test gives the toy pawn's mind no AI component, an invented state that says why: the
    fixture's `WillowAIComponent` meets the same unread override.
- `ow-world`: 1 new test of the fallback (none placed, a None `GRI`, a class without `GRI`, the handler winning).
- `ow-host`: its weapon tests pass unchanged on the moved reader.
- Gate numbers are in `docs/verification/GATES.md`.

**Installed game (local, outputs under ignored `local/p2/fix-fire-to-hit/`; "before" is main 86c28d00, "after"
this branch; no game data here).**

- **W6-COMBAT's 15 steps, Off.** Before: 0 rows, every spawn at the `GameReplicationInfo` query. After: 13 rows (the
  five spawns' health and maximum, and the bases). The hits stop at the `WillowAIComponent` override (`NotYet`).
  The two swings stop at the unbound native `WillowGameInfo.TravelPostCountdownInProcess()`, a new stop for link 4.
  The step naming `DmgType_Incendiary_Impact` fails in both modes as before (no such stock object).
- **The same steps, Reference:** 29 rows before and after, identical.
- **Off spawn health is wrong against a live row (reported, not fixed here).** With the spawns now running in Off,
  each kind's health is a fixed factor below Reference: dummy 5, regular 2, adult 6, ranged 4, baby 0.5. Off reads
  155.73 for the dummy, and 778.658, the Reference value, is the live dummy's recorded health (COMBAT_HIT_CHAIN.md).
  Off leaves out a per-kind health factor. These Off rows are counts, not parity.
- **The shot, with a weapon file under ignored `local/`** (a Tediore pistol rolled with `--item-roll`, level 8):
  - `weapon:` gives the same result in both modes. `InstantHitDamage` is 34.2408, the figure decision 61's host
    evaluation and the Python tool give for this pistol. The class is `WillowDmgSource_Pistol` (the live R6 class
    for pistols). The damage type is `DmgType_Normal`, the impact `Bullet_LargeCaliber`, the firing mode
    `Bullet_Pistol_Default` (`Bullet`).
  - Reference: four shots at the dummy, the bullymong and the adult, 18 rows, every step passing. The bullymong
    loses exactly the 34.2408.
  - Off: the weapon is built the same; each shot stops at the same `WillowAIComponent` override as the scripted hit.
- **`--slice-run`, Off** (`tools/verify_fire_event_order.py`'s steps): before and after give byte-identical output.
  The run exits 3 at `Pawn IAIInterface routing is unread` (NATIVE_AI_HOLDS.md) in the tick after the dummy's spawn,
  12 steps emitted, so it never reaches `hit:fire`. On a `NotYet` the command prints its report without the closing
  brackets, so the checker cannot parse it; that predates this lane and is reported.

**Open, for the hub.**

1. `WillowAIComponent`'s hold virtual overrides (a fresh analyst, with a way to reach that class's function table),
   then a fresh implementer. This is the last stop between an Off shot and its damage on an installed pawn.
2. The Off per-kind health factor above (5 / 2 / 6 / 4 / 0.5 against Reference and the live dummy).
3. `WillowGameInfo.TravelPostCountdownInProcess()`, unbound in Off, which stops every swing (link 4).
4. Pawn `IAIInterface` routing, which stops Off `--slice-run` after the dummy's spawn.
5. `--slice-run`'s partial report on a `NotYet` (not valid JSON).
6. The shot's open rules: pellets, beams and rockets, impact responses and `OnAnyImpact`, a firing mode's own damage
   type, fire modes other than 0, the bullet's flight and the momentum; the equipped weapon still comes from a file,
   not the inventory (FIX-EQUIP-NATIVES, D9).
7. Real game: a bl-sdk read of a held pistol's `InstantHitDamageTypeDefinitions[0]` and of the pipeline's damage type
   at its victim's `TakeDamage`, which would settle the reading above.
