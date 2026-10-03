#include "OpenWillowCombatTarget.h"
#include "OpenWillowShotFx.h"
#include "OpenWillowQuest.h"
#include "OpenWillowWalker.h"
#include "Animation/AnimSequence.h"
#include "Components/CapsuleComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Components/PointLightComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/DamageEvents.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "Materials/MaterialInstanceDynamic.h"

namespace
{
const FLinearColor BodyColor(0.20f, 0.07f, 0.04f);
const FLinearColor HeadColor(0.55f, 0.42f, 0.30f);
constexpr float RespawnSeconds = 3.f;

// Pivot height at Held seconds into the lift from From to To (LiftActionSkill.UpdateLiftedPawn / GetLiftLocation,
// read not run): up to SnapTimePct of the lift, From -> snap point (SnapHeightPct of the way) by a^2; then snap point ->
// To by 1 - (1 - a)^2 (InterpEaseOut with exponent 2). The bob after the lift is in Tick (GetBobLocation).
float LiftHeightAt(const FOpenWillowPhaselockData& D, float Held, float From, float To)
{
    if (Held >= D.LiftDuration) return To;
    const float U = D.LiftDuration > 0.f ? Held / D.LiftDuration : 1.f;
    const float Snap = FMath::Lerp(From, To, D.SnapHeightPct);
    if (U < D.SnapTimePct)
        return FMath::Lerp(From, Snap, FMath::Square(U / D.SnapTimePct));
    return FMath::InterpEaseOut(Snap, To, (U - D.SnapTimePct) / FMath::Max(1.f - D.SnapTimePct, KINDA_SMALL_NUMBER), 2.f);
}

UStaticMeshComponent* Part(AActor* Owner, USceneComponent* Parent, const TCHAR* Name,
    const TCHAR* Mesh, const FVector& Location, const FVector& Scale)
{
    auto* Component = Owner->CreateDefaultSubobject<UStaticMeshComponent>(Name);
    Component->SetupAttachment(Parent);
    Component->SetStaticMesh(LoadObject<UStaticMesh>(nullptr, Mesh));
    Component->SetRelativeLocation(Location);
    Component->SetRelativeScale3D(Scale);
    Component->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
    Component->SetCollisionResponseToAllChannels(ECR_Ignore);
    Component->SetCollisionResponseToChannel(ECC_Visibility, ECR_Block);
    return Component;
}

UMaterialInstanceDynamic* Tint(AActor* Owner, UStaticMeshComponent* Component, const FLinearColor& Color)
{
    UMaterialInterface* Base = LoadObject<UMaterialInterface>(nullptr,
        TEXT("/Engine/BasicShapes/BasicShapeMaterial.BasicShapeMaterial"));
    if (!Base) return nullptr;
    auto* Material = UMaterialInstanceDynamic::Create(Base, Owner);
    Material->SetVectorParameterValue(TEXT("Color"), Color);
    Component->SetMaterial(0, Material);
    return Material;
}
}

AOpenWillowCombatTarget::AOpenWillowCombatTarget()
{
    PrimaryActorTick.bCanEverTick = true;
    Base = CreateDefaultSubobject<USceneComponent>(TEXT("Floor"));
    RootComponent = Base;
    Pivot = CreateDefaultSubobject<USceneComponent>(TEXT("Pivot"));
    Pivot->SetupAttachment(Base);
    // A plain training dummy: engine cylinder is 100 cm tall, sphere 100 cm wide.
    Post = Part(this, Pivot, TEXT("Post"), TEXT("/Engine/BasicShapes/Cylinder.Cylinder"),
        FVector(0, 0, 45), FVector(0.12f, 0.12f, 0.9f));
    Torso = Part(this, Pivot, TEXT("Torso"), TEXT("/Engine/BasicShapes/Cylinder.Cylinder"),
        FVector(0, 0, 125), FVector(0.55f, 0.4f, 0.75f));
    Head = Part(this, Pivot, TEXT("Head"), TEXT("/Engine/BasicShapes/Sphere.Sphere"),
        FVector(0, 0, 182), FVector(0.3f));
    // The Phaselock bubble, light and target clips are created per lock from the stock data (BeginPhaselock).
}

