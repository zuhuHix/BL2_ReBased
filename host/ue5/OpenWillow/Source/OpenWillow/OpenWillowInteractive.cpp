// AI-assisted (Claude). See OpenWillowInteractive.h. Strict manifest reading and loot rolls first, then the lootable actor,
// the pickup rules, the subsystem (spawn, open, nearest use, auto pickup) and the in-engine test sequence.
#include "OpenWillowInteractive.h"
#include "OpenWillowMayaHUD.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "OpenWillowQuickChange.h"
#include "OpenWillowSkills.h"
#include "GameFramework/Controller.h"
#include "OpenWillowInteractivePickup.h"
#include "OpenWillowWalker.h"
#include "OpenWillowUseView.h"
#include "OpenWillowQuest.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowWeaponGen.h"
#include "Animation/AnimSequence.h"
#include "Components/BoxComponent.h"
#include "Components/CapsuleComponent.h"
#include "Components/SceneComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/Engine.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "EngineUtils.h"
#include "GameFramework/PlayerController.h"
#include "HAL/FileManager.h"
#include "Materials/MaterialInterface.h"
#include "Math/RandomStream.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "Dom/JsonObject.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "TimerManager.h"
#include "UnrealClient.h"
#include "GenericPlatform/GenericPlatformMisc.h"
#include "UObject/ConstructorHelpers.h"

namespace
{
// Host choices (UNVERIFIED in game): the reach of a lootable is the vendor stand-in's 350 uu; a use-key pickup is reached
// within 220 uu (the inventory pickups' reach); an auto pickup is taken within 120 uu.
constexpr double LootableReach = 350.0;
constexpr double UsePickupReach = 220.0;
constexpr float AutoPickupRadius = 120.f;
// Drops land a little in front of the lootable, fanned out across it (UNVERIFIED host choice).
constexpr float DropDistance = 110.f;
constexpr float DropSpacing = 45.f;
const TCHAR* LootableAssetRoot = TEXT("/Game/OpenWillow/Lootables/");

// A manifest number as an int32 in [Low, High]; false when it is not finite, not whole or out of range.
bool ManifestInt(double Number, int32 Low, int32 High, int32& Out)
{
    if (!FMath::IsFinite(Number) || Number != FMath::FloorToDouble(Number) || Number < double(Low) || Number > double(High))
        return false;
    Out = int32(Number);
    return true;
}

// A finite number field as a float; false when the field is missing, null or not finite.
bool ReadFloat(const TSharedPtr<FJsonObject>& Obj, const TCHAR* Key, float& Out)
{
    double Number = 0;
    if (!Obj.IsValid() || !Obj->TryGetNumberField(Key, Number) || !FMath::IsFinite(Number)) return false;
    Out = float(Number);
    return true;
}

// A whole-number field in [Low, High].
bool ReadInt(const TSharedPtr<FJsonObject>& Obj, const TCHAR* Key, int32 Low, int32 High, int32& Out)
{
    double Number = 0;
    return Obj.IsValid() && Obj->TryGetNumberField(Key, Number) && ManifestInt(Number, Low, High, Out);
}

// [x, y, z] of numbers, as-is.
bool ReadVector(const TSharedPtr<FJsonObject>& Obj, const TCHAR* Key, FVector& Out)
{
    const TArray<TSharedPtr<FJsonValue>>* Values = nullptr;
    if (!Obj->TryGetArrayField(Key, Values) || Values->Num() != 3) return false;
    for (int32 I = 0; I < 3; ++I)
    {
        double N = 0;
        if (!(*Values)[I].IsValid() || !(*Values)[I]->TryGetNumber(N) || !FMath::IsFinite(N)) return false;
        Out[I] = N;
    }
    return true;
}

// The last dotted segment of a package path: "Prop_Lootables.Meshes.Skel_DahlChest" gives "Skel_DahlChest".
FString TailName(const FString& Path)
{
    const int32 Dot = Path.Find(TEXT("."), ESearchCase::CaseSensitive, ESearchDir::FromEnd);
    return Dot == INDEX_NONE ? Path : Path.Mid(Dot + 1);
}

// Calls Fn with each object of a JSON array (other entries are skipped).
template <typename Fn>
void ForEachObject(const TArray<TSharedPtr<FJsonValue>>& Values, Fn&& Callback)
{
    for (const TSharedPtr<FJsonValue>& Value : Values)
    {
        const TSharedPtr<FJsonObject>* Json = nullptr;
        if (Value.IsValid() && Value->TryGetObject(Json) && Json->IsValid()) Callback(*Json);
    }
}

// One leaf: a positive, finite weight; an empty balance is dropped.
void ReadLeaves(const TSharedPtr<FJsonObject>& Attachment, FOpenWillowLootAttachment& Out)
{
    const TArray<TSharedPtr<FJsonValue>>* Items = nullptr;
    if (!Attachment->TryGetArrayField(TEXT("items"), Items)) return;
    ForEachObject(*Items, [&Out](const TSharedPtr<FJsonObject>& Json)
    {
        FOpenWillowLootLeaf Leaf;
        Json->TryGetStringField(TEXT("balance"), Leaf.Balance);
        Json->TryGetStringField(TEXT("class"), Leaf.Class);
        if (!ReadFloat(Json, TEXT("weight"), Leaf.Weight) || Leaf.Weight <= 0 || Leaf.Balance.IsEmpty()) return;
        Out.Leaves.Add(Leaf);
    });
}

FOpenWillowLootAttachment ReadAttachment(const TSharedPtr<FJsonObject>& Json)
{
    FOpenWillowLootAttachment Attachment;
    Json->TryGetStringField(TEXT("point"), Attachment.Point);
    // A probability that is missing, not finite or outside [0, 1] never fires (UNVERIFIED reading of the manifest).
    if (ReadFloat(Json, TEXT("probability"), Attachment.Probability) && Attachment.Probability >= 0 && Attachment.Probability <= 1)
        ReadLeaves(Json, Attachment);
    else
        Attachment.Probability = 0;
    return Attachment;
}

FOpenWillowLootConfig ReadConfig(const TSharedPtr<FJsonObject>& Json)
{
    FOpenWillowLootConfig Config;
    Json->TryGetStringField(TEXT("name"), Config.Name);
    // A null or negative weight is no weight: the configuration is never picked.
    if (!ReadFloat(Json, TEXT("weight"), Config.Weight) || Config.Weight < 0) Config.Weight = 0;
    const TArray<TSharedPtr<FJsonValue>>* Attachments = nullptr;
    if (Json->TryGetArrayField(TEXT("attachments"), Attachments))
    {
        ForEachObject(*Attachments, [&Config](const TSharedPtr<FJsonObject>& Attachment)
        {
            Config.Attachments.Add(ReadAttachment(Attachment));
        });
    }
    return Config;
}

// The first skeletal mesh component of the balance's visual (names only; the host imports them by tail name).
void ReadVisual(const TSharedPtr<FJsonObject>& Balance, FOpenWillowLootTable& Table)
{
    const TSharedPtr<FJsonObject>* Visual = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* Components = nullptr;
    if (!Balance->TryGetObjectField(TEXT("visual"), Visual) || !(*Visual)->TryGetArrayField(TEXT("components"), Components)
        || Components->IsEmpty())
        return;
    const TSharedPtr<FJsonObject>* First = nullptr;
    if (!(*Components)[0].IsValid() || !(*Components)[0]->TryGetObject(First) || !First->IsValid()) return;
    FString Mesh;
    if ((*First)->TryGetStringField(TEXT("skeletal_mesh"), Mesh)) Table.SkeletalMesh = TailName(Mesh);
    float Scale = 0;
    if (ReadFloat(*First, TEXT("scale"), Scale) && Scale > 0 && Scale <= 100.f) Table.VisualScale = Scale;
    const TArray<TSharedPtr<FJsonValue>>* SocketRows = nullptr;
    if ((*First)->TryGetArrayField(TEXT("sockets"), SocketRows))
    {
        ForEachObject(*SocketRows, [&Table](const TSharedPtr<FJsonObject>& Row)
        {
            FString Name, Bone;
            const TArray<TSharedPtr<FJsonValue>>* At = nullptr;
            if (!Row->TryGetStringField(TEXT("name"), Name) || !Row->TryGetStringField(TEXT("bone"), Bone)
                || !Row->TryGetArrayField(TEXT("location"), At) || At->Num() != 3)
                return;
            FOpenWillowLootSocket Socket;
            Socket.Bone = FName(*Bone);
            Socket.Offset = FVector((*At)[0]->AsNumber(), (*At)[1]->AsNumber(), (*At)[2]->AsNumber());
            Table.Sockets.Add(Name, Socket);
        });
    }
    const TArray<TSharedPtr<FJsonValue>>* AnimSets = nullptr;
    FString AnimSet;
    if ((*First)->TryGetArrayField(TEXT("anim_sets"), AnimSets) && !AnimSets->IsEmpty() && (*AnimSets)[0].IsValid()
        && (*AnimSets)[0]->TryGetString(AnimSet))
        Table.AnimSet = TailName(AnimSet);
}

// The amount rule (the pickup block). Only the fields the host uses; a missing field keeps its zero.
FOpenWillowPickupSpec ReadPickupSpec(const TSharedPtr<FJsonObject>& Pickup)
{
    FOpenWillowPickupSpec Spec;
    Pickup->TryGetStringField(TEXT("kind"), Spec.Kind);
    FString Mesh;
    if (Pickup->TryGetStringField(TEXT("mesh"), Mesh)) Spec.Mesh = TailName(Mesh);
    Pickup->TryGetBoolField(TEXT("auto_pickup"), Spec.bAutoPickup);
    ReadInt(Pickup, TEXT("amount"), 0, MAX_int32, Spec.Amount);
    Pickup->TryGetStringField(TEXT("ammo_type"), Spec.AmmoType);
    const TSharedPtr<FJsonObject>* Heal = nullptr;
    if (Pickup->TryGetObjectField(TEXT("heal"), Heal) && ReadFloat(*Heal, TEXT("fraction_of_max_health"), Spec.HealFraction))
        Spec.HealFraction = FMath::Clamp(Spec.HealFraction, 0.f, 1.f);
    const TSharedPtr<FJsonObject>* Cash = nullptr;
    if (!Pickup->TryGetObjectField(TEXT("cash"), Cash)) return Spec;
    const TSharedPtr<FJsonObject>* Constants = nullptr;
    if ((*Cash)->TryGetObjectField(TEXT("constants"), Constants))
    {
        ReadFloat(*Constants, TEXT("Multiplier"), Spec.Multiplier);
        ReadFloat(*Constants, TEXT("Level"), Spec.LevelBase);
    }
    ReadFloat(*Cash, TEXT("base_scale"), Spec.BaseScale);
    ReadFloat(*Cash, TEXT("per_grade_scale"), Spec.PerGradeScale);
    const TArray<TSharedPtr<FJsonValue>>* Range = nullptr;
    double LowNumber = 0, HighNumber = 0;
    int32 Low = 0, High = 0;
    if ((*Cash)->TryGetArrayField(TEXT("grade_range"), Range) && Range->Num() == 2 && (*Range)[0].IsValid()
        && (*Range)[1].IsValid() && (*Range)[0]->TryGetNumber(LowNumber) && (*Range)[1]->TryGetNumber(HighNumber)
        && ManifestInt(LowNumber, 0, 1000, Low) && ManifestInt(HighNumber, 0, 1000, High) && Low <= High)
    {
        Spec.GradeMin = Low;
        Spec.GradeMax = High;
    }
    return Spec;
}

FOpenWillowLootItem ReadItem(const FString& Balance, const TSharedPtr<FJsonObject>& Json)
{
    FOpenWillowLootItem Item;
    Item.Balance = Balance;
    Json->TryGetStringField(TEXT("kind"), Item.Kind);
    Json->TryGetStringField(TEXT("name"), Item.Name);
    const TSharedPtr<FJsonObject>* Value = nullptr;
    if (Json->TryGetObjectField(TEXT("value"), Value) && ReadFloat(*Value, TEXT("base_cost"), Item.ValueBaseCost)
        && ReadFloat(*Value, TEXT("scaler"), Item.ValueScaler))
    {
        const TArray<TSharedPtr<FJsonValue>>* Parts = nullptr;
        double Part = 0;
        if ((*Value)->TryGetArrayField(TEXT("part_totals"), Parts) && !Parts->IsEmpty() && (*Parts)[0].IsValid()
            && (*Parts)[0]->TryGetNumber(Part) && FMath::IsFinite(Part) && Part >= 0)
        {
            Item.ValuePartTotal = float(Part);
            Item.bHasValue = Item.ValueBaseCost > 0 && Item.ValueScaler > 0;
        }
    }
    const TSharedPtr<FJsonObject>* Pickup = nullptr;
    if (Json->TryGetObjectField(TEXT("pickup"), Pickup) && Pickup->IsValid()) Item.Pickup = ReadPickupSpec(*Pickup);
    return Item;
}

FVector DropSlot(const AOpenWillowLootable& Lootable, int32 Index, int32 Count)
{
    const float Lateral = (float(Index) - float(Count - 1) * 0.5f) * DropSpacing;
    return Lootable.GetActorLocation() + Lootable.GetActorForwardVector() * DropDistance + Lootable.GetActorRightVector() * Lateral
        + FVector(0.f, 0.f, 20.f);
}

FActorSpawnParameters SpawnParams()
{
    FActorSpawnParameters Params;
    Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
    return Params;
}
}

