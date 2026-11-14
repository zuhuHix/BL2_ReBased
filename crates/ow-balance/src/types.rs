//! Owner: lane W1-BALTYPES. The balance vocabulary types (architecture section 3.6). `GameStage` is final from the
//! scaffold (architecture section 15) so lanes below and beside W1-BALTYPES can name it; W1-BALTYPES keeps it as it
//! is, adds its methods and writes the rest of this file.

/// A game stage, the level number item and enemy balance is rolled at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameStage(pub i32);
