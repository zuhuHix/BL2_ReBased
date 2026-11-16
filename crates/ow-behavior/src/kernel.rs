//! Owner: lane W5-BEHAVIOR. The behavior kernel over one decoded provider: sequence enabling, event dispatch, behavior
//! threads, output links, delayed links and latent behaviors. AI-assisted (Claude Code). The port of the run half of
//! `src/behavior.cpp` (`BehaviorProvider`).
//!
//! Every rule below is read from the executable (docs/verification/NATIVE_MISSION_DISPATCH.md A1/A2 and
//! docs/verification/NATIVE_BEHAVIOR_POPULATION.md sections C and G) and is **UNVERIFIED in the running game**.
//!
//! **A1, firing an event** ([`Provider::fire_event`]). Every enabled sequence is visited in provider order; in it,
//! every event entry whose name matches (not only the first) is gated: skipped when `bEnabled` is false, when
//! `MaxTriggerCount > 0` and the trigger count has reached it, or when it has fired before and less than
//! `ReTriggerDelay` seconds have passed. Otherwise its count goes up, its time is set, its output variables are
//! published, and its links are walked in data order: a link is taken when the caller's link-id filter is -1 or equals
//! the link's id, and starts a thread due at now + `ActivateDelay`. A due thread runs at once, depth-first, before the
//! next link is considered. `FilterObject` is not evaluated (G3: the callers read pass no filter callback).
//!
//! **A2, running a thread.** Behaviors run until the thread ends or waits, at most 60 per call (a capped thread waits
//! for the next [`Provider::tick`]; what the game does with it was read only as "continues on the next runner pass").
//! A behavior's handler (`crate::registry`) returns the ids it selected; -1 is appended when the behavior's
//! `Context.bSupportsDefaultOutputLink` is set. For each id in order, every link of the behavior with that id is
//! selected, in data order; links with other ids are not followed. The first selected link continues this thread
//! (after its delay), every other one starts a new thread first. No deduplication: a behavior reached twice runs twice.
//! A runaway guard ends a public call after 10,000 behaviors (`behavior execution limit exceeded`, the waiting
//! threads dropped): the C++ guard, not a game rule.
//!
//! **Depth-first without recursion.** The C++ runs a started thread by recursion, so package data (a behavior that
//! links back to itself twice) drives the recursion up to the 10,000-behavior guard, deeper than a Rust stack takes.
//! Here a public call keeps an explicit stack of work (a link taken, the next behavior of a thread, a thread waiting on
//! its first link's delay, a latent behavior's wait) and always runs the most recently pushed item first. A behavior's
//! follow-up is pushed in reverse of the order the C++ performs it (the continuation or the wait first, then the other
//! selected links from last to first), so the operations happen in the C++ order: the same trace, the same parks and
//! the same order numbers. A handler that calls back into the kernel (a nested event) drains only the work it adds.
//!
//! **Latent behaviors** (G2). A handler may set [`RunInfo::wait`]: its selected links all start new threads now, and
//! the behavior runs again on this thread at [`ow_vm::latent::kernel_resume_at`] (`now + max(wait, 1/60)`), with
//! `initial_run` false and the same `state`. A negative wait means finished. A latent thread whose sequence was
//! disabled while it waited ends without running (G1); the record's check before **each** behavior of a running
//! thread is not applied, as in the C++.
//!
//! **Section C, enabling.** Enabling a sequence fires `OnBehaviorSequenceEnabled` on it after its enabled bit is set;
//! disabling fires `OnBehaviorSequenceDisabled` before the bit is cleared. Enabling a sequence with
//! `bSequenceEnabledMutex` first disables one other enabled sequence of the provider that has it (at most one).
//! [`Provider::register_consumer`] (`IntializeBehaviorProviderForConsumer` pass 1) disables every sequence, then enables
//! each one with `bEnabledOnSpawn` in order. Sequences are addressed by name; every sequence of that name is changed,
//! as the C++ does.
//!
//! **Time.** The kernel keeps its own time ([`Provider::now`], seconds as a double), advanced by [`Provider::tick`];
//! due threads run in due-time order and time advances to each due time, so a thread that parks during the call and
//! falls due before its end runs in the same call. A thread parked at the current time (the 60-behavior cap) waits for
//! the next call. Which clock the game's kernel reads is UNVERIFIED.
//!
//! **Context objects and input bindings** (G2, read coarsely). The objects a behavior runs on ([`Provider::contexts`])
//! are those of its Context-type variable link, else the consumer's own object. Before a handler runs, each Input link
//! whose property is a `...BehaviorContextData` struct gets the first linked object in its `ContextObject` and its
//! `BehaviorContext` selector set to 4 (`UseContextObject`); after the run `ContextObject` is None again and the
//! selector stays 4 (the C++ reading, NATIVE_BEHAVIOR_CONTEXT.md). The kernel runs on the behavior definitions the
//! store loaded, so these writes land on the shared definition object, as the record says the game's runner writes
//! the shared behavior object and clears it after (the C++ ran on a per-provider copy). The game's per-thread copy of a
//! latent behavior is the handler's `state` here.
//!
//! **Names.** Event and sequence names compare case-insensitively (FNames). The C++ compares the texts exactly; the
//! two differ only for a caller that spells a name in another case.

