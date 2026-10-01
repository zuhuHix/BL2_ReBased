#include "OpenWillowInventoryWidget.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInventoryPreviewActor.h"
#include "OpenWillowWalker.h"
#include "Blueprint/WidgetTree.h"
#include "Components/Border.h"
#include "Components/Button.h"
#include "Components/HorizontalBox.h"
#include "Components/HorizontalBoxSlot.h"
#include "Components/Image.h"
#include "Components/Overlay.h"
#include "Components/OverlaySlot.h"
#include "Components/ScrollBox.h"
#include "Components/SizeBox.h"
#include "Components/TextBlock.h"
#include "Components/VerticalBox.h"
#include "Components/VerticalBoxSlot.h"
#include "Engine/TextureRenderTarget2D.h"
#include "Styling/CoreStyle.h"

// Layout follows BL2's inventory (equipped slots and backpack beside an item
// card with better/worse comparison colours). Fonts, frames and icons are
// host stand-ins; BL2's Scaleform art is not used.
namespace
{
const FLinearColor CardPanelColor(0.02f, 0.03f, 0.05f, 0.88f);
const FLinearColor CardHeaderColor(0.95f, 0.8f, 0.3f);
const FLinearColor StatBetterColor(0.3f, 1.f, 0.3f);
const FLinearColor StatWorseColor(1.f, 0.3f, 0.25f);

FLinearColor RarityColor(int32 Rarity)
{
    switch (Rarity)
    {
    case 2: return FLinearColor(0.25f, 0.9f, 0.2f);
    case 3: return FLinearColor(0.2f, 0.5f, 1.f);
    case 4: return FLinearColor(0.65f, 0.3f, 1.f);
    case 5: return FLinearColor(1.f, 0.55f, 0.08f);
    default: return FLinearColor::White;
    }
}

UTextBlock* Label(UWidgetTree* Tree, const FText& Text, int32 Size, const FLinearColor& Color, bool bBold = false)
{
    UTextBlock* Block = Tree->ConstructWidget<UTextBlock>();
    Block->SetText(Text);
    Block->SetFont(FCoreStyle::GetDefaultFontStyle(bBold ? "Bold" : "Regular", Size));
    Block->SetColorAndOpacity(FSlateColor(Color));
    return Block;
}
}

void UOpenWillowItemRow::Setup(UOpenWillowInventoryWidget* InOwner, int32 InItem, int32 InSlot,
    const FText& InLabel, const FLinearColor& Color, bool bHighlighted)
{
    Owner = InOwner;
    Item = InItem;
    RowSlot = InSlot;
    // Constructing the row builds its tree; fill it afterwards.
    TakeWidget();
    Text->SetText(InLabel);
    Text->SetColorAndOpacity(FSlateColor(Color));
    Button->SetBackgroundColor(bHighlighted ? FLinearColor(0.35f, 0.3f, 0.12f, 1.f) : FLinearColor(0.08f, 0.09f, 0.11f, 1.f));
}

TSharedRef<SWidget> UOpenWillowItemRow::RebuildWidget()
{
    if (WidgetTree && !WidgetTree->RootWidget)
    {
        Button = WidgetTree->ConstructWidget<UButton>();
        Text = Label(WidgetTree, FText::GetEmpty(), 15, FLinearColor::White, true);
        Button->AddChild(Text);
        Button->OnClicked.AddDynamic(this, &UOpenWillowItemRow::Clicked);
        Button->OnHovered.AddDynamic(this, &UOpenWillowItemRow::Hovered);
        WidgetTree->RootWidget = Button;
    }
    return Super::RebuildWidget();
}

void UOpenWillowItemRow::Clicked() { if (Owner) Owner->RowClicked(Item, RowSlot); }
void UOpenWillowItemRow::Hovered() { if (Owner && Item != INDEX_NONE) Owner->ShowCard(Item); }

