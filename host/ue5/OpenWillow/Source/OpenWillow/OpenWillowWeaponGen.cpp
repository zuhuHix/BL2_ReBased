#include "OpenWillowWeaponGen.h"
#include "OpenWillowWalker.h"
#include "Dom/JsonObject.h"
#include "Dom/JsonValue.h"
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "HAL/FileManager.h"
#include "HAL/IConsoleManager.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "Misc/SecureHash.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"
#include "weapon_gen.hpp"
#include <memory>
#include <optional>
#include <string>
#include <vector>

namespace
{
// The item-card type label of each weapon type package, the same names ResolveAmmoType matches. The manifest carries
// the WeaponTypeDefinition path but not its Typename, so the label comes from the package prefix (UNVERIFIED for
// the types that are not in ResolveAmmoType).
struct FTypePrefix { const TCHAR* Prefix; const TCHAR* Label; };
constexpr FTypePrefix TypePrefixes[] =
{
    {TEXT("GD_Weap_Pistol"), TEXT("Pistol")},
    {TEXT("GD_Weap_SMG"), TEXT("SMG")},
    {TEXT("GD_Weap_AssaultRifle"), TEXT("Assault Rifle")},
    {TEXT("GD_Weap_Shotgun"), TEXT("Shotgun")},
    {TEXT("GD_Weap_SniperRifles"), TEXT("Sniper Rifle")},
    {TEXT("GD_Weap_Launchers"), TEXT("Launcher")},
};

FString FromUtf8(const std::string& Text)
{
    return FString(UTF8_TO_TCHAR(Text.c_str()));
}

FString FromUtf8(const std::optional<std::string>& Text)
{
    return Text ? FromUtf8(*Text) : FString();
}

std::string ToUtf8(const FString& Text)
{
    return std::string(TCHAR_TO_UTF8(*Text));
}

// Same test as ow-weapongen's IsWeaponBalance (src/weapon_gen_cli.cpp).
bool IsWeaponBalanceUtf8(const weapongen::WeaponGenManifest& Manifest, const std::string& Path)
{
    const auto Object = Manifest.objectByPath.find(Path);
    return Object != Manifest.objectByPath.end() && Manifest.balanceByObject.count(std::to_string(Object->second)) > 0;
}

constexpr int32 FiringTemplatesSchema = 2;   // tools/weapon_gen_firing.py FILE_SCHEMA

// tools/weapon_gen_firing.py's templates, next to the manifest, parsed once. Null when the file is missing or unreadable, older than
// this code, or made with another manifest: a matching key alone would silently supply old values.
const FJsonObject* FiringTemplates()
{
    static bool bTried = false;
    static TSharedPtr<FJsonObject> Root;
    if (bTried) return Root.Get();
    bTried = true;
    const FString Path = FPaths::Combine(FPaths::GetPath(FOpenWillowWeaponGen::ManifestPath()), TEXT("weapon_gen_firing.json"));
    FString Text;
    if (!FFileHelper::LoadFileToString(Text, *Path) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root.IsValid())
    {
        Root.Reset();
        UE_LOG(LogTemp, Warning, TEXT("OWWEAPONGEN no firing templates at %s: rolled guns fire a plain trace (python tools/weapon_gen_firing.py --game <BL2 folder>)"), *Path);
        return nullptr;
    }
    double Schema = 0;
    FString Stored;
    Root->TryGetNumberField(TEXT("schema"), Schema);
    Root->TryGetStringField(TEXT("manifest_md5"), Stored);
    const FString Current = LexToString(FMD5Hash::HashFile(*FOpenWillowWeaponGen::ManifestPath()));
    if (int32(Schema) != FiringTemplatesSchema || Stored.IsEmpty() || !Stored.Equals(Current, ESearchCase::IgnoreCase))
    {
        UE_LOG(LogTemp, Warning, TEXT("OWWEAPONGEN firing templates at %s refused (schema %d, want %d; manifest %s, made with %s): rolled guns fire a plain trace (rerun python tools/weapon_gen_firing.py --game <BL2 folder>)"),
            *Path, int32(Schema), FiringTemplatesSchema, *Current, *Stored);
        Root.Reset();
    }
    return Root.Get();
}

// The rolled gun's firing block: its template (weapon type, firing mode, element) filled from the card, then the parser recipes use.
// A gun with no template, or one that cannot fire as its kind, keeps the plain trace and its refire rule.
void ApplyFiringTemplate(const weapongen::WeaponGenResult& Result, const weapongen::WeaponGenCard& Card, FOpenWillowWeaponItem& Item)
{
    const FJsonObject* Root = FiringTemplates();
    const TSharedPtr<FJsonObject>* Templates = nullptr;
    if (!Root || !Root->TryGetObjectField(TEXT("templates"), Templates)) return;
    const FString Key = FromUtf8(Result.weaponType) + TEXT("|") + FromUtf8(Card.firingMode) + TEXT("|") + FromUtf8(Card.element);
    const TSharedPtr<FJsonObject>* Template = nullptr;
    if (!(*Templates)->TryGetObjectField(Key, Template))
    {
        UE_LOG(LogTemp, Warning, TEXT("OWWEAPONGEN no firing template for %s"), *Key);
        return;
    }
    const TArray<TSharedPtr<FJsonValue>>* Scales = nullptr;
    Root->TryGetArrayField(TEXT("level_chance_scales"), Scales);
    FString Missing;
    if (!FOpenWillowWeaponGen::MergeFiringTemplate(**Template, Scales, Card, Item, Missing))
        UE_LOG(LogTemp, Warning, TEXT("OWWEAPONGEN firing template %s refused: %s (python tools/weapon_gen_firing.py --game <BL2 folder>)"), *Key, *Missing);
}

// The manifest is parsed once, on the game thread, the first time something asks for it.
const weapongen::WeaponGenManifest* Manifest()
{
    static bool bTried = false;
    static std::unique_ptr<weapongen::WeaponGenManifest> Loaded;
    if (bTried) return Loaded.get();
    bTried = true;
    const FString Path = FOpenWillowWeaponGen::ManifestPath();
    if (!FPaths::FileExists(Path))
    {
        UE_LOG(LogTemp, Display, TEXT("OWWEAPONGEN off: no manifest at %s (python tools/weapon_gen_manifest.py --game <BL2 folder>)"), *Path);
        return nullptr;
    }
    std::string Error;
    weapongen::WeaponGenManifest Parsed = weapongen::LoadWeaponGenManifest(ToUtf8(Path), &Error);
    if (!Error.empty())
    {
        UE_LOG(LogTemp, Warning, TEXT("OWWEAPONGEN off: %s does not parse: %s"), *Path, *FromUtf8(Error));
        return nullptr;
    }
    Loaded = std::make_unique<weapongen::WeaponGenManifest>(std::move(Parsed));
    UE_LOG(LogTemp, Display, TEXT("OWWEAPONGEN manifest %s: %d balances, %d objects (generator rules UNVERIFIED in game)"),
        *Path, int32(Loaded->balances.size()), int32(Loaded->objects.size()));
    return Loaded.get();
}

// WeaponGenResult -> item. Field by field the same as UOpenWillowInventory::ReadRecipe reads a recipe's stats.card
// (and the recipe's top-level name, balance, type, material, gestalt, fragments and parts), with the same defaults.
FOpenWillowWeaponItem ToItem(const weapongen::WeaponGenResult& Result)
{
    const weapongen::WeaponGenCard& Card = Result.card;
    FOpenWillowWeaponItem Item;
    FString Leaf = FromUtf8(Result.balance);
    int32 Dot = INDEX_NONE;
    if (Leaf.FindLastChar('.', Dot)) Leaf = Leaf.Mid(Dot + 1);
    // Stable: the same balance, stage and seed always give the same Id, so the same roll is the same recipe.
    Item.Id = FString::Printf(TEXT("gen_%s_%d_%u"), *Leaf, Result.stage, Result.seed);
    Item.InstanceId = Item.Id;
    Item.Name = FromUtf8(Result.name);
    Item.Balance = FromUtf8(Result.balance);
    const FString WeaponType = FromUtf8(Result.weaponType);
    for (const FTypePrefix& Type : TypePrefixes)
    {
        if (WeaponType.StartsWith(Type.Prefix))
        {
            Item.Type = Type.Label;
            break;
        }
    }
    Item.Manufacturer = FromUtf8(Card.manufacturer);
    Item.Element = FromUtf8(Card.element);
    Item.DamageType = FromUtf8(Card.damageType);
    Item.Rarity = Card.rarity.value_or(1);
    Item.RarityColor = FromUtf8(Card.rarityColor);
    Item.Level = Card.level;
    Item.Damage = float(Card.damage.value_or(0));
    if (Card.damageStack)
    {
        // The terms later modifiers join (AOpenWillowWalker::ShotDamage), as a recipe's card has them.
        Item.bHasDamageStack = true;
        Item.DamageBase = float(Card.damageStack->base);
        Item.DamagePreAdd = float(Card.damageStack->preAdd);
        Item.DamagePostAdd = float(Card.damageStack->postAdd);
        Item.DamageScaleUp = float(Card.damageStack->scaleUp);
        Item.DamageScaleDown = float(Card.damageStack->scaleDown);
    }
    Item.FireRate = FMath::Max(0.1f, float(Card.fireRate.value_or(1)));
    Item.ReloadTime = float(Card.reloadTime.value_or(0));
    Item.Magazine = float(Card.magazine.value_or(0));
    Item.Spread = float(Card.spread.value_or(0));
    Item.ShotCost = float(Card.shotCost.value_or(1));
    // The refire rule (weapon_firing::Burst), known from the manifest alone; ApplyFiringTemplate adds the rest of the block.
    Item.Firing.BurstCount = FMath::Max(0, int32(Card.burstCount.value_or(0)));
    Item.Firing.BurstInterval = float(Card.burstInterval.value_or(0));
    Item.Firing.bBurstFromManifest = Card.BurstKnown();
    if (!Item.Firing.bBurstFromManifest)
    {
        static bool bWarned = false;
        if (!bWarned) UE_LOG(LogTemp, Warning, TEXT("OWWEAPONGEN the manifest has no burst data: rolled guns fire while held (rerun tools/weapon_gen_manifest.py)"));
        bWarned = true;
    }
    Item.SpinUp = float(Card.spinUp.value_or(0));
    Item.SpinMode = FromUtf8(Card.spinMode);
    Item.SpinStartIntervalScale = float(Card.spinStartIntervalScale.value_or(1));
    if (Card.accuracy && FMath::IsFinite(*Card.accuracy))
    {
        Item.Accuracy = float(FMath::Clamp(*Card.accuracy, 0.0, 100.0));
        Item.bHasAccuracy = true;
        Item.bAccuracyKnown = Card.accuracyKnown;
    }
    if (Card.saleValue && *Card.saleValue >= 0 && *Card.saleValue <= MAX_int32)
    {
        Item.SaleValue = int32(*Card.saleValue);
        Item.bHasSaleValue = true;
        Item.bSaleValueKnown = Card.saleValueKnown;
    }
    Item.FunStats = FromUtf8(Card.funStats);
    Item.FunStatsMarkup = FromUtf8(Card.funStatsMarkup);
    if (Card.projectiles && *Card.projectiles >= 1 && *Card.projectiles <= 1000) Item.Projectiles = *Card.projectiles;
    if (Card.hasStatus && Card.statusDps && Card.statusChance && *Card.statusDps >= 0 && *Card.statusChance >= 0)
    {
        Item.bHasStatus = true;
        Item.StatusDps = float(*Card.statusDps);
        Item.StatusChance = float(*Card.statusChance);
    }
    for (const std::string& Fragment : Result.fragments) Item.Fragments.Add(FromUtf8(Fragment));
    for (const weapongen::WeaponGenPart& Part : Result.parts)
    {
        FOpenWillowWeaponPart& Entry = Item.Parts.AddDefaulted_GetRef();
        Entry.Slot = FromUtf8(Part.slot);
        Entry.Part = FromUtf8(Part.part);
    }
    Item.Material = FromUtf8(Result.material);
    Item.GestaltMesh = FromUtf8(Result.gestalt);
    Item.WeaponType = FromUtf8(Result.weaponType);
    ApplyFiringTemplate(Result, Card, Item);
    return Item;
}

// The converted item as JSON, with the keys ow-weapongen's card uses where they match (the comparison in
// local/weapons/gen/host_check reads both).
TSharedPtr<FJsonObject> ItemJson(const FOpenWillowWeaponItem& Item)
{
    TSharedPtr<FJsonObject> Json = MakeShared<FJsonObject>();
    Json->SetStringField(TEXT("id"), Item.Id);
    Json->SetStringField(TEXT("instance_id"), Item.InstanceId);
    Json->SetStringField(TEXT("name"), Item.Name);
    Json->SetStringField(TEXT("type"), Item.Type);
    Json->SetStringField(TEXT("manufacturer"), Item.Manufacturer);
    Json->SetStringField(TEXT("element"), Item.Element);
    Json->SetStringField(TEXT("damage_type"), Item.DamageType);
    Json->SetStringField(TEXT("balance"), Item.Balance);
    Json->SetNumberField(TEXT("rarity"), Item.Rarity);
    Json->SetStringField(TEXT("rarity_color"), Item.RarityColor);
    Json->SetNumberField(TEXT("level"), Item.Level);
    Json->SetNumberField(TEXT("damage"), Item.Damage);
    Json->SetNumberField(TEXT("fire_rate"), Item.FireRate);
    Json->SetNumberField(TEXT("reload_time"), Item.ReloadTime);
    Json->SetNumberField(TEXT("magazine"), Item.Magazine);
    Json->SetNumberField(TEXT("spread"), Item.Spread);
    Json->SetNumberField(TEXT("shot_cost"), Item.ShotCost);
    Json->SetNumberField(TEXT("burst_count"), Item.Firing.BurstCount);
    Json->SetNumberField(TEXT("burst_interval"), Item.Firing.BurstInterval);
    Json->SetNumberField(TEXT("spin_up"), Item.SpinUp);
    Json->SetStringField(TEXT("spin_mode"), Item.SpinMode);
    Json->SetNumberField(TEXT("spin_start_interval_scale"), Item.SpinStartIntervalScale);
    Json->SetBoolField(TEXT("has_accuracy"), Item.bHasAccuracy);
    Json->SetNumberField(TEXT("accuracy"), Item.Accuracy);
    Json->SetBoolField(TEXT("accuracy_known"), Item.bAccuracyKnown);
    Json->SetBoolField(TEXT("has_sale_value"), Item.bHasSaleValue);
    Json->SetNumberField(TEXT("sale_value"), Item.SaleValue);
    Json->SetBoolField(TEXT("sale_value_known"), Item.bSaleValueKnown);
    Json->SetStringField(TEXT("fun_stats"), Item.FunStats);
    Json->SetStringField(TEXT("fun_stats_markup"), Item.FunStatsMarkup);
    Json->SetNumberField(TEXT("projectiles"), Item.Projectiles);
    Json->SetBoolField(TEXT("has_status"), Item.bHasStatus);
    Json->SetNumberField(TEXT("status_dps"), Item.StatusDps);
    Json->SetNumberField(TEXT("status_chance"), Item.StatusChance);
    TArray<TSharedPtr<FJsonValue>> Fragments;
    for (const FString& Fragment : Item.Fragments) Fragments.Add(MakeShared<FJsonValueString>(Fragment));
    Json->SetArrayField(TEXT("gestalt_fragments"), Fragments);
    TArray<TSharedPtr<FJsonValue>> Parts;
    for (const FOpenWillowWeaponPart& Part : Item.Parts)
    {
        TSharedPtr<FJsonObject> Entry = MakeShared<FJsonObject>();
        Entry->SetStringField(TEXT("slot"), Part.Slot);
        Entry->SetStringField(TEXT("part"), Part.Part);
        Parts.Add(MakeShared<FJsonValueObject>(Entry));
    }
    Json->SetArrayField(TEXT("parts"), Parts);
    Json->SetStringField(TEXT("material"), Item.Material);
    Json->SetStringField(TEXT("gestalt"), Item.GestaltMesh);
    Json->SetStringField(TEXT("weapon_type"), Item.WeaponType);
    return Json;
}

// The backpack of the first player's walker in a game world, or null (no walker: not started with -owwalk).
UOpenWillowInventory* PlayerInventory()
{
    if (!GEngine) return nullptr;
    for (const FWorldContext& Context : GEngine->GetWorldContexts())
    {
        UWorld* World = Context.World();
        if (!World || !World->IsGameWorld()) continue;
        const APlayerController* Controller = World->GetFirstPlayerController();
        AOpenWillowWalker* Walker = Controller ? Cast<AOpenWillowWalker>(Controller->GetPawn()) : nullptr;
        if (Walker && Walker->GetInventory()) return Walker->GetInventory();
    }
    return nullptr;
}

// A whole number written with digits only (no sign, no decimals) and at most Max; false for anything else.
bool ParseWholeNumber(const FString& Text, int64 Max, int64& Out)
{
    if (Text.IsEmpty() || Text.Len() > 10) return false;
    for (const TCHAR Char : Text)
    {
        if (!FChar::IsDigit(Char)) return false;
    }
    Out = FCString::Atoi64(*Text);
    return Out <= Max;
}

// ow.RollGun <balance-or-pool> [stage=1] [seed=1]: rolls one gun and adds it to the backpack. A balance rolls that gun;
// a pool or source rolls it and adds the first weapon it drops (the same gun as ow-weapongen --pool, seed + drop index).
void RollGunCommand(const TArray<FString>& Args)
{
    const TCHAR* Usage = TEXT("usage: ow.RollGun <weapon balance or pool/source path> [stage 1..80, default 1] [seed 0..4294967295, default 1]");
    if (Args.Num() < 1 || Args.Num() > 3)
    {
        UE_LOG(LogTemp, Display, TEXT("%s"), Usage);
        return;
    }
    int64 StageValue = 1;
    int64 SeedValue = 1;
    if (Args.Num() > 1 && (!ParseWholeNumber(Args[1], 80, StageValue) || StageValue < 1))
    {
        UE_LOG(LogTemp, Warning, TEXT("ow.RollGun: bad stage '%s'. %s"), *Args[1], Usage);
        return;
    }
    if (Args.Num() > 2 && !ParseWholeNumber(Args[2], MAX_uint32, SeedValue))
    {
        UE_LOG(LogTemp, Warning, TEXT("ow.RollGun: bad seed '%s'. %s"), *Args[2], Usage);
        return;
    }
    const int32 Stage = int32(StageValue);
    const uint32 Seed = uint32(SeedValue);
    if (!FOpenWillowWeaponGen::IsAvailable())
    {
        UE_LOG(LogTemp, Warning, TEXT("ow.RollGun: no weapon generator manifest (tools/weapon_gen_manifest.py)"));
        return;
    }
    FOpenWillowWeaponItem Item;
    bool bRolled = false;
    if (FOpenWillowWeaponGen::IsWeaponBalance(Args[0]))
    {
        bRolled = FOpenWillowWeaponGen::RollBalance(Args[0], Stage, Seed, Item);
    }
    else
    {
        TArray<FString> Drops;
        bRolled = FOpenWillowWeaponGen::RollPoolWeapon(Args[0], Stage, Seed, Item, &Drops);
        UE_LOG(LogTemp, Display, TEXT("ow.RollGun %s stage %d seed %u dropped %d item(s): %s"), *Args[0], Stage, Seed, Drops.Num(),
            *FString::Join(Drops, TEXT(", ")));
    }
    if (!bRolled)
    {
        UE_LOG(LogTemp, Warning, TEXT("ow.RollGun %s stage %d seed %u: no weapon rolled"), *Args[0], Stage, Seed);
        return;
    }
    UOpenWillowInventory* Inventory = PlayerInventory();
    if (!Inventory)
    {
        UE_LOG(LogTemp, Warning, TEXT("ow.RollGun: no walker in a game world (start with -owwalk); rolled %s \"%s\" not added"), *Item.Id, *Item.Name);
        return;
    }
    if (!Inventory->AddToBackpack(Item))
    {
        UE_LOG(LogTemp, Warning, TEXT("ow.RollGun: backpack full (%d); rolled %s \"%s\" not added"), Inventory->BackpackCount(), *Item.Id, *Item.Name);
        return;
    }
    UE_LOG(LogTemp, Display, TEXT("ow.RollGun added %s \"%s\" (%s, level %d, %s) to the backpack"), *Item.InstanceId, *Item.Name,
        *Item.Type, Item.Level, *Item.Balance);
}

FAutoConsoleCommand GRollGunCommand(TEXT("ow.RollGun"),
    TEXT("ow.RollGun <weapon balance or pool path> [stage=1] [seed=1]: roll a gun from the weapon generator into the backpack"),
    FConsoleCommandWithArgsDelegate::CreateStatic(&RollGunCommand));
}

