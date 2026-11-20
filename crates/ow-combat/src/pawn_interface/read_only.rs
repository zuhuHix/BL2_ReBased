//! Owner: lane IMPL-GEARBOXMIND-QUERYINTERFACE. `Controller`'s `QueryInterface` rule in Off, and the read-only forms
//! of the pawn rules it can reach. AI-assisted (Claude Code). Decision 2026-11-19-NNN.
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_GEARBOXMIND_QUERYINTERFACE.md` alone (#589, the analyst
//! lane ANALYST-GEARBOXMIND-QUERYINTERFACE), by a lane that did not read the natives. Every rule is **UNVERIFIED** in
//! the running game.
//!
//! **`Controller`'s rule** ([`controller_rule`], the note's section 2), registered for `Engine.Controller` (so
//! `AIController`, `GearboxAIController`, `GearboxMind` and `WillowMind` use it) with `Pawn` as "its pawn", and again
//! for `WillowGame.WillowMind` with `MyWillowPawn` (section 3). Asked for `I`, a controller `C`:
//! 1. answers none when it is already answering a question by this rule further up the chain, **whatever interface**
//!    (a guard keyed by the controller alone, as the game's; it sits on top of the dispatch's (object, interface)
//!    guard);
//! 2. for `I` named `IWorldBody` or `ISoundBehavior`, with its pawn set: whatever the pawn answers, none included;
//! 3. itself, when its class chain lists `I` ([`Vm::plain_interface_read_only`]: entries resolved at set-up only);
//! 4. with no pawn, none; else the pawn's answer when it gives one;
//! 5. else its `PlayerReplicationInfo`'s answer when that is set, none included; else none.
//!
//! **Read-only along the chain.** Each rule here takes `&Vm`: it writes, loads and draws nothing. Fields are read only
//! when a step reaches them; an object to ask, or whose field is read, that is an unloaded stand-in is
//! `NotYet { NotRead }`; an object whose class answers by a rule with no read-only form is `NotYet { NotPorted }`
//! ([`Vm::query_interface_read_only`]). A refusal from further down keeps its kind and record, with the step that
//! asked named ahead of it.
//!
//! **Read-only forms** of `pawn_interface`'s rules, so a controller can ask its pawn: `Pawn`'s ([`pawn_read_only`]),
//! `WillowAIPawn`'s ([`willow_ai_pawn_read_only`]: its `IPlayerMaster` step is compared by class path, with no lookup),
//! `WillowVehicle`'s and the open answers (the same `NotYet`s). `Engine.PlayerController`'s answer is `NotYet
//! { NotRead }` (that line was not read, so the `Controller` rule must not answer for it by inheritance);
//! `WillowPlayerController` keeps its own registered answer.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::{InterfaceAsking, ReadOnlyInterfaceRuleFn, Vm};
use ow_vm::error::VmError;

use super::{AI_INTERFACE, PLAYER_MASTER_INTERFACE};

/// The analyst note this module implements.
pub const RECORD: &str = "docs/verification/NATIVE_GEARBOXMIND_QUERYINTERFACE.md";

/// The classes the rule is registered for, with the field each reads as "its pawn".
pub const CONTROLLER: &str = "Engine.Controller";
pub const WILLOW_MIND: &str = "WillowGame.WillowMind";
pub const CONTROLLER_RULES: [(&str, &str); 2] = [(CONTROLLER, "Pawn"), (WILLOW_MIND, "MyWillowPawn")];
/// The class line the note did not read, answered `NotYet` so `Controller`'s rule never answers for it by inheritance.
pub const PLAYER_CONTROLLER: &str = "Engine.PlayerController";
/// The two interface names step 2 hands to the pawn first (compared by name, case-insensitively).
pub const PAWN_FIRST: [&str; 2] = ["IWorldBody", "ISoundBehavior"];

/// The guard's mark (step 1): keyed by the controller alone.
const GUARD: &str = "Controller.QueryInterface";

