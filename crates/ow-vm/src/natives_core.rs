//! Owner: lane W3-CORENAT. The Core natives on the W2-VMAPI registry (architecture section 3.8, lane W3-CORENAT):
//! a port of `src/natives_core.cpp`. AI-assisted (Claude Code).
//!
//! [`register`] (or [`Pack`], a [`NativePack`]) binds every native of `natives_core.cpp` under the same key
//! (`Class.Name(type,...)`, as the loader derives it from a native's declaration) and the two engine iterators
//! (`Actor.AllActors`, `Actor.DynamicActors`). Each implementation follows the C++ line for line; where the C++
//! itself is a reading of the language or of public UE3 rather than of the game, its UNVERIFIED label is kept.
//!
//! **Numbers.** Ints are 32-bit and wrap (C++ `makeInt` keeps the low 32 bits of an `int64_t` result). Floats are
//! read as doubles and every result is rounded once to single precision, as C++ `makeFloat` does, so compound
//! expressions (`Lerp`, `FCubicInterp`, the vector natives) round where the C++ rounds. Powers, exponentials,
//! logarithms, roots and trigonometry go through the `ow_core::num` choke points. Float to int goes through
//! [`trunc_i64_wrap_i32`] wherever the C++ converts with `int64_t(double)` (`*=(int,float)`, `Round`, `FCeil`,
//! `FFloor`, the rotator natives). `std::min`, `std::max` and `std::clamp` are written out in their C++ form, so a NaN
//! argument gives what the C++ gives.
//!
//! **Arguments.** The C++ `Value` keeps Int, Bool and Byte in one integer field and String and Name in one text field,
//! so an int parameter here accepts Int, Byte and Bool, a text parameter Str and Name, and an omitted argument
//! (`Value::None`) reads as the zero of the parameter, as the C++ `Value()` does. Any other variant is
//! [`VmError::BadArgument`] (the C++ would read a zero field; the interpreter converts arguments before a native
//! call, so that is a caller's bug, reported).
//!
//! **Strings** are UTF-8 (`Value::Str`), as the C++ `std::string`. Concatenation, search, replace, split, case
//! mapping and comparison work on that text and give the C++ results byte for byte. The natives that count
//! characters (`Len`, `Left`, `Right`, `Mid`, `InStr`, `Asc`, `Chr`) count UTF-16 code units, the unit of the game's
//! `FString`: for ASCII text that is the C++ byte count, for other text the C++ would split a UTF-8 sequence, which a
//! Rust `str` cannot hold. A lone surrogate or a cut through a surrogate pair gives U+FFFD (a limit of `Value::Str`).
//! `Chr` takes the low 16 bits (the C++ takes the low 8). `Caps`/`Locs` map ASCII only, as C++ `toupper`/`tolower` in
//! the C locale. Status: UNVERIFIED for non-ASCII text (no live check of a non-ASCII string yet). Decision
//! 2026-11-15-07.
//!
//! **Random numbers.** `Rand`/`FRand` draw from `vm.rng.script(ScriptStream::Rand / FRand)`: the C++ static LCGs
//! (`VmLcg`, seeds 12345 / 54321) under `ParityMode::Reference`, a `CrtRand` stream of the world seed otherwise
//! (architecture section 6.1; the per-draw formulas are the public UE3 convention, UNVERIFIED, see `ow_core::rand`).
//!
//! **States.** `GotoState` reproduces the C++ name-and-events approximation only under `ParityMode::Reference` (a
//! stand-in, section 6.1, pinned by `tests/vm_states_test.py`), and there `IsInState` compares the name and `Enable`
//! and `Disable` do nothing, as in the C++. Otherwise the four hand over to `crate::states`, the game's state machine
//! as NATIVE_ENGINE_CORE.md reads it (W5-STATES, decision 2026-11-15-27), and [`register`] also binds the natives the
//! C++ leaves unbound (`PushState`, `PopState`, `Actor.Sleep`). Under Reference it reads the states a class declares and
//! the method lookup by class and state (C++ `Class::states`, `Runtime::findMethod`) from the classes the VM loaded:
//! [`register`] wires `Vm::class_declares_state` and `Vm::find_method` (W3-INTERP; a class defined by hand, with no
//! package behind it, declares no state). The algorithm itself is tested here over a stand-in lookup (decision
//! 2026-11-15-07).
//!
//! **Interfaces.** `QueryInterface` and the iterators' interface filter ask `Vm::implements` (the C++
//! `Runtime::implements`, a class's interface table), which W4-INTERP2 added and wired into [`register`].

use std::cell::Cell;
use std::rc::Rc;

use ow_core::NotYet;
use ow_core::ParityMode;
use ow_core::num::{
    acos_f64, asin_f64, atan_f64, atan2_f64, cos_f64, exp_f64, ln_f64, pow_f64, sin_f64, sqrt_f64,
    tan_f64, trunc_i64_wrap_i32,
};
use ow_core::rand::ScriptStream;
use ow_object::ids::{ClassId, FuncId, NameId, ObjRef};
use ow_object::value::{Value, same_value};

use crate::api::{IteratorResult, NativeCall, NativePack, Vm};
use crate::error::VmError;

/// The Core natives as a [`NativePack`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        register(vm)
    }
}

/// Which list an engine actor iterator asks the world for ([`ActorSource`]).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorWalk {
    /// `Actor.AllActors`.
    All,
    /// `Actor.DynamicActors`.
    Dynamic,
    /// `Actor.LocalPlayerControllers`: the engine's local players' controllers, in player index order (decision
    /// 2026-11-16-81).
    LocalPlayers,
}

/// The walk behind `Actor.AllActors` and `Actor.DynamicActors`: the actors a world's level lists give, in walk order,
/// before the class and interface filters.
pub type ActorWalkFn = Rc<dyn Fn(&Vm, ActorWalk) -> Result<Vec<ObjRef>, VmError>>;

/// The world's actor lists for the two engine iterators, a VM service (seam added for lane W5-WORLD, decision
/// 2026-11-15-26): `ow-world` adds one, and the iterators then visit what it walks instead of the heap's live
/// instances (the C++ list). The walk borrows nothing across the call; the service cell is borrowed only to clone
/// the function out.
pub struct ActorSource {
    walk: ActorWalkFn,
}

impl ActorSource {
    pub fn new(walk: ActorWalkFn) -> Self {
        ActorSource { walk }
    }
}

impl std::fmt::Debug for ActorSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorSource").finish_non_exhaustive()
    }
}

/// Registers every Core native and the two engine iterators on `vm`. A second call on one VM is refused (duplicate
/// registration).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    /// `GotoState`'s lookup over the loaded classes: `Vm::class_declares_state` and `Vm::find_method` (W3-INTERP,
    /// decision 2026-11-15-07).
    struct LoadedStates;
    impl StateLookup for LoadedStates {
        fn declares_state(&self, vm: &mut Vm, class: ClassId, state: NameId) -> Result<bool, VmError> {
            vm.class_declares_state(class, state)
        }
        fn find_method(
            &self,
            vm: &mut Vm,
            class: ClassId,
            name: &str,
            state: Option<NameId>,
        ) -> Result<Option<FuncId>, VmError> {
            vm.find_method(class, name, state)
        }
    }
    register_with(vm, Rc::new(LoadedStates))?;
    // Outside Reference only (W5-STATES): PushState, PopState and Actor.Sleep.
    crate::states::register(vm)?;
    crate::latent::register(vm)?;
    // Outside Reference only (decision 2026-11-16-81): the C++ has no local-player list and stubs the iterator.
    if vm.parity != ParityMode::Reference {
        vm.register_iterator("Actor.LocalPlayerControllers(class,object)", Rc::new(local_player_controllers))?;
    }
    Ok(())
}

/// `Actor.LocalPlayerControllers(BaseClass, out PC)` (NATIVE_ENGINE_CORE.md, "Actor.LocalPlayerControllers"): a None
/// `BaseClass` yields nothing; otherwise the engine's local players' controllers in player index order (the world's
/// [`ActorWalk::LocalPlayers`]), kept when their class is `BaseClass` or a child of it. The loop binds `PC` (index 1).
/// Without an [`ActorSource`] there is no engine and no local player list: `NotYet { LayerAbsent }`. The list is taken
/// when the loop starts (as `AllActors`).
fn local_player_controllers(c: &mut NativeCall<'_>) -> Result<IteratorResult, VmError> {
    let base = if c.has(0) { class(c, 0)? } else { None };
    let Some(base) = base else {
        return Ok(IteratorResult::new(1, Vec::new()));
    };
    let walk = match c.vm.service::<ActorSource>() {
        Some(cell) => cell.try_borrow().map_err(|_| VmError::ServiceBorrowed("ActorSource"))?.walk.clone(),
        None => {
            return Err(NotYet::layer_absent(
                "Actor.LocalPlayerControllers: the engine's local player list",
                "ow-world (an ActorSource service)",
                "docs/verification/NATIVE_ENGINE_CORE.md",
            )
            .into());
        }
    };
    let mut items = Vec::new();
    for o in walk(c.vm, ActorWalk::LocalPlayers)? {
        let own = c.vm.space.heap.get(o)?.class;
        if c.vm.space.classes.is_child_of(own, base) {
            items.push(Value::Object(Some(o)));
        }
    }
    Ok(IteratorResult::new(1, items))
}

// ---------------------------------------------------------------------------------------------------------------
// State lookup (GotoState)
// ---------------------------------------------------------------------------------------------------------------

/// What `GotoState` needs from the loaded classes: C++ `Class::states` and `Runtime::findMethod`.
trait StateLookup {
    /// Whether `class` or a super class declares the state `state`.
    fn declares_state(&self, vm: &mut Vm, class: ClassId, state: NameId) -> Result<bool, VmError>;
    /// The function `name` as an object of `class` in state `state` (`None`: no state) sees it: for each class from
    /// `class` up, the state's own function first, then the class's.
    fn find_method(
        &self,
        vm: &mut Vm,
        class: ClassId,
        name: &str,
        state: Option<NameId>,
    ) -> Result<Option<FuncId>, VmError>;
}

// ---------------------------------------------------------------------------------------------------------------
// Argument readers and value makers
// ---------------------------------------------------------------------------------------------------------------

fn bad(c: &NativeCall<'_>, index: usize, reason: &'static str) -> VmError {
    VmError::BadArgument {
        function: match c.vm.function(c.function) {
            Some(d) => d.path.as_str().to_string(),
            None => format!("{:?}", c.function),
        },
        index,
        reason,
    }
}

/// The C++ `.i` of argument `i`: Int, Byte and Bool share it; an omitted argument is 0.
fn int(c: &NativeCall<'_>, i: usize) -> Result<i32, VmError> {
    match c.arg(i)? {
        Value::Int(v) => Ok(*v),
        Value::Byte(v) => Ok(i32::from(*v)),
        Value::Bool(v) => Ok(i32::from(*v)),
        Value::None => Ok(0),
        _ => Err(bad(c, i, "expected an int, byte or bool")),
    }
}

/// The C++ `.f` of argument `i` (a double holding a float); an omitted argument is 0.
fn float(c: &NativeCall<'_>, i: usize) -> Result<f64, VmError> {
    match c.arg(i)? {
        Value::Float(v) => Ok(f64::from(*v)),
        Value::None => Ok(0.0),
        _ => Err(bad(c, i, "expected a float")),
    }
}

/// The C++ `.s` of argument `i`: a string's text or a name's; an omitted argument is empty.
fn text(c: &NativeCall<'_>, i: usize) -> Result<String, VmError> {
    match c.arg(i)? {
        Value::Str(s) => Ok(s.to_string()),
        Value::Name(n) => match c.vm.space.names.get(*n) {
            Some(t) => Ok(t.into_owned()),
            None => Err(bad(c, i, "a name from another name table")),
        },
        Value::None => Ok(String::new()),
        _ => Err(bad(c, i, "expected a string or a name")),
    }
}

/// The C++ `.truth()` of argument `i`.
fn truth(c: &NativeCall<'_>, i: usize) -> Result<bool, VmError> {
    Ok(c.arg(i)?.truth(&c.vm.space.names))
}

/// The C++ `.cls` of argument `i`: a class reference; the null object (the script's `None`) and an omitted argument
/// are no class.
fn class(c: &NativeCall<'_>, i: usize) -> Result<Option<ClassId>, VmError> {
    match c.arg(i)? {
        Value::Class(k) => Ok(*k),
        Value::Object(None) | Value::None => Ok(None),
        _ => Err(bad(c, i, "expected a class")),
    }
}

/// C++ `makeFloat`: rounded once to single precision.
fn fl(x: f64) -> Value {
    Value::Float(x as f32)
}

/// C++ `makeByte`: the low 8 bits.
fn byte(x: i64) -> Value {
    Value::Byte(x as u8)
}

