//! The Currency, Random, AmmoDropWeight and Constraint resolvers on a synthetic world (rule 9: invented objects and numbers;
//! the class and field names are the packages' names, so the resolvers meet the shapes they read). Lane
//! FIX-ATTR-RESOLVERS, decision 2026-11-16-73.

use ow_core::rand::{CrtRand, CrtStream, Rand15};
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::store::PackageStore;

use super::testworld::{Fx, system};
use crate::api::{AttrError, EvalCx};

struct Cls {
    currency: ClassId,
    random: ClassId,
    ammo: ClassId,
    info: ClassId,
    controller: ClassId,
    pool: ClassId,
    resource: ClassId,
    state: StructId,
}

fn classes(fx: &mut Fx) -> Cls {
    let object = fx.c.object;
    let currency = fx.by_path("WillowGame.CurrencyAttributeValueResolver");
    let random = fx.by_path("GearboxFramework.RandomAttributeValueResolver");
    let ammo = fx.by_path("WillowGame.AmmoDropWeightAttributeValueResolver");
    let space = &mut fx.w.space;
    let d = |space: &mut ow_object::heap::ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(space.names.intern(n), k);
    let fields = vec![d(space, "CurrentAmount", PropKind::Int)];
    let state = space.classes.define_struct("Test.Info.CurrencyState", None, fields).unwrap();
    let decls = vec![d(space, "Currency", PropKind::Struct(state)).with_array_dim(13)];
    let info = space.classes.define("Test.Info", Some(object), decls).unwrap();
    let decls = vec![d(space, "PlayerReplicationInfo", PropKind::Object { class: object })];
    let controller = space.classes.define("Test.Controller", Some(object), decls).unwrap();
    let decls = vec![d(space, "MaxValue", PropKind::Float), d(space, "CurrentValue", PropKind::Float)];
    let pool = space.classes.define("Test.Pool", Some(object), decls).unwrap();
    let resource = space.classes.define("Test.Resource", Some(object), vec![]).unwrap();
    Cls { currency, random, ammo, info, controller, pool, resource, state }
}

fn mode(fx: &mut Fx, mode: ParityMode) {
    fx.w.defs = Some(DefStore::new(PackageStore::new_empty(), IdentityPolicy::FirstInScanOrder, mode));
}

/// An attribute with no context hop whose one value resolver is `resolver`.
fn attr(fx: &mut Fx, resolver: ObjRef) -> ObjRef {
    let ctx = fx.no_context();
    fx.attr("Toy_Attribute", vec![ctx], vec![resolver])
}

fn value(fx: &mut Fx, attr: ObjRef, source: ObjRef) -> Result<f32, AttrError> {
    system().value_of(attr, Some(source), None, &mut EvalCx::new(&mut fx.w)).map(|v| v.unwrap().value)
}

fn not_yet(r: Result<f32, AttrError>) -> Missing {
    match r {
        Err(AttrError::NotYet(n)) => n.missing,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

// ---- Currency ----

/// A replication info holding `amount` in slot `slot`, a controller holding the info, and a currency attribute of
/// `form`.
fn currency(fx: &mut Fx, c: &Cls, slot: u32, amount: i32, form: u8) -> (ObjRef, ObjRef, ObjRef) {
    let info = fx.obj(c.info, "Toy_Info", vec![]);
    let entry = fx.st(c.state, vec![("CurrentAmount", Value::Int(amount))]);
    let p = fx.w.space.prop(c.info, "Currency").unwrap();
    fx.w.space.write(info, PropAddr { prop: p, index: slot }, entry).unwrap();
    let controller = fx.obj(c.controller, "Toy_Controller", vec![("PlayerReplicationInfo", Value::Object(Some(info)))]);
    let r = fx.obj(c.currency, "Toy_Currency", vec![("FormOfCurrency", Value::Byte(form))]);
    let a = attr(fx, r);
    (info, controller, a)
}

#[test]
fn currency_reference_reads_the_slot_of_the_info_or_of_the_controllers_info() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Reference);
    let (info, controller, a) = currency(&mut fx, &c, 1, 312, 1);
    assert_eq!(value(&mut fx, a, info).unwrap(), 312.0);
    assert_eq!(value(&mut fx, a, controller).unwrap(), 312.0);
    // Another slot of the same table: its own amount (0 here).
    let r = fx.obj(c.currency, "Toy_Currency_Other", vec![("FormOfCurrency", Value::Byte(0))]);
    let other = attr(&mut fx, r);
    assert_eq!(value(&mut fx, other, controller).unwrap(), 0.0);
}