void AOpenWillowCombatTarget::BeginPlay()
{
    Super::BeginPlay();
    HomeLocation = GetActorLocation();
    BodyMaterial = Tint(this, Torso, BodyColor);
    Tint(this, Post, FLinearColor(0.05f, 0.05f, 0.05f));
    HeadMaterial = Tint(this, Head, HeadColor);
}

bool AOpenWillowCombatTarget::UseStockPawn(const FString& MeshPath, const FString& IdlePath, const FString& DeathPath,
    const FVector& MeshOffset)
{
    USkeletalMesh* Mesh = LoadObject<USkeletalMesh>(nullptr, *MeshPath);
    UAnimSequence* Idle = LoadObject<UAnimSequence>(nullptr, *IdlePath);
    StockDeath = LoadObject<UAnimSequence>(nullptr, *DeathPath);
    if (!Mesh || !Idle || !StockDeath) return false;
    for (UStaticMeshComponent* Shape : {Post.Get(), Torso.Get(), Head.Get()})
    {
        Shape->SetHiddenInGame(true);
        Shape->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    }
    StockMesh = NewObject<USkeletalMeshComponent>(this, TEXT("StockPawnMesh"));
    StockMesh->SetupAttachment(Pivot);
    StockMesh->SetSkeletalMesh(Mesh);
    StockMesh->SetRelativeLocation(MeshOffset);
    StockMesh->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    StockMesh->RegisterComponent();
    StockMesh->PlayAnimation(Idle, true);
    StockIdle = Idle;
    // SpecialMove_PhaseLock plays PhaseLock_Lift/Loop/Fall/Land from the pawn's own AnimSets. The host looks for them
    // next to the imported idle clip (same name with the clip part replaced); stock data gives the slice dummy none
    // (PHASELOCK_STOCK_DATA.md), so it keeps its idle and no other pawn's clips are borrowed.
    if (IdlePath.Contains(TEXT("Idle")))
    {
        auto Sibling = [&IdlePath](const TCHAR* Clip)
        {
            return LoadObject<UAnimSequence>(nullptr, *IdlePath.Replace(TEXT("Idle"), Clip), nullptr, LOAD_NoWarn | LOAD_Quiet);
        };
        LiftClip = Sibling(TEXT("PhaseLock_Lift"));
        LoopClip = Sibling(TEXT("PhaseLock_Loop"));
        FallClip = Sibling(TEXT("PhaseLock_Fall"));
        LandClip = Sibling(TEXT("PhaseLock_Land"));
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow target %s PhaseLock clips: lift %s loop %s fall %s land %s"), *GetName(),
        LiftClip ? TEXT("yes") : TEXT("no"), LoopClip ? TEXT("yes") : TEXT("no"), FallClip ? TEXT("yes") : TEXT("no"),
        LandClip ? TEXT("yes") : TEXT("no"));
    // Hit volume: a capsule from the imported mesh's bounds (host-chosen shape; the stock pawn's collision cylinder
    // is not read). The narrower horizontal extent is the radius, so outstretched bind-pose arms do not widen it.
    const FBoxSphereBounds Bounds = Mesh->GetBounds();
    HitVolume = NewObject<UCapsuleComponent>(this, TEXT("StockHitVolume"));
    HitVolume->SetupAttachment(Pivot);
    HitVolume->SetRelativeLocation(MeshOffset + Bounds.Origin);
    HitVolume->SetCapsuleSize(FMath::Min(Bounds.BoxExtent.X, Bounds.BoxExtent.Y), Bounds.BoxExtent.Z);
    HitVolume->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
    HitVolume->SetCollisionResponseToAllChannels(ECR_Ignore);
    HitVolume->SetCollisionResponseToChannel(ECC_Visibility, ECR_Block);
    HitVolume->SetHiddenInGame(true);
    HitVolume->RegisterComponent();
    return true;
}

float AOpenWillowCombatTarget::PhaselockReleasedAt() const { return ReleasedAt; }

float AOpenWillowCombatTarget::AutoAimRadius() const
{
    if (HitVolume) return HitVolume->GetScaledCapsuleRadius();
    const FVector Extent = Torso->Bounds.BoxExtent;
    return float(FMath::Min(Extent.X, Extent.Y));
}

float AOpenWillowCombatTarget::MeshBoundsRadius() const
{
    if (StockMesh) return StockMesh->Bounds.SphereRadius;
    FBoxSphereBounds Bounds = Post->Bounds;
    Bounds = Bounds + Torso->Bounds;
    Bounds = Bounds + Head->Bounds;
    return Bounds.SphereRadius;
}

