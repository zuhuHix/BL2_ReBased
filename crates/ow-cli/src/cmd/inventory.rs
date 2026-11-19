//! Owner: lane W5-EQUIP. Commands: `--inventory-move`: the port of its branch in `src/cli.cpp` and of
//! `src/inventory_navigation.cpp`. AI-assisted (Claude Code).
//!
//! `<package> --inventory-move <delta> <start> <count> --cooked <dir>` runs the installed
//! `WillowGame.InventoryListPanelGFxObject.MoveDelta` on an item-only list of `count` entries and prints
//! `{"index":<n>,"steps":<n>,"error":<text or null>}`, exit 0 without an error and 1 with one. It is a menu adapter,
//! not a rule: no navigation algorithm is written here (the menu's rules are the installed script, architecture rule
//! R2). The source package loads first, as the C++ loads it before it reads the mode.
//!
//! **What the adapter checks** (each the C++ check, with its text):
//! - `MoveDelta` is a script function of three int parameters `Delta`, `StartIndex`, `OriginalIndex` returning an
//!   int; `InventoryDataProviderGFxObject.GetEntryKindAtIndex` is a native of one int parameter.
//! - The getter's return property is a byte typed by an `Enum` export whose entries hold `EAK_Source` once; its
//!   position is the entry kind every list entry answers (an item-only list). The enum is read from its export the
//!   way the C++ reads it (`[net index][None][next][count][names]`, a count of 1 to 256, ending at the export's end),
//!   not through the reflection's enum table, so a bad enum gives the C++ text.
//! - The panel's `DataProvider` is an object property and the provider's `CachedObjects` an array; the getter is bound
//!   to answer only the provider and only an index inside the list.
//! - A request is a delta of -1 or 1, a count of 1 to 2048 and a start inside the list; the script may emit no log
//!   line and must return an index inside the list. The VM's step limit is 20,000, as the C++ sets.
//!
//! **Where it differs from the C++** (decision 2026-11-15-33):
//! - The getter's enum is read before the getter's signature is checked. The Rust reflection types a function's
//!   parameters with the enum, so a bad enum leaves the getter declared without parameters, and the signature check
//!   would name the wrong cause. A package that breaks both reports the enum here and the signature in the C++.
//! - The C++ decodes `MoveDelta`'s bytecode when the adapter opens; the VM decodes it at the first call, so a bytecode
//!   that does not decode is the move's `error` here and a failure to open there.
//! - The stub policy is `Refuse` with nothing allowed (architecture section 3.8, the gameplay commands): a native the
//!   script reaches that nothing binds stops the move with the VM's `UnboundNative` text, where the C++ logs a stub
//!   line, returns a zero and reports the log line as a diagnostic. Both make the move fail.
//!
//! **A Rust-only form**, `<package> --inventory-move --script-diff <weapons file> --cooked <dir> --output <file>`, is
//! the lane's differential test of the two script overrides of `ow_inventory::equip` (the installed script against the
//! Rust rule; [`script_diff`]). It is not a C++ command; `args.rs` lists only the C++ form in the usage text.
//!
//! **A second Rust-only form**, `<package> --inventory-move --equip-run <weapons file> --cooked <dir> --output <file>`,
//! runs the installed spawn, equip and unequip script of each weapon on a gameplay world and lists where each phase
//! stops (lane FIX-EQUIP-NATIVES; `equip_run`).
//!
//! `mode` is the run's parity mode, read once by `main`; it goes to the definition store and the VM.

use std::cell::Cell;
use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{FuncId, ObjRef};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::reflect::{CPF_NET, PROPERTY_HEADER};
use ow_object::value::Value;
use ow_package::package::Package;
use ow_package::reader::Reader;
use ow_package::tables::read_name;
use ow_vm::api::{NativeFn, Vm};
use ow_vm::error::VmError;
use ow_vm::interp::{instantiate_from_defaults, on_vm_stack};

use crate::args::{Command, USAGE};
use crate::cmd::properties::error_text;
use crate::cmd::tables::signed_number;
use crate::json::quote;

pub(crate) mod equip_run;

/// The script function the adapter runs.
pub const MOVE_DELTA: &str = "WillowGame.InventoryListPanelGFxObject.MoveDelta";
/// The native the script asks for each entry's kind.
pub const ENTRY_GETTER: &str = "WillowGame.InventoryDataProviderGFxObject.GetEntryKindAtIndex";
/// The panel and provider classes.
pub const PANEL_CLASS: &str = "WillowGame.InventoryListPanelGFxObject";
pub const PROVIDER_CLASS: &str = "WillowGame.InventoryDataProviderGFxObject";
/// The entry kind an item answers.
pub const SOURCE_ENTRY: &str = "EAK_Source";
/// The VM step limit of one move (`runtime.stepLimit = 20000`).
pub const STEP_LIMIT: u64 = 20_000;
/// The longest list a move takes.
pub const MAX_ENTRIES: i32 = 2048;
/// The C++ bounds on the entry enum's count.
pub const MAX_ENUM_ENTRIES: i32 = 256;

