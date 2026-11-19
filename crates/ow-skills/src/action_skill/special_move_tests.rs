//! Tests of the special-move play natives (NATIVE_SPECIAL_MOVE_PLAY.md; lane FIX-SM-PLAY, decision 2026-11-19-52) on
//! two invented packages built with `ow_package::testkit` (AGENTS.md rule 9; review of #437): `Engine` (actor, pawn,
//! world info, mesh and anim nodes, a toy skill whose script chooses a hit or fizzle move) and `GearboxFramework` (the
//! interface and its move data, the component, the definition and two toy moves). Every native is declared by the
//! packages and reached through the VM by its function path. The definition's script events (`AuthorityCanPlay`,
//! `ServerStarted`, `ClientStarted`, `ClientFinished`) and the callbacks are declared native here and answered by
//! closures that record their calls.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{Asm, CPF_OPT, CPF_OUT, CPF_PARM, CPF_RET, FunctionSpec, ScriptToy, function_payload};
use ow_package::testkit::{Export, ints};
use ow_vm::api::{NativeCall, NativePack, StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_world::world::{BalanceState, World};

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 14] = [
    "Class",
    "Function",
    "ScriptStruct",
    "Enum",
    "IntProperty",
    "FloatProperty",
    "BoolProperty",
    "ByteProperty",
    "NameProperty",
    "ObjectProperty",
    "ClassProperty",
    "InterfaceProperty",
    "ArrayProperty",
    "StructProperty",
];

const ROLES: [&str; 5] = ["ROLE_None", "ROLE_SimulatedProxy", "ROLE_AutonomousProxy", "ROLE_Authority", "ROLE_MAX"];
const NET_MODES: [&str; 5] = ["NM_Standalone", "NM_DedicatedServer", "NM_ListenServer", "NM_Client", "NM_MAX"];
const SIMULATED: u8 = 1;
const AUTONOMOUS: u8 = 2;
const AUTHORITY: u8 = 3;

/// One declared parameter: kind (without `Property`), name, extra flags, the property's tail words.
struct D<'a>(&'a str, &'a str, u64, Vec<i32>);

/// An Enum export `[0][None][0][count][values]` in `owner`.
fn enumeration(t: &mut ScriptToy, owner: i32, name: &str, values: &[&str]) -> i32 {
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    payload.extend(ints(&[0, i32::try_from(values.len()).unwrap()]));
    for v in values {
        payload.extend_from_slice(&t.p.fname(v, 0));
    }
    let mut e = Export::new(t.import("Enum"), name, payload);
    e.outer = owner;
    t.p.add_export_with(e)
}

