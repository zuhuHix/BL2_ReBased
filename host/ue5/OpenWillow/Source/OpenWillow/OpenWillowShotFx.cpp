#include "OpenWillowShotFx.h"
#include "Components/PointLightComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "Camera/PlayerCameraManager.h"
#include "GameFramework/PlayerController.h"
#include "Materials/MaterialInstanceDynamic.h"

namespace
{
const TCHAR* FxMaterialPath = TEXT("/Game/OpenWillow/Weapons/InfinityProxy/M_OW_FxAdditive.M_OW_FxAdditive");
const TCHAR* FlameMaterialPath = TEXT("/Game/OpenWillow/Weapons/InfinityProxy/M_OW_MuzzleFlame.M_OW_MuzzleFlame");
}

namespace
{
// Cosmetic budget: shot effects spawned per frame (flashes, sparks, tracers, flash quads, casings). A multi-pellet or fast
// gun can ask for hundreds in one frame; past the cap the extra cosmetic actors are skipped. Damage never goes through here.
// Host value, not a game rule.
constexpr int32 MaxShotFxPerFrame = 96;
uint64 BudgetFrame = 0;
int32 BudgetUsed = 0;
// Decals are components, not effect actors: a separate, smaller cap per frame.
constexpr int32 MaxDecalsPerFrame = 16;
uint64 DecalFrame = 0;
int32 DecalsUsed = 0;
}

bool AOpenWillowShotFx::TakeDecalBudget()
{
    if (DecalFrame != GFrameCounter) { DecalFrame = GFrameCounter; DecalsUsed = 0; }
    if (DecalsUsed >= MaxDecalsPerFrame) return false;
    ++DecalsUsed;
    return true;
}

AOpenWillowShotFx* AOpenWillowShotFx::SpawnBudgeted(UWorld* World, const FTransform& Pose)
{
    if (!World) return nullptr;
    if (BudgetFrame != GFrameCounter) { BudgetFrame = GFrameCounter; BudgetUsed = 0; }
    if (BudgetUsed >= MaxShotFxPerFrame) return nullptr;
    AOpenWillowShotFx* Fx = World->SpawnActorDeferred<AOpenWillowShotFx>(StaticClass(), Pose);
    if (!Fx) return nullptr;
    ++BudgetUsed;
    Fx->FinishSpawning(Pose);
    return Fx;
}

AOpenWillowShotFx::AOpenWillowShotFx()
{
    PrimaryActorTick.bCanEverTick = true;
    Visual = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("FxVisual"));
    RootComponent = Visual;
    Visual->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Visual->SetCastShadow(false);
    Light = CreateDefaultSubobject<UPointLightComponent>(TEXT("FxLight"));
    Light->SetupAttachment(Visual);
    Light->SetCastShadows(false);
    Light->SetIntensityUnits(ELightUnits::Candelas);
    Light->SetIntensity(0.f);
}

void AOpenWillowShotFx::Setup(UStaticMesh* Mesh, const FLinearColor& Color, float Intensity,
    float Life, float LightIntensity, bool bSoftFlame, float CoreStrength)
{
    Visual->SetStaticMesh(Mesh);
    UMaterialInterface* Base = LoadObject<UMaterialInterface>(nullptr, bSoftFlame ? FlameMaterialPath : FxMaterialPath);
    if (!Base && bSoftFlame) Base = LoadObject<UMaterialInterface>(nullptr, FxMaterialPath);
    if (Base)
    {
        Material = UMaterialInstanceDynamic::Create(Base, this);
        Material->SetVectorParameterValue(TEXT("Color"), Color);
        Material->SetScalarParameterValue(TEXT("Intensity"), Intensity);
        if (bSoftFlame)
        {
            Material->SetVectorParameterValue(TEXT("CoreColor"), FLinearColor(1.f, 0.68f, 0.12f, 1.f));
            Material->SetScalarParameterValue(TEXT("CoreStrength"), CoreStrength);
        }
        Visual->SetMaterial(0, Material);
    }
    Light->SetLightColor(Color);
    Light->SetIntensity(LightIntensity);
    Light->SetAttenuationRadius(LightIntensity > 0.f ? 220.f : 0.f);
    Born = GetWorld()->GetTimeSeconds();
    Lifetime = Life;
    StartIntensity = Intensity;
    StartLight = LightIntensity;
    SetLifeSpan(Life + 0.05f);
}

