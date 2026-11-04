#include "weapon_firing.hpp"

#include <algorithm>
#include <cmath>
#include <utility>

namespace weapon_firing {

Offset patternOffset(const Pattern& pattern, double spreadDegrees, int& index, double pick, double t) {
    const int count = int(pattern.lines.size());
    if (count == 0) return {};
    int at = pattern.fireRandomly ? std::min(int(pick * count), count - 1) : index;
    if (at < 0 || at >= count) at = 0;
    const PatternLine& line = pattern.lines[size_t(at)];
    // One t for both components; each is truncated to a whole rotator unit.
    Offset offset;
    if (line.startOnly) {
        offset = {double(line.startPitch), double(line.startYaw)};
    } else {
        offset.pitch = std::trunc(line.startPitch + (line.endPitch - line.startPitch) * t);
        offset.yaw = std::trunc(line.startYaw + (line.endYaw - line.startYaw) * t);
    }
    if (pattern.scaleBySpread && pattern.basePatternSpread > 0) {
        const double scale = std::max(spreadDegrees / pattern.basePatternSpread, pattern.minPatternScale);
        offset.pitch *= scale;
        offset.yaw *= scale;
    }
    index = (at + 1) % count;
    return offset;
}

Offset spreadOffset(double spreadDegrees, double r1, double r2) {
    if (spreadDegrees == 0) return {};
    const double a = std::trunc(spreadDegrees * 65536.0 / 360.0);
    const double u = 0.5 - r1;
    const double chord = std::sqrt(std::max(0.0, 1.0 - 4.0 * u * u));
    return {(0.5 - r2) * chord * a, u * a};
}

double fireInterval(double base, bool spinsUpToFullRate, double startMultiplier, double spinPercent) {
    if (!spinsUpToFullRate) return base;
    const double start = std::max(startMultiplier * base, base);
    return start - (start - base) * std::clamp(spinPercent, 0.0, 1.0);
}

double tickSpin(double percent, double dt, bool firing, double spinUpDuration, double spinDownDuration) {
    if (firing) percent = spinUpDuration <= 0 ? 1.0 : percent + dt / spinUpDuration;
    else percent = spinDownDuration <= 0 ? 0.0 : percent - dt / spinDownDuration;
    return std::clamp(percent, 0.0, 1.0);
}

double burstInterval(double interval, double currentFireInterval, double baseFireInterval) {
    return baseFireInterval > 0 ? interval * currentFireInterval / baseFireInterval : interval;
}

bool Burst::shot(int count) {
    if (count <= 0) { left = 0; return false; }
    left = left > 0 ? left - 1 : count - 1;
    if (left > 0) return false;
    done = true;
    return count > 1;
}

double multiProjectileDamage(double base, double preAdd, double postAdd, double scaleUp, double scaleDown, int pellets) {
    const double scale = (1.0 + scaleUp) / (1.0 - scaleDown);
    return base * scale + (preAdd * scale + postAdd) / std::max(1, pellets);
}

double radiusDamageScale(double distance, double radius, const Falloff& falloff, bool fullDamage) {
    if (radius <= 0 || distance > radius) return 0;
    if (fullDamage) return 1;
    const double f = distance / radius;
    const double band = falloff.minDamageRadius - falloff.maxDamageRadius;
    double scale = -1;
    if (band > 0) {
        if (f < falloff.maxDamageRadius) scale = 1;
        else if (f <= falloff.minDamageRadius)
            scale = falloff.minDamagePercent
                + (1 - falloff.minDamagePercent) * (1 - (f - falloff.maxDamageRadius) / band);
        else if (f < 1) scale = falloff.minDamagePercent;
    }
    if (scale < 0) scale = 1 - f;  // "not handled": the stock linear falloff
    return std::max(0.0, scale);
}

double explosionRadius(double formulaRadius) { return formulaRadius == 0 ? 256 : formulaRadius; }

double statusChance(double baseChancePercent, double targetModifier, double weaponBaseModifier,
                    double weaponChanceModifier, double fireIntervalFactor, double extraScale) {
    const double percent = baseChancePercent * targetModifier * weaponBaseModifier * weaponChanceModifier
        * fireIntervalFactor * extraScale;
    return std::clamp(percent * 0.01, 0.0, 1.0);
}

double fireIntervalChanceFactor(double currentInterval, double typeBaseInterval, double shortPower, double longPower) {
    if (typeBaseInterval == 0) return 1;
    const double r = currentInterval / typeBaseInterval;
    if (r == 1) return 1;
    return std::pow(r, r < 1 ? shortPower : longPower);
}

double radiusStatusScale(double distance, double targetRadius, double radius) {
    if (radius <= 0) return 0;
    return 1 - std::clamp(distance - targetRadius, 0.0, radius) / radius;
}

void step(Flight& flight, const Motion& motion, double dt) {
    Vec& v = flight.velocity;
    // Semi-implicit Euler: update the velocity, then move with the new velocity.
    const double drag = 1 - motion.airFriction * dt;
    v = {v.x * drag + motion.acceleration.x * dt, v.y * drag + motion.acceleration.y * dt,
         v.z * drag + (motion.acceleration.z + motion.gravityZ) * dt};
    if (motion.maxSpeed > 0) {
        const double speed = std::sqrt(v.x * v.x + v.y * v.y + v.z * v.z);
        if (speed > motion.maxSpeed) {
            const double k = motion.maxSpeed / speed;
            v = {v.x * k, v.y * k, v.z * k};
        }
    }
    flight.position = {flight.position.x + v.x * dt, flight.position.y + v.y * dt, flight.position.z + v.z * dt};
    flight.age += dt;
}

double dotAdvance(double& accumulated, double dt, double damagePerSecond, double period) {
    accumulated += std::max(0.0, dt);
    if (accumulated < period) return 0;
    const double damage = damagePerSecond * accumulated;
    accumulated -= period;
    return damage;
}

double dotFinish(double& accumulated, double damagePerSecond) {
    const double damage = damagePerSecond * accumulated;
    accumulated = 0;
    return damage;
}

double hitMomentum(double instantHitMomentum) { return instantHitMomentum * 100.0; }

Vec momentumVelocity(const Vec& momentum, double mass) {
    const double scale = mass > 0.01 ? 1.0 / mass : 100.0;
    return {momentum.x * scale, momentum.y * scale, momentum.z * scale};
}

double statusLevelScale(int instigatorLevel, int targetLevel, const std::vector<LevelChanceScale>& rows) {
    const int difference = instigatorLevel > targetLevel ? instigatorLevel - targetLevel : targetLevel - instigatorLevel;
    if (difference == 0 || rows.empty()) return 1;
    const LevelChanceScale& row = rows[static_cast<size_t>(std::min<int>(difference, static_cast<int>(rows.size())) - 1)];
    return instigatorLevel > targetLevel ? row.higher : row.lower;
}

namespace {
bool finite(double v) { return std::isfinite(v); }
// The pool's bounds in order (a bad config with min above max is read as the other way round).
void ordered(double& lo, double& hi) { if (lo > hi) std::swap(lo, hi); }
}  // namespace

void AccuracyPool::impulse(double amount) {
    if (!finite(amount)) return;   // a NaN or infinite impulse leaves the pool as it was
    double lo = minValue, hi = maxValue;
    ordered(lo, hi);
    value = std::clamp(finite(value) ? value + amount : lo, lo, hi);   // signed: a negative impulse narrows the cone
    idleFor = 0;
}

void AccuracyPool::tick(double dt) {
    if (!(dt > 0) || !finite(dt)) return;
    double lo = minValue, hi = maxValue;
    ordered(lo, hi);
    // Only the part of the frame after the idle delay regenerates.
    idleFor += dt;
    const double active = std::min(dt, std::max(0.0, idleFor - idleDelay));
    const double rate = finite(idleRate) ? idleRate : 0.0;
    value = std::clamp(finite(value) ? value + rate * active : lo, lo, hi);
}

Offset accuracyOffset(double poolDegrees, double zoomEffect, double r1, double r2) {
    if (!finite(poolDegrees) || poolDegrees <= 0) return {};
    const Offset o = spreadOffset(poolDegrees, r1, r2);
    const double keep = 1.0 - std::clamp(finite(zoomEffect) ? zoomEffect : 0.0, 0.0, 1.0);
    return {o.pitch * keep, o.yaw * keep};
}

double Kick::amount(double impulseDegrees, double zoomEffect, double zoomMultiplier) {
    if (!finite(impulseDegrees) || !finite(zoomMultiplier)) return 0;
    const double z = std::clamp(finite(zoomEffect) ? zoomEffect : 0.0, 0.0, 1.0);
    return impulseDegrees * 182.0444 * (1.0 + (zoomMultiplier - 1.0) * z);
}

void Kick::add(double kick, const KickData& d, double fovRatio, double r1, double r2) {
    if (!finite(kick) || !finite(fovRatio)) return;
    // Signed: the magnitude goes through the split, its sign is kept (a negative kick, from a negative impulse, kicks the
    // other way; NATIVE_WEAPON_FIRING 1.6 takes |f x K| for the split, the sign case is UNVERIFIED in game).
    const double m = std::fabs(fovRatio * kick);
    const double sign = fovRatio * kick < 0 ? -1.0 : 1.0;
    const double v = std::fabs(d.up + d.down) * r1 - d.down;
    const double h = std::fabs(d.right + d.left) * r2 - d.left;
    const double av = std::fabs(v), ah = std::fabs(h);
    double vertical = d.minVertical;
    if (av > 0) vertical += (1.0 - d.minVertical - d.minHorizontal) * av / (av + ah);
    const double horizontal = ah > 0 ? 1.0 - vertical : d.minHorizontal;
    targetPitch += sign * (v < 0 ? -1.0 : 1.0) * vertical * m * (av > 0 ? 1.0 : 0.0);
    targetYaw += sign * (h < 0 ? -1.0 : 1.0) * horizontal * m * (ah > 0 ? 1.0 : 0.0);
    recoverLeft = finite(d.recoveryTime) ? std::max(0.0, d.recoveryTime) : 0.0;
}

void Kick::tick(double dt, const KickData& d) {
    if (!(dt > 0) || !finite(dt)) return;
    // The chase is the per-frame linear step current += (target - current) x speed x dt: the live fit gave exactly
    // WeaponKickSpeed (8.00 on every frame pair at about 8.5 ms; the exponential 1 - exp(-speed dt) would read 7.73 there),
    // so it is kept for parity. A long frame is cut into steps of at most MaxStep so a hitch cannot snap or overshoot.
    const double speed = finite(d.speed) ? std::max(0.0, d.speed) : 0.0;
    const int steps = static_cast<int>(std::min(1000.0, std::ceil(dt / MaxStep)));
    const double step = dt / steps;
    for (int i = 0; i < steps; ++i) {
        if (recoverLeft > step) {
            const double keep = 1.0 - step / recoverLeft;
            targetPitch *= keep;
            targetYaw *= keep;
            recoverLeft -= step;
        } else {
            targetPitch = targetYaw = 0;
            recoverLeft = 0;
        }
        const double follow = std::min(1.0, speed * step);
        currentPitch += (targetPitch - currentPitch) * follow;
        currentYaw += (targetYaw - currentYaw) * follow;
    }
}

}  // namespace weapon_firing
