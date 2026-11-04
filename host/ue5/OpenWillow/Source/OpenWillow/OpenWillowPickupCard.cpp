#include "OpenWillowPickupCard.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowMayaState.h"
#include "OpenWillowSkills.h"
#include "OpenWillowWalker.h"
#include "Dom/JsonObject.h"
#include "Engine/Engine.h"
#include "Engine/GameViewportClient.h"
#include "Engine/World.h"
#include "EngineUtils.h"
#include "GameFramework/PlayerController.h"
#include "InputCoreTypes.h"
#include "Serialization/JsonSerializer.h"
#include "UnrealClient.h"

namespace
{
    // Reach: the same 220 uu that AOpenWillowWalker::PickupNearby uses, so the card shows only what E could take.
    // The game's own pickup reach and look-at test are native (UNVERIFIED).
    constexpr float Reach = 220.f;
    // The look-at cone (degrees from the view direction to the item's centre). A host choice, UNVERIFIED.
    constexpr float AimConeDegrees = 12.f;

    FString LastLoggedId;

    // The inventory's gear slot key for a gear item type (OpenWillowInventory's GearSlots order).
    const TCHAR* GearSlotForType(const FString& ItemType)
    {
        if (ItemType == TEXT("shield")) return TEXT("shield");
        if (ItemType == TEXT("grenade_mod")) return TEXT("grenadeMod");
        if (ItemType == TEXT("class_mod")) return TEXT("classMod");
        if (ItemType == TEXT("relic")) return TEXT("relic");
        return nullptr;
    }

    FString JsonText(const TSharedRef<FJsonObject>& Object)
    {
        FString Text;
        const TSharedRef<TJsonWriter<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>> Writer =
            TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Text);
        FJsonSerializer::Serialize(Object, Writer);
        return Text;
    }
}

AOpenWillowInventoryPickup* OpenWillowPickupCard::FindAimedPickup(const AOpenWillowWalker& Maya, const APlayerController& Player)
{
    UWorld* World = Maya.GetWorld();
    if (!World) return nullptr;
    FVector ViewLocation;
    FRotator ViewRotation;
    Player.GetPlayerViewPoint(ViewLocation, ViewRotation);
    const FVector ViewDirection = ViewRotation.Vector();
    const float MinCos = FMath::Cos(FMath::DegreesToRadians(AimConeDegrees));
    // Candidates within reach and the cone, ordered by angle to the view direction, then distance from Maya, then name,
    // so equal angles never depend on the actors' iteration order. The first one in sight wins.
    struct FCandidate { AOpenWillowInventoryPickup* Pickup; float Cos; float DistanceSquared; FString Name; };
    TArray<FCandidate> Candidates;
    for (TActorIterator<AOpenWillowInventoryPickup> It(World); It; ++It)
    {
        const float DistanceSquared = FVector::DistSquared(Maya.GetActorLocation(), It->GetActorLocation());
        if (DistanceSquared >= FMath::Square(Reach)) continue;
        const float Cos = FVector::DotProduct(ViewDirection, (It->GetActorLocation() - ViewLocation).GetSafeNormal());
        if (Cos < MinCos) continue;
        Candidates.Add({*It, Cos, DistanceSquared, It->DisplayName() + TEXT("|") + It->GetName()});
    }
    Candidates.Sort([](const FCandidate& A, const FCandidate& B)
    {
        // Angles within 1e-5 of each other (cosines) count as equal.
        if (FMath::Abs(A.Cos - B.Cos) > 1e-5f) return A.Cos > B.Cos;
        if (A.DistanceSquared != B.DistanceSquared) return A.DistanceSquared < B.DistanceSquared;
        return A.Name < B.Name;
    });
    for (const FCandidate& Candidate : Candidates)
    {
        // Line of sight from the view to the item, ignoring Maya and the pickup itself (no card through walls).
        FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowPickupCardSight), false, &Maya);
        Query.AddIgnoredActor(Candidate.Pickup);
        FHitResult Hit;
        if (!World->LineTraceSingleByChannel(Hit, ViewLocation, Candidate.Pickup->GetActorLocation(), ECC_Visibility, Query))
            return Candidate.Pickup;
    }
    return nullptr;
}

