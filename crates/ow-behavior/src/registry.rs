//! Owner: lane W5-BEHAVIOR. The handler registry: what runs a behavior of each class (architecture section 3.9,
//! contract "ow-behavior handler registry", section 7). AI-assisted (Claude Code).
//!
//! A behavior runs through the handler registered for its exact class path (`Package.Class`, compared as the C++
//! `std::unordered_map<std::string, Handler>` compares: exactly). Crates above this one register the classes they own
//! from their `NativePack::register` (`ow-combat`'s damage and loot-spawn behaviors, `ow-skills`, `ow-mission`), into
//! the VM's [`Registry`] service ([`register`]); this crate registers the handlers that need only the world
//! (`crate::handlers::Pack`). A [`crate::kernel::Provider`] may also hold handlers of its own, which it consults first
//! (the C++ registers per provider; the slice and the CLI do so).
//!
//! **An unregistered class is `NotYet`.** [`Registry::get`] answers `NotYet { NotPorted }` naming the class; the
//! kernel then records the C++ error text (`unsupported behavior class ...`) and stops that branch, as
//! `src/behavior.cpp` does. Nothing is run in its place.
//!
//! **Two kinds of handler** ([`Handler`]):
//! - [`Handler::Run`]: a function that runs the behavior and returns the output ids it selected
//!   (`BehaviorKernel.ActivateBehaviorOutputLink` calls, in call order, duplicates kept). It reads inputs through the
//!   provider (`object_input`, `int_input`), may record errors, and may make the behavior latent through the
//!   provider's run information (`crate::kernel::RunInfo::wait`).
//! - [`Handler::Boundary`]: the class acts on the world and has no binding yet. Each execution is listed in the
//!   provider's `boundary` and `boundary_calls` (not run, not an error, never counted as implemented); it selects the
//!   given outputs (the ids the class's installed script selects before it needs the host's answer; usually none), so
//!   otherwise only its default (-1) links are followed, and only when it supports the default output.
//!
//! **What a handler is not given yet** (contract note for wave 6). [`HandlerFn`] receives the VM, the provider, the
//! behavior and the event, but neither the consumer (the object the provider is registered on, the script's
//! `SelfObject`) nor the context object the behavior is being applied to (G2 runs a behavior once per context). The
//! kernel does not hold either yet. When W6 needs them, the route is additive: a new field on the `#[non_exhaustive]`
//! [`crate::kernel::RunInfo`] (read through `Provider::run_info`), or a `Provider` accessor, set by the kernel before
//! each call. Changing the `HandlerFn` signature is not that route: it breaks every registered handler.

use std::cell::RefCell;
use std::rc::Rc;

use ow_core::{DetMap, FixedHashMap, NotYet};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::kernel::{BehaviorId, Provider};
use crate::provider::BehaviorError;

/// The record of the registry rule.
pub const RECORD: &str = "docs/rust-port/ARCHITECTURE.md section 3.9 (behavior handler registry)";

/// Runs one behavior: the VM, the provider it belongs to, the behavior, and the event its thread started from.
/// Returns the selected output ids.
pub type HandlerFn =
    Rc<dyn Fn(&mut Vm, &mut Provider, BehaviorId, &str) -> Result<Vec<i32>, BehaviorError>>;

/// The decoded fields a boundary behavior reports (`BoundaryCall::fields`), keyed in key order as the C++
/// `std::map`.
pub type DescribeFn =
    Rc<dyn Fn(&mut Vm, &mut Provider, BehaviorId) -> Result<DetMap<String, String>, BehaviorError>>;

/// What runs a behavior class (module documentation).
#[non_exhaustive]
#[derive(Clone)]
pub enum Handler {
    Run(HandlerFn),
    Boundary {
        describe: Option<DescribeFn>,
        outputs: Vec<i32>,
    },
}

impl Handler {
    /// A [`Handler::Run`] of `f`.
    pub fn run(
        f: impl Fn(&mut Vm, &mut Provider, BehaviorId, &str) -> Result<Vec<i32>, BehaviorError>
        + 'static,
    ) -> Self {
        Handler::Run(Rc::new(f))
    }

