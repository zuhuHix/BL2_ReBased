//! Owner: lane W5-WORLD. Per-actor timers: the `Timers` array of NATIVE_ENGINE_CORE.md, "Actor timers", its nine
//! natives and the engine's timer update pass. AI-assisted (Claude Code).
//!
//! Every rule below is the record's reading of the executable, **UNVERIFIED in the running game**:
//!
//! - An entry is keyed by (function name, target object); an omitted or None object means the actor itself, an
//!   omitted name `Timer`. The same name on two objects is two timers.
//! - `SetTimer` replaces the first matching entry in place (rate 0: only `Rate` is set; otherwise `bLoop` and
//!   `Rate` overwritten and `Count` reset) and always clears the pause; a new entry is appended (also at rate 0)
//!   with `Count` 0 and dilation 1. A `bStatic` actor ignores the call. Negative rates are stored as given.
//! - `ClearTimer` and `ClearAllTimers` only set `Rate` to 0; entries leave only in the update pass.
//! - `PauseTimer` sets the flag on every match; `IsTimerActive`, `GetTimerCount` (-1 when absent), `GetTimerRate`
//!   (-1 when absent) and the dilation natives use the first match.
//! - The update pass ([`update_pass`]) grows each unpaused `Count` by `TimerTimeDilation * delta`, then walks the
//!   entries by index, re-reading the length: rate 0 or a gone target removes the entry; an entry is due when
//!   `Count > Rate` strictly; a loop fires `floor(Count / Rate)` times with `Count` reduced first; the callback is
//!   found by name in the target's state, then class chain (missing: removed silently) and called with zeroed
//!   parameters; after each callback the pass stops if the actor was destroyed, a cleared entry is removed, and a
//!   `Count` of exactly 0 (a re-set) keeps a non-loop entry; a due non-loop entry is removed afterwards.
//!
//! **Open, `NotYet`:** a due timer with a negative rate (the record says the code does not guard it but the outcome
//! was not traced), and a loop due more than `i32::MAX` times in one pass. **UNVERIFIED:** the delta the pass gets
//! (the world tick's delta, with the world `TimeDilation` applied by `SimClock` and an actor's `CustomTimeDilation`
//! not applied; where the game applies either was not located).
//!
//! The entries live in a [`Timers`] service keyed by actor, not in the heap's `Timers` property: script never reads
//! that array, and the natives are its only interface.

use std::cell::RefCell;
use std::rc::Rc;

use ow_core::num::trunc_i32;
use ow_core::{DetMap, NotYet};
use ow_object::ids::{NameId, ObjRef};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::actors::{arg_bool, arg_object, call_with_zeros, flag, is_destroyed};

/// The record these rules come from.
pub const RECORD: &str = "NATIVE_ENGINE_CORE.md (Actor timers)";

/// The default timer function name.
pub const DEFAULT_TIMER: &str = "Timer";

/// One entry of an actor's `Timers` array.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timer {
    pub func: NameId,
    pub object: ObjRef,
    pub rate: f32,
    pub count: f32,
    pub dilation: f32,
    pub looping: bool,
    pub paused: bool,
}

/// Every actor's timers (a VM service).
#[derive(Debug, Default)]
pub struct Timers {
    by_actor: DetMap<ObjRef, Vec<Timer>>,
}

impl Timers {
    pub fn new() -> Self {
        Self::default()
    }

    /// `actor`'s entries in array order.
    pub fn of(&self, actor: ObjRef) -> &[Timer] {
        self.by_actor.get(&actor).map_or(&[], Vec::as_slice)
    }

    fn first(&self, actor: ObjRef, func: NameId, object: ObjRef) -> Option<&Timer> {
        self.of(actor)
            .iter()
            .find(|t| t.func == func && t.object == object)
    }

    fn first_mut(&mut self, actor: ObjRef, func: NameId, object: ObjRef) -> Option<&mut Timer> {
        self.by_actor
            .get_mut(&actor)?
            .iter_mut()
            .find(|t| t.func == func && t.object == object)
    }

