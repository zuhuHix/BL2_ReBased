//! Owner: lane W3-DEFS. Overlay layers: value patches over the cooked definitions (architecture section 3.4,
//! decision D1).
//!
//! **What an overlay is.** A named list of [`Patch`]es, each setting one element of one property of one object path
//! to a value given as a [`PatchValue`] tree, typed through the property's declaration (reflection) when applied.
//! Overlays patch values only: they never define a class, so decision 2026-11-15-08's rule (an overlay that defines
//! classes must be installed before any decode) does not apply to them. The default overlay set is empty (D1): the
//! cooked values are the default runtime source, and a gate that wants the live values names its overlay file.
//!
//! **How patches apply** (`DefStore::push_overlay`): overlays apply in the order pushed, patches in list order, so a
//! later patch of the same element wins. Pushing applies at once to the objects already loaded and to every other
//! object when it loads. A patch replaces the whole element its [`PropertyPath`] ends at: a struct value starts from
//! the struct's defaults with the listed fields set over them; a list replaces the dynamic array. A patch that does not
//! type (an unknown property, a value of the wrong kind, an index outside a static array) leaves the cooked value and
//! is recorded in the overlay's [`OverlayStatus`]; it is never clamped or guessed. `DefStore::origin` tells, per
//! element ([`ow_object::ids::PropAddr`]), whether the value is cooked or which overlay wrote it last.
//!
//! **Formats.** [`Overlay::from_live_capture`] reads the file `tools/real_game/scripts/weapon_dump.py` writes and
//! `tools/real_game/live_overlay.py` reads: one JSON object `{path: {"class": .., "properties": {key: value}}}`, each
//! property one patch of the whole property, in the shape `live_overlay.py` cleans it to (object references written
//! `Class'path'` become the path; an enum given as a number is its position). [`Overlay::from_text`] reads the plain
//! test format: one patch per line, `<object path> <property path> = <JSON value>`, blank lines and lines starting
//! with `#` ignored. Both are game data when they come from the game: such files stay under ignored `local/`.

use std::path::Path;

use ow_core::{NotYet, ObjectPath};

use crate::store::{DefError, RECORD};

/// An overlay pushed into a definition store: its position in push order, from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OverlayId(u32);

impl OverlayId {
    /// The id of the overlay pushed `raw`-th (0 first). Only stores hand these out; other code uses this to rebuild
    /// one it stored as a number.
    pub const fn new(raw: u32) -> Self {
        OverlayId(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Where an overlay's values come from.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayKind {
    /// Values read from the running game (`weapon_dump.py`); their origin in the game is UNVERIFIED (question 3).
    LiveCapture,
    /// Gearbox micropatch values (question 19); not loaded by anything yet.
    Hotfix,
    /// A user's text mod.
    TextMod,
    /// Invented values for tests.
    Test,
}

/// A patch value before typing: the JSON value model. Numbers keep their text, so the declaration decides whether
/// they are an int, a float or a byte without a lossy round trip.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchValue {
    /// `null`: a null object, class or interface reference.
    Null,
    Bool(bool),
    /// A JSON number, as written (`12`, `-0.25`, `1e3`).
    Number(String),
    /// A JSON string: a string, a name, an enum value name, or an object path for a reference.
    Text(String),
    /// A JSON array: a dynamic array's elements.
    List(Vec<PatchValue>),
    /// A JSON object: a struct's fields by name, in the order written.
    Fields(Vec<(String, PatchValue)>),
}

/// One patch (architecture section 3.4).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    /// The object path the patch applies to, matched as `DefStore::find` matches it.
    pub object: ObjectPath,
    /// A property path ([`PropertyPath`]), for example `ClipSize`, `Prop[3].Field` or `Effects[0].BaseModifierValue`.
    pub property: String,
    pub value: PatchValue,
}

impl Patch {
    pub fn new(object: ObjectPath, property: impl Into<String>, value: PatchValue) -> Self {
        Patch {
            object,
            property: property.into(),
            value,
        }
    }
}

/// A named list of patches (architecture section 3.4).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overlay {
    pub name: String,
    pub kind: OverlayKind,
    pub patches: Vec<Patch>,
}

impl Overlay {
    pub fn new(name: impl Into<String>, kind: OverlayKind, patches: Vec<Patch>) -> Self {
        Overlay {
            name: name.into(),
            kind,
            patches,
        }
    }

    /// Reads a live capture (module documentation) as an overlay of kind [`OverlayKind::LiveCapture`]: one patch per
    /// object and property, objects in the file's key order, properties in their key order. Malformed JSON is
    /// [`DefError::OverlaySyntax`]; well-formed JSON of another shape is [`DefError::OverlayShape`].
    pub fn from_live_capture(name: &str, json: &str) -> Result<Overlay, DefError> {
        let _ = (name, json);
        Err(not_yet("Overlay::from_live_capture"))
    }

