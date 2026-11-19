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

use super::blackmarket_upgrade::{ARRAY_WRITE, CLASS, CONTROLLER};
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
    assert!(n.what.contains(CLASS), "{n:?}");
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
