#include "OpenWillowRange.h"
#include "OpenWillowMayaHUD.h"
#include "Engine/Engine.h"
#include "Engine/Texture.h"
#include "Materials/MaterialInstance.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowEnemy.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowSkills.h"
#include "OpenWillowWalker.h"
#include "OpenWillowWeaponGen.h"
#include "Camera/PlayerCameraManager.h"
#include "Components/CapsuleComponent.h"
#include "Components/PrimitiveComponent.h"
#include "Dom/JsonObject.h"
#include "Components/SkeletalMeshComponent.h"
#include "Engine/DamageEvents.h"
#include "Engine/World.h"
#include "GameFramework/Character.h"
#include "GameFramework/DamageType.h"
#include "GameFramework/PlayerController.h"
#include "HAL/FileManager.h"
#include "HAL/IConsoleManager.h"
#include "Kismet/GameplayStatics.h"
#include "Materials/MaterialInterface.h"
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
constexpr float PlaceReach = 3000.f;        // the range test's floor search for Maya's placements, above and below the spot
constexpr float LootNearCorpse = 400.f;     // the range test's limit for a drop's distance from the corpse (host choice)
// Floor rules of the scout and the formation (host choice, UNVERIFIED). Real terrain slopes: a walkable floor is one whose normal
// is within about 32 degrees of up (cos 32 = 0.85); the probes reach ProbeReach above and below, a probe may sit ProbeMaxRise
// above or below the centre, a formation slot SlotMaxRise (the enemies and dummies snap to the floor under them).
constexpr float WalkableNormalZ = 0.85f;
constexpr float ProbeReach = 1500.f;
constexpr float ProbeMaxRise = 250.f;
constexpr float SlotMaxRise = 400.f;

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

// A complex down trace for the floor. Its hit keeps the face index, which names the material (see FloorIsTextured).
bool TraceFloor(UWorld* World, const FVector& From, const FVector& To, FHitResult& Hit)
{
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowRangeFloor), true);
    Query.bTraceComplex = true;
    Query.bReturnFaceIndex = true;
    return World->LineTraceSingleByChannel(Hit, From, To, ECC_Visibility, Query);
}

// Is the floor a textured one? (host choice, UNVERIFIED) The hit's material is the one of the face that was hit (the first
// material when there is no face). Engine materials, and materials whose name holds Collision, Fallback or Neutral, do not count.
bool FloorIsTextured(const FHitResult& Hit, FString& OutPath)
{
    OutPath = TEXT("none");
    UPrimitiveComponent* Component = Hit.GetComponent();
    if (!Component) return false;
    UMaterialInterface* Material = nullptr;
    if (Hit.FaceIndex >= 0)
    {
        int32 SectionIndex = 0;
        Material = Component->GetMaterialFromCollisionFaceIndex(Hit.FaceIndex, SectionIndex);
    }
    else
    {
        Material = Component->GetMaterial(0);
    }
    if (!Material) return false;
    OutPath = Material->GetPathName();
    if (OutPath.StartsWith(TEXT("/Engine/"), ESearchCase::CaseSensitive)) return false;
    const FString Name = Material->GetName();
    for (const TCHAR* Placeholder : {TEXT("Collision"), TEXT("Fallback"), TEXT("Neutral")})
        if (Name.Contains(Placeholder, ESearchCase::IgnoreCase)) return false;
    // A name says little: the material must actually sample a texture of its own (one that is not an engine texture). An
    // imported surface without its diffuse (the dark canyon floor of batch 1) uses none, or only engine defaults.
    auto OwnTexture = [](const UTexture* Texture)
    {
        return Texture && !Texture->GetPathName().StartsWith(TEXT("/Engine/"), ESearchCase::CaseSensitive);
    };
    TArray<UTexture*> Textures;
    Material->GetUsedTextures(Textures);   // every quality level and shader platform (the default of the current API)
    for (const UTexture* Texture : Textures)
        if (OwnTexture(Texture)) return true;
    // Textures given as parameter values (a material instance, or the terrain blend whose layer textures are parameters; batch 4:
    // every terrain spot was M_TerrainBlend, which samples nothing of its own).
    TArray<FMaterialParameterInfo> Infos;
    TArray<FGuid> Ids;
    Material->GetAllTextureParameterInfo(Infos, Ids);
    for (const FMaterialParameterInfo& Info : Infos)
    {
        UTexture* Texture = nullptr;
        if (Material->GetTextureParameterValue(Info, Texture) && OwnTexture(Texture)) return true;
    }
    // The level import's terrain blend material (and its instances) is the textured terrain by construction (its weighted layers
    // are the cooked terrain layers: tools/prepare_terrain.py writes it with a terrain_blend block); counted as textured even when its layers are set
    // at run time (host choice, UNVERIFIED).
    for (const UMaterialInterface* Walk = Material; Walk; )
    {
        if (Walk->GetName().StartsWith(TEXT("M_TerrainBlend"))) return true;
        const UMaterialInstance* Instance = Cast<UMaterialInstance>(Walk);
        Walk = Instance ? Instance->Parent.Get() : nullptr;
    }
    OutPath += FString::Printf(TEXT(" (no texture of its own, %d used, %d texture parameters)"), Textures.Num(), Infos.Num());
    return false;
}

// The facing away from the town: the yaw from the plaza centre to Spot (only X and Y count for a yaw).
float AwayFromTown(const FVector& Spot, const FVector2D& Centre)
{
    return FVector(Spot.X - Centre.X, Spot.Y - Centre.Y, 0.f).Rotation().Yaw;
}

// The scout rules (host choice, UNVERIFIED): see UOpenWillowRangeSubsystem::ScoutSpot. The floor is the complex hit of a down
// trace around RefZ and must lie within MaxFloorDelta of it; OutFloorHit keeps that hit, OutWhy says which rule failed.
bool ScoutAccepts(UWorld* World, const FVector2D& XY, float RefZ, float MaxFloorDelta, FHitResult& OutFloorHit, FString& OutWhy)
{
    FHitResult Hit;
    // The floor: a down trace from 6000 uu above the reference height, flat enough and within MaxFloorDelta of it.
    if (!TraceFloor(World, FVector(XY.X, XY.Y, RefZ + 6000.f), FVector(XY.X, XY.Y, RefZ - 6000.f), Hit) || Hit.bStartPenetrating)
    {
        OutWhy = TEXT("no floor under the point");
        return false;
    }
    if (Hit.ImpactNormal.Z < WalkableNormalZ || FMath::Abs(Hit.Location.Z - RefZ) > MaxFloorDelta)
    {
        OutWhy = FString::Printf(TEXT("floor at z=%.0f is not flat or not within %.0f uu of %.0f"), Hit.Location.Z, MaxFloorDelta, RefZ);
        return false;
    }
    const float FloorZ = Hit.Location.Z;
    const FVector Floor = Hit.Location;
    OutFloorHit = Hit;
    // Eight probes on a 700 uu circle all find a walkable floor near the same height. Each probe starts ProbeReach above the
    // centre's floor: a probe that started lower would begin under a rising slope and never meet it (batch 4: "found no floor").
    for (int32 Probe = 0; Probe < 8; ++Probe)
    {
        const float Rad = FMath::DegreesToRadians(45.f * Probe);
        const FVector2D At = XY + FVector2D(FMath::Cos(Rad), FMath::Sin(Rad)) * 700.f;
        FHitResult Down;
        if (!TraceVisibility(World, FVector(At.X, At.Y, FloorZ + ProbeReach), FVector(At.X, At.Y, FloorZ - ProbeReach), Down)
            || Down.bStartPenetrating)
        {
            OutWhy = TEXT("a floor probe found no floor");
            return false;
        }
        if (FMath::Abs(Down.Location.Z - FloorZ) > ProbeMaxRise || Down.ImpactNormal.Z < WalkableNormalZ)
        {
            OutWhy = TEXT("the floor drops or slopes around the point");
            return false;
        }
    }
    // Nothing solid in a 250 uu sphere at floor + 300.
    FCollisionObjectQueryParams Objects;
    Objects.AddObjectTypesToQuery(ECC_WorldStatic);
    Objects.AddObjectTypesToQuery(ECC_WorldDynamic);
    if (World->OverlapAnyTestByObjectType(Floor + FVector(0.f, 0.f, 300.f), FQuat::Identity, Objects, FCollisionShape::MakeSphere(250.f)))
    {
        OutWhy = TEXT("a solid object stands on the floor");
        return false;
    }
    // Open sky: nothing between floor + 150 and floor + 1500.
    if (TraceVisibility(World, Floor + FVector(0.f, 0.f, 150.f), Floor + FVector(0.f, 0.f, 1500.f), Hit))
    {
        OutWhy = TEXT("something overhead");
        return false;
    }
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
    // The walker resets its fall tracking, so the teleport is never taken as a landing (PR #77, NotifyTeleported).
    if (AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(Pawn)) Walker->NotifyTeleported();
    if (APlayerController* PC = UGameplayStatics::GetPlayerController(World, 0)) PC->SetControlRotation(FRotator(0.f, Yaw, 0.f));
    UE_LOG(LogTemp, Display, TEXT("OWRANGE player moved to %s yaw %.0f"), *Spot.ToString(), Yaw);
}
}

namespace
{
// The floor of one formation slot (host choice, UNVERIFIED): a down trace near the spot's height that meets a walkable floor
// (normal Z >= 0.85) within 150 uu of the spot, and a textured one when bRequireTextured is set. False when there is none.
bool SlotFloor(UWorld* World, const FVector& Spot, const FVector& Centre, bool bRequireTextured, FVector& OutFloor)
{
    FHitResult Hit;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowRangeSlot), true);
    Query.bTraceComplex = true;
    Query.bReturnFaceIndex = true;
    if (!World->LineTraceSingleByChannel(Hit, FVector(Centre.X, Centre.Y, Spot.Z + ProbeReach), FVector(Centre.X, Centre.Y, Spot.Z - ProbeReach),
            ECC_Visibility, Query) || Hit.bStartPenetrating)
        return false;
    if (Hit.ImpactNormal.Z < WalkableNormalZ || FMath::Abs(Hit.Location.Z - Spot.Z) > SlotMaxRise) return false;
    FString Material;
    if (bRequireTextured && !FloorIsTextured(Hit, Material)) return false;
    OutFloor = Hit.Location;
    return true;
}
}

bool UOpenWillowRangeSubsystem::FormationSlots(const FVector& Spot, float Yaw, TArray<FVector>& OutDummies, TArray<FVector>& OutEnemies,
    FString& OutWhy, bool bRequireTexturedFloor) const
{
    // Every slot of the formation must stand on floor: the dummy row DummyAhead ahead and the enemy arc EnemyRing ahead, one
    // slot per enemy the manifest spawns. The scout and the candidates use this too, so a chosen spot always holds the whole range.
    UWorld* World = GetWorld();
    OutDummies.Reset();
    OutEnemies.Reset();
    if (!World) { OutWhy = TEXT("no world"); return false; }
    const TCHAR* Kind = bRequireTexturedFloor ? TEXT("textured walkable") : TEXT("walkable");
    const FVector Forward = FRotator(0.f, Yaw, 0.f).Vector();
    const FVector Side = FRotator(0.f, Yaw + 90.f, 0.f).Vector();
    for (int32 Index = 0; Index < ExpectedDummies; ++Index)
    {
        const FVector Centre = Spot + Forward * DummyAhead + Side * (float(Index) - 1.f) * DummySpacing;
        FVector Floor;
        if (!SlotFloor(World, Spot, Centre, bRequireTexturedFloor, Floor))
        {
            OutWhy = FString::Printf(TEXT("dummy slot %d at %s has no %s floor"), Index, *Centre.ToString(), Kind);
            return false;
        }
        OutDummies.Add(Floor);
    }
    const int32 Total = ExpectedEnemies();
    for (int32 Index = 0; Index < Total; ++Index)
    {
        const float Angle = Total > 1 ? FMath::Lerp(-EnemyArcDegrees, EnemyArcDegrees, float(Index) / float(Total - 1)) : 0.f;
        const FVector Centre = Spot + FRotator(0.f, Yaw + Angle, 0.f).Vector() * EnemyRing;
        FVector Floor;
        if (!SlotFloor(World, Spot, Centre, bRequireTexturedFloor, Floor))
        {
            OutWhy = FString::Printf(TEXT("enemy slot %d at %s has no %s floor"), Index, *Centre.ToString(), Kind);
            return false;
        }
        OutEnemies.Add(Floor);
    }
    return true;
}

