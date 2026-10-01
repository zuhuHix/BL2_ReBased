#include "OpenWillowInventoryActionTest.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInventoryMayaDisplay.h"
#include "Components/SkeletalMeshComponent.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowSkills.h"
#include "OpenWillowWalker.h"
#include "Dom/JsonObject.h"
#include "Engine/Engine.h"
#include "Engine/GameViewportClient.h"
#include "UnrealClient.h"
#include "EngineUtils.h"
#include "GameFramework/PlayerController.h"
#include "InputKeyEventArgs.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

namespace
{
constexpr float ActionWaitSeconds = 8.f;   // host must see the page's request within this
constexpr float ReportWaitSeconds = 2.f;   // page must answer a report request within this
constexpr float RetrySeconds = 0.7f;
constexpr float WholeRunSeconds = 420.f;

// Page report accessors. A null page or a missing/null field gives "" / the default, never a stale value.
FString PageString(const TSharedPtr<FJsonObject>& Page, const TCHAR* Field)
{
    FString Value;
    if (Page.IsValid()) Page->TryGetStringField(Field, Value);
    return Value;
}
double PageNumber(const TSharedPtr<FJsonObject>& Page, const TCHAR* Field, double Default = -1.)
{
    double Value = Default;
    if (!Page.IsValid() || !Page->TryGetNumberField(Field, Value)) return Default;
    return Value;
}
bool PageHasString(const TSharedPtr<FJsonObject>& Page, const TCHAR* Field)
{
    return Page.IsValid() && Page->HasTypedField<EJson::String>(Field);
}
}

UOpenWillowInventoryActionTest::UOpenWillowInventoryActionTest()
{
    PrimaryComponentTick.bCanEverTick = true;
    PrimaryComponentTick.TickGroup = TG_PostUpdateWork;
}

TSharedPtr<FJsonObject> UOpenWillowInventoryActionTest::PageObject() const
{
    if (!Hud || Hud->PageReport().IsEmpty()) return nullptr;
    TSharedPtr<FJsonObject> Object;
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Hud->PageReport()), Object)) return nullptr;
    return Object;
}

FString UOpenWillowInventoryActionTest::PageSlot(const TSharedPtr<FJsonObject>& Page, int32 Slot) const
{
    const TArray<TSharedPtr<FJsonValue>>* Slots = nullptr;
    FString Id;
    if (Page && Page->TryGetArrayField(TEXT("slots"), Slots) && Slots->IsValidIndex(Slot))
        (*Slots)[Slot]->TryGetString(Id); // a null slot leaves Id empty
    return Id;
}

bool UOpenWillowInventoryActionTest::PageItemFlags(const TSharedPtr<FJsonObject>& Page, const FString& Id,
    int32& Favorite, int32& Trash) const
{
    const TArray<TSharedPtr<FJsonValue>>* Items = nullptr;
    if (!Page || !Page->TryGetArrayField(TEXT("items"), Items)) return false;
    for (const TSharedPtr<FJsonValue>& Value : *Items)
    {
        const TArray<TSharedPtr<FJsonValue>>* Row = nullptr;
        FString RowId;
        if (!Value->TryGetArray(Row) || Row->Num() < 3 || !(*Row)[0]->TryGetString(RowId) || RowId != Id) continue;
        Favorite = int32((*Row)[1]->AsNumber());
        Trash = int32((*Row)[2]->AsNumber());
        return true;
    }
    return false;
}

bool UOpenWillowInventoryActionTest::PageHasItem(const TSharedPtr<FJsonObject>& Page, const FString& Id) const
{
    int32 Favorite, Trash;
    return PageItemFlags(Page, Id, Favorite, Trash);
}

FString UOpenWillowInventoryActionTest::HostSlotId(int32 Slot) const
{
    const FOpenWillowWeaponItem* Item = Walker->GetInventory()->SlotItem(Slot);
    return Item ? UOpenWillowInventory::StableId(*Item) : FString();
}

bool UOpenWillowInventoryActionTest::HostEquipped(const FString& Id) const
{
    for (int32 Slot = 0; Slot < UOpenWillowInventory::SlotCount; ++Slot)
        if (HostSlotId(Slot) == Id) return true;
    return false;
}

AActor* UOpenWillowInventoryActionTest::FindPickup(FString* OutName) const
{
    if (!GetWorld() || !Walker) return nullptr;
    for (TActorIterator<AOpenWillowInventoryPickup> It(GetWorld()); It; ++It)
    {
        if (FVector::Dist(It->GetActorLocation(), Walker->GetActorLocation()) > 300.f) continue;
        if (OutName) *OutName = It->DisplayName();
        return *It;
    }
    return nullptr;
}

void UOpenWillowInventoryActionTest::PressKey(const TCHAR* Name)
{
    if (!Hud) return;
    if (bUseJsKeys) Hud->SendPageKey(Name);
    else if (!Hud->SendSlateKey(Name)) UE_LOG(LogTemp, Warning, TEXT("OWINVTEST key %s has no Slate mapping"), Name);
}

void UOpenWillowInventoryActionTest::PressGameKey(const FKey& Key)
{
    // The player controller's own input path: the same action mappings a
    // keyboard press goes through (I opens the inventory, E picks up).
    if (!PC) return;
    PC->InputKey(FInputKeyEventArgs::CreateSimulated(Key, IE_Pressed, 1.f));
    PendingRelease = Key;
    bHavePendingRelease = true;
}

