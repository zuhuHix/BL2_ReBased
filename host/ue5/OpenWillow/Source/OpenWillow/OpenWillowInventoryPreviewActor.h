#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowInventoryPreviewActor.generated.h"

class USceneCaptureComponent2D;
class USceneComponent;
class USkeletalMeshComponent;
class UTextureRenderTarget2D;
class UPointLightComponent;
struct FOpenWillowWeaponItem;

// Isolated render target for the inventory card. The weapon is visible only
// to this scene capture, so previewing an item never puts it into the level.
UCLASS()
class OPENWILLOW_API AOpenWillowInventoryPreviewActor : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowInventoryPreviewActor();

    // Null clears the preview. The mesh is found the same way as the held weapon (UOpenWillowInventory::LoadWeaponMesh).
    bool SetItemPreview(const FOpenWillowWeaponItem* Item);
    // Absolute orbit angles, clamped by the host. Returns a local PNG data URL.
    FString InspectFrame(float Yaw, float Pitch);
    UTextureRenderTarget2D* GetRenderTarget() const { return RenderTarget; }

protected:
    virtual void BeginPlay() override;

private:
    UPROPERTY() TObjectPtr<USceneComponent> SceneRoot;
    UPROPERTY() TObjectPtr<USkeletalMeshComponent> PreviewWeapon;
    UPROPERTY() TObjectPtr<USceneCaptureComponent2D> Capture;
    UPROPERTY() TObjectPtr<UTextureRenderTarget2D> RenderTarget;
    UPROPERTY() TObjectPtr<UPointLightComponent> KeyLight;
    UPROPERTY() TObjectPtr<UPointLightComponent> FillLight;
    FString CurrentItemId;
    bool bHasPreview = false;
};
