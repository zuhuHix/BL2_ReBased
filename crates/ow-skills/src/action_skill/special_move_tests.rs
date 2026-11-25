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
use ow_package::testkit::reflect::{ClassBodySpec, class_body, toy_array_of, toy_struct};
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

/// How the record's declarations are built (lane IMPL-SET-REPLICATED-SMDATA, the note's test 7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Variant {
    Good,
    /// `ReplicatedSMData` of another struct (same fields, declared in `GearboxPawn`).
    OtherStruct,
    /// No `Duration` field.
    MissingField,
    /// `DirtyCounter` an int.
    IntCounter,
    /// `bNetDirty` an int.
    IntNetDirty,
    /// `SMD` an int.
    SmdNotObject,
    /// `PlayRateScale` an int.
    RateNotFloat,
    /// The definition class declares no `ServerStarted`.
    NoServerStarted,
    /// The record's struct also has `CallbackName` and `CallbackObject` fields (review r1 of #689, item 5).
    CallbackFields,
}

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
fn engine(variant: Variant) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let (friendly, alive) = (t.p.fname("IsFriendly", 0), t.p.fname("IsAliveAndWell", 0));
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let roles = enumeration(&mut t, actor, "ENetRole", &ROLES);
    t.prop("Byte", actor, "Role", 0, roles);
    t.prop("Bool", actor, "bDeleteMe", 0, 0);
    t.prop(if variant == Variant::IntNetDirty { "Int" } else { "Bool" }, actor, "bNetDirty", 0, 0);
    // Actor.Trace, which both binding sites bind in Off (lane IMPL-ACTOR-TRACE); no parameters are needed to tell
    // the shared function from the old refusal.
    native(&mut t, actor, "Trace", &[], Some(("Object", vec![actor])));
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
    // AttributeDefinition.GetValue, which the cast's pack binds in Off (lane IMPL-LINK9-ATTR-GETVALUE).
    let attribute = t.class("AttributeDefinition", object);
    native(&mut t, attribute, "GetValue", &[D("Object", "ContextSource", 0, vec![object])], Some(("Float", vec![0])));
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
fn gearbox(variant: Variant) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let blend_base = t.p.add_import("Core", "Class", engine, "AnimNodeBlendBase");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
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
    // The component's Owner, which Queue's forward test reads (lane IMPL-SPECIALMOVE-QUEUE).
    t.prop("Object", component, "Owner", 0, actor);
    // Two misshapen components for Queue's shape checks: `CurrentSMData` an int, and `SMDQueue` of ints.
    let odd_current = t.class("ToyOddCurrentComponent", object);
    t.prop("Interface", odd_current, "SMI", 0, interface);
    t.prop("Object", odd_current, "Owner", 0, actor);
    t.prop("Int", odd_current, "CurrentSMData", 0, 0);
    toy_array_of(&mut t, odd_current, "SMDQueue", "Struct", data);
    let odd_queue = t.class("ToyOddQueueComponent", object);
    t.prop("Interface", odd_queue, "SMI", 0, interface);
    t.prop("Object", odd_queue, "Owner", 0, actor);
    t.prop("Struct", odd_queue, "CurrentSMData", 0, data);
    toy_array_of(&mut t, odd_queue, "SMDQueue", "Int", 0);
    let smi = || D("Interface", "SMI", 0, vec![interface]);
    {
        let declared = [
            D("Object", "NewSMD", 0, vec![object]),
            D("Float", "PlayRateScale", CPF_OPT, vec![0]),
            D("Float", "Duration", CPF_OPT, vec![0]),
            D("Name", "CallbackName", CPF_OPT, vec![0]),
            D("Object", "CallbackObject", CPF_OPT, vec![object]),
            D("Object", "Data", CPF_OPT, vec![object]),
        ];
        native(&mut t, component, "Queue", &declared, None);
    }
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
    if variant != Variant::NoServerStarted {
        native(&mut t, definition, "ServerStarted", &[smi()], None);
    }
    native(&mut t, definition, "ServerFinished", &[smi(), D("Bool", "bInterrupted", 0, vec![0])], None);
    native(&mut t, definition, "ClientStarted", &[smi(), D("Struct", "SMData", 0, vec![data])], Some(("Float", vec![0])));
    native(&mut t, definition, "ClientFinished", &[smi(), D("Bool", "bInterrupted", 0, vec![0])], None);
    let toy_move = t.class("ToyMove", definition);
    // COLD-CLASS-AUDIT: a blend class no fixture call warms (`find_class` loads code; it gets a layout only).
    // `ToyColdMove` is declared below (#682's).
    t.class("ToyColdBlend", blend_base);
    t.class("ToyOtherMove", definition);
    // Queue's residency tests (review r1 of #682): `ToyHolder`, loaded, whose `Move` and `Actor` name two exports the
    // store leaves as lazy handles, a `ToyMove` and an `Engine.Pawn`.
    let bare = |t: &mut ScriptToy| {
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend_from_slice(&t.none);
        payload
    };
    let payload = bare(&mut t);
    let lazy_move = t.p.add_export(toy_move, "ToyLazyMove", payload);
    let payload = bare(&mut t);
    let lazy_pawn = t.p.add_export(pawn, "ToyLazyPawn", payload);
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Move", 0, definition);
    t.prop("Object", holder, "Actor", 0, actor);
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(t.p.object_tag("Move", lazy_move));
    payload.extend(t.p.object_tag("Actor", lazy_pawn));
    payload.extend_from_slice(&t.none);
    t.p.add_export(holder, "Holder", payload);
    // Review r2 of #682: `ToyColdMove`, a definition class nothing in the fixture warms, and one loaded object of it.
    let cold = t.class("ToyColdMove", definition);
    let payload = bare(&mut t);
    t.p.add_export(cold, "ToyColdMoveObject", payload);
    // A definition whose own GetSMDToPlay a test binds to answer another definition (review r1 of #689, item 1).
    let redirect = t.class("ToyRedirectMove", definition);
    native(&mut t, redirect, "GetSMDToPlay", &[smi()], Some(("Object", vec![definition])));
    t.class("AnimNodeSpecialMoveBlend", blend_base);
    // GearboxPawn and its replicated record (the note, section 1), of the struct `SpecialMoveInterface` declares (the
    // installed path, by the runner's read). `OtherStruct` declares a struct of the same name and fields in
    // `GearboxPawn` instead.
    let gearbox_pawn = t.class("GearboxPawn", pawn);
    let holder = if variant == Variant::OtherStruct { gearbox_pawn } else { interface };
    let rep = toy_struct(&mut t, holder, "ReplicatedSpecialMoveData", None);
    // The installed member order (the runner's read): Data, PlayRateScale, Duration, DirtyCounter, SMD.
    t.prop("Object", rep, "Data", 0, object);
    t.prop(if variant == Variant::RateNotFloat { "Int" } else { "Float" }, rep, "PlayRateScale", 0, 0);
    if variant != Variant::MissingField {
        t.prop("Float", rep, "Duration", 0, 0);
    }
    t.prop(if variant == Variant::IntCounter { "Int" } else { "Byte" }, rep, "DirtyCounter", 0, 0);
    if variant == Variant::SmdNotObject {
        t.prop("Int", rep, "SMD", 0, 0);
    } else {
        t.prop("Object", rep, "SMD", 0, definition);
    }
    if variant == Variant::CallbackFields {
        t.prop("Name", rep, "CallbackName", 0, 0);
        t.prop("Object", rep, "CallbackObject", 0, object);
    }
    t.prop("Struct", gearbox_pawn, "ReplicatedSMData", 0, rep);
    // A native class below GearboxPawn whose record was not read (the note's test 6).
    t.class("ToyNativePawn", gearbox_pawn);
    // A definition export and a pawn export the store leaves as handles, held by a holder export.
    let holder = t.class("ToyMoveHolder", object);
    t.prop("Object", holder, "Move", 0, definition);
    t.prop("Object", holder, "Pawn", 0, gearbox_pawn);
    let mut empty = ints(&[0]);
    empty.extend_from_slice(&t.none);
    let stored = t.p.add_export(toy_move, "ToyStoredMove", empty.clone());
    let stored_pawn = t.p.add_export(gearbox_pawn, "ToyStoredPawn", empty);
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Move", stored));
    h.extend(t.p.object_tag("Pawn", stored_pawn));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyMoveHolderObject", h);
    t.build()
}

