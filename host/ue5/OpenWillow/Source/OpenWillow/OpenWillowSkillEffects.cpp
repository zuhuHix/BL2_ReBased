#include "OpenWillowSkillEffects.h"
#include "OpenWillowInventory.h"
#include "OpenWillowSkills.h"
#include "OpenWillowWalker.h"
#include "Dom/JsonObject.h"
#include "GameFramework/Controller.h"
#include "Misc/FileHelper.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

namespace
{
// A number field, only when the JSON value really is a number: UE's TryGetNumberField also converts numeric-looking
// strings, which a malformed manifest must not pass. Present with another type (not null): sets *bWrongType.
template <typename T>
bool NumberField(const FJsonObject& Object, const TCHAR* Name, T& Out, bool* bWrongType = nullptr)
{
    const TSharedPtr<FJsonValue> Value = Object.TryGetField(Name);
    if (!Value.IsValid() || Value->Type == EJson::Null) return false;
    if (Value->Type != EJson::Number)
    {
        if (bWrongType) *bWrongType = true;
        return false;
    }
    Out = T(Value->AsNumber());
    return true;
}

FOpenWillowSkillEffects::FValue ReadValue(const FJsonObject* Object)
{
    FOpenWillowSkillEffects::FValue Value;
    if (!Object) return Value;
    double Number = 0;
    const FJsonObject* Formula = nullptr;
    {
        const TSharedPtr<FJsonObject>* Field = nullptr;
        if (Object->TryGetObjectField(TEXT("formula"), Field)) Formula = Field->Get();
    }
    if (Formula)
    {
        for (const TCHAR* Term : {TEXT("multiplier"), TEXT("level"), TEXT("power"), TEXT("offset")})
        {
            const TSharedPtr<FJsonObject>* Field = nullptr;
            Value.Formula.Add(Formula->TryGetObjectField(Term, Field) ? ReadValue(Field->Get()) : FOpenWillowSkillEffects::FValue());
        }
        const TSharedPtr<FJsonObject>* BaseField = nullptr;
        Value.Formula.Add(Object->TryGetObjectField(TEXT("base"), BaseField) ? ReadValue(BaseField->Get()) : FOpenWillowSkillEffects::FValue());
        NumberField(*Object, TEXT("mode"), Value.Mode);
        if (NumberField(*Object, TEXT("scale"), Number)) Value.Scale = float(Number);
        for (const FOpenWillowSkillEffects::FValue& Term : Value.Formula) Value.bResolved &= Term.bResolved;
    }
    else if (NumberField(*Object, TEXT("constant"), Number)) Value.Constant = float(Number);
    else if (Object->TryGetStringField(TEXT("attribute"), Value.Attribute))
    {
        if (NumberField(*Object, TEXT("scale"), Number)) Value.Scale = float(Number);
    }
    else Value.bResolved = false;   // {"unresolved": <definition>}
    return Value;
}

const FJsonObject* ObjectField(const FJsonObject& Object, const TCHAR* Name)
{
    const TSharedPtr<FJsonObject>* Field = nullptr;
    return Object.TryGetObjectField(Name, Field) ? Field->Get() : nullptr;
}

FOpenWillowSkillEffects::FTest ReadTest(const FJsonObject& Object)
{
    using FTest = FOpenWillowSkillEffects::FTest;
    FTest Test;
    FString Kind, Usage;
    Object.TryGetStringField(TEXT("kind"), Kind);   // a missing kind reads as Unknown: the skill stays off
    if (Kind == TEXT("attribute"))
    {
        Test.Kind = FTest::EKind::Attribute;
        Object.TryGetStringField(TEXT("attribute"), Test.Attribute);
        Object.TryGetStringField(TEXT("attribute2"), Test.Attribute2);
        Object.TryGetStringField(TEXT("op"), Test.Op);
        Test.bMultiply = Object.TryGetStringField(TEXT("usage"), Usage) && Usage == TEXT("Multiply");
        double Constant = 0;
        NumberField(Object, TEXT("constant"), Constant);
        Test.Constant = float(Constant);
    }
    else if (Kind == TEXT("and") || Kind == TEXT("or"))
    {
        Test.Kind = Kind == TEXT("and") ? FTest::EKind::And : FTest::EKind::Or;
        for (const TCHAR* Side : {TEXT("left"), TEXT("right")})
            if (const FJsonObject* Child = ObjectField(Object, Side)) Test.Children.Add(ReadTest(*Child));
    }
    else if (Kind == TEXT("onFoot")) Test.Kind = FTest::EKind::OnFoot;
    else if (Kind == TEXT("healthState"))
    {
        // HealthStateExpressionEvaluator with bHealthy: true while not in Fight For Your Life (the only form Maya's data uses).
        const FJsonObject* Props = ObjectField(Object, TEXT("properties"));
        bool bHealthy = false;
        Test.Kind = Props && Props->TryGetBoolField(TEXT("bHealthy"), bHealthy) && bHealthy ? FTest::EKind::Healthy : FTest::EKind::Unknown;
    }
    return Test;
}

float WorldNumber(const FOpenWillowSkillEffects::FValue& Value, const FOpenWillowSkillEffects::FWorld& World)
{
    if (!Value.bResolved) return 0.f;
    if (Value.Formula.Num() == 5)
    {
        const float Multiplier = WorldNumber(Value.Formula[0], World), Level = WorldNumber(Value.Formula[1], World);
        const float Power = WorldNumber(Value.Formula[2], World), Offset = WorldNumber(Value.Formula[3], World);
        const float Base = WorldNumber(Value.Formula[4], World);
        const float F = Multiplier * ((Power == 1.f ? Level : FMath::Pow(Level, Power)) + Offset);
        const float Combined = Value.Mode == 1 ? Base + F : Value.Mode == 2 ? Base * F : Value.Mode == 3 ? F - Base : F;
        return Combined * Value.Scale;
    }
    if (Value.Attribute.IsEmpty()) return Value.Constant;
    float Number = 0.f;
    return World.Value && World.Value(Value.Attribute, Number) ? Number * Value.Scale : 0.f;
}
}

