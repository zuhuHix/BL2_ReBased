//! Owner: lane W5-WORLD. The world side of the VM's attribute world: the [`WorldHooks`] implementation behind
//! `Vm`'s `AttrWorld` (architecture sections 3.8 and 3.9). AI-assisted (Claude Code).
//!
//! **What it answers.**
//! - `globals` and `game_info`: the objects the world was given ([`WorldQuery::set_globals`],
//!   [`WorldQuery::set_game_info`]). The running game always has both (`progression.cpp:84,87`), so an unset one is
//!   `NotYet { LayerAbsent }` naming the map load that makes it, never a silent None.
//! - `engine_query`: the handler registered for the query's kind ([`WorldQuery::register`]). The packs that own the
//!   engine state register them: resource pools (W5-POOLS, this crate), inventory and equipped items (W5-EQUIP,
//!   `ow-inventory`), player skills (W6-SKILLS). A kind with no handler is `NotYet { LayerAbsent }` naming that layer.
//!   The world itself answers none: how the game finds the player controller from a source, or redirects a pawn's
//!   attribute context, is not read here (NATIVE_ATTRIBUTES.md sections 5 and 10).
//!
//! **Borrows.** [`WorldQuery`] is a VM service (`vm.service::<WorldQuery>()`), so a pack registers its handler from
//! its `NativePack::register`. The hooks borrow it only to read a field or clone a handler out; a handler runs with
//! nothing borrowed. A handler gets the object space and the store (`WorldParts`), not the VM, so it cannot run
//! script (the `WorldHooks` contract).

use std::cell::RefCell;
use std::rc::Rc;

use ow_attr::api::{AttrError, EngineQuery, WorldParts};
use ow_core::{DetMap, NotYet};
use ow_object::ids::ObjRef;
use ow_vm::api::{Vm, WorldHooks};
use ow_vm::error::VmError;

/// The record behind the hooks.
pub const RECORD: &str =
    "architecture.md sections 3.8-3.9 (WorldHooks); NATIVE_ATTRIBUTES.md section 5";

/// The kind of an [`EngineQuery`], the key of its handler.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QueryKind {
    ResourcePool,
    EquippedInventory,
    Inventory,
    PlayerController,
    AttributeContextSource,
    PlayerSkill,
}

impl QueryKind {
    /// The kind of `q`; `None` for a query kind added to `ow-attr` after this list.
    pub fn of(q: &EngineQuery) -> Option<QueryKind> {
        Some(match q {
            EngineQuery::ResourcePool { .. } => QueryKind::ResourcePool,
            EngineQuery::EquippedInventory { .. } => QueryKind::EquippedInventory,
            EngineQuery::Inventory { .. } => QueryKind::Inventory,
            EngineQuery::PlayerController => QueryKind::PlayerController,
            EngineQuery::AttributeContextSource => QueryKind::AttributeContextSource,
            EngineQuery::PlayerSkill { .. } => QueryKind::PlayerSkill,
            _ => return None,
        })
    }

    /// The layer that answers this kind, for the `NotYet` of a kind with no handler.
    fn layer(self) -> &'static str {
        match self {
            QueryKind::ResourcePool => "ow-world pools (lane W5-POOLS)",
            QueryKind::EquippedInventory | QueryKind::Inventory => "ow-inventory (lane W5-EQUIP)",
            QueryKind::PlayerSkill => "ow-skills (lane W6-SKILLS)",
            QueryKind::PlayerController | QueryKind::AttributeContextSource => {
                "a handler for this query (the engine rule is not read: NATIVE_ATTRIBUTES.md sections 5, 10)"
            }
        }
    }
}

