//! Owner: lane IMPL-LOGIN-STATUS (decision 2026-11-19-95). The two natives on the login status path. AI-assisted
//! (Claude Code).
//!
//! Coded from NATIVE_LOGIN_STATUS.md alone (analyst lane ANALYST-LOGIN-STATUS, decision 2026-11-19-89, as revised by
//! the reviews of #474 at a2d70e4). No native code or analysis output was opened. **Every rule is UNVERIFIED in the
//! running game.** Off only: under `ParityMode::Reference` the C++ binds neither, so the VM's stub policy answers (R3).
//!
//! **The note's section 7 is the contract.** Everything else on the path is installed script, which the VM runs as it
//! is:
//! - `UIInteraction.GetLoginStatus` is script (section 2), so there is nothing to bind for it;
//! - `WillowPlayerReplicationInfo.IsInvalidName` reaches it only for a local owner (`IsLocalPlayerController`, already
//!   bound in Off, decision 2026-11-19-15). That gate is the script's own, and nothing here bypasses it.
//!
//! **`GameEngine.GetOnlineSubsystem()`** (section 3), with four answers that exclude each other:
//! - **no engine object** (the world has registered no [`EngineObject`], the case of every headless world today): the
//!   note's named `NotYet { LayerAbsent }` naming the engine object and its online subsystem, never None (open
//!   question 1);
//! - **an engine object that is not a `GameEngine`** (nor below it): None, the native's own answer;
//! - **a `GameEngine` whose `OnlineSubsystem` slot is empty:** None, the field as stored (a world that makes a
//!   `GameEngine` and leaves the slot empty has chosen this);
//! - **a `GameEngine` with a subsystem in the slot:** that object when the world's set-up record ([`OnlineSetUp`])
//!   says its interfaces were initialised; otherwise the note's named `NotYet { NotRead }` for the player interface's
//!   initialisation (open question 2). The script reads `PlayerInterface` as a plain field right after this call, which
//!   no native sees, so the refusal is made here, the last native boundary before that read: an empty field is status 0
//!   only when a set-up modelled it as empty.
//!
//! A `GameEngine` that declares no object `OnlineSubsystem`, or a slot that holds another kind, is a data error. It
//! reads and writes nothing.
//!
//! **`OnlineSubsystemSteamworks.GetLoginStatus(LocalUserNum)`** (section 4). The rule is `LoggedInStatus` when the
//! number equals `LoggedInPlayerNum`, and 0 otherwise. The stored fields are used only for a subsystem whose login
//! state the world's set-up has filled ([`OnlineSetUp`]); no world fills one yet (a later lane's set-up decision, open
//! question 1). Any other Steamworks subsystem is the note's named `NotYet { NotRead }` naming its login state: its
//! class defaults are not the game's state. It writes nothing.
//!
//! **The natives after the login status** (`SetupInputDevices`, `UpdateInputContext`, `UpdateURL`, `SaveConfig`) are
//! each a named `NotYet { NotRead }` when reached ([`UNREAD`]). `SetName` and `ServerChangeName` are script and run as
//! installed; with an empty name they return with no effects.
//!
//! **Writes and the preflight.** Neither native writes. The note rules that the refusal is the **runtime check**, at
//! the call: the gates that decide whether the script calls `GetOnlineSubsystem` are not proven stable between a
//! caller's preflight and the call (open question 5). So no caller preflights it, and a caller that wrote before the
//! call marks the refusal itself (`crate::pools::after_partial_writes`: "stopped after partial writes", the kind and
//! record kept, the writes left in place).

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The record these rules come from.
pub const RECORD: &str = "docs/verification/NATIVE_LOGIN_STATUS.md";

/// `GameEngine.GetOnlineSubsystem`.
pub const GET_ONLINE_SUBSYSTEM: &str = "Engine.GameEngine.GetOnlineSubsystem";
/// `OnlineSubsystemSteamworks.GetLoginStatus`.
pub const STEAMWORKS_LOGIN_STATUS: &str = "OnlineSubsystemSteamworks.OnlineSubsystemSteamworks.GetLoginStatus";

/// The layer `GetOnlineSubsystem`'s `NotYet` names.
pub const ONLINE_LAYER: &str = "the engine object and its online subsystem (the port's world has none)";

/// The native key of `GameEngine.GetOnlineSubsystem()`.
pub const GET_ONLINE_SUBSYSTEM_KEY: &str = "GameEngine.GetOnlineSubsystem()";
/// The native key of `OnlineSubsystemSteamworks.GetLoginStatus(byte)`.
pub const STEAMWORKS_LOGIN_STATUS_KEY: &str = "OnlineSubsystemSteamworks.GetLoginStatus(byte)";

