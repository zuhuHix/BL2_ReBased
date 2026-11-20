//! Owner: lane IMPL-GEARBOXMIND-QUERYINTERFACE. The read-only `QueryInterface` dispatch and plain answer, for Off
//! rules that must not write, load, fill a cache or draw along the chain they reach (NATIVE_GEARBOXMIND_QUERYINTERFACE.md
//! section 5, UNVERIFIED in the running game). AI-assisted (Claude Code).
//!
//! - [`Vm::plain_interface_read_only`]: the plain `Object` answer over interface-table entries resolved already
//!   ([`ow_object::reflect::CodeIndex::implements_resolved_chain`]); an entry not resolved yet is `NotYet { NotRead }`, never
//!   the caching [`Vm::implements`].
//! - [`Vm::query_interface_read_only`]: the dispatch: the (object, interface) guard and the depth cap of
//!   [`Vm::query_interface`], an unloaded stand-in refused `NotRead` before it is asked, then the nearest registered
//!   rule on the class chain: a read-only one runs; a rule with no read-only form is `NotYet { NotPorted }`; with none,
//!   the read-only plain answer.
//! - [`Vm::resolve_interface_tables`]: the set-up step that resolves a class chain's entries (a load: the harness logs
//!   it).
//!
//! Nothing here takes `&mut Vm` except registration and the set-up resolver.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef};

use super::{InterfaceAsking, InterfaceRuleFn, MAX_INTERFACE_DEPTH, ReadOnlyInterfaceRuleFn, Vm};
use crate::error::VmError;

/// The record of the read-only rules.
pub const READ_ONLY_RECORD: &str = "docs/verification/NATIVE_GEARBOXMIND_QUERYINTERFACE.md";

/// The record of the read-only dispatch's **own** refusals (review r2 of #648): an unloaded receiver, a rule with no
/// read-only form, an interface-table entry not resolved yet, and a world with no code layer. Each is a place where
/// the ordinary dispatch may load or resolve and answer, so a caller that asks read-only ahead of the ordinary path
/// can tell them from a rule's own refusal, which carries its rule's record.
pub const DISPATCH_RECORD: &str =
    "docs/verification/NATIVE_GEARBOXMIND_QUERYINTERFACE.md, section 5 (the read-only dispatch's own refusals)";

impl Vm {
    /// Binds `class`'s read-only `QueryInterface` answer, and the same rule as its ordinary answer
    /// ([`Vm::register_interface_rule`]), which runs it with the running chain's questions as its guard. A class with
    /// either form registered already is refused.
    pub fn register_read_only_interface_rule(&mut self, class: ClassId, f: ReadOnlyInterfaceRuleFn) -> Result<(), VmError> {
        if self.read_only_interface_rules.contains_key(&class) {
            return Err(VmError::Registration {
                key: self.space.classes.layout(class).path().to_string(),
                reason: "duplicate read-only interface rule registration",
            });
        }
        let run = f.clone();
        let ordinary: InterfaceRuleFn = Rc::new(move |vm: &mut Vm, this: ObjRef, iface: ClassId| {
            // The ordinary dispatch pushed (this, iface) already, so the chain's questions guard the read-only one.
            let mut asking = InterfaceAsking { asking: vm.interface_asking.clone(), marks: Vec::new() };
            refuse_unloaded(vm, this)?;
            run(vm, &mut asking, this, iface)
        });
        self.register_interface_rule(class, ordinary)?;
        self.read_only_interface_rules.insert(class, f);
        Ok(())
    }

    /// Binds only the read-only form of `class`'s answer, beside an ordinary answer registered on its own (a rule whose
    /// ordinary form does more than read, kept as it is). A class with a read-only form already is refused.
    pub fn register_read_only_form(&mut self, class: ClassId, f: ReadOnlyInterfaceRuleFn) -> Result<(), VmError> {
        if self.read_only_interface_rules.contains_key(&class) {
            return Err(VmError::Registration {
                key: self.space.classes.layout(class).path().to_string(),
                reason: "duplicate read-only interface rule registration",
            });
        }
        self.read_only_interface_rules.insert(class, f);
        Ok(())
    }

    /// Whether `class` itself (not a parent) has a read-only `QueryInterface` form registered.
    pub fn has_read_only_interface_rule(&self, class: ClassId) -> bool {
        self.read_only_interface_rules.contains_key(&class)
    }

