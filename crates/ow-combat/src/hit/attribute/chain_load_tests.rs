//! The walks load each chain entry before they dispatch it (lane IMPL-RESOLVER-CHAIN-LOADS; NATIVE_TARGETABLE_RESOLVER.md
//! section 5.1, its test sketch rows 1 to 6). An invented `WillowGame.upk` (AGENTS.md rule 9) beside the `Engine` and
//! `GearboxFramework` of [`super::tests`]: a toy `TargetableAttributeValueResolver` and attribute definitions whose
//! chain entries are subobject exports of the attribute, as the installed `Targetable_Is_Champion`'s resolver is, so a
//! store load of the attribute leaves them unloaded handles. AI-assisted (Claude Code).

use ow_core::{Missing, ObjectPath};
use ow_object::value::Value;
use ow_package::testkit::{Export, ints};
use ow_package::testkit::script::ScriptToy;

use super::tests::Fx;
use super::*;

/// The invented `WillowGame`: the resolver class, and three attributes (`Att_Toy`: context and value chains one
/// subobject each; `Att_Gap`: a None context entry before its resolver; `Att_Broken`: a value resolver whose payload is
/// cut short, so its load fails).
pub(super) fn willow() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Byte", "Object", "Array"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let attribute = t.p.add_import("Core", "Class", engine, "AttributeDefinition");
    let pawn_context = t.p.add_import("Core", "Class", engine, "PawnAttributeContextResolver");
    let targetable = t.class("TargetableAttributeValueResolver", object);
    t.prop("Byte", targetable, "ValueToResolve", 0, 0);
    let none = t.none;
    let refs = |r: &[i32]| r.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let sub = |t: &mut ScriptToy, class: i32, name: &str, outer: i32, payload: Vec<u8>| {
        let mut e = Export::new(class, name, payload);
        e.outer = outer;
        t.p.add_export_with(e)
    };
    let empty = |none: &[u8]| {
        let mut p = ints(&[0]);
        p.extend_from_slice(none);
        p
    };
    // Att_Toy, then its two subobjects.
    let a = t.p.next_export();
    let (ctx, res) = (a + 1, a + 2);
    let mut p = ints(&[0]);
    p.extend(t.p.array_tag("ContextResolverChain", 1, &refs(&[ctx])));
    p.extend(t.p.array_tag("ValueResolverChain", 1, &refs(&[res])));
    p.extend_from_slice(&none);
    assert_eq!(t.p.add_export(attribute, "Att_Toy", p), a);
    assert_eq!(sub(&mut t, pawn_context, "PawnAttributeContextResolver_0", a, empty(&none)), ctx);
    assert_eq!(sub(&mut t, targetable, "TargetableAttributeValueResolver_0", a, empty(&none)), res);
    // Att_Gap: a None context entry, then a context resolver; the same value resolver shape.
    let g = t.p.next_export();
    let (ctx, res) = (g + 1, g + 2);
    let mut p = ints(&[0]);
    p.extend(t.p.array_tag("ContextResolverChain", 2, &refs(&[0, ctx])));
    p.extend(t.p.array_tag("ValueResolverChain", 1, &refs(&[res])));
    p.extend_from_slice(&none);
    assert_eq!(t.p.add_export(attribute, "Att_Gap", p), g);
    sub(&mut t, pawn_context, "PawnAttributeContextResolver_0", g, empty(&none));
    sub(&mut t, targetable, "TargetableAttributeValueResolver_0", g, empty(&none));
    // Att_Broken: its value resolver's payload is cut short.
    let b = t.p.next_export();
    let (ctx, res) = (b + 1, b + 2);
    let mut p = ints(&[0]);
    p.extend(t.p.array_tag("ContextResolverChain", 1, &refs(&[ctx])));
    p.extend(t.p.array_tag("ValueResolverChain", 1, &refs(&[res])));
    p.extend_from_slice(&none);
    assert_eq!(t.p.add_export(attribute, "Att_Broken", p), b);
    sub(&mut t, pawn_context, "PawnAttributeContextResolver_0", b, empty(&none));
    let mut broken = 0i32.to_le_bytes().to_vec();
    broken.extend_from_slice(&[0xff; 16]);
    sub(&mut t, targetable, "TargetableAttributeValueResolver_0", b, broken);
    t.p.build()
}

impl Fx {
    fn handle(&mut self, path: &str) -> ObjRef {
        let path = ObjectPath::parse(path).unwrap();
        let vm = self.vm();
        vm.defs.find(&mut vm.space, &path).unwrap().unwrap()
    }
    /// The attribute's chain entries, in order (`None` kept), read from the loaded attribute.
    fn chain(&mut self, attr: ObjRef, name: &str) -> Vec<Option<ObjRef>> {
        match self.value(attr, name) {
            Value::Array(items) => items.iter().map(|v| if let Value::Object(o) = v { *o } else { panic!("{v:?}") }).collect(),
            other => panic!("{other:?}"),
        }
    }
    fn unloaded(&mut self, o: ObjRef) -> bool {
        let vm = self.vm();
        vm.defs.is_unloaded(&vm.space, o)
    }
    fn loads(&mut self) -> u64 {
        self.vm().defs.load_transitions()
    }
    /// The attribute loaded by a direct store call (standing in for the VM's `loaded_for_read`).
    fn load_direct(&mut self, attr: ObjRef) {
        let vm = self.vm();
        vm.defs.ensure_loaded(&mut vm.space, attr).unwrap();
    }
}

