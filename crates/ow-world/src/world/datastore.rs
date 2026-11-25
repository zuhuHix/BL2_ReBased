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
//! **The settings entry** (lane IMPL-SETTINGS-FILL, decision 2026-11-19-80; NATIVE_SETTINGS_FILL.md section 4, which
//! answers open question 2): the store's own initialisation fills it when the client makes the store.
//! - **At store creation:** for each entry of the store's `GameSettingsCfgList` (an online game settings store's class
//!   defaults), a new object of the entry's `GameSettingsClass` from its class defaults becomes its `GameSettings`.
//!   Then, only for an entry that got one, a new provider becomes its `Provider`: of the store's
//!   `SettingsProviderClass` when that is `UIDataProvider_Settings` or a subclass, else `UIDataProvider_Settings`.
//! - **At binding** (straight after): the provider's `Settings` becomes the new settings object and its `DataClass`
//!   that object's class. `SettingsArrayProviders` is not built: it is set empty when the provider is made, whatever
//!   a provider subclass's defaults hold (the note's unread part; review r1 of #464, item 6).
//! - **Publishing** (review r1 of #464, item 3): each entry's `GameSettings`, then its `Provider`, is written into the
//!   store's list as soon as it is made, in the note's order, so the writes made before a failure stay visible.
//! - **At registration:** the store's script `Registered` is not run (the note's open question 1).
//! - **The provider:** every native declared on `UIDataProvider`, `UIDynamicDataProvider` and `UIDataProvider_Settings`
//!   ([`PROVIDER_FUNCTIONS`]) answers [`PROVIDER_NOT_YET`] when called on a settings provider. Another provider gets
//!   a `NotYet` of kind `NotRead` naming the native. The script functions of those three classes are refused at
//!   dispatch with the same named `NotYet` when the receiver is a settings provider, inherited ones included (review
//!   r1 of #464, item 4); on another receiver they run as script. Functions of the provider's further bases
//!   (`UIRoot`, `Object`) are not the provider's and are not refused (UNVERIFIED gap).
//! - **Preflight** (the hub's ruling; review r1 of #464, item 2): the plan is checked before anything is made. First
//!   the declarations alone (the provider class, every field the fill reads or writes and its kind), then the class
//!   defaults' values (the list's classes, the provider class) through a read that publishes nothing
//!   (`DefStore::with_class_default`), then that every write's value is assignable to its field. A refusal there
//!   leaves nothing, not even the class default object. Before any class is taken (lane COLD-LOAD-FIXES), the getter
//!   asks the code index, read only, whether every class it needs is in the package set: the global stores and the
//!   client, then the settings provider class for a store that derives from the settings store. A missing one is
//!   `find_class`'s own error for it, with no class code loaded. The rest of the plan needs the classes' layouts, so
//!   it still follows `find_class`, which loads their code (a class load is a write; open).
//!
//! The installed `GetOnlineGameSettingsViaDatastore` script is no longer overridden: it reads the filled entry
//! (`GameSettingsCfgList[0].GameSettings`) as installed.
//!
//! **After the settings are found** (NATIVE_SETTINGS_FILL.md section 4, steps 5 to 9): field reads are plain reads and
//! the `Update...` steps are script, run as script. Bound in Off:
//! - `Settings.SetIntProperty` (section 3): the first entry with the id, when its data is the integer type, gets the
//!   value; otherwise nothing;
//! - `GetCurrPlaythrough`: the replication info's `CurrentPlaythrough`;
//! - `GameEngine.GetDLCManager`: None (the hub's ruling: the port's engine has no DLC manager), so the refresh skips
//!   the two DLC writes.
//!
//! - the two mission numbers, `GetActiveMissionNumber` and `GetActivePlotCriticalMissionNumber`, and the mission
//!   screen's `UpdateMissionTextList` the accept reaches after the refresh: [`super::mission_numbers`] (lane
//!   IMPL-MISSION-NUMBERS, decision 2026-11-19-93).
//!
//! [`UNREAD`] (the DLC manager's two queries) stay `NotYet { NotRead }`.
//!
//! **The refresh's write mark** (review r1 of #464, item 1): the refresh is the controller's script [`REFRESH`], run
//! as script; the port wraps it (`Vm::wrap_script`) and keeps one mark per running refresh. A write inside it (the
//! store's fill, a setting) sets the mark. A `NotYet` that leaves a refresh whose mark is set, from anything the
//! refresh called, is terminal: [`PARTIAL_WRITES`], its kind kept, nothing rolled back. Other errors leave unchanged.
//! A lookup or a setter called outside a refresh marks nothing, and a second lookup inside one does not clear its
//! mark. A nested refresh that wrote marks the outer one too.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{CallResult, NativeCall, NativeFn, ScriptBody, ScriptWrapFn, Vm};
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
/// The controller's refresh, wrapped for its write mark (module documentation).
pub const REFRESH: &str = "WillowGame.WillowPlayerController.UpdateOnlineGameSettings";
/// The controller's script that reads the settings entry (run as script since decision 2026-11-19-80).
pub const SETTINGS_VIA_DATASTORE: &str = "WillowGame.WillowPlayerController.GetOnlineGameSettingsViaDatastore";
/// The controller's script that looks the store up (run as script).
pub const SETTINGS_DATASTORE: &str = "GetOnlineGameSettingsDatastore";
/// The global data store classes the port registers, in order: the one of the engine configuration's list this path
/// needs (module documentation).
pub const GLOBAL_DATA_STORES: [&str; 1] = ["WillowGame.WillowOnlineGameSettings_DataStore"];

