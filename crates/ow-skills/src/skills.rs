//! Owner: lane W6-SKILLS. Skills as data and the rules on them: the `SkillDefinition` reading, the grade rule of an
//! effect (NATIVE_SKILLS.md section 4.3), the minimum-grade test, and the two tool views the Python tools gave
//! (`tools/skill_stats.py`: effect rows and info-box lines; `tools/prepare_skill_effects.py`: the effect manifest and
//! the constraint tree). AI-assisted (Claude Code).
//!
//! **Evidence.** Every rule here was read from the executable or from the installed data and is **UNVERIFIED in the
//! running game** unless a line says otherwise (NATIVE_SKILLS.md, NATIVE_PHASELOCK_TARGETING.md section 4). The info
//! box rules were fitted to a local UI trace of the game (decision 2026-09-27); they are observations, not the game's
//! code.
//!
//! **Enum values are read by name.** A byte property is named through the enum its declaration names (the installed
//! enum export), never by position; a byte whose declaration names no enum is bad data.
//!
//! **Where the Rust differs from the Python tools, on purpose** (each pinned by a test in `skills/tests.rs`):
//! - Numbers are the data's single-precision floats. The tools widen them to Python floats and round to 6 places for
//!   their JSON; the rule (and the C++ `MissionScript::effectAmount`) is single precision.
//! - The grade rule includes the `BonusUpgradeList` term (NATIVE_SKILLS.md 4.3); `skill_stats.py` leaves it out. No
//!   Python test case has a bonus row, so no pinned answer changes.
//! - An untagged `GradeToStartApplyingEffect` is the struct default as decoded (0 for the installed struct), as
//!   `prepare_skill_effects.py` and the native read it; `skill_stats.py` assumed 1. No Python test case relies on 1.
//! - A constant attribute is one whose `ValueResolverChain` is exactly one `ConstantAttributeValueResolver`; the tool
//!   took the first export named `<attribute>.ConstantAttributeValueResolver_*` whatever the chain held.

use std::borrow::Cow;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, EvalCx, InitData};
use ow_attr::eval::{
    class_path, elements, ensure_loaded, field, flag, init_data, number, object, property, required,
};
use ow_attr::stack::ModifierType;
use ow_core::NotYet;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::error::VmError;

/// The record of the skill rules.
pub const RECORD: &str = "NATIVE_SKILLS.md";
/// The record of the grade rule.
pub const RECORD_GRADE: &str = "NATIVE_SKILLS.md section 4.3";
/// The record of the constraints.
pub const RECORD_CONSTRAINTS: &str = "NATIVE_PHASELOCK_TARGETING.md section 4";

/// Outer chains and recursion through package data stop after this many steps (a cycle guard and a depth budget).
pub const DEPTH_BUDGET: usize = 64;

// ---------------------------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------------------------

/// What a skill rule can fail with (architecture rule R3: `NotYet` names what is missing).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum SkillError {
    /// The rule, the class or the layer is not there yet.
    NotYet(NotYet),
    /// The attribute system refused (its own `NotYet` arrives as [`SkillError::NotYet`]).
    Attr(AttrError),
    /// The VM refused (its own `NotYet` arrives as [`SkillError::NotYet`]).
    Vm(VmError),
    /// The data breaks an assumption the rule needs (a value of the wrong kind, an enum value the enum lacks).
    BadData { reason: Cow<'static, str> },
}

impl SkillError {
    pub(crate) fn bad(reason: impl Into<Cow<'static, str>>) -> Self {
        SkillError::BadData {
            reason: reason.into(),
        }
    }
}

impl fmt::Display for SkillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillError::NotYet(n) => write!(f, "{} {} ({})", n.what, n.missing, n.record),
            SkillError::Attr(e) => write!(f, "attribute system: {e}"),
            SkillError::Vm(e) => write!(f, "vm: {e}"),
            SkillError::BadData { reason } => write!(f, "bad skill data: {reason}"),
        }
    }
}

impl std::error::Error for SkillError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SkillError::Attr(e) => Some(e),
            SkillError::Vm(e) => Some(e),
            _ => None,
        }
    }
}

impl From<NotYet> for SkillError {
    fn from(n: NotYet) -> Self {
        SkillError::NotYet(n)
    }
}

impl From<AttrError> for SkillError {
    fn from(e: AttrError) -> Self {
        match e {
            AttrError::NotYet(n) => SkillError::NotYet(n),
            other => SkillError::Attr(other),
        }
    }
}

impl From<VmError> for SkillError {
    fn from(e: VmError) -> Self {
        match e {
            VmError::NotYet(n) => SkillError::NotYet(n),
            other => SkillError::Vm(other),
        }
    }
}

impl From<ow_object::heap::HeapError> for SkillError {
    fn from(e: ow_object::heap::HeapError) -> Self {
        SkillError::Attr(AttrError::from(e))
    }
}

