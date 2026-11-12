#include "OpenWillowEnemyAI.h"
#include "OpenWillowWalker.h"
#include "OpenWillowMayaState.h"
#include "GameFramework/DamageType.h"
#include "GameFramework/Pawn.h"
#include "Engine/World.h"
#include "Kismet/GameplayStatics.h"

// Host stand-ins for the range den (UNVERIFIED in game). The rules are the ones of AI_AWARENESS_LIVE 0 and 2; the
// order of effects and the times are as the records give them.
using namespace OpenWillowThreat;

namespace
{
// The melee volume of AI_MELEE_TARGETING_LIVE 5, as the fitted rule: a horizontal reach from an origin 100 uu behind the
// enemy, a 64 degree half angle from its facing, and a vertical band. Margin shortens the reach (the stop margin).
bool WithinMeleeVolume(const FVector& EnemyLocation, float YawDegrees, const FVector& MayaLocation, float Margin)
{
    const FVector Forward = FRotator(0.f, YawDegrees, 0.f).Vector();
    const float Height = MayaLocation.Z - EnemyLocation.Z;
    if (Height < MeleeBelow || Height > MeleeAbove) return false;
    FVector ToMaya = MayaLocation - (EnemyLocation - Forward * MeleeBackOffset);
    ToMaya.Z = 0.f;
    const float Distance = ToMaya.Size();
    if (Distance > MeleeReach - Margin) return false;
    if (Distance < KINDA_SMALL_NUMBER) return true;
    return FVector::DotProduct(Forward, ToMaya / Distance) >= FMath::Cos(FMath::DegreesToRadians(MeleeHalfAngleDegrees));
}

// The acceptance table's values (COMBAT_RANGE.md, "Acceptance table: melee per kind"). Adult: the cycle is the live medians of session 2
// of 2026-11-12 (1.220 to 1.223 s, 407 gaps over four recordings at about 120 fps), as the den read of 2026-11-07 had it (1.216 to
// 1.217 s); AI_MELEE_TARGETING_LIVE 4's median 1.249 is the outlier. What makes the cycle this long (clip length, frames) is UNVERIFIED;
// the stand distance is the den read's median (190 to 225 uu, 181 samples). Baby: both from the den read (its grab: 1.133 to 1.134 s over 31 gaps; 106 to
// 118 uu, 347 samples). The default range spawns no baby. Regular and slinger (2026-11-12): the cycle only, from the same session (the
// regular: medians 1.220 to 1.223 s, 96 gaps over three recordings; the slinger's melee: median 1.222 s, 28 gaps, one recording). Their
// stand distance is 0, the stand-in stop: the regular's median call distance sat beyond contact, unexplained, and nobody sampled either
// kind's stand distance.
const FOpenWillowKindMelee KindMeleeRows[] = {
    {TEXT("bullymong_adult"), 1.222f, 201.f},
    {TEXT("bullymong_baby"), 1.133f, 111.f},
    {TEXT("bullymong"), 1.222f, 0.f},
    {TEXT("bullymong_slinger"), 1.222f, 0.f},
};

void LogThreat(float Now, int32 Level, const TCHAR* Reason, const AActor* Who)
{
    const FString Name = Who ? Who->GetName() : FString(TEXT("den"));
    UE_LOG(LogTemp, Display, TEXT("OWRANGE threat level=%d reason=%s enemy=%s t=%.2f"), Level, Reason, *Name, Now);
}
}

