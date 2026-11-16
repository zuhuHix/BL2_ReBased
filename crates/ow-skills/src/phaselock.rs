//! Owner: lane W6-SKILLS. Phaselock: the lock duration `5 + 0.5 g` as the attribute rule gives it, the cast's
//! schedule, the auto-aim target score that picks what is lifted, the hover bob, and `VInterpTo`, the one native the
//! installed lift script needs that no pack binds. AI-assisted (Claude Code).
//!
//! **The lift runs as installed script** (architecture R2): `LiftActionSkill.GetLiftLocation`,
//! `GetBobLocation` and `UpdateLiftedPawn` run on the VM, with the world's and the Core natives plus [`Pack`]'s
//! `VInterpTo`; there is no `ow-ai` edge (the lifted pawn is moved by that script). What here restates script is an
//! oracle the installed-game run compares the script against ([`Schedule`], [`bob_target_z`]), not a second path the
//! game runs.
//!
//! **Evidence.**
//! - The duration: `Att_Phaselock_Duration` is a designer attribute whose base is 5; Suspension adds PostAdd `0.5 g`
//!   on its `Value` stack (data, PHASELOCK_STOCK_DATA.md; the engine rule, NATIVE_ATTRIBUTES.md section 5). The stack
//!   rule is `ow-attr`'s. UNVERIFIED in the game for g > 0 (no Suspension cast was timed).
//! - The schedule: script formulas, data numbers (NATIVE_ACTION_SKILL.md section 7). **Confirmed live** for the base
//!   first lock (release 5.7 s and end 6.7 s, within about two frames, two casts, 2026-11-12); the other rows were not
//!   seen.
//! - The bob: **confirmed live** (2026-10-02: this rule on the game's own frame times reproduced a lifted bullymong's
//!   height over 435 frames with RMS error 0.001 uu; NATIVE_PHASELOCK_TARGETING.md section 5).
//! - The target score: read from the executable (NATIVE_PHASELOCK_TARGETING.md sections 2-3), UNVERIFIED in the game.
//!   Its arithmetic precision is not read; it is done in doubles through the `ow_core::num` choke points.

use std::rc::Rc;

use ow_attr::stack::{CombineForm, Modifier, ModifierId, ModifierType, NumKind, combine};
use ow_combat::hit::bind;
use ow_core::NotYet;
use ow_core::num::{ln_f64, sqrt_f64};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;

/// The record of the schedule.
pub const RECORD: &str = "NATIVE_ACTION_SKILL.md section 7";
/// The record of the target score and the bob.
pub const RECORD_TARGETING: &str = "NATIVE_PHASELOCK_TARGETING.md sections 2-3, 5";

/// The lock duration attribute's base (`Att_Phaselock_Duration`, data; PHASELOCK_STOCK_DATA.md).
pub const LOCK_BASE: f32 = 5.0;
/// Suspension's base and per-grade value on the lock duration (data; PHASELOCK_STOCK_DATA.md).
pub const SUSPENSION_PER_GRADE: f32 = 0.5;
/// The pi the bob script multiplies by: the float constant in the installed `GetBobLocation` is the single nearest
/// to pi (`0x40490FDB`), which script listings print as `3.14159` (six significant digits), the form
/// NATIVE_PHASELOCK_TARGETING.md section 5 quotes. Found in the installed bytecode (a local check, decision
/// 2026-11-16-42); with `3.14159f32` the script's bob differs by one or more ulps.
pub const SCRIPT_PI: f32 = std::f32::consts::PI;

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

/// Maya's lock duration with Suspension at `grade` (0 for untrained): Suspension's PostAdd is its grade rule's value
/// (start 1, interval 1: `0.5 + 0.5 x (g - 1)`), so the duration is `5 + 0.5 g`.
pub fn maya_lock_duration(grade: i32) -> f32 {
    if grade < 1 {
        return lock_duration(LOCK_BASE, &[]);
    }
    // grade >= 1: the effect's steps are grade - 1 (int -> float, exact for any grade a tree holds).
    let post = SUSPENSION_PER_GRADE + SUSPENSION_PER_GRADE * (grade - 1) as f32;
    lock_duration(LOCK_BASE, &[(ModifierType::PostAdd, post)])
}

// ---------------------------------------------------------------------------------------------------------------
// The schedule (an oracle for the installed script)
// ---------------------------------------------------------------------------------------------------------------

/// The timers a cast on a liftable target sets, in seconds after the cast (section 7): the actor's
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
// The bob (an oracle for the installed `GetBobLocation`)
// ---------------------------------------------------------------------------------------------------------------

/// The bob's target height: `LiftEndLocation.Z + LiftBobAmplitude x sin((now - SkillStartTime) x LiftBobFrequency x
/// pi)` ([`SCRIPT_PI`]), before the `VInterpTo` smoothing (section 5). Single precision per operation, with `sin` in double
/// through the choke point, as the VM's `Sin` native.
pub fn bob_target_z(end_z: f32, amplitude: f32, frequency: f32, elapsed: f32) -> f32 {
    let angle = elapsed * frequency * SCRIPT_PI;
    // f64 -> f32: the sine of a finite angle is within [-1, 1], always representable (the VM's Sin rounds so).
    let sine = ow_core::num::sin_f64(f64::from(angle)) as f32;
    end_z + amplitude * sine
}

