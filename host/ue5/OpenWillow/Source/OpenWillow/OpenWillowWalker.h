#pragma once
#include "CoreMinimal.h"
#include "GameFramework/Character.h"
#include "OpenWillowWalker.generated.h"

UCLASS()
class OPENWILLOW_API AOpenWillowWalker : public ACharacter
{
    GENERATED_BODY()
public:
    AOpenWillowWalker();
    virtual void BeginPlay() override;
    virtual void SetupPlayerInputComponent(UInputComponent* Input) override;
private:
    void Forward(float Value);
    void Right(float Value);
    void Turn(float Value);
    void Look(float Value);
    UPROPERTY() TObjectPtr<class UCameraComponent> Camera;
    // First-person arms baked in the arms skeleton's Camera-bone space
    // (tools/prepare_character_pose.py); loaded from -owarms=<asset path>.
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Arms;
};
