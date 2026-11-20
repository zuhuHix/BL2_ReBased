//! Owner: lane IMPL-FLAG-EXPRESSION-EVALUATE. `FlagExpressionEvaluator.Evaluate(object)` in `ParityMode::Off`, coded
//! from docs/verification/NATIVE_FLAG_EXPRESSION_EVALUATE.md alone (PR #527 at f28a8d1d; clean room: lane
//! ANALYST-FLAG-EXPRESSION-EVALUATE read the native, this lane did not). AI-assisted (Claude Code). **Every rule here is
//! UNVERIFIED in the running game.** Reference binds nothing (its stub policy stands).
//!
//! **The rule** ([`evaluate`], note sections 3, 4 and 6 at the accepted head e2724bc9): the receiver and the
//! `FlagChain` shape are checked up front; then each conditional when the chain reaches it, so a short-circuited branch
//! is never checked:
//! 1. a None `FlagDefinition` answers false, before the type check;
//! 2. the timed tests (types 2 and 3) are `NotYet { NotRead }`;
//! 3. the context's rule engine `R` (reads only, worked out once, before the first definition is loaded: the note's
//!    order, so a refusal there is not marked because of the definition's load); with `R`, the store path (the definition lacking a context or a value
//!    resolver) is `NotYet { NotRead }`, "a rule engine's own flag store";
//! 4. the definition path (section 4.0): a None context answers false; the context passes through
//!    `ContextResolverChain` (None entries skipped, a resolver answering None ends it and the context so far stays);
//!    a computed flag is evaluated on the resolved context; else the value resolver answers through the mind's flag
//!    store; with neither, `IsTrue` is false and `IsFalse` is `NotYet { NotRead }`.
//!
//! The chain combines by `FlagChainOperator` (`CEOP_Or` 0, `CEOP_And` 1, any other value false), with the early exits.
//!
//! **Loads are writes** (section 6): a reached handle (the receiver, a definition, its expression, its resolvers, the
//! context resolvers' answers) is loaded when first used, through ow-attr's `ensure_loaded` (`LayerAbsent` with no
//! definition store, `NotRead` for a stand-in). The rule engine search alone reads without loading (an unloaded
//! object there is `NotRead`).
//!
//! **The interface classes** (lane FIX-FLAG-IMPLEMENTS-NOTYET, review r1 of #586): the rule asks objects for
//! `IRuleEngineOwner` and `IFlagProvider` by their class tables, which only hold an interface class once it is
//! loaded. An interface class that is not loaded is `NotYet { NotRead }`, "the interface class <path> is not loaded (a
//! load is a write)", never a "no"; a loaded class that does not implement one still answers no. Each is required
//! only where the remaining path may still ask for it, and before the next load, write or draw:
//! - `IRuleEngineOwner` where it is asked, in the rule engine search (step 3), which reads only and comes before the
//!   conditional's first load;
//! - `IFlagProvider` before a reached definition is loaded, when the definition may still need a store: it is a
//!   handle not loaded yet (its fields are unknown), or it has a `ValueResolver` (both read resolvers ask a store) or
//!   an `EvaluationExpression` (a nested evaluator may ask either interface; the port's conservative choice);
//! - both before a receiver handle is loaded, since its chain is unknown until then;
//! - `IFlagProvider` before the chain's first side effect (`provider_first`, lane FIX-FLAG-PROVIDER-ORDER and review r1
//!   of #630): the chain is read as the loop will reach it, following early exits on answers proven with nothing run,
//!   up to the first conditional whose answer is not proven. That one is required to have the interface when it may
//!   need a store, and also when it runs a context resolver, since a resolver may set a `ValueResolver` that would
//!   then ask for a store after its writes (refused up front with [`MUTABLE_PATH`]).
//!
//! With no context no interface question is reached (step 4's None context answers false, and `R` is None), so
//! nothing is required. An empty chain, a short-circuited branch, a None definition and a definition with neither
//! field answer as before.
//!
//! **Also `NotYet { NotRead }`:** a value resolver class other than the two read; a computed flag that also has a value
//! resolver (the hand-off); a flag store that is not a `GearboxMind`'s; a store whose two arrays are out of step. Two
//! are the port's own choices, not questions of the note: a rule engine owner other than a `GearboxMind` (only the
//! mind's `MyRuleEngine` is read), and a fractional function-call result (truncating and rounding disagree between 0.5
//! and 1, so only an integral result is taken).
//!
//! **Writes** (note section 4.1): a lookup that adds a definition to the mind's store, a function call's flag update,
//! a timed flag's expiry, and the loads. They are counted by #518's canonical counters (heap writes, generator draws,
//! definition-store load transitions), snapshot when the native is entered, so writes by callees (a context resolver,
//! a script `Evaluate` through `vm.call`) count too. A `NotYet` is marked "stopped after partial writes: " exactly when
//! a counter moved, its kind and record kept; other errors are returned as they are.

use std::rc::Rc;

use ow_attr::api::EvalCx;
use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The function path the rule is bound under.
pub const EVALUATE: &str = "GearboxFramework.FlagExpressionEvaluator.Evaluate";
/// The record of the rule.
pub const RECORD: &str = "NATIVE_FLAG_EXPRESSION_EVALUATE.md";
const UNIT: &str = "FlagExpressionEvaluator.Evaluate";

