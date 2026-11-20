//! `WillowGame.DamageTypeAttributeValueResolver` on invented data (AGENTS.md rule 9: the damage type names
//! `TestDmg_*` are invented; only the class and field names are the game's, as the resolver reads them). Lane
//! IMPL-DAMAGE-TYPE-RESOLVER.

use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::Origin;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;

use ow_package::store::PackageStore;

use super::damage_type::{CLASS, DAMAGE_TYPE_RECORD, DamageType};
use super::testworld::{Fx, system};
use crate::api::{AttrError, EvalCx, InitData, ValueResolver};

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

/// The invented `ValueIfNotMatched` of the fixtures: not 0, so a hardcoded 0 fails (review r1 of #584, item 2).
const FALLBACK: f32 = 7.0;

/// The resolver over `entries` (name, `ValueIfMatched`), with `ValueIfNotMatched` [`FALLBACK`].
fn resolver(fx: &mut Fx, entries: &[(&str, f32)]) -> ObjRef {
    let sel = fx.c.damage_sel;
    let mut list = Vec::new();
    for (n, v) in entries {
        let (name, value) = (fx.name(n), fx.k(*v));
        list.push(fx.st(sel, vec![("AssociatedDamageTypeName", name), ("ValueIfMatched", value)]));
    }
    let zero = fx.k(FALLBACK);
    let c = fx.by_path(CLASS);
    fx.obj(c, "DamageType", vec![("DamageTypes", Value::array(list)), ("ValueIfNotMatched", zero)])
}

/// An attribute over the resolver `r` behind a context resolver aimed at `context` (built before a counter is read).
fn attr_for(fx: &mut Fx, r: ObjRef, context: ObjRef) -> ObjRef {
    let ctx = fx.target(Some(context));
    fx.attr("WeaponDamage_Is_Test", vec![ctx], vec![r])
}

/// The value of `attr`, building nothing.
fn read_attr(fx: &mut Fx, attr: ObjRef) -> Result<Option<f32>, AttrError> {
    let sys = system();
    let mut cx = EvalCx::new(&mut fx.w);
    Ok(sys.value_of(attr, None, None, &mut cx)?.map(|v| v.value))
}

/// The value of an attribute over the resolver `r` behind a context resolver aimed at `context`.
fn value(fx: &mut Fx, r: ObjRef, context: ObjRef) -> Result<Option<f32>, AttrError> {
    let attr = attr_for(fx, r, context);
    let sys = system();
    let mut cx = EvalCx::new(&mut fx.w);
    Ok(sys.value_of(attr, None, None, &mut cx)?.map(|v| v.value))
}

/// Writes property `name` of `o` through its slot (no class check: a store handle may be set in a test-world field).
fn set(fx: &mut Fx, o: ObjRef, name: &str, v: Value) {
    let class = fx.w.space.heap.get(o).unwrap().class;
    let p = fx.w.space.prop(class, name).unwrap();
    *fx.w.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
}