/// A skill error as the VM error a native returns (`NotYet` stays `NotYet`).
impl From<SkillError> for VmError {
    fn from(e: SkillError) -> Self {
        match e {
            SkillError::NotYet(n) => VmError::NotYet(n),
            SkillError::Attr(a) => VmError::Attr(a),
            SkillError::Vm(v) => v,
            SkillError::BadData { reason } => VmError::Script {
                function: "ow-skills".to_string(),
                message: reason.into_owned(),
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Readers shared by the skill modules
// ---------------------------------------------------------------------------------------------------------------

/// A stored int (an int, or a byte widened).
pub(crate) fn int(what: &str, v: &Value) -> Result<i32, SkillError> {
    match v {
        Value::Int(i) => Ok(*i),
        Value::Byte(b) => Ok(i32::from(*b)),
        other => Err(SkillError::bad(format!(
            "{what}: expected an int, found {}",
            other.kind_name()
        ))),
    }
}

/// A string property's text.
pub(crate) fn text(what: &str, v: &Value) -> Result<String, SkillError> {
    match v {
        Value::Str(s) => Ok(s.to_string()),
        Value::None => Ok(String::new()),
        other => Err(SkillError::bad(format!(
            "{what}: expected a string, found {}",
            other.kind_name()
        ))),
    }
}

/// The value name of byte `b` in the enum `kind` declares (rule: by name, never by position).
pub(crate) fn enum_value_name(
    space: &ObjectSpace,
    what: &str,
    kind: &PropKind,
    v: &Value,
) -> Result<String, SkillError> {
    let b = match v {
        Value::Byte(b) => *b,
        other => {
            return Err(SkillError::bad(format!(
                "{what}: expected a byte, found {}",
                other.kind_name()
            )));
        }
    };
    let e = match kind {
        PropKind::Byte { enum_: Some(e) } | PropKind::ByteAttribute { enum_: Some(e) } => *e,
        _ => {
            return Err(SkillError::bad(format!(
                "{what}: the byte's declaration names no enum"
            )));
        }
    };
    let layout = space
        .classes
        .enum_layout(e)
        .ok_or_else(|| SkillError::bad(format!("{what}: an enum this space does not know")))?;
    let n = layout.values().get(usize::from(b)).ok_or_else(|| {
        SkillError::bad(format!(
            "{what}: value {b} is outside the enum {}",
            layout.path().as_str()
        ))
    })?;
    Ok(space.names.text(*n).into_owned())
}

/// The value name of the byte property `name` of `obj`.
pub(crate) fn prop_enum(
    cx: &mut EvalCx<'_>,
    obj: ObjRef,
    name: &str,
) -> Result<String, SkillError> {
    let v = required(cx, obj, name)?;
    let space = cx.world.parts().space;
    let class = space.heap.get(obj)?.class;
    let p = space.prop(class, name).ok_or_else(|| {
        SkillError::bad(format!("{} has no property {name}", class_path(space, obj)))
    })?;
    let kind = space
        .classes
        .decl(p)
        .ok_or_else(|| SkillError::bad(format!("{name}: a property without a declaration")))?
        .kind
        .clone();
    enum_value_name(space, name, &kind, &v)
}

/// The value name of the byte field `name` of the struct value `v`.
pub(crate) fn field_enum(space: &ObjectSpace, v: &Value, name: &str) -> Result<String, SkillError> {
    let s = v
        .as_struct()
        .ok_or_else(|| SkillError::bad(format!("expected a struct with field {name}")))?;
    let layout = space
        .classes
        .struct_layout(s.ty)
        .ok_or_else(|| SkillError::bad("a struct without a layout"))?;
    let decl = space
        .names
        .find(name)
        .and_then(|n| layout.field(n))
        .map(|(_, d)| d.kind.clone())
        .ok_or_else(|| {
            SkillError::bad(format!(
                "struct {} has no field {name}",
                layout.path().as_str()
            ))
        })?;
    enum_value_name(space, name, &decl, field(space, v, name)?)
}

/// Whether `obj`'s class is the class at `path` or a subclass of it (false when the space has no such class).
pub(crate) fn is_a(space: &ObjectSpace, obj: ObjRef, path: &str) -> Result<bool, SkillError> {
    let class = space.heap.get(obj)?.class;
    Ok(match space.classes.by_path(path) {
        Some(base) => class == base || space.classes.is_child_of(class, base),
        None => false,
    })
}

/// The object path `A.B.C` of `obj`, from its name and outer chain (a cycle guard and the depth budget).
pub fn object_path(space: &ObjectSpace, obj: ObjRef) -> Result<String, SkillError> {
    let mut parts = Vec::new();
    let mut cursor = Some(obj);
    while let Some(o) = cursor {
        if parts.len() >= DEPTH_BUDGET {
            return Err(SkillError::bad(
                "an outer chain longer than the depth budget (a cycle?)",
            ));
        }
        let x = space.heap.get(o)?;
        parts.push(space.names.text(x.name).into_owned());
        cursor = x.outer;
    }
    parts.reverse();
    Ok(parts.join("."))
}

/// The class path (`Package.Class`) of `obj`.
pub fn class_of(space: &ObjectSpace, obj: ObjRef) -> String {
    class_path(space, obj)
}

/// Strips `prefix` from an enum value name (`SKILL_TYPE_Kill` -> `Kill`), as the tools print them.
pub fn strip<'a>(value: &'a str, prefix: &str) -> &'a str {
    value.strip_prefix(prefix).unwrap_or(value)
}

// ---------------------------------------------------------------------------------------------------------------
// The enums (named from the package enums; NATIVE_SKILLS.md "Enums")
// ---------------------------------------------------------------------------------------------------------------

/// `ESkillType`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillType {
    Passive,
    Action,
    ActionAugment,
    Kill,
    Proficiency,
}

impl SkillType {
    /// From the package value name (`SKILL_TYPE_Passive`, ...).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match strip(name, "SKILL_TYPE_") {
            "Passive" => SkillType::Passive,
            "Action" => SkillType::Action,
            "ActionAugment" => SkillType::ActionAugment,
            "Kill" => SkillType::Kill,
            "Proficiency" => SkillType::Proficiency,
            _ => return None,
        })
    }

    /// The name the tools print (the value name without its prefix).
    pub fn label(self) -> &'static str {
        match self {
            SkillType::Passive => "Passive",
            SkillType::Action => "Action",
            SkillType::ActionAugment => "ActionAugment",
            SkillType::Kill => "Kill",
            SkillType::Proficiency => "Proficiency",
        }
    }
}

/// `ESkillState`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillState {
    Deactivated,
    Active,
    Paused,
}

impl SkillState {
    /// From the package value name (`SKILL_Deactivated`, ...).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match strip(name, "SKILL_") {
            "Deactivated" => SkillState::Deactivated,
            "Active" => SkillState::Active,
            "Paused" => SkillState::Paused,
            _ => return None,
        })
    }

    /// The name the tools print.
    pub fn label(self) -> &'static str {
        match self {
            SkillState::Deactivated => "Deactivated",
            SkillState::Active => "Active",
            SkillState::Paused => "Paused",
        }
    }

    /// The enum's byte (`ESkillState` Deactivated 0, Active 1, Paused 2; NATIVE_SKILLS.md "Enums").
    pub fn byte(self) -> u8 {
        match self {
            SkillState::Deactivated => 0,
            SkillState::Active => 1,
            SkillState::Paused => 2,
        }
    }
}

/// `EEffectDurationType`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DurationType {
    Infinite,
    Timed,
}

impl DurationType {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match strip(name, "DURATION_") {
            "Infinite" => DurationType::Infinite,
            "Timed" => DurationType::Timed,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            DurationType::Infinite => "Infinite",
            DurationType::Timed => "Timed",
        }
    }
}

/// `EEffectTarget`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectTarget {
    None,
    Self_,
    Allies,
    Enemies,
    All,
    Pets,
}

impl EffectTarget {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match strip(name, "TARGET_") {
            "None" => EffectTarget::None,
            "Self" => EffectTarget::Self_,
            "Allies" => EffectTarget::Allies,
            "Enemies" => EffectTarget::Enemies,
            "All" => EffectTarget::All,
            "Pets" => EffectTarget::Pets,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            EffectTarget::None => "None",
            EffectTarget::Self_ => "Self",
            EffectTarget::Allies => "Allies",
            EffectTarget::Enemies => "Enemies",
            EffectTarget::All => "All",
            EffectTarget::Pets => "Pets",
        }
    }
}