const EVALUATOR: &str = "GearboxFramework.FlagExpressionEvaluator";
const MIND: &str = "GearboxFramework.GearboxMind";
const RULE_ENGINE: &str = "GearboxFramework.RuleEngine";
const RULE: &str = "GearboxFramework.Rule";
const GEARBOX_PAWN: &str = "GearboxFramework.GearboxPawn";
const RULE_ENGINE_OWNER: &str = "GearboxFramework.IRuleEngineOwner";
const FLAG_PROVIDER: &str = "GearboxFramework.IFlagProvider";
const RULE_ENGINE_RESOLVER: &str = "GearboxFramework.RuleEngineFlagValueResolver";
const FUNCTION_RESOLVER: &str = "GearboxFramework.ObjectFunctionFlagValueResolver";
/// The context resolver whose script asks its source for [`AI_INTERFACE`] before anything else (read with
/// `research/script_disasm.py`; lane PREFLIGHT-ORDER-LINK9-CAST).
pub const AI_COMPONENT_RESOLVER: &str = "GearboxFramework.AIComponentAttributeContextResolver";
/// The interface [`AI_COMPONENT_RESOLVER`] asks for.
pub const AI_INTERFACE: &str = "GearboxFramework.IAIInterface";

/// Binds the rule in Off, first-wins; nothing under Reference.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    ow_combat::hit::bind(
        vm,
        EVALUATE,
        Rc::new(|c: &mut NativeCall<'_>| {
            let me = c.this;
            let context = match c.arg_count() {
                0 => None,
                _ => match c.arg(0)? {
                    Value::Object(o) => *o,
                    other => return Err(script(format!("ContextObject holds {}, not an object reference", other.kind_name()))),
                },
            };
            let start = WriteCounters::of(c.vm);
            let answer = evaluate(c.vm, me, context);
            answer.map(Value::Bool).map_err(|e| start.mark(c.vm, e))
        }),
    )
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{UNIT}: {message}") }
}

fn not_read(what: String) -> VmError {
    NotYet::not_read(format!("{UNIT}: {what}"), RECORD).into()
}

/// The canonical write counters (#518's tracker): heap writes, generator draws and definition-store load
/// transitions. Each only grows, so a refusal is after a write exactly when one moved since the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteCounters {
    heap: u64,
    draws: u64,
    loads: u64,
}

impl WriteCounters {
    /// The counters now.
    pub fn of(vm: &Vm) -> Self {
        WriteCounters { heap: vm.space.heap.committed_writes(), draws: vm.rng.draws(), loads: vm.defs.load_transitions() }
    }

    /// Whether a counter moved since `self`.
    pub fn moved(&self, vm: &Vm) -> bool {
        WriteCounters::of(vm) != *self
    }

    /// `e`, marked "stopped after partial writes: " when it is a `NotYet` and a counter moved since `self`.
    pub fn mark(&self, vm: &Vm, e: VmError) -> VmError {
        match e {
            VmError::NotYet(n) if self.moved(vm) => n.after_partial_writes().into(),
            other => other,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Reads (non-loading)
// ---------------------------------------------------------------------------------------------------------------

fn path_of(vm: &Vm, o: ObjRef) -> String {
    ow_attr::eval::class_path(&vm.space, o).to_string()
}

fn object_name(vm: &Vm, o: ObjRef) -> String {
    match ow_vm::path_name::path_name(vm, o) {
        Ok(p) => p,
        Err(_) => vm.space.heap.get(o).map(|x| vm.space.names.text(x.name).into_owned()).unwrap_or_default(),
    }
}

fn is_a(vm: &Vm, o: ObjRef, path: &str) -> Result<bool, VmError> {
    let class = vm.space.heap.get(o)?.class;
    Ok(vm.space.classes.by_path(path).is_some_and(|b| class == b || vm.space.classes.is_child_of(class, b)))
}

/// Whether `o`'s class declares (or inherits) the interface at `path`, from the class tables alone. An interface class
/// that is not loaded is `NotRead` ([`interface`]), not a "no": [`chain_of`] checks that before any write.
fn implements(vm: &mut Vm, o: ObjRef, path: &str) -> Result<bool, VmError> {
    let iface = interface(vm, path)?;
    let class = vm.space.heap.get(o)?.class;
    vm.implements_checked(class, iface)
}

/// [`implements`] reading only interface-table entries already resolved (lane FIX-FLAG-PROVIDER-ORDER, finding 2): the
/// rule engine search reads only, and resolving a cold entry loads its target class. An entry not resolved yet, when
/// no resolved one answers yes, is `NotYet { NotRead }`, and the entry stays cold.
fn implements_read_only(vm: &Vm, o: ObjRef, path: &str) -> Result<bool, VmError> {
    let iface = interface(vm, path)?;
    let class = vm.space.heap.get(o)?.class;
    match vm.implements_read_only(class, iface)? {
        Some(answer) => Ok(answer),
        None => Err(not_read(format!(
            "the interface table of {} has an entry not resolved (resolving it is a load), asked for {path}",
            ow_attr::eval::class_path(&vm.space, o)
        ))),
    }
}

/// Refuses an object the store has not loaded, where the note reads by reads only (the rule engine's search).
fn loaded(vm: &Vm, o: ObjRef, what: &str) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        return Err(not_read(format!("the unloaded {what} {}", object_name(vm, o))));
    }
    Ok(())
}

/// Loads a reached handle when it is first used (note section 6, "Loads are writes"): through ow-attr's
/// `ensure_loaded`, so a world with no definition store is `LayerAbsent` and a handle the store leaves a stand-in is
/// `NotRead`. The store's load-transition counter records the load for the mark (a retry of a failed handle changes
/// nothing, so it is not counted).
fn load(vm: &mut Vm, o: ObjRef) -> Result<(), VmError> {
    ow_attr::eval::ensure_loaded(&mut EvalCx::new(vm), o).map_err(VmError::from)
}

fn field(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let Some(p) = vm.space.prop(class, name) else {
        return Err(script(format!("{} declares no {name}", path_of(vm, o))));
    };
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn object_field(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    match field(vm, o, name)? {
        Value::Object(r) | Value::Interface(r) => Ok(r),
        other => Err(script(format!("{}.{name} holds {}, not an object reference", path_of(vm, o), other.kind_name()))),
    }
}

/// Field `name` of a struct value.
fn struct_field<'a>(vm: &Vm, v: &'a Value, name: &str) -> Result<&'a Value, VmError> {
    let s = v.as_struct().ok_or_else(|| script(format!("{name}: not in a struct value")))?;
    let id = vm.space.names.find(name).ok_or_else(|| script(format!("no field {name}")))?;
    let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| script(format!("{name}: an unknown struct")))?;
    let (entry, _) = layout.field(id).ok_or_else(|| script(format!("the struct declares no {name}")))?;
    s.fields.get(entry as usize).ok_or_else(|| script(format!("{name}: past the struct's fields")))
}

