//! Owner: lane W4-INTERP2 (architecture section 8). Struct and dynamic-array operations: struct members, array
//! elements (static and dynamic), the dynamic-array natives (length, add, insert, remove, find, sort) and struct
//! comparison, a port of the matching cases of `lval` and `eval` in `src/interp.cpp`. AI-assisted (Claude Code).
//!
//! **Places inside aggregates.** Where the C++ hands out a `Value*` into a struct field or an array element, the
//! interpreter keeps an `eval::Place::In`: the base place (a frame slot, an object's property element, a temporary)
//! and the path of struct entries and array elements below it ([`Step`]). A read walks the path; a write goes through
//! the heap's `write_in` for a property (which checks the value against the kind it lands in, after the
//! representation rules of `interp`'s module documentation) or through the frame's storage for a local (untyped, as
//! in the C++). A path is followed again on every use, so an element that was removed since the place was taken is a
//! typed `VmError::Script` (the C++ pointer would dangle).
//!
//! **Static arrays** (`array_dim > 1`) are consecutive slots here (decision 2026-11-15-02), not one array value as in
//! the C++. `A[i]` on a static-array local, property or struct field is the element's own slot or entry; an index
//! outside `0..dim` is logged (`accessed array out of bounds: <i>`) and reads None, as the C++. A static array read
//! as a whole gives what the C++ value is, an array of its `dim` elements ([`whole_static`]), and a script callee's
//! static-array parameter takes such an array one element a slot (`eval::bind_parameter`); that is how compiled
//! script passes one (decision 2026-11-15-28). Any other whole use (assigned, passed `out`, a parameter given a value
//! of another shape) stays `NotYet`.
//!
//! **Dynamic arrays** follow the C++ cases one for one, the evaluation order included (the array's place, then the
//! operands, then the array is made an array if it was not one). Elements a heap array gains (`Add`, `Insert`, a
//! length set, a write past the end) are the element kind's zero: for the elements the C++ adds that is the same value
//! (`zeroLike` of a typed element is the kind's zero), and the C++ `None` it adds to an empty array is stored as the
//! kind's zero by the heap's representation rule. A local array gains `zeroLike` of its last element, or None when it
//! is empty, as in the C++.
//!
//! **`DynArraySort`** is a stub site in the C++ (`UNIMPLEMENTED DynArraySort`, nothing evaluated, nothing sorted) and
//! stays one here (`StubPolicy`); no comparator runs, so there is no sort order to match.
//!
//! **Rust-only bounds** (typed `VmError::Script`, never an abort; neither is met by compiled script as far as read):
//! a dynamic array holds at most [`MAX_ARRAY_LEN`] elements, and a value nests at most [`MAX_VALUE_DEPTH`] structs and
//! arrays deep. Locals are untyped, as in the C++, so script could nest an array in itself without the second bound,
//! and comparing, printing or dropping such a value recurses once per level.
//!
//! Status: a port of the C++ reading; every rule here is UNVERIFIED in the running game.

use std::rc::Rc;

use ow_core::{FixedHashMap, NotYet};
use ow_object::heap::{HeapError, Step};
use ow_object::ids::ObjRef;
use ow_object::ids::PropAddr;
use ow_object::layout::PropKind;
use ow_object::value::{Value, same_value};
use ow_script::decode::Expr;
use ow_script::ops::op;

use super::convert::text_of;
use super::eval::{
    Ctx, Frame, Place, context_target, enter, eval, fit_for_heap, frame_value_mut, local_place_of,
    lval, note_none_context, read, store, through_native, zero_for,
};
use super::load::{class_default, decl_at, loaded_for_read, script_error};
use crate::api::Vm;
use crate::error::VmError;

/// The most elements a dynamic array may hold (a Rust-only refusal; the C++ resizes to any 32-bit count).
pub const MAX_ARRAY_LEN: usize = 1 << 20;

/// The most structs and arrays a value may nest (a Rust-only refusal; see the module documentation).
pub const MAX_VALUE_DEPTH: usize = 256;

/// A static array used as a whole: `NotYet` (module documentation, "Static arrays").
pub(crate) fn static_array() -> VmError {
    NotYet::not_ported(
        "a static-array variable used as a whole (only its elements are read and written)",
        "decision 2026-11-15-20 (static arrays are consecutive slots)",
    )
    .into()
}

/// Operand `i` of `e`, or a script error naming the opcode (the decoder's layout gives every operand read here; a
/// missing one is refused, never a panic).
fn kid<'e>(f: &Frame, e: &'e Expr, i: usize) -> Result<&'e Expr, VmError> {
    e.kids.get(i).ok_or_else(|| {
        script_error(
            f.path(),
            format!(
                "expression 0x{:02x} has no operand {i} in {}",
                e.op,
                f.path()
            ),
        )
    })
}

/// The C++ `Value::integer()` as an int64 (no 32-bit wrap): Int, Byte and Bool as they are, a float toward zero (an
/// out-of-range or NaN float is `INT64_MIN`, the MSVC conversion), anything else 0.
fn integer(v: &Value) -> i64 {
    match v {
        Value::Int(i) => i64::from(*i),
        Value::Byte(b) => i64::from(*b),
        Value::Bool(b) => i64::from(*b),
        Value::Float(x) => {
            let x = f64::from(*x);
            // -2^63 <= x < 2^63 converts; anything else (NaN included) is the "integer indefinite" value.
            if (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&x) {
                // The oracle step: MSVC `int64_t(double)` (truncation toward zero) after the range check above, so
                // `as` cannot saturate here. A bare `as` on a float is against crate_edges N5; a refusing `trunc_i64`
                // in `ow_core::num` is a hub follow-up (already listed from W3-BALANCE), and this line moves to it then.
                x as i64
            } else {
                i64::MIN
            }
        }
        _ => 0,
    }
}

fn out_of_bounds(vm: &mut Vm, index: i64) {
    vm.log
        .push(format!("accessed array out of bounds: {index}"));
}

// ------------------------------------------------------------------------------------------------ paths

/// The place one step below `base`.
pub(crate) fn inside(base: Place, step: Step) -> Place {
    match base {
        Place::In(b, mut path) => {
            path.push(step);
            Place::In(b, path)
        }
        other => Place::In(Box::new(other), vec![step]),
    }
}

fn step_ref(v: &Value, s: Step) -> Option<&Value> {
    match (v, s) {
        (Value::Array(items), Step::Element(i)) => items.get(i as usize),
        (Value::Struct(st), Step::Field(e)) => st.fields.get(e as usize),
        _ => None,
    }
}

fn step_mut(v: &mut Value, s: Step) -> Option<&mut Value> {
    match s {
        Step::Element(i) => v.array_mut()?.get_mut(i as usize),
        Step::Field(e) => v.struct_mut()?.fields.get_mut(e as usize),
        _ => None,
    }
}

fn gone() -> VmError {
    script_error(
        "",
        "an aggregate place names a struct entry or array element that no longer exists (it was removed since the \
         place was taken)",
    )
}

/// The value `path` names below `v` (`Place::In`'s read).
pub(crate) fn walk<'a>(v: &'a Value, path: &[Step]) -> Result<&'a Value, VmError> {
    path.iter()
        .try_fold(v, |v, &s| step_ref(v, s))
        .ok_or_else(gone)
}

fn walk_mut<'a>(v: &'a mut Value, path: &[Step]) -> Result<&'a mut Value, VmError> {
    let mut v = v;
    for &s in path {
        v = step_mut(v, s).ok_or_else(gone)?;
    }
    Ok(v)
}

/// An aggregate's elements or entries and its allocation's address (shared copies have one address).
fn children(v: &Value) -> Option<(&[Value], usize)> {
    match v {
        Value::Array(items) => Some((items.as_slice(), Rc::as_ptr(items) as *const u8 as usize)),
        Value::Struct(s) => Some((s.fields.as_slice(), Rc::as_ptr(s) as *const u8 as usize)),
        _ => None,
    }
}

/// How many structs and arrays `v` nests (0 for a scalar, 1 for an aggregate of scalars), without recursion. Each
/// shared allocation is measured once (a value may hold one aggregate many times, `Items.AddItem(Items)`), so this
/// is linear in the distinct aggregates; past [`MAX_VALUE_DEPTH`] levels it stops and answers more than the bound.
fn depth(v: &Value) -> usize {
    let Some((kids, key)) = children(v) else {
        return 0;
    };
    let mut known: FixedHashMap<usize, usize> = FixedHashMap::default();
    // (address, entries, next entry, tallest entry so far)
    let mut stack = vec![(key, kids, 0usize, 0usize)];
    loop {
        let Some(top) = stack.last_mut() else {
            return 0;
        };
        if let Some(child) = top.1.get(top.2) {
            top.2 += 1;
            if let Some((kids, key)) = children(child) {
                match known.get(&key) {
                    Some(&h) => top.3 = top.3.max(h),
                    None if stack.len() > MAX_VALUE_DEPTH => return stack.len() + 1,
                    None => stack.push((key, kids, 0, 0)),
                }
            }
            continue;
        }
        let (key, _, _, tallest) = stack.pop().unwrap_or_default();
        let height = tallest + 1;
        known.insert(key, height);
        match stack.last_mut() {
            Some(parent) => parent.3 = parent.3.max(height),
            None => return height,
        }
    }
}

/// Refuses a value that would nest deeper than [`MAX_VALUE_DEPTH`] once stored `above` levels below its root.
fn check_depth(above: usize, v: &Value) -> Result<(), VmError> {
    let total = above + depth(v);
    if total > MAX_VALUE_DEPTH {
        return Err(script_error(
            "",
            format!(
                "a value would nest {total} structs and arrays deep; the bound is {MAX_VALUE_DEPTH} (a Rust-only \
                 refusal)"
            ),
        ));
    }
    Ok(())
}

/// The kind `path` lands in below property element `prop`, when every step fits the declarations.
fn kind_at(vm: &Vm, at: PropAddr, path: &[Step]) -> Option<PropKind> {
    let classes = &vm.space.classes;
    let mut kind = classes.decl(at.prop)?.kind.clone();
    for &s in path {
        kind = match (kind, s) {
            (PropKind::Array(inner), Step::Element(_)) => *inner,
            (PropKind::Struct(ty), Step::Field(e)) => {
                classes.struct_layout(ty)?.entry_field(e)?.1.kind.clone()
            }
            _ => return None,
        };
    }
    Some(kind)
}

/// A heap refusal of an object of an engine-native stand-in class is not proven (decision 2026-11-15-08).
fn heap_refusal(vm: &Vm, e: HeapError) -> VmError {
    match e {
        HeapError::WrongClass { found, .. } if through_native(vm, found) => NotYet::not_read(
            "whether an object of an engine-native stand-in class fits a typed object property",
            "decision 2026-11-15-08 (engine-native stand-in classes)",
        )
        .into(),
        other => other.into(),
    }
}

/// Whether [`store_in`] of `value` would be accepted, writing nothing (lane IMPL-ACTOR-TRACE): the same depth check,
/// the heap's own checks for a property base, and a path that leads somewhere for a frame base.
pub(crate) fn check_store_in(vm: &Vm, base: &Place, path: &[Step], value: &Value) -> Result<(), VmError> {
    check_depth(path.len(), value)?;
    match base {
        Place::Temp(_) => Ok(()),
        Place::Prop(o, at) => {
            let value = match kind_at(vm, *at, path) {
                Some(kind) => fit_for_heap(vm, &kind, value.clone()),
                None => value.clone(),
            };
            vm.space.heap.check_write_in(&vm.space.classes, *o, *at, path, &value).map_err(|e| heap_refusal(vm, e))
        }
        Place::In(..) => Err(script_error("", "a nested aggregate place (an interpreter bug)")),
        frame => walk(super::eval::place_ref(vm, frame)?, path).map(|_| ()),
    }
}

