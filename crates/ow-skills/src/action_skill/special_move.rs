//! Owner: lane FIX-SM-PLAY. AI-assisted (Claude Code). Playing a special move: `SpecialMoveComponent.Play` and
//! `PlayLocal`, the component queries `GetSMNode` and `IsPlayingClass`, the definition natives `GetSMDToPlay` and
//! `IsLocalAuthority`, and `Pawn.IsAliveAndWell`, coded from NATIVE_SPECIAL_MOVE_PLAY.md alone (decision 2026-11-19-52).
//! This lane did not read the natives. Every rule is the note's reading of the executable, **UNVERIFIED** in the
//! running game. The script events (`AuthorityCanPlay`, `ServerStarted`, `ClientStarted`, `ClientFinished`) and
//! `LiftActionSkill.GetPlayerAnimation` are installed script and run as such; this module binds the natives they and
//! the cast reach.
//!
//! **Only in `ParityMode::Off`.** The C++ reference has no stand-in, so Reference binds nothing here (R3). Bound by
//! function path ([`ow_combat::hit::bind`]): the first binding of a key wins. `ow-combat`'s hit pack binds `Play` as a
//! no-op and `IsPlayingClass` as false (its presentation stand-ins, in both modes); in a world that registers that
//! pack first, those stay.
//!
//! - **`Pawn.IsAliveAndWell()`** (section 0): `bDeleteMe`, `bIsInjured` and `bIsDead` all clear.
//! - **`SpecialMoveDefinition.GetSMDToPlay(SMI)`** (section 1 step 2): the definition itself.
//! - **`SpecialMoveDefinition.IsLocalAuthority(SMI)`** (section 1 step 3): the owner's `Role` is at least
//!   `ROLE_Authority`, or at least `ROLE_AutonomousProxy` when the definition's `bClientHasAuthority` is set. The
//!   roles are compared by their order in the `Role` property's enum, looked up by name. No owner is `NotYet`.
//! - **`SpecialMoveComponent.IsPlayingClass(SMClass)`** (section 3): a current move whose definition is of that class
//!   or a subclass; false with no current move or no class.
//! - **`SpecialMoveComponent.GetSMNode()`** (section 3): None without an owner interface. A cached `SMBlendNode` whose
//!   `SkelComponent` is the owner's current `Mesh` is reused. Otherwise:
//!   - an owner with no `Mesh`, or a mesh with no `Animations`, is `NotYet { LayerAbsent }` naming the missing anim
//!     tree: the host's animation state is unavailable. The game's pawns always have a tree; the headless world has
//!     none, and the game's None here would make `Play` start nothing silently (the hub's decision for this lane);
//!   - with a tree, the node named `SpecialMoveBlend` (compared without case, as names are) is looked for from the
//!     tree's root down: each node, then its `Children` entries' `Anim` nodes in order, each node once, at most
//!     [`NODE_LIMIT`] nodes (more is a data error). The first node of that name answers when it is an
//!     `AnimNodeSpecialMoveBlend`, and is cached in `SMBlendNode`; a node of another class, or no node of that name,
//!     answers None (the note's None). The search order is the port's (UNVERIFIED): the note says only which node
//!     answers.
//! - **`SpecialMoveComponent.Play(NewSMD, PlayRateScale = 1, Duration = 0, CallbackName, CallbackObject, Data)`**
//!   (sections 1 and 4):
//!   1. 0 with no definition or no owner interface (`SMI`); then the definition to play (`GetSMDToPlay`), 0 with none;
//!   2. the gates, in this order: `IsLocalAuthority`, then the script event `AuthorityCanPlay` (not asked when the
//!      first fails; UNVERIFIED). Failed: 0, nothing changed;
//!   3. every refusal is decided before anything changes (review of #437), each `NotYet`:
//!      - an owner below `ROLE_Authority` in a world whose `WorldInfo.NetMode` is `NM_Client`: the forward to the
//!        server (`ServerSpecialMove_Play`, step 6) is network play, not ported;
//!      - with the gates passed, an owner at `ROLE_Authority`: the record the owner keeps of the current move for
//!        replication (step 4.4) is not named in the note, so it is `NotYet { NotRead }`. In the single-player slice
//!        this is every `Play` past the gates;
//!      - a definition with a `NextSpecialMove`: what the queued follow-ups carry is not read;
//!      - a stop this start may run (the current move's, or the new move's own when its duration is 0 or less) whose
//!        definition declares no `ClientFinished`, whose callback the target does not declare, or whose callback has
//!        a parameter other than one `bool bInterrupted`: the note names no other argument, and a zero or a default
//!        would be a guess;
//!   4. the start: the queue (`SMDQueue`) emptied; a current move stopped as interrupted; the request (its definition
//!      the one to play) made the current move (`CurrentSMData`) and `StartTime` the world time; the definition's
//!      `ClientStarted(SMI, SMData)` answers the duration, and a duration of 0 or less stops the move again, not
//!      interrupted. The answer is the duration.
//!
//!   A script event that fails during the start (`ClientFinished` or `ClientStarted` reaching an unbound native, for
//!   example) cannot be decided in advance. Then the component's `CurrentSMData`, `SMDQueue` and `StartTime` are put
//!   back as they were before the call and the error is answered. What the events themselves changed before failing
//!   is not undone. The game has no such failure; this keeps a refused `Play` from leaving the component half-started.
//!
//!   Stopping a move (section 4), as `Play` uses it (queue cleared, owner below authority): the queue emptied, the
//!   current move cleared, the old definition's `ClientFinished(SMI, bInterrupted)`, then the callback when the old
//!   definition passes `IsLocalAuthority` and the request named one: the function `CallbackName` on `CallbackObject`
//!   (or on the owner), with `bInterrupted` when it declares that one parameter. On the authority the stop would also
//!   run `ServerFinished` and update the replicated record; `Play` refuses before that case is reached.
//! - **`SpecialMoveComponent.PlayLocal(...)`** (section 1): 0 with no definition or owner interface; otherwise the
//!   definition to play (0 with none), its `ServerStarted(SMI)` and then its `ClientStarted(SMI, SMData)`, whose answer
//!   is the answer. No gate, no current move, no `StartTime`, no queue.
//!
//! **Still unread, so unbound here** (the stub policy answers): `RunBehaviors` (lane #432), the behavior provider's
//! `OnStart` / `OnServerStart`, and the blend node's `PlayData`, which `ClientStarted` reaches.

