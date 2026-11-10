#include "OpenWillowCombatTarget.h"
#include "OpenWillowShotFx.h"
#include "OpenWillowQuest.h"
#include "OpenWillowWalker.h"
#include "weapon_firing.hpp"
#include "OpenWillowTargetAnimInstance.h"
#include "OpenWillowStatusEffect.h"
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
#include "Materials/MaterialInterface.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

namespace
{
const FLinearColor BodyColor(0.20f, 0.07f, 0.04f);
const FLinearColor HeadColor(0.55f, 0.42f, 0.30f);
// Host calibration (UNVERIFIED): factor between the data's light brightness and UE5's unitless intensity (see UpdatePresentation).
constexpr float LockLightGain = 6.5f;

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
    KnockOrigin = HomeLocation;
    FParse::Value(FCommandLine::Get(), TEXT("owtargetlevel="), Level);
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
    StockMeshBaseOffset = MeshOffset;
    StockMesh->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    StockMesh->bPerBoneMotionBlur = false;   // Borderlands 2 draws no motion blur (the lift clips smeared in UE5)
    StockMesh->SetAnimationMode(EAnimationMode::AnimationBlueprint);
    StockMesh->SetAnimInstanceClass(UOpenWillowTargetAnimInstance::StaticClass());
    StockMesh->RegisterComponent();
    StockIdle = Idle;
    PlayStock(Idle, true);
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

void AOpenWillowCombatTarget::PlayStock(UAnimSequence* Clip, bool bLoop, float Rate, float BlendIn)
{
    // SpecialMove blends (GearboxAnimDefinition.PlayAnim and the special-move node, read not run): no blend when the clip's
    // play time is shorter than the blend (the node drops blends that do not fit; the blend-out of these moves is 0).
    if (!bLoop && Clip && Clip->GetPlayLength() / FMath::Max(Rate, KINDA_SMALL_NUMBER) < BlendIn) BlendIn = 0.f;
    if (auto* Anim = StockMesh ? Cast<UOpenWillowTargetAnimInstance>(StockMesh->GetAnimInstance()) : nullptr)
        Anim->Play(Clip, bLoop, Rate, BlendIn);
}

void AOpenWillowCombatTarget::AttachWornMeshes(const FJsonObject& Pawn)
{
    // The static meshes the stock body composition wears on its bones (spikes on the head, back and arms). The tool writes them
    // only after their import; a file without them leaves the bare body. They add nothing to StockMesh->Bounds, which the bubble's size
    // rule reads. Each sits on its bone with the stock component's own transform. The live Adult's bone matrices fit the imported frames
    // with no flip (one cast, approximate pose). UNVERIFIED: that composition, and the glTF axes through the import.
    const TArray<TSharedPtr<FJsonValue>>* Pieces = nullptr;
    if (!Pawn.TryGetArrayField(TEXT("attachments"), Pieces) || !Pieces) return;
    int32 Attached = 0;
    for (const TSharedPtr<FJsonValue>& Value : *Pieces)
    {
        const TSharedPtr<FJsonObject> A = Value->AsObject();
        const TArray<TSharedPtr<FJsonValue>>* L = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Q = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* S = nullptr;
        FString MeshPath, BoneName;
        if (!A || !A->TryGetStringField(TEXT("mesh"), MeshPath) || !A->TryGetStringField(TEXT("bone"), BoneName)
            || !A->TryGetArrayField(TEXT("location"), L) || L->Num() != 3 || !A->TryGetArrayField(TEXT("quat"), Q) || Q->Num() != 4
            || !A->TryGetArrayField(TEXT("scale"), S) || S->Num() != 3)
        {
            UE_LOG(LogTemp, Warning, TEXT("OpenWillow target: malformed attachment entry skipped"));
            continue;
        }
        UStaticMesh* Piece = LoadObject<UStaticMesh>(nullptr, *MeshPath);
        const FName Bone(*BoneName);
        if (!Piece || StockMesh->GetBoneIndex(Bone) == INDEX_NONE)
        {
            UE_LOG(LogTemp, Warning, TEXT("OpenWillow target: attachment %s on %s skipped (asset or bone missing)"), *MeshPath, *BoneName);
            continue;
        }
        UStaticMeshComponent* Component = NewObject<UStaticMeshComponent>(this);
        Component->SetStaticMesh(Piece);
        Component->SetCollisionEnabled(ECollisionEnabled::NoCollision);
        Component->SetupAttachment(StockMesh, Bone);
        Component->SetRelativeLocationAndRotation(
            FVector((*L)[0]->AsNumber(), (*L)[1]->AsNumber(), (*L)[2]->AsNumber()),
            FQuat((*Q)[0]->AsNumber(), (*Q)[1]->AsNumber(), (*Q)[2]->AsNumber(), (*Q)[3]->AsNumber()).GetNormalized());
        Component->SetRelativeScale3D(FVector((*S)[0]->AsNumber(), (*S)[1]->AsNumber(), (*S)[2]->AsNumber()));
        Component->RegisterComponent();
        ++Attached;
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow target: %d of %d attachments on the stock pawn"), Attached, Pieces->Num());
}

bool AOpenWillowCombatTarget::UseCommandLinePawn()
{
    FString Id;
    if (!FParse::Value(FCommandLine::Get(), TEXT("owcombatpawn="), Id) || Id.IsEmpty()) return false;
    const FString File = FPaths::ConvertRelativePathToFull(
        FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/phaselock/target/host_target.json")));
    FString Text;
    TSharedPtr<FJsonObject> Root;
    const TSharedPtr<FJsonObject>* Entry = nullptr;
    if (!FFileHelper::LoadFileToString(Text, *File) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root)
        || !Root || !Root->TryGetObjectField(Id, Entry))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow target: no '%s' in %s (run tools/import_phaselock_target.ps1)"), *Id, *File);
        return false;
    }
    const TSharedPtr<FJsonObject>& P = *Entry;
    // Mesh offset: UE3 puts the mesh at the pawn's Location (the cylinder centre) plus the mesh component's Translation;
    // this actor's origin is the floor, so the tool wrote CollisionHeight + Translation.Z.
    if (!UseStockPawn(P->GetStringField(TEXT("mesh")), P->GetStringField(TEXT("idle")), P->GetStringField(TEXT("death")),
            FVector(0, 0, P->GetNumberField(TEXT("mesh_offset_z")))))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow target: assets of '%s' did not load"), *Id);
        return false;
    }
    StockMesh->SetRelativeScale3D(FVector(P->GetNumberField(TEXT("mesh_scale"))));
    AttachWornMeshes(*P);
    const UMaterialInterface* SkinMaterial = StockMesh->GetMaterial(0);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow target: stock pawn %s skin material %s"), *Id,
        SkinMaterial ? *SkinMaterial->GetPathName() : TEXT("<none>"));
    // The stock pawn's collision cylinder replaces the bounds-sized capsule: its centre and half height are what the
    // lift trace uses (CollisionCentre). A UE capsule cannot be flatter than a sphere, so a cylinder wider than it is
    // tall (the adult bullymong's 150 x 90) keeps the half height and gets the half height as its radius (host choice;
    // the auto-aim radius is then smaller than the game's).
    const float Radius = P->GetNumberField(TEXT("collision_radius"));
    const float HalfHeight = P->GetNumberField(TEXT("collision_height"));
    HitVolume->SetCapsuleSize(FMath::Min(Radius, HalfHeight), HalfHeight);
    HitVolume->SetRelativeLocation(FVector(0, 0, HalfHeight));
    // The UE3 pawn's Location is its cylinder centre; the bones' box is centred centre_bias below it (observed fit, see
    // tools/prepare_phaselock_target.py), standing in for the stock physics asset's bounds.
    PawnLocationAbovePivot = HalfHeight - float(P->GetNumberField(TEXT("centre_bias")));
    UE_LOG(LogTemp, Display, TEXT("OpenWillow target: stock pawn %s, mesh offset %.1f, scale %.2f, cylinder %.0f x %.0f"),
        *Id, StockMeshBaseOffset.Z, StockMesh->GetRelativeScale3D().X, Radius, HalfHeight);
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
    // Test aid for matched comparisons: -owbubbleradius=<intro>,<loop>,<end> (uu) stands in for the lifted pawn's mesh
    // bounds sphere radius at each bubble template's spawn. The game's pose changes the bounds: the SDK probe of the adult
    // bullymong read 290 at the lock, a loop emitter scale equivalent to 260 0.2 s later, and 193 in the lifted pose that the
    // end template is spawned in (size_rule/adult68). The engine-shape dummy's own radius is about 109 uu and constant.
    static TArray<float> Override;
    static bool bRead = false;
    if (!bRead)
    {
        bRead = true;
        FString Text;
        if (FParse::Value(FCommandLine::Get(), TEXT("owbubbleradius="), Text, false))
        {
            TArray<FString> Parts;
            Text.ParseIntoArray(Parts, TEXT(","));
            for (const FString& Part : Parts) Override.Add(FCString::Atof(*Part));
        }
    }
    if (Override.Num() > 0) return Override[FMath::Clamp(BubbleStageNow, 0, Override.Num() - 1)] ;
    // Round 20 host reading (UNVERIFIED): use the live component scale of 1.0, so do not undo component scale here.
    // The raw host radii are 307 / 245 / 200 uu at the three spawns versus live-game reads 290 / 230-260 / 193 uu.
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
    return LockLight ? LockLightData : 0.f;   // in the data's brightness units, whatever gain the host applies
}

