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
        // Dead or phaselocked: the brain is paused. Passive: no notice and no melee. Either way the target and swing end, and so does a
        // leap or throw in progress (a held rock is dropped; a leaper in the air comes down where it is, the enemy's job).
        Orders.bAttackEnded = IsLeaping() || IsThrowing();
        Stop();
        bDamagedByMaya = false;
        return Orders;
    }
    LastNow = Now;

    // The state update (every UpdateInterval): the threat rules. The sight rule reads the line of sight of the previous update.
    const bool bStateUpdate = Now >= NextUpdateAt;
    if (bStateUpdate)
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

    // A leap or throw in progress runs its own timeline, and nothing else this tick: it is atomic once started (the throw by NATIVE_AI_LEAP_THROW
    // 5; the leap a host stand-in, the note leaves it open), so the close melee does not replace it, there is no chase beside it and no swing.
    if (IsLeaping() || IsThrowing())
    {
        AdvanceAttack(Now, In, Orders);
        return Orders;
    }
    // Without a target nothing runs: the random node's melee child ends, and the next target starts with a fresh draw.
    if (!bAware && Pick == EOpenWillowAttack::Melee) Pick = EOpenWillowAttack::None;
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

    // Leaps and thrown rocks (host stand-in for the stock tree, the hub's decision 1): at a state update, with Maya inside the kind's close
    // band the plain swing runs; otherwise one weighted draw per finished attack. A drawn leap or throw starts at once.
    if (bStateUpdate && bAware && In.Attacks && !bSwingPending)
    {
        ChooseAttack(Now, In, Den, FlatDistance, Orders);
        if (IsLeaping() || IsThrowing()) return Orders;
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
    EndAttack(TEXT("stopped"));
    Pick = EOpenWillowAttack::None;
    Forced = EOpenWillowAttack::None;
}

