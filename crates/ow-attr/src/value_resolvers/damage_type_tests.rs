//! `WillowGame.DamageTypeAttributeValueResolver` on invented data (AGENTS.md rule 9: the damage type names
//! `TestDmg_*` are invented; only the class and field names are the game's, as the resolver reads them). Lane
//! IMPL-DAMAGE-TYPE-RESOLVER.

use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;

use super::damage_type::{CLASS, DAMAGE_TYPE_RECORD};
use super::testworld::{Fx, system};
use crate::api::{AttrError, EvalCx};

/// A weapon whose `DefinitionData` holds `weapon_type` and `part`, written through the slot (the test world types
/// those fields for its own `Object`; a store handle is of another class).
fn weapon(fx: &mut Fx, weapon_type: Option<ObjRef>, part: Option<ObjRef>) -> ObjRef {
    let ty = fx.c.def_data;
    let data = fx.st(ty, vec![]);
    let gun = fx.obj(fx.c.weapon, "Weapon", vec![("DefinitionData", data)]);
    let p = fx.w.space.prop(fx.c.weapon, "DefinitionData").unwrap();
    let names = ["WeaponTypeDefinition", "ElementalPartDefinition"];
    for (n, v) in names.into_iter().zip([weapon_type, part]) {
        let id = fx.w.space.names.find(n).unwrap();
        let entry = fx.w.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0;
        let slot = fx.w.space.slot_mut(gun, PropAddr::scalar(p)).unwrap();
        slot.struct_mut().unwrap().fields[entry as usize] = Value::Object(v);
    }
    gun
}

fn damage_type(fx: &mut Fx, name: &str) -> ObjRef {
    let c = fx.c.damage_type;
    fx.obj(c, name, vec![])
}

fn weapon_type(fx: &mut Fx, default: Option<ObjRef>) -> ObjRef {
    let c = fx.c.weapon_type;
    fx.obj(c, "Type", vec![("DefaultDamageTypeDefinition", Value::Object(default))])
}

fn part(fx: &mut Fx, custom: Option<ObjRef>) -> ObjRef {
    let c = fx.c.elemental;
    fx.obj(c, "Part", vec![("CustomDamageTypeDefinition", Value::Object(custom))])
}

/// The resolver over `entries` (name, `ValueIfMatched`), with `ValueIfNotMatched` 0.
fn resolver(fx: &mut Fx, entries: &[(&str, f32)]) -> ObjRef {
    let sel = fx.c.damage_sel;
    let mut list = Vec::new();
    for (n, v) in entries {
        let (name, value) = (fx.name(n), fx.k(*v));
        list.push(fx.st(sel, vec![("AssociatedDamageTypeName", name), ("ValueIfMatched", value)]));
    }
    let zero = fx.k(0.0);
    let c = fx.by_path(CLASS);
    fx.obj(c, "DamageType", vec![("DamageTypes", Value::array(list)), ("ValueIfNotMatched", zero)])
}

/// The value of an attribute over the resolver `r` behind a context resolver aimed at `context`.
fn value(fx: &mut Fx, r: ObjRef, context: ObjRef) -> Result<Option<f32>, AttrError> {
    let ctx = fx.target(Some(context));
    let attr = fx.attr("WeaponDamage_Is_Test", vec![ctx], vec![r]);
    let sys = system();
    let mut cx = EvalCx::new(&mut fx.w);
    Ok(sys.value_of(attr, None, None, &mut cx)?.map(|v| v.value))
}

fn not_read(e: AttrError) -> String {
    match e {
        AttrError::NotYet(n) if n.missing == Missing::NotRead => format!("{} [{}]", n.what, n.record),
        other => panic!("expected NotYet {{ NotRead }}, got {other:?}"),
    }
}

/// The type's default damage type is the weapon's when no elemental part is set: in the list is the entry's value,
/// otherwise `ValueIfNotMatched`.
#[test]
fn the_default_damage_type_matches_by_name() {
    for (dmg, expected) in [("TestDmg_Acid", 1.0f32), ("TestDmg_Plain", 0.0)] {
        let mut fx = Fx::new();
        let d = damage_type(&mut fx, dmg);
        let t = weapon_type(&mut fx, Some(d));
        let gun = weapon(&mut fx, Some(t), None);
        let r = resolver(&mut fx, &[("TestDmg_Acid", 1.0), ("TestDmg_Acid_Lingering", 1.0)]);
        assert_eq!(value(&mut fx, r, gun), Ok(Some(expected)), "{dmg}");
    }
}