/// `this`'s read-only answer to `QueryInterface(iface)`, the entry a caller in Rust uses: `None` is a caller error
/// (`VmError::Script`; the script native returns None before any rule runs).
pub fn query_read_only(vm: &Vm, this: ObjRef, iface: Option<ClassId>) -> Result<Option<ObjRef>, VmError> {
    let Some(iface) = iface else {
        return Err(VmError::Script {
            function: "Object.QueryInterface".into(),
            message: "QueryInterface asked with no interface class (None)".into(),
        });
    };
    vm.query_interface_read_only(&mut InterfaceAsking::default(), this, iface)
}

/// `Controller`'s rule with `pawn_field` as "its pawn" (module notes).
pub fn controller_rule(pawn_field: &'static str) -> ReadOnlyInterfaceRuleFn {
    Rc::new(move |vm: &Vm, asking: &mut InterfaceAsking, c: ObjRef, iface: ClassId| {
        if asking.marks.contains(&(GUARD, c)) {
            return Ok(None);
        }
        asking.marks.push((GUARD, c));
        let answer = controller_steps(vm, asking, c, iface, pawn_field);
        asking.marks.pop();
        answer
    })
}

fn controller_steps(
    vm: &Vm,
    asking: &mut InterfaceAsking,
    c: ObjRef,
    iface: ClassId,
    pawn_field: &str,
) -> Result<Option<ObjRef>, VmError> {
    let who = "a controller";
    // Step 2.
    let name = interface_name(vm, iface)?;
    if PAWN_FIRST.iter().any(|n| n.eq_ignore_ascii_case(&name))
        && let Some(p) = field(vm, c, pawn_field, who)?
    {
        return ask(vm, asking, p, iface, who, "its pawn");
    }
    // Step 3.
    if let Some(me) = vm.plain_interface_read_only(c, iface)? {
        return Ok(Some(me));
    }
    // Step 4.
    let Some(p) = field(vm, c, pawn_field, who)? else {
        return Ok(None);
    };
    if let Some(answer) = ask(vm, asking, p, iface, who, "its pawn")? {
        return Ok(Some(answer));
    }
    // Step 5.
    match field(vm, c, "PlayerReplicationInfo", who)? {
        Some(pri) => ask(vm, asking, pri, iface, who, "its PlayerReplicationInfo"),
        None => Ok(None),
    }
}

/// The leaf name of interface class `iface` (a plain read; a class with no layout is `NotRead`).
fn interface_name(vm: &Vm, iface: ClassId) -> Result<String, VmError> {
    let Some(layout) = vm.space.classes.get_layout(iface) else {
        return Err(NotYet::not_read(format!("the interface class {iface:?} is not resolved at set-up"), RECORD).into());
    };
    let path = layout.path().to_string();
    Ok(path.rsplit_once('.').map_or(path.clone(), |(_, leaf)| leaf.to_string()))
}

/// The class path of `class` (read only).
fn class_path(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().to_string())
}

/// Object field `name` of `o`, read-only: an unloaded `o` is `NotYet { NotRead }`; a field its class does not
/// declare, or that holds something else than an object, is `VmError::Script` (a malformed class).
pub fn field(vm: &Vm, o: ObjRef, name: &str, who: &str) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    if vm.defs.is_unloaded(&vm.space, o) {
        return Err(NotYet::not_read(
            format!("{who}'s QueryInterface reads {name} of an unloaded {}: reading it is a load", class_path(vm, class)),
            RECORD,
        )
        .into());
    }
    let script = |message: String| VmError::Script { function: "Object.QueryInterface".into(), message };
    let Some(p) = vm.space.prop(class, name) else {
        return Err(script(format!("{who}'s QueryInterface: {} does not declare {name}", class_path(vm, class))));
    };
    match vm.space.read(o, PropAddr::scalar(p))? {
        Value::Object(x) | Value::Interface(x) => Ok(*x),
        other => Err(script(format!(
            "{who}'s QueryInterface: {}.{name} holds {}, not an object",
            class_path(vm, class),
            other.kind_name()
        ))),
    }
}

