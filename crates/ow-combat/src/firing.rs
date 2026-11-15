//! Owner: lane W1-FIRING.
//!
//! What one shot of a weapon does, as plain maths a host calls: the port of `src/weapon_firing.*` (the C++ copy stays
//! the reference until W7-RETIRE; architecture section 6, row `ow-combat::firing`).
//!
//! Rules are own-words readings in `docs/verification/NATIVE_WEAPON_FIRING.md` (section numbers below); every one is
//! **UNVERIFIED** in game unless `docs/verification/WEAPON_PROJECTILES.md` says how it was confirmed; each function's
//! `Status:` line cites that record's row (section 3 table, cited by its rule) or section 10, with the record's own
//! status. Porting a rule changes nothing about what is known about it.
//!
//! Conventions, kept from the C++ so results are bit-equal on the reference target (architecture 5.1 N1):
//! - Arithmetic is `f64`, in the C++ operation order (left to right, no reassociation, no fused multiply-add).
//! - Powers and roots go through `ow_core::num` (5.1 N3); float-to-int conversion uses `ow_core::num::trunc_i32_f64`
//!   (MSVC `int(double)`: `i32::MIN` for NaN and out of range), never `as` (5.1 N5).
//! - `std::min`/`std::max`/`std::clamp` are reproduced by [`cmin`], [`cmax`] and [`cclamp`], which keep the C++
//!   comparison order (so a NaN or a signed zero comes out the way the C++ returns it); `f64::min`/`max`/`clamp`
//!   would not.
//! - Angles are UE3 rotator units (65,536 per turn) unless a name says degrees. Random numbers are passed in, so the
//!   caller chooses the generator (a `GameRng` stream, `CrtStream::FiringSpread`) and tests can pin them.
//!
//! Not here (lane entry, "Not yet"): weapon state, ammo pools, projectiles in a world, status rolls against a world,
//! beams and ricochets. Those belong to W6-COMBAT and later lanes.

use ow_core::num::{pow_f64, sqrt_f64, trunc_i32_f64};

// --- C++ comparison helpers ------------------------------------------------------------------------------------------

/// `std::min(a, b)`: `(b < a) ? b : a`.
fn cmin(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// `std::max(a, b)`: `(a < b) ? b : a`.
fn cmax(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// `std::clamp(v, lo, hi)` for `lo <= hi`: `lo` when `v < lo`, `hi` when `hi < v`, else `v` (a NaN `v` comes back as
/// `v`). MSVC's `std::clamp` tests `hi < v` first (libstdc++ tests `v < lo` first); the two orders give the same result
/// whenever `lo <= hi`, which every call site keeps, as the C++ requires.
fn cclamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if hi < v {
        hi
    } else {
        v
    }
}

/// An aim offset in rotator units, applied in the aim's local frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Offset {
    pub pitch: f64,
    pub yaw: f64,
}

/// One `FiringModeDefinition.FiringPatternLines` entry (rotator units).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PatternLine {
    pub start_pitch: i32,
    pub start_yaw: i32,
    pub end_pitch: i32,
    pub end_yaw: i32,
    pub start_only: bool,
}

/// A firing pattern. Defaults are the class defaults the C++ uses.
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    pub lines: Vec<PatternLine>,
    /// `bFireRandomlyFromPattern`.
    pub fire_randomly: bool,
    /// `bScalePatternByWeaponSpread`.
    pub scale_by_spread: bool,
    /// Class default 1.
    pub base_pattern_spread: f64,
    pub min_pattern_scale: f64,
}

impl Default for Pattern {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            fire_randomly: false,
            scale_by_spread: false,
            base_pattern_spread: 1.0,
            min_pattern_scale: 0.0,
        }
    }
}