FString AOpenWillowCombatTarget::PresentationReport() const
{
    FString Out;
    for (const UOpenWillowFxComponent* C : {BubbleIntro.Get(), BubbleLoop.Get(), BubbleOutro.Get()})
        if (C) Out += TEXT("[") + C->Describe() + TEXT("] ");
    return Out;
}

float AOpenWillowCombatTarget::PhaselockLightIntensity() const
{
    return LockLight ? LockLight->Intensity : 0.f;
}

UOpenWillowFxComponent* AOpenWillowCombatTarget::SpawnBubble(const FString& TemplateName, float Now)
{
    FString Error;
    const FOwFxTemplate* Template = FOwFxTemplate::Load(Fx.EmitterDir, TemplateName, Error);
    if (!Template)
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow Phaselock bubble: %s"), *Error);
        return nullptr;
    }
    // LiftActionSkill.SpawnBubbleFX / TransitionToBubbleFX*: an emitter at the lift end, owned by the lifted pawn,
    // drawn at Mesh.Bounds.SphereRadius / BubbleFXScale. Basing it on the pawn (so it follows the bob) is a host reading.
    UOpenWillowFxComponent* C = NewObject<UOpenWillowFxComponent>(this);
    C->SetupAttachment(Pivot);
    C->SetRelativeLocation(BubbleOffset);
    C->RegisterComponent();
    C->Play(Template, BubbleScale);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock bubble %s at %.2f s, draw scale %.3f (radius %.1f / %.1f), %d emitters skipped"),
        *TemplateName, Now - LockStartedAt, BubbleScale, MeshBoundsRadius(), Fx.BubbleScaleDivisor, C->SkippedEmitters());
    return C;
}

void AOpenWillowCombatTarget::UpdatePresentation(float Now, float Held)
{
    if (!bFx) return;
    const FOpenWillowPhaselockTimeline& T = LockTimeline;
    // UpdatePhaselockLight: brightness x the lift fraction while lifting, x 1 while locked, x (1 - outro fraction) in
    // the outro (state fractions of the script's StateDuration).
    if (LockLight)
    {
        float Fraction = 1.f;
        if (Held < T.LockedAt) Fraction = T.LockedAt > 0.f ? Held / T.LockedAt : 1.f;
        else if (Held >= T.OutroAt) Fraction = 1.f - (Held - T.OutroAt) / FMath::Max(T.ReleasedAt - T.OutroAt, KINDA_SMALL_NUMBER);
        LockLight->SetIntensity(Fx.LightBrightness * FMath::Clamp(Fraction, 0.f, 1.f));
    }
    if (Held >= T.LockedAt && BubbleStageNow == 0)
    {
        // LockTarget -> SpawnBubbleFX: the intro template, scaled from the pawn's mesh bounds at this moment.
        BubbleScale = MeshBoundsRadius() / Fx.BubbleScaleDivisor;
        BubbleIntro = SpawnBubble(Fx.BubbleFadeIn, Now);
        BubbleStageNow = 1;
    }
    if (Held >= T.LockedAt + Fx.BubbleIntroTime && BubbleStageNow == 1 && Held < T.OutroAt)
    {
        // TransitionToBubbleFXLoop: life span and PhaselockLifeTime = the locked state's duration + the outro overlap;
        // the collapse starts so that it reaches MaxCollapseValue when the lock ends.
        const float StateDuration = T.OutroAt - T.LockedAt;
        CollapseStartAt = Now + StateDuration - Fx.CollapseDuration - Fx.BubbleIntroTime;
        BubbleLoop = NewObject<UOpenWillowFxComponent>(this);
        FString Error;
        if (const FOwFxTemplate* Template = FOwFxTemplate::Load(Fx.EmitterDir, Fx.BubbleLoop, Error))
        {
            BubbleLoop->SetupAttachment(Pivot);
            BubbleLoop->SetRelativeLocation(BubbleOffset);
            BubbleLoop->RegisterComponent();
            BubbleLoop->SetFloatParameter(Fx.LifeTimeParam, StateDuration + Fx.BubbleOutroOverlap);
            BubbleLoop->SetFloatParameter(Fx.CollapseParam, 0.f);
            BubbleLoop->Play(Template, BubbleScale);
            UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock bubble loop at %.2f s: life %.2f s, collapse from %.2f s"),
                Held, StateDuration + Fx.BubbleOutroOverlap, CollapseStartAt - LockStartedAt);
        }
        else UE_LOG(LogTemp, Warning, TEXT("OpenWillow Phaselock bubble: %s"), *Error);
        BubbleStageNow = 2;
    }
    if (BubbleLoop)
    {
        // UpdateEffects: (now - CollapseStartTime) / CollapseDuration x MaxCollapseValue, unclamped in the script; the
        // template's parameter range clamps it.
        CollapseNow = (Now - CollapseStartAt) / Fx.CollapseDuration * Fx.MaxCollapse;
        BubbleLoop->SetFloatParameter(Fx.CollapseParam, CollapseNow);
    }
    if (Held >= T.OutroAt && BubbleStageNow >= 1 && BubbleStageNow < 3)
    {
        // StartOutro -> TransitionToBubbleFXOutro: the loop is destroyed and the end template spawned.
        if (BubbleLoop) BubbleLoop->DestroyComponent();
        BubbleLoop = nullptr;
        BubbleOutro = SpawnBubble(Fx.BubbleFadeOut, Now);
        BubbleStageNow = 3;
    }
}

