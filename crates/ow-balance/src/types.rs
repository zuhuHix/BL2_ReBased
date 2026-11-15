//! Owner: lane W1-BALTYPES. The balance vocabulary types (architecture section 3.6). `GameStage` is final from the
//! scaffold (architecture section 15) so lanes below and beside W1-BALTYPES can name it; W1-BALTYPES keeps it as it
//! is, adds its methods and writes the rest of this file.
//!
//! These are types only: no balance rule (XP curve, region stage, item stage cap, playthrough modifiers) lives here;
//! those are W3-BALANCE's. Every range below names the record it comes from; a range no record gives is not invented,
//! and the type then accepts any value of its integer type. Nothing in this file has been confirmed in the running
//! game: every reading is UNVERIFIED unless its doc comment says otherwise.

use std::fmt;

use ow_core::NotYet;

/// Why a balance value was refused or is not available.
///
/// The `Attr(AttrError)` variant of architecture section 3.6 is added once `ow-attr` has an error type (W2-ATTRAPI);
/// the enum is `#[non_exhaustive]`, so that is an additive change.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BalanceError {
    /// The rule or branch is not read or not built yet (architecture rule R3).
    NotYet(NotYet),
    /// A value outside the range its record gives. `what` names the type and the range.
    OutOfRange { what: &'static str, value: i64 },
    /// A nonzero OP level outside the third playthrough (UVHM). Status: UNVERIFIED; the records only read OP levels in
    /// the third playthrough's level-cap branch (NATIVE_LOOT.md, "Level cap").
    OpLevelOutsideUvhm { op: u8, playthrough: Playthrough },
}

impl fmt::Display for BalanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BalanceError::NotYet(n) => write!(f, "{} {} ({})", n.what, n.missing, n.record),
            BalanceError::OutOfRange { what, value } => {
                write!(f, "{what}: {value} is out of range")
            }
            BalanceError::OpLevelOutsideUvhm { op, playthrough } => write!(
                f,
                "OP level {op} in playthrough index {}: OP levels exist only in UVHM (index 2), UNVERIFIED",
                playthrough.index()
            ),
        }
    }
}

impl std::error::Error for BalanceError {}

impl From<NotYet> for BalanceError {
    fn from(n: NotYet) -> Self {
        BalanceError::NotYet(n)
    }
}

/// A playthrough, held as the 0-based index the game counts with: 0 Normal (NVHM), 1 True Vault Hunter Mode (TVHM),
/// 2 Ultimate Vault Hunter Mode (UVHM).
///
/// Source: `WillowGameReplicationInfo.CurrentPlaythrough` and `WillowPlayerController.GetCurrentPlaythrough`
/// (NATIVE_CONTROLLER_HELPERS.md). Status: UNVERIFIED that the game's value is 0-based in this way (real-game question
/// 1, architecture section 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Playthrough(u8);

impl Playthrough {
    /// Normal mode, index 0.
    pub const NORMAL: Self = Playthrough(0);
    /// True Vault Hunter Mode, index 1.
    pub const TVHM: Self = Playthrough(1);
    /// Ultimate Vault Hunter Mode, index 2.
    pub const UVHM: Self = Playthrough(2);
    /// The highest index (UVHM).
    pub const MAX_INDEX: u8 = 2;

    /// The playthrough with 0-based index `i`; anything outside 0..=2 is refused.
    pub fn from_index(i: i32) -> Result<Self, BalanceError> {
        if (0..=i32::from(Self::MAX_INDEX)).contains(&i) {
            Ok(Playthrough(i as u8))
        } else {
            Err(BalanceError::OutOfRange {
                what: "Playthrough index (0..=2)",
                value: i64::from(i),
            })
        }
    }

    /// The playthrough with 1-based count `c` (1 Normal, 2 TVHM, 3 UVHM); anything outside 1..=3 is refused.
    /// Status: UNVERIFIED, the same `count = index + 1` reading as [`Playthrough::count`].
    pub fn from_count(c: i32) -> Result<Self, BalanceError> {
        if (1..=i32::from(Self::MAX_INDEX) + 1).contains(&c) {
            Ok(Playthrough((c - 1) as u8))
        } else {
            Err(BalanceError::OutOfRange {
                what: "Playthrough count (1..=3)",
                value: i64::from(c),
            })
        }
    }