    /// Reads the plain test format (module documentation) as an overlay of `kind`. Errors as
    /// [`Overlay::from_live_capture`], with the 1-based line and column.
    pub fn from_text(name: &str, kind: OverlayKind, text: &str) -> Result<Overlay, DefError> {
        let _ = (name, kind, text);
        Err(not_yet("Overlay::from_text"))
    }

    /// Reads an overlay file: a live capture when its first non-blank character is `{`, the test format otherwise
    /// (as kind [`OverlayKind::Test`]). The overlay is named after the file. A file that cannot be read is
    /// [`DefError::Io`].
    pub fn read_file(path: &Path) -> Result<Overlay, DefError> {
        let _ = path;
        Err(not_yet("Overlay::read_file"))
    }
}

/// What became of an overlay's patches so far (`DefStore::overlay_status`).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OverlayStatus {
    /// Patches written into a loaded object.
    pub applied: usize,
    /// Patches whose object is not loaded yet (or that no package holds); they apply when it loads.
    pub pending: usize,
    /// Patches that did not type, by position in the overlay, with why; the cooked value stays.
    pub failed: Vec<(usize, DefError)>,
}

/// Where a property element's value comes from (`DefStore::origin`).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueOrigin {
    /// The cooked packages: the export's own tags, or the class default it inherits.
    Cooked,
    /// The overlay that wrote it last.
    Overlay(OverlayId),
    /// The object is not a definition this store loaded (a run-time object, a class default object, a stale handle),
    /// or the address is not one of its properties.
    NotADefinition,
}

/// One step of a [`PropertyPath`]: a property or struct field name and, optionally, an element index.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub name: String,
    /// `[n]`: on a static array, element `n` (on the first segment, the `PropAddr` index); on a dynamic array, its
    /// element `n`, which must exist.
    pub index: Option<u32>,
}

/// A parsed property path: `Name[i]?(.Name[i]?)*`, names of ASCII letters, digits and `_`, indices decimal.
/// The first segment names a property of the object's class; each later one a field of the struct before it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyPath {
    pub segments: Vec<Segment>,
}

impl PropertyPath {
    /// Parses `text`; anything else than the grammar above is [`DefError::BadPropertyPath`].
    pub fn parse(text: &str) -> Result<PropertyPath, DefError> {
        let bad = |reason: &str| DefError::BadPropertyPath {
            path: text.to_string(),
            reason: reason.to_string(),
        };
        let mut segments = Vec::new();
        for part in text.split('.') {
            let (name, index) = match part.find('[') {
                None => (part, None),
                Some(open) => {
                    let inner = part[open + 1..]
                        .strip_suffix(']')
                        .ok_or_else(|| bad("an index must end the segment with ']'"))?;
                    if inner.is_empty() || !inner.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(bad("an index is a decimal number"));
                    }
                    let n = inner.parse::<u32>().map_err(|_| bad("an index past u32"))?;
                    (&part[..open], Some(n))
                }
            };
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                return Err(bad("a name is ASCII letters, digits and '_'"));
            }
            segments.push(Segment {
                name: name.to_string(),
                index,
            });
        }
        Ok(PropertyPath { segments })
    }
}

fn not_yet(what: &'static str) -> DefError {
    DefError::NotYet(NotYet::not_ported(what, RECORD))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(name: &str, index: Option<u32>) -> Segment {
        Segment {
            name: name.to_string(),
            index,
        }
    }

    #[test]
    fn property_paths_parse_the_static_array_and_field_forms() {
        assert_eq!(
            PropertyPath::parse("Prop[3].Field").unwrap().segments,
            vec![seg("Prop", Some(3)), seg("Field", None)]
        );
        assert_eq!(
            PropertyPath::parse("ClipSize").unwrap().segments,
            vec![seg("ClipSize", None)]
        );
        assert_eq!(
            PropertyPath::parse("A_1[0].B[12].C").unwrap().segments,
            vec![seg("A_1", Some(0)), seg("B", Some(12)), seg("C", None)]
        );
    }

    #[test]
    fn property_paths_outside_the_grammar_are_refused() {
        for bad in [
            "",
            ".",
            "A.",
            ".A",
            "A[",
            "A[]",
            "A[-1]",
            "A[+1]",
            "A[1",
            "A[1]x",
            "A B",
            "A[1][2]",
            "A[4294967296]",
        ] {
            assert!(
                matches!(
                    PropertyPath::parse(bad),
                    Err(DefError::BadPropertyPath { .. })
                ),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn parsers_are_not_yet_until_the_body_lands() {
        assert!(matches!(
            Overlay::from_text("t", OverlayKind::Test, ""),
            Err(DefError::NotYet(_))
        ));
        assert!(matches!(
            Overlay::from_live_capture("t", "{}"),
            Err(DefError::NotYet(_))
        ));
    }
}
