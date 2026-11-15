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
//! **Edge cases.**
//! - A static-array property (`array_dim > 1`) patched without an index on its first segment: a [`PatchValue::List`]
//!   sets the whole array (exactly `array_dim` elements, element `i` to `PropAddr` index `i`; another length does not
//!   type), any other value sets element 0. With an index, the patch sets that element only.
//! - A patch whose object is a class default (`Default__<Class>`, or a path inside one) counts as failed, not
//!   pending, and the error depends on the overlay's kind: for [`OverlayKind::LiveCapture`] (and `Test`) it is
//!   `DefError::NotADefinition`, as a capture lists definitions only; for [`OverlayKind::TextMod`] and
//!   [`OverlayKind::Hotfix`] it is `DefError::NotYet`, since mods and micropatches do set class defaults and patching
//!   them is a missing feature, not bad data.
//! - A patch whose object no package holds stays pending (`OverlayStatus::pending`).
//! - The numbers Python's `json.dumps` writes for non-finite floats, `NaN`, `Infinity` and `-Infinity`, are accepted
//!   as [`PatchValue::Number`] text (they type only into a float).
//!
//! **Formats.** [`Overlay::from_live_capture`] reads the file `tools/real_game/scripts/weapon_dump.py` writes and
//! `tools/real_game/live_overlay.py` reads: one JSON object `{path: {"class": .., "properties": {key: value}}}`, each
//! property one patch of the whole property, in the shape `live_overlay.py` cleans it to (object references written
//! `Class'path'` become the path; an enum given as a number is its position). [`Overlay::from_text`] reads the plain
//! test format: one patch per line, `<object path> <property path> = <JSON value>`, blank lines and lines starting
//! with `#` ignored. Both are game data when they come from the game: such files stay under ignored `local/`.

use std::path::Path;

use ow_core::ObjectPath;
use ow_object::heap::{ObjectSpace, Step};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;

use crate::store::DefError;

// An overlay pushed into a definition store: its position in push order, from 0.
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
    /// A JSON number, as written (`12`, `-0.25`, `1e3`), or one of the non-finite floats Python writes (`NaN`,
    /// `Infinity`, `-Infinity`).
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
        let top = json::parse_document(json)?;
        let PatchValue::Fields(objects) = top else {
            return Err(shape("(file)", "the capture is not a JSON object"));
        };
        let mut patches = Vec::new();
        for (path, row) in objects {
            let object = ObjectPath::parse(&path)
                .map_err(|e| shape(&path, &format!("not an object path: {e}")))?;
            let PatchValue::Fields(fields) = row else {
                return Err(shape(&path, "an entry is not a JSON object"));
            };
            let mut properties = None;
            for (key, value) in fields {
                match key.as_str() {
                    "properties" => properties = Some(value),
                    "class" => {}
                    other => return Err(shape(&path, &format!("unknown key {other:?}"))),
                }
            }
            let Some(PatchValue::Fields(properties)) = properties else {
                return Err(shape(&path, "no \"properties\" object"));
            };
            for (key, value) in properties {
                PropertyPath::parse(&key)?;
                let value = clean(value, Some(&key));
                patches.push(Patch::new(object.clone(), key, value));
            }
        }
        Ok(Overlay::new(name, OverlayKind::LiveCapture, patches))
    }

    /// Reads the plain test format (module documentation) as an overlay of `kind`. Errors as
    /// [`Overlay::from_live_capture`], with the 1-based line and column.
    pub fn from_text(name: &str, kind: OverlayKind, text: &str) -> Result<Overlay, DefError> {
        let mut patches = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line_no = n + 1;
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let column = |rest: &str| line.len() - rest.len() + 1;
            let syntax = |rest: &str, reason: &str| DefError::OverlaySyntax {
                line: line_no,
                column: column(rest),
                reason: reason.to_string(),
            };
            let split = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
            let (object_text, rest) = trimmed.split_at(split);
            let object = ObjectPath::parse(object_text)
                .map_err(|e| syntax(trimmed, &format!("not an object path: {e}")))?;
            let rest = rest.trim_start();
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '=')
                .unwrap_or(rest.len());
            let (property, after) = rest.split_at(end);
            if property.is_empty() {
                return Err(syntax(rest, "a property path is missing"));
            }
            PropertyPath::parse(property)?;
            let after = after.trim_start();
            let Some(value_text) = after.strip_prefix('=') else {
                return Err(syntax(after, "'=' expected after the property path"));
            };
            let value = json::parse_document(value_text).map_err(|e| match e {
                DefError::OverlaySyntax {
                    column: c, reason, ..
                } => DefError::OverlaySyntax {
                    line: line_no,
                    column: column(value_text) + c - 1,
                    reason,
                },
                other => other,
            })?;
            patches.push(Patch::new(object, property, value));
        }
        Ok(Overlay::new(name, kind, patches))
    }

    /// Reads an overlay file: a live capture when its first non-blank character is `{`, the test format otherwise
    /// (as kind [`OverlayKind::Test`]). The overlay is named after the file. A file that cannot be read is
    /// [`DefError::Io`]; a leading UTF-8 byte-order mark is skipped.
    pub fn read_file(path: &Path) -> Result<Overlay, DefError> {
        let text = std::fs::read_to_string(path).map_err(|e| DefError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if text.trim_start().starts_with('{') {
            Self::from_live_capture(&name, text)
        } else {
            Self::from_text(&name, OverlayKind::Test, text)
        }
    }
}