    /// The 0-based index (0, 1 or 2).
    pub fn index(self) -> u8 {
        self.0
    }

    /// The 1-based count view: `index + 1`, the reading of the game's `PlayThroughCount`-style values (1 = Normal).
    /// Status: UNVERIFIED (real-game question 1).
    pub fn count(self) -> u8 {
        self.0 + 1
    }

    /// True in the third playthrough (UVHM), the only one where the records read OP levels.
    pub fn is_uvhm(self) -> bool {
        self == Self::UVHM
    }
}

/// An Overpower (OP) level, 0..=10.
///
/// Source: NATIVE_LOOT.md, "Level cap": in the third playthrough the cap adds "the player's overpower level capped at
/// 10". Status: UNVERIFIED (the third-playthrough branch was read at overview level). An OP level is nonzero only in
/// UVHM; [`OpLevel::in_playthrough`] refuses anything else with [`BalanceError::OpLevelOutsideUvhm`] (also
/// UNVERIFIED).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct OpLevel(u8);

impl OpLevel {
    /// No OP level, the only value outside UVHM.
    pub const NONE: Self = OpLevel(0);
    /// The highest OP level the cap reading allows.
    pub const MAX: u8 = 10;

    /// The OP level `n`; anything outside 0..=10 is refused. This does not check the playthrough; use
    /// [`OpLevel::in_playthrough`] where one is known.
    pub fn new(n: i32) -> Result<Self, BalanceError> {
        if (0..=i32::from(Self::MAX)).contains(&n) {
            Ok(OpLevel(n as u8))
        } else {
            Err(BalanceError::OutOfRange {
                what: "OpLevel (0..=10)",
                value: i64::from(n),
            })
        }
    }

    /// The OP level `n` in playthrough `pt`: the range check of [`OpLevel::new`], then a nonzero level outside UVHM
    /// is refused (UNVERIFIED reading).
    pub fn in_playthrough(n: i32, pt: Playthrough) -> Result<Self, BalanceError> {
        let op = Self::new(n)?;
        op.check_playthrough(pt)?;
        Ok(op)
    }

    /// Refuses this level in `pt` when it is nonzero and `pt` is not UVHM.
    pub fn check_playthrough(self, pt: Playthrough) -> Result<(), BalanceError> {
        if self.0 != 0 && !pt.is_uvhm() {
            Err(BalanceError::OpLevelOutsideUvhm {
                op: self.0,
                playthrough: pt,
            })
        } else {
            Ok(())
        }
    }

    /// The level, 0..=10.
    pub fn get(self) -> u8 {
        self.0
    }
}

/// The number of players in the game, 1..=4; single player is 1.
///
/// Source: decision D12 (architecture section 11): single player only for now, with the game's `NumberOfPlayers`
/// resolving through its data chain to this value (WEAPON_BALANCE_DECODE.md names `NumberOfPlayers`, default 1).
/// The upper bound 4 is BL2's co-op size. Status: UNVERIFIED as a game rule; co-op is Phase 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerCount(u8);

impl PlayerCount {
    /// One player, the only count the slice uses (D12).
    pub const SINGLE: Self = PlayerCount(1);
    /// The highest count.
    pub const MAX: u8 = 4;

    /// `n` players; anything outside 1..=4 is refused.
    pub fn new(n: i32) -> Result<Self, BalanceError> {
        if (1..=i32::from(Self::MAX)).contains(&n) {
            Ok(PlayerCount(n as u8))
        } else {
            Err(BalanceError::OutOfRange {
                what: "PlayerCount (1..=4)",
                value: i64::from(n),
            })
        }
    }

    /// The count, 1..=4.
    pub fn get(self) -> u8 {
        self.0
    }
}

impl Default for PlayerCount {
    fn default() -> Self {
        Self::SINGLE
    }
}

/// A game stage, the level number item and enemy balance is rolled at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameStage(pub i32);

impl GameStage {
    /// The stage number.
    pub fn get(self) -> i32 {
        self.0
    }
}

/// An experience level (a player's or a pawn's), held as the script `int` the game uses.
///
/// No range is read for the type itself, so it accepts any `i32`. (NATIVE_SAVE_LOAD.md reads one clamp on load,
/// `max(saved level, 1)`, which is a rule for W3-BALANCE or the save lane, not an invariant of the type.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExpLevel(i32);

