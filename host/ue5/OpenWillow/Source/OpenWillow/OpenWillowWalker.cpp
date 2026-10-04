#include "OpenWillowWalker.h"
#include "OpenWillowMover.h"
#include "OpenWillowQuest.h"
#include "Engine/DamageEvents.h"
#include "GameFramework/WorldSettings.h"
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
#include "Materials/MaterialInstanceDynamic.h"
#include "Engine/Texture2D.h"
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
    Mover = CreateDefaultSubobject<UOpenWillowMover>(TEXT("InstalledMover"));
    Quest = CreateDefaultSubobject<UOpenWillowQuest>(TEXT("SliceQuest"));
    GetCharacterMovement()->MaxWalkSpeed = 450;
    GetCharacterMovement()->JumpZVelocity = 420;
    GetCharacterMovement()->MaxStepHeight = 35;
    GetCharacterMovement()->SetWalkableFloorAngle(45);
}

float AOpenWillowWalker::TakeDamage(float DamageAmount, const FDamageEvent& DamageEvent, AController* EventInstigator, AActor* DamageCauser)
{
    if (!bMayaActive && !bRespawnPointCaptured) return 0.f;
    const float Applied = FMath::Clamp(DamageAmount, 0.f, Health);
    Health -= Applied;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya took %.0f damage, health %.0f/%.0f"), Applied, Health, MaxHealth);
    // Going down while a target is held: Skill_Phaselock's while-active HealthState constraint fails and deactivates the
    // skill, which ends the lock (native reading, UNVERIFIED). The host has no injured state: health 0 is death.
    FString Failed;
    if (Health <= 0.f && IsPhaselockActive() && !Phaselock.GateOpen(PhaselockGateState(), false, Failed))
        EndPhaselockEarly(TEXT("skill constraint ") + Failed);
    if (Health <= 0.f) Respawn();
    return Applied;
}

void AOpenWillowWalker::Respawn()
{
    // With -owquest the slice data picks the station exit point (decoded selection rule); otherwise the host
    // stand-in: back to where this session started. Full health either way; mission state is not touched.
    Health = MaxHealth;
    bFireHeld = false;
    CancelReload();
    FVector Location = RespawnLocation;
    FRotator Rotation = RespawnRotation;
    FTransform Station;
    if (Quest && Quest->Enabled() && Quest->RespawnPoint(GetActorLocation(), Station))
    {
        Location = Station.GetLocation();
        Rotation = Station.Rotator();
    }
    SetActorLocationAndRotation(Location, Rotation, false, nullptr, ETeleportType::TeleportPhysics);
    if (Controller) Controller->SetControlRotation(Rotation);
    GetCharacterMovement()->StopMovementImmediately();
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya died and respawned at %s"), *Location.ToString());
    if (Quest) Quest->NotifyRespawn();
}

void AOpenWillowWalker::RefreshHealthForLevel()
{
    // -owquest: maximum health from the recovered Init_PlayerHealth formula at Maya's level (skills, class mods and
    // relics not applied); otherwise the 400 host stand-in stays.
    float FormulaHealth = 0.f;
    if (!Skills || Skills->GetLevel() == HealthLevel || !Quest || !Quest->Enabled()
        || !Quest->PlayerMaxHealth(Skills->GetLevel(), FormulaHealth))
        return;
    // Current health becomes the new maximum, at start and on every level change. For a level-up this follows the
    // installed data: WillowPlayerController.OnExpLevelChange (script) calls the native
    // RecalculateAttributeInitializedState, then, with bFeedback, runs PlayerClassDefinition.OnLevelUp
    // (GD_PlayerShared.Behaviors.PlayerBehavior_LevelUp), whose SkillDefinition_0 adds HealthMaxValue x 1 to
    // HealthCurrentValue (MT_PostAdd). UNVERIFIED (native): that the pool clamps the sum at the new maximum and the
    // timed effect acts once as a heal; OnExpLevelChange's 1 s guard (LastLevelUpTime) is not modelled; a level that
    // goes down (test fixtures only) is treated the same.
    const int32 FromLevel = HealthLevel;
    MaxHealth = Health = FormulaHealth;
    HealthLevel = Skills->GetLevel();
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya health %.1f/%.1f for level %d (was level %d)"), Health, MaxHealth, HealthLevel, FromLevel);
}

