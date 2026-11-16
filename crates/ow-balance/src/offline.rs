//! Owner: lane W3-BALANCE. The offline balance world: a game replication info, a player replication info, a player
//! controller and the engine globals object built in an [`ObjectSpace`] from a [`BalanceSnapshot`], with an
//! [`AttrWorld`] implementation, so offline tools and tests evaluate balance data through the same resolver chains as
//! the running game (architecture section 3.6).
//!
//! **What it models.** The classes come from the space (in the game: the code packages' reflection, laid out through
//! the store's `CodeIndex`; in tests: synthetic classes). It writes what the snapshot says where the records name the
//! field: the replication info's `ExpLevel` (the player's level, NATIVE_PROGRESSION.md section 2), the controller's
//! `PlayerReplicationInfo`, the game replication info's `CurrentPlaythrough` (NATIVE_CONTROLLER_HELPERS.md), and the
//! replication info's `Owner` when its class declares one. The OP level, stage, awesome level and level-cap packs are
//! held in the snapshot for the rules that take them ([`OfflineWorld::item_stage_cap`],
//! [`OfflineWorld::max_exp_level`]); where the game keeps them on its objects is not read.
//!
//! **What it does not model**, each answered with `NotYet`, never a stand-in: more than one player (decision D12;
//! the game info and the party are not modelled); a game info (`game_info` is `Ok(None)`, the contract's answer for an
//! offline world); engine queries other than the player controller of the controller itself and the game replication
//! info (answered with this world's, whatever the source); change notifications
//! (they dispatch the owner class's native or script, which only the running world has); a random source unless one
//! is given ([`OfflineWorld::with_rng`]).

pub(crate) mod data;
#[cfg(test)]
pub(crate) mod fixture;

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, AttrWorld, EngineQuery, EvalCx, GlobalSlots, NumForms, WorldParts};
use ow_core::rand::{CrtRand, CrtStream, Rand15};
use ow_core::{NotYet, ObjectPath};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId};
use ow_object::layout::PropKind;
use ow_object::value::Value;

use crate::cap;
use crate::stage::{RegionQuery, RegionStage, RegionStages, region_game_stage};
use crate::types::{BalanceError, BalanceSnapshot, ExpLevel, GameStage, PlayerCount};
use crate::xp::{skill_points_for_level_up, write_level};

/// The record the offline world names.
pub const RECORD: &str = "architecture section 3.6 (OfflineWorld)";

/// The player controller's class.
pub const PLAYER_CONTROLLER_CLASS: &str = "WillowGame.WillowPlayerController";
/// The player replication info's class.
pub const REPLICATION_INFO_CLASS: &str = "WillowGame.WillowPlayerReplicationInfo";
/// The game replication info's class.
pub const GAME_REPLICATION_INFO_CLASS: &str = "WillowGame.WillowGameReplicationInfo";
/// The engine globals' class (the object `NoContextNeededAttributeContextResolver` gives for a None source).
pub const GLOBALS_CLASS: &str = "WillowGame.WillowGlobals";

/// The offline world (module documentation).
pub struct OfflineWorld {
    space: ObjectSpace,
    defs: Option<DefStore>,
    snapshot: BalanceSnapshot,
    globals: ObjRef,
    game_replication_info: ObjRef,
    replication_info: ObjRef,
    controller: ObjRef,
    slots: GlobalSlots,
    rng: Option<CrtRand>,
    region_stages: RegionStages,
}

fn class(space: &ObjectSpace, path: &'static str) -> Result<ClassId, BalanceError> {
    space.classes.by_path(path).ok_or_else(|| {
        NotYet::layer_absent(
            format!("the offline world's class {path} (not laid out in this space)"),
            "ow-object reflection of the WillowGame classes",
            RECORD,
        )
        .into()
    })
}

fn declared(space: &ObjectSpace, obj: ObjRef, name: &str) -> Result<Option<PropId>, BalanceError> {
    let class = space.heap.get(obj).map_err(AttrError::from)?.class;
    Ok(space.prop(class, name))
}

fn must_declare(space: &ObjectSpace, obj: ObjRef, name: &str) -> Result<PropId, BalanceError> {
    declared(space, obj, name)?.ok_or_else(|| {
        BalanceError::Attr(AttrError::BadData {
            reason: format!(
                "{} declares no {name}",
                ow_attr::eval::class_path(space, obj)
            )
            .into(),
        })
    })
}

