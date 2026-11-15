//! Owner: lane W4-VMTOOLS. The native census and the native self-test: the port of `src/census.*` and of
//! `nativeSelfTest` (`src/natives.cpp`). AI-assisted (Claude Code).
//!
//! **What the census measures.** Which natives does a list of script entry points reach? Two estimates over the same
//! entries, neither a measurement of the real game:
//! - dynamic: each entry runs on the VM (objects built from class defaults, every native without an implementation
//!   a logged zero-result stub) and the VM's call census says which functions were entered and how often;
//! - static: the script functions and natives reachable through the decoded bytecode's final, virtual, global and
//!   native calls. A virtual call is resolved over the receiver's static class and every subclass that overrides the
//!   name, so that part is an upper bound; a receiver whose class cannot be inferred is listed as unresolved, not
//!   guessed. Closure A follows the primary edges (the function a call names, or the version visible at the
//!   receiver's static class), closure B adds the upper-bound edges (subclass overrides, interface implementers).
//!
//! **The entry file** (text; `#` starts a comment; tokens are separated by C-locale white space):
//! - `new <label> <Package.Class>`: an object built from the class defaults, named `<label>`;
//! - `export <label> <Package> <ObjectPath>`: in the C++, a new object named after the installed export, with its
//!   data (`instantiateExport(pkg, index, 4)`). Here `NotYet` after the package and export lookups (below);
//! - `set <path> <value>`, `add <path> <value>`: assign a property, append to an array property (a path is
//!   `$label.Property` then any `.Field` and `[index]` steps into structs and arrays that already exist);
//! - `run <Package.Class.Function> <self> [<value>...]`: one entry; `self` is `$label` or `-` (no object: the owner's
//!   class default object);
//! - `runclass <Package.Class>`: one entry per script function the class itself declares, no arguments.
//!
//! A value is `$label`, `none`, `class:<Package.Class>`, `struct:<Package.Struct path>` (its defaults), `i:`, `f:`,
//! `b:` (`1` or `true`), `y:`, `s:`, `n:`. Objects and their state persist from one entry to the next, in file order.
//!
//! **Where the Rust port differs from the C++ one** (each a representation rule of the typed heap, decision
//! 2026-11-15-02, not a game rule):
//! - `set` and `add` write through the heap's checked writes, so a value of the wrong kind for the property is an
//!   error (the C++ stores any value into its untyped bag, and `add` on a property that is not an array replaces it
//!   with an array). Before the write, `none` becomes the typed null of a class, interface or delegate property and
//!   an object becomes an interface reference for an interface property, as the interpreter's own stores do.
//! - A static-array property is consecutive heap slots, not one array value, so `$o.Arr[i]` addresses element `i`
//!   of the property.
//! - `struct:` of a struct that is not loaded is `NotYet` (a struct is defined when a class or function that uses it
//!   loads; the C++ `structAt` loads it).
//! - `f:` with hexadecimal float text is `NotYet` (no parser for it is written).
//! - `export` is `NotYet`: the definition store holds the export's object under its path, so a copy, as the C++
//!   makes, would leave two live objects where the C++ has one, and the engine iterators would visit both.
//!
//! A `NotYet` comes back as [`CensusError::Vm`] (see [`CensusError::not_yet`]); the command exits with code 3 for it.
//!
//! **The static walk** reads the bytecode itself (`ow_script::decode::decode`, the decoder the interpreter uses) and
//! types receivers from the declarations the definition store's `CodeIndex` gives
//! (`CodeIndex::child_declarations`), finding every function by its path (`Vm::find_function`), as the C++ walks the
//! runtime's own tables. A class the reflection made as an engine-native stand-in (decision 2026-11-15-08) gives no
//! receiver class, as the C++ `classAt` gives none for a class with no export.
//!
//! Status: a port of the C++ tool (decision 2026-11-15-23). The census measures the interpreter and asserts no game
//! behaviour; its counts say what the VM did, not what the game does.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ow_core::num::strtod_prefix;
use ow_core::{FixedHashMap, NotYet, ObjectPath};
use ow_object::heap::{HeapError, Step};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::reflect::{CODE_PACKAGES, ClassKey};
use ow_object::value::Value;
use ow_package::PackageId;
use ow_script::decode::{Code, Expr};
use ow_script::function::{FunctionInfo, is_function_export, read_function};
use ow_script::ops::flags::{FUNC_NATIVE, FUNC_OPERATOR, FUNC_PRE_OPERATOR};
use ow_script::ops::op;

use crate::api::{CallCensus, FunctionDecl, StubPolicy, Vm};
use crate::error::VmError;
use crate::sweep::what;

// ------------------------------------------------------------------------------------------------- self-test

/// What `--native-selftest` checked (the C++ summary's counts).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfTest {
    /// Natives registered and called.
    pub registered: usize,
    /// Unbound natives called as a logged stub.
    pub unimplemented: usize,
    /// A second registration of one key was refused.
    pub duplicate_rejected: bool,
}

/// The self-test of the native registry (C++ `nativeSelfTest`, here on the Rust VM's registry): a native registered
/// under a key is found and called, an unknown key is not found, an unbound native runs as the logged stub of
/// `StubPolicy::LogAndZero` with its zero result, and a second registration of a key is refused. Loads no package.
/// The error names the check that failed.
pub fn native_self_test() -> Result<SelfTest, String> {
    let defs = ow_defs::DefStore::new_empty();
    let mut vm = Vm::new(
        ow_object::heap::ObjectSpace::new(),
        defs,
        Rc::new(ow_attr::AttributeSystem::new_empty()),
        ow_core::ParityMode::Off,
    )
    .with_stub_policy(StubPolicy::LogAndZero);
    let path = |p: &str| ObjectPath::parse(p).map_err(|e| e.to_string());
    let echo: crate::api::NativeFn = Rc::new(|_| Ok(Value::string("a,b")));
    vm.register_native("Test.Echo()", echo.clone())
        .map_err(|e| e.to_string())?;
    if !vm.has_native("Test.Echo()") || vm.has_native("Test.Missing()") {
        return Err("registry lookup failed".into());
    }
    let registered = vm
        .declare_function(FunctionDecl::native(path("Test.Test.Echo")?, "Test.Echo()"))
        .map_err(|e| e.to_string())?;
    let called = vm
        .call(registered, None, Vec::new())
        .map_err(|e| e.to_string())?;
    if called.ret != Value::string("a,b") {
        return Err("registered dispatch failed".into());
    }
    let stub = vm
        .declare_function(FunctionDecl::native(
            path("WillowGame.WillowWeapon.Fire")?,
            "WillowWeapon.Fire()",
        ))
        .map_err(|e| e.to_string())?;
    vm.log.clear();
    let stubbed = vm.call(stub, None, Vec::new()).map_err(|e| e.to_string())?;
    if vm.log != ["UNIMPLEMENTED WillowWeapon.Fire()"] || stubbed.ret != Value::None {
        return Err("stub log mismatch".into());
    }
    let duplicate_rejected = vm.register_native("Test.Echo()", echo).is_err();
    if !duplicate_rejected {
        return Err("duplicate registration accepted".into());
    }
    Ok(SelfTest {
        registered: 1,
        unimplemented: 1,
        duplicate_rejected,
    })
}

// ------------------------------------------------------------------------------------------------- the report

/// What the census runs (C++ `CensusOptions`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CensusOptions {
    /// Executed steps per entry.
    pub step_limit: u64,
    /// Walk the bytecode for the static closure.
    pub static_closure: bool,
}

impl Default for CensusOptions {
    /// The C++ defaults: 2,000,000 steps per entry, the static closure on.
    fn default() -> Self {
        CensusOptions {
            step_limit: 2_000_000,
            static_closure: true,
        }
    }
}

/// The static closure counts of one entry, for closure A or B.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReachCounts {
    pub scripts: usize,
    pub natives: usize,
    pub natives_implemented: usize,
}

/// Closures A and B of one entry.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryReach {
    pub a: ReachCounts,
    pub b: ReachCounts,
}

/// One entry that ran (the C++ `Run`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CensusEntry {
    /// The function's path.
    pub function: String,
    /// The `self` token, or `new <Package.Class>` for a `runclass` entry.
    pub this: String,
    /// The argument tokens as written.
    pub args: Vec<String>,
    /// The function's parameter names.
    pub params: Vec<String>,
    /// `step limit`, `call depth limit` or `exception` when the entry stopped.
    pub stop_reason: Option<String>,
    /// The error text when the entry stopped.
    pub error: Option<String>,
    /// The result in `Value::describe` form (`struct` and `array` for those kinds); meaningful when completed.
    pub result: String,
    pub steps: u64,
    /// Path -> calls, for script functions entered, implemented natives, stubbed natives.
    pub scripts: BTreeMap<String, u64>,
    pub natives: BTreeMap<String, u64>,
    pub stubs: BTreeMap<String, u64>,
    /// Script function -> context expressions whose object was None.
    pub none_contexts: BTreeMap<String, u64>,
    /// Log lines other than the stub lines the counters cover, grouped with digits dropped (90 bytes each).
    pub log_categories: BTreeMap<String, u64>,
    /// The first 12 of those lines, 200 bytes each.
    pub log_head: Vec<String>,
    /// The static closure counts (none with the closure off).
    pub reach: Option<EntryReach>,
}

impl CensusEntry {
    pub fn completed(&self) -> bool {
        self.error.is_none()
    }
}

/// One script function the census saw.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptRow {
    pub dynamic_calls: u64,
    /// `A`, `B`, or `-` (entered by a run, in neither closure).
    pub static_closure: char,
}

/// One native the census saw, dynamically or statically.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRow {
    pub key: String,
    pub package: String,
    pub owner: String,
    pub name: String,
    pub native_index: u32,
    pub operator: bool,
    pub implemented: bool,
    /// Entry index -> calls.
    pub dynamic_by_entry: BTreeMap<usize, u64>,
    pub static_sites_a: u64,
    pub static_sites_b: u64,
    pub static_entries_a: Vec<usize>,
    pub static_entries_b: Vec<usize>,
    /// At most three callers in closure B, most sites first (equal counts: by path).
    pub top_callers: Vec<String>,
}

/// The static walk's own totals.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticSummary {
    pub script_functions_walked: usize,
    pub called_script_functions: usize,
    pub called_natives: usize,
    pub delegate_sites: u64,
    pub unresolved_virtual: BTreeMap<String, u64>,
    pub missing_method: BTreeMap<String, u64>,
    pub unknown_native_index: BTreeMap<u32, u64>,
    pub undecodable: BTreeSet<String>,
    /// Script functions the runs entered that closure B does not hold: a check on the walker.
    pub dynamic_outside_static: Vec<String>,
}

/// The census report (the C++ JSON's parts, each in key order).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CensusReport {
    pub entries: Vec<CensusEntry>,
    pub script_functions: BTreeMap<String, ScriptRow>,
    pub natives: BTreeMap<String, NativeRow>,
    /// None with the static closure off.
    pub static_summary: Option<StaticSummary>,
}

