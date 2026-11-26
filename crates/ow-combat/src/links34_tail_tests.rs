//! NATIVE_LINKS_3_4_TAIL.md section 11, tests 2 to 5, and section 4.2's plain reads, on invented classes (AGENTS.md
//! rule 9: the class and field names are the packages'; every value is invented). AI-assisted (Claude Code).

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
    play_through: StructId,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn fx() -> Fx {
    let mut vm = Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let object = vm.space.classes.define("Core.Object", None, vec![]).unwrap();
    let o = PropKind::Object { class: object };
    let props = vec![decl(&mut vm, "GRI", o.clone()), decl(&mut vm, "WorldInfo", o.clone())];
    let actor = vm.space.classes.define("Engine.Actor", Some(object), props).unwrap();
    vm.space.classes.define("Engine.WorldInfo", Some(actor), vec![]).unwrap();
    let props = vec![decl(&mut vm, "CurrentPlaythrough", PropKind::Int)];
    vm.space.classes.define("WillowGame.WillowGameReplicationInfo", Some(actor), props).unwrap();
    vm.space.classes.define("Engine.GameReplicationInfo", Some(actor), vec![]).unwrap();
    // The pipeline's reads.
    let props = vec![
        decl(&mut vm, "BarrelSourceTime", PropKind::Float),
        decl(&mut vm, "PlantSourceTime", PropKind::Float),
        decl(&mut vm, "bCanHitFriendly", PropKind::Bool),
    ];
    vm.space.classes.define("Engine.DamagePipeline", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "BarrelSourceTime", PropKind::Int)];
    vm.space.classes.define("Test.BadPipeline", Some(object), props).unwrap();
    // The balance classes.
    let props = vec![decl(&mut vm, "DisplayName", PropKind::Str)];
    let play_through = vm.space.classes.define_struct("WillowGame.AIPawnBalanceDefinition.PlayThroughData", None, props).unwrap();
    let base = vm.space.classes.define("Engine.BaseBalanceDefinition", Some(object), vec![]).unwrap();
    let props = vec![decl(&mut vm, "PlayThroughs", PropKind::Array(Box::new(PropKind::Struct(play_through))))];
    vm.space.classes.define("WillowGame.AIPawnBalanceDefinition", Some(base), props).unwrap();
    vm.space.classes.define("Test.OtherBalance", Some(base), vec![]).unwrap();
    // The controller, the pawn, the globals and the dialog.
    let props = vec![
        decl(&mut vm, "PlayerStats", o.clone()),
        decl(&mut vm, "PlayerClass", o.clone()),
        decl(&mut vm, "PlayerSkillTree", o.clone()),
        decl(&mut vm, "RecentKillHistory", PropKind::Array(Box::new(PropKind::Int))),
        decl(&mut vm, "CurrKillingSpree", PropKind::Int),
    ];
    vm.space.classes.define("WillowGame.WillowPlayerController", Some(actor), props).unwrap();
    let props = vec![decl(&mut vm, "MyActionSkill", o.clone())];
    vm.space.classes.define("WillowGame.WillowPawn", Some(actor), props).unwrap();
    vm.space.classes.define("WillowGame.WillowDialogGlobalsDefinition", Some(object), vec![]).unwrap();
    let props = vec![decl(&mut vm, "Dialog", o.clone())];
    vm.space.classes.define("WillowGame.GlobalsDefinition", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "Dialog", PropKind::Int)];
    vm.space.classes.define("Test.BadDefinition", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "TheGlobalsDefinition", o)];
    vm.space.classes.define("WillowGame.WillowGlobals", Some(object), props).unwrap();
    Fx { vm, play_through }
}

