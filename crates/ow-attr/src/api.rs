//! Owner: lane W2-ATTRAPI. The attribute system's upward edge (architecture section 3.5): the [`AttrWorld`] trait a
//! running or offline world implements, the evaluation context [`EvalCx`], the resolver traits, the data the
//! evaluator reads ([`InitData`], [`ExprData`], [`EffectData`]), the outcomes of the modifier operations and
//! [`AttrError`].
//!
//! This file is a contract: it holds shapes, not rules. Every rule behind these shapes is in
//! `docs/verification/NATIVE_ATTRIBUTES.md` and is UNVERIFIED in the running game; the lanes that implement them
//! (W3-ATTREVAL, W3-ATTRCTX, W3-ATTREFFECTS) keep that label.
//!
//! Borrowing: the world is lent once, as `&mut dyn AttrWorld` inside an [`EvalCx`]. Whatever needs the object space
//! and the definition store together takes both from one [`AttrWorld::parts`] call, so no two methods ever borrow the
//! world mutably at the same time (E0499).

use std::borrow::Cow;
use std::fmt;

use ow_core::NotYet;
use ow_core::rand::{CrtStream, Rand15};
use ow_defs::DefStore;
use ow_defs::store::DefError;
use ow_object::heap::{HeapError, ObjectSpace};
use ow_object::ids::{ObjRef, PropId};
use ow_object::layout::{AttrCompanions, LayoutError};

use crate::math::PowForm;
use crate::stack::{CombineForm, ModifierId, ModifierType, NumKind};

use crate::registry::AttributeSystem;

// ---------------------------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------------------------

/// Why an attribute operation could not run. A refusal the game itself answers with `false` (a constant attribute
/// asked to take a modifier, a context that does not resolve) is **not** an error: it is an outcome
/// ([`AddModifierOutcome`], [`RemoveModifierOutcome`], `Ok(None)`).
///
/// `Clone + PartialEq` but not `Eq` on purpose: `ow_balance::BalanceError` and `ow_vm::VmError` carry it as
/// `Attr(AttrError)` and derive `Clone + PartialEq` themselves. `#[non_exhaustive]`; the `Def` variant was added by
/// W3-DEFS (an additive change, section 7).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum AttrError {
    /// The rule, the resolver class or the layer is not there yet (architecture section 4).
    NotYet(NotYet),
    /// A heap read or write was refused (a stale handle, a wrong value kind).
    Heap(HeapError),
    /// A class or property lookup was refused.
    Layout(LayoutError),
    /// The data breaks an assumption the engine treats as a data bug, not a gameplay path, for example a stack
    /// attribute whose modifier stack property cannot be found (NATIVE_ATTRIBUTES.md section 3, "Missing stack").
    BadData { reason: Cow<'static, str> },
    /// A resolver class path was registered twice in one [`AttributeSystem`].
    DuplicateResolver { class: String },
    /// An error raised by the world implementation (for example script run by the VM's change notification),
    /// carried as text because `ow-attr` cannot name the world's error type.
    World(String),
    /// The definition store failed (lookup, load, overlay). Its `NotYet` and heap errors are not here: they convert
    /// to [`AttrError::NotYet`] and [`AttrError::Heap`] (`From<DefError>`).
    Def(DefError),
}

impl fmt::Display for AttrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttrError::NotYet(n) => write!(f, "{} {} ({})", n.what, n.missing, n.record),
            AttrError::Heap(e) => write!(f, "heap: {e}"),
            AttrError::Layout(e) => write!(f, "layout: {e}"),
            AttrError::BadData { reason } => write!(f, "bad attribute data: {reason}"),
            AttrError::DuplicateResolver { class } => {
                write!(f, "resolver class {class} registered twice")
            }
            AttrError::World(message) => write!(f, "world: {message}"),
            AttrError::Def(e) => write!(f, "definitions: {e}"),
        }
    }
}

impl std::error::Error for AttrError {}

impl From<NotYet> for AttrError {
    fn from(n: NotYet) -> Self {
        AttrError::NotYet(n)
    }
}

impl From<HeapError> for AttrError {
    fn from(e: HeapError) -> Self {
        AttrError::Heap(e)
    }
}

impl From<LayoutError> for AttrError {
    fn from(e: LayoutError) -> Self {
        AttrError::Layout(e)
    }
}