/// Section 1.3. `index` is the weapon's `NextFiringPatternIndex` (a script int) and is advanced; `pick` (0..1) chooses
/// the line when firing randomly; `t` (0..1) is the point on the line. No lines: no offset and `index` untouched. An
/// `index` (or a random pick) below 0 or past the last line starts again at line 0, as the C++.
/// Status: read from native code (NATIVE_WEAPON_FIRING.md 1.3); WEAPON_PROJECTILES.md section 3, row "Pattern": the
/// cooked lines equal the live dump; the offset rule itself is UNVERIFIED in game.
pub fn pattern_offset(
    pattern: &Pattern,
    spread_degrees: f64,
    index: &mut i32,
    pick: f64,
    t: f64,
) -> Offset {
    let count = pattern.lines.len();
    if count == 0 {
        return Offset::default();
    }
    // C++: int at = fireRandomly ? std::min(int(pick * count), count - 1) : index; if (at < 0 || at >= count) at = 0;
    // A line count is far below 2^31, so it converts exactly, as the C++ `int(lines.size())` does.
    let count_i = i32::try_from(count).unwrap_or(i32::MAX);
    let at: i32 = if pattern.fire_randomly {
        trunc_i32_f64(pick * f64::from(count_i)).min(count_i - 1)
    } else {
        *index
    };
    let at = if at < 0 || at >= count_i {
        0
    } else {
        usize::try_from(at).unwrap_or(0)
    };
    let line = &pattern.lines[at];
    // One t for both components; each is truncated to a whole rotator unit.
    let mut offset = if line.start_only {
        Offset {
            pitch: f64::from(line.start_pitch),
            yaw: f64::from(line.start_yaw),
        }
    } else {
        Offset {
            pitch: (f64::from(line.start_pitch)
                + f64::from(line.end_pitch.wrapping_sub(line.start_pitch)) * t)
                .trunc(),
            yaw: (f64::from(line.start_yaw)
                + f64::from(line.end_yaw.wrapping_sub(line.start_yaw)) * t)
                .trunc(),
        }
    };
    if pattern.scale_by_spread && pattern.base_pattern_spread > 0.0 {
        let scale = cmax(
            spread_degrees / pattern.base_pattern_spread,
            pattern.min_pattern_scale,
        );
        offset.pitch *= scale;
        offset.yaw *= scale;
    }
    // `at` < count <= i32::MAX, so the sum and the conversion back are exact.
    *index = i32::try_from((at + 1) % count).unwrap_or(0);
    offset
}

/// Section 1.4: yaw uniform across a disc of diameter `spread_degrees`, pitch uniform on the chord at that yaw.
/// `r1`, `r2` in [0, 1). Status: **confirmed live** 2026-11-03 (WEAPON_PROJECTILES.md section 3, row "Spread", and
/// section 10: 420 `AddSpread` draws, yaw and pitch/chord SDs of a uniform draw, none outside the disc).
pub fn spread_offset(spread_degrees: f64, r1: f64, r2: f64) -> Offset {
    if spread_degrees == 0.0 {
        return Offset::default();
    }
    // C++: std::trunc(spreadDegrees * 65536.0 / 360.0), in that order (not times RotatorUnitsPerDegree).
    let a = (spread_degrees * 65536.0 / 360.0).trunc();
    let u = 0.5 - r1;
    let chord = sqrt_f64(cmax(0.0, 1.0 - 4.0 * u * u));
    Offset {
        pitch: (0.5 - r2) * chord * a,
        yaw: u * a,
    }
}

/// Section 1.5. Spin-up: the interval moves linearly from `max(multiplier * base, base)` to `base` as the percent goes
/// 0 -> 1. Status: read from native code only (WEAPON_PROJECTILES.md section 3, row "Spin-up"); UNVERIFIED in game.
pub fn fire_interval(
    base: f64,
    spins_up_to_full_rate: bool,
    start_multiplier: f64,
    spin_percent: f64,
) -> f64 {
    if !spins_up_to_full_rate {
        return base;
    }
    let start = cmax(start_multiplier * base, base);
    start - (start - base) * cclamp(spin_percent, 0.0, 1.0)
}

/// Section 1.5: the spin percent after `dt` seconds; a zero duration snaps. Status: UNVERIFIED.
pub fn tick_spin(
    percent: f64,
    dt: f64,
    firing: bool,
    spin_up_duration: f64,
    spin_down_duration: f64,
) -> f64 {
    let percent = if firing {
        if spin_up_duration <= 0.0 {
            1.0
        } else {
            percent + dt / spin_up_duration
        }
    } else if spin_down_duration <= 0.0 {
        0.0
    } else {
        percent - dt / spin_down_duration
    };
    cclamp(percent, 0.0, 1.0)
}

