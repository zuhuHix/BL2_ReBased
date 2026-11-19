//! Owner: lane IMPL-INIT-PLAYER-SKILL-TREE. AI-assisted (Claude Code). `WillowPlayerController.InitPlayerSkillTree()`
//! in `ParityMode::Off`, implemented from NATIVE_INIT_PLAYER_SKILL_TREE.md alone (decision 2026-11-19-88; the analyst
//! was lane ANALYST-INIT-PLAYER-SKILL-TREE, decision 2026-11-19-78, PR #466). Every rule is the note's reading of the
//! executable, **UNVERIFIED** in the running game.
//!
//! **The Off answer** (the note's section 5): one answer per case. The note's Off assumptions A1 to A5 are each a
//! named function here, so a revision of the note changes one place:
//! - A1 and A2 ([`request_row`]): the definition is never already loaded and the immediate-load switch is off, so a
//!   request is never delivered inside the call;
//! - A3 ([`record_request`]): this lane delivers nothing; a request stays pending in the [`PendingSkillTreeLoads`]
//!   service for a later lane, which delivers it to its `PlayerContext` through `RunStreamingDataEvent`;
//! - A4 and A5 ([`reset_new_tree`]): the reset of a tree this call has just made changes nothing, its refund is 0,
//!   and no client event is sent.
//!
//! **Preflight**, every read before the allocation, in the note's order:
//! - P1: the controller's class declares `PlayerSkillTree` as an object property (else a data error);
//! - P2 ([`existing_tree`]): a set `PlayerSkillTree` is the named `NotYet` (`NotRead`) for the reset of an existing
//!   tree; None goes on; another kind is a data error;
//! - P3: the class `WillowGame.PlayerSkillTree`, the store's own error propagated unchanged;
//! - P4 ([`pawn_data_manager`]): the world's globals (its own `NotYet` when it has none) and their
//!   `ThePlayerPawnDataManager`, declared as an object property; None means no request;
//! - P5a ([`player_class`]): the controller's `PlayerClass` (an object property), read whether or not P4 found a
//!   manager; None means no player class;
//! - P5b ([`skill_tree_path`]): its `SkillTreePath` (a string property), read only when P4 gave a manager and P5a a
//!   player class;
//! - P6 ([`request_row`]): a request exactly when P4 and P5 both gave one, its row by the path's shape.
//!
//! **Writes**, for a new tree: W1 the tree, outer the controller, a generated name (the native passes none); W2 the
//! controller's `PlayerSkillTree`; W3 one pending request for a dotted path, nothing for a dotless one; W4 the no-op
//! reset. With A1 to A5 nothing after W1 can raise a `NotYet`; should one ever surface after a write, it stops the
//! call with "stopped after partial writes: ", its kind and record kept, nothing rolled back ([`after_write`]). Other
//! errors propagate unchanged.
//!
//! **Reference** (architecture R3): the C++ VM binds no stand-in, so under Reference the native stays unbound and the
//! stub policy answers it, as before.

use std::cell::RefCell;
use std::rc::Rc;

use ow_attr::api::EvalCx;
use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr, PropId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;

/// The record this native follows.
pub const RECORD: &str = "docs/verification/NATIVE_INIT_PLAYER_SKILL_TREE.md";
/// The script function bound (its declaration's own native key is used).
pub const PATH: &str = "WillowGame.WillowPlayerController.InitPlayerSkillTree";
/// The class of the tree it makes.
pub const TREE_CLASS: &str = "WillowGame.PlayerSkillTree";
/// The completion's event name (the note's section 1, step 4).
pub const EVENT: &str = "SkillTreeDefinitionLoaded";
/// The mark of a failure after state was written (the text of `ow_behavior::kernel::run::PARTIAL_WRITES`).
pub const PARTIAL_WRITES: &str = "stopped after partial writes";

const WHAT: &str = "WillowPlayerController.InitPlayerSkillTree";

/// One request to `WillowPlayerPawnDataManager.LoadSkillTreeDefinitionAsync`, with all seven arguments (the note's
/// section 1, step 4).
#[derive(Debug, Clone, PartialEq)]
pub struct SkillTreeRequest {
    /// `SkillTreeDefinitionPath`: the player class's `SkillTreePath`.
    pub path: String,
    /// `PlayerContext`: the controller.
    pub player_context: ObjRef,
    /// `Payload`: None.
    pub payload: Option<ObjRef>,
    /// `Instigator`: the controller.
    pub instigator: ObjRef,
    /// `IntParam`: 0.
    pub int_param: i32,
    /// `FloatParam`: 0.0.
    pub float_param: f32,
    /// `CallbackCustomEventName`: [`EVENT`].
    pub event: String,
    /// The manager asked.
    pub manager: ObjRef,
}

