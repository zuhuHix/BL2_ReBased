#include "OpenWillowMinimap.h"
#include "OpenWillowQuest.h"
#include "OpenWillowWalker.h"
#include "Dom/JsonObject.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "HAL/FileManager.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "UnrealClient.h"

namespace
{
    struct FManifest
    {
        bool bLoaded = false;
        FString Movie;
        double CentreX = 0, CentreY = 0;
        FString Blobs = TEXT("{}");                 // JSON object text: fog blob name -> uncovered
        TArray<TPair<FString, FVector>> Icons;      // stock icons: kind, location
    };

    FString ManifestPath()
    {
        FString Path;
        if (FParse::Value(FCommandLine::Get(), TEXT("owminimap="), Path)) return Path;
        return FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/slice/minimap.json")));
    }

    const FManifest& Manifest()
    {
        static FManifest M;
        static bool bTried = false;
        if (bTried) return M;
        bTried = true;
        const FString Path = ManifestPath();
        FString Text;
        TSharedPtr<FJsonObject> Json;
        if (!FFileHelper::LoadFileToString(Text, *Path) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Json)
            || !Json.IsValid())
        {
            UE_LOG(LogTemp, Display, TEXT("OWMINIMAP no manifest at %s (tools/prepare_minimap.py); the minimap stays as authored"), *Path);
            return M;
        }
        const TArray<TSharedPtr<FJsonValue>>* Centre = nullptr;
        if (!Json->TryGetStringField(TEXT("movie"), M.Movie) || !Json->TryGetArrayField(TEXT("centre"), Centre) || Centre->Num() != 2)
        {
            UE_LOG(LogTemp, Warning, TEXT("OWMINIMAP %s lacks movie or centre; no minimap"), *Path);
            return M;
        }
        M.CentreX = (*Centre)[0]->AsNumber();
        M.CentreY = (*Centre)[1]->AsNumber();
        const TSharedPtr<FJsonObject>* Blobs = nullptr;
        if (Json->TryGetObjectField(TEXT("blobs"), Blobs))
        {
            FString Out;
            const auto Writer = TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Out);
            FJsonSerializer::Serialize(Blobs->ToSharedRef(), Writer);
            M.Blobs = Out;
        }
        const TArray<TSharedPtr<FJsonValue>>* Icons = nullptr;
        if (Json->TryGetArrayField(TEXT("icons"), Icons))
        {
            for (const TSharedPtr<FJsonValue>& Value : *Icons)
            {
                const TSharedPtr<FJsonObject> Icon = Value->AsObject();
                const TArray<TSharedPtr<FJsonValue>>* L = nullptr;
                FString Kind;
                if (!Icon || !Icon->TryGetStringField(TEXT("kind"), Kind) || !Icon->TryGetArrayField(TEXT("location"), L) || L->Num() != 3) continue;
                M.Icons.Emplace(Kind, FVector((*L)[0]->AsNumber(), (*L)[1]->AsNumber(), (*L)[2]->AsNumber()));
            }
        }
        M.bLoaded = true;
        UE_LOG(LogTemp, Display, TEXT("OWMINIMAP manifest %s: movie %s, centre (%.1f, %.1f), %d icons"), *Path, *M.Movie, M.CentreX, M.CentreY, M.Icons.Num());
        return M;
    }

    // The real-game capture spots (docs/verification/HUD_MINIMAP.md, local/realgame/minimap): pawn location and view yaw.
    struct FSpot { float X, Y, Z, Yaw; };
    const FSpot Spots[] = {
        {8109, 2930, 3780, 0}, {8109, 2930, 3780, 90}, {8109, 2930, 3780, 180}, {8109, 2930, 3780, 270},
        {10300, 4800, 3760, 0}, {10300, 4800, 3760, 90}, {10300, 4800, 3760, 200}, {10300, 4800, 3760, 300},
    };
    int32 ShotStep = 0;
    float ShotAt = 0;
    FString LastLine;
}

