//! Owner: lane IMPL-COMBAT-MUSIC-AND-AFTER (decision 2026-11-26-65). The spawn's balance tail and the combat music
//! manager on the swing's path (links 3, 4, 13 and 14), in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_COMBAT_MUSIC_AND_AFTER.md sections 3 to 6 (#801 as landed)
//! alone; this lane did not read the executable. **Every rule is UNVERIFIED in the running game.** Reference binds
//! nothing. Each field is read with its declared kind, else `VmError::Script`; each object read must be resident, else
//! `NotRead`.
//!
//! - **`BaseBalanceDefinition.ApplyGradeCustomizations(Pawn, GradeIndex)`** (section 3): on a balance that is not an
//!   `AIPawnBalanceDefinition`, a logged engine error and false. On one: the pawn query (a None argument false; a pawn
//!   itself; anything else [`PAWN_QUERY`], by fit); then the balance's own playthrough index ([`playthrough_index`];
//!   `GradeIndex` is not used), clamped to the last `PlayThroughs` entry; out of range false; otherwise that entry's
//!   `OnSpawnCustomizations` run on the pawn with its `Instigator` through the port's `BehaviorBase.RunBehaviors` (by
//!   fit), and true. Everything before the behaviours is write-free and load-free; a refusal after the first
//!   behaviour's write carries the partial-writes prefix. An empty list runs nothing.
//! - **`WillowAIPawn.UpdatePlayThroughMaterial()`** (section 4): with a `Mesh`, an `AIPawnBalanceDefinition` in
//!   `BalanceDefinitionState.BalanceDefinition` and the index in range, that entry's `MeshMaterial`: None does nothing,
//!   set is [`MATERIAL_SLOTS`] (the slot range was not read). Any gate that fails does nothing. Write-free.
//! - **`WillowGameInfo.GetCombatMusicManager()`** (section 5): the field when set. When None: the preflight (the
//!   `WillowGame.CombatMusicManager` class in the class table, else [`MANAGER_CLASS`]; the world's globals, their
//!   globals definition and its `CombatMusicProperties` resident; the fields declared), then a manager from the class
//!   defaults with the game info as its outer, stored in `GlobalCombatMusicManager`, its `CombatMusicParams` := the
//!   definition's `CombatMusicProperties`. A refusal after the creation carries the partial-writes prefix.
//! - **`CombatMusicManager.PlayerDamagedByEnemy(Player, Enemy, Damage, DamageType)`** (section 6): nothing without
//!   `CombatMusicParams` or without an `Enemy`, or when the damage type's class default has `bCausedByWorld`; past the
//!   gates [`THREAT_MODEL`] (`NotRead`), or, under the Off-only [`ThreatModelNoOp`] harness setting (the hub's ruling on
//!   review of #801), a logged no-op ([`THREAT_LINE`]). The declarations the gates and the threat call read
//!   (`CombatMusicParams`, `bCausedByWorld`, `EnemyDamagesPlayerWeight`) are checked first.

use ow_core::{NotYet, ParityMode};
use ow_object::heap::Origin;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::hit::bind_fn;

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_COMBAT_MUSIC_AND_AFTER.md";
pub const PAWN_QUERY: &str = "ApplyGradeCustomizations: the actor's pawn query";
pub const MATERIAL_SLOTS: &str = "UpdatePlayThroughMaterial: the material slots set";
pub const MANAGER_CLASS: &str = "GetCombatMusicManager: the manager class";
pub const THREAT_MODEL: &str = "CombatMusicManager.PlayerDamagedByEnemy: the threat model";
pub const DAMAGE_DEFAULT: &str = "PlayerDamagedByEnemy: the damage type's class default, not built (building it loads)";
/// The engine error line a base balance logs (section 3).
pub const BASE_ERROR_LINE: &str =
    "BaseBalanceDefinition.ApplyGradeCustomizations: called on a base balance definition (the engine's error; false)";