/// The requests made and not delivered (a VM service; A3). A later lane delivers them.
#[derive(Debug, Default)]
pub struct PendingSkillTreeLoads {
    pub requests: Vec<SkillTreeRequest>,
}

/// The VM's [`PendingSkillTreeLoads`] service, added empty on first use.
pub fn pending(vm: &mut Vm) -> Result<Rc<RefCell<PendingSkillTreeLoads>>, VmError> {
    match vm.service::<PendingSkillTreeLoads>() {
        Some(p) => Ok(p),
        None => vm.add_service(PendingSkillTreeLoads::default()),
    }
}

fn bad(message: impl Into<String>) -> VmError {
    VmError::Script {
        function: WHAT.to_string(),
        message: message.into(),
    }
}

fn class_path(vm: &Vm, o: ObjRef) -> String {
    ow_attr::eval::class_path(&vm.space, o)
}

/// The property `name` of `o`'s class, declared with a kind `ok` accepts (else a data error naming `want`).
fn declared(vm: &Vm, o: ObjRef, name: &str, want: &str, ok: fn(&PropKind) -> bool) -> Result<PropId, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| bad(format!("{} declares no {name}", class_path(vm, o))))?;
    match vm.space.classes.decl(p) {
        Some(d) if ok(&d.kind) => Ok(p),
        _ => Err(bad(format!("{} declares {name} as something other than {want}", class_path(vm, o)))),
    }
}

fn is_object(k: &PropKind) -> bool {
    matches!(k, PropKind::Object { .. })
}

fn is_str(k: &PropKind) -> bool {
    matches!(k, PropKind::Str)
}

/// An object value (None or a reference), else a data error.
fn object_value(v: &Value, name: &str) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) => Ok(*o),
        Value::None => Ok(None),
        other => Err(bad(format!("{name} holds a {}, not an object", other.kind_name()))),
    }
}

/// Loads `o` when it is a store handle not yet loaded; a store error converts through `VmError::from`.
fn loaded(vm: &mut Vm, o: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        let r = vm.defs.ensure_loaded(&mut vm.space, o);
        let log = vm.defs.take_log();
        vm.log.extend(log);
        r.map_err(VmError::from)?;
    }
    Ok(())
}

/// P2: an existing tree is the named `NotYet`; its reset is the full NATIVE_SKILLS.md section 3.1 reset, not ported.
/// The only installed caller clears `PlayerSkillTree` first, so the slice does not meet it.
fn existing_tree(tree: ObjRef) -> VmError {
    NotYet::not_read(
        format!(
            "InitPlayerSkillTree: the reset of an existing skill tree (NATIVE_SKILLS.md section 3.1 is not ported; tree {tree:?})"
        ),
        RECORD,
    )
    .into()
}

/// P4: the globals' `ThePlayerPawnDataManager`, None meaning no request. A world without globals answers its own
/// `NotYet`, unchanged.
fn pawn_data_manager(vm: &mut Vm) -> Result<Option<ObjRef>, VmError> {
    let globals = {
        let cx = EvalCx::new(vm);
        cx.world.globals()?
    };
    loaded(vm, globals)?;
    let p = declared(vm, globals, "ThePlayerPawnDataManager", "an object property", is_object)?;
    object_value(vm.space.read(globals, PropAddr::scalar(p))?, "ThePlayerPawnDataManager")
}

/// P5a: the controller's `PlayerClass`, read whether or not P4 found a manager; None means no player class.
fn player_class(vm: &mut Vm, controller: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let p = declared(vm, controller, "PlayerClass", "an object property", is_object)?;
    object_value(vm.space.read(controller, PropAddr::scalar(p))?, "PlayerClass")
}

/// P5b: the player class's `SkillTreePath`, read only when P4 gave a manager and P5a a player class.
fn skill_tree_path(vm: &mut Vm, class: ObjRef) -> Result<String, VmError> {
    loaded(vm, class)?;
    let p = declared(vm, class, "SkillTreePath", "a string property", is_str)?;
    match vm.space.read(class, PropAddr::scalar(p))? {
        Value::Str(s) => Ok(s.to_string()),
        other => Err(bad(format!("SkillTreePath holds a {}, not a string", other.kind_name()))),
    }
}

