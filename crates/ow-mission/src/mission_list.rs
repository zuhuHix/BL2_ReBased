//! Owner: lane IMPL-MISSIONLIST-BUILD (decision 2026-11-19-108, number pending). The mission tracker's `MissionList`
//! build and the mission numbering, in Off. AI-assisted (Claude Code).
//!
//! Coded from NATIVE_MISSION_LIST_BUILD.md alone (analyst lane ANALYST-MISSIONLIST-BUILD, not this thread), with the
//! hub's three rulings of 2026-11-19 (port Off answers, UNVERIFIED). No native code or analysis output was opened.
//! **Every rule is UNVERIFIED in the running game.** Reference builds nothing (the C++ has no list).
//!
//! **When** (hub ruling 1): once, at the mission bridge's world set-up, right after the tracker is made and before any
//! script runs or reads the list. A second build request is `NotYet { NotRead }` ([`SECOND_BUILD`]).
//!
//! **Which definitions** (hub ruling 2, the port's stand-in for "every `MissionDefinition` in memory"): every export
//! whose class is exactly `WillowGame.MissionDefinition` (no subclass is declared in the installed packages),
//! `Default__` objects left out, in every package the store has loaded when the build starts, each loaded there as its
//! own definition (`DefStore::find`). That load is part of set-up, before any script runs, and counts as a write (the
//! hub's ruling on review r1 of #496): the equal-key check comes before it, and every `NotYet` after the first load
//! is marked "stopped after partial writes". A definition the build reads that is still an unloaded handle after that
//! (a dependency, a chain member, an expansion) is `NotYet { NotRead }` ([`NEEDS_LOAD`]), marked so, before any field
//! write. A loaded `Dependencies` or `NextMissionInChain` target that is not a `MissionDefinition` is bad data.
//!
//! **The order** (hub ruling 3, fitted, not proven: UNVERIFIED): the engine's name-table order is approximated by
//! first insertion. Each name text gets the index of its first appearance when the loaded packages' name tables are
//! read in load order, each table in its own order. Names the engine registers itself come before any package name in
//! the game; the port has no list of them, and none is taken to name a mission. Then the instance number. Two
//! definitions with the same name and number are `NotYet { NotRead }` ([`TIE`]): where the game's binary search puts
//! the second was not read.
//!
//! **The build** (section 2), for each definition in list order: `bGameStageLocked` cleared, `MissionNumber` -1,
//! `GameStage` 0 (on the shared definition objects), then one record (every field zero or None, `Status` 0) with its
//! `MissionDef`. **The numbering** (section 3) writes `MissionNumber` only.
//!
//! **A world without a list:** a tracker class that declares no `MissionList` builds nothing (the installed packages
//! declare it; invented package sets may not), and the bridge keeps answering `GetAllMissions` with its not-built
//! `NotYet`.
//!
//! **A set-up refusal** (the hub's steer of 2026-11-19): a `NotYet` of the build does not fail the bridge's set-up. The
//! bridge records it, kind and record kept, and `GetAllMissions` answers with that same `NotYet` at the first list read.
//! Every other error fails the set-up as before.
//!
//! **Order of work** (the hub rules): every read, every check and the whole numbering are computed before the first
//! field write, so every `NotYet` and every bad-data error leaves the definitions' fields and the list unchanged (the
//! set's loads, which come first, stay). Bad data is a
//! `VmError::Script` naming what is wrong: a missing or mistyped field, a `NextMissionInChain` loop, a cycle among side
//! missions' dependencies (section 4, step 3). The writes then follow in the game's order: the resets and the list,
//! then the numbers.

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The record these rules come from.
pub const RECORD: &str = "docs/verification/NATIVE_MISSION_LIST_BUILD.md";
/// The definition class the build collects.
pub const DEFINITION_CLASS: &str = "WillowGame.MissionDefinition";
/// The `NotYet` of a second build in one world.
pub const SECOND_BUILD: &str = "a second run of the tracker's list build";
/// The `NotYet` of a definition that would still need loading after set-up.
pub const NEEDS_LOAD: &str = "a reference the set-up did not load";
/// The `NotYet` of two definitions with the same name and number.
pub const TIE: &str = "the visit order of equal mission names";
/// The `NotYet` of the reorder pass's edge (section 3, step 6).
pub const EDGE: &str = "a dependency that stands after its mission in the tracker's reorder pass";