/// The property flag of a return value (`isReturn`).
const RETURN: u64 = PropertyDecl::RETURN;

pub fn run(_command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let args = args.to_vec();
    let outcome = on_vm_stack(move || {
        let mut out = String::new();
        let result = execute(&args, mode, &mut out);
        (out, result)
    });
    let (out, result) = match outcome {
        Ok(pair) => pair,
        Err(e) => (String::new(), Err(e.to_string())),
    };
    // Ignore a failed write (a closed pipe), as the C++ binary does.
    let _ = std::io::stdout().lock().write_all(out.as_bytes());
    match result {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("ow-package: {message}");
            ExitCode::from(1)
        }
    }
}

/// The command; `Ok` holds the exit code of a move that printed its line, `Err` the C++ `what()` of a failure that
/// printed none.
fn execute(args: &[String], mode: ParityMode, out: &mut String) -> Result<u8, String> {
    let source = args.first().ok_or_else(|| USAGE.to_string())?;
    Package::load(Path::new(source)).map_err(|e| error_text(&e))?;
    if args.iter().any(|a| a == "--script-diff") {
        return script_diff(args, mode, out);
    }
    if args.iter().any(|a| a == "--equip-run") {
        return equip_run::equip_run(args, mode, out);
    }
    if args.len() != 7 || args[5] != "--cooked" {
        return Err(USAGE.to_string());
    }
    // The C++ makes the adapter before it reads the three numbers.
    let mut navigation = Navigation::open(Path::new(&args[6]), mode)?;
    let delta = signed_number(&args[2])?;
    let start = signed_number(&args[3])?;
    let count = signed_number(&args[4])?;
    let moved = navigation.move_delta(delta, start, count);
    out.push_str(&format!(
        "{{\"index\":{},\"steps\":{},\"error\":{}}}\n",
        moved.index,
        moved.steps,
        moved.error.as_deref().map_or_else(|| "null".to_string(), quote)
    ));
    Ok(if moved.error.is_none() { 0 } else { 1 })
}

/// The result of one move (C++ `InventoryNavigation::Result`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// The index `MoveDelta` returned; -1 when the move failed.
    pub index: i32,
    /// The VM steps the move used (0 when no script ran).
    pub steps: u64,
    pub error: Option<String>,
}

/// The adapter over a VM of the cooked folder (C++ `InventoryNavigation::Impl`).
pub struct Navigation {
    vm: Vm,
    movement: FuncId,
    panel: ObjRef,
    provider: ObjRef,
    /// The length of the list of the move in progress, which the bound getter checks.
    count: Rc<Cell<i32>>,
}

/// A script or VM error as the C++ `what()` reads it.
fn vm_text(e: &VmError) -> String {
    match e {
        VmError::Script { message, .. } => message.clone(),
        other => error_text(other),
    }
}

fn kind_is_int(d: &PropertyDecl) -> bool {
    matches!(d.kind, PropKind::Int)
}

