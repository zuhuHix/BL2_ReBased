// AI-assisted (Claude). Test-only player driver, see OpenWillowPlaytest.h. Host behaviour only.
#include "OpenWillowPlaytest.h"

#include "Camera/CameraComponent.h"
#include "EngineUtils.h"
#include "Framework/Application/SlateApplication.h"
#include "Framework/Commands/InputChord.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "GameFramework/PlayerController.h"
#include "HighResScreenshot.h"
#include "InputCoreTypes.h"
#include "InputKeyEventArgs.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowEnemy.h"
#include "OpenWillowInteractive.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowNpc.h"
#include "OpenWillowQuest.h"
#include "OpenWillowQuickChange.h"
#include "OpenWillowSkills.h"
#include "OpenWillowVendor.h"
#include "OpenWillowWalker.h"
#include "UnrealClient.h"

namespace
{
    AOpenWillowWalker* Maya(const UActorComponent* Component) { return Cast<AOpenWillowWalker>(Component->GetOwner()); }
    APlayerController* Pc(const UActorComponent* Component)
    {
        const AOpenWillowWalker* W = Maya(Component);
        return W ? Cast<APlayerController>(W->GetController()) : nullptr;
    }
}

UOpenWillowPlaytest::UOpenWillowPlaytest()
{
    PrimaryComponentTick.bCanEverTick = true;
    PrimaryComponentTick.bTickEvenWhenPaused = true;
}

void UOpenWillowPlaytest::BeginPlay()
{
    Super::BeginPlay();
    FString File;
    FParse::Value(FCommandLine::Get(), TEXT("owplaytest="), File);
    FString Text;
    if (!FFileHelper::LoadFileToString(Text, *File))
    {
        UE_LOG(LogTemp, Error, TEXT("OWPLAY no script at %s"), *File);
        return;
    }
    Text.ParseIntoArrayLines(Lines);
    UE_LOG(LogTemp, Display, TEXT("OWPLAY script %s: %d lines"), *File, Lines.Num());
}

void UOpenWillowPlaytest::TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function)
{
    Super::TickComponent(Delta, Type, Function);
    AOpenWillowWalker* W = Maya(this);
    if (!W || !W->IsMayaActive()) return;
    if (BurstLeft > 0)
    {
        BurstClock -= Delta;
        if (BurstClock <= 0.f)
        {
            FScreenshotRequest::RequestScreenshot(FString::Printf(TEXT("OWPlay_%s_%02d.png"), *BurstName, BurstIndex++), true, false);
            BurstClock = BurstEvery;
            --BurstLeft;
        }
    }
    WaitLeft -= Delta;
    if (WaitLeft > 0.f) return;
    // One command per tick, except that a wait stops the loop; a shot needs its own frame anyway.
    while (Next < Lines.Num() && WaitLeft <= 0.f)
    {
        const FString Line = Lines[Next++].TrimStartAndEnd();
        if (Line.IsEmpty() || Line.StartsWith(TEXT("#"))) continue;
        RunCommand(Line);
        if (Line.StartsWith(TEXT("shot")) || Line.StartsWith(TEXT("tp"))) break;
    }
}

void UOpenWillowPlaytest::SendKey(const FString& Name, bool bDown, bool bUp)
{
    const FKey Key(*Name);
    if (!Key.IsValid())
    {
        UE_LOG(LogTemp, Warning, TEXT("OWPLAY bad key %s"), *Name);
        return;
    }
    const uint32* KeyCode = nullptr;
    const uint32* CharCode = nullptr;
    FInputKeyManager::Get().GetCodesFromKey(Key, KeyCode, CharCode);
    const FKeyEvent Event(Key, FModifierKeysState(), uint32(0), false, CharCode ? *CharCode : 0, KeyCode ? *KeyCode : 0);
    FSlateApplication& Slate = FSlateApplication::Get();
    const TSharedPtr<SWidget> Focus = Slate.GetKeyboardFocusedWidget();
    bool bHandledDown = false, bHandledUp = false;
    if (bDown) bHandledDown = Slate.ProcessKeyDownEvent(Event);
    if (bUp) bHandledUp = Slate.ProcessKeyUpEvent(Event);
    UE_LOG(LogTemp, Display, TEXT("OWPLAY key %s down=%d up=%d handled=%d/%d focus=%s"), *Name, bDown, bUp, bHandledDown, bHandledUp,
        Focus.IsValid() ? *Focus->GetTypeAsString() : TEXT("none"));
}