/// `Place::In`'s write: `value` stored at `path` below `base`, as it is (the C++ write through the pointer), with
/// the heap's representation rules for a property element.
pub(crate) fn store_in(
    vm: &mut Vm,
    base: &Place,
    path: &[Step],
    value: Value,
) -> Result<(), VmError> {
    check_depth(path.len(), &value)?;
    match base {
        Place::Temp(_) => Ok(()),
        Place::Prop(o, at) => {
            let value = match kind_at(vm, *at, path) {
                Some(kind) => fit_for_heap(vm, &kind, value),
                None => value,
            };
            vm.space
                .write_in(*o, *at, path, value)
                .map_err(|e| heap_refusal(vm, e))
        }
        Place::In(..) => Err(script_error(
            "",
            "a nested aggregate place (an interpreter bug)",
        )),
        frame => {
            let root = frame_value_mut(vm, frame)?
                .ok_or_else(|| script_error("", "not a frame place (an interpreter bug)"))?;
            *walk_mut(root, path)? = value;
            Ok(())
        }
    }
}

/// What an array gains: the element kind's zero for a heap array, `zeroLike` of the last element (None when empty)
/// for a local or temporary one (module documentation, "Dynamic arrays").
#[derive(Clone)]
enum Fill {
    Typed { zero: Value, inner: PropKind },
    Untyped,
}

impl Fill {
    fn like(&self, items: &[Value]) -> Value {
        match self {
            Fill::Typed { zero, .. } => zero.clone(),
            Fill::Untyped => items.last().map_or(Value::None, zero_like),
        }
    }
}

/// The C++ `zeroLike`: the zero of a value's own kind, a struct with every entry zeroed, an empty array; None for
/// None and delegates (an interface reference, which the C++ holds as an object, becomes a null one).
fn zero_like(v: &Value) -> Value {
    match v {
        Value::Int(_) => Value::Int(0),
        Value::Float(_) => Value::Float(0.0),
        Value::Bool(_) => Value::Bool(false),
        Value::Byte(_) => Value::Byte(0),
        Value::Str(_) => Value::string(""),
        Value::Name(_) => Value::Name(ow_object::ids::NameId::NONE),
        Value::Object(_) => Value::Object(None),
        Value::Interface(_) => Value::Interface(None),
        Value::Class(_) => Value::Class(None),
        Value::Struct(s) => {
            let mut z = Value::Struct(s.clone());
            if let Some(st) = z.struct_mut() {
                st.fields.iter_mut().for_each(|f| *f = zero_like(f));
            }
            z
        }
        Value::Array(_) => Value::array(Vec::new()),
        Value::None | Value::Delegate(_) => Value::None,
    }
}

/// Runs `change` on the array at `place` (the C++ `Value*` into it), with what it may gain; `None` when the place
/// does not hold an array. A temporary's array is changed on a copy that is dropped, as the C++ `scratch`; a
/// temporary that does not hold an array is the empty array the C++ makes of the scratch first (`ensure_array`
/// cannot store into a temporary).
fn with_array<R>(
    vm: &mut Vm,
    place: &Place,
    change: impl FnOnce(&mut Vec<Value>, &Fill) -> Result<R, VmError>,
) -> Result<Option<R>, VmError> {
    let (base, path): (&Place, &[Step]) = match place {
        Place::In(base, path) => (base, path),
        other => (other, &[]),
    };
    match base {
        Place::Temp(v) => {
            let mut copy = match walk(v, path) {
                Ok(array @ Value::Array(_)) => array.clone(),
                _ => Value::array(Vec::new()),
            };
            match copy.array_mut() {
                Some(items) => change(items, &Fill::Untyped).map(Some),
                None => Ok(None),
            }
        }
        Place::Prop(o, at) => {
            let fill = match kind_at(vm, *at, path) {
                Some(PropKind::Array(inner)) => Fill::Typed {
                    zero: vm.space.classes.zero_value(&inner),
                    inner: *inner,
                },
                _ => Fill::Untyped,
            };
            let root = vm.space.slot_mut(*o, *at)?;
            let result = match walk_mut(root, path)?.array_mut() {
                Some(items) => {
                    // Every array operation that changes an array changes its length (a set, an add or insert of
                    // one or more, a removal of present items); one that does not is a no-op (review r2 of #518).
                    let before = items.len();
                    let r = change(items, &fill)?;
                    Some((r, items.len() != before))
                }
                None => None,
            };
            Ok(result.map(|(r, changed)| {
                if changed {
                    vm.space.heap.note_write();
                }
                r
            }))
        }
        Place::In(..) => Err(script_error(
            "",
            "a nested aggregate place (an interpreter bug)",
        )),
        frame => {
            let root = frame_value_mut(vm, frame)?
                .ok_or_else(|| script_error("", "not a frame place (an interpreter bug)"))?;
            match walk_mut(root, path)?.array_mut() {
                Some(items) => change(items, &Fill::Untyped).map(Some),
                None => Ok(None),
            }
        }
    }
}

/// The C++ `if (array->kind != K::Array) *array = Value::makeArray();` (the heap refuses it for a property that is
/// not an array).
fn ensure_array(vm: &mut Vm, place: &Place) -> Result<(), VmError> {
    if !matches!(read(vm, place)?, Value::Array(_)) {
        store(vm, place, Value::array(Vec::new()))?;
    }
    Ok(())
}

/// Whether `place` is a temporary or lies inside one (the C++ `scratch`: writes into it are dropped).
fn in_temp(place: &Place) -> bool {
    match place {
        Place::Temp(_) => true,
        Place::In(base, _) => matches!(**base, Place::Temp(_)),
        _ => false,
    }
}

/// The levels of structs and arrays above `place` inside its root value.
fn path_len(place: &Place) -> usize {
    match place {
        Place::In(_, path) => path.len(),
        _ => 0,
    }
}

/// An element for a heap array: the representation rules, then the heap's check against the element kind.
fn typed_item(vm: &Vm, fill: &Fill, item: Value) -> Result<Value, VmError> {
    match fill {
        Fill::Typed { inner, .. } => {
            let item = fit_for_heap(vm, inner, item);
            vm.space
                .heap
                .check_value(&vm.space.classes, inner, &item)
                .map_err(|e| heap_refusal(vm, e))?;
            Ok(item)
        }
        Fill::Untyped => Ok(item),
    }
}

/// The fill an array at `place` would use, for checking an item before the array is borrowed.
fn fill_of(vm: &Vm, place: &Place) -> Fill {
    let (base, path): (&Place, &[Step]) = match place {
        Place::In(base, path) => (base, path),
        other => (other, &[]),
    };
    match base {
        Place::Prop(_, at) => match kind_at(vm, *at, path) {
            Some(PropKind::Array(inner)) => Fill::Typed {
                zero: vm.space.classes.zero_value(&inner),
                inner: *inner,
            },
            _ => Fill::Untyped,
        },
        _ => Fill::Untyped,
    }
}

/// A new length, refused past [`MAX_ARRAY_LEN`] (and reserved fallibly) before anything changes.
fn grow_to(items: &mut Vec<Value>, len: usize, like: Value) -> Result<(), VmError> {
    if len > MAX_ARRAY_LEN {
        return Err(script_error(
            "",
            format!(
                "a dynamic array would hold {len} elements; the bound is {MAX_ARRAY_LEN} (a Rust-only refusal)"
            ),
        ));
    }
    if len > items.len() {
        items.try_reserve_exact(len - items.len()).map_err(|_| {
            script_error(
                "",
                format!("cannot allocate a dynamic array of {len} elements"),
            )
        })?;
    }
    items.resize(len, like);
    Ok(())
}

/// An element count from a script integer: 0 for a negative one, refused past [`MAX_ARRAY_LEN`].
fn count_of(n: i64) -> Result<usize, VmError> {
    let n = usize::try_from(n.max(0)).unwrap_or(usize::MAX);
    if n > MAX_ARRAY_LEN {
        return Err(script_error(
            "",
            format!(
                "a dynamic-array operation asks for {n} elements; the bound is {MAX_ARRAY_LEN} (a Rust-only refusal)"
            ),
        ));
    }
    Ok(n)
}

fn element_step(index: usize) -> Result<Step, VmError> {
    u32::try_from(index)
        .map(Step::Element)
        .map_err(|_| script_error("", format!("array index {index} does not fit 32 bits")))
}

// ------------------------------------------------------------------------------------------------ members

/// The struct entry `name` of the struct held at `base`: its first entry and its static-array size, if `base` holds a
/// struct whose layout has the field (the C++ `field(name)`, any case).
fn field_of(
    vm: &Vm,
    base: &Place,
    name: ow_object::ids::NameId,
) -> Result<Option<(u32, i32)>, VmError> {
    let ty = match read(vm, base)? {
        Value::Struct(s) => s.ty,
        _ => return Ok(None),
    };
    Ok(vm
        .space
        .classes
        .struct_layout(ty)
        .and_then(|l| l.field(name))
        .map(|(first, d)| (first, d.array_dim)))
}

/// `StructMember` as a place (C++ `lval`): the struct's place first, then the member; a value that is not a struct,
/// or has no such member, is a temporary None (the C++ `scratch`).
pub(crate) fn member_place(
    vm: &mut Vm,
    f: &mut Frame,
    e: &Expr,
    ctx: Ctx,
) -> Result<Place, VmError> {
    let base = lval(vm, f, kid(f, e, 0)?, ctx)?;
    let member = decl_at(vm, f.code.package, e.refs[0], f.path())?;
    match field_of(vm, &base, member.name)? {
        None => Ok(Place::Temp(Value::None)),
        Some((_, dim)) if dim > 1 => Err(static_array()),
        Some((first, _)) => Ok(inside(base, Step::Field(first))),
    }
}

// ------------------------------------------------------------------------------------------------ elements

/// A static array (module documentation): its element 0 and its size.
enum StaticArray {
    /// Frame slots of a local: element `i` is `Place::Slot(at, slot + i)`.
    Local {
        at: super::eval::FrameRef,
        slot: usize,
        dim: u32,
    },
    /// A property of an object (or of a class default object).
    Prop {
        object: ObjRef,
        prop: ow_object::ids::PropId,
        dim: u32,
    },
    /// A struct field: entries `first ..` of the struct at `base`.
    Field { base: Place, first: u32, dim: u32 },
}

impl StaticArray {
    fn dim(&self) -> u32 {
        match self {
            StaticArray::Local { dim, .. }
            | StaticArray::Prop { dim, .. }
            | StaticArray::Field { dim, .. } => *dim,
        }
    }

    fn element(&self, i: u32) -> Place {
        match self {
            StaticArray::Local { at, slot, .. } => Place::Slot(*at, slot + i as usize),
            StaticArray::Prop { object, prop, .. } => {
                Place::Prop(*object, PropAddr::element(*prop, i))
            }
            StaticArray::Field { base, first, .. } => inside(base.clone(), Step::Field(first + i)),
        }
    }
}

/// What the array expression of an `ArrayElement` names.
enum ArrayBase {
    Static(StaticArray),
    Place(Place),
}

fn dim_of(d: i32) -> Option<u32> {
    u32::try_from(d).ok().filter(|&d| d > 1)
}

