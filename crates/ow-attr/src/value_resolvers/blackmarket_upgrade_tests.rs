//! `BlackMarketUpgradeAttributeValueResolver` on a synthetic world (rule 9: invented objects and numbers; the class
//! and field names are the packages' names). Each test names the case of NATIVE_BLACKMARKET_UPGRADES.md ("note",
//! section 6, "Off answers, per case") it checks.

use ow_core::rand::{CrtStream, Rand15};
use ow_core::{Missing, NotYet, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::Origin;
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::store::PackageStore;

use super::blackmarket_upgrade::{ARRAY_WRITE, CACHE_WRITE, CLASS, CONTROLLER};
use super::testworld::{Fx, World, system};
use crate::api::{AttrError, AttrWorld, EngineQuery, EvalCx, GlobalSlots, NumForms, WorldParts};

/// The classes the resolver reads, beside the test world's.
struct Cls {
    upgrade: ClassId,
    definition: ClassId,
    controller: ClassId,
    mind: ClassId,
}

fn classes(fx: &mut Fx) -> Cls {
    let object = fx.c.object;
    let space = &mut fx.w.space;
    let d = |space: &mut ow_object::heap::ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(space.names.intern(n), k);
    let decls = vec![d(space, "CachedUpgradeIndex", PropKind::Int)];
    let upgrade = space.classes.define("WillowGame.BlackMarketUpgradeDefinition", Some(object), decls).unwrap();
    let decls = vec![d(space, "BlackMarketUpgrades", PropKind::Array(Box::new(PropKind::Object { class: object })))];
    let definition = space.classes.define("WillowGame.GlobalsDefinition", Some(object), decls).unwrap();
    let decls = vec![d(space, "BlackMarketUpgrades", PropKind::Array(Box::new(PropKind::Int)))];
    let controller = space.classes.define(CONTROLLER, Some(object), decls).unwrap();
    let decls = vec![d(space, "BlackMarketUpgrades", PropKind::Array(Box::new(PropKind::Int)))];
    let mind = space.classes.define("WillowGame.WillowMind", Some(object), decls).unwrap();
    Cls { upgrade, definition, controller, mind }
}

/// The test world with an answer to [`AttrWorld::globals_definition`] (`Err` for a world without engine globals),
/// counting how often it is asked.
struct GlobalsWorld<'a> {
    inner: &'a mut World,
    definition: Result<Option<ObjRef>, ()>,
    asked: u32,
}

impl AttrWorld for GlobalsWorld<'_> {
    fn parts(&mut self) -> WorldParts<'_> {
        self.inner.parts()
    }
    fn globals(&mut self) -> Result<ObjRef, AttrError> {
        self.inner.globals()
    }
    fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
        self.inner.game_info()
    }
    fn engine_query(&mut self, q: EngineQuery, source: ObjRef) -> Result<Option<ObjRef>, AttrError> {
        self.inner.engine_query(q, source)
    }
    fn globals_definition(&mut self) -> Result<Option<ObjRef>, AttrError> {
        self.asked += 1;
        self.definition
            .map_err(|()| NotYet::layer_absent("the engine globals object", "ow-world", "test world").into())
    }
    fn global_slots(&mut self) -> &mut GlobalSlots {
        self.inner.global_slots()
    }
    fn attribute_changed(&mut self, o: ObjRef, p: PropId, f: NumForms) -> Result<(), AttrError> {
        self.inner.attribute_changed(o, p, f)
    }
    fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        self.inner.rng(s)
    }
}

/// The `NotYet` inside an error, if it is one.
fn not_yet(e: &AttrError) -> Option<&NotYet> {
    match e {
        AttrError::NotYet(n) => Some(n),
        _ => None,
    }
}

fn upgrade(fx: &mut Fx, c: &Cls, name: &str, cached: i32) -> ObjRef {
    fx.obj(c.upgrade, name, vec![("CachedUpgradeIndex", Value::Int(cached))])
}

fn definition(fx: &mut Fx, c: &Cls, list: &[Option<ObjRef>]) -> ObjRef {
    let items = list.iter().map(|o| Value::Object(*o)).collect();
    fx.obj(c.definition, "Globals", vec![("BlackMarketUpgrades", Value::array(items))])
}

fn holder(fx: &mut Fx, class: ClassId, name: &str, levels: &[i32]) -> ObjRef {
    let items = levels.iter().map(|i| Value::Int(*i)).collect();
    fx.obj(class, name, vec![("BlackMarketUpgrades", Value::array(items))])
}

/// The installed shape: the attribute's context is `context` (a fixed test context), its one value resolver ours.
fn attr(fx: &mut Fx, context: ObjRef, upgrade: Option<ObjRef>) -> ObjRef {
    let c = fx.by_path(CLASS);
    let r = fx.obj(c, "BM", vec![("AssociatedBlackMarketUpgrade", Value::Object(upgrade))]);
    let ctx = fx.target(Some(context));
    fx.attr("Att_BM_Test", vec![ctx], vec![r])
}