/// C++ `std::min(a, b)`: `b < a ? b : a`.
fn cmin(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// C++ `std::max(a, b)`: `a < b ? b : a`.
fn cmax(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// C++ `std::clamp(v, lo, hi)`: `v < lo ? lo : hi < v ? hi : v`.
fn cclamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if hi < v {
        hi
    } else {
        v
    }
}

/// ASCII lower case, as C++ `tolower` in the C locale byte by byte (`lowered`).
fn lowered(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// The text of a name id of this VM.
fn name_text(vm: &Vm, n: NameId) -> String {
    vm.space
        .names
        .get(n)
        .map_or_else(String::new, |t| t.into_owned())
}

/// A rotator axis wrapped into a signed 16-bit turn (65536 units = 360 degrees), C++ `normalizeAxis`.
fn normalize_axis(angle: i64) -> i64 {
    let a = angle & 0xFFFF;
    if a > 32767 { a - 65536 } else { a }
}

/// The C++ `comp(v, name)` of argument `i`: the number in the struct field `field`.
fn comp(c: &NativeCall<'_>, i: usize, field: &str) -> Result<f64, VmError> {
    let Some(s) = c.arg(i)?.as_struct() else {
        return Err(bad(c, i, "expected a struct"));
    };
    let classes = &c.vm.space.classes;
    let layout = classes
        .struct_layout(s.ty)
        .ok_or_else(|| bad(c, i, "a struct type this VM does not know"))?;
    let entry =
        c.vm.space
            .names
            .find(field)
            .and_then(|n| layout.field(n))
            .map(|(entry, _)| entry)
            .ok_or_else(|| bad(c, i, "the struct lacks a field this native reads"))?;
    match s.fields.get(entry as usize) {
        Some(Value::Float(v)) => Ok(f64::from(*v)),
        Some(Value::Int(v)) => Ok(f64::from(*v)),
        Some(Value::Byte(v)) => Ok(f64::from(*v)),
        Some(Value::Bool(v)) => Ok(if *v { 1.0 } else { 0.0 }),
        _ => Err(bad(c, i, "a struct field that is not a number")),
    }
}

fn vec3(c: &NativeCall<'_>, i: usize) -> Result<[f64; 3], VmError> {
    Ok([comp(c, i, "X")?, comp(c, i, "Y")?, comp(c, i, "Z")?])
}

/// The rotator axes of argument `i` as C++ `int64_t(comp(..))`, kept to their low 32 bits: every rotator native
/// either wraps its result to 32 bits (`makeInt`) or keeps 16 bits, so the low 32 bits are all it uses.
fn rot3(c: &NativeCall<'_>, i: usize) -> Result<[i32; 3], VmError> {
    Ok([
        trunc_i64_wrap_i32(comp(c, i, "Pitch")?),
        trunc_i64_wrap_i32(comp(c, i, "Yaw")?),
        trunc_i64_wrap_i32(comp(c, i, "Roll")?),
    ])
}

/// C++ `zeroStruct(name)` with fields set: the struct at `path` at its zero value, then each named field.
fn make_struct(vm: &Vm, path: &'static str, fields: [(&str, Value); 3]) -> Result<Value, VmError> {
    let classes = &vm.space.classes;
    let layout = classes
        .struct_by_path(path)
        .and_then(|s| classes.struct_layout(s))
        .ok_or_else(|| {
            VmError::from(NotYet::layer_absent(
                format!("the {path} struct"),
                "ow-object reflection of the Core package",
                "lanes W2-REFLECT, W3-INTERP",
            ))
        })?;
    let mut value = layout.zero().clone();
    let Some(s) = value.struct_mut() else {
        return Err(VmError::Decode(format!(
            "{path}: the zero value is not a struct"
        )));
    };
    for (name, v) in fields {
        let entry = vm
            .space
            .names
            .find(name)
            .and_then(|n| layout.field(n))
            .map(|(entry, _)| entry as usize)
            .filter(|&e| e < s.fields.len())
            .ok_or_else(|| VmError::Decode(format!("{path} has no field {name}")))?;
        s.fields[entry] = v;
    }
    Ok(value)
}

/// C++ `makeVector`.
fn make_vector(vm: &Vm, [x, y, z]: [f64; 3]) -> Result<Value, VmError> {
    make_struct(
        vm,
        "Core.Object.Vector",
        [("X", fl(x)), ("Y", fl(y)), ("Z", fl(z))],
    )
}

/// C++ `makeRotator` (each axis wrapped to 32 bits by `makeInt`).
fn make_rotator(vm: &Vm, [p, y, r]: [i64; 3]) -> Result<Value, VmError> {
    make_struct(
        vm,
        "Core.Object.Rotator",
        [
            ("Pitch", Value::Int(p as i32)),
            ("Yaw", Value::Int(y as i32)),
            ("Roll", Value::Int(r as i32)),
        ],
    )
}

/// The C++ `Divide by zero` log line.
fn divide_by_zero(c: &mut NativeCall<'_>) {
    c.vm.log.push("Divide by zero".to_string());
}

/// Argument `i` as it stands (C++ `return c.out(i)`).
fn current(c: &NativeCall<'_>, i: usize) -> Result<Value, VmError> {
    Ok(c.arg(i)?.clone())
}

/// Writes out argument `i` and returns the new value (C++ `c.out(i) = v; return c.out(i)`).
fn put(c: &mut NativeCall<'_>, i: usize, v: Value) -> Result<Value, VmError> {
    c.set_out(i, v.clone())?;
    Ok(v)
}

/// Writes out argument `i` and returns its old value (C++ post-increment).
fn put_old(c: &mut NativeCall<'_>, i: usize, v: Value) -> Result<Value, VmError> {
    let old = current(c, i)?;
    c.set_out(i, v)?;
    Ok(old)
}

/// UTF-16 code units of `s` (see the module documentation on strings).
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn from_units(u: &[u16]) -> Value {
    Value::string(&String::from_utf16_lossy(u))
}

/// C++ `size_t(std::clamp<int64_t>(n, 0, len))`.
fn clamp_count(n: i32, len: usize) -> usize {
    usize::try_from(n).map_or(0, |n| n.min(len))
}

/// `std::string::find(needle, from)` on code units.
fn find_units(hay: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    if from > hay.len() {
        return None;
    }
    if needle.is_empty() {
        return Some(from);
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// `std::string::rfind(needle)` on code units.
fn rfind_units(hay: &[u16], needle: &[u16]) -> Option<usize> {
    if needle.is_empty() {
        return Some(hay.len());
    }
    hay.windows(needle.len()).rposition(|w| w == needle)
}

/// A count or position as C++ `makeInt(int64_t(n))`: the low 32 bits.
fn count_value(n: usize) -> Value {
    Value::Int(n as i64 as i32)
}

type Imp = fn(&mut NativeCall<'_>) -> Result<Value, VmError>;

fn reg(vm: &mut Vm, key: &str, f: Imp) -> Result<(), VmError> {
    vm.register_native(key, Rc::new(f))
}

// ---------------------------------------------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------------------------------------------

fn register_with(vm: &mut Vm, states: Rc<dyn StateLookup>) -> Result<(), VmError> {
    interfaces(vm)?;
    integers(vm)?;
    bytes(vm)?;
    floats(vm)?;
    booleans_and_names(vm)?;
    strings(vm)?;
    vectors(vm)?;
    rotators(vm)?;
    // Object.PathName (lane FIX-PHASELOCK-CHAIN, decision 2026-11-16-69).
    crate::path_name::register(vm)?;
    objects(vm, states)
}

fn interfaces(vm: &mut Vm) -> Result<(), VmError> {
    // Object.QueryInterface (NATIVE_CONTROLLER_HELPERS.md, UNVERIFIED): the object itself when its class is the
    // interface class or derives from it, or implements it (`Vm::implements`, the C++ Runtime::implements, wired by
    // W4-INTERP2); else None. A "no" through an engine-native stand-in class is NotYet (decision 2026-11-15-08), as
    // the casts answer it: `Vm::implements` is a plain bool and its callers apply that check.
    reg(vm, "Object.QueryInterface(class)", |c| {
        let want = class(c, 0)?;
        let (Some(this), Some(want)) = (c.this, want) else {
            return Ok(Value::Object(None));
        };
        let own = c.vm.space.heap.get(this)?.class;
        let yes = c.vm.space.classes.is_child_of(own, want) || c.vm.implements(own, want);
        if !yes {
            crate::interp::is_child(c.vm, own, want)?;
        }
        Ok(Value::Object(yes.then_some(this)))
    })
}

fn integers(vm: &mut Vm) -> Result<(), VmError> {
    reg(vm, "Object.+(int,int)", |c| {
        Ok(Value::Int(int(c, 0)?.wrapping_add(int(c, 1)?)))
    })?;
    reg(vm, "Object.-(int,int)", |c| {
        Ok(Value::Int(int(c, 0)?.wrapping_sub(int(c, 1)?)))
    })?;
    reg(vm, "Object.*(int,int)", |c| {
        Ok(Value::Int(int(c, 0)?.wrapping_mul(int(c, 1)?)))
    })?;
    reg(vm, "Object./(int,int)", |c| {
        let (a, b) = (int(c, 0)?, int(c, 1)?);
        if b == 0 {
            divide_by_zero(c);
            return Ok(Value::Int(0));
        }
        // In int64 (the C++ field), so i32::MIN / -1 is 2^31, wrapped back to i32::MIN.
        Ok(Value::Int(a.wrapping_div(b)))
    })?;
    reg(vm, "Object.%(int,int)", |c| {
        let (a, b) = (int(c, 0)?, int(c, 1)?);
        if b == 0 {
            divide_by_zero(c);
            return Ok(Value::Int(0));
        }
        Ok(Value::Int(a.wrapping_rem(b)))
    })?;
    reg(vm, "Object.&(int,int)", |c| {
        Ok(Value::Int(int(c, 0)? & int(c, 1)?))
    })?;
    reg(vm, "Object.|(int,int)", |c| {
        Ok(Value::Int(int(c, 0)? | int(c, 1)?))
    })?;
    reg(vm, "Object.^(int,int)", |c| {
        Ok(Value::Int(int(c, 0)? ^ int(c, 1)?))
    })?;
    reg(vm, "Object.<<(int,int)", |c| {
        Ok(Value::Int(
            ((int(c, 0)? as u32) << (int(c, 1)? & 31)) as i32,
        ))
    })?;
    reg(vm, "Object.>>(int,int)", |c| {
        Ok(Value::Int(int(c, 0)? >> (int(c, 1)? & 31)))
    })?;
    reg(vm, "Object.>>>(int,int)", |c| {
        Ok(Value::Int(
            ((int(c, 0)? as u32) >> (int(c, 1)? & 31)) as i32,
        ))
    })?;
    reg(vm, "Object.~_pre(int)", |c| Ok(Value::Int(!int(c, 0)?)))?;
    reg(vm, "Object.-_pre(int)", |c| {
        Ok(Value::Int(int(c, 0)?.wrapping_neg()))
    })?;
    reg(vm, "Object.<(int,int)", |c| {
        Ok(Value::Bool(int(c, 0)? < int(c, 1)?))
    })?;
    reg(vm, "Object.>(int,int)", |c| {
        Ok(Value::Bool(int(c, 0)? > int(c, 1)?))
    })?;
    reg(vm, "Object.<=(int,int)", |c| {
        Ok(Value::Bool(int(c, 0)? <= int(c, 1)?))
    })?;
    reg(vm, "Object.>=(int,int)", |c| {
        Ok(Value::Bool(int(c, 0)? >= int(c, 1)?))
    })?;
    reg(vm, "Object.==(int,int)", |c| {
        Ok(Value::Bool(int(c, 0)? == int(c, 1)?))
    })?;
    reg(vm, "Object.!=(int,int)", |c| {
        Ok(Value::Bool(int(c, 0)? != int(c, 1)?))
    })?;
    reg(vm, "Object.Min(int,int)", |c| {
        Ok(Value::Int(int(c, 0)?.min(int(c, 1)?)))
    })?;
    reg(vm, "Object.Max(int,int)", |c| {
        Ok(Value::Int(int(c, 0)?.max(int(c, 1)?)))
    })?;
    reg(vm, "Object.Clamp(int,int,int)", |c| {
        Ok(Value::Int(int(c, 0)?.max(int(c, 1)?).min(int(c, 2)?)))
    })?;
    reg(vm, "Object.NormalizeRotAxis(int)", |c| {
        Ok(Value::Int(normalize_axis(i64::from(int(c, 0)?)) as i32))
    })?;

    // Compound assignment and increment on out parameters.
    reg(vm, "Object.+=(int,int)", |c| {
        let v = int(c, 0)?.wrapping_add(int(c, 1)?);
        put(c, 0, Value::Int(v))
    })?;
    reg(vm, "Object.-=(int,int)", |c| {
        let v = int(c, 0)?.wrapping_sub(int(c, 1)?);
        put(c, 0, Value::Int(v))
    })?;
    reg(vm, "Object.*=(int,float)", |c| {
        let v = trunc_i64_wrap_i32(f64::from(int(c, 0)?) * float(c, 1)?);
        put(c, 0, Value::Int(v))
    })?;
    reg(vm, "Object./=(int,float)", |c| {
        let f = float(c, 1)?;
        if f == 0.0 {
            divide_by_zero(c);
            return current(c, 0);
        }
        let v = trunc_i64_wrap_i32(f64::from(int(c, 0)?) / f);
        put(c, 0, Value::Int(v))
    })?;
    reg(vm, "Object.++_pre(int)", |c| {
        let v = int(c, 0)?.wrapping_add(1);
        put(c, 0, Value::Int(v))
    })?;
    reg(vm, "Object.--_pre(int)", |c| {
        let v = int(c, 0)?.wrapping_sub(1);
        put(c, 0, Value::Int(v))
    })?;
    reg(vm, "Object.++(int)", |c| {
        let v = int(c, 0)?.wrapping_add(1);
        put_old(c, 0, Value::Int(v))
    })?;
    reg(vm, "Object.--(int)", |c| {
        let v = int(c, 0)?.wrapping_sub(1);
        put_old(c, 0, Value::Int(v))
    })
}

fn bytes(vm: &mut Vm) -> Result<(), VmError> {
    reg(vm, "Object.+=(byte,byte)", |c| {
        let v = byte(i64::from(int(c, 0)?) + i64::from(int(c, 1)?));
        put(c, 0, v)
    })?;
    reg(vm, "Object.-=(byte,byte)", |c| {
        let v = byte(i64::from(int(c, 0)?) - i64::from(int(c, 1)?));
        put(c, 0, v)
    })?;
    reg(vm, "Object.*=(byte,byte)", |c| {
        let v = byte(i64::from(int(c, 0)?) * i64::from(int(c, 1)?));
        put(c, 0, v)
    })?;
    reg(vm, "Object./=(byte,byte)", |c| {
        let b = int(c, 1)?;
        if b == 0 {
            divide_by_zero(c);
            return current(c, 0);
        }
        let v = byte(i64::from(int(c, 0)?) / i64::from(b));
        put(c, 0, v)
    })?;
    reg(vm, "Object.*=(byte,float)", |c| {
        // int64_t(double) then uint8_t: the low 8 bits, which the low 32 bits of the wrap keep.
        let v = byte(i64::from(trunc_i64_wrap_i32(
            f64::from(int(c, 0)?) * float(c, 1)?,
        )));
        put(c, 0, v)
    })?;
    reg(vm, "Object.++_pre(byte)", |c| {
        let v = byte(i64::from(int(c, 0)?) + 1);
        put(c, 0, v)
    })?;
    reg(vm, "Object.--_pre(byte)", |c| {
        let v = byte(i64::from(int(c, 0)?) - 1);
        put(c, 0, v)
    })?;
    reg(vm, "Object.++(byte)", |c| {
        let v = byte(i64::from(int(c, 0)?) + 1);
        put_old(c, 0, v)
    })?;
    reg(vm, "Object.--(byte)", |c| {
        let v = byte(i64::from(int(c, 0)?) - 1);
        put_old(c, 0, v)
    })
}

fn floats(vm: &mut Vm) -> Result<(), VmError> {
    reg(vm, "Object.+(float,float)", |c| {
        Ok(fl(float(c, 0)? + float(c, 1)?))
    })?;
    reg(vm, "Object.-(float,float)", |c| {
        Ok(fl(float(c, 0)? - float(c, 1)?))
    })?;
    reg(vm, "Object.*(float,float)", |c| {
        Ok(fl(float(c, 0)? * float(c, 1)?))
    })?;
    reg(vm, "Object./(float,float)", |c| {
        let (a, b) = (float(c, 0)?, float(c, 1)?);
        if b == 0.0 {
            divide_by_zero(c);
            return Ok(fl(0.0));
        }
        Ok(fl(a / b))
    })?;
    // `%` on f64 is C fmod (exact).
    reg(vm, "Object.%(float,float)", |c| {
        let (a, b) = (float(c, 0)?, float(c, 1)?);
        Ok(fl(if b == 0.0 { 0.0 } else { a % b }))
    })?;
    reg(vm, "Object.**(float,float)", |c| {
        Ok(fl(pow_f64(float(c, 0)?, float(c, 1)?)))
    })?;
    reg(vm, "Object.-_pre(float)", |c| Ok(fl(-float(c, 0)?)))?;
    reg(vm, "Object.<(float,float)", |c| {
        Ok(Value::Bool(float(c, 0)? < float(c, 1)?))
    })?;
    reg(vm, "Object.>(float,float)", |c| {
        Ok(Value::Bool(float(c, 0)? > float(c, 1)?))
    })?;
    reg(vm, "Object.<=(float,float)", |c| {
        Ok(Value::Bool(float(c, 0)? <= float(c, 1)?))
    })?;
    reg(vm, "Object.>=(float,float)", |c| {
        Ok(Value::Bool(float(c, 0)? >= float(c, 1)?))
    })?;
    reg(vm, "Object.==(float,float)", |c| {
        Ok(Value::Bool(float(c, 0)? == float(c, 1)?))
    })?;
    reg(vm, "Object.!=(float,float)", |c| {
        Ok(Value::Bool(float(c, 0)? != float(c, 1)?))
    })?;
    reg(vm, "Object.~=(float,float)", |c| {
        Ok(Value::Bool((float(c, 0)? - float(c, 1)?).abs() < 0.0001))
    })?;
    reg(vm, "Object.+=(float,float)", |c| {
        let v = fl(float(c, 0)? + float(c, 1)?);
        put(c, 0, v)
    })?;
    reg(vm, "Object.-=(float,float)", |c| {
        let v = fl(float(c, 0)? - float(c, 1)?);
        put(c, 0, v)
    })?;
    reg(vm, "Object.*=(float,float)", |c| {
        let v = fl(float(c, 0)? * float(c, 1)?);
        put(c, 0, v)
    })?;
    reg(vm, "Object./=(float,float)", |c| {
        let b = float(c, 1)?;
        if b == 0.0 {
            divide_by_zero(c);
            return current(c, 0);
        }
        let v = fl(float(c, 0)? / b);
        put(c, 0, v)
    })?;
    reg(vm, "Object.Abs(float)", |c| Ok(fl(float(c, 0)?.abs())))?;
    reg(vm, "Object.Sin(float)", |c| Ok(fl(sin_f64(float(c, 0)?))))?;
    reg(vm, "Object.Cos(float)", |c| Ok(fl(cos_f64(float(c, 0)?))))?;
    reg(vm, "Object.Tan(float)", |c| Ok(fl(tan_f64(float(c, 0)?))))?;
    reg(vm, "Object.Asin(float)", |c| Ok(fl(asin_f64(float(c, 0)?))))?;
    reg(vm, "Object.Acos(float)", |c| Ok(fl(acos_f64(float(c, 0)?))))?;
    reg(vm, "Object.Atan(float)", |c| Ok(fl(atan_f64(float(c, 0)?))))?;
    reg(vm, "Object.Atan2(float,float)", |c| {
        Ok(fl(atan2_f64(float(c, 0)?, float(c, 1)?)))
    })?;
    reg(vm, "Object.Exp(float)", |c| Ok(fl(exp_f64(float(c, 0)?))))?;
    reg(vm, "Object.Loge(float)", |c| Ok(fl(ln_f64(float(c, 0)?))))?;
    reg(vm, "Object.Sqrt(float)", |c| Ok(fl(sqrt_f64(float(c, 0)?))))?;
    reg(vm, "Object.Square(float)", |c| {
        let a = float(c, 0)?;
        Ok(fl(a * a))
    })?;
    reg(vm, "Object.Round(float)", |c| {
        Ok(Value::Int(trunc_i64_wrap_i32((float(c, 0)? + 0.5).floor())))
    })?;
    reg(vm, "Object.FCeil(float)", |c| {
        Ok(Value::Int(trunc_i64_wrap_i32(float(c, 0)?.ceil())))
    })?;
    reg(vm, "Object.FFloor(float)", |c| {
        Ok(Value::Int(trunc_i64_wrap_i32(float(c, 0)?.floor())))
    })?;
    reg(vm, "Object.FMin(float,float)", |c| {
        Ok(fl(cmin(float(c, 0)?, float(c, 1)?)))
    })?;
    reg(vm, "Object.FMax(float,float)", |c| {
        Ok(fl(cmax(float(c, 0)?, float(c, 1)?)))
    })?;
    reg(vm, "Object.FClamp(float,float,float)", |c| {
        Ok(fl(cmin(cmax(float(c, 0)?, float(c, 1)?), float(c, 2)?)))
    })?;
    reg(vm, "Object.Lerp(float,float,float)", |c| {
        let (a, b, alpha) = (float(c, 0)?, float(c, 1)?, float(c, 2)?);
        Ok(fl(a + alpha * (b - a)))
    })?;
    reg(
        vm,
        "Object.FCubicInterp(float,float,float,float,float)",
        |c| {
            // Cubic Hermite interpolation of P0/T0 to P1/T1 at A.
            let (p0, t0, p1, t1, a) = (
                float(c, 0)?,
                float(c, 1)?,
                float(c, 2)?,
                float(c, 3)?,
                float(c, 4)?,
            );
            let a2 = a * a;
            let a3 = a2 * a;
            Ok(fl((2.0 * a3 - 3.0 * a2 + 1.0) * p0
                + (a3 - 2.0 * a2 + a) * t0
                + (a3 - a2) * t1
                + (-2.0 * a3 + 3.0 * a2) * p1))
        },
    )?;
    reg(vm, "Object.FInterpTo(float,float,float,float)", |c| {
        let (current, target, delta, speed) =
            (float(c, 0)?, float(c, 1)?, float(c, 2)?, float(c, 3)?);
        if speed <= 0.0 {
            return Ok(fl(target));
        }
        let distance = target - current;
        if distance * distance < 1e-8 {
            return Ok(fl(target));
        }
        Ok(fl(current + distance * cclamp(delta * speed, 0.0, 1.0)))
    })?;
    // VInterpTo (granted to lane W6-SKILLS as a seam edit, review r1 of PR #342; NATIVE_PHASELOCK_TARGETING.md section
    // 5, read from the executable, not in the C++): the target when the speed is not positive or the squared distance
    // is below 0.0001; otherwise the current vector moved by `clamp(delta x speed, 0, 1)` of the difference. Each axis
    // in double and rounded once, as FInterpTo here; the native's own precision is UNVERIFIED (the bob it smooths was
    // matched live to RMS 0.001 uu). No C++ counterpart: a C++ sweep that reaches it logs a stub where this binds it.
    reg(vm, "Object.VInterpTo(vector,vector,float,float)", |c| {
        let (current, target) = (vec3(c, 0)?, vec3(c, 1)?);
        let (delta, speed) = (float(c, 2)?, float(c, 3)?);
        let d = [
            target[0] - current[0],
            target[1] - current[1],
            target[2] - current[2],
        ];
        if speed <= 0.0 || d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < 0.0001 {
            return make_vector(c.vm, target);
        }
        let alpha = cclamp(delta * speed, 0.0, 1.0);
        make_vector(
            c.vm,
            [
                current[0] + d[0] * alpha,
                current[1] + d[1] * alpha,
                current[2] + d[2] * alpha,
            ],
        )
    })?;
    // VLerp (lane FIX-PHASELOCK-CAST, decision 2026-11-19-02): the stock UE3 Core vector lerp, `A + Alpha x (B - A)`
    // per axis, from the public UnrealScript declaration and the float `Lerp` above; no record of the executable and
    // no C++ counterpart. Alpha is not clamped (as `Lerp`). Each axis in double and rounded once, as `Lerp` here; the
    // native's own single-precision order is UNVERIFIED. Bound in both modes with one rule.
    reg(vm, "Object.VLerp(vector,vector,float)", |c| {
        let (a, b, alpha) = (vec3(c, 0)?, vec3(c, 1)?, float(c, 2)?);
        make_vector(
            c.vm,
            [
                a[0] + alpha * (b[0] - a[0]),
                a[1] + alpha * (b[1] - a[1]),
                a[2] + alpha * (b[2] - a[2]),
            ],
        )
    })?;
    reg(
        vm,
        "Object.FInterpConstantTo(float,float,float,float)",
        |c| {
            let (current, target, delta, speed) =
                (float(c, 0)?, float(c, 1)?, float(c, 2)?, float(c, 3)?);
            let distance = target - current;
            if distance * distance < 1e-8 {
                return Ok(fl(target));
            }
            let step = speed * delta;
            Ok(fl(current + cclamp(distance, -step, step)))
        },
    )?;
    // The VM's script streams: VmLcg under ParityMode::Reference, CrtRand otherwise (ow_core::rand::GameRng).
    reg(vm, "Object.Rand(int)", |c| {
        let limit = int(c, 0)?;
        Ok(Value::Int(
            c.vm.rng.script(ScriptStream::Rand).rand_limit(limit),
        ))
    })?;
    reg(vm, "Object.FRand()", |c| {
        Ok(Value::Float(c.vm.rng.script(ScriptStream::FRand).frand()))
    })
}

fn booleans_and_names(vm: &mut Vm) -> Result<(), VmError> {
    reg(vm, "Object.!_pre(bool)", |c| Ok(Value::Bool(!truth(c, 0)?)))?;
    reg(vm, "Object.&&(bool,bool)", |c| {
        Ok(Value::Bool(truth(c, 0)? && truth(c, 1)?))
    })?;
    reg(vm, "Object.||(bool,bool)", |c| {
        Ok(Value::Bool(truth(c, 0)? || truth(c, 1)?))
    })?;
    reg(vm, "Object.^^(bool,bool)", |c| {
        Ok(Value::Bool(truth(c, 0)? != truth(c, 1)?))
    })?;
    reg(vm, "Object.==(bool,bool)", |c| {
        Ok(Value::Bool(truth(c, 0)? == truth(c, 1)?))
    })?;
    reg(vm, "Object.!=(bool,bool)", |c| {
        Ok(Value::Bool(truth(c, 0)? != truth(c, 1)?))
    })?;
    reg(vm, "Object.==(name,name)", |c| {
        Ok(Value::Bool(lowered(&text(c, 0)?) == lowered(&text(c, 1)?)))
    })?;
    reg(vm, "Object.!=(name,name)", |c| {
        Ok(Value::Bool(lowered(&text(c, 0)?) != lowered(&text(c, 1)?)))
    })?;
    reg(vm, "Object.==(object,object)", |c| {
        Ok(Value::Bool(same_value(c.arg(0)?, c.arg(1)?)))
    })?;
    reg(vm, "Object.!=(object,object)", |c| {
        Ok(Value::Bool(!same_value(c.arg(0)?, c.arg(1)?)))
    })?;
    reg(vm, "Object.==(interface,interface)", |c| {
        Ok(Value::Bool(same_value(c.arg(0)?, c.arg(1)?)))
    })?;
    reg(vm, "Object.!=(interface,interface)", |c| {
        Ok(Value::Bool(!same_value(c.arg(0)?, c.arg(1)?)))
    })
}

fn strings(vm: &mut Vm) -> Result<(), VmError> {
    reg(vm, "Object.$(string,string)", |c| {
        Ok(Value::string(&(text(c, 0)? + &text(c, 1)?)))
    })?;
    reg(vm, "Object.@(string,string)", |c| {
        Ok(Value::string(&(text(c, 0)? + " " + &text(c, 1)?)))
    })?;
    reg(vm, "Object.$=(string,string)", |c| {
        let v = Value::string(&(text(c, 0)? + &text(c, 1)?));
        put(c, 0, v)
    })?;
    reg(vm, "Object.@=(string,string)", |c| {
        let v = Value::string(&(text(c, 0)? + " " + &text(c, 1)?));
        put(c, 0, v)
    })?;
    reg(vm, "Object.-=(string,string)", |c| {
        // Removes every occurrence of the second string (UE3 definition, UNVERIFIED for overlapping matches): the
        // C++ loop, which searches again from the erase point.
        let mut s = text(c, 0)?;
        let cut = text(c, 1)?;
        if !cut.is_empty() {
            let mut from = 0;
            while let Some(at) = s[from..].find(&cut).map(|p| p + from) {
                s.replace_range(at..at + cut.len(), "");
                from = at;
            }
        }
        put(c, 0, Value::string(&s))
    })?;
    // Byte-wise comparison of the UTF-8 text, as std::string's.
    reg(vm, "Object.<(string,string)", |c| {
        Ok(Value::Bool(text(c, 0)? < text(c, 1)?))
    })?;
    reg(vm, "Object.>(string,string)", |c| {
        Ok(Value::Bool(text(c, 0)? > text(c, 1)?))
    })?;
    reg(vm, "Object.<=(string,string)", |c| {
        Ok(Value::Bool(text(c, 0)? <= text(c, 1)?))
    })?;
    reg(vm, "Object.>=(string,string)", |c| {
        Ok(Value::Bool(text(c, 0)? >= text(c, 1)?))
    })?;
    reg(vm, "Object.==(string,string)", |c| {
        Ok(Value::Bool(text(c, 0)? == text(c, 1)?))
    })?;
    reg(vm, "Object.!=(string,string)", |c| {
        Ok(Value::Bool(text(c, 0)? != text(c, 1)?))
    })?;
    reg(vm, "Object.~=(string,string)", |c| {
        Ok(Value::Bool(lowered(&text(c, 0)?) == lowered(&text(c, 1)?)))
    })?;
    reg(vm, "Object.Len(string)", |c| {
        Ok(count_value(units(&text(c, 0)?).len()))
    })?;
    reg(vm, "Object.Left(string,int)", |c| {
        let u = units(&text(c, 0)?);
        let n = clamp_count(int(c, 1)?, u.len());
        Ok(from_units(&u[..n]))
    })?;
    reg(vm, "Object.Right(string,int)", |c| {
        let u = units(&text(c, 0)?);
        let n = clamp_count(int(c, 1)?, u.len());
        Ok(from_units(&u[u.len() - n..]))
    })?;
    reg(vm, "Object.Mid(string,int,int)", |c| {
        let u = units(&text(c, 0)?);
        let start = clamp_count(int(c, 1)?, u.len());
        // An omitted count is the whole length; a negative one is 0; substr stops at the end.
        let count = if c.has(2) {
            usize::try_from(int(c, 2)?).unwrap_or(0)
        } else {
            u.len()
        };
        let end = start + count.min(u.len() - start);
        Ok(from_units(&u[start..end]))
    })?;
    reg(vm, "Object.Caps(string)", |c| {
        Ok(Value::string(&text(c, 0)?.to_ascii_uppercase()))
    })?;
    reg(vm, "Object.Locs(string)", |c| {
        Ok(Value::string(&lowered(&text(c, 0)?)))
    })?;
    reg(vm, "Object.Chr(int)", |c| {
        Ok(from_units(&[int(c, 0)? as u16]))
    })?;
    reg(vm, "Object.Asc(string)", |c| {
        Ok(Value::Int(
            units(&text(c, 0)?).first().map_or(0, |&u| i32::from(u)),
        ))
    })?;
    reg(vm, "Object.InStr(string,string,bool,bool,int)", |c| {
        let (mut hay, mut needle) = (text(c, 0)?, text(c, 1)?);
        let from_right = c.has(2) && truth(c, 2)?;
        let ignore_case = c.has(3) && truth(c, 3)?;
        let start = if c.has(4) { int(c, 4)? } else { 0 };
        if ignore_case {
            hay = lowered(&hay);
            needle = lowered(&needle);
        }
        let (hay, needle) = (units(&hay), units(&needle));
        let at = if from_right {
            rfind_units(&hay, &needle)
        } else {
            find_units(&hay, &needle, usize::try_from(start).unwrap_or(0))
        };
        Ok(at.map_or(Value::Int(-1), count_value))
    })?;
    reg(
        vm,
        "Object.ParseStringIntoArray(string,array,string,bool)",
        |c| {
            // Splits the text at every delimiter; bCullEmpty drops empty pieces. The result replaces the array.
            let source = text(c, 0)?;
            let delimiter = text(c, 2)?;
            let cull_empty = c.has(3) && truth(c, 3)?;
            let mut pieces = Vec::new();
            let mut add = |piece: &str| {
                if !cull_empty || !piece.is_empty() {
                    pieces.push(Value::string(piece));
                }
            };
            if delimiter.is_empty() {
                add(&source);
            } else {
                let mut from = 0;
                while let Some(at) = source[from..].find(&delimiter).map(|p| p + from) {
                    add(&source[from..at]);
                    from = at + delimiter.len();
                }
                add(&source[from..]);
            }
            let n = pieces.len();
            c.set_out(1, Value::array(pieces))?;
            Ok(count_value(n))
        },
    )?;
    reg(vm, "Object.Repl(string,string,string,bool)", |c| {
        let source = text(c, 0)?;
        let matched = text(c, 1)?;
        let with = text(c, 2)?;
        let case_sensitive = !c.has(3) || truth(c, 3)?;
        if matched.is_empty() {
            return Ok(Value::string(&source));
        }
        // ASCII case mapping keeps every byte offset, so positions found in the folded text cut the original.
        let (hay, needle) = if case_sensitive {
            (source.clone(), matched)
        } else {
            (lowered(&source), lowered(&matched))
        };
        let mut out = String::new();
        let mut from = 0;
        while let Some(at) = hay[from..].find(&needle).map(|p| p + from) {
            out.push_str(&source[from..at]);
            out.push_str(&with);
            from = at + needle.len();
        }
        out.push_str(&source[from..]);
        Ok(Value::string(&out))
    })
}

fn vectors(vm: &mut Vm) -> Result<(), VmError> {
    reg(vm, "Object.+(vector,vector)", |c| {
        let ([ax, ay, az], [bx, by, bz]) = (vec3(c, 0)?, vec3(c, 1)?);
        make_vector(c.vm, [ax + bx, ay + by, az + bz])
    })?;
    reg(vm, "Object.-(vector,vector)", |c| {
        let ([ax, ay, az], [bx, by, bz]) = (vec3(c, 0)?, vec3(c, 1)?);
        make_vector(c.vm, [ax - bx, ay - by, az - bz])
    })?;
    reg(vm, "Object.*(vector,vector)", |c| {
        let ([ax, ay, az], [bx, by, bz]) = (vec3(c, 0)?, vec3(c, 1)?);
        make_vector(c.vm, [ax * bx, ay * by, az * bz])
    })?;
    reg(vm, "Object.*(vector,float)", |c| {
        let ([x, y, z], s) = (vec3(c, 0)?, float(c, 1)?);
        make_vector(c.vm, [x * s, y * s, z * s])
    })?;
    reg(vm, "Object.*(float,vector)", |c| {
        let (s, [x, y, z]) = (float(c, 0)?, vec3(c, 1)?);
        make_vector(c.vm, [x * s, y * s, z * s])
    })?;
    reg(vm, "Object./(vector,float)", |c| {
        let ([x, y, z], f) = (vec3(c, 0)?, float(c, 1)?);
        let s = if f == 0.0 { 0.0 } else { 1.0 / f };
        make_vector(c.vm, [x * s, y * s, z * s])
    })?;
    reg(vm, "Object.-_pre(vector)", |c| {
        let [x, y, z] = vec3(c, 0)?;
        make_vector(c.vm, [-x, -y, -z])
    })?;
    reg(vm, "Object.==(vector,vector)", |c| {
        Ok(Value::Bool(same_value(c.arg(0)?, c.arg(1)?)))
    })?;
    reg(vm, "Object.!=(vector,vector)", |c| {
        Ok(Value::Bool(!same_value(c.arg(0)?, c.arg(1)?)))
    })?;
    reg(vm, "Object.Dot(vector,vector)", |c| {
        let ([ax, ay, az], [bx, by, bz]) = (vec3(c, 0)?, vec3(c, 1)?);
        Ok(fl(ax * bx + ay * by + az * bz))
    })?;
    reg(vm, "Object.Cross(vector,vector)", |c| {
        let ([ax, ay, az], [bx, by, bz]) = (vec3(c, 0)?, vec3(c, 1)?);
        make_vector(
            c.vm,
            [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx],
        )
    })?;
    reg(vm, "Object.VSize(vector)", |c| {
        let [x, y, z] = vec3(c, 0)?;
        Ok(fl(sqrt_f64(x * x + y * y + z * z)))
    })?;
    reg(vm, "Object.VSizeSq(vector)", |c| {
        let [x, y, z] = vec3(c, 0)?;
        Ok(fl(x * x + y * y + z * z))
    })?;
    reg(vm, "Object.Normal(vector)", |c| {
        let [x, y, z] = vec3(c, 0)?;
        let length = sqrt_f64(x * x + y * y + z * z);
        if length <= 1e-8 {
            return make_vector(c.vm, [0.0, 0.0, 0.0]);
        }
        make_vector(c.vm, [x / length, y / length, z / length])
    })?;
    reg(vm, "Object.IsZero(vector)", |c| {
        let [x, y, z] = vec3(c, 0)?;
        Ok(Value::Bool(x == 0.0 && y == 0.0 && z == 0.0))
    })?;
    reg(vm, "Object.+=(vector,vector)", |c| {
        let ([ax, ay, az], [bx, by, bz]) = (vec3(c, 0)?, vec3(c, 1)?);
        let v = make_vector(c.vm, [ax + bx, ay + by, az + bz])?;
        put(c, 0, v)
    })?;
    reg(vm, "Object.-=(vector,vector)", |c| {
        let ([ax, ay, az], [bx, by, bz]) = (vec3(c, 0)?, vec3(c, 1)?);
        let v = make_vector(c.vm, [ax - bx, ay - by, az - bz])?;
        put(c, 0, v)
    })?;
    reg(vm, "Object.*=(vector,float)", |c| {
        let ([x, y, z], s) = (vec3(c, 0)?, float(c, 1)?);
        let v = make_vector(c.vm, [x * s, y * s, z * s])?;
        put(c, 0, v)
    })?;
    reg(vm, "Object./=(vector,float)", |c| {
        let ([x, y, z], f) = (vec3(c, 0)?, float(c, 1)?);
        let s = if f == 0.0 { 0.0 } else { 1.0 / f };
        let v = make_vector(c.vm, [x * s, y * s, z * s])?;
        put(c, 0, v)
    })
}

fn rotators(vm: &mut Vm) -> Result<(), VmError> {
    fn same(a: [i32; 3], b: [i32; 3]) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x & 0xFFFF) == (y & 0xFFFF))
    }
    reg(vm, "Object.==(rotator,rotator)", |c| {
        Ok(Value::Bool(same(rot3(c, 0)?, rot3(c, 1)?)))
    })?;
    reg(vm, "Object.!=(rotator,rotator)", |c| {
        Ok(Value::Bool(!same(rot3(c, 0)?, rot3(c, 1)?)))
    })?;
    reg(vm, "Object.+(rotator,rotator)", |c| {
        let ([ap, ay, ar], [bp, by, br]) = (rot3(c, 0)?, rot3(c, 1)?);
        let w = |a: i32, b: i32| i64::from(a) + i64::from(b);
        make_rotator(c.vm, [w(ap, bp), w(ay, by), w(ar, br)])
    })?;
    reg(vm, "Object.-(rotator,rotator)", |c| {
        let ([ap, ay, ar], [bp, by, br]) = (rot3(c, 0)?, rot3(c, 1)?);
        let w = |a: i32, b: i32| i64::from(a) - i64::from(b);
        make_rotator(c.vm, [w(ap, bp), w(ay, by), w(ar, br)])
    })?;
    // int64_t(comp * s): the product of the double axis and the scale, truncated and wrapped.
    fn scaled(c: &NativeCall<'_>, r: usize, s: f64) -> Result<[i64; 3], VmError> {
        let t = |field| Ok::<_, VmError>(i64::from(trunc_i64_wrap_i32(comp(c, r, field)? * s)));
        Ok([t("Pitch")?, t("Yaw")?, t("Roll")?])
    }
    reg(vm, "Object.*(rotator,float)", |c| {
        let s = float(c, 1)?;
        let axes = scaled(c, 0, s)?;
        make_rotator(c.vm, axes)
    })?;
    reg(vm, "Object.*(float,rotator)", |c| {
        let s = float(c, 0)?;
        let axes = scaled(c, 1, s)?;
        make_rotator(c.vm, axes)
    })?;
    reg(vm, "Object.Normalize(rotator)", |c| {
        let [p, y, r] = rot3(c, 0)?;
        let n = |a: i32| normalize_axis(i64::from(a));
        make_rotator(c.vm, [n(p), n(y), n(r)])
    })?;

    // GetAxes / GetUnAxes: the rotator's three axes (X forward, Y right, Z up) and their inverse transform. The
    // rotation matrix is the standard UE3 one (UNVERIFIED against the game; yaw-only results are exact).
    fn axes(c: &mut NativeCall<'_>, inverse: bool) -> Result<Value, VmError> {
        let k = std::f64::consts::PI / 32768.0;
        let pitch = comp(c, 0, "Pitch")? * k;
        let yaw = comp(c, 0, "Yaw")? * k;
        let roll = comp(c, 0, "Roll")? * k;
        let (cp, sp) = (cos_f64(pitch), sin_f64(pitch));
        let (cy, sy) = (cos_f64(yaw), sin_f64(yaw));
        let (cr, sr) = (cos_f64(roll), sin_f64(roll));
        let mut m = [
            [cp * cy, cp * sy, sp],
            [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp],
            [-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp],
        ];
        if inverse {
            for (i, j) in [(0, 1), (0, 2), (1, 2)] {
                let t = m[i][j];
                m[i][j] = m[j][i];
                m[j][i] = t;
            }
        }
        for (row, axis) in m.into_iter().enumerate() {
            let v = make_vector(c.vm, axis)?;
            c.set_out(row + 1, v)?;
        }
        Ok(Value::None)
    }
    reg(vm, "Object.GetAxes(rotator,vector,vector,vector)", |c| {
        axes(c, false)
    })?;
    reg(vm, "Object.GetUnAxes(rotator,vector,vector,vector)", |c| {
        axes(c, true)
    })
}

fn objects(vm: &mut Vm, states: Rc<dyn StateLookup>) -> Result<(), VmError> {
    // Through an engine-native stand-in class the real chain is unknown: a "no" there is NotYet, not false
    // (crate::interp, decision 2026-11-15-08).
    reg(vm, "Object.IsA(name)", |c| {
        let Some(this) = c.this else {
            return Ok(Value::Bool(false));
        };
        let wanted = text(c, 0)?;
        let class = c.vm.space.heap.get(this)?.class;
        Ok(Value::Bool(crate::interp::is_a_named(c.vm, class, &wanted)?))
    })?;
    reg(vm, "Object.ClassIsChildOf(class,class)", |c| {
        let (a, b) = (class(c, 0)?, class(c, 1)?);
        Ok(Value::Bool(match (a, b) {
            (Some(a), Some(b)) => crate::interp::is_child(c.vm, a, b)?,
            _ => false,
        }))
    })?;

    // GotoState(NewState, Label, bForceEvents, bKeepStack) (parameter order read from the Core package by the C++).
    let depth = Rc::new(Cell::new(0u32));
    vm.register_native(
        "Object.GotoState(name,name,bool,bool)",
        Rc::new(move |c: &mut NativeCall<'_>| goto_state(c, &*states, &depth)),
    )?;

    // Engine iterators: Actor.AllActors(BaseClass, out Actor, optional InterfaceClass) and Actor.DynamicActors (same
    // parameters, read from the Engine package by the C++); the loop binds the out parameter (index 1).
    //
    // Without an [`ActorSource`] service (the C++ list): both visit the live instances of BaseClass or a child of it,
    // in creation order, and a None BaseClass matches nothing. UNVERIFIED (as in the C++): the real DynamicActors
    // visits only actors spawned at run time, here it is the same list as AllActors.
    //
    // With an [`ActorSource`] (lane W5-WORLD adds one): the candidates are the world's walk for that iterator, kept
    // when their class is BaseClass or a child of it; a None BaseClass keeps every candidate (NATIVE_ENGINE_CORE.md,
    // "Iterators": None means Actor, and the world's lists hold actors only).
    //
    // Either way the list is taken when the loop starts. The game's walk is live (NATIVE_ENGINE_CORE.md, read from
    // the executable): an actor added during the body is visited, and removing an earlier actor shifts the scan so
    // the next actor is skipped. The snapshot reproduces neither; it is this port's known divergence (the `foreach`
    // contract takes a list; decision 2026-11-15-26, a follow-up). An InterfaceClass keeps the objects whose class implements it
    // (`Vm::implements`, the C++ Runtime::implements, wired by W4-INTERP2); a "no" for an object whose class chain
    // runs through an engine-native stand-in class is NotYet (decision 2026-11-15-08). Only `implements` decides, as
    // in the C++; `is_child` is asked for that NotYet alone.
    fn actors(c: &mut NativeCall<'_>, walk: ActorWalk) -> Result<IteratorResult, VmError> {
        let base = if c.has(0) { class(c, 0)? } else { None };
        let interface = if c.has(2) { class(c, 2)? } else { None };
        let source = match c.vm.service::<ActorSource>() {
            Some(cell) => Some(
                cell.try_borrow()
                    .map_err(|_| VmError::ServiceBorrowed("ActorSource"))?
                    .walk
                    .clone(),
            ),
            None => None,
        };
        let candidates: Vec<ObjRef> = match source {
            Some(walk_fn) => {
                let listed = walk_fn(c.vm, walk)?;
                match base {
                    None => listed,
                    Some(b) => {
                        let mut kept = Vec::with_capacity(listed.len());
                        for o in listed {
                            let own = c.vm.space.heap.get(o)?.class;
                            if c.vm.space.classes.is_child_of(own, b) {
                                kept.push(o);
                            }
                        }
                        kept
                    }
                }
            }
            None => match base {
                Some(base) => c.vm.space.live(base).collect(),
                None => return Ok(IteratorResult::new(1, Vec::new())),
            },
        };
        let mut items = Vec::with_capacity(candidates.len());
        for o in candidates {
            let keep = match interface {
                None => true,
                Some(iface) => {
                    let own = c.vm.space.heap.get(o)?.class;
                    let yes = c.vm.implements(own, iface);
                    if !yes {
                        crate::interp::is_child(c.vm, own, iface)?;
                    }
                    yes
                }
            };
            if keep {
                items.push(Value::Object(Some(o)));
            }
        }
        Ok(IteratorResult::new(1, items))
    }
    vm.register_iterator(
        "Actor.AllActors(class,object,class)",
        Rc::new(|c: &mut NativeCall<'_>| actors(c, ActorWalk::All)),
    )?;
    vm.register_iterator(
        "Actor.DynamicActors(class,object,class)",
        Rc::new(|c: &mut NativeCall<'_>| actors(c, ActorWalk::Dynamic)),
    )?;

    reg(vm, "Object.GetStateName()", |c| {
        let state = match c.this {
            Some(this) => c.vm.space.heap.get(this)?.state,
            None => None,
        };
        Ok(Value::Name(state.unwrap_or(NameId::NONE)))
    })?;
    // Under Reference bTestStateStack is ignored (there is no state stack), as in the C++; otherwise crate::states.
    reg(vm, "Object.IsInState(name,bool)", |c| {
        if c.vm.parity != ParityMode::Reference {
            return crate::states::is_in_state_native(c);
        }
        let Some(this) = c.this else {
            return Ok(Value::Bool(false));
        };
        let state = c.vm.space.heap.get(this)?.state;
        let current = state.map_or_else(String::new, |s| name_text(c.vm, s));
        Ok(Value::Bool(lowered(&current) == lowered(&text(c, 0)?)))
    })?;
    // Under Reference probe enabling is not modelled (as in the C++); otherwise crate::states.
    reg(vm, "Object.Enable(name)", |c| {
        if c.vm.parity != ParityMode::Reference {
            return crate::states::enable_native(c, true);
        }
        Ok(Value::None)
    })?;
    reg(vm, "Object.Disable(name)", |c| {
        if c.vm.parity != ParityMode::Reference {
            return crate::states::enable_native(c, false);
        }
        Ok(Value::None)
    })?;
    reg(vm, "Object.LogInternal(string,name)", |c| {
        let line = format!("Log: {}", text(c, 0)?);
        c.vm.log.push(line);
        Ok(Value::None)
    })?;
    reg(vm, "Object.WarnInternal(string)", |c| {
        let line = format!("Warning: {}", text(c, 0)?);
        c.vm.log.push(line);
        Ok(Value::None)
    })
}

/// A GotoState inside an event is legitimate and finishes on its own; a chain this deep is a script that never
/// settles (C++ `MAX_NESTED_STATE_CHANGES`).
const MAX_NESTED_STATE_CHANGES: u32 = 8;

/// `Object.GotoState`. Under `ParityMode::Reference`, the C++ approximation (`natives_core.cpp:459-506`): the state
/// name is tracked so state-specific function overrides are selected, and the state events run in the public UE3
/// order (UNVERIFIED against the game): when the state changes, or bForceEvents is set, EndState(new state) runs on
/// the old state, then the name is set, then BeginState(old state) runs on the new one. A state name that no class
/// of the chain declares only sets the name (logged). Latent state code (labels, Sleep, FinishAnim) is not run: a
/// Label argument is logged and ignored, and bKeepStack is ignored. Otherwise the game's state machine
/// (`crate::states`, W5-STATES).
fn goto_state(
    c: &mut NativeCall<'_>,
    states: &dyn StateLookup,
    depth: &Cell<u32>,
) -> Result<Value, VmError> {
    if c.vm.parity != ParityMode::Reference {
        return crate::states::goto_state_native(c);
    }
    let Some(this) = c.this else {
        return Ok(Value::None);
    };
    if !c.has(0) {
        return Ok(Value::None);
    }
    let requested = text(c, 0)?;
    let to_default = lowered(&requested) == "none";
    let target = if to_default {
        None
    } else {
        Some(c.vm.space.names.intern(&requested))
    };
    let target_text = if to_default { String::new() } else { requested };
    let (class_id, previous) = {
        let object = c.vm.space.heap.get(this)?;
        (object.class, object.state)
    };
    let previous_text = previous.map_or_else(String::new, |s| name_text(c.vm, s));
    let force = c.has(2) && truth(c, 2)?;
    if c.has(1) {
        let label = text(c, 1)?;
        if lowered(&label) != "none" {
            c.vm.log.push(format!(
                "GotoState: label {label} not run (latent state code is not run)"
            ));
        }
    }
    if lowered(&previous_text) == lowered(&target_text) && !force {
        return Ok(Value::None);
    }
    if let Some(t) = target
        && !states.declares_state(c.vm, class_id, t)?
    {
        let class_name =
            c.vm.space
                .classes
                .get_layout(class_id)
                .map_or("?", |l| l.path().name())
                .to_string();
        c.vm.log.push(format!(
            "GotoState: state {target_text} is not declared on {class_name}; name set, no state events"
        ));
        c.vm.space.heap.get_mut(this)?.state = target;
        return Ok(Value::None);
    }
    if depth.get() >= MAX_NESTED_STATE_CHANGES {
        c.vm.log.push(format!(
            "GotoState: {target_text} abandoned, state events nested too deep"
        ));
        return Ok(Value::None);
    }
    depth.set(depth.get() + 1);
    let result = state_events(c, states, this, class_id, previous, target);
    depth.set(depth.get() - 1);
    result
}

/// The two state events of a GotoState that changes (or forces) the state.
fn state_events(
    c: &mut NativeCall<'_>,
    states: &dyn StateLookup,
    this: ObjRef,
    class_id: ClassId,
    previous: Option<NameId>,
    target: Option<NameId>,
) -> Result<Value, VmError> {
    let name_of = |s: Option<NameId>| Value::Name(s.unwrap_or(NameId::NONE));
    // The old state's EndState (the state's own, else the class-level event Object declares empty).
    if let Some(end) = states.find_method(c.vm, class_id, "EndState", previous)? {
        c.call_script(end, Some(this), vec![name_of(target)])?;
    }
    // The event itself changed state (a GotoState inside EndState): that change stands, this one is dropped.
    if c.vm.space.heap.get(this)?.state != previous {
        return Ok(Value::None);
    }
    c.vm.space.heap.get_mut(this)?.state = target;
    if let Some(begin) = states.find_method(c.vm, class_id, "BeginState", target)? {
        c.call_script(begin, Some(this), vec![name_of(previous)])?;
    }
    Ok(Value::None)
}

// ---------------------------------------------------------------------------------------------------------------
// Tests: one or more direct cases per native, read from the C++ source (`src/natives_core.cpp`), on invented classes
// and values (rule 9). Exact values are written by hand where the arithmetic is exact. Where a result depends on
// rounding or on the C runtime (the choke-point functions, pow, Lerp, a reciprocal, GetAxes), the expectation is an
// f32 bit pattern marked "C++ build": the C++ `ow-package` (Release, MSVC 14.50) confirmed it with `--run` on a
// synthetic package whose script functions return `Native(constants) == FloatConst(bits)`. Each returned true, and
// one ulp off returned false (review r1; the generator stays under ignored local/). Rand/FRand are the C++ values,
// confirmed by the same build and by a hand computation of the C++ LCG.
// ---------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use ow_attr::AttributeSystem;
    use ow_core::rand::{GameRng, ScriptRng, VmLcg};
    use ow_core::{Missing, ObjectPath};
    use ow_defs::DefStore;
    use ow_object::heap::ObjectSpace;
    use ow_object::layout::{PropKind, PropertyDecl};

    use super::*;
    use crate::api::{Arg, CallResult, FunctionDecl, NativeFn};

    struct H {
        vm: Vm,
        next: u32,
        object: ClassId,
        actor: ClassId,
        pawn: ClassId,
        vector: ow_object::ids::StructId,
        rotator: ow_object::ids::StructId,
    }

    /// Core.Object, Engine.Actor, Game.Pawn (an Actor), Core.Object.Vector (X, Y, Z: float) and Core.Object.Rotator
    /// (Pitch, Yaw, Roll: int); the Core natives registered with `states` as the GotoState lookup.
    fn build(mode: ParityMode, with_structs: bool, states: Option<Rc<dyn StateLookup>>) -> H {
        let mut space = ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let actor = space
            .classes
            .define("Engine.Actor", Some(object), vec![])
            .unwrap();
        let pawn = space
            .classes
            .define("Game.Pawn", Some(actor), vec![])
            .unwrap();
        let mut decls = |names: [&str; 3], kind: PropKind| -> Vec<PropertyDecl> {
            names
                .iter()
                .map(|n| PropertyDecl::new(space.names.intern(n), kind.clone()))
                .collect()
        };
        let v = decls(["X", "Y", "Z"], PropKind::Float);
        let r = decls(["Pitch", "Yaw", "Roll"], PropKind::Int);
        let (vector, rotator) = if with_structs {
            (
                space
                    .classes
                    .define_struct("Core.Object.Vector", None, v)
                    .unwrap(),
                space
                    .classes
                    .define_struct("Core.Object.Rotator", None, r)
                    .unwrap(),
            )
        } else {
            // Other paths, so the natives cannot find Core.Object.Vector / Rotator.
            (
                space
                    .classes
                    .define_struct("Test.Other.Vector", None, v)
                    .unwrap(),
                space
                    .classes
                    .define_struct("Test.Other.Rotator", None, r)
                    .unwrap(),
            )
        };
        let mut vm = Vm::new(
            space,
            DefStore::new_empty(),
            Rc::new(AttributeSystem::new_empty()),
            mode,
        );
        match states {
            Some(s) => register_with(&mut vm, s).unwrap(),
            None => register(&mut vm).unwrap(),
        }
        H {
            vm,
            next: 0,
            object,
            actor,
            pawn,
            vector,
            rotator,
        }
    }

    fn off() -> H {
        build(ParityMode::Off, true, None)
    }

    impl H {
        /// Declares a native function with `key` (parameters `out` where listed) and calls it.
        fn try_call(
            &mut self,
            key: &str,
            this: Option<ObjRef>,
            args: Vec<Value>,
            outs: &[usize],
        ) -> Result<CallResult, VmError> {
            let id = self.declare(
                key,
                args.len()
                    .max(outs.iter().map(|o| o + 1).max().unwrap_or(0)),
                outs,
            );
            self.vm.call(id, this, args)
        }

        /// A call of a function with `params` parameters and fewer arguments (the rest omitted).
        fn call_padded(&mut self, key: &str, params: usize, args: Vec<Value>) -> Value {
            let id = self.declare(key, params, &[]);
            self.vm.call(id, None, args).unwrap().ret
        }

        fn declare(&mut self, key: &str, params: usize, outs: &[usize]) -> FuncId {
            self.next += 1;
            let params = (0..params)
                .map(|i| {
                    let n = self.vm.space.names.intern(&format!("P{i}"));
                    let flags = PropertyDecl::PARAM
                        | if outs.contains(&i) {
                            PropertyDecl::OUT
                        } else {
                            0
                        };
                    PropertyDecl::new(n, PropKind::Int).with_flags(flags)
                })
                .collect();
            let path = ObjectPath::parse(&format!("Core.Object.N{}", self.next)).unwrap();
            self.vm
                .declare_function(FunctionDecl::native(path, key).with_params(params))
                .unwrap()
        }

        fn call(&mut self, key: &str, args: Vec<Value>) -> Value {
            self.try_call(key, None, args, &[]).unwrap().ret
        }

        /// A call with out parameters: (result, the arguments after the call).
        fn call_out(&mut self, key: &str, args: Vec<Value>, outs: &[usize]) -> (Value, Vec<Value>) {
            let r = self.try_call(key, None, args, outs).unwrap();
            (r.ret, r.outs)
        }

        fn call_on(&mut self, key: &str, this: ObjRef, args: Vec<Value>) -> Value {
            self.try_call(key, Some(this), args, &[]).unwrap().ret
        }

        fn name(&mut self, s: &str) -> Value {
            Value::Name(self.vm.space.names.intern(s))
        }

        fn vector(&self, x: f32, y: f32, z: f32) -> Value {
            Value::structure(
                self.vector,
                vec![Value::Float(x), Value::Float(y), Value::Float(z)],
            )
        }

        fn rotator(&self, p: i32, y: i32, r: i32) -> Value {
            Value::structure(
                self.rotator,
                vec![Value::Int(p), Value::Int(y), Value::Int(r)],
            )
        }

        fn log_has(&self, line: &str) -> bool {
            self.vm.log.iter().any(|l| l == line)
        }
    }

    fn i(v: i32) -> Value {
        Value::Int(v)
    }
    fn f(v: f32) -> Value {
        Value::Float(v)
    }
    fn b(v: bool) -> Value {
        Value::Bool(v)
    }
    fn s(v: &str) -> Value {
        Value::string(v)
    }
    /// A float from its bit pattern.
    fn fb(bits: u32) -> Value {
        Value::Float(f32::from_bits(bits))
    }
    fn fields(v: &Value) -> Vec<Value> {
        v.as_struct().unwrap().fields.clone()
    }
    fn not_yet(e: VmError) -> Missing {
        match e {
            VmError::NotYet(n) => n.missing,
            other => panic!("not a NotYet: {other:?}"),
        }
    }

    // ---- registration ----------------------------------------------------------------------------------------

    #[test]
    fn every_cpp_key_is_registered_and_a_second_registration_is_refused() {
        let mut h = off();
        for key in [
            "Object.QueryInterface(class)",
            "Object.+(int,int)",
            "Object.--(byte)",
            "Object.FRand()",
            "Object.!=(interface,interface)",
            "Object.Repl(string,string,string,bool)",
            "Object./=(vector,float)",
            "Object.GetUnAxes(rotator,vector,vector,vector)",
            "Object.GotoState(name,name,bool,bool)",
            "Object.WarnInternal(string)",
        ] {
            assert!(h.vm.has_native(key), "{key}");
        }
        let id = h.declare("Actor.AllActors(class,object,class)", 3, &[1]);
        assert!(h.vm.iterator_for(id).is_some());
        let id = h.declare("Actor.DynamicActors(class,object,class)", 3, &[1]);
        assert!(h.vm.iterator_for(id).is_some());
        assert!(matches!(
            register(&mut h.vm),
            Err(VmError::Registration { .. })
        ));
        assert!(matches!(
            Pack.register(&mut h.vm),
            Err(VmError::Registration { .. })
        ));
    }

    #[test]
    fn argument_kinds_follow_the_cpp_value_fields() {
        let mut h = off();
        // Int, Byte and Bool share the integer field; an omitted argument is 0.
        assert_eq!(
            h.call("Object.+(int,int)", vec![Value::Byte(200), b(true)]),
            i(201)
        );
        assert_eq!(h.call_padded("Object.+(int,int)", 2, vec![i(5)]), i(5));
        // A name is text to a string native; a float is not an int.
        let n = h.name("Abc");
        assert_eq!(h.call("Object.Len(string)", vec![n]), i(3));
        let e = h
            .try_call("Object.+(int,int)", None, vec![f(1.0), i(1)], &[])
            .unwrap_err();
        assert!(matches!(e, VmError::BadArgument { index: 0, .. }), "{e:?}");
        let e = h
            .try_call("Object.Sin(float)", None, vec![i(1)], &[])
            .unwrap_err();
        assert!(matches!(e, VmError::BadArgument { index: 0, .. }), "{e:?}");
    }

    // ---- integers --------------------------------------------------------------------------------------------

    #[test]
    fn integer_arithmetic_wraps_at_32_bits() {
        let mut h = off();
        assert_eq!(
            h.call("Object.+(int,int)", vec![i(i32::MAX), i(1)]),
            i(i32::MIN)
        );
        assert_eq!(
            h.call("Object.-(int,int)", vec![i(i32::MIN), i(1)]),
            i(i32::MAX)
        );
        assert_eq!(h.call("Object.*(int,int)", vec![i(65536), i(65536)]), i(0));
        assert_eq!(h.call("Object.*(int,int)", vec![i(-7), i(6)]), i(-42));
        assert_eq!(h.call("Object./(int,int)", vec![i(-7), i(2)]), i(-3));
        assert_eq!(
            h.call("Object./(int,int)", vec![i(i32::MIN), i(-1)]),
            i(i32::MIN)
        );
        assert_eq!(h.call("Object.%(int,int)", vec![i(-7), i(3)]), i(-1));
        assert_eq!(h.call("Object.%(int,int)", vec![i(i32::MIN), i(-1)]), i(0));
        assert!(h.vm.log.is_empty());
        assert_eq!(h.call("Object./(int,int)", vec![i(5), i(0)]), i(0));
        assert_eq!(h.call("Object.%(int,int)", vec![i(5), i(0)]), i(0));
        assert_eq!(h.vm.log, ["Divide by zero", "Divide by zero"]);
        assert_eq!(h.call("Object.-_pre(int)", vec![i(i32::MIN)]), i(i32::MIN));
        assert_eq!(h.call("Object.-_pre(int)", vec![i(4)]), i(-4));
        assert_eq!(h.call("Object.~_pre(int)", vec![i(0)]), i(-1));
    }

    #[test]
    fn integer_bits_and_shifts() {
        let mut h = off();
        assert_eq!(
            h.call("Object.&(int,int)", vec![i(0b1100), i(0b1010)]),
            i(0b1000)
        );
        assert_eq!(
            h.call("Object.|(int,int)", vec![i(0b1100), i(0b1010)]),
            i(0b1110)
        );
        assert_eq!(
            h.call("Object.^(int,int)", vec![i(0b1100), i(0b1010)]),
            i(0b0110)
        );
        // The shift count is taken mod 32.
        assert_eq!(h.call("Object.<<(int,int)", vec![i(1), i(33)]), i(2));
        assert_eq!(h.call("Object.<<(int,int)", vec![i(1), i(31)]), i(i32::MIN));
        assert_eq!(h.call("Object.>>(int,int)", vec![i(-8), i(1)]), i(-4));
        assert_eq!(h.call("Object.>>>(int,int)", vec![i(-8), i(28)]), i(15));
        assert_eq!(h.call("Object.>>>(int,int)", vec![i(-1), i(32)]), i(-1));
    }

    #[test]
    fn integer_comparisons_and_limits() {
        let mut h = off();
        let cases = [
            ("Object.<(int,int)", [false, false, true]),
            ("Object.>(int,int)", [false, true, false]),
            ("Object.<=(int,int)", [true, false, true]),
            ("Object.>=(int,int)", [true, true, false]),
            ("Object.==(int,int)", [true, false, false]),
            ("Object.!=(int,int)", [false, true, true]),
        ];
        for (key, want) in cases {
            for ((a, c), w) in [(2, 2), (3, 2), (-3, 2)].into_iter().zip(want) {
                assert_eq!(h.call(key, vec![i(a), i(c)]), b(w), "{key} {a} {c}");
            }
        }
        assert_eq!(h.call("Object.Min(int,int)", vec![i(3), i(-2)]), i(-2));
        assert_eq!(h.call("Object.Max(int,int)", vec![i(3), i(-2)]), i(3));
        assert_eq!(
            h.call("Object.Clamp(int,int,int)", vec![i(12), i(0), i(10)]),
            i(10)
        );
        assert_eq!(
            h.call("Object.Clamp(int,int,int)", vec![i(-1), i(0), i(10)]),
            i(0)
        );
        assert_eq!(
            h.call("Object.Clamp(int,int,int)", vec![i(4), i(0), i(10)]),
            i(4)
        );
        // min(max(v, lo), hi): with lo above hi, hi wins (as the C++ form gives).
        assert_eq!(
            h.call("Object.Clamp(int,int,int)", vec![i(4), i(10), i(0)]),
            i(0)
        );
        assert_eq!(
            h.call("Object.NormalizeRotAxis(int)", vec![i(40000)]),
            i(-25536)
        );
        assert_eq!(h.call("Object.NormalizeRotAxis(int)", vec![i(65541)]), i(5));
        assert_eq!(h.call("Object.NormalizeRotAxis(int)", vec![i(-1)]), i(-1));
        assert_eq!(
            h.call("Object.NormalizeRotAxis(int)", vec![i(32768)]),
            i(-32768)
        );
        assert_eq!(
            h.call("Object.NormalizeRotAxis(int)", vec![i(32767)]),
            i(32767)
        );
    }

    #[test]
    fn integer_compound_assignment_writes_the_out_parameter() {
        let mut h = off();
        assert_eq!(
            h.call_out("Object.+=(int,int)", vec![i(i32::MAX), i(2)], &[0]),
            (i(i32::MIN + 1), vec![i(i32::MIN + 1), i(2)])
        );
        assert_eq!(
            h.call_out("Object.-=(int,int)", vec![i(5), i(7)], &[0]).1[0],
            i(-2)
        );
        // int64_t(double(i) * f), then the 32-bit wrap: toward zero, 2^32 wraps to 0, NaN gives 0.
        assert_eq!(
            h.call_out("Object.*=(int,float)", vec![i(7), f(0.5)], &[0])
                .1[0],
            i(3)
        );
        assert_eq!(
            h.call_out("Object.*=(int,float)", vec![i(-7), f(0.5)], &[0])
                .1[0],
            i(-3)
        );
        assert_eq!(
            h.call_out("Object.*=(int,float)", vec![i(65536), f(65536.0)], &[0])
                .1[0],
            i(0)
        );
        assert_eq!(
            h.call_out("Object.*=(int,float)", vec![i(1), f(f32::NAN)], &[0])
                .1[0],
            i(0)
        );
        assert_eq!(
            h.call_out("Object./=(int,float)", vec![i(7), f(2.0)], &[0]),
            (i(3), vec![i(3), f(2.0)])
        );
        assert!(h.vm.log.is_empty());
        // By zero: logged, the out parameter unchanged and returned.
        assert_eq!(
            h.call_out("Object./=(int,float)", vec![i(7), f(0.0)], &[0]),
            (i(7), vec![i(7), f(0.0)])
        );
        assert_eq!(h.vm.log, ["Divide by zero"]);
        assert_eq!(
            h.call_out("Object.++_pre(int)", vec![i(i32::MAX)], &[0]),
            (i(i32::MIN), vec![i(i32::MIN)])
        );
        assert_eq!(
            h.call_out("Object.--_pre(int)", vec![i(0)], &[0]),
            (i(-1), vec![i(-1)])
        );
        // Post forms return the old value.
        assert_eq!(
            h.call_out("Object.++(int)", vec![i(4)], &[0]),
            (i(4), vec![i(5)])
        );
        assert_eq!(
            h.call_out("Object.--(int)", vec![i(4)], &[0]),
            (i(4), vec![i(3)])
        );
    }

    // ---- bytes -----------------------------------------------------------------------------------------------

    #[test]
    fn byte_operators_keep_the_low_8_bits() {
        let mut h = off();
        let by = Value::Byte;
        assert_eq!(
            h.call_out("Object.+=(byte,byte)", vec![by(250), by(10)], &[0]),
            (by(4), vec![by(4), by(10)])
        );
        assert_eq!(
            h.call_out("Object.-=(byte,byte)", vec![by(3), by(5)], &[0])
                .1[0],
            by(254)
        );
        assert_eq!(
            h.call_out("Object.*=(byte,byte)", vec![by(16), by(17)], &[0])
                .1[0],
            by(16)
        );
        assert_eq!(
            h.call_out("Object./=(byte,byte)", vec![by(200), by(7)], &[0])
                .1[0],
            by(28)
        );
        assert!(h.vm.log.is_empty());
        assert_eq!(
            h.call_out("Object./=(byte,byte)", vec![by(200), by(0)], &[0]),
            (by(200), vec![by(200), by(0)])
        );
        assert_eq!(h.vm.log, ["Divide by zero"]);
        assert_eq!(
            h.call_out("Object.*=(byte,float)", vec![by(100), f(3.0)], &[0])
                .1[0],
            by(44)
        );
        assert_eq!(
            h.call_out("Object.*=(byte,float)", vec![by(5), f(0.5)], &[0])
                .1[0],
            by(2)
        );
        assert_eq!(
            h.call_out("Object.++_pre(byte)", vec![by(255)], &[0]),
            (by(0), vec![by(0)])
        );
        assert_eq!(
            h.call_out("Object.--_pre(byte)", vec![by(0)], &[0]),
            (by(255), vec![by(255)])
        );
        assert_eq!(
            h.call_out("Object.++(byte)", vec![by(255)], &[0]),
            (by(255), vec![by(0)])
        );
        assert_eq!(
            h.call_out("Object.--(byte)", vec![by(0)], &[0]),
            (by(0), vec![by(255)])
        );
    }

    // ---- floats ----------------------------------------------------------------------------------------------

    #[test]
    fn float_arithmetic_rounds_once_to_single() {
        let mut h = off();
        // C++ build.
        assert_eq!(
            h.call("Object.+(float,float)", vec![f(0.1), f(0.2)]),
            fb(0x3e99_999a)
        );
        assert_eq!(
            h.call("Object.-(float,float)", vec![f(1.5), f(2.0)]),
            f(-0.5)
        );
        assert_eq!(
            h.call("Object.*(float,float)", vec![f(1.5), f(-2.0)]),
            f(-3.0)
        );
        assert_eq!(
            h.call("Object./(float,float)", vec![f(1.0), f(4.0)]),
            f(0.25)
        );
        assert!(h.vm.log.is_empty());
        assert_eq!(
            h.call("Object./(float,float)", vec![f(1.0), f(0.0)]),
            f(0.0)
        );
        assert_eq!(h.vm.log, ["Divide by zero"]);
        // fmod, and 0 (not logged) for a zero divisor.
        assert_eq!(
            h.call("Object.%(float,float)", vec![f(5.5), f(2.0)]),
            f(1.5)
        );
        assert_eq!(
            h.call("Object.%(float,float)", vec![f(-5.5), f(2.0)]),
            f(-1.5)
        );
        assert_eq!(
            h.call("Object.%(float,float)", vec![f(5.5), f(0.0)]),
            f(0.0)
        );
        assert_eq!(h.vm.log.len(), 1);
        assert_eq!(
            h.call("Object.**(float,float)", vec![f(2.0), f(10.0)]),
            f(1024.0)
        );
        // C++ build.
        assert_eq!(
            h.call("Object.**(float,float)", vec![f(1.7), f(0.3)]),
            fb(0x3f96_1669)
        );
        assert_eq!(h.call("Object.-_pre(float)", vec![f(2.5)]), f(-2.5));
        assert_eq!(h.call("Object.Square(float)", vec![f(-3.0)]), f(9.0));
        assert_eq!(h.call("Object.Abs(float)", vec![f(-3.25)]), f(3.25));
    }

    #[test]
    fn float_comparisons() {
        let mut h = off();
        let cases = [
            ("Object.<(float,float)", [false, false, true]),
            ("Object.>(float,float)", [false, true, false]),
            ("Object.<=(float,float)", [true, false, true]),
            ("Object.>=(float,float)", [true, true, false]),
            ("Object.==(float,float)", [true, false, false]),
            ("Object.!=(float,float)", [false, true, true]),
        ];
        for (key, want) in cases {
            for ((a, c), w) in [(2.0, 2.0), (3.0, 2.0), (-3.0, 2.0)].into_iter().zip(want) {
                assert_eq!(h.call(key, vec![f(a), f(c)]), b(w), "{key} {a} {c}");
            }
        }
        assert_eq!(
            h.call("Object.==(float,float)", vec![f(f32::NAN), f(f32::NAN)]),
            b(false)
        );
        assert_eq!(
            h.call("Object.!=(float,float)", vec![f(f32::NAN), f(f32::NAN)]),
            b(true)
        );
        assert_eq!(
            h.call("Object.~=(float,float)", vec![f(1.0), f(1.00005)]),
            b(true)
        );
        assert_eq!(
            h.call("Object.~=(float,float)", vec![f(1.0), f(1.001)]),
            b(false)
        );
    }

    #[test]
    fn float_compound_assignment() {
        let mut h = off();
        assert_eq!(
            h.call_out("Object.+=(float,float)", vec![f(1.5), f(2.0)], &[0]),
            (f(3.5), vec![f(3.5), f(2.0)])
        );
        assert_eq!(
            h.call_out("Object.-=(float,float)", vec![f(1.5), f(2.0)], &[0])
                .1[0],
            f(-0.5)
        );
        assert_eq!(
            h.call_out("Object.*=(float,float)", vec![f(1.5), f(2.0)], &[0])
                .1[0],
            f(3.0)
        );
        assert_eq!(
            h.call_out("Object./=(float,float)", vec![f(1.5), f(2.0)], &[0])
                .1[0],
            f(0.75)
        );
        assert!(h.vm.log.is_empty());
        assert_eq!(
            h.call_out("Object./=(float,float)", vec![f(1.5), f(0.0)], &[0]),
            (f(1.5), vec![f(1.5), f(0.0)])
        );
        assert_eq!(h.vm.log, ["Divide by zero"]);
    }

    #[test]
    fn float_functions_go_through_the_choke_points() {
        let mut h = off();
        // C++ build, every value at 0.7f32.
        let one = [
            ("Object.Sin(float)", 0x3f24_eb73),
            ("Object.Cos(float)", 0x3f43_ccb3),
            ("Object.Tan(float)", 0x3f57_a036),
            ("Object.Asin(float)", 0x3f46_8073),
            ("Object.Acos(float)", 0x3f4b_9f42),
            ("Object.Atan(float)", 0x3f1c_5889),
            ("Object.Exp(float)", 0x4000_e153),
            ("Object.Loge(float)", 0xbeb6_9e1a),
            ("Object.Sqrt(float)", 0x3f56_2f5a),
        ];
        for (key, bits) in one {
            assert_eq!(h.call(key, vec![f(0.7)]), fb(bits), "{key}");
        }
        // C++ build.
        assert_eq!(
            h.call("Object.Atan2(float,float)", vec![f(1.0), f(-2.0)]),
            fb(0x402b_6374)
        );
        assert_eq!(h.call("Object.Sqrt(float)", vec![f(16.0)]), f(4.0));
        // Out of the domain: NaN, as the C runtime gives.
        let Value::Float(nan) = h.call("Object.Sqrt(float)", vec![f(-1.0)]) else {
            panic!()
        };
        assert!(nan.is_nan());
    }

    #[test]
    fn float_to_int_rounding() {
        let mut h = off();
        // floor(f + 0.5): halves go up.
        assert_eq!(h.call("Object.Round(float)", vec![f(2.5)]), i(3));
        assert_eq!(h.call("Object.Round(float)", vec![f(-2.5)]), i(-2));
        assert_eq!(h.call("Object.Round(float)", vec![f(-2.6)]), i(-3));
        assert_eq!(h.call("Object.FCeil(float)", vec![f(1.2)]), i(2));
        assert_eq!(h.call("Object.FCeil(float)", vec![f(-1.2)]), i(-1));
        assert_eq!(h.call("Object.FFloor(float)", vec![f(-1.2)]), i(-2));
        // int64_t then the wrap: 2^32 + 2^31 wraps to i32::MIN; NaN gives 0.
        assert_eq!(
            h.call("Object.FFloor(float)", vec![f(6_442_450_944.0)]),
            i(i32::MIN)
        );
        assert_eq!(h.call("Object.FFloor(float)", vec![f(f32::NAN)]), i(0));
    }

    #[test]
    fn float_min_max_clamp_in_their_cpp_form() {
        let mut h = off();
        assert_eq!(
            h.call("Object.FMin(float,float)", vec![f(1.0), f(-2.0)]),
            f(-2.0)
        );
        assert_eq!(
            h.call("Object.FMax(float,float)", vec![f(1.0), f(-2.0)]),
            f(1.0)
        );
        // std::min(a, b) is b < a ? b : a, so a NaN first argument comes back; std::max likewise.
        let Value::Float(m) = h.call("Object.FMin(float,float)", vec![f(f32::NAN), f(1.0)]) else {
            panic!()
        };
        assert!(m.is_nan());
        assert_eq!(
            h.call("Object.FMin(float,float)", vec![f(1.0), f(f32::NAN)]),
            f(1.0)
        );
        assert_eq!(
            h.call("Object.FMax(float,float)", vec![f(1.0), f(f32::NAN)]),
            f(1.0)
        );
        assert_eq!(
            h.call(
                "Object.FClamp(float,float,float)",
                vec![f(5.0), f(0.0), f(1.0)]
            ),
            f(1.0)
        );
        assert_eq!(
            h.call(
                "Object.FClamp(float,float,float)",
                vec![f(-5.0), f(0.0), f(1.0)]
            ),
            f(0.0)
        );
        assert_eq!(
            h.call(
                "Object.FClamp(float,float,float)",
                vec![f(0.5), f(0.0), f(1.0)]
            ),
            f(0.5)
        );
    }

    #[test]
    fn interpolation() {
        let mut h = off();
        assert_eq!(
            h.call(
                "Object.Lerp(float,float,float)",
                vec![f(0.0), f(10.0), f(0.25)]
            ),
            f(2.5)
        );
        // Computed in double and rounded once. C++ build.
        assert_eq!(
            h.call(
                "Object.Lerp(float,float,float)",
                vec![f(0.1), f(0.7), f(0.3)]
            ),
            fb(0x3e8f_5c29)
        );
        let cubic = "Object.FCubicInterp(float,float,float,float,float)";
        assert_eq!(
            h.call(cubic, vec![f(1.0), f(5.0), f(3.0), f(7.0), f(0.0)]),
            f(1.0)
        );
        assert_eq!(
            h.call(cubic, vec![f(1.0), f(5.0), f(3.0), f(7.0), f(1.0)]),
            f(3.0)
        );
        // a = 0.5: 0.5*p0 + 0.125*t0 - 0.125*t1 + 0.5*p1 = 0.5 + 0.625 - 0.875 + 1.5.
        assert_eq!(
            h.call(cubic, vec![f(1.0), f(5.0), f(3.0), f(7.0), f(0.5)]),
            f(1.75)
        );

        let to = "Object.FInterpTo(float,float,float,float)";
        assert_eq!(
            h.call(to, vec![f(0.0), f(10.0), f(0.1), f(0.0)]),
            f(10.0),
            "no speed: the target"
        );
        assert_eq!(
            h.call(to, vec![f(0.0), f(0.00005), f(0.1), f(1.0)]),
            f(0.00005),
            "close enough: the target"
        );
        assert_eq!(h.call(to, vec![f(0.0), f(10.0), f(0.1), f(2.0)]), f(2.0));
        assert_eq!(
            h.call(to, vec![f(0.0), f(10.0), f(1.0), f(2.0)]),
            f(10.0),
            "alpha clamped to 1"
        );
        assert_eq!(
            h.call(to, vec![f(0.0), f(10.0), f(-1.0), f(2.0)]),
            f(0.0),
            "alpha clamped to 0"
        );

        let vto = "Object.VInterpTo(vector,vector,float,float)";
        let (zero, up) = (h.vector(0.0, 0.0, 0.0), h.vector(0.0, 0.0, 30.0));
        assert_eq!(
            h.call(vto, vec![zero.clone(), up.clone(), f(0.1), f(0.0)]),
            up,
            "no speed: the target"
        );
        let close = h.vector(0.0, 0.0, 0.009);
        assert_eq!(
            h.call(vto, vec![zero.clone(), close.clone(), f(0.1), f(1.0)]),
            close,
            "squared distance below 0.0001: the target"
        );
        assert_eq!(
            h.call(vto, vec![zero.clone(), up.clone(), f(0.1), f(1.0)]),
            h.vector(0.0, 0.0, 3.0)
        );
        assert_eq!(
            h.call(vto, vec![zero, up.clone(), f(5.0), f(1.0)]),
            up,
            "alpha clamped to 1"
        );
        assert_eq!(
            h.call(vto, vec![h.vector(1.0, 2.0, 3.0), up, f(-1.0), f(1.0)]),
            h.vector(1.0, 2.0, 3.0),
            "alpha clamped to 0"
        );

        let constant = "Object.FInterpConstantTo(float,float,float,float)";
        assert_eq!(
            h.call(constant, vec![f(0.0), f(10.0), f(0.5), f(4.0)]),
            f(2.0)
        );
        assert_eq!(
            h.call(constant, vec![f(0.0), f(-10.0), f(0.5), f(4.0)]),
            f(-2.0)
        );
        assert_eq!(
            h.call(constant, vec![f(0.0), f(1.0), f(0.5), f(4.0)]),
            f(1.0)
        );
        assert_eq!(
            h.call(constant, vec![f(3.0), f(3.00001), f(0.5), f(4.0)]),
            f(3.00001)
        );
        // A negative step: std::clamp's form (v < lo ? lo : hi < v ? hi : v) with lo = 2, hi = -2.
        assert_eq!(
            h.call(constant, vec![f(0.0), f(10.0), f(0.5), f(-4.0)]),
            f(-2.0)
        );
    }

    #[test]
    fn vlerp_is_one_rule_in_both_modes() {
        let key = "Object.VLerp(vector,vector,float)";
        for mode in [ParityMode::Off, ParityMode::Reference] {
            let mut h = build(mode, true, None);
            let (a, b) = (h.vector(0.0, 10.0, -4.0), h.vector(8.0, 30.0, 4.0));
            assert_eq!(
                h.call(key, vec![a.clone(), b.clone(), f(0.0)]),
                a,
                "{mode:?}: alpha 0 is A"
            );
            assert_eq!(
                h.call(key, vec![a.clone(), b.clone(), f(1.0)]),
                b,
                "{mode:?}: alpha 1 is B"
            );
            assert_eq!(
                h.call(key, vec![a.clone(), b.clone(), f(0.25)]),
                h.vector(2.0, 15.0, -2.0),
                "{mode:?}: per axis"
            );
            // Not clamped, as the float Lerp: 1.5 and -0.5 extrapolate past B and before A.
            assert_eq!(
                h.call(key, vec![a.clone(), b.clone(), f(1.5)]),
                h.vector(12.0, 40.0, 8.0),
                "{mode:?}: alpha above 1"
            );
            assert_eq!(
                h.call(key, vec![a.clone(), b.clone(), f(-0.5)]),
                h.vector(-4.0, 0.0, -8.0),
                "{mode:?}: alpha below 0"
            );
            // Each axis agrees with the float Lerp bit for bit (double, rounded once).
            let (p, q) = (h.vector(0.1, -3.3, 1.0e6), h.vector(0.7, 2.9, -7.5));
            let lerp = |h: &mut H, x: f32, y: f32| {
                h.call("Object.Lerp(float,float,float)", vec![f(x), f(y), f(0.3)])
            };
            let want = [
                lerp(&mut h, 0.1, 0.7),
                lerp(&mut h, -3.3, 2.9),
                lerp(&mut h, 1.0e6, -7.5),
            ];
            let want = match want {
                [Value::Float(x), Value::Float(y), Value::Float(z)] => h.vector(x, y, z),
                other => panic!("Lerp gave {other:?}"),
            };
            assert_eq!(
                h.call(key, vec![p, q, f(0.3)]),
                want,
                "{mode:?}: as Lerp per axis"
            );
        }
    }

    // ---- random numbers --------------------------------------------------------------------------------------

    #[test]
    fn rand_and_frand_reproduce_the_cpp_lcgs_under_reference() {
        let mut h = build(ParityMode::Reference, true, None);
        // C++ build and by hand: state 12345 -> 87628868, (>> 8) % 10 = 0; -> 71072467, (>> 8) % 1000 = 626. FRand:
        // 54321 -> 1238253532, (>> 8) / 2^24 = 0.28830331563949585 (f32 0x3e939c7e).
        assert_eq!(h.call("Object.Rand(int)", vec![i(10)]), i(0));
        assert_eq!(h.call("Object.Rand(int)", vec![i(1000)]), i(626));
        assert_eq!(h.call("Object.FRand()", vec![]), fb(0x3e93_9c7e));
        assert_eq!(
            f64::from(f32::from_bits(0x3e93_9c7e)),
            0.288_303_315_639_495_85
        );
        // Further draws agree with ow-core's VmLcg (limit 0 and below still step the state, as the C++ does).
        let mut h = build(ParityMode::Reference, true, None);
        let mut rand = VmLcg::new(VmLcg::RAND_SEED);
        let mut frand = VmLcg::new(VmLcg::FRAND_SEED);
        for limit in [10, 1, 0, -5, 1000, 7] {
            let want = rand.rand_limit(limit);
            assert_eq!(
                h.call("Object.Rand(int)", vec![i(limit)]),
                i(want),
                "limit {limit}"
            );
            assert_eq!(h.call("Object.FRand()", vec![]), f(frand.frand()));
        }
    }

    /// The replacement test of the VM LCG stand-in (architecture 6.1): outside Reference both natives draw from the
    /// world seed's CrtRand script streams.
    #[test]
    fn rand_and_frand_draw_from_the_crt_script_streams_otherwise() {
        let mut h = off();
        let mut streams = GameRng::from_world_seed(0, ParityMode::Off);
        for limit in [10, 32768, 0, 3] {
            let want = streams.script(ScriptStream::Rand).rand_limit(limit);
            assert_eq!(h.call("Object.Rand(int)", vec![i(limit)]), i(want));
            let want = streams.script(ScriptStream::FRand).frand();
            assert_eq!(h.call("Object.FRand()", vec![]), f(want));
        }
        // Another world seed, another sequence.
        let mut seeded = off();
        seeded.vm.rng = GameRng::from_world_seed(77, ParityMode::Off);
        let mut streams = GameRng::from_world_seed(77, ParityMode::Off);
        let want = streams.script(ScriptStream::Rand).rand_limit(1000);
        assert_eq!(seeded.call("Object.Rand(int)", vec![i(1000)]), i(want));
    }

    // ---- booleans, names, references -------------------------------------------------------------------------

    #[test]
    fn boolean_operators_use_truth() {
        let mut h = off();
        assert_eq!(h.call("Object.!_pre(bool)", vec![b(false)]), b(true));
        assert_eq!(h.call("Object.!_pre(bool)", vec![i(2)]), b(false));
        for (x, y) in [(false, false), (false, true), (true, false), (true, true)] {
            assert_eq!(h.call("Object.&&(bool,bool)", vec![b(x), b(y)]), b(x && y));
            assert_eq!(h.call("Object.||(bool,bool)", vec![b(x), b(y)]), b(x || y));
            assert_eq!(h.call("Object.^^(bool,bool)", vec![b(x), b(y)]), b(x != y));
            assert_eq!(h.call("Object.==(bool,bool)", vec![b(x), b(y)]), b(x == y));
            assert_eq!(h.call("Object.!=(bool,bool)", vec![b(x), b(y)]), b(x != y));
        }
    }

    #[test]
    fn names_compare_without_case_and_references_by_identity() {
        let mut h = off();
        let (a, a2, c) = (h.name("Alpha"), Value::string("ALPHA"), h.name("Beta"));
        assert_eq!(h.call("Object.==(name,name)", vec![a.clone(), a2]), b(true));
        assert_eq!(
            h.call("Object.==(name,name)", vec![a.clone(), c.clone()]),
            b(false)
        );
        assert_eq!(h.call("Object.!=(name,name)", vec![a, c]), b(true));
        let p = h.vm.instantiate(h.pawn, "P").unwrap();
        let q = h.vm.instantiate(h.pawn, "Q").unwrap();
        let (op, oq, none) = (
            Value::Object(Some(p)),
            Value::Object(Some(q)),
            Value::Object(None),
        );
        assert_eq!(
            h.call("Object.==(object,object)", vec![op.clone(), op.clone()]),
            b(true)
        );
        assert_eq!(
            h.call("Object.==(object,object)", vec![op.clone(), oq.clone()]),
            b(false)
        );
        assert_eq!(
            h.call("Object.!=(object,object)", vec![op.clone(), none.clone()]),
            b(true)
        );
        assert_eq!(
            h.call("Object.==(object,object)", vec![none.clone(), Value::None]),
            b(true)
        );
        let (ip, iq) = (Value::Interface(Some(p)), Value::Interface(Some(q)));
        assert_eq!(
            h.call(
                "Object.==(interface,interface)",
                vec![ip.clone(), ip.clone()]
            ),
            b(true)
        );
        assert_eq!(
            h.call("Object.!=(interface,interface)", vec![ip, iq]),
            b(true)
        );
    }

    // ---- strings ---------------------------------------------------------------------------------------------

    #[test]
    fn string_concatenation_and_removal() {
        let mut h = off();
        assert_eq!(
            h.call("Object.$(string,string)", vec![s("ab"), s("cd")]),
            s("abcd")
        );
        assert_eq!(
            h.call("Object.@(string,string)", vec![s("ab"), s("cd")]),
            s("ab cd")
        );
        assert_eq!(
            h.call_out("Object.$=(string,string)", vec![s("ab"), s("cd")], &[0]),
            (s("abcd"), vec![s("abcd"), s("cd")])
        );
        assert_eq!(
            h.call_out("Object.@=(string,string)", vec![s("ab"), s("cd")], &[0])
                .1[0],
            s("ab cd")
        );
        // A name in the out parameter becomes a string (C++ sets the kind).
        let n = h.name("Nm");
        assert_eq!(
            h.call_out("Object.$=(string,string)", vec![n, s("!")], &[0])
                .1[0],
            s("Nm!")
        );
        let cut = "Object.-=(string,string)";
        assert_eq!(
            h.call_out(cut, vec![s("banana"), s("an")], &[0]).1[0],
            s("ba")
        );
        assert_eq!(h.call_out(cut, vec![s("aaa"), s("aa")], &[0]).1[0], s("a"));
        // The search resumes at the erase point, so a match formed across it (here the "ab" made of the first "a" and
        // the last "b") is not removed: "aabb" gives "ab".
        assert_eq!(
            h.call_out(cut, vec![s("aabb"), s("ab")], &[0]).1[0],
            s("ab")
        );
        assert_eq!(h.call_out(cut, vec![s("abc"), s("")], &[0]).1[0], s("abc"));
    }

    #[test]
    fn string_comparisons() {
        let mut h = off();
        // Byte order: upper case sorts before lower case.
        assert_eq!(
            h.call("Object.<(string,string)", vec![s("B"), s("a")]),
            b(true)
        );
        assert_eq!(
            h.call("Object.>(string,string)", vec![s("b"), s("a")]),
            b(true)
        );
        assert_eq!(
            h.call("Object.<=(string,string)", vec![s("a"), s("a")]),
            b(true)
        );
        assert_eq!(
            h.call("Object.>=(string,string)", vec![s("a"), s("ab")]),
            b(false)
        );
        assert_eq!(
            h.call("Object.==(string,string)", vec![s("a"), s("A")]),
            b(false)
        );
        assert_eq!(
            h.call("Object.!=(string,string)", vec![s("a"), s("A")]),
            b(true)
        );
        assert_eq!(
            h.call("Object.~=(string,string)", vec![s("aBc"), s("AbC")]),
            b(true)
        );
        assert_eq!(
            h.call("Object.~=(string,string)", vec![s("abc"), s("abd")]),
            b(false)
        );
    }

    #[test]
    fn string_slicing_counts_utf16_units() {
        let mut h = off();
        assert_eq!(h.call("Object.Len(string)", vec![s("hello")]), i(5));
        assert_eq!(
            h.call("Object.Len(string)", vec![s("h\u{e9}llo")]),
            i(5),
            "one unit, two UTF-8 bytes"
        );
        assert_eq!(
            h.call("Object.Len(string)", vec![s("\u{1F600}")]),
            i(2),
            "a surrogate pair"
        );
        assert_eq!(
            h.call("Object.Left(string,int)", vec![s("hello"), i(2)]),
            s("he")
        );
        assert_eq!(
            h.call("Object.Left(string,int)", vec![s("hello"), i(-1)]),
            s("")
        );
        assert_eq!(
            h.call("Object.Left(string,int)", vec![s("hello"), i(99)]),
            s("hello")
        );
        assert_eq!(
            h.call("Object.Right(string,int)", vec![s("hello"), i(3)]),
            s("llo")
        );
        assert_eq!(
            h.call("Object.Right(string,int)", vec![s("hello"), i(-3)]),
            s("")
        );
        assert_eq!(
            h.call("Object.Right(string,int)", vec![s("hello"), i(9)]),
            s("hello")
        );
        assert_eq!(
            h.call("Object.Left(string,int)", vec![s("h\u{e9}llo"), i(2)]),
            s("h\u{e9}")
        );
        assert_eq!(
            h.call("Object.Left(string,int)", vec![s("\u{1F600}"), i(1)]),
            s("\u{FFFD}")
        );
        let mid = "Object.Mid(string,int,int)";
        assert_eq!(h.call(mid, vec![s("hello"), i(1), i(3)]), s("ell"));
        assert_eq!(
            h.call(mid, vec![s("hello"), i(1)]),
            s("ello"),
            "an omitted count: to the end"
        );
        assert_eq!(h.call(mid, vec![s("hello"), i(-4), i(2)]), s("he"));
        assert_eq!(h.call(mid, vec![s("hello"), i(3), i(99)]), s("lo"));
        assert_eq!(h.call(mid, vec![s("hello"), i(9), i(2)]), s(""));
        assert_eq!(h.call(mid, vec![s("hello"), i(1), i(-2)]), s(""));
    }

    #[test]
    fn string_case_and_characters() {
        let mut h = off();
        // A known limit of Value::Str (decision 2026-11-15-07): a lone surrogate cannot be held, so the two halves of
        // a pair made with Chr come back as two U+FFFD, not as the pair.
        let (hi, lo) = (
            h.call("Object.Chr(int)", vec![i(0xD83D)]),
            h.call("Object.Chr(int)", vec![i(0xDE00)]),
        );
        assert_eq!(
            h.call("Object.$(string,string)", vec![hi, lo]),
            s("\u{FFFD}\u{FFFD}")
        );
        assert_eq!(
            h.call("Object.Caps(string)", vec![s("aB1\u{e9}")]),
            s("AB1\u{e9}"),
            "ASCII only"
        );
        assert_eq!(
            h.call("Object.Locs(string)", vec![s("aB1\u{c9}")]),
            s("ab1\u{c9}")
        );
        assert_eq!(h.call("Object.Chr(int)", vec![i(65)]), s("A"));
        assert_eq!(
            h.call("Object.Chr(int)", vec![i(0x1_0041)]),
            s("A"),
            "the low 16 bits"
        );
        assert_eq!(h.call("Object.Chr(int)", vec![i(0)]), s("\0"));
        assert_eq!(h.call("Object.Asc(string)", vec![s("Abc")]), i(65));
        assert_eq!(h.call("Object.Asc(string)", vec![s("")]), i(0));
        assert_eq!(h.call("Object.Asc(string)", vec![s("\u{e9}")]), i(0xE9));
    }

    #[test]
    fn instr_searches_like_std_string() {
        let mut h = off();
        let key = "Object.InStr(string,string,bool,bool,int)";
        assert_eq!(h.call(key, vec![s("abcabc"), s("bc")]), i(1));
        assert_eq!(h.call(key, vec![s("abcabc"), s("x")]), i(-1));
        assert_eq!(h.call(key, vec![s("abcabc"), s("BC")]), i(-1));
        assert_eq!(
            h.call(key, vec![s("abcabc"), s("BC"), b(false), b(true)]),
            i(1),
            "ignore case"
        );
        assert_eq!(
            h.call(key, vec![s("abcabc"), s("bc"), b(true)]),
            i(4),
            "from the right"
        );
        assert_eq!(
            h.call(key, vec![s("abcabc"), s("bc"), b(false), b(false), i(2)]),
            i(4)
        );
        assert_eq!(
            h.call(key, vec![s("abcabc"), s("bc"), b(false), b(false), i(-3)]),
            i(1)
        );
        assert_eq!(
            h.call(key, vec![s("abc"), s("c"), b(false), b(false), i(9)]),
            i(-1)
        );
        assert_eq!(
            h.call(key, vec![s("abc"), s(""), b(false), b(false), i(3)]),
            i(3),
            "empty needle at the start"
        );
        assert_eq!(
            h.call(key, vec![s("abc"), s(""), b(false), b(false), i(4)]),
            i(-1)
        );
        assert_eq!(h.call(key, vec![s("abc"), s(""), b(true)]), i(3));
        // The start position is ignored from the right, as in the C++.
        assert_eq!(
            h.call(key, vec![s("abcabc"), s("bc"), b(true), b(false), i(5)]),
            i(4)
        );
        assert_eq!(
            h.call(key, vec![s("\u{e9}bc"), s("bc")]),
            i(1),
            "a position in UTF-16 units"
        );
    }

    #[test]
    fn parse_string_into_array_and_repl() {
        let mut h = off();
        let key = "Object.ParseStringIntoArray(string,array,string,bool)";
        let empty = Value::array(vec![s("old")]);
        let (n, outs) = h.call_out(key, vec![s("a,,b"), empty.clone(), s(",")], &[1]);
        assert_eq!(
            (n, &outs[1]),
            (i(3), &Value::array(vec![s("a"), s(""), s("b")]))
        );
        let (n, outs) = h.call_out(key, vec![s("a,,b,"), empty.clone(), s(","), b(true)], &[1]);
        assert_eq!((n, &outs[1]), (i(2), &Value::array(vec![s("a"), s("b")])));
        let (n, outs) = h.call_out(key, vec![s("a,b"), empty.clone(), s("")], &[1]);
        assert_eq!((n, &outs[1]), (i(1), &Value::array(vec![s("a,b")])));
        let (n, outs) = h.call_out(key, vec![s(""), empty.clone(), s(","), b(true)], &[1]);
        assert_eq!((n, &outs[1]), (i(0), &Value::array(vec![])));
        let (n, outs) = h.call_out(key, vec![s("x--y"), empty, s("--")], &[1]);
        assert_eq!((n, &outs[1]), (i(2), &Value::array(vec![s("x"), s("y")])));

        let repl = "Object.Repl(string,string,string,bool)";
        assert_eq!(h.call(repl, vec![s("a-b-c"), s("-"), s("+")]), s("a+b+c"));
        assert_eq!(
            h.call(repl, vec![s("aXbx"), s("x"), s("_")]),
            s("aXb_"),
            "case-sensitive by default"
        );
        assert_eq!(
            h.call(repl, vec![s("aXbx"), s("x"), s("_"), b(false)]),
            s("a_b_")
        );
        assert_eq!(h.call(repl, vec![s("abc"), s(""), s("_")]), s("abc"));
        assert_eq!(
            h.call(repl, vec![s("aaa"), s("a"), s("aa")]),
            s("aaaaaa"),
            "no rescan of the replacement"
        );
    }

    // ---- vectors ---------------------------------------------------------------------------------------------

    #[test]
    fn vector_arithmetic() {
        let mut h = off();
        let (a, c) = (h.vector(1.0, 2.0, 3.0), h.vector(4.0, -5.0, 0.5));
        assert_eq!(
            h.call("Object.+(vector,vector)", vec![a.clone(), c.clone()]),
            h.vector(5.0, -3.0, 3.5)
        );
        assert_eq!(
            h.call("Object.-(vector,vector)", vec![a.clone(), c.clone()]),
            h.vector(-3.0, 7.0, 2.5)
        );
        assert_eq!(
            h.call("Object.*(vector,vector)", vec![a.clone(), c.clone()]),
            h.vector(4.0, -10.0, 1.5)
        );
        assert_eq!(
            h.call("Object.*(vector,float)", vec![a.clone(), f(2.0)]),
            h.vector(2.0, 4.0, 6.0)
        );
        assert_eq!(
            h.call("Object.*(float,vector)", vec![f(-1.0), a.clone()]),
            h.vector(-1.0, -2.0, -3.0)
        );
        assert_eq!(
            h.call("Object./(vector,float)", vec![a.clone(), f(2.0)]),
            h.vector(0.5, 1.0, 1.5)
        );
        assert_eq!(
            h.call("Object./(vector,float)", vec![a.clone(), f(0.0)]),
            h.vector(0.0, 0.0, 0.0),
            "not logged"
        );
        assert!(h.vm.log.is_empty());
        // Divided as times the double reciprocal, then rounded. C++ build.
        assert_eq!(
            fields(&h.call("Object./(vector,float)", vec![a.clone(), f(3.0)])),
            vec![fb(0x3eaa_aaab), fb(0x3f2a_aaab), fb(0x3f80_0000)]
        );
        assert_eq!(
            h.call("Object.-_pre(vector)", vec![a.clone()]),
            h.vector(-1.0, -2.0, -3.0)
        );
        assert_eq!(
            h.call(
                "Object.==(vector,vector)",
                vec![a.clone(), h.vector(1.0, 2.0, 3.0)]
            ),
            b(true)
        );
        assert_eq!(
            h.call("Object.!=(vector,vector)", vec![a.clone(), c.clone()]),
            b(true)
        );
        assert_eq!(
            h.call("Object.Dot(vector,vector)", vec![a.clone(), c.clone()]),
            f(-4.5)
        );
        assert_eq!(
            h.call(
                "Object.Cross(vector,vector)",
                vec![h.vector(1.0, 0.0, 0.0), h.vector(0.0, 1.0, 0.0)]
            ),
            h.vector(0.0, 0.0, 1.0)
        );
        assert_eq!(
            h.call("Object.Cross(vector,vector)", vec![a.clone(), c.clone()]),
            h.vector(16.0, 11.5, -13.0)
        );
    }

    #[test]
    fn vector_size_normal_and_zero() {
        let mut h = off();
        assert_eq!(
            h.call("Object.VSize(vector)", vec![h.vector(3.0, 4.0, 0.0)]),
            f(5.0)
        );
        assert_eq!(
            h.call("Object.VSizeSq(vector)", vec![h.vector(3.0, 4.0, 1.0)]),
            f(26.0)
        );
        assert_eq!(
            h.call("Object.Normal(vector)", vec![h.vector(0.0, 3.0, 4.0)]),
            h.vector(0.0, 0.6, 0.8)
        );
        assert_eq!(
            h.call("Object.Normal(vector)", vec![h.vector(0.0, 0.0, 0.0)]),
            h.vector(0.0, 0.0, 0.0)
        );
        assert_eq!(
            h.call("Object.Normal(vector)", vec![h.vector(1e-9, 0.0, 0.0)]),
            h.vector(0.0, 0.0, 0.0)
        );
        assert_eq!(
            h.call("Object.IsZero(vector)", vec![h.vector(0.0, -0.0, 0.0)]),
            b(true)
        );
        assert_eq!(
            h.call("Object.IsZero(vector)", vec![h.vector(0.0, 1e-30, 0.0)]),
            b(false)
        );
    }

    #[test]
    fn vector_compound_assignment() {
        let mut h = off();
        let (a, c) = (h.vector(1.0, 2.0, 3.0), h.vector(1.0, 1.0, 1.0));
        let (r, outs) = h.call_out("Object.+=(vector,vector)", vec![a.clone(), c.clone()], &[0]);
        assert_eq!(
            (r, outs[0].clone()),
            (h.vector(2.0, 3.0, 4.0), h.vector(2.0, 3.0, 4.0))
        );
        assert_eq!(
            h.call_out("Object.-=(vector,vector)", vec![a.clone(), c], &[0])
                .1[0],
            h.vector(0.0, 1.0, 2.0)
        );
        assert_eq!(
            h.call_out("Object.*=(vector,float)", vec![a.clone(), f(2.0)], &[0])
                .1[0],
            h.vector(2.0, 4.0, 6.0)
        );
        assert_eq!(
            h.call_out("Object./=(vector,float)", vec![a.clone(), f(2.0)], &[0])
                .1[0],
            h.vector(0.5, 1.0, 1.5)
        );
        assert_eq!(
            h.call_out("Object./=(vector,float)", vec![a, f(0.0)], &[0])
                .1[0],
            h.vector(0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn a_vector_needs_its_struct_and_a_struct_argument() {
        let mut h = build(ParityMode::Off, false, None);
        let a = h.vector(1.0, 2.0, 3.0);
        // Reading works on any struct with the fields; making a Vector needs Core.Object.Vector.
        assert_eq!(
            h.call("Object.Dot(vector,vector)", vec![a.clone(), a.clone()]),
            f(14.0)
        );
        let e = h
            .try_call("Object.+(vector,vector)", None, vec![a.clone(), a], &[])
            .unwrap_err();
        assert!(matches!(not_yet(e), Missing::LayerAbsent(_)));
        let mut h = off();
        let e = h
            .try_call("Object.VSize(vector)", None, vec![i(3)], &[])
            .unwrap_err();
        assert!(matches!(e, VmError::BadArgument { index: 0, .. }), "{e:?}");
        let r = h.rotator(1, 2, 3);
        let e = h
            .try_call("Object.VSize(vector)", None, vec![r], &[])
            .unwrap_err();
        assert!(
            matches!(e, VmError::BadArgument { index: 0, .. }),
            "no X field: {e:?}"
        );
    }

    // ---- rotators --------------------------------------------------------------------------------------------

    #[test]
    fn rotator_operators() {
        let mut h = off();
        let (a, c) = (h.rotator(1, 2, 3), h.rotator(65537, 2 - 65536, 3));
        assert_eq!(
            h.call("Object.==(rotator,rotator)", vec![a.clone(), c.clone()]),
            b(true),
            "compared mod 65536"
        );
        assert_eq!(
            h.call("Object.!=(rotator,rotator)", vec![a.clone(), c.clone()]),
            b(false)
        );
        assert_eq!(
            h.call(
                "Object.!=(rotator,rotator)",
                vec![a.clone(), h.rotator(1, 2, 4)]
            ),
            b(true)
        );
        assert_eq!(
            h.call(
                "Object.+(rotator,rotator)",
                vec![a.clone(), h.rotator(10, 20, 30)]
            ),
            h.rotator(11, 22, 33)
        );
        assert_eq!(
            h.call(
                "Object.+(rotator,rotator)",
                vec![h.rotator(i32::MAX, 0, 0), h.rotator(1, 0, 0)]
            ),
            h.rotator(i32::MIN, 0, 0),
            "wrapped by makeInt, not normalized"
        );
        assert_eq!(
            h.call(
                "Object.-(rotator,rotator)",
                vec![a.clone(), h.rotator(10, 20, 30)]
            ),
            h.rotator(-9, -18, -27)
        );
        // int64_t(axis * s): toward zero.
        assert_eq!(
            h.call(
                "Object.*(rotator,float)",
                vec![h.rotator(3, -3, 100), f(0.5)]
            ),
            h.rotator(1, -1, 50)
        );
        assert_eq!(
            h.call(
                "Object.*(float,rotator)",
                vec![f(2.5), h.rotator(3, -3, 100)]
            ),
            h.rotator(7, -7, 250)
        );
        assert_eq!(
            h.call(
                "Object.Normalize(rotator)",
                vec![h.rotator(40000, 65541, -1)]
            ),
            h.rotator(-25536, 5, -1)
        );
    }

    #[test]
    fn rotator_axes() {
        let mut h = off();
        let zero = h.vector(0.0, 0.0, 0.0);
        let key = "Object.GetAxes(rotator,vector,vector,vector)";
        let identity = h.rotator(0, 0, 0);
        let (r, outs) = h.call_out(
            key,
            vec![identity, zero.clone(), zero.clone(), zero.clone()],
            &[1, 2, 3],
        );
        assert_eq!(r, Value::None);
        assert_eq!(
            outs[1..],
            [
                h.vector(1.0, 0.0, 0.0),
                h.vector(0.0, 1.0, 0.0),
                h.vector(0.0, 0.0, 1.0)
            ]
        );
        // A rotation with all three axes: GetUnAxes is the transpose of GetAxes.
        let rot = h.rotator(3000, -12000, 7000);
        let (_, axes) = h.call_out(
            key,
            vec![rot.clone(), zero.clone(), zero.clone(), zero.clone()],
            &[1, 2, 3],
        );
        let (_, un) = h.call_out(
            "Object.GetUnAxes(rotator,vector,vector,vector)",
            vec![rot, zero.clone(), zero.clone(), zero],
            &[1, 2, 3],
        );
        for row in 0..3 {
            for col in 0..3 {
                assert_eq!(
                    fields(&axes[row + 1])[col],
                    fields(&un[col + 1])[row],
                    "{row} {col}"
                );
            }
        }
        // All three rows, C++ build.
        let rows = [
            [0x3ec8_5626, 0xbf60_1e13, 0x3e91_3d79],
            [0x3f49_755a, 0x3e22_4dd3, 0xbf18_a824],
            [0x3ef4_44d9, 0x3ee9_c294, 0x3f40_3ec8],
        ];
        for (row, bits) in rows.into_iter().enumerate() {
            assert_eq!(fields(&axes[row + 1]), bits.map(fb).to_vec(), "row {row}");
        }
    }

    // ---- objects and classes ---------------------------------------------------------------------------------

    #[test]
    fn isa_and_class_is_child_of() {
        let mut h = off();
        let p = h.vm.instantiate(h.pawn, "P").unwrap();
        for (name, want) in [
            ("Pawn", true),
            ("actor", true),
            ("OBJECT", true),
            ("Controller", false),
        ] {
            let n = h.name(name);
            assert_eq!(h.call_on("Object.IsA(name)", p, vec![n]), b(want), "{name}");
        }
        let n = h.name("Object");
        assert_eq!(h.call("Object.IsA(name)", vec![n]), b(false), "no object");
        let (pawn, actor) = (Value::Class(Some(h.pawn)), Value::Class(Some(h.actor)));
        assert_eq!(
            h.call(
                "Object.ClassIsChildOf(class,class)",
                vec![pawn.clone(), actor.clone()]
            ),
            b(true)
        );
        assert_eq!(
            h.call(
                "Object.ClassIsChildOf(class,class)",
                vec![actor.clone(), pawn.clone()]
            ),
            b(false)
        );
        assert_eq!(
            h.call(
                "Object.ClassIsChildOf(class,class)",
                vec![pawn.clone(), pawn.clone()]
            ),
            b(true)
        );
        assert_eq!(
            h.call(
                "Object.ClassIsChildOf(class,class)",
                vec![pawn, Value::Class(None)]
            ),
            b(false)
        );
    }

    #[test]
    fn isa_and_class_is_child_of_answer_not_yet_through_an_engine_native_class() {
        // Through an engine-native stand-in class (decision 2026-11-15-08) the real chain is unknown: a "no" there is
        // NotYet, a "yes" found before it still stands (crate::interp::is_a_named / is_child).
        let mut h = off();
        let object = h.object;
        let native = h.vm.space.classes.define("Engine.StandIn", Some(object), vec![]).unwrap();
        h.vm.space.classes.mark_native(native).unwrap();
        let leaf = h.vm.space.classes.define("Game.Leaf", Some(native), vec![]).unwrap();
        let o = h.vm.instantiate(leaf, "L").unwrap();
        for name in ["Leaf", "standin"] {
            let n = h.name(name);
            assert_eq!(h.call_on("Object.IsA(name)", o, vec![n]), b(true), "{name}");
        }
        let actor = h.name("Actor");
        let id = h.declare("Object.IsA(name)", 1, &[]);
        let e = h.vm.call(id, Some(o), vec![actor]).unwrap_err();
        assert_eq!(not_yet(e), Missing::NotRead);
        let key = "Object.ClassIsChildOf(class,class)";
        let (leaf_c, native_c) = (Value::Class(Some(leaf)), Value::Class(Some(native)));
        assert_eq!(h.call(key, vec![leaf_c.clone(), native_c]), b(true));
        let e = h
            .try_call(key, None, vec![leaf_c, Value::Class(Some(h.actor))], &[])
            .unwrap_err();
        assert_eq!(not_yet(e), Missing::NotRead);
    }

    #[test]
    fn the_interface_query_answers_not_yet_through_an_engine_native_class() {
        // A class whose super is an engine-native stand-in (decision 2026-11-15-08): a "no" from the interface table is
        // not proven, so QueryInterface and the iterators' InterfaceClass filter answer NotYet, as DynamicCast does.
        let mut h = off();
        let object = h.object;
        let native = h.vm.space.classes.define("Engine.StandIn", Some(object), vec![]).unwrap();
        h.vm.space.classes.mark_native(native).unwrap();
        let leaf = h.vm.space.classes.define("Game.Leaf", Some(native), vec![]).unwrap();
        let o = h.vm.instantiate(leaf, "L").unwrap();
        let key = "Object.QueryInterface(class)";
        // A yes by the class chain still stands.
        assert_eq!(h.call_on(key, o, vec![Value::Class(Some(native))]), Value::Object(Some(o)));
        let e = h
            .try_call(key, Some(o), vec![Value::Class(Some(h.actor))], &[])
            .unwrap_err();
        assert_eq!(not_yet(e), Missing::NotRead);
        for key in [
            "Actor.AllActors(class,object,class)",
            "Actor.DynamicActors(class,object,class)",
        ] {
            let id = h.declare(key, 3, &[1]);
            let it = h.vm.iterator_for(id).unwrap();
            let args = vec![
                Arg::new(Value::Class(Some(leaf)), false, true),
                Arg::new(Value::None, true, true),
                Arg::new(Value::Class(Some(h.actor)), false, true),
            ];
            let mut call = NativeCall::new(&mut h.vm, id, None, args);
            let e = it(&mut call).unwrap_err();
            assert_eq!(not_yet(e), Missing::NotRead, "{key}");
        }
    }

    /// `Actor.LocalPlayerControllers` (decision 2026-11-16-81): outside Reference only; the world's local players in
    /// order, filtered by BaseClass; a None BaseClass yields nothing; no world, `NotYet { LayerAbsent }`.
    #[test]
    fn local_player_controllers() {
        let key = "Actor.LocalPlayerControllers(class,object)";
        let iterate = |h: &mut H, base: Option<ClassId>| -> Result<Vec<Value>, VmError> {
            let id = h.declare(key, 2, &[1]);
            let it = h.vm.iterator_for(id).expect("bound outside Reference");
            let args = vec![Arg::new(Value::Class(base), false, true), Arg::new(Value::None, true, true)];
            let mut call = NativeCall::new(&mut h.vm, id, None, args);
            Ok(it(&mut call)?.items)
        };
        let mut h = off();
        let (actor, pawn) = (h.actor, h.pawn);
        let e = iterate(&mut h, Some(actor)).unwrap_err();
        assert!(matches!(not_yet(e), Missing::LayerAbsent(_)), "no world, no local player list");
        let first = h.vm.instantiate(actor, "PC_0").unwrap();
        let second = h.vm.instantiate(pawn, "PC_1").unwrap();
        let level = h.vm.instantiate(pawn, "LevelPawn").unwrap();
        let walk: ActorWalkFn = Rc::new(move |_vm: &Vm, which| {
            Ok(match which {
                ActorWalk::LocalPlayers => vec![first, second],
                _ => vec![level],
            })
        });
        h.vm.add_service(ActorSource::new(walk)).unwrap();
        let o = |x| Value::Object(Some(x));
        assert_eq!(iterate(&mut h, Some(actor)).unwrap(), vec![o(first), o(second)], "player index order");
        assert_eq!(iterate(&mut h, Some(pawn)).unwrap(), vec![o(second)], "the class filter");
        assert_eq!(iterate(&mut h, None).unwrap(), vec![], "a None BaseClass yields nothing");
        let mut r = build(ParityMode::Reference, true, None);
        let id = r.declare(key, 2, &[1]);
        assert!(r.vm.iterator_for(id).is_none(), "Reference keeps the C++ (unbound)");
    }

    #[test]
    fn query_interface() {
        let mut h = off();
        let p = h.vm.instantiate(h.pawn, "P").unwrap();
        let key = "Object.QueryInterface(class)";
        assert_eq!(
            h.call_on(key, p, vec![Value::Class(Some(h.actor))]),
            Value::Object(Some(p))
        );
        assert_eq!(
            h.call_on(key, p, vec![Value::Class(None)]),
            Value::Object(None)
        );
        assert_eq!(
            h.call(key, vec![Value::Class(Some(h.actor))]),
            Value::Object(None)
        );
        // Not derived: the interface table answers (`Vm::implements`); a class defined by hand, with no package
        // export behind it, implements nothing. The table itself is tested in `interp::delegate`.
        let a = h.vm.instantiate(h.actor, "A").unwrap();
        assert_eq!(
            h.call_on(key, a, vec![Value::Class(Some(h.pawn))]),
            Value::Object(None)
        );
    }

    #[test]
    fn actor_iterators_visit_live_instances_in_creation_order() {
        let mut h = off();
        let a = h.vm.instantiate(h.actor, "A").unwrap();
        let _o = h.vm.instantiate(h.object, "O").unwrap();
        let p = h.vm.instantiate(h.pawn, "P").unwrap();
        let gone = h.vm.instantiate(h.pawn, "Gone").unwrap();
        h.vm.space.heap.destroy(gone).unwrap();
        let empty =
            h.vm.space
                .classes
                .define("Game.Empty", Some(h.actor), vec![])
                .unwrap();
        for key in [
            "Actor.AllActors(class,object,class)",
            "Actor.DynamicActors(class,object,class)",
        ] {
            let id = h.declare(key, 3, &[1]);
            let it = h.vm.iterator_for(id).unwrap();
            let run = |vm: &mut Vm, args: Vec<Arg>| {
                let mut call = NativeCall::new(vm, id, None, args);
                it(&mut call)
            };
            let arg = |v: Value, supplied: bool| Arg::new(v, false, supplied);
            let none = Arg::new(Value::None, true, true);
            let r = run(
                &mut h.vm,
                vec![
                    arg(Value::Class(Some(h.actor)), true),
                    none.clone(),
                    arg(Value::None, false),
                ],
            )
            .unwrap();
            assert_eq!(
                (r.out_param, r.items),
                (1, vec![Value::Object(Some(a)), Value::Object(Some(p))])
            );
            let r = run(
                &mut h.vm,
                vec![
                    arg(Value::Class(Some(h.pawn)), true),
                    none.clone(),
                    arg(Value::None, false),
                ],
            )
            .unwrap();
            assert_eq!(r.items, vec![Value::Object(Some(p))]);
            let r = run(
                &mut h.vm,
                vec![
                    arg(Value::Class(None), true),
                    none.clone(),
                    arg(Value::None, false),
                ],
            )
            .unwrap();
            assert!(r.items.is_empty(), "a None base class matches nothing");
            // The interface filter keeps the classes the interface table says implement it (`Vm::implements`); a
            // class defined by hand implements nothing. The table itself is tested in `interp::delegate`.
            let iface = arg(Value::Class(Some(h.object)), true);
            let r = run(
                &mut h.vm,
                vec![
                    arg(Value::Class(Some(h.actor)), true),
                    none.clone(),
                    iface.clone(),
                ],
            )
            .unwrap();
            assert!(r.items.is_empty());
            let r = run(
                &mut h.vm,
                vec![arg(Value::Class(Some(empty)), true), none, iface],
            )
            .unwrap();
            assert!(r.items.is_empty());
        }
    }

    #[test]
    fn state_name_queries_and_logging() {
        let mut h = off();
        let p = h.vm.instantiate(h.pawn, "P").unwrap();
        assert_eq!(
            h.call_on("Object.GetStateName()", p, vec![]),
            Value::Name(NameId::NONE)
        );
        assert_eq!(
            h.call("Object.GetStateName()", vec![]),
            Value::Name(NameId::NONE)
        );
        let none = h.name("None");
        assert_eq!(
            h.call_on("Object.IsInState(name,bool)", p, vec![none]),
            b(false),
            "no state is not 'None'"
        );
        let state = h.vm.space.names.intern("Firing");
        h.vm.space.heap.get_mut(p).unwrap().state = Some(state);
        assert_eq!(
            h.call_on("Object.GetStateName()", p, vec![]),
            Value::Name(state)
        );
        // Outside Reference IsInState reads the object's state frame (W5-STATES): a name poked into the heap with
        // no frame is not a state. The name comparison below is the C++'s, under Reference.
        let firing = h.name("FIRING");
        assert_eq!(
            h.call_on("Object.IsInState(name,bool)", p, vec![firing.clone()]),
            b(false)
        );
        let mut h = build(ParityMode::Reference, true, None);
        let p = h.vm.instantiate(h.pawn, "P").unwrap();
        h.vm.space.heap.get_mut(p).unwrap().state = Some(h.vm.space.names.intern("Firing"));
        let firing = h.name("FIRING");
        assert_eq!(
            h.call_on(
                "Object.IsInState(name,bool)",
                p,
                vec![firing.clone(), b(true)]
            ),
            b(true)
        );
        let idle = h.name("Idle");
        assert_eq!(
            h.call_on("Object.IsInState(name,bool)", p, vec![idle]),
            b(false)
        );
        assert_eq!(
            h.call("Object.IsInState(name,bool)", vec![firing]),
            b(false),
            "no object"
        );
        let n = h.name("Tick");
        assert_eq!(
            h.call_on("Object.Enable(name)", p, vec![n.clone()]),
            Value::None
        );
        assert_eq!(h.call_on("Object.Disable(name)", p, vec![n]), Value::None);
        assert!(h.vm.log.is_empty());
        let tag = h.name("Tag");
        assert_eq!(
            h.call("Object.LogInternal(string,name)", vec![s("hi there"), tag]),
            Value::None
        );
        assert_eq!(
            h.call("Object.WarnInternal(string)", vec![s("careful")]),
            Value::None
        );
        assert_eq!(h.vm.log, ["Log: hi there", "Warning: careful"]);
    }

    // ---- GotoState -------------------------------------------------------------------------------------------

    const GOTO: &str = "Object.GotoState(name,name,bool,bool)";

    /// Outside Reference GotoState is the game's state machine (W5-STATES, `crate::states`, whose own tests cover
    /// it on package-loaded states). Here, on classes defined by hand (no state declared): a state name that no
    /// class of the chain declares leaves no state and is logged; no object does nothing.
    #[test]
    fn goto_state_outside_reference_is_the_state_machine() {
        let mut h = off();
        let p = h.vm.instantiate(h.pawn, "P").unwrap();
        let a = h.name("A");
        assert_eq!(h.try_call(GOTO, Some(p), vec![a], &[]).unwrap().ret, Value::None);
        assert_eq!(h.vm.space.heap.get(p).unwrap().state, None);
        assert_eq!(h.vm.log.len(), 1);
        assert!(h.vm.log[0].starts_with("GotoState (A): state not found in "), "{:?}", h.vm.log);
        assert_eq!(h.try_call(GOTO, None, vec![], &[]).unwrap().ret, Value::None);
    }

    #[test]
    fn goto_state_under_reference_reads_the_loaded_states() {
        // `register` wires Vm::class_declares_state and Vm::find_method (W3-INTERP). These classes are defined by
        // hand, with no package behind them, so they declare no state: a change only sets the name (logged), and
        // no state event runs. The package-loaded case is in crates/ow-vm/src/interp/tests.rs.
        let mut h = build(ParityMode::Reference, true, None);
        let p = h.vm.instantiate(h.pawn, "P").unwrap();
        // Nothing to look up: no object, no state argument, or the same state without bForceEvents.
        let a = h.name("A");
        assert_eq!(
            h.try_call(GOTO, None, vec![a.clone()], &[]).unwrap().ret,
            Value::None
        );
        assert_eq!(
            h.try_call(GOTO, Some(p), vec![], &[]).unwrap().ret,
            Value::None
        );
        let none = h.name("None");
        let begin = h.name("Begin");
        assert_eq!(
            h.try_call(GOTO, Some(p), vec![none.clone(), begin], &[])
                .unwrap()
                .ret,
            Value::None
        );
        assert_eq!(
            h.vm.log,
            ["GotoState: label Begin not run (latent state code is not run)"]
        );
        h.vm.log.clear();
        assert_eq!(
            h.try_call(GOTO, Some(p), vec![a], &[]).unwrap().ret,
            Value::None
        );
        assert_eq!(
            h.vm.log,
            ["GotoState: state A is not declared on Pawn; name set, no state events"]
        );
        let state = h.vm.space.heap.get(p).unwrap().state;
        assert_eq!(state.map(|s| h.vm.space.names.text(s).into_owned()), Some("A".to_string()));
        // Back to the default state with bForceEvents: no EndState or BeginState is declared anywhere.
        h.vm.log.clear();
        h.try_call(GOTO, Some(p), vec![none, Value::Name(NameId::NONE), b(true)], &[])
            .unwrap();
        assert_eq!(h.vm.space.heap.get(p).unwrap().state, None);
        assert!(h.vm.log.is_empty(), "{:?}", h.vm.log);
    }

    /// A stand-in lookup for one class: declared states, and per (state or class level, event) one function.
    struct FakeStates {
        declared: Vec<String>,
        methods: RefCell<Vec<(Option<String>, String, FuncId)>>,
    }

    impl StateLookup for FakeStates {
        fn declares_state(&self, vm: &mut Vm, _: ClassId, state: NameId) -> Result<bool, VmError> {
            Ok(self.declared.contains(&lowered(&name_text(vm, state))))
        }
        fn find_method(
            &self,
            vm: &mut Vm,
            _: ClassId,
            name: &str,
            state: Option<NameId>,
        ) -> Result<Option<FuncId>, VmError> {
            let state = state.map(|s| lowered(&name_text(vm, s)));
            let methods = self.methods.borrow();
            let find = |st: &Option<String>| {
                methods
                    .iter()
                    .find(|(s, n, _)| s == st && n.eq_ignore_ascii_case(name))
                    .map(|m| m.2)
            };
            Ok(state
                .as_ref()
                .and_then(|_| find(&state))
                .or_else(|| find(&None)))
        }
    }

    /// Core natives under Reference with a state class: states A, B, C (BeginState goes to A), D (EndState towards A
    /// goes to B instead), Loop (BeginState forces Loop again); each event logs `b<State>(<arg>);` or
    /// `e<State>(<arg>);`.
    fn states_fixture() -> (H, ObjRef) {
        let fake = Rc::new(FakeStates {
            declared: ["a", "b", "c", "d", "loop"].map(String::from).to_vec(),
            methods: RefCell::new(Vec::new()),
        });
        let mut h = build(
            ParityMode::Reference,
            true,
            Some(fake.clone() as Rc<dyn StateLookup>),
        );
        let goto = h.declare(GOTO, 4, &[]);
        // (state, b/e, then: (GotoState target, bForceEvents, only when the event's argument is this state)).
        type Then = Option<(&'static str, bool, Option<&'static str>)>;
        let events: [(&str, char, Then); 10] = [
            ("A", 'b', None),
            ("A", 'e', None),
            ("B", 'b', None),
            ("B", 'e', None),
            ("C", 'b', Some(("A", false, None))),
            ("C", 'e', None),
            ("D", 'b', None),
            ("D", 'e', Some(("B", false, Some("A")))),
            ("Loop", 'b', Some(("Loop", true, None))),
            ("Loop", 'e', None),
        ];
        for (state, kind, then) in events {
            let event = if kind == 'b' {
                "BeginState"
            } else {
                "EndState"
            };
            let path = ObjectPath::parse(&format!("Game.Pawn.{state}.{event}")).unwrap();
            let arg = PropertyDecl::new(h.vm.space.names.intern("Other"), PropKind::Name)
                .with_flags(PropertyDecl::PARAM);
            let id =
                h.vm.declare_function(FunctionDecl::script(path.clone()).with_params(vec![arg]))
                    .unwrap();
            let then = then.map(|(s, force, when)| (h.vm.space.names.intern(s), force, when));
            let binding: NativeFn = Rc::new(move |c: &mut NativeCall<'_>| {
                let other = text(c, 0)?;
                c.vm.log.push(format!("{kind}{state}({other});"));
                if let Some((next, force, when)) = then
                    && when.is_none_or(|w| w.eq_ignore_ascii_case(&other))
                {
                    let this = c.this;
                    c.call_script(
                        goto,
                        this,
                        vec![
                            Value::Name(next),
                            Value::Name(NameId::NONE),
                            Value::Bool(force),
                        ],
                    )?;
                }
                Ok(Value::None)
            });
            h.vm.override_script(path.as_str(), binding).unwrap();
            fake.methods
                .borrow_mut()
                .push((Some(lowered(state)), event.to_string(), id));
        }
        let m = h.vm.instantiate(h.pawn, "M").unwrap();
        (h, m)
    }

    fn goto(h: &mut H, m: ObjRef, state: &str, force: bool) -> String {
        h.vm.log.clear();
        let n = h.name(state);
        h.try_call(
            GOTO,
            Some(m),
            vec![n, Value::Name(NameId::NONE), b(force)],
            &[],
        )
        .unwrap();
        h.vm.log.concat()
    }

    fn state_of(h: &H, m: ObjRef) -> String {
        h.vm.space
            .heap
            .get(m)
            .unwrap()
            .state
            .map_or_else(|| "-".to_string(), |s| name_text(&h.vm, s))
    }

    #[test]
    fn goto_state_runs_end_then_begin_with_the_other_state() {
        let (mut h, m) = states_fixture();
        assert_eq!(
            goto(&mut h, m, "A", false),
            "bA(None);",
            "from no state: no EndState, BeginState(None)"
        );
        assert_eq!(state_of(&h, m), "A");
        assert_eq!(
            goto(&mut h, m, "a", false),
            "",
            "the same state (any case): no events"
        );
        assert_eq!(goto(&mut h, m, "B", false), "eA(B);bB(A);");
        assert_eq!(
            goto(&mut h, m, "B", true),
            "eB(B);bB(B);",
            "bForceEvents on the same state"
        );
        assert_eq!(
            goto(&mut h, m, "None", false),
            "eB(None);",
            "to no state: EndState only"
        );
        assert_eq!(state_of(&h, m), "-");
        assert_eq!(goto(&mut h, m, "None", false), "");
    }

    #[test]
    fn goto_state_of_an_undeclared_state_only_sets_the_name() {
        let (mut h, m) = states_fixture();
        goto(&mut h, m, "A", false);
        assert_eq!(
            goto(&mut h, m, "Nope", false),
            "GotoState: state Nope is not declared on Pawn; name set, no state events"
        );
        assert_eq!(state_of(&h, m), "Nope");
    }

    #[test]
    fn goto_state_inside_the_events() {
        let (mut h, m) = states_fixture();
        // BeginState(C) goes to A: C ends once, A begins.
        assert_eq!(goto(&mut h, m, "C", false), "bC(None);eC(A);bA(C);");
        assert_eq!(state_of(&h, m), "A");
        // EndState(D) goes to B: the inner change stands, the outer one is dropped.
        let (mut h, m) = states_fixture();
        assert_eq!(goto(&mut h, m, "D", false), "bD(None);");
        assert_eq!(goto(&mut h, m, "A", false), "eD(A);eD(B);bB(D);");
        assert_eq!(state_of(&h, m), "B");
    }

    #[test]
    fn goto_state_nesting_is_bounded_at_8_levels() {
        let (mut h, m) = states_fixture();
        let trace = goto(&mut h, m, "Loop", false);
        let want = String::from("bLoop(None);")
            + &"eLoop(Loop);bLoop(Loop);".repeat(7)
            + "GotoState: Loop abandoned, state events nested too deep";
        assert_eq!(trace, want);
        // The counter unwound: the next GotoState runs again.
        assert_eq!(goto(&mut h, m, "A", false), "eLoop(A);bA(Loop);");
    }

    #[test]
    fn goto_state_logs_a_label_and_runs_nothing_for_it() {
        let (mut h, m) = states_fixture();
        h.vm.log.clear();
        let (a, begin) = (h.name("A"), h.name("Begin"));
        h.try_call(GOTO, Some(m), vec![a, begin], &[]).unwrap();
        assert!(h.log_has("GotoState: label Begin not run (latent state code is not run)"));
        assert!(h.log_has("bA(None);"));
        assert_eq!(state_of(&h, m), "A");
    }
}
