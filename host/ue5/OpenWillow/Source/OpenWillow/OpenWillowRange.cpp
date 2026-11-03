#include "OpenWillowRange.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowEnemy.h"
#include "OpenWillowSkills.h"
#include "OpenWillowWalker.h"
#include "Camera/PlayerCameraManager.h"
#include "Components/CapsuleComponent.h"
#include "Components/PrimitiveComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Engine/DamageEvents.h"
#include "Engine/World.h"
#include "GameFramework/Character.h"
#include "GameFramework/DamageType.h"
#include "GameFramework/PlayerController.h"
#include "HAL/FileManager.h"
#include "HAL/IConsoleManager.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "PhysicsEngine/PhysicsAsset.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "UnrealClient.h"
#include <exception>

// Layout and scout rules are host choices (UNVERIFIED in game). The manifest's numbers come from local/ at run time.
namespace
{
constexpr float DummyAhead = 900.f;         // the dummy row is this far ahead of the spot
constexpr float DummySpacing = 300.f;
constexpr float EnemyRing = 1600.f;         // the enemies stand on an arc this far ahead
constexpr float EnemyArcDegrees = 60.f;     // the arc runs from -60 to +60 degrees around the facing
constexpr int32 ScoutRings = 9;             // radii 6000, 7000, ... 14000 uu
constexpr int32 ScoutAngles = 24;
const FVector2D ScoutCentre(0.f, 0.f); // set from ignored local data later in this stack

// -<Switch>=<path>, else the project-relative default.
FString RangeFile(const TCHAR* Switch, const TCHAR* ProjectRelative)
{
    FString Value;
    if (FParse::Value(FCommandLine::Get(), Switch, Value) && !Value.IsEmpty()) return Value;
    return FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), ProjectRelative));
}

// Any Visibility hit between the two points.
bool TraceVisibility(UWorld* World, const FVector& From, const FVector& To, FHitResult& Hit)
{
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowRangeScout), true);
    return World->LineTraceSingleByChannel(Hit, From, To, ECC_Visibility, Query);
}

// The scout rules (host choice, UNVERIFIED): see UOpenWillowRangeSubsystem::ScoutSpot.
bool ScoutAccepts(UWorld* World, const FVector2D& XY, float PlazaZ, FVector& OutFloor)
{
    FHitResult Hit;
    // The floor: a down trace from 6000 uu above the plaza floor, flat enough and within 1500 uu of it.
    if (!TraceVisibility(World, FVector(XY.X, XY.Y, PlazaZ + 6000.f), FVector(XY.X, XY.Y, PlazaZ - 6000.f), Hit)
        || Hit.bStartPenetrating)
        return false;
    if (Hit.ImpactNormal.Z < 0.9f || FMath::Abs(Hit.Location.Z - PlazaZ) > 1500.f) return false;
    const float FloorZ = Hit.Location.Z;
    OutFloor = Hit.Location;
    // Eight probes on a 700 uu circle all find the floor at the same height, and it is flat there.
    for (int32 Probe = 0; Probe < 8; ++Probe)
    {
        const float Rad = FMath::DegreesToRadians(45.f * Probe);
        const FVector2D At = XY + FVector2D(FMath::Cos(Rad), FMath::Sin(Rad)) * 700.f;
        FHitResult Down;
        if (!TraceVisibility(World, FVector(At.X, At.Y, FloorZ + 300.f), FVector(At.X, At.Y, FloorZ - 600.f), Down)
            || Down.bStartPenetrating)
            return false;
        if (FMath::Abs(Down.Location.Z - FloorZ) > 100.f || Down.ImpactNormal.Z < 0.85f) return false;
    }
    // Nothing solid in a 250 uu sphere at floor + 300.
    FCollisionObjectQueryParams Objects;
    Objects.AddObjectTypesToQuery(ECC_WorldStatic);
    Objects.AddObjectTypesToQuery(ECC_WorldDynamic);
    if (World->OverlapAnyTestByObjectType(OutFloor + FVector(0.f, 0.f, 300.f), FQuat::Identity, Objects,
            FCollisionShape::MakeSphere(250.f)))
        return false;
    // Open sky: nothing between floor + 150 and floor + 1500.
    if (TraceVisibility(World, OutFloor + FVector(0.f, 0.f, 150.f), OutFloor + FVector(0.f, 0.f, 1500.f), Hit)) return false;
    return true;
}

// Moves the player pawn to the spot, standing on it, facing Yaw. Host stand-in for the console command and the test.
void TeleportMaya(UWorld* World, const FVector& Spot, float Yaw)
{
    APawn* Pawn = UGameplayStatics::GetPlayerPawn(World, 0);
    if (!Pawn) return;
    float HalfHeight = 90.f;
    if (const ACharacter* Character = Cast<ACharacter>(Pawn))
        HalfHeight = Character->GetCapsuleComponent()->GetScaledCapsuleHalfHeight();
    Pawn->SetActorLocation(Spot + FVector(0.f, 0.f, HalfHeight), false, nullptr, ETeleportType::TeleportPhysics);
    if (APlayerController* PC = UGameplayStatics::GetPlayerController(World, 0)) PC->SetControlRotation(FRotator(0.f, Yaw, 0.f));
    UE_LOG(LogTemp, Display, TEXT("OWRANGE player moved to %s yaw %.0f"), *Spot.ToString(), Yaw);
}
}

namespace
{
// The floor of one formation slot (host choice, UNVERIFIED): a down trace near the spot's height that meets a walkable floor
// (normal Z >= 0.85) within 150 uu of the spot. False when there is none.
bool SlotFloor(UWorld* World, const FVector& Spot, const FVector& Centre, FVector& OutFloor)
{
    FHitResult Hit;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowRangeSlot), true);
    if (!World->LineTraceSingleByChannel(Hit, FVector(Centre.X, Centre.Y, Spot.Z + 300.f), FVector(Centre.X, Centre.Y, Spot.Z - 600.f),
            ECC_Visibility, Query) || Hit.bStartPenetrating)
        return false;
    if (Hit.ImpactNormal.Z < 0.85f || FMath::Abs(Hit.Location.Z - Spot.Z) > 150.f) return false;
    OutFloor = Hit.Location;
    return true;
}
}

bool UOpenWillowRangeSubsystem::FormationSlots(const FVector& Spot, float Yaw, TArray<FVector>& OutDummies, TArray<FVector>& OutEnemies,
    FString& OutWhy) const
{
    // Every slot of the formation must stand on floor: the dummy row DummyAhead ahead and the enemy arc EnemyRing ahead, one
    // slot per enemy the manifest spawns. The scout uses this too, so a scouted spot always holds the whole range.
    UWorld* World = GetWorld();
    OutDummies.Reset();
    OutEnemies.Reset();
    if (!World) { OutWhy = TEXT("no world"); return false; }
    const FVector Forward = FRotator(0.f, Yaw, 0.f).Vector();
    const FVector Side = FRotator(0.f, Yaw + 90.f, 0.f).Vector();
    for (int32 Index = 0; Index < ExpectedDummies; ++Index)
    {
        const FVector Centre = Spot + Forward * DummyAhead + Side * (float(Index) - 1.f) * DummySpacing;
        FVector Floor;
        if (!SlotFloor(World, Spot, Centre, Floor)) { OutWhy = FString::Printf(TEXT("dummy slot %d at %s has no walkable floor"), Index, *Centre.ToString()); return false; }
        OutDummies.Add(Floor);
    }
    const int32 Total = ExpectedEnemies();
    for (int32 Index = 0; Index < Total; ++Index)
    {
        const float Angle = Total > 1 ? FMath::Lerp(-EnemyArcDegrees, EnemyArcDegrees, float(Index) / float(Total - 1)) : 0.f;
        const FVector Centre = Spot + FRotator(0.f, Yaw + Angle, 0.f).Vector() * EnemyRing;
        FVector Floor;
        if (!SlotFloor(World, Spot, Centre, Floor)) { OutWhy = FString::Printf(TEXT("enemy slot %d at %s has no walkable floor"), Index, *Centre.ToString()); return false; }
        OutEnemies.Add(Floor);
    }
    return true;
}

int32 UOpenWillowRangeSubsystem::ExpectedEnemies() const
{
    int32 Total = 0;
    for (const FOpenWillowRangeSpawn& Spawn : Manifest.Spawns)
        if (Manifest.Kinds.Contains(Spawn.Kind)) Total += FMath::Max(0, Spawn.Count);
    return Total;
}

