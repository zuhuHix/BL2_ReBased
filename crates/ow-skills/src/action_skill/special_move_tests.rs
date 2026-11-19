//! Tests of the special-move play natives (NATIVE_SPECIAL_MOVE_PLAY.md; lane FIX-SM-PLAY, decision 2026-11-19-52) on
//! invented classes, objects and numbers (AGENTS.md rule 9), in both parity modes. The definition's script events
//! (`AuthorityCanPlay`, `ServerStarted`, `ClientStarted`, `ClientFinished`) and the callback are toy natives that
//! record their calls; the installed script is not run here.

use std::cell::RefCell;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, NativeCall, Vm};
use ow_vm::error::VmError;
use ow_world::actors::Actors;

use super::*;

const ROLES: [&str; 5] = ["ROLE_None", "ROLE_SimulatedProxy", "ROLE_AutonomousProxy", "ROLE_Authority", "ROLE_MAX"];
const NET_MODES: [&str; 5] = ["NM_Standalone", "NM_DedicatedServer", "NM_ListenServer", "NM_Client", "NM_MAX"];
const AUTONOMOUS: u8 = 2;
const AUTHORITY: u8 = 3;

/// The toy world: what the toy events saw, in order, and what they answer.
#[derive(Default)]
struct Script {
    calls: Vec<String>,
    can_play: bool,
    duration: f32,
}

struct Toy {
    vm: Vm,
    script: Rc<RefCell<Script>>,
    world_info: ObjRef,
}

fn decl(vm: &mut ObjectSpace, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.names.intern(name), kind)
}

fn param(space: &mut ObjectSpace, name: &str, kind: PropKind, optional: bool) -> PropertyDecl {
    let flags = PropertyDecl::PARAM | if optional { PropertyDecl::OPTIONAL } else { 0 };
    decl(space, name, kind).with_flags(flags)
}

fn class_of(vm: &Vm, path: &str) -> ClassId {
    vm.space.classes.by_path(path).unwrap()
}

