#include "OpenWillowMelee.h"
#include "OpenWillowHitEvent.h"
#include "OpenWillowInventoryWidget.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowVendor.h"
#include "OpenWillowSkills.h"
#include "OpenWillowArmsAnimInstance.h"
#include "OpenWillowWalker.h"
#include "Camera/CameraComponent.h"
#include "GameFramework/PlayerController.h"
#include "CollisionQueryParams.h"
#include "Engine/World.h"
#include "EngineUtils.h"
#include "Animation/AnimSequence.h"

float OpenWillowMelee::DamageAtLevel(int32 Level)
{
    return 2.5f * 8.f * FMath::Pow(1.13f, float(Level));
}

int32 OpenWillowMelee::ChooseVictim(const FVector& Origin, const FVector& Forward, const TArray<FCandidate>& Candidates)
{
    const double MinDot = FMath::Cos(FMath::DegreesToRadians(double(ConeAngleDegrees)));
    int32 Best = INDEX_NONE;
    double BestDot = MinDot;
    for (int32 Index = 0; Index < Candidates.Num(); ++Index)
    {
        const FCandidate& Candidate = Candidates[Index];
        const FVector To = Candidate.Centre - Origin;
        const double Distance = To.Size();
        if (!Candidate.bClearLine || Distance > TraceDistance || Distance <= UE_KINDA_SMALL_NUMBER) continue;
        const double Dot = FVector::DotProduct(Forward, To / Distance);
        if (Dot < BestDot || (Best != INDEX_NONE && Dot == BestDot)) continue;
        Best = Index;
        BestDot = Dot;
    }
    return Best;
}

bool OpenWillowMelee::FSwing::Press(float Now, float Duration)
{
    if (IsBusy(Now)) return false;
    EndsAt = Now + Duration;
    HitAt = Now + HitDelay;
    bHitPending = true;
    return true;
}

bool OpenWillowMelee::FSwing::ConsumeHit(float Now)
{
    if (!bHitPending || Now < HitAt) return false;
    bHitPending = false;
    return true;
}

void AOpenWillowWalker::MeleePressed()
{
    // The input set for going down includes the melee action, so the down state does not refuse it by itself; the host uses the same
    // weapon-ready rule as the trigger (UNVERIFIED while down). A menu holds the keyboard, so only the presentation check is needed.
    if (!bMayaActive || bInventoryPresentation || !DownWeaponReady() || !GetWorld()) return;
    const float Now = GetWorld()->GetTimeSeconds();
    if (Now < ThrowLockUntil)
    {
        ++MeleeRefusedByThrow;
        UE_LOG(LogTemp, Display, TEXT("OWMELEE press refused: grenade throw in progress t=%.2f"), Now);
        return;
    }
    // At least as long as the hit's delay, so the weapons stay busy until the hit has landed.
    const float Duration = FMath::Max(MeleeAnim ? MeleeAnim->GetPlayLength() : OpenWillowMelee::NoClipDuration, OpenWillowMelee::HitDelay);
    if (!MeleeSwing.Press(Now, Duration))
    {
        UE_LOG(LogTemp, Display, TEXT("OWMELEE press refused: swing in progress t=%.2f"), Now);
        return;
    }
    // PerformSharedWeaponActions: the reload stops and the aim ends for the action's length; the trigger waits (MeleeBusy).
    CancelReload();
    AimReleased();
    Burst.interrupt();   // as a reload does: firing again needs a new press
    if (ArmsAnim && MeleeAnim) ArmsAnim->PlayAction(MeleeAnim, 1.f, 1.f, OpenWillowMelee::BlendIn, OpenWillowMelee::BlendOut);
    UE_LOG(LogTemp, Display, TEXT("OWMELEE press t=%.2f duration=%.2f clip=%d"), Now, Duration, MeleeAnim ? 1 : 0);
}

bool AOpenWillowWalker::MeleeBusy() const
{
    return GetWorld() && MeleeSwing.IsBusy(GetWorld()->GetTimeSeconds());
}