fn struct_field_mut<'a>(vm: &Vm, v: &'a mut Value, name: &str) -> Result<&'a mut Value, VmError> {
    let ty = v.as_struct().ok_or_else(|| script(format!("{name}: not in a struct value")))?.ty;
    let id = vm.space.names.find(name).ok_or_else(|| script(format!("no field {name}")))?;
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| script(format!("{name}: an unknown struct")))?;
    let (entry, _) = layout.field(id).ok_or_else(|| script(format!("the struct declares no {name}")))?;
    let s = v.struct_mut().expect("checked above");
    s.fields.get_mut(entry as usize).ok_or_else(|| script(format!("{name}: past the struct's fields")))
}

fn byte_of(v: &Value, what: &str) -> Result<u8, VmError> {
    match v {
        Value::Byte(b) => Ok(*b),
        other => Err(script(format!("{what} holds {}, not a byte", other.kind_name()))),
    }
}

fn float_of(v: &Value, what: &str) -> Result<f32, VmError> {
    match v {
        Value::Float(f) => Ok(*f),
        other => Err(script(format!("{what} holds {}, not a float", other.kind_name()))),
    }
}

fn object_of(v: &Value, what: &str) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) | Value::Interface(o) => Ok(*o),
        other => Err(script(format!("{what} holds {}, not an object reference", other.kind_name()))),
    }
}

/// The world's time, `WorldInfo.TimeSeconds`.
fn now(vm: &Vm) -> Result<f32, VmError> {
    let actors = ow_world::actors::actors_of(vm)?;
    let info = actors.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info();
    let Some(info) = info else {
        return Err(NotYet::layer_absent(format!("{UNIT}: the world's time"), "the map load (WorldInfo)", RECORD).into());
    };
    float_of(&field(vm, info, "TimeSeconds")?, "WorldInfo.TimeSeconds")
}

// ---------------------------------------------------------------------------------------------------------------
// The evaluator
// ---------------------------------------------------------------------------------------------------------------

/// One conditional of a chain.
struct Conditional {
    kind: u8,
    definition: Option<ObjRef>,
}

/// The receiver's chain and operator: the note's eager checks (section 6), the receiver loaded when it is a handle.
/// With a context, a receiver handle's load needs both interface classes first (module documentation), and the rule
/// engine search must be able to answer by reads alone (review r2 of #630, item 2): its refusal, an interface-table
/// entry not resolved for instance, comes before the load, since the receiver's chain is unknown until then.
fn chain_of(vm: &mut Vm, me: Option<ObjRef>, context: Option<ObjRef>) -> Result<(ObjRef, Vec<Conditional>, u8), VmError> {
    let me = me.ok_or_else(|| script("called with no evaluator".to_string()))?;
    if !is_a(vm, me, EVALUATOR)? {
        return Err(script(format!("{} is not a FlagExpressionEvaluator", path_of(vm, me))));
    }
    if context.is_some() && vm.defs.is_unloaded(&vm.space, me) {
        for path in [RULE_ENGINE_OWNER, FLAG_PROVIDER] {
            interface(vm, path)?;
        }
        rule_engine(vm, context)?;
        // The chain is unknown until the load: any definition may reach the AI component resolver.
        if let Some(c) = context {
            ai_question(vm, c)?;
        }
    }
    load(vm, me)?;
    let chain = field(vm, me, "FlagChain")?;
    let items = chain.elements().ok_or_else(|| script("FlagChain is not an array".to_string()))?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(Conditional {
            kind: byte_of(struct_field(vm, item, "FlagEvalType")?, "FlagEvalType")?,
            definition: object_of(struct_field(vm, item, "FlagDefinition")?, "FlagDefinition")?,
        });
    }
    let operator = byte_of(&field(vm, me, "FlagChainOperator")?, "FlagChainOperator")?;
    Ok((me, out, operator))
}

