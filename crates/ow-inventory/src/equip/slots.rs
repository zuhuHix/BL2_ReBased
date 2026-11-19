//! Owner: lane IMPL-INIT-ATTRIBUTE-SLOTS. AI-assisted (Claude Code). An item's attribute slots:
//! `WillowInventory.InitializeAttributeSlots(bool bIncludeNameParts)` and `InitializeAttributeSlotsForNameParts()`,
//! implemented from NATIVE_INIT_ATTRIBUTE_SLOTS.md alone (decision 2026-11-19-82; the analyst was lane
//! ANALYST-INIT-ATTRIBUTE-SLOTS, decision 2026-11-19-73). Every rule is the note's reading of the executable,
//! **UNVERIFIED** in the running game.
//!
//! **The lists** (section 2): on a `WillowWeapon` the definition is `DefinitionData.WeaponTypeDefinition`, the parts
//! are Body, Grip, Barrel, Sight, Stock, Elemental, Accessory1, Accessory2 and Material, and the name parts Prefix
//! then Title; on a `WillowItem` the definition is `ItemDefinition`, the parts Alpha to Theta and Material, and the
//! name parts `PrefixItemNamePartDefinition` then `TitleItemNamePartDefinition`. Empty slots are left out. Another
//! class's lists are not read: a `NotYet`.
//!
//! **InitializeAttributeSlots(bool)** (section 3), in order:
//! 1. a working set of slot records, keyed by `SlotName`, empty;
//! 2. the definition's `AttributeSlotUpgrades`, then each part's, in list order. Each entry's record is found: the
//!    working set's of that name, else a new record copied from the first `AttributeSlotEffects` entry of that name in
//!    the definition, else in the parts in list order; an entry whose slot is found nowhere is ignored. A record that
//!    is not activated, under an entry with `bActivateSlot`, becomes activated and its `EffectGrade` grows by the
//!    definition's `AttributeSlotBaseGrade`, evaluated and rounded as the note's section 3 says ([`round_base_grade`]:
//!    2x + 0.5 in single precision, to the nearest int with ties to even, then halved toward minus infinity, which
//!    is `floor(x + 0.5)` whenever 2x + 0.5 is exact); every found entry then adds its `GradeIncrease`, in 32-bit
//!    integers;
//! 3. a new record copies `SlotName`, `bExternalSlot`, `bRunEffectsAsSkill`, `bIncludeAlliesAsTarget`, the two enforce
//!    flags, `MinimumGrade`, `MaximumGrade`, `TargetInstanceDataName`, `AttributeToModify`, `ConstraintAttribute`,
//!    `ModifierType`, `BaseModifierValue` and `PerGradeUpgrade`; `bIncludeInModifierText` takes `bIncludeInFunStats`;
//!    `bActivated` starts false, `EffectGrade` and `ComputedModifierValue` 0. Those 18 are every field of the
//!    installed `AttributeSlotData`; a field a struct declares beyond them keeps the struct's default;
//! 4. the write: the slot limit is the definition's `AttributeSlotMaxActivated`, 19 when below 1 or above 19
//!    ([`super::natives::clamped_slot_max`]), and never past the declared dimension of `AttributeSlots`. The item's
//!    activated slots among the first slot-limit ones are kept, by name, untouched; they must be a packed, unique
//!    prefix (below), so their count is `K`, the number of distinct kept names. Each activated working record whose
//!    name is not kept is copied whole to index `K`, then `K + 1` and on, in working-set order (first-found order,
//!    open question 3), until the limit; then `MaximumGrade` caps its grade (when
//!    `bEnforceMaximumGrade`) and `MinimumGrade` raises it (when `bEnforceMinimumGrade`), so the minimum wins.
//!    Records that are not activated, and activated ones past the limit, are never written;
//! 5. the name-part pass, when the bool is true; 6. the value pass.
//!
//! **InitializeAttributeSlotsForNameParts()** (section 4): the same, except that the working set starts with the
//! item's own activated slots among the first slot-limit ones, and only the name parts' upgrades apply, prefix then
//! title. A name part's upgrade to one of those slots changes it **in place**: its grade grows and is not clamped
//! again (open question 4). A slot the item does not hold is looked up in the definition's and the parts'
//! `AttributeSlotEffects`, never in the name parts' own. Then the write of step 4 and the value pass.
//!
//! **The value pass** (section 5), over the first slot-limit slots. On the authority (the item's `Role` is
//! `ROLE_Authority`, looked up by its enum name in the `Role` declaration), each activated slot: the item's
//! `TempStatModifier` stack attribute takes the grade as a float in its value and its base, its modifier stack
//! emptied; then `ComputedModifierValue = BaseModifierValue + (EffectGrade - 1) x PerGradeUpgrade` in single precision,
//! copied to the slot's element of `ReplicatedAttributeSlotModifierValues`, and `bNetDirty` set. The 1 is subtracted
//! in integers, then one single-precision multiply and one add. Both values are evaluated with the item as the
//! context and no second context. An external run-as-skill slot then takes the skill part: in the definition's
//! `AttributeSlotEffectMode` 2 there is no skill and nothing more happens (not refused). A slot that is not
//! activated is left as it is. Elsewhere, each slot's `ComputedModifierValue` takes its replicated element, and
//! nothing else is written. The writes are plain property writes (no attribute-change notification is sent: the note
//! names none, UNVERIFIED).
//!
//! **The Off answers that are `NotYet`** (the note's section 7), each `NotRead` with the note as its record, each
//! text starting with the note's identifier:
//! - `IAS-NO-DEFINITION`: a None definition (the native reads the slot limit from it unguarded);
//! - `IAS-SLOT-SHAPE`: the item's activated slots among the first slot-limit ones are not a packed, unique prefix
//!   (an empty slot before an activated one, or a name twice), whether or not a copy must follow them;
//! - `IAS-BASE-GRADE-CONTEXT` and `IAS-VALUE-CONTEXT`: `AttributeSlotBaseGrade`, or `BaseModifierValue` or
//!   `PerGradeUpgrade`, when the value the call needs is not context-free, that is, reads a `BaseValueAttribute` or
//!   an `InitializationDefinition` (open question 1: lifting these is the hub's call). Context-free data is
//!   `BaseValueConstant x BaseValueScaleConstant` whatever the context;
//! - `IAS-ROUNDING-RANGE`: a base grade whose 2x + 0.5 is outside the native's exact conversion (-2^31 included, up
//!   to just below 2^31) or not finite, and a grade sum that would wrap a 32-bit int (both port restrictions);
//! - `IAS-RUNTIME-SKILL`: an activated external run-as-skill slot on the authority with an `AttributeSlotEffectMode`
//!   other than 2 (open question 2: the runtime `SkillDefinition`'s outer and name).
//! A class that is neither a `WillowWeapon` nor a `WillowItem` is this lane's own `NotYet` (its lists are not read).
//!
//! **Before any write** (the hub's ruling; the note's section 8): every step runs on copies in memory, the slot array,
//! the replicated values and the stat, in the note's planning order (the definition, the slot shape, the base pass,
//! its projected write, the name-part pass, then the value passes), and every `NotYet` and data error above is decided
//! there. The plan records its writes in the native's order and only then replays them: the base pass's copies, with
//! the bool the name-part pass's in-place grades, copies and its own value pass, then the value pass, each value pass
//! per slot in section 5's order. So `InitializeAttributeSlots(true)` keeps its two value passes as two write passes
//! (they write the same values for what Off plans). No planned `NotYet` follows a write; one that still surfaced
//! after a write would stop the call marked "stopped after partial writes", its kind kept and the writes left.
//!
//! **Only in `ParityMode::Off`.** The C++ reference has no stand-in, so under Reference both stay unbound and the stub
//! policy answers them (architecture R3).