    /// `SetTimer` on an actor that is not static (module documentation).
    pub fn set(&mut self, actor: ObjRef, rate: f32, looping: bool, func: NameId, object: ObjRef) {
        if let Some(t) = self.first_mut(actor, func, object) {
            if rate == 0.0 {
                t.rate = 0.0;
            } else {
                t.looping = looping;
                t.rate = rate;
                t.count = 0.0;
            }
            t.paused = false;
            return;
        }
        self.by_actor.entry(actor).or_default().push(Timer {
            func,
            object,
            rate,
            count: 0.0,
            dilation: 1.0,
            looping,
            paused: false,
        });
    }

    /// `ClearTimer`: every match gets rate 0.
    pub fn clear(&mut self, actor: ObjRef, func: NameId, object: ObjRef) {
        for t in self.by_actor.get_mut(&actor).into_iter().flatten() {
            if t.func == func && t.object == object {
                t.rate = 0.0;
            }
        }
    }

    /// `ClearAllTimers`: every entry targeting `object` gets rate 0, whatever its name.
    pub fn clear_all(&mut self, actor: ObjRef, object: ObjRef) {
        for t in self.by_actor.get_mut(&actor).into_iter().flatten() {
            if t.object == object {
                t.rate = 0.0;
            }
        }
    }

    /// `PauseTimer`: every match.
    pub fn pause(&mut self, actor: ObjRef, paused: bool, func: NameId, object: ObjRef) {
        for t in self.by_actor.get_mut(&actor).into_iter().flatten() {
            if t.func == func && t.object == object {
                t.paused = paused;
            }
        }
    }

    /// `IsTimerActive`: the first match has a positive rate (paused timers count).
    pub fn is_active(&self, actor: ObjRef, func: NameId, object: ObjRef) -> bool {
        self.first(actor, func, object)
            .is_some_and(|t| t.rate > 0.0)
    }

    /// `GetTimerCount`: the first match's `Count`, or -1.
    pub fn count(&self, actor: ObjRef, func: NameId, object: ObjRef) -> f32 {
        self.first(actor, func, object).map_or(-1.0, |t| t.count)
    }

    /// `GetTimerRate`: the first match's `Rate`, or -1.
    pub fn rate(&self, actor: ObjRef, func: NameId, object: ObjRef) -> f32 {
        self.first(actor, func, object).map_or(-1.0, |t| t.rate)
    }

    /// `ModifyTimerTimeDilation` (and `ResetTimerTimeDilation` with 1.0): the first match.
    pub fn set_dilation(&mut self, actor: ObjRef, func: NameId, object: ObjRef, dilation: f32) {
        if let Some(t) = self.first_mut(actor, func, object) {
            t.dilation = dilation;
        }
    }

    fn entry(&self, actor: ObjRef, i: usize) -> Option<Timer> {
        self.by_actor.get(&actor).and_then(|v| v.get(i)).copied()
    }

    fn remove(&mut self, actor: ObjRef, i: usize) {
        if let Some(v) = self.by_actor.get_mut(&actor)
            && i < v.len()
        {
            v.remove(i);
        }
    }
}

/// The world's [`Timers`] service, or `NotYet` when this VM has no world.
pub fn timers_of(vm: &Vm) -> Result<Rc<RefCell<Timers>>, VmError> {
    vm.service::<Timers>().ok_or_else(|| {
        NotYet::layer_absent("the actors' timers", "ow-world (World::new)", RECORD).into()
    })
}

fn borrow(t: &RefCell<Timers>) -> Result<std::cell::Ref<'_, Timers>, VmError> {
    t.try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("Timers"))
}

fn borrow_mut(t: &RefCell<Timers>) -> Result<std::cell::RefMut<'_, Timers>, VmError> {
    t.try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("Timers"))
}