/// Binds the two natives by native key (Off only; the caller checks the mode). By key, not by function lookup, so the
/// binding holds whether or not the package that declares the function is loaded yet.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    vm.register_native(GET_ONLINE_SUBSYSTEM_KEY, Rc::new(get_online_subsystem))?;
    vm.register_native(STEAMWORKS_LOGIN_STATUS_KEY, Rc::new(steamworks_login_status))?;
    for &(key, what) in UNREAD {
        vm.register_native(key, Rc::new(move |_: &mut NativeCall<'_>| Err(unread(what))))?;
    }
    override_if_script(vm, SETUP_INPUT_DEVICES, Rc::new(setup_input_devices))?;
    override_if_script(vm, SERVER_CHANGE_NAME, Rc::new(server_change_name))
}

/// The controller's script `SetupInputDevices` (section 6.3), the boundary that owns the clear.
pub const SETUP_INPUT_DEVICES: &str = "WillowGame.WillowPlayerController.SetupInputDevices";
/// The controller's script `ServerChangeName` (section 6.2), the boundary of the game info's `ChangeName` dispatch.
pub const SERVER_CHANGE_NAME: &str = "Engine.PlayerController.ServerChangeName";
/// The base `GameInfo.ChangeName` (script, read by the note); an override was not read.
pub const BASE_CHANGE_NAME: &str = "Engine.GameInfo.ChangeName";

/// Overrides the script function at `path` in Off when the package set declares it as script; otherwise nothing.
fn override_if_script(vm: &mut Vm, path: &str, f: ow_vm::api::NativeFn) -> Result<(), VmError> {
    // A package set that does not declare the function skips it: the package is not indexed at all, or it has no such
    // export (`Ok(None)`). Any other lookup or load error propagates (review r2 of #479, item 1).
    let package = path.split_once('.').map_or(path, |(p, _)| p);
    let found = match vm.find_function_if_any(path) {
        Ok(found) => found,
        // The only refusal taken as absence: the package set has no package of that name at all.
        Err(_) if vm.defs.packages().paths_named(package).is_empty() => None,
        Err(e) => return Err(e),
    };
    if let Some(id) = found
        && vm.function(id).is_some_and(|d| !d.is_native())
    {
        vm.override_script(path, f)?;
    }
    Ok(())
}

/// The object property `name` of `o`; None when `o`'s class declares no such property or it holds None.
fn object_prop(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let Some(prop) = vm.space.prop(class, name) else {
        return Ok(None);
    };
    Ok(match vm.space.read(o, prop.into())? {
        Value::Object(v) | Value::Interface(v) => *v,
        _ => None,
    })
}

/// Calls the method `name` on `o`, resolved on its class and state; a class without one is a data error.
fn call_method(vm: &mut Vm, o: ObjRef, name: &str, args: Vec<Value>) -> Result<Value, VmError> {
    let (class, state) = {
        let obj = vm.space.heap.get(o)?;
        (obj.class, obj.state)
    };
    let f = vm.find_method(class, name, state)?.ok_or_else(|| bad(&format!("no {name} to call")))?;
    Ok(vm.call(f, Some(o), args)?.ret)
}

/// `WillowPlayerController.SetupInputDevices` in Off, the note's section 6.3 in order (review r1 of #479, item 1):
/// 1. with a `WillowPlayerInput` in `PlayerInput`, its native `SetupInputDevices`. Nothing was written yet by this
///    function, so its refusal passes unchanged;
/// 2. the clear of `InputContextSelector`, in either case;
/// 3. `UpdateInputContext`, in either case. Its refusal comes after the clear, so it is terminal here: "stopped after
///    partial writes: ", its kind and record kept, the clear left in place.
fn setup_input_devices(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let this = c.this.ok_or_else(|| bad("SetupInputDevices without a controller"))?;
    // What the clear and step 3 need is looked up first, before any call or write, so a controller class without
    // them is refused unmarked (review r2 of #479, item 2).
    let (class, state) = {
        let o = c.vm.space.heap.get(this)?;
        (o.class, o.state)
    };
    let prop = c.vm.space.prop(class, "InputContextSelector").ok_or_else(|| bad("no InputContextSelector"))?;
    let update =
        c.vm.find_method(class, "UpdateInputContext", state)?.ok_or_else(|| bad("no UpdateInputContext to call"))?;
    if let Some(input) = object_prop(c.vm, this, "PlayerInput")? {
        let input_class = c.vm.space.heap.get(input)?.class;
        if descends_from(c.vm, input_class, "WillowGame.WillowPlayerInput")? {
            call_method(c.vm, input, "SetupInputDevices", vec![])?;
        }
    }
    c.vm.space.write(this, ow_object::ids::PropAddr::scalar(prop), Value::Object(None))?;
    c.vm.call(update, Some(this), vec![]).map_err(crate::pools::after_partial_writes)?;
    Ok(Value::None)
}

/// `PlayerController.ServerChangeName(S)` in Off, the note's section 6.2 (review r1 of #479, item 4): an empty name
/// returns with no effects; with no `WorldInfo` or no game info, the call is skipped (the accessed-None answer).
/// Otherwise the game info's `ChangeName` is resolved first: the base `GameInfo.ChangeName` runs as installed with
/// `(Self, S, true)`, and an override (not read) is a named `NotYet { NotRead }` before its body runs.
fn server_change_name(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let this = c.this.ok_or_else(|| bad("ServerChangeName without a controller"))?;
    let name = match c.arg(0)? {
        Value::Str(s) => s.clone(),
        other => return Err(bad(&format!("ServerChangeName: S holds a {}, not a string", other.kind_name()))),
    };
    if name.is_empty() {
        return Ok(Value::None);
    }
    let Some(world) = object_prop(c.vm, this, "WorldInfo")? else {
        return Ok(Value::None);
    };
    let Some(game) = object_prop(c.vm, world, "Game")? else {
        return Ok(Value::None);
    };
    let (class, state) = {
        let g = c.vm.space.heap.get(game)?;
        (g.class, g.state)
    };
    let resolved = c.vm.find_method(class, "ChangeName", state)?;
    let base = c.vm.find_function(BASE_CHANGE_NAME).ok();
    match resolved {
        Some(f) if Some(f) == base => {
            c.vm.call(f, Some(game), vec![Value::Object(Some(this)), Value::Str(name), Value::Bool(true)])?;
            Ok(Value::None)
        }
        _ => {
            let path = c.vm.space.classes.get_layout(class).map(|l| l.path().as_str().to_string()).unwrap_or_default();
            Err(NotYet::not_read(
                format!("PlayerController.ServerChangeName: the game info {path}'s ChangeName, an override not read (section 6.2, open question 3)"),
                RECORD,
            )
            .into())
        }
    }
}

/// The natives after the login status that the note names but did not read (section 7), each bound in Off as its own
/// named `NotYet { NotRead }`, with the key the installed declaration carries. Each refuses only when the script
/// reaches it, in the script's order:
/// - `SetupInputDevices` (script) calls `WillowPlayerInput.SetupInputDevices` only with a `WillowPlayerInput`, so that
///   refusal comes **before** the script's clear of `InputContextSelector`;
/// - `WillowPlayerController.UpdateInputContext` is reached with or without a player input, **after** the clear, which
///   stays (no rollback);
/// - `PlayerController.UpdateURL` and `Object.SaveConfig` are reached only by `SetName` with a non-empty name, after
///   `ServerChangeName`. They write outside the world (the URL options and the player's settings). An empty name returns
///   from `SetName` with no effects, by its installed script, so nothing refuses then.
///
/// A refusal after an earlier write of the caller is terminal, marked by the caller (`crate::pools::after_partial_writes`).
pub const UNREAD: &[(&str, &str)] = &[
    (
        "WillowPlayerInput.SetupInputDevices()",
        "WillowPlayerInput.SetupInputDevices: the player input's device set-up (section 6.3, open question 4)",
    ),
    (
        "WillowPlayerController.UpdateInputContext()",
        "WillowPlayerController.UpdateInputContext: the controller's input context update (section 6.3, open question 4)",
    ),
    (
        "PlayerController.UpdateURL(string,string,bool)",
        "PlayerController.UpdateURL: the URL option write of SetName, outside the world (section 6.2, open question 3)",
    ),
    ("Object.SaveConfig()", "Object.SaveConfig: a configuration save, outside the world (section 6.2, open question 3)"),
];

fn unread(what: &'static str) -> VmError {
    NotYet::not_read(what, RECORD).into()
}

/// The engine object, when a world has one (a VM service). No world registers it yet: the port's engine is the world
/// itself, so `GetOnlineSubsystem` answers the no-engine `NotYet` (module documentation).
#[derive(Debug, Clone, Copy)]
pub struct EngineObject(pub ObjRef);

/// Registers the slice worlds' engine object (decision 2026-11-19-NNN, the hub's ruling of 2026-11-19, reversible): a
/// `GameEngine` made from its class defaults, with its `OnlineSubsystem` slot written empty.
///
/// **Port Off answer: the port has no online subsystem; UNVERIFIED against a logged-in game.** It is a fact of the
/// port, not an invented game value, and it matches a real game state (playing offline). No Steamworks object is made
/// and no login status is set, so `GetOnlineSubsystem` answers None by the note's empty-slot rule, and
/// `UIInteraction.GetLoginStatus` answers 0 by its script.
///
/// Off only: in Reference it refuses, so a Reference world is never given one. A world that has an engine object
/// already is refused too. Loading `Engine.GameEngine` here is world set-up, before any flow runs. A package set
/// that holds no `Engine.GameEngine` (a synthetic world) registers nothing and answers `Ok(None)`: its
/// `GetOnlineSubsystem` keeps the no-engine `NotYet { LayerAbsent }`.
pub fn register_offline_engine(vm: &mut Vm) -> Result<Option<ObjRef>, VmError> {
    let refused = |reason: &'static str| VmError::Registration {
        key: "EngineObject".to_string(),
        reason,
    };
    if vm.parity == ow_core::ParityMode::Reference {
        return Err(refused("the offline engine object is an Off answer; Reference keeps no engine object"));
    }
    if vm.service::<EngineObject>().is_some() {
        return Err(refused("the world has an engine object already"));
    }
    let Some(class) = vm.find_class_if_present(GAME_ENGINE)? else {
        return Ok(None);
    };
    // The declaration is checked before the object is made, so a refusal leaves nothing on the heap (review r1 of #501).
    let prop = vm.space.prop(class, "OnlineSubsystem").ok_or_else(|| bad("GameEngine declares no OnlineSubsystem"))?;
    let engine = ow_vm::interp::instantiate_from_defaults(vm, class)?;
    vm.space.write(engine, ow_object::ids::PropAddr::scalar(prop), Value::Object(None))?;
    vm.add_service(EngineObject(engine))?;
    Ok(Some(engine))
}