TSharedRef<SWidget> UOpenWillowInventoryWidget::RebuildWidget()
{
    if (WidgetTree && !WidgetTree->RootWidget)
    {
        UBorder* Backdrop = WidgetTree->ConstructWidget<UBorder>();
        Backdrop->SetBrushColor(FLinearColor(0.f, 0.f, 0.f, 0.55f));
        Backdrop->SetPadding(FMargin(80.f, 60.f));
        UHorizontalBox* Columns = WidgetTree->ConstructWidget<UHorizontalBox>();
        Backdrop->SetContent(Columns);

        // Left: equipped slots, then the backpack list.
        UBorder* LeftPanel = WidgetTree->ConstructWidget<UBorder>();
        LeftPanel->SetBrushColor(CardPanelColor);
        LeftPanel->SetPadding(FMargin(18.f));
        UVerticalBox* Left = WidgetTree->ConstructWidget<UVerticalBox>();
        LeftPanel->SetContent(Left);
        Left->AddChildToVerticalBox(Label(WidgetTree, FText::FromString(TEXT("EQUIPPED")), 20, CardHeaderColor, true));
        SlotList = WidgetTree->ConstructWidget<UVerticalBox>();
        Left->AddChildToVerticalBox(SlotList);
        UTextBlock* BackpackTitle = Label(WidgetTree, FText::FromString(TEXT("BACKPACK")), 20, CardHeaderColor, true);
        Left->AddChildToVerticalBox(BackpackTitle)->SetPadding(FMargin(0, 18.f, 0, 0));
        UScrollBox* Scroll = WidgetTree->ConstructWidget<UScrollBox>();
        BackpackList = WidgetTree->ConstructWidget<UVerticalBox>();
        Scroll->AddChild(BackpackList);
        Left->AddChildToVerticalBox(Scroll)->SetSize(FSlateChildSize(ESlateSizeRule::Fill));
        Left->AddChildToVerticalBox(Label(WidgetTree, FText::FromString(
            TEXT("Click a slot to target it, then click an item to equip it.  I / Esc: close")), 12,
            FLinearColor(0.7f, 0.7f, 0.7f)))->SetPadding(FMargin(0, 10.f, 0, 0));
        USizeBox* LeftSize = WidgetTree->ConstructWidget<USizeBox>();
        LeftSize->SetWidthOverride(460.f);
        LeftSize->SetContent(LeftPanel);
        Columns->AddChildToHorizontalBox(LeftSize);

        // Right: the item card.
        UBorder* CardPanel = WidgetTree->ConstructWidget<UBorder>();
        CardPanel->SetBrushColor(CardPanelColor);
        CardPanel->SetPadding(FMargin(26.f));

        UVerticalBox* CardLayout = WidgetTree->ConstructWidget<UVerticalBox>();
        UBorder* PreviewPanel = WidgetTree->ConstructWidget<UBorder>();
        PreviewPanel->SetBrushColor(FLinearColor(0.01f, 0.015f, 0.02f, 1.f));
        PreviewPanel->SetPadding(FMargin(6.f));
        UOverlay* PreviewOverlay = WidgetTree->ConstructWidget<UOverlay>();
        PreviewImage = WidgetTree->ConstructWidget<UImage>();
        PreviewImage->SetVisibility(ESlateVisibility::Collapsed);
        PreviewOverlay->AddChildToOverlay(PreviewImage);
        PreviewStatus = Label(WidgetTree, FText::FromString(TEXT("Select a weapon to preview")), 15,
            FLinearColor(0.65f, 0.68f, 0.72f));
        UOverlaySlot* StatusSlot = PreviewOverlay->AddChildToOverlay(PreviewStatus);
        StatusSlot->SetHorizontalAlignment(HAlign_Center);
        StatusSlot->SetVerticalAlignment(VAlign_Center);
        PreviewPanel->SetContent(PreviewOverlay);
        USizeBox* PreviewSize = WidgetTree->ConstructWidget<USizeBox>();
        PreviewSize->SetHeightOverride(236.f);
        PreviewSize->SetContent(PreviewPanel);
        CardLayout->AddChildToVerticalBox(PreviewSize);

        Card = WidgetTree->ConstructWidget<UVerticalBox>();
        CardLayout->AddChildToVerticalBox(Card)->SetPadding(FMargin(0.f, 16.f, 0.f, 0.f));
        CardPanel->SetContent(CardLayout);
        USizeBox* CardSize = WidgetTree->ConstructWidget<USizeBox>();
        CardSize->SetWidthOverride(460.f);
        CardSize->SetContent(CardPanel);
        UHorizontalBoxSlot* CardSlot = Columns->AddChildToHorizontalBox(CardSize);
        CardSlot->SetPadding(FMargin(40.f, 0, 0, 0));
        CardSlot->SetVerticalAlignment(VAlign_Top);
        WidgetTree->RootWidget = Backdrop;
    }
    return Super::RebuildWidget();
}

void UOpenWillowInventoryWidget::Bind(AOpenWillowWalker* InWalker)
{
    Walker = InWalker;
    const int32 Held = Walker ? Walker->GetInventory()->GetActiveSlot() : INDEX_NONE;
    TargetSlot = Held == INDEX_NONE ? 0 : Held;
    CardItem = INDEX_NONE;
}

