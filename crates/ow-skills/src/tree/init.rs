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
//! - P3 ([`tree_class`]): the class `WillowGame.PlayerSkillTree`, looked up once at setup ([`setup`], the
//!   [`TreeClassSetup`] service); P3 answers that result, its original error included, and never looks the class up
//!   again. The tree field's declared class must accept it, else the heap's typed `WrongClass` error, before W1;
//! - P4 ([`pawn_data_manager`]): the world's globals (its own `NotYet` when it has none) and their
//!   `ThePlayerPawnDataManager`, declared as an object property; None means no request;
//! - P5a ([`player_class`]): the controller's `PlayerClass` (an object property), read whether or not P4 finds a
//!   manager; None means no player class;
//! - P5b ([`skill_tree_path`]): its `SkillTreePath` (a string property), read only when P4 gave a manager and P5a a
//!   player class;
//! - the preflight reads only resident definitions (hub ruling, review r1 of #475: a lazy load is a write): globals
//!   or a player class that is still an unloaded store handle is a named `NotYet` ([`resident`]), and nothing is
//!   loaded;
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
use ow_object::heap::HeapError;
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId};
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

/// The plan reads only resident definitions (hub ruling, review r1 of #475: a lazy load is a write). A store handle
/// that is not loaded yet is a named `NotYet`, raised before any write, and nothing is loaded.
fn resident(vm: &Vm, o: ObjRef, what: &str) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        return Err(NotYet::not_ported(
            format!(
                "InitPlayerSkillTree: {what} ({}) is not loaded; loading it in the preflight would be a write before the plan's refusals (hub ruling: a lazy load is a write)",
                class_path(vm, o)
            ),
            RECORD,
        )
        .into());
    }
    Ok(())
}

/// P2: an existing tree is the named `NotYet`; its reset is the full NATIVE_SKILLS.md section 3.1 reset, not ported.
/// The only installed caller clears `PlayerSkillTree` first, so the slice does not meet it.
fn existing_tree() -> VmError {
    NotYet::not_read(EXISTING_TREE, RECORD).into()
}

/// P2's diagnostic, the note's exact text.
pub const EXISTING_TREE: &str =
    "InitPlayerSkillTree: the reset of an existing skill tree (NATIVE_SKILLS.md section 3.1 is not ported)";

/// P4: the globals' `ThePlayerPawnDataManager`, None meaning no request. A world without globals answers its own
/// `NotYet`, unchanged.
fn pawn_data_manager(vm: &mut Vm) -> Result<Option<ObjRef>, VmError> {
    let globals = {
        let cx = EvalCx::new(vm);
        cx.world.globals()?
    };
    resident(vm, globals, "the engine globals")?;
    let p = declared(vm, globals, "ThePlayerPawnDataManager", "an object property", is_object)?;
    object_value(vm.space.read(globals, PropAddr::scalar(p))?, "ThePlayerPawnDataManager")
}

/// P5a: the controller's `PlayerClass`, read whether or not P4 found a manager; None means no player class.
fn player_class(vm: &Vm, controller: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let p = declared(vm, controller, "PlayerClass", "an object property", is_object)?;
    object_value(vm.space.read(controller, PropAddr::scalar(p))?, "PlayerClass")
}