/// Why the census did not run to its report (C++ `nativeCensus` throws).
///
/// A script that fails while an entry runs is a stopped entry in the report, never this error.
#[non_exhaustive]
#[derive(Debug)]
pub enum CensusError {
    /// The entry file is malformed or names something the census cannot use: the C++ `RuntimeError` text
    /// (`entry file line <n>: ...` where the C++ says so).
    Entry(String),
    /// The VM refused a lookup or a value: a class or function not found, or `NotYet` for what the Rust port does not
    /// do yet (a struct that is not loaded, an `export` line, hexadecimal float text).
    Vm(VmError),
}

impl CensusError {
    /// The `NotYet` behind this error, if that is what it is (the command exits with code 3 then).
    pub fn not_yet(&self) -> Option<&NotYet> {
        match self {
            CensusError::Vm(VmError::NotYet(n)) => Some(n),
            _ => None,
        }
    }
}

impl std::fmt::Display for CensusError {
    /// The C++ `what()` text without the causes, which `source()` carries: a script error's own message, a
    /// definition store error's own text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CensusError::Entry(text) => f.write_str(text),
            CensusError::Vm(VmError::Script { message, .. }) => f.write_str(message),
            CensusError::Vm(VmError::Def(e)) => write!(f, "{e}"),
            CensusError::Vm(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CensusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CensusError::Entry(_) | CensusError::Vm(VmError::Script { .. }) => None,
            CensusError::Vm(VmError::Def(e)) => e.source(),
            CensusError::Vm(e) => e.source(),
        }
    }
}

impl From<VmError> for CensusError {
    fn from(e: VmError) -> Self {
        CensusError::Vm(e)
    }
}

impl From<NotYet> for CensusError {
    fn from(e: NotYet) -> Self {
        CensusError::Vm(VmError::NotYet(e))
    }
}

impl From<HeapError> for CensusError {
    fn from(e: HeapError) -> Self {
        CensusError::Vm(VmError::Heap(e))
    }
}

/// A message of the census itself (the C++ `RuntimeError` text).
impl From<String> for CensusError {
    fn from(text: String) -> Self {
        CensusError::Entry(text)
    }
}

// ------------------------------------------------------------------------------------------------ the census

/// A line of the entry file.
struct Line {
    words: Vec<String>,
    number: usize,
}

/// C-locale `isspace`: space, tab, newline, vertical tab, form feed, carriage return.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r')
}

fn fail(line: &Line, why: impl std::fmt::Display) -> CensusError {
    CensusError::Entry(format!("entry file line {}: {why}", line.number))
}

/// Digits replaced by `#` in the first 90 bytes (the C++ log category).
fn log_category(text: &str) -> String {
    crate::sweep::category(text)
}

/// The first `n` bytes of `text` (a cut through a UTF-8 sequence reads as U+FFFD, as for the categories).
fn head(text: &str, n: usize) -> String {
    if text.len() <= n {
        return text.to_string();
    }
    String::from_utf8_lossy(&text.as_bytes()[..n]).into_owned()
}

/// `std::stoll(text)` (base 10): the value of the longest prefix, after leading white space and a sign. Any
/// failure is `None` (the census reports "bad value").
fn stoll(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let mut at = 0;
    while at < b.len() && is_space(b[at] as char) {
        at += 1;
    }
    let start = at;
    if at < b.len() && (b[at] == b'+' || b[at] == b'-') {
        at += 1;
    }
    let digits = at;
    while at < b.len() && b[at].is_ascii_digit() {
        at += 1;
    }
    if at == digits {
        return None;
    }
    text[start..at].parse::<i64>().ok()
}

/// `std::stod(text)`: decimal text, `inf` and `nan` (`ow_core::num::strtod_prefix`); an overflow to an infinity
/// fails, as the MSVC `out_of_range`. Hexadecimal float text is `NotYet` (no parser for it is written).
fn stod(text: &str) -> Result<Option<f64>, NotYet> {
    let Some(prefix) = strtod_prefix(text) else {
        return Ok(None);
    };
    if prefix.hexadecimal {
        return Err(NotYet::not_ported(
            format!("hexadecimal float text in a census value: {text}"),
            "decision 2026-11-15-23 (census values)",
        ));
    }
    let lower = text[prefix.start..prefix.end].to_ascii_lowercase();
    let unsigned = lower.trim_start_matches(['+', '-']);
    let value = if unsigned.starts_with("inf") {
        f64::INFINITY
    } else if unsigned.starts_with("nan") {
        f64::NAN
    } else {
        match unsigned.parse::<f64>() {
            Ok(v) if v.is_finite() => v,
            _ => return Ok(None),
        }
    };
    Ok(Some(if lower.starts_with('-') {
        -value
    } else {
        value
    }))
}

/// `std::stoul` of an index: the MSVC exception texts, which the C++ census does not catch.
fn stoul(text: &str) -> Result<u64, String> {
    let b = text.as_bytes();
    let mut at = 0;
    while at < b.len() && is_space(b[at] as char) {
        at += 1;
    }
    let negative = at < b.len() && b[at] == b'-';
    if at < b.len() && (b[at] == b'+' || b[at] == b'-') {
        at += 1;
    }
    let digits = at;
    while at < b.len() && b[at].is_ascii_digit() {
        at += 1;
    }
    if at == digits {
        return Err("invalid stoul argument".into());
    }
    let value: u64 = text[digits..at]
        .parse()
        .map_err(|_| "stoul argument out of range".to_string())?;
    // `strtoul` negates a negative number in the unsigned type.
    Ok(if negative {
        value.wrapping_neg()
    } else {
        value
    })
}

/// A new object of `class` named `name` that starts from the class default object's values (C++
/// `Runtime::instantiate(cls, name)`); an empty name gives the VM's unnamed form.
fn instantiate_named(vm: &mut Vm, class: ClassId, name: &str) -> Result<ObjRef, VmError> {
    if name.is_empty() {
        return crate::interp::instantiate_from_defaults(vm, class);
    }
    let path = class_path(vm, class);
    let made = vm.defs.class_default(&mut vm.space, &path);
    vm.log.extend(vm.defs.take_log());
    let defaults = made?;
    let object = vm.instantiate(class, name)?;
    let slots = vm.space.heap.get(defaults)?.slots.clone();
    vm.space.heap.get_mut(object)?.slots = slots;
    Ok(object)
}

fn class_path(vm: &Vm, class: ClassId) -> String {
    vm.space
        .classes
        .get_layout(class)
        .map(|l| l.path().as_str().to_string())
        .unwrap_or_default()
}

fn class_name(vm: &Vm, class: ClassId) -> String {
    vm.space
        .classes
        .get_layout(class)
        .map(|l| l.path().name().to_string())
        .unwrap_or_default()
}

/// The C++ `Value` kept in a struct or array for `describeBrief`: `struct` and `array` for those kinds, the
/// `describe` text otherwise.
fn describe_brief(vm: &Vm, value: &Value) -> String {
    match value {
        Value::Struct(_) => "struct".into(),
        Value::Array(_) => "array".into(),
        other => crate::interp::describe(vm, other),
    }
}

/// `value` made fit for a property of kind `kind`, as the interpreter's stores do (module documentation).
fn fit(kind: &PropKind, value: Value) -> Value {
    match (kind, value) {
        (PropKind::Class { .. }, Value::Object(None)) => Value::Class(None),
        (PropKind::Interface { .. }, Value::Object(o)) => Value::Interface(o),
        (PropKind::Delegate, Value::Object(None)) => Value::Delegate(None),
        (PropKind::Object { .. }, Value::Interface(o)) => Value::Object(o),
        (_, v) => v,
    }
}

/// A place a `set` or `add` path names: the property element, the steps inside it and the kind there.
struct Target {
    object: ObjRef,
    addr: PropAddr,
    path: Vec<Step>,
    kind: PropKind,
}

/// The census's own state over one run.
struct Census<'a> {
    vm: &'a mut Vm,
    labels: BTreeMap<String, Value>,
    /// Packages whose name tables are interned ([`Census::intern_package_names`]).
    named: BTreeSet<PackageId>,
}