impl Fx {
    fn class(&self, path: &str) -> ClassId {
        self.vm.space.classes.by_path(path).unwrap()
    }
    fn make(&mut self, path: &str) -> ObjRef {
        let c = self.class(path);
        self.vm.space.alloc(c, "Toy", None, Origin::Runtime).unwrap()
    }
    fn lazy(&mut self, path: &str) -> ObjRef {
        let c = self.class(path);
        self.vm.space.alloc_stand_in(c, "Lazy", Origin::Default { class: c }).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }
    /// A world: a world info whose GRI is a Willow replication info at `playthrough` (None: no replication info), and
    /// a globals object (`globals` false: none). Answers the globals object.
    fn world(&mut self, playthrough: Option<i32>, globals: bool) -> Option<ObjRef> {
        let actor = self.class("Engine.Actor");
        let info = self.make("Engine.WorldInfo");
        if let Some(p) = playthrough {
            let gri = self.make("WillowGame.WillowGameReplicationInfo");
            self.set(gri, "CurrentPlaythrough", Value::Int(p));
            self.set(info, "GRI", Value::Object(Some(gri)));
        }
        let mut actors = ow_world::actors::Actors::new(actor);
        actors.place_world_info(info).unwrap();
        self.vm.add_service(actors).unwrap();
        let q = ow_world::query::install(&mut self.vm).unwrap();
        if !globals {
            return None;
        }
        let g = self.make("WillowGame.WillowGlobals");
        q.borrow_mut().set_globals(g);
        Some(g)
    }
    fn balance(&mut self, names: &[&str]) -> ObjRef {
        let b = self.make("WillowGame.AIPawnBalanceDefinition");
        let entries = names
            .iter()
            .map(|n| Value::structure(self.play_through, vec![Value::Str((*n).into())]))
            .collect();
        self.set(b, "PlayThroughs", Value::array(entries));
        b
    }
    fn marks(&self) -> (u64, u64, u64) {
        (self.vm.space.heap.committed_writes(), self.vm.rng.draws(), self.vm.defs.load_transitions())
    }
}

fn not_read<T: std::fmt::Debug>(r: Result<T, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            n.what.to_string()
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_pipeline_reads_are_its_declared_fields() {
    let mut x = fx();
    let p = x.make("Engine.DamagePipeline");
    x.set(p, "BarrelSourceTime", Value::Float(1.5));
    x.set(p, "bCanHitFriendly", Value::Bool(true));
    assert_eq!(float(&x.vm, p, "BarrelSourceTime", "t").unwrap(), 1.5);
    assert!(boolean(&x.vm, p, "bCanHitFriendly", "t").unwrap());
    let bad = x.make("Test.BadPipeline");
    assert!(matches!(float(&x.vm, bad, "BarrelSourceTime", "t"), Err(VmError::Script { .. })), "an int is a data error");
    assert!(matches!(float(&x.vm, bad, "PlantSourceTime", "t"), Err(VmError::Script { .. })), "undeclared");
}

// Test 2.
#[test]
fn get_display_name_at_grade_by_its_own_playthrough_source() {
    for (playthrough, want) in [(Some(0), "Normal"), (Some(1), "True"), (None, "Normal"), (Some(-3), "")] {
        let mut x = fx();
        x.world(playthrough, true);
        let b = x.balance(&["Normal", "True"]);
        assert_eq!(display_name(&mut x.vm, b).unwrap(), want, "{playthrough:?}");
    }
    // Beyond the list: the last entry; an empty list: an empty string.
    let mut x = fx();
    x.world(Some(1), true);
    let one = x.balance(&["Only"]);
    assert_eq!(display_name(&mut x.vm, one).unwrap(), "Only");
    let none = x.balance(&[]);
    assert_eq!(display_name(&mut x.vm, none).unwrap(), "");
    // No globals object: an empty string.
    let mut x = fx();
    x.world(Some(0), false);
    let b = x.balance(&["Normal"]);
    assert_eq!(display_name(&mut x.vm, b).unwrap(), "");
    // Playthrough 2: the fixup's NotRead, nothing written.
    let mut x = fx();
    x.world(Some(2), true);
    let b = x.balance(&["Normal", "True", "Ultimate"]);
    let before = x.marks();
    assert_eq!(not_read(display_name(&mut x.vm, b)), NAME_FIXUP);
    assert_eq!(x.marks(), before);
    // The base class: an empty string; another balance class: NotRead.
    let base = x.make("Engine.BaseBalanceDefinition");
    assert_eq!(display_name(&mut x.vm, base).unwrap(), "");
    let other = x.make("Test.OtherBalance");
    assert!(not_read(display_name(&mut x.vm, other)).starts_with(OTHER_BALANCE));
}

// Test 3.
#[test]
fn notify_killed_enemy_s_gates() {
    let mut x = fx();
    let globals = x.world(Some(0), true).unwrap();
    let controller = x.make("WillowGame.WillowPlayerController");
    let info = x.make("Engine.WorldInfo");
    x.set(controller, "WorldInfo", Value::Object(Some(info)));
    // No globals definition yet: its first load, NotRead.
    let before = x.marks();
    assert_eq!(not_read(notify_killed_enemy(&x.vm, controller)), DEFINITION_COLD);
    // A definition not built (a stand-in short of its slots): NotRead naming it.
    let lazy = x.lazy("WillowGame.GlobalsDefinition");
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(lazy)));
    assert!(not_read(notify_killed_enemy(&x.vm, controller)).contains("WillowGame.GlobalsDefinition"));
    assert_eq!((x.marks().1, x.marks().2), (before.1, before.2), "nothing drawn or loaded");
    let definition = x.make("WillowGame.GlobalsDefinition");
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(definition)));
    // PlayerStats None: nothing; set: NotRead; nothing written either way.
    notify_killed_enemy(&x.vm, controller).unwrap();
    let stats = x.make("Core.Object");
    x.set(controller, "PlayerStats", Value::Object(Some(stats)));
    let before_set = x.marks();
    assert_eq!(not_read(notify_killed_enemy(&x.vm, controller)), KILL_BOOKKEEPING);
    assert_eq!(x.marks(), before_set);
    // No WorldInfo: nothing, before the definition is read.
    x.set(controller, "WorldInfo", Value::Object(None));
    x.set(globals, "TheGlobalsDefinition", Value::Object(None));
    notify_killed_enemy(&x.vm, controller).unwrap();
}