impl Navigation {
    /// Opens the adapter on the cooked folder `cooked`: every check of the module documentation that does not need a
    /// request.
    pub fn open(cooked: &Path, mode: ParityMode) -> Result<Navigation, String> {
        let defs = DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, mode)
            .map_err(|e| error_text(&e))?;
        let mut vm = Vm::new(
            ObjectSpace::new(),
            defs,
            Rc::new(AttributeSystem::new_empty()),
            mode,
        );
        ow_vm::natives_core::register(&mut vm).map_err(|e| vm_text(&e))?;
        vm.limits.step_limit = STEP_LIMIT;
        let movement = vm.find_function(MOVE_DELTA).map_err(|e| vm_text(&e))?;
        {
            let decl = vm
                .function(movement)
                .ok_or_else(|| "unsupported inventory MoveDelta signature".to_string())?;
            let result_int = decl.result.as_ref().is_some_and(kind_is_int);
            if decl.is_native() || decl.params.len() != 3 || !result_int {
                return Err("unsupported inventory MoveDelta signature".into());
            }
            for (param, name) in decl
                .params
                .iter()
                .zip(["Delta", "StartIndex", "OriginalIndex"])
            {
                if vm.space.names.text(param.name) != name || !kind_is_int(param) {
                    return Err("unsupported inventory MoveDelta parameter".into());
                }
            }
        }
        let getter = vm.find_function(ENTRY_GETTER).map_err(|e| vm_text(&e))?;
        // The enum first (module documentation, "Where it differs").
        let source = source_entry_kind(&mut vm, getter)?;
        let key = {
            let decl = vm
                .function(getter)
                .ok_or_else(|| "unsupported inventory entry getter signature".to_string())?;
            let one_int = decl.params.len() == 1 && kind_is_int(&decl.params[0]);
            match (&decl.native_key, one_int) {
                (Some(key), true) => key.clone(),
                _ => return Err("unsupported inventory entry getter signature".into()),
            }
        };
        let panel_class = vm.find_class(PANEL_CLASS).map_err(|e| vm_text(&e))?;
        let panel = instantiate_from_defaults(&mut vm, panel_class).map_err(|e| vm_text(&e))?;
        let provider_class = vm.find_class(PROVIDER_CLASS).map_err(|e| vm_text(&e))?;
        let provider =
            instantiate_from_defaults(&mut vm, provider_class).map_err(|e| vm_text(&e))?;
        let unsupported = || "unsupported inventory provider properties".to_string();
        let data = vm.space.prop(panel_class, "DataProvider").ok_or_else(unsupported)?;
        let cached = vm
            .space
            .prop(provider_class, "CachedObjects")
            .ok_or_else(unsupported)?;
        let data_is_object = matches!(
            vm.space.read(panel, ow_object::ids::PropAddr::scalar(data)),
            Ok(Value::Object(_))
        );
        let cached_is_array = matches!(
            vm.space.read(provider, ow_object::ids::PropAddr::scalar(cached)),
            Ok(Value::Array(_))
        );
        if !data_is_object || !cached_is_array {
            return Err(unsupported());
        }
        vm.space
            .write(
                panel,
                ow_object::ids::PropAddr::scalar(data),
                Value::Object(Some(provider)),
            )
            .map_err(|e| error_text(&e))?;
        let count = Rc::new(Cell::new(0));
        let bound_count = Rc::clone(&count);
        let entry: NativeFn = Rc::new(move |call| {
            let index = match call.arg(0) {
                Ok(Value::Int(i)) => Some(*i),
                _ => None,
            };
            let inside = index.is_some_and(|i| i >= 0 && i < bound_count.get());
            if call.this != Some(provider) || call.arg_count() != 1 || !inside {
                return Err(VmError::Script {
                    function: ENTRY_GETTER.to_string(),
                    message: "inventory entry getter outside supplied item list".to_string(),
                });
            }
            Ok(Value::Byte(source))
        });
        vm.register_native(&key, entry)
            .map_err(|e| vm_text(&e))?;
        Ok(Navigation {
            vm,
            movement,
            panel,
            provider,
            count,
        })
    }

    /// One move (C++ `InventoryNavigation::move`): `MoveDelta(delta, start, start)` on a list of `count` items.
    pub fn move_delta(&mut self, delta: i32, start: i32, count: i32) -> Moved {
        self.vm.log.clear();
        let mut ran = false;
        let outcome = self.try_move(delta, start, count, &mut ran);
        Moved {
            index: *outcome.as_ref().unwrap_or(&-1),
            steps: if ran { self.vm.steps() } else { 0 },
            error: outcome.err(),
        }
    }

    fn try_move(&mut self, delta: i32, start: i32, count: i32, ran: &mut bool) -> Result<i32, String> {
        if (delta != -1 && delta != 1) || count <= 0 || count > MAX_ENTRIES || start < 0 || start >= count {
            return Err("invalid inventory movement request".into());
        }
        self.count.set(count);
        let provider_class = self.vm.space.heap.get(self.provider).map_err(|e| error_text(&e))?.class;
        let cached = self
            .vm
            .space
            .prop(provider_class, "CachedObjects")
            .ok_or_else(|| "unsupported inventory provider properties".to_string())?;
        // `count` is 1..=2048 here.
        let entries = vec![Value::Object(None); usize::try_from(count).unwrap_or(0)];
        self.vm
            .space
            .write(self.provider, ow_object::ids::PropAddr::scalar(cached), Value::array(entries))
            .map_err(|e| error_text(&e))?;
        *ran = true;
        let result = self
            .vm
            .call(
                self.movement,
                Some(self.panel),
                vec![Value::Int(delta), Value::Int(start), Value::Int(start)],
            )
            .map_err(|e| vm_text(&e))?;
        if let Some(first) = self.vm.log.first() {
            return Err(format!("inventory movement emitted a diagnostic: {first}"));
        }
        match result.ret {
            Value::Int(i) if i >= 0 && i < count => Ok(i),
            _ => Err("inventory movement returned an invalid index".into()),
        }
    }
}

/// A property export's flags and first type reference, read as the C++ `Runtime::readProperty` reads them: flags at
/// +20 (low word) and +24 (high word), the type reference after the 40-byte header and, with flag `0x20`, a 2-byte
/// replication offset. `None` for an export smaller than the header ("not a property export"), a type reference
/// past the export's end reads as 0.
fn raw_property(package: &Package, index: i32) -> Result<(u64, i32), String> {
    let object = package.object(index).map_err(|e| error_text(&e))?;
    let (Ok(at), Ok(size)) = (usize::try_from(object.offset), usize::try_from(object.size)) else {
        return Err("not a property export".into());
    };
    if size < PROPERTY_HEADER {
        return Err("not a property export".into());
    }
    let mut r = Reader::new(&package.data);
    let word = |r: &mut Reader<'_>, pos: usize| -> Result<u32, String> {
        r.set_pos(pos);
        r.u32().map_err(|e| error_text(&e))
    };
    let flags = u64::from(word(&mut r, at + 20)?) | (u64::from(word(&mut r, at + 24)?) << 32);
    let type_at = at + PROPERTY_HEADER + if flags & CPF_NET != 0 { 2 } else { 0 };
    let type_ref = if type_at + 4 <= at + size {
        i32::from_le_bytes(word(&mut r, type_at)?.to_le_bytes())
    } else {
        0
    };
    Ok((flags, type_ref))
}

