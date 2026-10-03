#include "OpenWillowInventoryMayaDisplay.h"
#include "OpenWillowInventory.h"
#include "Camera/PlayerCameraManager.h"
#include "Components/DirectionalLightComponent.h"
#include "Components/PostProcessComponent.h"
#include "Components/SceneComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Engine/Engine.h"
#include "Engine/GameViewportClient.h"
#include "Engine/SkeletalMesh.h"
#include "Animation/AnimSequence.h"
#include "Materials/MaterialInterface.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "GameFramework/PlayerController.h"
#include "Kismet/GameplayStatics.h"
#include "Math/RotationMatrix.h"

namespace
{
const TCHAR* BodyPath = TEXT("/Game/OpenWillow/Characters/Maya/Meshes/Skel_SirenBody/SkeletalMeshes/Skel_SirenBody.Skel_SirenBody");
const TCHAR* HeadPath = TEXT("/Game/OpenWillow/Characters/Maya/Meshes/Skel_Siren000/SkeletalMeshes/Skel_Siren000.Skel_Siren000");
// Ink outline from host/ue5/import_character_menu_look.py (absent: no outline).
const TCHAR* OutlinePath = TEXT("/Game/OpenWillow/Characters/Maya/M_OW_CharacterOutline.M_OW_CharacterOutline");
const TCHAR* MenuHeadPath = TEXT("/Game/OpenWillow/Characters/Maya/Materials/MI_InventorySirenHead.MI_InventorySirenHead");
const TCHAR* BackdropPath = TEXT("/Game/OpenWillow/Characters/Maya/M_OW_MenuBackdrop.M_OW_MenuBackdrop");

UDirectionalLightComponent* MakeMayaOnlyLight(AActor* Owner, USceneComponent* Root, const TCHAR* Name, int32 ForwardPriority)
{
    UDirectionalLightComponent* Light = Owner->CreateDefaultSubobject<UDirectionalLightComponent>(Name);
    Light->SetupAttachment(Root);
    Light->SetMobility(EComponentMobility::Movable);
    Light->SetCastShadows(false);
    // Distinct priorities: otherwise the editor prints "Multiple directional lights" over the menu.
    Light->SetForwardShadingPriority(ForwardPriority);
    Light->SetAtmosphereSunLight(false);
    // Lighting channel 1 only: the world (channel 0) is not affected, and the
    // Maya meshes join channel 1 in addition to channel 0.
    Light->LightingChannels.bChannel0 = false;
    Light->LightingChannels.bChannel1 = true;
    return Light;
}
}

AOpenWillowInventoryMayaDisplay::AOpenWillowInventoryMayaDisplay()
{
    PrimaryActorTick.bCanEverTick = false;
    SceneRoot = CreateDefaultSubobject<USceneComponent>(TEXT("SceneRoot"));
    SetRootComponent(SceneRoot);
    Body = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("MayaBody"));
    Body->SetupAttachment(SceneRoot);
    Head = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("MayaHead"));
    Head->SetupAttachment(SceneRoot);
    BodyOutline = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("MayaBodyOutline"));
    BodyOutline->SetupAttachment(SceneRoot);
    HeadOutline = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("MayaHeadOutline"));
    HeadOutline->SetupAttachment(SceneRoot);
    Weapon = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("MenuWeapon"));
    Weapon->SetupAttachment(Body, TEXT("R_Weapon_Bone"));
    WeaponOutline = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("MenuWeaponOutline"));
    WeaponOutline->SetupAttachment(Body, TEXT("R_Weapon_Bone"));
    for (USkeletalMeshComponent* Part : {Body.Get(), Head.Get(), BodyOutline.Get(), HeadOutline.Get(), Weapon.Get(), WeaponOutline.Get()})
    {
        Part->SetCollisionEnabled(ECollisionEnabled::NoCollision);
        Part->SetGenerateOverlapEvents(false);
        Part->SetCastShadow(false);
        Part->SetBoundsScale(2.f);
        Part->SetLightingChannels(true, true, false);
        Part->SetRenderCustomDepth(true);
        Part->SetCustomDepthStencilValue(247);
    }
    Backdrop = CreateDefaultSubobject<UPostProcessComponent>(TEXT("MenuBackdrop"));
    Backdrop->SetupAttachment(SceneRoot);
    Backdrop->bUnbound = true;
    Backdrop->Priority = 100.f;
    Backdrop->BlendWeight = 1.f;
    KeyLight = MakeMayaOnlyLight(this, SceneRoot, TEXT("MayaKeyLight"), 2);
    RimLight = MakeMayaOnlyLight(this, SceneRoot, TEXT("MayaRimLight"), 1);
}