fn not_ported(e: AttrError) -> String {
    match e {
        AttrError::NotYet(n) if n.missing == Missing::NotPorted => n.what.into_owned(),
        other => panic!("expected NotYet {{ NotPorted }}, got {other:?}"),
    }
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
    for (dmg, expected) in [("TestDmg_Acid", 1.0f32), ("TestDmg_Plain", FALLBACK)] {
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
    for (custom, expected) in [(Some("TestDmg_Acid"), 1.0f32), (None, FALLBACK)] {
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
    assert_eq!(value(&mut fx, r, gun), Ok(Some(FALLBACK)), "the part is not read without a type");
    let t = weapon_type(&mut fx, None);
    let gun = weapon(&mut fx, Some(t), None);
    assert_eq!(value(&mut fx, r, gun), Ok(Some(FALLBACK)));
}

/// An empty list is `ValueIfNotMatched`; with several matching entries the first one's value is taken. Each entry
/// carries its own `ValueIfMatched` (the installed class layout), so a match answers that entry's value; the installed
/// lists all hold 1 (review r1 of #584, item 4).
#[test]
fn an_empty_list_is_not_matched_and_the_first_match_wins() {
    let mut fx = Fx::new();
    let d = damage_type(&mut fx, "TestDmg_Acid");
    let t = weapon_type(&mut fx, Some(d));
    let gun = weapon(&mut fx, Some(t), None);
    let empty = resolver(&mut fx, &[]);
    assert_eq!(value(&mut fx, empty, gun), Ok(Some(FALLBACK)));
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

    // A cold resolver (a store handle, called directly as a value or base-value read would reach it): refused by
    // name before any field of it is read, with nothing loaded or written (review r1 of #584, item 1).
    let gun = weapon(&mut fx, Some(type_handle), None);
    let before = counters(&fx);
    let sys = system();
    for base in [false, true] {
        let mut cx = EvalCx::new(&mut fx.w);
        let e = if base {
            DamageType.base_value(&sys, part_handle, gun, 0.0, &mut cx)
        } else {
            DamageType.value(&sys, part_handle, gun, 0.0, &mut cx)
        };
        let what = not_read(e.unwrap_err());
        assert!(what.contains("the resolver") && what.contains("a load is a write"), "{what}");
    }
    assert_eq!(counters(&fx), before, "no write, no load");
    assert!(fx.w.defs.as_ref().unwrap().is_unloaded(&fx.w.space, part_handle));

    // A fallback whose InitializationDefinition is a cold handle: refused before it is touched (item 2).
    let plain = damage_type(&mut fx, "TestDmg_Plain");
    let warm_type = weapon_type(&mut fx, Some(plain));
    let warm_gun = weapon(&mut fx, Some(warm_type), None);
    let cold_fallback = resolver(&mut fx, &[]);
    let data = fx.of_def(part_handle);
    set(&mut fx, cold_fallback, "ValueIfNotMatched", data);
    let at = attr_for(&mut fx, cold_fallback, warm_gun);
    let before = counters(&fx);
    let what = not_ported(read_attr(&mut fx, at).unwrap_err());
    assert!(what.contains("ValueIfNotMatched"), "{what}");
    assert_eq!(counters(&fx), before, "no write, no load");
    assert!(fx.w.defs.as_ref().unwrap().is_unloaded(&fx.w.space, part_handle));

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

/// Only constant `AttributeInitializationData` is evaluated: a matched entry or a fallback with a
/// `BaseValueAttribute` or an `InitializationDefinition` is `NotYet { NotPorted }` with nothing written (review r1 of
/// #584, item 2). A scaled constant is its constant times its scale.
#[test]
fn only_constant_values_are_evaluated() {
    let mut fx = Fx::new();
    let d = damage_type(&mut fx, "TestDmg_Acid");
    let t = weapon_type(&mut fx, Some(d));
    let gun = weapon(&mut fx, Some(t), None);
    let attr = fx.constant_attr("Inner", 4.0);
    let def = fx.definition(vec![]);
    let scaled = fx.data(InitData { base_value_scale_constant: 3.0, ..InitData::constant(2.0) });
    let r = resolver(&mut fx, &[("TestDmg_Acid", 1.0)]);
    let sel = fx.c.damage_sel;
    let n = fx.name("TestDmg_Acid");
    let entry = fx.st(sel, vec![("AssociatedDamageTypeName", n.clone()), ("ValueIfMatched", scaled)]);
    set(&mut fx, r, "DamageTypes", Value::array(vec![entry]));
    assert_eq!(value(&mut fx, r, gun), Ok(Some(6.0)));
    for (which, data) in [("attribute", fx.of_attr(attr)), ("definition", fx.of_def(def))] {
        let entry = fx.st(sel, vec![("AssociatedDamageTypeName", n.clone()), ("ValueIfMatched", data.clone())]);
        set(&mut fx, r, "DamageTypes", Value::array(vec![entry]));
        let unmatched = resolver(&mut fx, &[]);
        set(&mut fx, unmatched, "ValueIfNotMatched", data);
        let (matched_attr, unmatched_attr) = (attr_for(&mut fx, r, gun), attr_for(&mut fx, unmatched, gun));
        let writes = fx.w.space.heap.committed_writes();
        let what = not_ported(read_attr(&mut fx, matched_attr).unwrap_err());
        assert!(what.contains("a ValueIfMatched with"), "{which}: {what}");
        let what = not_ported(read_attr(&mut fx, unmatched_attr).unwrap_err());
        assert!(what.contains("a ValueIfNotMatched with"), "{which}: {what}");
        assert_eq!(fx.w.space.heap.committed_writes(), writes, "{which}");
    }
}

/// Reference keeps the refusal of the unregistered class: the loading and resident preflights, a value, a base value
/// and the storage (review r1 of #584, item 3).
#[test]
fn reference_keeps_the_unregistered_refusal() {
    let mut fx = Fx::new();
    fx.w.defs = Some(DefStore::new(PackageStore::new_empty(), IdentityPolicy::FirstInScanOrder, ParityMode::Reference));
    let d = damage_type(&mut fx, "TestDmg_Acid");
    let t = weapon_type(&mut fx, Some(d));
    let gun = weapon(&mut fx, Some(t), None);
    let r = resolver(&mut fx, &[("TestDmg_Acid", 1.0)]);
    let ctx = fx.target(Some(gun));
    let attr = fx.attr("WeaponDamage_Is_Test", vec![ctx], vec![r]);
    let sys = system();
    let expected = format!("value resolver class {CLASS} [NATIVE_ATTRIBUTES.md section 6]");
    let writes = fx.w.space.heap.committed_writes();
    let mut cx = EvalCx::new(&mut fx.w);
    assert_eq!(not_read(sys.preflight_read(attr, &mut cx).unwrap_err()), expected, "preflight");
    assert_eq!(not_read(sys.preflight_read_resident(attr, &mut cx).unwrap_err()), expected, "resident preflight");
    assert_eq!(not_read(DamageType.value(&sys, r, gun, 0.0, &mut cx).unwrap_err()), expected, "value");
    assert_eq!(not_read(DamageType.base_value(&sys, r, gun, 0.0, &mut cx).unwrap_err()), expected, "base value");
    assert_eq!(not_read(DamageType.storage(&sys, r, gun, &mut cx).unwrap_err()), expected, "storage");
    assert_eq!(not_read(sys.value_of(attr, None, None, &mut cx).unwrap_err()), expected, "a read");
    assert_eq!(fx.w.space.heap.committed_writes(), writes);
}

/// The kind and text of a `NotYet`, or a panic.
fn not_yet(e: AttrError) -> (Missing, String) {
    match e {
        AttrError::NotYet(n) => (n.missing, n.what.into_owned()),
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// Review r2 of #584, item 1: in Reference the resident preflight refuses a cold handle of a class registered to this
/// resolver by the class's name, through the registry's dispatch, before any residency check. The store's class is
/// invented (`WillowGame.TestColdDamageTypeResolver`, registered to [`DamageType`] here): the test world already
/// declares the game's class path with its own layout, and the store would not bind a second one to it.
#[test]
fn reference_refuses_a_cold_resolver_by_its_class_first() {
    use ow_package::testkit::script::ScriptToy;
    // An invented WillowGame.upk declaring a resolver class (no fields), one export of it, and a holder naming it.
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let resolver_class = t.class("TestColdDamageTypeResolver", object);
    let holder_class = t.class("TestHolder", object);
    t.prop("Object", holder_class, "Resolver", 0, resolver_class);
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&t.none);
    let cold = t.p.add_export(resolver_class, "ColdResolver", empty);
    let mut holder = 0i32.to_le_bytes().to_vec();
    holder.extend(t.p.object_tag("Resolver", cold));
    holder.extend_from_slice(&t.none);
    t.p.add_export(holder_class, "TestHolderObject", holder);
    let bytes = t.build();
    let dir = std::env::temp_dir().join(format!("ow-attr-damage-type-ref-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut fx = Fx::new();
    let defs = DefStore::from_synthetic(
        &dir,
        &[("WillowGame.upk", bytes.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Reference,
    )
    .unwrap();
    fx.w.defs = Some(defs);
    let path = ow_core::ObjectPath::parse("WillowGame.TestHolderObject").unwrap();
    let holder = fx.w.defs.as_mut().unwrap().find(&mut fx.w.space, &path).unwrap().unwrap();
    let class = fx.w.space.heap.get(holder).unwrap().class;
    let p = fx.w.space.prop(class, "Resolver").unwrap();
    let handle = match fx.w.space.read(holder, PropAddr::scalar(p)).unwrap() {
        Value::Object(Some(h)) => *h,
        other => panic!("{other:?}"),
    };
    assert!(fx.w.defs.as_ref().unwrap().is_unloaded(&fx.w.space, handle), "the handle is cold");
    const COLD_CLASS: &str = "WillowGame.TestColdDamageTypeResolver";
    assert_eq!(crate::eval::class_path(&fx.w.space, handle), COLD_CLASS);
    let sys = crate::AttributeSystem::new_empty().with_value_resolver(COLD_CLASS, DamageType).unwrap();
    let warm = resolver(&mut fx, &[]);
    let attr = fx.attr("WeaponDamage_Is_Test", vec![], vec![warm]);
    set(&mut fx, attr, "ValueResolverChain", Value::array(vec![Value::Object(Some(handle))]));
    let loads = fx.w.defs.as_ref().unwrap().load_transitions();
    let e = sys.preflight_read_resident(attr, &mut EvalCx::new(&mut fx.w)).unwrap_err();
    assert_eq!(not_read(e), format!("value resolver class {COLD_CLASS} [NATIVE_ATTRIBUTES.md section 6]"));
    assert_eq!(fx.w.defs.as_ref().unwrap().load_transitions(), loads, "nothing loaded");
    assert!(fx.w.defs.as_ref().unwrap().is_unloaded(&fx.w.space, handle));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review r2 of #584, item 2: slotless resource objects keep the refusals of a loading read, checked without one:
/// `LayerAbsent` for `ow-defs` with no store, and `NotRead` for a stand-in the store leaves slotless; for the
/// resolver and for an item context alike.
#[test]
fn slotless_resolvers_and_contexts_keep_their_refusals() {
    for with_store in [false, true] {
        let mut fx = Fx::new();
        if with_store {
            fx.w.defs = Some(DefStore::new_empty());
        }
        let d = damage_type(&mut fx, "TestDmg_Acid");
        let t = weapon_type(&mut fx, Some(d));
        let gun = weapon(&mut fx, Some(t), None);
        let warm = resolver(&mut fx, &[("TestDmg_Acid", 1.0)]);
        let resolver_class = fx.by_path(CLASS);
        let at = Origin::Resource { package: 0, index: 1 };
        let cold_resolver = fx.w.space.alloc_stand_in(resolver_class, "StandInResolver", at).unwrap();
        let cold_gun = fx.w.space.alloc_stand_in(fx.c.weapon, "StandInWeapon", Origin::Resource { package: 0, index: 2 }).unwrap();
        let sys = system();
        for (case, r, ctx) in [("resolver", cold_resolver, gun), ("context", warm, cold_gun)] {
            let mut cx = EvalCx::new(&mut fx.w);
            let (missing, what) = not_yet(DamageType.value(&sys, r, ctx, 0.0, &mut cx).unwrap_err());
            if with_store {
                assert_eq!(missing, Missing::NotRead, "{case}");
                assert!(what.contains("stays a stand-in the definition store does not load"), "{case}: {what}");
            } else {
                assert_eq!(missing, Missing::LayerAbsent("ow-defs"), "{case}: {what}");
            }
        }
    }
}

/// Review r2 of #584, item 3: the list is checked whole on every path, a weapon with no damage type included, and the
/// fallback is read only when no entry matched.
#[test]
fn the_list_is_checked_on_every_path_and_the_fallback_read_last() {
    let mut fx = Fx::new();
    let d = damage_type(&mut fx, "TestDmg_Acid");
    let t = weapon_type(&mut fx, Some(d));
    let typed = weapon(&mut fx, Some(t), None);
    let untyped = weapon(&mut fx, None, None);
    let sys = system();
    // A list whose second entry's name is not a name: bad data for both weapons, even past the matching first entry.
    let bad = resolver(&mut fx, &[]);
    let sel = fx.c.damage_sel;
    let (acid, one) = (fx.name("TestDmg_Acid"), fx.k(1.0));
    let good = fx.st(sel, vec![("AssociatedDamageTypeName", acid), ("ValueIfMatched", one.clone())]);
    let broken = fx.st(sel, vec![("AssociatedDamageTypeName", Value::Int(3)), ("ValueIfMatched", one)]);
    set(&mut fx, bad, "DamageTypes", Value::array(vec![good, broken]));
    for gun in [typed, untyped] {
        let e = DamageType.value(&sys, bad, gun, 0.0, &mut EvalCx::new(&mut fx.w)).unwrap_err();
        assert!(matches!(e, AttrError::BadData { .. }), "{e:?}");
    }
    // A fallback that is not initialisation data: never read when an entry matches, bad data when none does.
    let r = resolver(&mut fx, &[("TestDmg_Acid", 2.0)]);
    set(&mut fx, r, "ValueIfNotMatched", Value::Int(3));
    assert_eq!(DamageType.value(&sys, r, typed, 0.0, &mut EvalCx::new(&mut fx.w)), Ok(2.0));
    let e = DamageType.value(&sys, r, untyped, 0.0, &mut EvalCx::new(&mut fx.w)).unwrap_err();
    assert!(matches!(e, AttrError::BadData { .. }), "{e:?}");
}