/// `NotYet` and heap errors keep their own variants (a definition that is not there yet is a `NotYet` like any other);
/// the rest is [`AttrError::Def`].
impl From<DefError> for AttrError {
    fn from(e: DefError) -> Self {
        match e {
            DefError::NotYet(n) => AttrError::NotYet(n),
            DefError::Heap(h) => AttrError::Heap(h),
            other => AttrError::Def(other),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The world
// ---------------------------------------------------------------------------------------------------------------

/// The object space and the definition store, borrowed together from one [`AttrWorld::parts`] call.
///
/// `defs` is `None` in a world that has no store yet (the test worlds before W3-DEFS); [`WorldParts::defs`] turns
/// that into `NotYet { LayerAbsent("ow-defs") }`.
#[non_exhaustive]
pub struct WorldParts<'a> {
    pub space: &'a mut ObjectSpace,
    pub defs: Option<&'a mut DefStore>,
}

impl<'a> WorldParts<'a> {
    pub fn new(space: &'a mut ObjectSpace, defs: Option<&'a mut DefStore>) -> Self {
        WorldParts { space, defs }
    }

    /// The store, or `NotYet { LayerAbsent("ow-defs") }` when this world has none.
    pub fn defs(&mut self) -> Result<&mut DefStore, AttrError> {
        match self.defs.as_deref_mut() {
            Some(d) => Ok(d),
            None => Err(NotYet::layer_absent(
                "the definition store of this world",
                "ow-defs",
                "architecture.md section 3.5 (W2-ATTRAPI)",
            )
            .into()),
        }
    }
}

/// An engine-side lookup a context resolver cannot do with property reads alone (NATIVE_ATTRIBUTES.md section 5).
/// Plain property hops (a controller's `Pawn`, a pawn's `Weapon`) are read from the object space by the resolver
/// itself; what needs the running engine (a pool manager, an inventory manager, an interface call) goes through
/// [`AttrWorld::engine_query`].
///
/// `#[non_exhaustive]`: W3-ATTRCTX adds the queries its resolvers need (an additive change, section 7).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineQuery {
    /// The source's resource pool for the `ResourceDefinition` object `resource` (ResourcePool resolvers).
    ResourcePool { resource: ObjRef },
    /// The item the source's inventory manager holds at the `EquipmentLocation` byte `location`
    /// (`EquippedInventoryAttributeContextResolver`).
    EquippedInventory { location: u8 },
    /// The source's item matching the `InventoryDefinition` object `definition`, only equipped items when
    /// `equipped_only` (`InventoryAttributeContextResolver`; low confidence in the record).
    Inventory {
        definition: ObjRef,
        equipped_only: bool,
    },
    /// The player controller reachable from the source (itself, or through its owner interface).
    PlayerController,
    /// The object a pawn or interactive object redirects its attribute context to (`GetAttributeContextSource`,
    /// NATIVE_ATTRIBUTES.md section 10).
    AttributeContextSource,
    /// The skill object for the `SkillDefinition` object `definition` in the player's skill tree reachable from the
    /// source (`PlayerSkillAttributeValueResolver`, NATIVE_ATTRIBUTES.md section 6); `Ok(None)` when the player or
    /// the tree is missing, or the tree has no such skill. Added by W3-ATTREVAL (an additive change, section 7).
    PlayerSkill { definition: ObjRef },
}

/// The world the attribute system reads and edits: the running VM (`ow_vm::Vm`), an offline world
/// (`ow_balance::OfflineWorld`), or an adapter (`ow_items::RollWorld`, which forwards everything except [`rng`] and
/// [`attribute_changed`]; it forwards the notification only over a world that runs no script, else answers `NotYet`).
///
/// Implementations must give every borrow from one method at a time: [`parts`] is the only way to the object
/// space and the store, and it gives both at once.
///
/// **State that outlives one evaluation lives in the world**, so a nested evaluation (script run by
/// [`attribute_changed`], which builds a fresh `EvalCx::new(world)`) sees the same state: the [`GlobalSlots`] (one
/// process-wide pair in the game) and the generators. The only per-call state is [`EvalCx::forms`], and
/// [`attribute_changed`] receives it so the nested context is rebuilt exactly.
///
/// **Adapters** that override [`rng`] (a roll's one generator) must not hand [`attribute_changed`] to a world that
/// runs script with its own generators, or the nested evaluation would bypass the roll's: such an adapter runs the
/// notification against itself, or runs no script (`Ok(())` for a notification its rule set has none for, or
/// `NotYet`).
///
/// [`rng`]: AttrWorld::rng
/// [`parts`]: AttrWorld::parts
/// [`attribute_changed`]: AttrWorld::attribute_changed
pub trait AttrWorld {
    /// The object space and the definition store, borrowed together (one split borrow: `DefStore` lookups need the
    /// space too).
    fn parts(&mut self) -> WorldParts<'_>;

    /// The engine globals object: what `NoContextNeededAttributeContextResolver` returns for a None source
    /// (NATIVE_ATTRIBUTES.md section 5: that resolver never answers None; `progression.cpp:84`). A world without one
    /// answers `NotYet { LayerAbsent(..) }`, never a silent None, so a constant attribute cannot quietly fall back to
    /// its `BaseValueConstant` or a default.
    fn globals(&mut self) -> Result<ObjRef, AttrError>;

    /// The game info object (`GameInfoAttributeContextResolver`). `Ok(None)` only for a world that models no game
    /// info at all (an offline tool's world, a unit test); the running game always has one
    /// (`progression.cpp:87` treats the resolver as always resolving), so the VM world answers `Some` or an error.
    fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError>;

    /// An engine-side lookup for a context resolver. `Ok(None)`: the lookup ran and found nothing (the chain stops).
    /// The default answers `NotYet { LayerAbsent("ow-world") }`, which is what every world without engine state
    /// gives.
    fn engine_query(
        &mut self,
        q: EngineQuery,
        source: ObjRef,
    ) -> Result<Option<ObjRef>, AttrError> {
        let _ = source;
        Err(NotYet::layer_absent(
            format!("attribute engine query {q:?}"),
            "ow-world",
            "NATIVE_ATTRIBUTES.md section 5",
        )
        .into())
    }

    /// `GlobalAttributeValueResolver`'s slots. The world owns them because the game keeps one process-wide pair
    /// (`SetGlobalAttributeValue` stores into it; the XP curve sets slot 0 before it evaluates).
    fn global_slots(&mut self) -> &mut GlobalSlots;

    /// The change notification (NATIVE_ATTRIBUTES.md section 4): called after a successful add, remove or set-base
    /// that was not suppressed, **only** for a property carrying the notify flag (`PropertyDecl::notifies`).
    /// `value_prop` is the attribute's value property on `owner`; `forms` are the calling evaluation's
    /// [`EvalCx::forms`], for a nested `EvalCx::new(self).with_forms(forms)`. The VM implementation dispatches the
    /// owner class's override (`ResourcePool` clamps `CurrentValue`, `Skill` flags a refresh on `Grade`), which may
    /// run script, so it has the whole world.
    fn attribute_changed(
        &mut self,
        owner: ObjRef,
        value_prop: PropId,
        forms: NumForms,
    ) -> Result<(), AttrError>;