/// `AttributeModifier.EModifierType` from its value name (`MT_Scale`, `MT_PreAdd`, `MT_PostAdd`).
pub fn modifier_type_from_name(name: &str) -> Option<ModifierType> {
    Some(match strip(name, "MT_") {
        "Scale" => ModifierType::Scale,
        "PreAdd" => ModifierType::PreAdd,
        "PostAdd" => ModifierType::PostAdd,
        _ => return None,
    })
}

/// The name the tools print for a modifier type.
pub fn modifier_type_label(t: ModifierType) -> &'static str {
    match t {
        ModifierType::Scale => "Scale",
        ModifierType::PreAdd => "PreAdd",
        ModifierType::PostAdd => "PostAdd",
    }
}

fn parse<T>(what: &str, name: &str, f: impl Fn(&str) -> Option<T>) -> Result<T, SkillError> {
    f(name).ok_or_else(|| SkillError::bad(format!("{what}: unknown value {name}")))
}

// ---------------------------------------------------------------------------------------------------------------
// The definition
// ---------------------------------------------------------------------------------------------------------------

/// One `SkillBonusUpgrade` entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bonus {
    /// `GradeToApplyAt`.
    pub grade: i32,
    /// `Modifier`.
    pub value: f32,
}

/// One `SkillEffectData` entry, the fields the rules read.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SkillEffect {
    /// `AttributeToModify` (None: a row that only carries a number for a behavior or the info box).
    pub attribute: Option<ObjRef>,
    pub modifier_type: ModifierType,
    pub target: EffectTarget,
    /// `bIncludeSelfAsTarget`.
    pub include_self: bool,
    /// `GradeToStartApplyingEffect`.
    pub start_grade: i32,
    /// `PerGradeUpgradeInterval`.
    pub interval: i32,
    /// `BaseModifierValue`.
    pub base: InitData,
    /// `PerGradeUpgrade`.
    pub per_grade: InitData,
    /// `BonusUpgradeList`, in data order.
    pub bonus: Vec<Bonus>,
}

/// One `SkillConstraintData` entry.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    /// The evaluator objects in order: `Evaluator` (if any), then each `EvaluatorDefinitions` entry's `Evaluator`.
    pub evaluators: Vec<ObjRef>,
    /// `bApplyConstraintOnActivatation` (the package's spelling).
    pub on_activation: bool,
    /// `bApplyConstraintWhileActive`.
    pub while_active: bool,
    /// `bApplyConstraintWhilePaused`.
    pub while_paused: bool,
    /// `OnFailure`.
    pub on_failure: SkillState,
}

/// A `SkillDefinition`, the fields the rules read.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SkillDef {
    pub object: ObjRef,
    /// `SkillName`.
    pub name: String,
    pub skill_type: SkillType,
    /// `MaxGrade`.
    pub max_grade: i32,
    /// `bSubjectToGradeRules`.
    pub subject_to_grade_rules: bool,
    pub duration_type: DurationType,
    /// `InitialDuration`.
    pub initial_duration: f32,
    /// `DefaultStartingGrade`.
    pub default_starting_grade: i32,
    /// `bCanBeToggledOff`.
    pub can_be_toggled_off: bool,
    /// `ActionSkillArchetype`.
    pub action_skill_archetype: Option<ObjRef>,
    pub effects: Vec<SkillEffect>,
    pub constraints: Vec<Constraint>,
    /// `SkillEffectPresentations`.
    pub presentations: Vec<ObjRef>,
}

fn struct_flag(space: &ObjectSpace, v: &Value, name: &str) -> Result<bool, SkillError> {
    Ok(flag(name, field(space, v, name)?)?)
}

fn struct_int(space: &ObjectSpace, v: &Value, name: &str) -> Result<i32, SkillError> {
    int(name, field(space, v, name)?)
}

/// Reads one `SkillEffectData` struct value.
pub(crate) fn read_effect(space: &ObjectSpace, v: &Value) -> Result<SkillEffect, SkillError> {
    let bonus = elements("BonusUpgradeList", field(space, v, "BonusUpgradeList")?)?
        .iter()
        .map(|b| {
            Ok(Bonus {
                grade: struct_int(space, b, "GradeToApplyAt")?,
                value: number("Modifier", field(space, b, "Modifier")?)?,
            })
        })
        .collect::<Result<Vec<_>, SkillError>>()?;
    Ok(SkillEffect {
        attribute: object("AttributeToModify", field(space, v, "AttributeToModify")?)?,
        modifier_type: parse(
            "ModifierType",
            &field_enum(space, v, "ModifierType")?,
            modifier_type_from_name,
        )?,
        target: parse(
            "EffectTarget",
            &field_enum(space, v, "EffectTarget")?,
            EffectTarget::from_name,
        )?,
        include_self: struct_flag(space, v, "bIncludeSelfAsTarget")?,
        start_grade: struct_int(space, v, "GradeToStartApplyingEffect")?,
        interval: struct_int(space, v, "PerGradeUpgradeInterval")?,
        base: init_data(space, field(space, v, "BaseModifierValue")?)?,
        per_grade: init_data(space, field(space, v, "PerGradeUpgrade")?)?,
        bonus,
    })
}

fn read_constraint(cx: &mut EvalCx<'_>, v: &Value) -> Result<Constraint, SkillError> {
    let (first, definitions, on_activation, while_active, while_paused, on_failure) = {
        let space = cx.world.parts().space;
        (
            object("Evaluator", field(space, v, "Evaluator")?)?,
            elements(
                "EvaluatorDefinitions",
                field(space, v, "EvaluatorDefinitions")?,
            )?
            .iter()
            .map(|d| object("EvaluatorDefinitions", d))
            .collect::<Result<Vec<_>, AttrError>>()?,
            struct_flag(space, v, "bApplyConstraintOnActivatation")?,
            struct_flag(space, v, "bApplyConstraintWhileActive")?,
            struct_flag(space, v, "bApplyConstraintWhilePaused")?,
            parse(
                "OnFailure",
                &field_enum(space, v, "OnFailure")?,
                SkillState::from_name,
            )?,
        )
    };
    let mut evaluators: Vec<ObjRef> = first.into_iter().collect();
    for d in definitions.into_iter().flatten() {
        // A definition entry contributes its own evaluator (prepare_skill_effects.conditions); a None one adds none.
        if let Some(e) = object("Evaluator", &required(cx, d, "Evaluator")?)? {
            evaluators.push(e);
        }
    }
    Ok(Constraint {
        evaluators,
        on_activation,
        while_active,
        while_paused,
        on_failure,
    })
}

