#include "Misc/AutomationTest.h"
#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowWalker.h"
#include "OpenWillowCollision.h"
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "Engine/StaticMesh.h"
#include "Engine/StaticMeshActor.h"
#include "Components/StaticMeshComponent.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "GameFramework/PlayerController.h"
#include "HAL/PlatformTime.h"
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
#endif