const UNIT: &str = "MissionTracker list build";

fn bad(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

fn field(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| bad(format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| bad(format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// The definitions the build collects, each with its order key, in load order (module documentation).
struct Candidate {
    path: String,
    key: (u64, i32),
}

/// The name text and instance number of an export's name as the package table stores it (`Name_N-1` for number
/// N > 0): a text the package's own name table holds is number 0; otherwise a trailing `_<digits>` is the number
/// minus one.
fn split_name(names: &[String], name: &str) -> (String, i32) {
    if names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        return (name.to_string(), 0);
    }
    if let Some((base, digits)) = name.rsplit_once('_')
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && let Ok(n) = digits.parse::<i32>()
        && names.iter().any(|t| t.eq_ignore_ascii_case(base))
    {
        return (base.to_string(), n.saturating_add(1));
    }
    (name.to_string(), 0)
}

/// Every candidate in the loaded packages, with its first-insertion key (module documentation).
fn candidates(vm: &Vm) -> Result<Vec<Candidate>, VmError> {
    let store = vm.defs.packages();
    let mut first: ow_core::DetMap<String, u64> = ow_core::DetMap::new();
    let mut out = Vec::new();
    for id in 0..store.loaded_count() {
        let package = store.get(u32::try_from(id).map_err(|_| bad("too many packages".to_string()))?);
        for n in &package.names {
            let next = first.len() as u64;
            first.entry(n.to_ascii_lowercase()).or_insert(next);
        }
        for (i, e) in package.exports.iter().enumerate() {
            if e.class == 0 || e.name.starts_with("Default__") {
                continue;
            }
            let class_path = if e.class < 0 {
                package.path_of(e.class).map_err(|err| bad(format!("{}: {err}", package.name)))?
            } else {
                let own = package.path_of(e.class).map_err(|err| bad(format!("{}: {err}", package.name)))?;
                format!("{}.{own}", package.name)
            };
            if !class_path.eq_ignore_ascii_case(DEFINITION_CLASS) {
                continue;
            }
            let reference = i32::try_from(i + 1).map_err(|_| bad("export index past i32".to_string()))?;
            let path = package.path_of(reference).map_err(|err| bad(format!("{}: {err}", package.name)))?;
            let (text, number) = split_name(&package.names, &e.name);
            let index = *first.get(&text.to_ascii_lowercase()).unwrap_or(&u64::MAX);
            out.push(Candidate { path: format!("{}.{path}", package.name), key: (index, number) });
        }
    }
    Ok(out)
}

/// One definition as the numbering reads it.
struct Def {
    obj: ObjRef,
    plot: bool,
    deps: Vec<Option<ObjRef>>,
    next: Option<ObjRef>,
    dlc: Option<ObjRef>,
}

fn object(v: Value, what: &str) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) => Ok(o),
        other => Err(bad(format!("{what} holds {}, not an object", other.kind_name()))),
    }
}

fn loaded(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        return Err(NotYet::not_read(NEEDS_LOAD, RECORD).into());
    }
    Ok(())
}

/// A loaded `Dependencies` or `NextMissionInChain` target must be a `MissionDefinition` (section 4: bad data otherwise,
/// review r1 of #496).
fn mission_target(vm: &Vm, o: ObjRef, field_name: &str) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let is_mission = vm.space.classes.by_path(DEFINITION_CLASS).is_some_and(|m| vm.space.classes.is_child_of(class, m));
    if !is_mission {
        return Err(bad(format!(
            "{field_name} names {}, not a MissionDefinition",
            ow_attr::eval::class_path(&vm.space, o)
        )));
    }
    Ok(())
}