    /// The generator for stream `s`: random variance in initialization data draws from
    /// `CrtStream::AttributeVariance`. A roll's world returns its one generator for every stream. A world with no
    /// generator answers `NotYet { LayerAbsent(..) }`, as the oracle refuses ("no random source",
    /// `progression.cpp:209`).
    fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError>;
}

// ---------------------------------------------------------------------------------------------------------------
// Evaluation context
// ---------------------------------------------------------------------------------------------------------------

/// `GlobalAttributeValueResolver`'s two slots (`EGlobalAttributes`; NATIVE_ATTRIBUTES.md section 6): 0
/// `ExperiencePointTestLevel`, 1 `BadassTokenTestRank`. The XP curve sets slot 0 to the level it evaluates
/// (NATIVE_PROGRESSION.md section 3). Owned by the world ([`AttrWorld::global_slots`]), as the game keeps one pair.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GlobalSlots {
    /// Slot 0, `ExperiencePointTestLevel`.
    pub experience_point_test_level: f32,
    /// Slot 1, `BadassTokenTestRank`.
    pub badass_token_test_rank: f32,
}

impl GlobalSlots {
    /// `GetGlobalAttributeValue`: the slot, 0.0 for an index at or beyond 2. Status: UNVERIFIED
    /// (NATIVE_ATTRIBUTES.md section 6).
    pub fn get(&self, index: u8) -> f32 {
        match index {
            0 => self.experience_point_test_level,
            1 => self.badass_token_test_rank,
            _ => 0.0,
        }
    }

    /// `SetGlobalAttributeValue`: stores slot 0 or 1. What the game does for another index is not read, so it is
    /// `NotYet { NotRead }`, never a guess.
    pub fn set(&mut self, index: u8, value: f32) -> Result<(), AttrError> {
        match index {
            0 => self.experience_point_test_level = value,
            1 => self.badass_token_test_rank = value,
            _ => {
                return Err(NotYet::not_read(
                    format!("SetGlobalAttributeValue for slot {index}"),
                    "NATIVE_ATTRIBUTES.md section 6",
                )
                .into());
            }
        }
        Ok(())
    }
}

/// The numeric forms an evaluation uses where the two C++ oracles disagree (architecture section 5.3). The defaults
/// are the forms behind the item parity (`DoubleThenSingle`, `WeaponGen`); a caller whose oracle is another form
/// passes it. Which form the game uses is UNVERIFIED (real-game questions 21 and 22).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NumForms {
    /// `Multiplier * (Level ^ Power + Offset)`: how the power is taken.
    pub pow: PowForm,
    /// How a stack attribute's value is recomputed from its base and stack.
    pub combine: CombineForm,
}

/// One evaluation's context: the lent world and the numeric forms. Everything else an evaluation reads or keeps
/// (the global slots, the generators) is the world's, so a nested `EvalCx::new(world).with_forms(forms)` is the same
/// context.
///
/// Built from anything that implements [`AttrWorld`]: `EvalCx::new(&mut vm)`.
pub struct EvalCx<'a> {
    pub world: &'a mut dyn AttrWorld,
    pub forms: NumForms,
}

impl<'a> EvalCx<'a> {
    /// The default forms.
    pub fn new(world: &'a mut dyn AttrWorld) -> Self {
        EvalCx {
            world,
            forms: NumForms::default(),
        }
    }

    pub fn with_forms(mut self, forms: NumForms) -> Self {
        self.forms = forms;
        self
    }
}

impl fmt::Debug for EvalCx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EvalCx")
            .field("forms", &self.forms)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Resolvers
// ---------------------------------------------------------------------------------------------------------------

/// A context resolver class's `GetAttributeContext(Attribute, AttributeContextSource)` (NATIVE_ATTRIBUTES.md
/// section 5). Registered in [`AttributeSystem`] by exact class path; an unregistered class is
/// `NotYet { NotRead }`.
pub trait ContextResolver {
    /// The context for `source`, or `Ok(None)` (the chain stops there). `resolver` is the resolver object (its
    /// fields, for example a `Resource`), `attr` the attribute definition being resolved. `source` is `None` only
    /// for the first resolver of a chain whose caller passed no source (`NoContextNeeded` then answers the
    /// globals object). `sys` is the system that called, for resolvers that resolve another definition's chain.
    fn resolve(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError>;
}

/// A value resolver class's virtuals (NATIVE_ATTRIBUTES.md section 6). Registered in [`AttributeSystem`] by exact
/// class path; an unregistered class is `NotYet { NotRead }`.
pub trait ValueResolver {
    /// The value on `context`. `previous` is the result of the chain's earlier resolver (0.0 for the first), which
    /// `SimpleMath` combines with.
    fn value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError>;

    /// The base value. The class default is the same as [`ValueResolver::value`] (section 6).
    fn base_value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        self.value(sys, resolver, context, previous, cx)
    }

    /// Where the number lives, for add, remove and set-base. `Ok(None)` is the class default: every operation
    /// answers false and nothing changes (only `ObjectPropertyAttributeValueResolver` is writable in the record).
    fn storage(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<Storage>, AttrError> {
        let _ = (sys, resolver, context, cx);
        Ok(None)
    }
}

/// Where a writable attribute's number lives (NATIVE_ATTRIBUTES.md section 3).
///
/// `#[non_exhaustive]`: a resolver with its own set-base (`BadassAttributeValueResolver`, not read in detail) would
/// add a variant.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Storage {
    /// A stack attribute: value, base and modifier stack properties on `owner`.
    Stack {
        owner: ObjRef,
        companions: AttrCompanions,
        kind: NumKind,
    },
    /// A plain attribute: one property on `owner`, edited in place; its remove is a no-op that succeeds.
    Plain {
        owner: ObjRef,
        prop: PropId,
        kind: NumKind,
    },
    /// A noun entry (`NounAttributeValueResolver`, NATIVE_NOUN_ATTRIBUTES.md section 5): element `index` of the
    /// `NounAttributeState` array property `state` (`NounState`) on the provider `owner`, which holds its own value,
    /// base and modifier stack. A float, recomputed in single precision; no change notification. Added by lane
    /// FIX-NOUN-OFF (decision 2026-11-16-43). `#[non_exhaustive]`: the replicated slot may need a field once its
    /// mirror is read.
    #[non_exhaustive]
    Noun {
        owner: ObjRef,
        state: PropId,
        index: usize,
    },
}