/// `WillowGame`: `WillowPawn`, `WillowPlayerPawn` and `WillowAIPawn` (each a native class: their toy bodies do not
/// decode, which the native walk counts as native), and `ToyMayaPawn`, a script-only subclass of `WillowPlayerPawn`
/// with a decodable, non-native class body (the note's test 1).
fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let gbx = t.p.add_import("Core", "Package", 0, "GearboxFramework");
    let gearbox_pawn = t.p.add_import("Core", "Class", gbx, "GearboxPawn");
    let willow_pawn = t.class("WillowPawn", gearbox_pawn);
    let player = t.class("WillowPlayerPawn", willow_pawn);
    t.class("WillowAIPawn", willow_pawn);
    let at = t.p.next_export();
    let body = class_body(&mut t.p, &ClassBodySpec { flags: 2, ..ClassBodySpec::new(player, 0, at + 1) });
    let mut export = Export::new(0, "ToyMayaPawn", body);
    export.super_ = player;
    assert_eq!(t.p.add_export_with(export), at);
    let mut default = ints(&[0]);
    default.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(at, "Default__ToyMayaPawn", default));
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
    /// Each event first records this pawn's replicated record as "rec(counter,smd,rate)" (lane
    /// IMPL-SET-REPLICATED-SMDATA): the order of the record write against the server events.
    spy: Option<ObjRef>,
    /// Each event also records this component's current move as "cur(smd,rate)".
    spy_component: Option<ObjRef>,
    /// `ServerStarted` fails (an unbound native it reaches, say).
    fail_server_start: bool,
}

/// The recorded events and callbacks: function path and what each answers.
const EVENTS: [&str; 7] = [
    "GearboxFramework.SpecialMoveDefinition.AuthorityCanPlay",
    "GearboxFramework.SpecialMoveDefinition.ServerStarted",
    "GearboxFramework.SpecialMoveDefinition.ServerFinished",
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
    fx_variant(mode, packs, Variant::Good)
}