void AOpenWillowCombatTarget::ClearPresentation()
{
    for (UOpenWillowFxComponent* C : {BubbleIntro.Get(), BubbleLoop.Get(), BubbleOutro.Get()})
        if (C) C->DestroyComponent();
    BubbleIntro = BubbleLoop = BubbleOutro = nullptr;
    if (LockLight) LockLight->DestroyComponent();
    LockLight = nullptr;
}

void AOpenWillowCombatTarget::EndPhaselockNow(float Now)
{
    if (!bPhaselocked) return;
    const float Held = Now - LockStartedAt;
    LockTimeline.OutroAt = FMath::Min(LockTimeline.OutroAt, Held);
    LockTimeline.ReleasedAt = FMath::Min(LockTimeline.ReleasedAt, Held);
    LockEndsAt = FMath::Min(LockEndsAt, Now);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock on %s ended early at %.2f s"), *GetName(), Held);
}
float AOpenWillowCombatTarget::LiftedHeight() const { return Pivot->GetRelativeLocation().Z; }

FVector AOpenWillowCombatTarget::AimPoint() const
{
    if (HitVolume) return HitVolume->GetComponentLocation();
    return Torso->GetComponentLocation() + FVector(0, 0, 10);
}

void AOpenWillowCombatTarget::CollisionCentre(FVector& OutCentre, float& OutHalfHeight) const
{
    // Colliding components only: the engine shapes, or the stock pawn's hit capsule (the shell and the stock mesh have
    // no collision).
    FVector Extent;
    GetActorBounds(true, OutCentre, Extent);
    OutHalfHeight = Extent.Z;
}

float AOpenWillowCombatTarget::PhaselockLiftHeight(const FOpenWillowPhaselockData& Data) const
{
    // LiftActionSkill.BeginLifting (script, read not run). Both traces are Actor.Trace calls on the lifted pawn with
    // extent (1,1,1) and TRACEFLAG_Blocking ("8" in Engine.upk), actors included. Host choices, UNVERIFIED: a 1 uu box
    // sweep on ECC_Visibility (the channel the host's floor and Phaselock traces use) that ignores this target; the
    // bounds of the colliding components for the pawn's Location and CylinderComponent.CollisionHeight; a sweep that
    // starts inside geometry counts as no hit, as in the host's targeting.
    FVector Centre;
    float HalfHeight = 0.f;
    CollisionCentre(Centre, HalfHeight);
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowPhaselockLift), true, this);
    const FCollisionShape Box = FCollisionShape::MakeBox(FVector(1.f));
    FHitResult Hit;
    // Ground within HeightFromGround below the centre; without it the end stays at the centre (no lift).
    if (!GetWorld()->SweepSingleByChannel(Hit, Centre, Centre - FVector(0, 0, Data.HeightFromGround), FQuat::Identity,
            ECC_Visibility, Box, Query) || Hit.bStartPenetrating)
        return 0.f;
    FVector End = Hit.Location + FVector(0, 0, HalfHeight + Data.HeightFromGround);
    // Ceiling clamp: a surface between the centre and the end lowers the end to that surface minus the half height, so
    // the top of the collision touches it. Only the path of the centre is traced: a surface above the end but below
    // the end's top is not seen by the script either. A surface closer than the half height above the centre (a host
    // target placed overlapping it, which a UE3 pawn's collision would prevent) gives a negative lift, kept as the
    // script computes it.
    if (GetWorld()->SweepSingleByChannel(Hit, Centre, End, FQuat::Identity, ECC_Visibility, Box, Query) && !Hit.bStartPenetrating)
        End = Hit.Location - FVector(0, 0, HalfHeight);
    return End.Z - Centre.Z;
}

