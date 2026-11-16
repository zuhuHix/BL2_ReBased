//! The Currency, Random, AmmoDropWeight and Constraint resolvers on a synthetic world (rule 9: invented objects and numbers;
//! the class and field names are the packages' names, so the resolvers meet the shapes they read). Lane
//! FIX-ATTR-RESOLVERS, decision 2026-11-16-73; the Off rules (NATIVE_ATTR_RESOLVERS.md, implemented by the fresh lane
//! FIX-ATTR-RESOLVERS-OFF) decision 2026-11-16-80.

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
    let decls = vec![
        d(space, "MaxValue", PropKind::Float),
        d(space, "CurrentValue", PropKind::Float),
        d(space, "MinValue", PropKind::Float),
        d(space, "bCreatedAndNotModified", PropKind::Bool),
    ];
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

/// Off (decision 80): a replication info answers its `GetCurrencyOnHand(FormOfCurrency)`; anything else, a controller
/// holding the info included, answers 0 (no move to the info).
#[test]
fn currency_off_asks_the_replication_info_and_gives_a_controller_zero() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Off);
    let object = fx.c.object;
    let pri = fx.w.space.classes.define("WillowGame.WillowPlayerReplicationInfo", Some(object), vec![]).unwrap();
    let sub = fx.w.space.classes.define("Test.SubInfo", Some(pri), vec![]).unwrap();
    let info = fx.obj(sub, "Toy_Pri", vec![]);
    let controller = fx.obj(c.controller, "Toy_Pc", vec![("PlayerReplicationInfo", Value::Object(Some(info)))]);
    let r = fx.obj(c.currency, "Toy_Cash", vec![("FormOfCurrency", Value::Byte(1))]);
    let a = attr(&mut fx, r);
    // The world's answer is not modelled: its NotYet, never a silent 0.
    assert!(matches!(not_yet(value(&mut fx, a, info)), Missing::LayerAbsent(_)));
    fx.w.currency = Some(vec![(info, 1, 312), (info, 0, 7)]);
    assert_eq!(value(&mut fx, a, info).unwrap(), 312.0);
    assert_eq!(value(&mut fx, a, controller).unwrap(), 0.0, "a controller is not moved to its info");
    let other = fx.obj(c.info, "Toy_NotAPri", vec![]);
    assert_eq!(value(&mut fx, a, other).unwrap(), 0.0);
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

/// Off (decision 80), no lifetime: real `low + r / 32767 * (high - low)`; integer `trunc(low) + trunc(r / 32768 *
/// span)` with `span = trunc(high) + 1 - trunc(low)`; a span below 1 draws nothing; a span past 512 is `NotYet`.
#[test]
fn random_off_draws_as_the_note_reads() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Off);
    let source = fx.obj(c.pool, "Toy_Source", vec![]);
    // trunc(-2.7) = -2, trunc(1.9) = 1: span 4 (the stand-in's ceil and floor would give -2..1 too, so also check
    // a case where they differ: trunc(-2.2) = -2 where ceil is -2, trunc(-0.5) = 0 where floor is -1).
    let int = random(&mut fx, &c, -2.7, 1.9, true);
    let neg = random(&mut fx, &c, -2.2, -0.5, true);
    let real = random(&mut fx, &c, 1.5, 3.5, false);
    fx.w.rng = Some(CrtRand::new(20_261_116));
    let mut expected = CrtRand::new(20_261_116);
    for _ in 0..8 {
        let r = f64::from(expected.rand15());
        assert_eq!(value(&mut fx, int, source).unwrap(), (-2.0 + (r / 32768.0 * 4.0).trunc()) as f32);
        let r = f64::from(expected.rand15());
        assert_eq!(value(&mut fx, neg, source).unwrap(), (-2.0 + (r / 32768.0 * 3.0).trunc()) as f32);
        let r = f32::from(expected.rand15());
        assert_eq!(value(&mut fx, real, source).unwrap(), 1.5f32 + r / 32767.0 * (3.5 - 1.5));
    }
    assert!(fx.w.streams.iter().all(|s| *s == CrtStream::AttributeVariance));
    assert_eq!(fx.w.streams.len(), 24);
    // trunc(3.9) = 3, trunc(2.1) = 2: span 0, the lower bound truncated, no draw.
    let empty = random(&mut fx, &c, 3.9, 2.1, true);
    assert_eq!(value(&mut fx, empty, source).unwrap(), 3.0);
    assert_eq!(fx.w.streams.len(), 24, "no draw");
    // A span of 1001: the product's precision would matter (decision 80), so NotYet.
    let wide = random(&mut fx, &c, 0.0, 1000.0, true);
    assert_eq!(not_yet(value(&mut fx, wide, source)), Missing::NotRead);
}