// ---------------------------------------------------------------------------------------------------------------
// The data the evaluator reads
// ---------------------------------------------------------------------------------------------------------------

/// `AttributeInitializationData`, the 16-byte struct of NATIVE_PROGRESSION.md section 1, with the package's field
/// names.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InitData {
    /// `BaseValueConstant`.
    pub base_value_constant: f32,
    /// `BaseValueAttribute`: an attribute definition, or None.
    pub base_value_attribute: Option<ObjRef>,
    /// `InitializationDefinition`: an `AttributeInitializationDefinition`, or None.
    pub initialization_definition: Option<ObjRef>,
    /// `BaseValueScaleConstant` (the struct default is 1).
    pub base_value_scale_constant: f32,
}

impl InitData {
    /// A plain constant: `value`, no attribute, no definition, scale 1.
    pub fn constant(value: f32) -> Self {
        InitData {
            base_value_constant: value,
            base_value_attribute: None,
            initialization_definition: None,
            base_value_scale_constant: 1.0,
        }
    }
}

/// `AttributeExpressionData.ComparisonOperator` (NATIVE_ATTRIBUTES.md section 11), the package's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Comparison {
    EqualTo = 0,
    NotEqualTo = 1,
    LessThan = 2,
    LessThanOrEqual = 3,
    GreaterThan = 4,
    GreaterThanOrEqual = 5,
}

impl Comparison {
    /// The enum value as the package stores it; `None` for a value outside 0..=5.
    pub fn from_raw(raw: i64) -> Option<Self> {
        Some(match raw {
            0 => Comparison::EqualTo,
            1 => Comparison::NotEqualTo,
            2 => Comparison::LessThan,
            3 => Comparison::LessThanOrEqual,
            4 => Comparison::GreaterThan,
            5 => Comparison::GreaterThanOrEqual,
            _ => return None,
        })
    }
}

/// `AttributeExpressionData.Operand2Usage` (NATIVE_ATTRIBUTES.md section 11), the package's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operand2Usage {
    PreferAttribute = 0,
    Multiply = 1,
}

impl Operand2Usage {
    /// The enum value as the package stores it; `None` for a value outside 0..=1.
    pub fn from_raw(raw: i64) -> Option<Self> {
        match raw {
            0 => Some(Operand2Usage::PreferAttribute),
            1 => Some(Operand2Usage::Multiply),
            _ => None,
        }
    }
}

/// `AttributeExpressionData` (NATIVE_ATTRIBUTES.md section 11), with the package's field names.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExprData {
    /// `AttributeOperand1`: an attribute definition, or None (the expression is then false).
    pub attribute_operand1: Option<ObjRef>,
    /// `ComparisonOperator`.
    pub comparison_operator: Comparison,
    /// `Operand2Usage`.
    pub operand2_usage: Operand2Usage,
    /// `AttributeOperand2`: an attribute definition, or None (the constant is used).
    pub attribute_operand2: Option<ObjRef>,
    /// `ConstantOperand2`.
    pub constant_operand2: f32,
}

/// `EvaluateExpressions`' mode (NATIVE_ATTRIBUTES.md section 11): 0 And, 1 Or; both answer true for an empty list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExprMode {
    And = 0,
    Or = 1,
}

impl ExprMode {
    /// The enum value as the package stores it; `None` for a value outside 0..=1.
    pub fn from_raw(raw: i64) -> Option<Self> {
        match raw {
            0 => Some(ExprMode::And),
            1 => Some(ExprMode::Or),
            _ => None,
        }
    }
}

/// One `AttributeEffectData` entry (NATIVE_ATTRIBUTES.md section 7), the fields `ApplyAttributeEffects` reads.
///
/// `#[non_exhaustive]`: the package struct may carry fields the apply loop does not read; a lane that needs one adds
/// it. Built with [`EffectData::new`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectData {
    /// `AttributeToModify`: an attribute definition, or None (the entry is skipped).
    pub attribute_to_modify: Option<ObjRef>,
    /// `ModifierType`.
    pub modifier_type: ModifierType,
    /// `BaseModifierValue`, evaluated once at apply time.
    pub base_modifier_value: InitData,
}

impl EffectData {
    pub fn new(
        attribute_to_modify: Option<ObjRef>,
        modifier_type: ModifierType,
        base_modifier_value: InitData,
    ) -> Self {
        EffectData {
            attribute_to_modify,
            modifier_type,
            base_modifier_value,
        }
    }
}

/// One `AppliedAttributeEffect` record: the context the modifier landed on, the attribute and the
/// `AttributeModifier` object that was added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppliedEffect {
    pub context: ObjRef,
    pub attribute: ObjRef,
    pub modifier: ObjRef,
}

/// What [`AttributeSystem::apply_effects`] added, in effect order; failed entries leave no record
/// (NATIVE_ATTRIBUTES.md section 7). Handed back to [`AttributeSystem::remove_effects`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied(pub Vec<AppliedEffect>);

/// A resolved attribute read: the context the chain resolved to and the value read on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttrValue {
    pub context: ObjRef,
    pub value: f32,
}

