## 2026-11-19: GetExpLevelFromManufacturerData, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-GET-EXP-LEVEL, slice link 8. Coded from
docs/verification/NATIVE_GET_EXP_LEVEL_FROM_MANUFACTURER_DATA.md alone (#567, accepted at 11798cc1). Clean room: lane
ANALYST-GET-EXP-LEVEL-FROM-MANUFACTURER-DATA (another session) read the executable; this lane did not open it or that
lane's analysis output. Synthetic fixtures only.

- **What changes (`ow-combat` `loot`):**
  - **One rule, two sets of reads.** `exp_level_rule` holds the rule of note section 2. The loot roll's
    `exp_level_for` now runs it through `LootReads`, its own reads unchanged: it loads as it goes, as the C++ does.
    The new Off native runs it through `exp_level::OffReads`, which load nothing.
  - **The rule:**
    - the cycle guard first (a balance already in the call-local list answers 1);
    - then the interpolation test (the word, 0 included);
    - then the port's depth limit (`MAX_BALANCE_CHAIN`, 64);
    - a non-interpolating balance answers from its own identity-matched entry and the grade `word & 0xFFFF` (the
      grade's `ExpLevel` when the high half is 0, else the high half capped at 63);
    - else its base's answer, else 1.
  - **The native** (`exp_level`): `WillowGame.InventoryBalanceDefinition.GetExpLevelFromManufacturerData`, bound in
    Off, first-wins, from the combat pack. Reference binds nothing. The note's section 4 preflight is the
    reads themselves:
    - a visited balance that is unloaded (a store handle, or a slotless stand-in) is `NotYet { NotRead }`, "the
      balance is not loaded (a load is a write)";
    - shapes on the visited balances: `bInterpolateExpLevel` a bool and `BaseDefinition` an object; for a
      non-interpolating balance, `Manufacturers` an array of structs with `Manufacturer` and an array `Grades`, and
      the picked grade's `GradeModifiers.ExpLevel` an int when present (absent is 1). Anything else is
      `VmError::Script`;
    - the manufacturer is matched by `ObjRef` identity with no load. A listed stand-in outside the store, which
      identity cannot settle, is `NotYet { NotRead }`;
    - 0 is a valid answer. The native writes nothing, so its refusals are never marked.
- **The harness (`ow-cli` `inventory::equip_run`, the hub's ruling), Off only:**
  - the equip harness sets the definition data's `ManufacturerDefinition` to the balance's first listed manufacturer
    (`GD_Manufacturers.Manufacturers.Bandit` for link 8's rifle), as the game's spawn does;
  - it sets `ManufacturerGradeIndex` to the level;
  - both are logged as an UNVERIFIED harness input.
  - So on link 8, whose balance interpolates, the native answers the level (1), not 0.
- **Tests** (`ow-combat` `loot/exp_level_tests.rs`, 6, hand-defined classes):
  - interpolation answers the word, 0 included, without reading the manufacturers;
  - the matching grade: an `ExpLevel` of 0, an absent `ExpLevel` (1), and the high half capped at 63;
  - the base, then 1: a None manufacturer and an index past the grades both fall through;
  - the cycle guard and the depth limit;
  - the preflight refusals: an unloaded balance and an unloaded base, an unsettled manufacturer stand-in, and three
    malformed shapes;
  - the VM binding answers an int and writes nothing, and Reference stays unbound.

  The existing loot tests pass unchanged (`ow-combat` 193 passed).
- **Checks:** `cargo test --workspace -j 2` 2,450 passed, 0 failed, 46 ignored; clippy shows nothing new in these
  files (the three loot warnings are on main); crate edges OK.
- **Installed `--slice-gate`, Off, strict, under the machine lock,** on a throwaway local merge, never pushed, with:
  - #534 (8fc0420b) and #552 (0438f6b3). Their `card.rs` conflict was joined by hand: #552's documentation, and both
    sides' constants;
  - #558 (3a598e4c);
  - the particle implementation (`fix/rust-get-particle-effect`, 04227798). Its `equip_run.rs` conflict kept both
    sides.

  Result: 1 ok / 5 not yet / 8 error.
  - Link 8 runs `make` and stops in `init` at "stopped after partial writes: value resolver class
    WillowGame.DamageTypeAttributeValueResolver" (`NotRead`).
  - A/B: the same build without this native's registration stops at the same place. So on this stack link 8 stops
    before it reaches `GetExpLevelFromManufacturerData`, and this rule is not yet measured on link 8.
- **Review r1 of #577:**
  - **the harness inputs are opt-in:** a `WeaponRoute` names the run a weapon is made for. Only `--equip-run` and
    link 8 (`prepare_line`) set `ManufacturerDefinition` and `ManufacturerGradeIndex`. The pickup links' weapons,
    link 7's unowned decoy included, keep their definition data as it was. A test pins the routes;
  - **shapes before the answer:** each visited balance's `BaseDefinition` is checked (an object) even when it
    interpolates, and it is never followed. A picked grade's `ExpLevel` shape is checked even when the high half
    answers. An int `BaseDefinition` and a string `ExpLevel` are now `VmError::Script`. The loot adapter is unchanged
    (its trait methods default to no check);
  - **the unsettled manufacturer:** a listed package object (export or resource origin) that is neither the store's
    loaded object nor its unloaded handle is `NotYet { NotRead }`, whatever its class's slot count. A canonical
    unloaded export settles by identity. Tested on a synthetic package with a zero-slot class, asserting that neither
    is loaded;
  - **the depth limit** is checked on each fresh visit in Off, after the cycle guard, so an interpolating 65th
    balance is refused and a 64-deep chain answers. The loot roll keeps the C++ order;
  - tests: `exp_level` 9 (the two shape cases, the depth tails at 64 and 65, the stand-in versus canonical fixture).
- **UNVERIFIED.** Nothing here was checked in the running game. The note's live checks would confirm it.
- **Landing (hub run, 2026-11-20).** Merged with #506 r2 and #573 on the chain: link 8's weapon is made by
  `prepare_link8_typed` (`WeaponRoute::Link8`, with #506's write mark), so link 8 keeps the manufacturer inputs;
  `--equip-run` makes its weapon with #573's `prepare_equip` on `WeaponRoute::EquipRun`; the pickup links stay on
  `prepare_line_typed` (`WeaponRoute::Pickup`). The manufacturer-fields test uses `prepare_link8_typed`.