/// A random resolver with a lifetime (a constant), on the shared fixture.
fn random_kept(fx: &mut Fx, c: &Cls, lifetime: f32) -> (ObjRef, ObjRef) {
    let (l, h, t) = (fx.k(0.0), fx.k(1.0), fx.k(lifetime));
    let r = fx.obj(c.random, "Toy_KeptRandom", vec![
        ("LowerBound", l),
        ("UpperBound", h),
        ("bUseIntegerRandomization", Value::Bool(false)),
        ("ValueLifetime", t),
    ]);
    (attr(fx, r), r)
}

fn saved(fx: &mut Fx, r: ObjRef) -> Vec<(String, f32, f32)> {
    let space = &fx.w.space;
    let p = space.prop(space.heap.get(r).unwrap().class, "SavedRandomValues").unwrap();
    let Value::Array(list) = space.read(r, PropAddr::scalar(p)).unwrap().clone() else { panic!() };
    list.iter()
        .map(|e| {
            let f = |n: &str| crate::eval::field(space, e, n).unwrap().clone();
            let Value::Name(n) = f("ContextName") else { panic!() };
            let num = |v: Value| crate::eval::number("x", &v).unwrap();
            (space.names.text(n).into_owned(), num(f("SavedValue")), num(f("ExpirationTime")))
        })
        .collect()
}

/// Off (decision 80), with a lifetime: one saved value per context name until it expires; an expired entry is removed
/// as the walk meets it; a lifetime at most 0.0001 saves nothing; with a lifetime the world's clock is needed.
#[test]
fn random_off_keeps_a_value_per_context_name_for_its_lifetime() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Off);
    let a_ctx = fx.obj(c.pool, "Toy_A", vec![]);
    let b_ctx = fx.obj(c.pool, "Toy_B", vec![]);
    let (a, r) = random_kept(&mut fx, &c, 5.0);
    // No clock modelled: NotYet, and nothing drawn.
    assert!(matches!(not_yet(value(&mut fx, a, a_ctx)), Missing::LayerAbsent(_)));
    assert!(fx.w.streams.is_empty());
    fx.w.clock = Some(Some(10.0));
    let first = value(&mut fx, a, a_ctx).unwrap();
    assert_eq!(value(&mut fx, a, a_ctx).unwrap(), first, "kept");
    assert_eq!(fx.w.streams.len(), 1, "one draw for two reads");
    let b = value(&mut fx, a, b_ctx).unwrap();
    assert_eq!(fx.w.streams.len(), 2);
    assert_eq!(saved(&mut fx, r), vec![("Toy_A".to_string(), first, 15.0), ("Toy_B".to_string(), b, 15.0)]);
    // At 15 nothing has expired (expiration before the clock); at 15.5 both have: reading B removes A, then B, and
    // appends a fresh B.
    fx.w.clock = Some(Some(15.0));
    assert_eq!(value(&mut fx, a, b_ctx).unwrap(), b);
    fx.w.clock = Some(Some(15.5));
    let b2 = value(&mut fx, a, b_ctx).unwrap();
    assert_eq!(saved(&mut fx, r), vec![("Toy_B".to_string(), b2, 20.5)]);
    // A live match stops the walk: entries after it stay, expired or not.
    fx.w.clock = Some(Some(16.0));
    value(&mut fx, a, a_ctx).unwrap();
    fx.w.clock = Some(Some(20.0));
    assert_eq!(value(&mut fx, a, b_ctx).unwrap(), b2, "B first, alive");
    assert_eq!(saved(&mut fx, r).len(), 2, "A after it untouched");
    // No world: the clock is 0.
    fx.w.clock = Some(None);
    let (z, zr) = random_kept(&mut fx, &c, 2.0);
    let v = value(&mut fx, z, a_ctx).unwrap();
    assert_eq!(saved(&mut fx, zr), vec![("Toy_A".to_string(), v, 2.0)]);
    // A lifetime at most 0.0001: a draw every read, nothing saved, no clock asked.
    fx.w.clock = None;
    let (n, nr) = random_kept(&mut fx, &c, 0.0001);
    let before = fx.w.streams.len();
    value(&mut fx, n, a_ctx).unwrap();
    value(&mut fx, n, a_ctx).unwrap();
    assert_eq!(fx.w.streams.len(), before + 2);
    assert!(saved(&mut fx, nr).is_empty());
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
    // Off asks other queries: the local-player one does not answer it.
    mode(&mut fx, ParityMode::Off);
    fx.w.local_pools = Some(vec![]);
    assert!(matches!(not_yet(value(&mut fx, a, source)), Missing::LayerAbsent(_)));
}