float AOpenWillowCombatTarget::BubbleDrawScale() const
{
    // Confirmed in game on 2026-10-03 (SDK reads of the bubble emitters' DrawScale and frames, three bullymong variants):
    // each bubble emitter takes DrawScale = the lifted pawn's Mesh.Bounds.SphereRadius at its own spawn / BubbleFXScale,
    // and the loop's visible rim (the blue-minus-red ridge on the frames) sits 48.6 uu out per DrawScale unit. Measured
    // with the same detector on host frames, the host draws that ridge at 0.88 of the Sphere emitter's half-width. Round 6
    // used the bubble texture's own ring (0.78) instead, which drew the bubble about 13% too large (round 9: the ridge detector
    // finds the rim's outer glow, at 0.88 with the blue tint, not the texture ring), so the sprite is drawn at
    // 48.6 / (0.5 x 0.88 x StartSize) of the draw scale, about 0.56 (cause of the factor not found; UNVERIFIED). All three bubble
    // templates use it, since the streaks' measured thickness in game frames matches the same factor.
    constexpr float GameRimPerDrawScale = 48.6f, TextureRim = 0.88f;
    float Calibration = 1.f;
    FString Error;
    if (const FOwFxTemplate* Loop = FOwFxTemplate::Load(Fx.EmitterDir, Fx.BubbleLoop, Error))
        for (const FOwFxEmitter& E : Loop->Emitters)
            if (E.Material == TEXT("Mat_SirenEnemyOrb") && E.StartSize.Kind == FOwFxDistribution::EKind::Constant && E.StartSize.Low.X > 0.0)
                Calibration = GameRimPerDrawScale / (0.5f * TextureRim * float(E.StartSize.Low.X));
    return MeshBoundsRadius() / Fx.BubbleScaleDivisor * Calibration;
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
    BubbleScale = BubbleDrawScale();
    C->Play(Template, BubbleScale);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock bubble %s at %.2f s, draw scale %.3f calibrated (stock %.1f / %.1f = %.3f, game rim %.1f uu), %d emitters skipped"),
        *TemplateName, Now - LockStartedAt, BubbleScale, MeshBoundsRadius(), Fx.BubbleScaleDivisor, MeshBoundsRadius() / Fx.BubbleScaleDivisor,
        48.6f * MeshBoundsRadius() / Fx.BubbleScaleDivisor, C->SkippedEmitters());
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
        // Host calibration (UNVERIFIED): the data's brightness times LockLightGain is UE5's unitless intensity; at 1x the pool under the
        // target is invisible on dark asphalt, where the game frames show a pale blue pool on snow and sand.
        LockLightData = Fx.LightBrightness * FMath::Clamp(Fraction, 0.f, 1.f);
        LockLight->SetIntensity(LockLightGain * LockLightData);
    }
    if (Held >= T.LockedAt && BubbleStageNow == 0)
    {
        // LockTarget -> SpawnBubbleFX: the intro template, scaled from the pawn's mesh bounds at this moment.
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
            // The loop takes its own draw scale from the mesh bounds at its spawn (confirmed in game, see BubbleDrawScale).
            BubbleScale = BubbleDrawScale();
            BubbleLoop->Play(Template, BubbleScale);
            UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock bubble loop at %.2f s: life %.2f s, collapse from %.2f s, draw scale %.3f calibrated (stock %.3f, game rim %.1f uu)"),
                Held, StateDuration + Fx.BubbleOutroOverlap, CollapseStartAt - LockStartedAt, BubbleScale, MeshBoundsRadius() / Fx.BubbleScaleDivisor,
                48.6f * MeshBoundsRadius() / Fx.BubbleScaleDivisor);
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
    LockLightData = 0.f;
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