impl Census<'_> {
    /// Interns every name of the packages that hold `class` and its super classes, before an object is named
    /// after a label. The VM's name table folds case and keeps the first spelling it meets, as UE3 FNames do, and
    /// in UE3 a loaded package's name table is in the FName table before any script runs. Without this, a label
    /// such as `d` met first would respell a name `D` the package uses later (a state name, say), which the C++ census,
    /// holding names as strings, never does.
    fn intern_package_names(&mut self, class: ClassId) {
        let mut cursor = Some(class);
        let mut left = self.vm.space.classes.len();
        while let Some(c) = cursor.filter(|_| left > 0) {
            left -= 1;
            if let Some(key) = self.vm.defs.code().key_of(c) {
                let package = self.vm.defs.code().class(key).package;
                self.intern_names_of(package);
            }
            cursor = self
                .vm
                .space
                .classes
                .get_layout(c)
                .and_then(|l| l.super_class());
        }
    }

    fn intern_names_of(&mut self, package: PackageId) {
        if self.named.insert(package) {
            let pkg = self.vm.defs.packages().shared(package);
            for name in &pkg.names {
                self.vm.space.names.intern(name);
            }
        }
    }

    fn object(&self, line: &Line, token: &str) -> Result<ObjRef, CensusError> {
        let Some(label) = token.strip_prefix('$') else {
            return Err(fail(line, format!("expected $label, got {token}")));
        };
        match self.labels.get(label) {
            Some(Value::Object(Some(o))) => Ok(*o),
            _ => Err(fail(line, format!("unknown label {token}"))),
        }
    }

    fn value(&mut self, line: &Line, token: &str) -> Result<Value, CensusError> {
        if token == "none" {
            return Ok(Value::Object(None));
        }
        if token.starts_with('$') {
            return Ok(Value::Object(Some(self.object(line, token)?)));
        }
        let Some((kind, body)) = token.split_once(':') else {
            return Err(fail(line, format!("unknown value {token}")));
        };
        let bad = || fail(line, format!("bad value {token}"));
        Ok(match kind {
            // The C++ int and byte keep the low bits of the 64-bit number, as these casts do.
            "i" => Value::Int(stoll(body).ok_or_else(bad)? as i32),
            "f" => Value::Float(stod(body)?.ok_or_else(bad)? as f32), // f64 to f32: rounds, as the C++ float
            "b" => Value::Bool(body == "1" || body == "true"),
            "y" => Value::Byte(stoll(body).ok_or_else(bad)? as u8),
            "s" => Value::string(body),
            "n" => Value::Name(self.vm.space.names.intern(body)),
            "class" => Value::Class(Some(self.vm.find_class(body)?)),
            "struct" => self.new_struct(body)?,
            _ => return Err(fail(line, format!("unknown value kind {kind}"))),
        })
    }

    /// C++ `Runtime::newStruct`: the struct export at `Package.<path>`, with its defaults.
    fn new_struct(&mut self, path: &str) -> Result<Value, CensusError> {
        let Some((package, rest)) = path.split_once('.') else {
            return Err(format!("struct path must be Package.Struct: {path}").into());
        };
        let full = {
            let (code, store) = self.vm.defs.code_mut();
            let id = code.package(store, package).map_err(|e| chain(&e))?;
            let index = code.find_export(store, id, rest);
            if index == 0 {
                return Err(format!("struct not found: {path}").into());
            }
            let pkg = store.get(id);
            format!(
                "{}.{}",
                pkg.name,
                pkg.path_of(index).map_err(|e| e.to_string())?
            )
        };
        // The C++ `structAt` loads the struct; here a struct is defined when a class or function that uses it loads.
        let id = self.vm.space.classes.struct_by_path(&full).ok_or_else(|| {
            NotYet::not_ported(
                format!("the census value struct:{path}: the struct {full} is not loaded"),
                "decision 2026-11-15-23 (a struct loads with a class or function that uses it)",
            )
        })?;
        Ok(self
            .vm
            .space
            .classes
            .struct_defaults(id)
            .map_err(HeapError::from)?)
    }

    /// `$label.Property`, then `.Field` and `[index]` steps (C++ `property`); the element must exist.
    fn target(&mut self, line: &Line, target: &str) -> Result<Target, CensusError> {
        let Some(dot) = target.find('.') else {
            return Err(fail(
                line,
                format!("expected $label.Property, got {target}"),
            ));
        };
        let object = self.object(line, &target[..dot])?;
        let bytes = target.as_bytes();
        let mut found: Option<Target> = None;
        // The first entry and dimension of the last struct field stepped into, for an index into its static array.
        let mut field_dim: Option<(u32, i32)> = None;
        let mut at = dot + 1;
        while at < target.len() {
            let end = target[at..]
                .find(['.', '['])
                .map_or(target.len(), |p| at + p);
            if bytes[at] == b'[' {
                let close = target[at..].find(']').map(|p| at + p);
                let (Some(t), Some(close)) = (found.as_mut(), close) else {
                    return Err(fail(line, format!("bad index in {target}")));
                };
                let index = stoul(&target[at + 1..close])?;
                let no_element = || fail(line, format!("no element in {target}"));
                let index = u32::try_from(index).map_err(|_| no_element())?;
                if t.path.is_empty() && field_dim.is_none() {
                    // A static-array property: element `index` of it.
                    let dim = self
                        .vm
                        .space
                        .classes
                        .decl(t.addr.prop)
                        .map_or(1, |d| d.array_dim);
                    if dim > 1 && t.addr.index == 0 {
                        if i64::from(index) >= i64::from(dim) {
                            return Err(no_element());
                        }
                        t.addr.index = index;
                        at = close + 1;
                        if at < target.len() && bytes[at] == b'.' {
                            at += 1;
                        }
                        continue;
                    }
                }
                if let Some((first, dim)) = field_dim.take()
                    && dim > 1
                {
                    // A static-array struct field: its elements are consecutive entries.
                    if i64::from(index) >= i64::from(dim) {
                        return Err(no_element());
                    }
                    t.path.pop();
                    t.path.push(Step::Field(
                        first.checked_add(index).ok_or_else(no_element)?,
                    ));
                    at = close + 1;
                    if at < target.len() && bytes[at] == b'.' {
                        at += 1;
                    }
                    continue;
                }
                let current = self.read(t).map_err(|e| chain(&e))?;
                let (Value::Array(items), PropKind::Array(inner)) = (&current, &t.kind) else {
                    return Err(no_element());
                };
                if index as usize >= items.len() {
                    return Err(no_element());
                }
                t.path.push(Step::Element(index));
                t.kind = (**inner).clone();
                at = close + 1;
                if at < target.len() && bytes[at] == b'.' {
                    at += 1;
                }
                continue;
            }
            let name = &target[at..end];
            let missing = || fail(line, format!("no property {name} in {target}"));
            field_dim = None;
            match found.as_mut() {
                None => {
                    let class = self.vm.space.heap.get(object).map_err(|e| chain(&e))?.class;
                    let prop = self.vm.space.prop(class, name).ok_or_else(missing)?;
                    let kind = self
                        .vm
                        .space
                        .classes
                        .decl(prop)
                        .map(|d| d.kind.clone())
                        .ok_or_else(missing)?;
                    found = Some(Target {
                        object,
                        addr: PropAddr { prop, index: 0 },
                        path: Vec::new(),
                        kind,
                    });
                }
                Some(t) => {
                    let current = self.read(t).map_err(|e| chain(&e))?;
                    let (Value::Struct(s), Some(id)) = (&current, self.vm.space.names.find(name))
                    else {
                        return Err(missing());
                    };
                    let layout = self
                        .vm
                        .space
                        .classes
                        .struct_layout(s.ty)
                        .ok_or_else(missing)?;
                    let (entry, decl) = layout.field(id).ok_or_else(missing)?;
                    field_dim = Some((entry, decl.array_dim));
                    t.kind = decl.kind.clone();
                    t.path.push(Step::Field(entry));
                }
            }
            at = if end < target.len() && bytes[end] == b'.' {
                end + 1
            } else {
                end
            };
        }
        found.ok_or_else(|| fail(line, format!("no property in {target}")))
    }

    /// The value at `t` now.
    fn read(&self, t: &Target) -> Result<Value, HeapError> {
        let mut current = self.vm.space.read(t.object, t.addr)?.clone();
        for step in &t.path {
            current = match (step, &current) {
                (Step::Element(i), Value::Array(items)) => items[*i as usize].clone(),
                (Step::Field(e), Value::Struct(s)) => s.fields[*e as usize].clone(),
                _ => {
                    return Err(HeapError::BadPath {
                        at: 0,
                        reason: "the path no longer matches the value",
                    });
                }
            };
        }
        Ok(current)
    }

    fn write(&mut self, t: &Target, value: Value) -> Result<(), CensusError> {
        Ok(self
            .vm
            .space
            .write_in(t.object, t.addr, &t.path, fit(&t.kind, value))?)
    }

    /// C++ `instantiateExport(package, index, 4)`: a new object named after the export, from the export's data.
    /// The package and the export are looked up as the C++ does; making the object is `NotYet`. The definition store
    /// holds the export's object under its path, so a copy of it would leave two live objects where the C++ makes
    /// one, and the engine iterators would visit both (decision 2026-11-15-23).
    fn export(&mut self, line: &Line, package: &str, path: &str) -> Result<ObjRef, CensusError> {
        let (code, store) = self.vm.defs.code_mut();
        let id = code.package(store, package).map_err(|e| chain(&e))?;
        if code.find_export(store, id, path) <= 0 {
            return Err(fail(line, format!("export not found: {package} {path}")));
        }
        Err(NotYet::not_ported(
            format!("the census line export {package} {path}: one object for an installed export"),
            "decision 2026-11-15-23 (census export)",
        )
        .into())
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

/// Runs one entry and files what the call census and the log recorded (C++ `execute`). An error is a counted
/// function the VM cannot find again by its path (C++ `countedFunctions.at` throws).
fn execute(
    vm: &mut Vm,
    function: FuncId,
    this: Option<ObjRef>,
    args: Vec<Value>,
) -> Result<CensusEntry, CensusError> {
    vm.log.clear();
    vm.census = Some(CallCensus::default());
    let decl = vm.function(function).cloned();
    let path = decl
        .as_ref()
        .map(|d| d.path.as_str().to_string())
        .unwrap_or_default();
    let params = decl
        .as_ref()
        .map(|d| {
            d.params
                .iter()
                .map(|p| vm.space.names.text(p.name).into_owned())
                .collect()
        })
        .unwrap_or_default();
    let mut entry = CensusEntry {
        function: path.clone(),
        this: String::new(),
        args: Vec::new(),
        params,
        stop_reason: None,
        error: None,
        result: String::new(),
        steps: 0,
        scripts: BTreeMap::new(),
        natives: BTreeMap::new(),
        stubs: BTreeMap::new(),
        none_contexts: BTreeMap::new(),
        log_categories: BTreeMap::new(),
        log_head: Vec::new(),
        reach: None,
    };
    match vm.call(function, this, args) {
        Ok(r) => entry.result = describe_brief(vm, &r.ret),
        Err(e) => {
            let mut message = what(&e);
            // The C++ `Runtime::call` text for an argument list longer than the parameters.
            if let VmError::Script { message: m, .. } = &e
                && m.starts_with("too many arguments (")
            {
                message = format!("too many arguments for {path}");
            }
            entry.stop_reason = Some(
                if message.contains("step limit") {
                    "step limit"
                } else if message.contains("call depth") {
                    "call depth limit"
                } else {
                    "exception"
                }
                .to_string(),
            );
            entry.error = Some(message);
        }
    }
    entry.steps = vm.steps();
    let census = vm.census.take().unwrap_or_default();
    entry.scripts = census.script_calls.into_iter().collect();
    entry.natives = census.native_calls.into_iter().collect();
    entry.stubs = census.stub_calls.into_iter().collect();
    entry.none_contexts = census.none_contexts.into_iter().collect();
    // Log lines other than the stub lines the counters already cover, grouped with digits dropped.
    let mut stub_lines = BTreeSet::new();
    for stub in entry.stubs.keys() {
        let f = counted(vm, stub)?;
        let key = vm
            .function(f)
            .map_or_else(|| stub.clone(), |d| d.display_key().to_string());
        stub_lines.insert(format!("UNIMPLEMENTED {key}"));
    }
    for line in &vm.log {
        if stub_lines.contains(line) {
            continue;
        }
        *entry.log_categories.entry(log_category(line)).or_insert(0) += 1;
        if entry.log_head.len() < 12 {
            entry.log_head.push(head(line, 200));
        }
    }
    Ok(entry)
}

/// The function the call census counted under `path` (C++ `countedFunctions.at(path)`), found again by its path.
fn counted(vm: &mut Vm, path: &str) -> Result<FuncId, CensusError> {
    vm.find_function(path).map_err(|e| {
        CensusError::Entry(format!(
            "the call census counted {path}, which the VM does not find again: {}",
            what(&e)
        ))
    })
}

/// Runs the entry file `text` on `vm` and returns the report (C++ `nativeCensus`). The VM should use
/// `StubPolicy::LogAndZero` and the Core natives, as the command builds it. An error is a malformed file, an object,
/// class or function that cannot be found, or a value that cannot be made ([`CensusError`]: `entry file line <n>: ...`
/// where the C++ says so, or `NotYet` for what the port does not do yet); a script that fails while it runs is a
/// stopped entry, not an error. The census is off again on success.
pub fn native_census(
    vm: &mut Vm,
    text: &str,
    options: &CensusOptions,
) -> Result<CensusReport, CensusError> {
    let mut lines = Vec::new();
    // `std::getline`: lines end at `\n`; a `\r` before it is white space to the tokenizer.
    for (i, raw) in text.split('\n').enumerate() {
        let kept = raw.split('#').next().unwrap_or("");
        let words: Vec<String> = kept
            .split(is_space)
            .filter(|w| !w.is_empty())
            .map(String::from)
            .collect();
        if !words.is_empty() {
            lines.push(Line {
                words,
                number: i + 1,
            });
        }
    }
    vm.limits.step_limit = options.step_limit;
    let mut entries: Vec<CensusEntry> = Vec::new();
    let mut functions: Vec<FuncId> = Vec::new();
    {
        let mut c = Census {
            vm,
            labels: BTreeMap::new(),
            named: BTreeSet::new(),
        };
        for line in &lines {
            let w = &line.words;
            match (w[0].as_str(), w.len()) {
                ("new", 3) => {
                    let class = c.vm.find_class(&w[2])?;
                    c.intern_package_names(class);
                    let o = instantiate_named(c.vm, class, &w[1])?;
                    c.labels.insert(w[1].clone(), Value::Object(Some(o)));
                }
                ("export", 4) => {
                    let o = c.export(line, &w[2], &w[3])?;
                    c.labels.insert(w[1].clone(), Value::Object(Some(o)));
                }
                ("set", 3) => {
                    // C++20 sequences the right operand of `=` first: the value, then the place.
                    let v = c.value(line, &w[2])?;
                    let t = c.target(line, &w[1])?;
                    c.write(&t, v)?;
                }
                ("add", 3) => {
                    let t = c.target(line, &w[1])?;
                    let current = c.read(&t).map_err(|e| chain(&e))?;
                    let v = c.value(line, &w[2])?;
                    let PropKind::Array(inner) = &t.kind else {
                        return Err(fail(line, format!("add needs an array property: {}", w[1])));
                    };
                    let mut items = current
                        .elements()
                        .map(<[Value]>::to_vec)
                        .unwrap_or_default();
                    items.push(fit(inner, v));
                    c.write(&t, Value::array(items))?;
                }
                ("run", n) if n >= 3 => {
                    let function = c.vm.find_function(&w[1])?;
                    let this = if w[2] == "-" {
                        None
                    } else {
                        Some(c.object(line, &w[2])?)
                    };
                    let mut args = Vec::new();
                    for token in &w[3..] {
                        args.push(c.value(line, token)?);
                    }
                    let mut entry = execute(c.vm, function, this, args)?;
                    entry.this = w[2].clone();
                    entry.args = w[3..].to_vec();
                    entries.push(entry);
                    functions.push(function);
                }
                ("runclass", 2) => {
                    // Every script function the class itself declares, once each on one object built from its
                    // defaults, with no arguments. Natives and inherited functions are not entries.
                    let class = c.vm.find_class(&w[1])?;
                    let this = crate::interp::instantiate_from_defaults(c.vm, class)?;
                    let declared = declared_script_functions(c.vm, class);
                    let shown = format!("new {}", class_path(c.vm, class));
                    for function in declared.into_values() {
                        let mut entry = execute(c.vm, function, Some(this), Vec::new())?;
                        entry.this = shown.clone();
                        entries.push(entry);
                        functions.push(function);
                    }
                }
                _ => return Err(fail(line, format!("cannot parse: {}", w[0]))),
            }
        }
    }
    vm.census = None;
    report(vm, entries, &functions, options)
}

/// The script functions class `class` itself declares (not its states'), by name (C++ `cls->functions` filtered,
/// keyed by `Function::name`).
fn declared_script_functions(vm: &mut Vm, class: ClassId) -> BTreeMap<String, FuncId> {
    let mut out = BTreeMap::new();
    let Some(key) = vm.defs.code().key_of(class) else {
        return out;
    };
    let info = vm.defs.code().class(key);
    let (package, exports): (PackageId, Vec<i32>) =
        (info.package, info.functions.values().copied().collect());
    for export in exports {
        if let Some(f) = function_of_export(vm, package, export)
            && let Some(d) = vm.function(f)
            && !d.is_native()
        {
            out.insert(d.path.name().to_string(), f);
        }
    }
    out
}

/// The VM's function for export `export` of `package` (C++ `functionAt`); `None` where it is not a function or does
/// not load.
fn function_of_export(vm: &mut Vm, package: PackageId, export: i32) -> Option<FuncId> {
    crate::sweep::function_of_export(vm, package, export)
}

// ------------------------------------------------------------------------------------------ the static closure

/// A receiver's inferred class. `iface` marks an interface-typed receiver (implementers are not known statically).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Type {
    class: Option<ClassId>,
    iface: bool,
}

