#pragma once

#include "CoreMinimal.h"
#include "OpenWillowLeapThrow.h"

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
constexpr float SwingInterval = 1.25f;       // seconds from one swing start to the next: one adult's median (section 4); kinds with no row in
                                             // KindMeleeRows (OpenWillowEnemyAI.cpp) borrow it as a stand-in
constexpr float MeleeReach = 640.f;          // horizontal radius from the origin
constexpr float MeleeBackOffset = 100.f;     // the origin is this far behind the enemy, along its facing
constexpr float MeleeHalfAngleDegrees = 64.f;
constexpr float MeleeBelow = -143.f;         // Maya's centre Z minus the enemy's Z, lowest and highest
constexpr float MeleeAbove = 339.f;
constexpr float MeleeStopMargin = 60.f;      // host choice: the enemy stops running this far inside its swing range (or the reach)
constexpr float StandTolerance = 1.f;        // host choice: within this of a live stand distance counts as there (the run is clamped to it)
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
    // The leap and throw resources (NATIVE_AI_LEAP_THROW 2.4): how many of the den's enemies run a leap or a throw that holds one now. The
    // note leaves the scope open; the hub chose the range's one den (UNVERIFIED). Not part of the threat record: ResetRecord leaves them.
    int32 LeapUsers = 0;
    int32 ThrowUsers = 0;

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
    float SwingInterval = OpenWillowThreat::SwingInterval;   // the kind's swing cycle (FindKindMelee), else the stand-in
    float StandDistance = 0.f;                 // the kind's live stand distance (FindKindMelee); 0 = the stand-in stop inside the swing range
    float MayaRadius = 0.f;                    // Maya's capsule radius: with CollisionRadius, the body contact a live stand distance never goes inside
    // Leaps and thrown rocks (NATIVE_AI_LEAP_THROW.md). Null: the plain melee only, as before (a kind with no row, or the test fixture).
    const FOpenWillowKindAttacks* Attacks = nullptr;
    float HalfHeight = 0.f;                    // the enemy's collision half height: its centre stands this far above its origin
    float MayaHalfHeight = 0.f;                // Maya's capsule half height (her origin is her centre)
    FVector MayaVelocity = FVector::ZeroVector;   // the lead of a leap or a rock
    FVector HandLocation = FVector::ZeroVector;   // where a rock leaves the hand (the enemy fills it during a throw)
};

// Melee per range kind from live reads: the acceptance table of docs/verification/COMBAT_RANGE.md, held to tools/check_range.py's
// ACCEPTANCE by tests/range_kind_melee_test.py. One session per value, UNVERIFIED beyond it. A kind with no row has no live read and keeps
// the stand-ins (SwingInterval; a stop MeleeStopMargin inside its swing range).
struct FOpenWillowKindMelee
{
    const TCHAR* Kind;      // the range manifest's kind name
    float SwingInterval;    // s, one swing start to the next
    float StandDistance;    // uu, horizontal, the enemy's origin to Maya's, where it stands to swing once it has closed; 0 = no live
                            // stand distance (the cycle only): the stand-in stop MeleeStopMargin inside its swing range
};
// The row for Kind (case-sensitive), or null when no live read covers it.
const FOpenWillowKindMelee* FindKindMelee(const FString& Kind);

// What the brain asks the enemy to do this tick.
struct FOpenWillowBrainOrders
{
    bool bChase = false;          // go for ChaseTo: turn to it, and run unless bRun is false
    FVector ChaseTo = FVector::ZeroVector;   // where Maya was last seen (the target's location, or a lost target's last-known spot)
    bool bRun = false;            // run at her (false inside the stop margin, or during a swing)
    bool bSwingStarted = false;   // a swing started this tick: play the melee clip
    float StepLimit = -1.f;       // with bRun, for a kind with a live stand distance: the most it may move this tick, the flat gap to
                                  // Maya's current position minus the stop distance (never below 0); -1 = no limit
    bool bStandOnContact = false; // the live stand distance was inside body contact, so the stop is at contact
    // Leaps and throws (the enemy moves, animates and spawns; the brain times and resolves them).
    bool bLeapLaunch = false;     // the launch clip starts this tick
    bool bLeapTakeoff = false;    // it leaves the ground this tick (the arc is GetLeap().Arc)
    bool bLeapLanded = false;     // it touches down this tick: the landing clip
    bool bThrowStarted = false;   // the throw clip starts this tick
    bool bRockReleased = false;   // a rock leaves the hand this tick
    FOpenWillowArc RockArc;       // with bRockReleased
    float RockDamage = 0.f;       // with bRockReleased: what the rock delivers on a touch (0 = no melee numbers: it does no damage)
    bool bAttackEnded = false;    // a leap or throw ended (or was stopped) this tick: the move clips start again
};