/// Writes the integer `n` into `prop` of `obj` as the property's declared kind (an int or a byte, plain or
/// attribute); a byte out of range is refused.
fn write_integer(
    space: &mut ObjectSpace,
    obj: ObjRef,
    prop: PropId,
    n: i32,
) -> Result<(), BalanceError> {
    let kind = space
        .classes
        .decl(prop)
        .map(|d| d.kind.clone())
        .ok_or_else(|| {
            BalanceError::Attr(AttrError::BadData {
                reason: "a property without a declaration".into(),
            })
        })?;
    let v = match kind {
        PropKind::Int | PropKind::IntAttribute => Value::Int(n),
        PropKind::Byte { .. } | PropKind::ByteAttribute { .. } => {
            Value::Byte(u8::try_from(n).map_err(|_| BalanceError::OutOfRange {
                what: "a byte property (0..=255)",
                value: i64::from(n),
            })?)
        }
        other => {
            return Err(BalanceError::Attr(AttrError::BadData {
                reason: format!("an integer written to a {} property", other.kind_name()).into(),
            }));
        }
    };
    space
        .write(obj, PropAddr::scalar(prop), v)
        .map_err(AttrError::from)?;
    Ok(())
}

impl OfflineWorld {
    /// Builds the world's objects in `space` from `snapshot` (module documentation). The space must lay out
    /// [`PLAYER_CONTROLLER_CLASS`], [`REPLICATION_INFO_CLASS`], [`GAME_REPLICATION_INFO_CLASS`] and
    /// [`GLOBALS_CLASS`] (`NotYet { LayerAbsent }` otherwise), with `ExpLevel` on the replication info,
    /// `PlayerReplicationInfo` on the controller and `CurrentPlaythrough` on the game replication info (bad data
    /// otherwise). More than one player is `NotYet` (D12).
    pub fn new(
        mut space: ObjectSpace,
        defs: Option<DefStore>,
        snapshot: BalanceSnapshot,
    ) -> Result<Self, BalanceError> {
        snapshot.validate()?;
        if snapshot.players != PlayerCount::SINGLE {
            return Err(NotYet::not_read(
                format!(
                    "an offline world with {} players: the game info and the party are not modelled (decision D12)",
                    snapshot.players.get()
                ),
                RECORD,
            )
            .into());
        }
        let alloc = |space: &mut ObjectSpace, path: &'static str, name: &str| {
            let c = class(space, path)?;
            space
                .alloc(c, name, None, Origin::Runtime)
                .map_err(|e| BalanceError::from(AttrError::from(e)))
        };
        let globals = alloc(&mut space, GLOBALS_CLASS, "WillowGlobals_0")?;
        let game_replication_info = alloc(
            &mut space,
            GAME_REPLICATION_INFO_CLASS,
            "WillowGameReplicationInfo_0",
        )?;
        let replication_info = alloc(
            &mut space,
            REPLICATION_INFO_CLASS,
            "WillowPlayerReplicationInfo_0",
        )?;
        let controller = alloc(
            &mut space,
            PLAYER_CONTROLLER_CLASS,
            "WillowPlayerController_0",
        )?;

