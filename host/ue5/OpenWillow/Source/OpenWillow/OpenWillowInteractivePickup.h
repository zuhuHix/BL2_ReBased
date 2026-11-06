// AI-assisted (Claude). A loot pickup dropped by a lootable: cash, ammo, eridium or a health vial (PR 3). What each kind does
// when taken is UNVERIFIED in game; the amounts come from the manifest (OpenWillowInteractive.h). Gun and gear drops are not
// this actor: they use the existing AOpenWillowInventoryPickup.
#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowInteractive.h"
#include "OpenWillowInteractivePickup.generated.h"

class AOpenWillowWalker;
class UStaticMeshComponent;
class UTextRenderComponent;

UCLASS()
class OPENWILLOW_API AOpenWillowLootPickup : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowLootPickup();
    // Value: the cash amount for a cash pickup (computed by the spawner); the amount for the other kinds.
    void Configure(const FOpenWillowPickupSpec& InSpec, int32 InValue, const FString& InBalance);
    // Applies the pickup to the walker and logs one OWPICKUP line. True when it was taken (the actor is then destroyed).
    // A refusal leaves the pickup where it is.
    bool TryCollect(AOpenWillowWalker& Walker);
    // Whether TryCollect would take it now. One that would refuse (a full pool, full health) is not a use target at all.
    bool WouldCollect(const AOpenWillowWalker& Walker) const;

    const FOpenWillowPickupSpec& Spec() const { return Spec_; }
    int32 Value() const { return Value_; }
    const FString& Balance() const { return Balance_; }
    // Auto pickups are retried by the subsystem's tick no sooner than this world time (a refusal waits two seconds).
    double NextAutoTryAt = 0;

private:
    bool CollectMoney(AOpenWillowWalker& Walker, FString& OutReason, double& OutBefore, double& OutAfter, double& OutCap);
    bool CollectEridium(AOpenWillowWalker& Walker, FString& OutReason, double& OutBefore, double& OutAfter, double& OutCap);
    bool CollectAmmo(AOpenWillowWalker& Walker, FString& OutReason, double& OutBefore, double& OutAfter, double& OutCap);
    bool CollectHealth(AOpenWillowWalker& Walker, FString& OutReason, double& OutBefore, double& OutAfter, double& OutCap);

    FOpenWillowPickupSpec Spec_;
    int32 Value_ = 0;
    FString Balance_;
    UPROPERTY() TObjectPtr<UStaticMeshComponent> Visual;
    UPROPERTY() TObjectPtr<UTextRenderComponent> Label;
};
