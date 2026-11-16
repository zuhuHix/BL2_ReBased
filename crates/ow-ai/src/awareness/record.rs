//! Owner: lane W7-AI (part 1). The plain data of awareness: threat levels, exposure, opinions, one target record, an
//! actor's facts and the settings read from the definitions. AI-assisted (Claude Code).

use ow_core::units::Rotator;
use ow_object::ids::ObjRef;

use super::Vec3;

/// `WillowAIComponent.EThreatLevel` (NATIVE_AI_AWARENESS.md, "Threat level"). The VM adapter names the byte from the
/// installed enum, never by position.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThreatLevel {
    NoThreat,
    InThreatRadius,
    SeenInThreatRadius,
    Provoked,
}

impl ThreatLevel {
    /// The enum value name.
    pub fn name(self) -> &'static str {
        match self {
            ThreatLevel::NoThreat => "ETL_NoThreat",
            ThreatLevel::InThreatRadius => "ETL_InThreatRadius",
            ThreatLevel::SeenInThreatRadius => "ETL_SeenInThreatRadius",
            ThreatLevel::Provoked => "ETL_Provoked",
        }
    }

    /// The level of an enum value name (case folded), if it is one.
    pub fn from_name(name: &str) -> Option<ThreatLevel> {
        [
            ThreatLevel::NoThreat,
            ThreatLevel::InThreatRadius,
            ThreatLevel::SeenInThreatRadius,
            ThreatLevel::Provoked,
        ]
        .into_iter()
        .find(|l| l.name().eq_ignore_ascii_case(name))
    }

    /// Level 2 or more: what acquiring and keeping a target need (`ValidCurrentTarget`).
    pub fn seen(self) -> bool {
        self >= ThreatLevel::SeenInThreatRadius
    }
}

/// `WillowMindTargetInfo.Exposure` (`TargetExposure`).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exposure {
    None,
    Half,
    Full,
}

impl Exposure {
    /// The three-level byte of an exposure fraction (`WillowTI_CalcExposure`, NATIVE_AI_MELEE.md): below 0.4 none,
    /// below 0.9 half, else full. UNVERIFIED thresholds: the game showed 0.0 none, 0.5 half, 1.0 full (L8); -1.0, the
    /// value the game shows on a fresh record, is below 0.4.
    pub fn of(fraction: f32) -> Exposure {
        if fraction < 0.4 {
            Exposure::None
        } else if fraction < 0.9 {
            Exposure::Half
        } else {
            Exposure::Full
        }
    }
}

/// `Pawn.GetOpinion` (NATIVE_COMBAT_GROUNDWORK.md section 6: Enemy 0, Neutral 1, Friendly 2).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opinion {
    Enemy,
    Neutral,
    Friendly,
}

/// What kind of actor a candidate is, as far as `ValidTarget` asks.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    /// A pawn: whether a controller possesses it, whether it drives a vehicle, whether it is a vehicle weapon seat
    /// (`WillowWeaponPawn`).
    Pawn {
        controlled: bool,
        driving: bool,
        weapon_seat: bool,
    },
    /// A vehicle: whether any seat has an occupant.
    Vehicle { occupied: bool },
    /// Any other targetable actor.
    Other,
}

/// What the rules read of one actor now ([`super::Senses::facts`]).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActorFacts {
    pub location: Vec3,
    pub rotation: Rotator,
    pub kind: ActorKind,
    /// The actor's own "can be targeted" answer (`ITargetable`; the engine's).
    pub can_be_targeted: bool,
    /// Whether it has an allegiance object.
    pub has_allegiance: bool,
    /// `WillowPawn.bCloaked` (a non-pawn: false).
    pub cloaked: bool,
    /// `Actor.bHidden`.
    pub hidden: bool,
    /// `Actor.bDeleteMe`.
    pub deleted: bool,
}

impl ActorFacts {
    pub fn new(location: Vec3, rotation: Rotator, kind: ActorKind) -> Self {
        ActorFacts {
            location,
            rotation,
            kind,
            can_be_targeted: true,
            has_allegiance: true,
            cloaked: false,
            hidden: false,
            deleted: false,
        }
    }
}

