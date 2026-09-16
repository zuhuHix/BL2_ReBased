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

// Exercise real imported BSP floors; success requires source identity, floor
// height and lateral position to agree, preventing displaced endpoints passing.
class FOpenWillowBspCheck : public IAutomationLatentCommand
{
public:
    explicit FOpenWillowBspCheck(FAutomationTestBase* InTest) : Test(InTest), Started(FPlatformTime::Seconds()) {}
    bool Update() override
    {
        UWorld* World = nullptr;
        for (const auto& C : GEngine->GetWorldContexts()) if (C.WorldType == EWorldType::Game) World = C.World();
        auto* PC = World ? World->GetFirstPlayerController() : nullptr;
        auto* Pawn = PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr;
        if (FPlatformTime::Seconds()-Started > 180) { Test->AddError(TEXT("BSP walking timed out")); return true; }
        if (!Pawn) return false;
        auto* Move = Pawn->GetCharacterMovement();
        const double Time = World->GetTimeSeconds();
        if (Stage == 0)
        {
            if (FPlatformTime::Seconds()-Started < 8) return false;
            FString Text;
            const FString Root = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_SCENE"));
            TSharedPtr<FJsonObject> Json;
            if (!FFileHelper::LoadFileToString(Text, *FPaths::Combine(Root,TEXT("bsp-runtime.json")))
                || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text),Json) || !Json.IsValid()
                || !Json->GetBoolField(TEXT("collision")))
            { Test->AddError(TEXT("Missing collision-enabled bsp-runtime.json")); return true; }
            for (const auto& Value : Json->GetArrayField(TEXT("models"))) Models.Add(Value->AsObject());
            if (Models.IsEmpty()) { Test->AddError(TEXT("No BSP runtime models")); return true; }
            Origin = Pawn->GetActorLocation(); Stage = 1;
        }
        if (Model >= Models.Num())
        {
            Pawn->SetActorLocation(Origin,false,nullptr,ETeleportType::TeleportPhysics);
            Move->StopMovementImmediately();
            Test->AddInfo(FString::Printf(TEXT("BSP runtime summary: stood_and_walked=%d/%d rejected_candidates=%d"), Passed,Models.Num(),Rejected));
            return true;
        }
        const auto& Candidates = Models[Model]->GetArrayField(TEXT("candidates"));
        if (Candidate >= Candidates.Num())
        { Test->AddError(TEXT("No unobstructed BSP candidate passed standing and walking")); ++Model; Candidate=0; Stage=1; return false; }
        const auto P = Candidates[Candidate]->AsObject();
        const FString Source = P->GetStringField(TEXT("source"));
        const FVector Start = Point(P,TEXT("start")), End = Point(P,TEXT("end"));
        if (Stage == 1)
        {
            Pawn->SetActorLocation(Start+FVector(0,0,150),false,nullptr,ETeleportType::TeleportPhysics);
            Move->StopMovementImmediately(); Move->SetMovementMode(MOVE_Falling);
            Stage=2; StageTime=Time; return false;
        }
        if (Stage == 2 && Time-StageTime > 2)
        {
            const bool Valid = OnSurface(World,Pawn,Source,Start);
            Test->AddInfo(FString::Printf(TEXT("BSP stand: %s node=%d valid=%d pawn=%s"),*Source,
                static_cast<int>(P->GetNumberField(TEXT("node"))),Valid,*Pawn->GetActorLocation().ToString()));
            if (!Valid) { ++Rejected; ++Candidate; Stage=1; return false; }
            FRotator View=(End-Start).Rotation(); View.Pitch=-35; PC->SetControlRotation(View);
            FScreenshotRequest::RequestScreenshot(FString::Printf(TEXT("OpenWillowBsp_%d.png"),Model),false,false);
            Stage=3; StageTime=Time; return false;
        }
        if (Stage == 3)
        {
            FVector Direction=End-Pawn->GetActorLocation(); Direction.Z=0;
            if (Direction.Size2D()>20 && Time-StageTime<3)
            { Pawn->AddMovementInput(Direction.GetSafeNormal(),1,true); return false; }
            Move->StopMovementImmediately();
            const bool Valid=OnSurface(World,Pawn,Source,End);
            Test->TestTrue(TEXT("Pawn walks 200 cm on the identified BSP floor"),Valid);
            Test->AddInfo(FString::Printf(TEXT("BSP walk: %s valid=%d pawn=%s"),*Source,Valid,*Pawn->GetActorLocation().ToString()));
            Passed+=Valid; ++Model; Candidate=0; Stage=1;
        }
        return false;
    }
private:
    static FVector Point(const TSharedPtr<FJsonObject>& P,const TCHAR* Key)
    { const auto& V=P->GetArrayField(Key); return FVector(V[0]->AsNumber(),V[1]->AsNumber(),V[2]->AsNumber()); }
    static bool OnSurface(UWorld* World,AOpenWillowWalker* Pawn,const FString& Source,const FVector& Expected)
    {
        const FVector Position=Pawn->GetActorLocation();
        FHitResult Hit; FCollisionQueryParams Params(SCENE_QUERY_STAT(OpenWillowBsp),true,Pawn);
        if (!World->LineTraceSingleByChannel(Hit,Position,Position-FVector(0,0,200),ECC_Visibility,Params)) return false;
        const auto* Actor=Hit.GetActor();
        return Pawn->GetCharacterMovement()->IsMovingOnGround() && Actor && Actor->Tags.Num()>0
            && Actor->Tags[0].ToString()==Source && !Hit.bStartPenetrating
            && FMath::Abs(Hit.ImpactPoint.Z-Expected.Z)<5 && FVector::Dist2D(Position,Expected)<35;
    }
    FAutomationTestBase* Test;
    double Started,StageTime=0;
    int Stage=0,Model=0,Candidate=0,Passed=0,Rejected=0;
    FVector Origin;
    TArray<TSharedPtr<FJsonObject>> Models;
};
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowBspWalkingTest,"OpenWillow.BspWalking",EAutomationTestFlags::ClientContext|EAutomationTestFlags::EngineFilter)
bool FOpenWillowBspWalkingTest::RunTest(const FString& Parameters)
{ ADD_LATENT_AUTOMATION_COMMAND(FOpenWillowBspCheck(this)); return true; }
#endif
