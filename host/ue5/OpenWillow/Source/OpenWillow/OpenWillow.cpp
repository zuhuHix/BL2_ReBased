#include "Modules/ModuleManager.h"
#include "HAL/PlatformMisc.h"
#include "Misc/Paths.h"
#include "Misc/MessageDialog.h"

class FOpenWillowModule : public FDefaultGameModuleImpl
{
public:
    virtual void StartupModule() override
    {
        const FString GameRoot = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_BL2"));
        const bool Installed = !GameRoot.IsEmpty()
            && FPaths::FileExists(FPaths::Combine(GameRoot, TEXT("Binaries/Win32/Borderlands2.exe")))
            && FPaths::FileExists(FPaths::Combine(GameRoot, TEXT("WillowGame/CookedPCConsole/Core.upk")))
            && FPaths::FileExists(FPaths::Combine(GameRoot, TEXT("WillowGame/CookedPCConsole/WillowGame.upk")));
        if (!Installed)
        {
            UE_LOG(LogTemp, Error, TEXT("Set OPENWILLOW_BL2 to your installed Borderlands 2 directory."));
            FPlatformMisc::RequestExit(true);
            return;
        }
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Phase 0 host initialized. Original game installation found."));
    }
};

IMPLEMENT_PRIMARY_GAME_MODULE(FOpenWillowModule, OpenWillow, "OpenWillow");