FOpenWillowBrainOrders FOpenWillowEnemyBrain::Tick(float Now, const FOpenWillowBrainInput& In, FOpenWillowThreatDen& Den,
    bool& bDamagedByMaya)
{
    FOpenWillowBrainOrders Orders;
    if (In.bPaused || In.bPassive)
    {
        // Dead or phaselocked: the brain is paused. Passive: no notice and no melee. Either way the target and swing end.
        Stop();
        bDamagedByMaya = false;
        return Orders;
    }

    // The state update (every UpdateInterval): the threat rules. The sight rule reads the line of sight of the previous update.
    if (Now >= NextUpdateAt)
    {
        NextUpdateAt = Now + UpdateInterval;
        const bool bDamaged = bDamagedByMaya;
        bDamagedByMaya = false;
        const bool bSightBefore = bExposedBefore;
        bExposedBefore = In.bLineOfSight;
        if (In.bHasMaya) UpdateThreat(Now, In, Den, bDamaged, bSightBefore);
        // The enemy's own target (see bTargeting): valid while the den is alerted and Maya is inside its ThreatRadius.
        const float FromDen = In.bHasMaya
            ? float(FVector2D::Distance(FVector2D(In.MayaLocation.X, In.MayaLocation.Y), Den.Centre)) : 0.f;
        const bool bValid = In.bHasMaya && Den.Level >= SeenInThreatRadius && FromDen < Den.ThreatRadius && Now >= GiveUpUntil;
        const bool bClose = In.bHasMaya && float((In.MayaLocation - In.Location).Size()) - In.CollisionRadius < AICloseThreatRadius;
        if (bValid && (In.bLineOfSight || bClose))
        {
            // Where it sees (or hears) her now: the spot it goes for.
            TargetLocation = In.MayaLocation;
            bHasTargetLocation = true;
        }
        if (bTargeting && !bValid)
        {
            UE_LOG(LogTemp, Display, TEXT("OWRANGE target_lost enemy=%s t=%.2f last_known=%s"), *In.Self->GetName(), Now,
                bHasTargetLocation ? *TargetLocation.ToString() : TEXT("none"));
        }
        bTargeting = bValid;
    }

    // Aware for attack decisions only after AggroTime + MinAggroTimeToConsiderAwareOfTarget (AI_AWARENESS_LIVE 2).
    const bool bAware = bTargeting && In.bHasMaya && Den.Level >= SeenInThreatRadius
        && Now >= Den.AggroTime + MinAggroTimeToConsiderAwareOfTarget;
    const bool bInVolume = In.bHasMaya && WithinMeleeVolume(In.Location, In.YawDegrees, In.MayaLocation, 0.f);
    // Starting a swing and landing it are separate questions. The volume above decides whether a swing lands (the forced swings of
    // AI_MELEE_TARGETING_LIVE 5). A swing starts only inside the stock attack action's range (its AttackLocation range, read by
    // tools/prepare_range.py; the live swings of section 4 had the player within about 300 uu). Measuring it horizontally from the
    // enemy's location is UNVERIFIED. An old manifest has no range: then the reach decides, as before.
    const float FlatDistance = In.bHasMaya ? float(FVector::Dist2D(In.Location, In.MayaLocation)) : 0.f;
    const bool bHasStartRange = In.SwingStartRange > 0.f;
    const bool bInStartRange = bInVolume && (!bHasStartRange || FlatDistance <= In.SwingStartRange);
    // Where it stops running: the kind's live stand distance when it has one, but never inside body contact (its radius plus Maya's: the
    // enemy's root does not collide, so nothing else stops it there); else MeleeStopMargin inside its swing range (stand-in). With a live
    // stand distance the enemy's last step is clamped to it (Orders.StepLimit), so where it stands does not depend on the frame time.
    const bool bLiveStand = In.StandDistance > 0.f;
    const float Contact = In.CollisionRadius + In.MayaRadius;
    const float StopAt = bLiveStand ? FMath::Max(In.StandDistance, Contact)
        : FMath::Max(In.SwingStartRange - MeleeStopMargin, FMath::Min(In.CollisionRadius, In.SwingStartRange));
    LastStopAt = StopAt;
    const bool bInStop = (bHasStartRange || bLiveStand) ? (bInVolume && FlatDistance <= StopAt + (bLiveStand ? StandTolerance : 0.f))
        : In.bHasMaya && WithinMeleeVolume(In.Location, In.YawDegrees, In.MayaLocation, MeleeStopMargin);

    // The damage moment of a swing: SwingDamageDelay after its start, Maya takes the hit if she is still in the volume.
    if (bSwingPending && Now >= SwingStartAt + SwingDamageDelay)
    {
        bSwingPending = false;
        ResolveSwing(Now, In, bInVolume);
    }

    // A swing cycle starts when Maya is inside the start range, one kind's SwingInterval after the previous start. No melee without the
    // manifest's melee numbers for this level.
    const bool bCanSwing = In.ArrivalDamage > 0.f && In.Ratio > 0.f;
    if (bAware && bCanSwing && !bSwingPending && bInStartRange && Now >= LastSwingStart + In.SwingInterval)
    {
        LastSwingDistance = FlatDistance;
        StartSwing(Now, In);
        Orders.bSwingStarted = true;
    }

    // Going for the target's last-known spot; a lost target's spot is visited once (reached: back to the patrol).
    if (!bTargeting && bHasTargetLocation
        && FVector::Dist2D(In.Location, TargetLocation) < SearchArrival) bHasTargetLocation = false;
    Orders.bChase = (bAware || !bTargeting) && bHasTargetLocation;
    Orders.ChaseTo = TargetLocation;
    Orders.bRun = Orders.bChase && !bSwingPending && !(bAware && bInStop);
    // Measured to where Maya is now, as the stop test is, not to ChaseTo (refreshed only on the state update): a Maya walking away is
    // followed, not trailed by up to one update of her movement.
    Orders.StepLimit = (bAware && bLiveStand) ? FMath::Max(0.f, FlatDistance - StopAt) : -1.f;
    Orders.bStandOnContact = bLiveStand && Contact > In.StandDistance;
    return Orders;
}

