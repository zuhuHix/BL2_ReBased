#include "OpenWillowInventoryActionTest.h"
#include "OpenWillowInventory.h"
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

    // Tab is the primary key (the controller forwards it to Maya); I is exercised at the reopen step.
    Add(TEXT("open_inventory"), false, [this] { PressGameKey(EKeys::Tab); }, OpenVerify, 120.f);

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
    Add(TEXT("inspect_escape_returns_inventory"), false, [this] { PressKey(TEXT("Escape")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            bool Open = true;
            if (!Page || !Page->TryGetBoolField(TEXT("inspect"), Open) || Open || !Hud->IsInventoryOpen())
            { D = TEXT("inspect did not close back to inventory"); return false; }
            D = TEXT("Escape closed inspect and kept inventory open"); return true;
        });

    Add(TEXT("equipped_enter_starts_transfer"), false,
        [this] { PressKey(TEXT("Enter")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            FString Source, Compare, Selected;
            if (!Page || !Page->TryGetStringField(TEXT("transfer"), Source)
                || !Page->TryGetStringField(TEXT("compare"), Compare)
                || !Page->TryGetStringField(TEXT("sel"), Selected)
                || Source != HostSlotId(0) || Compare != Source || Selected == Source || HostEquipped(Selected))
            { D = TEXT("equipped Enter did not open a backpack transfer comparison"); return false; }
            D = TEXT("equipped Enter selected a backpack candidate and preserved equipped comparison"); return true;
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
            D = FString::Printf(TEXT("source preserved; card widths %.1f / %.1f"), MainMax-MainMin, OtherMax-OtherMin); return true;
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
                || Walker->GetInventory()->BackpackCount() != CountBefore)
            { D = TEXT("swap mode accepted Drop or Sort"); return false; }
            D = TEXT("swap remained active; Drop and Sort changed neither items nor sort mode"); return true;
        });
    Add(TEXT("transfer_escape_returns_equipped"), false,
        [this] { PressKey(TEXT("Escape")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            FString Selected;
            if (!Page || !Page->TryGetStringField(TEXT("sel"), Selected) || Selected != HostSlotId(0)
                || !Hud->IsInventoryOpen()) { D = TEXT("transfer Escape did not restore equipped selection"); return false; }
            D = TEXT("Escape cancelled transfer and restored equipped selection without closing inventory"); return true;
        });
    Add(TEXT("select_backpack_weapon"), false,
        [this]
        {
            PrevSel.Reset();
            if (TSharedPtr<FJsonObject> Page = PageObject()) Page->TryGetStringField(TEXT("sel"), PrevSel);
            PressKey(TEXT("ArrowDown"));
        },
        [this, Snapshot](FString& D)
        {
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            FString Sel;
            Page->TryGetStringField(TEXT("sel"), Sel);
            if (Sel.IsEmpty() || Sel == PrevSel) { D = FString::Printf(TEXT("selection did not move (sel='%s', before='%s'); key did not reach the page"), *Sel, *PrevSel); return false; }
            if (!Walker->GetInventory()->FindItemById(Sel) || HostEquipped(Sel)) { D = FString::Printf(TEXT("selected '%s' is not an unequipped host weapon"), *Sel); return false; }
            SelId = Sel;
            D = FString::Printf(TEXT("page selected backpack weapon %s"), *Sel);
            return true;
        });

    Add(TEXT("pageup_sorts_preserving_selection"), false,
        [this] { PressKey(TEXT("PageUp")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            double Sort = -1;
            FString Selected;
            bool RowsAligned = false;
            if (!Page || !Page->TryGetNumberField(TEXT("sort"), Sort) || Sort != 1
                || !Page->TryGetStringField(TEXT("sel"), Selected) || Selected != SelId
                || !Page->TryGetBoolField(TEXT("backpackRowsAligned"), RowsAligned) || !RowsAligned)
            { D = TEXT("PageUp failed to sort while retaining selected instance"); return false; }
            D = TEXT("PageUp advanced sort and retained the selected stable ID"); return true;
        });
    Add(TEXT("pagedown_reverses_sort"), false,
        [this] { PressKey(TEXT("PageDown")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            double Sort = -1;
            FString Selected;
            if (!Page || !Page->TryGetNumberField(TEXT("sort"), Sort) || Sort != 0
                || !Page->TryGetStringField(TEXT("sel"), Selected) || Selected != SelId)
            { D = TEXT("PageDown failed to restore sort while retaining selected instance"); return false; }
            D = TEXT("PageDown reversed sort and retained the selected stable ID"); return true;
        });
    Add(TEXT("backpack_select_starts_transfer_without_equipping"), false,
        [this] { PressKey(TEXT("e")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            FString Source, Selected;
            bool FromEquipped = true;
            if (!Page || !Page->TryGetStringField(TEXT("transfer"), Source) || Source != SelId
                || !Page->TryGetStringField(TEXT("sel"), Selected) || Selected != SelId
                || !Page->TryGetBoolField(TEXT("transferFromEquipped"), FromEquipped) || FromEquipped
                || HostEquipped(SelId))
            { D = TEXT("backpack E failed to pin the source or equipped it prematurely"); return false; }
            D = TEXT("backpack E pinned source; host equipment unchanged"); return true;
        });
    Add(TEXT("backpack_transfer_changes_destination"), false,
        [this] { PressKey(TEXT("2")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            FString Source, Selected, Compare;
            double Target = -1;
            if (!Page || !Page->TryGetStringField(TEXT("transfer"), Source) || Source != SelId
                || !Page->TryGetStringField(TEXT("sel"), Selected) || Selected != SelId
                || !Page->TryGetStringField(TEXT("compare"), Compare) || Compare != HostSlotId(1)
                || !Page->TryGetNumberField(TEXT("target"), Target) || Target != 1 || HostEquipped(SelId))
            { D = TEXT("destination change lost source or comparison"); return false; }
            D = TEXT("slot 2 chosen; source fixed and destination weapon compared"); return true;
        });
    Add(TEXT("backpack_transfer_cancel_keeps_source"), false,
        [this] { PressKey(TEXT("Escape")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            FString Selected;
            if (!Page || !Page->TryGetStringField(TEXT("sel"), Selected) || Selected != SelId
                || HostEquipped(SelId) || !Hud->IsInventoryOpen()
                || Page->HasTypedField<EJson::String>(TEXT("transfer")))
            { D = TEXT("cancel lost source, changed equipment or closed inventory"); return false; }
            D = TEXT("Escape cancelled swap, retained backpack source and menu"); return true;
        });
    Add(TEXT("equip_weapon_slot2"), true,
        [this] { DisplacedId = HostSlotId(1); PressKey(TEXT("2")); PressKey(TEXT("Enter")); PressKey(TEXT("Enter")); },
        [this, CheckAction, Snapshot](FString& D)
        {
            if (!CheckAction(TEXT("equip"), true, SelId, D)) return false;
            if (HostSlotId(1) != SelId) { D = FString::Printf(TEXT("host slot 2 holds '%s', wanted %s"), *HostSlotId(1), *SelId); return false; }
            if (!DisplacedId.IsEmpty() && (!Walker->GetInventory()->FindItemById(DisplacedId) || HostEquipped(DisplacedId)))
            { D = FString::Printf(TEXT("displaced weapon %s did not return to the backpack"), *DisplacedId); return false; }
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            if (PageSlot(Page, 1) != SelId) { D = FString::Printf(TEXT("page slot 2 shows '%s' (snapshot not refreshed)"), *PageSlot(Page, 1)); return false; }
            D = FString::Printf(TEXT("%s in slot 2, displaced '%s' back in backpack, page refreshed"), *SelId, *DisplacedId);
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

    Add(TEXT("backpack_transfer_empty_destination"), false,
        [this] { PressKey(TEXT("e")); },
        [this, Snapshot](FString& D)
        {
            auto Page = Snapshot(D);
            FString Source;
            double Target = -1;
            if (!Page || !Page->TryGetStringField(TEXT("transfer"), Source) || Source != SelId
                || !Page->TryGetNumberField(TEXT("target"), Target) || Target != 1
                || Page->HasTypedField<EJson::String>(TEXT("compare")) || !HostSlotId(1).IsEmpty()
                || HostEquipped(SelId))
            { D = TEXT("empty destination did not retain backpack source with no comparison"); return false; }
            D = TEXT("empty slot 2 remains a valid pending destination; no premature equip"); return true;
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

    Add(TEXT("unequip_active_weapon"), true,
        [this] { PressKey(TEXT("Delete")); },
        [this, CheckAction, Snapshot](FString& D)
        {
            if (!CheckAction(TEXT("unequip"), true, FString(), D)) return false;
            if (!HostSlotId(0).IsEmpty()) { D = TEXT("host slot 1 still holds a weapon"); return false; }
            if (Walker->HasWeaponOut() || Walker->GetInventory()->ActiveWeapon()) { D = TEXT("weapon still held after unequipping the active slot"); return false; }
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            if (!PageSlot(Page, 0).IsEmpty()) { D = TEXT("page slot 1 not refreshed"); return false; }
            D = TEXT("active weapon holstered and returned to backpack");
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
    Add(TEXT("drag_weapon_back_to_backpack"), true,
        [this] { Hud->SendPageDrag(SelId, -1); },
        [this, CheckAction, Snapshot](FString& D)
        {
            if (!CheckAction(TEXT("unequip"), true, FString(), D)) return false;
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page || !HostSlotId(1).IsEmpty() || !PageSlot(Page, 1).IsEmpty()
                || !Walker->GetInventory()->FindItemById(SelId) || HostEquipped(SelId))
            { D = TEXT("drag did not return the equipped item to the backpack"); return false; }
            D = TEXT("DOM drag/drop returned the weapon to the backpack without losing it");
            return true;
        });
    Add(TEXT("drag_weapon_to_gear_refused"), false,
        [this] { Hud->SendPageDrag(SelId, 4); },
        [this](FString& D)
        {
            if (Hud->LastInventoryAction().Serial != ActionSerialAtBegin || HostEquipped(SelId))
            { D = TEXT("invalid weapon-to-shield drag sent a request or changed inventory"); return false; }
            D = TEXT("weapon-to-shield drag rejected by the page without sending a request");
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
    Add(TEXT("favorite_on"), true, [this] { PressKey(TEXT("v")); },
        [MarkVerify](FString& D) { return MarkVerify(TEXT("favorite"), 1, 0, D); });
    Add(TEXT("trash_on_clears_favorite"), true, [this] { PressKey(TEXT("t")); },
        [MarkVerify](FString& D) { return MarkVerify(TEXT("trash"), 0, 1, D); });
    Add(TEXT("trash_off"), true, [this] { PressKey(TEXT("t")); },
        [MarkVerify](FString& D) { return MarkVerify(TEXT("trash"), 0, 0, D); });

    Add(TEXT("drop_weapon"), true,
        [this]
        {
            CountBefore = Walker->GetInventory()->BackpackCount();
            DropId = SelId;
            const FOpenWillowWeaponItem* Item = Walker->GetInventory()->FindItemById(DropId);
            DropName = Item ? Item->Name : FString();
            PressKey(TEXT("q"));
        },
        [this, CheckAction, Snapshot](FString& D)
        {
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

    auto CloseVerify = [this](FString& D)
    {
        if (Hud->IsInventoryOpen()) { D = TEXT("inventory page is still open"); return false; }
        if (PC->bShowMouseCursor) { D = TEXT("mouse cursor still shown after closing"); return false; }
        D = TEXT("page closed itself through its close route and game input returned");
        return true;
    };
    Add(TEXT("close_inventory_escape"), false, [this] { PressKey(TEXT("Escape")); }, CloseVerify, 6.f, false);

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

    Add(TEXT("reopen_inventory"), false, [this] { PressGameKey(EKeys::I); },
        [this, OpenVerify, Snapshot](FString& D)
        {
            if (!OpenVerify(D)) return false;
            if (!PageHasItem(Snapshot(D), DropId)) { D = TEXT("reopened page does not list the picked-up weapon"); return false; }
            D += FString::Printf(TEXT("; lists picked-up %s"), *DropId);
            return true;
        }, 20.f);

    // slots=2 (-owslots=2): slots 3 and 4 are locked.
    Add(TEXT("locked_slot_ignored_by_page"), false,
        [this] { PressKey(TEXT("2")); PressKey(TEXT("4")); },
        [this, Snapshot](FString& D)
        {
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            double Target = -1;
            Page->TryGetNumberField(TEXT("target"), Target);
            if (int32(Target) == 3) { D = TEXT("page targeted locked slot 4"); return false; }
            if (int32(Target) != 1) { D = FString::Printf(TEXT("target slot %d, wanted 2 as the key-delivery control"), int32(Target) + 1); return false; }
            D = TEXT("key 2 moved the target to slot 2 (control), key 4 (locked) was ignored");
            return true;
        });

    Add(TEXT("locked_slot_refused_by_host"), true,
        [this]
        {
            // Skip the page's own check: the host must refuse on its own.
            FString Id = SelId;
            if (!Walker->GetInventory()->FindItemById(Id) && Walker->GetInventory()->Items().Num())
                Id = UOpenWillowInventory::StableId(Walker->GetInventory()->Items()[0]);
            SelId = Id;
            Hud->InjectPageRequest(FString::Printf(TEXT("{\"action\":\"equip\",\"id\":\"%s\",\"slot\":3}"), *Id));
        },
        [this, CheckAction](FString& D)
        {
            if (!CheckAction(TEXT("equip"), false, SelId, D)) return false;
            if (!HostSlotId(3).IsEmpty() || !HostSlotId(2).IsEmpty()) { D = TEXT("a locked slot holds a weapon"); return false; }
            D = TEXT("host refused a forged equip into locked slot 4; slots 3-4 empty");
            return true;
        }, 5.f, false);

    // Gear: the shield in the local manifest is level 36.
    Add(TEXT("select_shield_category"), false,
        [this]
        {
            ShieldId.Reset();
            for (const FOpenWillowGearItem& Gear : Walker->GetInventory()->GearItemList())
                if (Gear.ItemType == TEXT("shield")) { ShieldId = Gear.Id; break; }
            PressKey(TEXT("]"));
            PressKey(TEXT("]"));
        },
        [this, Snapshot](FString& D)
        {
            if (ShieldId.IsEmpty()) { D = TEXT("no shield in the local gear manifest"); return false; }
            TSharedPtr<FJsonObject> Page = Snapshot(D);
            if (!Page) return false;
            FString Sel;
            double Category = -1;
            Page->TryGetStringField(TEXT("sel"), Sel);
            Page->TryGetNumberField(TEXT("cat"), Category);
            if (int32(Category) != 2 || Sel != ShieldId)
            { D = FString::Printf(TEXT("category %d selection '%s', wanted category 2 and %s"), int32(Category), *Sel, *ShieldId); return false; }
            D = FString::Printf(TEXT("shield category shows %s selected"), *ShieldId);
            return true;
        });

    auto LevelVerify = [this, Snapshot](int32 Level, FString& D)
    {
        TSharedPtr<FJsonObject> Page = Snapshot(D);
        if (!Page) return false;
        double PageLevel = -1;
        Page->TryGetNumberField(TEXT("level"), PageLevel);
        if (int32(PageLevel) != Level) { D = FString::Printf(TEXT("page level %d, wanted %d (snapshot not refreshed yet)"), int32(PageLevel), Level); return false; }
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

    Add(TEXT("gear_level_gate_in_page"), false, [this] { PressKey(TEXT("Enter")); },
        [this, ShieldWorn](FString& D)
        {
            if (Hud->LastInventoryAction().Serial != ActionSerialAtBegin) { D = TEXT("page sent an equip request below the level requirement"); return false; }
            FString Worn;
            ShieldWorn(Worn);
            if (!Worn.IsEmpty()) { D = TEXT("shield equipped below the level requirement"); return false; }
            D = TEXT("level 35 vs shield level 36: page sent no request, shield stays off");
            return true;
        }, 0.1f, false, 1.5f);

    Add(TEXT("gear_level_gate_in_host"), true,
        [this]
        {
            Hud->InjectPageRequest(FString::Printf(TEXT("{\"action\":\"equip\",\"id\":\"%s\",\"gearSlot\":\"shield\"}"), *ShieldId));
        },
        [this, CheckAction, ShieldWorn](FString& D)
        {
            if (!CheckAction(TEXT("equip"), false, ShieldId, D)) return false;
            FString Worn;
            ShieldWorn(Worn);
            if (!Worn.IsEmpty()) { D = TEXT("host equipped the shield below its level"); return false; }
            D = TEXT("host refused a forged shield equip at level 35");
            return true;
        }, 5.f, false);

    Add(TEXT("set_level_36"), false, [this] { Walker->GetSkills()->SetLevel(36); },
        [LevelVerify](FString& D) { return LevelVerify(36, D); }, 8.f);

    Add(TEXT("gear_equip_shield"), true, [this] { PressKey(TEXT("Enter")); PressKey(TEXT("Enter")); },
        [this, CheckAction, Snapshot, ShieldWorn](FString& D)
        {
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

    Add(TEXT("gear_unequip_shield"), true, [this] { PressKey(TEXT("Delete")); },
        [this, CheckAction, Snapshot, ShieldWorn](FString& D)
        {
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

    // Header tabs: K on the inventory page asks the host for the Skills page (a fresh browser, so it
    // may take a while to load), and I on the Skills page brings the inventory page back.
    Add(TEXT("tab_to_skills"), false, [this] { PressKey(TEXT("k")); },
        [this](FString& D)
        {
            if (!Hud->IsSkillsOpen() || Hud->IsInventoryOpen()) { D = TEXT("skills page is not the open status page"); return false; }
            D = TEXT("inventory page routed to the Skills page through the tab route");
            return true;
        }, 60.f, false, 8.f);
    Add(TEXT("tab_back_to_inventory"), false, [this] { PressKey(TEXT("i")); }, OpenVerify, 60.f, true, 2.f);

    // The page closes itself on Tab as well as Escape.
    Add(TEXT("close_inventory_final"), false, [this] { PressKey(TEXT("Tab")); }, CloseVerify, 6.f, false);
}

void UOpenWillowInventoryActionTest::BeginStep(float Now)
{
    FStep& Step = Steps[StepIndex];
    ActionSerialAtBegin = Hud->LastInventoryAction().Serial;
    StepStart = Now;
    FirstVerifyAt = -1.f;
    UE_LOG(LogTemp, Display, TEXT("OWINVTEST begin step=%d action=%s"), StepIndex + 1, *Step.Name);
    if (Step.Begin) Step.Begin();
    Phase = Step.bExpectAction ? EPhase::WaitAction : EPhase::Settle;
    PhaseStart = Now;
}

void UOpenWillowInventoryActionTest::FinishStep(bool bOk, const FString& Detail)
{
    const FStep& Step = Steps[StepIndex];
    UE_LOG(LogTemp, Display, TEXT("OWINVTEST step=%d action=%s ok=%d detail=%s"),
        StepIndex + 1, *Step.Name, bOk ? 1 : 0, *Detail.Replace(TEXT("\n"), TEXT(" ")));
    if (!bOk && Hud)
        UE_LOG(LogTemp, Display, TEXT("OWINVTEST page report at failure: %s"), *Hud->PageReport().Left(1200));
    if (bOk && FParse::Param(FCommandLine::Get(), TEXT("owinventoryshots"))
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
    (bOk ? Passed : Failed)++;
    const bool bAbort = !bOk && Step.Name == TEXT("open_inventory");
    ++StepIndex;
    if (bAbort) { Summarize(TEXT("aborted: the inventory page never opened")); return; }
    if (!Steps.IsValidIndex(StepIndex)) { Summarize(nullptr); return; }
    if (!PendingScreenshot.IsEmpty()) Phase = EPhase::WaitCapture;
    else BeginStep(GetWorld()->GetRealTimeSeconds());
}

void UOpenWillowInventoryActionTest::Summarize(const TCHAR* Reason)
{
    const int32 NotRun = Steps.Num() - Passed - Failed;
    const bool bPass = Failed == 0 && NotRun == 0 && !Reason;
    UE_LOG(LogTemp, Display, TEXT("OWINVTEST SUMMARY result=%s steps=%d passed=%d failed=%d not_run=%d keys=%s%s%s"),
        bPass ? TEXT("PASS") : TEXT("FAIL"), Steps.Num(), Passed, Failed, NotRun,
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