fn read_def(vm: &Vm, obj: ObjRef) -> Result<Def, VmError> {
    let plot = match field(vm, obj, "bPlotCritical")? {
        Value::Bool(b) => b,
        other => return Err(bad(format!("bPlotCritical holds {}", other.kind_name()))),
    };
    for (name, want) in [("bGameStageLocked", "a bool"), ("MissionNumber", "an int"), ("GameStage", "an int")] {
        let v = field(vm, obj, name)?;
        if !matches!((name, &v), ("bGameStageLocked", Value::Bool(_)) | ("MissionNumber" | "GameStage", Value::Int(_))) {
            return Err(bad(format!("{name} holds {}, not {want}", v.kind_name())));
        }
    }
    let deps = match field(vm, obj, "Dependencies")? {
        Value::Array(a) => a.iter().map(|v| object(v.clone(), "Dependencies")).collect::<Result<Vec<_>, _>>()?,
        other => return Err(bad(format!("Dependencies holds {}, not an array", other.kind_name()))),
    };
    let next = object(field(vm, obj, "NextMissionInChain")?, "NextMissionInChain")?;
    let dlc = object(field(vm, obj, "DlcExpansion")?, "DlcExpansion")?;
    for o in deps.iter().flatten().chain(next.iter()).chain(dlc.iter()) {
        loaded(vm, *o)?;
    }
    for o in deps.iter().flatten() {
        mission_target(vm, *o, "Dependencies")?;
    }
    if let Some(o) = next {
        mission_target(vm, o, "NextMissionInChain")?;
    }
    Ok(Def { obj, plot, deps, next, dlc })
}

