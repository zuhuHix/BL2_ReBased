#pragma once
#include "CoreMinimal.h"
#include "GameFramework/GameModeBase.h"
#include "OpenWillowGameMode.generated.h"

class APlayerController;

// A free-flight camera for inspecting the frozen imported scene.
UCLASS()
class OPENWILLOW_API AOpenWillowGameMode : public AGameModeBase
{
    GENERATED_BODY()
public:
    AOpenWillowGameMode();

    virtual void RestartPlayer(AController* NewPlayer) override;

protected:
    virtual AActor* ChoosePlayerStart_Implementation(AController* Player) override;
};