    /// A [`Handler::Boundary`] with no description and no outputs (the C++ `reportAtBoundary(cls)`).
    pub fn boundary() -> Self {
        Handler::Boundary {
            describe: None,
            outputs: Vec::new(),
        }
    }
}

impl std::fmt::Debug for Handler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Handler::Run(_) => f.write_str("Handler::Run"),
            Handler::Boundary { describe, outputs } => f
                .debug_struct("Handler::Boundary")
                .field("describe", &describe.is_some())
                .field("outputs", outputs)
                .finish(),
        }
    }
}

/// Handlers by exact class path. Lookups only (N7: no iteration over the hash map).
#[derive(Debug, Default, Clone)]
pub struct Registry {
    handlers: FixedHashMap<String, Handler>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the handler of `class`, replacing any earlier one (the C++ `handle` / `reportAtBoundary`, per provider).
    pub fn set(&mut self, class: &str, handler: Handler) {
        self.handlers.insert(class.to_string(), handler);
    }

    /// Adds the handler of `class`; a class registered twice is refused (two packs claiming one class is a wiring
    /// bug, as a native registered twice is).
    pub fn add(&mut self, class: &str, handler: Handler) -> Result<(), VmError> {
        if self.handlers.contains_key(class) {
            return Err(VmError::Registration {
                key: class.to_string(),
                reason: "a behavior handler for this class is already registered",
            });
        }
        self.handlers.insert(class.to_string(), handler);
        Ok(())
    }

    /// Whether `class` has a handler.
    pub fn contains(&self, class: &str) -> bool {
        self.handlers.contains_key(class)
    }

    /// The handler of `class`, or `NotYet { NotPorted }` naming the class (module documentation).
    pub fn get(&self, class: &str) -> Result<Handler, NotYet> {
        self.handlers
            .get(class)
            .cloned()
            .ok_or_else(|| NotYet::not_ported(format!("behavior class {class}"), RECORD))
    }
}

/// The VM's registry service, added empty on first use.
pub fn install(vm: &mut Vm) -> Result<Rc<RefCell<Registry>>, VmError> {
    match vm.service::<Registry>() {
        Some(r) => Ok(r),
        None => vm.add_service(Registry::new()),
    }
}

/// Registers `handler` for `class` in the VM's registry ([`Registry::add`]: a second registration is refused).
pub fn register(vm: &mut Vm, class: &str, handler: Handler) -> Result<(), VmError> {
    let registry = install(vm)?;
    let mut registry = registry
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("ow_behavior::registry::Registry"))?;
    registry.add(class, handler)
}

/// The VM registry's handler of `class`, or `NotYet` when the VM has no registry or the class no handler.
pub fn lookup(vm: &Vm, class: &str) -> Result<Handler, NotYet> {
    match vm.service::<Registry>() {
        Some(r) => match r.try_borrow() {
            Ok(r) => r.get(class),
            Err(_) => Err(NotYet::not_ported(
                format!("behavior class {class}"),
                RECORD,
            )),
        },
        None => Err(NotYet::not_ported(
            format!("behavior class {class}"),
            RECORD,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_core::Missing;

    #[test]
    fn an_unregistered_class_is_not_yet_not_ported() {
        let r = Registry::new();
        let n = r.get("WillowGame.Behavior_Nothing").unwrap_err();
        assert_eq!(n.missing, Missing::NotPorted);
        assert!(n.what.contains("WillowGame.Behavior_Nothing"));
    }

    #[test]
    fn set_replaces_and_add_refuses_a_second_registration() {
        let mut r = Registry::new();
        r.set("A.B", Handler::boundary());
        r.set(
            "A.B",
            Handler::Boundary {
                describe: None,
                outputs: vec![0],
            },
        );
        match r.get("A.B").unwrap() {
            Handler::Boundary { outputs, .. } => assert_eq!(outputs, vec![0]),
            other => panic!("{other:?}"),
        }
        assert!(r.add("A.B", Handler::boundary()).is_err());
        assert!(r.add("A.C", Handler::boundary()).is_ok());
        // Exact class paths, as the C++ map compares them.
        assert!(!r.contains("a.c"));
    }
}