/// Section 1.5: the burst interval follows the current fire interval. Status: UNVERIFIED.
pub fn burst_interval(interval: f64, current_fire_interval: f64, base_fire_interval: f64) -> f64 {
    if base_fire_interval > 0.0 {
        interval * current_fire_interval / base_fire_interval
    } else {
        interval
    }
}

/// Section 1.5 refire (`ShouldRefire`): with `AutomaticBurstCount` N > 0 a press fires N shots even if the trigger is
/// let go, then waits for a new press; N = 0 fires while the trigger is held. The state belongs to the weapon in hand:
/// the host resets it when the weapon changes or is holstered. Status: consistent with live (WEAPON_PROJECTILES.md
/// section 3, row "AutomaticBurstCount": count-1 guns fired once per hold, count-0 guns repeatedly); bursts of more
/// than one shot are not measured, UNVERIFIED.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Burst {
    /// Shots still owed by the burst in progress.
    pub left: i32,
    /// The burst finished; the trigger must be pressed again.
    pub done: bool,
}

impl Burst {
    pub fn press(&mut self) {
        self.done = false;
    }
    /// A different weapon in hand: a fresh state.
    pub fn reset(&mut self) {
        self.left = 0;
        self.done = false;
    }
    /// Reload, death, menus: owed shots dropped, a new press needed.
    pub fn interrupt(&mut self) {
        self.left = 0;
        self.done = true;
    }
    pub fn wants_fire(&self, held: bool) -> bool {
        (held && !self.done) || self.left > 0
    }
    pub fn active(&self) -> bool {
        self.left > 0
    }
    /// After a shot of a weapon with burst count `count`; true when this shot ended a burst of more than one shot (the
    /// next shot waits for the burst interval).
    pub fn shot(&mut self, count: i32) -> bool {
        if count <= 0 {
            self.left = 0;
            return false;
        }
        self.left = if self.left > 0 {
            self.left - 1
        } else {
            count - 1
        };
        if self.left > 0 {
            return false;
        }
        self.done = true;
        count > 1
    }
}

/// Section 1.7: flat additions are divided among pellets, the scaled base is not. Not called by a host yet (the
/// retired UE5 host fired the evaluated card damage); kept for when skills and relics add modifiers at run time (the
/// split was seen live with a shield skill). Status: **confirmed live** (WEAPON_PROJECTILES.md section 3, row
/// "Per-pellet damage": a skill's flat addition split over 7 Jakobs pellets).
pub fn multi_projectile_damage(
    base: f64,
    pre_add: f64,
    post_add: f64,
    scale_up: f64,
    scale_down: f64,
    pellets: i32,
) -> f64 {
    let scale = (1.0 + scale_up) / (1.0 - scale_down);
    base * scale + (pre_add * scale + post_add) / f64::from(pellets.max(1))
}

/// Section 3.1 radius falloff, as fractions of the radius. Defaults are the class defaults 0.2 / 0.8 / 0.2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Falloff {
    pub max_damage_radius: f64,
    pub min_damage_radius: f64,
    pub min_damage_percent: f64,
}

impl Default for Falloff {
    fn default() -> Self {
        Self {
            max_damage_radius: 0.2,
            min_damage_radius: 0.8,
            min_damage_percent: 0.2,
        }
    }
}