/// The harness line of the threat model's no-op stand-in, once per call past the gates.
pub const THREAT_LINE: &str = "harness input: CombatMusicManager.PlayerDamagedByEnemy: the threat model is not \
     modelled (no-op; the hub's ruling on review of #801; UNVERIFIED)";
/// The set-up line of [`enable_threat_model_no_op`].
pub const THREAT_INPUT: &str = "harness input: the world turns on the combat music threat model's no-op stand-in (the \
     hub's ruling on review of #801; UNVERIFIED)";

pub const APPLY_GRADE: [&str; 2] =
    ["Engine.BaseBalanceDefinition.ApplyGradeCustomizations", "WillowGame.AIPawnBalanceDefinition.ApplyGradeCustomizations"];
pub const UPDATE_MATERIAL: &str = "WillowGame.WillowAIPawn.UpdatePlayThroughMaterial";
pub const COMBAT_MUSIC: &str = crate::bcyl_tail::COMBAT_MUSIC;
pub const PLAYER_DAMAGED: &str = "WillowGame.CombatMusicManager.PlayerDamagedByEnemy";
const MANAGER: &str = "WillowGame.CombatMusicManager";
const AI_BALANCE: &str = "WillowGame.AIPawnBalanceDefinition";

/// The hub's ruling on review of #801: the threat model past the gates is a logged no-op (a VM service; its presence
/// turns it on). Off only; the slice's combat set-up turns it on.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThreatModelNoOp;

/// Turns [`ThreatModelNoOp`] on, logging [`THREAT_INPUT`] (Off only, else a registration refusal; twice is a
/// registration refusal too).
pub fn enable_threat_model_no_op(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Err(VmError::Registration {
            key: "ThreatModelNoOp".to_string(),
            reason: "the threat model's no-op is an Off harness setting; Reference binds nothing",
        });
    }
    vm.add_service(ThreatModelNoOp)?;
    vm.log.push(THREAT_INPUT.to_string());
    Ok(())
}

fn bad(what: &str, message: impl Into<String>) -> VmError {
    VmError::Script { function: what.to_string(), message: message.into() }
}

fn path(vm: &Vm, o: ObjRef) -> String {
    vm.space.heap.get(o).ok().and_then(|x| vm.space.classes.get_layout(x.class)).map_or_else(String::new, |l| l.path().to_string())
}

fn read(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<(Value, PropKind), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(what, format!("{} declares no {name}", path(vm, o))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(what, format!("{name} undeclared")))?;
    Ok((vm.space.read(o, PropAddr::scalar(p))?.clone(), kind))
}

fn object(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    match read(vm, o, name, what)? {
        (Value::Object(x), PropKind::Object { .. }) => Ok(x),
        _ => Err(bad(what, format!("{name} is not declared as an object"))),
    }
}

fn array(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Vec<Value>, VmError> {
    match read(vm, o, name, what)? {
        (Value::Array(items), PropKind::Array(_)) => Ok((*items).clone()),
        _ => Err(bad(what, format!("{name} is not declared as an array"))),
    }
}

fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(format!("{}: an object the store has not loaded (a load is a write)", path(vm, o)), RECORD)
            .into());
    }
    Ok(())
}

fn is_a(vm: &Vm, o: ObjRef, wanted: &str) -> Result<bool, VmError> {
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    let mut steps = 0;
    while let Some(c) = cursor.filter(|_| steps < 256) {
        steps += 1;
        let Some(l) = vm.space.classes.get_layout(c) else { return Ok(false) };
        if l.path().as_str().eq_ignore_ascii_case(wanted) {
            return Ok(true);
        }
        cursor = l.super_class();
    }
    Ok(false)
}