/// The value and base value through a world answering `definition`, and how often the globals were asked.
fn read(fx: &mut Fx, attr: ObjRef, definition: Result<Option<ObjRef>, ()>, base: bool) -> (Result<f32, AttrError>, u32) {
    let sys = system();
    let mut w = GlobalsWorld { inner: &mut fx.w, definition, asked: 0 };
    let r = {
        let mut cx = EvalCx::new(&mut w);
        if base {
            sys.base_value_of(attr, None, None, &mut cx)
        } else {
            sys.value_of(attr, None, None, &mut cx)
        }
    };
    (r.map(|v| v.expect("the fixed context resolves").value), w.asked)
}

fn cache(fx: &Fx, upgrade: ObjRef) -> i32 {
    let p = fx.w.space.prop(fx.w.space.heap.get(upgrade).unwrap().class, "CachedUpgradeIndex").unwrap();
    match fx.w.space.read(upgrade, PropAddr::scalar(p)).unwrap() {
        Value::Int(i) => *i,
        other => panic!("{other:?}"),
    }
}

fn array(fx: &Fx, o: ObjRef) -> Vec<i32> {
    let p = fx.w.space.prop(fx.w.space.heap.get(o).unwrap().class, "BlackMarketUpgrades").unwrap();
    match fx.w.space.read(o, PropAddr::scalar(p)).unwrap() {
        Value::Array(items) => items.iter().map(|v| if let Value::Int(i) = v { *i } else { panic!("{v:?}") }).collect(),
        other => panic!("{other:?}"),
    }
}

/// Three listed upgrades (one gap) and an unlisted one, all cold, and the globals definition listing them.
fn setup(fx: &mut Fx, c: &Cls) -> ([ObjRef; 3], ObjRef, ObjRef) {
    let a = upgrade(fx, c, "UpgradeA", -1);
    let b = upgrade(fx, c, "UpgradeB", -1);
    let z = upgrade(fx, c, "UpgradeZ", -1);
    let unlisted = upgrade(fx, c, "Unlisted", -1);
    let def = definition(fx, c, &[Some(a), None, Some(b), Some(z)]);
    ([a, b, z], unlisted, def)
}

#[test]
fn cold_cache_accepted_controller_fills_every_cache_and_grows_the_array() {
    // Note section 6: the cold rule, then the zero-fill to index + 1, then the entry.
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let ([a, b, z], _, def) = setup(&mut fx, &c);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let at = attr(&mut fx, pc, Some(b));
    let (v, asked) = read(&mut fx, at, Ok(Some(def)), false);
    assert_eq!(v.unwrap(), 0.0);
    assert_eq!(asked, 1);
    assert_eq!((cache(&fx, a), cache(&fx, b), cache(&fx, z)), (0, 2, 3));
    assert_eq!(array(&fx, pc), vec![0, 0, 0]);
}

#[test]
fn a_warm_cache_skips_the_globals_and_keeps_existing_entries() {
    // Note section 6: warm cache, no globals lookup; growth keeps the entries; value and base value agree.
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let up = upgrade(&mut fx, &c, "Warm", 4);
    let pc = holder(&mut fx, c.controller, "PC", &[2, 7]);
    let at = attr(&mut fx, pc, Some(up));
    let (v, asked) = read(&mut fx, at, Err(()), false);
    assert_eq!((v.unwrap(), asked), (0.0, 0));
    assert_eq!(array(&fx, pc), vec![2, 7, 0, 0, 0]);
    let up1 = upgrade(&mut fx, &c, "Warm1", 1);
    let at = attr(&mut fx, pc, Some(up1));
    assert_eq!(read(&mut fx, at, Err(()), false).0.unwrap(), 7.0);
    assert_eq!(read(&mut fx, at, Err(()), true).0.unwrap(), 7.0);
    assert_eq!(array(&fx, pc), vec![2, 7, 0, 0, 0], "a long enough array is not written");
}

#[test]
fn a_rejected_context_still_fills_a_cold_cache_and_answers_zero() {
    // Note section 6: a context that is not a WillowPlayerController.
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let ([a, b, _], _, def) = setup(&mut fx, &c);
    let mind = holder(&mut fx, c.mind, "Mind", &[]);
    let at = attr(&mut fx, mind, Some(b));
    assert_eq!(read(&mut fx, at, Ok(Some(def)), false).0.unwrap(), 0.0);
    assert_eq!((cache(&fx, a), cache(&fx, b)), (0, 2));
    assert!(array(&fx, mind).is_empty(), "the rejected context is not touched");
}

#[test]
fn no_upgrade_answers_zero_with_nothing_read_or_written() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let at = attr(&mut fx, pc, None);
    let (v, asked) = read(&mut fx, at, Err(()), false);
    assert_eq!((v.unwrap(), asked), (0.0, 0));
    assert!(array(&fx, pc).is_empty());
}

