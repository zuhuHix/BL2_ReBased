#include "Misc/AutomationTest.h"
#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowSkills.h"
#include "Dom/JsonObject.h"

// Spend rules on a synthetic tree (no game data): branch 0 has a tier with
// cells 0 (max 3) and 2 (max 2) that opens the next tier after 3 points,
// then one cell 1 (max 1); branch 1 has one cell 1 (max 2).
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowSkillsTest, "OpenWillow.Skills",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)
bool FOpenWillowSkillsTest::RunTest(const FString&)
{
    UOpenWillowSkills* Skills = NewObject<UOpenWillowSkills>();
    using FTier = UOpenWillowSkills::FTier;
    FTier First{3, {{TEXT("Test.A"), 0, 3}, {TEXT("Test.B"), 2, 2}}};
    FTier Second{0, {{TEXT("Test.C"), 1, 1}}};
    Skills->AddBranch({First, Second});
    Skills->AddBranch({FTier{0, {{TEXT("Test.D"), 1, 2}}}});
    Skills->SetActionSkill(1, 1);

    Skills->SetLevel(4);
    TestEqual(TEXT("No points before level 5"), Skills->EarnedPoints(), 0);
    Skills->SetLevel(5);
    TestEqual(TEXT("First point at level 5"), Skills->EarnedPoints(), 1);
    Skills->SetLevel(9);
    TestEqual(TEXT("Five points at level 9"), Skills->AvailablePoints(), 5);

    FString Reason;
    auto Spend = [&](int32 B, int32 T, int32 C) { Reason.Reset(); return Skills->TrySpend(B, T, C, Reason); };
    TestFalse(TEXT("Trees wait for the action skill"), Spend(0, 0, 0));
    TestTrue(TEXT("Action skill spend"), Spend(-1, -1, -1));
    TestFalse(TEXT("Action skill max grade"), Spend(-1, -1, -1));
    TestFalse(TEXT("Second tier locked at 0 of 3"), Spend(0, 1, 1));
    TestTrue(TEXT("Grade 1"), Spend(0, 0, 0));
    TestTrue(TEXT("Grade 2"), Spend(0, 0, 0));
    TestTrue(TEXT("Grade 3"), Spend(0, 0, 0));
    TestFalse(TEXT("Max grade refused"), Spend(0, 0, 0));
    TestEqual(TEXT("Max grade reason"), Reason, FString(TEXT("skill at max grade")));
    TestTrue(TEXT("Second tier opens at 3 of 3"), Spend(0, 1, 1));
    TestEqual(TEXT("All points spent"), Skills->AvailablePoints(), 0);
    TestFalse(TEXT("No points left"), Spend(1, 0, 1));
    TestEqual(TEXT("No points reason"), Reason, FString(TEXT("no skill points")));
    TestFalse(TEXT("Empty cell"), Spend(0, 0, 1));
    TestFalse(TEXT("Unknown branch"), Spend(5, 0, 0));
    TestEqual(TEXT("State for the page"), Skills->StateJson(),
        FString(TEXT("{\"points\":0,\"actionGrade\":1,\"grades\":{\"Test.A\":3,\"Test.C\":1}}")));

    // Respec: everything back, the action skill included; level stays; the refund is what was spent.
    TestEqual(TEXT("Respec refund"), Skills->ResetTree(), 5);
    TestEqual(TEXT("Respec frees every point"), Skills->AvailablePoints(), 5);
    TestEqual(TEXT("Respec clears grades and the action skill"), Skills->StateJson(),
        FString(TEXT("{\"points\":5,\"actionGrade\":0,\"grades\":{}}")));
    TestEqual(TEXT("Respec keeps the level"), Skills->GetLevel(), 9);
    TestFalse(TEXT("Trees closed again after a respec"), Spend(0, 0, 0));
    TestTrue(TEXT("Points can be spent again"), Spend(-1, -1, -1) && Spend(0, 0, 0) && Spend(0, 0, 0) && Spend(0, 0, 2));
    // Cost 100 x 1.12 ^ level, truncated: 310 at level 10 (310.58), 28,900 at level 50 (28,900.2).
    TestEqual(TEXT("Respec cost at level 10"), UOpenWillowSkills::ResetCost(10), 310);
    TestEqual(TEXT("Respec cost at level 50"), UOpenWillowSkills::ResetCost(50), 28900);
    int32 Purse = UOpenWillowSkills::ResetCost(9) - 1;
    TestFalse(TEXT("Respec refused one credit short"), Skills->PurchaseReset(Purse, Reason));
    TestEqual(TEXT("Refused respec keeps the grades"), Skills->GradeOf(TEXT("Test.A")), 2);
    Purse += 11;
    TestTrue(TEXT("Respec bought"), Skills->PurchaseReset(Purse, Reason));
    TestEqual(TEXT("Respec cost taken"), Purse, 10);
    TestEqual(TEXT("Bought respec clears the tree"), Skills->SpentPoints(), 0);
    // Spend back to the state the checks below expect.
    TestTrue(TEXT("Respend"), Spend(-1, -1, -1) && Spend(0, 0, 0) && Spend(0, 0, 0) && Spend(0, 0, 0) && Spend(0, 1, 1));

    // Level curve (NATIVE_PROGRESSION.md section 3): level 46 is the one threshold seen in the real game; the others
    // are the note's values, where the old floor(60 L^2.8 - 60) curve was one point lower.
    TestEqual(TEXT("Level 1 needs nothing"), UOpenWillowSkills::ExperienceForLevel(1), int64(0));
    TestEqual(TEXT("Level 2 threshold"), UOpenWillowSkills::ExperienceForLevel(2), int64(358));
    TestEqual(TEXT("Level 5 threshold"), UOpenWillowSkills::ExperienceForLevel(5), int64(5376));
    TestEqual(TEXT("Level 9 threshold"), UOpenWillowSkills::ExperienceForLevel(9), int64(28126));
    // Synthetic constants. 1.5 x (n^2 + 0.4): f(1) = 2.1 -> 2, f(2) = 6.6 -> 6, R(2) = 4; with the offset outside
    // (1.9 -> 1, 6.4 -> 6) it would be 5. 1.5 x (n^2 + 0.2): 1.8 -> 1, 6.3 -> 6, R(2) = 5, where flooring the
    // unrounded difference (4.5) would give 4.
    TestEqual(TEXT("Offset inside the multiplier"), UOpenWillowSkills::RequiredExperience(1.5f, 2.f, 0.4f, 2), int64(4));
    TestEqual(TEXT("Points truncated before the difference"), UOpenWillowSkills::RequiredExperience(1.5f, 2.f, 0.2f, 2), int64(5));
    Skills->SetLevel(46);
    TestEqual(TEXT("Traced level 46 threshold"), Skills->GetExperience(), int64(2715586));
    Skills->AddExperience(UOpenWillowSkills::ExperienceForLevel(47) - Skills->GetExperience() - 1);
    TestEqual(TEXT("One XP short of 47"), Skills->GetLevel(), 46);
    Skills->AddExperience(1);
    TestEqual(TEXT("Level up at the threshold"), Skills->GetLevel(), 47);
    TestEqual(TEXT("Grades survive a level up"), Skills->GetActionGrade(), 1);

    // Quest-save progression: a fresh component with the same tree gets the same state back.
    Skills->AddExperience(5);
    const TSharedPtr<FJsonObject> Saved = Skills->ProgressionJson();
    UOpenWillowSkills* Loaded = NewObject<UOpenWillowSkills>();
    Loaded->AddBranch({First, Second});
    Loaded->AddBranch({FTier{0, {{TEXT("Test.D"), 1, 2}}}});
    Loaded->SetActionSkill(1, 1);
    Loaded->SetLevel(8);
    TestTrue(TEXT("Progression restores"), Loaded->RestoreProgression(*Saved, Reason));
    TestEqual(TEXT("Restored state"), Loaded->StateJson(), Skills->StateJson());
    TestEqual(TEXT("Restored experience"), Loaded->GetExperience(), Skills->GetExperience());
    TestEqual(TEXT("Restored level"), Loaded->GetLevel(), 47);
    Saved->GetObjectField(TEXT("grades"))->SetNumberField(TEXT("Test.C"), 2);
    TestFalse(TEXT("Grade above its maximum refused"), Loaded->RestoreProgression(*Saved, Reason));
    Saved->GetObjectField(TEXT("grades"))->SetNumberField(TEXT("Test.C"), 1);
    Saved->GetObjectField(TEXT("grades"))->SetNumberField(TEXT("Test.Unknown"), 1);
    TestFalse(TEXT("Unknown skill refused"), Loaded->RestoreProgression(*Saved, Reason));

    // Level cap 50: a grant past level 50's threshold stops there, experience keeps counting, and such a state
    // round-trips through the save; a level above the cap is refused.
    Skills->SetLevel(49);
    Skills->AddExperience(UOpenWillowSkills::ExperienceForLevel(52) - Skills->GetExperience());
    TestEqual(TEXT("Level stops at the cap"), Skills->GetLevel(), UOpenWillowSkills::MaxLevel);
    TestEqual(TEXT("Experience past the cap kept"), Skills->GetExperience(), UOpenWillowSkills::ExperienceForLevel(52));
    Skills->SetLevel(60);
    TestEqual(TEXT("SetLevel clamps to the cap"), Skills->GetLevel(), 50);
    Skills->AddExperience(UOpenWillowSkills::ExperienceForLevel(52));
    const TSharedPtr<FJsonObject> Capped = Skills->ProgressionJson();
    TestTrue(TEXT("Capped progression restores"), Loaded->RestoreProgression(*Capped, Reason));
    TestEqual(TEXT("Capped level restored"), Loaded->GetLevel(), 50);
    Capped->SetNumberField(TEXT("level"), 51);
    TestFalse(TEXT("Level above the cap refused"), Loaded->RestoreProgression(*Capped, Reason));
    return true;
}
#endif

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowSkillEffects.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