// ---- Loot tables and pickups (plain rules) ---------------------------------------------------------------------------

bool FOpenWillowLootTable::Load(const TSharedPtr<FJsonObject>& Balance, FString& OutError)
{
    Configs.Reset();
    SkeletalMesh.Reset();
    AnimSet.Reset();
    VisualScale = 1.f;
    Sockets.Reset();
    if (!Balance.IsValid())
    {
        OutError = TEXT("balance object missing");
        return false;
    }
    const TArray<TSharedPtr<FJsonValue>>* ConfigValues = nullptr;
    if (Balance->TryGetArrayField(TEXT("configurations"), ConfigValues))
    {
        ForEachObject(*ConfigValues, [this](const TSharedPtr<FJsonObject>& Json) { Configs.Add(ReadConfig(Json)); });
    }
    ReadVisual(Balance, *this);
    if (Configs.IsEmpty())
    {
        OutError = TEXT("no configurations");
        return false;
    }
    return true;
}

int32 FOpenWillowLootTable::WeightedIndex(const TArray<float>& Weights, float Unit)
{
    double Total = 0;
    int32 Last = INDEX_NONE;
    for (int32 I = 0; I < Weights.Num(); ++I)
    {
        if (!FMath::IsFinite(Weights[I]) || Weights[I] <= 0) continue;
        Total += Weights[I];
        Last = I;
    }
    if (Last == INDEX_NONE) return INDEX_NONE;
    double Target = double(FMath::Clamp(Unit, 0.f, 1.f)) * Total;
    for (int32 I = 0; I < Weights.Num(); ++I)
    {
        if (!FMath::IsFinite(Weights[I]) || Weights[I] <= 0) continue;
        if (Target < Weights[I]) return I;
        Target -= Weights[I];
    }
    return Last; // the total rounding left a sliver: the last weighted entry takes it
}

void FOpenWillowLootTable::Roll(FRandomStream& Rng, TArray<FOpenWillowLootLeaf>& OutLeaves, FString& OutConfig) const
{
    OutLeaves.Reset();
    OutConfig = TEXT("none");
    TArray<float> ConfigWeights;
    for (const FOpenWillowLootConfig& Config : Configs) ConfigWeights.Add(Config.Weight);
    const int32 Index = WeightedIndex(ConfigWeights, Rng.FRand());
    if (Index == INDEX_NONE) return;
    const FOpenWillowLootConfig& Config = Configs[Index];
    OutConfig = Config.Name;
    for (const FOpenWillowLootAttachment& Attachment : Config.Attachments)
    {
        // The probability draw comes first and is always made, so a point's chance does not shift the later rolls.
        const bool bFires = Rng.FRand() < Attachment.Probability;
        if (!bFires) continue;
        TArray<float> LeafWeights;
        for (const FOpenWillowLootLeaf& Leaf : Attachment.Leaves) LeafWeights.Add(Leaf.Weight);
        const int32 Pick = WeightedIndex(LeafWeights, Rng.FRand());
        if (Pick == INDEX_NONE) continue;
        OutLeaves.Add(Attachment.Leaves[Pick]);
        OutLeaves.Last().Point = Attachment.Point;
    }
}

int32 FOpenWillowPickupSpec::CashAmount(int32 Level, int32 Grade) const
{
    if (Kind != TEXT("cash") || Multiplier <= 0 || LevelBase <= 0) return 0;
    const double Calc = double(Multiplier) * FMath::Pow(double(LevelBase), double(Level));
    const double Value = Calc * (double(BaseScale) + double(PerGradeScale) * double(Grade));
    if (!FMath::IsFinite(Value) || Value < 0) return 0;
    return int32(FMath::Min(Value, double(MAX_int32))); // the cast truncates a positive value, which is rounding down
}

int32 FOpenWillowLootItem::GearValueAt(int32 Level) const
{
    if (!bHasValue) return -1;
    const double Value = double(ValuePartTotal) * FMath::Pow(double(ValueScaler), double(Level)) * double(ValueBaseCost);
    if (!FMath::IsFinite(Value) || Value < 0 || Value >= 2147483520.0) return -1;
    return int32(Value);
}

int32 OpenWillowInteractiveRules::SaturatingAdd(int32 A, int32 B)
{
    return int32(FMath::Clamp(int64(A) + int64(B), int64(0), int64(MAX_int32)));
}

bool OpenWillowInteractiveRules::AmmoAfter(int32 Current, int32 Max, int32 Amount, int32& OutAfter)
{
    if (Amount <= 0 || Max <= 0 || Current >= Max) return false;
    OutAfter = int32(FMath::Min(int64(Max), int64(FMath::Max(Current, 0)) + int64(Amount)));
    return true;
}