/// [`fx_with`] over the record's declaration `variant`, with the `WillowGame` package.
fn fx_variant(mode: ParityMode, packs: &[&dyn NativePack], variant: Variant) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-skills-sm-play-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g, w) = (engine(variant), gearbox(variant), willow());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice()), ("WillowGame.upk", w.as_slice())],
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
        // The NoServerStarted variant declares no ServerStarted.
        let Ok(f) = world.vm.find_function(path) else { continue };
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
                    let (probe, demote, end, spy, spy_component) = {
                        let s = s.borrow();
                        (s.probe, s.demote, s.end_on_start, s.spy, s.spy_component)
                    };
                    if let Some(pawn) = spy {
                        let seen = record_text(c.vm, pawn);
                        s.borrow_mut().calls.push(seen);
                    }
                    if let Some(component) = spy_component {
                        let seen = current_text(c.vm, component);
                        s.borrow_mut().calls.push(seen);
                    }
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
                        "ServerStarted" if s.fail_server_start => {
                            Err(VmError::NotYet(ow_core::NotYet::not_read("a toy unbound native in OnServerStart", "test")))
                        }
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
        self.set(component, "Owner", Value::Object(Some(pawn)));
        (pawn, component)
    }

    /// [`Fx::owner`] with a pawn of `class`.
    fn owner_of(&mut self, class: &str, role: u8) -> (ObjRef, ObjRef) {
        let pawn = self.obj(class);
        self.set(pawn, "Role", Value::Byte(role));
        let component = self.obj("GearboxFramework.SpecialMoveComponent");
        self.set(component, "SMI", Value::Interface(Some(pawn)));
        self.set(component, "Owner", Value::Object(Some(pawn)));
        (pawn, component)
    }

    /// The pawn's record field `f`.
    fn record(&mut self, pawn: ObjRef, f: &str) -> Value {
        let rep = self.get(pawn, "ReplicatedSMData");
        field(&self.world.vm, &rep, f, "test").unwrap()
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
    // Lane IMPL-SET-REPLICATED-SMDATA: the authority's record of a pawn class whose record was not read.
    let (missing, text) = run(&mut x, AUTHORITY, None, vec![Value::Object(Some(d))]);
    assert_eq!(missing, Missing::NotRead, "{text}");
    assert!(text.contains("the replicated record of Engine.Pawn"), "{text}");
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

/// The cast's pack binds `AttributeDefinition.GetValue` in Off (lane IMPL-LINK9-ATTR-GETVALUE): the combat pack's one
/// Off binding (decision 2026-11-19-58), since the cast's worlds install no combat pack. Under Reference it stays
/// unbound.
#[test]
fn the_cast_pack_binds_get_value_in_off_only() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut x = fx(mode);
        let vm = &mut x.world.vm;
        let id = vm.find_function("Engine.AttributeDefinition.GetValue").unwrap();
        let key = vm.function(id).and_then(|d| d.native_key.clone()).expect("a native");
        assert_eq!(vm.has_native(&key), mode == ParityMode::Off, "{mode:?}");
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

/// NATIVE_ACTOR_TRACE.md test 9 (lane IMPL-ACTOR-TRACE): `Actor.Trace` gets the one shared function in Off whatever
/// the pack set (the Phaselock set's action skill pack alone, or with the combat pack before or after it), never the
/// go-down worlds' old layer-absent refusal. Called with no receiver, the shared function answers a script error,
/// where the old refusal answered `NotYet`. Under Reference neither site binds it.
#[test]
fn actor_trace_is_the_shared_function_in_every_pack_order() {
    let combat = ow_combat::hit::Pack;
    let skills = crate::action_skill::Pack;
    let sets: [&[&dyn NativePack]; 3] = [&[&skills], &[&combat, &skills], &[&skills, &combat]];
    for (i, packs) in sets.iter().enumerate() {
        let mut x = fx_with(ParityMode::Off, packs);
        let vm = x.vm();
        let f = vm.find_function(ow_world::world::actor_trace::PATH).unwrap();
        match vm.call(f, None, vec![]) {
            Err(VmError::Script { function, .. }) => assert_eq!(function, "Actor.Trace", "set {i}"),
            other => panic!("set {i}: expected the shared function's script error, got {other:?}"),
        }
        let mut x = fx_with(ParityMode::Reference, packs);
        let vm = x.vm();
        let f = vm.find_function(ow_world::world::actor_trace::PATH).unwrap();
        let key = vm.function(f).unwrap().native_key.clone().unwrap();
        assert!(!vm.has_native(&key), "set {i}: Reference binds nothing");
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

// ---- SpecialMoveComponent.Queue (NATIVE_SPECIAL_MOVE_QUEUE.md section 6; lane IMPL-SPECIALMOVE-QUEUE) ------------

impl Fx {
    fn queue(&mut self, component: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        self.call(PATHS[7], component, args)
    }

    /// #518's three counters and the name table's length.
    fn counters(&mut self) -> (u64, u64, u64, usize) {
        let vm = self.vm();
        (vm.space.heap.committed_writes() as u64, vm.rng.draws() as u64, vm.defs.load_transitions() as u64, vm.space.names.len())
    }

    /// The six arguments, every one given: rate 0.5, duration 3, callback `ToyDone` on `callback`, data `data`.
    fn six(&mut self, smd: ObjRef, callback: ObjRef, data: ObjRef) -> Vec<Value> {
        let name = self.name("ToyDone");
        vec![
            Value::Object(Some(smd)),
            Value::Float(0.5),
            Value::Float(3.0),
            name,
            Value::Object(Some(callback)),
            Value::Object(Some(data)),
        ]
    }

    fn queue_entries(&mut self, component: ObjRef) -> Vec<Value> {
        self.get(component, "SMDQueue").elements().unwrap().to_vec()
    }
}

/// A refusal of `Queue`: its kind and text, the component's state, #518's counters and the name table unchanged, and
/// no event.
fn queue_refused(x: &mut Fx, component: ObjRef, args: Vec<Value>) -> (Missing, String) {
    let (state, counters) = (x.state(component), x.counters());
    x.clear_calls();
    let r = not_yet(x.queue(component, args));
    assert_eq!(x.state(component), state, "nothing written");
    assert_eq!(x.counters(), counters, "write-, draw-, load- and intern-free");
    assert!(x.calls().is_empty(), "no event");
    assert!(!r.1.contains("stopped after partial writes"), "{}", r.1);
    r
}

/// Test 1: no definition, or an owner interface unset: nothing changes, no refusal, no event.
#[test]
fn queue_with_nothing_to_queue_does_nothing() {
    let mut x = fx(ParityMode::Off);
    let (pawn, component) = x.owner(AUTHORITY);
    let d = x.definition("GearboxFramework.ToyMove", false);
    x.seed(component, d, None);
    let args = x.six(d, pawn, pawn);
    let (state, counters) = (x.state(component), x.counters());
    assert_eq!(x.queue(component, vec![Value::Object(None)]).unwrap(), Value::None);
    assert_eq!(x.counters(), counters, "no definition: nothing written");
    x.set(component, "SMI", Value::Interface(None));
    let counters = x.counters();
    assert_eq!(x.queue(component, args).unwrap(), Value::None);
    assert_eq!(x.counters(), counters, "no owner interface: nothing written");
    assert_eq!(x.state(component), state);
    assert!(x.calls().is_empty());
}

/// Test 2: an owner interface and no component `Owner`: the `Owner` refusal, nothing written.
#[test]
fn queue_without_an_owner_refuses() {
    let mut x = fx(ParityMode::Off);
    let (pawn, component) = x.owner(AUTHORITY);
    x.set(component, "Owner", Value::Object(None));
    let d = x.definition("GearboxFramework.ToyMove", false);
    let args = x.six(d, pawn, pawn);
    let (missing, what) = queue_refused(&mut x, component, args);
    assert_eq!(missing, Missing::NotRead);
    assert!(what.contains(QUEUE_NO_OWNER), "{what}");
}

/// Tests 3 and 4: an owner below authority is the forward's refusal, with or without a current move; an autonomous
/// proxy with a definition that sets `bClientHasAuthority` gets it too (preflight step 3 before step 4), nothing queued.
#[test]
fn queue_on_an_owner_below_authority_refuses_the_forward_first() {
    for (role, client_authority, current) in [(SIMULATED, false, false), (SIMULATED, false, true), (AUTONOMOUS, true, true)] {
        let mut x = fx(ParityMode::Off);
        let (pawn, component) = x.owner(role);
        let d = x.definition("GearboxFramework.ToyMove", client_authority);
        if current {
            let other = x.definition("GearboxFramework.ToyOtherMove", false);
            x.seed(component, other, None);
        }
        let args = x.six(d, pawn, pawn);
        let (missing, what) = queue_refused(&mut x, component, args);
        assert_eq!(missing, Missing::NotRead, "{role} {client_authority} {current}");
        assert!(what.contains(QUEUE_FORWARD), "{what}");
    }
}

/// Test 5: behind a current move, on the authority: one entry appended with all six fields as given (`Data`
/// included, the definition as given); a second `Queue` of the same definition appends a second entry after it. The
/// current move, `StartTime` and the existing entry are unchanged, and no definition event runs.
#[test]
fn queue_behind_a_current_move_appends_full_entries_in_order() {
    let mut x = fx(ParityMode::Off);
    let (pawn, component) = x.owner(AUTHORITY);
    let current = x.definition("GearboxFramework.ToyOtherMove", false);
    x.seed(component, current, None);
    let (before_current, before_queue, before_time) = x.state(component);
    let existing = before_queue.elements().unwrap()[0].clone();
    let d = x.definition("GearboxFramework.ToyMove", false);
    let (callback, data) = (x.obj("Engine.Pawn"), x.obj("Engine.Actor"));
    x.clear_calls();
    let args = x.six(d, callback, data);
    assert_eq!(x.queue(component, args).unwrap(), Value::None);
    let data2 = x.obj("Engine.Actor");
    let mut args = x.six(d, callback, data2);
    args.truncate(2);
    x.queue(component, args).unwrap();
    let entries = x.queue_entries(component);
    assert_eq!(entries.len(), 3, "the existing entry, then the two queued");
    assert_eq!(entries[0], existing);
    let vm = &x.world.vm;
    let f = |e: &Value, n: &str| field(vm, e, n, "test").unwrap();
    assert_eq!(f(&entries[1], "SMD"), Value::Object(Some(d)));
    assert_eq!(f(&entries[1], "PlayRateScale"), Value::Float(0.5));
    assert_eq!(f(&entries[1], "Duration"), Value::Float(3.0));
    assert_eq!(f(&entries[1], "CallbackName"), Value::Name(vm.space.names.find("ToyDone").unwrap()));
    assert_eq!(f(&entries[1], "CallbackObject"), Value::Object(Some(callback)));
    assert_eq!(f(&entries[1], "Data"), Value::Object(Some(data)), "Data kept behind a current move");
    // The second, its omitted arguments at the defaults.
    assert_eq!(f(&entries[2], "SMD"), Value::Object(Some(d)), "the same definition twice");
    assert_eq!(f(&entries[2], "PlayRateScale"), Value::Float(0.5));
    assert_eq!(f(&entries[2], "Duration"), Value::Float(0.0));
    assert_eq!(f(&entries[2], "Data"), Value::Object(None));
    let _ = pawn;
    assert_eq!(x.get(component, "CurrentSMData"), before_current);
    assert_eq!(x.get(component, "StartTime"), before_time);
    assert!(x.calls().is_empty(), "no event: {:?}", x.calls());
}

/// Test 6, the integration: nothing current, on the authority. Play's path runs, and its own refusal stands, with no
/// "stopped after partial writes" prefix and the queue unchanged. Since lane IMPL-SET-REPLICATED-SMDATA, Play on the
/// authority checks the replicated record, and the fixture's plain `Engine.Pawn` owner is a class whose record was not
/// read: that `NotRead`.
#[test]
fn queue_with_nothing_current_on_the_authority_meets_play_s_refusal() {
    let mut x = fx(ParityMode::Off);
    let (pawn, component) = x.owner(AUTHORITY);
    let waiting = x.definition("GearboxFramework.ToyOtherMove", false);
    let mut entry = x.get(component, "CurrentSMData");
    entry = with_field(&x.world.vm, entry, "SMD", Value::Object(Some(waiting)), "test").unwrap();
    x.set(component, "SMDQueue", Value::array(vec![entry]));
    let d = x.definition("GearboxFramework.ToyMove", false);
    let args = x.six(d, pawn, pawn);
    let (missing, what) = queue_refused(&mut x, component, args);
    assert_eq!(missing, Missing::NotRead);
    assert!(what.contains("the replicated record of Engine.Pawn"), "{what}");
    assert!(!what.contains("stopped after partial writes"), "{what}");
    assert!(what.contains(RECORD_REPLICATED), "Play's own record: {what}");
}

/// Test 6b, the delegation seam: the arguments Queue hands to Play's path are the five as given (an omitted one
/// omitted) and `Data` None, even when the caller passed a `Data` object.
#[test]
fn queue_hands_play_the_five_arguments_and_no_data() {
    let mut x = fx(ParityMode::Off);
    let (pawn, component) = x.owner(AUTHORITY);
    let d = x.definition("GearboxFramework.ToyMove", false);
    let data = x.obj("Engine.Actor");
    let given = x.six(d, pawn, data);
    for supplied in [6, 2] {
        let args: Vec<ow_vm::api::Arg> =
            given.iter().take(supplied).map(|v| ow_vm::api::Arg::new(v.clone(), false, true)).collect();
        let vm = x.vm();
        let f = vm.find_function(PATHS[7]).unwrap();
        let c = NativeCall::new(vm, f, Some(component), args);
        let handed = play_args(&c).unwrap();
        assert_eq!(handed.len(), 6, "{supplied}");
        for (i, a) in handed.iter().enumerate().take(5) {
            assert_eq!(a.supplied, i < supplied, "{supplied}: argument {i}'s presence");
            if i < supplied {
                assert_eq!(a.value, given[i], "{supplied}: argument {i} as given");
            }
        }
        assert!(handed[5].supplied && handed[5].value == Value::Object(None), "{supplied}: Data None");
    }
}

/// Test 7: `CurrentSMData` not a struct, or `SMDQueue` of another element type: a `VmError::Script` naming the field,
/// nothing written.
#[test]
fn queue_on_misshapen_fields_is_a_script_error() {
    for (class, field_name) in [
        ("GearboxFramework.ToyOddCurrentComponent", "CurrentSMData"),
        ("GearboxFramework.ToyOddQueueComponent", "SMDQueue"),
    ] {
        let mut x = fx(ParityMode::Off);
        let pawn = x.obj("Engine.Pawn");
        x.set(pawn, "Role", Value::Byte(AUTHORITY));
        let component = x.obj(class);
        x.set(component, "SMI", Value::Interface(Some(pawn)));
        x.set(component, "Owner", Value::Object(Some(pawn)));
        let d = x.definition("GearboxFramework.ToyMove", false);
        let args: Vec<ow_vm::api::Arg> = vec![ow_vm::api::Arg::new(Value::Object(Some(d)), false, true)];
        let before = x.counters();
        let vm = x.vm();
        let f = vm.find_function(PATHS[7]).unwrap();
        let mut c = NativeCall::new(vm, f, Some(component), args);
        match queue(&mut c) {
            Err(VmError::Script { message, .. }) => assert!(message.contains(field_name), "{class}: {message}"),
            other => panic!("{class}: expected a script error, got {other:?}"),
        }
        assert_eq!(x.counters(), before, "{class}: nothing written");
    }
}

/// The lazy handles `ToyHolder` names: (a `ToyMove`, an `Engine.Pawn`), both unloaded.
fn lazy(x: &mut Fx) -> (ObjRef, ObjRef) {
    let path = ow_core::ObjectPath::parse("GearboxFramework.Holder").unwrap();
    let vm = x.vm();
    let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let (m, a) = (x.get(holder, "Move"), x.get(holder, "Actor"));
    let (Value::Object(Some(m)), Value::Object(Some(a))) = (m, a) else { panic!("the holder names both") };
    assert!(x.world.vm.defs.is_unloaded(&x.world.vm.space, m) && x.world.vm.defs.is_unloaded(&x.world.vm.space, a));
    (m, a)
}

/// Test 2a: the component's `Owner`, the `SMI` actor, or `NewSMD` as a lazy handle (one case each): the residency
/// `NotRead`, before anything reads it; nothing loaded or written, and the handle still unloaded.
#[test]
fn queue_refuses_a_lazy_owner_actor_or_definition_without_loading_it() {
    for case in ["Owner", "SMI", "NewSMD"] {
        let mut x = fx(ParityMode::Off);
        let (pawn, component) = x.owner(AUTHORITY);
        let (lazy_move, lazy_pawn) = lazy(&mut x);
        let d = x.definition("GearboxFramework.ToyMove", false);
        let mut args = x.six(d, pawn, pawn);
        let handle = match case {
            "Owner" => {
                x.set(component, "Owner", Value::Object(Some(lazy_pawn)));
                lazy_pawn
            }
            "SMI" => {
                x.set(component, "SMI", Value::Interface(Some(lazy_pawn)));
                lazy_pawn
            }
            _ => {
                args[0] = Value::Object(Some(lazy_move));
                lazy_move
            }
        };
        let (missing, what) = queue_refused(&mut x, component, args);
        assert_eq!(missing, Missing::NotRead, "{case}");
        assert!(what.starts_with(&format!("{QUEUE_NOT_LOADED}: ")) && what.ends_with(&format!("(see {QUEUE_RECORD})")), "{case}: {what}");
        assert!(x.world.vm.defs.is_unloaded(&x.world.vm.space, handle), "{case}: still unloaded");
    }
}

/// Test 2a's stand-in case: a definition that is a stand-in with no slots is refused the same way.
#[test]
fn queue_refuses_a_stand_in_definition() {
    let mut x = fx(ParityMode::Off);
    let (pawn, component) = x.owner(AUTHORITY);
    let move_class = x.vm().find_class("GearboxFramework.ToyMove").unwrap();
    let cold = x
        .vm()
        .space
        .alloc_stand_in(move_class, "ToyColdMove", ow_object::heap::Origin::Resource { package: 0, index: -1 })
        .unwrap();
    let args = x.six(cold, pawn, pawn);
    let (missing, what) = queue_refused(&mut x, component, args);
    assert_eq!(missing, Missing::NotRead);
    assert!(what.contains(QUEUE_NOT_LOADED), "{what}");
}

/// Test 2b: the component's `Owner` and the `SMI` actor are two resident actors, both at authority: the mismatch
/// `NotRead`, nothing written.
#[test]
fn queue_refuses_an_owner_that_is_not_the_interface_s_actor() {
    let mut x = fx(ParityMode::Off);
    let (pawn, component) = x.owner(AUTHORITY);
    let other = x.obj("Engine.Pawn");
    x.set(other, "Role", Value::Byte(AUTHORITY));
    x.set(component, "Owner", Value::Object(Some(other)));
    let d = x.definition("GearboxFramework.ToyMove", true);
    let args = x.six(d, pawn, pawn);
    let (missing, what) = queue_refused(&mut x, component, args);
    assert_eq!(missing, Missing::NotRead);
    assert!(what.contains(QUEUE_OWNER_MISMATCH), "{what}");
}

/// Review r2 of #682: nothing current, on the authority, with a resident definition whose class code is cold (its
/// object loaded through the store, the class never looked up): refused before `Play`'s path dispatches
/// `GetSMDToPlay`, so the class code stays unloaded, nothing is written and no name is interned.
#[test]
fn queue_refuses_a_definition_whose_class_code_is_cold() {
    let mut x = fx(ParityMode::Off);
    let (pawn, component) = x.owner(AUTHORITY);
    let path = ow_core::ObjectPath::parse("GearboxFramework.ToyColdMoveObject").unwrap();
    let vm = x.vm();
    let d = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let class = vm.space.heap.get(d).unwrap().class;
    assert!(!vm.defs.is_unloaded(&vm.space, d), "the definition is resident");
    assert!(!vm.class_code_loaded(class), "its class code is cold");
    let args = x.six(d, pawn, pawn);
    let (missing, what) = queue_refused(&mut x, component, args);
    assert_eq!(missing, Missing::NotRead);
    assert!(what.contains(QUEUE_CODE_NOT_LOADED), "{what}");
    assert!(!x.world.vm.class_code_loaded(class), "still cold");
}

/// Review r2 of #682, test 2a's precedence: a lazy definition is the residency refusal both with a resident owner
/// below authority (before the forward) and with no `Owner` at all (before the missing-Owner refusal).
#[test]
fn queue_residency_comes_before_the_owner_and_forward_refusals() {
    for case in ["below authority", "no Owner"] {
        let mut x = fx(ParityMode::Off);
        let (pawn, component) = x.owner(SIMULATED);
        if case == "no Owner" {
            x.set(component, "Owner", Value::Object(None));
        }
        let (lazy_move, _) = lazy(&mut x);
        let args = x.six(lazy_move, pawn, pawn);
        let (missing, what) = queue_refused(&mut x, component, args);
        assert_eq!(missing, Missing::NotRead, "{case}");
        assert!(what.starts_with(&format!("{QUEUE_NOT_LOADED}: ")), "{case}: {what}");
        assert!(x.world.vm.defs.is_unloaded(&x.world.vm.space, lazy_move), "{case}: still unloaded");
    }
}

/// The spy's text for `pawn`'s record (lane IMPL-SET-REPLICATED-SMDATA): "rec(counter,smd,rate)".
fn record_text(vm: &Vm, pawn: ObjRef) -> String {
    let class = vm.space.heap.get(pawn).unwrap().class;
    let p = vm.space.prop(class, "ReplicatedSMData").unwrap();
    let rep = vm.space.read(pawn, PropAddr::scalar(p)).unwrap().clone();
    let counter = match field(vm, &rep, "DirtyCounter", "test").unwrap() {
        Value::Byte(b) => b,
        other => panic!("{other:?}"),
    };
    let smd = match field(vm, &rep, "SMD", "test").unwrap() {
        Value::Object(Some(_)) => "set",
        _ => "none",
    };
    let rate = match field(vm, &rep, "PlayRateScale", "test").unwrap() {
        Value::Float(r) => r,
        other => panic!("{other:?}"),
    };
    format!("rec({counter},{smd},{rate})")
}

/// The note's test 1: a start on an exact-authority owner (a script-only subclass of `WillowPlayerPawn`) with nothing
/// current: the record holds the request's `SMD`, `Data`, rate and duration, `DirtyCounter` 0 to 1, `bNetDirty` set;
/// the order is the record, `ServerStarted`, then `ClientStarted`.
#[test]
fn a_start_on_the_authority_writes_the_record_before_server_started() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", false);
    let (pawn, component) = x.owner_of("WillowGame.ToyMayaPawn", AUTHORITY);
    let data = x.obj("Engine.PawnAllegiance");
    x.script.borrow_mut().spy = Some(pawn);
    let no_callback = x.name("None");
    let args = vec![
        Value::Object(Some(d)),
        Value::Float(1.5),
        Value::Float(4.0),
        no_callback,
        Value::Object(None),
        Value::Object(Some(data)),
    ];
    assert_eq!(x.play(component, args).unwrap(), Value::Float(2.5));
    assert_eq!(
        x.calls(),
        vec![
            "rec(0,none,0)",
            "AuthorityCanPlay on ToyMove(_)",
            "rec(1,set,1.5)",
            "ServerStarted on ToyMove(_)",
            "rec(1,set,1.5)",
            "ClientStarted on ToyMove(_,data)",
        ]
    );
    assert_eq!(x.record(pawn, "SMD"), Value::Object(Some(d)));
    assert_eq!(x.record(pawn, "Data"), Value::Object(Some(data)));
    assert_eq!(x.record(pawn, "PlayRateScale"), Value::Float(1.5));
    assert_eq!(x.record(pawn, "Duration"), Value::Float(4.0));
    assert_eq!(x.record(pawn, "DirtyCounter"), Value::Byte(1));
    assert_eq!(x.get(pawn, "bNetDirty"), Value::Bool(true));
}

/// The note's tests 2 and 4: a start over a running move steps `DirtyCounter` twice. The interrupting stop runs
/// `ServerFinished` first, then its write holds `SMD` None and the old move's other fields (seen at `ClientFinished`);
/// the start's write holds the new move. The component keeps the stopped move's fields with `SMD` None between them.
#[test]
fn a_start_over_a_running_move_steps_the_counter_twice() {
    let mut x = fx(ParityMode::Off);
    let (first, second) = (x.definition("GearboxFramework.ToyMove", false), x.definition("GearboxFramework.ToyOtherMove", false));
    let (pawn, component) = x.owner_of("WillowGame.ToyMayaPawn", AUTHORITY);
    assert_eq!(x.play(component, vec![Value::Object(Some(first)), Value::Float(2.0)]).unwrap(), Value::Float(2.5));
    x.script.borrow_mut().spy = Some(pawn);
    x.clear_calls();
    assert_eq!(x.play(component, vec![Value::Object(Some(second))]).unwrap(), Value::Float(2.5));
    assert_eq!(
        x.calls(),
        vec![
            "rec(1,set,2)",
            "AuthorityCanPlay on ToyOtherMove(_)",
            "rec(1,set,2)",
            "ServerFinished on ToyMove(_,true)",
            "rec(2,none,2)",
            "ClientFinished on ToyMove(_,true)",
            "rec(3,set,1)",
            "ServerStarted on ToyOtherMove(_)",
            "rec(3,set,1)",
            "ClientStarted on ToyOtherMove(_,data)",
        ]
    );
    assert_eq!(x.record(pawn, "SMD"), Value::Object(Some(second)));
    assert_eq!(x.record(pawn, "DirtyCounter"), Value::Byte(3));
}

/// The note's test 4 on a stop of its own: a zero duration stops the new move, not interrupted: `ServerFinished(SMI,
/// false)` before the write, which holds `SMD` None and the stopped move's rate and duration.
#[test]
fn a_stop_on_the_authority_writes_the_record_after_server_finished() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", false);
    let (pawn, component) = x.owner_of("WillowGame.ToyMayaPawn", AUTHORITY);
    x.script.borrow_mut().duration = 0.0;
    x.script.borrow_mut().spy = Some(pawn);
    x.script.borrow_mut().spy_component = Some(component);
    assert_eq!(x.play(component, vec![Value::Object(Some(d)), Value::Float(0.5), Value::Float(3.0)]).unwrap(), Value::Float(0.0));
    let calls = x.calls();
    let at = calls.iter().position(|c| c == "ServerFinished on ToyMove(_,false)").expect("ServerFinished");
    // During ServerFinished: the component's move has SMD None and keeps its rate; the record holds its old values.
    assert_eq!(calls[at - 2], "rec(1,set,0.5)", "the start's write, not yet the stop's");
    assert_eq!(calls[at - 1], "cur(none,0.5)", "only the SMD is cleared before the event");
    // After it (seen at ClientFinished): the stop's write.
    assert_eq!(calls[at + 1], "rec(2,none,0.5)", "the stop's write comes after ServerFinished");
    assert_eq!(x.record(pawn, "SMD"), Value::Object(None));
    assert_eq!((x.record(pawn, "PlayRateScale"), x.record(pawn, "Duration")), (Value::Float(0.5), Value::Float(3.0)));
    assert_eq!(x.current(component, "SMD"), Value::Object(None));
    assert_eq!(x.current(component, "Duration"), Value::Float(3.0), "only the SMD is cleared");
}

/// The note's test 3: `DirtyCounter` 255 wraps to 0 on one write.
#[test]
fn the_dirty_counter_wraps() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", false);
    let (pawn, component) = x.owner_of("WillowGame.WillowAIPawn", AUTHORITY);
    let rep = x.get(pawn, "ReplicatedSMData");
    let rep = with_field(&x.world.vm, rep, "DirtyCounter", Value::Byte(255), "test").unwrap();
    x.set(pawn, "ReplicatedSMData", rep);
    x.play(component, vec![Value::Object(Some(d))]).unwrap();
    assert_eq!(x.record(pawn, "DirtyCounter"), Value::Byte(0));
}

