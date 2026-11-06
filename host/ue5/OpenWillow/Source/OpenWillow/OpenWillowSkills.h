#pragma once

#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "GameFramework/Actor.h"
#include "OpenWillowSkillEffects.h"
#include "OpenWillowSkills.generated.h"

class FJsonObject;
struct FOpenWillowHitEvent;

// Maya's level, experience and skill grades. The host owns this state; the
// Skills page (tools/hud_overlay/skills.js) only reports clicks and shows
// what the host sends back. Rules and what they rest on are in DECISIONS.md
// (2026-09-27, host-owned skill points).
UCLASS()
class OPENWILLOW_API UOpenWillowSkills : public UActorComponent
{
    GENERATED_BODY()
public:
    UOpenWillowSkills();
    // Behaviour skills (OpenWillowSkillBehaviours.cpp): kill-skill timer, health regeneration and the Phaselock
    // variants, run on Maya's pawn.
    virtual void TickComponent(float DeltaTime, ELevelTick TickType, FActorComponentTickFunction* ThisTickFunction) override;
    // Listens to OpenWillowHit::OnHit() for the hit-driven skills (Cloud Kill, Life Tap, Immolate).
    virtual void BeginPlay() override;
    virtual void EndPlay(const EEndPlayReason::Type Reason) override;
    // Kill skills (WillowPlayerController.UpdateKillSkills, timer native): a kill (re)starts the timer at
    // KillSkillSeconds; they count while it runs. Refresh rather than add is an UNVERIFIED guess.
    // GD_Globals.General.Globals.KillSkillDurationsPerPlayers for one player, times KillSkillDurationMultiplier (1).
    static constexpr float KillSkillSeconds = 7.f;
    void OnEnemyKilled(float Now) { KillSkillsEndAt = Now + KillSkillSeconds; }
    bool KillSkillsActiveAt(float Now) const { return Now < KillSkillsEndAt; }
    bool KillSkillsActive() const;
    // PlayerSkillDamage = 8 x 1.13 ^ level and PlayerSkillStatusEffectDamage = 1.13 ^ level (Init_PlayerSkillDamage,
    // Init_PlayerSkillStatusEffectDamage with Att_UniversalBalanceScaler 1.13; read from data, UNVERIFIED in game).
    static float SkillDamageAt(int32 AtLevel) { return 8.f * FMath::Pow(1.13f, float(AtLevel)); }
    static float SkillStatusDamageAt(int32 AtLevel) { return FMath::Pow(1.13f, float(AtLevel)); }
    // PlayerMeleeDamage = 2.5 x (8 x 1.13 ^ level + InstigatedMeleeDamageAddedBuff), the buff taken as 0 (no melee buffs
    // in the host). Init_PlayerMeleeDamage, read from data, UNVERIFIED in game.
    static float MeleeDamageAt(int32 AtLevel) { return 2.5f * SkillDamageAt(AtLevel); }
    // A Helios blast and a Ruin pulse. Live values, checked at level 8 only; other levels are UNVERIFIED. 0.29 is fitted to the
    // live read and its reason is not known (the data reading gave 0.2); a Ruin pulse is 1 x PlayerSkillDamage.
    static float HeliosDamageAt(int32 Grade, int32 AtLevel) { return 0.29f * Grade * SkillDamageAt(AtLevel); }
    static float RuinPulseDamageAt(int32 AtLevel) { return SkillDamageAt(AtLevel); }
    // A Converge blast: 1.16 x PlayerSkillDamage, fitted to one live read at level 8, UNVERIFIED beyond it (the data reading gave 1 x
    // and the reason for the difference is not known, as with Helios's 0.29).
    static float ConvergeDamageAt(int32 AtLevel) { return 1.16f * SkillDamageAt(AtLevel); }
    // Mind's Eye: what the skill adds to a hit's crit bonus, 2.0 x 0.05 x grade (0 when untrained or without effects data).
    // In game the skill's Scale joins the attribute's stack: (2.0 + PreAdd) x (1 + Scale sum), seen live to 4 decimals at
    // grades 1, 3 and 5 with other modifiers on the stack, at level 8 only; other levels are UNVERIFIED. The host has none of
    // those modifiers, so the base is 2.0 and this is the whole of the Scale's part. Cached; recomputed after any grade change.
    float CritBonusAdd() const;
    // Recompense's proc roll, chance 0.1 x grade, from the component's own random stream (seed: -owskillseed=<n>, or
    // SetRandomSeed for tests).
    bool RollRecompense(int32 Grade) { return Grade > 0 && Random.FRand() < 0.1f * Grade; }
    void SetRandomSeed(int32 Seed) { Random.Initialize(Seed); }
    // Grade of the tree skill with this effects name (e.g. TEXT("Helios")); 0 when untrained or unknown.
    int32 GradeOfNamed(const TCHAR* Name) const;

    struct FSkill { FString Id; int32 Cell = 0; int32 MaxGrade = 1; int32 Grade = 0; };
    struct FTier { int32 PointsToUnlockNext = 0; TArray<FSkill> Skills; };

