//! Owner: lane W4-VMTOOLS. The VM sweep: the port of `sweepPackage` (`src/vm_sweep.cpp`). AI-assisted (Claude Code).
//!
//! Every script function of one package runs once, with zero-valued arguments, on a new object of its class built
//! from the class defaults (no object for a static function), and the report says how many ran to the end, what
//! stopped the others (grouped by message, digits dropped) and which natives with no implementation the scripts
//! reached. It is a robustness and coverage measurement of the interpreter, not a behaviour test: nothing here is a
//! game rule, and a function that "succeeded" only ran without an error under the logged-stub policy.
//!
//! The command builds the VM ([`crate::api::StubPolicy::LogAndZero`] and the Core natives) and prints the report as
//! the C++ JSON; this module only runs the sweep and ranks what it saw.
//!
//! **Skipped functions, as the C++ counts them.** A native is skipped. A function whose header does not read
//! (`readFunction` throws in C++ `functionAt`) is skipped and counted as undecodable. So is a function whose
//! bytecode failed to decode on an earlier call: the C++ marks it `decodeFailed` on that call (`interp.cpp:702-707`)
//! and the sweep skips it later, while a function first decoded by its own sweep call is attempted and fails with
//! `cannot run <path>: <why>`. The Rust interpreter keeps that state inside `interp`, so the sweep follows it from
//! outside: the VM's call census is on during the sweep, a script function in its `script_calls` was entered (and so
//! decoded, `Vm::invoke` counts it before `interp::prepare` decodes it, as the C++ `invoke` counts before it
//! decodes), and such a function is skipped when its bytecode does not decode (decoded again here with the same
//! `ow_script::decode::decode`). A function whose declarations did not type is attempted and fails with the
//! loader's `cannot run` text; the C++ `functionAt` throws there instead and the sweep passes over it uncounted.
//! That case is not met on the nine code packages (the class-body and declaration gates of W2-REFLECT).
//!
//! **Objects between attempts.** After each attempt the objects it made that nothing references are released, as the
//! C++ reference counts release them ([`release_unreferenced`]), so the engine iterators of later attempts see what
//! they see in the C++ sweep.
//!
//! The functions are found by path through their owner class ([`function_of_export`]), so the package's script is
//! the one the store files under that name (the C++ sweep runs the package it loaded from the command line; for the
//! code packages under the cooked folder both are the same file). The differences from the C++ output met on the
//! nine code packages, and their causes, are in decision 2026-11-15-23.

use std::collections::BTreeMap;
use std::time::Instant;

use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_package::PackageId;
use ow_script::function::{is_function_export, read_function};

use crate::api::{CallCensus, Vm};
use crate::error::VmError;

/// What the sweep runs (C++ `SweepOptions`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepOptions {
    /// Only functions whose owner class has this name (`""`: every class). Compared as the C++ does, case-sensitive.
    pub class_filter: String,
    /// Stop after this many attempted functions (0: all).
    pub limit: usize,
    /// Steps per function (`Limits::step_limit` for the sweep).
    pub step_limit: u64,
    /// Entries kept per ranked list. 0 keeps none, as the C++ loop (`i < top`) does; the usage text is the C++ one,
    /// byte for byte, so it does not say so.
    pub top: usize,
}

impl Default for SweepOptions {
    /// The C++ defaults: every class, no limit, 200,000 steps per function, 25 entries per list.
    fn default() -> Self {
        SweepOptions {
            class_filter: String::new(),
            limit: 0,
            step_limit: 200_000,
            top: 25,
        }
    }
}

/// One ranked failure group: how many functions stopped with a message of this shape, and the first of them as
/// `<function path>: <message>`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureGroup {
    pub count: usize,
    pub example: String,
}

/// One native with no implementation, and how many swept functions reached it (each counted once per function).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnimplementedNative {
    pub functions_reaching: usize,
    /// The stub line without its `UNIMPLEMENTED ` prefix (a native key, `function <Class>.<Name>`, ...).
    pub native: String,
}