use std::rc::Rc;

use ow_attr::api::{EvalCx, InitData};
use ow_core::ParityMode;
use ow_core::notyet::NotYet;
use ow_object::ids::{ObjRef, PropAddr, PropId, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::natives::{ITEM_ALL_PARTS, WEAPON_ALL_PARTS, clamped_slot_max};
use super::{bad, field, object_of, read, required};

/// The record these natives follow.
pub const RECORD: &str = "NATIVE_INIT_ATTRIBUTE_SLOTS.md";

/// The native keys [`register`] binds in Off.
pub const INIT_SLOTS: &str = "WillowInventory.InitializeAttributeSlots(bool)";
pub const INIT_NAME_PART_SLOTS: &str = "WillowInventory.InitializeAttributeSlotsForNameParts()";

/// The fields a new record copies from its `AttributeSlotEffectData` entry as they are (section 3 step 3).
const COPIED: [&str; 14] = [
    "SlotName",
    "bExternalSlot",
    "bRunEffectsAsSkill",
    "bIncludeAlliesAsTarget",
    "bEnforceMinimumGrade",
    "bEnforceMaximumGrade",
    "MinimumGrade",
    "MaximumGrade",
    "TargetInstanceDataName",
    "AttributeToModify",
    "ConstraintAttribute",
    "ModifierType",
    "BaseModifierValue",
    "PerGradeUpgrade",
];

/// One class's lists (section 2).
struct Lists {
    definition: &'static str,
    slots: &'static [&'static str; 11],
}