use std::rc::Rc;

use ow_combat::hit::bind;
use ow_core::NotYet;
use ow_object::ids::{FuncId, ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::{method_of, need, script_error};

/// The record these natives follow.
pub const RECORD: &str = "NATIVE_SPECIAL_MOVE_PLAY.md";

/// The function paths [`register`] binds in Off.
pub const PATHS: [&str; 7] = [
    "Engine.Pawn.IsAliveAndWell",
    "GearboxFramework.SpecialMoveDefinition.GetSMDToPlay",
    "GearboxFramework.SpecialMoveDefinition.IsLocalAuthority",
    "GearboxFramework.SpecialMoveComponent.IsPlayingClass",
    "GearboxFramework.SpecialMoveComponent.GetSMNode",
    "GearboxFramework.SpecialMoveComponent.Play",
    "GearboxFramework.SpecialMoveComponent.PlayLocal",
];

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| script_error(what, "called without an object"))
}

fn object(v: &Value, what: &str) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) | Value::Interface(o) => Ok(*o),
        Value::None => Ok(None),
        other => Err(script_error(what, format!("expected an object, found {}", other.kind_name()))),
    }
}

fn flag(vm: &mut Vm, o: ObjRef, name: &str, what: &str) -> Result<bool, VmError> {
    match need(vm, o, name)? {
        Value::Bool(b) => Ok(b),
        other => Err(script_error(what, format!("{name} is {}, not a bool", other.kind_name()))),
    }
}

fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value, what: &str) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| script_error(what, format!("the class declares no {name}")))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// The order of `o`'s byte property `name` and of the enum entry `entry`, both by the property's enum.
fn enum_order(vm: &mut Vm, o: ObjRef, name: &str, entry: &str, what: &str) -> Result<(u8, usize), VmError> {
    let raw = match need(vm, o, name)? {
        Value::Byte(b) => b,
        other => return Err(script_error(what, format!("{name} is {}, not a byte", other.kind_name()))),
    };
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script_error(what, format!("no {name}")))?;
    let Some(PropKind::Byte { enum_: Some(e) }) = vm.space.classes.decl(p).map(|d| d.kind.clone()) else {
        return Err(script_error(what, format!("{name} is not an enum byte")));
    };
    let id = vm.space.names.find(entry);
    let index = id
        .and_then(|id| vm.space.classes.enum_layout(e).and_then(|l| l.index_of(id)))
        .ok_or_else(|| script_error(what, format!("{name}'s enum has no {entry}")))?;
    Ok((raw, index))
}

/// Whether `o`'s `Role` is at least `entry`.
fn role_at_least(vm: &mut Vm, o: ObjRef, entry: &str, what: &str) -> Result<bool, VmError> {
    let (raw, index) = enum_order(vm, o, "Role", entry, what)?;
    Ok(usize::from(raw) >= index)
}

/// Calls `name` on `o` (its class's function) and answers its result.
fn call(vm: &mut Vm, o: ObjRef, name: &str, args: Vec<Value>) -> Result<Value, VmError> {
    let Some(f) = method_of(vm, o, name)? else {
        return Err(NotYet::not_read(
            format!("{name} on {} (the class declares no such function)", ow_attr::eval::class_path(&vm.space, o)),
            RECORD,
        )
        .into());
    };
    Ok(vm.call(f, Some(o), args)?.ret)
}

fn call_bool(vm: &mut Vm, o: ObjRef, name: &str, args: Vec<Value>) -> Result<bool, VmError> {
    match call(vm, o, name, args)? {
        Value::Bool(b) => Ok(b),
        other => Err(script_error(name, format!("expected a bool result, found {}", other.kind_name()))),
    }
}

/// The owner interface's object: the component's `SMI`.
fn owner_of(vm: &mut Vm, component: ObjRef, what: &str) -> Result<Option<ObjRef>, VmError> {
    object(&need(vm, component, "SMI")?, what)
}

fn interface(owner: ObjRef) -> Value {
    Value::Interface(Some(owner))
}

// ---------------------------------------------------------------------------------------------------------------
// The small natives
// ---------------------------------------------------------------------------------------------------------------

fn is_alive_and_well(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "Pawn.IsAliveAndWell";
    let pawn = this(c, WHAT)?;
    let mut alive = true;
    for name in ["bDeleteMe", "bIsInjured", "bIsDead"] {
        alive &= !flag(c.vm, pawn, name, WHAT)?;
    }
    Ok(Value::Bool(alive))
}

fn is_local_authority_of(vm: &mut Vm, definition: ObjRef, smi: &Value) -> Result<bool, VmError> {
    const WHAT: &str = "SpecialMoveDefinition.IsLocalAuthority";
    let Some(owner) = object(smi, WHAT)? else {
        return Err(NotYet::not_read("IsLocalAuthority with no owner interface (the note reads only an owner)", RECORD).into());
    };
    if role_at_least(vm, owner, "ROLE_Authority", WHAT)? {
        return Ok(true);
    }
    Ok(flag(vm, definition, "bClientHasAuthority", WHAT)? && role_at_least(vm, owner, "ROLE_AutonomousProxy", WHAT)?)
}

fn is_playing_class(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "SpecialMoveComponent.IsPlayingClass";
    let component = this(c, WHAT)?;
    let class = match c.arg(0)? {
        Value::Class(k) => *k,
        Value::None => None,
        other => return Err(script_error(WHAT, format!("expected a class, found {}", other.kind_name()))),
    };
    let Some(class) = class else { return Ok(Value::Bool(false)) };
    let current = need(c.vm, component, "CurrentSMData")?;
    let Some(smd) = object(&field(c.vm, &current, "SMD", WHAT)?, WHAT)? else {
        return Ok(Value::Bool(false));
    };
    let own = c.vm.space.heap.get(smd)?.class;
    Ok(Value::Bool(c.vm.space.classes.is_child_of(own, class)))
}