AActor* UOpenWillowPlaytest::FindTarget(const FString& What) const
{
    const AOpenWillowWalker* W = Maya(this);
    AActor* Best = nullptr;
    double BestDistance = TNumericLimits<double>::Max();
    const auto Offer = [&](AActor* Actor)
    {
        if (!Actor || Actor == W) return;
        const double Distance = FVector::DistSquared(Actor->GetActorLocation(), W->GetActorLocation());
        if (Distance < BestDistance) { BestDistance = Distance; Best = Actor; }
    };
    FString Kind, Part;
    if (!What.Split(TEXT(":"), &Kind, &Part)) Kind = What;
    UWorld* World = GetWorld();
    if (Kind == TEXT("enemy"))
    {
        for (TActorIterator<AOpenWillowEnemy> It(World); It; ++It) if (!It->IsDead()) Offer(*It);
    }
    else if (Kind == TEXT("dummy"))
    {
        for (TActorIterator<AOpenWillowCombatTarget> It(World); It; ++It) if (!It->IsDead() && !Cast<AOpenWillowEnemy>(*It)) Offer(*It);
    }
    else if (Kind == TEXT("npc")) { for (TActorIterator<AOpenWillowNpc> It(World); It; ++It) Offer(*It); }
    else if (Kind == TEXT("pickup")) { for (TActorIterator<AOpenWillowInventoryPickup> It(World); It; ++It) Offer(*It); }
    else if (Kind == TEXT("station")) { for (TActorIterator<AOpenWillowQuickChangeStation> It(World); It; ++It) Offer(*It); }
    else if (Kind == TEXT("vendor"))
    {
        for (TActorIterator<AOpenWillowVendingMachine> It(World); It; ++It) if (Part.IsEmpty() || It->Kind == Part) Offer(*It);
    }
    else if (Kind == TEXT("lootable"))
    {
        for (TActorIterator<AOpenWillowLootable> It(World); It; ++It)
            if (!It->IsOpened() && (Part.IsEmpty() || It->GetName().Contains(Part))) Offer(*It);
    }
    else if (Kind == TEXT("actor"))
    {
        for (TActorIterator<AActor> It(World); It; ++It) if (It->GetName().Contains(Part)) Offer(*It);
    }
    return Best;
}

FVector UOpenWillowPlaytest::TargetPoint(AActor* Target, float ZOffset) const
{
    FVector Origin, Extent;
    Target->GetActorBounds(true, Origin, Extent);
    return (Extent.IsNearlyZero() ? Target->GetActorLocation() : Origin) + FVector(0, 0, ZOffset);
}

void UOpenWillowPlaytest::AimAtPoint(const FVector& Point)
{
    AOpenWillowWalker* W = Maya(this);
    const UCameraComponent* Camera = W->FindComponentByClass<UCameraComponent>();
    const FVector Eye = Camera ? Camera->GetComponentLocation() : W->GetActorLocation();
    if (APlayerController* PC = Pc(this)) PC->SetControlRotation((Point - Eye).Rotation());
}

