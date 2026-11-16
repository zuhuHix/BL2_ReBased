//! Owner: lane W6-SKILLS. Phaselock: the lock duration as the attribute stack rule gives it, the cast's schedule (an
//! oracle), and the auto-aim target score that picks what is lifted. AI-assisted (Claude Code).
//!
//! **The lift runs as installed script** (architecture R2): `LiftActionSkill.GetLiftLocation`, `GetBobLocation` and
//! `UpdateLiftedPawn` run on the VM with the world's and the Core natives (`Object.VInterpTo` is `ow-vm`'s,
//! `natives_core`, review r1 of PR #342); there is no `ow-ai` edge (the lifted pawn is moved by that script). Nothing
//! here is a second path the game runs: [`Schedule`] is an oracle, and the bob's restatement lives only in the tests
//! (`oracle`, `#[cfg(test)]`).
//!
//! **Evidence.**
//! - The duration: `Att_Phaselock_Duration` is a designer attribute whose base is 5 in the data; Suspension adds
//!   PostAdd `0.5 g` on its `Value` stack (data, PHASELOCK_STOCK_DATA.md; the engine rule, NATIVE_ATTRIBUTES.md
//!   section 5). The stack rule is `ow-attr`'s; [`lock_duration`] only hands it the modifiers. Other modifiers
//!   (Thoughtlock and the rest) come through the same stack. UNVERIFIED in the game for g > 0.
//! - The schedule: script formulas, data numbers (NATIVE_ACTION_SKILL.md section 7). **Confirmed live** for the base
//!   first lock (release 5.7 s and end 6.7 s, within about two frames, two casts, 2026-11-12); the other rows were not
//!   seen.
//! - The bob: **confirmed live** to RMS 0.001 uu (2026-10-02, 435 frames; NATIVE_PHASELOCK_TARGETING.md section 5).
//! - The target score: read from the executable (NATIVE_PHASELOCK_TARGETING.md sections 2-3), UNVERIFIED in the game.
//!   Its arithmetic precision is not read; it is done in doubles through the `ow_core::num` choke points.

use ow_attr::stack::{CombineForm, Modifier, ModifierId, ModifierType, NumKind, combine};
use ow_core::num::{ln_f64, sqrt_f64};

/// The record of the schedule.
pub const RECORD: &str = "NATIVE_ACTION_SKILL.md section 7";
/// The record of the target score and the bob.
pub const RECORD_TARGETING: &str = "NATIVE_PHASELOCK_TARGETING.md sections 2-3, 5";

// ---------------------------------------------------------------------------------------------------------------
// The lock duration
// ---------------------------------------------------------------------------------------------------------------

/// The lock duration: the designer attribute's `Value` from its base and the modifiers on its stack, by the attribute
/// stack rule (`ow-attr`, NATIVE_ATTRIBUTES.md section 3; the default combine form, architecture 5.3).
pub fn lock_duration(base: f32, modifiers: &[(ModifierType, f32)]) -> f32 {
    let mods: Vec<Modifier> = modifiers
        .iter()
        .enumerate()
        .map(|(i, &(kind, value))| Modifier {
            kind,
            value,
            id: ModifierId(u64::try_from(i).unwrap_or(u64::MAX)),
        })
        .collect();
    combine(base, &mods, CombineForm::default(), NumKind::Float).to_f32()
}

// ---------------------------------------------------------------------------------------------------------------
// The schedule (an oracle for the installed script)
// ---------------------------------------------------------------------------------------------------------------

/// **An oracle, not for gameplay callers** (it restates the installed `LiftActionSkill.OnActionSkillStarted` and
/// `PhaseLockTarget`, which the game runs; the installed-game run and the tests compare against it). The timers a cast
/// on a liftable target sets, in seconds after the cast (section 7): the actor's
/// `SkillDuration = LiftDuration + LockDuration(Maya) x TimeScale(target)`, `LockTarget` at the lift's end,
/// `StartOutro` at `SkillDuration - LockFadeOutTime`, the release at `SkillDuration`, `EndSkill` at
/// `SkillDuration + ReleaseBufferTime`. Single precision, one rounding per operation, as the script's float operators.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Schedule {
    pub skill_duration: f32,
    pub lock_at: f32,
    pub outro_at: f32,
    pub release_at: f32,
    pub end_at: f32,
}

