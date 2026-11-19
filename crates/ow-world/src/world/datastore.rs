//! Owner: lane IMPL-ONLINE-GAME-SETTINGS (decision 2026-11-19-65). The engine's data store client, its global data
//! stores and the natives of the online game settings path a mission accept reaches. AI-assisted (Claude Code).
//!
//! Coded from NATIVE_ONLINE_GAME_SETTINGS.md alone (analyst lane ANALYST-NEXT-STOP, decision 2026-11-19-61). No native
//! code or analysis output was opened. **Every rule is UNVERIFIED in the running game.** Off only: under
//! `ParityMode::Reference` the C++ binds none of these, so the VM's stub policy answers (R3).
//!
//! **The note's section 5 is the contract** (hub ruling of 2026-11-19).
//!
//! **Branch 1, the online subsystem:** the host has none, so the subsystem's `GetGameSettings` is never called and
//! the refresh takes branch 2, the data store. That is the engine's fact, not a refusal: nothing is bound for it.
//!
//! **`UIInteraction.GetDataStoreClient()`** (section 2): the engine's data store client. The port's engine is the
//! world, and it has one client per world ([`DataStores`], a VM service), made on the first call from the
//! `Engine.DataStoreClient` class defaults together with its global data stores. The note's fallbacks (the
//! `UIInteraction` class default's client, then None) are out of scope for Off (section 5).
//! - **Before anything is made** (review r2 of #448, item 3): the service's borrow is taken, and every class the
//!   getter makes is found. A refusal there leaves nothing behind.
//! - **After that** (item 4, hub ruling): a failure while making the objects is terminal. A `NotYet` gets the
//!   [`PARTIAL_WRITES`] mark (its kind kept), any other error propagates unchanged, and nothing is rolled back.
//!
//! **The global data stores** (section 4): the engine configuration lists the client's global data store classes. The
//! port reads no configuration file, so [`GLOBAL_DATA_STORES`] names the one the note read there that this path needs,
//! `WillowGame.WillowOnlineGameSettings_DataStore`. It is made from its class defaults, so its `Tag`
//! (`WillowOnlineGameSettings`) and its settings list are the installed defaults.
//!
//! **`DataStoreClient.FindDataStore(Tag, optional Player)`** (section 3), the store lookup only:
//! - a `None` tag answers None;
//! - with a player, that player's own data stores are searched first. The port's players hold no data stores, so
//!   that search finds nothing and the lookup goes on to the global stores, as the note's step 3 does on a miss;
//! - the client's global stores, in registration order, by `Tag` (names compare without case, as UE3 names do);
//! - no match: None;
//! - called on another client than the world's: `NotYet { NotPorted }`.
//!
//! It changes nothing and creates nothing.
//!
//! **The settings entry read** (section 5, review r2 of #448, item 2): Off does not assume the store's settings entry
//! (`GameSettingsCfgList[0].GameSettings`) is empty, and reading it is a named `NotYet` naming open question 2. The
//! read is the controller's script `GetOnlineGameSettingsViaDatastore`, the one of the note's two route functions
//! that answers the settings. (Its sibling `GetOnlineGameSettingsDatastore`, with its `gameSettingsName` local, answers
//! the store; both by their installed declarations.) In Off that function is overridden: it runs the store lookup as
//! script; with no store it answers None, as the note's route does ("with no store ... the refresh does nothing");
//! otherwise it is the named `NotYet`, whatever the entry holds.
//!
//! **After the settings are found** (section 5, steps 1 to 5): field reads are plain reads and the `Update...` steps
//! are script, run as script. The natives among them are not read: [`UNREAD`] are bound in Off as
//! `NotYet { NotRead }` naming the note, so a run that reaches one stops there by name, with nothing changed:
//! `GetActiveMissionNumber`, `GetActivePlotCriticalMissionNumber`, `GetCurrPlaythrough`, `GameEngine.GetDLCManager`
//! and, behind its gate, `HasCompatibilityData` and `StaticGetDlcContentIdForCurrentMap`, and `Settings.SetIntProperty`
//! (each `Update...` writes through it). While the settings entry is a refusal, none of them is reached. A path the
//! package set does not declare as a native binds nothing.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

/// The record these rules come from.
pub const RECORD: &str = "docs/verification/NATIVE_ONLINE_GAME_SETTINGS.md";