/// The position of [`SOURCE_ENTRY`] in the getter's return enum (C++ `sourceEntryKind`).
fn source_entry_kind(vm: &mut Vm, getter: FuncId) -> Result<u8, String> {
    let path = vm
        .function(getter)
        .map(|d| d.path.as_str().to_string())
        .unwrap_or_default();
    let (package_name, function_path) = path.split_once('.').unwrap_or((path.as_str(), ""));
    let (code, store) = vm.defs.code_mut();
    let package = code.package(store, package_name).map_err(|e| error_text(&e))?;
    let function = code.find_export(store, package, function_path);
    let shared = store.shared(package);
    // The return property among the function's child properties (the C++ `result`).
    let mut result = None;
    for (at, export) in shared.exports.iter().enumerate() {
        let index = i32::try_from(at + 1).map_err(|_| "too many exports".to_string())?;
        if function == 0 || export.outer != function {
            continue;
        }
        if !code.class_name(store, package, index).ends_with("Property") {
            continue;
        }
        let (flags, type_ref) = raw_property(&shared, index)?;
        if flags & RETURN != 0 {
            result = Some((index, type_ref));
        }
    }
    let no_enum = || "inventory entry getter has no enum return".to_string();
    let (result_index, type_ref) = result.ok_or_else(no_enum)?;
    if code.class_name(store, package, result_index) != "ByteProperty" {
        return Err(no_enum());
    }
    let unresolved = || "inventory entry enum is unresolved".to_string();
    let (enum_package, enum_index) = code
        .resolve_ref(store, package, type_ref)
        .ok_or_else(unresolved)?;
    if code.class_name(store, enum_package, enum_index) != "Enum" {
        return Err(unresolved());
    }
    let holder = store.shared(enum_package);
    let object = holder.object(enum_index).map_err(|e| error_text(&e))?;
    let (Ok(start), Ok(size)) = (usize::try_from(object.offset), usize::try_from(object.size)) else {
        return Err("unsupported inventory entry enum".into());
    };
    let read = |e: ow_package::reader::ReadError| error_text(&e);
    let mut r = Reader::new(&holder.data);
    r.set_pos(start);
    r.require(size).map_err(read)?;
    r.set_limit(start + size);
    r.i32().map_err(read)?;
    if read_name(&mut r, &holder.names).map_err(read)? != "None" {
        return Err("unexpected enum property prefix".into());
    }
    let imports = i32::try_from(holder.imports.len()).map_err(|_| "too many imports".to_string())?;
    let exports = i32::try_from(holder.exports.len()).map_err(|_| "too many exports".to_string())?;
    r.reference(imports, exports).map_err(read)?;
    let count = r.i32().map_err(read)?;
    if count <= 0 || count > MAX_ENUM_ENTRIES {
        return Err("invalid inventory entry enum count".into());
    }
    // count is 1..=256: the product fits.
    r.require(count as usize * 8).map_err(read)?;
    let mut source: Option<u8> = None;
    for i in 0..count {
        if read_name(&mut r, &holder.names).map_err(read)? == SOURCE_ENTRY {
            if source.is_some() {
                return Err("duplicate source entry enum".into());
            }
            // i is below 256.
            source = Some(u8::try_from(i).map_err(|_| "unsupported inventory entry enum".to_string())?);
        }
    }
    match source {
        Some(s) if r.pos() == r.limit() => Ok(s),
        _ => Err("unsupported inventory entry enum".into()),
    }
}

// ------------------------------------------------------------------------------------------------ --script-diff

