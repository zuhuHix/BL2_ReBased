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
    // A tracer that flies from Start to End at Speed (uu/s) as a streak of Length, the way a bullet's tracer follows the bullet
    // (bStartParticleTracerAtWeapon); Tracer() above is the instant one.
    static void TracerFlight(UWorld* World, const FVector& Start, const FVector& End, float Speed, float Length,
        const FLinearColor& Color = FLinearColor(1.f, 0.78f, 0.45f), float Radius = 0.8f);
    static void Flash(UWorld* World, const FVector& Location, const FLinearColor& Color,
        float Radius, float Life, float LightIntensity);
private:
    void Setup(UStaticMesh* Mesh, const FLinearColor& Color, float Intensity, float Life, float LightIntensity);
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Visual;
    UPROPERTY() TObjectPtr<class UPointLightComponent> Light;
    UPROPERTY() TObjectPtr<class UMaterialInstanceDynamic> Material;
    // TracerFlight: the streak's head runs Start + Dir * min(Speed * age, Distance) and its tail trails by FlightLength.
    bool bFlying = false;
    FVector FlightStart = FVector::ZeroVector;
    FVector FlightDir = FVector::ForwardVector;
    float FlightDistance = 0;
    float FlightSpeed = 0;
    float FlightLength = 0;
    float Born = 0;
    float Lifetime = 0.05f;
    float StartIntensity = 0;
    float StartLight = 0;
};