/// The sweep of one package (the fields of the C++ JSON, in its order).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SweepReport {
    pub package: String,
    pub attempted: usize,
    pub succeeded: usize,
    pub undecodable_skipped: usize,
    pub no_owner_skipped: usize,
    pub total_steps: u64,
    /// The function that used the most steps (the first of equals), and its steps; `("", 0)` when none ran a step.
    pub slowest: (String, u64),
    /// Wall-clock time of the sweep loop. Not deterministic; the parity comparison leaves it out.
    pub seconds: f64,
    /// The failure groups, most frequent first (equal counts: the larger message shape first), at most `top`.
    pub failures: Vec<FailureGroup>,
    /// The unimplemented natives, most reached first (equal counts: the larger text first), at most `top`.
    pub unimplemented_natives: Vec<UnimplementedNative>,
}

impl SweepReport {
    /// The functions attempted that did not run to the end.
    pub fn failed(&self) -> usize {
        self.attempted - self.succeeded
    }
}

/// The C++ `what()` of a VM error: a script error's own message (the C++ `RuntimeError` text), a definition store
/// error's text and causes, anything else its text and causes.
pub(crate) fn what(e: &VmError) -> String {
    match e {
        VmError::Script { message, .. } => message.clone(),
        VmError::Def(inner) => chain(inner),
        other => chain(other),
    }
}

fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(c) = cause {
        text.push_str(": ");
        text.push_str(&c.to_string());
        cause = c.source();
    }
    text
}