/// The natives after the settings are found that stay unread: `NotYet { NotRead }` in Off, marked after a write
/// (module documentation). The two mission numbers left this list with decision 2026-11-19-93.
pub const UNREAD: [&str; 2] = [
    "WillowGame.WillowDownloadableContentManager.HasCompatibilityData",
    "WillowGame.WillowDownloadableContentManager.StaticGetDlcContentIdForCurrentMap",
];

/// The world's data store client and its global data stores, made on the first `GetDataStoreClient`.
#[derive(Debug, Default)]
pub struct DataStores {
    client: Option<ObjRef>,
    globals: Vec<ObjRef>,
    /// One mark per running refresh, innermost last: whether it wrote state (module documentation).
    refresh: Vec<bool>,
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

/// Binds this module's natives, the provider refusals and the refresh's wrapper (Off only; the caller checks the mode).
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
    bind(vm, SET_INT_PROPERTY, Rc::new(set_int_property))?;
    bind(vm, GET_CURR_PLAYTHROUGH, Rc::new(get_curr_playthrough))?;
    bind(vm, GET_DLC_MANAGER, Rc::new(|_c: &mut NativeCall<'_>| Ok(Value::Object(None))))?;
    {
        use super::mission_numbers as m;
        bind(vm, m::ACTIVE_MISSION_NUMBER, Rc::new(m::n_active_mission_number))?;
        bind(vm, m::PLOT_CRITICAL_NUMBER, Rc::new(m::n_plot_critical_number))?;
        bind(vm, m::UPDATE_MISSION_TEXT_LIST, Rc::new(m::n_update_mission_text_list))?;
        bind(vm, m::GET_VARIABLE_OBJECT, Rc::new(m::n_movie_query))?;
        bind(vm, m::FOCUS_ON, Rc::new(m::n_focus))?;
        // Bound by dispatch key, which names no package: see the known limit in `mission_numbers`' documentation.
        bind(vm, m::RECORD_STATUS_CHANGED, Rc::new(m::n_status_changed_telemetry))?;
        bind(vm, m::RECORD_OBJECTIVE_CHANGED, Rc::new(m::n_objective_changed_telemetry))?;
    }
    for path in PROVIDER_FUNCTIONS {
        let Ok(f) = vm.find_function(path) else { continue };
        if vm.function(f).is_some_and(|d| d.is_native()) {
            bind(
                vm,
                path,
                Rc::new(move |c: &mut NativeCall<'_>| -> Result<Value, VmError> {
                    if is_settings_provider(c.vm, c.this)? {
                        Err(NotYet::not_read(PROVIDER_NOT_YET, RECORD_FILL).into())
                    } else {
                        let what = format!("{path} on a provider that is not a settings provider: not read");
                        Err(NotYet::not_read(what, RECORD_FILL).into())
                    }
                }),
            )?;
        } else {
            // A script function: refused at dispatch for a settings provider, run as script for anything else.
            let wrap: ScriptWrapFn = Rc::new(|vm: &mut Vm, this: Option<ObjRef>, body: &mut ScriptBody<'_>| {
                if is_settings_provider(vm, this)? {
                    return Err(NotYet::not_read(PROVIDER_NOT_YET, RECORD_FILL).into());
                }
                body(vm)
            });
            vm.wrap_script(path, wrap)?;
        }
    }
    if vm.find_function(REFRESH).is_ok() {
        vm.wrap_script(REFRESH, Rc::new(refresh))?;
    }
    Ok(())
}