void UOpenWillowInventoryActionTest::BuildSteps()
{
    // Each step: Begin() sends input, Verify() compares host and page state.
    auto Add = [this](const TCHAR* Name, bool bExpectAction, TFunction<void()> Begin, TFunction<bool(FString&)> Verify,
        float Deadline = 5.f, bool bPageReport = true, float Settle = 1.f)
    {
        FStep Step;
        Step.Name = Name;
        Step.bExpectAction = bExpectAction;
        Step.bPageReport = bPageReport;
        Step.SettleSeconds = Settle;
        Step.DeadlineSeconds = Deadline;
        Step.Begin = MoveTemp(Begin);
        Step.Verify = MoveTemp(Verify);
        Steps.Add(MoveTemp(Step));
    };
    // The host must have handled a new request of this kind, and accepted or
    // refused it as expected.
    auto CheckAction = [this](const TCHAR* Action, bool bWantAccepted, const FString& WantId, FString& D)
    {
        const AOpenWillowMayaHUD::FInventoryActionRecord& Record = Hud->LastInventoryAction();
        if (Record.Serial <= ActionSerialAtBegin) { D = TEXT("host saw no new page request"); return false; }
        if (Record.Action != Action) { D = FString::Printf(TEXT("host handled '%s', wanted '%s'"), *Record.Action, Action); return false; }
        if (!WantId.IsEmpty() && Record.Id != WantId) { D = FString::Printf(TEXT("host handled id %s, wanted %s"), *Record.Id, *WantId); return false; }
        if (Record.bAccepted != bWantAccepted) { D = FString::Printf(TEXT("host %s the request"), Record.bAccepted ? TEXT("accepted") : TEXT("refused")); return false; }
        D = FString::Printf(TEXT("%s %s"), Action, bWantAccepted ? TEXT("accepted") : TEXT("refused"));
        return true;
    };
    auto Snapshot = [this](FString& D)
    {
        TSharedPtr<FJsonObject> Page = PageObject();
        if (!Page) { D = TEXT("no page report"); return TSharedPtr<FJsonObject>(); }
        return Page;
    };
    auto OpenVerify = [this, Snapshot](FString& D)
    {
        if (!Hud->IsInventoryOpen()) { D = TEXT("inventory page is not open"); return false; }
        TSharedPtr<FJsonObject> Page = Snapshot(D);
        if (!Page) return false;
        bool bReady = false, bHasState = false;
        Page->TryGetBoolField(TEXT("ready"), bReady);
        Page->TryGetBoolField(TEXT("hasState"), bHasState);
        if (!bReady || !bHasState) { D = FString::Printf(TEXT("page ready=%d state=%d"), bReady, bHasState); return false; }
        bool bHeaderAnchored = false;
        Page->TryGetBoolField(TEXT("backpackHeaderAnchored"), bHeaderAnchored);
        if (!bHeaderAnchored) { D = TEXT("backpack category controls are not above the first row"); return false; }
        FString Shown;
        for (int32 Slot = 0; Slot < UOpenWillowInventory::SlotCount; ++Slot)
        {
            if (PageSlot(Page, Slot) != HostSlotId(Slot))
            {
                D = FString::Printf(TEXT("page slot %d shows '%s', host has '%s'"), Slot + 1, *PageSlot(Page, Slot), *HostSlotId(Slot));
                return false;
            }
            Shown += FString::Printf(TEXT("%s%s"), Slot ? TEXT(",") : TEXT(""), *HostSlotId(Slot));
        }
        D = FString::Printf(TEXT("page open and ready, slots [%s] match host, keys=%s"), *Shown, bUseJsKeys ? TEXT("js") : TEXT("slate"));
        return true;
    };

    // ---- Per-step preconditions and shared helpers -------------------------------------------------
    // Pre() attaches a precondition to the step just added. It runs before any input is sent, against a
    // fresh page report (unless bFreshReport is false, e.g. while the page is closed). A step whose
    // precondition fails is NOT_RUN: it sent nothing and claims nothing, so a wrong state left by an
    // earlier step shows up as a cascade of NOT_RUN rows instead of false passes.
    auto Pre = [this](TFunction<bool(FString&)> Check, bool bFreshReport = true)
    {
        Steps.Last().Precondition = MoveTemp(Check);
        Steps.Last().bPreReport = bFreshReport;
    };
    auto Diverges = [this](const TCHAR* Reason) { Steps.Last().DivergenceReason = Reason; };
    auto PageOrWhy = [this](FString& Why)
    {
        TSharedPtr<FJsonObject> Page = PageObject();
        if (!Page) Why = TEXT("no page report");
        return Page;
    };
    // A weapon that exists in the host backpack and is not in a weapon slot.
    auto InBackpack = [this](const FString& Id)
    {
        return !Id.IsEmpty() && Walker->GetInventory()->FindItemById(Id) && !HostEquipped(Id);
    };
    // "<what> is '<got>', needed '<want>'"
    auto Needed = [](FString& Why, const TCHAR* What, const FString& Got, const FString& Want)
    {
        Why = FString::Printf(TEXT("%s is '%s', needed '%s'"), What, *Got, *Want);
        return false;
    };
    // Walk the page selection to Target with arrow keys. Each attempt sends one burst of up to eight presses
    // (the page queues them in order) and then waits for the selection to arrive where the burst should end
    // before sending the next, so a slow page report cannot make it overshoot. Used instead of assuming
    // where a drag, drop, unequip or reopen left the selection.
    auto WalkTo = [this, PageOrWhy](const FString& Target, FString& D)
    {
        TSharedPtr<FJsonObject> Page = PageOrWhy(D);
        if (!Page) return false;
        if (Target.IsEmpty()) { D = TEXT("no target id to walk to"); return false; }
        const FString Sel = PageString(Page, TEXT("sel"));
        if (Sel == Target) { D = FString::Printf(TEXT("page selection is %s"), *Target); return true; }
        const TArray<TSharedPtr<FJsonValue>>* Rows = nullptr;
        if (!Page->TryGetArrayField(TEXT("backpack"), Rows)) { D = TEXT("page lists no backpack rows"); return false; }
        int32 TargetRow = INDEX_NONE, SelRow = INDEX_NONE;
        for (int32 I = 0; I < Rows->Num(); ++I)
        {
            const FString Row = (*Rows)[I]->AsString();
            if (Row == Target) TargetRow = I;
            if (Row == Sel) SelRow = I;
        }
        if (TargetRow == INDEX_NONE) { D = FString::Printf(TEXT("%s is not among the page's backpack rows"), *Target); return false; }
        const float Now = GetWorld()->GetRealTimeSeconds();
        const bool bArrived = WalkExpected.IsEmpty() ? Sel != WalkLastSel : Sel == WalkExpected;
        if (!bArrived && Now - WalkLastPressAt < 6.f) { D = TEXT("waiting for the previous arrow keys to take effect"); return false; }
        WalkLastSel = Sel;
        WalkLastPressAt = Now;
        if (SelRow == INDEX_NONE)
        {
            // A selection outside the backpack list (an equipment cell, or none) re-enters the backpack with Right.
            WalkExpected.Reset();
            PressKey(TEXT("ArrowRight"));
        }
        else
        {
            const int32 Distance = FMath::Abs(TargetRow - SelRow), Burst = FMath::Min(Distance, 8);
            WalkExpected = (*Rows)[SelRow + (TargetRow > SelRow ? Burst : -Burst)]->AsString();
            for (int32 I = 0; I < Burst; ++I) PressKey(TargetRow > SelRow ? TEXT("ArrowDown") : TEXT("ArrowUp"));
        }
        D = FString::Printf(TEXT("walking: selection '%s' -> '%s'"), *Sel, *Target);
        return false;
    };

    // Synthetic gear fixture. The suite's gear steps must not depend on a local manifest, which can only come
    // from a player's save. The test adds one obviously fake shield (level 36, fake stats) to the host
    // inventory before the page opens. The weapons are the seeded local recipe demo set, selected by what
    // the page reports and never by a hard-coded id.
    SelId.Reset(); PrevSel.Reset(); DisplacedId.Reset(); DropId.Reset(); DropName.Reset(); ShieldId.Reset();
    TransferCandidateId.Reset(); ForgedId.Reset(); FillerIds.Reset();
    bDragControlOk = false;
    {
        FOpenWillowGearItem Shield;
        Shield.Id = TEXT("test_shield_synthetic_1");
        Shield.ItemType = TEXT("shield");
        Shield.Name = TEXT("TEST SHIELD (SYNTHETIC)");
        Shield.Manufacturer = TEXT("TESTCO");
        Shield.RarityColor = TEXT("#FFFFFF");
        Shield.FunStats = TEXT("Synthetic test fixture, not game data.");
        Shield.Level = 36; Shield.bLevelKnown = true;
        Shield.Rarity = 1; Shield.bRarityKnown = true;
        Shield.SaleValue = 1; Shield.bSaleValueKnown = true;
        FOpenWillowGearStat Capacity; Capacity.Label = TEXT("Test Capacity"); Capacity.Value = TEXT("100");
        Shield.Stats.Add(MoveTemp(Capacity));
        UOpenWillowInventory* Inv = Walker->GetInventory();
        if (Inv->AddGearToBackpack(MoveTemp(Shield))) ShieldId = Inv->GearItemList().Last().Id;
        UE_LOG(LogTemp, Display, TEXT("OWINVTEST fixture synthetic shield id='%s' (empty means it could not be added)"), *ShieldId);
    }

    // Tab is the primary key (the controller forwards it to Maya); I is exercised at the reopen step.
    Add(TEXT("open_inventory"), false, [this] { PressGameKey(EKeys::Tab); }, OpenVerify, 120.f);
    Add(TEXT("maya_armed_idle_full_loop_framing"), false,
        [this]
        {
            bMeasurePreviewLoop = true;
            PreviewLoopSamples = 0;
            PreviewHeadMin = FVector2D(1., 1.);
            PreviewHeadMax = FVector2D(0., 0.);
        },
        [this](FString& D)
        {
            bMeasurePreviewLoop = false;
            D = FString::Printf(TEXT("%d head samples over 13 s; normalized x %.3f..%.3f y %.3f..%.3f"),
                PreviewLoopSamples, PreviewHeadMin.X, PreviewHeadMax.X, PreviewHeadMin.Y, PreviewHeadMax.Y);
            return PreviewLoopSamples >= 100 && PreviewHeadMin.X >= .78 && PreviewHeadMax.X <= .98
                && PreviewHeadMin.Y >= .10 && PreviewHeadMax.Y <= .40;
        }, 18.f, false, 13.f);
    Pre([this](FString& Why) { if (!Hud->IsInventoryOpen()) { Why = TEXT("inventory page is not open"); return false; } return true; }, false);

    Add(TEXT("backpack_wheel_scrolls_one_row"), false,
        [this]
        {
            VisibleBeforeScroll.Reset();
            if (auto Page = PageObject())
            {
                const TArray<TSharedPtr<FJsonValue>>* Visible = nullptr;
                if (Page->TryGetArrayField(TEXT("visibleBackpack"), Visible))
                    for (const auto& Id : *Visible) VisibleBeforeScroll.Add(Id->AsString());
            }
            Hud->SendPageBackpackWheel(100);
        },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            const TArray<TSharedPtr<FJsonValue>>* Visible = nullptr;
            double First = -1;
            if (!Page || !Page->TryGetNumberField(TEXT("firstRow"), First) || First != 1
                || !Page->TryGetArrayField(TEXT("visibleBackpack"), Visible)
                || VisibleBeforeScroll.Num() != 7 || Visible->Num() != 7)
            { D = TEXT("wheel did not advance exactly one visible row"); return false; }
            for (int32 I = 0; I < 6; ++I)
                if ((*Visible)[I]->AsString() != VisibleBeforeScroll[I+1])
                { D = TEXT("wheel skipped/reordered the retained six rows"); return false; }
            D = TEXT("one row advanced; six previous items retained in order"); return true;
        });
    Pre([this, PageOrWhy](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            const TArray<TSharedPtr<FJsonValue>>* Rows = nullptr;
            if (!Page) return false;
            if (PageNumber(Page, TEXT("firstRow")) != 0) { Why = TEXT("backpack is not scrolled to the top"); return false; }
            if (!Page->TryGetArrayField(TEXT("backpack"), Rows) || Rows->Num() < 9) { Why = TEXT("fewer than nine backpack rows, a one-row scroll cannot be shown"); return false; }
            return true;
        });
    Add(TEXT("backpack_scroll_clamps_at_top"), false,
        [this] { Hud->SendPageBackpackWheel(-10000); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            const TArray<TSharedPtr<FJsonValue>>* Visible = nullptr;
            double First = -1;
            if (!Page || !Page->TryGetNumberField(TEXT("firstRow"), First) || First != 0
                || !Page->TryGetArrayField(TEXT("visibleBackpack"), Visible) || Visible->Num() != VisibleBeforeScroll.Num())
            { D = TEXT("scroll did not clamp to the original top window"); return false; }
            for (int32 I = 0; I < VisibleBeforeScroll.Num(); ++I)
                if ((*Visible)[I]->AsString() != VisibleBeforeScroll[I])
                { D = TEXT("top window differs after scroll return"); return false; }
            D = TEXT("large upward wheel delta clamps to original top items"); return true;
        });
    Pre([this, PageOrWhy](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageNumber(Page, TEXT("firstRow")) != 1) { Why = TEXT("backpack is not scrolled down one row"); return false; }
            return true;
        });

    Add(TEXT("inspect_weapon"), false, [this] { PressKey(TEXT("f")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            bool Open = false;
            double Bytes = 0;
            if (!Page || !Page->TryGetBoolField(TEXT("inspect"), Open) || !Open
                || !Page->TryGetNumberField(TEXT("inspectImageBytes"), Bytes) || Bytes < 100)
            { D = TEXT("waiting for an imported weapon's 3D inspect frame"); return false; }
            D = FString::Printf(TEXT("native 3D frame received (%d data URL bytes)"), int32(Bytes));
            return true;
        }, 12.f);
    Pre([this, PageOrWhy](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            bool Open = true;
            Page->TryGetBoolField(TEXT("inspect"), Open);
            if (Open) { Why = TEXT("inspect is already open"); return false; }
            const FString Sel = PageString(Page, TEXT("sel"));
            if (!Walker->GetInventory()->FindItemById(Sel)) { Why = FString::Printf(TEXT("page selection '%s' is not a host weapon"), *Sel); return false; }
            return true;
        });
    Add(TEXT("inspect_rotate_weapon"), false, [this] { Hud->SendPageInspectDrag(); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            double Yaw = 0, Frames = 0;
            if (!Page || !Page->TryGetNumberField(TEXT("inspectYaw"), Yaw) || FMath::Abs(Yaw) < 20
                || !Page->TryGetNumberField(TEXT("inspectFrames"), Frames) || Frames < 2)
            { D = TEXT("waiting for rotated native frame after DOM pointer drag"); return false; }
            D = FString::Printf(TEXT("pointer drag rotated %.1f degrees; %d frames received"), Yaw, int32(Frames));
            return true;
        }, 12.f);
    Pre([this, PageOrWhy](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            bool Open = false;
            if (!Page) return false;
            Page->TryGetBoolField(TEXT("inspect"), Open);
            if (!Open) { Why = TEXT("inspect is not open"); return false; }
            return true;
        });
    Add(TEXT("inspect_escape_returns_inventory"), false, [this] { PressKey(TEXT("Escape")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            bool Open = true;
            if (!Page || !Page->TryGetBoolField(TEXT("inspect"), Open) || Open || !Hud->IsInventoryOpen())
            { D = TEXT("inspect did not close back to inventory"); return false; }
            D = TEXT("Escape closed inspect and kept inventory open"); return true;
        });
    Pre([this, PageOrWhy](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            bool Open = false;
            if (!Page) return false;
            Page->TryGetBoolField(TEXT("inspect"), Open);
            if (!Open) { Why = TEXT("inspect is not open"); return false; }
            return true;
        });

    Add(TEXT("equipped_enter_starts_transfer"), false,
        [this] { PressKey(TEXT("Enter")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            if (!Page) return false;
            const FString Source = PageString(Page, TEXT("transfer")), Compare = PageString(Page, TEXT("compare")), Selected = PageString(Page, TEXT("sel"));
            if (Source.IsEmpty() || Source != HostSlotId(0) || Compare != Source || Selected == Source
                || !Walker->GetInventory()->FindItemById(Selected) || HostEquipped(Selected))
            { D = TEXT("equipped Enter did not open a backpack transfer comparison"); return false; }
            TransferCandidateId = Selected;
            D = FString::Printf(TEXT("equipped Enter selected backpack candidate %s and preserved equipped comparison"), *Selected); return true;
        });
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (HostSlotId(0).IsEmpty()) { Why = TEXT("host weapon slot 1 is empty"); return false; }
            if (PageString(Page, TEXT("sel")) != HostSlotId(0)) return Needed(Why, TEXT("page selection"), PageString(Page, TEXT("sel")), HostSlotId(0));
            if (PageHasString(Page, TEXT("transfer"))) { Why = TEXT("a transfer is already active"); return false; }
            return true;
        });
    Add(TEXT("transfer_selection_preserves_full_size_cards"), false,
        [this] { PressKey(TEXT("ArrowDown")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            FString Source, Compare;
            const TSharedPtr<FJsonObject>* Main = nullptr;
            const TSharedPtr<FJsonObject>* Other = nullptr;
            double MainMin = 0, MainMax = 0, OtherMin = 0, OtherMax = 0;
            bool ValuesVisible = false;
            bool RowsAligned = false;
            if (!Page || !Page->TryGetStringField(TEXT("transfer"), Source)
                || !Page->TryGetStringField(TEXT("compare"), Compare) || Source != HostSlotId(0) || Compare != Source
                || !Page->TryGetObjectField(TEXT("mainCardBounds"), Main) || !Page->TryGetObjectField(TEXT("compareCardBounds"), Other)
                || !(*Main)->TryGetNumberField(TEXT("xMin"), MainMin) || !(*Main)->TryGetNumberField(TEXT("xMax"), MainMax)
                || !(*Other)->TryGetNumberField(TEXT("xMin"), OtherMin) || !(*Other)->TryGetNumberField(TEXT("xMax"), OtherMax)
                || MainMax-MainMin < 230 || OtherMax-OtherMin < 230
                || !Page->TryGetBoolField(TEXT("compareStatsVisible"), ValuesVisible) || !ValuesVisible
                || !Page->TryGetBoolField(TEXT("backpackRowsAligned"), RowsAligned) || !RowsAligned)
            { D = TEXT("selection lost the source comparison or shrank its cards"); return false; }
            if (PageString(Page, TEXT("sel")) == TransferCandidateId)
            { D = TEXT("ArrowDown did not move the candidate selection"); return false; }
            D = FString::Printf(TEXT("candidate moved off %s; source preserved; card widths %.1f / %.1f"), *TransferCandidateId, MainMax-MainMin, OtherMax-OtherMin); return true;
        });
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageString(Page, TEXT("transfer")) != HostSlotId(0) || HostSlotId(0).IsEmpty()) return Needed(Why, TEXT("transfer source"), PageString(Page, TEXT("transfer")), HostSlotId(0));
            if (TransferCandidateId.IsEmpty() || PageString(Page, TEXT("sel")) != TransferCandidateId) return Needed(Why, TEXT("page selection"), PageString(Page, TEXT("sel")), TransferCandidateId);
            return true;
        });
    Add(TEXT("transfer_blocks_drop_and_sort"), false,
        [this] { CountBefore = Walker->GetInventory()->BackpackCount(); PressKey(TEXT("q")); PressKey(TEXT("PageDown")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            FString Source;
            double Sort = -1;
            if (!Page || !Page->TryGetStringField(TEXT("transfer"), Source) || Source != HostSlotId(0)
                || !Page->TryGetNumberField(TEXT("sort"), Sort) || Sort != 0
                || Walker->GetInventory()->BackpackCount() != CountBefore
                || Hud->LastInventoryAction().Serial != ActionSerialAtBegin)
            { D = TEXT("swap mode accepted Drop or Sort"); return false; }
            D = TEXT("swap remained active; Drop sent no request and Sort changed neither items nor sort mode"); return true;
        });
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageString(Page, TEXT("transfer")) != HostSlotId(0) || HostSlotId(0).IsEmpty()) return Needed(Why, TEXT("transfer source"), PageString(Page, TEXT("transfer")), HostSlotId(0));
            if (PageNumber(Page, TEXT("sort")) != 0) { Why = TEXT("sort mode is not 0"); return false; }
            return true;
        });
    Add(TEXT("transfer_escape_returns_equipped"), false,
        [this] { PressKey(TEXT("Escape")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            if (!Page || PageString(Page, TEXT("sel")) != HostSlotId(0) || HostSlotId(0).IsEmpty()
                || PageHasString(Page, TEXT("transfer")) || PageHasString(Page, TEXT("compare")) || !Hud->IsInventoryOpen())
            { D = TEXT("transfer Escape did not restore equipped selection and clear the transfer"); return false; }
            D = TEXT("Escape cancelled transfer and restored equipped selection without closing inventory"); return true;
        });
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageString(Page, TEXT("transfer")) != HostSlotId(0) || HostSlotId(0).IsEmpty()) return Needed(Why, TEXT("transfer source"), PageString(Page, TEXT("transfer")), HostSlotId(0));
            return true;
        });
    Add(TEXT("select_backpack_weapon"), false,
        [this]
        {
            PrevSel = PageString(PageObject(), TEXT("sel"));
            // Host navigation: Right walks the equipment cells, then enters
            // Backpack; further Right presses there are no-ops. Eight covers
            // every cell. Stock traversal is UNVERIFIED.
            for (int32 i = 0; i < 8; ++i) PressKey(TEXT("ArrowRight"));
        },
        [this, Snapshot](FString& D)
        {
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            const FString Sel = PageString(Page, TEXT("sel"));
            if (Sel.IsEmpty() || Sel == PrevSel) { D = FString::Printf(TEXT("selection did not move (sel='%s', before='%s'); key did not reach the page"), *Sel, *PrevSel); return false; }
            if (!Walker->GetInventory()->FindItemById(Sel) || HostEquipped(Sel)) { D = FString::Printf(TEXT("selected '%s' is not an unequipped host weapon"), *Sel); return false; }
            SelId = Sel;
            D = FString::Printf(TEXT("page selected backpack weapon %s"), *Sel);
            return true;
        });
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageHasString(Page, TEXT("transfer"))) { Why = TEXT("a transfer is still active"); return false; }
            if (HostSlotId(0).IsEmpty() || PageString(Page, TEXT("sel")) != HostSlotId(0)) return Needed(Why, TEXT("page selection"), PageString(Page, TEXT("sel")), HostSlotId(0));
            if (PageNumber(Page, TEXT("cat")) != 0 || PageNumber(Page, TEXT("sort")) != 0) { Why = TEXT("backpack category/sort is not the default"); return false; }
            return true;
        });

    for (const int32 Delta : {1, -1})
    {
        Add(Delta == 1 ? TEXT("vm_backpack_down") : TEXT("vm_backpack_up"), false,
            [this, Delta]
            {
                VmExpectedId.Reset(); VmCallsBefore = -1;
                const auto Page = PageObject();
                const TArray<TSharedPtr<FJsonValue>>* Rows = nullptr;
                const TSharedPtr<FJsonObject>* Vm = nullptr;
                FString Selected;
                if (Page && Page->TryGetArrayField(TEXT("backpack"), Rows)
                    && Page->TryGetStringField(TEXT("sel"), Selected) && Page->TryGetObjectField(TEXT("vm"), Vm))
                {
                    VmCallsBefore = int32((*Vm)->GetNumberField(TEXT("calls")));
                    for (int32 I = 0; I < Rows->Num(); ++I)
                        if ((*Rows)[I]->AsString() == Selected)
                            VmExpectedId = (*Rows)[FMath::Clamp(I + Delta, 0, Rows->Num() - 1)]->AsString();
                }
                PressKey(Delta == 1 ? TEXT("ArrowDown") : TEXT("ArrowUp"));
            },
            [this, Snapshot, Delta](FString& D)
            {
                const auto Page = Snapshot(D);
                const TSharedPtr<FJsonObject>* Vm = nullptr;
                FString Selected;
                bool Enabled = false;
                double Calls = 0, Errors = -1, Expressions = 0;
                if (!Page || !Page->TryGetObjectField(TEXT("vm"), Vm) || !Page->TryGetStringField(TEXT("sel"), Selected)
                    || !(*Vm)->TryGetBoolField(TEXT("enabled"), Enabled) || !Enabled
                    || !(*Vm)->TryGetNumberField(TEXT("calls"), Calls) || Calls <= VmCallsBefore
                    || !(*Vm)->TryGetNumberField(TEXT("errors"), Errors) || Errors != 0
                    || !(*Vm)->TryGetNumberField(TEXT("steps"), Expressions) || Expressions <= 0
                    || VmExpectedId.IsEmpty() || Selected != VmExpectedId)
                { D = TEXT("original MoveDelta did not produce the expected page selection without diagnostics"); return false; }
                if (Delta < 0 && Selected != SelId)
                { D = FString::Printf(TEXT("Up did not return to %s (selected %s)"), *SelId, *Selected); return false; }
                D = FString::Printf(TEXT("MoveDelta calls=%.0f expressions=%.0f selected=%s"), Calls, Expressions, *Selected);
                return true;
            });
        Pre([this, PageOrWhy, Needed, Delta](FString& Why)
            {
                auto Page = PageOrWhy(Why);
                if (!Page) return false;
                const TSharedPtr<FJsonObject>* Vm = nullptr;
                bool Enabled = false;
                if (!Page->TryGetObjectField(TEXT("vm"), Vm) || !(*Vm)->TryGetBoolField(TEXT("enabled"), Enabled) || !Enabled)
                { Why = TEXT("the inventory VM is not enabled on the page"); return false; }
                if (PageHasString(Page, TEXT("transfer"))) { Why = TEXT("a transfer is active"); return false; }
                const FString Sel = PageString(Page, TEXT("sel"));
                // Down starts from the weapon chosen by select_backpack_weapon; Up from the row Down moved to.
                if (Delta > 0 && Sel != SelId) return Needed(Why, TEXT("page selection"), Sel, SelId);
                if (Delta < 0 && (Sel.IsEmpty() || Sel == SelId)) { Why = TEXT("the previous Down step did not move the selection"); return false; }
                const TArray<TSharedPtr<FJsonValue>>* Rows = nullptr;
                if (!Page->TryGetArrayField(TEXT("backpack"), Rows)) { Why = TEXT("page lists no backpack rows"); return false; }
                int32 Row = INDEX_NONE;
                for (int32 I = 0; I < Rows->Num(); ++I) if ((*Rows)[I]->AsString() == Sel) Row = I;
                if (Row == INDEX_NONE || (Delta > 0 && Row >= Rows->Num() - 1) || (Delta < 0 && Row <= 0))
                { Why = TEXT("the selected row cannot move in that direction"); return false; }
                return true;
            });
    }

    Add(TEXT("pageup_sorts_preserving_selection"), false,
        [this] { PressKey(TEXT("PageUp")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            double Sort = -1;
            bool RowsAligned = false;
            if (!Page || !Page->TryGetNumberField(TEXT("sort"), Sort) || Sort != 1
                || PageString(Page, TEXT("sel")) != SelId
                || !Page->TryGetBoolField(TEXT("backpackRowsAligned"), RowsAligned) || !RowsAligned)
            { D = TEXT("PageUp failed to advance the host sort mode while retaining the selected instance"); return false; }
            D = TEXT("host: PageUp advanced to host sort mode 1 and kept the selected stable ID"); return true;
        });
    Diverges(TEXT("stock PageDown is forward (ALL>TYPES>BRANDS>ITEMS>VALUE, PageUp reverses) and every step selects the first cell; the host's PageUp is forward over its own DEFAULT/NAME/RARITY/LEVEL/DAMAGE modes and keeps the selection. The stock sort list is a separate unfinished feature"));
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageString(Page, TEXT("sel")) != SelId || SelId.IsEmpty()) return Needed(Why, TEXT("page selection"), PageString(Page, TEXT("sel")), SelId);
            if (PageNumber(Page, TEXT("sort")) != 0 || PageHasString(Page, TEXT("transfer"))) { Why = TEXT("sort is not 0 or a transfer is active"); return false; }
            return true;
        });
    Add(TEXT("pagedown_reverses_sort"), false,
        [this] { PressKey(TEXT("PageDown")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            double Sort = -1;
            if (!Page || !Page->TryGetNumberField(TEXT("sort"), Sort) || Sort != 0
                || PageString(Page, TEXT("sel")) != SelId)
            { D = TEXT("PageDown failed to restore the host sort mode while retaining the selected instance"); return false; }
            D = TEXT("host: PageDown stepped back to host sort mode 0 and kept the selected stable ID"); return true;
        });
    Diverges(TEXT("stock PageDown advances the sort (ALL>TYPES>BRANDS>ITEMS>VALUE) and selects the first cell; the host's PageDown steps back through its own modes and keeps the selection. The stock sort list is a separate unfinished feature"));
    Pre([this, PageOrWhy](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageNumber(Page, TEXT("sort")) != 1) { Why = TEXT("sort mode is not 1, so there is nothing to step back from"); return false; }
            return true;
        });
    Add(TEXT("backpack_select_starts_transfer_without_equipping"), false,
        [this] { PressKey(TEXT("e")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            bool FromEquipped = true;
            if (!Page || PageString(Page, TEXT("transfer")) != SelId || PageString(Page, TEXT("sel")) != SelId
                || !Page->TryGetBoolField(TEXT("transferFromEquipped"), FromEquipped) || FromEquipped
                || HostEquipped(SelId))
            { D = TEXT("backpack E failed to pin the source or equipped it prematurely"); return false; }
            D = TEXT("backpack E pinned source; host equipment unchanged"); return true;
        });
    Pre([this, PageOrWhy, Needed, InBackpack](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageString(Page, TEXT("sel")) != SelId || SelId.IsEmpty()) return Needed(Why, TEXT("page selection"), PageString(Page, TEXT("sel")), SelId);
            if (PageNumber(Page, TEXT("sort")) != 0 || PageHasString(Page, TEXT("transfer"))) { Why = TEXT("sort is not 0 or a transfer is active"); return false; }
            if (!InBackpack(SelId)) { Why = TEXT("the selected weapon is not an unequipped host weapon"); return false; }
            return true;
        });
    Add(TEXT("backpack_transfer_changes_destination"), false,
        [this] { PressKey(TEXT("2")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            double Target = -1;
            if (!Page || PageString(Page, TEXT("transfer")) != SelId || PageString(Page, TEXT("sel")) != SelId
                || HostSlotId(1).IsEmpty() || PageString(Page, TEXT("compare")) != HostSlotId(1)
                || !Page->TryGetNumberField(TEXT("target"), Target) || Target != 1 || HostEquipped(SelId))
            { D = TEXT("destination change lost source or comparison"); return false; }
            D = TEXT("slot 2 chosen; source fixed and destination weapon compared"); return true;
        });
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageString(Page, TEXT("transfer")) != SelId || SelId.IsEmpty()) return Needed(Why, TEXT("transfer source"), PageString(Page, TEXT("transfer")), SelId);
            if (PageNumber(Page, TEXT("target")) == 1) { Why = TEXT("destination is already slot 2, so key 2 would prove nothing"); return false; }
            if (HostSlotId(1).IsEmpty()) { Why = TEXT("host weapon slot 2 is empty, so there is no destination weapon to compare"); return false; }
            return true;
        });
    Add(TEXT("backpack_transfer_cancel_keeps_source"), false,
        [this] { PressKey(TEXT("Escape")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            if (!Page || PageString(Page, TEXT("sel")) != SelId
                || HostEquipped(SelId) || !Hud->IsInventoryOpen()
                || Page->HasTypedField<EJson::String>(TEXT("transfer")))
            { D = TEXT("cancel lost source, changed equipment or closed inventory"); return false; }
            D = TEXT("Escape cancelled swap, retained backpack source and menu"); return true;
        });
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageString(Page, TEXT("transfer")) != SelId || SelId.IsEmpty()) return Needed(Why, TEXT("transfer source"), PageString(Page, TEXT("transfer")), SelId);
            return true;
        });
    Add(TEXT("equip_weapon_slot2"), true,
        [this] { DisplacedId = HostSlotId(1); PressKey(TEXT("2")); PressKey(TEXT("Enter")); PressKey(TEXT("Enter")); },
        [this, CheckAction, Snapshot](FString& D)
        {
            if (!CheckAction(TEXT("equip"), true, SelId, D)) return false;
            if (HostSlotId(1) != SelId) { D = FString::Printf(TEXT("host slot 2 holds '%s', wanted %s"), *HostSlotId(1), *SelId); return false; }
            if (DisplacedId.IsEmpty() || !Walker->GetInventory()->FindItemById(DisplacedId) || HostEquipped(DisplacedId))
            { D = FString::Printf(TEXT("displaced weapon '%s' did not return to the backpack"), *DisplacedId); return false; }
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            if (PageSlot(Page, 1) != SelId) { D = FString::Printf(TEXT("page slot 2 shows '%s' (snapshot not refreshed)"), *PageSlot(Page, 1)); return false; }
            D = FString::Printf(TEXT("%s in slot 2, displaced '%s' back in backpack, page refreshed"), *SelId, *DisplacedId);
            return true;
        });
    Pre([this, PageOrWhy, Needed, InBackpack](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageString(Page, TEXT("sel")) != SelId || SelId.IsEmpty()) return Needed(Why, TEXT("page selection"), PageString(Page, TEXT("sel")), SelId);
            if (PageHasString(Page, TEXT("transfer"))) { Why = TEXT("a transfer is active"); return false; }
            if (!InBackpack(SelId)) { Why = TEXT("the selected weapon is not an unequipped host weapon"); return false; }
            if (HostSlotId(1).IsEmpty()) { Why = TEXT("host slot 2 is empty, so no weapon would be displaced"); return false; }
            return true;
        });

    Add(TEXT("unequip_weapon_slot2"), true,
        [this] { CountBefore = Walker->GetInventory()->BackpackCount(); PressKey(TEXT("Delete")); },
        [this, CheckAction, Snapshot](FString& D)
        {
            if (!CheckAction(TEXT("unequip"), true, FString(), D)) return false;
            if (!HostSlotId(1).IsEmpty()) { D = TEXT("host slot 2 still holds a weapon"); return false; }
            if (!Walker->GetInventory()->FindItemById(SelId)) { D = TEXT("unequipped weapon vanished from the inventory"); return false; }
            if (Walker->GetInventory()->BackpackCount() != CountBefore + 1)
            { D = FString::Printf(TEXT("backpack count %d, wanted %d"), Walker->GetInventory()->BackpackCount(), CountBefore + 1); return false; }
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            if (!PageSlot(Page, 1).IsEmpty()) { D = TEXT("page slot 2 still shows a weapon (snapshot not refreshed)"); return false; }
            D = FString::Printf(TEXT("slot 2 empty, %s back in backpack (%d)"), *SelId, CountBefore + 1);
            return true;
        });
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (HostSlotId(1) != SelId || SelId.IsEmpty()) return Needed(Why, TEXT("host slot 2"), HostSlotId(1), SelId);
            if (PageNumber(Page, TEXT("target")) != 1 || PageHasString(Page, TEXT("gear"))) { Why = TEXT("page target is not weapon slot 2"); return false; }
            return true;
        });

    Add(TEXT("backpack_transfer_empty_destination"), false,
        // After the unequip the page keeps the now-empty equipment cell selected (as the
        // original does for an empty cell), so Right re-enters Backpack on its remembered row.
        [this] { PressKey(TEXT("ArrowRight")); PressKey(TEXT("e")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            double Target = -1;
            if (!Page || PageString(Page, TEXT("transfer")) != SelId
                || !Page->TryGetNumberField(TEXT("target"), Target) || Target != 1
                || Page->HasTypedField<EJson::String>(TEXT("compare")) || !HostSlotId(1).IsEmpty()
                || HostEquipped(SelId))
            { D = TEXT("empty destination did not retain backpack source with no comparison"); return false; }
            D = TEXT("empty slot 2 remains a valid pending destination; no premature equip"); return true;
        });
    Pre([this, PageOrWhy, InBackpack](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (!HostSlotId(1).IsEmpty()) { Why = TEXT("host slot 2 is not empty"); return false; }
            if (!InBackpack(SelId)) { Why = TEXT("the selected weapon is not an unequipped host weapon"); return false; }
            if (PageHasString(Page, TEXT("transfer"))) { Why = TEXT("a transfer is already active"); return false; }
            return true;
        });
    Add(TEXT("backpack_empty_transfer_cancel"), false,
        [this] { PressKey(TEXT("Escape")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            if (!Page || Page->HasTypedField<EJson::String>(TEXT("transfer")) || HostEquipped(SelId)
                || !HostSlotId(1).IsEmpty() || !Hud->IsInventoryOpen())
            { D = TEXT("empty-slot cancel changed equipment or closed inventory"); return false; }
            D = TEXT("empty-slot transfer cancelled without changing equipment"); return true;
        });
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageString(Page, TEXT("transfer")) != SelId || SelId.IsEmpty()) return Needed(Why, TEXT("transfer source"), PageString(Page, TEXT("transfer")), SelId);
            return true;
        });
    Add(TEXT("equip_weapon_slot1_active"), true,
        [this] { DisplacedId = HostSlotId(0); PressKey(TEXT("1")); PressKey(TEXT("Enter")); PressKey(TEXT("Enter")); },
        [this, CheckAction, Snapshot](FString& D)
        {
            if (!CheckAction(TEXT("equip"), true, SelId, D)) return false;
            const UOpenWillowInventory* Inv = Walker->GetInventory();
            if (HostSlotId(0) != SelId) { D = FString::Printf(TEXT("host slot 1 holds '%s'"), *HostSlotId(0)); return false; }
            const FOpenWillowWeaponItem* Active = Inv->ActiveWeapon();
            if (!Walker->HasWeaponOut() || !Active || UOpenWillowInventory::StableId(*Active) != SelId)
            { D = TEXT("the newly equipped weapon is not the one held in hand"); return false; }
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            if (PageSlot(Page, 0) != SelId) { D = TEXT("page slot 1 not refreshed"); return false; }
            D = FString::Printf(TEXT("%s replaced '%s' in the held slot and is drawn"), *SelId, *DisplacedId);
            return true;
        });
    Pre([this, PageOrWhy, Needed, InBackpack](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageString(Page, TEXT("sel")) != SelId || SelId.IsEmpty()) return Needed(Why, TEXT("page selection"), PageString(Page, TEXT("sel")), SelId);
            if (PageHasString(Page, TEXT("transfer"))) { Why = TEXT("a transfer is active"); return false; }
            if (!InBackpack(SelId)) { Why = TEXT("the selected weapon is not an unequipped host weapon"); return false; }
            if (HostSlotId(0).IsEmpty()) { Why = TEXT("host slot 1 is empty, so no weapon would be displaced"); return false; }
            return true;
        });

    Add(TEXT("unequip_active_weapon"), true,
        [this] { PressKey(TEXT("Delete")); },
        [this, CheckAction, Snapshot](FString& D)
        {
            if (!CheckAction(TEXT("unequip"), true, FString(), D)) return false;
            if (!HostSlotId(0).IsEmpty()) { D = TEXT("host slot 1 still holds a weapon"); return false; }
            if (Walker->HasWeaponOut() || Walker->GetInventory()->ActiveWeapon()) { D = TEXT("weapon still held after unequipping the active slot"); return false; }
            if (!Walker->GetInventory()->FindItemById(SelId) || HostEquipped(SelId)) { D = TEXT("the unequipped weapon is not back in the backpack"); return false; }
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            if (!PageSlot(Page, 0).IsEmpty()) { D = TEXT("page slot 1 not refreshed"); return false; }
            D = TEXT("active weapon holstered and returned to backpack");
            return true;
        });
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (HostSlotId(0) != SelId || SelId.IsEmpty()) return Needed(Why, TEXT("host slot 1"), HostSlotId(0), SelId);
            if (PageNumber(Page, TEXT("target")) != 0 || PageHasString(Page, TEXT("gear"))) { Why = TEXT("page target is not weapon slot 1"); return false; }
            return true;
        });

    Add(TEXT("drag_weapon_to_slot2"), true,
        [this] { Hud->SendPageDrag(SelId, 1); },
        [this, CheckAction, Snapshot](FString& D)
        {
            if (!CheckAction(TEXT("equip"), true, SelId, D)) return false;
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page || HostSlotId(1) != SelId || PageSlot(Page, 1) != SelId)
            { D = TEXT("drag did not equip the weapon on host and page"); return false; }
            D = TEXT("DOM drag/drop equipped the weapon in slot 2 on host and page");
            return true;
        });
    Pre([this, PageOrWhy, InBackpack](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (!InBackpack(SelId)) { Why = TEXT("the dragged weapon is not an unequipped host weapon"); return false; }
            if (!HostSlotId(1).IsEmpty()) { Why = TEXT("host slot 2 is not empty"); return false; }
            if (!PageHasItem(Page, SelId)) { Why = TEXT("the page does not list the dragged weapon"); return false; }
            return true;
        });
    Add(TEXT("drag_weapon_back_to_backpack"), true,
        [this] { Hud->SendPageDrag(SelId, -1); },
        [this, CheckAction, Snapshot](FString& D)
        {
            if (!CheckAction(TEXT("unequip"), true, FString(), D)) return false;
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page || !HostSlotId(1).IsEmpty() || !PageSlot(Page, 1).IsEmpty()
                || !Walker->GetInventory()->FindItemById(SelId) || HostEquipped(SelId))
            { D = TEXT("drag did not return the equipped item to the backpack"); return false; }
            bDragControlOk = true; // DOM drag demonstrably reaches the host: control for the refused drag below
            D = TEXT("DOM drag/drop returned the weapon to the backpack without losing it");
            return true;
        });
    Pre([this](FString& Why)
        {
            if (HostSlotId(1) != SelId || SelId.IsEmpty()) { Why = FString::Printf(TEXT("host slot 2 is '%s', needed '%s'"), *HostSlotId(1), *SelId); return false; }
            return true;
        }, false);
    Add(TEXT("drag_weapon_to_gear_refused"), false,
        [this] { Hud->SendPageDrag(SelId, 4); },
        [this](FString& D)
        {
            if (Hud->LastInventoryAction().Serial != ActionSerialAtBegin || HostEquipped(SelId))
            { D = TEXT("invalid weapon-to-shield drag sent a request or changed inventory"); return false; }
            // A negative check: the page cannot confirm the drag target existed, so the positive control is the
            // two preceding drag steps (same dispatch path). UNVERIFIED that shield cell 4 itself was hit.
            D = TEXT("weapon-to-shield drag sent no request and equipped nothing (negative check; dispatch proven by the preceding drag steps)");
            return true;
        });
    Pre([this, InBackpack](FString& Why)
        {
            if (!bDragControlOk) { Why = TEXT("the positive drag control (previous two steps) did not pass"); return false; }
            if (!InBackpack(SelId)) { Why = TEXT("the dragged weapon is not an unequipped host weapon"); return false; }
            return true;
        }, false);

    // The drag steps leave the page selection somewhere else, so select the working weapon explicitly
    // with arrow keys before any step that acts on "the selected item".
    Add(TEXT("select_marking_target"), false,
        [this] { WalkLastSel.Reset(); WalkExpected.Reset(); WalkLastPressAt = -100.f; },
        [this, WalkTo](FString& D) { return WalkTo(SelId, D); }, 40.f, true, 0.2f);
    Pre([this, PageOrWhy, InBackpack](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (!InBackpack(SelId)) { Why = TEXT("the working weapon is not an unequipped host weapon"); return false; }
            if (PageHasString(Page, TEXT("transfer"))) { Why = TEXT("a transfer is active"); return false; }
            bool Open = false;
            Page->TryGetBoolField(TEXT("inspect"), Open);
            if (Open) { Why = TEXT("inspect is open"); return false; }
            return true;
        });

    // Favorite / trash toggles, checked on the host and on the page's snapshot.
    auto MarkVerify = [this, CheckAction, Snapshot](const TCHAR* Action, int32 WantFavorite, int32 WantTrash, FString& D)
    {
        if (!CheckAction(Action, true, SelId, D)) return false;
        const FOpenWillowWeaponItem* Item = Walker->GetInventory()->FindItemById(SelId);
        if (!Item) { D = TEXT("item missing"); return false; }
        if (int32(Item->bFavorite) != WantFavorite || int32(Item->bTrash) != WantTrash)
        { D = FString::Printf(TEXT("host fav=%d trash=%d, wanted fav=%d trash=%d"), Item->bFavorite, Item->bTrash, WantFavorite, WantTrash); return false; }
        TSharedPtr<FJsonObject> Page = Snapshot(D);
        if (!Page) return false;
        int32 Favorite = -1, Trash = -1;
        if (!PageItemFlags(Page, SelId, Favorite, Trash) || Favorite != WantFavorite || Trash != WantTrash)
        { D = FString::Printf(TEXT("page fav=%d trash=%d (snapshot not refreshed), wanted fav=%d trash=%d"), Favorite, Trash, WantFavorite, WantTrash); return false; }
        D = FString::Printf(TEXT("%s ok: fav=%d trash=%d on host and page"), Action, WantFavorite, WantTrash);
        return true;
    };
    // Marking acts on the page's selected item, so the page must select the working weapon and the host
    // item must be in the state this step starts from.
    auto MarkPre = [this, PageOrWhy, Needed](int32 HaveFavorite, int32 HaveTrash, FString& Why)
    {
        auto Page = PageOrWhy(Why);
        if (!Page) return false;
        if (SelId.IsEmpty() || PageString(Page, TEXT("sel")) != SelId) return Needed(Why, TEXT("page selection"), PageString(Page, TEXT("sel")), SelId);
        const FOpenWillowWeaponItem* Item = Walker->GetInventory()->FindItemById(SelId);
        if (!Item) { Why = TEXT("working weapon missing from the host inventory"); return false; }
        if (int32(Item->bFavorite) != HaveFavorite || int32(Item->bTrash) != HaveTrash)
        { Why = FString::Printf(TEXT("host fav=%d trash=%d, needed fav=%d trash=%d"), Item->bFavorite, Item->bTrash, HaveFavorite, HaveTrash); return false; }
        return true;
    };
    Add(TEXT("favorite_on"), true, [this] { PressKey(TEXT("v")); },
        [MarkVerify](FString& D) { return MarkVerify(TEXT("favorite"), 1, 0, D); });
    Pre([MarkPre](FString& Why) { return MarkPre(0, 0, Why); });
    Add(TEXT("trash_on_clears_favorite"), true, [this] { PressKey(TEXT("t")); },
        [MarkVerify](FString& D) { return MarkVerify(TEXT("trash"), 0, 1, D); });
    Pre([MarkPre](FString& Why) { return MarkPre(1, 0, Why); });
    Add(TEXT("trash_off"), true, [this] { PressKey(TEXT("t")); },
        [MarkVerify](FString& D) { return MarkVerify(TEXT("trash"), 0, 0, D); });
    Pre([MarkPre](FString& Why) { return MarkPre(0, 1, Why); });

    Add(TEXT("drop_weapon"), true,
        [this]
        {
            // Precondition guarantees the page selects SelId and the host has it; DropId is never left stale.
            CountBefore = Walker->GetInventory()->BackpackCount();
            DropId = SelId;
            const FOpenWillowWeaponItem* Item = Walker->GetInventory()->FindItemById(DropId);
            DropName = Item ? Item->Name : FString();
            PressKey(TEXT("q"));
        },
        [this, CheckAction, Snapshot](FString& D)
        {
            if (DropId.IsEmpty() || DropName.IsEmpty()) { D = TEXT("no drop target was recorded"); return false; }
            if (!CheckAction(TEXT("drop"), true, DropId, D)) return false;
            if (Walker->GetInventory()->FindItemById(DropId)) { D = TEXT("dropped weapon is still in the host inventory"); return false; }
            if (Walker->GetInventory()->BackpackCount() != CountBefore - 1)
            { D = FString::Printf(TEXT("backpack count %d, wanted %d"), Walker->GetInventory()->BackpackCount(), CountBefore - 1); return false; }
            FString PickupName;
            if (!FindPickup(&PickupName)) { D = TEXT("no pickup actor spawned within 300 cm of Maya"); return false; }
            if (PickupName != DropName) { D = FString::Printf(TEXT("pickup shows '%s', wanted '%s'"), *PickupName, *DropName); return false; }
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            if (PageHasItem(Page, DropId)) { D = TEXT("page snapshot still lists the dropped weapon"); return false; }
            D = FString::Printf(TEXT("%s (%s) left the backpack (%d) and became a pickup"), *DropId, *DropName, CountBefore - 1);
            return true;
        });
    Pre([this, PageOrWhy, Needed](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (SelId.IsEmpty() || PageString(Page, TEXT("sel")) != SelId) return Needed(Why, TEXT("page selection"), PageString(Page, TEXT("sel")), SelId);
            const FOpenWillowWeaponItem* Item = Walker->GetInventory()->FindItemById(SelId);
            if (!Item || Item->Name.IsEmpty()) { Why = TEXT("working weapon missing or unnamed on the host"); return false; }
            if (HostEquipped(SelId)) { Why = TEXT("working weapon is equipped"); return false; }
            if (Item->bFavorite || Item->bTrash) { Why = TEXT("working weapon is still marked favorite/trash"); return false; }
            if (FindPickup()) { Why = TEXT("a pickup is already within 300 cm, so the dropped one could not be told apart"); return false; }
            return true;
        });

    auto CloseVerify = [this](FString& D)
    {
        if (Hud->IsInventoryOpen()) { D = TEXT("inventory page is still open"); return false; }
        if (PC->bShowMouseCursor) { D = TEXT("mouse cursor still shown after closing"); return false; }
        D = TEXT("page closed itself through its close route and game input returned");
        return true;
    };
    auto InventoryOpenPre = [this](FString& Why) { if (!Hud->IsInventoryOpen()) { Why = TEXT("inventory page is not open"); return false; } return true; };
    Add(TEXT("close_inventory_escape"), false, [this] { PressKey(TEXT("Escape")); }, CloseVerify, 6.f, false);
    Pre(InventoryOpenPre, false);

    // The dropped weapon must really be on the ground and out of the inventory for the next two steps.
    auto DroppedPre = [this](FString& Why)
    {
        if (DropId.IsEmpty()) { Why = TEXT("no weapon was dropped (drop_weapon did not pass)"); return false; }
        if (Walker->GetInventory()->FindItemById(DropId)) { Why = FString::Printf(TEXT("%s is still in the inventory"), *DropId); return false; }
        if (!FindPickup()) { Why = TEXT("no pickup actor within 300 cm of Maya"); return false; }
        if (Hud->IsInventoryOpen()) { Why = TEXT("inventory page is open"); return false; }
        return true;
    };
    Add(TEXT("pickup_refused_when_backpack_full"), false,
        [this]
        {
            UOpenWillowInventory* Inv = Walker->GetInventory();
            PickupAttemptsBefore = Walker->PickupAttemptCount();
            FillerIds.Reset();
            while (Inv->CanAddToBackpack() && Inv->Items().Num())
            {
                FOpenWillowWeaponItem Copy = Inv->Items()[0];
                Copy.InstanceId.Reset();
                if (!Inv->AddToBackpack(Copy)) break;
                FillerIds.Add(UOpenWillowInventory::StableId(Inv->Items().Last()));
            }
            PressGameKey(EKeys::E);
        },
        [this](FString& D)
        {
            const UOpenWillowInventory* Inv = Walker->GetInventory();
            if (Inv->CanAddToBackpack()) { D = TEXT("test could not fill the backpack"); return false; }
            if (Walker->PickupAttemptCount() <= PickupAttemptsBefore) { D = TEXT("E press never reached the pickup action"); return false; }
            if (Walker->LastPickupAccepted()) { D = TEXT("pickup accepted into a full backpack"); return false; }
            if (Inv->FindItemById(DropId)) { D = TEXT("dropped item entered a full backpack"); return false; }
            if (!FindPickup()) { D = TEXT("pickup actor vanished though it was refused"); return false; }
            D = FString::Printf(TEXT("backpack full (%d/%d), E refused, pickup stays in the world"), Inv->BackpackCount(), Inv->GetBackpackCapacity());
            return true;
        }, 3.f, false, 1.f);
    Pre(DroppedPre, false);

    Add(TEXT("pickup_returns_item_to_backpack"), false,
        [this]
        {
            UOpenWillowInventory* Inv = Walker->GetInventory();
            for (const FString& Id : FillerIds)
            {
                FOpenWillowTakenInventoryItem Discard;
                Inv->TakeById(Id, Discard);
            }
            FillerIds.Reset();
            PickupAttemptsBefore = Walker->PickupAttemptCount();
            PressGameKey(EKeys::E);
        },
        [this](FString& D)
        {
            const UOpenWillowInventory* Inv = Walker->GetInventory();
            if (Walker->PickupAttemptCount() <= PickupAttemptsBefore) { D = TEXT("E press never reached the pickup action"); return false; }
            if (!Walker->LastPickupAccepted()) { D = TEXT("pickup refused"); return false; }
            const FOpenWillowWeaponItem* Item = Inv->FindItemById(DropId);
            if (!Item) { D = TEXT("item did not return with its stable id"); return false; }
            if (Item->bFavorite || Item->bTrash) { D = TEXT("item flags changed across drop and pickup"); return false; }
            if (Inv->BackpackCount() != CountBefore) { D = FString::Printf(TEXT("backpack count %d, wanted %d"), Inv->BackpackCount(), CountBefore); return false; }
            if (FindPickup()) { D = TEXT("pickup actor still in the world"); return false; }
            D = FString::Printf(TEXT("%s back in the backpack (%d), pickup actor removed"), *DropId, CountBefore);
            return true;
        }, 3.f, false, 1.f);
    Pre(DroppedPre, false);

    Add(TEXT("reopen_inventory"), false, [this] { PressGameKey(EKeys::I); },
        [this, OpenVerify, Snapshot](FString& D)
        {
            if (!OpenVerify(D)) return false;
            if (DropId.IsEmpty() || !PageHasItem(Snapshot(D), DropId)) { D = TEXT("reopened page does not list the picked-up weapon"); return false; }
            D += FString::Printf(TEXT("; lists picked-up %s"), *DropId);
            return true;
        }, 20.f);
    Pre([this](FString& Why)
        {
            if (Hud->IsInventoryOpen()) { Why = TEXT("inventory page is already open"); return false; }
            if (DropId.IsEmpty() || !Walker->GetInventory()->FindItemById(DropId)) { Why = TEXT("the picked-up weapon is not back in the host inventory"); return false; }
            return true;
        }, false);

    // slots=2 (-owslots=2): slots 3 and 4 are locked.
    Add(TEXT("locked_slot_ignored_by_page"), false,
        [this] { PressKey(TEXT("2")); PressKey(TEXT("4")); },
        [this, Snapshot](FString& D)
        {
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            const double Target = PageNumber(Page, TEXT("target"));
            if (int32(Target) == 3) { D = TEXT("page targeted locked slot 4"); return false; }
            if (int32(Target) != 1) { D = FString::Printf(TEXT("target slot %d, wanted 2 as the key-delivery control"), int32(Target) + 1); return false; }
            D = TEXT("key 2 moved the target to slot 2 (control), key 4 (locked) was ignored");
            return true;
        });
    Pre([this, PageOrWhy](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (PageNumber(Page, TEXT("target")) == 1) { Why = TEXT("target is already slot 2, so key 2 would not show the keys arrive"); return false; }
            if (Walker->GetInventory()->GetWeaponSlotsUnlocked() != 2) { Why = TEXT("the run does not have exactly two unlocked slots"); return false; }
            return true;
        });

    Add(TEXT("locked_slot_refused_by_host"), true,
        [this]
        {
            // Skip the page's own check: the host must refuse on its own.
            ForgedId = SelId;
            if (!Walker->GetInventory()->FindItemById(ForgedId) && Walker->GetInventory()->Items().Num())
                ForgedId = UOpenWillowInventory::StableId(Walker->GetInventory()->Items()[0]);
            Hud->InjectPageRequest(FString::Printf(TEXT("{\"action\":\"equip\",\"id\":\"%s\",\"slot\":3}"), *ForgedId));
        },
        [this, CheckAction](FString& D)
        {
            if (ForgedId.IsEmpty()) { D = TEXT("no weapon id was forged"); return false; }
            if (!CheckAction(TEXT("equip"), false, ForgedId, D)) return false;
            if (!HostSlotId(3).IsEmpty() || !HostSlotId(2).IsEmpty()) { D = TEXT("a locked slot holds a weapon"); return false; }
            D = TEXT("host refused a forged equip into locked slot 4; slots 3-4 empty");
            return true;
        }, 5.f, false);
    Pre([this](FString& Why)
        {
            if (!Walker->GetInventory()->Items().Num()) { Why = TEXT("the host inventory has no weapon to forge a request with"); return false; }
            if (!HostSlotId(2).IsEmpty() || !HostSlotId(3).IsEmpty()) { Why = TEXT("a locked slot already holds a weapon"); return false; }
            return true;
        }, false);

    // Gear: the synthetic shield above is level 36. It is selected by walking to it in the unfiltered
    // backpack; the host's [ ] category filter is not used (it has no stock counterpart).
    Add(TEXT("select_shield_in_backpack"), false,
        [this] { WalkLastSel.Reset(); WalkExpected.Reset(); WalkLastPressAt = -100.f; },
        [this, WalkTo](FString& D)
        {
            if (!WalkTo(ShieldId, D)) return false;
            const TSharedPtr<FJsonObject> Page = PageObject();
            if (PageString(Page, TEXT("gear")) != TEXT("shield")) { D = TEXT("shield selected but the page's gear slot is not 'shield'"); return false; }
            D = FString::Printf(TEXT("synthetic shield %s selected in the backpack"), *ShieldId);
            return true;
        }, 40.f, true, 0.2f);
    Pre([this, PageOrWhy](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (ShieldId.IsEmpty() || !Walker->GetInventory()->FindGearById(ShieldId)) { Why = TEXT("the synthetic shield fixture is not in the host inventory"); return false; }
            if (!PageHasItem(Page, ShieldId)) { Why = TEXT("the page does not list the synthetic shield"); return false; }
            if (PageHasString(Page, TEXT("transfer"))) { Why = TEXT("a transfer is active"); return false; }
            if (PageNumber(Page, TEXT("cat")) != 0) { Why = TEXT("backpack category filter is not ALL"); return false; }
            return true;
        });

    auto LevelVerify = [this, Snapshot](int32 Level, FString& D)
    {
        TSharedPtr<FJsonObject> Page = Snapshot(D);
        if (!Page) return false;
        const double PageLevel = PageNumber(Page, TEXT("level"));
        if (int32(PageLevel) != Level || Walker->GetSkills()->GetLevel() != Level)
        { D = FString::Printf(TEXT("page level %d / host level %d, wanted %d (snapshot not refreshed yet)"), int32(PageLevel), Walker->GetSkills()->GetLevel(), Level); return false; }
        D = FString::Printf(TEXT("host set level %d and the page snapshot shows it"), Level);
        return true;
    };
    auto ShieldWorn = [this](FString& Worn)
    {
        const FOpenWillowGearItem* Gear = Walker->GetInventory()->GearSlotItem(TEXT("shield"));
        Worn = Gear ? Gear->Id : FString();
    };
    Add(TEXT("set_level_35"), false, [this] { Walker->GetSkills()->SetLevel(35); },
        [LevelVerify](FString& D) { return LevelVerify(35, D); }, 8.f);
    Pre([this, PageOrWhy](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (int32(PageNumber(Page, TEXT("level"))) == 35 || Walker->GetSkills()->GetLevel() == 35) { Why = TEXT("level is already 35, so the change would show nothing"); return false; }
            return true;
        });

    Add(TEXT("gear_level_gate_in_page"), false, [this] { PressKey(TEXT("Enter")); },
        [this, ShieldWorn, Snapshot](FString& D)
        {
            if (Hud->LastInventoryAction().Serial != ActionSerialAtBegin) { D = TEXT("page sent an equip request below the level requirement"); return false; }
            FString Worn;
            ShieldWorn(Worn);
            if (!Worn.IsEmpty()) { D = TEXT("shield equipped below the level requirement"); return false; }
            auto Page = Snapshot(D);
            if (!Page) return false;
            if (PageHasString(Page, TEXT("transfer"))) { D = TEXT("Enter started a transfer for an item above the player's level"); return false; }
            // A negative check: Enter delivery cannot be observed when it is correctly ignored; the key path is
            // the same Slate route every other step uses.
            D = TEXT("level 35 vs shield level 36: Enter sent no request, started no transfer, shield stays off (negative check)");
            return true;
        }, 0.1f, true, 1.5f);
    Pre([this, PageOrWhy, Needed, ShieldWorn](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            const FOpenWillowGearItem* Gear = Walker->GetInventory()->FindGearById(ShieldId);
            if (!Gear || Gear->Level != 36) { Why = TEXT("the synthetic shield is missing or is not level 36"); return false; }
            if (int32(PageNumber(Page, TEXT("level"))) != 35 || Walker->GetSkills()->GetLevel() != 35) { Why = TEXT("player level is not 35"); return false; }
            if (PageString(Page, TEXT("sel")) != ShieldId) return Needed(Why, TEXT("page selection"), PageString(Page, TEXT("sel")), ShieldId);
            FString Worn; ShieldWorn(Worn);
            if (!Worn.IsEmpty()) { Why = TEXT("a shield is already worn"); return false; }
            if (PageHasString(Page, TEXT("transfer"))) { Why = TEXT("a transfer is active"); return false; }
            return true;
        });

    Add(TEXT("gear_level_gate_in_host"), true,
        [this]
        {
            Hud->InjectPageRequest(FString::Printf(TEXT("{\"action\":\"equip\",\"id\":\"%s\",\"gearSlot\":\"shield\"}"), *ShieldId));
        },
        [this, CheckAction, ShieldWorn](FString& D)
        {
            if (ShieldId.IsEmpty()) { D = TEXT("no shield id was forged"); return false; }
            if (!CheckAction(TEXT("equip"), false, ShieldId, D)) return false;
            FString Worn;
            ShieldWorn(Worn);
            if (!Worn.IsEmpty()) { D = TEXT("host equipped the shield below its level"); return false; }
            D = TEXT("host refused a forged shield equip at level 35");
            return true;
        }, 5.f, false);
    Pre([this, ShieldWorn](FString& Why)
        {
            const FOpenWillowGearItem* Gear = Walker->GetInventory()->FindGearById(ShieldId);
            if (!Gear || Gear->Level != 36) { Why = TEXT("the synthetic shield is missing or is not level 36"); return false; }
            if (Walker->GetSkills()->GetLevel() != 35) { Why = TEXT("host level is not 35"); return false; }
            FString Worn; ShieldWorn(Worn);
            if (!Worn.IsEmpty()) { Why = TEXT("a shield is already worn"); return false; }
            return true;
        }, false);

    Add(TEXT("set_level_36"), false, [this] { Walker->GetSkills()->SetLevel(36); },
        [LevelVerify](FString& D) { return LevelVerify(36, D); }, 8.f);
    Pre([this](FString& Why)
        {
            if (Walker->GetSkills()->GetLevel() != 35) { Why = TEXT("host level is not 35, so raising it to 36 would show nothing"); return false; }
            return true;
        }, false);

    Add(TEXT("gear_equip_shield"), true, [this] { PressKey(TEXT("Enter")); PressKey(TEXT("Enter")); },
        [this, CheckAction, Snapshot, ShieldWorn](FString& D)
        {
            if (ShieldId.IsEmpty()) { D = TEXT("no shield id"); return false; }
            if (!CheckAction(TEXT("equip"), true, ShieldId, D)) return false;
            FString Worn;
            ShieldWorn(Worn);
            if (Worn != ShieldId) { D = FString::Printf(TEXT("host shield slot holds '%s'"), *Worn); return false; }
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            const TSharedPtr<FJsonObject>* GearSlots = nullptr;
            FString PageShield;
            if (Page->TryGetObjectField(TEXT("gearSlots"), GearSlots)) (*GearSlots)->TryGetStringField(TEXT("shield"), PageShield);
            if (PageShield != ShieldId) { D = FString::Printf(TEXT("page shield slot '%s' (snapshot not refreshed)"), *PageShield); return false; }
            D = FString::Printf(TEXT("shield %s equipped at level 36 on host and page"), *ShieldId);
            return true;
        });
    Pre([this, PageOrWhy, Needed, ShieldWorn](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            if (ShieldId.IsEmpty() || PageString(Page, TEXT("sel")) != ShieldId) return Needed(Why, TEXT("page selection"), PageString(Page, TEXT("sel")), ShieldId);
            if (Walker->GetSkills()->GetLevel() != 36 || int32(PageNumber(Page, TEXT("level"))) != 36) { Why = TEXT("player level is not 36"); return false; }
            FString Worn; ShieldWorn(Worn);
            if (!Worn.IsEmpty()) { Why = TEXT("a shield is already worn"); return false; }
            if (PageHasString(Page, TEXT("transfer"))) { Why = TEXT("a transfer is active"); return false; }
            return true;
        });

    Add(TEXT("gear_unequip_shield"), true, [this] { PressKey(TEXT("Delete")); },
        [this, CheckAction, Snapshot, ShieldWorn](FString& D)
        {
            if (ShieldId.IsEmpty()) { D = TEXT("no shield id"); return false; }
            if (!CheckAction(TEXT("unequip"), true, FString(), D)) return false;
            FString Worn;
            ShieldWorn(Worn);
            if (!Worn.IsEmpty()) { D = TEXT("shield still equipped on the host"); return false; }
            if (!Walker->GetInventory()->FindGearById(ShieldId)) { D = TEXT("shield vanished from the inventory"); return false; }
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            const TSharedPtr<FJsonObject>* GearSlots = nullptr;
            FString PageShield;
            if (Page->TryGetObjectField(TEXT("gearSlots"), GearSlots)) (*GearSlots)->TryGetStringField(TEXT("shield"), PageShield);
            if (!PageShield.IsEmpty()) { D = TEXT("page still shows the shield equipped"); return false; }
            D = TEXT("shield unequipped and back in the backpack on host and page");
            return true;
        });
    Pre([this, PageOrWhy, ShieldWorn](FString& Why)
        {
            auto Page = PageOrWhy(Why);
            if (!Page) return false;
            FString Worn; ShieldWorn(Worn);
            if (ShieldId.IsEmpty() || Worn != ShieldId) { Why = FString::Printf(TEXT("host shield slot holds '%s', needed '%s'"), *Worn, *ShieldId); return false; }
            if (PageString(Page, TEXT("gear")) != TEXT("shield")) { Why = TEXT("page target is not the shield gear slot"); return false; }
            return true;
        });

    // Header tabs: K on the inventory page asks the host for the Skills page (a fresh browser, so it
    // may take a while to load), and I on the Skills page brings the inventory page back.
    Add(TEXT("tab_to_skills"), false, [this] { PressKey(TEXT("k")); },
        [this](FString& D)
        {
            if (!Hud->IsSkillsOpen() || Hud->IsInventoryOpen()) { D = TEXT("skills page is not the open status page"); return false; }
            D = TEXT("inventory page routed to the Skills page through the tab route");
            return true;
        }, 60.f, false, 8.f);
    Pre(InventoryOpenPre, false);
    Add(TEXT("tab_back_to_inventory"), false, [this] { PressKey(TEXT("i")); }, OpenVerify, 60.f, true, 2.f);
    Pre([this](FString& Why) { if (!Hud->IsSkillsOpen()) { Why = TEXT("skills page is not open"); return false; } return true; }, false);

    // The page closes itself on Tab as well as Escape.
    Add(TEXT("close_inventory_final"), false, [this] { PressKey(TEXT("Tab")); }, CloseVerify, 6.f, false);
    Pre(InventoryOpenPre, false);
}