void UOpenWillowRangeSubsystem::EnsureManifest()
{
    if (bManifestLoaded) return;
    bManifestLoaded = true;
    const FString Path = RangeFile(TEXT("owrange="), TEXT("../../../local/range/range_manifest.json"));
    FString Why;
    Manifest = FOpenWillowRangeManifest::Load(Path, Why);
    if (Manifest.bFromFile)
    {
        UE_LOG(LogTemp, Display, TEXT("OWRANGE manifest %s: %d kinds, %d spawn entries, respawn %.0f s"), *Path, Manifest.Kinds.Num(),
            Manifest.Spawns.Num(), Manifest.RespawnSeconds);
    }
    else
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE manifest %s: %s; stand-in kind (engine shapes, health 400)"), *Path, *Why);
    }
}

FOpenWillowRangeManifest FOpenWillowRangeManifest::Load(const FString& Path, FString& OutLog)
{
    FOpenWillowRangeManifest Out;
    FString Text;
    TSharedPtr<FJsonObject> Root;
    FString Schema;
    if (!FFileHelper::LoadFileToString(Text, *Path))
        OutLog = TEXT("no file");
    else if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root)
        OutLog = TEXT("not valid JSON");
    else if (!Root->TryGetStringField(TEXT("schema"), Schema) || Schema != TEXT("ow-range-v1"))
        OutLog = TEXT("schema is not ow-range-v1");
    else
    {
        Out.bFromFile = true;
        double Number = 0.0;
        if (Root->TryGetNumberField(TEXT("respawn_seconds"), Number)) Out.RespawnSeconds = float(Number);
        const TArray<TSharedPtr<FJsonValue>>* SpawnList = nullptr;
        if (Root->TryGetArrayField(TEXT("spawns"), SpawnList))
            for (const TSharedPtr<FJsonValue>& Value : *SpawnList)
            {
                const TSharedPtr<FJsonObject> Entry = Value.IsValid() ? Value->AsObject() : nullptr;
                if (!Entry.IsValid()) continue;
                FOpenWillowRangeSpawn Spawn;
                double Count = 0.0;
                Entry->TryGetStringField(TEXT("kind"), Spawn.Kind);
                if (Entry->TryGetNumberField(TEXT("count"), Count)) Spawn.Count = int32(Count);
                Out.Spawns.Add(Spawn);
            }
        const TSharedPtr<FJsonObject>* KindObjects = nullptr;
        if (Root->TryGetObjectField(TEXT("kinds"), KindObjects))
            for (const auto& Pair : (*KindObjects)->Values)
            {
                const TSharedPtr<FJsonObject> Obj = Pair.Value.IsValid() ? Pair.Value->AsObject() : nullptr;
                if (!Obj.IsValid()) continue;
                FOpenWillowRangeKind Kind;
                Kind.Name = Pair.Key;
                Obj->TryGetStringField(TEXT("label"), Kind.Label);
                Obj->TryGetStringField(TEXT("mesh"), Kind.Mesh);
                Obj->TryGetStringField(TEXT("idle"), Kind.Idle);
                Obj->TryGetStringField(TEXT("death"), Kind.Death);
                Obj->TryGetStringField(TEXT("walk"), Kind.Walk);
                Obj->TryGetStringField(TEXT("run"), Kind.Run);
                Obj->TryGetStringField(TEXT("hit"), Kind.Hit);
                Obj->TryGetStringField(TEXT("melee"), Kind.Melee);
                Obj->TryGetStringField(TEXT("head_bone"), Kind.HeadBone);
                auto Read = [&Obj](const TCHAR* Field, float& Target)
                {
                    double Value = 0.0;
                    if (Obj->TryGetNumberField(Field, Value)) Target = float(Value);
                };
                Read(TEXT("mesh_offset_z"), Kind.MeshOffsetZ);
                Read(TEXT("mesh_scale"), Kind.MeshScale);
                Read(TEXT("collision_radius"), Kind.CollisionRadius);
                Read(TEXT("collision_height"), Kind.CollisionHeight);
                Read(TEXT("head_radius"), Kind.HeadRadius);
                Read(TEXT("walk_speed"), Kind.WalkSpeed);
                Read(TEXT("patrol_radius"), Kind.PatrolRadius);
                Read(TEXT("run_speed"), Kind.RunSpeed);
                Read(TEXT("shield"), Kind.Shield);
                Read(TEXT("shield_delay"), Kind.ShieldDelay);
                Read(TEXT("shield_rate"), Kind.ShieldRate);
                // A table of level -> number, keyed by the level as a string in the manifest.
                auto ReadLevels = [&Obj](const TCHAR* Field, TMap<int32, float>& Table)
                {
                    const TSharedPtr<FJsonObject>* Object = nullptr;
                    if (Obj->TryGetObjectField(Field, Object))
                        for (const auto& Entry : (*Object)->Values)
                        {
                            double Value = 0.0;
                            if (Entry.Value.IsValid() && Entry.Value->TryGetNumber(Value))
                                Table.Add(FCString::Atoi(*Entry.Key), float(Value));
                        }
                };
                ReadLevels(TEXT("health_by_level"), Kind.HealthByLevel);
                ReadLevels(TEXT("melee_arrival_by_level"), Kind.MeleeArrivalByLevel);
                ReadLevels(TEXT("melee_ratio_by_level"), Kind.MeleeRatioByLevel);
                Out.Kinds.Add(Kind.Name, Kind);
            }
        OutLog = TEXT("ok");
    }

    if (!Out.bFromFile)
    {
        // Stand-in: one engine-shape kind (no mesh), three of them, the 400 health stand-in and the default respawn.
        Out = FOpenWillowRangeManifest();
        FOpenWillowRangeKind Stand;
        Stand.Name = TEXT("stand-in");
        Stand.Label = TEXT("stand-in (engine shapes)");
        Out.Kinds.Add(Stand.Name, Stand);
        FOpenWillowRangeSpawn Spawn;
        Spawn.Kind = Stand.Name;
        Spawn.Count = 3;
        Out.Spawns.Add(Spawn);
    }
    return Out;
}

void UOpenWillowRangeSubsystem::Initialize(FSubsystemCollectionBase& Collection)
{
    Super::Initialize(Collection);
    bTestMode = FParse::Param(FCommandLine::Get(), TEXT("owrangetest"));
}

bool UOpenWillowRangeSubsystem::DoesSupportWorldType(const EWorldType::Type WorldType) const
{
    return WorldType == EWorldType::Game || WorldType == EWorldType::PIE;
}

TStatId UOpenWillowRangeSubsystem::GetStatId() const
{
    RETURN_QUICK_DECLARE_CYCLE_STAT(UOpenWillowRangeSubsystem, STATGROUP_Tickables);
}

void UOpenWillowRangeSubsystem::Tick(float DeltaTime)
{
    const float Now = GetWorld()->GetTimeSeconds();
    if (bBuilt) UpdateThreatDen(Den, Now);   // PR2: the den's fall-back check (every second)
    if (bTestMode && !bTestDone) TickTest(Now);
}

bool UOpenWillowRangeSubsystem::LoadSliceData()
{
    // The Fire mission's stock dummy comes from the manifests the quest reads (-owslice, -ownpcs, -owaudio). Loaded once.
    if (bSliceTried) return bSliceLoaded;
    bSliceTried = true;
    FString WorldFile, NpcFile, AudioFile;
    if (!FParse::Value(FCommandLine::Get(), TEXT("owslice="), WorldFile) || !FParse::Value(FCommandLine::Get(), TEXT("ownpcs="), NpcFile)
        || !FParse::Value(FCommandLine::Get(), TEXT("owaudio="), AudioFile))
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE stock dummy data missing (needs -owslice, -ownpcs, -owaudio): engine-shape dummies"));
        return false;
    }
    try
    {
        SliceData.Load(WorldFile, NpcFile, AudioFile);
        bSliceLoaded = true;
    }
    catch (const std::exception& Error)
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE stock dummy data did not load (%s): engine-shape dummies"), UTF8_TO_TCHAR(Error.what()));
    }
    return bSliceLoaded;
}

int32 UOpenWillowRangeSubsystem::EnemyLevel() const
{
    if (LevelOverride > 0) return LevelOverride;
    const AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(UGameplayStatics::GetPlayerPawn(this, 0));
    if (Walker && Walker->GetSkills()) return Walker->GetSkills()->GetLevel();
    return 1;
}