bool AOpenWillowCombatTarget::BeginPhaselock(float Now, const FOpenWillowPhaselockData& Data, const FOpenWillowPhaselockTimeline& Timeline,
    const FOpenWillowPhaselockFxData* InFx)
{
    if (bPhaselocked || bDead) return false;
    Lock = Data;
    LockTimeline = Timeline;
    LockStartedAt = Now;
    LockEndsAt = Now + Timeline.ReleasedAt;
    DropStartedAt = -10;
    LiftFrom = Pivot->GetRelativeLocation().Z;
    const float Lift = PhaselockLiftHeight(Data);
    LiftTo = LiftFrom + Lift;
    bBobStarted = false;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock lift for %s: %.1f -> %.1f uu (HeightFromGround %.0f)"),
        *GetName(), LiftFrom, LiftTo, Data.HeightFromGround);
    bPhaselocked = true;
    ClearPresentation();
    BubbleStageNow = 0;
    CollapseNow = 0;
    bFx = InFx && InFx->bLoaded;
    if (bFx)
    {
        Fx = *InFx;
        // The lift end: the pawn's location (collision centre here) raised by the lift, in Pivot space.
        FVector Centre;
        float Half = 0.f;
        CollisionCentre(Centre, Half);
        BubbleOffset = Pivot->GetComponentTransform().InverseTransformPosition(Centre);
        // PhaselockLight: the class default PointLightComponent (radius, brightness, colour, falloff, no shadows),
        // attached to the lifted pawn. UE3 brightness is used as UE5's unitless intensity with UE3-style radial falloff
        // (bUseInverseSquaredFalloff off, same exponent) and no indirect lighting, as a UE3 dynamic light has none; that
        // mapping is UNVERIFIED (exposure differs).
        LockLight = NewObject<UPointLightComponent>(this);
        LockLight->SetupAttachment(Pivot);
        LockLight->SetRelativeLocation(BubbleOffset);
        LockLight->bUseInverseSquaredFalloff = false;
        LockLight->SetIntensityUnits(ELightUnits::Unitless);
        LockLight->SetLightFalloffExponent(Fx.LightFalloffExponent);
        LockLight->SetAttenuationRadius(Fx.LightRadius);
        LockLight->SetLightColor(FLinearColor(Fx.LightColor));
        LockLight->SetCastShadows(Fx.bLightShadows);
        LockLight->SetIndirectLightingIntensity(0.f);
        LockLight->SetIntensity(0.f);
        // Host stand-in (UNVERIFIED): the data says the light affects static and dynamic primitives
        // (LAC_DYNAMIC_AND_STATIC_AFFECTING), yet the 2026-10-02 game capture shows no pool under a lifted bullymong,
        // while UE5 draws a strong violet one on Sanctuary's floor. Until the UE3 -> UE5 brightness mapping is known,
        // the light reaches only this target (lighting channel 1, which the target's primitives join).
        LockLight->SetLightingChannels(false, true, false);
        TInlineComponentArray<UPrimitiveComponent*> Primitives(this);
        for (UPrimitiveComponent* Primitive : Primitives)
            Primitive->SetLightingChannels(Primitive->LightingChannels.bChannel0, true, Primitive->LightingChannels.bChannel2);
        LockLight->RegisterComponent();
    }
    if (StockMesh && LiftClip)
    {
        StockMesh->PlayAnimation(LiftClip, false);
        AnimClipEndsAt = Now + LiftClip->GetPlayLength();
        AnimStage = 1;
    }
    else if (StockMesh)
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock: %s has no PhaseLock_Lift clip in its imported AnimSet (stock data); idle kept"), *GetName());
    return true;
}