/// One `WillowMindTargetInfo` (the fields part 1 fills; NATIVE_AI_MELEE.md, AI_MELEE_TARGETING_LIVE.md 3.3).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub target: ObjRef,
    pub distance: f32,
    pub distance_2d: f32,
    pub distance_vertical: f32,
    pub dot_to_target: f32,
    pub dot_from_target: f32,
    pub dot_target_facing: f32,
    pub exposure_to_me: f32,
    pub exposure: Exposure,
    pub exposure_change_time: f32,
    pub can_see_or_hear: bool,
    pub can_fire_at: bool,
    pub threat: ThreatLevel,
    pub priority: f32,
    pub one_priority: f32,
    pub bonus_priority: f32,
    pub aggro_time: f32,
    pub forget_time: f32,
    pub last_known_location: Vec3,
}

impl Record {
    /// A fresh record of `target`. Its level is `ETL_InThreatRadius`: the code writes `ETL_NoThreat`, but the first
    /// level the game shows for a new record is 1 and only "no record" answers 0 (L9 section 2; the writer of the 1
    /// was not located). Everything else is zero, `Exposure_None`.
    pub fn new(target: ObjRef) -> Self {
        Record {
            target,
            distance: 0.0,
            distance_2d: 0.0,
            distance_vertical: 0.0,
            dot_to_target: 0.0,
            dot_from_target: 0.0,
            dot_target_facing: 0.0,
            exposure_to_me: 0.0,
            exposure: Exposure::None,
            exposure_change_time: 0.0,
            can_see_or_hear: false,
            can_fire_at: false,
            threat: ThreatLevel::InThreatRadius,
            priority: 0.0,
            one_priority: 0.0,
            bonus_priority: 0.0,
            aggro_time: 0.0,
            forget_time: 0.0,
            last_known_location: [0.0; 3],
        }
    }
}

/// A Knowledge iterator of `TargetingDefinition.Knowledge`, by class.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knowledge {
    /// `WillowGame.WillowTI_CalcThreat`.
    Threat,
    /// `WillowGame.WillowTI_CalcCanFire`.
    CanFire,
    /// `WillowGame.WillowTI_CalcExposure`.
    Exposure,
    /// `WillowGame.WillowTI_CalcDistances`.
    Distances,
    /// `WillowGame.WillowTI_CalcCombat`: no native body was found, so it behaves as the base `TI_Calc`, which does
    /// nothing (NATIVE_AI_AWARENESS.md, UNVERIFIED).
    Combat,
    /// An entry that holds no object (the live definition's fifth entry, L8).
    Empty,
}

/// A Prioritization iterator of `TargetingDefinition.Prioritization`, by class, with its fields.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Prioritizer {
    /// `TargetIteratorPrioritizeExposedTargets`: `TargetExposureToMe`, unclamped (L8 saw -1.0 pass through).
    Exposed,
    /// `TargetIteratorPrioritizeNearbyTarget`: `clamp(1 - Distance / MaxDistance, 0, 1)` with the 3D distance (L8).
    Nearby { max_distance: f32 },
    /// `TargetIteratorPrioritizePreviousTarget`: 1 for the current target, else 0 (L8).
    Previous,
    /// `TI_PrioritizeBonus`: `BonusPriority` (plus an AI-interface term the game showed as 0, L8).
    Bonus,
    /// `WillowTI_PrioritizeIdle`: `clamp(1 - count / MaxAICurrentlyTargeting, 0, 1)` (G25, UNVERIFIED).
    Idle { max_targeting: Option<f32> },
    /// `WillowTI_PrioritizeAttacking`: a damage-share meter over the hit history (G25); not modelled.
    Attacking,
}