/// The class rule for `item`: a weapon's or an item's lists; another class's are not read.
fn lists_of(vm: &mut Vm, item: ObjRef, what: &str) -> Result<Lists, VmError> {
    let class = vm.space.heap.get(item)?.class;
    for (path, definition, slots) in [
        ("WillowGame.WillowWeapon", "WeaponTypeDefinition", &WEAPON_ALL_PARTS),
        ("WillowGame.WillowItem", "ItemDefinition", &ITEM_ALL_PARTS),
    ] {
        if let Ok(c) = vm.find_class(path)
            && vm.space.classes.is_child_of(class, c)
        {
            return Ok(Lists { definition, slots });
        }
    }
    Err(NotYet::not_read(
        format!(
            "{what} on a {}, neither a WillowWeapon nor a WillowItem (its lists are not read)",
            ow_attr::eval::class_path(&vm.space, item)
        ),
        RECORD,
    )
    .into())
}

/// A struct value's field `name` set to `v` (declared, else a data error).
fn set_field(vm: &Vm, s: &mut Value, name: &str, v: Value, what: &str) -> Result<(), VmError> {
    let ty = s
        .as_struct()
        .ok_or_else(|| bad(what, format!("expected a struct for field {name}, found {}", s.kind_name())))?
        .ty;
    let at = vm
        .space
        .names
        .find(name)
        .and_then(|id| vm.space.classes.struct_layout(ty)?.field(id))
        .map(|(i, _)| i)
        .ok_or_else(|| bad(what, format!("the slot struct declares no {name}")))?;
    let fields = &mut s.struct_mut().ok_or_else(|| bad(what, "not a struct"))?.fields;
    let slot = fields
        .get_mut(usize::try_from(at).map_err(|_| bad(what, "a field index past the platform's range"))?)
        .ok_or_else(|| bad(what, format!("field {name} is past the struct's value")))?;
    *slot = v;
    Ok(())
}

fn flag(vm: &Vm, s: &Value, name: &str, what: &str) -> Result<bool, VmError> {
    match field(vm, s, name)? {
        Value::Bool(b) => Ok(b),
        other => Err(bad(what, format!("{name} holds {}, not a bool", other.kind_name()))),
    }
}

fn int_field(vm: &Vm, s: &Value, name: &str, what: &str) -> Result<i32, VmError> {
    match field(vm, s, name)? {
        Value::Int(i) => Ok(i),
        other => Err(bad(what, format!("{name} holds {}, not an int", other.kind_name()))),
    }
}

fn name_field(vm: &Vm, s: &Value, what: &str) -> Result<Value, VmError> {
    match field(vm, s, "SlotName")? {
        n @ Value::Name(_) => Ok(n),
        other => Err(bad(what, format!("SlotName holds {}, not a name", other.kind_name()))),
    }
}

/// The elements of an array property `name` of `holder`; none when the class declares no such property.
fn list(vm: &Vm, holder: ObjRef, name: &str) -> Result<Vec<Value>, VmError> {
    match read(vm, holder, name)? {
        Some(v) => Ok(ow_attr::eval::elements(name, &v)?.to_vec()),
        None => Ok(Vec::new()),
    }
}

/// Whether init data reads nothing but its constants (section 7).
fn context_free(data: &InitData) -> bool {
    data.base_value_attribute.is_none() && data.initialization_definition.is_none()
}

/// Init data `v` evaluated when context-free (the item as the context, no second context), else the named `NotYet`
/// `id` of open question 1 (`IAS-BASE-GRADE-CONTEXT` or `IAS-VALUE-CONTEXT`).
fn evaluate(vm: &mut Vm, v: &Value, item: ObjRef, id: &str, subject: &str, what: &str) -> Result<f32, VmError> {
    let data = ow_attr::eval::init_data(&vm.space, v)?;
    if !context_free(&data) {
        return Err(not_yet(
            id,
            what,
            &format!("{subject} reads an attribute or an initialization definition (lifting this on the note's reading of the context is the hub's call, open question 1)"),
        ));
    }
    let attrs = vm.attrs.clone();
    Ok(attrs.evaluate_init(&data, Some(item), None, &mut EvalCx::new(vm))?)
}