/// An engine-query handler: `Ok(None)` means it ran and found nothing.
pub type QueryFn =
    Rc<dyn Fn(EngineQuery, ObjRef, WorldParts<'_>) -> Result<Option<ObjRef>, AttrError>>;

/// What the hooks answer from (module documentation). A VM service.
#[derive(Default)]
pub struct WorldQuery {
    globals: Option<ObjRef>,
    game_info: Option<ObjRef>,
    handlers: DetMap<QueryKind, QueryFn>,
}

impl std::fmt::Debug for WorldQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldQuery")
            .field("globals", &self.globals)
            .field("game_info", &self.game_info)
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl WorldQuery {
    pub fn new() -> Self {
        Self::default()
    }

    /// The engine globals object (`WillowGlobals`), made by the map load.
    pub fn set_globals(&mut self, o: ObjRef) {
        self.globals = Some(o);
    }

    /// The game info object, made by the map load.
    pub fn set_game_info(&mut self, o: ObjRef) {
        self.game_info = Some(o);
    }

    pub fn globals(&self) -> Option<ObjRef> {
        self.globals
    }

    pub fn game_info(&self) -> Option<ObjRef> {
        self.game_info
    }

    /// Registers the handler for `kind`; a kind registered twice is refused.
    pub fn register(&mut self, kind: QueryKind, f: QueryFn) -> Result<(), VmError> {
        if self.handlers.contains_key(&kind) {
            return Err(VmError::Registration {
                key: format!("engine query {kind:?}"),
                reason: "a handler for this engine query kind is already registered",
            });
        }
        self.handlers.insert(kind, f);
        Ok(())
    }

    pub fn has_handler(&self, kind: QueryKind) -> bool {
        self.handlers.contains_key(&kind)
    }
}

/// The [`WorldHooks`] the VM holds: a view of the [`WorldQuery`] service.
struct Hooks {
    state: Rc<RefCell<WorldQuery>>,
}

impl Hooks {
    fn read<R>(&self, what: &str, f: impl FnOnce(&WorldQuery) -> R) -> Result<R, AttrError> {
        let state = self.state.try_borrow().map_err(|_| {
            AttrError::World(format!("{what}: the world query service is borrowed"))
        })?;
        Ok(f(&state))
    }
}

impl WorldHooks for Hooks {
    fn globals(&mut self, _w: WorldParts<'_>) -> Result<ObjRef, AttrError> {
        self.read("the engine globals object", WorldQuery::globals)?
            .ok_or_else(|| {
                NotYet::layer_absent(
                    "the engine globals object of this world",
                    "the map load that makes it (WorldQuery::set_globals)",
                    RECORD,
                )
                .into()
            })
    }

    fn game_info(&mut self, _w: WorldParts<'_>) -> Result<ObjRef, AttrError> {
        self.read("the game info object", WorldQuery::game_info)?
            .ok_or_else(|| {
                NotYet::layer_absent(
                    "the game info object of this world",
                    "the map load that makes it (WorldQuery::set_game_info)",
                    RECORD,
                )
                .into()
            })
    }

    fn engine_query(
        &mut self,
        q: EngineQuery,
        source: ObjRef,
        w: WorldParts<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        let Some(kind) = QueryKind::of(&q) else {
            return Err(NotYet::not_ported(
                format!("engine query {q:?} (a kind this world does not list)"),
                RECORD,
            )
            .into());
        };
        let handler = self.read("an engine query", |s| s.handlers.get(&kind).cloned())?;
        match handler {
            Some(f) => f(q, source, w),
            None => Err(NotYet::layer_absent(
                format!("engine query {q:?} from {source:?}"),
                kind.layer(),
                RECORD,
            )
            .into()),
        }
    }
}

/// Adds the [`WorldQuery`] service to `vm` and sets the hooks over it (once per VM: a second call is refused by
/// `add_service`).
pub fn install(vm: &mut Vm) -> Result<Rc<RefCell<WorldQuery>>, VmError> {
    let state = vm.add_service(WorldQuery::new())?;
    vm.set_world_hooks(Rc::new(RefCell::new(Hooks {
        state: state.clone(),
    })))?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_attr::AttributeSystem;
    use ow_attr::api::AttrWorld;
    use ow_core::ParityMode;
    use ow_core::notyet::Missing;
    use ow_defs::DefStore;
    use ow_object::heap::{ObjectSpace, Origin};

    fn vm_with_object() -> (Vm, ObjRef) {
        let mut space = ObjectSpace::new();
        let c = space
            .classes
            .define("Invented.Thing", None, Vec::new())
            .unwrap();
        let o = space.alloc(c, "Thing_0", None, Origin::Runtime).unwrap();
        let vm = Vm::new(
            space,
            DefStore::new_empty(),
            Rc::new(AttributeSystem::new_empty()),
            ParityMode::Off,
        );
        (vm, o)
    }

    fn missing(e: AttrError) -> Missing {
        match e {
            AttrError::NotYet(n) => n.missing,
            other => panic!("expected NotYet, got {other:?}"),
        }
    }

    #[test]
    fn globals_and_game_info_are_not_yet_until_the_map_load_sets_them() {
        let (mut vm, o) = vm_with_object();
        let q = install(&mut vm).unwrap();
        assert!(matches!(
            missing(vm.globals().unwrap_err()),
            Missing::LayerAbsent(_)
        ));
        assert!(matches!(
            missing(vm.game_info().unwrap_err()),
            Missing::LayerAbsent(_)
        ));
        q.borrow_mut().set_globals(o);
        q.borrow_mut().set_game_info(o);
        assert_eq!(vm.globals().unwrap(), o);
        assert_eq!(vm.game_info().unwrap(), Some(o));
    }

    #[test]
    fn engine_queries_go_to_the_registered_handler_or_name_the_missing_layer() {
        let (mut vm, o) = vm_with_object();
        let q = install(&mut vm).unwrap();
        let e = vm
            .engine_query(EngineQuery::ResourcePool { resource: o }, o)
            .unwrap_err();
        match e {
            AttrError::NotYet(n) => assert_eq!(
                n.missing,
                Missing::LayerAbsent("ow-world pools (lane W5-POOLS)")
            ),
            other => panic!("{other:?}"),
        }
        let handler: QueryFn = Rc::new(move |q, source, _w| match q {
            EngineQuery::ResourcePool { resource } if resource == source => Ok(Some(source)),
            _ => Ok(None),
        });
        q.borrow_mut()
            .register(QueryKind::ResourcePool, handler.clone())
            .unwrap();
        assert!(
            q.borrow_mut()
                .register(QueryKind::ResourcePool, handler)
                .is_err(),
            "registered twice"
        );
        assert_eq!(
            vm.engine_query(EngineQuery::ResourcePool { resource: o }, o)
                .unwrap(),
            Some(o)
        );
        assert!(matches!(
            missing(
                vm.engine_query(EngineQuery::PlayerController, o)
                    .unwrap_err()
            ),
            Missing::LayerAbsent(_)
        ));
    }

    #[test]
    fn a_borrowed_query_service_is_an_error_not_a_panic() {
        let (mut vm, o) = vm_with_object();
        let q = install(&mut vm).unwrap();
        q.borrow_mut().set_globals(o);
        let held = q.borrow_mut();
        assert!(matches!(vm.globals().unwrap_err(), AttrError::World(_)));
        drop(held);
        assert_eq!(vm.globals().unwrap(), o);
        assert!(install(&mut vm).is_err(), "a second install is refused");
    }
}