void UOpenWillowInventoryWidget::Refresh()
{
    if (!Walker || !SlotList) return;
    const UOpenWillowInventory* Inventory = Walker->GetInventory();
    SlotList->ClearChildren();
    BackpackList->ClearChildren();
    for (int32 SlotIndex = 0; SlotIndex < UOpenWillowInventory::SlotCount; ++SlotIndex)
    {
        const FOpenWillowWeaponItem* Held = Inventory->SlotItem(SlotIndex);
        const int32 HeldIndex = Held ? int32(Held - Inventory->Items().GetData()) : INDEX_NONE;
        UOpenWillowItemRow* Row = CreateWidget<UOpenWillowItemRow>(this);
        Row->Setup(this, HeldIndex, SlotIndex, FText::FromString(FString::Printf(TEXT("%d   %s"), SlotIndex + 1,
            Held ? *Held->Name : TEXT("- empty -"))), Held ? RarityColor(Held->Rarity) : FLinearColor(0.5f, 0.5f, 0.5f),
            SlotIndex == TargetSlot);
        SlotList->AddChildToVerticalBox(Row)->SetPadding(FMargin(0, 3.f));
    }
    for (int32 Index = 0; Index < Inventory->Items().Num(); ++Index)
    {
        const FOpenWillowWeaponItem& Item = Inventory->Items()[Index];
        UOpenWillowItemRow* Row = CreateWidget<UOpenWillowItemRow>(this);
        Row->Setup(this, Index, INDEX_NONE, FText::FromString(FString::Printf(TEXT("%s   Lv %d"), *Item.Name, Item.Level)),
            RarityColor(Item.Rarity), Index == CardItem);
        BackpackList->AddChildToVerticalBox(Row)->SetPadding(FMargin(0, 2.f));
    }
    if (CardItem == INDEX_NONE && Inventory->SlotItem(TargetSlot))
        CardItem = int32(Inventory->SlotItem(TargetSlot) - Inventory->Items().GetData());
    ShowCard(CardItem);
}

