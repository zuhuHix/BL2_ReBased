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
//   OWINVTEST step=<n> action=<name> status=<PASS|FAIL|NOT_RUN|KNOWN_DIVERGENCE> ok=<0|1> detail=<text>
// and the run ends with "OWINVTEST SUMMARY result=... passed= failed= not_run=
// known_divergence=", then quits. Status meanings:
//   PASS              precondition held, the exact postcondition was observed.
//   FAIL              the postcondition was not observed.
//   NOT_RUN           the step's own precondition did not hold (an earlier step
//                     left the wrong state), so no input was sent and nothing was
//                     claimed. Also used for steps never reached after an abort.
//   KNOWN_DIVERGENCE  the host does what it is documented to do today, and that is
//                     known to differ from the original game. Never counted as a
//                     pass; the reason is part of the detail.
// result is PASS only when every step passed; PASS_WITH_KNOWN_DIVERGENCE when the
// only non-passes are known divergences; FAIL otherwise.
// The inventory fixtures that test host behaviour are synthetic (a fake shield
// added by the test itself); nothing from a player's save is read.
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
        // Checked before Begin() against a fresh page report (when bPageReport):
        // false means NOT_RUN with the returned reason, and no input is sent.
        TFunction<bool(FString&)> Precondition;
        bool bPreReport = true;       // fetch a fresh page report first (false when the page is closed)
        // Verify passing is reported as KNOWN_DIVERGENCE with this reason.
        FString DivergenceReason;
    };
    enum class EStatus : uint8 { Pass, Fail, NotRun, KnownDivergence };
    enum class EPhase : uint8 { Waiting, PreReport, WaitAction, Settle, WaitReport, Retry, WaitCapture, AdvanceAfterCapture, Finished };

    void BuildSteps();
    void BeginStep(float Now);
    void Attempt(float Now, bool bGotReport);
    void FinishStep(bool bOk, const FString& Detail);
    void FinishStepWithStatus(EStatus Status, const FString& Detail);
    void RunBegin(float Now);
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
    int32 NotRun = 0;
    int32 KnownDivergences = 0;
    FKey PendingRelease;
    bool bHavePendingRelease = false;
    FString PendingScreenshot;
    float ScreenshotAt = 0.f;
    // Carried between steps.
    FString SelId, PrevSel, DisplacedId, DropId, DropName, ShieldId, TransferCandidateId, ForgedId;
    bool bDragControlOk = false;       // DOM drag reached the host (positive control for the refused drag)
    FString WalkLastSel;               // WalkTo: where the last arrow burst started from
    FString WalkExpected;              // WalkTo: where that burst should end ("" = any change)
    float WalkLastPressAt = -100.f;
    int32 SortBefore = -1;
    int32 CountBefore = 0, PickupAttemptsBefore = 0;
    int32 VmCallsBefore = 0;
    FString VmExpectedId;
    TArray<FString> FillerIds;
    TArray<FString> VisibleBeforeScroll;
    bool bMeasurePreviewLoop = false;
    int32 PreviewLoopSamples = 0;
    FVector2D PreviewHeadMin, PreviewHeadMax;
};
