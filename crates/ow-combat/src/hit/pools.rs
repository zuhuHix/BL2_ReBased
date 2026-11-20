//! Four Off resource-pool rules from NATIVE_RESOURCE_POOL_IDLE.md, implemented from its own-words note only.
//! AI-assisted (Codex/GPT-6.1 Sol). All native/gameplay behavior remains UNVERIFIED in the running game.
//! Reference retains its existing bindings and stubs. No regeneration is implemented here.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, StructId};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use ow_object::layout::PropKind;

use super::access::{get, object, object_of, required, set_required};
use super::{bind_fn, this};

pub const RECORD: &str = "NATIVE_RESOURCE_POOL_IDLE.md";
/// The record of the class starting values (lane FIX-MIND-ADOPT, decision 2026-11-19-26).
pub const RECORD_STARTING: &str = "NATIVE_COMBAT_GROUNDWORK.md sections 1.10 and 1.11";
/// The player controller's native override of the starting values (decision 2026-11-19-46; its rule, decision
/// 2026-11-19-57).
pub const PLAYER_STARTING_VALUES: &str = "WillowGame.WillowPlayerController.ApplyCharacterClassStartingValues";
/// The note both natives are implemented from (lane FIX-CLASS-STARTING-VALUES, decision 2026-11-19-57; the analyst was
/// lane ANALYST-CLASS-STARTING-VALUES, decision 2026-11-19-50).
pub const RECORD_CLASS_STARTING: &str = "docs/verification/NATIVE_CLASS_STARTING_VALUES.md";

fn float(vm: &mut Vm, pool: ObjRef, name: &str) -> Result<f32, VmError> {
    match required(vm, pool, name, "resource pool")? {
        Value::Float(value) => Ok(value),
        _ => Err(NotYet::not_ported(format!("resource pool float {name}"), RECORD).into()),
    }
}

/// Stamp first, then the class's native hook. The note records four empty hook implementations and leaves the
/// shield implementation unread. It gives no rule for an additional subclass's native override: refuse after
/// the stamp, even if that subclass inherits ResourcePool's script functions. Script method inheritance cannot
/// prove native virtual-hook inheritance. Exact qualified identities prevent an unrelated same-named class
/// from being treated as one of the implementations the note read.
///
/// The identities are the installed packages' (lane LINK-1-NEXT-STOP, decision 2026-11-19-98): `ResourcePool` and
/// `HealthResourcePool` are declared in `Engine`; `AmmoResourcePool`, `ExperienceResourcePool` and `ShieldResourcePool`
/// in `WillowGame`. The note names the classes without their package. Before this, all five were matched under
/// `Engine`, so the installed `WillowGame.AmmoResourcePool` met the unrecorded-override refusal.
pub fn reset_idle_timer(vm: &mut Vm, pool: ObjRef) -> Result<(), VmError> {
    let time = vm.world_time_seconds()?;
    set_required(
        vm,
        pool,
        "PoolIdleDelayStartTime",
        Value::Float(time),
        "resource pool",
    )?;
    let class = vm.space.heap.get(pool)?.class;
    let path = vm
        .space
        .classes
        .get_layout(class)
        .map(|l| l.path().to_string())
        .unwrap_or_default();
    match path.to_ascii_lowercase().as_str() {
        "engine.resourcepool" | "engine.healthresourcepool" | "willowgame.ammoresourcepool"
        | "willowgame.experienceresourcepool" => Ok(()),
        "willowgame.shieldresourcepool" => Err(NotYet::not_read(
            "ShieldResourcePool idle-reset hook: provider chain and final function (stamp already written)", RECORD,
        ).into()),
        _ => Err(NotYet::not_read(
            format!("{path} native idle-reset hook/override (stamp already written)"), RECORD,
        ).into()),
    }
}

/// Strict stored-float delay comparison; only a passed delay clears the stamp. No world read or regeneration.
pub fn has_idle_delay_passed(vm: &mut Vm, pool: ObjRef, current: f32) -> Result<bool, VmError> {
    let start = float(vm, pool, "PoolIdleDelayStartTime")?;
    let delay = float(vm, pool, "OnIdleRegenerationDelay")?;
    if start > 0.0 && current < start + delay {
        return Ok(false);
    }
    set_required(
        vm,
        pool,
        "PoolIdleDelayStartTime",
        Value::Float(0.0),
        "resource pool",
    )?;
    Ok(true)
}