bool OpenWillowInteractiveRules::HealAfter(float Health, float MaxHealth, float Fraction, bool bDown, float& OutAfter)
{
    if (bDown || !FMath::IsFinite(Health) || !FMath::IsFinite(MaxHealth) || MaxHealth <= 0 || Fraction <= 0 || Health >= MaxHealth)
        return false;
    OutAfter = FMath::Min(MaxHealth, Health + Fraction * MaxHealth);
    return true;
}

TArray<int32> OpenWillowInteractiveRules::LidCandidateOrder(int32 Count)
{
    TArray<int32> Order;
    for (int32 Index = Count - 1; Index >= (Count > 1 ? 1 : 0); --Index) Order.Add(Index);
    return Order;
}

bool OpenWillowInteractiveRules::AmmoTypeFromName(const FString& Name, EOpenWillowAmmoType& OutType)
{
    if (Name == TEXT("pistol")) OutType = EOpenWillowAmmoType::Pistol;
    else if (Name == TEXT("smg")) OutType = EOpenWillowAmmoType::SMG;
    else if (Name == TEXT("ar")) OutType = EOpenWillowAmmoType::AssaultRifle;
    else if (Name == TEXT("shotgun")) OutType = EOpenWillowAmmoType::Shotgun;
    else if (Name == TEXT("sniper")) OutType = EOpenWillowAmmoType::Sniper;
    else if (Name == TEXT("launcher")) OutType = EOpenWillowAmmoType::Launcher;
    else if (Name == TEXT("grenade")) OutType = EOpenWillowAmmoType::Grenade;
    else return false;
    return true;
}

// ---- Lootable actor --------------------------------------------------------------------------------------------------

AOpenWillowLootable::AOpenWillowLootable()
{
    PrimaryActorTick.bCanEverTick = false;
    Root = CreateDefaultSubobject<USceneComponent>(TEXT("Root"));
    SetRootComponent(Root);
    // UNVERIFIED stand-in: a 60 cm box with a lid on a back hinge, until the imported mesh loads.
    Body = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("PlaceholderBody"));
    Body->SetupAttachment(Root);
    Body->SetRelativeLocation(FVector(0.f, 0.f, 30.f));
    Body->SetRelativeScale3D(FVector(0.6f, 0.6f, 0.6f));
    LidPivot = CreateDefaultSubobject<USceneComponent>(TEXT("LidPivot"));
    LidPivot->SetupAttachment(Root);
    LidPivot->SetRelativeLocation(FVector(-30.f, 0.f, 60.f));
    Lid = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("PlaceholderLid"));
    Lid->SetupAttachment(LidPivot);
    Lid->SetRelativeLocation(FVector(30.f, 0.f, 0.f));
    Lid->SetRelativeScale3D(FVector(0.6f, 0.6f, 0.05f));
    static ConstructorHelpers::FObjectFinder<UStaticMesh> Cube(TEXT("/Engine/BasicShapes/Cube.Cube"));
    if (Cube.Succeeded())
    {
        Body->SetStaticMesh(Cube.Object);
        Lid->SetStaticMesh(Cube.Object);
    }
    Skeletal = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("ImportedMesh"));
    Skeletal->SetupAttachment(Root);
    Skeletal->SetVisibility(false);
}

void AOpenWillowLootable::Configure(const FString& InId, const FString& InBalance, const FString& InKind, int32 InStageMin,
    int32 InStageMax, const FOpenWillowLootTable& Visual)
{
    Id = InId;
    Balance = InBalance;
    Kind = InKind;
    StageMin = InStageMin;
    StageMax = InStageMax;
    ApplyVisual(Visual);
}

void AOpenWillowLootable::ApplyVisual(const FOpenWillowLootTable& Visual)
{
    if (Visual.SkeletalMesh.IsEmpty()) return;
    // The importer's layout: <Name>/<Name>/SkeletalMeshes/<Name>.
    const FString& MeshName = Visual.SkeletalMesh;
    const FString MeshPath = FString::Printf(TEXT("%s%s/%s/SkeletalMeshes/%s.%s"), LootableAssetRoot, *MeshName, *MeshName, *MeshName, *MeshName);
    USkeletalMesh* Mesh = LoadObject<USkeletalMesh>(nullptr, *MeshPath);
    UAnimSequence* Clip = nullptr;
    if (!Visual.AnimSet.IsEmpty())
    {
        const FString AnimName = Visual.AnimSet + TEXT("_Open");
        const FString AnimPath = FString(LootableAssetRoot) + TEXT("Anims/") + AnimName + TEXT(".") + AnimName;
        Clip = LoadObject<UAnimSequence>(nullptr, *AnimPath);
    }
    // Both or neither: the placeholder's lid is the only visible opening when the clip is missing.
    if (!Mesh || !Clip) return;
    OpenAnim = Clip;
    Sockets = Visual.Sockets;
    Skeletal->SetSkeletalMesh(Mesh);
    Skeletal->SetRelativeScale3D(FVector(Visual.VisualScale));
    Skeletal->SetVisibility(true);
    Body->SetVisibility(false);
    Lid->SetVisibility(false);
    bSkeletalShown = true;
}

bool AOpenWillowLootable::Open()
{
    if (bOpened) return false;
    bOpened = true;
    if (bSkeletalShown)
    {
        // Plays the open sequence once (UNVERIFIED: the imported anim set is not checked in game).
        Skeletal->PlayAnimation(OpenAnim, false);
    }
    else
    {
        LidPivot->SetRelativeRotation(FRotator(-100.f, 0.f, 0.f)); // placeholder lid, UNVERIFIED stand-in
    }
    return true;
}

void AOpenWillowLootable::ShowMeshForTest(USkeletalMesh* Mesh, const TMap<FString, FOpenWillowLootSocket>& InSockets)
{
    Skeletal->SetSkeletalMesh(Mesh);
    Sockets = InSockets;
    bSkeletalShown = true;
}

bool AOpenWillowLootable::SocketSlot(const FString& Point, FVector& OutSlot) const
{
    const FOpenWillowLootSocket* Socket = bSkeletalShown ? Sockets.Find(Point) : nullptr;
    if (!Socket || !Skeletal->DoesSocketExist(Socket->Bone)) return false;
    // The imported bone takes the package's offset unchanged: checked offline, it lands inside each mesh (VENDORS.md section 7).
    OutSlot = Skeletal->GetSocketTransform(Socket->Bone).TransformPosition(Socket->Offset);
    return true;
}

// ---- Subsystem -------------------------------------------------------------------------------------------------------

UOpenWillowInteractiveSubsystem* UOpenWillowInteractiveSubsystem::SubsystemOf(UWorld* World)
{
    return World ? World->GetSubsystem<UOpenWillowInteractiveSubsystem>() : nullptr;
}

void UOpenWillowInteractiveSubsystem::OnWorldBeginPlay(UWorld& InWorld)
{
    Super::OnWorldBeginPlay(InWorld);
    if (!InWorld.IsGameWorld()) return;
    FString Path;
    if (!FParse::Value(FCommandLine::Get(), TEXT("owinteractives="), Path))
    {
        UE_LOG(LogTemp, Display, TEXT("OpenWillow interactives: off (no -owinteractives=<interactives.json>)"));
        return;
    }
    TSharedPtr<FJsonObject> Root;
    if (!LoadManifest(Path, Root)) return;
    ReadItems(Root);
    ReadBalances(Root);
    SpawnObjects(InWorld, Root);
    bOn = true;
    InWorld.GetTimerManager().SetTimer(AutoTimer,
        FTimerDelegate::CreateUObject(this, &UOpenWillowInteractiveSubsystem::AutoCollectTick), 0.1f, true);
    if (FParse::Param(FCommandLine::Get(), TEXT("owinteractivetest")))
        InWorld.GetTimerManager().SetTimer(TestTimer,
            FTimerDelegate::CreateUObject(this, &UOpenWillowInteractiveSubsystem::TickInteractiveTest), 2.f, true, 5.f);
}

void UOpenWillowInteractiveSubsystem::Deinitialize()
{
    if (UWorld* World = GetWorld())
    {
        World->GetTimerManager().ClearTimer(AutoTimer);
        World->GetTimerManager().ClearTimer(TestTimer);
    }
    Lootables.Reset();
    Tables.Reset();
    Items.Reset();
    Super::Deinitialize();
}

bool UOpenWillowInteractiveSubsystem::LoadManifest(const FString& Path, TSharedPtr<FJsonObject>& OutRoot)
{
    FString Text;
    if (!FFileHelper::LoadFileToString(Text, *Path))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow interactives: cannot read %s; no lootables"), *Path);
        return false;
    }
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), OutRoot) || !OutRoot.IsValid())
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow interactives: %s is not JSON; no lootables"), *Path);
        return false;
    }
    FString Schema;
    OutRoot->TryGetStringField(TEXT("schema"), Schema);
    if (Schema != TEXT("openwillow-interactives/1"))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow interactives: schema '%s' is not openwillow-interactives/1; no lootables"), *Schema);
        return false;
    }
    return true;
}

