#pragma once
#include "CoreMinimal.h"
#include "Blueprint/UserWidget.h"
#include "GameFramework/Character.h"
#include "OpenWillowPhaselock.h"
#include "OpenWillowPhaselockFx.h"
#include "OpenWillowFiring.h"
#include "OpenWillowWalker.generated.h"

struct FOpenWillowTakenInventoryItem;
struct FOpenWillowWeaponItem;

// Procedural scope mask and reticle for recipes whose selected sight carries a
// first-person particle component. Its match to the stock scope presentation is UNVERIFIED.
UCLASS()
class OPENWILLOW_API UOpenWillowScopeOverlay : public UUserWidget
{
    GENERATED_BODY()
protected:
    virtual int32 NativePaint(const FPaintArgs& Args, const FGeometry& AllottedGeometry,
        const FSlateRect& MyCullingRect, FSlateWindowElementList& OutDrawElements, int32 LayerId,
        const FWidgetStyle& InWidgetStyle, bool bParentEnabled) const override;
};

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
    // Mission weapon lend/return (UOpenWillowQuest): the grant adds the item to the backpack (observed in the real game: not equipped; the player
    // readies it with EquipItem); the return removes it wherever it is and draws another weapon only if it was the held one.
    bool LendWeapon(const FOpenWillowWeaponItem& Item);
    bool ReturnLentWeapon(const FString& Id);
    // Draws the equipped weapon with this stable id; false when it is not in a slot.
    bool DrawItemById(const FString& Id);
    // One trigger pull of the held weapon (test hook; the fire button goes through the same shot code).
    bool FireOnce();
    // Stock damage type path of the shot being applied right now (the held item's card damage type); "" otherwise.
    const FString& ShotDamageType() const { return ShotDamageTypeInFlight; }
    // True while a burst still owes shots (the -owfiringtest harness waits for it before the next pull).
    bool IsBurstFiring() const { return Burst.active(); }
    // Damage from one of Maya's shots, its projectile, splash or status tick: the target reads DamageType (the stock
    // damage type path) while the damage is applied.
    // Momentum is the hit's momentum vector (UE3 TakeDamage's Momentum); the target reads it with ShotMomentum().
    void ApplyShotDamage(AActor* Target, float Amount, const FString& DamageType, const FVector& Direction,
        const FHitResult& Hit, const FVector& Momentum = FVector::ZeroVector);
    // Momentum of the shot being applied right now; zero otherwise.
    const FVector& ShotMomentum() const { return ShotMomentumInFlight; }
    // Phaselock from the action-skill manifest (-owactionskill=, default local/character/action_skill_siren.json).
    void UsePhaselock();
    // Whether a cast would start now; otherwise why not (not bought, cooldown, or the failing skill constraint).
    bool CanCastPhaselock(FString& OutReason) const;
    const FOpenWillowPhaselockData& GetPhaselockData() const { return Phaselock; }
    const FString& GetPhaselockFile() const { return PhaselockFile; }
    // Object path of the mesh shown in Maya's hand ("" when none).
    FString HeldWeaponMesh() const;
    float PhaselockRemaining() const;
    // The level-up skill's modifier on the action-skill cooldown's current value (the VM's ActionSkillCooldown event, swap 10): a plain attribute, so it takes
    // effect in place at once: Scale multiplies the current value by `Amount` (0 resets it), PreAdd and PostAdd add. A cast in progress is not ended and the
    // release is not moved (observed in game, COMBAT_GOLDEN_TRACE.md section E: the cooldown read 0 during the hold and after the release). Returns true when
    // the modifier ended a running cooldown (the game then runs ActionSkillCooldownComplete; the host has no such handler, it logs it, and the HUD ring reads PhaselockRemaining).
    bool ModifyPhaselockCooldown(int32 ModifierType, float Amount);
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
    // ---- Fight For Your Life: the host's down state (rules and their live numbers: docs/verification/COMBAT_DOWN_RESPAWN.md) ----
    // Health reaching 0 puts Maya down (Injured) instead of respawning her at once. 2.28 s later the bleed-out timer starts
    // (12 s, cut for successive injuries); a kill while Injured is a second wind (health back to 25 % of the maximum); the end
    // of the bleed-out is death (Dead), and the respawn follows 5.76 s later. Read by the HUD (AOpenWillowMayaHUD) and the checks.
    enum class EDownPhase : uint8 { Up, Injured, Dead };
    EDownPhase GetDownPhase() const { return DownPhase; }
    bool IsDown() const { return DownPhase != EDownPhase::Up; }
    // The bleed-out left as a fraction of its total: 1 until its timer starts, falling to 0 at death; 0 while Dead, 1 when up.
    float DownBleedLeft() const;
    // Whether the weapon is in hand for firing and reloading: always when up; while Injured only after the put-away special
    // move ends (2.28 s after the injury); never while Dead (see GoDown, UNVERIFIED).
    bool DownWeaponReady() const;
    bool IsReloading() const { return bReloading; }
    // Test drivers (-owhuddownshots): the same handlers the fire and reload input bindings call.
    void TestFireInput(bool bPressed) { if (bPressed) FirePressed(); else FireReleased(); }
    void TestReloadInput() { ReloadPressed(); }
    float DownBleedTotal() const { return DownBleedSeconds; }
    float LastDownAt() const { return DownAt; }
    float LastSecondWindAt() const { return SecondWindAt; }
    float LastRespawnAt() const { return RespawnAt; }
    // Maya killed an enemy (AOpenWillowCombatTarget calls it with her as the damage causer): while Injured, a second wind.
    void NotifyKilledEnemy();
    // -owquest: sets maximum health from the recovered formula when Maya's level differs from the level it was last
    // set for (start, XP level-up, test fixtures); does nothing otherwise.
    void RefreshHealthForLevel();
    // A level-up from the mission script's experience is handled by the VM (swap 7b): the host's own refresh waits (HoldHealthForScript) until the
    // script's Health event arrives, and ApplyScriptHealth sets the maximum and current health from it (and ends the wait).
    void HoldHealthForScript(bool bHold) { bHealthHeldForScript = bHold; }
    bool IsHealthHeldForScript() const { return bHealthHeldForScript; }
    void ApplyScriptHealth(float NewMax, float NewCurrent);
    // A natural level-up's OnLevelUpNaturally skill (the VM's WeaponDamageScale event, swap 9): a Scale modifier of `Scale` on the held weapon's damage for
    // `Seconds` of game time from now. Each call is its own modifier, so two stack. NATIVE_LEVEL_UP_ATTRIBUTES.md section 7; observed in game: +1.0 on the
    // scale sum for 30 s (REALGAME_GROUND_TRUTH.md). Not saved (it is gone 30 s later).
    void AddWeaponDamageScale(float Scale, float Seconds);
    // The sum of the scale modifiers still running at game time `Now` (0 when none).
    float WeaponDamageScaleAt(float Now) const;
    // The damage of one shot of `Weapon` at game time `Now`. The stock rule is damage = (base + PreAdd) x (1 + up) / (1 + down) + PostAdd over the weapon's
    // stack (NATIVE_ATTRIBUTES.md section 3; the card's "damage_stack" holds the terms), a running Scale s joins `up` (observed: sum 2.017 to 3.017, damage
    // 41.40 to 61.92). A weapon without the stack (a recipe from before the field) falls back to card damage x (1 + s): HOST STAND-IN (UNVERIFIED), S taken
    // as 1, an overstatement when other Scale modifiers are on the weapon. Which of the two applies is the weapon's, not a setting.
    float ShotDamage(const FOpenWillowWeaponItem& Weapon, float Now) const;