bool UOpenWillowRangeSubsystem::BuildRange(const FVector& Spot, float Yaw)
{
    UWorld* World = GetWorld();
    if (!World) return false;
    ClearRange();
    EnsureManifest();
    // The whole formation must stand on walkable floor before anything is spawned (an incomplete range is a failed build).
    TArray<FVector> DummySlots, EnemySlots;
    FString Why;
    if (!FormationSlots(Spot, Yaw, DummySlots, EnemySlots, Why))
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE build failed at %s yaw %.0f: %s"), *Spot.ToString(), Yaw, *Why);
        return false;
    }
    SpotNow = Spot;
    SpotYawNow = Yaw;
    // PR2: the range is one den, centred on the spot; its record starts fresh.
    Den.Centre = FVector2D(Spot.X, Spot.Y);
    Den.ResetRecord();
    // Three static dummies in a row, facing the spot. The Fire mission's stock dummy when its data loads, else engine shapes.
    const bool bStockDummy = LoadSliceData();
    for (int32 Index = 0; Index < DummySlots.Num(); ++Index)
    {
        const FVector& At = DummySlots[Index];
        FActorSpawnParameters Params;
        Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
        AOpenWillowCombatTarget* Dummy = World->SpawnActor<AOpenWillowCombatTarget>(At, FRotator(0.f, Yaw + 180.f, 0.f), Params);
        if (!Dummy) break;
        const bool bStock = bStockDummy
            && Dummy->UseStockPawn(SliceData.Dummy.Mesh, SliceData.Dummy.Idle, SliceData.Dummy.Death, SliceData.Dummy.MeshOffset);
        Dummy->SetRespawn(true, Manifest.RespawnSeconds);
        Dummy->SetInvulnerable(bModeInvulnerable);
        Dummies.Add(Dummy);
        UE_LOG(LogTemp, Display, TEXT("OWRANGE dummy %d at %s stock=%s respawn %.0f s"), Index, *At.ToString(),
            bStock ? TEXT("yes") : TEXT("no (engine shapes)"), Manifest.RespawnSeconds);
    }

    // Moving enemies: the manifest's spawns on the arc slots, in manifest order, each facing the spot.
    const int32 Level = EnemyLevel();
    int32 Index = 0;
    for (const FOpenWillowRangeSpawn& Spawn : Manifest.Spawns)
    {
        const FOpenWillowRangeKind* Kind = Manifest.Kinds.Find(Spawn.Kind);
        if (!Kind)
        {
            UE_LOG(LogTemp, Warning, TEXT("OWRANGE spawn kind %s is not in the manifest, skipped"), *Spawn.Kind);
            continue;
        }
        for (int32 Copy = 0; Copy < Spawn.Count && EnemySlots.IsValidIndex(Index); ++Copy, ++Index)
        {
            const FVector& At = EnemySlots[Index];
            FActorSpawnParameters Params;
            Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
            AOpenWillowEnemy* Enemy = World->SpawnActor<AOpenWillowEnemy>(At, FRotator(0.f, (Spot - At).Rotation().Yaw, 0.f), Params);
            if (!Enemy) continue;
            Enemy->SetupKind(*Kind, Level, Manifest.RespawnSeconds);
            Enemy->SetInvulnerable(bModeInvulnerable);
            Enemy->SetPassive(bModePassive);
            Enemy->SetDen(&Den);
            Enemy->OnEnemyDied.AddUObject(this, &UOpenWillowRangeSubsystem::HandleEnemyDied);
            Enemies.Add(Enemy);
        }
    }
    // Every slot must hold its actor; an incomplete range is a failed build (nothing is left standing).
    if (Dummies.Num() != DummySlots.Num() || Enemies.Num() != EnemySlots.Num())
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE build failed: spawned %d of %d dummies and %d of %d enemies"), Dummies.Num(),
            DummySlots.Num(), Enemies.Num(), EnemySlots.Num());
        ClearRange();
        return false;
    }
    bBuilt = true;
    UE_LOG(LogTemp, Display, TEXT("OWRANGE built spot=%s yaw=%.0f dummies=%d enemies=%d level=%d invulnerable=%d passive=%d"),
        *Spot.ToString(), Yaw, Dummies.Num(), Enemies.Num(), Level, bModeInvulnerable ? 1 : 0, bModePassive ? 1 : 0);
    return true;
}

void UOpenWillowRangeSubsystem::ClearRange()
{
    int32 Count = 0;
    for (const auto& Weak : Dummies)
        if (AOpenWillowCombatTarget* Dummy = Weak.Get())
        {
            Dummy->Destroy();
            ++Count;
        }
    for (const auto& Weak : Enemies)
        if (AOpenWillowEnemy* Enemy = Weak.Get())
        {
            Enemy->Destroy();
            ++Count;
        }
    Dummies.Reset();
    Enemies.Reset();
    bBuilt = false;
    if (Count > 0) UE_LOG(LogTemp, Display, TEXT("OWRANGE cleared %d range actor(s)"), Count);
}

void UOpenWillowRangeSubsystem::RebuildLast()
{
    if (bBuilt) BuildRange(SpotNow, SpotYawNow);
}

void UOpenWillowRangeSubsystem::SetMode(bool bInvulnerableNow, bool bPassiveNow)
{
    bModeInvulnerable = bInvulnerableNow;
    bModePassive = bPassiveNow;
    for (const auto& Weak : Dummies)
        if (AOpenWillowCombatTarget* Dummy = Weak.Get()) Dummy->SetInvulnerable(bModeInvulnerable);
    for (const auto& Weak : Enemies)
        if (AOpenWillowEnemy* Enemy = Weak.Get())
        {
            Enemy->SetInvulnerable(bModeInvulnerable);
            Enemy->SetPassive(bModePassive);
        }
    UE_LOG(LogTemp, Display, TEXT("OWRANGE mode invulnerable=%d passive=%d (%d dummies, %d enemies)"), bModeInvulnerable ? 1 : 0,
        bModePassive ? 1 : 0, Dummies.Num(), Enemies.Num());
}

bool UOpenWillowRangeSubsystem::ResolveSpot(FVector& OutSpot, float& OutYaw, FString& OutSource)
{
    // 1. -owrangeat=X,Y,Z[,Yaw]
    FString At;
    if (FParse::Value(FCommandLine::Get(), TEXT("owrangeat="), At))
    {
        TArray<FString> Parts;
        At.ParseIntoArray(Parts, TEXT(","));
        if (Parts.Num() >= 3)
        {
            OutSpot = FVector(FCString::Atof(*Parts[0]), FCString::Atof(*Parts[1]), FCString::Atof(*Parts[2]));
            OutYaw = Parts.Num() >= 4 ? FCString::Atof(*Parts[3]) : 0.f;
            OutSource = TEXT("owrangeat");
            return true;
        }
    }
    // 2. local/range/range.json: {"spot": {"x": .., "y": .., "z": .., "yaw": ..}} (yaw optional)
    const FString File = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/range/range.json")));
    FString Text;
    TSharedPtr<FJsonObject> Root;
    if (FFileHelper::LoadFileToString(Text, *File) && FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) && Root)
    {
        const TSharedPtr<FJsonObject>* Spot = nullptr;
        double X = 0.0, Y = 0.0, Z = 0.0, Yaw = 0.0;
        if (Root->TryGetObjectField(TEXT("spot"), Spot) && (*Spot)->TryGetNumberField(TEXT("x"), X)
            && (*Spot)->TryGetNumberField(TEXT("y"), Y) && (*Spot)->TryGetNumberField(TEXT("z"), Z))
        {
            (*Spot)->TryGetNumberField(TEXT("yaw"), Yaw);
            OutSpot = FVector(X, Y, Z);
            OutYaw = float(Yaw);
            OutSource = TEXT("range.json");
            return true;
        }
    }
    // 3. The scout.
    FString Log;
    if (ScoutSpot(OutSpot, OutYaw, Log))
    {
        OutSource = TEXT("scout");
        return true;
    }
    UE_LOG(LogTemp, Warning, TEXT("OWRANGE no spot: %s"), *Log);
    return false;
}

