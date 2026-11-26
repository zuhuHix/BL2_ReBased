//! `TargetableAttributeValueResolver` on a synthetic world (rule 9: invented objects; the class, field and attribute
//! names are the packages' names, so the resolver meets the shapes it reads).

use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
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

/// The Off rule (lane IMPL-TARGETABLE-RESOLVER; NATIVE_TARGETABLE_RESOLVER.md section 6). Invented classes on the
/// packages' paths: the targetable classes and the AI pawn's balance, with the class table's native mark standing for
/// the first native class (these classes have no code in the store).
struct Off {
    fx: Fx,
    champion: ObjRef,
    resolver: ObjRef,
}

impl Off {
    fn new(store: bool) -> Off {
        let mut fx = Fx::new();
        if store {
            mode(&mut fx, ParityMode::Off);
        }
        let champion = attr(&mut fx, "Targetable_Is_Champion", true);
        let resolver = {
            let p = fx.w.space.prop(fx.c.attribute, "ValueResolverChain").unwrap();
            let chain = fx.w.space.read(champion, PropAddr::scalar(p)).unwrap();
            let Value::Object(Some(r)) = chain.elements().unwrap()[0] else { panic!("no resolver") };
            r
        };
        let object = fx.c.object;
        let space = &mut fx.w.space;
        let define = |space: &mut ObjectSpace, path: &str, sup: ClassId, props: Vec<(&str, PropKind)>, native: bool| {
            let decls = props.into_iter().map(|(n, k)| PropertyDecl::new(space.names.intern(n), k)).collect();
            let c = space.classes.define(path, Some(sup), decls).unwrap();
            if native {
                space.classes.mark_native(c).unwrap();
            }
            c
        };
        let bdd = PropertyDecl::new(space.names.intern("BalanceDefinition"), PropKind::Object { class: object });
        let state = space.classes.define_struct("WillowGame.WillowAIPawn.BalanceDefinitionState", None, vec![bdd]).unwrap();
        let actor = define(space, "Engine.Actor", object, vec![], false);
        let pawn = define(space, "Engine.Pawn", actor, vec![], true);
        let willow = define(space, "WillowGame.WillowPawn", pawn, vec![], true);
        define(space, "WillowGame.WillowPlayerPawn", willow, vec![], true);
        let ai = define(space, "WillowGame.WillowAIPawn", willow, vec![("BalanceDefinitionState", PropKind::Struct(state))], true);
        // A script subclass, as a population's pawn class is: its first native class is the AI pawn.
        define(space, "Toy.ToyAIPawn", ai, vec![], false);
        define(space, "WillowGame.WillowVehicle", pawn, vec![], true);
        define(space, "WillowGame.WillowProjectile", actor, vec![], true);
        define(space, "WillowGame.WillowInteractiveObject", actor, vec![], true);
        let controller = define(space, "Engine.Controller", actor, vec![], true);
        define(space, "WillowGame.WillowMind", controller, vec![], true);
        let base = define(space, "GearboxFramework.BaseBalanceDefinition", object, vec![("ToyField", PropKind::Bool)], true);
        define(space, "WillowGame.AIPawnBalanceDefinition", base, vec![("Champion", PropKind::Bool)], true);
        define(space, "WillowGame.InventoryBalanceDefinition", base, vec![], true);
        Off { fx, champion, resolver }
    }

    fn make(&mut self, path: &str) -> ObjRef {
        let c = self.fx.by_path(path);
        self.fx.w.space.alloc(c, "Toy", None, Origin::Runtime).unwrap()
    }

    fn lazy(&mut self, path: &str) -> ObjRef {
        let c = self.fx.by_path(path);
        self.fx.w.space.alloc_stand_in(c, "Lazy", Origin::Default { class: c }).unwrap()
    }

    /// An AI pawn (of `class`) on `balance`.
    fn ai(&mut self, class: &str, balance: Option<ObjRef>) -> ObjRef {
        let pawn = self.make(class);
        let space = &mut self.fx.w.space;
        let st = space.classes.struct_by_path("WillowGame.WillowAIPawn.BalanceDefinitionState").unwrap();
        let p = space.prop(space.heap.get(pawn).unwrap().class, "BalanceDefinitionState").unwrap();
        *space.slot_mut(pawn, PropAddr::scalar(p)).unwrap() = Value::structure(st, vec![Value::Object(balance)]);
        pawn
    }

    fn balance(&mut self, path: &str, champion: Option<bool>) -> ObjRef {
        let b = self.make(path);
        if let Some(c) = champion {
            let space = &mut self.fx.w.space;
            let p = space.prop(space.heap.get(b).unwrap().class, "Champion").unwrap();
            *space.slot_mut(b, PropAddr::scalar(p)).unwrap() = Value::Bool(c);
        }
        b
    }

    fn set_value(&mut self, v: u8) {
        let space = &mut self.fx.w.space;
        let p = space.prop(space.heap.get(self.resolver).unwrap().class, "ValueToResolve").unwrap();
        *space.slot_mut(self.resolver, PropAddr::scalar(p)).unwrap() = Value::Byte(v);
    }

    fn marks(&self) -> (u64, u64, usize) {
        let w = &self.fx.w;
        (w.space.heap.committed_writes(), w.defs.as_ref().map_or(0, |d| d.load_transitions()), w.space.names.len())
    }