private:
    struct FViewModel;
    struct FWeaponSocketData;
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
    void AimPressed();
    void AimReleased();
    void ReloadPressed();
    // Starts a timed reload of the held weapon; false when it cannot reload.
    bool StartReload(float Now);
    void CancelReload();
    // Moves the magazine refill once the reload passes its ReloadCompletePercent point, into the weapon
    // that started the reload (never a weapon swapped in since).
    void RefillIfDue(float Now);
    FOpenWillowWeaponItem* ReloadingWeapon();
    void FireWeapon();
    void BeginAimTransition(float Now);
    void UpdateAim(float Now);
    const FViewModel* ActiveViewModel() const;
    void RunReloadPhaselockTest(float Now);
    FTransform WeaponSocketWorld(const FWeaponSocketData& Socket) const;
    void PickupNearby();
    // Arms are shown while a pose clip set is loaded (a drawn weapon, or Unarmed clips if imported) and no menu covers them.
    void UpdateArmsVisibility();
    void RunCombatShots(float Now);
    // -owgunshots -owgunids=a,b,c: captures idle, ADS, firing, reload and side-view frames per named item.
    void RunGunShots(float Now);
    // -owfiringtest -owgunids=a,b,c (with -owcombattest): fires each named gun at the dummy, at a wall and (status guns)
    // at the dummy again, then logs one OWFIRING result line per gun (tools/test_weapon_firing.ps1).
    void RunFiringTest(float Now);
    // -owfiringshots -owgunids=a,b,c: per gun, the real reference's pose (pitch 0, yaw 90, FOV 77.55), a flat wall 465.9 uu ahead,
    // one trigger pull and a frame every ~46 ms from two frames before the shot (tools/capture_firing_frames.ps1).
    void RunFiringShots(float Now);
    void SendInventoryKey(const TCHAR* Key);
    void SpawnCombatTarget();
    void AimAt(const FVector& Point);
    // Muzzle flash and tracer origin: the view model's Muzzle socket, else the recipe's decoded Muzzle socket, else a
    // camera-space estimate (logged, counted).
    FVector MuzzleLocation() const;
    // World position of a socket of the held gun (recipe "sockets"; WEAPON_VISUALS.md section 17); false when the gun has none.
    // "EjectPort" is exposed this way for shell casings, which the host does not spawn yet.
    bool GunSocketLocation(const TCHAR* Name, FVector& OutWorld) const;
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
    UPROPERTY() TObjectPtr<UOpenWillowScopeOverlay> ScopeOverlayWidget;
    // Host stand-ins (UNVERIFIED) unless -owquest supplies the slice data: then health follows the recovered
    // formula and respawn the decoded station selection (UOpenWillowQuest).
    float MaxHealth = 400.f;
    float Health = 400.f;
    int32 HealthLevel = 0;              // the level MaxHealth was last set for (0: not yet)
    bool bHealthHeldForScript = false;  // a script level-up is pending: RefreshHealthForLevel does nothing
    struct FDamageScale { float Scale = 0; float EndsAt = 0; };
    TArray<FDamageScale> DamageScales;  // the running weapon damage Scale modifiers (AddWeaponDamageScale); ended ones are dropped by Tick
    FVector RespawnLocation = FVector::ZeroVector;
    FRotator RespawnRotation = FRotator::ZeroRotator;
    bool bRespawnPointCaptured = false;
    void Respawn();
    // The down state (see EDownPhase above). LastInjuryAt starts far in the past: the game's default for its counterpart is not
    // known (COMBAT_DOWN_RESPAWN.md, UNVERIFIED), and this keeps a first injury early in a session uncut.
    EDownPhase DownPhase = EDownPhase::Up;
    float DownAt = -1000.f, DownBleedSeconds = 12.f, DeadAt = -1000.f, SecondWindAt = -1000.f, RespawnAt = -1000.f;
    float LastInjuryAt = -1000.f;
    int32 SuccessiveInjuries = 0;
    void GoDown();
    void TickDown(float Now);
    void EndDownOnRefill(const TCHAR* Why);
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
    bool bAimHeld = false;
    float WorldFovDefault = 90.f;
    float CameraAspect = 16.f / 9.f;
    float AimTransitionStart = 0.f;
    float AimTransitionDuration = 0.2f;
    float AimTransitionStartAlpha = 0.f;
    float AimTransitionTargetAlpha = 0.f;
    float AimAlpha = 0.f;
    FString AimTransitionWeaponId;
    int32 ShotCount = 0;
    int32 CombatShotStep = 0;
    int32 GunShotStep = 0;
    int32 GunShotBurstCount = 0;
    float GunShotNextBurstAt = 0.f;
    float GunShotPhaseAt = 0.f;
    float GunShotFinishedAt = -1.f;
    TWeakObjectPtr<class AOpenWillowInventoryPreviewActor> GunShotPreview;
    float NextShotAt = 0;
    bool bReloading = false;
    float ReloadEndsAt = 0;
    float ReloadRefillAt = 0;
    float ReloadCompletePercent = 1.f; // missing view-model field preserves the old full-duration refill
    FString ReloadCompletePercentSource = TEXT("default: missing cooked type field; legacy full-duration refill");
    int32 ReloadAmmoMoved = 0;
    bool bReloadRefilled = false;
    FString ReloadItemId; // weapon the running reload belongs to
    UPROPERTY() TObjectPtr<class UAnimSequence> WeaponReloadAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> ArmsReloadAnim;
    bool bOutOfAmmoLogged = false;
    float LandUntil = 0;
    float TargetHitAt = -10;
    // Per-recipe first-person presentation from weapon_view.json (tools/weapon_view_model.py): cooked view/FOV/kick/reload
    // values and the selected recipe fragments' socket transforms.
    struct FWeaponSocketData
    {
        bool bValid = false;
        FString Name;
        FString Bone;
        FVector Location = FVector::ZeroVector;
        FRotator Rotation = FRotator::ZeroRotator;
    };
    struct FViewModel
    {
        FVector Offset = FVector::ZeroVector;
        float Fov = 0.f;
        float ZoomFov = 0.f;
        float ZoomBlendTime = 0.2f;
        float ReloadCompletePercent = 1.f; // compatibility default for older manifests
        FString ReloadCompletePercentSource = TEXT("default: missing cooked type field; legacy full-duration refill");
        FVector ZoomOffset = FVector::ZeroVector;
        FVector ZoomPositionFit = FVector::ZeroVector;
        FWeaponSocketData ZoomSocket;
        FWeaponSocketData MuzzleSocket;
        FWeaponSocketData ShellSocket;
        bool bScopeOverlay = false;
        bool bTracerEnabled = false;
        bool bShellCasingEnabled = false;
        float MuzzleFlashDuration = 0.33f;
        float MuzzleFlashRadius = 3.5f; // UNVERIFIED host stand-in; WeaponTypeDefinition has no scalar flash size.
        float TracerRadius = 0.8f;      // UNVERIFIED host stand-in; no type tracer template is present in the measured slice.
        float TracerLife = 0.09f;       // UNVERIFIED host stand-in; no type tracer template is present in the measured slice.
        FLinearColor TracerColor = FLinearColor(200.f / 255.f, 200.f / 255.f, 200.f / 255.f, 1.f);
        float RecoilWeight = 0.45f;
        float ZoomedRecoilScale = 1.f;
        FString ReloadAsset;
        FString ReloadAnimation;
        FString ArmsAnimSet;
    };
    TMap<FString, FViewModel> ViewModels;
    bool bViewModelActive = false;      // foreground FOV on (the default; -owfpfov=0 turns it and the offsets off)
    bool bViewFovForced = false;        // -owfpfov=<n> given: that value for every weapon
    float ViewFovDefault = 45.f;        // horizontal degrees
    void LoadViewModels(const FString& ItemDir);
    void ApplyViewModel(const FString& ItemId);
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
    int32 ReloadPhaselockTestStep = 0;
    float ReloadPhaselockTestRefillAt = 0.f;
    float ReloadPhaselockTestEndsAt = 0.f;
    float ReloadPhaselockTestCastAt = 0.f;
    // Cooldown pool model (UNVERIFIED semantics, PHASELOCK_STOCK_DATA.md): refilled at activation, drained at the held
    // rate until PhaselockHeldUntil (the release), then at the base rate; a miss resets it at PhaselockResetAt.
    float PhaselockCastAt = -100.f;
    float PhaselockHeldUntil = -100.f;
    float PhaselockResetAt = -100.f;
    float PhaselockCooldownAdjust = 0.f;    // what ModifyPhaselockCooldown added to the pool's current value since the last cast (the cast refills it: 0)
    bool bPhaselockHit = false;
    bool bPhaselockBlocked = false;
    FOpenWillowPhaselockTimeline PhaselockTimeline;
    TWeakObjectPtr<class AOpenWillowCombatTarget> PhaselockTarget;
    FString ShotDamageTypeInFlight;
    FVector ShotMomentumInFlight = FVector::ZeroVector;
    // Pattern index, barrel spin and the host random stream between shots, per weapon (OpenWillowFiring).
    TMap<FString, FOpenWillowFiringState> FiringStates;
    FOpenWillowFiringState& FiringStateFor(const struct FOpenWillowWeaponItem& Weapon);
    // AutomaticBurstCount (ShouldRefire): a burst finishes its shots even if the trigger is let go, then waits for a
    // new press; 0 fires while the trigger is held.
    weapon_firing::Burst Burst;
    FString BurstWeaponId;     // the weapon the burst state belongs to
    void StartReloadThrow(const struct FOpenWillowWeaponItem& Weapon);
    void TracePellet(const FVector& Start, const FVector& Direction, const FVector& Muzzle);
    bool bWantsCombatTarget = false;
    bool bBarrelAxisLogged = false;
    // Muzzle fallbacks used (shots), and the item/socket pairs whose socket check was already logged.
    mutable int32 MuzzleFallbackShots = 0;
    mutable TSet<FString> GunSocketLogged;
    // Look-input weapon sway, in degrees (yaw, pitch).
    FVector2D LookInput = FVector2D::ZeroVector;
    FVector2D Sway = FVector2D::ZeroVector;
};