/// A named `NotYet` of the note's section 7: its text starts with its identifier.
fn not_yet(id: &str, what: &str, subject: &str) -> VmError {
    NotYet::not_read(format!("{id}: {what}: {subject}"), RECORD).into()
}

/// The base grade's rounding (section 3, "Types and the rounding"): `y` = 2x + 0.5 in single precision, `n` = `y` to
/// the nearest 32-bit int with ties to even, then `n` shifted right by one (`floor(n / 2)`). The conversion is exact
/// from -2^31 (included) to just below 2^31; any other `y`, infinite or not a number included, is `NotYet`
/// IAS-ROUNDING-RANGE (the native gives the processor's "integer indefinite" value there, which the port does not
/// reproduce).
pub fn round_base_grade(x: f32, what: &str) -> Result<i32, VmError> {
    let y = 2.0f32 * x + 0.5f32;
    if !(-2_147_483_648.0f32..2_147_483_648.0f32).contains(&y) {
        return Err(not_yet(
            "IAS-ROUNDING-RANGE",
            what,
            &format!("AttributeSlotBaseGrade {x} gives 2x + 0.5 = {y}, outside the int range the port reproduces"),
        ));
    }
    Ok((y.round_ties_even() as i32) >> 1)
}

/// One record of the working set: its value and, in the name-part pass, the item slot it stands for.
struct Rec {
    value: Value,
    in_place: Option<usize>,
}

/// What one call reads, and the copies it changes in memory.
struct State {
    item: ObjRef,
    what: &'static str,
    definition: ObjRef,
    parts: Vec<ObjRef>,
    names: Vec<ObjRef>,
    /// The item's `AttributeSlots`, every declared element.
    slots: Vec<Value>,
    slot_ty: StructId,
    slots_prop: PropId,
    limit: usize,
    /// `ReplicatedAttributeSlotModifierValues`, every declared element.
    replicated: Vec<f32>,
    replicated_prop: PropId,
    authority: bool,
    /// The planned writes, in the native's order (the note's section 8 step 7).
    ops: Vec<Op>,
    /// The definition's `AttributeSlotBaseGrade`, evaluated once when an activation needs it.
    base_grade: Option<i32>,
}

/// The declared static array `name` of `item`: its property, element kind and every element.
fn static_array(vm: &Vm, item: ObjRef, name: &str, what: &str) -> Result<(PropId, PropKind, Vec<Value>), VmError> {
    let class = vm.space.heap.get(item)?.class;
    let declared = |m: &str| bad(what, format!("{} {m}", ow_attr::eval::class_path(&vm.space, item)));
    let p = vm.space.prop(class, name).ok_or_else(|| declared(&format!("declares no {name}")))?;
    let decl = vm.space.classes.decl(p).ok_or_else(|| declared(&format!("has no declaration for {name}")))?;
    let dim = u32::try_from(decl.array_dim).map_err(|_| declared(&format!("declares {name} with a negative dimension")))?;
    let kind = decl.kind.clone();
    let values = (0..dim)
        .map(|i| Ok(vm.space.read(item, PropAddr::element(p, i))?.clone()))
        .collect::<Result<Vec<_>, VmError>>()?;
    Ok((p, kind, values))
}

/// Whether `item`'s `Role` is the `ROLE_Authority` entry of the enum its declaration names.
fn is_authority(vm: &Vm, item: ObjRef, what: &str) -> Result<bool, VmError> {
    let class = vm.space.heap.get(item)?.class;
    let p = vm
        .space
        .prop(class, "Role")
        .ok_or_else(|| bad(what, format!("{} declares no Role", ow_attr::eval::class_path(&vm.space, item))))?;
    let authority = match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(PropKind::Byte { enum_: Some(e) }) => vm
            .space
            .classes
            .enum_layout(*e)
            .and_then(|l| vm.space.names.find("ROLE_Authority").and_then(|n| l.index_of(n))),
        _ => return Err(bad(what, "Role is not declared as an enum byte")),
    }
    .ok_or_else(|| bad(what, "Role's enum has no ROLE_Authority"))?;
    match vm.space.read(item, PropAddr::scalar(p))? {
        Value::Byte(b) => Ok(usize::from(*b) == authority),
        other => Err(bad(what, format!("Role holds {}, not a byte", other.kind_name()))),
    }
}