/// The C++ `category`: the first 90 bytes of `message`, every ASCII digit replaced by `#`, so messages that differ
/// only in numbers group together. Bytes, as the C++ `substr`; a cut through a UTF-8 sequence reads as U+FFFD.
pub(crate) fn category(message: &str) -> String {
    let bytes: Vec<u8> = message
        .bytes()
        .take(90)
        .map(|b| if b.is_ascii_digit() { b'#' } else { b })
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// `(count, key)` pairs ranked as the C++ `std::sort(ranked.rbegin(), ranked.rend())`: descending by count, then by
/// key; the first `top` kept.
fn ranked(counts: &BTreeMap<String, usize>, top: usize) -> Vec<(usize, String)> {
    let mut list: Vec<(usize, String)> = counts.iter().map(|(k, &n)| (n, k.clone())).collect();
    list.sort_by(|a, b| b.cmp(a));
    list.truncate(top);
    list
}

/// The C++ `Runtime::zeroValue` of a parameter: the kind's initial value (a struct with its defaults), and for a
/// static array (`array_dim > 1`) an array of that many.
pub(crate) fn zero_argument(
    vm: &Vm,
    decl: &ow_object::layout::PropertyDecl,
) -> Result<Value, VmError> {
    let value = vm
        .space
        .classes
        .default_value(&decl.kind)
        .map_err(ow_object::heap::HeapError::from)?;
    if decl.array_dim > 1 && !matches!(decl.kind, PropKind::Array(_)) {
        let count = usize::try_from(decl.array_dim).unwrap_or(0);
        return Ok(Value::array(vec![value; count]));
    }
    Ok(value)
}

/// Runs every script function of `package` (an id of the VM's package store) once, as the C++ `sweepPackage` does.
/// The VM should use `StubPolicy::LogAndZero` (the C++ sweep logs every stub). It sets the VM's step limit to
/// `options.step_limit` and leaves the call census on, holding every call the sweep made.
///
/// A function that cannot be found, or whose owner class does not load, is passed over uncounted, as the C++ catches
/// `functionAt`. Errors inside a call are the report's failures, never this function's error.
pub fn sweep_package(vm: &mut Vm, package: PackageId, options: &SweepOptions) -> SweepReport {
    let pkg = vm.defs.packages().shared(package);
    let mut report = SweepReport {
        package: pkg.name.clone(),
        attempted: 0,
        succeeded: 0,
        undecodable_skipped: 0,
        no_owner_skipped: 0,
        total_steps: 0,
        slowest: (String::new(), 0),
        seconds: 0.0,
        failures: Vec::new(),
        unimplemented_natives: Vec::new(),
    };
    let mut failures: BTreeMap<String, usize> = BTreeMap::new();
    let mut samples: BTreeMap<String, String> = BTreeMap::new();
    let mut unimplemented: BTreeMap<String, usize> = BTreeMap::new();
    vm.limits.step_limit = options.step_limit;
    vm.census = Some(CallCensus::default());
    // The root class, for the objects the attempts make (`release_unreferenced`).
    let root = vm.find_class("Core.Object").ok();
    let mut allocated = vm.space.heap.len();
    let started = Instant::now();
    let count = i32::try_from(pkg.exports.len()).unwrap_or(i32::MAX);
    for index in 1..=count {
        if !is_function_export(&pkg, index) {
            continue;
        }
        let Some(function) = function_of_export(vm, package, index) else {
            continue;
        };
        let Some(decl) = vm.function(function).cloned() else {
            continue;
        };
        let path = decl.path.as_str().to_string();
        let header = read_function(&pkg, index);
        let decode_failed = match &header {
            Err(_) => true,
            Ok(info) => {
                let entered = vm
                    .census
                    .as_ref()
                    .is_some_and(|c| c.script_calls.contains_key(&path));
                entered && ow_script::decode::decode(&pkg, info).is_err()
            }
        };
        if decl.is_native() || decode_failed {
            if decode_failed {
                report.undecodable_skipped += 1;
            }
            continue;
        }
        let Some(owner) = decl.owner else {
            report.no_owner_skipped += 1;
            continue;
        };
        if !options.class_filter.is_empty() {
            let name = vm
                .space
                .classes
                .get_layout(owner)
                .map(|l| l.path().name().to_string())
                .unwrap_or_default();
            if name != options.class_filter {
                continue;
            }
        }
        if options.limit != 0 && report.attempted >= options.limit {
            break;
        }
        report.attempted += 1;
        vm.log.clear();
        let state = state_of(&pkg, index);
        let outcome = attempt(vm, function, &decl, state.as_deref());
        let steps = vm.steps();
        report.total_steps += steps;
        if steps > report.slowest.1 {
            report.slowest = (path.clone(), steps);
        }
        match outcome {
            Ok(()) => report.succeeded += 1,
            Err(e) => {
                let message = what(&e);
                let key = category(&message);
                *failures.entry(key.clone()).or_insert(0) += 1;
                samples
                    .entry(key)
                    .or_insert_with(|| format!("{path}: {message}"));
            }
        }
        if let Some(root) = root
            && vm.space.heap.len() != allocated
        {
            release_unreferenced(vm, root);
            allocated = vm.space.heap.len();
        }
        let mut seen = std::collections::BTreeSet::new();
        for line in &vm.log {
            if let Some(native) = line.strip_prefix("UNIMPLEMENTED ")
                && seen.insert(line.clone())
            {
                *unimplemented.entry(native.to_string()).or_insert(0) += 1;
            }
        }
    }
    report.seconds = started.elapsed().as_secs_f64();
    report.failures = ranked(&failures, options.top)
        .into_iter()
        .map(|(count, key)| FailureGroup {
            count,
            example: samples.get(&key).cloned().unwrap_or_default(),
        })
        .collect();
    report.unimplemented_natives = ranked(&unimplemented, options.top)
        .into_iter()
        .map(|(functions_reaching, native)| UnimplementedNative {
            functions_reaching,
            native,
        })
        .collect();
    report
}

/// Releases the objects made at run time that nothing the heap lists references, as the C++ VM's reference counts do
/// once an attempt's frames and its `self` are gone (`ObjectPtr` is a `std::shared_ptr`; its `live_` list is weak, so
/// the engine iterators stop seeing a released object). The released objects are freed
/// ([`ow_object::heap::ObjectSpace::free_many`], one heap walk), so the heap does not grow with the sweep.
///
/// - **Holders:** every instance `Heap::live` lists, whatever its origin: the runtime objects and the definitions the
///   store loaded (`Origin::Export`). A reference is a property value (inside structs and arrays too), an interface,
///   a delegate's object, or the `Outer`.
/// - **Released:** a runtime object (`Origin::Runtime`) that no holder references once the released ones are gone.
///   A cycle keeps itself, as shared pointers leak a cycle (a camera and its modifiers, which point back at it, stay
///   alive in the C++ sweep).
/// - **Holders it cannot see** (decision 2026-11-15-23, open): class default objects, resource stand-ins and objects
///   script has destroyed are not instances `Heap::live` lists, and the heap lists no other objects to a caller
///   outside `ow-object`. An object only they reference is released here, where the C++ keeps it, and freeing it
///   clears their reference.
fn release_unreferenced(vm: &mut Vm, root: ow_object::ids::ClassId) {
    use ow_object::heap::Origin;
    use ow_object::ids::ObjRef;
    let holders: Vec<ObjRef> = vm.space.live(root).collect();
    let index: BTreeMap<ObjRef, usize> = holders.iter().enumerate().map(|(i, &o)| (o, i)).collect();
    let mut candidate = vec![false; holders.len()];
    let mut incoming = vec![0usize; holders.len()];
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); holders.len()];
    for (i, &o) in holders.iter().enumerate() {
        let Ok(object) = vm.space.heap.get(o) else {
            continue;
        };
        candidate[i] = object.origin == Origin::Runtime;
        let mut found = Vec::new();
        if let Some(outer) = object.outer {
            found.push(outer);
        }
        for v in &object.slots {
            references(v, &mut found);
        }
        for target in found {
            if let Some(&t) = index.get(&target) {
                incoming[t] += 1;
                outgoing[i].push(t);
            }
        }
    }
    let mut queue: Vec<usize> = (0..holders.len())
        .filter(|&i| candidate[i] && incoming[i] == 0)
        .collect();
    let mut released = vec![false; holders.len()];
    while let Some(i) = queue.pop() {
        released[i] = true;
        for &t in &outgoing[i] {
            incoming[t] -= 1;
            if incoming[t] == 0 && candidate[t] && !released[t] {
                queue.push(t);
            }
        }
    }
    let freed: Vec<ObjRef> = holders
        .iter()
        .zip(&released)
        .filter_map(|(&o, &r)| r.then_some(o))
        .collect();
    // Every handle is current and listed once (`Heap::live`), so the free is not refused; if it were, the objects
    // would stay, as before this rule, and the sweep would go on.
    if vm.space.free_many(&freed).is_ok() {
        // A freed object's state frame is dropped too (decision 2026-11-15-27: every path that frees objects calls
        // `states::forget`). It fails only while the state machine is borrowed, which no sweep code holds here; a
        // frame left behind is never read again (its handle is stale), it only holds memory.
        for &o in &freed {
            if crate::states::forget(vm, o).is_err() {
                break;
            }
        }
    }
}

