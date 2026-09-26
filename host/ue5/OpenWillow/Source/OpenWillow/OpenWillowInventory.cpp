#include "OpenWillowInventory.h"
#include "Dom/JsonObject.h"
#include "HAL/FileManager.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

namespace
{
float Number(const TSharedPtr<FJsonObject>& Card, const TCHAR* Field, float Fallback)
{
    double Value = Fallback;
    return Card->TryGetNumberField(Field, Value) ? float(Value) : Fallback;
}
}

int32 UOpenWillowInventory::LoadRecipes(const FString& Directory)
{
    TArray<FString> Files;
    IFileManager::Get().FindFiles(Files, *FPaths::Combine(Directory, TEXT("*.json")), true, false);
    Files.Sort();
    for (const FString& File : Files)
    {
        FString Text;
        if (!FFileHelper::LoadFileToString(Text, *FPaths::Combine(Directory, File))) continue;
        TSharedPtr<FJsonObject> Recipe;
        if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Recipe) || !Recipe) continue;
        const TSharedPtr<FJsonObject>* Stats = nullptr;
        const TSharedPtr<FJsonObject>* Card = nullptr;
        // Only recipes that tools/weapon_stats.py has evaluated are items.
        if (!Recipe->TryGetObjectField(TEXT("stats"), Stats) || !(*Stats)->TryGetObjectField(TEXT("card"), Card))
            continue;
        FOpenWillowWeaponItem Item;
        Item.Id = FPaths::GetBaseFilename(File);
        Item.Name = Recipe->GetStringField(TEXT("name"));
        Item.Balance = Recipe->GetStringField(TEXT("balance"));
        (*Card)->TryGetStringField(TEXT("manufacturer"), Item.Manufacturer);
        (*Card)->TryGetStringField(TEXT("element"), Item.Element);
        Item.Rarity = int32(Number(*Card, TEXT("rarity"), 1));
        Item.Level = int32(Number(*Card, TEXT("level"), 1));
        Item.Damage = Number(*Card, TEXT("damage"), 0);
        Item.FireRate = FMath::Max(0.1f, Number(*Card, TEXT("fire_rate"), 1));
        Item.ReloadTime = Number(*Card, TEXT("reload_time"), 0);
        Item.Magazine = Number(*Card, TEXT("magazine"), 0);
        Item.Spread = Number(*Card, TEXT("spread"), 0);
        Item.ShotCost = Number(*Card, TEXT("shot_cost"), 1);
        Item.SpinUp = Number(*Card, TEXT("spin_up"), 0);
        Recipe->TryGetStringArrayField(TEXT("gestalt_fragments"), Item.Fragments);
        Backpack.Add(MoveTemp(Item));
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow inventory loaded %d weapon recipe(s) from %s"), Backpack.Num(), *Directory);
    return Backpack.Num();
}

bool UOpenWillowInventory::Equip(int32 Item, int32 Slot)
{
    if (!Backpack.IsValidIndex(Item) || Slot < 0 || Slot >= SlotCount) return false;
    // An item occupies at most one slot.
    for (int32& Held : Slots) if (Held == Item) Held = INDEX_NONE;
    Slots[Slot] = Item;
    return true;
}

const FOpenWillowWeaponItem* UOpenWillowInventory::SlotItem(int32 Slot) const
{
    if (Slot < 0 || Slot >= SlotCount || !Backpack.IsValidIndex(Slots[Slot])) return nullptr;
    return &Backpack[Slots[Slot]];
}

const FOpenWillowWeaponItem* UOpenWillowInventory::ActiveWeapon() const
{
    return SlotItem(ActiveSlot);
}