bool FOpenWillowWeaponGen::MergeFiringTemplate(const FJsonObject& Template, const TArray<TSharedPtr<FJsonValue>>* LevelScales,
    const weapongen::WeaponGenCard& Card, FOpenWillowWeaponItem& Item, FString& OutMissing)
{
    // A shallow copy: only top-level fields change, and the parser only reads.
    const TSharedRef<FJsonObject> Block = MakeShared<FJsonObject>();
    Block->Values = Template.Values;
    Block->SetNumberField(TEXT("pellets"), Item.Projectiles);
    Block->SetNumberField(TEXT("spread"), Item.Spread);
    Block->SetNumberField(TEXT("burst_count"), Item.Firing.BurstCount);
    Block->SetNumberField(TEXT("burst_interval"), Item.Firing.BurstInterval);
    Block->SetNumberField(TEXT("spin_up"), Item.SpinUp);
    if (Card.hasStatus && Card.statusEffectDefinition)
    {
        const TSharedRef<FJsonObject> Status = MakeShared<FJsonObject>();
        Status->SetStringField(TEXT("definition"), FromUtf8(Card.statusEffectDefinition));
        Status->SetStringField(TEXT("type"), FromUtf8(Card.statusEffect));
        Status->SetNumberField(TEXT("duration"), Card.statusDuration.value_or(0));
        Status->SetNumberField(TEXT("dps"), Card.statusDps.value_or(0));
        const TSharedRef<FJsonObject> Surfaces = MakeShared<FJsonObject>();
        for (const auto& [Surface, Chance] : Card.statusChanceBySurface) Surfaces->SetNumberField(FromUtf8(Surface), Chance);
        Status->SetObjectField(TEXT("chance_by_surface"), Surfaces);
        if (LevelScales) Status->SetArrayField(TEXT("level_chance_scales"), *LevelScales);
        Block->SetObjectField(TEXT("status"), Status);
    }
    const bool bBurstKnown = Item.Firing.bBurstFromManifest;
    FOpenWillowFiringData Parsed;
    if (!FOpenWillowFiringData::Parse(*Block, Item.Damage, Parsed))
    {
        OutMissing = TEXT("schema");
        return false;
    }
    const float SpeedMultiplier = float(Card.speedMultiplier.value_or(1));
    Parsed.BulletSpeed *= SpeedMultiplier;
    Parsed.ProjectileSpeed *= SpeedMultiplier;
    Parsed.bBurstFromManifest = bBurstKnown;
    Parsed.bSpinsToFullRate = Item.SpinMode == TEXT("BSM_SpinUpToFullFireRate");
    Parsed.SpinStartMultiplier = Item.SpinStartIntervalScale;
    Parsed.SpinUpDuration = Item.SpinUp;
    if (!Parsed.CompleteForShotKind(OutMissing)) return false;
    Item.Firing = MoveTemp(Parsed);
    return true;
}