void AOpenWillowWalker::TickMelee()
{
    if (!GetWorld() || !Camera || !MeleeBusy()) return;
    const float Now = GetWorld()->GetTimeSeconds();
    // Going down, a status page or a shop ends the swing with its pending hit. A host rule, UNVERIFIED (CONTROLS.md).
    const APlayerController* PC = Cast<APlayerController>(Controller);
    const AOpenWillowMayaHUD* HUD = PC ? Cast<AOpenWillowMayaHUD>(PC->GetHUD()) : nullptr;
    const UOpenWillowVendorSubsystem* Vendors = GetWorld()->GetSubsystem<UOpenWillowVendorSubsystem>();
    const TCHAR* Cancel = IsDown() ? TEXT("Maya went down") : bInventoryPresentation || (HUD && HUD->IsSkillsOpen()) || IsInventoryWidgetOpen() ? TEXT("a status page opened")
        : Vendors && Vendors->IsShopOpen() ? TEXT("a shop opened") : nullptr;
    if (Cancel)
    {
        MeleeSwing.Reset();
        if (ArmsAnim && MeleeAnim && ArmsAnim->Action == MeleeAnim) ArmsAnim->PlayAction(nullptr);
        UE_LOG(LogTemp, Display, TEXT("OWMELEE swing cancelled: %s t=%.2f"), Cancel, Now);
        return;
    }
    if (!MeleeSwing.ConsumeHit(Now)) return;

    // The stock origin comes from a native the bytecode does not show; the view is the host's stand-in (UNVERIFIED).
    const FVector Forward = GetViewRotation().Vector();
    const FVector Origin = Camera->GetComponentLocation() + Forward * OpenWillowMelee::HitStartOffset;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OWMelee), true, this);
    FHitResult Hit;
    const FVector End = Origin + Forward * OpenWillowMelee::TraceDistance;
    const bool bBlocked = GetWorld()->LineTraceSingleByChannel(Hit, Origin, End, ECC_Visibility, Query);
    auto Takes = [](AActor* Actor)
    {
        const IOpenWillowHitTarget* Target = OpenWillowHit::AsTarget(Actor);
        return Target && Target->CanTakeHits();
    };

    AActor* Victim = bBlocked && Takes(Hit.GetActor()) ? Hit.GetActor() : nullptr;
    const TCHAR* How = TEXT("trace");
    if (!Victim && !bBlocked)
    {
        // The trace reached nothing: the cone. A trace that struck a wall ends the swing there.
        How = TEXT("cone");
        TArray<AActor*> Actors;
        TArray<OpenWillowMelee::FCandidate> Candidates;
        for (TActorIterator<AActor> It(GetWorld()); It; ++It)
        {
            if (*It == this || !Takes(*It)) continue;
            FVector Centre;
            float Radius = 0.f;
            OpenWillowHit::AsTarget(*It)->GetHitCentre(Centre, Radius);
            FHitResult Line;
            FCollisionQueryParams LineQuery(SCENE_QUERY_STAT(OWMeleeLine), true, this);
            LineQuery.AddIgnoredActor(*It);
            OpenWillowMelee::FCandidate Candidate;
            Candidate.Centre = Centre;
            Candidate.bClearLine = !GetWorld()->LineTraceSingleByChannel(Line, Origin, Centre, ECC_Visibility, LineQuery);
            Actors.Add(*It);
            Candidates.Add(Candidate);
        }
        const int32 Chosen = OpenWillowMelee::ChooseVictim(Origin, Forward, Candidates);
        if (Chosen != INDEX_NONE) Victim = Actors[Chosen];
    }
    if (!Victim)
    {
        UE_LOG(LogTemp, Display, TEXT("OWMELEE swing t=%.2f missed"), Now);
        return;
    }

    // Through the shots' damage path, so targets, hit counts and the hit event behave as for a bullet.
    FOpenWillowHitEvent In;
    In.Source = EOpenWillowHitSource::Melee;
    In.WeaponId = TEXT("melee");
    In.Origin = Origin;
    In.Damage = OpenWillowMelee::DamageAtLevel(Skills ? Skills->GetLevel() : 1);
    In.DamageType = TEXT("GD_Impact.DamageType.DmgType_Normal");
    const FVector ToVictim = Victim->GetActorLocation() - GetActorLocation();
    In.Direction = ToVictim.GetSafeNormal();
    // Knockback x the factor, along the horizontal line from attacker to victim (NATIVE_AI_MELEE.md, live for a bullymong).
    In.Momentum = ToVictim.GetSafeNormal2D() * OpenWillowMelee::Knockback * OpenWillowMelee::KnockbackScale;
    // No bone: the melee hit is not a critical here (the game has a melee critical attribute; how it applies is unread).
    FHitResult VictimHit = Hit;
    VictimHit.BoneName = NAME_None;
    if (!bBlocked) VictimHit.ImpactPoint = Victim->GetActorLocation();
    const float Applied = ApplyShotDamage(Victim, In, VictimHit);
    UE_LOG(LogTemp, Display, TEXT("OWMELEE hit %s by %s damage=%.2f applied=%.2f t=%.2f"), *Victim->GetName(), How, In.Damage, Applied, Now);
}
