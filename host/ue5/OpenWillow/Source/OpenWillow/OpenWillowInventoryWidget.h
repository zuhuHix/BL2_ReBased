#pragma once

#include "CoreMinimal.h"
#include "Blueprint/UserWidget.h"
#include "OpenWillowInventoryWidget.generated.h"

class AOpenWillowWalker;
class AOpenWillowInventoryPreviewActor;
class UVerticalBox;
class UTextBlock;
class UImage;

// One clickable row: an equipped slot or a backpack item. Built in C++ so
// the layout is reviewable text, not a binary Blueprint asset.
UCLASS()
class OPENWILLOW_API UOpenWillowItemRow : public UUserWidget
{
    GENERATED_BODY()
public:
    void Setup(class UOpenWillowInventoryWidget* InOwner, int32 InItem, int32 InSlot, const FText& Label,
        const FLinearColor& Color, bool bHighlighted);
protected:
    virtual TSharedRef<SWidget> RebuildWidget() override;
private:
    UFUNCTION() void Clicked();
    UFUNCTION() void Hovered();
    UPROPERTY() TObjectPtr<class UOpenWillowInventoryWidget> Owner;
    UPROPERTY() TObjectPtr<class UButton> Button;
    UPROPERTY() TObjectPtr<UTextBlock> Text;
    int32 Item = INDEX_NONE;
    int32 RowSlot = INDEX_NONE;
};

// BL2-style inventory screen: equipped slots and backpack on the left, the
// item card on the right with stats compared against the equipped weapon.
UCLASS()
class OPENWILLOW_API UOpenWillowInventoryWidget : public UUserWidget
{
    GENERATED_BODY()
public:
    // Opens on the held weapon's slot, like BL2's inventory.
    void Bind(AOpenWillowWalker* InWalker);
    void Refresh();
    void ShowCard(int32 Item);
    void RowClicked(int32 Item, int32 ClickedSlot);
protected:
    virtual TSharedRef<SWidget> RebuildWidget() override;
    virtual void NativeDestruct() override;
    virtual FReply NativeOnKeyDown(const FGeometry& Geometry, const FKeyEvent& Event) override;
private:
    void EnsurePreviewActor();
    UPROPERTY() TObjectPtr<AOpenWillowWalker> Walker;
    UPROPERTY() TObjectPtr<AOpenWillowInventoryPreviewActor> PreviewActor;
    UPROPERTY() TObjectPtr<UVerticalBox> SlotList;
    UPROPERTY() TObjectPtr<UVerticalBox> BackpackList;
    UPROPERTY() TObjectPtr<UVerticalBox> Card;
    UPROPERTY() TObjectPtr<UImage> PreviewImage;
    UPROPERTY() TObjectPtr<UTextBlock> PreviewStatus;
    int32 CardItem = INDEX_NONE;
    int32 TargetSlot = 0;
};