#[test]
fn currency_reference_refuses_a_context_without_a_table_and_a_slot_past_it() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Reference);
    let (_, _, a) = currency(&mut fx, &c, 0, 5, 0);
    let pool = fx.obj(c.pool, "Toy_NoTable", vec![]);
    assert_eq!(not_yet(value(&mut fx, a, pool)), Missing::NotRead);
    let (info, _, past) = currency(&mut fx, &c, 0, 5, 13);
    assert_eq!(not_yet(value(&mut fx, past, info)), Missing::NotRead);
}

#[test]
fn currency_off_is_not_yet() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Off);
    let (info, _, a) = currency(&mut fx, &c, 0, 5, 0);
    assert_eq!(not_yet(value(&mut fx, a, info)), Missing::NotRead);
}

// ---- Random ----

fn random(fx: &mut Fx, c: &Cls, low: f32, high: f32, integer: bool) -> ObjRef {
    let (l, h) = (fx.k(low), fx.k(high));
    let r = fx.obj(c.random, "Toy_Random", vec![
        ("LowerBound", l),
        ("UpperBound", h),
        ("bUseIntegerRandomization", Value::Bool(integer)),
    ]);
    attr(fx, r)
}

#[test]
fn random_reference_draws_once_from_the_variance_stream_and_replaces_the_value() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Reference);
    let source = fx.obj(c.pool, "Toy_Source", vec![]);
    let int = random(&mut fx, &c, -2.0, 1.0, true);
    let real = random(&mut fx, &c, 1.5, 3.5, false);
    fx.w.rng = Some(CrtRand::new(20_261_116));
    let mut expected = CrtRand::new(20_261_116);
    for _ in 0..8 {
        // Integer: ceil(low) + floor(d * span), span = floor(high) - ceil(low) + 1 = 4, both ends reachable.
        let d = f64::from(expected.rand15()) / 32768.0;
        let want = (-2.0 + (d * 4.0).floor()) as f32;
        assert_eq!(value(&mut fx, int, source).unwrap(), want);
        let d = f64::from(expected.rand15()) / 32768.0;
        let want = 1.5f32 + (d as f32) * (3.5 - 1.5);
        assert_eq!(value(&mut fx, real, source).unwrap(), want);
    }
    assert!(fx.w.streams.iter().all(|s| *s == CrtStream::AttributeVariance));
    assert_eq!(fx.w.streams.len(), 16, "one draw per read");
}

#[test]
fn random_off_is_not_yet_and_draws_nothing() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Off);
    let source = fx.obj(c.pool, "Toy_Source", vec![]);
    let int = random(&mut fx, &c, -2.0, 1.0, true);
    fx.w.rng = Some(CrtRand::new(1));
    assert_eq!(not_yet(value(&mut fx, int, source)), Missing::NotRead);
    assert!(fx.w.streams.is_empty());
}

// ---- AmmoDropWeight ----

/// A drop-weight attribute for a resource, with invented weights: threshold 0.4, above 0.03, min 0.1, max 0.25, no
/// pool 0.5.
fn ammo(fx: &mut Fx, c: &Cls) -> (ObjRef, ObjRef) {
    let resource = fx.obj(c.resource, "Toy_Resource", vec![]);
    let ws: Vec<(&str, Value)> = [
        ("ResourceThreshold", 0.4),
        ("AboveThresholdWeight", 0.03),
        ("MinBelowThresholdWeight", 0.1),
        ("MaxBelowThresholdWeight", 0.25),
        ("NoPoolWeight", 0.5),
    ]
    .into_iter()
    .map(|(n, v)| (n, fx.k(v)))
    .collect();
    let mut props = vec![("Resource", Value::Object(Some(resource)))];
    props.extend(ws);
    let r = fx.obj(c.ammo, "Toy_AmmoDropWeight", props);
    (attr(fx, r), resource)
}