/// The mark of a failure after state was written. The same text as `ow_behavior::kernel::run::PARTIAL_WRITES`
/// (`ow-world` cannot depend on `ow-behavior`), which the mission bridge and the slice commands stop on.
pub const PARTIAL_WRITES: &str = "stopped after partial writes";

/// `UIInteraction.GetDataStoreClient`.
pub const GET_DATA_STORE_CLIENT: &str = "Engine.UIInteraction.GetDataStoreClient";
/// `DataStoreClient.FindDataStore`.
pub const FIND_DATA_STORE: &str = "Engine.DataStoreClient.FindDataStore";
/// The class of the engine's data store client.
pub const CLIENT_CLASS: &str = "Engine.DataStoreClient";
/// The controller's script that reads the settings entry (overridden in Off; module documentation).
pub const SETTINGS_VIA_DATASTORE: &str = "WillowGame.WillowPlayerController.GetOnlineGameSettingsViaDatastore";
/// The controller's script that looks the store up (run as script).
pub const SETTINGS_DATASTORE: &str = "GetOnlineGameSettingsDatastore";
/// The global data store classes the port registers, in order: the one of the engine configuration's list this path
/// needs (module documentation).
pub const GLOBAL_DATA_STORES: [&str; 1] = ["WillowGame.WillowOnlineGameSettings_DataStore"];

/// The natives after the settings are found, not read: `NotYet { NotRead }` in Off (the note's section 5, in its
/// order; module documentation).
pub const UNREAD: [&str; 7] = [
    "WillowGame.WillowPlayerController.GetActiveMissionNumber",
    "WillowGame.WillowPlayerController.GetActivePlotCriticalMissionNumber",
    "WillowGame.WillowGameReplicationInfo.GetCurrPlaythrough",
    "Engine.GameEngine.GetDLCManager",
    "WillowGame.WillowDownloadableContentManager.HasCompatibilityData",
    "WillowGame.WillowDownloadableContentManager.StaticGetDlcContentIdForCurrentMap",
    "Engine.Settings.SetIntProperty",
];

/// The world's data store client and its global data stores, made on the first `GetDataStoreClient`.
#[derive(Debug, Default)]
pub struct DataStores {
    client: Option<ObjRef>,
    globals: Vec<ObjRef>,
}

impl DataStores {
    /// The client, once made.
    pub fn client(&self) -> Option<ObjRef> {
        self.client
    }

    /// The global data stores, in registration order.
    pub fn globals(&self) -> &[ObjRef] {
        &self.globals
    }
}

/// Binds this module's natives and the settings-entry override (Off only; the caller checks the mode).
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.service::<DataStores>().is_none() {
        vm.add_service(DataStores::default())?;
    }
    bind(vm, GET_DATA_STORE_CLIENT, Rc::new(get_data_store_client))?;
    bind(vm, FIND_DATA_STORE, Rc::new(find_data_store))?;
    for path in UNREAD {
        bind(
            vm,
            path,
            Rc::new(move |_c: &mut NativeCall<'_>| -> Result<Value, VmError> {
                Err(NotYet::not_read(
                    format!("{path}: behind the online game settings refresh, not read (section 5)"),
                    RECORD,
                )
                .into())
            }),
        )?;
    }
    // The settings entry read, where the package set declares the script.
    if let Ok(f) = vm.find_function(SETTINGS_VIA_DATASTORE)
        && vm.function(f).is_some_and(|d| !d.is_native())
    {
        vm.override_script(SETTINGS_VIA_DATASTORE, Rc::new(settings_via_datastore))?;
    }
    Ok(())
}

/// Binds `f` to the native `path` declares; a path the package set does not declare as a native binds nothing.
fn bind(vm: &mut Vm, path: &str, f: NativeFn) -> Result<(), VmError> {
    let Ok(id) = vm.find_function(path) else {
        return Ok(());
    };
    let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) else {
        return Ok(());
    };
    vm.register_native(&key, f)
}

fn service(vm: &Vm) -> Result<Rc<std::cell::RefCell<DataStores>>, VmError> {
    vm.service::<DataStores>().ok_or_else(|| {
        NotYet::layer_absent("the engine's data store client", "ow-world (a World)", RECORD).into()
    })
}

/// A failure after the getter began making objects (module documentation): a `NotYet` is marked [`PARTIAL_WRITES`]
/// with its kind kept; any other error is returned as it is.
fn after_writes(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) if !n.what.contains(PARTIAL_WRITES) => {
            NotYet::new(format!("{PARTIAL_WRITES}: {}", n.what), n.missing, n.record).into()
        }
        other => other,
    }
}