/// The most nodes the `SpecialMoveBlend` search visits (module documentation): an invented bound, far above a pawn's
/// tree, so a malformed tree is a data error rather than a long walk.
pub const NODE_LIMIT: usize = 4096;

/// The first node named `name` (without case) from `root` down, children in order, each node once.
fn find_node(vm: &mut Vm, root: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    let mut stack = vec![root];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(node) = stack.pop() {
        if !seen.insert((node.index(), node.generation())) {
            continue;
        }
        if seen.len() > NODE_LIMIT {
            return Err(script_error(what, format!("the anim tree has more than {NODE_LIMIT} nodes")));
        }
        if let Some(Value::Name(n)) = super::get(vm, node, "NodeName")?
            && vm.space.names.text(n).eq_ignore_ascii_case(name)
        {
            return Ok(Some(node));
        }
        let children = match super::get(vm, node, "Children")? {
            Some(Value::Array(items)) => items,
            Some(other) => return Err(script_error(what, format!("Children is {}, not an array", other.kind_name()))),
            None => continue,
        };
        let mut next = Vec::with_capacity(children.len());
        for child in children.iter() {
            if let Some(anim) = object(&field(vm, child, "Anim", what)?, what)? {
                next.push(anim);
            }
        }
        stack.extend(next.into_iter().rev());
    }
    Ok(None)
}

fn get_sm_node(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "SpecialMoveComponent.GetSMNode";
    let component = this(c, WHAT)?;
    let Some(owner) = owner_of(c.vm, component, WHAT)? else {
        return Ok(Value::Object(None));
    };
    let mesh = object(&need(c.vm, owner, "Mesh")?, WHAT)?;
    if let Some(cached) = object(&need(c.vm, component, "SMBlendNode")?, WHAT)?
        && mesh.is_some()
        && object(&need(c.vm, cached, "SkelComponent")?, WHAT)? == mesh
    {
        return Ok(Value::Object(Some(cached)));
    }
    let tree = match mesh {
        Some(m) => object(&need(c.vm, m, "Animations")?, WHAT)?,
        None => None,
    };
    let Some(tree) = tree else {
        return Err(NotYet::layer_absent(
            "GetSMNode on an owner with no anim tree (no Mesh, or a mesh with no Animations): the game's pawns always \
             have one, so the game's None (no SpecialMoveBlend node) is not answered for the headless world",
            "ow-host (the owner's mesh and anim tree)",
            RECORD,
        )
        .into());
    };
    let blend = c.vm.find_class("GearboxFramework.AnimNodeSpecialMoveBlend")?;
    match find_node(c.vm, tree, "SpecialMoveBlend", WHAT)? {
        Some(node) if c.vm.space.classes.is_child_of(c.vm.space.heap.get(node)?.class, blend) => {
            write(c.vm, component, "SMBlendNode", Value::Object(Some(node)), WHAT)?;
            Ok(Value::Object(Some(node)))
        }
        _ => Ok(Value::Object(None)),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Play, PlayLocal and the stop
// ---------------------------------------------------------------------------------------------------------------

fn field(vm: &Vm, v: &Value, name: &str, what: &str) -> Result<Value, VmError> {
    let Value::Struct(s) = v else {
        return Err(script_error(what, format!("expected a struct, found {}", v.kind_name())));
    };
    let id = vm.space.names.find(name).ok_or_else(|| script_error(what, format!("no field {name}")))?;
    let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| script_error(what, "unknown struct"))?;
    let (at, _) = layout.field(id).ok_or_else(|| script_error(what, format!("no field {name}")))?;
    Ok(s.fields[at as usize].clone())
}

fn with_field(vm: &Vm, mut v: Value, name: &str, x: Value, what: &str) -> Result<Value, VmError> {
    let ty = match &v {
        Value::Struct(s) => s.ty,
        other => return Err(script_error(what, format!("expected a struct, found {}", other.kind_name()))),
    };
    let id = vm.space.names.find(name).ok_or_else(|| script_error(what, format!("no field {name}")))?;
    let (at, _) = vm
        .space
        .classes
        .struct_layout(ty)
        .and_then(|l| l.field(id))
        .ok_or_else(|| script_error(what, format!("no field {name}")))?;
    v.struct_mut()
        .ok_or_else(|| script_error(what, "not a struct"))?
        .fields[at as usize] = x;
    Ok(v)
}

/// The struct type of the component's `CurrentSMData` (`SpecialMoveData`).
fn data_type(vm: &Vm, component: ObjRef, what: &str) -> Result<StructId, VmError> {
    let class = vm.space.heap.get(component)?.class;
    let p = vm.space.prop(class, "CurrentSMData").ok_or_else(|| script_error(what, "no CurrentSMData"))?;
    match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(PropKind::Struct(s)) => Ok(*s),
        _ => Err(script_error(what, "CurrentSMData is not a struct")),
    }
}