void AOpenWillowCombatTarget::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    const float Now = GetWorld()->GetTimeSeconds();
    Popups.RemoveAll([Now](const FOpenWillowDamagePopup& Popup) { return Now - Popup.Born > 1.2f; });
    if (bDead)
    {
        if (StockMesh) return; // the stock dummy keeps its death pose until the sequence destroys it
        const float Since = Now - DiedAt;
        Pivot->SetRelativeScale3D(FVector(FMath::Max(0.01f, 1.f - Since / 0.25f)));
        if (Since < RespawnSeconds) return;
        bDead = false;
        Health = MaxHealth;
        SetActorLocation(HomeLocation);
        Pivot->SetRelativeScale3D(FVector(1.f));
        Pivot->SetRelativeLocation(FVector::ZeroVector);
        SetActorHiddenInGame(false);
        SetActorEnableCollision(true);
        return;
    }
    // Damped spring: hits kick the dummy, which settles back upright.
    WobbleVelocity += (-60.f * Wobble - 7.f * WobbleVelocity) * DeltaSeconds;
    Wobble += WobbleVelocity * DeltaSeconds;
    float Height = Pivot->GetRelativeLocation().Z;
    if (bPhaselocked)
    {
        const float Held = Now - LockStartedAt;
        Height = LiftHeightAt(Lock, Held, LiftFrom, LiftTo);
        if (Held >= Lock.LiftDuration)
        {
            // GetBobLocation: LiftEndLocation + BobAmplitude x sin((now - SkillStartTime) x BobFrequency x pi), then
            // VInterpTo(previous, target, dt, 1) starting from the lift end (NATIVE_PHASELOCK_TARGETING.md section 5).
            if (!bBobStarted) { BobHeight = LiftTo; bBobStarted = true; }
            const float Target = LiftTo + Lock.BobAmplitude * FMath::Sin(Held * Lock.BobFrequency * PI);
            BobHeight += (Target - BobHeight) * FMath::Clamp(DeltaSeconds * 1.f, 0.f, 1.f);
            Height = BobHeight;
        }
        UpdatePresentation(Now, Held);
        if (AnimStage == 1 && Now >= AnimClipEndsAt && StockMesh && LoopClip) { StockMesh->PlayAnimation(LoopClip, true); AnimStage = 2; }
        // The script moves the lifted pawn without rotating it; only hit wobble remains.
        Pivot->SetRelativeRotation(FRotator(Wobble.Y, Pivot->GetRelativeRotation().Yaw, Wobble.X));
        FallVelocity = 0.f;
        if (Now >= LockEndsAt)
        {
            // ReleaseTarget: drop over DropTime; OnReleasedTarget activates Skill_Phaselock_DiminishingReturns on the
            // target for its InitialDuration.
            bPhaselocked = false;
            ReleasedAt = Now;
            DropStartedAt = Now;
            DropFromHeight = Height;
            DiminishedUntil = Now + Lock.DiminishingSeconds;
            if (LockLight) LockLight->DestroyComponent();
            LockLight = nullptr;
            if (BubbleLoop) BubbleLoop->DestroyComponent();
            BubbleLoop = nullptr;
            // DropTarget: the drop clip stretched to DropTime, else the loop is stopped (idle).
            if (StockMesh && FallClip)
            {
                StockMesh->PlayAnimation(FallClip, false);
                StockMesh->SetPlayRate(FallClip->GetPlayLength() / FMath::Max(Lock.DropTime, KINDA_SMALL_NUMBER));
                AnimStage = 3;
            }
            else if (StockMesh && AnimStage != 0) { StockMesh->PlayAnimation(StockIdle, true); AnimStage = 0; }
            UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock released %s after %.2f s"), *GetName(), Held);
        }
    }
    else if (Now - DropStartedAt < Lock.DropTime)
    {
        // Drop back over DropTime (quadratic ease-in: host shape, UNVERIFIED; the stock drop plays DropAnim).
        Height = DropFromHeight * (1.f - FMath::Square((Now - DropStartedAt) / Lock.DropTime));
        FallVelocity = 0.f;
        Pivot->SetRelativeRotation(FRotator(Wobble.Y, Pivot->GetRelativeRotation().Yaw, Wobble.X));
    }
    else
    {
        // Released targets drop back under BL2's -500 cm/s^2 world gravity.
        FallVelocity = Height > 0.f ? FallVelocity - 500.f * DeltaSeconds : 0.f;
        Height = FMath::Max(0.f, Height + FallVelocity * DeltaSeconds);
        Pivot->SetRelativeRotation(FRotator(Wobble.Y, Pivot->GetRelativeRotation().Yaw, Wobble.X));
    }
    // CheckLandTarget: the land clip once the pawn is down again, then idle.
    if (AnimStage == 3 && Height <= 0.f && StockMesh)
    {
        StockMesh->SetPlayRate(1.f);
        if (LandClip) { StockMesh->PlayAnimation(LandClip, false); AnimClipEndsAt = Now + LandClip->GetPlayLength(); AnimStage = 4; }
        else { StockMesh->PlayAnimation(StockIdle, true); AnimStage = 0; }
    }
    if (AnimStage == 4 && Now >= AnimClipEndsAt && StockMesh) { StockMesh->PlayAnimation(StockIdle, true); AnimStage = 0; }
    // Finished bubble templates are removed (the intro and end emitters have no life span of their own here).
    if (BubbleIntro && BubbleIntro->IsFinished()) { BubbleIntro->DestroyComponent(); BubbleIntro = nullptr; }
    if (BubbleOutro && BubbleOutro->IsFinished()) { BubbleOutro->DestroyComponent(); BubbleOutro = nullptr; }
    Pivot->SetRelativeLocation(FVector(0, 0, Height));
    const float Flash = FMath::Clamp(1.f - (Now - LastHitAt) / 0.12f, 0.f, 1.f);
    if (BodyMaterial) BodyMaterial->SetVectorParameterValue(TEXT("Color"),
        FMath::Lerp(BodyColor, FLinearColor(1.f, 0.85f, 0.7f), Flash));
    if (HeadMaterial) HeadMaterial->SetVectorParameterValue(TEXT("Color"),
        FMath::Lerp(HeadColor, FLinearColor(1.f, 0.9f, 0.8f), Flash));
}