namespace
{
    // The last search, at most SearchHz times a second, and the card JSON built for its target.
    constexpr double SearchHz = 10.;
    double NextSearchAt = 0.;
    TWeakObjectPtr<AOpenWillowInventoryPickup> AimedTarget;
    FString CardKey, CardBody;
}

FString OpenWillowPickupCard::StateField(const AOpenWillowWalker& Maya, const APlayerController& Player)
{
    const UOpenWillowInventory* Inventory = Maya.GetInventory();
    // No card while Maya is down or dead.
    if (!Inventory || Maya.IsDown())
    {
        AimedTarget.Reset();
        if (!LastLoggedId.IsEmpty()) UE_LOG(LogTemp, Display, TEXT("OWPICKUPCARD hidden"));
        LastLoggedId.Reset();
        return TEXT("null");
    }
    const double Now = FPlatformTime::Seconds();
    if (Now >= NextSearchAt)
    {
        AimedTarget = FindAimedPickup(Maya, Player);
        NextSearchAt = Now + 1. / SearchHz;
    }
    const AOpenWillowInventoryPickup* Pickup = AimedTarget.Get();
    if (!Pickup)
    {
        if (!LastLoggedId.IsEmpty()) UE_LOG(LogTemp, Display, TEXT("OWPICKUPCARD hidden"));
        LastLoggedId.Reset();
        return TEXT("null");
    }
    const FOpenWillowTakenInventoryItem& Taken = Pickup->GetItem();
    const FOpenWillowWeaponItem* Held = Inventory->ActiveWeapon();
    const int32 Level = Maya.GetSkills() ? Maya.GetSkills()->GetLevel() : 1;
    const FString GearSlot = Taken.bIsGear ? FString(GearSlotForType(Taken.Gear.ItemType)) : FString();
    const FOpenWillowGearItem* EquippedGear = GearSlot.IsEmpty() ? nullptr : Inventory->GearSlotItem(GearSlot);
    // The card body (item, compare, level) is rebuilt only when what it shows changes.
    const FString Key = FString::Printf(TEXT("%s|%s|%s|%d"), *Pickup->GetName(),
        Taken.bIsGear ? (EquippedGear ? *EquippedGear->Id : TEXT("")) : (Held ? *UOpenWillowInventory::StableId(*Held) : TEXT("")),
        Taken.bIsGear ? TEXT("gear") : TEXT("weapon"), Level);
    FString Id, Name;
    if (Key != CardKey)
    {
        TSharedRef<FJsonObject> Card = MakeShared<FJsonObject>();
        TSharedPtr<FJsonObject> Compare;
        if (Taken.bIsGear)
        {
            Card->SetObjectField(TEXT("item"), UOpenWillowInventory::GearItemJson(Taken.Gear));
            if (EquippedGear) Compare = UOpenWillowInventory::GearItemJson(*EquippedGear);
        }
        else
        {
            Card->SetObjectField(TEXT("item"), UOpenWillowInventory::WeaponItemJson(Taken.Weapon));
            // The equipped analogue of a weapon is the held one, whatever its type (WillowInventoryManager.GetEquippedAnalogue
            // returns the pawn's current weapon; the menu compares the same way). Whether the native pickup card uses the
            // same analogue is not read (UNVERIFIED).
            if (Held) Compare = UOpenWillowInventory::WeaponItemJson(*Held);
        }
        if (Compare.IsValid()) Card->SetObjectField(TEXT("compare"), Compare);
        else Card->SetField(TEXT("compare"), MakeShared<FJsonValueNull>());
        Card->SetStringField(TEXT("id"), Taken.bIsGear ? Taken.Gear.Id : UOpenWillowInventory::StableId(Taken.Weapon));
        Card->SetNumberField(TEXT("level"), Level);
        CardBody = JsonText(Card).LeftChop(1); // without the closing brace: the screen point follows
        CardKey = Key;
    }
    Id = Taken.bIsGear ? Taken.Gear.Id : UOpenWillowInventory::StableId(Taken.Weapon);
    Name = Taken.bIsGear ? Taken.Gear.Name : Taken.Weapon.Name;
    // Where the item is on screen, as viewport fractions; the page places the card from it (placement UNVERIFIED).
    FVector2D Screen(0.5f, 0.5f);
    int32 Width = 0, Height = 0;
    Player.GetViewportSize(Width, Height);
    FVector2D Pixel;
    if (Width > 0 && Height > 0 && Player.ProjectWorldLocationToScreen(Pickup->GetActorLocation(), Pixel))
        Screen = FVector2D(Pixel.X / Width, Pixel.Y / Height);
    if (Id != LastLoggedId)
    {
        UE_LOG(LogTemp, Display, TEXT("OWPICKUPCARD shown id=%s name=\"%s\" compare=%s"), *Id, *Name,
            Taken.bIsGear ? (EquippedGear ? *EquippedGear->Id : TEXT("none")) : (Held ? *UOpenWillowInventory::StableId(*Held) : TEXT("none")));
        LastLoggedId = Id;
    }
    return CardBody + FString::Printf(TEXT(",\"x\":%.3f,\"y\":%.3f}"), Screen.X, Screen.Y);
}

