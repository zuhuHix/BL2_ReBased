//! Owner: lane W5-BEHAVIOR. Behavior provider decoding: a `BehaviorProviderDefinition` (or
//! `AIBehaviorProviderDefinition`) read into its sequences, events, behaviors, output links, variables and
//! property-to-variable links, plus the untagged variable value block that follows its tags. AI-assisted (Claude Code).
//! The port of the constructor and `decodeValues` of `src/behavior.cpp`.
//!
//! **What a provider holds** (docs/verification/BEHAVIOR_DATA_DECODE.md). Each element of `BehaviorSequences` is a
//! sequence: its name, `bEnabledOnSpawn`, `bSequenceEnabledMutex`, `CustomEnableCondition`, the events
//! (`EventData2`), the behaviors (`BehaviorData2`), the packed output links (`ConsolidatedOutputLinkData`) and the
//! variables (`VariableData`). A packed range is `ArrayIndexAndLength = index << 16 | length`; a link word is
//! `behaviorIndex | idByte << 24`, the id byte read **signed** (255 is -1, the default output; NATIVE_MISSION_DISPATCH.md
//! A1, read from the executable, UNVERIFIED in the game). Behaviors and events link their properties to variables
//! through `ConsolidatedVariableLinkData` (property name, link type, connection index, a packed range into
//! `ConsolidatedLinkedVariables`, which holds variable indices).
//!
//! **Structural oracles.** Every event and behavior link range must lie inside the link array and every link target
//! must be a behavior of the sequence, or the provider is refused ([`BehaviorError::Data`], the C++ texts). A variable
//! link range or index outside its array is not fatal: it is reported in [`ProviderData::diagnostics`] and the value
//! block is then not trusted.
//!
//! **The value block is FITTED, not proven** (AGENTS.md rule 4). The variables' values are not tagged: they follow
//! the provider's tags ([`ow_defs::DefStore::raw_tail`]) as one untagged block, a fixed number of bytes per variable
//! type ([`VALUE_SIZES`]), in sequence order then variable order. The sizes were fitted from the packages and checked by
//! exact consumption over every provider of the installed game (32,185 providers, 0 mismatches, `research/
//! behavior_census.py`, BEHAVIOR_DATA_DECODE.md "Layout"). Here the oracle is applied to every provider decoded: the
//! block must end **exactly** at the end of the export, every type must be listed, and every non-zero object word must
//! be a reference inside the package's tables. If any check fails, no value is trusted
//! ([`ProviderData::values_decoded`] is false, a diagnostic says why) and reading a variable input is an error. Only
//! the first word of a value is kept (int, float bits, bool, object reference); the inner layout of the larger types
//! is UNVERIFIED beyond the census checks. An object variable's word is a reference relative to the package that holds
//! the provider, so its path is the path in that package (an import's path is the full path of the object it names).
//!
//! **Enum values are read by name.** `VariableData.Type` and `VariableLinkType` are bytes of
//! `BehaviorProviderDefinition.EBehaviorVariableType` / `EBehaviorVariableLinkType`; the names are read from the enum
//! exports of the installed `GearboxFramework` package ([`enum_names`], the C++ `enumNames`), so no enum position is
//! assumed. A byte outside the enum reads as `?<n>`.
//!
//! **Names.** Name values (sequence, event, variable and property names) are the object space's name texts: names
//! compare case-insensitively as FNames do, and the text is the first spelling the space saw (the C++ keeps each
//! package's spelling). Behavior names and paths are read from the package tables, as the C++ prints them.

use std::fmt;

