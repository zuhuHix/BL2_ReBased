#include "OpenWillowSkills.h"
#include "Dom/JsonObject.h"
#include "Misc/FileHelper.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

bool UOpenWillowSkills::LoadTree(const FString& File)
{
    FString Text;
    TSharedPtr<FJsonObject> Root;
    if (!FFileHelper::LoadFileToString(Text, *File)
        || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root)
        return false;
    Branches.Reset();
    const TSharedPtr<FJsonObject>* Action = nullptr;
    int32 ActionMax = 1;
    if (Root->TryGetObjectField(TEXT("actionSkill"), Action)) (*Action)->TryGetNumberField(TEXT("maxGrade"), ActionMax);
    // The root branch's first-tier PointsToUnlockNextTier (tools/prepare_skill_tree.py).
    int32 UnlockTrees = 1;
    Root->TryGetNumberField(TEXT("actionSkillPoints"), UnlockTrees);
    SetActionSkill(ActionMax, UnlockTrees);
    for (const TSharedPtr<FJsonValue>& BranchValue : Root->GetArrayField(TEXT("branches")))
    {
        TArray<FTier> Tiers;
        for (const TSharedPtr<FJsonValue>& TierValue : BranchValue->AsObject()->GetArrayField(TEXT("tiers")))
        {
            const TSharedPtr<FJsonObject> TierObject = TierValue->AsObject();
            FTier& Tier = Tiers.AddDefaulted_GetRef();
            TierObject->TryGetNumberField(TEXT("pointsToUnlockNext"), Tier.PointsToUnlockNext);
            for (const TSharedPtr<FJsonValue>& SkillValue : TierObject->GetArrayField(TEXT("skills")))
            {
                const TSharedPtr<FJsonObject> SkillObject = SkillValue->AsObject();
                FSkill& Skill = Tier.Skills.AddDefaulted_GetRef();
                Skill.Id = SkillObject->GetStringField(TEXT("id"));
                SkillObject->TryGetNumberField(TEXT("cell"), Skill.Cell);
                SkillObject->TryGetNumberField(TEXT("maxGrade"), Skill.MaxGrade);
            }
        }
        AddBranch(Tiers);
    }
    return Branches.Num() > 0;
}

void UOpenWillowSkills::SetActionSkill(int32 MaxGrade, int32 PointsToUnlockTrees)
{
    ActionMaxGrade = FMath::Max(1, MaxGrade);
    ActionPointsToUnlockTrees = FMath::Max(0, PointsToUnlockTrees);
}

int64 UOpenWillowSkills::ExperienceForLevel(int32 InLevel)
{
    // floor(60 * L^2.8 - 60): 0 at level 1. It matches the one threshold a
    // real-game trace shows (2,715,586 for level 46); UNVERIFIED elsewhere.
    return InLevel <= 1 ? 0 : int64(FMath::FloorToDouble(60.0 * FMath::Pow(double(InLevel), 2.8) - 60.0));
}

void UOpenWillowSkills::SetLevel(int32 NewLevel)
{
    Level = FMath::Max(1, NewLevel);
    Experience = ExperienceForLevel(Level);
}

void UOpenWillowSkills::AddExperience(int64 Amount)
{
    // No level cap is modelled yet.
    Experience += FMath::Max<int64>(0, Amount);
    while (Experience >= ExperienceForLevel(Level + 1)) ++Level;
}

float UOpenWillowSkills::LevelProgress() const
{
    const int64 From = ExperienceForLevel(Level);
    const int64 To = ExperienceForLevel(Level + 1);
    return To > From ? FMath::Clamp(float(double(Experience - From) / double(To - From)), 0.f, 1.f) : 0.f;
}

int32 UOpenWillowSkills::Invested(const TArray<FTier>& Branch) const
{
    int32 Sum = 0;
    for (const FTier& Tier : Branch)
        for (const FSkill& Skill : Tier.Skills) Sum += Skill.Grade;
    return Sum;
}

int32 UOpenWillowSkills::GradeOf(const FString& Id) const
{
    for (const TArray<FTier>& Branch : Branches)
        for (const FTier& Tier : Branch)
            for (const FSkill& Skill : Tier.Skills)
                if (Skill.Id == Id) return Skill.Grade;
    return 0;
}

bool UOpenWillowSkills::FindSkill(const FString& Id, int32& OutBranch, int32& OutTier, int32& OutCell) const
{
    for (int32 Branch = 0; Branch < Branches.Num(); ++Branch)
        for (int32 Tier = 0; Tier < Branches[Branch].Num(); ++Tier)
            for (const FSkill& Skill : Branches[Branch][Tier].Skills)
                if (Skill.Id == Id)
                {
                    OutBranch = Branch;
                    OutTier = Tier;
                    OutCell = Skill.Cell;
                    return true;
                }
    return false;
}

int32 UOpenWillowSkills::SpentPoints() const
{
    int32 Sum = ActionGrade;
    for (const TArray<FTier>& Branch : Branches) Sum += Invested(Branch);
    return Sum;
}

bool UOpenWillowSkills::TrySpend(int32 Branch, int32 Tier, int32 Cell, FString& OutReason)
{
    if (Branch == -1 && Tier == -1 && Cell == -1)
    {
        if (ActionGrade >= ActionMaxGrade) { OutReason = TEXT("action skill at max grade"); return false; }
        if (AvailablePoints() < 1) { OutReason = TEXT("no skill points"); return false; }
        ++ActionGrade;
        return true;
    }
    if (!Branches.IsValidIndex(Branch) || !Branches[Branch].IsValidIndex(Tier))
    {
        OutReason = TEXT("no such branch or tier");
        return false;
    }
    TArray<FTier>& Tiers = Branches[Branch];
    FSkill* Skill = Tiers[Tier].Skills.FindByPredicate([Cell](const FSkill& S) { return S.Cell == Cell; });
    if (!Skill) { OutReason = TEXT("no skill in that cell"); return false; }
    if (Skill->Grade >= Skill->MaxGrade) { OutReason = TEXT("skill at max grade"); return false; }
    if (AvailablePoints() < 1) { OutReason = TEXT("no skill points"); return false; }
    if (ActionGrade < ActionPointsToUnlockTrees) { OutReason = TEXT("action skill not unlocked"); return false; }
    // A tier opens once the branch holds the points every lower tier asks for.
    int32 Required = 0;
    for (int32 Lower = 0; Lower < Tier; ++Lower) Required += Tiers[Lower].PointsToUnlockNext;
    if (Invested(Tiers) < Required)
    {
        OutReason = FString::Printf(TEXT("tier locked (%d of %d points in branch)"), Invested(Tiers), Required);
        return false;
    }
    ++Skill->Grade;
    return true;
}

FString UOpenWillowSkills::StateJson() const
{
    // Skill ids are installed object paths (letters, digits, '.', '_'), so
    // they need no JSON escaping.
    TArray<FString> Grades;
    for (const TArray<FTier>& Branch : Branches)
        for (const FTier& Tier : Branch)
            for (const FSkill& Skill : Tier.Skills)
                if (Skill.Grade > 0) Grades.Add(FString::Printf(TEXT("\"%s\":%d"), *Skill.Id, Skill.Grade));
    return FString::Printf(TEXT("{\"points\":%d,\"actionGrade\":%d,\"grades\":{%s}}"),
        AvailablePoints(), ActionGrade, *FString::Join(Grades, TEXT(",")));
}
