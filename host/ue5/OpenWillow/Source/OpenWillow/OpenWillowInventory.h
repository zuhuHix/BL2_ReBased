#pragma once

#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "OpenWillowInventory.generated.h"

// One rolled weapon, loaded from a tools/weapon_recipe.py recipe that
// tools/weapon_stats.py has filled in. Values are the evaluated item-card
// stats; see DECISIONS.md for which combination rules are UNVERIFIED.
USTRUCT()
struct FOpenWillowWeaponItem
{
    GENERATED_BODY()
    FString Id;            // recipe file stem, also the imported mesh name
    FString Name;          // prefix + title, e.g. "Burning Infinity"
    FString Manufacturer;
    FString Element;       // "None", "Fire", "Shock", ...
    FString Balance;
    int32 Rarity = 1;      // 1 Common .. 5 Legendary
    int32 Level = 1;
    float Damage = 0;
    float FireRate = 1;    // shots per second
    float ReloadTime = 0;
    float Magazine = 0;
    float Spread = 0;
    float ShotCost = 1;    // 0 means firing costs no ammo (Infinity)
    float SpinUp = 0;      // seconds; 0 when the barrel does not spin
    TArray<FString> Fragments;
};

// Backpack plus BL2's four weapon slots. Holds data only; the walker asks it
// for the active weapon and the HUD/inventory screens read it.
UCLASS()
class OPENWILLOW_API UOpenWillowInventory : public UActorComponent
{
    GENERATED_BODY()
public:
    static constexpr int32 SlotCount = 4;
    // Loads every *.json recipe with stats under Directory; returns the count.
    int32 LoadRecipes(const FString& Directory);
    // Puts backpack item Item into weapon slot Slot (0-3).
    bool Equip(int32 Item, int32 Slot);
    // Selects the slot to hold; an empty slot or -1 means unarmed.
    void SetActiveSlot(int32 Slot) { ActiveSlot = Slot; }
    int32 GetActiveSlot() const { return ActiveSlot; }
    const FOpenWillowWeaponItem* ActiveWeapon() const;
    const FOpenWillowWeaponItem* SlotItem(int32 Slot) const;
    const TArray<FOpenWillowWeaponItem>& Items() const { return Backpack; }
private:
    TArray<FOpenWillowWeaponItem> Backpack;
    int32 Slots[SlotCount] = {INDEX_NONE, INDEX_NONE, INDEX_NONE, INDEX_NONE};
    int32 ActiveSlot = INDEX_NONE;
};
