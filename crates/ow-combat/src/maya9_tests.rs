//! `ApplyFlagInitializationData` over a store-decoded source array (lane MAYA9-SKIPPED-FLAG-LAYERABSENT;
//! NATIVE_MAYA9_AND_AFTER.md section 3.3 and test 8's third case). An invented `GearboxFramework.upk` (AGENTS.md rule
//! 9): a toy character class whose `DesignerFlagStartingValues` is an array of a struct, with three exports, one the
//! store skips (its tag says two elements and holds garbage), one decoded empty and one decoded with one entry. The
//! native is called as script calls it, its `const out` argument aliasing the export's property
//! ([`Arg::aliasing_property`]). Every call leaves #518's counters, `class_code_loads` and the name table unchanged.
//! AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{FuncId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::ints;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::{Arg, FunctionDecl, NativeCall, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const FIELD: &str = "DesignerFlagStartingValues";

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ScriptStruct", "ArrayProperty", "StructProperty", "IntProperty"]);
    let object = t.class("Object", 0);
    let char_class = t.class("ToyCharClass", object);
    let entry = toy_struct(&mut t, char_class, "FlagInitData", None);
    t.prop("Int", entry, "ToyValue", 0, 0);
    toy_array_of(&mut t, char_class, FIELD, "Struct", entry);
    t.class("FlagDefinition", object);
    let none = t.none;
    // Skipped: the tag says two elements and holds four bytes.
    let mut p = ints(&[0]);
    p.extend(t.p.array_tag(FIELD, 2, &ints(&[5])));
    p.extend_from_slice(&none);
    t.p.add_export(char_class, "Skipped_0", p);
    // Decoded empty.
    let mut p = ints(&[0]);
    p.extend(t.p.array_tag(FIELD, 0, &[]));
    p.extend_from_slice(&none);
    t.p.add_export(char_class, "Empty_0", p);
    // Decoded with one entry (an invented value).
    let mut body = t.p.int_tag("ToyValue", 7);
    body.extend_from_slice(&none);
    let mut p = ints(&[0]);
    p.extend(t.p.array_tag(FIELD, 1, &body));
    p.extend_from_slice(&none);
    t.p.add_export(char_class, "One_0", p);
    t.p.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    native: FuncId,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-combat-maya9-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs =
        DefStore::from_synthetic(&dir, &[("GearboxFramework.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
            .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let class = vm.find_class("GearboxFramework.ToyCharClass").unwrap();
    let object = vm.find_class("GearboxFramework.Object").unwrap();
    let field = vm.space.prop(class, FIELD).unwrap();
    let kind = vm.space.classes.decl(field).unwrap().kind.clone();
    let p = |vm: &mut Vm, n: &str, k: PropKind, f: u64| PropertyDecl::new(vm.space.names.intern(n), k).with_flags(f);
    // The declared signature: `const out` FlagsToSet (the installed flags), then the context object.
    let params = vec![
        p(&mut vm, "FlagsToSet", kind, PropertyDecl::PARAM | PropertyDecl::OUT),
        p(&mut vm, "ContextSource", PropKind::Object { class: object }, PropertyDecl::PARAM),
    ];
    let native = vm
        .declare_function(
            FunctionDecl::native(ObjectPath::parse(APPLY_FLAG_INITIALIZATION).unwrap(), APPLY_FLAG_INITIALIZATION)
                .with_static(true)
                .with_params(params),
        )
        .unwrap();
    Fx { vm, dir, native }
}

impl Fx {
    fn export(&mut self, name: &str) -> ObjRef {
        let path = ObjectPath::parse(&format!("GearboxFramework.{name}")).unwrap();
        let o = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        self.vm.defs.ensure_loaded(&mut self.vm.space, o).unwrap();
        o
    }

    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (
            vm.space.heap.committed_writes(),
            vm.rng.draws(),
            vm.defs.load_transitions(),
            vm.class_code_loads(),
            vm.space.names.len(),
        )
    }

    /// The native with its first argument aliasing `owner`'s array (as script passes it), or by value when
    /// `owner` is `None` (`value` then the array).
    fn call(&mut self, owner: Option<ObjRef>, value: Value) -> Result<Value, VmError> {
        let class = self.vm.find_class("GearboxFramework.Object").unwrap();
        let context = self.vm.space.alloc(class, "Context", None, Origin::Runtime).unwrap();
        let first = match owner {
            Some(o) => {
                let c = self.vm.space.heap.get(o).unwrap().class;
                let at = PropAddr::scalar(self.vm.space.prop(c, FIELD).unwrap());
                let v = self.vm.space.read(o, at).unwrap().clone();
                Arg::aliasing_property(o, at, v)
            }
            None => Arg::new(value, true, true),
        };
        let args = vec![first, Arg::new(Value::Object(Some(context)), false, true)];
        let before = self.marks();
        let f = self.native;
        let r = apply_flag_initialization(&mut NativeCall::new(&mut self.vm, f, None, args));
        assert_eq!(self.marks(), before, "nothing written, drawn, loaded or interned");
        r
    }
}

/// The note's test 8, third case: a source array the store skipped is `LayerAbsent`, not an empty array.
#[test]
fn a_skipped_source_array_is_layer_absent() {
    let mut x = fx();
    let skipped = x.export("Skipped_0");
    let class = x.vm.space.heap.get(skipped).unwrap().class;
    let prop = x.vm.space.prop(class, FIELD).unwrap();
    assert!(x.vm.defs.is_skipped(&x.vm.space, skipped, prop), "the store marked it");
    match x.call(Some(skipped), Value::None) {
        Err(VmError::NotYet(n)) => {
            assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            assert_eq!(n.what.as_ref(), FLAG_SKIPPED);
        }
        other => panic!("{other:?}"),
    }
}

/// A decoded source array answers as before: empty, nothing; one entry, the entry's `NotRead`. And an argument that
/// aliases no property (a call from the host) is answered by its value.
#[test]
fn a_decoded_source_array_answers_as_before() {
    let mut x = fx();
    let empty = x.export("Empty_0");
    assert_eq!(x.call(Some(empty), Value::None).unwrap(), Value::None, "decoded empty");
    let one = x.export("One_0");
    let class = x.vm.space.heap.get(one).unwrap().class;
    let at = PropAddr::scalar(x.vm.space.prop(class, FIELD).unwrap());
    assert_eq!(x.vm.space.read(one, at).unwrap().elements().map(<[Value]>::len), Some(1), "one entry decoded");
    match x.call(Some(one), Value::None) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, FLAG_ENTRY)),
        other => panic!("{other:?}"),
    }
    assert_eq!(x.call(None, Value::array(vec![])).unwrap(), Value::None, "by value, empty");
}
