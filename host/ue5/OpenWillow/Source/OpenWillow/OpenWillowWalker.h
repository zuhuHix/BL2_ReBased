#pragma once
#include "CoreMinimal.h"
#include "GameFramework/Character.h"
#include "OpenWillowPhaselock.h"
#include "OpenWillowPhaselockFx.h"
#include "OpenWillowWalker.generated.h"

struct FOpenWillowTakenInventoryItem;
struct FOpenWillowWeaponItem;

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
    // Mission weapon lend/return (UOpenWillowQuest): the item goes into a weapon slot and is drawn; on return it is
    // removed and the slot held before the lend is drawn again. Placement rule: host choice, UNVERIFIED.
    bool LendWeapon(const FOpenWillowWeaponItem& Item);
    bool ReturnLentWeapon(const FString& Id);
    // Draws the equipped weapon with this stable id; false when it is not in a slot.
    bool DrawItemById(const FString& Id);
    // One trigger pull of the held weapon (test hook; the fire button goes through the same shot code).
    bool FireOnce();
    // Stock damage type path of the shot being applied right now (the held item's card damage type); "" otherwise.
    const FString& ShotDamageType() const { return ShotDamageTypeInFlight; }
    // Phaselock from the action-skill manifest (-owactionskill=, default local/character/action_skill_siren.json).
    void UsePhaselock();
    // Whether a cast would start now; otherwise why not (not bought, cooldown, or the failing skill constraint).
    bool CanCastPhaselock(FString& OutReason) const;
    const FOpenWillowPhaselockData& GetPhaselockData() const { return Phaselock; }
    const FString& GetPhaselockFile() const { return PhaselockFile; }
    // Object path of the mesh shown in Maya's hand ("" when none).
    FString HeldWeaponMesh() const;
    float PhaselockRemaining() const;
    float PhaselockCooldown() const { return Phaselock.CooldownSeconds; }
    // Last cast: world time, whether it lifted a target, whether the target was blocked (CanLiftTargetIf failed), and
    // its timeline (zero for a miss or a blocked target).
    float LastPhaselockCastAt() const { return PhaselockCastAt; }
    bool LastPhaselockHit() const { return bPhaselockHit; }
    bool LastPhaselockBlocked() const { return bPhaselockBlocked; }
    const FOpenWillowPhaselockTimeline& LastPhaselockTimeline() const { return PhaselockTimeline; }
    class AOpenWillowCombatTarget* LastPhaselockTarget() const { return PhaselockTarget.Get(); }
    // Host reading of WillowAutoAimStrategy.GetPreferredTarget for the action skill (native; NATIVE_PHASELOCK_TARGETING.md
    // sections 1-3, UNVERIFIED): every live host target in the targetable list is scored by screen-space magnetism, depth
    // and line of sight from the player camera; the best score above 0 wins (ties keep the first found).
    class AOpenWillowCombatTarget* PreferredPhaselockTarget(FString* OutLog = nullptr) const;
    struct FPhaselockAimScore
    {
        float Score = 0, Depth = 0, ScreenOffset = 0, TargetRadius = 0, MagnetRadius = 0;
        FString Rejected;           // why the score is 0 ("" when it scored)
    };
    FPhaselockAimScore ScorePhaselockTarget(const class AOpenWillowCombatTarget* Target) const;
    // Maya's state as the constraint evaluators read it (host mapping, FOpenWillowPhaselockGateState).
    FOpenWillowPhaselockGateState PhaselockGateState() const;
    // From a cast that lifted a target until EndSkill (or an early end by a while-active constraint).
    bool IsPhaselockActive() const;
    const FString& LastPhaselockEndReason() const { return PhaselockEndReason; }
    const FOpenWillowPhaselockFxData& GetPhaselockFx() const { return PhaselockFx; }
    // Presentation state for checks and logs: tattoo glow parameter now, live hand-orb and screen particles.
    float TattooGlowNow() const { return TattooGlow; }
    int32 HandFxParticles() const;
    int32 ScreenFxParticles() const;
    bool AreArmsShown() const;
    float LastTargetHitAt() const { return TargetHitAt; }
    class UOpenWillowMover* GetMover() const { return Mover; }
    class UOpenWillowQuest* GetQuest() const { return Quest; }
    virtual float TakeDamage(float DamageAmount, struct FDamageEvent const& DamageEvent,
        class AController* EventInstigator, AActor* DamageCauser) override;
    float GetHealth() const { return Health; }
    float GetMaxHealth() const { return MaxHealth; }
    // -owquest: sets maximum health from the recovered formula when Maya's level differs from the level it was last
    // set for (start, XP level-up, test fixtures); does nothing otherwise.
    void RefreshHealthForLevel();
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
    // Arms are shown while a pose clip set is loaded (a drawn weapon, or Unarmed clips if imported) and no menu covers them.
    void UpdateArmsVisibility();
    void RunCombatShots(float Now);
    // -owgunshots -owgunids=a,b,c: equips each named item in turn and writes a first-person frame and a side-view frame
    // of it (lane C review captures; docs/verification/WEAPON_VISUALS.md).
    void RunGunShots(float Now);
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
    int32 HealthLevel = 0;              // the level MaxHealth was last set for (0: not yet)
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
    int32 GunShotStep = 0;
    TWeakObjectPtr<class AOpenWillowInventoryPreviewActor> GunShotPreview;
    float NextShotAt = 0;
    bool bReloading = false;
    float ReloadEndsAt = 0;
    bool bOutOfAmmoLogged = false;
    float LandUntil = 0;
    float TargetHitAt = -10;
    // Weapon glow after shots (native rule, UNVERIFIED: +0.25 per shot, cap 5, decay 3.5/s from 0.2 s after the shot; the
    // emissive scale is the base times 1 + impulse). Drives the paint material's OW_Emissive.
    float GlowImpulse = 0.f;
    float LastGlowShotAt = -10.f;
    float AppliedGlow = -1.f;
    FOpenWillowPhaselockData Phaselock;
    FString PhaselockFile;
    // Stock presentation (tools/prepare_phaselock_fx.py manifest, -owphaselockfx=): hand orb at the arms clip's notify,
    // tattoo glow on the arms, screen particle while the skill runs.
    FOpenWillowPhaselockFxData PhaselockFx;
    UPROPERTY() TArray<TObjectPtr<UObject>> PhaselockFxAssets;   // preloaded emitter materials and meshes (GC roots)
    UPROPERTY() TObjectPtr<class UOpenWillowFxComponent> HandFx;
    UPROPERTY() TObjectPtr<class UOpenWillowFxComponent> ScreenFx;
    UPROPERTY() TObjectPtr<class UMaterialInstanceDynamic> TattooGlowMaterial;
    float HandFxAt = -1;
    bool bHandFxMiss = false;
    float GlowStartedAt = -100;
    float TattooGlow = 0;
    float Bl2FovSetting = 90;           // the BL2 FOV setting (UE3 FOVAngle, horizontal at 4:3)
    FString PhaselockEndReason;
    float PhaselockEndedEarlyAt = -100;
    void UpdatePhaselockPresentation(float Now);
    void EndPhaselockEarly(const FString& Reason);
    // -owphaselockshots (with -owcombattest): an unattended cast with timed captures (OWPhaselock_*.png).
    void RunPhaselockShots(float Now);
    int32 PhaselockShotStep = 0;
    float PhaselockShotCastAt = -1;
    // Cooldown pool model (UNVERIFIED semantics, PHASELOCK_STOCK_DATA.md): refilled at activation, drained at the held
    // rate until PhaselockHeldUntil (the release), then at the base rate; a miss resets it at PhaselockResetAt.
    float PhaselockCastAt = -100.f;
    float PhaselockHeldUntil = -100.f;
    float PhaselockResetAt = -100.f;
    bool bPhaselockHit = false;
    bool bPhaselockBlocked = false;
    FOpenWillowPhaselockTimeline PhaselockTimeline;
    TWeakObjectPtr<class AOpenWillowCombatTarget> PhaselockTarget;
    FString ShotDamageTypeInFlight;
    int32 PreLendSlot = INDEX_NONE;
    bool bWantsCombatTarget = false;
    bool bBarrelAxisLogged = false;
    // Look-input weapon sway, in degrees (yaw, pitch).
    FVector2D LookInput = FVector2D::ZeroVector;
    FVector2D Sway = FVector2D::ZeroVector;
};
