// AI-assisted (Claude). See OpenWillowQuickChange.h.
#include "OpenWillowQuickChange.h"
#include "OpenWillowInventory.h"
#include "OpenWillowSkills.h"
#include "OpenWillowWalker.h"
#include "Components/SceneComponent.h"
#include "Components/TextRenderComponent.h"
#include "Engine/World.h"

bool FOpenWillowQuickChangePress::Press(double Now)
{
    const bool bConfirmed = PromptedAt >= 0.0 && Now - PromptedAt <= ConfirmSeconds;
    PromptedAt = bConfirmed ? -1.0 : Now;
    return bConfirmed;
}

AOpenWillowQuickChangeStation::AOpenWillowQuickChangeStation()
{
    PrimaryActorTick.bCanEverTick = false;
    RootComponent = CreateDefaultSubobject<USceneComponent>(TEXT("Root"));
    Label = CreateDefaultSubobject<UTextRenderComponent>(TEXT("Label"));
    Label->SetupAttachment(RootComponent);
    Label->SetRelativeLocation(FVector(0.f, 0.f, 200.f));
    Label->SetHorizontalAlignment(EHTA_Center);
    Label->SetWorldSize(16.f);
    Label->SetText(FText::GetEmpty());
}

bool AOpenWillowQuickChangeStation::Use(AOpenWillowWalker& Walker)
{
    UOpenWillowSkills* Skills = Walker.GetSkills();
    UOpenWillowInventory* Inventory = Walker.GetInventory();
    if (!Skills || !Inventory) return false;
    int32 Money = 0;
    Inventory->GetMoney(Money); // unknown money counts as 0
    const int32 Cost = UOpenWillowSkills::ResetCost(Skills->GetLevel());
    if (!Press.Press(GetWorld()->GetTimeSeconds()))
    {
        Label->SetText(FText::FromString(FString::Printf(TEXT("Reset skills: $%d (use again)"), Cost)));
        UE_LOG(LogTemp, Display, TEXT("OWQUICKCHANGE prompt id=%s cost=%d money=%d"), *Id, Cost, Money);
        return true;
    }
    const int32 Before = Money;
    FString Reason;
    const bool bOk = Skills->PurchaseReset(Money, Reason); // changes nothing when refused
    if (bOk) Inventory->SetMoney(Money);
    Label->SetText(FText::FromString(bOk ? TEXT("Skills reset") : Reason));
    UE_LOG(LogTemp, Display, TEXT("OWQUICKCHANGE reset id=%s ok=%d cost=%d before=%d after=%d reason=%s"),
        *Id, bOk ? 1 : 0, Cost, Before, Money, *Reason);
    return true;
}