void UOpenWillowInventoryWidget::ShowCard(int32 Index)
{
    if (!Walker || !Card) return;
    const UOpenWillowInventory* Inventory = Walker->GetInventory();
    Card->ClearChildren();
    if (!Inventory->Items().IsValidIndex(Index))
    {
        if (PreviewActor) PreviewActor->SetItemPreview(nullptr);
        if (PreviewImage) PreviewImage->SetVisibility(ESlateVisibility::Collapsed);
        if (PreviewStatus)
        {
            PreviewStatus->SetText(FText::FromString(TEXT("Select a weapon to preview")));
            PreviewStatus->SetVisibility(ESlateVisibility::Visible);
        }
        return;
    }

    CardItem = Index;
    const FOpenWillowWeaponItem& Item = Inventory->Items()[Index];
    EnsurePreviewActor();
    const bool bPreviewLoaded = PreviewActor && PreviewActor->SetItemPreview(&Item)
        && PreviewActor->GetRenderTarget();
    if (PreviewImage)
    {
        if (bPreviewLoaded)
        {
            FSlateBrush Brush;
            Brush.SetResourceObject(PreviewActor->GetRenderTarget());
            Brush.ImageSize = FVector2D(408.f, 224.f);
            PreviewImage->SetBrush(Brush);
            PreviewImage->SetVisibility(ESlateVisibility::Visible);
        }
        else
        {
            PreviewImage->SetVisibility(ESlateVisibility::Collapsed);
        }
    }
    if (PreviewStatus)
    {
        PreviewStatus->SetText(FText::FromString(bPreviewLoaded
            ? TEXT("") : TEXT("Weapon model unavailable")));
        PreviewStatus->SetVisibility(bPreviewLoaded
            ? ESlateVisibility::Collapsed : ESlateVisibility::Visible);
    }

    const FOpenWillowWeaponItem* Equipped = Inventory->SlotItem(TargetSlot);
    Card->AddChildToVerticalBox(Label(WidgetTree, FText::FromString(Item.Name), 30, RarityColor(Item.Rarity), true));
    Card->AddChildToVerticalBox(Label(WidgetTree, FText::FromString(FString::Printf(TEXT("Level %d  %s Pistol"),
        Item.Level, *Item.Manufacturer)), 14, FLinearColor(0.8f, 0.8f, 0.8f)))->SetPadding(FMargin(0, 2.f, 0, 16.f));

    // Each stat shows the delta against the targeted slot's weapon. For
    // reload and spread, lower is better, as on BL2's cards.
    auto Stat = [&](const TCHAR* Name, float Value, float Other, bool bLowerBetter, int32 Decimals)
    {
        UHorizontalBox* Line = WidgetTree->ConstructWidget<UHorizontalBox>();
        Line->AddChildToHorizontalBox(Label(WidgetTree, FText::FromString(Name), 17, FLinearColor::White))
            ->SetSize(FSlateChildSize(ESlateSizeRule::Fill));
        Line->AddChildToHorizontalBox(Label(WidgetTree, FText::FromString(FString::Printf(TEXT("%.*f"), Decimals, Value)), 17,
            FLinearColor::White, true));
        if (Equipped && &Item != Equipped && !FMath::IsNearlyEqual(Value, Other, 0.005f))
        {
            const bool bBetter = bLowerBetter ? Value < Other : Value > Other;
            Line->AddChildToHorizontalBox(Label(WidgetTree, FText::FromString(bBetter ? TEXT("  ▲") : TEXT("  ▼")),
                15, bBetter ? StatBetterColor : StatWorseColor, true));
        }
        Card->AddChildToVerticalBox(Line)->SetPadding(FMargin(0, 3.f));
    };
    Stat(TEXT("Damage"), Item.Damage, Equipped ? Equipped->Damage : 0, false, 0);
    Stat(TEXT("Fire Rate"), Item.FireRate, Equipped ? Equipped->FireRate : 0, false, 1);
    Stat(TEXT("Reload Speed"), Item.ReloadTime, Equipped ? Equipped->ReloadTime : 0, true, 1);
    Stat(TEXT("Magazine Size"), Item.Magazine, Equipped ? Equipped->Magazine : 0, false, 0);
    Stat(TEXT("Spread"), Item.Spread, Equipped ? Equipped->Spread : 0, true, 2);
    if (Item.Element != TEXT("None") && !Item.Element.IsEmpty())
        Card->AddChildToVerticalBox(Label(WidgetTree, FText::FromString(Item.Element + TEXT(" damage")), 16,
            FLinearColor(1.f, 0.6f, 0.2f), true))->SetPadding(FMargin(0, 10.f, 0, 0));
    if (Item.ShotCost <= 0.f)
        Card->AddChildToVerticalBox(Label(WidgetTree, FText::FromString(TEXT("Consumes no ammo")), 15,
            FLinearColor(1.f, 0.4f, 0.3f)))->SetPadding(FMargin(0, 6.f, 0, 0));
    if (Item.SpinUp > 0.f)
        Card->AddChildToVerticalBox(Label(WidgetTree, FText::FromString(FString::Printf(
            TEXT("Barrel spins up in %.2f s"), Item.SpinUp)), 13, FLinearColor(0.75f, 0.75f, 0.75f)));
    Card->AddChildToVerticalBox(Label(WidgetTree, FText::FromString(TEXT(
        "Stats evaluated from game data; combination rules UNVERIFIED.")), 11,
        FLinearColor(0.55f, 0.55f, 0.55f)))->SetPadding(FMargin(0, 18.f, 0, 0));
}

void UOpenWillowInventoryWidget::EnsurePreviewActor()
{
    if (PreviewActor || !Walker || !GetWorld()) return;

    FActorSpawnParameters SpawnParameters;
    SpawnParameters.Owner = Walker;
    SpawnParameters.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
    PreviewActor = GetWorld()->SpawnActor<AOpenWillowInventoryPreviewActor>(
        AOpenWillowInventoryPreviewActor::StaticClass(), Walker->GetActorTransform(), SpawnParameters);
}

void UOpenWillowInventoryWidget::NativeDestruct()
{
    Super::NativeDestruct();
    if (IsValid(PreviewActor))
    {
        PreviewActor->Destroy();
    }
    PreviewActor = nullptr;
}

void UOpenWillowInventoryWidget::RowClicked(int32 Item, int32 ClickedSlot)
{
    if (!Walker) return;
    if (ClickedSlot != INDEX_NONE)
    {
        TargetSlot = ClickedSlot;           // pick which slot the next item goes into
        CardItem = Item;
    }
    else if (Item != INDEX_NONE)
    {
        Walker->EquipItem(Item, TargetSlot);
        CardItem = Item;
    }
    Refresh();
}

FReply UOpenWillowInventoryWidget::NativeOnKeyDown(const FGeometry& Geometry, const FKeyEvent& Event)
{
    if (Walker && Event.GetKey() == EKeys::K)
    {
        Walker->ToggleSkills();
        return FReply::Handled();
    }
    if (Walker && (Event.GetKey() == EKeys::I || Event.GetKey() == EKeys::Escape))
    {
        Walker->ToggleInventory();
        return FReply::Handled();
    }
    return Super::NativeOnKeyDown(Geometry, Event);
}