pub(super) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    vm.add_service(ow_world::pools::ResetIdleTimerOff(reset_idle_timer))?;
    bind_fn(vm, "Engine.ResourcePoolManager.ShutDown", |c| {
        let manager = this(c, "ResourcePoolManager.ShutDown")?;
        // The existing body preserves authority, slot order, script dispatch and partial effects on error.
        ow_world::pools::delete_all_pools(c.vm, Some(manager))?;
        Ok(Value::None)
    })?;
    bind_fn(vm, "Engine.Controller.ApplyCharacterClassStartingValues", |c| {
        let controller = this(c, "Controller.ApplyCharacterClassStartingValues")?;
        let class = class_argument(c.arg(0)?, "Controller.ApplyCharacterClassStartingValues")?;
        apply_class_starting_values(c.vm, controller, class)?;
        Ok(Value::None)
    })?;
    // The player controller's own override is native too (its declaration carries no script; lane FIX-PLAYER-CLASS-B,
    // decision 2026-11-19-46), NATIVE_CLASS_STARTING_VALUES.md section 2 (decision 2026-11-19-57).
    bind_fn(vm, PLAYER_STARTING_VALUES, |c| {
        let controller = this(c, "WillowPlayerController.ApplyCharacterClassStartingValues")?;
        let class = class_argument(c.arg(0)?, "WillowPlayerController.ApplyCharacterClassStartingValues")?;
        apply_class_starting_values(c.vm, controller, class)?;
        Ok(Value::None)
    })?;
    // Two natives of the AI mind's class route (NATIVE_MIND_ROUTE.md sections 1 and 2; decision 2026-11-19-32).
    bind_fn(vm, "WillowGame.WillowPawn.SetDefaultPhysics", |c| {
        let pawn = this(c, "WillowPawn.SetDefaultPhysics")?;
        set_default_physics(c.vm, pawn)?;
        Ok(Value::None)
    })?;
    for path in ["WillowGame.WillowPawn.GetExpLevel", "WillowGame.WillowAIPawn.GetExpLevel"] {
        bind_fn(vm, path, |c| {
            let pawn = this(c, "WillowPawn.GetExpLevel")?;
            get_exp_level(c.vm, pawn)
        })?;
    }
    // IsStaggered on the two pawn classes that declare it (NATIVE_IS_STAGGERED.md; lane IMPL-IS-STAGGERED).
    for path in IS_STAGGERED {
        bind_fn(vm, path, |c| {
            let pawn = this(c, "WillowPawn.IsStaggered")?;
            Ok(Value::Bool(is_staggered(c.vm, pawn)?))
        })?;
    }
    // DropCombatPerch (NATIVE_DROP_COMBAT_PERCH.md; lane IMPL-DROPCOMBATPERCH).
    bind_fn(vm, DROP_COMBAT_PERCH, |c| {
        let pawn = this(c, "WillowAIPawn.DropCombatPerch")?;
        drop_combat_perch(c.vm, pawn)?;
        Ok(Value::None)
    })?;
    bind_fn(vm, "Engine.ResourcePool.GetTimeSeconds", |c| {
        this(c, "ResourcePool.GetTimeSeconds")?;
        Ok(Value::Float(c.vm.world_time_seconds()?))
    })?;
    bind_fn(vm, "Engine.ResourcePool.HasIdleDelayPassed", |c| {
        let pool = this(c, "ResourcePool.HasIdleDelayPassed")?;
        let Value::Float(current) = c.arg(0)? else {
            return Err(NotYet::not_ported("HasIdleDelayPassed float CurrentTime", RECORD).into());
        };
        let current = *current;
        Ok(Value::Bool(has_idle_delay_passed(c.vm, pool, current)?))
    })
}

/// The class argument of either native. One that is not an object reference is a script error, never "no class"
/// (which would delete every pool; review r1 of #412).
fn class_argument(v: &Value, unit: &str) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) => Ok(*o),
        other => Err(super::access::error(format!(
            "{unit}: the class argument is not an object reference ({})",
            other.kind_name()
        ))),
    }
}

