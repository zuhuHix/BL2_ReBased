//! Owner: lane W1-NUM. The game's units (architecture rule R9): Unreal units (uu) for distance, rotator units (65,536
//! per turn) for angles, Z-up vectors, seconds as `f32`. Every crate keeps these units; only `ow-host` converts to the
//! engine's frame.
//!
//! AI-assisted (Claude Code).

/// A UE3 rotator: pitch, yaw and roll in rotator units, 65,536 per full turn (the `Rotator` struct's three ints).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rotator {
    pub pitch: i32,
    pub yaw: i32,
    pub roll: i32,
}

impl Rotator {
    pub const fn new(pitch: i32, yaw: i32, roll: i32) -> Self {
        Self { pitch, yaw, roll }
    }
}

/// Rotator units per degree: 65,536 / 360.
pub const ROTATOR_UNITS_PER_DEGREE: f64 = 65536.0 / 360.0;

/// A distance in Unreal units. Z-up vectors are plain `[f32; 3]` named in uu.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Uu(pub f32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quarter_turn() {
        assert_eq!(90.0 * ROTATOR_UNITS_PER_DEGREE, 16384.0);
        assert_eq!(
            Rotator::new(1, 2, 3),
            Rotator {
                pitch: 1,
                yaw: 2,
                roll: 3
            }
        );
        assert_eq!(Rotator::default(), Rotator::new(0, 0, 0));
    }
}
