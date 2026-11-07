#pragma once
#include "CoreMinimal.h"
#include "Blueprint/UserWidget.h"
#include "GameFramework/Character.h"
#include "OpenWillowPhaselock.h"
#include "OpenWillowPhaselockFx.h"
#include "OpenWillowFiring.h"
#include "OpenWillowHitEvent.h"
#include "OpenWillowMelee.h"
#include "OpenWillowShotFx.h"
#include "OpenWillowMayaState.h"
#include "OpenWillowRespawnFee.h"
#include "OpenWillowMayaMovement.h"
#include "OpenWillowWalker.generated.h"

struct FOpenWillowTakenInventoryItem;
struct FOpenWillowWeaponItem;

// Procedural scope mask and reticle for recipes whose selected sight carries a
// first-person particle component. Its match to the stock scope presentation is UNVERIFIED.
enum class EOpenWillowUseTarget : uint8; // OpenWillowInteractive.h

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
    // For the view-model test: the foreground FOV of Item's view model, composed if need be; negative when it has none.
    float ViewModelFovForTests(const struct FOpenWillowWeaponItem& Item) const;
    const class UOpenWillowInventory* GetInventory() const { return Inventory; }
    class UOpenWillowSkills* GetSkills() const { return Skills; }
    // Every use target in its own reach now (OpenWillowInteractive.h), and the nearest dropped item.
    struct FOpenWillowUseDistances UseDistances(class AOpenWillowInventoryPickup*& OutPickup) const;
    // Which one the press would go to first.
    EOpenWillowUseTarget NearestUseTarget() const;
    // The use key's own path, for tests that must go through its routing.
    void PressUseKey() { PickupNearby(); }
    // Puts backpack item Item into weapon slot Slot and draws it (inventory screen).
    bool EquipItem(int32 Item, int32 Slot);
    void SetInventoryPresentation(bool bShow);
    // Returns the weapon to the backpack and holsters it if it was active.
    bool UnequipSlot(int32 Slot);
    // Removes by stable instance ID and preserves the payload for a pickup actor.
    bool TakeInventoryItemById(const FString& Id, FOpenWillowTakenInventoryItem& OutItem);
    void ToggleInventory();
    // The use key's own check (PickupNearby), and the prompt's: false when a press would do nothing.
    bool CanUseNow() const { return bMayaActive && Inventory && !bInventoryPresentation && !IsDown(); }
    bool IsInventoryWidgetOpen() const; // the legacy UMG inventory, used when the HUD has no inventory page
    void ToggleSkills();
    // The map and missions keys (OpenWillowControls.cpp, docs/verification/CONTROLS.md).
    void ToggleMapPage();
    void ToggleMissionsPage();
    bool MeleeBusy() const;   // a swing holds the weapons until its clip ends
    // For the suite: when the last swing started, whether a burst owes shots, and presses the swing rules refused.
    float MeleeStartedAt() const { return MeleeSwing.StartedAt(); }
    bool BurstActive() const { return Burst.active(); }
    int32 MeleeRefusedByThrow = 0, GrenadesRefusedBySwing = 0, HolstersRefusedBySwing = 0, PagesRefusedWhileDown = 0;
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
    // Damage from one of Maya's shots, its projectile, splash or status tick: the one entry for every hit. The caller fills
    // Event's Kind, Damage, DamageType, Direction, Momentum, Origin, RadiusScale (and WeaponId when the shot was fired
    // earlier); this fills Instigator, Target, Location, Bone, Element, CritBonus, Time, sends it as the target's
    // FOpenWillowDamageEvent and then to OpenWillowHit::OnHit(). Returns the damage the target applied.
    // While it runs, ShotDamageType() and ShotMomentum() return the event's damage type and momentum (older readers).
    float ApplyShotDamage(AActor* Target, const FOpenWillowHitEvent& Event, const FHitResult& Hit);
    // Momentum of the shot being applied right now; zero otherwise.
    const FVector& ShotMomentum() const { return ShotMomentumInFlight; }
    // Phaselock from the action-skill manifest (-owactionskill=, default local/character/action_skill_siren.json).
    void UsePhaselock();
    // Throws the grenade mod's grenade (tools/grenade_mod.py block, -owgrenade=<json>, default
    // local/items/grenades/grenade_mod_standard_l8.json): one from the Grenade ammo pool, a falling projectile with the
    // mod's speed, gravity, bounces, fuse and explosion. False when nothing was thrown (no data, no grenade, busy).
    bool ThrowGrenade();
    void ThrowGrenadeKey() { ThrowGrenade(); }
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
    bool IsWeaponShown() const;
    float LastTargetHitAt() const { return TargetHitAt; }
    class UOpenWillowMover* GetMover() const { return Mover; }
    class UOpenWillowQuest* GetQuest() const { return Quest; }
    virtual float TakeDamage(float DamageAmount, struct FDamageEvent const& DamageEvent,
        class AController* EventInstigator, AActor* DamageCauser) override;
    // A teleport (respawn, fast travel, test drivers, the range lane): resets the fall tracking so the landing after it is not charged
    // for a fall that began elsewhere. A generic check in TickMovement backs this up for callers that do not call it.
    void NotifyTeleported();
    // Damage from the world (OpenWillowApplyDamageToPlayer, and TakeDamage for UE's ApplyDamage): shield first, then health,
    // then the down state. SourceLocation is where the hit came from (ZeroVector: unknown). Returns what the pools took.
    float ApplyDamageToPlayer(float Amount, AActor* Causer, const FVector& SourceLocation);
    // Health, shield and Fight For Your Life live in FOpenWillowMayaState (OpenWillowMayaState.h); the walker forwards.
    const FOpenWillowMayaState& GetMayaState() const { return MayaState; }
    FOpenWillowMayaState& GetMayaState() { return MayaState; }
    float GetHealth() const { return MayaState.Health; }
    float GetMaxHealth() const { return MayaState.Effective().MaxHealth; }
    float GetShield() const { return MayaState.Shield; }
    float GetShieldCapacity() const { return MayaState.Effective().ShieldCapacity; }
    // Skill healing (Sweet Release, regeneration): up to the effective maximum, not while down. Skills lane hook.
    void Heal(float Amount) { if (!IsDown() && Amount > 0.f) MayaState.Health = FMath::Min(GetMaxHealth(), MayaState.Health + Amount); }
    // ---- Fight For Your Life: the host's down state (rules and their live numbers: docs/verification/COMBAT_DOWN_RESPAWN.md) ----
    // Health reaching 0 puts Maya down (Injured) instead of respawning her at once. 2.28 s later the bleed-out timer starts
    // (12 s, cut for successive injuries); a kill while Injured is a second wind (health back to 25 % of the maximum); the end
    // of the bleed-out is death (Dead), and the respawn follows 5.76 s later. Read by the HUD (AOpenWillowMayaHUD) and the checks.
    using EDownPhase = EOpenWillowDownPhase;
    EDownPhase GetDownPhase() const { return MayaState.Phase; }
    bool IsDown() const { return MayaState.IsDown(); }
    // The bleed-out left as a fraction of its total: 1 until its timer starts, falling to 0 at death; 0 while Dead, 1 when up.
    float DownBleedLeft() const;
    // Whether the weapon is in hand for firing and reloading: always when up; while Injured only after the put-away special
    // move ends (2.28 s after the injury); never while Dead (see GoDown, UNVERIFIED).
    bool DownWeaponReady() const;
    bool IsReloading() const { return bReloading; }
    float ReloadRefillTime() const { return ReloadRefillAt; }   // the test's clock for the refill point