/// Field `name` of the struct value `v`, with its declared kind.
fn member(vm: &Vm, v: &Value, name: &str, what: &str) -> Result<(Value, PropKind), VmError> {
    let s = v.as_struct().ok_or_else(|| bad(what, format!("a {name} holder is not a struct")))?;
    let id = vm.space.names.find(name).ok_or_else(|| bad(what, format!("the struct declares no {name}")))?;
    let (i, d) = vm
        .space
        .classes
        .struct_layout(s.ty)
        .and_then(|l| l.field(id).map(|(i, d)| (i, d.kind.clone())))
        .ok_or_else(|| bad(what, format!("the struct declares no {name}")))?;
    let v = s.fields.get(i as usize).cloned().ok_or_else(|| bad(what, format!("no {name} slot")))?;
    Ok((v, d))
}

fn member_object(vm: &Vm, v: &Value, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    match member(vm, v, name, what)? {
        (Value::Object(x), PropKind::Object { .. }) => Ok(x),
        _ => Err(bad(what, format!("{name} is not declared as an object"))),
    }
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without a receiver"))
}

fn arg_object(c: &NativeCall<'_>, i: usize, what: &str) -> Result<Option<ObjRef>, VmError> {
    if i >= c.arg_count() {
        return Ok(None);
    }
    match c.arg(i)? {
        Value::Object(o) | Value::Interface(o) => Ok(*o),
        Value::None => Ok(None),
        other => Err(bad(what, format!("argument {i} is {}, not an object", other.kind_name()))),
    }
}

fn marks(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
}

// --- The playthrough index ------------------------------------------------------------------------------------------

/// The balance's own playthrough index (section 3, step 2): the world info's `GRI` as a `WillowGameReplicationInfo`,
/// its `CurrentPlaythrough` (the port's `GetCurrPlaythrough` rule); a replication info that is not a Willow one (None
/// included, by fit: the cast fails) 0; no world info -1; a negative playthrough -1. Not yet clamped. Reads only.
pub fn playthrough_index(vm: &Vm) -> Result<i32, VmError> {
    const WHAT: &str = "AIPawnBalanceDefinition.ApplyGradeCustomizations";
    // No world (no actor list, or no world info): -1.
    let Some(actors) = vm.service::<ow_world::actors::Actors>() else { return Ok(-1) };
    let Some(info) = actors.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info() else {
        return Ok(-1);
    };
    resident(vm, info)?;
    let Some(gri) = object(vm, info, "GRI", WHAT)? else { return Ok(0) };
    resident(vm, gri)?;
    if !is_a(vm, gri, "WillowGame.WillowGameReplicationInfo")? {
        return Ok(0);
    }
    let playthrough = match read(vm, gri, "CurrentPlaythrough", WHAT)? {
        (Value::Int(i), PropKind::Int) => i,
        (Value::Byte(b), PropKind::Byte { .. }) => i32::from(b),
        _ => return Err(bad(WHAT, "CurrentPlaythrough is not declared as an int or a byte")),
    };
    Ok(if playthrough < 0 { -1 } else { playthrough })
}

/// `balance`'s `PlayThroughs` entry for [`playthrough_index`], clamped to the last entry; `None` out of range.
fn playthrough_entry(vm: &Vm, balance: ObjRef, what: &str) -> Result<Option<Value>, VmError> {
    let entries = array(vm, balance, "PlayThroughs", what)?;
    let index = playthrough_index(vm)?.min(entries.len() as i32 - 1);
    if index < 0 {
        return Ok(None);
    }
    Ok(entries.get(index as usize).cloned())
}

// --- ApplyGradeCustomizations ---------------------------------------------------------------------------------------

/// What `ApplyGradeCustomizations` will do, decided write-free and load-free (section 3).
pub enum GradePlan {
    /// The answer, with nothing to run (`base_error`: the base balance's log line is due).
    Answer { answer: bool, base_error: bool },
    /// Run `behaviors` on `pawn` with `instigator`, then answer true.
    Run { pawn: ObjRef, instigator: Option<ObjRef>, behaviors: Value },
}