/// The differential test of the two script overrides (Rust-only, lane W5-EQUIP's done-when; decision
/// 2026-11-15-33): `<package> --inventory-move --script-diff <weapons file> --cooked <dir> --output <file>`.
///
/// Each line of the weapons file is `<weapon type> <level> <balance> [<Slot>=<part>]...` (slots `Body`, `Grip`,
/// `Barrel`, `Sight`, `Stock`, `Elemental`, `Accessory1`, `Accessory2`, `Material`, `Prefix`, `Title`). Two VMs over
/// the cooked folder, each a W5-WORLD world with the standard attribute system: on one the installed script runs,
/// on the other `ow_inventory::manager::Pack` binds `CalculateWeaponBaseValues` and `ApplyAttributeEffects` to
/// their Rust rules. For each weapon both make a `WillowWeapon` from its class defaults with that definition data
/// and level, call `CalculateWeaponBaseValues`, then `ApplyAttributeEffects` with the weapon as the source once per
/// holder (the type, then each part in definition-data order) on the holder's `WeaponAttributeEffects`, each call
/// given the previous call's records as its `OutModifiers` (so all but the first append to a non-empty array). Compared:
/// each call's error, every numeric and array property of the weapon (floats bit for bit, a modifier by its `Type`
/// and `Value`, any other object by its path), and each call's records. One line per weapon goes to `--output`, a
/// summary to stdout; exit 0 when every weapon is identical.
///
/// Both VMs bind the three `AttributeDefinitionBase` natives the installed `ApplyAttributeEffects` script calls
/// (`ResolveContext`, `AddAttributeModifier`, `RemoveAttributeModifier`) to `ow-attr`: the script side here, the
/// Rust side through its pack in Off (`ow_inventory::equip::natives`, lane FIX-EQUIP-NATIVES) and here in Reference,
/// where the pack binds none. Both are the same `ow-attr` calls.
fn script_diff(args: &[String], mode: ParityMode, out: &mut String) -> Result<u8, String> {
    let mut list = None;
    let mut cooked = None;
    let mut output = None;
    let mut i = 2;
    while i < args.len() {
        let value = args.get(i + 1).cloned().ok_or_else(|| USAGE.to_string());
        match args[i].as_str() {
            "--script-diff" => list = Some(value?),
            "--cooked" => cooked = Some(value?),
            "--output" => output = Some(value?),
            _ => return Err(USAGE.to_string()),
        }
        i += 2;
    }
    let (Some(list), Some(cooked), Some(output)) = (list, cooked, output) else {
        return Err(USAGE.to_string());
    };
    let text = std::fs::read_to_string(&list).map_err(|e| format!("{list}: {e}"))?;
    let mut script = diff_world(Path::new(&cooked), mode, false)?;
    let mut rust = diff_world(Path::new(&cooked), mode, true)?;
    let (mut weapons, mut identical, mut fields, mut records) = (0usize, 0usize, 0usize, 0usize);
    let mut lines = String::new();
    let mut first_difference: Option<String> = None;
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        weapons += 1;
        let request = diff_request(line).map_err(|e| format!("line {}: {e}", n + 1))?;
        let a = run_weapon(&mut script.vm, &request);
        let b = run_weapon(&mut rust.vm, &request);
        let (same, compared, why) = compare_runs(&a, &b);
        fields += compared;
        records += a.records;
        if same {
            identical += 1;
        } else if first_difference.is_none() {
            first_difference = Some(format!("line {}: {why}", n + 1));
        }
        lines.push_str(&format!(
            "{{\"line\":{},\"weapon_type\":{},\"fields\":{},\"identical\":{},\"script_error\":{},\"rust_error\":{},\"difference\":{},\"script_steps\":{},\"rust_steps\":{},\"script_records\":{},\"rust_records\":{},\"script_sample\":{},\"rust_sample\":{}}}\n",
            n + 1,
            quote(&request.weapon_type),
            compared,
            same,
            a.error.as_deref().map_or_else(|| "null".into(), quote),
            b.error.as_deref().map_or_else(|| "null".into(), quote),
            if same { "null".to_string() } else { quote(&why) },
            a.steps,
            b.steps,
            a.records,
            b.records,
            sample(&a),
            sample(&b),
        ));
    }
    std::fs::write(&output, lines).map_err(|e| format!("{output}: {e}"))?;
    out.push_str(&format!(
        "{{\"weapons\":{weapons},\"identical\":{identical},\"fields_compared\":{fields},\"records\":{records},\"first_difference\":{}}}\n",
        first_difference.as_deref().map_or_else(|| "null".into(), quote)
    ));
    Ok(if identical == weapons { 0 } else { 1 })
}

/// One weapon of the list.
struct DiffRequest {
    weapon_type: String,
    level: i32,
    balance: String,
    parts: Vec<(String, String)>,
}

fn diff_request(line: &str) -> Result<DiffRequest, String> {
    let mut words = line.split_whitespace();
    let weapon_type = words.next().ok_or("no weapon type")?.to_string();
    let level = words
        .next()
        .ok_or("no level")?
        .parse::<i32>()
        .map_err(|e| format!("the level: {e}"))?;
    let balance = words.next().ok_or("no balance")?.to_string();
    let mut parts = Vec::new();
    for w in words {
        let (slot, part) = w
            .split_once('=')
            .ok_or_else(|| format!("{w}: not <Slot>=<part>"))?;
        let field = format!("{slot}PartDefinition");
        let known = matches!(slot, "Prefix" | "Title")
            || ow_inventory::equip::WEAPON_PART_FIELDS.contains(&field.as_str());
        if !known {
            return Err(format!("{slot}: not a weapon slot"));
        }
        parts.push((field, part.to_string()));
    }
    Ok(DiffRequest {
        weapon_type,
        level,
        balance,
        parts,
    })
}