/// The context's rule engine `R` (note section 3, step 1), by reads only (nothing resolved or loaded: the interface
/// question reads resolved entries only, finding 2); a None context gives none.
fn rule_engine(vm: &Vm, context: Option<ObjRef>) -> Result<Option<ObjRef>, VmError> {
    let Some(c) = context else {
        return Ok(None);
    };
    let owner_engine = |vm: &Vm, o: ObjRef| -> Result<Option<Option<ObjRef>>, VmError> {
        if !implements_read_only(vm, o, RULE_ENGINE_OWNER)? {
            return Ok(None);
        }
        loaded(vm, o, "rule engine owner")?;
        if !is_a(vm, o, MIND)? {
            // The port's own choice: only a mind's `MyRuleEngine` is read.
            return Err(not_read(format!("the rule engine of a {} (only GearboxMind's is read)", path_of(vm, o))));
        }
        Ok(Some(object_field(vm, o, "MyRuleEngine")?))
    };
    if let Some(engine) = owner_engine(vm, c)? {
        if engine.is_some() {
            return Ok(engine);
        }
    } else if is_a(vm, c, GEARBOX_PAWN)? {
        loaded(vm, c, "pawn")?;
        let controller = match object_field(vm, c, "Controller")? {
            Some(controller) => Some(controller),
            None => match object_field(vm, c, "DrivenVehicle")? {
                Some(vehicle) => {
                    loaded(vm, vehicle, "vehicle")?;
                    object_field(vm, vehicle, "Controller")?
                }
                None => None,
            },
        };
        if let Some(controller) = controller
            && let Some(engine) = owner_engine(vm, controller)?
            && engine.is_some()
        {
            return Ok(engine);
        }
    }
    if is_a(vm, c, RULE_ENGINE)? {
        return Ok(Some(c));
    }
    if is_a(vm, c, RULE)? {
        loaded(vm, c, "rule")?;
        return object_field(vm, c, "MyRuleEngine");
    }
    Ok(None)
}

/// `FlagExpressionEvaluator.Evaluate(context)` on `me` (module documentation); the caller marks a `NotYet` after a
/// write ([`WriteCounters::mark`]). Only the
/// receiver and the chain's shape are checked up front; each conditional is checked when it is reached (note section
/// 6, "Per conditional").
pub fn evaluate(vm: &mut Vm, me: Option<ObjRef>, context: Option<ObjRef>) -> Result<bool, VmError> {
    let (_, chain, operator) = chain_of(vm, me, context)?;
    let mut engine: Option<Option<ObjRef>> = None;
    provider_first(vm, &chain, context, &mut engine, operator)?;
    let mut result = true;
    for (i, c) in chain.iter().enumerate() {
        let answer = match c.definition {
            // Step 1: a None definition answers false, before the type check.
            None => false,
            Some(definition) => {
                // Step 2: the type.
                if c.kind == 2 || c.kind == 3 {
                    return Err(not_read(
                        "the timed flag tests (HasBeenTrueForXSeconds, HasBeenFalseForXSeconds)".to_string(),
                    ));
                }
                if c.kind > 3 {
                    return Err(script(format!("FlagEvalType {} is not a flag test", c.kind)));
                }
                // Step 3: the context's rule engine, by reads only, before the definition is loaded (the note's
                // order), so a refusal there is not marked because of the definition's load.
                if engine.is_none() {
                    engine = Some(rule_engine(vm, context)?);
                }
                // Finding 3: a loaded definition that takes the store path reports that before the provider guard.
                if !vm.defs.is_unloaded(&vm.space, definition) && takes_store_path(vm, definition, engine.flatten())? {
                    return Err(not_read("a rule engine's own flag store".to_string()));
                }
                if context.is_some() && may_need_store(vm, definition)? {
                    interface(vm, FLAG_PROVIDER)?;
                }
                if let Some(c) = context
                    && may_ask_ai_first(vm, definition)?
                {
                    ai_question(vm, c)?;
                }
                load(vm, definition)?;
                if takes_store_path(vm, definition, engine.flatten())? {
                    return Err(not_read("a rule engine's own flag store".to_string()));
                }
                definition_answer(vm, definition, context, c.kind == 0)?
            }
        };
        result = if i == 0 {
            answer
        } else {
            match operator {
                0 => result || answer,
                1 => result && answer,
                _ => false,
            }
        };
        if (operator == 0 && result) || (operator == 1 && !result) {
            return Ok(result);
        }
    }
    Ok(result)
}