/// Every object `v` references (inside structs and arrays too), once per reference.
fn references(v: &Value, out: &mut Vec<ow_object::ids::ObjRef>) {
    match v {
        Value::Object(Some(o)) | Value::Interface(Some(o)) => out.push(*o),
        Value::Delegate(Some(d)) => out.extend(d.object),
        Value::Struct(s) => s.fields.iter().for_each(|f| references(f, out)),
        Value::Array(items) => items.iter().for_each(|f| references(f, out)),
        _ => {}
    }
}

/// The VM's function for function export `index` of `package` (C++ `functionAt(package, index)`); `None` where it is
/// not a function export or does not load.
///
/// Found by path, through the owner class: loading the class (`Vm::find_class`) declares every function it and its
/// states declare, under their own paths, and `Vm::find_function` then finds the declared one. Only a function with
/// no class above it goes through the path lookup of the code packages, whose `find_export` drops a leading
/// `<package>.` (as the C++ `findExport`); a path that starts with the package name again is given with the package
/// name doubled, so the lookup sees the path as it is.
pub(crate) fn function_of_export(
    vm: &mut Vm,
    package: PackageId,
    index: i32,
) -> Option<ow_object::ids::FuncId> {
    let pkg = vm.defs.packages().shared(package);
    if !is_function_export(&pkg, index) {
        return None;
    }
    let rest = pkg.path_of(index).ok()?;
    let mut outer = pkg.exports[index as usize - 1].outer;
    let class_of = |at: i32| -> Option<String> {
        let export = pkg.exports.get(usize::try_from(at).ok()?.checked_sub(1)?)?;
        let class = if export.class == 0 {
            "Class".to_string()
        } else {
            pkg.object(export.class).ok()?.name.clone()
        };
        Some(class)
    };
    if class_of(outer).as_deref() == Some("State") {
        outer = pkg.exports[outer as usize - 1].outer;
    }
    if outer > 0 && class_of(outer).as_deref() == Some("Class") {
        let class = format!("{}.{}", pkg.name, pkg.path_of(outer).ok()?);
        vm.find_class(&class).ok()?;
    }
    let path = format!("{}.{rest}", pkg.name);
    match vm.find_function(&path) {
        Ok(f) => Some(f),
        Err(_)
            if rest
                .to_ascii_lowercase()
                .starts_with(&format!("{}.", pkg.name.to_ascii_lowercase())) =>
        {
            vm.find_function(&format!("{}.{path}", pkg.name)).ok()
        }
        Err(_) => None,
    }
}

