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
//!   0. the first reads, loaded-only (lane IMPL-SET-REPLICATED-SMDATA, as NATIVE_SPECIAL_MOVE_QUEUE.md section 5):
//!      the component resident; with a `NewSMD` and an `SMI` set, those two and the component's `Owner` resident
//!      ([`NOT_LOADED`]); `Owner` None ([`NO_OWNER`]) or not the `SMI` actor ([`OTHER_OWNER`]) is `NotYet { NotRead }`.
//!      The method checks of step 3 look in loaded code only, and code not loaded is `NotYet { NotRead }`;
//!   1. 0 with no definition or no owner interface (`SMI`); then the definition to play (`GetSMDToPlay`), 0 with none;
//!   2. the gates, in this order: `IsLocalAuthority`, then the script event `AuthorityCanPlay` (not asked when the
//!      first fails; UNVERIFIED). Failed: 0, nothing changed;
//!   3. every refusal that can be decided is decided before the first event and before any write (reviews r1 and r2
//!      of #437), each `NotYet`:
//!      - an owner below `ROLE_Authority` in a world whose `WorldInfo.NetMode` is `NM_Client`: the forward to the
//!        server (`ServerSpecialMove_Play`, step 6) is network play, not ported;
//!      - lane IMPL-SET-REPLICATED-SMDATA (NATIVE_SET_REPLICATED_SMDATA.md, clean room) lifts the old refusal of an
//!        owner at `ROLE_Authority`. When the component's `Owner` is exactly `ROLE_Authority`, the replicated record
//!        ([`RECORD_REPLICATED`]) is checked here, before `AuthorityCanPlay`: the `SMI` actor's first native class is
//!        one of [`REPLICATED_OWNERS`] (else `NotRead`), `ReplicatedSMData` is declared as [`REPLICATED_STRUCT`] with
//!        its five fields of the read kinds and `bNetDirty` as a bool (else `VmError::Script`), and the definitions
//!        whose server events the commit runs declare them in loaded code (`ServerStarted`, and `ServerFinished` for
//!        each stop the start may run; none declared is a `VmError::Script`). These come after the refusals below,
//!        in the note's order;
//!      - a definition with a `NextSpecialMove`: what the queued follow-ups carry is not read;
//!      - a stop this start may run (the current move's, or the new move's own when its duration is 0 or less) whose
//!        definition declares no `ClientFinished`, whose callback the target does not declare, or whose callback has
//!        a parameter other than one `bool bInterrupted`: the note names no other argument, and a zero or a default
//!        would be a guess;
//!   4. the gate event `AuthorityCanPlay`. A node `GetSMNode` finds while it runs is cached in `SMBlendNode` only once
//!      `Play` is committed (the hub's ruling in review r2 of #437; the game caches it at once, UNVERIFIED whether
//!      that is observable). A refusal raised inside it (`GetSMNode`'s missing anim tree, say) is answered as it is:
//!      nothing this port binds writes during it;
//!   5. committed, the start: the node cache written; the queue (`SMDQueue`) emptied; a current move stopped as
//!      interrupted; the request (its definition the one to play) made the current move (`CurrentSMData`) and
//!      `StartTime` the world time; on an exact-authority `Owner`, the record write (`SetReplicatedSMData` with
//!      `CurrentSMData`) and then the definition's `ServerStarted(SMI)`; the definition's `ClientStarted(SMI, SMData)`
//!      answers the duration, and a
//!      duration of 0 or less stops the move again, not interrupted, if it is still the current move. The answer is
//!      the duration.
//!
//!   A refusal after the commit (a script event reaching an unbound native, say) cannot be decided in advance and is
//!   **terminal**: nothing is put back, and its text starts "stopped after partial writes" (the hub's ruling in review
//!   r2 of #437, the same rule as #439), its kind kept.
//!
//!   Stopping a move (section 4), as `Play` uses it (queue cleared): the queue emptied, the current move cleared by
//!   setting only its `SMD` to None (NATIVE_SET_REPLICATED_SMDATA.md section 3; its other fields keep the stopped
//!   move's values), on an exact-authority `Owner` the old definition's `ServerFinished(SMI, bInterrupted)` and then
//!   the record write with `CurrentSMData` as it reads after that event, the old definition's
//!   `ClientFinished(SMI, bInterrupted)`, then the callback when the old
//!   definition passes `IsLocalAuthority` and the request named one: the function `CallbackName` on `CallbackObject`
//!   (or on the owner), with `bInterrupted` when it declares that one parameter, and only if the old definition still
//!   passes `IsLocalAuthority` when the callback is dispatched.
//!
//!   **The record write** (`SetReplicatedSMData`, NATIVE_SET_REPLICATED_SMDATA.md section 2; UNVERIFIED): on the
//!   `SMI` actor, `ReplicatedSMData`'s `SMD`, `Data`, `PlayRateScale` and `Duration` from the move, its
//!   `DirtyCounter` one higher as a byte (255 wraps to 0), and `bNetDirty` set. The callback is not copied. No script
//!   calls the native, so nothing is bound for it.
//! - **`SpecialMoveComponent.PlayLocal(...)`** (section 1): 0 with no definition or owner interface; otherwise the
//!   definition to play (0 with none), its `ServerStarted(SMI)` and then its `ClientStarted(SMI, SMData)`, whose answer
//!   is the answer. No gate, no current move, no `StartTime`, no queue.
//!
//! - **`SpecialMoveComponent.Queue(NewSMD, PlayRateScale = 1, Duration = 0, CallbackName, CallbackObject, Data)`**
//!   (lane IMPL-SPECIALMOVE-QUEUE, coded from NATIVE_SPECIAL_MOVE_QUEUE.md section 5 at 7dcb5cd alone; its record,
//!   [`QUEUE_RECORD`]). No answer. The preflight, write-free, in the note's order:
//!   1. no definition, or no owner interface (`SMI` None): nothing (the component is checked resident before its `SMI`
//!      is read);
//!   2. residency: the component's `Owner` (when set), the definition and the `SMI` actor must be resident (not a lazy
//!      handle, not a stand-in with no slots), else [`QUEUE_NOT_LOADED`]. Only then do the loading helpers read them;
//!   3. no component `Owner`: [`QUEUE_NO_OWNER`];
//!   4. an `Owner` that is not the `SMI` actor: [`QUEUE_OWNER_MISMATCH`] (the forward reads one, the gate the other);
//!   5. the `Owner`'s `Role` below `ROLE_Authority`: [`QUEUE_FORWARD`], the forward to the server is network play, not
//!      ported;
//!   6. `IsLocalAuthority(NewSMD)` (not resolved through `GetSMDToPlay`) false: nothing;
//!   7. `CurrentSMData` a `SpecialMoveData` struct and `SMDQueue` an array of that struct, else a `VmError::Script`
//!      naming the field.
//!
//!   Each refusal is `NotYet { NotRead }`. Then: **no current move** (`CurrentSMData.SMD` None): first, the
//!   definition's `GetSMDToPlay` must resolve without a load (`Vm::find_loaded_method`), else
//!   [`QUEUE_CODE_NOT_LOADED`] (review r2 of #682: `Play`'s dispatch would load cold class code); then `Play`'s own path
//!   here, with the arguments [`play_args`] builds (the first five as given, `Data` None, as the note reads the game),
//!   its answer dropped; whatever it refuses stands, with its own record and its own "stopped after partial writes"
//!   rule (Queue writes nothing before it). **A current move:** one `SpecialMoveData` appended at the end of
//!   `SMDQueue`, every field as given (`Data` included) and `SMD` the definition as given; the only write. Draining
//!   the queue when a move stops is not part of this native and is not ported: an entry queued here waits in Off.
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
/// The record of the replicated-record write (lane IMPL-SET-REPLICATED-SMDATA).
pub const RECORD_REPLICATED: &str = "docs/verification/NATIVE_SET_REPLICATED_SMDATA.md";
/// The first native classes whose record write was read (the note, section 2).
pub const REPLICATED_OWNERS: [&str; 4] = [
    "GearboxFramework.GearboxPawn",
    "WillowGame.WillowPawn",
    "WillowGame.WillowPlayerPawn",
    "WillowGame.WillowAIPawn",
];
/// `ReplicatedSMData`'s struct, by its path in the installed packages: declared in the `SpecialMoveInterface`
/// interface class, the only struct of that name (the runner's names-only read, 2026-11-25; review r1 of #689).
pub const REPLICATED_STRUCT: &str = "GearboxFramework.SpecialMoveInterface.ReplicatedSpecialMoveData";
/// A Play read on an object the store has not loaded.
pub const NOT_LOADED: &str = "SpecialMoveComponent.Play: an object the store has not loaded (a load is a write)";
/// A component with an owner interface and no `Owner`.
pub const NO_OWNER: &str =
    "SpecialMoveComponent.Play: a component with an owner interface and no Owner (the game reads its Role unchecked)";