/// Reads the `SkillDefinition` `obj` (loaded through the world's store when it is an unloaded definition).
pub fn read_skill(cx: &mut EvalCx<'_>, obj: ObjRef) -> Result<SkillDef, SkillError> {
    ensure_loaded(cx, obj)?;
    let name = text("SkillName", &required(cx, obj, "SkillName")?)?;
    let skill_type = parse(
        "SkillType",
        &prop_enum(cx, obj, "SkillType")?,
        SkillType::from_name,
    )?;
    let duration_type = parse(
        "DurationType",
        &prop_enum(cx, obj, "DurationType")?,
        DurationType::from_name,
    )?;
    let max_grade = int("MaxGrade", &required(cx, obj, "MaxGrade")?)?;
    let subject_to_grade_rules = flag(
        "bSubjectToGradeRules",
        &required(cx, obj, "bSubjectToGradeRules")?,
    )?;
    let initial_duration = number("InitialDuration", &required(cx, obj, "InitialDuration")?)?;
    let default_starting_grade = int(
        "DefaultStartingGrade",
        &required(cx, obj, "DefaultStartingGrade")?,
    )?;
    let can_be_toggled_off = flag("bCanBeToggledOff", &required(cx, obj, "bCanBeToggledOff")?)?;
    let action_skill_archetype = object(
        "ActionSkillArchetype",
        &required(cx, obj, "ActionSkillArchetype")?,
    )?;
    let effects_value = required(cx, obj, "SkillEffectDefinitions")?;
    let effects = {
        let space = cx.world.parts().space;
        elements("SkillEffectDefinitions", &effects_value)?
            .iter()
            .map(|e| read_effect(space, e))
            .collect::<Result<Vec<_>, _>>()?
    };
    let constraints_value = required(cx, obj, "SkillConstraints")?;
    let mut constraints = Vec::new();
    for c in elements("SkillConstraints", &constraints_value)? {
        constraints.push(read_constraint(cx, c)?);
    }
    let presentations = elements(
        "SkillEffectPresentations",
        &required(cx, obj, "SkillEffectPresentations")?,
    )?
    .iter()
    .map(|p| object("SkillEffectPresentations", p))
    .collect::<Result<Vec<_>, AttrError>>()?
    .into_iter()
    .flatten()
    .collect();
    Ok(SkillDef {
        object: obj,
        name,
        skill_type,
        max_grade,
        subject_to_grade_rules,
        duration_type,
        initial_duration,
        default_starting_grade,
        can_be_toggled_off,
        action_skill_archetype,
        effects,
        constraints,
        presentations,
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The grade rules
// ---------------------------------------------------------------------------------------------------------------

/// `SkillDefinition.DoesSkillPassMinGradeTest(Grade)`: true unless `bSubjectToGradeRules` is set and the grade is 0 or
/// less (NATIVE_SKILLS.md 4.1). Status: UNVERIFIED in the running game.
pub fn passes_min_grade_test(subject_to_grade_rules: bool, grade: i32) -> bool {
    !(subject_to_grade_rules && grade <= 0)
}

/// `Skill.UpdateGrade(NewGrade)`: the skill's `Grade` base is `max(NewGrade, 1)` (NATIVE_SKILLS.md 4.5). Confirmed
/// in the game in one direction (skills live capture, third run, 2026-11-06): Helios put back to grade 0 still fired
/// at its grade 1 damage.
pub fn update_grade(new_grade: i32) -> i32 {
    new_grade.max(1)
}

/// The bonus term of the grade rule: `Modifier` of the entry with the largest `GradeToApplyAt` at or below `grade`; 0
/// when none is, or when that entry is 9999 or more grades below (NATIVE_SKILLS.md 4.3; the entries are not summed).
/// Status: UNVERIFIED (read from the executable; no installed Maya skill has a bonus row this lane checked).
pub fn bonus_at(bonus: &[Bonus], grade: i32) -> f32 {
    let best = bonus
        .iter()
        .filter(|b| b.grade <= grade)
        // max_by_key keeps the last of equal keys; the record names no tie rule, and the first is kept here.
        .fold(None::<&Bonus>, |best, b| match best {
            Some(x) if x.grade >= b.grade => Some(x),
            _ => Some(b),
        });
    match best {
        Some(b) => match grade.checked_sub(b.grade) {
            Some(gap) if gap < 9999 => b.value,
            _ => 0.0,
        },
        None => 0.0,
    }
}

/// `Skill.CalculateModifierValue(EffectData, Grade, Context)` with the two initialization values already evaluated
/// (`base` from `BaseModifierValue`, `per` from `PerGradeUpgrade`, both with the effect instigator as context): 0 below
/// `GradeToStartApplyingEffect`, else `base + per x ((g - start) div max(interval, 1)) + bonus`, integer division,
/// single precision (NATIVE_SKILLS.md 4.3; the C++ `MissionScript::effectAmount` has the same form without the bonus).
/// Status: UNVERIFIED in the running game; the grade path itself (a grade change reaching a running skill) was seen
/// live for Mind's Eye (third run, 2026-11-06).
pub fn modifier_value(effect: &SkillEffect, grade: i32, base: f32, per: f32) -> f32 {
    match steps(effect, grade) {
        None => 0.0,
        Some(k) => base + per * k + bonus_at(&effect.bonus, grade),
    }
}

/// The value the info box and the tool rows show at `grade`: `None` below the start grade (the line is not shown),
/// else [`modifier_value`].
pub fn shown_value(effect: &SkillEffect, grade: i32, base: f32, per: f32) -> Option<f32> {
    steps(effect, grade).map(|k| base + per * k + bonus_at(&effect.bonus, grade))
}

/// `(g - start) div max(interval, 1)` as a float, `None` below the start grade.
fn steps(effect: &SkillEffect, grade: i32) -> Option<f32> {
    if grade < effect.start_grade {
        return None;
    }
    let step = effect.interval.max(1);
    // grade >= start, so the difference is non-negative; i64 holds it for any i32 pair.
    let k = (i64::from(grade) - i64::from(effect.start_grade)) / i64::from(step);
    // int -> float: exact below 2^24, nearest above, as the native's conversion.
    Some(k as f32)
}

// ---------------------------------------------------------------------------------------------------------------
// The tool view of an initialization value (`prepare_skill_effects.init_value`)
// ---------------------------------------------------------------------------------------------------------------

/// The four terms of a `ValueFormula`.
#[derive(Debug, Clone, PartialEq)]
pub struct FormulaTerms {
    pub multiplier: InitValue,
    pub level: InitValue,
    pub power: InitValue,
    pub offset: InitValue,
}

/// An `AttributeInitializationData` as far as it reduces without a running world.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum InitValue {
    /// A number here (constants, constant attributes, a designer attribute's `BaseValue`, a formula of constants).
    Constant(f32),
    /// Another attribute's run-time value times `scale`.
    Attribute { attribute: ObjRef, scale: f32 },
    /// A definition whose only rule is an enabled `ValueFormula` with a term that needs run-time state.
    Formula {
        terms: Box<FormulaTerms>,
        base: Box<InitValue>,
        /// `BaseValueMode`'s value name.
        mode: String,
        scale: f32,
    },
    /// A definition with a condition, variance, range or rounding, or a formula this view does not reduce.
    Unresolved(ObjRef),
}

/// The offline reader of initialization values: what `skill_stats.Resolver` and `prepare_skill_effects.init_value`
/// did, on the object model. Unresolved definitions and attributes are collected, in first-seen order.
pub struct Reader<'s> {
    pub system: &'s AttributeSystem,
    pub unresolved: Vec<ObjRef>,
}

impl<'s> Reader<'s> {
    pub fn new(system: &'s AttributeSystem) -> Self {
        Reader {
            system,
            unresolved: Vec::new(),
        }
    }

    fn note(&mut self, o: ObjRef) {
        if !self.unresolved.contains(&o) {
            self.unresolved.push(o);
        }
    }

    /// The constant value of attribute `attr`, if it has one: a `DesignerAttributeDefinition`'s `BaseValue` (when it
    /// reduces), or an attribute whose `ValueResolverChain` is one `ConstantAttributeValueResolver` (its
    /// `ConstantValue`). `None` for an attribute that needs run-time state.
    pub fn constant_of(
        &mut self,
        cx: &mut EvalCx<'_>,
        attr: ObjRef,
    ) -> Result<Option<f32>, SkillError> {
        self.constant_at(cx, attr, 0)
    }

    fn constant_at(
        &mut self,
        cx: &mut EvalCx<'_>,
        attr: ObjRef,
        depth: usize,
    ) -> Result<Option<f32>, SkillError> {
        if depth >= DEPTH_BUDGET {
            return Err(SkillError::bad(
                "attribute references deeper than the depth budget (a cycle?)",
            ));
        }
        ensure_loaded(cx, attr)?;
        if is_a(
            cx.world.parts().space,
            attr,
            "WillowGame.DesignerAttributeDefinition",
        )? {
            let base = required(cx, attr, "BaseValue")?;
            let data = init_data(cx.world.parts().space, &base)?;
            return Ok(match self.classify_at(cx, &data, depth + 1)? {
                InitValue::Constant(c) => Some(c),
                _ => None,
            });
        }
        let Some(chain) = property(cx, attr, "ValueResolverChain")? else {
            return Ok(None);
        };
        let resolvers = elements("ValueResolverChain", &chain)?
            .iter()
            .map(|r| object("ValueResolverChain", r))
            .collect::<Result<Vec<_>, AttrError>>()?;
        if let [Some(r)] = resolvers.as_slice() {
            ensure_loaded(cx, *r)?;
            if is_a(
                cx.world.parts().space,
                *r,
                "GearboxFramework.ConstantAttributeValueResolver",
            )? {
                return Ok(Some(number(
                    "ConstantValue",
                    &required(cx, *r, "ConstantValue")?,
                )?));
            }
        }
        Ok(None)
    }

    /// The tool view of `data` (`prepare_skill_effects.init_value`).
    pub fn classify(
        &mut self,
        cx: &mut EvalCx<'_>,
        data: &InitData,
    ) -> Result<InitValue, SkillError> {
        self.classify_at(cx, data, 0)
    }

    fn classify_at(
        &mut self,
        cx: &mut EvalCx<'_>,
        data: &InitData,
        depth: usize,
    ) -> Result<InitValue, SkillError> {
        if depth >= DEPTH_BUDGET {
            return Err(SkillError::bad(
                "initialization data deeper than the depth budget (a cycle?)",
            ));
        }
        let scale = data.base_value_scale_constant;
        if let Some(definition) = data.initialization_definition {
            return self.definition(cx, data, definition, depth);
        }
        if let Some(attribute) = data.base_value_attribute {
            return Ok(match self.constant_at(cx, attribute, depth + 1)? {
                Some(c) => InitValue::Constant(c * scale),
                None => {
                    self.note(attribute);
                    InitValue::Attribute { attribute, scale }
                }
            });
        }
        Ok(InitValue::Constant(data.base_value_constant * scale))
    }

    fn definition(
        &mut self,
        cx: &mut EvalCx<'_>,
        data: &InitData,
        definition: ObjRef,
        depth: usize,
    ) -> Result<InitValue, SkillError> {
        ensure_loaded(cx, definition)?;
        let formula = required(cx, definition, "ValueFormula")?;
        let enabled = |cx: &mut EvalCx<'_>, name: &str| -> Result<bool, SkillError> {
            let v = required(cx, definition, name)?;
            Ok(flag(name, field(cx.world.parts().space, &v, "bEnabled")?)?)
        };
        let conditional = enabled(cx, "ConditionalInitialization")?;
        let variance = enabled(cx, "RandomVariance")?;
        // A range restriction or a rounding mode set on the definition (the tool's `props.get(...)` truth test).
        let range = required(cx, definition, "RangeRestriction")?;
        let range_set = {
            let space = cx.world.parts().space;
            struct_flag(space, &range, "bEnableMinValueRestriction")?
                || struct_flag(space, &range, "bEnableMaxValueRestriction")?
        };
        let rounding = prop_enum(cx, definition, "RoundingMode")?;
        let rounding_set = rounding != "ATTRROUNDING_Float";
        let formula_on = flag(
            "bEnabled",
            field(cx.world.parts().space, &formula, "bEnabled")?,
        )?;
        if !formula_on || conditional || variance || range_set || rounding_set {
            self.note(definition);
            return Ok(InitValue::Unresolved(definition));
        }
        let mut terms = Vec::with_capacity(4);
        for key in ["Multiplier", "Level", "Power", "Offset"] {
            let t = {
                let space = cx.world.parts().space;
                init_data(space, field(space, &formula, key)?)?
            };
            terms.push((t, self.classify_at(cx, &t, depth + 1)?));
        }
        let all_plain = terms.iter().all(|(t, v)| {
            t.base_value_attribute.is_none()
                && t.initialization_definition.is_none()
                && matches!(v, InitValue::Constant(_))
        });
        if all_plain && data.base_value_attribute.is_none() {
            // Every term a plain constant: the one evaluator gives the number (no context is read).
            let value = self.system.evaluate_init(data, None, None, cx)?;
            return Ok(InitValue::Constant(value));
        }
        if terms
            .iter()
            .any(|(_, v)| matches!(v, InitValue::Unresolved(_)))
        {
            self.note(definition);
            return Ok(InitValue::Unresolved(definition));
        }
        let base = InitData {
            initialization_definition: None,
            base_value_scale_constant: 1.0,
            ..*data
        };
        let base = self.classify_at(cx, &base, depth + 1)?;
        let mode = prop_enum(cx, definition, "BaseValueMode")?;
        let mut it = terms.into_iter().map(|(_, v)| v);
        let (Some(multiplier), Some(level), Some(power), Some(offset)) =
            (it.next(), it.next(), it.next(), it.next())
        else {
            return Err(SkillError::bad("a formula without its four terms"));
        };
        Ok(InitValue::Formula {
            terms: Box::new(FormulaTerms {
                multiplier,
                level,
                power,
                offset,
            }),
            base: Box::new(base),
            mode,
            scale: data.base_value_scale_constant,
        })
    }

    /// The number `data` gives here, if it reduces to one ([`InitValue::Constant`]).
    pub fn value(
        &mut self,
        cx: &mut EvalCx<'_>,
        data: &InitData,
    ) -> Result<Option<f32>, SkillError> {
        Ok(match self.classify(cx, data)? {
            InitValue::Constant(c) => Some(c),
            _ => None,
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The constraint tree (`prepare_skill_effects.expression`)
// ---------------------------------------------------------------------------------------------------------------

/// An evaluator object as a condition node.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Test {
    /// `AttributeExpressionEvaluator.Expression`.
    Attribute {
        attribute: Option<ObjRef>,
        /// `ComparisonOperator`'s value name without `OPERATOR_`.
        op: String,
        attribute2: Option<ObjRef>,
        /// `Operand2Usage`'s value name without `OPERAND_`.
        usage: String,
        constant: f32,
    },
    /// `CompoundExpressionEvaluator`: `Expression1 <op> Expression2`.
    Compound {
        /// `Operator`'s value name without `CEOP_`, lower case (`and`, `or`).
        op: String,
        left: Box<Test>,
        right: Box<Test>,
    },
    /// `VehiclePassengerExpressionEvaluator` (the tools' `onFoot`).
    OnFoot { object: ObjRef },
    /// `HealthStateExpressionEvaluator`.
    HealthState { object: ObjRef },
    /// Any other evaluator class.
    Other { object: ObjRef, class: String },
}

impl Test {
    /// The kind the tools print.
    pub fn kind(&self) -> &str {
        match self {
            Test::Attribute { .. } => "attribute",
            Test::Compound { op, .. } => op,
            Test::OnFoot { .. } => "onFoot",
            Test::HealthState { .. } => "healthState",
            Test::Other { .. } => "other",
        }
    }
}

/// The condition tree of evaluator `obj` (a cycle guard and the depth budget).
pub fn expression(cx: &mut EvalCx<'_>, obj: ObjRef) -> Result<Test, SkillError> {
    expression_at(cx, obj, 0)
}

fn expression_at(cx: &mut EvalCx<'_>, obj: ObjRef, depth: usize) -> Result<Test, SkillError> {
    if depth >= DEPTH_BUDGET {
        return Err(SkillError::bad(
            "an evaluator tree deeper than the depth budget (a cycle?)",
        ));
    }
    ensure_loaded(cx, obj)?;
    let space = cx.world.parts().space;
    let class = class_of(space, obj);
    let leaf = class.rsplit('.').next().unwrap_or(&class).to_string();
    match leaf.as_str() {
        "AttributeExpressionEvaluator" => {
            let e = required(cx, obj, "Expression")?;
            let space = cx.world.parts().space;
            Ok(Test::Attribute {
                attribute: object("AttributeOperand1", field(space, &e, "AttributeOperand1")?)?,
                op: strip(&field_enum(space, &e, "ComparisonOperator")?, "OPERATOR_").to_string(),
                attribute2: object("AttributeOperand2", field(space, &e, "AttributeOperand2")?)?,
                usage: strip(&field_enum(space, &e, "Operand2Usage")?, "OPERAND_").to_string(),
                constant: number("ConstantOperand2", field(space, &e, "ConstantOperand2")?)?,
            })
        }
        "CompoundExpressionEvaluator" => {
            let op = strip(&prop_enum(cx, obj, "Operator")?, "CEOP_").to_lowercase();
            let e1 = object("Expression1", &required(cx, obj, "Expression1")?)?
                .ok_or_else(|| SkillError::bad("a compound evaluator without Expression1"))?;
            let e2 = object("Expression2", &required(cx, obj, "Expression2")?)?
                .ok_or_else(|| SkillError::bad("a compound evaluator without Expression2"))?;
            Ok(Test::Compound {
                op,
                left: Box::new(expression_at(cx, e1, depth + 1)?),
                right: Box::new(expression_at(cx, e2, depth + 1)?),
            })
        }
        "VehiclePassengerExpressionEvaluator" => Ok(Test::OnFoot { object: obj }),
        "HealthStateExpressionEvaluator" => Ok(Test::HealthState { object: obj }),
        _ => Ok(Test::Other { object: obj, class }),
    }
}

/// One constraint row of the tool manifest (`prepare_skill_effects.conditions`): one per evaluator.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub test: Test,
    pub on_activation: bool,
    pub while_active: bool,
    pub while_paused: bool,
    pub on_failure: SkillState,
}

/// The constraint rows of `def`, one per evaluator, in order.
pub fn conditions(cx: &mut EvalCx<'_>, def: &SkillDef) -> Result<Vec<Condition>, SkillError> {
    let mut rows = Vec::new();
    for c in &def.constraints {
        for e in &c.evaluators {
            rows.push(Condition {
                test: expression(cx, *e)?,
                on_activation: c.on_activation,
                while_active: c.while_active,
                while_paused: c.while_paused,
                on_failure: c.on_failure,
            });
        }
    }
    Ok(rows)
}

/// One effect row of the tool manifest (`prepare_skill_effects.skill_entry`'s `effects`).
#[derive(Debug, Clone, PartialEq)]
pub struct EffectEntry {
    pub attribute: Option<ObjRef>,
    pub modifier_type: ModifierType,
    pub target: EffectTarget,
    pub include_self: bool,
    pub start_grade: i32,
    pub interval: i32,
    pub base: InitValue,
    pub per_grade: InitValue,
    pub bonus: Vec<Bonus>,
}

/// One skill of the tool manifest (`prepare_skill_effects.skill_entry`).
#[derive(Debug, Clone, PartialEq)]
pub struct SkillEntry {
    pub def: SkillDef,
    pub effects: Vec<EffectEntry>,
    pub conditions: Vec<Condition>,
}

/// The manifest entry of skill `obj`.
pub fn skill_entry(
    cx: &mut EvalCx<'_>,
    reader: &mut Reader<'_>,
    obj: ObjRef,
) -> Result<SkillEntry, SkillError> {
    let def = read_skill(cx, obj)?;
    let mut effects = Vec::with_capacity(def.effects.len());
    for e in &def.effects {
        effects.push(EffectEntry {
            attribute: e.attribute,
            modifier_type: e.modifier_type,
            target: e.target,
            include_self: e.include_self,
            start_grade: e.start_grade,
            interval: e.interval,
            base: reader.classify(cx, &e.base)?,
            per_grade: reader.classify(cx, &e.per_grade)?,
            bonus: e.bonus.clone(),
        });
    }
    let conditions = conditions(cx, &def)?;
    Ok(SkillEntry {
        def,
        effects,
        conditions,
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Effect rows and the info box (`tools/skill_stats.py`)
// ---------------------------------------------------------------------------------------------------------------

/// One row of `skill_stats.effect_rows`: the effect's attribute, modifier type, target and start grade, and its value
/// at grades `0..=grades` (`None` below the start grade or where the value does not reduce here).
#[derive(Debug, Clone, PartialEq)]
pub struct EffectRow {
    pub attribute: Option<ObjRef>,
    pub modifier_type: ModifierType,
    pub target: EffectTarget,
    pub start_grade: i32,
    pub values: Vec<Option<f32>>,
}

/// `skill_stats.effect_rows`: one row per effect of `def`, values at grades `0..=grades`.
pub fn effect_rows(
    cx: &mut EvalCx<'_>,
    reader: &mut Reader<'_>,
    def: &SkillDef,
    grades: i32,
) -> Result<Vec<EffectRow>, SkillError> {
    let mut rows = Vec::with_capacity(def.effects.len());
    for e in &def.effects {
        let base = reader.value(cx, &e.base)?;
        let per = reader.value(cx, &e.per_grade)?;
        let values = (0..=grades)
            .map(|g| match (base, per) {
                // An unreduced per-grade value counts as 0, as the tool's `per or 0.0`.
                (Some(b), p) => shown_value(e, g, b, p.unwrap_or(0.0)),
                (None, _) => None,
            })
            .collect();
        rows.push(EffectRow {
            attribute: e.attribute,
            modifier_type: e.modifier_type,
            target: e.target,
            start_grade: e.start_grade,
            values,
        });
    }
    Ok(rows)
}

/// `ESignStyle` as the info box reads it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignStyle {
    /// The value's own sign, `+` for a non-negative value.
    #[default]
    Default,
    Positive,
    Negative,
}

/// The number flags of an `AttributePresentationDefinition`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberFlags {
    /// `bDisplayAsPercentage` (the class default is true).
    pub as_percentage: bool,
    /// `RoundingMode` is `ATTRROUNDING_Float`.
    pub rounding_float: bool,
    /// `bDisplayPercentAsFloat`.
    pub percent_as_float: bool,
    pub sign: SignStyle,
    /// `bDontDisplayPlusSign`.
    pub no_plus_sign: bool,
}

impl Default for NumberFlags {
    fn default() -> Self {
        NumberFlags {
            as_percentage: true,
            rounding_float: false,
            percent_as_float: false,
            sign: SignStyle::Default,
            no_plus_sign: false,
        }
    }
}

/// `skill_stats.format_number`: the magnitude (times 100 with `%` as a percentage), rounded to an integer, or to one
/// decimal with trailing zeros dropped when the rounding mode is `ATTRROUNDING_Float` or the percent is shown as a
/// float, then the sign. Python's `round` and `.1f` round half to even on the exact value; the integer and fixed
/// printers here do the same. Fitted to the traced info box (UNVERIFIED as the game's code). `value` is the
/// double the tool widens the stored float to.
pub fn format_number(value: f64, flags: NumberFlags) -> String {
    let shown = value.abs() * if flags.as_percentage { 100.0 } else { 1.0 };
    let keep_float = flags.rounding_float || flags.percent_as_float;
    let text = if keep_float {
        let t = ow_core::fmt::fmt_f(shown, 1);
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        ow_core::fmt::fmt_f(ow_core::num::round_half_even(shown), 0)
    };
    let sign = match flags.sign {
        SignStyle::Positive => "+",
        SignStyle::Negative => "-",
        SignStyle::Default if value < 0.0 => "-",
        SignStyle::Default => "+",
    };
    let sign = if sign == "+" && flags.no_plus_sign {
        ""
    } else {
        sign
    };
    format!("{sign}{text}{}", if flags.as_percentage { "%" } else { "" })
}

/// `skill_stats.localization_key`: the object path without its package, with `:` before the first object inside an
/// outer that is not a package. `class_of(path)` answers an outer's class path (`None`: a package).
pub fn localization_key(path: &str, class_of: impl Fn(&str) -> Option<String>) -> String {
    let parts: Vec<&str> = path.split('.').collect();
    let Some(first) = parts.get(1) else {
        return String::new();
    };
    let mut key = (*first).to_string();
    let mut joined = false;
    for i in 2..parts.len() {
        let outer = parts[..i].join(".");
        let is_package = class_of(&outer).is_none_or(|c| c == "Core.Package");
        let sep = if !joined && !is_package { ':' } else { '.' };
        joined = joined || sep == ':';
        key.push(sep);
        key.push_str(parts[i]);
    }
    key
}

/// `skill_stats.read_localization`: a `<Package>.int` file (UTF-16, as the install ships them) as
/// `(section, key) -> value`, quotes around a value removed. A section header's last space-separated word is dropped
/// (`[Name Class]` gives `Name`). Missing files give an empty map (the tool's rule).
pub fn read_localization(
    path: &Path,
) -> Result<ow_core::DetMap<(String, String), String>, SkillError> {
    let mut entries = ow_core::DetMap::new();
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(entries);
    };
    let units: Vec<u16> = match bytes.as_slice() {
        [0xFF, 0xFE, rest @ ..] => rest
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect(),
        [0xFE, 0xFF, rest @ ..] => rest
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect(),
        rest => rest
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect(),
    };
    let text = String::from_utf16(&units)
        .map_err(|e| SkillError::bad(format!("{}: not UTF-16 ({e})", path.display())))?;
    let mut section: Option<String> = None;
    for line in text.lines() {
        if line.starts_with('[') {
            let inner = line.trim();
            let inner = inner.get(1..inner.len().saturating_sub(1)).unwrap_or("");
            section = Some(match inner.rsplit_once(' ') {
                Some((head, _)) => head.to_string(),
                None => inner.to_string(),
            });
        } else if let (Some(s), Some((key, value))) = (section.as_ref(), line.split_once('=')) {
            let value = value.trim();
            let value = if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
                &value[1..value.len() - 1]
            } else {
                value
            };
            entries.insert((s.clone(), key.trim().to_string()), value.to_string());
        }
    }
    Ok(entries)
}

/// The presentation fields the info box reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Presentation {
    pub object: ObjRef,
    /// `Attribute`.
    pub attribute: Option<ObjRef>,
    /// `Description`.
    pub description: String,
    pub number: NumberFlags,
    /// `bUseCustomNumberPlacement`.
    pub custom_placement: bool,
    /// `bDontDisplayNumber`.
    pub no_number: bool,
}

