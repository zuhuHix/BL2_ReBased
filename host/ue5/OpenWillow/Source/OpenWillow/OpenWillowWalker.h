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
    virtual void Tick(float DeltaSeconds) override;
    virtual void SetupPlayerInputComponent(UInputComponent* Input) override;
private:
    void Forward(float Value);
    void Right(float Value);
    void Turn(float Value);
    void Look(float Value);
    void Play(class UAnimSequence* Anim, bool bLoop);
    UPROPERTY() TObjectPtr<class UCameraComponent> Camera;
    // Maya's first-person arms (-owmaya). Their animations carry a root
    // correction that keeps the arms skeleton's Camera bone at this
    // component's origin (tools/prepare_character_anims.py).
    UPROPERTY() TObjectPtr<class USkeletalMeshComponent> Arms;
    UPROPERTY() TObjectPtr<class UAnimSequence> IdleAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> RunAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> JumpAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> LandAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> Current;
    bool bWasFalling = false;
    bool bSpawnProbeLogged = false;
    float LandUntil = 0;
};
