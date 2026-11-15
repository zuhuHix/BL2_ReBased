//! Owner: lane W2-REFLECT. Reflection exports for synthetic packages, built on [`super::PackageBuilder`] (invented
//! values only, AGENTS.md rule 9). They re-express these Python helpers byte for byte (the tests at the end pin the
//! bytes against them):
//!
//! | Python helper | Here |
//! |---|---|
//! | `declarations(p, kinds)` of `tests/behavior_test.py` (from line 101) and its twin in `tests/kismet_test.py` (lines 84-96): `prop`, `array_of`, `struct_`, `cls`, `enum` | [`Declarations`] |
//! | `Toy.array_of`, `Toy.struct_` of `tests/combat_test.py` (lines 178-190) and `tests/mission_script_test.py` (lines 207-220) | [`toy_array_of`], [`toy_struct`] on W2-SCRIPT's [`ScriptToy`] (which has `cls` and `prop`) |
//! | `prop` of `build_package()` in `tests/vm_test.py` (line 92), with `dim` and its optional type word | [`property_payload`], [`Declarations::prop_with`] |
//! | `class_body()` of `tests/vm_test.py` (from line 113) | [`class_body`] with [`ClassBodySpec`] |
//! | `make_class()` of `vm_test.py`'s interface fixture (from line 619) | [`Declarations::real_class`] |
//!
//! The property header is the 40 bytes `src/vm.cpp` (`readProperty`) reads: net index, the `None` name, the next
//! field, the array dimension (low 16 bits), the 64-bit flags, the category name and one more word, then the type
//! references from byte 40 (byte 42 when flag `0x20` adds a 16-bit replication offset). The Class body is the layout
//! of `docs/verification/NATIVE_CLASS_SERIAL_LAYOUT.md` that `decodeClassBody` reads. An Enum export is
//! `[net index][None][next][count][count names]` (`src/vm.cpp` `enumPosition`, `src/behavior.cpp` `enumNames`).

use super::script::ScriptToy;
use super::{Export, PackageBuilder, ints};

/// The class flags `class_body()` writes by default (bit 1).
pub const DEFAULT_CLASS_FLAGS: u32 = 2;
/// Class flag bit 14: an interface class (`CLASS_INTERFACE` of `vm_test.py`).
pub const CLASS_INTERFACE: u32 = 0x4000;
/// Property flag `0x20`: a 16-bit replication offset follows the 40-byte header.
pub const CPF_NET: u64 = 0x20;

/// The payload of a property export: the 40-byte header with `dim` and `flags`, then `tail` (the type references).
/// `none` is the package's `None` FName.
pub fn property_payload(none: &[u8; 8], dim: u32, flags: u64, tail: &[u8]) -> Vec<u8> {
    let mut payload = ints(&[0]);
    payload.extend_from_slice(none);
    payload.extend_from_slice(&ints(&[0]));
    payload.extend_from_slice(&dim.to_le_bytes());
    payload.extend_from_slice(&flags.to_le_bytes());
    payload.extend_from_slice(none);
    payload.extend_from_slice(&ints(&[0]));
    payload.extend_from_slice(tail);
    payload
}

/// What [`class_body`] writes: `class_body(p, super_ref, within, default, flags, interfaces, first_child, script)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassBodySpec {
    pub super_ref: i32,
    pub within: i32,
    pub default: i32,
    pub flags: u32,
    /// `(interface class reference, table property reference)` pairs.
    pub interfaces: Vec<(i32, i32)>,
    pub first_child: i32,
    /// The class's own script bytes (no references, so the in-memory size equals the stored one).
    pub script: Vec<u8>,
}

impl ClassBodySpec {
    /// The Python defaults: flags 2, no interfaces, no first child, no script.
    pub fn new(super_ref: i32, within: i32, default: i32) -> Self {
        ClassBodySpec {
            super_ref,
            within,
            default,
            flags: DEFAULT_CLASS_FLAGS,
            interfaces: Vec::new(),
            first_child: 0,
            script: Vec::new(),
        }
    }
}