bool FOpenWillowSkillEffects::Load(const FString& File, FString& OutError)
{
    FString Text;
    TSharedPtr<FJsonObject> Root;
    if (!FFileHelper::LoadFileToString(Text, *File)) { OutError = TEXT("cannot read ") + File; return false; }
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root) { OutError = TEXT("not JSON: ") + File; return false; }
    return LoadJson(*Root, OutError);
}

bool FOpenWillowSkillEffects::LoadJson(const FJsonObject& Root, FString& OutError)
{
    // Every field is read with a checked accessor: a malformed manifest is refused whole, leaving no effects loaded,
    // so every hook returns its base value.
    Skills.Reset();
    auto Fail = [&OutError](const FString& Why) { OutError = TEXT("malformed skill effects: ") + Why; return false; };
    FString Format;
    if (!Root.TryGetStringField(TEXT("format"), Format) || Format != TEXT("openwillow.skill_effects/1"))
        return Fail(TEXT("format is not openwillow.skill_effects/1 (regenerate with tools/prepare_skill_effects.py)"));
    const TArray<TSharedPtr<FJsonValue>>* SkillValues = nullptr;
    if (!Root.TryGetArrayField(TEXT("skills"), SkillValues)) return Fail(TEXT("no skills array"));
    TArray<FSkill> Read;
    for (int32 SkillIndex = 0; SkillIndex < SkillValues->Num(); ++SkillIndex)
    {
        const TSharedPtr<FJsonObject>* ObjectPtr = nullptr;
        if (!(*SkillValues)[SkillIndex].IsValid() || !(*SkillValues)[SkillIndex]->TryGetObject(ObjectPtr) || !ObjectPtr->IsValid())
            return Fail(FString::Printf(TEXT("skill %d is not an object"), SkillIndex));
        const FJsonObject& Object = **ObjectPtr;
        FSkill& Skill = Read.AddDefaulted_GetRef();
        FString Type;
        const TArray<TSharedPtr<FJsonValue>>* EffectValues = nullptr;
        if (!Object.TryGetStringField(TEXT("id"), Skill.Id) || !Object.TryGetStringField(TEXT("type"), Type)
            || !NumberField(Object, TEXT("maxGrade"), Skill.MaxGrade) || !Object.TryGetArrayField(TEXT("effects"), EffectValues))
            return Fail(FString::Printf(TEXT("skill %d lacks id, type, maxGrade or effects"), SkillIndex));
        Object.TryGetStringField(TEXT("name"), Skill.Name);
        Skill.Type = Type == TEXT("Action") ? ESkillType::Action : Type == TEXT("ActionAugment") ? ESkillType::ActionAugment
            : Type == TEXT("Kill") ? ESkillType::Kill : Type == TEXT("Proficiency") ? ESkillType::Proficiency : ESkillType::Passive;
        Object.TryGetBoolField(TEXT("subjectToGradeRules"), Skill.bGradeRules);
        for (const TSharedPtr<FJsonValue>& EffectValue : *EffectValues)
        {
            const TSharedPtr<FJsonObject>* EPtr = nullptr;
            FString Mod, Target;
            if (!EffectValue.IsValid() || !EffectValue->TryGetObject(EPtr) || !EPtr->IsValid())
                return Fail(Skill.Id + TEXT(": an effect is not an object"));
            const FJsonObject& E = **EPtr;
            if (!E.TryGetStringField(TEXT("modifierType"), Mod) || !E.TryGetStringField(TEXT("target"), Target))
                return Fail(Skill.Id + TEXT(": an effect lacks modifierType or target"));
            FEffect& Effect = Skill.Effects.AddDefaulted_GetRef();
            E.TryGetStringField(TEXT("attribute"), Effect.Attribute);   // null stays empty
            Effect.Type = Mod == TEXT("PreAdd") ? EModifier::PreAdd : Mod == TEXT("PostAdd") ? EModifier::PostAdd : EModifier::Scale;
            bool bIncludeSelf = false;
            E.TryGetBoolField(TEXT("includeSelf"), bIncludeSelf);
            Effect.bSelf = Target == TEXT("Self") || ((Target == TEXT("Allies") || Target == TEXT("All")) && bIncludeSelf);
            bool bWrongType = false;
            NumberField(E, TEXT("startGrade"), Effect.StartGrade, &bWrongType);
            NumberField(E, TEXT("interval"), Effect.Interval, &bWrongType);
            if (bWrongType) return Fail(Skill.Id + TEXT(": startGrade or interval is not a number"));
            Effect.Base = ReadValue(ObjectField(E, TEXT("base")));
            Effect.PerGrade = ReadValue(ObjectField(E, TEXT("perGrade")));
            const TArray<TSharedPtr<FJsonValue>>* Bonus = nullptr;
            if (E.TryGetArrayField(TEXT("bonus"), Bonus))
                for (const TSharedPtr<FJsonValue>& B : *Bonus)
                {
                    const TSharedPtr<FJsonObject>* BPtr = nullptr;
                    int32 Grade = 0;
                    double Value = 0;
                    if (!B.IsValid() || !B->TryGetObject(BPtr) || !BPtr->IsValid() || !NumberField(**BPtr, TEXT("grade"), Grade)
                        || !NumberField(**BPtr, TEXT("value"), Value))
                        return Fail(Skill.Id + TEXT(": a bonus entry lacks grade or value"));
                    Effect.Bonus.Add({Grade, float(Value)});
                }
        }
        const TArray<TSharedPtr<FJsonValue>>* Conditions = nullptr;
        if (Object.TryGetArrayField(TEXT("conditions"), Conditions))
            for (const TSharedPtr<FJsonValue>& Row : *Conditions)
            {
                const TSharedPtr<FJsonObject>* RowPtr = nullptr;
                if (!Row.IsValid() || !Row->TryGetObject(RowPtr) || !RowPtr->IsValid())
                    return Fail(Skill.Id + TEXT(": a condition is not an object"));
                // Only constraints applied while active decide whether the modifiers are on (Maya's all are).
                bool bWhileActive = false;
                (*RowPtr)->TryGetBoolField(TEXT("whileActive"), bWhileActive);
                if (const FJsonObject* Test = ObjectField(**RowPtr, TEXT("test")); Test && bWhileActive)
                    Skill.Conditions.Add(ReadTest(*Test));
            }
    }
    if (Read.Num() == 0) return Fail(TEXT("no skills"));
    Skills = MoveTemp(Read);
    return true;
}