/// A component whose `Owner` is not its owner interface's actor.
pub const OTHER_OWNER: &str = "SpecialMoveComponent.Play: a component whose Owner is not its owner interface's actor \
                               (the record hook reads Owner's Role, the gate reads SMI)";

/// The record `Queue` follows (lane IMPL-SPECIALMOVE-QUEUE).
pub const QUEUE_RECORD: &str = "NATIVE_SPECIAL_MOVE_QUEUE.md";

/// The refusal for a component with an owner interface and no `Owner` (the note's preflight step 3, at 7dcb5cd).
pub const QUEUE_NO_OWNER: &str =
    "SpecialMoveComponent.Queue: a component with an owner interface and no Owner (the game reads its Role unchecked)";

/// The refusal for an owner below authority (the note's preflight step 5, at 7dcb5cd).
pub const QUEUE_FORWARD: &str =
    "SpecialMoveComponent.Queue: the forward to the server (ServerSpecialMove_Queue) on an owner below authority";

/// The function paths [`register`] binds in Off.
pub const PATHS: [&str; 8] = [
    "Engine.Pawn.IsAliveAndWell",
    "GearboxFramework.SpecialMoveDefinition.GetSMDToPlay",
    "GearboxFramework.SpecialMoveDefinition.IsLocalAuthority",
    "GearboxFramework.SpecialMoveComponent.IsPlayingClass",
    "GearboxFramework.SpecialMoveComponent.GetSMNode",
    "GearboxFramework.SpecialMoveComponent.Play",
    "GearboxFramework.SpecialMoveComponent.PlayLocal",
    "GearboxFramework.SpecialMoveComponent.Queue",
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
            if !defer_cache(c.vm, component, node)? {
                write(c.vm, component, "SMBlendNode", Value::Object(Some(node)), WHAT)?;
            }
            Ok(Value::Object(Some(node)))
        }
        _ => Ok(Value::Object(None)),
    }
}