/// The note's test 5, and the exact-authority test: an owner below `ROLE_Authority`, or at `ROLE_MAX` (which passes
/// the at-least gate), gets no record write and no server event.
#[test]
fn below_or_above_exact_authority_no_record_is_written() {
    for role in [AUTONOMOUS, AUTHORITY + 1] {
        let mut x = fx(ParityMode::Off);
        let d = x.definition("GearboxFramework.ToyMove", true);
        let (pawn, component) = x.owner_of("WillowGame.ToyMayaPawn", role);
        let before = x.get(pawn, "ReplicatedSMData");
        assert_eq!(x.play(component, vec![Value::Object(Some(d))]).unwrap(), Value::Float(2.5), "role {role}");
        assert_eq!(x.get(pawn, "ReplicatedSMData"), before, "role {role}");
        assert_eq!(x.get(pawn, "bNetDirty"), Value::Bool(false), "role {role}");
        assert!(x.calls().iter().all(|c| !c.starts_with("Server")), "role {role}: {:?}", x.calls());
    }
}

/// #518's three counters and the name table's length.
fn counters(x: &Fx) -> (u64, u64, u64, usize) {
    let vm = &x.world.vm;
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len())
}

/// A preflight refusal: the `NotYet` or error, with the counters, the name table, the component, the pawn's record
/// and the event list unchanged.
fn refused(x: &mut Fx, pawn: ObjRef, component: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
    let before = (counters(x), x.state(component), x.get(pawn, "ReplicatedSMData"));
    x.clear_calls();
    let r = x.play(component, args);
    assert!(r.is_err(), "{r:?}");
    assert_eq!((counters(x), x.state(component), x.get(pawn, "ReplicatedSMData")), before, "nothing changed: {r:?}");
    assert!(x.calls().is_empty(), "no event: {:?}", x.calls());
    r
}

