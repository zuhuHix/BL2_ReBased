//! NATIVE_LINKS_3_4_TAIL.md section 11, test 1, on invented classes (AGENTS.md rule 9: the class and field names are
//! the packages'; every value is invented). AI-assisted (Claude Code).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

struct Fx {
    vm: Vm,
    reaction: StructId,
    pawn: ObjRef,
    helper: ObjRef,
    region: ObjRef,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn fx() -> Fx {
    let space = ObjectSpace::new();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let object = vm.space.classes.define("Core.Object", None, vec![]).unwrap();
    let o = PropKind::Object { class: object };
    let objects = PropKind::Array(Box::new(o.clone()));
    let floats = PropKind::Array(Box::new(PropKind::Float));
    let props = vec![decl(&mut vm, "GeneratedEvents", objects.clone())];
    let actor = vm.space.classes.define("Engine.Actor", Some(object), props).unwrap();
    let pawn = vm.space.classes.define("Engine.Pawn", Some(actor), vec![]).unwrap();
    let props = vec![decl(&mut vm, "MyHitRegionHelper", o.clone()), decl(&mut vm, "HitRegionHealthScaleAttribute", PropKind::Float)];
    vm.space.classes.define("WillowGame.WillowPawn", Some(pawn), props).unwrap();
    vm.space.classes.define("WillowGame.SeqEvent_TakeHitRegionDamage", Some(object), vec![]).unwrap();
    let props = vec![decl(&mut vm, "BehaviorSequences", objects.clone())];
    vm.space.classes.define("GearboxFramework.BehaviorProviderDefinition", Some(object), props).unwrap();
    let props = vec![
        decl(&mut vm, "BodyHitRegion", o.clone()),
        decl(&mut vm, "Damage", PropKind::Float),
        decl(&mut vm, "RecentDamage", PropKind::Float),
        decl(&mut vm, "bHasGored", PropKind::Bool),
        decl(&mut vm, "LastPctHealthForRunDamageTriggeredBehaviors", floats.clone()),
        decl(&mut vm, "LastPctHealthForRunHealingTriggeredBehaviors", floats),
    ];
    let record = vm.space.classes.define_struct("WillowGame.HitRegionHelper.HitRegionDamage", None, props).unwrap();
    let props = vec![decl(&mut vm, "HitRegionDamageList", PropKind::Array(Box::new(PropKind::Struct(record))))];
    vm.space.classes.define("WillowGame.HitRegionHelper", Some(object), props).unwrap();
    let props = vec![
        decl(&mut vm, "bOnlyTriggeredOnOwnerDeath", PropKind::Bool),
        decl(&mut vm, "PercentOfHealthTrigger", PropKind::Float),
        decl(&mut vm, "bDamageThreshold", PropKind::Bool),
        decl(&mut vm, "bRepeatableEvent", PropKind::Bool),
        decl(&mut vm, "DamageTriggers", PropKind::Array(Box::new(PropKind::Int))),
    ];
    let reaction = vm.space.classes.define_struct("WillowGame.BodyHitRegionDefinition.DamageReactionData", None, props).unwrap();
    let reactions = PropKind::Array(Box::new(PropKind::Struct(reaction)));
    let mut props = Vec::new();
    for f in ["bTrackDamage", "bScaleHealthWithAttribute", "bUseIndependentHealth", "bShowDamageNumbers"] {
        props.push(decl(&mut vm, f, PropKind::Bool));
    }
    for f in ["OwnerHealthPercent", "PercentOfMaxHealthLimit"] {
        props.push(decl(&mut vm, f, PropKind::Float));
    }
    props.push(decl(&mut vm, "AllowedDamageTypes", objects.clone()));
    props.push(decl(&mut vm, "DamageTriggeredReactions", reactions.clone()));
    props.push(decl(&mut vm, "HealingTriggeredReactions", reactions));
    props.push(decl(&mut vm, "RegionDeathBehaviors", objects));
    props.push(decl(&mut vm, "BehaviorProviderDefinition", o));
    vm.space.classes.define("WillowGame.BodyHitRegionDefinition", Some(object), props).unwrap();
    let mut make = |path: &str| {
        let c = vm.space.classes.by_path(path).unwrap();
        vm.space.alloc(c, "Toy", None, Origin::Runtime).unwrap()
    };
    let (pawn, helper, region) =
        (make("WillowGame.WillowPawn"), make("WillowGame.HitRegionHelper"), make("WillowGame.BodyHitRegionDefinition"));
    let mut x = Fx { vm, reaction, pawn, helper, region };
    x.set(x.pawn, "MyHitRegionHelper", Value::Object(Some(x.helper)));
    x.set(x.region, "bTrackDamage", Value::Bool(true));
    x
}

impl Fx {
    fn class(&self, path: &str) -> ClassId {
        self.vm.space.classes.by_path(path).unwrap()
    }
    fn make(&mut self, path: &str) -> ObjRef {
        let c = self.class(path);
        self.vm.space.alloc(c, "Toy", None, Origin::Runtime).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }
    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }
    fn hit(&mut self, d: f32, max: f32) -> Result<f32, VmError> {
        let (pawn, region) = (self.pawn, self.region);
        cap(&mut self.vm, pawn, region, None, None, d, max, max)
    }
    fn hit_by(&mut self, d: f32, max: f32, instigator: Option<ObjRef>, t: Option<ObjRef>) -> Result<f32, VmError> {
        let (pawn, region) = (self.pawn, self.region);
        cap(&mut self.vm, pawn, region, instigator, t, d, max, max)
    }
    /// The record's (Damage, RecentDamage, bHasGored), None with no record.
    fn record(&self) -> Option<(f32, f32, bool)> {
        let Value::Array(list) = self.get(self.helper, "HitRegionDamageList") else { panic!() };
        let e = list.first()?;
        let f = |n: &str| member(&self.vm, e, n, "t").unwrap().0;
        let num = |n: &str| if let Value::Float(v) = f(n) { v } else { panic!() };
        Some((num("Damage"), num("RecentDamage"), f("bHasGored") == Value::Bool(true)))
    }
    fn writes(&self) -> u64 {
        self.vm.space.heap.committed_writes()
    }
}

