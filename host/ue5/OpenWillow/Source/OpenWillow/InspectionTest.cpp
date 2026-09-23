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

struct FOpenWillowInspectionPose
{
    FVector Position;
    FRotator Rotation;
    float Fov = 75;
};

// Repeatable host viewpoints for visual investigations. This checks capture
// positions and optional host target visibility; comparison with original-game
// images remains a human gate.
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
                FOpenWillowInspectionPose Base;
                if (!Pose(Object, Base))
                { Test->AddError(TEXT("View requires finite location, rotation and FOV in 10..150")); return true; }
                TArray<FOpenWillowInspectionPose> ViewCandidates;
                ViewCandidates.Add(Base);
                const TArray<TSharedPtr<FJsonValue>>* CandidateRows = nullptr;
                if (Object->TryGetArrayField(TEXT("candidates"), CandidateRows))
                {
                    if (CandidateRows->Num() < 1 || CandidateRows->Num() > 8)
                    { Test->AddError(TEXT("Inspection candidates require 1..8 poses")); return true; }
                    ViewCandidates.Reset();
                    for (const auto& CandidateRow : *CandidateRows)
                    {
                        const TSharedPtr<FJsonObject>* CandidateObject = nullptr;
                        if (!CandidateRow.IsValid() || !CandidateRow->TryGetObject(CandidateObject)
                            || !CandidateObject || !CandidateObject->IsValid())
                        { Test->AddError(TEXT("Invalid inspection candidate")); return true; }
                        FOpenWillowInspectionPose Candidate;
                        if (!Pose(*CandidateObject, Candidate))
                        { Test->AddError(TEXT("Inspection candidate requires finite location, rotation and FOV in 10..150")); return true; }
                        ViewCandidates.Add(Candidate);
                    }
                }
                Candidates.Add(ViewCandidates);
                FVector Target = FVector::ZeroVector;
                HasTargets.Add(Vector(Object, TEXT("target"), Target));
                Targets.Add(Target);
                FString TargetActor;
                Object->TryGetStringField(TEXT("target_actor_contains"), TargetActor);
                TargetActorContains.Add(TargetActor);
            }
            Origin = Pawn->GetActorLocation(); OriginalRotation = PC->GetControlRotation();
            OriginalFov = PC->PlayerCameraManager->GetFOVAngle();
            Loaded = true;
        }
        if (Index >= Candidates.Num())
        {
            Pawn->SetActorLocation(Origin, false, nullptr, ETeleportType::TeleportPhysics);
            PC->SetControlRotation(OriginalRotation); PC->PlayerCameraManager->SetFOV(OriginalFov);
            Test->AddInfo(FString::Printf(TEXT("Inspection captured %d views; visual acceptance pending"), Index));
            return true;
        }
        if (Stage == 0)
        {
            if (auto* Movement = Pawn->GetMovementComponent()) Movement->StopMovementImmediately();
            const FOpenWillowInspectionPose& PoseValue = Candidates[Index][CandidateIndex];
            Pawn->SetActorLocation(PoseValue.Position, false, nullptr, ETeleportType::TeleportPhysics);
            PC->SetControlRotation(PoseValue.Rotation); PC->PlayerCameraManager->SetFOV(PoseValue.Fov);
            Stage = 1; StageStarted = Now; return false;
        }
        if (Stage == 1 && Now - StageStarted > 8)
        {
            const FOpenWillowInspectionPose& PoseValue = Candidates[Index][CandidateIndex];
            Test->TestTrue(TEXT("Inspection camera position matches requested view"),
                PC->PlayerCameraManager->GetCameraLocation().Equals(PoseValue.Position, 1));
            Test->TestTrue(TEXT("Inspection camera rotation matches requested view"),
                PC->PlayerCameraManager->GetCameraRotation().Equals(PoseValue.Rotation, .1));
            Test->TestTrue(TEXT("Inspection FOV matches requested view"),
                FMath::Abs(PC->PlayerCameraManager->GetFOVAngle() - PoseValue.Fov) < .1);
            FString TargetHit = TEXT("none");
            bool TargetTraceHit = false, TargetInView = false, TargetMatched = false;
            if (HasTargets[Index])
            {
                FHitResult Hit;
                FCollisionQueryParams Params(SCENE_QUERY_STAT(OpenWillowInspectionTarget), true, Pawn);
                TargetTraceHit = World->LineTraceSingleByChannel(
                    Hit, PoseValue.Position, Targets[Index], ECC_Visibility, Params);
                const FVector ToTarget = (Targets[Index] - PoseValue.Position).GetSafeNormal();
                const float MinimumCosine = FMath::Cos(FMath::DegreesToRadians(PoseValue.Fov * .6f));
                TargetInView = FVector::DotProduct(PoseValue.Rotation.Vector(), ToTarget) >= MinimumCosine;
                const AActor* HitActor = Hit.GetActor();
                TargetHit = HitActor ? (HitActor->Tags.Num() > 0
                    ? HitActor->Tags[0].ToString() : HitActor->GetName()) : TEXT("none");
                TargetMatched = TargetTraceHit && TargetInView && (TargetActorContains[Index].IsEmpty()
                    || TargetHit.Contains(TargetActorContains[Index]));
                if (!TargetMatched && CandidateIndex + 1 < Candidates[Index].Num())
                {
                    Test->AddWarning(FString::Printf(
                        TEXT("Inspection view %d candidate %d obstructed before target: hit=%s expected=%s; trying candidate %d"),
                        Index, CandidateIndex, *TargetHit, *TargetActorContains[Index], CandidateIndex + 1));
                    ++CandidateIndex; Stage = 0; StageStarted = Now; return false;
                }
            }
            const FString Name = FPaths::Combine(FPaths::ScreenShotDir(),
                FString::Printf(TEXT("%s_inspection_%02d.png"), *World->GetMapName(), Index));
            FScreenshotRequest::RequestScreenshot(Name, false, true);
            Test->AddInfo(FString::Printf(TEXT("Inspection view %d candidate %d: location=%s rotation=%s fov=%.1f target_trace_hit=%d target_in_view=%d target_trace=%s target_matches=%d screenshot=%s"),
                Index, CandidateIndex, *PoseValue.Position.ToString(), *PoseValue.Rotation.ToString(), PoseValue.Fov,
                TargetTraceHit, TargetInView, *TargetHit, TargetMatched, *Name));
            Stage = 2; StageStarted = Now;
        }
        else if (Stage == 2 && Now - StageStarted > 2) { ++Index; CandidateIndex = 0; Stage = 0; }
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
    static bool Pose(const TSharedPtr<FJsonObject>& Object, FOpenWillowInspectionPose& Out)
    {
        FVector Position, Rotation;
        double Fov = 75;
        if (!Vector(Object, TEXT("location"), Position)
            || !Vector(Object, TEXT("rotation"), Rotation)
            || !Object->TryGetNumberField(TEXT("fov"), Fov)
            || !FMath::IsFinite(Fov) || Fov < 10 || Fov > 150)
            return false;
        Out.Position = Position;
        Out.Rotation = FRotator(Rotation.X, Rotation.Y, Rotation.Z);
        Out.Fov = static_cast<float>(Fov);
        return true;
    }
    FAutomationTestBase* Test;
    double Started, StageStarted = 0;
    bool Loaded = false;
    int Index = 0, Stage = 0;
    FVector Origin;
    FRotator OriginalRotation;
    float OriginalFov = 75;
    TArray<TArray<FOpenWillowInspectionPose>> Candidates;
    TArray<FVector> Targets;
    TArray<bool> HasTargets;
    TArray<FString> TargetActorContains;
    int CandidateIndex = 0;
};
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowInspectionTest, "OpenWillow.Inspection",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::ProductFilter)
bool FOpenWillowInspectionTest::RunTest(const FString& Parameters)
{ ADD_LATENT_AUTOMATION_COMMAND(FOpenWillowInspectionCheck(this)); return true; }
#endif