/// [`GradePlan`] for `balance` and the `Pawn` argument (module documentation). Reads only.
pub fn grade_plan(vm: &Vm, balance: ObjRef, pawn: Option<ObjRef>) -> Result<GradePlan, VmError> {
    const WHAT: &str = "AIPawnBalanceDefinition.ApplyGradeCustomizations";
    resident(vm, balance)?;
    if !is_a(vm, balance, AI_BALANCE)? {
        return Ok(GradePlan::Answer { answer: false, base_error: true });
    }
    let Some(actor) = pawn else { return Ok(GradePlan::Answer { answer: false, base_error: false }) };
    resident(vm, actor)?;
    if !is_a(vm, actor, "Engine.Pawn")? {
        return Err(NotYet::not_read(PAWN_QUERY, RECORD).into());
    }
    let Some(entry) = playthrough_entry(vm, balance, WHAT)? else {
        return Ok(GradePlan::Answer { answer: false, base_error: false });
    };
    let (behaviors, kind) = member(vm, &entry, "OnSpawnCustomizations", WHAT)?;
    // An array of behaviour references (review r2 of #808, item 5): any other element kind is a data error.
    if !matches!(&kind, PropKind::Array(inner) if matches!(**inner, PropKind::Object { .. })) {
        return Err(bad(WHAT, format!("OnSpawnCustomizations is declared as {kind:?}, not an array of objects")));
    }
    let instigator = object(vm, actor, "Instigator", WHAT)?;
    if behaviors.elements().is_none_or(|e| e.is_empty()) {
        return Ok(GradePlan::Answer { answer: true, base_error: false });
    }
    Ok(GradePlan::Run { pawn: actor, instigator, behaviors })
}

fn apply_grade(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "BaseBalanceDefinition.ApplyGradeCustomizations";
    let balance = this(c, WHAT)?;
    let pawn = arg_object(c, 0, WHAT)?;
    match grade_plan(c.vm, balance, pawn)? {
        GradePlan::Answer { answer, base_error } => {
            if base_error {
                c.vm.log.push(BASE_ERROR_LINE.to_string());
            }
            Ok(Value::Bool(answer))
        }
        GradePlan::Run { pawn, instigator, behaviors } => {
            // The shared runner with its write-aware policy: a refusal is prefixed only after a counted write
            // (review of #808, item 1).
            ow_behavior::kernel::run::run_behaviors_counted(c.vm, &behaviors, Some(pawn), instigator)?;
            Ok(Value::Bool(true))
        }
    }
}

// --- UpdatePlayThroughMaterial --------------------------------------------------------------------------------------

/// `UpdatePlayThroughMaterial()` (module documentation): `Ok` for nothing, else [`MATERIAL_SLOTS`]. Reads only.
pub fn update_material(vm: &Vm, pawn: ObjRef) -> Result<(), VmError> {
    const WHAT: &str = "WillowAIPawn.UpdatePlayThroughMaterial";
    resident(vm, pawn)?;
    // Every field the gates read, declared first.
    let mesh = object(vm, pawn, "Mesh", WHAT)?;
    let (state, kind) = read(vm, pawn, "BalanceDefinitionState", WHAT)?;
    if !matches!(kind, PropKind::Struct(_)) {
        return Err(bad(WHAT, "BalanceDefinitionState is not declared as a struct"));
    }
    let balance = member_object(vm, &state, "BalanceDefinition", WHAT)?;
    let (Some(_), Some(balance)) = (mesh, balance) else { return Ok(()) };
    resident(vm, balance)?;
    if !is_a(vm, balance, AI_BALANCE)? {
        return Ok(());
    }
    let Some(entry) = playthrough_entry(vm, balance, WHAT)? else { return Ok(()) };
    match member_object(vm, &entry, "MeshMaterial", WHAT)? {
        None => Ok(()),
        Some(_) => Err(NotYet::not_read(MATERIAL_SLOTS, RECORD).into()),
    }
}

// --- GetCombatMusicManager ------------------------------------------------------------------------------------------

