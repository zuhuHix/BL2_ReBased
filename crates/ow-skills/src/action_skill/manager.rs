//! Owner: lane FIX-SKILLMGR-B. AI-assisted (Claude Code). The game's skill manager: `WillowGameInfo.GetSkillManager`
//! and the one case of `SkillEffectManager.DeactivateAllSkillsForInstigator` the record settles, implemented from
//! NATIVE_SKILL_MANAGER.md alone (decision 2026-11-19-14).
//!
//! **GetSkillManager.** The game info's `GlobalSkillManager` when it is set (whatever it holds: the native does not
//! check whether it was destroyed). Otherwise, when the game info is the authority (`Role` 3), a `SkillEffectManager`
//! is spawned through the world's actor spawn ([`ow_world::actors::spawn`]) by the game info, with the game info as
//! owner, no template, at location (0, 0, 0) and rotation (0, 0, 0); the result is stored only when it is a
//! `SkillEffectManager`, else None is stored, and the stored value is returned. Not the authority and no manager yet:
//! None. The record reads the game info as the spawn's owner without proving the argument position (UNVERIFIED). A
//! world that cannot spawn answers `NotYet` from the spawn itself (no actor list, no `WorldInfo`, the collision
//! layer), never None.
//!
//! **DeactivateAllSkillsForInstigator(Controller).** The manager's `ActiveSkills` in index order; for an entry that is
//! set and whose `SkillInstigator` is the controller, the record's walk calls one script function of the skill whose
//! name it did not resolve. So: no such entry (an empty list, only empty entries, only other instigators' skills) is a
//! no-op, and the first matching entry is `NotYet { NotRead }`, before anything is called.
//!
//! **Only in `ParityMode::Off`.** The C++ reference has no stand-in for either native, so under Reference both stay
//! unbound and the stub policy answers them, as before (architecture R3).
//!
//! Status: the record's reading of the executable; UNVERIFIED in the running game.

use std::rc::Rc;

use ow_combat::hit::access::set_required;
use ow_combat::hit::bind;
use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;
use ow_world::actors::{SpawnRequest, spawn};

use super::{arg_obj, byte_of, need, obj_value, script_error, this};

/// The record these natives follow.
pub const RECORD: &str = "NATIVE_SKILL_MANAGER.md";

/// `ROLE_Authority`.
const ROLE_AUTHORITY: u8 = 3;

/// The function paths this module binds.
pub const NATIVE_PATHS: [&str; 2] = [
    "WillowGame.WillowGameInfo.GetSkillManager",
    "WillowGame.SkillEffectManager.DeactivateAllSkillsForInstigator",
];

/// The declared kind of property `name` of `o`'s class.
fn kind_of(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<PropKind, VmError> {
    let class = vm.space.heap.get(o)?.class;
    vm.space
        .prop(class, name)
        .and_then(|p| vm.space.classes.decl(p))
        .map(|d| d.kind.clone())
        .ok_or_else(|| script_error(what, format!("the object has no property {name}")))
}

/// The zero value of the struct type of `o`'s property `name` (the game info's `Location` and `Rotation`: a vector
/// and a rotator at 0).
fn zero_like(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Value, VmError> {
    match kind_of(vm, o, name, what)? {
        PropKind::Struct(s) => vm
            .space
            .classes
            .struct_layout(s)
            .map(|l| l.zero().clone())
            .ok_or_else(|| script_error(what, format!("{name}: its struct has no layout"))),
        _ => Err(script_error(what, format!("{name}: expected a struct property"))),
    }
}

/// `WillowGameInfo.GetSkillManager()` on the game info `game` (module documentation). The manager's class is the one
/// `GlobalSkillManager` is declared with (`SkillEffectManager` in the installed package).
pub fn get_skill_manager(vm: &mut Vm, game: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let what = NATIVE_PATHS[0];
    if let Some(m) = obj_value(what, &need(vm, game, "GlobalSkillManager")?)? {
        return Ok(Some(m));
    }
    if byte_of(&need(vm, game, "Role")?, "Role")? != ROLE_AUTHORITY {
        return Ok(None);
    }
    let PropKind::Object { class } = kind_of(vm, game, "GlobalSkillManager", what)? else {
        return Err(script_error(what, "GlobalSkillManager: expected an object property"));
    };
    let mut req = SpawnRequest::new(Some(class));
    req.owner = Some(game);
    req.location = Some(zero_like(vm, game, "Location", what)?);
    req.rotation = Some(zero_like(vm, game, "Rotation", what)?);
    let made = spawn(vm, game, &req)?;
    let kept = match made {
        Some(m) if vm.space.classes.is_child_of(vm.space.heap.get(m)?.class, class) => Some(m),
        _ => None,
    };
    set_required(vm, game, "GlobalSkillManager", Value::Object(kept), what)?;
    Ok(kept)
}

/// `SkillEffectManager.DeactivateAllSkillsForInstigator(Controller)` on `manager` (module documentation).
pub fn deactivate_all_skills_for_instigator(
    vm: &mut Vm,
    manager: ObjRef,
    controller: Option<ObjRef>,
) -> Result<(), VmError> {
    let what = NATIVE_PATHS[1];
    let list = need(vm, manager, "ActiveSkills")?;
    let entries = list.elements().ok_or_else(|| {
        script_error(
            what,
            format!(
                "ActiveSkills: expected an array, found {}",
                list.kind_name()
            ),
        )
    })?;
    let mut skills = Vec::with_capacity(entries.len());
    for e in entries {
        if let Some(s) = obj_value("ActiveSkills", e)? {
            skills.push(s);
        }
    }
    for s in skills {
        let instigator = obj_value("SkillInstigator", &need(vm, s, "SkillInstigator")?)?;
        if instigator == controller {
            return Err(NotYet::not_read(
                "DeactivateAllSkillsForInstigator on a skill of the instigator: the script function the walk calls \
                 on it (its name is not resolved)",
                RECORD,
            )
            .into());
        }
    }
    Ok(())
}

/// The two natives as a [`NativePack`], bound by function path ([`bind`]) and only in `ParityMode::Off`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        if vm.parity == ParityMode::Reference {
            return Ok(());
        }
        let [get, deactivate] = NATIVE_PATHS;
        bind(
            vm,
            get,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let game = this(c, get)?;
                Ok(Value::Object(get_skill_manager(c.vm, game)?))
            }),
        )?;
        bind(
            vm,
            deactivate,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let manager = this(c, deactivate)?;
                let controller = arg_obj(c, 0)?;
                deactivate_all_skills_for_instigator(c.vm, manager, controller)?;
                Ok(Value::None)
            }),
        )
    }
}

#[cfg(test)]
mod tests;
