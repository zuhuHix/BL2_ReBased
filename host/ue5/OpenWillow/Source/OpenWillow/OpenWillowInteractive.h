// AI-assisted (Claude). Lootables and chests (PR 3, the interactives slice): the loot tables, the placed lootable actor and
// the world subsystem that spawns them from the ignored local/ interactives.json (tools/prepare_interactives.py, schema
// openwillow-interactives/1). Loot pickups (cash, ammo, eridium, health) live in OpenWillowInteractivePickup.h. Game-derived
// values come only from that file at run time; nothing here is game data. Rules marked UNVERIFIED are host choices or
// stand-ins, not behaviour confirmed in the running game.
#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "Math/RandomStream.h"
#include "Subsystems/WorldSubsystem.h"
#include "Engine/TimerHandle.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInteractive.generated.h"

class AOpenWillowWalker;
class AOpenWillowLootPickup;
class FJsonObject;
class USceneComponent;
class USkeletalMeshComponent;
class UStaticMeshComponent;
class UAnimSequence;

// One leaf of an attachment: a weapon balance or an item balance, with its weight (the point's probability is applied
// on the attachment, not here).
struct FOpenWillowLootLeaf
{
    FString Balance;   // game object path (a weapon balance, or an item grade for pickups and gear)
    FString Class;     // WeaponBalanceDefinition, InventoryBalanceDefinition, ...
    float Weight = 0;  // only leaves with a weight above 0 are ever picked
};

// One attachment point: the point rolls its pool with Probability, then gives one leaf by weight.
struct FOpenWillowLootAttachment
{
    FString Point;
    float Probability = 0;   // a missing or invalid probability never fires (UNVERIFIED reading of the manifest)
    TArray<FOpenWillowLootLeaf> Leaves;
};

// One configuration of a balance: picked by weight, then every attachment rolls.
struct FOpenWillowLootConfig
{
    FString Name;
    float Weight = 0;
    TArray<FOpenWillowLootAttachment> Attachments;
};

// The loot tables and the visual of one balance (one kind of lootable). Plain data with the rules, no UObject.
struct FOpenWillowLootTable
{
    TArray<FOpenWillowLootConfig> Configs;
    // Visual, from the balance's first skeletal mesh component (tail names of the package paths; the host imports them under
    // /Game/OpenWillow/Lootables/). Empty = the placeholder box.
    FString SkeletalMesh;
    FString AnimSet;          // tail name of the first anim set; the open anim is <AnimSet>_Open
    float VisualScale = 1.f;

    // Reads one balance object. False with OutError when it has no usable configurations.
    bool Load(const TSharedPtr<FJsonObject>& Balance, FString& OutError);
    // One roll: one configuration by weight, then each attachment fires with its probability and gives one leaf by weight.
    // OutLeaves gets the leaves that dropped in order; OutConfig is the configuration's name ("none" when none has weight).
    void Roll(FRandomStream& Rng, TArray<FOpenWillowLootLeaf>& OutLeaves, FString& OutConfig) const;
    // Index of a weighted pick. Unit (in [0, 1]) is a fraction of the total weight. INDEX_NONE when no weight is above 0.
    static int32 WeightedIndex(const TArray<float>& Weights, float Unit);
};

// The amount rule of a pickup (the pickup block of an items entry). Cash: calc(L) * (BaseScale + PerGradeScale * grade),
// calc(L) = Multiplier * LevelBase^L, rounded down (UNVERIFIED in game; the constants come from the manifest).
struct FOpenWillowPickupSpec
{
    FString Kind;               // ammo, cash, eridium or health
    FString Mesh;               // tail name of the pickup mesh (the host imports it under /Game/OpenWillow/Pickups/)
    bool bAutoPickup = false;   // collected when the walker is close (the tick), else by the use key
    int32 Amount = 0;           // ammo rounds for the pool, eridium count
    FString AmmoType;           // the manifest's ammo kind: pistol, smg, ar, shotgun, sniper, launcher, grenade
    float HealFraction = 0;     // health: the fraction of the maximum health the vial gives
    float Multiplier = 0;       // cash constants
    float LevelBase = 0;
    float BaseScale = 0;
    float PerGradeScale = 0;
    int32 GradeMin = 0;         // the slot grade is a uniform whole number in [GradeMin, GradeMax]
    int32 GradeMax = 0;

