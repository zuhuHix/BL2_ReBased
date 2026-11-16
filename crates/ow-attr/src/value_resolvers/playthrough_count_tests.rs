//! `PlayThroughCountAttributeValueResolver` on a synthetic world (rule 9: invented objects and numbers; the class and
//! field names are the packages' names).

use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_package::store::PackageStore;

use super::playthrough_count::NOTE;
use super::testworld::{Fx, system};
use crate::api::{AttrError, Comparison, EvalCx, InitData, Operand2Usage};

const CLASS: &str = "WillowGame.PlayThroughCountAttributeValueResolver";
const MUL: u8 = 2;

/// A store-backed world in `mode` (a world without a store runs `Off`).
fn with_mode(fx: &mut Fx, mode: ParityMode) {
    fx.w.defs = Some(DefStore::new(
        PackageStore::new_empty(),
        IdentityPolicy::FirstInScanOrder,
        mode,
    ));
}

/// A resolver of the class with the given `IncludePlaythroughThree`.
fn resolver(fx: &mut Fx, include_three: i32) -> ObjRef {
    let c = fx.by_path(CLASS);
    fx.obj(
        c,
        "PlayThroughCountAttributeValueResolver_0",
        vec![("IncludePlaythroughThree", Value::Int(include_three))],
    )
}

/// The installed shape: a NoContextNeeded attribute whose value chain is `before` then the resolver.
fn count_attr(fx: &mut Fx, include_three: i32, before: Vec<ObjRef>) -> ObjRef {
    let ctx = fx.no_context();
    let r = resolver(fx, include_three);
    let mut values = before;
    values.push(r);
    fx.attr("PlayThroughCount", vec![ctx], values)
}

fn value(fx: &mut Fx, attr: ObjRef) -> Result<f32, AttrError> {
    let sys = system();
    sys.value_of(attr, None, None, &mut EvalCx::new(&mut fx.w))
        .map(|v| v.expect("the no-context chain resolves").value)
}

fn base(fx: &mut Fx, attr: ObjRef) -> Result<f32, AttrError> {
    let sys = system();
    sys.base_value_of(attr, None, None, &mut EvalCx::new(&mut fx.w))
        .map(|v| v.expect("the no-context chain resolves").value)
}

/// The mission_script_test.py reward shape on the real class: 0.5, then x a conditional (4 when PlayThroughCount == 2,
/// else 3).
fn reward(fx: &mut Fx, count: ObjRef) -> Result<f32, AttrError> {
    let e = fx.expr(
        Some(count),
        Comparison::EqualTo,
        Operand2Usage::PreferAttribute,
        None,
        2.0,
    );
    let (four, three) = (fx.k(4.0), fx.k(3.0));
    let cond = fx.conditional(true, vec![(vec![e], four)], three);
    let multiplier = fx.definition(vec![("ConditionalInitialization", cond)]);
    let (ctx, half) = (fx.no_context(), fx.constant(0.5));
    let arg = fx.of_def(multiplier);
    let math = fx.math(MUL, arg, None);
    let pct = fx.attr("XPReward", vec![ctx], vec![half, math]);
    let sys = system();
    let data = InitData {
        base_value_attribute: Some(pct),
        ..InitData::constant(0.0)
    };
    sys.evaluate_init(&data, None, None, &mut EvalCx::new(&mut fx.w))
}

fn not_ported_awaiting(r: Result<f32, AttrError>) {
    match r {
        Err(AttrError::NotYet(e)) => {
            assert_eq!(e.missing, Missing::NotPorted);
            assert!(
                e.what.contains("awaiting clean-room implementation"),
                "{}",
                e.what
            );
            assert_eq!(e.record, NOTE);
        }
        other => panic!("expected NotYet NotPorted, got {other:?}"),
    }
}

// ----- Reference mode: the C++ reading -----

#[test]
fn reference_reads_one_whatever_the_field() {
    for include_three in [0, 1, 7] {
        let mut fx = Fx::new();
        with_mode(&mut fx, ParityMode::Reference);
        let attr = count_attr(&mut fx, include_three, vec![]);
        assert_eq!(value(&mut fx, attr), Ok(1.0), "field {include_three}");
        // Get-base is the same read.
        assert_eq!(base(&mut fx, attr), Ok(1.0), "field {include_three}");
    }
}

#[test]
fn reference_ignores_the_previous_value() {
    let mut fx = Fx::new();
    with_mode(&mut fx, ParityMode::Reference);
    let seven = fx.constant(7.0);
    let attr = count_attr(&mut fx, 0, vec![seven]);
    assert_eq!(value(&mut fx, attr), Ok(1.0));
}

#[test]
fn reference_takes_the_first_playthrough_branch_of_a_conditional() {
    // 0.5 x 3: the "== 2" entry does not hold for a count of 1 (the C++ reward percentage reading).
    let mut fx = Fx::new();
    with_mode(&mut fx, ParityMode::Reference);
    let count = count_attr(&mut fx, 0, vec![]);
    assert_eq!(reward(&mut fx, count), Ok(1.5));
}

#[test]
fn reference_writes_are_not_ported() {
    let mut fx = Fx::new();
    with_mode(&mut fx, ParityMode::Reference);
    let r = resolver(&mut fx, 0);
    let globals = fx.w.globals;
    let sys = system();
    let res = sys.value_resolver(&fx.w.space, r).unwrap();
    match res.storage(&sys, r, globals, &mut EvalCx::new(&mut fx.w)) {
        Err(AttrError::NotYet(e)) => {
            assert_eq!(e.missing, Missing::NotPorted);
            assert_eq!(e.record, NOTE);
        }
        other => panic!("expected NotYet NotPorted, got {other:?}"),
    }
}

// ----- Off mode: the native rule waits for a clean-room implementation -----

#[test]
fn off_is_not_ported_yet_with_or_without_a_store() {
    for store in [true, false] {
        let mut fx = Fx::new();
        if store {
            with_mode(&mut fx, ParityMode::Off);
        }
        let attr = count_attr(&mut fx, 0, vec![]);
        not_ported_awaiting(value(&mut fx, attr));
        not_ported_awaiting(base(&mut fx, attr));
    }
}

#[test]
fn off_stops_a_conditional_that_reads_the_count() {
    let mut fx = Fx::new();
    with_mode(&mut fx, ParityMode::Off);
    let count = count_attr(&mut fx, 0, vec![]);
    not_ported_awaiting(reward(&mut fx, count));
}

#[test]
fn off_writes_are_not_ported() {
    let mut fx = Fx::new();
    let r = resolver(&mut fx, 1);
    let globals = fx.w.globals;
    let sys = system();
    let res = sys.value_resolver(&fx.w.space, r).unwrap();
    match res.storage(&sys, r, globals, &mut EvalCx::new(&mut fx.w)) {
        Err(AttrError::NotYet(e)) => assert_eq!(e.missing, Missing::NotPorted),
        other => panic!("expected NotYet NotPorted, got {other:?}"),
    }
}
