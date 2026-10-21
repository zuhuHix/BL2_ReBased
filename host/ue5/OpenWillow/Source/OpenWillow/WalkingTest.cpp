#include "Misc/AutomationTest.h"
#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowWalker.h"
#include "OpenWillowCollision.h"
#include "EngineUtils.h"
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "Engine/StaticMesh.h"
#include "Engine/StaticMeshActor.h"
#include "Components/CapsuleComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Dom/JsonObject.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "GameFramework/PlayerController.h"
#include "HAL/PlatformTime.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "PhysicsEngine/BodySetup.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "UnrealClient.h"

class FOpenWillowWalkingCheck : public IAutomationLatentCommand
{
public:
    explicit FOpenWillowWalkingCheck(FAutomationTestBase* InTest) : Test(InTest), Started(FPlatformTime::Seconds()) {}
    bool Update() override
    {
        UWorld* World = nullptr;
        for (const auto& C : GEngine->GetWorldContexts()) if (C.WorldType == EWorldType::Game) World = C.World();
        auto* PC = World ? World->GetFirstPlayerController() : nullptr;
        auto* Pawn = PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr;
        if (FPlatformTime::Seconds() - Started > 100) { Test->AddError(TEXT("Walking test timed out")); Cleanup(); return true; }
        if (!Pawn) return false;
        auto* Move = Pawn->GetCharacterMovement();
        const double Time = World->GetTimeSeconds();
        if (Stage == 0)
        {
            if (FPlatformTime::Seconds() - Started < 8) return false;
            Test->TestTrue(TEXT("Walking pawn possesses camera"), PC->GetViewTarget() == Pawn);
            Test->TestTrue(TEXT("Imported map supports standing at player start"), Move->IsMovingOnGround());
            Test->AddInfo(FString::Printf(TEXT("Initial walking position: %s; grounded=%d"), *Pawn->GetActorLocation().ToString(), Move->IsMovingOnGround()));
            Origin = Pawn->GetActorLocation();
            FScreenshotRequest::RequestScreenshot(TEXT("OpenWillowWalkingStart.png"), false, false);
            Saved = Pawn;
            Stage = -1; StageTime = Time;
        }
        else if (Stage == -1 && Time - StageTime > 1)
        {
            // Synthetic collision is spatially isolated from all imported geometry.
            Base = FVector(1000000, 1000000, 1000000);
            AddBox(World, Base + FVector(0,0,-25), FVector(1000,500,25));
            AddBox(World, Base + FVector(500,0,150), FVector(25,500,150));
            Pawn->SetActorLocation(Base + FVector(0,0,100), false, nullptr, ETeleportType::TeleportPhysics);
            Move->StopMovementImmediately();
            Move->SetMovementMode(MOVE_Falling);
            Stage = 1; StageTime = Time;
        }
        else if (Stage == 1 && Time - StageTime > 2)
        {
            Test->TestTrue(TEXT("Gravity lands capsule on imported-style hull"), Move->IsMovingOnGround());
            Test->TestTrue(TEXT("Capsule stands above hull top"), FMath::Abs(Pawn->GetActorLocation().Z - Base.Z - 90) < 5);
            Stage = 2; StageTime = Time;
        }
        else if (Stage == 2)
        {
            Pawn->AddMovementInput(FVector::ForwardVector, 1, true);
            if (Time - StageTime < 3) return false;
            const double X = Pawn->GetActorLocation().X - Base.X;
            Test->TestTrue(TEXT("Walking reaches and stops at wall"), X > 350 && X < 445);
            Pawn->Jump(); Stage = 3; StageTime = Time; MaxZ = Pawn->GetActorLocation().Z;
        }
        else if (Stage == 3)
        {
            MaxZ = FMath::Max(MaxZ, Pawn->GetActorLocation().Z);
            Pawn->StopJumping();
            if (Time - StageTime < 2) return false;
            Test->TestTrue(TEXT("Jump rises above standing height"), MaxZ > Base.Z + 120);
            Test->TestTrue(TEXT("Jump lands on floor"), Move->IsMovingOnGround());
            Pawn->SetActorLocation(Base + FVector(-950,0,90), false, nullptr, ETeleportType::TeleportPhysics);
            Move->StopMovementImmediately(); Stage = 4; StageTime = Time;
        }
        else if (Stage == 4)
        {
            Pawn->AddMovementInput(-FVector::ForwardVector, 1, true);
            if (Time - StageTime < 2) return false;
            Test->TestTrue(TEXT("Walking off edge falls under gravity"), Move->IsFalling() && Pawn->GetActorLocation().Z < Base.Z - 100);
            Base += FVector(0, 2000, 0);
            AddBox(World, Base, FVector(1000,500,25));
            Actors.Last()->SetActorRotation(FRotator(20,0,0));
            Pawn->SetActorLocation(Base + FVector(0,0,150), false, nullptr, ETeleportType::TeleportPhysics);
            Move->StopMovementImmediately(); Move->SetMovementMode(MOVE_Falling);
            Stage = 5; StageTime = Time;
        }
        else if (Stage == 5 && Time - StageTime > 2)
        {
            Test->TestTrue(TEXT("Capsule stands on a 20 degree slope"), Move->IsMovingOnGround());
            SlopeStart = Pawn->GetActorLocation(); Stage = 6; StageTime = Time;
        }
        else if (Stage == 6)
        {
            Pawn->AddMovementInput(FVector::ForwardVector, 1, true);
            if (Time - StageTime < 1) return false;
            Test->TestTrue(TEXT("Walking climbs the slope"), Move->IsMovingOnGround() && Pawn->GetActorLocation().Z > SlopeStart.Z + 50);
            Cleanup(); return true;
        }
        return false;
    }
private:
    void AddBox(UWorld* World, FVector Center, FVector Extent)
    {
        auto* Mesh = NewObject<UStaticMesh>(World);
        FOpenWillowHull Hull;
        for (int X : {-1,1}) for (int Y : {-1,1}) for (int Z : {-1,1})
            Hull.Vertices.Add(FVector(X*Extent.X,Y*Extent.Y,Z*Extent.Z));
        Test->TestTrue(TEXT("Synthetic hull cooks"), UOpenWillowCollision::SetHulls(Mesh, {Hull}));
        auto* Actor = World->SpawnActor<AStaticMeshActor>(Center, FRotator::ZeroRotator);
        auto* Component = Actor->GetStaticMeshComponent();
        Component->SetMobility(EComponentMobility::Movable);
        Component->SetStaticMesh(Mesh);
        Component->SetCollisionProfileName(TEXT("BlockAll"));
        Component->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
        Actors.Add(Actor);
    }
    void Cleanup()
    {
        if (Saved.IsValid())
        {
            Saved->SetActorLocation(Origin, false, nullptr, ETeleportType::TeleportPhysics);
            Saved->GetCharacterMovement()->StopMovementImmediately();
        }
        for (auto Actor : Actors) if (Actor.IsValid()) Actor->Destroy();
        Actors.Empty();
    }
    FAutomationTestBase* Test;
    double Started, StageTime = 0, MaxZ = 0;
    int Stage = 0;
    FVector Base, Origin, SlopeStart;
    TWeakObjectPtr<AOpenWillowWalker> Saved;
    TArray<TWeakObjectPtr<AStaticMeshActor>> Actors;
};
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowWalkingTest, "OpenWillow.Walking", EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)
bool FOpenWillowWalkingTest::RunTest(const FString& Parameters)
{
    ADD_LATENT_AUTOMATION_COMMAND(FOpenWillowWalkingCheck(this));
    return true;
}
// Route probes recorded in the running game (tools/walking_routes.py, walk-routes.json in
// the prepared scene; the file is game data and stays local). Each probe is replayed with
// the same steering rule the capture used: face the current waypoint, full forward input,
// advance at the recorded end radius. Where the original game reached the end, the host
// walker must reach it as well, never drop more than FallMargin below the lowest feet height
// the original passed through, and finish within HeightMargin of its end height. Probes the
// original did not finish are reported, not asserted. Then every built blocking volume must
// stop a pawn-sized sweep while a line trace passes through it. Passing shows the host
// collision agrees with these recorded walks; it says nothing about unrecorded routes.
class FOpenWillowRouteCheck : public IAutomationLatentCommand
{
public:
    explicit FOpenWillowRouteCheck(FAutomationTestBase* InTest) : Test(InTest), Started(FPlatformTime::Seconds()) {}
    bool Update() override
    {
        UWorld* World = nullptr;
        for (const auto& C : GEngine->GetWorldContexts()) if (C.WorldType == EWorldType::Game) World = C.World();
        auto* PC = World ? World->GetFirstPlayerController() : nullptr;
        auto* Pawn = PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr;
        if (FPlatformTime::Seconds() - Started > 900) { Test->AddError(TEXT("Route walking timed out")); return true; }
        if (!Pawn) return false;
        auto* Move = Pawn->GetCharacterMovement();
        const double Time = World->GetTimeSeconds();
        if (Stage == 0)
        {
            if (FPlatformTime::Seconds() - Started < 8) return false;
            if (!Load()) return true;
            Origin = Pawn->GetActorLocation();
            auto* Capsule = Pawn->GetCapsuleComponent();
            Test->AddInfo(FString::Printf(TEXT("Host walker: radius %.1f half-height %.1f step %.1f walkable Z %.3f speed %.0f; original: %s"),
                Capsule->GetUnscaledCapsuleRadius(), Capsule->GetUnscaledCapsuleHalfHeight(), Move->MaxStepHeight,
                Move->GetWalkableFloorZ(), Move->MaxWalkSpeed, *CaptureSummary));
            // Replay with the original pawn's cylinder and movement limits, so the routes test the
            // collision geometry rather than the walker's placeholder settings; restored at the end.
            Saved = {Capsule->GetUnscaledCapsuleRadius(), Capsule->GetUnscaledCapsuleHalfHeight(), Move->MaxStepHeight,
                Move->GetWalkableFloorZ(), Move->MaxWalkSpeed};
            if (Captured.Num() == 5)
            {
                Capsule->SetCapsuleSize(float(Captured[0]), float(Captured[1]));
                Move->MaxStepHeight = float(Captured[2]); Move->SetWalkableFloorZ(float(Captured[3])); Move->MaxWalkSpeed = float(Captured[4]);
            }
            Stage = 1; Route = 0;
        }
        if (Stage == 1)
        {
            if (Route >= Routes.Num()) { Stage = 4; return false; }
            const FRoute& R = Routes[Route];
            FVector Facing = R.Points[1] - R.Points[0]; Facing.Z = 0;
            Pawn->SetActorLocation(R.Points[0], false, nullptr, ETeleportType::TeleportPhysics);
            PC->SetControlRotation(Facing.Rotation());
            Move->StopMovementImmediately(); Move->SetMovementMode(MOVE_Falling);
            Waypoint = 1; StageTime = Time; Stage = 2;
            return false;
        }
        const double FeetZ = Pawn->GetActorLocation().Z - Pawn->GetCapsuleComponent()->GetScaledCapsuleHalfHeight();
        if (Stage == 2)
        {
            if (Time - StageTime < 2.5) return false;
            MinFeet = MinGround = TNumericLimits<double>::Max(); MaxFeet = FeetZ;
            MinPos = Best = Pawn->GetActorLocation(); BestTime = Time;
            StageTime = Time; Stage = 3;
        }
        if (Stage == 3)
        {
            const FRoute& R = Routes[Route];
            const FVector P = Pawn->GetActorLocation();
            const double GroundZ = Move->IsMovingOnGround() ? GroundUnder(World, Pawn, P, FeetZ) : FeetZ;
            // Count from where the original first stood on the ground (see walking_routes.py summarize).
            if (FVector::Dist2D(P, R.Points[0]) >= R.LandingDistance - 10)
            {
                if (GroundZ < MinGround) { MinGround = GroundZ; MinPos = P; }
                MinFeet = FMath::Min(MinFeet, FeetZ);
            }
            MaxFeet = FMath::Max(MaxFeet, FeetZ);
            while (Waypoint < R.Points.Num() - 1 && FVector::Dist2D(P, R.Points[Waypoint]) <= EndRadius) ++Waypoint;
            const bool Reached = Waypoint == R.Points.Num() - 1 && FVector::Dist2D(P, R.Points[Waypoint]) <= EndRadius;
            if (FVector::Dist2D(P, Best) > 10) { Best = P; BestTime = Time; }
            const bool Stuck = Time - BestTime > 2, Late = Time - StageTime > R.Limit;
            if (!Reached && !Stuck && !Late)
            {
                FVector Direction = R.Points[Waypoint] - P; Direction.Z = 0;
                PC->SetControlRotation(Direction.Rotation());
                Pawn->AddMovementInput(Direction.GetSafeNormal(), 1, true);
                return false;
            }
            Move->StopMovementImmediately();
            // Compare the ground under the centre: a UE3 cylinder and a UE5 capsule stand at
            // different feet heights on a slope (see tools/walking_routes.py ground()).
            const double EndGround = Move->IsMovingOnGround() ? GroundUnder(World, Pawn, P, FeetZ) : FeetZ;
            const bool Fell = MinGround < R.MinGround - FallMargin;
            const bool Height = FMath::Abs(EndGround - R.FinalGround) <= HeightMargin;
            Test->AddInfo(FString::Printf(TEXT("Route %s: original reached=%d end ground %.1f min ground %.1f (feet min %.1f); host reached=%d stuck=%d late=%d end ground %.1f min ground %.1f (feet min %.1f max %.1f) in %.1f s; host lowest at %s on %s"),
                *R.Name, R.Reached, R.FinalGround, R.MinGround, R.MinFeet, Reached, Stuck, Late, EndGround, MinGround, MinFeet, MaxFeet, Time - StageTime,
                *MinPos.ToString(), *FloorName(World, Pawn, MinPos)));
            if (R.Reached)
            {
                ++Asserted;
                const bool Pass = Reached && !Fell && Height;
                Passed += Pass;
                Test->TestTrue(*FString::Printf(TEXT("%s: host walker follows the recorded route (reached, no fall, end height)"), *R.Name), Pass);
                if (!Pass && Failed++ < 6)
                    FScreenshotRequest::RequestScreenshot(FString::Printf(TEXT("OpenWillowRoute_%s.png"), *R.Name), false, false);
            }
            else { ++NotAsserted; HostReachedUnfinished += Reached; }
            ++Route; Stage = 1;
            return false;
        }
        if (Stage == 4)
        {
            if (Saved.Num() == 5)
            {
                Pawn->GetCapsuleComponent()->SetCapsuleSize(float(Saved[0]), float(Saved[1]));
                Move->MaxStepHeight = float(Saved[2]); Move->SetWalkableFloorZ(float(Saved[3])); Move->MaxWalkSpeed = float(Saved[4]);
            }
            Pawn->SetActorLocation(Origin, false, nullptr, ETeleportType::TeleportPhysics);
            Move->StopMovementImmediately();
            CheckVolumes(World, Pawn);
            Test->AddInfo(FString::Printf(TEXT("Route summary: passed=%d/%d original_unfinished=%d (host reached %d of them) volumes_blocking=%d/%d"),
                Passed, Asserted, NotAsserted, HostReachedUnfinished, VolumesPassed, VolumesTotal));
            return true;
        }
        return false;
    }
private:
    struct FRoute { FString Name; TArray<FVector> Points; bool Reached = false; double MinFeet = 0, MinGround = 0, FinalGround = 0, LandingDistance = 0, Limit = 10; };
    static FVector Vec(const TArray<TSharedPtr<FJsonValue>>& V) { return FVector(V[0]->AsNumber(), V[1]->AsNumber(), V[2]->AsNumber()); }
    bool Load()
    {
        FString Text;
        TSharedPtr<FJsonObject> Json;
        const FString Root = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_SCENE"));
        if (!FFileHelper::LoadFileToString(Text, *FPaths::Combine(Root, TEXT("walk-routes.json")))
            || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Json) || !Json.IsValid()
            || Json->GetIntegerField(TEXT("schema")) != 1)
        { Test->AddError(TEXT("Missing walk-routes.json (tools/walking_routes.py routes)")); return false; }
        EndRadius = Json->GetNumberField(TEXT("end_radius"));
        const TSharedPtr<FJsonObject>* Info = nullptr;
        if (Json->TryGetObjectField(TEXT("capture"), Info) && Info->IsValid() && (*Info)->HasField(TEXT("cylinder")))
        {
            const auto Cyl = (*Info)->GetObjectField(TEXT("cylinder"));
            Captured = {Cyl->GetNumberField(TEXT("radius")), Cyl->GetNumberField(TEXT("height")),
                (*Info)->GetNumberField(TEXT("MaxStepHeight")), (*Info)->GetNumberField(TEXT("WalkableFloorZ")),
                (*Info)->GetNumberField(TEXT("GroundSpeed"))};
            CaptureSummary = FString::Printf(TEXT("radius %.1f half-height %.1f step %.1f walkable Z %.3f speed %.0f"),
                Cyl->GetNumberField(TEXT("radius")), Cyl->GetNumberField(TEXT("height")), (*Info)->GetNumberField(TEXT("MaxStepHeight")),
                (*Info)->GetNumberField(TEXT("WalkableFloorZ")), (*Info)->GetNumberField(TEXT("GroundSpeed")));
        }
        for (const auto& Value : Json->GetArrayField(TEXT("routes")))
        {
            const auto R = Value->AsObject();
            const auto Original = R->GetObjectField(TEXT("original"));
            FRoute Entry;
            Entry.Name = R->GetStringField(TEXT("name"));
            for (const auto& P : R->GetArrayField(TEXT("points"))) Entry.Points.Add(Vec(P->AsArray()));
            if (Entry.Points.Num() < 2) continue;
            Entry.Reached = Original->GetBoolField(TEXT("reached"));
            Entry.MinFeet = Original->GetNumberField(TEXT("min_feet"));
            Entry.MinGround = Original->GetNumberField(TEXT("min_ground"));
            Entry.LandingDistance = Original->GetNumberField(TEXT("landing_distance"));
            Entry.FinalGround = Original->GetNumberField(TEXT("final_ground"));
            // Allow for a host walk slower than the original's.
            Entry.Limit = Original->GetNumberField(TEXT("duration")) * 2 + 4;
            Routes.Add(MoveTemp(Entry));
        }
        // A partial manifest (interrupted capture, edited file) must not pass with fewer routes.
        int32 Expected = -1;
        if (!Json->TryGetNumberField(TEXT("expected_routes"), Expected) || Expected != Routes.Num() || Expected < 1)
        {
            Test->AddError(FString::Printf(TEXT("walk-routes.json lists %d usable routes, expected_routes %d"), Routes.Num(), Expected));
            return false;
        }
        Test->AddInfo(FString::Printf(TEXT("Route probes: %d"), Routes.Num()));
        return true;
    }
    static double GroundUnder(UWorld* World, AOpenWillowWalker* Pawn, const FVector& Centre, double Fallback)
    {
        FHitResult Hit;
        FCollisionQueryParams Params(SCENE_QUERY_STAT(OpenWillowRouteGround), true, Pawn);
        return World->LineTraceSingleByChannel(Hit, Centre, Centre - FVector(0, 0, 300), ECC_Pawn, Params) ? Hit.ImpactPoint.Z : Fallback;
    }
    static FString FloorName(UWorld* World, AOpenWillowWalker* Pawn, const FVector& At)
    {
        FHitResult Hit;
        FCollisionQueryParams Params(SCENE_QUERY_STAT(OpenWillowRouteFloor), true, Pawn);
        if (!World->LineTraceSingleByChannel(Hit, At, At - FVector(0, 0, 300), ECC_Pawn, Params) || !Hit.GetComponent())
            return TEXT("nothing");
        const AActor* Actor = Hit.GetActor();
        const FString Label = Actor && Actor->Tags.Num() ? Actor->Tags[0].ToString() : Actor ? Actor->GetName() : TEXT("?");
        return FString::Printf(TEXT("%s (%s) at Z %.1f"), *Label, *Hit.GetComponent()->GetName(), Hit.ImpactPoint.Z);
    }
    void CheckVolumes(UWorld* World, AOpenWillowWalker* Pawn)
    {
        // The count to check comes from the same volumes-runtime.json the volumes actor reads.
        FString Text;
        TSharedPtr<FJsonObject> Json;
        const FString Root = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_SCENE"));
        const int32 Expected = FFileHelper::LoadFileToString(Text, *FPaths::Combine(Root, TEXT("volumes-runtime.json")))
            && FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Json) && Json.IsValid()
            ? Json->GetArrayField(TEXT("volumes")).Num() : -1;
        for (TActorIterator<AOpenWillowVolumes> It(World); It; ++It)
            for (UActorComponent* C : It->GetComponents())
            {
                auto* Volume = Cast<UStaticMeshComponent>(C);
                if (!Volume || Volume->ComponentTags.IsEmpty()) continue;
                ++VolumesTotal;
                // Test this component alone (other geometry may cover it): a pawn capsule swept
                // down onto it must hit, and its channel responses must block pawns but not traces.
                // Render bounds are empty for these collision-only meshes; use the collision box.
                const UBodySetup* Body = Volume->GetStaticMesh() ? Volume->GetStaticMesh()->GetBodySetup() : nullptr;
                if (!Body) { Test->AddError(TEXT("Volume without body setup")); continue; }
                const FBox Box = Body->AggGeom.CalcAABB(Volume->GetComponentTransform());
                const FVector Above = Box.GetCenter() + FVector(0, 0, Box.GetExtent().Z + 200), Centre = Box.GetCenter();
                const FCollisionShape Shape = FCollisionShape::MakeCapsule(Pawn->GetCapsuleComponent()->GetScaledCapsuleRadius(),
                    Pawn->GetCapsuleComponent()->GetScaledCapsuleHalfHeight());
                FHitResult Hit;
                const bool Blocks = Volume->SweepComponent(Hit, Above, Centre, FQuat::Identity, Shape)
                    && Volume->GetCollisionResponseToChannel(ECC_Pawn) == ECR_Block;
                const bool TracePasses = Volume->GetCollisionResponseToChannel(ECC_Visibility) == ECR_Ignore;
                VolumesPassed += Blocks && TracePasses;
                Test->TestTrue(*FString::Printf(TEXT("%s blocks a pawn sweep and lets a line trace through"), *Volume->ComponentTags[0].ToString()),
                    Blocks && TracePasses);
            }
        if (Expected < 0) Test->AddError(TEXT("Missing volumes-runtime.json (tools/refresh_collision.py)"));
        else if (Expected == 0) Test->AddWarning(TEXT("volumes-runtime.json lists no blocking volumes; none checked"));
        Test->TestEqual(TEXT("Every blocking volume in volumes-runtime.json was built and checked"), VolumesTotal, FMath::Max(Expected, 0));
    }
    FAutomationTestBase* Test;
    double Started, StageTime = 0, BestTime = 0, MinFeet = 0, MaxFeet = 0, MinGround = 0, EndRadius = 40;
    static constexpr double FallMargin = 30, HeightMargin = 30;
    int Stage = 0, Route = 0, Waypoint = 1, Passed = 0, Asserted = 0, NotAsserted = 0, HostReachedUnfinished = 0, Failed = 0;
    int VolumesTotal = 0, VolumesPassed = 0;
    FVector Origin, Best, MinPos;
    FString CaptureSummary = TEXT("not recorded");
    TArray<double> Captured, Saved;  // radius, half-height, step, walkable Z, speed
    TArray<FRoute> Routes;
};
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowRouteWalkingTest, "OpenWillow.RouteWalking", EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)
bool FOpenWillowRouteWalkingTest::RunTest(const FString& Parameters)
{
    ADD_LATENT_AUTOMATION_COMMAND(FOpenWillowRouteCheck(this));
    return true;
}
// Sphere and capsule elements cook, are counted back, and a scaled capsule frame is refused.
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowCollisionShapesTest, "OpenWillow.CollisionShapes", EAutomationTestFlags::EditorContext | EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)
bool FOpenWillowCollisionShapesTest::RunTest(const FString& Parameters)
{
    auto* Mesh = NewObject<UStaticMesh>(GetTransientPackage());
    FOpenWillowSphere Sphere; Sphere.Center = FVector(1, 2, 3); Sphere.Radius = 10;
    FOpenWillowCapsule Capsule; Capsule.Center = FVector(4, 5, 6); Capsule.Radius = 6; Capsule.Length = 168;
    Capsule.AxisX = FVector(0, 0, 1); Capsule.AxisY = FVector(0, 1, 0); Capsule.AxisZ = FVector(-1, 0, 0);
    TestTrue(TEXT("Sphere and capsule cook"), UOpenWillowCollision::SetShapes(Mesh, {}, {Sphere}, {Capsule}));
    TestEqual(TEXT("One sphere"), UOpenWillowCollision::CountSpheres(Mesh), 1);
    TestEqual(TEXT("One capsule"), UOpenWillowCollision::CountCapsules(Mesh), 1);
    const FKSphylElem& Elem = Mesh->GetBodySetup()->AggGeom.SphylElems[0];
    TestTrue(TEXT("Capsule long axis follows the source Z row"),
        Elem.Rotation.RotateVector(FVector::ZAxisVector).Equals(FVector(-1, 0, 0), 1e-3));
    Capsule.AxisX *= 2;
    TestFalse(TEXT("Scaled capsule frame is refused"), UOpenWillowCollision::SetShapes(Mesh, {}, {}, {Capsule}));
    return true;
}
#endif