/// The numbers the numbering gives, by list index (-1 for unnumbered), computed without writing (section 3).
fn numbering(vm: &Vm, defs: &[Def]) -> Result<Vec<i32>, VmError> {
    let index_of = |o: ObjRef| defs.iter().position(|d| d.obj == o);
    // Every target is a loaded MissionDefinition (read_def), and every loaded MissionDefinition export is in the list:
    // a miss is inconsistent data.
    let inside = |o: ObjRef| index_of(o).ok_or_else(|| bad("a mission the numbering reaches is not in the built list".to_string()));
    let mut number = vec![-1i32; defs.len()];
    // The groups by expansion, in list order of first appearance.
    let mut groups: Vec<Option<ObjRef>> = Vec::new();
    for d in defs {
        if !groups.contains(&d.dlc) {
            groups.push(d.dlc);
        }
    }
    let mut counters = Vec::with_capacity(groups.len());
    // 1. Each group's main chain.
    for g in &groups {
        let mut counter = match g {
            None => 0,
            Some(e) => match field(vm, *e, "MissionNumberBase")? {
                Value::Int(i) => i,
                other => return Err(bad(format!("MissionNumberBase holds {}", other.kind_name()))),
            },
        };
        if let Some(start) = defs.iter().position(|d| d.dlc == *g && d.plot && d.deps.is_empty()) {
            let mut cur = Some(start);
            let mut seen = vec![false; defs.len()];
            while let Some(i) = cur {
                if seen[i] {
                    return Err(bad("a NextMissionInChain loop".to_string()));
                }
                seen[i] = true;
                number[i] = counter;
                counter = counter.saturating_add(1);
                cur = match defs[i].next {
                    Some(n) => Some(inside(n)?),
                    None => None,
                };
            }
        }
        counters.push(counter);
    }
    // 2. Each group's side missions: anchors, buckets, the second pass, the numbers.
    for (gi, g) in groups.iter().enumerate() {
        let members: Vec<usize> = (0..defs.len()).filter(|&i| defs[i].dlc == *g).collect();
        let top = counters[gi];
        let buckets_len = usize::try_from(top.max(-1).saturating_add(1)).unwrap_or(0);
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); buckets_len];
        let mut memo: Vec<Option<i32>> = vec![None; defs.len()];
        for &m in members.iter().filter(|&&m| !defs[m].plot) {
            let mut visiting = vec![false; defs.len()];
            let a = anchor(defs, &number, m, &mut memo, &mut visiting, &inside)?;
            let Ok(slot) = usize::try_from(a) else { continue };
            if slot >= buckets.len() {
                continue;
            }
            let side = |o: Option<ObjRef>| -> Result<Option<usize>, VmError> {
                match o {
                    Some(o) => {
                        let i = inside(o)?;
                        Ok((!defs[i].plot).then_some(i))
                    }
                    None => Ok(None),
                }
            };
            let bucket = &buckets[slot];
            let mut best: Option<usize> = None;
            for dep in &defs[m].deps {
                let Some(mut w) = side(*dep)? else { continue };
                let mut steps = 0usize;
                loop {
                    if let Some(p) = bucket.iter().position(|&x| x == w) {
                        best = Some(best.map_or(p, |b| b.max(p)));
                        break;
                    }
                    steps += 1;
                    if steps > defs.len() {
                        return Err(bad("a cycle among side missions' first dependencies".to_string()));
                    }
                    match defs[w].deps.first().copied().flatten() {
                        Some(o) => match side(Some(o))? {
                            Some(i) => w = i,
                            None => break,
                        },
                        None => break,
                    }
                }
            }
            let at = match best {
                Some(p) => p + 1,
                None => {
                    let mut found = None;
                    for (p, &x) in bucket.iter().enumerate() {
                        if depends_on(defs, x, m, &side)? {
                            found = Some(p);
                            break;
                        }
                    }
                    found.unwrap_or(bucket.len())
                }
            };
            buckets[slot].insert(at, m);
        }
        // The second pass (section 3, step 6).
        for bucket in &mut buckets {
            let multi: Vec<usize> = bucket.iter().copied().filter(|&x| defs[x].deps.len() >= 2).collect();
            for m in multi {
                let mut cands: Vec<usize> = Vec::new();
                for dep in defs[m].deps.iter().flatten() {
                    let d = inside(*dep)?;
                    if !bucket.contains(&d) || cands.contains(&d) {
                        continue;
                    }
                    let others_depend = bucket
                        .iter()
                        .any(|&y| y != m && defs[y].deps.iter().flatten().any(|o| *o == defs[d].obj));
                    if !others_depend {
                        cands.push(d);
                    }
                }
                if cands.is_empty() {
                    continue;
                }
                let pos = |b: &Vec<usize>, x: usize| b.iter().position(|&y| y == x).unwrap_or(usize::MAX);
                let at_m = pos(bucket, m);
                if cands.iter().any(|&c| pos(bucket, c) > at_m) {
                    return Err(NotYet::not_read(EDGE, RECORD).into());
                }
                cands.sort_by_key(|&c| pos(bucket, c));
                bucket.retain(|x| !cands.contains(x));
                let at = pos(bucket, m);
                for (k, c) in cands.into_iter().enumerate() {
                    bucket.insert(at + k, c);
                }
            }
        }
        let mut counter = top;
        for bucket in &buckets {
            for &m in bucket {
                number[m] = counter;
                counter = counter.saturating_add(1);
            }
        }
    }
    Ok(number)
}

/// The anchor of side mission `m` (section 3, step 4): the largest number among the plot-critical missions it depends
/// on, directly or through other side missions; -1 with none. A cycle among side missions is bad data.
fn anchor(
    defs: &[Def],
    number: &[i32],
    m: usize,
    memo: &mut [Option<i32>],
    visiting: &mut [bool],
    inside: &dyn Fn(ObjRef) -> Result<usize, VmError>,
) -> Result<i32, VmError> {
    if let Some(a) = memo[m] {
        return Ok(a);
    }
    if visiting[m] {
        return Err(bad("a cycle among side missions' dependencies".to_string()));
    }
    visiting[m] = true;
    let mut best = -1;
    for dep in defs[m].deps.iter().flatten() {
        let d = inside(*dep)?;
        let a = if defs[d].plot { number[d] } else { anchor(defs, number, d, memo, visiting, inside)? };
        best = best.max(a);
    }
    visiting[m] = false;
    memo[m] = Some(best);
    Ok(best)
}