/// Reads the `AttributePresentationDefinition` `obj`.
pub fn read_presentation(cx: &mut EvalCx<'_>, obj: ObjRef) -> Result<Presentation, SkillError> {
    ensure_loaded(cx, obj)?;
    let b = |cx: &mut EvalCx<'_>, name: &str| -> Result<bool, SkillError> {
        Ok(flag(name, &required(cx, obj, name)?)?)
    };
    let sign = match strip(&prop_enum(cx, obj, "SignStyle")?, "SIGNSTYLE_") {
        "Positive" => SignStyle::Positive,
        "Negative" => SignStyle::Negative,
        _ => SignStyle::Default,
    };
    Ok(Presentation {
        object: obj,
        attribute: object("Attribute", &required(cx, obj, "Attribute")?)?,
        description: text("Description", &required(cx, obj, "Description")?)?,
        number: NumberFlags {
            as_percentage: b(cx, "bDisplayAsPercentage")?,
            rounding_float: prop_enum(cx, obj, "RoundingMode")? == "ATTRROUNDING_Float",
            percent_as_float: b(cx, "bDisplayPercentAsFloat")?,
            sign,
            no_plus_sign: b(cx, "bDontDisplayPlusSign")?,
        },
        custom_placement: b(cx, "bUseCustomNumberPlacement")?,
        no_number: b(cx, "bDontDisplayNumber")?,
    })
}

