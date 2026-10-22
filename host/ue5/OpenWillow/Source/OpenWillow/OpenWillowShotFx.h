#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowShotFx.generated.h"

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
        float Radius, float Life, float LightIntensity);
    // Two-layer host muzzle effect. Radius remains an UNVERIFIED stand-in because the type has no scalar size field.
    static void MuzzleFlash(UWorld* World, const FVector& Location, float Radius, float Life,
        const FLinearColor& Color);
    // Low-cost brass cylinder stand-in, launched from the selected type/body socket toward the right. Its size, life and
    // ballistic constants are UNVERIFIED host values; the type data only identifies the casing template/socket.
    static void Casing(UWorld* World, const FVector& Location, const FVector& Velocity, float Scale = 1.f);
private:
    void Setup(UStaticMesh* Mesh, const FLinearColor& Color, float Intensity, float Life, float LightIntensity);
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Visual;
    UPROPERTY() TObjectPtr<class UPointLightComponent> Light;
    UPROPERTY() TObjectPtr<class UMaterialInstanceDynamic> Material;
    float Born = 0;
    float Lifetime = 0.05f;
    float StartIntensity = 0;
    float StartLight = 0;
    FVector CasingVelocity = FVector::ZeroVector;
    bool bIsCasing = false;
};
