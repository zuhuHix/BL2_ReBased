#include "OpenWillowInventoryPreviewActor.h"
#include "OpenWillowInventory.h"
#include "Components/SceneCaptureComponent2D.h"
#include "Components/SkeletalMeshComponent.h"
#include "Components/PointLightComponent.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/TextureRenderTarget2D.h"
#include "ImageUtils.h"
#include "Misc/Base64.h"

AOpenWillowInventoryPreviewActor::AOpenWillowInventoryPreviewActor()
{
    PrimaryActorTick.bCanEverTick = false;

    SceneRoot = CreateDefaultSubobject<USceneComponent>(TEXT("SceneRoot"));
    SetRootComponent(SceneRoot);

    PreviewWeapon = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("PreviewWeapon"));
    PreviewWeapon->SetupAttachment(SceneRoot);
    PreviewWeapon->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    PreviewWeapon->SetGenerateOverlapEvents(false);
    PreviewWeapon->SetCastShadow(false);
    PreviewWeapon->SetVisibleInSceneCaptureOnly(true);
    PreviewWeapon->SetLightingChannels(false, false, true);

    KeyLight = CreateDefaultSubobject<UPointLightComponent>(TEXT("PreviewKey"));
    FillLight = CreateDefaultSubobject<UPointLightComponent>(TEXT("PreviewFill"));
    for (UPointLightComponent* Light : {KeyLight.Get(), FillLight.Get()})
    {
        Light->SetupAttachment(SceneRoot);
        Light->SetMobility(EComponentMobility::Movable);
        Light->SetCastShadows(false);
        Light->LightingChannels.bChannel0 = false;
        Light->LightingChannels.bChannel2 = true;
        Light->SetIntensityUnits(ELightUnits::Lumens);
        Light->SetAttenuationRadius(1000.f);
    }
    KeyLight->SetRelativeLocation(FVector(150, -200, 150));
    KeyLight->SetIntensity(2000.f);
    FillLight->SetRelativeLocation(FVector(-100, 150, 80));
    FillLight->SetIntensity(1000.f);

    Capture = CreateDefaultSubobject<USceneCaptureComponent2D>(TEXT("InventoryCapture"));
    Capture->SetupAttachment(SceneRoot);
    Capture->PrimitiveRenderMode = ESceneCapturePrimitiveRenderMode::PRM_UseShowOnlyList;
    Capture->ShowOnlyComponents.Add(PreviewWeapon);
    Capture->CaptureSource = SCS_FinalColorLDR;
    Capture->bCaptureEveryFrame = false;
    Capture->bCaptureOnMovement = false;
    Capture->FOVAngle = 30.f;
    Capture->PostProcessSettings.bOverride_AutoExposureMethod = true;
    Capture->PostProcessSettings.AutoExposureMethod = AEM_Manual;
    Capture->PostProcessSettings.bOverride_AutoExposureApplyPhysicalCameraExposure = true;
    Capture->PostProcessSettings.AutoExposureApplyPhysicalCameraExposure = false;
}

void AOpenWillowInventoryPreviewActor::BeginPlay()
{
    Super::BeginPlay();

    RenderTarget = NewObject<UTextureRenderTarget2D>(this, TEXT("InventoryWeaponPreview"));
    RenderTarget->ClearColor = FLinearColor(0.02f, 0.025f, 0.035f, 1.f);
    RenderTarget->InitAutoFormat(512, 320);
    Capture->TextureTarget = RenderTarget;
}

bool AOpenWillowInventoryPreviewActor::SetItemPreview(const FOpenWillowWeaponItem* Item)
{
    if (!RenderTarget || !Capture || !PreviewWeapon)
        return false;
    const FString ItemId = Item ? Item->Id : FString();
    if (CurrentItemId == ItemId)
        return bHasPreview;

    CurrentItemId = ItemId;
    bHasPreview = false;
    PreviewWeapon->SetSkeletalMesh(nullptr);

    if (Item)
    {
        if (USkeletalMesh* Mesh = UOpenWillowInventory::LoadWeaponMesh(*Item))
        {
            PreviewWeapon->SetSkeletalMesh(Mesh);
            // Match the orientation used by the held weapon. Center the mesh
            // and frame it from its own imported bounds so different gun
            // lengths fit the same card.
            PreviewWeapon->SetRelativeRotation(FRotator(0.f, 90.f, 0.f));
            const FBoxSphereBounds Bounds = PreviewWeapon->GetLocalBounds();
            PreviewWeapon->SetRelativeLocation(-Bounds.Origin);
            const float Radius = FMath::Max(Bounds.SphereRadius, 20.f);
            const FVector CameraOffset(Radius * 2.2f, -Radius * 5.2f, Radius * 1.2f);
            Capture->SetRelativeLocation(CameraOffset);
            Capture->SetRelativeRotation((-CameraOffset).Rotation());
            bHasPreview = true;
        }
    }

    // Also clears the previous weapon from the render target when this item
    // has no imported mesh.
    Capture->CaptureScene();
    return bHasPreview;
}

FString AOpenWillowInventoryPreviewActor::InspectFrame(float Yaw, float Pitch)
{
    if (!bHasPreview || !RenderTarget || !FMath::IsFinite(Yaw) || !FMath::IsFinite(Pitch)) return FString();
    const FBoxSphereBounds Bounds = PreviewWeapon->GetLocalBounds();
    const float Radius = FMath::Max(Bounds.SphereRadius, 20.f);
    const FVector BaseOffset(Radius * 2.2f, -Radius * 5.2f, Radius * 1.2f);
    const FVector Offset = FRotator(FMath::Clamp(Pitch, -80.f, 80.f), FMath::Fmod(Yaw, 360.f), 0.f).RotateVector(BaseOffset);
    Capture->SetRelativeLocation(Offset);
    Capture->SetRelativeRotation((-Offset).Rotation());
    Capture->CaptureScene();
    FImage Image;
    TArray64<uint8> Png;
    if (!FImageUtils::GetRenderTargetImage(RenderTarget, Image)
        || !FImageUtils::CompressImage(Png, TEXT("png"), Image, 0) || Png.IsEmpty()) return FString();
    return TEXT("data:image/png;base64,") + FBase64::Encode(Png.GetData(), Png.Num());
}
