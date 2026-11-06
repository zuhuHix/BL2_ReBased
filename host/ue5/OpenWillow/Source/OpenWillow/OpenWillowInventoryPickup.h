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
    const FOpenWillowTakenInventoryItem& GetItem() const { return Item; }
    // The colour of the drop's locator beam: the item card's "#RRGGBB" (read from the game's rarity table by the recipe tools,
    // under ignored local/), white when the item has none. Public for the automation test.
    static FLinearColor RarityBeamColor(const FString& HexColor);
    // Capture helper (-owdropshots in the -owgunshots sequence): drops a copy of Weapon on the ground Distance cm in front of
    // Viewer, for LifeSeconds, so the frame shows the world-drop look. Returns null when no ground was found.
    static AOpenWillowInventoryPickup* SpawnShowcase(AActor* Viewer, const FOpenWillowWeaponItem& Weapon,
                                                     float Distance = 450.f, float LifeSeconds = 1.5f);

private:
    UPROPERTY() TObjectPtr<USceneComponent> SceneRoot;
    UPROPERTY() TObjectPtr<USkeletalMeshComponent> WeaponVisual;
    UPROPERTY() TObjectPtr<UStaticMeshComponent> GearVisual;
    UPROPERTY() TObjectPtr<UTextRenderComponent> Label;
    UPROPERTY() TObjectPtr<UStaticMeshComponent> RarityBeam;
    FOpenWillowTakenInventoryItem Item;
    bool bInitialized = false;
};