impl Type {
    fn of(class: Option<ClassId>, iface: bool) -> Type {
        Type { class, iface }
    }
}

/// Most functions of one name accepted as candidates when the receiver class is unknown.
const MAX_NAME_ONLY: usize = 8;

fn is_typed_slot(o: u8) -> bool {
    (op::OP_4C..=op::OP_50).contains(&o)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    Primary,
    Upper,
}

/// Call sites in one caller's body, by how they reach the target.
#[derive(Debug, Clone, Copy, Default)]
struct Sites {
    primary: u64,
    upper: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct Edges {
    primary: bool,
    upper: bool,
}

/// Lower-cased function name -> the functions of that name one class and its states declare.
type Declared = BTreeMap<String, Vec<FuncId>>;

/// What the walk knows of a function: its export and its decoded script.
struct Source {
    package: PackageId,
    info: Result<FunctionInfo, String>,
    code: Option<Result<Rc<Code>, String>>,
}

struct Closure {
    /// Every function called from walked bytecode: target -> calling script function -> its sites.
    targets: BTreeMap<FuncId, BTreeMap<FuncId, Sites>>,
    /// Walked script function -> what it calls, by edge kind.
    callees: BTreeMap<FuncId, BTreeMap<FuncId, Edges>>,
    unresolved_virtual: BTreeMap<String, u64>,
    missing_method: BTreeMap<String, u64>,
    unknown_native_index: BTreeMap<u32, u64>,
    undecodable: BTreeSet<String>,
    delegate_sites: u64,
    pending: Vec<FuncId>,
    walked: BTreeSet<FuncId>,
    all_classes: Option<Vec<ClassId>>,
    descendants: BTreeMap<ClassId, Rc<Vec<ClassId>>>,
    /// Class -> what it and its states declare, by lower-cased name ([`Closure::declared`]).
    declared: BTreeMap<ClassId, Rc<Declared>>,
    by_name: Option<BTreeMap<String, Vec<FuncId>>>,
    slot_types: BTreeMap<(u8, i32), Type>,
    sources: FixedHashMap<FuncId, Source>,
    natives: Option<FixedHashMap<u32, FuncId>>,
    /// (package, outer export) -> its typed child declarations, or nothing where they did not type.
    children: FixedHashMap<(PackageId, i32), Option<Rc<Vec<PropertyDecl>>>>,
}

impl Closure {
    fn new() -> Self {
        Closure {
            targets: BTreeMap::new(),
            callees: BTreeMap::new(),
            unresolved_virtual: BTreeMap::new(),
            missing_method: BTreeMap::new(),
            unknown_native_index: BTreeMap::new(),
            undecodable: BTreeSet::new(),
            delegate_sites: 0,
            pending: Vec::new(),
            walked: BTreeSet::new(),
            all_classes: None,
            descendants: BTreeMap::new(),
            declared: BTreeMap::new(),
            by_name: None,
            slot_types: BTreeMap::new(),
            sources: FixedHashMap::default(),
            natives: None,
            children: FixedHashMap::default(),
        }
    }

    /// Walks `entry` and everything it reaches (each function body once, however many entries reach it).
    fn analyse(&mut self, vm: &mut Vm, entry: FuncId) -> Result<(), String> {
        self.pending.push(entry);
        while let Some(f) = self.pending.pop() {
            self.walk_function(vm, f)?;
        }
        Ok(())
    }

    /// `entry` and the functions reachable from it through the walked call edges: closure A (primary edges only) or
    /// closure B (primary and upper-bound edges).
    fn reachable_from(&self, entry: FuncId, with_upper: bool) -> BTreeSet<FuncId> {
        let mut seen = BTreeSet::from([entry]);
        let mut stack = vec![entry];
        while let Some(f) = stack.pop() {
            let Some(callees) = self.callees.get(&f) else {
                continue;
            };
            for (&callee, edge) in callees {
                if (edge.primary || (with_upper && edge.upper)) && seen.insert(callee) {
                    stack.push(callee);
                }
            }
        }
        seen
    }

    // ---- functions and classes -------------------------------------------------------------------------------

    /// The export behind `f` and its header (found by the function's path).
    fn source(&mut self, vm: &mut Vm, f: FuncId) -> Option<&mut Source> {
        if !self.sources.contains_key(&f) {
            let path = vm.function(f)?.path.as_str().to_string();
            let (package_name, _) = path.split_once('.')?;
            let (code, store) = vm.defs.code_mut();
            let package = code.package(store, package_name).ok()?;
            // `find_export` drops one leading `<package>.`; the full path keeps a class named like its package
            // (`Engine.Engine.GetEngine`) whole.
            let index = code.find_export(store, package, &path);
            if index == 0 {
                return None;
            }
            let info = read_function(store.get(package), index).map_err(|e| e.to_string());
            self.sources.insert(
                f,
                Source {
                    package,
                    info,
                    code: None,
                },
            );
        }
        self.sources.get_mut(&f)
    }

    fn package_of(&mut self, vm: &mut Vm, f: FuncId) -> Option<PackageId> {
        self.source(vm, f).map(|s| s.package)
    }

    /// The function `reference` of `package` names (C++ `functionAt`, errors as nothing).
    fn function_at(&mut self, vm: &mut Vm, package: PackageId, reference: i32) -> Option<FuncId> {
        let (p, i) = {
            let (code, store) = vm.defs.code_mut();
            code.resolve_ref(store, package, reference)?
        };
        function_of_export(vm, p, i)
    }

    /// The class `reference` of `package` names (C++ `classAt`, errors as nothing), its layout defined.
    fn class_at(&mut self, vm: &mut Vm, package: PackageId, reference: i32) -> Option<ClassId> {
        let key = {
            let (code, store) = vm.defs.code_mut();
            code.class_at(store, package, reference).ok()??
        };
        self.layout_of(vm, key)
    }

    fn layout_of(&mut self, vm: &mut Vm, key: ClassKey) -> Option<ClassId> {
        let (code, store) = vm.defs.code_mut();
        code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key)
            .ok()
    }

    /// Every class of the code packages the store holds (C++ `buildClasses`).
    fn all_classes(&mut self, vm: &mut Vm) -> Vec<ClassId> {
        if let Some(all) = &self.all_classes {
            return all.clone();
        }
        let mut all = Vec::new();
        for name in CODE_PACKAGES {
            let package = {
                let (code, store) = vm.defs.code_mut();
                match code.package(store, name) {
                    Ok(p) => p,
                    Err(_) => continue,
                }
            };
            let count = vm.defs.packages().get(package).exports.len();
            for index in 1..=i32::try_from(count).unwrap_or(i32::MAX) {
                let is_class = {
                    let (code, store) = vm.defs.code_mut();
                    code.class_name(store, package, index) == "Class"
                };
                if is_class && let Some(c) = self.class_at(vm, package, index) {
                    all.push(c);
                }
            }
        }
        self.all_classes = Some(all.clone());
        all
    }

