# InventoryBalanceDefinition.GetExpLevelFromManufacturerData on link 8 (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-GET-EXP-LEVEL-FROM-MANUFACTURER-DATA (decision
2026-11-19-NNN, number pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the script, read with `research/script_disasm.py` (callers in the nine code packages);
- the installed packages through `ow-package --def-dump` (the slice gun's balance: names and counts only);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock), to
  check the existing reading;
- a local, uncommitted probe of the installed slice gate (section 5).

There is no bl-sdk session and no recorded live evidence. Raw output stays under ignored `local/analysis/explevel/`.
This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every
rule here is UNVERIFIED** in the running game.

**What was already covered.** NATIVE_LOOT.md (on main) reads this native ("InventoryBalanceDefinition.
GetExpLevelFromManufacturerData", NATIVE_INDEX row "Dropped item level = min(stage, cap)"). `ow-combat`'s loot code
and `ow-items`' candidate expansion carry the rule as internal helpers for the loot roll. **No lane binds it as the
script-callable native**, and no open head does: a `git grep` of every remote head finds it only in those helpers and
in NATIVE_LOOT.md and NATIVE_WEAPON_RULES.md. That is why link 8 stops at it. This note re-checks the rule, places it
on link 8, and gives the Off binding.

## 1. Callers and the result's use (question a)

- **Callers (script):** `WillowWeapon.ClientInitializeFromWeaponTypeDefinition`, `WillowWeapon.InitializeFromDefinitionData`
  and `WillowWeapon.InitializeInventory`, and the three `WillowItem` functions of the same names. Each sets the
  inventory's `ExpLevel` to `DefinitionData.BalanceDefinition.GetExpLevelFromManufacturerData(
  DefinitionData.ManufacturerDefinition, DefinitionData.ManufacturerGradeIndex)`.
- **On link 8:** the call in `ClientInitializeFromWeaponTypeDefinition`, right after the muzzle flash
  (NATIVE_GET_PARTICLE_EFFECT.md, #562) and before `ClientInitializeInventoryFromDefinition` and
  `ApplyAnimThresholdAttributeValues`.
- **The result** is written to `WillowInventory.ExpLevel`. Script readers include the level the player needs to use
  the item and the projectile fire paths; native readers (attribute contexts that ask an inventory's level) were not
  searched.

## 2. The rule (question b; re-checked against NATIVE_LOOT.md)

The reading in NATIVE_LOOT.md holds. In our words, with `word` the second argument (the grade index, or the level in
the base game):
1. **The cycle guard:** a balance already being asked further up the same chain answers **1**.
2. **Interpolating balance** (`bInterpolateExpLevel`, class default true): the answer is `word` unchanged. The
   manufacturer argument is not looked at, and no base is asked.
3. **Otherwise:** find the manufacturer argument in this balance's `Manufacturers`, the first entry whose
   `Manufacturer` equals it. If found and `word`'s low 16 bits are a valid index into that entry's `Grades`:
   - when `word`'s high 16 bits are zero: that grade's `GradeModifiers.ExpLevel`;
   - otherwise: the high 16 bits, capped at 63.
4. **Not resolved by step 3,** and the balance has a `BaseDefinition`: the base's answer, by the same rule.
5. **Nothing resolved:** 1.

There is no further clamp, and no write, random draw or load inside the native. In the loot flow `word` is
`min(stage, cap)` for an interpolating balance (NATIVE_LOOT.md), which is why "level = game stage" holds there.

## 3. The slice gun (question c; names and counts only)

Link 8's weapon is built by the equip run's harness (`EQUIP_WEAPON`):
- its balance is `GD_Weap_AssaultRifle.A_Weapons.AR_Bandit` (a `WeaponBalanceDefinition`). It **interpolates**, has
  no `BaseDefinition`, and lists 1 manufacturer (`GD_Manufacturers.Manufacturers.Bandit`) with 1 grade (window 1 to
  10000, `ExpLevel` 0);
- the harness sets `BalanceDefinition`, `WeaponTypeDefinition`, `GameStage` (1), the parts and `ExpLevel` (1). It
  does **not** set `ManufacturerDefinition` or `ManufacturerGradeIndex`, which stay None and 0;
- **loaded or not:** the probe found the balance **already loaded** on entry. The interpolating path reads only its
  flag.

**So on link 8 the native answers 0**, the harness's `ManufacturerGradeIndex`, and `ExpLevel` goes from the harness's
1 to 0. In the game, the spawn path sets the grade index word to the level (NATIVE_LOOT.md, the candidate expansion),
so a real level-1 Bandit rifle gets 1. **For the hub:** this is a harness input gap, not a native question. The
harness would have to set `ManufacturerGradeIndex` to the level, and `ManufacturerDefinition` to Bandit, as the
expansion does. Otherwise link 8 runs on with `ExpLevel` 0 (UNVERIFIED whether any later step notices).

## 4. Proposed Off answer (question d; for a separate implementer)

**Every rule below is UNVERIFIED.** Bind `InventoryBalanceDefinition.GetExpLevelFromManufacturerData(object,int)` in
Off; Reference keeps the stub policy. Reuse one rule with the loot helpers rather than a third copy. Where the rule
lives is the implementer's call; `ow-combat`'s loot helper loads lazily, so it cannot be used as is.

**Preflight (write-free, loads nothing):**
1. `this` (the balance), and each `BaseDefinition` the walk would reach, is an unloaded handle: `NotYet { NotRead }`,
   "GetExpLevelFromManufacturerData: the balance is not loaded (a load is a write)". The walk stops at step 2 for an
   interpolating balance, so only the balances it actually visits are checked. The probe saw the slice balance
   loaded, so this does not fire on link 8.
2. `Manufacturers` is an array of a struct with `Manufacturer` and `Grades`, each grade with `GradeModifiers.ExpLevel`
   as an int; `bInterpolateExpLevel` is a bool; `BaseDefinition` is an object. Otherwise a `VmError::Script` naming
   the field.
3. A chain deeper than the port's existing balance-chain limit is a `VmError::Script`, as the loot helper does. The
   game's guard is only the cycle rule of section 2, step 1.

**The answer:** section 2's rule, as an int. Manufacturer `None` matches no entry, so it falls through to the base,
then to 1. The native writes nothing; the script writes `ExpLevel`.

**The counters** (#518, open): the native neither writes nor loads, so its own refusals never take the
"stopped after partial writes: " prefix.

## 5. What link 8 meets after it (question e; local probe, uncommitted)

The probe ran on the same local merge as NATIVE_GET_PARTICLE_EFFECT.md section 5 (#558 3a598e4c, #534 8fc0420b, #552
f17a9d60), with throwaway stand-ins:
- for `GetParticleEffect`, the walk, with its handles loaded by the probe and the 5 unbound evaluators taken as false;
- for this native, the interpolating path only.

**Installed Off `--slice-gate` strict: 1 ok, 4 notyet, 9 error** (unchanged). **Link 8 is 1 of 5 at `init`**, and its
first stop moves to the unbound **`AttributeInitializationDefinition.SetBaseValue(object,attributeinitializationdata,
object,object)`**. That is called by `ApplyAnimThresholdAttributeValues`, once per entry of the weapon type's
`AnimThresholdValues`, after `ClientInitializeInventoryFromDefinition`.

The probe recorded: the balance loaded on entry, manufacturer None, `word` 0, and the stand-in answer 0 (section 3).

## Open questions

1. **For the hub:** the harness's `ManufacturerDefinition` and `ManufacturerGradeIndex` (section 3).
2. **Not searched:** the native readers of `ExpLevel`.
3. **UNVERIFIED:** whether the cycle guard's answer of 1 can be reached by installed data (no balance cycle was
   searched for).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Hook `GetExpLevelFromManufacturerData` (pre and post) while a level-N interpolating weapon is built from a chest or
   a drop. Expect the grade-index argument to equal N and the answer to equal it.
2. On a non-interpolating balance, `GD_GrenadeMods.A_Item_Custom.GM_SkyRocket` (NATIVE_LOOT.md), expect the grade's
   `ExpLevel` when the high 16 bits are zero.
3. Read the weapon's `ExpLevel` and `DefinitionData.ManufacturerGradeIndex` after the build, and compare.