void UOpenWillowInteractiveSubsystem::ReadItems(const TSharedPtr<FJsonObject>& Root)
{
    const TSharedPtr<FJsonObject>* ItemsObject = nullptr;
    if (!Root->TryGetObjectField(TEXT("items"), ItemsObject)) return;
    for (const TPair<FString, TSharedPtr<FJsonValue>>& Pair : (*ItemsObject)->Values)
    {
        const TSharedPtr<FJsonObject>* Json = nullptr;
        if (Pair.Value.IsValid() && Pair.Value->TryGetObject(Json) && Json->IsValid())
            Items.Add(Pair.Key, ReadItem(Pair.Key, *Json));
    }
}

void UOpenWillowInteractiveSubsystem::ReadBalances(const TSharedPtr<FJsonObject>& Root)
{
    const TSharedPtr<FJsonObject>* BalancesObject = nullptr;
    if (!Root->TryGetObjectField(TEXT("balances"), BalancesObject)) return;
    for (const TPair<FString, TSharedPtr<FJsonValue>>& Pair : (*BalancesObject)->Values)
    {
        const TSharedPtr<FJsonObject>* Json = nullptr;
        if (!Pair.Value.IsValid() || !Pair.Value->TryGetObject(Json) || !Json->IsValid()) continue;
        FOpenWillowLootTable Table;
        FString Error;
        if (Table.Load(*Json, Error)) Tables.Add(Pair.Key, MoveTemp(Table));
        else UE_LOG(LogTemp, Warning, TEXT("OpenWillow interactives: balance %s not loaded: %s"), *Pair.Key, *Error);
    }
}

void UOpenWillowInteractiveSubsystem::SpawnObjects(UWorld& World, const TSharedPtr<FJsonObject>& Root)
{
    const TArray<TSharedPtr<FJsonValue>>* Objects = nullptr;
    if (!Root->TryGetArrayField(TEXT("objects"), Objects))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow interactives: no objects list; no lootables"));
        return;
    }
    int32 Unsupported = 0;
    ForEachObject(*Objects, [this, &World, &Unsupported](const TSharedPtr<FJsonObject>& Json)
    {
        bool bSupported = false;
        Json->TryGetBoolField(TEXT("supported"), bSupported);
        if (!bSupported)
        {
            ++Unsupported;
            FString Id;
            Json->TryGetStringField(TEXT("id"), Id);
            UE_LOG(LogTemp, Display, TEXT("OpenWillow interactives: %s is not supported yet; not spawned"), *Id);
            return;
        }
        SpawnOne(World, Json);
    });
    UE_LOG(LogTemp, Display, TEXT("OpenWillow interactives: %d lootable(s) spawned, %d unsupported not spawned"),
        Lootables.Num(), Unsupported);
    SpawnStations(World, Root);
}

bool UOpenWillowInteractiveSubsystem::SpawnOne(UWorld& World, const TSharedPtr<FJsonObject>& Json)
{
    FString Id, Balance, Kind;
    Json->TryGetStringField(TEXT("id"), Id);
    Json->TryGetStringField(TEXT("balance"), Balance);
    Json->TryGetStringField(TEXT("kind"), Kind);
    const FOpenWillowLootTable* Table = Tables.Find(Balance);
    if (Id.IsEmpty() || !Table)
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow interactives: %s has no usable balance; not spawned"), *Id);
        return false;
    }
    FVector Location, Rotation;
    const TSharedPtr<FJsonObject>* Stage = nullptr;
    int32 StageMin = 0, StageMax = 0;
    if (!ReadVector(Json, TEXT("location"), Location) || !ReadVector(Json, TEXT("rotation"), Rotation)
        || !Json->TryGetObjectField(TEXT("game_stage"), Stage) || !ReadInt(*Stage, TEXT("min"), 1, 1000, StageMin)
        || !ReadInt(*Stage, TEXT("max"), 1, 1000, StageMax) || StageMin > StageMax)
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow interactives: %s has no usable location, rotation or stage; not spawned"), *Id);
        return false;
    }
    // Same convention as the vendors: location as-is, rotation in degrees [Pitch, Yaw, Roll].
    const FTransform Pose(FRotator(Rotation.X, Rotation.Y, Rotation.Z), Location);
    AOpenWillowLootable* Lootable = World.SpawnActor<AOpenWillowLootable>(AOpenWillowLootable::StaticClass(), Pose, SpawnParams());
    if (!Lootable) return false;
    Lootable->Configure(Id, Balance, Kind, StageMin, StageMax, *Table);
    Lootables.Add(Lootable);
    return true;
}

FString AOpenWillowLootable::RenderedMaterialPath() const
{
    const UMaterialInterface* Material = Skeletal && Skeletal->GetSkeletalMeshAsset() ? Skeletal->GetMaterial(0) : nullptr;
    return Material ? Material->GetPathName() : FString();
}

bool UOpenWillowInteractiveSubsystem::FindNearest(const AOpenWillowWalker& Walker, AOpenWillowLootable*& OutLootable,
    AOpenWillowLootPickup*& OutPickup, double& OutDistanceSquared)
{
    UWorld* World = Walker.GetWorld();
    const FVector From = Walker.GetActorLocation();
    OutLootable = nullptr;
    OutPickup = nullptr;
    UOpenWillowInteractiveSubsystem* Subsystem = SubsystemOf(World);
    if (!Subsystem || !Subsystem->bOn) return false;
    double BestLootable = FMath::Square(LootableReach);
    for (const TWeakObjectPtr<AOpenWillowLootable>& Candidate : Subsystem->Lootables)
    {
        AOpenWillowLootable* Lootable = Candidate.Get();
        if (!Lootable || Lootable->IsOpened()) continue;
        const double D2 = FVector::DistSquared(From, Lootable->GetActorLocation());
        if (D2 >= BestLootable) continue;
        FVector Origin, Extent;
        Lootable->GetActorBounds(false, Origin, Extent);
        if (!OpenWillowUseView::LooksAt(Walker, FBox::BuildAABB(Origin, Extent), Lootable)) continue;
        BestLootable = D2;
        OutLootable = Lootable;
    }
    double BestPickup = FMath::Square(UsePickupReach);
    for (TActorIterator<AOpenWillowLootPickup> It(World); It; ++It)
    {
        if (It->Spec().bAutoPickup) continue; // the tick takes these
        if (!It->WouldCollect(Walker)) continue; // not a target: the press goes on in distance order
        const double D2 = FVector::DistSquared(From, It->GetActorLocation());
        if (D2 >= BestPickup) continue;
        BestPickup = D2;
        OutPickup = *It;
    }
    // The same rule as the walker's, so a lootable and a loot pickup compare as any two use targets do.
    FOpenWillowUseDistances Targets;
    if (OutLootable) Targets.Interactive = BestLootable;
    if (OutPickup) Targets.Pickup = BestPickup;
    switch (Targets.Nearest())
    {
    case EOpenWillowUseTarget::Interactive:
        OutDistanceSquared = BestLootable;
        OutPickup = nullptr;
        return true;
    case EOpenWillowUseTarget::Pickup:
        OutDistanceSquared = BestPickup;
        OutLootable = nullptr;
        return true;
    default:
        return false;
    }
}

TArray<EOpenWillowUseTarget> FOpenWillowUseDistances::Ordered() const
{
    struct FRow
    {
        double Distance;
        EOpenWillowUseTarget Target;
    };
    TArray<FRow> Rows;
    if (Vendor.IsSet()) Rows.Add({Vendor.GetValue(), EOpenWillowUseTarget::Vendor});
    if (Interactive.IsSet()) Rows.Add({Interactive.GetValue(), EOpenWillowUseTarget::Interactive});
    if (Door.IsSet()) Rows.Add({Door.GetValue(), EOpenWillowUseTarget::Door});
    if (Pickup.IsSet()) Rows.Add({Pickup.GetValue(), EOpenWillowUseTarget::Pickup});
    Rows.StableSort([](const FRow& A, const FRow& B) { return A.Distance < B.Distance; });
    TArray<EOpenWillowUseTarget> Out;
    for (const FRow& Row : Rows) Out.Add(Row.Target);
    return Out;
}

EOpenWillowUseTarget FOpenWillowUseDistances::Nearest() const
{
    const TArray<EOpenWillowUseTarget> Rows = Ordered();
    return Rows.Num() > 0 ? Rows[0] : EOpenWillowUseTarget::None;
}

bool FOpenWillowUseDistances::Offer(TFunctionRef<bool(EOpenWillowUseTarget)> Use) const
{
    for (const EOpenWillowUseTarget Target : Ordered())
        if (Use(Target)) return true;
    return false;
}

AOpenWillowQuickChangeStation* UOpenWillowInteractiveSubsystem::NearestStation(const AOpenWillowWalker& Walker, double& OutDistanceSquared) const
{
    const FVector From = Walker.GetActorLocation();
    AOpenWillowQuickChangeStation* Nearest = nullptr;
    double Best = FMath::Square(LootableReach);
    for (const TWeakObjectPtr<AOpenWillowQuickChangeStation>& Candidate : Stations)
    {
        AOpenWillowQuickChangeStation* Station = Candidate.Get();
        if (!Station) continue;
        const double D2 = FVector::DistSquared(From, Station->GetActorLocation());
        if (D2 >= Best || !OpenWillowUseView::LooksAt(Walker, Station->UseBox(), Station, Station->GetPlacedBody())) continue;
        Best = D2;
        Nearest = Station;
    }
    OutDistanceSquared = Best;
    return Nearest;
}

