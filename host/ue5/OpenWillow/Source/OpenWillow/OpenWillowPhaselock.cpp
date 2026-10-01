#include "OpenWillowPhaselock.h"
#include "Dom/JsonObject.h"
#include "Misc/FileHelper.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

namespace
{
const TCHAR* const DiminishingSkill = TEXT("GD_Siren_Skills.Phaselock.Skill_Phaselock_DiminishingReturns");
const TCHAR* const CooldownManagerSkill = TEXT("GD_Siren_Skills.Phaselock.Skill_Phaselock_CooldownManager");
// Constraint evaluator classes the host maps to its own state (GateOpen). Their Evaluate() is native.
const TCHAR* const WeaponActionEvaluator = TEXT("WillowGame.WeaponActionAvailableExpressionEvaluator");
const TCHAR* const HealthStateEvaluator = TEXT("WillowGame.HealthStateExpressionEvaluator");
const TCHAR* const VehiclePassengerEvaluator = TEXT("WillowGame.VehiclePassengerExpressionEvaluator");

// The single value of a helper skill's effect on Attribute with ModifierType (helpers carry one value per grade, all
// equal in this data; anything else is reported, not guessed).
bool HelperValue(const TSharedPtr<FJsonObject>& Skill, const FString& Attribute, const TCHAR* Modifier, float& Out)
{
    for (const auto& EffectValue : Skill->GetArrayField(TEXT("effects")))
    {
        const auto Effect = EffectValue->AsObject();
        if (Effect->GetStringField(TEXT("attribute")) != Attribute || Effect->GetStringField(TEXT("modifierType")) != Modifier) continue;
        TOptional<double> Value;
        for (const auto& V : Effect->GetArrayField(TEXT("values")))
        {
            double N = 0;
            if (!V->TryGetNumber(N)) continue;   // null below startGrade
            if (Value.IsSet() && *Value != N) return false;
            Value = N;
        }
        if (!Value.IsSet()) return false;
        Out = float(*Value);
        return true;
    }
    return false;
}
}

