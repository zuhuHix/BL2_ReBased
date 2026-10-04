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
    // Capture/review helper (lane C): writes a side view of the current preview to a PNG at Width x Height. Radius is the
    // framing radius in cm (the mesh bounds count the whole gestalt, so they are no use here); Yaw/Pitch orbit as InspectFrame.
    // Resizes the render target to the requested size and leaves it that way.
    bool SaveFrame(const FString& Path, float Yaw, float Pitch, float Radius, int32 Width, int32 Height);

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