/// What the rules read from the definitions of one component (the VM adapter fills it).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// `AIComponent.UpdateRate`.
    pub update_rate: f32,
    /// `TargetingDefinition.SearchRadius` when `bOverrideSearchRadius`, else `AIDefinition.TargetSearchRadius`.
    pub search_radius: f32,
    /// `WillowMind.SightViewCone` (a cosine threshold).
    pub sight_view_cone: f32,
    /// `GlobalsDefinition.AICloseThreatRadius`.
    pub close_threat_radius: f32,
    /// The AI pawn's collision radius; `None` when it has no collision component (the threat rules' gate).
    pub collision_radius: Option<f32>,
    /// `WillowMind.MinAggroTimeToConsiderAwareOfTarget`.
    pub min_aggro_time: f32,
    /// How long a level-2 notice lasts (`ForgetTime`): `MinEngagementTime` of a den definition, else 2.5 s.
    pub seen_time: f32,
    /// How long a provocation lasts: `MinProvokedTime` of a den definition, else 10.0 s.
    pub provoked_time: f32,
    pub knowledge: Vec<Knowledge>,
    /// Each iterator with its `Weight`.
    pub prioritization: Vec<(Prioritizer, f32)>,
    /// `TargetingDefinition.bCanTargetFriendlies`.
    pub can_target_friendlies: bool,
    /// Whether `CanTargetIf`, `StopTargetingIf` and `CanTargetFriendliesIf` are set (evaluators; not run here).
    pub can_target_if: bool,
    pub stop_targeting_if: bool,
    pub can_target_friendlies_if: bool,
    /// Whether the component has a parent component (a den member).
    pub has_parent: bool,
}

impl Settings {
    /// The settings of a lone bullymong as the records give them (AI_MELEE_TARGETING_LIVE.md section 2,
    /// AI_AWARENESS_LIVE.md section 8; the live iterator weights, not the cooked ones), for tests and examples.
    pub fn live_bullymong(collision_radius: f32) -> Self {
        Settings {
            update_rate: 0.2,
            search_radius: 4096.0,
            sight_view_cone: -4.371e-8,
            close_threat_radius: 768.0,
            collision_radius: Some(collision_radius),
            min_aggro_time: 0.1,
            seen_time: 2.5,
            provoked_time: 10.0,
            knowledge: vec![
                Knowledge::Threat,
                Knowledge::CanFire,
                Knowledge::Exposure,
                Knowledge::Distances,
                Knowledge::Empty,
            ],
            prioritization: vec![
                (Prioritizer::Idle { max_targeting: None }, 0.0),
                (Prioritizer::Attacking, 0.0),
                (Prioritizer::Nearby { max_distance: 4096.0 }, 1.0),
                (Prioritizer::Exposed, 1.0),
                (Prioritizer::Previous, 0.4),
                (Prioritizer::Bonus, 1.0),
            ],
            can_target_friendlies: false,
            can_target_if: false,
            stop_targeting_if: false,
            can_target_friendlies_if: false,
            has_parent: false,
        }
    }
}

/// One AI component's awareness state (the fields of `AIComponent` / `WillowAIComponent` and `WillowMind` part 1
/// reads and writes).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    /// The AI pawn the component's mind controls.
    pub pawn: ObjRef,
    pub settings: Settings,
    /// `Targets`, in discovery order.
    pub records: Vec<Record>,
    /// `CurrentTarget`.
    pub current: Option<ObjRef>,
    /// `ScriptedTarget`: when set, only that actor is a valid current target.
    pub scripted_target: Option<ObjRef>,
    /// `LastStateUpdateTime` / `LastTargetUpdateTime`.
    pub last_state_update: f32,
    pub last_target_update: f32,
    /// Whether `Holds` has an entry (the component is on hold).
    pub on_hold: bool,
    /// `WillowMind.bProvoked` / `ProvokedLocation`.
    pub provoked: bool,
    pub provoked_location: Vec3,
    /// `LastAttackedTime`, `LastHurtTime`, `LastAttacker` (`WillowMind.NotifyAttackedBy`).
    pub last_attacked_time: f32,
    pub last_hurt_time: f32,
    pub last_attacker: Option<ObjRef>,
    /// Whether the first tick's initialisation ran.
    pub initialised: bool,
}

impl Component {
    pub fn new(pawn: ObjRef, settings: Settings) -> Self {
        Component {
            pawn,
            settings,
            records: Vec::new(),
            current: None,
            scripted_target: None,
            last_state_update: 0.0,
            last_target_update: 0.0,
            on_hold: false,
            provoked: false,
            provoked_location: [0.0; 3],
            last_attacked_time: 0.0,
            last_hurt_time: 0.0,
            last_attacker: None,
            initialised: false,
        }
    }

    /// The record of `target`, if any (one record per actor).
    pub fn record(&self, target: ObjRef) -> Option<&Record> {
        self.records.iter().find(|r| r.target == target)
    }

    pub fn record_mut(&mut self, target: ObjRef) -> Option<&mut Record> {
        self.records.iter_mut().find(|r| r.target == target)
    }
}