const FOpenWillowSkillEffects::FSkill* FOpenWillowSkillEffects::Find(const FString& IdOrName) const
{
    // The full object path, the display name ("Sweet Release") or the path's last part ("SweetRelease").
    return Skills.FindByPredicate([&](const FSkill& S)
        { return S.Id == IdOrName || S.Name == IdOrName || S.Id.EndsWith(TEXT(".") + IdOrName, ESearchCase::CaseSensitive); });
}

float FOpenWillowSkillEffects::ModifierValue(const FEffect& Effect, int32 Grade, const FWorld& World)
{
    if (Grade < Effect.StartGrade) return 0.f;
    const int32 Steps = (Grade - Effect.StartGrade) / FMath::Max(Effect.Interval, 1);
    // The single bonus entry with the largest grade at or below Grade (entries are not summed).
    const TPair<int32, float>* Best = nullptr;
    for (const TPair<int32, float>& Entry : Effect.Bonus)
        if (Entry.Key <= Grade && (!Best || Entry.Key > Best->Key)) Best = &Entry;
    return WorldNumber(Effect.Base, World) + WorldNumber(Effect.PerGrade, World) * float(Steps) + (Best ? Best->Value : 0.f);
}

bool FOpenWillowSkillEffects::Evaluate(const FTest& Test, const FWorld& World)
{
    switch (Test.Kind)
    {
    case FTest::EKind::OnFoot: return true;   // the host has no vehicles
    case FTest::EKind::Healthy:
    {
        float Down = 0.f;
        return !(World.Value && World.Value(TEXT("OpenWillow.Down"), Down) && Down > 0.f);
    }
    case FTest::EKind::And:
        for (const FTest& Child : Test.Children) if (!Evaluate(Child, World)) return false;
        return true;
    case FTest::EKind::Or:
        for (const FTest& Child : Test.Children) if (Evaluate(Child, World)) return true;
        return Test.Children.Num() == 0;
    case FTest::EKind::Attribute:
    {
        // AttributeExpression.EvaluateExpression (NATIVE_ATTRIBUTES.md section 11): an operand that does not resolve
        // makes the expression false.
        float Left = 0.f, Right = Test.Constant;
        if (!World.Value || !World.Value(Test.Attribute, Left)) return false;
        if (!Test.Attribute2.IsEmpty())
        {
            float Second = 0.f;
            if (!World.Value(Test.Attribute2, Second)) return false;
            Right = Test.bMultiply ? Test.Constant * Second : Second;
        }
        if (Test.Op == TEXT("NotEqualTo")) return Left != Right;
        if (Test.Op == TEXT("LessThan")) return Left < Right;
        if (Test.Op == TEXT("LessThanOrEqual")) return Left <= Right;
        if (Test.Op == TEXT("GreaterThan")) return Left > Right;
        if (Test.Op == TEXT("GreaterThanOrEqual")) return Left >= Right;
        return Left == Right;
    }
    default: return false;   // an evaluator the host does not model keeps the skill off
    }
}

