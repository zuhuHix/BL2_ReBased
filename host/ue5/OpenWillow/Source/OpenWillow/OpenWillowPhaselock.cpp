#include "OpenWillowPhaselock.h"
#include "Dom/JsonObject.h"
#include "Misc/FileHelper.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

namespace
{
const TCHAR* const DiminishingSkill = TEXT("GD_Siren_Skills.Phaselock.Skill_Phaselock_DiminishingReturns");
const TCHAR* const CooldownManagerSkill = TEXT("GD_Siren_Skills.Phaselock.Skill_Phaselock_CooldownManager");

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
        || Root->GetStringField(TEXT("format")) != TEXT("openwillow.action_skill/1"))
    {
        OutError = TEXT("not an openwillow.action_skill/1 manifest: ") + File;
        return false;
    }
    const TSharedPtr<FJsonObject>* Action = nullptr;
    const TSharedPtr<FJsonObject>* Settings = nullptr;
    const TSharedPtr<FJsonObject>* Cooldown = nullptr;
    const TSharedPtr<FJsonObject>* Helpers = nullptr;
    const TSharedPtr<FJsonObject>* Upgrade = nullptr;
    if (!Root->TryGetObjectField(TEXT("actionSkill"), Action) || !(*Action)->TryGetObjectField(TEXT("settings"), Settings)
        || !Root->TryGetObjectField(TEXT("cooldown"), Cooldown) || !Root->TryGetObjectField(TEXT("helperSkills"), Helpers)
        || !Root->TryGetObjectField(TEXT("upgradePath"), Upgrade))
    {
        OutError = TEXT("manifest lacks actionSkill/settings, cooldown, helperSkills or upgradePath");
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
    if (!bOk) { OutError = TEXT("a Phaselock number is missing from the manifest"); return false; }

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