/// Whether `e` names a static-array variable or member (read from the declarations, nothing evaluated): a local, an
/// instance, state or default variable, a struct member, or one of them under `Context` wrappers.
pub(crate) fn names_static(vm: &mut Vm, f: &Frame, e: &Expr) -> bool {
    let mut e = e;
    while matches!(e.op, op::CONTEXT | op::CLASS_CONTEXT) {
        match e.kids.get(1) {
            Some(member) => e = member,
            None => return false,
        }
    }
    match e.op {
        op::LOCAL_VARIABLE | op::LOCAL_OUT_VARIABLE => f
            .code
            .by_export
            .get(&e.refs[0])
            .is_some_and(|&entry| f.code.locals[entry].decl.array_dim > 1),
        op::INSTANCE_VARIABLE | op::STATE_VARIABLE | op::DEFAULT_VARIABLE | op::STRUCT_MEMBER => {
            decl_at(vm, f.code.package, e.refs[0], f.path()).is_ok_and(|d| d.array_dim > 1)
        }
        _ => false,
    }
}

/// The array expression of an `ArrayElement` taken as a place, as `lval` takes it (one step and one nesting level per
/// expression, as `lval` counts them), for an expression [`names_static`] said names a static array.
fn static_base(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<ArrayBase, VmError> {
    enter(vm, f)?;
    let base = static_base_inner(vm, f, e, ctx);
    vm.code.nesting -= 1;
    base
}

fn static_base_inner(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<ArrayBase, VmError> {
    vm.tick()?;
    let temp = |v| Ok(ArrayBase::Place(Place::Temp(v)));
    match e.op {
        op::LOCAL_VARIABLE | op::LOCAL_OUT_VARIABLE => {
            let entry = f.code.by_export[&e.refs[0]];
            let dim = dim_of(f.code.locals[entry].decl.array_dim).ok_or_else(static_array)?;
            match local_place_of(f, entry) {
                Place::Slot(at, slot) => {
                    Ok(ArrayBase::Static(StaticArray::Local { at, slot, dim }))
                }
                // An out parameter aliasing the caller's static array (a whole static array passed).
                _ => Err(static_array()),
            }
        }
        op::INSTANCE_VARIABLE | op::STATE_VARIABLE | op::DEFAULT_VARIABLE => {
            let Some(object) = ctx else {
                return temp(Value::None);
            };
            let decl = decl_at(vm, f.code.package, e.refs[0], f.path())?;
            let object = if e.op == op::DEFAULT_VARIABLE {
                let class = vm.space.heap.get(object)?.class;
                class_default(vm, class)?
            } else {
                if !loaded_for_read(vm, object, f.path())? {
                    return temp(Value::None);
                }
                object
            };
            let class = vm.space.heap.get(object)?.class;
            let Some(prop) = vm.space.classes.prop(class, decl.name) else {
                return temp(Value::None);
            };
            let dim = vm
                .space
                .classes
                .decl(prop)
                .and_then(|d| dim_of(d.array_dim));
            Ok(match dim {
                Some(dim) => ArrayBase::Static(StaticArray::Prop { object, prop, dim }),
                None => ArrayBase::Place(Place::Prop(object, PropAddr::scalar(prop))),
            })
        }
        op::STRUCT_MEMBER => {
            let base = lval(vm, f, kid(f, e, 0)?, ctx)?;
            let member = decl_at(vm, f.code.package, e.refs[0], f.path())?;
            Ok(match field_of(vm, &base, member.name)? {
                None => ArrayBase::Place(Place::Temp(Value::None)),
                Some((first, dim)) => match dim_of(dim) {
                    Some(dim) => ArrayBase::Static(StaticArray::Field { base, first, dim }),
                    None => ArrayBase::Place(inside(base, Step::Field(first))),
                },
            })
        }
        op::CONTEXT | op::CLASS_CONTEXT => match context_target(vm, f, e, ctx)? {
            None => {
                note_none_context(vm, f);
                temp(zero_for(vm, f, e.refs[0]))
            }
            Some(target) => static_base(vm, f, kid(f, e, 1)?, Some(target)),
        },
        other => Err(script_error(
            f.path(),
            format!(
                "expression 0x{other:02x} names no static array in {}",
                f.path()
            ),
        )),
    }
}

/// A static-array variable or member read as a whole (C++ `eval` as `*lval(e)`, the array value of its elements): an
/// array of its `dim` elements, read in order, taking the place as `lval` takes it (the same steps). Where the C++
/// gets a scratch value (no object, no such property, a None context), so does this. Decision 2026-11-15-28.
pub(crate) fn whole_static(
    vm: &mut Vm,
    f: &mut Frame,
    e: &Expr,
    ctx: Ctx,
) -> Result<Value, VmError> {
    match static_base(vm, f, e, ctx)? {
        ArrayBase::Static(s) => {
            let mut items = Vec::new();
            items
                .try_reserve_exact(s.dim() as usize)
                .map_err(|_| script_error(f.path(), "cannot allocate a static array's elements"))?;
            for i in 0..s.dim() {
                items.push(read(vm, &s.element(i))?);
            }
            Ok(Value::array(items))
        }
        ArrayBase::Place(p) => read(vm, &p),
    }
}

/// The index, then the array's place (the C++ order).
fn index_and_base(
    vm: &mut Vm,
    f: &mut Frame,
    e: &Expr,
    ctx: Ctx,
) -> Result<(i64, ArrayBase), VmError> {
    let index = integer(&eval(vm, f, kid(f, e, 0)?, ctx)?);
    let array = kid(f, e, 1)?;
    let base = if names_static(vm, f, array) {
        static_base(vm, f, array, ctx)?
    } else {
        ArrayBase::Place(lval(vm, f, array, ctx)?)
    };
    Ok((index, base))
}

/// `ArrayElement` / `DynArrayElement` as a place (C++ `lval`): a dynamic array is made one if it is not; an index
/// below 0, or past the end of a static array, is logged and gives a temporary None; a `DynArrayElement` past the end
/// grows the array to hold it, an `ArrayElement` past the end is logged.
pub(crate) fn element_place(
    vm: &mut Vm,
    f: &mut Frame,
    e: &Expr,
    ctx: Ctx,
) -> Result<Place, VmError> {
    let (index, base) = index_and_base(vm, f, e, ctx)?;
    let base = match base {
        ArrayBase::Static(s) => {
            return Ok(match u32::try_from(index).ok().filter(|&i| i < s.dim()) {
                Some(i) => s.element(i),
                None => {
                    out_of_bounds(vm, index);
                    Place::Temp(Value::None)
                }
            });
        }
        ArrayBase::Place(p) => p,
    };
    ensure_array(vm, &base)?;
    let Ok(at) = usize::try_from(index) else {
        out_of_bounds(vm, index);
        return Ok(Place::Temp(Value::None));
    };
    let len = read(vm, &base)?.elements().map_or(0, <[Value]>::len);
    if at >= len {
        if e.op == op::ARRAY_ELEMENT {
            out_of_bounds(vm, index);
            return Ok(Place::Temp(Value::None));
        }
        // A dynamic array grows when written past its end (the C++ `resize(index + 1)` with None elements). A
        // temporary grows in the C++ scratch, which nothing reads again: the element is a temporary None.
        let need = at.checked_add(1).unwrap_or(usize::MAX);
        if in_temp(&base) {
            count_of(i64::try_from(need).unwrap_or(i64::MAX))?;
            return Ok(Place::Temp(Value::None));
        }
        with_array(vm, &base, |items, fill| {
            let like = match fill {
                Fill::Typed { zero, .. } => zero.clone(),
                Fill::Untyped => Value::None,
            };
            grow_to(items, need, like)
        })?;
    }
    Ok(inside(base, element_step(at)?))
}

/// `ArrayElement` / `DynArrayElement` read (C++ `eval`): past either end, or on a value that is not an array, a
/// logged None; reading never grows the array.
pub(crate) fn read_element(
    vm: &mut Vm,
    f: &mut Frame,
    e: &Expr,
    ctx: Ctx,
) -> Result<Value, VmError> {
    let (index, base) = index_and_base(vm, f, e, ctx)?;
    let place = match base {
        ArrayBase::Static(s) => u32::try_from(index)
            .ok()
            .filter(|&i| i < s.dim())
            .map(|i| s.element(i)),
        ArrayBase::Place(p) => {
            let v = read(vm, &p)?;
            let len = v.elements().map(<[Value]>::len);
            match (usize::try_from(index), len) {
                (Ok(i), Some(len)) if i < len => Some(inside(p, element_step(i)?)),
                _ => None,
            }
        }
    };
    match place {
        Some(p) => read(vm, &p),
        None => {
            out_of_bounds(vm, index);
            Ok(Value::None)
        }
    }
}

// ------------------------------------------------------------------------------------------------ dynamic arrays

/// `Let` whose left side is `DynArrayLength` (C++ `eval`, `EX_Let`): the array's place, then the length; the array is
/// resized (a negative length empties it) and the requested length is the result.
pub(crate) fn set_length(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    let lhs = kid(f, e, 0)?;
    let array = lval(vm, f, kid(f, lhs, 0)?, ctx)?;
    let length = integer(&eval(vm, f, kid(f, e, 1)?, ctx)?);
    ensure_array(vm, &array)?;
    let len = count_of(length)?;
    with_array(vm, &array, |items, fill| {
        let like = fill.like(items);
        grow_to(items, len, like)
    })?;
    Ok(Value::Int(length as i32))
}

/// The struct comparisons and the dynamic-array natives (C++ `eval`).
pub(crate) fn expression(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    match e.op {
        op::STRUCT_CMP_EQ | op::STRUCT_CMP_NE => {
            let a = eval(vm, f, kid(f, e, 0)?, ctx)?;
            let b = eval(vm, f, kid(f, e, 1)?, ctx)?;
            let same = same_value(&a, &b);
            Ok(Value::Bool(if e.op == op::STRUCT_CMP_EQ {
                same
            } else {
                !same
            }))
        }
        op::DYN_ARRAY_LENGTH => {
            let array = eval(vm, f, kid(f, e, 0)?, ctx)?;
            let len = array.elements().map_or(0, <[Value]>::len);
            Ok(Value::Int(i32::try_from(len).unwrap_or(i32::MAX)))
        }
        op::DYN_ARRAY_ADD => {
            let array = lval(vm, f, kid(f, e, 0)?, ctx)?;
            let count = integer(&eval(vm, f, kid(f, e, 1)?, ctx)?);
            ensure_array(vm, &array)?;
            let add = count_of(count)?;
            let first = with_array(vm, &array, |items, fill| {
                let first = items.len();
                let like = fill.like(items);
                grow_to(items, first.saturating_add(add), like)?;
                Ok(first)
            })?
            .unwrap_or(0);
            Ok(Value::Int(first as i32))
        }
        op::DYN_ARRAY_ADD_ITEM => {
            let array = lval(vm, f, kid(f, e, 0)?, ctx)?;
            let item = eval(vm, f, kid(f, e, 1)?, ctx)?;
            ensure_array(vm, &array)?;
            check_depth(path_len(&array) + 1, &item)?;
            let item = typed_item(vm, &fill_of(vm, &array), item)?;
            let last = with_array(vm, &array, |items, _| {
                let len = items.len() + 1;
                grow_to(items, len, item)?;
                Ok(items.len() - 1)
            })?
            .unwrap_or(0);
            Ok(Value::Int(last as i32))
        }
        op::DYN_ARRAY_INSERT => {
            let array = lval(vm, f, kid(f, e, 0)?, ctx)?;
            let at = integer(&eval(vm, f, kid(f, e, 1)?, ctx)?);
            let count = integer(&eval(vm, f, kid(f, e, 2)?, ctx)?);
            ensure_array(vm, &array)?;
            let count = count_of(count)?;
            with_array(vm, &array, |items, fill| {
                let position = clamp_index(at, items.len());
                let like = fill.like(items);
                let len = items.len().saturating_add(count);
                grow_to(items, len, like.clone())?;
                // The new elements were added at the end; move them to `position`.
                items[position..].rotate_right(count);
                Ok(())
            })?;
            Ok(Value::None)
        }
        op::DYN_ARRAY_INSERT_ITEM => {
            let array = lval(vm, f, kid(f, e, 0)?, ctx)?;
            let at = integer(&eval(vm, f, kid(f, e, 1)?, ctx)?);
            let item = eval(vm, f, kid(f, e, 2)?, ctx)?;
            ensure_array(vm, &array)?;
            check_depth(path_len(&array) + 1, &item)?;
            let item = typed_item(vm, &fill_of(vm, &array), item)?;
            with_array(vm, &array, |items, _| {
                let position = clamp_index(at, items.len());
                let len = items.len() + 1;
                grow_to(items, len, item)?;
                items[position..].rotate_right(1);
                Ok(())
            })?;
            Ok(Value::None)
        }
        op::DYN_ARRAY_REMOVE => {
            let array = lval(vm, f, kid(f, e, 0)?, ctx)?;
            let at = integer(&eval(vm, f, kid(f, e, 1)?, ctx)?);
            let count = integer(&eval(vm, f, kid(f, e, 2)?, ctx)?);
            with_array(vm, &array, |items, _| {
                if let Ok(at) = usize::try_from(at)
                    && at <= items.len()
                {
                    let count = usize::try_from(count.max(0)).unwrap_or(usize::MAX);
                    let last = items.len().min(at.saturating_add(count));
                    items.drain(at..last);
                }
                Ok(())
            })?;
            Ok(Value::None)
        }
        op::DYN_ARRAY_REMOVE_ITEM => {
            let array = lval(vm, f, kid(f, e, 0)?, ctx)?;
            let item = eval(vm, f, kid(f, e, 1)?, ctx)?;
            with_array(vm, &array, |items, _| {
                items.retain(|v| !same_value(v, &item));
                Ok(())
            })?;
            Ok(Value::None)
        }
        op::DYN_ARRAY_FIND => {
            let array = eval(vm, f, kid(f, e, 0)?, ctx)?;
            let item = eval(vm, f, kid(f, e, 1)?, ctx)?;
            let items = array.elements().unwrap_or(&[]);
            Ok(Value::Int(found(
                items.iter().position(|v| same_value(v, &item)),
            )))
        }
        op::DYN_ARRAY_FIND_STRUCT => {
            // After the array: the member's name (a name or string expression) and the value to match.
            let array = eval(vm, f, kid(f, e, 0)?, ctx)?;
            let member = eval(vm, f, kid(f, e, 1)?, ctx)?;
            let wanted = eval(vm, f, kid(f, e, 2)?, ctx)?;
            let name = vm.space.names.find(&text_of(vm, &member));
            let items = array.elements().unwrap_or(&[]);
            let at = name.and_then(|name| {
                items.iter().position(|item| {
                    member_value(vm, item, name).is_some_and(|v| same_value(&v, &wanted))
                })
            });
            Ok(Value::Int(found(at)))
        }
        op::DYN_ARRAY_SORT => {
            // The C++ stub: nothing is evaluated and nothing sorted (interp.cpp:466-468).
            vm.stub_site("DynArraySort", "UNIMPLEMENTED DynArraySort".to_string())?;
            Ok(Value::None)
        }
        other => Err(script_error(
            f.path(),
            format!("expression 0x{other:02x} is not an aggregate operation"),
        )),
    }
}

/// The C++ `std::clamp(at, 0, size)`.
fn clamp_index(at: i64, len: usize) -> usize {
    usize::try_from(at.max(0)).unwrap_or(usize::MAX).min(len)
}

fn found(at: Option<usize>) -> i32 {
    at.map_or(-1, |i| i32::try_from(i).unwrap_or(i32::MAX))
}

/// The member `name` of a struct value (the C++ `field(name)`): the entry, or a static-array field's entries as an
/// array (the C++ holds a static array as one array value).
fn member_value(vm: &Vm, item: &Value, name: ow_object::ids::NameId) -> Option<Value> {
    let s = item.as_struct()?;
    let (first, decl) = vm.space.classes.struct_layout(s.ty)?.field(name)?;
    let first = first as usize;
    match dim_of(decl.array_dim) {
        None => s.fields.get(first).cloned(),
        Some(dim) => s
            .fields
            .get(first..first + dim as usize)
            .map(|entries| Value::array(entries.to_vec())),
    }
}

/// The fixture the W4-INTERP2 tests share (an invented "Core" package, AGENTS.md rule 9), and the aggregate tests.
/// The struct, array, iterator and delegate cases of `tests/vm_test.py` are re-expressed here, in `iter.rs` and in
/// `delegate.rs`, with the C++ results; so are the cases of `tests/ui_trace_returns_test.py`, as the VM side of the
/// scalar trace replay (the out struct a native completes, a plain return, a failed call isolated from the next).
/// Every object is made with `Vm::instantiate` (zero slots), since `DefStore::class_default` waits for W3-DEFS.
#[cfg(test)]
pub(super) mod tests {
    use std::cell::Cell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use ow_attr::AttributeSystem;
    use ow_core::ParityMode;
    use ow_defs::DefStore;
    use ow_defs::identity::IdentityPolicy;
    use ow_object::heap::ObjectSpace;
    use ow_object::ids::{ObjRef, PropAddr};
    use ow_object::value::Value;
    use ow_package::testkit::Export;
    use ow_package::testkit::ints;
    use ow_package::testkit::reflect::{property_payload, toy_array_of, toy_struct};
    use ow_package::testkit::script::{Asm, CPF_OUT, FunctionIds, FunctionSpec, Param, ScriptToy};

    use crate::api::{CallResult, NativeCall, StubPolicy, Vm};
    use crate::error::VmError;
    use crate::natives_core;

    pub(crate) const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
    pub(crate) const OPERATOR: u32 = 0x400 | 0x1000 | 0x23000;
    pub(crate) const NATIVE: u32 = 0x400 | 0x20000;

    /// The invented package: `Object` (with the `+` operator, native 146) and `Foo`, a child of `Object`.
    pub(crate) struct Toy {
        pub(crate) t: ScriptToy,
        pub(crate) object: i32,
        pub(crate) foo: i32,
    }

    impl Toy {
        pub(crate) fn new() -> Toy {
            let mut t = ScriptToy::with_imports(&[
                "Class",
                "Function",
                "State",
                "ScriptStruct",
                "IntProperty",
                "FloatProperty",
                "BoolProperty",
                "StrProperty",
                "NameProperty",
                "ByteProperty",
                "ObjectProperty",
                "ClassProperty",
                "ArrayProperty",
                "StructProperty",
                "DelegateProperty",
                "InterfaceProperty",
            ]);
            let object = t.class("Object", 0);
            let mut s = FunctionSpec::new(OPERATOR, "+");
            s.native = 146;
            let ints = [Param::new("Int", "P0"), Param::new("Int", "P1")];
            t.function(
                object,
                "Add_IntInt",
                &ints,
                Some(("Int", 0)),
                &[],
                &s,
                |_| None,
            );
            let foo = t.class("Foo", object);
            Toy { t, object, foo }
        }

        /// A script function with `flags` whose body `body` builds from the child ids.
        #[allow(clippy::too_many_arguments)] // one call per invented function, each a full declaration
        pub(crate) fn function(
            &mut self,
            owner: i32,
            name: &str,
            flags: u32,
            params: &[Param],
            result: Option<(&str, i32)>,
            locals: &[(&str, &str)],
            body: impl FnOnce(&FunctionIds, &mut Asm),
        ) -> FunctionIds {
            let spec = FunctionSpec::new(flags, name);
            self.t
                .function(owner, name, params, result, locals, &spec, |ids| {
                    let mut a = Asm::new();
                    body(ids, &mut a);
                    Some(a)
                })
        }

        /// A final script function of `Foo`.
        pub(crate) fn script(
            &mut self,
            name: &str,
            params: &[Param],
            result: Option<(&str, i32)>,
            body: impl FnOnce(&FunctionIds, &mut Asm),
        ) -> FunctionIds {
            let foo = self.foo;
            self.function(foo, name, SCRIPT, params, result, &[], body)
        }

        /// A native of `owner` (native index `number`, 0 for none) with no script.
        pub(crate) fn native(
            &mut self,
            owner: i32,
            name: &str,
            params: &[Param],
            result: Option<(&str, i32)>,
            number: u16,
        ) -> FunctionIds {
            let mut spec = FunctionSpec::new(NATIVE, name);
            spec.native = number;
            self.t
                .function(owner, name, params, result, &[], &spec, |_| None)
        }

        /// The inner declaration of an array parameter or result, made before the function (re-parent it with
        /// [`Toy::adopt`] once the array's export is known).
        pub(crate) fn inner(&mut self, kind: &str, type_ref: i32) -> i32 {
            let name = format!("Inner{}", self.t.p.next_export());
            self.t.prop(kind, 0, &name, 0, type_ref)
        }

        pub(crate) fn adopt(&mut self, inner: i32, array: i32) {
            self.t.p.set_outer(inner, array);
        }

        /// A `<kind>Property` of `owner` with `dim` elements and the type words `tail`.
        pub(crate) fn prop_dim(
            &mut self,
            kind: &str,
            owner: i32,
            name: &str,
            dim: u32,
            tail: &[i32],
        ) -> i32 {
            let payload = property_payload(&self.t.none, dim, 0, &ints(tail));
            let mut export = Export::new(self.t.import(&format!("{kind}Property")), name, payload);
            export.outer = owner;
            self.t.p.add_export_with(export)
        }

        /// A struct `name` of `Object` with the fields `fields` (kind, name, dim), exported right after it.
        pub(crate) fn structure(&mut self, name: &str, fields: &[(&str, &str, u32)]) -> i32 {
            let object = self.object;
            let s = toy_struct(&mut self.t, object, name, None);
            for &(kind, field, dim) in fields {
                self.prop_dim(kind, s, field, dim, &[]);
            }
            s
        }

        /// An `array<kind>` property of `owner`.
        pub(crate) fn array_prop(&mut self, owner: i32, name: &str, kind: &str) -> i32 {
            toy_array_of(&mut self.t, owner, name, kind, 0)
        }

        pub(crate) fn run(&mut self, mode: ParityMode) -> Run {
            run_on(&self.t.build(), mode)
        }
    }

    /// `Return <expr>`, then `EndOfScript`.
    pub(crate) fn returns(a: &mut Asm, expr: impl FnOnce(&mut Asm)) {
        a.stmt().raw(&[0x04]);
        expr(a);
        a.end();
    }

    /// `Let lhs = rhs` as one statement.
    pub(crate) fn let_(a: &mut Asm, lhs: impl FnOnce(&mut Asm), rhs: impl FnOnce(&mut Asm)) {
        a.stmt().raw(&[0x0F]);
        lhs(a);
        rhs(a);
    }

    /// `expr` as one statement.
    pub(crate) fn stmt(a: &mut Asm, expr: impl FnOnce(&mut Asm)) {
        a.stmt();
        expr(a);
    }

    /// `Array.AddItem(item)` (0x55: array, skip, item, end of parameters).
    pub(crate) fn add_item(a: &mut Asm, array: impl FnOnce(&mut Asm), item: impl FnOnce(&mut Asm)) {
        a.raw(&[0x55]);
        array(a);
        a.w(0);
        item(a);
        a.raw(&[0x16]);
    }

    /// An opcode whose operands are the array, then parameters up to `EndFunctionParms` (`EP`).
    pub(crate) fn array_call(
        a: &mut Asm,
        op: u8,
        array: impl FnOnce(&mut Asm),
        params: impl FnOnce(&mut Asm),
    ) {
        a.raw(&[op]);
        array(a);
        params(a);
        a.raw(&[0x16]);
    }

    /// `Array.Length`.
    pub(crate) fn length(a: &mut Asm, array: impl FnOnce(&mut Asm)) {
        a.raw(&[0x36]);
        array(a);
    }

    /// `Array[index]`, dynamic (0x10) or static (0x1A).
    pub(crate) fn element(
        a: &mut Asm,
        op: u8,
        index: impl FnOnce(&mut Asm),
        array: impl FnOnce(&mut Asm),
    ) {
        a.raw(&[op]);
        index(a);
        array(a);
    }

    /// A VM over one package, its own temporary folder removed on drop.
    pub(crate) struct Run {
        pub(crate) vm: Vm,
        dir: PathBuf,
    }

    impl Drop for Run {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    static NEXT: AtomicU32 = AtomicU32::new(0);

    pub(crate) fn run_on(bytes: &[u8], mode: ParityMode) -> Run {
        let dir = std::env::temp_dir().join(format!(
            "ow-vm-interp2-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let defs = DefStore::from_synthetic(
            &dir,
            &[("Core.upk", bytes)],
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .unwrap();
        let mut vm = Vm::new(
            ObjectSpace::new(),
            defs,
            Rc::new(AttributeSystem::new_empty()),
            mode,
        )
        .with_stub_policy(StubPolicy::LogAndZero);
        natives_core::register(&mut vm).unwrap();
        Run { vm, dir }
    }

    impl Run {
        pub(crate) fn object(&mut self, class: &str) -> ObjRef {
            let c = self.vm.find_class(class).unwrap();
            self.vm.instantiate(c, "").unwrap()
        }

        pub(crate) fn call(
            &mut self,
            path: &str,
            this: Option<ObjRef>,
            args: Vec<Value>,
        ) -> Result<CallResult, VmError> {
            let f = self.vm.find_function(path)?;
            self.vm.call(f, this, args)
        }

        /// Calls `path` on `this`, returning the result.
        pub(crate) fn on(&mut self, this: ObjRef, path: &str, args: Vec<Value>) -> Value {
            self.call(path, Some(this), args).unwrap().ret
        }

        /// Element `index` of property `name` of `o`.
        pub(crate) fn prop(&self, o: ObjRef, name: &str, index: u32) -> Value {
            let class = self.vm.space.heap.get(o).unwrap().class;
            let id = self.vm.space.names.find(name).unwrap();
            let prop = self.vm.space.classes.prop(class, id).unwrap();
            self.vm
                .space
                .read(o, PropAddr::element(prop, index))
                .unwrap()
                .clone()
        }
    }

    /// Gives class parameter `child` its two type words (`Core.Class`, then the metaclass `meta`), which
    /// `ScriptToy::function` cannot write.
    pub(crate) fn class_param(toy: &mut Toy, child: i32, flags: u64, meta: i32) {
        let tail = ints(&[toy.t.import("Class"), meta]);
        let none = toy.t.none;
        toy.t
            .p
            .set_payload(child, property_payload(&none, 1, flags, &tail));
    }

    pub(crate) fn int(v: i32) -> Value {
        Value::Int(v)
    }

    pub(crate) fn ints_of(values: &[i32]) -> Value {
        Value::array(values.iter().map(|&v| Value::Int(v)).collect())
    }

    pub(crate) fn script_message(r: Result<impl std::fmt::Debug, VmError>) -> String {
        match r {
            Err(VmError::Script { message, .. }) => message,
            other => panic!("expected a script error, got {other:?}"),
        }
    }

    /// An `array<int>` parameter `name` (not supplied by the tests, so it starts as the empty array) and its inner
    /// declaration (to [`Toy::adopt`]).
    pub(crate) fn int_array(toy: &mut Toy, name: &str) -> (Param, i32) {
        let inner = toy.inner("Int", 0);
        (
            Param {
                type_ref: inner,
                ..Param::new("Array", name)
            },
            inner,
        )
    }

    #[test]
    fn a_local_array_grows_by_add_item_and_by_a_write_past_its_end() {
        let mut toy = Toy::new();
        // vm_test.py's Arr: AddItem 3 and 4, return the length; AddItem's result is the new element's index.
        let (items, inner) = int_array(&mut toy, "Items");
        let ids = toy.script("Arr", &[items], Some(("Int", 0)), |ids, a| {
            for v in [3, 4] {
                stmt(a, |a| {
                    add_item(
                        a,
                        |a| {
                            a.local(ids.get("Items"));
                        },
                        |a| {
                            a.int_const(v);
                        },
                    )
                });
            }
            returns(a, |a| {
                length(a, |a| {
                    a.local(ids.get("Items"));
                })
            });
        });
        toy.adopt(inner, ids.get("Items"));
        let (items, inner) = int_array(&mut toy, "Items");
        let ids = toy.script("FirstIndex", &[items], Some(("Int", 0)), |ids, a| {
            returns(a, |a| {
                add_item(
                    a,
                    |a| {
                        a.local(ids.get("Items"));
                    },
                    |a| {
                        a.int_const(9);
                    },
                )
            });
        });
        toy.adopt(inner, ids.get("Items"));
        // vm_test.py's Append: Items[Items.Length] = 7, then 8: one element per assignment (the target is taken
        // once).
        for (name, last) in [("Append", false), ("AppendLast", true)] {
            let (items, inner) = int_array(&mut toy, "Items");
            let ids = toy.script(name, &[items], Some(("Int", 0)), |ids, a| {
                let items = ids.get("Items");
                for v in [7, 8] {
                    let_(
                        a,
                        |a| {
                            element(
                                a,
                                0x10,
                                |a| {
                                    length(a, |a| {
                                        a.local(items);
                                    })
                                },
                                |a| {
                                    a.local(items);
                                },
                            )
                        },
                        |a| {
                            a.int_const(v);
                        },
                    );
                }
                if last {
                    returns(a, |a| {
                        element(
                            a,
                            0x10,
                            |a| {
                                a.int_const(1);
                            },
                            |a| {
                                a.local(items);
                            },
                        )
                    });
                } else {
                    returns(a, |a| {
                        length(a, |a| {
                            a.local(items);
                        })
                    });
                }
            });
            toy.adopt(inner, ids.get("Items"));
        }
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        assert_eq!(r.on(o, "Core.Foo.Arr", vec![]), int(2));
        assert_eq!(r.on(o, "Core.Foo.FirstIndex", vec![]), int(0));
        assert_eq!(r.on(o, "Core.Foo.Append", vec![]), int(2));
        assert_eq!(r.on(o, "Core.Foo.AppendLast", vec![]), int(8));
    }

    #[test]
    fn length_add_insert_remove_and_find_follow_the_cxx_cases() {
        let mut toy = Toy::new();
        let (items, inner) = int_array(&mut toy, "Items");
        let result = toy.inner("Int", 0);
        let ids = toy.script("Edit", &[items], Some(("Array", result)), |ids, a| {
            let items = ids.get("Items");
            let local = move |a: &mut Asm| {
                a.local(items);
            };
            for v in [1, 2, 3] {
                stmt(a, |a| {
                    add_item(a, local, |a| {
                        a.int_const(v);
                    })
                });
            }
            // Insert(1, 2): two zeroLike(3) = 0 at 1; InsertItem clamps its index to 0 .. length.
            stmt(a, |a| {
                array_call(a, 0x39, local, |a| {
                    a.int_const(1).int_const(2);
                })
            });
            stmt(a, |a| {
                array_call(a, 0x57, local, |a| {
                    a.int_const(99).int_const(7);
                })
            });
            stmt(a, |a| {
                array_call(a, 0x57, local, |a| {
                    a.int_const(-5).int_const(8);
                })
            });
            // [8, 1, 0, 0, 2, 3, 7]: Remove(1, 2), RemoveItem(0) removes every 0, Remove at the length or below 0
            // removes nothing.
            stmt(a, |a| {
                array_call(a, 0x40, local, |a| {
                    a.int_const(1).int_const(2);
                })
            });
            stmt(a, |a| {
                a.raw(&[0x56]);
                local(a);
                a.w(0).int_const(0).raw(&[0x16]);
            });
            stmt(a, |a| {
                array_call(a, 0x40, local, |a| {
                    a.int_const(4).int_const(9);
                })
            });
            stmt(a, |a| {
                array_call(a, 0x40, local, |a| {
                    a.int_const(-1).int_const(1);
                })
            });
            // Add(2) gives two zeroLike(7) = 0; the length set to 5 drops the last one.
            stmt(a, |a| {
                array_call(a, 0x54, local, |a| {
                    a.int_const(2);
                })
            });
            let_(
                a,
                |a| length(a, local),
                |a| {
                    a.int_const(5);
                },
            );
            returns(a, local);
        });
        toy.adopt(inner, ids.get("Items"));
        toy.adopt(result, ids.get("ReturnValue"));
        // Find on a supplied array.
        let (items, inner) = int_array(&mut toy, "Items");
        let ids = toy.script(
            "Find",
            &[items, Param::new("Int", "X")],
            Some(("Int", 0)),
            |ids, a| {
                returns(a, |a| {
                    a.raw(&[0x46])
                        .local(ids.get("Items"))
                        .w(0)
                        .local(ids.get("X"))
                        .raw(&[0x16]);
                });
            },
        );
        toy.adopt(inner, ids.get("Items"));
        // The length set's result is the length asked for; a negative one empties the array.
        let (items, inner) = int_array(&mut toy, "Items");
        let ids = toy.script("Clear", &[items], Some(("Int", 0)), |ids, a| {
            let items = ids.get("Items");
            stmt(a, |a| {
                add_item(
                    a,
                    |a| {
                        a.local(items);
                    },
                    |a| {
                        a.int_const(1);
                    },
                )
            });
            returns(a, |a| {
                a.raw(&[0x0F]);
                length(a, |a| {
                    a.local(items);
                });
                a.int_const(-3);
            });
        });
        toy.adopt(inner, ids.get("Items"));
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        assert_eq!(r.on(o, "Core.Foo.Edit", vec![]), ints_of(&[8, 2, 3, 7, 0]));
        let list = ints_of(&[4, 5, 6, 5]);
        assert_eq!(r.on(o, "Core.Foo.Find", vec![list.clone(), int(5)]), int(1));
        assert_eq!(r.on(o, "Core.Foo.Find", vec![list, int(9)]), int(-1));
        assert_eq!(r.on(o, "Core.Foo.Clear", vec![]), int(-3));
    }

    #[test]
    fn find_struct_matches_a_member_by_name() {
        let mut toy = Toy::new();
        let pair = toy.structure("Pair", &[("Int", "Tag", 1), ("Int", "Other", 1)]);
        let inner = toy.inner("Struct", pair);
        let tag = toy.t.p.fname("Tag", 0);
        let items = Param {
            type_ref: inner,
            ..Param::new("Array", "Items")
        };
        let ids = toy.script(
            "FindTag",
            &[items, Param::new("Int", "X")],
            Some(("Int", 0)),
            |ids, a| {
                // FindStruct: the array, a skip, the member's name (a NameConst), the value, end of parameters.
                returns(a, |a| {
                    a.raw(&[0x47])
                        .local(ids.get("Items"))
                        .w(0)
                        .raw(&[0x21])
                        .raw(&tag);
                    a.local(ids.get("X")).raw(&[0x16]);
                });
            },
        );
        toy.adopt(inner, ids.get("Items"));
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        let f = r.vm.find_function("Core.Foo.FindTag").unwrap();
        let classes = &r.vm.space.classes;
        let layout = classes
            .struct_by_path("Core.Object.Pair")
            .and_then(|s| classes.struct_layout(s))
            .unwrap();
        let names = &r.vm.space.names;
        let entry = |n: &str| layout.field(names.find(n).unwrap()).unwrap().0 as usize;
        let make = |t: i32, other: i32| {
            let mut v = layout.zero().clone();
            let s = v.struct_mut().unwrap();
            s.fields[entry("Tag")] = int(t);
            s.fields[entry("Other")] = int(other);
            v
        };
        let list = Value::array(vec![make(1, 5), make(5, 1)]);
        assert_eq!(
            r.vm.call(f, Some(o), vec![list.clone(), int(5)])
                .unwrap()
                .ret,
            int(1)
        );
        assert_eq!(
            r.vm.call(f, Some(o), vec![list, int(9)]).unwrap().ret,
            int(-1)
        );
    }

    #[test]
    fn a_local_array_set_longer_while_empty_gains_none_elements() {
        let mut toy = Toy::new();
        let (items, inner) = int_array(&mut toy, "Items");
        let result = toy.inner("Int", 0);
        let ids = toy.script("Grow", &[items], Some(("Array", result)), |ids, a| {
            let items = ids.get("Items");
            let_(
                a,
                |a| {
                    length(a, |a| {
                        a.local(items);
                    })
                },
                |a| {
                    a.int_const(2);
                },
            );
            returns(a, |a| {
                a.local(items);
            });
        });
        toy.adopt(inner, ids.get("Items"));
        toy.adopt(result, ids.get("ReturnValue"));
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        // The C++ resizes an empty array with `Value()` (None); a local is untyped, as there.
        assert_eq!(
            r.on(o, "Core.Foo.Grow", vec![]),
            Value::array(vec![Value::None, Value::None])
        );
    }

    #[test]
    fn an_instance_array_grows_with_the_element_kinds_zero_and_the_heap_checks_items() {
        let mut toy = Toy::new();
        let foo = toy.foo;
        let nums = toy.array_prop(foo, "Nums", "Int");
        toy.script("Write", &[], None, |_, a| {
            // Nums[2] = 5 grows the array with typed zeros; Nums.Length = 4 adds one more.
            let_(
                a,
                |a| {
                    element(
                        a,
                        0x10,
                        |a| {
                            a.int_const(2);
                        },
                        |a| {
                            a.instance(nums);
                        },
                    )
                },
                |a| {
                    a.int_const(5);
                },
            );
            let_(
                a,
                |a| {
                    length(a, |a| {
                        a.instance(nums);
                    })
                },
                |a| {
                    a.int_const(4);
                },
            );
            a.end();
        });
        toy.script("ReadFar", &[], Some(("Int", 0)), |_, a| {
            returns(a, |a| {
                element(
                    a,
                    0x10,
                    |a| {
                        a.int_const(10);
                    },
                    |a| {
                        a.instance(nums);
                    },
                )
            });
        });
        toy.script("AddFloat", &[], None, |_, a| {
            stmt(a, |a| {
                add_item(
                    a,
                    |a| {
                        a.instance(nums);
                    },
                    |a| {
                        a.float_const(1.5);
                    },
                )
            });
            a.end();
        });
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        r.on(o, "Core.Foo.Write", vec![]);
        assert_eq!(r.prop(o, "Nums", 0), ints_of(&[0, 0, 5, 0]));
        // Reading past the end is logged, reads None and does not grow the array.
        assert_eq!(r.on(o, "Core.Foo.ReadFar", vec![]), Value::None);
        assert!(
            r.vm.log
                .iter()
                .any(|l| l == "accessed array out of bounds: 10"),
            "{:?}",
            r.vm.log
        );
        assert_eq!(r.prop(o, "Nums", 0), ints_of(&[0, 0, 5, 0]));
        // An item of another kind is refused by the heap, and nothing is added.
        assert!(matches!(
            r.call("Core.Foo.AddFloat", Some(o), vec![]),
            Err(VmError::Heap(_))
        ));
        assert_eq!(r.prop(o, "Nums", 0), ints_of(&[0, 0, 5, 0]));
    }

    #[test]
    fn static_array_elements_are_their_own_slots() {
        let mut toy = Toy::new();
        let foo = toy.foo;
        let grid = toy.prop_dim("Int", foo, "Grid", 3, &[]);
        let pair = toy.structure("Pair", &[("Int", "Tag", 1), ("Int", "Slots", 2)]);
        let slots = pair + 2;
        toy.script("SetGrid", &[], Some(("Int", 0)), |_, a| {
            let_(
                a,
                |a| {
                    element(
                        a,
                        0x1A,
                        |a| {
                            a.int_const(1);
                        },
                        |a| {
                            a.instance(grid);
                        },
                    )
                },
                |a| {
                    a.int_const(7);
                },
            );
            // Writing below 0 or past the end is logged and writes nothing.
            for index in [-1, 3] {
                let_(
                    a,
                    |a| {
                        element(
                            a,
                            0x1A,
                            |a| {
                                a.int_const(index);
                            },
                            |a| {
                                a.instance(grid);
                            },
                        )
                    },
                    |a| {
                        a.int_const(4);
                    },
                );
            }
            returns(a, |a| {
                element(
                    a,
                    0x1A,
                    |a| {
                        a.int_const(1);
                    },
                    |a| {
                        a.instance(grid);
                    },
                )
            });
        });
        toy.script("ReadGridFar", &[], Some(("Int", 0)), |_, a| {
            returns(a, |a| {
                element(
                    a,
                    0x1A,
                    |a| {
                        a.int_const(3);
                    },
                    |a| {
                        a.instance(grid);
                    },
                )
            });
        });
        toy.script("WholeGrid", &[], Some(("Int", 0)), |_, a| {
            returns(a, |a| {
                a.instance(grid);
            });
        });
        // A static-array local (dim 2), and a struct's static-array field through a struct member.
        let s = Param {
            type_ref: pair,
            ..Param::new("Struct", "S")
        };
        let ids = toy.script(
            "Cells",
            &[Param::new("Int", "Cells"), s],
            Some(("Int", 0)),
            |ids, a| {
                let cells = ids.get("Cells");
                let s = ids.get("S");
                let_(
                    a,
                    |a| {
                        element(
                            a,
                            0x1A,
                            |a| {
                                a.int_const(1);
                            },
                            |a| {
                                a.local(cells);
                            },
                        )
                    },
                    |a| {
                        a.int_const(9);
                    },
                );
                let_(
                    a,
                    |a| {
                        element(
                            a,
                            0x1A,
                            |a| {
                                a.int_const(1);
                            },
                            |a| {
                                a.struct_member(slots, pair, |a| {
                                    a.local(s);
                                });
                            },
                        )
                    },
                    |a| {
                        a.int_const(6);
                    },
                );
                returns(a, |a| {
                    a.native(146, |a| {
                        element(
                            a,
                            0x1A,
                            |a| {
                                a.int_const(1);
                            },
                            |a| {
                                a.local(cells);
                            },
                        );
                        element(
                            a,
                            0x1A,
                            |a| {
                                a.int_const(1);
                            },
                            |a| {
                                a.struct_member(slots, pair, |a| {
                                    a.local(s);
                                });
                            },
                        );
                    });
                });
            },
        );
        let none = toy.t.none;
        toy.t
            .p
            .set_payload(ids.get("Cells"), property_payload(&none, 2, 0x80, &[]));
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        assert_eq!(r.on(o, "Core.Foo.SetGrid", vec![]), int(7));
        let grid: Vec<Value> = (0..3).map(|i| r.prop(o, "Grid", i)).collect();
        assert_eq!(grid, vec![int(0), int(7), int(0)]);
        let logged =
            r.vm.log
                .iter()
                .filter(|l| l.starts_with("accessed array out of bounds"))
                .count();
        assert_eq!(logged, 2, "{:?}", r.vm.log);
        assert_eq!(r.on(o, "Core.Foo.ReadGridFar", vec![]), Value::None);
        assert!(
            r.vm.log
                .iter()
                .any(|l| l == "accessed array out of bounds: 3")
        );
        assert_eq!(r.on(o, "Core.Foo.Cells", vec![]), int(15));
        // A static array read as a whole is the array of its elements, as the C++ value is (decision 28).
        assert_eq!(r.on(o, "Core.Foo.WholeGrid", vec![]), ints_of(&[0, 7, 0]));
    }

    #[test]
    fn struct_members_copy_on_assignment_and_compare_field_by_field() {
        let mut toy = Toy::new();
        let pair = toy.structure("Pair", &[("Int", "Tag", 1), ("Int", "Other", 1)]);
        let tag = pair + 1;
        let two = [
            Param {
                type_ref: pair,
                ..Param::new("Struct", "S")
            },
            Param {
                type_ref: pair,
                ..Param::new("Struct", "T")
            },
        ];
        let member = move |a: &mut Asm, of: i32| {
            a.struct_member(tag, pair, |a| {
                a.local(of);
            });
        };
        toy.script("Copy", &two, Some(("Int", 0)), |ids, a| {
            let (s, t) = (ids.get("S"), ids.get("T"));
            let_(
                a,
                |a| member(a, s),
                |a| {
                    a.int_const(5);
                },
            );
            let_(
                a,
                |a| {
                    a.local(t);
                },
                |a| {
                    a.local(s);
                },
            );
            let_(
                a,
                |a| member(a, t),
                |a| {
                    a.int_const(6);
                },
            );
            returns(a, |a| member(a, s));
        });
        for (name, op, other) in [("Same", 0x32, 5), ("Differ", 0x32, 4), ("NotSame", 0x33, 5)] {
            toy.script(name, &two, Some(("Bool", 0)), |ids, a| {
                let (s, t) = (ids.get("S"), ids.get("T"));
                let_(
                    a,
                    |a| member(a, s),
                    |a| {
                        a.int_const(5);
                    },
                );
                let_(
                    a,
                    |a| member(a, t),
                    |a| {
                        a.int_const(other);
                    },
                );
                returns(a, |a| {
                    a.raw(&[op]).reference(pair).local(s).local(t);
                });
            });
        }
        // A member of a value that is not a struct is a temporary None (the C++ scratch).
        toy.script(
            "NotAStruct",
            &[Param::new("Int", "X")],
            Some(("Int", 0)),
            |ids, a| {
                returns(a, |a| member(a, ids.get("X")));
            },
        );
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        assert_eq!(r.on(o, "Core.Foo.Copy", vec![]), int(5));
        assert_eq!(r.on(o, "Core.Foo.Same", vec![]), Value::Bool(true));
        assert_eq!(r.on(o, "Core.Foo.Differ", vec![]), Value::Bool(false));
        assert_eq!(r.on(o, "Core.Foo.NotSame", vec![]), Value::Bool(false));
        assert_eq!(r.on(o, "Core.Foo.NotAStruct", vec![int(3)]), Value::None);
    }

    /// `Bump(out int X) { X = X + 1 }` and `Shrink(out int X) { Nums.Remove(0, 9); X = 1 }` on `Foo`.
    fn bump_and_shrink(toy: &mut Toy, nums: i32) -> (i32, i32) {
        let x = Param {
            flags: CPF_OUT,
            ..Param::new("Int", "X")
        };
        let bump = toy.script("Bump", std::slice::from_ref(&x), None, |ids, a| {
            let x = ids.get("X");
            let_(
                a,
                |a| {
                    a.raw(&[0x48]).reference(x);
                },
                |a| {
                    a.native(146, |a| {
                        a.raw(&[0x48]).reference(x).int_const(1);
                    });
                },
            );
            a.end();
        });
        let shrink = toy.script("Shrink", std::slice::from_ref(&x), None, |ids, a| {
            stmt(a, |a| {
                array_call(
                    a,
                    0x40,
                    |a| {
                        a.instance(nums);
                    },
                    |a| {
                        a.int_const(0).int_const(9);
                    },
                )
            });
            let_(
                a,
                |a| {
                    a.raw(&[0x48]).reference(ids.get("X"));
                },
                |a| {
                    a.int_const(1);
                },
            );
            a.end();
        });
        (bump.function, shrink.function)
    }

    #[test]
    fn an_out_argument_aliases_an_array_element_and_a_removed_one_is_a_typed_error() {
        let mut toy = Toy::new();
        let foo = toy.foo;
        let nums = toy.array_prop(foo, "Nums", "Int");
        let (bump, shrink) = bump_and_shrink(&mut toy, nums);
        let (items, inner) = int_array(&mut toy, "Items");
        let ids = toy.script("BumpLocal", &[items], Some(("Int", 0)), |ids, a| {
            let items = ids.get("Items");
            stmt(a, |a| {
                add_item(
                    a,
                    |a| {
                        a.local(items);
                    },
                    |a| {
                        a.int_const(41);
                    },
                )
            });
            stmt(a, |a| {
                a.final_function(bump, |a| {
                    element(
                        a,
                        0x10,
                        |a| {
                            a.int_const(0);
                        },
                        |a| {
                            a.local(items);
                        },
                    );
                });
            });
            returns(a, |a| {
                element(
                    a,
                    0x10,
                    |a| {
                        a.int_const(0);
                    },
                    |a| {
                        a.local(items);
                    },
                )
            });
        });
        toy.adopt(inner, ids.get("Items"));
        toy.script("BumpInstance", &[], None, |_, a| {
            for v in [1, 2] {
                stmt(a, |a| {
                    add_item(
                        a,
                        |a| {
                            a.instance(nums);
                        },
                        |a| {
                            a.int_const(v);
                        },
                    )
                });
            }
            stmt(a, |a| {
                a.final_function(bump, |a| {
                    element(
                        a,
                        0x10,
                        |a| {
                            a.int_const(1);
                        },
                        |a| {
                            a.instance(nums);
                        },
                    );
                });
            });
            a.end();
        });
        toy.script("ShrinkInstance", &[], None, |_, a| {
            stmt(a, |a| {
                a.final_function(shrink, |a| {
                    element(
                        a,
                        0x10,
                        |a| {
                            a.int_const(1);
                        },
                        |a| {
                            a.instance(nums);
                        },
                    );
                });
            });
            a.end();
        });
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        assert_eq!(r.on(o, "Core.Foo.BumpLocal", vec![]), int(42));
        r.on(o, "Core.Foo.BumpInstance", vec![]);
        assert_eq!(r.prop(o, "Nums", 0), ints_of(&[1, 3]));
        // The C++ pointer would dangle; here the path no longer leads anywhere.
        let message = script_message(r.call("Core.Foo.ShrinkInstance", Some(o), vec![]));
        assert!(message.contains("no longer exists"), "{message}");
        assert_eq!(r.prop(o, "Nums", 0), ints_of(&[]));
    }

    #[test]
    fn a_write_past_the_end_of_a_temporary_array_is_dropped_as_the_cxx_scratch() {
        let mut toy = Toy::new();
        let foo = toy.foo;
        let nums = toy.array_prop(foo, "Nums", "Int");
        let o = Param {
            type_ref: toy.object,
            ..Param::new("Object", "O")
        };
        // O.Nums[3] = 1 and O.Nums.AddItem(2) with O None: the C++ grows its scratch; nothing is written anywhere.
        toy.script("GrowNone", &[o], Some(("Int", 0)), |ids, a| {
            let o = ids.get("O");
            let through = move |a: &mut Asm| {
                a.context(
                    |a| {
                        a.local(o);
                    },
                    |a| {
                        a.instance(nums);
                    },
                );
            };
            let_(
                a,
                |a| {
                    element(
                        a,
                        0x10,
                        |a| {
                            a.int_const(3);
                        },
                        through,
                    )
                },
                |a| {
                    a.int_const(1);
                },
            );
            returns(a, |a| {
                add_item(a, through, |a| {
                    a.int_const(2);
                })
            });
        });
        let mut r = toy.run(ParityMode::Off);
        let me = r.object("Core.Foo");
        assert_eq!(
            r.on(me, "Core.Foo.GrowNone", vec![Value::Object(None)]),
            int(0)
        );
        assert_eq!(r.prop(me, "Nums", 0), ints_of(&[]));
    }

    #[test]
    fn a_value_nested_past_the_bound_is_refused_not_an_overflow() {
        let mut toy = Toy::new();
        for op in [0x55u8, 0x10] {
            let (items, inner) = int_array(&mut toy, "Items");
            let name = if op == 0x55 {
                "SelfAppend"
            } else {
                "SelfStore"
            };
            let ids = toy.script(name, &[items], None, |ids, a| {
                let items = ids.get("Items");
                let start = a.here();
                if op == 0x55 {
                    // Items.AddItem(Items) forever: each pass nests the array one level deeper.
                    stmt(a, |a| {
                        add_item(
                            a,
                            |a| {
                                a.local(items);
                            },
                            |a| {
                                a.local(items);
                            },
                        )
                    });
                } else {
                    // Items[0] = Items forever.
                    let_(
                        a,
                        |a| {
                            element(
                                a,
                                0x10,
                                |a| {
                                    a.int_const(0);
                                },
                                |a| {
                                    a.local(items);
                                },
                            )
                        },
                        |a| {
                            a.local(items);
                        },
                    );
                }
                a.stmt().raw(&[0x06]).w(u16::try_from(start).unwrap());
                a.end();
            });
            toy.adopt(inner, ids.get("Items"));
        }
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        for name in ["Core.Foo.SelfAppend", "Core.Foo.SelfStore"] {
            let message = script_message(r.call(name, Some(o), vec![]));
            assert!(
                message.contains("structs and arrays deep"),
                "{name}: {message}"
            );
        }
    }

    #[test]
    fn dyn_array_sort_is_the_cxx_stub_site_and_evaluates_nothing() {
        let mut toy = Toy::new();
        let answer = toy.script("Answer", &[], Some(("Int", 0)), |_, a| {
            returns(a, |a| {
                a.int_const(42);
            })
        });
        let (items, inner) = int_array(&mut toy, "Items");
        let result = toy.inner("Int", 0);
        let ids = toy.script("Sort", &[items], Some(("Array", result)), |ids, a| {
            let items = ids.get("Items");
            for v in [3, 1, 2] {
                stmt(a, |a| {
                    add_item(
                        a,
                        |a| {
                            a.local(items);
                        },
                        |a| {
                            a.int_const(v);
                        },
                    )
                });
            }
            stmt(a, |a| {
                a.raw(&[0x59]).local(items);
                a.final_function(answer.function, |_| {});
                a.raw(&[0x16]);
            });
            returns(a, |a| {
                a.local(items);
            });
        });
        toy.adopt(inner, ids.get("Items"));
        toy.adopt(result, ids.get("ReturnValue"));
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        r.vm.trace = Some(Vec::new());
        assert_eq!(r.on(o, "Core.Foo.Sort", vec![]), ints_of(&[3, 1, 2]));
        assert!(
            r.vm.log.iter().any(|l| l == "UNIMPLEMENTED DynArraySort"),
            "{:?}",
            r.vm.log
        );
        assert_eq!(
            r.vm.trace.as_ref().unwrap(),
            &["Core.Foo.Sort".to_string()],
            "the comparator never ran"
        );
        r.vm.stub_policy = StubPolicy::refuse_all();
        let refused = r.call("Core.Foo.Sort", Some(o), vec![]);
        assert!(matches!(refused, Err(VmError::UnboundNative(k)) if k == "DynArraySort"));
    }

    // ---- tests/ui_trace_returns_test.py, as the VM side of the scalar trace replay

    #[test]
    fn the_display_getter_keeps_the_natives_completed_out_struct_and_the_receiver() {
        let mut toy = Toy::new();
        let info = toy.structure(
            "DisplayInfo",
            &[("Float", "X", 1), ("Float", "Z", 1), ("Bool", "hasZ", 1)],
        );
        let x = info + 1;
        let d = Param {
            flags: CPF_OUT,
            type_ref: info,
            ..Param::new("Struct", "D")
        };
        let foo = toy.foo;
        let getter = toy.native(foo, "GetDisplayInfo", std::slice::from_ref(&d), None, 0);
        let s = Param {
            type_ref: info,
            ..Param::new("Struct", "S")
        };
        toy.script("ReadX", &[s], Some(("Float", 0)), |ids, a| {
            stmt(a, |a| {
                a.final_function(getter.function, |a| {
                    a.local(ids.get("S"));
                });
            });
            returns(a, |a| {
                a.struct_member(x, info, |a| {
                    a.local(ids.get("S"));
                });
            });
        });
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        let seen = Rc::new(Cell::new(None));
        let id = r.vm.find_function("Core.Foo.GetDisplayInfo").unwrap();
        let key = r.vm.function(id).unwrap().native_key.clone().unwrap();
        let receiver = seen.clone();
        // The struct's entries in its layout's order.
        let entries: Vec<usize> = {
            let classes = &r.vm.space.classes;
            let layout = classes
                .struct_by_path("Core.Object.DisplayInfo")
                .and_then(|s| classes.struct_layout(s))
                .unwrap();
            ["X", "Z", "hasZ"]
                .iter()
                .map(|n| layout.field(r.vm.space.names.find(n).unwrap()).unwrap().0 as usize)
                .collect()
        };
        let at = entries.clone();
        r.vm.register_native(
            &key,
            Rc::new(move |c: &mut NativeCall<'_>| {
                receiver.set(c.this);
                // The native completes D: the value before the call is the empty struct.
                let mut out = c.arg(0)?.clone();
                let s = out.struct_mut().expect("D is a struct");
                s.fields[at[0]] = Value::Float(120.0);
                s.fields[at[1]] = Value::Float(-300.0);
                s.fields[at[2]] = Value::Bool(true);
                c.set_out(0, out)?;
                Ok(Value::None)
            }),
        )
        .unwrap();
        // Through script: the caller's struct holds the completed output.
        assert_eq!(r.on(o, "Core.Foo.ReadX", vec![]), Value::Float(120.0));
        assert_eq!(seen.get(), Some(o), "the receiver identity");
        // From the host: the out value is the completed one, and there is no return value.
        let kind = r.vm.function(id).unwrap().params[0].kind.clone();
        let empty = r.vm.space.classes.default_value(&kind).unwrap();
        let done = r.vm.call(id, Some(o), vec![empty]).unwrap();
        assert_eq!(done.ret, Value::None);
        let out = done.outs[0].as_struct().unwrap();
        let got: Vec<Value> = entries.iter().map(|&e| out.fields[e].clone()).collect();
        assert_eq!(
            got,
            vec![Value::Float(120.0), Value::Float(-300.0), Value::Bool(true)]
        );
    }

    #[test]
    fn other_returns_need_no_display_arguments() {
        let mut toy = Toy::new();
        toy.script("ReadCount", &[], Some(("Int", 0)), |_, a| {
            returns(a, |a| {
                a.int_const(7);
            })
        });
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        let done = r.call("Core.Foo.ReadCount", Some(o), vec![]).unwrap();
        assert_eq!(done.ret, int(7));
        assert!(done.outs.is_empty());
    }

    #[test]
    fn a_failed_call_is_isolated_from_the_next_one() {
        let mut toy = Toy::new();
        let foo = toy.foo;
        let nums = toy.array_prop(foo, "Nums", "Int");
        let (_, shrink) = bump_and_shrink(&mut toy, nums);
        toy.script("Fail", &[], None, |_, a| {
            stmt(a, |a| {
                add_item(
                    a,
                    |a| {
                        a.instance(nums);
                    },
                    |a| {
                        a.int_const(1);
                    },
                )
            });
            stmt(a, |a| {
                a.final_function(shrink, |a| {
                    element(
                        a,
                        0x10,
                        |a| {
                            a.int_const(0);
                        },
                        |a| {
                            a.instance(nums);
                        },
                    );
                });
            });
            a.end();
        });
        toy.script("ReadCount", &[], Some(("Int", 0)), |_, a| {
            returns(a, |a| {
                a.int_const(7);
            })
        });
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        assert!(r.call("Core.Foo.Fail", Some(o), vec![]).is_err());
        // Nothing of the failed call is left on the frame stack, and the next call runs as on a fresh VM.
        assert!(
            r.vm.code.frames.is_empty() && r.vm.code.nesting == 0 && r.vm.code.stack.is_empty()
        );
        let done = r.call("Core.Foo.ReadCount", Some(o), vec![]).unwrap();
        assert_eq!(done.ret, int(7));
        // One step for the call, one for the statement, one for the constant.
        assert_eq!(r.vm.steps(), 3);
    }

    #[test]
    fn depth_is_linear_in_the_shared_aggregates() {
        // A value holding one array twice at each of 40 levels: 2^40 paths, 41 allocations.
        let mut v = Value::array(vec![]);
        for _ in 0..40 {
            v = Value::array(vec![v.clone(), v]);
        }
        assert_eq!(super::depth(&v), 41);
        assert_eq!(super::depth(&int(1)), 0);
        let mut deep = Value::array(vec![]);
        for _ in 0..(super::MAX_VALUE_DEPTH + 5) {
            deep = Value::array(vec![deep]);
        }
        assert!(super::depth(&deep) > super::MAX_VALUE_DEPTH);
        assert!(super::check_depth(0, &deep).is_err());
    }

    // ---- decision 2026-11-15-28: the sweep's cause C gaps, on invented data

    /// A static array passed whole: read as an array of its elements, and spread into the callee's static-array
    /// parameter, from a property and from a local; another shape for that parameter is `NotYet`.
    #[test]
    fn a_static_array_passes_whole_into_a_static_array_parameter() {
        let mut toy = Toy::new();
        let foo = toy.foo;
        let grid = toy.prop_dim("Int", foo, "Grid", 3, &[]);
        let none = toy.t.none;
        // TakeGrid(int G[3]) returns G[1] + G[2].
        let take = toy.script(
            "TakeGrid",
            &[Param::new("Int", "G")],
            Some(("Int", 0)),
            |ids, a| {
                let g = ids.get("G");
                returns(a, |a| {
                    a.native(146, |a| {
                        for i in [1, 2] {
                            element(
                                a,
                                0x1A,
                                |a| {
                                    a.int_const(i);
                                },
                                |a| {
                                    a.local(g);
                                },
                            );
                        }
                    });
                });
            },
        );
        toy.t
            .p
            .set_payload(take.get("G"), property_payload(&none, 3, 0x80, &[]));
        toy.script("PassGrid", &[], Some(("Int", 0)), |_, a| {
            for (i, v) in [(1, 7), (2, 5)] {
                let_(
                    a,
                    |a| {
                        element(
                            a,
                            0x1A,
                            |a| {
                                a.int_const(i);
                            },
                            |a| {
                                a.instance(grid);
                            },
                        )
                    },
                    |a| {
                        a.int_const(v);
                    },
                );
            }
            returns(a, |a| {
                a.final_function(take.function, |a| {
                    a.instance(grid);
                });
            });
        });
        // PassLocal(int L[3]) passes its own static-array parameter on whole.
        let pass = toy.script(
            "PassLocal",
            &[Param::new("Int", "L")],
            Some(("Int", 0)),
            |ids, a| {
                let l = ids.get("L");
                returns(a, |a| {
                    a.final_function(take.function, |a| {
                        a.local(l);
                    });
                });
            },
        );
        toy.t
            .p
            .set_payload(pass.get("L"), property_payload(&none, 3, 0x80, &[]));
        // FillGrid(out int F[3]) sets F[2] = 9; from the host (as the sweep calls) its out value is the whole array.
        let fill = toy.script("FillGrid", &[Param::new("Int", "F")], None, |ids, a| {
            let f = ids.get("F");
            let_(
                a,
                |a| {
                    element(
                        a,
                        0x1A,
                        |a| {
                            a.int_const(2);
                        },
                        |a| {
                            a.local(f);
                        },
                    )
                },
                |a| {
                    a.int_const(9);
                },
            );
            a.end();
        });
        toy.t.p.set_payload(
            fill.get("F"),
            property_payload(&none, 3, 0x80 | CPF_OUT, &[]),
        );
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        assert_eq!(r.on(o, "Core.Foo.PassGrid", vec![]), int(12));
        assert_eq!(
            r.on(o, "Core.Foo.PassLocal", vec![ints_of(&[1, 2, 3])]),
            int(5)
        );
        // A value that is not an array of the parameter's size is NotYet, and leaves no frame behind.
        for bad in [int(4), ints_of(&[1, 2])] {
            assert!(matches!(
                r.call("Core.Foo.TakeGrid", Some(o), vec![bad]),
                Err(VmError::NotYet(_))
            ));
        }
        assert_eq!(r.on(o, "Core.Foo.PassGrid", vec![]), int(12));
        let filled = r
            .call("Core.Foo.FillGrid", Some(o), vec![ints_of(&[1, 2, 3])])
            .unwrap();
        assert_eq!(filled.outs, vec![ints_of(&[1, 2, 9])]);
    }

    /// A `None` given by value to a native's struct parameter (here an element read past the end of an empty dynamic
    /// array) reaches the native as the struct's zero; a script callee keeps the `None`.
    #[test]
    fn a_none_given_to_a_natives_struct_parameter_is_the_zero_struct() {
        let mut toy = Toy::new();
        let foo = toy.foo;
        let vec = toy.structure(
            "Vec",
            &[("Float", "X", 1), ("Float", "Y", 1), ("Float", "Z", 1)],
        );
        let vecs = toy_array_of(&mut toy.t, foo, "Vecs", "Struct", vec);
        let v = Param {
            type_ref: vec,
            ..Param::new("Struct", "V")
        };
        let native = toy.native(
            foo,
            "EchoVec",
            std::slice::from_ref(&v),
            Some(("Struct", vec)),
            0,
        );
        let script = toy.script(
            "ScriptVec",
            std::slice::from_ref(&v),
            Some(("Struct", vec)),
            |ids, a| {
                returns(a, |a| {
                    a.local(ids.get("V"));
                });
            },
        );
        let past_end = |a: &mut Asm| {
            element(
                a,
                0x10,
                |a| {
                    a.int_const(0);
                },
                |a| {
                    a.instance(vecs);
                },
            );
        };
        toy.script("ToNative", &[], Some(("Struct", vec)), |_, a| {
            returns(a, |a| {
                a.final_function(native.function, past_end);
            });
        });
        toy.script("ToScript", &[], Some(("Struct", vec)), |_, a| {
            returns(a, |a| {
                a.final_function(script.function, past_end);
            });
        });
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        let id = r.vm.find_function("Core.Foo.EchoVec").unwrap();
        let key = r.vm.function(id).unwrap().native_key.clone().unwrap();
        r.vm.register_native(
            &key,
            Rc::new(|c: &mut NativeCall<'_>| Ok(c.arg(0)?.clone())),
        )
        .unwrap();
        let zero = {
            let classes = &r.vm.space.classes;
            let s = classes.struct_by_path("Core.Object.Vec").unwrap();
            classes.zero_value(&ow_object::layout::PropKind::Struct(s))
        };
        assert!(zero.as_struct().is_some());
        assert_eq!(r.on(o, "Core.Foo.ToNative", vec![]), zero);
        assert!(
            r.vm.log
                .iter()
                .any(|l| l == "accessed array out of bounds: 0"),
            "{:?}",
            r.vm.log
        );
        assert_eq!(r.on(o, "Core.Foo.ToScript", vec![]), Value::None);
        // From the host the argument is as given.
        assert_eq!(
            r.call("Core.Foo.EchoVec", Some(o), vec![Value::None])
                .unwrap()
                .ret,
            Value::None
        );
        // A key kept by `Vm::keep_struct_none` is handed the `None` in Off (review r1 of #638, item 2), and only while
        // the VM is in Off: switched to Reference afterwards, the conversion is back (review r2 of #638, item 2).
        r.vm.keep_struct_none(&key);
        assert_eq!(r.on(o, "Core.Foo.ToNative", vec![]), Value::None);
        r.vm.parity = ParityMode::Reference;
        r.vm.rng = ow_core::rand::GameRng::from_world_seed(r.vm.rng.world_seed(), ParityMode::Reference);
        assert!(!r.vm.keeps_struct_none(&key));
        assert_eq!(r.on(o, "Core.Foo.ToNative", vec![]), zero);
    }

    /// A byte stored into an int slot of the heap is the int of the same number (the C++ keeps both kinds in one
    /// integer field), here added to an `array<int>`. The rule is that narrow: a byte for an `array<float>` is still refused.
    #[test]
    fn a_byte_stored_into_an_int_slot_is_that_int() {
        let mut toy = Toy::new();
        let foo = toy.foo;
        let nums = toy.array_prop(foo, "Nums", "Int");
        let reals = toy.array_prop(foo, "Reals", "Float");
        let add = |array: i32| {
            move |_: &FunctionIds, a: &mut Asm| {
                stmt(a, |a| {
                    add_item(
                        a,
                        |a| {
                            a.instance(array);
                        },
                        |a| {
                            a.byte_const(200);
                        },
                    )
                });
                a.end();
            }
        };
        toy.script("AddByte", &[], None, add(nums));
        toy.script("AddByteReal", &[], None, add(reals));
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        r.on(o, "Core.Foo.AddByte", vec![]);
        assert_eq!(r.prop(o, "Nums", 0), ints_of(&[200]));
        assert!(matches!(
            r.call("Core.Foo.AddByteReal", Some(o), vec![]),
            Err(VmError::Heap(_))
        ));
        assert_eq!(r.prop(o, "Reals", 0), Value::array(Vec::new()));
    }
}