#if WITH_EDITOR
    // Test drivers (-owhuddownshots, which runs as UnrealEditor -game): the same handlers the fire and reload input bindings
    // call. Editor builds only, so Shipping has no way to inject input.
    void TestFireInput(bool bPressed) { if (bPressed) FirePressed(); else FireReleased(); }
    void TestReloadInput() { ReloadPressed(); }
#endif
    float DownBleedTotal() const { return MayaState.BleedSeconds; }
    float LastDownAt() const { return MayaState.DownAt; }
    float LastSecondWindAt() const { return MayaState.SecondWindAt; }
    float LastRespawnAt() const { return MayaState.RespawnAt; }
    // The fee and message after a respawn (the HUD draws the message; the suite reads the fee).
    const FOpenWillowRespawnFee& GetRespawnFee() const { return RespawnFee; }
    FOpenWillowRespawnFee& GetRespawnFee() { return RespawnFee; }
    // Hits that took health and came from a known place (the HUD's damage-direction arrows): how many so far, and the world
    // direction from Maya toward the last one's source (a point hit's shot direction reversed, else the causer's or the
    // instigator's position).
    int32 GetHitCount() const { return HitsTaken; }
    FVector GetLastHitFrom() const { return LastHitFrom; }
    // The slice gate only (off in normal play; switching either way clears it): how many of those hits each causer made
    // (null: none given). Several can land in one tick, so a last-causer read misses some.
    void SetHitCauserTracking(bool bOn) { bTrackHitCausers = bOn; HitsByCauser.Reset(); }
    int32 GetHitCountFrom(const AActor* Causer) const { const int32* Count = HitsByCauser.Find(TObjectKey<AActor>(Causer)); return Count ? *Count : 0; }
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
    void CrouchPressed();
    void CrouchReleased();
    // Maya's movement feel each tick (NATIVE_PLAYER_MOVEMENT.md, OpenWillowMayaMovement.h): speeds from the stats, the sprint
    // state machine, the eye height and bob on the camera, the sprint FOV (read by UpdateAim). Maya mode only.
    void TickMovement(float Now, float DeltaSeconds);
    OpenWillowMovement::FSprintInput SprintInputNow() const;
    void LogSprint(bool bOn, const TCHAR* Why) const;
    virtual void Landed(const FHitResult& Hit) override;
    void ApplyMayaMovementDefaults();
    void JumpPressed();
    void SetMayaCrouched(bool bCrouch);
    void SelectSlot(int32 Slot);
    void Holster();
    void ToggleMapOrMissionsPage(bool bMap);
    // The melee key (OpenWillowMelee.cpp).
    void MeleePressed();
    void TickMelee();
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
    // -owreloadtest (with -owlevel and -owitems): a gun that spends rounds, fired until it reloads; one OWRELOAD result line.
    void RunReloadTest(float Now);
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
    // Health, shield and the down state. Host stand-ins (UNVERIFIED) unless -owquest supplies the slice data: then health
    // follows the recovered formula and respawn the decoded station selection (UOpenWillowQuest).
    FOpenWillowMayaState MayaState;
    FString EquippedShieldId;           // the shield MayaState was last given (TickShield)
    bool bShieldEverEquipped = false;
    void TickShield(float Now);
    FOpenWillowStandInShieldRule StandInShieldRule;
    bool bStandInShieldRuleRead = false;