    /// The value and the base value through the system (they must agree), with nothing written, loaded or interned.
    fn ask(&mut self, context: ObjRef) -> Result<f32, AttrError> {
        let before = self.marks();
        let v = value(&mut self.fx, self.champion, context);
        let b = base(&mut self.fx, self.champion, context);
        assert_eq!(self.marks(), before, "nothing written, loaded or interned");
        match (&v, &b) {
            (Ok(x), Ok(y)) => assert_eq!(x, y, "the base value equals the value"),
            (Err(x), Err(y)) => assert_eq!(text(x), text(y), "the base value refuses as the value"),
            _ => panic!("value {v:?} and base {b:?} differ"),
        }
        v
    }

    fn refused(&mut self, context: ObjRef) -> String {
        let e = self.ask(context).unwrap_err();
        let AttrError::NotYet(n) = &e else { panic!("{e:?}") };
        assert_eq!((n.missing.clone(), n.record), (Missing::NotRead, super::targetable::RECORD_OFF), "{e}");
        n.what.to_string()
    }
}

// 1 and 7.
#[test]
fn off_an_ai_pawn_answers_its_balance_s_champion() {
    for store in [true, false] {
        let mut x = Off::new(store);
        for (champion, want) in [(true, 1.0), (false, 0.0)] {
            let b = x.balance("WillowGame.AIPawnBalanceDefinition", Some(champion));
            for class in ["WillowGame.WillowAIPawn", "Toy.ToyAIPawn"] {
                let pawn = x.ai(class, Some(b));
                assert_eq!(x.ask(pawn).unwrap(), want, "{class} champion {champion}");
            }
        }
        let pawn = x.ai("WillowGame.WillowAIPawn", None);
        assert_eq!(x.ask(pawn).unwrap(), 0.0, "no balance");
    }
}

// 2 and 3.
#[test]
fn off_the_never_champion_pawns_a_mind_and_no_context_answer_zero() {
    let mut x = Off::new(true);
    for class in ["WillowGame.WillowPlayerPawn", "WillowGame.WillowPawn", "Engine.Pawn", "WillowGame.WillowMind"] {
        let o = x.make(class);
        assert_eq!(x.ask(o).unwrap(), 0.0, "{class}");
    }
    let before = x.marks();
    let mut cx = EvalCx::new(&mut x.fx.w);
    assert_eq!(super::targetable::read_off(&mut cx, x.resolver, None).unwrap(), 0.0, "no context");
    assert_eq!(x.marks(), before);
}

// 4.
#[test]
fn off_a_value_to_resolve_other_than_is_champion_answers_zero() {
    let mut x = Off::new(true);
    let b = x.balance("WillowGame.AIPawnBalanceDefinition", Some(true));
    let pawn = x.ai("WillowGame.WillowAIPawn", Some(b));
    assert_eq!(x.ask(pawn).unwrap(), 1.0);
    x.set_value(1);
    assert_eq!(x.ask(pawn).unwrap(), 0.0, "TARGETABLE_MAX");
}

// 5.
#[test]
fn off_is_champion_of_a_class_not_read_is_named() {
    let mut x = Off::new(true);
    for class in ["WillowGame.WillowVehicle", "WillowGame.WillowProjectile", "WillowGame.WillowInteractiveObject"] {
        let o = x.make(class);
        assert_eq!(x.refused(o), format!("{} {class}", super::targetable::IS_CHAMPION));
    }
}

// 6.
#[test]
fn off_another_balance_class_and_lazy_objects_are_not_read() {
    let mut x = Off::new(true);
    let other = x.balance("WillowGame.InventoryBalanceDefinition", None);
    let pawn = x.ai("WillowGame.WillowAIPawn", Some(other));
    assert_eq!(
        x.refused(pawn),
        format!("{} WillowGame.InventoryBalanceDefinition", super::targetable::DOES_SPAWN_CHAMPION)
    );
    let lazy_balance = x.lazy("WillowGame.AIPawnBalanceDefinition");
    let pawn = x.ai("WillowGame.WillowAIPawn", Some(lazy_balance));
    assert_eq!(x.refused(pawn), super::targetable::BALANCE_NOT_LOADED);
    let lazy_pawn = x.lazy("WillowGame.WillowAIPawn");
    assert_eq!(x.refused(lazy_pawn), super::targetable::CONTEXT_NOT_LOADED);
}

// Step 0 (review of #783): a resolver the store has not loaded is not loaded by the get.
#[test]
fn off_an_unloaded_resolver_is_not_read_and_not_loaded() {
    let mut x = Off::new(true);
    let pawn = x.make("WillowGame.WillowPlayerPawn");
    let class = x.fx.w.space.heap.get(x.resolver).unwrap().class;
    x.fx.w.space.heap.get_mut(x.resolver).unwrap().slots.clear();
    assert!(x.fx.w.space.classes.get_layout(class).unwrap().slot_count() > 0);
    let before = x.marks();
    let mut cx = EvalCx::new(&mut x.fx.w);
    let e = super::targetable::read_off(&mut cx, x.resolver, Some(pawn)).unwrap_err();
    let AttrError::NotYet(n) = &e else { panic!("{e:?}") };
    assert_eq!((n.missing.clone(), n.what.as_ref()), (Missing::NotRead, super::targetable::RESOLVER_NOT_LOADED));
    assert_eq!(x.marks(), before, "nothing loaded");
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