    /// The functions class `class` declares, and those its states declare (C++ `cls->functions`, `cls->states`), by
    /// lower-cased name: for each name, the class's function first, then the states' in state order. Built once per
    /// class.
    fn declared(&mut self, vm: &mut Vm, class: ClassId) -> Rc<Declared> {
        if let Some(found) = self.declared.get(&class) {
            return found.clone();
        }
        let mut by_key: Declared = BTreeMap::new();
        if let Some(key) = vm.defs.code().key_of(class) {
            let info = vm.defs.code().class(key);
            let package = info.package;
            let mut exports: Vec<(String, i32)> = info
                .functions
                .iter()
                .map(|(k, &e)| (k.clone(), e))
                .collect();
            for state in info.states().values() {
                exports.extend(state.functions.iter().map(|(k, &e)| (k.clone(), e)));
            }
            for (name, export) in exports {
                if let Some(f) = function_of_export(vm, package, export) {
                    by_key.entry(name).or_default().push(f);
                }
            }
        }
        let made = Rc::new(by_key);
        self.declared.insert(class, made.clone());
        made
    }

    /// Every class-level or state function named `name` (any case), over every class (C++ `declaredNamed`).
    fn declared_named(&mut self, vm: &mut Vm, name: &str) -> Vec<FuncId> {
        if self.by_name.is_none() {
            let mut by_name: BTreeMap<String, Vec<FuncId>> = BTreeMap::new();
            for class in self.all_classes(vm) {
                for (key, functions) in self.declared(vm, class).iter() {
                    by_name.entry(key.clone()).or_default().extend(functions);
                }
            }
            self.by_name = Some(by_name);
        }
        self.by_name
            .as_ref()
            .and_then(|m| m.get(&name.to_ascii_lowercase()))
            .cloned()
            .unwrap_or_default()
    }

    fn descendants_of(&mut self, vm: &mut Vm, base: ClassId) -> Rc<Vec<ClassId>> {
        if let Some(found) = self.descendants.get(&base) {
            return found.clone();
        }
        let found: Rc<Vec<ClassId>> = Rc::new(
            self.all_classes(vm)
                .into_iter()
                .filter(|&c| c != base && vm.space.classes.is_child_of(c, base))
                .collect(),
        );
        self.descendants.insert(base, found.clone());
        found
    }

    fn find_method(vm: &mut Vm, class: ClassId, name: &str) -> Option<FuncId> {
        vm.find_method(class, name, None).ok().flatten()
    }

    // ---- typing ----------------------------------------------------------------------------------------------

    /// A class of a declaration, or nothing for an engine-native stand-in (the C++ `classAt` finds no export).
    fn real(vm: &Vm, class: ClassId) -> Option<ClassId> {
        (!vm.space.classes.is_native(class)).then_some(class)
    }

    fn type_of_kind(vm: &Vm, kind: &PropKind) -> Type {
        match kind {
            PropKind::Object { class } => Type::of(Self::real(vm, *class), false),
            PropKind::Class { meta } => Type::of(Self::real(vm, *meta), false),
            PropKind::Interface { class } => Type::of(Self::real(vm, *class), true),
            _ => Type::default(),
        }
    }

    /// The typed declaration `reference` of `package` names (C++ `declAt`, errors as nothing).
    fn decl_at(&mut self, vm: &mut Vm, package: PackageId, reference: i32) -> Option<PropertyDecl> {
        let (target, outer) = {
            let (code, store) = vm.defs.code_mut();
            let (p, i) = code.resolve_ref(store, package, reference)?;
            ((p, i), store.get(p).exports.get(i as usize - 1)?.outer)
        };
        let key = (target.0, outer);
        if !self.children.contains_key(&key) {
            let (code, store) = vm.defs.code_mut();
            let typed = code
                .child_declarations(
                    store,
                    &mut vm.space.names,
                    &mut vm.space.classes,
                    target.0,
                    outer,
                )
                .ok()
                .map(Rc::new);
            self.children.insert(key, typed);
        }
        self.children
            .get(&key)?
            .as_ref()?
            .iter()
            .find(|d| d.source == Some(target))
            .cloned()
    }

    /// The kind an expression reads or writes, for variables, struct members and array elements (C++ `declOf`).
    fn kind_of(&mut self, vm: &mut Vm, e: &Expr, package: PackageId) -> Option<PropKind> {
        match e.op {
            op::LOCAL_VARIABLE
            | op::LOCAL_OUT_VARIABLE
            | op::INSTANCE_VARIABLE
            | op::DEFAULT_VARIABLE
            | op::STATE_VARIABLE
            | op::OP_5E
            | op::STRUCT_MEMBER => self.decl_at(vm, package, *e.refs.first()?).map(|d| d.kind),
            op::ARRAY_ELEMENT | op::DYN_ARRAY_ELEMENT => {
                match self.kind_of(vm, e.kids.get(1)?, package)? {
                    PropKind::Array(inner) => Some(*inner),
                    other => Some(other),
                }
            }
            op::CONTEXT | op::CLASS_CONTEXT => self.kind_of(vm, e.kids.get(1)?, package),
            op::BOOL_VARIABLE | op::INTERFACE_CONTEXT => self.kind_of(vm, e.kids.first()?, package),
            _ => None,
        }
    }

    fn type_of(&mut self, vm: &mut Vm, e: &Expr, f: &Walking, recv: Type) -> Type {
        match e.op {
            op::SELF => return Type::of(f.owner, false),
            op::DYNAMIC_CAST | op::INTERFACE_CAST => {
                let class = e
                    .refs
                    .first()
                    .and_then(|&r| self.class_at(vm, f.package, r));
                return Type::of(class, e.op == op::INTERFACE_CAST);
            }
            op::OBJECT_CONST => {
                let Some(&reference) = e.refs.first() else {
                    return Type::default();
                };
                let resolved = {
                    let (code, store) = vm.defs.code_mut();
                    code.resolve_ref(store, f.package, reference)
                        .filter(|&(p, i)| {
                            let pkg = store.get(p);
                            let class = pkg.exports[i as usize - 1].class;
                            // A class export has no class reference of its own (0 stands for Class).
                            class == 0 || pkg.object(class).is_ok_and(|c| c.name == "Class")
                        })
                };
                let class = resolved.and_then(|(p, i)| self.class_at(vm, p, i));
                return Type::of(class, false);
            }
            op::CONTEXT | op::CLASS_CONTEXT => {
                let (Some(left), Some(right)) = (e.kids.first(), e.kids.get(1)) else {
                    return Type::default();
                };
                let inner = self.type_of(vm, left, f, recv);
                return self.type_of(vm, right, f, inner);
            }
            op::CONDITIONAL => {
                return match e.kids.get(1) {
                    Some(k) => self.type_of(vm, k, f, recv),
                    None => Type::default(),
                };
            }
            o if is_typed_slot(o) => {
                return e
                    .ints
                    .first()
                    .and_then(|&n| self.slot_types.get(&(o, n)).copied())
                    .unwrap_or_default();
            }
            op::FINAL_FUNCTION | op::VIRTUAL_FUNCTION | op::GLOBAL_FUNCTION => {
                let target = if e.op == op::FINAL_FUNCTION {
                    e.refs
                        .first()
                        .and_then(|&r| self.function_at(vm, f.package, r))
                } else {
                    match (recv.class, e.names.first()) {
                        (Some(c), Some(name)) => Self::find_method(vm, c, name),
                        _ => None,
                    }
                };
                return target
                    .and_then(|t| vm.function(t).and_then(|d| d.result.clone()))
                    .map_or_else(Type::default, |r| Self::type_of_kind(vm, &r.kind));
            }
            _ => {}
        }
        self.kind_of(vm, e, f.package)
            .map_or_else(Type::default, |k| Self::type_of_kind(vm, &k))
    }

    /// Typed temporaries are assigned once near the top of a function; remember what each is assigned.
    fn learn_slots(&mut self, vm: &mut Vm, e: &Expr, f: &Walking, this: Type) {
        if (e.op == op::LET || e.op == op::OP_5F)
            && e.kids.len() == 2
            && is_typed_slot(e.kids[0].op)
            && let Some(&n) = e.kids[0].ints.first()
        {
            let key = (e.kids[0].op, n);
            if !self.slot_types.contains_key(&key) {
                let ty = self.type_of(vm, &e.kids[1], f, this);
                if ty.class.is_some() {
                    self.slot_types.insert(key, ty);
                }
            }
        }
        for kid in &e.kids {
            self.learn_slots(vm, kid, f, this);
        }
    }

    // ---- walking ---------------------------------------------------------------------------------------------

    fn record(&mut self, vm: &Vm, caller: FuncId, target: FuncId, how: Reach) {
        let sites = self
            .targets
            .entry(target)
            .or_default()
            .entry(caller)
            .or_default();
        let edge = self
            .callees
            .entry(caller)
            .or_default()
            .entry(target)
            .or_default();
        match how {
            Reach::Primary => {
                sites.primary += 1;
                edge.primary = true;
            }
            Reach::Upper => {
                sites.upper += 1;
                edge.upper = true;
            }
        }
        let native = vm.function(target).is_some_and(FunctionDecl::is_native);
        if !native && !self.walked.contains(&target) {
            self.pending.push(target);
        }
    }

    /// The methods a call by name may run when the receiver's static class is `class`: the version visible at that
    /// class, plus every override declared by a subclass (also inside states). An upper bound.
    fn resolve_by_name(&mut self, vm: &mut Vm, class: ClassId, name: &str) -> Vec<FuncId> {
        let key = name.to_ascii_lowercase();
        let mut found: Vec<FuncId> = Self::find_method(vm, class, name).into_iter().collect();
        let descendants = self.descendants_of(vm, class);
        for &c in std::iter::once(&class).chain(descendants.iter()) {
            if let Some(functions) = self.declared(vm, c).get(&key) {
                for &f in functions {
                    if !found.contains(&f) {
                        found.push(f);
                    }
                }
            }
        }
        found
    }

    /// A Context expression names the return property of the function it calls; that property's outer is the
    /// function as the compiler resolved it, which gives a receiver class when none can be inferred.
    fn declared_function(
        &mut self,
        vm: &mut Vm,
        context: &Expr,
        package: PackageId,
    ) -> Option<FuncId> {
        let reference = *context.refs.first().filter(|&&r| r != 0)?;
        let (p, outer) = {
            let (code, store) = vm.defs.code_mut();
            let (p, i) = code.resolve_ref(store, package, reference)?;
            (p, store.get(p).exports.get(i as usize - 1)?.outer)
        };
        if outer <= 0 {
            return None;
        }
        function_of_export(vm, p, outer)
    }