/// The note's test 6: an owner whose first native class is not one of the four read is the class `NotRead`.
#[test]
fn an_unread_native_owner_class_is_not_read() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", false);
    let (pawn, component) = x.owner_of("GearboxFramework.ToyNativePawn", AUTHORITY);
    let (missing, text) = not_yet(refused(&mut x, pawn, component, vec![Value::Object(Some(d))]));
    assert_eq!(missing, Missing::NotRead);
    assert!(text.contains("the replicated record of GearboxFramework.ToyNativePawn"), "{text}");
}

/// The note's test 7: the record's declarations, each a script error naming the field, before any write.
#[test]
fn the_record_declarations_are_checked_before_any_write() {
    for (variant, needle) in [
        (Variant::OtherStruct, "struct of another type"),
        (Variant::MissingField, "Duration"),
        (Variant::IntCounter, "DirtyCounter"),
        (Variant::IntNetDirty, "bNetDirty"),
        (Variant::SmdNotObject, "SMD"),
        (Variant::RateNotFloat, "PlayRateScale"),
        (Variant::NoServerStarted, "declares no ServerStarted"),
    ] {
        let mut x = fx_variant(ParityMode::Off, &[&crate::action_skill::Pack], variant);
        let d = x.definition("GearboxFramework.ToyMove", false);
        let (pawn, component) = x.owner_of("WillowGame.ToyMayaPawn", AUTHORITY);
        match refused(&mut x, pawn, component, vec![Value::Object(Some(d))]) {
            Err(VmError::Script { message, .. }) => assert!(message.contains(needle), "{variant:?}: {message}"),
            other => panic!("{variant:?}: expected a script error, got {other:?}"),
        }
    }
}