/// What the lazy creation needs, settled write-free and load-free (section 5): the manager class and the params.
fn manager_plan(vm: &mut Vm, game: ObjRef) -> Result<(ClassId, Option<ObjRef>), VmError> {
    const WHAT: &str = "WillowGameInfo.GetCombatMusicManager";
    // A class id the table holds only as a reserved reference (its layout not loaded) is the same NotRead (review r2
    // of #808, item 4).
    let Some(class) =
        vm.space.classes.by_path(MANAGER).filter(|&c| vm.space.classes.get_layout(c).is_some_and(|l| l.is_defined()))
    else {
        return Err(NotYet::not_read(MANAGER_CLASS, RECORD).into());
    };
    object(vm, game, "GlobalCombatMusicManager", WHAT)?;
    // Both destinations' declared reference types accept what is written into them, checked before anything is made
    // (review of #808, item 2): the game info's field takes the manager, the manager's field the params.
    accepts(vm, vm.space.heap.get(game)?.class, "GlobalCombatMusicManager", class, WHAT)?;
    let globals = ow_inventory::globals::world_globals(vm)?;
    resident(vm, globals)?;
    // The two definition classes from the resident class table only (review of #808, item 4): a class the table does
    // not hold is a typed NotRead, so no class is loaded into the reflection cache.
    let table = |path: &str| -> Result<ClassId, VmError> {
        vm.space.classes.by_path(path).filter(|&c| vm.space.classes.get_layout(c).is_some_and(|l| l.is_defined())).ok_or_else(|| {
            NotYet::not_read(format!("{GLOBALS_CLASS_COLD}: {path}"), RECORD).into()
        })
    };
    let gearbox = table(ow_inventory::globals::GEARBOX_GLOBALS_DEFINITION)?;
    let globals_class = table(ow_inventory::globals::GLOBALS_DEFINITION_CLASS)?;
    let definition = ow_inventory::globals::globals_definition_of(vm, globals, gearbox, globals_class, WHAT)?;
    resident(vm, definition)?;
    let params = object(vm, definition, "CombatMusicProperties", WHAT)?;
    match params {
        Some(params) => {
            resident(vm, params)?;
            accepts(vm, class, "CombatMusicParams", vm.space.heap.get(params)?.class, WHAT)?;
        }
        None => {
            declared_object(vm, class, "CombatMusicParams", WHAT)?;
        }
    }
    Ok((class, params))
}

/// The harness line of [`preload_manager_inputs`]' loads.
pub fn manager_input_line(path: &str) -> String {
    format!("harness input: preload {path} (the combat music manager's input, which the game keeps resident; UNVERIFIED)")
}

/// The combat world's set-up for the manager's lazy creation (review r2 of #808, item 1): the globals definition
/// (`TheGlobalsDefinition`, which the caller sets first when the route left it unset) and its `CombatMusicProperties`,
/// each loaded when it is a handle the store has not loaded, and logged ([`manager_input_line`]); a failed load is
/// logged and left, so the creation's own refusal shows it. Off only; the number loaded.
pub fn preload_manager_inputs(vm: &mut Vm) -> Result<usize, VmError> {
    const WHAT: &str = "the combat music manager's set-up";
    if vm.parity != ParityMode::Off {
        return Ok(0);
    }
    let globals = ow_inventory::globals::world_globals(vm)?;
    let Some(definition) = object(vm, globals, "TheGlobalsDefinition", WHAT)? else { return Ok(0) };
    let mut loaded = 0;
    let mut load = |vm: &mut Vm, o: ObjRef| -> bool {
        if !vm.defs.is_unloaded(&vm.space, o) {
            return !vm.defs.is_failed(&vm.space, o);
        }
        let path = vm.defs.handle_path(o).unwrap_or_else(|| path(vm, o));
        vm.log.push(manager_input_line(&path));
        let r = vm.defs.ensure_loaded(&mut vm.space, o);
        vm.log.extend(vm.defs.take_log());
        match r {
            Ok(()) => {
                loaded += 1;
                true
            }
            Err(e) => {
                vm.log.push(format!("harness input: preload {path}: failed ({e})"));
                false
            }
        }
    };
    if !load(vm, definition) {
        return Ok(loaded);
    }
    if let Some(params) = object(vm, definition, "CombatMusicProperties", WHAT)? {
        load(vm, params);
    }
    Ok(loaded)
}