    // Cash value of an item at level Level and slot grade Grade. 0 when the constants are not set.
    int32 CashAmount(int32 Level, int32 Grade) const;
};

// One leaf balance from the manifest's items block: a pickup, or a shield / grenade mod with its value formula.
struct FOpenWillowLootItem
{
    FString Balance;
    FString Kind;               // shield, grenade_mod, ammo, usable, health, ...
    FString Name;
    bool bHasValue = false;     // gear: the value formula is present
    float ValuePartTotal = 0;   // gear value = trunc(part_total * scaler^L * base_cost) (UNVERIFIED in game)
    float ValueScaler = 0;
    float ValueBaseCost = 0;
    FOpenWillowPickupSpec Pickup;

    bool IsPickup() const { return !Pickup.Kind.IsEmpty(); }
    bool IsGear() const { return bHasValue && (Kind == TEXT("shield") || Kind == TEXT("grenade_mod")); }
    // The gear's stand-in value at level Level; -1 when there is no formula or the result is not an int32.
    int32 GearValueAt(int32 Level) const;
};

// Plain rules for the pickups and the stock of a walker's pools. No UObject, so the automation test drives them directly.
namespace OpenWillowInteractiveRules
{
    // Money and eridium never wrap: sums clamp to [0, MAX_int32].
    int32 SaturatingAdd(int32 A, int32 B);
    // Ammo: refused when the pool is full (false), else the pool takes min(Max, Current + Amount).
    bool AmmoAfter(int32 Current, int32 Max, int32 Amount, int32& OutAfter);
    // Health: refused while down or at full health. Else min(MaxHealth, Health + Fraction * MaxHealth).
    bool HealAfter(float Health, float MaxHealth, float Fraction, bool bDown, float& OutAfter);
    // The manifest's ammo kind as the inventory's pool type. False for an unknown name.
    bool AmmoTypeFromName(const FString& Name, EOpenWillowAmmoType& OutType);
}

// The placed lootable: a chest, a box or a crate from the manifest. The body is the engine cube stand-in; the imported
// skeletal mesh replaces it when it loads (UNVERIFIED: the assets are not imported yet). Opens once per session.
UCLASS()
class OPENWILLOW_API AOpenWillowLootable : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowLootable();
    void Configure(const FString& InId, const FString& InBalance, const FString& InKind, int32 InStageMin, int32 InStageMax,
        const FOpenWillowLootTable& Visual);
    // Plays the open anim (or lifts the placeholder lid). False when it was open already.
    bool Open();
    bool IsOpened() const { return bOpened; }
    // True when the imported open clip loaded (the lid-opening capture of -owinteractivetest uses these).
    bool HasOpenClip() const { return OpenAnim != nullptr; }

    FString Id;
    FString Balance;
    FString Kind;
    int32 StageMin = 1;
    int32 StageMax = 1;

private:
    void ApplyVisual(const FOpenWillowLootTable& Visual);

    UPROPERTY() TObjectPtr<USceneComponent> Root;
    UPROPERTY() TObjectPtr<UStaticMeshComponent> Body;
    UPROPERTY() TObjectPtr<USceneComponent> LidPivot;   // the hinge at the back edge of the placeholder lid
    UPROPERTY() TObjectPtr<UStaticMeshComponent> Lid;
    UPROPERTY() TObjectPtr<USkeletalMeshComponent> Skeletal;
    UPROPERTY() TObjectPtr<UAnimSequence> OpenAnim;
    bool bSkeletalShown = false;
    bool bOpened = false;
};