/// A world for one side of the comparison ([`script_diff`]).
fn diff_world(
    cooked: &Path,
    mode: ParityMode,
    rust: bool,
) -> Result<ow_world::world::World, String> {
    use ow_balance::types::{
        AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
        Playthrough,
    };
    let defs = DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, mode)
        .map_err(|e| error_text(&e))?;
    let attrs = AttributeSystem::standard().map_err(|e| error_text(&e))?;
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode);
    ow_vm::natives_core::register(&mut vm).map_err(|e| vm_text(&e))?;
    // In Off the Rust side's pack binds the same three natives itself (`ow_inventory::equip::natives`).
    if !(rust && mode == ParityMode::Off) {
        bind_attribute_natives(&mut vm).map_err(|e| vm_text(&e))?;
    }
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::new(0).map_err(|e| error_text(&e))?,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .map_err(|e| error_text(&e))?;
    let pack = ow_inventory::manager::Pack;
    let packs: Vec<&dyn ow_vm::api::NativePack> = if rust { vec![&pack] } else { Vec::new() };
    ow_world::world::World::new(vm, ow_world::world::BalanceState::new(snapshot), &packs)
        .map_err(|e| error_text(&e))
}

fn native_object(v: &Value) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) => Ok(*o),
        other => Err(VmError::Script {
            function: "AttributeDefinitionBase".into(),
            message: format!("expected an object, found {}", other.kind_name()),
        }),
    }
}

fn native_this(call: &ow_vm::api::NativeCall<'_>) -> Result<ObjRef, VmError> {
    call.this.ok_or_else(|| VmError::Script {
        function: "AttributeDefinitionBase".into(),
        message: "called without an attribute definition".into(),
    })
}

fn native_suppress(call: &ow_vm::api::NativeCall<'_>) -> Result<bool, VmError> {
    Ok(call.has(2) && matches!(call.arg(2)?, Value::Bool(true)))
}

/// The three `AttributeDefinitionBase` natives the installed `ApplyAttributeEffects` script calls, bound to
/// `ow-attr` (harness only; [`script_diff`]).
fn bind_attribute_natives(vm: &mut Vm) -> Result<(), VmError> {
    let resolve: NativeFn = Rc::new(|call| {
        let attr = native_this(call)?;
        let source = native_object(call.arg(0)?)?;
        let attrs = call.vm.attrs.clone();
        let context = attrs.resolve_context(attr, source, &mut ow_attr::api::EvalCx::new(call.vm))?;
        Ok(Value::Object(context))
    });
    vm.register_native("AttributeDefinitionBase.ResolveContext(object)", resolve)?;
    let add: NativeFn = Rc::new(|call| {
        let attr = native_this(call)?;
        let context = native_object(call.arg(0)?)?;
        let Some(modifier) = native_object(call.arg(1)?)? else {
            return Ok(Value::Bool(false));
        };
        let s = native_suppress(call)?;
        let attrs = call.vm.attrs.clone();
        let r = attrs.add_modifier(attr, context, modifier, s, &mut ow_attr::api::EvalCx::new(call.vm))?;
        Ok(Value::Bool(r.succeeded()))
    });
    vm.register_native("AttributeDefinitionBase.AddAttributeModifier(object,object,bool)", add)?;
    let remove: NativeFn = Rc::new(|call| {
        let attr = native_this(call)?;
        let context = native_object(call.arg(0)?)?;
        let Some(modifier) = native_object(call.arg(1)?)? else {
            return Ok(Value::Bool(false));
        };
        let s = native_suppress(call)?;
        let attrs = call.vm.attrs.clone();
        let r = attrs.remove_modifier(attr, context, modifier, s, &mut ow_attr::api::EvalCx::new(call.vm))?;
        Ok(Value::Bool(r.succeeded()))
    });
    vm.register_native("AttributeDefinitionBase.RemoveAttributeModifier(object,object,bool)", remove)
}

/// What one side saw for one weapon: the values compared, in a fixed order, and the first error.
struct WeaponRun {
    values: Vec<(String, String)>,
    error: Option<String>,
    /// The VM steps `CalculateWeaponBaseValues` used (bytecode on the script side, 0 for the override): shown, not
    /// compared.
    steps: u64,
    /// The records the last `ApplyAttributeEffects` call returned: every call's, as each appends to the previous
    /// call's array (compared through the values).
    records: usize,
}

/// The values shown on each output line (evidence that the comparison is not empty), when present.
const SAMPLED: [&str; 6] = [
    "InstantHitDamage",
    "InstantHitDamageBaseValue",
    "ClipSize",
    "FireRate",
    "Spread",
    "ApplyAttributeEffects[0]",
];

fn sample(run: &WeaponRun) -> String {
    let shown: Vec<String> = run
        .values
        .iter()
        .filter(|(n, _)| SAMPLED.contains(&n.as_str()))
        .map(|(n, v)| format!("{}:{}", quote(n), quote(v)))
        .collect();
    format!("{{{}}}", shown.join(","))
}

