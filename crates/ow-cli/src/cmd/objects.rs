//! Owner: lane W3-DECODE. Command: `--object-dump <export-index> <prefix> --cooked <directory> [--all]`, the port of
//! its branch in `src/cli.cpp` and of `valueJson`.
//!
//! The export is instantiated through `ow_object::cdo::instantiate_export` (its class's default object, then its own
//! tags from the prefix on) over a store of the cooked root, and printed as
//! `{"path":..,"class":..,"properties":{..},"log":[..]}`: the export's path, its class's `Package.Class` path, the
//! properties keyed by their lower-cased names in byte order, and the decode log. Without `--all`, only the properties
//! whose value differs from the class default object's are printed, compared as the C++ `sameValue` compares them
//! (a delegate never equals anything there, so every delegate property is printed; this is the C++ output rule, kept
//! by architecture R7). Values print as `valueJson` prints them: numbers, `true`/`false`, strings and names quoted, a
//! null reference as `null`, an object reference as its path in its package, a class as `class:<Package.Class>` (or
//! `class:None`), a delegate as `delegate:<function>`, a map as `null`, structs as objects of their fields and arrays
//! (static arrays too) as arrays, both cut to `"..."` past 12 levels. A struct's fields print in its declared order,
//! except the eight native structs, which print in the C++ order (`ow_object::decode::native_layout`).
//!
//! Order, as the C++: the source package is loaded first (before the options are checked), then the options (exactly
//! `--cooked <directory>` and an optional `--all`), the store scans the cooked root, the source package is loaded
//! again through the store by its name, and then the prefix and the index are read (the C++ reads them as the
//! arguments of one call; MSVC evaluates the prefix first).
//!
//! The parity mode is passed to the decoder: under `OW_PARITY=reference` arrays inside struct defaults stay empty and
//! enum names the C++ cannot place give 0, as in the C++ (`ow_object::decode`). Two differences from the C++ output
//! remain in both modes, because the typed slot model holds what the bag did not (decision 2026-11-15-08): a class
//! property prints `class:<Package>.<Class>` where the C++, which keeps an object stand-in there, prints the class's
//! object path (or `null` for an explicit null tag); and a property the class does not declare (an editor-only one
//! stripped from the cooked class, still tagged) is not printed, where the C++ bag prints it. Lines end in `\n` (the
//! parity harness normalises line ends).

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ow_core::fmt::fmt_ostream;
use ow_core::{FixedHashMap, PackageName, ParityMode};
use ow_object::cdo::{class_default, instantiate_export};
use ow_object::decode::{DecodeState, ValueCx, native_layout};
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::prefix::PropertyPrefix;
use ow_object::reflect::CodeIndex;
use ow_object::value::{Value, same_value};
use ow_package::package::Package;
use ow_package::store::PackageStore;

use crate::args::{Command, USAGE};
use crate::cmd::properties::{error_text, unsigned_number};
use crate::cmd::tables::signed_number;
use crate::json::quote;

/// Struct and array values print `"..."` for their members past this depth (the C++ `depth > 12`).
const MAX_PRINT_DEPTH: u32 = 12;

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let _ = command;
    match execute(args, mode) {
        Ok(out) => {
            // Ignore a failed write (a closed pipe), as the C++ binary does.
            let _ = std::io::stdout().lock().write_all(out.as_bytes());
            ExitCode::SUCCESS
        }
        Err(message) => {
            let _ = writeln!(std::io::stderr().lock(), "ow-package: {message}");
            ExitCode::from(1)
        }
    }
}

fn text(e: &dyn std::error::Error) -> String {
    error_text(e)
}