/// The state a function export is declared in (C++ `Function::state`): the name of its outer export when that is a
/// State export.
fn state_of(pkg: &ow_package::package::Package, index: i32) -> Option<String> {
    let outer = pkg
        .exports
        .get(usize::try_from(index).ok()?.checked_sub(1)?)?
        .outer;
    if outer <= 0 {
        return None;
    }
    let state = pkg.exports.get(usize::try_from(outer).ok()? - 1)?;
    let class = pkg.object(state.class).ok()?;
    (state.class != 0 && class.name == "State").then(|| state.name.clone())
}

/// One attempt (the body of the C++ `try`): the object (none for a static function), its state set to the
/// function's, the zero arguments, the call.
fn attempt(
    vm: &mut Vm,
    function: ow_object::ids::FuncId,
    decl: &crate::api::FunctionDecl,
    state: Option<&str>,
) -> Result<(), VmError> {
    let this = match (decl.is_static, decl.owner) {
        (false, Some(owner)) => {
            let object = crate::interp::instantiate_from_defaults(vm, owner)?;
            if let Some(name) = state {
                let id = vm.space.names.intern(name);
                vm.space.heap.get_mut(object)?.state = Some(id);
            }
            Some(object)
        }
        _ => None,
    };
    let mut args = Vec::with_capacity(decl.params.len());
    for param in &decl.params {
        args.push(zero_argument(vm, param)?);
    }
    vm.call(function, this, args).map(|_| ())
}

#[cfg(test)]
mod tests {
    use ow_core::ParityMode;

    use super::*;
    use crate::census::toy;

    fn sweep(t: &mut toy::ToyVm, options: &SweepOptions) -> SweepReport {
        let source =
            t.vm.defs
                .code_mut()
                .1
                .load_path(&t.dir.join("Core.upk"))
                .unwrap();
        sweep_package(&mut t.vm, source, options)
    }

    #[test]
    fn every_script_function_runs_once_and_a_failed_decode_is_skipped_later() {
        let mut t = toy::vm(ParityMode::Off);
        let r = sweep(&mut t, &SweepOptions::default());
        // SumTo, Foo.Get, Bar.Get, CallGet, CallMystery, NoneCtx, GetCount, Link, CallsBad, Twice; the natives are
        // skipped, and Bad, whose script CallsBad's call failed to decode, counts as undecodable.
        assert_eq!(
            (r.package.as_str(), r.attempted, r.succeeded),
            ("Core", 10, 9)
        );
        assert_eq!((r.undecodable_skipped, r.no_owner_skipped), (1, 0));
        assert_eq!(r.failures.len(), 1);
        assert_eq!(r.failures[0].count, 1);
        let example = &r.failures[0].example;
        assert!(
            example.starts_with("Core.Foo.CallsBad: cannot run Core.Foo.Bad: "),
            "{example}"
        );
        assert_eq!(r.unimplemented_natives.len(), 1);
        let stub = &r.unimplemented_natives[0];
        assert_eq!(
            (stub.functions_reaching, stub.native.as_str()),
            (1, "Object.Mystery(int)")
        );
        // SumTo(0) runs its loop test once: the slowest is still one of the short functions, decided by steps.
        assert!(r.slowest.1 > 0 && r.total_steps >= r.slowest.1);
    }

    #[test]
    fn limit_class_and_top_narrow_the_sweep() {
        let mut t = toy::vm(ParityMode::Off);
        let limited = SweepOptions {
            limit: 3,
            ..SweepOptions::default()
        };
        assert_eq!(sweep(&mut t, &limited).attempted, 3);
        let mut t = toy::vm(ParityMode::Off);
        let bar = SweepOptions {
            class_filter: "Bar".into(),
            ..SweepOptions::default()
        };
        let r = sweep(&mut t, &bar);
        assert_eq!((r.attempted, r.succeeded), (1, 1));
        let mut t = toy::vm(ParityMode::Off);
        let none = SweepOptions {
            top: 0,
            ..SweepOptions::default()
        };
        let r = sweep(&mut t, &none);
        assert!(r.failures.is_empty() && r.unimplemented_natives.is_empty());
        assert_eq!(r.failed(), 1);
    }