bool UOpenWillowRangeSubsystem::TableLevels(int32& OutLow, int32& OutHigh)
{
    EnsureManifest();
    OutLow = MAX_int32;
    OutHigh = 0;
    for (const auto& Pair : Manifest.Kinds)
        for (const auto& Row : Pair.Value.HealthByLevel)
        {
            OutLow = FMath::Min(OutLow, Row.Key);
            OutHigh = FMath::Max(OutHigh, Row.Key);
        }
    return OutHigh > 0;
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
        const TArray<TSharedPtr<FJsonValue>>* CandidateList = nullptr;
        if (Root->TryGetArrayField(TEXT("spot_candidates"), CandidateList))
            for (const TSharedPtr<FJsonValue>& Value : *CandidateList)
            {
                const TSharedPtr<FJsonObject> Entry = Value.IsValid() ? Value->AsObject() : nullptr;
                double X = 0.0, Y = 0.0, Z = 0.0;
                if (!Entry.IsValid() || !Entry->TryGetNumberField(TEXT("x"), X) || !Entry->TryGetNumberField(TEXT("y"), Y)
                    || !Entry->TryGetNumberField(TEXT("z"), Z))
                    continue;
                FOpenWillowRangeCandidate Candidate;
                Candidate.Location = FVector(X, Y, Z);
                Entry->TryGetStringField(TEXT("source"), Candidate.Source);
                Entry->TryGetBoolField(TEXT("gate_terrain"), Candidate.bGateTerrain);
                Out.Candidates.Add(Candidate);
            }
        const TSharedPtr<FJsonObject>* CentreObject = nullptr;
        if (Root->TryGetObjectField(TEXT("scout_centre"), CentreObject))
        {
            double CentreX = 0.0, CentreY = 0.0;
            if ((*CentreObject)->TryGetNumberField(TEXT("x"), CentreX) && (*CentreObject)->TryGetNumberField(TEXT("y"), CentreY))
            {
                Out.ScoutCentre = FVector2D(CentreX, CentreY);
                Out.bHasScoutCentre = true;
            }
        }
        double DefaultCandidate = -1.0;
        if (Root->TryGetNumberField(TEXT("default_spot_candidate"), DefaultCandidate)) Out.DefaultSpotCandidate = int32(DefaultCandidate);
        const TSharedPtr<FJsonObject>* KindObjects = nullptr;
        if (Root->TryGetObjectField(TEXT("kinds"), KindObjects))
            for (const auto& Pair : (*KindObjects)->Values)
            {
                const TSharedPtr<FJsonObject> Obj = Pair.Value.IsValid() ? Pair.Value->AsObject() : nullptr;
                if (!Obj.IsValid()) continue;
                FOpenWillowRangeKind Kind;
                Kind.Name = Pair.Key;
                Kind.Json = Obj;
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
                Read(TEXT("melee_start_range"), Kind.MeleeStartRange);
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
                // PR3: the loot of each level (the VM's pool chances; the host rolls them). A level with a bad shape is skipped.
                const TSharedPtr<FJsonObject>* LootObject = nullptr;
                if (Obj->TryGetObjectField(TEXT("loot_by_level"), LootObject))
                    for (const auto& LevelPair : (*LootObject)->Values)
                    {
                        const TSharedPtr<FJsonObject> LevelObj = LevelPair.Value.IsValid() ? LevelPair.Value->AsObject() : nullptr;
                        if (!LevelObj.IsValid()) continue;
                        FOpenWillowRangeLoot Loot;
                        double ItemStage = 0.0;
                        if (LevelObj->TryGetNumberField(TEXT("item_stage"), ItemStage)) Loot.ItemStage = int32(ItemStage);
                        // The stage range (prepare_range.py: min and max over several kills); one VM draw when absent.
                        double StageMin = ItemStage, StageMax = ItemStage;
                        LevelObj->TryGetNumberField(TEXT("item_stage_min"), StageMin);
                        LevelObj->TryGetNumberField(TEXT("item_stage_max"), StageMax);
                        Loot.ItemStageMin = FMath::Max(1, int32(StageMin));
                        Loot.ItemStageMax = FMath::Max(Loot.ItemStageMin, int32(StageMax));
                        const TArray<TSharedPtr<FJsonValue>>* PoolList = nullptr;
                        if (LevelObj->TryGetArrayField(TEXT("pools"), PoolList))
                            for (const TSharedPtr<FJsonValue>& Value : *PoolList)
                            {
                                const TSharedPtr<FJsonObject> PoolObj = Value.IsValid() ? Value->AsObject() : nullptr;
                                if (!PoolObj.IsValid()) continue;
                                FOpenWillowRangeLootPool Pool;
                                double Chance = 0.0;
                                PoolObj->TryGetStringField(TEXT("pool"), Pool.Pool);
                                if (PoolObj->TryGetNumberField(TEXT("chance"), Chance)) Pool.Chance = float(Chance);
                                PoolObj->TryGetStringField(TEXT("kind"), Pool.Kind);
                                Loot.Pools.Add(Pool);
                            }
                        Kind.LootByLevel.Add(FCString::Atoi(*LevelPair.Key), Loot);
                    }
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
    bSurveyMode = FParse::Param(FCommandLine::Get(), TEXT("owrangesurvey"));
    bStartSwitch = FParse::Param(FCommandLine::Get(), TEXT("owrangestart"));
    bKillRow = bStartSwitch && FParse::Param(FCommandLine::Get(), TEXT("owrangekillrow"));
    FString ModeSwitch;
    if (FParse::Value(FCommandLine::Get(), TEXT("owrangemode="), ModeSwitch))
    {
        // The toggles ow.RangeMode sets, from the start (read by the build).
        bModeInvulnerable = ModeSwitch.Equals(TEXT("invulnerable"), ESearchCase::IgnoreCase) || ModeSwitch.Equals(TEXT("both"), ESearchCase::IgnoreCase);
        bModePassive = ModeSwitch.Equals(TEXT("passive"), ESearchCase::IgnoreCase) || ModeSwitch.Equals(TEXT("both"), ESearchCase::IgnoreCase);
    }
    if (bStartSwitch && (bTestMode || bSurveyMode))
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE -owrangestart is ignored with -owrangetest or -owrangesurvey: they build their own range"));
        bStartSwitch = false;
    }
    FString LootSwitch;
    if (FParse::Value(FCommandLine::Get(), TEXT("owrangeloot="), LootSwitch))
    {
        if (LootSwitch.Equals(TEXT("always"), ESearchCase::IgnoreCase)) LootMode = EOpenWillowRangeLootMode::Always;
        else if (LootSwitch.Equals(TEXT("off"), ESearchCase::IgnoreCase)) LootMode = EOpenWillowRangeLootMode::Off;
        else LootMode = EOpenWillowRangeLootMode::Normal;
        UE_LOG(LogTemp, Display, TEXT("OWRANGE loot mode from the command line: %s"), *LootSwitch);
    }
    if (bTestMode && bSurveyMode)
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE -owrangetest and -owrangesurvey are exclusive: the survey is off"));
        bSurveyMode = false;
    }
    FString CountText;
    if (FParse::Value(FCommandLine::Get(), TEXT("owrangesurveycount="), CountText)) SurveyCount = FMath::Max(0, FCString::Atoi(*CountText));
    // PR3: the death roll's stream (-owrangeseed=N; the default makes a run repeat).
    FString SeedText;
    if (FParse::Value(FCommandLine::Get(), TEXT("owrangeseed="), SeedText)) LootSeed = FCString::Atoi(*SeedText);
    ResetLootStream();
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
    // A run that asked for the movie HUD holds the test, the survey and the kill row until it is ready. If it fails, quit with no more frames.
    const bool bDriver = (bTestMode && !bTestDone) || (bSurveyMode && !bSurveyDone) || (bKillRow && !bKillRowDone);
    const EOpenWillowCaptureHud MovieHud = bDriver ? AOpenWillowMayaHUD::CaptureHudGate(GetWorld(), TEXT("range")) : EOpenWillowCaptureHud::Go;
    if (MovieHud == EOpenWillowCaptureHud::Failed)
    {
        bTestDone = bSurveyDone = bKillRowDone = true;
        if (APlayerController* PC = GetWorld()->GetFirstPlayerController()) PC->ConsoleCommand(TEXT("quit"));
        return;
    }
    const bool bGo = MovieHud == EOpenWillowCaptureHud::Go;
    if (bGo && bTestMode && !bTestDone) TickTest(Now);
    if (bGo && bSurveyMode && !bSurveyDone) TickSurvey(Now);
    if (bStartSwitch && !bStartSwitchDone) TickStartSwitch(Now);
    if (bGo && bKillRow && !bKillRowDone && bStartSwitchDone) TickKillRow(Now);
}