use ow_core::NotYet;
use ow_core::PackageName;
use ow_defs::store::DefError;
use ow_object::heap::{HeapError, ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::package::{Package, PackageError};
use ow_package::tables;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The record of the variable value block and the provider layout.
pub const RECORD_DECODE: &str = "docs/verification/BEHAVIOR_DATA_DECODE.md";

/// The two provider classes the kernel reads (`src/behavior.cpp` accepts exactly these).
pub const PROVIDER_CLASSES: [&str; 2] = [
    "GearboxFramework.BehaviorProviderDefinition",
    "GearboxFramework.AIBehaviorProviderDefinition",
];

/// The package and paths of the two enums the provider data is typed by.
pub const ENUM_PACKAGE: &str = "GearboxFramework";
pub const VARIABLE_TYPE_ENUM: &str = "BehaviorProviderDefinition.EBehaviorVariableType";
pub const LINK_TYPE_ENUM: &str = "BehaviorProviderDefinition.EBehaviorVariableLinkType";

/// More enum values than this in an enum export is refused (the C++ `enumNames` bound).
pub const MAX_ENUM_VALUES: i32 = 1024;

/// Where an Enum export's value count sits: after the net index (4), the `None` FName (8) and the next field (4)
/// (`src/behavior.cpp` `enumNames`, `[NetIndex][None][Next][count][FName * count]`).
const ENUM_COUNT_OFFSET: usize = 16;

/// Bytes per variable in the untagged value block, by `EBehaviorVariableType` name. FITTED (module documentation):
/// the C++ table `valueSizes()` of `src/behavior.cpp`, checked by exact consumption over the installed game's
/// providers. A type not listed here makes the whole block untrusted rather than guessed.
pub const VALUE_SIZES: [(&str, usize); 15] = [
    ("BVAR_Bool", 4),
    ("BVAR_Int", 4),
    ("BVAR_Float", 4),
    ("BVAR_Object", 4),
    ("BVAR_Vector", 12),
    ("BVAR_DirectionVector", 48),
    ("BVAR_InstanceData", 12),
    ("BVAR_Attribute", 20),
    ("BVAR_UnaryMath", 8),
    ("BVAR_BinaryMath", 12),
    ("BVAR_NamedVariable", 0),
    ("BVAR_NamedKismetVariable", 0),
    ("BVAR_AllPlayers", 0),
    ("BVAR_AttachmentLocation", 32),
    ("BVAR_Flag", 8),
];

/// The block size of a variable of type `type_name` (exact name, as the enum spells it), or `None` when the type is
/// not in [`VALUE_SIZES`].
pub fn value_size(type_name: &str) -> Option<usize> {
    VALUE_SIZES
        .iter()
        .find(|(name, _)| *name == type_name)
        .map(|&(_, size)| size)
}

// ---------------------------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------------------------

/// Why a provider could not be read or a kernel call failed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum BehaviorError {
    /// The VM, the object space or the definition store failed; a `NotYet` is [`VmError::NotYet`] inside.
    Vm(VmError),
    /// A package table read failed (an enum export, an object reference's path).
    Package(PackageError),
    /// The provider data is not what the kernel reads (the C++ `RuntimeError` texts: not a provider, a link range
    /// outside its array, an unresolved behavior).
    Data(String),
}

impl BehaviorError {
    /// The `NotYet` inside, if this error is one.
    pub fn not_yet(&self) -> Option<&NotYet> {
        match self {
            BehaviorError::Vm(VmError::NotYet(n)) => Some(n),
            _ => None,
        }
    }
}

impl fmt::Display for BehaviorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BehaviorError::Vm(e) => e.fmt(f),
            BehaviorError::Package(e) => e.fmt(f),
            BehaviorError::Data(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for BehaviorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BehaviorError::Vm(e) => e.source(),
            BehaviorError::Package(e) => e.source(),
            BehaviorError::Data(_) => None,
        }
    }
}

impl From<VmError> for BehaviorError {
    fn from(e: VmError) -> Self {
        BehaviorError::Vm(e)
    }
}

impl From<DefError> for BehaviorError {
    fn from(e: DefError) -> Self {
        BehaviorError::Vm(VmError::from(e))
    }
}

impl From<HeapError> for BehaviorError {
    fn from(e: HeapError) -> Self {
        BehaviorError::Vm(VmError::from(e))
    }
}

impl From<NotYet> for BehaviorError {
    fn from(n: NotYet) -> Self {
        BehaviorError::Vm(VmError::NotYet(n))
    }
}

impl From<PackageError> for BehaviorError {
    fn from(e: PackageError) -> Self {
        BehaviorError::Package(e)
    }
}

fn data(text: String) -> BehaviorError {
    BehaviorError::Data(text)
}

// ---------------------------------------------------------------------------------------------------------------
// The decoded provider
// ---------------------------------------------------------------------------------------------------------------