bool UOpenWillowRangeSubsystem::ScoutSpot(FVector& OutSpot, float& OutYaw, FString& OutLog)
{
    EnsureManifest();   // the formation check needs the number of enemies
    UWorld* World = GetWorld();
    FHitResult Plaza;
    if (!World || !TraceVisibility(World, FVector(ScoutCentre.X, ScoutCentre.Y, 10000.f),
            FVector(ScoutCentre.X, ScoutCentre.Y, -10000.f), Plaza))
    {
        OutLog = TEXT("no plaza floor under the scout centre");
        return false;
    }
    const float PlazaZ = Plaza.Location.Z;
    const FVector Centre(ScoutCentre.X, ScoutCentre.Y, PlazaZ);
    // Rings closest first; the first accepted candidate of the first ring that has one wins (all of its ring is counted).
    for (int32 Ring = 0; Ring < ScoutRings; ++Ring)
    {
        const float Radius = 6000.f + 1000.f * float(Ring);
        int32 Accepted = 0;
        bool bWon = false;
        for (int32 Index = 0; Index < ScoutAngles; ++Index)
        {
            const float Rad = FMath::DegreesToRadians(360.f * float(Index) / float(ScoutAngles));
            const FVector2D XY = ScoutCentre + FVector2D(FMath::Cos(Rad), FMath::Sin(Rad)) * Radius;
            FVector Floor;
            if (!ScoutAccepts(World, XY, PlazaZ, Floor)) continue;
            // The whole formation (dummy row and enemy arc, facing away from the town) must have floor too.
            TArray<FVector> DummySlots, EnemySlots;
            FString Why;
            if (!FormationSlots(Floor, (Floor - Centre).Rotation().Yaw, DummySlots, EnemySlots, Why)) continue;
            ++Accepted;
            if (!bWon)
            {
                bWon = true;
                OutSpot = Floor;
                OutYaw = (Floor - Centre).Rotation().Yaw;   // facing away from the town
            }
        }
        UE_LOG(LogTemp, Display, TEXT("OWRANGE scout ring=%.0f accepted=%d of %d"), Radius, Accepted, ScoutAngles);
        if (bWon)
        {
            UE_LOG(LogTemp, Display, TEXT("OWRANGE spot source=scout at=%.0f,%.0f,%.0f yaw=%.0f ring=%.0f"), OutSpot.X, OutSpot.Y, OutSpot.Z,
                OutYaw, Radius);
            OutLog = TEXT("scout");
            return true;
        }
    }
    OutLog = TEXT("no accepted candidate on rings 6000-14000 uu");
    return false;
}

void UOpenWillowRangeSubsystem::Report() const
{
    if (Dummies.Num() == 0 && Enemies.Num() == 0) UE_LOG(LogTemp, Display, TEXT("OWRANGE report: no range actors"));
    for (const auto& Weak : Dummies)
        if (const AOpenWillowCombatTarget* Dummy = Weak.Get())
            UE_LOG(LogTemp, Display, TEXT("OWRANGE report dummy=%s health=%.0f/%.0f dead=%d invulnerable=%d stock=%s location=%s"),
                *Dummy->GetName(), Dummy->GetHealth(), Dummy->GetMaxHealth(), Dummy->IsDead() ? 1 : 0, Dummy->IsInvulnerable() ? 1 : 0,
                Dummy->IsStockPawn() ? TEXT("yes") : TEXT("no"), *Dummy->GetActorLocation().ToString());
    for (const auto& Weak : Enemies)
        if (const AOpenWillowEnemy* Enemy = Weak.Get())
            UE_LOG(LogTemp, Display, TEXT("OWRANGE report %s"), *Enemy->StateReport());
}

void UOpenWillowRangeSubsystem::HandleEnemyDied(AOpenWillowEnemy* Enemy, AActor* Killer)
{
    ++DeathsSeen;
}

// ---------------------------------------------------------------------------------------------- test driver (-owrangetest)

void UOpenWillowRangeSubsystem::TestCheck(const TCHAR* Step, bool bPass, const FString& Detail)
{
    if (bPass) ++TestPass;
    else ++TestFail;
    UE_LOG(LogTemp, Display, TEXT("OWRANGETEST step=%s result=%s %s"), Step, bPass ? TEXT("PASS") : TEXT("FAIL"), *Detail);
}

void UOpenWillowRangeSubsystem::TestSkip(const TCHAR* Step, const TCHAR* Reason)
{
    ++TestSkipped;
    UE_LOG(LogTemp, Display, TEXT("OWRANGETEST step=%s result=SKIP reason=%s"), Step, Reason);
}

void UOpenWillowRangeSubsystem::TestHit(AOpenWillowCombatTarget* Target, float Amount, UPrimitiveComponent* Where,
    AController* Instigator, AActor* Causer)
{
    if (!Target) return;
    // A hit as a shot would make it: a point hit on the given component, shot horizontally toward its centre (null: the
    // target's origin). A body hit lands in the lower part of the volume, so its shot line stays below the head sphere.
    FHitResult Hit;
    FVector At = Where ? Where->GetComponentLocation() : Target->GetActorLocation();
    if (const auto* Enemy = Cast<AOpenWillowEnemy>(Target); Enemy && Where && Where == Enemy->BodyComponent())
        At -= FVector(0.f, 0.f, 0.6f * Where->Bounds.BoxExtent.Z);
    const FVector Direction = (FVector(At.X, At.Y, 0.f) - FVector(TestSpot.X, TestSpot.Y, 0.f)).GetSafeNormal();
    Hit.Component = Where;
    Hit.Location = At;
    Hit.ImpactPoint = At - Direction * 5.f;
    UGameplayStatics::ApplyPointDamage(Target, Amount, Direction.IsNearlyZero() ? FVector(1.f, 0.f, 0.f) : Direction, Hit, Instigator,
        Causer, UDamageType::StaticClass());
}

void UOpenWillowRangeSubsystem::TestShot(int32 Number)
{
    if (!FParse::Param(FCommandLine::Get(), TEXT("owrangeshots"))) return;
    const FString Dir = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/range/shots")));
    IFileManager::Get().MakeDirectory(*Dir, true);
    FScreenshotRequest::RequestScreenshot(FPaths::Combine(Dir, FString::Printf(TEXT("range_%d.png"), Number)), true, false);
}