/// A function with these parameters (each property's tail given whole, so a class property names its metaclass) and
/// result, the return value and the parameters exported first (as `ScriptToy::function` does). `body` gets the
/// parameter exports by name. Returns the function export.
fn function(
    t: &mut ScriptToy,
    owner: i32,
    name: &str,
    declared: &[D<'_>],
    result: Option<(&str, Vec<i32>)>,
    flags: u32,
    body: impl FnOnce(&dyn Fn(&str) -> i32) -> Option<Asm>,
) -> i32 {
    let mut ordered: Vec<(String, String, u64, Vec<i32>)> = Vec::new();
    if let Some((kind, tail)) = result {
        ordered.push((kind.into(), "ReturnValue".into(), CPF_PARM | CPF_OUT | CPF_RET, tail));
    }
    for d in declared.iter().rev() {
        ordered.push((d.0.into(), d.1.into(), CPF_PARM | d.2, d.3.clone()));
    }
    let f = t.p.next_export() + i32::try_from(ordered.len()).unwrap();
    let mut ids = Vec::new();
    for (kind, child, flags, tail) in &ordered {
        ids.push((child.clone(), t.prop_tail(kind, f, child, *flags, &ints(tail))));
    }
    let lookup = |n: &str| ids.iter().find(|(c, _)| c == n).map(|x| x.1).expect("a parameter");
    let asm = body(&lookup);
    let payload = function_payload(&mut t.p, asm.as_ref(), &FunctionSpec::new(flags, name));
    let mut e = Export::new(t.import("Function"), name, payload);
    e.outer = owner;
    assert_eq!(t.p.add_export_with(e), f);
    f
}

fn native(t: &mut ScriptToy, owner: i32, name: &str, declared: &[D<'_>], result: Option<(&str, Vec<i32>)>) {
    function(t, owner, name, declared, result, NATIVE, |_| None);
}

/// `Engine`: `Object`, `Actor` (`Role`, `bDeleteMe`), `Pawn` (the health flags, `Mesh`, `Allegiance`, the natives
/// `IsAliveAndWell`, `IsFriendly` and two toy callbacks), `PawnAllegiance`, `WorldInfo`, `SkeletalMeshComponent`, the
/// anim nodes, and `ToyLiftSkill`, whose script `GetPlayerAnimation(Target)` has the shape of the cast's choice:
/// `if (Caster.IsFriendly(Target)) return Miss; if (Target.IsAliveAndWell()) return Hit; return Miss;`.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let (friendly, alive) = (t.p.fname("IsFriendly", 0), t.p.fname("IsAliveAndWell", 0));
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let roles = enumeration(&mut t, actor, "ENetRole", &ROLES);
    t.prop("Byte", actor, "Role", 0, roles);
    t.prop("Bool", actor, "bDeleteMe", 0, 0);
    let allegiance = t.class("PawnAllegiance", object);
    t.prop("Byte", allegiance, "SelfOpinion", 0, 0);
    let pawn = t.class("Pawn", actor);
    t.prop("Bool", pawn, "bIsInjured", 0, 0);
    t.prop("Bool", pawn, "bIsDead", 0, 0);
    t.prop("Object", pawn, "Mesh", 0, object);
    t.prop("Object", pawn, "Allegiance", 0, allegiance);
    native(&mut t, pawn, "IsAliveAndWell", &[], Some(("Bool", vec![0])));
    native(&mut t, pawn, "IsFriendly", &[D("Object", "Other", 0, vec![pawn])], Some(("Bool", vec![0])));
    native(&mut t, pawn, "GetOpinion", &[D("Object", "Other", 0, vec![pawn])], Some(("Byte", vec![0])));
    native(&mut t, pawn, "IsEnemy", &[D("Object", "Other", 0, vec![pawn])], Some(("Bool", vec![0])));
    native(&mut t, pawn, "ToyDone", &[D("Bool", "bInterrupted", 0, vec![0])], None);
    let controller = t.class("Controller", actor);
    t.prop("Object", controller, "Pawn", 0, pawn);
    native(&mut t, pawn, "ToyDoneWithCount", &[D("Int", "Count", 0, vec![0]), D("Bool", "bInterrupted", 0, vec![0])], None);
    let world = t.class("WorldInfo", actor);
    let modes = enumeration(&mut t, world, "ENetMode", &NET_MODES);
    t.prop("Byte", world, "NetMode", 0, modes);
    t.prop("Float", world, "TimeSeconds", 0, 0);
    let mesh = t.class("SkeletalMeshComponent", object);
    t.prop("Object", mesh, "Animations", 0, object);
    let node = t.class("AnimNode", object);
    t.prop("Object", node, "SkelComponent", 0, mesh);
    t.prop("Name", node, "NodeName", 0, 0);
    let blend = t.class("AnimNodeBlendBase", node);
    let child = toy_struct(&mut t, blend, "AnimBlendChild", None);
    t.prop("Name", child, "Name", 0, 0);
    t.prop("Object", child, "Anim", 0, node);
    toy_array_of(&mut t, blend, "Children", "Struct", child);
    let skill = t.class("ToyLiftSkill", object);
    let caster = t.prop("Object", skill, "Caster", 0, pawn);
    let hit = t.prop("Object", skill, "Hit", 0, object);
    let miss = t.prop("Object", skill, "Miss", 0, object);
    function(
        &mut t,
        skill,
        "GetPlayerAnimation",
        &[D("Object", "Target", 0, vec![pawn])],
        Some(("Object", vec![object])),
        SCRIPT,
        |p| {
            let target = p("Target");
            let mut a = Asm::new();
            a.stmt();
            let friendly_at = a.jump_if_not(|a| {
                a.context(|a| { a.instance(caster); }, |a| { a.raw(&[0x1B]).raw(&friendly).local(target).raw(&[0x16]); });
            });
            a.stmt().raw(&[0x04]).instance(miss);
            a.patch(friendly_at);
            a.stmt();
            let alive_at = a.jump_if_not(|a| {
                a.context(|a| { a.local(target); }, |a| { a.raw(&[0x1B]).raw(&alive).raw(&[0x16]); });
            });
            a.stmt().raw(&[0x04]).instance(hit);
            a.patch(alive_at);
            a.stmt().raw(&[0x04]).instance(miss);
            a.end();
            Some(a)
        },
    );
    t.build()
}

/// `GearboxFramework`: `SpecialMoveInterface` with `SpecialMoveData`, `SpecialMoveComponent` with its natives,
/// `SpecialMoveDefinition` with its natives and events, the toy moves, and `AnimNodeSpecialMoveBlend`.
fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let blend_base = t.p.add_import("Core", "Class", engine, "AnimNodeBlendBase");
    let interface = t.class("SpecialMoveInterface", object);
    let data = toy_struct(&mut t, interface, "SpecialMoveData", None);
    t.prop("Object", data, "Data", 0, object);
    t.prop("Float", data, "PlayRateScale", 0, 0);
    t.prop("Float", data, "Duration", 0, 0);
    t.prop("Name", data, "CallbackName", 0, 0);
    t.prop("Object", data, "CallbackObject", 0, object);
    t.prop("Object", data, "SMD", 0, object);
    let component = t.class("SpecialMoveComponent", object);
    t.prop("Float", component, "StartTime", 0, 0);
    t.prop("Interface", component, "SMI", 0, interface);
    t.prop("Struct", component, "CurrentSMData", 0, data);
    toy_array_of(&mut t, component, "SMDQueue", "Struct", data);
    t.prop("Object", component, "SMBlendNode", 0, object);
    let smi = || D("Interface", "SMI", 0, vec![interface]);
    for name in ["Play", "PlayLocal"] {
        let declared = [
            D("Object", "NewSMD", 0, vec![object]),
            D("Float", "PlayRateScale", CPF_OPT, vec![0]),
            D("Float", "Duration", CPF_OPT, vec![0]),
            D("Name", "CallbackName", CPF_OPT, vec![0]),
            D("Object", "CallbackObject", CPF_OPT, vec![object]),
            D("Object", "Data", CPF_OPT, vec![object]),
        ];
        native(&mut t, component, name, &declared, Some(("Float", vec![0])));
    }
    native(&mut t, component, "GetSMNode", &[], Some(("Object", vec![object])));
    native(&mut t, component, "IsPlayingAny", &[], Some(("Bool", vec![0])));
    native(&mut t, component, "IsPlayingClass", &[D("Class", "SMClass", 0, vec![object, object])], Some(("Bool", vec![0])));
    let definition = t.class("SpecialMoveDefinition", object);
    t.prop("Object", definition, "NextSpecialMove", 0, definition);
    t.prop("Bool", definition, "bClientHasAuthority", 0, 0);
    native(&mut t, definition, "GetSMDToPlay", &[smi()], Some(("Object", vec![definition])));
    native(&mut t, definition, "IsLocalAuthority", &[smi()], Some(("Bool", vec![0])));
    native(&mut t, definition, "AuthorityCanPlay", &[smi()], Some(("Bool", vec![0])));
    native(&mut t, definition, "ServerStarted", &[smi()], None);
    native(&mut t, definition, "ClientStarted", &[smi(), D("Struct", "SMData", 0, vec![data])], Some(("Float", vec![0])));
    native(&mut t, definition, "ClientFinished", &[smi(), D("Bool", "bInterrupted", 0, vec![0])], None);
    t.class("ToyMove", definition);
    t.class("ToyOtherMove", definition);
    t.class("AnimNodeSpecialMoveBlend", blend_base);
    t.build()
}