/// A Class export body in the layout of NATIVE_CLASS_SERIAL_LAYOUT.md (`class_body()` of `vm_test.py`, word for
/// word): the base-object and struct part, the state part (empty function map), the class part (config name `None`,
/// empty component map, the interface table, four empty name lists, the unknown number, an empty name list, an empty
/// string, the discarded name `None`, the unknown byte 1) and the default object reference.
pub fn class_body(p: &mut PackageBuilder, spec: &ClassBodySpec) -> Vec<u8> {
    let script_len = i32::try_from(spec.script.len()).expect("script fits i32");
    let interfaces = i32::try_from(spec.interfaces.len()).expect("interface count fits i32");
    let none = p.fname("None", 0);
    let mut body = ints(&[7]); // first word (unknown meaning)
    body.extend(ints(&[0, spec.super_ref, 0, spec.first_child, 0])); // next field, super, script text, children, C++ text
    body.extend(ints(&[-1, -1])); // source line, text position
    body.extend(ints(&[script_len, script_len])); // script size in memory, size in the file
    body.extend_from_slice(&spec.script);
    body.extend_from_slice(&0u32.to_le_bytes()); // probe mask
    body.extend_from_slice(&65_535u16.to_le_bytes()); // label table offset
    body.extend_from_slice(&2u16.to_le_bytes()); // state flags
    body.extend(ints(&[0])); // function map count
    body.extend_from_slice(&spec.flags.to_le_bytes());
    body.extend(ints(&[spec.within]));
    body.extend_from_slice(&none); // config name
    body.extend(ints(&[0])); // component map count
    body.extend(ints(&[interfaces]));
    for &(class, table) in &spec.interfaces {
        body.extend(ints(&[class, table]));
    }
    body.extend(ints(&[0, 0, 0, 0])); // four name lists
    body.extend(ints(&[0, 0, 0])); // unknown number, name list, empty string
    body.extend_from_slice(&none); // discarded name
    body.push(1); // unknown byte
    body.extend(ints(&[spec.default]));
    body
}

/// One property of [`Declarations::prop_with`]: `prop(kind, owner, name, flags, dim, type_ref, tail)` of
/// `vm_test.py`. The type word is written only when `type_ref` is not 0; `tail`, when given, replaces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropSpec {
    /// The property kind without `Property` (`Int`, `Struct`, ...).
    pub kind: String,
    pub owner: i32,
    pub name: String,
    pub flags: u64,
    pub dim: u32,
    pub type_ref: i32,
    pub tail: Option<Vec<u8>>,
}

impl PropSpec {
    /// A scalar with no flags and no type reference.
    pub fn new(kind: &str, owner: i32, name: &str) -> Self {
        PropSpec {
            kind: kind.to_owned(),
            owner,
            name: name.to_owned(),
            flags: 0,
            dim: 1,
            type_ref: 0,
            tail: None,
        }
    }
}

/// Reflection helpers over one package: `declarations(p, kinds)` of `behavior_test.py` and `kismet_test.py`. The
/// Core class imports are `Class`, `ScriptStruct`, `Enum`, then `<kind>Property` for each kind (or the list given
/// to [`Declarations::with_imports`]); `None` is named after them, as the Python does.
#[derive(Debug, Clone)]
pub struct Declarations {
    pub p: PackageBuilder,
    /// The `None` FName.
    pub none: [u8; 8],
    imports: Vec<(String, i32)>,
}

impl Declarations {
    /// `declarations(Package(), kinds)`.
    pub fn new(kinds: &[&str]) -> Self {
        let mut names: Vec<String> = ["Class", "ScriptStruct", "Enum"].map(String::from).to_vec();
        names.extend(kinds.iter().map(|k| format!("{k}Property")));
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        Declarations::with_imports(&names)
    }

    /// Imports exactly `names` (each of class `Core.Class`, in order), then names `None`; `build_package()` of
    /// `vm_test.py` uses its own list.
    pub fn with_imports(names: &[&str]) -> Self {
        let mut p = PackageBuilder::new_empty();
        let imports = names
            .iter()
            .map(|&n| (n.to_owned(), p.add_import("Core", "Class", 0, n)))
            .collect();
        let none = p.fname("None", 0);
        Declarations { p, none, imports }
    }

    /// The import reference of class `name`; panics (a test bug) when it was not imported.
    pub fn import(&self, name: &str) -> i32 {
        self.imports
            .iter()
            .find(|(n, _)| n == name)
            .map(|&(_, r)| r)
            .unwrap_or_else(|| panic!("no import {name}"))
    }

