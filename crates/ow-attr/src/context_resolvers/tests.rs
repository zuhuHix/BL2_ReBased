//! Synthetic-world tests of the context resolvers (AGENTS.md rule 9: invented classes, objects and values; the
//! class and property names are the engine's, as the resolvers read them).
//!
//! Chains are walked by the evaluator itself (`eval.rs`, W3-ATTREVAL): [`walk`] writes the chain into a synthetic
//! attribute definition and calls [`AttributeSystem::resolve_context`]; [`walk_with_override`] calls
//! [`AttributeSystem::value_of`] (an empty value chain) for its override-first rule and keeps the resolved context.
//! The rules (NATIVE_ATTRIBUTES.md section 1): each resolver gets the previous answer, the first `None` ends the
//! walk, an empty chain is `None`, a null entry is skipped, and the override source is tried first.

use ow_core::rand::{CrtStream, Rand15};
use ow_core::{Missing, ObjectPath};
use ow_defs::DefStore;
use ow_object::heap::{HeapError, ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;

use super::*;
use crate::api::{AttrWorld, ContextResolver, EngineQuery, GlobalSlots, NumForms, WorldParts};

// ---------------------------------------------------------------------------------------------------------------
// The world
// ---------------------------------------------------------------------------------------------------------------

/// A hand-built object space, the globals and game info objects, an engine-query table, and optionally a store.
struct World {
    space: ObjectSpace,
    defs: Option<DefStore>,
    globals: Option<ObjRef>,
    /// The attribute definition the tests write their chains into (the resolvers do not read it).
    attr: ObjRef,
    game_info: Option<ObjRef>,
    /// The engine's answers; a query not in the table answers `Ok(None)` (the lookup ran and found nothing).
    queries: Vec<((EngineQuery, ObjRef), ObjRef)>,
    slots: GlobalSlots,
}

impl AttrWorld for World {
    fn parts(&mut self) -> WorldParts<'_> {
        WorldParts::new(&mut self.space, self.defs.as_mut())
    }
    fn globals(&mut self) -> Result<ObjRef, AttrError> {
        self.globals
            .ok_or_else(|| NotYet::layer_absent("globals", "ow-world", "test world").into())
    }
    fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
        Ok(self.game_info)
    }
    fn engine_query(
        &mut self,
        q: EngineQuery,
        source: ObjRef,
    ) -> Result<Option<ObjRef>, AttrError> {
        Ok(self
            .queries
            .iter()
            .find(|(k, _)| *k == (q, source))
            .map(|(_, v)| *v))
    }
    fn global_slots(&mut self) -> &mut GlobalSlots {
        &mut self.slots
    }
    fn attribute_changed(&mut self, _: ObjRef, _: PropId, _: NumForms) -> Result<(), AttrError> {
        Ok(())
    }
    fn rng(&mut self, _s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        Err(NotYet::layer_absent("a random source", "ow-world", "test world").into())
    }
}

/// The classes of the synthetic world.
struct Classes {
    object: ClassId,
    actor: ClassId,
    pawn: ClassId,
    ai_pawn: ClassId,
    vehicle: ClassId,
    controller: ClassId,
    player_controller: ClassId,
    mind: ClassId,
    pri: ClassId,
    weapon: ClassId,
    /// A pawn class without `OffHandWeapon`.
    bare_pawn: ClassId,
}

fn obj_kind(class: ClassId) -> PropKind {
    PropKind::Object { class }
}

fn build_classes(space: &mut ObjectSpace) -> Classes {
    let c = &mut space.classes;
    let n = &mut space.names;
    let object = c.define("Core.Object", None, vec![]).unwrap();
    let pawn_id = c.declare("Engine.Pawn").unwrap();
    let controller_id = c.declare("Engine.Controller").unwrap();
    let pri_id = c.declare("Engine.PlayerReplicationInfo").unwrap();
    let weapon_id = c.declare("Engine.Weapon").unwrap();
    let mind_id = c.declare("WillowGame.WillowMind").unwrap();
    let mut decl = |name: &str, kind| PropertyDecl::new(n.intern(name), kind);
    let instigator = decl("Instigator", obj_kind(pawn_id));
    let pawn_props = vec![
        decl("Controller", obj_kind(controller_id)),
        decl("PlayerReplicationInfo", obj_kind(pri_id)),
        decl("Weapon", obj_kind(weapon_id)),
        decl("DrivenVehicle", obj_kind(pawn_id)),
    ];
    let off_hand = decl("OffHandWeapon", obj_kind(weapon_id));
    let my_mind = decl("MyWillowMind", obj_kind(mind_id));
    let controller_props = vec![
        decl("Pawn", obj_kind(pawn_id)),
        decl("PlayerReplicationInfo", obj_kind(pri_id)),
    ];
    let b_off_hand = decl("bOffHand", PropKind::Bool);
    let actor = c
        .define("Engine.Actor", Some(object), vec![instigator])
        .unwrap();
    let pawn = c.define("Engine.Pawn", Some(actor), pawn_props).unwrap();
    let bare_pawn = c.define("Test.BarePawn", Some(pawn), vec![]).unwrap();
    let willow_pawn = c
        .define("WillowGame.WillowPawn", Some(pawn), vec![off_hand])
        .unwrap();
    let ai_pawn = c
        .define("WillowGame.WillowAIPawn", Some(willow_pawn), vec![my_mind])
        .unwrap();
    let vehicle = c.define("Engine.Vehicle", Some(pawn), vec![]).unwrap();
    let controller = c
        .define("Engine.Controller", Some(actor), controller_props)
        .unwrap();
    let player_controller = c
        .define("Engine.PlayerController", Some(controller), vec![])
        .unwrap();
    let mind = c
        .define("WillowGame.WillowMind", Some(controller), vec![])
        .unwrap();
    let pri = c
        .define("Engine.PlayerReplicationInfo", Some(actor), vec![])
        .unwrap();
    let weapon = c
        .define("Engine.Weapon", Some(actor), vec![b_off_hand])
        .unwrap();
    Classes {
        object,
        actor,
        pawn: willow_pawn,
        ai_pawn,
        vehicle,
        controller,
        player_controller,
        mind,
        pri,
        weapon,
        bare_pawn,
    }
}

