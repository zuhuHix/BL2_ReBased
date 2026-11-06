#pragma once
#include "CoreMinimal.h"

// Maya's melee (docs/verification/CONTROLS.md, "Melee"): plain rules with no engine state, so the test can check them directly.
// Numbers are read from the game's data, not confirmed by a live swing; UNVERIFIED in game until the record says otherwise.
namespace OpenWillowMelee
{
    // BehaviorData_PlayerMeleeAttack_Siren.
    constexpr float TraceDistance = 330.f;
    constexpr float ConeAngleDegrees = 40.f;     // used as the angle itself, not halved (NATIVE_AI_MELEE.md, low confidence)
    constexpr float HitStartOffset = -30.f;      // along the attacker's forward axis
    constexpr float Knockback = 0.1f;
    constexpr float KnockbackScale = 100000.f;   // live for the bullymong's radius melee (AI_MELEE_TARGETING_LIVE.md); UNVERIFIED here
    // WeaponAnim_Melee: the hit lands this long after the press. The Siren's move says 0.30 and the shared one 0.20; which one Maya
    // plays is unread, and the Siren's is the one naming her melee definition.
    constexpr float HitDelay = 0.30f;
    // The first-person move's blends (GD_PlayerShared.Anims1st.WeaponAnim_Melee).
    constexpr float BlendIn = 0.1f;
    constexpr float BlendOut = 0.15f;
    // Host choice, used only without the clip: the game holds the weapons for the clip's length, which is not read yet.
    constexpr float NoClipDuration = 1.f;

    // Init_PlayerMeleeDamage: 2.5 x (8 x 1.13^level). The buff the definition adds (InstigatedMeleeDamageAddedBuff) is not modelled.
    float DamageAtLevel(int32 Level);

    // One possible victim, as ChooseVictim sees it.
    struct FCandidate
    {
        FVector Centre = FVector::ZeroVector;
        bool bClearLine = false;   // nothing solid between the swing's origin and the candidate
    };

    // The cone path, when the straight trace hit nothing: the index of the candidate most in line with Forward, within TraceDistance
    // and ConeAngleDegrees and with a clear line, or INDEX_NONE. A tie keeps the earlier candidate.
    int32 ChooseVictim(const FVector& Origin, const FVector& Forward, const TArray<FCandidate>& Candidates);

    // One swing's timing. Press starts it; ConsumeHit is true once, when the hit is due; a press during a swing does nothing, as the
    // game refuses a weapon action during a shared one (WillowPlayerController.CanPerformWeaponAction).
    class FSwing
    {
    public:
        bool Press(float Now, float Duration);
        bool ConsumeHit(float Now);
        bool IsBusy(float Now) const { return Now < EndsAt; }
        float StartedAt() const { return HitAt - HitDelay; }
        void Reset() { EndsAt = -1.f; HitAt = -1.f; bHitPending = false; }
    private:
        float EndsAt = -1.f;
        float HitAt = -1.f;
        bool bHitPending = false;
    };
}