fn shape(object: &str, reason: &str) -> DefError {
    DefError::OverlayShape {
        object: object.to_string(),
        reason: reason.to_string(),
    }
}

/// The value `tools/real_game/live_overlay.py`'s `clean` makes of a captured value, step for step: inside a JSON object
/// the entries that clean to `null`, `"None"`, `""`, `[]` or `{}` are dropped, and `BaseValueScaleConstant` is dropped
/// where it is 1.0 and no `BaseValueAttribute` is left; list elements are cleaned with the list's key; a
/// `ModifierType` given as 0, 1 or 2 becomes `MT_Scale`, `MT_PreAdd` or `MT_PostAdd` (the script's `ENUM_MODIFIER_TYPE`,
/// "observed order, same as the cooked decode"); a string holding a `'` becomes the text between its first two quotes
/// (`Class'path'` -> `path`).
fn clean(value: PatchValue, key: Option<&str>) -> PatchValue {
    match value {
        PatchValue::Fields(fields) => {
            let mut out: Vec<(String, PatchValue)> = fields
                .into_iter()
                .map(|(k, v)| {
                    let v = clean(v, Some(&k));
                    (k, v)
                })
                .filter(|(_, v)| !empty(v))
                .collect();
            let unit_scale = out.iter().any(|(k, v)| {
                k == "BaseValueScaleConstant"
                    && matches!(v, PatchValue::Number(n) if n.parse::<f64>() == Ok(1.0))
            });
            if unit_scale && !out.iter().any(|(k, _)| k == "BaseValueAttribute") {
                out.retain(|(k, _)| k != "BaseValueScaleConstant");
            }
            PatchValue::Fields(out)
        }
        PatchValue::List(items) => {
            PatchValue::List(items.into_iter().map(|v| clean(v, key)).collect())
        }
        PatchValue::Number(n) if key == Some("ModifierType") => match n.as_str() {
            "0" => PatchValue::Text("MT_Scale".into()),
            "1" => PatchValue::Text("MT_PreAdd".into()),
            "2" => PatchValue::Text("MT_PostAdd".into()),
            _ => PatchValue::Number(n),
        },
        PatchValue::Text(t) if t.contains('\'') => {
            PatchValue::Text(t.split('\'').nth(1).unwrap_or_default().to_string())
        }
        other => other,
    }
}