const FOpenWillowKindMelee* FindKindMelee(const FString& Kind)
{
    for (const FOpenWillowKindMelee& Row : KindMeleeRows)
    {
        if (Kind.Equals(Row.Kind, ESearchCase::CaseSensitive)) return &Row;
    }
    return nullptr;
}

void FOpenWillowEnemyBrain::GiveUp(float Now, float Seconds)
{
    bTargeting = false;
    bHasTargetLocation = false;
    GiveUpUntil = Now + Seconds;
}

void FOpenWillowEnemyBrain::Stop()
{
    bTargeting = false;
    bHasTargetLocation = false;
    bSwingPending = false;
    bExposedBefore = false;   // the sight rule reads a sample of this scene, not one from before the stop
}

void FOpenWillowEnemyBrain::UpdateThreat(float Now, const FOpenWillowBrainInput& In, FOpenWillowThreatDen& Den, bool bDamaged,
    bool bSightBefore)
{
    if (bDamaged)
    {
        // Damage by Maya: level 3 and ForgetTime 25 s from the hit (AI_AWARENESS_LIVE 3). A hit while at level 3 only refreshes it.
        if (Den.Level != ProvokedLevel)
        {
            Den.Level = ProvokedLevel;
            Den.AggroTime = Now;
            LogThreat(Now, ProvokedLevel, TEXT("provoked"), In.Self);
        }
        Den.ForgetTime = Now + MinProvokedTime;
        return;
    }

    const FVector ToMaya = In.MayaLocation - In.Location;
    const FVector Facing = FRotator(0.f, In.YawDegrees, 0.f).Vector();
    const float FromDen = FVector2D::Distance(FVector2D(In.MayaLocation.X, In.MayaLocation.Y), Den.Centre);
    // Sight: the line of sight of the previous update, Maya in the front half-plane, and within the den's threat radius.
    const bool bSight = bSightBefore && FVector::DotProduct(Facing, FVector(ToMaya.X, ToMaya.Y, 0.f)) > 0.f
        && FromDen < Den.ThreatRadius;
    // Proximity: the 3D distance minus the body radius under AICloseThreatRadius, and within the den's patrol radius.
    const bool bProximity = ToMaya.Size() - In.CollisionRadius < AICloseThreatRadius && FromDen < Den.PatrolRadius;
    if (!bSight && !bProximity) return;

    if (Den.Level < SeenInThreatRadius)
    {
        Den.Level = SeenInThreatRadius;
        Den.AggroTime = Now;
        LogThreat(Now, SeenInThreatRadius, bSight ? TEXT("sight") : TEXT("proximity"), In.Self);
    }
    // The notice refreshes ForgetTime to 10 s at every update while a rule holds. At level 3 (provoked) it only extends it.
    if (Den.Level == SeenInThreatRadius) Den.ForgetTime = Now + MinEngagementTime;
    else Den.ForgetTime = FMath::Max(Den.ForgetTime, Now + MinEngagementTime);
}

