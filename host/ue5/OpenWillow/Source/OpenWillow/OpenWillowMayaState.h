// AI-assisted (Claude). Maya's survival state in the host: health, shield and Fight For Your Life (the down state).
// Plain C++ (no UObject, no world): the walker owns one FOpenWillowMayaState, passes game time in, and does the
// presentation (weapon put-away, teleport, logs). The rules and their live numbers are in
// docs/verification/COMBAT_DOWN_RESPAWN.md ("The host's state"); the synthetic checks are in OpenWillowMayaStateTest.cpp.
#pragma once
#include "CoreMinimal.h"

class AActor;
class UObject;

// Every number the survival rules read, in one struct. `Base` is what the data gives (level formula, equipped shield,
// the injured definition); the skills lane applies its modifiers in FOpenWillowMayaState::Effective(), the one place the
// rules read stats from. Defaults are host stand-ins (UNVERIFIED unless the comment names the live read).
struct OPENWILLOW_API FOpenWillowMayaStats
{
    float MaxHealth = 400.f;              // replaced by the level formula with -owquest
    float HealthRegenRate = 0.f;          // per second; live: 0 (health did not regenerate, COMBAT_GOLDEN_TRACE.md step 10)
    float ShieldCapacity = 0.f;           // 0: no shield equipped
    float ShieldRechargeDelay = 1.611f;   // s after the last hit; live read for one shield (step 10), the stand-in when the card has none
    float ShieldRechargeRate = 25.88f;    // per second; live read for the same shield (step 10), the stand-in when the card has none
    float BleedoutMultiplier = 1.f;       // TimeToBeRevivedMultiplier (attribute default 1)
    float SecondWindHealthPct = 0.25f;    // RejuvenatedHealthPctOfMax (data 0.25; live 136.97 = 0.25 x 547.874)
    float RevivalHealthMultiplier = 1.f;  // the attribute, 1 at its default
    float SecondWindShieldPct = 0.9f;     // live: the shield went to 90 % at a second wind (COMBAT_GOLDEN_TRACE.md section D)
    float RespawnHealthPct = 1.f;         // ResurrectedHealthPctOfMax (data 1.0)
    // Movement (NATIVE_PLAYER_MOVEMENT.md section 1, cooked values; the rules are in OpenWillowMayaMovement.h).
    float GroundSpeed = 440.f;            // CharacterClassDefinition default (FootSpeed resolves to it)
    float SprintSpeedScale = 0.35f;       // SprintDefinition_Default: FootSpeed MT_Scale +0.35 (594 while sprinting)
    float CrouchedPct = 0.5f;             // CrouchedPct (220 crouched)
    float JumpZ = 630.f;                  // CharClass_Siren
    float AirControl = 0.11f;             // GD_Globals PlayerAirControl
};

enum class EOpenWillowDownPhase : uint8 { Up, Injured, Dead };

struct FOpenWillowDamageResult
{
    float ToShield = 0.f;
    float ToHealth = 0.f;
    bool bWentDown = false;
    bool bIgnored = false;   // down already: live, damage reaches the injured state but health does not move
};

class OPENWILLOW_API FOpenWillowMayaState
{
public:
    // Live numbers of COMBAT_GOLDEN_TRACE.md section D, reproduced by the VM's run of the installed script.
    static constexpr float TargetedDelay = 2.28f;      // the weapon put-away special move ends; the bleed-out timer starts
    static constexpr float BaseBleedout = 12.f;        // BaseRejuvenateDelay for one player
    static constexpr float SuccessiveWindow = 30.f;    // MaxTimeBetweenInjuries
    static constexpr int32 MaxSuccessive = 3;          // MaxDesiredSuccessiveInjuries
    static constexpr float DeathCamera = 5.76f;        // dead state to the respawn

    FOpenWillowMayaStats Base;
    // The stats the rules use: Base with the skills lane's modifiers (none yet).
    FOpenWillowMayaStats Effective() const { return Base; }

    float Health = 400.f;
    float Shield = 0.f;
    EOpenWillowDownPhase Phase = EOpenWillowDownPhase::Up;
    // LastInjuryAt starts far in the past: the game's default for its counterpart is not known (UNVERIFIED); this keeps a
    // first injury early in a session uncut.
    float DownAt = -1000.f, BleedSeconds = BaseBleedout, DeadAt = -1000.f, SecondWindAt = -1000.f, RespawnAt = -1000.f;
    float LastInjuryAt = -1000.f, LastDamageAt = -1000.f;
    int32 SuccessiveInjuries = 0;

    // Shield first, the remainder to health; health 0 puts Maya down. Amount is the damage as it arrives at the pools
    // (the stock pipeline's resistances and scales are the caller's: not modelled here).
    FOpenWillowDamageResult ApplyDamage(float Amount, float Now);

    enum class ETickEvent : uint8 { None, BledOut, RespawnDue };
    // Shield recharge (and health regeneration when the rate is above 0) while up, the bleed-out while Injured, the death
    // camera while Dead. RespawnDue asks the caller to move Maya and call Respawn.
    ETickEvent Tick(float Now, float DeltaSeconds);

    // A kill while Injured: health to at least SecondWindHealthPct x RevivalHealthMultiplier of the maximum, the shield to
    // at least SecondWindShieldPct of its capacity. Returns false (nothing changes) when not Injured.
    bool SecondWind(float Now);
    // Full health and shield, the successive-injury counter reset (ResurrectPlayer), up again.
    void Respawn(float Now);
    // New maximum and current health (a level change). A refill while Injured ends the down state like a second wind
    // (widely reported in game, UNVERIFIED). Returns true when it did.
    bool SetHealth(float NewMax, float NewCurrent, float Now);
    // An equipped shield (capacity 0: none). bStartFull fills it (the session's first shield); otherwise it starts empty and
    // recharges after the delay (host rule for a swap, UNVERIFIED).
    void EquipShield(float Capacity, float RechargeDelay, float RechargeRate, bool bStartFull, float Now);

    bool IsDown() const { return Phase != EOpenWillowDownPhase::Up; }
    // Weapon in hand: always when up; while Injured only after the put-away special move ends; never while Dead.
    bool WeaponReady(float Now) const;
    // The bleed-out left as a fraction: 1 until its timer starts, falling to 0 at death; 0 while Dead, 1 when up.
    float BleedLeft(float Now) const;
    float HealthFraction() const;
    // 0 with no shield.
    float ShieldFraction() const;

private:
    void GoDown(float Now);
};

// The entry point for anything in the world that hurts the player (the range lane's enemies, hazards): finds the player
// walker of WorldContext's world and applies Amount through its damage path (shield, health, down state, the HUD's hit
// direction). SourceLocation is where the hit came from (ZeroVector: unknown, the causer's position is used). Returns the
// damage the pools took (shield plus health); 0 when there is no player or she is already down.
OPENWILLOW_API float OpenWillowApplyDamageToPlayer(const UObject* WorldContext, float Amount, AActor* Causer = nullptr,
    const FVector& SourceLocation = FVector::ZeroVector);