bool FOpenWillowPhaselockData::Load(const FString& File, FString& OutError)
{
    *this = FOpenWillowPhaselockData();
    FString Text;
    TSharedPtr<FJsonObject> Root;
    if (!FFileHelper::LoadFileToString(Text, *File)) { OutError = TEXT("cannot read ") + File; return false; }
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root
        || Root->GetStringField(TEXT("format")) != TEXT("openwillow.action_skill/2"))
    {
        OutError = TEXT("not an openwillow.action_skill/2 manifest (regenerate it with tools/prepare_action_skill.py): ") + File;
        return false;
    }
    const TSharedPtr<FJsonObject>* Action = nullptr;
    const TSharedPtr<FJsonObject>* Settings = nullptr;
    const TSharedPtr<FJsonObject>* Cooldown = nullptr;
    const TSharedPtr<FJsonObject>* Helpers = nullptr;
    const TSharedPtr<FJsonObject>* Upgrade = nullptr;
    const TSharedPtr<FJsonObject>* Skill = nullptr;
    const TSharedPtr<FJsonObject>* AutoAim = nullptr;
    const TSharedPtr<FJsonObject>* AimSettings = nullptr;
    if (!Root->TryGetObjectField(TEXT("actionSkill"), Action) || !(*Action)->TryGetObjectField(TEXT("settings"), Settings)
        || !Root->TryGetObjectField(TEXT("cooldown"), Cooldown) || !Root->TryGetObjectField(TEXT("helperSkills"), Helpers)
        || !Root->TryGetObjectField(TEXT("upgradePath"), Upgrade) || !Root->TryGetObjectField(TEXT("skill"), Skill)
        || !Root->TryGetObjectField(TEXT("autoAim"), AutoAim) || !(*AutoAim)->TryGetObjectField(TEXT("settings"), AimSettings))
    {
        OutError = TEXT("manifest lacks actionSkill/settings, cooldown, helperSkills, upgradePath, skill or autoAim/settings");
        return false;
    }
    bool bOk = true;
    auto Read = [&bOk](const TSharedPtr<FJsonObject>& Object, const TCHAR* Key, float& Out)
    {
        double N = 0;
        if (!Object->TryGetNumberField(Key, N) || !FMath::IsFinite(N)) bOk = false;
        Out = float(N);
    };
    Read(*Settings, TEXT("LiftDuration"), LiftDuration);
    Read(*Settings, TEXT("LockFadeOutTime"), LockFadeOutTime);
    Read(*Settings, TEXT("ReleaseBufferTime"), ReleaseBufferTime);
    Read(*Settings, TEXT("LiftSnapTimePct"), SnapTimePct);
    Read(*Settings, TEXT("LiftSnapHeightPct"), SnapHeightPct);
    Read(*Settings, TEXT("LiftBobAmplitude"), BobAmplitude);
    Read(*Settings, TEXT("LiftBobFrequency"), BobFrequency);
    const auto Definitions = (*Action)->GetObjectField(TEXT("phaseLockDefinitions"))->GetObjectField(TEXT("classDefaults"));
    Read(Definitions, TEXT("HeightFromGround"), HeightFromGround);
    Read(Definitions, TEXT("DropTime"), DropTime);
    const auto Duration = (*Action)->GetObjectField(TEXT("lockDuration"));
    Read(Duration, TEXT("base"), LockDurationBase);
    Read((*Action)->GetObjectField(TEXT("lockDurationScale")), TEXT("default"), TimeScaleDefault);
    Read(*Cooldown, TEXT("seconds"), CooldownSeconds);
    Read(*Cooldown, TEXT("baseConsumptionRate"), CooldownRate);
    Read(*AimSettings, TEXT("MinTargetDistance"), TargetMinDistance);
    Read(*AimSettings, TEXT("MaxTargetDistance"), TargetMaxDistance);
    if (!bOk) { OutError = TEXT("a Phaselock number is missing from the manifest"); return false; }
    if (TargetMaxDistance <= TargetMinDistance)
    {
        OutError = TEXT("auto-aim MaxTargetDistance is not above MinTargetDistance");
        return false;
    }

    // Activation constraints. Only the evaluator classes and property shapes present in this data are mapped; any
    // other constraint is listed as not evaluated rather than guessed.
    const TArray<TSharedPtr<FJsonValue>>* Constraints = nullptr;
    if (!(*Skill)->TryGetArrayField(TEXT("constraintEvaluators"), Constraints))
    {
        OutError = TEXT("manifest lacks skill.constraintEvaluators");
        return false;
    }
    for (const auto& Value : *Constraints)
    {
        const auto Row = Value->AsObject();
        bool bOnActivation = false;
        if (!Row->TryGetBoolField(TEXT("onActivation"), bOnActivation) || !bOnActivation) continue;
        const FString Class = Row->GetStringField(TEXT("class"));
        const auto Properties = Row->GetObjectField(TEXT("properties"));
        bool bFlag = false;
        const bool bMapped = (Class == WeaponActionEvaluator && Properties->Values.Num() == 0)
            || (Class == HealthStateEvaluator && Properties->Values.Num() == 1 && Properties->TryGetBoolField(TEXT("bHealthy"), bFlag) && bFlag)
            || (Class == VehiclePassengerEvaluator && Properties->TryGetBoolField(TEXT("bNotInVehicle"), bFlag) && bFlag);
        (bMapped ? GateEvaluators : GateNotEvaluated).Add(Class);
    }

    // CanLiftTargetIf: one FLAG_IsTrue test of Flag_Skills_CanPhaseLock in this data; any other shape is not modelled.
    const TSharedPtr<FJsonObject>* LiftIf = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* LiftIfChain = nullptr;
    if (!(*Action)->TryGetObjectField(TEXT("canLiftTargetIfChain"), LiftIf) || !(*LiftIf)->TryGetArrayField(TEXT("chain"), LiftIfChain)
        || LiftIfChain->Num() != 1 || (*LiftIfChain)[0]->AsObject()->GetStringField(TEXT("test")) != TEXT("FLAG_IsTrue"))
    {
        OutError = TEXT("CanLiftTargetIf is not a single FLAG_IsTrue flag test; not modelled");
        return false;
    }
    CanLiftFlag = (*LiftIfChain)[0]->AsObject()->GetStringField(TEXT("flag"));

    // Helper skills: the diminishing-returns modifier on the target and the cooldown manager's rate change.
    const TSharedPtr<FJsonObject>* Diminishing = nullptr;
    const TSharedPtr<FJsonObject>* Manager = nullptr;
    const FString TimeScaleAttribute = (*Action)->GetObjectField(TEXT("lockDurationScale"))->GetStringField(TEXT("attribute"));
    float ManagerPreAdd = 0;
    if (!(*Helpers)->TryGetObjectField(DiminishingSkill, Diminishing) || !(*Helpers)->TryGetObjectField(CooldownManagerSkill, Manager)
        || !(*Diminishing)->TryGetNumberField(TEXT("initialDuration"), DiminishingSeconds)
        || !HelperValue(*Diminishing, TimeScaleAttribute, TEXT("MT_Scale"), DiminishingScale)
        || !HelperValue(*Manager, TEXT("D_Attributes.ActiveSkillCooldownResource.ActiveSkillCooldownConsumptionRate"), TEXT("MT_PreAdd"), ManagerPreAdd))
    {
        OutError = TEXT("diminishing-returns or cooldown-manager helper data missing or not single-valued");
        return false;
    }
    CooldownHeldRate = FMath::Max(0.f, CooldownRate + ManagerPreAdd);

    // The upgrade-path skill that changes the lock duration attribute (Suspension in this data).
    const FString DurationAttribute = Duration->GetStringField(TEXT("attribute"));
    for (const auto& Tier : (*Upgrade)->GetObjectField(TEXT("branch"))->GetArrayField(TEXT("tiers")))
        for (const auto& SkillValue : Tier->AsObject()->GetArrayField(TEXT("skills")))
            for (const auto& EffectValue : SkillValue->AsObject()->GetArrayField(TEXT("effects")))
            {
                const auto Effect = EffectValue->AsObject();
                if (Effect->GetStringField(TEXT("attribute")) != DurationAttribute) continue;
                if (Effect->GetStringField(TEXT("modifierType")) != TEXT("MT_PostAdd") || !DurationSkill.IsEmpty())
                {
                    OutError = TEXT("lock duration is changed by more than one PostAdd effect; not modelled");
                    return false;
                }
                DurationSkill = SkillValue->AsObject()->GetStringField(TEXT("id"));
                for (const auto& V : Effect->GetArrayField(TEXT("values")))
                {
                    double N = 0;
                    DurationPostAdd.Add(V->TryGetNumber(N) ? float(N) : 0.f);   // null = grade 0
                }
            }
    bLoaded = true;
    return true;
}