void UOpenWillowInventoryActionTest::BeginStep(float Now)
{
    FStep& Step = Steps[StepIndex];
    ActionSerialAtBegin = Hud->LastInventoryAction().Serial;
    StepStart = Now;
    FirstVerifyAt = -1.f;
    UE_LOG(LogTemp, Display, TEXT("OWINVTEST begin step=%d action=%s"), StepIndex + 1, *Step.Name);
    if (Step.Precondition && Step.bPreReport)
    {
        // The precondition is judged against the page as it is now, not against an older report.
        ReportSerialAtRequest = Hud->PageReportSerial();
        Hud->RequestPageReport();
        Phase = EPhase::PreReport;
        PhaseStart = Now;
        return;
    }
    RunBegin(Now);
}

void UOpenWillowInventoryActionTest::RunBegin(float Now)
{
    FStep& Step = Steps[StepIndex];
    if (Step.Precondition)
    {
        FString Why;
        if (!Step.Precondition(Why))
        {
            FinishStepWithStatus(EStatus::NotRun, FString::Printf(TEXT("precondition not met: %s"), *Why));
            return;
        }
    }
    ActionSerialAtBegin = Hud->LastInventoryAction().Serial;
    if (Step.Begin) Step.Begin();
    Phase = Step.bExpectAction ? EPhase::WaitAction : EPhase::Settle;
    PhaseStart = Now;
}

