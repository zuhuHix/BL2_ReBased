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
    // Auto-aim strategy data (GD_Autoaim.Default via WillowGlobals.AutoAimDefinition): Min/MaxTargetDistance. The game
    // picks the target natively (WillowAutoAimStrategy.GetPreferredTarget); the host uses these as the range of its
    // own view-ray selection, which is UNVERIFIED as a reading of what the native code does with them.
    float TargetMinDistance = 0, TargetMaxDistance = 0;
    // Skill_Phaselock.SkillConstraints applied at activation whose evaluator class the host has state for, in data
    // order (GateOpen), and the classes it cannot evaluate. Every Evaluate() is native: each mapping is UNVERIFIED.
    TArray<FString> GateEvaluators;
    TArray<FString> GateNotEvaluated;
    // CanLiftTargetIf's flag (Flag_Skills_CanPhaseLock): the host target's "can be phaselocked" property stands for it.
    FString CanLiftFlag;

    bool Load(const FString& File, FString& OutError);
    // The activation constraints the host evaluates, in data order: a weapon action in progress (host: reloading)
    // fails WeaponActionAvailable, health at 0 fails HealthState bHealthy, and VehiclePassenger always passes (the host
    // has no vehicles). False with the failing evaluator class in OutFailed.
    bool GateOpen(bool bWeaponActionBusy, float Health, FString& OutFailed) const;
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