/// The `IFlagProvider` guard ahead of the chain's first side effect (lane FIX-FLAG-PROVIDER-ORDER, finding 1, and
/// review r1 of #630). A conditional's context resolvers and loads are writes, so a missing-interface refusal met after
/// them would come after writes. This reads the chain in order, as the loop will reach it, by reads only:
/// - while every answer so far is **proven** without running anything, the loop's early exits are followed, so a
///   conditional the chain cannot reach asks for nothing. Proven answers: a None definition (false), and `IsTrue` on a
///   loaded definition off the store path with no `ValueResolver`, no `EvaluationExpression` and no context resolver
///   (false, with nothing run: review r1 of #630, item 1);
/// - the first conditional whose answer is not proven decides, and the scan ends there. A refusal of its own with no
///   side effect (a timed test, a type past the tests, a loaded definition on the store path, `IsFalse` with neither
///   field and no context resolver) is left to the loop. Otherwise it is certainly reached and it writes before it
///   answers (a load, a store lookup, a nested evaluation or a context resolver), so `IFlagProvider` is required now:
///   - when it may still need a store itself ([`may_need_store`]);
///   - when it runs a context resolver, one bound by the attribute system or not loaded yet (a resident unbound one
///     is the loop's own refusal, with nothing run: review r2 of #630, item 1), (review r1 of #630, item 2): a
///     resolver may set any definition's
///     `ValueResolver` or `EvaluationExpression`, its own or a later one's, so which stores the rest of the chain asks
///     for is not known before that write. With `IFlagProvider` not loaded this is `NotYet { NotRead }` before the
///     first write ([`MUTABLE_PATH`]), not a guarantee made at set-up.
///
///   Every later conditional is reached, if at all, only after those writes, and by then `IFlagProvider` is known to
///   be loaded, so its per-conditional check cannot refuse on the interface.
///
/// The rule engine `R` this needs is worked out here (reads only) and kept for the loop. A field this cannot read
/// leniently is left to the loop, which reports it when the conditional is reached.
fn provider_first(
    vm: &Vm,
    chain: &[Conditional],
    context: Option<ObjRef>,
    engine: &mut Option<Option<ObjRef>>,
    operator: u8,
) -> Result<(), VmError> {
    if context.is_none() {
        return Ok(());
    }
    let mut result = true;
    for (i, c) in chain.iter().enumerate() {
        let answer = match c.definition {
            None => false,
            Some(definition) => {
                if c.kind > 1 {
                    return Ok(());
                }
                if engine.is_none() {
                    *engine = Some(rule_engine(vm, context)?);
                }
                let resident = !vm.defs.is_unloaded(&vm.space, definition);
                if resident && takes_store_path(vm, definition, engine.flatten()).unwrap_or(false) {
                    return Ok(());
                }
                if may_need_store(vm, definition)? {
                    interface(vm, FLAG_PROVIDER)?;
                    return Ok(());
                }
                // Loaded, with neither field: its context resolvers decide whether anything runs first.
                let Ok(first) = first_resolver(vm, definition) else {
                    return Ok(());
                };
                if let Some(resolver) = first {
                    // A resident resolver whose class the attribute system does not bind is the loop's own refusal,
                    // met with nothing run or written (review r2 of #630, item 1).
                    let resident = !vm.defs.is_unloaded(&vm.space, resolver) && !vm.defs.is_failed(&vm.space, resolver);
                    if resident && vm.attrs.context_resolver(&vm.space, resolver).is_err() {
                        return Ok(());
                    }
                    if vm.space.classes.by_path(FLAG_PROVIDER).is_none() {
                        return Err(not_read(MUTABLE_PATH.to_string()));
                    }
                    return Ok(());
                }
                if c.kind == 1 {
                    // IsFalse with neither field: the loop's own refusal, with nothing run.
                    return Ok(());
                }
                false
            }
        };
        result = if i == 0 {
            answer
        } else {
            match operator {
                0 => result || answer,
                1 => result && answer,
                _ => false,
            }
        };
        if (operator == 0 && result) || (operator == 1 && !result) {
            return Ok(());
        }
    }
    Ok(())
}

/// The refusal of [`provider_first`] for a reached context resolver while `IFlagProvider` is not loaded (review r1 of
/// #630, item 2).
pub const MUTABLE_PATH: &str = "a context resolver runs before the chain's store needs are known (it may set a \
     ValueResolver or EvaluationExpression), and the interface class GearboxFramework.IFlagProvider is not loaded (a \
     load is a write)";

/// The first resolver [`resolve`] reaches in a definition's `ContextResolverChain` (its first non-None entry, decoded
/// as `resolve` decodes it), by reads only. A malformed chain or entry is `resolve`'s own error.
fn first_resolver(vm: &Vm, definition: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let chain = field(vm, definition, "ContextResolverChain")?;
    let entries = chain.elements().ok_or_else(|| script("ContextResolverChain is not an array".to_string()))?;
    for entry in entries {
        if let Some(resolver) = object_of(entry, "ContextResolverChain entry")? {
            return Ok(Some(resolver));
        }
    }
    Ok(None)
}

/// Whether a reached definition may still ask for `IFlagProvider` (module documentation), by reads only: a handle not
/// loaded yet, or a set `ValueResolver` or `EvaluationExpression`. A field the class does not declare, or one that
/// holds no object reference, counts as "may": the reads after the load report it.
fn may_need_store(vm: &Vm, definition: ObjRef) -> Result<bool, VmError> {
    if vm.defs.is_unloaded(&vm.space, definition) {
        return Ok(true);
    }
    let class = vm.space.heap.get(definition)?.class;
    for name in ["ValueResolver", "EvaluationExpression"] {
        let Some(p) = vm.space.prop(class, name) else {
            return Ok(true);
        };
        match vm.space.read(definition, PropAddr::scalar(p))? {
            Value::Object(None) | Value::Interface(None) => {}
            _ => return Ok(true),
        }
    }
    Ok(false)
}