void UOpenWillowInventoryActionTest::FinishStep(bool bOk, const FString& Detail)
{
    const bool bDivergence = bOk && !Steps[StepIndex].DivergenceReason.IsEmpty();
    FinishStepWithStatus(bDivergence ? EStatus::KnownDivergence : bOk ? EStatus::Pass : EStatus::Fail, Detail);
}

void UOpenWillowInventoryActionTest::FinishStepWithStatus(EStatus Status, const FString& Detail)
{
    const FStep& Step = Steps[StepIndex];
    static const TCHAR* const Names[] = {TEXT("PASS"), TEXT("FAIL"), TEXT("NOT_RUN"), TEXT("KNOWN_DIVERGENCE")};
    FString Text = Detail.Replace(TEXT("\n"), TEXT(" "));
    if (Status == EStatus::KnownDivergence) Text += FString::Printf(TEXT(" | known divergence from the original game: %s"), *Step.DivergenceReason);
    UE_LOG(LogTemp, Display, TEXT("OWINVTEST step=%d action=%s status=%s ok=%d detail=%s"),
        StepIndex + 1, *Step.Name, Names[int32(Status)], Status == EStatus::Pass ? 1 : 0, *Text);
    if (Status == EStatus::Fail && Hud)
        UE_LOG(LogTemp, Display, TEXT("OWINVTEST page report at failure: %s"), *Hud->PageReport().Left(1200));
    if (Status == EStatus::Pass && FParse::Param(FCommandLine::Get(), TEXT("owinventoryshots"))
        && (Step.Name == TEXT("open_inventory") || Step.Name == TEXT("gear_equip_shield")
            || Step.Name == TEXT("inspect_weapon") || Step.Name == TEXT("inspect_rotate_weapon")
            || Step.Name == TEXT("transfer_selection_preserves_full_size_cards")
            || Step.Name == TEXT("backpack_transfer_changes_destination")
            || Step.Name == TEXT("backpack_transfer_empty_destination")
            || Step.Name == TEXT("close_inventory_final")))
    {
        // A page report can precede CEF's composited pixels. Hold the current
        // menu state before capturing, then advance only after the capture frame.
        PendingScreenshot = FString::Printf(TEXT("OWInventory_%s"), *Step.Name);
        ScreenshotAt = GetWorld()->GetRealTimeSeconds() + 0.8f;
    }
    switch (Status)
    {
    case EStatus::Pass: ++Passed; break;
    case EStatus::Fail: ++Failed; break;
    case EStatus::NotRun: ++NotRun; break;
    case EStatus::KnownDivergence: ++KnownDivergences; break;
    }
    const bool bAbort = Status != EStatus::Pass && Step.Name == TEXT("open_inventory");
    ++StepIndex;
    if (bAbort) { Summarize(TEXT("aborted: the inventory page never opened")); return; }
    if (!Steps.IsValidIndex(StepIndex)) { Summarize(nullptr); return; }
    if (!PendingScreenshot.IsEmpty()) Phase = EPhase::WaitCapture;
    else BeginStep(GetWorld()->GetRealTimeSeconds());
}