// Skill effects on an invented manifest (no game data): grade formula, stack combination, skill types and conditions.
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowSkillEffectsTest, "OpenWillow.SkillEffects",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)
bool FOpenWillowSkillEffectsTest::RunTest(const FString&)
{
    const TCHAR* Json = TEXT(R"({"format":"openwillow.skill_effects/1","skills":[
 {"id":"T.Act","name":"Act","type":"Action","maxGrade":1,"subjectToGradeRules":true,"effects":[],"conditions":[]},
 {"id":"T.Dmg","name":"Dmg","type":"Passive","maxGrade":5,"subjectToGradeRules":true,"conditions":[],"effects":[
  {"attribute":"A.Damage","modifierType":"Scale","target":"Self","startGrade":1,"interval":1,"base":{"constant":0.05},"perGrade":{"constant":0.05},"bonus":[]},
  {"attribute":"A.Interval","modifierType":"Scale","target":"Self","startGrade":1,"interval":1,"base":{"constant":-0.1},"perGrade":{"constant":-0.1},"bonus":[]},
  {"attribute":null,"modifierType":"PostAdd","target":"None","startGrade":1,"interval":2,"base":{"constant":1},"perGrade":{"constant":1},"bonus":[{"grade":3,"value":10},{"grade":5,"value":20}]}]},
 {"id":"T.Gated","name":"Gated","type":"Passive","maxGrade":5,"subjectToGradeRules":true,"effects":[
  {"attribute":"A.Damage","modifierType":"PostAdd","target":"Self","startGrade":1,"interval":1,"base":{"attribute":"A.Max","scale":0.5},"perGrade":{"constant":0},"bonus":[]}],
  "conditions":[{"whileActive":true,"test":{"kind":"and","left":{"kind":"attribute","attribute":"A.Locked","op":"EqualTo","constant":1},"right":{"kind":"onFoot"}}}]},
 {"id":"T.Aug","name":"Aug Skill","type":"ActionAugment","maxGrade":5,"subjectToGradeRules":true,"conditions":[],"effects":[
  {"attribute":"A.Damage","modifierType":"PreAdd","target":"Self","startGrade":1,"interval":1,"base":{"constant":2},"perGrade":{"constant":0},"bonus":[]}]},
 {"id":"T.Kill","name":"Kill","type":"Kill","maxGrade":5,"subjectToGradeRules":true,"conditions":[],"effects":[
  {"attribute":"A.Damage","modifierType":"PreAdd","target":"Self","startGrade":1,"interval":1,"base":{"constant":100},"perGrade":{"constant":0},"bonus":[]}]}]})");
    TSharedPtr<FJsonObject> Root;
    FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Json), Root);
    UOpenWillowSkills* Tree = NewObject<UOpenWillowSkills>();
    using FTier = UOpenWillowSkills::FTier;
    Tree->AddBranch({FTier{0, {{TEXT("T.Dmg"), 0, 5}, {TEXT("T.Gated"), 1, 5}, {TEXT("T.Aug"), 2, 5}}}});
    Tree->AddBranch({FTier{0, {{TEXT("T.Kill"), 0, 5}}}});
    Tree->SetActionSkill(1, 1, TEXT("T.Act"));
    Tree->SetLevel(50);
    FString Error;
    FOpenWillowSkillEffects& Fx = Tree->GetEffectsMutable();
    TestTrue(TEXT("Manifest loads"), Root && Fx.LoadJson(*Root, Error));
    TestTrue(TEXT("Find by id, display name and the id's last part"),
        Fx.Find(TEXT("Aug Skill")) && Fx.Find(TEXT("Aug")) == Fx.Find(TEXT("T.Aug")) && Fx.Find(TEXT("Aug")) == Fx.Find(TEXT("Aug Skill"))
        && !Fx.Find(TEXT("ug")));

    bool bLocked = false, bRunning = false;
    FOpenWillowSkillEffects::FWorld World;
    World.Value = [&](const FString& A, float& Out)
    {
        if (A == TEXT("A.Locked")) { Out = bLocked ? 1.f : 0.f; return true; }
        if (A == TEXT("A.Max")) { Out = 40.f; return true; }
        return false;
    };
    auto Stack = [&](const TCHAR* A) { World.bActionSkillRunning = bRunning; return Fx.Stack(*Tree, A, World); };
    TestTrue(TEXT("Nothing trained, nothing applies"), Stack(TEXT("A.Damage")).IsEmpty());

    FString Why;
    Tree->TrySpend(-1, -1, -1, Why);
    for (int32 I = 0; I < 3; ++I) Tree->TrySpend(0, 0, 0, Why);
    const FOpenWillowModifierStack D = Stack(TEXT("A.Damage"));
    TestTrue(TEXT("Grade 3 scale = 0.05 + 0.05 x 2"), FMath::IsNearlyEqual(D.ScaleUp, 0.15f) && D.Count == 1);
    TestTrue(TEXT("Damage 100 -> 115"), FMath::IsNearlyEqual(D.Apply(100.f), 115.f, 1e-3f));
    // A negative Scale divides: interval 1 x 1 / (1 + 0.3).
    TestTrue(TEXT("Interval scale -0.3 divides"), FMath::IsNearlyEqual(Stack(TEXT("A.Interval")).Apply(1.f), 1.f / 1.3f, 1e-5f));
    // Interval 2 and the bonus list: grade 3 -> 1 + 1 x (2 div 2) + bonus(3) = 12; grade 4 -> 1 + 1 + 10 = 12; grade 5 -> 1 + 2 + 20.
    TestTrue(TEXT("Effect value with interval and bonus at grade 3"), FMath::IsNearlyEqual(Fx.EffectValue(*Tree, TEXT("Dmg"), 2, World), 12.f));
    Tree->TrySpend(0, 0, 0, Why);
    Tree->TrySpend(0, 0, 0, Why);
    TestTrue(TEXT("Best bonus only, at grade 5"), FMath::IsNearlyEqual(Fx.EffectValue(*Tree, TEXT("Dmg"), 2, World), 23.f));
    TestEqual(TEXT("Below the start grade the value is 0"), FOpenWillowSkillEffects::ModifierValue(Fx.GetSkills()[1].Effects[0], 0, World), 0.f);

    // Conditions, augments and kill skills.
    Tree->TrySpend(0, 0, 1, Why);
    Tree->TrySpend(0, 0, 2, Why);
    Tree->TrySpend(1, 0, 0, Why);
    TestEqual(TEXT("Gated, augment and kill skills off at first"), Stack(TEXT("A.Damage")).Count, 1);
    bLocked = true;
    const FOpenWillowModifierStack Gated = Stack(TEXT("A.Damage"));
    TestTrue(TEXT("Condition met: attribute-valued PostAdd 40 x 0.5"), Gated.Count == 2 && FMath::IsNearlyEqual(Gated.PostAdd, 20.f));
    bRunning = true;
    const FOpenWillowModifierStack Running = Stack(TEXT("A.Damage"));
    TestTrue(TEXT("Augment applies while the action skill runs"), Running.Count == 3 && FMath::IsNearlyEqual(Running.PreAdd, 2.f));
    // (100 + 2) x 1.25 + 20
    TestTrue(TEXT("Stack formula"), FMath::IsNearlyEqual(Running.Apply(100.f), 147.5f, 1e-3f));
    // Kill skills count only while the kill-skill timer runs; a kill restarts it at 7 s (refresh is UNVERIFIED).
    World.bKillSkillsActive = true;
    TestEqual(TEXT("Kill skill applies while the timer runs"), Stack(TEXT("A.Damage")).Count, 4);
    World.bKillSkillsActive = false;
    Tree->OnEnemyKilled(10.f);
    TestTrue(TEXT("Kill timer runs after a kill"), Tree->KillSkillsActiveAt(16.9f));
    TestFalse(TEXT("Kill timer ends after 7 s"), Tree->KillSkillsActiveAt(17.f));
    Tree->OnEnemyKilled(15.f);
    TestTrue(TEXT("A second kill restarts the timer"), Tree->KillSkillsActiveAt(21.9f));

    // A balance-formula value: 0.5 x (A.Max ^ 1 + (-1 x A.Cur)) = half the missing amount; mode 1 adds the base 3.
    FOpenWillowSkillEffects::FEffect Missing;
    FOpenWillowSkillEffects::FValue Constant, Max, Cur, Power;
    Constant.Constant = 0.5f; Max.Attribute = TEXT("A.Max"); Cur.Attribute = TEXT("A.Cur"); Cur.Scale = -1.f; Power.Constant = 1.f;
    FOpenWillowSkillEffects::FValue Base; Base.Constant = 3.f;
    Missing.Base.Formula = {Constant, Max, Power, Cur, Base};
    Missing.Base.Mode = 1;
    World.Value = [](const FString& A, float& Out)
    {
        if (A == TEXT("A.Max")) { Out = 40.f; return true; }
        if (A == TEXT("A.Cur")) { Out = 10.f; return true; }
        return false;
    };
    TestTrue(TEXT("Formula value with base mode add"), FMath::IsNearlyEqual(FOpenWillowSkillEffects::ModifierValue(Missing, 1, World), 18.f));

    // Skill damage formulas: 8 x 1.13 ^ level and 1.13 ^ level.
    TestTrue(TEXT("PlayerSkillDamage at level 1"), FMath::IsNearlyEqual(UOpenWillowSkills::SkillDamageAt(1), 9.04f, 1e-3f));
    TestTrue(TEXT("PlayerSkillStatusEffectDamage at level 2"), FMath::IsNearlyEqual(UOpenWillowSkills::SkillStatusDamageAt(2), 1.2769f, 1e-3f));

    Tree->ResetTree();
    TestTrue(TEXT("A respec removes every modifier"), Stack(TEXT("A.Damage")).IsEmpty());

    // Malformed manifests are refused whole and leave no effects loaded (every hook then returns its base value).
    const TCHAR* BadManifests[] = {
        TEXT("{\"format\":\"openwillow.skill_effects/1\",\"skills\":[null]}"),
        TEXT("{\"format\":\"openwillow.skill_effects/1\",\"skills\":[{\"id\":\"T.X\",\"type\":\"Passive\",\"maxGrade\":\"five\",\"effects\":[]}]}"),
        TEXT("{\"format\":\"openwillow.skill_effects/1\",\"skills\":[{\"id\":\"T.X\",\"type\":\"Passive\",\"maxGrade\":1,\"effects\":[7]}]}"),
        TEXT("{\"format\":\"openwillow.skill_effects/1\",\"skills\":[{\"id\":\"T.X\",\"type\":\"Passive\",\"maxGrade\":1,\"effects\":[],\"conditions\":[null]}]}"),
        TEXT("{\"format\":\"openwillow.skill_effects/1\",\"skills\":{\"id\":\"T.X\"}}"),
        TEXT("{\"format\":\"openwillow.skill_effects/1\",\"skills\":[{\"id\":\"T.X\",\"type\":\"Passive\",\"maxGrade\":\"5\",\"effects\":[]}]}"),
        TEXT("{\"format\":\"openwillow.skill_effects/1\",\"skills\":[{\"id\":\"T.X\",\"type\":\"Passive\",\"maxGrade\":1,\"effects\":[{\"modifierType\":\"Scale\",\"target\":\"Self\",\"startGrade\":\"1\"}]}]}"),
        TEXT("{\"format\":\"openwillow.skill_effects/1\",\"skills\":[{\"id\":\"T.X\",\"type\":\"Passive\",\"maxGrade\":1,\"effects\":[{\"modifierType\":\"Scale\",\"target\":\"Self\",\"bonus\":[{\"grade\":\"2\",\"value\":1}]}]}]}"),
        TEXT("{\"format\":2,\"skills\":[]}")};
    for (const TCHAR* Bad : BadManifests)
    {
        TSharedPtr<FJsonObject> BadRoot;
        FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Bad), BadRoot);
        FString BadError;
        TestFalse(FString::Printf(TEXT("Malformed manifest refused: %s"), Bad), BadRoot && Fx.LoadJson(*BadRoot, BadError));
        TestFalse(TEXT("No effects left loaded after a refused manifest"), Fx.IsLoaded());
    }
    return true;
}
#endif
