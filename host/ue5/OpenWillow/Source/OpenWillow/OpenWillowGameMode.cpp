#include "OpenWillowGameMode.h"
#include "Camera/CameraActor.h"
#include "OpenWillowWalker.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "Camera/CameraComponent.h"
#include "Camera/PlayerCameraManager.h"
#include "EngineUtils.h"
#include "GameFramework/Controller.h"
#include "GameFramework/PlayerController.h"
#include "GameFramework/PlayerStart.h"
#include "GameFramework/SpectatorPawn.h"

AOpenWillowGameMode::AOpenWillowGameMode()
{
    DefaultPawnClass = FParse::Param(FCommandLine::Get(), TEXT("owwalk"))
        ? AOpenWillowWalker::StaticClass() : ASpectatorPawn::StaticClass();
}

AActor* AOpenWillowGameMode::ChoosePlayerStart_Implementation(AController* Player)
{
    UWorld* World = GetWorld();
    if (World)
    {
        for (TActorIterator<APlayerStart> It(World); It; ++It)
        {
            if (It->ActorHasTag(TEXT("OpenWillow_PlayerStart")))
            {
                return *It;
            }
        }
    }
    return Super::ChoosePlayerStart_Implementation(Player);
}

void AOpenWillowGameMode::RestartPlayer(AController* NewPlayer)
{
    Super::RestartPlayer(NewPlayer);
    APlayerController* Player = Cast<APlayerController>(NewPlayer);
    APawn* Pawn = Player ? Player->GetPawn() : nullptr;
    if (!Pawn)
    {
        return;
    }

    if (Cast<AOpenWillowWalker>(Pawn))
    {
        Player->SetViewTarget(Pawn);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow walking pawn activated at %s"), *Pawn->GetActorLocation().ToString());
        return;
    }

    // Phase 1 is a free-flight inspector. Imported visibility/collision-only
    // geometry must not trap its camera; walking collision is a separate gate.
    Pawn->SetActorEnableCollision(false);

    UWorld* World = GetWorld();
    if (!World)
    {
        return;
    }

    for (TActorIterator<ACameraActor> It(World); It; ++It)
    {
        if (It->ActorHasTag(TEXT("OpenWillow_InspectionCamera")))
        {
            // The saved camera defines the starting pose. Keep the view on the
            // possessed pawn so movement and mouse look move the actual view.
            Pawn->SetActorLocationAndRotation(It->GetActorLocation(), It->GetActorRotation());
            Player->SetControlRotation(It->GetActorRotation());
            Player->SetViewTargetWithBlend(Pawn, 0.0f);
            if (Player->PlayerCameraManager)
            {
                Player->PlayerCameraManager->SetFOV(It->GetCameraComponent()->FieldOfView);
            }
            UE_LOG(LogTemp, Display, TEXT("OpenWillow free-flight pawn activated at %s"),
                   *Pawn->GetActorLocation().ToString());
            return;
        }
    }

    UE_LOG(LogTemp, Warning, TEXT("OpenWillow inspection camera tag was not found"));
}