#[test]
fn no_globals_definition_found_answers_zero_with_nothing_written() {
    // Note section 6: the lookup is supported and finds nothing.
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let ([a, b, _], _, _) = setup(&mut fx, &c);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let at = attr(&mut fx, pc, Some(b));
    assert_eq!(read(&mut fx, at, Ok(None), false).0.unwrap(), 0.0);
    assert_eq!((cache(&fx, a), cache(&fx, b)), (-1, -1));
    assert!(array(&fx, pc).is_empty());
}

#[test]
fn a_world_without_engine_globals_is_its_not_yet_before_any_write() {
    // Note section 6: no engine-globals support at all is not "none found".
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let ([a, b, _], _, _) = setup(&mut fx, &c);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let at = attr(&mut fx, pc, Some(b));
    let e = read(&mut fx, at, Err(()), false).0.unwrap_err();
    assert!(matches!(not_yet(&e).map(|n| &n.missing), Some(Missing::LayerAbsent(_))), "{e:?}");
    assert_eq!((cache(&fx, a), cache(&fx, b)), (-1, -1));
    assert!(array(&fx, pc).is_empty());
}

#[test]
fn an_unlisted_upgrade_stays_cold_and_the_listed_ones_are_assigned() {
    // Note section 3 and 6: the field stays -1, the array unchanged, 0.0; the next read walks again.
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let ([a, b, z], unlisted, def) = setup(&mut fx, &c);
    let pc = holder(&mut fx, c.controller, "PC", &[5]);
    let at = attr(&mut fx, pc, Some(unlisted));
    let (v, asked) = read(&mut fx, at, Ok(Some(def)), false);
    assert_eq!((v.unwrap(), asked), (0.0, 1));
    assert_eq!((cache(&fx, a), cache(&fx, b), cache(&fx, z), cache(&fx, unlisted)), (0, 2, 3, -1));
    assert_eq!(array(&fx, pc), vec![5]);
    assert_eq!(read(&mut fx, at, Ok(Some(def)), false).1, 1, "still cold: the walk runs again");
}

#[test]
fn a_duplicate_ends_with_its_last_position() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let a = upgrade(&mut fx, &c, "Dup", -1);
    let def = definition(&mut fx, &c, &[Some(a), Some(a)]);
    let pc = holder(&mut fx, c.controller, "PC", &[3, 9]);
    let at = attr(&mut fx, pc, Some(a));
    assert_eq!(read(&mut fx, at, Ok(Some(def)), false).0.unwrap(), 9.0);
    assert_eq!(cache(&fx, a), 1);
}

#[test]
fn bad_data_refuses_before_either_write() {
    // The hub's ruling: both effects checked before either write. A world that declares no WillowPlayerController
    // accepts every context (note section 2), so a holder whose levels are floats reaches the array check.
    let mut fx = Fx::new();
    let object = fx.c.object;
    let space = &mut fx.w.space;
    let d = |space: &mut ow_object::heap::ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(space.names.intern(n), k);
    let decls = vec![d(space, "CachedUpgradeIndex", PropKind::Int)];
    let up_class = space.classes.define("WillowGame.BlackMarketUpgradeDefinition", Some(object), decls).unwrap();
    let decls = vec![d(space, "BlackMarketUpgrades", PropKind::Array(Box::new(PropKind::Object { class: object })))];
    let def_class = space.classes.define("WillowGame.GlobalsDefinition", Some(object), decls).unwrap();
    let decls = vec![d(space, "BlackMarketUpgrades", PropKind::Array(Box::new(PropKind::Float)))];
    let floats = space.classes.define("Test.FloatLevels", Some(object), decls).unwrap();
    let decls = vec![d(space, "CachedUpgradeIndex", PropKind::Float)];
    let odd = space.classes.define("Test.OddUpgrade", Some(object), decls).unwrap();
    let a = fx.obj(up_class, "A", vec![("CachedUpgradeIndex", Value::Int(-1))]);
    let def = fx.obj(def_class, "Globals", vec![("BlackMarketUpgrades", Value::array(vec![Value::Object(Some(a))]))]);
    let holder = fx.obj(floats, "Holder", vec![("BlackMarketUpgrades", Value::array(vec![Value::Float(1.0)]))]);
    let at = attr(&mut fx, holder, Some(a));
    let e = read(&mut fx, at, Ok(Some(def)), false).0.unwrap_err();
    assert!(matches!(e, AttrError::BadData { .. }), "{e:?}");
    assert_eq!(cache(&fx, a), -1, "no cache write before the refusal");
    // A listed cache field that is not an int: bad data, nothing written.
    let o = fx.obj(odd, "Odd", vec![("CachedUpgradeIndex", Value::Float(-1.0))]);
    let def2 = fx.obj(def_class, "Globals2", vec![("BlackMarketUpgrades", Value::array(vec![Value::Object(Some(a)), Value::Object(Some(o))]))]);
    let at = attr(&mut fx, holder, Some(a));
    assert!(matches!(read(&mut fx, at, Ok(Some(def2)), false).0.unwrap_err(), AttrError::BadData { .. }));
    assert_eq!(cache(&fx, a), -1);
}