public:
    // What a shield item gives: its card's stats, the stand-in rule's capacity when the card has none, the stats' defaults otherwise.
    struct FShieldStats { float Capacity = 0.f; float RechargeDelay = 0.f; float RechargeRate = 0.f; bool bStandIn = false; };
    static FShieldStats ReadShieldStats(const struct FOpenWillowGearItem* Item, const FOpenWillowStandInShieldRule& Rule);
private:
    int32 HealthLevel = 0;              // the level MaxHealth was last set for (0: not yet)
    bool bHealthHeldForScript = false;  // a script level-up is pending: RefreshHealthForLevel does nothing
    struct FDamageScale { float Scale = 0; float EndsAt = 0; };
    TArray<FDamageScale> DamageScales;  // the running weapon damage Scale modifiers (AddWeaponDamageScale); ended ones are dropped by Tick
    FVector RespawnLocation = FVector::ZeroVector;
    FRotator RespawnRotation = FRotator::ZeroRotator;
    bool bRespawnPointCaptured = false;
    void Respawn();
    FOpenWillowRespawnFee RespawnFee;
    int32 HitsTaken = 0;
    FVector LastHitFrom = FVector::ZeroVector;
    bool bTrackHitCausers = false;
    TMap<TObjectKey<AActor>, int32> HitsByCauser;
    void OnWentDown();
    void TickDown(float Now, float DeltaSeconds);
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
    UPROPERTY() TObjectPtr<class UAnimSequence> MeleeAnim;
    OpenWillowMelee::FSwing MeleeSwing;
    TWeakObjectPtr<class AOpenWillowCombatTarget> CombatTarget;
    bool bSprintHeld = false;           // sprint key down
    OpenWillowMovement::FSprint Sprint;  // the script's sprint state machine (press, wait for the cone, want, active)
    bool bCrouchHeld = false;
    bool bMayaCrouched = false;         // the host's crouch (SetMayaCrouched): cylinder 21 x 50
    OpenWillowMovement::FFallTimer FallTimer;
    FVector PreviousLocation = FVector::ZeroVector;   // last tick's location, for the teleport check
    bool bTeleportedInFall = false;
    float MoveForwardInput = 0.f, MoveRightInput = 0.f;   // this frame's move axes
    OpenWillowMovement::FBob WalkBob;
    OpenWillowMovement::FEyeHeight EyeState;
    float BaseFovNow = 0.f;             // the world FOV before zoom: WorldFovDefault, eased toward the sprint FOV while sprinting
    float LastFallVz = 0.f, JumpStartZ = 0.f, JumpApexZ = 0.f, FallStartedAt = 0.f;   // the jump arc logged at landing
    bool bArcActive = false;
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
    int32 GunShotDropTicks = 0;  // -owdropshots: ticks left before the gun sequence moves on
    float GunShotFinishedAt = -1.f;
    TWeakObjectPtr<class AOpenWillowInventoryPreviewActor> GunShotPreview;
    float NextShotAt = 0;
    bool bReloading = false;
    int32 CombatShotsBeforeFiring = 0; // -owcombatshots: the shot count when its firing exercise starts
    int32 CombatMagazineAtFiring = 0;  // -owcombatshots: the magazine read back after setting it, which clamps to the gun's size
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
        // The type's muzzle-flash particle emitters per element ("Default", "Fire", "Shock", ...), from weapon_view.json
        // "muzzle_flash_fx"; empty for older manifests (the procedural MuzzleFlash is used then).
        TMap<FString, TArray<FOpenWillowFlashLayer>> FlashLayers;
        // The type's shell-casing particle motion (weapon_view.json "shell_casing_fx"); speeds in cm/s.
        bool bCasingFx = false;
        float CasingLifeMin = 0.22f, CasingLifeMax = 0.22f;
        float CasingSpeedMin = 200.f, CasingSpeedMax = 200.f;
        float CasingGravityMin = 980.f, CasingGravityMax = 980.f;
        float TracerRadius = 0.8f;      // UNVERIFIED host stand-in; no type tracer template is present in the measured slice.
        float TracerLife = 0.09f;       // UNVERIFIED host stand-in; no type tracer template is present in the measured slice.
        FLinearColor TracerColor = FLinearColor(200.f / 255.f, 200.f / 255.f, 200.f / 255.f, 1.f);
        float RecoilWeight = 0.45f;
        float ZoomedRecoilScale = 1.f;
        FString ReloadAsset;
        FString ReloadAnimation;
        FString ArmsAnimSet;
    };
    mutable TMap<FString, FViewModel> ViewModels;
    FString HeldArmsSet = TEXT("Pistol");  // the clip set chosen at equip; a reload plays from the same one
    bool bViewModelActive = false;      // foreground FOV on (the default; -owfpfov=0 turns it and the offsets off)
    bool bViewFovForced = false;        // -owfpfov=<n> given: that value for every weapon
    float ViewFovDefault = 45.f;        // horizontal degrees
    float FlashTextureFill = 0.5f;      // UNVERIFIED host fit: share of a cooked flash quad the real sprite covers
    void LoadViewModels(const FString& ItemDir);
    void AddViewModels(const TSharedPtr<class FJsonObject>& Root) const;
    // The item's view model: its weapon_view.json line, else one composed by weapon type and parts (OpenWillowViewTypes), kept by id.
    FViewModel* FindViewModel(const struct FOpenWillowWeaponItem* Item) const;
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
    // The accuracy pool and the view kick (OpenWillowFiring::TickAim / ShotAim / OnShot). Off in every scripted test or
    // capture mode (OpenWillowFiring::AimModelEnabled) unless -owaimmodel is given, so scripted shots keep their aim.
    FOpenWillowAimState AimState;
    // The grenade mod's grenade (ThrowGrenade), loaded on first use; ThrowLockUntil holds the next throw back and
    // GrenadeReleaseAt is when a pressed throw leaves the hand (0: none pending).
    FOpenWillowFiringData GrenadeData;
    bool bGrenadeTried = false;
    float ThrowLockUntil = -100.f;
    float GrenadeReleaseAt = 0.f;
    void ReleaseGrenade();
    FRandomStream AimRng{0x41AC};
    bool bAimModel = true;
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
