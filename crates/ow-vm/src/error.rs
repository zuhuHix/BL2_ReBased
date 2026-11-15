//! Owner: lane W2-VMAPI. The VM's error type (architecture section 3.8).
//!
//! Every failure the VM or a native reports is a [`VmError`]. The C++ VM throws `vm::RuntimeError` with a text; the
//! Rust VM keeps the cases a caller can act on apart and puts the rest in [`VmError::Script`]. `NotYet` is how a
//! missing layer or an unread rule shows (architecture rule R3): never a stand-in value.
//!
//! `Def(DefError)` of the section 3.8 sketch was added by W3-DEFS (an additive change, section 7).

use std::fmt;

use ow_attr::api::AttrError;
use ow_core::NotYet;
use ow_defs::store::DefError;
use ow_object::heap::HeapError;

/// Why a VM operation, a call or a native failed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum VmError {
    /// A runtime error inside a function (the C++ `RuntimeError` texts: "too many arguments for ...", "call depth
    /// limit reached ...").
    Script { function: String, message: String },
    /// The executed-step budget of one top-level call ran out (`Limits::step_limit`; C++ "step limit exceeded").
    StepLimit,
    /// A native with no implementation was called under [`crate::api::StubPolicy::Refuse`]; the text is its key.
    UnboundNative(String),
    /// Something this needs is not there yet (architecture section 4).
    NotYet(NotYet),
    /// The object heap refused an access.
    Heap(HeapError),
    /// The attribute system failed.
    Attr(AttrError),
    /// The definition store failed (lookup, load, overlay). Its `NotYet` and heap errors convert to
    /// [`VmError::NotYet`] and [`VmError::Heap`] instead (`From<DefError>`).
    Def(DefError),
    /// A function's bytecode or a value did not decode.
    Decode(String),
    /// A registration was refused: a key registered twice (C++ "duplicate native registration"), an override of a
    /// function that is native or unknown, a service added twice.
    Registration { key: String, reason: &'static str },
    /// A native asked for an argument the call does not have, or wrote back into one that is not `out`.
    BadArgument {
        function: String,
        index: usize,
        reason: &'static str,
    },
    /// A service was still borrowed when a native called back into script, which the contract forbids (a native
    /// borrows a service only for a short scope, never across `call_script`).
    ServiceBorrowed(&'static str),
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::Script { function, message } => write!(f, "{function}: {message}"),
            VmError::StepLimit => f.write_str("step limit exceeded (runaway script?)"),
            VmError::UnboundNative(key) => write!(f, "unbound native {key}"),
            VmError::NotYet(n) => n.fmt(f),
            VmError::Heap(e) => write!(f, "heap: {e}"),
            VmError::Attr(e) => write!(f, "attribute system: {e}"),
            VmError::Def(e) => write!(f, "definitions: {e}"),
            VmError::Decode(text) => write!(f, "decode: {text}"),
            VmError::Registration { key, reason } => write!(f, "cannot register {key}: {reason}"),
            VmError::BadArgument {
                function,
                index,
                reason,
            } => write!(f, "{function}: argument {index}: {reason}"),
            VmError::ServiceBorrowed(name) => {
                write!(f, "service {name} is still borrowed across a script call")
            }
        }
    }
}

impl std::error::Error for VmError {}

impl From<NotYet> for VmError {
    fn from(n: NotYet) -> Self {
        VmError::NotYet(n)
    }
}

impl From<HeapError> for VmError {
    fn from(e: HeapError) -> Self {
        VmError::Heap(e)
    }
}

/// `NotYet` and heap errors keep their own variants, as for attribute errors; the rest is [`VmError::Def`].
impl From<DefError> for VmError {
    fn from(e: DefError) -> Self {
        match e {
            DefError::NotYet(n) => VmError::NotYet(n),
            DefError::Heap(h) => VmError::Heap(h),
            other => VmError::Def(other),
        }
    }
}

/// `NotYet` and heap errors keep their own variants (so a `NotYet` raised inside a notification override still matches
/// `VmError::NotYet` and gives the CLI's "not yet" exit code); a definition store error converts as a `DefError` does
/// (so its `NotYet` and heap errors keep their variants too); the rest is `VmError::Attr`.
impl From<AttrError> for VmError {
    fn from(e: AttrError) -> Self {
        match e {
            AttrError::NotYet(n) => VmError::NotYet(n),
            AttrError::Heap(h) => VmError::Heap(h),
            AttrError::Def(d) => d.into(),
            other => VmError::Attr(other),
        }
    }
}