/// The refusal for a definition class the resident class table does not hold.
pub const GLOBALS_CLASS_COLD: &str = "GetCombatMusicManager: a globals definition class not in the class table (a class \
     load is a write)";

/// `class`'s `name` declared as one object reference, else a data error.
fn declared_object(vm: &Vm, class: ClassId, name: &str, what: &str) -> Result<ClassId, VmError> {
    let path = vm.space.classes.get_layout(class).map_or_else(String::new, |l| l.path().to_string());
    let p = vm.space.prop(class, name).ok_or_else(|| bad(what, format!("{path} declares no {name}")))?;
    match vm.space.classes.decl(p).map(|d| (d.kind.clone(), d.array_dim)) {
        Some((PropKind::Object { class: c }, 1)) => Ok(c),
        other => Err(bad(what, format!("{name} is declared as {other:?}, not one object reference"))),
    }
}

/// `class`'s `name` declared as an object reference that accepts a `value_class` object, else a data error.
fn accepts(vm: &Vm, class: ClassId, name: &str, value_class: ClassId, what: &str) -> Result<(), VmError> {
    let declared = declared_object(vm, class, name, what)?;
    if !vm.space.classes.is_child_of(value_class, declared) {
        return Err(bad(what, format!("{name} is declared as a reference that does not accept the value written into it")));
    }
    Ok(())
}

/// `GetCombatMusicManager()` (module documentation).
pub fn combat_music_manager(vm: &mut Vm, game: ObjRef) -> Result<ObjRef, VmError> {
    const WHAT: &str = "WillowGameInfo.GetCombatMusicManager";
    resident(vm, game)?;
    if let Some(manager) = object(vm, game, "GlobalCombatMusicManager", WHAT)? {
        return Ok(manager);
    }
    let (class, params) = manager_plan(vm, game)?;
    let entry = marks(vm);
    let made = (|| -> Result<ObjRef, VmError> {
        // From the class defaults: the resident class default object's values when it is built, else the store
        // builds it (a load, in the commit).
        let manager = match built_default(vm, class) {
            Some(default) => {
                let slots = vm.space.heap.get(default)?.slots.clone();
                let m = vm.space.alloc(class, "CombatMusicManager", Some(game), Origin::Runtime)?;
                vm.space.heap.get_mut(m)?.slots = slots;
                m
            }
            None => ow_vm::interp::instantiate_from_defaults(vm, class)?,
        };
        vm.space.heap.get_mut(manager)?.outer = Some(game);
        let class_of_game = vm.space.heap.get(game)?.class;
        let field = vm.space.prop(class_of_game, "GlobalCombatMusicManager").ok_or_else(|| bad(WHAT, "no GlobalCombatMusicManager"))?;
        vm.space.write(game, PropAddr::scalar(field), Value::Object(Some(manager)))?;
        let p = vm.space.prop(class, "CombatMusicParams").ok_or_else(|| bad(WHAT, "no CombatMusicParams"))?;
        vm.space.write(manager, PropAddr::scalar(p), Value::Object(params))?;
        Ok(manager)
    })();
    match made {
        Ok(m) => Ok(m),
        Err(e) if marks(vm) != entry => Err(ow_world::pools::after_partial_writes(e)),
        Err(e) => Err(e),
    }
}

// --- PlayerDamagedByEnemy -------------------------------------------------------------------------------------------

/// The class default object of `class` when it is resident with all its slots (a heap read; nothing is built).
fn built_default(vm: &Vm, class: ClassId) -> Option<ObjRef> {
    let slot_count = vm.space.classes.get_layout(class).map_or(0, |l| l.slot_count() as usize);
    vm.space.heap.handles().into_iter().find(|&o| {
        vm.space.heap.get(o).is_ok_and(|x| x.class == class && matches!(x.origin, Origin::Default { .. }) && x.slots.len() >= slot_count)
    })
}

