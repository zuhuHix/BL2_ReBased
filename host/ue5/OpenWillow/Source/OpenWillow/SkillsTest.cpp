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