    /// `prop(kind, owner, name, type_ref)`: a scalar `<kind>Property` with no flags whose payload ends in the type
    /// word (written even when 0).
    pub fn prop(&mut self, kind: &str, owner: i32, name: &str, type_ref: i32) -> i32 {
        let payload = property_payload(&self.none, 1, 0, &ints(&[type_ref]));
        self.add_property(kind, owner, name, payload)
    }

    /// `vm_test.py`'s `prop(kind, owner, name, flags, dim, type_ref, tail)`.
    pub fn prop_with(&mut self, spec: &PropSpec) -> i32 {
        let tail = match (&spec.tail, spec.type_ref) {
            (Some(tail), _) => tail.clone(),
            (None, 0) => Vec::new(),
            (None, type_ref) => ints(&[type_ref]),
        };
        let payload = property_payload(&self.none, spec.dim, spec.flags, &tail);
        self.add_property(&spec.kind, spec.owner, &spec.name, payload)
    }

    fn add_property(&mut self, kind: &str, owner: i32, name: &str, payload: Vec<u8>) -> i32 {
        let mut export = Export::new(self.import(&format!("{kind}Property")), name, payload);
        export.outer = owner;
        self.p.add_export_with(export)
    }

    /// `array_of(owner, name, inner_kind, inner_ref)`: an `ArrayProperty` whose type word names the inner
    /// declaration, exported right after it with the array as its outer. Returns the array's reference.
    pub fn array_of(&mut self, owner: i32, name: &str, inner_kind: &str, inner_ref: i32) -> i32 {
        let index = self.p.next_export();
        self.prop("Array", owner, name, index + 1);
        self.prop(inner_kind, index, name, inner_ref);
        index
    }

    /// `struct_(name, defaults)`: a top-level `ScriptStruct` export. Without defaults the payload is 16 zero bytes;
    /// with them, a 52-byte zero header and then the default tags.
    pub fn struct_(&mut self, name: &str, defaults: Option<&[u8]>) -> i32 {
        let payload = struct_payload(defaults);
        let class = self.import("ScriptStruct");
        self.p.add_export(class, name, payload)
    }

    /// A `ScriptStruct` export inside `outer` with super struct `super_ref` (the Python helpers write neither; the
    /// payload is [`Declarations::struct_`]'s).
    pub fn struct_in(
        &mut self,
        outer: i32,
        name: &str,
        super_ref: i32,
        defaults: Option<&[u8]>,
    ) -> i32 {
        let mut export = Export::new(self.import("ScriptStruct"), name, struct_payload(defaults));
        export.outer = outer;
        export.super_ = super_ref;
        self.p.add_export_with(export)
    }

    /// `cls(name, super_ref)`: a Class export of class `Core.Class` with a 16-byte zero body (no decodable body).
    pub fn cls(&mut self, name: &str, super_ref: i32) -> i32 {
        let mut export = Export::new(self.import("Class"), name, ints(&[0, 0, 0, 0]));
        export.super_ = super_ref;
        self.p.add_export_with(export)
    }

    /// `enum(owner, name, values)`: an Enum export `[0][None][0][count][values as FNames]` in `owner`.
    pub fn enum_(&mut self, owner: i32, name: &str, values: &[&str]) -> i32 {
        let mut payload = ints(&[0]);
        payload.extend_from_slice(&self.none);
        payload.extend(ints(&[
            0,
            i32::try_from(values.len()).expect("value count fits i32"),
        ]));
        for v in values {
            payload.extend_from_slice(&self.p.fname(v, 0));
        }
        let mut export = Export::new(self.import("Enum"), name, payload);
        export.outer = owner;
        self.p.add_export_with(export)
    }

    /// `make_class(name, super_ref, flags, table, with_property)` of `vm_test.py`'s interface fixture: a Class
    /// export with class reference 0 and a decodable body whose within class is `within`; when `table_property` is
    /// given, a `StructProperty` child of that name follows it and every interface entry points at it; then
    /// `Default__<name>`, the default object the body names. Returns the class's reference.
    pub fn real_class(
        &mut self,
        name: &str,
        super_ref: i32,
        within: i32,
        flags: u32,
        interfaces: &[i32],
        table_property: Option<&str>,
    ) -> i32 {
        let at = self.p.next_export();
        let child = if table_property.is_some() { at + 1 } else { 0 };
        let default = at + 1 + i32::from(table_property.is_some());
        let spec = ClassBodySpec {
            flags,
            interfaces: interfaces.iter().map(|&c| (c, child)).collect(),
            first_child: child,
            ..ClassBodySpec::new(super_ref, within, default)
        };
        let body = class_body(&mut self.p, &spec);
        let mut export = Export::new(0, name, body);
        export.super_ = super_ref;
        self.p.add_export_with(export);
        if let Some(property) = table_property {
            self.prop_with(&PropSpec::new("Struct", at, property));
        }
        self.default_object(at, name);
        at
    }