/// Whether `PlayerDamagedByEnemy` reaches the threat model (`true`), or a gate stops it (`false`). Reads only.
pub fn damage_gates(vm: &Vm, manager: ObjRef, enemy: Option<ObjRef>, damage_type: Option<ClassId>) -> Result<bool, VmError> {
    const WHAT: &str = "CombatMusicManager.PlayerDamagedByEnemy";
    resident(vm, manager)?;
    // The two no-op gates first (review of #808, item 3): no params or no enemy answers before the damage default.
    let Some(params) = object(vm, manager, "CombatMusicParams", WHAT)? else { return Ok(false) };
    if enemy.is_none() {
        return Ok(false);
    }
    let caused_by_world = match damage_type {
        None => false,
        Some(class) => {
            let p = vm.space.prop(class, "bCausedByWorld").ok_or_else(|| bad(WHAT, "the damage type declares no bCausedByWorld"))?;
            if !matches!(vm.space.classes.decl(p).map(|d| &d.kind), Some(PropKind::Bool)) {
                return Err(bad(WHAT, "bCausedByWorld is not declared as a bool"));
            }
            let Some(default) = built_default(vm, class) else {
                return Err(NotYet::not_read(DAMAGE_DEFAULT, RECORD).into());
            };
            matches!(vm.space.read(default, PropAddr::scalar(p))?, Value::Bool(true))
        }
    };
    if caused_by_world {
        return Ok(false);
    }
    // The threat call's weight, only past the world-damage gate.
    resident(vm, params)?;
    match read(vm, params, "EnemyDamagesPlayerWeight", WHAT)? {
        (Value::Float(_), PropKind::Float) => {}
        _ => return Err(bad(WHAT, "EnemyDamagesPlayerWeight is not declared as a float")),
    }
    Ok(true)
}

fn player_damaged(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "CombatMusicManager.PlayerDamagedByEnemy";
    let manager = this(c, WHAT)?;
    let enemy = arg_object(c, 1, WHAT)?;
    let damage_type = if c.arg_count() > 3 {
        match c.arg(3)? {
            Value::Class(k) => *k,
            Value::Object(None) | Value::None => None,
            other => return Err(bad(WHAT, format!("DamageType holds {}, not a class", other.kind_name()))),
        }
    } else {
        None
    };
    player_damaged_answer(c.vm, manager, enemy, damage_type)?;
    Ok(Value::None)
}

/// `PlayerDamagedByEnemy` past its arguments (module documentation): the gates, then the no-op stand-in under its
/// setting (one log line), else [`THREAT_MODEL`]. Writes nothing.
pub fn player_damaged_answer(vm: &mut Vm, manager: ObjRef, enemy: Option<ObjRef>, damage_type: Option<ClassId>) -> Result<(), VmError> {
    if !damage_gates(vm, manager, enemy, damage_type)? {
        return Ok(());
    }
    if vm.service::<ThreatModelNoOp>().is_some() {
        vm.log.push(THREAT_LINE.to_string());
        return Ok(());
    }
    Err(NotYet::not_read(THREAT_MODEL, RECORD).into())
}

/// Binds the keys in Off (Reference binds nothing). Called before `bcyl_tail`'s, whose read-only
/// `GetCombatMusicManager` this lazy creation replaces.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    for key in APPLY_GRADE {
        bind_fn(vm, key, apply_grade)?;
    }
    bind_fn(vm, UPDATE_MATERIAL, |c| {
        let me = this(c, "WillowAIPawn.UpdatePlayThroughMaterial")?;
        update_material(c.vm, me)?;
        Ok(Value::None)
    })?;
    bind_fn(vm, COMBAT_MUSIC, |c| {
        let me = this(c, "WillowGameInfo.GetCombatMusicManager")?;
        Ok(Value::Object(Some(combat_music_manager(c.vm, me)?)))
    })?;
    bind_fn(vm, PLAYER_DAMAGED, player_damaged)?;
    Ok(())
}

#[cfg(test)]
#[path = "combat_music_tests.rs"]
mod tests;