impl State {
    fn read(vm: &mut Vm, item: ObjRef, what: &'static str) -> Result<State, VmError> {
        let lists = lists_of(vm, item, what)?;
        let data = required(vm, item, "DefinitionData", what)?;
        let Some(definition) = object_of(&field(vm, &data, lists.definition)?, what)? else {
            return Err(not_yet(
                "IAS-NO-DEFINITION",
                what,
                &format!("an item without a {} (the native reads the slot limit from it unguarded)", lists.definition),
            ));
        };
        let mut parts = Vec::new();
        let mut names = Vec::new();
        for (i, slot) in lists.slots.iter().enumerate() {
            if let Some(p) = object_of(&field(vm, &data, slot)?, what)? {
                if i < 9 { parts.push(p) } else { names.push(p) }
            }
        }
        for o in std::iter::once(&definition).chain(parts.iter()).chain(names.iter()) {
            super::value::loaded(vm, *o)?;
        }
        let stored = match required(vm, definition, "AttributeSlotMaxActivated", what)? {
            Value::Int(i) => i,
            other => return Err(bad(what, format!("AttributeSlotMaxActivated holds {}, not an int", other.kind_name()))),
        };
        let (slots_prop, kind, slots) = static_array(vm, item, "AttributeSlots", what)?;
        let PropKind::Struct(slot_ty) = kind else {
            return Err(bad(what, "AttributeSlots is not declared as slot structs"));
        };
        let limit = usize::try_from(clamped_slot_max(stored)).unwrap_or(0).min(slots.len());
        let (replicated_prop, kind, values) = static_array(vm, item, "ReplicatedAttributeSlotModifierValues", what)?;
        if !matches!(kind, PropKind::Float) || values.len() < limit {
            return Err(bad(what, "ReplicatedAttributeSlotModifierValues is not a float array of at least the slot limit"));
        }
        let replicated = values
            .iter()
            .map(|v| match v {
                Value::Float(f) => Ok(*f),
                other => Err(bad(what, format!("a replicated value holds {}", other.kind_name()))),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let authority = is_authority(vm, item, what)?;
        Ok(State {
            item,
            what,
            definition,
            parts,
            names,
            slots,
            slot_ty,
            slots_prop,
            limit,
            replicated,
            replicated_prop,
            authority,
            ops: Vec::new(),
            base_grade: None,
        })
    }

    /// A new record from the first `AttributeSlotEffects` entry named `name`, in the definition, then the parts in
    /// list order (section 3 step 3); `None` when none names it.
    fn new_record(&self, vm: &Vm, name: &Value) -> Result<Option<Value>, VmError> {
        let what = self.what;
        for holder in std::iter::once(self.definition).chain(self.parts.iter().copied()) {
            for entry in list(vm, holder, "AttributeSlotEffects")? {
                if name_field(vm, &entry, what)? != *name {
                    continue;
                }
                let mut rec = vm.space.classes.struct_defaults(self.slot_ty).map_err(|e| bad(what, e.to_string()))?;
                for f in COPIED {
                    set_field(vm, &mut rec, f, field(vm, &entry, f)?, what)?;
                }
                set_field(vm, &mut rec, "bIncludeInModifierText", field(vm, &entry, "bIncludeInFunStats")?, what)?;
                set_field(vm, &mut rec, "bActivated", Value::Bool(false), what)?;
                set_field(vm, &mut rec, "EffectGrade", Value::Int(0), what)?;
                set_field(vm, &mut rec, "ComputedModifierValue", Value::Float(0.0), what)?;
                return Ok(Some(rec));
            }
        }
        Ok(None)
    }

    /// The item's activated slots among the first slot-limit ones: (index, name).
    fn kept(&self, vm: &Vm) -> Result<Vec<(usize, Value)>, VmError> {
        let mut out = Vec::new();
        for (i, s) in self.slots[..self.limit].iter().enumerate() {
            if flag(vm, s, "bActivated", self.what)? {
                out.push((i, name_field(vm, s, self.what)?));
            }
        }
        Ok(out)
    }

    /// The item's slots before the call (section 3 step 4.5): the activated ones among the first slot-limit slots must
    /// be a packed, unique prefix (indices 0 to K - 1, distinct names), else `NotYet` IAS-SLOT-SHAPE.
    fn check_shape(&self, vm: &Vm) -> Result<(), VmError> {
        let kept = self.kept(vm)?;
        for (k, (i, name)) in kept.iter().enumerate() {
            if *i != k {
                return Err(not_yet(
                    "IAS-SLOT-SHAPE",
                    self.what,
                    &format!("activated slot {i} follows an empty slot (not a packed prefix)"),
                ));
            }
            if kept[..k].iter().any(|(_, n)| n == name) {
                return Err(not_yet("IAS-SLOT-SHAPE", self.what, &format!("activated slot {i} repeats an earlier name")));
            }
        }
        Ok(())
    }

    /// Applies `holders`' `AttributeSlotUpgrades` to `working`, in order (section 3 step 2).
    fn upgrade(&mut self, vm: &mut Vm, holders: &[ObjRef], working: &mut Vec<Rec>) -> Result<(), VmError> {
        let what = self.what;
        for &holder in holders {
            for entry in list(vm, holder, "AttributeSlotUpgrades")? {
                let name = name_field(vm, &entry, what)?;
                let mut at = None;
                for (i, r) in working.iter().enumerate() {
                    if name_field(vm, &r.value, what)? == name {
                        at = Some(i);
                        break;
                    }
                }
                let i = match at {
                    Some(i) => i,
                    None => match self.new_record(vm, &name)? {
                        Some(value) => {
                            working.push(Rec { value, in_place: None });
                            working.len() - 1
                        }
                        None => continue,
                    },
                };
                let mut grade = int_field(vm, &working[i].value, "EffectGrade", what)?;
                if !flag(vm, &working[i].value, "bActivated", what)? && flag(vm, &entry, "bActivateSlot", what)? {
                    set_field(vm, &mut working[i].value, "bActivated", Value::Bool(true), what)?;
                    grade = grade.checked_add(self.base_grade(vm)?).ok_or_else(|| wraps(what))?;
                }
                let increase = int_field(vm, &entry, "GradeIncrease", what)?;
                grade = grade.checked_add(increase).ok_or_else(|| wraps(what))?;
                set_field(vm, &mut working[i].value, "EffectGrade", Value::Int(grade), what)?;
            }
        }
        Ok(())
    }

    fn base_grade(&mut self, vm: &mut Vm) -> Result<i32, VmError> {
        if let Some(g) = self.base_grade {
            return Ok(g);
        }
        let v = required(vm, self.definition, "AttributeSlotBaseGrade", self.what)?;
        let x = evaluate(vm, &v, self.item, "IAS-BASE-GRADE-CONTEXT", "the definition's AttributeSlotBaseGrade", self.what)?;
        let g = round_base_grade(x, self.what)?;
        self.base_grade = Some(g);
        Ok(g)
    }

    /// Writes `working` into the slot copies (section 3 step 4): in-place records back to their slot, unclamped; each
    /// activated record not kept after the kept slots, clamped, up to the limit.
    fn write_records(&mut self, vm: &Vm, working: Vec<Rec>) -> Result<(), VmError> {
        let what = self.what;
        let kept = self.kept(vm)?;
        let mut copies = Vec::new();
        for r in working {
            if let Some(i) = r.in_place {
                self.ops.push(Op::Slot(i, r.value.clone()));
                self.slots[i] = r.value;
                continue;
            }
            if !flag(vm, &r.value, "bActivated", what)? {
                continue;
            }
            let name = name_field(vm, &r.value, what)?;
            if kept.iter().any(|(_, n)| *n == name) {
                continue;
            }
            copies.push(r.value);
        }
        // K: the number of distinct kept names; the shape check (IAS-SLOT-SHAPE) made the kept slots a packed, unique
        // prefix, so it is also their count and the first copy's index.
        let mut next = kept.len();
        for mut rec in copies {
            if next >= self.limit {
                break;
            }
            let mut grade = int_field(vm, &rec, "EffectGrade", what)?;
            if flag(vm, &rec, "bEnforceMaximumGrade", what)? {
                grade = grade.min(int_field(vm, &rec, "MaximumGrade", what)?);
            }
            if flag(vm, &rec, "bEnforceMinimumGrade", what)? {
                grade = grade.max(int_field(vm, &rec, "MinimumGrade", what)?);
            }
            set_field(vm, &mut rec, "EffectGrade", Value::Int(grade), what)?;
            self.ops.push(Op::Slot(next, rec.clone()));
            self.slots[next] = rec;
            next += 1;
        }
        Ok(())
    }

    /// The value pass on the copies (section 5).
    fn value_pass(&mut self, vm: &mut Vm) -> Result<(), VmError> {
        let what = self.what;
        for i in 0..self.limit {
            if !self.authority {
                let r = self.replicated[i];
                set_field(vm, &mut self.slots[i], "ComputedModifierValue", Value::Float(r), what)?;
                self.ops.push(Op::Slot(i, self.slots[i].clone()));
                continue;
            }
            let slot = self.slots[i].clone();
            if !flag(vm, &slot, "bActivated", what)? {
                continue;
            }
            let grade = int_field(vm, &slot, "EffectGrade", what)?;
            self.ops.push(Op::Stat(grade as f32));
            let base = evaluate(vm, &field(vm, &slot, "BaseModifierValue")?, self.item, "IAS-VALUE-CONTEXT", "a slot's BaseModifierValue", what)?;
            let per = evaluate(vm, &field(vm, &slot, "PerGradeUpgrade")?, self.item, "IAS-VALUE-CONTEXT", "a slot's PerGradeUpgrade", what)?;
            // The 1 is subtracted in integers, then one single-precision multiply and one add (no fused multiply-add).
            let steps = grade.checked_sub(1).ok_or_else(|| bad(what, "EffectGrade - 1 overflows"))?;
            let computed = base + (steps as f32) * per;
            set_field(vm, &mut self.slots[i], "ComputedModifierValue", Value::Float(computed), what)?;
            self.ops.push(Op::Slot(i, self.slots[i].clone()));
            self.replicated[i] = computed;
            self.ops.push(Op::Replicated(i, computed));
            self.ops.push(Op::NetDirty);
            if flag(vm, &slot, "bExternalSlot", what)? && flag(vm, &slot, "bRunEffectsAsSkill", what)? {
                let mode = match required(vm, self.definition, "AttributeSlotEffectMode", what)? {
                    Value::Byte(b) => b,
                    other => return Err(bad(what, format!("AttributeSlotEffectMode holds {}, not a byte", other.kind_name()))),
                };
                // Mode 2: no skill, nothing appended, RuntimeAttributeSlotSkill left as it was (the value, not refused).
                if mode != 2 {
                    return Err(not_yet(
                        "IAS-RUNTIME-SKILL",
                        what,
                        &format!("an activated external run-as-skill slot with AttributeSlotEffectMode {mode} (the runtime SkillDefinition's outer and name are not read, open question 2)"),
                    ));
                }
            }
        }
        Ok(())
    }

    /// The name-part pass (section 4).
    fn name_part_pass(&mut self, vm: &mut Vm) -> Result<(), VmError> {
        let mut working = Vec::new();
        for (i, _) in self.kept(vm)? {
            working.push(Rec { value: self.slots[i].clone(), in_place: Some(i) });
        }
        let names = self.names.clone();
        self.upgrade(vm, &names, &mut working)?;
        self.write_records(vm, working)?;
        self.value_pass(vm)
    }

    /// The planned writes, replayed in order (module documentation). Every planned `NotYet` was decided before the
    /// first; should a `NotYet` still surface after a write (a port defect), it stops the call with the partial-writes
    /// mark, its kind and record kept, and the writes already made stay. Other errors pass unchanged.
    fn commit(self, vm: &mut Vm) -> Result<(), VmError> {
        let what = self.what;
        let mut wrote = false;
        for op in self.ops {
            let r = match op {
                Op::Slot(i, v) => index(i).and_then(|i| Ok(vm.space.write(self.item, PropAddr::element(self.slots_prop, i), v)?)),
                Op::Replicated(i, f) => {
                    index(i).and_then(|i| Ok(vm.space.write(self.item, PropAddr::element(self.replicated_prop, i), Value::Float(f))?))
                }
                Op::Stat(grade) => stat_companions(vm, self.item, what).and_then(|c| {
                    vm.space.write(self.item, PropAddr::scalar(c.value), Value::Float(grade))?;
                    vm.space.write(self.item, PropAddr::scalar(c.base), Value::Float(grade))?;
                    vm.space.write(self.item, PropAddr::scalar(c.stack), Value::array(Vec::new()))?;
                    Ok(())
                }),
                Op::NetDirty => super::write(vm, self.item, "bNetDirty", Value::Bool(true), what),
            };
            match r {
                Ok(()) => wrote = true,
                Err(VmError::NotYet(n)) if wrote && !n.what.contains(PARTIAL_WRITES) => {
                    return Err(NotYet::new(format!("{PARTIAL_WRITES}: {}", n.what), n.missing, n.record).into());
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

/// One planned write.
enum Op {
    /// An element of `AttributeSlots`, whole.
    Slot(usize, Value),
    /// An element of `ReplicatedAttributeSlotModifierValues`.
    Replicated(usize, f32),
    /// `TempStatModifier`'s value and base set to the grade, its stack emptied.
    Stat(f32),
    NetDirty,
}

/// The mark of a failure after state was written (the text of `ow_behavior::kernel::run::PARTIAL_WRITES`).
pub const PARTIAL_WRITES: &str = "stopped after partial writes";

/// A grade sum that would wrap: the port's restriction, `NotYet` IAS-ROUNDING-RANGE (the note's section 3).
fn wraps(what: &str) -> VmError {
    not_yet("IAS-ROUNDING-RANGE", what, "a grade sum would wrap a 32-bit int (the native wraps; the port does not reproduce it)")
}

fn index(i: usize) -> Result<u32, VmError> {
    u32::try_from(i).map_err(|_| VmError::Script { function: RECORD.into(), message: "a slot index past u32".into() })
}

/// `TempStatModifier`'s companions: a float stack attribute (checked before any write).
fn stat_companions(vm: &Vm, item: ObjRef, what: &str) -> Result<ow_object::layout::AttrCompanions, VmError> {
    let class = vm.space.heap.get(item)?.class;
    let p = vm
        .space
        .prop(class, "TempStatModifier")
        .ok_or_else(|| bad(what, format!("{} declares no TempStatModifier", ow_attr::eval::class_path(&vm.space, item))))?;
    match vm.space.classes.decl(p) {
        Some(d) if matches!(d.kind, PropKind::FloatAttribute) => {
            d.attr.ok_or_else(|| bad(what, "TempStatModifier is not a stack attribute"))
        }
        _ => Err(bad(what, "TempStatModifier is not a float attribute")),
    }
}

/// One call (module documentation): `name_parts` is the bool of `InitializeAttributeSlots`; `only_name_parts` makes
/// it `InitializeAttributeSlotsForNameParts`.
pub fn initialize(vm: &mut Vm, item: ObjRef, name_parts: bool, only_name_parts: bool) -> Result<(), VmError> {
    let what = if only_name_parts { INIT_NAME_PART_SLOTS } else { INIT_SLOTS };
    let mut s = State::read(vm, item, what)?;
    s.check_shape(vm)?;
    if s.authority {
        // Checked first, so a bad declaration is refused before anything is written.
        stat_companions(vm, item, what)?;
        if read(vm, item, "bNetDirty")?.is_none() {
            return Err(bad(what, "the item declares no bNetDirty"));
        }
    }
    if only_name_parts {
        s.name_part_pass(vm)?;
    } else {
        let mut working = Vec::new();
        let holders: Vec<ObjRef> = std::iter::once(s.definition).chain(s.parts.iter().copied()).collect();
        s.upgrade(vm, &holders, &mut working)?;
        s.write_records(vm, working)?;
        if name_parts {
            s.name_part_pass(vm)?;
        }
        s.value_pass(vm)?;
    }
    s.commit(vm)
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without an inventory"))
}

/// Binds the module's natives in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let natives: [(&str, NativeFn); 2] = [
        (
            INIT_SLOTS,
            Rc::new(|c: &mut NativeCall<'_>| {
                let item = this(c, INIT_SLOTS)?;
                let names = c.has(0) && c.arg(0)?.truth(&c.vm.space.names);
                initialize(c.vm, item, names, false)?;
                Ok(Value::None)
            }),
        ),
        (
            INIT_NAME_PART_SLOTS,
            Rc::new(|c: &mut NativeCall<'_>| {
                let item = this(c, INIT_NAME_PART_SLOTS)?;
                initialize(c.vm, item, false, true)?;
                Ok(Value::None)
            }),
        ),
    ];
    for (key, f) in natives {
        vm.register_native(key, f)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
