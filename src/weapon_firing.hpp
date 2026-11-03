#pragma once
#include <vector>

// What one shot of a weapon does, as plain maths the UE5 host calls (OpenWillowFiring / OpenWillowProjectile).
// Rules are our own-words readings in docs/verification/NATIVE_WEAPON_FIRING.md (section numbers below); every one is
// UNVERIFIED in game unless that note or docs/verification/WEAPON_PROJECTILES.md says how it was confirmed.
// Angles are UE3 rotator units (65536 per turn) unless a name says degrees. Random numbers are passed in, so callers
// choose the generator and tests can pin them.
namespace weapon_firing {

constexpr double RotatorUnitsPerDegree = 65536.0 / 360.0;

struct Offset { double pitch = 0, yaw = 0; };  // rotator units, applied in the aim's local frame

// FiringModeDefinition.FiringPatternLines entry (rotator units).
struct PatternLine { int startPitch = 0, startYaw = 0, endPitch = 0, endYaw = 0; bool startOnly = false; };
struct Pattern {
    std::vector<PatternLine> lines;
    bool fireRandomly = false;          // bFireRandomlyFromPattern
    bool scaleBySpread = false;         // bScalePatternByWeaponSpread
    double basePatternSpread = 1;       // class default
    double minPatternScale = 0;
};

// Section 1.3. `index` is the weapon's NextFiringPatternIndex and is advanced; `pick` (0..1) chooses the line when
// firing randomly; `t` (0..1) is the point on the line. No lines: no offset and index untouched.
Offset patternOffset(const Pattern& pattern, double spreadDegrees, int& index, double pick, double t);

// Section 1.4: yaw uniform across a disc of diameter `spreadDegrees`, pitch uniform on the chord at that yaw.
// r1, r2 in [0, 1).
Offset spreadOffset(double spreadDegrees, double r1, double r2);

// Section 1.5. Spin-up: the interval moves linearly from max(multiplier * base, base) to base as percent goes 0 -> 1.
double fireInterval(double base, bool spinsUpToFullRate, double startMultiplier, double spinPercent);
double tickSpin(double percent, double dt, bool firing, double spinUpDuration, double spinDownDuration);
double burstInterval(double burstInterval, double currentFireInterval, double baseFireInterval);

// Section 1.5 refire (ShouldRefire): with AutomaticBurstCount N > 0 a press fires N shots even if the trigger is let go,
// then waits for a new press; N = 0 fires while the trigger is held. The state belongs to the weapon in hand: the host
// resets it when the weapon changes or is holstered.
struct Burst {
    int left = 0;        // shots still owed by the burst in progress
    bool done = false;   // the burst finished; the trigger must be pressed again
    void press() { done = false; }
    void reset() { left = 0; done = false; }                 // a different weapon in hand: a fresh state
    void interrupt() { left = 0; done = true; }   // reload, death, menus: owed shots dropped, a new press needed
    bool wantsFire(bool held) const { return (held && !done) || left > 0; }
    bool active() const { return left > 0; }
    // After a shot of a weapon with burst count `count`; true when this shot ended a burst of more than one shot (the
    // next shot waits for the burst interval).
    bool shot(int count);
};

// Section 1.7: flat additions are divided among pellets, the scaled base is not. NOT called by the host: the host has
// no runtime modifier stack and fires the evaluated card damage (tools/weapon_stats.py), which already includes every
// modifier; kept for when skills and relics add modifiers at run time (the split was seen live with a shield skill).
double multiProjectileDamage(double base, double preAdd, double postAdd, double scaleUp, double scaleDown, int pellets);

// Section 3.1 (WillowDamageTypeDefinition.CalcRadiusDamageScale with the stock linear fallback). Fractions of the
// radius; class defaults 0.2 / 0.8 / 0.2. Returns 0 outside the radius.
struct Falloff { double maxDamageRadius = 0.2, minDamageRadius = 0.8, minDamagePercent = 0.2; };
double radiusDamageScale(double distance, double radius, const Falloff& falloff, bool fullDamage = false);
double explosionRadius(double formulaRadius);  // section 3.2: 0 becomes 256

// Section 4.2: the product of the percent factors, times 0.01, clamped to [0, 1]. The host's hit path passes the radius
// scale times statusLevelScale (below) as extraScale; friendly fire, skills and relics are not modelled and count as 1.
double statusChance(double baseChancePercent, double targetModifier, double weaponBaseModifier,
                    double weaponChanceModifier, double fireIntervalFactor, double extraScale);
// Section 4.2 LevelScale: GlobalsDefinition.StatusEffectChanceScales. With n = min(|instigator - target|, rows), row n - 1;
// an instigator above the target takes HigherLevelChanceScale, below LowerLevelChanceScale; equal levels (or no rows) 1.
// UNVERIFIED: the higher/lower mapping and the row choice come from the data and fit the live Maliwan proc counts (shooter
// 54 against a level-7 target); the native roll was not read and the same-level case was not measured.
struct LevelChanceScale { int levelDifference = 0; double higher = 1, lower = 1; };
double statusLevelScale(int instigatorLevel, int targetLevel, const std::vector<LevelChanceScale>& rows);
// Section 4.2 fire-interval factor: r = current interval / type base interval.
double fireIntervalChanceFactor(double currentInterval, double typeBaseInterval, double shortPower, double longPower);
// Section 4.2 ExtraScale for radius damage: 1 - clamp(distance - cylinder radius, 0, R) / R.
double radiusStatusScale(double distance, double targetRadius, double radius);

// Flight of one projectile or light bullet for `dt` seconds. Rockets (section 2.1): velocity MaxSpeed * direction,
// constant acceleration along the launch direction, gravity scaled by GravityScaling. Light bullets (section 2.2): the
// manager's flight was not read; AirFriction changes the speed in proportion to the speed each frame, v *= 1 - F dt
// (Torgue gyrojets carry -10 / -6: the live fit, 2026-10-23, grew x(1 + 10 dt) per frame, uncapped, no gravity).
struct Vec { double x = 0, y = 0, z = 0; };
struct Flight { Vec position, velocity; double age = 0; };
struct Motion {
    Vec acceleration;           // uu/s^2, world frame (rockets: AccelRate along the launch direction)
    double gravityZ = 0;        // uu/s^2 (negative), already times the projectile's GravityScaling
    double airFriction = 0;     // 1/s (light bullets)
    double maxSpeed = 0;        // 0 = no cap
};
void step(Flight& flight, const Motion& motion, double dt);

// Momentum of a bullet or trace hit (section 1.2 / NATIVE_WEAPON_TRIGGER_AMMO ProcessInstantHitBullet): the weapon type's
// InstantHitMomentum is multiplied by 100 once at weapon setup; the hit passes ray direction x that (confirmed live:
// 800 for a type value of 8). The victim's velocity changes by momentum / Mass, or momentum x 100 when Mass <= 0.01
// (engine Pawn.TakeDamage, NATIVE_DAMAGE_DEATH 0.1; the upward part some damage types add is not included).
double hitMomentum(double instantHitMomentum);
Vec momentumVelocity(const Vec& momentum, double mass);

// Damage-over-time clock of one status effect application, as measured in the running game (2026-10-23, 94 ticks;
// docs/verification/WEAPON_PROJECTILES.md): the elapsed time builds up each frame; once it reaches the period (0.33 s)
// a tick deals dps x the time built up and the clock drops by one period, so a frame's overshoot is paid again by the
// next tick. When the effect ends, what is left is paid as a final partial tick. The native DOT code was not read.
constexpr double DotPeriod = 0.33;
// Advances by dt (already clamped by the caller to the effect's remaining time); returns the damage of this frame's
// tick, 0 when none.
double dotAdvance(double& accumulated, double dt, double damagePerSecond, double period = DotPeriod);
// The final partial tick when the duration runs out; resets the clock.
double dotFinish(double& accumulated, double damagePerSecond);

// NATIVE_WEAPON_TRIGGER_AMMO section 4A: the player's accuracy pool (a resource pool; its value is the full cone angle in
// degrees, shared by every pellet of a shot). Data: D_Resourcepools.PlayerPools.AccuracyPool (min 2, max 12, start at max,
// idle rate -8 per second after a 0.2 s delay), scaled by the held weapon's external accuracy slots. Each shot adds the
// type's PerShotAccuracyImpulse (x BurstShotAccuracyImpulseScale inside an automatic burst); the sum is clamped to
// [min, max] and restarts the idle delay. UNVERIFIED in game beyond the bound of the cone (PROJECTILE_FIRING_PATTERNS_LIVE R2).
struct AccuracyPool {
    double minValue = 2, maxValue = 12;
    double idleRate = -8;        // per second, once the delay has passed (negative: the cone shrinks)
    double idleDelay = 0.2;      // s
    double value = 12;
    double idleFor = 0;          // s since the last change by an impulse
    void reset(bool startWithMax) { value = startWithMax ? maxValue : minValue; idleFor = idleDelay; }
    void impulse(double amount);
    void tick(double dt);
};
// The aim offset of one shot: the same ellipse draw as spreadOffset with the pool value as diameter, times
// 1 - zoomEffect (fully zoomed: no pool cone).
Offset accuracyOffset(double poolDegrees, double zoomEffect, double r1, double r2);

// NATIVE_WEAPON_FIRING 1.6 / NATIVE_WEAPON_TRIGGER_AMMO section 5: the view kick. A shot's kick (rotator units) is
// impulse x 182.0444 x lerp(1, WeaponKickZoomMultiplier, zoom) (or the Hyperion AlternativeWeaponKick), rescaled by the FOV
// ratio and split into vertical and horizontal parts by two draws; the targets decay linearly to 0 over
// WeaponKickRecoveryTime from the last kick and the current amounts chase them at WeaponKickSpeed. Whether the current
// amounts are a view offset or a per-frame view change is open (to be read live): the host uses them as an offset.
struct KickData {
    double up = 1, down = 0, left = 0, right = 0;          // WeaponKickUp/Down/Left/Right
    double minVertical = 0, minHorizontal = 0;             // MinimumVertical/HorizontalPercentage
    double speed = 4;                                      // WeaponKickSpeed (class default)
    double recoveryTime = 0.5;                             // WeaponKickRecoveryTime (class default)
    double zoomMultiplier = 1;                             // WeaponKickZoomMultiplier
};
struct Kick {
    double targetPitch = 0, targetYaw = 0;    // rotator units
    double currentPitch = 0, currentYaw = 0;
    double recoverLeft = 0;                   // s until the targets reach 0
    // Kick amount (rotator units) of one shot before the split.
    static double amount(double impulseDegrees, double zoomEffect, double zoomMultiplier);
    // AddWeaponKick: r1, r2 in [0, 1) pick the vertical and horizontal draws; fovRatio = tan(FOV/2) / tan(DefaultFOV/2).
    void add(double kick, const KickData& data, double fovRatio, double r1, double r2);
    // ApplyWanderingAndKick for one frame (first-order chase: the routine's exact form was not read, UNVERIFIED).
    void tick(double dt, const KickData& data);
};

}  // namespace weapon_firing