/// Whether `x` depends on `m` directly, or through its chain of first dependencies, side missions only.
fn depends_on(
    defs: &[Def],
    x: usize,
    m: usize,
    side: &dyn Fn(Option<ObjRef>) -> Result<Option<usize>, VmError>,
) -> Result<bool, VmError> {
    if defs[x].deps.iter().flatten().any(|o| *o == defs[m].obj) {
        return Ok(true);
    }
    let mut w = side(defs[x].deps.first().copied().flatten())?;
    let mut steps = 0usize;
    while let Some(i) = w {
        if i == m {
            return Ok(true);
        }
        steps += 1;
        if steps > defs.len() {
            return Err(bad("a cycle among side missions' first dependencies".to_string()));
        }
        w = side(defs[i].deps.first().copied().flatten())?;
    }
    Ok(false)
}

/// The tracker's list build at set-up (module documentation). `ran` is the bridge's record that the set-up build ran:
/// it is set on entry, whatever the outcome (a refusal included), so every later request is [`SECOND_BUILD`] (review
/// r1 of #496).
pub fn build(vm: &mut Vm, tracker: ObjRef, ran: &mut bool) -> Result<(), VmError> {
    if *ran {
        return Err(NotYet::not_read(SECOND_BUILD, RECORD).into());
    }
    *ran = true;
    // The list's declaration, before anything is loaded.
    let class = vm.space.heap.get(tracker)?.class;
    // A tracker class that declares no list (an invented package set without one) has nothing to build: the world
    // keeps no list, and `GetAllMissions` answers its not-built `NotYet`.
    let Some(p) = vm.space.prop(class, "MissionList") else {
        return Ok(());
    };
    let PropKind::Array(inner) = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad("MissionList has no declaration".to_string()))? else {
        return Err(bad("MissionList is not an array".to_string()));
    };
    let PropKind::Struct(record_ty) = *inner else {
        return Err(bad("MissionList is not an array of structs".to_string()));
    };
    // The set (hub ruling 2) and the order (hub ruling 3): the keys and paths are known before anything is loaded, so
    // the equal-key check comes first (review r1 of #496, the hub's ruling).
    let mut found = dedupe(candidates(vm)?);
    found.sort_by_key(|c| c.key);
    if found.windows(2).any(|w| w[0].key == w[1].key) {
        return Err(NotYet::not_read(TIE, RECORD).into());
    }
    if found.is_empty() {
        return writes(vm, tracker, &[], &[], &record_zero(vm, record_ty)?, 0).map(|_| ());
    }
    // The loads count as writes (the hub's ruling): from the first one on, a `NotYet` is terminal, marked "stopped
    // after partial writes" (kind and record kept) through `ow_world::pools::after_partial_writes`; other errors pass
    // unchanged; nothing is rolled back.
    after_loads(vm, tracker, record_ty, &found).map_err(ow_world::pools::after_partial_writes)
}

/// The candidates with each object path once, compared with ASCII case folded (review r2 of #496): one package listed
/// twice names the same objects, which is not a tie of two definitions. The first of each is kept, in load order.
fn dedupe(found: Vec<Candidate>) -> Vec<Candidate> {
    let mut seen: Vec<String> = Vec::with_capacity(found.len());
    let mut out = Vec::with_capacity(found.len());
    for c in found {
        let folded = c.path.to_ascii_lowercase();
        if !seen.contains(&folded) {
            seen.push(folded);
            out.push(c);
        }
    }
    out
}