float AOpenWillowCombatTarget::TakeDamage(float DamageAmount, const FDamageEvent& DamageEvent,
    AController* EventInstigator, AActor* DamageCauser)
{
    if (bDead) return 0.f;
    // The dummy's own behavior provider (OnTakeDamage) runs in the quest component of the shooter, with the stock
    // damage type path of the shot in flight ("" = None for damage that is not one of Maya's shots).
    if (const auto* Shooter = Cast<AOpenWillowWalker>(DamageCauser))
        if (UOpenWillowQuest* Quest = Shooter->GetQuest(); Quest && Quest->Enabled())
            Quest->OnDummyDamaged(this, Shooter->ShotDamageType());
    const float Now = GetWorld()->GetTimeSeconds();
    const float Applied = FMath::Clamp(DamageAmount, 0.f, Health);
    Health -= Applied;
    LastHitAt = Now;
    FVector Where = AimPoint();
    FVector Direction = FVector::ZeroVector;
    if (DamageEvent.IsOfType(FPointDamageEvent::ClassID))
    {
        const auto& Point = static_cast<const FPointDamageEvent&>(DamageEvent);
        Where = Point.HitInfo.ImpactPoint;
        Direction = Point.ShotDirection;
    }
    const FVector Local = GetActorTransform().InverseTransformVectorNoScale(Direction);
    WobbleVelocity += FVector2D(Local.Y, -Local.X) * 220.f;
    Popups.Add({Where + FVector(FMath::FRandRange(-12.f, 12.f), FMath::FRandRange(-12.f, 12.f), 10.f), Applied, Now});
    UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target hit for %.0f, health %.0f/%.0f"),
        Applied, Health, MaxHealth);
    if (Health <= 0.f)
    {
        bDead = true;
        bPhaselocked = false;
        DiedAt = Now;
        ClearPresentation();
        SetActorEnableCollision(false);
        if (StockMesh && StockDeath) StockMesh->PlayAnimation(StockDeath, false);
        AOpenWillowShotFx::Flash(GetWorld(), AimPoint(), FLinearColor(1.f, 0.55f, 0.2f), 70.f, 0.25f, 4000.f);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target destroyed; respawns in %.0fs"), RespawnSeconds);
    }
    return Applied;
}
