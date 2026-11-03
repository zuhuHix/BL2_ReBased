#pragma once

#include "CoreMinimal.h"

class AActor;
class FJsonObject;
class UOpenWillowSkills;

// Skill effects: which stat modifiers Maya's trained skills put on her attributes, read from the stock SkillDefinitions
// (tools/prepare_skill_effects.py writes local/ui/run/skilleffects_<class>.json; UOpenWillowSkills::LoadTree loads it).
// Rules, from the native notes and all UNVERIFIED in game unless the record says otherwise:
// - a modifier's value at grade g (Skill.CalculateModifierValue, NATIVE_SKILLS.md 4.3): 0 below the effect's start
//   grade, else base + per-grade x ((g - start) div max(interval, 1)) + the best bonus entry at or below g;
// - g is the skill's grade, at least 1 once active (Skill.UpdateGrade), with no class-mod bonus (none in the slice);
// - which skills contribute (4.1): Passive skills with a point in them (bSubjectToGradeRules); ActionAugment and the
//   Action skill while Phaselock runs; Kill skills never yet (no kill timer in the host); a skill whose constraint fails
//   contributes nothing (paused and deactivated are not told apart);
// - only TARGET_Self rows with an attribute make a modifier; the others carry numbers for behaviours (EffectValue);
// - modifiers on one attribute combine as (base + PreAdd) x (1 + positive Scales) / (1 - non-positive Scales) + PostAdd
//   (NATIVE_WEAPON_RULES.md section 1).
// Condition attributes the host can answer are listed in OpenWillowSkillEffects.cpp (WorldValue); any other attribute
// does not resolve, which makes its comparison false, as the native expression does.

// The modifier sums on one attribute.
struct OPENWILLOW_API FOpenWillowModifierStack
{
    float PreAdd = 0.f;
    float ScaleUp = 0.f;     // sum of Scale values > 0
    float ScaleDown = 0.f;   // sum of Scale values <= 0 (a non-positive number)
    float PostAdd = 0.f;
    int32 Count = 0;
    bool IsEmpty() const { return Count == 0; }
    float Apply(float Base) const { return float((double(Base) + PreAdd) * ((1.0 + ScaleUp) / (1.0 - ScaleDown)) + PostAdd); }
};

class OPENWILLOW_API FOpenWillowSkillEffects
{
public:
    enum class EModifier : uint8 { Scale, PreAdd, PostAdd };
    enum class ESkillType : uint8 { Passive, Action, ActionAugment, Kill, Proficiency };
    // A base or per-grade number: a constant, or another attribute's value times Scale; unresolved ones count as 0.
    struct FValue { float Constant = 0.f; FString Attribute; float Scale = 1.f; bool bResolved = true; };
    struct FEffect
    {
        FString Attribute;
        EModifier Type = EModifier::Scale;
        bool bSelf = false;
        int32 StartGrade = 0;
        int32 Interval = 0;
        FValue Base, PerGrade;
        TArray<TPair<int32, float>> Bonus;   // (GradeToApplyAt, Modifier)
    };
    // A constraint test: an attribute comparison, And/Or of two, or a fixed answer for the evaluators the host maps.
    struct FTest
    {
        enum class EKind : uint8 { Attribute, And, Or, OnFoot, Healthy, Unknown } Kind = EKind::Unknown;
        FString Attribute, Attribute2, Op;
        bool bMultiply = false;
        float Constant = 0.f;
        TArray<FTest> Children;
    };
    struct FSkill
    {
        FString Id, Name;
        ESkillType Type = ESkillType::Passive;
        int32 MaxGrade = 1;
        bool bGradeRules = true;
        TArray<FEffect> Effects;
        TArray<FTest> Conditions;   // all must hold (constraints checked while active)
    };
    // What the host knows about Maya for conditions and attribute-valued numbers.
    struct FWorld
    {
        bool bActionSkillRunning = false;
        TFunction<bool(const FString& Attribute, float& OutValue)> Value;   // false: the attribute does not resolve
    };

    bool Load(const FString& File, FString& OutError);
    bool LoadJson(const FJsonObject& Root, FString& OutError);
    bool IsLoaded() const { return Skills.Num() > 0; }
    const TArray<FSkill>& GetSkills() const { return Skills; }
    const FSkill* Find(const FString& IdOrName) const;

    // Value of one effect at a grade (Skill.CalculateModifierValue).
    static float ModifierValue(const FEffect& Effect, int32 Grade, const FWorld& World);
    static bool Evaluate(const FTest& Test, const FWorld& World);
    // Whether the skill contributes now, at the grade the tree holds.
    bool IsActive(const FSkill& Skill, int32 Grade, const FWorld& World) const;
    // Sum of the modifiers every contributing skill puts on Attribute.
    FOpenWillowModifierStack Stack(const UOpenWillowSkills& Tree, const FString& Attribute, const FWorld& World) const;
    // Effect Index of a skill at its trained grade, whether or not it is active (for behaviour skills); 0 when untrained,
    // below the start grade or unknown.
    float EffectValue(const UOpenWillowSkills& Tree, const FString& IdOrName, int32 Index, const FWorld& World) const;

private:
    TArray<FSkill> Skills;
};

// One-line call sites for the firing, Phaselock and Maya-state code. Maya is her pawn (AOpenWillowWalker) or anything
// whose owner chain reaches it; with no skills component or no effects data the base comes back unchanged.
namespace OpenWillowSkillEffects
{
    OPENWILLOW_API FOpenWillowModifierStack Stack(const AActor* Maya, const TCHAR* Attribute);
    OPENWILLOW_API float Apply(const AActor* Maya, const TCHAR* Attribute, float Base);
    OPENWILLOW_API float EffectValue(const AActor* Maya, const TCHAR* Skill, int32 Index);
    OPENWILLOW_API bool IsLoaded(const AActor* Maya);
    // Attribute names the slice hooks use (stock object paths).
    namespace Attr
    {
        inline const TCHAR* WeaponDamage = TEXT("D_Attributes.Weapon.WeaponDamage");
        inline const TCHAR* WeaponFireInterval = TEXT("D_Attributes.Weapon.WeaponFireInterval");
        inline const TCHAR* WeaponReloadSpeed = TEXT("D_Attributes.Weapon.WeaponReloadSpeed");
        inline const TCHAR* WeaponClipSize = TEXT("D_Attributes.Weapon.WeaponClipSize");
        inline const TCHAR* WeaponStatusEffectChance = TEXT("D_Attributes.Weapon.WeaponStatusEffectChanceModifier");
        inline const TCHAR* PhaselockDuration = TEXT("GD_Siren_Skills.Misc.Att_Phaselock_Duration");
        inline const TCHAR* PhaselockCooldown = TEXT("GD_Siren_Skills.Misc.Cooldown_Phaselock");
        inline const TCHAR* CooldownRate = TEXT("D_Attributes.ActiveSkillCooldownResource.ActiveSkillCooldownConsumptionRate");
        inline const TCHAR* HealthMax = TEXT("D_Attributes.HealthResourcePool.HealthMaxValue");
        inline const TCHAR* FootSpeed = TEXT("D_Attributes.GameplayAttributes.FootSpeed");
    }
}