// The last leap of an enemy, for its log lines and the range test.
struct FOpenWillowLeapRecord
{
    float StartAt = -10.f;        // the launch clip's start: the host's Attack moment (the hub's decision 7)
    float TakeoffAt = -10.f;
    float LandAt = -10.f;         // touch-down (the landing hit)
    float StartDistance = 0.f;    // horizontal, the enemy's origin to Maya's, when the leap was chosen
    FOpenWillowArc Arc;
    FVector Facing = FVector::ForwardVector;   // the launch direction (horizontal): the landing volume lies ahead along it
    bool bLanded = false;
    bool bHit = false;
    float Expected = 0.f;         // arrival x ratio x level scale
    float Applied = 0.f;
    float Knock = 0.f;            // the horizontal speed added to Maya (0: none)
    float LandError = 0.f;        // horizontal, where it came down to the arc's aim point
    bool bForced = false;         // the range test asked for it
};

// The last rock of an enemy: its release here, its impact written by the rock (AOpenWillowRangeRock).
struct FOpenWillowRockRecord
{
    float StartAt = -10.f;        // the throw clip's start
    float ReleaseAt = -10.f;
    float ReleaseDue = 0.f;       // the clip's release moment, seconds after StartAt
    float StartDistance = 0.f;
    FOpenWillowArc Arc;
    float Damage = 0.f;
    bool bImpact = false;
    FString What;                 // maya, world or expired
    float ImpactAt = -10.f;
    float MissDistance = 0.f;     // horizontal, the impact point to the arc's aim point
    bool bHit = false;
    float Applied = 0.f;
    float Knock = 0.f;
    bool bForced = false;
};

