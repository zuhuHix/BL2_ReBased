#pragma once

#include "CoreMinimal.h"

class AActor;

// Range enemy AI (PR2): notice, approach, melee. Host stand-ins throughout, UNVERIFIED in game; every rule is cited to
// its live record where the rule is coded (docs/verification/AI_AWARENESS_LIVE.md, AI_MELEE_TARGETING_LIVE.md,
// COMBAT_MELEE.md). The stock AI tree is not run.
namespace OpenWillowThreat
{
// Threat levels (AI_AWARENESS_LIVE 0): 1 InThreatRadius (the default of a record), 2 SeenInThreatRadius, 3 Provoked.
constexpr int32 InThreatRadius = 1;
constexpr int32 SeenInThreatRadius = 2;
constexpr int32 ProvokedLevel = 3;
constexpr float MinEngagementTime = 10.f;    // seconds of ForgetTime after a notice (AI_AWARENESS_LIVE 2)
constexpr float MinProvokedTime = 25.f;      // seconds of ForgetTime after damage by Maya (AI_AWARENESS_LIVE 3)
constexpr float MinAggroTimeToConsiderAwareOfTarget = 0.1f;   // AI_AWARENESS_LIVE 2
constexpr float UpdateInterval = 0.2f;       // the state update of each enemy (AI_MELEE_TARGETING_LIVE 4)
constexpr float DenUpdateInterval = 1.f;     // the den's fall-back update (AI_AWARENESS_LIVE 2)
constexpr float AICloseThreatRadius = 768.f; // proximity rule (AI_AWARENESS_LIVE 0)
// Melee (AI_MELEE_TARGETING_LIVE 4-5, MeleeDef_PrimalBeast_Melee): the damage moment and the swing interval are read; the
// volume is the fitted rule of section 5 (UNVERIFIED as the native formula).
constexpr float SwingDamageDelay = 0.41f;    // seconds from the swing start to the damage moment
constexpr float SwingInterval = 1.25f;       // seconds from one swing start to the next
constexpr float MeleeReach = 640.f;          // horizontal radius from the origin
constexpr float MeleeBackOffset = 100.f;     // the origin is this far behind the enemy, along its facing
constexpr float MeleeHalfAngleDegrees = 64.f;
constexpr float MeleeBelow = -143.f;         // Maya's centre Z minus the enemy's Z, lowest and highest
constexpr float MeleeAbove = 339.f;
constexpr float MeleeStopMargin = 60.f;      // host choice: the enemy stops running this far inside its swing range (or the reach)
constexpr float DefaultCollisionRadius = 50.f;  // host stand-in when the kind's collision radius is 0
constexpr float SearchArrival = 100.f;       // host choice: a lost target's last-known spot counts as reached this close
}

// The one threat record of the range den (AI_AWARENESS_LIVE 0, 2, 3). The range is one den: its centre is the range spot,
// and the radii are the values of the one den read in AI_AWARENESS_LIVE 0/2 (host stand-in to use them for the range).
// Owned by the range subsystem; the enemies hold a pointer to it.
struct FOpenWillowThreatDen
{
    FVector2D Centre = FVector2D::ZeroVector;
    float ThreatRadius = 4665.625f;
    float PatrolRadius = 3667.19f;
    int32 Level = OpenWillowThreat::InThreatRadius;
    float AggroTime = 0.f;        // when the level was last raised by a notice or a hit (0 at level 1)
    float ForgetTime = 0.f;       // when the level falls back (the den's update checks it)
    float NextCheckAt = 0.f;      // the den's fall-back check runs every DenUpdateInterval

    // Test-only and build-time: a fresh record (level 1, no times).
    void ResetRecord()
    {
        Level = OpenWillowThreat::InThreatRadius;
        AggroTime = 0.f;
        ForgetTime = 0.f;
    }
};

// What the enemy tells its brain each tick (filled by AOpenWillowEnemy).
struct FOpenWillowBrainInput
{
    AActor* Self = nullptr;                    // the enemy: its name in the logs, the causer of its damage
    FVector Location = FVector::ZeroVector;    // the enemy's origin (on the floor)
    float YawDegrees = 0.f;                    // its facing
    float CollisionRadius = 0.f;               // the body radius for the proximity rule
    bool bPassive = false;                     // no notice, no melee
    bool bPaused = false;                      // dead or phaselocked: the brain stands still
    bool bHasMaya = false;                     // Maya is alive and up
    FVector MayaLocation = FVector::ZeroVector;   // Maya's centre (her actor origin)
    int32 MayaLevel = 1;
    bool bLineOfSight = false;                 // a clear line from the enemy's centre to Maya's centre, now (the exposure stand-in)
    int32 EnemyLevel = 1;
    float ArrivalDamage = 0.f;                 // manifest melee_arrival_by_level at the enemy's level (0 = no melee)
    float Ratio = 0.f;                         // manifest melee_ratio_by_level at the enemy's level (0 = no melee)
    float SwingStartRange = 0.f;               // manifest melee_start_range, the stock attack range (0 = an old manifest: the reach decides)
};

