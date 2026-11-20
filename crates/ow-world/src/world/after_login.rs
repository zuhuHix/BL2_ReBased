//! Owner: lane IMPL-LINK1-AFTER-LOGIN (decision 2026-11-19-113, number pending). The three natives the login path
//! reaches after `SetName` and `SetupInputDevices`. AI-assisted (Claude Code).
//!
//! Coded from NATIVE_LOGIN_URL_CONFIG_INPUT.md alone (#494; analyst lane ANALYST-LINK1-AFTER-LOGIN, not this thread).
//! No
//! native code or analysis output was opened. **Every rule is UNVERIFIED in the running game.** Off only: the caller
//! (`online::register`) binds these in Off, and Reference keeps the VM's stub policy.
//!
//! **`PlayerController.UpdateURL(NewOption, NewValue, bSaveDefault)`** (section 1): **nothing**, no write, `None`
//! returned. The port never writes the player's config, and the rule that adds the option to `GameEngine.LastURL` was
//! not read, so neither write is made. That is allowed only under the note's read-back check (section 1.3), kept as
//! the test `off_url_and_config_have_no_readers` (`ow-cli`, `cmd::after_login_census`):
//! 1. no script reads `GameEngine.LastURL` (the note's scan of the decodable script);
//! 2. the readers of URL options and saved default-player options (`PlayerController.GetDefaultURL`,
//!    `WorldInfo.GetLocalURL`, `WorldInfo.GetAddressURL`, `Actor.GetURLMap`, `Engine.BrowseToURL`) are reached only
//!    by travel and restart, off the slice path;
//! 3. none of those natives is bound in the slice drivers' Off worlds, and no crate reads a config file or the player's
//!    config folder. The test keeps this part: a binding for one of them, or a config reader, fails it, and the answer
//!    must then become the note's
//!    `NotYet { LayerAbsent }`, [`URL_LAYER_ABSENT`].
//!
//! **`Object.SaveConfig()`** (section 2): **nothing**, under the note's read-back check (section 2.3), kept by the same
//! test: saved config is read back only by a config load, the port reads no config file, and no crate binds
//! `Object.GetPerObjectConfigSections` or any config load. If that stops holding, the answer becomes
//! [`CONFIG_LAYER_ABSENT`].
//!
//! **`WillowPlayerController.UpdateInputContext()`** (section 3.2), the native's own rule:
//! 1. the controller's `PlayerInput`;
//! 2. None, or not a `WillowGame.WillowPlayerInput` (nor a subclass): return, no write. The slice's headless controller
//!    has no `PlayerInput`, so this is the slice's case on every path that reaches the native;
//! 3. a `WillowPlayerInput`: `bUpdateInputContext` = true on it, nothing else. A `WillowPlayerInput` class without a
//!    bool `bUpdateInputContext` is a data error (`VmError::Script`) at the write.
//!
//! The cast class `WillowPlayerInput` and the flag `bUpdateInputContext` are **UNVERIFIED (fitted)**: the note matched
//! them by fit (the flag is the matching declared bool; the layout tool cannot lay out `PlayerInput`), not by layout.
//!
//! None of the three refuses, so none needs a preflight; none loads anything. A `NotYet` these answers would become is
//! plain at the native's entry: the caller that wrote before the call adds the "stopped after partial writes: " prefix
//! (`crate::pools::after_partial_writes`), as `SetupInputDevices` does.
//!
//! **An accepted divergence** (the hub's steer): the slice's controllers have no `PlayerInput` at every call
//! (asserted in `ow-cli`'s slice tests where the call is reached). A world that sets one (links 13 and 14 with a
//! player input) gets the flag raised, and it stays raised: the port runs no input frame, so `WillowPlayerInput`'s
//! per-frame event never clears it.

use std::rc::Rc;

use ow_object::ids::PropAddr;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The record these rules come from.
pub const RECORD: &str = "docs/verification/NATIVE_LOGIN_URL_CONFIG_INPUT.md";
/// The native keys, as the installed declarations carry them (the keys #479 bound as `NotYet`).
pub const UPDATE_URL_KEY: &str = "PlayerController.UpdateURL(string,string,bool)";
pub const SAVE_CONFIG_KEY: &str = "Object.SaveConfig()";
pub const UPDATE_INPUT_CONTEXT_KEY: &str = "WillowPlayerController.UpdateInputContext()";
/// The player input class whose flag `UpdateInputContext` raises: UNVERIFIED (fitted), as is the flag
/// `bUpdateInputContext` (module documentation).
pub const WILLOW_PLAYER_INPUT: &str = "WillowGame.WillowPlayerInput";
/// The `NotYet` texts the two "nothing" answers become if their read-back checks stop holding (not used while they
/// hold; named here for the next lane).
pub const URL_LAYER_ABSENT: &str = "PlayerController.UpdateURL: the engine URL and the config layer";
pub const CONFIG_LAYER_ABSENT: &str = "Object.SaveConfig: the config layer";

const UNIT: &str = "WillowPlayerController.UpdateInputContext";

/// Binds the three natives by key (Off only; the caller checks the mode).
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    vm.register_native(UPDATE_URL_KEY, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::None)))?;
    vm.register_native(SAVE_CONFIG_KEY, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::None)))?;
    vm.register_native(UPDATE_INPUT_CONTEXT_KEY, Rc::new(update_input_context))
}

fn bad(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

/// A record of each `UpdateInputContext` call: whether the controller had a `PlayerInput` then. Kept only when a test
/// adds this service (the slice test's check that the slice's calls have none); the port never adds it.
#[derive(Debug, Default)]
pub struct InputContextCalls(pub Vec<bool>);

fn update_input_context(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| bad("called with no controller".to_string()))?;
    let class = c.vm.space.heap.get(me)?.class;
    let p = c
        .vm
        .space
        .prop(class, "PlayerInput")
        .ok_or_else(|| bad(format!("{} declares no PlayerInput", ow_attr::eval::class_path(&c.vm.space, me))))?;
    let input = match c.vm.space.read(me, PropAddr::scalar(p))? {
        Value::Object(o) => *o,
        other => return Err(bad(format!("PlayerInput holds {}, not an object", other.kind_name()))),
    };
    if let Some(log) = c.vm.service::<InputContextCalls>() {
        log.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("InputContextCalls"))?.0.push(input.is_some());
    }
    let Some(input) = input else { return Ok(Value::None) };
    let Some(willow) = c.vm.space.classes.by_path(WILLOW_PLAYER_INPUT) else { return Ok(Value::None) };
    let input_class = c.vm.space.heap.get(input)?.class;
    if !c.vm.space.classes.is_child_of(input_class, willow) {
        return Ok(Value::None);
    }
    let flag = c
        .vm
        .space
        .prop(input_class, "bUpdateInputContext")
        .ok_or_else(|| bad("WillowPlayerInput declares no bUpdateInputContext".to_string()))?;
    if !matches!(c.vm.space.classes.decl(flag).map(|d| &d.kind), Some(PropKind::Bool)) {
        return Err(bad("WillowPlayerInput.bUpdateInputContext is not a bool".to_string()));
    }
    c.vm.space.write(input, PropAddr::scalar(flag), Value::Bool(true))?;
    Ok(Value::None)
}