/// The world's set-up record of its online subsystems (a VM service). No world registers it yet.
/// - `interfaces`: the subsystems whose interfaces (`PlayerInterface` among them) a set-up step initialised. Only for
///   those is an empty `PlayerInterface` a modelled "empty", which the script answers with status 0.
/// - `login_state`: the Steamworks subsystems whose login state (`LoggedInStatus`, `LoggedInPlayerNum`) it filled.
#[derive(Debug, Clone, Default)]
pub struct OnlineSetUp {
    pub interfaces: Vec<ObjRef>,
    pub login_state: Vec<ObjRef>,
}

/// Whether `vm`'s set-up record lists `o` in the list `pick` chooses.
fn set_up(vm: &Vm, o: ObjRef, pick: fn(&OnlineSetUp) -> &Vec<ObjRef>) -> Result<bool, VmError> {
    match vm.service::<OnlineSetUp>() {
        Some(s) => {
            let record = s.try_borrow().map_err(|_| VmError::ServiceBorrowed("OnlineSetUp"))?;
            Ok(pick(&record).contains(&o))
        }
        None => Ok(false),
    }
}

/// The class `GetOnlineSubsystem` requires of the engine object.
pub const GAME_ENGINE: &str = "Engine.GameEngine";

/// What `GameEngine.GetOnlineSubsystem()` answers on `vm`, read only (module documentation): the subsystem; None for an
/// engine object that is not a `GameEngine` or for an empty slot; the named `NotYet { LayerAbsent }` with no engine.
pub fn subsystem(vm: &mut Vm) -> Result<Option<ObjRef>, VmError> {
    let Some(engine) = vm.service::<EngineObject>() else {
        return Err(NotYet::layer_absent(
            "GameEngine.GetOnlineSubsystem: the world has no engine object (section 7; never None, open question 1)",
            ONLINE_LAYER,
            RECORD,
        )
        .into());
    };
    let engine = engine.try_borrow().map_err(|_| VmError::ServiceBorrowed("EngineObject"))?.0;
    let class = vm.space.heap.get(engine)?.class;
    // The engine object's own class chain, read from the layout table: no class is looked up or loaded to answer
    // None (review r1 of #479, item 2).
    if !descends_from(vm, class, GAME_ENGINE)? {
        return Ok(None);
    }
    let Some(prop) = vm.space.prop(class, "OnlineSubsystem") else {
        return Err(bad("GameEngine declares no OnlineSubsystem"));
    };
    let sub = match vm.space.read(engine, prop.into())? {
        Value::Object(slot) => *slot,
        other => return Err(bad(&format!("GameEngine.OnlineSubsystem holds a {}, not an object", other.kind_name()))),
    };
    let Some(s) = sub else {
        return Ok(None);
    };
    if !set_up(vm, s, |r| &r.interfaces)? {
        return Err(unread(
            "GameEngine.GetOnlineSubsystem: the online subsystem's player interface initialisation, which no set-up \
             step recorded (section 7, open question 2); an empty PlayerInterface is not taken as status 0",
        ));
    }
    // The receiver every `PlayerInterface` dispatch on the login path will reach (review r1 of #479, item 3). Each
    // such dispatch first asks this native for the subsystem, and nothing on the path writes the field in between
    // (sections 2, 5 and 6.2), so the receiver is checked here: only the Steamworks rule was read.
    let receiver = match field(vm, s, "PlayerInterface")? {
        Value::Object(o) | Value::Interface(o) => o,
        other => return Err(bad(&format!("OnlineSubsystem.PlayerInterface holds a {}", other.kind_name()))),
    };
    if let Some(r) = receiver {
        let class = vm.space.heap.get(r)?.class;
        if !descends_from(vm, class, STEAMWORKS)? {
            let name = vm.space.classes.get_layout(class).map(|l| l.path().as_str().to_string()).unwrap_or_default();
            return Err(NotYet::not_read(
                format!(
                    "GameEngine.GetOnlineSubsystem: a PlayerInterface of class {name}, whose GetLoginStatus was not \
                     read (section 7; only the Steamworks rule was read)"
                ),
                RECORD,
            )
            .into());
        }
    }
    Ok(Some(s))
}