/// Section 3.1 (`WillowDamageTypeDefinition.CalcRadiusDamageScale` with the stock linear fallback). Returns 0 outside
/// the radius. Status: **confirmed live** for the full-damage flag (WEAPON_PROJECTILES.md section 3, row "Splash":
/// the same damage at all 21 distances) and for the band formula (row "Falloff otherwise": `CalcRadiusDamageScale`
/// returned 0.603975 where the formula gives 0.603976); the "not handled" linear fallback is UNVERIFIED.
pub fn radius_damage_scale(
    distance: f64,
    radius: f64,
    falloff: &Falloff,
    full_damage: bool,
) -> f64 {
    if radius <= 0.0 || distance > radius {
        return 0.0;
    }
    if full_damage {
        return 1.0;
    }
    let f = distance / radius;
    let band = falloff.min_damage_radius - falloff.max_damage_radius;
    let mut scale = -1.0;
    if band > 0.0 {
        if f < falloff.max_damage_radius {
            scale = 1.0;
        } else if f <= falloff.min_damage_radius {
            scale = falloff.min_damage_percent
                + (1.0 - falloff.min_damage_percent)
                    * (1.0 - (f - falloff.max_damage_radius) / band);
        } else if f < 1.0 {
            scale = falloff.min_damage_percent;
        }
    }
    if scale < 0.0 {
        scale = 1.0 - f; // "not handled": the stock linear falloff
    }
    cmax(0.0, scale)
}

/// Section 3.2: a formula radius of 0 becomes 256. Status: UNVERIFIED.
pub fn explosion_radius(formula_radius: f64) -> f64 {
    if formula_radius == 0.0 {
        256.0
    } else {
        formula_radius
    }
}

/// Section 4.2: the product of the percent factors, times 0.01, clamped to [0, 1]. The hit path passes the radius
/// scale times [`status_level_scale`] as `extra_scale`; friendly fire, skills and relics are not modelled and each
/// counts as one. Status: UNVERIFIED (WEAPON_PROJECTILES.md section 3, row "Status chance": the live proc counts run above the
/// card chance and the missing factor is not settled; sections 10 and 13; the native roll was not read).
pub fn status_chance(
    base_chance_percent: f64,
    target_modifier: f64,
    weapon_base_modifier: f64,
    weapon_chance_modifier: f64,
    fire_interval_factor: f64,
    extra_scale: f64,
) -> f64 {
    let percent = base_chance_percent
        * target_modifier
        * weapon_base_modifier
        * weapon_chance_modifier
        * fire_interval_factor
        * extra_scale;
    cclamp(percent * 0.01, 0.0, 1.0)
}

/// One row of `GlobalsDefinition.StatusEffectChanceScales`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelChanceScale {
    pub level_difference: i32,
    pub higher: f64,
    pub lower: f64,
}

impl Default for LevelChanceScale {
    fn default() -> Self {
        Self {
            level_difference: 0,
            higher: 1.0,
            lower: 1.0,
        }
    }
}

/// Section 4.2 LevelScale. With n = min(|instigator - target|, rows), row n - 1; an instigator above the target takes
/// `higher`, below takes `lower`; equal levels (or no rows) give 1.
/// Status: UNVERIFIED (WEAPON_PROJECTILES.md section 3, row "Status chance x the level-difference scale"): the
/// mapping and the row choice come from the data; equal levels did not give x1 with skills on (section 10) and were
/// consistent with x1 with skills off (section 13, small sample); the higher-level scale is unmeasured.
pub fn status_level_scale(
    instigator_level: i32,
    target_level: i32,
    rows: &[LevelChanceScale],
) -> f64 {
    // i64 so an extreme pair cannot overflow (the C++ int subtraction would be undefined there).
    let difference = (i64::from(instigator_level) - i64::from(target_level)).abs();
    if difference == 0 || rows.is_empty() {
        return 1.0;
    }
    let n = difference.min(i64::try_from(rows.len()).unwrap_or(i64::MAX));
    let row = &rows[usize::try_from(n - 1).unwrap_or(0)];
    if instigator_level > target_level {
        row.higher
    } else {
        row.lower
    }
}

/// Section 4.2 fire-interval factor: r = current interval / type base interval, raised to `short_power` for a faster
/// gun (r < 1) and `long_power` for a slower one. Status: UNVERIFIED.
pub fn fire_interval_chance_factor(
    current_interval: f64,
    type_base_interval: f64,
    short_power: f64,
    long_power: f64,
) -> f64 {
    if type_base_interval == 0.0 {
        return 1.0;
    }
    let r = current_interval / type_base_interval;
    if r == 1.0 {
        return 1.0;
    }
    pow_f64(r, if r < 1.0 { short_power } else { long_power })
}

