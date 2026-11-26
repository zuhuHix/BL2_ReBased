//! The body class's referenced definitions, preloaded at the `maya` step's set-up (lane IMPL-BODYCLASS-PRELOAD, queue
//! 0aj; the hub's ruling on NATIVE_MAYA9_AND_AFTER.md section 4.2). AI-assisted (Claude Code).
//!
//! In the game, the body class's referenced definitions (stances, behavior providers) stream in with the character.
//! The port's store loads lazily, so `InitializeBehaviorProviders`' preflight met `RunStance0` as an unloaded handle.
//! This harness input loads **exactly** the objects that preflight reads from the body class (`ow-behavior`
//! `kernel::natives::pawn_providers`, steps 6 (a) to (f)), and each one's `BehaviorProviderDefinition`, whose
//! `BehaviorSequences` the kernel's gate reads. Not a transitive closure. In order, each once:
//! 1. the body class itself (the archetype's `BodyClass`);
//! 2. (a) the `DefaultHitRegion`, only when the `HitRegionList` holds it; (b) each `HitRegionList` entry, then its
//!    `OverrideDeathDef`; (c) each `ScrapedStances` entry; (e) `DeathDef`; (f) `PlayerEventDef`;
//! 3. after each of those, its `BehaviorProviderDefinition`.
//!
//! Step (d), the pawn's instance-data hit regions, adds nothing for component entries: the body composition refuses a
//! component with a hit region, so its entries hold none. It is the only writer of `InstanceDataState.Data` in the
//! port, so no other kind of entry is there; for another kind the union's hit-region field has no meaning this
//! preload could rely on, so it is not read (review of #779). `ApplyBodyClassStartingValues` itself is script, and its other bound native,
//! `SetDeathDefinition`, reads the `DeathDef` already listed. A definition's sequences' own references (conditions,
//! behaviors) are not loaded: the preflight does not read them, and the commit's decode loads what it needs.
//!
//! Each load is logged ([`body_class_preload_line`]): an UNVERIFIED stand-in for the streaming load. An object already
//! resident is not logged. A field the object does not have, a None entry, or an object the store keeps as a stand-in
//! is skipped and logged; a load that fails is the step's error. Only the `maya` step's Off harness calls it.

use ow_object::ids::ObjRef;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::{object_path, object_slot, objects_of, present};

/// The log line of one body-class preload.
pub(crate) fn body_class_preload_line(path: &str) -> String {
    format!("harness input: set-up preload: {path} (UNVERIFIED: streamed with the character)")
}

/// Loads `o` through the store when it is a handle not loaded yet, logged before the load. Answers whether it loaded.
fn load(vm: &mut Vm, o: ObjRef) -> Result<bool, VmError> {
    if !vm.defs.is_unloaded(&vm.space, o) {
        return Ok(false);
    }
    let path = vm.defs.handle_path(o).unwrap_or_else(|| object_path(vm, o));
    vm.log.push(body_class_preload_line(&path));
    let loaded = vm.defs.ensure_loaded(&mut vm.space, o);
    vm.log.extend(vm.defs.take_log());
    if let Err(e) = loaded {
        vm.log.push(format!("harness input: set-up preload: {path}: failed ({e})"));
        return Err(VmError::from(e));
    }
    Ok(true)
}

/// Loads `o`, then reports whether it can be read; a stand-in (or a failed load) is logged as skipped.
fn load_readable(vm: &mut Vm, o: ObjRef, what: &str, loaded: &mut usize) -> Result<bool, VmError> {
    if load(vm, o)? {
        *loaded += 1;
    }
    if present(vm, o)? {
        return Ok(true);
    }
    vm.log.push(format!("harness input: set-up preload: the {what} {} is not resident; skipped", object_path(vm, o)));
    Ok(false)
}

/// The preload (module documentation). Answers the number of objects loaded, which is logged when it is not 0.
pub(crate) fn preload_body_class_definitions(vm: &mut Vm, archetype: ObjRef) -> Result<usize, VmError> {
    let mut loaded = 0;
    let Some(body) = object_slot(vm, archetype, "BodyClass")? else {
        vm.log.push("harness input: set-up preload: no BodyClass on the archetype, so no body-class definitions".into());
        return Ok(0);
    };
    if !load_readable(vm, body, "body class", &mut loaded)? {
        return Ok(loaded);
    }
    // The holders, in the preflight's order; each region's OverrideDeathDef is read once the region is loaded.
    let regions = objects_of(vm, body, "HitRegionList")?;
    let mut holders: Vec<ObjRef> = Vec::new();
    if let Some(d) = object_slot(vm, body, "DefaultHitRegion")?
        && regions.contains(&d)
    {
        holders.push(d);
    }
    let mut seen: Vec<ObjRef> = Vec::new();
    let mut visit = |vm: &mut Vm, o: ObjRef, what: &str, loaded: &mut usize| -> Result<bool, VmError> {
        if seen.contains(&o) {
            return Ok(present(vm, o)?);
        }
        seen.push(o);
        if !load_readable(vm, o, what, loaded)? {
            return Ok(false);
        }
        if let Some(def) = object_slot(vm, o, "BehaviorProviderDefinition")?
            && !seen.contains(&def)
        {
            seen.push(def);
            load_readable(vm, def, "behavior provider definition", loaded)?;
        }
        Ok(true)
    };
    for d in holders {
        visit(vm, d, "hit region", &mut loaded)?;
    }
    for region in regions {
        if visit(vm, region, "hit region", &mut loaded)?
            && let Some(death) = object_slot(vm, region, "OverrideDeathDef")?
        {
            visit(vm, death, "death definition", &mut loaded)?;
        }
    }
    for stance in objects_of(vm, body, "ScrapedStances")? {
        visit(vm, stance, "stance", &mut loaded)?;
    }
    for (name, what) in [("DeathDef", "death definition"), ("PlayerEventDef", "player event definition")] {
        if let Some(o) = object_slot(vm, body, name)? {
            visit(vm, o, what, &mut loaded)?;
        }
    }
    if loaded > 0 {
        vm.log.push(format!("harness input: set-up preload: {loaded} body-class definition objects loaded"));
    }
    Ok(loaded)
}

#[cfg(test)]
#[path = "body_class_preload_tests.rs"]
mod tests;