/// Whether a reached definition may ask its context for [`AI_INTERFACE`] before anything changes that context (lane
/// PREFLIGHT-ORDER-LINK9-CAST), by reads only: a handle not loaded yet (its fields are unknown), or a chain whose first
/// set entry is an [`AI_COMPONENT_RESOLVER`] (its script asks its source, the context, first). A later entry asks a
/// context the chain's earlier resolvers have changed, which cannot be known before they run, and a nested evaluator
/// checks its own definitions: the dispatch decides there.
fn may_ask_ai_first(vm: &Vm, definition: ObjRef) -> Result<bool, VmError> {
    if vm.defs.is_unloaded(&vm.space, definition) {
        return Ok(true);
    }
    let class = vm.space.heap.get(definition)?.class;
    let Some(p) = vm.space.prop(class, "ContextResolverChain") else {
        // Undeclared: the reads after the load report it.
        return Ok(false);
    };
    let first = vm.space.read(definition, PropAddr::scalar(p))?.elements().and_then(|entries| {
        entries.iter().find_map(|v| match v {
            Value::Object(Some(o)) => Some(*o),
            _ => None,
        })
    });
    match first {
        Some(resolver) => is_a(vm, resolver, AI_COMPONENT_RESOLVER),
        None => Ok(false),
    }
}

/// The preflight of the AI component resolver's first question (lane PREFLIGHT-ORDER-LINK9-CAST): the context's
/// `QueryInterface(IAIInterface)` through the read-only dispatch ([`Vm::query_interface_read_only`]), before the next
/// load, so a question with no bound answer refuses before the evaluator's first write rather than after it. Its
/// `NotYet` is returned as it is, text and kind kept. Left to the dispatch: an answer (the question is asked again
/// when the resolver runs), a rule with no read-only form (`NotPorted`: the ordinary rule may answer), any other
/// error, and an interface class that is not loaded.
fn ai_question(vm: &Vm, context: ObjRef) -> Result<(), VmError> {
    let Some(iface) = vm.space.classes.by_path(AI_INTERFACE) else {
        return Ok(());
    };
    match vm.query_interface_read_only(&mut ow_vm::api::InterfaceAsking::default(), context, iface) {
        Err(VmError::NotYet(n)) if n.missing != ow_core::Missing::NotPorted => Err(n.into()),
        _ => Ok(()),
    }
}

/// Whether a definition takes the store path (note section 3, step 3.2) given `R`.
fn takes_store_path(vm: &Vm, definition: ObjRef, engine: Option<ObjRef>) -> Result<bool, VmError> {
    if engine.is_none() {
        return Ok(false);
    }
    let chain = field(vm, definition, "ContextResolverChain")?;
    let has_context = chain.elements().is_some_and(|e| e.iter().any(|v| matches!(v, Value::Object(Some(_)))));
    Ok(!has_context || object_field(vm, definition, "ValueResolver")?.is_none())
}

/// A flag definition's answer on the definition path (note section 4.0): `IsTrue` when `is_true`, else `IsFalse`.
fn definition_answer(
    vm: &mut Vm,
    definition: ObjRef,
    context: Option<ObjRef>,
    is_true: bool,
) -> Result<bool, VmError> {
    // Step 1: a None context answers false, and nothing else runs.
    let Some(context) = context else {
        return Ok(false);
    };
    let resolved = resolve(vm, definition, context)?;
    let resolver = object_field(vm, definition, "ValueResolver")?;
    if let Some(r) = resolver {
        load(vm, r)?;
        if !is_a(vm, r, RULE_ENGINE_RESOLVER)? && !is_a(vm, r, FUNCTION_RESOLVER)? {
            return Err(NotYet::not_read(
                format!("FlagDefinition: the value resolver class {} is not read", path_of(vm, r)),
                RECORD,
            )
            .into());
        }
    }
    if let Some(expression) = object_field(vm, definition, "EvaluationExpression")? {
        load(vm, expression)?;
        let answer = call_evaluate(vm, expression, Some(resolved))?;
        if resolver.is_some() {
            return Err(NotYet::not_read("FlagDefinition: handing a computed answer to its value resolver", RECORD).into());
        }
        return Ok(answer == is_true);
    }
    let Some(resolver) = resolver else {
        // Neither: IsTrue is false; IsFalse is not defined (section 4.0, step 4).
        if is_true {
            return Ok(false);
        }
        return Err(NotYet::not_read(
            "FlagDefinition: IsFalse with neither an EvaluationExpression nor a ValueResolver",
            RECORD,
        )
        .into());
    };
    let value = if is_a(vm, resolver, RULE_ENGINE_RESOLVER)? {
        let Some(store) = rule_engine_store(vm, resolved)? else {
            return Ok(false);
        };
        let entry = lookup(vm, store, definition)?;
        read_flag(vm, store, entry)?
    } else {
        let Some(store) = vm.query_interface(resolved, interface(vm, FLAG_PROVIDER)?)? else {
            return Ok(false);
        };
        check_store(vm, store)?;
        let entry = lookup(vm, store, definition)?;
        function_update(vm, store, entry, resolver, resolved)?;
        read_flag(vm, store, entry)?
    };
    Ok(value == is_true)
}

/// The interface class at `path`, which must be loaded: the class tables only answer for a loaded class, and loading
/// one is a write the rule does not make (lane FIX-FLAG-IMPLEMENTS-NOTYET).
fn interface(vm: &Vm, path: &str) -> Result<ClassId, VmError> {
    vm.space
        .classes
        .by_path(path)
        .ok_or_else(|| not_read(format!("the interface class {path} is not loaded (a load is a write)")))
}