/// The first reads (the hub's heads-up for #684): no `Owner`, or an `Owner` that is not the `SMI` actor, is a typed
/// `NotRead`; a definition the store has not loaded is `NOT_LOADED` and stays unloaded; a definition whose class code
/// is not loaded is `NotRead` and its code stays unloaded. Each writes and loads nothing.
#[test]
fn the_first_reads_are_loaded_only() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", false);
    let (pawn, component) = x.owner_of("WillowGame.ToyMayaPawn", AUTHORITY);
    x.set(component, "Owner", Value::Object(None));
    let (_, text) = not_yet(refused(&mut x, pawn, component, vec![Value::Object(Some(d))]));
    assert!(text.contains(NO_OWNER), "{text}");
    let (other, _) = x.owner_of("WillowGame.ToyMayaPawn", AUTHORITY);
    x.set(component, "Owner", Value::Object(Some(other)));
    let (_, text) = not_yet(refused(&mut x, pawn, component, vec![Value::Object(Some(d))]));
    assert!(text.contains(OTHER_OWNER), "{text}");
    x.set(component, "Owner", Value::Object(Some(pawn)));
    // A lazy definition handle.
    let path = ow_core::ObjectPath::parse("GearboxFramework.ToyMoveHolderObject").unwrap();
    let vm = x.vm();
    let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let lazy = match x.get(holder, "Move") {
        Value::Object(Some(o)) => o,
        other => panic!("{other:?}"),
    };
    assert!(x.world.vm.defs.is_unloaded(&x.world.vm.space, lazy));
    let (missing, text) = not_yet(refused(&mut x, pawn, component, vec![Value::Object(Some(lazy))]));
    assert_eq!(missing, Missing::NotRead);
    assert!(text.contains(NOT_LOADED), "{text}");
    assert!(x.world.vm.defs.is_unloaded(&x.world.vm.space, lazy), "still unloaded");
    // A lazy Owner (before the mismatch test), then a lazy SMI actor with a resident Owner (before the mismatch test).
    let lazy_pawn = match x.get(holder, "Pawn") {
        Value::Object(Some(o)) => o,
        other => panic!("{other:?}"),
    };
    x.set(component, "Owner", Value::Object(Some(lazy_pawn)));
    let (_, text) = not_yet(refused(&mut x, pawn, component, vec![Value::Object(Some(d))]));
    assert!(text.contains(NOT_LOADED), "a lazy Owner: {text}");
    x.set(component, "Owner", Value::Object(Some(pawn)));
    x.set(component, "SMI", Value::Interface(Some(lazy_pawn)));
    let (_, text) = not_yet(refused(&mut x, pawn, component, vec![Value::Object(Some(d))]));
    assert!(text.contains(NOT_LOADED), "a lazy SMI actor: {text}");
    assert!(x.world.vm.defs.is_unloaded(&x.world.vm.space, lazy_pawn), "still unloaded");
    x.set(component, "SMI", Value::Interface(Some(pawn)));
    x.set(component, "Owner", Value::Object(Some(pawn)));
    // A definition whose class code is not loaded.
    let cold = {
        let vm = x.vm();
        let (code, store) = vm.defs.code_mut();
        let key = code.find_class(store, "GearboxFramework.ToyColdMove").unwrap();
        let c = code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap();
        vm.space.alloc(c, "Cold_0", None, ow_object::heap::Origin::Runtime).unwrap()
    };
    let class = x.world.vm.space.heap.get(cold).unwrap().class;
    assert!(!x.world.vm.class_code_loaded(class));
    let (missing, text) = not_yet(refused(&mut x, pawn, component, vec![Value::Object(Some(cold))]));
    assert_eq!(missing, Missing::NotRead);
    assert!(text.contains("not loaded"), "{text}");
    assert!(!x.world.vm.class_code_loaded(class), "its code is still not loaded");
}