void FOpenWillowEnemyBrain::ChooseAttack(float Now, const FOpenWillowBrainInput& In, FOpenWillowThreatDen& Den, float FlatDistance,
    FOpenWillowBrainOrders& Orders)
{
    using namespace OpenWillowLeapThrow;
    const FOpenWillowKindAttacks& Row = *In.Attacks;
    // The close melee (the baby's grab) sits above the random node: inside its band it runs, and it replaces the node's melee child.
    if (FlatDistance < Row.CloseBand)
    {
        Pick = EOpenWillowAttack::Melee;
        return;
    }
    // While a drawn child runs, the random node re-evaluates only that child (NATIVE_AI_LEAP_THROW 2.2).
    if (Pick != EOpenWillowAttack::None) return;
    // What can run now: the band (horizontal, the enemy's origin to Maya's; no target-size allowance, the hub's decision 2), the line of
    // sight, Maya in front for the throws that ask for it, and a free resource (one leaper and three throwers in the den, decision 3). The
    // note's "not injured" gate on the adult's leap (2.1, 2.2) is not modelled: range enemies have no injured state (host stand-in).
    const FVector ToMaya = In.MayaLocation - In.Location;
    const bool bFront = FVector::DotProduct(FRotator(0.f, In.YawDegrees, 0.f).Vector(), FVector(ToMaya.X, ToMaya.Y, 0.f)) > 0.f;
    const bool bLeapOk = Row.LeapMax > 0.f && FlatDistance >= Row.LeapMin && FlatDistance <= Row.LeapMax && In.bLineOfSight
        && (!Row.bLeapResource || Den.LeapUsers < LeapUsersMax);
    const bool bThrowOk = Row.ThrowMax > 0.f && FlatDistance >= Row.ThrowMin && FlatDistance <= Row.ThrowMax && In.bLineOfSight
        && (!Row.bThrowNeedsFront || bFront) && (!Row.bThrowResource || Den.ThrowUsers < ThrowUsersMax);
    // The host's melee child can always walk in (no pathing failure is modelled), so the back-up throws below the random node never run.
    const bool bMeleeOk = Row.WeightMelee > 0.f;
    EOpenWillowAttack Chosen = EOpenWillowAttack::None;
    bool bForcedNow = false;
    if (Forced != EOpenWillowAttack::None)
    {
        const bool bOk = Forced == EOpenWillowAttack::Leap ? bLeapOk : Forced == EOpenWillowAttack::Throw ? bThrowOk : bMeleeOk;
        if (bOk)
        {
            Chosen = Forced;
            bForcedNow = true;
        }
        else
        {
            UE_LOG(LogTemp, Display, TEXT("OWRANGE attack_forced_refused enemy=%s kind=%s attack=%s distance=%.0f los=%d front=%d leap_users=%d throw_users=%d t=%.2f"),
                *In.Self->GetName(), Row.Kind, OpenWillowAttackName(Forced), FlatDistance, In.bLineOfSight ? 1 : 0, bFront ? 1 : 0, Den.LeapUsers,
                Den.ThrowUsers, Now);
        }
        Forced = EOpenWillowAttack::None;
    }
    if (!bForcedNow) Chosen = OpenWillowDrawAttack(Row, bMeleeOk, bLeapOk, bThrowOk, AttackDraw);
    // Nothing can run (the baby outside its leap band, the slinger outside its throw band): it walks in for the plain swing (stand-in).
    if (Chosen == EOpenWillowAttack::None) Chosen = EOpenWillowAttack::Melee;
    UE_LOG(LogTemp, Display, TEXT("OWRANGE attack_pick enemy=%s kind=%s pick=%s distance=%.0f leap_ok=%d throw_ok=%d forced=%d t=%.2f"),
        *In.Self->GetName(), Row.Kind, OpenWillowAttackName(Chosen), FlatDistance, bLeapOk ? 1 : 0, bThrowOk ? 1 : 0, bForcedNow ? 1 : 0, Now);
    Pick = Chosen;
    if (Chosen == EOpenWillowAttack::Melee) return;

    RunningRow = &Row;
    RunningSelf = In.Self;
    AttackStartAt = Now;
    if (Chosen == EOpenWillowAttack::Leap)
    {
        // The adult's roar before its leap is skipped (the hub's decision 4: two observations contradict a mandatory roar).
        if (Row.bLeapResource)
        {
            ++Den.LeapUsers;
            bHoldsLeap = true;
            HeldDen = &Den;
        }
        LeapPhase = 1;
        Leap = FOpenWillowLeapRecord();
        Leap.StartAt = Now;
        Leap.StartDistance = FlatDistance;
        Leap.bForced = bForcedNow;
        ++LeapCount;
        Orders.bLeapLaunch = true;
        UE_LOG(LogTemp, Display, TEXT("OWRANGE leap_start enemy=%s kind=%s t=%.3f distance=%.0f band_min=%.0f band_max=%.0f launch_clip=%.3f roar=skipped_stand_in resource=%d forced=%d"),
            *In.Self->GetName(), Row.Kind, Now, FlatDistance, Row.LeapMin, Row.LeapMax, Row.LeapStartSeconds, Row.bLeapResource ? 1 : 0, bForcedNow ? 1 : 0);
        return;
    }
    if (Row.bThrowResource)
    {
        ++Den.ThrowUsers;
        bHoldsThrow = true;
        HeldDen = &Den;
    }
    // One clip of the kind's set, equal weights (note 4.1), and the lead: the note's two throw behaviours lead (the row's cap) or do not
    // (cap 0), and what picks one is not decoded (4.2). Host stand-in: an even draw per throw.
    ThrowClip = &Row.Clip[Row.ThrowClips > 1 ? AttackDraw.RandRange(0, Row.ThrowClips - 1) : 0];
    ThrowLeadCap = AttackDraw.FRand() < 0.5f ? Row.ThrowLeadCap : 0.f;
    ReleasesDone = 0;
    Rock = FOpenWillowRockRecord();
    Rock.StartAt = Now;
    Rock.StartDistance = FlatDistance;
    Rock.bForced = bForcedNow;
    ++ThrowCount;
    Orders.bThrowStarted = true;
    UE_LOG(LogTemp, Display, TEXT("OWRANGE throw_start enemy=%s kind=%s t=%.3f distance=%.0f band_min=%.0f band_max=%.0f front=%d clip=%.3f releases=%d lead_cap=%.0f resource=%d forced=%d"),
        *In.Self->GetName(), Row.Kind, Now, FlatDistance, Row.ThrowMin, Row.ThrowMax, bFront ? 1 : 0, ThrowClip->ClipSeconds, ThrowClip->Releases,
        ThrowLeadCap, Row.bThrowResource ? 1 : 0, bForcedNow ? 1 : 0);
}