FString OpenWillowMinimap::StateField(const AOpenWillowWalker& Maya, const APlayerController& Player)
{
    const FManifest& M = Manifest();
    if (!M.bLoaded) return FString();
    TArray<TPair<FString, FVector>> Icons = M.Icons;
    if (const UOpenWillowQuest* Quest = Maya.GetQuest()) Quest->MinimapMarkers(Icons);
    const FVector P = Maya.GetActorLocation();
    // The widget follows the view's yaw (CachedPlayerYaw tracked the controller, not the pawn, in the game).
    const double Yaw = FRotator::NormalizeAxis(Player.GetControlRotation().Yaw);
    FString IconJson;
    for (const TPair<FString, FVector>& Icon : Icons)
        IconJson += FString::Printf(TEXT("%s{\"kind\":\"%s\",\"x\":%.0f,\"y\":%.0f}"), IconJson.IsEmpty() ? TEXT("") : TEXT(","), *Icon.Key, Icon.Value.X, Icon.Value.Y);
    const FString State = FString::Printf(TEXT("{\"movie\":\"%s\",\"centre\":[%.1f,%.1f],\"blobs\":%s,\"x\":%.0f,\"y\":%.0f,\"yaw\":%.1f,\"icons\":[%s]}"),
        *M.Movie, M.CentreX, M.CentreY, *M.Blobs, P.X, P.Y, Yaw, *IconJson);
    const FString Line = FString::Printf(TEXT("x=%.0f y=%.0f yaw=%.1f icons=%d"), P.X, P.Y, Yaw, Icons.Num());
    if (Line != LastLine)
    {
        UE_LOG(LogTemp, Display, TEXT("OWMINIMAP t=%.3f %s"), Maya.GetWorld()->GetTimeSeconds(), *Line);
        LastLine = Line;
    }
    return State;
}

void OpenWillowMinimap::TickShots(AOpenWillowWalker& Maya, APlayerController& Player)
{
    static const bool bShots = FParse::Param(FCommandLine::Get(), TEXT("owminimapshots"));
    if (!bShots || ShotStep < 0) return;
    // The movie needs a few seconds to load and settle (the 3D-turn stand-in waits a second after it is ready);
    // each spot gets 2 s for the pawn to land before its capture.
    constexpr float Start = 12.f, Settle = 2.f;
    constexpr int32 Count = UE_ARRAY_COUNT(Spots);
    const float Now = Maya.GetWorld()->GetTimeSeconds();
    if (Now < Start) return;
    if (ShotStep >= 2 * Count)
    {
        if (Now < ShotAt + 1.f) return;
        UE_LOG(LogTemp, Display, TEXT("OWMINIMAPSHOTS done"));
        Player.ConsoleCommand(TEXT("quit"));
        ShotStep = -1;
        return;
    }
    const FSpot& Spot = Spots[ShotStep / 2];
    if (ShotStep % 2 == 0)
    {
        Maya.SetActorLocation(FVector(Spot.X, Spot.Y, Spot.Z), false, nullptr, ETeleportType::TeleportPhysics);
        Player.SetControlRotation(FRotator(0, Spot.Yaw, 0));
        ShotAt = Now;
        ++ShotStep;
        return;
    }
    if (Now < ShotAt + Settle) return;
    const FString Name = FString::Printf(TEXT("OWMinimap_%03d.png"), ShotStep / 2);
    FScreenshotRequest::RequestScreenshot(Name, true, false);
    const FVector P = Maya.GetActorLocation();
    UE_LOG(LogTemp, Display, TEXT("OWMINIMAPSHOTS shot %s spot %d at (%.0f, %.0f, %.0f) yaw %.1f t=%.3f"), *Name, ShotStep / 2, P.X, P.Y, P.Z,
        FRotator::NormalizeAxis(Player.GetControlRotation().Yaw), Now);
    ShotAt = Now;
    ++ShotStep;
}