#[test]
fn a_cold_lookup_decides_growth_from_the_pending_assignments() {
    // The final round of #451: the index the growth needs is the pending one (a duplicate's last position).
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let a = upgrade(&mut fx, &c, "Dup", -1);
    let def = definition(&mut fx, &c, &[Some(a), None, Some(a)]);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let at = attr(&mut fx, pc, Some(a));
    assert_eq!(read(&mut fx, at, Ok(Some(def)), false).0.unwrap(), 0.0);
    assert_eq!((cache(&fx, a), array(&fx, pc)), (2, vec![0, 0, 0]));
}

#[test]
fn a_growth_the_world_cannot_make_is_the_array_not_yet_with_nothing_written() {
    // Note section 6: a needed array growth the world cannot make (a controller that is a stand-in holding no
    // properties), before any write: the cache fields stay cold.
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let ([a, b, _], _, def) = setup(&mut fx, &c);
    let pc = fx.w.space.alloc_stand_in(c.controller, "PC", Origin::Resource { package: 0, index: 1 }).unwrap();
    let at = attr(&mut fx, pc, Some(b));
    let e = read(&mut fx, at, Ok(Some(def)), false).0.unwrap_err();
    let n = not_yet(&e).expect("a NotYet");
    assert!(matches!(n.missing, Missing::NotPorted), "{n:?}");
    assert_eq!(n.what, ARRAY_WRITE);
    assert_eq!((cache(&fx, a), cache(&fx, b)), (-1, -1));
}

#[test]
fn reference_answers_the_unregistered_refusal() {
    let mut fx = Fx::new();
    fx.w.defs = Some(DefStore::new(PackageStore::new_empty(), IdentityPolicy::FirstInScanOrder, ParityMode::Reference));
    let c = classes(&mut fx);
    let up = upgrade(&mut fx, &c, "Warm", 0);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let at = attr(&mut fx, pc, Some(up));
    let e = read(&mut fx, at, Ok(None), false).0.unwrap_err();
    let n = not_yet(&e).expect("a NotYet");
    assert!(matches!(n.missing, Missing::NotRead), "{n:?}");
    // The class as the object space spells it, as the registry printed it (review r3 of #458, item 4).
    let spelled = crate::eval::class_path(&fx.w.space, fx.w.space.heap.get(at).map(|_| ()).map(|()| {
        let p = fx.w.space.prop(fx.w.space.heap.get(at).unwrap().class, "ValueResolverChain").unwrap();
        match fx.w.space.read(at, PropAddr::scalar(p)).unwrap() {
            Value::Array(items) => match items[0] {
                Value::Object(Some(r)) => r,
                _ => panic!(),
            },
            _ => panic!(),
        }
    }).unwrap());
    assert_eq!(n.what.as_ref(), format!("value resolver class {spelled}"));
    assert!(array(&fx, pc).is_empty());
}

#[test]
fn the_default_globals_definition_follows_the_configured_name() {
    // The port's reading (decision 2026-11-19-72): the globals object's DefaultGlobalsDefinitionName.
    let mut fx = Fx::new();
    let object = fx.c.object;
    let decl = PropertyDecl::new(fx.w.space.names.intern("DefaultGlobalsDefinitionName"), PropKind::Str);
    let class = fx.w.space.classes.define("WillowGame.WillowGlobals", Some(object), vec![decl]).unwrap();
    let g = fx.obj(class, "TheGlobals", vec![("DefaultGlobalsDefinitionName", Value::string(""))]);
    fx.w.globals = g;
    assert_eq!(fx.w.globals_definition().unwrap(), None, "an empty name finds none");
    let p = fx.w.space.prop(class, "DefaultGlobalsDefinitionName").unwrap();
    fx.w.space.write(g, PropAddr::scalar(p), Value::string("GD_Test.General.Globals")).unwrap();
    let e = fx.w.globals_definition().unwrap_err();
    assert!(matches!(not_yet(&e).map(|n| &n.missing), Some(Missing::LayerAbsent(_))), "no store: {e:?}");
    fx.w.defs = Some(DefStore::new(PackageStore::new_empty(), IdentityPolicy::FirstInScanOrder, ParityMode::Off));
    assert_eq!(fx.w.globals_definition().unwrap(), None, "the store finds none");
    let bare = fx.w.space.alloc(object, "Bare", None, Origin::Runtime).unwrap();
    fx.w.globals = bare;
    assert!(matches!(fx.w.globals_definition().unwrap_err(), AttrError::BadData { .. }));
}

#[test]
fn the_standard_system_registers_the_class() {
    let mut fx = Fx::new();
    let sys = crate::registry::AttributeSystem::standard().unwrap();
    let c = fx.by_path(CLASS);
    let o = fx.obj(c, "R", vec![]);
    assert!(sys.value_resolver(&fx.w.space, o).is_ok());
}

// ---------------------------------------------------------------------------------------------------------------
// Review r1 of #458
// ---------------------------------------------------------------------------------------------------------------