// Test 4.
#[test]
fn is_action_skill_running_is_my_action_skill_set() {
    let mut x = fx();
    let pawn = x.make("WillowGame.WillowPawn");
    assert_eq!(object(&x.vm, pawn, "MyActionSkill", "t").unwrap(), None);
    let skill = x.make("Core.Object");
    x.set(pawn, "MyActionSkill", Value::Object(Some(skill)));
    assert_eq!(object(&x.vm, pawn, "MyActionSkill", "t").unwrap(), Some(skill));
}

// Test 5.
#[test]
fn the_dialog_globals_from_the_globals_definition() {
    let mut x = fx();
    let globals = x.world(Some(0), true).unwrap();
    assert_eq!(dialog_globals(&x.vm).unwrap(), None, "no definition: None");
    let definition = x.make("WillowGame.GlobalsDefinition");
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(definition)));
    assert_eq!(dialog_globals(&x.vm).unwrap(), None, "no Dialog: None");
    let other = x.make("Core.Object");
    x.set(definition, "Dialog", Value::Object(Some(other)));
    assert_eq!(dialog_globals(&x.vm).unwrap(), None, "another class: None");
    let dialog = x.make("WillowGame.WillowDialogGlobalsDefinition");
    x.set(definition, "Dialog", Value::Object(Some(dialog)));
    assert_eq!(dialog_globals(&x.vm).unwrap(), Some(dialog));
    // A globals definition not built (a stand-in short of its slots): NotRead, nothing loaded.
    let lazy = x.lazy("WillowGame.GlobalsDefinition");
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(lazy)));
    let before = x.marks();
    assert!(not_read(dialog_globals(&x.vm)).contains("WillowGame.GlobalsDefinition"));
    assert_eq!(x.marks(), before);
    // A definition declaring Dialog as an int: a data error.
    let bad = x.make("Test.BadDefinition");
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(bad)));
    assert!(matches!(dialog_globals(&x.vm), Err(VmError::Script { .. })));
    // Reference: the set-up preload does nothing.
    x.vm.parity = ParityMode::Reference;
    assert_eq!(preload_dialog_globals(&mut x.vm).unwrap(), 0);
}