/// P6, with A1 (never already loaded) and A2 (the immediate-load switch off): the note's "not loaded, switch off"
/// column of its request cases. A dotted path is one pending request; a dotless one (the empty string included) is nothing, as the game's
/// "not loaded" column drops it.
fn request_row(manager: Option<ObjRef>, path: Option<String>, controller: ObjRef) -> Option<SkillTreeRequest> {
    let (manager, path) = (manager?, path?);
    if !path.contains('.') {
        return None;
    }
    Some(SkillTreeRequest {
        path,
        player_context: controller,
        payload: None,
        instigator: controller,
        int_param: 0,
        float_param: 0.0,
        event: EVENT.to_string(),
        manager,
    })
}

/// W3 with A3: the request is recorded as pending, and nothing is delivered.
fn record_request(vm: &mut Vm, request: SkillTreeRequest) -> Result<(), VmError> {
    pending(vm)?
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("ow_skills::tree::init::PendingSkillTreeLoads"))?
        .requests
        .push(request);
    Ok(())
}

/// W4 with A4 and A5: the reset of the tree this call has just made (no definition, no listener) changes nothing,
/// its refund is 0, and no client event is sent. Nothing is written.
fn reset_new_tree(_vm: &mut Vm, _controller: ObjRef, _tree: ObjRef) -> Result<(), VmError> {
    Ok(())
}

/// A failure after the first write (the hub's rule): a `NotYet` is terminal and gets the partial-writes prefix once,
/// its kind and record kept; any other error passes unchanged. Nothing is rolled back.
fn after_write(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) if !n.what.contains(PARTIAL_WRITES) => {
            NotYet::new(format!("{PARTIAL_WRITES}: {}", n.what), n.missing, n.record).into()
        }
        other => other,
    }
}

/// What the preflight decided.
struct Plan {
    tree_class: ow_object::ids::ClassId,
    tree_prop: PropId,
    request: Option<SkillTreeRequest>,
}

/// P1 to P6 (module documentation). Nothing is written.
fn preflight(vm: &mut Vm, controller: ObjRef) -> Result<Plan, VmError> {
    let tree_prop = declared(vm, controller, "PlayerSkillTree", "an object property", is_object)?;
    if let Some(tree) = object_value(vm.space.read(controller, PropAddr::scalar(tree_prop))?, "PlayerSkillTree")? {
        return Err(existing_tree(tree));
    }
    let tree_class = vm.find_class(TREE_CLASS)?;
    let manager = pawn_data_manager(vm)?;
    let class = player_class(vm, controller)?;
    let path = match (manager, class) {
        (Some(_), Some(class)) => Some(skill_tree_path(vm, class)?),
        _ => None,
    };
    let request = request_row(manager, path, controller);
    Ok(Plan { tree_class, tree_prop, request })
}

/// `InitPlayerSkillTree()` on `controller` (module documentation).
pub fn init_player_skill_tree(vm: &mut Vm, controller: ObjRef) -> Result<(), VmError> {
    let plan = preflight(vm, controller)?;
    // W1: the first write.
    let tree = ow_vm::interp::instantiate_from_defaults(vm, plan.tree_class)?;
    let rest = |vm: &mut Vm| -> Result<(), VmError> {
        vm.space.heap.get_mut(tree)?.outer = Some(controller);
        vm.space.write(controller, PropAddr::scalar(plan.tree_prop), Value::Object(Some(tree)))?;
        if let Some(request) = plan.request.clone() {
            record_request(vm, request)?;
        }
        reset_new_tree(vm, controller, tree)
    };
    rest(vm).map_err(after_write)
}

/// Binds the native in `ParityMode::Off` by its declaration's key; Reference binds nothing (module documentation).
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        if vm.parity != ParityMode::Off {
            return Ok(());
        }
        pending(vm)?;
        ow_combat::hit::bind(
            vm,
            PATH,
            Rc::new(|c: &mut NativeCall<'_>| {
                let controller = c.this.ok_or_else(|| bad("called without a controller"))?;
                init_player_skill_tree(c.vm, controller)?;
                Ok(Value::None)
            }),
        )
    }
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