void UOpenWillowInventoryActionTest::Summarize(const TCHAR* Reason)
{
    // Steps never reached (abort, timeout) are NOT_RUN rows, not silently missing ones.
    for (; Steps.IsValidIndex(StepIndex); ++StepIndex)
    {
        UE_LOG(LogTemp, Display, TEXT("OWINVTEST step=%d action=%s status=NOT_RUN ok=0 detail=never reached: %s"),
            StepIndex + 1, *Steps[StepIndex].Name, Reason ? Reason : TEXT("run ended"));
        ++NotRun;
    }
    const bool bBad = Failed > 0 || NotRun > 0 || Reason;
    UE_LOG(LogTemp, Display, TEXT("OWINVTEST SUMMARY result=%s steps=%d passed=%d failed=%d not_run=%d known_divergence=%d keys=%s%s%s"),
        bBad ? TEXT("FAIL") : KnownDivergences > 0 ? TEXT("PASS_WITH_KNOWN_DIVERGENCE") : TEXT("PASS"),
        Steps.Num(), Passed, Failed, NotRun, KnownDivergences,
        bUseJsKeys ? TEXT("js") : TEXT("slate"), Reason ? TEXT(" reason=") : TEXT(""), Reason ? Reason : TEXT(""));
    Phase = EPhase::Finished;
    FinishedAt = GetWorld()->GetRealTimeSeconds();
}

