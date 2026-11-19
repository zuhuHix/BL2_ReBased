//! `StaticGetWeaponDamageType` in Off on an invented `WillowGame.upk` in a synthetic store (AGENTS.md rule 9: invented
//! classes and values). Loading `ToyHolder` leaves its references as store handles not yet loaded; a test loads the
//! ones it needs loaded. The argument struct is built by hand. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "IntProperty"]);
    let object = t.class("Object", 0);
    let weapon_type = t.class("ToyWeaponType", object);
    t.prop("Object", weapon_type, "DefaultDamageTypeDefinition", 0, object);
    let part = t.class("ToyPart", object);
    t.prop("Object", part, "CustomDamageTypeDefinition", 0, object);
    let plain = t.class("ToyPlain", object);
    let int_type = t.class("ToyIntType", object);
    t.prop("Int", int_type, "DefaultDamageTypeDefinition", 0, 0);
    let holder = t.class("Holder", object);
    for name in ["Type", "Part", "PartNone", "Plain", "IntType", "Normal", "Shock"] {
        t.prop("Object", holder, name, 0, object);
    }
    // A payload starts with its net index (0), then the tags, then None.
    let empty = || 0i32.to_le_bytes().to_vec();
    let with_none = |t: &ScriptToy, mut v: Vec<u8>| {
        v.extend_from_slice(&t.none);
        v
    };
    let normal = { let v = with_none(&t, empty()); t.p.add_export(plain, "DmgType_Normal", v) };
    let shock = { let v = with_none(&t, empty()); t.p.add_export(plain, "DmgType_Shock", v) };
    let mut v = empty();
    v.extend(t.p.object_tag("DefaultDamageTypeDefinition", normal));
    let v = with_none(&t, v);
    let ty = t.p.add_export(weapon_type, "WT_Toy", v);
    let mut v = empty();
    v.extend(t.p.object_tag("CustomDamageTypeDefinition", shock));
    let v = with_none(&t, v);
    let pt = t.p.add_export(part, "Part_Shock", v);
    let v = with_none(&t, empty());
    let pt_none = t.p.add_export(part, "Part_Plain", v);
    let v = with_none(&t, empty());
    let pl = t.p.add_export(plain, "Plain_Def", v);
    let v = with_none(&t, empty());
    let it = t.p.add_export(int_type, "WT_IntField", v);
    let mut h = empty();
    for (name, r) in [("Type", ty), ("Part", pt), ("PartNone", pt_none), ("Plain", pl), ("IntType", it), ("Normal", normal), ("Shock", shock)] {
        h.extend(t.p.object_tag(name, r));
    }
    let h = with_none(&t, h);
    t.p.add_export(holder, "ToyHolder", h);
    t.p.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    data: StructId,
    /// The argument struct without `ElementalPartDefinition`, and one with it as an int.
    no_part: StructId,
    int_part: StructId,
    no_type: StructId,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-damage-type-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    let object = vm.find_class("WillowGame.Object").unwrap();
    let o = PropKind::Object { class: object };
    let strukt = |vm: &mut Vm, path: &str, fields: &[(&str, PropKind)]| {
        let decls = fields.iter().map(|(n, k)| PropertyDecl::new(vm.space.names.intern(n), k.clone())).collect();
        vm.space.classes.define_struct(path, None, decls).unwrap()
    };
    let data = strukt(&mut vm, "Test.WeaponDefinitionData", &[("WeaponTypeDefinition", o.clone()), ("ElementalPartDefinition", o.clone())]);
    let no_part = strukt(&mut vm, "Test.NoPartData", &[("WeaponTypeDefinition", o.clone())]);
    let int_part = strukt(&mut vm, "Test.IntPartData", &[("WeaponTypeDefinition", o.clone()), ("ElementalPartDefinition", PropKind::Int)]);
    let no_type = strukt(&mut vm, "Test.NoTypeData", &[("ElementalPartDefinition", o)]);
    Fx { vm, dir, data, no_part, int_part, no_type }
}

impl Fx {
    /// The holder's reference `name`: a store handle, not loaded.
    fn handle(&mut self, name: &str) -> ObjRef {
        let path = ObjectPath::parse("WillowGame.ToyHolder").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        match crate::equip::read_prop(&self.vm, holder, name) {
            Some(Value::Object(Some(o))) => o,
            other => panic!("{name}: {other:?}"),
        }
    }

    fn loaded(&mut self, name: &str) -> ObjRef {
        let h = self.handle(name);
        self.vm.defs.ensure_loaded(&mut self.vm.space, h).unwrap();
        h
    }

    fn args(&self, ty: StructId, fields: &[(&str, Option<ObjRef>)]) -> Value {
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        for (name, o) in fields {
            let id = self.vm.space.names.find(name).unwrap();
            let at = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0;
            v.struct_mut().unwrap().fields[at as usize] = Value::Object(*o);
        }
        v
    }

    /// The answer, with nothing loaded or made by the call.
    fn answer(&mut self, data: &Value) -> Result<Option<ObjRef>, VmError> {
        let objects = self.vm.space.heap.len();
        let r = damage_type(&self.vm, data);
        assert_eq!(self.vm.space.heap.len(), objects, "nothing made");
        r
    }
}