void UOpenWillowRangeSubsystem::TickTest(float Now)
{
    // Steps: 0 wait for Maya, build and move her; 1 teleport; 2 placement; 3 body hit; 4 crit hit; 5 crit trace; 6 shield order;
    // 7 invulnerable; 8 passive; 9 passive held; 10 patrol; 11 death; 12 ragdoll; 13 respawn; 14-15 dummy respawn;
    // 16-36 enemy AI (TickAITest); 99 summary.
    UWorld* World = GetWorld();
    APawn* Pawn = UGameplayStatics::GetPlayerPawn(World, 0);
    APlayerController* PC = UGameplayStatics::GetPlayerController(World, 0);
    AOpenWillowEnemy* Enemy0 = Enemies.Num() > 0 ? Enemies[0].Get() : nullptr;
    AOpenWillowEnemy* Enemy1 = Enemies.Num() > 1 ? Enemies[1].Get() : nullptr;
    AOpenWillowCombatTarget* Dummy0 = Dummies.Num() > 0 ? Dummies[0].Get() : nullptr;

    if (TestStep >= 16 && TestStep < 99)
    {
        TickAITest(Now);
        return;
    }
    switch (TestStep)
    {
    case 0:
    {
        if (!Pawn) return;
        if (TestArmedAt < 0.f) { TestArmedAt = Now; return; }
        if (Now - TestArmedAt < 4.f) return;
        UE_LOG(LogTemp, Display, TEXT("OWRANGETEST start"));
        FVector Spot;
        float Yaw = 0.f;
        FString Source;
        if (!ResolveSpot(Spot, Yaw, Source) || !BuildRange(Spot, Yaw))
        {
            TestCheck(TEXT("teleport"), false, TEXT("no spot or the range did not build"));
            TestStep = 99;
            return;
        }
        TestSpot = Spot;
        TeleportMaya(World, Spot, Yaw);
        UE_LOG(LogTemp, Display, TEXT("OWRANGETEST spot source=%s at=%s yaw=%.0f"), *Source, *Spot.ToString(), Yaw);
        StepStart = Now;
        TestStep = 1;
        return;
    }
    case 1:
    {
        if (!Pawn || Now - StepStart < 0.5f) return;
        const float Dist = FVector::Dist2D(Pawn->GetActorLocation(), TestSpot);
        TestCheck(TEXT("teleport"), Dist <= 100.f, FString::Printf(TEXT("%.1f uu from the spot"), Dist));
        TestShot(1);
        TestStep = 2;
        return;
    }
    case 2:
    {
        int32 Checked = 0, Off = 0;
        auto Probe = [World, &Checked, &Off](AActor* Actor)
        {
            ++Checked;
            const FVector At = Actor->GetActorLocation();
            FVector Floor;
            if (!AOpenWillowEnemy::TraceFloor(World, FVector2D(At.X, At.Y), At.Z + 100.f, At.Z - 300.f, Actor, Floor)
                || FMath::Abs(Floor.Z - At.Z) > 50.f)
                ++Off;
        };
        for (const auto& Weak : Dummies)
            if (AOpenWillowCombatTarget* Dummy = Weak.Get()) Probe(Dummy);
        for (const auto& Weak : Enemies)
            if (AOpenWillowEnemy* Enemy = Weak.Get()) Probe(Enemy);
        // The build is complete: as many dummies and enemies as the formation and the manifest ask for, all on the floor.
        const bool bCounts = Dummies.Num() == ExpectedDummies && Enemies.Num() == ExpectedEnemies() && ExpectedEnemies() > 0;
        TestCheck(TEXT("placement"), bCounts && Off == 0, FString::Printf(TEXT("dummies %d/%d, enemies %d/%d, %d actor(s) checked, %d off the floor"),
            Dummies.Num(), ExpectedDummies, Enemies.Num(), ExpectedEnemies(), Checked, Off));
        TestStep = 3;
        return;
    }
    case 3:
    {
        if (!Enemy0)
        {
            TestSkip(TEXT("body_hit"), TEXT("no enemy"));
            TestStep = 4;
            return;
        }
        const float Before = Enemy0->GetHealth();
        TestHit(Enemy0, 10.f, Enemy0->BodyComponent());
        const float Delta = Before - Enemy0->GetHealth();
        const bool bCrit = Enemy0->Popups.Num() > 0 && Enemy0->Popups.Last().bCritical;
        TestCheck(TEXT("body_hit"), FMath::IsNearlyEqual(Delta, 10.f, 0.01f) && !bCrit,
            FString::Printf(TEXT("health delta %.2f, crit=%d"), Delta, bCrit ? 1 : 0));
        TestStep = 4;
        return;
    }
    case 4:
    {
        UPrimitiveComponent* Crit = Enemy0 ? Enemy0->CritComponent() : nullptr;
        if (!Crit)
        {
            TestSkip(TEXT("crit_hit"), TEXT("no critical region"));
            TestSkip(TEXT("popup_crit"), TEXT("no critical region"));
            TestStep = 5;
            return;
        }
        const float Before = Enemy0->GetHealth();
        TestHit(Enemy0, 10.f, Crit);
        const float Delta = Before - Enemy0->GetHealth();
        const bool bCrit = Enemy0->Popups.Num() > 0 && Enemy0->Popups.Last().bCritical;
        TestCheck(TEXT("crit_hit"), FMath::IsNearlyEqual(Delta, 20.f, 0.01f) && bCrit,
            FString::Printf(TEXT("health delta %.2f, crit=%d"), Delta, bCrit ? 1 : 0));
        TestCheck(TEXT("popup_crit"), bCrit, FString::Printf(TEXT("last popup critical=%d"), bCrit ? 1 : 0));
        TestStep = 5;
        return;
    }
    case 5:
    {
        UPrimitiveComponent* Crit = Enemy0 ? Enemy0->CritComponent() : nullptr;
        if (!Crit || !PC || !Pawn)
        {
            TestSkip(TEXT("crit_trace"), TEXT("no critical region"));
            TestStep = 6;
            return;
        }
        // Aim the view at the critical region's centre and trace what the camera sees first.
        const APlayerCameraManager* Camera = UGameplayStatics::GetPlayerCameraManager(World, 0);
        const FVector Eye = Camera ? Camera->GetCameraLocation() : Pawn->GetActorLocation();
        const FVector Aim = Crit->GetComponentLocation();
        PC->SetControlRotation((Aim - Eye).Rotation());
        FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowRangeCritTrace), true);
        Query.AddIgnoredActor(Pawn);
        FHitResult Hit;
        const bool bHit = World->LineTraceSingleByChannel(Hit, Eye, Eye + (Aim - Eye) * 1.2f, ECC_Visibility, Query);
        FString First = TEXT("nothing hit");
        if (bHit && Hit.GetActor())
            First = FString::Printf(TEXT("first hit %s on %s"), *Hit.GetActor()->GetName(),
                Hit.Component.IsValid() ? *Hit.Component->GetName() : TEXT("no component"));
        // What counts is the enemy's own decision for that hit (the body capsule may be met first; IsCritRegion follows the
        // shot line into the head sphere).
        const bool bCrit = bHit && Hit.GetActor() == Enemy0
            && Enemy0->IsCritRegion(Hit.Component.Get(), Hit.BoneName, Hit.ImpactPoint, (Aim - Eye).GetSafeNormal());
        TestCheck(TEXT("crit_trace"), bCrit, First + FString::Printf(TEXT(", critical=%d"), bCrit ? 1 : 0));
        TestStep = 6;
        return;
    }
    case 6:
    {
        if (!Enemy1)
        {
            TestSkip(TEXT("shield_order"), TEXT("needs two enemies"));
            TestStep = 7;
            return;
        }
        Enemy1->SetShieldForTest(15.f);
        const float H0 = Enemy1->GetHealth();
        TestHit(Enemy1, 10.f, Enemy1->BodyComponent());
        const float H1 = Enemy1->GetHealth();
        const float S1 = Enemy1->ShieldNow();
        TestHit(Enemy1, 10.f, Enemy1->BodyComponent());
        const float H2 = Enemy1->GetHealth();
        const float S2 = Enemy1->ShieldNow();
        const bool bOk = FMath::IsNearlyEqual(S1, 5.f, 0.01f) && FMath::IsNearlyEqual(H0 - H1, 0.f, 0.01f)
            && FMath::IsNearlyEqual(S2, 0.f, 0.01f) && FMath::IsNearlyEqual(H1 - H2, 5.f, 0.01f);
        TestCheck(TEXT("shield_order"), bOk, FString::Printf(TEXT("shield 15 -> %.1f -> %.1f, health loss %.1f then %.1f"), S1, S2, H0 - H1, H1 - H2));
        TestStep = 7;
        return;
    }
    case 7:
    {
        if (!Enemy0)
        {
            TestSkip(TEXT("invulnerable"), TEXT("no enemy"));
            TestStep = 8;
            return;
        }
        SetMode(true, false);
        const float Before = Enemy0->GetHealth();
        const int32 PopupsNow = Enemy0->Popups.Num();
        TestHit(Enemy0, 50.f, Enemy0->BodyComponent());
        const bool bOk = FMath::IsNearlyEqual(Enemy0->GetHealth(), Before, 0.01f) && Enemy0->Popups.Num() == PopupsNow + 1;
        TestCheck(TEXT("invulnerable"), bOk, FString::Printf(TEXT("health %.1f -> %.1f, popups %d -> %d"), Before, Enemy0->GetHealth(),
            PopupsNow, Enemy0->Popups.Num()));
        SetMode(false, false);
        TestStep = 8;
        return;
    }
    case 8:
    {
        if (Enemies.Num() == 0)
        {
            TestSkip(TEXT("passive"), TEXT("no enemies"));
            TestSkip(TEXT("patrol"), TEXT("no enemies"));
            TestStep = 11;
            return;
        }
        SetMode(false, true);
        Recorded.Reset();
        for (const auto& Weak : Enemies)
        {
            const AOpenWillowEnemy* Enemy = Weak.Get();
            Recorded.Add(Enemy ? Enemy->GetActorLocation() : FVector::ZeroVector);
        }
        StepStart = Now;
        TestStep = 9;
        return;
    }
    case 9:
    {
        if (Now - StepStart < 2.f) return;
        float Most = 0.f;
        for (int32 Index = 0; Index < Enemies.Num() && Recorded.IsValidIndex(Index); ++Index)
            if (const AOpenWillowEnemy* Enemy = Enemies[Index].Get())
                Most = FMath::Max(Most, FVector::Dist(Enemy->GetActorLocation(), Recorded[Index]));
        TestCheck(TEXT("passive"), Most <= 5.f, FString::Printf(TEXT("most moved %.1f uu in 2 s"), Most));
        SetMode(false, false);
        Recorded.Reset();
        for (const auto& Weak : Enemies)
        {
            const AOpenWillowEnemy* Enemy = Weak.Get();
            Recorded.Add(Enemy ? Enemy->GetActorLocation() : FVector::ZeroVector);
        }
        StepStart = Now;
        TestStep = 10;
        return;
    }
    case 10:
    {
        if (Now - StepStart < 3.f) return;
        float Most = 0.f;
        for (int32 Index = 0; Index < Enemies.Num() && Recorded.IsValidIndex(Index); ++Index)
            if (const AOpenWillowEnemy* Enemy = Enemies[Index].Get())
                Most = FMath::Max(Most, FVector::Dist(Enemy->GetActorLocation(), Recorded[Index]));
        TestCheck(TEXT("patrol"), Most > 30.f, FString::Printf(TEXT("most moved %.1f uu in 3 s"), Most));
        TestShot(8);
        TestStep = 11;
        return;
    }
    case 11:
    {
        if (!Enemy0)
        {
            TestSkip(TEXT("death"), TEXT("no enemy"));
            TestStep = 14;
            return;
        }
        DeathsBefore = DeathsSeen;
        TestHit(Enemy0, 1.e6f, Enemy0->BodyComponent());
        const bool bOk = Enemy0->IsDead() && DeathsSeen == DeathsBefore + 1;
        TestCheck(TEXT("death"), bOk, FString::Printf(TEXT("dead=%d death events=%d"), Enemy0->IsDead() ? 1 : 0, DeathsSeen - DeathsBefore));
        DeathAt = Now;
        TestStep = 12;
        return;
    }
    case 12:
    {
        if (Now - DeathAt < 1.04f) return;   // the ragdoll delay (0.84 s) plus 0.2 s
        const USkeletalMeshComponent* Mesh = Enemy0 ? Enemy0->GetStockMesh() : nullptr;
        if (!Mesh) TestSkip(TEXT("ragdoll"), TEXT("engine shapes, no stock mesh"));
        else if (!Mesh->GetPhysicsAsset()) TestSkip(TEXT("ragdoll"), TEXT("no physics asset"));
        else TestCheck(TEXT("ragdoll"), Mesh->IsSimulatingPhysics(), FString::Printf(TEXT("simulating=%d"), Mesh->IsSimulatingPhysics() ? 1 : 0));
        TestStep = 13;
        return;
    }
    case 13:
    {
        if (Now - DeathAt < Manifest.RespawnSeconds + 0.5f) return;
        if (!Enemy0)
        {
            TestSkip(TEXT("respawn"), TEXT("no enemy"));
            TestStep = 14;
            return;
        }
        const float Gap = FVector::Dist(Enemy0->GetActorLocation(), Enemy0->GetHome());
        const bool bOk = !Enemy0->IsDead() && FMath::IsNearlyEqual(Enemy0->GetHealth(), Enemy0->GetMaxHealth(), 0.01f) && Gap < 50.f;
        TestCheck(TEXT("respawn"), bOk, FString::Printf(TEXT("dead=%d health %.0f/%.0f, %.1f uu from home"), Enemy0->IsDead() ? 1 : 0,
            Enemy0->GetHealth(), Enemy0->GetMaxHealth(), Gap));
        TestStep = 14;
        return;
    }
    case 14:
    {
        if (!Dummy0)
        {
            TestSkip(TEXT("dummy_respawn"), TEXT("no dummy"));
            TestStep = 16;
            return;
        }
        TestHit(Dummy0, 1.e6f, nullptr);
        if (!Dummy0->IsDead())
        {
            TestCheck(TEXT("dummy_respawn"), false, TEXT("the dummy did not die"));
            TestStep = 16;
            return;
        }
        DeathAt = Now;
        TestStep = 15;
        return;
    }
    case 15:
    {
        if (Now - DeathAt < Manifest.RespawnSeconds + 0.5f) return;
        const bool bOk = Dummy0 && !Dummy0->IsDead() && FMath::IsNearlyEqual(Dummy0->GetHealth(), Dummy0->GetMaxHealth(), 0.01f);
        TestCheck(TEXT("dummy_respawn"), bOk, FString::Printf(TEXT("alive=%d health %.0f/%.0f"), (Dummy0 && !Dummy0->IsDead()) ? 1 : 0,
            Dummy0 ? Dummy0->GetHealth() : 0.f, Dummy0 ? Dummy0->GetMaxHealth() : 0.f));
        TestStep = 16;
        return;
    }
    case 99:
    default:
    {
        UE_LOG(LogTemp, Display, TEXT("OWRANGETEST SUMMARY result=%s pass=%d fail=%d skip=%d"), TestFail == 0 ? TEXT("PASS") : TEXT("FAIL"),
            TestPass, TestFail, TestSkipped);
        bTestDone = true;
        if (PC) PC->ConsoleCommand(TEXT("quit"));
        return;
    }
    }
}