    fn call_by_name(
        &mut self,
        vm: &mut Vm,
        caller: FuncId,
        mut recv: Type,
        name: &str,
        declared: Option<FuncId>,
    ) {
        if name == "None" {
            return; // an empty delegate
        }
        if recv.class.is_none()
            && let Some(d) = declared.and_then(|d| vm.function(d))
            && let Some(owner) = d.owner
            && d.path.name().eq_ignore_ascii_case(name)
        {
            recv = Type::of(Some(owner), false);
        }
        let Some(class) = recv.class else {
            // No class could be inferred: a name that few classes declare is taken as upper-bound candidates.
            let candidates = self.declared_named(vm, name);
            if candidates.is_empty() || candidates.len() > MAX_NAME_ONLY {
                *self.unresolved_virtual.entry(name.to_string()).or_insert(0) += 1;
                return;
            }
            for f in candidates {
                self.record(vm, caller, f, Reach::Upper);
            }
            return;
        };
        if recv.iface {
            let candidates = self.declared_named(vm, name);
            if candidates.is_empty() {
                let key = format!("{}.{name}", class_name(vm, class));
                *self.missing_method.entry(key).or_insert(0) += 1;
            }
            for f in candidates {
                self.record(vm, caller, f, Reach::Upper);
            }
            return;
        }
        let found = self.resolve_by_name(vm, class, name);
        if found.is_empty() {
            let key = format!("{}.{name}", class_name(vm, class));
            *self.missing_method.entry(key).or_insert(0) += 1;
            return;
        }
        let visible = Self::find_method(vm, class, name);
        for f in found {
            let how = if Some(f) == visible {
                Reach::Primary
            } else {
                Reach::Upper
            };
            self.record(vm, caller, f, how);
        }
    }

    fn walk(
        &mut self,
        vm: &mut Vm,
        e: &Expr,
        f: &Walking,
        recv: Type,
        declared: Option<FuncId>,
    ) -> Result<(), String> {
        let this = Type::of(f.owner, false);
        if e.is_native_call() {
            match self.native_by_index(vm, e.native) {
                Some(native) => self.record(vm, f.id, native, Reach::Primary),
                None => *self.unknown_native_index.entry(e.native).or_insert(0) += 1,
            }
            for kid in &e.kids {
                self.walk(vm, kid, f, this, None)?;
            }
            return Ok(());
        }
        match e.op {
            op::FINAL_FUNCTION => {
                match e
                    .refs
                    .first()
                    .and_then(|&r| self.function_at(vm, f.package, r))
                {
                    Some(target) => self.record(vm, f.id, target, Reach::Primary),
                    None => {
                        *self
                            .unresolved_virtual
                            .entry("<unresolved final reference>".into())
                            .or_insert(0) += 1
                    }
                }
                for kid in &e.kids {
                    self.walk(vm, kid, f, this, None)?;
                }
            }
            op::VIRTUAL_FUNCTION | op::GLOBAL_FUNCTION => {
                let name = e.names.first().ok_or_else(|| missing_operand(f, e))?;
                self.call_by_name(vm, f.id, recv, name, declared);
                for kid in &e.kids {
                    self.walk(vm, kid, f, this, None)?;
                }
            }
            op::DELEGATE_FUNCTION => {
                // The delegate's own declaration; what it is bound to at run time is not known statically.
                if let Some(target) = e
                    .refs
                    .first()
                    .and_then(|&r| self.function_at(vm, f.package, r))
                {
                    self.record(vm, f.id, target, Reach::Primary);
                }
                self.delegate_sites += 1;
                for kid in &e.kids {
                    self.walk(vm, kid, f, this, None)?;
                }
            }
            op::DELEGATE_PROPERTY | op::INSTANCE_DELEGATE => {
                // A function name stored into a delegate: treated as reachable (it is called when the delegate fires).
                let name = e.names.first().ok_or_else(|| missing_operand(f, e))?;
                self.call_by_name(vm, f.id, recv, name, None);
                self.delegate_sites += 1;
            }
            op::CONTEXT | op::CLASS_CONTEXT => {
                let (Some(left), Some(right)) = (e.kids.first(), e.kids.get(1)) else {
                    return Err(missing_operand(f, e));
                };
                self.walk(vm, left, f, recv, None)?;
                let resolved = self.declared_function(vm, e, f.package);
                let mut inner = self.type_of(vm, left, f, recv);
                if inner.class.is_none()
                    && left.op == op::INTERFACE_CONTEXT
                    && let Some(owner) = resolved.and_then(|r| vm.function(r)).and_then(|d| d.owner)
                {
                    inner = Type::of(Some(owner), true);
                }
                self.walk(vm, right, f, inner, resolved)?;
            }
            _ => {
                for kid in &e.kids {
                    self.walk(vm, kid, f, recv, None)?;
                }
            }
        }
        Ok(())
    }

    /// The numbered native `index` (C++ `nativeByIndex`): the first native with that index over the code packages
    /// the store holds, in the order the interpreter builds its own index (`interp`, `build_native_index`).
    fn native_by_index(&mut self, vm: &mut Vm, index: u32) -> Option<FuncId> {
        if self.natives.is_none() {
            let mut table = FixedHashMap::default();
            for name in CODE_PACKAGES {
                let package = {
                    let (code, store) = vm.defs.code_mut();
                    match code.package(store, name) {
                        Ok(p) => p,
                        Err(_) => continue,
                    }
                };
                let pkg = vm.defs.packages().shared(package);
                for export in 1..=i32::try_from(pkg.exports.len()).unwrap_or(i32::MAX) {
                    if !is_function_export(&pkg, export) {
                        continue;
                    }
                    let Ok(info) = read_function(&pkg, export) else {
                        continue;
                    };
                    let number = u32::from(info.native);
                    if info.flags & FUNC_NATIVE == 0 || number == 0 || table.contains_key(&number) {
                        continue;
                    }
                    if let Some(f) = function_of_export(vm, package, export) {
                        table.insert(number, f);
                    }
                }
            }
            self.natives = Some(table);
        }
        self.natives.as_ref()?.get(&index).copied()
    }

    fn walk_function(&mut self, vm: &mut Vm, f: FuncId) -> Result<(), String> {
        let Some(decl) = vm.function(f).cloned() else {
            return Ok(());
        };
        if decl.is_native() || !self.walked.insert(f) {
            return Ok(());
        }
        let path = decl.path.as_str().to_string();
        let script = {
            let Some(package) = self.package_of(vm, f) else {
                self.undecodable.insert(path);
                return Ok(());
            };
            let pkg = vm.defs.packages().shared(package);
            let Some(source) = self.sources.get_mut(&f) else {
                return Ok(());
            };
            let decoded = source.code.get_or_insert_with(|| match &source.info {
                Ok(info) => ow_script::decode::decode(&pkg, info)
                    .map(Rc::new)
                    .map_err(|e| e.to_string()),
                Err(why) => Err(why.clone()),
            });
            match decoded {
                Ok(code) => (code.clone(), package),
                Err(_) => {
                    self.undecodable.insert(path);
                    return Ok(());
                }
            }
        };
        let (code, package) = script;
        let walking = Walking {
            id: f,
            package,
            owner: decl.owner,
            path,
        };
        let this = Type::of(decl.owner, false);
        self.slot_types.clear();
        for statement in &code.statements {
            self.learn_slots(vm, statement, &walking, this);
        }
        for statement in &code.statements {
            self.walk(vm, statement, &walking, this, None)?;
        }
        Ok(())
    }
}

/// The function being walked.
struct Walking {
    id: FuncId,
    package: PackageId,
    owner: Option<ClassId>,
    path: String,
}

/// An expression without the operand its opcode always has (the C++ `.at()` throws there and the census stops).
fn missing_operand(f: &Walking, e: &Expr) -> String {
    format!(
        "{}: expression 0x{:02x} at {} has no operand to walk",
        f.path, e.op, e.mem
    )
}

// ------------------------------------------------------------------------------------------------ the report

/// What the report says of one native (C++ `nativeInfo`).
fn native_row(vm: &mut Vm, closure: &mut Closure, f: FuncId) -> NativeRow {
    let decl = vm.function(f).cloned();
    let info = closure
        .source(vm, f)
        .and_then(|s| s.info.as_ref().ok().cloned());
    let package = closure
        .package_of(vm, f)
        .map(|p| vm.defs.packages().get(p).name.clone())
        .unwrap_or_default();
    let key = decl
        .as_ref()
        .and_then(|d| d.native_key.clone())
        .unwrap_or_default();
    NativeRow {
        implemented: vm.has_native(&key),
        key,
        package,
        owner: decl
            .as_ref()
            .and_then(|d| d.owner)
            .map_or_else(|| "?".to_string(), |c| class_name(vm, c)),
        name: decl
            .as_ref()
            .map(|d| d.path.name().to_string())
            .unwrap_or_default(),
        native_index: info.as_ref().map_or(0, |i| u32::from(i.native)),
        operator: info
            .as_ref()
            .is_some_and(|i| i.flags & (FUNC_OPERATOR | FUNC_PRE_OPERATOR) != 0),
        dynamic_by_entry: BTreeMap::new(),
        static_sites_a: 0,
        static_sites_b: 0,
        static_entries_a: Vec::new(),
        static_entries_b: Vec::new(),
        top_callers: Vec::new(),
    }
}

