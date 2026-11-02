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

// Section 4.2: the product of the percent factors, times 0.01, clamped to [0, 1]. Factors the host does not model
// (level difference, friendly fire, skills and relics) are 1 at the call site.
double statusChance(double baseChancePercent, double targetModifier, double weaponBaseModifier,
                    double weaponChanceModifier, double fireIntervalFactor, double extraScale);
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

}  // namespace weapon_firing