fn off_pool(fx: &mut Fx, c: &Cls, max: f32, min: f32, current: f32, untouched: bool) -> ObjRef {
    fx.obj(c.pool, "Toy_OffPool", vec![
        ("MaxValue", Value::Float(max)),
        ("MinValue", Value::Float(min)),
        ("CurrentValue", Value::Float(current)),
        ("bCreatedAndNotModified", Value::Bool(untouched)),
    ])
}

/// Off (decision 80): every controller's pool, measured from its `MinValue` against `MaxValue * threshold`, pools
/// never modified skipped, the weights summed; no pool counted gives the no-pool weight, pools counted but none
/// below the threshold the above weight. Weights as `ammo`: threshold 0.4, above 0.03, min 0.1, max 0.25, no pool 0.5.
#[test]
fn ammo_drop_weight_off_sums_every_controllers_pool() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Off);
    let (a, resource) = ammo(&mut fx, &c);
    let source = fx.obj(c.pool, "Toy_Killed", vec![]);
    let (one, two, three) = (
        fx.obj(c.controller, "Toy_C1", vec![]),
        fx.obj(c.controller, "Toy_C2", vec![]),
        fx.obj(c.controller, "Toy_C3", vec![]),
    );
    let term = |f: f32| 0.1f32 + (0.25f32 - 0.1f32) * f.min(1.0);
    let run = |fx: &mut Fx, list: Vec<ObjRef>, pools: Vec<(ObjRef, Option<ObjRef>)>| {
        fx.w.controllers = Some(list);
        fx.w.controller_pools = Some(pools.into_iter().map(|(c, p)| (c, resource, p)).collect());
        value(fx, a, source).unwrap()
    };
    // An empty list, a controller without the pool, a pool never modified: no pool counted.
    assert_eq!(run(&mut fx, vec![], vec![]), 0.5);
    assert_eq!(run(&mut fx, vec![one], vec![(one, None)]), 0.5);
    let fresh = off_pool(&mut fx, &c, 100.0, 0.0, 0.0, true);
    assert_eq!(run(&mut fx, vec![one], vec![(one, Some(fresh))]), 0.5);
    // One pool at 20 of M = 40 from 0: f = 0.5.
    let half = off_pool(&mut fx, &c, 100.0, 0.0, 20.0, false);
    assert_eq!(run(&mut fx, vec![one], vec![(one, Some(half))]), term(0.5));
    // Measured from MinValue: at 10 with MinValue 10, f = (40 - 10) / (40 - 10) = 1, the maximum weight.
    let floor = off_pool(&mut fx, &c, 100.0, 10.0, 10.0, false);
    assert_eq!(run(&mut fx, vec![one], vec![(one, Some(floor))]), 0.25);
    // Above M (50 > 40): counted, adds nothing; alone, the above weight.
    let full = off_pool(&mut fx, &c, 100.0, 0.0, 50.0, false);
    assert_eq!(run(&mut fx, vec![one], vec![(one, Some(full))]), 0.03);
    // M - m not above 0 (MinValue 40): counted, adds nothing.
    let flat = off_pool(&mut fx, &c, 100.0, 40.0, 40.0, false);
    assert_eq!(run(&mut fx, vec![one], vec![(one, Some(flat))]), 0.03);
    // Every controller, summed in list order: 0.5 and 1 add, the full one adds nothing.
    let empty = off_pool(&mut fx, &c, 100.0, 0.0, 0.0, false);
    let got = run(&mut fx, vec![one, two, three], vec![(one, Some(half)), (two, Some(full)), (three, Some(empty))]);
    assert_eq!(got, 0.0f32 + term(0.5) + term(1.0));
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

/// Off (decision 80): the note's rule is the stand-in's, so Off clamps as Reference does.
#[test]
fn constraint_off_clamps_as_the_stand_in() {
    let mut fx = Fx::new();
    let c = classes(&mut fx);
    mode(&mut fx, ParityMode::Off);
    let source = fx.obj(c.pool, "Toy_Source", vec![]);
    for (v, min, max, want) in [(5.0, Some(2.0), Some(4.0), 4.0), (1.0, Some(2.0), None, 2.0), (1.0, Some(6.0), Some(4.0), 4.0), (9.0, None, None, 9.0)] {
        let a = clamped(&mut fx, v, min, max);
        assert_eq!(value(&mut fx, a, source).unwrap(), want, "{v} {min:?} {max:?}");
    }
}
