#include "OpenWillowMayaHUD.h"
#include "OpenWillowWalker.h"
#include "Engine/Canvas.h"
#include "GameFramework/PlayerController.h"

void AOpenWillowMayaHUD::DrawHUD()
{
    Super::DrawHUD();
    if (!Canvas || !PlayerOwner) return;
    const AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(PlayerOwner->GetPawn());
    if (!Maya) return;
    const float X = Canvas->ClipX * 0.5f;
    const float Y = Canvas->ClipY * 0.5f;
    DrawLine(X - 9.f, Y, X - 3.f, Y, FLinearColor::White, 1.5f);
    DrawLine(X + 3.f, Y, X + 9.f, Y, FLinearColor::White, 1.5f);
    DrawLine(X, Y - 9.f, X, Y - 3.f, FLinearColor::White, 1.5f);
    DrawLine(X, Y + 3.f, X, Y + 9.f, FLinearColor::White, 1.5f);
    DrawText(Maya->IsInfinityEquipped() ? TEXT("Infinity | ammo: unlimited") : TEXT("Unarmed"),
        FLinearColor::White, 28.f, Canvas->ClipY - 80.f);
    const float Remaining = Maya->PhaselockRemaining();
    DrawText(Remaining > 0.f ? FString::Printf(TEXT("Phaselock: %.1fs"), Remaining) : TEXT("Phaselock: ready"),
        Remaining > 0.f ? FLinearColor(0.65f, 0.4f, 0.9f) : FLinearColor(0.8f, 0.6f, 1.f),
        28.f, Canvas->ClipY - 55.f);
    DrawText(TEXT("LMB fire  |  F Phaselock  |  1 equip  |  0 holster"),
        FLinearColor(0.8f, 0.8f, 0.8f), 28.f, Canvas->ClipY - 30.f);
}
