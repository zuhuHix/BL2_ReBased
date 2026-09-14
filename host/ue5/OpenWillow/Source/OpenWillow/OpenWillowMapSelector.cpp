#include "OpenWillowMapSelector.h"
#include "Components/InputComponent.h"
#include "Dom/JsonObject.h"
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "HAL/FileManager.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

namespace
{
    // On-screen debug message keys; one per menu line.
    constexpr uint64 MenuMessageKey = 0x4F574D41500ULL;
    constexpr int32 MaxDigitEntries = 9;

    // Same rule as run_ue_level.ps1: an Unreal-safe persistent map name.
    bool MatchesPersistentName(const FString& Name)
    {
        if (Name.Len() < 3 || !Name.EndsWith(TEXT("_P"), ESearchCase::CaseSensitive)) return false;
        for (TCHAR C : Name)
        {
            if (!FChar::IsAlnum(C) && C != TEXT('_')) return false;
        }
        return true;
    }

    FString ReadManifestMap(const FString& ManifestPath)
    {
        FString Text;
        if (!FFileHelper::LoadFileToString(Text, *ManifestPath)) return FString();
        TSharedPtr<FJsonObject> Root;
        if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root.IsValid()) return FString();
        FString Map;
        Root->TryGetStringField(TEXT("map"), Map);
        return Map;
    }
}

bool AOpenWillowPlayerController::IsPersistentMapName(const FString& Name)
{
    return MatchesPersistentName(Name);
}

TArray<FOpenWillowMapEntry> AOpenWillowPlayerController::DiscoverMaps()
{
    TMap<FString, FOpenWillowMapEntry> Found;
    IFileManager& Files = IFileManager::Get();

    // Prepared scenes: <repo>/local/<name>/scene.json, keyed by the manifest's map.
    const FString LocalDir = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local")));
    TArray<FString> SceneDirs;
    Files.FindFiles(SceneDirs, *FPaths::Combine(LocalDir, TEXT("*")), false, true);
    for (const FString& Dir : SceneDirs)
    {
        const FString Map = ReadManifestMap(FPaths::Combine(LocalDir, Dir, TEXT("scene.json")));
        if (!MatchesPersistentName(Map)) continue;
        FOpenWillowMapEntry& Entry = Found.FindOrAdd(Map);
        Entry.Map = Map;
        Entry.bPrepared = true;
    }

    // Imported maps: Content/OpenWillow/<Map>/<Map>.umap.
    const FString ContentDir = FPaths::Combine(FPaths::ProjectContentDir(), TEXT("OpenWillow"));
    TArray<FString> MapDirs;
    Files.FindFiles(MapDirs, *FPaths::Combine(ContentDir, TEXT("*")), false, true);
    for (const FString& Map : MapDirs)
    {
        if (!MatchesPersistentName(Map)) continue;
        if (!Files.FileExists(*FPaths::Combine(ContentDir, Map, Map + TEXT(".umap")))) continue;
        FOpenWillowMapEntry& Entry = Found.FindOrAdd(Map);
        Entry.Map = Map;
        Entry.bImported = true;
    }

    TArray<FOpenWillowMapEntry> Entries;
    Found.GenerateValueArray(Entries);
    Entries.Sort([](const FOpenWillowMapEntry& A, const FOpenWillowMapEntry& B) { return A.Map < B.Map; });
    return Entries;
}

void AOpenWillowPlayerController::BeginPlay()
{
    Super::BeginPlay();
    Maps = DiscoverMaps();
    UE_LOG(LogTemp, Display, TEXT("OpenWillow map selector: %d entries (Tab lists them, 1-9 open an imported map)"), Maps.Num());
    if (GEngine)
    {
        GEngine->AddOnScreenDebugMessage(MenuMessageKey, 8.0f, FColor::Yellow, TEXT("Tab: map list"));
    }
}