/// `VInterpTo` on one axis set: the target when `speed <= 0` or the squared distance is below 0.0001, else the
/// current value moved by `clamp(dt x speed, 0, 1)` of the remaining difference (NATIVE_PHASELOCK_TARGETING.md
/// section 5). Each component in double, rounded once to single, the form of the VM's `FInterpTo`. Confirmed in the
/// game as part of the bob rule (above).
pub fn vinterp_to(current: [f64; 3], target: [f64; 3], dt: f64, speed: f64) -> [f64; 3] {
    let d = [
        target[0] - current[0],
        target[1] - current[1],
        target[2] - current[2],
    ];
    if speed <= 0.0 || d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < 0.0001 {
        return target.map(round_single);
    }
    let alpha = (dt * speed).clamp(0.0, 1.0);
    [0, 1, 2].map(|i| round_single(current[i] + d[i] * alpha))
}

fn round_single(x: f64) -> f64 {
    ow_core::num::f32r(x)
}

// ---------------------------------------------------------------------------------------------------------------
// The native
// ---------------------------------------------------------------------------------------------------------------

/// The function path [`Pack`] binds.
pub const VINTERP_TO: &str = "Core.Object.VInterpTo";

fn vector_of(c: &NativeCall<'_>, i: usize) -> Result<[f64; 3], VmError> {
    let v = c.arg(i)?;
    let s = v.as_struct().ok_or_else(|| bad(i, "expected a vector"))?;
    let layout =
        c.vm.space
            .classes
            .struct_layout(s.ty)
            .ok_or_else(|| bad(i, "a struct this VM does not know"))?;
    let mut out = [0.0; 3];
    for (k, n) in ["X", "Y", "Z"].iter().enumerate() {
        let entry =
            c.vm.space
                .names
                .find(n)
                .and_then(|id| layout.field(id))
                .map(|(e, _)| e)
                .ok_or_else(|| bad(i, "a vector without its axes"))?;
        out[k] = match s
            .fields
            .get(usize::try_from(entry).map_err(|_| bad(i, "an entry out of range"))?)
        {
            Some(Value::Float(x)) => f64::from(*x),
            _ => return Err(bad(i, "an axis that is not a float")),
        };
    }
    Ok(out)
}

fn float_arg(c: &NativeCall<'_>, i: usize) -> Result<f64, VmError> {
    match c.arg(i)? {
        Value::Float(x) => Ok(f64::from(*x)),
        _ => Err(bad(i, "expected a float")),
    }
}

fn bad(i: usize, why: &str) -> VmError {
    VmError::Script {
        function: "Object.VInterpTo".to_string(),
        message: format!("argument {i}: {why}"),
    }
}

fn make_vector(vm: &Vm, [x, y, z]: [f64; 3]) -> Result<Value, VmError> {
    let classes = &vm.space.classes;
    let layout = classes
        .struct_by_path("Core.Object.Vector")
        .and_then(|s| classes.struct_layout(s))
        .ok_or_else(|| {
            VmError::from(NotYet::layer_absent(
                "the Core.Object.Vector struct",
                "ow-object reflection of the Core package",
                RECORD_TARGETING,
            ))
        })?;
    let mut value = layout.zero().clone();
    let Some(s) = value.struct_mut() else {
        return Err(VmError::Decode(
            "Core.Object.Vector: the zero value is not a struct".to_string(),
        ));
    };
    for (n, v) in [("X", x), ("Y", y), ("Z", z)] {
        let entry = vm
            .space
            .names
            .find(n)
            .and_then(|id| layout.field(id))
            .map(|(e, _)| e as usize)
            .filter(|&e| e < s.fields.len())
            .ok_or_else(|| VmError::Decode(format!("Core.Object.Vector has no field {n}")))?;
        // A rounded double holds a single exactly (vinterp_to rounds each axis once).
        s.fields[entry] = Value::Float(v as f32);
    }
    Ok(value)
}

/// `VInterpTo` as a [`NativePack`], bound by path (`ow_combat::hit::bind`): skipped when the VM does not declare it,
/// and the first binding of the key wins, so a Core pack that binds it later takes precedence only if registered first.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        bind(
            vm,
            VINTERP_TO,
            Rc::new(|c: &mut NativeCall<'_>| {
                let (current, target) = (vector_of(c, 0)?, vector_of(c, 1)?);
                let (dt, speed) = (float_arg(c, 2)?, float_arg(c, 3)?);
                make_vector(c.vm, vinterp_to(current, target, dt, speed))
            }),
        )
    }
}

#[cfg(test)]
mod installed;
#[cfg(test)]
mod tests;