    /// The release rule on hand-made objects of `Core.Foo` (its `Other` property is a `Foo`).
    #[test]
    fn the_release_rule_counts_live_holders_and_frees_the_rest() {
        use ow_object::heap::Origin;
        use ow_object::ids::{ObjRef, PropAddr};
        let mut t = toy::vm(ParityMode::Off);
        let vm = &mut t.vm;
        let root = vm.find_class("Core.Object").unwrap();
        let foo = vm.find_class("Core.Foo").unwrap();
        let other = PropAddr::scalar(vm.space.prop(foo, "Other").unwrap());
        let package = {
            let (code, store) = vm.defs.code_mut();
            code.package(store, "Core").unwrap()
        };
        let make = |vm: &mut Vm, name: &str, origin: Origin| -> ObjRef {
            vm.space.alloc(foo, name, None, origin).unwrap()
        };
        let link = |vm: &mut Vm, from: ObjRef, to: ObjRef| {
            vm.space
                .write(from, other, Value::Object(Some(to)))
                .unwrap()
        };
        let export = Origin::Export { package, index: 1 };
        // A loaded definition holds R1, and R1 holds R2: both stay.
        let holder = make(vm, "Holder", export);
        let (r1, r2) = (
            make(vm, "R1", Origin::Runtime),
            make(vm, "R2", Origin::Runtime),
        );
        link(vm, holder, r1);
        link(vm, r1, r2);
        // Nothing holds Lone; only a class default object holds ByDefault (not a holder the sweep can see).
        let lone = make(vm, "Lone", Origin::Runtime);
        let default = make(vm, "Default__Foo", Origin::Default { class: foo });
        let by_default = make(vm, "ByDefault", Origin::Runtime);
        link(vm, default, by_default);
        // A destroyed object and a resource stand-in are neither freed nor counted as holders.
        let dead = make(vm, "Dead", Origin::Runtime);
        let by_dead = make(vm, "ByDead", Origin::Runtime);
        link(vm, dead, by_dead);
        vm.space.heap.destroy(dead).unwrap();
        let resource = make(vm, "Resource", Origin::Resource { package, index: 1 });
        let by_resource = make(vm, "ByResource", Origin::Runtime);
        link(vm, resource, by_resource);
        let before = vm.space.heap.len();
        release_unreferenced(vm, root);
        for kept in [holder, r1, r2, default, dead, resource] {
            assert!(vm.space.heap.get(kept).is_ok(), "{kept:?} was freed");
        }
        assert_eq!(
            vm.space.read(holder, other).unwrap(),
            &Value::Object(Some(r1))
        );
        assert_eq!(vm.space.read(r1, other).unwrap(), &Value::Object(Some(r2)));
        for freed in [lone, by_default, by_dead, by_resource] {
            assert!(vm.space.heap.get(freed).is_err(), "{freed:?} was not freed");
        }
        assert_eq!(vm.space.heap.len(), before - 4);
        // Freeing cleared the references the uncounted holders had.
        for holder in [default, dead, resource] {
            assert_eq!(vm.space.read(holder, other).unwrap(), &Value::Object(None));
        }
        assert!(vm.space.heap.get(dead).unwrap().destroyed);
    }

    #[test]
    fn objects_nothing_references_are_released_and_a_cycle_stays() {
        let mut t = toy::vm(ParityMode::Off);
        sweep(&mut t, &SweepOptions::default());
        // Every attempt made one Foo or Bar (Twice is static); only Link's points at itself through Other.
        let foo = t.vm.find_class("Core.Foo").unwrap();
        let live: Vec<_> = t.vm.space.live(foo).collect();
        assert_eq!(live.len(), 1);
        let other = t.vm.space.prop(foo, "Other").unwrap();
        let held =
            t.vm.space
                .read(live[0], ow_object::ids::PropAddr::scalar(other))
                .unwrap();
        assert_eq!(held, &Value::Object(Some(live[0])));
    }

    #[test]
    fn category_drops_digits_and_keeps_ninety_bytes() {
        assert_eq!(
            category("jump target 9472 is not"),
            "jump target #### is not"
        );
        let long = "x".repeat(100);
        assert_eq!(category(&long).len(), 90);
    }

    #[test]
    fn ranking_is_descending_by_count_then_key() {
        let mut counts = BTreeMap::new();
        counts.insert("a".to_string(), 2);
        counts.insert("b".to_string(), 2);
        counts.insert("c".to_string(), 5);
        counts.insert("d".to_string(), 1);
        assert_eq!(
            ranked(&counts, 3),
            vec![(5, "c".into()), (2, "b".into()), (2, "a".into())]
        );
    }
}