/// Defines the resolver class at `path` with the given own declarations.
fn resolver_class(
    space: &mut ObjectSpace,
    object: ClassId,
    path: &str,
    props: Vec<(&str, PropKind)>,
) -> ClassId {
    let props = props
        .into_iter()
        .map(|(name, kind)| PropertyDecl::new(space.names.intern(name), kind))
        .collect();
    space.classes.define(path, Some(object), props).unwrap()
}

impl World {
    fn new() -> (World, Classes) {
        let mut space = ObjectSpace::new();
        let classes = build_classes(&mut space);
        let globals = space
            .alloc(classes.object, "Globals", None, Origin::Runtime)
            .unwrap();
        let game_info = space
            .alloc(classes.actor, "TheGameInfo", None, Origin::Runtime)
            .unwrap();
        let chain_kind = PropKind::Array(Box::new(obj_kind(classes.object)));
        let def_props = vec![
            ("ContextResolverChain", chain_kind.clone()),
            ("ValueResolverChain", chain_kind),
        ];
        let def_class = resolver_class(
            &mut space,
            classes.object,
            "Test.AttributeDefinition",
            def_props,
        );
        let attr = space
            .alloc(def_class, "Att_Test", None, Origin::Runtime)
            .unwrap();
        let w = World {
            space,
            defs: None,
            globals: Some(globals),
            attr,
            game_info: Some(game_info),
            queries: Vec::new(),
            slots: GlobalSlots::default(),
        };
        (w, classes)
    }

    fn obj(&mut self, class: ClassId, name: &str) -> ObjRef {
        self.space
            .alloc(class, name, None, Origin::Runtime)
            .unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.space.heap.get(o).unwrap().class;
        let prop = self.space.prop(class, name).unwrap();
        self.space.write(o, prop.into(), v).unwrap();
    }

    fn link(&mut self, o: ObjRef, name: &str, to: ObjRef) {
        self.set(o, name, Value::Object(Some(to)));
    }