void FOpenWillowEnemyBrain::StartSwing(float Now, const FOpenWillowBrainInput& In)
{
    bSwingPending = true;
    SwingStartAt = Now;
    PrevSwingStart = LastSwingStart;
    LastSwingStart = Now;
    ++SwingCount;
    UE_LOG(LogTemp, Display, TEXT("OWRANGE swing enemy=%s t=%.2f distance=%.0f start_range=%.0f"), *In.Self->GetName(), Now,
        LastSwingDistance, In.SwingStartRange);
}

void FOpenWillowEnemyBrain::ResolveSwing(float Now, const FOpenWillowBrainInput& In, bool bHit)
{
    // The hit: manifest arrival x the level's ratio x the level scale (COMBAT_MELEE stage 2). A miss delivers nothing.
    LastScale = MeleeLevelScale(In.EnemyLevel, In.MayaLevel);
    LastArrival = In.ArrivalDamage;
    LastRatio = In.Ratio;
    LastHit = bHit;
    LastAt = Now;
    LastApplied = bHit ? RangeDamagePlayer(In.Self, In.ArrivalDamage * In.Ratio * LastScale) : 0.f;
    ++MeleeResolved;
    UE_LOG(LogTemp, Display, TEXT("OWRANGE melee enemy=%s hit=%d arrival=%.3f ratio=%.4f level_scale=%.2f applied=%.2f t=%.2f"),
        *In.Self->GetName(), bHit ? 1 : 0, LastArrival, LastRatio, LastScale, LastApplied, Now);
}

void UpdateThreatDen(FOpenWillowThreatDen& Den, float Now)
{
    if (Now < Den.NextCheckAt) return;
    Den.NextCheckAt = Now + DenUpdateInterval;
    if (Den.Level >= SeenInThreatRadius && Now > Den.ForgetTime)
    {
        // Nobody held a notice rule for MinEngagementTime (or MinProvokedTime after a hit): back to level 1.
        Den.Level = InThreatRadius;
        Den.AggroTime = 0.f;
        LogThreat(Now, InThreatRadius, TEXT("forget"), nullptr);
    }
}

float MeleeLevelScale(int32 EnemyLevel, int32 MayaLevel)
{
    const int32 Up = EnemyLevel - MayaLevel;   // the levels the attacker is above Maya
    if (Up <= 0) return 1.f;                   // an attacker at or below her: 1.0 (LowerLevelAttackerDmgScale)
    if (Up <= 2) return 1.f + 0.1f * Up;       // 1.1 one level up, 1.2 two up (HigherLevelAttackerDmgScale)
    if (Up >= 16) return 3.f;
    return 1.2f + float(Up - 2) * (3.f - 1.2f) / 14.f;   // host stand-in, UNVERIFIED beyond two levels
}

float RangeDamagePlayer(AActor* Causer, float Amount)
{
    if (!Causer || Amount <= 0.f) return 0.f;
    const AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(UGameplayStatics::GetPlayerPawn(Causer, 0));
    const float Before = Walker ? Walker->GetHealth() + Walker->GetShield() : 0.f;
    // The maya lane's entry point for world damage (PR #72): shield first, then health, then Fight For Your Life; the hit
    // comes from the enemy's location (the HUD's hit direction). Returns what the pools took.
    const float Applied = OpenWillowApplyDamageToPlayer(Causer, Amount, Causer, Causer->GetActorLocation());
    const float After = Walker ? Walker->GetHealth() + Walker->GetShield() : 0.f;
    UE_LOG(LogTemp, Display, TEXT("OWRANGE melee_delivery amount=%.2f applied=%.2f maya_pools_before=%.2f after=%.2f"), Amount, Applied,
        Before, After);
    return Applied;
}