/// `args` is the command line without the program name: `args[0]` the source package, `args[1]` the mode.
fn execute(args: &[String], mode: ParityMode) -> Result<String, String> {
    let source = Package::load(Path::new(&args[0])).map_err(|e| text(&e))?;
    let everything = args.len() == 7;
    if (args.len() != 6 && args.len() != 7)
        || args[4] != "--cooked"
        || (everything && args[6] != "--all")
    {
        return Err(USAGE.to_string());
    }
    let mut store = PackageStore::open(PathBuf::from(&args[5])).map_err(|e| text(&e))?;
    let name = PackageName::parse(&source.name).map_err(|e| text(&e))?;
    let package = store.load(&name).map_err(|e| text(&e))?;
    let prefix = unsigned_number(&args[3])?;
    let index = signed_number(&args[2])?;
    let prefix = PropertyPrefix::from_bytes(prefix)
        .ok_or_else(|| ow_object::decode::DecodeError::UnsupportedRequest.to_string())?;

    let mut code = CodeIndex::new();
    let mut space = ObjectSpace::new();
    let mut state = DecodeState::new();
    let mut cx = ValueCx::new(&mut store, &mut code, &mut space, &mut state, mode);
    let object = instantiate_export(&mut cx, package, index, prefix).map_err(|e| text(&e))?;
    let class_id = cx.space.heap.get(object).map_err(|e| text(&e))?.class;
    let key = cx
        .code
        .key_of(class_id)
        .ok_or_else(|| "instantiated object has no loaded class".to_string())?;
    let defaults = class_default(&mut cx, key).map_err(|e| text(&e))?;

    let printer = Printer {
        store: cx.store,
        code: cx.code,
        space: cx.space,
    };
    let path = printer
        .store
        .get(package)
        .path_of(index)
        .map_err(|e| text(&e))?;
    let class_path = printer.code.class(key).path.clone();
    let mut out = format!(
        "{{\"path\":{},\"class\":{},\"properties\":{{",
        quote(&path),
        quote(&class_path)
    );
    let object_slots = &printer.space.heap.get(object).map_err(|e| text(&e))?.slots;
    let default_slots = &printer
        .space
        .heap
        .get(defaults)
        .map_err(|e| text(&e))?
        .slots;
    let mut first = true;
    for (key, (slot, decl)) in printer.properties(class_id)? {
        let value = printer.property(object_slots, slot, decl);
        if !everything && cpp_same(&printer.property(default_slots, slot, decl), &value) {
            continue;
        }
        if !first {
            out.push(',');
        }
        first = false;
        let _ = write!(out, "{}:", quote(&key));
        printer.value(&mut out, &value, 0)?;
    }
    out.push_str("},\"log\":[");
    for (i, line) in cx.state.log.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&quote(line));
    }
    out.push_str("]}\n");
    Ok(out)
}

/// The C++ `sameValue` for two values of one property (both from the same slots, so of the same kinds): what
/// [`same_value`] answers, except that a delegate equals nothing, inside structs and arrays too.
fn cpp_same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Delegate(_), _) | (_, Value::Delegate(_)) => false,
        (Value::Struct(x), Value::Struct(y)) => {
            x.fields.len() == y.fields.len()
                && x.fields.iter().zip(&y.fields).all(|(p, q)| cpp_same(p, q))
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| cpp_same(p, q))
        }
        _ => same_value(a, b),
    }
}

/// What printing reads.
struct Printer<'a> {
    store: &'a PackageStore,
    code: &'a CodeIndex,
    space: &'a ObjectSpace,
}