/// A value as compared: floats by their bits, a modifier by its type and value, another object by its path.
fn canonical(vm: &Vm, v: &Value) -> String {
    match v {
        Value::Float(x) => format!("f{:08x}", x.to_bits()),
        Value::Int(i) => format!("i{i}"),
        Value::Byte(b) => format!("y{b}"),
        Value::Bool(b) => format!("b{b}"),
        Value::Object(None) => "None".into(),
        Value::Object(Some(o)) => {
            let class = vm.space.heap.get(*o).map(|x| x.class).ok();
            let is_modifier = class
                .and_then(|c| vm.space.classes.get_layout(c))
                .is_some_and(|l| l.path().as_str() == ow_attr::storage::ATTRIBUTE_MODIFIER_CLASS);
            if is_modifier {
                let shown = |name: &str| match ow_inventory::equip::read_prop(vm, *o, name) {
                    Some(x) => canonical(vm, &x),
                    None => "?".into(),
                };
                format!("modifier({},{})", shown("Type"), shown("Value"))
            } else {
                ow_inventory::spawn::object_path(vm, *o).unwrap_or_else(|e| format!("<{e}>"))
            }
        }
        Value::Array(items) => {
            let shown: Vec<String> = items.iter().map(|x| canonical(vm, x)).collect();
            format!("[{}]", shown.join(","))
        }
        Value::Struct(s) => {
            let shown: Vec<String> = s.fields.iter().map(|x| canonical(vm, x)).collect();
            format!("{{{}}}", shown.join(","))
        }
        Value::Name(n) => format!("n{}", vm.space.names.text(*n)),
        other => other.kind_name().to_string(),
    }
}

fn run_weapon(vm: &mut Vm, r: &DiffRequest) -> WeaponRun {
    let mut run = WeaponRun {
        values: Vec::new(),
        error: None,
        steps: 0,
        records: 0,
    };
    if let Err(e) = run_weapon_steps(vm, r, &mut run) {
        run.error = Some(e);
    }
    run
}

fn set_field(vm: &Vm, data: &mut Value, name: &str, v: Value) -> Result<(), String> {
    let Value::Struct(s) = data else {
        return Err("DefinitionData is not a struct".into());
    };
    let layout = vm
        .space
        .classes
        .struct_layout(s.ty)
        .ok_or("the definition data struct has no layout")?;
    let id = vm
        .space
        .names
        .find(name)
        .ok_or_else(|| format!("no field {name}"))?;
    let (at, _) = layout.field(id).ok_or_else(|| format!("no field {name}"))?;
    let at = usize::try_from(at).map_err(|_| "a field index past usize".to_string())?;
    let fields = &mut Rc::make_mut(s).fields;
    *fields.get_mut(at).ok_or("a field past the struct")? = v;
    Ok(())
}

fn find_object(vm: &mut Vm, path: &str) -> Result<ObjRef, String> {
    let p = ow_core::ObjectPath::parse(path).map_err(|e| format!("{path}: {e}"))?;
    vm.defs
        .find(&mut vm.space, &p)
        .map_err(|e| error_text(&e))?
        .ok_or_else(|| format!("{path}: no package holds it"))
}

fn run_weapon_steps(vm: &mut Vm, r: &DiffRequest, run: &mut WeaponRun) -> Result<(), String> {
    use ow_inventory::equip::{APPLY_ATTRIBUTE_EFFECTS, CALCULATE_WEAPON_BASE_VALUES, read_prop};
    use ow_object::ids::PropAddr;
    let class = vm
        .find_class("WillowGame.WillowWeapon")
        .map_err(|e| vm_text(&e))?;
    let weapon = instantiate_from_defaults(vm, class).map_err(|e| vm_text(&e))?;
    let weapon_type = find_object(vm, &r.weapon_type)?;
    let balance = find_object(vm, &r.balance)?;
    let mut data = read_prop(vm, weapon, "DefinitionData").ok_or("the weapon has no DefinitionData")?;
    set_field(vm, &mut data, "WeaponTypeDefinition", Value::Object(Some(weapon_type)))?;
    set_field(vm, &mut data, "BalanceDefinition", Value::Object(Some(balance)))?;
    set_field(vm, &mut data, "GameStage", Value::Int(r.level))?;
    for (field, part) in &r.parts {
        let o = find_object(vm, part)?;
        set_field(vm, &mut data, field, Value::Object(Some(o)))?;
    }
    let mut holders = vec![weapon_type];
    for field in ow_inventory::equip::WEAPON_PART_FIELDS {
        if let Some((_, part)) = r.parts.iter().find(|(f, _)| f == field) {
            holders.push(find_object(vm, part)?);
        }
    }
    let write = |vm: &mut Vm, name: &str, v: Value| -> Result<(), String> {
        let p = vm
            .space
            .prop(class, name)
            .ok_or_else(|| format!("WillowWeapon declares no {name}"))?;
        vm.space
            .write(weapon, PropAddr::scalar(p), v)
            .map_err(|e| error_text(&e))
    };
    write(vm, "DefinitionData", data)?;
    // The level as W4-STATS' instantiate_stats writes it (ExpLevel is a plain int attribute).
    write(vm, "ExpLevel", Value::Int(r.level))?;
    let bases = vm
        .find_function(CALCULATE_WEAPON_BASE_VALUES)
        .map_err(|e| vm_text(&e))?;
    let called = vm.call(bases, Some(weapon), Vec::new());
    run.steps = vm.steps();
    let shown = match &called {
        Ok(_) => "ok".to_string(),
        Err(e) => format!("error {}", vm_text(e)),
    };
    run.values.push(("CalculateWeaponBaseValues".into(), shown));
    called.map_err(|e| format!("CalculateWeaponBaseValues: {}", vm_text(&e)))?;
    let apply = vm
        .find_function(APPLY_ATTRIBUTE_EFFECTS)
        .map_err(|e| vm_text(&e))?;
    // Each call gets the previous call's records as its OutModifiers, so every call after the first appends to a
    // non-empty array (the script appends and never empties it).
    let mut existing = Value::array(Vec::new());
    for (k, holder) in holders.iter().enumerate() {
        let effects = read_prop(vm, *holder, "WeaponAttributeEffects")
            .unwrap_or_else(|| Value::array(Vec::new()));
        let called = vm.call(
            apply,
            None,
            vec![
                Value::Object(Some(weapon)),
                effects,
                existing.clone(),
                Value::Object(None),
            ],
        );
        if let Ok(result) = &called
            && let Some(out) = result.outs.get(2)
        {
            if let Value::Array(items) = out {
                run.records = items.len();
            }
            existing = out.clone();
        }
        let shown = match &called {
            Ok(result) => result
                .outs
                .get(2)
                .map_or_else(|| "no out".into(), |v| canonical(vm, v)),
            Err(e) => format!("error {}", vm_text(e)),
        };
        run.values.push((format!("ApplyAttributeEffects[{k}]"), shown));
        called.map_err(|e| format!("ApplyAttributeEffects: {}", vm_text(&e)))?;
    }
    let props: Vec<(ow_object::ids::PropId, String)> = vm
        .space
        .classes
        .props(class)
        .map_err(|e| error_text(&e))?
        .into_iter()
        .map(|(id, d)| (id, vm.space.names.text(d.name).to_string()))
        .collect();
    for (id, name) in props {
        let v = vm
            .space
            .read(weapon, PropAddr::scalar(id))
            .map_err(|e| error_text(&e))?
            .clone();
        let compared = matches!(
            v,
            Value::Float(_) | Value::Int(_) | Value::Byte(_) | Value::Bool(_) | Value::Array(_)
        );
        if compared {
            run.values.push((name, canonical(vm, &v)));
        }
    }
    Ok(())
}