/// The engine's timer update pass for `actor` with this tick's `delta` (module documentation).
pub fn update_pass(vm: &mut Vm, actor: ObjRef, delta: f32) -> Result<(), VmError> {
    let timers = timers_of(vm)?;
    {
        let mut t = borrow_mut(&timers)?;
        for e in t.by_actor.get_mut(&actor).into_iter().flatten() {
            if !e.paused {
                e.count += e.dilation * delta;
            }
        }
    }
    let mut i = 0usize;
    loop {
        if is_destroyed(&vm.space, actor)? {
            return Ok(());
        }
        let Some(e) = borrow(&timers)?.entry(actor, i) else {
            return Ok(());
        };
        if e.rate == 0.0 || is_destroyed(&vm.space, e.object)? {
            borrow_mut(&timers)?.remove(actor, i);
            continue;
        }
        // Strictly greater; a NaN count is not due.
        let due = e.count > e.rate;
        if !due {
            i += 1;
            continue;
        }
        if e.rate < 0.0 {
            return Err(NotYet::not_read("a due timer with a negative rate", RECORD).into());
        }
        let firings = if e.looping {
            // floor(Count / Rate) of two positive floats; the integer part through the attribute path's truncation,
            // which gives i32::MIN outside the int range (refused below).
            let n = trunc_i32(e.count / e.rate);
            if n < 1 {
                return Err(NotYet::not_read(
                    "a loop timer due more than i32::MAX times in one pass",
                    RECORD,
                )
                .into());
            }
            n
        } else {
            1
        };
        let (class, state) = {
            let x = vm.space.heap.get(e.object)?;
            (x.class, x.state)
        };
        let name = vm.space.names.text(e.func).into_owned();
        let Some(f) = vm.find_method(class, &name, state)? else {
            borrow_mut(&timers)?.remove(actor, i);
            continue;
        };
        if e.looping {
            // i32 to f32 is exact for the counts met in practice (at most 2^24 firings) and rounds to nearest
            // beyond, as the engine's int-to-float conversion does.
            let firings_f = firings as f32;
            let mut t = borrow_mut(&timers)?;
            if let Some(entry) = t.by_actor.get_mut(&actor).and_then(|v| v.get_mut(i)) {
                entry.count -= firings_f * e.rate;
            }
        }
        let mut keep = e.looping;
        let mut removed = false;
        for _ in 0..firings {
            call_with_zeros(vm, f, Some(e.object), Vec::new())?;
            if is_destroyed(&vm.space, actor)? {
                return Ok(());
            }
            let now = borrow(&timers)?.entry(actor, i);
            match now {
                Some(n) if n.rate == 0.0 => {
                    borrow_mut(&timers)?.remove(actor, i);
                    removed = true;
                    break;
                }
                Some(n) if n.count == 0.0 => keep = true,
                _ => {}
            }
        }
        if removed {
            continue;
        }
        if !keep {
            borrow_mut(&timers)?.remove(actor, i);
            continue;
        }
        i += 1;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Natives
// ---------------------------------------------------------------------------------------------------------------

const SET: &str = "Actor.SetTimer(float,bool,name,object)";
const CLEAR: &str = "Actor.ClearTimer(name,object)";
const CLEAR_ALL: &str = "Actor.ClearAllTimers(object)";
const PAUSE: &str = "Actor.PauseTimer(bool,name,object)";
const ACTIVE: &str = "Actor.IsTimerActive(name,object)";
const COUNT: &str = "Actor.GetTimerCount(name,object)";
const RATE: &str = "Actor.GetTimerRate(name,object)";
const MODIFY: &str = "Actor.ModifyTimerTimeDilation(name,float,object)";
const RESET: &str = "Actor.ResetTimerTimeDilation(name,object)";

fn actor(c: &NativeCall<'_>, native: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| VmError::Script {
        function: native.to_string(),
        message: "called without an actor".to_string(),
    })
}

fn bad(native: &str, index: usize, reason: &'static str) -> VmError {
    VmError::BadArgument {
        function: native.to_string(),
        index,
        reason,
    }
}

