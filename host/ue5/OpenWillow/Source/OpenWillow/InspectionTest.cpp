#include "Misc/AutomationTest.h"
#if WITH_DEV_AUTOMATION_TESTS
#include "Camera/PlayerCameraManager.h"
#include "Dom/JsonObject.h"
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "GameFramework/Pawn.h"
#include "GameFramework/PlayerController.h"
#include "GameFramework/PawnMovementComponent.h"
#include "HAL/PlatformTime.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "UnrealClient.h"

// Repeatable host viewpoints for visual investigations. This checks capture
// positions only; comparison with original-game images remains a human gate.
class FOpenWillowInspectionCheck : public IAutomationLatentCommand
{
public:
    explicit FOpenWillowInspectionCheck(FAutomationTestBase* InTest)
        : Test(InTest), Started(FPlatformTime::Seconds()) {}

    bool Update() override
    {
        const double Now = FPlatformTime::Seconds();
        if (Now - Started > 240)
        { Test->AddError(TEXT("Inspection capture timed out")); return true; }
        UWorld* World = nullptr;
        for (const auto& Context : GEngine->GetWorldContexts())
            if (Context.WorldType == EWorldType::Game) World = Context.World();
        auto* PC = World ? World->GetFirstPlayerController() : nullptr;
        APawn* Pawn = PC ? PC->GetPawn() : nullptr;
        if (!Pawn || !PC->PlayerCameraManager) return false;
        if (!Loaded)
        {
            if (Now - Started < 15) return false;
            FString Text;
            TSharedPtr<FJsonObject> Json;
            const FString Root = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_SCENE"));
            if (!FFileHelper::LoadFileToString(Text, *FPaths::Combine(Root, TEXT("inspection-views.json")))
                || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Json) || !Json.IsValid())
            { Test->AddError(TEXT("Missing or invalid inspection-views.json")); return true; }
            const TArray<TSharedPtr<FJsonValue>>* Rows = nullptr;
            if (!Json->TryGetArrayField(TEXT("views"), Rows) || Rows->Num() < 1 || Rows->Num() > 12)
            { Test->AddError(TEXT("Inspection requires 1..12 views")); return true; }
            for (const auto& Row : *Rows)
            {
                if (!Row.IsValid() || Row->Type != EJson::Object)
                { Test->AddError(TEXT("Invalid inspection view")); return true; }
                const auto Object = Row->AsObject();
                FVector Position, Rotation;
                double Fov = 75;
                if (!Vector(Object, TEXT("location"), Position)
                    || !Vector(Object, TEXT("rotation"), Rotation)
                    || !Object->TryGetNumberField(TEXT("fov"), Fov)
                    || !FMath::IsFinite(Fov) || Fov < 10 || Fov > 150)
                { Test->AddError(TEXT("View requires finite location, rotation and FOV in 10..150")); return true; }
                Positions.Add(Position); Rotations.Add(FRotator(Rotation.X, Rotation.Y, Rotation.Z)); Fovs.Add(Fov);
            }
            Origin = Pawn->GetActorLocation(); OriginalRotation = PC->GetControlRotation();
            OriginalFov = PC->PlayerCameraManager->GetFOVAngle();
            Loaded = true;
        }
        if (Index >= Positions.Num())
        {
            Pawn->SetActorLocation(Origin, false, nullptr, ETeleportType::TeleportPhysics);
            PC->SetControlRotation(OriginalRotation); PC->PlayerCameraManager->SetFOV(OriginalFov);
            Test->AddInfo(FString::Printf(TEXT("Inspection captured %d views; visual acceptance pending"), Index));
            return true;
        }
        if (Stage == 0)
        {
            if (auto* Movement = Pawn->GetMovementComponent()) Movement->StopMovementImmediately();
            Pawn->SetActorLocation(Positions[Index], false, nullptr, ETeleportType::TeleportPhysics);
            PC->SetControlRotation(Rotations[Index]); PC->PlayerCameraManager->SetFOV(Fovs[Index]);
            Stage = 1; StageStarted = Now; return false;
        }
        if (Stage == 1 && Now - StageStarted > 8)
        {
            Test->TestTrue(TEXT("Inspection camera position matches requested view"),
                PC->PlayerCameraManager->GetCameraLocation().Equals(Positions[Index], 1));
            Test->TestTrue(TEXT("Inspection camera rotation matches requested view"),
                PC->PlayerCameraManager->GetCameraRotation().Equals(Rotations[Index], .1));
            Test->TestTrue(TEXT("Inspection FOV matches requested view"),
                FMath::Abs(PC->PlayerCameraManager->GetFOVAngle() - Fovs[Index]) < .1);
            const FString Name = FPaths::Combine(FPaths::ScreenShotDir(),
                FString::Printf(TEXT("%s_inspection_%02d.png"), *World->GetMapName(), Index));
            FScreenshotRequest::RequestScreenshot(Name, false, true);
            Test->AddInfo(FString::Printf(TEXT("Inspection view %d: location=%s rotation=%s fov=%.1f screenshot=%s"),
                Index, *Positions[Index].ToString(), *Rotations[Index].ToString(), Fovs[Index], *Name));
            Stage = 2; StageStarted = Now;
        }
        else if (Stage == 2 && Now - StageStarted > 2) { ++Index; Stage = 0; }
        return false;
    }
private:
    static bool Vector(const TSharedPtr<FJsonObject>& Object, const TCHAR* Key, FVector& Out)
    {
        const TArray<TSharedPtr<FJsonValue>>* Values = nullptr;
        if (!Object->TryGetArrayField(Key, Values) || Values->Num() != 3) return false;
        double Components[3];
        for (int I = 0; I < 3; ++I)
            if (!(*Values)[I].IsValid() || !(*Values)[I]->TryGetNumber(Components[I])
                || !FMath::IsFinite(Components[I])) return false;
        Out = FVector(Components[0], Components[1], Components[2]); return true;
    }
    FAutomationTestBase* Test;
    double Started, StageStarted = 0;
    bool Loaded = false;
    int Index = 0, Stage = 0;
    FVector Origin;
    FRotator OriginalRotation;
    float OriginalFov = 75;
    TArray<FVector> Positions;
    TArray<FRotator> Rotations;
    TArray<double> Fovs;
};
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowInspectionTest, "OpenWillow.Inspection",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::ProductFilter)
bool FOpenWillowInspectionTest::RunTest(const FString& Parameters)
{ ADD_LATENT_AUTOMATION_COMMAND(FOpenWillowInspectionCheck(this)); return true; }
#endif