/// The context through the definition's `ContextResolverChain` (note section 4.0, step 2): a None entry is skipped; a
/// resolver that answers None ends the chain and the context so far stays; an empty chain leaves the context as it
/// is. Each resolver handle, and each answer, is loaded when reached (note section 6); a resolver class the attribute
/// system does not bind is that system's own refusal.
fn resolve(vm: &mut Vm, definition: ObjRef, context: ObjRef) -> Result<ObjRef, VmError> {
    let chain = field(vm, definition, "ContextResolverChain")?;
    let entries = chain.elements().ok_or_else(|| script("ContextResolverChain is not an array".to_string()))?.to_vec();
    let mut current = context;
    let attrs = vm.attrs.clone();
    for entry in entries {
        let Some(resolver) = object_of(&entry, "ContextResolverChain entry")? else {
            continue;
        };
        load(vm, resolver)?;
        let r = attrs.context_resolver(&vm.space, resolver).map_err(VmError::from)?;
        match r.resolve(&attrs, resolver, definition, Some(current), &mut EvalCx::new(vm)).map_err(VmError::from)? {
            Some(next) => {
                load(vm, next)?;
                current = next;
            }
            None => break,
        }
    }
    Ok(current)
}

/// The store `RuleEngineFlagValueResolver` reads for `context`: the context when it answers `IFlagProvider` (by its
/// class); a `RuleEngine` context, or a pawn's controller's rule engine, is a rule engine's own store (not read).
fn rule_engine_store(vm: &mut Vm, context: ObjRef) -> Result<Option<ObjRef>, VmError> {
    if is_a(vm, context, RULE_ENGINE)? {
        return Err(not_read("a rule engine's own flag store".to_string()));
    }
    if implements(vm, context, FLAG_PROVIDER)? {
        check_store(vm, context)?;
        return Ok(Some(context));
    }
    if is_a(vm, context, "Engine.Pawn")? {
        load(vm, context)?;
        if let Some(controller) = object_field(vm, context, "Controller")?
            && rule_engine(vm, Some(controller))?.is_some()
        {
            return Err(not_read("a rule engine's own flag store".to_string()));
        }
    }
    Ok(None)
}