/// One property-to-variable link (`ConsolidatedVariableLinkData`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableLink {
    pub property: String,
    /// The `EBehaviorVariableLinkType` name, e.g. `BVARLINK_Input`.
    pub link_type: String,
    pub connection: i32,
    /// Indices into the sequence's variables, each checked to be in range.
    pub variables: Vec<usize>,
}

/// One variable of a sequence (`VariableData`) and its current value.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    /// The `EBehaviorVariableType` name, e.g. `BVAR_Object`.
    pub var_type: String,
    /// Object variables: the current value as a stock object path (`""` is None).
    pub object: String,
    /// The first word of the decoded value (int, float bits, bool, object reference); 0 when not decoded.
    pub word: i32,
    /// Object variables: the live object an event payload published (not a stock path).
    pub live: Option<ObjRef>,
}

/// One behavior of a sequence (`BehaviorData2`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Behavior {
    /// The behavior definition, loaded. The kernel runs on this object (module documentation of `crate::kernel`).
    pub object: ObjRef,
    /// The behavior's class as `Package.Class`, the key of its handler.
    pub class: String,
    pub name: String,
    /// Its output links: `length` links of the sequence's link array from `start`.
    pub start: usize,
    pub length: usize,
    /// The index of the owning sequence.
    pub sequence: usize,
    /// `Context.bSupportsDefaultOutputLink` (true when the class declares no `Context`, the `BehaviorBase` default).
    pub default_output: bool,
    pub variables: Vec<VariableLink>,
}

/// One event entry of a sequence (`EventData2`) with its gates (`BehaviorEventUserData`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub name: String,
    pub start: usize,
    pub length: usize,
    pub variables: Vec<VariableLink>,
    pub enabled: bool,
    pub max_trigger_count: i32,
    /// Seconds, as the C++ keeps them (the float field widened to double).
    pub re_trigger_delay: f64,
}

/// One packed output link.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Link {
    /// The linked behavior's index in the sequence (checked).
    pub behavior: usize,
    /// The signed id byte (-1 is the default output).
    pub id: i32,
    /// `ActivateDelay` in seconds.
    pub delay: f64,
}

/// One behavior sequence.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    pub name: String,
    pub enabled_on_spawn: bool,
    pub mutex: bool,
    /// `CustomEnableCondition`, loaded.
    pub condition: Option<ObjRef>,
    pub events: Vec<Event>,
    pub behaviors: Vec<Behavior>,
    pub links: Vec<Link>,
    pub variables: Vec<Variable>,
}

/// A decoded provider definition.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderData {
    /// The definition's path in the package that holds it (the C++ `Package::path`).
    pub path: String,
    /// The provider definition object.
    pub definition: ObjRef,
    pub sequences: Vec<Sequence>,
    /// Whether the value block passed every check (module documentation).
    pub values_decoded: bool,
    /// Why the value block was not trusted.
    pub diagnostics: Vec<String>,
}

// ---------------------------------------------------------------------------------------------------------------
// Reading values the C++ way
// ---------------------------------------------------------------------------------------------------------------

/// The C++ `Value::integer()`: ints, bytes and bools as they are, floats truncated (`int64_t(f)`), anything else 0.
fn integer(v: &Value) -> i64 {
    match v {
        Value::Int(i) => i64::from(*i),
        Value::Byte(b) => i64::from(*b),
        Value::Bool(b) => i64::from(*b),
        // The C++ casts the float to int64_t; trunc_i64_wrap_i32 is that cast kept to 32 bits (N5), which every
        // field read here (counts, packed words, enum bytes) fits.
        Value::Float(f) => i64::from(ow_core::num::trunc_i64_wrap_i32(f64::from(*f))),
        _ => 0,
    }
}

/// The C++ `Value::number()`: a float widened, ints, bytes and bools as doubles, anything else 0.
fn number(v: &Value) -> f64 {
    match v {
        Value::Float(f) => f64::from(*f),
        other => integer(other) as f64,
    }
}

/// The C++ `Value::truth()` for the flags read here.
fn truth(space: &ObjectSpace, v: &Value) -> bool {
    v.truth(&space.names)
}

/// The C++ `->s` of a name value (any other value: empty).
fn text(space: &ObjectSpace, v: &Value) -> String {
    match v {
        Value::Name(n) => space.names.text(*n).into_owned(),
        Value::Str(s) => s.to_string(),
        _ => String::new(),
    }
}

