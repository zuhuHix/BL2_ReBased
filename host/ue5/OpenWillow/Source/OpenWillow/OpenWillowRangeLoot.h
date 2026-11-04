#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "Math/RandomStream.h"
#include "OpenWillowInventory.h"
#include "OpenWillowRangeKind.h"
#include "OpenWillowRangeLoot.generated.h"

class AOpenWillowEnemy;
class AOpenWillowWalker;
class UPrimitiveComponent;
class USphereComponent;
class UStaticMeshComponent;
class UWorld;

// ow.RangeLoot: normal rolls the manifest's chances; always tries every weapon, money and ammo pool (a tester gets drops);
// off drops nothing. Host choice, UNVERIFIED in game.
enum class EOpenWillowRangeLootMode : uint8
{
    Normal,
    Always,
    Off
};

enum class EOpenWillowRangePickupKind : uint8
{
    Money,
    Ammo
};

// A money or ammo pickup of the range (PR3). Host stand-in look and value (UNVERIFIED in game): a gold cylinder for money, a
// grey box for ammo. Walking into it (overlap with Maya) adds the value to her inventory and destroys it.
UCLASS()
class OPENWILLOW_API AOpenWillowRangePickup : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowRangePickup();
    // Money: Amount is the money. Ammo: Amount rounds of pool InAmmo (the reserve is capped at the pool's maximum).
    void Initialize(EOpenWillowRangePickupKind InKind, int32 InAmount, EOpenWillowAmmoType InAmmo);
    virtual void Tick(float DeltaSeconds) override;
    EOpenWillowRangePickupKind GetPickupKind() const { return PickupKind; }
    int32 GetAmount() const { return Amount; }

private:
    UFUNCTION()
    void OnBodyBeginOverlap(UPrimitiveComponent* OverlappedComponent, AActor* OtherActor, UPrimitiveComponent* OtherComp,
        int32 OtherBodyIndex, bool bFromSweep, const FHitResult& SweepResult);
    void Collect(AOpenWillowWalker* Walker);

    UPROPERTY() TObjectPtr<USphereComponent> Body;
    UPROPERTY() TObjectPtr<UStaticMeshComponent> Visual;
    EOpenWillowRangePickupKind PickupKind = EOpenWillowRangePickupKind::Money;
    int32 Amount = 0;
    EOpenWillowAmmoType AmmoType = EOpenWillowAmmoType::Pistol;
    bool bCollected = false;
};

// What one enemy death rolled. The range keeps the last one (the test driver reads it).
struct FOpenWillowRangeLootResult
{
    int32 Weapons = 0, Money = 0, Ammo = 0;   // pickups spawned by the roll, by kind
    TArray<TWeakObjectPtr<AActor>> Spawned;   // every pickup the roll spawned, in spawn order
    FString Trace;                            // "pool:tried:roll;" for each pool, in order (the determinism check)
};

// The death roll (PR3): COMBAT_LOOT's loop as the host runs it. For every pool of the level entry, in order, one FRand draw;
// the pool is tried when its chance is above 0 and the draw is at most the chance (Always: weapon, money and ammo pools have
// chance 1). The tried pools spawn their drops at the enemy's drop spots. Draws come from Stream only. Host stand-in, UNVERIFIED.
// Called from UOpenWillowRangeSubsystem::HandleEnemyDied with the enemy's kind name and the level it was set up at.
FOpenWillowRangeLootResult RollRangeLoot(UWorld* World, AOpenWillowEnemy* Enemy, const FString& KindName,
    const FOpenWillowRangeKind& Kind, int32 Level, FRandomStream& Stream, EOpenWillowRangeLootMode Mode);
