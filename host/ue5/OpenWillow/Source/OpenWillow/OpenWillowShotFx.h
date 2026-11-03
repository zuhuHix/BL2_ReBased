#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowShotFx.generated.h"

// One emitter of the game's muzzle-flash particle system, as tools/weapon_muzzle_fx.py reads it from the cooked data
// (weapon_view.json "muzzle_flash_fx"): sizes and offsets in cm in the muzzle's frame (X along the barrel), lifetimes in s,
// colour as the emitter's HDR start colour. Axis: "EPAL_Y" / "EPAL_Z" lock a rectangle to the barrel's side / top plane,
// "EPAL_X" / "EPAL_NEGATIVE_X" or none face along the barrel; smoke layers face the camera.
struct FOpenWillowFlashLayer
{
    FString Name;
    FString Axis;
    float LifeMin = 0.05f, LifeMax = 0.05f;
    float LengthMin = 10.f, LengthMax = 10.f;   // size_x
    float Width = 0.f;                          // size_y (0: square)
    float OffsetX = 0.f;
    FLinearColor Color = FLinearColor(1.5f, 0.25f, 0.09f);
    float AlphaMin = 1.f, AlphaMax = 1.f;
    bool bSmoke = false;
    bool bAngleFade = false;                    // the glow material fades when its quad is seen edge-on
    bool bModulate = false;                     // a modulate (multiply) glow material: it tints the scene, it does not add light
};

// Short-lived host effects (tracer, muzzle flash, impact spark). These are
// stand-ins built from engine shapes and M_OW_FxAdditive, not BL2's particle
// systems (e.g. FX_WEP_Pistol), which are not hosted yet.
UCLASS()
class OPENWILLOW_API AOpenWillowShotFx : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowShotFx();
    virtual void Tick(float DeltaSeconds) override;
    // The fallback line size/lifetime are UNVERIFIED host stand-ins; data-backed weapons with no tracer template suppress it.
    static void Tracer(UWorld* World, const FVector& Start, const FVector& End,
        const FLinearColor& Color = FLinearColor(1.f, 0.78f, 0.45f), float Radius = 0.8f, float Life = 0.09f);
    static void Flash(UWorld* World, const FVector& Location, const FLinearColor& Color,
        float Radius, float Life, float LightIntensity, float Intensity = 8.f);
    // Soft, procedural host muzzle plume. Radius remains an UNVERIFIED stand-in because the type has no scalar size field.
    static void MuzzleFlash(UWorld* World, const FVector& Location, float Radius, float Life,
        const FLinearColor& Color);
    // The game's flash emitters drawn as soft quads at the muzzle (Forward = barrel direction, Up = the gun's up).
    // The emitter textures are not hosted: TextureFill (the share of a quad the real sprite covers) and the intensity
    // mapping are UNVERIFIED host fits; sizes, offsets, lifetimes, colours and axis locks are the cooked values.
    static void MuzzleFlashLayers(UWorld* World, const FVector& Location, const FVector& Forward, const FVector& Up,
        const TArray<FOpenWillowFlashLayer>& FlashLayers, float TextureFill = 0.5f);
    // Low-cost brass cylinder stand-in, launched from the selected type/body socket toward the right. Its size, life and
    // ballistic constants are UNVERIFIED host values; the type data only identifies the casing template/socket.
    // A small glowing sphere thrown from an impact or explosion (falls like a casing; no spin of note). Host stand-in for
    // the impact and explosion particle debris (weapons projectile lane).
    static void Spark(UWorld* World, const FVector& Location, const FVector& Velocity, const FLinearColor& Color,
        float Radius, float Life, float Intensity = 4.f);
    // Life and Gravity default to the older host values; a type with a cooked casing system passes its own (0.35 s and
    // the particle acceleration for the assault rifle casing).
    static void Casing(UWorld* World, const FVector& Location, const FVector& Velocity, float Scale = 1.f,
        float Life = 0.22f, float Gravity = 980.f);
private:
    // Spawns one effect actor unless this frame's cosmetic budget is spent or the spawn fails (nullptr then).
    static AOpenWillowShotFx* SpawnBudgeted(UWorld* World, const FTransform& Pose);
    void Setup(UStaticMesh* Mesh, const FLinearColor& Color, float Intensity, float Life, float LightIntensity,
        bool bSoftFlame = false, float CoreStrength = 0.f);
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Visual;
    UPROPERTY() TObjectPtr<class UPointLightComponent> Light;
    UPROPERTY() TObjectPtr<class UMaterialInstanceDynamic> Material;
    float Born = 0;
    float Lifetime = 0.05f;
    float StartIntensity = 0;
    float StartLight = 0;
    FVector CasingVelocity = FVector::ZeroVector;
    float CasingGravity = 980.f;
    bool bIsCasing = false;
};