        let p = must_declare(&space, replication_info, "ExpLevel")?;
        write_integer(&mut space, replication_info, p, snapshot.player_level.get())?;
        let p = must_declare(&space, controller, "PlayerReplicationInfo")?;
        space
            .write(
                controller,
                PropAddr::scalar(p),
                Value::Object(Some(replication_info)),
            )
            .map_err(AttrError::from)?;
        let p = must_declare(&space, game_replication_info, "CurrentPlaythrough")?;
        write_integer(
            &mut space,
            game_replication_info,
            p,
            i32::from(snapshot.playthrough.index()),
        )?;
        if let Some(p) = declared(&space, replication_info, "Owner")? {
            space
                .write(
                    replication_info,
                    PropAddr::scalar(p),
                    Value::Object(Some(controller)),
                )
                .map_err(AttrError::from)?;
        }
        Ok(OfflineWorld {
            space,
            defs,
            snapshot,
            globals,
            game_replication_info,
            replication_info,
            controller,
            slots: GlobalSlots::default(),
            rng: None,
            region_stages: RegionStages::new(),
        })
    }

    /// This world with a generator: every stream draws from it (an offline tool's one seed).
    pub fn with_rng(mut self, rng: CrtRand) -> Self {
        self.rng = Some(rng);
        self
    }

    pub fn snapshot(&self) -> &BalanceSnapshot {
        &self.snapshot
    }
    pub fn space(&self) -> &ObjectSpace {
        &self.space
    }
    pub fn space_mut(&mut self) -> &mut ObjectSpace {
        &mut self.space
    }
    pub fn defs_mut(&mut self) -> Option<&mut DefStore> {
        self.defs.as_mut()
    }
    pub fn controller(&self) -> ObjRef {
        self.controller
    }
    pub fn replication_info(&self) -> ObjRef {
        self.replication_info
    }
    pub fn game_replication_info(&self) -> ObjRef {
        self.game_replication_info
    }
    pub fn globals_object(&self) -> ObjRef {
        self.globals
    }
    /// The region stages the player controller holds.
    pub fn region_stages(&self) -> &RegionStages {
        &self.region_stages
    }
    pub fn region_stages_mut(&mut self) -> &mut RegionStages {
        &mut self.region_stages
    }

    /// The definition at `path`, through the world's store (`NotYet { LayerAbsent("ow-defs") }` without one).
    pub fn find(&mut self, path: &ObjectPath) -> Result<Option<ObjRef>, BalanceError> {
        let Some(defs) = self.defs.as_mut() else {
            return Err(NotYet::layer_absent(
                format!(
                    "finding {} (this offline world has no definition store)",
                    path.as_str()
                ),
                "ow-defs",
                RECORD,
            )
            .into());
        };
        defs.find(&mut self.space, path)
            .map_err(|e| BalanceError::from(AttrError::from(e)))
    }

    /// The player's level as the replication info holds it.
    pub fn player_level(&self) -> ExpLevel {
        let level = must_declare(&self.space, self.replication_info, "ExpLevel")
            .ok()
            .and_then(|p| {
                self.space
                    .read(self.replication_info, PropAddr::scalar(p))
                    .ok()
            })
            .and_then(|v| match v {
                Value::Int(i) => Some(*i),
                Value::Byte(b) => Some(i32::from(*b)),
                _ => None,
            });
        // new() wrote it; a later write through space_mut() of another kind is the caller's.
        ExpLevel::new(level.unwrap_or(self.snapshot.player_level.get()))
    }

    /// Sets the player's level in the snapshot and on the replication info.
    pub fn set_player_level(&mut self, level: ExpLevel) -> Result<(), BalanceError> {
        let p = must_declare(&self.space, self.replication_info, "ExpLevel")?;
        write_integer(&mut self.space, self.replication_info, p, level.get())?;
        self.snapshot.player_level = level;
        Ok(())
    }

    /// The player's maximum level for the snapshot's level-cap packs ([`cap::max_exp_level`]).
    pub fn max_exp_level(&self) -> ExpLevel {
        cap::max_exp_level(self.snapshot.level_cap_packs)
    }

    /// The item stage cap for the snapshot's playthrough, OP level and packs ([`cap::item_stage_cap`]).
    pub fn item_stage_cap(&self) -> Result<GameStage, BalanceError> {
        cap::item_stage_cap(
            self.snapshot.playthrough,
            self.snapshot.op,
            self.snapshot.level_cap_packs,
        )
    }

    /// The region's game stage for this world's player ([`region_game_stage`]), stored in this world's
    /// [`RegionStages`]. The world has no pawn, so the player's level stands for the local player pawn's experience
    /// level (UNVERIFIED that the two are equal); `completed_missions` are the missions the player has complete.
    pub fn region_game_stage(
        &mut self,
        sys: &AttributeSystem,
        globals_definition: ObjRef,
        region: ObjRef,
        completed_missions: &[ObjRef],
    ) -> Result<RegionStage, BalanceError> {
        let q = RegionQuery::new(
            region,
            self.snapshot.playthrough,
            Some(self.player_level()),
            completed_missions,
            Some(self.controller),
        );
        let mut stages = std::mem::take(&mut self.region_stages);
        let result = {
            let mut cx = EvalCx::new(self);
            region_game_stage(sys, &mut cx, globals_definition, &mut stages, &q)
        };
        self.region_stages = stages;
        result
    }

    /// The general skill points a player at `level` has earned: the sum of [`skill_points_for_level_up`] over the
    /// level-ups 2..=`level`, each evaluated with the replication info at the level just reached, as the script
    /// `ExpLevelUp` evaluates it after the increment. The replication info's level is put back afterwards, also on
    /// an error.
    pub fn skill_points_earned(
        &mut self,
        sys: &AttributeSystem,
        globals_definition: ObjRef,
        level: ExpLevel,
    ) -> Result<i32, BalanceError> {
        let before = self.player_level();
        let (controller, pri) = (self.controller, self.replication_info);
        let result = (|| {
            let mut cx = EvalCx::new(self);
            let mut total: i32 = 0;
            for k in 2..=level.get() {
                write_level(&mut cx, pri, ExpLevel::new(k))?;
                let points =
                    skill_points_for_level_up(sys, &mut cx, globals_definition, Some(controller))?;
                total = total.checked_add(points).ok_or(BalanceError::OutOfRange {
                    what: "skill points earned (the sum overflows an int)",
                    value: i64::from(total),
                })?;
            }
            Ok(total)
        })();
        let mut cx = EvalCx::new(self);
        write_level(&mut cx, pri, before)?;
        result
    }
}