void UOpenWillowPlaytest::LogState(const FString& Label) const
{
    AOpenWillowWalker* W = Maya(this);
    const UOpenWillowInventory* Inv = W->GetInventory();
    UOpenWillowSkills* Skills = W->GetSkills();
    int32 Money = -1, Eridium = -1;
    if (Inv) { Inv->GetMoney(Money); Inv->GetEridium(Eridium); }
    const FOpenWillowWeaponItem* Gun = Inv ? Inv->ActiveWeapon() : nullptr;
    EOpenWillowAmmoType Ammo;
    const int32 Reserve = Gun && Inv && UOpenWillowInventory::ResolveAmmoType(*Gun, Ammo) ? Inv->AmmoPool(Ammo).Current : -1;
    const FVector P = W->GetActorLocation();
    const FRotator V = W->GetViewRotation();
    UE_LOG(LogTemp, Display, TEXT("OWPLAY state %s t=%.1f pos=(%.0f %.0f %.0f) yaw=%.0f pitch=%.0f speed=%.0f ground=%d mode=%d hp=%.0f/%.0f shield=%.0f/%.0f down=%d level=%d xp=%lld points=%d money=%d erid=%d"),
        *Label, GetWorld()->GetTimeSeconds(), P.X, P.Y, P.Z, V.Yaw, V.Pitch, W->GetVelocity().Size(),
        W->GetCharacterMovement()->IsMovingOnGround(), int32(W->GetCharacterMovement()->MovementMode.GetValue()), W->GetHealth(), W->GetMaxHealth(), W->GetShield(),
        W->GetShieldCapacity(), W->IsDown(), Skills ? Skills->GetLevel() : -1, Skills ? Skills->GetExperience() : -1,
        Skills ? Skills->AvailablePoints() : -1, Money, Eridium);
    UE_LOG(LogTemp, Display, TEXT("OWPLAY state %s gun slot=%d name=\"%s\" id=%s mesh=%s mag=%d/%d reserve=%d out=%d reloading=%d backpack=%d/%d"),
        *Label, Inv ? Inv->GetActiveSlot() : -1, Gun ? *Gun->Name : TEXT(""), Gun ? *Gun->InstanceId : TEXT(""), *W->HeldWeaponMesh(),
        Gun ? UOpenWillowInventory::MagazineLeft(*Gun) : -1, Gun ? UOpenWillowInventory::MagazineSize(*Gun) : -1, Reserve, W->HasWeaponOut(), W->IsReloading(),
        Inv ? Inv->BackpackCount() : -1, Inv ? Inv->GetBackpackCapacity() : -1);
    FString SkillWhy;
    const bool bCan = W->CanCastPhaselock(SkillWhy);
    const UOpenWillowQuest* Quest = W->GetQuest();
    const APlayerController* PC = Pc(this);
    const AOpenWillowMayaHUD* Hud = PC ? Cast<AOpenWillowMayaHUD>(PC->GetHUD()) : nullptr;
    const UOpenWillowVendorSubsystem* Vendors = GetWorld()->GetSubsystem<UOpenWillowVendorSubsystem>();
    const TCHAR* UseNames[] = {TEXT("none"), TEXT("vendor"), TEXT("interactive"), TEXT("door"), TEXT("pickup")};
    UE_LOG(LogTemp, Display, TEXT("OWPLAY state %s skill can=%d why=\"%s\" active=%d left=%.1f hit=%d grade=%d quest=%d use=%s hud[menu=%d inv=%d map=%d log=%d shop=%d] arms=%d"),
        *Label, bCan, *SkillWhy, W->IsPhaselockActive(), W->PhaselockRemaining(), W->LastPhaselockHit(), Skills ? Skills->GetActionGrade() : -1,
        Quest ? Quest->Status() : -1, UseNames[int32(W->NearestUseTarget())], Hud && Hud->IsSkillsOpen(), Hud && Hud->IsInventoryOpen(), Hud && Hud->IsMapOpen(),
        Hud && Hud->IsMissionsOpen(), Vendors && Vendors->IsShopOpen(), W->AreArmsShown());
    int32 Alive = 0, Dead = 0, Pickups = 0;
    FString Nearest;
    double NearestDistance = TNumericLimits<double>::Max();
    for (TActorIterator<AOpenWillowEnemy> It(GetWorld()); It; ++It)
    {
        It->IsDead() ? ++Dead : ++Alive;
        const double Distance = FVector::Dist(It->GetActorLocation(), P);
        if (!It->IsDead() && Distance < NearestDistance)
        {
            NearestDistance = Distance;
            Nearest = FString::Printf(TEXT("%s hp=%.0f/%.0f shield=%.0f at %.0f uu"), *It->GetName(), It->GetHealth(), It->GetMaxHealth(), It->ShieldNow(), Distance);
        }
    }
    FString Drops;
    for (TActorIterator<AOpenWillowInventoryPickup> It(GetWorld()); It; ++It)
    {
        ++Pickups;
        if (Pickups <= 4) Drops += FString::Printf(TEXT(" %s@%.0f"), *It->GetName(), FVector::Dist(It->GetActorLocation(), P));
    }
    UE_LOG(LogTemp, Display, TEXT("OWPLAY state %s world enemies=%d dead=%d nearest=[%s] pickups=%d%s"), *Label, Alive, Dead, *Nearest, Pickups, *Drops);
}

