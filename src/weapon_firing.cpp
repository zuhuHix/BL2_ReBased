#include "weapon_firing.hpp"

#include <algorithm>
#include <cmath>

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

}  // namespace weapon_firing