/// The components whose `Play` is asking its gates, and the node caches their `GetSMNode` found meanwhile, written
/// only once that `Play` is committed (review r2 of #437, the hub's ruling).
#[derive(Debug, Default)]
struct Deferred {
    components: Vec<ObjRef>,
    pending: Vec<(ObjRef, ObjRef)>,
}

fn deferred(vm: &Vm) -> Result<Rc<std::cell::RefCell<Deferred>>, VmError> {
    vm.service::<Deferred>()
        .ok_or_else(|| script_error("SpecialMoveComponent.Play", "the special-move natives were not registered"))
}

fn borrow_err<E>(_: E) -> VmError {
    VmError::ServiceBorrowed("special_move::Deferred")
}

/// Records `node` as `component`'s pending cache when its `Play` is asking its gates; false otherwise.
fn defer_cache(vm: &mut Vm, component: ObjRef, node: ObjRef) -> Result<bool, VmError> {
    let Some(d) = vm.service::<Deferred>() else { return Ok(false) };
    let mut d = d.try_borrow_mut().map_err(borrow_err)?;
    if !d.components.contains(&component) {
        return Ok(false);
    }
    d.pending.retain(|(c, _)| *c != component);
    d.pending.push((component, node));
    Ok(true)
}

/// `AuthorityCanPlay` with `component`'s cache writes deferred; answers the gate and the node to cache, if any.
fn authority_can_play(vm: &mut Vm, component: ObjRef, smd: ObjRef, smi: Value) -> Result<(bool, Option<ObjRef>), VmError> {
    let d = deferred(vm)?;
    d.try_borrow_mut().map_err(borrow_err)?.components.push(component);
    let r = call_bool(vm, smd, "AuthorityCanPlay", vec![smi]);
    let mut d = d.try_borrow_mut().map_err(borrow_err)?;
    if let Some(i) = d.components.iter().rposition(|c| *c == component) {
        d.components.remove(i);
    }
    let node = d.pending.iter().position(|(c, _)| *c == component).map(|i| d.pending.remove(i).1);
    Ok((r?, node))
}

/// A refusal that comes after `Play` dispatched its first committed event: terminal, its text saying so (review r2
/// of #437, the same rule as #439). The kind of the refusal is kept.
fn after_partial_writes(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) => NotYet::new(format!("stopped after partial writes: {}", n.what), n.missing, n.record).into(),
        other => script_error("SpecialMoveComponent.Play", format!("stopped after partial writes: {other}")),
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
fn to_play(
    c: &mut NativeCall<'_>,
    component: ObjRef,
    loaded_only: bool,
    what: &str,
) -> Result<Option<(ObjRef, ObjRef)>, VmError> {
    let Some(requested) = object(c.arg(0)?, what)? else { return Ok(None) };
    let Some(owner) = owner_of(c.vm, component, what)? else { return Ok(None) };
    // Play looks it up in loaded code only, so a cold definition class is refused, not loaded (lane
    // IMPL-SET-REPLICATED-SMDATA). PlayLocal has no write-free preflight and calls its events through the loading
    // lookup anyway, so it keeps that lookup here too (review r1 of #689, item 3).
    if loaded_only {
        require_method(c.vm, requested, "GetSMDToPlay")?;
    }
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

/// `NotYet { NotRead }` ([`NOT_LOADED`]) unless `o` is resident: not a store handle still unloaded (or failed), and
/// not a stand-in with fewer slots than its class.
fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(NOT_LOADED, RECORD_REPLICATED).into());
    }
    Ok(())
}

