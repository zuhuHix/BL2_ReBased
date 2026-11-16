//! Owner: lane W5-WORLD. The world side of the VM's attribute world: the [`WorldHooks`] implementation behind
//! `Vm`'s `AttrWorld` (architecture sections 3.8 and 3.9). AI-assisted (Claude Code).
//!
//! **What it answers.**
//! - `globals` and `game_info`: the objects the world was given ([`WorldQuery::set_globals`],
//!   [`WorldQuery::set_game_info`]). The running game always has both (`progression.cpp:84,87`), so an unset one is
//!   `NotYet { LayerAbsent }` naming the map load that makes it, never a silent None.
//! - The game info falls back to the level's `WorldInfo.Game` (lane FIX-ATTR-CONTEXTS, decision 2026-11-16-49): with
//!   none set, a query that follows the actor list ([`WorldQuery::follow_world_info`], which `World::new` calls)
//!   answers the `Game` of the `WorldInfo` placed there. That is the engine's game-info object, which the C++
//!   combat world also makes there (`CombatSystem::enableDeaths`, `src/combat_death.cpp`; in Rust
//!   `ow_combat::death::enable_deaths`). A None `Game`, or no `WorldInfo` placed, is still the `NotYet` above.
//!   Which object the game's own `GameInfoAttributeContextResolver` answers is NATIVE_ATTRIBUTES.md section 5's
//!   "the game info" (confidence medium); that it is `WorldInfo.Game` is UNVERIFIED in the running game.
//! - `engine_query`: the handler registered for the query's kind ([`WorldQuery::register`]). The packs that own the
//!   engine state register them: resource pools (W5-POOLS, this crate), inventory and equipped items (W5-EQUIP,
//!   `ow-inventory`), player skills (W6-SKILLS), the game replication info (the map load, not built yet; added by
//!   FIX-PLAYTHROUGH-OFF). A kind with no handler is `NotYet { LayerAbsent }` naming that layer.
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
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{Vm, WorldHooks};
use ow_vm::error::VmError;

use ow_attr::context_resolvers::{RECORD_PLAYER_CONTROLLER, player_controller_of};

use crate::actors::Actors;

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
    GameReplicationInfo,
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
            EngineQuery::GameReplicationInfo => QueryKind::GameReplicationInfo,
            _ => return None,
        })
    }

    /// The layer that answers this kind, for the `NotYet` of a kind with no handler.
    fn layer(self) -> &'static str {
        match self {
            QueryKind::ResourcePool => "ow-world pools (lane W5-POOLS)",
            QueryKind::EquippedInventory | QueryKind::Inventory => "ow-inventory (lane W5-EQUIP)",
            QueryKind::PlayerSkill => "ow-skills (lane W6-SKILLS)",
            QueryKind::GameReplicationInfo => {
                "the map load that makes the game replication info (a handler for this query)"
            }
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
    /// The actor list whose placed `WorldInfo`'s `Game` is the game info when none is set.
    actors: Option<Rc<RefCell<Actors>>>,
    handlers: DetMap<QueryKind, QueryFn>,
}