/// The request's arguments as a `SpecialMoveData` with definition `smd` (the note's defaults for the missing ones:
/// rate 1, duration 0, no callback, no data).
fn request(c: &NativeCall<'_>, component: ObjRef, smd: ObjRef, what: &str) -> Result<Value, VmError> {
    let vm = &*c.vm;
    let ty = data_type(vm, component, what)?;
    let mut v = vm.space.classes.struct_defaults(ty).map_err(|e| script_error(what, e.to_string()))?;
    let float = |i: usize, default: f32| -> Result<f32, VmError> {
        if !c.has(i) {
            return Ok(default);
        }
        match c.arg(i)? {
            Value::Float(x) => Ok(*x),
            other => Err(script_error(what, format!("argument {i} is {}, not a float", other.kind_name()))),
        }
    };
    v = with_field(vm, v, "PlayRateScale", Value::Float(float(1, 1.0)?), what)?;
    v = with_field(vm, v, "Duration", Value::Float(float(2, 0.0)?), what)?;
    for (i, name) in [(3, "CallbackName"), (4, "CallbackObject"), (5, "Data")] {
        if c.has(i) {
            v = with_field(vm, v, name, c.arg(i)?.clone(), what)?;
        }
    }
    with_field(vm, v, "SMD", Value::Object(Some(smd)), what)
}

/// The definition `Play` and `PlayLocal` start, with the owner: None for the note's 0 answers.
fn to_play(c: &mut NativeCall<'_>, component: ObjRef, what: &str) -> Result<Option<(ObjRef, ObjRef)>, VmError> {
    let Some(requested) = object(c.arg(0)?, what)? else { return Ok(None) };
    let Some(owner) = owner_of(c.vm, component, what)? else { return Ok(None) };
    let smd = object(&call(c.vm, requested, "GetSMDToPlay", vec![interface(owner)])?, what)?;
    Ok(smd.map(|s| (s, owner)))
}

fn world_time(vm: &mut Vm) -> Result<f32, VmError> {
    match world_info(vm)? {
        Some(w) => match need(vm, w, "TimeSeconds")? {
            Value::Float(t) => Ok(t),
            other => Err(script_error("Play", format!("TimeSeconds is {}", other.kind_name()))),
        },
        None => Err(NotYet::layer_absent("Play's StartTime (the world time)", "a level with a WorldInfo", RECORD).into()),
    }
}

fn world_info(vm: &mut Vm) -> Result<Option<ObjRef>, VmError> {
    let actors = ow_world::actors::actors_of(vm)?;
    let wi = actors
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("ow_world::actors::Actors"))?
        .world_info();
    Ok(wi)
}

/// Whether the world is a network client (`WorldInfo.NetMode` is `NM_Client`).
fn is_network_client(vm: &mut Vm, what: &str) -> Result<bool, VmError> {
    let Some(wi) = world_info(vm)? else {
        return Err(NotYet::layer_absent("Play by an owner below authority (the world's net mode)", "a level with a WorldInfo", RECORD).into());
    };
    let (raw, index) = enum_order(vm, wi, "NetMode", "NM_Client", what)?;
    Ok(usize::from(raw) == index)
}

/// A stop's callback, resolved before anything changes: none, or the function on its target and whether it takes
/// `bInterrupted`.
enum Callback {
    None,
    Call { target: ObjRef, f: FuncId, interrupted_param: bool },
}