/// Field `name` of the resident `o`, by name lookup only (no load); a class that declares none is `VmError::Script`.
fn loaded_field(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script_error(what, format!("the class declares no {name}")))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

/// `name` as `o`'s class sees it, in loaded code only (lane IMPL-SET-REPLICATED-SMDATA): code on the chain not loaded
/// yet is `NotYet { NotRead }`. A class with no package code (defined by hand, no code key) falls back to its declared
/// functions by path, as [`method_of`] does. That lookup cannot load: such a class has no package export, so a path
/// not declared is a confirmed absence. It is checked anyway (review r1 of #689, item 4): if a store load happened or
/// a name was interned (as a code load does), the lookup fails as a script error. A lookup error is returned, not
/// hidden.
fn loaded_method(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Option<FuncId>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    match vm.find_loaded_method(class, name, None) {
        Some(Some(f)) => return Ok(Some(f)),
        Some(None) => {}
        None => {
            let path = ow_attr::eval::class_path(&vm.space, o);
            return Err(NotYet::not_read(format!("{name} on {path}: the class's code is not loaded (a load is a write)"), RECORD).into());
        }
    }
    let mut cursor = Some(class);
    while let Some(c) = cursor {
        let layout = vm.space.classes.layout(c);
        let path = format!("{}.{name}", layout.path().as_str());
        cursor = layout.super_class();
        if vm.defs.code().key_of(c).is_some() {
            continue;
        }
        // A code load interns the names it declares; `Vm::class_code_loads` (#685) joins this check once on main.
        let before = (vm.defs.load_transitions(), vm.space.names.len());
        let found = vm.find_function_if_any(&path)?;
        if (vm.defs.load_transitions(), vm.space.names.len()) != before {
            return Err(script_error("SpecialMoveComponent.Play", format!("looking up {path} loaded code")));
        }
        if let Some(f) = found {
            return Ok(Some(f));
        }
    }
    Ok(None)
}

/// A server event the commit will call, resolved in loaded code before the commit: code not loaded is `NotYet {
/// NotRead }` (a load is a write); a class that declares none is a `VmError::Script` (the note's section 4).
fn require_event(vm: &mut Vm, o: ObjRef, name: &str) -> Result<(), VmError> {
    match loaded_method(vm, o, name)? {
        Some(_) => Ok(()),
        None => Err(script_error(
            "SpecialMoveComponent.Play",
            format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o)),
        )),
    }
}

/// Whether `o`'s `Role` is exactly `ROLE_Authority` (the record hook's test; the local-authority gate keeps its
/// at-least comparison).
fn exact_authority(vm: &mut Vm, o: ObjRef, what: &str) -> Result<bool, VmError> {
    let (raw, index) = enum_order(vm, o, "Role", "ROLE_Authority", what)?;
    Ok(usize::from(raw) == index)
}

/// The record write's target, checked by [`record_preflight`].
#[derive(Debug, Clone, Copy)]
struct Record {
    receiver: ObjRef,
    replicated: ow_object::ids::PropId,
    dirty: ow_object::ids::PropId,
}

