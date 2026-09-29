#include "OpenWillowInventoryPreviewActor.h"
#include "Components/SceneCaptureComponent2D.h"
#include "Components/SkeletalMeshComponent.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/TextureRenderTarget2D.h"

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

    Capture = CreateDefaultSubobject<USceneCaptureComponent2D>(TEXT("InventoryCapture"));
    Capture->SetupAttachment(SceneRoot);
    Capture->PrimitiveRenderMode = ESceneCapturePrimitiveRenderMode::PRM_UseShowOnlyList;
    Capture->ShowOnlyComponents.Add(PreviewWeapon);
    Capture->CaptureSource = SCS_FinalColorLDR;
    Capture->bCaptureEveryFrame = false;
    Capture->bCaptureOnMovement = false;
    Capture->FOVAngle = 30.f;
}

void AOpenWillowInventoryPreviewActor::BeginPlay()
{
    Super::BeginPlay();

    RenderTarget = NewObject<UTextureRenderTarget2D>(this, TEXT("InventoryWeaponPreview"));
    RenderTarget->ClearColor = FLinearColor(0.02f, 0.025f, 0.035f, 1.f);
    RenderTarget->InitAutoFormat(512, 320);
    Capture->TextureTarget = RenderTarget;
}

bool AOpenWillowInventoryPreviewActor::SetItemPreview(const FString& ItemId)
{
    if (!RenderTarget || !Capture || !PreviewWeapon)
        return false;
    if (CurrentItemId == ItemId)
        return bHasPreview;

    CurrentItemId = ItemId;
    bHasPreview = false;
    PreviewWeapon->SetSkeletalMesh(nullptr);

    if (!ItemId.IsEmpty())
    {
        const FString MeshPath = FString::Printf(
            TEXT("/Game/OpenWillow/Weapons/Items/SK_%s.SK_%s"), *ItemId, *ItemId);
        if (USkeletalMesh* Mesh = LoadObject<USkeletalMesh>(nullptr, *MeshPath))
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