/// The callback a stop of the move `data` (definition `smd`) would call (module documentation), or the `NotYet` of an
/// unsupported one.
fn callback_of(vm: &mut Vm, data: &Value, smd: ObjRef, owner: ObjRef, zero: &Value, what: &str) -> Result<Callback, VmError> {
    let name = field(vm, data, "CallbackName", what)?;
    if name == field(vm, zero, "CallbackName", what)? || !is_local_authority_of(vm, smd, &interface(owner))? {
        return Ok(Callback::None);
    }
    let Value::Name(name) = name else {
        return Err(script_error(what, "CallbackName is not a name"));
    };
    let target = object(&field(vm, data, "CallbackObject", what)?, what)?.unwrap_or(owner);
    let text = vm.space.names.text(name).to_string();
    let Some(f) = method_of(vm, target, &text)? else {
        return Err(NotYet::not_read(format!("a special-move callback {text} the target does not declare"), RECORD).into());
    };
    let params = vm.function(f).map(|d| d.params.clone()).unwrap_or_default();
    let interrupted_param = match params.as_slice() {
        [] => false,
        [p] if vm.space.names.text(p.name).eq_ignore_ascii_case("bInterrupted") && p.kind == PropKind::Bool => true,
        _ => {
            return Err(NotYet::not_read(
                format!(
                    "a special-move callback {text} with parameters other than one bool bInterrupted (the note names \
                     no other argument)"
                ),
                RECORD,
            )
            .into());
        }
    };
    Ok(Callback::Call { target, f, interrupted_param })
}

/// `NotYet` unless `o`'s class declares `name` (decided before a start changes anything).
fn require_method(vm: &mut Vm, o: ObjRef, name: &str) -> Result<(), VmError> {
    match method_of(vm, o, name)? {
        Some(_) => Ok(()),
        None => Err(NotYet::not_read(
            format!("{name} on {} (the class declares no such function)", ow_attr::eval::class_path(&vm.space, o)),
            RECORD,
        )
        .into()),
    }
}

/// Stops the component's current move `smd` with its resolved callback (module documentation), the queue emptied.
fn stop(
    vm: &mut Vm,
    component: ObjRef,
    owner: ObjRef,
    smd: ObjRef,
    callback: &Callback,
    interrupted: bool,
    zero: &Value,
    what: &str,
) -> Result<(), VmError> {
    write(vm, component, "SMDQueue", Value::array(Vec::new()), what)?;
    write(vm, component, "CurrentSMData", zero.clone(), what)?;
    call(vm, smd, "ClientFinished", vec![interface(owner), Value::Bool(interrupted)])?;
    if let Callback::Call { target, f, interrupted_param } = *callback {
        let args = if interrupted_param { vec![Value::Bool(interrupted)] } else { Vec::new() };
        vm.call(f, Some(target), args)?;
    }
    Ok(())
}

/// What a start needs, every check already passed.
struct Start {
    component: ObjRef,
    owner: ObjRef,
    smd: ObjRef,
    data: Value,
    now: f32,
    zero: Value,
    old: Option<(ObjRef, Callback)>,
    own: Callback,
}

/// The start of module documentation step 4.
fn start(vm: &mut Vm, s: &Start, what: &str) -> Result<Value, VmError> {
    write(vm, s.component, "SMDQueue", Value::array(Vec::new()), what)?;
    if let Some((old, callback)) = &s.old {
        stop(vm, s.component, s.owner, *old, callback, true, &s.zero, what)?;
    }
    write(vm, s.component, "CurrentSMData", s.data.clone(), what)?;
    write(vm, s.component, "StartTime", Value::Float(s.now), what)?;
    let duration = match call(vm, s.smd, "ClientStarted", vec![interface(s.owner), s.data.clone()])? {
        Value::Float(d) => d,
        other => return Err(script_error(what, format!("ClientStarted answered {}", other.kind_name()))),
    };
    if duration <= 0.0 {
        stop(vm, s.component, s.owner, s.smd, &s.own, false, &s.zero, what)?;
    }
    Ok(Value::Float(duration))
}

