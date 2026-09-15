#include "Misc/AutomationTest.h"
#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowWalker.h"
#include "Dom/JsonObject.h"
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "GameFramework/PlayerController.h"
#include "HAL/PlatformTime.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "UnrealClient.h"

// Runtime check of imported terrain floors, separate from the saved-asset
// verification scripts. It reads terrain-runtime.json from the prepared scene:
// for every terrain, stand on a corroborated cell, confirm a flagged hole cell
// does not carry this floor, and walk across a component seam when one exists.
// Passing shows the host collision behaves as the decoded topology predicts;
// it does not establish original-game parity.
class FOpenWillowTerrainCheck : public IAutomationLatentCommand
{
public:
    explicit FOpenWillowTerrainCheck(FAutomationTestBase* InTest) : Test(InTest), Started(FPlatformTime::Seconds()) {}

    bool Update() override
    {
        UWorld* World = nullptr;
        for (const auto& C : GEngine->GetWorldContexts()) if (C.WorldType == EWorldType::Game) World = C.World();
        auto* PC = World ? World->GetFirstPlayerController() : nullptr;
        auto* Pawn = PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr;
        if (FPlatformTime::Seconds() - Started > 280) { Test->AddError(TEXT("Terrain walking test timed out")); return true; }
        if (!Pawn) return false;
        auto* Move = Pawn->GetCharacterMovement();
        const double Time = World->GetTimeSeconds();
        if (Stage == 0)
        {
            if (FPlatformTime::Seconds() - Started < 8) return false;
            if (!Load()) return true;
            Origin = Pawn->GetActorLocation();
            Test->AddInfo(FString::Printf(TEXT("Terrain probes: %d"), Probes.Num()));
            Stage = 1; Probe = 0; Step = 0; StageTime = Time;
            return false;
        }
        if (Probe >= Probes.Num())
        {
            Pawn->SetActorLocation(Origin, false, nullptr, ETeleportType::TeleportPhysics);
            Move->StopMovementImmediately();
            Test->AddInfo(FString::Printf(TEXT("Terrain runtime summary: stand=%d/%d occluded_stands=%d hole=%d/%d hole_on_other=%d seam=%d/%d skipped_seams=%d"),
                StandPassed, StandTotal, StandOccluded, HolePassed, HoleTotal, HoleOnOther, SeamPassed, SeamTotal, SeamSkipped));
            return true;
        }
        const TSharedPtr<FJsonObject>& P = Probes[Probe];
        const FString Source = P->GetStringField(TEXT("source"));
        if (Step == 0)
        {
            Stands.Reset();
            if (P->HasTypedField<EJson::Array>(TEXT("stands")))
                for (const auto& Value : P->GetArrayField(TEXT("stands"))) Stands.Add(Value->AsObject());
            if (Stands.IsEmpty()) Stands.Add(P->GetObjectField(TEXT("stand")));
            Candidate = 0;
            Teleport(Pawn, Point(Stands[0], TEXT("point")));
            Step = 1; StageTime = Time; return false;
        }
        if (Step == 1 && Time - StageTime > 2.5)
        {
            const TSharedPtr<FJsonObject> Stand = Stands[Candidate];
            FString Hit; FVector Where;
            const bool OnTerrain = TraceDown(World, Pawn->GetActorLocation(), Hit, Where);
            const bool Grounded = Move->IsMovingOnGround();
            const double FeetZ = Pawn->GetActorLocation().Z - 88;
            const bool Rests = Grounded && OnTerrain && WithinSurface(Stand, FeetZ);
            // Other imported geometry (building floors, props) may carry the pawn
            // above this cell; that neither confirms nor refutes the terrain floor.
            const bool Occluded = !Rests && Grounded && !OnTerrain && FeetZ > Stand->GetArrayField(TEXT("surface"))[1]->AsNumber() + 40;
            Test->AddInfo(FString::Printf(TEXT("%s stand[%d]: grounded=%d hit=%s pawn=%s%s"), *Source, Candidate, Grounded, *Hit,
                *Pawn->GetActorLocation().ToString(), Occluded ? TEXT(" (occluded by other geometry)") : TEXT("")));
            if (Occluded && Candidate + 1 < Stands.Num())
            {
                ++Candidate;
                Teleport(Pawn, Point(Stands[Candidate], TEXT("point")));
                StageTime = Time; return false;
            }
            ++StandTotal; StandPassed += Rests; StandOccluded += Occluded;
            if (!Occluded) Test->TestTrue(*FString::Printf(TEXT("%s: pawn stands on the imported terrain floor"), *Source), Rests);
            else Test->AddWarning(FString::Printf(TEXT("%s: every stand candidate is covered by other geometry; floor unverified at runtime"), *Source));
            if (Probe == 0) FScreenshotRequest::RequestScreenshot(TEXT("OpenWillowTerrainStand.png"), false, false);
            if (P->HasTypedField<EJson::Object>(TEXT("hole")))
            {
                Teleport(Pawn, Point(P->GetObjectField(TEXT("hole")), TEXT("point")));
                Step = 2; StageTime = Time;
                LogHolePath(World, Pawn, Source, 0);
            }
            else Step = 3;
            return false;
        }
        if (Step == 2) LogHolePath(World, Pawn, Source, Time - StageTime);
        if (Step == 2 && Time - StageTime > 2.5)
        {
            const TSharedPtr<FJsonObject> Hole = P->GetObjectField(TEXT("hole"));
            FString Hit; FVector Where;
            const bool OnTerrain = TraceDown(World, Pawn->GetActorLocation(), Hit, Where);
            const bool RestsOnTerrainAtHole = Move->IsMovingOnGround() && OnTerrain && WithinSurface(Hole, Pawn->GetActorLocation().Z - 88);
            ++HoleTotal; HolePassed += !RestsOnTerrainAtHole;
            HoleOnOther += Move->IsMovingOnGround() && !OnTerrain;
            Test->TestFalse(*FString::Printf(TEXT("%s: flagged hole cell carries no terrain floor"), *Source), RestsOnTerrainAtHole);
            // Report displacement separately; the per-frame path records contact
            // and movement evidence without assigning a cause from distance alone.
            Test->AddInfo(FString::Printf(TEXT("%s hole: falling=%d hit=%s pawn=%s drift=%.0f"), *Source, Move->IsFalling(), *Hit,
                *Pawn->GetActorLocation().ToString(), FVector::Dist2D(Pawn->GetActorLocation(), Point(Hole, TEXT("point")))));
            Step = 3; return false;
        }
        if (Step == 3)
        {
            if (!P->HasTypedField<EJson::Object>(TEXT("seam"))) { NextProbe(); return false; }
            const TSharedPtr<FJsonObject> Seam = P->GetObjectField(TEXT("seam"));
            const TArray<TSharedPtr<FJsonValue>>& Surface = Seam->GetArrayField(TEXT("surface"));
            if (Surface[1]->AsNumber() - Surface[0]->AsNumber() > 300)
            {
                ++SeamSkipped;
                Test->AddInfo(FString::Printf(TEXT("%s seam: skipped, surface range %.0f cm is not walkable"), *Source, Surface[1]->AsNumber() - Surface[0]->AsNumber()));
                NextProbe(); return false;
            }
            SeamEnd = Point(Seam, TEXT("end"));
            Teleport(Pawn, Point(Seam, TEXT("start")));
            Step = 4; StageTime = Time; return false;
        }
        if (Step == 4 && Time - StageTime > 2)
        {
            FString Hit; FVector Where;
            TraceDown(World, Pawn->GetActorLocation(), SeamStartHit, Where);
            Step = 5; StageTime = Time; return false;
        }
        if (Step == 5)
        {
            FVector Direction = SeamEnd - Pawn->GetActorLocation(); Direction.Z = 0;
            Pawn->AddMovementInput(Direction.GetSafeNormal(), 1, true);
            if (Time - StageTime < 2.5 && FVector::Dist2D(Pawn->GetActorLocation(), SeamEnd) > 40) return false;
            FString Hit; FVector Where;
            const bool OnTerrain = TraceDown(World, Pawn->GetActorLocation(), Hit, Where);
            const bool Crossed = Move->IsMovingOnGround() && OnTerrain && Hit != SeamStartHit && FVector::Dist2D(Pawn->GetActorLocation(), SeamEnd) < 80;
            ++SeamTotal; SeamPassed += Crossed;
            Test->TestTrue(*FString::Printf(TEXT("%s: pawn walks across a component seam"), *Source), Crossed);
            Test->AddInfo(FString::Printf(TEXT("%s seam: from=%s to=%s grounded=%d pawn=%s"), *Source, *SeamStartHit, *Hit, Move->IsMovingOnGround(), *Pawn->GetActorLocation().ToString()));
            NextProbe(); return false;
        }
        return false;
    }
private:
    void LogHolePath(UWorld* World, AOpenWillowWalker* Pawn, const FString& Source, double Elapsed)
    {
        auto* Move = Pawn->GetCharacterMovement();
        FHitResult Hit;
        FCollisionQueryParams Params(SCENE_QUERY_STAT(OpenWillowTerrainPath), true, Pawn);
        const FVector Position = Pawn->GetActorLocation();
        World->LineTraceSingleByChannel(Hit, Position, Position - FVector(0, 0, 2000), ECC_Visibility, Params);
        const auto Label = [](const AActor* Actor) -> FString
        {
            return !Actor ? TEXT("none") : Actor->Tags.Num() > 0 ? Actor->Tags[0].ToString() : Actor->GetName();
        };
        const FHitResult& Floor = Move->CurrentFloor.HitResult;
        Test->AddInfo(FString::Printf(TEXT("Terrain hole path: %s t=%.4f dt=%.4f pawn=%s velocity=%s input=%s mode=%d floor=%s floor_normal=%s floor_penetrating=%d trace=%s trace_point=%s trace_normal=%s trace_penetrating=%d"),
            *Source, Elapsed, World->GetDeltaSeconds(), *Position.ToString(), *Move->Velocity.ToString(),
            *Pawn->GetLastMovementInputVector().ToString(), static_cast<int>(Move->MovementMode),
            *Label(Floor.GetActor()), *Floor.ImpactNormal.ToString(), Floor.bStartPenetrating,
            *Label(Hit.GetActor()), *Hit.ImpactPoint.ToString(), *Hit.ImpactNormal.ToString(), Hit.bStartPenetrating));
    }
    bool Load()
    {
        const FString Root = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_SCENE"));
        FString Text;
        if (Root.IsEmpty() || !FFileHelper::LoadFileToString(Text, *FPaths::Combine(Root, TEXT("terrain-runtime.json"))))
        {
            Test->AddError(TEXT("OPENWILLOW_SCENE must point at a scene with terrain-runtime.json")); return false;
        }
        TSharedPtr<FJsonObject> Json;
        if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Json) || !Json.IsValid()
            || !Json->GetBoolField(TEXT("collision")))
        {
            Test->AddError(TEXT("terrain-runtime.json is invalid or was prepared without --collision")); return false;
        }
        for (const auto& Value : Json->GetArrayField(TEXT("probes")))
        {
            const TSharedPtr<FJsonObject>* Object;
            if (Value->TryGetObject(Object)) Probes.Add(*Object);
        }
        if (Probes.IsEmpty()) { Test->AddError(TEXT("No terrain probes")); return false; }
        return true;
    }
    static FVector Point(const TSharedPtr<FJsonObject>& Object, const TCHAR* Field)
    {
        const auto& Values = Object->GetArrayField(Field);
        return FVector(Values[0]->AsNumber(), Values[1]->AsNumber(), Values[2]->AsNumber());
    }
    static bool WithinSurface(const TSharedPtr<FJsonObject>& Object, double FeetZ)
    {
        const auto& Range = Object->GetArrayField(TEXT("surface"));
        // Capsule feet may sit up to the step height above a sloped cell.
        return FeetZ > Range[0]->AsNumber() - 20 && FeetZ < Range[1]->AsNumber() + 40;
    }
    void Teleport(AOpenWillowWalker* Pawn, const FVector& Location)
    {
        Pawn->SetActorLocation(Location, false, nullptr, ETeleportType::TeleportPhysics);
        Pawn->GetCharacterMovement()->StopMovementImmediately();
        Pawn->GetCharacterMovement()->SetMovementMode(MOVE_Falling);
    }
    static bool TraceDown(UWorld* World, const FVector& From, FString& Label, FVector& Where)
    {
        FHitResult Hit;
        FCollisionQueryParams Params(SCENE_QUERY_STAT(OpenWillowTerrain), true);
        Label = TEXT("none");
        if (!World->LineTraceSingleByChannel(Hit, From, From - FVector(0, 0, 400), ECC_Visibility, Params)) return false;
        Where = Hit.ImpactPoint;
        const AActor* Actor = Hit.GetActor();
        // Imported actors carry their source object path as the first tag.
        Label = !Actor ? TEXT("unknown") : Actor->Tags.Num() > 0 ? Actor->Tags[0].ToString() : Actor->GetName();
        return Actor && Actor->Tags.Num() > 0 && Actor->Tags[0].ToString().Contains(TEXT("TerrainComponent"));
    }
    void NextProbe() { ++Probe; Step = 0; }
    FAutomationTestBase* Test;
    double Started, StageTime = 0;
    int Stage = 0, Probe = 0, Step = 0, Candidate = 0;
    int StandPassed = 0, StandTotal = 0, StandOccluded = 0, HolePassed = 0, HoleTotal = 0, HoleOnOther = 0;
    int SeamPassed = 0, SeamTotal = 0, SeamSkipped = 0;
    TArray<TSharedPtr<FJsonObject>> Stands;
    FVector Origin, SeamEnd;
    FString SeamStartHit;
    TArray<TSharedPtr<FJsonObject>> Probes;
};
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowTerrainWalkingTest, "OpenWillow.TerrainWalking", EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)
bool FOpenWillowTerrainWalkingTest::RunTest(const FString& Parameters)
{
    ADD_LATENT_AUTOMATION_COMMAND(FOpenWillowTerrainCheck(this));
    return true;
}
#endif