fn pool(fx: &mut Fx, c: &Cls, max: f32, current: f32) -> ObjRef {
    fx.obj(c.pool, "Toy_Pool", vec![("MaxValue", Value::Float(max)), ("CurrentValue", Value::Float(current))])
}

#[test]
fn ammo_drop_weight_reference_follows_the_local_players_share() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Reference);
    let (a, resource) = ammo(&mut fx, &c);
    let source = fx.obj(c.pool, "Toy_Killed", vec![]);
    let cases = [
        // Below the threshold: max + (share / threshold) * (min - max), single precision.
        (100.0, 20.0, 0.25f32 + (0.2f32 / 0.4f32) * (0.1f32 - 0.25f32)),
        // At or above it: the above weight.
        (100.0, 40.0, 0.03),
        (100.0, 90.0, 0.03),
        // An empty pool: the maximum weight.
        (100.0, 0.0, 0.25),
        // A maximum not above 0: share 0.
        (0.0, 7.0, 0.25),
    ];
    for (max, current, want) in cases {
        let p = pool(&mut fx, &c, max, current);
        fx.w.local_pools = Some(vec![(resource, Some(p))]);
        assert_eq!(value(&mut fx, a, source).unwrap(), want, "{current} of {max}");
    }
    // No such pool: the no-pool weight.
    fx.w.local_pools = Some(vec![]);
    assert_eq!(value(&mut fx, a, source).unwrap(), 0.5);
}

#[test]
fn ammo_drop_weight_reference_without_the_query_and_off_are_not_yet() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Reference);
    let (a, _) = ammo(&mut fx, &c);
    let source = fx.obj(c.pool, "Toy_Killed", vec![]);
    // A world that answers no local-player query: its NotYet, never a silent no-pool weight.
    assert!(matches!(not_yet(value(&mut fx, a, source)), Missing::LayerAbsent(_)));
    mode(&mut fx, ParityMode::Off);
    fx.w.local_pools = Some(vec![]);
    assert_eq!(not_yet(value(&mut fx, a, source)), Missing::NotRead);
}

// ---- Constraint ----

/// An attribute whose chain is a constant `v` then a constraint of `min` / `max` (each `None`: not enabled).
fn clamped(fx: &mut Fx, v: f32, min: Option<f32>, max: Option<f32>) -> ObjRef {
    let k = fx.constant(v);
    let range = fx.range(min, max);
    let c = fx.by_path("GearboxFramework.ConstraintAttributeValueResolver");
    let r = fx.obj(c, "Toy_Constraint", vec![("Constraints", range)]);
    let ctx = fx.no_context();
    fx.attr("Toy_Clamped", vec![ctx], vec![k, r])
}

#[test]
fn constraint_reference_clamps_the_running_value_minimum_first() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Reference);
    let source = fx.obj(c.pool, "Toy_Source", vec![]);
    for (v, min, max, want) in [
        (5.0, Some(2.0), Some(4.0), 4.0),
        (1.0, Some(2.0), Some(4.0), 2.0),
        (3.0, Some(2.0), Some(4.0), 3.0),
        (9.0, None, None, 9.0),
        (-1.0, Some(0.0), None, 0.0),
        // A minimum above the maximum: the maximum is applied last.
        (1.0, Some(6.0), Some(4.0), 4.0),
    ] {
        let a = clamped(&mut fx, v, min, max);
        assert_eq!(value(&mut fx, a, source).unwrap(), want, "{v} {min:?} {max:?}");
    }
}

#[test]
fn constraint_off_is_not_yet() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Off);
    let source = fx.obj(c.pool, "Toy_Source", vec![]);
    let a = clamped(&mut fx, 5.0, Some(2.0), Some(4.0));
    assert_eq!(not_yet(value(&mut fx, a, source)), Missing::NotRead);
}
