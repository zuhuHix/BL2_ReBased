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
//!   that object's class. `SettingsArrayProviders` is not built and stays empty (the note's unread part).
//! - **At registration:** the store's script `Registered` is not run (the note's open question 1).
//! - **The provider:** every native declared on `UIDataProvider`, `UIDynamicDataProvider` and `UIDataProvider_Settings`
//!   ([`PROVIDER_FUNCTIONS`]) answers [`PROVIDER_NOT_YET`] when called on a settings provider. Another provider gets
//!   a `NotYet` of kind `NotRead` naming the native. The settings provider's own script functions are overridden to
//!   the same named `NotYet`. Script functions it inherits from its base classes are not overridden: they run as
//!   script, and stop at their first native call (UNVERIFIED gap).
//! - **Preflight** (the hub's ruling): the plan (the class defaults' list and classes, the provider class and its two
//!   fields, each entry's fields) is read before anything is made. A refusal there leaves nothing.
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
//! [`UNREAD`] (the two mission numbers and the DLC manager's two queries) stay `NotYet { NotRead }`. A `NotYet` that
//! comes after this refresh wrote state (the store's fill, or a setting) is terminal: [`PARTIAL_WRITES`], its kind
//! kept, nothing rolled back. A refresh begins at its data store client lookup, which resets that mark. Other errors
//! propagate unchanged.

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

/// The natives after the settings are found that stay unread: `NotYet { NotRead }` in Off, marked after a write
/// (module documentation).
pub const UNREAD: [&str; 4] = [
    "WillowGame.WillowPlayerController.GetActiveMissionNumber",
    "WillowGame.WillowPlayerController.GetActivePlotCriticalMissionNumber",
    "WillowGame.WillowDownloadableContentManager.HasCompatibilityData",
    "WillowGame.WillowDownloadableContentManager.StaticGetDlcContentIdForCurrentMap",
];