    /// `Default__<name>` of class `class`, with the 12-byte payload the fixtures write (`w32(0) + None`).
    pub fn default_object(&mut self, class: i32, name: &str) -> i32 {
        let mut payload = ints(&[0]);
        payload.extend_from_slice(&self.none);
        self.p
            .add_export(class, &format!("Default__{name}"), payload)
    }

    /// The package bytes.
    pub fn build(&mut self) -> Vec<u8> {
        self.p.build()
    }
}

fn struct_payload(defaults: Option<&[u8]>) -> Vec<u8> {
    match defaults {
        None => ints(&[0, 0, 0, 0]),
        Some(tags) => {
            let mut payload = vec![0u8; 52];
            payload.extend_from_slice(tags);
            payload
        }
    }
}

/// `Toy.array_of(owner, name, inner_kind, inner_ref)` of `combat_test.py` / `mission_script_test.py` on W2-SCRIPT's
/// [`ScriptToy`]: an `ArrayProperty` naming the inner declaration exported right after it.
pub fn toy_array_of(
    t: &mut ScriptToy,
    owner: i32,
    name: &str,
    inner_kind: &str,
    inner_ref: i32,
) -> i32 {
    let index = t.p.next_export();
    t.prop("Array", owner, name, 0, index + 1);
    t.prop(inner_kind, index, name, 0, inner_ref);
    index
}