impl std::fmt::Debug for WorldQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldQuery")
            .field("globals", &self.globals)
            .field("game_info", &self.game_info)
            .field("follows_world_info", &self.actors.is_some())
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

    /// The game info object, made by the map load. It wins over [`WorldQuery::follow_world_info`].
    pub fn set_game_info(&mut self, o: ObjRef) {
        self.game_info = Some(o);
    }

    /// With no game info set, answer the `Game` of the `WorldInfo` placed in `actors` (module documentation).
    pub fn follow_world_info(&mut self, actors: Rc<RefCell<Actors>>) {
        self.actors = Some(actors);
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

    fn game_info(&mut self, w: WorldParts<'_>) -> Result<ObjRef, AttrError> {
        let (set, actors) =
            self.read("the game info object", |s| (s.game_info, s.actors.clone()))?;
        if let Some(game) = set {
            return Ok(game);
        }
        let world_info = match actors {
            Some(actors) => actors
                .try_borrow()
                .map_err(|_| {
                    AttrError::World("the game info object: the actor list is borrowed".into())
                })?
                .world_info(),
            None => None,
        };
        match world_info {
            Some(info) => world_info_game(w.space, info)?.ok_or_else(|| {
                NotYet::layer_absent(
                    "the game info object of this world (its WorldInfo's Game is None)",
                    "the map load that spawns the game info into WorldInfo.Game (or WorldQuery::set_game_info)",
                    RECORD,
                )
                .into()
            }),
            None => Err(NotYet::layer_absent(
                "the game info object of this world",
                "the map load that makes it (WorldQuery::set_game_info, or a placed WorldInfo's Game)",
                RECORD,
            )
            .into()),
        }
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

/// The `Game` reference of the `WorldInfo` `info`: `Ok(None)` for a None reference. A class without an object property
/// `Game` is bad data, never a default.
fn world_info_game(space: &ObjectSpace, info: ObjRef) -> Result<Option<ObjRef>, AttrError> {
    let class = space.heap.get(info)?.class;
    let Some(prop) = space.prop(class, "Game") else {
        return Err(AttrError::BadData {
            reason: "the game info object: the placed WorldInfo's class declares no Game".into(),
        });
    };
    match space.read(info, prop.into())? {
        Value::Object(r) => Ok(*r),
        other => Err(AttrError::BadData {
            reason: format!(
                "the game info object: WorldInfo.Game holds {}, not an object reference",
                other.kind_name()
            )
            .into(),
        }),
    }
}

/// Adds the [`WorldQuery`] service to `vm` and sets the hooks over it (once per VM: a second call is refused by
/// `add_service`).
/// The [`QueryKind::PlayerController`] handler (lane FIX-PHASELOCK-CHAIN, decision 2026-11-16-69): the rule of
/// `ow_attr::context_resolvers::player_controller_of` (NATIVE_ITEM_RULES.md: the source if it is an
/// `IPlayerBehavior`, else through `IPlayerMaster`, else None; UNVERIFIED in the running game).
pub fn player_controller_handler() -> QueryFn {
    Rc::new(|q, source, w| {
        if !matches!(q, EngineQuery::PlayerController) {
            return Err(NotYet::not_ported(
                format!("engine query {q:?} sent to the player-controller handler"),
                RECORD_PLAYER_CONTROLLER,
            )
            .into());
        }
        player_controller_of(w, source)
    })
}

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

    /// The player-controller handler runs `player_controller_of` (its cases are tested in `ow-attr`): here, a VM
    /// whose store holds no code package cannot tell an interface (the empty store has no `IPlayerBehavior`, so no
    /// implementer: None), and another query kind sent to it is refused (lane FIX-PHASELOCK-CHAIN).
    #[test]
    fn the_player_controller_handler_runs_the_rule_and_refuses_other_queries() {
        let (mut vm, o) = vm_with_object();
        let handler = player_controller_handler();
        let parts = WorldParts::new(&mut vm.space, Some(&mut vm.defs));
        assert_eq!(handler(EngineQuery::PlayerController, o, parts).unwrap(), None);
        let parts = WorldParts::new(&mut vm.space, Some(&mut vm.defs));
        assert_eq!(
            missing(handler(EngineQuery::GameReplicationInfo, o, parts).unwrap_err()),
            Missing::NotPorted
        );
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

    /// A VM with an invented `WorldInfo` class (an object property `Game`) and a `Thing` class without one, an
    /// installed query service following an actor list, and that list.
    fn vm_with_world_info() -> (
        Vm,
        Rc<RefCell<WorldQuery>>,
        Rc<RefCell<Actors>>,
        ObjRef,
        ObjRef,
        ObjRef,
    ) {
        use ow_object::layout::{PropKind, PropertyDecl};
        let mut space = ObjectSpace::new();
        let actor = space
            .classes
            .define("Invented.Actor", None, Vec::new())
            .unwrap();
        let game_prop = PropertyDecl::new(
            space.names.intern("Game"),
            PropKind::Object { class: actor },
        );
        let info_class = space
            .classes
            .define("Invented.WorldInfo", Some(actor), vec![game_prop])
            .unwrap();
        let info = space
            .alloc(info_class, "WorldInfo_0", None, Origin::Runtime)
            .unwrap();
        let game = space.alloc(actor, "Game_0", None, Origin::Runtime).unwrap();
        let thing = space
            .alloc(actor, "Thing_0", None, Origin::Runtime)
            .unwrap();
        let mut vm = Vm::new(
            space,
            DefStore::new_empty(),
            Rc::new(AttributeSystem::new_empty()),
            ParityMode::Off,
        );
        let q = install(&mut vm).unwrap();
        let actors = Rc::new(RefCell::new(Actors::new(actor)));
        q.borrow_mut().follow_world_info(actors.clone());
        (vm, q, actors, info, game, thing)
    }

    fn set_game(vm: &mut Vm, info: ObjRef, game: Option<ObjRef>) {
        let class = vm.space.heap.get(info).unwrap().class;
        let prop = vm.space.prop(class, "Game").unwrap();
        vm.space
            .write(info, prop.into(), Value::Object(game))
            .unwrap();
    }

    /// Decision 2026-11-16-49: with no game info set, the game info is the placed `WorldInfo`'s `Game`; no
    /// `WorldInfo`, or a None `Game`, is still `NotYet`; a set game info wins.
    #[test]
    fn the_game_info_falls_back_to_the_placed_world_infos_game() {
        let (mut vm, q, actors, info, game, thing) = vm_with_world_info();
        // No WorldInfo placed yet.
        assert!(matches!(
            missing(vm.game_info().unwrap_err()),
            Missing::LayerAbsent(_)
        ));
        actors.borrow_mut().place_world_info(info).unwrap();
        // Placed, with a None Game.
        assert!(matches!(
            missing(vm.game_info().unwrap_err()),
            Missing::LayerAbsent(_)
        ));
        set_game(&mut vm, info, Some(game));
        assert_eq!(vm.game_info().unwrap(), Some(game));
        // The globals do not follow the WorldInfo.
        assert!(matches!(
            missing(vm.globals().unwrap_err()),
            Missing::LayerAbsent(_)
        ));
        // A set game info wins over the WorldInfo's.
        q.borrow_mut().set_game_info(thing);
        assert_eq!(vm.game_info().unwrap(), Some(thing));
    }

    #[test]
    fn a_world_info_without_game_or_a_borrowed_list_is_an_error_not_a_panic() {
        let (mut vm, q, _actors, _info, _game, thing) = vm_with_world_info();
        // A WorldInfo whose class declares no Game is bad data.
        let other = Rc::new(RefCell::new(Actors::new(
            vm.space.heap.get(thing).unwrap().class,
        )));
        other.borrow_mut().place_world_info(thing).unwrap();
        q.borrow_mut().follow_world_info(other.clone());
        assert!(matches!(
            vm.game_info().unwrap_err(),
            AttrError::BadData { .. }
        ));
        // The actor list borrowed elsewhere.
        let held = other.borrow_mut();
        assert!(matches!(vm.game_info().unwrap_err(), AttrError::World(_)));
        drop(held);
        // A query that follows no list answers as before.
        let (mut plain, o) = vm_with_object();
        install(&mut plain).unwrap();
        assert!(matches!(
            missing(plain.game_info().unwrap_err()),
            Missing::LayerAbsent(_)
        ));
        let _ = o;
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
    fn the_game_replication_info_query_waits_for_the_map_load_handler() {
        let (mut vm, o) = vm_with_object();
        let q = install(&mut vm).unwrap();
        // No handler: NotYet naming the map load, never a silent None (which would read as "no replication info").
        match vm
            .engine_query(EngineQuery::GameReplicationInfo, o)
            .unwrap_err()
        {
            AttrError::NotYet(n) => assert_eq!(
                n.missing,
                Missing::LayerAbsent(
                    "the map load that makes the game replication info (a handler for this query)"
                )
            ),
            other => panic!("{other:?}"),
        }
        let gri = o;
        let handler: QueryFn = Rc::new(move |q, _source, _w| match q {
            EngineQuery::GameReplicationInfo => Ok(Some(gri)),
            _ => Ok(None),
        });
        q.borrow_mut()
            .register(QueryKind::GameReplicationInfo, handler)
            .unwrap();
        assert!(q.borrow().has_handler(QueryKind::GameReplicationInfo));
        assert_eq!(
            vm.engine_query(EngineQuery::GameReplicationInfo, o)
                .unwrap(),
            Some(gri)
        );
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