fn report(
    vm: &mut Vm,
    mut entries: Vec<CensusEntry>,
    functions: &[FuncId],
    options: &CensusOptions,
) -> Result<CensusReport, CensusError> {
    let mut closure = Closure::new();
    let mut reach_a: Vec<BTreeSet<FuncId>> = vec![BTreeSet::new(); entries.len()];
    let mut reach_b = reach_a.clone();
    let (mut union_a, mut union_b) = (BTreeSet::new(), BTreeSet::new());
    if options.static_closure {
        for (i, &f) in functions.iter().enumerate() {
            closure.analyse(vm, f)?;
            reach_a[i] = closure.reachable_from(f, false);
            reach_b[i] = closure.reachable_from(f, true);
            union_a.extend(reach_a[i].iter().copied());
            union_b.extend(reach_b[i].iter().copied());
        }
    }
    let is_native = |vm: &Vm, f: FuncId| vm.function(f).is_some_and(FunctionDecl::is_native);
    let path_of = |vm: &Vm, f: FuncId| {
        vm.function(f)
            .map(|d| d.path.as_str().to_string())
            .unwrap_or_default()
    };

    // Every native seen, dynamic or static, and the script functions with their dynamic calls.
    let mut natives: BTreeMap<String, (FuncId, NativeRow)> = BTreeMap::new();
    let mut script_dynamic: BTreeMap<String, u64> = BTreeMap::new();
    for (i, entry) in entries.iter().enumerate() {
        for (path, &count) in entry.natives.iter().chain(&entry.stubs) {
            let f = counted(vm, path)?;
            if !natives.contains_key(path) {
                let row = native_row(vm, &mut closure, f);
                natives.insert(path.clone(), (f, row));
            }
            if let Some((_, row)) = natives.get_mut(path) {
                *row.dynamic_by_entry.entry(i).or_insert(0) += count;
            }
        }
        for (path, &count) in &entry.scripts {
            *script_dynamic.entry(path.clone()).or_insert(0) += count;
        }
    }
    let targets: Vec<FuncId> = closure.targets.keys().copied().collect();
    for f in targets {
        if is_native(vm, f) {
            let path = path_of(vm, f);
            if !natives.contains_key(&path) {
                let row = native_row(vm, &mut closure, f);
                natives.insert(path, (f, row));
            }
        }
    }
    let mut script_functions: BTreeMap<String, ScriptRow> = BTreeMap::new();
    for (path, &calls) in &script_dynamic {
        script_functions.insert(
            path.clone(),
            ScriptRow {
                dynamic_calls: calls,
                static_closure: '-',
            },
        );
    }
    for (set, mark) in [(&union_b, 'B'), (&union_a, 'A')] {
        for &f in set.iter() {
            if is_native(vm, f) {
                continue;
            }
            let path = path_of(vm, f);
            let calls = script_dynamic.get(&path).copied().unwrap_or(0);
            script_functions.insert(
                path,
                ScriptRow {
                    dynamic_calls: calls,
                    static_closure: mark,
                },
            );
        }
    }

    if options.static_closure {
        for (i, entry) in entries.iter_mut().enumerate() {
            let mut reach = EntryReach::default();
            for (set, counts) in [(&reach_a[i], &mut reach.a), (&reach_b[i], &mut reach.b)] {
                for &f in set {
                    match vm.function(f) {
                        Some(d) if d.is_native() => {
                            counts.natives += 1;
                            if d.native_key.as_deref().is_some_and(|k| vm.has_native(k)) {
                                counts.natives_implemented += 1;
                            }
                        }
                        _ => counts.scripts += 1,
                    }
                }
            }
            entry.reach = Some(reach);
        }
        for (f, row) in natives.values_mut() {
            let mut callers: Vec<(u64, String)> = Vec::new();
            if let Some(data) = closure.targets.get(f) {
                for (caller, sites) in data {
                    if union_a.contains(caller) {
                        row.static_sites_a += sites.primary;
                    }
                    if union_b.contains(caller) {
                        row.static_sites_b += sites.primary + sites.upper;
                        callers.push((sites.primary + sites.upper, path_of(vm, *caller)));
                    }
                }
            }
            for i in 0..entries.len() {
                if reach_a[i].contains(f) {
                    row.static_entries_a.push(i);
                }
                if reach_b[i].contains(f) {
                    row.static_entries_b.push(i);
                }
            }
            callers.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            row.top_callers = callers.into_iter().take(3).map(|(_, p)| p).collect();
        }
    }

    let static_summary = options.static_closure.then(|| {
        let (mut scripts, mut called_natives) = (0, 0);
        for &f in closure.targets.keys() {
            if is_native(vm, f) {
                called_natives += 1;
            } else {
                scripts += 1;
            }
        }
        StaticSummary {
            script_functions_walked: closure.walked.len(),
            called_script_functions: scripts,
            called_natives,
            delegate_sites: closure.delegate_sites,
            unresolved_virtual: closure.unresolved_virtual.clone(),
            missing_method: closure.missing_method.clone(),
            unknown_native_index: closure.unknown_native_index.clone(),
            undecodable: closure.undecodable.clone(),
            dynamic_outside_static: script_functions
                .iter()
                .filter(|(_, s)| s.static_closure == '-')
                .map(|(p, _)| p.clone())
                .collect(),
        }
    });
    Ok(CensusReport {
        entries,
        script_functions,
        natives: natives.into_iter().map(|(p, (_, row))| (p, row)).collect(),
        static_summary,
    })
}

// ---------------------------------------------------------------------------------------------------------- tests

/// An invented `Core` package for the census and sweep tests (AGENTS.md rule 9), the census classes of
/// `tests/vm_test.py`'s fixture: `Object` with the operators `+` (146) and `<=` (152) and the unbound native
/// `Mystery`; `Foo` with `Count`, `Other` (a `Foo`), `SumTo`, `Get`, `CallGet`, `CallMystery`, `NoneCtx`, `GetCount`,
/// `Link` (stores `Self` in `Other`), `CallsBad` and `Bad` (a header whose script size does not meet the script, so
/// it does not decode), the static `Twice`; `Bar` (extends `Foo`) with its own `Get`.
#[cfg(test)]
pub(crate) mod toy {
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use ow_attr::AttributeSystem;
    use ow_core::ParityMode;
    use ow_defs::DefStore;
    use ow_defs::identity::IdentityPolicy;
    use ow_object::heap::ObjectSpace;
    use ow_package::testkit::script::{Asm, FunctionIds, FunctionSpec, Param, ScriptToy};

    use crate::api::{StubPolicy, Vm};

    const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
    const VIRTUAL: u32 = 0x2 | 0x20000;
    const STATIC: u32 = SCRIPT | 0x2000;
    const OPERATOR: u32 = 0x400 | 0x1000 | 0x23000;

    fn returns(a: &mut Asm, expr: impl FnOnce(&mut Asm)) {
        a.stmt().raw(&[0x04]);
        expr(a);
        a.end();
    }

    /// A script function returning an int, its body built from the child ids.
    fn script(
        t: &mut ScriptToy,
        owner: i32,
        name: &str,
        flags: u32,
        params: &[Param],
        locals: &[(&str, &str)],
        body: impl Fn(&FunctionIds) -> Asm,
    ) -> FunctionIds {
        t.function(
            owner,
            name,
            params,
            Some(("Int", 0)),
            locals,
            &FunctionSpec::new(flags, name),
            |ids| Some(body(ids)),
        )
    }

    /// SumTo(n): i = 1; total = 0; while (i <= n) { total += i; i++ } return total, with the loop's offsets.
    fn sum_to(ids: &FunctionIds, start: u16, end: u16) -> (Asm, u16, u16) {
        let (i, total, n) = (ids.get("I"), ids.get("Total"), ids.get("N"));
        let mut a = Asm::new();
        a.stmt().raw(&[0x0F]).local(i).raw(&[0x26]);
        a.stmt().raw(&[0x0F]).local(total).raw(&[0x25]);
        let looped = a.here() as u16;
        a.stmt().raw(&[0x07]).w(end).native(152, |a| {
            a.local(i).local(n);
        });
        a.stmt().raw(&[0x0F]).local(total).native(146, |a| {
            a.local(total).local(i);
        });
        a.stmt().raw(&[0x0F]).local(i).native(146, |a| {
            a.local(i).int_const(1);
        });
        a.stmt().raw(&[0x06]).w(start);
        let ended = a.here() as u16;
        a.stmt().raw(&[0x04]).local(total);
        a.end();
        (a, looped, ended)
    }

    pub(crate) fn package() -> Vec<u8> {
        let mut t = ScriptToy::with_imports(&[
            "Class",
            "Function",
            "IntProperty",
            "BoolProperty",
            "ObjectProperty",
        ]);
        let object = t.class("Object", 0);
        for (name, friendly, number, result) in [
            ("Add_IntInt", "+", 146u16, "Int"),
            ("LessEq_IntInt", "<=", 152, "Bool"),
        ] {
            let mut s = FunctionSpec::new(OPERATOR, friendly);
            s.native = number;
            let params = [Param::new("Int", "A"), Param::new("Int", "B")];
            t.function(object, name, &params, Some((result, 0)), &[], &s, |_| None);
        }
        let mut s = FunctionSpec::new(0x400 | 0x20000, "Mystery");
        s.native = 999;
        let params = [Param::new("Int", "A")];
        let mystery = t
            .function(
                object,
                "Mystery",
                &params,
                Some(("Int", 0)),
                &[],
                &s,
                |_| None,
            )
            .function;
        let foo = t.class("Foo", object);
        let bar = t.class("Bar", foo);
        let count = t.prop("Int", foo, "Count", 0, 0);
        let other = t.prop("Object", foo, "Other", 0, foo);
        let n = [Param::new("Int", "N")];
        script(
            &mut t,
            foo,
            "SumTo",
            SCRIPT,
            &n,
            &[("Int", "I"), ("Int", "Total")],
            |ids| {
                let (_, looped, ended) = sum_to(ids, 0, 0);
                sum_to(ids, looped, ended).0
            },
        );
        for (owner, value) in [(foo, 1), (bar, 2)] {
            script(&mut t, owner, "Get", VIRTUAL, &[], &[], |_| {
                let mut a = Asm::new();
                returns(&mut a, |a| {
                    a.int_const(value);
                });
                a
            });
        }
        let get = t.p.name("Get");
        script(&mut t, foo, "CallGet", SCRIPT, &[], &[], |_| {
            let mut a = Asm::new();
            returns(&mut a, |a| {
                a.raw(&[0x1B]).i32(get).i32(0).raw(&[0x16]);
            });
            a
        });
        script(&mut t, foo, "CallMystery", SCRIPT, &[], &[], |_| {
            let mut a = Asm::new();
            returns(&mut a, |a| {
                a.final_function(mystery, |a| {
                    a.int_const(5);
                });
            });
            a
        });
        // Context on a None object (NoObject): yields a zero value; the census counts it.
        script(&mut t, foo, "NoneCtx", SCRIPT, &[], &[], |_| {
            let mut a = Asm::new();
            returns(&mut a, |a| {
                a.raw(&[0x19, 0x2A]).w(1).reference(0).raw(&[0, 0x25]);
            });
            a
        });
        script(&mut t, foo, "GetCount", SCRIPT, &[], &[], |_| {
            let mut a = Asm::new();
            returns(&mut a, |a| {
                a.instance(count);
            });
            a
        });
        script(&mut t, foo, "Link", SCRIPT, &[], &[], |_| {
            let mut a = Asm::new();
            a.stmt().raw(&[0x0F]).instance(other).self_();
            returns(&mut a, |a| {
                a.int_const(1);
            });
            a
        });
        // CallsBad comes first, so its call is the first to decode Bad (the next function export, after its own
        // return value property and Bad's).
        let bad = t.p.next_export() + 3;
        script(&mut t, foo, "CallsBad", SCRIPT, &[], &[], |_| {
            let mut a = Asm::new();
            returns(&mut a, |a| {
                a.final_function(bad, |_| {});
            });
            a
        });
        let mut broken = FunctionSpec::new(SCRIPT, "Bad");
        broken.memory_size = Some(999);
        let made = t.function(foo, "Bad", &[], Some(("Int", 0)), &[], &broken, |_| {
            let mut a = Asm::new();
            returns(&mut a, |a| {
                a.int_const(3);
            });
            Some(a)
        });
        assert_eq!(made.function, bad, "Bad's reserved index");
        script(&mut t, foo, "Twice", STATIC, &n, &[], |ids| {
            let mut a = Asm::new();
            returns(&mut a, |a| {
                a.native(146, |a| {
                    a.local(ids.get("N")).local(ids.get("N"));
                });
            });
            a
        });
        t.build()
    }

    /// A VM over the toy package as the commands build it (`LogAndZero`, the Core natives), and its folder.
    pub(crate) struct ToyVm {
        pub(crate) vm: Vm,
        pub(crate) dir: PathBuf,
    }

    impl Drop for ToyVm {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    static NEXT: AtomicU32 = AtomicU32::new(0);