impl Toy {
    fn new(mode: ParityMode) -> Toy {
        let mut s = ObjectSpace::new();
        let object = s.classes.define("Core.Object", None, vec![]).unwrap();
        let obj = PropKind::Object { class: object };
        let roles = ROLES.iter().map(|r| s.names.intern(r)).collect();
        let role = s.classes.define_enum("Engine.Actor.ENetRole", roles).unwrap();
        let modes = NET_MODES.iter().map(|m| s.names.intern(m)).collect();
        let net = s.classes.define_enum("Engine.WorldInfo.ENetMode", modes).unwrap();
        let a = vec![
            decl(&mut s, "Role", PropKind::Byte { enum_: Some(role) }),
            decl(&mut s, "bDeleteMe", PropKind::Bool),
        ];
        let actor = s.classes.define("Engine.Actor", Some(object), a).unwrap();
        let p = vec![
            decl(&mut s, "bIsInjured", PropKind::Bool),
            decl(&mut s, "bIsDead", PropKind::Bool),
            decl(&mut s, "Mesh", obj.clone()),
        ];
        let pawn = s.classes.define("Engine.Pawn", Some(actor), p).unwrap();
        s.classes.define("Test.Pawn", Some(pawn), vec![]).unwrap();
        let w = vec![
            decl(&mut s, "NetMode", PropKind::Byte { enum_: Some(net) }),
            decl(&mut s, "TimeSeconds", PropKind::Float),
        ];
        s.classes.define("Engine.WorldInfo", Some(actor), w).unwrap();
        let m = vec![decl(&mut s, "Animations", obj.clone())];
        s.classes.define("Engine.SkeletalMeshComponent", Some(object), m).unwrap();
        let n = vec![decl(&mut s, "SkelComponent", obj.clone())];
        let node = s.classes.define("Engine.AnimNode", Some(object), n).unwrap();
        s.classes.define("GearboxFramework.AnimNodeSpecialMoveBlend", Some(node), vec![]).unwrap();
        let interface_class = s.classes.define("GearboxFramework.SpecialMoveInterface", Some(object), vec![]).unwrap();
        let smi = PropKind::Interface { class: interface_class };
        let fields = vec![
            decl(&mut s, "Data", obj.clone()),
            decl(&mut s, "PlayRateScale", PropKind::Float),
            decl(&mut s, "Duration", PropKind::Float),
            decl(&mut s, "CallbackName", PropKind::Name),
            decl(&mut s, "CallbackObject", obj.clone()),
            decl(&mut s, "SMD", obj.clone()),
        ];
        let data = s
            .classes
            .define_struct("GearboxFramework.SpecialMoveInterface.SpecialMoveData", None, fields)
            .unwrap();
        let c = vec![
            decl(&mut s, "StartTime", PropKind::Float),
            decl(&mut s, "SMI", smi.clone()),
            decl(&mut s, "CurrentSMData", PropKind::Struct(data)),
            decl(&mut s, "SMDQueue", PropKind::Array(Box::new(PropKind::Struct(data)))),
            decl(&mut s, "SMBlendNode", obj.clone()),
        ];
        let component = s.classes.define("GearboxFramework.SpecialMoveComponent", Some(object), c).unwrap();
        let d = vec![
            decl(&mut s, "NextSpecialMove", obj.clone()),
            decl(&mut s, "bClientHasAuthority", PropKind::Bool),
        ];
        let definition = s.classes.define("GearboxFramework.SpecialMoveDefinition", Some(object), d).unwrap();
        s.classes.define("Test.Move", Some(definition), vec![]).unwrap();
        s.classes.define("Test.OtherMove", Some(definition), vec![]).unwrap();
        let mut vm = Vm::new(s, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);

        let class = |vm: &Vm, p: &str| class_of(vm, p);
        let ret = |vm: &mut Vm, kind: PropKind| decl(&mut vm.space, "ReturnValue", kind).with_flags(PropertyDecl::RETURN);
        let declare = |vm: &mut Vm, path: &str, key: &str, owner: &str, params: Vec<PropertyDecl>, result: Option<PropKind>| {
            let mut f = FunctionDecl::native(ObjectPath::parse(path).unwrap(), key)
                .with_owner(class(vm, owner))
                .with_params(params);
            if let Some(k) = result {
                f = f.with_result(ret(vm, k));
            }
            vm.declare_function(f).unwrap();
        };
        let smi_param = |vm: &mut Vm| param(&mut vm.space, "SMI", smi.clone(), false);
        declare(&mut vm, "Engine.Pawn.IsAliveAndWell", "Pawn.IsAliveAndWell()", "Engine.Pawn", vec![], Some(PropKind::Bool));
        let p = vec![smi_param(&mut vm)];
        declare(&mut vm, PATHS[1], "SpecialMoveDefinition.GetSMDToPlay(interface)", "GearboxFramework.SpecialMoveDefinition", p, Some(obj.clone()));
        let p = vec![smi_param(&mut vm)];
        declare(&mut vm, PATHS[2], "SpecialMoveDefinition.IsLocalAuthority(interface)", "GearboxFramework.SpecialMoveDefinition", p, Some(PropKind::Bool));
        let p = vec![param(&mut vm.space, "SMClass", PropKind::Class { meta: object }, false)];
        declare(&mut vm, PATHS[3], "SpecialMoveComponent.IsPlayingClass(class)", "GearboxFramework.SpecialMoveComponent", p, Some(PropKind::Bool));
        declare(&mut vm, PATHS[4], "SpecialMoveComponent.GetSMNode()", "GearboxFramework.SpecialMoveComponent", vec![], Some(obj.clone()));
        for (path, key) in [
            (PATHS[5], "SpecialMoveComponent.Play(object,float,float,name,object,object)"),
            (PATHS[6], "SpecialMoveComponent.PlayLocal(object,float,float,name,object,object)"),
        ] {
            let p = vec![
                param(&mut vm.space, "NewSMD", obj.clone(), false),
                param(&mut vm.space, "PlayRateScale", PropKind::Float, true),
                param(&mut vm.space, "Duration", PropKind::Float, true),
                param(&mut vm.space, "CallbackName", PropKind::Name, true),
                param(&mut vm.space, "CallbackObject", obj.clone(), true),
                param(&mut vm.space, "Data", obj.clone(), true),
            ];
            declare(&mut vm, path, key, "GearboxFramework.SpecialMoveComponent", p, Some(PropKind::Float));
        }
        // The toy script events and callback (invented keys), each recording its call.
        let script = Rc::new(RefCell::new(Script { can_play: true, duration: 2.5, ..Default::default() }));
        let events: [(&str, Vec<PropertyDecl>, Option<PropKind>); 5] = [
            ("AuthorityCanPlay", vec![smi_param(&mut vm)], Some(PropKind::Bool)),
            ("ServerStarted", vec![smi_param(&mut vm)], None),
            ("ClientStarted", vec![smi_param(&mut vm), param(&mut vm.space, "SMData", PropKind::Struct(data), false)], Some(PropKind::Float)),
            ("ClientFinished", vec![smi_param(&mut vm), param(&mut vm.space, "bInterrupted", PropKind::Bool, false)], None),
            ("Done", vec![param(&mut vm.space, "Count", PropKind::Int, false), param(&mut vm.space, "bInterrupted", PropKind::Bool, false)], None),
        ];
        for (name, params, result) in events {
            let owner = if name == "Done" { "Test.Pawn" } else { "GearboxFramework.SpecialMoveDefinition" };
            let key = format!("Toy.{name}");
            declare(&mut vm, &format!("{owner}.{name}"), &key, owner, params, result);
            let s = script.clone();
            vm.register_native(
                &key,
                Rc::new(move |c: &mut NativeCall<'_>| {
                    let me = ow_attr::eval::class_path(&c.vm.space, c.this.unwrap());
                    let args: Vec<String> = (0..c.arg_count())
                        .map(|i| match c.arg(i).unwrap() {
                            Value::Bool(b) => format!("{b}"),
                            Value::Int(n) => format!("{n}"),
                            Value::Struct(_) => "data".into(),
                            _ => "_".into(),
                        })
                        .collect();
                    let mut s = s.borrow_mut();
                    s.calls.push(format!("{name} on {me}({})", args.join(",")));
                    Ok(match name {
                        "AuthorityCanPlay" => Value::Bool(s.can_play),
                        "ClientStarted" => Value::Float(s.duration),
                        _ => Value::None,
                    })
                }),
            )
            .unwrap();
        }
        let _ = component;
        let actors = vm.add_service(Actors::new(actor)).unwrap();
        let wc = class_of(&vm, "Engine.WorldInfo");
        let world_info = vm.space.alloc(wc, "WorldInfo", None, Origin::Runtime).unwrap();
        actors.borrow_mut().place_world_info(world_info).unwrap();
        register(&mut vm).unwrap();
        let mut t = Toy { vm, script, world_info };
        t.set(world_info, "TimeSeconds", Value::Float(12.0));
        t
    }

    fn obj(&mut self, class: &str, name: &str) -> ObjRef {
        let c = class_of(&self.vm, class);
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
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

    /// A pawn of role `role` and its component (the component's `SMI` is the pawn).
    fn owner(&mut self, role: u8) -> (ObjRef, ObjRef) {
        let pawn = self.obj("Test.Pawn", "Maya");
        self.set(pawn, "Role", Value::Byte(role));
        let component = self.obj("GearboxFramework.SpecialMoveComponent", "SpecialMoveComponent0");
        self.set(component, "SMI", Value::Interface(Some(pawn)));
        (pawn, component)
    }

    fn definition(&mut self, class: &str, client_authority: bool) -> ObjRef {
        let d = self.obj(class, "Phaselock_Toy_SM");
        self.set(d, "bClientHasAuthority", Value::Bool(client_authority));
        d
    }

    fn call(&mut self, path: &str, this: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path)?;
        Ok(self.vm.call(f, Some(this), args)?.ret)
    }

    fn play(&mut self, component: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        self.call(PATHS[5], component, args)
    }

    fn calls(&self) -> Vec<String> {
        self.script.borrow().calls.clone()
    }

    fn current_smd(&self, component: ObjRef) -> Value {
        let current = self.get(component, "CurrentSMData");
        field(&self.vm, &current, "SMD", "test").unwrap()
    }
}