/// P5b: the player class's `SkillTreePath`, read only when P4 gave a manager and P5a a player class.
fn skill_tree_path(vm: &Vm, class: ObjRef) -> Result<String, VmError> {
    resident(vm, class, "the player class")?;
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

/// W3 with A3: the request is recorded as pending, and nothing is delivered. The queue's borrow was taken before W1
/// (review r1 of #475), so this cannot fail.
fn record_request(queue: &mut PendingSkillTreeLoads, request: SkillTreeRequest) {
    queue.requests.push(request);
}

/// W4 with A4 and A5: the reset of the tree this call has just made (no definition, no listener) changes nothing,
/// its refund is 0, and no client event is sent. Nothing is written.
fn reset_new_tree(_vm: &mut Vm, _controller: ObjRef, _tree: ObjRef) -> Result<(), VmError> {
    Ok(())
}

/// A failure after the first write (the hub's rule): `ow_world::pools::after_partial_writes`, which adds the
/// "stopped after partial writes: " prefix unless the message already starts with it, keeping kind and record; any
/// other error passes unchanged. Nothing is rolled back.
fn after_write(e: VmError) -> VmError {
    ow_world::pools::after_partial_writes(e)
}

/// What the preflight decided.
struct Plan {
    tree_class: ow_object::ids::ClassId,
    tree_prop: PropId,
    request: Option<SkillTreeRequest>,
}

/// The tree class as setup resolved it (a VM service): the whole lookup result, kept so that P3 answers the original
/// error and never looks the class up again (review r2 of #475).
#[derive(Debug, Clone)]
pub struct TreeClassSetup(pub Result<ClassId, VmError>);

/// Setup: looks the tree class up once and keeps the result in the VM ([`TreeClassSetup`]). [`Pack`] runs it; a caller
/// of [`init_player_skill_tree`] without the pack runs it first.
pub fn setup(vm: &mut Vm) -> Result<(), VmError> {
    let resolved = vm.find_class(TREE_CLASS);
    match vm.service::<TreeClassSetup>() {
        Some(s) => {
            s.try_borrow_mut()
                .map_err(|_| VmError::ServiceBorrowed("ow_skills::tree::init::TreeClassSetup"))?
                .0 = resolved;
        }
        None => {
            vm.add_service(TreeClassSetup(resolved))?;
        }
    }
    Ok(())
}

/// The setup result; a VM whose setup never ran is a data error.
fn setup_result(vm: &Vm) -> Result<Result<ClassId, VmError>, VmError> {
    let s = vm
        .service::<TreeClassSetup>()
        .ok_or_else(|| bad("the tree class setup did not run (ow_skills::tree::init::setup)"))?;
    let r = s
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("ow_skills::tree::init::TreeClassSetup"))?
        .0
        .clone();
    Ok(r)
}

/// P3: the tree class as setup resolved it; a lookup setup could not make answers its original error, unchanged. The
/// tree field's declared class must accept it (review r1 of #475), else the heap's own typed `WrongClass` error,
/// before W1.
fn tree_class(vm: &mut Vm, resolved: Result<ClassId, VmError>, tree_prop: PropId) -> Result<ClassId, VmError> {
    let class = resolved?;
    if let Some(PropKind::Object { class: wanted }) = vm.space.classes.decl(tree_prop).map(|d| d.kind.clone())
        && !vm.space.classes.is_child_of(class, wanted)
    {
        return Err(HeapError::WrongClass { expected: wanted, found: class }.into());
    }
    Ok(class)
}

/// P1 to P6 in the note's order (module documentation). Nothing is written and nothing is loaded.
fn preflight(vm: &mut Vm, controller: ObjRef, resolved: Result<ClassId, VmError>) -> Result<Plan, VmError> {
    let tree_prop = declared(vm, controller, "PlayerSkillTree", "an object property", is_object)?;
    if object_value(vm.space.read(controller, PropAddr::scalar(tree_prop))?, "PlayerSkillTree")?.is_some() {
        return Err(existing_tree());
    }
    let tree_class = tree_class(vm, resolved, tree_prop)?;
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
/// The setup result ([`setup`]) is read from the VM, as the bound native reads it.
pub fn init_player_skill_tree(vm: &mut Vm, controller: ObjRef) -> Result<(), VmError> {
    let resolved = setup_result(vm)?;
    let plan = preflight(vm, controller, resolved)?;
    // The queue's borrow, taken before W1 when there is a request (review r1 of #475).
    let queue = pending(vm)?;
    let mut held = match plan.request {
        Some(_) => Some(
            queue
                .try_borrow_mut()
                .map_err(|_| VmError::ServiceBorrowed("ow_skills::tree::init::PendingSkillTreeLoads"))?,
        ),
        None => None,
    };
    // W1: the first write.
    let tree = ow_vm::interp::instantiate_from_defaults(vm, plan.tree_class)?;
    let mut rest = |vm: &mut Vm| -> Result<(), VmError> {
        vm.space.heap.get_mut(tree)?.outer = Some(controller);
        vm.space.write(controller, PropAddr::scalar(plan.tree_prop), Value::Object(Some(tree)))?;
        if let (Some(request), Some(q)) = (plan.request.clone(), held.as_mut()) {
            record_request(q, request);
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
        // P3's class metadata, resolved at setup (reviews r1 and r2 of #475); its result, error included, is kept.
        setup(vm)?;
        ow_combat::hit::bind(
            vm,
            PATH,
            Rc::new(move |c: &mut NativeCall<'_>| {
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
