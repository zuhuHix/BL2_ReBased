//! `TargetableAttributeValueResolver` on a synthetic world (rule 9: invented objects; the class, field and attribute
//! names are the packages' names, so the resolver meets the shapes it reads).

use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::Origin;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::store::PackageStore;

use super::testworld::{Fx, system};
use crate::api::{AttrError, EvalCx};

const CLASS: &str = "WillowGame.TargetableAttributeValueResolver";

/// An attribute named `name` whose one value resolver is a Targetable resolver; the resolver is a subobject of the
/// attribute when `listed_as_subobject` (the installed shape), else a top-level object; the chain lists it either way.
fn attr(fx: &mut Fx, name: &str, listed_as_subobject: bool) -> ObjRef {
    let ctx = fx.no_context();
    let attr = fx.attr(name, vec![ctx], vec![]);
    let c = fx.by_path(CLASS);
    let outer = listed_as_subobject.then_some(attr);
    let r =
        fx.w.space
            .alloc(
                c,
                "TargetableAttributeValueResolver_0",
                outer,
                Origin::Runtime,
            )
            .unwrap();
    let p =
        fx.w.space
            .prop(fx.c.attribute, "ValueResolverChain")
            .unwrap();
    fx.w.space
        .write(
            attr,
            PropAddr::scalar(p),
            Value::array(vec![Value::Object(Some(r))]),
        )
        .unwrap();
    attr
}

fn mode(fx: &mut Fx, mode: ParityMode) {
    fx.w.defs = Some(DefStore::new(
        PackageStore::new_empty(),
        IdentityPolicy::FirstInScanOrder,
        mode,
    ));
}

fn value(fx: &mut Fx, attr: ObjRef, source: ObjRef) -> Result<f32, AttrError> {
    system()
        .value_of(attr, Some(source), None, &mut EvalCx::new(&mut fx.w))
        .map(|v| v.unwrap().value)
}

fn base(fx: &mut Fx, attr: ObjRef, source: ObjRef) -> Result<f32, AttrError> {
    system()
        .base_value_of(attr, Some(source), None, &mut EvalCx::new(&mut fx.w))
        .map(|v| v.unwrap().value)
}

fn missing(e: AttrError) -> Missing {
    match e {
        AttrError::NotYet(n) => n.missing,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn text(e: &AttrError) -> String {
    e.to_string()
}

#[test]
fn reference_answers_not_a_champion_for_the_champion_attribute_whatever_the_context() {
    let mut fx = Fx::new();
    mode(&mut fx, ParityMode::Reference);
    let champion = attr(&mut fx, "Targetable_Is_Champion", true);
    let c = fx.c.object;
    let a = fx.w.space.alloc(c, "A", None, Origin::Runtime).ok();
    let sources: Vec<ObjRef> = [a, Some(fx.no_context())].into_iter().flatten().collect();
    for source in sources {
        assert_eq!(value(&mut fx, champion, source).unwrap(), 0.0);
        assert_eq!(base(&mut fx, champion, source).unwrap(), 0.0);
    }
}

#[test]
fn reference_refuses_any_other_attribute_and_an_outer_that_does_not_list_it() {
    let mut fx = Fx::new();
    mode(&mut fx, ParityMode::Reference);
    let source = fx.no_context();
    let other = attr(&mut fx, "Targetable_Is_Something", true);
    let e = value(&mut fx, other, source).unwrap_err();
    assert!(
        text(&e).contains("stands in only Targetable_Is_Champion"),
        "{e}"
    );
    assert!(matches!(missing(e), Missing::NotRead));
    // Named as the champion attribute, but the resolver is not its subobject: not the installed shape.
    let loose = attr(&mut fx, "Targetable_Is_Champion", false);
    let e = value(&mut fx, loose, source).unwrap_err();
    assert!(text(&e).contains("is not an attribute definition"), "{e}");
    assert!(matches!(missing(e), Missing::NotRead));
}

#[test]
fn off_mode_and_a_world_without_a_store_are_not_yet_for_get_and_base() {
    for m in [Some(ParityMode::Off), None] {
        let mut fx = Fx::new();
        if let Some(m) = m {
            mode(&mut fx, m);
        }
        let champion = attr(&mut fx, "Targetable_Is_Champion", true);
        let source = fx.no_context();
        let e = value(&mut fx, champion, source).unwrap_err();
        assert!(text(&e).contains("its native is not read"), "{e}");
        assert!(matches!(missing(e), Missing::NotRead));
        let e = base(&mut fx, champion, source).unwrap_err();
        assert!(matches!(missing(e), Missing::NotRead));
    }
}

#[test]
fn writes_are_not_yet_in_both_modes() {
    for (m, expected) in [
        (ParityMode::Reference, "the C++ has no write"),
        (ParityMode::Off, "its native is not read"),
    ] {
        let mut fx = Fx::new();
        mode(&mut fx, m);
        let champion = attr(&mut fx, "Targetable_Is_Champion", true);
        let source = fx.no_context();
        let sys = system();
        let resolver = {
            let p =
                fx.w.space
                    .prop(fx.c.attribute, "ValueResolverChain")
                    .unwrap();
            let chain = fx.w.space.read(champion, PropAddr::scalar(p)).unwrap();
            let Value::Object(Some(r)) = chain.elements().unwrap()[0] else {
                panic!("no resolver")
            };
            r
        };
        let r = sys.value_resolver(&fx.w.space, resolver).unwrap();
        let e = r
            .storage(&sys, resolver, source, &mut EvalCx::new(&mut fx.w))
            .unwrap_err();
        assert!(text(&e).contains(expected), "{m:?}: {e}");
    }
}