fn not_read(r: Result<f32, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            n.what.to_string()
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_clear_track_flag_or_no_helper_is_nothing() {
    let mut x = fx();
    x.set(x.region, "bTrackDamage", Value::Bool(false));
    let before = x.writes();
    assert_eq!(x.hit(30.0, 100.0).unwrap(), 30.0);
    assert_eq!(x.writes(), before);
    x.set(x.region, "bTrackDamage", Value::Bool(true));
    x.set(x.pawn, "MyHitRegionHelper", Value::Object(None));
    let before = x.writes();
    assert_eq!(x.hit(30.0, 100.0).unwrap(), 30.0);
    assert_eq!(x.writes(), before);
}

#[test]
fn an_unlimited_region_keeps_the_damage_and_sums_the_record() {
    let mut x = fx();
    assert_eq!(x.hit(30.0, 100.0).unwrap(), 30.0);
    assert_eq!(x.record(), Some((30.0, 30.0, false)));
    assert_eq!(x.hit(20.0, 100.0).unwrap(), 20.0);
    assert_eq!(x.record(), Some((50.0, 50.0, false)), "the same record, summed");
}

#[test]
fn a_limit_of_a_half_caps_at_the_share_then_takes_nothing() {
    let mut x = fx();
    x.set(x.region, "PercentOfMaxHealthLimit", Value::Float(0.5));
    assert_eq!(x.hit(60.0, 100.0).unwrap(), 50.0, "M = 50: the hit takes what is left");
    let (damage, _, gored) = x.record().unwrap();
    assert_eq!((damage, gored), (50.0, true), "spent: the region's death sets Damage to M");
    assert_eq!(x.hit(10.0, 100.0).unwrap(), 0.0, "the next hit: nothing left");
}

#[test]
fn an_independent_region_takes_the_damage_away_from_the_owner() {
    let mut x = fx();
    x.set(x.region, "bUseIndependentHealth", Value::Bool(true));
    x.set(x.region, "OwnerHealthPercent", Value::Float(0.25));
    assert_eq!(x.hit(10.0, 100.0).unwrap(), 0.0);
    assert_eq!(x.record(), Some((10.0, 10.0, false)), "the bookkeeping first");
    // With an instigator and damage numbers: the notification's NotRead, the record unchanged.
    x.set(x.region, "bShowDamageNumbers", Value::Bool(true));
    let instigator = x.make("Engine.Actor");
    let before = x.writes();
    assert_eq!(not_read(x.hit_by(5.0, 100.0, Some(instigator), None)), DAMAGE_NUMBERS);
    assert_eq!(x.writes(), before);
    assert_eq!(x.record(), Some((10.0, 10.0, false)));
}

#[test]
fn a_damage_type_the_region_does_not_allow_does_nothing() {
    let mut x = fx();
    let allowed = x.make("Core.Object");
    let other = x.make("Core.Object");
    x.set(x.region, "AllowedDamageTypes", Value::array(vec![Value::Object(Some(allowed))]));
    assert_eq!(x.hit_by(30.0, 100.0, None, Some(other)).unwrap(), 0.0);
    assert_eq!(x.hit_by(30.0, 100.0, None, Some(allowed)).unwrap(), 30.0);
}

#[test]
fn a_lethal_hit_gores_and_behaviours_refuse_before_any_write() {
    let mut x = fx();
    x.set(x.region, "PercentOfMaxHealthLimit", Value::Float(0.5));
    // Region death behaviours: refused, nothing written.
    let behaviour = x.make("Core.Object");
    x.set(x.region, "RegionDeathBehaviors", Value::array(vec![Value::Object(Some(behaviour))]));
    let before = x.writes();
    assert!(not_read(x.hit(60.0, 100.0)).starts_with(BEHAVIORS));
    assert_eq!((x.writes(), x.record()), (before, None));
    // A provider holding sequences: refused.
    x.set(x.region, "RegionDeathBehaviors", Value::array(vec![]));
    let provider = x.make("GearboxFramework.BehaviorProviderDefinition");
    let sequence = x.make("Core.Object");
    x.set(provider, "BehaviorSequences", Value::array(vec![Value::Object(Some(sequence))]));
    x.set(x.region, "BehaviorProviderDefinition", Value::Object(Some(provider)));
    let before = x.writes();
    assert!(not_read(x.hit(60.0, 100.0)).starts_with(BEHAVIORS));
    assert_eq!(x.writes(), before);
    // An unloaded provider: NotRead naming it.
    let class = x.class("GearboxFramework.BehaviorProviderDefinition");
    let lazy = x.vm.space.alloc_stand_in(class, "Lazy", Origin::Default { class }).unwrap();
    x.set(x.region, "BehaviorProviderDefinition", Value::Object(Some(lazy)));
    assert!(matches!(x.hit(60.0, 100.0), Err(VmError::NotYet(_))));
    // An empty provider: the death goes through.
    x.set(provider, "BehaviorSequences", Value::array(vec![]));
    x.set(x.region, "BehaviorProviderDefinition", Value::Object(Some(provider)));
    assert_eq!(x.hit(60.0, 100.0).unwrap(), 50.0);
    assert_eq!(x.record().map(|r| r.2), Some(true));
}

#[test]
fn a_reaction_that_would_fire_refuses_and_one_that_would_not_passes() {
    let mut x = fx();
    let entry = |x: &Fx, trigger: f32| {
        let mut v = x.vm.space.classes.default_value(&PropKind::Struct(x.reaction)).unwrap();
        let s = v.struct_mut().unwrap();
        let layout = x.vm.space.classes.struct_layout(s.ty).unwrap().clone();
        let (i, _) = layout.field(x.vm.space.names.find("PercentOfHealthTrigger").unwrap()).unwrap();
        s.fields[i as usize] = Value::Float(trigger);
        v
    };
    x.set(x.region, "DamageTriggeredReactions", Value::array(vec![entry(&x, 0.9)]));
    assert_eq!(x.hit(10.0, 100.0).unwrap(), 10.0, "10% does not reach a 90% trigger");
    x.set(x.region, "DamageTriggeredReactions", Value::array(vec![entry(&x, 0.1)]));
    let before = x.writes();
    assert!(not_read(x.hit(10.0, 100.0)).starts_with(BEHAVIORS));
    assert_eq!(x.writes(), before);
}

#[test]
fn the_kismet_event_and_a_non_willow_victim_are_not_read() {
    let mut x = fx();
    let event = x.make("WillowGame.SeqEvent_TakeHitRegionDamage");
    x.set(x.pawn, "GeneratedEvents", Value::array(vec![Value::Object(Some(event))]));
    let before = x.writes();
    assert_eq!(not_read(x.hit(10.0, 100.0)), KISMET);
    assert_eq!(x.writes(), before);
    let other = x.make("Engine.Pawn");
    let region = x.region;
    assert!(not_read(cap(&mut x.vm, other, region, None, None, 10.0, 100.0, 100.0)).starts_with(CONSUMER));
}

#[test]
fn a_mistyped_field_is_a_data_error() {
    let mut x = fx();
    // A helper whose list is not of structs.
    let object = x.class("Core.Object");
    let props = vec![decl(&mut x.vm, "HitRegionDamageList", PropKind::Array(Box::new(PropKind::Int)))];
    x.vm.space.classes.define("Test.BadHelper", Some(object), props).unwrap();
    let helper = x.make("Test.BadHelper");
    x.set(x.pawn, "MyHitRegionHelper", Value::Object(Some(helper)));
    assert!(matches!(x.hit(10.0, 100.0), Err(VmError::Script { .. })));
}