// Rows 1, 2 and 5: the attribute is loaded (here by the store's lookup), and an evaluation through `GetValue`
// (`preflight_read` first) loads each of its two unloaded chain entries: 1+N = 3 load transitions. The get answers by
// section 5 (0.0 on a pawn), with no step-0 refusal.
#[test]
fn a_read_loads_the_attribute_then_each_chain_entry() {
    let mut fx = Fx::new(ParityMode::Off);
    let pawn = fx.pawn(1.0);
    // The lookup loads the attribute (the store's `find`), so the count starts before it.
    let before = fx.loads();
    let a = fx.handle("WillowGame.Att_Toy");
    let (ret, out) = fx.get_value(a, Value::Object(Some(pawn)), Value::Object(None)).unwrap();
    assert_eq!((ret, out), (Value::Float(0.0), Value::Object(Some(pawn))), "section 5: a pawn is never a champion");
    let (ctx, res) = (fx.chain(a, "ContextResolverChain")[0].unwrap(), fx.chain(a, "ValueResolverChain")[0].unwrap());
    assert!(!fx.unloaded(ctx), "row 2: the context walk loaded its entry");
    assert!(!fx.unloaded(res), "row 1: the value walk loaded its entry");
    assert_eq!(fx.loads() - before, 3, "row 5: 1+N load transitions (the attribute, then N = 2 entries)");
}

// Row 3: the attribute resident, the resolver a handle. In a load-free scope: the existing `NotRead` naming the entry,
// nothing loaded. Loaded by a direct store call, then a read outside a scope: the walk loads the resolver.
#[test]
fn a_resident_attribute_s_handle_is_refused_in_a_load_free_scope_and_loaded_outside() {
    let mut fx = Fx::new(ParityMode::Off);
    let pawn = fx.pawn(1.0);
    let a = fx.handle("WillowGame.Att_Toy");
    fx.load_direct(a);
    let res = fx.chain(a, "ValueResolverChain")[0].unwrap();
    assert!(fx.unloaded(res), "a direct load of the attribute leaves its entries handles");
    let attrs = fx.vm().attrs.clone();
    let before = fx.loads();
    let got = ow_attr::eval::load_free("chain-load test", || {
        attrs.value_of(a, Some(pawn), None, &mut EvalCx::new(fx.vm()))
    });
    match got {
        Err(ow_attr::api::AttrError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.contains("is an unloaded handle (a load is a write)"), "{n:?}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(fx.loads(), before, "nothing loaded in the scope");
    let v = attrs.value_of(a, Some(pawn), None, &mut EvalCx::new(fx.vm())).unwrap().unwrap();
    assert_eq!(v.value, 0.0);
    assert!(!fx.unloaded(res), "the walk loaded the resolver");
}

// Row 4: a None chain entry is skipped by the walk's load, as the game's walks skip it.
#[test]
fn a_none_chain_entry_is_skipped() {
    let mut fx = Fx::new(ParityMode::Off);
    let pawn = fx.pawn(1.0);
    let a = fx.handle("WillowGame.Att_Gap");
    let (ret, out) = fx.get_value(a, Value::Object(Some(pawn)), Value::Object(None)).unwrap();
    assert_eq!((ret, out), (Value::Float(0.0), Value::Object(Some(pawn))));
    assert_eq!(fx.chain(a, "ContextResolverChain")[0], None);
}

// Row 6: a resolver whose load fails: the handle stays, and the walk's residency check answers the store's `BadData`
// where the entry is reached; no refusal before it.
#[test]
fn a_resolver_whose_load_fails_is_bad_data_where_the_walk_reaches_it() {
    let mut fx = Fx::new(ParityMode::Off);
    let pawn = fx.pawn(1.0);
    let a = fx.handle("WillowGame.Att_Broken");
    fx.load_direct(a);
    let res = fx.chain(a, "ValueResolverChain")[0].unwrap();
    let attrs = fx.vm().attrs.clone();
    let got = attrs.value_of(a, Some(pawn), None, &mut EvalCx::new(fx.vm()));
    match got {
        Err(ow_attr::api::AttrError::BadData { reason }) => assert!(reason.contains("failed to load"), "{reason}"),
        other => panic!("{other:?}"),
    }
    let vm = fx.vm();
    assert!(vm.defs.is_failed(&vm.space, res), "the failed handle is left in place");
    let ctx = fx.chain(a, "ContextResolverChain")[0].unwrap();
    assert!(!fx.unloaded(ctx), "the context walk ran first and loaded its entry: no refusal before the value entry");
}