/// The [`ModifierId`] of an `AttributeModifier` object: its arena index and generation, so two live objects never
/// share an id and a freed object's id is not reused while the space lives. The id is what a [`crate::stack::Stack`]
/// dedupes on.
pub fn modifier_id_of(modifier: ObjRef) -> ModifierId {
    ModifierId((u64::from(modifier.generation()) << 32) | u64::from(modifier.index()))
}

// ---------------------------------------------------------------------------------------------------------------
// Outcomes
// ---------------------------------------------------------------------------------------------------------------

// How a chain answers (NATIVE_ATTRIBUTES.md section 1): the value resolvers are asked in order, each only while the
// previous one succeeded, and the chain's outcome is the answer of the LAST resolver asked (the first failure, or the
// last success). Nothing is rolled back: when a later resolver fails, what an earlier writable resolver changed stays
// changed. So a failing outcome says the chain failed, not that no storage changed; implementations must not add a
// rollback. With one resolver (the usual chain) a failing outcome does leave that resolver's storage as it was.

/// Why the chain answered false (NATIVE_ATTRIBUTES.md sections 1 and 3).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// No context was given (the context is None).
    NoContext,
    /// The attribute has no value resolvers.
    NoValueResolvers,
    /// A value resolver entry is None (it counts as a failure).
    NullResolver,
    /// The resolver class is not writable (every class except `ObjectPropertyAttributeValueResolver` and, in Off
    /// mode, `NounAttributeValueResolver`), or a noun resolver found no store on the context.
    NotWritable,
    /// The property is neither a stack nor a plain attribute.
    NotAnAttribute,
}

/// What [`AttributeSystem::add_modifier`] answered: the last resolver asked (see the chain rule above this type; no
/// rollback). [`AddModifierOutcome::succeeded`] is the bool the script sees.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddModifierOutcome {
    /// Stack attribute: appended, value recomputed (true).
    Added,
    /// Stack attribute: that modifier object is already on this stack, which this resolver left unchanged (false).
    Duplicate,
    /// Plain attribute: the stored value was edited in place, permanently (true).
    AppliedInPlace,
    /// The chain failed here (false). Earlier resolvers' changes, if any, stay.
    Refused(Refusal),
}

impl AddModifierOutcome {
    /// The native's return value.
    pub fn succeeded(self) -> bool {
        matches!(
            self,
            AddModifierOutcome::Added | AddModifierOutcome::AppliedInPlace
        )
    }
}

/// What [`AttributeSystem::remove_modifier`] answered: the last resolver asked (see the chain rule above
/// [`Refusal`]; no rollback). [`RemoveModifierOutcome::succeeded`] is the bool the script sees.
///
/// An enum, not a count: a plain attribute's remove answers true and changes nothing (NATIVE_ATTRIBUTES.md
/// section 3), which a count of 0 would report as false.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RemoveModifierOutcome {
    /// Stack attribute: every copy removed (`count >= 1`), value recomputed (true).
    Removed { count: usize },
    /// Stack attribute: the modifier was not on this stack, which this resolver left unchanged (false).
    NotPresent,
    /// Plain attribute: a successful no-op (true).
    PlainNoOp,
    /// The chain failed here (false). Earlier resolvers' changes, if any, stay.
    Refused(Refusal),
}

impl RemoveModifierOutcome {
    /// The native's return value.
    pub fn succeeded(self) -> bool {
        matches!(
            self,
            RemoveModifierOutcome::Removed { .. } | RemoveModifierOutcome::PlainNoOp
        )
    }
}

#[cfg(test)]
mod tests {
    //! A test `AttrWorld` over a synthetic `ObjectSpace` (rule 9: invented classes and values only), checked against
    //! every trait of this contract.

    use std::rc::Rc;

    use ow_core::Missing;
    use ow_core::rand::CrtRand;
    use ow_object::heap::Origin;
    use ow_object::ids::ClassId;

    use super::*;

    /// What the test world's notification saw: the owner, the property, the forms it was handed, and what a nested
    /// context rebuilt from them reads (its forms and global slot 0).
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Seen {
        owner: ObjRef,
        prop: PropId,
        nested_forms: NumForms,
        nested_slot0: f32,
    }

    /// The test world: a hand-filled space, no definition store, one generator, the global slots, a log of
    /// notifications. It holds its `Rc<AttributeSystem>` the way the VM does, so the clone-the-Rc pattern is
    /// exercised. It runs no script.
    struct TestWorld {
        space: ObjectSpace,
        attrs: Rc<AttributeSystem>,
        globals: Option<ObjRef>,
        rng: Option<CrtRand>,
        slots: GlobalSlots,
        changed: Vec<Seen>,
        streams: Vec<CrtStream>,
    }

