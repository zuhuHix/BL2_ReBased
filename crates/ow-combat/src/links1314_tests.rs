//! NATIVE_LINKS_13_14.md section 8, tests 3 to 9, on invented classes (AGENTS.md rule 9: the class and field names
//! are the packages'; every value is invented). Each read is checked write-free. AI-assisted (Claude Code).

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
    summary: StructId,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn fx() -> Fx {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let o = PropKind::Object { class: object };
    let actor = vm.space.classes.define("Engine.Actor", Some(object), vec![]).unwrap();
    let props = vec![
        decl(&mut vm, "Controller", o.clone()),
        decl(&mut vm, "DrivenVehicle", o.clone()),
        decl(&mut vm, "bCanBeInjured", PropKind::Bool),
    ];
    let pawn = vm.space.classes.define("Engine.Pawn", Some(actor), props).unwrap();
    vm.space.classes.define("Engine.Vehicle", Some(pawn), vec![]).unwrap();
    let props = vec![decl(&mut vm, "PlayerMaster", o.clone()), decl(&mut vm, "ThoughtlockMaster", o.clone())];
    vm.space.classes.define("WillowGame.WillowAIPawn", Some(pawn), props).unwrap();
    let mut props = vec![
        decl(&mut vm, "Pawn", o.clone()),
        decl(&mut vm, "bGodMode", PropKind::Bool),
        decl(&mut vm, "bNonCheatGodMode", PropKind::Bool),
    ];
    for (_, kind, _) in THEFT_SOURCES {
        for prefix in ["Instigated", "Received"] {
            props.push(decl(&mut vm, &format!("{prefix}{kind}DamageAmmoTheftAmount"), PropKind::IntAttribute));
        }
    }
    let controller = vm.space.classes.define("Engine.Controller", Some(actor), props).unwrap();
    let props = vec![decl(&mut vm, "PlayerStats", o.clone())];
    vm.space.classes.define("WillowGame.WillowPlayerController", Some(controller), props).unwrap();
    let props = vec![decl(&mut vm, "bCountsTowardDamageAndKillStats", PropKind::Bool)];
    let character = vm.space.classes.define("WillowGame.WillowCharacterClassDefinition", Some(object), props).unwrap();
    vm.space.classes.define("WillowGame.AIClassDefinition", Some(character), vec![]).unwrap();
    for (path, _, _) in THEFT_SOURCES {
        let c = vm.space.classes.define(path, Some(object), vec![]).unwrap();
        if path.ends_with("Bullet") {
            vm.space.classes.define("Test.ToyBulletSource", Some(c), vec![]).unwrap();
        }
        if path.ends_with("Grenade") {
            vm.space.classes.define("Test.ToyGrenadeSource", Some(c), vec![]).unwrap();
        }
    }
    let mut fields = vec![decl(&mut vm, "FinalDamage", PropKind::Float)];
    for list in crate::damage::TAG_LISTS {
        fields.push(decl(&mut vm, list, PropKind::Byte { enum_: None }).with_array_dim(crate::damage::TAG_LIST_LEN));
    }
    let summary = vm.space.classes.define_struct("WillowGame.WillowDamagePipeline.DamageEventSummary", None, fields).unwrap();
    let props = vec![decl(&mut vm, "DamageSummary", PropKind::Struct(summary))];
    vm.space.classes.define("WillowGame.WillowDamagePipeline", Some(object), props).unwrap();
    // A pipeline class whose lists are four bytes: a mistyped summary.
    let mut fields = vec![decl(&mut vm, "FinalDamage", PropKind::Float)];
    for list in crate::damage::TAG_LISTS {
        fields.push(decl(&mut vm, list, PropKind::Byte { enum_: None }).with_array_dim(4));
    }
    let short = vm.space.classes.define_struct("Test.ShortPipeline.DamageEventSummary", None, fields).unwrap();
    let props = vec![decl(&mut vm, "DamageSummary", PropKind::Struct(short))];
    vm.space.classes.define("Test.ShortPipeline", Some(object), props).unwrap();
    Fx { vm, summary }
}

impl Fx {
    fn class(&self, path: &str) -> ClassId {
        self.vm.space.classes.by_path(path).unwrap()
    }

    fn make(&mut self, path: &str) -> ObjRef {
        let c = self.class(path);
        self.vm.space.alloc(c, "Toy", None, Origin::Runtime).unwrap()
    }

    fn stand_in(&mut self, path: &str) -> ObjRef {
        let c = self.class(path);
        self.vm.space.alloc_stand_in(c, "Lazy", Origin::Default { class: c }).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }

