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
//! 3. the context's rule engine `R` (reads only); with `R`, the store path (the definition lacking a context or a value
//!    resolver) is `NotYet { NotRead }`, "a rule engine's own flag store";
//! 4. the definition path (section 4.0): a None context answers false; the context passes through
//!    `ContextResolverChain` (None entries skipped, a resolver answering None ends it and the context so far stays);
//!    a computed flag is evaluated on the resolved context; else the value resolver answers through the mind's flag
//!    store; with neither, `IsTrue` is false and `IsFalse` is `NotYet { NotRead }`.
//!
//! The chain combines by `FlagChainOperator` (`CEOP_Or` 0, `CEOP_And` 1, any other value false), with the early exits.
//!
//! **Loads are writes** (section 6): a reached handle (the receiver, a definition, its expression, its resolvers) is
//! loaded when first used, through ow-attr's `ensure_loaded` (`LayerAbsent` with no definition store, `NotRead` for a
//! stand-in), and the load counts for the mark.
//!
//! **Also `NotYet { NotRead }`:** a rule engine owner other than a `GearboxMind`; a value resolver class other than the
//! two read; a computed flag that also has a value resolver (the hand-off); a flag store that is not a `GearboxMind`'s;
//! a fractional function-call result (its rounding is not read); a store whose two arrays are out of step.
//!
//! **Writes** (note section 4.1): a lookup that adds a definition to the mind's store, a computed flag's
//! re-evaluation, a function call's flag update, and a timed flag's expiry. A `NotYet` after one of them is marked
//! "stopped after partial writes: ", its kind and record kept; other errors are returned as they are.

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
            let mut run = Run::default();
            let answer = evaluate(c.vm, me, context, &mut run);
            answer.map(Value::Bool).map_err(|e| run.mark(e))
        }),
    )
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{UNIT}: {message}") }
}

fn not_read(what: String) -> VmError {
    NotYet::not_read(format!("{UNIT}: {what}"), RECORD).into()
}

/// Whether the native has written (its partial-writes mark).
#[derive(Debug, Default)]
pub struct Run {
    pub wrote: bool,
}