FString FOpenWillowWeaponGen::ManifestPath()
{
    FString Path = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/weapons/gen/weapon_gen_manifest.txt")));
    FParse::Value(FCommandLine::Get(), TEXT("owweapongen="), Path);
    return Path;
}

bool FOpenWillowWeaponGen::IsAvailable()
{
    return Manifest() != nullptr;
}

bool FOpenWillowWeaponGen::IsWeaponBalance(const FString& Path)
{
    const weapongen::WeaponGenManifest* M = Manifest();
    return M && IsWeaponBalanceUtf8(*M, ToUtf8(Path));
}

bool FOpenWillowWeaponGen::FindGunWithDamageStack(int32 Stage, float Scale, FOpenWillowWeaponItem& OutItem, FString& OutReason)
{
    const weapongen::WeaponGenManifest* M = Manifest();
    if (!M)
    {
        OutReason = TEXT("no_manifest");
        return false;
    }
    bool bAnyStack = false;
    for (const weapongen::GenBalance& Balance : M->balances)
    {
        FOpenWillowWeaponItem Item;
        if (!RollBalance(FromUtf8(M->objects[size_t(Balance.object)].path), Stage, 1u, Item) || !Item.bHasDamageStack || Item.Damage <= 0.f) continue;
        bAnyStack = true;
        const float Stack = (Item.DamageBase + Item.DamagePreAdd) * (1.f + Item.DamageScaleUp + Scale) / (1.f + Item.DamageScaleDown) + Item.DamagePostAdd;
        if (!FMath::IsNearlyEqual(Stack, Item.Damage * (1.f + Scale), 0.01f * Item.Damage))
        {
            OutItem = MoveTemp(Item);
            return true;
        }
    }
    OutReason = bAnyStack ? TEXT("no_gun") : TEXT("no_stack");
    return false;
}

