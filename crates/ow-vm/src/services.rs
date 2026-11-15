//! Owner: lane W2-VMAPI. The VM's services (architecture sections 3.8 and 3.9).
//!
//! State that belongs to a crate above the VM (the world's balance state, the inventory manager's tables, a native
//! pack's bookkeeping) is a **service**: one value per type, held as `Rc<RefCell<T>>` in a map keyed by `TypeId`, so
//! that everything a native needs is reachable from `&mut Vm` and no native needs a `&mut World`. The one upward
//! hook the VM itself calls, [`crate::api::WorldHooks`], has its own slot ([`Services::hooks`]) because it is held as
//! a trait object.
//!
//! **The borrow rule** (the review bar's first check for every native): a native borrows a service only for a short
//! scope (`vm.service::<T>()` clones the `Rc` out, then `borrow_mut()` for the few lines that need it) and **never
//! holds a borrow across `NativeCall::call_script`**. The VM enforces it: a nested call made while any service is
//! borrowed fails with [`VmError::ServiceBorrowed`] instead of panicking later inside the callee.
//!
//! Nothing iterates this map for output, so the `TypeId` order (which differs between builds) never reaches a result
//! (architecture N7).

use std::any::{Any, TypeId, type_name};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::api::WorldHooks;
use crate::error::VmError;

/// Says whether a service cell is borrowed right now, without knowing its type.
trait BorrowProbe {
    fn borrowed(&self) -> bool;
}

impl<T: ?Sized> BorrowProbe for RefCell<T> {
    fn borrowed(&self) -> bool {
        self.try_borrow_mut().is_err()
    }
}

struct Entry {
    /// The `Rc<RefCell<T>>`, as `Any` for the typed lookup.
    any: Rc<dyn Any>,
    /// The same cell, for the borrow check.
    probe: Rc<dyn BorrowProbe>,
    name: &'static str,
}

/// The service map of one VM. `!Send`, as the VM is (decision D18).
#[derive(Default)]
pub struct Services {
    map: BTreeMap<TypeId, Entry>,
    hooks: Option<Rc<RefCell<dyn WorldHooks>>>,
}

impl std::fmt::Debug for Services {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.map.values().map(|e| e.name).collect();
        f.debug_struct("Services")
            .field("services", &names)
            .field("hooks", &self.hooks.is_some())
            .finish()
    }
}

impl Services {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the service of type `T` and returns its cell. A second service of the same type is refused (two packs
    /// each thinking they own the state is a wiring bug, not something to resolve by order).
    pub fn add<T: 'static>(&mut self, value: T) -> Result<Rc<RefCell<T>>, VmError> {
        let id = TypeId::of::<T>();
        if self.map.contains_key(&id) {
            return Err(VmError::Registration {
                key: type_name::<T>().to_string(),
                reason: "a service of this type is already added",
            });
        }
        let cell = Rc::new(RefCell::new(value));
        self.map.insert(
            id,
            Entry {
                any: cell.clone(),
                probe: cell.clone(),
                name: type_name::<T>(),
            },
        );
        Ok(cell)
    }

    /// The service of type `T`, if one was added. The `Rc` is a clone: holding it borrows nothing.
    pub fn get<T: 'static>(&self) -> Option<Rc<RefCell<T>>> {
        let entry = self.map.get(&TypeId::of::<T>())?;
        entry.any.clone().downcast::<RefCell<T>>().ok()
    }

    /// Sets the world hooks (`ow-world` does this once). A second set is refused.
    pub fn set_hooks(&mut self, hooks: Rc<RefCell<dyn WorldHooks>>) -> Result<(), VmError> {
        if self.hooks.is_some() {
            return Err(VmError::Registration {
                key: "WorldHooks".to_string(),
                reason: "the world hooks are already set",
            });
        }
        self.hooks = Some(hooks);
        Ok(())
    }

    /// The world hooks, cloned out (so no borrow of this map is live while they run). `None` before `ow-world`.
    pub fn hooks(&self) -> Option<Rc<RefCell<dyn WorldHooks>>> {
        self.hooks.clone()
    }

    /// `Err(ServiceBorrowed)` naming the first service (in map order) or the hooks that a borrow still holds.
    pub fn check_none_borrowed(&self) -> Result<(), VmError> {
        if let Some(entry) = self.map.values().find(|e| e.probe.borrowed()) {
            return Err(VmError::ServiceBorrowed(entry.name));
        }
        if self.hooks.as_ref().is_some_and(|h| h.borrowed()) {
            return Err(VmError::ServiceBorrowed("WorldHooks"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Counter(u32);
    struct Other;

    #[test]
    fn typed_lookup_and_duplicates() {
        let mut s = Services::new();
        assert!(s.get::<Counter>().is_none());
        let cell = s.add(Counter(1)).unwrap();
        cell.borrow_mut().0 += 1;
        assert_eq!(*s.get::<Counter>().unwrap().borrow(), Counter(2));
        assert!(s.get::<Other>().is_none());
        assert!(matches!(
            s.add(Counter(9)),
            Err(VmError::Registration { .. })
        ));
        // The refused add changed nothing.
        assert_eq!(*s.get::<Counter>().unwrap().borrow(), Counter(2));
    }

    #[test]
    fn a_live_borrow_is_reported() {
        let mut s = Services::new();
        s.add(Other).unwrap();
        let cell = s.add(Counter(0)).unwrap();
        assert_eq!(s.check_none_borrowed(), Ok(()));
        let guard = cell.borrow();
        assert!(matches!(
            s.check_none_borrowed(),
            Err(VmError::ServiceBorrowed(name)) if name.ends_with("Counter")
        ));
        drop(guard);
        assert_eq!(s.check_none_borrowed(), Ok(()));
    }
}
