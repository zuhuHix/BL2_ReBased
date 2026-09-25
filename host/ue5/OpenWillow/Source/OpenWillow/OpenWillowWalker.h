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
    bool IsInfinityEquipped() const { return bInfinityEquipped; }
    float PhaselockRemaining() const;
    float PhaselockCooldown() const { return PhaselockCooldownSeconds; }
    float LastTargetHitAt() const { return TargetHitAt; }
private:
    void Forward(float Value);
    void Right(float Value);
    void Turn(float Value);
    void Look(float Value);
    void SprintPressed();
    void SprintReleased();
    void EquipInfinity();
    void HolsterInfinity();
    void FirePressed();
    void FireReleased();
    void FireInfinity();
    void UsePhaselock();
    void RunCombatShots(float Now);
    void SpawnCombatTarget();
    void AimAt(const FVector& Point);
    FVector MuzzleLocation() const;
    UPROPERTY() TObjectPtr<class UCameraComponent> Camera;
    // Maya's first-person arms (-owmaya). Their animations carry a root
    // correction that keeps the arms skeleton's Camera bone at this
    // component's origin (tools/prepare_character_anims.py).
    UPROPERTY() TObjectPtr<class USkeletalMeshComponent> Arms;
    UPROPERTY() TObjectPtr<class USkeletalMeshComponent> WeaponVisual;
    UPROPERTY() TObjectPtr<class UAnimSequence> IdleAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> RunAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> SprintAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> JumpAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> LandAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> DrawPistolAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> FirePistolAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> PhaselockAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> PhaselockFailAnim;
    UPROPERTY() TObjectPtr<class UOpenWillowArmsAnimInstance> ArmsAnim;
    TWeakObjectPtr<class AOpenWillowCombatTarget> CombatTarget;
    bool bSprintHeld = false;
    bool bWasFalling = false;
    bool bSpawnProbeLogged = false;
    bool bMayaActive = false;
    bool bInfinityEquipped = false;
    bool bFireHeld = false;
    int32 InfinityShot = 0;
    int32 CombatShotStep = 0;
    float NextShotAt = 0;
    float PhaselockReadyAt = 0;
    float LandUntil = 0;
    float TargetHitAt = -10;
    float PhaselockBeamUntil = 0;
    float PhaselockCooldownSeconds = 13.f; // host prototype; BL2 formula still needs evaluation
    bool bWantsCombatTarget = false;
    bool bBarrelAxisLogged = false;
    // Look-input weapon sway, in degrees (yaw, pitch).
    FVector2D LookInput = FVector2D::ZeroVector;
    FVector2D Sway = FVector2D::ZeroVector;
};
