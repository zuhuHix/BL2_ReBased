#pragma once

#include "CoreMinimal.h"
#include "GameFramework/HUD.h"
#include "OpenWillowMayaHUD.generated.h"

UCLASS()
class OPENWILLOW_API AOpenWillowMayaHUD : public AHUD
{
    GENERATED_BODY()
public:
    virtual void DrawHUD() override;
};