bool FOpenWillowSkillEffects::IsActive(const FSkill& Skill, int32 Grade, const FWorld& World) const
{
    if (Skill.bGradeRules && Grade <= 0) return false;   // SkillDefinition.DoesSkillPassMinGradeTest
    switch (Skill.Type)
    {
    case ESkillType::Kill: if (!World.bKillSkillsActive) return false; break;
    case ESkillType::Action:
    case ESkillType::ActionAugment: if (!World.bActionSkillRunning) return false; break;
    default: break;
    }
    for (const FTest& Test : Skill.Conditions)
        if (!Evaluate(Test, World)) return false;
    return true;
}

FOpenWillowModifierStack FOpenWillowSkillEffects::Stack(const UOpenWillowSkills& Tree, const FString& Attribute, const FWorld& World) const
{
    FOpenWillowModifierStack Out;
    for (const FSkill& Skill : Skills)
    {
        const int32 Grade = Tree.GradeOfAny(Skill.Id);
        if (!IsActive(Skill, Grade, World)) continue;
        for (const FEffect& Effect : Skill.Effects)
        {
            if (!Effect.bSelf || Effect.Attribute != Attribute) continue;
            // Skill.UpdateGrade keeps the effective grade at 1 or more.
            const float Value = ModifierValue(Effect, FMath::Max(Grade, 1), World);
            switch (Effect.Type)
            {
            case EModifier::PreAdd: Out.PreAdd += Value; break;
            case EModifier::PostAdd: Out.PostAdd += Value; break;
            default: (Value > 0.f ? Out.ScaleUp : Out.ScaleDown) += Value; break;
            }
            ++Out.Count;
        }
    }
    return Out;
}

float FOpenWillowSkillEffects::EffectValue(const UOpenWillowSkills& Tree, const FString& IdOrName, int32 Index, const FWorld& World) const
{
    const FSkill* Skill = Find(IdOrName);
    if (!Skill || !Skill->Effects.IsValidIndex(Index)) return 0.f;
    const int32 Grade = Tree.GradeOfAny(Skill->Id);
    return Grade > 0 ? ModifierValue(Skill->Effects[Index], Grade, World) : 0.f;
}