use std::any::Any;
use std::rc::Rc;

use ow_core::{DetMap, NotYet};
use ow_object::heap::Step;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::latent::kernel_resume_at;

use crate::provider::{
    self, Behavior, BehaviorError, ProviderData, Variable, VariableLink, stock_path,
};
use crate::registry::{self, Handler, Registry};

/// The records of the kernel rules.
pub const RECORD_DISPATCH: &str = "docs/verification/NATIVE_MISSION_DISPATCH.md (A1, A2)";
pub const RECORD_KERNEL: &str = "docs/verification/NATIVE_BEHAVIOR_POPULATION.md (sections C, G)";

/// Behaviors one thread runs per call before it waits for the next tick (A2).
pub const THREAD_BEHAVIOR_CAP: usize = 60;

/// Behaviors one outermost public call may run (the C++ runaway guard).
pub const CALL_BUDGET: usize = 10_000;

/// The selector `BehaviorContextData.BehaviorContext` is set to for a bound input (`UseContextObject`).
pub const USE_CONTEXT_OBJECT: u8 = 4;

/// The event fired on a sequence that was just enabled / is being disabled.
pub const SEQUENCE_ENABLED_EVENT: &str = "OnBehaviorSequenceEnabled";
pub const SEQUENCE_DISABLED_EVENT: &str = "OnBehaviorSequenceDisabled";

/// One behavior of the provider: its sequence index and its index in that sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BehaviorId {
    pub sequence: usize,
    pub index: usize,
}

/// A behavior's view of its current run (G2 call information, UNVERIFIED). A handler that sets `wait` (not negative)
/// becomes latent (module documentation); `state` survives the wait.
#[non_exhaustive]
#[derive(Clone)]
pub struct RunInfo {
    /// The first run of this behavior in its thread.
    pub initial_run: bool,
    /// The behavior has any output link.
    pub has_linked_outputs: bool,
    /// Seconds to wait before running again; negative means finished.
    pub wait: f64,
    pub state: Option<Rc<dyn Any>>,
}

impl Default for RunInfo {
    fn default() -> Self {
        RunInfo {
            initial_run: true,
            has_linked_outputs: false,
            wait: -1.0,
            state: None,
        }
    }
}

impl std::fmt::Debug for RunInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunInfo")
            .field("initial_run", &self.initial_run)
            .field("has_linked_outputs", &self.has_linked_outputs)
            .field("wait", &self.wait)
            .field("state", &self.state.is_some())
            .finish()
    }
}

/// A behavior reached but not run (a boundary class), with the decoded fields the host needs to run it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCall {
    pub event: String,
    pub class: String,
    pub name: String,
    pub sequence: String,
    pub fields: DetMap<String, String>,
}

/// Called with the kernel's time each time [`Provider::tick`] advances it (to a thread's due time, and to the end of
/// the call), before the thread runs.
pub type TimeHook = Rc<dyn Fn(&mut Vm, f64)>;

#[derive(Debug, Clone, Copy, PartialEq)]
struct EventState {
    trigger_count: i32,
    last_trigger_time: f64,
}

#[derive(Clone)]
struct Thread {
    due: f64,
    order: u64,
    sequence: usize,
    behavior: usize,
    event: String,
    resumed: bool,
    state: Option<Rc<dyn Any>>,
}

/// One unit of the runner's work (module documentation, "Depth-first without recursion").
enum Work {
    /// A link taken: parked when it has a delay, else a new thread at its behavior.
    Start {
        s: usize,
        b: usize,
        delay: f64,
        event: String,
    },
    /// The next behavior of a running thread.
    Run {
        s: usize,
        b: usize,
        event: String,
        resumed: bool,
        state: Option<Rc<dyn Any>>,
        ran: usize,
    },
    /// A thread's first selected link has a delay: the thread waits for it.
    ParkAfter {
        delay: f64,
        s: usize,
        b: usize,
        event: String,
    },
    /// A latent behavior waits, after the links it selected have started.
    Wait {
        wait: f64,
        s: usize,
        b: usize,
        event: String,
        state: Option<Rc<dyn Any>>,
    },
}

/// Where a thread is: its behavior, its event, whether it resumes a latent behavior, the latent state, and how
/// many behaviors it ran in this call.
struct ThreadAt {
    s: usize,
    b: usize,
    event: String,
    resumed: bool,
    state: Option<Rc<dyn Any>>,
    ran: usize,
}

/// The kernel over one provider (module documentation).
pub struct Provider {
    data: ProviderData,
    /// Per sequence: enabled, and each event's trigger state.
    enabled: Vec<bool>,
    events: Vec<Vec<EventState>>,
    local: Registry,
    kismet: DetMap<String, ObjRef>,
    waiting: Vec<Thread>,
    work: Vec<Work>,
    now: f64,
    order: u64,
    budget: usize,
    depth: u32,
    registered: bool,
    run: RunInfo,
    /// Unhandled behavior classes, the execution limit, unknown inputs (the C++ texts).
    pub errors: Vec<String>,
    /// The `NotYet` of each unregistered class reached, in order.
    pub not_yet: Vec<NotYet>,
    /// Boundary behaviors reached (`event -> class:name`).
    pub boundary: Vec<String>,
    pub boundary_calls: Vec<BoundaryCall>,
    /// `event -> behavior` in execution order.
    pub trace: Vec<String>,
    pub on_time: Option<TimeHook>,
}