/// A name argument; omitted (or `Value::None`) is `default`.
fn arg_name(
    c: &mut NativeCall<'_>,
    native: &str,
    i: usize,
    default: Option<&str>,
) -> Result<NameId, VmError> {
    let given = if c.has(i) {
        Some(c.arg(i)?.clone())
    } else {
        None
    };
    match (given, default) {
        (Some(Value::Name(n)), _) => Ok(n),
        (None | Some(Value::None), Some(d)) => Ok(c.vm.space.names.intern(d)),
        (None | Some(Value::None), None) => Ok(NameId::NONE),
        _ => Err(bad(native, i, "expected a name")),
    }
}

/// A float argument; omitted (or `Value::None`) is 0, as the Core natives read an omitted argument.
fn arg_float(c: &NativeCall<'_>, native: &str, i: usize) -> Result<f32, VmError> {
    if !c.has(i) {
        return Ok(0.0);
    }
    match c.arg(i)? {
        Value::Float(f) => Ok(*f),
        Value::None => Ok(0.0),
        _ => Err(bad(native, i, "expected a float")),
    }
}

/// The target object argument: omitted or None is the actor itself.
fn target(c: &NativeCall<'_>, native: &str, i: usize, actor: ObjRef) -> Result<ObjRef, VmError> {
    Ok(arg_object(c, native, i)?.unwrap_or(actor))
}

fn reg(
    vm: &mut Vm,
    key: &'static str,
    f: impl Fn(&mut NativeCall<'_>, ObjRef, &RefCell<Timers>) -> Result<Value, VmError> + 'static,
) -> Result<(), VmError> {
    vm.register_native(
        key,
        Rc::new(move |c: &mut NativeCall<'_>| {
            let a = actor(c, key)?;
            let timers = timers_of(c.vm)?;
            f(c, a, &timers)
        }),
    )
}