impl ExpLevel {
    pub const fn new(level: i32) -> Self {
        ExpLevel(level)
    }

    pub fn get(self) -> i32 {
        self.0
    }
}

/// An awesome level, the `int AwesomeLevel` argument of the item-pool spawn natives (NATIVE_LOOT.md) and a pawn's
/// `GetAwesomeLevel` (COMBAT_DEATH.md). Enemy drops pass 0 (COMBAT_LOOT.md). What it changes and its range are not
/// read, so the type accepts any `i32`. Status: UNVERIFIED.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AwesomeLevel(i32);

impl AwesomeLevel {
    pub const fn new(level: i32) -> Self {
        AwesomeLevel(level)
    }

    pub fn get(self) -> i32 {
        self.0
    }
}

/// How many level-cap upgrade packs are present, 0..=2.
///
/// Caps as the architecture gives them (section 3.6): 0 -> 50, 1 -> 61, 2 -> 72. NATIVE_LOOT.md, "Level cap", reads
/// 50 outside the third playthrough and, in the third with the upgrades, "80 (50 plus 11, 11 and 8)" plus the OP
/// level; how the intermediate values are chosen is open there. The third-playthrough cap rule (W3-BALANCE) reads
/// this type. Status: UNVERIFIED (overview-level reading). This type holds the count only; no cap is computed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LevelCapUpgrades(u8);

impl LevelCapUpgrades {
    /// No upgrade pack.
    pub const NONE: Self = LevelCapUpgrades(0);
    /// The highest count.
    pub const MAX: u8 = 2;

    /// `n` upgrade packs; anything outside 0..=2 is refused.
    pub fn new(n: i32) -> Result<Self, BalanceError> {
        if (0..=i32::from(Self::MAX)).contains(&n) {
            Ok(LevelCapUpgrades(n as u8))
        } else {
            Err(BalanceError::OutOfRange {
                what: "LevelCapUpgrades (0..=2)",
                value: i64::from(n),
            })
        }
    }

    /// The count, 0..=2.
    pub fn get(self) -> u8 {
        self.0
    }
}

/// The balance inputs of one moment: the playthrough, OP level, player count, player level, stage and awesome level
/// that offline rules and the offline world (W3-BALANCE) read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BalanceSnapshot {
    pub playthrough: Playthrough,
    pub op: OpLevel,
    pub players: PlayerCount,
    pub player_level: ExpLevel,
    pub stage: GameStage,
    pub awesome: AwesomeLevel,
}