void AOpenWillowWalker::BeginPlay()
{
    Super::BeginPlay();
    RespawnLocation = GetActorLocation();
    RespawnRotation = GetActorRotation();
    bRespawnPointCaptured = true;
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
    // Phaselock numbers from tools/prepare_action_skill.py's manifest; without it the skill is unavailable.
    PhaselockFile = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/character/action_skill_siren.json")));
    FParse::Value(FCommandLine::Get(), TEXT("owactionskill="), PhaselockFile);
    FString PhaselockError;
    if (!Phaselock.Load(PhaselockFile, PhaselockError))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow Phaselock unavailable: %s"), *PhaselockError);
    }
    else
    {
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock data: lift %.2f s, lock %.2f x scale %.2f, fade %.2f s, buffer %.2f s, cooldown %.1f s (held rate %.2f), diminishing x%.2f for %.0f s, %s +%d grades"),
            Phaselock.LiftDuration, Phaselock.LockDurationBase, Phaselock.TimeScaleDefault, Phaselock.LockFadeOutTime, Phaselock.ReleaseBufferTime,
            Phaselock.CooldownSeconds, Phaselock.CooldownHeldRate, 1.f + Phaselock.DiminishingScale, Phaselock.DiminishingSeconds,
            *Phaselock.DurationSkill, FMath::Max(0, Phaselock.DurationPostAdd.Num() - 1));
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock targeting %.0f-%.0f uu (auto-aim data), lift if %s; constraints evaluated [%s], not evaluated [%s]"),
            Phaselock.TargetMinDistance, Phaselock.TargetMaxDistance, *Phaselock.CanLiftFlag,
            *FString::Join(Phaselock.GateEvaluators, TEXT(", ")), *FString::Join(Phaselock.GateNotEvaluated, TEXT(", ")));
    }
    // Phaselock presentation numbers (tools/prepare_phaselock_fx.py); without them no stock effect is drawn.
    FString FxFile = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/phaselock/fx_manifest.json")));
    FParse::Value(FCommandLine::Get(), TEXT("owphaselockfx="), FxFile);
    FString FxError;
    if (!PhaselockFx.Load(FxFile, FxError))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow Phaselock presentation unavailable: %s"), *FxError);
    }
    else
    {
        // Load every template's materials and meshes now (and wait for their shaders in editor builds) so that the first
        // cast neither hitches nor draws its translucent quads late (FOwFxTemplate::Preload).
        int32 Materials = 0;
        for (const FString& Name : {PhaselockFx.HandHitTemplate, PhaselockFx.HandMissTemplate, PhaselockFx.ScreenTemplate,
                 PhaselockFx.BubbleFadeIn, PhaselockFx.BubbleLoop, PhaselockFx.BubbleFadeOut})
        {
            FString TemplateError;
            if (const FOwFxTemplate* Template = FOwFxTemplate::Load(PhaselockFx.EmitterDir, Name, TemplateError))
                Materials += Template->Preload(PhaselockFxAssets);
            else UE_LOG(LogTemp, Warning, TEXT("OpenWillow Phaselock presentation: %s"), *TemplateError);
        }
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock presentation: hand %s at %s+%.2f s (miss %.2f s), bubble /%.1f collapse %.2f over %.1f s, light r%.0f b%.1f, glow %d keys over %.1f s, screen %s; %d emitter materials preloaded"),
            *PhaselockFx.HandHitTemplate, *PhaselockFx.HandBone.ToString(), PhaselockFx.LiftNotifyTime, PhaselockFx.FailNotifyTime,
            PhaselockFx.BubbleScaleDivisor, PhaselockFx.MaxCollapse, PhaselockFx.CollapseDuration, PhaselockFx.LightRadius,
            PhaselockFx.LightBrightness, PhaselockFx.GlowPoints.Num(), PhaselockFx.GlowDuration, *PhaselockFx.ScreenTemplate, Materials);
    }
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
    Bl2FovSetting = Bl2Fov;
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
    // No clip set yet: the arms would be in their bind pose, without the clips' root correction (presumably out of
    // view; not checked), so they stay hidden until a weapon is drawn. What the original shows with no weapon is not
    // observed: UNVERIFIED.
    UpdateArmsVisibility();
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
    // -owquest: a quest save's progression block (level, experience, skill grades) replaces the start level above,
    // before the level gates which weapons are equipped. Saves without the block keep the start level.
    if (Quest && Quest->Enabled()) Quest->RestoreProgression(*Skills);

    FString GearManifestPath = FPaths::ConvertRelativePathToFull(
        FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/inventory/gear_manifest.json")));
    FParse::Value(FCommandLine::Get(), TEXT("owgear="), GearManifestPath);
    Inventory->LoadGearManifest(GearManifestPath);
    // The host's new-session purse starts at zero (no BL2 save is imported).
    // Explicit -owmoney / -owerid values and the capture fixture override it.
    Inventory->SetMoney(0);
    Inventory->SetEridium(0);
    // -owslots=<2..4> overrides the prototype's slot availability.
    // -owinventoryselftest runs the synthetic checks.
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
    RefreshHealthForLevel();
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya level %d, %d skill points, action grade %d, health %.1f"),
        Skills->GetLevel(), Skills->AvailablePoints(), Skills->GetActionGrade(), MaxHealth);
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
    if (bMayaActive)
    {
        // Skill constraints flagged while active (healthy, on foot) end a running Phaselock when they fail.
        FString Failed;
        if (IsPhaselockActive() && !Phaselock.GateOpen(PhaselockGateState(), false, Failed))
            EndPhaselockEarly(TEXT("skill constraint ") + Failed);
        UpdatePhaselockPresentation(Now);
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
    static const bool bPhaselockShots = FParse::Param(FCommandLine::Get(), TEXT("owphaselockshots"));
    if (bPhaselockShots && bMayaActive && Controller) RunPhaselockShots(Now);
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
    // Each rolled weapon imports as SK_<recipe id> (or names its mesh); fall back to the older
    // single Infinity assembly when that item has not been imported.
    USkeletalMesh* WeaponMesh = UOpenWillowInventory::LoadWeaponMesh(*Item);
    if (!WeaponMesh) WeaponMesh = LoadObject<USkeletalMesh>(nullptr, TEXT("/Game/OpenWillow/Weapons/InfinityProxy/SK_InfinityProxy.SK_InfinityProxy"));
    UE_LOG(LogTemp, Display, TEXT("OpenWillow weapon mesh for %s: %s"), *Item->Id, WeaponMesh ? *WeaponMesh->GetPathName() : TEXT("none"));
    WeaponVisual->SetSkeletalMesh(WeaponMesh);
    WeaponVisual->SetHiddenInGame(WeaponMesh == nullptr || bInventoryPresentation);
    IdleAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Idle"));
    RunAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Run_F"));
    SprintAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Sprint"));
    JumpAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Jump_Idle"));
    LandAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Jump_End"));
    ArmsAnim->SetClips(IdleAnim, RunAnim, SprintAnim, JumpAnim, LandAnim);
    UpdateArmsVisibility();
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
bool AOpenWillowWalker::LendWeapon(const FOpenWillowWeaponItem& Item)
{
    if (!Inventory || Inventory->FindItemIndexById(Item.Id) != INDEX_NONE || !Inventory->AddToBackpack(Item)) return false;
    const int32 Index = Inventory->FindItemIndexById(Item.Id);
    // Where the game puts a lent weapon is native (MissionTracker) and not observed. Host rule (UNVERIFIED): the first
    // empty unlocked slot, else the last unlocked slot (its weapon goes back to the backpack); drawn at once. The
    // level requirement is not checked for a lent weapon.
    int32 Slot = Inventory->GetWeaponSlotsUnlocked() - 1;
    for (int32 Candidate = 0; Candidate < Inventory->GetWeaponSlotsUnlocked(); ++Candidate)
        if (!Inventory->SlotItem(Candidate)) { Slot = Candidate; break; }
    PreLendSlot = Inventory->GetActiveSlot();
    if (Index == INDEX_NONE || !Inventory->Equip(Index, Slot)) return false;
    SelectSlot(Slot);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow lent weapon %s (%s, level %d, %.1f damage, %s) in slot %d"), *Item.Id, *Item.Name,
        Item.Level, Item.Damage, *Item.DamageType, Slot + 1);
    return true;
}
bool AOpenWillowWalker::ReturnLentWeapon(const FString& Id)
{
    FOpenWillowTakenInventoryItem Taken;
    if (!TakeInventoryItemById(Id, Taken)) return false;
    // Draw what was held before the lend, else the first equipped weapon (host rule, UNVERIFIED).
    int32 Slot = Inventory->SlotItem(PreLendSlot) ? PreLendSlot : INDEX_NONE;
    for (int32 Candidate = 0; Slot == INDEX_NONE && Candidate < UOpenWillowInventory::SlotCount; ++Candidate)
        if (Inventory->SlotItem(Candidate)) Slot = Candidate;
    if (Slot != INDEX_NONE) SelectSlot(Slot);
    else Holster();
    UE_LOG(LogTemp, Display, TEXT("OpenWillow returned lent weapon %s; holding slot %d"), *Id, Slot + 1);
    return true;
}
bool AOpenWillowWalker::DrawItemById(const FString& Id)
{
    for (int32 Slot = 0; Slot < UOpenWillowInventory::SlotCount; ++Slot)
        if (const FOpenWillowWeaponItem* Item = Inventory->SlotItem(Slot); Item && UOpenWillowInventory::StableId(*Item) == Id)
        {
            SelectSlot(Slot);
            return true;
        }
    return false;
}
bool AOpenWillowWalker::FireOnce()
{
    FOpenWillowWeaponItem* Weapon = Inventory ? Inventory->ActiveWeaponMutable() : nullptr;
    if (!bMayaActive || !bWeaponOut || !Weapon || bReloading || !Inventory->ConsumeShot(*Weapon)) return false;
    FireWeapon();
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
    if (Quest && Quest->TryUse()) return;   // talk to Marcus (slice mission) when in reach
    if (Mover && Mover->TryInteract()) return;
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
    UpdateArmsVisibility();
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
    UpdateArmsVisibility();
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya holstered her weapon (arms %s)"), AreArmsShown() ? TEXT("shown") : TEXT("hidden: no Unarmed clips imported"));
}
FString AOpenWillowWalker::HeldWeaponMesh() const
{
    return WeaponVisual && bWeaponOut && WeaponVisual->GetSkinnedAsset() ? WeaponVisual->GetSkinnedAsset()->GetPathName() : FString();
}
bool AOpenWillowWalker::AreArmsShown() const
{
    return Arms && Arms->GetSkinnedAsset() && !bInventoryPresentation && IdleAnim;
}
void AOpenWillowWalker::UpdateArmsVisibility()
{
    if (Arms) Arms->SetHiddenInGame(bInventoryPresentation || !IdleAnim);
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
        // The shot carries the item's stock damage type path (card damage_type); the target reads it while the
        // damage is applied. Damage source (DamageSource output) is not passed: its stock value is not decoded.
        ShotDamageTypeInFlight = Weapon ? Weapon->DamageType : FString();
        UE_LOG(LogTemp, Display, TEXT("OpenWillow shot %s hits %s (damage type %s)"), Weapon ? *Weapon->Id : TEXT("-"),
            *Target->GetName(), ShotDamageTypeInFlight.IsEmpty() ? TEXT("None") : *ShotDamageTypeInFlight);
        UGameplayStatics::ApplyPointDamage(Target, Weapon ? Weapon->Damage : 0.f, Direction, Hit,
            GetController(), this, UDamageType::StaticClass());
        ShotDamageTypeInFlight.Reset();
        TargetHitAt = GetWorld()->GetTimeSeconds();
    }
}
bool AOpenWillowWalker::CanCastPhaselock(FString& OutReason) const
{
    // WillowPlayerController.ServerStartActionSkill (script): the action skill must be in the skill tree (host:
    // Phaselock bought, as traced in the Skills tab), not on cooldown (IsActionSkillOnCooldown: pool value above 0) and
    // not active (here covered by the cooldown, which is full while a target is held). Ladders and vehicle rider seats,
    // which the script also refuses, do not exist in the host. The native activation then applies the
    // Skill_Phaselock.SkillConstraints flagged for activation (GateOpen; native readings, UNVERIFIED).
    if (!Phaselock.bLoaded) { OutReason = TEXT("no Phaselock manifest"); return false; }
    if (!Skills || Skills->GetActionGrade() < 1) { OutReason = TEXT("Phaselock not bought"); return false; }
    if (PhaselockRemaining() > 0.f) { OutReason = TEXT("on cooldown"); return false; }
    FString Failed;
    if (!Phaselock.GateOpen(PhaselockGateState(), true, Failed)) { OutReason = TEXT("skill constraint ") + Failed; return false; }
    return true;
}
FOpenWillowPhaselockGateState AOpenWillowWalker::PhaselockGateState() const
{
    // Host mapping (UNVERIFIED): a weapon in any slot is the pawn's WillowWeapon; holstered (no weapon drawn) is its
    // Inactive state; the host's weapon swaps have no put-away phase, and it has no melee, grenade, weapon
    // restriction, injured state or vehicle, so those inputs stay false. A reload is reported but does not block.
    FOpenWillowPhaselockGateState State;
    for (int32 Slot = 0; Inventory && Slot < UOpenWillowInventory::SlotCount; ++Slot)
        State.bHoldsWeapon |= Inventory->SlotItem(Slot) != nullptr;
    State.bWeaponInactive = State.bHoldsWeapon && !bWeaponOut;
    State.bReloading = bReloading;
    State.bAlive = Health > 0.f;
    return State;
}
bool AOpenWillowWalker::IsPhaselockActive() const
{
    if (!bPhaselockHit || !GetWorld() || PhaselockEndedEarlyAt >= PhaselockCastAt) return false;
    return GetWorld()->GetTimeSeconds() < PhaselockCastAt + PhaselockTimeline.EndSkillAt;
}
void AOpenWillowWalker::EndPhaselockEarly(const FString& Reason)
{
    const float Now = GetWorld()->GetTimeSeconds();
    PhaselockEndedEarlyAt = Now;
    PhaselockEndReason = Reason;
    // The skill ends: the target is released now (OnReleasedTarget turns the cooldown manager off, so the pool drains
    // from here) and the screen particle is hidden (OnActionSkillDeactivated).
    if (AOpenWillowCombatTarget* Target = PhaselockTarget.Get()) Target->EndPhaselockNow(Now);
    PhaselockHeldUntil = FMath::Min(PhaselockHeldUntil, Now);
    if (ScreenFx) ScreenFx->StopEmitting();
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya Phaselock deactivated %.2f s after the cast: %s"), Now - PhaselockCastAt, *Reason);
}
AOpenWillowWalker::FPhaselockAimScore AOpenWillowWalker::ScorePhaselockTarget(const AOpenWillowCombatTarget* Target) const
{
    // NATIVE_PHASELOCK_TARGETING.md section 3, in our own words. Projection: the host camera with BL2's FOV handling
    // (vertical FOV kept from the 4:3 setting), so x and y are fractions of the half-screen height; depth is the
    // view-space distance along the view direction. The target's "location" is its collision centre and its auto-aim
    // radius and aim point are host stand-ins (AOpenWillowCombatTarget::AutoAimRadius / AimPoint).
    FPhaselockAimScore R;
    const FVector View = Camera->GetComponentLocation();
    const FRotationMatrix Axes(GetViewRotation());   // the control rotation: current even right after an aim change
    FVector Centre;
    float Half = 0.f;
    Target->CollisionCentre(Centre, Half);
    const FVector Delta = Centre - View;
    R.Depth = float(FVector::DotProduct(Delta, Axes.GetUnitAxis(EAxis::X)));
    if (R.Depth <= 0.f) { R.Rejected = TEXT("behind the view plane"); return R; }
    const float TanHalfFov = FMath::Tan(FMath::DegreesToRadians(Bl2FovSetting) * 0.5f);
    const float TanHalfVertical = TanHalfFov * 0.75f;
    const float X = float(FVector::DotProduct(Delta, Axes.GetUnitAxis(EAxis::Y))) / R.Depth / TanHalfVertical;
    const float Y = float(FVector::DotProduct(Delta, Axes.GetUnitAxis(EAxis::Z))) / R.Depth / TanHalfVertical;
    R.ScreenOffset = FMath::Sqrt(X * X + Y * Y);
    R.TargetRadius = Target->AutoAimRadius() * Phaselock.RadiusMultiplier / (R.Depth * TanHalfFov);
    // Magnetism radius on a log2 scale of the depth past DistanceOffset; never narrower than the target itself.
    const float LogMax = FMath::Log2(Phaselock.TargetMaxDistance);
    const float Reach = 1.f + LogMax - FMath::Log2(FMath::Max(1.f, R.Depth - Phaselock.DistanceOffset));
    R.MagnetRadius = FMath::Max(Phaselock.MaxSnapAngle * FMath::Max(0.f, Reach) / LogMax, R.TargetRadius);
    if (R.MagnetRadius <= 0.f || R.ScreenOffset > R.MagnetRadius) { R.Rejected = TEXT("outside the magnetism radius"); return R; }
    const float Score = 0.5f * ((2.f - R.Depth / Phaselock.TargetMaxDistance) - R.ScreenOffset / R.MagnetRadius);
    if (R.Depth < Phaselock.TargetMinDistance || R.Depth > Phaselock.TargetMaxDistance) { R.Rejected = TEXT("outside the distance range"); return R; }
    // Line of sight to the aim point: clear, the target itself, or something hard-attached to it.
    FHitResult Hit;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowPhaselockSight), true, this);
    if (GetWorld()->LineTraceSingleByChannel(Hit, View, Target->AimPoint(), ECC_Visibility, Query))
    {
        const AActor* Blocker = Hit.GetActor();
        while (Blocker && Blocker != Target) Blocker = Blocker->GetAttachParentActor();
        if (Blocker != Target)
        {
            R.Rejected = FString::Printf(TEXT("no line of sight (%s)"), Hit.GetActor() ? *Hit.GetActor()->GetName() : TEXT("world"));
            return R;
        }
    }
    R.Score = Score;
    return R;
}
AOpenWillowCombatTarget* AOpenWillowWalker::PreferredPhaselockTarget(FString* OutLog) const
{
    AOpenWillowCombatTarget* Best = nullptr;
    float BestScore = 0.f;
    for (TActorIterator<AOpenWillowCombatTarget> It(GetWorld()); It; ++It)
    {
        // Candidates: live targets in the targetable list. The stock dummy joins it through its installed
        // Behavior_RegisterTargetable (UOpenWillowQuest); host-made targets count as registered (host stand-in).
        if (!It->IsAutoAimTarget()) continue;
        if (It->IsStockPawn() && Quest && Quest->Enabled() && !Quest->IsRegisteredTargetable(*It))
        {
            if (OutLog) *OutLog += FString::Printf(TEXT("%s not in the targetable list; "), *It->GetName());
            continue;
        }
        const FPhaselockAimScore S = ScorePhaselockTarget(*It);
        if (OutLog)
            *OutLog += FString::Printf(TEXT("%s score %.3f (depth %.0f, offset %.3f, magnet %.3f, own %.3f)%s%s; "), *It->GetName(), S.Score,
                S.Depth, S.ScreenOffset, S.MagnetRadius, S.TargetRadius, S.Rejected.IsEmpty() ? TEXT("") : TEXT(" "), *S.Rejected);
        if (S.Score > BestScore) { BestScore = S.Score; Best = *It; }
    }
    return Best;
}
void AOpenWillowWalker::UsePhaselock()
{
    if (!bMayaActive) return;
    // What the game does when the key is pressed while a cast is refused (nothing, a message or a sound) is UNVERIFIED.
    FString Refused;
    if (!CanCastPhaselock(Refused))
    {
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya Phaselock not cast: %s"), *Refused);
        return;
    }
    const float Now = GetWorld()->GetTimeSeconds();
    // ActionSkillCallback refills the cooldown pool at activation, hit or miss.
    PhaselockCastAt = Now;
    PhaselockTimeline = FOpenWillowPhaselockTimeline();
    bPhaselockBlocked = false;
    PhaselockEndReason.Reset();
    // Skill_Phaselock's OnActivated enables the Phaselock_TatooGlow coordinated effect (arms material curve).
    GlowStartedAt = Now;
    // Target choice is native in the game: WillowPlayerController.StartActionSkill asks its auto-aim strategy for the
    // instantaneous preferred target (host reading above).
    FString Candidates;
    AOpenWillowCombatTarget* Target = PreferredPhaselockTarget(&Candidates);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock targeting: %s"), Candidates.IsEmpty() ? TEXT("no candidates") : *Candidates);
    // LiftActionSkill.SelectTarget (script): a target that passes CanPhaseLockTarget is lifted when CanLiftTargetIf
    // (Flag_Skills_CanPhaseLock) holds and it drives no vehicle (the host has none), else blocked; no target, or one
    // that fails CanPhaseLockTarget, fizzles (FizzleOut below).
    if (Target && Target->CanPhaseLockTarget() && !Target->CanPhaseLockFlag())
    {
        // TargetBlocked fires OnTargetBlocked: no lift and no LiftActionSkill timers. That event's Behavior_CauseDamage
        // (Phaselock_Impact) is NOT applied here: its amount is not recovered. Nothing resets the cooldown (no
        // Fizzled), so the pool refilled at activation drains at the base rate from the cast (read from script and
        // data, never run: UNVERIFIED). Its effects (Part_PhaseLock_EnemyCannotBeLocked) are not drawn by the host.
        bPhaselockHit = false;
        bPhaselockBlocked = true;
        PhaselockTarget = nullptr;
        PhaselockHeldUntil = Now;
        PhaselockResetAt = TNumericLimits<float>::Max();
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya Phaselock blocked by %s (%s false): no lift, OnTargetBlocked damage not applied (amount not recovered), cooldown runs"),
            *Target->GetName(), *Phaselock.CanLiftFlag);
        return;
    }
    // SkillDuration = LiftDuration + Att_Phaselock_Duration (with the duration skill's grade) x the target's
    // PhaselockTimeScale (lower while its diminishing returns run).
    const int32 Grade = Skills->GradeOf(Phaselock.DurationSkill);
    if (Target)
        PhaselockTimeline = Phaselock.Timeline(Phaselock.LockDuration(Grade), Target->PhaselockTimeScale(Now, Phaselock));
    if (!Target || !Target->BeginPhaselock(Now, Phaselock, PhaselockTimeline, &PhaselockFx))
    {
        // FizzleOut: no lift; after ReleaseBufferTime Fizzled resets the cooldown and ends the skill. The fail clip's
        // notify plays the fizzle hand effect.
        bPhaselockHit = false;
        PhaselockTarget = nullptr;
        PhaselockTimeline = FOpenWillowPhaselockTimeline();
        PhaselockHeldUntil = Now;
        PhaselockResetAt = Now + Phaselock.ReleaseBufferTime;
        if (ArmsAnim && PhaselockFailAnim) ArmsAnim->PlayAction(PhaselockFailAnim);
        HandFxAt = PhaselockFx.bLoaded ? Now + PhaselockFx.FailNotifyTime : -1.f;
        bHandFxMiss = true;
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya Phaselock missed (%s; cooldown resets after %.1f s)"),
            Target ? TEXT("target cannot be phaselocked") : TEXT("no preferred target"), Phaselock.ReleaseBufferTime);
        return;
    }
    bPhaselockHit = true;
    PhaselockTarget = Target;
    // The cooldown manager holds the pool from OnSelectedTarget (the cast) to OnReleasedTarget.
    PhaselockHeldUntil = Now + PhaselockTimeline.ReleasedAt;
    PhaselockResetAt = TNumericLimits<float>::Max();
    // Host calibration (UNVERIFIED): the cast clip plays at 0.85 speed, because the game frames show the arm dropping about
    // 0.05-0.07 s after the host's at full speed. The hand effect starts at the clip's 0.25 s notify counted in seconds.
    if (ArmsAnim && PhaselockAnim) ArmsAnim->PlayAction(PhaselockAnim, 1.f, 0.85f);
    // Phase_Lock_Lift's AnimNotify_UseBehavior fires PlayPhaselockHandFXFirstPerson at its time into the clip.
    HandFxAt = PhaselockFx.bLoaded ? Now + PhaselockFx.LiftNotifyTime : -1.f;
    bHandFxMiss = false;
    // OnSelectedTarget shows the screen particle (Behavior_ScreenParticle); OnActionSkillDeactivated hides it.
    if (PhaselockFx.bLoaded)
    {
        FString Error;
        if (const FOwFxTemplate* Screen = FOwFxTemplate::Load(PhaselockFx.EmitterDir, PhaselockFx.ScreenTemplate, Error))
        {
            if (ScreenFx) ScreenFx->DestroyComponent();
            ScreenFx = NewObject<UOpenWillowFxComponent>(this);
            ScreenFx->SetupAttachment(Camera);
            ScreenFx->bFillScreen = true;
            ScreenFx->SortPriorityBase = 100;
            ScreenFx->RegisterComponent();
            ScreenFx->Play(Screen, 1.f);
        }
        else UE_LOG(LogTemp, Warning, TEXT("OpenWillow Phaselock screen effect: %s"), *Error);
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya Phaselock activated on %s: %s grade %d, skill %.2f s (locked %.2f, outro %.2f, release %.2f, end %.2f), cooldown ready at +%.2f s"),
        *Target->GetName(), *Phaselock.DurationSkill, Grade, PhaselockTimeline.SkillDuration, PhaselockTimeline.LockedAt,
        PhaselockTimeline.OutroAt, PhaselockTimeline.ReleasedAt, PhaselockTimeline.EndSkillAt,
        PhaselockTimeline.ReleasedAt + Phaselock.CooldownSeconds / FMath::Max(Phaselock.CooldownRate, KINDA_SMALL_NUMBER));
}
void AOpenWillowWalker::UpdatePhaselockPresentation(float Now)
{
    if (!PhaselockFx.bLoaded) return;
    // Hand orb: LiftActionSkill.RunCustomEvent attaches the template to the arms socket (FirstPersonAttachmentName) with
    // the first-person translation (in the socket's frame) and scale, owner-only and in the foreground; the component is
    // removed when the system finishes. The socket's UE3 bone-space offset is used as is (UNVERIFIED axis convention).
    if (HandFxAt >= 0.f && Now >= HandFxAt)
    {
        HandFxAt = -1.f;
        if (HandFx) HandFx->DestroyComponent();
        HandFx = nullptr;
        FString Error;
        const FOwFxTemplate* Template = FOwFxTemplate::Load(PhaselockFx.EmitterDir,
            bHandFxMiss ? PhaselockFx.HandMissTemplate : PhaselockFx.HandHitTemplate, Error);
        if (!Template)
        {
            UE_LOG(LogTemp, Warning, TEXT("OpenWillow Phaselock hand effect: %s"), *Error);
        }
        else if (Arms->GetBoneIndex(PhaselockFx.HandBone) == INDEX_NONE)
        {
            UE_LOG(LogTemp, Warning, TEXT("OpenWillow Phaselock hand effect: arms have no bone %s"), *PhaselockFx.HandBone.ToString());
        }
        else
        {
            HandFx = NewObject<UOpenWillowFxComponent>(this);
            HandFx->SetupAttachment(Arms, PhaselockFx.HandBone);
            const FTransform Socket(PhaselockFx.HandSocketRotation, PhaselockFx.HandSocketLocation);
            HandFx->SetRelativeLocationAndRotation(Socket.TransformPosition(PhaselockFx.HandTranslation), PhaselockFx.HandSocketRotation);
            HandFx->SortPriorityBase = 50;
            HandFx->RegisterComponent();
            HandFx->Play(Template, PhaselockFx.HandScale);
            UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock hand effect %s at +%.2f s (%d emitters skipped)"), *Template->Name,
                Now - PhaselockCastAt, HandFx->SkippedEmitters());
        }
    }
    if (HandFx && HandFx->IsFinished()) { HandFx->DestroyComponent(); HandFx = nullptr; }
    // Screen particle: hidden when the skill deactivates (EndSkill) and removed once its particles are gone.
    if (ScreenFx && bPhaselockHit && Now >= PhaselockCastAt + PhaselockTimeline.EndSkillAt) ScreenFx->StopEmitting();
    if (ScreenFx && ScreenFx->IsFinished()) { ScreenFx->DestroyComponent(); ScreenFx = nullptr; }
    // Tattoo glow: the coordinated effect's p_EnablePowerEmissive curve over EffectDuration, drawn as an additive
    // overlay on the arms masked to the tattoo (host material stand-in for Master_Player's power emissive).
    const float GlowTime = Now - GlowStartedAt;
    if (GlowTime >= 0.f && GlowTime <= PhaselockFx.GlowDuration)
    {
        TattooGlow = FOpenWillowPhaselockFxData::EvalStored(PhaselockFx.GlowPoints, GlowTime);
        if (!TattooGlowMaterial)
            if (UMaterialInterface* Base = LoadObject<UMaterialInterface>(nullptr,
                TEXT("/Game/OpenWillow/Phaselock/Materials/M_OW_PlTattooGlow.M_OW_PlTattooGlow"), nullptr, LOAD_NoWarn | LOAD_Quiet))
            {
                TattooGlowMaterial = UMaterialInstanceDynamic::Create(Base, this);
                // Host calibration (UNVERIFIED): x0.3, because the shader's full emissive colour turns the tattoo bands white-cyan from
                // 0.55 s where the game keeps solid blue bands on normal skin.
                TattooGlowMaterial->SetVectorParameterValue(TEXT("GlowColor"), PhaselockFx.GlowColor * 0.3f);
                if (UTexture* Masks = LoadObject<UTexture2D>(nullptr, *FString::Printf(TEXT("%s/Textures/SirenHands_Msk.SirenHands_Msk"), MayaRoot)))
                    TattooGlowMaterial->SetTextureParameterValue(TEXT("Masks"), Masks);
                if (UTexture* Diffuse = LoadObject<UTexture2D>(nullptr, *FString::Printf(TEXT("%s/Textures/SirenHands_Dif.SirenHands_Dif"), MayaRoot)))
                    TattooGlowMaterial->SetTextureParameterValue(TEXT("Diffuse"), Diffuse);
            }
        if (TattooGlowMaterial)
        {
            TattooGlowMaterial->SetScalarParameterValue(TEXT("Enable"), TattooGlow);
            if (Arms->GetOverlayMaterial() != TattooGlowMaterial) Arms->SetOverlayMaterial(TattooGlowMaterial);
        }
    }
    else if (GlowStartedAt > -100.f && (TattooGlow != 0.f || Arms->GetOverlayMaterial()))
    {
        TattooGlow = 0.f;
        Arms->SetOverlayMaterial(nullptr);
    }
}
int32 AOpenWillowWalker::HandFxParticles() const { return HandFx ? HandFx->LiveParticles() : 0; }
int32 AOpenWillowWalker::ScreenFxParticles() const { return ScreenFx ? ScreenFx->LiveParticles() : 0; }
void AOpenWillowWalker::RunPhaselockShots(float Now)
{
    // Captures of the cast at fixed times after it (screen space, no UI), then a miss for the fizzle hand effect.
    static const float HitShots[] = {0.12f, 0.25f, 0.30f, 0.35f, 0.40f, 0.45f, 0.5f, 0.55f, 0.6f, 0.65f, 0.70f, 0.75f, 0.8f, 1.2f, 1.5f, 2.0f, 3.0f, 3.6f, 4.2f, 4.5f, 4.8f, 5.0f, 5.3f, 6.2f};
    static const float MissShots[] = {0.08f, 0.2f, 0.4f, 0.7f};
    auto Shot = [this, Now](const TCHAR* Kind, float At)
    {
        const FString Name = FString::Printf(TEXT("OWPhaselock_%s_%04d.png"), Kind, FMath::RoundToInt(At * 1000.f));
        FScreenshotRequest::RequestScreenshot(Name, false, false);
        const AOpenWillowCombatTarget* T = PhaselockTarget.Get();
        // The name carries the scheduled time; a long frame can take the shot later, so the actual time is logged too.
        UE_LOG(LogTemp, Display, TEXT("OpenWillow phaselock capture %s at +%.3f s: glow %.3f, hand particles %d, screen particles %d, bubble stage %d, collapse %.3f, light %.2f, lift %.1f"),
            *Name, Now - PhaselockShotCastAt, TattooGlow, HandFxParticles(), ScreenFxParticles(), T ? T->BubbleStage() : -1,
            T ? T->BubbleCollapse() : 0.f, T ? T->PhaselockLightIntensity() : 0.f, T ? T->LiftedHeight() : 0.f);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow phaselock capture %s emitters: hand [%s] screen [%s] bubble %s"), *Name,
            HandFx ? *HandFx->Describe() : TEXT("-"), ScreenFx ? *ScreenFx->Describe() : TEXT("-"), T ? *T->PresentationReport() : TEXT("-"));
    };
    const int32 HitCount = UE_ARRAY_COUNT(HitShots), MissCount = UE_ARRAY_COUNT(MissShots);
    if (PhaselockShotStep == 0)
    {
        // Captures only: game time advances at most 1/60 s per frame, so that a screenshot's write stall (0.1-0.4 s,
        // up to the default 0.4 s clamp) does not move the effect on and each shot lands on its scheduled time.
        // (FApp's fixed time step is compiled out in this engine build.)
        if (AWorldSettings* Settings = GetWorld()->GetWorldSettings()) Settings->MaxUndilatedFrameTime = 1.f / 60.f;
        if (Now < 7.f || !CombatTarget.IsValid()) return;
        AimAt(CombatTarget->AimPoint() - FVector(0, 0, 20));
        ++PhaselockShotStep;
    }
    else if (PhaselockShotStep == 1)
    {
        if (Now < 8.f) return;
        UsePhaselock();
        PhaselockShotCastAt = Now;
        ++PhaselockShotStep;
    }
    else if (PhaselockShotStep < 2 + HitCount)
    {
        const float At = HitShots[PhaselockShotStep - 2];
        if (Now - PhaselockShotCastAt < At) return;
        Shot(TEXT("hit"), At);
        ++PhaselockShotStep;
    }
    else if (PhaselockShotStep == 2 + HitCount)
    {
        // Wait for the cooldown, then cast with the view well above the target: no preferred target, a fizzle.
        if (PhaselockRemaining() > 0.f) return;
        Controller->SetControlRotation(Controller->GetControlRotation() + FRotator(50.f, 0, 0));
        ++PhaselockShotStep;
    }
    else if (PhaselockShotStep == 3 + HitCount)
    {
        UsePhaselock();
        PhaselockShotCastAt = Now;
        ++PhaselockShotStep;
    }
    else if (PhaselockShotStep < 4 + HitCount + MissCount)
    {
        const float At = MissShots[PhaselockShotStep - 4 - HitCount];
        if (Now - PhaselockShotCastAt < At) return;
        Shot(TEXT("miss"), At);
        ++PhaselockShotStep;
    }
    else if (PhaselockShotStep == 4 + HitCount + MissCount)
    {
        if (Now - PhaselockShotCastAt < 2.f) return;
        if (APlayerController* PC = Cast<APlayerController>(Controller)) PC->ConsoleCommand(TEXT("quit"));
        ++PhaselockShotStep;
    }
}
float AOpenWillowWalker::PhaselockRemaining() const
{
    // Pool seconds left (BaseMaxValue drained per second), refilled at the last cast.
    if (!GetWorld() || !Phaselock.bLoaded) return 0.f;
    const float Now = GetWorld()->GetTimeSeconds();
    if (Now >= PhaselockResetAt) return 0.f;
    const float Held = FMath::Clamp(Now, PhaselockCastAt, PhaselockHeldUntil) - PhaselockCastAt;
    const float Free = FMath::Max(0.f, Now - FMath::Max(PhaselockCastAt, PhaselockHeldUntil));
    return FMath::Max(0.f, Phaselock.CooldownSeconds - Held * Phaselock.CooldownHeldRate - Free * Phaselock.CooldownRate);
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
