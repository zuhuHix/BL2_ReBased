## 2026-11-19: Rust CacheWeaponCard's slot count in Off

AI-assisted (Claude Code). Lane IMPL-CARD-SLOT-COUNT, implementer, for slice link 8 (equip), stacked on #521
(`fix/rust-cache-weapon-card-body` at 83d4e90, after its r2 fix; first built on d479a55). `NNN` is a placeholder number, as on #506 and #521, until the maintainer
rules on three-digit numbers. The decisions check refuses it until then.

- **Clean room:** coded from `docs/verification/NATIVE_CARD_SLOT_COUNT.md` alone (PR #525,
  `research/rust-card-slot-count`; the hub named 283aabf, and the branch head read here was 9f67e56). The implementer
  did not read the executable for it. The note lives on #525's branch, so the module's `SLOT_COUNT_RECORD` path resolves
  once #525 lands.
- **No live session.** Every rule stays **UNVERIFIED** in the running game. The note's last section lists the bl-sdk
  checks.

- **What changed** in `ow-inventory` `equip::card`:
  - **#521's slot-count refusal is gone.** The count is the weapon type's `AttributeSlotMaxActivated` (its class
    default when not stored), 1 to 19 as it is and 19 otherwise. It is the same rule as the existing
    `GetAttributeSlotMaxActivated` binding: both call `equip::natives::clamped_slot_max`. The count never goes past
    the weapon's declared `AttributeSlots`, and reading it writes nothing.
  - **Slots 0 to count - 1 all run the same step**, in index order. A slot that is not `bActivated` is skipped.
    Otherwise both presentations are looked up, and a slot marked `bIncludeInModifierText` appends an entry. A None
    presentation clears that slot's own flag (a planned write; `Card::clear_slot_flags` now lists the slots). Slots
    at or past the count are not read.
  - **The order of reads** follows the note's section 4, step 3: the globals, then the weapon type field (loading
    nothing), then the None early return. A weapon type that is an unloaded definition handle is then
    `NotYet { NotRead }`, "WillowWeapon.CacheWeaponCard: the weapon type <name> is an unloaded definition handle",
    with this note as its record. That refusal comes before any load and before the balance, damage-type and slot
    reads. The type is no longer loaded by the card.
  - **The schema** (in the equip driver's preflight) requires `AttributeSlotMaxActivated` on the weapon type, through
    its parents, as an int.
- **Synthetic tests** (`equip::card::tests`, 33 `#[test]`s on the rebased head, invented classes and values only):
  - the clamp, with stored 0, 1, 19, 20 and -3 on a 20-slot weapon, slots 0, 1, 18 and 19 marked: 1 runs slot 0 only,
    and the others run slots 0 to 18;
  - an unset count taking the class default;
  - a slot past 0 running slot 0's step (skipped, unmarked, and a None presentation clearing its own flag);
  - a missing count field as a schema error;
  - an unloaded weapon type on a synthetic store (`Core.upk` and `ToyPkg.upk`), refused as `NotRead` with this
    record, the handle left unloaded and nothing written.

  The fixture's `AttributeSlots` is now 20 wide.
- **Installed Off `--slice-gate`, strict, under the machine run lock:**
  - before (#521 at 83d4e90; the same row as at d479a55): link 8 `notyet`, 1 of 5 steps, stopped at `init` on "stopped after partial writes:
    WillowWeapon.CacheWeaponCard: the card's slot count";
  - after: link 8 `error`, 1 of 5 steps, stopped at `init` on "unbound native
    WillowWeapon.RecomputeAttributeBaseValues()". The card ran to the end: neither the body note's tie-order nor its
    displayed-value refusal was met on the installed weapon;
  - the other 13 rows are identical; overall 0 ok / 5 NotYet / 9 errors (before: 0 ok / 6 NotYet / 8 errors). The
    run on the rebased head gives the same rows as the first run on d479a55.
- **Not done:** the installed weapon type's stored count and class default were not read directly (the note's open
  question 2); `RecomputeAttributeBaseValues` is a separate native.