    impl AttrWorld for TestWorld {
        fn parts(&mut self) -> WorldParts<'_> {
            WorldParts::new(&mut self.space, None)
        }
        fn globals(&mut self) -> Result<ObjRef, AttrError> {
            self.globals.ok_or_else(|| {
                NotYet::layer_absent("the engine globals object", "ow-world", "test world").into()
            })
        }
        fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
            Ok(None)
        }
        fn global_slots(&mut self) -> &mut GlobalSlots {
            &mut self.slots
        }
        fn attribute_changed(
            &mut self,
            owner: ObjRef,
            value_prop: PropId,
            forms: NumForms,
        ) -> Result<(), AttrError> {
            // A nested evaluation, as the VM's script would start one.
            let nested = EvalCx::new(self).with_forms(forms);
            let nested_forms = nested.forms;
            let nested_slot0 = nested.world.global_slots().get(0);
            self.changed.push(Seen {
                owner,
                prop: value_prop,
                nested_forms,
                nested_slot0,
            });
            Ok(())
        }
        fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
            self.streams.push(s);
            match self.rng.as_mut() {
                Some(r) => Ok(r),
                None => {
                    Err(NotYet::layer_absent("a random source", "ow-world", "test world").into())
                }
            }
        }
    }

    /// The `ow_items::RollWorld` shape: everything forwarded to `inner` except `rng`. Forwarding
    /// `attribute_changed` is allowed here only because the inner test world runs no script (the trait's adapter
    /// rule).
    struct OneRng<'a> {
        inner: &'a mut dyn AttrWorld,
        rng: &'a mut CrtRand,
    }

    impl AttrWorld for OneRng<'_> {
        fn parts(&mut self) -> WorldParts<'_> {
            self.inner.parts()
        }
        fn globals(&mut self) -> Result<ObjRef, AttrError> {
            self.inner.globals()
        }
        fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
            self.inner.game_info()
        }
        fn engine_query(
            &mut self,
            q: EngineQuery,
            source: ObjRef,
        ) -> Result<Option<ObjRef>, AttrError> {
            self.inner.engine_query(q, source)
        }
        fn global_slots(&mut self) -> &mut GlobalSlots {
            self.inner.global_slots()
        }
        fn attribute_changed(
            &mut self,
            owner: ObjRef,
            value_prop: PropId,
            forms: NumForms,
        ) -> Result<(), AttrError> {
            self.inner.attribute_changed(owner, value_prop, forms)
        }
        fn rng(&mut self, _s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
            Ok(self.rng)
        }
    }

    /// Answers a fixed number plus the previous one (an invented resolver, not a game class).
    struct AddConstant(f32);

    impl ValueResolver for AddConstant {
        fn value(
            &self,
            _sys: &AttributeSystem,
            _resolver: ObjRef,
            _context: ObjRef,
            previous: f32,
            _cx: &mut EvalCx<'_>,
        ) -> Result<f32, AttrError> {
            Ok(previous + self.0)
        }
    }

    /// Delegates to the resolver object `next` through the system: a resolver that re-enters the registry with the
    /// same `EvalCx`, the shape SimpleMath and ResourcePool need.
    struct Forward {
        next: ObjRef,
    }

    impl ValueResolver for Forward {
        fn value(
            &self,
            sys: &AttributeSystem,
            _resolver: ObjRef,
            context: ObjRef,
            previous: f32,
            cx: &mut EvalCx<'_>,
        ) -> Result<f32, AttrError> {
            let next = sys.value_resolver(cx.world.parts().space, self.next)?;
            next.value(sys, self.next, context, previous, cx)
        }
    }

    /// The source itself, or the world's globals for a None source (the NoContextNeeded shape).
    struct SourceOrGlobals;

    impl ContextResolver for SourceOrGlobals {
        fn resolve(
            &self,
            _sys: &AttributeSystem,
            _resolver: ObjRef,
            _attr: ObjRef,
            source: Option<ObjRef>,
            cx: &mut EvalCx<'_>,
        ) -> Result<Option<ObjRef>, AttrError> {
            match source {
                Some(s) => Ok(Some(s)),
                None => cx.world.globals().map(Some),
            }
        }
    }

    struct Classes {
        object: ClassId,
        known: ClassId,
        sub: ClassId,
        unknown: ClassId,
        ctx: ClassId,
    }

    fn world(attrs: AttributeSystem) -> (TestWorld, Classes) {
        let mut space = ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let define = |space: &mut ObjectSpace, path: &str, sup: ClassId| {
            space.classes.define(path, Some(sup), vec![]).unwrap()
        };
        let known = define(&mut space, "Test.KnownValueResolver", object);
        let sub = define(&mut space, "Test.SubOfKnownValueResolver", known);
        let unknown = define(&mut space, "Test.UnreadValueResolver", object);
        let ctx = define(&mut space, "Test.SourceContextResolver", object);
        let globals = space
            .alloc(object, "Globals", None, Origin::Runtime)
            .unwrap();
        let w = TestWorld {
            space,
            attrs: Rc::new(attrs),
            globals: Some(globals),
            rng: Some(CrtRand::new(7)),
            slots: GlobalSlots::default(),
            changed: Vec::new(),
            streams: Vec::new(),
        };
        let classes = Classes {
            object,
            known,
            sub,
            unknown,
            ctx,
        };
        (w, classes)
    }

    fn obj(w: &mut TestWorld, c: ClassId, name: &str) -> ObjRef {
        w.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn prop0(c: &Classes) -> PropId {
        PropId {
            slot: 0,
            declared_in: c.object,
        }
    }

    fn registered() -> AttributeSystem {
        AttributeSystem::new_empty()
            .with_value_resolver("Test.KnownValueResolver", AddConstant(2.5))
            .unwrap()
            .with_context_resolver("Test.SourceContextResolver", SourceOrGlobals)
            .unwrap()
    }

    fn missing(e: AttrError) -> (Missing, String) {
        match e {
            AttrError::NotYet(n) => (n.missing, n.what.into_owned()),
            other => panic!("expected NotYet, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_resolver_class_is_not_read() {
        let standard = AttributeSystem::standard().unwrap();
        for sys in [AttributeSystem::new_empty(), standard, registered()] {
            let (mut w, c) = world(AttributeSystem::new_empty());
            let r = obj(&mut w, c.unknown, "R");
            let e = sys.value_resolver(&w.space, r).err().unwrap();
            let expected = "value resolver class Test.UnreadValueResolver".to_string();
            assert_eq!(missing(e), (Missing::NotRead, expected));
            let e = sys.context_resolver(&w.space, r).err().unwrap();
            assert_eq!(missing(e).0, Missing::NotRead);
        }
    }

    #[test]
    fn lookup_is_by_exact_class_path_any_case() {
        let (mut w, c) = world(AttributeSystem::new_empty());
        let sys = AttributeSystem::new_empty()
            .with_value_resolver("test:knownvalueresolver", AddConstant(1.0))
            .unwrap();
        let known = obj(&mut w, c.known, "Known");
        assert!(sys.value_resolver(&w.space, known).is_ok());
        // A subclass is not served by its parent's entry: it may override the virtual, and it is not read.
        let sub = obj(&mut w, c.sub, "Sub");
        let e = sys.value_resolver(&w.space, sub).err().unwrap();
        assert_eq!(missing(e).0, Missing::NotRead);
        // A value resolver entry does not answer for the context table.
        let e = sys.context_resolver(&w.space, known).err().unwrap();
        assert_eq!(missing(e).0, Missing::NotRead);
    }

    #[test]
    fn a_class_registered_twice_is_refused() {
        let e = registered()
            .with_value_resolver("TEST.KnownValueResolver", AddConstant(0.0))
            .err()
            .unwrap();
        let class = "TEST.KnownValueResolver".to_string();
        assert_eq!(e, AttrError::DuplicateResolver { class });
        let bad = AttributeSystem::new_empty().with_context_resolver("A..B", SourceOrGlobals);
        assert!(matches!(bad, Err(AttrError::BadData { .. })));
    }

    #[test]
    fn a_stale_resolver_handle_is_a_heap_error() {
        let (mut w, c) = world(AttributeSystem::new_empty());
        let r = obj(&mut w, c.known, "Gone");
        w.space.free(r).unwrap();
        assert!(matches!(
            registered().value_resolver(&w.space, r),
            Err(AttrError::Heap(HeapError::Stale(_)))
        ));
    }

    #[test]
    fn resolvers_run_through_the_lent_world_and_re_enter_the_system() {
        let (mut w, c) = world(registered());
        let known = obj(&mut w, c.known, "Known");
        let ctx_r = obj(&mut w, c.ctx, "Ctx");
        let attr = obj(&mut w, c.object, "Att_Test");
        let target = obj(&mut w, c.object, "Target");
        let globals = w.globals;

        // The clone-the-Rc pattern: no borrow of `w.attrs` is live while `w` is lent.
        let attrs = w.attrs.clone();
        let mut cx = EvalCx::new(&mut w);
        let context = attrs
            .context_resolver(cx.world.parts().space, ctx_r)
            .unwrap();
        let got = context.resolve(&attrs, ctx_r, attr, Some(target), &mut cx);
        assert_eq!(got, Ok(Some(target)));
        let got = context.resolve(&attrs, ctx_r, attr, None, &mut cx);
        assert_eq!(got, Ok(globals));

        let value = attrs.value_resolver(cx.world.parts().space, known).unwrap();
        assert_eq!(value.value(&attrs, known, target, 1.0, &mut cx), Ok(3.5));
        // The class defaults: the base is the value, and storage is "not writable".
        assert_eq!(
            value.base_value(&attrs, known, target, 0.0, &mut cx),
            Ok(2.5)
        );
        assert_eq!(value.storage(&attrs, known, target, &mut cx), Ok(None));

        // A resolver that re-enters the registry with the same context.
        let fwd = Forward { next: known };
        assert_eq!(fwd.value(&attrs, known, target, 0.5, &mut cx), Ok(3.0));
    }

    #[test]
    fn a_world_without_globals_or_rng_says_so() {
        let (mut w, c) = world(registered());
        let ctx_r = obj(&mut w, c.ctx, "Ctx");
        let attr = obj(&mut w, c.object, "Att");
        w.globals = None;
        w.rng = None;
        let attrs = w.attrs.clone();
        let mut cx = EvalCx::new(&mut w);
        let context = attrs
            .context_resolver(cx.world.parts().space, ctx_r)
            .unwrap();
        // NoContextNeeded never answers None; a world without globals is an error, not a silent None.
        let e = context
            .resolve(&attrs, ctx_r, attr, None, &mut cx)
            .err()
            .unwrap();
        assert_eq!(missing(e).0, Missing::LayerAbsent("ow-world"));
        let e = cx.world.rng(CrtStream::AttributeVariance).err().unwrap();
        assert_eq!(missing(e).0, Missing::LayerAbsent("ow-world"));
    }

    #[test]
    fn a_world_without_a_store_says_which_layer_is_absent() {
        let (mut w, _) = world(AttributeSystem::new_empty());
        let e = w.parts().defs().err().unwrap();
        assert_eq!(missing(e).0, Missing::LayerAbsent("ow-defs"));
        let mut space = ObjectSpace::new();
        let mut defs = DefStore::new_empty();
        let mut parts = WorldParts::new(&mut space, Some(&mut defs));
        assert!(parts.defs().is_ok());
        // Both halves at once: one split borrow.
        let WorldParts { space, defs, .. } = parts;
        assert!(space.heap.is_empty() && defs.is_some());
    }

    #[test]
    fn engine_queries_default_to_the_absent_world_layer() {
        let (mut w, c) = world(AttributeSystem::new_empty());
        let s = obj(&mut w, c.object, "Source");
        let inventory = EngineQuery::Inventory {
            definition: s,
            equipped_only: true,
        };
        for q in [
            EngineQuery::ResourcePool { resource: s },
            EngineQuery::EquippedInventory { location: 1 },
            inventory,
            EngineQuery::PlayerController,
            EngineQuery::AttributeContextSource,
            EngineQuery::PlayerSkill { definition: s },
        ] {
            let e = w.engine_query(q, s).err().unwrap();
            assert_eq!(missing(e).0, Missing::LayerAbsent("ow-world"));
        }
    }

    #[test]
    fn a_nested_evaluation_sees_the_same_slots_and_forms() {
        let (mut w, c) = world(AttributeSystem::new_empty());
        let owner = obj(&mut w, c.object, "Pool");
        let p = prop0(&c);
        let forms = NumForms {
            pow: PowForm::Single,
            combine: CombineForm::Vm5F,
        };
        {
            let cx = EvalCx::new(&mut w).with_forms(forms);
            cx.world.global_slots().set(0, 12.0).unwrap();
            cx.world.attribute_changed(owner, p, cx.forms).unwrap();
        }
        let seen = Seen {
            owner,
            prop: p,
            nested_forms: forms,
            nested_slot0: 12.0,
        };
        assert_eq!(w.changed, vec![seen]);
        let mut same = CrtRand::new(7);
        let first = w.rng(CrtStream::AttributeVariance).unwrap().rand15();
        assert_eq!(first, same.rand15());
        assert_eq!(w.streams, vec![CrtStream::AttributeVariance]);
    }

    #[test]
    fn an_adapter_forwards_everything_but_rng() {
        let (mut w, c) = world(AttributeSystem::new_empty());
        let owner = obj(&mut w, c.object, "Owner");
        let globals = w.globals.unwrap();
        let p = prop0(&c);
        let mut roll = CrtRand::new(99);
        let mut expected = CrtRand::new(99);
        {
            let mut adapter = OneRng {
                inner: &mut w,
                rng: &mut roll,
            };
            let cx = EvalCx::new(&mut adapter);
            assert_eq!(cx.world.globals(), Ok(globals));
            assert_eq!(cx.world.game_info(), Ok(None));
            assert!(cx.world.parts().space.heap.contains(owner));
            cx.world.global_slots().set(1, 2.0).unwrap();
            for s in [CrtStream::AttributeVariance, CrtStream::ItemRoll] {
                assert_eq!(cx.world.rng(s).unwrap().rand15(), expected.rand15());
            }
            cx.world.attribute_changed(owner, p, cx.forms).unwrap();
        }
        // The inner world's generator was never drawn from; its slots and notification log were written.
        assert!(w.streams.is_empty());
        assert_eq!(w.slots.get(1), 2.0);
        assert_eq!(w.changed.len(), 1);
    }

    #[test]
    fn outcomes_map_to_the_natives_bool() {
        use AddModifierOutcome as A;
        use RemoveModifierOutcome as R;
        assert!(A::Added.succeeded());
        assert!(A::AppliedInPlace.succeeded());
        assert!(!A::Duplicate.succeeded());
        assert!(!A::Refused(Refusal::NoContext).succeeded());
        assert!(R::Removed { count: 2 }.succeeded());
        // A plain attribute's remove answers true and changes nothing (NATIVE_ATTRIBUTES.md section 3).
        assert!(R::PlainNoOp.succeeded());
        assert!(!R::NotPresent.succeeded());
        assert!(!R::Refused(Refusal::NotWritable).succeeded());
    }

    #[test]
    fn global_slots_hold_two_values() {
        let mut g = GlobalSlots::default();
        g.set(0, 12.0).unwrap();
        g.set(1, 3.0).unwrap();
        assert_eq!(
            (g.get(0), g.get(1), g.get(2), g.get(255)),
            (12.0, 3.0, 0.0, 0.0)
        );
        assert_eq!(missing(g.set(2, 1.0).err().unwrap()).0, Missing::NotRead);
        let (mut w, _) = world(AttributeSystem::new_empty());
        let cx = EvalCx::new(&mut w);
        let defaults = NumForms::default();
        assert_eq!(cx.forms, defaults);
        assert_eq!(
            (defaults.pow, defaults.combine),
            (PowForm::default(), CombineForm::default())
        );
    }

    #[test]
    fn modifier_ids_follow_object_identity() {
        let (mut w, c) = world(AttributeSystem::new_empty());
        let a = obj(&mut w, c.object, "A");
        let b = obj(&mut w, c.object, "B");
        assert_ne!(modifier_id_of(a), modifier_id_of(b));
        assert_eq!(modifier_id_of(a), modifier_id_of(a));
        w.space.free(a).unwrap();
        let again = obj(&mut w, c.object, "A2");
        // The freed slot may be reused, but the generation differs, so the id does too.
        assert_ne!(modifier_id_of(again), modifier_id_of(a));
    }

    #[test]
    fn errors_are_clone_partial_eq_and_display() {
        let e: AttrError =
            NotYet::not_read("resolver class X", "NATIVE_ATTRIBUTES.md section 6").into();
        assert_eq!(e.clone(), e);
        assert!(e.to_string().contains("resolver class X"));
        let w = AttrError::World("script failed".into());
        assert_ne!(w, e);
        assert_eq!(w.to_string(), "world: script failed");
    }

    #[test]
    fn package_enums_decode_their_values() {
        assert_eq!(
            Comparison::from_raw(5),
            Some(Comparison::GreaterThanOrEqual)
        );
        assert_eq!(Comparison::from_raw(6), None);
        assert_eq!(Operand2Usage::from_raw(1), Some(Operand2Usage::Multiply));
        assert_eq!(Operand2Usage::from_raw(-1), None);
        assert_eq!(ExprMode::from_raw(0), Some(ExprMode::And));
        assert_eq!(ExprMode::from_raw(2), None);
        let d = InitData::constant(4.0);
        assert_eq!(
            (d.base_value_scale_constant, d.base_value_attribute),
            (1.0, None)
        );
    }

    #[test]
    fn definition_store_errors_keep_not_yet_and_heap() {
        let n = NotYet::not_ported("DefStore::find", "r");
        assert_eq!(
            AttrError::from(DefError::NotYet(n.clone())),
            AttrError::NotYet(n)
        );
        let heap = HeapError::InstanceStandIn(Origin::Runtime);
        assert_eq!(
            AttrError::from(DefError::Heap(heap.clone())),
            AttrError::Heap(heap)
        );
        let d = DefError::NotADefinition("Core.Object".into());
        assert_eq!(AttrError::from(d.clone()), AttrError::Def(d));
    }
}