/// Field `name` of a struct value (any case), or `None` when `v` is not a struct or its type declares no such
/// field (the C++ `Value::field`).
pub(crate) fn field<'v>(space: &ObjectSpace, v: &'v Value, name: &str) -> Option<&'v Value> {
    let s = v.as_struct()?;
    let id = space.names.find(name)?;
    let (entry, _) = space.classes.struct_layout(s.ty)?.field(id)?;
    s.fields.get(entry as usize)
}

/// The value of property `name` of `o` (element 0), or `None` when its class declares no such property (the C++
/// `Runtime::property` returning null).
pub(crate) fn property(
    space: &ObjectSpace,
    o: ObjRef,
    name: &str,
) -> Result<Option<Value>, BehaviorError> {
    let class = space.heap.get(o)?.class;
    match space.prop(class, name) {
        Some(p) => Ok(Some(space.read(o, PropAddr::scalar(p))?.clone())),
        None => Ok(None),
    }
}

/// `ArrayIndexAndLength = index << 16 | length` of a `SubarrayData` value (0 when absent).
fn unpack(space: &ObjectSpace, packed: Option<&Value>) -> (usize, usize) {
    let raw = packed
        .and_then(|p| field(space, p, "ArrayIndexAndLength"))
        .map_or(0, integer);
    // The C++ keeps the low 32 bits (`uint32_t(field->integer())`).
    let raw = raw as u32;
    ((raw >> 16) as usize, (raw & 0xFFFF) as usize)
}

/// `names[index]`, or `?<index>` outside the list (the C++ `nameAt`).
fn name_at(names: &[String], index: i64) -> String {
    usize::try_from(index)
        .ok()
        .and_then(|i| names.get(i))
        .cloned()
        .unwrap_or_else(|| format!("?{index}"))
}

/// The stock object path of `o`: its path in the package it was read from (an export's path in its file, an
/// unresolved import's full path), or `""` for an object made at run time or a class default object (the C++
/// `refPath`, which is empty for an object with no resource package).
pub fn stock_path(vm: &Vm, o: ObjRef) -> Result<String, BehaviorError> {
    match vm.space.heap.get(o)?.origin {
        Origin::Export { package, index } | Origin::Resource { package, index } => {
            Ok(vm.defs.packages().get(package).path_of(index)?)
        }
        _ => Ok(String::new()),
    }
}

/// The class of `o` as `Package.Class` (the code index's path, as the C++ `Class::path`).
pub fn class_path_of(vm: &Vm, o: ObjRef) -> Result<String, BehaviorError> {
    let class = vm.space.heap.get(o)?.class;
    Ok(match vm.defs.code().key_of(class) {
        Some(key) => vm.defs.code().class(key).path.clone(),
        None => vm.space.classes.layout(class).path().as_str().to_string(),
    })
}

/// The name of `o` as its package spells it (the export or import entry's name), else the space's name text.
pub fn object_name(vm: &Vm, o: ObjRef) -> Result<String, BehaviorError> {
    let object = vm.space.heap.get(o)?;
    if let Origin::Export { package, index } | Origin::Resource { package, index } = object.origin
        && let Ok(entry) = vm.defs.packages().get(package).object(index)
    {
        return Ok(entry.name.clone());
    }
    Ok(vm.space.names.text(object.name).into_owned())
}

// ---------------------------------------------------------------------------------------------------------------
// Enums and the path leaf
// ---------------------------------------------------------------------------------------------------------------

/// The value names of Enum export `enum_path` of package `package` (`[NetIndex][None][Next][count][FName * count]`,
/// the C++ `enumNames`), loaded through the store. A missing enum or a count outside `0..=1024` is refused with the
/// C++ texts.
pub fn enum_names(
    vm: &mut Vm,
    package: &str,
    enum_path: &str,
) -> Result<Vec<String>, BehaviorError> {
    let name = PackageName::parse(package).map_err(|e| data(format!("{package}: {e}")))?;
    let (_, store) = vm.defs.code_mut();
    let id = store
        .load(&name)
        .map_err(|e| BehaviorError::from(DefError::from(e)))?;
    let pkg = store.get(id);
    let index = match pkg.find_export(enum_path)? {
        Some(i) if i > 0 => i,
        _ => return Err(data(format!("enum not found: {package}.{enum_path}"))),
    };
    read_enum(pkg, index, enum_path)
}