/// Section 4.2 ExtraScale for radius damage: `1 - clamp(distance - cylinder radius, 0, R) / R`. Status: UNVERIFIED.
pub fn radius_status_scale(distance: f64, target_radius: f64, radius: f64) -> f64 {
    if radius <= 0.0 {
        return 0.0;
    }
    1.0 - cclamp(distance - target_radius, 0.0, radius) / radius
}

/// A world-frame vector in uu (Z-up; the C++ `Vec`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// One projectile or light bullet in flight.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flight {
    pub position: Vec3,
    pub velocity: Vec3,
    pub age: f64,
}

/// What moves a [`Flight`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    /// uu/s^2, world frame (rockets: AccelRate along the launch direction).
    pub acceleration: Vec3,
    /// uu/s^2 (negative), already times the projectile's GravityScaling.
    pub gravity_z: f64,
    /// 1/s (light bullets).
    pub air_friction: f64,
    /// 0 = no cap.
    pub max_speed: f64,
}

/// Flight of one projectile or light bullet for `dt` seconds. Rockets (section 2.1): constant acceleration along the
/// launch direction, gravity scaled by GravityScaling. Light bullets (section 2.2): the manager's flight was not read;
/// AirFriction changes the speed in proportion to the speed each frame, `v *= 1 - F dt` (Torgue gyrojets carry
/// -10 / -6: the live fit, 2026-10-23, grew x(1 + 10 dt) per frame, uncapped, no gravity). Semi-implicit Euler: the
/// velocity is updated, then the position moves with the new velocity. Status (WEAPON_PROJECTILES.md section 3):
/// **confirmed live** that light bullets fly straight at constant speed with no gravity (row "Light bullets"), the
/// AirFriction factor x(1 - F dt) per frame, uncapped (row "AirFriction"), and the rocket MaxSpeed (row "Rocket
/// MaxSpeed"); weapon rockets were seen not to fall at all (row "Weapon rockets do not fall"), so a caller passes
/// `gravity_z` 0 for them; the integration scheme itself is the host's, UNVERIFIED.
pub fn step(flight: &mut Flight, motion: &Motion, dt: f64) {
    let v = flight.velocity;
    let drag = 1.0 - motion.air_friction * dt;
    let mut v = Vec3 {
        x: v.x * drag + motion.acceleration.x * dt,
        y: v.y * drag + motion.acceleration.y * dt,
        z: v.z * drag + (motion.acceleration.z + motion.gravity_z) * dt,
    };
    if motion.max_speed > 0.0 {
        let speed = sqrt_f64(v.x * v.x + v.y * v.y + v.z * v.z);
        if speed > motion.max_speed {
            let k = motion.max_speed / speed;
            v = Vec3 {
                x: v.x * k,
                y: v.y * k,
                z: v.z * k,
            };
        }
    }
    flight.velocity = v;
    flight.position = Vec3 {
        x: flight.position.x + v.x * dt,
        y: flight.position.y + v.y * dt,
        z: flight.position.z + v.z * dt,
    };
    flight.age += dt;
}

/// Momentum of a bullet or trace hit (section 1.2, NATIVE_WEAPON_TRIGGER_AMMO `ProcessInstantHitBullet`): the weapon
/// type's InstantHitMomentum is multiplied by 100 once at weapon setup. Status (WEAPON_PROJECTILES.md section 3, row
/// "Per-hit momentum"): the type values match live reads; the x100 factor is read from native code (the C++ header
/// also notes 800 read for a type value of 8); the effect of the hit on a pawn is not measured, UNVERIFIED.
pub fn hit_momentum(instant_hit_momentum: f64) -> f64 {
    instant_hit_momentum * 100.0
}

/// The victim's velocity change: momentum / Mass, or momentum x 100 when Mass <= 0.01 (engine `Pawn.TakeDamage`,
/// NATIVE_DAMAGE_DEATH 0.1; the upward part some damage types add is not included). Status: UNVERIFIED
/// (WEAPON_PROJECTILES.md section 3, row "A hit changes the target's velocity").
pub fn momentum_velocity(momentum: &Vec3, mass: f64) -> Vec3 {
    let scale = if mass > 0.01 { 1.0 / mass } else { 100.0 };
    Vec3 {
        x: momentum.x * scale,
        y: momentum.y * scale,
        z: momentum.z * scale,
    }
}