void UOpenWillowRangeSubsystem::TickAITest(float Now)
{
    // Steps 16-36 (PR2, enemy AI). Each step sets its scene and checks it after its wait. The threat record is reset between
    // steps (test only), and the other enemies are passive where a step needs one enemy alone. Maya is not healed: a step
    // that finds her down logs it and fails. The host stand-ins are UNVERIFIED in game; the timings are the spec's tolerances.
    UWorld* World = GetWorld();
    APawn* Pawn = UGameplayStatics::GetPlayerPawn(World, 0);
    APlayerController* PC = UGameplayStatics::GetPlayerController(World, 0);
    const AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(Pawn);
    AOpenWillowEnemy* E0 = Enemies.Num() > 0 ? Enemies[0].Get() : nullptr;
    if (!Walker || !PC)
    {
        TestSkip(TEXT("ai"), TEXT("no walker pawn"));
        TestStep = 99;
        return;
    }
    if (!E0)
    {
        const TCHAR* Names[] = {TEXT("notice_proximity"), TEXT("notice_sight"), TEXT("no_notice_far"), TEXT("provoked"),
            TEXT("approach"), TEXT("melee_hit"), TEXT("melee_interval"), TEXT("melee_miss"), TEXT("forget"), TEXT("passive_no_melee")};
        for (const TCHAR* Name : Names) TestSkip(Name, TEXT("no enemy"));
        TestStep = 99;
        return;
    }
    const FOpenWillowEnemyBrain& Brain = E0->GetBrain();
    const int32 Seen = OpenWillowThreat::SeenInThreatRadius;

    // Maya on the floor under XY (her origin at the floor plus her half height), facing Yaw. False when there is no floor there.
    auto PlaceMaya = [this, World, Pawn](const FVector2D& XY, float Yaw) -> bool
    {
        FVector Floor;
        if (!AOpenWillowEnemy::TraceFloor(World, XY, SpotNow.Z + 300.f, SpotNow.Z - 900.f, Pawn, Floor)) return false;
        TeleportMaya(World, Floor, Yaw);
        return true;
    };
    // The enemy's facing, horizontal.
    auto Forward = [](const AOpenWillowEnemy* Enemy) { return FRotator(0.f, Enemy->GetActorRotation().Yaw, 0.f).Vector(); };
    auto YawToward = [](const FVector& From, const FVector& To) { return (To - From).Rotation().Yaw; };
    auto Reset = [this](const TCHAR* Why)
    {
        Den.ResetRecord();
        UE_LOG(LogTemp, Display, TEXT("OWRANGE threat reset (test, %s) level=1"), Why);
    };
    // Only Keep is active; every other enemy is passive (no notice, no melee).
    auto OnlyActive = [this](const AOpenWillowEnemy* Keep)
    {
        for (const auto& Weak : Enemies)
            if (AOpenWillowEnemy* Enemy = Weak.Get()) Enemy->SetPassive(Enemy != Keep);
    };
    // Maya 400 uu ahead of an enemy, inside its melee volume (the origin is 100 uu behind it, 500 uu to Maya).
    auto PlaceInVolume = [&](AOpenWillowEnemy* Enemy) -> bool
    {
        const FVector At = Enemy->GetActorLocation() + Forward(Enemy) * 400.f;
        return PlaceMaya(FVector2D(At.X, At.Y), YawToward(At, Enemy->GetActorLocation()));
    };

    switch (TestStep)
    {
    case 16: // notice_proximity: Maya 500 uu behind E0 (away from her facing). Proximity raises the level at once.
    {
        Reset(TEXT("proximity"));
        OnlyActive(E0);
        const FVector At = E0->GetActorLocation() - Forward(E0) * 500.f;
        if (!PlaceMaya(FVector2D(At.X, At.Y), YawToward(At, E0->GetActorLocation())))
        {
            TestSkip(TEXT("notice_proximity"), TEXT("no floor 500 uu behind the enemy"));
            TestStep = 18;
            return;
        }
        StepStart = Now;
        TestStep = 17;
        return;
    }
    case 17:
    {
        if (Now - StepStart < 0.5f) return;
        TestCheck(TEXT("notice_proximity"), Den.Level >= Seen, FString::Printf(TEXT("level %d, %.2f s after the teleport (500 uu behind)"),
            Den.Level, Now - StepStart));
        TestStep = 18;
        return;
    }
    case 18: // notice_sight: Maya 2000 uu in front of E0, in its line of sight.
    {
        Reset(TEXT("sight"));
        OnlyActive(E0);
        const FVector At = E0->GetActorLocation() + Forward(E0) * 2000.f;
        if (!PlaceMaya(FVector2D(At.X, At.Y), YawToward(At, E0->GetActorLocation())))
        {
            TestSkip(TEXT("notice_sight"), TEXT("no floor 2000 uu in front of the enemy"));
            TestStep = 20;
            return;
        }
        StepStart = Now;
        TestStep = 19;
        return;
    }
    case 19:
    {
        if (Now - StepStart < 0.5f) return;
        TestCheck(TEXT("notice_sight"), Den.Level >= Seen, FString::Printf(TEXT("level %d, %.2f s after the teleport (2000 uu in front)"),
            Den.Level, Now - StepStart));
        TestStep = 20;
        return;
    }
    case 20: // no_notice_far: Maya 6000 uu from the spot, on the far side of the enemies. No rule holds for 3 s.
    {
        Reset(TEXT("far"));
        OnlyActive(E0);
        const FVector At = SpotNow + FRotator(0.f, SpotYawNow, 0.f).Vector() * 6000.f;
        if (!PlaceMaya(FVector2D(At.X, At.Y), YawToward(At, E0->GetActorLocation())))
        {
            TestSkip(TEXT("no_notice_far"), TEXT("no floor 6000 uu from the spot"));
            TestStep = 22;
            return;
        }
        TestMaxLevel = Den.Level;
        StepStart = Now;
        TestStep = 21;
        return;
    }
    case 21:
    {
        TestMaxLevel = FMath::Max(TestMaxLevel, Den.Level);
        if (Now - StepStart < 3.f) return;
        TestCheck(TEXT("no_notice_far"), TestMaxLevel == OpenWillowThreat::InThreatRadius,
            FString::Printf(TEXT("highest level %d in 3 s, Maya %.0f uu from the spot"), TestMaxLevel,
                FVector::Dist2D(Walker->GetActorLocation(), SpotNow)));
        TestStep = 22;
        return;
    }
    case 22: // provoked: a hit on E0 by Maya (her pawn as the instigator, from 6000 uu away).
    {
        Reset(TEXT("provoked"));
        OnlyActive(E0);
        TestHit(E0, 1.f, E0->BodyComponent(), PC, Pawn);
        StepStart = Now;
        TestStep = 23;
        return;
    }
    case 23:
    {
        if (Now - StepStart < 0.5f) return;
        const float Span = Den.ForgetTime - Den.AggroTime;
        TestCheck(TEXT("provoked"), Den.Level == OpenWillowThreat::ProvokedLevel && Den.AggroTime >= StepStart
            && FMath::IsNearlyEqual(Span, OpenWillowThreat::MinProvokedTime, 0.01f),
            FString::Printf(TEXT("level %d, forget %.2f s after the stamp, E0 health %.1f"), Den.Level, Span, E0->GetHealth()));
        TestStep = 24;
        return;
    }
    case 24: // approach: Maya 2000 uu in front of E0 again; once aware, E0 closes the distance.
    {
        Reset(TEXT("approach"));
        OnlyActive(E0);
        const FVector At = E0->GetActorLocation() + Forward(E0) * 2000.f;
        if (!PlaceMaya(FVector2D(At.X, At.Y), YawToward(At, E0->GetActorLocation())))
        {
            TestSkip(TEXT("approach"), TEXT("no floor 2000 uu in front of the enemy"));
            TestStep = 26;
            return;
        }
        TestDistance = FVector::Dist2D(E0->GetActorLocation(), Walker->GetActorLocation());
        StepStart = Now;
        TestStep = 25;
        return;
    }
    case 25:
    {
        if (Now - StepStart < 2.f) return;
        const float Gap = FVector::Dist2D(E0->GetActorLocation(), Walker->GetActorLocation());
        TestCheck(TEXT("approach"), TestDistance - Gap >= 300.f && Den.Level >= Seen,
            FString::Printf(TEXT("closed %.0f uu in 2 s (%.0f to %.0f), level %d"), TestDistance - Gap, TestDistance, Gap, Den.Level));
        TestStep = 26;
        return;
    }
    case 26: // melee_hit: Maya in E0's volume; one hit 0.41 s after the swing start, of the table's amount.
    {
        Reset(TEXT("melee"));
        OnlyActive(E0);
        if (!PlaceInVolume(E0))
        {
            TestSkip(TEXT("melee_hit"), TEXT("no floor in front of the enemy"));
            TestStep = 28;
            return;
        }
        TestSwingBase = Brain.SwingCount;
        TestResolvedBase = Brain.MeleeResolved;
        TestHealthBefore = Walker->GetHealth();
        StepStart = Now;
        TestStep = 27;
        return;
    }
    case 27:
    {
        if (Walker->IsDown())
        {
            UE_LOG(LogTemp, Warning, TEXT("OWRANGE maya down during melee_hit (not restored)"));
            TestCheck(TEXT("melee_hit"), false, TEXT("Maya is down (not restored)"));
            TestStep = 28;
            return;
        }
        if (Brain.MeleeResolved <= TestResolvedBase)
        {
            if (Now - StepStart > 3.f)
            {
                TestCheck(TEXT("melee_hit"), false, TEXT("no swing resolved in 3 s"));
                TestStep = 28;
            }
            return;
        }
        const float Delta = TestHealthBefore - Walker->GetHealth();
        const float Expected = Brain.LastArrival * Brain.LastRatio * Brain.LastScale;
        const float Timing = Brain.LastAt - Brain.LastSwingStart;
        const bool bOk = Brain.MeleeResolved == TestResolvedBase + 1 && Brain.LastHit && Brain.LastApplied > 0.f
            && FMath::IsNearlyEqual(Brain.LastApplied, Expected, 0.01f) && FMath::Abs(Timing - OpenWillowThreat::SwingDamageDelay) <= 0.1f
            && FMath::IsNearlyEqual(Delta, Brain.LastApplied, 0.01f);
        TestCheck(TEXT("melee_hit"), bOk, FString::Printf(TEXT("hit %d at +%.3f s, applied %.2f (arrival %.2f x ratio %.4f x scale %.2f), "
            "maya health %.1f -> %.1f"), Brain.LastHit ? 1 : 0, Timing, Brain.LastApplied, Brain.LastArrival, Brain.LastRatio,
            Brain.LastScale, TestHealthBefore, Walker->GetHealth()));
        TestStep = 28;
        return;
    }
    case 28: // melee_interval setup: Maya stays in the volume.
    {
        Reset(TEXT("melee"));
        OnlyActive(E0);
        if (!PlaceInVolume(E0))
        {
            TestSkip(TEXT("melee_interval"), TEXT("no floor in front of the enemy"));
            TestStep = 30;
            return;
        }
        TestSwingBase = Brain.SwingCount;
        StepStart = Now;
        TestStep = 29;
        return;
    }
    case 29:
    {
        if (Walker->IsDown())
        {
            UE_LOG(LogTemp, Warning, TEXT("OWRANGE maya down during melee_interval (not restored)"));
            TestCheck(TEXT("melee_interval"), false, TEXT("Maya is down (not restored)"));
            TestStep = 30;
            return;
        }
        if (Brain.SwingCount < TestSwingBase + 2)
        {
            if (Now - StepStart > 6.f)
            {
                TestCheck(TEXT("melee_interval"), false, TEXT("fewer than two swing starts in 6 s"));
                TestStep = 30;
            }
            return;
        }
        const float Gap = Brain.LastSwingStart - Brain.PrevSwingStart;
        TestCheck(TEXT("melee_interval"), FMath::Abs(Gap - OpenWillowThreat::SwingInterval) <= 0.1f,
            FString::Printf(TEXT("%.3f s between the last two swing starts"), Gap));
        TestStep = 30;
        return;
    }
    case 30: // melee_miss setup: Maya in the volume; she steps out after the swing starts.
    {
        Reset(TEXT("melee"));
        OnlyActive(E0);
        if (!PlaceInVolume(E0))
        {
            TestSkip(TEXT("melee_miss"), TEXT("no floor in front of the enemy"));
            TestStep = 32;
            return;
        }
        TestSwingBase = Brain.SwingCount;
        TestResolvedBase = Brain.MeleeResolved;
        TestHealthBefore = Walker->GetHealth();
        TestMovedAt = -1.f;
        StepStart = Now;
        TestStep = 31;
        return;
    }
    case 31:
    {
        if (Walker->IsDown())
        {
            UE_LOG(LogTemp, Warning, TEXT("OWRANGE maya down during melee_miss (not restored)"));
            TestCheck(TEXT("melee_miss"), false, TEXT("Maya is down (not restored)"));
            TestStep = 32;
            return;
        }
        if (TestMovedAt < 0.f)
        {
            if (Brain.SwingCount <= TestSwingBase)
            {
                if (Now - StepStart > 3.f)
                {
                    TestCheck(TEXT("melee_miss"), false, TEXT("no swing start in 3 s"));
                    TestStep = 32;
                }
                return;
            }
            // Maya steps away at once: 3000 uu behind E0, out of its volume and out of its notice rules.
            const FVector At = E0->GetActorLocation() - Forward(E0) * 3000.f;
            PlaceMaya(FVector2D(At.X, At.Y), YawToward(At, E0->GetActorLocation()));
            TestMovedAt = Now;
            return;
        }
        if (Brain.MeleeResolved <= TestResolvedBase)
        {
            if (Now - TestMovedAt > 2.f)
            {
                TestCheck(TEXT("melee_miss"), false, TEXT("no swing resolved in 2 s"));
                TestStep = 32;
            }
            return;
        }
        const bool bOk = !Brain.LastHit && FMath::IsNearlyZero(Brain.LastApplied)
            && FMath::IsNearlyEqual(Walker->GetHealth(), TestHealthBefore, 0.01f)
            && TestMovedAt < Brain.LastSwingStart + OpenWillowThreat::SwingDamageDelay;
        TestCheck(TEXT("melee_miss"), bOk, FString::Printf(TEXT("hit %d, applied %.2f, moved %.3f s after the swing start, maya health %.1f -> %.1f"),
            Brain.LastHit ? 1 : 0, Brain.LastApplied, TestMovedAt - Brain.LastSwingStart, TestHealthBefore, Walker->GetHealth()));
        TestStep = 32;
        return;
    }
    case 32: // forget setup: Maya 500 uu behind E0 raises the level by proximity, with no swing (she is outside the volume).
    {
        Reset(TEXT("forget"));
        OnlyActive(E0);
        const FVector At = E0->GetActorLocation() - Forward(E0) * 500.f;
        if (!PlaceMaya(FVector2D(At.X, At.Y), YawToward(At, E0->GetActorLocation())))
        {
            TestSkip(TEXT("forget"), TEXT("no floor 500 uu behind the enemy"));
            TestStep = 35;
            return;
        }
        StepStart = Now;
        TestStep = 33;
        return;
    }
    case 33:
    {
        if (Now - StepStart < 0.5f) return;
        if (Den.Level < Seen)
        {
            TestCheck(TEXT("forget"), false, TEXT("not raised by the proximity rule"));
            TestStep = 35;
            return;
        }
        // Out of every rule now: far from the enemies. The record is left to run out.
        const FVector At = SpotNow + FRotator(0.f, SpotYawNow, 0.f).Vector() * 6000.f;
        if (!PlaceMaya(FVector2D(At.X, At.Y), YawToward(At, E0->GetActorLocation())))
        {
            TestSkip(TEXT("forget"), TEXT("no floor 6000 uu from the spot"));
            TestStep = 35;
            return;
        }
        TestStep = 34;
        return;
    }
    case 34:
    {
        // The den checks once a second, so the fall can come up to a second after ForgetTime; 0.2 s more for the enemy's update.
        if (Now < Den.ForgetTime + 1.2f) return;
        TestCheck(TEXT("forget"), Den.Level == OpenWillowThreat::InThreatRadius && !Brain.IsTargeting(),
            FString::Printf(TEXT("level %d, targeting %d, %.2f s after ForgetTime"), Den.Level, Brain.IsTargeting() ? 1 : 0,
                Now - Den.ForgetTime));
        TestStep = 35;
        return;
    }
    case 35: // passive_no_melee: every enemy passive, Maya in E0's volume. Nothing notices and nothing swings.
    {
        Reset(TEXT("passive"));
        SetMode(false, true);
        if (!PlaceInVolume(E0))
        {
            TestSkip(TEXT("passive_no_melee"), TEXT("no floor in front of the enemy"));
            SetMode(false, false);
            TestStep = 99;
            return;
        }
        TestSwingBase = Brain.SwingCount;
        TestHealthBefore = Walker->GetHealth();
        StepStart = Now;
        TestStep = 36;
        return;
    }
    case 36:
    {
        if (Now - StepStart < 2.f) return;
        const bool bOk = Brain.SwingCount == TestSwingBase && FMath::IsNearlyEqual(Walker->GetHealth(), TestHealthBefore, 0.01f)
            && Den.Level == OpenWillowThreat::InThreatRadius;
        TestCheck(TEXT("passive_no_melee"), bOk, FString::Printf(TEXT("swing starts %d, maya health %.1f -> %.1f, level %d"),
            Brain.SwingCount - TestSwingBase, TestHealthBefore, Walker->GetHealth(), Den.Level));
        SetMode(false, false);
        TestStep = 99;
        return;
    }
    default:
        TestStep = 99;
        return;
    }
}