impl BalanceSnapshot {
    /// Checks the cross-field rule the types alone cannot: a nonzero OP level only in UVHM (UNVERIFIED).
    pub fn validate(&self) -> Result<(), BalanceError> {
        self.op.check_playthrough(self.playthrough)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playthrough_index_and_count() {
        let pt = Playthrough::from_index(0).unwrap();
        assert_eq!(pt, Playthrough::NORMAL);
        assert_eq!(pt.index(), 0);
        assert_eq!(pt.count(), 1);
        assert_eq!(Playthrough::from_index(1).unwrap(), Playthrough::TVHM);
        assert_eq!(Playthrough::TVHM.count(), 2);
        assert_eq!(Playthrough::from_index(2).unwrap(), Playthrough::UVHM);
        assert_eq!(Playthrough::UVHM.count(), 3);
        for i in 0..=2 {
            let pt = Playthrough::from_index(i).unwrap();
            assert_eq!(Playthrough::from_count(i32::from(pt.count())).unwrap(), pt);
        }
        assert!(Playthrough::UVHM.is_uvhm());
        assert!(!Playthrough::TVHM.is_uvhm());
    }

    #[test]
    fn playthrough_out_of_range_refused() {
        for i in [-1, 3, 4, i32::MIN, i32::MAX] {
            assert_eq!(
                Playthrough::from_index(i),
                Err(BalanceError::OutOfRange {
                    what: "Playthrough index (0..=2)",
                    value: i64::from(i)
                })
            );
        }
        for c in [0, 4, -1] {
            assert!(matches!(
                Playthrough::from_count(c),
                Err(BalanceError::OutOfRange { .. })
            ));
        }
    }

    #[test]
    fn op_level_range() {
        assert_eq!(OpLevel::new(0).unwrap(), OpLevel::NONE);
        assert_eq!(OpLevel::new(10).unwrap().get(), 10);
        for n in [11, -1, 255, i32::MAX] {
            assert_eq!(
                OpLevel::new(n),
                Err(BalanceError::OutOfRange {
                    what: "OpLevel (0..=10)",
                    value: i64::from(n)
                })
            );
        }
    }

    #[test]
    fn op_level_nonzero_only_in_uvhm() {
        for n in 0..=10 {
            assert_eq!(
                OpLevel::in_playthrough(n, Playthrough::UVHM).unwrap().get(),
                n as u8
            );
        }
        for pt in [Playthrough::NORMAL, Playthrough::TVHM] {
            assert_eq!(OpLevel::in_playthrough(0, pt).unwrap(), OpLevel::NONE);
            assert_eq!(
                OpLevel::in_playthrough(1, pt),
                Err(BalanceError::OpLevelOutsideUvhm {
                    op: 1,
                    playthrough: pt
                })
            );
        }
        // The range check comes first.
        assert!(matches!(
            OpLevel::in_playthrough(11, Playthrough::NORMAL),
            Err(BalanceError::OutOfRange { .. })
        ));
    }

    #[test]
    fn player_count_range() {
        assert_eq!(PlayerCount::default(), PlayerCount::SINGLE);
        assert_eq!(PlayerCount::SINGLE.get(), 1);
        for n in 1..=4 {
            assert_eq!(PlayerCount::new(n).unwrap().get(), n as u8);
        }
        for n in [0, 5, -1] {
            assert!(matches!(
                PlayerCount::new(n),
                Err(BalanceError::OutOfRange { .. })
            ));
        }
    }

    #[test]
    fn level_cap_upgrades_range() {
        for n in 0..=2 {
            assert_eq!(LevelCapUpgrades::new(n).unwrap().get(), n as u8);
        }
        assert_eq!(LevelCapUpgrades::default(), LevelCapUpgrades::NONE);
        for n in [3, -1] {
            assert_eq!(
                LevelCapUpgrades::new(n),
                Err(BalanceError::OutOfRange {
                    what: "LevelCapUpgrades (0..=2)",
                    value: i64::from(n)
                })
            );
        }
    }

    #[test]
    fn game_stage_kept_as_w0_placed_it() {
        // GameStage stays a public i32 newtype (architecture section 15).
        let s = GameStage(8);
        assert_eq!(s.0, 8);
        assert_eq!(s.get(), 8);
        assert!(GameStage(7) < GameStage(8));
        assert_eq!(crate::GameStage(-3), GameStage(-3));
    }

    #[test]
    fn levels_accept_any_int() {
        assert_eq!(ExpLevel::new(8).get(), 8);
        assert_eq!(ExpLevel::new(-5).get(), -5);
        assert_eq!(AwesomeLevel::default().get(), 0);
        assert_eq!(AwesomeLevel::new(3).get(), 3);
    }

    #[test]
    fn snapshot_validate() {
        let mut snap = BalanceSnapshot {
            playthrough: Playthrough::NORMAL,
            op: OpLevel::NONE,
            players: PlayerCount::SINGLE,
            player_level: ExpLevel::new(8),
            stage: GameStage(8),
            awesome: AwesomeLevel::new(0),
        };
        assert_eq!(snap.validate(), Ok(()));
        snap.op = OpLevel::new(2).unwrap();
        assert_eq!(
            snap.validate(),
            Err(BalanceError::OpLevelOutsideUvhm {
                op: 2,
                playthrough: Playthrough::NORMAL
            })
        );
        snap.playthrough = Playthrough::UVHM;
        assert_eq!(snap.validate(), Ok(()));
    }

    #[test]
    fn errors_display() {
        let e = Playthrough::from_index(5).unwrap_err();
        assert_eq!(
            e.to_string(),
            "Playthrough index (0..=2): 5 is out of range"
        );
        let e: BalanceError =
            NotYet::needs_real_game("UVHM region stage", 2, "architecture section 12").into();
        assert!(matches!(e, BalanceError::NotYet(_)));
    }
}