bool FOpenWillowPhaselockData::GateOpen(bool bWeaponActionBusy, float Health, FString& OutFailed) const
{
    for (const FString& Class : GateEvaluators)
    {
        // Host readings of native evaluators (UNVERIFIED): a reload is the only timed weapon action the host has; health
        // above 0 stands for "healthy" (the host has no injured state); the host has no vehicles, so Maya is on foot.
        const bool bMet = Class == WeaponActionEvaluator ? !bWeaponActionBusy
            : Class == HealthStateEvaluator ? Health > 0.f
            : true;
        if (!bMet)
        {
            OutFailed = Class;
            return false;
        }
    }
    return true;
}

float FOpenWillowPhaselockData::LockDuration(int32 Grade) const
{
    return LockDurationBase + (DurationPostAdd.IsValidIndex(Grade) ? DurationPostAdd[Grade] : 0.f);
}

FOpenWillowPhaselockTimeline FOpenWillowPhaselockData::Timeline(float Lock, float TargetTimeScale) const
{
    FOpenWillowPhaselockTimeline T;
    T.SkillDuration = LiftDuration + Lock * TargetTimeScale;
    T.LockedAt = LiftDuration;
    T.OutroAt = T.SkillDuration - LockFadeOutTime;
    T.ReleasedAt = T.SkillDuration;
    T.EndSkillAt = T.SkillDuration + ReleaseBufferTime;
    return T;
}