/// Whether `this` is a `UIDataProvider_Settings` (or a subclass).
fn is_settings_provider(vm: &Vm, this: Option<ObjRef>) -> Result<bool, VmError> {
    Ok(match (this, vm.space.classes.by_path(PROVIDER_CLASS)) {
        (Some(o), Some(p)) => vm.space.classes.is_child_of(vm.space.heap.get(o)?.class, p),
        _ => false,
    })
}

/// The refresh's wrapper (module documentation): its own mark for the length of its script; a `NotYet` that leaves
/// it after a write is marked [`PARTIAL_WRITES`]; a write marks the outer refresh too.
fn refresh(vm: &mut Vm, _this: Option<ObjRef>, body: &mut ScriptBody<'_>) -> Result<CallResult, VmError> {
    let stores = service(vm)?;
    stores.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("DataStores"))?.refresh.push(false);
    let result = body(vm);
    let popped = stores.try_borrow_mut().map(|mut s| {
        let wrote = s.refresh.pop().unwrap_or(false);
        if wrote && let Some(outer) = s.refresh.last_mut() {
            *outer = true;
        }
        wrote
    });
    match (result, popped) {
        (Err(e), Ok(true)) => Err(after_writes(e)),
        (result, Ok(_)) => result,
        (Err(e), Err(_)) => Err(e),
        (Ok(_), Err(_)) => Err(VmError::ServiceBorrowed("DataStores")),
    }
}

/// Binds `f` to the native `path` declares; a path the package set does not declare as a native binds nothing.
fn bind(vm: &mut Vm, path: &str, f: NativeFn) -> Result<(), VmError> {
    let Ok(id) = vm.find_function(path) else {
        return Ok(());
    };
    if !vm.function(id).is_some_and(|d| d.is_native()) {
        return Ok(());
    }
    vm.register_native_for(id, f)
}

fn service(vm: &Vm) -> Result<Rc<std::cell::RefCell<DataStores>>, VmError> {
    vm.service::<DataStores>().ok_or_else(|| {
        NotYet::layer_absent("the engine's data store client", "ow-world (a World)", RECORD).into()
    })
}

/// A failure after the getter began making objects (module documentation): a `NotYet` is marked [`PARTIAL_WRITES`]
/// with its kind kept; any other error is returned as it is.
pub(super) fn after_writes(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) => n.after_partial_writes().into(),
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
    classes_present(vm)?;
    let mut classes: Vec<ClassId> = Vec::with_capacity(GLOBAL_DATA_STORES.len());
    for path in GLOBAL_DATA_STORES {
        classes.push(vm.find_class(path)?);
    }
    let client_class = vm.find_class(CLIENT_CLASS)?;
    // Each store's fill is planned before anything is made (the settings fill's preflight).
    let mut plans = Vec::with_capacity(classes.len());
    for &class in &classes {
        plans.push(plan_fill(vm, class)?);
    }
    let mut globals = Vec::with_capacity(classes.len());
    for (class, plan) in classes.into_iter().zip(&plans) {
        let store = ow_vm::interp::instantiate_from_defaults(vm, class).map_err(after_writes)?;
        if let Some(mark) = s.refresh.last_mut() {
            *mark = true;
        }
        globals.push(store);
        s.globals = globals.clone();
        if let Some(plan) = plan {
            apply_fill(vm, store, plan, &mut ow_vm::interp::instantiate_from_defaults).map_err(after_writes)?;
        }
    }
    let client = ow_vm::interp::instantiate_from_defaults(vm, client_class).map_err(after_writes)?;
    s.client = Some(client);
    if let Some(mark) = s.refresh.last_mut() {
        *mark = true;
    }
    Ok(client)
}