/// Item 1: an empty array declared with the wrong element kind is refused before either write, for the controller's
/// levels (floats) and for the globals list (ints).
#[test]
fn empty_arrays_of_the_wrong_kind_refuse_before_either_write() {
    let mut fx = Fx::new();
    let object = fx.c.object;
    let space = &mut fx.w.space;
    let d = |space: &mut ow_object::heap::ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(space.names.intern(n), k);
    let decls = vec![d(space, "CachedUpgradeIndex", PropKind::Int)];
    let up_class = space.classes.define("WillowGame.BlackMarketUpgradeDefinition", Some(object), decls).unwrap();
    let decls = vec![d(space, "BlackMarketUpgrades", PropKind::Array(Box::new(PropKind::Object { class: object })))];
    let def_class = space.classes.define("WillowGame.GlobalsDefinition", Some(object), decls).unwrap();
    let decls = vec![d(space, "BlackMarketUpgrades", PropKind::Array(Box::new(PropKind::Int)))];
    let int_list = space.classes.define("Test.IntListGlobals", Some(object), decls).unwrap();
    let decls = vec![d(space, "BlackMarketUpgrades", PropKind::Array(Box::new(PropKind::Float)))];
    let floats = space.classes.define("Test.FloatLevels", Some(object), decls).unwrap();
    let a = fx.obj(up_class, "A", vec![("CachedUpgradeIndex", Value::Int(-1))]);
    let def = fx.obj(def_class, "Globals", vec![("BlackMarketUpgrades", Value::array(vec![Value::Object(Some(a))]))]);
    // No WillowPlayerController is declared, so every context is accepted (note section 2).
    let holder = fx.obj(floats, "Holder", vec![]);
    let at = attr(&mut fx, holder, Some(a));
    assert!(matches!(read(&mut fx, at, Ok(Some(def)), false).0.unwrap_err(), AttrError::BadData { .. }));
    assert_eq!(cache(&fx, a), -1, "the empty float array is refused before the cache commits");
    let bad_list = fx.obj(int_list, "IntGlobals", vec![]);
    let e = read(&mut fx, at, Ok(Some(bad_list)), false).0.unwrap_err();
    assert!(matches!(e, AttrError::BadData { .. }), "{e:?}");
    assert_eq!(cache(&fx, a), -1);
}

/// Item 2: a warm cache past the array bound is bad data before any write, the array unchanged.
#[test]
fn a_cached_index_past_the_array_bound_is_bad_data_before_any_write() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let up = upgrade(&mut fx, &c, "Huge", i32::MAX);
    let pc = holder(&mut fx, c.controller, "PC", &[4]);
    let at = attr(&mut fx, pc, Some(up));
    let e = read(&mut fx, at, Err(()), false).0.unwrap_err();
    assert!(matches!(e, AttrError::BadData { .. }), "{e:?}");
    assert_eq!(array(&fx, pc), vec![4]);
}

/// Item 3: a listed stand-in after a writable entry is the named cache-write `NotPorted`, and the writable entry is
/// not written; a handle with no store keeps its load error.
#[test]
fn a_listed_stand_in_is_the_cache_write_not_yet_and_a_load_failure_keeps_its_error() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let a = upgrade(&mut fx, &c, "Writable", -1);
    let s = fx.w.space.alloc_stand_in(c.upgrade, "StandIn", Origin::Default { class: c.upgrade }).unwrap();
    let def = definition(&mut fx, &c, &[Some(a), Some(s)]);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let at = attr(&mut fx, pc, Some(a));
    let e = read(&mut fx, at, Ok(Some(def)), false).0.unwrap_err();
    let n = not_yet(&e).expect("a NotYet");
    assert_eq!((n.missing.clone(), n.what.as_ref()), (Missing::NotPorted, CACHE_WRITE));
    assert_eq!(cache(&fx, a), -1);
    assert!(array(&fx, pc).is_empty());
    let r = fx.w.space.alloc_stand_in(c.upgrade, "Handle", Origin::Resource { package: 0, index: 1 }).unwrap();
    let def = definition(&mut fx, &c, &[Some(a), Some(r)]);
    let e = read(&mut fx, at, Ok(Some(def)), false).0.unwrap_err();
    assert!(matches!(not_yet(&e).map(|n| &n.missing), Some(Missing::LayerAbsent(_))), "the load error: {e:?}");
    assert_eq!(cache(&fx, a), -1);
}