/// The spy's text for `component`'s current move: "cur(smd,rate)".
fn current_text(vm: &Vm, component: ObjRef) -> String {
    let class = vm.space.heap.get(component).unwrap().class;
    let p = vm.space.prop(class, "CurrentSMData").unwrap();
    let cur = vm.space.read(component, PropAddr::scalar(p)).unwrap().clone();
    let smd = match field(vm, &cur, "SMD", "test").unwrap() {
        Value::Object(Some(_)) => "set",
        _ => "none",
    };
    let rate = match field(vm, &cur, "PlayRateScale", "test").unwrap() {
        Value::Float(r) => r,
        other => panic!("{other:?}"),
    };
    format!("cur({smd},{rate})")
}

/// The note's test 5b: a `ServerStarted` that reaches an unbound native after the commit is terminal, "stopped after
/// partial writes", and the record and `CurrentSMData` keep what the commit wrote.
#[test]
fn a_server_started_failure_after_the_commit_is_terminal() {
    let mut x = fx(ParityMode::Off);
    let d = x.definition("GearboxFramework.ToyMove", false);
    let (pawn, component) = x.owner_of("WillowGame.ToyMayaPawn", AUTHORITY);
    x.script.borrow_mut().fail_server_start = true;
    let (missing, text) = not_yet(x.play(component, vec![Value::Object(Some(d)), Value::Float(1.5)]));
    assert_eq!(missing, Missing::NotRead);
    assert!(text.starts_with("stopped after partial writes: a toy unbound native in OnServerStart"), "{text}");
    assert_eq!((x.record(pawn, "SMD"), x.record(pawn, "DirtyCounter")), (Value::Object(Some(d)), Value::Byte(1)));
    assert_eq!(x.current(component, "SMD"), Value::Object(Some(d)));
    assert!(x.calls().iter().all(|c| !c.starts_with("ClientStarted")), "{:?}", x.calls());
}