/// The damage-over-time period measured in the running game (2026-10-23, 94 ticks; WEAPON_PROJECTILES.md).
pub const DOT_PERIOD: f64 = 0.33;

/// Damage-over-time clock of one status effect application, as measured in the running game: the elapsed time builds
/// up each frame; once it reaches the period a tick deals dps x the time built up and the clock drops by one period,
/// so a frame's overshoot is paid again by the next tick. `dt` is already clamped by the caller to the effect's
/// remaining time. Returns this frame's tick damage, 0 when none. Status: **confirmed live** on 94 ticks
/// (WEAPON_PROJECTILES.md section 3, row "DOT", ratio 1.00000); the native DOT code was not read.
pub fn dot_advance(accumulated: &mut f64, dt: f64, damage_per_second: f64, period: f64) -> f64 {
    *accumulated += cmax(0.0, dt);
    if *accumulated < period {
        return 0.0;
    }
    let damage = damage_per_second * *accumulated;
    *accumulated -= period;
    damage
}

/// The final partial tick when the duration runs out; resets the clock.
pub fn dot_finish(accumulated: &mut f64, damage_per_second: f64) -> f64 {
    let damage = damage_per_second * *accumulated;
    *accumulated = 0.0;
    damage
}

/// The pool's bounds in order (a bad config with min above max is read the other way round).
fn ordered(lo: f64, hi: f64) -> (f64, f64) {
    if lo > hi { (hi, lo) } else { (lo, hi) }
}

/// NATIVE_WEAPON_TRIGGER_AMMO section 4A: the player's accuracy pool (a resource pool whose value is the full cone
/// angle in degrees, shared by every pellet of a shot). Defaults are the data's `D_Resourcepools.PlayerPools.AccuracyPool`
/// values the C++ uses (min 2, max 12, start at max, idle rate -8 per second after a 0.2 s delay), before the held
/// weapon's external accuracy slots scale them. Each shot adds the type's PerShotAccuracyImpulse (x
/// BurstShotAccuracyImpulseScale inside an automatic burst); the sum is clamped to [min, max] and restarts the idle
/// delay. Status (WEAPON_PROJECTILES.md section 10, one gun with Maya's skills on): **confirmed live** that the pool
/// rises by the shot's impulse, holds for the idle delay (0.176-0.200 s against the data's 0.2 s) and then falls
/// linearly to its floor; the per-gun values (rate and floor with the weapon's scales) are UNVERIFIED live.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AccuracyPool {
    pub min_value: f64,
    pub max_value: f64,
    /// Per second, once the delay has passed (negative: the cone shrinks).
    pub idle_rate: f64,
    /// Seconds.
    pub idle_delay: f64,
    pub value: f64,
    /// Seconds since the last change by an impulse.
    pub idle_for: f64,
}

impl Default for AccuracyPool {
    fn default() -> Self {
        Self {
            min_value: 2.0,
            max_value: 12.0,
            idle_rate: -8.0,
            idle_delay: 0.2,
            value: 12.0,
            idle_for: 0.0,
        }
    }
}

impl AccuracyPool {
    pub fn reset(&mut self, start_with_max: bool) {
        self.value = if start_with_max {
            self.max_value
        } else {
            self.min_value
        };
        self.idle_for = self.idle_delay;
    }

    /// Adds a signed impulse (a negative one narrows the cone); a NaN or infinite impulse leaves the pool as it was.
    pub fn impulse(&mut self, amount: f64) {
        if !amount.is_finite() {
            return;
        }
        let (lo, hi) = ordered(self.min_value, self.max_value);
        self.value = cclamp(
            if self.value.is_finite() {
                self.value + amount
            } else {
                lo
            },
            lo,
            hi,
        );
        self.idle_for = 0.0;
    }

