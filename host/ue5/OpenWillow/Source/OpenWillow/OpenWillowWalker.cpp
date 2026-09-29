#include "OpenWillowWalker.h"
#include "OpenWillowArmsAnimInstance.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInventoryActionTest.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowInventoryWidget.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowSkills.h"
#include "Blueprint/UserWidget.h"
#include "Misc/Paths.h"
#include "OpenWillowShotFx.h"
#include "Animation/AnimSequence.h"
#include "Camera/CameraComponent.h"
#include "Components/CapsuleComponent.h"
#include "Components/InputComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Engine/Engine.h"
#include "EngineUtils.h"
#include "Engine/GameViewportClient.h"
#include "Engine/SkeletalMesh.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "Kismet/GameplayStatics.h"
#include "Materials/MaterialInterface.h"
#include "GameFramework/PlayerController.h"
#include "UnrealClient.h"

namespace
{
const TCHAR* MayaRoot = TEXT("/Game/OpenWillow/Characters/Maya");
UAnimSequence* LoadArmsAnim(const TCHAR* Set, const TCHAR* Clip)
{
    const FString Path = FString::Printf(TEXT("%s/FirstPerson/Anim_%s_%s.Anim_%s_%s"),
        MayaRoot, Set, Clip, Set, Clip);
    UAnimSequence* Anim = LoadObject<UAnimSequence>(nullptr, *Path);
    if (!Anim) UE_LOG(LogTemp, Warning, TEXT("OpenWillow arms animation not found: %s"), *Path);
    return Anim;
}
}

AOpenWillowWalker::AOpenWillowWalker()
{
    PrimaryActorTick.bCanEverTick = true;
    GetCapsuleComponent()->InitCapsuleSize(34, 88);
    Camera = CreateDefaultSubobject<UCameraComponent>(TEXT("WalkingCamera"));
    Camera->SetupAttachment(GetCapsuleComponent());
    Camera->SetRelativeLocation(FVector(0, 0, 64));
    Camera->bUsePawnControlRotation = true;
    Arms = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("FirstPersonArms"));
    Arms->SetupAttachment(Camera);
    Arms->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Arms->SetCastShadow(false);
    // The animation root moves the mesh far from its bind-pose bounds.
    Arms->SetBoundsScale(4);
    WeaponVisual = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("InfinityVisualProxy"));
    WeaponVisual->SetupAttachment(Arms, TEXT("R_Weapon_Bone"));
    WeaponVisual->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    WeaponVisual->SetCastShadow(false);
    WeaponVisual->SetHiddenInGame(true);
    Inventory = CreateDefaultSubobject<UOpenWillowInventory>(TEXT("Inventory"));
    Skills = CreateDefaultSubobject<UOpenWillowSkills>(TEXT("Skills"));
    GetCharacterMovement()->MaxWalkSpeed = 450;
    GetCharacterMovement()->JumpZVelocity = 420;
    GetCharacterMovement()->MaxStepHeight = 35;
    GetCharacterMovement()->SetWalkableFloorAngle(45);
}

