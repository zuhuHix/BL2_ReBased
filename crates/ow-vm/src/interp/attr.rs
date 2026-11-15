//! Owner: lane W3-INTERP. Opcode 0x5F, "let attribute" (NATIVE_BYTECODE_OPCODES.md, UNVERIFIED in the running game):
//! the port of `letAttribute` in `src/interp.cpp`.
//!
//! The left side names an attribute value property (a bare 0x5E, or `Context(...)` around one). The right side is
//! converted by the VM's assignment rule against the property's base companion, then `ow_attr::registry::let_base`
//! stores it into the base and recomputes the value from the base and the modifier stack in the 0x5F precision
//! (`CombineForm::Vm5F`). No change notification runs, and the engine's replication-dirty hook (an empty virtual in
//! the classes read) is skipped, as in the C++. A None context logs the error and still evaluates the right side.

use ow_attr::stack::CombineForm;
use ow_object::ids::PropAddr;
use ow_script::decode::Expr;
use ow_script::ops::op;

use super::convert;
use super::eval::{Ctx, Frame, context_target, eval, note_none_context};
use super::load::{decl_at, loaded_for_read, script_error};
use crate::api::Vm;
use crate::error::VmError;

pub(crate) fn let_attribute(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<(), VmError> {
    let mut left = &e.kids[0];
    if left.op == op::DYN_ARRAY_LENGTH {
        vm.log
            .push("Illegal call to execLetAttribute() for Array".to_string());
        return Ok(());
    }
    let mut container = ctx;
    while matches!(left.op, op::CONTEXT | op::CLASS_CONTEXT) {
        container = context_target(vm, f, left, container)?;
        if container.is_none() {
            break;
        }
        left = &left.kids[1];
    }
    let Some(object) = container else {
        note_none_context(vm, f);
        vm.log
            .push("Attempt to assign attribute through None".to_string());
        eval(vm, f, &e.kids[1], ctx)?;
        return Ok(());
    };
    let path = f.path().to_string();
    if left.op != op::OP_5E {
        return Err(script_error(
            &path,
            format!("let attribute: the left side is not an attribute reference in {path}"),
        ));
    }
    let decl = decl_at(vm, f.code.package, left.refs[0], &path)?;
    let name = vm.space.names.text(decl.name).into_owned();
    if !decl.kind.is_attribute() || !decl.is_stack_attribute() {
        return Err(script_error(
            &path,
            format!("let attribute: {name} is not an attribute value property in {path}"),
        ));
    }
    let rhs = eval(vm, f, &e.kids[1], ctx)?;
    let missing = || {
        script_error(
            &path,
            format!("let attribute: {name} is missing on its object in {path}"),
        )
    };
    if !loaded_for_read(vm, object, &path)? {
        return Err(missing());
    }
    let class = vm.space.heap.get(object)?.class;
    let prop = vm
        .space
        .classes
        .prop(class, decl.name)
        .ok_or_else(missing)?;
    let companions = vm
        .space
        .classes
        .decl(prop)
        .and_then(|d| d.attr)
        .ok_or_else(|| {
            script_error(
                &path,
                format!("let attribute: {name} has no modifier stack and base value in {path}"),
            )
        })?;
    let base = vm
        .space
        .read(object, PropAddr::scalar(companions.base))?
        .clone();
    let rhs = convert::assign(vm, &base, rhs);
    ow_attr::registry::let_base(&mut vm.space, object, companions, rhs, CombineForm::Vm5F)?;
    Ok(())
}