void AOpenWillowCombatTarget::ClearSwing()
{
    Wobble = FVector2D::ZeroVector;
    WobbleVelocity = FVector2D::ZeroVector;
    Pivot->SetRelativeRotation(FRotator(0.f, Pivot->GetRelativeRotation().Yaw, 0.f));
}

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
        // Host calibration (UNVERIFIED): a softer edge than the data's exponent gives (the pool is a hard-edged disc on dark asphalt;
        // the game's is pale and soft), and a colour pulled 20% toward white.
        LockLight->SetLightFalloffExponent(FMath::Max(Fx.LightFalloffExponent, 1.75f));
        LockLight->SetAttenuationRadius(0.85f * Fx.LightRadius);   // host calibration (UNVERIFIED): 0.85x the data radius, so the pool is about 300 px wide as in the game
        LockLight->SetLightColor(FMath::Lerp(FLinearColor(Fx.LightColor), FLinearColor::White, 0.2f));
        LockLight->SetCastShadows(Fx.bLightShadows);
        LockLight->SetIndirectLightingIntensity(0.f);
        LockLight->SetIntensity(0.f);
        // The light reaches the floor, as its data says (LAC_DYNAMIC_AND_STATIC_AFFECTING): the 2026-10-03 matched-distance
        // and size-rule game captures show a pale blue pool under every lifted bullymong. (Rounds 1-4 limited it to the
        // target after the 2026-10-02 captures, where the pool was not noticed on snow.)
        LockLight->RegisterComponent();
    }
    if (StockMesh && LiftClip)
    {
        PlayStock(LiftClip, false, 1.f, LiftBlendIn);
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
        // The stock dummy keeps its death pose until the sequence destroys it; with respawn enabled it ragdolls and respawns.
        // A status effect can be applied after the lethal hit (the shooter rolls it once TakeDamage returns); a range target
        // (respawn enabled) clears it while dead so it does not tick on into the next life.
        if (bRespawnStock && UOpenWillowStatusEffects::AnyActive(this)) UOpenWillowStatusEffects::ClearAll(this);
        if (StockMesh)
        {
            if (bRespawnStock) TickStockRespawn(Now);
            return;
        }
        const float Since = Now - DiedAt;
        Pivot->SetRelativeScale3D(FVector(FMath::Max(0.01f, 1.f - Since / 0.25f)));
        if (Since < RespawnDelay) return;
        bDead = false;
        Health = MaxHealth;
        // A respawned target starts at rest: no knock and no last-hit momentum carried over from the previous life.
        Knock = FVector::ZeroVector;
        LastMomentum = FVector::ZeroVector;
        LastVelocityChange = FVector::ZeroVector;
        SetActorLocation(HomeLocation);
        KnockOrigin = HomeLocation;
        Pivot->SetRelativeScale3D(FVector(1.f));
        Pivot->SetRelativeLocation(FVector::ZeroVector);
        ClearSwing();
        SetActorHiddenInGame(false);
        SetActorEnableCollision(true);
        OnRespawned();
        return;
    }
    // Momentum from hits: slide by the knock velocity on the ground plane, slowed by the friction stand-in.
    if (!Knock.IsNearlyZero(0.01f))
    {
        if (!bPhaselocked) AddActorWorldOffset(FVector(Knock.X, Knock.Y, 0.f) * DeltaSeconds, true);
        Knock *= FMath::Max(0.f, 1.f - KnockFriction * DeltaSeconds);
    }
    else Knock = FVector::ZeroVector;
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
        if (AnimStage == 1 && Now >= AnimClipEndsAt && StockMesh && LoopClip) { PlayStock(LoopClip, true, 1.f, MoveBlendIn); AnimStage = 2; }
        // UpdateLiftedPawnMeshOffset (script, read not run; NATIVE_PHASELOCK_PRESENTATION.md): after the snap part of the
        // lift the mesh is moved in Z so that its bounds centre sits on the pawn's location, eased by VInterpTo speed 5.
        if (StockMesh && Held > Lock.LiftDuration * Lock.SnapTimePct)
        {
            // The script centres Mesh.Bounds, which UE3 takes from the stock physics asset. The host's mesh has UE5's
            // auto-generated physics asset, whose bounds sat 60-80 uu below the visible bullymong (2026-10-21 captures), so
            // the host centres the box around the bones instead (host stand-in, UNVERIFIED). The root bone is left out: it
            // carries no skin and stays at the mesh origin (the feet) while the lifted pose curls up.
            FBox Bones(ForceInit);
            for (int32 Bone = 0; Bone < StockMesh->GetNumBones(); ++Bone)
            {
                const FName Name = StockMesh->GetBoneName(Bone);
                if (!StockMesh->GetParentBone(Name).IsNone()) Bones += StockMesh->GetBoneLocation(Name);
            }
            const float CentreZ = Bones.IsValid ? float(Bones.GetCenter().Z) : float(StockMesh->Bounds.Origin.Z);
            const float Wanted = Pivot->GetComponentLocation().Z + PawnLocationAbovePivot - (CentreZ - MeshExtraZ);
            MeshExtraZ += (Wanted - MeshExtraZ) * FMath::Clamp(5.f * DeltaSeconds, 0.f, 1.f);
            StockMesh->SetRelativeLocation(StockMeshBaseOffset + FVector(0, 0, MeshExtraZ));
            if (FMath::FloorToInt(Held * 2.f) != FMath::FloorToInt((Held - DeltaSeconds) * 2.f))
                UE_LOG(LogTemp, Display, TEXT("OpenWillow lifted mesh at %.2f s: pivot %.1f, pawn location %.1f, bone box centre %.1f, bounds centre %.1f (r %.1f), extra %.1f, bubble %.1f"),
                    Held, Pivot->GetComponentLocation().Z, Pivot->GetComponentLocation().Z + PawnLocationAbovePivot,
                    CentreZ, StockMesh->Bounds.Origin.Z, StockMesh->Bounds.SphereRadius, MeshExtraZ,
                    BubbleLoop ? BubbleLoop->GetComponentLocation().Z : (BubbleIntro ? BubbleIntro->GetComponentLocation().Z : 0.f));
        }
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
                PlayStock(FallClip, false, FallClip->GetPlayLength() / FMath::Max(Lock.DropTime, KINDA_SMALL_NUMBER), MoveBlendIn);
                AnimStage = 3;
            }
            else if (StockMesh && AnimStage != 0) { PlayStock(StockIdle, true); AnimStage = 0; }
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
    if (StockMesh && !bPhaselocked && !FMath::IsNearlyZero(MeshExtraZ, 0.05f))
    {
        MeshExtraZ += (0.f - MeshExtraZ) * FMath::Clamp(5.f * DeltaSeconds, 0.f, 1.f);
        StockMesh->SetRelativeLocation(StockMeshBaseOffset + FVector(0, 0, MeshExtraZ));
    }
    // CheckLandTarget: the land clip once the pawn is down again, then idle.
    if (AnimStage == 3 && Height <= 0.f && StockMesh)
    {
        if (LandClip) { PlayStock(LandClip, false, 1.f, MoveBlendIn); AnimClipEndsAt = Now + LandClip->GetPlayLength(); AnimStage = 4; }
        else { PlayStock(StockIdle, true); AnimStage = 0; }
    }
    if (AnimStage == 4 && Now >= AnimClipEndsAt && StockMesh) { PlayStock(StockIdle, true); AnimStage = 0; }
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
            Quest->OnDummyDamaged(this, Shooter->ShotDamageType(), DamageEvent.IsOfType(FOpenWillowDamageEvent::ClassID)
                && static_cast<const FOpenWillowDamageEvent&>(DamageEvent).Hit.Source == EOpenWillowHitSource::Status);
    const float Now = GetWorld()->GetTimeSeconds();
    bool bCritical = false;
    const float Dealt = AdjustIncomingDamage(DamageAmount, DamageEvent, bCritical);
    // Invulnerable (range toggle): the hit is recorded, popup included, but health is not lowered.
    const float Applied = bInvulnerable ? FMath::Max(0.f, Dealt) : FMath::Clamp(Dealt, 0.f, Health);
    if (!bInvulnerable) Health -= Applied;
    // The hit event's result fields (OpenWillowHitEvent.h): the target reports what it applied and whether it was critical,
    // so OnHit listeners see the same result as the damage number.
    if (DamageEvent.IsOfType(FOpenWillowDamageEvent::ClassID))
    {
        const FOpenWillowHitEvent& HitEvent = static_cast<const FOpenWillowDamageEvent&>(DamageEvent).Hit;
        HitEvent.bCritical = bCritical;
        HitEvent.Applied = Applied;
    }
    LastHitAt = Now;
    FVector Where = AimPoint();
    FVector Direction = FVector::ZeroVector;
    if (DamageEvent.IsOfType(FPointDamageEvent::ClassID))
    {
        const auto& Point = static_cast<const FPointDamageEvent&>(DamageEvent);
        Where = Point.HitInfo.ImpactPoint;
        Direction = Point.ShotDirection;
    }
    if (const auto* Shooter = Cast<AOpenWillowWalker>(DamageCauser); Shooter && !Shooter->ShotMomentum().IsNearlyZero())
    {
        LastMomentum = Shooter->ShotMomentum();
        LastVelocityChange = bPhaselocked ? FVector::ZeroVector : MomentumVelocity(LastMomentum, Mass);
        Knock += LastVelocityChange;
        UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target momentum %.1f -> velocity change %.3f uu/s (mass %.1f)"),
            LastMomentum.Size(), LastVelocityChange.Size(), Mass);
    }
    const FVector Local = GetActorTransform().InverseTransformVectorNoScale(Direction);
    WobbleVelocity += FVector2D(Local.Y, -Local.X) * 220.f;
    Popups.Add({Where + FVector(FMath::FRandRange(-12.f, 12.f), FMath::FRandRange(-12.f, 12.f), 10.f), Applied, Now, bCritical});
    UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target hit for %.0f, health %.0f/%.0f"),
        Applied, Health, MaxHealth);
    if (Health <= 0.f)
    {
        bDead = true;
        bPhaselocked = false;
        // A dead target does not slide. The last-hit record (LastMomentum, LastVelocityChange) stays until respawn: the
        // shooter reads it right after this hit to count the push.
        Knock = FVector::ZeroVector;
        DiedAt = Now;
        ClearPresentation();
        SetActorEnableCollision(false);
        if (StockMesh && StockDeath) PlayStock(StockDeath, false);
        AOpenWillowShotFx::Flash(GetWorld(), AimPoint(), FLinearColor(1.f, 0.55f, 0.2f), 70.f, 0.25f, 4000.f);
        // Engine-shape targets always respawn after RespawnDelay; a stock pawn only when respawn is enabled (the range).
        if (!StockMesh || bRespawnStock)
        {
            UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target destroyed; respawns in %.0fs"), RespawnDelay);
        }
        else
        {
            UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target destroyed; no respawn"));
        }
        // A kill by Maya: while she is down it is her second wind.
        if (auto* Killer = Cast<AOpenWillowWalker>(DamageCauser)) Killer->NotifyKilledEnemy();
        OnDied(DamageCauser);
    }
    return Applied;
}