void AOpenWillowShotFx::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    if (bIsCasing)
    {
        // UNVERIFIED host stand-in: gravity and spin keep the brass visible without a physics body per shot.
        CasingVelocity.Z -= CasingGravity * DeltaSeconds;
        AddActorWorldOffset(CasingVelocity * DeltaSeconds, false);
        AddActorLocalRotation(FRotator(540.f * DeltaSeconds, 1080.f * DeltaSeconds, 720.f * DeltaSeconds));
    }
    const float Fade = 1.f - FMath::Clamp((GetWorld()->GetTimeSeconds() - Born) / Lifetime, 0.f, 1.f);
    if (Material) Material->SetScalarParameterValue(TEXT("Intensity"), StartIntensity * Fade);
    Light->SetIntensity(StartLight * Fade);
}

void AOpenWillowShotFx::Tracer(UWorld* World, const FVector& Start, const FVector& End,
    const FLinearColor& Color, float Radius, float Life)
{
    const FVector Span = End - Start;
    const float Length = Span.Size();
    if (!World || Length < 1.f) return;
    // The engine cylinder is 100 cm tall along Z with a 50 cm radius.
    const FTransform Pose(FRotationMatrix::MakeFromZ(Span / Length).Rotator(),
        Start + Span * 0.5f, FVector(Radius / 50.f, Radius / 50.f, Length / 100.f));
    AOpenWillowShotFx* Fx = SpawnBudgeted(World, Pose);
    if (!Fx) return;
    Fx->Setup(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cylinder.Cylinder")),
        Color, 6.f, Life, 0.f);
}

void AOpenWillowShotFx::Flash(UWorld* World, const FVector& Location, const FLinearColor& Color,
    float Radius, float Life, float LightIntensity, float Intensity)
{
    if (!World) return;
    const FTransform Pose(FRotator(FMath::FRandRange(0.f, 360.f), FMath::FRandRange(0.f, 360.f), 0.f),
        Location, FVector(Radius / 50.f));
    AOpenWillowShotFx* Fx = SpawnBudgeted(World, Pose);
    if (!Fx) return;
    Fx->Setup(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Sphere.Sphere")),
        Color, Intensity, Life, LightIntensity);
}