/// The elemental part's custom damage type replaces the default; a part with no custom type keeps the default.
#[test]
fn the_elemental_parts_custom_type_replaces_the_default() {
    for (custom, expected) in [(Some("TestDmg_Acid"), 1.0f32), (None, 0.0)] {
        let mut fx = Fx::new();
        let plain = damage_type(&mut fx, "TestDmg_Plain");
        let t = weapon_type(&mut fx, Some(plain));
        let c = custom.map(|n| damage_type(&mut fx, n));
        let p = part(&mut fx, c);
        let gun = weapon(&mut fx, Some(t), Some(p));
        let r = resolver(&mut fx, &[("TestDmg_Acid", 1.0)]);
        assert_eq!(value(&mut fx, r, gun), Ok(Some(expected)), "{custom:?}");
    }
}

/// A weapon with no type definition has no damage type (#576: a None type answers None, its elemental part unread),
/// and that matches no entry, not even one named `None`. A type with no default and no custom type matches none too.
#[test]
fn no_damage_type_matches_no_entry() {
    let mut fx = Fx::new();
    let acid = damage_type(&mut fx, "TestDmg_Acid");
    let p = part(&mut fx, Some(acid));
    let gun = weapon(&mut fx, None, Some(p));
    let r = resolver(&mut fx, &[("None", 1.0), ("TestDmg_Acid", 1.0)]);
    assert_eq!(value(&mut fx, r, gun), Ok(Some(0.0)), "the part is not read without a type");
    let t = weapon_type(&mut fx, None);
    let gun = weapon(&mut fx, Some(t), None);
    assert_eq!(value(&mut fx, r, gun), Ok(Some(0.0)));
}

/// An empty list is `ValueIfNotMatched`; with several matching entries the first one's value is taken.
#[test]
fn an_empty_list_is_not_matched_and_the_first_match_wins() {
    let mut fx = Fx::new();
    let d = damage_type(&mut fx, "TestDmg_Acid");
    let t = weapon_type(&mut fx, Some(d));
    let gun = weapon(&mut fx, Some(t), None);
    let empty = resolver(&mut fx, &[]);
    assert_eq!(value(&mut fx, empty, gun), Ok(Some(0.0)));
    let twice = resolver(&mut fx, &[("TestDmg_Other", 5.0), ("TestDmg_Acid", 2.0), ("TestDmg_Acid", 3.0)]);
    assert_eq!(value(&mut fx, twice, gun), Ok(Some(2.0)));
}

/// A context that is not an item (no `DefinitionData`) is refused, as the manufacturer resolver refuses it.
#[test]
fn a_non_item_context_is_not_read() {
    let mut fx = Fx::new();
    let other = fx.obj(fx.c.object, "NotAWeapon", vec![]);
    let r = resolver(&mut fx, &[("TestDmg_Acid", 1.0)]);
    let what = not_read(value(&mut fx, r, other).unwrap_err());
    assert!(what.contains("DamageTypeAttributeValueResolver") && what.contains("no DefinitionData"), "{what}");
}

/// The class is registered, and the resident preflight passes it with nothing written.
#[test]
fn the_class_is_registered_and_passes_the_resident_preflight() {
    let mut fx = Fx::new();
    let d = damage_type(&mut fx, "TestDmg_Acid");
    let t = weapon_type(&mut fx, Some(d));
    let gun = weapon(&mut fx, Some(t), None);
    let r = resolver(&mut fx, &[("TestDmg_Acid", 1.0)]);
    let ctx = fx.target(Some(gun));
    let attr = fx.attr("WeaponDamage_Is_Test", vec![ctx], vec![r]);
    let writes = fx.w.space.heap.committed_writes();
    assert_eq!(system().preflight_read_resident(attr, &mut EvalCx::new(&mut fx.w)), Ok(()));
    assert_eq!(fx.w.space.heap.committed_writes(), writes);
}