void UOpenWillowPlaytest::RunCommand(const FString& Line)
{
    TArray<FString> T;
    Line.ParseIntoArrayWS(T);
    AOpenWillowWalker* W = Maya(this);
    APlayerController* PC = Pc(this);
    if (!W || !PC || T.Num() == 0) return;
    const FString& C = T[0];
    const int32 N = T.Num();
    const auto Num = [&](int32 I, float Default = 0.f) { return I < N ? FCString::Atof(*T[I]) : Default; };
    UE_LOG(LogTemp, Display, TEXT("OWPLAY > %s"), *Line);
    if (C == TEXT("wait")) WaitLeft = Num(1, 1.f);
    else if (C == TEXT("tap") && N > 1) SendKey(T[1], true, true);
    else if (C == TEXT("down") && N > 1) SendKey(T[1], true, false);
    else if (C == TEXT("up") && N > 1) SendKey(T[1], false, true);
    else if (C == TEXT("mouse") && N > 2)
    {
        const FKey Key = T[1] == TEXT("L") ? EKeys::LeftMouseButton : EKeys::RightMouseButton;
        const bool bDown = T[2] == TEXT("down");
        PC->InputKey(FInputKeyEventArgs::CreateSimulated(Key, bDown ? IE_Pressed : IE_Released, bDown ? 1.f : 0.f));
    }
    else if (C == TEXT("aim")) PC->SetControlRotation(FRotator(Num(2), Num(1), 0.f));
    else if (C == TEXT("tp") && N > 3)
    {
        W->SetActorLocation(FVector(Num(1), Num(2), Num(3)), false, nullptr, ETeleportType::TeleportPhysics);
        W->NotifyTeleported();
        if (N > 4) PC->SetControlRotation(FRotator(0.f, Num(4), 0.f));
    }
    else if ((C == TEXT("tpnear") || C == TEXT("aimat")) && N > 1)
    {
        AActor* Target = FindTarget(T[1]);
        if (!Target) { UE_LOG(LogTemp, Warning, TEXT("OWPLAY no target %s"), *T[1]); return; }
        UE_LOG(LogTemp, Display, TEXT("OWPLAY target %s = %s at %s yaw %.0f"), *T[1], *Target->GetName(), *Target->GetActorLocation().ToCompactString(), Target->GetActorRotation().Yaw);
        if (C == TEXT("tpnear"))
        {
            const FVector Dir = Target->GetActorForwardVector().RotateAngleAxis(Num(3), FVector::UpVector);
            const FVector Spot = Target->GetActorLocation() + Dir * Num(2, 150.f) + FVector(0, 0, Num(4, 100.f));
            W->SetActorLocation(Spot, false, nullptr, ETeleportType::TeleportPhysics);
            W->NotifyTeleported();
            AimAtPoint(TargetPoint(Target, 0.f));
        }
        else AimAtPoint(TargetPoint(Target, Num(2)));
    }
    else if (C == TEXT("exec"))
    {
        const FString Command = Line.Mid(4).TrimStartAndEnd();
        PC->ConsoleCommand(Command, true);
    }
    else if (C == TEXT("shot") && N > 1) FScreenshotRequest::RequestScreenshot(FString::Printf(TEXT("OWPlay_%s.png"), *T[1]), true, false);
    else if (C == TEXT("burst") && N > 3)
    {
        BurstName = T[1]; BurstIndex = 0; BurstLeft = FCString::Atoi(*T[2]); BurstEvery = Num(3, 0.1f); BurstClock = 0.f;
    }
    else if (C == TEXT("state")) LogState(N > 1 ? T[1] : TEXT("?"));
    else if (C == TEXT("list"))
    {
        const UOpenWillowInventory* Inv = W->GetInventory();
        if (!Inv) return;
        for (int32 I = 0; I < Inv->Items().Num(); ++I)
        {
            const FOpenWillowWeaponItem& Item = Inv->Items()[I];
            UE_LOG(LogTemp, Display, TEXT("OWPLAY item %d \"%s\" id=%s stem=%s mesh=%s"), I, *Item.Name, *Item.InstanceId, *Item.Id, *Item.MeshPath);
        }
    }
    else if (C == TEXT("equip") && N > 2)
    {
        const bool bOk = W->EquipItem(FCString::Atoi(*T[1]), FCString::Atoi(*T[2]));
        UE_LOG(LogTemp, Display, TEXT("OWPLAY equip %s -> %d"), *Line, bOk);
    }
    else if (C == TEXT("equipname") && N > 2)
    {
        const UOpenWillowInventory* Inv = W->GetInventory();
        int32 Found = INDEX_NONE;
        FString Id;
        for (int32 I = 0; Inv && I < Inv->Items().Num(); ++I)
            if (Inv->Items()[I].Name.Contains(T[1]) || Inv->Items()[I].Id.Contains(T[1])) { Found = I; Id = Inv->Items()[I].InstanceId; break; }
        const bool bEquipped = Found != INDEX_NONE && W->EquipItem(Found, FCString::Atoi(*T[2]));
        const bool bDrawn = bEquipped && W->DrawItemById(Id);
        UE_LOG(LogTemp, Display, TEXT("OWPLAY equipname %s index=%d id=%s equipped=%d drawn=%d"), *T[1], Found, *Id, bEquipped, bDrawn);
    }
    else if (C == TEXT("setlevel") && N > 1)
    {
        if (UOpenWillowSkills* Skills = W->GetSkills()) { Skills->SetLevel(FCString::Atoi(*T[1])); W->RefreshHealthForLevel(); }
    }
    else if (C == TEXT("spend") && N > 3)
    {
        FString Why;
        UOpenWillowSkills* Skills = W->GetSkills();
        const bool bOk = Skills && Skills->TrySpend(FCString::Atoi(*T[1]), FCString::Atoi(*T[2]), FCString::Atoi(*T[3]), Why);
        UE_LOG(LogTemp, Display, TEXT("OWPLAY spend %s -> %d why=\"%s\""), *Line, bOk, *Why);
    }
    else if (C == TEXT("hurt") && N > 1)
    {
        W->ApplyDamageToPlayer(Num(1), nullptr, W->GetActorLocation() + W->GetActorForwardVector() * 300.f);
    }
    else if (C == TEXT("quit")) { PC->ConsoleCommand(TEXT("quit")); }
    else UE_LOG(LogTemp, Warning, TEXT("OWPLAY unknown command: %s"), *Line);
}
