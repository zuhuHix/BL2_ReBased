#pragma once

#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "OpenWillowSkills.generated.h"

class FJsonObject;

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

    // WillowPlayerController.GetMaxExpLevel without DLC: 50 (docs/verification/NATIVE_PROGRESSION.md section 3,
    // UNVERIFIED in game). TODO: each licensed level-cap DLC adds its increment (clamped to 50 + all increments);
    // no DLC is modelled.
    static constexpr int32 MaxLevel = 50;
    // Experience needed to reach Level: R(n) = max(0, trunc(f(n)) - trunc(f(1))) with
    // f(n) = Multiplier x (n ^ Power + Offset) evaluated in single precision (NATIVE_PROGRESSION.md section 3).
    static int64 RequiredExperience(float Multiplier, float Power, float Offset, int32 Level);
    // R(n) with GlobalsDefinition.ExpPointsRequiredForLevel's constants (60, 2.8, 7.33); UNVERIFIED except level 46.
    static int64 ExperienceForLevel(int32 Level);
    // Sets the level (1..MaxLevel) and puts experience at that level's threshold.
    void SetLevel(int32 NewLevel);
    // Adds experience and levels up while the next threshold is met, up to MaxLevel (experience keeps counting there).
    void AddExperience(int64 Amount);
    int32 GetLevel() const { return Level; }
    int64 GetExperience() const { return Experience; }
    // 0..1 progress from this level's threshold to the next.
    float LevelProgress() const;

    // One point per level from level 5.
    static int32 EarnedPointsAt(int32 AtLevel) { return FMath::Max(0, AtLevel - 4); }
    int32 EarnedPoints() const { return EarnedPointsAt(Level); }
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

    // Progression for the quest save: {"level":N,"experience":N,"actionGrade":N,"points":N,"grades":{"<skill path>":N}}.
    // "points" (available points) is derived from the rest and written only so a resumed session can be checked.
    TSharedPtr<FJsonObject> ProgressionJson() const;
    // Puts back what ProgressionJson wrote (the loaded tree must be the same). On a missing field, an experience
    // outside the level's band, an unknown skill, a grade above its maximum or more points spent than the level
    // earns, returns false with a reason and changes nothing.
    bool RestoreProgression(const FJsonObject& Data, FString& OutError);

private:
    int32 Invested(const TArray<FTier>& Branch) const;

    TArray<TArray<FTier>> Branches;
    int32 ActionMaxGrade = 1;
    int32 ActionPointsToUnlockTrees = 1;
    int32 ActionGrade = 0;
    int32 Level = 1;
    int64 Experience = 0;
};