    // Reads a skilltree_<class>.json written by tools/prepare_skill_tree.py, and the skilleffects_<class>.json beside it
    // (tools/prepare_skill_effects.py) when present; without it no skill changes a stat (logged).
    bool LoadTree(const FString& File);
    // The skill effects read beside the tree (OpenWillowSkillEffects.h); tests load their own with GetEffectsMutable.
    const FOpenWillowSkillEffects& GetEffects() const { return Effects; }
    FOpenWillowSkillEffects& GetEffectsMutable() { return Effects; }
    // LoadTree found no skilleffects_<class>.json beside the tree: local data not generated, as opposed to a file that failed to load.
    bool IsEffectsFileMissing() const { return bEffectsFileMissing; }
    // LoadTree uses these; tests build synthetic trees with them.
    void AddBranch(const TArray<FTier>& Tiers) { Branches.Add(Tiers); }
    void SetActionSkill(int32 MaxGrade, int32 PointsToUnlockTrees, const FString& Id = FString());

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
    // GradeOf, or the action skill's grade for its id.
    int32 GradeOfAny(const FString& Id) const { return !ActionSkillId.IsEmpty() && Id == ActionSkillId ? ActionGrade : GradeOf(Id); }
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

    // Respec, as WillowPlayerController.ResetSkillTree (NATIVE_SKILLS.md section 3.1): every grade, the action skill's
    // included, goes back to 0 and the refund (points spent before) is returned; level and experience stay. The
    // local real-game respec trace (DECISIONS.md 2026-09-27) shows a level 45 Maya with 41 free points and no grades
    // afterwards, which is this rule at one level.
    int32 ResetTree();
    // Credits a respec costs at a level: GD_Globals.Skills.CostToResetSkillPoints is the balance formula
    // 100 x (1.12 ^ level), 1.12 being GD_Economy.Universal.Att_UniversalPriceIncreasePerLevelScaler (read from data,
    // UNVERIFIED in game), evaluated in single precision and truncated.
    static int32 ResetCost(int32 AtLevel);
    // ServerPurchaseSkillTreeReset: refuses when InOutMoney is below ResetCost(Level); otherwise takes the cost from
    // InOutMoney and resets the tree. As in the script, a tree with nothing spent can still be bought back (the
    // Quick Change dialog that may stop that is not read). Returns false with a reason and changes nothing on refusal.
    bool PurchaseReset(int32& InOutMoney, FString& OutReason);

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
    FOpenWillowSkillEffects Effects;
    bool bEffectsFileMissing = false;
    // Bumped by every grade change (spend, respec, load); caches compare against it. GradesChanged also makes Maya's
    // survival stats recompute.
    int32 GradeVersion = 0;
    void GradesChanged();
    mutable int32 CritAddVersion = -1;
    mutable float CritAdd = 0.f;
    FRandomStream Random;
    // Behaviour state (OpenWillowSkillBehaviours.cpp).
    struct FAction
    {
        float At = 0.f;
        enum class EKind : uint8 { Blast, HealOrb, SingleHit } Kind = EKind::Blast;   // SingleHit: one hit on Target
        TWeakObjectPtr<AActor> Target;   // centre of a blast while it exists
        FVector Where = FVector::ZeroVector;
        float Radius = 0.f, Damage = 0.f;
        FString DamageType, Skill, Why;   // Skill: the effects name, for the hit event's "skill:<name>" weapon id
    };
    TArray<FAction> Actions;
    float KillSkillsEndAt = -1.f;
    float SeenCastAt = -1.f;
    TWeakObjectPtr<AActor> HeldTarget;
    float HeldUntil = -1.f;
    bool bHeldKillSeen = false;
    FDelegateHandle HitHandle;
    float CloudKillReadyAt = -1.f;   // CloudKill_Feedback runs 5.5 s and blocks the next proc
    float NextBlightAt = -1.f;
    void HandleHit(const FOpenWillowHitEvent& Event);
    // FOpenWillowMayaState::Modifiers: the skills' stat modifiers on Maya's survival stats.
    void ApplyStatModifiers(struct FOpenWillowMayaStats& Stats) const;
    mutable bool bApplyingStats = false;   // re-entry guard
public:
    // While ApplyStatModifiers runs: the effective maximum health computed so far (formulas on max health, such as
    // Sustenance's missing health and Elated's regeneration, read it); -1 otherwise (they read the pool's Base).
    float StatsMaxHealth() const { return ComputedMaxHealth; }
private:
    mutable float ComputedMaxHealth = -1.f;
public:
    // Recompense (AOpenWillowWalker::ApplyDamageToPlayer hook): Maya lost Damage health to Causer (or Instigator's pawn).
    void OnMayaDamaged(AActor* Causer, class AController* Instigator, float Damage);
private:
    void TickBehaviours(float Now, float DeltaTime);
    void RunAction(const FAction& Action);
    FString ActionSkillId;
    int32 ActionMaxGrade = 1;
    int32 ActionPointsToUnlockTrees = 1;
    int32 ActionGrade = 0;
    int32 Level = 1;
    int64 Experience = 0;
};