void UOpenWillowRangeSubsystem::TickStartSwitch(float Now)
{
    // As the test driver: wait for Maya, then give the level's streaming the same few seconds before the floor traces of the spot.
    if (!UGameplayStatics::GetPlayerPawn(this, 0)) return;
    if (StartSwitchArmedAt < 0.f) { StartSwitchArmedAt = Now; return; }
    if (Now - StartSwitchArmedAt < 4.f) return;
    bStartSwitchDone = true;
    const bool bBuilt_ = BuildAtResolvedSpot();
    UE_LOG(LogTemp, Display, TEXT("OWRANGE start switch: built=%d"), bBuilt_ ? 1 : 0);
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

void UOpenWillowRangeSubsystem::RefuseBuild(const FString& Why)
{
    BuildRefusal = Why;
    UE_LOG(LogTemp, Warning, TEXT("OWRANGE build refused: %s"), *Why);
    if (GEngine) GEngine->AddOnScreenDebugMessage(-1, 30.f, FColor::Red, FString::Printf(TEXT("Range not built: %s"), *Why));
}

FString UOpenWillowRangeSubsystem::ManifestProblem() const
{
    if (!Manifest.bFromFile) return TEXT("no range manifest: run tools/prepare_range.py");
    if (Manifest.Spawns.Num() == 0) return TEXT("the range manifest spawns nothing: re-run tools/prepare_range.py");
    for (const FOpenWillowRangeSpawn& Spawn : Manifest.Spawns)
    {
        const FOpenWillowRangeKind* Kind = Manifest.Kinds.Find(Spawn.Kind);
        if (!Kind) return FString::Printf(TEXT("the range manifest spawns %s, which it does not define: re-run tools/prepare_range.py"), *Spawn.Kind);
        if (Kind->MeleeArrivalByLevel.Num() == 0 || Kind->MeleeRatioByLevel.Num() == 0 || Kind->MeleeStartRange <= 0.f)
            return FString::Printf(TEXT("kind %s has no melee tables or swing range (an old or --no-vm manifest): its enemies could not fight as the range expects. Re-run tools/prepare_range.py"), *Spawn.Kind);
    }
    return FString();
}

bool UOpenWillowRangeSubsystem::BuildRange(const FVector& Spot, float Yaw)
{
    // Whichever way it was started (ow.Range, -owrangestart, the test): nothing is built on a manifest that cannot fight, and the person who
    // asked sees why, not only the log.
    EnsureManifest();
    BuildRefusal.Reset();
    const FString Problem = ManifestProblem();
    if (!Problem.IsEmpty())
    {
        RefuseBuild(Problem);
        return false;
    }
    UWorld* World = GetWorld();
    if (!World) return false;
    ClearRange();
    EnsureManifest();
    // The whole formation must stand on walkable floor before anything is spawned (an incomplete range is a failed build).
    TArray<FVector> DummySlots, EnemySlots;
    FString Why;
    if (!FormationSlots(Spot, Yaw, DummySlots, EnemySlots, Why))
    {
        RefuseBuild(FString::Printf(TEXT("the formation does not fit at %s yaw %.0f: %s"), *Spot.ToString(), Yaw, *Why));
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

    // Moving enemies: the manifest's spawns on the arc slots, in manifest order, each facing away from the spot.
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
            // Host layout, not a game rule: the den stands facing away from the spot, so arriving does not trip the sight rule on the
            // first update. Its patrol, or a shot, or coming close, starts the fight by the same rules as anywhere.
            AOpenWillowEnemy* Enemy = World->SpawnActor<AOpenWillowEnemy>(At, FRotator(0.f, (At - Spot).Rotation().Yaw, 0.f), Params);
            if (!Enemy) continue;
            Enemy->SetupKind(*Kind, Level, Manifest.RespawnSeconds);
            Enemy->SetInvulnerable(bModeInvulnerable);
            Enemy->SetPassive(bModePassive);
            Enemy->SetAIEnabled(bModeAI);
            Enemy->SetDen(&Den);
            Enemy->OnEnemyDied.AddUObject(this, &UOpenWillowRangeSubsystem::HandleEnemyDied);
            Enemies.Add(Enemy);
            EnemyKinds.Add(Enemy, Spawn.Kind);   // PR3: the death roll looks the enemy's loot up by its kind
        }
    }
    // Every slot must hold its actor; an incomplete range is a failed build (nothing is left standing).
    if (Dummies.Num() != DummySlots.Num() || Enemies.Num() != EnemySlots.Num())
    {
        RefuseBuild(FString::Printf(TEXT("spawned %d of %d dummies and %d of %d enemies"), Dummies.Num(),
            DummySlots.Num(), Enemies.Num(), EnemySlots.Num()));
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
    // The loot of the range (gun drops and the range's own pickups) goes with it.
    for (const auto& Weak : LootActors)
        if (AActor* Loot = Weak.Get())
        {
            Loot->Destroy();
            ++Count;
        }
    LootActors.Reset();
    Dummies.Reset();
    Enemies.Reset();
    EnemyKinds.Reset();
    bBuilt = false;
    if (Count > 0) UE_LOG(LogTemp, Display, TEXT("OWRANGE cleared %d range actor(s)"), Count);
}

void UOpenWillowRangeSubsystem::RebuildLast()
{
    if (bBuilt) BuildRange(SpotNow, SpotYawNow);
}

void UOpenWillowRangeSubsystem::SetAI(bool bOn)
{
    bModeAI = bOn;
    for (const auto& Weak : Enemies)
        if (AOpenWillowEnemy* Enemy = Weak.Get()) Enemy->SetAIEnabled(bModeAI);
    if (!bModeAI) Den.ResetRecord();
    UE_LOG(LogTemp, Display, TEXT("OWRANGE ai=%d (%d enemies)"), bModeAI ? 1 : 0, Enemies.Num());
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

bool UOpenWillowRangeSubsystem::BuildAtResolvedSpot()
{
    FVector Spot;
    float Yaw = 0.f;
    FString Source;
    if (bTestNoSpot || !ResolveSpot(Spot, Yaw, Source))
    {
        RefuseBuild(TEXT("no spot for the range: no candidate or scouted place fits it (the log says why for each)"));
        return false;
    }
    if (!BuildRange(Spot, Yaw)) return false;
    UE_LOG(LogTemp, Display, TEXT("OWRANGE spot source=%s at=%s yaw=%.0f"), *Source, *Spot.ToString(), Yaw);
    TeleportMaya(GetWorld(), Spot, Yaw);
    return true;
}

bool UOpenWillowRangeSubsystem::ResolveSpot(FVector& OutSpot, float& OutYaw, FString& OutSource)
{
    // 1. -owrangeat=X,Y,Z[,Yaw]
    FString At;
    if (FParse::Value(FCommandLine::Get(), TEXT("owrangeat="), At, /*bShouldStopOnSeparator=*/false))
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
    // 3. The default spot: the candidate the manifest names by index (the hub's pick from the survey frames; local data only). The
    // candidate rules turn it into the point and yaw. The other candidates follow when it no longer fits.
    EnsureManifest();
    if (Manifest.Candidates.IsValidIndex(Manifest.DefaultSpotCandidate))
    {
        FString Material, Why;
        if (ResolveCandidate(Manifest.DefaultSpotCandidate, OutSpot, OutYaw, Material, Why))
        {
            UE_LOG(LogTemp, Display, TEXT("OWRANGE spot source=default_candidate index=%d terrain=%s at=%.0f,%.0f,%.0f yaw=%.0f material=%s"),
                Manifest.DefaultSpotCandidate, *Manifest.Candidates[Manifest.DefaultSpotCandidate].Source, OutSpot.X, OutSpot.Y, OutSpot.Z, OutYaw,
                *Material);
            OutSource = TEXT("default_candidate");
            return true;
        }
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE default candidate index=%d not accepted: %s"), Manifest.DefaultSpotCandidate, *Why);
    }
    // 4. The manifest's candidates, in manifest order (gate terrain first).
    for (int32 Index = 0; Index < Manifest.Candidates.Num(); ++Index)
    {
        FString Material, Why;
        if (ResolveCandidate(Index, OutSpot, OutYaw, Material, Why))
        {
            UE_LOG(LogTemp, Display, TEXT("OWRANGE spot source=candidate index=%d terrain=%s at=%.0f,%.0f,%.0f yaw=%.0f material=%s"), Index,
                *Manifest.Candidates[Index].Source, OutSpot.X, OutSpot.Y, OutSpot.Z, OutYaw, *Material);
            OutSource = TEXT("candidate");
            return true;
        }
        UE_LOG(LogTemp, Display, TEXT("OWRANGE candidate index=%d not accepted: %s"), Index, *Why);
    }
    // 5. The scout.
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
    if (!Manifest.bHasScoutCentre)
    {
        OutLog = TEXT("the manifest has no scout centre (python tools/prepare_range.py)");
        return false;
    }
    const FVector2D ScoutCentre = Manifest.ScoutCentre;
    UWorld* World = GetWorld();
    FHitResult Plaza;
    if (!World || !TraceVisibility(World, FVector(ScoutCentre.X, ScoutCentre.Y, 10000.f),
            FVector(ScoutCentre.X, ScoutCentre.Y, -10000.f), Plaza))
    {
        OutLog = TEXT("no plaza floor under the scout centre");
        return false;
    }
    const float PlazaZ = Plaza.Location.Z;
    // Rings closest first; the first accepted candidate of the first ring that has one wins (all of its ring is counted).
    for (int32 Ring = 0; Ring < ScoutRings; ++Ring)
    {
        const float Radius = 6000.f + 1000.f * float(Ring);
        int32 Accepted = 0;
        bool bWon = false;
        FString WonMaterial;
        for (int32 Index = 0; Index < ScoutAngles; ++Index)
        {
            const float Rad = FMath::DegreesToRadians(360.f * float(Index) / float(ScoutAngles));
            const FVector2D XY = ScoutCentre + FVector2D(FMath::Cos(Rad), FMath::Sin(Rad)) * Radius;
            FHitResult FloorHit;
            FString Material, Why;
            if (!ScoutAccepts(World, XY, PlazaZ, 1500.f, FloorHit, Why) || !FloorIsTextured(FloorHit, Material)) continue;
            const FVector Floor = FloorHit.Location;
            // The whole formation (dummy row and enemy arc, facing away from the town) must have textured floor too.
            TArray<FVector> DummySlots, EnemySlots;
            if (!FormationSlots(Floor, AwayFromTown(Floor, Manifest.ScoutCentre), DummySlots, EnemySlots, Why, true)) continue;
            ++Accepted;
            if (!bWon)
            {
                bWon = true;
                OutSpot = Floor;
                OutYaw = AwayFromTown(Floor, Manifest.ScoutCentre);   // facing away from the town
                WonMaterial = Material;
            }
        }
        UE_LOG(LogTemp, Display, TEXT("OWRANGE scout ring=%.0f accepted=%d of %d"), Radius, Accepted, ScoutAngles);
        if (bWon)
        {
            UE_LOG(LogTemp, Display, TEXT("OWRANGE spot source=scout at=%.0f,%.0f,%.0f yaw=%.0f ring=%.0f material=%s"), OutSpot.X, OutSpot.Y,
                OutSpot.Z, OutYaw, Radius, *WonMaterial);
            OutLog = TEXT("scout");
            return true;
        }
    }
    OutLog = TEXT("no accepted candidate on rings 6000-14000 uu");
    return false;
}

bool UOpenWillowRangeSubsystem::ResolveCandidate(int32 Index, FVector& OutSpot, float& OutYaw, FString& OutMaterial, FString& OutWhy)
{
    // One candidate (PR2 follow-up, host choice, UNVERIFIED): the candidate point, then eight points on a 600 uu ring and eight on
    // a 1200 uu ring, in that order. The first point that passes every rule wins. The floor must lie within 300 uu of the
    // candidate's own height (the gate terrain sits well below the plaza), be textured, and the whole formation must stand on
    // textured floor, facing away from the town.
    EnsureManifest();
    UWorld* World = GetWorld();
    if (!World || !Manifest.Candidates.IsValidIndex(Index)) { OutWhy = TEXT("no such candidate"); return false; }
    const FVector Candidate = Manifest.Candidates[Index].Location;
    TArray<FVector2D> Points;
    Points.Add(FVector2D(Candidate.X, Candidate.Y));
    for (const float Radius : {600.f, 1200.f, 2400.f, 3600.f})
        for (int32 Angle = 0; Angle < 8; ++Angle)
        {
            const float Rad = FMath::DegreesToRadians(45.f * float(Angle));
            Points.Add(FVector2D(Candidate.X, Candidate.Y) + FVector2D(FMath::Cos(Rad), FMath::Sin(Rad)) * Radius);
        }
    FString FirstWhy;
    for (const FVector2D& XY : Points)
    {
        FHitResult FloorHit;
        FString Material, Why;   // Why is set by whichever rule fails
        if (ScoutAccepts(World, XY, Candidate.Z, 300.f, FloorHit, Why))
        {
            if (!FloorIsTextured(FloorHit, Material))
            {
                Why = FString::Printf(TEXT("floor is not textured (material %s)"), *Material);
            }
            else
            {
                TArray<FVector> DummySlots, EnemySlots;
                const FVector Floor = FloorHit.Location;
                if (FormationSlots(Floor, AwayFromTown(Floor, Manifest.ScoutCentre), DummySlots, EnemySlots, Why, true))
                {
                    OutSpot = Floor;
                    OutYaw = AwayFromTown(Floor, Manifest.ScoutCentre);
                    OutMaterial = Material;
                    return true;
                }
            }
        }
        if (FirstWhy.IsEmpty()) FirstWhy = Why;
    }
    OutWhy = FirstWhy;
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
    // PR3: the death roll, with the enemy's kind (as built) and its level now. Nothing is rolled for an unknown kind.
    const FString* KindName = EnemyKinds.Find(Enemy);
    const FOpenWillowRangeKind* Kind = KindName ? Manifest.Kinds.Find(*KindName) : nullptr;
    if (!Kind)
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE loot no_kind: the enemy was not built by the range, nothing rolled"));
        LastLoot = FOpenWillowRangeLootResult();
        return;
    }
    LastLoot = RollRangeLoot(GetWorld(), Enemy, *KindName, *Kind, Enemy->GetLevel(), LootStream, LootMode);
    // Every drop is the range's: ow.RangeClear removes it (gone ones are pruned here).
    LootActors.RemoveAll([](const TWeakObjectPtr<AActor>& Weak) { return !Weak.IsValid(); });
    LootActors.Append(LastLoot.Spawned);
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

// A frame saved as local/range/shots/<FileName> (the test and the survey use it; -owrangeshots gates both).
namespace
{
void RequestRangeShot(const FString& FileName)
{
    const FString Dir = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/range/shots")));
    IFileManager::Get().MakeDirectory(*Dir, true);
    AOpenWillowMayaHUD::CaptureShot(TEXT("range"), FPaths::Combine(Dir, FileName));
}
}

void UOpenWillowRangeSubsystem::TestShot(int32 Number)
{
    if (!FParse::Param(FCommandLine::Get(), TEXT("owrangeshots"))) return;
    RequestRangeShot(FString::Printf(TEXT("range_%d.png"), Number));
}

void UOpenWillowRangeSubsystem::FinishKill(bool bOk, const FString& Why)
{
    if (!bOk) ++KillFailed;
    UE_LOG(LogTemp, Display, TEXT("OWRANGEKILL gun=%d result=%s %s"), KillGun, bOk ? TEXT("PASS") : TEXT("FAIL"), *Why);
    ++KillGun;
    KillStage = 0;
}

static constexpr int32 KillGunStage = 8;

// The kill row: what the first playtest saw flung, from the same kind of shots. Passive enemies, so nothing else moves them. Each gun kills one enemy from
// about 700 uu; frames at the times the playtest's stills were taken; the corpse's own numbers after its respawn. Sanctuary only, host behaviour only.
void UOpenWillowRangeSubsystem::TickKillRow(float Now)
{
    static const TCHAR* const Pools[] = {TEXT("GD_Itempools.WeaponPools.Pool_Weapons_Pistols"), TEXT("GD_Itempools.WeaponPools.Pool_Weapons_Shotguns")};
    UWorld* World = GetWorld();
    AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(UGameplayStatics::GetPlayerPawn(World, 0));
    APlayerController* PC = UGameplayStatics::GetPlayerController(World, 0);
    UOpenWillowInventory* Inventory = Walker ? Walker->GetInventory() : nullptr;
    if (KillGun >= UE_ARRAY_COUNT(Pools) || !bBuilt || !PC || !Inventory)
    {
        if (KillGun < UE_ARRAY_COUNT(Pools)) ++KillFailed;
        UE_LOG(LogTemp, Display, TEXT("OWRANGEKILL done guns=%d fail=%d"), KillGun, KillFailed);
        bKillRowDone = true;
        if (PC) PC->ConsoleCommand(TEXT("quit"));
        return;
    }
    AOpenWillowEnemy* Enemy = KillTarget.Get();
    switch (KillStage)
    {
    case 0: // the next enemy, Maya in front of it, the gun rolled, equipped and drawn
    {
        Enemy = Enemies.IsValidIndex(KillGun) ? Enemies[KillGun].Get() : nullptr;
        if (!Enemy || !Enemy->IsStockPawn() || Enemy->IsDead()) { FinishKill(false, TEXT("no living stock enemy for this gun")); return; }
        KillTarget = Enemy;
        const FVector At = Enemy->GetActorLocation() + (SpotNow - Enemy->GetActorLocation()).GetSafeNormal2D() * 700.f;
        FVector Floor;
        if (!AOpenWillowEnemy::TraceFloor(World, FVector2D(At.X, At.Y), At.Z + 500.f, At.Z - 500.f, Walker, Floor)) { FinishKill(false, TEXT("no floor in front of the enemy")); return; }
        TeleportMaya(World, Floor, (Enemy->GetActorLocation() - Floor).Rotation().Yaw);
        // Test fixture: room for the gun and for what the equip swaps out. Maya's stand-in gear can fill the backpack (Z16), and nothing here is saved.
        const FOpenWillowWeaponItem* Active = Inventory->ActiveWeapon();
        const FString ActiveId = Active ? UOpenWillowInventory::StableId(*Active) : FString();
        TArray<FString> Spare;
        for (const FOpenWillowWeaponItem& Item : Inventory->Items())
            if (UOpenWillowInventory::StableId(Item) != ActiveId) Spare.Add(UOpenWillowInventory::StableId(Item));
        int32 Freed = 0;
        for (int32 Index = Spare.Num() - 1; Index >= 0 && Freed < 2; --Index)
        {
            FOpenWillowTakenInventoryItem Taken;
            if (Walker->TakeInventoryItemById(Spare[Index], Taken)) ++Freed;
        }
        UE_LOG(LogTemp, Display, TEXT("OWRANGEKILL gun=%d fixture: %d item(s) taken from the backpack, %d left"), KillGun, Freed, Inventory->BackpackCount());
        TSet<FString> Before;
        for (const FOpenWillowWeaponItem& Item : Inventory->Items()) Before.Add(UOpenWillowInventory::StableId(Item));
        // The playtest's stage: at stage 1 these pools give nothing, and the shotgun pool still nothing at 2 (the core generator agrees).
        const int32 Stage = KillGunStage;
        GEngine->Exec(World, *FString::Printf(TEXT("ow.RollGun %s %d %d"), Pools[KillGun], Stage, 101 + KillGun));
        FString Id;
        for (const FOpenWillowWeaponItem& Item : Inventory->Items())
            if (!Before.Contains(UOpenWillowInventory::StableId(Item))) { Id = UOpenWillowInventory::StableId(Item); break; }
        const int32 Index = Id.IsEmpty() ? INDEX_NONE : Inventory->FindItemIndexById(Id);
        if (Index == INDEX_NONE) { FinishKill(false, FString::Printf(TEXT("%s: no new gun in the backpack (%d items): the ow.RollGun line above says why"), Pools[KillGun], Inventory->BackpackCount())); return; }
        if (!Walker->EquipItem(Index, 0) || !Walker->DrawItemById(Id)) { FinishKill(false, FString::Printf(TEXT("could not equip and draw %s"), *Id)); return; }
        const FOpenWillowWeaponItem* Held = Inventory->ActiveWeapon();
        KillGunName = Held ? FString::Printf(TEXT("\"%s\" (%s, level %d)"), *Held->Name, *Held->Type, Held->Level) : Id;
        KillShots = 0;
        KillAt = Now;
        KillStage = 1;
        return;
    }
    case 1: // real shots until it dies
    {
        if (!Enemy) { FinishKill(false, TEXT("the enemy is gone")); return; }
        if (Now - KillAt < 1.f) return;   // the gun is drawn
        if (Enemy->IsDead())
        {
            // Only this gun's shots count: a death with none fired is not its kill.
            if (KillShots == 0) { FinishKill(false, FString::Printf(TEXT("%s: the enemy died with no shot fired"), *KillGunName)); return; }
            UE_LOG(LogTemp, Display, TEXT("OWRANGEKILL gun=%d %s killed %s with %d shot(s), %.0f uu away"), KillGun, *KillGunName, *Enemy->GetName(), KillShots,
                FVector::Dist(Walker->GetActorLocation(), Enemy->GetActorLocation()));
            KillAt = Now;
            KillStage = 2;
            return;
        }
        if (Now - KillAt > 40.f) { FinishKill(false, FString::Printf(TEXT("%s: not dead after %d shots"), *KillGunName, KillShots)); return; }
        const APlayerCameraManager* Camera = UGameplayStatics::GetPlayerCameraManager(World, 0);
        const FVector Eye = Camera ? Camera->GetCameraLocation() : Walker->GetActorLocation();
        PC->SetControlRotation((Enemy->AimPoint() - Eye).Rotation());
        if (Walker->FireOnce()) { ++KillShots; return; }
        FOpenWillowWeaponItem* Held = Inventory->ActiveWeaponMutable();
        if (!Held || Walker->IsReloading() || UOpenWillowInventory::MagazineLeft(*Held) > 0) return;
        EOpenWillowAmmoType Ammo;
        if (!Inventory->CanReload(*Held) && UOpenWillowInventory::ResolveAmmoType(*Held, Ammo)) Inventory->SetAmmoCurrent(Ammo, Inventory->AmmoPool(Ammo).Max);
#if WITH_EDITOR
        Walker->TestReloadInput();
#endif
        return;
    }
    case 2: // frames at the playtest's times after the death: 0.56 s and 2.17 s into the ragdoll
        if (Now - KillAt < 1.4f) return;
        RequestRangeShot(FString::Printf(TEXT("kill_%d_a.png"), KillGun));
        KillStage = 3;
        return;
    case 3:
        if (Now - KillAt < 3.f) return;
        RequestRangeShot(FString::Printf(TEXT("kill_%d_b.png"), KillGun));
        KillStage = 4;
        return;
    default: // after its respawn: what the corpse did
    {
        if (!Enemy) { FinishKill(false, TEXT("the enemy is gone")); return; }
        if (Enemy->IsDead()) return;
        FinishKill(KillShots > 0 && Enemy->CorpseMeasured(), FString::Printf(TEXT("%s: %d shots, rose %.0f uu, drifted %.0f uu, fastest body %.0f uu/s, %d speed caps, measured %d"), *KillGunName, KillShots,
            Enemy->CorpsePeakRise(), Enemy->CorpsePeakDrift(), Enemy->CorpsePeakSpeed(), Enemy->CorpseSpeedCaps(), Enemy->CorpseMeasured() ? 1 : 0));
        return;
    }
    }
}

void UOpenWillowRangeSubsystem::TickTest(float Now)
{
    // Steps: 0 wait for Maya, build and move her; 1 teleport; 2 placement; 3 body hit; 4 crit hit; 5 crit trace; 6 shield order;
    // 7 invulnerable; 8 passive; 9 passive held; 10 patrol; 11 death; 12 ragdoll; 13 respawn; 14-15 dummy respawn;
    // 16-36 enemy AI (TickAITest); 37-44 loot on death (TickLootTest, PR3); 99 summary.
    UWorld* World = GetWorld();
    APawn* Pawn = UGameplayStatics::GetPlayerPawn(World, 0);
    APlayerController* PC = UGameplayStatics::GetPlayerController(World, 0);
    AOpenWillowEnemy* Enemy0 = Enemies.Num() > 0 ? Enemies[0].Get() : nullptr;
    AOpenWillowEnemy* Enemy1 = Enemies.Num() > 1 ? Enemies[1].Get() : nullptr;
    AOpenWillowCombatTarget* Dummy0 = Dummies.Num() > 0 ? Dummies[0].Get() : nullptr;

    if ((TestStep >= 16 && TestStep < 37) || (TestStep >= 90 && TestStep <= 93))
    {
        // The AI steps need the AI; the target steps before them ran with it off (an alerted range swarmed Maya and put her
        // down before the AI steps began, batch 2).
        if (!bModeAI) SetAI(true);
        TickAITest(Now);
        return;
    }
    if (TestStep >= 37 && TestStep < 90)
    {
        // The loot steps test the drops: the AI is off again so no blow lands on Maya while she walks onto a pickup.
        // The AI steps also left every enemy but one passive.
        if (bModeAI)
        {
            SetAI(false);
            SetMode(false, false);
        }
        TickLootTest(Now);
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
        // The target steps (1-15) test the targets, not the AI: the enemies only patrol until the AI steps switch it on.
        SetAI(false);
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
        // Mind's Eye on a real crit (skills lane): a gun hit on the head through the shared dispatcher does the event's bonus,
        // 2.0 untrained and 2.0 x (1 + 0.05 x 5) = 2.5 at grade 5. Progression is put back in this same tick. 1 point of damage
        // keeps the enemy's health for the later steps. SKIP only when the local skill effects file does not exist; anything else
        // that stops the check is a FAIL.
        AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(Pawn);
        UOpenWillowSkills* Tree = Maya ? Maya->GetSkills() : nullptr;
        if (Tree && Tree->IsEffectsFileMissing())
        {
            TestSkip(TEXT("crit_minds_eye"), TEXT("no local skill effects file"));
            TestStep = 5;
            return;
        }
        const FOpenWillowSkillEffects::FSkill* MindsEye = Tree ? Tree->GetEffects().Find(TEXT("MindsEye")) : nullptr;
        int32 Branch = 0, Tier = 0, Cell = 0;
        const TCHAR* Blocked = !Tree ? TEXT("no walker or skill tree")
            : !Tree->GetEffects().IsLoaded() ? TEXT("the skill effects file exists but did not load")
            : !MindsEye ? TEXT("no Mind's Eye in the loaded skill effects")
            : !Tree->FindSkill(MindsEye->Id, Branch, Tier, Cell) ? TEXT("Mind's Eye's id is not in the skill tree") : nullptr;
        if (Blocked)
        {
            TestCheck(TEXT("crit_minds_eye"), false, Blocked);
            TestStep = 5;
            return;
        }
        auto GunCrit = [&]()
        {
            FHitResult Hit;
            const FVector At = Crit->GetComponentLocation();
            const FVector Direction = (FVector(At.X, At.Y, 0.f) - FVector(TestSpot.X, TestSpot.Y, 0.f)).GetSafeNormal();
            Hit.Component = Crit;
            Hit.Location = At;
            Hit.ImpactPoint = At - Direction * 5.f;
            FOpenWillowHitEvent Event;
            Event.Kind = EOpenWillowHitKind::Direct;
            Event.Source = EOpenWillowHitSource::Bullet;
            Event.WeaponId = TEXT("test:crit_minds_eye");
            Event.Damage = 1.f;
            Event.DamageType = TEXT("GD_Impact.DamageType.DmgType_Normal");
            Event.Direction = Direction.IsNearlyZero() ? FVector(1.f, 0.f, 0.f) : Direction;
            Event.Origin = FVector(TestSpot.X, TestSpot.Y, At.Z);
            const float HealthBefore = Enemy0->GetHealth();
            Maya->ApplyShotDamage(Enemy0, Event, Hit);
            return HealthBefore - Enemy0->GetHealth();
        };
        const TSharedPtr<FJsonObject> Saved = Tree->ProgressionJson();
        Tree->ResetTree();
        const float Untrained = GunCrit();
        Tree->SetLevel(10);   // 6 points: the action skill, then Mind's Eye x 5 (tier 0 of its branch)
        FString Why;
        bool bSpent = Tree->TrySpend(-1, -1, -1, Why);
        for (int32 Grade = 0; Grade < 5; ++Grade) bSpent = bSpent && Tree->TrySpend(Branch, Tier, Cell, Why);
        const float Trained = GunCrit();
        FString RestoreError;
        const bool bRestored = Tree->RestoreProgression(*Saved, RestoreError);
        TestCheck(TEXT("crit_minds_eye"), bSpent && bRestored && FMath::IsNearlyEqual(Untrained, 2.f, 0.01f) && FMath::IsNearlyEqual(Trained, 2.5f, 0.01f),
            FString::Printf(TEXT("1 damage: untrained %.2f, Mind's Eye 5 %.2f, spent=%d %s, restored=%d %s"), Untrained, Trained, bSpent ? 1 : 0, *Why, bRestored ? 1 : 0, *RestoreError));
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
        // A shot from directly behind, as a player can fire it. The patrol may have walked the enemy behind a rock from Maya (Z16), and a low shot from her can
        // meet the ground first (Z13), so: the enemy stands at home and is turned until both lines below reach it from 500 uu. No side clear is a failure.
        // What makes a shot from behind critical: its line, followed on from the impact, must cross the head sphere itself. So the step also needs:
        // - the head to stand past the capsule-width reach the check used to stop at, or this shot could not catch that reach coming back;
        // - a shot at the small of the back, on the body and below the head, not to count.
        const UPrimitiveComponent* Body = Enemy0->BodyComponent();
        const UCapsuleComponent* Capsule = Cast<UCapsuleComponent>(Body);
        FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowRangeCritTrace), true);
        Query.AddIgnoredActor(Pawn);
        FHitResult Hit, BackHit;
        FVector Aim = FVector::ZeroVector, Back = FVector::ZeroVector, Forward = FVector::ForwardVector;
        bool bHeadLine = false, bBackLine = false;
        float TurnedTo = 0.f;
        FString Blocked;
        for (int32 Side = 0; Side < 8 && Body && !(bHeadLine && bBackLine); ++Side)
        {
            TurnedTo = (SpotNow - Enemy0->GetHome()).Rotation().Yaw + 180.f + 45.f * float(Side);
            Enemy0->ResetForAITest(Now, TurnedTo, 3.f);
            Forward = FRotator(0.f, TurnedTo, 0.f).Vector();
            Aim = Crit->GetComponentLocation();
            Back = Body->Bounds.Origin - FVector(0.f, 0.f, Body->Bounds.BoxExtent.Z * 0.5f);
            const FVector HeadFrom = Aim - Forward * 500.f, BackFrom = Back - Forward * 500.f;
            bHeadLine = World->LineTraceSingleByChannel(Hit, HeadFrom, Aim + Forward * 100.f, ECC_Visibility, Query) && Hit.GetActor() == Enemy0;
            bBackLine = World->LineTraceSingleByChannel(BackHit, BackFrom, Back + Forward * 100.f, ECC_Visibility, Query) && BackHit.GetActor() == Enemy0;
            if (!bHeadLine || !bBackLine)
                Blocked += FString::Printf(TEXT(" yaw %.0f: %s/%s;"), TurnedTo, Hit.GetActor() ? *Hit.GetActor()->GetName() : TEXT("nothing"),
                    BackHit.GetActor() ? *BackHit.GetActor()->GetName() : TEXT("nothing"));
        }
        const bool bClear = bHeadLine && bBackLine;
        const bool bCrit = bClear && Enemy0->IsCritRegion(Hit.Component.Get(), Hit.BoneName, Hit.ImpactPoint, Forward);
        const bool bBackCrit = bClear && Enemy0->IsCritRegion(BackHit.Component.Get(), BackHit.BoneName, BackHit.ImpactPoint, Forward);
        const float OldReach = Capsule ? 2.f * FMath::Max(Capsule->GetScaledCapsuleRadius(), Capsule->GetScaledCapsuleHalfHeight()) : 0.f;
        const float HeadPast = bClear ? float(FVector::Dist(Hit.ImpactPoint, Aim)) : 0.f;
        TestCheck(TEXT("crit_trace"), bClear && bCrit && Capsule && HeadPast > OldReach && !bBackCrit,
            FString::Printf(TEXT("clear side %d (yaw %.0f), first hit on %s, critical=%d, head %.0f uu past the impact (old reach %.0f), small of the back: first hit on %s, critical=%d; blocked:%s"),
                bClear ? 1 : 0, TurnedTo, Hit.Component.IsValid() ? *Hit.Component->GetName() : TEXT("nothing"), bCrit ? 1 : 0, HeadPast, OldReach,
                BackHit.Component.IsValid() ? *BackHit.Component->GetName() : TEXT("nothing"), bBackCrit ? 1 : 0, *Blocked));
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
        // SKIP only when the manifest asks for no stock mesh. One it names that did not load, or came without a physics asset, is a failure.
        const FOpenWillowRangeKind* Kind0 = Enemy0 ? Manifest.Kinds.Find(EnemyKinds.FindRef(Enemy0)) : nullptr;
        const bool bWantsStock = Kind0 && !Kind0->Mesh.IsEmpty();
        if (!Mesh && !bWantsStock) TestSkip(TEXT("ragdoll"), TEXT("the manifest names no stock mesh"));
        else if (!Mesh) TestCheck(TEXT("ragdoll"), false, TEXT("the manifest names a stock mesh, and it did not load"));
        else if (!Mesh->GetPhysicsAsset()) TestCheck(TEXT("ragdoll"), false, TEXT("the stock mesh has no physics asset"));
        else TestCheck(TEXT("ragdoll"), Mesh->IsSimulatingPhysics(), FString::Printf(TEXT("simulating=%d"), Mesh->IsSimulatingPhysics() ? 1 : 0));
        TestStep = 13;
        return;
    }
    case 13:
    {
        // A second before the respawn: the body stayed down (the host's sanity bounds for a limp body, not game values). This kill has no shot
        // behind it, so it may never reach a cap: corpse_cap then throws one body to see the cap act.
        if (TestCorpseStage == 0)
        {
            if (Now - DeathAt < FMath::Max(Manifest.RespawnSeconds - 1.f, 1.2f)) return;
            const FOpenWillowRangeKind* Kind0 = Enemy0 ? Manifest.Kinds.Find(EnemyKinds.FindRef(Enemy0)) : nullptr;
            if (!Enemy0 || !Enemy0->CorpseMeasured())
            {
                if (Kind0 && !Kind0->Mesh.IsEmpty())
                {
                    TestCheck(TEXT("corpse_limp"), false, TEXT("the manifest names a stock mesh, and no ragdoll simulated"));
                    TestCheck(TEXT("corpse_cap"), false, TEXT("the manifest names a stock mesh, and no ragdoll simulated"));
                }
                else
                {
                    TestSkip(TEXT("corpse_limp"), TEXT("the manifest names no stock mesh"));
                    TestSkip(TEXT("corpse_cap"), TEXT("the manifest names no stock mesh"));
                }
                TestCorpseStage = 2;
                return;
            }
            TestCheck(TEXT("corpse_limp"), Enemy0->CorpsePeakRise() <= 200.f && Enemy0->CorpsePeakDrift() <= 500.f,
                FString::Printf(TEXT("rose %.0f uu, drifted %.0f uu, fastest body %.0f uu/s, %d speed caps"), Enemy0->CorpsePeakRise(), Enemy0->CorpsePeakDrift(),
                    Enemy0->CorpsePeakSpeed(), Enemy0->CorpseSpeedCaps()));
            TestCorpseCaps = Enemy0->CorpseSpeedCaps();
            if (!Enemy0->KickCorpseForTest(3000.f))
            {
                TestCheck(TEXT("corpse_cap"), false, TEXT("no simulating body to throw"));
                TestCorpseStage = 2;
                return;
            }
            TestCorpseKickAt = Now;
            TestCorpseStage = 1;
            return;
        }
        if (TestCorpseStage == 1)
        {
            if (Now - TestCorpseKickAt < 0.1f) return;
            const float Rise = Enemy0 ? Enemy0->CorpseKickedRiseSpeed() : 0.f;
            const int32 Caps = Enemy0 ? Enemy0->CorpseSpeedCaps() - TestCorpseCaps : 0;
            TestCheck(TEXT("corpse_cap"), Caps > 0 && Rise < 1000.f, FString::Printf(TEXT("thrown at 3000 uu/s: %d caps, rising at %.0f uu/s 0.1 s later"), Caps, Rise));
            TestCorpseStage = 2;
            return;
        }
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
        // A way in that fails for want of a spot must leave a reason, or the slice gate takes another target for the enemy (Luna r3). Last, so it can't disturb a step.
        bTestNoSpot = true;
        BuildRefusal.Reset();
        const bool bBuiltNoSpot = BuildAtResolvedSpot();
        bTestNoSpot = false;
        TestCheck(TEXT("refusal_no_spot"), !bBuiltNoSpot && BuildRefusal.StartsWith(TEXT("no spot")), FString::Printf(TEXT("built=%d, refusal: %s"), bBuiltNoSpot ? 1 : 0, *BuildRefusal));
        BuildRefusal.Reset();
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
        TestStep = 37;
        return;
    }
    if (!E0)
    {
        const TCHAR* Names[] = {TEXT("notice_proximity"), TEXT("notice_sight"), TEXT("no_notice_far"), TEXT("provoked"),
            TEXT("approach"), TEXT("melee_hit"), TEXT("melee_interval"), TEXT("melee_miss"), TEXT("forget"), TEXT("passive_no_melee"),
            TEXT("swing_range")};
        for (const TCHAR* Name : Names) TestSkip(Name, TEXT("no enemy"));
        TestStep = 37;
        return;
    }
    const FOpenWillowEnemyBrain& Brain = E0->GetBrain();
    // What Maya's pools hold (health plus shield): the delivery drains the shield first, then health.
    auto Pools = [Walker]() { return Walker->GetHealth() + Walker->GetShield(); };
    const int32 Seen = OpenWillowThreat::SeenInThreatRadius;

    // The enemy's facing, horizontal.
    auto Forward = [](const AOpenWillowEnemy* Enemy) { return FRotator(0.f, Enemy->GetActorRotation().Yaw, 0.f).Vector(); };
    auto YawToward = [](const FVector& From, const FVector& To) { return (To - From).Rotation().Yaw; };
    // Maya on the floor under XY (her origin at the floor plus her half height), facing Yaw. False when there is no floor there.
    auto PlaceMaya = [this, World, Pawn](const FVector2D& XY, float Yaw) -> bool
    {
        FVector Floor;
        if (!AOpenWillowEnemy::TraceFloor(World, XY, SpotNow.Z + PlaceReach, SpotNow.Z - PlaceReach, Pawn, Floor)) return false;
        TeleportMaya(World, Floor, Yaw);
        return true;
    };
    // Maya in E0's sight: ahead of it, beyond the proximity reach, inside the den's ThreatRadius, on a floor, with the enemy's own
    // sight line clear (at a fixed 2000 uu the ground at this spot blocked it: batch 7). False when no such place is found.
    auto PlaceInSight = [&](FString& OutWhere) -> bool
    {
        const float HalfHeight = Walker->GetCapsuleComponent()->GetScaledCapsuleHalfHeight();
        for (const float Distance : {2000.f, 1600.f, 1300.f, 1000.f})
            for (const float Angle : {0.f, 15.f, -15.f, 30.f, -30.f, 45.f, -45.f})
            {
                const FVector Dir = FRotator(0.f, E0->GetActorRotation().Yaw + Angle, 0.f).Vector();
                const FVector At = E0->GetActorLocation() + Dir * Distance;
                FVector Floor;
                if (!AOpenWillowEnemy::TraceFloor(World, FVector2D(At.X, At.Y), SpotNow.Z + PlaceReach, SpotNow.Z - PlaceReach, Pawn, Floor)) continue;
                if (FVector2D::Distance(FVector2D(Floor.X, Floor.Y), Den.Centre) >= Den.ThreatRadius) continue;
                if (!E0->HasLineOfSightTo(Floor + FVector(0.f, 0.f, HalfHeight), Walker)) continue;
                TeleportMaya(World, Floor, YawToward(Floor, E0->GetActorLocation()));
                OutWhere = FString::Printf(TEXT("%.0f uu at %.0f degrees"), Distance, Angle);
                return true;
            }
        return false;
    };
    // Maya far from the den: outside its ThreatRadius, on a floor, wherever the terrain has one (a fixed direction ran off the
    // terrain at this spot: batch 7). False when no ring has a floor.
    auto PlaceFar = [&]() -> bool
    {
        for (const float Radius : {Den.ThreatRadius + 500.f, Den.ThreatRadius + 1500.f, Den.ThreatRadius + 3000.f})
            for (int32 Index = 0; Index < 12; ++Index)
            {
                const FVector At = SpotNow + FRotator(0.f, SpotYawNow + 30.f * float(Index), 0.f).Vector() * Radius;
                if (PlaceMaya(FVector2D(At.X, At.Y), YawToward(At, E0->GetActorLocation()))) return true;
            }
        return false;
    };
    // The scene of a check, for its log line: Maya's distance from E0 and from the den's centre, E0's distance from home, the den's radii.
    auto Scene = [this, Walker, E0]()
    {
        const FVector Maya = Walker->GetActorLocation();
        const float FromDen = float(FVector2D::Distance(FVector2D(Maya.X, Maya.Y), Den.Centre));
        return FString::Printf(TEXT("maya_to_e0=%.0f maya_from_den=%.0f (threat %.0f, patrol %.0f) e0_from_home=%.0f ai=%d passive=%d down=%d"),
            FVector::Dist(Maya, E0->GetActorLocation()), FromDen, Den.ThreatRadius, Den.PatrolRadius,
            FVector::Dist2D(E0->GetActorLocation(), E0->GetHome()), E0->IsAIEnabled() ? 1 : 0, E0->IsPassive() ? 1 : 0,
            Walker->IsDown() ? 1 : 0);
    };
    auto Reset = [this, Pawn, E0, Now](const TCHAR* Why)
    {
        Den.ResetRecord();
        UE_LOG(LogTemp, Display, TEXT("OWRANGE threat reset (test, %s) level=1"), Why);
        // E0 back home, facing the spot as built, its patrol held 3 s: every step lays its scene out from the same place.
        E0->ResetForAITest(Now, (SpotNow - E0->GetHome()).Rotation().Yaw, 3.f);
        // Test fixture: Maya back to full health before each step, so the earlier steps' blows cannot put her down
        // (ApplyScriptHealth sets the maximum and current health; it does not raise a downed Maya, which the steps report).
        if (AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(Pawn); Maya && !Maya->IsDown())
        {
            const float Before = Maya->GetHealth();
            Maya->ApplyScriptHealth(Maya->GetMaxHealth(), Maya->GetMaxHealth());
            UE_LOG(LogTemp, Display, TEXT("OWRANGE maya restored health %.1f -> %.1f (test)"), Before, Maya->GetHealth());
        }
    };
    // Only Keep is active; every other enemy is passive (no notice, no melee).
    auto OnlyActive = [this](const AOpenWillowEnemy* Keep)
    {
        for (const auto& Weak : Enemies)
            if (AOpenWillowEnemy* Enemy = Weak.Get()) Enemy->SetPassive(Enemy != Keep);
    };
    // Maya 250 uu ahead of an enemy, inside its melee volume (the origin is 100 uu behind it, 350 uu to Maya). A kind whose swing range
    // is shorter closes the rest first; the steps allow for that.
    auto PlaceInVolume = [&](AOpenWillowEnemy* Enemy) -> bool
    {
        const FVector At = Enemy->GetActorLocation() + Forward(Enemy) * 250.f;
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
        TestCheck(TEXT("notice_proximity"), Den.Level >= Seen, FString::Printf(TEXT("level %d, %.2f s after the teleport (500 uu behind), %s"),
            Den.Level, Now - StepStart, *Scene()));
        TestStep = 18;
        return;
    }
    case 18: // notice_sight: Maya 2000 uu in front of E0, in its line of sight.
    {
        Reset(TEXT("sight"));
        OnlyActive(E0);
        if (!PlaceInSight(TestWhere))
        {
            TestCheck(TEXT("notice_sight"), false, TEXT("no place ahead of the enemy with a clear sight line"));
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
        TestCheck(TEXT("notice_sight"), Den.Level >= Seen, FString::Printf(TEXT("level %d, %.2f s after the teleport (%s in front), %s"),
            Den.Level, Now - StepStart, *TestWhere, *Scene()));
        TestStep = 20;
        return;
    }
    case 20: // no_notice_far: Maya 6000 uu from the spot, on the far side of the enemies. No rule holds for 3 s.
    {
        Reset(TEXT("far"));
        OnlyActive(E0);
        if (!PlaceFar())
        {
            TestCheck(TEXT("no_notice_far"), false, TEXT("no floor outside the den's threat radius"));
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
        // A hit from afar: only the damage can raise the level. Near her a notice rule could raise it too and the step would prove
        // nothing, so without a far place the step fails before the hit.
        if (!PlaceFar())
        {
            TestCheck(TEXT("provoked"), false, TEXT("no floor outside the den's threat radius: no hit made"));
            TestStep = 24;
            return;
        }
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
        if (!PlaceInSight(TestWhere))
        {
            TestCheck(TEXT("approach"), false, TEXT("no place ahead of the enemy with a clear sight line"));
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
            FString::Printf(TEXT("closed %.0f uu in 2 s (%.0f to %.0f), level %d, %s"), TestDistance - Gap, TestDistance, Gap, Den.Level, *Scene()));
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
        TestHealthBefore = Pools();
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
        const float Delta = TestHealthBefore - Pools();
        const float Expected = Brain.LastArrival * Brain.LastRatio * Brain.LastScale;
        const float Timing = Brain.LastAt - Brain.LastSwingStart;
        const bool bOk = Brain.MeleeResolved == TestResolvedBase + 1 && Brain.LastHit && Brain.LastApplied > 0.f
            && FMath::IsNearlyEqual(Brain.LastApplied, Expected, 0.01f) && FMath::Abs(Timing - OpenWillowThreat::SwingDamageDelay) <= 0.1f
            && FMath::IsNearlyEqual(Delta, Brain.LastApplied, 0.01f);
        TestCheck(TEXT("melee_hit"), bOk, FString::Printf(TEXT("hit %d at +%.3f s, applied %.2f (arrival %.2f x ratio %.4f x scale %.2f), "
            "maya health %.1f -> %.1f"), Brain.LastHit ? 1 : 0, Timing, Brain.LastApplied, Brain.LastArrival, Brain.LastRatio,
            Brain.LastScale, TestHealthBefore, Pools()));
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
        // The second swing is still pending at its start: wait for it to resolve, so its hit cannot land inside the next step.
        if (Brain.IsSwingPending()) return;
        const float Gap = Brain.LastSwingStart - Brain.PrevSwingStart;
        TestCheck(TEXT("melee_interval"), FMath::Abs(Gap - OpenWillowThreat::SwingInterval) <= 0.1f,
            FString::Printf(TEXT("%.3f s between the last two swing starts"), Gap));
        TestStep = 30;
        return;
    }
    case 30: // melee_miss setup: Maya in the volume; she steps out after the swing starts.
    {
        // No swing of the earlier steps may still be pending when the baseline is taken (it would resolve inside this step).
        if (E0 && E0->GetBrain().IsSwingPending()) return;
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
        TestHealthBefore = Pools();
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
            && FMath::IsNearlyEqual(Pools(), TestHealthBefore, 0.01f)
            && TestMovedAt < Brain.LastSwingStart + OpenWillowThreat::SwingDamageDelay;
        TestCheck(TEXT("melee_miss"), bOk, FString::Printf(TEXT("hit %d, applied %.2f, moved %.3f s after the swing start, maya health %.1f -> %.1f"),
            Brain.LastHit ? 1 : 0, Brain.LastApplied, TestMovedAt - Brain.LastSwingStart, TestHealthBefore, Pools()));
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
            TestCheck(TEXT("forget"), false, FString::Printf(TEXT("not raised by the proximity rule, %s"), *Scene()));
            TestStep = 35;
            return;
        }
        // Out of every rule now: far from the enemies. The record is left to run out.
        if (!PlaceFar())
        {
            TestCheck(TEXT("forget"), false, TEXT("no floor outside the den's threat radius"));
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
            TestStep = 37;
            return;
        }
        TestSwingBase = Brain.SwingCount;
        TestHealthBefore = Pools();
        StepStart = Now;
        TestStep = 36;
        return;
    }
    case 36:
    {
        if (Now - StepStart < 2.f) return;
        const bool bOk = Brain.SwingCount == TestSwingBase && FMath::IsNearlyEqual(Pools(), TestHealthBefore, 0.01f)
            && Den.Level == OpenWillowThreat::InThreatRadius;
        TestCheck(TEXT("passive_no_melee"), bOk, FString::Printf(TEXT("swing starts %d, maya health %.1f -> %.1f, level %d"),
            Brain.SwingCount - TestSwingBase, TestHealthBefore, Pools(), Den.Level));
        SetMode(false, false);
        TestStep = 92;
        return;
    }
    case 92: // swing_range: Maya 480 uu ahead of E0: the blow would reach her there, but the swing must wait for its stock range.
    {
        Reset(TEXT("swing range"));
        OnlyActive(E0);
        const FVector At = E0->GetActorLocation() + Forward(E0) * 480.f;
        if (!PlaceMaya(FVector2D(At.X, At.Y), YawToward(At, E0->GetActorLocation())))
        {
            TestSkip(TEXT("swing_range"), TEXT("no floor in front of the enemy"));
            TestStep = 90;
            return;
        }
        TestSwingBase = Brain.SwingCount;
        StepStart = Now;
        TestStep = 93;
        return;
    }
    case 93:
    {
        if (Brain.SwingCount <= TestSwingBase)
        {
            if (Now - StepStart > 4.f)
            {
                TestCheck(TEXT("swing_range"), false, TEXT("no swing start in 4 s"));
                TestStep = 90;
            }
            return;
        }
        // E0's kind only. A range of 480 or more would let an old-style swing at once pass, so the range itself is checked first.
        const FOpenWillowRangeKind* Kind = Manifest.Kinds.Find(EnemyKinds.FindRef(E0));
        const float Range = Kind ? Kind->MeleeStartRange : 0.f;
        TestCheck(TEXT("swing_range"), Range > 0.f && Range < 480.f && Brain.LastSwingDistance > 0.f && Brain.LastSwingDistance <= Range,
            FString::Printf(TEXT("kind %s: swing started %.0f uu from Maya, %.2f s after she stood 480 uu ahead, stock range %.0f"), *EnemyKinds.FindRef(E0),
                Brain.LastSwingDistance, Now - StepStart, Range));
        TestStep = 90;
        return;
    }
    case 90: // psycho_melee: the range's psycho swings for its own kind's numbers (VM only, UNVERIFIED), not the bullymong's.
    {
        Reset(TEXT("psycho"));
        TestPsycho = nullptr;
        for (const auto& Pair : EnemyKinds)
            if (Pair.Value == TEXT("psycho")) TestPsycho = const_cast<AOpenWillowEnemy*>(Pair.Key);
        AOpenWillowEnemy* Psycho = TestPsycho.Get();
        if (!Psycho)
        {
            TestSkip(TEXT("psycho_melee"), TEXT("no psycho on this range (its mesh is not imported)"));
            TestStep = 37;
            return;
        }
        Psycho->ResetForAITest(Now, (SpotNow - Psycho->GetHome()).Rotation().Yaw, 3.f);
        OnlyActive(Psycho);
        if (!PlaceInVolume(Psycho))
        {
            TestCheck(TEXT("psycho_melee"), false, TEXT("no floor in front of the psycho"));
            TestStep = 37;
            return;
        }
        TestResolvedBase = Psycho->GetBrain().MeleeResolved;
        TestHealthBefore = Pools();
        StepStart = Now;
        TestStep = 91;
        return;
    }
    case 91:
    {
        AOpenWillowEnemy* Psycho = TestPsycho.Get();
        if (!Psycho)
        {
            TestCheck(TEXT("psycho_melee"), false, TEXT("the psycho is gone"));
            TestStep = 37;
            return;
        }
        const FOpenWillowEnemyBrain& PsychoBrain = Psycho->GetBrain();
        if (PsychoBrain.MeleeResolved <= TestResolvedBase)
        {
            if (Now - StepStart > 3.f)
            {
                TestCheck(TEXT("psycho_melee"), false, TEXT("no swing resolved in 3 s"));
                TestStep = 37;
            }
            return;
        }
        // What this measures: the manifest's own mesh is on the pawn, and Maya's pools lost what the psycho's tables say, worked out here and not
        // taken from the brain. It says nothing of the psycho's stock AI tree (not run), or of whether those VM numbers match the game.
        float Arrival = 0.f, Ratio = 0.f;
        const FOpenWillowRangeKind* Kind = Manifest.Kinds.Find(TEXT("psycho"));
        const bool bTable = Kind && OpenWillowValueAtLevel(Kind->MeleeArrivalByLevel, Psycho->GetLevel(), Arrival)
            && OpenWillowValueAtLevel(Kind->MeleeRatioByLevel, Psycho->GetLevel(), Ratio);
        const USkeletalMeshComponent* PsychoMesh = Psycho->GetStockMesh();
        const USkeletalMesh* PsychoAsset = PsychoMesh ? PsychoMesh->GetSkeletalMeshAsset() : nullptr;
        const FString AssetPath = PsychoAsset ? PsychoAsset->GetPathName() : FString();
        const bool bOwnMesh = Kind && !Kind->Mesh.IsEmpty() && AssetPath == Kind->Mesh;
        const int32 MayaLevel = Walker->GetSkills() ? Walker->GetSkills()->GetLevel() : Psycho->GetLevel();
        const float Expected = FMath::Min(Arrival * Ratio * MeleeLevelScale(Psycho->GetLevel(), MayaLevel), TestHealthBefore);
        const float Delta = TestHealthBefore - Pools();
        const bool bOk = bTable && bOwnMesh && PsychoBrain.LastHit && Expected > 0.f && FMath::IsNearlyEqual(Delta, Expected, 0.01f);
        TestCheck(TEXT("psycho_melee"), bOk, FString::Printf(TEXT("mesh %s (own %d), hit %d, arrival %.2f x ratio %.4f, pools %.1f -> %.1f, expected loss %.2f"),
            *AssetPath, bOwnMesh ? 1 : 0, PsychoBrain.LastHit ? 1 : 0, Arrival, Ratio, TestHealthBefore, Pools(), Expected));
        TestStep = 37;
        return;
    }
    default:
        TestStep = 37;
        return;
    }
}