namespace
{
    int32 ShotStep = 0;
    float ShotAt = 0.f;
    FVector DropLocation = FVector::ZeroVector;
    bool bReleaseUse = false;

    void Aim(APlayerController& Player, const FVector& Target, float ExtraYaw = 0.f)
    {
        FVector ViewLocation;
        FRotator ViewRotation;
        Player.GetPlayerViewPoint(ViewLocation, ViewRotation);
        FRotator Look = (Target - ViewLocation).Rotation();
        Look.Yaw += ExtraYaw;
        Player.SetControlRotation(Look);
    }

    void Shot(int32 Index, const TCHAR* Name, float Now)
    {
        const FString File = FString::Printf(TEXT("OWPickupCard_%d_%s.png"), Index, Name);
        FScreenshotRequest::RequestScreenshot(File, true, false);
        UE_LOG(LogTemp, Display, TEXT("OWPICKUPCARDSHOTS shot %s t=%.3f"), *File, Now);
    }

    // A generated gun into the backpack (ow.RollGun <balance> 34 <seed>; -owpickupcardgun=<balance> overrides the default),
    // then dropped 150 uu ahead of Maya on the ground, as the inventory's drop does. False (logged) when it failed.
    bool DropRolledGun(AOpenWillowWalker& Maya, int32 Seed)
    {
        UWorld* World = Maya.GetWorld();
        UOpenWillowInventory* Inventory = Maya.GetInventory();
        FString Balance = TEXT("GD_Weap_SMG.A_Weapons_Legendary.SMG_Maliwan_5_HellFire");
        FParse::Value(FCommandLine::Get(), TEXT("owpickupcardgun="), Balance);
        TSet<FString> Before;
        for (const FOpenWillowWeaponItem& Item : Inventory->Items()) Before.Add(UOpenWillowInventory::StableId(Item));
        if (GEngine) GEngine->Exec(World, *FString::Printf(TEXT("ow.RollGun %s 34 %d"), *Balance, Seed));
        FString NewId;
        for (const FOpenWillowWeaponItem& Item : Inventory->Items())
            if (!Before.Contains(UOpenWillowInventory::StableId(Item))) NewId = UOpenWillowInventory::StableId(Item);
        FOpenWillowTakenInventoryItem Taken;
        if (NewId.IsEmpty() || !Maya.TakeInventoryItemById(NewId, Taken))
        {
            UE_LOG(LogTemp, Warning, TEXT("OWPICKUPCARDSHOTS failed: ow.RollGun added no gun (weapon generator manifest missing?)"));
            return false;
        }
        const FVector Ahead = Maya.GetActorLocation() + Maya.GetActorForwardVector() * 150.f;
        FHitResult Ground;
        FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowPickupCardDrop), false, &Maya);
        DropLocation = World->LineTraceSingleByChannel(Ground, Ahead + FVector(0.f, 0.f, 100.f), Ahead - FVector(0.f, 0.f, 450.f),
            ECC_WorldStatic, Query) ? Ground.ImpactPoint + FVector(0.f, 0.f, 12.f) : Ahead;
        FActorSpawnParameters Spawn;
        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
        AOpenWillowInventoryPickup* Pickup = World->SpawnActor<AOpenWillowInventoryPickup>(
            AOpenWillowInventoryPickup::StaticClass(), FTransform(DropLocation), Spawn);
        if (!Pickup)
        {
            UE_LOG(LogTemp, Warning, TEXT("OWPICKUPCARDSHOTS failed: the pickup did not spawn"));
            return false;
        }
        Pickup->Initialize(MoveTemp(Taken));
        UE_LOG(LogTemp, Display, TEXT("OWPICKUPCARDSHOTS dropped %s at (%.0f, %.0f, %.0f)"), *NewId, DropLocation.X, DropLocation.Y, DropLocation.Z);
        return true;
    }
}