/// One info-box line: the text before the number, the number, the text after it.
pub type Line = (String, String, String);

/// `skill_stats.skill_lines`: the lines at grades `0..=grades` and the notes. A presentation shows the first unused
/// effect on its attribute (or on no attribute, when both name none); the text is the localized `Description` (key
/// `(localization_key, "Description")`) or the object's own. Without custom placement the line is the number, a
/// space and the text; `bDontDisplayNumber` leaves the number empty and keeps the space (traced for Ruin). Custom
/// placement without `$NUMBER$` shows no number (Converge; not traced, UNVERIFIED).
pub fn skill_lines(
    cx: &mut EvalCx<'_>,
    reader: &mut Reader<'_>,
    def: &SkillDef,
    localization: &ow_core::DetMap<(String, String), String>,
    grades: i32,
) -> Result<(Vec<Vec<Line>>, Vec<String>), SkillError> {
    let mut values = Vec::with_capacity(def.effects.len());
    for e in &def.effects {
        values.push((reader.value(cx, &e.base)?, reader.value(cx, &e.per_grade)?));
    }
    let count = grades
        .max(-1)
        .checked_add(1)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| SkillError::bad("a grade count out of range"))?;
    let mut lines: Vec<Vec<Line>> = vec![Vec::new(); count];
    let mut notes = Vec::new();
    let mut used = vec![false; def.effects.len()];
    for &p in &def.presentations {
        let pres = read_presentation(cx, p)?;
        let path = object_path(cx.world.parts().space, p)?;
        let key = {
            let space = cx.world.parts().space;
            let classes = collect_outer_classes(space, p)?;
            localization_key(&path, |outer| {
                classes
                    .iter()
                    .find(|(o, _)| o == outer)
                    .map(|(_, c)| c.clone())
            })
        };
        let label = localization
            .get(&(key, "Description".to_string()))
            .cloned()
            .unwrap_or_else(|| pres.description.clone());
        let found = def
            .effects
            .iter()
            .enumerate()
            .find(|(i, e)| !used[*i] && e.attribute == pres.attribute)
            .map(|(i, _)| i);
        let Some(i) = found else {
            let attr = match pres.attribute {
                Some(a) => object_path(cx.world.parts().space, a)?,
                None => "None".to_string(),
            };
            notes.push(format!("{path}: no effect on {attr}"));
            continue;
        };
        used[i] = true;
        let effect = &def.effects[i];
        let (base, per) = values[i];
        for (g, row) in lines.iter_mut().enumerate() {
            let g = i32::try_from(g).unwrap_or(i32::MAX);
            if g < effect.start_grade {
                continue;
            }
            let number = if pres.no_number {
                String::new()
            } else {
                match base.and_then(|b| shown_value(effect, g, b, per.unwrap_or(0.0))) {
                    Some(v) => format_number(f64::from(v), pres.number),
                    None => continue,
                }
            };
            let line = if pres.custom_placement {
                match label.split_once("$NUMBER$") {
                    Some((before, after)) => (before.to_string(), number, after.to_string()),
                    None => (label.clone(), String::new(), String::new()),
                }
            } else {
                (String::new(), number, format!(" {label}"))
            };
            row.push(line);
        }
        if base.is_none() {
            notes.push(format!("{path}: unresolved value"));
        }
    }
    Ok((lines, notes))
}