fn refused(r: Result<Option<ObjRef>, VmError>, what: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!((&*n.what, n.missing, n.record), (what, Missing::NotRead, RECORD));
            assert!(!n.what.contains("partial writes"));
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn script(r: Result<Option<ObjRef>, VmError>, field: &str) {
    match r {
        Err(VmError::Script { message, .. }) => assert!(message.contains(field), "{field}: {message}"),
        other => panic!("expected a script error naming {field}, got {other:?}"),
    }
}

/// Checks 1 and 2: a None type answers None, whatever the elemental field holds or whether it exists.
#[test]
fn a_none_type_answers_none_without_the_elemental_field() {
    let mut x = fx(ParityMode::Off);
    let part = x.handle("Part");
    for data in [
        x.args(x.data, &[("WeaponTypeDefinition", None), ("ElementalPartDefinition", Some(part))]),
        x.args(x.no_part, &[]),
        x.args(x.int_part, &[]),
    ] {
        assert_eq!(x.answer(&data).unwrap(), None);
    }
    assert!(x.vm.defs.is_unloaded(&x.vm.space, part), "the part was not looked at, nor loaded");
    script(x.answer(&x.args(x.no_type, &[])), "WeaponTypeDefinition");
}

/// The rule: the type's default with no part or a part with no custom type; the part's custom type otherwise. The
/// damage type is returned as stored, never loaded.
#[test]
fn the_part_s_custom_type_replaces_the_type_s_default() {
    let mut x = fx(ParityMode::Off);
    let (ty, part, plain_part) = (x.loaded("Type"), x.loaded("Part"), x.loaded("PartNone"));
    let (normal, shock) = (x.handle("Normal"), x.handle("Shock"));
    let none = x.args(x.data, &[("WeaponTypeDefinition", Some(ty))]);
    assert_eq!(x.answer(&none).unwrap(), Some(normal));
    let custom = x.args(x.data, &[("WeaponTypeDefinition", Some(ty)), ("ElementalPartDefinition", Some(part))]);
    assert_eq!(x.answer(&custom).unwrap(), Some(shock));
    let plain = x.args(x.data, &[("WeaponTypeDefinition", Some(ty)), ("ElementalPartDefinition", Some(plain_part))]);
    assert_eq!(x.answer(&plain).unwrap(), Some(normal));
    assert!(x.vm.defs.is_unloaded(&x.vm.space, normal) && x.vm.defs.is_unloaded(&x.vm.space, shock), "never loaded");
}

/// Check 3, and the hub's point 2 on #576: an unloaded type refuses even when a loaded part has its own type.
#[test]
fn an_unloaded_type_refuses_over_a_loaded_part_with_its_own_type() {
    let mut x = fx(ParityMode::Off);
    let ty = x.handle("Type");
    let part = x.loaded("Part");
    let data = x.args(x.data, &[("WeaponTypeDefinition", Some(ty)), ("ElementalPartDefinition", Some(part))]);
    refused(x.answer(&data), TYPE_UNLOADED);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, ty), "nothing loaded");
}

/// Check 4 before the elemental part: a type whose class lacks the field (or declares it as an int) is a script error,
/// even with an unloaded part, a missing elemental field or a malformed part.
#[test]
fn the_type_s_field_is_checked_before_anything_of_the_part() {
    let mut x = fx(ParityMode::Off);
    let (plain, int_type) = (x.loaded("Plain"), x.loaded("IntType"));
    let unloaded_part = x.handle("Part");
    for ty in [plain, int_type] {
        let data = x.args(x.data, &[("WeaponTypeDefinition", Some(ty)), ("ElementalPartDefinition", Some(unloaded_part))]);
        script(x.answer(&data), "DefaultDamageTypeDefinition");
        script(x.answer(&x.args(x.no_part, &[("WeaponTypeDefinition", Some(ty))])), "DefaultDamageTypeDefinition");
        let both_bad = x.args(x.data, &[("WeaponTypeDefinition", Some(ty)), ("ElementalPartDefinition", Some(plain))]);
        script(x.answer(&both_bad), "DefaultDamageTypeDefinition");
    }
}

/// Checks 5 to 7: the elemental field, an unloaded part, a part whose class lacks its field.
#[test]
fn the_elemental_checks_follow_in_order() {
    let mut x = fx(ParityMode::Off);
    let ty = x.loaded("Type");
    script(x.answer(&x.args(x.no_part, &[("WeaponTypeDefinition", Some(ty))])), "ElementalPartDefinition");
    script(x.answer(&x.args(x.int_part, &[("WeaponTypeDefinition", Some(ty))])), "ElementalPartDefinition");
    let part = x.handle("Part");
    refused(
        x.answer(&x.args(x.data, &[("WeaponTypeDefinition", Some(ty)), ("ElementalPartDefinition", Some(part))])),
        PART_UNLOADED,
    );
    let plain = x.loaded("Plain");
    script(
        x.answer(&x.args(x.data, &[("WeaponTypeDefinition", Some(ty)), ("ElementalPartDefinition", Some(plain))])),
        "CustomDamageTypeDefinition",
    );
}

/// Off binds the key; Reference binds nothing (the stub policy).
#[test]
fn off_binds_and_reference_does_not() {
    let mut x = fx(ParityMode::Off);
    register(&mut x.vm).unwrap();
    assert!(x.vm.has_native(KEY));
    register(&mut x.vm).unwrap();
    let mut r = fx(ParityMode::Reference);
    register(&mut r.vm).unwrap();
    assert!(!r.vm.has_native(KEY));
}

/// The shared core only chooses between references already read.
#[test]
fn the_core_chooses_the_custom_type_else_the_default() {
    let mut x = fx(ParityMode::Off);
    let (a, b) = (x.handle("Normal"), x.handle("Shock"));
    assert_eq!(choose(Some(a), None), Some(a));
    assert_eq!(choose(Some(a), Some(b)), Some(b));
    assert_eq!(choose(None, None), None);
}