impl Printer<'_> {
    /// Every property of class `c` by lower-cased name, in byte order, with its first slot. Where two classes of the
    /// chain declare one name, the subclass's declaration is the one printed (the C++ bag keeps one value per name).
    fn properties(&self, c: ClassId) -> Result<Vec<(String, (usize, &PropertyDecl))>, String> {
        let mut by_name: FixedHashMap<String, (usize, &PropertyDecl)> = FixedHashMap::default();
        for (prop, decl) in self.space.classes.props(c).map_err(|e| text(&e))? {
            let key = self.space.names.text(decl.name).to_ascii_lowercase();
            by_name.insert(key, (prop.slot as usize, decl));
        }
        let mut sorted: Vec<_> = by_name.into_iter().collect();
        sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        Ok(sorted)
    }

    /// The value a property shows: its slot, or its slots as one array for a static array (the C++ form).
    fn property(&self, slots: &[Value], slot: usize, decl: &PropertyDecl) -> Value {
        if decl.array_dim > 1 && !matches!(decl.kind, PropKind::Array(_)) {
            Value::array(slots[slot..slot + decl.array_dim as usize].to_vec())
        } else {
            slots[slot].clone()
        }
    }

    /// `valueJson`.
    fn value(&self, out: &mut String, value: &Value, depth: u32) -> Result<(), String> {
        match value {
            Value::None => out.push_str("null"),
            Value::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Value::Byte(b) => {
                let _ = write!(out, "{b}");
            }
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Float(f) => out.push_str(&fmt_ostream(f64::from(*f))),
            Value::Str(s) => out.push_str(&quote(s)),
            Value::Name(n) => out.push_str(&quote(&self.space.names.text(*n))),
            Value::Delegate(d) => {
                let function = d
                    .map(|d| self.space.names.text(d.function).into_owned())
                    .unwrap_or_default();
                out.push_str(&quote(&format!("delegate:{function}")));
            }
            Value::Class(c) => {
                let path = match c {
                    Some(c) => self.class_path(*c),
                    None => "None".to_string(),
                };
                out.push_str(&quote(&format!("class:{path}")));
            }
            Value::Object(o) | Value::Interface(o) => match o {
                None => out.push_str("null"),
                Some(o) => out.push_str(&quote(&self.object_path(*o)?)),
            },
            Value::Struct(s) => self.structure(out, s.ty, &s.fields, depth)?,
            Value::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    self.member(out, item, depth)?;
                }
                out.push(']');
            }
        }
        Ok(())
    }

    fn member(&self, out: &mut String, value: &Value, depth: u32) -> Result<(), String> {
        if depth > MAX_PRINT_DEPTH {
            out.push_str("\"...\"");
            Ok(())
        } else {
            self.value(out, value, depth + 1)
        }
    }

    /// A struct's fields in declared order (static arrays as arrays), or a native struct's in the C++ order.
    fn structure(
        &self,
        out: &mut String,
        ty: StructId,
        fields: &[Value],
        depth: u32,
    ) -> Result<(), String> {
        let layout = self
            .space
            .classes
            .struct_layout(ty)
            .ok_or_else(|| "struct value of an undefined struct".to_string())?;
        let mut members: Vec<(String, Value)> = Vec::new();
        if let Some((names, _)) = native_layout(layout.path().name()) {
            for name in names {
                let found = self
                    .space
                    .names
                    .find(name)
                    .and_then(|n| layout.field(n))
                    .ok_or_else(|| format!("native struct field {name} is not declared"))?;
                members.push((name.to_string(), fields[found.0 as usize].clone()));
            }
        } else {
            for (first, decl) in layout.fields() {
                let first = first as usize;
                let value = if decl.array_dim > 1 && !matches!(decl.kind, PropKind::Array(_)) {
                    Value::array(fields[first..first + decl.array_dim as usize].to_vec())
                } else {
                    fields[first].clone()
                };
                members.push((self.space.names.text(decl.name).into_owned(), value));
            }
        }
        out.push('{');
        for (i, (name, value)) in members.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let _ = write!(out, "{}:", quote(name));
            self.member(out, value, depth)?;
        }
        out.push('}');
        Ok(())
    }

    fn class_path(&self, c: ClassId) -> String {
        match self.code.key_of(c) {
            Some(key) => self.code.class(key).path.clone(),
            None => self.space.classes.layout(c).path().as_str().to_string(),
        }
    }

    /// An object reference as the C++ prints it: a stand-in or an instantiated export by its path in its package,
    /// any other object by its name.
    fn object_path(&self, o: ObjRef) -> Result<String, String> {
        let object = self.space.heap.get(o).map_err(|e| text(&e))?;
        match object.origin {
            Origin::Resource { package, index } | Origin::Export { package, index } => {
                self.store.get(package).path_of(index).map_err(|e| text(&e))
            }
            _ => Ok(self.space.names.text(object.name).into_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_package::testkit::ints;
    use ow_package::testkit::reflect::{Declarations, PropSpec};

    /// `Core.upk`, every name and value invented: native structs Plane (extends Vector) and Color, a struct Pair with a
    /// static array, a map, a delegate, a class property and a static array on class Thing, whose default object
    /// sets some of them; export Item (a Thing) changes one.
    fn fixture() -> Vec<u8> {
        let mut d = Declarations::with_imports(&[
            "Class",
            "ScriptStruct",
            "Enum",
            "IntProperty",
            "FloatProperty",
            "NameProperty",
            "StructProperty",
            "ArrayProperty",
            "ObjectProperty",
            "ClassProperty",
            "DelegateProperty",
            "MapProperty",
            "ByteProperty",
        ]);
        let object = d.cls("Object", 0);
        d.default_object(object, "Object");
        let vector = d.struct_in(object, "Vector", 0, None);
        for f in ["Z", "Y", "X"] {
            d.prop("Float", vector, f, 0);
        }
        let plane = d.struct_in(object, "Plane", vector, None);
        d.prop("Float", plane, "W", 0);
        let color = d.struct_in(object, "Color", 0, None);
        for f in ["A", "R", "G", "B"] {
            d.prop("Byte", color, f, 0);
        }
        let pair = d.struct_in(object, "Pair", 0, None);
        d.prop_with(&PropSpec {
            dim: 2,
            ..PropSpec::new("Int", pair, "Two")
        });
        d.prop("Name", pair, "Label", 0);
        let thing = d.cls("Thing", object);
        let class_import = d.import("Class");
        d.prop("Map", thing, "Table", 0);
        d.prop("Delegate", thing, "OnDone", 0);
        d.prop_with(&PropSpec {
            tail: Some(ints(&[class_import, object])),
            ..PropSpec::new("Class", thing, "Kind")
        });
        d.prop("Object", thing, "Owner", object);
        d.array_of(thing, "List", "Int", 0);
        d.prop_with(&PropSpec {
            dim: 2,
            ..PropSpec::new("Int", thing, "Count")
        });
        d.prop("Struct", thing, "Pair", pair);
        d.prop("Struct", thing, "Col", color);
        d.prop("Struct", thing, "P", plane);
        d.prop("Float", thing, "F", 0);
        let mut plane_bytes = Vec::new();
        for v in [4.0f32, 0.1, 2.5, 3.0e10] {
            plane_bytes.extend_from_slice(&v.to_le_bytes());
        }
        let item = d.p.next_export() + 1;
        let label = d.p.name_tag("Label", "Tag");
        let mut pair_body = label;
        pair_body.extend_from_slice(&d.p.tag_full(
            "Two",
            1,
            "IntProperty",
            4,
            &[],
            &7i32.to_le_bytes(),
        ));
        pair_body.extend_from_slice(&d.none);
        let tags = [
            d.p.float_tag("F", 1.0 / 3.0),
            d.p.struct_tag("P", "Plane", &plane_bytes),
            d.p.struct_tag("Col", "Color", &[1, 2, 3, 4]),
            d.p.struct_tag("Pair", "Pair", &pair_body),
            d.p.tag_full("Count", 1, "IntProperty", 4, &[], &5i32.to_le_bytes()),
            d.p.array_tag("List", 2, &ints(&[8, 9])),
            d.p.object_tag("Owner", item),
            d.p.tag("Kind", "ClassProperty", &thing.to_le_bytes()),
        ];
        let mut payload = ints(&[0]);
        for t in &tags {
            payload.extend_from_slice(t);
        }
        payload.extend_from_slice(&d.none);
        d.p.add_export(thing, "Default__Thing", payload);
        let mut payload = ints(&[0]);
        payload.extend(d.p.float_tag("F", -2.0));
        payload.extend_from_slice(&d.none);
        assert_eq!(d.p.add_export(thing, "Item", payload), item);
        d.build()
    }

    fn run_dump(index: &str, all: bool, mode: ParityMode) -> Result<String, String> {
        let dir = std::env::temp_dir().join(format!(
            "ow-cli-w3decode-{}-{index}-{all}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let package = dir.join("Core.upk");
        std::fs::write(&package, fixture()).unwrap();
        let mut args: Vec<String> = [
            package.to_str().unwrap(),
            "--object-dump",
            index,
            "4",
            "--cooked",
            dir.to_str().unwrap(),
        ]
        .map(String::from)
        .to_vec();
        if all {
            args.push("--all".into());
        }
        let out = execute(&args, mode);
        let _ = std::fs::remove_dir_all(&dir);
        out
    }

    // Each expected text below is what the C++ binary printed for the same package (line end aside), checked locally.
    #[test]
    fn prints_what_the_cxx_value_json_prints() {
        assert_eq!(
            run_dump("29", true, ParityMode::Off).unwrap(),
            concat!(
                r#"{"path":"Default__Thing","class":"Core.Thing","properties":{"col":{"B":1,"G":2,"R":3,"A":4},"#,
                r#""count":[0,5],"f":0.333333,"kind":"class:Core.Thing","list":[8,9],"ondone":"delegate:","owner":"Item","#,
                r#""p":{"W":4,"X":0.1,"Y":2.5,"Z":3e+10},"pair":{"Label":"Tag","Two":[0,7]},"table":null},"log":[]}"#,
                "
"
            )
        );
        assert_eq!(
            run_dump("30", true, ParityMode::Off).unwrap(),
            concat!(
                r#"{"path":"Item","class":"Core.Thing","properties":{"col":{"B":1,"G":2,"R":3,"A":4},"count":[0,5],"#,
                r#""f":-2,"kind":"class:Core.Thing","list":[8,9],"ondone":"delegate:","owner":"Item","#,
                r#""p":{"W":4,"X":0.1,"Y":2.5,"Z":3e+10},"pair":{"Label":"Tag","Two":[0,7]},"table":null},"log":[]}"#,
                "
"
            )
        );
    }

    #[test]
    fn without_all_only_what_differs_from_the_class_default_prints_and_a_delegate_always_does() {
        assert_eq!(
            run_dump("30", false, ParityMode::Off).unwrap(),
            "{\"path\":\"Item\",\"class\":\"Core.Thing\",\"properties\":{\"f\":-2,\"ondone\":\"delegate:\"},\"log\":[]}
"
        );
        assert_eq!(
            run_dump("29", false, ParityMode::Reference).unwrap(),
            "{\"path\":\"Default__Thing\",\"class\":\"Core.Thing\",\"properties\":{\"ondone\":\"delegate:\"},\"log\":[]}
"
        );
    }

    #[test]
    fn errors_are_the_cxx_texts() {
        assert_eq!(
            run_dump("0", true, ParityMode::Off).unwrap_err(),
            "unsupported exported object request"
        );
        assert_eq!(
            run_dump("17", true, ParityMode::Off).unwrap_err(),
            "cannot instantiate a null class"
        );
        assert_eq!(
            run_dump("x", true, ParityMode::Off).unwrap_err(),
            "invalid stoll argument"
        );
    }

    #[test]
    fn the_cxx_same_value_never_equates_delegates() {
        let d = Value::Delegate(None);
        assert!(!cpp_same(&d, &d));
        assert!(!cpp_same(
            &Value::array(vec![d.clone()]),
            &Value::array(vec![d])
        ));
        assert!(cpp_same(&Value::Int(3), &Value::Int(3)));
        assert!(cpp_same(&Value::None, &Value::None));
        assert!(!cpp_same(
            &Value::array(vec![Value::Int(1)]),
            &Value::array(vec![])
        ));
    }
}