bool FOpenWillowWeaponGen::FindGunForBurstTest(int32 WantedBurst, int32 Stage, FOpenWillowWeaponItem& OutItem, FString& OutReason)
{
    const weapongen::WeaponGenManifest* M = Manifest();
    if (!M)
    {
        OutReason = TEXT("no_manifest");
        return false;
    }
    for (const weapongen::GenBalance& Balance : M->balances)
    {
        FOpenWillowWeaponItem Item;
        if (!RollBalance(FromUtf8(M->objects[size_t(Balance.object)].path), Stage, 1u, Item)) continue;
        if (!Item.Firing.bBurstFromManifest)
        {
            OutReason = TEXT("stale_manifest");
            return false;
        }
        if (!Item.Firing.bLoaded)
        {
            OutReason = TEXT("no_firing_templates");
            return false;
        }
        // BurstTestRocket wants any gun that launches a projectile, whatever its burst count.
        const bool bWanted = WantedBurst == BurstTestRocket ? Item.Firing.Kind == EOpenWillowShotKind::Rocket : Item.Firing.BurstCount == WantedBurst;
        // Three shots in the magazine, so that it is not what stops the shots, and a level Maya can equip.
        if (bWanted && Item.Magazine >= 3.f * FMath::Max(1.f, Item.ShotCost) && Item.Level <= Stage && Item.FireRate > 0.f)
        {
            OutItem = MoveTemp(Item);
            return true;
        }
    }
    OutReason = TEXT("no_gun");
    return false;
}

