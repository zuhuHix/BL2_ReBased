## 2026-11-19: Rust slice gate drivers for links 8 and 9, sharper reasons for 2, 6 and 7

AI-assisted (Claude Code). Lane SLICE-GATE-DRIVERS. Number 39 was given by the hub. It builds on decision 2026-11-19-08
(`--slice-gate`). No gameplay rule and no native is added: each driver uses existing code. Parsing does not change.
Nothing here was checked in the running game.

- **Link 8, `equip_gun`, driver `equip-run`.** It runs the phases of `--inventory-move --equip-run` (decision
  2026-11-19-03) on one fixed weapon, `EQUIP_WEAPON`, on that command's own Off world:
  - `make`: the weapon and its owner, as the equip run makes them;
  - `bases`, `equip` and `unequip`, one step each.
  The weapon is a level-1 Bandit assault rifle with the parts of a seed-1 Reference roll. It is not link 5's drop,
  because no headless run spawns that drop as an inventory, so the link needs no other link. The spawn's
  `InitializeInternal` belongs to the loot spawn and is not run. No shot is fired.
  - The evidence is the link's own (decision 2026-11-19-08's rule): external records on the weapon after the equip,
    none after the unequip, and no error line.
  - `equip_run.rs` is split so the gate can run one phase at a time (`prepare_line`, `run_phase`,
    `external_records`, `gate_world`; the module is `pub(crate)`). `--equip-run`'s output is byte-identical before
    and after the split on the 50 installed weapons of decision 03.
- **Link 9, `phaselock_enemy`, driver `phaselock-cast`.** It calls the installed `LiftActionSkill.OnActionSkillStarted`
  on Maya's `ActionSkill_Phaselock`, with a player pawn, its controller and an AI pawn made from class defaults. The
  world has the Core natives, `ow-skills`' tree and action-skill packs and a `WorldInfo`, as `ow-skills`' installed
  run builds it. No evidence check is chosen yet, because what a lock leaves on its target is not settled. So a cast
  that returns is graded `error` ("no evidence"), never `ok`.
- **Links 2, 6 and 7 stay without a driver, with sharper reasons:**
  - link 2: the only mover is H4's, in `ow-host`, which ow-cli does not depend on; no fight area is placed, and the
    map world is not brought up for play;
  - link 6: no pickup path exists in code (the death roll makes records only, and no pickup actor is spawned);
  - link 7: `ow_mission::interactive::choose_usable` exists, but no pickup actor is spawned to choose between, and no
    headless world makes the view trace it takes.
- **The checker** (`tools/check_slice_run_off.py`) accepts the two driver names. Its synthetic test has one new case.
- **Installed Sanctuary, Off** (desktop, 2026-11-19, `origin/main` 179b120; outputs under ignored
  `local/p2/slice-gate-drivers/`):

  | Grading | Before | After |
  |---|---|---|
  | strict | 11 notyet, 3 error | 10 notyet, 4 error |
  | past-errors | 14 notyet | 13 notyet, 1 error |

  Only rows 2, 6, 7, 8 and 9 differ, in both gradings.
  - Link 8 stops at step `equip`: "AttributeSlots: expected an array, found Struct". That is
    `ApplyExternalSlotEffectModifiers` (decision 03) reading the item's slots as a dynamic array. The installed
    `WillowInventory` declares `AttributeSlots[19]`, a fixed array of 19 structs (`class_layout.py`). It is a bug in
    `ow-inventory`, outside this lane's owners; it is reported, not fixed here.
  - Link 9 is `notyet` at step `cast`, on `GearboxPawn.GetSMComponent`: the pawn's `SMComponent` is not its own
    instance (NATIVE_SPECIAL_MOVE_COMPONENT.md, open question 1). A pawn made from class defaults shares the
    default's component. The stop comes from that harness choice.
- **Verified on invented data:** the `slice_gate` unit tests pass 15 of 15, three of them new: links 8 and 9 have
  their own drivers and evidence; a driver's own evidence decides, and a stop wins over it; past-errors notes an
  error line and goes on, and a `NotYet` stops. `tests/check_slice_run_off_test.py` passes 12 of 12.
- **Open:**
  - the `AttributeSlots` read (ow-inventory);
  - an evidence check for link 9;
  - the shared special-move component in link 9's harness;
  - link 8 on link 5's real drop, once a headless run spawns it;
  - drivers for links 2, 6 and 7.