impl Schedule {
    pub fn new(lift: f32, lock: f32, scale: f32, fade_out: f32, release_buffer: f32) -> Schedule {
        let skill_duration = lift + lock * scale;
        Schedule {
            skill_duration,
            lock_at: lift,
            outro_at: skill_duration - fade_out,
            release_at: skill_duration,
            end_at: skill_duration + release_buffer,
        }
    }

    /// When the cooldown is ready: the drain starts at the release (the cooldown manager is switched off by
    /// `OnReleasedTarget`) and takes `cooldown` seconds at 1 per second.
    pub fn cooldown_ready(&self, cooldown: f32) -> f32 {
        self.release_at + cooldown
    }
}

/// The time scale of a target locked again within 25 s: base 1 with `Skill_Phaselock_DiminishingReturns`' Scale
/// -0.4 on its `PhaselockTimeScale` (data), by the attribute stack rule, where a negative Scale divides by
/// `1 + |down|`: `1 / 1.4`. **This disagrees with PHASELOCK_STOCK_DATA.md**, whose repeat row (3.7 s) takes the scale
/// as `1 - 0.4 = 0.6` from the fitted form `(base + PreAdd) x (1 + Scale) + PostAdd` it labels UNVERIFIED; the stack
/// rule read later from the executable (NATIVE_SKILLS.md 5.2, NATIVE_WEAPON_RULES.md section 1) divides. Neither was
/// seen in the game (a finding for the maintainer, decision 2026-11-16-42).
pub fn repeat_target_scale() -> f32 {
    combine(
        1.0,
        &[Modifier {
            kind: ModifierType::Scale,
            value: -0.4,
            id: ModifierId(0),
        }],
        CombineForm::default(),
        NumKind::Float,
    )
    .to_f32()
}

// ---------------------------------------------------------------------------------------------------------------
// The target score (`WillowAutoAimStrategy.GetPreferredTarget`'s search)
// ---------------------------------------------------------------------------------------------------------------

/// The auto-aim definition's numbers (`GD_Autoaim.Default`'s fields).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoAim {
    pub max_target_distance: f64,
    pub min_target_distance: f64,
    pub radius_multiplier: f64,
    pub max_snap_angle: f64,
    pub distance_offset: f64,
}

impl AutoAim {
    pub fn new(
        max_target_distance: f64,
        min_target_distance: f64,
        radius_multiplier: f64,
        max_snap_angle: f64,
        distance_offset: f64,
    ) -> Self {
        AutoAim {
            max_target_distance,
            min_target_distance,
            radius_multiplier,
            max_snap_angle,
            distance_offset,
        }
    }
}

/// One candidate as the search sees it after projection: in front of the view plane, its aspect-corrected screen
/// position (fractions of the half-screen height), its view-space depth, its auto-aim radius, and whether the line
/// of sight passes (nothing hit, the target, or an actor hard-attached to it).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub in_front: bool,
    pub x: f64,
    pub y: f64,
    pub depth: f64,
    pub radius: f64,
    pub line_of_sight: bool,
}

impl Candidate {
    pub fn new(
        in_front: bool,
        x: f64,
        y: f64,
        depth: f64,
        radius: f64,
        line_of_sight: bool,
    ) -> Self {
        Candidate {
            in_front,
            x,
            y,
            depth,
            radius,
            line_of_sight,
        }
    }
}

/// What the score of one candidate is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scored {
    /// 0 when rejected.
    pub score: f64,
    /// `s <= rT`.
    pub within_world_space_radius: bool,
}

fn log2(x: f64) -> f64 {
    ln_f64(x) / ln_f64(2.0)
}

/// The magnetism radius at `depth` before the target's own radius: `MaxSnapAngle x max(0, 1 + log2(Max) -
/// log2(max(1, depth - DistanceOffset))) / log2(Max)`.
pub fn magnetism(a: &AutoAim, depth: f64) -> f64 {
    let top = log2(a.max_target_distance);
    a.max_snap_angle * (1.0 + top - log2((depth - a.distance_offset).max(1.0))).max(0.0) / top
}