/// The world's data store client and its global data stores, made on the first `GetDataStoreClient`.
#[derive(Debug, Default)]
pub struct DataStores {
    client: Option<ObjRef>,
    globals: Vec<ObjRef>,
    /// Whether the current refresh wrote state (module documentation).
    written: bool,
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
            Rc::new(move |c: &mut NativeCall<'_>| -> Result<Value, VmError> {
                let refusal: VmError = NotYet::not_read(
                    format!("{path}: behind the online game settings refresh, not read (section 5)"),
                    RECORD,
                )
                .into();
                Err(if written(c.vm)? { after_writes(refusal) } else { refusal })
            }),
        )?;
    }
    bind(vm, SET_INT_PROPERTY, Rc::new(set_int_property))?;
    bind(vm, GET_CURR_PLAYTHROUGH, Rc::new(get_curr_playthrough))?;
    bind(vm, GET_DLC_MANAGER, Rc::new(|_c: &mut NativeCall<'_>| Ok(Value::Object(None))))?;
    for path in PROVIDER_FUNCTIONS {
        let Ok(f) = vm.find_function(path) else { continue };
        let native = vm.function(f).is_some_and(|d| d.is_native());
        let own = path.starts_with("Engine.UIDataProvider_Settings.");
        let refuse: NativeFn = Rc::new(move |c: &mut NativeCall<'_>| -> Result<Value, VmError> {
            let settings_provider = match (c.this, c.vm.space.classes.by_path(PROVIDER_CLASS)) {
                (Some(o), Some(p)) => c.vm.space.classes.is_child_of(c.vm.space.heap.get(o)?.class, p),
                _ => false,
            };
            let refusal: VmError = if settings_provider {
                NotYet::not_read(PROVIDER_NOT_YET, RECORD_FILL).into()
            } else {
                NotYet::not_read(format!("{path} on a provider that is not a settings provider: not read"), RECORD_FILL).into()
            };
            Err(if written(c.vm)? { after_writes(refusal) } else { refusal })
        });
        if native {
            bind(vm, path, refuse)?;
        } else if own {
            vm.override_script(path, refuse)?;
        }
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
    // A refresh begins here (module documentation): its write mark starts clear.
    s.written = false;
    if let Some(client) = s.client {
        return Ok(client);
    }
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
        s.written = true;
        globals.push(store);
        s.globals = globals.clone();
        if let Some(plan) = plan {
            apply_fill(vm, store, plan).map_err(after_writes)?;
        }
    }
    let client = ow_vm::interp::instantiate_from_defaults(vm, client_class).map_err(after_writes)?;
    s.client = Some(client);
    s.written = true;
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

/// The plan for a store of `class` (None for a store that is not an online game settings store), with every field it
/// will write checked on the classes (the cheap and certain part, before any write).
fn plan_fill(vm: &mut Vm, class: ClassId) -> Result<Option<FillPlan>, VmError> {
    const UNIT: &str = "UIDataStore_OnlineGameSettings (the settings fill)";
    let Some(settings_store) = vm.space.classes.by_path(SETTINGS_STORE_CLASS) else {
        return Ok(None);
    };
    if !vm.space.classes.is_child_of(class, settings_store) {
        return Ok(None);
    }
    let path = vm.space.classes.get_layout(class).map(|l| l.path().as_str().to_string()).unwrap_or_default();
    let defaults = vm.defs.class_default(&mut vm.space, &path);
    vm.log.extend(vm.defs.take_log());
    let defaults = defaults?;
    let provider_base = vm.find_class(PROVIDER_CLASS)?;
    let provider = match read_field(&vm.space, defaults, "SettingsProviderClass", UNIT)? {
        Value::Class(Some(c)) if vm.space.classes.is_child_of(c, provider_base) => c,
        Value::Class(_) => provider_base,
        other => return Err(fill_error(UNIT, format!("SettingsProviderClass holds {}", other.kind_name()))),
    };
    for field in ["Settings", "DataClass"] {
        if vm.space.prop(provider, field).is_none() {
            return Err(fill_error(UNIT, format!("the provider class declares no {field}")));
        }
    }
    let list = read_field(&vm.space, defaults, "GameSettingsCfgList", UNIT)?;
    let Some(list) = list.elements() else {
        return Err(fill_error(UNIT, "GameSettingsCfgList is not an array".to_string()));
    };
    let mut entries = Vec::with_capacity(list.len());
    for entry in list {
        for field in ["GameSettings", "Provider"] {
            struct_index(&vm.space, entry, field, UNIT)?;
        }
        match struct_get(&vm.space, entry, "GameSettingsClass", UNIT)? {
            Value::Class(c) => entries.push(c),
            other => return Err(fill_error(UNIT, format!("GameSettingsClass holds {}", other.kind_name()))),
        }
    }
    Ok(Some(FillPlan { entries, provider }))
}

/// Applies a fill plan to the new store (section 4): each entry's settings object from its class defaults, then its
/// provider bound to it (`Settings`, `DataClass`); `SettingsArrayProviders` stays empty; `Registered` is not run.
fn apply_fill(vm: &mut Vm, store: ObjRef, plan: &FillPlan) -> Result<(), VmError> {
    const UNIT: &str = "UIDataStore_OnlineGameSettings (the settings fill)";
    let list = read_field(&vm.space, store, "GameSettingsCfgList", UNIT)?;
    let mut entries = list.elements().map(<[Value]>::to_vec).unwrap_or_default();
    for (entry, class) in entries.iter_mut().zip(&plan.entries) {
        let Some(class) = class else { continue };
        let settings = ow_vm::interp::instantiate_from_defaults(vm, *class)?;
        struct_set(&vm.space, entry, "GameSettings", Value::Object(Some(settings)), UNIT)?;
        let provider = ow_vm::interp::instantiate_from_defaults(vm, plan.provider)?;
        write_field(&mut vm.space, provider, "Settings", Value::Object(Some(settings)), UNIT)?;
        write_field(&mut vm.space, provider, "DataClass", Value::Class(Some(*class)), UNIT)?;
        struct_set(&vm.space, entry, "Provider", Value::Object(Some(provider)), UNIT)?;
    }
    write_field(&mut vm.space, store, "GameSettingsCfgList", Value::array(entries), UNIT)
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

/// Notes that this refresh wrote state (the store's fill, a setting), so a later `NotYet` of the refresh is marked.
fn mark_written(vm: &Vm) -> Result<(), VmError> {
    service(vm)?.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("DataStores"))?.written = true;
    Ok(())
}

/// Whether this refresh wrote state.
fn written(vm: &Vm) -> Result<bool, VmError> {
    Ok(service(vm)?.try_borrow().map_err(|_| VmError::ServiceBorrowed("DataStores"))?.written)
}

#[cfg(test)]
#[path = "datastore_tests.rs"]
mod tests;