/// The Steamworks subsystem class, the one receiver whose `GetLoginStatus` the note read.
pub const STEAMWORKS: &str = "OnlineSubsystemSteamworks.OnlineSubsystemSteamworks";

/// Whether `class`, or a class above it, has the path `path`, walking the chain already in the layout table. No class
/// is looked up or loaded. A class id the table did not make is a data error.
fn descends_from(vm: &Vm, class: ow_object::ids::ClassId, path: &str) -> Result<bool, VmError> {
    let target = ow_core::ObjectPath::parse(path).map_err(|_| bad(&format!("{path} is not an object path")))?;
    let mut cursor = Some(class);
    while let Some(c) = cursor {
        let layout = vm.space.classes.get_layout(c).ok_or_else(|| bad("a class id the layout table did not make"))?;
        if *layout.path() == target {
            return Ok(true);
        }
        cursor = layout.super_class();
    }
    Ok(false)
}

fn bad(reason: &str) -> VmError {
    ow_attr::api::AttrError::BadData { reason: reason.to_string().into() }.into()
}

/// `GameEngine.GetOnlineSubsystem()` (module documentation).
fn get_online_subsystem(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    Ok(Value::Object(subsystem(c.vm)?))
}

/// The value of `this`'s property `name`; a data error when its class declares none.
fn field(vm: &Vm, this: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(this)?.class;
    let prop = vm.space.prop(class, name).ok_or_else(|| bad(&format!("the subsystem declares no {name}")))?;
    Ok(vm.space.read(this, prop.into())?.clone())
}

