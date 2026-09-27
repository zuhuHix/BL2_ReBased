#include "Misc/AutomationTest.h"
#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowSkills.h"

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

    Skills->SetLevel(46);
    TestEqual(TEXT("Traced level 46 threshold"), Skills->GetExperience(), int64(2715586));
    Skills->AddExperience(UOpenWillowSkills::ExperienceForLevel(47) - Skills->GetExperience() - 1);
    TestEqual(TEXT("One XP short of 47"), Skills->GetLevel(), 46);
    Skills->AddExperience(1);
    TestEqual(TEXT("Level up at the threshold"), Skills->GetLevel(), 47);
    TestEqual(TEXT("Grades survive a level up"), Skills->GetActionGrade(), 1);
    return true;
}
#endif