void OpenWillowPickupCard::TickShots(AOpenWillowWalker& Maya, APlayerController& Player)
{
    static const bool bShots = FParse::Param(FCommandLine::Get(), TEXT("owpickupcardshots"));
    if (!bShots || ShotStep < 0) return;
    UWorld* World = Maya.GetWorld();
    UOpenWillowInventory* Inventory = Maya.GetInventory();
    if (!World || !Inventory) return;
    if (bReleaseUse)
    {
        Player.InputKey(FInputKeyEventArgs::CreateSimulated(EKeys::E, IE_Released, 0.f));
        bReleaseUse = false;
    }
    // The movie HUD settles a few seconds after it is ready (as the minimap shots); each pose gets 1.5 s.
    constexpr float Start = 10.f, Settle = 1.5f;
    const float Now = World->GetTimeSeconds();
    if (Now < Start || Now < ShotAt + Settle) return;
    auto Fail = [&Player]() { Player.ConsoleCommand(TEXT("quit")); ShotStep = -1; };
    switch (ShotStep)
    {
    case 0:
        if (!DropRolledGun(Maya, 1)) { Fail(); return; }
        Aim(Player, DropLocation, 70.f);
        break;
    case 1: Shot(1, TEXT("Away"), Now); Aim(Player, DropLocation); break;
    case 2: Shot(2, TEXT("Aimed"), Now); Aim(Player, DropLocation, 40.f); break;
    case 3: Shot(3, TEXT("AimedAway"), Now); Aim(Player, DropLocation); break;
    case 4:
        Shot(4, TEXT("AimedAgain"), Now);
        // E picks up (the player controller's own input path, as the inventory suite presses it).
        Player.InputKey(FInputKeyEventArgs::CreateSimulated(EKeys::E, IE_Pressed, 1.f));
        bReleaseUse = true;
        break;
    case 5:
        Shot(5, TEXT("PickedUp"), Now);
        // A second gun, aimed at, then Maya goes down: the card must hide.
        if (!DropRolledGun(Maya, 2)) { Fail(); return; }
        Aim(Player, DropLocation);
        break;
    case 6:
        Shot(6, TEXT("SecondAimed"), Now);
        OpenWillowApplyDamageToPlayer(World, 1.e6f);
        UE_LOG(LogTemp, Display, TEXT("OWPICKUPCARDSHOTS down=%d"), Maya.IsDown() ? 1 : 0);
        break;
    case 7: Shot(7, TEXT("Down"), Now); break;
    default:
        UE_LOG(LogTemp, Display, TEXT("OWPICKUPCARDSHOTS done"));
        Fail();
        return;
    }
    ShotAt = Now;
    ++ShotStep;
}
