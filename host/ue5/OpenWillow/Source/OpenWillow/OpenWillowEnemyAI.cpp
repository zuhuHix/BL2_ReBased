#include "OpenWillowEnemyAI.h"
#include "OpenWillowWalker.h"
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
        bTargeting = In.bHasMaya && Den.Level >= SeenInThreatRadius;
    }

    // Aware for attack decisions only after AggroTime + MinAggroTimeToConsiderAwareOfTarget (AI_AWARENESS_LIVE 2).
    const bool bAware = bTargeting && In.bHasMaya && Den.Level >= SeenInThreatRadius
        && Now >= Den.AggroTime + MinAggroTimeToConsiderAwareOfTarget;
    const bool bInVolume = In.bHasMaya && WithinMeleeVolume(In.Location, In.YawDegrees, In.MayaLocation, 0.f);
    const bool bInStop = In.bHasMaya && WithinMeleeVolume(In.Location, In.YawDegrees, In.MayaLocation, MeleeStopMargin);

    // The damage moment of a swing: SwingDamageDelay after its start, Maya takes the hit if she is still in the volume.
    if (bSwingPending && Now >= SwingStartAt + SwingDamageDelay)
    {
        bSwingPending = false;
        ResolveSwing(Now, In, bInVolume);
    }

    // A swing cycle starts when Maya is inside the volume, one SwingInterval after the previous start. No melee without the
    // manifest's melee numbers for this level.
    const bool bCanSwing = In.ArrivalDamage > 0.f && In.Ratio > 0.f;
    if (bAware && bCanSwing && !bSwingPending && bInVolume && Now >= LastSwingStart + SwingInterval)
    {
        StartSwing(Now, In);
        Orders.bSwingStarted = true;
    }

    Orders.bChase = bAware;
    Orders.bRun = bAware && !bInStop && !bSwingPending;
    return Orders;
}

void FOpenWillowEnemyBrain::Stop()
{
    bTargeting = false;
    bSwingPending = false;
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
    UE_LOG(LogTemp, Display, TEXT("OWRANGE swing enemy=%s t=%.2f"), *In.Self->GetName(), Now);
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
    UWorld* World = Causer ? Causer->GetWorld() : nullptr;
    APawn* Maya = World ? UGameplayStatics::GetPlayerPawn(World, 0) : nullptr;
    if (!Maya || Amount <= 0.f) return 0.f;
    const AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(Maya);
    const float Before = Walker ? Walker->GetHealth() : 0.f;
    // Stub until PR #72 merges: the engine damage path on Maya's pawn, with no instigator controller.
    // After #72: OpenWillowApplyDamageToPlayer(Causer, Amount, Causer, Causer->GetActorLocation());
    const float Applied = UGameplayStatics::ApplyDamage(Maya, Amount, nullptr, Causer, UDamageType::StaticClass());
    const float After = Walker ? Walker->GetHealth() : 0.f;
    UE_LOG(LogTemp, Display, TEXT("OWRANGE melee_stub amount=%.2f maya_health_before=%.2f after=%.2f"), Amount, Before, After);
    return Applied;
}