void AOpenWillowInventoryMayaDisplay::PreloadAssets(TArray<TObjectPtr<UObject>>& Out)
{
    const AOpenWillowInventoryMayaDisplay* Defaults = GetDefault<AOpenWillowInventoryMayaDisplay>();
    auto Keep = [&Out](UObject* Asset) { if (Asset) Out.Add(Asset); };
    Keep(LoadObject<USkeletalMesh>(nullptr, BodyPath));
    Keep(LoadObject<USkeletalMesh>(nullptr, HeadPath));
    Keep(LoadObject<UMaterialInterface>(nullptr, MenuHeadPath));
    Keep(LoadObject<UMaterialInterface>(nullptr, BackdropPath));
    if (Defaults->OutlineThicknessCm > 0.f) Keep(LoadObject<UMaterialInterface>(nullptr, OutlinePath));
    Keep(LoadObject<UAnimSequence>(nullptr, *Defaults->IdleAnimation));
    Keep(LoadObject<UAnimSequence>(nullptr, *Defaults->WeaponIdleAnimation));
}

void AOpenWillowInventoryMayaDisplay::SetPreviewWeapon(const FOpenWillowWeaponItem* Item)
{
    const FString RecipeId = Item ? Item->Id : FString();
    if (PreviewRecipeId == RecipeId || !bHasMeshes) return;
    PreviewRecipeId = RecipeId;
    USkeletalMesh* Mesh = Item ? UOpenWillowInventory::LoadWeaponMesh(*Item) : nullptr;
    UAnimSequence* Idle = LoadObject<UAnimSequence>(nullptr, Mesh ? *WeaponIdleAnimation : *IdleAnimation);
    // Missing armed animation must not put a weapon through the unarmed hand.
    if (Mesh && !Idle) Mesh = nullptr;
    Weapon->SetSkeletalMesh(Mesh);
    WeaponOutline->SetSkeletalMesh(Mesh);
    UMaterialInterface* Outline = Mesh && OutlineThicknessCm > 0.f ? LoadObject<UMaterialInterface>(nullptr, OutlinePath) : nullptr;
    WeaponOutline->SetVisibility(Outline != nullptr);
    if (Outline)
    {
        for (int32 Slot = 0; Slot < WeaponOutline->GetNumMaterials(); ++Slot) WeaponOutline->SetMaterial(Slot, Outline);
        WeaponOutline->SetScalarParameterValueOnMaterials(TEXT("ThicknessCm"), OutlineThicknessCm);
        WeaponOutline->SetLeaderPoseComponent(Weapon);
    }
    if (!Idle) Idle = LoadObject<UAnimSequence>(nullptr, *IdleAnimation);
    if (Idle) Body->PlayAnimation(Idle, true);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow menu weapon preview: %s mesh=%d armedIdle=%d"), *RecipeId, Mesh != nullptr, Mesh && Idle);
}

void AOpenWillowInventoryMayaDisplay::ApplyBackdrop()
{
    FPostProcessSettings& S = Backdrop->Settings;
    // Grade only visible world pixels; Maya and her ink hull keep their colour.
    if (UMaterialInterface* Material = LoadObject<UMaterialInterface>(nullptr, BackdropPath))
    {
        UMaterialInstanceDynamic* Instance = UMaterialInstanceDynamic::Create(Material, this);
        Instance->SetVectorParameterValue(TEXT("BackdropGain"), BackdropGain);
        Instance->SetScalarParameterValue(TEXT("BackdropSaturation"), BackdropSaturation);
        Instance->SetScalarParameterValue(TEXT("VignetteIntensity"), VignetteIntensity);
        S.AddBlendable(Instance, 1.f);
    }
    else
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow menu backdrop material missing; run import_character_menu_look.py (world dimming disabled)"));
    // Depth of field focused on Maya so the world behind her blurs.
    S.bOverride_DepthOfFieldFocalDistance = true;
    S.DepthOfFieldFocalDistance = DistanceCm;
    S.bOverride_DepthOfFieldFstop = true;
    S.DepthOfFieldFstop = DepthOfFieldFstop;
    KeyLight->SetLightColor(KeyColor);
    KeyLight->SetIntensity(KeyIntensity);
    RimLight->SetLightColor(RimColor);
    RimLight->SetIntensity(RimIntensity);
}