fn read_enum(pkg: &Package, index: i32, enum_path: &str) -> Result<Vec<String>, BehaviorError> {
    let entry = pkg.object(index)?;
    let start = usize::try_from(entry.offset)
        .map_err(|_| data(format!("invalid enum export {enum_path}")))?;
    let size = usize::try_from(entry.size)
        .map_err(|_| data(format!("invalid enum export {enum_path}")))?;
    let end = start
        .checked_add(size)
        .ok_or_else(|| data(format!("invalid enum export {enum_path}")))?;
    let mut r = pkg.reader().with_limit(end);
    let at = start
        .checked_add(ENUM_COUNT_OFFSET)
        .ok_or_else(|| data(format!("invalid enum export {enum_path}")))?;
    r.set_pos(at);
    let count = r.i32().map_err(PackageError::from)?;
    if !(0..=MAX_ENUM_VALUES).contains(&count) {
        return Err(data(format!("invalid enum count in {enum_path}")));
    }
    let mut names = Vec::new();
    for _ in 0..count {
        names.push(tables::read_name(&mut r, &pkg.names).map_err(PackageError::from)?);
    }
    Ok(names)
}

/// The last component of a `ProviderDefinitionPathName`'s `PathComponentNames`: the provider's own name. The field
/// is a static array of names, read here as its last element that is neither empty nor `None`; a single name is
/// itself (the C++ `providerPathLeaf`).
pub fn provider_path_leaf(space: &ObjectSpace, components: Option<&Value>) -> String {
    let Some(components) = components else {
        return String::new();
    };
    let Some(elements) = components.elements() else {
        return text(space, components);
    };
    let mut leaf = String::new();
    for element in elements {
        let t = text(space, element);
        if !t.is_empty() && t != "None" {
            leaf = t;
        }
    }
    leaf
}

// ---------------------------------------------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------------------------------------------

/// The object an object-valued field names, loaded (`None` for a null reference or a value that is not an object).
fn loaded(vm: &mut Vm, v: Option<&Value>) -> Result<Option<ObjRef>, BehaviorError> {
    match v {
        Some(Value::Object(Some(o))) => Ok(Some(vm.defs.load(&mut vm.space, *o)?)),
        _ => Ok(None),
    }
}

/// Decodes the provider definition `definition` (loaded or an unloaded handle of the definition store): its class
/// must be one of [`PROVIDER_CLASSES`]; its sequences are read, the structural oracles applied (module
/// documentation) and the value block decoded. Every behavior and enable condition is loaded.
pub fn decode(vm: &mut Vm, definition: ObjRef) -> Result<ProviderData, BehaviorError> {
    let definition = vm.defs.load(&mut vm.space, definition)?;
    let path = stock_path(vm, definition)?;
    let class = class_path_of(vm, definition)?;
    if !PROVIDER_CLASSES.contains(&class.as_str()) {
        return Err(data(format!("not a behavior provider: {path} ({class})")));
    }
    let variable_types = enum_names(vm, ENUM_PACKAGE, VARIABLE_TYPE_ENUM)?;
    let link_types = enum_names(vm, ENUM_PACKAGE, LINK_TYPE_ENUM)?;
    let mut out = ProviderData {
        path,
        definition,
        sequences: Vec::new(),
        values_decoded: false,
        diagnostics: Vec::new(),
    };
    let sequences = match property(&vm.space, definition, "BehaviorSequences")? {
        Some(Value::Array(items)) => items,
        _ => {
            decode_values(vm, &mut out);
            return Ok(out);
        }
    };
    let mut links_usable = true;
    for item in sequences.iter() {
        let sequence = decode_sequence(
            vm,
            &out,
            item,
            &variable_types,
            &link_types,
            &mut links_usable,
        )?;
        out.sequences.push(sequence);
    }
    if !links_usable {
        out.diagnostics.push(format!(
            "variable link range outside its array in {}",
            out.path
        ));
    }
    decode_values(vm, &mut out);
    if !links_usable {
        out.values_decoded = false;
    }
    Ok(out)
}