    /// Only the part of the frame after the idle delay regenerates.
    pub fn tick(&mut self, dt: f64) {
        // C++: if (!(dt > 0) || !finite(dt)) return;
        if dt.is_nan() || dt <= 0.0 || !dt.is_finite() {
            return;
        }
        let (lo, hi) = ordered(self.min_value, self.max_value);
        self.idle_for += dt;
        let active = cmin(dt, cmax(0.0, self.idle_for - self.idle_delay));
        let rate = if self.idle_rate.is_finite() {
            self.idle_rate
        } else {
            0.0
        };
        self.value = cclamp(
            if self.value.is_finite() {
                self.value + rate * active
            } else {
                lo
            },
            lo,
            hi,
        );
    }
}

/// The aim offset of one shot from the accuracy pool: the same draw as [`spread_offset`] with the pool value as the
/// diameter, times `1 - zoom_effect` (fully zoomed: no pool cone). Status: UNVERIFIED (WEAPON_PROJECTILES.md section
/// 10: `1 - ZoomEffect` only partly seen; zoomed pitch offsets near 0, zoomed yaw offsets larger than the cone).
pub fn accuracy_offset(pool_degrees: f64, zoom_effect: f64, r1: f64, r2: f64) -> Offset {
    if !pool_degrees.is_finite() || pool_degrees <= 0.0 {
        return Offset::default();
    }
    let o = spread_offset(pool_degrees, r1, r2);
    let keep = 1.0
        - cclamp(
            if zoom_effect.is_finite() {
                zoom_effect
            } else {
                0.0
            },
            0.0,
            1.0,
        );
    Offset {
        pitch: o.pitch * keep,
        yaw: o.yaw * keep,
    }
}

/// The weapon type's kick fields. Defaults are the C++ defaults (class defaults for speed and recovery time).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KickData {
    /// WeaponKickUp / Down / Left / Right.
    pub up: f64,
    pub down: f64,
    pub left: f64,
    pub right: f64,
    /// MinimumVerticalPercentage / MinimumHorizontalPercentage.
    pub min_vertical: f64,
    pub min_horizontal: f64,
    /// WeaponKickSpeed (class default 4).
    pub speed: f64,
    /// WeaponKickRecoveryTime (class default 0.5).
    pub recovery_time: f64,
    /// WeaponKickZoomMultiplier.
    pub zoom_multiplier: f64,
}

impl Default for KickData {
    fn default() -> Self {
        Self {
            up: 1.0,
            down: 0.0,
            left: 0.0,
            right: 0.0,
            min_vertical: 0.0,
            min_horizontal: 0.0,
            speed: 4.0,
            recovery_time: 0.5,
            zoom_multiplier: 1.0,
        }
    }
}

/// NATIVE_WEAPON_FIRING 1.6 / NATIVE_WEAPON_TRIGGER_AMMO section 5: the view kick. A shot's kick (rotator units) is
/// impulse x 182.0444 x lerp(1, WeaponKickZoomMultiplier, zoom) (or the Hyperion AlternativeWeaponKick), rescaled by
/// the FOV ratio and split into vertical and horizontal parts by two draws; the targets decay linearly to 0 over
/// WeaponKickRecoveryTime from the last kick and the current amounts chase them at WeaponKickSpeed. The current
/// amounts are a view offset that returns to the aim, not a permanent view change.
/// Status (WEAPON_PROJECTILES.md section 10): **confirmed live** that the controller rotation never moves (so the kick
/// is a view offset), that the targets fall linearly to 0 at WeaponKickRecoveryTime, and that the current amounts chase
/// them by (target - current) x WeaponKickSpeed x dt per frame (fitted 8.00, the type's 8). The vertical / horizontal
/// split and its signs are not fitted: UNVERIFIED. The 1/30 s substeps are the host's guard against hitches.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kick {
    /// Rotator units.
    pub target_pitch: f64,
    pub target_yaw: f64,
    pub current_pitch: f64,
    pub current_yaw: f64,
    /// Seconds until the targets reach 0.
    pub recover_left: f64,
}

impl Kick {
    /// [`Kick::tick`] substeps longer frames (seconds).
    pub const MAX_STEP: f64 = 1.0 / 30.0;