void AOpenWillowWalker::BeginPlay()
{
    Super::BeginPlay();
    // BL2's installed DefaultGame.ini sets Engine.WorldInfo.DefaultGravityZ
    // to -500 cm/s^2. Scale the host world's gravity to that magnitude rather
    // than inheriting UE5's default -980, so the jump arc uses BL2 gravity.
    const float HostGravity = GetWorld() ? FMath::Abs(GetWorld()->GetGravityZ()) : 0.f;
    if (HostGravity > KINDA_SMALL_NUMBER)
    {
        GetCharacterMovement()->GravityScale = 500.f / HostGravity;
    }
    // JumpZVelocity=420 is still an estimate: at -500 cm/s^2 it predicts a
    // 176 cm apex and 1.68 s ideal flight, pending a measured BL2 jump.
    bMayaActive = FParse::Param(FCommandLine::Get(), TEXT("owmaya"));
    if (!bMayaActive) return;
    // GD_Siren_Streaming.Pawn_Siren: CylinderComponent CollisionRadius 42,
    // CollisionHeight 80 (UE3 half-height) and BaseEyeHeight 70 above the
    // pawn centre, so a 150 cm standing eye. Its serialized EyeHeight is 77;
    // using BaseEyeHeight (UE3's standing target) is an assumption.
    GetCapsuleComponent()->SetCapsuleSize(42, 80);
    Camera->SetRelativeLocation(FVector(0, 0, 70));
    // BL2 keeps vertical FOV (DefaultEngine.ini: AspectRatio_MaintainYFOV) and,
    // as UE3 does, reads its FOV setting as horizontal at 4:3. UE's camera FOV
    // is horizontal at the actual aspect, so convert. -owfov=<BL2 setting>; the
    // 90 default matches a maintainer capture by eye, not a read setting.
    float Bl2Fov = 90;
    FParse::Value(FCommandLine::Get(), TEXT("owfov="), Bl2Fov);
    float Aspect = 16.f / 9.f;
    if (GEngine && GEngine->GameViewport)
    {
        FVector2D ViewportSize = FVector2D::ZeroVector;
        GEngine->GameViewport->GetViewportSize(ViewportSize);
        if (ViewportSize.Y > KINDA_SMALL_NUMBER)
            Aspect = ViewportSize.X / ViewportSize.Y;
    }
    Camera->SetFieldOfView(FMath::RadiansToDegrees(2 * FMath::Atan(
        FMath::Tan(FMath::DegreesToRadians(Bl2Fov) / 2) * Aspect / (4.f / 3.f))));
    const FString MeshPath = FString::Printf(TEXT("%s/Meshes/Hands_Siren/SkeletalMeshes/Hands_Siren.Hands_Siren"), MayaRoot);
    USkeletalMesh* ArmsMesh = LoadObject<USkeletalMesh>(nullptr, *MeshPath);
    if (!ArmsMesh) { UE_LOG(LogTemp, Warning, TEXT("OpenWillow arms mesh not found: %s"), *MeshPath); return; }
    Arms->SetSkeletalMesh(ArmsMesh);
    Arms->SetAnimInstanceClass(UOpenWillowArmsAnimInstance::StaticClass());
    ArmsAnim = Cast<UOpenWillowArmsAnimInstance>(Arms->GetAnimInstance());
    DrawPistolAnim = LoadArmsAnim(TEXT("PistolCombat"), TEXT("Draw"));
    FirePistolAnim = LoadArmsAnim(TEXT("PistolCombat"), TEXT("ADD_Fire_Recoil"));
    PhaselockAnim = LoadArmsAnim(TEXT("SirenCombat"), TEXT("Phase_Lock_Lift"));
    PhaselockFailAnim = LoadArmsAnim(TEXT("SirenCombat"), TEXT("Phase_Lock_Fail"));
    // UE3 gestalt guns point along -Y; the hand's weapon bone expects X.
    // The yaw is an observed fit (see the barrel-axis log), not read data.
    float WeaponYaw = 90.f;
    FParse::Value(FCommandLine::Get(), TEXT("owweaponyaw="), WeaponYaw);
    WeaponVisual->SetRelativeRotation(FRotator(0, WeaponYaw, 0));
    // Rolled weapons: recipes from tools/weapon_recipe.py + weapon_stats.py,
    // by default under the repository's ignored local/items (-owitems=<dir>).
    FString ItemDir = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/items")));
    FParse::Value(FCommandLine::Get(), TEXT("owitems="), ItemDir);
    // The local recipe library is a demo set larger than BL2's 12-slot base backpack (that base is
    // itself UNVERIFIED here), so raise the cap to the class maximum before loading; otherwise the
    // first 12 file names win and the menu shows near-identical guns. -owbackpack=<12..39> overrides.
    int32 DemoBackpack = UOpenWillowInventory::MaximumBackpackCapacity;
    FParse::Value(FCommandLine::Get(), TEXT("owbackpack="), DemoBackpack);
    Inventory->SetBackpackCapacity(DemoBackpack);
    const int32 Loaded = Inventory->LoadRecipes(ItemDir);
    // Skill tree from tools/prepare_skill_tree.py, the same file the Skills
    // page shows (-owskilltree=<file>). The host earns no XP yet, so
    // -owlevel=<N> sets Maya's starting level (default 1, no skill points).
    FString TreeFile = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/ui/run/skilltree_siren.json")));
    FParse::Value(FCommandLine::Get(), TEXT("owskilltree="), TreeFile);
    if (!Skills->LoadTree(TreeFile)) UE_LOG(LogTemp, Warning, TEXT("OpenWillow skill tree not loaded: %s"), *TreeFile);
    int32 StartLevel = 1;
    const bool bCombatShotsRun = FParse::Param(FCommandLine::Get(), TEXT("owcombatshots"));
    // Test-only: the capture run needs the level-30 weapon recipes and the
    // level-36 gear manifest item to be equippable, so it starts at 36 unless
    // -owlevel says otherwise.
    // -owinventoryactions (UOpenWillowInventoryActionTest) needs the same.
    const bool bInventoryActionsRun = FParse::Param(FCommandLine::Get(), TEXT("owinventoryactions"));
    if (bCombatShotsRun || bInventoryActionsRun) StartLevel = 36;
    FParse::Value(FCommandLine::Get(), TEXT("owlevel="), StartLevel);
    Skills->SetLevel(StartLevel);
    // -owcombattest: spawn the stand-in target once Maya has landed, so it
    // stands on the ground in front of her rather than at the drop height.
    bWantsCombatTarget = FParse::Param(FCommandLine::Get(), TEXT("owcombattest"));
    // The scripted combat run casts Phaselock, so it starts with the action
    // skill trained (at least level 5, one point spent).
    if (bWantsCombatTarget)
    {
        if (Skills->GetLevel() < 5) Skills->SetLevel(5);
        FString Reason;
        Skills->TrySpend(-1, -1, -1, Reason);
    }

    FString GearManifestPath = FPaths::ConvertRelativePathToFull(
        FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/inventory/gear_manifest.json")));
    FParse::Value(FCommandLine::Get(), TEXT("owgear="), GearManifestPath);
    Inventory->LoadGearManifest(GearManifestPath);
    // -owslots=<2..4>: unlocked weapon slots (default 4, UNVERIFIED for a new
    // character). -owmoney / -owerid set the purse; unset they stay out of
    // the inventory snapshot. -owinventoryselftest runs the synthetic checks.
    int32 SlotsUnlocked = UOpenWillowInventory::SlotCount;
    if (FParse::Value(FCommandLine::Get(), TEXT("owslots="), SlotsUnlocked)
        && !Inventory->SetWeaponSlotsUnlocked(SlotsUnlocked))
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow ignored -owslots=%d (must be 2..4)"), SlotsUnlocked);
    int32 Purse = 0;
    if (FParse::Value(FCommandLine::Get(), TEXT("owmoney="), Purse)) Inventory->SetMoney(Purse);
    if (FParse::Value(FCommandLine::Get(), TEXT("owerid="), Purse)) Inventory->SetEridium(Purse);
    if (bCombatShotsRun)
    {
        // Test-only demo purse so the capture shows the currency bar. These
        // numbers are made up, not game data.
        if (!FParse::Value(FCommandLine::Get(), TEXT("owmoney="), Purse)) Inventory->SetMoney(1234567);
        if (!FParse::Value(FCommandLine::Get(), TEXT("owerid="), Purse)) Inventory->SetEridium(42);
        // Made-up reserves so the ammo panel shows current vs. max.
        Inventory->SetAmmoCurrent(EOpenWillowAmmoType::Pistol, 166);
        Inventory->SetAmmoCurrent(EOpenWillowAmmoType::SMG, 312);
        Inventory->SetAmmoCurrent(EOpenWillowAmmoType::AssaultRifle, 143);
        Inventory->SetAmmoCurrent(EOpenWillowAmmoType::Shotgun, 37);
        Inventory->SetAmmoCurrent(EOpenWillowAmmoType::Sniper, 20);
        Inventory->SetAmmoCurrent(EOpenWillowAmmoType::Launcher, 7);
        Inventory->SetAmmoCurrent(EOpenWillowAmmoType::Grenade, 2);
    }
    if (FParse::Param(FCommandLine::Get(), TEXT("owinventoryselftest"))) UOpenWillowInventory::RunSelfTest();

    // The recipe folder is a local test inventory. Only equip weapons whose
    // explicit level requirement is met by this Maya setup. The capture run
    // (test only) fills the slots with one weapon per ammo type first, so the
    // screenshot shows mixed gear rather than four copies of one pistol.
    TArray<int32> EquipOrder;
    for (int32 ItemIndex = 0; ItemIndex < Inventory->Items().Num(); ++ItemIndex) EquipOrder.Add(ItemIndex);
    if (bCombatShotsRun)
    {
        TArray<int32> Diverse, Rest;
        TSet<int32> SeenTypes;
        for (const int32 ItemIndex : EquipOrder)
        {
            EOpenWillowAmmoType Type;
            if (UOpenWillowInventory::ResolveAmmoType(Inventory->Items()[ItemIndex], Type) && !SeenTypes.Contains(int32(Type)))
            {
                SeenTypes.Add(int32(Type));
                Diverse.Add(ItemIndex);
            }
            else Rest.Add(ItemIndex);
        }
        EquipOrder = Diverse;
        EquipOrder.Append(Rest);
    }
    bool bSelectedInitialWeapon = false;
    int32 InitialSlot = 0;
    for (const int32 ItemIndex : EquipOrder)
    {
        if (InitialSlot >= Inventory->GetWeaponSlotsUnlocked()) break;
        const FOpenWillowWeaponItem& Item = Inventory->Items()[ItemIndex];
        if (Item.Level > Skills->GetLevel() || !Inventory->Equip(ItemIndex, InitialSlot)) continue;
        if (!bSelectedInitialWeapon)
        {
            SelectSlot(InitialSlot);
            bSelectedInitialWeapon = true;
        }
        ++InitialSlot;
    }
    if (bCombatShotsRun)
    {
        // Test-only: wear whatever gear the manifest holds so the gear cells
        // are populated (one item per slot type; equip checks the level).
        const TArray<FOpenWillowGearItem> Gear = Inventory->GearItemList();
        for (const FOpenWillowGearItem& Item : Gear)
            if (!Inventory->GearSlotItem(Item.ItemType)) Inventory->EquipGearById(Item.Id, Item.ItemType, Skills->GetLevel());
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya level %d, %d skill points, action grade %d"),
        Skills->GetLevel(), Skills->AvailablePoints(), Skills->GetActionGrade());
    if (bInventoryActionsRun)
    {
        UOpenWillowInventoryActionTest* Test = NewObject<UOpenWillowInventoryActionTest>(this, TEXT("InventoryActionTest"));
        Test->RegisterComponent();
    }
}

void AOpenWillowWalker::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    if (!bSpawnProbeLogged && FParse::Param(FCommandLine::Get(), TEXT("owspawnprobe"))
        && GetWorld()->GetTimeSeconds() > 5)
    {
        bSpawnProbeLogged = true;
        const auto* Floor = GetCharacterMovement()->CurrentFloor.HitResult.GetActor();
        const FString Source = Floor && Floor->Tags.Num() ? Floor->Tags[0].ToString() : TEXT("none");
        UE_LOG(LogTemp, Display, TEXT("OpenWillow spawn probe grounded=%d pawn=%s floor=%s"),
            GetCharacterMovement()->IsMovingOnGround(), *GetActorLocation().ToString(), *Source);
    }
    // -owautowalk: walk a slow circle unattended so captures can check the run
    // clip without running into a wall.
    static const bool bAutoWalk = FParse::Param(FCommandLine::Get(), TEXT("owautowalk"));
    if (bAutoWalk && Controller)
    {
        Controller->SetControlRotation(Controller->GetControlRotation() + FRotator(0, 60 * DeltaSeconds, 0));
        AddMovementInput(FRotator(0, GetControlRotation().Yaw, 0).Vector(), 1);
    }
    const float Now = GetWorld()->GetTimeSeconds();
    if (bWantsCombatTarget && !CombatTarget.IsValid() && Now > 1.f
        && GetCharacterMovement()->IsMovingOnGround())
    {
        bWantsCombatTarget = false;
        SpawnCombatTarget();
    }
    if (bMayaActive)
    {
        // Arms lag slightly behind fast look input and settle back.
        Sway = FMath::Vector2DInterpTo(Sway, FVector2D(
            FMath::Clamp(-LookInput.X * 1.2f, -4.f, 4.f), FMath::Clamp(LookInput.Y * 1.2f, -3.f, 3.f)),
            DeltaSeconds, 8.f);
        Arms->SetRelativeRotation(FRotator(Sway.Y, Sway.X, 0));
        LookInput = FVector2D::ZeroVector;
    }
    if (bMayaActive && Now < PhaselockBeamUntil && CombatTarget.IsValid()
        && Arms->GetBoneIndex(TEXT("L_Hand")) != INDEX_NONE)
    {
        const float Fade = FMath::Clamp((PhaselockBeamUntil - Now) / 0.3f, 0.f, 1.f);
        AOpenWillowShotFx::Tracer(GetWorld(), Arms->GetBoneLocation(TEXT("L_Hand")) + Camera->GetForwardVector() * 8.f,
            CombatTarget->AimPoint(), FLinearColor(0.6f, 0.2f, 1.f) * Fade, 0.9f, 0.02f);
    }
    if (bMayaActive && !bBarrelAxisLogged && Now > 6.f && WeaponVisual->GetSkinnedAsset())
    {
        bBarrelAxisLogged = true;
        const FVector Axis = (WeaponVisual->GetBoneLocation(TEXT("Barrel"))
            - WeaponVisual->GetBoneLocation(TEXT("WeaponOffset"))).GetSafeNormal();
        UE_LOG(LogTemp, Display, TEXT("OpenWillow barrel axis in view: forward=%.2f right=%.2f up=%.2f"),
            FVector::DotProduct(Axis, Camera->GetForwardVector()), FVector::DotProduct(Axis, Camera->GetRightVector()),
            FVector::DotProduct(Axis, Camera->GetUpVector()));
    }
    // -owcombatshots: an unattended equip/fire/Phaselock sequence with
    // numbered captures, so presentation reviews use repeatable views.
    static const bool bCombatShots = FParse::Param(FCommandLine::Get(), TEXT("owcombatshots"));
    if (bCombatShots && bMayaActive && Controller) RunCombatShots(Now);
    FOpenWillowWeaponItem* Weapon = Inventory->ActiveWeaponMutable();
    if (bReloading && Now >= ReloadEndsAt)
    {
        bReloading = false;
        const int32 Moved = Weapon ? Inventory->CompleteReload(*Weapon) : 0;
        UE_LOG(LogTemp, Display, TEXT("OpenWillow reload finished: +%d, magazine %d"), Moved,
            Weapon ? UOpenWillowInventory::MagazineLeft(*Weapon) : 0);
    }
    if (bMayaActive && bWeaponOut && Weapon && bFireHeld && !bReloading && Now >= NextShotAt)
    {
        if (Inventory->ConsumeShot(*Weapon))
        {
            FireWeapon();
            NextShotAt = Now + 1.f / Weapon->FireRate; // evaluated item-card fire rate
        }
        else if (!StartReload(Now) && !bOutOfAmmoLogged)
        {
            // Magazine empty and the pool holds nothing: the trigger does nothing.
            bOutOfAmmoLogged = true;
            UE_LOG(LogTemp, Display, TEXT("OpenWillow %s: out of ammo"), *Weapon->Name);
        }
    }
    if (!ArmsAnim) return;
    // BL2's own AnimTree is not reproduced; the native arms instance blends
    // the imported poses according to measured speed and the jump state.
    const bool bFalling = GetCharacterMovement()->IsFalling();
    if (bWasFalling && !bFalling && LandAnim)
        LandUntil = Now + LandAnim->GetPlayLength();
    bWasFalling = bFalling;
    ArmsAnim->SetMovement(GetVelocity().Size2D(), bFalling, Now < LandUntil);
}