impl std::fmt::Debug for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Provider")
            .field("path", &self.data.path)
            .field("now", &self.now)
            .field("errors", &self.errors)
            .field("trace", &self.trace)
            .finish_non_exhaustive()
    }
}

impl Provider {
    /// Decodes `definition` ([`provider::decode`]) and starts the kernel over it: each sequence enabled as its
    /// `bEnabledOnSpawn` says, time 0, nothing waiting.
    pub fn new(vm: &mut Vm, definition: ObjRef) -> Result<Self, BehaviorError> {
        Ok(Self::from_data(provider::decode(vm, definition)?))
    }

    /// The kernel over already decoded data.
    pub fn from_data(data: ProviderData) -> Self {
        let enabled = data.sequences.iter().map(|s| s.enabled_on_spawn).collect();
        let events = data
            .sequences
            .iter()
            .map(|s| {
                vec![
                    EventState {
                        trigger_count: 0,
                        last_trigger_time: 0.0,
                    };
                    s.events.len()
                ]
            })
            .collect();
        Provider {
            data,
            enabled,
            events,
            local: Registry::new(),
            kismet: DetMap::new(),
            waiting: Vec::new(),
            work: Vec::new(),
            now: 0.0,
            order: 0,
            budget: 0,
            depth: 0,
            registered: false,
            run: RunInfo::default(),
            errors: Vec::new(),
            not_yet: Vec::new(),
            boundary: Vec::new(),
            boundary_calls: Vec::new(),
            trace: Vec::new(),
            on_time: None,
        }
    }

    // ---- handlers ------------------------------------------------------------------------------------------------

    /// Sets this provider's own handler for `class` (consulted before the VM's registry).
    pub fn handle(&mut self, class: &str, handler: Handler) {
        self.local.set(class, handler);
    }

    /// The class acts on the world and has no binding yet (`crate::registry`, [`Handler::Boundary`]).
    pub fn report_at_boundary(
        &mut self,
        class: &str,
        describe: Option<registry::DescribeFn>,
        outputs: Vec<i32>,
    ) {
        self.local
            .set(class, Handler::Boundary { describe, outputs });
    }

    fn handler_for(&self, vm: &Vm, class: &str) -> Result<Handler, NotYet> {
        match self.local.get(class) {
            Ok(h) => Ok(h),
            Err(_) => registry::lookup(vm, class),
        }
    }

    // ---- reading ----------------------------------------------------------------------------------------------------

    pub fn data(&self) -> &ProviderData {
        &self.data
    }

    /// The definition's path in its package.
    pub fn path(&self) -> &str {
        &self.data.path
    }

    /// The provider definition object.
    pub fn definition(&self) -> ObjRef {
        self.data.definition
    }

    pub fn values_decoded(&self) -> bool {
        self.data.values_decoded
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.data.diagnostics
    }

    pub fn registered(&self) -> bool {
        self.registered
    }

    /// The kernel's time in seconds.
    pub fn now(&self) -> f64 {
        self.now
    }

    /// The running behavior's call information.
    pub fn run_info(&mut self) -> &mut RunInfo {
        &mut self.run
    }

    pub fn behavior(&self, id: BehaviorId) -> &Behavior {
        &self.data.sequences[id.sequence].behaviors[id.index]
    }

    /// The name of the sequence `id` belongs to.
    pub fn sequence_name(&self, id: BehaviorId) -> &str {
        &self.data.sequences[id.sequence].name
    }

    /// The sequence names in provider order (repeated names repeated).
    pub fn sequence_names(&self) -> Vec<String> {
        self.data.sequences.iter().map(|s| s.name.clone()).collect()
    }