void AOpenWillowPlayerController::SetupInputComponent()
{
    Super::SetupInputComponent();
    if (!InputComponent) return;
    InputComponent->BindKey(EKeys::Tab, IE_Pressed, this, &AOpenWillowPlayerController::ToggleMenu);
    const FKey Digits[MaxDigitEntries] = {EKeys::One, EKeys::Two, EKeys::Three, EKeys::Four, EKeys::Five,
                                          EKeys::Six, EKeys::Seven, EKeys::Eight, EKeys::Nine};
    for (const FKey& Key : Digits)
    {
        InputComponent->BindKey(Key, IE_Pressed, this, &AOpenWillowPlayerController::OnDigitKey);
    }
}

void AOpenWillowPlayerController::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    if (bMenuOpen) DrawMenu(0.5f);
}

void AOpenWillowPlayerController::ToggleMenu()
{
    bMenuOpen = !bMenuOpen;
    if (bMenuOpen)
    {
        Maps = DiscoverMaps();
    }
    else if (GEngine)
    {
        for (int32 i = 0; i <= MaxDigitEntries; ++i) GEngine->RemoveOnScreenDebugMessage(MenuMessageKey + i);
    }
}

void AOpenWillowPlayerController::OnDigitKey(FKey Key)
{
    if (!bMenuOpen) return;
    static const FKey Digits[MaxDigitEntries] = {EKeys::One, EKeys::Two, EKeys::Three, EKeys::Four, EKeys::Five,
                                                 EKeys::Six, EKeys::Seven, EKeys::Eight, EKeys::Nine};
    for (int32 i = 0; i < MaxDigitEntries; ++i)
    {
        if (Key == Digits[i])
        {
            OpenEntry(i + 1);
            return;
        }
    }
}

void AOpenWillowPlayerController::DrawMenu(float Duration) const
{
    if (!GEngine) return;
    const FString Current = GetWorld() ? GetWorld()->GetMapName() : FString();
    GEngine->AddOnScreenDebugMessage(MenuMessageKey, Duration, FColor::Yellow,
        FString::Printf(TEXT("OpenWillow maps (%d) - digit opens, Tab closes"), Maps.Num()));
    for (int32 i = 0; i < Maps.Num() && i < MaxDigitEntries; ++i)
    {
        const FOpenWillowMapEntry& Entry = Maps[i];
        FString Status = Entry.bImported ? TEXT("imported") : TEXT("not imported");
        if (!Entry.bPrepared) Status += TEXT(", no local scene");
        if (Entry.Map == Current) Status += TEXT(", current");
        GEngine->AddOnScreenDebugMessage(MenuMessageKey + 1 + i, Duration,
            Entry.bImported ? FColor::White : FColor::Silver,
            FString::Printf(TEXT("  %d. %s  [%s]"), i + 1, *Entry.Map, *Status));
    }
}

bool AOpenWillowPlayerController::OpenEntry(int32 Number)
{
    if (Maps.Num() == 0) Maps = DiscoverMaps();
    if (Number < 1 || Number > Maps.Num())
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow map selector: no entry %d"), Number);
        return false;
    }
    const FOpenWillowMapEntry& Entry = Maps[Number - 1];
    if (!Entry.bImported)
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow map selector: %s is not imported; run tools/viewer.py --action import"), *Entry.Map);
        if (GEngine) GEngine->AddOnScreenDebugMessage(MenuMessageKey + 10, 4.0f, FColor::Red, Entry.Map + TEXT(" is not imported"));
        return false;
    }
    if (bOpenRequested) return false;
    bOpenRequested = true;
    bMenuOpen = false;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow map selector: opening %s"), *Entry.LevelPath());
    UGameplayStatics::OpenLevel(this, FName(*Entry.LevelPath()));
    return true;
}

void AOpenWillowPlayerController::OWMapList()
{
    Maps = DiscoverMaps();
    for (int32 i = 0; i < Maps.Num(); ++i)
    {
        UE_LOG(LogTemp, Display, TEXT("OpenWillow map %d: %s prepared=%d imported=%d"),
               i + 1, *Maps[i].Map, Maps[i].bPrepared, Maps[i].bImported);
    }
}

void AOpenWillowPlayerController::OWMapOpen(int32 Number)
{
    OpenEntry(Number);
}
