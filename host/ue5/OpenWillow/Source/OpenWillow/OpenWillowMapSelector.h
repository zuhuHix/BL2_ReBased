#pragma once
#include "CoreMinimal.h"
#include "GameFramework/PlayerController.h"
#include "OpenWillowMapSelector.generated.h"

// One entry of the in-game map list: a prepared scene under local/ and/or an
// imported map under Content/OpenWillow. Preparation and import stay CLI-only.
struct FOpenWillowMapEntry
{
    FString Map;
    bool bPrepared = false;
    bool bImported = false;
    FString LevelPath() const { return FString::Printf(TEXT("/Game/OpenWillow/%s/%s"), *Map, *Map); }
};

// Tab toggles an on-screen list of maps; the digit keys open an imported one.
// A UI convenience over the existing saved scenes, not a loading pipeline.
UCLASS()
class OPENWILLOW_API AOpenWillowPlayerController : public APlayerController
{
    GENERATED_BODY()
public:
    static TArray<FOpenWillowMapEntry> DiscoverMaps();
    static bool IsPersistentMapName(const FString& Name);

    // Returns true when a level open was requested.
    bool OpenEntry(int32 Number);
    const TArray<FOpenWillowMapEntry>& Entries() const { return Maps; }
    bool IsMenuOpen() const { return bMenuOpen; }

    UFUNCTION(Exec) void OWMapList();
    UFUNCTION(Exec) void OWMapOpen(int32 Number);

protected:
    virtual void BeginPlay() override;
    virtual void SetupInputComponent() override;
    virtual void Tick(float DeltaSeconds) override;

private:
    void ToggleMenu();
    void OnDigitKey(FKey Key);
    void DrawMenu(float Duration) const;

    TArray<FOpenWillowMapEntry> Maps;
    bool bMenuOpen = false;
    bool bOpenRequested = false;
};