/// Review r1 of #689, item 1: a definition whose `GetSMDToPlay` answers another definition the store has not loaded is
/// the residency `NotRead`, before any read of it; the answered definition stays unloaded and nothing is written.
#[test]
fn a_redirected_definition_must_be_resident() {
    let mut x = fx(ParityMode::Off);
    let path = ow_core::ObjectPath::parse("GearboxFramework.ToyMoveHolderObject").unwrap();
    let vm = x.vm();
    let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let lazy = match x.get(holder, "Move") {
        Value::Object(Some(o)) => o,
        other => panic!("{other:?}"),
    };
    let f = x.vm().find_function("GearboxFramework.ToyRedirectMove.GetSMDToPlay").unwrap();
    let key = x.vm().function(f).unwrap().native_key.clone().unwrap();
    x.vm().register_native(&key, Rc::new(move |_: &mut NativeCall<'_>| Ok(Value::Object(Some(lazy))))).unwrap();
    let d = x.definition("GearboxFramework.ToyRedirectMove", true);
    let (pawn, component) = x.owner_of("WillowGame.ToyMayaPawn", AUTONOMOUS);
    let (missing, text) = not_yet(refused(&mut x, pawn, component, vec![Value::Object(Some(d))]));
    assert_eq!(missing, Missing::NotRead);
    assert!(text.contains(NOT_LOADED), "{text}");
    assert!(x.world.vm.defs.is_unloaded(&x.world.vm.space, lazy), "still unloaded");
}

/// Review r1 of #689, item 3: `PlayLocal` keeps the loading lookup (it has no write-free preflight; its events go
/// through it anyway). On a definition whose class code is not loaded, it loads that code and plays, as before this
/// lane; `Play` refuses the same definition write-free (`the_first_reads_are_loaded_only`).
#[test]
fn play_local_loads_a_cold_definition_class_as_before() {
    let mut x = fx(ParityMode::Off);
    let (_, component) = x.owner(AUTONOMOUS);
    let cold = {
        let vm = x.vm();
        let (code, store) = vm.defs.code_mut();
        let key = code.find_class(store, "GearboxFramework.ToyColdMove").unwrap();
        let c = code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap();
        vm.space.alloc(c, "Cold_0", None, ow_object::heap::Origin::Runtime).unwrap()
    };
    let class = x.world.vm.space.heap.get(cold).unwrap().class;
    assert!(!x.world.vm.class_code_loaded(class));
    assert_eq!(x.call(PATHS[6], component, vec![Value::Object(Some(cold))]).unwrap(), Value::Float(2.5));
    assert!(x.world.vm.class_code_loaded(class), "PlayLocal loaded the class's code, as main does");
    assert_eq!(x.calls(), vec!["ServerStarted on ToyColdMove(_)", "ClientStarted on ToyColdMove(_,data)"]);
}

/// Review r1 of #689, item 5: with a record struct that has `CallbackName` and `CallbackObject` fields, a start with a
/// callback leaves both at their defaults: the callback is not copied.
#[test]
fn the_callback_is_not_copied_into_the_record() {
    let mut x = fx_variant(ParityMode::Off, &[&crate::action_skill::Pack], Variant::CallbackFields);
    let d = x.definition("GearboxFramework.ToyMove", false);
    let (pawn, component) = x.owner_of("WillowGame.ToyMayaPawn", AUTHORITY);
    let before = (x.record(pawn, "CallbackName"), x.record(pawn, "CallbackObject"));
    let done = x.name("ToyDone");
    let args = vec![Value::Object(Some(d)), Value::Float(1.0), Value::Float(0.0), done, Value::Object(Some(pawn))];
    assert_eq!(x.play(component, args).unwrap(), Value::Float(2.5));
    assert_eq!(x.record(pawn, "SMD"), Value::Object(Some(d)), "the record was written");
    assert_eq!((x.record(pawn, "CallbackName"), x.record(pawn, "CallbackObject")), before, "not copied");
    assert_ne!(x.current(component, "CallbackName"), before.0, "the move itself has the callback");
}

// ---------------------------------------------------------------------------------------------------------------
// COLD-CLASS-AUDIT (lane COLD-CLASS-AUDIT-SKILLS): each Off native of this module, called with an object whose class has
// a layout but no loaded code (the fixtures above warm their classes through `find_class`, which hides a code load).
// A load is a write, so the native must leave `Vm::class_code_loads`, #518's three counters and the name table as they
// were. A test that fails because the native loads cold code is `#[ignore]`d with its reason, for the hub to route.
// ---------------------------------------------------------------------------------------------------------------

/// The code loads, #518's three counters and the name table's length.
fn cold_marks(x: &Fx) -> (u64, u64, u64, u64, usize) {
    let vm = &x.world.vm;
    (vm.class_code_loads(), vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len())
}

impl Fx {
    /// An object of `class` with its layout defined and its code not loaded.
    fn cold(&mut self, class: &str) -> ObjRef {
        let vm = self.vm();
        let (code, store) = vm.defs.code_mut();
        let key = code.find_class(store, class).unwrap();
        let c = code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap();
        assert!(!vm.class_code_loaded(c), "{class} starts cold");
        vm.space.alloc(c, "Cold", None, ow_object::heap::Origin::Runtime).unwrap()
    }

    /// Runs `f` and asserts the code loads, counters and name table unchanged; answers `f`'s result.
    fn no_code_load<T: std::fmt::Debug>(&mut self, label: &str, f: impl FnOnce(&mut Fx) -> T) -> T {
        let before = cold_marks(self);
        let r = f(self);
        assert_eq!(cold_marks(self), before, "{label}: no code load, write, draw, store load or name: {r:?}");
        r
    }
}

/// `Play` of a definition whose class code is not loaded.
#[test]
#[ignore = "COLD-CLASS-AUDIT: Play loads the cold definition's class code (GetSMDToPlay through method_of -> find_method); #689 makes the lookup loaded-only"]
fn cold_play_definition_loads_no_code() {
    let mut x = fx(ParityMode::Off);
    let (_, component) = x.owner(AUTONOMOUS);
    let d = x.cold("GearboxFramework.ToyColdMove");
    let _ = x.no_code_load("Play", |x| x.play(component, vec![Value::Object(Some(d))]));
}

/// `PlayLocal` of a definition whose class code is not loaded.
#[test]
#[ignore = "COLD-CLASS-AUDIT: PlayLocal loads the cold definition's class code (GetSMDToPlay through method_of -> find_method)"]
fn cold_play_local_definition_loads_no_code() {
    let mut x = fx(ParityMode::Off);
    let (_, component) = x.owner(AUTONOMOUS);
    let d = x.cold("GearboxFramework.ToyColdMove");
    let _ = x.no_code_load("PlayLocal", |x| x.call(PATHS[6], component, vec![Value::Object(Some(d))]));
}

/// The definition natives `GetSMDToPlay` and `IsLocalAuthority` on a cold definition.
#[test]
fn cold_definition_natives_load_no_code() {
    let mut x = fx(ParityMode::Off);
    let (pawn, _) = x.owner(AUTONOMOUS);
    let d = x.cold("GearboxFramework.ToyColdMove");
    let smi = Value::Interface(Some(pawn));
    let got = x.no_code_load("GetSMDToPlay", |x| x.call(PATHS[1], d, vec![smi.clone()]));
    assert_eq!(got.unwrap(), Value::Object(Some(d)));
    let got = x.no_code_load("IsLocalAuthority", |x| x.call(PATHS[2], d, vec![smi]));
    assert_eq!(got.unwrap(), Value::Bool(false), "an autonomous owner, bClientHasAuthority clear");
}

/// `IsPlayingClass` with a current move whose definition's class code is not loaded.
#[test]
fn cold_is_playing_class_loads_no_code() {
    let mut x = fx(ParityMode::Off);
    let (_, component) = x.owner(AUTONOMOUS);
    let d = x.cold("GearboxFramework.ToyColdMove");
    x.seed(component, d, None);
    let class = x.world.vm.space.classes.by_path("GearboxFramework.ToyMove");
    let got = x.no_code_load("IsPlayingClass", |x| x.call(PATHS[3], component, vec![Value::Class(class)]));
    assert_eq!(got.unwrap(), Value::Bool(false));
}

/// `GetSMNode` on a tree whose `SpecialMoveBlend` node is of a cold class.
#[test]
#[ignore = "COLD-CLASS-AUDIT: GetSMNode calls vm.find_class(AnimNodeSpecialMoveBlend), which loads that class's code when cold"]
fn cold_get_sm_node_loads_no_code() {
    let mut x = fx(ParityMode::Off);
    let (pawn, component) = x.owner(AUTONOMOUS);
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    x.set(pawn, "Mesh", Value::Object(Some(mesh)));
    let node = x.cold("GearboxFramework.ToyColdBlend");
    let n = x.name("SpecialMoveBlend");
    x.set(node, "NodeName", n);
    x.set(mesh, "Animations", Value::Object(Some(node)));
    let before = x.world.vm.class_code_loads();
    let _ = x.call(PATHS[4], component, vec![]);
    assert_eq!(x.world.vm.class_code_loads(), before, "no class code loaded");
}
