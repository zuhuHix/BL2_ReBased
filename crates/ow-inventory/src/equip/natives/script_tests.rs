//! Review follow-up of #413: `AllParts` driven by an assembled script loop, through the VM's engine-iterator path
//! (`ow_vm::interp::iter`'s `engine`), not by calling the iterator directly. An invented `WillowGame.upk` (AGENTS.md
//! rule 9): a weapon class with a definition-data struct of eleven part slots, the native iterator `AllParts`, and
//! one script function. NATIVE_EQUIP_PARTS.md section 2: a loop that runs to its end leaves the out argument None.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::toy_struct;
use ow_package::testkit::script::{Asm, CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::error::VmError;

use super::WEAPON_ALL_PARTS;

const NATIVE_ITERATOR: u32 = 0x400 | 0x20000 | 0x4;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

/// `LastVisited()`: `foreach AllParts(DefinitionData, Cursor, false) { Seen = Cursor; } return Cursor;`, with the out
/// argument an instance variable (the toy's locals carry no class reference).
fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ScriptStruct",
        "BoolProperty",
        "ObjectProperty",
        "StructProperty",
    ]);
    let object = t.class("Object", 0);
    let weapon = t.class("WillowWeapon", object);
    let data = toy_struct(&mut t, weapon, "WeaponDefinitionData", None);
    for slot in WEAPON_ALL_PARTS {
        t.prop("Object", data, slot, 0, object);
    }
    let definition = t.prop("Struct", weapon, "DefinitionData", 0, data);
    let seen = t.prop("Object", weapon, "Seen", 0, object);
    let cursor = t.prop("Object", weapon, "Cursor", 0, object);
    let all_parts = t
        .function(
            weapon,
            "AllParts",
            &[
                Param {
                    type_ref: data,
                    ..Param::new("Struct", "DefinitionData")
                },
                Param {
                    flags: CPF_OUT,
                    type_ref: object,
                    ..Param::new("Object", "Part")
                },
                Param::new("Bool", "bIncludeNameParts"),
            ],
            None,
            &[],
            &FunctionSpec::new(NATIVE_ITERATOR, "AllParts"),
            |_| None,
        )
        .function;
    t.function(
        weapon,
        "LastVisited",
        &[],
        Some(("Object", object)),
        &[],
        &FunctionSpec::new(SCRIPT, "LastVisited"),
        |_| {
            let mut a = Asm::new();
            // Iterator(FinalFunction AllParts(DefinitionData, Cursor, false), <end>) ... IteratorNext; IteratorPop.
            a.stmt().raw(&[0x2F]).raw(&[0x1C]).reference(all_parts);
            a.instance(definition)
                .instance(cursor)
                .false_()
                .raw(&[0x16]);
            let at = a.bytes.len();
            a.w(0);
            a.stmt().raw(&[0x0F]).instance(seen).instance(cursor);
            a.stmt().raw(&[0x31]);
            a.patch(at);
            a.stmt().raw(&[0x30]);
            a.stmt().raw(&[0x04]).instance(cursor);
            a.end();
            Some(a)
        },
    );
    t.p.build()
}

struct Fx {
    vm: Vm,
    weapon: ObjRef,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fixture(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-allparts-script-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(
        &dir,
        &[("WillowGame.upk", &bytes)],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    )
    .with_stub_policy(StubPolicy::refuse_all());
    let class = vm.find_class("WillowGame.WillowWeapon").unwrap();
    let weapon = ow_vm::interp::instantiate_from_defaults(&mut vm, class).unwrap();
    super::register(&mut vm).unwrap();
    Fx { vm, weapon, dir }
}

impl Fx {
    fn part(&mut self, name: &str) -> ObjRef {
        let object = self.vm.find_class("WillowGame.Object").unwrap();
        self.vm
            .space
            .alloc(object, name, None, ow_object::heap::Origin::Runtime)
            .unwrap()
    }

    /// Sets the definition data's slots `(slot index, part)`; the others stay None.
    fn set_slots(&mut self, parts: &[(usize, ObjRef)]) {
        let class = self.vm.space.heap.get(self.weapon).unwrap().class;
        let prop = self.vm.space.prop(class, "DefinitionData").unwrap();
        let mut data = self
            .vm
            .space
            .read(self.weapon, PropAddr::scalar(prop))
            .unwrap()
            .clone();
        let ty = data.as_struct().unwrap().ty;
        for &(slot, part) in parts {
            let name = self.vm.space.names.find(WEAPON_ALL_PARTS[slot]).unwrap();
            let (i, _) = self
                .vm
                .space
                .classes
                .struct_layout(ty)
                .unwrap()
                .field(name)
                .unwrap();
            data.struct_mut().unwrap().fields[usize::try_from(i).unwrap()] =
                Value::Object(Some(part));
        }
        self.vm
            .space
            .write(self.weapon, PropAddr::scalar(prop), data)
            .unwrap();
    }

    fn seen(&self) -> Value {
        let class = self.vm.space.heap.get(self.weapon).unwrap().class;
        let prop = self.vm.space.prop(class, "Seen").unwrap();
        self.vm
            .space
            .read(self.weapon, PropAddr::scalar(prop))
            .unwrap()
            .clone()
    }

    fn last_visited(&mut self) -> Result<Value, VmError> {
        let f = self
            .vm
            .find_function("WillowGame.WillowWeapon.LastVisited")?;
        Ok(self.vm.call(f, Some(self.weapon), vec![])?.ret)
    }
}

#[test]
fn a_script_foreach_over_all_parts_leaves_the_out_argument_none_after_a_normal_end() {
    let mut x = fixture(ParityMode::Off);
    let (body, barrel, title) = (x.part("Body"), x.part("Barrel"), x.part("Title"));
    // Body (slot 0) and Barrel (slot 2) are visited; Title (slot 10, a name part) is not without the flag.
    x.set_slots(&[(0, body), (2, barrel), (10, title)]);
    let r = x.last_visited().unwrap();
    assert_eq!(
        x.seen(),
        Value::Object(Some(barrel)),
        "the body ran for each part, Barrel last"
    );
    assert_eq!(
        r,
        Value::Object(None),
        "the out argument is None after the loop ran to its end"
    );
    // No set slot: the body never runs, and the out argument is None as well.
    let mut x = fixture(ParityMode::Off);
    assert_eq!(x.last_visited().unwrap(), Value::Object(None));
    assert_eq!(x.seen(), Value::Object(None));
}

/// Reference: `AllParts` stays unbound, so the same loop is the refusing stub site.
#[test]
fn reference_refuses_the_unbound_all_parts_loop() {
    let mut x = fixture(ParityMode::Reference);
    let body = x.part("Body");
    x.set_slots(&[(0, body)]);
    match x.last_visited() {
        Err(VmError::UnboundNative(key)) => assert!(key.contains("AllParts"), "{key}"),
        other => panic!("expected UnboundNative, got {other:?}"),
    }
}