float AOpenWillowCombatTarget::AdjustIncomingDamage(float Amount, const FDamageEvent& DamageEvent, bool& bOutCritical)
{
    // Base target: no critical hits, the damage passes through unchanged.
    bOutCritical = false;
    return Amount;
}

void AOpenWillowCombatTarget::SetRespawn(bool bEnabled, float DelaySeconds)
{
    bRespawnStock = bEnabled;
    if (DelaySeconds > 0.f) RespawnDelay = DelaySeconds;
}

void AOpenWillowCombatTarget::TickStockRespawn(float Now)
{
    // Host stand-in (UNVERIFIED): the death clip plays; after RagdollDelay the mesh goes limp (physics only when the imported
    // mesh has a physics asset; the log says which); after RespawnDelay the mesh is put back on its pose and the target stands again.
    const float Since = Now - DiedAt;
    if (bRagdollSimulating) TrackCorpse();
    if (!bRagdoll && Since >= RagdollDelay)
    {
        bRagdoll = true;
        // The death turned the actor's collision off; the ragdoll needs it back to land on the floor, while the hit volume
        // stays off (a corpse takes no shots: TakeDamage returns at once while dead).
        SetActorEnableCollision(true);
        if (HitVolume) HitVolume->SetCollisionEnabled(ECollisionEnabled::NoCollision);
        StockMesh->SetCollisionProfileName(TEXT("Ragdoll"));
        const bool bPhysics = StockMesh->GetPhysicsAsset() != nullptr;
        if (bPhysics)
        {
            // Nothing throws a corpse on purpose: the death zeroed the knock, and no velocity is handed to the ragdoll. What can is the
            // moment physics takes over, with bodies overlapping each other or the floor (the suspect for the flung corpses of the first
            // playtest, UNVERIFIED): so that push is limited, and TrackCorpse caps what still gets through and logs it.
            for (FBodyInstance* Body : StockMesh->Bodies)
                if (Body) Body->SetMaxDepenetrationVelocity(RagdollDepenetrationSpeed);
            StockMesh->SetSimulatePhysics(true);
            StockMesh->SetAllPhysicsLinearVelocity(FVector::ZeroVector);
            StockMesh->SetAllPhysicsAngularVelocityInDegrees(FVector::ZeroVector);
            bRagdollSimulating = true;
            bCorpseMeasured = true;
            CorpseFrom = StockMesh->Bounds.Origin;
            CorpseRise = CorpseDrift = CorpseSpeed = 0.f;
            CorpseCaps = 0;
        }
        UE_LOG(LogTemp, Display, TEXT("OpenWillow target %s ragdoll at %.2f s: physics asset %s"), *GetName(), Since,
            bPhysics ? TEXT("yes, simulating") : TEXT("none, not simulated"));
    }
    if (Since < RespawnDelay) return;
    if (bRagdoll) StockMesh->SetSimulatePhysics(false);
    if (bRagdollSimulating)
    {
        bRagdollSimulating = false;
        UE_LOG(LogTemp, Display, TEXT("OpenWillow target %s corpse rose %.0f uu, drifted %.0f uu, fastest body %.0f uu/s, %d speed caps"), *GetName(),
            CorpseRise, CorpseDrift, CorpseSpeed, CorpseCaps);
    }
    // The mesh goes back under the pivot at its rest offset; the host never sets its rotation, so zero is its rest rotation.
    StockMesh->AttachToComponent(Pivot, FAttachmentTransformRules::KeepRelativeTransform);
    StockMesh->SetRelativeLocation(StockMeshBaseOffset);
    StockMesh->SetRelativeRotation(FRotator::ZeroRotator);
    MeshExtraZ = 0.f;
    Pivot->SetRelativeLocation(FVector::ZeroVector);
    ClearSwing();
    bRagdoll = false;
    bDead = false;
    Health = MaxHealth;
    // A respawned target starts at rest: no knock and no last-hit momentum carried over from the previous life.
    Knock = FVector::ZeroVector;
    LastMomentum = FVector::ZeroVector;
    LastVelocityChange = FVector::ZeroVector;
    SetActorLocation(HomeLocation);
    KnockOrigin = HomeLocation;
    SetActorHiddenInGame(false);
    SetActorEnableCollision(true);
    StockMesh->SetCollisionEnabled(ECollisionEnabled::NoCollision);   // the hit volume is the target's collision, as before the death
    if (HitVolume) HitVolume->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
    AnimStage = 0;
    PlayStock(StockIdle, true);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow target %s respawned after %.2f s at %s"), *GetName(), Since, *HomeLocation.ToString());
    OnRespawned();
}