    pub(crate) fn vm(mode: ParityMode) -> ToyVm {
        let dir = std::env::temp_dir().join(format!(
            "ow-vm-census-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let defs = DefStore::from_synthetic(
            &dir,
            &[("Core.upk", &package())],
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
        crate::natives_core::register(&mut vm).unwrap();
        ToyVm { vm, dir }
    }
}

#[cfg(test)]
mod tests {
    use ow_core::ParityMode;

    use super::*;

    /// The entry file of `tests/vm_test.py`'s census block.
    const ENTRIES: &str = "# comment\nnew foo Core.Foo\nnew bar Core.Bar\nrun Core.Foo.SumTo - i:3\n\
                           run Core.Foo.CallGet $foo\nrun Core.Foo.CallGet $bar\nrun Core.Foo.CallMystery $foo\n\
                           run Core.Foo.NoneCtx $foo\nrunclass Core.Bar\n";

    /// The census of `text`, its error as the command prints it (the text and its causes).
    fn census(text: &str, options: &CensusOptions) -> Result<CensusReport, String> {
        let mut t = toy::vm(ParityMode::Off);
        native_census(&mut t.vm, text, options).map_err(|e| chain(&e))
    }

    #[test]
    fn what_the_port_does_not_do_yet_is_not_yet() {
        let new = "new foo Core.Foo\n";
        for (text, what) in [
            (
                format!("{new}set $foo.Count struct:Core.Foo\n"),
                "the census value struct:Core.Foo: the struct Core.Foo is not loaded",
            ),
            (
                format!("{new}set $foo.Count f:0x1p3\n"),
                "hexadecimal float text in a census value: 0x1p3",
            ),
            (
                "export e Core Foo\n".to_string(),
                "the census line export Core Foo: one object for an installed export",
            ),
        ] {
            let mut t = toy::vm(ParityMode::Off);
            let e = native_census(&mut t.vm, &text, &CensusOptions::default()).unwrap_err();
            let n = e.not_yet().unwrap_or_else(|| panic!("{text:?}: {e}"));
            assert_eq!(n.what, what);
        }
        // An export the package does not hold is the C++ error, not NotYet.
        let mut t = toy::vm(ParityMode::Off);
        let e = native_census(&mut t.vm, "export e Core Nope\n", &CensusOptions::default())
            .unwrap_err();
        assert!(e.not_yet().is_none());
        assert_eq!(
            e.to_string(),
            "entry file line 1: export not found: Core Nope"
        );
    }

    #[test]
    fn the_census_block_of_vm_test() {
        let r = census(ENTRIES, &CensusOptions::default()).unwrap();
        let e = &r.entries;
        assert_eq!(e.len(), 6);
        assert!(e.iter().all(CensusEntry::completed), "{e:?}");
        // SumTo(3): the loop test runs 4 times, the two additions 3 times each; one script function entered once.
        assert_eq!(e[0].scripts.values().sum::<u64>(), 1);
        assert_eq!(e[0].natives.values().sum::<u64>(), 10);
        assert!(e[0].stubs.is_empty());
        assert_eq!(e[0].result, "6");
        assert_eq!(e[0].this, "-");
        assert_eq!(e[0].args, ["i:3"]);
        assert_eq!(e[0].params, ["N"]);
        // Virtual dispatch: the receiver's own Get runs.
        assert_eq!((e[1].scripts.len(), e[2].scripts.len()), (2, 2));
        let natives = &r.natives;
        let less = &natives["Core.Object.LessEq_IntInt"];
        assert_eq!(less.dynamic_by_entry.values().sum::<u64>(), 4);
        assert!(less.implemented && less.operator);
        assert_eq!(less.key, "Object.<=(int,int)");
        assert_eq!(
            (less.native_index, less.owner.as_str(), less.name.as_str()),
            (152, "Object", "LessEq_IntInt")
        );
        let add = &natives["Core.Object.Add_IntInt"];
        assert_eq!(add.dynamic_by_entry.values().sum::<u64>(), 6);
        let scripts = &r.script_functions;
        assert_eq!(scripts["Core.Foo.Get"].dynamic_calls, 1);
        assert_eq!(scripts["Core.Bar.Get"].dynamic_calls, 2); // runclass too
        // An unimplemented native is a stub, not an implemented call, and appears once in the table.
        assert_eq!(
            (e[3].stubs.values().sum::<u64>(), e[3].natives.len()),
            (1, 0)
        );
        let stub = &natives["Core.Object.Mystery"];
        assert!(!stub.implemented && !stub.operator);
        assert_eq!(stub.dynamic_by_entry, BTreeMap::from([(3, 1)]));
        assert!(
            e[3].log_categories.is_empty(),
            "the stub line is the counters'"
        );
        // A Context whose object is None is recorded against the function that evaluated it.
        let none = BTreeMap::from([("Core.Foo.NoneCtx".to_string(), 1)]);
        assert_eq!(e[4].none_contexts, none);
        assert_eq!(e[5].this, "new Core.Bar");
        // Static closure A follows the method visible at the receiver's class, B adds the subclass override.
        assert_eq!(scripts["Core.Foo.Get"].static_closure, 'A');
        assert!("AB".contains(scripts["Core.Bar.Get"].static_closure));
        let reach = e[1].reach.as_ref().unwrap();
        assert_eq!((reach.a.scripts, reach.b.scripts), (2, 3));
        assert_eq!(
            (stub.static_sites_a, stub.static_entries_a.as_slice()),
            (1, &[3][..])
        );
        assert_eq!((add.static_sites_a, less.static_sites_a), (2, 1));
        assert_eq!(add.top_callers, ["Core.Foo.SumTo"]);
        let s = r.static_summary.as_ref().unwrap();
        assert!(s.dynamic_outside_static.is_empty(), "{s:?}");
        assert!(
            s.unresolved_virtual.is_empty() && s.unknown_native_index.is_empty(),
            "{s:?}"
        );
    }

    #[test]
    fn a_step_limit_stops_an_entry_and_the_closure_can_be_left_out() {
        let limited = CensusOptions {
            step_limit: 30,
            ..CensusOptions::default()
        };
        let r = census(ENTRIES, &limited).unwrap();
        assert_eq!(r.entries[0].stop_reason.as_deref(), Some("step limit"));
        let message = Some("step limit exceeded (runaway script?)");
        assert_eq!(r.entries[0].error.as_deref(), message);
        let quick = CensusOptions {
            static_closure: false,
            ..CensusOptions::default()
        };
        let r = census(ENTRIES, &quick).unwrap();
        assert!(r.static_summary.is_none() && r.entries[0].reach.is_none());
        let add = &r.natives["Core.Object.Add_IntInt"];
        assert_eq!(add.dynamic_by_entry.values().sum::<u64>(), 6);
        assert_eq!((add.static_sites_a, add.static_entries_a.len()), (0, 0));
    }

    #[test]
    fn set_writes_properties_and_objects_persist_between_entries() {
        let text = "new foo Core.Foo\nnew bar Core.Bar\nset $foo.Count i:7\nrun Core.Foo.GetCount $foo\n\
                    set $foo.Other $bar\nset $bar.Other none\nrun Core.Foo.Link $foo\nrun Core.Foo.Twice - i:21\n\
                    run Core.Foo.SumTo $foo i:1 i:2\n";
        let r = census(text, &CensusOptions::default()).unwrap();
        let results: Vec<&str> = r.entries.iter().map(|e| e.result.as_str()).collect();
        assert_eq!(&results[..3], ["7", "1", "42"]);
        let too_many = &r.entries[3];
        let message = Some("too many arguments for Core.Foo.SumTo");
        assert_eq!(too_many.error.as_deref(), message);
        assert_eq!(
            (too_many.stop_reason.as_deref(), too_many.steps),
            (Some("exception"), 0)
        );
    }

    #[test]
    fn a_bad_entry_file_names_its_line() {
        let new = "new foo Core.Foo\n";
        for (text, message) in [
            (
                "frob x\n".to_string(),
                "entry file line 1: cannot parse: frob",
            ),
            (
                "\n# only\nnew\n".into(),
                "entry file line 3: cannot parse: new",
            ),
            (
                format!("{new}set $foo.Nope i:1\n"),
                "entry file line 2: no property Nope in $foo.Nope",
            ),
            (
                format!("{new}set $foo i:1\n"),
                "entry file line 2: expected $label.Property, got $foo",
            ),
            (
                format!("{new}set $foo.Count i:x\n"),
                "entry file line 2: bad value i:x",
            ),
            (
                format!("{new}set $foo.Count q:1\n"),
                "entry file line 2: unknown value kind q",
            ),
            (
                format!("{new}set $foo.Count 12\n"),
                "entry file line 2: unknown value 12",
            ),
            (
                format!("{new}set $foo.Count[0] i:1\n"),
                "entry file line 2: no element in $foo.Count[0]",
            ),
            (
                format!("{new}set $foo.[0] i:1\n"),
                "entry file line 2: bad index in $foo.[0]",
            ),
            (
                "run Core.Foo.Get $nobody\n".into(),
                "entry file line 1: unknown label $nobody",
            ),
            (
                "run Core.Foo.Get foo\n".into(),
                "entry file line 1: expected $label, got foo",
            ),
            (
                "run Core.Foo.Missing -\n".into(),
                "function not found: Core.Foo.Missing",
            ),
            (
                format!("{new}add $foo.Count i:1\n"),
                "entry file line 2: add needs an array property: $foo.Count",
            ),
        ] {
            let got = census(&text, &CensusOptions::default()).unwrap_err();
            assert_eq!(got, message, "{text:?}");
        }
    }

    #[test]
    fn the_static_walk_lists_what_it_cannot_decode() {
        let r = census("run Core.Foo.CallsBad -\n", &CensusOptions::default()).unwrap();
        let e = &r.entries[0];
        assert_eq!(e.stop_reason.as_deref(), Some("exception"));
        let error = e.error.as_deref().unwrap_or_default();
        assert!(error.starts_with("cannot run Core.Foo.Bad: "), "{e:?}");
        let s = r.static_summary.unwrap();
        assert_eq!(s.undecodable, BTreeSet::from(["Core.Foo.Bad".to_string()]));
        assert_eq!(
            (s.script_functions_walked, s.called_script_functions),
            (2, 1)
        );
        assert_eq!(r.script_functions["Core.Foo.Bad"].static_closure, 'A');
    }

    #[test]
    fn label_names_do_not_respell_the_package_names() {
        // `get` met first as an object name would make the name `Get` read `get` (the table keeps the first
        // spelling); the package's names are interned before the object is named.
        let mut t = toy::vm(ParityMode::Off);
        native_census(&mut t.vm, "new get Core.Foo\n", &CensusOptions::default()).unwrap();
        let id = t.vm.space.names.find("GET").unwrap();
        assert_eq!(t.vm.space.names.text(id), "Get");
    }

    #[test]
    fn the_self_test_passes() {
        let expected = SelfTest {
            registered: 1,
            unimplemented: 1,
            duplicate_rejected: true,
        };
        assert_eq!(native_self_test().unwrap(), expected);
    }

    #[test]
    fn number_texts_follow_the_c_runtime() {
        assert_eq!(stoll(" -42x"), Some(-42));
        assert_eq!(stoll("x"), None);
        assert_eq!(stoll("99999999999999999999"), None);
        assert_eq!(stod("1.5e2xyz"), Ok(Some(150.0)));
        assert_eq!(stod("1e999"), Ok(None));
        assert!(stod("0x1p3").is_err());
        assert_eq!(stoul("12]"), Ok(12));
        assert_eq!(stoul("-1"), Ok(u64::MAX));
        assert_eq!(stoul("x").unwrap_err(), "invalid stoul argument");
    }
}
