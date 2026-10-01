#pragma once
#include "CoreMinimal.h"
#include "GameFramework/Character.h"
#include "OpenWillowWalker.generated.h"

struct FOpenWillowTakenInventoryItem;

UCLASS()
class OPENWILLOW_API AOpenWillowWalker : public ACharacter
{
    GENERATED_BODY()
public:
    AOpenWillowWalker();
    virtual void BeginPlay() override;
    virtual void Tick(float DeltaSeconds) override;
    virtual void SetupPlayerInputComponent(UInputComponent* Input) override;
    bool HasWeaponOut() const { return bWeaponOut; }
    bool IsMayaActive() const { return bMayaActive; }
    // Pickup presses that found a dropped item in reach (test hook).
    int32 PickupAttemptCount() const { return PickupAttempts; }
    bool LastPickupAccepted() const { return bLastPickupAccepted; }
    class UOpenWillowInventory* GetInventory() { return Inventory; }
    const class UOpenWillowInventory* GetInventory() const { return Inventory; }
    class UOpenWillowSkills* GetSkills() const { return Skills; }
    // Puts backpack item Item into weapon slot Slot and draws it (inventory screen).
    bool EquipItem(int32 Item, int32 Slot);
    void SetInventoryPresentation(bool bShow);
    // Returns the weapon to the backpack and holsters it if it was active.
    bool UnequipSlot(int32 Slot);
    // Removes by stable instance ID and preserves the payload for a pickup actor.
    bool TakeInventoryItemById(const FString& Id, FOpenWillowTakenInventoryItem& OutItem);
    void ToggleInventory();
    void ToggleSkills();
    float PhaselockRemaining() const;
    float PhaselockCooldown() const { return PhaselockCooldownSeconds; }
    float LastTargetHitAt() const { return TargetHitAt; }
    class UOpenWillowMover* GetMover() const { return Mover; }
    class UOpenWillowQuest* GetQuest() const { return Quest; }
    virtual float TakeDamage(float DamageAmount, struct FDamageEvent const& DamageEvent,
        class AController* EventInstigator, AActor* DamageCauser) override;
    float GetHealth() const { return Health; }
    float GetMaxHealth() const { return MaxHealth; }
private:
    void Forward(float Value);
    void Right(float Value);
    void Turn(float Value);
    void Look(float Value);
    void SprintPressed();
    void SprintReleased();
    void SelectSlot(int32 Slot);
    void Holster();
    void SelectSlot1() { SelectSlot(0); }
    void SelectSlot2() { SelectSlot(1); }
    void SelectSlot3() { SelectSlot(2); }
    void SelectSlot4() { SelectSlot(3); }
    void FirePressed();
    void FireReleased();
    void ReloadPressed();
    // Starts a timed reload of the held weapon; false when it cannot reload.
    bool StartReload(float Now);
    void CancelReload();
    void FireWeapon();
    void PickupNearby();
    void UsePhaselock();
    void RunCombatShots(float Now);
    void SendInventoryKey(const TCHAR* Key);
    void SpawnCombatTarget();
    void AimAt(const FVector& Point);
    FVector MuzzleLocation() const;
    UPROPERTY() TObjectPtr<class UCameraComponent> Camera;
    // Maya's first-person arms (-owmaya). Their animations carry a root
    // correction that keeps the arms skeleton's Camera bone at this
    // component's origin (tools/prepare_character_anims.py).
    UPROPERTY() TObjectPtr<class USkeletalMeshComponent> Arms;
    UPROPERTY() TObjectPtr<class USkeletalMeshComponent> WeaponVisual;
    UPROPERTY() TObjectPtr<class UOpenWillowInventory> Inventory;
    UPROPERTY() TObjectPtr<class UOpenWillowSkills> Skills;
    UPROPERTY() TObjectPtr<class UOpenWillowMover> Mover;
    UPROPERTY() TObjectPtr<class UOpenWillowQuest> Quest;
    // Host stand-ins (UNVERIFIED) unless -owquest supplies the slice data: then health follows the recovered
    // formula and respawn the decoded station selection (UOpenWillowQuest).
    float MaxHealth = 400.f;
    float Health = 400.f;
    FVector RespawnLocation = FVector::ZeroVector;
    FRotator RespawnRotation = FRotator::ZeroRotator;
    bool bRespawnPointCaptured = false;
    void Respawn();
    UPROPERTY() TObjectPtr<class UOpenWillowInventoryWidget> InventoryScreen;
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
    bool bWeaponOut = false;
    bool bInventoryPresentation = false;
    int32 PickupAttempts = 0;
    bool bLastPickupAccepted = false;
    bool bFireHeld = false;
    int32 ShotCount = 0;
    int32 CombatShotStep = 0;
    float NextShotAt = 0;
    bool bReloading = false;
    float ReloadEndsAt = 0;
    bool bOutOfAmmoLogged = false;
    float PhaselockReadyAt = 0;
    float LandUntil = 0;
    float TargetHitAt = -10;
    float PhaselockBeamUntil = 0;
    // Cooldown_Phaselock ConstantAttributeValueResolver: 13 s (no skill/class mods).
    float PhaselockCooldownSeconds = 13.f;
    bool bWantsCombatTarget = false;
    bool bBarrelAxisLogged = false;
    // Look-input weapon sway, in degrees (yaw, pitch).
    FVector2D LookInput = FVector2D::ZeroVector;
    FVector2D Sway = FVector2D::ZeroVector;
};