    /// The plain `Object` answer read-only: `this` when its class is `iface` or derives from it, or when a resolved
    /// interface-table entry on its chain is `iface` or derives from it. Refusals, each changing nothing:
    /// - a world whose code layer holds no class at all: `NotYet { LayerAbsent }` (a class defined by hand, with no
    ///   package export behind it, implements nothing, as [`Vm::implements`] answers it);
    /// - an entry not resolved yet, when no resolved one answers yes: `NotYet { NotRead }`;
    /// - a "no" through an engine-native stand-in class: `NotYet`, as [`Vm::plain_interface`] answers it.
    pub fn plain_interface_read_only(&self, this: ObjRef, iface: ClassId) -> Result<Option<ObjRef>, VmError> {
        let own = self.space.heap.get(this)?.class;
        if self.space.classes.is_child_of(own, iface) {
            return Ok(Some(this));
        }
        let code = self.defs.code();
        if code.classes().next().is_none() {
            return Err(NotYet::layer_absent(
                format!(
                    "the interface table of {}: the world has no code layer (no class export is loaded)",
                    self.path_of(own)
                ),
                "the code layer (package class exports)",
                DISPATCH_RECORD,
            )
            .into());
        }
        let (Some(c), Some(i)) = (code.key_of(own), code.key_of(iface)) else {
            // A class with no package export behind it (one defined by hand) implements nothing, as `Vm::implements`
            // answers it.
            crate::interp::is_child(self, own, iface)?;
            return Ok(None);
        };
        match code.implements_resolved_chain(c, i) {
            Ok(true) => Ok(Some(this)),
            Ok(false) => {
                crate::interp::is_child(self, own, iface)?;
                Ok(None)
            }
            Err(at) => Err(NotYet::not_read(
                format!(
                    "the interface table of {} has an entry not resolved at set-up (resolving it is a load)",
                    code.class(at).path
                ),
                DISPATCH_RECORD,
            )
            .into()),
        }
    }

    /// `this`'s answer to `QueryInterface(iface)`, read-only (module notes): none when the question is being answered
    /// further up `asking`'s chain; an error past [`MAX_INTERFACE_DEPTH`] objects; `NotYet { NotRead }` for an
    /// unloaded stand-in; else the nearest rule on the class chain, read-only, or `NotYet { NotPorted }` when that
    /// rule has no read-only form; with no rule, [`Vm::plain_interface_read_only`].
    pub fn query_interface_read_only(
        &self,
        asking: &mut InterfaceAsking,
        this: ObjRef,
        iface: ClassId,
    ) -> Result<Option<ObjRef>, VmError> {
        if asking.asking.contains(&(this, iface)) {
            return Ok(None);
        }
        if asking.asking.len() >= MAX_INTERFACE_DEPTH {
            return Err(VmError::Script {
                function: "Object.QueryInterface".to_string(),
                message: format!("the question went through more than {MAX_INTERFACE_DEPTH} answering objects"),
            });
        }
        refuse_unloaded(self, this)?;
        let class = self.space.heap.get(this)?.class;
        let mut cursor = Some(class);
        let mut rule = None;
        while let Some(c) = cursor {
            if let Some(f) = self.read_only_interface_rules.get(&c) {
                rule = Some(f.clone());
                break;
            }
            if self.interface_rules.contains_key(&c) {
                return Err(NotYet::not_ported(
                    format!("{}'s QueryInterface rule is not ported read-only", self.path_of(c)),
                    DISPATCH_RECORD,
                )
                .into());
            }
            cursor = self.space.classes.layout(c).super_class();
        }
        let Some(rule) = rule else {
            return self.plain_interface_read_only(this, iface);
        };
        asking.asking.push((this, iface));
        let answer = rule(self, asking, this, iface);
        asking.asking.pop();
        answer
    }

    /// Resolves the interface-table entries of `class`'s chain not resolved yet (a set-up step: a load, which the
    /// caller logs). Returns how many it resolved; a class the code layer does not hold resolves none.
    pub fn resolve_interface_tables(&mut self, class: ClassId) -> usize {
        let (code, store) = self.defs.code_mut();
        code.key_of(class).map_or(0, |k| code.resolve_interfaces(store, k))
    }

    /// How many interface-table entries of `class`'s chain are not resolved yet (read only; 0 for a class the code
    /// layer does not hold).
    pub fn unresolved_interface_entries(&self, class: ClassId) -> usize {
        let code = self.defs.code();
        code.key_of(class).map_or(0, |k| code.unresolved_interfaces(k))
    }

    fn path_of(&self, class: ClassId) -> String {
        self.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().to_string())
    }
}

/// `NotYet { NotRead }` for an object the store has not loaded (an unloaded stand-in, or one whose load failed),
/// before anything is asked of it.
fn refuse_unloaded(vm: &Vm, this: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, this) {
        let class = vm.space.heap.get(this)?.class;
        return Err(NotYet::not_read(
            format!(
                "QueryInterface asked of an unloaded {} ({}): reading it is a load",
                vm.path_of(class),
                vm.space.names.text(vm.space.heap.get(this)?.name)
            ),
            DISPATCH_RECORD,
        )
        .into());
    }
    Ok(())
}

// Tested through `ow-combat`'s `pawn_interface::read_only` tests, on decoded invented classes.