void AOpenWillowCombatTarget::TrackCorpse()
{
    const FVector At = StockMesh->Bounds.Origin;
    CorpseRise = FMath::Max(CorpseRise, float(At.Z - CorpseFrom.Z));
    CorpseDrift = FMath::Max(CorpseDrift, float(FVector::Dist2D(At, CorpseFrom)));
    for (FBodyInstance* Body : StockMesh->Bodies)
    {
        // Through the component: a kinematic body keeps its velocity, and FBodyInstance's own check needs the PhysicsCore module linked.
        if (!Body || !StockMesh->IsSimulatingPhysics(StockMesh->GetBoneName(Body->InstanceBoneIndex))) continue;
        FVector Velocity = Body->GetUnrealWorldVelocity();
        CorpseSpeed = FMath::Max(CorpseSpeed, float(Velocity.Size()));
        const FVector2D Flat(Velocity.X, Velocity.Y);
        const double FlatSpeed = Flat.Size();
        if (Velocity.Z <= RagdollMaxRiseSpeed && FlatSpeed <= RagdollMaxFlatSpeed) continue;
        if (FlatSpeed > RagdollMaxFlatSpeed)
        {
            Velocity.X *= RagdollMaxFlatSpeed / FlatSpeed;
            Velocity.Y *= RagdollMaxFlatSpeed / FlatSpeed;
        }
        Velocity.Z = FMath::Min(Velocity.Z, double(RagdollMaxRiseSpeed));
        Body->SetLinearVelocity(Velocity, false);
        ++CorpseCaps;
    }
}

bool AOpenWillowCombatTarget::KickCorpseForTest(float UpSpeed)
{
    if (!bRagdollSimulating) return false;
    for (FBodyInstance* Body : StockMesh->Bodies)
    {
        if (!Body) continue;
        const FName Bone = StockMesh->GetBoneName(Body->InstanceBoneIndex);
        if (!StockMesh->IsSimulatingPhysics(Bone)) continue;
        Body->SetLinearVelocity(FVector(0.f, 0.f, UpSpeed), false);
        CorpseKickBone = Bone;
        return true;
    }
    return false;
}

float AOpenWillowCombatTarget::CorpseKickedRiseSpeed() const
{
    return CorpseKickBone.IsNone() || !bRagdollSimulating ? 0.f : float(StockMesh->GetPhysicsLinearVelocity(CorpseKickBone).Z);
}

FVector AOpenWillowCombatTarget::MomentumVelocity(const FVector& Momentum, float Mass)
{
    const weapon_firing::Vec V = weapon_firing::momentumVelocity({Momentum.X, Momentum.Y, Momentum.Z}, Mass);
    return FVector(V.x, V.y, V.z);
}