/// The record's preflight on `receiver`, the `SMI` actor (module documentation): reads declarations only.
fn record_preflight(vm: &Vm, receiver: ObjRef) -> Result<Record, VmError> {
    const WHAT: &str = "SpecialMoveComponent: the replicated record";
    let bad = |m: String| script_error(WHAT, m);
    let class = vm.space.heap.get(receiver)?.class;
    // The first native class from the receiver's class up (the fitted native bit, as `ow-combat`'s read-only walk).
    let code = vm.defs.code();
    let mut cursor = Some(class);
    let mut first_native = None;
    while let Some(c) = cursor {
        let layout = vm.space.classes.layout(c);
        let native = match code.key_of(c) {
            Some(k) => code.class(k).body.as_ref().map_or(true, |b| b.is_native()),
            None => vm.space.classes.is_native(c),
        };
        if native {
            first_native = Some(layout.path().to_string());
            break;
        }
        cursor = layout.super_class();
    }
    let read = first_native.as_deref().is_some_and(|p| REPLICATED_OWNERS.iter().any(|r| r.eq_ignore_ascii_case(p)));
    if !read {
        let path = first_native.unwrap_or_else(|| ow_attr::eval::class_path(&vm.space, receiver));
        return Err(NotYet::not_read(format!("SpecialMoveComponent: the replicated record of {path}"), RECORD_REPLICATED).into());
    }
    let replicated = vm.space.prop(class, "ReplicatedSMData").ok_or_else(|| bad("the owner declares no ReplicatedSMData".into()))?;
    let ty = match vm.space.classes.decl(replicated).map(|d| (&d.kind, d.array_dim)) {
        Some((PropKind::Struct(ty), 1)) => *ty,
        _ => return Err(bad("ReplicatedSMData is not declared as a struct".into())),
    };
    if vm.space.classes.struct_by_path(REPLICATED_STRUCT) != Some(ty) {
        return Err(bad(format!("ReplicatedSMData is a struct of another type, not a {REPLICATED_STRUCT}")));
    }
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| bad("ReplicatedSMData's struct has no layout".into()))?;
    let definition = vm.space.classes.by_path("GearboxFramework.SpecialMoveDefinition");
    for (name, want) in [("SMD", "definition"), ("Data", "object"), ("PlayRateScale", "float"), ("Duration", "float"), ("DirtyCounter", "byte")] {
        let decl = vm.space.names.find(name).and_then(|id| layout.field(id)).map(|(_, d)| (d.kind.clone(), d.array_dim));
        let fits = match (want, &decl) {
            ("definition", Some((PropKind::Object { class }, 1))) => definition == Some(*class),
            ("object", Some((PropKind::Object { .. }, 1))) => true,
            ("float", Some((PropKind::Float, 1))) => true,
            ("byte", Some((PropKind::Byte { .. }, 1))) => true,
            _ => false,
        };
        if !fits {
            return Err(bad(format!("ReplicatedSMData.{name} is not declared as {want}")));
        }
    }
    let dirty = vm.space.prop(class, "bNetDirty").ok_or_else(|| bad("the owner declares no bNetDirty".into()))?;
    if !matches!(vm.space.classes.decl(dirty).map(|d| &d.kind), Some(PropKind::Bool)) {
        return Err(bad("bNetDirty is not declared as a bool".into()));
    }
    Ok(Record { receiver, replicated, dirty })
}