void AOpenWillowWalker::SetupPlayerInputComponent(UInputComponent* Input)
{
    Super::SetupPlayerInputComponent(Input);
    Input->BindAxis(TEXT("OWForward"), this, &AOpenWillowWalker::Forward);
    Input->BindAxis(TEXT("OWRight"), this, &AOpenWillowWalker::Right);
    Input->BindAxis(TEXT("OWTurn"), this, &AOpenWillowWalker::Turn);
    Input->BindAxis(TEXT("OWLook"), this, &AOpenWillowWalker::Look);
    Input->BindAction(TEXT("OWJump"), IE_Pressed, this, &ACharacter::Jump);
    Input->BindAction(TEXT("OWJump"), IE_Released, this, &ACharacter::StopJumping);
    Input->BindAction(TEXT("OWSprint"), IE_Pressed, this, &AOpenWillowWalker::SprintPressed);
    Input->BindAction(TEXT("OWSprint"), IE_Released, this, &AOpenWillowWalker::SprintReleased);
    Input->BindAction(TEXT("OWWeapon1"), IE_Pressed, this, &AOpenWillowWalker::SelectSlot1);
    Input->BindAction(TEXT("OWWeapon2"), IE_Pressed, this, &AOpenWillowWalker::SelectSlot2);
    Input->BindAction(TEXT("OWWeapon3"), IE_Pressed, this, &AOpenWillowWalker::SelectSlot3);
    Input->BindAction(TEXT("OWWeapon4"), IE_Pressed, this, &AOpenWillowWalker::SelectSlot4);
    Input->BindAction(TEXT("OWHolster"), IE_Pressed, this, &AOpenWillowWalker::Holster);
    Input->BindAction(TEXT("OWInventory"), IE_Pressed, this, &AOpenWillowWalker::ToggleInventory);
    Input->BindAction(TEXT("OWPickup"), IE_Pressed, this, &AOpenWillowWalker::PickupNearby);
    Input->BindAction(TEXT("OWSkills"), IE_Pressed, this, &AOpenWillowWalker::ToggleSkills);
    Input->BindAction(TEXT("OWReload"), IE_Pressed, this, &AOpenWillowWalker::ReloadPressed);
    Input->BindAction(TEXT("OWFire"), IE_Pressed, this, &AOpenWillowWalker::FirePressed);
    Input->BindAction(TEXT("OWFire"), IE_Released, this, &AOpenWillowWalker::FireReleased);
    Input->BindAction(TEXT("OWPhaselock"), IE_Pressed, this, &AOpenWillowWalker::UsePhaselock);
}
void AOpenWillowWalker::SelectSlot(int32 Slot)
{
    if (!bMayaActive || !ArmsAnim) return;
    const FOpenWillowWeaponItem* Item = Inventory->SlotItem(Slot);
    if (!Item) return; // an empty slot keeps the current weapon, as in BL2
    Inventory->SetActiveSlot(Slot);
    bWeaponOut = true;
    bFireHeld = false;
    CancelReload();
    bOutOfAmmoLogged = false;
    // Each rolled weapon imports as SK_<recipe id>; fall back to the older
    // single Infinity assembly when that item has not been imported.
    const FString ItemMesh = FString::Printf(TEXT("/Game/OpenWillow/Weapons/Items/SK_%s.SK_%s"), *Item->Id, *Item->Id);
    USkeletalMesh* WeaponMesh = LoadObject<USkeletalMesh>(nullptr, *ItemMesh);
    if (!WeaponMesh) WeaponMesh = LoadObject<USkeletalMesh>(nullptr, TEXT("/Game/OpenWillow/Weapons/InfinityProxy/SK_InfinityProxy.SK_InfinityProxy"));
    WeaponVisual->SetSkeletalMesh(WeaponMesh);
    WeaponVisual->SetHiddenInGame(WeaponMesh == nullptr || bInventoryPresentation);
    IdleAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Idle"));
    RunAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Run_F"));
    SprintAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Sprint"));
    JumpAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Jump_Idle"));
    LandAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Jump_End"));
    ArmsAnim->SetClips(IdleAnim, RunAnim, SprintAnim, JumpAnim, LandAnim);
    if (DrawPistolAnim) ArmsAnim->PlayAction(DrawPistolAnim);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya equipped slot %d: %s (rarity %d, %.0f dmg, %.1f/s)"),
        Slot + 1, *Item->Name, Item->Rarity, Item->Damage, Item->FireRate);
}
bool AOpenWillowWalker::EquipItem(int32 Item, int32 Slot)
{
    if (!Inventory->Items().IsValidIndex(Item) || !Skills || Inventory->Items()[Item].Level > Skills->GetLevel()) return false;
    const int32 HeldSlot = Inventory->GetActiveSlot();
    const FOpenWillowWeaponItem* PreviouslyHeld = Inventory->ActiveWeapon();
    if (!Inventory->Equip(Item, Slot)) return false;
    // A slot swap can change the weapon in the active slot even when the
    // requested destination is another slot. Refresh the visual in that case.
    if (!bWeaponOut || HeldSlot == Slot || !Inventory->ActiveWeapon()) SelectSlot(Slot);
    else if (Inventory->ActiveWeapon() != PreviouslyHeld) SelectSlot(HeldSlot);
    return true;
}
bool AOpenWillowWalker::UnequipSlot(int32 Slot)
{
    if (!Inventory || !Inventory->SlotItem(Slot)) return false;
    const bool bWasActive = Inventory->GetActiveSlot() == Slot;
    if (!Inventory->Unequip(Slot)) return false;
    if (bWasActive)
    {
        bWeaponOut = false;
        bFireHeld = false;
        CancelReload();
        if (WeaponVisual) WeaponVisual->SetHiddenInGame(true);
    }
    return true;
}
bool AOpenWillowWalker::TakeInventoryItemById(const FString& Id, FOpenWillowTakenInventoryItem& OutItem)
{
    if (!Inventory) return false;
    const FOpenWillowWeaponItem* Active = Inventory->ActiveWeapon();
    const bool bWasActiveWeapon = Active && UOpenWillowInventory::StableId(*Active) == Id;
    if (!Inventory->TakeById(Id, OutItem)) return false;
    if (bWasActiveWeapon)
    {
        bWeaponOut = false;
        bFireHeld = false;
        CancelReload();
        if (WeaponVisual) WeaponVisual->SetHiddenInGame(true);
    }
    return true;
}
void AOpenWillowWalker::PickupNearby()
{
    if (!bMayaActive || bInventoryPresentation || !Inventory || !GetWorld()) return;
    AOpenWillowInventoryPickup* Nearest = nullptr;
    float BestDistanceSquared = FMath::Square(220.f);
    for (TActorIterator<AOpenWillowInventoryPickup> It(GetWorld()); It; ++It)
    {
        const float DistanceSquared = FVector::DistSquared(GetActorLocation(), It->GetActorLocation());
        if (DistanceSquared >= BestDistanceSquared) continue;
        BestDistanceSquared = DistanceSquared;
        Nearest = *It;
    }
    if (!Nearest) return;
    const FString Name = Nearest->DisplayName();
    const bool bPickedUp = Nearest->TryPickup(Inventory);
    ++PickupAttempts;
    bLastPickupAccepted = bPickedUp;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow pickup %s: %s"), *Name,
        bPickedUp ? TEXT("accepted") : TEXT("rejected: backpack full"));
}
void AOpenWillowWalker::SetInventoryPresentation(bool bShow)
{
    bInventoryPresentation = bShow;
    if (Arms) Arms->SetHiddenInGame(bShow);
    if (WeaponVisual) WeaponVisual->SetHiddenInGame(bShow || !bWeaponOut);
}
void AOpenWillowWalker::ToggleInventory()
{
    APlayerController* PC = Cast<APlayerController>(Controller);
    if (!bMayaActive || !PC) return;
    bFireHeld = false;
    if (AOpenWillowMayaHUD* HUD = Cast<AOpenWillowMayaHUD>(PC->GetHUD()))
    {
        if (HUD->ToggleInventory()) return;
        HUD->CloseSkills();
    }
    if (InventoryScreen && InventoryScreen->IsInViewport())
    {
        InventoryScreen->RemoveFromParent();
        PC->SetInputMode(FInputModeGameOnly());
        PC->SetShowMouseCursor(false);
        return;
    }
    if (!InventoryScreen) InventoryScreen = CreateWidget<UOpenWillowInventoryWidget>(PC);
    InventoryScreen->Bind(this);
    InventoryScreen->AddToViewport(10);
    InventoryScreen->Refresh();
    InventoryScreen->SetIsFocusable(true);
    FInputModeUIOnly Mode;
    Mode.SetWidgetToFocus(InventoryScreen->TakeWidget());
    PC->SetInputMode(Mode);
    PC->SetShowMouseCursor(true);
}
void AOpenWillowWalker::ToggleSkills()
{
    APlayerController* PC = Cast<APlayerController>(Controller);
    if (!bMayaActive || !PC) return;
    bFireHeld = false;
    if (InventoryScreen && InventoryScreen->IsInViewport())
    {
        InventoryScreen->RemoveFromParent();
        PC->SetInputMode(FInputModeGameOnly());
        PC->SetShowMouseCursor(false);
    }
    if (AOpenWillowMayaHUD* HUD = Cast<AOpenWillowMayaHUD>(PC->GetHUD())) HUD->ToggleSkills();
}
void AOpenWillowWalker::Holster()
{
    if (!bMayaActive || !ArmsAnim) return;
    bWeaponOut = false;
    Inventory->SetActiveSlot(INDEX_NONE);
    bFireHeld = false;
    CancelReload();
    WeaponVisual->SetHiddenInGame(true);
    IdleAnim = LoadArmsAnim(TEXT("Unarmed"), TEXT("Idle"));
    RunAnim = LoadArmsAnim(TEXT("Unarmed"), TEXT("Run_F"));
    SprintAnim = LoadArmsAnim(TEXT("Unarmed"), TEXT("Sprint"));
    JumpAnim = LoadArmsAnim(TEXT("Unarmed"), TEXT("Jump_Idle"));
    LandAnim = LoadArmsAnim(TEXT("Unarmed"), TEXT("Jump_End"));
    ArmsAnim->SetClips(IdleAnim, RunAnim, SprintAnim, JumpAnim, LandAnim);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya holstered her weapon"));
}
void AOpenWillowWalker::FirePressed()
{
    const FOpenWillowWeaponItem* Weapon = Inventory->ActiveWeapon();
    if (!bMayaActive || !bWeaponOut || !Weapon) return;
    bFireHeld = true;
    // BSM_SpinUpToFullFireRate (the Vladof pistol type) fires at once while the
    // barrel spins up; the name reads as a fire-interval ramp from
    // StartingSpinUpFireIntervalMultiplier x interval down to the interval.
    // The Infinity's multiplier is the class default 1, so that ramp is flat
    // and is not modelled. Other modes keep the older wait-for-spin-up guess.
    // Both readings are UNVERIFIED against the native weapon code.
    if (Weapon->SpinMode != TEXT("BSM_SpinUpToFullFireRate"))
        NextShotAt = FMath::Max(NextShotAt, GetWorld()->GetTimeSeconds() + Weapon->SpinUp);
    else if (Weapon->SpinStartIntervalScale != 1.f)
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow %s: spin-up fire-rate ramp (start x%.2f) is not modelled"),
            *Weapon->Name, Weapon->SpinStartIntervalScale);
}
void AOpenWillowWalker::FireReleased() { bFireHeld = false; }
void AOpenWillowWalker::ReloadPressed()
{
    if (bMayaActive && bWeaponOut && !bInventoryPresentation) StartReload(GetWorld()->GetTimeSeconds());
}
bool AOpenWillowWalker::StartReload(float Now)
{
    const FOpenWillowWeaponItem* Weapon = Inventory->ActiveWeapon();
    if (bReloading || !Weapon || !Inventory->CanReload(*Weapon)) return false;
    // The evaluated reload time is the whole reload; no reload animation is
    // played yet (the imported pistol reload clip is not wired up).
    bReloading = true;
    ReloadEndsAt = Now + FMath::Max(0.1f, Weapon->ReloadTime);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow reload started: %s, %.1f s"), *Weapon->Name, Weapon->ReloadTime);
    return true;
}
void AOpenWillowWalker::CancelReload() { bReloading = false; }
void AOpenWillowWalker::FireWeapon()
{
    // ADD_Fire_Recoil is a UE3 additive clip; layer it on the held pose.
    if (ArmsAnim && FirePistolAnim) ArmsAnim->PlayAdditive(FirePistolAnim, 0.45f);
    // A deterministic figure-eight is a visual proxy for the installed
    // FiringPatternLines array, whose serialized elements remain unsupported.
    const float Phase = ShotCount++ * (PI / 8.f);
    const float Spread = FMath::Tan(FMath::DegreesToRadians(2.1f));
    const FVector ForwardAim = Camera->GetForwardVector();
    const FVector Direction = (ForwardAim + Spread * (FMath::Sin(Phase) * Camera->GetRightVector()
        + 0.5f * FMath::Sin(2.f * Phase) * Camera->GetUpVector())).GetSafeNormal();
    const FVector Start = Camera->GetComponentLocation();
    const FVector End = Start + Direction * 10000.f;
    FHitResult Hit;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowInfinity), true, this);
    const bool bHit = GetWorld()->LineTraceSingleByChannel(Hit, Start, End, ECC_Visibility, Query);
    const FVector Impact = bHit ? Hit.ImpactPoint : End;
    const FVector Muzzle = MuzzleLocation();
    AOpenWillowShotFx::Flash(GetWorld(), Muzzle, FLinearColor(1.f, 0.5f, 0.15f), 3.5f, 0.05f, 40.f);
    AOpenWillowShotFx::Tracer(GetWorld(), Muzzle + (Impact - Muzzle).GetSafeNormal() * 30.f, Impact);
    if (!bHit) return;
    AOpenWillowShotFx::Flash(GetWorld(), Impact + Hit.ImpactNormal * 2.f,
        FLinearColor(1.f, 0.75f, 0.35f), 5.f, 0.3f, 60.f);
    AOpenWillowCombatTarget* Target = Cast<AOpenWillowCombatTarget>(Hit.GetActor());
    if (!Target)
    {
        if (UMaterialInterface* Hole = LoadObject<UMaterialInterface>(nullptr,
            TEXT("/Game/OpenWillow/Weapons/InfinityProxy/M_OW_BulletHole.M_OW_BulletHole")))
            UGameplayStatics::SpawnDecalAtLocation(GetWorld(), Hole, FVector(6.f, 7.f, 7.f), Impact,
                (-Hit.ImpactNormal).Rotation(), 12.f);
        return;
    }
    {
        // Evaluated item-card damage (tools/weapon_stats.py); no crits, element
        // or target resistances yet.
        const FOpenWillowWeaponItem* Weapon = Inventory->ActiveWeapon();
        UGameplayStatics::ApplyPointDamage(Target, Weapon ? Weapon->Damage : 0.f, Direction, Hit,
            GetController(), this, UDamageType::StaticClass());
        TargetHitAt = GetWorld()->GetTimeSeconds();
    }
}
void AOpenWillowWalker::UsePhaselock()
{
    if (!bMayaActive) return;
    // Phaselock needs its skill point, as in the game, where the action skill
    // is bought in the Skills tab (traced). What the game does when the key is
    // pressed before that (nothing, a message or a sound) is UNVERIFIED.
    if (!Skills || Skills->GetActionGrade() < 1) return;
    const float Now = GetWorld()->GetTimeSeconds();
    if (Now < PhaselockReadyAt) return;
    const FVector Start = Camera->GetComponentLocation();
    FHitResult Hit;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowPhaselock), true, this);
    // A small sphere sweep gives the aim assist a skill cast needs.
    if (!GetWorld()->SweepSingleByChannel(Hit, Start, Start + Camera->GetForwardVector() * 2500.f,
        FQuat::Identity, ECC_Visibility, FCollisionShape::MakeSphere(30.f), Query))
    {
        if (ArmsAnim && PhaselockFailAnim) ArmsAnim->PlayAction(PhaselockFailAnim);
        return;
    }
    AOpenWillowCombatTarget* Target = Cast<AOpenWillowCombatTarget>(Hit.GetActor());
    // ActionSkill_Phaselock.LockDurationFormula -> Att_Phaselock_Duration
    // base 5 s, times PhaselockTimeScale (default 1). Skill mods not applied.
    if (!Target || !Target->BeginPhaselock(Now, 5.f))
    {
        if (ArmsAnim && PhaselockFailAnim) ArmsAnim->PlayAction(PhaselockFailAnim);
        return;
    }
    if (ArmsAnim && PhaselockAnim) ArmsAnim->PlayAction(PhaselockAnim);
    // Host cast cue: Tick draws a violet beam from Maya's raised left hand to
    // the target for the lift, so it follows the Phase_Lock_Lift pose.
    PhaselockBeamUntil = Now + 0.9f;
    PhaselockReadyAt = Now + PhaselockCooldownSeconds;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya Phaselock activated on %s"), *Target->GetName());
}
float AOpenWillowWalker::PhaselockRemaining() const
{
    return GetWorld() ? FMath::Max(0.f, PhaselockReadyAt - GetWorld()->GetTimeSeconds()) : 0.f;
}
void AOpenWillowWalker::RunCombatShots(float Now)
{
    // Give the imported inventory the same loading allowance as Skills.
    FString InventoryMovieUrl;
    if (CombatShotStep >= 16 && FParse::Value(FCommandLine::Get(), TEXT("owflashinventory="), InventoryMovieUrl)) Now -= 15.f;
    auto Shot = [](const TCHAR* Name)
    {
        FScreenshotRequest::RequestScreenshot(FString::Printf(TEXT("OWCombat_%s.png"), Name), true, false);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow combat capture %s"), Name);
    };
    auto Turn = [this](float Yaw, float Pitch)
    {
        Controller->SetControlRotation(Controller->GetControlRotation() + FRotator(Pitch, Yaw, 0));
    };
    const FVector TargetPoint = CombatTarget.IsValid() ? CombatTarget->AimPoint() : FVector::ZeroVector;
    // Each step runs once, in order, at its world time in seconds.
    switch (CombatShotStep)
    {
    case 0: if (Now < 7.f || !CombatTarget.IsValid()) return; AimAt(TargetPoint); break;
    case 1: if (Now < 8.f) return; Shot(TEXT("1_Idle")); break;
    case 2: if (Now < 8.5f) return; FirePressed(); break;
    case 3: if (Now < 9.55f) return; Shot(TEXT("2_Firing")); break;
    case 4: if (Now < 9.8f) return; FireReleased(); break;
    case 5: if (Now < 10.5f) return; AimAt(TargetPoint); UsePhaselock(); break;
    case 6: if (Now < 11.1f) return; Shot(TEXT("3_PhaselockCast")); break;
    case 7: if (Now < 12.2f) return; AimAt(TargetPoint); FirePressed(); break;
    case 8: if (Now < 13.3f) return; Shot(TEXT("4_FiringLocked")); break;
    case 9: if (Now < 13.5f) return; FireReleased(); Turn(35.f, 0.f); break;
    case 10: if (Now < 14.f) return; FirePressed(); break;
    case 11: if (Now < 15.1f) return; Shot(TEXT("5_FiringWall")); break;
    case 12: if (Now < 15.3f) return; FireReleased(); break;
    case 13: if (Now < 15.6f) return; Turn(-35.f, 0.f); SelectSlot(3); break;
    case 14: if (Now < 16.6f) return; Shot(TEXT("6_Slot4")); break;
    case 15: if (Now < 17.f) return; ToggleInventory(); if (InventoryScreen) InventoryScreen->ShowCard(4); break;
    case 16: if (Now < 17.8f) return; Shot(TEXT("7_Inventory"));
        UE_LOG(LogTemp, Display, TEXT("OpenWillow inventory snapshot: %s"), *Inventory->StateJson(Skills->GetLevel())); break;
    // Drive the imported page with the same keys a player would press: Down
    // selects the first backpack item, E compares it with the equipped one,
    // F toggles inspect.
    case 17: if (Now < 18.3f) return; SendInventoryKey(TEXT("ArrowDown")); break;
    case 18: if (Now < 18.8f) return; SendInventoryKey(TEXT("e")); break;
    case 19: if (Now < 19.8f) return; Shot(TEXT("7b_InventoryCompare")); break;
    case 20: if (Now < 20.2f) return; SendInventoryKey(TEXT("e")); SendInventoryKey(TEXT("f")); break;
    case 21: if (Now < 21.2f) return; Shot(TEXT("7c_InventoryInspect")); break;
    case 22: if (Now < 21.6f) return; SendInventoryKey(TEXT("f")); break;
    // Exercise the real reload path: hold the slot-2 gun with a nearly empty
    // magazine, fire until it auto-reloads from the pool, then log the state.
    case 23: if (Now < 22.f) return; SelectSlot(1);
        if (FOpenWillowWeaponItem* Held = Inventory->ActiveWeaponMutable()) Held->MagazineLeft = 2;
        FirePressed(); break;
    case 24: if (Now < 30.f) return; FireReleased();
        UE_LOG(LogTemp, Display, TEXT("OpenWillow inventory snapshot after firing: %s"), *Inventory->StateJson(Skills->GetLevel())); break;
    case 25: if (Now < 30.3f) return;
        if (FParse::Param(FCommandLine::Get(), TEXT("owskillshots"))) ToggleSkills();
        else if (APlayerController* PC = Cast<APlayerController>(Controller)) PC->ConsoleCommand(TEXT("quit"));
        break;
    // CEF loads the StatusMenu plus shared imports and thirty icon movies;
    // allow it to reach the populated frame before the visual capture.
    case 26: if (Now < 33.f) return; Shot(TEXT("8_Skills")); break;
    case 27: if (Now < 34.f) return;
        if (APlayerController* PC = Cast<APlayerController>(Controller))
            if (AOpenWillowMayaHUD* HUD = Cast<AOpenWillowMayaHUD>(PC->GetHUD())) HUD->RequestSkillsCloseFromPage();
        break;
    case 28: if (Now < 35.f) return; Shot(TEXT("9_AfterSkills")); break;
    case 29: if (Now < 35.5f) return;
        if (APlayerController* PC = Cast<APlayerController>(Controller)) PC->ConsoleCommand(TEXT("quit"));
        break;
    default: return;
    }
    ++CombatShotStep;
}
void AOpenWillowWalker::SendInventoryKey(const TCHAR* Key)
{
    // Capture-only: same key events inventory.js listens for on window.
    if (APlayerController* PC = Cast<APlayerController>(Controller))
        if (AOpenWillowMayaHUD* HUD = Cast<AOpenWillowMayaHUD>(PC->GetHUD())) HUD->SendPageKey(Key);
}
void AOpenWillowWalker::SpawnCombatTarget()
{
    // Stand the dummy on whatever floor is 650 cm ahead of the pawn.
    const FVector Ahead = GetActorLocation() + FRotator(0, GetControlRotation().Yaw, 0).Vector() * 650.f;
    FHitResult Floor;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowTargetFloor), true, this);
    const float FeetZ = GetActorLocation().Z - GetCapsuleComponent()->GetScaledCapsuleHalfHeight();
    FVector Location(Ahead.X, Ahead.Y, FeetZ);
    if (GetWorld()->LineTraceSingleByChannel(Floor, Ahead + FVector(0, 0, 300), Ahead - FVector(0, 0, 600),
        ECC_Visibility, Query)) Location = Floor.ImpactPoint;
    FActorSpawnParameters Params;
    Params.Owner = this;
    const FRotator Facing(0, GetControlRotation().Yaw + 180.f, 0);
    CombatTarget = GetWorld()->SpawnActor<AOpenWillowCombatTarget>(Location, Facing, Params);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target spawned at %s"), *Location.ToString());
}
void AOpenWillowWalker::AimAt(const FVector& Point)
{
    if (Controller) Controller->SetControlRotation((Point - Camera->GetComponentLocation()).Rotation());
}
FVector AOpenWillowWalker::MuzzleLocation() const
{
    // Pistol_Barrel_Vladof's gestalt bounds end ~27 cm ahead of its Barrel
    // bone. Projecting along the view approximates the unhosted Muzzle socket.
    if (WeaponVisual->GetSkinnedAsset() && WeaponVisual->GetBoneIndex(TEXT("Barrel")) != INDEX_NONE)
        return WeaponVisual->GetBoneLocation(TEXT("Barrel")) + Camera->GetForwardVector() * 27.f;
    return Camera->GetComponentLocation() + Camera->GetForwardVector() * 60.f
        + Camera->GetRightVector() * 12.f - Camera->GetUpVector() * 10.f;
}
void AOpenWillowWalker::SprintPressed()
{
    bSprintHeld = true;
    GetCharacterMovement()->MaxWalkSpeed = 650.f; // estimated pending BL2 timing measurement
    UE_LOG(LogTemp, Display, TEXT("OpenWillow sprint on, max speed 650 cm/s"));
}
void AOpenWillowWalker::SprintReleased()
{
    bSprintHeld = false;
    GetCharacterMovement()->MaxWalkSpeed = 450.f;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow sprint off, max speed 450 cm/s"));
}
void AOpenWillowWalker::Forward(float Value) { AddMovementInput(FRotator(0, GetControlRotation().Yaw, 0).Vector(), Value); }
void AOpenWillowWalker::Right(float Value) { AddMovementInput(FRotationMatrix(FRotator(0, GetControlRotation().Yaw, 0)).GetUnitAxis(EAxis::Y), Value); }
void AOpenWillowWalker::Turn(float Value) { LookInput.X += Value; AddControllerYawInput(Value); }
void AOpenWillowWalker::Look(float Value) { LookInput.Y += Value; AddControllerPitchInput(Value); }