/// Item 4: a later resolver's `NotYet` after the black-market writes is terminal and prefixed, its kind kept, also
/// on a direct library read.
#[test]
fn a_later_not_yet_after_the_writes_is_prefixed_on_a_library_read() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let ([_, b, _], _, def) = setup(&mut fx, &c);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let bm = fx.by_path(CLASS);
    let r = fx.obj(bm, "BM", vec![("AssociatedBlackMarketUpgrade", Value::Object(Some(b)))]);
    let unread_class = fx.by_path("Test.UnreadValueResolver");
    let unread = fx.obj(unread_class, "Unread", vec![]);
    let ctx = fx.target(Some(pc));
    let at = fx.attr("Att_BM_Then_Unread", vec![ctx], vec![r, unread]);
    let e = read(&mut fx, at, Ok(Some(def)), false).0.unwrap_err();
    let n = not_yet(&e).expect("a NotYet");
    assert_eq!(n.missing, Missing::NotRead);
    assert!(n.what.starts_with("stopped after partial writes: value resolver class Test.UnreadValueResolver"), "{n:?}");
    assert_eq!(cache(&fx, b), 2, "the writes stay");
    // Without writes the same chain refuses plainly.
    let up = upgrade(&mut fx, &c, "NoUpgrade", 0);
    let _ = up;
    let r2 = fx.obj(bm, "BM2", vec![("AssociatedBlackMarketUpgrade", Value::Object(None))]);
    let at = fx.attr("Att_None_Then_Unread", vec![ctx], vec![r2, unread]);
    let n = read(&mut fx, at, Ok(Some(def)), false).0.unwrap_err();
    assert!(!not_yet(&n).unwrap().what.starts_with("stopped"), "{n:?}");
}

/// Item 5: under Reference the preflight refuses the class as it refused an unregistered one.
#[test]
fn reference_preflight_refuses_the_class_as_before() {
    let mut fx = Fx::new();
    fx.w.defs = Some(DefStore::new(PackageStore::new_empty(), IdentityPolicy::FirstInScanOrder, ParityMode::Reference));
    let c = classes(&mut fx);
    let up = upgrade(&mut fx, &c, "Warm", 0);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let at = attr(&mut fx, pc, Some(up));
    let sys = system();
    let e = sys.preflight_read(at, &mut EvalCx::new(&mut fx.w)).unwrap_err();
    let n = not_yet(&e).expect("a NotYet");
    assert_eq!(n.missing, Missing::NotRead);
    assert_eq!(n.what.as_ref(), format!("value resolver class {CLASS}"));
    // Off passes the same preflight.
    let mut off = Fx::new();
    let c = classes(&mut off);
    let up = upgrade(&mut off, &c, "Warm", 0);
    let pc = holder(&mut off, c.controller, "PC", &[]);
    let at = attr(&mut off, pc, Some(up));
    assert!(system().preflight_read(at, &mut EvalCx::new(&mut off.w)).is_ok());
}

/// Item 6: a world that supports a globals object but has none answers "none found" (0.0), unlike a world without
/// the support (its `NotYet`).
#[test]
fn no_globals_object_is_none_found_and_no_support_is_its_not_yet() {
    struct NoGlobals<'a>(&'a mut World);
    impl AttrWorld for NoGlobals<'_> {
        fn parts(&mut self) -> WorldParts<'_> {
            self.0.parts()
        }
        fn globals(&mut self) -> Result<ObjRef, AttrError> {
            Err(NotYet::layer_absent("the engine globals object", "ow-world", "test world").into())
        }
        fn globals_object(&mut self) -> Result<Option<ObjRef>, AttrError> {
            Ok(None)
        }
        fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
            Ok(None)
        }
        fn global_slots(&mut self) -> &mut GlobalSlots {
            self.0.global_slots()
        }
        fn attribute_changed(&mut self, o: ObjRef, p: PropId, f: NumForms) -> Result<(), AttrError> {
            self.0.attribute_changed(o, p, f)
        }
        fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
            self.0.rng(s)
        }
    }
    let mut fx = Fx::new();
    assert_eq!(NoGlobals(&mut fx.w).globals_definition().unwrap(), None);
    struct NoSupport<'a>(&'a mut World);
    impl AttrWorld for NoSupport<'_> {
        fn parts(&mut self) -> WorldParts<'_> {
            self.0.parts()
        }
        fn globals(&mut self) -> Result<ObjRef, AttrError> {
            Err(NotYet::layer_absent("the engine globals object", "ow-world", "test world").into())
        }
        fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
            Ok(None)
        }
        fn global_slots(&mut self) -> &mut GlobalSlots {
            self.0.global_slots()
        }
        fn attribute_changed(&mut self, o: ObjRef, p: PropId, f: NumForms) -> Result<(), AttrError> {
            self.0.attribute_changed(o, p, f)
        }
        fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
            self.0.rng(s)
        }
    }
    let e = NoSupport(&mut fx.w).globals_definition().unwrap_err();
    assert!(matches!(not_yet(&e).map(|n| &n.missing), Some(Missing::LayerAbsent(_))), "{e:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// Review r3 of #458
// ---------------------------------------------------------------------------------------------------------------

/// Item 1: a chain [BlackMarket, None] commits the writes, then the None entry's `NotYet` leaves the fold prefixed.
#[test]
fn a_none_entry_after_the_writes_is_prefixed() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let ([_, b, _], _, def) = setup(&mut fx, &c);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let bm = fx.by_path(CLASS);
    let r = fx.obj(bm, "BM", vec![("AssociatedBlackMarketUpgrade", Value::Object(Some(b)))]);
    let ctx = fx.target(Some(pc));
    let at = fx.attr("Att_BM_Then_None", vec![ctx], vec![r]);
    // Append a None entry to the value chain.
    let p = fx.w.space.prop(fx.w.space.heap.get(at).unwrap().class, "ValueResolverChain").unwrap();
    fx.w.space
        .write(at, PropAddr::scalar(p), Value::array(vec![Value::Object(Some(r)), Value::Object(None)]))
        .unwrap();
    let e = read(&mut fx, at, Ok(Some(def)), false).0.unwrap_err();
    let n = not_yet(&e).expect("a NotYet");
    assert_eq!(n.missing, Missing::NotRead);
    assert!(n.what.starts_with("stopped after partial writes: a None entry"), "{n:?}");
    assert_eq!((cache(&fx, b), array(&fx, pc)), (2, vec![0, 0, 0]), "the writes stay");
}

