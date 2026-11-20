//! Owner: lane IMPL-BODYCOMP-SOCKET-BONE-TABLES (decision 2026-11-19-NNN). The slice harness's preload of a skeletal
//! mesh's socket objects (hub ruling, 05:35Z), next to #592's mesh preload. AI-assisted (Claude Code).
//!
//! In the game the mesh's socket objects are in the same package as the mesh, and are presumably loaded with it.
//! The port's store leaves them as handles until something asks. The socket attach's rule
//! (NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md, `ow_world::world::skeletal::find_socket`) loads nothing itself, and refuses
//! `LayerAbsent` at the first unloaded socket it reaches. So the harness loads them here, as a labelled input:
//! **an UNVERIFIED stand-in for the streaming load**.
//!
//! - **Off only.** Outside Off nothing is done or logged.
//! - **Logged before and after:** the line it is about to do, then the counts, or the failure. The counts are the
//!   sockets loaded here, the ones already readable, and the unresolved stand-ins: an object with no slots whose class
//!   has some, which the store does not count as unloaded but the socket lookup refuses (review r1 of #641, item 3).
//!   A stand-in is not loaded here.
//! - **The mesh** is found through the store, which loads what it finds. #592's `preload_mesh` runs first and has
//!   loaded it already, so in the slice this adds no load for the mesh. A path no package holds is logged and skipped.
//! - **A failed socket load keeps its error:** it is logged and returned, as `preload_mesh` does.

use ow_core::{ObjectPath, ParityMode};
use ow_object::ids::PropAddr;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The start of each of this preload's harness lines.
pub(crate) const INPUT: &str = "harness input: preload the sockets of";

fn script(message: String) -> VmError {
    VmError::Script { function: "slice-harness".into(), message }
}

/// Loads the unloaded socket objects of the skeletal mesh at `path` (module documentation). Off only.
pub(crate) fn preload_mesh_sockets(vm: &mut Vm, path: &str) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    vm.log.push(format!("{INPUT} {path} (UNVERIFIED stand-in for the streaming load of the mesh's package)"));
    let parsed = ObjectPath::parse(path).map_err(|e| script(e.to_string()))?;
    let found = vm.defs.find(&mut vm.space, &parsed);
    vm.log.extend(vm.defs.take_log());
    let mesh = match found {
        Ok(Some(m)) => m,
        Ok(None) => {
            vm.log.push(format!("{INPUT} {path}: found nothing"));
            return Ok(());
        }
        Err(e) => {
            vm.log.push(format!("{INPUT} {path}: failed ({e})"));
            return Err(VmError::Def(e));
        }
    };
    let class = vm.space.heap.get(mesh)?.class;
    let Some(p) = vm.space.prop(class, "Sockets") else {
        return Err(script(format!("{path} has no Sockets")));
    };
    let list = vm.space.read(mesh, PropAddr::scalar(p))?.clone();
    let items = list.elements().ok_or_else(|| script(format!("{path}.Sockets is not an array")))?.to_vec();
    let (mut loaded, mut readable, mut stand_ins) = (0usize, 0usize, 0usize);
    for item in items {
        let Value::Object(Some(s)) = item else {
            continue;
        };
        if !vm.defs.is_unloaded(&vm.space, s) {
            let x = vm.space.heap.get(s)?;
            let stand_in = x.slots.is_empty() && vm.space.classes.get_layout(x.class).is_some_and(|l| l.slot_count() > 0);
            if stand_in {
                stand_ins += 1;
            } else {
                readable += 1;
            }
            continue;
        }
        let name = vm.space.names.text(vm.space.heap.get(s)?.name).into_owned();
        let r = vm.defs.ensure_loaded(&mut vm.space, s);
        vm.log.extend(vm.defs.take_log());
        if let Err(e) = r {
            vm.log.push(format!("{INPUT} {path}: the socket {name} failed ({e})"));
            return Err(VmError::Def(e));
        }
        loaded += 1;
    }
    vm.log.push(format!(
        "{INPUT} {path}: {loaded} loaded, {readable} already readable, {stand_ins} unresolved stand-ins"
    ));
    Ok(())
}

#[cfg(test)]
#[path = "mesh_sockets_tests.rs"]
mod tests;