fn decode_sequence(
    vm: &mut Vm,
    out: &ProviderData,
    item: &Value,
    variable_types: &[String],
    link_types: &[String],
    links_usable: &mut bool,
) -> Result<Sequence, BehaviorError> {
    let index = out.sequences.len();
    let space = &vm.space;
    let name =
        field(space, item, "BehaviorSequenceName").map_or_else(String::new, |v| text(space, v));
    let enabled_on_spawn = field(space, item, "bEnabledOnSpawn").is_none_or(|v| truth(space, v));
    let mutex = field(space, item, "bSequenceEnabledMutex").is_some_and(|v| truth(space, v));
    let condition_ref = field(space, item, "CustomEnableCondition").cloned();
    let condition = loaded(vm, condition_ref.as_ref())?;
    let space = &vm.space;

    let mut links = Vec::new();
    for link in field(space, item, "ConsolidatedOutputLinkData")
        .and_then(Value::elements)
        .unwrap_or(&[])
    {
        let raw = field(space, link, "LinkIdAndLinkedBehavior").map_or(0, integer) as u32;
        // The id byte is signed (255 = -1, the default output). NATIVE_MISSION_DISPATCH.md A1, UNVERIFIED.
        let id = i32::from((raw >> 24) as u8 as i8);
        let delay = field(space, link, "ActivateDelay").map_or(0.0, number);
        links.push(Link {
            behavior: (raw & 0x00FF_FFFF) as usize,
            id,
            delay,
        });
    }

    let mut variables = Vec::new();
    for variable in field(space, item, "VariableData")
        .and_then(Value::elements)
        .unwrap_or(&[])
    {
        variables.push(Variable {
            name: field(space, variable, "Name").map_or_else(String::new, |v| text(space, v)),
            var_type: field(space, variable, "Type")
                .map_or_else(String::new, |v| name_at(variable_types, integer(v))),
            object: String::new(),
            word: 0,
            live: None,
        });
    }

    // Property-to-variable links: a packed range into ConsolidatedVariableLinkData, each holding a packed range into
    // ConsolidatedLinkedVariables (variable indices). Every range and index is checked.
    let linked: Vec<i64> = field(space, item, "ConsolidatedLinkedVariables")
        .and_then(Value::elements)
        .unwrap_or(&[])
        .iter()
        .map(integer)
        .collect();
    let mut all_links = Vec::new();
    for value in field(space, item, "ConsolidatedVariableLinkData")
        .and_then(Value::elements)
        .unwrap_or(&[])
    {
        let mut parsed = VariableLink {
            property: field(space, value, "PropertyName")
                .map_or_else(String::new, |v| text(space, v)),
            link_type: field(space, value, "VariableLinkType")
                .map_or_else(String::new, |v| name_at(link_types, integer(v))),
            // A byte or an int; the C++ keeps it as an int.
            connection: field(space, value, "ConnectionIndex").map_or(0, |v| integer(v) as i32),
            variables: Vec::new(),
        };
        let (first, count) = unpack(space, field(space, value, "LinkedVariables"));
        if first + count > linked.len() {
            *links_usable = false;
        } else {
            for &v in &linked[first..first + count] {
                match usize::try_from(v) {
                    Ok(v) if v < variables.len() => parsed.variables.push(v),
                    _ => *links_usable = false,
                }
            }
        }
        all_links.push(parsed);
    }
    let mut slice = |packed: Option<&Value>| -> Vec<VariableLink> {
        let (first, count) = unpack(space, packed);
        if first + count > all_links.len() {
            *links_usable = false;
            return Vec::new();
        }
        all_links[first..first + count].to_vec()
    };

    let mut events = Vec::new();
    for event in field(space, item, "EventData2")
        .and_then(Value::elements)
        .unwrap_or(&[])
    {
        let user = field(space, event, "UserData");
        let name = user
            .and_then(|u| field(space, u, "EventName"))
            .map(|v| text(space, v))
            .ok_or_else(|| data(format!("event without an EventName in {}", out.path)))?;
        let user = user.expect("EventName was found in UserData");
        // Event gates (NATIVE_MISSION_DISPATCH.md A1 step 3, UNVERIFIED); an undeclared field keeps the default.
        let (start, length) = unpack(space, field(space, event, "OutputLinks"));
        events.push(Event {
            name,
            start,
            length,
            variables: slice(field(space, event, "OutputVariables")),
            enabled: field(space, user, "bEnabled").is_none_or(|v| truth(space, v)),
            max_trigger_count: field(space, user, "MaxTriggerCount")
                .map_or(0, |v| integer(v) as i32),
            re_trigger_delay: field(space, user, "ReTriggerDelay").map_or(0.0, number),
        });
    }

    let mut pending = Vec::new();
    for behavior in field(space, item, "BehaviorData2")
        .and_then(Value::elements)
        .unwrap_or(&[])
    {
        let reference = match field(space, behavior, "Behavior") {
            Some(Value::Object(Some(o))) => *o,
            _ => return Err(data(format!("unresolved behavior in {}", out.path))),
        };
        let (start, length) = unpack(space, field(space, behavior, "OutputLinks"));
        let variables = slice(field(space, behavior, "LinkedVariables"));
        pending.push((reference, start, length, variables));
    }

    let mut behaviors = Vec::new();
    for (reference, start, length, links_of) in pending {
        let object = vm.defs.load(&mut vm.space, reference)?;
        let class = class_path_of(vm, object)?;
        let name = object_name(vm, object)?;
        let default_output = match property(&vm.space, object, "Context")? {
            Some(context) => field(&vm.space, &context, "bSupportsDefaultOutputLink")
                .is_none_or(|v| truth(&vm.space, v)),
            None => true,
        };
        behaviors.push(Behavior {
            object,
            class,
            name,
            start,
            length,
            sequence: index,
            default_output,
            variables: links_of,
        });
    }

    // Structural oracle for the packing: ranges stay inside the link array and every target exists.
    let total = links.len();
    let inside =
        |start: usize, length: usize| start.checked_add(length).is_some_and(|end| end <= total);
    if events.iter().any(|e| !inside(e.start, e.length)) {
        return Err(data(format!(
            "event link range outside link array in {}",
            out.path
        )));
    }
    if behaviors.iter().any(|b| !inside(b.start, b.length)) {
        return Err(data(format!(
            "behavior link range outside link array in {}",
            out.path
        )));
    }
    if links.iter().any(|l| l.behavior >= behaviors.len()) {
        return Err(data(format!(
            "behavior link target out of range in {}",
            out.path
        )));
    }
    Ok(Sequence {
        name,
        enabled_on_spawn,
        mutex,
        condition,
        events,
        behaviors,
        links,
        variables,
    })
}