/// For a world that runs script inside the attribute system (a change-notification override, a resolver that calls
/// into the VM): `NotYet`, heap and attribute errors pass through unchanged and a definition store error converts as a
/// `DefError` does, so a caller still sees what is missing; every other VM error travels as `AttrError::World` with its
/// text.
impl From<VmError> for AttrError {
    fn from(e: VmError) -> Self {
        match e {
            VmError::NotYet(n) => AttrError::NotYet(n),
            VmError::Heap(h) => AttrError::Heap(h),
            VmError::Attr(a) => a,
            VmError::Def(d) => d.into(),
            other => AttrError::World(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_conversions() {
        assert_eq!(
            VmError::StepLimit.to_string(),
            "step limit exceeded (runaway script?)"
        );
        assert_eq!(
            VmError::UnboundNative("Object.Foo(int)".into()).to_string(),
            "unbound native Object.Foo(int)"
        );
        let n = NotYet::not_ported("the interpreter", "lane W3-INTERP");
        assert_eq!(VmError::from(n.clone()), VmError::NotYet(n));
        let a = AttrError::World("w".into());
        assert_eq!(VmError::from(a.clone()), VmError::Attr(a));
    }

    #[test]
    fn not_yet_and_heap_keep_their_variants_both_ways() {
        let n = NotYet::not_read("x", "r");
        // An attribute NotYet is the VM's NotYet (CLI exit code 3), not Attr(NotYet).
        assert_eq!(
            VmError::from(AttrError::NotYet(n.clone())),
            VmError::NotYet(n.clone())
        );
        // Round trip: VmError::NotYet -> AttrError -> VmError::NotYet.
        let back = VmError::from(AttrError::from(VmError::NotYet(n.clone())));
        assert_eq!(back, VmError::NotYet(n));
        let heap = HeapError::WrongKind {
            expected: "Int",
            found: "Float",
        };
        let back = VmError::from(AttrError::from(VmError::Heap(heap.clone())));
        assert_eq!(back, VmError::Heap(heap));
    }

    #[test]
    fn vm_errors_reach_the_attribute_system() {
        let n = NotYet::layer_absent("x", "ow-world", "r");
        assert_eq!(
            AttrError::from(VmError::NotYet(n.clone())),
            AttrError::NotYet(n)
        );
        let heap = HeapError::WrongKind {
            expected: "Int",
            found: "Float",
        };
        assert_eq!(
            AttrError::from(VmError::Heap(heap.clone())),
            AttrError::Heap(heap)
        );
        let inner = AttrError::World("inner".into());
        assert_eq!(AttrError::from(VmError::Attr(inner.clone())), inner);
        assert_eq!(
            AttrError::from(VmError::StepLimit),
            AttrError::World("step limit exceeded (runaway script?)".into())
        );
    }

    #[test]
    fn definition_store_errors_keep_not_yet_and_heap_and_pass_through_unchanged() {
        let n = NotYet::not_ported("DefStore::find", "r");
        assert_eq!(
            VmError::from(DefError::NotYet(n.clone())),
            VmError::NotYet(n)
        );
        let heap = HeapError::WrongKind {
            expected: "Int",
            found: "Float",
        };
        assert_eq!(
            VmError::from(DefError::Heap(heap.clone())),
            VmError::Heap(heap)
        );
        let d = DefError::NotADefinition("Core.Object".into());
        assert_eq!(VmError::from(d.clone()), VmError::Def(d.clone()));
        assert_eq!(
            AttrError::from(VmError::Def(d.clone())),
            AttrError::Def(d.clone())
        );
        assert_eq!(VmError::from(AttrError::Def(d.clone())), VmError::Def(d));
        // A NotYet held in a Def reaches the NotYet variant both ways.
        let n = NotYet::not_read("y", "r");
        assert_eq!(
            VmError::from(AttrError::Def(DefError::NotYet(n.clone()))),
            VmError::NotYet(n.clone())
        );
        assert_eq!(
            AttrError::from(VmError::Def(DefError::NotYet(n.clone()))),
            AttrError::NotYet(n)
        );
    }
}