impl Run {
    fn mark(&self, e: VmError) -> VmError {
        match e {
            VmError::NotYet(n) if self.wrote => n.after_partial_writes().into(),
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

/// Whether `o`'s class declares (or inherits) the interface at `path`, from the class tables alone.
fn implements(vm: &mut Vm, o: ObjRef, path: &str) -> Result<bool, VmError> {
    let Some(iface) = vm.space.classes.by_path(path) else {
        return Ok(false);
    };
    let class = vm.space.heap.get(o)?.class;
    vm.implements_checked(class, iface)
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
/// `NotRead`. A load counts as a write for the mark.
fn load(vm: &mut Vm, o: ObjRef, run: &mut Run) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        run.wrote = true;
    }
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
fn chain_of(vm: &mut Vm, me: Option<ObjRef>, run: &mut Run) -> Result<(ObjRef, Vec<Conditional>, u8), VmError> {
    let me = me.ok_or_else(|| script("called with no evaluator".to_string()))?;
    if !is_a(vm, me, EVALUATOR)? {
        return Err(script(format!("{} is not a FlagExpressionEvaluator", path_of(vm, me))));
    }
    load(vm, me, run)?;
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

/// The context's rule engine `R` (note section 3, step 1), by reads only; a None context gives none.
fn rule_engine(vm: &mut Vm, context: Option<ObjRef>) -> Result<Option<ObjRef>, VmError> {
    let Some(c) = context else {
        return Ok(None);
    };
    let owner_engine = |vm: &mut Vm, o: ObjRef| -> Result<Option<Option<ObjRef>>, VmError> {
        if !implements(vm, o, RULE_ENGINE_OWNER)? {
            return Ok(None);
        }
        loaded(vm, o, "rule engine owner")?;
        if !is_a(vm, o, MIND)? {
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

/// `FlagExpressionEvaluator.Evaluate(context)` on `me` (module documentation). `run` records a write. Only the
/// receiver and the chain's shape are checked up front; each conditional is checked when it is reached (note section
/// 6, "Per conditional").
pub fn evaluate(vm: &mut Vm, me: Option<ObjRef>, context: Option<ObjRef>, run: &mut Run) -> Result<bool, VmError> {
    let (_, chain, operator) = chain_of(vm, me, run)?;
    let mut engine: Option<Option<ObjRef>> = None;
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
                load(vm, definition, run)?;
                // Step 3: the path.
                if engine.is_none() {
                    engine = Some(rule_engine(vm, context)?);
                }
                if takes_store_path(vm, definition, engine.flatten())? {
                    return Err(not_read("a rule engine's own flag store".to_string()));
                }
                definition_answer(vm, definition, context, c.kind == 0, run)?
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
    run: &mut Run,
) -> Result<bool, VmError> {
    // Step 1: a None context answers false, and nothing else runs.
    let Some(context) = context else {
        return Ok(false);
    };
    let resolved = resolve(vm, definition, context, run)?;
    let resolver = object_field(vm, definition, "ValueResolver")?;
    if let Some(r) = resolver {
        load(vm, r, run)?;
        if !is_a(vm, r, RULE_ENGINE_RESOLVER)? && !is_a(vm, r, FUNCTION_RESOLVER)? {
            return Err(NotYet::not_read(
                format!("FlagDefinition: the value resolver class {} is not read", path_of(vm, r)),
                RECORD,
            )
            .into());
        }
    }
    if let Some(expression) = object_field(vm, definition, "EvaluationExpression")? {
        load(vm, expression, run)?;
        let answer = call_evaluate(vm, expression, Some(resolved), run)?;
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
        let entry = lookup(vm, store, definition, run)?;
        read_flag(vm, store, entry, run)?
    } else {
        let Some(store) = vm.query_interface(resolved, interface(vm, FLAG_PROVIDER)?)? else {
            return Ok(false);
        };
        check_store(vm, store)?;
        let entry = lookup(vm, store, definition, run)?;
        function_update(vm, store, entry, resolver, resolved, run)?;
        read_flag(vm, store, entry, run)?
    };
    Ok(value == is_true)
}

fn interface(vm: &Vm, path: &str) -> Result<ClassId, VmError> {
    vm.space.classes.by_path(path).ok_or_else(|| not_read(format!("the interface {path} is not in the loaded classes")))
}

/// The context through the definition's `ContextResolverChain` (note section 4.0, step 2): a None entry is skipped; a
/// resolver that answers None ends the chain and the context so far stays; an empty chain leaves the context as it
/// is. Each resolver handle is loaded when reached (a write); a resolver class the attribute system does not bind is
/// that system's own refusal.
fn resolve(vm: &mut Vm, definition: ObjRef, context: ObjRef, run: &mut Run) -> Result<ObjRef, VmError> {
    let chain = field(vm, definition, "ContextResolverChain")?;
    let entries = chain.elements().ok_or_else(|| script("ContextResolverChain is not an array".to_string()))?.to_vec();
    let mut current = context;
    let attrs = vm.attrs.clone();
    for entry in entries {
        let Some(resolver) = object_of(&entry, "ContextResolverChain entry")? else {
            continue;
        };
        load(vm, resolver, run)?;
        let r = attrs.context_resolver(&vm.space, resolver).map_err(VmError::from)?;
        match r.resolve(&attrs, resolver, definition, Some(current), &mut EvalCx::new(vm)).map_err(VmError::from)? {
            Some(next) => current = next,
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
        loaded(vm, context, "pawn")?;
        if let Some(controller) = object_field(vm, context, "Controller")?
            && rule_engine(vm, Some(controller))?.is_some()
        {
            return Err(not_read("a rule engine's own flag store".to_string()));
        }
    }
    Ok(None)
}

/// Only a `GearboxMind`'s flag store is read (note section 4.1).
fn check_store(vm: &Vm, store: ObjRef) -> Result<(), VmError> {
    loaded(vm, store, "flag store")?;
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

fn write_field(vm: &mut Vm, o: ObjRef, name: &str, v: Value, run: &mut Run) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} declares no {name}", path_of(vm, o))))?;
    run.wrote = true;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// The index of `definition`'s entry in `store`, adding it (definition and zeroed entry) when absent: a write.
fn lookup(vm: &mut Vm, store: ObjRef, definition: ObjRef, run: &mut Run) -> Result<usize, VmError> {
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
            write_field(vm, store, "DynamicFlagDefinitions", Value::array(defs), run)?;
            write_field(vm, store, "DynamicFlags", Value::array(entries), run)?;
            i
        }
    };
    // A computed flag is re-evaluated, with the mind as context, once per world time.
    if let Some(expression) = object_field(vm, definition, "EvaluationExpression")? {
        let time = now(vm)?;
        let entry = entry_value(vm, store, index)?;
        let last = float_of(struct_field(vm, &entry, "LastEvaluatedExpressionTime")?, "LastEvaluatedExpressionTime")?;
        if last != time {
            let answer = call_evaluate(vm, expression, Some(store), run)?;
            set_flag(vm, store, index, answer, false, run)?;
            update_entry(vm, store, index, run, |vm, e| {
                *struct_field_mut(vm, e, "LastEvaluatedExpressionTime")? = Value::Float(time);
                Ok(())
            })?;
        }
    }
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
    run: &mut Run,
    f: impl FnOnce(&Vm, &mut Value) -> Result<(), VmError>,
) -> Result<(), VmError> {
    let flags = field(vm, store, "DynamicFlags")?;
    let mut entries = flags.elements().map(<[Value]>::to_vec).unwrap_or_default();
    let entry = entries.get_mut(index).ok_or_else(|| script("a flag entry past DynamicFlags".to_string()))?;
    f(vm, entry)?;
    write_field(vm, store, "DynamicFlags", Value::array(entries), run)
}

fn flag_get(vm: &Vm, entry: &Value, name: &str) -> Result<Value, VmError> {
    Ok(struct_field(vm, struct_field(vm, entry, "DynamicFlag")?, name)?.clone())
}

fn flag_set(vm: &Vm, entry: &mut Value, name: &str, v: Value) -> Result<(), VmError> {
    *struct_field_mut(vm, struct_field_mut(vm, entry, "DynamicFlag")?, name)? = v;
    Ok(())
}

/// Sets the flag (note section 4.2); `refresh` stamps the time again when the value does not change.
fn set_flag(vm: &mut Vm, store: ObjRef, index: usize, value: bool, refresh: bool, run: &mut Run) -> Result<(), VmError> {
    let time = now(vm)?;
    update_entry(vm, store, index, run, |vm, e| {
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
fn read_flag(vm: &mut Vm, store: ObjRef, index: usize, run: &mut Run) -> Result<bool, VmError> {
    let entry = entry_value(vm, store, index)?;
    let value = byte_of(&flag_get(vm, &entry, "Value")?, "Flag.Value")? != 0;
    let duration = float_of(&flag_get(vm, &entry, "Duration")?, "Flag.Duration")?;
    if value && duration > 0.0001 {
        let time = now(vm)?;
        let raised = float_of(&flag_get(vm, &entry, "RaisedTime")?, "Flag.RaisedTime")?;
        if time < raised || time - raised > duration {
            set_flag(vm, store, index, false, false, run)?;
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
    run: &mut Run,
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
        // How a fractional result is rounded is not read (open question 5): only an integral value is taken.
        Value::Float(x) if x.fract() == 0.0 => x as i64,
        Value::Float(_) => return Err(not_read("the rounding of a fractional function-call flag result".to_string())),
        other => return Err(not_read(format!("a function-call flag result of kind {}", other.kind_name()))),
    };
    set_flag(vm, store, index, number != 0, true, run)
}

/// An expression's `Evaluate(context)` through the VM; a nested flag evaluator goes through this rule directly, so its
/// writes count for the caller's mark.
fn call_evaluate(vm: &mut Vm, expression: ObjRef, context: Option<ObjRef>, run: &mut Run) -> Result<bool, VmError> {
    if is_a(vm, expression, EVALUATOR)? {
        return evaluate(vm, Some(expression), context, run);
    }
    load(vm, expression, run)?;
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
