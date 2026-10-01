#pragma once

#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "OpenWillowSkills.generated.h"

// Maya's level, experience and skill grades. The host owns this state; the
// Skills page (tools/hud_overlay/skills.js) only reports clicks and shows
// what the host sends back. Rules and what they rest on are in DECISIONS.md
// (2026-09-27, host-owned skill points). Skill effects are not applied.
UCLASS()
class OPENWILLOW_API UOpenWillowSkills : public UActorComponent
{
    GENERATED_BODY()
public:
    struct FSkill { FString Id; int32 Cell = 0; int32 MaxGrade = 1; int32 Grade = 0; };
    struct FTier { int32 PointsToUnlockNext = 0; TArray<FSkill> Skills; };

    // Reads a skilltree_<class>.json written by tools/prepare_skill_tree.py.
    bool LoadTree(const FString& File);
    // LoadTree uses these; tests build synthetic trees with them.
    void AddBranch(const TArray<FTier>& Tiers) { Branches.Add(Tiers); }
    void SetActionSkill(int32 MaxGrade, int32 PointsToUnlockTrees);

    // Experience needed to reach Level (UNVERIFIED curve; see DECISIONS.md).
    static int64 ExperienceForLevel(int32 Level);
    // Sets the level and puts experience at that level's threshold.
    void SetLevel(int32 NewLevel);
    void AddExperience(int64 Amount);
    int32 GetLevel() const { return Level; }
    int64 GetExperience() const { return Experience; }
    // 0..1 progress from this level's threshold to the next.
    float LevelProgress() const;

    // One point per level from level 5.
    int32 EarnedPoints() const { return FMath::Max(0, Level - 4); }
    int32 SpentPoints() const;
    int32 AvailablePoints() const { return FMath::Max(0, EarnedPoints() - SpentPoints()); }
    int32 GetActionGrade() const { return ActionGrade; }
    // Grade of the tree skill with this id (installed object path); 0 when absent or not trained.
    int32 GradeOf(const FString& Id) const;
    // Where the skill sits, in TrySpend's terms; false when the tree has no such skill.
    bool FindSkill(const FString& Id, int32& OutBranch, int32& OutTier, int32& OutCell) const;
    // The tier's PointsToUnlockNextTier (0 when absent).
    int32 TierPoints(int32 Branch, int32 Tier) const
    {
        return Branches.IsValidIndex(Branch) && Branches[Branch].IsValidIndex(Tier) ? Branches[Branch][Tier].PointsToUnlockNext : 0;
    }

    // A spend as the StatusMenu movie reports it: extCellClicked(branch, tier,
    // cell), with -1, -1, -1 for the action skill. Returns false and a reason
    // when the spend is refused; state changes only on success.
    bool TrySpend(int32 Branch, int32 Tier, int32 Cell, FString& OutReason);

    // {"points":N,"actionGrade":N,"grades":{"<skill path>":N,...}} for owSkills().
    FString StateJson() const;

private:
    int32 Invested(const TArray<FTier>& Branch) const;

    TArray<TArray<FTier>> Branches;
    int32 ActionMaxGrade = 1;
    int32 ActionPointsToUnlockTrees = 1;
    int32 ActionGrade = 0;
    int32 Level = 1;
    int64 Experience = 0;
};