void FOpenWillowEnemyBrain::AdvanceAttack(float Now, const FOpenWillowBrainInput& In, FOpenWillowBrainOrders& Orders)
{
    using namespace OpenWillowLeapThrow;
    const FOpenWillowKindAttacks& Row = *RunningRow;
    if (IsLeaping())
    {
        // Take-off when the launch clip ends (note 3.1). With no target then, there is no flight.
        if (LeapPhase == 1 && Now >= AttackStartAt + Row.LeapStartSeconds)
        {
            if (!In.bHasMaya)
            {
                EndAttack(TEXT("target_gone_before_takeoff"));
                Orders.bAttackEnded = true;
                return;
            }
            // Centre to centre, the aim raised by the leaper's half height minus Maya's so the bodies' bottoms meet (note 3.1); the lead
            // uses her velocity at take-off (note, decision 5).
            const FVector Start = In.Location + FVector(0.f, 0.f, In.HalfHeight);
            const FVector Aim = In.MayaLocation + FVector(0.f, 0.f, In.HalfHeight - In.MayaHalfHeight);
            Leap.Arc = OpenWillowSolveArc(Start, Aim, In.MayaVelocity, Row.LeapSpeed, Row.LeapAnglePct, Row.LeapLeadCap);
            if (!Leap.Arc.bValid)
            {
                EndAttack(TEXT("no_arc"));
                Orders.bAttackEnded = true;
                return;
            }
            LeapPhase = 2;
            Leap.TakeoffAt = Now;
            Leap.Facing = FVector(Leap.Arc.Velocity.X, Leap.Arc.Velocity.Y, 0.f).GetSafeNormal();
            Orders.bLeapTakeoff = true;
            UE_LOG(LogTemp, Display, TEXT("OWRANGE leap_takeoff enemy=%s kind=%s t=%.3f windup=%.3f d0=%.1f dz=%.1f d1=%.1f lead=%.1f speed=%.0f angle_pct=%.3f lead_cap=%.0f angle=%.2f flight=%.4f gravity=%.1f gravity_scale=%.4f peak=%.1f"),
                *In.Self->GetName(), Row.Kind, Now, Now - AttackStartAt, Leap.Arc.D0, Leap.Arc.Dz, Leap.Arc.D1, Leap.Arc.Lead, Row.LeapSpeed,
                Row.LeapAnglePct, Row.LeapLeadCap, Leap.Arc.AngleDegrees, Leap.Arc.FlightTime, Leap.Arc.Gravity, Leap.Arc.GravityScale(), Leap.Arc.Peak());
            return;
        }
        // Touch-down at the end of the flight time: the landing hit at the start of the landing clip (note 3.3).
        if (LeapPhase == 2 && Now >= Leap.TakeoffAt + Leap.Arc.FlightTime)
        {
            LeapPhase = 3;
            Leap.LandAt = Now;
            Leap.bLanded = true;
            const FVector Centre = Leap.Arc.At(Leap.Arc.FlightTime);
            Leap.LandError = float(FVector::Dist2D(Centre, Leap.Arc.Aim));
            Leap.Expected = HitAmount(In);
            Leap.bHit = In.bHasMaya && OpenWillowLeapLandingHits(Centre, Leap.Facing, In.MayaLocation);
            if (Leap.bHit)
            {
                ++LeapHits;
                Leap.Applied = Leap.Expected > 0.f ? RangeDamagePlayer(In.Self, Leap.Expected) : 0.f;
                // Knockback 1.0: a horizontal momentum of 100,000 away from the leaper (along its facing when it came down on her).
                FVector Away = In.MayaLocation - Centre;
                Away.Z = 0.f;
                if (Away.Size() < 1.f) Away = Leap.Facing;
                Leap.Knock = RangeKnockPlayer(In.Self, Away.GetSafeNormal() * LeapMomentum, TEXT("leap"));
            }
            Orders.bLeapLanded = true;
            UE_LOG(LogTemp, Display, TEXT("OWRANGE leap_land enemy=%s kind=%s t=%.3f flight=%.4f predicted=%.4f attack_to_landed=%.3f land_error=%.1f hit=%d expected=%.2f applied=%.2f knock=%.0f landing_clip=%.3f"),
                *In.Self->GetName(), Row.Kind, Now, Now - Leap.TakeoffAt, Leap.Arc.FlightTime, Now - Leap.StartAt, Leap.LandError, Leap.bHit ? 1 : 0,
                Leap.Expected, Leap.Applied, Leap.Knock, Row.LeapEndSeconds);
            return;
        }
        // The action ends with the landing clip.
        if (LeapPhase == 3 && Now >= Leap.LandAt + Row.LeapEndSeconds)
        {
            EndAttack(TEXT("finished"));
            Orders.bAttackEnded = true;
        }
        return;
    }
    if (!IsThrowing() || !ThrowClip) return;
    // A rock leaves the hand at each release moment of the clip (note 4.1), solved toward Maya now (note 4.2: speed 2000, angle fraction
    // 0.3, no offset, this throw's lead cap). With no target at a release, the throw stops and the held rock is dropped.
    if (ReleasesDone < ThrowClip->Releases && Now >= AttackStartAt + ThrowClip->Release[ReleasesDone])
    {
        if (!In.bHasMaya)
        {
            EndAttack(TEXT("target_gone_before_release"));
            Orders.bAttackEnded = true;
            return;
        }
        const FOpenWillowArc Arc = OpenWillowSolveArc(In.HandLocation, In.MayaLocation, In.MayaVelocity, Row.ThrowSpeed, Row.ThrowAnglePct,
            ThrowLeadCap);
        const float Due = ThrowClip->Release[ReleasesDone];
        ++ReleasesDone;
        if (Arc.bValid)
        {
            Orders.bRockReleased = true;
            Orders.RockArc = Arc;
            Orders.RockDamage = HitAmount(In);
            const float StartDistance = Rock.StartDistance;
            const bool bForced = Rock.bForced;
            Rock = FOpenWillowRockRecord();
            Rock.StartAt = AttackStartAt;
            Rock.ReleaseAt = Now;
            Rock.ReleaseDue = Due;
            Rock.StartDistance = StartDistance;
            Rock.bForced = bForced;
            Rock.Arc = Arc;
            Rock.Damage = Orders.RockDamage;
            ++RocksReleased;
            UE_LOG(LogTemp, Display, TEXT("OWRANGE rock_release enemy=%s kind=%s t=%.3f release=%.3f due=%.3f d0=%.1f dz=%.1f d1=%.1f lead=%.1f speed=%.0f angle_pct=%.3f lead_cap=%.0f angle=%.2f flight=%.4f gravity=%.1f gravity_scale=%.4f peak=%.1f damage=%.2f"),
                *In.Self->GetName(), Row.Kind, Now, Now - AttackStartAt, Due, Arc.D0, Arc.Dz, Arc.D1, Arc.Lead, Row.ThrowSpeed, Row.ThrowAnglePct, ThrowLeadCap,
                Arc.AngleDegrees, Arc.FlightTime, Arc.Gravity, Arc.GravityScale(), Arc.Peak(), Orders.RockDamage);
        }
        else
        {
            UE_LOG(LogTemp, Display, TEXT("OWRANGE rock_release enemy=%s kind=%s t=%.3f no_arc (Maya straight above or below the hand)"), *In.Self->GetName(), Row.Kind, Now);
        }
        return;
    }
    if (Now >= AttackStartAt + ThrowClip->ClipSeconds)
    {
        EndAttack(TEXT("finished"));
        Orders.bAttackEnded = true;
    }
}