// ---------------------------------------------------------------------------------------------- loot driver (PR3, steps 37-44)

namespace
{
// The pickups of a roll that are still in the world: weapon pickups (bWeapon) or range pickups (money and ammo).
int32 CountLivePickups(const FOpenWillowRangeLootResult& Loot, bool bWeapon)
{
    int32 Count = 0;
    for (const TWeakObjectPtr<AActor>& Weak : Loot.Spawned)
        if (Weak.IsValid() && (bWeapon ? Weak->IsA<AOpenWillowInventoryPickup>() : Weak->IsA<AOpenWillowRangePickup>())) ++Count;
    return Count;
}

// The first live pickup of that kind (see CountLivePickups), or null.
AActor* FirstLivePickup(const FOpenWillowRangeLootResult& Loot, bool bWeapon)
{
    for (const TWeakObjectPtr<AActor>& Weak : Loot.Spawned)
        if (Weak.IsValid() && (bWeapon ? Weak->IsA<AOpenWillowInventoryPickup>() : Weak->IsA<AOpenWillowRangePickup>())) return Weak.Get();
    return nullptr;
}
}

void UOpenWillowRangeSubsystem::TickLootTest(float Now)
{
    // Steps 37-44 (PR3, loot on death). A kill's roll runs inside the kill (HandleEnemyDied), so LastLoot is ready at once; the
    // pickups are read after their own short waits. Mode always is set for the roll checks; the mode is normal again at the end.
    // Host stand-ins (UNVERIFIED in game): the drops, the money value and the pickup look.
    UWorld* World = GetWorld();
    APawn* Pawn = UGameplayStatics::GetPlayerPawn(World, 0);
    const AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(Pawn);
    const UOpenWillowInventory* Inventory = Walker ? Walker->GetInventory() : nullptr;
    AOpenWillowEnemy* E0 = Enemies.Num() > 0 ? Enemies[0].Get() : nullptr;
    AOpenWillowEnemy* E1 = Enemies.Num() > 1 ? Enemies[1].Get() : nullptr;
    const bool bWeaponsAvailable = FOpenWillowWeaponGen::IsAvailable();

    switch (TestStep)
    {
    case 37: // loot_roll: mode always, kill enemy 0. Money (and a weapon, when the weapon manifest is there) lies at its spot.
    {
        if (!Walker || !Inventory || !E0 || E0->IsDead())
        {
            const TCHAR* Names[] = {TEXT("loot_near_corpse"), TEXT("loot_roll"), TEXT("loot_money_pickup"), TEXT("loot_weapon_item"), TEXT("loot_off"),
                TEXT("loot_seeded_roll")};
            for (const TCHAR* Name : Names) TestSkip(Name, TEXT("no walker or no live enemy 0"));
            TestStep = 99;
            return;
        }
        SetLootMode(EOpenWillowRangeLootMode::Always);
        const FVector Corpse = E0->GetActorLocation();
        TestHit(E0, 1.e6f, E0->BodyComponent());
        LootDeathAt = Now;
        // Every drop of the roll lies near the corpse (the drop rings reach 180 uu; the slack covers the floor's height).
        float Farthest = 0.f;
        int32 Drops = 0;
        for (const TWeakObjectPtr<AActor>& Weak : LastLoot.Spawned)
            if (const AActor* Drop = Weak.Get())
            {
                ++Drops;
                Farthest = FMath::Max(Farthest, float(FVector::Dist(Drop->GetActorLocation(), Corpse)));
            }
        // No drop may have used the "no room" fall-back: the room test on sloped ground is what this step proves.
        TestCheck(TEXT("loot_near_corpse"), Drops > 0 && Farthest <= LootNearCorpse && LastLoot.DropFallbacks == 0,
            FString::Printf(TEXT("%d drop(s), farthest %.0f uu from the corpse (limit %.0f), %d at the no-room fall-back"), Drops, Farthest,
                LootNearCorpse, LastLoot.DropFallbacks));
        const int32 MoneyCount = CountLivePickups(LastLoot, false);
        const int32 WeaponCount = CountLivePickups(LastLoot, true);
        const bool bOk = E0->IsDead() && MoneyCount >= 1 && (WeaponCount >= 1 || !bWeaponsAvailable);
        TestCheck(TEXT("loot_roll"), bOk, FString::Printf(TEXT("money pickups %d, weapon pickups %d (weapon manifest %s), ammo pickups %d"),
            MoneyCount, WeaponCount, bWeaponsAvailable ? TEXT("yes") : TEXT("no"), LastLoot.Ammo));
        TestStep = 38;
        return;
    }
    case 38: // loot_money_pickup: Maya is moved onto the roll's money pickup.
    {
        AActor* Money = FirstLivePickup(LastLoot, false);
        if (!Money || !Inventory)
        {
            TestCheck(TEXT("loot_money_pickup"), false, TEXT("no money pickup from the roll"));
            TestStep = 40;
            return;
        }
        LootMoneyPickup = Money;
        LootMoneyAmount = Cast<AOpenWillowRangePickup>(Money)->GetAmount();
        Inventory->GetMoney(LootMoneyBefore);
        TeleportMaya(World, Money->GetActorLocation(), Pawn->GetActorRotation().Yaw);
        StepStart = Now;
        TestStep = 39;
        return;
    }
    case 39: // loot_money_pickup, a second later: the money rose by the pickup's amount and the pickup is gone.
    {
        if (Now - StepStart < 1.f) return;
        int32 MoneyAfter = 0;
        if (Inventory) Inventory->GetMoney(MoneyAfter);
        const bool bGone = !LootMoneyPickup.IsValid();
        const bool bOk = bGone && MoneyAfter == LootMoneyBefore + LootMoneyAmount;
        TestCheck(TEXT("loot_money_pickup"), bOk, FString::Printf(TEXT("money %d -> %d (pickup amount %d), pickup gone=%d"),
            LootMoneyBefore, MoneyAfter, LootMoneyAmount, bGone ? 1 : 0));
        TestStep = 40;
        return;
    }
    case 40: // loot_weapon_item: the roll's weapon pickup has a name (it carries a rolled gun).
    {
        if (!bWeaponsAvailable)
        {
            TestSkip(TEXT("loot_weapon_item"), TEXT("weapon generator manifest missing: no gun is rolled"));
            TestStep = 41;
            return;
        }
        const AOpenWillowInventoryPickup* Weapon = Cast<AOpenWillowInventoryPickup>(FirstLivePickup(LastLoot, true));
        const FString Name = Weapon ? Weapon->DisplayName() : FString();
        TestCheck(TEXT("loot_weapon_item"), Weapon && !Name.IsEmpty(),
            Weapon ? FString::Printf(TEXT("pickup %s"), *Name) : FString(TEXT("no weapon pickup from the roll")));
        TestStep = 41;
        return;
    }
    case 41: // loot_off: mode off, kill enemy 1 (it must be alive). The mode is normal again straight after that kill.
    {
        SetLootMode(EOpenWillowRangeLootMode::Normal);
        if (!E1 || E1->IsDead())
        {
            TestSkip(TEXT("loot_off"), TEXT("no second live enemy"));
            TestStep = 43;
            return;
        }
        SetLootMode(EOpenWillowRangeLootMode::Off);
        TestHit(E1, 1.e6f, E1->BodyComponent());
        SetLootMode(EOpenWillowRangeLootMode::Normal);
        StepStart = Now;
        TestStep = 42;
        return;
    }
    case 42: // loot_off, half a second later: the kill left nothing behind.
    {
        if (Now - StepStart < 0.5f) return;
        const bool bDead = E1 && E1->IsDead();
        const bool bOk = bDead && LastLoot.Spawned.Num() == 0 && LastLoot.Trace.IsEmpty();
        TestCheck(TEXT("loot_off"), bOk, FString::Printf(TEXT("enemy 1 dead=%d, pickups from the kill %d"), bDead ? 1 : 0,
            LastLoot.Spawned.Num()));
        TestStep = 43;
        return;
    }
    case 43: // loot_seeded_roll, first kill: enemy 0 after its respawn, the stream reset, mode normal.
    {
        if (!E0)
        {
            TestSkip(TEXT("loot_seeded_roll"), TEXT("no enemy 0"));
            TestStep = 99;
            return;
        }
        if (E0->IsDead())
        {
            if (Now - LootDeathAt > Manifest.RespawnSeconds + 5.f)
            {
                TestCheck(TEXT("loot_seeded_roll"), false, TEXT("enemy 0 did not respawn"));
                TestStep = 99;
            }
            return;
        }
        ResetLootStream();
        TestHit(E0, 1.e6f, E0->BodyComponent());
        LootTraceA = LastLoot.Trace;
        LootDeathAt = Now;
        TestStep = 44;
        return;
    }
    case 44: // loot_seeded_roll, second kill: the stream reset to its seed again, so the roll's trace must repeat.
    {
        if (!E0)
        {
            TestCheck(TEXT("loot_seeded_roll"), false, TEXT("enemy 0 is gone"));
            TestStep = 99;
            return;
        }
        if (E0->IsDead())
        {
            if (Now - LootDeathAt > Manifest.RespawnSeconds + 5.f)
            {
                TestCheck(TEXT("loot_seeded_roll"), false, TEXT("enemy 0 did not respawn for the second kill"));
                TestStep = 99;
            }
            return;
        }
        ResetLootStream();
        TestHit(E0, 1.e6f, E0->BodyComponent());
        const bool bOk = !LootTraceA.IsEmpty() && LastLoot.Trace == LootTraceA;
        TestCheck(TEXT("loot_seeded_roll"), bOk, FString::Printf(TEXT("first %s, second %s"), *LootTraceA, *LastLoot.Trace));
        SetLootMode(EOpenWillowRangeLootMode::Normal);
        TestStep = 99;
        return;
    }
    default:
        SetLootMode(EOpenWillowRangeLootMode::Normal);
        TestStep = 99;
        return;
    }
}