void UOpenWillowInventoryActionTest::Attempt(float Now, bool bGotReport)
{
    FStep& Step = Steps[StepIndex];
    if (FirstVerifyAt < 0.f) FirstVerifyAt = Now;
    FString Detail = TEXT("no page report arrived");
    const bool bOk = (!Step.bPageReport || bGotReport) && Step.Verify && Step.Verify(Detail);
    if (bOk) { FinishStep(true, Detail); return; }
    if (Now - FirstVerifyAt >= Step.DeadlineSeconds) { FinishStep(false, Detail); return; }
    Phase = EPhase::Retry;
    RetryAt = Now + RetrySeconds;
}

void UOpenWillowInventoryActionTest::TickComponent(float DeltaTime, ELevelTick TickType, FActorComponentTickFunction* ThisTickFunction)
{
    Super::TickComponent(DeltaTime, TickType, ThisTickFunction);
    Walker = Cast<AOpenWillowWalker>(GetOwner());
    PC = Walker ? Cast<APlayerController>(Walker->GetController()) : nullptr;
    Hud = PC ? Cast<AOpenWillowMayaHUD>(PC->GetHUD()) : nullptr;
    if (!Walker || !PC || !Hud || !GetWorld()) return;
    const float Now = GetWorld()->GetRealTimeSeconds();
    if (bMeasurePreviewLoop)
    {
        int32 Width = 0, Height = 0;
        PC->GetViewportSize(Width, Height);
        for (TActorIterator<AOpenWillowInventoryMayaDisplay> It(GetWorld()); It; ++It)
        {
            TArray<USkeletalMeshComponent*> Parts;
            It->GetComponents(Parts);
            for (USkeletalMeshComponent* Part : Parts)
            {
                if (Part->GetFName() != TEXT("MayaBody") || !Part->DoesSocketExist(TEXT("Head"))) continue;
                FVector2D Screen;
                if (Width > 0 && Height > 0 && PC->ProjectWorldLocationToScreen(Part->GetBoneLocation(TEXT("Head")), Screen))
                {
                    Screen /= FVector2D(Width, Height);
                    PreviewHeadMin.X = FMath::Min(PreviewHeadMin.X, Screen.X);
                    PreviewHeadMin.Y = FMath::Min(PreviewHeadMin.Y, Screen.Y);
                    PreviewHeadMax.X = FMath::Max(PreviewHeadMax.X, Screen.X);
                    PreviewHeadMax.Y = FMath::Max(PreviewHeadMax.Y, Screen.Y);
                    ++PreviewLoopSamples;
                }
            }
        }
    }
    if (!PendingScreenshot.IsEmpty() && Now >= ScreenshotAt)
    {
        FScreenshotRequest::RequestScreenshot(PendingScreenshot, true, false);
        PendingScreenshot.Reset();
        if (Phase == EPhase::WaitCapture) { Phase = EPhase::AdvanceAfterCapture; return; }
    }
    if (bHavePendingRelease)
    {
        // Released a tick after the press so the action mapping sees both.
        bHavePendingRelease = false;
        PC->InputKey(FInputKeyEventArgs::CreateSimulated(PendingRelease, IE_Released, 0.f));
    }
    if (Phase == EPhase::Finished)
    {
        if (Now - FinishedAt > 1.5f) PC->ConsoleCommand(TEXT("quit"));
        return;
    }
    if (StartedAt < 0.f)
    {
        if (!Walker->IsMayaActive() || GetWorld()->GetTimeSeconds() < 3.f) return;
        StartedAt = Now;
        bUseJsKeys = FParse::Param(FCommandLine::Get(), TEXT("owinventoryjskeys"));
        BuildSteps();
        UE_LOG(LogTemp, Display, TEXT("OWINVTEST start steps=%d keys=%s slots=%d level=%d capacity=%d"), Steps.Num(),
            bUseJsKeys ? TEXT("js") : TEXT("slate"), Walker->GetInventory()->GetWeaponSlotsUnlocked(),
            Walker->GetSkills()->GetLevel(), Walker->GetInventory()->GetBackpackCapacity());
        BeginStep(Now);
        return;
    }
    if (Now - StartedAt > WholeRunSeconds) { Summarize(TEXT("timeout")); return; }
    switch (Phase)
    {
    case EPhase::AdvanceAfterCapture:
        BeginStep(Now);
        break;
    case EPhase::PreReport:
        if (Hud->PageReportSerial() > ReportSerialAtRequest) RunBegin(Now);
        else if (Now - PhaseStart > ReportWaitSeconds)
            FinishStepWithStatus(EStatus::NotRun, TEXT("precondition not met: no fresh page report to check it against"));
        break;
    case EPhase::WaitAction:
        if (Hud->LastInventoryAction().Serial > ActionSerialAtBegin) { Phase = EPhase::Settle; PhaseStart = Now; }
        else if (Now - PhaseStart > ActionWaitSeconds)
            FinishStep(false, TEXT("the host never received the page's request (key not delivered to the page, page did not send it, or the message was lost)"));
        break;
    case EPhase::Settle:
    {
        const FStep& Step = Steps[StepIndex];
        // After a host action the snapshot was already pushed in the same frame.
        if (Now - PhaseStart < (Step.bExpectAction ? 0.25f : Step.SettleSeconds)) break;
        if (!Step.bPageReport) { Attempt(Now, false); break; }
        ReportSerialAtRequest = Hud->PageReportSerial();
        Hud->RequestPageReport();
        Phase = EPhase::WaitReport;
        PhaseStart = Now;
        break;
    }
    case EPhase::WaitReport:
        if (Hud->PageReportSerial() > ReportSerialAtRequest) Attempt(Now, true);
        else if (Now - PhaseStart > ReportWaitSeconds) Attempt(Now, false);
        break;
    case EPhase::Retry:
        if (Now < RetryAt) break;
        if (!Steps[StepIndex].bPageReport) { Attempt(Now, false); break; }
        ReportSerialAtRequest = Hud->PageReportSerial();
        Hud->RequestPageReport();
        Phase = EPhase::WaitReport;
        PhaseStart = Now;
        break;
    default:
        break;
    }
}