/// `OnlineSubsystemSteamworks.GetLoginStatus(LocalUserNum)` (module documentation).
fn steamworks_login_status(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let filled = match c.this {
        Some(this) => set_up(c.vm, this, |r| &r.login_state)?,
        None => false,
    };
    let Some(this) = c.this.filter(|_| filled) else {
        return Err(NotYet::not_read(
            "OnlineSubsystemSteamworks.GetLoginStatus: the subsystem's login state (LoggedInStatus, \
             LoggedInPlayerNum), which the world's set-up has not filled (section 7, open question 1)",
            RECORD,
        )
        .into());
    };
    let number = match c.arg(0)? {
        Value::Byte(b) => i32::from(*b),
        other => return Err(bad(&format!("GetLoginStatus: LocalUserNum holds a {}, not a byte", other.kind_name()))),
    };
    let player = match field(c.vm, this, "LoggedInPlayerNum")? {
        Value::Int(i) => i,
        other => return Err(bad(&format!("LoggedInPlayerNum holds a {}, not an int", other.kind_name()))),
    };
    if number != player {
        return Ok(Value::Byte(0));
    }
    match field(c.vm, this, "LoggedInStatus")? {
        Value::Byte(b) => Ok(Value::Byte(b)),
        other => Err(bad(&format!("LoggedInStatus holds a {}, not a byte", other.kind_name()))),
    }
}

#[cfg(test)]
#[path = "online_tests.rs"]
mod tests;