// ---------------------------------------------------------------------------------------------- survey driver (-owrangesurvey)

void UOpenWillowRangeSubsystem::TickSurvey(float Now)
{
    // PR2 follow-up: for each of the first SurveyCount manifest candidates, the accepted spot (a range is built there) or, when no
    // point passes, the raw candidate (no range). Maya is moved there, a frame is saved 2.5 s later, and the next candidate starts
    // a second after that frame, so the next move is not in its picture. Host stand-in layout (UNVERIFIED in game).
    UWorld* World = GetWorld();
    APawn* Pawn = UGameplayStatics::GetPlayerPawn(World, 0);
    APlayerController* PC = UGameplayStatics::GetPlayerController(World, 0);
    if (!Pawn) return;
    if (SurveyArmedAt < 0.f) { SurveyArmedAt = Now; return; }
    if (Now - SurveyArmedAt < 4.f) return;
    EnsureManifest();
    const int32 Total = FMath::Min(SurveyCount, Manifest.Candidates.Num());
    if (SurveyStepAt < 0.f)
    {
        if (SurveyIndex >= Total)
        {
            UE_LOG(LogTemp, Display, TEXT("OWRANGESURVEY done candidates=%d accepted=%d"), Total, SurveyAccepted);
            bSurveyDone = true;
            if (PC) PC->ConsoleCommand(TEXT("quit"));
            return;
        }
        // One candidate: its accepted spot with the range built there, else the raw candidate with no range.
        ClearRange();
        const FString Terrain = Manifest.Candidates[SurveyIndex].Source;
        FVector Spot = Manifest.Candidates[SurveyIndex].Location;
        float Yaw = AwayFromTown(Spot, Manifest.ScoutCentre);
        FString Material = TEXT("none"), Why;
        bool bAccepted = false;
        FVector Found;
        float FoundYaw = 0.f;
        FString FoundMaterial;
        if (ResolveCandidate(SurveyIndex, Found, FoundYaw, FoundMaterial, Why))
        {
            if (BuildRange(Found, FoundYaw))
            {
                bAccepted = true;
                ++SurveyAccepted;
                Spot = Found;
                Yaw = FoundYaw;
                Material = FoundMaterial;
                Why = TEXT("none");
            }
            else
            {
                Why = TEXT("build failed");
            }
        }
        TeleportMaya(World, Spot, Yaw);
        UE_LOG(LogTemp, Display, TEXT("OWRANGESURVEY index=%d accepted=%d at=%.0f,%.0f,%.0f yaw=%.0f terrain=%s material=%s reason=%s"),
            SurveyIndex, bAccepted ? 1 : 0, Spot.X, Spot.Y, Spot.Z, Yaw, *Terrain, *Material, *Why);
        SurveyStepAt = Now;
        bSurveyShot = false;
        return;
    }
    if (!bSurveyShot && Now - SurveyStepAt >= 2.5f)
    {
        if (FParse::Param(FCommandLine::Get(), TEXT("owrangeshots"))) RequestRangeShot(FString::Printf(TEXT("survey_%d.png"), SurveyIndex));
        bSurveyShot = true;
    }
    else if (bSurveyShot && Now - SurveyStepAt >= 3.5f)
    {
        ++SurveyIndex;
        SurveyStepAt = -1.f;
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
    else
    {
        Range->BuildAtResolvedSpot();
        return;
    }
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
    // Clamped to the levels the manifest's tables cover (the health, melee and loot rows stop there).
    int32 Wanted = FCString::Atoi(*Args[0]);
    int32 Low = 0, High = 0;
    if (Wanted > 0 && Range->TableLevels(Low, High) && (Wanted < Low || Wanted > High))
    {
        const int32 Clamped = FMath::Clamp(Wanted, Low, High);
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE ow.RangeLevel %d is outside the manifest's levels %d-%d: using %d"), Wanted, Low, High, Clamped);
        Wanted = Clamped;
    }
    Range->SetLevelOverride(Wanted);
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

void RangeLootCommand(const TArray<FString>& Args, UWorld* World)
{
    UOpenWillowRangeSubsystem* Range = World ? World->GetSubsystem<UOpenWillowRangeSubsystem>() : nullptr;
    if (!Range) return;
    const FString LootModeName = Args.Num() > 0 ? Args[0] : FString();
    if (LootModeName.Equals(TEXT("normal"), ESearchCase::IgnoreCase)) Range->SetLootMode(EOpenWillowRangeLootMode::Normal);
    else if (LootModeName.Equals(TEXT("always"), ESearchCase::IgnoreCase)) Range->SetLootMode(EOpenWillowRangeLootMode::Always);
    else if (LootModeName.Equals(TEXT("off"), ESearchCase::IgnoreCase)) Range->SetLootMode(EOpenWillowRangeLootMode::Off);
    else
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE usage: ow.RangeLoot normal|always|off"));
        return;
    }
    UE_LOG(LogTemp, Display, TEXT("OWRANGE loot mode=%s"), *LootModeName);
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
void RangeAICommand(const TArray<FString>& Args, UWorld* World)
{
    UOpenWillowRangeSubsystem* Range = World ? World->GetSubsystem<UOpenWillowRangeSubsystem>() : nullptr;
    if (!Range) return;
    const FString Mode = Args.Num() > 0 ? Args[0] : FString();
    if (Mode.Equals(TEXT("on"), ESearchCase::IgnoreCase)) Range->SetAI(true);
    else if (Mode.Equals(TEXT("off"), ESearchCase::IgnoreCase)) Range->SetAI(false);
    else UE_LOG(LogTemp, Warning, TEXT("OWRANGE usage: ow.RangeAI on|off"));
}

FAutoConsoleCommandWithWorldAndArgs GRangeAICommand(TEXT("ow.RangeAI"),
    TEXT("ow.RangeAI on|off: range enemies notice and attack Maya (on, default) or only patrol (off)"),
    FConsoleCommandWithWorldAndArgsDelegate::CreateStatic(&RangeAICommand));
FAutoConsoleCommandWithWorldAndArgs GRangeClearCommand(TEXT("ow.RangeClear"),
    TEXT("ow.RangeClear: remove the range actors"),
    FConsoleCommandWithWorldAndArgsDelegate::CreateStatic(&RangeClearCommand));
FAutoConsoleCommandWithWorldAndArgs GRangeReportCommand(TEXT("ow.RangeReport"),
    TEXT("ow.RangeReport: log the state of every range actor"),
    FConsoleCommandWithWorldAndArgsDelegate::CreateStatic(&RangeReportCommand));
FAutoConsoleCommandWithWorldAndArgs GRangeLootCommand(TEXT("ow.RangeLoot"),
    TEXT("ow.RangeLoot normal|always|off: loot on death (always = every gun, money and ammo pool is tried)"),
    FConsoleCommandWithWorldAndArgsDelegate::CreateStatic(&RangeLootCommand));
}