void AOpenWillowShotFx::MuzzleFlash(UWorld* World, const FVector& Location, float Radius, float Life,
    const FLinearColor& Color)
{
    if (!World) return;
    const float SafeRadius = FMath::Max(0.1f, Radius);
    const float SafeLife = FMath::Max(0.01f, Life);
    // Host-made, soft additive quads. The selected muzzle socket anchors every layer; the mask shape,
    // size, tint and short render life are UNVERIFIED visual fits to the lane E burst references.
    const float PlumeLife = FMath::Min(SafeLife, 0.12f);
    const float Extent = FMath::Clamp(SafeRadius * 1.25f, 4.5f, 6.f);
    UStaticMesh* Plane = LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Plane.Plane"));
    if (!Plane) return;
    FRotator CameraRotation = FRotator::ZeroRotator;
    if (APlayerController* PC = World->GetFirstPlayerController())
        if (PC->PlayerCameraManager) CameraRotation = PC->PlayerCameraManager->GetCameraRotation();
    const FRotationMatrix CameraBasis(CameraRotation);
    const FVector ViewForward = CameraBasis.GetUnitAxis(EAxis::X);
    const FQuat Facing = FQuat::FindBetweenNormals(FVector::UpVector, -ViewForward);
    for (int32 I = 0; I < 6; ++I)
    {
        const float Height = Extent * FMath::FRandRange(1.1f, 1.75f);
        const float Width = Extent * FMath::FRandRange(0.55f, 0.95f);
        const float Roll = FMath::FRandRange(0.f, 360.f);
        const FQuat Twist(-ViewForward, FMath::DegreesToRadians(Roll));
        const FTransform Pose(Twist * Facing, Location,
            FVector(Width / 100.f, Height / 100.f, 1.f));
        AOpenWillowShotFx* Fx = SpawnBudgeted(World, Pose);
        if (!Fx) continue;
        Fx->Setup(Plane, Color, 1.3f, PlumeLife, 0.f, true, 0.f);
        Fx->Visual->SetFirstPersonPrimitiveType(EFirstPersonPrimitiveType::FirstPerson);
    }
    const float CoreSize = Extent * 0.44f;
    const FTransform CorePose(Facing, Location, FVector(CoreSize / 100.f, CoreSize / 100.f, 1.f));
    AOpenWillowShotFx* Core = SpawnBudgeted(World, CorePose);
    if (!Core) return;
    Core->Setup(Plane, Color, 2.5f, PlumeLife, 90.f, true, 1.f);
    Core->Visual->SetFirstPersonPrimitiveType(EFirstPersonPrimitiveType::FirstPerson);
}

