//! Owner: lane W3-BALANCE. The two level caps: the player's maximum experience level
//! (`WillowPlayerController.GetMaxExpLevel`, NATIVE_PROGRESSION.md section 3, "Level cap") and the item stage cap the
//! spawn natives and the mission reward roll use (NATIVE_LOOT.md, "Level cap").
//!
//! Both read the licensed level-cap packs ([`LevelCapUpgrades`]), which come from the world (the installed and
//! licensed DLC), never from a guess: the caller passes the set its world holds (`BalanceSnapshot::level_cap_packs`).
//!
//! **Data.** The base cap 50 and the three increments 11, 11 and 8 are the default values in the executable's data
//! (NATIVE_SAVE_LOAD.md, the `ExpLevel` clamp on load; NATIVE_LOOT.md, "Level cap"). Whether the game zeroes an
//! increment when its DLC is missing is not read. **Status: UNVERIFIED** in the running game, both rules; the live
//! install that answered the curve questions had the cap at 80 (REALGAME_GROUND_TRUTH.md), which fits all three packs.

use ow_core::NotYet;

use crate::types::{
    BalanceError, ExpLevel, GameStage, LevelCapPack, LevelCapUpgrades, OpLevel, Playthrough,
};

/// The base level cap without any level-cap pack.
pub const BASE_LEVEL_CAP: i32 = 50;

/// The record the player cap comes from.
pub const PLAYER_CAP_RECORD: &str = "NATIVE_PROGRESSION.md section 3, \"Level cap\"";
/// The record the item stage cap comes from.
pub const ITEM_CAP_RECORD: &str = "NATIVE_LOOT.md, \"Level cap\"";

/// A level-cap pack's increment: the default values in the executable's data, in the records' order (11, 11, 8).
/// Status: UNVERIFIED (whether an increment is zero without its DLC is not read).
pub fn level_cap_increment(pack: LevelCapPack) -> i32 {
    match pack {
        LevelCapPack::First => 11,
        LevelCapPack::Second => 11,
        LevelCapPack::Third => 8,
    }
}

/// `WillowPlayerController.GetMaxExpLevel`: 50 plus the increment of each licensed level-cap pack, clamped to 50 plus
/// the sum of all increments (80). The clamp is the game's own; with three packs it never binds. In every
/// playthrough (the record names no playthrough test). Status: UNVERIFIED.
pub fn max_exp_level(upgrades: LevelCapUpgrades) -> ExpLevel {
    let all: i32 = LevelCapPack::ALL
        .iter()
        .map(|p| level_cap_increment(*p))
        .sum();
    let licensed: i32 = LevelCapPack::ALL
        .iter()
        .filter(|p| upgrades.contains(**p))
        .map(|p| level_cap_increment(*p))
        .sum();
    ExpLevel::new((BASE_LEVEL_CAP + licensed).min(BASE_LEVEL_CAP + all))
}

/// The item stage cap (an internal routine of the spawn natives and the mission reward roll; NATIVE_LOOT.md, "Level
/// cap"): 50 in every playthrough but the third. In the third (UVHM) with every level-cap pack, 80 (50 plus 11, 11 and
/// 8) plus the active overpower level capped at 10.
///
/// - An OP level outside UVHM is refused ([`BalanceError::OpLevelOutsideUvhm`]), as [`OpLevel::check_playthrough`]
///   refuses it.
/// - UVHM without all three packs: the record says "the intermediate value is returned" and lists how the
///   intermediate values are chosen as open, so this is `NotYet { NotRead }`, never a guessed sum.
///
/// Status: UNVERIFIED; the third-playthrough branch was read at overview level.
pub fn item_stage_cap(
    pt: Playthrough,
    op: OpLevel,
    upgrades: LevelCapUpgrades,
) -> Result<GameStage, BalanceError> {
    op.check_playthrough(pt)?;
    if !pt.is_uvhm() {
        return Ok(GameStage(BASE_LEVEL_CAP));
    }
    if upgrades != LevelCapUpgrades::ALL {
        return Err(NotYet::not_read(
            format!(
                "the item stage cap in UVHM with level-cap packs {:#x} (not all three): how the intermediate value is chosen",
                upgrades.bits()
            ),
            ITEM_CAP_RECORD,
        )
        .into());
    }
    let all: i32 = LevelCapPack::ALL
        .iter()
        .map(|p| level_cap_increment(*p))
        .sum();
    // "capped at 10": the game's own cap; OpLevel's type range is the same 0..=10.
    let op = i32::from(op.get()).min(i32::from(OpLevel::MAX));
    Ok(GameStage(BASE_LEVEL_CAP + all + op))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(packs: &[LevelCapPack]) -> LevelCapUpgrades {
        packs.iter().fold(LevelCapUpgrades::NONE, |s, p| s.with(*p))
    }

    #[test]
    fn the_player_cap_adds_the_licensed_packs() {
        use LevelCapPack::*;
        // NATIVE_PROGRESSION.md "Level cap": 50 + licensed increments; the increments 11, 11, 8 (NATIVE_LOOT.md).
        assert_eq!(max_exp_level(LevelCapUpgrades::NONE).get(), 50);
        assert_eq!(max_exp_level(set(&[First])).get(), 61);
        assert_eq!(max_exp_level(set(&[Second])).get(), 61);
        assert_eq!(max_exp_level(set(&[Third])).get(), 58);
        // A set, not a count: 11 + 11 and 11 + 8 differ.
        assert_eq!(max_exp_level(set(&[First, Second])).get(), 72);
        assert_eq!(max_exp_level(set(&[First, Third])).get(), 69);
        assert_eq!(max_exp_level(LevelCapUpgrades::ALL).get(), 80);
    }

    #[test]
    fn the_item_cap_is_fifty_outside_uvhm_whatever_the_packs() {
        // NATIVE_LOOT.md "Level cap": 50 for every playthrough except the third.
        for pt in [Playthrough::NORMAL, Playthrough::TVHM] {
            for bits in 0..=7 {
                let up = LevelCapUpgrades::from_bits(bits).unwrap();
                assert_eq!(
                    item_stage_cap(pt, OpLevel::NONE, up).unwrap(),
                    GameStage(50)
                );
            }
            assert!(matches!(
                item_stage_cap(pt, OpLevel::new(1).unwrap(), LevelCapUpgrades::ALL),
                Err(BalanceError::OpLevelOutsideUvhm { op: 1, .. })
            ));
        }
    }

    #[test]
    fn the_uvhm_item_cap_with_every_pack_adds_the_op_level() {
        // "80 (50 plus 11, 11 and 8) plus the player's overpower level capped at 10 (8 plus 2)".
        for op in 0..=10 {
            let cap = item_stage_cap(
                Playthrough::UVHM,
                OpLevel::new(op).unwrap(),
                LevelCapUpgrades::ALL,
            )
            .unwrap();
            assert_eq!(cap, GameStage(80 + op));
        }
    }

    #[test]
    fn the_uvhm_item_cap_without_every_pack_is_not_read() {
        use LevelCapPack::*;
        for up in [LevelCapUpgrades::NONE, set(&[First]), set(&[First, Second])] {
            match item_stage_cap(Playthrough::UVHM, OpLevel::NONE, up) {
                Err(BalanceError::NotYet(n)) => {
                    assert_eq!(n.missing, ow_core::notyet::Missing::NotRead);
                    assert_eq!(n.record, ITEM_CAP_RECORD);
                }
                other => panic!("expected NotYet, got {other:?}"),
            }
        }
    }
}
