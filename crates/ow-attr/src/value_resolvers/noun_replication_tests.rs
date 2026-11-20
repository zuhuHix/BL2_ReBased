//! The providers' arrays and the replicated copy (NATIVE_NOUN_ATTRIBUTES.md section 10, lane IMPL-NOUN-REPLICATION),
//! on invented objects (rule 9). The classes carry the packages' paths and field names so the resolver reads the
//! shapes it reads from reflection; every number is invented.

use super::*;
use crate::api::{EvalCx, ProbeOp};

/// The provider classes of section 10.2, an unread pawn subclass and `WillowMind`.
struct Providers {
    rep: StructId,
    pawn: ClassId,
    ai_pawn: ClassId,
    player_pawn: ClassId,
    pri: ClassId,
    gearbox_mind: ClassId,
    willow_mind: ClassId,
    weapon: ClassId,
    other_pawn: ClassId,
}

fn providers(n: &mut Noun) -> Providers {
    providers_with(n, PropKind::Float)
}

/// [`providers`], with the replicated slot's `Value` declared as `value` (a malformed slot when not a float).
fn providers_with(n: &mut Noun, value: PropKind) -> Providers {
    let object = n.fx.c.object;
    let state = n.state;
    let space = &mut n.fx.w.space;
    let d = |space: &mut ow_object::heap::ObjectSpace, name: &str, k: PropKind| {
        PropertyDecl::new(space.names.intern(name), k)
    };
    let fields = vec![
        d(space, "NounName", PropKind::Name),
        d(space, "Value", value),
        d(space, "bOnlyRelevantToOwner", PropKind::Bool),
    ];
    let rep = space
        .classes
        .define_struct("GearboxFramework.INounAttributeProvider.ReplicatedNounAttributeState", None, fields)
        .unwrap();
    let full = |space: &mut ow_object::heap::ObjectSpace| {
        vec![
            d(space, "Role", PropKind::Byte { enum_: None }),
            d(space, "bNetDirty", PropKind::Bool),
            d(space, "NounState", PropKind::Array(Box::new(PropKind::Struct(state)))),
            d(space, "ReplicatedNouns", PropKind::Struct(rep)).with_array_dim(20),
            d(space, "NumReplicatedNouns", PropKind::Int),
        ]
    };
    let short = |space: &mut ow_object::heap::ObjectSpace| {
        vec![
            d(space, "Role", PropKind::Byte { enum_: None }),
            d(space, "bNetDirty", PropKind::Bool),
            d(space, "NounState", PropKind::Array(Box::new(PropKind::Struct(state)))),
        ]
    };
    let decls = full(space);
    let pawn = space.classes.define("WillowGame.WillowPawn", Some(object), decls).unwrap();
    let ai_pawn = space.classes.define("WillowGame.WillowAIPawn", Some(pawn), vec![]).unwrap();
    let player_pawn = space.classes.define("WillowGame.WillowPlayerPawn", Some(pawn), vec![]).unwrap();
    let other_pawn = space.classes.define("Test.OtherWillowPawn", Some(pawn), vec![]).unwrap();
    let decls = full(space);
    let pri = space.classes.define("WillowGame.WillowPlayerReplicationInfo", Some(object), decls).unwrap();
    let decls = short(space);
    let gearbox_mind = space.classes.define("GearboxFramework.GearboxMind", Some(object), decls).unwrap();
    let willow_mind = space.classes.define("WillowGame.WillowMind", Some(gearbox_mind), vec![]).unwrap();
    let decls = short(space);
    let weapon = space.classes.define("WillowGame.WillowWeapon", Some(object), decls).unwrap();
    Providers { rep, pawn, ai_pawn, player_pawn, pri, gearbox_mind, willow_mind, weapon, other_pawn }
}