impl AttrWorld for OfflineWorld {
    fn parts(&mut self) -> WorldParts<'_> {
        WorldParts::new(&mut self.space, self.defs.as_mut())
    }

    fn globals(&mut self) -> Result<ObjRef, AttrError> {
        Ok(self.globals)
    }

    /// An offline world models no game info.
    fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
        Ok(None)
    }

    /// The player controller of the controller is itself, and the game replication info is this world's (the query
    /// does not read its source); every other query needs the running world.
    fn engine_query(
        &mut self,
        q: EngineQuery,
        source: ObjRef,
    ) -> Result<Option<ObjRef>, AttrError> {
        match q {
            EngineQuery::PlayerController if source == self.controller => Ok(Some(self.controller)),
            EngineQuery::GameReplicationInfo => Ok(Some(self.game_replication_info)),
            other => Err(NotYet::layer_absent(
                format!("engine query {other:?} from {source:?} in the offline world"),
                "ow-world",
                RECORD,
            )
            .into()),
        }
    }

    fn global_slots(&mut self) -> &mut GlobalSlots {
        &mut self.slots
    }

    /// A change notification dispatches the owner class's native or script, which the offline world does not have.
    fn attribute_changed(
        &mut self,
        owner: ObjRef,
        value_prop: PropId,
        _forms: NumForms,
    ) -> Result<(), AttrError> {
        let class = ow_attr::eval::class_path(&self.space, owner);
        Err(NotYet::layer_absent(
            format!(
                "the change notification of {class} (property {value_prop:?}) in the offline world"
            ),
            "ow-world",
            RECORD,
        )
        .into())
    }

    fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        match self.rng.as_mut() {
            Some(r) => Ok(r),
            None => Err(NotYet::layer_absent(
                format!("a random source for {s:?} (this offline world was given none)"),
                "ow-world",
                RECORD,
            )
            .into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{Fx, snapshot};
    use super::*;
    use crate::types::{AwesomeLevel, LevelCapUpgrades, OpLevel, Playthrough};
    use ow_attr::math::PowForm;
    use ow_core::notyet::Missing;
    use ow_core::num::pow_f32;
    use ow_core::parity::ParityMode;
    use ow_defs::identity::IdentityPolicy;
    use std::hint::black_box;

    #[test]
    fn the_world_writes_the_snapshot_onto_its_objects() {
        let fx = Fx::new();
        let (_sys, world) = fx.into_world();
        let space = world.space();
        let read = |o: ObjRef, n: &str| {
            let p = must_declare(space, o, n).unwrap();
            space.read(o, PropAddr::scalar(p)).unwrap().clone()
        };
        assert_eq!(read(world.replication_info(), "ExpLevel"), Value::Int(8));
        assert_eq!(
            read(world.controller(), "PlayerReplicationInfo"),
            Value::Object(Some(world.replication_info()))
        );
        assert_eq!(
            read(world.game_replication_info(), "CurrentPlaythrough"),
            Value::Int(0)
        );
        assert_eq!(
            read(world.replication_info(), "Owner"),
            Value::Object(Some(world.controller()))
        );
        assert_eq!(world.player_level(), ExpLevel::new(8));
        assert_eq!(world.max_exp_level(), ExpLevel::new(50));
        assert_eq!(world.item_stage_cap().unwrap(), GameStage(50));
    }

    #[test]
    fn the_health_base_reproduces_the_live_numbers() {
        // REALGAME_GROUND_TRUTH.md: the health pool's MaxValueBaseValue read live, 102.152 (L2), 212.6755 (L8),
        // 638.886 (L17), 415,509.44 (L70) = 80 x 1.13^L to float precision. Evaluated through Init_PlayerHealth's
        // recorded shape (NATIVE_PROGRESSION.md section 4): the 94 and the 1 are replaced by the attributes, the level
        // comes from the player's replication info. Also bit-equal to the record's closed form in single precision.
        let live = [
            (2, 102.152, 1e-3),
            (8, 212.6755, 1e-4),
            (17, 638.886, 1e-3),
            (70, 415_509.44, 1e-2),
        ];
        for pow in [PowForm::DoubleThenSingle, PowForm::Single] {
            for (level, shown, unit) in live {
                let mut fx = Fx::new();
                let health = fx.player_health_data();
                let (sys, mut world) = fx.into_world();
                world.set_player_level(ExpLevel::new(level)).unwrap();
                let controller = world.controller();
                let mut cx = EvalCx::new(&mut world);
                cx.forms.pow = pow;
                let got = sys
                    .evaluate_init(&health, Some(controller), None, &mut cx)
                    .unwrap();
                assert!(
                    (f64::from(got) - shown).abs() <= unit / 2.0,
                    "{pow:?} L{level}"
                );
                let closed = 80.0f32 * pow_f32(1.13, black_box(level) as f32);
                assert_eq!(
                    got.to_bits(),
                    closed.max(20.0).to_bits(),
                    "{pow:?} L{level}"
                );
            }
        }
    }

    #[test]
    fn more_players_and_op_outside_uvhm_are_refused() {
        let mut s = snapshot();
        s.players = PlayerCount::new(2).unwrap();
        let fx = Fx::new();
        assert!(matches!(
            OfflineWorld::new(fx.space_only(), None, s),
            Err(BalanceError::NotYet(n)) if n.missing == Missing::NotRead
        ));
        let mut s = snapshot();
        s.op = OpLevel::new(3).unwrap();
        let fx = Fx::new();
        assert!(matches!(
            OfflineWorld::new(fx.space_only(), None, s),
            Err(BalanceError::OpLevelOutsideUvhm { op: 3, .. })
        ));
    }

    #[test]
    fn a_space_without_the_classes_is_a_missing_layer() {
        let s = snapshot();
        match OfflineWorld::new(ObjectSpace::new(), None, s) {
            Err(BalanceError::NotYet(n)) => {
                assert!(matches!(n.missing, Missing::LayerAbsent(_)));
            }
            other => panic!("expected NotYet, got {:?}", other.err()),
        }
    }

    #[test]
    fn the_snapshot_drives_the_caps() {
        let fx = Fx::new();
        let s = BalanceSnapshot::new(
            Playthrough::UVHM,
            OpLevel::new(4).unwrap(),
            PlayerCount::SINGLE,
            ExpLevel::new(72),
            GameStage(72),
            AwesomeLevel::new(0),
            LevelCapUpgrades::ALL,
        )
        .unwrap();
        let world = OfflineWorld::new(fx.space_only(), None, s).unwrap();
        assert_eq!(world.max_exp_level(), ExpLevel::new(80));
        assert_eq!(world.item_stage_cap().unwrap(), GameStage(84));
        let space = world.space();
        let p = must_declare(space, world.game_replication_info(), "CurrentPlaythrough").unwrap();
        assert_eq!(
            space
                .read(world.game_replication_info(), PropAddr::scalar(p))
                .unwrap(),
            &Value::Int(2)
        );
    }

    #[test]
    fn the_world_answers_its_game_replication_info_and_the_count_reads_it() {
        // The query answers this world's replication info whatever the source; the playthrough count
        // (NATIVE_PLAYTHROUGH_ATTRIBUTES.md section 3, Off mode: no store) reads its CurrentPlaythrough, an int as the
        // note reads it (the int path). Installed field 0: Normal 1, TVHM 2, UVHM 2; a non-zero field: UVHM 3.
        for (pt, field0, field1) in [
            (Playthrough::NORMAL, 1.0, 1.0),
            (Playthrough::TVHM, 2.0, 2.0),
            (Playthrough::UVHM, 2.0, 3.0),
        ] {
            let mut fx = Fx::new();
            let a0 = fx.playthrough_count_attr(0);
            let a1 = fx.playthrough_count_attr(1);
            let s = BalanceSnapshot::new(
                pt,
                OpLevel::NONE,
                PlayerCount::SINGLE,
                ExpLevel::new(8),
                GameStage(8),
                AwesomeLevel::new(0),
                LevelCapUpgrades::NONE,
            )
            .unwrap();
            let mut world = OfflineWorld::new(fx.space_only(), None, s).unwrap();
            let gri = world.game_replication_info();
            for source in [world.controller(), world.replication_info(), gri] {
                assert_eq!(
                    world
                        .engine_query(EngineQuery::GameReplicationInfo, source)
                        .unwrap(),
                    Some(gri)
                );
            }
            let sys = AttributeSystem::standard().unwrap();
            let controller = world.controller();
            for (attr, want) in [(a0, field0), (a1, field1)] {
                let v = sys
                    .value_of(attr, Some(controller), None, &mut EvalCx::new(&mut world))
                    .unwrap()
                    .expect("the no-context chain resolves");
                assert_eq!(v.value, want, "{pt:?}");
            }
        }
    }

    #[test]
    fn the_world_answers_not_yet_for_what_it_does_not_model() {
        let fx = Fx::new();
        let (_sys, mut world) = fx.into_world();
        let controller = world.controller();
        let pri = world.replication_info();
        assert_eq!(world.game_info().unwrap(), None);
        assert_eq!(
            world
                .engine_query(EngineQuery::PlayerController, controller)
                .unwrap(),
            Some(controller)
        );
        assert!(matches!(
            world.engine_query(EngineQuery::PlayerController, pri),
            Err(AttrError::NotYet(_))
        ));
        assert!(matches!(
            world.rng(CrtStream::AttributeVariance),
            Err(AttrError::NotYet(_))
        ));
        let p = must_declare(world.space(), pri, "ExpLevel").unwrap();
        assert!(matches!(
            world.attribute_changed(pri, p, NumForms::default()),
            Err(AttrError::NotYet(_))
        ));
        let mut world = world.with_rng(CrtRand::new(1));
        assert!(world.rng(CrtStream::AttributeVariance).is_ok());
        // No store: finding a path is a missing layer.
        assert!(matches!(
            world.find(&ObjectPath::parse("GD_Globals.General.Globals").unwrap()),
            Err(BalanceError::NotYet(n)) if matches!(n.missing, Missing::LayerAbsent("ow-defs"))
        ));
    }

    #[test]
    fn find_passes_the_store_answer_through_unchanged() {
        // Whatever the store answers (NotYet while the DefStore body is in review, a found object once it lands),
        // OfflineWorld::find hands it on, converted only by the error From chain. The expected answer comes from a
        // second store over the same directory and a space built the same way, so the handles agree.
        let dir = std::env::temp_dir().join(format!("ow-balance-offline-{}", std::process::id()));
        let path = ObjectPath::parse("Test.Curve").unwrap();
        let defs = DefStore::from_synthetic(&dir, &[], IdentityPolicy::default(), ParityMode::Off);
        let other = DefStore::open(dir.clone(), IdentityPolicy::default(), ParityMode::Off);
        let (defs, mut other) = match (defs, other) {
            (Ok(a), Ok(b)) => (a, b),
            (a, b) => {
                let _ = std::fs::remove_dir_all(&dir);
                panic!("synthetic stores: {:?} / {:?}", a.err(), b.err());
            }
        };
        let mut world = OfflineWorld::new(Fx::new().space_only(), Some(defs), snapshot()).unwrap();
        let mut twin = OfflineWorld::new(Fx::new().space_only(), None, snapshot()).unwrap();
        let got = world.find(&path);
        let want = other
            .find(twin.space_mut(), &path)
            .map_err(|e| BalanceError::from(AttrError::from(e)));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got, want);
    }

    #[test]
    fn region_stage_through_the_world_is_stored_with_its_controller() {
        let mut fx = Fx::new();
        let r = fx.region_table(7, 9, &[(8, 11)], 0);
        let (sys, mut world) = fx.into_world();
        assert_eq!(
            world
                .region_game_stage(&sys, r.globals, r.region, &[])
                .unwrap()
                .stage,
            GameStage(8)
        );
        world.set_player_level(ExpLevel::new(12)).unwrap();
        assert_eq!(
            world
                .region_game_stage(&sys, r.globals, r.region, &[r.missions[0]])
                .unwrap()
                .stage,
            GameStage(8)
        );
        assert_eq!(world.region_stages().len(), 1);
    }
}