/// Whether the two runs agree, the number of values compared, and the first difference.
fn compare_runs(a: &WeaponRun, b: &WeaponRun) -> (bool, usize, String) {
    if a.error != b.error {
        return (
            false,
            0,
            format!("errors differ: script {:?}, rust {:?}", a.error, b.error),
        );
    }
    if a.values.len() != b.values.len() {
        return (
            false,
            0,
            format!("{} values against {}", a.values.len(), b.values.len()),
        );
    }
    for ((na, va), (nb, vb)) in a.values.iter().zip(&b.values) {
        if na != nb || va != vb {
            return (false, a.values.len(), format!("{na}: script {va}, rust {vb}"));
        }
    }
    (a.error.is_none(), a.values.len(), String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_weapons_line_names_its_type_level_balance_and_slots() {
        let r = diff_request("T.Type 8 T.Balance Barrel=T.B Prefix=T.P Material=T.M").unwrap();
        assert_eq!((r.weapon_type.as_str(), r.level, r.balance.as_str()), ("T.Type", 8, "T.Balance"));
        let fields: Vec<&str> = r.parts.iter().map(|(f, _)| f.as_str()).collect();
        assert_eq!(fields, ["BarrelPartDefinition", "PrefixPartDefinition", "MaterialPartDefinition"]);
        assert!(diff_request("T.Type x T.Balance").is_err());
        assert!(diff_request("T.Type 8 T.Balance Wheel=T.W").is_err());
        assert!(diff_request("T.Type 8 T.Balance Barrel").is_err());
        assert!(diff_request("T.Type 8").is_err());
    }

    fn run(values: &[(&str, &str)], error: Option<&str>) -> WeaponRun {
        WeaponRun {
            values: values.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
            error: error.map(str::to_string),
            steps: 0,
            records: 0,
        }
    }

    #[test]
    fn runs_agree_only_on_equal_values_and_no_error() {
        let a = run(&[("X", "f3f800000"), ("Y", "i2")], None);
        assert_eq!(compare_runs(&a, &a), (true, 2, String::new()));
        let b = run(&[("X", "f3f800001"), ("Y", "i2")], None);
        let (same, n, why) = compare_runs(&a, &b);
        assert!(!same && n == 2 && why.starts_with("X:"), "{why}");
        let c = run(&[("X", "f3f800000")], None);
        assert!(!compare_runs(&a, &c).0);
        let e = run(&[], Some("boom"));
        assert!(!compare_runs(&e, &e).0, "an error on both sides is not a pass");
        assert!(!compare_runs(&a, &e).0);
    }

    #[test]
    fn the_enum_bounds_are_the_c_plus_plus_ones() {
        assert_eq!((MAX_ENUM_ENTRIES, MAX_ENTRIES, STEP_LIMIT), (256, 2048, 20_000));
    }
}
