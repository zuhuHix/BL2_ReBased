#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowInventoryMayaDisplay.generated.h"

class UDirectionalLightComponent;
class UPoseableMeshComponent;
class UPostProcessComponent;
class USceneComponent;
class USkeletalMeshComponent;
struct FOpenWillowWeaponItem;

// Local inventory presentation of Maya's imported body and head. The movie
// draws the panels; this actor lets the same local meshes appear beside them.
//
// The actor owns everything that changes while the menu is open, so closing
// the menu (Destroy) restores the gameplay view by itself: it locks the
// camera FOV, adds an unbound post-process (dimmed, desaturated, blurred
// backdrop plus a light vignette) and two lights that only affect Maya
// (lighting channel 1). The look is an art-direction approximation of the
// real inventory screen, not a reproduction of BL2's own rig (UNVERIFIED).
//
// All values below are tunable in DefaultGame.ini without a rebuild:
//   [/Script/OpenWillow.OpenWillowInventoryMayaDisplay]
//   DistanceCm=300
// Screen fractions are 0..1 from the top-left of the viewport.
UCLASS(Config=Game)
class OPENWILLOW_API AOpenWillowInventoryMayaDisplay : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowInventoryMayaDisplay();
    // Null shows Maya unarmed. The mesh is found the same way as the held weapon (UOpenWillowInventory::LoadWeaponMesh).
    void SetPreviewWeapon(const FOpenWillowWeaponItem* Item);
    virtual void BeginPlay() override;
    virtual void EndPlay(const EEndPlayReason::Type Reason) override;

    bool HasMeshes() const { return bHasMeshes; }
    void UpdateView(const FVector& CameraLocation, const FRotator& CameraRotation);

    // Framing. The camera FOV is locked while the menu is open so Maya can be
    // large without wide-angle distortion. Horizontal FOV in degrees.
    UPROPERTY(EditAnywhere, Config, Category = "Framing") float MenuHorizontalFov = 38.f;
    // Distance from the camera to Maya, cm.
    UPROPERTY(EditAnywhere, Config, Category = "Framing") float DistanceCm = 300.f;
    // Where Maya's centre line lands horizontally (0 = left edge, 1 = right).
    UPROPERTY(EditAnywhere, Config, Category = "Framing") float ScreenX = 0.86f;
    // Where the top of her head lands vertically (0 = top edge). The rest of
    // her extends below the bottom of the frame, like the real screen.
    UPROPERTY(EditAnywhere, Config, Category = "Framing") float HeadTopScreenY = 0.16f;
    // Uniform scale of the display copy, purely to trim the height fit.
    UPROPERTY(EditAnywhere, Config, Category = "Framing") float MeshScale = 1.f;
    // Degrees she is turned from facing the camera, toward the menu (screen
    // left). Real screen: roughly a three-quarter turn.
    UPROPERTY(EditAnywhere, Config, Category = "Framing") float TurnTowardMenuDeg = 28.f;

    // Animation played on the display copy (the game's inventory idle by default).
    UPROPERTY(EditAnywhere, Config, Category = "Pose") FString IdleAnimation =
        TEXT("/Game/OpenWillow/Characters/Maya/ThirdPerson/Anim_Body_Idle_Inventory.Anim_Body_Idle_Inventory");
    UPROPERTY(EditAnywhere, Config, Category = "Pose") FString WeaponIdleAnimation =
        TEXT("/Game/OpenWillow/Characters/Maya/ThirdPerson/Anim_InventoryRifle_Idle_Inventory.Anim_InventoryRifle_Idle_Inventory");
    // Width of the ink outline (0 = none). UNVERIFIED art-direction value.
    UPROPERTY(EditAnywhere, Config, Category = "Pose") float OutlineThicknessCm = 0.35f;

    // Backdrop post-process. Stencil 247 excludes visible Maya/outline pixels.
    UPROPERTY(EditAnywhere, Config, Category = "Backdrop") FLinearColor BackdropGain = FLinearColor(0.70f, 0.72f, 0.78f);
    UPROPERTY(EditAnywhere, Config, Category = "Backdrop") float BackdropSaturation = 0.75f;
    // Cinematic depth of field focused on Maya; lower f-stop = blurrier
    // backdrop.
    UPROPERTY(EditAnywhere, Config, Category = "Backdrop") float DepthOfFieldFstop = 16.f;
    UPROPERTY(EditAnywhere, Config, Category = "Backdrop") float VignetteIntensity = 0.35f;

    // Lights that only affect Maya. Intensities are lux.
    UPROPERTY(EditAnywhere, Config, Category = "Lighting") FLinearColor KeyColor = FLinearColor(1.f, 0.66f, 0.42f);
    UPROPERTY(EditAnywhere, Config, Category = "Lighting") float KeyIntensity = 1.5f;
    UPROPERTY(EditAnywhere, Config, Category = "Lighting") FLinearColor RimColor = FLinearColor(0.35f, 0.7f, 1.f);
    UPROPERTY(EditAnywhere, Config, Category = "Lighting") float RimIntensity = 3.f;

private:
    UPROPERTY() TObjectPtr<USceneComponent> SceneRoot;
    UPROPERTY() TObjectPtr<USkeletalMeshComponent> Body;
    UPROPERTY() TObjectPtr<USkeletalMeshComponent> Head;
    UPROPERTY() TObjectPtr<USkeletalMeshComponent> BodyOutline;
    UPROPERTY() TObjectPtr<USkeletalMeshComponent> HeadOutline;
    UPROPERTY() TObjectPtr<USkeletalMeshComponent> Weapon;
    UPROPERTY() TObjectPtr<USkeletalMeshComponent> WeaponOutline;
    FString PreviewRecipeId;
    UPROPERTY() TObjectPtr<UPostProcessComponent> Backdrop;
    UPROPERTY() TObjectPtr<UDirectionalLightComponent> KeyLight;
    UPROPERTY() TObjectPtr<UDirectionalLightComponent> RimLight;
    bool bHasMeshes = false;
    bool bLockedFov = false;
    // Mesh-space top of the tallest part in the bind pose, cm (unscaled).
    float MeshTopZ = 170.f;
    void ApplyBackdrop();
};