/// `Toy.struct_(owner, name, defaults)`: a `ScriptStruct` export in `owner` (16 zero bytes, or the 52-byte header and
/// the default tags).
pub fn toy_struct(t: &mut ScriptToy, owner: i32, name: &str, defaults: Option<&[u8]>) -> i32 {
    let mut export = Export::new(t.import("ScriptStruct"), name, struct_payload(defaults));
    export.outer = owner;
    t.p.add_export_with(export)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FNV-1a 64 of the bytes, for pins too long to write out.
    fn fnv1a64(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        })
    }

    /// `declarations(p, ('Int', 'Name', 'Struct', 'Array', 'Byte'))` of `behavior_test.py` with these calls, then
    /// `p.build()`: `bpd = cls('Thing')`, `enum(bpd, 'EKind', ['K_A', 'K_B'])`, `s = struct_('Pair')`,
    /// `prop('Int', s, 'Left')`, `prop('Name', s, 'Right')`, `t = struct_('Holder', tag bytes b'\x07' * 8)`,
    /// `prop('Struct', t, 'Inner', s)`, `array_of(bpd, 'Items', 'Struct', s)`, `prop('Byte', bpd, 'Kind', 2)`.
    /// Pinned from a local run of the Python helpers (2026-11-15).
    #[test]
    fn declarations_match_the_python_helpers() {
        let mut d = Declarations::new(&["Int", "Name", "Struct", "Array", "Byte"]);
        let bpd = d.cls("Thing", 0);
        let kinds = d.enum_(bpd, "EKind", &["K_A", "K_B"]);
        let s = d.struct_("Pair", None);
        d.prop("Int", s, "Left", 0);
        d.prop("Name", s, "Right", 0);
        let t = d.struct_("Holder", Some(&[7u8; 8]));
        d.prop("Struct", t, "Inner", s);
        let items = d.array_of(bpd, "Items", "Struct", s);
        d.prop("Byte", bpd, "Kind", kinds);
        assert_eq!((bpd, kinds, s, t, items), (1, 2, 3, 6, 8));
        let bytes = d.build();
        assert_eq!((bytes.len(), fnv1a64(&bytes)), PYTHON_DECLARATIONS);
    }

    /// `vm_test.py`'s `build_package()` imports (`Class`, `Function`, `IntProperty`, `StructProperty`,
    /// `FloatAttributeProperty`, `ArrayProperty`, `ObjectProperty`, `FloatProperty` in that order here), its `prop`
    /// with `dim` and flags, `class_body` and the interface fixture's `make_class`: `obj` = an Object export with
    /// `class_body(p, 0, 1, 2)` and `Default__Object`, then `make_class('IThing', obj, 2 | CLASS_INTERFACE)`,
    /// `make_class('Impl', obj, table=[thing], with_property='VfTable_IThing')`, then on Impl
    /// `prop('Int', impl, 'Slots', dim=3)`, `prop('FloatAttribute', impl, 'Speed', flags=1 << 63, tail=w32(12, 13))`
    /// and `prop('Float', impl, 'SpeedBase', flags=1 << 62)`. Pinned from a local run of the Python helpers.
    #[test]
    fn vm_test_helpers_match_the_python_helpers() {
        let mut d = Declarations::with_imports(&[
            "Class",
            "Function",
            "IntProperty",
            "StructProperty",
            "FloatAttributeProperty",
            "ArrayProperty",
            "ObjectProperty",
            "FloatProperty",
        ]);
        let object_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
        let obj = d.p.add_export(0, "Object", object_body);
        d.default_object(obj, "Object");
        let thing = d.real_class(
            "IThing",
            obj,
            obj,
            DEFAULT_CLASS_FLAGS | CLASS_INTERFACE,
            &[],
            None,
        );
        let implementer = d.real_class(
            "Impl",
            obj,
            obj,
            DEFAULT_CLASS_FLAGS,
            &[thing],
            Some("VfTable_IThing"),
        );
        let slots = d.prop_with(&PropSpec {
            dim: 3,
            ..PropSpec::new("Int", implementer, "Slots")
        });
        d.prop_with(&PropSpec {
            flags: 1 << 63,
            tail: Some(ints(&[12, 13])),
            ..PropSpec::new("FloatAttribute", implementer, "Speed")
        });
        d.prop_with(&PropSpec {
            flags: 1 << 62,
            ..PropSpec::new("Float", implementer, "SpeedBase")
        });
        assert_eq!((obj, thing, implementer, slots), (1, 3, 5, 8));
        let bytes = d.build();
        assert_eq!((bytes.len(), fnv1a64(&bytes)), PYTHON_VM_HELPERS);
    }

    /// `Toy(('Int', 'Array', 'Struct'))` of `mission_script_test.py` with `c = cls('Thing')`,
    /// `s = struct_(c, 'Pair')`, `prop('Int', s, 'A')`, `array_of(c, 'Pairs', 'Struct', s)`,
    /// `struct_(c, 'WithDefaults', b'\x01\x02')`. Pinned from a local run of the Python helpers.
    #[test]
    fn toy_helpers_match_the_python_toy() {
        let mut t = ScriptToy::new(&["Int", "Array", "Struct"]);
        let c = t.class("Thing", 0);
        let s = toy_struct(&mut t, c, "Pair", None);
        t.prop("Int", s, "A", 0, 0);
        let pairs = toy_array_of(&mut t, c, "Pairs", "Struct", s);
        let defaults = toy_struct(&mut t, c, "WithDefaults", Some(&[1, 2]));
        assert_eq!((c, s, pairs, defaults), (1, 2, 4, 6));
        let bytes = t.build();
        assert_eq!((bytes.len(), fnv1a64(&bytes)), PYTHON_TOY);
    }

    #[test]
    fn property_payload_is_the_forty_byte_header_and_the_tail() {
        let none = [9u8, 0, 0, 0, 0, 0, 0, 0];
        let payload = property_payload(&none, 4, 0x8000_0000_0000_0080, &ints(&[-3]));
        assert_eq!(payload.len(), 44);
        assert_eq!(&payload[16..20], &4u32.to_le_bytes());
        assert_eq!(&payload[20..28], &0x8000_0000_0000_0080u64.to_le_bytes());
        assert_eq!(&payload[40..44], &(-3i32).to_le_bytes());
    }

    const PYTHON_DECLARATIONS: (usize, u64) = (1757, 0x22e1_8cc7_8d94_5f78);
    const PYTHON_VM_HELPERS: (usize, u64) = (1975, 0x94c6_da99_de88_a76a);
    const PYTHON_TOY: (usize, u64) = (1154, 0xd671_4983_75c5_afa4);
}
