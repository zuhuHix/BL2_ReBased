//! Owner: lane W1-ATTRMATH. Modifier stacks (architecture section 3.5). `ModifierId` is final from the scaffold
//! (architecture section 15); W1-ATTRMATH keeps it, adds its methods and writes the rest of this file.

/// The identity of an `AttributeModifier` on a stack: a stack refuses a second modifier with the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModifierId(pub u64);