/// What the toy events saw, in order, and what they answer.
#[derive(Default)]
struct Script {
    calls: Vec<String>,
    can_play: bool,
    duration: f32,
    /// `ClientStarted` fails (an unbound native it reaches, say).
    fail_start: bool,
    /// `AuthorityCanPlay` asks this component's `GetSMNode` first (as the installed script does).
    probe: Option<ObjRef>,
    /// `ClientFinished` demotes this pawn to a simulated proxy.
    demote: Option<ObjRef>,
    /// `ClientStarted` ends this component's current move itself.
    end_on_start: Option<ObjRef>,
}

/// The recorded events and callbacks: function path and what each answers.
const EVENTS: [&str; 6] = [
    "GearboxFramework.SpecialMoveDefinition.AuthorityCanPlay",
    "GearboxFramework.SpecialMoveDefinition.ServerStarted",
    "GearboxFramework.SpecialMoveDefinition.ClientStarted",
    "GearboxFramework.SpecialMoveDefinition.ClientFinished",
    "Engine.Pawn.ToyDone",
    "Engine.Pawn.ToyDoneWithCount",
];

struct Fx {
    world: World,
    script: Rc<RefCell<Script>>,
    world_info: ObjRef,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The action skill pack alone.
fn fx(mode: ParityMode) -> Fx {
    fx_with(mode, &[&crate::action_skill::Pack])
}

/// A world over the two packages in `mode` with `packs`, the toy events bound, and a placed `WorldInfo` at time 12.
fn fx_with(mode: ParityMode, packs: &[&dyn NativePack]) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-skills-sm-play-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g) = (engine(), gearbox());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .unwrap();
    let mut world = World::new(vm, BalanceState::new(snapshot), packs).unwrap();
    let script = Rc::new(RefCell::new(Script { can_play: true, duration: 2.5, ..Default::default() }));
    for path in EVENTS {
        let f = world.vm.find_function(path).unwrap();
        let key = world.vm.function(f).unwrap().native_key.clone().unwrap();
        let name = path.rsplit('.').next().unwrap();
        let s = script.clone();
        world
            .vm
            .register_native(
                &key,
                Rc::new(move |c: &mut NativeCall<'_>| {
                    let me = ow_attr::eval::class_path(&c.vm.space, c.this.unwrap()).rsplit('.').next().unwrap().to_string();
                    let args: Vec<String> = (0..c.arg_count())
                        .map(|i| match c.arg(i).unwrap() {
                            Value::Bool(b) => format!("{b}"),
                            Value::Int(n) => format!("{n}"),
                            Value::Struct(_) => "data".into(),
                            _ => "_".into(),
                        })
                        .collect();
                    let (probe, demote, end) = {
                        let s = s.borrow();
                        (s.probe, s.demote, s.end_on_start)
                    };
                    if name == "AuthorityCanPlay"
                        && let Some(component) = probe
                    {
                        let f = c.vm.find_function(PATHS[4])?;
                        c.vm.call(f, Some(component), vec![])?;
                    }
                    if name == "ClientFinished"
                        && let Some(pawn) = demote
                    {
                        let class = c.vm.space.heap.get(pawn)?.class;
                        let p = c.vm.space.prop(class, "Role").unwrap();
                        c.vm.space.write(pawn, PropAddr::scalar(p), Value::Byte(SIMULATED))?;
                    }
                    if name == "ClientStarted"
                        && let Some(component) = end
                    {
                        let class = c.vm.space.heap.get(component)?.class;
                        let p = c.vm.space.prop(class, "CurrentSMData").unwrap();
                        let current = c.vm.space.read(component, PropAddr::scalar(p))?.clone();
                        let ended = with_field(c.vm, current, "SMD", Value::Object(None), "test")?;
                        c.vm.space.write(component, PropAddr::scalar(p), ended)?;
                    }
                    let mut s = s.borrow_mut();
                    s.calls.push(format!("{name} on {me}({})", args.join(",")));
                    match name {
                        "AuthorityCanPlay" => Ok(Value::Bool(s.can_play)),
                        "ClientStarted" if s.fail_start => Err(VmError::NotYet(ow_core::NotYet::not_read("a toy unbound native", "test"))),
                        "ClientStarted" => Ok(Value::Float(s.duration)),
                        _ => Ok(Value::None),
                    }
                }),
            )
            .unwrap();
    }
    let wi = make(&mut world.vm, "Engine.WorldInfo");
    world.actors().borrow_mut().place_world_info(wi).unwrap();
    let mut x = Fx { world, script, world_info: wi, dir };
    x.set(wi, "TimeSeconds", Value::Float(12.0));
    x
}

fn make(vm: &mut Vm, class: &str) -> ObjRef {
    let c = vm.find_class(class).unwrap();
    ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
}

impl Fx {
    fn vm(&mut self) -> &mut Vm {
        &mut self.world.vm
    }

