#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInventoryPickup.generated.h"

class USceneComponent;
class USkeletalMeshComponent;
class UStaticMeshComponent;
class UTextRenderComponent;

// A dropped item in the current world. The inventory remains the authority
// for capacity and stable instance identity when it is picked up again.
UCLASS()
class OPENWILLOW_API AOpenWillowInventoryPickup : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowInventoryPickup();
    void Initialize(FOpenWillowTakenInventoryItem InItem);
    bool TryPickup(UOpenWillowInventory* Inventory);
    FString DisplayName() const;

private:
    UPROPERTY() TObjectPtr<USceneComponent> SceneRoot;
    UPROPERTY() TObjectPtr<USkeletalMeshComponent> WeaponVisual;
    UPROPERTY() TObjectPtr<UStaticMeshComponent> GearVisual;
    UPROPERTY() TObjectPtr<UTextRenderComponent> Label;
    FOpenWillowTakenInventoryItem Item;
    bool bInitialized = false;
};