/// `ApplyCharacterClassStartingValues(inCharacterClass)` on any controller, Off, from NATIVE_CLASS_STARTING_VALUES.md
/// alone (clean room; decision 2026-11-19-57). A `WillowPlayerController` (or a subclass) gets the override of section
/// 2, every other controller (AI minds included) the `Controller` base of section 1. Both natives' bindings come here,
/// so a call through the base on a player controller runs the override, as a native virtual does.
///
/// **The base (section 1):** every pool of the controller's `ResourcePoolManager` is deleted, always. Then, with a
/// class, one pool per entry of its `ResourcePools` in array order, then `AccuracyPoolDefinition` into `AccuracyPool`
/// and `OffHandAccuracyPoolDefinition` into `OffHandAccuracyPool`, each only when set. The pools are made by
/// `CreatePool` with the controller as owner (the lowest free slot); a manager that call spawns is written back to
/// `ResourcePoolManager`. No role check, and nothing else changes.
///
/// **The override (section 2):** the base first, with the same class. Then, decided from the controller as it stands
/// after the base, only on the authority (`Role` is `ROLE_Authority`): with the controller's own `PlayerClass` (not the
/// argument), pools for its `ExperiencePoolDefinition`, `SkillCooldownPoolDefinition` and
/// `MeleeSkillCooldownPoolDefinition`, in that order and each only when set, stored as `ExpPool`, `SkillCooldownPool`
/// and `MeleeSkillCooldownPool`; then, with or without a `PlayerClass`, the controller's script event
/// `OnPlayerClassChange()`, run through the VM.
///
/// **References left as they were:** no stored pool reference is cleared. With no class, an unset definition, a
/// controller that is not the authority or no `PlayerClass`, the field keeps what it held. Its pool was deleted, so it
/// reads as not valid by the manager's GUID rule (open question 2 is not decided here).
///
/// **Refusals (the hub's ruling in review r1 of #444, the same rule as #437, #439 and #442).** Every refusal that can
/// be decided is decided before the first delete, on the controller as it is when the call starts:
/// - bad data: a class whose `ResourcePools` is not an array of objects, or whose accuracy definition fields are not
///   objects (all three must be declared); a `Role` that is not a byte; a `PlayerClass` or a player definition field
///   that is not an object; a reference field that is not a pool reference struct (all four fields, their kinds);
/// - `NotYet { NotRead }`: pools that name one resource twice (the base's and the override's together; `CreatePool`'s
///   reinitialise flag is not given), and any pool to make when the controller's manager is not the authority, or when
///   there is no manager and none would be spawned (the note's open question 3: what creation does on such a manager);
/// - `NotYet { NotPorted }`: when the event will run and resolves to the stock declaration, its final call's
///   `InitPlayerSkillTree` being a native nothing binds (NATIVE_SKILLS.md; an override is taken as it is); a planned
///   pool class's `InitializeAuthoritative`, or an existing pool's `ShutDown`, being a native nothing binds (a planned
///   pool class without `InitializeAuthoritative` is `NotYet { NotRead }`; a pool without `ShutDown` is skipped by the
///   pool record's rule); a controller without `OnPlayerClassChange` is bad data;
/// - bad data also: a `ResourcePools` declared with another element kind (even when empty), and a reference struct whose
///   `PoolManager` or `Data` cannot hold the manager or the pool class.
///
/// The preflight checks only what is cheap and certain before deleting: method lookups, binding checks, declared types
/// (the hub's ruling in review r2 of #444, as #439 r3). Before the override's pools, the manager's authority is checked
/// again.
///
/// After the first delete, a refusal (a pool's `ShutDown` or `InitializeAuthoritative`, the event's script, or a check
/// on the override decided again because the base changed `Role` or `PlayerClass`) is **terminal**: nothing is put
/// back, its text starts "stopped after partial writes", and its kind is kept.
///
/// Open question 1 (whether the argument and `PlayerClass` can differ) does not change the rule: each step reads what
/// the note says it reads. UNVERIFIED in the running game.
pub fn apply_class_starting_values(vm: &mut Vm, controller: ObjRef, class: Option<ObjRef>) -> Result<(), VmError> {
    let player = match vm.find_class("WillowGame.WillowPlayerController") {
        Ok(player) => {
            let own = vm.space.heap.get(controller)?.class;
            vm.space.classes.is_child_of(own, player)
        }
        Err(_) => false,
    };
    let unit = if player {
        "WillowPlayerController.ApplyCharacterClassStartingValues"
    } else {
        "Controller.ApplyCharacterClassStartingValues"
    };
    let base = base_plan(vm, class, unit)?;
    let (authority, extra) = if player { override_plan(vm, controller, unit)? } else { (false, Vec::new()) };
    let all: Vec<Step> = base.iter().chain(&extra).copied().collect();
    let types = check_plan(vm, controller, &all, unit)?;
    if player && authority {
        check_event(vm, controller, unit)?;
    }
    check_deletes(vm, controller)?;
    // The first delete: from here on a refusal is terminal.
    let committed = (|| -> Result<(), VmError> {
        let mut manager = object(vm, controller, "ResourcePoolManager")?;
        ow_world::pools::delete_all_pools(vm, manager)?;
        for (&(def, field), ty) in base.iter().zip(&types) {
            create(vm, controller, &mut manager, def, field, *ty, unit)?;
        }
        if !player {
            return Ok(());
        }
        // The override is decided from the controller after the base (the note's order), checked again when the base
        // changed what the plan was made from.
        let (authority_now, extra_now) = override_plan(vm, controller, unit)?;
        let extra_types = if (authority_now, &extra_now) == (authority, &extra) {
            types[base.len()..].to_vec()
        } else {
            let all: Vec<Step> = base.iter().chain(&extra_now).copied().collect();
            let types = check_plan(vm, controller, &all, unit)?;
            if authority_now {
                check_event(vm, controller, unit)?;
            }
            types[base.len()..].to_vec()
        };
        // The manager the override's pools go into, checked again after the base (review r2 of #444, item 4).
        if !extra_now.is_empty() {
            check_creation(vm, controller)?;
        }
        for (&(def, field), ty) in extra_now.iter().zip(&extra_types) {
            create(vm, controller, &mut manager, def, field, *ty, unit)?;
        }
        if authority_now {
            super::call_method(vm, controller, "OnPlayerClassChange", vec![])?;
        }
        Ok(())
    })();
    committed.map_err(ow_world::pools::after_partial_writes)
}

