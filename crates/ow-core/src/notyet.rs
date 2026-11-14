//! Owner: lane W0-SCAFFOLD (complete). The typed "not yet" every crate's error enum carries (architecture rule R3).
//!
//! An unread resolver class, an unread playthrough branch, an unported native, a missing lower layer: each returns a
//! [`NotYet`] naming what is missing and the record that says so. It is never replaced by a stand-in value.

use std::borrow::Cow;
use std::fmt;

/// Why something is not available yet.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Missing {
    /// The rule is not read from native code or script yet.
    NotRead,
    /// The rule exists in C++ or Python but is not ported to Rust yet.
    NotPorted,
    /// The lower layer this needs is not built yet; the text names it (for example `"ow-items stats"`).
    LayerAbsent(&'static str),
    /// Blocked on real-game question N (architecture.md section 12).
    NeedsRealGame(u8),
}

/// Something that is not there yet, said explicitly instead of guessed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotYet {
    /// What was asked for, for example `"ow_container::decode_package"` or `"resolver class Foo.Bar"`.
    pub what: Cow<'static, str>,
    /// Why it is not available.
    pub missing: Missing,
    /// The record, document or lane that says so, for example `"NATIVE_ATTRIBUTES.md section 5"`.
    pub record: &'static str,
}

impl NotYet {
    pub fn new(what: impl Into<Cow<'static, str>>, missing: Missing, record: &'static str) -> Self {
        NotYet {
            what: what.into(),
            missing,
            record,
        }
    }

    /// The rule is not read from native code or script yet.
    pub fn not_read(what: impl Into<Cow<'static, str>>, record: &'static str) -> Self {
        Self::new(what, Missing::NotRead, record)
    }

    /// The rule exists in C++ or Python but is not ported yet.
    pub fn not_ported(what: impl Into<Cow<'static, str>>, record: &'static str) -> Self {
        Self::new(what, Missing::NotPorted, record)
    }

    /// The lower layer `layer` is not built yet.
    pub fn layer_absent(
        what: impl Into<Cow<'static, str>>,
        layer: &'static str,
        record: &'static str,
    ) -> Self {
        Self::new(what, Missing::LayerAbsent(layer), record)
    }

    /// Blocked on real-game question `question` (architecture.md section 12).
    pub fn needs_real_game(
        what: impl Into<Cow<'static, str>>,
        question: u8,
        record: &'static str,
    ) -> Self {
        Self::new(what, Missing::NeedsRealGame(question), record)
    }
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Missing::NotRead => f.write_str("not read from native code or script yet"),
            Missing::NotPorted => f.write_str("not ported to Rust yet"),
            Missing::LayerAbsent(layer) => write!(f, "needs {layer}, which is not built yet"),
            Missing::NeedsRealGame(n) => write!(f, "blocked on real-game question {n}"),
        }
    }
}

impl fmt::Display for NotYet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} (see {})", self.what, self.missing, self.record)
    }
}

impl std::error::Error for NotYet {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_what_why_and_the_record() {
        let e = NotYet::not_ported("ow_container::decode_package", "lane W1-LZO");
        assert_eq!(
            e.to_string(),
            "ow_container::decode_package: not ported to Rust yet (see lane W1-LZO)"
        );
        let e = NotYet::layer_absent("shield capacity", "ow-items stats", "lane W4-STATS");
        assert_eq!(
            e.to_string(),
            "shield capacity: needs ow-items stats, which is not built yet (see lane W4-STATS)"
        );
        let e = NotYet::needs_real_game("UVHM region stage", 2, "architecture.md section 12");
        assert_eq!(e.missing, Missing::NeedsRealGame(2));
        assert!(e.to_string().contains("real-game question 2"));
        let e = NotYet::not_read(
            String::from("resolver class A.B"),
            "NATIVE_ATTRIBUTES.md section 5",
        );
        assert_eq!(e.missing, Missing::NotRead);
        assert_eq!(e.what, "resolver class A.B");
    }
}