bool FOpenWillowWeaponGen::RollBalance(const FString& Balance, int32 Stage, uint32 Seed, FOpenWillowWeaponItem& OutItem)
{
    const weapongen::WeaponGenManifest* M = Manifest();
    if (!M) return false;
    const weapongen::WeaponGenResult Result = weapongen::RollWeapon(*M, ToUtf8(Balance), Stage, Seed);
    if (!Result.ok)
    {
        UE_LOG(LogTemp, Warning, TEXT("OWWEAPONGEN %s stage %d seed %u: %s"), *Balance, Stage, Seed, *FromUtf8(Result.error));
        return false;
    }
    OutItem = ToItem(Result);
    return true;
}

bool FOpenWillowWeaponGen::RollPoolWeapon(const FString& Pool, int32 Stage, uint32 Seed, FOpenWillowWeaponItem& OutItem, TArray<FString>* OutDrops,
    int32 StageCap)
{
    const weapongen::WeaponGenManifest* M = Manifest();
    if (!M) return false;
    const std::string Path = ToUtf8(Pool);
    weapongen::CrtRand Rng(Seed);
    const std::vector<std::string> Drops = weapongen::IsPoolSource(*M, Path)
        ? weapongen::RollSource(*M, Path, Stage, Rng) : weapongen::RollPool(*M, Path, Stage, Rng);
    if (OutDrops)
    {
        OutDrops->Reset();
        for (const std::string& Drop : Drops) OutDrops->Add(FromUtf8(Drop));
    }
    // ow-weapongen rolls drop k with seed + k, counting every drop; the first weapon that rolls ok is the gun.
    for (size_t K = 0; K < Drops.size(); ++K)
    {
        if (!IsWeaponBalanceUtf8(*M, Drops[K])) continue;
        const std::optional<int> Cap = StageCap > 0 ? std::optional<int>(StageCap) : std::nullopt;
        const weapongen::WeaponGenResult Result = weapongen::RollWeapon(*M, Drops[K], Stage, Seed + uint32(K), Cap);
        if (!Result.ok) continue;
        OutItem = ToItem(Result);
        return true;
    }
    return false;
}