/// Only a `GearboxMind`'s flag store is read (note section 4.1). The store is loaded when reached (an
/// `IFlagProvider` answer, or the resolved context).
fn check_store(vm: &mut Vm, store: ObjRef) -> Result<(), VmError> {
    load(vm, store)?;
    if !is_a(vm, store, MIND)? {
        return Err(NotYet::not_read(
            format!("FlagDefinition: the flag store of a {} is not read", path_of(vm, store)),
            RECORD,
        )
        .into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The mind's flag store (note sections 4.1 and 4.2)
// ---------------------------------------------------------------------------------------------------------------

/// The `DynamicFlags` element struct of `store`'s class.
fn flags_struct(vm: &Vm, store: ObjRef) -> Result<StructId, VmError> {
    let class = vm.space.heap.get(store)?.class;
    let p = vm.space.prop(class, "DynamicFlags").ok_or_else(|| script(format!("{} declares no DynamicFlags", path_of(vm, store))))?;
    match vm.space.classes.decl(p).map(|d| d.kind.clone()) {
        Some(PropKind::Array(inner)) => match *inner {
            PropKind::Struct(s) => Ok(s),
            _ => Err(script("DynamicFlags is not an array of structs".to_string())),
        },
        _ => Err(script("DynamicFlags is not an array".to_string())),
    }
}

fn write_field(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} declares no {name}", path_of(vm, o))))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// The index of `definition`'s entry in `store`, adding it (definition and zeroed entry) when absent: a write.
fn lookup(vm: &mut Vm, store: ObjRef, definition: ObjRef) -> Result<usize, VmError> {
    let definitions = field(vm, store, "DynamicFlagDefinitions")?;
    let flags = field(vm, store, "DynamicFlags")?;
    let (Some(defs), Some(entries)) = (definitions.elements(), flags.elements()) else {
        return Err(script("DynamicFlagDefinitions or DynamicFlags is not an array".to_string()));
    };
    if defs.len() != entries.len() {
        return Err(not_read("a flag store whose DynamicFlagDefinitions and DynamicFlags are out of step".to_string()));
    }
    let index = match defs.iter().position(|v| matches!(v, Value::Object(Some(o)) if *o == definition)) {
        Some(i) => i,
        None => {
            let s = flags_struct(vm, store)?;
            let zero = vm.space.classes.struct_defaults(s).map_err(|e| script(e.to_string()))?;
            let mut defs = defs.to_vec();
            let mut entries = entries.to_vec();
            defs.push(Value::Object(Some(definition)));
            entries.push(zero);
            let i = defs.len() - 1;
            write_field(vm, store, "DynamicFlagDefinitions", Value::array(defs))?;
            write_field(vm, store, "DynamicFlags", Value::array(entries))?;
            i
        }
    };
    Ok(index)
}

fn entry_value(vm: &Vm, store: ObjRef, index: usize) -> Result<Value, VmError> {
    let flags = field(vm, store, "DynamicFlags")?;
    flags.elements().and_then(|e| e.get(index)).cloned().ok_or_else(|| script("a flag entry past DynamicFlags".to_string()))
}

fn update_entry(
    vm: &mut Vm,
    store: ObjRef,
    index: usize,
    f: impl FnOnce(&Vm, &mut Value) -> Result<(), VmError>,
) -> Result<(), VmError> {
    let flags = field(vm, store, "DynamicFlags")?;
    let mut entries = flags.elements().map(<[Value]>::to_vec).unwrap_or_default();
    let entry = entries.get_mut(index).ok_or_else(|| script("a flag entry past DynamicFlags".to_string()))?;
    f(vm, entry)?;
    write_field(vm, store, "DynamicFlags", Value::array(entries))
}

fn flag_get(vm: &Vm, entry: &Value, name: &str) -> Result<Value, VmError> {
    Ok(struct_field(vm, struct_field(vm, entry, "DynamicFlag")?, name)?.clone())
}

fn flag_set(vm: &Vm, entry: &mut Value, name: &str, v: Value) -> Result<(), VmError> {
    *struct_field_mut(vm, struct_field_mut(vm, entry, "DynamicFlag")?, name)? = v;
    Ok(())
}

/// Sets the flag (note section 4.2); `refresh` stamps the time again when the value does not change.
fn set_flag(vm: &mut Vm, store: ObjRef, index: usize, value: bool, refresh: bool) -> Result<(), VmError> {
    let time = now(vm)?;
    update_entry(vm, store, index, |vm, e| {
        let was = byte_of(&flag_get(vm, e, "Value")?, "Flag.Value")? != 0;
        if value != was {
            flag_set(vm, e, "Value", Value::Byte(u8::from(value)))?;
            flag_set(vm, e, "Raised", Value::Byte(u8::from(value)))?;
            flag_set(vm, e, "Lowered", Value::Byte(u8::from(!value)))?;
            flag_set(vm, e, if value { "RaisedTime" } else { "LoweredTime" }, Value::Float(time))?;
        } else if refresh {
            flag_set(vm, e, if value { "RaisedTime" } else { "LoweredTime" }, Value::Float(time))?;
        }
        flag_set(vm, e, "Duration", Value::Float(0.0))
    })
}

/// Reads the flag, after a timed flag's expiry (a write).
fn read_flag(vm: &mut Vm, store: ObjRef, index: usize) -> Result<bool, VmError> {
    let entry = entry_value(vm, store, index)?;
    let value = byte_of(&flag_get(vm, &entry, "Value")?, "Flag.Value")? != 0;
    let duration = float_of(&flag_get(vm, &entry, "Duration")?, "Flag.Duration")?;
    if value && duration > 0.0001 {
        let time = now(vm)?;
        let raised = float_of(&flag_get(vm, &entry, "RaisedTime")?, "Flag.RaisedTime")?;
        if time < raised || time - raised > duration {
            set_flag(vm, store, index, false, false)?;
            return Ok(false);
        }
    }
    Ok(value)
}

/// `ObjectFunctionFlagValueResolver`: when the time since the flag last changed is at least `CheckRate`, the script
/// function `FunctionCall` on the resolved context, with no arguments, sets the flag from its rounded result.
fn function_update(
    vm: &mut Vm,
    store: ObjRef,
    index: usize,
    resolver: ObjRef,
    context: ObjRef,
) -> Result<(), VmError> {
    let rate = float_of(&field(vm, resolver, "CheckRate")?, "CheckRate")?;
    let name = match field(vm, resolver, "FunctionCall")? {
        Value::Str(s) => s.to_string(),
        other => return Err(script(format!("FunctionCall holds {}, not a string", other.kind_name()))),
    };
    let entry = entry_value(vm, store, index)?;
    let value = byte_of(&flag_get(vm, &entry, "Value")?, "Flag.Value")? != 0;
    let changed = float_of(&flag_get(vm, &entry, if value { "RaisedTime" } else { "LoweredTime" })?, "the flag's time")?;
    if now(vm)? - changed < rate {
        return Ok(());
    }
    let Some(f) = crate::action_skill::method_of(vm, context, &name)? else {
        return Err(not_read(format!("the function {name} on {} (its class declares no such function)", path_of(vm, context))));
    };
    let number = match vm.call(f, Some(context), Vec::new())?.ret {
        Value::Bool(b) => i64::from(b),
        Value::Byte(b) => i64::from(b),
        Value::Int(i) => i64::from(i),
        // The port's own choice: only an integral value is taken, since truncating and rounding disagree between 0.5
        // and 1.
        Value::Float(x) if x.fract() == 0.0 => x as i64,
        Value::Float(_) => {
            return Err(not_read(
                "a fractional function-call flag result (the port takes only an integral one: truncating and rounding \
                 disagree between 0.5 and 1)"
                    .to_string(),
            ));
        }
        other => return Err(not_read(format!("a function-call flag result of kind {}", other.kind_name()))),
    };
    set_flag(vm, store, index, number != 0, true)
}

/// An expression's `Evaluate(context)` through the VM; a nested flag evaluator goes through this rule directly. The
/// counters see the writes of either.
fn call_evaluate(vm: &mut Vm, expression: ObjRef, context: Option<ObjRef>) -> Result<bool, VmError> {
    if is_a(vm, expression, EVALUATOR)? {
        return evaluate(vm, Some(expression), context);
    }
    load(vm, expression)?;
    let Some(f) = crate::action_skill::method_of(vm, expression, "Evaluate")? else {
        return Err(not_read(format!("Evaluate on {} (its class declares no such function)", path_of(vm, expression))));
    };
    match vm.call(f, Some(expression), vec![Value::Object(context)])?.ret {
        Value::Bool(b) => Ok(b),
        other => Err(script(format!("an expression's Evaluate answered {}, not a bool", other.kind_name()))),
    }
}

#[cfg(test)]
#[path = "flag_expression_tests.rs"]
mod tests;