    fn obj(&mut self, class: &str) -> ObjRef {
        make(self.vm(), class)
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let vm = self.vm();
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap();
        vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&mut self, o: ObjRef, name: &str) -> Value {
        let vm = self.vm();
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap();
        vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    /// A pawn of role `role` and its component (the component's `SMI` is the pawn).
    fn owner(&mut self, role: u8) -> (ObjRef, ObjRef) {
        let pawn = self.obj("Engine.Pawn");
        self.set(pawn, "Role", Value::Byte(role));
        let component = self.obj("GearboxFramework.SpecialMoveComponent");
        self.set(component, "SMI", Value::Interface(Some(pawn)));
        (pawn, component)
    }

    fn definition(&mut self, class: &str, client_authority: bool) -> ObjRef {
        let d = self.obj(class);
        self.set(d, "bClientHasAuthority", Value::Bool(client_authority));
        d
    }

    fn call(&mut self, path: &str, this: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let vm = self.vm();
        let f = vm.find_function(path)?;
        Ok(vm.call(f, Some(this), args)?.ret)
    }

    fn play(&mut self, component: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        self.call(PATHS[5], component, args)
    }

    fn calls(&self) -> Vec<String> {
        self.script.borrow().calls.clone()
    }

    fn clear_calls(&self) {
        self.script.borrow_mut().calls.clear();
    }

    fn current(&mut self, component: ObjRef, f: &str) -> Value {
        let current = self.get(component, "CurrentSMData");
        field(&self.world.vm, &current, f, "test").unwrap()
    }

    fn name(&mut self, text: &str) -> Value {
        Value::Name(self.vm().space.names.intern(text))
    }

    /// The component's three mutable fields, for "nothing changed".
    fn state(&mut self, component: ObjRef) -> (Value, Value, Value) {
        (self.get(component, "CurrentSMData"), self.get(component, "SMDQueue"), self.get(component, "StartTime"))
    }

    /// Seeds a current move `smd` (with callback `callback`), a one-entry queue and a `StartTime` of 5.
    fn seed(&mut self, component: ObjRef, smd: ObjRef, callback: Option<&str>) {
        let mut current = self.get(component, "CurrentSMData");
        current = with_field(&self.world.vm, current, "SMD", Value::Object(Some(smd)), "test").unwrap();
        if let Some(cb) = callback {
            let n = self.name(cb);
            current = with_field(&self.world.vm, current, "CallbackName", n, "test").unwrap();
        }
        self.set(component, "CurrentSMData", current.clone());
        self.set(component, "SMDQueue", Value::array(vec![current]));
        self.set(component, "StartTime", Value::Float(5.0));
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
    let mut x = fx(ParityMode::Off);
    let (pawn, _) = x.owner(AUTHORITY);
    assert_eq!(x.call(PATHS[0], pawn, vec![]).unwrap(), Value::Bool(true));
    for flag in ["bDeleteMe", "bIsInjured", "bIsDead"] {
        x.set(pawn, flag, Value::Bool(true));
        assert_eq!(x.call(PATHS[0], pawn, vec![]).unwrap(), Value::Bool(false), "{flag}");
        x.set(pawn, flag, Value::Bool(false));
    }
}

/// `GetSMDToPlay` is the definition itself; `IsLocalAuthority` is the authority, or an autonomous proxy when the
/// definition sets `bClientHasAuthority`.
#[test]
fn the_definition_natives() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", false);
    let (pawn, _) = x.owner(AUTHORITY);
    let smi = Value::Interface(Some(pawn));
    assert_eq!(x.call(PATHS[1], d, vec![smi.clone()]).unwrap(), Value::Object(Some(d)));
    assert_eq!(x.call(PATHS[2], d, vec![smi.clone()]).unwrap(), Value::Bool(true));
    x.set(pawn, "Role", Value::Byte(AUTONOMOUS));
    assert_eq!(x.call(PATHS[2], d, vec![smi.clone()]).unwrap(), Value::Bool(false));
    x.set(d, "bClientHasAuthority", Value::Bool(true));
    assert_eq!(x.call(PATHS[2], d, vec![smi.clone()]).unwrap(), Value::Bool(true));
    x.set(pawn, "Role", Value::Byte(SIMULATED));
    assert_eq!(x.call(PATHS[2], d, vec![smi]).unwrap(), Value::Bool(false), "a simulated proxy");
}

/// `IsPlayingClass`: false with no class or no current move; true for the current definition's class and a parent.
#[test]
fn is_playing_class_follows_the_current_move() {
    let mut x = fx(ParityMode::Off);
    let (_, component) = x.owner(AUTHORITY);
    let class = |x: &mut Fx, p: &str| Value::Class(Some(x.vm().find_class(p).unwrap()));
    let (toy, base, other) = (
        class(&mut x, "GearboxFramework.ToyMove"),
        class(&mut x, "GearboxFramework.SpecialMoveDefinition"),
        class(&mut x, "GearboxFramework.ToyOtherMove"),
    );
    assert_eq!(x.call(PATHS[3], component, vec![toy.clone()]).unwrap(), Value::Bool(false));
    let d = x.definition("GearboxFramework.ToyMove", false);
    x.seed(component, d, None);
    assert_eq!(x.call(PATHS[3], component, vec![toy]).unwrap(), Value::Bool(true));
    assert_eq!(x.call(PATHS[3], component, vec![base]).unwrap(), Value::Bool(true));
    assert_eq!(x.call(PATHS[3], component, vec![other]).unwrap(), Value::Bool(false));
    assert_eq!(x.call(PATHS[3], component, vec![Value::Class(None)]).unwrap(), Value::Bool(false));
}

/// `GetSMNode`: None with no owner; `NotYet { LayerAbsent }` with no mesh or no anim tree (the headless case); with a
/// tree, the node named `SpecialMoveBlend` of the blend class is found below the root, cached and reused; a tree
/// without one, or with a node of that name of another class, answers None.
#[test]
fn get_sm_node_finds_the_blend_node_and_needs_a_tree() {
    let mut x = fx(ParityMode::Off);
    let bare = x.obj("GearboxFramework.SpecialMoveComponent");
    assert_eq!(x.call(PATHS[4], bare, vec![]).unwrap(), Value::Object(None));
    let (pawn, component) = x.owner(AUTHORITY);
    let (missing, text) = not_yet(x.call(PATHS[4], component, vec![]));
    assert!(matches!(missing, Missing::LayerAbsent(_)), "{text}");
    assert!(text.contains("no anim tree"), "{text}");
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    x.set(pawn, "Mesh", Value::Object(Some(mesh)));
    let (missing, _) = not_yet(x.call(PATHS[4], component, vec![]));
    assert!(matches!(missing, Missing::LayerAbsent(_)), "a mesh with no Animations");
    // Root -> [Slot, Upper]; Upper -> [the blend node]. The names are invented except the one looked for.
    let root = x.obj("Engine.AnimNodeBlendBase");
    let slot = x.obj("Engine.AnimNode");
    let upper = x.obj("Engine.AnimNodeBlendBase");
    let n = x.name("Slot");
    x.set(slot, "NodeName", n);
    let child = |x: &mut Fx, anim: ObjRef| {
        let ty = {
            let vm = x.vm();
            let c = vm.space.heap.get(root).unwrap().class;
            let p = vm.space.prop(c, "Children").unwrap();
            match &vm.space.classes.decl(p).unwrap().kind {
                ow_object::layout::PropKind::Array(inner) => match **inner {
                    ow_object::layout::PropKind::Struct(s) => s,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        };
        let v = x.vm().space.classes.struct_defaults(ty).unwrap();
        with_field(&x.world.vm, v, "Anim", Value::Object(Some(anim)), "test").unwrap()
    };
    x.set(mesh, "Animations", Value::Object(Some(root)));
    let (a, b) = (child(&mut x, slot), child(&mut x, upper));
    x.set(root, "Children", Value::array(vec![a, b]));
    assert_eq!(x.call(PATHS[4], component, vec![]).unwrap(), Value::Object(None), "no node of that name");
    let wrong = x.obj("Engine.AnimNode");
    let n = x.name("SpecialMoveBlend");
    x.set(wrong, "NodeName", n.clone());
    let n = n.clone();
    let c = child(&mut x, wrong);
    x.set(upper, "Children", Value::array(vec![c]));
    assert_eq!(x.call(PATHS[4], component, vec![]).unwrap(), Value::Object(None), "a node of another class");
    let node = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
    x.set(node, "NodeName", n);
    x.set(node, "SkelComponent", Value::Object(Some(mesh)));
    let c = child(&mut x, node);
    x.set(upper, "Children", Value::array(vec![c]));
    assert_eq!(x.call(PATHS[4], component, vec![]).unwrap(), Value::Object(Some(node)), "found");
    assert_eq!(x.get(component, "SMBlendNode"), Value::Object(Some(node)), "cached");
    x.set(mesh, "Animations", Value::Object(None));
    assert_eq!(x.call(PATHS[4], component, vec![]).unwrap(), Value::Object(Some(node)), "the cache on this mesh");
    let other_mesh = x.obj("Engine.SkeletalMeshComponent");
    x.set(pawn, "Mesh", Value::Object(Some(other_mesh)));
    let (missing, _) = not_yet(x.call(PATHS[4], component, vec![]));
    assert!(matches!(missing, Missing::LayerAbsent(_)), "a cache on another mesh is not reused");
}

/// `Play`'s zero answers: no definition, no owner interface, a failed gate (nothing called past it, nothing changed).
#[test]
fn play_answers_zero_without_a_definition_an_owner_or_the_gates() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", true);
    let (_, component) = x.owner(AUTONOMOUS);
    assert_eq!(x.play(component, vec![Value::Object(None)]).unwrap(), Value::Float(0.0));
    let bare = x.obj("GearboxFramework.SpecialMoveComponent");
    assert_eq!(x.play(bare, vec![Value::Object(Some(d))]).unwrap(), Value::Float(0.0));
    x.script.borrow_mut().can_play = false;
    assert_eq!(x.play(component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(0.0));
    assert_eq!(x.calls(), vec!["AuthorityCanPlay on ToyMove(_)"]);
    let plain = x.definition("GearboxFramework.ToyMove", false);
    x.script.borrow_mut().can_play = true;
    assert_eq!(x.play(component, vec![Value::Object(Some(plain))]).unwrap(), Value::Float(0.0), "not the local authority");
    assert_eq!(x.calls().len(), 1, "AuthorityCanPlay is not asked when IsLocalAuthority fails");
    assert_eq!(x.current(component, "SMD"), Value::Object(None));
}

/// The start on an owner below authority in a standalone world: the request becomes the current move with the world
/// time, `ClientStarted` answers the duration; a second play stops the first as interrupted and calls its callback on
/// the owner with `bInterrupted`.
#[test]
fn play_starts_the_move_and_a_second_play_interrupts_the_first() {
    let mut x = fx(ParityMode::Off);
    let first = x.definition("GearboxFramework.ToyMove", true);
    let (_, component) = x.owner(AUTONOMOUS);
    let done = x.name("ToyDone");
    let got = x.play(component, vec![Value::Object(Some(first)), Value::Float(1.5), Value::Float(0.0), done]).unwrap();
    assert_eq!(got, Value::Float(2.5));
    assert_eq!(x.calls(), vec!["AuthorityCanPlay on ToyMove(_)", "ClientStarted on ToyMove(_,data)"]);
    assert_eq!(x.current(component, "SMD"), Value::Object(Some(first)));
    assert_eq!(x.get(component, "StartTime"), Value::Float(12.0));
    assert_eq!(x.current(component, "PlayRateScale"), Value::Float(1.5));
    let second = x.definition("GearboxFramework.ToyOtherMove", true);
    let wi = x.world_info;
    x.set(wi, "TimeSeconds", Value::Float(13.0));
    x.clear_calls();
    assert_eq!(x.play(component, vec![Value::Object(Some(second))]).unwrap(), Value::Float(2.5));
    assert_eq!(
        x.calls(),
        vec![
            "AuthorityCanPlay on ToyOtherMove(_)",
            "ClientFinished on ToyMove(_,true)",
            "ToyDone on Pawn(true)",
            "ClientStarted on ToyOtherMove(_,data)",
        ]
    );
    assert_eq!(x.current(component, "SMD"), Value::Object(Some(second)));
    assert_eq!(x.get(component, "StartTime"), Value::Float(13.0));
    assert_eq!(x.current(component, "PlayRateScale"), Value::Float(1.0), "the default rate");
}

/// A duration of 0 or less stops the move again, not interrupted; no callback was asked for.
#[test]
fn a_zero_duration_stops_the_move_again() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", true);
    let (_, component) = x.owner(AUTONOMOUS);
    x.script.borrow_mut().duration = 0.0;
    assert_eq!(x.play(component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(0.0));
    assert_eq!(
        x.calls(),
        vec!["AuthorityCanPlay on ToyMove(_)", "ClientStarted on ToyMove(_,data)", "ClientFinished on ToyMove(_,false)"]
    );
    assert_eq!(x.current(component, "SMD"), Value::Object(None));
}

/// Every refusal decided before the first event leaves a seeded component (a current move, a one-entry queue,
/// `StartTime` 5) as it was, and asks no gate event (reviews r1 and r2 of #437): the authority's replication record, a
/// network client's forward, follow-ups, an old callback the owner does not declare, and a callback with another
/// parameter (the old move's and the new move's own).
#[test]
fn play_refusals_change_nothing() {
    let mut x = fx(ParityMode::Off);
    let old = x.definition("GearboxFramework.ToyOtherMove", true);
    let d = x.definition("GearboxFramework.ToyMove", true);
    let run = |x: &mut Fx, role: u8, callback: Option<&str>, args: Vec<Value>| {
        let (_, component) = x.owner(role);
        x.seed(component, old, callback);
        let before = x.state(component);
        x.clear_calls();
        let r = x.play(component, args);
        let after = x.state(component);
        assert_eq!(before, after, "nothing changed");
        assert_eq!(x.calls(), Vec::<String>::new(), "no event ran");
        not_yet(r)
    };
    let (missing, text) = run(&mut x, AUTHORITY, None, vec![Value::Object(Some(d))]);
    assert_eq!(missing, Missing::NotRead, "{text}");
    assert!(text.contains("replication"), "{text}");
    let wi = x.world_info;
    x.set(wi, "NetMode", Value::Byte(3));
    let (missing, _) = run(&mut x, AUTONOMOUS, None, vec![Value::Object(Some(d))]);
    assert_eq!(missing, Missing::NotPorted);
    x.set(wi, "NetMode", Value::Byte(0));
    let (_, text) = run(&mut x, AUTONOMOUS, Some("NoSuchCallback"), vec![Value::Object(Some(d))]);
    assert!(text.contains("does not declare"), "{text}");
    let (_, text) = run(&mut x, AUTONOMOUS, Some("ToyDoneWithCount"), vec![Value::Object(Some(d))]);
    assert!(text.contains("bInterrupted"), "{text}");
    let counted = x.name("ToyDoneWithCount");
    let (_, text) = run(&mut x, AUTONOMOUS, None, vec![Value::Object(Some(d)), Value::Float(1.0), Value::Float(0.0), counted]);
    assert!(text.contains("ToyDoneWithCount"), "the new move's own callback: {text}");
    let next = x.definition("GearboxFramework.ToyOtherMove", true);
    x.set(d, "NextSpecialMove", Value::Object(Some(next)));
    let (_, text) = run(&mut x, AUTONOMOUS, None, vec![Value::Object(Some(d))]);
    assert!(text.contains("NextSpecialMove"), "{text}");
}

/// `PlayLocal`: no gate and no current move; `ServerStarted` then `ClientStarted`, whose answer is the answer.
#[test]
fn play_local_runs_both_starts_and_keeps_no_current_move() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", false);
    let (_, component) = x.owner(AUTHORITY);
    x.script.borrow_mut().can_play = false;
    assert_eq!(x.call(PATHS[6], component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(2.5));
    assert_eq!(x.calls(), vec!["ServerStarted on ToyMove(_)", "ClientStarted on ToyMove(_,data)"]);
    assert_eq!(x.current(component, "SMD"), Value::Object(None));
    assert_eq!(x.call(PATHS[6], component, vec![Value::Object(None)]).unwrap(), Value::Float(0.0));
}

/// The toy `GetPlayerAnimation` script (the cast's choice) through the action skill pack's one cast path (review r2 of
/// #437): in Off `Pawn.IsFriendly` is NATIVE_IS_FRIENDLY.md's rule (lane IMPL-IS-FRIENDLY), never the combat
/// allegiance boolean, and this fixture's targets are plain pawns, not `WillowPawn`s, whose allegiance the note does
/// not read: its `NotYet`. In Reference the stand-in answers, so a friend gets the fizzle move, and an enemy reaches
/// `IsAliveAndWell`, which Reference leaves unbound. (The rule on `WillowPawn`s is `ow-combat`'s `hit::opinion` tests.)
#[test]
fn the_cast_script_asks_is_friendly_through_the_pack() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut x = fx(mode);
        let skill = x.obj("Engine.ToyLiftSkill");
        let (hit, miss) =
            (x.definition("GearboxFramework.ToyMove", true), x.definition("GearboxFramework.ToyOtherMove", true));
        let (caster, _) = x.owner(AUTHORITY);
        x.set(skill, "Caster", Value::Object(Some(caster)));
        x.set(skill, "Hit", Value::Object(Some(hit)));
        x.set(skill, "Miss", Value::Object(Some(miss)));
        let side = x.obj("Engine.PawnAllegiance");
        x.set(side, "SelfOpinion", Value::Byte(2));
        x.set(caster, "Allegiance", Value::Object(Some(side)));
        let (friend, _) = x.owner(AUTHORITY);
        x.set(friend, "Allegiance", Value::Object(Some(side)));
        let (enemy, _) = x.owner(AUTHORITY);
        let choose = |x: &mut Fx, target: ObjRef| {
            x.call("Engine.ToyLiftSkill.GetPlayerAnimation", skill, vec![Value::Object(Some(target))])
        };
        if mode == ParityMode::Off {
            for target in [friend, enemy] {
                match choose(&mut x, target).unwrap_err() {
                    VmError::NotYet(n) => {
                        assert_eq!(n.missing, Missing::NotRead);
                        assert_eq!(n.record, crate::action_skill::RECORD_IS_FRIENDLY);
                    }
                    e => panic!("expected the IsFriendly NotYet, got {e}"),
                }
            }
        } else {
            assert_eq!(choose(&mut x, friend).unwrap(), Value::Object(Some(miss)), "a friend: fizzle");
            assert!(choose(&mut x, enemy).is_err(), "an enemy: IsAliveAndWell is unbound in Reference");
        }
    }
}

/// The node `GetSMNode` finds while `AuthorityCanPlay` runs is cached only once `Play` is committed (review r2 of
/// #437): not when the gate fails, and not when `Play` refuses on the authority, which it does before asking the gate.
#[test]
fn the_node_cache_is_written_only_by_a_committed_play() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", true);
    let tree_owner = |x: &mut Fx, role: u8| {
        let (pawn, component) = x.owner(role);
        let mesh = x.obj("Engine.SkeletalMeshComponent");
        x.set(pawn, "Mesh", Value::Object(Some(mesh)));
        let node = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
        let n = x.name("SpecialMoveBlend");
        x.set(node, "NodeName", n);
        x.set(mesh, "Animations", Value::Object(Some(node)));
        x.script.borrow_mut().probe = Some(component);
        (component, node)
    };
    let (component, _) = tree_owner(&mut x, AUTONOMOUS);
    x.script.borrow_mut().can_play = false;
    assert_eq!(x.play(component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(0.0));
    assert_eq!(x.get(component, "SMBlendNode"), Value::Object(None), "the gate failed: no cache");
    x.script.borrow_mut().can_play = true;
    let (component, _) = tree_owner(&mut x, AUTHORITY);
    x.clear_calls();
    not_yet(x.play(component, vec![Value::Object(Some(d))]));
    assert_eq!(x.calls(), Vec::<String>::new(), "the authority is refused before the gate");
    assert_eq!(x.get(component, "SMBlendNode"), Value::Object(None));
    let (component, node) = tree_owner(&mut x, AUTONOMOUS);
    assert_eq!(x.play(component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(2.5));
    assert_eq!(x.get(component, "SMBlendNode"), Value::Object(Some(node)), "committed: cached");
    // Outside Play, GetSMNode caches at once (section 3).
    let (component, node) = tree_owner(&mut x, AUTONOMOUS);
    assert_eq!(x.call(PATHS[4], component, vec![]).unwrap(), Value::Object(Some(node)));
    assert_eq!(x.get(component, "SMBlendNode"), Value::Object(Some(node)));
}

/// A refusal after the commit is terminal (review r2 of #437, the rule of #439): nothing is put back, and its text
/// starts "stopped after partial writes", the refusal's kind kept.
#[test]
fn a_refusal_after_the_commit_is_terminal() {
    let mut x = fx(ParityMode::Off);
    let old = x.definition("GearboxFramework.ToyOtherMove", true);
    let d = x.definition("GearboxFramework.ToyMove", true);
    let (_, component) = x.owner(AUTONOMOUS);
    x.seed(component, old, Some("ToyDone"));
    x.script.borrow_mut().fail_start = true;
    let (missing, text) = not_yet(x.play(component, vec![Value::Object(Some(d))]));
    assert_eq!(missing, Missing::NotRead);
    assert!(text.starts_with("stopped after partial writes: a toy unbound native"), "{text}");
    assert_eq!(x.current(component, "SMD"), Value::Object(Some(d)), "the new move stays: nothing is put back");
    assert_eq!(x.get(component, "StartTime"), Value::Float(12.0));
    assert_eq!(x.get(component, "SMDQueue"), Value::array(vec![]));
    assert!(x.calls().contains(&"ToyDone on Pawn(true)".to_string()), "the old move's stop ran: {:?}", x.calls());
}

/// The callback's authority is asked again when it is dispatched: a `ClientFinished` that demotes the owner below
/// the move's authority leaves the callback uncalled (review r2 of #437).
#[test]
fn the_callback_rechecks_authority_at_dispatch() {
    let mut x = fx(ParityMode::Off);
    let old = x.definition("GearboxFramework.ToyOtherMove", true);
    let d = x.definition("GearboxFramework.ToyMove", true);
    let (pawn, component) = x.owner(AUTONOMOUS);
    x.seed(component, old, Some("ToyDone"));
    x.script.borrow_mut().demote = Some(pawn);
    assert_eq!(x.play(component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(2.5));
    assert!(x.calls().iter().all(|c| !c.starts_with("ToyDone")), "{:?}", x.calls());
}

/// A zero duration stops the move only while it is still the current one: when `ClientStarted` already ended it,
/// `ClientFinished` does not run a second time (review r2 of #437).
#[test]
fn a_zero_duration_does_not_stop_a_move_that_already_ended() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", true);
    let (_, component) = x.owner(AUTONOMOUS);
    x.script.borrow_mut().duration = 0.0;
    x.script.borrow_mut().end_on_start = Some(component);
    assert_eq!(x.play(component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(0.0));
    assert_eq!(x.calls(), vec!["AuthorityCanPlay on ToyMove(_)", "ClientStarted on ToyMove(_,data)"]);
}

/// Names are compared without case: on a fresh fixture the lower-case spelling is interned first, so the node's
/// name text really is lower case (review r2 of #437).
#[test]
fn get_sm_node_compares_names_without_case() {
    let mut x = fx(ParityMode::Off);
    let lower = x.name("specialmoveblend");
    let Value::Name(id) = lower else { unreachable!() };
    assert_eq!(x.vm().space.names.text(id), "specialmoveblend", "the first spelling is kept");
    let (pawn, component) = x.owner(AUTONOMOUS);
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    x.set(pawn, "Mesh", Value::Object(Some(mesh)));
    let node = x.obj("GearboxFramework.AnimNodeSpecialMoveBlend");
    x.set(node, "NodeName", lower);
    x.set(mesh, "Animations", Value::Object(Some(node)));
    assert_eq!(x.call(PATHS[4], component, vec![]).unwrap(), Value::Object(Some(node)));
}

/// Reviews r1 and r2 of #437: `ow-combat`'s hit pack binds its `Play` no-op and its `IsPlayingClass` and
/// `IsPlayingAny` false in Reference only, so in Off this module's natives answer whichever pack registers first and
/// `IsPlayingAny` stays unbound; in Reference the stand-ins answer and this module binds nothing.
#[test]
fn the_combat_stand_ins_never_shadow_off_in_either_order() {
    let combat = ow_combat::hit::Pack;
    let skills = crate::action_skill::Pack;
    let orders: [[&dyn NativePack; 2]; 2] = [[&combat, &skills], [&skills, &combat]];
    for (i, packs) in orders.iter().enumerate() {
        let mut x = fx_with(ParityMode::Off, packs);
        let (_, component) = x.owner(AUTONOMOUS);
        let d = x.definition("GearboxFramework.ToyMove", true);
        x.seed(component, d, None);
        let toy = Value::Class(Some(x.vm().find_class("GearboxFramework.ToyMove").unwrap()));
        assert_eq!(x.call(PATHS[3], component, vec![toy]).unwrap(), Value::Bool(true), "order {i}: IsPlayingClass");
        assert_eq!(x.play(component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(2.5), "order {i}: Play");
        let f = x.vm().find_function("GearboxFramework.SpecialMoveComponent.IsPlayingAny").unwrap();
        let key = x.vm().function(f).unwrap().native_key.clone().unwrap();
        assert!(!x.vm().has_native(&key), "order {i}: IsPlayingAny is unbound in Off");
        let mut x = fx_with(ParityMode::Reference, packs);
        let (_, component) = x.owner(AUTONOMOUS);
        let d = x.definition("GearboxFramework.ToyMove", true);
        x.seed(component, d, None);
        let toy = Value::Class(Some(x.vm().find_class("GearboxFramework.ToyMove").unwrap()));
        assert_eq!(x.call(PATHS[3], component, vec![toy]).unwrap(), Value::Bool(false), "order {i}: the stand-in");
        assert_eq!(x.play(component, vec![Value::Object(Some(d))]).unwrap(), Value::None, "order {i}: the no-op");
        let any = x.call("GearboxFramework.SpecialMoveComponent.IsPlayingAny", component, vec![]).unwrap();
        assert_eq!(any, Value::Bool(false), "order {i}: the IsPlayingAny stand-in");
    }
}

/// Whatever order a world registers the combat and action skill packs in, `Pawn.IsFriendly` is one binding (lane
/// IMPL-IS-FRIENDLY): in Off the note's rule, which does not read a controller through its pawn (Neutral, so false);
/// in Reference the groundwork stand-in, which does (the controller's pawn shares the caster's allegiance: true).
#[test]
fn is_friendly_is_one_binding_in_either_pack_order() {
    let combat = ow_combat::hit::Pack;
    let skills = crate::action_skill::Pack;
    let orders: [[&dyn NativePack; 2]; 2] = [[&combat, &skills], [&skills, &combat]];
    for (i, packs) in orders.iter().enumerate() {
        for mode in [ParityMode::Off, ParityMode::Reference] {
            let mut x = fx_with(mode, packs);
            let side = x.obj("Engine.PawnAllegiance");
            x.set(side, "SelfOpinion", Value::Byte(2));
            let (caster, _) = x.owner(AUTHORITY);
            let (ally, _) = x.owner(AUTHORITY);
            x.set(caster, "Allegiance", Value::Object(Some(side)));
            x.set(ally, "Allegiance", Value::Object(Some(side)));
            let controller = x.obj("Engine.Controller");
            x.set(controller, "Pawn", Value::Object(Some(ally)));
            let r = x.call("Engine.Pawn.IsFriendly", caster, vec![Value::Object(Some(controller))]).unwrap();
            assert_eq!(r, Value::Bool(mode == ParityMode::Reference), "order {i}, {mode:?}");
        }
    }
}

/// Reference binds none of them (no C++ stand-in; only the allegiance natives the fixture adds are bound).
#[test]
fn reference_binds_nothing() {
    let mut x = fx(ParityMode::Reference);
    for path in PATHS {
        let vm = x.vm();
        let f = vm.find_function(path).unwrap();
        let key = vm.function(f).unwrap().native_key.clone().unwrap();
        assert!(!vm.has_native(&key), "{path}");
    }
}

/// The action skill pack alone binds no groundwork stand-in in Off (review r2 of #449): `IsEnemy` stays unbound
/// there, and Reference binds it with the combat pack's allegiance natives.
#[test]
fn the_cast_pack_binds_no_groundwork_test_in_off() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut x = fx(mode);
        let f = x.vm().find_function("Engine.Pawn.IsEnemy").unwrap();
        let key = x.vm().function(f).unwrap().native_key.clone().unwrap();
        assert_eq!(x.vm().has_native(&key), mode == ParityMode::Reference, "{mode:?}");
    }
}