/// The world's client, made with its global stores on the first call (module documentation): the service's borrow
/// and every class are taken before anything is made; a failure after that is terminal, with nothing rolled back.
pub fn data_store_client(vm: &mut Vm) -> Result<ObjRef, VmError> {
    let stores = service(vm)?;
    let mut s = stores.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("DataStores"))?;
    if let Some(client) = s.client {
        return Ok(client);
    }
    let mut classes: Vec<ClassId> = Vec::with_capacity(GLOBAL_DATA_STORES.len());
    for path in GLOBAL_DATA_STORES {
        classes.push(vm.find_class(path)?);
    }
    let client_class = vm.find_class(CLIENT_CLASS)?;
    let mut globals = Vec::with_capacity(classes.len());
    for class in classes {
        let store = ow_vm::interp::instantiate_from_defaults(vm, class).map_err(after_writes)?;
        globals.push(store);
        s.globals = globals.clone();
    }
    let client = ow_vm::interp::instantiate_from_defaults(vm, client_class).map_err(after_writes)?;
    s.client = Some(client);
    Ok(client)
}

fn get_data_store_client(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    Ok(Value::Object(Some(data_store_client(c.vm)?)))
}

fn find_data_store(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let tag = match c.arg(0)? {
        Value::Name(n) => *n,
        other => {
            return Err(VmError::Script {
                function: FIND_DATA_STORE.to_string(),
                message: format!("Tag holds {}, not a name", other.kind_name()),
            });
        }
    };
    let tag_text = c.vm.space.names.text(tag).to_string();
    if tag_text.is_empty() || tag_text.eq_ignore_ascii_case("None") {
        return Ok(Value::Object(None));
    }
    let stores = service(c.vm)?;
    let (client, globals) = {
        let s = stores.try_borrow().map_err(|_| VmError::ServiceBorrowed("DataStores"))?;
        (s.client, s.globals.clone())
    };
    if c.this.is_none() || c.this != client {
        return Err(NotYet::not_ported(
            "DataStoreClient.FindDataStore on a client other than the world's engine client",
            RECORD,
        )
        .into());
    }
    // A player's own data stores come first (section 3, step 2); the port's players hold none, so that search finds
    // nothing and the global stores follow (step 3).
    for store in globals {
        let class = c.vm.space.heap.get(store)?.class;
        let p = c.vm.space.prop(class, "Tag").ok_or_else(|| VmError::Script {
            function: FIND_DATA_STORE.to_string(),
            message: format!("{} declares no Tag", ow_attr::eval::class_path(&c.vm.space, store)),
        })?;
        if let Value::Name(t) = c.vm.space.read(store, PropAddr::scalar(p))?
            && c.vm.space.names.text(*t).eq_ignore_ascii_case(&tag_text)
        {
            return Ok(Value::Object(Some(store)));
        }
    }
    Ok(Value::Object(None))
}

/// `WillowPlayerController.GetOnlineGameSettingsViaDatastore` in Off (module documentation): the store lookup runs as
/// script; no store answers None; a store's settings entry read is the named `NotYet` (open question 2).
fn settings_via_datastore(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| VmError::Script {
        function: SETTINGS_VIA_DATASTORE.to_string(),
        message: "called with no controller".to_string(),
    })?;
    let (class, state) = {
        let o = c.vm.space.heap.get(me)?;
        (o.class, o.state)
    };
    let f = c.vm.find_method(class, SETTINGS_DATASTORE, state)?.ok_or_else(|| VmError::Script {
        function: SETTINGS_VIA_DATASTORE.to_string(),
        message: format!("the controller has no {SETTINGS_DATASTORE}"),
    })?;
    let store = match c.vm.call(f, Some(me), Vec::new())?.ret {
        Value::Object(o) | Value::Interface(o) => o,
        _ => None,
    };
    let Some(store) = store else {
        return Ok(Value::Object(None));
    };
    Err(NotYet::not_read(
        format!(
            "WillowPlayerController.GetOnlineGameSettingsViaDatastore: the settings entry of {} \
             (GameSettingsCfgList[0].GameSettings): what fills it in a world with no online subsystem is not read \
             (open question 2; section 5: it is not assumed empty)",
            ow_attr::eval::class_path(&c.vm.space, store)
        ),
        RECORD,
    )
    .into())
}

#[cfg(test)]
#[path = "datastore_tests.rs"]
mod tests;