namespace
{
const AOpenWillowWalker* FindMaya(const AActor* Actor)
{
    for (int32 Depth = 0; Actor && Depth < 8; ++Depth)
    {
        if (const AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(Actor)) return Maya;
        if (const AController* Controller = Cast<AController>(Actor)) { Actor = Controller->GetPawn(); continue; }
        Actor = Actor->GetOwner() ? Actor->GetOwner() : Actor->GetInstigator();
    }
    return nullptr;
}

// The attributes the host can answer for conditions and attribute-valued numbers (host mappings, UNVERIFIED):
// Att_EnemyIsPhaseLocked is 1 while Maya's Phaselock holds a target; the weapon damage-type flags come from the held
// gun's damage type name; OpenWillow.Down is the host's own name for Fight For Your Life. The host has no shield, so
// the shield attributes do not resolve.
FOpenWillowSkillEffects::FWorld WorldOf(const AOpenWillowWalker& Maya)
{
    FOpenWillowSkillEffects::FWorld World;
    World.bActionSkillRunning = Maya.IsPhaselockActive();
    World.bKillSkillsActive = Maya.GetSkills() && Maya.GetSkills()->KillSkillsActive();
    World.Value = [&Maya](const FString& Attribute, float& Out)
    {
        if (Attribute == TEXT("GD_Siren_Skills.Misc.Att_EnemyIsPhaseLocked")) { Out = Maya.IsPhaselockActive() && Maya.LastPhaselockHit() ? 1.f : 0.f; return true; }
        if (Attribute == TEXT("OpenWillow.Down")) { Out = Maya.IsDown() ? 1.f : 0.f; return true; }
        if (Attribute == TEXT("D_Attributes.HealthResourcePool.HealthMaxValue_Player")
            || Attribute == TEXT("D_Attributes.HealthResourcePool.HealthMaxValue")) { Out = Maya.GetMaxHealth(); return true; }
        if (Attribute == TEXT("D_Attributes.HealthResourcePool.HealthCurrentValue")) { Out = Maya.GetHealth(); return true; }
        const bool bNormal = Attribute == TEXT("D_Attributes.WeaponDamageType.WeaponDamage_Is_Normal");
        const bool bExplosive = Attribute == TEXT("D_Attributes.WeaponDamageType.WeaponDamage_Is_Explosive");
        if (bNormal || bExplosive)
        {
            const FOpenWillowWeaponItem* Weapon = Maya.GetInventory() ? Maya.GetInventory()->ActiveWeapon() : nullptr;
            if (!Weapon || Weapon->DamageType.IsEmpty()) return false;
            Out = (bNormal ? Weapon->DamageType.Contains(TEXT("DmgType_Normal")) : Weapon->DamageType.Contains(TEXT("Explosive"))) ? 1.f : 0.f;
            return true;
        }
        return false;
    };
    return World;
}
}

FOpenWillowModifierStack OpenWillowSkillEffects::Stack(const AActor* Actor, const TCHAR* Attribute)
{
    const AOpenWillowWalker* Maya = FindMaya(Actor);
    const UOpenWillowSkills* Tree = Maya ? Maya->GetSkills() : nullptr;
    if (!Tree || !Tree->GetEffects().IsLoaded()) return FOpenWillowModifierStack();
    return Tree->GetEffects().Stack(*Tree, Attribute, WorldOf(*Maya));
}

float OpenWillowSkillEffects::Apply(const AActor* Actor, const TCHAR* Attribute, float Base)
{
    const FOpenWillowModifierStack S = Stack(Actor, Attribute);
    return S.IsEmpty() ? Base : S.Apply(Base);
}

float OpenWillowSkillEffects::EffectValue(const AActor* Actor, const TCHAR* Skill, int32 Index)
{
    const AOpenWillowWalker* Maya = FindMaya(Actor);
    const UOpenWillowSkills* Tree = Maya ? Maya->GetSkills() : nullptr;
    if (!Tree || !Tree->GetEffects().IsLoaded()) return 0.f;
    return Tree->GetEffects().EffectValue(*Tree, Skill, Index, WorldOf(*Maya));
}

bool OpenWillowSkillEffects::IsLoaded(const AActor* Actor)
{
    const AOpenWillowWalker* Maya = FindMaya(Actor);
    return Maya && Maya->GetSkills() && Maya->GetSkills()->GetEffects().IsLoaded();
}