/// `o` asked `iface` through the read-only dispatch; a `NotYet` gets the step named ahead of it, its kind and record
/// kept.
fn ask(
    vm: &Vm,
    asking: &mut InterfaceAsking,
    o: ObjRef,
    iface: ClassId,
    who: &str,
    step: &str,
) -> Result<Option<ObjRef>, VmError> {
    match vm.query_interface_read_only(asking, o, iface) {
        Err(VmError::NotYet(n)) => {
            let class = vm.space.heap.get(o).map(|x| class_path(vm, x.class)).unwrap_or_default();
            let text = format!("{who}'s QueryInterface({}) goes to {step}, a {class}: {}", class_path(vm, iface), n.what);
            Err(NotYet::new(text, n.missing, n.record).into())
        }
        other => other,
    }
}

/// Names a `NotYet` from a field read with the step that read it.
fn stepped<T>(r: Result<T, VmError>, vm: &Vm, iface: ClassId, step: &str) -> Result<T, VmError> {
    match r {
        Err(VmError::NotYet(n)) => {
            let text = format!("a pawn's QueryInterface({}) reads {step}: {}", class_path(vm, iface), n.what);
            Err(NotYet::new(text, n.missing, n.record).into())
        }
        other => other,
    }
}

/// `Pawn`'s rule read-only (`pawn_interface` module notes, steps 1 to 4).
pub fn pawn_read_only(vm: &Vm, asking: &mut InterfaceAsking, pawn: ObjRef, iface: ClassId) -> Result<Option<ObjRef>, VmError> {
    let who = "a pawn";
    if let Some(me) = vm.plain_interface_read_only(pawn, iface)? {
        return Ok(Some(me));
    }
    if let Some(controller) = field(vm, pawn, "Controller", who)? {
        if let Some(answer) = ask(vm, asking, controller, iface, who, "its Controller")? {
            return Ok(Some(answer));
        }
    } else if let Some(vehicle) = field(vm, pawn, "DrivenVehicle", who)?
        && let Some(controller) = stepped(field(vm, vehicle, "Controller", who), vm, iface, "its driven vehicle's Controller")?
        && let Some(answer) = ask(vm, asking, controller, iface, who, "its driven vehicle's Controller")?
    {
        return Ok(Some(answer));
    }
    if let Some(mesh) = field(vm, pawn, "Mesh", who)?
        && let Some(answer) = ask(vm, asking, mesh, iface, who, "its Mesh")?
    {
        return Ok(Some(answer));
    }
    Ok(None)
}

/// `WillowAIPawn`'s rule read-only: `Pawn`'s, then for exactly `IPlayerMaster` (compared by class path) the same
/// `NotYet` as its ordinary form.
pub fn willow_ai_pawn_read_only(
    vm: &Vm,
    asking: &mut InterfaceAsking,
    pawn: ObjRef,
    iface: ClassId,
) -> Result<Option<ObjRef>, VmError> {
    if let Some(answer) = pawn_read_only(vm, asking, pawn, iface)? {
        return Ok(Some(answer));
    }
    if class_path(vm, iface).eq_ignore_ascii_case(PLAYER_MASTER_INTERFACE) {
        return Err(NotYet::not_read(
            "WillowAIPawn's QueryInterface(IPlayerMaster) after Pawn's rule declines: it asks the pawn's player \
             master, a field the layout does not place exactly (PlayerMaster, identified by meaning only)",
            super::RECORD,
        )
        .into());
    }
    Ok(None)
}

/// A read-only form that is the given `NotYet { NotRead }` for every question (`words` names the open answer); with
/// `ai_only`, only for `IAIInterface` (compared by class path), the read-only plain answer otherwise.
pub fn open_read_only(words: &'static str, ai_only: bool, record: &'static str) -> ReadOnlyInterfaceRuleFn {
    Rc::new(move |vm: &Vm, _asking: &mut InterfaceAsking, this: ObjRef, iface: ClassId| {
        if ai_only && !class_path(vm, iface).eq_ignore_ascii_case(AI_INTERFACE) {
            return vm.plain_interface_read_only(this, iface);
        }
        Err(NotYet::not_read(
            format!(
                "{words} is not read (asked of {} for {})",
                class_path(vm, vm.space.heap.get(this)?.class),
                class_path(vm, iface)
            ),
            record,
        )
        .into())
    })
}