void FOpenWillowEnemyBrain::EndAttack(const TCHAR* Why)
{
    // The resource goes back on every way out (finished, stopped, the target gone).
    if (HeldDen)
    {
        if (bHoldsLeap) HeldDen->LeapUsers = FMath::Max(0, HeldDen->LeapUsers - 1);
        if (bHoldsThrow) HeldDen->ThrowUsers = FMath::Max(0, HeldDen->ThrowUsers - 1);
    }
    HeldDen = nullptr;
    bHoldsLeap = false;
    bHoldsThrow = false;
    if (IsLeaping() || IsThrowing())
    {
        UE_LOG(LogTemp, Display, TEXT("OWRANGE attack_end enemy=%s kind=%s attack=%s why=%s t=%.3f"), RunningSelf ? *RunningSelf->GetName() : TEXT("?"),
            RunningRow ? RunningRow->Kind : TEXT("?"), OpenWillowAttackName(Pick), Why, LastNow);
        // One draw per finished attack: the next state update draws again.
        Pick = EOpenWillowAttack::None;
    }
    LeapPhase = 0;
    ThrowClip = nullptr;
    RunningRow = nullptr;
}

float FOpenWillowEnemyBrain::HitAmount(const FOpenWillowBrainInput& In) const
{
    // The plain melee's numbers (COMBAT_MELEE: the leap's delivery gives the same arrival and ratio as the swing; the rock's damage is the
    // same Init_EnemyNonWeaponDamage at scale 1, note 4.3), times the level scale. 0 without the manifest's numbers for this level.
    if (In.ArrivalDamage <= 0.f || In.Ratio <= 0.f) return 0.f;
    return In.ArrivalDamage * In.Ratio * MeleeLevelScale(In.EnemyLevel, In.MayaLevel);
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
    // The swing is the random node's finished attack: the next state update draws again (outside the close band).
    if (Pick == EOpenWillowAttack::Melee) Pick = EOpenWillowAttack::None;
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

float RangeKnockPlayer(const UObject* WorldContext, const FVector& Momentum, const TCHAR* Source)
{
    AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(UGameplayStatics::GetPlayerPawn(WorldContext, 0));
    if (!Walker) return 0.f;
    // Host stand-in (UNVERIFIED): the momentum over a mass of KnockbackMass, horizontal, added to her velocity.
    const FVector Added(Momentum.X / OpenWillowLeapThrow::KnockbackMass, Momentum.Y / OpenWillowLeapThrow::KnockbackMass, 0.f);
    const FVector Applied = Walker->ApplyKnockback(Added);
    UE_LOG(LogTemp, Display, TEXT("OWRANGE knockback source=%s momentum=%.0f added_speed=%.0f applied=%d"), Source, Momentum.Size2D(), Applied.Size2D(),
        Applied.IsNearlyZero() ? 0 : 1);
    return float(Applied.Size2D());
}