    fn first_named(&self, name: &str) -> Option<usize> {
        self.data
            .sequences
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// Whether the first sequence of that name is enabled (false when there is none).
    pub fn sequence_enabled(&self, name: &str) -> bool {
        self.first_named(name).is_some_and(|s| self.enabled[s])
    }

    /// The first sequence of that name's `CustomEnableCondition`.
    pub fn enable_condition(&self, name: &str) -> Option<ObjRef> {
        self.first_named(name)
            .and_then(|s| self.data.sequences[s].condition)
    }

    /// The first sequence of that name's variables (empty when there is none).
    pub fn variables(&self, name: &str) -> &[Variable] {
        self.first_named(name)
            .map_or(&[], |s| &self.data.sequences[s].variables)
    }

    /// The first sequence of that name's behaviors (empty when there is none).
    pub fn behaviors(&self, name: &str) -> &[Behavior] {
        self.first_named(name)
            .map_or(&[], |s| &self.data.sequences[s].behaviors)
    }

    /// The wiring of every sequence of that name as text lines (`--behavior-dump`): each event's links, then each
    /// behavior's, as `<from> -> <behavior> id=<id> delay=<delay>` with the delay as `std::to_string` prints it.
    pub fn graph(&self, name: &str) -> Vec<String> {
        let mut lines = Vec::new();
        for sequence in self
            .data
            .sequences
            .iter()
            .filter(|s| s.name.eq_ignore_ascii_case(name))
        {
            let mut describe = |from: &str, start: usize, length: usize| {
                for link in &sequence.links[start..start + length] {
                    lines.push(format!(
                        "{from} -> {} id={} delay={}",
                        sequence.behaviors[link.behavior].name,
                        link.id,
                        ow_core::fmt::fmt_f(link.delay, 6)
                    ));
                }
            };
            for event in &sequence.events {
                describe(&format!("event {}", event.name), event.start, event.length);
            }
            for behavior in &sequence.behaviors {
                describe(
                    &format!("{}:{}", behavior.class, behavior.name),
                    behavior.start,
                    behavior.length,
                );
            }
        }
        lines
    }

    /// Whether any sequence has an event of that name.
    pub fn has_event(&self, event: &str) -> bool {
        self.data
            .sequences
            .iter()
            .any(|s| s.events.iter().any(|e| e.name.eq_ignore_ascii_case(event)))
    }

    // ---- the call guard -------------------------------------------------------------------------------------------

    /// One public call; the outermost refills the runaway budget. A call made while a behavior of this provider runs
    /// (a handler firing an event on it) keeps that behavior's run information.
    fn enter(&mut self) -> (bool, RunInfo) {
        let nested = self.depth > 0;
        let saved = self.run.clone();
        if self.depth == 0 {
            self.budget = CALL_BUDGET;
        }
        self.depth += 1;
        (nested, saved)
    }

    fn leave(&mut self, (nested, saved): (bool, RunInfo)) {
        self.depth -= 1;
        if nested {
            self.run = saved;
        }
    }

    fn call<T>(
        &mut self,
        vm: &mut Vm,
        body: impl FnOnce(&mut Self, &mut Vm) -> Result<T, BehaviorError>,
    ) -> Result<T, BehaviorError> {
        let guard = self.enter();
        let result = body(self, vm);
        self.leave(guard);
        result
    }

    // ---- enabling ---------------------------------------------------------------------------------------------------

    /// Enables or disables every sequence of that name (section C, module documentation). False when there is none.
    pub fn set_sequence_enabled(
        &mut self,
        vm: &mut Vm,
        name: &str,
        enabled: bool,
    ) -> Result<bool, BehaviorError> {
        let mut found = false;
        for s in 0..self.data.sequences.len() {
            if !self.data.sequences[s].name.eq_ignore_ascii_case(name) {
                continue;
            }
            found = true;
            if self.enabled[s] == enabled {
                continue;
            }
            self.call(vm, |p, vm| {
                if enabled {
                    if p.data.sequences[s].mutex {
                        let other = (0..p.data.sequences.len())
                            .find(|&o| o != s && p.enabled[o] && p.data.sequences[o].mutex);
                        if let Some(o) = other {
                            let other_name = p.data.sequences[o].name.clone();
                            p.set_sequence_enabled(vm, &other_name, false)?;
                        }
                    }
                    // The enabled event is delivered after the bit is set ...
                    p.enabled[s] = true;
                    p.fire_in(vm, s, SEQUENCE_ENABLED_EVENT, &DetMap::new(), -1, &[])
                } else {
                    // ... the disabled event before it is cleared.
                    let fired = p.fire_in(vm, s, SEQUENCE_DISABLED_EVENT, &DetMap::new(), -1, &[]);
                    p.enabled[s] = false;
                    fired
                }
            })?;
        }
        Ok(found)
    }

    /// Registering the provider on a consumer (`IntializeBehaviorProviderForConsumer` pass 1, section C): every
    /// sequence disabled, then every sequence with `bEnabledOnSpawn` enabled in order (its enabled event fires). The
    /// caller then applies the enable conditions (pass 2) and only then fires `OnSpawned`. Once per provider.
    pub fn register_consumer(&mut self, vm: &mut Vm) -> Result<(), BehaviorError> {
        if self.registered {
            return Ok(());
        }
        self.registered = true;
        self.enabled.iter_mut().for_each(|e| *e = false);
        let on_spawn: Vec<String> = self
            .data
            .sequences
            .iter()
            .filter(|s| s.enabled_on_spawn)
            .map(|s| s.name.clone())
            .collect();
        for name in on_spawn {
            self.set_sequence_enabled(vm, &name, true)?;
        }
        Ok(())
    }

    // ---- variables and inputs --------------------------------------------------------------------------------------

    /// Binds a named Kismet variable (`BVAR_NamedKismetVariable`): the level sequence's object variable of that name,
    /// which the kernel reads when a behavior is applied. The owner of the world binds it; `None` clears the name. An
    /// unbound name resolves to no object (UNVERIFIED: docs/verification/FIRE_MISSION_GOLDEN_TRACE.md, lane L4).
    pub fn bind_kismet_variable(&mut self, name: &str, object: Option<ObjRef>) {
        match object {
            Some(o) => {
                self.kismet.insert(name.to_string(), o);
            }
            None => {
                self.kismet.remove(name);
            }
        }
    }

    /// The objects variable link `link` of behavior `id` resolves to: a named-variable reference resolves to the
    /// first same-named variable of the sequence that is not itself a reference; an object variable contributes its
    /// live object when it has one; a named Kismet variable contributes the object bound to that name, if any.
    pub fn linked_objects(&self, id: BehaviorId, link: &VariableLink) -> Vec<ObjRef> {
        let variables = &self.data.sequences[id.sequence].variables;
        let mut result = Vec::new();
        for &index in &link.variables {
            let mut variable = &variables[index];
            if variable.var_type == "BVAR_NamedKismetVariable" {
                if let Some(&o) = self.kismet.get(&variable.name) {
                    result.push(o);
                }
                continue;
            }
            if variable.var_type == "BVAR_NamedVariable" {
                let name = &variable.name;
                match variables.iter().find(|c| {
                    &c.name == name
                        && c.var_type != "BVAR_NamedVariable"
                        && c.var_type != "BVAR_NamedKismetVariable"
                }) {
                    Some(c) => variable = c,
                    None => continue,
                }
            }
            if variable.var_type == "BVAR_Object"
                && let Some(o) = variable.live
            {
                result.push(o);
            }
        }
        result
    }

    /// The objects behavior `id` runs on: those of its Context-type variable link, else `own` (the consumer's own
    /// object). G2, UNVERIFIED. An empty list means the behavior does not run.
    pub fn contexts(&self, id: BehaviorId, own: ObjRef) -> Vec<ObjRef> {
        let behavior = self.behavior(id);
        match behavior
            .variables
            .iter()
            .find(|l| l.link_type == "BVARLINK_Context")
        {
            Some(link) => self.linked_objects(id, link),
            None => vec![own],
        }
    }

    /// A property linked with a type that is neither Input nor Output means the link data was not understood: an
    /// error, never a silent fall back to the behavior's own property.
    fn unexpected_link(&mut self, id: BehaviorId, property: &str) -> bool {
        let behavior = self.behavior(id);
        if let Some(link) = behavior.variables.iter().find(|l| {
            l.property == property
                && l.link_type != "BVARLINK_Input"
                && l.link_type != "BVARLINK_Output"
        }) {
            let text = format!(
                "unexpected variable link type {} for {}.{property}",
                link.link_type, behavior.name
            );
            self.errors.push(text);
            return true;
        }
        false
    }

    /// The input variable behind `property` of behavior `id`, or `Ok(None)` when the property has no Input link
    /// (`Err(())` when the value is unknown: the error is recorded).
    fn input_variable(
        &mut self,
        id: BehaviorId,
        property: &str,
    ) -> Result<Option<Option<Variable>>, ()> {
        let behavior = self.behavior(id);
        let Some(link) = behavior
            .variables
            .iter()
            .find(|l| l.property == property && l.link_type == "BVARLINK_Input")
        else {
            return Ok(None);
        };
        if !self.data.values_decoded {
            let text = format!(
                "variable values not decoded for {}.{property} in {}",
                behavior.name, self.data.path
            );
            self.errors.push(text);
            return Err(());
        }
        // UNVERIFIED: an input linked to several variables reads the first one (none observed on the slice route).
        Ok(Some(link.variables.first().map(|&v| {
            self.data.sequences[id.sequence].variables[v].clone()
        })))
    }

    /// The object behavior property `property` reads, as a stock path (`""` is None): through its Input variable link
    /// if it has one (the variable's current value), else the behavior object's own property. `Ok(None)` (and an
    /// entry in `errors`) when the linked value is unknown.
    pub fn object_input(
        &mut self,
        vm: &Vm,
        id: BehaviorId,
        property: &str,
    ) -> Result<Option<String>, BehaviorError> {
        if self.unexpected_link(id, property) {
            return Ok(None);
        }
        match self.input_variable(id, property) {
            Err(()) => Ok(None),
            Ok(Some(None)) => Ok(Some(String::new())),
            Ok(Some(Some(variable))) => {
                if variable.var_type != "BVAR_Object" {
                    let text = format!(
                        "object input {}.{property} is linked to a {} variable",
                        self.behavior(id).name,
                        variable.var_type
                    );
                    self.errors.push(text);
                    return Ok(None);
                }
                Ok(Some(variable.object))
            }
            Ok(None) => match provider::property(&vm.space, self.behavior(id).object, property)? {
                Some(Value::Object(Some(o))) => Ok(Some(stock_path(vm, o)?)),
                _ => Ok(Some(String::new())),
            },
        }
    }

    /// The same for an int property; the second value tells whether it came from a variable.
    pub fn int_input(
        &mut self,
        vm: &Vm,
        id: BehaviorId,
        property: &str,
    ) -> Result<(Option<i32>, bool), BehaviorError> {
        if self.unexpected_link(id, property) {
            return Ok((None, false));
        }
        match self.input_variable(id, property) {
            Err(()) => Ok((None, true)),
            Ok(Some(None)) => Ok((Some(0), true)),
            Ok(Some(Some(variable))) => {
                if variable.var_type != "BVAR_Int" {
                    let text = format!(
                        "int input {}.{property} is linked to a {} variable",
                        self.behavior(id).name,
                        variable.var_type
                    );
                    self.errors.push(text);
                    return Ok((None, true));
                }
                Ok((Some(variable.word), true))
            }
            Ok(None) => {
                let own = provider::property(&vm.space, self.behavior(id).object, property)?;
                let value = match own {
                    Some(Value::Int(i)) => i,
                    Some(Value::Byte(b)) => i32::from(b),
                    Some(Value::Bool(b)) => i32::from(b),
                    // The C++ `int32_t(own->integer())`: int64_t(float), then kept to 32 bits (N5).
                    Some(Value::Float(f)) => ow_core::num::trunc_i64_wrap_i32(f64::from(f)),
                    _ => 0,
                };
                Ok((Some(value), false))
            }
        }
    }

    /// Whether `v` is a `...BehaviorContextData` struct (any case), the C++ `isContextData`.
    fn is_context_data(vm: &Vm, v: &Value) -> bool {
        v.as_struct()
            .and_then(|s| vm.space.classes.struct_layout(s.ty))
            .is_some_and(|l| {
                l.path()
                    .as_str()
                    .to_ascii_lowercase()
                    .ends_with("behaviorcontextdata")
            })
    }

    /// Writes field `name` of struct property `prop` of `o` (when both exist).
    fn write_context_field(
        vm: &mut Vm,
        o: ObjRef,
        prop: &str,
        name: &str,
        v: Value,
    ) -> Result<(), BehaviorError> {
        let class = vm.space.heap.get(o)?.class;
        let Some(p) = vm.space.prop(class, prop) else {
            return Ok(());
        };
        let addr = PropAddr::scalar(p);
        let entry = match vm.space.read(o, addr)? {
            Value::Struct(s) => vm
                .space
                .names
                .find(name)
                .and_then(|n| vm.space.classes.struct_layout(s.ty)?.field(n))
                .map(|(entry, _)| entry),
            _ => None,
        };
        if let Some(entry) = entry {
            vm.space.write_in(o, addr, &[Step::Field(entry)], v)?;
        }
        Ok(())
    }

    /// The runner's input binding of `BehaviorContextData` properties (module documentation), before a run.
    fn bind_context_inputs(&mut self, vm: &mut Vm, id: BehaviorId) -> Result<(), BehaviorError> {
        let behavior = self.behavior(id).clone();
        for link in behavior
            .variables
            .iter()
            .filter(|l| l.link_type == "BVARLINK_Input")
        {
            let Some(value) = provider::property(&vm.space, behavior.object, &link.property)?
            else {
                continue;
            };
            if !Self::is_context_data(vm, &value) {
                continue;
            }
            let first = self.linked_objects(id, link).first().copied();
            Self::write_context_field(
                vm,
                behavior.object,
                &link.property,
                "ContextObject",
                Value::Object(first),
            )?;
            Self::write_context_field(
                vm,
                behavior.object,
                &link.property,
                "BehaviorContext",
                Value::Byte(USE_CONTEXT_OBJECT),
            )?;
        }
        Ok(())
    }

    /// After a run: `ContextObject` is None again (the selector stays).
    fn clear_context_inputs(&mut self, vm: &mut Vm, id: BehaviorId) -> Result<(), BehaviorError> {
        let behavior = self.behavior(id).clone();
        for link in behavior
            .variables
            .iter()
            .filter(|l| l.link_type == "BVARLINK_Input")
        {
            let Some(value) = provider::property(&vm.space, behavior.object, &link.property)?
            else {
                continue;
            };
            if Self::is_context_data(vm, &value) {
                Self::write_context_field(
                    vm,
                    behavior.object,
                    &link.property,
                    "ContextObject",
                    Value::Object(None),
                )?;
            }
        }
        Ok(())
    }

    // ---- events -----------------------------------------------------------------------------------------------------

    /// Fires `event` on every enabled sequence (A1, module documentation). `outputs` are the event's output values by
    /// property name (a stock object path each), written to the object variables the event links as outputs (a
    /// property the caller did not supply is None). `link_id` is the caller's filter: -1 follows every link.
    /// `payload` is the objects the raiser attached: when given, an Output link whose connection index is below its
    /// length takes that element into its object variables, by index and not by property name
    /// (NATIVE_MARCUS_USE_CHAIN.md, "Event payload to named variable", UNVERIFIED); `outputs` is then not consulted.
    pub fn fire_event(
        &mut self,
        vm: &mut Vm,
        event: &str,
        outputs: &DetMap<String, String>,
        link_id: i32,
        payload: &[Option<ObjRef>],
    ) -> Result<(), BehaviorError> {
        self.call(vm, |p, vm| {
            for s in 0..p.data.sequences.len() {
                if p.enabled[s] {
                    p.fire_in(vm, s, event, outputs, link_id, payload)?;
                }
            }
            Ok(())
        })
    }

    fn fire_in(
        &mut self,
        vm: &mut Vm,
        s: usize,
        event: &str,
        outputs: &DetMap<String, String>,
        link_id: i32,
        payload: &[Option<ObjRef>],
    ) -> Result<(), BehaviorError> {
        for e in 0..self.data.sequences[s].events.len() {
            let entry = &self.data.sequences[s].events[e];
            if !entry.name.eq_ignore_ascii_case(event) || !entry.enabled {
                continue;
            }
            let state = self.events[s][e];
            if entry.max_trigger_count > 0 && state.trigger_count >= entry.max_trigger_count {
                continue;
            }
            if state.trigger_count >= 1
                && self.now - state.last_trigger_time < entry.re_trigger_delay
            {
                continue;
            }
            self.events[s][e] = EventState {
                trigger_count: state.trigger_count.saturating_add(1),
                last_trigger_time: self.now,
            };
            // The event publishes its outputs into the variables it links.
            let links: Vec<VariableLink> = entry.variables.clone();
            let (start, length) = (entry.start, entry.length);
            for link in links.iter().filter(|l| l.link_type == "BVARLINK_Output") {
                if !payload.is_empty() {
                    let Some(element) = usize::try_from(link.connection)
                        .ok()
                        .and_then(|c| payload.get(c))
                        .copied()
                    else {
                        continue;
                    };
                    let path = match element {
                        Some(o) => stock_path(vm, o)?,
                        None => String::new(),
                    };
                    for &v in &link.variables {
                        let variable = &mut self.data.sequences[s].variables[v];
                        if variable.var_type == "BVAR_Object" {
                            variable.live = element;
                            variable.object = path.clone();
                        }
                    }
                    continue;
                }
                let value = outputs.get(&link.property).cloned().unwrap_or_default();
                for &v in &link.variables {
                    let variable = &mut self.data.sequences[s].variables[v];
                    if variable.var_type == "BVAR_Object" {
                        variable.object = value.clone();
                        variable.live = None;
                    }
                }
            }
            for i in start..start + length {
                let link = self.data.sequences[s].links[i];
                if link_id == -1 || link.id == link_id {
                    self.start(vm, s, link.behavior, link.delay, event)?;
                }
            }
        }
        Ok(())
    }

    /// Starts a thread at behavior `b` of sequence `s`: parked when the link has a delay, else run at once,
    /// depth-first, before this returns.
    fn start(
        &mut self,
        vm: &mut Vm,
        s: usize,
        b: usize,
        delay: f64,
        event: &str,
    ) -> Result<(), BehaviorError> {
        let base = self.work.len();
        self.work.push(Work::Start {
            s,
            b,
            delay,
            event: event.to_string(),
        });
        self.drain(vm, base)
    }

    fn park(
        &mut self,
        due: f64,
        s: usize,
        b: usize,
        event: &str,
        resumed: bool,
        state: Option<Rc<dyn Any>>,
    ) {
        self.waiting.push(Thread {
            due,
            order: self.order,
            sequence: s,
            behavior: b,
            event: event.to_string(),
            resumed,
            state,
        });
        self.order += 1;
    }

    /// Advances the kernel's time by `seconds`, running the threads that fall due in due-time order (module
    /// documentation).
    pub fn tick(&mut self, vm: &mut Vm, seconds: f64) -> Result<(), BehaviorError> {
        let target = self.now + seconds;
        self.call(vm, |p, vm| {
            let horizon = p.order;
            loop {
                let mut next: Option<usize> = None;
                for (i, t) in p.waiting.iter().enumerate() {
                    if !(t.due <= target && (t.order < horizon || t.due > p.now)) {
                        continue;
                    }
                    let better = match next {
                        None => true,
                        Some(n) => {
                            let best = &p.waiting[n];
                            t.due < best.due || (t.due == best.due && t.order < best.order)
                        }
                    };
                    if better {
                        next = Some(i);
                    }
                }
                let Some(n) = next else { break };
                let thread = p.waiting.remove(n);
                // std::max(now, due): the due time when it is later.
                if p.now < thread.due {
                    p.now = thread.due;
                }
                if let Some(hook) = p.on_time.clone() {
                    hook(vm, p.now);
                }
                // A latent thread whose sequence was disabled while it waited ends without running (G1).
                if thread.resumed && !p.enabled[thread.sequence] {
                    continue;
                }
                let base = p.work.len();
                p.work.push(Work::Run {
                    s: thread.sequence,
                    b: thread.behavior,
                    event: thread.event,
                    resumed: thread.resumed,
                    state: thread.state,
                    ran: 0,
                });
                p.drain(vm, base)?;
            }
            p.now = target;
            if let Some(hook) = p.on_time.clone() {
                hook(vm, p.now);
            }
            Ok(())
        })
    }

    /// Runs the work above `base` until none is left (module documentation, "Depth-first without recursion"). An
    /// error drops the rest of that work, as the C++ exception unwinds its recursion.
    fn drain(&mut self, vm: &mut Vm, base: usize) -> Result<(), BehaviorError> {
        while self.work.len() > base {
            let item = self.work.pop().expect("work above base");
            if let Err(e) = self.step(vm, item) {
                self.work.truncate(base);
                return Err(e);
            }
        }
        Ok(())
    }

    /// One unit of work.
    fn step(&mut self, vm: &mut Vm, item: Work) -> Result<(), BehaviorError> {
        match item {
            Work::Start { s, b, delay, event } => {
                if delay > 0.0 {
                    self.park(self.now + delay, s, b, &event, false, None);
                } else {
                    self.work.push(Work::Run {
                        s,
                        b,
                        event,
                        resumed: false,
                        state: None,
                        ran: 0,
                    });
                }
                Ok(())
            }
            Work::ParkAfter { delay, s, b, event } => {
                self.park(self.now + delay, s, b, &event, false, None);
                Ok(())
            }
            Work::Wait {
                wait,
                s,
                b,
                event,
                state,
            } => {
                // Pushed only for a wait that is not negative; the due time is taken now, after the links ran.
                if let Some(due) = kernel_resume_at(self.now, wait) {
                    self.park(due, s, b, &event, true, state);
                }
                Ok(())
            }
            Work::Run {
                s,
                b,
                event,
                resumed,
                state,
                ran,
            } => self.run_behavior(
                vm,
                ThreadAt {
                    s,
                    b,
                    event,
                    resumed,
                    state,
                    ran,
                },
            ),
        }
    }

    /// A2 (module documentation): one behavior of a thread; what follows it is pushed as work.
    fn run_behavior(&mut self, vm: &mut Vm, at: ThreadAt) -> Result<(), BehaviorError> {
        let ThreadAt {
            s,
            b,
            event,
            resumed,
            state,
            ran,
        } = at;
        if ran == THREAD_BEHAVIOR_CAP {
            self.park(self.now, s, b, &event, resumed, state);
            return Ok(());
        }
        if self.budget == 0 {
            let text = format!("behavior execution limit exceeded in {}", self.data.path);
            if !self.errors.contains(&text) {
                self.errors.push(text);
            }
            self.waiting.clear();
            return Ok(());
        }
        self.budget -= 1;
        let id = BehaviorId {
            sequence: s,
            index: b,
        };
        let (class, name, length, default_output) = {
            let behavior = self.behavior(id);
            (
                behavior.class.clone(),
                behavior.name.clone(),
                behavior.length,
                behavior.default_output,
            )
        };
        self.trace.push(format!("{event} -> {name}"));
        let handler = match self.handler_for(vm, &class) {
            Ok(h) => h,
            Err(n) => {
                self.errors
                    .push(format!("unsupported behavior class {class} ({name})"));
                self.not_yet.push(n);
                return Ok(());
            }
        };
        self.run = RunInfo {
            initial_run: !resumed,
            has_linked_outputs: length > 0,
            wait: -1.0,
            state,
        };
        self.bind_context_inputs(vm, id)?;
        let mut ids = match handler {
            Handler::Run(f) => f(vm, self, id, &event)?,
            Handler::Boundary { describe, outputs } => {
                self.boundary.push(format!("{event} -> {class}:{name}"));
                let fields = match describe {
                    Some(d) => d(vm, self, id)?,
                    None => DetMap::new(),
                };
                self.boundary_calls.push(BoundaryCall {
                    event: event.clone(),
                    class: class.clone(),
                    name: name.clone(),
                    sequence: self.data.sequences[s].name.clone(),
                    fields,
                });
                outputs
            }
        };
        self.clear_context_inputs(vm, id)?;
        let wait = self.run.wait;
        let state = self.run.state.take();
        if default_output {
            ids.push(-1);
        }
        let behavior = self.behavior(id);
        let links = &self.data.sequences[s].links[behavior.start..behavior.start + behavior.length];
        let selected: Vec<_> = ids
            .iter()
            .flat_map(|&want| links.iter().filter(move |l| l.id == want).copied())
            .collect();
        // Work runs last pushed first, so what must happen last is pushed first.
        if kernel_resume_at(self.now, wait).is_some() {
            // Latent: every selected link starts a new thread, in order, then this one waits.
            self.work.push(Work::Wait {
                wait,
                s,
                b,
                event: event.clone(),
                state,
            });
            for link in selected.iter().rev() {
                self.work.push(Work::Start {
                    s,
                    b: link.behavior,
                    delay: link.delay,
                    event: event.clone(),
                });
            }
            return Ok(());
        }
        let Some((first, rest)) = selected.split_first() else {
            return Ok(());
        };
        // The first selected link continues this thread once the others have started (each depth-first).
        if first.delay > 0.0 {
            self.work.push(Work::ParkAfter {
                delay: first.delay,
                s,
                b: first.behavior,
                event: event.clone(),
            });
        } else {
            self.work.push(Work::Run {
                s,
                b: first.behavior,
                event: event.clone(),
                resumed: false,
                state: None,
                ran: ran + 1,
            });
        }
        for link in rest.iter().rev() {
            self.work.push(Work::Start {
                s,
                b: link.behavior,
                delay: link.delay,
                event: event.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;