// Spawns the lootables of -owinteractives=<interactives.json> when the world begins; runs the nearest-use rule for
// lootables and loot pickups; collects auto pickups by a tick; runs the in-engine test (-owinteractivetest).
// Without the switch the subsystem does nothing.
UCLASS()
class OPENWILLOW_API UOpenWillowInteractiveSubsystem : public UWorldSubsystem
{
    GENERATED_BODY()
public:
    virtual void OnWorldBeginPlay(UWorld& InWorld) override;
    virtual void Deinitialize() override;

    // Called from AOpenWillowWalker::PickupNearby. True when the press was used; false lets the other targets go on.
    static bool TryUseNearest(AOpenWillowWalker& Walker);
    // The reach test of TryUseNearest on its own: the squared distance to the nearest closed lootable or use-key pickup in
    // reach. The walker compares it with its other use targets.
    static bool NearestInReach(const AOpenWillowWalker& Walker, double& OutDistanceSquared);

    // Opens a closed lootable (its drops are spawned around it). False when it is open already or nothing can be rolled.
    bool OpenLootable(AOpenWillowWalker& Walker, AOpenWillowLootable& Lootable);
    int32 LootableCount() const { return Lootables.Num(); }
    AOpenWillowLootable* LootableAt(int32 Index) const { return Lootables.IsValidIndex(Index) ? Lootables[Index].Get() : nullptr; }

private:
    static UOpenWillowInteractiveSubsystem* SubsystemOf(UWorld* World);
    // The nearest closed lootable and the nearest use-key pickup in reach, with the squared distance of whichever is nearer.
    static bool FindNearest(UWorld* World, const FVector& From, AOpenWillowLootable*& OutLootable,
        AOpenWillowLootPickup*& OutPickup, double& OutDistanceSquared);
    bool LoadManifest(const FString& Path, TSharedPtr<FJsonObject>& OutRoot);
    void ReadItems(const TSharedPtr<FJsonObject>& Root);
    void ReadBalances(const TSharedPtr<FJsonObject>& Root);
    void SpawnObjects(UWorld& World, const TSharedPtr<FJsonObject>& Root);
    bool SpawnOne(UWorld& World, const TSharedPtr<FJsonObject>& Json);
    void SpawnLeaf(AOpenWillowWalker& Walker, const AOpenWillowLootable& Lootable, const FOpenWillowLootLeaf& Leaf,
        int32 Level, int32 Index, int32 Count, FRandomStream& Rng, int32& Spawned);
    void SpawnWeapon(UWorld& World, const AOpenWillowLootable& Lootable, const FString& Balance, int32 Level, uint32 GunSeed,
        const FVector& Slot, int32& Spawned);
    void SpawnGear(UWorld& World, const AOpenWillowLootable& Lootable, const FOpenWillowLootItem& Item, int32 Level,
        const FVector& Slot, int32& Spawned);
    void SpawnPickup(UWorld& World, const AOpenWillowLootable& Lootable, const FOpenWillowLootItem& Item, int32 Level,
        FRandomStream& Rng, const FVector& Slot, int32& Spawned);
    void AutoCollectTick();
    void TickInteractiveTest();
    void TestOpen(AOpenWillowWalker& Walker);
    // The lid capture: one lootable per balance with an imported clip, Maya in front of it, frames closed, mid-open and
    // opened (OWLootLid_<balance leaf>_<phase>.png). True while it is still working.
    bool TickLidShots(AOpenWillowWalker& Walker);
    TArray<TWeakObjectPtr<AOpenWillowLootable>> LidQueue;
    bool bLidQueueBuilt = false;
    int32 LidPhase = 0;
    FTimerHandle LidMidTimer;

    TMap<FString, FOpenWillowLootTable> Tables;       // by balance path
    TMap<FString, FOpenWillowLootItem> Items;         // by leaf balance path
    TArray<TWeakObjectPtr<AOpenWillowLootable>> Lootables;  // in manifest order
    uint32 OpenSerial = 0;            // counts the opens; part of each open's seed
    FTimerHandle AutoTimer;
    FTimerHandle TestTimer;
    int32 TestStep = 0;
    int32 TestWaits = 0;
    bool bOn = false;                 // -owinteractives= was given and the manifest loaded
};