FString FOpenWillowWeaponGen::RewardSource(const FString& ItemDir)
{
    TArray<FString> Files;
    IFileManager::Get().FindFiles(Files, *FPaths::Combine(ItemDir, TEXT("*.json")), true, false);
    Files.Sort();
    for (const FString& File : Files)
    {
        FString Text;
        TSharedPtr<FJsonObject> Recipe;
        const TSharedPtr<FJsonObject>* Provenance = nullptr;
        FString Kind, Source;
        if (FFileHelper::LoadFileToString(Text, *FPaths::Combine(ItemDir, File))
            && FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Recipe) && Recipe
            && Recipe->TryGetObjectField(TEXT("provenance"), Provenance) && Provenance
            && (*Provenance)->TryGetStringField(TEXT("kind"), Kind) && Kind == TEXT("reward_roll")
            && (*Provenance)->TryGetStringField(TEXT("source"), Source))
            return Source;
    }
    return FString();
}

void FOpenWillowWeaponGen::RunSelfCheck(const FString& RewardSourcePath)
{
    // Seven balances spread evenly over the manifest's list, at mixed stages and seeds. The list comes from the local
    // manifest, so no stock object path is written into the code.
    struct FCase { FString Balance; int32 Stage; uint32 Seed; };
    TArray<FCase> Cases;
    if (const weapongen::WeaponGenManifest* M = Manifest())
    {
        static const int32 Stages[] = {1, 8, 15, 30, 50, 80, 30};
        const int32 Count = int32(M->balances.size());
        for (int32 Index = 0; Index < 7 && Count > 0; ++Index)
        {
            const weapongen::GenBalance& Balance = M->balances[size_t(int64(Index) * Count / 7)];
            Cases.Add({FromUtf8(M->objects[size_t(Balance.object)].path), Stages[Index], uint32(Index + 1)});
        }
    }
    TArray<TSharedPtr<FJsonValue>> Items;
    for (const FCase& Case : Cases)
    {
        FOpenWillowWeaponItem Item;
        if (!RollBalance(Case.Balance, Case.Stage, Case.Seed, Item))
        {
            UE_LOG(LogTemp, Warning, TEXT("OWWEAPONGEN check: %s did not roll"), *Case.Balance);
            continue;
        }
        UE_LOG(LogTemp, Display, TEXT("OWWEAPONGEN check %s stage %d seed %u -> \"%s\" type %s level %d damage %.4f fire %.4f mag %.2f burst %d parts %d"),
            *Case.Balance, Case.Stage, Case.Seed, *Item.Name, *Item.Type, Item.Level, Item.Damage, Item.FireRate, Item.Magazine, Item.Firing.BurstCount, Item.Parts.Num());
        Items.Add(MakeShared<FJsonValueObject>(ItemJson(Item)));
    }
    // The reward source: its drops and the first weapon, as the quest turn-in would roll it at stage 8; then at stage
    // 80, where the normal-mode cap must hold the gun at level 50 (the first seed from 1 whose roll drops a weapon).
    auto PoolCheck = [&RewardSourcePath](int32 Stage, uint32 Seed, bool& bOutRolled, FOpenWillowWeaponItem& OutPoolItem)
    {
        TSharedPtr<FJsonObject> Pool = MakeShared<FJsonObject>();
        TArray<FString> Drops;
        bOutRolled = !RewardSourcePath.IsEmpty() && RollPoolWeapon(RewardSourcePath, Stage, Seed, OutPoolItem, &Drops);
        Pool->SetStringField(TEXT("pool"), RewardSourcePath);
        Pool->SetNumberField(TEXT("stage"), Stage);
        Pool->SetNumberField(TEXT("seed"), Seed);
        Pool->SetNumberField(TEXT("stage_cap"), NormalStageCap);
        TArray<TSharedPtr<FJsonValue>> DropValues;
        for (const FString& Drop : Drops) DropValues.Add(MakeShared<FJsonValueString>(Drop));
        Pool->SetArrayField(TEXT("drops"), DropValues);
        if (bOutRolled) Pool->SetObjectField(TEXT("item"), ItemJson(OutPoolItem));
        UE_LOG(LogTemp, Display, TEXT("OWWEAPONGEN check pool %s stage %d seed %u: %d drop(s), weapon %s"), *RewardSourcePath, Stage, Seed,
            Drops.Num(), bOutRolled ? *OutPoolItem.Name : TEXT("none"));
        return Pool;
    };
    bool bPoolRolled = false;
    FOpenWillowWeaponItem PoolItem;
    TSharedPtr<FJsonObject> Pool = PoolCheck(8, 1, bPoolRolled, PoolItem);
    TSharedPtr<FJsonObject> CappedPool;
    for (uint32 Seed = 1; Seed <= 50 && !RewardSourcePath.IsEmpty(); ++Seed)
    {
        bool bRolled = false;
        FOpenWillowWeaponItem Capped;
        CappedPool = PoolCheck(80, Seed, bRolled, Capped);
        if (!bRolled) continue;
        UE_LOG(LogTemp, Display, TEXT("OWWEAPONGEN check stage cap: stage 80 pool gun level %d (%s, cap %d)"), Capped.Level,
            Capped.Level == NormalStageCap ? TEXT("PASS") : TEXT("FAIL"), NormalStageCap);
        break;
    }

    TSharedPtr<FJsonObject> Root = MakeShared<FJsonObject>();
    Root->SetStringField(TEXT("manifest"), ManifestPath());
    Root->SetArrayField(TEXT("items"), Items);
    Root->SetObjectField(TEXT("pool"), Pool);
    if (CappedPool) Root->SetObjectField(TEXT("pool_capped"), CappedPool);
    FString Text;
    auto Writer = TJsonWriterFactory<>::Create(&Text);
    FJsonSerializer::Serialize(Root.ToSharedRef(), Writer);
    Writer->Close();
    const FString Dir = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/weapons/gen/host_check")));
    IFileManager::Get().MakeDirectory(*Dir, true);
    const FString File = FPaths::Combine(Dir, TEXT("host_items.json"));
    if (FFileHelper::SaveStringToFile(Text, *File))
    {
        UE_LOG(LogTemp, Display, TEXT("OWWEAPONGEN host check: %d item(s) written to %s"), Items.Num(), *File);
    }
    else
    {
        UE_LOG(LogTemp, Warning, TEXT("OWWEAPONGEN host check: cannot write %s"), *File);
    }
}