/// One pool to make: its definition and the controller field that stores its reference (none for `ResourcePools`).
type Step = (ObjRef, Option<&'static str>);

/// The base's pools from the class argument: its `ResourcePools` in order (None entries skipped: `CreatePool` makes
/// nothing for one), then the two accuracy definitions when set. The three fields must be declared with their kinds.
fn base_plan(vm: &mut Vm, class: Option<ObjRef>, unit: &str) -> Result<Vec<Step>, VmError> {
    let Some(class) = class else { return Ok(Vec::new()) };
    let class = super::access::loaded(vm, class)?;
    let mut plan = Vec::new();
    // The declared element kind, checked even for an empty array (review r2 of #444, item 5).
    let own = vm.space.heap.get(class)?.class;
    let declared = vm.space.prop(own, "ResourcePools").and_then(|p| vm.space.classes.decl(p)).map(|d| d.kind.clone());
    if !matches!(&declared, Some(PropKind::Array(inner)) if matches!(**inner, PropKind::Object { .. })) {
        return Err(super::access::error(format!(
            "{unit}: {} does not declare ResourcePools as an array of objects",
            super::access::class_name(vm, class)
        )));
    }
    match required(vm, class, "ResourcePools", unit)? {
        Value::Array(list) => {
            for v in list.iter() {
                match v {
                    Value::Object(Some(d)) => plan.push((*d, None)),
                    Value::Object(None) => {}
                    other => {
                        return Err(super::access::error(format!(
                            "{unit}: an entry of the class's ResourcePools holds {}, not an object",
                            other.kind_name()
                        )));
                    }
                }
            }
        }
        other => {
            return Err(super::access::error(format!(
                "{unit}: the class's ResourcePools holds {}, not an array",
                other.kind_name()
            )));
        }
    }
    for (def, field) in [("AccuracyPoolDefinition", "AccuracyPool"), ("OffHandAccuracyPoolDefinition", "OffHandAccuracyPool")] {
        if let Some(d) = object_field(vm, class, def, unit)? {
            plan.push((d, Some(field)));
        }
    }
    Ok(plan)
}

/// The override's part, from the controller as it is now: whether it is the authority, and the `PlayerClass` pools
/// it will make (none off the authority or without a `PlayerClass`).
fn override_plan(vm: &mut Vm, controller: ObjRef, unit: &str) -> Result<(bool, Vec<Step>), VmError> {
    let authority = super::with_state(vm, |s| s.authority)?;
    let authority = match required(vm, controller, "Role", unit)? {
        Value::Byte(r) => i64::from(r) == i64::from(authority),
        other => return Err(super::access::error(format!("{unit}: Role holds {}, not a byte", other.kind_name()))),
    };
    if !authority {
        return Ok((false, Vec::new()));
    }
    let mut plan = Vec::new();
    if let Some(pc) = object_field(vm, controller, "PlayerClass", unit)? {
        let pc = super::access::loaded(vm, pc)?;
        for (def, field) in [
            ("ExperiencePoolDefinition", "ExpPool"),
            ("SkillCooldownPoolDefinition", "SkillCooldownPool"),
            ("MeleeSkillCooldownPoolDefinition", "MeleeSkillCooldownPool"),
        ] {
            if let Some(d) = object_field(vm, pc, def, unit)? {
                plan.push((d, Some(field)));
            }
        }
    }
    Ok((true, plan))
}

/// A declared object field of `o`: its object, or None when unset; undeclared or of another kind is bad data.
fn object_field(vm: &mut Vm, o: ObjRef, name: &str, unit: &str) -> Result<Option<ObjRef>, VmError> {
    match required(vm, o, name, unit)? {
        Value::Object(v) => Ok(v),
        other => Err(super::access::error(format!(
            "{unit}: {} {name} holds {}, not an object",
            super::access::class_name(vm, o),
            other.kind_name()
        ))),
    }
}

/// The checks on a plan that change nothing (reviews r1 and r2 of #444):
/// - one resource twice is `NotYet { NotRead }`;
/// - every pool to make: its pool class's `InitializeAuthoritative` must exist (else `NotYet { NotRead }` naming it) and
///   must not be a native nothing binds (`NotYet { NotPorted }`);
/// - every reference field to be written must be a pool reference struct (its type is returned) whose `PoolManager`
///   can hold the manager the pools go into and whose `Data` can hold the pool class (else bad data);
/// - any pool to make needs an authoritative manager, or one that would be spawned ([`check_creation`]).
fn check_plan(vm: &mut Vm, controller: ObjRef, plan: &[Step], unit: &str) -> Result<Vec<Option<StructId>>, VmError> {
    let mut resources = Vec::new();
    let mut types = Vec::new();
    for &(def, field) in plan {
        let def = super::access::loaded(vm, def)?;
        let resource = object(vm, def, "Resource")?;
        if resource.is_some() && resources.contains(&resource) {
            return Err(NotYet::not_read(
                "ApplyCharacterClassStartingValues for pools that name one resource twice (CreatePool's reinitialise flag is not given)",
                RECORD_CLASS_STARTING,
            )
            .into());
        }
        resources.push(resource);
        let pool_class = ow_world::pools::pool_class_of(vm, def)?;
        if let Some(class) = pool_class {
            let Some(f) = vm.find_method(class, "InitializeAuthoritative", None)? else {
                return Err(NotYet::not_read(
                    format!(
                        "ApplyCharacterClassStartingValues: the pool class {} has no InitializeAuthoritative",
                        class_path(vm, class)
                    ),
                    RECORD_CLASS_STARTING,
                )
                .into());
            };
            unbound_native(vm, f, "a pool's InitializeAuthoritative")?;
        }
        types.push(match field {
            Some(field) => {
                let ty = reference_type(vm, controller, field, unit)?;
                check_reference_holds(vm, controller, ty, pool_class, field, unit)?;
                Some(ty)
            }
            None => None,
        });
    }
    if !plan.is_empty() {
        check_creation(vm, controller)?;
    }
    Ok(types)
}

/// Pools are made only on an authoritative manager, or one that would be spawned: else `NotYet { NotRead }`, what
/// creation does there being the note's open question 3. Checked before the first delete and again before the
/// override's pools (review r2 of #444, item 4).
fn check_creation(vm: &mut Vm, controller: ObjRef) -> Result<(), VmError> {
    let creates = match object(vm, controller, "ResourcePoolManager")? {
        Some(m) => ow_world::pools::is_authoritative(&vm.space, m)?,
        None => ow_world::pools::can_spawn_manager(vm, controller)?,
    };
    if creates {
        return Ok(());
    }
    Err(NotYet::not_read(
        "ApplyCharacterClassStartingValues making pools on a manager that is not the authority, or with no manager and \
         none spawned (NATIVE_CLASS_STARTING_VALUES.md open question 3)",
        RECORD_CLASS_STARTING,
    )
    .into())
}

/// The existing pools' `ShutDown`, which the delete runs (review r2 of #444, item 1): a class without one is skipped
/// by the pool record's rule; one that is a native nothing binds is `NotYet { NotPorted }`. Only an authoritative
/// manager's pools are deleted.
fn check_deletes(vm: &mut Vm, controller: ObjRef) -> Result<(), VmError> {
    let Some(m) = object(vm, controller, "ResourcePoolManager")? else { return Ok(()) };
    if !ow_world::pools::is_authoritative(&vm.space, m)? {
        return Ok(());
    }
    for (_, pool) in ow_world::pools::pools_of(&vm.space, m)? {
        let (class, state) = {
            let x = vm.space.heap.get(pool)?;
            (x.class, x.state)
        };
        if let Some(f) = vm.find_method(class, "ShutDown", state)? {
            unbound_native(vm, f, "an existing pool's ShutDown")?;
        }
    }
    Ok(())
}

/// `NotYet { NotPorted }` when `f` is a native nothing binds.
fn unbound_native(vm: &Vm, f: ow_object::ids::FuncId, what: &str) -> Result<(), VmError> {
    if let Some(key) = vm.function(f).and_then(|d| d.native_key.clone())
        && !vm.is_bound(f)
    {
        return Err(NotYet::not_ported(
            format!("ApplyCharacterClassStartingValues: {what} is the native {key}, which nothing binds"),
            RECORD_CLASS_STARTING,
        )
        .into());
    }
    Ok(())
}

fn class_path(vm: &Vm, class: ow_object::ids::ClassId) -> String {
    vm.space.classes.get_layout(class).map(|l| l.path().to_string()).unwrap_or_default()
}

/// The reference struct `ty` of `field` must be able to hold what the call writes (review r2 of #444, item 5): its
/// `PoolManager` the manager the pool goes into (the controller's, or the class a spawn makes), its `Data` the pool
/// class `pool_class` (when the definition makes one).
fn check_reference_holds(
    vm: &mut Vm,
    controller: ObjRef,
    ty: StructId,
    pool_class: Option<ow_object::ids::ClassId>,
    field: &str,
    unit: &str,
) -> Result<(), VmError> {
    let manager_class = match object(vm, controller, "ResourcePoolManager")? {
        Some(m) => vm.space.heap.get(m)?.class,
        None => vm.find_class(ow_world::pools::MANAGER_CLASS)?,
    };
    for (name, held) in [("PoolManager", Some(manager_class)), ("Data", pool_class)] {
        let Some(held) = held else { continue };
        let target = vm
            .space
            .names
            .find(name)
            .and_then(|n| vm.space.classes.struct_layout(ty)?.field(n).map(|(_, d)| d.kind.clone()));
        let fits = match target {
            Some(PropKind::Object { class }) => vm.space.classes.is_child_of(held, class),
            _ => false,
        };
        if !fits {
            return Err(super::access::error(format!(
                "{unit}: {field}.{name} cannot hold a {}",
                class_path(vm, held)
            )));
        }
    }
    Ok(())
}

/// The event, checked before it runs (review r2 of #444, item 3). `OnPlayerClassChange` is resolved virtually on the
/// controller's own class; none is bad data. When it resolves to the stock declaration on `WillowPlayerController`
/// (the event the note describes), its final call to `WillowPlayerController.InitPlayerSkillTree` is known: a native
/// nothing binds there is `NotYet { NotPorted }` (NATIVE_SKILLS.md). An override is taken as it is: no assumption is
/// made about what it calls, and a refusal inside it is terminal.
fn check_event(vm: &mut Vm, controller: ObjRef, unit: &str) -> Result<(), VmError> {
    let own = vm.space.heap.get(controller)?.class;
    let state = vm.space.heap.get(controller)?.state;
    let Some(event) = vm.find_method(own, "OnPlayerClassChange", state)? else {
        return Err(super::access::error(format!("{unit}: the controller has no OnPlayerClassChange")));
    };
    let stock = vm.find_function("WillowGame.WillowPlayerController.OnPlayerClassChange").ok();
    if stock == Some(event)
        && let Ok(tree) = vm.find_function("WillowGame.WillowPlayerController.InitPlayerSkillTree")
        && let Some(key) = vm.function(tree).and_then(|d| d.native_key.clone())
        && !vm.is_bound(tree)
    {
        return Err(NotYet::not_ported(
            format!(
                "ApplyCharacterClassStartingValues: the stock OnPlayerClassChange calls {key}, which nothing binds \
                 (NATIVE_SKILLS.md)"
            ),
            RECORD_CLASS_STARTING,
        )
        .into());
    }
    Ok(())
}

/// Makes one pool through `CreatePool`, writes the manager back, and stores the reference in `field` when there is one.
fn create(
    vm: &mut Vm,
    controller: ObjRef,
    manager: &mut Option<ObjRef>,
    def: ObjRef,
    field: Option<&'static str>,
    ty: Option<StructId>,
    unit: &str,
) -> Result<(), VmError> {
    let made = ow_world::pools::create_pool(vm, *manager, Some(controller), Some(def), false)?;
    *manager = made.manager;
    set_required(vm, controller, "ResourcePoolManager", Value::Object(*manager), unit)?;
    if let (Some(field), Some(ty)) = (field, ty) {
        let v = ow_world::pools::ref_value(&vm.space, ty, made.reference)?;
        set_required(vm, controller, field, v, unit)?;
    }
    Ok(())
}

/// The struct type of the controller's pool reference field `field`, checked to be a pool reference struct.
fn reference_type(vm: &Vm, controller: ObjRef, field: &str, unit: &str) -> Result<StructId, VmError> {
    let class_id = vm.space.heap.get(controller)?.class;
    let ty = vm
        .space
        .prop(class_id, field)
        .and_then(|p| vm.space.classes.decl(p))
        .and_then(|d| match d.kind {
            PropKind::Struct(t) => Some(t),
            _ => None,
        })
        .ok_or_else(|| super::access::error(format!("{unit}: the controller has no struct property {field}")))?;
    ow_world::pools::check_reference_type(&vm.space, ty)?;
    Ok(ty)
}

/// The record of the route natives (decision 2026-11-19-32).
pub const RECORD_ROUTE: &str = "NATIVE_MIND_ROUTE.md";

/// `WillowPawn.SetDefaultPhysics()` (NATIVE_MIND_ROUTE.md section 1), Off. The gates first. Nothing changes when:
/// - the pawn is gone (`bDeleteMe` or `bIsDead`);
/// - it has no `SMComponent`;
/// - that component's `CurrentSMData.SMD` is a `WillowGame.SpecialMove_Spawned` (a pawn in its spawn move keeps its
///   physics); with a special move set, a world whose classes lack `SpecialMove_Spawned` is an error.
///
/// The gate on the pawn's `ObjectArchetype` being a `Pawn` always passes for a pawn here: its archetype is its template
/// or its class default object, both pawns (UNVERIFIED; the port does not model `ObjectArchetype`).
///
/// Past the gates the native sets the default physics, copies `bCollideWorld` and, unless two pawn queries both answer
/// true, the collision flags from the archetype. Those two queries are not named (the note's "Not read"), and an AI
/// pawn's default physics also hangs on an unnamed navigation-handle query in its last case. No branch is picked: this
/// is `NotYet { NotRead }`, decided before anything changes. UNVERIFIED in the running game.
pub fn set_default_physics(vm: &mut Vm, pawn: ObjRef) -> Result<(), VmError> {
    let flag = |vm: &mut Vm, name: &str| -> Result<bool, VmError> {
        Ok(matches!(get(vm, pawn, name)?, Some(Value::Bool(true))))
    };
    if flag(vm, "bDeleteMe")? || flag(vm, "bIsDead")? {
        return Ok(());
    }
    let Some(component) = object(vm, pawn, "SMComponent")? else {
        return Ok(());
    };
    let current = get(vm, component, "CurrentSMData")?;
    let smd = current
        .as_ref()
        .and_then(|v| ow_combat_field(vm, v, "SMD"))
        .and_then(|v| object_of(&v));
    if let Some(smd) = smd {
        // `SpecialMove_Spawned` is a `WillowGame` class (research/native_by_class.json). A failed lookup is an error,
        // never a gate that silently passes (review r1 of #418).
        let spawned = vm.find_class("WillowGame.SpecialMove_Spawned")?;
        let own = vm.space.heap.get(smd)?.class;
        if vm.space.classes.is_child_of(own, spawned) {
            return Ok(());
        }
    }
    Err(NotYet::not_read(
        "WillowPawn.SetDefaultPhysics past its gates: the two pawn queries that decide whether the collision flags are \
         copied, and the navigation-handle query of an AI pawn's default physics, are not named (NATIVE_MIND_ROUTE.md \
         section 1, not read)",
        RECORD_ROUTE,
    )
    .into())
}

/// A field of a struct value by name.
fn ow_combat_field(vm: &Vm, v: &Value, name: &str) -> Option<Value> {
    let s = v.as_struct()?;
    let id = vm.space.names.find(name)?;
    let (index, _) = vm.space.classes.struct_layout(s.ty)?.field(id)?;
    s.fields.get(usize::try_from(index).ok()?).cloned()
}

/// `GetExpLevel()` on a pawn (NATIVE_MIND_ROUTE.md section 2), Off: on a `WillowAIPawn`, the pawn's own `IBalancedActor`
/// answer, its `ExpLevel` int as stored. Another pawn's `IBalancedActor` answer is not read: `NotYet { NotRead }`.
/// UNVERIFIED in the running game.
pub fn get_exp_level(vm: &mut Vm, pawn: ObjRef) -> Result<Value, VmError> {
    let ai = vm.find_class("WillowGame.WillowAIPawn")?;
    let own = vm.space.heap.get(pawn)?.class;
    if !vm.space.classes.is_child_of(own, ai) {
        return Err(NotYet::not_read(
            "GetExpLevel on a pawn that is not a WillowAIPawn (its IBalancedActor answer is not read)",
            RECORD_ROUTE,
        )
        .into());
    }
    match get(vm, pawn, "ExpLevel")? {
        Some(v @ Value::Int(_)) => Ok(v),
        _ => Err(super::access::error("WillowAIPawn.GetExpLevel: the pawn has no int ExpLevel")),
    }
}

/// The record of [`is_staggered`].
pub const RECORD_STAGGER: &str = "docs/verification/NATIVE_IS_STAGGERED.md";
/// The two declarations bound to [`is_staggered`].
pub const IS_STAGGERED: [&str; 2] = ["WillowGame.WillowPawn.IsStaggered", "WillowGame.WillowAIPawn.IsStaggered"];

/// The native (C++) descendants of `WillowAIPawn`, carried with the binding so the class check stays load-free: the
/// two children of `WillowAIPawn` in `WillowGame`'s export super chain (NATIVE_IS_STAGGERED.md section 3, step 4;
/// NATIVE_DROP_COMBAT_PERCH.md, D2). `research/native_by_class.json` holds function counts only, so it cannot decide
/// which class is native; the list is recorded instead.
pub const NATIVE_AI_PAWN_DESCENDANTS: [&str; 2] = ["WillowGame.WillowAICranePawn", "WillowGame.DeployableTurretActor"];
/// The struct type the AI rule reads.
pub const STAGGER_STATE_DATA: &str = "WillowGame.WillowAIPawn.StaggerStateData";

/// `IsStaggered()` on a pawn in Off (NATIVE_IS_STAGGERED.md section 3, the note as final at b8b22422; lane
/// IMPL-IS-STAGGERED, coded from that note alone, clean room). **UNVERIFIED in the running game.** It writes, loads and
/// draws nothing. [`is_staggered_with`] with [`NATIVE_AI_PAWN_DESCENDANTS`].
pub fn is_staggered(vm: &Vm, pawn: ObjRef) -> Result<bool, VmError> {
    is_staggered_with(vm, pawn, &NATIVE_AI_PAWN_DESCENDANTS)
}

/// [`is_staggered`] with `native` as the native descendants of `WillowAIPawn`. The preflight, in order, the first
/// failure the answer:
/// 1. the receiver's class is `WillowPawn` or a child (the class table's `by_path` and `is_child_of` only): otherwise a
///    `VmError::Script`;
/// 2. **S1**, `NotYet { NotRead }`: an unloaded handle, or a slotless stand-in the store does not load (two texts);
/// 3. the class:
///    - exactly `WillowAIPawn`, or a script-only child of it (no class between it and `WillowAIPawn`, itself included,
///      is in `native`): the AI rule;
///    - a native descendant of `WillowAIPawn`: **S2** (its slot was not read);
///    - exactly `WillowPawn`: false (its virtual's stub);
///    - any other child of `WillowPawn` (`WillowPlayerPawn`, a script-only child of `WillowPawn` too): **S2**;
/// 4. the AI rule reads by name, no offset: `StaggerState` declared as a struct of type `WillowAIPawn.StaggerStateData`
///    ([`STAGGER_STATE_DATA`]), its member `bIsStaggered` a bool. Otherwise a `VmError::Script` naming the field. The
///    answer is that bool, from the resident pawn's slot.
pub fn is_staggered_with(vm: &Vm, pawn: ObjRef, native: &[&str]) -> Result<bool, VmError> {
    let space = &vm.space;
    let path = ow_attr::eval::class_path(space, pawn);
    let obj = space.heap.get(pawn)?;
    let class = obj.class;
    let willow_pawn = space.classes.by_path("WillowGame.WillowPawn");
    if !willow_pawn.is_some_and(|p| space.classes.is_child_of(class, p)) {
        return Err(super::access::error(format!("WillowPawn.IsStaggered: {path} is not a WillowPawn")));
    }
    // S1 names the object, as ow-attr's eval.rs stand-in text does: its class path and its own name (review r1 of #619).
    let object = format!("{path} {}", space.names.text(obj.name));
    if vm.defs.is_unloaded(space, pawn) {
        return Err(NotYet::not_read(
            format!("WillowPawn.IsStaggered: the pawn {object} is an unloaded handle (a load is a write)"),
            RECORD_STAGGER,
        )
        .into());
    }
    // A stand-in with no properties: no slots although its class has fields (ow-attr's eval.rs condition).
    let fields = space.classes.get_layout(class).map_or(0, |l| l.slot_count());
    if obj.slots.is_empty() && fields > 0 {
        return Err(NotYet::not_read(
            format!(
                "WillowPawn.IsStaggered: the pawn {object} is a stand-in with no properties (an object the store \
                 cannot load)"
            ),
            RECORD_STAGGER,
        )
        .into());
    }
    let s2 = || -> VmError {
        NotYet::not_read(
            format!("WillowPawn.IsStaggered: the stagger rule of {path} (its virtual was not read)"),
            RECORD_STAGGER,
        )
        .into()
    };
    let ai = space.classes.by_path("WillowGame.WillowAIPawn");
    if let Some(ai) = ai.filter(|&a| space.classes.is_child_of(class, a)) {
        // A native class at or above the receiver, below WillowAIPawn, holds its own slot.
        let natives: Vec<_> = native.iter().filter_map(|p| space.classes.by_path(p)).collect();
        let mut cursor = Some(class);
        while let Some(c) = cursor.filter(|&c| c != ai) {
            if natives.contains(&c) {
                return Err(s2());
            }
            cursor = space.classes.get_layout(c).and_then(|l| l.super_class());
        }
        let bad = |why: &str| super::access::error(format!("WillowAIPawn.IsStaggered: {path}.StaggerState {why}"));
        let p = space.prop(class, "StaggerState").ok_or_else(|| bad("is not declared"))?;
        let Some(PropKind::Struct(sid)) = space.classes.decl(p).map(|d| &d.kind) else {
            return Err(bad("is not a struct"));
        };
        let typed = space.classes.struct_layout(*sid).is_some_and(|l| l.path().as_str().eq_ignore_ascii_case(STAGGER_STATE_DATA));
        if !typed {
            return Err(bad(&format!("is not a {STAGGER_STATE_DATA}")));
        }
        let state = space.read(pawn, ow_object::ids::PropAddr::scalar(p))?;
        return match ow_combat_field(vm, state, "bIsStaggered") {
            Some(Value::Bool(b)) => Ok(b),
            Some(other) => Err(bad(&format!("has a bIsStaggered of kind {}, not a bool", other.kind_name()))),
            None => Err(bad("has no bIsStaggered")),
        };
    }
    if willow_pawn == Some(class) {
        return Ok(false);
    }
    Err(s2())
}

/// The record of [`drop_combat_perch`].
pub const RECORD_PERCH: &str = "docs/verification/NATIVE_DROP_COMBAT_PERCH.md";
/// The declaration bound to [`drop_combat_perch`].
pub const DROP_COMBAT_PERCH: &str = "WillowGame.WillowAIPawn.DropCombatPerch";
/// D3's text: the drop itself, read only to its edge.
pub const PERCH_DROP: &str = "WillowAIPawn.DropCombatPerch: the perch projectile's spawn and launch (not read)";
/// `ROLE_Authority` as the raw byte the guard compares (the stock enum, UNVERIFIED).
pub const ROLE_AUTHORITY: u8 = 3;

/// `WillowAIPawn.DropCombatPerch()` in Off (NATIVE_DROP_COMBAT_PERCH.md section 3; lane IMPL-DROPCOMBATPERCH, coded from
/// that note alone, clean room). **UNVERIFIED in the running game.** [`drop_combat_perch_with`] with
/// [`NATIVE_AI_PAWN_DESCENDANTS`].
pub fn drop_combat_perch(vm: &Vm, pawn: ObjRef) -> Result<(), VmError> {
    drop_combat_perch_with(vm, pawn, &NATIVE_AI_PAWN_DESCENDANTS)
}

/// [`drop_combat_perch`] with `native` as the native descendants of `WillowAIPawn`. Write-free, load-free and RNG-free;
/// the first failure is the answer:
/// 1. the receiver's class is `WillowAIPawn` or a child (`by_path` and `is_child_of` only): otherwise a `VmError::Script`;
/// 2. **D1**, `NotYet { NotRead }`: an unloaded handle, or a slotless stand-in (two texts);
/// 3. **D2**, `NotYet { NotRead }`: a native descendant of `WillowAIPawn`, or a child of one (its slot was not read).
///    Script-only children take `WillowAIPawn`'s rule;
/// 4. the declarations, by name: `Role` a byte, `PerchProjectile` an object, `bPerchAttached` a bool. A missing field
///    or a wrong kind is a `VmError::Script` naming it;
/// 5. the guard, in order: `Role` not [`ROLE_AUTHORITY`], or `PerchProjectile` None, or `bPerchAttached` clear: the
///    answer, with nothing written;
/// 6. **D3** (the guard holds): `NotYet { NotRead }` ([`PERCH_DROP`]), before any write.
pub fn drop_combat_perch_with(vm: &Vm, pawn: ObjRef, native: &[&str]) -> Result<(), VmError> {
    const WHAT: &str = "WillowAIPawn.DropCombatPerch";
    let space = &vm.space;
    let path = ow_attr::eval::class_path(space, pawn);
    let obj = space.heap.get(pawn)?;
    let class = obj.class;
    let Some(ai) = space.classes.by_path("WillowGame.WillowAIPawn").filter(|&a| space.classes.is_child_of(class, a)) else {
        return Err(super::access::error(format!("{WHAT}: {path} is not a WillowAIPawn")));
    };
    if vm.defs.is_unloaded(space, pawn) {
        return Err(NotYet::not_read(format!("{WHAT}: the pawn {path} is an unloaded handle (a load is a write)"), RECORD_PERCH).into());
    }
    if u32::try_from(obj.slots.len()).ok() != space.classes.get_layout(class).map(|l| l.slot_count()) {
        return Err(NotYet::not_read(
            format!("{WHAT}: the pawn {path} is a stand-in with no properties (an object the store cannot load)"),
            RECORD_PERCH,
        )
        .into());
    }
    let natives: Vec<_> = native.iter().filter_map(|p| space.classes.by_path(p)).collect();
    let mut cursor = Some(class);
    while let Some(c) = cursor.filter(|&c| c != ai) {
        if natives.contains(&c) {
            return Err(NotYet::not_read(format!("{WHAT}: the perch rule of {path} (its virtual was not read)"), RECORD_PERCH).into());
        }
        cursor = space.classes.get_layout(c).and_then(|l| l.super_class());
    }
    let field = |name: &str, want: &str, ok: fn(&PropKind) -> bool| -> Result<Value, VmError> {
        let bad = |why: String| super::access::error(format!("{WHAT}: {path}.{name} {why}"));
        let p = space.prop(class, name).ok_or_else(|| bad("is not declared".to_string()))?;
        match space.classes.decl(p).map(|d| &d.kind) {
            Some(k) if ok(k) => Ok(space.read(pawn, ow_object::ids::PropAddr::scalar(p))?.clone()),
            _ => Err(bad(format!("is not {want}"))),
        }
    };
    let role = field("Role", "a byte", |k| matches!(k, PropKind::Byte { .. }))?;
    let projectile = field("PerchProjectile", "an object", |k| matches!(k, PropKind::Object { .. }))?;
    let attached = field("bPerchAttached", "a bool", |k| matches!(k, PropKind::Bool))?;
    let role = matches!(role, Value::Byte(ROLE_AUTHORITY));
    let projectile = matches!(projectile, Value::Object(Some(_)));
    let attached = matches!(attached, Value::Bool(true));
    if !role || !projectile || !attached {
        return Ok(());
    }
    Err(NotYet::not_read(PERCH_DROP, RECORD_PERCH).into())
}

#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "pools/stagger_tests.rs"]
mod stagger_tests;
#[cfg(test)]
#[path = "pools/perch_tests.rs"]
mod perch_tests;
