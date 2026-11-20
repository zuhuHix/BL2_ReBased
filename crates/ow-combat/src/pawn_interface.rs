//! Owner: lane IMPL-PAWN-AI-INTERFACE. How a pawn answers `Object.QueryInterface` in Off: `Pawn`'s rule, and
//! `WillowAIPawn`'s one extra step. AI-assisted (Claude Code). Decision 2026-11-19-62.
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_PAWN_AI_INTERFACE.md` alone (the analyst was lane
//! ANALYST-PAWN-AI-INTERFACE, decision 2026-11-19-51), by a lane that did not read the natives. Every rule is
//! **UNVERIFIED** in the running game.
//!
//! **Pawn's rule** ([`pawn_answer`], the note's section 2). Asked for interface class `I`, a pawn gives the first answer
//! that is not none of:
//! 1. **its own class**: the pawn itself when its class or a parent lists `I` (the plain `Object` answer,
//!    [`Vm::plain_interface`]);
//! 2. **one controller**: its `Controller` when set, whatever that controller answers (a declining controller does not
//!    lead to the vehicle); only when `Controller` is not set, the `Controller` of its `DrivenVehicle`, when both are
//!    set;
//! 3. **its `Mesh`**, when set;
//! 4. otherwise none.
//!
//! The three fields are read by name and must be declared (a class without one is an error, never "not set"). A
//! question that comes back to an object already answering it gets none ([`Vm::query_interface`]).
//!
//! **What stays `NotYet` in Off** (the note's "Open" list, never a guess). Steps 2 and 3 hand the question to another
//! object, which "answers by its own rule": [`Vm::query_interface`], the same dispatch as a direct `QueryInterface`
//! on that object (review r1 of #456, item 3). The answers the note leaves open are bound in Off as rules that are
//! `NotYet { NotRead }`, so a direct question and a handed-on one meet the same refusal:
//! - (`GearboxMind`'s answer was open here; it is `Controller`'s rule now, [`read_only`], lane
//!   IMPL-GEARBOXMIND-QUERYINTERFACE, from NATIVE_GEARBOXMIND_QUERYINTERFACE.md);
//! - `WillowPlayerController`'s answer for `IAIInterface` (other interfaces keep the plain answer: the note's open item
//!   is that one question);
//! - a component class other than exactly `ActorComponent`, `SkeletalMeshComponent`, `AIComponent` and
//!   `WillowAIComponent`, for every interface (below).
//!
//! **A component's answer** ([`component_answer`], the note's section 6.4 at 17b9c330, lane
//! IMPL-LINK11-QUERY-INTERFACE): for a receiver whose class is exactly `Engine.ActorComponent` or
//! `Engine.SkeletalMeshComponent`, the plain answer, then, only after a successful none, its `Owner`'s answer through
//! [`Vm::query_interface`] (whose cycle guard gives none for an object already answering). The rule is registered on
//! both classes; the VM lets every other component class inherit it, so the rule tests the exact class first and any
//! other class is the open `NotYet { NotRead }`, never `ActorComponent`'s rule by inheritance. It writes and loads
//! nothing: an unloaded receiver or `Owner` is `NotYet { NotRead }`.
//!
//! **The AI components** (NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md, #600 at e7dcddec; lane
//! IMPL-WILLOWAICOMPONENT-QUERYINTERFACE): exactly `GearboxFramework.AIComponent` and `WillowGame.WillowAIComponent`
//! hold `ActorComponent`'s body too, so they take the same rule, with one Off refusal (the hub's ruling): `Owner` None on
//! a component whose `bAttached` is false is `NotYet { NotRead }`, because the port does not run the attach step that
//! sets `Owner` and so cannot tell "never attached" from "not attached by the port". `Owner` None on an attached one
//! answers none. A spawned mind's component gets its `Owner` on the spawn path (#610), so the refusal is reachable
//! for a map-placed mind's component. The check reads `bAttached` only once `Owner` reads None; undeclared or not a
//! bool is a `VmError::Script`. `ActorComponent` and `SkeletalMeshComponent` keep "`Owner` None answers none". Their
//! subclasses keep the open answer. `WillowAIComponent.PhaseLockOnHold` is `crate::holds`'.
//!
//! **`WillowAIPawn`** ([`willow_ai_pawn_answer`], section 3): `Pawn`'s rule, then, only when that gives none and the
//! asked class is `IPlayerMaster`, its player master. That field's identification is open (UNVERIFIED), so the step
//! is `NotYet`. For `IAIInterface` a `WillowAIPawn` answers exactly as `Pawn` does.
//!
//! **`WillowVehicle`** answers its own way (the note's "Vehicles", recorded for completeness and not on the slice path):
//! not implemented, so a vehicle's question is `NotYet` rather than `Pawn`'s rule.
//!
//! **Reference** (R3) registers none of this: `crate::hit::ai_interface_for` keeps the C++ stand-in there.

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef};
use ow_object::value::Value;
use ow_vm::api::{InterfaceAsking, InterfaceRuleFn, ReadOnlyInterfaceRuleFn, Vm};
use ow_vm::error::VmError;

use crate::hit::access::object_of;
use std::collections::BTreeMap;

/// The analyst note this module implements.
pub const RECORD: &str = "docs/verification/NATIVE_PAWN_AI_INTERFACE.md";

/// The classes whose answers are registered.
pub const PAWN: &str = "Engine.Pawn";
pub const ACTOR_COMPONENT: &str = "Engine.ActorComponent";
pub const SKELETAL_MESH_COMPONENT: &str = "Engine.SkeletalMeshComponent";
pub const AI_COMPONENT: &str = "GearboxFramework.AIComponent";
pub const WILLOW_AI_COMPONENT: &str = "WillowGame.WillowAIComponent";
/// The classes whose answer is `ActorComponent`'s body (the note's section 6.1, and
/// NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md section 1 for the AI components), compared exactly.
pub const COMPONENT_RULE_CLASSES: [&str; 4] = [ACTOR_COMPONENT, SKELETAL_MESH_COMPONENT, AI_COMPONENT, WILLOW_AI_COMPONENT];
/// The component classes whose unattached `Owner` None is refused in Off (module notes, the hub's ruling).
pub const ATTACH_CHECKED_CLASSES: [&str; 2] = [AI_COMPONENT, WILLOW_AI_COMPONENT];
/// The record of the AI components' rules.
pub const AI_COMPONENT_RECORD: &str = "docs/verification/NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md";
pub const WILLOW_AI_PAWN: &str = "WillowGame.WillowAIPawn";
pub const WILLOW_VEHICLE: &str = "WillowGame.WillowVehicle";

/// The interface `Behavior_AIHold` asks for, and the one `WillowAIPawn` adds a step for.
pub const AI_INTERFACE: &str = "GearboxFramework.IAIInterface";
pub const PLAYER_MASTER_INTERFACE: &str = "WillowGame.IPlayerMaster";

/// The classes whose own answer the note leaves open, with how it names them, and whether the open item is the
/// `IAIInterface` question only (module notes). `GearboxMind`'s answer is no longer open: it is `Controller`'s
/// ([`read_only`], lane IMPL-GEARBOXMIND-QUERYINTERFACE).
pub const OPEN_ANSWERS: [(&str, &str, bool); 1] = [
    ("WillowGame.WillowPlayerController", "WillowPlayerController's QueryInterface answer for IAIInterface", true),
];

/// Registers `Pawn`'s, `WillowAIPawn`'s and `WillowVehicle`'s answers, and the open answers as `NotYet` rules, each
/// with its read-only form ([`read_only`]); `Controller`'s rule (for `Engine.Controller` and `WillowGame.WillowMind`)
/// and `Engine.PlayerController`'s `NotYet`, both read-only rules (lane IMPL-GEARBOXMIND-QUERYINTERFACE). In Off, for
/// each class the installed packages hold; nothing under Reference. A class that already has an answer keeps it (and
/// a read-only form it has). A class lookup that fails for another reason than the class being absent is returned as
/// the error, so a class that is present but fails to load never quietly takes a parent's rule (review r1 of #456,
/// item 4).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let vehicle_read_only: ReadOnlyInterfaceRuleFn =
        Rc::new(|vm: &Vm, _asking: &mut InterfaceAsking, o: ObjRef, iface: ClassId| Err(vehicle_not_yet(vm, o, iface)));
    // Each rule with its read-only form, or none: the component rule has no read-only form yet, so the read-only chain
    // answers `NotPorted` where it reaches a component (the hub's ruling in review r1 of #602, item 1).
    let mut rules: Vec<(&str, InterfaceRuleFn, Option<ReadOnlyInterfaceRuleFn>)> = vec![
        (PAWN, Rc::new(pawn_answer), Some(Rc::new(read_only::pawn_read_only))),
        (WILLOW_AI_PAWN, Rc::new(willow_ai_pawn_answer), Some(Rc::new(read_only::willow_ai_pawn_read_only))),
        (WILLOW_VEHICLE, Rc::new(vehicle_answer), Some(vehicle_read_only)),
    ];
    // The component rule's `Owner` declarations, judged once here from the class metadata (review r1 of #524, item
    // 3): a declared target the class table has only reserved (its superclass not filled in) is defined now, so its
    // ancestry is read from the packages and never from a half-built runtime chain.
    // The AI components' `bAttached` declarations are judged here too (NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md r1).
    let mut owners = BTreeMap::new();
    for path in COMPONENT_RULE_CLASSES {
        if let Some(class) = vm.find_class_if_present(path)? {
            let owner = owner_declaration(vm, class)?;
            let attached = ATTACH_CHECKED_CLASSES.contains(&path).then(|| attached_declaration(vm, class));
            owners.insert(class, ComponentDeclarations { owner, attached });
        }
    }
    let owners = Rc::new(owners);
    for path in COMPONENT_RULE_CLASSES {
        let owners = owners.clone();
        rules.push((path, Rc::new(move |vm: &mut Vm, this: ObjRef, iface: ClassId| component_answer(vm, this, iface, &owners)), None));
    }
    for (path, words, ai_only) in OPEN_ANSWERS {
        rules.push((path, open_answer(words, ai_only), Some(read_only::open_read_only(words, ai_only, RECORD))));
    }
    for (path, rule, form) in rules {
        let Some(class) = vm.find_class_if_present(path)? else {
            continue;
        };
        if !vm.has_interface_rule(class) {
            vm.register_interface_rule(class, rule)?;
            if let Some(form) = form {
                vm.register_read_only_form(class, form)?;
            }
        }
    }
    let mut controllers: Vec<(&str, ReadOnlyInterfaceRuleFn)> =
        read_only::CONTROLLER_RULES.iter().map(|&(path, pawn)| (path, read_only::controller_rule(pawn))).collect();
    controllers.push((read_only::PLAYER_CONTROLLER, read_only::player_controller_rule()));
    for (path, rule) in controllers {
        let Some(class) = vm.find_class_if_present(path)? else {
            continue;
        };
        if !vm.has_interface_rule(class) && !vm.has_read_only_interface_rule(class) {
            vm.register_read_only_interface_rule(class, rule)?;
        }
    }
    Ok(())
}

/// An open answer's rule: `NotYet { NotRead }` naming it (for `IAIInterface` only when `ai_only`; the plain answer
/// for the other interfaces then).
fn open_answer(words: &'static str, ai_only: bool) -> InterfaceRuleFn {
    Rc::new(move |vm: &mut Vm, this: ObjRef, iface: ClassId| {
        if ai_only && !is_interface(vm, iface, AI_INTERFACE)? {
            return vm.plain_interface(this, iface);
        }
        Err(NotYet::not_read(
            format!(
                "{words} is not read (asked of {} for {})",
                class_path(vm, vm.space.heap.get(this)?.class),
                class_path(vm, iface)
            ),
            RECORD,
        )
        .into())
    })
}

/// Whether `iface` is exactly the class at `path` (an absent class is never it; another lookup failure is the error).
fn is_interface(vm: &mut Vm, iface: ClassId, path: &str) -> Result<bool, VmError> {
    Ok(vm.find_class_if_present(path)? == Some(iface))
}

/// The class path of `class`.
fn class_path(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.layout(class).path().to_string()
}

/// Object property `name` of `o`, which its class must declare (a `VmError::Script` otherwise). It loads nothing
/// (review r1 of #524, item 2): a load is a store write, so an `o` that is an unloaded definition handle is
/// `NotYet { NotRead }` naming the read.
fn field(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    const UNIT: &str = "the pawn QueryInterface rule";
    if vm.defs.is_unloaded(&vm.space, o) {
        return Err(NotYet::not_read(
            format!("{UNIT}: reading {name} of {}, an unloaded definition handle", object_path(vm, o)),
            RECORD,
        )
        .into());
    }
    let class = vm.space.heap.get(o)?.class;
    let Some(p) = vm.space.prop(class, name) else {
        return Err(crate::hit::access::error(format!(
            "{UNIT}: {} has no property {name}",
            crate::hit::access::class_name(vm, o)
        )));
    };
    Ok(object_of(vm.space.read(o, ow_object::ids::PropAddr::scalar(p))?))
}

/// Steps 2 and 3: `step`'s object `o` answers `iface` by its own rule, through the one dispatch
/// ([`Vm::query_interface`]); its `NotYet` is prefixed with the step (module notes).
fn ask(vm: &mut Vm, o: ObjRef, iface: ClassId, step: &str) -> Result<Option<ObjRef>, VmError> {
    match vm.query_interface(o, iface) {
        Err(VmError::NotYet(n)) => {
            let class = class_path(vm, vm.space.heap.get(o)?.class);
            let text = format!(
                "a pawn's QueryInterface({}) goes to {step}, a {class}: {}",
                class_path(vm, iface),
                n.what
            );
            Err(NotYet::new(text, n.missing, n.record).into())
        }
        other => other,
    }
}

/// `Pawn`'s answer (module notes, steps 1 to 4).
pub fn pawn_answer(vm: &mut Vm, pawn: ObjRef, iface: ClassId) -> Result<Option<ObjRef>, VmError> {
    if let Some(me) = vm.plain_interface(pawn, iface)? {
        return Ok(Some(me));
    }
    if let Some(controller) = field(vm, pawn, "Controller")? {
        if let Some(answer) = ask(vm, controller, iface, "its Controller")? {
            return Ok(Some(answer));
        }
    } else if let Some(vehicle) = field(vm, pawn, "DrivenVehicle")?
        && let Some(controller) = field(vm, vehicle, "Controller")?
        && let Some(answer) = ask(vm, controller, iface, "its driven vehicle's Controller")?
    {
        return Ok(Some(answer));
    }
    if let Some(mesh) = field(vm, pawn, "Mesh")?
        && let Some(answer) = ask(vm, mesh, iface, "its Mesh")?
    {
        return Ok(Some(answer));
    }
    Ok(None)
}

/// `WillowAIPawn`'s answer: `Pawn`'s, then the player master step for exactly `IPlayerMaster` (`NotYet`: the field is
/// not identified).
pub fn willow_ai_pawn_answer(vm: &mut Vm, pawn: ObjRef, iface: ClassId) -> Result<Option<ObjRef>, VmError> {
    if let Some(answer) = pawn_answer(vm, pawn, iface)? {
        return Ok(Some(answer));
    }
    if is_interface(vm, iface, PLAYER_MASTER_INTERFACE)? {
        return Err(NotYet::not_read(
            "WillowAIPawn's QueryInterface(IPlayerMaster) after Pawn's rule declines: it asks the pawn's player \
             master, a field the layout does not place exactly (PlayerMaster, identified by meaning only)",
            RECORD,
        )
        .into());
    }
    Ok(None)
}

/// How an exact component class declares `Owner` (judged at registration, [`owner_declaration`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerDeclaration {
    /// An object reference to `Engine.Actor` or a subclass.
    Actor(ow_object::ids::PropId),
    /// Not declared.
    Missing,
    /// Declared with another type.
    Mistyped,
}

/// `class`'s `Owner` declaration. The declared target's ancestry is read from the class metadata only (review r2 of
/// #524): the packages' class records and their super chain, with no layout defined and nothing else loaded, so a
/// target the class table holds only as a reserved name, or whose layout the port cannot build, is still judged. A
/// target no package holds (a class defined in the port alone) is judged on the class table.
pub fn owner_declaration(vm: &mut Vm, class: ClassId) -> Result<OwnerDeclaration, VmError> {
    let Some(prop) = vm.space.prop(class, "Owner") else {
        return Ok(OwnerDeclaration::Missing);
    };
    let target = match vm.space.classes.decl(prop).map(|d| d.kind.clone()) {
        Some(ow_object::layout::PropKind::Object { class: target }) => target,
        _ => return Ok(OwnerDeclaration::Mistyped),
    };
    let path = class_path(vm, target);
    let derives = {
        let (code, store) = vm.defs.code_mut();
        match (code.find_class(store, &path), code.find_class(store, "Engine.Actor")) {
            (Ok(t), Ok(a)) => Some(code.super_chain(t).any(|k| k == a)),
            _ => None,
        }
    };
    let derives = match derives {
        Some(d) => d,
        None => vm.space.classes.by_path("Engine.Actor").is_some_and(|a| vm.space.classes.is_child_of(target, a)),
    };
    Ok(if derives {
        OwnerDeclaration::Actor(prop)
    } else {
        OwnerDeclaration::Mistyped
    })
}

/// An exact component class's declarations, judged at registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentDeclarations {
    /// How the class declares `Owner`.
    pub owner: OwnerDeclaration,
    /// For an AI component ([`ATTACH_CHECKED_CLASSES`]) only: `bAttached`, when declared as a bool (`Some(None)`:
    /// undeclared or another kind). `None` for the other classes, whose `bAttached` is never read.
    pub attached: Option<Option<ow_object::ids::PropId>>,
}

/// `class`'s `bAttached`, when it is declared as a bool (from the class table; nothing loaded).
fn attached_declaration(vm: &Vm, class: ClassId) -> Option<ow_object::ids::PropId> {
    let prop = vm.space.prop(class, "bAttached")?;
    matches!(vm.space.classes.decl(prop).map(|d| &d.kind), Some(ow_object::layout::PropKind::Bool)).then_some(prop)
}

/// The unit the component rule's errors and refusals name (the note's section 6.4).
pub const COMPONENT_UNIT: &str = "ActorComponent.QueryInterface";

/// A component's answer (the note's section 6.4, the hub's rulings on review r1 of #512, at 17b9c330):
/// 1. the receiver's class exactly one of [`COMPONENT_RULE_CLASSES`]; any other class reaching this rule by
///    inheritance is the open `NotYet { NotRead }`, "what a <class> answers to QueryInterface is not read (asked of
///    <class> for <interface>)";
/// 2. the plain answer: an object is the answer, a `NotYet` or error passes through, only a successful none goes on;
/// 3. the `Owner` field: undeclared, or declared as anything but an object reference to `Engine.Actor` or a subclass,
///    is a `VmError::Script` with the note's text; the receiver or the `Owner` an unloaded definition handle is
///    `NotYet { NotRead }` (nothing is loaded: a load is a store write); `Owner` None answers none, except on an
///    unattached AI component ([`ATTACH_CHECKED_CLASSES`], the module notes);
/// 4. the owner's answer through [`Vm::query_interface`], the VM's cycle guard and depth limit unchanged, passed
///    through as it is.
///
/// Writes nothing and loads nothing.
pub fn component_answer(
    vm: &mut Vm,
    component: ObjRef,
    iface: ClassId,
    owners: &BTreeMap<ClassId, ComponentDeclarations>,
) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(component)?.class;
    let Some(ComponentDeclarations { owner: declaration, attached }) = owners.get(&class).copied() else {
        let name = class_path(vm, class);
        return Err(NotYet::not_read(
            format!(
                "what a {name} answers to QueryInterface is not read (asked of {name} for {})",
                class_path(vm, iface)
            ),
            RECORD,
        )
        .into());
    };
    // The plain answer first, unchanged in every case: an object, a `NotYet` or an error is the answer (the hub's
    // ruling on review r2 of #512, review r1 of #524 item 1). Only a successful None reaches the checks below.
    if let Some(me) = vm.plain_interface(component, iface)? {
        return Ok(Some(me));
    }
    if vm.defs.is_unloaded(&vm.space, component) {
        return Err(NotYet::not_read(
            format!("{COMPONENT_UNIT}: the component {} is an unloaded definition handle", object_path(vm, component)),
            RECORD,
        )
        .into());
    }
    let script = |message: String| VmError::Script { function: COMPONENT_UNIT.to_string(), message };
    let prop = match declaration {
        OwnerDeclaration::Actor(prop) => prop,
        OwnerDeclaration::Missing => {
            return Err(script(format!("{COMPONENT_UNIT}: {} declares no Owner", class_path(vm, class))));
        }
        OwnerDeclaration::Mistyped => {
            return Err(script(format!(
                "{COMPONENT_UNIT}: {}.Owner is not an object reference to Engine.Actor",
                class_path(vm, class)
            )));
        }
    };
    let owner = match vm.space.read(component, ow_object::ids::PropAddr::scalar(prop))? {
        Value::Object(o) => *o,
        _ => {
            return Err(script(format!(
                "{COMPONENT_UNIT}: {}.Owner is not an object reference to Engine.Actor",
                class_path(vm, class)
            )));
        }
    };
    let Some(owner) = owner else {
        return match attached {
            Some(attached) => unowned(vm, component, class, attached, iface),
            None => Ok(None),
        };
    };
    if vm.defs.is_unloaded(&vm.space, owner) {
        return Err(NotYet::not_read(
            format!(
                "{COMPONENT_UNIT}: the Owner {} of {} is an unloaded definition handle",
                object_path(vm, owner),
                object_path(vm, component)
            ),
            RECORD,
        )
        .into());
    }
    vm.query_interface(owner, iface)
}

/// `Owner` None on an AI component (module notes): none when attached; not attached is the hub's
/// `NotYet { NotRead }`. `attached` is its `bAttached` as judged at registration; undeclared or not a bool is a
/// `VmError::Script`. Reads only.
fn unowned(
    vm: &Vm,
    component: ObjRef,
    class: ClassId,
    attached: Option<ow_object::ids::PropId>,
    iface: ClassId,
) -> Result<Option<ObjRef>, VmError> {
    let path = class_path(vm, class);
    let script = |message: String| VmError::Script { function: COMPONENT_UNIT.to_string(), message };
    let Some(p) = attached else {
        return Err(script(format!("{COMPONENT_UNIT}: {path}.bAttached is not declared as a bool")));
    };
    match vm.space.read(component, ow_object::ids::PropAddr::scalar(p))? {
        Value::Bool(true) => Ok(None),
        Value::Bool(false) => Err(NotYet::not_read(
            format!(
                "the component {} has no Owner and is not attached: the attach step that sets Owner is not ported \
                 (asked for {})",
                object_path(vm, component),
                class_path(vm, iface)
            ),
            AI_COMPONENT_RECORD,
        )
        .into()),
        other => Err(script(format!("{COMPONENT_UNIT}: {path}.bAttached holds {}, not a bool", other.kind_name()))),
    }
}

/// `o`'s full path name for a refusal's text, best effort (review r1 of #524, item 4): when the path cannot be made
/// (an unloaded handle has no outer chain yet, or the chain is past `PathName`'s limit), `o`'s own name, and "an object"
/// for a stale handle. A diagnostic name never turns the refusal into another error.
fn object_path(vm: &Vm, o: ObjRef) -> String {
    match ow_vm::path_name::path_name(vm, o) {
        Ok(path) => path,
        Err(_) => vm
            .space
            .heap
            .get(o)
            .map(|object| vm.space.names.text(object.name).into_owned())
            .unwrap_or_else(|_| "an object".to_string()),
    }
}

/// `WillowVehicle`'s answer: its own rule is recorded for completeness only and not implemented.
fn vehicle_answer(vm: &mut Vm, vehicle: ObjRef, iface: ClassId) -> Result<Option<ObjRef>, VmError> {
    Err(vehicle_not_yet(vm, vehicle, iface))
}

fn vehicle_not_yet(vm: &Vm, vehicle: ObjRef, iface: ClassId) -> VmError {
    NotYet::not_read(
        format!(
            "WillowVehicle's own QueryInterface({}) rule (its controller unless a human player's, IAnimBehavior from \
             its Mesh), recorded for completeness and not implemented, asked of {}",
            class_path(vm, iface),
            crate::hit::access::class_name(vm, vehicle)
        ),
        RECORD,
    )
    .into()
}

pub mod read_only;

#[cfg(test)]
mod tests;