/// `Engine.PlayerController`'s answer: not read (module notes).
pub fn player_controller_rule() -> ReadOnlyInterfaceRuleFn {
    open_read_only(
        "PlayerController's own QueryInterface answer (PlayerController and GamePlayerController were not read; \
         WillowPlayerController has its own)",
        false,
        RECORD,
    )
}

/// The classes a harness set-up resolves for the rule (lane IMPL-GEARBOXMIND-QUERYINTERFACE): the interfaces the
/// rules compare by path, `IRuleEngineOwner`, and the controller classes the rule is registered for.
/// `IRuleEngineOwner` is the flag evaluator's need (#586's recommendation), not this rule's: it moves to the
/// evaluator's own set-up list when #579 adds one (review r1 of #602, item 6). `IWorldBody` and `ISoundBehavior` are compared by the asked class's name, so
/// they need no lookup (their packages are not identified).
pub const SETUP_CLASSES: [&str; 7] = [
    AI_INTERFACE,
    PLAYER_MASTER_INTERFACE,
    "GearboxFramework.IRuleEngineOwner",
    CONTROLLER,
    PLAYER_CONTROLLER,
    "GearboxFramework.GearboxMind",
    WILLOW_MIND,
];

/// The object fields the set-up follows from its roots to find the objects the rules can ask.
pub const SETUP_FIELDS: [&str; 7] =
    ["Controller", "Pawn", "MyWillowPawn", "MyWillowMind", "DrivenVehicle", "Mesh", "PlayerReplicationInfo"];

/// A harness set-up step, before any question (a write the caller logs, UNVERIFIED stand-in for the game having
/// these loaded): resolves [`SETUP_CLASSES`], then the interface-table entries of the class chain of every object
/// reachable from `roots` through [`SETUP_FIELDS`] (loaded objects only: an unloaded one is noted and not loaded, so
/// the rule meets it as `NotRead`). Returns one `harness input:` line per class, in order.
pub fn prepare(vm: &mut Vm, roots: &[ObjRef]) -> Result<Vec<String>, VmError> {
    let mut lines = Vec::new();
    for path in SETUP_CLASSES {
        let before = vm.space.classes.by_path(path).is_some();
        let line = match vm.find_class_if_present(path)? {
            Some(_) if before => "in the class table already, nothing loaded",
            Some(_) => "loaded at set-up (a write; UNVERIFIED stand-in for the game having it loaded)",
            None => "not in the packages, nothing loaded",
        };
        lines.push(format!("harness input: interface set-up class {path}: {line}"));
    }
    let mut seen: Vec<ObjRef> = Vec::new();
    let mut queue: Vec<ObjRef> = roots.to_vec();
    let mut classes: Vec<ClassId> = Vec::new();
    while let Some(o) = queue.pop() {
        if seen.contains(&o) {
            continue;
        }
        seen.push(o);
        if vm.defs.is_unloaded(&vm.space, o) {
            let class = vm.space.heap.get(o)?.class;
            lines.push(format!(
                "harness input: interface set-up: an unloaded {} is left unloaded (the rule refuses it NotRead)",
                class_path(vm, class)
            ));
            continue;
        }
        let class = vm.space.heap.get(o)?.class;
        let mut cursor = Some(class);
        while let Some(c) = cursor {
            if !classes.contains(&c) {
                classes.push(c);
            }
            cursor = vm.space.classes.layout(c).super_class();
        }
        for name in SETUP_FIELDS {
            if let Some(p) = vm.space.prop(class, name)
                && let Value::Object(Some(x)) | Value::Interface(Some(x)) = vm.space.read(o, PropAddr::scalar(p))?
            {
                queue.push(*x);
            }
        }
    }
    for c in classes {
        let resolved = vm.resolve_interface_tables(c);
        if resolved > 0 {
            lines.push(format!(
                "harness input: interface tables of {}: {resolved} entries resolved at set-up (a load; UNVERIFIED stand-in for the game having them resolved)",
                class_path(vm, c)
            ));
        }
    }
    // The store's own lines for these loads follow the harness lines (review r1 of #602, item 6).
    lines.extend(vm.defs.take_log());
    Ok(lines)
}

#[cfg(test)]
#[path = "read_only_tests.rs"]
mod tests;