void AOpenWillowInventoryMayaDisplay::BeginPlay()
{
    Super::BeginPlay();
    USkeletalMesh* BodyMesh = LoadObject<USkeletalMesh>(nullptr, BodyPath);
    USkeletalMesh* HeadMesh = LoadObject<USkeletalMesh>(nullptr, HeadPath);
    bHasMeshes = BodyMesh && HeadMesh;
    if (!bHasMeshes)
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow inventory Maya meshes missing; display hidden"));
        SetActorHiddenInGame(true);
        return;
    }
    Body->SetSkeletalMesh(BodyMesh);
    Head->SetSkeletalMesh(HeadMesh);
    // Preview-specific palette correction; the gameplay mesh/material stays separate.
    if (UMaterialInterface* MenuHead = LoadObject<UMaterialInterface>(nullptr, MenuHeadPath))
    {
        Head->SetMaterial(0, MenuHead);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow inventory Maya preview head palette loaded"));
    }
    // BL2's own inventory idle (Base_Siren.Idle_Inventory, imported by import_character_anims.py).
    // The head mesh has its own 7-bone skeleton with the body's bone names, so it follows the body.
    if (UAnimSequence* Idle = LoadObject<UAnimSequence>(nullptr, *IdleAnimation))
        Body->PlayAnimation(Idle, true);
    else
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow inventory Maya idle missing (%s); bind pose shown"), *IdleAnimation);
    Head->SetLeaderPoseComponent(Body);
    // Inverted-hull ink line: copies of both meshes that follow the body, drawn back faces only and
    // pushed out along the normal by the material.
    UMaterialInterface* Outline = OutlineThicknessCm > 0.f ? LoadObject<UMaterialInterface>(nullptr, OutlinePath) : nullptr;
    const TPair<USkeletalMeshComponent*, USkeletalMesh*> Outlines[] = {{BodyOutline, BodyMesh}, {HeadOutline, HeadMesh}};
    for (const auto& Pair : Outlines)
    {
        Pair.Key->SetVisibility(Outline != nullptr);
        if (!Outline) continue;
        Pair.Key->SetSkeletalMesh(Pair.Value);
        for (int32 Slot = 0; Slot < Pair.Key->GetNumMaterials(); ++Slot)
            Pair.Key->SetMaterial(Slot, Outline);
        Pair.Key->SetScalarParameterValueOnMaterials(TEXT("ThicknessCm"), OutlineThicknessCm);
        Pair.Key->SetLeaderPoseComponent(Body);
    }
    SetActorScale3D(FVector(MeshScale));
    const FBoxSphereBounds BodyBounds = BodyMesh->GetBounds();
    const FBoxSphereBounds HeadBounds = HeadMesh->GetBounds();
    MeshTopZ = FMath::Max(BodyBounds.Origin.Z + BodyBounds.BoxExtent.Z, HeadBounds.Origin.Z + HeadBounds.BoxExtent.Z);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow inventory Maya bind-pose top Z %.1f cm"), MeshTopZ);
    ApplyBackdrop();
    if (APlayerController* PC = UGameplayStatics::GetPlayerController(this, 0))
        if (PC->PlayerCameraManager)
        {
            PC->PlayerCameraManager->SetFOV(MenuHorizontalFov);
            bLockedFov = true;
        }
}

void AOpenWillowInventoryMayaDisplay::EndPlay(const EEndPlayReason::Type Reason)
{
    // Give the gameplay camera back its own FOV; the post-process and lights
    // go away with the components.
    if (bLockedFov)
        if (APlayerController* PC = UGameplayStatics::GetPlayerController(this, 0))
            if (PC->PlayerCameraManager) PC->PlayerCameraManager->UnlockFOV();
    bLockedFov = false;
    Super::EndPlay(Reason);
}

void AOpenWillowInventoryMayaDisplay::UpdateView(const FVector& CameraLocation, const FRotator& CameraRotation)
{
    if (!bHasMeshes) return;
    float Aspect = 16.f / 9.f;
    if (GEngine && GEngine->GameViewport)
    {
        FVector2D Size = FVector2D::ZeroVector;
        GEngine->GameViewport->GetViewportSize(Size);
        if (Size.Y > KINDA_SMALL_NUMBER) Aspect = Size.X / Size.Y;
    }
    const float TanH = FMath::Tan(FMath::DegreesToRadians(MenuHorizontalFov) * 0.5f);
    const float TanV = TanH / Aspect;
    const FRotationMatrix CameraAxes(CameraRotation);
    const FVector Forward = CameraAxes.GetUnitAxis(EAxis::X);
    const FVector Right = CameraAxes.GetUnitAxis(EAxis::Y);
    const FVector Up = CameraAxes.GetUnitAxis(EAxis::Z);
    // Anchor the top of her head at the requested screen point (a point at
    // DistanceCm ahead spans +-TanH*Distance sideways, +-TanV*Distance up),
    // then hang the meshes below it. Upright and turned toward the menu.
    const FVector HeadTop = CameraLocation + Forward * DistanceCm
        + Right * ((ScreenX * 2.f - 1.f) * TanH * DistanceCm)
        + Up * ((1.f - HeadTopScreenY * 2.f) * TanV * DistanceCm);
    // This is a presentation copy: orient its up axis with the menu camera,
    // rather than the world, so looking up/down before opening the menu does
    // not change Maya's screen size, lean or overlap with the Backpack panel.
    const FQuat DisplayRotation = CameraRotation.Quaternion()
        * FRotator(0.f, 180.f + TurnTowardMenuDeg, 0.f).Quaternion();
    SetActorRotation(DisplayRotation);
    SetActorLocation(HeadTop - DisplayRotation.RotateVector(FVector::UpVector * (MeshTopZ * MeshScale)));

    // Light travel directions in camera space. Key: warm, from the front-right
    // and above. Rim: cool, from behind-left (the menu side) and above.
    KeyLight->SetWorldRotation(FRotationMatrix::MakeFromX(Forward - Right * 0.7f - Up * 0.6f).Rotator());
    RimLight->SetWorldRotation(FRotationMatrix::MakeFromX(-Forward + Right * 0.9f - Up * 0.4f).Rotator());
}
