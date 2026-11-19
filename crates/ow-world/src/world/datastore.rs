//! Owner: lane IMPL-ONLINE-GAME-SETTINGS (decision 2026-11-19-65). The engine's data store client, its global data
//! stores and the natives of the online game settings path a mission accept reaches. AI-assisted (Claude Code).
//!
//! Coded from NATIVE_ONLINE_GAME_SETTINGS.md alone (analyst lane ANALYST-NEXT-STOP, decision 2026-11-19-61). No native
//! code or analysis output was opened. **Every rule is UNVERIFIED in the running game.** Off only: under
//! `ParityMode::Reference` the C++ binds none of these, so the VM's stub policy answers (R3).
//!
//! **The note's section 5 is the contract** (hub ruling of 2026-11-19; research/rust-next-stop 40c0a89).
//!
//! **Branch 1, the online subsystem:** the host has none, so the subsystem's `GetGameSettings` is never called and
//! the refresh takes branch 2, the data store. That is the engine's fact, not a refusal: nothing is bound for it.
//!
//! **`UIInteraction.GetDataStoreClient()`** (section 2): the engine's data store client. The port's engine is the
//! world, and it has one client per world ([`DataStores`], a VM service), made on the first call from the
//! `Engine.DataStoreClient` class defaults together with its global data stores. So the note's fallbacks (the
//! `UIInteraction` class default's client, then None) are never reached here: the port's engine always has its client,
//! as the shipped game's does once it is running. Nothing else is read or written.
//!
//! **The global data stores** (section 4): the engine configuration lists the client's global data store classes. The
//! port reads no configuration file, so [`GLOBAL_DATA_STORES`] names the one the note read there that this path needs,
//! `WillowGame.WillowOnlineGameSettings_DataStore`. Each store is made from its class defaults, so its `Tag`
//! (`WillowOnlineGameSettings`) and its one `GameSettingsCfgList` entry with an empty `GameSettings` are the installed
//! defaults. Who fills `GameSettings` when the engine starts is the note's open question 2. Off does not assume it
//! empty (section 5: "never empty, so return with no writes"): `FindDataStore` answers such a store only when its first
//! entry's `GameSettings` is set, and is `NotYet { NotRead }` naming open question 2 otherwise. The script reads the
//! entry as a plain field right after the lookup, so the lookup is the last native boundary before that read.
//!
//! **`DataStoreClient.FindDataStore(Tag, optional Player)`** (section 3):
//! - a `None` tag answers None;
//! - with a player, that player's own data stores come first: the port models no player data stores, so a call with
//!   a player is `NotYet { NotPorted }`;
//! - then the client's global stores, in registration order, by `Tag` (names compare without case, as UE3 names do);
//! - no match: the game answers None, but the port registers only part of the configuration's global stores, so a tag
//!   none of them has is `NotYet { NotRead }` rather than a None the game might not give;
//! - called on another client than the world's: `NotYet { NotPorted }`.
//!
//! It changes nothing and creates nothing.
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
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

/// The record these rules come from.
pub const RECORD: &str = "docs/verification/NATIVE_ONLINE_GAME_SETTINGS.md";

/// `UIInteraction.GetDataStoreClient`.
pub const GET_DATA_STORE_CLIENT: &str = "Engine.UIInteraction.GetDataStoreClient";
/// `DataStoreClient.FindDataStore`.
pub const FIND_DATA_STORE: &str = "Engine.DataStoreClient.FindDataStore";
/// The class of the engine's data store client.
pub const CLIENT_CLASS: &str = "Engine.DataStoreClient";
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

/// Binds this module's natives (Off only; the caller checks the mode).
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