/// With a store: an unloaded weapon type definition or elemental part is refused by name with nothing loaded, and the
/// damage type handle a resident type names is matched by its name and left unloaded.
#[test]
fn store_handles_are_read_only_when_resident() {
    use ow_package::testkit::script::ScriptToy;
    // An invented package: a type class with DefaultDamageTypeDefinition, a part class with
    // CustomDamageTypeDefinition, a damage type class, and a holder whose fields name the exports as handles.
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let dmg_class = t.class("TestDamageType", object);
    let type_class = t.class("TestWeaponType", object);
    let part_class = t.class("TestElementalPart", object);
    let holder_class = t.class("TestHolder", object);
    t.prop("Object", type_class, "DefaultDamageTypeDefinition", 0, dmg_class);
    t.prop("Object", part_class, "CustomDamageTypeDefinition", 0, dmg_class);
    t.prop("Object", holder_class, "Type", 0, type_class);
    t.prop("Object", holder_class, "Part", 0, part_class);
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&t.none);
    let dmg = t.p.add_export(dmg_class, "TestDmg_Acid", empty.clone());
    let mut ty = 0i32.to_le_bytes().to_vec();
    ty.extend(t.p.object_tag("DefaultDamageTypeDefinition", dmg));
    ty.extend_from_slice(&t.none);
    let ty = t.p.add_export(type_class, "TestType", ty);
    let mut pa = 0i32.to_le_bytes().to_vec();
    pa.extend(t.p.object_tag("CustomDamageTypeDefinition", dmg));
    pa.extend_from_slice(&t.none);
    let pa = t.p.add_export(part_class, "TestPart", pa);
    let mut holder = 0i32.to_le_bytes().to_vec();
    holder.extend(t.p.object_tag("Type", ty));
    holder.extend(t.p.object_tag("Part", pa));
    holder.extend_from_slice(&t.none);
    t.p.add_export(holder_class, "TestHolderObject", holder);
    let bytes = t.build();
    let dir = std::env::temp_dir().join(format!("ow-attr-damage-type-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut fx = Fx::new();
    let defs =
        DefStore::from_synthetic(&dir, &[("Engine.upk", bytes.as_slice())], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
            .unwrap();
    fx.w.defs = Some(defs);
    let path = ow_core::ObjectPath::parse("Engine.TestHolderObject").unwrap();
    let holder = fx.w.defs.as_mut().unwrap().find(&mut fx.w.space, &path).unwrap().unwrap();
    let handle = |fx: &mut Fx, field: &str| {
        let class = fx.w.space.heap.get(holder).unwrap().class;
        let p = fx.w.space.prop(class, field).unwrap();
        match fx.w.space.read(holder, PropAddr::scalar(p)).unwrap() {
            Value::Object(Some(h)) => *h,
            other => panic!("{other:?}"),
        }
    };
    let (type_handle, part_handle) = (handle(&mut fx, "Type"), handle(&mut fx, "Part"));
    let r = resolver(&mut fx, &[("TestDmg_Acid", 1.0)]);
    let counters = |fx: &Fx| (fx.w.space.heap.committed_writes(), fx.w.defs.as_ref().unwrap().load_transitions());

    // The type is cold: refused by name, nothing loaded.
    let gun = weapon(&mut fx, Some(type_handle), None);
    let before = counters(&fx);
    let what = not_read(value(&mut fx, r, gun).unwrap_err());
    assert!(what.contains("the weapon type definition") && what.contains("a load is a write"), "{what}");
    assert!(what.contains(DAMAGE_TYPE_RECORD), "{what}");
    assert_eq!(counters(&fx).1, before.1, "no load");
    assert!(fx.w.defs.as_ref().unwrap().is_unloaded(&fx.w.space, type_handle));

    // The type resident, the part cold: refused by the part's name.
    fx.w.defs.as_mut().unwrap().ensure_loaded(&mut fx.w.space, type_handle).unwrap();
    let gun = weapon(&mut fx, Some(type_handle), Some(part_handle));
    let what = not_read(value(&mut fx, r, gun).unwrap_err());
    assert!(what.contains("the elemental part definition"), "{what}");
    assert!(fx.w.defs.as_ref().unwrap().is_unloaded(&fx.w.space, part_handle));

    // No part: the type's default handle is matched by its name and stays unloaded.
    let gun = weapon(&mut fx, Some(type_handle), None);
    let dmg_handle = {
        let class = fx.w.space.heap.get(type_handle).unwrap().class;
        let p = fx.w.space.prop(class, "DefaultDamageTypeDefinition").unwrap();
        match fx.w.space.read(type_handle, PropAddr::scalar(p)).unwrap() {
            Value::Object(Some(h)) => *h,
            other => panic!("{other:?}"),
        }
    };
    assert!(fx.w.defs.as_ref().unwrap().is_unloaded(&fx.w.space, dmg_handle));
    let loads = counters(&fx).1;
    assert_eq!(value(&mut fx, r, gun), Ok(Some(1.0)));
    assert_eq!(counters(&fx).1, loads, "the damage type is never loaded");
    assert!(fx.w.defs.as_ref().unwrap().is_unloaded(&fx.w.space, dmg_handle));
    let _ = std::fs::remove_dir_all(&dir);
}