impl Noun {
    /// An object of `class` with `Role` = `role` (and an empty `NounState`).
    fn provider(&mut self, class: ClassId, role: u8) -> ObjRef {
        self.fx.obj(class, "Provider", vec![("Role", Value::Byte(role)), ("NounState", Value::array(vec![]))])
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.fx.w.space.heap.get(o).unwrap().class;
        let p = self.fx.w.space.prop(class, name).unwrap();
        self.fx.w.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn put(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.fx.w.space.heap.get(o).unwrap().class;
        let p = self.fx.w.space.prop(class, name).unwrap();
        self.fx.w.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    /// `ReplicatedNouns[i]` as (name, value, owner-only).
    fn slot(&self, o: ObjRef, i: u32) -> (String, f32, bool) {
        let space = &self.fx.w.space;
        let class = space.heap.get(o).unwrap().class;
        let p = space.prop(class, "ReplicatedNouns").unwrap();
        let v = space.read(o, PropAddr::element(p, i)).unwrap().clone();
        let f = |n: &str| crate::eval::field(space, &v, n).unwrap().clone();
        let name = match f("NounName") {
            Value::Name(n) => space.names.text(n).into_owned(),
            other => panic!("{other:?}"),
        };
        let owner_only = matches!(f("bOnlyRelevantToOwner"), Value::Bool(true));
        (name, crate::eval::number("Value", &f("Value")).unwrap(), owner_only)
    }

    /// Fills `ReplicatedNouns[i]` with (name, value).
    fn fill_slot(&mut self, o: ObjRef, i: u32, rep: StructId, name: &str, value: f32) {
        let n = self.fx.name(name);
        let v = self.fx.st(rep, vec![("NounName", n), ("Value", Value::Float(value))]);
        let class = self.fx.w.space.heap.get(o).unwrap().class;
        let p = self.fx.w.space.prop(class, "ReplicatedNouns").unwrap();
        self.fx.w.space.write(o, PropAddr::element(p, i), v).unwrap();
    }

    /// An attribute whose Noun resolver has strategy `strategy` and the constant default `d`.
    fn noun_attr(&mut self, name: &str, strategy: u8, d: f32) -> ObjRef {
        let default = self.fx.k(d);
        let attr = self.attr(name, default);
        self.set_resolver(attr, "ReplicationStrategy", Value::Byte(strategy));
        attr
    }

    fn heap(&self) -> String {
        format!("{:?}", self.fx.w.space.heap)
    }
}

fn bad_reason(e: AttrError) -> String {
    match e {
        AttrError::BadData { reason } => reason.into_owned(),
        other => panic!("expected BadData, got {other:?}"),
    }
}

/// Section 10.3: a pawn (each read pawn class) or a replication info on authority creates a missing strategy-1 or
/// strategy-2 entry with a replicated slot: the count grows, the slot holds the key, `d` and the owner-only flag
/// (strategy 1 only), `bNetDirty` is set, then the entry is appended with that slot.
#[test]
fn a_read_provider_on_authority_creates_the_entry_with_a_slot() {
    for which in 0..4 {
        for strategy in [1u8, 2] {
            let mut n = off();
            let p = providers(&mut n);
            let class = [p.pawn, p.ai_pawn, p.player_pawn, p.pri][which];
            let attr = n.noun_attr("IsPhaselocked", strategy, 0.5);
            let ctx = n.provider(class, 3);
            n.put(ctx, "NumReplicatedNouns", Value::Int(2));
            assert_eq!(n.value(attr, ctx), Ok(0.5));
            assert_eq!(n.get(ctx, "NumReplicatedNouns"), Value::Int(3));
            assert_eq!(n.slot(ctx, 2), ("IsPhaselocked".to_string(), 0.5, strategy == 1));
            assert_eq!(n.get(ctx, "bNetDirty"), Value::Bool(true));
            assert_eq!(n.entries(ctx), vec![Entry { strategy, slot: 2, ..Noun::created("IsPhaselocked", 0.5) }]);
            // The second read finds the entry and creates nothing more.
            assert_eq!(n.base(attr, ctx), Ok(0.5));
            assert_eq!(n.get(ctx, "NumReplicatedNouns"), Value::Int(3));
        }
    }
}

/// Section 10.3, the full array: at 20 there is no slot. Nothing is written for it, `bNetDirty` stays, the entry gets
/// slot -1.
#[test]
fn a_full_replicated_array_gives_no_slot() {
    let mut n = off();
    let p = providers(&mut n);
    let attr = n.noun_attr("IsPhaselocked", 2, 1.0);
    let ctx = n.provider(p.pawn, 3);
    n.put(ctx, "NumReplicatedNouns", Value::Int(20));
    let before = n.slot(ctx, 19);
    assert_eq!(n.value(attr, ctx), Ok(1.0));
    assert_eq!(n.get(ctx, "NumReplicatedNouns"), Value::Int(20));
    assert_eq!(n.get(ctx, "bNetDirty"), Value::Bool(false));
    assert_eq!(n.slot(ctx, 19), before);
    assert_eq!(n.entries(ctx), vec![Entry { strategy: 2, ..Noun::created("IsPhaselocked", 1.0) }]);
}

/// The mirror (section 10.3): a write to a slotted entry writes the entry, then the slot's `Value`, then `bNetDirty`.
#[test]
fn writes_to_a_slotted_entry_are_mirrored() {
    let mut n = off();
    let p = providers(&mut n);
    let attr = n.noun_attr("IsPhaselocked", 2, 0.0);
    let ctx = n.provider(p.ai_pawn, 3);
    assert_eq!(n.value(attr, ctx), Ok(0.0));
    n.put(ctx, "bNetDirty", Value::Bool(false));
    assert_eq!(n.set_base(attr, ctx, 1.0), Ok(true));
    assert_eq!(n.slot(ctx, 0), ("IsPhaselocked".to_string(), 1.0, false));
    assert_eq!(n.get(ctx, "bNetDirty"), Value::Bool(true));
    n.put(ctx, "bNetDirty", Value::Bool(false));
    let m = n.modifier(PRE, 2.0);
    assert_eq!(n.add(attr, ctx, m), Ok(AddModifierOutcome::Added));
    assert_eq!(n.value(attr, ctx), Ok(3.0));
    assert_eq!(n.slot(ctx, 0).1, 3.0);
    assert_eq!(n.get(ctx, "bNetDirty"), Value::Bool(true));
    // A duplicate add writes nothing.
    n.put(ctx, "bNetDirty", Value::Bool(false));
    assert_eq!(n.add(attr, ctx, m), Ok(AddModifierOutcome::Duplicate));
    assert_eq!(n.get(ctx, "bNetDirty"), Value::Bool(false));
    assert_eq!(n.remove(attr, ctx, m), Ok(RemoveModifierOutcome::Removed { count: 1 }));
    assert_eq!(n.slot(ctx, 0).1, 1.0);
    assert_eq!(n.get(ctx, "bNetDirty"), Value::Bool(true));
}

/// Section 10.4: off authority, a pawn or a replication info hands over no `NounState`. Strategy 0 has no store;
/// strategies 1 and 2 read `ReplicatedNouns` alone (the first matching occupied slot, else 0.0; get-base 0.0) and
/// write nothing.
#[test]
fn off_authority_a_pawn_reads_its_replicated_array_alone() {
    for class in [0, 1] {
        let mut n = off();
        let p = providers(&mut n);
        let class = [p.player_pawn, p.pri][class];
        let ctx = n.provider(class, 1);
        n.fill_slot(ctx, 0, p.rep, "Other", 9.0);
        n.fill_slot(ctx, 1, p.rep, "IsPhaselocked", 4.0);
        n.fill_slot(ctx, 2, p.rep, "Unoccupied", 7.0);
        n.put(ctx, "NumReplicatedNouns", Value::Int(2));
        let attr = n.noun_attr("IsPhaselocked", 2, 1.0);
        let before = n.heap();
        assert_eq!(n.value(attr, ctx), Ok(4.0));
        assert_eq!(n.base(attr, ctx), Ok(0.0));
        assert_eq!(n.set_base(attr, ctx, 5.0), Ok(false));
        assert_eq!(n.heap(), before, "nothing written");
        // A slot past the count is not read.
        let unoccupied = n.noun_attr("Unoccupied", 2, 1.0);
        assert_eq!(n.value(unoccupied, ctx), Ok(0.0));
        // Strategy 0 has no store off authority.
        let plain = n.noun_attr("Attribute_A", 0, 1.0);
        assert_eq!(n.value(plain, ctx), Ok(0.0));
        assert!(n.entries(ctx).is_empty());
    }
}

/// `GearboxMind` hands over `NounState` on authority only and no replicated array; `WillowWeapon` hands over
/// `NounState` always. Strategies 1 and 2 have no store on either.
#[test]
fn a_mind_and_a_weapon_hand_over_their_arrays_as_the_note_reads() {
    let mut n = off();
    let p = providers(&mut n);
    let plain = n.noun_attr("Attribute_A", 0, 2.0);
    let replicated = n.noun_attr("IsPhaselocked", 2, 1.0);
    let mind = n.provider(p.gearbox_mind, 3);
    assert_eq!(n.value(plain, mind), Ok(2.0));
    assert_eq!(n.entries(mind), vec![Noun::created("Attribute_A", 2.0)]);
    assert_eq!(n.value(replicated, mind), Ok(0.0));
    assert_eq!(n.set_base(replicated, mind, 1.0), Ok(false));
    assert_eq!(n.entries(mind).len(), 1, "nothing created for strategy 2");
    let remote = n.provider(p.gearbox_mind, 1);
    assert_eq!(n.value(plain, remote), Ok(0.0));
    assert!(n.entries(remote).is_empty());
    let weapon = n.provider(p.weapon, 1);
    assert_eq!(n.value(plain, weapon), Ok(2.0), "a weapon hands over NounState whatever its Role");
    assert_eq!(n.entries(weapon), vec![Noun::created("Attribute_A", 2.0)]);
    assert_eq!(n.value(replicated, weapon), Ok(0.0));
    assert_eq!(n.entries(weapon).len(), 1);
}

/// Preflight step 3: an unread class (`WillowMind`, a pawn subclass not read) refuses strategies 1 and 2 with the
/// note's `NotRead`, and keeps today's strategy-0 path with no `Role` test.
#[test]
fn an_unread_class_refuses_the_replicated_strategies_only() {
    let mut n = off();
    let p = providers(&mut n);
    for (class, path) in [(p.willow_mind, "WillowGame.WillowMind"), (p.other_pawn, "Test.OtherWillowPawn")] {
        let ctx = n.provider(class, 0);
        let replicated = n.noun_attr("IsPhaselocked", 2, 1.0);
        let before = n.heap();
        match n.value(replicated, ctx) {
            Err(AttrError::NotYet(e)) => {
                assert_eq!(e.missing, Missing::NotRead);
                assert_eq!(
                    e.what,
                    format!(
                        "NounAttributeValueResolver: the noun arrays a {path} hands over, and its \
                         replicated-copy-changed call, are not read (ReplicationStrategy 2)"
                    )
                );
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(n.heap(), before, "nothing written");
        let plain = n.noun_attr("Attribute_A", 0, 2.0);
        assert_eq!(n.value(plain, ctx), Ok(2.0), "strategy 0 with Role 0 is today's path");
        assert_eq!(n.entries(ctx), vec![Noun::created("Attribute_A", 2.0)]);
    }
}

/// Preflight steps 4 to 6 leave the heap unchanged: a count outside 0..=20, a slot outside the occupied ones and a
/// missing or mistyped declaration are `BadData` with the note's texts, before `d` is evaluated.
#[test]
fn the_preflight_writes_nothing_and_names_the_fault() {
    let mut n = off();
    let p = providers(&mut n);
    let attr = n.noun_attr("IsPhaselocked", 2, 1.0);
    let ctx = n.provider(p.pawn, 3);
    n.put(ctx, "NumReplicatedNouns", Value::Int(21));
    let before = n.heap();
    let reason = bad_reason(n.value(attr, ctx).unwrap_err());
    assert_eq!(reason, "NounAttributeValueResolver: Provider holds NumReplicatedNouns 21, outside 0..=20");
    assert_eq!(n.heap(), before);
    // An existing entry whose slot is not occupied (the count is 1, the slot 3).
    n.put(ctx, "NumReplicatedNouns", Value::Int(1));
    let e = Entry { strategy: 2, slot: 3, ..Noun::created("IsPhaselocked", 1.0) };
    let noun = n.full_noun(&e);
    n.put(ctx, "NounState", Value::array(vec![noun]));
    let before = n.heap();
    let reason = bad_reason(n.set_base(attr, ctx, 2.0).unwrap_err());
    assert_eq!(
        reason,
        "NounAttributeValueResolver: the entry IsPhaselocked on Provider holds IndexInReplicationArray 3, outside -1 \
         and the occupied slots 0..1"
    );
    assert_eq!(n.heap(), before);
    // A mind's entry with any slot but -1 is malformed (no replicated array).
    let mind = n.provider(p.gearbox_mind, 3);
    let e = Entry { slot: 0, ..Noun::created("Attribute_A", 1.0) };
    let noun = n.full_noun(&e);
    n.put(mind, "NounState", Value::array(vec![noun]));
    let plain = n.noun_attr("Attribute_A", 0, 1.0);
    let reason = bad_reason(n.value(plain, mind).unwrap_err());
    assert!(reason.ends_with("outside -1 and the occupied slots 0..0"), "{reason}");
    // A missing and a mistyped declaration.
    let mut m = off();
    let object = m.fx.c.object;
    let state = m.state;
    let space = &mut m.fx.w.space;
    let decls = vec![PropertyDecl::new(space.names.intern("NounState"), PropKind::Array(Box::new(PropKind::Struct(state))))];
    let bare = space.classes.define("WillowGame.WillowWeapon", Some(object), decls).unwrap();
    let decls = vec![
        PropertyDecl::new(space.names.intern("Role"), PropKind::Int),
        PropertyDecl::new(space.names.intern("NounState"), PropKind::Array(Box::new(PropKind::Struct(state)))),
    ];
    let odd = space.classes.define("GearboxFramework.GearboxMind", Some(object), decls).unwrap();
    let attr = m.noun_attr("Attribute_A", 0, 1.0);
    let weapon = m.fx.obj(bare, "Weapon", vec![]);
    let before = m.heap();
    assert_eq!(
        bad_reason(m.value(attr, weapon).unwrap_err()),
        "NounAttributeValueResolver: WillowGame.WillowWeapon has no Role"
    );
    assert_eq!(m.heap(), before);
    let mind = m.fx.obj(odd, "Mind", vec![]);
    let before_mind = m.heap();
    assert_eq!(
        bad_reason(m.value(attr, mind).unwrap_err()),
        "NounAttributeValueResolver: GearboxFramework.GearboxMind.Role is not a byte"
    );
    assert_eq!(m.heap(), before_mind);
}

/// The write signal (section 10.5): a read that creates an entry signals `EvalCx::note_writes`; a read of an
/// existing entry does not.
#[test]
fn a_creating_read_signals_its_writes() {
    let mut n = off();
    let p = providers(&mut n);
    let attr = n.noun_attr("IsPhaselocked", 2, 1.0);
    let ctx = n.provider(p.pawn, 3);
    let sys = system();
    let mut cx = EvalCx::new(&mut n.fx.w);
    sys.value_of(attr, Some(ctx), None, &mut cx).unwrap();
    assert!(cx.wrote(), "the creation is signalled");
    let mut cx = EvalCx::new(&mut n.fx.w);
    sys.value_of(attr, Some(ctx), None, &mut cx).unwrap();
    assert!(!cx.wrote(), "a plain read is not");
}

/// Review r1 of #523, item 6: the count is read again after `d`. At 19, a default that reads a second, missing
/// strategy-2 noun on the same pawn lets that nested noun take slot 19; the outer entry then finds 20 and gets -1.
#[test]
fn the_count_is_read_again_after_the_default() {
    let mut n = off();
    let p = providers(&mut n);
    let nested = n.noun_attr("Nested", 2, 4.0);
    let default = n.fx.data(InitData { base_value_attribute: Some(nested), ..InitData::constant(0.0) });
    let outer = n.attr("Outer", default);
    n.set_resolver(outer, "ReplicationStrategy", Value::Byte(2));
    let ctx = n.provider(p.pawn, 3);
    n.put(ctx, "NumReplicatedNouns", Value::Int(19));
    assert_eq!(n.value(outer, ctx), Ok(4.0));
    assert_eq!(n.get(ctx, "NumReplicatedNouns"), Value::Int(20));
    assert_eq!(n.slot(ctx, 19).0, "Nested");
    let got: Vec<(String, i32)> = n.entries(ctx).into_iter().map(|e| (e.name, e.slot)).collect();
    assert_eq!(got, vec![("Nested".to_string(), 19), ("Outer".to_string(), -1)]);
}

/// Review r1 of #523, item 1: the probe validates the default without evaluating it, so a default that reads a
/// second, missing strategy-2 noun on the same authority pawn creates nothing during the chain check.
#[test]
fn the_probe_does_not_evaluate_the_default() {
    let mut n = off();
    let p = providers(&mut n);
    let nested = n.noun_attr("Nested", 2, 4.0);
    let default = n.fx.data(InitData { base_value_attribute: Some(nested), ..InitData::constant(0.0) });
    let outer = n.attr("Outer", default);
    n.set_resolver(outer, "ReplicationStrategy", Value::Byte(2));
    let ctx = n.provider(p.pawn, 3);
    let before = n.heap();
    let sys = system();
    sys.check_modifier_chain(outer, ctx, ProbeOp::Add, &mut EvalCx::new(&mut n.fx.w)).unwrap();
    assert_eq!(n.heap(), before, "no count, slot, dirty bit or entry");
}

/// Review r1 of #523, item 2: every destination is checked before the first write. A replicated slot whose `Value`
/// is not a float fails the creation before the count, the slot or `bNetDirty` is written, and fails the mirror
/// before `NounState` is rewritten.
#[test]
fn a_malformed_slot_fails_before_any_write() {
    let mut n = off();
    let p = providers_with(&mut n, PropKind::Int);
    let attr = n.noun_attr("IsPhaselocked", 2, 1.0);
    let ctx = n.provider(p.pawn, 3);
    let before = n.heap();
    let e = n.value(attr, ctx).unwrap_err();
    assert!(matches!(e, AttrError::BadData { .. }), "{e:?}");
    assert_eq!(n.heap(), before, "creation wrote nothing");
    // The mirror: an existing entry in slot 0 (count 1); a set-base would rewrite NounState, then the slot.
    let entry = Entry { strategy: 2, slot: 0, ..Noun::created("IsPhaselocked", 1.0) };
    let noun = n.full_noun(&entry);
    n.put(ctx, "NounState", Value::array(vec![noun]));
    n.put(ctx, "NumReplicatedNouns", Value::Int(1));
    let before = n.heap();
    let e = n.set_base(attr, ctx, 2.0).unwrap_err();
    assert!(matches!(e, AttrError::BadData { .. }), "{e:?}");
    assert_eq!(n.heap(), before, "the mirror wrote nothing");
}

/// Review r1 of #523, item 4: the Off add, remove and set-base keep a write scope. A committed noun creation followed
/// by a `NotYet` from the chain's next resolver comes back prefixed, its kind kept.
#[test]
fn a_not_yet_after_a_committed_noun_write_is_prefixed() {
    let mut n = off();
    let _ = providers(&mut n);
    let ctx = n.provider(n.fx.by_path("GearboxFramework.GearboxMind"), 3);
    let first = n.noun_attr("First", 0, 1.0);
    let second = n.noun_attr("Second", 3, 1.0);
    // One attribute whose chain is the two resolvers: the first creates its entry, the second refuses (strategy 3).
    let r1 = n.resolver_of(first);
    let r2 = n.resolver_of(second);
    let ctx_resolver = n.fx.no_context();
    let both = n.fx.attr("Both", vec![ctx_resolver], vec![r1, r2]);
    let m = n.modifier(PRE, 1.0);
    for op in 0..2 {
        let e = if op == 0 { n.add(both, ctx, m).unwrap_err() } else { n.set_base(both, ctx, 2.0).unwrap_err() };
        match e {
            AttrError::NotYet(e) => {
                assert_eq!(e.missing, Missing::NotRead);
                assert!(e.what.starts_with("stopped after partial writes: "), "{}", e.what);
                assert_eq!(e.what.matches("stopped after partial writes").count(), 1);
            }
            other => panic!("{other:?}"),
        }
    }
    // Without a committed write first, the refusal is unprefixed.
    let mut fresh = off();
    let _ = providers(&mut fresh);
    let ctx = fresh.provider(fresh.fx.by_path("GearboxFramework.GearboxMind"), 3);
    let second = fresh.noun_attr("Second", 3, 1.0);
    let m = fresh.modifier(PRE, 1.0);
    match fresh.add(second, ctx, m).unwrap_err() {
        AttrError::NotYet(e) => assert!(!e.what.starts_with("stopped"), "{}", e.what),
        other => panic!("{other:?}"),
    }
}

/// Review r1 of #523, item 5: off authority, get-base answers 0.0 without reading a slot, so a malformed matching
/// slot does not fail it; get still reads the slot and reports the fault.
#[test]
fn get_base_off_authority_reads_no_slot() {
    let mut n = off();
    let p = providers(&mut n);
    let ctx = n.provider(p.pawn, 1);
    n.fill_slot(ctx, 0, p.rep, "IsPhaselocked", 4.0);
    n.put(ctx, "NumReplicatedNouns", Value::Int(1));
    // Plant a malformed Value in the matching slot, past the heap's kind check.
    let prop = n.fx.w.space.prop(p.pawn, "ReplicatedNouns").unwrap();
    let slot = n.fx.w.space.slot_mut(ctx, PropAddr::element(prop, 0)).unwrap();
    slot.struct_mut().unwrap().fields[1] = Value::Bool(true);
    let attr = n.noun_attr("IsPhaselocked", 2, 1.0);
    assert_eq!(n.base(attr, ctx), Ok(0.0));
    assert!(n.value(attr, ctx).is_err());
}

/// Review r2 of #523, item 1: the heap's own checks run before the first write. An existing `NounState` entry whose
/// `BaseValue` is malformed (planted past the heap's kind check) passes the lookup by name; the creation of a second
/// noun on the same pawn then fails before its count, slot or dirty bit is written, as `BadData`.
#[test]
fn a_malformed_existing_entry_fails_the_creation_before_any_write() {
    let mut n = off();
    let p = providers(&mut n);
    let ctx = n.provider(p.pawn, 3);
    let other = Entry { strategy: 0, ..Noun::created("Other", 1.0) };
    let noun = n.full_noun(&other);
    n.put(ctx, "NounState", Value::array(vec![noun]));
    let prop = n.fx.w.space.prop(p.pawn, "NounState").unwrap();
    let bad_name = Value::Name(n.fx.w.space.names.intern("NotAFloat"));
    let slot = n.fx.w.space.slot_mut(ctx, PropAddr::scalar(prop)).unwrap();
    let Value::Array(items) = slot else { panic!() };
    let items = std::rc::Rc::make_mut(items);
    items[0].struct_mut().unwrap().fields[2] = bad_name;
    let attr = n.noun_attr("IsPhaselocked", 2, 1.0);
    let before = n.heap();
    let e = n.value(attr, ctx).unwrap_err();
    assert!(matches!(e, AttrError::BadData { .. }), "{e:?}");
    assert_eq!(n.heap(), before, "no count, slot, dirty bit or entry written");
}

/// Review r2 of #523, item 1: a replicated slot holding a struct of another type, with a compatible `Value` field,
/// passes the plan's field check but not the heap's; the mirror fails before `NounState` changes, as `BadData`.
#[test]
fn a_wrong_struct_in_the_slot_fails_the_mirror_before_any_write() {
    let mut n = off();
    let p = providers(&mut n);
    let attr = n.noun_attr("IsPhaselocked", 2, 1.0);
    let ctx = n.provider(p.pawn, 3);
    let entry = Entry { strategy: 2, slot: 0, ..Noun::created("IsPhaselocked", 1.0) };
    let noun = n.full_noun(&entry);
    n.put(ctx, "NounState", Value::array(vec![noun.clone()]));
    n.put(ctx, "NumReplicatedNouns", Value::Int(1));
    let prop = n.fx.w.space.prop(p.pawn, "ReplicatedNouns").unwrap();
    *n.fx.w.space.slot_mut(ctx, PropAddr::element(prop, 0)).unwrap() = noun;
    let before = n.heap();
    let e = n.set_base(attr, ctx, 2.0).unwrap_err();
    assert!(matches!(e, AttrError::BadData { .. }), "{e:?}");
    assert_eq!(n.heap(), before, "NounState unchanged");
}

/// Review r2 of #523, item 3: a successful noun removal followed by the next resolver's refusal is prefixed.
#[test]
fn a_not_yet_after_a_noun_removal_is_prefixed() {
    let mut n = off();
    let _ = providers(&mut n);
    let ctx = n.provider(n.fx.by_path("GearboxFramework.GearboxMind"), 3);
    let first = n.noun_attr("First", 0, 1.0);
    let second = n.noun_attr("Second", 3, 1.0);
    let m = n.modifier(PRE, 1.0);
    assert_eq!(n.add(first, ctx, m), Ok(AddModifierOutcome::Added));
    let r1 = n.resolver_of(first);
    let r2 = n.resolver_of(second);
    let ctx_resolver = n.fx.no_context();
    let both = n.fx.attr("Both", vec![ctx_resolver], vec![r1, r2]);
    match n.remove(both, ctx, m).unwrap_err() {
        AttrError::NotYet(e) => assert!(e.what.starts_with("stopped after partial writes: "), "{}", e.what),
        other => panic!("{other:?}"),
    }
    assert!(n.entries(ctx)[0].stack.is_empty(), "the removal stays");
}

/// An invented package (rule 9): `Holder_0` points at `ColdThing`, an export of a class that declares `NounState`, so
/// loading the holder leaves `ColdThing` a store handle not yet loaded. Returns that cold context handle.
fn cold_provider(n: &mut Noun) -> ObjRef {
    use ow_package::testkit::script::ScriptToy;
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "IntProperty"]);
    let object = t.class("Object", 0);
    let provider = t.class("ColdProvider", object);
    t.prop("Int", provider, "NounState", 0, 0);
    let holder = t.class("Holder", object);
    t.prop("Object", holder, "Target", 0, object);
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend_from_slice(&t.none);
    let thing = t.p.add_export(provider, "ColdThing", payload);
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(t.p.object_tag("Target", thing));
    payload.extend_from_slice(&t.none);
    t.p.add_export(holder, "Holder_0", payload);
    let bytes = t.build();
    let dir = std::env::temp_dir().join(format!("ow-attr-noun-cold-{}-{}", std::process::id(), n.fx.w.space.heap.len()));
    let _ = std::fs::remove_dir_all(&dir);
    let defs = DefStore::from_synthetic(&dir, &[("ColdPkg.upk", bytes.as_slice())], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    n.fx.w.defs = Some(defs);
    let path = ow_core::ObjectPath::parse("ColdPkg.Holder_0").unwrap();
    let h = n.fx.w.defs.as_mut().unwrap().find(&mut n.fx.w.space, &path).unwrap().unwrap();
    let class = n.fx.w.space.heap.get(h).unwrap().class;
    let p = n.fx.w.space.prop(class, "Target").unwrap();
    let got = n.fx.w.space.read(h, PropAddr::scalar(p)).unwrap().clone();
    let Value::Object(Some(c)) = got else { panic!("Target = {got:?}") };
    assert!(n.fx.w.defs.as_ref().unwrap().is_unloaded(&n.fx.w.space, c), "the context is cold");
    c
}

/// Review r2 of #523, items 2 and 3, store-backed: the load of a cold context is the operation's only write. An add,
/// a remove and a set-base through a strategy-3 noun on it refuse prefixed. The chain check alone follows #518's rule
/// (kept when #523 was rebased onto it): it may load, it does not prefix, and its caller tracks the load with
/// `DefStore::load_transitions`.
#[test]
fn a_load_that_is_the_only_write_prefixes_the_refusal() {
    for op in 0..3 {
        let mut n = off();
        let ctx = cold_provider(&mut n);
        let attr = n.noun_attr("Strategy3", 3, 1.0);
        let m = n.modifier(PRE, 1.0);
        let e = match op {
            0 => n.add(attr, ctx, m).map(|_| ()).unwrap_err(),
            1 => n.remove(attr, ctx, m).map(|_| ()).unwrap_err(),
            _ => n.set_base(attr, ctx, 1.0).map(|_| ()).unwrap_err(),
        };
        match e {
            AttrError::NotYet(e) => {
                assert_eq!(e.missing, Missing::NotRead);
                assert!(e.what.starts_with("stopped after partial writes: "), "op {op}: {}", e.what);
            }
            other => panic!("op {op}: {other:?}"),
        }
    }
    // The chain check: the same refusal unprefixed, with the load visible to its caller through #518's counter.
    let mut n = off();
    let ctx = cold_provider(&mut n);
    let attr = n.noun_attr("Strategy3", 3, 1.0);
    let loads = n.fx.w.defs.as_ref().unwrap().load_transitions();
    let sys = system();
    match sys.check_modifier_chain(attr, ctx, ProbeOp::Add, &mut EvalCx::new(&mut n.fx.w)).unwrap_err() {
        AttrError::NotYet(e) => {
            assert_eq!(e.missing, Missing::NotRead);
            assert!(!e.what.starts_with("stopped"), "{}", e.what);
        }
        other => panic!("{other:?}"),
    }
    assert!(n.fx.w.defs.as_ref().unwrap().load_transitions() > loads, "the probe loaded the context");
    // The control: with the context already loaded, the same refusal is unprefixed.
    let mut n = off();
    let ctx = cold_provider(&mut n);
    n.fx.w.defs.as_mut().unwrap().ensure_loaded(&mut n.fx.w.space, ctx).unwrap();
    let attr = n.noun_attr("Strategy3", 3, 1.0);
    let m = n.modifier(PRE, 1.0);
    match n.add(attr, ctx, m).unwrap_err() {
        AttrError::NotYet(e) => assert!(!e.what.starts_with("stopped"), "{}", e.what),
        other => panic!("{other:?}"),
    }
}

/// Review r2 of #523, item 4: on an unread class, a strategy-0 entry with a replicated slot: the removal of an absent
/// modifier passes the probe and answers NotPresent in execution; the removal of the held one is refused by both.
#[test]
fn the_removal_probe_and_execution_agree() {
    let mut n = off();
    let default = n.fx.data(InitData::constant(3.0));
    let attr = n.attr("Attribute_HealthMultiplier", default);
    let held = n.modifier(2, 1.0);
    let absent = n.modifier(2, 1.0);
    let e = Entry { slot: 2, stack: vec![Some(held)], ..Noun::created("Attribute_HealthMultiplier", 1.0) };
    let noun = n.full_noun(&e);
    let mind = n.mind(None, vec![noun]);
    let sys = system();
    sys.check_modifier_chain(attr, mind, ProbeOp::Remove(absent), &mut EvalCx::new(&mut n.fx.w)).unwrap();
    assert_eq!(n.remove(attr, mind, absent), Ok(RemoveModifierOutcome::NotPresent));
    let probe = sys.check_modifier_chain(attr, mind, ProbeOp::Remove(held), &mut EvalCx::new(&mut n.fx.w));
    assert!(matches!(probe, Err(AttrError::NotYet(_))), "{probe:?}");
    assert!(matches!(n.remove(attr, mind, held), Err(AttrError::NotYet(_))));
    assert_eq!(n.entries(mind), vec![e], "unchanged");
}