/// The world's client, made with its global stores on the first call: the stores first, then the client, and any
/// object made before a failure is freed again, so a failure leaves nothing behind.
pub fn data_store_client(vm: &mut Vm) -> Result<ObjRef, VmError> {
    let stores = service(vm)?;
    if let Some(client) = stores.try_borrow().map_err(|_| VmError::ServiceBorrowed("DataStores"))?.client {
        return Ok(client);
    }
    let mut made: Vec<ObjRef> = Vec::new();
    let result = (|| -> Result<ObjRef, VmError> {
        for path in GLOBAL_DATA_STORES {
            let class = vm.find_class(path)?;
            made.push(ow_vm::interp::instantiate_from_defaults(vm, class)?);
        }
        let class = vm.find_class(CLIENT_CLASS)?;
        let client = ow_vm::interp::instantiate_from_defaults(vm, class)?;
        made.push(client);
        Ok(client)
    })();
    match result {
        Ok(client) => {
            let mut s = stores.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("DataStores"))?;
            s.globals = made[..made.len() - 1].to_vec();
            s.client = Some(client);
            Ok(client)
        }
        Err(e) => {
            let _ = vm.space.free_many(&made);
            Err(e)
        }
    }
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
    let player = if c.arg_count() > 1 {
        match c.arg(1)? {
            Value::Object(o) | Value::Interface(o) => *o,
            _ => None,
        }
    } else {
        None
    };
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
    if player.is_some() {
        return Err(NotYet::not_ported(
            "DataStoreClient.FindDataStore with a player: a player's own data stores (the port models none)",
            RECORD,
        )
        .into());
    }
    for store in globals {
        let class = c.vm.space.heap.get(store)?.class;
        let p = c.vm.space.prop(class, "Tag").ok_or_else(|| VmError::Script {
            function: FIND_DATA_STORE.to_string(),
            message: format!("{} declares no Tag", ow_attr::eval::class_path(&c.vm.space, store)),
        })?;
        if let Value::Name(t) = c.vm.space.read(store, PropAddr::scalar(p))?
            && c.vm.space.names.text(*t).eq_ignore_ascii_case(&tag_text)
        {
            settings_known(c.vm, store)?;
            return Ok(Value::Object(Some(store)));
        }
    }
    Err(NotYet::not_read(
        format!(
            "DataStoreClient.FindDataStore: the tag {tag_text}, which no global data store the port registers has (the \
             engine configuration's other global stores are not registered)"
        ),
        RECORD,
    )
    .into())
}

/// The hub's ruling for this lane (review of the note, #443): Off does not assume a game settings store's settings
/// entry is empty. A store that declares `GameSettingsCfgList` is answered only when its first entry's `GameSettings`
/// is set; otherwise what fills it in a world with no online subsystem is not read (the note's open question 2), and
/// the lookup is `NotYet { NotRead }`, before the script reads the entry, rather than a store whose empty entry would
/// make the refresh write nothing on a guess. A store that declares no such list is answered as it is.
fn settings_known(vm: &Vm, store: ObjRef) -> Result<(), VmError> {
    let class = vm.space.heap.get(store)?.class;
    let Some(p) = vm.space.prop(class, "GameSettingsCfgList") else {
        return Ok(());
    };
    let first = match vm.space.read(store, PropAddr::scalar(p))? {
        Value::Array(items) => items.first().cloned(),
        _ => None,
    };
    let filled = first.as_ref().and_then(|entry| {
        let s = entry.as_struct()?;
        let layout = vm.space.classes.struct_layout(s.ty)?;
        let (at, _) = layout.field(vm.space.names.find("GameSettings")?)?;
        match s.fields.get(usize::try_from(at).ok()?)? {
            Value::Object(Some(_)) => Some(()),
            _ => None,
        }
    });
    if filled.is_some() {
        return Ok(());
    }
    Err(NotYet::not_read(
        format!(
            "DataStoreClient.FindDataStore: {}'s first GameSettingsCfgList entry has no GameSettings, and what fills it \
             in a world with no online subsystem is not read (open question 2; the hub does not assume it empty)",
            ow_attr::eval::class_path(&vm.space, store)
        ),
        RECORD,
    )
    .into())
}

#[cfg(test)]
#[path = "datastore_tests.rs"]
mod tests;