void AOpenWillowShotFx::Spark(UWorld* World, const FVector& Location, const FVector& Velocity, const FLinearColor& Color,
    float Radius, float Life, float Intensity)
{
    if (!World) return;
    const FTransform Pose(FRotator::ZeroRotator, Location, FVector(FMath::Max(0.01f, Radius) / 50.f));
    AOpenWillowShotFx* Fx = SpawnBudgeted(World, Pose);
    if (!Fx) return;
    Fx->Setup(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Sphere.Sphere")), Color, Intensity, Life, 0.f);
    Fx->bIsCasing = true;   // the casing motion: velocity with gravity
    Fx->CasingVelocity = Velocity;
}

void AOpenWillowShotFx::MuzzleFlashLayers(UWorld* World, const FVector& Location, const FVector& Forward, const FVector& Up,
    const TArray<FOpenWillowFlashLayer>& FlashLayers, float TextureFill)
{
    if (!World || FlashLayers.IsEmpty()) return;
    UStaticMesh* Plane = LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Plane.Plane"));
    if (!Plane) return;
    const FVector X = Forward.GetSafeNormal();
    const FVector Side = FVector::CrossProduct(Up, X).GetSafeNormal();  // the barrel's sideways axis
    const FVector Top = FVector::CrossProduct(X, Side).GetSafeNormal();
    FVector ViewDir = X;
    if (APlayerController* PC = World->GetFirstPlayerController())
        if (PC->PlayerCameraManager) ViewDir = (Location - PC->PlayerCameraManager->GetCameraLocation()).GetSafeNormal();
    for (const FOpenWillowFlashLayer& L : FlashLayers)
    {
        const float Life = FMath::Max(0.01f, FMath::FRandRange(L.LifeMin, L.LifeMax));
        // A rectangle's flame starts at the muzzle: its offset along the barrel shrinks with its length. Its visible
        // streak is narrower than the quad (critic round 2: rectangles about 60 percent of the first fit's width).
        const float Length = FMath::FRandRange(L.LengthMin, L.LengthMax) * TextureFill;
        // Square additive sprites facing along the barrel (the assault rifle's front flash) draw a small star in the game;
        // with the host's soft quad they read as a block, so they keep 30 percent of the fill (host fit, critic round 2).
        const float SquareFill = !L.bSmoke && !L.bModulate ? 0.3f : 1.f;
        const float Width = L.Width > 0.f ? L.Width * TextureFill * 0.6f
            : FMath::FRandRange(L.LengthMin, L.LengthMax) * TextureFill * SquareFill;
        // The engine plane is 100 x 100 cm in its local X/Y with the normal on Z.
        FVector Normal = X, PlaneX = Side;
        if (L.bSmoke) { Normal = -ViewDir; PlaneX = FVector::CrossProduct(Top, Normal).GetSafeNormal(); }
        else if (L.Axis == TEXT("EPAL_Y")) { Normal = Side; PlaneX = X; }
        else if (L.Axis == TEXT("EPAL_Z")) { Normal = Top; PlaneX = X; }
        if (PlaneX.IsNearlyZero()) PlaneX = Side;
        const FVector PlaneY = FVector::CrossProduct(Normal, PlaneX).GetSafeNormal();
        // Square sprites spin about their normal (the game picks a random sub-image and rotation).
        FQuat Basis = FMatrix(PlaneX, PlaneY, Normal, FVector::ZeroVector).ToQuat();
        if (L.Width <= 0.f && !L.bSmoke) Basis = FQuat(Normal, FMath::FRandRange(0.f, 2.f * PI)) * Basis;
        const bool bRect = L.Width > 0.f && !L.bSmoke;
        const FTransform Pose(Basis, Location + X * L.OffsetX * TextureFill,
            FVector((bRect ? Length : Width) / 100.f, Width / 100.f, 1.f));
        // Brightness: the colour's peak channel times the start alpha; the glow material fades toward edge-on views.
        const float Peak = FMath::Max3(L.Color.R, L.Color.G, L.Color.B);
        float Strength = FMath::FRandRange(L.AlphaMin, L.AlphaMax) * Peak;
        if (L.bAngleFade) Strength *= FMath::Abs(FVector::DotProduct(ViewDir, Normal));
        if (L.bSmoke) Strength = FMath::Min(Strength, 1.f) * 0.12f;  // host stand-in: additive grey for a lit smoke puff
        // The host has only an additive material: a modulate glow is drawn as a faint tint of its colour (host fit).
        if (L.bModulate) Strength *= 0.06f;
        if (Strength < 0.02f) continue;
        const FLinearColor Tint = L.bSmoke ? FLinearColor(0.6f, 0.58f, 0.55f) : L.Color / FMath::Max(Peak, 1e-3f);
        AOpenWillowShotFx* Fx = SpawnBudgeted(World, Pose);
        if (!Fx) continue;
        Fx->Setup(Plane, Tint, Strength, Life, 0.f, true, L.bSmoke ? 0.f : 0.3f);  // a softer white core (critic round 2: too pale)
        Fx->Visual->SetFirstPersonPrimitiveType(EFirstPersonPrimitiveType::FirstPerson);
    }
}

void AOpenWillowShotFx::Casing(UWorld* World, const FVector& Location, const FVector& Velocity, float Scale, float Life,
    float Gravity)
{
    if (!World) return;
    const FVector SafeScale(FMath::Max(0.001f, 0.003f * Scale), FMath::Max(0.001f, 0.003f * Scale),
        FMath::Max(0.001f, 0.012f * Scale));
    // UNVERIFIED host stand-ins: short cylinder, orientation, subdued brass tint, intensity and lifetime.
    const FTransform Pose(FRotator(90.f, 0.f, 0.f), Location, SafeScale);
    AOpenWillowShotFx* Fx = SpawnBudgeted(World, Pose);
    if (!Fx) return;
    Fx->Setup(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cylinder.Cylinder")),
        FLinearColor(0.72f, 0.34f, 0.055f, 1.f), 0.55f, FMath::Max(0.05f, Life), 0.f);
    Fx->Visual->SetFirstPersonPrimitiveType(EFirstPersonPrimitiveType::FirstPerson);
    Fx->bIsCasing = true;
    Fx->CasingGravity = Gravity;
    Fx->CasingVelocity = Velocity;
}