// The range enemy's brain: the threat rules (on the state update) and the melee timing (every tick). A plain class: the
// enemy owns it and ticks it. Movement, animation and the threat record's fall-back are the enemy's and the den's.
class FOpenWillowEnemyBrain
{
public:
    // Runs the brain for one tick. bDamagedByMaya is the enemy's "hit by Maya" flag: read and cleared at each state update.
    FOpenWillowBrainOrders Tick(float Now, const FOpenWillowBrainInput& In, FOpenWillowThreatDen& Den, bool& bDamagedByMaya);
    // Drops the target and any swing in progress (death, respawn, passive, paused). A leap or throw in progress ends and gives its
    // resource back.
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
    // Leaps and throws. The draw's stream is seeded per enemy by the range (-owrangeaiseed=N), so a run can repeat its draws.
    void SeedAttackDraw(int32 Seed) { AttackDraw.Initialize(Seed); }
    // Test fixture: the next draw takes this attack when it can run (else it logs the refusal and draws as usual).
    void ForceAttackForTest(EOpenWillowAttack Attack) { Forced = Attack; }
    EOpenWillowAttack GetAttack() const { return Pick; }
    bool IsLeaping() const { return Pick == EOpenWillowAttack::Leap; }
    bool IsThrowing() const { return Pick == EOpenWillowAttack::Throw; }
    bool IsAirborne() const { return Pick == EOpenWillowAttack::Leap && LeapPhase == 2; }
    bool IsLeapWindUp() const { return Pick == EOpenWillowAttack::Leap && LeapPhase == 1; }
    // Seconds into the current attack (from the launch clip's or the throw clip's start), and into the flight.
    float AttackSeconds(float Now) const { return Now - AttackStartAt; }
    float FlightSeconds(float Now) const { return Now - Leap.TakeoffAt; }
    const FOpenWillowLeapRecord& GetLeap() const { return Leap; }
    FOpenWillowRockRecord& GetRock() { return Rock; }
    const FOpenWillowRockRecord& GetRock() const { return Rock; }
    int32 LeapCount = 0;
    int32 LeapHits = 0;
    int32 ThrowCount = 0;
    int32 RocksReleased = 0;

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
    float LastStopAt = 0.f;         // where it stops running toward Maya (the last tick's value)

private:
    void UpdateThreat(float Now, const FOpenWillowBrainInput& In, FOpenWillowThreatDen& Den, bool bDamaged, bool bSightBefore);
    void StartSwing(float Now, const FOpenWillowBrainInput& In);
    void ResolveSwing(float Now, const FOpenWillowBrainInput& In, bool bHit);
    // Leaps and throws: the draw at a state update, then the running attack's timeline every tick.
    void ChooseAttack(float Now, const FOpenWillowBrainInput& In, FOpenWillowThreatDen& Den, float FlatDistance, FOpenWillowBrainOrders& Orders);
    void AdvanceAttack(float Now, const FOpenWillowBrainInput& In, FOpenWillowBrainOrders& Orders);
    void EndAttack(const TCHAR* Why);
    float HitAmount(const FOpenWillowBrainInput& In) const;

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
    // The attack the random node runs (None: draw at the next state update) and its timeline. A leap or throw is atomic once started: the
    // note (NATIVE_AI_LEAP_THROW 5) says so for the throw and leaves it open for the leap, so for the leap it is a host stand-in. The close
    // melee does not replace either; death, a hold, passive and a lost target before take-off or a release end it.
    EOpenWillowAttack Pick = EOpenWillowAttack::None;
    EOpenWillowAttack Forced = EOpenWillowAttack::None;
    FRandomStream AttackDraw;
    float AttackStartAt = -10.f;
    int32 LeapPhase = 0;          // 1 launch clip, 2 in the air, 3 landing clip
    const FOpenWillowThrowClip* ThrowClip = nullptr;
    float ThrowLeadCap = 0.f;     // this throw's lead cap: the row's or 0 (note 4.2: which of the two behaviours runs is not decoded)
    int32 ReleasesDone = 0;
    FOpenWillowLeapRecord Leap;
    FOpenWillowRockRecord Rock;
    const FOpenWillowKindAttacks* RunningRow = nullptr;
    const AActor* RunningSelf = nullptr;       // for the log line of an attack that Stop() ends
    FOpenWillowThreatDen* HeldDen = nullptr;   // the den whose resource this enemy holds (given back by EndAttack, which Stop calls)
    bool bHoldsLeap = false;
    bool bHoldsThrow = false;
    float LastNow = 0.f;          // the last tick's time (Stop's log line)
};

// The den's fall-back: when the level is 2 or more and now is past ForgetTime, the level drops to 1 (every DenUpdateInterval).
void UpdateThreatDen(FOpenWillowThreatDen& Den, float Now);

// The level scale of a range enemy's hit on Maya (COMBAT_MELEE stage 2): 1.0 at or below her level, 1.1 one level up,
// 1.2 two up, then a straight line to 3.0 at sixteen up (host stand-in beyond two, UNVERIFIED).
float MeleeLevelScale(int32 EnemyLevel, int32 MayaLevel);

// The ONE delivery of range melee to Maya: OpenWillowApplyDamageToPlayer (OpenWillowMayaState.h, the maya lane's PR #72), shield
// first, then health, then the down state. Returns the damage the pools took.
float RangeDamagePlayer(AActor* Causer, float Amount);
// The knock-back of a leap or a rock on Maya (AOpenWillowWalker::ApplyKnockback): the game's momentum (a horizontal vector) becomes an added
// velocity of momentum / OpenWillowLeapThrow::KnockbackMass (host stand-in, UNVERIFIED). Returns the speed added (0: none, she is down).
float RangeKnockPlayer(const UObject* WorldContext, const FVector& Momentum, const TCHAR* Source);
