// Synthetic checks of src/weapon_firing (no game data): each case restates a rule of
// docs/verification/NATIVE_WEAPON_FIRING.md with made-up numbers.
#include "weapon_firing.hpp"

#include <cmath>
#include <cstdio>

using namespace weapon_firing;

static int failures = 0;
static void check(bool ok, const char* what) {
    if (!ok) { std::printf("FAIL %s\n", what); ++failures; }
}
static bool near(double a, double b, double eps = 1e-6) { return std::fabs(a - b) <= eps; }

int main() {
    // Spread: diameter in degrees, so the extreme yaw is half the spread; pitch is 0 at the disc's left/right edge.
    const double a = std::trunc(4.0 * RotatorUnitsPerDegree);
    Offset edge = spreadOffset(4.0, 0.0, 0.0);
    check(near(edge.yaw, 0.5 * a) && near(edge.pitch, 0), "spread: edge yaw is half the diameter, pitch 0");
    Offset centre = spreadOffset(4.0, 0.5, 0.0);
    check(near(centre.yaw, 0) && near(centre.pitch, 0.5 * a), "spread: centre column reaches full pitch");
    check(near(spreadOffset(0.0, 0.1, 0.9).yaw, 0), "spread: zero spread leaves the aim");

    // Pattern: index advances per pellet; t interpolates both components; spread scaling.
    Pattern pattern;
    pattern.lines = {{0, -100, 0, 100, false}, {10, 0, 10, 0, true}};
    int index = 0;
    Offset first = patternOffset(pattern, 1.0, index, 0.0, 0.25);
    check(near(first.yaw, -50) && index == 1, "pattern: line 0 at t=0.25, index -> 1");
    Offset second = patternOffset(pattern, 1.0, index, 0.0, 0.9);
    check(near(second.pitch, 10) && near(second.yaw, 0) && index == 0, "pattern: start-only line, index wraps");
    pattern.scaleBySpread = true;
    pattern.basePatternSpread = 2.0;
    pattern.minPatternScale = 0.75;
    index = 0;
    check(near(patternOffset(pattern, 1.0, index, 0.0, 1.0).yaw, 75), "pattern: scale clamps to MinPatternScale");
    pattern.fireRandomly = true;
    index = 0;
    patternOffset(pattern, 4.0, index, 0.99, 0.0);
    check(index == 0, "pattern: random pick chose the last line and wrapped");
    Pattern none;
    index = 3;
    check(near(patternOffset(none, 1.0, index, 0.5, 0.5).yaw, 0) && index == 3, "pattern: no lines, no change");

    // Fire interval and spin-up.
    check(near(fireInterval(0.1, true, 3.0, 0.0), 0.3), "spin: starts at multiplier x base");
    check(near(fireInterval(0.1, true, 3.0, 0.5), 0.2), "spin: linear in the percent");
    check(near(fireInterval(0.1, true, 0.5, 0.0), 0.1), "spin: a multiplier below 1 never speeds up");
    check(near(fireInterval(0.1, false, 3.0, 0.0), 0.1), "spin: not spinning keeps base");
    check(near(tickSpin(0.0, 0.25, true, 1.0, 2.0), 0.25) && near(tickSpin(0.5, 0.5, false, 1.0, 2.0), 0.25),
          "spin: up at 1/duration, down at 1/spin-down duration");
    check(near(tickSpin(0.2, 0.1, true, 0.0, 1.0), 1.0), "spin: zero duration snaps up");
    check(near(burstInterval(0.5, 0.05, 0.1), 0.25) && near(burstInterval(0.5, 0.05, 0.0), 0.5),
          "burst interval follows the current fire interval");

    // Bursts: a press owes the whole burst, then waits for a new press; a weapon change resets the state.
    Burst burst;
    burst.press();
    check(burst.wantsFire(true) && !burst.shot(3) && burst.left == 2, "burst: first shot of 3 leaves 2");
    check(burst.wantsFire(false), "burst: released trigger still fires the rest of the burst");
    check(!burst.shot(3) && burst.shot(3) && burst.done && !burst.wantsFire(true), "burst: ends, held trigger waits");
    burst.press();
    check(burst.wantsFire(true), "burst: a new press starts the next burst");
    burst.shot(3);
    burst.reset();   // switched to an automatic weapon mid-burst
    check(!burst.wantsFire(false) && !burst.active(), "burst: weapon change drops the owed shots");
    check(!burst.shot(0) && burst.wantsFire(true) && !burst.wantsFire(false), "burst: count 0 fires only while held");
    // Lifecycle: a reload or a respawn mid-burst drops the owed shots, and a held trigger does not start a new burst.
    Burst reloaded;
    reloaded.press();
    reloaded.shot(3);
    reloaded.interrupt();
    check(!reloaded.wantsFire(false) && !reloaded.wantsFire(true), "burst: interrupted mid-burst fires nothing, held or not");
    reloaded.press();
    check(reloaded.wantsFire(true), "burst: after the interruption a new press fires");
    Burst single;
    single.press();
    check(!single.shot(1) && single.done && !single.wantsFire(true), "burst: count 1 is one shot per press");

    // Per-pellet damage (not called by the host; see the header).
    check(near(multiProjectileDamage(10, 4, 6, 1.0, 0.0, 5), 10 * 2 + (4 * 2 + 6) / 5.0), "pellet damage splits flat adds");

    // Splash falloff with the class defaults.
    Falloff defaults;
    check(near(radiusDamageScale(10, 100, defaults), 1.0), "falloff: full inside 20 %");
    check(near(radiusDamageScale(50, 100, defaults), 0.2 + 0.8 * 0.5), "falloff: linear in the band");
    check(near(radiusDamageScale(90, 100, defaults), 0.2), "falloff: floor to the edge");
    check(near(radiusDamageScale(101, 100, defaults), 0.0), "falloff: nothing outside");
    check(near(radiusDamageScale(90, 100, defaults, true), 1.0), "falloff: full-damage flag");
    Falloff flat{0.5, 0.5, 0.3};
    check(near(radiusDamageScale(25, 100, flat), 0.75), "falloff: empty band falls back to 1 - d/R");
    check(near(explosionRadius(0), 256) && near(explosionRadius(120), 120), "explosion radius 0 -> 256");

    // Status chance.
    check(near(statusChance(20, 1, 0.4, 1.0, 1, 1), 0.08), "status: product x 0.01");
    check(near(statusChance(80, 1, 3, 1, 1, 1), 1.0), "status: clamps to 1");
    check(near(fireIntervalChanceFactor(0.05, 0.1, 0.5, 2), std::sqrt(0.5)), "status: faster gun uses short power");
    check(near(fireIntervalChanceFactor(0.2, 0.1, 0.5, 2), 4.0), "status: slower gun uses long power");
    check(near(fireIntervalChanceFactor(0.2, 0.0, 0.5, 2), 1.0), "status: no type rate gives 1");
    check(near(radiusStatusScale(70, 20, 100), 0.5) && near(radiusStatusScale(10, 20, 100), 1.0),
          "status: radius scale from the cylinder edge");

    // Flight.
    Flight flight;
    flight.velocity = {1000, 0, 0};
    Motion rocket;
    rocket.gravityZ = -100;
    for (int i = 0; i < 100; ++i) step(flight, rocket, 0.01);
    check(near(flight.position.x, 1000, 1e-6) && flight.position.z < -45 && flight.position.z > -55,
          "flight: straight at speed, gravity drop about g t^2 / 2");
    Flight gyro;
    gyro.velocity = {1000, 0, 0};
    Motion gyroMotion;
    gyroMotion.airFriction = -1;
    gyroMotion.maxSpeed = 1500;
    for (int i = 0; i < 100; ++i) step(gyro, gyroMotion, 0.01);
    check(gyro.velocity.x > 1000 && gyro.velocity.x <= 1500 + 1e-9, "flight: negative friction speeds up, capped");

    // DOT clock: ticks of dps x accumulated time at 0.33 s, overshoot carried, a partial tick at the end.
    double clock = 0;
    check(near(dotAdvance(clock, 0.2, 30), 0) && near(clock, 0.2), "dot: no tick before the period");
    check(near(dotAdvance(clock, 0.2, 30), 30 * 0.4) && near(clock, 0.4 - 0.33), "dot: tick pays the built-up time");
    check(near(dotFinish(clock, 30), 30 * (0.4 - 0.33)) && near(clock, 0), "dot: final partial tick");
    double total = 0, steady = 0;
    for (int i = 0; i < 120; ++i) total += dotAdvance(steady, 1.0 / 60.0, 10);
    check(total > 10 * 2.0 * 1.0 && total < 10 * 2.0 * 1.2, "dot: frame overshoot adds a little over dps x time");

    if (failures) return 1;
    std::printf("weapon_firing_math: all checks passed\n");
    return 0;
}