    /// Kick amount (rotator units) of one shot before the split; keeps the impulse's sign. Status: **confirmed live**
    /// that the kick passed to `AddWeaponKick` is impulse x 182.0444, x WeaponKickZoomMultiplier when fully zoomed
    /// (WEAPON_PROJECTILES.md section 10); partial zoom (the lerp) and a negative impulse are UNVERIFIED.
    pub fn amount(impulse_degrees: f64, zoom_effect: f64, zoom_multiplier: f64) -> f64 {
        if !impulse_degrees.is_finite() || !zoom_multiplier.is_finite() {
            return 0.0;
        }
        let z = cclamp(
            if zoom_effect.is_finite() {
                zoom_effect
            } else {
                0.0
            },
            0.0,
            1.0,
        );
        impulse_degrees * 182.0444 * (1.0 + (zoom_multiplier - 1.0) * z)
    }

    /// `AddWeaponKick`: `r1`, `r2` in [0, 1) pick the vertical and horizontal draws;
    /// `fov_ratio` = tan(FOV/2) / tan(DefaultFOV/2). Signed: the magnitude goes through the split and its sign is kept
    /// (NATIVE_WEAPON_FIRING 1.6 takes |f x K| for the split; the sign case is UNVERIFIED in game).
    pub fn add(&mut self, kick: f64, d: &KickData, fov_ratio: f64, r1: f64, r2: f64) {
        if !kick.is_finite() || !fov_ratio.is_finite() {
            return;
        }
        let m = (fov_ratio * kick).abs();
        let sign = if fov_ratio * kick < 0.0 { -1.0 } else { 1.0 };
        let v = (d.up + d.down).abs() * r1 - d.down;
        let h = (d.right + d.left).abs() * r2 - d.left;
        let av = v.abs();
        let ah = h.abs();
        let mut vertical = d.min_vertical;
        if av > 0.0 {
            vertical += (1.0 - d.min_vertical - d.min_horizontal) * av / (av + ah);
        }
        let horizontal = if ah > 0.0 {
            1.0 - vertical
        } else {
            d.min_horizontal
        };
        self.target_pitch += sign
            * (if v < 0.0 { -1.0 } else { 1.0 })
            * vertical
            * m
            * (if av > 0.0 { 1.0 } else { 0.0 });
        self.target_yaw += sign
            * (if h < 0.0 { -1.0 } else { 1.0 })
            * horizontal
            * m
            * (if ah > 0.0 { 1.0 } else { 0.0 });
        self.recover_left = if d.recovery_time.is_finite() {
            cmax(0.0, d.recovery_time)
        } else {
            0.0
        };
    }

    /// `ApplyWanderingAndKick` for one frame: the targets fall linearly to 0 and the current amounts take the
    /// per-frame linear step (target - current) x speed x dt. The live fit gave exactly WeaponKickSpeed (8.00 on every
    /// frame pair at about 8.5 ms; the exponential 1 - exp(-speed dt) would read 7.73 there), so it is kept. A long
    /// frame is cut into steps of at most [`Kick::MAX_STEP`] so a hitch cannot snap or overshoot. Non-finite inputs
    /// are ignored; a negative speed counts as 0.
    pub fn tick(&mut self, dt: f64, d: &KickData) {
        // C++: if (!(dt > 0) || !finite(dt)) return;
        if dt.is_nan() || dt <= 0.0 || !dt.is_finite() {
            return;
        }
        let speed = if d.speed.is_finite() {
            cmax(0.0, d.speed)
        } else {
            0.0
        };
        // C++: static_cast<int>(std::min(1000.0, std::ceil(dt / MaxStep))); dt > 0 and finite, so 1..=1000.
        let steps = trunc_i32_f64(cmin(1000.0, (dt / Self::MAX_STEP).ceil()));
        let step = dt / f64::from(steps);
        for _ in 0..steps {
            if self.recover_left > step {
                let keep = 1.0 - step / self.recover_left;
                self.target_pitch *= keep;
                self.target_yaw *= keep;
                self.recover_left -= step;
            } else {
                self.target_pitch = 0.0;
                self.target_yaw = 0.0;
                self.recover_left = 0.0;
            }
            let follow = cmin(1.0, speed * step);
            self.current_pitch += (self.target_pitch - self.current_pitch) * follow;
            self.current_yaw += (self.target_yaw - self.current_yaw) * follow;
        }
    }
}

#[cfg(test)]
mod tests;