/// The untagged value block (module documentation): one entry per variable, sizes by type, in sequence order then
/// variable order, ending exactly at the end of the export. Anything else leaves every value untrusted.
fn decode_values(vm: &Vm, out: &mut ProviderData) {
    out.values_decoded = false;
    let Some(tail) = vm.defs.raw_tail(out.definition) else {
        out.diagnostics
            .push(format!("tag stream: no tag stream end for {}", out.path));
        return;
    };
    let bytes = tail.bytes();
    let end = bytes.len();
    let mut at = 0usize;
    let mut decoded: Vec<Vec<Variable>> = Vec::new();
    for sequence in &out.sequences {
        let mut variables = sequence.variables.clone();
        for variable in &mut variables {
            let Some(size) = value_size(&variable.var_type) else {
                out.diagnostics.push(format!(
                    "unknown variable type {} in {}",
                    variable.var_type, out.path
                ));
                return;
            };
            if at + size > end {
                out.diagnostics.push(format!(
                    "variable value block shorter than its variables in {}",
                    out.path
                ));
                return;
            }
            if size >= 4 {
                variable.word =
                    i32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
            }
            if variable.var_type == "BVAR_Object" && variable.word != 0 {
                match tail.data.path_of(variable.word) {
                    Ok(path) => variable.object = path,
                    Err(_) => {
                        out.diagnostics.push(format!(
                            "object variable reference out of range in {}",
                            out.path
                        ));
                        return;
                    }
                }
            }
            at += size;
        }
        decoded.push(variables);
    }
    if at != end {
        out.diagnostics.push(format!(
            "variable value block size mismatch in {}",
            out.path
        ));
        return;
    }
    for (sequence, variables) in out.sequences.iter_mut().zip(decoded) {
        sequence.variables = variables;
    }
    out.values_decoded = true;
}

#[cfg(test)]
mod tests;