/// Item 2: two independent reads on one context: the first commits writes and succeeds; the second refuses plainly.
#[test]
fn a_reused_context_does_not_carry_one_reads_writes_into_another() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let ([_, b, _], _, def) = setup(&mut fx, &c);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let writes = attr(&mut fx, pc, Some(b));
    let unread_class = fx.by_path("Test.UnreadValueResolver");
    let unread = fx.obj(unread_class, "Unread", vec![]);
    let ctx = fx.target(Some(pc));
    let refuses = fx.attr("Att_Unread", vec![ctx], vec![unread]);
    let sys = system();
    let mut w = GlobalsWorld { inner: &mut fx.w, definition: Ok(Some(def)), asked: 0 };
    let mut cx = EvalCx::new(&mut w);
    assert_eq!(sys.value_of(writes, None, None, &mut cx).unwrap().unwrap().value, 0.0);
    let e = sys.value_of(refuses, None, None, &mut cx).unwrap_err();
    let n = not_yet(&e).expect("a NotYet");
    assert!(!n.what.starts_with("stopped"), "{n:?}");
    assert_eq!(n.missing, Missing::NotRead);
}

/// Item 3: with a store, after a writable list entry: a load that succeeds but leaves a stand-in is the named
/// cache-write `NotPorted`, and a real load error (a store handle whose export does not decode) comes back unchanged;
/// the earlier cache stays cold in both.
#[test]
fn store_backed_stand_ins_and_load_errors_after_a_writable_entry() {
    use ow_package::testkit::script::ScriptToy;
    // An invented package: a Thing class with an object property Target; ToyTarget's payload does not decode, and
    // loading ToyArchetype makes its Target a store handle that is not loaded.
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "IntProperty"]);
    let object = t.class("Object", 0);
    let thing = t.class("Thing", object);
    t.prop("Object", thing, "Target", 0, thing);
    t.prop("Int", thing, "CachedUpgradeIndex", 0, 0);
    let mut broken = 0i32.to_le_bytes().to_vec();
    broken.extend_from_slice(&[0xff; 16]);
    let toy_target = t.p.add_export(thing, "ToyTarget", broken);
    let mut archetype = 0i32.to_le_bytes().to_vec();
    archetype.extend(t.p.object_tag("Target", toy_target));
    archetype.extend_from_slice(&t.none);
    t.p.add_export(thing, "ToyArchetype", archetype);
    let bytes = t.build();
    for case in ["stand-in", "load error"] {
        let dir = std::env::temp_dir().join(format!("ow-attr-bm-store-{}-{}", std::process::id(), case.replace(' ', "-")));
        let _ = std::fs::remove_dir_all(&dir);
        let mut fx = Fx::new();
        let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", bytes.as_slice())], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
            .unwrap();
        fx.w.defs = Some(defs);
        let c = classes(&mut fx);
        let thing_class = fx.w.defs.as_mut().unwrap().class_id(&mut fx.w.space, "Engine.Thing").unwrap();
        let a = fx.obj(thing_class, "Writable", vec![("CachedUpgradeIndex", Value::Int(-1))]);
        let second = match case {
            // A slotless object the store does not hold as a handle: its load succeeds and leaves it a stand-in.
            "stand-in" => fx.w.space.alloc_stand_in(thing_class, "StandIn", Origin::Resource { package: 0, index: 1 }).unwrap(),
            // The store's own handle for ToyTarget: its load fails.
            _ => {
                let path = ow_core::ObjectPath::parse("Engine.ToyArchetype").unwrap();
                let arch = fx.w.defs.as_mut().unwrap().find(&mut fx.w.space, &path).unwrap().unwrap();
                let p = fx.w.space.prop(thing_class, "Target").unwrap();
                match fx.w.space.read(arch, PropAddr::scalar(p)).unwrap() {
                    Value::Object(Some(h)) => *h,
                    other => panic!("{other:?}"),
                }
            }
        };
        // A globals list of Things (a test-local class; the shape the resolver reads).
        let decls = vec![PropertyDecl::new(
            fx.w.space.names.intern("BlackMarketUpgrades"),
            PropKind::Array(Box::new(PropKind::Object { class: thing_class })),
        )];
        let list_class = fx.w.space.classes.define("Test.ThingGlobals", Some(fx.c.object), decls).unwrap();
        let def = fx.obj(list_class, "Globals", vec![("BlackMarketUpgrades", Value::array(vec![Value::Object(Some(a)), Value::Object(Some(second))]))]);
        let pc = holder(&mut fx, c.controller, "PC", &[]);
        // The resolver's field is typed for the test world's Object; the store's Thing is set through the slot.
        let at = attr(&mut fx, pc, None);
        let chain = fx.w.space.prop(fx.w.space.heap.get(at).unwrap().class, "ValueResolverChain").unwrap();
        let resolver = match fx.w.space.read(at, PropAddr::scalar(chain)).unwrap() {
            Value::Array(items) => match items[0] {
                Value::Object(Some(r)) => r,
                _ => panic!(),
            },
            _ => panic!(),
        };
        let field = fx.w.space.prop(fx.w.space.heap.get(resolver).unwrap().class, "AssociatedBlackMarketUpgrade").unwrap();
        *fx.w.space.slot_mut(resolver, PropAddr::scalar(field)).unwrap() = Value::Object(Some(a));
        let e = read(&mut fx, at, Ok(Some(def)), false).0.expect_err(case);
        match case {
            "stand-in" => {
                let n = not_yet(&e).expect("a NotYet");
                assert_eq!((n.missing.clone(), n.what.as_ref()), (Missing::NotPorted, CACHE_WRITE));
            }
            _ => assert!(matches!(e, AttrError::Def(_)), "the store's own decode error, unchanged: {e:?}"),
        }
        assert_eq!(cache(&fx, a), -1, "{case}: the earlier cache stays cold");
        assert!(array(&fx, pc).is_empty(), "{case}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A registered resolver whose `available`, given the mutable context, writes a cache and then refuses (an invented
/// extension, not a game class), counting its calls. Its value is never asked here.
struct WritesWhenAsked {
    asked: std::rc::Rc<std::cell::Cell<u32>>,
}

impl crate::api::ValueResolver for WritesWhenAsked {
    fn available(&self, resolver: ObjRef, cx: &mut EvalCx<'_>) -> Result<(), AttrError> {
        self.asked.set(self.asked.get() + 1);
        let space = cx.world.parts().space;
        let class = space.heap.get(resolver)?.class;
        let p = space.prop(class, "AssociatedBlackMarketUpgrade").unwrap();
        space.write(resolver, PropAddr::scalar(p), Value::Object(None))?;
        Err(NotYet::not_read("a resolver that wrote while asked", "a test record").into())
    }

    fn value(
        &self,
        _sys: &crate::AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        _cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        panic!("the preflights evaluate nothing")
    }
}

/// Review r2 of #573: the resident preflight asks a resolver's mode availability as metadata only. A resolver whose
/// `available` would write is never called by it (no call, no write); the loading preflight does call it.
#[test]
fn the_resident_preflight_never_calls_a_resolvers_available() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    let up = upgrade(&mut fx, &c, "Warm", 0);
    let class = fx.by_path(CLASS);
    let r = fx.obj(class, "BM", vec![("AssociatedBlackMarketUpgrade", Value::Object(Some(up)))]);
    let at = fx.attr("Att_BM_Test", vec![], vec![r]);
    let asked = std::rc::Rc::new(std::cell::Cell::new(0));
    let sys = crate::AttributeSystem::new_empty().with_value_resolver(CLASS, WritesWhenAsked { asked: asked.clone() }).unwrap();
    let writes = fx.w.space.heap.committed_writes();
    assert!(sys.preflight_read_resident(at, &mut EvalCx::new(&mut fx.w)).is_ok());
    assert_eq!(asked.get(), 0, "the resident preflight never calls available");
    assert_eq!(fx.w.space.heap.committed_writes(), writes, "and nothing is written");
    let e = sys.preflight_read(at, &mut EvalCx::new(&mut fx.w)).unwrap_err();
    assert_eq!(not_yet(&e).unwrap().what.as_ref(), "a resolver that wrote while asked");
    assert_eq!(asked.get(), 1, "the loading preflight still asks it");
}

/// Review r2 of #573: the resident preflight keeps the class's Reference refusal, now asked as metadata.
#[test]
fn the_resident_preflight_keeps_the_reference_refusal() {
    let mut fx = Fx::new();
    fx.w.defs = Some(DefStore::new(PackageStore::new_empty(), IdentityPolicy::FirstInScanOrder, ParityMode::Reference));
    let c = classes(&mut fx);
    let up = upgrade(&mut fx, &c, "Warm", 0);
    let pc = holder(&mut fx, c.controller, "PC", &[]);
    let at = attr(&mut fx, pc, Some(up));
    let e = system().preflight_read_resident(at, &mut EvalCx::new(&mut fx.w)).unwrap_err();
    let n = not_yet(&e).expect("a NotYet");
    assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, format!("value resolver class {CLASS}").as_str()));
}
