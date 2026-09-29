#pragma once

#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "InputCoreTypes.h"
#include "OpenWillowInventoryActionTest.generated.h"

class FJsonObject;
class AOpenWillowMayaHUD;
class AOpenWillowWalker;
class APlayerController;

// -owinventoryactions: drives the real inventory page -> host round trip in a
// running game. It opens the imported inventory page with the I key, sends the
// page the keys a player would press (Enter, Delete, V, T, Q, ...), waits for
// the host to apply each OWITEM request, asks the page what it now shows and
// compares both with the host inventory. Every step logs one line:
//   OWINVTEST step=<n> action=<name> ok=<0|1> detail=<text>
// and the run ends with "OWINVTEST SUMMARY result=PASS|FAIL ...", then quits.
// Keys reach the page as Slate key events by default (-owinventoryjskeys sends
// synthetic DOM events instead, the way -owcombatshots does). Test only: it
// edits the inventory (fills the backpack, changes Maya's level).
UCLASS()
class OPENWILLOW_API UOpenWillowInventoryActionTest : public UActorComponent
{
    GENERATED_BODY()
public:
    UOpenWillowInventoryActionTest();
    virtual void TickComponent(float DeltaTime, ELevelTick TickType, FActorComponentTickFunction* ThisTickFunction) override;

private:
    struct FStep
    {
        FString Name;
        bool bExpectAction = false;   // wait for the host to process a page request
        bool bPageReport = true;      // ask the page for its state before verifying
        float SettleSeconds = 1.f;    // pause when no host action is expected
        float DeadlineSeconds = 5.f;  // verify may keep failing (and retry) this long
        TFunction<void()> Begin;
        TFunction<bool(FString&)> Verify;
    };
    enum class EPhase : uint8 { Waiting, WaitAction, Settle, WaitReport, Retry, Finished };

    void BuildSteps();
    void BeginStep(float Now);
    void Attempt(float Now, bool bGotReport);
    void FinishStep(bool bOk, const FString& Detail);
    void Summarize(const TCHAR* Reason);
    void PressKey(const TCHAR* Name);
    void PressGameKey(const FKey& Key);
    TSharedPtr<FJsonObject> PageObject() const;
    FString PageSlot(const TSharedPtr<FJsonObject>& Page, int32 Slot) const;
    bool PageItemFlags(const TSharedPtr<FJsonObject>& Page, const FString& Id, int32& Favorite, int32& Trash) const;
    bool PageHasItem(const TSharedPtr<FJsonObject>& Page, const FString& Id) const;
    FString HostSlotId(int32 Slot) const;
    bool HostEquipped(const FString& Id) const;
    AActor* FindPickup(FString* OutName = nullptr) const;

    // Refreshed every tick; only used inside a tick or a step callback.
    AOpenWillowWalker* Walker = nullptr;
    APlayerController* PC = nullptr;
    AOpenWillowMayaHUD* Hud = nullptr;
    TArray<FStep> Steps;
    int32 StepIndex = 0;
    EPhase Phase = EPhase::Waiting;
    float PhaseStart = 0.f;
    float StepStart = 0.f;
    float NextReportAt = 0.f;
    float FinishedAt = 0.f;
    float FirstVerifyAt = -1.f;
    float RetryAt = 0.f;
    float StartedAt = -1.f;
    int32 ActionSerialAtBegin = 0;
    int32 ReportSerialAtRequest = 0;
    bool bUseJsKeys = false;
    int32 Passed = 0;
    int32 Failed = 0;
    FKey PendingRelease;
    bool bHavePendingRelease = false;
    // Carried between steps.
    FString SelId, PrevSel, DisplacedId, DropId, DropName, ShieldId;
    int32 CountBefore = 0, PickupAttemptsBefore = 0;
    TArray<FString> FillerIds;
};