/// The getter's presence check (module documentation, "Preflight"; lane COLD-LOAD-FIXES): every class it takes, in
/// its order, then the settings provider class where a store derives from the settings store, each looked up in the
/// code index by path, read only. The first one no package holds is `find_class`'s own error for it, which loads no
/// class code, since there is no class to load.
fn classes_present(vm: &mut Vm) -> Result<(), VmError> {
    let missing = {
        let (code, store) = vm.defs.code_mut();
        let mut missing = None;
        let mut keys = Vec::with_capacity(GLOBAL_DATA_STORES.len());
        for path in GLOBAL_DATA_STORES.iter().copied().chain([CLIENT_CLASS]) {
            match code.find_class(store, path) {
                Ok(k) => keys.push(k),
                Err(_) => {
                    missing = Some(path);
                    break;
                }
            }
        }
        if missing.is_none()
            && let Ok(settings) = code.find_class(store, SETTINGS_STORE_CLASS)
            && keys.iter().take(GLOBAL_DATA_STORES.len()).any(|&k| code.is_child_of(k, settings))
            && code.find_class(store, PROVIDER_CLASS).is_err()
        {
            missing = Some(PROVIDER_CLASS);
        }
        missing
    };
    match missing {
        Some(path) => vm.find_class(path).map(|_| ()),
        None => Ok(()),
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

// ---------------------------------------------------------------------------------------------------------------
// The settings fill (lane IMPL-SETTINGS-FILL, decision 2026-11-19-80), from NATIVE_SETTINGS_FILL.md section 4
// ---------------------------------------------------------------------------------------------------------------

/// The record of the settings fill.
pub const RECORD_FILL: &str = "docs/verification/NATIVE_SETTINGS_FILL.md";
/// The settings provider class (section 1, step 2).
pub const PROVIDER_CLASS: &str = "Engine.UIDataProvider_Settings";
/// The store class whose entries are filled at creation.
pub const SETTINGS_STORE_CLASS: &str = "Engine.UIDataStore_OnlineGameSettings";
/// The enum and entry of the integer settings data type `SetIntProperty` tests (section 3).
pub const DATA_TYPE_ENUM: &str = "Engine.Settings.ESettingsDataType";
pub const INT_DATA_TYPE: &str = "SDT_Int32";
/// The named `NotYet` any call on the settings provider answers (section 4).
pub const PROVIDER_NOT_YET: &str = "UIDataProvider_Settings: the settings provider's list providers are not built \
                                    (NATIVE_SETTINGS_FILL.md section 4)";
/// The functions a call on the provider can reach, by their installed declarations: the natives of the provider's
/// classes (each refused for a settings provider), and the settings provider's own script functions.
pub const PROVIDER_FUNCTIONS: [&str; 24] = [
    "Engine.UIDataProvider.OnDataProviderPropertyChange",
    "Engine.UIDataProvider.GetProviderFieldType",
    "Engine.UIDataProvider.ParseArrayDelimiter",
    "Engine.UIDataProvider.GetSupportedScriptFields",
    "Engine.UIDataProvider.AllowPublishingToField",
    "Engine.UIDataProvider.GetFieldValue",
    "Engine.UIDataProvider.SetFieldValue",
    "Engine.UIDataProvider.GenerateScriptMarkupString",
    "Engine.UIDataProvider.GenerateFillerData",
    "Engine.UIDataProvider.IsProviderDisabled",
    "Engine.UIDataProvider.IsCollectionDataType",
    "Engine.UIDataProvider.NotifyPropertyChanged",
    "Engine.UIDataProvider.AddPropertyNotificationChangeRequest",
    "Engine.UIDataProvider.RemovePropertyNotificationChangeRequest",
    "Engine.UIDataProvider.ParseTagArrayDelimiter",
    "Engine.UIDynamicDataProvider.BindProviderInstance",
    "Engine.UIDynamicDataProvider.UnbindProviderInstance",
    "Engine.UIDynamicDataProvider.ProviderInstanceBound",
    "Engine.UIDynamicDataProvider.ProviderInstanceUnbound",
    "Engine.UIDynamicDataProvider.IsValidDataSourceClass",
    "Engine.UIDynamicDataProvider.GetDataSource",
    "Engine.UIDynamicDataProvider.CleanupDataProvider",
    "Engine.UIDataProvider_Settings.ArrayProviderPropertyChanged",
    "Engine.UIDataProvider_Settings.OnSettingValueUpdated",
];

/// The natives the fill binds in Off: the setter and the two engine reads (section 3, and the hub's DLC ruling).
pub const SET_INT_PROPERTY: &str = "Engine.Settings.SetIntProperty";
pub const GET_CURR_PLAYTHROUGH: &str = "WillowGame.WillowGameReplicationInfo.GetCurrPlaythrough";
pub const GET_DLC_MANAGER: &str = "Engine.GameEngine.GetDLCManager";

fn fill_error(function: &str, message: String) -> VmError {
    VmError::Script { function: function.to_string(), message }
}

/// The value of `o.name`; a class that does not declare it is bad data.
fn read_field(space: &ow_object::heap::ObjectSpace, o: ObjRef, name: &str, unit: &str) -> Result<Value, VmError> {
    let class = space.heap.get(o)?.class;
    let p = space.prop(class, name).ok_or_else(|| {
        fill_error(unit, format!("{} declares no {name}", ow_attr::eval::class_path(space, o)))
    })?;
    Ok(space.read(o, PropAddr::scalar(p))?.clone())
}

fn write_field(space: &mut ow_object::heap::ObjectSpace, o: ObjRef, name: &str, v: Value, unit: &str) -> Result<(), VmError> {
    let class = space.heap.get(o)?.class;
    let p = space.prop(class, name).ok_or_else(|| {
        fill_error(unit, format!("{} declares no {name}", ow_attr::eval::class_path(space, o)))
    })?;
    space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// The index of struct field `name` in `v`, or bad data.
fn struct_index(space: &ow_object::heap::ObjectSpace, v: &Value, name: &str, unit: &str) -> Result<usize, VmError> {
    let bad = || fill_error(unit, format!("a struct without the field {name}"));
    let s = v.as_struct().ok_or_else(bad)?;
    let id = space.names.find(name).ok_or_else(bad)?;
    let (index, _) = space.classes.struct_layout(s.ty).and_then(|l| l.field(id)).ok_or_else(bad)?;
    usize::try_from(index).map_err(|_| bad())
}

fn struct_get(space: &ow_object::heap::ObjectSpace, v: &Value, name: &str, unit: &str) -> Result<Value, VmError> {
    let i = struct_index(space, v, name, unit)?;
    v.as_struct().and_then(|s| s.fields.get(i).cloned()).ok_or_else(|| fill_error(unit, format!("no value for {name}")))
}

fn struct_set(space: &ow_object::heap::ObjectSpace, v: &mut Value, name: &str, x: Value, unit: &str) -> Result<(), VmError> {
    let i = struct_index(space, v, name, unit)?;
    let slot = v.struct_mut().and_then(|s| s.fields.get_mut(i)).ok_or_else(|| fill_error(unit, format!("no slot for {name}")))?;
    *slot = x;
    Ok(())
}

/// The fill of one settings store, decided before anything is made (section 4, "at store creation" and "at binding"):
/// for each entry of the class defaults' `GameSettingsCfgList`, its settings class (None: that entry stays empty, as
/// the making fails); and the provider class, the store's `SettingsProviderClass` when it is `UIDataProvider_Settings`
/// or a subclass, else `UIDataProvider_Settings`.
struct FillPlan {
    entries: Vec<Option<ClassId>>,
    provider: ClassId,
}

const FILL_UNIT: &str = "UIDataStore_OnlineGameSettings (the settings fill)";

/// The declared kind of `class.name`, or bad data.
fn prop_kind(space: &ow_object::heap::ObjectSpace, class: ClassId, name: &str) -> Result<PropKind, VmError> {
    space
        .prop(class, name)
        .and_then(|p| space.classes.decl(p))
        .map(|d| d.kind.clone())
        .ok_or_else(|| fill_error(FILL_UNIT, format!("{} declares no {name}", class_name(space, class))))
}

/// The declared kind of field `name` of struct `ty`, or bad data.
fn field_kind(space: &ow_object::heap::ObjectSpace, ty: StructId, name: &str) -> Result<PropKind, VmError> {
    let id = space.names.find(name);
    id.and_then(|id| space.classes.struct_layout(ty).and_then(|l| l.field(id)).map(|(_, d)| d.kind.clone()))
        .ok_or_else(|| fill_error(FILL_UNIT, format!("the settings entry struct declares no {name}")))
}

fn class_name(space: &ow_object::heap::ObjectSpace, class: ClassId) -> String {
    space.classes.get_layout(class).map(|l| l.path().as_str().to_string()).unwrap_or_default()
}

/// Whether an object field (`Object` kind) can hold an object of class `c`. A class field cannot (review r3 of #464,
/// item 4): the fill writes objects there.
fn holds_object(space: &ow_object::heap::ObjectSpace, kind: &PropKind, c: ClassId) -> bool {
    matches!(kind, PropKind::Object { class } if space.classes.is_child_of(c, *class))
}

/// Whether a class field (`Class` kind) can hold the class `c`.
fn holds_class(space: &ow_object::heap::ObjectSpace, kind: &PropKind, c: ClassId) -> bool {
    matches!(kind, PropKind::Class { meta } if space.classes.is_child_of(c, *meta))
}

/// The plan for a store of `class` (None for a store that is not an online game settings store). Nothing is made and
/// nothing is published on any path (module documentation, "Preflight"): the declarations first, then the class
/// defaults' values through a read that publishes nothing, then the assignability of every value the fill writes.
fn plan_fill(vm: &mut Vm, class: ClassId) -> Result<Option<FillPlan>, VmError> {
    let Some(settings_store) = vm.space.classes.by_path(SETTINGS_STORE_CLASS) else {
        return Ok(None);
    };
    if !vm.space.classes.is_child_of(class, settings_store) {
        return Ok(None);
    }
    // 1. The declarations alone.
    let provider_base = vm.find_class(PROVIDER_CLASS)?;
    let space = &vm.space;
    let bad_kind = |what: &str| fill_error(FILL_UNIT, format!("{what} is not declared as the fill needs"));
    let (settings_field, data_class_field) = (prop_kind(space, provider_base, "Settings")?, prop_kind(space, provider_base, "DataClass")?);
    if !matches!(settings_field, PropKind::Object { .. }) {
        return Err(bad_kind("UIDataProvider_Settings.Settings"));
    }
    if !matches!(data_class_field, PropKind::Class { .. }) {
        return Err(bad_kind("UIDataProvider_Settings.DataClass"));
    }
    if !matches!(prop_kind(space, provider_base, "SettingsArrayProviders")?, PropKind::Array(_)) {
        return Err(bad_kind("UIDataProvider_Settings.SettingsArrayProviders"));
    }
    if !matches!(prop_kind(space, class, "SettingsProviderClass")?, PropKind::Class { .. }) {
        return Err(bad_kind("SettingsProviderClass"));
    }
    let PropKind::Array(inner) = prop_kind(space, class, "GameSettingsCfgList")? else {
        return Err(bad_kind("GameSettingsCfgList"));
    };
    let PropKind::Struct(cfg) = *inner else {
        return Err(bad_kind("GameSettingsCfgList"));
    };
    if !matches!(field_kind(space, cfg, "GameSettingsClass")?, PropKind::Class { .. }) {
        return Err(bad_kind("GameSettingsCfg.GameSettingsClass"));
    }
    let (settings_slot, provider_slot) = (field_kind(space, cfg, "GameSettings")?, field_kind(space, cfg, "Provider")?);
    // 2. The class defaults' values, read without publishing the class default object.
    let path = class_name(space, class);
    let read = vm.defs.with_class_default(&mut vm.space, &path, |space, defaults| {
        let provider = match read_field(space, defaults, "SettingsProviderClass", FILL_UNIT)? {
            Value::Class(c) => c,
            other => return Err(fill_error(FILL_UNIT, format!("SettingsProviderClass holds {}", other.kind_name()))),
        };
        let list = read_field(space, defaults, "GameSettingsCfgList", FILL_UNIT)?;
        let Some(list) = list.elements() else {
            return Err(fill_error(FILL_UNIT, "GameSettingsCfgList is not an array".to_string()));
        };
        let mut entries = Vec::with_capacity(list.len());
        for entry in list {
            match struct_get(space, entry, "GameSettingsClass", FILL_UNIT)? {
                Value::Class(c) => entries.push(c),
                other => return Err(fill_error(FILL_UNIT, format!("GameSettingsClass holds {}", other.kind_name()))),
            }
        }
        Ok((provider, entries))
    });
    vm.log.extend(vm.defs.take_log());
    let (provider, entries) = read??;
    let space = &vm.space;
    let provider = match provider {
        Some(c) if space.classes.is_child_of(c, provider_base) => c,
        _ => provider_base,
    };
    // 3. Every value the fill writes is assignable to the field the write resolves (review r3 of #464, item 4): the
    // provider's fields on the selected provider class, which may shadow the base class's; objects into object
    // fields, the settings class into a class field.
    let (settings_field, data_class_field) = (prop_kind(space, provider, "Settings")?, prop_kind(space, provider, "DataClass")?);
    if !matches!(prop_kind(space, provider, "SettingsArrayProviders")?, PropKind::Array(_)) {
        return Err(bad_kind("the provider class's SettingsArrayProviders"));
    }
    if !holds_object(space, &provider_slot, provider) {
        return Err(fill_error(FILL_UNIT, format!("GameSettingsCfg.Provider cannot hold a {}", class_name(space, provider))));
    }
    for &c in entries.iter().flatten() {
        let fits = [
            (holds_object(space, &settings_slot, c), "GameSettingsCfg.GameSettings"),
            (holds_object(space, &settings_field, c), "the provider's Settings"),
            (holds_class(space, &data_class_field, c), "the provider's DataClass"),
        ];
        for (ok, field) in fits {
            if !ok {
                return Err(fill_error(FILL_UNIT, format!("{field} cannot hold a {}", class_name(space, c))));
            }
        }
    }
    Ok(Some(FillPlan { entries, provider }))
}

/// Applies a fill plan to the new store (section 4), publishing as it goes (module documentation): for each entry,
/// its settings object from its class defaults, written into the store's list; then its provider, made with an empty
/// `SettingsArrayProviders`, written into the list; then the provider bound to the settings (`Settings`,
/// `DataClass`). `Registered` is not run. `make` makes each object from its class defaults
/// (`instantiate_from_defaults`; a test passes one that fails).
fn apply_fill(
    vm: &mut Vm,
    store: ObjRef,
    plan: &FillPlan,
    make: &mut dyn FnMut(&mut Vm, ClassId) -> Result<ObjRef, VmError>,
) -> Result<(), VmError> {
    let list = read_field(&vm.space, store, "GameSettingsCfgList", FILL_UNIT)?;
    let mut entries = list.elements().map(<[Value]>::to_vec).unwrap_or_default();
    for (i, class) in plan.entries.iter().enumerate().take(entries.len()) {
        let Some(class) = *class else { continue };
        let settings = make(vm, class)?;
        struct_set(&vm.space, &mut entries[i], "GameSettings", Value::Object(Some(settings)), FILL_UNIT)?;
        write_field(&mut vm.space, store, "GameSettingsCfgList", Value::array(entries.clone()), FILL_UNIT)?;
        let provider = make(vm, plan.provider)?;
        write_field(&mut vm.space, provider, "SettingsArrayProviders", Value::array(vec![]), FILL_UNIT)?;
        struct_set(&vm.space, &mut entries[i], "Provider", Value::Object(Some(provider)), FILL_UNIT)?;
        write_field(&mut vm.space, store, "GameSettingsCfgList", Value::array(entries.clone()), FILL_UNIT)?;
        write_field(&mut vm.space, provider, "Settings", Value::Object(Some(settings)), FILL_UNIT)?;
        write_field(&mut vm.space, provider, "DataClass", Value::Class(Some(class)), FILL_UNIT)?;
    }
    Ok(())
}

/// `Settings.SetIntProperty(PropertyId, Value)` in Off (section 3): the first `Properties` entry with that
/// `PropertyId`, when its data is of the integer type (`SDT_Int32`, by its enum entry name), gets `Value` as its
/// integer value (`SettingsData.Value1`, the data's one int field; the note says "its value": UNVERIFIED which field
/// the game writes). Otherwise nothing happens. The enum and every field are read before the write.
fn set_int_property(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const UNIT: &str = "Settings.SetIntProperty";
    let me = c.this.ok_or_else(|| fill_error(UNIT, "called with no settings object".to_string()))?;
    let (Value::Int(id), Value::Int(value)) = (c.arg(0)?.clone(), c.arg(1)?.clone()) else {
        return Err(fill_error(UNIT, "the arguments are not ints".to_string()));
    };
    let list = read_field(&c.vm.space, me, "Properties", UNIT)?;
    let Some(entries) = list.elements() else {
        return Err(fill_error(UNIT, "Properties is not an array".to_string()));
    };
    let mut entries = entries.to_vec();
    let mut hit = None;
    for (i, entry) in entries.iter().enumerate() {
        if struct_get(&c.vm.space, entry, "PropertyId", UNIT)? == Value::Int(id) {
            hit = Some(i);
            break;
        }
    }
    let Some(i) = hit else { return Ok(Value::None) };
    let mut data = struct_get(&c.vm.space, &entries[i], "Data", UNIT)?;
    let int_type = int_data_type(&c.vm.space, &data, UNIT)?;
    if struct_get(&c.vm.space, &data, "Type", UNIT)? != Value::Byte(int_type) {
        return Ok(Value::None);
    }
    struct_set(&c.vm.space, &mut data, "Value1", Value::Int(value), UNIT)?;
    struct_set(&c.vm.space, &mut entries[i], "Data", data, UNIT)?;
    write_field(&mut c.vm.space, me, "Properties", Value::array(entries), UNIT)?;
    mark_written(c.vm)?;
    Ok(Value::None)
}

/// The byte of the integer settings data type: the entry `SDT_Int32` of the enum the data's `Type` field declares (the
/// package's own enum, by its entry name), read before any write.
fn int_data_type(space: &ow_object::heap::ObjectSpace, data: &Value, unit: &str) -> Result<u8, VmError> {
    let ty = data.as_struct().map(|s| s.ty).ok_or_else(|| fill_error(unit, "Data is not a struct".to_string()))?;
    let id = space.names.find("Type").ok_or_else(|| fill_error(unit, "no Type field".to_string()))?;
    let kind = space.classes.struct_layout(ty).and_then(|l| l.field(id)).map(|(_, d)| d.kind.clone());
    let Some(ow_object::layout::PropKind::Byte { enum_: Some(e) }) = kind else {
        return Err(fill_error(unit, format!("SettingsData.Type is not a byte of {DATA_TYPE_ENUM}")));
    };
    let layout = space.classes.enum_layout(e).ok_or_else(|| fill_error(unit, "the enum of Type is undefined".to_string()))?;
    if layout.path().as_str() != DATA_TYPE_ENUM {
        return Err(fill_error(unit, format!("SettingsData.Type names {}, not {DATA_TYPE_ENUM}", layout.path().as_str())));
    }
    let at = layout.values().iter().position(|n| space.names.text(*n) == INT_DATA_TYPE);
    u8::try_from(at.ok_or_else(|| fill_error(unit, format!("{DATA_TYPE_ENUM} has no {INT_DATA_TYPE}")))?)
        .map_err(|_| fill_error(unit, "the enum entry is past a byte".to_string()))
}

/// `WillowGameReplicationInfo.GetCurrPlaythrough()` in Off (section 3): its `CurrentPlaythrough`.
fn get_curr_playthrough(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const UNIT: &str = "WillowGameReplicationInfo.GetCurrPlaythrough";
    let me = c.this.ok_or_else(|| fill_error(UNIT, "called with no replication info".to_string()))?;
    match read_field(&c.vm.space, me, "CurrentPlaythrough", UNIT)? {
        Value::Int(i) => Ok(Value::Int(i)),
        Value::Byte(b) => Ok(Value::Int(i32::from(b))),
        other => Err(fill_error(UNIT, format!("CurrentPlaythrough holds {}", other.kind_name()))),
    }
}

/// Notes that the running refresh, if any, wrote state (module documentation).
pub(super) fn mark_written(vm: &Vm) -> Result<(), VmError> {
    let stores = service(vm)?;
    let mut s = stores.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("DataStores"))?;
    if let Some(mark) = s.refresh.last_mut() {
        *mark = true;
    }
    Ok(())
}

#[cfg(test)]
#[path = "datastore_tests.rs"]
mod tests;