    /// A resolver object of a new class at `path`.
    fn resolver(&mut self, object: ClassId, path: &str, props: Vec<(&str, PropKind)>) -> ObjRef {
        let class = resolver_class(&mut self.space, object, path, props);
        let name = path.rsplit('.').next().unwrap_or(path).to_string();
        self.obj(class, &name)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The chain walk: the evaluator's own (eval.rs)
// ---------------------------------------------------------------------------------------------------------------

fn walk(
    sys: &AttributeSystem,
    chain: &[Option<ObjRef>],
    source: Option<ObjRef>,
    w: &mut World,
) -> Result<Option<ObjRef>, AttrError> {
    let attr = set_chain(w, chain);
    sys.resolve_context(attr, source, &mut EvalCx::new(w))
}

fn walk_with_override(
    sys: &AttributeSystem,
    chain: &[Option<ObjRef>],
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    w: &mut World,
) -> Result<Option<ObjRef>, AttrError> {
    let attr = set_chain(w, chain);
    let read = sys.value_of(attr, source, override_source, &mut EvalCx::new(w))?;
    Ok(read.map(|v| v.context))
}

/// Writes `chain` as the test definition's `ContextResolverChain` (the value chain stays empty).
fn set_chain(w: &mut World, chain: &[Option<ObjRef>]) -> ObjRef {
    let attr = w.attr;
    let entries = chain.iter().map(|r| Value::Object(*r)).collect();
    w.set(attr, "ContextResolverChain", Value::array(entries));
    attr
}

// ---------------------------------------------------------------------------------------------------------------
// The scene: a player, an AI enemy, weapons, pools
// ---------------------------------------------------------------------------------------------------------------

struct Scene {
    w: World,
    c: Classes,
    sys: AttributeSystem,
    pc: ObjRef,
    player: ObjRef,
    pri: ObjRef,
    gun: ObjRef,
    off_gun: ObjRef,
    enemy: ObjRef,
    mind: ObjRef,
    /// A pawn whose `Instigator` is the enemy.
    minion: ObjRef,
    /// A non-pawn actor whose `Instigator` is the player.
    projectile: ObjRef,
    /// A pawn with no `Instigator`, driving `vehicle`.
    driver: ObjRef,
    vehicle: ObjRef,
    vehicle_controller: ObjRef,
    /// An object that is not an actor.
    thing: ObjRef,
    /// A `NoContextNeeded` resolver object, used to show a chain stopped before it.
    no_context: ObjRef,
}

const NO_CONTEXT: &str = "GearboxFramework.NoContextNeededAttributeContextResolver";

fn scene() -> Scene {
    let (mut w, c) = World::new();
    let pc = w.obj(c.player_controller, "PC");
    let player = w.obj(c.pawn, "PlayerPawn");
    let pri = w.obj(c.pri, "PRI");
    let gun = w.obj(c.weapon, "Gun");
    let off_gun = w.obj(c.weapon, "OffGun");
    w.set(off_gun, "bOffHand", Value::Bool(true));
    w.link(pc, "Pawn", player);
    w.link(pc, "PlayerReplicationInfo", pri);
    w.link(player, "Controller", pc);
    w.link(player, "Instigator", player);
    w.link(player, "PlayerReplicationInfo", pri);
    w.link(player, "Weapon", gun);
    w.link(player, "OffHandWeapon", off_gun);

    let enemy = w.obj(c.ai_pawn, "Enemy");
    let mind = w.obj(c.mind, "Mind");
    w.link(enemy, "Controller", mind);
    w.link(enemy, "MyWillowMind", mind);
    w.link(mind, "Pawn", enemy);

    let minion = w.obj(c.pawn, "Minion");
    w.link(minion, "Instigator", enemy);
    let projectile = w.obj(c.actor, "Projectile");
    w.link(projectile, "Instigator", player);

    let driver = w.obj(c.pawn, "Driver");
    let vehicle = w.obj(c.vehicle, "Vehicle");
    let vehicle_controller = w.obj(c.controller, "VehicleController");
    w.link(driver, "DrivenVehicle", vehicle);
    w.link(vehicle, "Controller", vehicle_controller);
    let thing = w.obj(c.object, "Thing");

    let object = c.object;
    let no_context = w.resolver(object, NO_CONTEXT, vec![]);
    let sys = AttributeSystem::standard().unwrap();
    Scene {
        w,
        c,
        sys,
        pc,
        player,
        pri,
        gun,
        off_gun,
        enemy,
        mind,
        minion,
        projectile,
        driver,
        vehicle,
        vehicle_controller,
        thing,
        no_context,
    }
}

/// The three chain rules for resolver object `r` of `s.sys`: `good` resolves to `expected`; a chain stopped by `r`
/// on `bad` ends there (the `NoContextNeeded` after it, which would answer the globals for a None, is not asked);
/// an empty chain is None; the override source is tried first, and a failing override falls back to the source.
fn three_rules(s: &mut Scene, r: ObjRef, good: ObjRef, expected: ObjRef, bad: Option<ObjRef>) {
    let sys = &s.sys;
    let w = &mut s.w;
    assert_eq!(
        walk(sys, &[Some(r)], Some(good), w),
        Ok(Some(expected)),
        "resolves"
    );
    assert_eq!(
        walk(sys, &[None, Some(r)], Some(good), w),
        Ok(Some(expected)),
        "null entry skipped"
    );
    // The first None ends the chain: the NoContextNeeded after `r`, which would answer the globals, is not asked.
    let after = [Some(r), Some(s.no_context)];
    if let Some(bad) = bad {
        assert_eq!(walk(sys, &[Some(r)], Some(bad), w), Ok(None), "bad source");
        assert_eq!(
            walk(sys, &after, Some(bad), w),
            Ok(None),
            "stops at the first None"
        );
        // The override does not resolve: the source is used.
        assert_eq!(
            walk_with_override(sys, &[Some(r)], Some(good), Some(bad), w),
            Ok(Some(expected))
        );
        // The override resolves: it wins over a source that does not.
        assert_eq!(
            walk_with_override(sys, &[Some(r)], Some(bad), Some(good), w),
            Ok(Some(expected))
        );
    }
    assert_eq!(walk(sys, &[], Some(good), w), Ok(None), "empty chain");
    assert_eq!(
        walk_with_override(sys, &[], Some(good), Some(good), w),
        Ok(None),
        "empty chain, override"
    );
}

fn missing(e: Result<Option<ObjRef>, AttrError>) -> Missing {
    match e {
        Err(AttrError::NotYet(n)) => n.missing,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn registered(s: &mut Scene, path: &str, props: Vec<(&str, PropKind)>) -> ObjRef {
    let object = s.c.object;
    s.w.resolver(object, path, props)
}

// ---------------------------------------------------------------------------------------------------------------
// One test per resolver
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn no_context_needed_passes_the_source_and_answers_the_globals_for_none() {
    let mut s = scene();
    let r = s.no_context;
    let (player, enemy) = (s.player, s.enemy);
    three_rules(&mut s, r, player, player, None);
    let globals = s.w.globals;
    assert_eq!(walk(&s.sys, &[Some(r)], None, &mut s.w), Ok(globals));
    // The override (None here) falls back to the source.
    assert_eq!(
        walk_with_override(&s.sys, &[Some(r)], Some(enemy), None, &mut s.w),
        Ok(Some(enemy))
    );
    // Never None: a world without globals is an error.
    s.w.globals = None;
    assert_eq!(
        missing(walk(&s.sys, &[Some(r)], None, &mut s.w)),
        Missing::LayerAbsent("ow-world")
    );
}

#[test]
fn a_chain_stopped_before_no_context_needed_is_none_not_the_globals() {
    let mut s = scene();
    let pawn = registered(&mut s, "Engine.PawnAttributeContextResolver", vec![]);
    let chain = [Some(pawn), Some(s.no_context)];
    let thing = s.thing;
    assert_eq!(walk(&s.sys, &chain, Some(thing), &mut s.w), Ok(None));
    assert_eq!(walk(&s.sys, &chain, None, &mut s.w), Ok(None));
}

#[test]
fn controller() {
    let mut s = scene();
    let r = registered(&mut s, "Engine.ControllerAttributeContextResolver", vec![]);
    let (pc, player, thing) = (s.pc, s.player, s.thing);
    three_rules(&mut s, r, pc, pc, Some(thing));
    let sys = &s.sys;
    let w = &mut s.w;
    // A pawn with an Instigator: the instigator's controller (the player's own, here).
    assert_eq!(walk(sys, &[Some(r)], Some(player), w), Ok(Some(pc)));
    // A pawn instigated by the enemy: the enemy's controller, the mind.
    assert_eq!(walk(sys, &[Some(r)], Some(s.minion), w), Ok(Some(s.mind)));
    // A non-pawn actor with an Instigator.
    assert_eq!(walk(sys, &[Some(r)], Some(s.projectile), w), Ok(Some(pc)));
    // A pawn without an Instigator, driving: the vehicle's controller.
    assert_eq!(
        walk(sys, &[Some(r)], Some(s.driver), w),
        Ok(Some(s.vehicle_controller))
    );
    // A pawn without an Instigator, not driving: its own controller (the enemy has no Instigator).
    assert_eq!(walk(sys, &[Some(r)], Some(s.enemy), w), Ok(Some(s.mind)));
    // The vehicle itself: a pawn without an Instigator, not driving: its own controller.
    assert_eq!(
        walk(sys, &[Some(r)], Some(s.vehicle), w),
        Ok(Some(s.vehicle_controller))
    );
    // An actor that is no pawn and has no Instigator: None.
    let game_info = w.game_info;
    assert_eq!(walk(sys, &[Some(r)], game_info, w), Ok(None));
    assert_eq!(walk(sys, &[Some(r)], None, w), Ok(None));
}

#[test]
fn pawn() {
    let mut s = scene();
    let r = registered(&mut s, "Engine.PawnAttributeContextResolver", vec![]);
    let (pc, player, thing) = (s.pc, s.player, s.thing);
    three_rules(&mut s, r, pc, player, Some(thing));
    let sys = &s.sys;
    let w = &mut s.w;
    assert_eq!(walk(sys, &[Some(r)], Some(player), w), Ok(Some(player)));
    assert_eq!(walk(sys, &[Some(r)], Some(s.mind), w), Ok(Some(s.enemy)));
    assert_eq!(
        walk(sys, &[Some(r)], Some(s.projectile), w),
        Ok(Some(player))
    );
    // A controller without a pawn, an actor without an Instigator: None.
    assert_eq!(
        walk(sys, &[Some(r)], Some(s.vehicle_controller), w),
        Ok(None)
    );
    let game_info = w.game_info;
    assert_eq!(walk(sys, &[Some(r)], game_info, w), Ok(None));
}

#[test]
fn weapon() {
    let mut s = scene();
    let r = registered(&mut s, "Engine.WeaponAttributeContextResolver", vec![]);
    let (pc, gun, thing) = (s.pc, s.gun, s.thing);
    three_rules(&mut s, r, pc, gun, Some(thing));
    let sys = &s.sys;
    let w = &mut s.w;
    assert_eq!(walk(sys, &[Some(r)], Some(s.player), w), Ok(Some(gun)));
    assert_eq!(
        walk(sys, &[Some(r)], Some(s.off_gun), w),
        Ok(Some(s.off_gun))
    );
    // An unarmed pawn, a controller without a pawn: None.
    assert_eq!(walk(sys, &[Some(r)], Some(s.enemy), w), Ok(None));
    assert_eq!(
        walk(sys, &[Some(r)], Some(s.vehicle_controller), w),
        Ok(None)
    );
}

#[test]
fn off_hand_weapon() {
    let mut s = scene();
    // Not registered by `standard` (package not recorded): the test registers it under an invented path.
    s.sys = AttributeSystem::new_empty()
        .with_context_resolver("Test.OffHandWeaponAttributeContextResolver", OffHandWeapon)
        .unwrap()
        .with_context_resolver(NO_CONTEXT, NoContextNeeded)
        .unwrap();
    let r = registered(&mut s, "Test.OffHandWeaponAttributeContextResolver", vec![]);
    let (pc, off_gun, thing) = (s.pc, s.off_gun, s.thing);
    three_rules(&mut s, r, pc, off_gun, Some(thing));
    let sys = &s.sys;
    let w = &mut s.w;
    assert_eq!(walk(sys, &[Some(r)], Some(s.player), w), Ok(Some(off_gun)));
    assert_eq!(walk(sys, &[Some(r)], Some(s.gun), w), Ok(Some(s.gun)));
    assert_eq!(walk(sys, &[Some(r)], Some(s.enemy), w), Ok(None));
    // A pawn class that declares no OffHandWeapon: not read, not a guessed None.
    let bare = w.obj(s.c.bare_pawn, "Bare");
    assert_eq!(
        missing(walk(sys, &[Some(r)], Some(bare), w)),
        Missing::NotRead
    );
}

#[test]
fn player_replication_info() {
    let mut s = scene();
    let r = registered(
        &mut s,
        "WillowGame.PlayerReplicationInfoAttributeContextResolver",
        vec![],
    );
    let (pc, pri, enemy) = (s.pc, s.pri, s.enemy);
    three_rules(&mut s, r, pc, pri, Some(enemy));
    let sys = &s.sys;
    let w = &mut s.w;
    assert_eq!(walk(sys, &[Some(r)], Some(s.player), w), Ok(Some(pri)));
    assert_eq!(walk(sys, &[Some(r)], Some(pri), w), Ok(Some(pri)));
    assert_eq!(walk(sys, &[Some(r)], Some(s.mind), w), Ok(None));
    assert_eq!(walk(sys, &[Some(r)], Some(s.thing), w), Ok(None));
}

#[test]
fn player_controller_is_the_engines_lookup() {
    let mut s = scene();
    s.sys = AttributeSystem::new_empty()
        .with_context_resolver(
            "Test.PlayerControllerAttributeContextResolver",
            PlayerController,
        )
        .unwrap()
        .with_context_resolver(NO_CONTEXT, NoContextNeeded)
        .unwrap();
    let r = registered(
        &mut s,
        "Test.PlayerControllerAttributeContextResolver",
        vec![],
    );
    let (pc, player, thing) = (s.pc, s.player, s.thing);
    s.w.queries.push(((EngineQuery::PlayerController, pc), pc));
    s.w.queries
        .push(((EngineQuery::PlayerController, player), pc));
    three_rules(&mut s, r, player, pc, Some(thing));
    // A world without engine state: the default query answer, which says the layer is absent.
    struct Bare(World);
    impl AttrWorld for Bare {
        fn parts(&mut self) -> WorldParts<'_> {
            self.0.parts()
        }
        fn globals(&mut self) -> Result<ObjRef, AttrError> {
            self.0.globals()
        }
        fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
            self.0.game_info()
        }
        fn global_slots(&mut self) -> &mut GlobalSlots {
            self.0.global_slots()
        }
        fn attribute_changed(
            &mut self,
            o: ObjRef,
            p: PropId,
            f: NumForms,
        ) -> Result<(), AttrError> {
            self.0.attribute_changed(o, p, f)
        }
        fn rng(&mut self, st: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
            self.0.rng(st)
        }
    }
    let mut bare = Bare(s.w);
    let attr = r;
    let mut cx = EvalCx::new(&mut bare);
    let got = PlayerController.resolve(&s.sys, r, attr, Some(pc), &mut cx);
    assert_eq!(missing(got), Missing::LayerAbsent("ow-world"));
}

#[test]
fn game_info() {
    let mut s = scene();
    let r = registered(&mut s, "Engine.GameInfoAttributeContextResolver", vec![]);
    let (pc, game_info) = (s.pc, s.w.game_info.unwrap());
    three_rules(&mut s, r, pc, game_info, None);
    assert_eq!(
        walk(&s.sys, &[Some(r)], None, &mut s.w),
        Ok(Some(game_info))
    );
    // A world that models no game info: None, and the chain ends.
    s.w.game_info = None;
    let chain = [Some(r), Some(s.no_context)];
    assert_eq!(walk(&s.sys, &chain, Some(pc), &mut s.w), Ok(None));
}

#[test]
fn willow_mind() {
    let mut s = scene();
    let r = registered(
        &mut s,
        "WillowGame.WillowMindAttributeContextResolver",
        vec![],
    );
    let (enemy, mind, pc) = (s.enemy, s.mind, s.pc);
    three_rules(&mut s, r, enemy, mind, Some(pc));
    assert_eq!(
        walk(&s.sys, &[Some(r)], Some(mind), &mut s.w),
        Ok(Some(mind))
    );
    let player = s.player;
    assert_eq!(walk(&s.sys, &[Some(r)], Some(player), &mut s.w), Ok(None));
}

#[test]
fn resource_pool() {
    let mut s = scene();
    let res_class = resolver_class(
        &mut s.w.space,
        s.c.object,
        "Test.ResourceDefinition",
        vec![],
    );
    let health = s.w.obj(res_class, "Health");
    let shield = s.w.obj(res_class, "Shield");
    let pool_class = resolver_class(&mut s.w.space, s.c.object, "Engine.ResourcePool", vec![]);
    let health_pool = s.w.obj(pool_class, "HealthPool");
    let props = vec![("Resource", obj_kind(res_class))];
    let r = registered(&mut s, "Engine.ResourcePoolAttributeContextResolver", props);
    s.w.link(r, "Resource", health);
    let (player, enemy) = (s.player, s.enemy);
    s.w.queries.push((
        (EngineQuery::ResourcePool { resource: health }, player),
        health_pool,
    ));
    three_rules(&mut s, r, player, health_pool, Some(enemy));
    // Another resource: no pool, None.
    s.w.link(r, "Resource", shield);
    assert_eq!(walk(&s.sys, &[Some(r)], Some(player), &mut s.w), Ok(None));
    // A None Resource: None, as the C++ oracle.
    s.w.set(r, "Resource", Value::Object(None));
    assert_eq!(walk(&s.sys, &[Some(r)], Some(player), &mut s.w), Ok(None));
    // A resolver class without a Resource property: not read.
    let odd = registered(&mut s, "Test.OddPoolResolver", vec![]);
    s.sys = AttributeSystem::new_empty()
        .with_context_resolver("Test.OddPoolResolver", ResourcePool)
        .unwrap();
    assert_eq!(
        missing(walk(&s.sys, &[Some(odd)], Some(player), &mut s.w)),
        Missing::NotRead
    );
}

#[test]
fn weapon_resource_pool_follows_the_hand() {
    let mut s = scene();
    s.sys = AttributeSystem::new_empty()
        .with_context_resolver(
            "Test.WeaponResourcePoolAttributeContextResolver",
            WeaponResourcePool,
        )
        .unwrap()
        .with_context_resolver(NO_CONTEXT, NoContextNeeded)
        .unwrap();
    let res_class = resolver_class(
        &mut s.w.space,
        s.c.object,
        "Test.ResourceDefinition",
        vec![],
    );
    let main = s.w.obj(res_class, "MainAmmo");
    let off = s.w.obj(res_class, "OffAmmo");
    let pool_class = resolver_class(&mut s.w.space, s.c.object, "Engine.ResourcePool", vec![]);
    let main_pool = s.w.obj(pool_class, "MainPool");
    let off_pool = s.w.obj(pool_class, "OffPool");
    let props = vec![
        ("PrimaryHandResource", obj_kind(res_class)),
        ("OffHandResource", obj_kind(res_class)),
    ];
    let r = registered(
        &mut s,
        "Test.WeaponResourcePoolAttributeContextResolver",
        props,
    );
    s.w.link(r, "PrimaryHandResource", main);
    s.w.link(r, "OffHandResource", off);
    let (gun, off_gun) = (s.gun, s.off_gun);
    s.w.queries.push((
        (EngineQuery::ResourcePool { resource: main }, gun),
        main_pool,
    ));
    s.w.queries.push((
        (EngineQuery::ResourcePool { resource: off }, off_gun),
        off_pool,
    ));
    // A weapon whose pool is missing resolves to None (the bad source here).
    let spare = s.w.obj(s.c.weapon, "Spare");
    three_rules(&mut s, r, gun, main_pool, Some(spare));
    assert_eq!(
        walk(&s.sys, &[Some(r)], Some(off_gun), &mut s.w),
        Ok(Some(off_pool))
    );
    // A source that is not a weapon: not read.
    let player = s.player;
    assert_eq!(
        missing(walk(&s.sys, &[Some(r)], Some(player), &mut s.w)),
        Missing::NotRead
    );
}

#[test]
fn equipped_inventory() {
    let mut s = scene();
    s.sys = AttributeSystem::new_empty()
        .with_context_resolver(
            "Test.EquippedInventoryAttributeContextResolver",
            EquippedInventory,
        )
        .unwrap()
        .with_context_resolver(NO_CONTEXT, NoContextNeeded)
        .unwrap();
    let props = vec![("EquipmentLocation", PropKind::Byte { enum_: None })];
    let r = registered(
        &mut s,
        "Test.EquippedInventoryAttributeContextResolver",
        props,
    );
    s.w.set(r, "EquipmentLocation", Value::Byte(3));
    let item = s.w.obj(s.c.actor, "ClassMod");
    let (player, enemy) = (s.player, s.enemy);
    s.w.queries.push((
        (EngineQuery::EquippedInventory { location: 3 }, player),
        item,
    ));
    three_rules(&mut s, r, player, item, Some(enemy));
    s.w.set(r, "EquipmentLocation", Value::Byte(1));
    assert_eq!(walk(&s.sys, &[Some(r)], Some(player), &mut s.w), Ok(None));
}

#[test]
fn the_unported_resolvers_say_what_they_wait_for() {
    let mut s = scene();
    let cases: [(&str, Missing); 6] = [
        (
            "Engine.BalancedActorAttributeContextResolver",
            Missing::NotPorted,
        ),
        (
            "WillowGame.StatusEffectChanceModifierResolver",
            Missing::NotRead,
        ),
        (
            "WillowGame.DesignerAttributeContextResolver",
            Missing::NotPorted,
        ),
        (
            "WillowGame.DesignerAttributeContextResolverByName",
            Missing::NotPorted,
        ),
        (
            "WillowGame.InventoryAttributeContextResolver",
            Missing::NotRead,
        ),
        (
            "Engine.ProjectileAttributeContextResolver",
            Missing::NotRead,
        ),
    ];
    let player = s.player;
    for (path, expected) in cases {
        let r = registered(&mut s, path, vec![]);
        assert_eq!(
            missing(walk(&s.sys, &[Some(r)], Some(player), &mut s.w)),
            expected,
            "{path}"
        );
        // A None source ends a chain without asking for what is missing, except for an unread class.
        let none = walk(&s.sys, &[Some(r)], None, &mut s.w);
        if path.contains("Projectile") {
            assert_eq!(missing(none), Missing::NotRead);
        } else {
            assert_eq!(none, Ok(None), "{path}");
            assert_eq!(
                walk(&s.sys, &[Some(r), Some(s.no_context)], None, &mut s.w),
                Ok(None)
            );
            assert_eq!(walk(&s.sys, &[], Some(player), &mut s.w), Ok(None));
        }
    }
    // The skill resolver (package not recorded), registered here under an invented path.
    let sys = AttributeSystem::new_empty()
        .with_context_resolver("Test.SkillAttributeContextResolver", Skill)
        .unwrap();
    let r = registered(&mut s, "Test.SkillAttributeContextResolver", vec![]);
    assert_eq!(
        missing(walk(&sys, &[Some(r)], Some(player), &mut s.w)),
        Missing::NotRead
    );
    assert_eq!(walk(&sys, &[Some(r)], None, &mut s.w), Ok(None));
}

// ---------------------------------------------------------------------------------------------------------------
// Registration, loading, stand-ins
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn standard_registers_every_recorded_class_once_and_no_guessed_path() {
    let mut s = scene();
    let recorded = [
        NO_CONTEXT,
        "Engine.ControllerAttributeContextResolver",
        "Engine.PawnAttributeContextResolver",
        "Engine.WeaponAttributeContextResolver",
        "Engine.ResourcePoolAttributeContextResolver",
        "Engine.GameInfoAttributeContextResolver",
        "Engine.BalancedActorAttributeContextResolver",
        "WillowGame.PlayerReplicationInfoAttributeContextResolver",
        "WillowGame.WillowMindAttributeContextResolver",
        "WillowGame.InventoryAttributeContextResolver",
        "WillowGame.StatusEffectChanceModifierResolver",
        "WillowGame.DesignerAttributeContextResolver",
        "WillowGame.DesignerAttributeContextResolverByName",
    ];
    let object = s.c.object;
    for path in recorded.iter().copied().chain(UNREAD.iter().copied()) {
        let r = if path == NO_CONTEXT {
            s.no_context
        } else {
            s.w.resolver(object, path, vec![])
        };
        assert!(s.sys.context_resolver(&s.w.space, r).is_ok(), "{path}");
        // Registering it again is refused: `standard` holds it once.
        let again = AttributeSystem::standard()
            .unwrap()
            .with_context_resolver(path, Unread);
        assert!(
            matches!(again, Err(AttrError::DuplicateResolver { .. })),
            "{path}"
        );
    }
    // The classes whose package is not recorded are served under no package.
    for (class, _) in PACKAGE_NOT_RECORDED {
        for package in ["Engine", "GearboxFramework", "WillowGame"] {
            let path = format!("{package}.{class}");
            let r = s.w.resolver(object, &path, vec![]);
            let got = s.sys.context_resolver(&s.w.space, r).map(|_| ());
            assert!(matches!(got, Err(AttrError::NotYet(_))), "{path}");
        }
    }
    let key = ObjectPath::parse("Engine.PawnAttributeContextResolver").unwrap();
    assert_eq!(key.as_str(), "Engine.PawnAttributeContextResolver");
}

#[test]
fn an_unloaded_resolver_definition_is_loaded_through_the_store() {
    let mut s = scene();
    let res_class = resolver_class(
        &mut s.w.space,
        s.c.object,
        "Test.ResourceDefinition",
        vec![],
    );
    let props = vec![("Resource", obj_kind(res_class))];
    let class = resolver_class(
        &mut s.w.space,
        s.c.object,
        "Engine.ResourcePoolAttributeContextResolver",
        props,
    );
    // A slotless handle, as the store makes for a definition named before it is loaded.
    let handle =
        s.w.space
            .alloc_stand_in(
                class,
                "Unloaded",
                Origin::Resource {
                    package: Default::default(),
                    index: 1,
                },
            )
            .unwrap();
    let player = s.player;
    // No store: the layer is absent.
    let got = walk(&s.sys, &[Some(handle)], Some(player), &mut s.w);
    assert_eq!(missing(got), Missing::LayerAbsent("ow-defs"));
    // With a store, a slotless object that is none of its unloaded handles stays a stand-in: reading its field fails
    // in the heap, not with a guessed value.
    s.w.defs = Some(DefStore::new_empty());
    let got = walk(&s.sys, &[Some(handle)], Some(player), &mut s.w);
    assert!(
        matches!(got, Err(AttrError::Heap(HeapError::SlotMissing { .. }))),
        "{got:?}"
    );
    // A loaded (slotted) resolver never touches the store.
    let loaded = s.w.obj(class, "Loaded");
    assert_eq!(
        walk(&s.sys, &[Some(loaded)], Some(player), &mut s.w),
        Ok(None)
    );
}

#[test]
fn an_ancestry_through_a_native_stand_in_cannot_answer() {
    let mut s = scene();
    let native =
        s.w.space
            .classes
            .define("Engine.NativeOnlyThing", None, vec![])
            .unwrap();
    s.w.space.classes.mark_native(native).unwrap();
    let sub =
        s.w.space
            .classes
            .define("Test.SubOfNative", Some(native), vec![])
            .unwrap();
    let odd = s.w.obj(sub, "Odd");
    let r = registered(&mut s, "Engine.PawnAttributeContextResolver", vec![]);
    assert_eq!(
        missing(walk(&s.sys, &[Some(r)], Some(odd), &mut s.w)),
        Missing::NotRead
    );
    assert_eq!(is_a(&s.w.space, s.player, "Engine.Pawn"), Ok(true));
    assert_eq!(is_a(&s.w.space, s.player, "No.SuchClass"), Ok(false));
}

#[test]
fn a_reference_property_of_another_kind_is_bad_data() {
    let mut s = scene();
    let props = vec![("Resource", PropKind::Int)];
    let r = registered(&mut s, "Engine.ResourcePoolAttributeContextResolver", props);
    let player = s.player;
    let got = walk(&s.sys, &[Some(r)], Some(player), &mut s.w);
    assert!(matches!(got, Err(AttrError::BadData { .. })), "{got:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// The C++ oracle: the pawn / mind / controller cases of src/progression.hpp
// ---------------------------------------------------------------------------------------------------------------

/// `src/progression.hpp`'s `AttributeContext` holds a fixed `replicationInfo`, `controller`, `pawn` and `mind`,
/// filled by `src/combat.cpp` `describe` from the source: a player controller gives its replication info and
/// itself; a `WillowMind` gives itself and its `Pawn`; a `WillowAIPawn` gives itself and its `MyWillowMind`. Its
/// resolvers (`src/progression.cpp` `attributeValue`) answer that field or end the chain. Every case the C++
/// answers reproduces here. Where the C++ subset has no field (a player controller's pawn, an AI's controller) it
/// answers None and the port follows NATIVE_ATTRIBUTES.md section 5 instead; those cases are listed in
/// [`where_the_port_goes_past_the_cpp_subset`].
#[test]
fn the_progression_hpp_pawn_mind_and_controller_cases_reproduce() {
    let mut s = scene();
    let object = s.c.object;
    let pri_r = s.w.resolver(
        object,
        "WillowGame.PlayerReplicationInfoAttributeContextResolver",
        vec![],
    );
    let controller_r =
        s.w.resolver(object, "Engine.ControllerAttributeContextResolver", vec![]);
    let pawn_r =
        s.w.resolver(object, "Engine.PawnAttributeContextResolver", vec![]);
    let mind_r = s.w.resolver(
        object,
        "WillowGame.WillowMindAttributeContextResolver",
        vec![],
    );
    let nc = s.no_context;
    let (pc, pri, enemy, mind) = (s.pc, s.pri, s.enemy, s.mind);
    // (source, chain, the C++ answer: Some(context), or None for "does not resolve")
    let cases: Vec<(ObjRef, Vec<ObjRef>, Option<ObjRef>)> = vec![
        // a player controller: context.replicationInfo, context.controller
        (pc, vec![pri_r], Some(pri)),
        (pc, vec![controller_r], Some(pc)),
        (pc, vec![nc, pri_r], Some(pri)),
        (pc, vec![mind_r], None),
        // a WillowMind: context.mind, context.pawn = mind.Pawn
        (mind, vec![mind_r], Some(mind)),
        (mind, vec![pawn_r], Some(enemy)),
        (mind, vec![nc, mind_r], Some(mind)),
        (mind, vec![pri_r], None),
        // a WillowAIPawn: context.pawn, context.mind = pawn.MyWillowMind
        (enemy, vec![pawn_r], Some(enemy)),
        (enemy, vec![mind_r], Some(mind)),
        (enemy, vec![nc, pawn_r], Some(enemy)),
        (enemy, vec![pawn_r, mind_r], Some(mind)),
        (enemy, vec![pri_r], None),
        // the C++ "empty chain: no value" rule
        (enemy, vec![], None),
    ];
    for (source, chain, expected) in cases {
        let chain: Vec<Option<ObjRef>> = chain.into_iter().map(Some).collect();
        assert_eq!(
            walk(&s.sys, &chain, Some(source), &mut s.w),
            Ok(expected),
            "{source:?} {chain:?}"
        );
    }
    // A mind with no pawn and an AI pawn with no mind: the C++ field is null, the chain does not resolve.
    let lone_mind = s.w.obj(s.c.mind, "LoneMind");
    let lone_pawn = s.w.obj(s.c.ai_pawn, "LonePawn");
    assert_eq!(
        walk(&s.sys, &[Some(pawn_r)], Some(lone_mind), &mut s.w),
        Ok(None)
    );
    assert_eq!(
        walk(&s.sys, &[Some(mind_r)], Some(lone_pawn), &mut s.w),
        Ok(None)
    );
}

/// The cases where the C++ subset answers None only because its `AttributeContext` has no such field, and the port
/// answers what NATIVE_ATTRIBUTES.md section 5 says. Both are UNVERIFIED; this pins the difference.
#[test]
fn where_the_port_goes_past_the_cpp_subset() {
    let mut s = scene();
    let object = s.c.object;
    let controller_r =
        s.w.resolver(object, "Engine.ControllerAttributeContextResolver", vec![]);
    let pawn_r =
        s.w.resolver(object, "Engine.PawnAttributeContextResolver", vec![]);
    let (pc, player, enemy, mind) = (s.pc, s.player, s.enemy, s.mind);
    // A player controller's pawn (C++: no pawn field for a player source).
    assert_eq!(
        walk(&s.sys, &[Some(pawn_r)], Some(pc), &mut s.w),
        Ok(Some(player))
    );
    // An AI's controller (C++: no controller field for an AI source): the mind, a controller.
    assert_eq!(
        walk(&s.sys, &[Some(controller_r)], Some(mind), &mut s.w),
        Ok(Some(mind))
    );
    assert_eq!(
        walk(&s.sys, &[Some(controller_r)], Some(enemy), &mut s.w),
        Ok(Some(mind))
    );
}