void UOpenWillowInteractiveSubsystem::SpawnStations(UWorld& World, const TSharedPtr<FJsonObject>& Root)
{
    const TArray<TSharedPtr<FJsonValue>>* Rows = nullptr;
    if (!Root->TryGetArrayField(TEXT("stations"), Rows)) return;
    for (const TSharedPtr<FJsonValue>& Value : *Rows)
    {
        const TSharedPtr<FJsonObject>* Row = nullptr;
        FString Id;
        FVector Location, Rotation;
        if (!Value.IsValid() || !Value->TryGetObject(Row) || !(*Row)->TryGetStringField(TEXT("id"), Id)
            || !ReadVector(*Row, TEXT("location"), Location) || !ReadVector(*Row, TEXT("rotation"), Rotation))
        {
            UE_LOG(LogTemp, Warning, TEXT("OpenWillow interactives: a station row is not usable; not spawned"));
            continue;
        }
        FActorSpawnParameters Params;
        Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
        const FTransform Pose(FRotator(Rotation.X, Rotation.Y, Rotation.Z), Location);
        AOpenWillowQuickChangeStation* Station = World.SpawnActor<AOpenWillowQuickChangeStation>(AOpenWillowQuickChangeStation::StaticClass(), Pose, Params);
        if (!Station) continue;
        Station->Configure(Id);
        FString ObjectName, Problem;
        Id.Split(TEXT(":"), nullptr, &ObjectName);
        OpenWillowUseView::FPlacedBody Body;
        if (OpenWillowUseView::FindPlacedBody(World, TEXT("TheWorld.PersistentLevel.") + ObjectName, Location, Body, Problem))
        {
            Station->SetPlacedBody(Body);
            UE_LOG(LogTemp, Display, TEXT("OpenWillow interactives: station %s is its imported machine, %d section(s)"), *Id, Body.Sections.Num());
        }
        else
        {
            UE_LOG(LogTemp, Warning, TEXT("OpenWillow interactives: station %s uses the stand-in box: %s"), *Id, *Problem);
        }
        Stations.Add(Station);
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow interactives: %d Quick Change station(s) spawned"), Stations.Num());
}

bool UOpenWillowInteractiveSubsystem::NearestInReach(const AOpenWillowWalker& Walker, double& OutDistanceSquared)
{
    AOpenWillowLootable* Lootable = nullptr;
    AOpenWillowLootPickup* Pickup = nullptr;
    const bool bLoot = FindNearest(Walker, Lootable, Pickup, OutDistanceSquared);
    UOpenWillowInteractiveSubsystem* Subsystem = SubsystemOf(Walker.GetWorld());
    double StationDistance = 0;
    if (Subsystem && Subsystem->NearestStation(Walker, StationDistance) && (!bLoot || StationDistance < OutDistanceSquared))
    {
        OutDistanceSquared = StationDistance;
        return true;
    }
    return bLoot;
}

bool UOpenWillowInteractiveSubsystem::TryUseNearest(AOpenWillowWalker& Walker)
{
    UOpenWillowInteractiveSubsystem* Subsystem = SubsystemOf(Walker.GetWorld());
    AOpenWillowLootable* Lootable = nullptr;
    AOpenWillowLootPickup* Pickup = nullptr;
    double DistanceSquared = 0;
    if (!Subsystem) return false;
    const bool bLoot = FindNearest(Walker, Lootable, Pickup, DistanceSquared);
    double StationDistance = 0;
    AOpenWillowQuickChangeStation* Station = Subsystem->NearestStation(Walker, StationDistance);
    if (Station && (!bLoot || StationDistance < DistanceSquared)) return Station->Use(Walker);
    if (!bLoot) return false;
    if (Lootable) return Subsystem->OpenLootable(Walker, *Lootable);
    return Pickup && Pickup->TryCollect(Walker);
}

bool UOpenWillowInteractiveSubsystem::OpenLootable(AOpenWillowWalker& Walker, AOpenWillowLootable& Lootable)
{
    UOpenWillowInventory* Inventory = Walker.GetInventory();
    const FOpenWillowLootTable* Table = Tables.Find(Lootable.Balance);
    if (!Inventory || !Table || Lootable.IsOpened()) return false;
    const uint32 Serial = ++OpenSerial;
    FRandomStream Rng(static_cast<int32>(GetTypeHash(Lootable.Id) + Serial * 7919u));
    const int32 Level = Rng.RandRange(Lootable.StageMin, Lootable.StageMax);
    TArray<FOpenWillowLootLeaf> Leaves;
    FString Config;
    Table->Roll(Rng, Leaves, Config);
    Lootable.Open();
    UWorld* World = Walker.GetWorld();
    int32 Spawned = 0;
    for (int32 I = 0; World && I < Leaves.Num(); ++I)
        SpawnLeaf(*World, Lootable, Leaves[I], Level, I, Leaves.Num(), Rng, Spawned);
    UE_LOG(LogTemp, Display, TEXT("OWLOOT open id=%s config=%s drops=%d"), *Lootable.Id, *Config, Spawned);
    if (Spawned == 0)
    {
        const TCHAR* Reason = Config == TEXT("none") ? TEXT("no configuration has weight")
            : Leaves.IsEmpty() ? TEXT("no attachment dropped") : TEXT("no drop could be spawned");
        UE_LOG(LogTemp, Display, TEXT("OWLOOT empty id=%s reason=%s"), *Lootable.Id, Reason);
    }
    return true;
}

AActor* UOpenWillowInteractiveSubsystem::SpawnLeaf(UWorld& World, const AOpenWillowLootable& Lootable,
    const FOpenWillowLootLeaf& Leaf, int32 Level, int32 Index, int32 Count, FRandomStream& Rng, int32& Spawned)
{
    FVector Slot;
    const bool bAtSocket = Lootable.SocketSlot(Leaf.Point, Slot);
    if (!bAtSocket) Slot = DropSlot(Lootable, Index, Count);
    AActor* Drop = nullptr;
    if (Leaf.Class == TEXT("WeaponBalanceDefinition"))
    {
        Drop = SpawnWeapon(World, Lootable, Leaf.Balance, Level, static_cast<uint32>(Rng.RandRange(1, MAX_int32)), Slot, Spawned);
    }
    else if (const FOpenWillowLootItem* Item = Items.Find(Leaf.Balance))
    {
        if (Item->IsPickup()) Drop = SpawnPickup(World, Lootable, *Item, Level, Rng, Slot, Spawned);
        else if (Item->IsGear()) Drop = SpawnGear(World, Lootable, *Item, Level, Slot, Spawned);
        else UE_LOG(LogTemp, Display, TEXT("OWLOOT skip id=%s balance=%s reason=no pickup or gear value"), *Lootable.Id, *Leaf.Balance);
    }
    else
    {
        UE_LOG(LogTemp, Display, TEXT("OWLOOT skip id=%s balance=%s reason=not in the items block"), *Lootable.Id, *Leaf.Balance);
    }
    if (!Drop) return nullptr;
    if (bAtSocket)
    {
        if (AOpenWillowInventoryPickup* Held = Cast<AOpenWillowInventoryPickup>(Drop)) Held->SetContainer(&Lootable);
    }
    // After the spawn, from the actor: this says where a drop that exists really lies.
    const FVector At = Drop->GetActorLocation();
    const double FromContainer = FVector::Dist(At, Lootable.GetActorLocation());
    if (bAtSocket)
    {
        UE_LOG(LogTemp, Display, TEXT("OWLOOT slot id=%s point=%s at=socket x=%.1f y=%.1f z=%.1f from_container=%.1f"), *Lootable.Id, *Leaf.Point,
            At.X, At.Y, At.Z, FromContainer);
    }
    else
    {
        UE_LOG(LogTemp, Display, TEXT("OWLOOT slot id=%s point=%s at=front x=%.1f y=%.1f z=%.1f from_container=%.1f reason=%s"), *Lootable.Id,
            *Leaf.Point, At.X, At.Y, At.Z, FromContainer, Lootable.ShowsImportedMesh() ? TEXT("no socket") : TEXT("placeholder"));
    }
    return Drop;
}

AActor* UOpenWillowInteractiveSubsystem::TestDrop(AOpenWillowLootable& Lootable, const FOpenWillowLootItem& Item, const FString& Point)
{
    UWorld* World = GetWorld();
    if (!World) return nullptr;
    Items.Add(Item.Balance, Item);
    FOpenWillowLootLeaf Leaf;
    Leaf.Balance = Item.Balance;
    Leaf.Class = TEXT("InventoryBalanceDefinition");
    Leaf.Weight = 1.f;
    Leaf.Point = Point;
    FRandomStream Rng(1);
    int32 Spawned = 0;
    return SpawnLeaf(*World, Lootable, Leaf, 5, 0, 1, Rng, Spawned);
}

AActor* UOpenWillowInteractiveSubsystem::SpawnWeapon(UWorld& World, const AOpenWillowLootable& Lootable, const FString& Balance,
    int32 Level, uint32 GunSeed, const FVector& Slot, int32& Spawned)
{
    FOpenWillowWeaponItem Gun;
    if (!FOpenWillowWeaponGen::RollBalance(Balance, Level, GunSeed, Gun))
    {
        UE_LOG(LogTemp, Display, TEXT("OWLOOT skip id=%s balance=%s reason=weapon generator unavailable or balance unknown"),
            *Lootable.Id, *Balance);
        return nullptr;
    }
    FOpenWillowTakenInventoryItem Taken;
    Taken.Weapon = Gun;
    AOpenWillowInventoryPickup* Pickup = World.SpawnActor<AOpenWillowInventoryPickup>(AOpenWillowInventoryPickup::StaticClass(),
        FTransform(Slot), SpawnParams());
    if (!Pickup) return nullptr;
    Pickup->Initialize(Taken);
    ++Spawned;
    UE_LOG(LogTemp, Display, TEXT("OWLOOT drop id=%s kind=weapon balance=%s level=%d value=%d"), *Lootable.Id, *Balance, Level,
        Gun.bHasSaleValue ? Gun.SaleValue : 0);
    return Pickup;
}

AActor* UOpenWillowInteractiveSubsystem::SpawnGear(UWorld& World, const AOpenWillowLootable& Lootable, const FOpenWillowLootItem& Item,
    int32 Level, const FVector& Slot, int32& Spawned)
{
    const int32 Value = Item.GearValueAt(Level);
    if (Value < 0)
    {
        UE_LOG(LogTemp, Display, TEXT("OWLOOT skip id=%s balance=%s reason=gear value formula does not resolve"), *Lootable.Id, *Item.Balance);
        return nullptr;
    }
    // Stand-in gear: the name and value come from the manifest, the rarity is the lowest (UNVERIFIED host choice).
    FOpenWillowGearItem Gear;
    Gear.Id = FString::Printf(TEXT("loot_%s_%d"), *TailName(Item.Balance), Level);
    Gear.ItemType = Item.Kind;
    Gear.Name = Item.Name.IsEmpty() ? TailName(Item.Balance) : Item.Name;
    Gear.Level = Level;
    Gear.bLevelKnown = true;
    Gear.Rarity = 1;
    Gear.SaleValue = Value;
    FOpenWillowTakenInventoryItem Taken;
    Taken.bIsGear = true;
    Taken.Gear = Gear;
    AOpenWillowInventoryPickup* Pickup = World.SpawnActor<AOpenWillowInventoryPickup>(AOpenWillowInventoryPickup::StaticClass(),
        FTransform(Slot), SpawnParams());
    if (!Pickup) return nullptr;
    Pickup->Initialize(Taken);
    ++Spawned;
    UE_LOG(LogTemp, Display, TEXT("OWLOOT drop id=%s kind=gear balance=%s level=%d value=%d"), *Lootable.Id, *Item.Balance, Level, Value);
    return Pickup;
}

AActor* UOpenWillowInteractiveSubsystem::SpawnPickup(UWorld& World, const AOpenWillowLootable& Lootable, const FOpenWillowLootItem& Item,
    int32 Level, FRandomStream& Rng, const FVector& Slot, int32& Spawned)
{
    const FOpenWillowPickupSpec& Spec = Item.Pickup;
    const int32 Grade = Spec.Kind == TEXT("cash") ? Rng.RandRange(Spec.GradeMin, Spec.GradeMax) : 0;
    const int32 Value = Spec.Kind == TEXT("cash") ? Spec.CashAmount(Level, Grade) : Spec.Amount;
    if (Spec.Kind == TEXT("cash") && Value <= 0)
    {
        UE_LOG(LogTemp, Display, TEXT("OWLOOT skip id=%s balance=%s reason=cash constants not set"), *Lootable.Id, *Item.Balance);
        return nullptr;
    }
    AOpenWillowLootPickup* Pickup = World.SpawnActor<AOpenWillowLootPickup>(AOpenWillowLootPickup::StaticClass(), FTransform(Slot),
        SpawnParams());
    if (!Pickup) return nullptr;
    Pickup->Configure(Spec, Value, Item.Balance);
    ++Spawned;
    UE_LOG(LogTemp, Display, TEXT("OWLOOT drop id=%s kind=%s balance=%s level=%d value=%d"), *Lootable.Id, *Spec.Kind, *Item.Balance,
        Level, Value);
    return Pickup;
}

void UOpenWillowInteractiveSubsystem::AutoCollectTick()
{
    UWorld* World = GetWorld();
    APlayerController* PC = World ? World->GetFirstPlayerController() : nullptr;
    AOpenWillowWalker* Walker = PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr;
    const double Now = World ? World->GetTimeSeconds() : 0.0;
    // Before the walker check: a prompt must still expire, and its label clear, with no current walker.
    if (World && Stations.Num() > 0)
    {
        const AOpenWillowQuickChangeStation* Selected = Walker ? SelectedStation(*Walker) : nullptr;
        for (const TWeakObjectPtr<AOpenWillowQuickChangeStation>& Station : Stations)
            if (Station.IsValid()) Station->TickPrompt(Now, Walker, Station.Get() == Selected);
    }
    if (!Walker) return;
    const FVector From = Walker->GetActorLocation();
    // Collected first, then tried: a taken pickup is destroyed, which must not happen during the iteration.
    TArray<AOpenWillowLootPickup*> Near;
    for (TActorIterator<AOpenWillowLootPickup> It(World); It; ++It)
    {
        if (It->Spec().bAutoPickup && Now >= It->NextAutoTryAt
            && FVector::DistSquared(From, It->GetActorLocation()) <= FMath::Square(double(AutoPickupRadius)))
            Near.Add(*It);
    }
    for (AOpenWillowLootPickup* Pickup : Near)
    {
        if (!Pickup->TryCollect(*Walker)) Pickup->NextAutoTryAt = Now + 2.0; // a refusal is retried in two seconds
    }
}

// ---- In-engine test (-owinteractivetest) -----------------------------------------------------------------------------

void UOpenWillowInteractiveSubsystem::TickInteractiveTest()
{
    UWorld* World = GetWorld();
    APlayerController* PC = World ? World->GetFirstPlayerController() : nullptr;
    AOpenWillowWalker* Walker = PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr;
    if (!Walker)
    {
        if (++TestWaits > 60)
        {
            UE_LOG(LogTemp, Error, TEXT("OWINTERACTIVETEST FAIL no walker after 60 s"));
            FPlatformMisc::RequestExit(false);
        }
        return;
    }
    if (TickStationTest(*Walker)) return;
    if (TickLidShots(*Walker)) return;
    switch (TestStep++)
    {
    case 0: TestOpen(*Walker); break;
    case 1:
    {
        // Half-empty pistol pool and half health, so the ammo and health pickups have something to refill.
        if (UOpenWillowInventory* Inventory = Walker->GetInventory())
        {
            const int32 Max = Inventory->AmmoPool(EOpenWillowAmmoType::Pistol).Max;
            Inventory->SetAmmoCurrent(EOpenWillowAmmoType::Pistol, Max / 2);
        }
        Walker->GetMayaState().Health = Walker->GetMaxHealth() * 0.5f; // test fixture: current health only
        UE_LOG(LogTemp, Display, TEXT("OWINTERACTIVETEST prepared pistol and health at half"));
        break;
    }
    case 2:
    {
        // Every spawned loot pickup, through the same Collect the walker's use key and the tick use.
        TArray<AOpenWillowLootPickup*> All;
        for (TActorIterator<AOpenWillowLootPickup> It(World); It; ++It) All.Add(*It);
        int32 Taken = 0;
        for (AOpenWillowLootPickup* Pickup : All) if (Pickup->TryCollect(*Walker)) ++Taken;
        UE_LOG(LogTemp, Display, TEXT("OWINTERACTIVETEST collected %d of %d pickup(s)"), Taken, All.Num());
        break;
    }
    case 3:
    {
        int32 Opened = 0;
        for (const TWeakObjectPtr<AOpenWillowLootable>& Candidate : Lootables) if (Candidate.IsValid() && Candidate->IsOpened()) ++Opened;
        UE_LOG(LogTemp, Display, TEXT("OWINTERACTIVETEST done spawned=%d opened=%d"), Lootables.Num(), Opened);
        break;
    }
    case 4: FScreenshotRequest::RequestScreenshot(TEXT("OWInteractive_open.png"), false, false); break;
    case 6:
        World->GetTimerManager().ClearTimer(TestTimer);
        FPlatformMisc::RequestExit(false);
        break;
    default: break;
    }
}

void UOpenWillowInteractiveSubsystem::CancelStationPrompts(UWorld* World, const TCHAR* Reason)
{
    if (UOpenWillowInteractiveSubsystem* Subsystem = SubsystemOf(World))
        for (const TWeakObjectPtr<AOpenWillowQuickChangeStation>& Station : Subsystem->Stations)
            if (Station.IsValid()) Station->CancelPrompt(Reason);
}

FString UOpenWillowInteractiveSubsystem::UseVerb(const AOpenWillowWalker& Walker)
{
    const UOpenWillowInteractiveSubsystem* Subsystem = SubsystemOf(Walker.GetWorld());
    if (!Subsystem) return FString();
    if (Subsystem->SelectedStation(Walker)) return TEXT("use");
    AOpenWillowLootable* Lootable = nullptr;
    AOpenWillowLootPickup* Pickup = nullptr;
    double Distance = 0;
    if (!FindNearest(Walker, Lootable, Pickup, Distance)) return FString();
    return Pickup ? TEXT("pickup") : TEXT("open");
}

AOpenWillowQuickChangeStation* UOpenWillowInteractiveSubsystem::SelectedStation(const AOpenWillowWalker& Walker) const
{
    if (Walker.NearestUseTarget() != EOpenWillowUseTarget::Interactive) return nullptr;
    AOpenWillowLootable* Lootable = nullptr;
    AOpenWillowLootPickup* Pickup = nullptr;
    double LootDistance = 0, StationDistance = 0;
    const bool bLoot = FindNearest(Walker, Lootable, Pickup, LootDistance);
    AOpenWillowQuickChangeStation* Station = NearestStation(Walker, StationDistance);
    return Station && (!bLoot || StationDistance < LootDistance) ? Station : nullptr;
}

bool UOpenWillowInteractiveSubsystem::TickStationTest(AOpenWillowWalker& Walker)
{
    if (Stations.Num() == 0 || !Stations[0].IsValid() || TestStationStep > 8) return false;
    AOpenWillowQuickChangeStation& Station = *Stations[0].Get();
    UOpenWillowSkills* Skills = Walker.GetSkills();
    UOpenWillowInventory* Inventory = Walker.GetInventory();
    if (!Skills || !Inventory) return false;
    const auto Beside = [&Station, &Walker]()
    {
        // In front of the machine and clear of it, where a player can stand; a little to its side, as a pillar stands straight in front. Inside its collision the
        // eye starts in the machine, and she stays stuck there. Dropped from above the floor, so she lands.
        const FBox Box = Station.UseBox();
        const float Along = Box.GetExtent().Size2D() + Walker.GetCapsuleComponent()->GetScaledCapsuleRadius() + 20.f;
        FVector At = Box.GetCenter() + Station.GetActorForwardVector() * Along - Station.GetActorRightVector() * 30.f;
        At.Z = Station.GetActorLocation().Z + 100.f;
        Walker.SetActorLocation(At, false, nullptr, ETeleportType::TeleportPhysics);
    };
    // Aims at the station, or Turn degrees to the side of it.
    const auto Face = [&Station, &Walker](float Turn)
    {
        if (AController* Controller = Walker.GetController())
            Controller->SetControlRotation((Station.UseBox().GetCenter() - OpenWillowUseView::EyeOf(Walker)).Rotation() + FRotator(0.f, Turn, 0.f));
    };
    Face(0.f);   // again at each step: Maya may still be landing
    // Marcus or a waiting reward takes the press before the targets (PickupNearby), so neither may be in play for a press here to say anything about the targets.
    const auto QuestTakesPress = [&Walker]()
    {
        const UOpenWillowQuest* Quest = Walker.GetQuest();
        return Quest && Quest->MayTakeUse();
    };
    // What the press would go to now, whichever kind; the station not being selected does not say the press goes nowhere.
    const auto TargetNow = [&Walker]() -> const TCHAR*
    {
        switch (Walker.NearestUseTarget())
        {
        case EOpenWillowUseTarget::None: return TEXT("none");
        case EOpenWillowUseTarget::Vendor: return TEXT("vendor");
        case EOpenWillowUseTarget::Interactive: return TEXT("interactive");
        case EOpenWillowUseTarget::Door: return TEXT("door");
        case EOpenWillowUseTarget::Pickup: return TEXT("pickup");
        }
        return TEXT("unknown");
    };
    int32 Money = 0;
    Inventory->GetMoney(Money);
    switch (TestStationStep++)
    {
    case 0:
    {
        // Test fixture: a spent point, so a reset has something to do and a refusal has something to leave alone.
        if (Skills->GetLevel() < 5) Skills->SetLevel(5);
        FString Reason;
        Skills->TrySpend(-1, -1, -1, Reason);
        Beside();
        // In reach with the station behind her: not the target, and a press asks nothing.
        Face(180.f);
        const bool bSelectedAway = SelectedStation(Walker) != nullptr;
        const TCHAR* TargetAway = TargetNow();
        const bool bQuestAway = QuestTakesPress();
        Walker.PressUseKey();
        UE_LOG(LogTemp, Display, TEXT("OWQCTEST looked away selected=%d target=%s quest=%d"), bSelectedAway ? 1 : 0, TargetAway, bQuestAway ? 1 : 0);
        Face(0.f);
        // Test fixture: a sheet across the ray, halfway to the station, for the next step. Only the visibility channel sees it, so it cannot move her.
        const FVector Eye = OpenWillowUseView::EyeOf(Walker);
        const FVector Direction = Walker.GetViewRotation().Vector();
        double ToStation = 0;
        OpenWillowUseView::RayEntersBox(Eye, Direction, Station.UseBox(), ToStation);
        const FVector Halfway = Eye + Direction * (ToStation * .5);
        AActor* Blocker = GetWorld()->SpawnActor<AActor>(Halfway, Direction.Rotation());
        if (Blocker)
        {
            UBoxComponent* Sheet = NewObject<UBoxComponent>(Blocker, TEXT("ViewBlockerFixture"));
            Sheet->SetBoxExtent(FVector(2.f, 150.f, 150.f));
            Sheet->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
            Sheet->SetCollisionResponseToAllChannels(ECR_Ignore);
            Sheet->SetCollisionResponseToChannel(ECC_Visibility, ECR_Block);
            Blocker->SetRootComponent(Sheet);
            Sheet->RegisterComponent();
            Sheet->SetWorldLocationAndRotation(Halfway, Direction.Rotation());
        }
        TestViewBlocker = Blocker;
        UE_LOG(LogTemp, Display, TEXT("OWQCTEST ready money=%d"), Money);
        break;
    }
    case 1:
        // In reach and looked at, with the sheet in the way: not the target. The next step is the same spot without it.
        UE_LOG(LogTemp, Display, TEXT("OWQCTEST blocked selected=%d target=%s fixture=%d"), SelectedStation(Walker) ? 1 : 0, TargetNow(), TestViewBlocker.IsValid() ? 1 : 0);
        UE_LOG(LogTemp, Display, TEXT("OWQCTEST ray blocked %s"), *OpenWillowUseView::Describe(Walker, Station.UseBox(), &Station, Station.GetPlacedBody()));
        if (TestViewBlocker.IsValid()) TestViewBlocker->Destroy();
        break;
    case 2:
        UE_LOG(LogTemp, Display, TEXT("OWQCTEST clear selected=%d target=%s quest=%d"), SelectedStation(Walker) ? 1 : 0, TargetNow(), QuestTakesPress() ? 1 : 0);
        UE_LOG(LogTemp, Display, TEXT("OWQCTEST ray clear %s"), *OpenWillowUseView::Describe(Walker, Station.UseBox(), &Station, Station.GetPlacedBody()));
        Walker.PressUseKey(); // through the use-key path: asks
        break;
    case 3:
        // Walks away; the subsystem's tick must end the prompt.
        Walker.SetActorLocation(Station.GetActorLocation() + FVector(1000.f, 0.f, 100.f), false, nullptr, ETeleportType::TeleportPhysics);
        UE_LOG(LogTemp, Display, TEXT("OWQCTEST walked away"));
        break;
    case 4:
        // Back inside the 5 s window: one press must ask again, not reset.
        Beside();
        Face(0.f);
        Walker.PressUseKey();
        break;
    case 5:
        // Too little money: the confirm is refused and changes nothing.
        TestSavedMoney = Money;
        TestStateBefore = Skills->StateJson();
        Inventory->SetMoney(0);
        Walker.PressUseKey();
        break;
    case 6:
    {
        UE_LOG(LogTemp, Display, TEXT("OWQCTEST refusal money=%d grades_same=%d"), Money, Skills->StateJson() == TestStateBefore ? 1 : 0);
        Inventory->SetMoney(TestSavedMoney);
        Walker.PressUseKey(); // asks
        break;
    }
    case 7: TestStateBefore = Skills->StateJson(); Walker.PressUseKey(); break; // confirms
    default:
        UE_LOG(LogTemp, Display, TEXT("OWQCTEST reset grades_changed=%d"), Skills->StateJson() == TestStateBefore ? 0 : 1);
        break;
    }
    return true;
}

namespace
{
    // Why Maya cannot view this lootable from this side (one of eight, 220 uu out, starting in front), or null when she can:
    // her capsule fits on walkable floor at its level and her view, as wide as she is, is clear. The view stops short of the
    // lootable, so a wall right behind it does not count. Many lootables are placed against a wall.
    const TCHAR* LidSideFailure(const UWorld& World, const AOpenWillowWalker& Walker, const AOpenWillowLootable& Lootable, int32 Side, FVector& OutStand)
    {
        const UCapsuleComponent* Capsule = Walker.GetCapsuleComponent();
        const float Radius = Capsule->GetScaledCapsuleRadius();
        const float HalfHeight = Capsule->GetScaledCapsuleHalfHeight();
        const ECollisionChannel PawnChannel = Capsule->GetCollisionObjectType();
        const FVector Base = Lootable.GetActorLocation();
        const FVector Probe = Base + (Lootable.GetActorForwardVector() * 220.f).RotateAngleAxis(45.f * Side, FVector::UpVector);
        FCollisionQueryParams Params(SCENE_QUERY_STAT(OWLidView), true);
        Params.AddIgnoredActor(&Lootable);
        Params.AddIgnoredActor(&Walker);
        FHitResult Floor;
        if (!World.LineTraceSingleByChannel(Floor, Probe + FVector(0.f, 0.f, 120.f), Probe - FVector(0.f, 0.f, 120.f), PawnChannel, Params)) return TEXT("no_floor");
        if (Floor.ImpactNormal.Z < Walker.GetCharacterMovement()->GetWalkableFloorZ()) return TEXT("steep_floor");
        OutStand = Floor.ImpactPoint + FVector(0.f, 0.f, HalfHeight + 2.f);
        if (World.OverlapBlockingTestByChannel(OutStand, FQuat::Identity, PawnChannel, FCollisionShape::MakeCapsule(Radius, HalfHeight), Params)) return TEXT("capsule");
        const FVector Eye = OutStand + (Walker.GetPawnViewLocation() - Walker.GetActorLocation());
        const FVector Target = Base + FVector(0.f, 0.f, 50.f);
        const FVector Near = Target + (Eye - Target).GetSafeNormal() * (Radius + 30.f);
        if (World.SweepTestByChannel(Eye, Near, FQuat::Identity, ECC_Visibility, FCollisionShape::MakeSphere(Radius), Params)) return TEXT("view");
        return nullptr;
    }
}

bool UOpenWillowInteractiveSubsystem::TickLidShots(AOpenWillowWalker& Walker)
{
    UWorld* World = GetWorld();
    // A run that asked for the movie HUD holds the queue until it is ready. If it fails, the test's own exit, with no more frames.
    const EOpenWillowCaptureHud MovieHud = AOpenWillowMayaHUD::CaptureHudGate(World, TEXT("lid"));
    if (MovieHud == EOpenWillowCaptureHud::Wait) return true;
    if (MovieHud == EOpenWillowCaptureHud::Failed)
    {
        World->GetTimerManager().ClearTimer(TestTimer);
        FPlatformMisc::RequestExit(false);
        return true;
    }
    if (!bLidQueueBuilt)
    {
        bLidQueueBuilt = true;
        TMap<FString, TArray<AOpenWillowLootable*>> ByBalance;
        for (const TWeakObjectPtr<AOpenWillowLootable>& Candidate : Lootables)
            if (Candidate.IsValid() && Candidate->HasOpenClip()) ByBalance.FindOrAdd(Candidate->Balance).Add(Candidate.Get());
        UE_LOG(LogTemp, Display, TEXT("OWLOOTLID queue=%d (balances with an imported open clip)"), ByBalance.Num());
        for (const TPair<FString, TArray<AOpenWillowLootable*>>& Row : ByBalance)
        {
            const TArray<int32> Order = OpenWillowInteractiveRules::LidCandidateOrder(Row.Value.Num());
            FLidStop Stop;
            int32 ClearYaw = -1;
            int32 Tried = 0;
            for (int32 Index = 0; Index < Order.Num() && ClearYaw < 0; ++Index)
            {
                AOpenWillowLootable* Candidate = Row.Value[Order[Index]];
                ++Tried;
                for (int32 Side = 0; Side < 8 && ClearYaw < 0; ++Side)
                {
                    FVector Stand;
                    if (const TCHAR* Failed = LidSideFailure(*World, Walker, *Candidate, Side, Stand))
                    {
                        UE_LOG(LogTemp, Display, TEXT("OWLOOTLID side balance=%s id=%s yaw=%d failed=%s"), *Row.Key, *Candidate->Id, 45 * Side, Failed);
                        continue;
                    }
                    ClearYaw = 45 * Side;
                    Stop.Lootable = Candidate;
                    Stop.Stand = Stand;
                }
            }
            if (ClearYaw < 0)
            {
                // No lootable of this balance works: the frames are still taken, and the checker reports INCOMPLETE.
                AOpenWillowLootable* Candidate = Row.Value[Order[0]];
                Stop.Lootable = Candidate;
                Stop.Stand = Candidate->GetActorLocation() + Candidate->GetActorForwardVector() * 220.f + FVector(0.f, 0.f, 100.f);
            }
            const AOpenWillowLootable* Chosen = Stop.Lootable.Get();
            // The slot lives on the mesh asset, which every lootable of a balance shares, so one line stands for them all.
            UE_LOG(LogTemp, Display, TEXT("OWLOOTLID material balance=%s id=%s path=%s"), *Row.Key, Chosen ? *Chosen->Id : TEXT("none"),
                Chosen ? *Chosen->RenderedMaterialPath() : TEXT(""));
            UE_LOG(LogTemp, Display, TEXT("OWLOOTLID view balance=%s clear=%d yaw=%d id=%s tried=%d of %d"), *Row.Key, ClearYaw >= 0 ? 1 : 0,
                ClearYaw, Chosen ? *Chosen->Id : TEXT("none"), Tried, Row.Value.Num());
            LidQueue.Add(Stop);
        }
    }
    if (LidQueue.Num() == 0) return false;
    AOpenWillowLootable* Lootable = LidQueue[0].Lootable.Get();
    if (!Lootable)
    {
        LidQueue.RemoveAt(0);
        LidPhase = 0;
        return true;
    }
    const FString FullBalance = Lootable->Balance;
    const FString FileKey = FullBalance.Replace(TEXT("."), TEXT("_"));   // two balances with one short name must not share a file
    auto Shot = [&FileKey, &FullBalance](const TCHAR* Phase)
    {
        const FString File = FString::Printf(TEXT("OWLootLid_%s_%s.png"), *FileKey, Phase);
        AOpenWillowMayaHUD::CaptureShot(TEXT("lid"), File);
        UE_LOG(LogTemp, Display, TEXT("OWLOOTLID shot balance=%s phase=%s"), *FullBalance, Phase);
    };
    switch (LidPhase++)
    {
    case 0:
    {
        // Test fixture: Maya goes to the spot the queue chose and looks at the lootable. What this leaves behind may stay: she
        // stays at the last spot and these lootables stay open, TestOpen skips open ones, the later steps do not depend on
        // where she stands, and the run exits at the end.
        const FVector EyeOffset = Walker.GetPawnViewLocation() - Walker.GetActorLocation();
        const FVector Target = Lootable->GetActorLocation() + FVector(0.f, 0.f, 50.f);
        const FVector From = LidQueue[0].Stand;
        Walker.SetActorLocation(From, false, nullptr, ETeleportType::TeleportPhysics);
        if (AController* Controller = Walker.GetController()) Controller->SetControlRotation((Target - (From + EyeOffset)).Rotation());
        break;
    }
    case 1: Shot(TEXT("closed")); break;
    case 2:
    {
        OpenLootable(Walker, *Lootable);
        // A frame while the clip plays (the 2 s test step is longer than a lid's opening).
        World->GetTimerManager().SetTimer(LidMidTimer, FTimerDelegate::CreateLambda([FileKey, FullBalance]()
            {
                AOpenWillowMayaHUD::CaptureShot(TEXT("lid"), FString::Printf(TEXT("OWLootLid_%s_mid.png"), *FileKey));
                UE_LOG(LogTemp, Display, TEXT("OWLOOTLID shot balance=%s phase=mid"), *FullBalance);
            }), 0.4f, false);
        break;
    }
    default:
        Shot(TEXT("opened"));
        LidQueue.RemoveAt(0);
        LidPhase = 0;
        break;
    }
    return true;
}

void UOpenWillowInteractiveSubsystem::TestOpen(AOpenWillowWalker& Walker)
{
    // The first three closed lootables with different balances, opened without the reach test (a test entry point).
    TSet<FString> Seen;
    int32 Opened = 0;
    for (const TWeakObjectPtr<AOpenWillowLootable>& Candidate : Lootables)
    {
        AOpenWillowLootable* Lootable = Candidate.Get();
        // Skip one the lid capture already opened (a balance with a single placed object), so three balances still open.
        if (!Lootable || Opened >= 3 || Lootable->IsOpened() || Seen.Contains(Lootable->Balance)) continue;
        Seen.Add(Lootable->Balance);
        if (OpenLootable(Walker, *Lootable))
        {
            ++Opened;
            UE_LOG(LogTemp, Display, TEXT("OWINTERACTIVETEST test_open id=%s balance=%s"), *Lootable->Id, *Lootable->Balance);
        }
    }
    UE_LOG(LogTemp, Display, TEXT("OWINTERACTIVETEST opened %d lootable(s) of different balances"), Opened);
}