/// Registers the nine timer natives.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    reg(vm, SET, |c, a, t| {
        let rate = arg_float(c, SET, 0)?;
        let looping = arg_bool(c, SET, 1)?;
        let func = arg_name(c, SET, 2, Some(DEFAULT_TIMER))?;
        let object = target(c, SET, 3, a)?;
        if !flag(&c.vm.space, a, "bStatic")? {
            borrow_mut(t)?.set(a, rate, looping, func, object);
        }
        Ok(Value::None)
    })?;
    reg(vm, CLEAR, |c, a, t| {
        let func = arg_name(c, CLEAR, 0, Some(DEFAULT_TIMER))?;
        let object = target(c, CLEAR, 1, a)?;
        borrow_mut(t)?.clear(a, func, object);
        Ok(Value::None)
    })?;
    reg(vm, CLEAR_ALL, |c, a, t| {
        let object = target(c, CLEAR_ALL, 0, a)?;
        borrow_mut(t)?.clear_all(a, object);
        Ok(Value::None)
    })?;
    reg(vm, PAUSE, |c, a, t| {
        let paused = arg_bool(c, PAUSE, 0)?;
        let func = arg_name(c, PAUSE, 1, Some(DEFAULT_TIMER))?;
        let object = target(c, PAUSE, 2, a)?;
        borrow_mut(t)?.pause(a, paused, func, object);
        Ok(Value::None)
    })?;
    reg(vm, ACTIVE, |c, a, t| {
        let func = arg_name(c, ACTIVE, 0, Some(DEFAULT_TIMER))?;
        let object = target(c, ACTIVE, 1, a)?;
        Ok(Value::Bool(borrow(t)?.is_active(a, func, object)))
    })?;
    reg(vm, COUNT, |c, a, t| {
        let func = arg_name(c, COUNT, 0, Some(DEFAULT_TIMER))?;
        let object = target(c, COUNT, 1, a)?;
        Ok(Value::Float(borrow(t)?.count(a, func, object)))
    })?;
    reg(vm, RATE, |c, a, t| {
        let func = arg_name(c, RATE, 0, Some(DEFAULT_TIMER))?;
        let object = target(c, RATE, 1, a)?;
        Ok(Value::Float(borrow(t)?.rate(a, func, object)))
    })?;
    // The name has no default in the two dilation natives (the record): omitted is the name None.
    reg(vm, MODIFY, |c, a, t| {
        let func = arg_name(c, MODIFY, 0, None)?;
        let dilation = arg_float(c, MODIFY, 1)?;
        let object = target(c, MODIFY, 2, a)?;
        borrow_mut(t)?.set_dilation(a, func, object, dilation);
        Ok(Value::None)
    })?;
    reg(vm, RESET, |c, a, t| {
        let func = arg_name(c, RESET, 0, None)?;
        let object = target(c, RESET, 1, a)?;
        borrow_mut(t)?.set_dilation(a, func, object, 1.0);
        Ok(Value::None)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! The table rules on invented handles; the update pass and the natives are tested through script in
    //! `world/tests.rs`.
    use super::*;
    use ow_object::heap::{ObjectSpace, Origin};

    fn two() -> (ObjRef, ObjRef, NameId, NameId) {
        let mut s = ObjectSpace::new();
        let c = s
            .classes
            .define("Invented.Thing", None, Vec::new())
            .unwrap();
        let a = s.alloc(c, "A", None, Origin::Runtime).unwrap();
        let b = s.alloc(c, "B", None, Origin::Runtime).unwrap();
        (a, b, s.names.intern("Timer"), s.names.intern("Ping"))
    }

    #[test]
    fn set_replaces_in_place_and_a_zero_rate_only_clears_the_rate() {
        let (a, b, timer, ping) = two();
        let mut t = Timers::new();
        t.set(a, 2.0, true, ping, a);
        t.by_actor.get_mut(&a).unwrap()[0].count = 1.5;
        t.set_dilation(a, ping, a, 0.5);
        t.pause(a, true, ping, a);
        t.set(a, 3.0, false, ping, a);
        let e = t.of(a)[0];
        assert_eq!(
            (e.rate, e.looping, e.count, e.paused, e.dilation),
            (3.0, false, 0.0, false, 0.5)
        );
        t.by_actor.get_mut(&a).unwrap()[0].count = 1.0;
        t.pause(a, true, ping, a);
        t.set(a, 0.0, true, ping, a);
        let e = t.of(a)[0];
        assert_eq!(
            (e.rate, e.looping, e.count, e.paused),
            (0.0, false, 1.0, false),
            "rate 0: bLoop and Count kept"
        );
        // A new (name, object) pair appends, also at rate 0; the same name on another object is another timer.
        t.set(a, 0.0, false, timer, a);
        t.set(a, 1.0, false, ping, b);
        assert_eq!(t.of(a).len(), 3);
        assert_eq!(t.of(a)[1].dilation, 1.0);
        t.set(a, -1.0, false, timer, b);
        assert_eq!(t.of(a)[3].rate, -1.0, "negative rates are stored");
    }

    #[test]
    fn clear_keeps_the_entry_and_the_queries_read_the_first_match() {
        let (a, b, timer, ping) = two();
        let mut t = Timers::new();
        assert_eq!(
            (
                t.count(a, ping, a),
                t.rate(a, ping, a),
                t.is_active(a, ping, a)
            ),
            (-1.0, -1.0, false)
        );
        t.set(a, 2.0, false, ping, a);
        t.set(a, 4.0, false, timer, a);
        t.set(a, 5.0, false, ping, b);
        t.pause(a, true, ping, a);
        assert!(t.is_active(a, ping, a), "a paused timer is active");
        t.clear(a, ping, a);
        assert_eq!(t.of(a).len(), 3, "ClearTimer removes nothing");
        assert_eq!(
            (
                t.is_active(a, ping, a),
                t.rate(a, ping, a),
                t.count(a, ping, a)
            ),
            (false, 0.0, 0.0)
        );
        assert_eq!(
            t.rate(a, ping, b),
            5.0,
            "the other object's timer is untouched"
        );
        t.clear_all(a, a);
        assert_eq!(t.rate(a, timer, a), 0.0);
        assert_eq!(
            t.rate(a, ping, b),
            5.0,
            "ClearAllTimers matches the object, not the name"
        );
    }
}
