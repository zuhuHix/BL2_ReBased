#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "HAL/PlatformTime.h"
#include "OpenWillowMapSelector.h"

// Run in a standalone -game instance on any imported map. Opens a different
// imported map through the selector and waits for the world to change. The
// digit/Tab bindings themselves still need a physical key press to confirm.
class FOpenWillowMapSelectorCheck : public IAutomationLatentCommand
{
public:
    explicit FOpenWillowMapSelectorCheck(FAutomationTestBase* InTest)
        : Test(InTest), Started(FPlatformTime::Seconds()) {}

    bool Update() override
    {
        const double Now = FPlatformTime::Seconds();
        UWorld* World = nullptr;
        for (const FWorldContext& Context : GEngine->GetWorldContexts())
        {
            if (Context.WorldType == EWorldType::Game)
            {
                World = Context.World();
                break;
            }
        }
        if (Now - Started > 180)
        {
            Test->AddError(TEXT("Map selector check timed out"));
            return true;
        }
        AOpenWillowPlayerController* Player = World
            ? Cast<AOpenWillowPlayerController>(World->GetFirstPlayerController()) : nullptr;
        if (!Player || !Player->GetPawn()) return false;

        if (Stage == 0)
        {
            if (Now - Started < 10) return false;
            const FString Current = World->GetMapName();
            const TArray<FOpenWillowMapEntry> Maps = AOpenWillowPlayerController::DiscoverMaps();
            int32 Imported = 0, CurrentIndex = 0, TargetIndex = 0;
            for (int32 i = 0; i < Maps.Num(); ++i)
            {
                Test->AddInfo(FString::Printf(TEXT("Map selector entry %d: %s prepared=%d imported=%d"),
                    i + 1, *Maps[i].Map, Maps[i].bPrepared, Maps[i].bImported));
                if (Maps[i].bImported) ++Imported;
                if (Maps[i].Map == Current) CurrentIndex = i + 1;
                else if (Maps[i].bImported && !TargetIndex) TargetIndex = i + 1;
            }
            Test->TestTrue(TEXT("Selector lists the running map as imported"), CurrentIndex > 0);
            Test->TestTrue(TEXT("Selector rejects an out-of-range entry"), !Player->OpenEntry(Maps.Num() + 1));
            if (!TargetIndex)
            {
                Test->AddWarning(TEXT("Only one imported map; the level switch was not exercised"));
                return true;
            }
            Target = Maps[TargetIndex - 1].Map;
            Test->AddInfo(FString::Printf(TEXT("Map selector: %d imported, switching %s -> %s"), Imported, *Current, *Target));
            Test->TestTrue(TEXT("Selector accepts an imported entry"), Player->OpenEntry(TargetIndex));
            Stage = 1;
            StageStarted = Now;
        }
        else if (Stage == 1)
        {
            if (World->GetMapName() != Target) return false;
            if (Now - StageStarted < 5) return false;
            Test->TestTrue(TEXT("Selected map became the running world"), World->GetMapName() == Target);
            Test->TestTrue(TEXT("New world uses the selector controller"), Player != nullptr);
            Test->AddInfo(FString::Printf(TEXT("Map selector: now running %s after %.1f s"), *World->GetMapName(), Now - StageStarted));
            return true;
        }
        return false;
    }

private:
    FAutomationTestBase* Test;
    double Started;
    double StageStarted = 0;
    int32 Stage = 0;
    FString Target;
};

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowMapSelectorTest, "OpenWillow.MapSelector",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::ProductFilter)

bool FOpenWillowMapSelectorTest::RunTest(const FString& Parameters)
{
    ADD_LATENT_AUTOMATION_COMMAND(FOpenWillowMapSelectorCheck(this));
    return true;
}
#endif