// What the brain asks the enemy to do this tick.
struct FOpenWillowBrainOrders
{
    bool bChase = false;          // go for ChaseTo: turn to it, and run unless bRun is false
    FVector ChaseTo = FVector::ZeroVector;   // where Maya was last seen (the target's location, or a lost target's last-known spot)
    bool bRun = false;            // run at her (false inside the stop margin, or during a swing)
    bool bSwingStarted = false;   // a swing started this tick: play the melee clip
};

// The range enemy's brain: the threat rules (on the state update) and the melee timing (every tick). A plain class: the
// enemy owns it and ticks it. Movement, animation and the threat record's fall-back are the enemy's and the den's.
class FOpenWillowEnemyBrain
{
public:
    // Runs the brain for one tick. bDamagedByMaya is the enemy's "hit by Maya" flag: read and cleared at each state update.
    FOpenWillowBrainOrders Tick(float Now, const FOpenWillowBrainInput& In, FOpenWillowThreatDen& Den, bool& bDamagedByMaya);
    // Drops the target and any swing in progress (death, respawn, passive, paused).
    void Stop();
    // Test fixture: Stop, and the next tick runs a state update.
    void ResetForTest() { Stop(); NextUpdateAt = 0.f; }
    // The enemy could not make progress toward its target: drop it, forget the last-known spot, and do not take a target
    // again for Seconds (host choice, UNVERIFIED).
    void GiveUp(float Now, float Seconds);
    // True when the next tick runs a state update (the enemy samples the line of sight only then).
    bool IsUpdateDue(float Now) const { return Now >= NextUpdateAt; }
    bool IsTargeting() const { return bTargeting; }
    bool IsSwingPending() const { return bSwingPending; }

    // Counters and the last swing, read by the logs and the range test.
    int32 SwingCount = 0;
    float LastSwingStart = -10.f;
    float PrevSwingStart = -10.f;
    int32 MeleeResolved = 0;
    bool LastHit = false;           // the last resolved swing hit Maya
    float LastAt = -10.f;           // when it resolved (the damage moment)
    float LastArrival = 0.f;
    float LastRatio = 0.f;
    float LastScale = 1.f;
    float LastApplied = 0.f;        // what the delivery applied (0 on a miss)
    float LastSwingDistance = -1.f; // the horizontal distance to Maya at the last swing start

private:
    void UpdateThreat(float Now, const FOpenWillowBrainInput& In, FOpenWillowThreatDen& Den, bool bDamaged, bool bSightBefore);
    void StartSwing(float Now, const FOpenWillowBrainInput& In);
    void ResolveSwing(float Now, const FOpenWillowBrainInput& In, bool bHit);

    float NextUpdateAt = 0.f;
    bool bExposedBefore = false;    // the line of sight at the previous state update (the sight rule reads it)
    // The enemy's own target, apart from the den's threat record (AI_AWARENESS_LIVE 2: targets are dropped at the member's
    // next update once its rules no longer hold). Host stand-in: a target is valid while the den level is 2 or more and Maya is
    // within the den's ThreatRadius of its centre; the enemy goes for where it last saw her (line of sight or proximity), and
    // after losing the target it walks to that spot once, then patrols.
    bool bTargeting = false;
    bool bHasTargetLocation = false;
    FVector TargetLocation = FVector::ZeroVector;
    float GiveUpUntil = -10.f;
    bool bSwingPending = false;
    float SwingStartAt = -10.f;
};

// The den's fall-back: when the level is 2 or more and now is past ForgetTime, the level drops to 1 (every DenUpdateInterval).
void UpdateThreatDen(FOpenWillowThreatDen& Den, float Now);

// The level scale of a range enemy's hit on Maya (COMBAT_MELEE stage 2): 1.0 at or below her level, 1.1 one level up,
// 1.2 two up, then a straight line to 3.0 at sixteen up (host stand-in beyond two, UNVERIFIED).
float MeleeLevelScale(int32 EnemyLevel, int32 MayaLevel);

// The ONE delivery of range melee to Maya: OpenWillowApplyDamageToPlayer (OpenWillowMayaState.h, the maya lane's PR #72), shield
// first, then health, then the down state. Returns the damage the pools took.
float RangeDamagePlayer(AActor* Causer, float Amount);