// ---------------------------------------------------------------------------------------------- console commands

namespace
{
void RangeCommand(const TArray<FString>& Args, UWorld* World)
{
    UOpenWillowRangeSubsystem* Range = World ? World->GetSubsystem<UOpenWillowRangeSubsystem>() : nullptr;
    if (!Range) return;
    FVector Spot;
    float Yaw = 0.f;
    FString Source;
    if (Args.Num() >= 3)
    {
        Spot = FVector(FCString::Atof(*Args[0]), FCString::Atof(*Args[1]), FCString::Atof(*Args[2]));
        Yaw = Args.Num() >= 4 ? FCString::Atof(*Args[3]) : 0.f;
        Source = TEXT("args");
    }
    else if (!Range->ResolveSpot(Spot, Yaw, Source))
        return;
    if (!Range->BuildRange(Spot, Yaw)) return;
    UE_LOG(LogTemp, Display, TEXT("OWRANGE spot source=%s at=%s yaw=%.0f"), *Source, *Spot.ToString(), Yaw);
    TeleportMaya(World, Spot, Yaw);
}

void RangeModeCommand(const TArray<FString>& Args, UWorld* World)
{
    UOpenWillowRangeSubsystem* Range = World ? World->GetSubsystem<UOpenWillowRangeSubsystem>() : nullptr;
    if (!Range) return;
    const FString Mode = Args.Num() > 0 ? Args[0] : FString();
    if (Mode.Equals(TEXT("normal"), ESearchCase::IgnoreCase)) Range->SetMode(false, false);
    else if (Mode.Equals(TEXT("invulnerable"), ESearchCase::IgnoreCase)) Range->SetMode(true, false);
    else if (Mode.Equals(TEXT("passive"), ESearchCase::IgnoreCase)) Range->SetMode(false, true);
    else if (Mode.Equals(TEXT("both"), ESearchCase::IgnoreCase)) Range->SetMode(true, true);
    else UE_LOG(LogTemp, Warning, TEXT("OWRANGE usage: ow.RangeMode normal|invulnerable|passive|both"));
}

void RangeLevelCommand(const TArray<FString>& Args, UWorld* World)
{
    UOpenWillowRangeSubsystem* Range = World ? World->GetSubsystem<UOpenWillowRangeSubsystem>() : nullptr;
    if (!Range) return;
    if (Args.Num() < 1)
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE usage: ow.RangeLevel <N> (0 = the walker's level)"));
        return;
    }
    Range->SetLevelOverride(FCString::Atoi(*Args[0]));
    Range->RebuildLast();
}

void RangeClearCommand(const TArray<FString>& Args, UWorld* World)
{
    if (UOpenWillowRangeSubsystem* Range = World ? World->GetSubsystem<UOpenWillowRangeSubsystem>() : nullptr) Range->ClearRange();
}

void RangeReportCommand(const TArray<FString>& Args, UWorld* World)
{
    if (UOpenWillowRangeSubsystem* Range = World ? World->GetSubsystem<UOpenWillowRangeSubsystem>() : nullptr) Range->Report();
}

FAutoConsoleCommandWithWorldAndArgs GRangeCommand(TEXT("ow.Range"),
    TEXT("ow.Range [x y z [yaw]]: build the range at the spot (args, -owrangeat, local/range/range.json or the scout) and move Maya there"),
    FConsoleCommandWithWorldAndArgsDelegate::CreateStatic(&RangeCommand));
FAutoConsoleCommandWithWorldAndArgs GRangeModeCommand(TEXT("ow.RangeMode"),
    TEXT("ow.RangeMode normal|invulnerable|passive|both: range toggles"),
    FConsoleCommandWithWorldAndArgsDelegate::CreateStatic(&RangeModeCommand));
FAutoConsoleCommandWithWorldAndArgs GRangeLevelCommand(TEXT("ow.RangeLevel"),
    TEXT("ow.RangeLevel <N>: enemy level for the range (0 = the walker's level), rebuilds the range"),
    FConsoleCommandWithWorldAndArgsDelegate::CreateStatic(&RangeLevelCommand));
FAutoConsoleCommandWithWorldAndArgs GRangeClearCommand(TEXT("ow.RangeClear"),
    TEXT("ow.RangeClear: remove the range actors"),
    FConsoleCommandWithWorldAndArgsDelegate::CreateStatic(&RangeClearCommand));
FAutoConsoleCommandWithWorldAndArgs GRangeReportCommand(TEXT("ow.RangeReport"),
    TEXT("ow.RangeReport: log the state of every range actor"),
    FConsoleCommandWithWorldAndArgsDelegate::CreateStatic(&RangeReportCommand));
}
