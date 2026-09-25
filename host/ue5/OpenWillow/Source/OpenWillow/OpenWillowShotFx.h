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
    static void Tracer(UWorld* World, const FVector& Start, const FVector& End,
        const FLinearColor& Color = FLinearColor(1.f, 0.78f, 0.45f), float Radius = 0.8f, float Life = 0.09f);
    static void Flash(UWorld* World, const FVector& Location, const FLinearColor& Color,
        float Radius, float Life, float LightIntensity);
private:
    void Setup(UStaticMesh* Mesh, const FLinearColor& Color, float Intensity, float Life, float LightIntensity);
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Visual;
    UPROPERTY() TObjectPtr<class UPointLightComponent> Light;
    UPROPERTY() TObjectPtr<class UMaterialInstanceDynamic> Material;
    float Born = 0;
    float Lifetime = 0.05f;
    float StartIntensity = 0;
    float StartLight = 0;
};