fn record_zero(vm: &Vm, record_ty: ow_object::ids::StructId) -> Result<Value, VmError> {
    vm.space
        .classes
        .struct_layout(record_ty)
        .map(|l| l.zero().clone())
        .ok_or_else(|| bad("the record struct has no layout".to_string()))
}

/// The build from the first load on (module documentation).
fn after_loads(vm: &mut Vm, tracker: ObjRef, record_ty: ow_object::ids::StructId, found: &[Candidate]) -> Result<(), VmError> {
    let mut set: Vec<ObjRef> = Vec::with_capacity(found.len());
    for c in found {
        let path = ow_core::ObjectPath::parse(&c.path).map_err(|e| bad(format!("{}: {e}", c.path)))?;
        let o = vm.defs.find(&mut vm.space, &path);
        vm.log.extend(vm.defs.take_log());
        // An enumerated export the store cannot find is inconsistent data, not a silent drop (review r1 of #496).
        let o = o.map_err(VmError::from)?.ok_or_else(|| bad(format!("{}: listed in its package, not found by the store", c.path)))?;
        set.push(o);
    }
    // Every read and the numbering, before the field writes.
    let defs: Vec<Def> = set.iter().map(|o| read_def(vm, *o)).collect::<Result<_, _>>()?;
    let numbers = numbering(vm, &defs)?;
    let zero = record_zero(vm, record_ty)?;
    let def_field = record_field(vm, &zero, "MissionDef")?;
    // The writes, in the game's order: the resets and the records, then the numbers. They go to this world's own
    // definition objects (its VM's object space), so nothing reaches another world.
    writes(vm, tracker, &defs, &numbers, &zero, def_field)
}

fn writes(
    vm: &mut Vm,
    tracker: ObjRef,
    defs: &[Def],
    numbers: &[i32],
    zero: &Value,
    def_field: usize,
) -> Result<(), VmError> {
    let mut records = Vec::with_capacity(defs.len());
    for d in defs {
        write(vm, d.obj, "bGameStageLocked", Value::Bool(false))?;
        write(vm, d.obj, "MissionNumber", Value::Int(-1))?;
        write(vm, d.obj, "GameStage", Value::Int(0))?;
        let mut r = zero.clone();
        if let Some(s) = r.struct_mut() {
            s.fields[def_field] = Value::Object(Some(d.obj));
        }
        records.push(r);
    }
    write(vm, tracker, "MissionList", Value::array(records))?;
    for (d, n) in defs.iter().zip(numbers) {
        if *n != -1 {
            write(vm, d.obj, "MissionNumber", Value::Int(*n))?;
        }
    }
    Ok(())
}

fn record_field(vm: &Vm, v: &Value, name: &str) -> Result<usize, VmError> {
    let missing = || bad(format!("the record struct declares no {name}"));
    let ty = v.as_struct().map(|s| s.ty).ok_or_else(missing)?;
    let id = vm.space.names.find(name).ok_or_else(missing)?;
    let (index, _) = vm.space.classes.struct_layout(ty).and_then(|l| l.field(id)).ok_or_else(missing)?;
    usize::try_from(index).map_err(|_| missing())
}

/// The definitions of the built list, in its order (what `GetAllMissions` appends).
pub fn missions(vm: &Vm, tracker: ObjRef) -> Result<Vec<Value>, VmError> {
    let list = field(vm, tracker, "MissionList")?;
    let Some(records) = list.elements() else { return Err(bad("MissionList is not an array".to_string())) };
    let mut out = Vec::with_capacity(records.len());
    for r in records {
        let i = record_field(vm, r, "MissionDef")?;
        out.push(r.as_struct().and_then(|s| s.fields.get(i).cloned()).unwrap_or(Value::Object(None)));
    }
    Ok(out)
}

#[cfg(test)]
#[path = "mission_list_tests.rs"]
mod tests;