/// The record write (`SetReplicatedSMData`, module documentation) from the move `data`.
fn write_record(vm: &mut Vm, record: &Record, data: &Value, what: &str) -> Result<(), VmError> {
    let mut rep = vm.space.read(record.receiver, PropAddr::scalar(record.replicated))?.clone();
    for name in ["SMD", "Data", "PlayRateScale", "Duration"] {
        let v = field(vm, data, name, what)?;
        rep = with_field(vm, rep, name, v, what)?;
    }
    let counter = match field(vm, &rep, "DirtyCounter", what)? {
        Value::Byte(b) => b.wrapping_add(1),
        other => return Err(script_error(what, format!("ReplicatedSMData.DirtyCounter holds {}", other.kind_name()))),
    };
    rep = with_field(vm, rep, "DirtyCounter", Value::Byte(counter), what)?;
    vm.space.write(record.receiver, PropAddr::scalar(record.replicated), rep)?;
    vm.space.write(record.receiver, PropAddr::scalar(record.dirty), Value::Bool(true))?;
    Ok(())
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
    resident(vm, target)?;
    let Some(f) = loaded_method(vm, target, &text)? else {
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

/// `NotYet` unless `o`'s class declares `name` in loaded code (decided before a start changes anything; loads
/// nothing, lane IMPL-SET-REPLICATED-SMDATA).
fn require_method(vm: &mut Vm, o: ObjRef, name: &str) -> Result<(), VmError> {
    match loaded_method(vm, o, name)? {
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
    record: Option<&Record>,
    what: &str,
) -> Result<(), VmError> {
    write(vm, component, "SMDQueue", Value::array(Vec::new()), what)?;
    // Only the SMD is cleared; the other fields keep the stopped move's values (NATIVE_SET_REPLICATED_SMDATA.md
    // section 3).
    let current = need(vm, component, "CurrentSMData")?;
    let cleared = with_field(vm, current, "SMD", Value::Object(None), what)?;
    write(vm, component, "CurrentSMData", cleared, what)?;
    if let Some(record) = record {
        call(vm, smd, "ServerFinished", vec![interface(owner), Value::Bool(interrupted)])?;
        // The record copies CurrentSMData as it reads after the event.
        let after = need(vm, component, "CurrentSMData")?;
        write_record(vm, record, &after, what)?;
    }
    call(vm, smd, "ClientFinished", vec![interface(owner), Value::Bool(interrupted)])?;
    // The authority is asked again at dispatch: `ClientFinished` may have changed it since the callback was resolved.
    if let Callback::Call { target, f, interrupted_param } = *callback
        && is_local_authority_of(vm, smd, &interface(owner))?
    {
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
    old: Option<(ObjRef, Callback)>,
    own: Callback,
    /// The record to write on an exact-authority owner (lane IMPL-SET-REPLICATED-SMDATA).
    record: Option<Record>,
}

/// The start of module documentation step 4.
fn start(vm: &mut Vm, s: &Start, what: &str) -> Result<Value, VmError> {
    write(vm, s.component, "SMDQueue", Value::array(Vec::new()), what)?;
    if let Some((old, callback)) = &s.old {
        stop(vm, s.component, s.owner, *old, callback, true, s.record.as_ref(), what)?;
    }
    write(vm, s.component, "CurrentSMData", s.data.clone(), what)?;
    write(vm, s.component, "StartTime", Value::Float(s.now), what)?;
    if let Some(record) = &s.record {
        write_record(vm, record, &s.data, what)?;
        call(vm, s.smd, "ServerStarted", vec![interface(s.owner)])?;
    }
    let duration = match call(vm, s.smd, "ClientStarted", vec![interface(s.owner), s.data.clone()])? {
        Value::Float(d) => d,
        other => return Err(script_error(what, format!("ClientStarted answered {}", other.kind_name()))),
    };
    // The current move is read again first: if `ClientStarted` already ended it, it is not stopped (and its
    // `ClientFinished` not run) a second time.
    let current = need(vm, s.component, "CurrentSMData")?;
    if duration <= 0.0 && object(&field(vm, &current, "SMD", what)?, what)? == Some(s.smd) {
        stop(vm, s.component, s.owner, s.smd, &s.own, false, s.record.as_ref(), what)?;
    }
    Ok(Value::Float(duration))
}

fn play(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "SpecialMoveComponent.Play";
    let component = this(c, WHAT)?;
    // Step 0: the first reads, loaded-only (module documentation).
    resident(c.vm, component)?;
    let requested = object(c.arg(0)?, WHAT)?;
    let smi_actor = object(&loaded_field(c.vm, component, "SMI", WHAT)?, WHAT)?;
    let (Some(requested), Some(smi_actor)) = (requested, smi_actor) else {
        return Ok(Value::Float(0.0));
    };
    let owner_field = object(&loaded_field(c.vm, component, "Owner", WHAT)?, WHAT)?;
    if let Some(o) = owner_field {
        resident(c.vm, o)?;
    }
    resident(c.vm, smi_actor)?;
    resident(c.vm, requested)?;
    match owner_field {
        None => return Err(NotYet::not_read(NO_OWNER, RECORD_REPLICATED).into()),
        Some(o) if o != smi_actor => return Err(NotYet::not_read(OTHER_OWNER, RECORD_REPLICATED).into()),
        Some(_) => {}
    }
    let Some((smd, owner)) = to_play(c, component, true, WHAT)? else {
        return Ok(Value::Float(0.0));
    };
    // The definition GetSMDToPlay answers: a script override can name another one, which must be resident too before
    // any read of it (review r1 of #689, item 1).
    resident(c.vm, smd)?;
    let smi = interface(owner);
    // Every refusal that can be decided is decided before the first event and before any write (module
    // documentation, step 3; review r2 of #437).
    let authority = role_at_least(c.vm, owner, "ROLE_Authority", WHAT)?;
    if !authority && is_network_client(c.vm, WHAT)? {
        return Err(NotYet::not_ported(
            "Play by an owner below authority on a network client (the forward to the server, ServerSpecialMove_Play)",
            RECORD,
        )
        .into());
    }
    if !is_local_authority_of(c.vm, smd, &smi)? {
        return Ok(Value::Float(0.0));
    }
    if object(&need(c.vm, smd, "NextSpecialMove")?, WHAT)?.is_some() {
        return Err(NotYet::not_read("Play of a definition with a NextSpecialMove (what the queued follow-ups carry)", RECORD).into());
    }
    let data = request(c, component, smd, WHAT)?;
    let vm = &mut *c.vm;
    let now = world_time(vm)?;
    let ty = data_type(vm, component, WHAT)?;
    let zero = vm.space.classes.struct_defaults(ty).map_err(|e| script_error(WHAT, e.to_string()))?;
    let current = need(vm, component, "CurrentSMData")?;
    let old = match object(&field(vm, &current, "SMD", WHAT)?, WHAT)? {
        Some(o) => {
            resident(vm, o)?;
            require_method(vm, o, "ClientFinished")?;
            Some((o, callback_of(vm, &current, o, owner, &zero, WHAT)?))
        }
        None => None,
    };
    require_method(vm, smd, "ClientStarted")?;
    require_method(vm, smd, "ClientFinished")?;
    let own = callback_of(vm, &data, smd, owner, &zero, WHAT)?;
    // Step 4 of NATIVE_SET_REPLICATED_SMDATA.md's preflight, after Play's own refusals: only when the Owner (here the
    // SMI actor, checked above) is exactly ROLE_Authority, the record and the server events the commit will call.
    let record = if exact_authority(vm, owner, WHAT)? {
        let record = record_preflight(vm, owner)?;
        require_event(vm, smd, "ServerStarted")?;
        require_event(vm, smd, "ServerFinished")?;
        if let Some((o, _)) = &old {
            require_event(vm, *o, "ServerFinished")?;
        }
        Some(record)
    } else {
        None
    };
    // The gate event. The node GetSMNode finds meanwhile is cached only once Play is committed.
    let (passes, node) = authority_can_play(vm, component, smd, smi)?;
    if !passes {
        return Ok(Value::Float(0.0));
    }
    // Committed: from here a refusal is terminal (module documentation).
    let s = Start { component, owner, smd, data, now, old, own, record };
    let committed = |vm: &mut Vm| -> Result<Value, VmError> {
        if let Some(node) = node {
            write(vm, component, "SMBlendNode", Value::Object(Some(node)), WHAT)?;
        }
        start(vm, &s, WHAT)
    };
    committed(vm).map_err(after_partial_writes)
}

fn play_local(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "SpecialMoveComponent.PlayLocal";
    let component = this(c, WHAT)?;
    let Some((smd, owner)) = to_play(c, component, false, WHAT)? else {
        return Ok(Value::Float(0.0));
    };
    let data = request(c, component, smd, WHAT)?;
    call(c.vm, smd, "ServerStarted", vec![interface(owner)])?;
    match call(c.vm, smd, "ClientStarted", vec![interface(owner), data])? {
        Value::Float(d) => Ok(Value::Float(d)),
        other => Err(script_error(WHAT, format!("ClientStarted answered {}", other.kind_name()))),
    }
}

/// The element struct of `SMDQueue`, which must be `ty` (`CurrentSMData`'s), else a `VmError::Script` naming it.
fn require_queue_of(vm: &Vm, component: ObjRef, ty: StructId, what: &str) -> Result<(), VmError> {
    let class = vm.space.heap.get(component)?.class;
    let p = vm.space.prop(class, "SMDQueue").ok_or_else(|| script_error(what, "no SMDQueue"))?;
    match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(PropKind::Array(inner)) if **inner == PropKind::Struct(ty) => Ok(()),
        _ => Err(script_error(what, "SMDQueue is not an array of CurrentSMData's struct (SpecialMoveData)")),
    }
}

/// The refusal for a definition whose class code is not loaded, on the delegated path (review r2 of #682): `Play`'s
/// path looks up `GetSMDToPlay` on it, and that lookup would load the class's code (a write: it registers functions
/// and interns names).
pub const QUEUE_CODE_NOT_LOADED: &str = "SpecialMoveComponent.Queue: the definition's class code is not loaded \
     (Play's GetSMDToPlay lookup would load it; a load is a write)";

/// The residency refusal's text (the note's section 5, "Loaded-only reads").
pub const QUEUE_NOT_LOADED: &str = "SpecialMoveComponent.Queue: an object the store has not loaded (a load is a write)";

/// The refusal for an `Owner` that is not the owner interface's actor (the note's preflight step 4).
pub const QUEUE_OWNER_MISMATCH: &str = "SpecialMoveComponent.Queue: a component whose Owner is not its owner \
     interface's actor (the forward reads Owner, the gate reads SMI)";

/// `NotYet { NotRead }` ([`QUEUE_NOT_LOADED`]) for an object that is not resident: a handle the store has not loaded,
/// or a stand-in with no slots (as `Actor.Trace`'s residency check). The loading helpers (`need`) are used only on
/// objects that passed this, so they load nothing.
fn loaded(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let obj = vm.space.heap.get(o)?;
    let full = u32::try_from(obj.slots.len()).ok() == vm.space.classes.get_layout(obj.class).map(|l| l.slot_count());
    if vm.defs.is_unloaded(&vm.space, o) || !full {
        return Err(NotYet::not_read(QUEUE_NOT_LOADED, QUEUE_RECORD).into());
    }
    Ok(())
}

/// The arguments `Queue` hands to `Play`'s path (case 3a): the first five as the caller gave them (an omitted one
/// stays omitted), and `Data` passed as None whatever the caller gave (the note reads the game dropping it).
fn play_args(c: &NativeCall<'_>) -> Result<Vec<ow_vm::api::Arg>, VmError> {
    let mut args: Vec<ow_vm::api::Arg> = (0..5.min(c.arg_count()))
        .map(|i| Ok(ow_vm::api::Arg::new(c.arg(i)?.clone(), false, c.has(i))))
        .collect::<Result<_, VmError>>()?;
    args.resize_with(5, || ow_vm::api::Arg::new(Value::None, false, false));
    args.push(ow_vm::api::Arg::new(Value::Object(None), false, true));
    Ok(args)
}

/// `Queue` (module documentation; NATIVE_SPECIAL_MOVE_QUEUE.md section 5 at 7dcb5cd).
fn queue(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "SpecialMoveComponent.Queue";
    let component = this(c, WHAT)?;
    // 1. Nothing to queue: no definition, or no owner interface.
    let Some(requested) = object(c.arg(0)?, WHAT)? else { return Ok(Value::None) };
    loaded(c.vm, component)?;
    let smi = need(c.vm, component, "SMI")?;
    let Some(interface_actor) = object(&smi, WHAT)? else {
        return Ok(Value::None);
    };
    // 2. Residency of everything the preflight and Play's path read, before any of it is read.
    let owner = object(&need(c.vm, component, "Owner")?, WHAT)?;
    for o in owner.into_iter().chain([requested, interface_actor]) {
        loaded(c.vm, o)?;
    }
    // 3. to 5. The Owner, the same actor as the owner interface's, and its Role (the forward is not ported).
    let Some(owner) = owner else {
        return Err(NotYet::not_read(QUEUE_NO_OWNER, QUEUE_RECORD).into());
    };
    if owner != interface_actor {
        return Err(NotYet::not_read(QUEUE_OWNER_MISMATCH, QUEUE_RECORD).into());
    }
    if !role_at_least(c.vm, owner, "ROLE_Authority", WHAT)? {
        return Err(NotYet::not_read(QUEUE_FORWARD, QUEUE_RECORD).into());
    }
    // 6. The authority gate, on the definition as given.
    if !is_local_authority_of(c.vm, requested, &smi)? {
        return Ok(Value::None);
    }
    // 7. The fields' shapes, then the current move.
    let ty = data_type(c.vm, component, WHAT)?;
    require_queue_of(c.vm, component, ty, WHAT)?;
    let current = need(c.vm, component, "CurrentSMData")?;
    if object(&field(c.vm, &current, "SMD", WHAT)?, WHAT)?.is_none() {
        // 3a: Play's path; its answer dropped, its refusals as they are. Its first step dispatches `GetSMDToPlay` on
        // the definition, by a lookup that loads cold class code; so the lookup must answer without a load first
        // (review r2 of #682). With the code loaded, that lookup loads nothing.
        let (class, state) = {
            let d = c.vm.space.heap.get(requested)?;
            (d.class, d.state)
        };
        if c.vm.find_loaded_method(class, "GetSMDToPlay", state).is_none() {
            return Err(NotYet::not_read(QUEUE_CODE_NOT_LOADED, QUEUE_RECORD).into());
        }
        let f = c.vm.find_function(PATHS[5])?;
        let args = play_args(c)?;
        let mut inner = NativeCall::new(&mut *c.vm, f, Some(component), args);
        play(&mut inner)?;
        return Ok(Value::None);
    }
    // 3b: one entry appended, every field as given, the definition as given.
    let entry = request(c, component, requested, WHAT)?;
    let mut list = need(c.vm, component, "SMDQueue")?;
    list.array_mut().ok_or_else(|| script_error(WHAT, "SMDQueue is not an array"))?.push(entry);
    write(c.vm, component, "SMDQueue", list, WHAT)?;
    Ok(Value::None)
}

/// Binds the module's natives in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ow_core::ParityMode::Off {
        return Ok(());
    }
    if vm.service::<Deferred>().is_none() {
        vm.add_service(Deferred::default())?;
    }
    let [alive, to_play_path, local, playing, node, play_path, local_play, queue_path] = PATHS;
    let natives: [(&str, NativeFn); 8] = [
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
        (queue_path, Rc::new(queue)),
    ];
    for (path, f) in natives {
        bind(vm, path, f)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "special_move_tests.rs"]
mod tests;