fn not_yet(r: Result<Value, VmError>) -> (Missing, String) {
    match r {
        Err(VmError::NotYet(n)) => (n.missing.clone(), n.to_string()),
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// `IsAliveAndWell`: true only with `bDeleteMe`, `bIsInjured` and `bIsDead` all clear.
#[test]
fn is_alive_and_well_needs_all_three_flags_clear() {
    let mut t = Toy::new(ParityMode::Off);
    let (pawn, _) = t.owner(AUTHORITY);
    assert_eq!(t.call(PATHS[0], pawn, vec![]).unwrap(), Value::Bool(true));
    for flag in ["bDeleteMe", "bIsInjured", "bIsDead"] {
        t.set(pawn, flag, Value::Bool(true));
        assert_eq!(t.call(PATHS[0], pawn, vec![]).unwrap(), Value::Bool(false), "{flag}");
        t.set(pawn, flag, Value::Bool(false));
    }
}

/// `GetSMDToPlay` is the definition itself; `IsLocalAuthority` is the authority, or an autonomous proxy when the
/// definition sets `bClientHasAuthority`.
#[test]
fn the_definition_natives() {
    let mut t = Toy::new(ParityMode::Off);
    let d = t.definition("Test.Move", false);
    let (pawn, _) = t.owner(AUTHORITY);
    let smi = Value::Interface(Some(pawn));
    assert_eq!(t.call(PATHS[1], d, vec![smi.clone()]).unwrap(), Value::Object(Some(d)));
    assert_eq!(t.call(PATHS[2], d, vec![smi.clone()]).unwrap(), Value::Bool(true));
    t.set(pawn, "Role", Value::Byte(AUTONOMOUS));
    assert_eq!(t.call(PATHS[2], d, vec![smi.clone()]).unwrap(), Value::Bool(false));
    t.set(d, "bClientHasAuthority", Value::Bool(true));
    assert_eq!(t.call(PATHS[2], d, vec![smi.clone()]).unwrap(), Value::Bool(true));
    t.set(pawn, "Role", Value::Byte(1));
    assert_eq!(t.call(PATHS[2], d, vec![smi]).unwrap(), Value::Bool(false), "a simulated proxy");
}

/// `IsPlayingClass`: false with no class or no current move; true for the current definition's class and a parent.
#[test]
fn is_playing_class_follows_the_current_move() {
    let mut t = Toy::new(ParityMode::Off);
    let (_, component) = t.owner(AUTHORITY);
    let moves = class_of(&t.vm, "Test.Move");
    let base = class_of(&t.vm, "GearboxFramework.SpecialMoveDefinition");
    let other = class_of(&t.vm, "Test.OtherMove");
    assert_eq!(t.call(PATHS[3], component, vec![Value::Class(Some(moves))]).unwrap(), Value::Bool(false));
    let d = t.definition("Test.Move", false);
    let current = t.get(component, "CurrentSMData");
    let current = with_field(&t.vm, current, "SMD", Value::Object(Some(d)), "test").unwrap();
    t.set(component, "CurrentSMData", current);
    assert_eq!(t.call(PATHS[3], component, vec![Value::Class(Some(moves))]).unwrap(), Value::Bool(true));
    assert_eq!(t.call(PATHS[3], component, vec![Value::Class(Some(base))]).unwrap(), Value::Bool(true));
    assert_eq!(t.call(PATHS[3], component, vec![Value::Class(Some(other))]).unwrap(), Value::Bool(false));
    assert_eq!(t.call(PATHS[3], component, vec![Value::Class(None)]).unwrap(), Value::Bool(false));
}

/// `GetSMNode`: None with no owner; the cached node on the owner's mesh; otherwise `NotYet`, naming the missing anim
/// tree for a headless owner, and the node search for an owner with a tree.
#[test]
fn get_sm_node_answers_not_yet_for_a_headless_owner() {
    let mut t = Toy::new(ParityMode::Off);
    let bare = t.obj("GearboxFramework.SpecialMoveComponent", "NoOwner");
    assert_eq!(t.call(PATHS[4], bare, vec![]).unwrap(), Value::Object(None));
    let (pawn, component) = t.owner(AUTHORITY);
    let (missing, text) = not_yet(t.call(PATHS[4], component, vec![]));
    assert!(matches!(missing, Missing::LayerAbsent(_)), "{text}");
    assert!(text.contains("no anim tree"), "{text}");
    let mesh = t.obj("Engine.SkeletalMeshComponent", "Mesh0");
    t.set(pawn, "Mesh", Value::Object(Some(mesh)));
    let (missing, _) = not_yet(t.call(PATHS[4], component, vec![]));
    assert!(matches!(missing, Missing::LayerAbsent(_)), "a mesh with no Animations");
    let tree = t.obj("Engine.AnimNode", "Tree");
    t.set(mesh, "Animations", Value::Object(Some(tree)));
    let (missing, text) = not_yet(t.call(PATHS[4], component, vec![]));
    assert_eq!(missing, Missing::NotPorted, "{text}");
    let node = t.obj("GearboxFramework.AnimNodeSpecialMoveBlend", "SpecialMoveBlend");
    t.set(node, "SkelComponent", Value::Object(Some(mesh)));
    t.set(component, "SMBlendNode", Value::Object(Some(node)));
    assert_eq!(t.call(PATHS[4], component, vec![]).unwrap(), Value::Object(Some(node)), "the cached node");
    let other_mesh = t.obj("Engine.SkeletalMeshComponent", "Mesh1");
    t.set(pawn, "Mesh", Value::Object(Some(other_mesh)));
    let (missing, _) = not_yet(t.call(PATHS[4], component, vec![]));
    assert!(matches!(missing, Missing::LayerAbsent(_)), "a cache on another mesh is not reused");
}

/// `Play`'s zero answers: no definition, no owner interface, a failed gate (nothing called past it, nothing changed).
#[test]
fn play_answers_zero_without_a_definition_an_owner_or_the_gates() {
    let mut t = Toy::new(ParityMode::Off);
    let d = t.definition("Test.Move", true);
    let (_, component) = t.owner(AUTONOMOUS);
    assert_eq!(t.play(component, vec![Value::Object(None)]).unwrap(), Value::Float(0.0));
    let bare = t.obj("GearboxFramework.SpecialMoveComponent", "NoOwner");
    assert_eq!(t.play(bare, vec![Value::Object(Some(d))]).unwrap(), Value::Float(0.0));
    t.script.borrow_mut().can_play = false;
    assert_eq!(t.play(component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(0.0));
    assert_eq!(t.calls(), vec!["AuthorityCanPlay on Test.Move(_)"]);
    let plain = t.definition("Test.Move", false);
    t.script.borrow_mut().can_play = true;
    assert_eq!(t.play(component, vec![Value::Object(Some(plain))]).unwrap(), Value::Float(0.0), "not the local authority");
    assert_eq!(t.calls().len(), 1, "AuthorityCanPlay is not asked when IsLocalAuthority fails");
    assert_eq!(t.current_smd(component), Value::Object(None));
}

/// The start on an owner below authority in a standalone world: the request becomes the current move with the world
/// time, `ClientStarted` answers the duration; a second play stops the first as interrupted and calls its callback on
/// the owner, `bInterrupted` set and the other parameter zero.
#[test]
fn play_starts_the_move_and_a_second_play_interrupts_the_first() {
    let mut t = Toy::new(ParityMode::Off);
    let first = t.definition("Test.Move", true);
    let (_, component) = t.owner(AUTONOMOUS);
    let done = Value::Name(t.vm.space.names.intern("Done"));
    let got = t
        .play(component, vec![Value::Object(Some(first)), Value::Float(1.5), Value::Float(0.0), done])
        .unwrap();
    assert_eq!(got, Value::Float(2.5));
    assert_eq!(t.calls(), vec!["AuthorityCanPlay on Test.Move(_)", "ClientStarted on Test.Move(_,data)"]);
    assert_eq!(t.current_smd(component), Value::Object(Some(first)));
    assert_eq!(t.get(component, "StartTime"), Value::Float(12.0));
    let current = t.get(component, "CurrentSMData");
    assert_eq!(field(&t.vm, &current, "PlayRateScale", "test").unwrap(), Value::Float(1.5));
    let second = t.definition("Test.OtherMove", true);
    let wi = t.world_info;
    t.set(wi, "TimeSeconds", Value::Float(13.0));
    t.script.borrow_mut().calls.clear();
    assert_eq!(t.play(component, vec![Value::Object(Some(second))]).unwrap(), Value::Float(2.5));
    assert_eq!(
        t.calls(),
        vec![
            "AuthorityCanPlay on Test.OtherMove(_)",
            "ClientFinished on Test.Move(_,true)",
            "Done on Test.Pawn(0,true)",
            "ClientStarted on Test.OtherMove(_,data)",
        ]
    );
    assert_eq!(t.current_smd(component), Value::Object(Some(second)));
    assert_eq!(t.get(component, "StartTime"), Value::Float(13.0));
    let current = t.get(component, "CurrentSMData");
    assert_eq!(field(&t.vm, &current, "PlayRateScale", "test").unwrap(), Value::Float(1.0), "the default rate");
}

/// A duration of 0 or less stops the move again, not interrupted; no callback was asked for.
#[test]
fn a_zero_duration_stops_the_move_again() {
    let mut t = Toy::new(ParityMode::Off);
    let d = t.definition("Test.Move", true);
    let (_, component) = t.owner(AUTONOMOUS);
    t.script.borrow_mut().duration = 0.0;
    assert_eq!(t.play(component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(0.0));
    assert_eq!(
        t.calls(),
        vec!["AuthorityCanPlay on Test.Move(_)", "ClientStarted on Test.Move(_,data)", "ClientFinished on Test.Move(_,false)"]
    );
    assert_eq!(t.current_smd(component), Value::Object(None));
}

/// The refusals, each before anything changes: the authority's replication record, a network client's forward, a
/// definition with follow-ups.
#[test]
fn play_refuses_what_the_note_leaves_open() {
    let mut t = Toy::new(ParityMode::Off);
    let d = t.definition("Test.Move", true);
    let (_, component) = t.owner(AUTHORITY);
    let (missing, text) = not_yet(t.play(component, vec![Value::Object(Some(d))]));
    assert_eq!(missing, Missing::NotRead, "{text}");
    assert!(text.contains("replication"), "{text}");
    assert_eq!(t.current_smd(component), Value::Object(None));
    let (_, component) = t.owner(AUTONOMOUS);
    let wi = t.world_info;
    t.set(wi, "NetMode", Value::Byte(3));
    let (missing, text) = not_yet(t.play(component, vec![Value::Object(Some(d))]));
    assert_eq!(missing, Missing::NotPorted, "{text}");
    t.set(wi, "NetMode", Value::Byte(0));
    let next = t.definition("Test.OtherMove", true);
    t.set(d, "NextSpecialMove", Value::Object(Some(next)));
    let (missing, text) = not_yet(t.play(component, vec![Value::Object(Some(d))]));
    assert_eq!(missing, Missing::NotRead, "{text}");
    assert!(text.contains("NextSpecialMove"), "{text}");
    assert_eq!(t.current_smd(component), Value::Object(None));
}

/// `PlayLocal`: no gate and no current move; `ServerStarted` then `ClientStarted`, whose answer is the answer.
#[test]
fn play_local_runs_both_starts_and_keeps_no_current_move() {
    let mut t = Toy::new(ParityMode::Off);
    let d = t.definition("Test.Move", false);
    let (_, component) = t.owner(AUTHORITY);
    t.script.borrow_mut().can_play = false;
    assert_eq!(t.call(PATHS[6], component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(2.5));
    assert_eq!(t.calls(), vec!["ServerStarted on Test.Move(_)", "ClientStarted on Test.Move(_,data)"]);
    assert_eq!(t.current_smd(component), Value::Object(None));
    assert_eq!(t.call(PATHS[6], component, vec![Value::Object(None)]).unwrap(), Value::Float(0.0));
}

/// Reference binds none of them (no C++ stand-in).
#[test]
fn reference_binds_nothing() {
    let mut t = Toy::new(ParityMode::Reference);
    for path in PATHS {
        let f = t.vm.find_function(path).unwrap();
        let key = t.vm.function(f).unwrap().native_key.clone().unwrap();
        assert!(!t.vm.has_native(&key), "{path}");
    }
}