/// `(outer path, class path)` of every outer of `obj`, for [`localization_key`].
fn collect_outer_classes(
    space: &ObjectSpace,
    obj: ObjRef,
) -> Result<Vec<(String, String)>, SkillError> {
    let mut out = Vec::new();
    let mut cursor = space.heap.get(obj)?.outer;
    while let Some(o) = cursor {
        if out.len() >= DEPTH_BUDGET {
            return Err(SkillError::bad(
                "an outer chain longer than the depth budget (a cycle?)",
            ));
        }
        out.push((object_path(space, o)?, class_of(space, o)));
        cursor = space.heap.get(o)?.outer;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// Output folders
// ---------------------------------------------------------------------------------------------------------------

/// `prepare_skill_effects.local_output`: `path` made absolute and normalized, refused unless it lies strictly below
/// `<repo>/local/` (game-derived output never lands in a tracked folder). `..` segments are resolved lexically.
pub fn local_output(path: &Path, repo: &Path) -> Result<PathBuf, SkillError> {
    let normal = |p: &Path| -> PathBuf {
        let mut out = PathBuf::new();
        for c in p.components() {
            match c {
                Component::ParentDir => {
                    out.pop();
                }
                Component::CurDir => {}
                other => out.push(other.as_os_str()),
            }
        }
        out
    };
    let base =
        std::env::current_dir().map_err(|e| SkillError::bad(format!("no current folder: {e}")))?;
    let resolved = normal(&base.join(path));
    let local = normal(&base.join(repo).join("local"));
    if resolved != local && resolved.starts_with(&local) {
        Ok(resolved)
    } else {
        Err(SkillError::bad(format!(
            "refusing output {}: it must be inside {} (ignored game-derived output)",
            path.display(),
            local.display()
        )))
    }
}

#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod tests;
