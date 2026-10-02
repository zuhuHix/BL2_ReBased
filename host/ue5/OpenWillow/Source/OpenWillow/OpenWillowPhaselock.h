#pragma once
#include "CoreMinimal.h"

// One Phaselock cast's timeline, in seconds after the cast (LiftActionSkill script, read not run; UNVERIFIED).
struct FOpenWillowPhaselockTimeline
{
    float SkillDuration = 0;   // LiftDuration + LockDuration x TimeScale(target)
    float LockedAt = 0;        // LockTarget timer: LiftDuration
    float OutroAt = 0;         // StartOutro timer: SkillDuration - LockFadeOutTime
    float ReleasedAt = 0;      // ReleaseTarget after the outro: SkillDuration
    float EndSkillAt = 0;      // EndSkill timer: SkillDuration + ReleaseBufferTime
};

// Maya's state as the native constraint evaluators of Skill_Phaselock.SkillConstraints read it
// (docs/verification/NATIVE_PHASELOCK_TARGETING.md section 4; native readings, UNVERIFIED until a game check).
struct FOpenWillowPhaselockGateState
{
    // WeaponActionAvailableExpressionEvaluator -> WillowPlayerController.CanPerformWeaponAction: false while weapons are
    // restricted, while a shared weapon action (melee or grenade style) runs, or when the held weapon cannot act: it is
    // being put away or is inactive (holstered). Reloading does NOT block (the weapon's CanPerformAction is true then).
    bool bWeaponsRestricted = false;
    bool bHoldsWeapon = false;
    bool bWeaponPuttingDown = false;
    bool bWeaponInactive = false;
    bool bSharedWeaponAction = false;
    bool bReloading = false;            // carried for logs and checks only
    // HealthStateExpressionEvaluator bHealthy -> Pawn.IsAliveAndWell, read as alive and not injured (Fight For Your Life).
    bool bHasPawn = true;
    bool bAlive = true;
    bool bInjured = false;
    // VehiclePassengerExpressionEvaluator bNotInVehicle: the host has no vehicles.
    bool bInVehicle = false;
};

// Phaselock numbers read at run time from the ignored manifest local/character/action_skill_siren.json
// (tools/prepare_action_skill.py; record docs/verification/PHASELOCK_STOCK_DATA.md). Nothing is compiled in: without
// the manifest Phaselock is unavailable rather than falling back to invented values.
struct FOpenWillowPhaselockData
{
    bool bLoaded = false;
    // ActionSkill_Phaselock settings.
    float LiftDuration = 0, LockFadeOutTime = 0, ReleaseBufferTime = 0;
    float SnapTimePct = 0, SnapHeightPct = 0, BobAmplitude = 0, BobFrequency = 0;
    // Default__PhaseLockDefinition (PhaseLockDef_Default does not override them).
    float HeightFromGround = 0, DropTime = 0;
    // Att_Phaselock_Duration on Maya and the target's PhaselockTimeScale default.
    float LockDurationBase = 0, TimeScaleDefault = 0;
    // Cooldown pool BaseMaxValue (Cooldown_Phaselock), drained at BaseConsumptionRate per second.
    float CooldownSeconds = 0, CooldownRate = 0;
    // Drain rate while the target is held: Skill_Phaselock_CooldownManager (on from OnSelectedTarget to OnReleasedTarget)
    // PreAdds to the consumption rate. With the data's -1 this is 0, i.e. the cooldown is paused.
    float CooldownHeldRate = 0;
    // Skill_Phaselock_DiminishingReturns on the target: MT_Scale on PhaselockTimeScale for its InitialDuration.
    float DiminishingSeconds = 0, DiminishingScale = 0;
    // The skill on the upgrade path that adds to Att_Phaselock_Duration (Suspension): MT_PostAdd value per grade.
    FString DurationSkill;
    TArray<float> DurationPostAdd;      // index = grade; [0] is 0
    // Auto-aim strategy data (GD_Autoaim.Default via WillowGlobals.AutoAimDefinition), used by the host's reading of
    // WillowAutoAimStrategy.GetPreferredTarget (native; NATIVE_PHASELOCK_TARGETING.md sections 2-3, UNVERIFIED).
    float TargetMinDistance = 0, TargetMaxDistance = 0;
    float MaxSnapAngle = 0, RadiusMultiplier = 0, DistanceOffset = 0;
    // Skill_Phaselock.SkillConstraints whose evaluator class the host has state for, in data order, with the data's
    // flags for when each applies (GateOpen), and the classes it cannot evaluate. Every Evaluate() is native.
    struct FConstraint
    {
        FString Class;
        bool bOnActivation = false;
        bool bWhileActive = false;
    };
    TArray<FConstraint> Constraints;
    TArray<FString> GateEvaluators;      // classes applied at activation (for logs)
    TArray<FString> GateNotEvaluated;
    // CanLiftTargetIf's flag (Flag_Skills_CanPhaseLock): the host target's "can be phaselocked" property stands for it.
    FString CanLiftFlag;

    bool Load(const FString& File, FString& OutError);
    // The constraints that apply now, in data order: at activation (bActivation) those flagged for activation, else
    // those flagged while active. The first failing one (all fail to SKILL_Deactivated in this data) ends the check:
    // false with its evaluator class in OutFailed.
    bool GateOpen(const FOpenWillowPhaselockGateState& State, bool bActivation, FString& OutFailed) const;
    // One evaluator's reading (true = holds).
    static bool EvaluatorHolds(const FString& Class, const FOpenWillowPhaselockGateState& State);
    // Lock duration attribute at that grade of DurationSkill: (base + PreAdd) x (1 + Scale) + PostAdd, the rule fitted
    // for weapons (UNVERIFIED for skills). Only a PostAdd exists on this path.
    float LockDuration(int32 DurationSkillGrade) const;
    FOpenWillowPhaselockTimeline Timeline(float LockDuration, float TargetTimeScale) const;
    // The target's time scale with or without the diminishing-returns modifier.
    float TargetTimeScale(bool bDiminished) const
    {
        return bDiminished ? TimeScaleDefault * (1.f + DiminishingScale) : TimeScaleDefault;
    }
};
