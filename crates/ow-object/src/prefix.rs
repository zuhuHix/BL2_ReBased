//! Owner: lane W3-DECODE. Where an exported object's tagged properties start: the property prefix of
//! `instantiate_export` (architecture section 3.3), the three values the C++ `Runtime::instantiateExport` accepts
//! (`src/vm.cpp`).
//!
//! An export's payload does not start with its tags. Before them come a few bytes that depend on what kind of object
//! it is; the C++ VM and the tools pass the count in by hand: 4 for an ordinary object (one word: the net index), 8 for
//! a component, 26 for an actor placed in a level. The numbers are copied from the C++ and the tools that established
//! them (`docs/verification/SANCTUARY_MOVER_PROTOTYPE.md`, `SANCTUARY_TERRAIN_BSP_HANDOFF.md`); what the bytes mean,
//! and which prefix a given export needs, are observations of the installed packages, UNVERIFIED as a rule of the game
//! (ROADMAP Phase 0, "the observed 4-byte prefix is UNVERIFIED as a rule").

/// The bytes before an export's tags. The discriminant is the byte count. `#[non_exhaustive]` (granted to W3-DEFS):
/// distribution subobjects may need a 16-byte case (PHASELOCK_STOCK_DATA.md). [`PropertyPrefix::Template`] is that
/// case for a component inside a class default object (decision 2026-11-16-70); the C++ VM accepts only the other
/// three, so [`PropertyPrefix::from_bytes`] does not give it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyPrefix {
    /// An ordinary object: 4 bytes.
    Ordinary = 4,
    /// An actor placed in a level: 26 bytes.
    PlacedActor = 26,
    /// A component: 8 bytes.
    Component = 8,
    /// A component inside a class default object (a template): 16 bytes, its own name as a name at 4 (decision
    /// 2026-11-16-70, a census of the installed packages; UNVERIFIED as a rule of the game). The definition store's
    /// only, `ParityMode::Off`: the C++ VM reads no such object.
    Template = 16,
}

impl PropertyPrefix {
    /// The byte count.
    pub fn bytes(self) -> usize {
        self as usize
    }

    /// The prefix of `bytes` bytes; `None` for any count the C++ refuses ("unsupported exported object request").
    pub fn from_bytes(bytes: usize) -> Option<PropertyPrefix> {
        match bytes {
            4 => Some(PropertyPrefix::Ordinary),
            8 => Some(PropertyPrefix::Component),
            26 => Some(PropertyPrefix::PlacedActor),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_three_cxx_prefixes_exist() {
        for prefix in [
            PropertyPrefix::Ordinary,
            PropertyPrefix::Component,
            PropertyPrefix::PlacedActor,
        ] {
            assert_eq!(PropertyPrefix::from_bytes(prefix.bytes()), Some(prefix));
        }
        assert_eq!(PropertyPrefix::Ordinary.bytes(), 4);
        assert_eq!(PropertyPrefix::Component.bytes(), 8);
        assert_eq!(PropertyPrefix::PlacedActor.bytes(), 26);
        // The template prefix is not one the C++ accepts.
        assert_eq!(PropertyPrefix::Template.bytes(), 16);
        assert_eq!(PropertyPrefix::from_bytes(16), None);
        for other in [0, 1, 3, 5, 12, 25, 27, usize::MAX] {
            assert_eq!(PropertyPrefix::from_bytes(other), None);
        }
    }
}