fn play(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "SpecialMoveComponent.Play";
    let component = this(c, WHAT)?;
    let Some((smd, owner)) = to_play(c, component, WHAT)? else {
        return Ok(Value::Float(0.0));
    };
    let smi = interface(owner);
    let authority = role_at_least(c.vm, owner, "ROLE_Authority", WHAT)?;
    if !authority && is_network_client(c.vm, WHAT)? {
        return Err(NotYet::not_ported(
            "Play by an owner below authority on a network client (the forward to the server, ServerSpecialMove_Play)",
            RECORD,
        )
        .into());
    }
    let passes = is_local_authority_of(c.vm, smd, &smi)? && call_bool(c.vm, smd, "AuthorityCanPlay", vec![smi])?;
    if !passes {
        return Ok(Value::Float(0.0));
    }
    if authority {
        return Err(NotYet::not_read(
            "Play on the authority: the record the owner keeps of the current move for replication (section 1 step \
             4.4 does not name it)",
            RECORD,
        )
        .into());
    }
    if object(&need(c.vm, smd, "NextSpecialMove")?, WHAT)?.is_some() {
        return Err(NotYet::not_read("Play of a definition with a NextSpecialMove (what the queued follow-ups carry)", RECORD).into());
    }
    // Every other refusal, before anything changes (module documentation, step 3).
    let data = request(c, component, smd, WHAT)?;
    let vm = &mut *c.vm;
    let now = world_time(vm)?;
    let ty = data_type(vm, component, WHAT)?;
    let zero = vm.space.classes.struct_defaults(ty).map_err(|e| script_error(WHAT, e.to_string()))?;
    let current = need(vm, component, "CurrentSMData")?;
    let old = match object(&field(vm, &current, "SMD", WHAT)?, WHAT)? {
        Some(o) => {
            require_method(vm, o, "ClientFinished")?;
            Some((o, callback_of(vm, &current, o, owner, &zero, WHAT)?))
        }
        None => None,
    };
    require_method(vm, smd, "ClientStarted")?;
    require_method(vm, smd, "ClientFinished")?;
    let own = callback_of(vm, &data, smd, owner, &zero, WHAT)?;
    let saved = [
        ("CurrentSMData", current),
        ("SMDQueue", need(vm, component, "SMDQueue")?),
        ("StartTime", need(vm, component, "StartTime")?),
    ];
    let s = Start { component, owner, smd, data, now, zero, old, own };
    match start(vm, &s, WHAT) {
        Ok(v) => Ok(v),
        Err(e) => {
            for (name, v) in saved {
                write(vm, component, name, v, WHAT)?;
            }
            Err(e)
        }
    }
}

fn play_local(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "SpecialMoveComponent.PlayLocal";
    let component = this(c, WHAT)?;
    let Some((smd, owner)) = to_play(c, component, WHAT)? else {
        return Ok(Value::Float(0.0));
    };
    let data = request(c, component, smd, WHAT)?;
    call(c.vm, smd, "ServerStarted", vec![interface(owner)])?;
    match call(c.vm, smd, "ClientStarted", vec![interface(owner), data])? {
        Value::Float(d) => Ok(Value::Float(d)),
        other => Err(script_error(WHAT, format!("ClientStarted answered {}", other.kind_name()))),
    }
}

/// Binds the module's natives in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ow_core::ParityMode::Off {
        return Ok(());
    }
    let [alive, to_play_path, local, playing, node, play_path, local_play] = PATHS;
    let natives: [(&str, NativeFn); 7] = [
        (alive, Rc::new(is_alive_and_well)),
        (to_play_path, Rc::new(|c: &mut NativeCall<'_>| Ok(Value::Object(Some(this(c, "GetSMDToPlay")?))))),
        (
            local,
            Rc::new(|c: &mut NativeCall<'_>| {
                let me = this(c, "IsLocalAuthority")?;
                let smi = c.arg(0)?.clone();
                Ok(Value::Bool(is_local_authority_of(c.vm, me, &smi)?))
            }),
        ),
        (playing, Rc::new(is_playing_class)),
        (node, Rc::new(get_sm_node)),
        (play_path, Rc::new(play)),
        (local_play, Rc::new(play_local)),
    ];
    for (path, f) in natives {
        bind(vm, path, f)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "special_move_tests.rs"]
mod tests;