fn empty(v: &PatchValue) -> bool {
    match v {
        PatchValue::Null => true,
        PatchValue::Text(t) => t.is_empty() || t == "None",
        PatchValue::List(items) => items.is_empty(),
        PatchValue::Fields(fields) => fields.is_empty(),
        _ => false,
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

// ------------------------------------------------------------------------------------------------ typing a patch

/// What typing a patch needs from the store: the object space, a handle for an object path (not loaded), a class by
/// path, a struct's defaults. Errors are reasons, which the store turns into `DefError::BadPatch`.
pub(crate) trait Linking {
    fn space(&mut self) -> &mut ObjectSpace;
    fn object(&mut self, path: &str) -> Result<ObjRef, String>;
    fn class(&mut self, path: &str) -> Result<ClassId, String>;
    fn struct_defaults(&mut self, s: StructId) -> Result<Value, String>;
}

/// How deeply a patch value may nest structs and arrays (a Rust-only bound; the JSON reader allows more).
const MAX_VALUE_DEPTH: usize = 64;

/// Writes `value` at `path` into loaded object `obj` (module documentation, "How patches apply" and "Edge cases"),
/// checked by the heap, and returns the elements written (for `DefStore::origin`).
pub(crate) fn write_patch(
    l: &mut dyn Linking,
    obj: ObjRef,
    path: &PropertyPath,
    value: &PatchValue,
) -> Result<Vec<PropAddr>, String> {
    let (first, rest) = path
        .segments
        .split_first()
        .ok_or_else(|| "an empty property path".to_string())?;
    let space = l.space();
    let class = space.heap.get(obj).map_err(|e| e.to_string())?.class;
    let prop = space
        .names
        .find(&first.name)
        .and_then(|n| space.classes.prop(class, n))
        .ok_or_else(|| format!("the class declares no property {}", first.name))?;
    let decl = space
        .classes
        .decl(prop)
        .cloned()
        .ok_or_else(|| format!("no declaration for {}", first.name))?;
    let dim = decl.array_dim.max(1) as u32;
    let is_static = dim > 1 && !matches!(decl.kind, PropKind::Array(_));
    let mut steps: Vec<Step> = Vec::new();
    let mut kind = decl.kind.clone();
    let addr = if is_static {
        match first.index {
            Some(k) if k >= dim => {
                return Err(format!(
                    "index {k} is past the static array's {dim} elements"
                ));
            }
            Some(k) => PropAddr { prop, index: k },
            None => {
                if let (true, PatchValue::List(items)) = (rest.is_empty(), value) {
                    if items.len() != dim as usize {
                        return Err(format!(
                            "{} values for a static array of {dim} elements",
                            items.len()
                        ));
                    }
                    let mut typed_items = Vec::with_capacity(items.len());
                    for item in items {
                        typed_items.push(typed(l, &kind, item, 0)?);
                    }
                    let space = l.space();
                    let mut written = Vec::with_capacity(typed_items.len());
                    for (k, v) in typed_items.into_iter().enumerate() {
                        let at = PropAddr {
                            prop,
                            index: k as u32,
                        };
                        space
                            .heap
                            .write(&space.classes, obj, at, v)
                            .map_err(|e| e.to_string())?;
                        written.push(at);
                    }
                    return Ok(written);
                }
                PropAddr { prop, index: 0 }
            }
        }
    } else {
        if let Some(k) = first.index {
            match kind {
                PropKind::Array(inner) => {
                    steps.push(Step::Element(k));
                    kind = *inner;
                }
                _ => return Err(format!("{} is not an array", first.name)),
            }
        }
        PropAddr { prop, index: 0 }
    };
    for segment in rest {
        let PropKind::Struct(s) = kind else {
            return Err(format!("{} is not inside a struct", segment.name));
        };
        let space = l.space();
        let layout = space
            .classes
            .struct_layout(s)
            .ok_or_else(|| "a struct that is not defined".to_string())?;
        let (entry, field) = space
            .names
            .find(&segment.name)
            .and_then(|n| layout.field(n))
            .map(|(entry, decl)| (entry, decl.clone()))
            .ok_or_else(|| format!("the struct declares no field {}", segment.name))?;
        let field_dim = field.array_dim.max(1) as u32;
        if field_dim > 1 && !matches!(field.kind, PropKind::Array(_)) {
            let k = segment.index.unwrap_or(0);
            if k >= field_dim {
                return Err(format!(
                    "index {k} is past the field's {field_dim} elements"
                ));
            }
            steps.push(Step::Field(entry + k));
            kind = field.kind;
        } else {
            steps.push(Step::Field(entry));
            kind = field.kind;
            if let Some(k) = segment.index {
                match kind {
                    PropKind::Array(inner) => {
                        steps.push(Step::Element(k));
                        kind = *inner;
                    }
                    _ => return Err(format!("{} is not an array", segment.name)),
                }
            }
        }
    }
    let v = typed(l, &kind, value, 0)?;
    let space = l.space();
    space
        .heap
        .write_in(&space.classes, obj, addr, &steps, v)
        .map_err(|e| e.to_string())?;
    Ok(vec![addr])
}

/// The number text of a float: JSON's grammar, or Python's non-finite spellings. Read as a double and rounded to
/// single precision, as the capture's floats were widened from single precision by the SDK.
fn float_of(text: &str) -> Option<f32> {
    match text {
        "NaN" => Some(f32::NAN),
        "Infinity" => Some(f32::INFINITY),
        "-Infinity" => Some(f32::NEG_INFINITY),
        _ => text.parse::<f64>().ok().map(|d| d as f32),
    }
}

/// `v` typed as a value of `kind` (module documentation).
fn typed(
    l: &mut dyn Linking,
    kind: &PropKind,
    v: &PatchValue,
    depth: usize,
) -> Result<Value, String> {
    if depth > MAX_VALUE_DEPTH {
        return Err("the value nests too deep".to_string());
    }
    let wrong = || {
        format!(
            "a {} value does not fit a {} property",
            what(v),
            kind.kind_name()
        )
    };
    Ok(match (kind, v) {
        (PropKind::Int | PropKind::IntAttribute, PatchValue::Number(n)) => {
            Value::Int(n.parse::<i32>().map_err(|_| format!("{n} is not an int"))?)
        }
        (PropKind::Float | PropKind::FloatAttribute, PatchValue::Number(n)) => {
            Value::Float(float_of(n).ok_or_else(|| format!("{n} is not a float"))?)
        }
        (PropKind::Bool, PatchValue::Bool(b)) => Value::Bool(*b),
        (PropKind::Byte { .. } | PropKind::ByteAttribute { .. }, PatchValue::Number(n)) => {
            Value::Byte(n.parse::<u8>().map_err(|_| format!("{n} is not a byte"))?)
        }
        (PropKind::Byte { enum_ } | PropKind::ByteAttribute { enum_ }, PatchValue::Text(t)) => {
            let space = l.space();
            let position = enum_
                .and_then(|e| space.classes.enum_layout(e))
                .zip(space.names.find(t))
                .and_then(|(layout, n)| layout.index_of(n))
                .ok_or_else(|| format!("{t} is not a value of the declared enum"))?;
            Value::Byte(u8::try_from(position).map_err(|_| format!("{t} is past 255"))?)
        }
        (PropKind::Str, PatchValue::Text(t)) => Value::string(t),
        (PropKind::Name, PatchValue::Text(t)) => Value::Name(l.space().names.intern(t)),
        (PropKind::Object { .. }, PatchValue::Null) => Value::Object(None),
        (PropKind::Interface { .. }, PatchValue::Null) => Value::Interface(None),
        (PropKind::Class { .. }, PatchValue::Null) => Value::Class(None),
        (PropKind::Object { .. }, PatchValue::Text(t)) if t == "None" => Value::Object(None),
        (PropKind::Interface { .. }, PatchValue::Text(t)) if t == "None" => Value::Interface(None),
        (PropKind::Class { .. }, PatchValue::Text(t)) if t == "None" => Value::Class(None),
        (PropKind::Object { .. }, PatchValue::Text(t)) => Value::Object(Some(l.object(t)?)),
        (PropKind::Interface { .. }, PatchValue::Text(t)) => Value::Interface(Some(l.object(t)?)),
        (PropKind::Class { .. }, PatchValue::Text(t)) => Value::Class(Some(l.class(t)?)),
        (PropKind::Struct(s), PatchValue::Fields(fields)) => {
            let mut value = l.struct_defaults(*s)?;
            for (name, field_value) in fields {
                let space = l.space();
                let layout = space
                    .classes
                    .struct_layout(*s)
                    .ok_or_else(|| "a struct that is not defined".to_string())?;
                let (entry, decl) = space
                    .names
                    .find(name)
                    .and_then(|n| layout.field(n))
                    .map(|(entry, decl)| (entry as usize, decl.clone()))
                    .ok_or_else(|| format!("the struct declares no field {name}"))?;
                let dim = decl.array_dim.max(1) as usize;
                let entries: Vec<Value> = match (
                    dim > 1 && !matches!(decl.kind, PropKind::Array(_)),
                    field_value,
                ) {
                    (true, PatchValue::List(items)) if items.len() == dim => {
                        let mut out = Vec::with_capacity(dim);
                        for item in items {
                            out.push(typed(l, &decl.kind, item, depth + 1)?);
                        }
                        out
                    }
                    (true, PatchValue::List(items)) => {
                        return Err(format!(
                            "{} values for the field {name} of {dim} elements",
                            items.len()
                        ));
                    }
                    _ => vec![typed(l, &decl.kind, field_value, depth + 1)?],
                };
                let fields = &mut value
                    .struct_mut()
                    .ok_or_else(|| "struct defaults that are not a struct".to_string())?
                    .fields;
                for (k, entry_value) in entries.into_iter().enumerate() {
                    let slot = fields
                        .get_mut(entry + k)
                        .ok_or_else(|| format!("the field {name} is past the struct's entries"))?;
                    *slot = entry_value;
                }
            }
            value
        }
        (PropKind::Array(inner), PatchValue::List(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(typed(l, inner, item, depth + 1)?);
            }
            Value::array(out)
        }
        _ => return Err(wrong()),
    })
}

fn what(v: &PatchValue) -> &'static str {
    match v {
        PatchValue::Null => "null",
        PatchValue::Bool(_) => "bool",
        PatchValue::Number(_) => "number",
        PatchValue::Text(_) => "string",
        PatchValue::List(_) => "list",
        PatchValue::Fields(_) => "object",
    }
}

// -------------------------------------------------------------------------------------------------------- JSON

/// A JSON reader into [`PatchValue`]s: RFC 8259, plus the non-finite numbers Python writes (`NaN`, `Infinity`,
/// `-Infinity`). Object keys keep their order (a repeated key is kept twice). Nesting is bounded.
pub(crate) mod json {
    use super::PatchValue;
    use crate::store::DefError;

    /// How deeply arrays and objects may nest (a Rust-only bound against stack overflow).
    pub(crate) const MAX_DEPTH: usize = 128;

    /// The one JSON value `text` holds, with only white space around it.
    pub(crate) fn parse_document(text: &str) -> Result<PatchValue, DefError> {
        let mut p = Parser { text, at: 0 };
        p.ws();
        let value = p.value(0)?;
        p.ws();
        if p.at != text.len() {
            return Err(p.error("text after the value"));
        }
        Ok(value)
    }

    struct Parser<'a> {
        text: &'a str,
        at: usize,
    }

    impl Parser<'_> {
        fn bytes(&self) -> &[u8] {
            self.text.as_bytes()
        }

        fn peek(&self) -> Option<u8> {
            self.bytes().get(self.at).copied()
        }

        fn error(&self, reason: &str) -> DefError {
            let before = &self.text[..self.at.min(self.text.len())];
            let line = before.matches('\n').count() + 1;
            let line_start = before.rfind('\n').map_or(0, |i| i + 1);
            DefError::OverlaySyntax {
                line,
                column: before[line_start..].chars().count() + 1,
                reason: reason.to_string(),
            }
        }

        fn ws(&mut self) {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.at += 1;
            }
        }

        fn eat(&mut self, literal: &str) -> bool {
            if self.text[self.at..].starts_with(literal) {
                self.at += literal.len();
                true
            } else {
                false
            }
        }

        fn value(&mut self, depth: usize) -> Result<PatchValue, DefError> {
            if depth > MAX_DEPTH {
                return Err(self.error("values nest too deep"));
            }
            match self.peek() {
                Some(b'{') => self.object(depth),
                Some(b'[') => self.array(depth),
                Some(b'"') => Ok(PatchValue::Text(self.string()?)),
                Some(b't') if self.eat("true") => Ok(PatchValue::Bool(true)),
                Some(b'f') if self.eat("false") => Ok(PatchValue::Bool(false)),
                Some(b'n') if self.eat("null") => Ok(PatchValue::Null),
                Some(b'N') if self.eat("NaN") => Ok(PatchValue::Number("NaN".into())),
                Some(b'I') if self.eat("Infinity") => Ok(PatchValue::Number("Infinity".into())),
                Some(b'-') if self.eat("-Infinity") => Ok(PatchValue::Number("-Infinity".into())),
                Some(b'-' | b'0'..=b'9') => self.number(),
                None => Err(self.error("a value is missing")),
                Some(_) => Err(self.error("not a JSON value")),
            }
        }

        fn object(&mut self, depth: usize) -> Result<PatchValue, DefError> {
            self.at += 1;
            let mut fields = Vec::new();
            self.ws();
            if self.peek() == Some(b'}') {
                self.at += 1;
                return Ok(PatchValue::Fields(fields));
            }
            loop {
                self.ws();
                if self.peek() != Some(b'"') {
                    return Err(self.error("a string key expected"));
                }
                let key = self.string()?;
                self.ws();
                if self.peek() != Some(b':') {
                    return Err(self.error("':' expected"));
                }
                self.at += 1;
                self.ws();
                let value = self.value(depth + 1)?;
                fields.push((key, value));
                self.ws();
                match self.peek() {
                    Some(b',') => self.at += 1,
                    Some(b'}') => {
                        self.at += 1;
                        return Ok(PatchValue::Fields(fields));
                    }
                    _ => return Err(self.error("',' or '}' expected")),
                }
            }
        }

        fn array(&mut self, depth: usize) -> Result<PatchValue, DefError> {
            self.at += 1;
            let mut items = Vec::new();
            self.ws();
            if self.peek() == Some(b']') {
                self.at += 1;
                return Ok(PatchValue::List(items));
            }
            loop {
                self.ws();
                items.push(self.value(depth + 1)?);
                self.ws();
                match self.peek() {
                    Some(b',') => self.at += 1,
                    Some(b']') => {
                        self.at += 1;
                        return Ok(PatchValue::List(items));
                    }
                    _ => return Err(self.error("',' or ']' expected")),
                }
            }
        }

        fn digits(&mut self) -> usize {
            let start = self.at;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.at += 1;
            }
            self.at - start
        }

        fn number(&mut self) -> Result<PatchValue, DefError> {
            let start = self.at;
            if self.peek() == Some(b'-') {
                self.at += 1;
            }
            match self.peek() {
                Some(b'0') => self.at += 1,
                Some(b'1'..=b'9') => {
                    self.digits();
                }
                _ => return Err(self.error("a digit expected")),
            }
            if self.peek() == Some(b'.') {
                self.at += 1;
                if self.digits() == 0 {
                    return Err(self.error("a digit expected after '.'"));
                }
            }
            if matches!(self.peek(), Some(b'e' | b'E')) {
                self.at += 1;
                if matches!(self.peek(), Some(b'+' | b'-')) {
                    self.at += 1;
                }
                if self.digits() == 0 {
                    return Err(self.error("a digit expected in the exponent"));
                }
            }
            Ok(PatchValue::Number(self.text[start..self.at].to_string()))
        }

        fn hex4(&mut self) -> Result<u32, DefError> {
            let digits = self
                .text
                .get(self.at..self.at + 4)
                .filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()))
                .ok_or_else(|| self.error("four hex digits expected after \\u"))?;
            let value =
                u32::from_str_radix(digits, 16).map_err(|_| self.error("bad \\u escape"))?;
            self.at += 4;
            Ok(value)
        }

        fn string(&mut self) -> Result<String, DefError> {
            self.at += 1;
            let mut out = String::new();
            loop {
                let Some(c) = self.text[self.at..].chars().next() else {
                    return Err(self.error("the string does not end"));
                };
                match c {
                    '"' => {
                        self.at += 1;
                        return Ok(out);
                    }
                    '\\' => {
                        self.at += 1;
                        let Some(e) = self.peek() else {
                            return Err(self.error("the string does not end"));
                        };
                        self.at += 1;
                        match e {
                            b'"' => out.push('"'),
                            b'\\' => out.push('\\'),
                            b'/' => out.push('/'),
                            b'b' => out.push('\u{8}'),
                            b'f' => out.push('\u{c}'),
                            b'n' => out.push('\n'),
                            b'r' => out.push('\r'),
                            b't' => out.push('\t'),
                            b'u' => {
                                let unit = self.hex4()?;
                                let code = if (0xD800..0xDC00).contains(&unit) {
                                    if !self.eat("\\u") {
                                        return Err(self.error("a lone high surrogate"));
                                    }
                                    let low = self.hex4()?;
                                    if !(0xDC00..0xE000).contains(&low) {
                                        return Err(
                                            self.error("a high surrogate without its low half")
                                        );
                                    }
                                    0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
                                } else {
                                    unit
                                };
                                out.push(
                                    char::from_u32(code)
                                        .ok_or_else(|| self.error("a lone low surrogate"))?,
                                );
                            }
                            _ => return Err(self.error("an unknown escape")),
                        }
                    }
                    c if (c as u32) < 0x20 => {
                        return Err(self.error("a control character inside a string"));
                    }
                    c => {
                        out.push(c);
                        self.at += c.len_utf8();
                    }
                }
            }
        }
    }
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
    fn json_reads_python_s_output_with_its_non_finite_numbers() {
        let v = json::parse_document(
            r#" {"b": [1, -0.5, 1e3, NaN, Infinity, -Infinity], "a": {"x": "q\"\u00e9\ud83d\ude00", "n": null, "t": true}} "#,
        )
        .unwrap();
        let n = |s: &str| PatchValue::Number(s.to_string());
        assert_eq!(
            v,
            PatchValue::Fields(vec![
                (
                    "b".into(),
                    PatchValue::List(vec![
                        n("1"),
                        n("-0.5"),
                        n("1e3"),
                        n("NaN"),
                        n("Infinity"),
                        n("-Infinity")
                    ])
                ),
                (
                    "a".into(),
                    PatchValue::Fields(vec![
                        ("x".into(), PatchValue::Text("q\"\u{e9}\u{1f600}".into())),
                        ("n".into(), PatchValue::Null),
                        ("t".into(), PatchValue::Bool(true)),
                    ])
                ),
            ])
        );
        assert_eq!(float_of("NaN").map(f32::is_nan), Some(true));
        assert_eq!(float_of("-Infinity"), Some(f32::NEG_INFINITY));
        assert_eq!(float_of("0.30000001192092896"), Some(0.3f32));
    }

    #[test]
    fn json_errors_say_where() {
        for (text, line, column) in [
            ("{\n  \"a\": 01}", 2, 9),
            ("[1,]", 1, 4),
            ("\"\\ud800\"", 1, 8),
            ("[1] x", 1, 5),
            ("{\"a\" 1}", 1, 6),
            ("\"tab\there\"", 1, 5),
            ("", 1, 1),
        ] {
            match json::parse_document(text) {
                Err(DefError::OverlaySyntax {
                    line: l, column: c, ..
                }) => assert_eq!((l, c), (line, column), "{text:?}"),
                other => panic!("{text:?}: {other:?}"),
            }
        }
        let deep = "[".repeat(json::MAX_DEPTH + 2);
        assert!(matches!(
            json::parse_document(&deep),
            Err(DefError::OverlaySyntax { .. })
        ));
    }

    #[test]
    fn a_live_capture_reads_as_live_overlay_py_cleans_it() {
        // Invented paths and values, in the shape weapon_dump.py writes.
        let capture = r#"{"GD_Test.Parts.Part_A": {"class": "WeaponPartDefinition", "properties": {
            "ClipSize": {"BaseValueConstant": 12.0, "BaseValueAttribute": "None", "InitializationDefinition": null,
                         "BaseValueScaleConstant": 1.0},
            "WeaponAttributeEffects": [{"AttributeToModify": "AttributeDefinition'GD_Test.Attr.Damage'",
                                        "ModifierType": 1, "BaseModifierValue": {"BaseValueConstant": 0.5}}],
            "Rarity": {}}},
          "GD_Test.Parts.Part_B": {"class": "WeaponPartDefinition", "properties": {"Priority": 2.5}}}"#;
        let o = Overlay::from_live_capture("cap", capture).unwrap();
        assert_eq!(o.kind, OverlayKind::LiveCapture);
        let got: Vec<(&str, &str, &PatchValue)> = o
            .patches
            .iter()
            .map(|p| (p.object.as_str(), p.property.as_str(), &p.value))
            .collect();
        let n = |s: &str| PatchValue::Number(s.to_string());
        assert_eq!(got.len(), 4);
        assert_eq!(
            got[0],
            (
                "GD_Test.Parts.Part_A",
                "ClipSize",
                &PatchValue::Fields(vec![("BaseValueConstant".into(), n("12.0"))])
            )
        );
        assert_eq!(
            got[1].2,
            &PatchValue::List(vec![PatchValue::Fields(vec![
                (
                    "AttributeToModify".into(),
                    PatchValue::Text("GD_Test.Attr.Damage".into())
                ),
                ("ModifierType".into(), PatchValue::Text("MT_PreAdd".into())),
                (
                    "BaseModifierValue".into(),
                    PatchValue::Fields(vec![("BaseValueConstant".into(), n("0.5"))])
                ),
            ])])
        );
        // An empty top-level value is still a patch (it sets the property to its empty value).
        assert_eq!(
            got[2],
            (
                "GD_Test.Parts.Part_A",
                "Rarity",
                &PatchValue::Fields(vec![])
            )
        );
        assert_eq!(got[3], ("GD_Test.Parts.Part_B", "Priority", &n("2.5")));
        for bad in [
            "[1]",
            r#"{"A.B": 3}"#,
            r#"{"A.B": {"class": "X"}}"#,
            r#"{"A.B": {"properties": {}, "extra": 1}}"#,
            r#"{"A..B": {"properties": {}}}"#,
        ] {
            assert!(
                matches!(
                    Overlay::from_live_capture("x", bad),
                    Err(DefError::OverlayShape { .. })
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn the_text_format_reads_one_patch_per_line() {
        let text = "# invented values\n\nGD_Test.Part_A ClipSize = 12\n  GD_Test.Part_A Pair[1].Label=\"x y\"\n";
        let o = Overlay::from_text("t", OverlayKind::TextMod, text).unwrap();
        assert_eq!(o.kind, OverlayKind::TextMod);
        assert_eq!(o.patches.len(), 2);
        assert_eq!(o.patches[1].property, "Pair[1].Label");
        assert_eq!(o.patches[1].value, PatchValue::Text("x y".into()));
        for (bad, column) in [
            ("GD_Test.A ClipSize 12", 20),
            ("GD_Test.A ClipSize = [1,", 25),
            ("GD_Test.A", 10),
        ] {
            match Overlay::from_text("t", OverlayKind::Test, bad) {
                Err(DefError::OverlaySyntax {
                    line: 1, column: c, ..
                }) => {
                    assert_eq!(c, column, "{bad}")
                }
                other => panic!("{bad}: {other:?}"),
            }
        }
        assert!(matches!(
            Overlay::from_text("t", OverlayKind::Test, "GD_Test.A Bad[x] = 1"),
            Err(DefError::BadPropertyPath { .. })
        ));
    }

    #[test]
    fn a_file_is_read_by_its_first_character() {
        let dir = std::env::temp_dir().join(format!("ow-defs-overlay-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let live = dir.join("live.json");
        std::fs::write(&live, "\u{feff}  {\"A.B\": {\"properties\": {\"X\": 1}}}").unwrap();
        let text = dir.join("mod.txt");
        std::fs::write(&text, "A.B X = 2\n").unwrap();
        let a = Overlay::read_file(&live).unwrap();
        assert_eq!(
            (a.name.as_str(), a.kind),
            ("live.json", OverlayKind::LiveCapture)
        );
        let b = Overlay::read_file(&text).unwrap();
        assert_eq!((b.name.as_str(), b.kind), ("mod.txt", OverlayKind::Test));
        assert!(matches!(
            Overlay::read_file(&dir.join("missing.txt")),
            Err(DefError::Io { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