    /// A pipeline whose summary has `final_damage` and the taken list `taken` (the dealt list all 0).
    fn pipeline(&mut self, final_damage: f32, taken: [u8; 8]) -> ObjRef {
        let p = self.make("WillowGame.WillowDamagePipeline");
        let mut v = self.vm.space.classes.zero_value(&PropKind::Struct(self.summary));
        let s = v.struct_mut().unwrap();
        s.fields[0] = Value::Float(final_damage);
        for (k, b) in taken.iter().enumerate() {
            s.fields[1 + k] = Value::Byte(*b);
        }
        self.set(p, "DamageSummary", v);
        p
    }

    fn summary(&self, p: ObjRef) -> Vec<Value> {
        let class = self.vm.space.heap.get(p).unwrap().class;
        let prop = self.vm.space.prop(class, "DamageSummary").unwrap();
        self.vm.space.read(p, PropAddr::scalar(prop)).unwrap().as_struct().unwrap().fields.clone()
    }
}

fn not_read(e: VmError) -> String {
    match e {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            n.what.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// Test 3 (section 5): the reset zeroes the summary and sets both lists to 255 in every byte; a summary whose lists
/// are not eight bytes is a script error, nothing written.
#[test]
fn adjust_damage_s_reset_sets_both_lists_to_255() {
    let mut x = fx();
    let p = x.pipeline(13.95, [3, 9, 0, 0, 0, 0, 0, 0]);
    crate::damage::reset_summary(&mut x.vm, p).unwrap();
    let fields = x.summary(p);
    assert_eq!(fields[0], Value::Float(0.0), "the summary zeroed");
    assert!(fields[1..].iter().all(|v| *v == Value::Byte(255)), "{fields:?}");
    let short = x.make("Test.ShortPipeline");
    let before = x.marks();
    assert!(matches!(crate::damage::reset_summary(&mut x.vm, short), Err(VmError::Script { .. })));
    assert_eq!(x.marks(), before);
}

/// Test 4: `GetController`.
#[test]
fn get_controller_s_cases() {
    let mut x = fx();
    let pawn = x.make("Engine.Pawn");
    let own = x.make("Engine.Controller");
    x.set(pawn, "Controller", Value::Object(Some(own)));
    assert_eq!(get_controller(&x.vm, pawn).unwrap(), Some(own));
    x.set(pawn, "Controller", Value::Object(None));
    assert_eq!(get_controller(&x.vm, pawn).unwrap(), None, "none at all");
    let car = x.make("Engine.Vehicle");
    let driver = x.make("Engine.Controller");
    x.set(car, "Controller", Value::Object(Some(driver)));
    x.set(pawn, "DrivenVehicle", Value::Object(Some(car)));
    let before2 = x.marks();
    assert_eq!(get_controller(&x.vm, pawn).unwrap(), Some(driver), "the vehicle's controller");
    assert_eq!(x.marks(), before2, "write-free");
    let lazy = x.stand_in("Engine.Vehicle");
    x.set(pawn, "DrivenVehicle", Value::Object(Some(lazy)));
    assert!(not_read(get_controller(&x.vm, pawn).unwrap_err()).ends_with(NOT_LOADED));
}

/// Test 5: `UpdateTookDamageFromEnemyStats`' credited player.
#[test]
fn update_took_damage_from_enemy_stats_s_cases() {
    let mut x = fx();
    let ai_class = x.make("WillowGame.AIClassDefinition");
    let mind = x.make("Engine.Controller");
    let ai_pawn = x.make("WillowGame.WillowAIPawn");
    x.set(mind, "Pawn", Value::Object(Some(ai_pawn)));
    // A non-counting AI class: nothing, whoever the instigator is.
    let player = x.make("WillowGame.WillowPlayerController");
    let stats = x.make("Core.Object");
    x.set(player, "PlayerStats", Value::Object(Some(stats)));
    assert_eq!(credited_player(&x.vm, Some(player), Some(ai_class)).unwrap(), None);
    x.set(ai_class, "bCountsTowardDamageAndKillStats", Value::Bool(true));
    // An AI instigator with no master: nothing.
    assert_eq!(credited_player(&x.vm, Some(mind), Some(ai_class)).unwrap(), None);
    // A PlayerMaster with no PlayerStats: nothing.
    let master = x.make("WillowGame.WillowPlayerController");
    x.set(ai_pawn, "PlayerMaster", Value::Object(Some(master)));
    assert_eq!(credited_player(&x.vm, Some(mind), Some(ai_class)).unwrap(), None);
    // A ThoughtlockMaster when there is no PlayerMaster.
    x.set(ai_pawn, "PlayerMaster", Value::Object(None));
    x.set(ai_pawn, "ThoughtlockMaster", Value::Object(Some(player)));
    assert_eq!(credited_player(&x.vm, Some(mind), Some(ai_class)).unwrap(), Some(player));
    // A player instigator with PlayerStats: the player (the native's NotRead), and no instigator: nothing.
    assert_eq!(credited_player(&x.vm, Some(player), None).unwrap(), Some(player));
    let before = x.marks();
    assert_eq!(credited_player(&x.vm, None, None).unwrap(), None);
    assert_eq!(credited_player(&x.vm, Some(mind), Some(ai_class)).unwrap(), Some(player));
    assert_eq!(x.marks(), before, "write-free");
}

/// Test 6: `GetAmmoTheftAmounts` for each kind, a Bullet subclass (matched), a Grenade subclass (not), no source, and
/// no victim controller.
#[test]
fn get_ammo_theft_amounts_s_cases() {
    let mut x = fx();
    let (a, b) = (x.make("Engine.Controller"), x.make("Engine.Controller"));
    for (i, (_, kind, _)) in THEFT_SOURCES.iter().enumerate() {
        x.set(a, &format!("Instigated{kind}DamageAmmoTheftAmount"), Value::Int(10 + i as i32));
        x.set(b, &format!("Received{kind}DamageAmmoTheftAmount"), Value::Int(20 + i as i32));
    }
    let before = x.marks();
    for (i, (path, _, _)) in THEFT_SOURCES.iter().enumerate() {
        let source = Some(x.class(path));
        assert_eq!(ammo_theft(&x.vm, source, Some(a), Some(b)).unwrap(), (10 + i as i32, 20 + i as i32), "{path}");
    }
    let bullet_sub = Some(x.class("Test.ToyBulletSource"));
    assert_eq!(ammo_theft(&x.vm, bullet_sub, Some(a), Some(b)).unwrap(), (10, 20), "Bullet with its subclasses");
    let grenade_sub = Some(x.class("Test.ToyGrenadeSource"));
    assert_eq!(ammo_theft(&x.vm, grenade_sub, Some(a), Some(b)).unwrap(), (0, 0), "Grenade exactly");
    assert_eq!(ammo_theft(&x.vm, None, Some(a), Some(b)).unwrap(), (0, 0), "no source");
    let melee = Some(x.class("WillowGame.WillowDmgSource_Melee"));
    assert_eq!(ammo_theft(&x.vm, melee, Some(a), None).unwrap(), (14, 0), "no victim controller");
    assert_eq!(x.marks(), before, "write-free");
}

/// Test 7: the iterators' tags.
#[test]
fn the_skill_event_iterators_stop_at_255_and_seven() {
    let mut x = fx();
    let p = x.pipeline(0.0, [3, 9, 255, 1, 1, 1, 1, 1]);
    assert_eq!(skill_event_tags(&x.vm, p, crate::damage::TAG_LISTS[0]).unwrap(), vec![3, 9]);
    let p = x.pipeline(0.0, [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(skill_event_tags(&x.vm, p, crate::damage::TAG_LISTS[0]).unwrap(), vec![1, 2, 3, 4, 5, 6, 7]);
    let p = x.pipeline(0.0, [255; 8]);
    assert!(skill_event_tags(&x.vm, p, crate::damage::TAG_LISTS[0]).unwrap().is_empty());
    // The dealt list after a reset: nothing.
    crate::damage::reset_summary(&mut x.vm, p).unwrap();
    assert!(skill_event_tags(&x.vm, p, crate::damage::TAG_LISTS[1]).unwrap().is_empty());
    let short = x.make("Test.ShortPipeline");
    assert!(matches!(skill_event_tags(&x.vm, short, crate::damage::TAG_LISTS[0]), Err(VmError::Script { .. })));
}

/// Test 8: `PawnCheckPlayHitPreconditions`.
#[test]
fn play_hit_preconditions_s_cases() {
    let mut x = fx();
    let pawn = x.make("Engine.Pawn");
    let controller = x.make("Engine.Controller");
    x.set(pawn, "Controller", Value::Object(Some(controller)));
    let hit = x.pipeline(5.0, [255; 8]);
    let miss = x.pipeline(0.0, [255; 8]);
    assert!(!play_hit_preconditions(&x.vm, pawn, Some(hit)).unwrap(), "FinalDamage 5: false");
    assert!(play_hit_preconditions(&x.vm, pawn, Some(miss)).unwrap(), "0, no god mode: true");
    assert!(play_hit_preconditions(&x.vm, pawn, None).unwrap(), "no pipeline: the god-mode test only");
    x.set(controller, "bGodMode", Value::Bool(true));
    let mid2 = x.marks();
    assert!(!play_hit_preconditions(&x.vm, pawn, Some(miss)).unwrap(), "0 with bGodMode: false");
    assert!(!play_hit_preconditions(&x.vm, pawn, None).unwrap());
    assert_eq!(x.marks(), mid2, "write-free");
}