/// The score of one candidate (steps 1-8): rejected (0) behind the view plane, outside the magnetism radius
/// `rS = max(magnetism, rT)` with `rT = radius x RadiusMultiplier / (depth x tan(FOV / 2))`, outside the distance
/// gate or without line of sight; otherwise `0.5 x ((2 - depth / Max) - s / rS)` with `s = sqrt(x^2 + y^2)`.
pub fn score(a: &AutoAim, c: &Candidate, tan_half_fov: f64) -> Scored {
    let none = Scored {
        score: 0.0,
        within_world_space_radius: false,
    };
    if !c.in_front {
        return none;
    }
    let r_t = c.radius * a.radius_multiplier / (c.depth * tan_half_fov);
    let r_s = magnetism(a, c.depth).max(r_t);
    let s = sqrt_f64(c.x * c.x + c.y * c.y);
    let within = s <= r_t;
    if s > r_s
        || c.depth < a.min_target_distance
        || c.depth > a.max_target_distance
        || !c.line_of_sight
    {
        return Scored {
            score: 0.0,
            within_world_space_radius: within,
        };
    }
    Scored {
        score: 0.5 * ((2.0 - c.depth / a.max_target_distance) - s / r_s),
        within_world_space_radius: within,
    }
}

/// The best candidate: the highest score above 0, the first of equal scores; `None` when none scores.
pub fn pick_target(a: &AutoAim, candidates: &[Candidate], tan_half_fov: f64) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, c) in candidates.iter().enumerate() {
        let s = score(a, c, tan_half_fov).score;
        if s > 0.0 && best.is_none_or(|(_, b)| s > b) {
            best = Some((i, s));
        }
    }
    best.map(|(i, _)| i)
}

// ---------------------------------------------------------------------------------------------------------------
// The bob, restated for the tests only
// ---------------------------------------------------------------------------------------------------------------

/// The bob's restatement, an oracle the installed-game run compares the installed `GetBobLocation` against (test
/// only: the game runs the script).
#[cfg(test)]
pub(crate) mod oracle {
    /// The pi the bob script multiplies by: the float constant in the installed `GetBobLocation` is the single
    /// nearest to pi (`0x40490FDB`), which script listings print as `3.14159` (six significant digits), the form
    /// NATIVE_PHASELOCK_TARGETING.md section 5 quotes. Found in the installed bytecode (a local check, decision
    /// 2026-11-16-42).
    pub const SCRIPT_PI: f32 = std::f32::consts::PI;

    /// The bob's target height: `LiftEndLocation.Z + LiftBobAmplitude x sin((now - SkillStartTime) x
    /// LiftBobFrequency x pi)`, before the `VInterpTo` smoothing (section 5). Single precision per operation, with
    /// `sin` in double through the choke point, as the VM's `Sin` native.
    pub fn bob_target_z(end_z: f32, amplitude: f32, frequency: f32, elapsed: f32) -> f32 {
        let angle = elapsed * frequency * SCRIPT_PI;
        // f64 -> f32: the sine of a finite angle is within [-1, 1], always representable (the VM's Sin rounds so).
        let sine = ow_core::num::sin_f64(f64::from(angle)) as f32;
        end_z + amplitude * sine
    }

    /// `VInterpTo` as the record reads it (NATIVE_PHASELOCK_TARGETING.md section 5), written here from the record,
    /// apart from `ow-vm`'s binding: the target when `speed <= 0` or the squared distance is below 0.0001, else the
    /// current value moved by `clamp(dt x speed, 0, 1)` of the difference, each axis in double and rounded once. The
    /// precision is UNVERIFIED.
    pub fn vinterp_to(current: [f64; 3], target: [f64; 3], dt: f64, speed: f64) -> [f64; 3] {
        let d = [
            target[0] - current[0],
            target[1] - current[1],
            target[2] - current[2],
        ];
        if speed <= 0.0 || d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < 0.0001 {
            return target.map(ow_core::num::f32r);
        }
        let alpha = (dt * speed).clamp(0.0, 1.0);
        [0, 1, 2].map(|i| ow_core::num::f32r(current[i] + d[i] * alpha))
    }
}

#[cfg(test)]
mod installed;
#[cfg(test)]
mod tests;
