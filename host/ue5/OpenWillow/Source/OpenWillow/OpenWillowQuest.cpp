#include "OpenWillowQuest.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowAudio.h"
#include "Components/AudioComponent.h"
#include "OpenWillowVendor.h"
#include "Misc/PackageName.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowMover.h"
#include "OpenWillowNpc.h"
#include "OpenWillowFocusCam.h"
#include "OpenWillowProjectile.h"
#include "OpenWillowSkills.h"
#include "OpenWillowSliceData.h"
#include "OpenWillowWalker.h"
#include "OpenWillowWeaponGen.h"
#include "slice.hpp"
#include "Camera/CameraComponent.h"
#include "Components/BoxComponent.h"
#include "Components/CapsuleComponent.h"
#include "Engine/Engine.h"
#include "Engine/GameViewportClient.h"
#include "Engine/World.h"
#include "EngineUtils.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "GameFramework/PlayerController.h"
#include "HAL/PlatformMisc.h"
#include "InputKeyEventArgs.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/CommandLine.h"
#include "OpenWillowMissionPage.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"
#include "UnrealClient.h"
#include <algorithm>
#include <filesystem>
#include <stdexcept>

namespace {
const char* const MissionPath = "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire";
const char* const DummyProvider = "GD_TargetDummy.Character.CharClass_TargetDummy.BehaviorProviderDefinition_5";

// JSON text of an object. The test compares a saved block with a freshly built one this way: both come from
// UOpenWillowSkills::ProgressionJson, so equal fields give equal text.
FString JsonText(const TSharedPtr<FJsonObject>& Object)
{
    FString Text;
    if (Object) FJsonSerializer::Serialize(Object.ToSharedRef(), TJsonWriterFactory<>::Create(&Text));
    return Text;
}
}

struct UOpenWillowQuest::FImpl {
    PackageStore Store;
    vm::Runtime Runtime;
    std::unique_ptr<vm::FireMissionSlice> Slice;
    std::set<std::string> Completed;
    FOpenWillowSliceData Data;
    TArray<int32> StationCounters;     // GetNextExitPoint round robin, per station
    explicit FImpl(const std::filesystem::path& Cooked) : Store(Cooked), Runtime(Store) {
        Runtime.registerCoreNatives();
        Slice = std::make_unique<vm::FireMissionSlice>(Runtime, MissionPath, "Sanctuary_Dynamic", DummyProvider);
        // FIXTURE standing in for save state, not satisfied from data. A real save carries the completed missions;
        // nothing installed says which missions are complete for a player in Sanctuary (the level travel station into
        // Sanctuary already opens while the dependency mission is still active, and the fast-forward that completes
        // missions in bulk is native, with its trigger set at run time). The fixture marks the mission's own declared
        // Dependencies complete, read from the definition, so no mission is named here.
        for (const auto& Dependency : Slice->mission().dependencies()) Completed.insert(Dependency);
        Fixture = Completed;
    }
    std::set<std::string> Fixture;     // what the fixture added (checked by the suite)
    std::string ObjectiveState(const FString& Objective) const {
        return Slice->mission().objectiveState(TCHAR_TO_UTF8(*Objective));
    }
};

UOpenWillowQuest::UOpenWillowQuest()
{
    PrimaryComponentTick.bCanEverTick = true;
}

void UOpenWillowQuest::BeginPlay()
{
    Super::BeginPlay();
    // After the walker's tick, which sets the camera's FOV each frame: the page's view (TickFocusCamera) goes on top of it.
    if (AActor* Owner = GetOwner()) AddTickPrerequisiteActor(Owner);
    bEnabled = FParse::Param(FCommandLine::Get(), TEXT("owquest"));
    bTesting = FParse::Param(FCommandLine::Get(), TEXT("owquesttest"));
    bResume = FParse::Param(FCommandLine::Get(), TEXT("owquestresume"));
    // -owweapongentest: the weapon generator's host self-check (local/weapons/gen/host_check); runs with or without the quest.
    if (FParse::Param(FCommandLine::Get(), TEXT("owweapongentest")))
    {
        FString CheckItems;
        FParse::Value(FCommandLine::Get(), TEXT("owitems="), CheckItems);
        FOpenWillowWeaponGen::RunSelfCheck(CheckItems.IsEmpty() ? FString() : FOpenWillowWeaponGen::RewardSource(CheckItems));
    }
    if (!bEnabled) { SetComponentTickEnabled(false); return; }
    try {
        FString Save, World, Npcs, Audio;
        if (!FParse::Value(FCommandLine::Get(), TEXT("owquestsave="), Save) || Save.IsEmpty())
            throw std::runtime_error("-owquest needs -owquestsave=<file under local/>");
        if (!FParse::Value(FCommandLine::Get(), TEXT("owslice="), World) || !FParse::Value(FCommandLine::Get(), TEXT("ownpcs="), Npcs)
            || !FParse::Value(FCommandLine::Get(), TEXT("owaudio="), Audio))
            throw std::runtime_error("-owquest needs -owslice=<world.json> -ownpcs=<npc_assets.json> -owaudio=<audio.json>");
        SavePath = Save;
        const FString Game = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_BL2"));
        if (!FPaths::FileExists(FPaths::Combine(Game, TEXT("Binaries/Win32/Borderlands2.exe"))))
            throw std::runtime_error("installed Borderlands 2 is required");
        Impl = MakeShared<FImpl>(std::filesystem::path(*FPaths::Combine(Game, TEXT("WillowGame/CookedPCConsole"))));
        Impl->Data.Load(World, Npcs, Audio);
        for (const auto& Path : Impl->Fixture)
            UE_LOG(LogTemp, Display, TEXT("OWQUEST FIXTURE (save-state stand-in, not stock data): dependency %hs treated as complete"), Path.c_str());
        Impl->StationCounters.Init(0, Impl->Data.Stations.Num());
        {
            // Marcus can talk: the most common talker name tag of the manifest's dialog entries (HOST CHOICE; the quest has one giver).
            // Without a registered pawn the stock dialog would use an echo caller.
            TMap<FString, int32> Counts;
            for (const auto& Pair : Impl->Data.Audio) {
                const TSharedPtr<FJsonObject>* Trigger = nullptr;
                FString NameTag;
                if (Pair.Key.StartsWith(TEXT("dialog:")) && Pair.Value->TryGetObjectField(TEXT("trigger"), Trigger) && (*Trigger)->TryGetStringField(TEXT("name_tag"), NameTag) && !NameTag.IsEmpty())
                    ++Counts.FindOrAdd(NameTag);
            }
            FString Best;
            for (const auto& Pair : Counts) if (Best.IsEmpty() || Pair.Value > Counts[Best]) Best = Pair.Key;
            if (!Best.IsEmpty()) {
                Impl->Slice->mission().dialog().registerTalker(TCHAR_TO_UTF8(*Best));
                RegisteredTalker = Best;
                UE_LOG(LogTemp, Display, TEXT("OWQUEST dialog talker registered: %s (host choice from the audio manifest)"), *Best);
            }
        }
        if (OpenWillowAudio::Enabled()) {
            // With sound on, the host is the stock dialog's audio device (SLICE_AUDIO_CHAIN.md section 5c). The VM has already chosen the line (event,
            // talk act, talker, priority); the host plays the play map's "line:<event tag>" and gives back its file's length, and the dialog's own clock
            // ends the line at that length (plus the act's OutputDelay), so the next behavior waits for it as live (FIRE_MISSION_GOLDEN_TRACE.md, lane L4).
            // A line with no file in the map is not played (-1) and ends on the next update. Without -owaudioplay no player is set and nothing here changes.
            Impl->Slice->mission().dialog().setTimedLinePlayer([this](const vm::DialogSystem::Line& Line) {
                float Seconds = 0.f;
                TWeakObjectPtr<UAudioComponent> Sound;
                // A line the VM gave to the registered talker comes from Marcus's pawn (section 5d); an echo caller or another talker stays 2D.
                USceneComponent* From = !Line.echo && Marcus && RegisteredTalker == UTF8_TO_TCHAR(Line.talker.c_str()) ? Marcus->GetRootComponent() : nullptr;
                if (!OpenWillowAudio::Play(GetOwner(), *(TEXT("line:") + FString(UTF8_TO_TCHAR(Line.eventTag.c_str()))), &Seconds, &Sound, From)) return -1.0;
                // Only sounds still playing are kept: a line that ended by itself drops out here, so the map stays as small as what is sounding.
                for (auto It = LineSounds.CreateIterator(); It; ++It)
                    if (!It.Value().IsValid() || !It.Value()->IsPlaying()) It.RemoveCurrent();
                LineSounds.Add(Line.id, Sound);
                return double(Seconds);
            });
            // A line the dialog stops while it plays (the talker's next line, a group silence: StopTalking) loses its sound too, as the stock
            // component's StopTalking stops its audio (NATIVE_DIALOG.md "Talk" step 5). Without this an interrupted line kept sounding to its end.
            Impl->Slice->mission().dialog().onLineStopped = [this](int LineId) {
                TWeakObjectPtr<UAudioComponent> Sound;
                if (LineSounds.RemoveAndCopyValue(LineId, Sound) && Sound.IsValid()) {
                    Sound->Stop();
                    UE_LOG(LogTemp, Display, TEXT("OWQUEST dialog line %d stopped by the dialog while it played; its sound is stopped"), LineId);
                }
            };
            UE_LOG(LogTemp, Display, TEXT("OWQUEST dialog line player set: lines play from the audio play map and last as long as their files"));
        }
        // Slice gear (tools/weapon_slice_gear.py): the recipe of the mission's own MissionWeapon and the level the
        // gear was rolled at (an UNVERIFIED slice choice; the mission level is the region stage, see FixRegionStage).
        if (!FParse::Value(FCommandLine::Get(), TEXT("owitems="), ItemDir) || ItemDir.IsEmpty())
            throw std::runtime_error("-owquest needs -owitems=<local/items/slice> (tools/weapon_slice_gear.py)");
        const FString WeaponDefinition = UTF8_TO_TCHAR(Impl->Slice->mission().weaponDefinition().c_str());
        if (!UOpenWillowInventory::FindRecipe(ItemDir, TEXT("mission_weapon"), WeaponDefinition, MissionWeapon))
            throw std::runtime_error(TCHAR_TO_UTF8(*FString::Printf(TEXT("no mission-weapon recipe for %s under %s"), *WeaponDefinition, *ItemDir)));
        // The corrected mesh of the recipe's own parts (tools/weapon_refresh_fragments.py, host/ue5/import_gun_meshes.py); the NPC lane's
        // rolled sample (Data.PistolMesh) stays the fallback where that asset has not been imported.
        {
            const FString OwnMesh = FString::Printf(TEXT("/Game/OpenWillow/Weapons/SliceItems/SK_%s.SK_%s"), *MissionWeapon.Id, *MissionWeapon.Id);
            MissionWeapon.MeshPath = FPackageName::DoesPackageExist(OwnMesh.Left(OwnMesh.Find(TEXT(".")))) ? OwnMesh : Impl->Data.PistolMesh;
        }
        FString GearText;
        TSharedPtr<FJsonObject> Gear;
        if (!FFileHelper::LoadFileToString(GearText, *FPaths::Combine(ItemDir, TEXT("slice_manifest.json")))
            || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(GearText), Gear) || !Gear || !Gear->TryGetNumberField(TEXT("level"), GearLevel))
            throw std::runtime_error("slice_manifest.json (level) missing under -owitems");
        UE_LOG(LogTemp, Display, TEXT("OWQUEST mission weapon %s -> recipe %s \"%s\" level %d, %.2f damage, %.2f/s, magazine %.2f, damage type %s, mesh %s; gear level %d (slice choice, UNVERIFIED)"),
            *WeaponDefinition, *MissionWeapon.Id, *MissionWeapon.Name, MissionWeapon.Level, MissionWeapon.Damage, MissionWeapon.FireRate,
            MissionWeapon.Magazine, *MissionWeapon.DamageType, *MissionWeapon.MeshPath, GearLevel);
        // No turn-in loot: the stock data gives this mission no item reward (DECISIONS.md 2026-10-07); the weapon generator's
        // reward roll from main (2026-10-23) is therefore not wired here. The generator stays available (-owweapongentest).
        const auto& D = Impl->Data;
        TArray<FString> Overrides;
        for (const auto& O : D.StageOverrides) Overrides.Add(FString::Printf(TEXT("%s %d..%d"), *O.Mission, O.Min, O.Max));
        UE_LOG(LogTemp, Display, TEXT("OWQUEST region stage table %s (%s): default %d..%d, boost %d, %d overrides [%s]"), *D.StageRegion,
            D.bRegionStageFromData ? TEXT("from world.json values.xp.region_stage") : TEXT("STAND-IN bounds, manifest has no region_stage"),
            D.StageDefaultMin, D.StageDefaultMax, D.StageBoost, D.StageOverrides.Num(), *FString::Join(Overrides, TEXT(", ")));
        // Both world-data objectives must belong to the installed mission.
        if (Impl->ObjectiveState(Impl->Data.TriggerObjective).empty() || Impl->ObjectiveState(Impl->Data.DummyObjective).empty())
            throw std::runtime_error("slice manifest objectives are not objectives of the installed mission");
        FString Text;
        if (FFileHelper::LoadFileToString(Text, *SavePath)) {
            TSharedPtr<FJsonObject> Data;
            if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Data) || !Data)
                throw std::runtime_error("quest save is not valid JSON");
            for (const auto& Value : Data->GetArrayField(TEXT("completed"))) Impl->Completed.insert(TCHAR_TO_UTF8(*Value->AsString()));
            if (!Impl->Slice->loadState(TCHAR_TO_UTF8(*Data->GetStringField(TEXT("state")))))
                throw std::runtime_error("quest save state was rejected");
            Rewards = int32(Data->GetNumberField(TEXT("rewards")));
            Respawns = int32(Data->GetNumberField(TEXT("respawns")));
            // A lend in progress is redone on the first tick, once the walker has set up its inventory.
            bLendPending = Data->GetBoolField(TEXT("weapon_lent"));
            // Maya's progression (added 2026-10-01; older saves have no block). Applied by the walker through
            // RestoreProgression once her skill tree and start level are set.
            const TSharedPtr<FJsonObject>* Progression = nullptr;
            if (Data->TryGetObjectField(TEXT("progression"), Progression)) SavedProgression = *Progression;
            // The region stage fixed in an earlier session (added 2026-10-02; older saves have none and fix it anew).
            const TSharedPtr<FJsonObject>* Stage = nullptr;
            if (Data->TryGetObjectField(TEXT("region_stage"), Stage)) {
                FString Region;
                int32 Value = 0, Level = 0;
                if (!(*Stage)->TryGetStringField(TEXT("region"), Region) || Region != Impl->Data.StageRegion
                    || !(*Stage)->TryGetNumberField(TEXT("stage"), Value) || !(*Stage)->TryGetNumberField(TEXT("player_level"), Level) || Value < 1)
                    throw std::runtime_error("quest save region_stage is not a stage of the mission's region");
                RegionStage = Value;
                RegionStageLevel = Level;
                bRegionStageFromSave = true;
                UE_LOG(LogTemp, Display, TEXT("OWQUEST region stage from save: %s stage %d (fixed at player level %d)"), *Region, Value, Level);
            }
            UE_LOG(LogTemp, Display, TEXT("OWQUEST loaded save: status=%d rewards=%d respawns=%d progression=%s"), Status(), Rewards, Respawns,
                SavedProgression ? TEXT("yes") : TEXT("none (older save)"));
        }
        SpawnMarcus();
        UE_LOG(LogTemp, Display, TEXT("OWQUEST READY mission=%hs status=%d trigger=%s r=%.2f hh=%.2f walk_nodes=%d stations=%d audio_entries=%d"),
            MissionPath, Status(), *Impl->Data.TriggerObject, Impl->Data.TriggerRadius, Impl->Data.TriggerHalfHeight,
            Impl->Data.Walk.Num(), Impl->Data.Stations.Num(), Impl->Data.Audio.Num());
    } catch (const std::exception& Error) { Fail(UTF8_TO_TCHAR(Error.what())); }
}

void UOpenWillowQuest::EndPlay(const EEndPlayReason::Type Reason)
{
    if (ScreenPage) CloseScreenPage(TEXT("end of play"));
    ScreenPage.Reset();
    UE_LOG(LogTemp, Display, TEXT("OWQUEST save calls=%d writes=%d write time %.1f ms total (%.3f ms per write)"),
        SaveCalls, SaveWrites, SaveWriteSeconds * 1000., SaveWrites ? SaveWriteSeconds * 1000. / SaveWrites : 0.);
    Impl.Reset();
    Super::EndPlay(Reason);
}

int32 UOpenWillowQuest::Status() const
{
    return Impl ? int32(Impl->Slice->mission().status()) : -1;
}

void UOpenWillowQuest::MinimapMarkers(TArray<TPair<FString, FVector>>& Out) const
{
    if (!Impl) return;
    using S = vm::MissionSystem::Status;
    const S Current = Impl->Slice->mission().status();
    if (Marcus && Current == S::NotStarted) Out.Emplace(TEXT("eligible"), Marcus->GetActorLocation());
    if (Marcus && Current == S::ReadyToTurnIn) Out.Emplace(TEXT("turnin"), Marcus->GetActorLocation());
    if (Current != S::Active) return;
    if (Impl->ObjectiveState(Impl->Data.TriggerObjective) == "Active") Out.Emplace(TEXT("objective"), Impl->Data.TriggerCenter);
    else if (Dummy && Impl->ObjectiveState(Impl->Data.DummyObjective) == "Active") Out.Emplace(TEXT("objective"), Dummy->GetActorLocation());
}

FString UOpenWillowQuest::MissionLogJson() const
{
    TSharedRef<FJsonObject> Log = MakeShared<FJsonObject>();
    Log->SetBoolField(TEXT("enabled"), Impl.IsValid());
    TArray<TSharedPtr<FJsonValue>> Missions;
    if (Impl)
    {
        using S = vm::MissionSystem::Status;
        const vm::MissionSystem& Mission = Impl->Slice->mission();
        const S Current = Mission.status();
        static const TCHAR* const Names[] = {TEXT("notstarted"), TEXT("active"), TEXT("turnin"), TEXT("complete")};
        TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
        Entry->SetStringField(TEXT("id"), UTF8_TO_TCHAR(Mission.path().c_str()));
        Entry->SetStringField(TEXT("name"), UTF8_TO_TCHAR(Mission.name().c_str()));
        Entry->SetStringField(TEXT("status"), Names[int32(Current)]);
        Entry->SetBoolField(TEXT("plot"), Mission.plotCritical());
        Entry->SetStringField(TEXT("description"), UTF8_TO_TCHAR(Mission.description().c_str()));
        Entry->SetStringField(TEXT("summary"), UTF8_TO_TCHAR(Mission.summary().c_str()));
        Entry->SetStringField(TEXT("turnInDescription"), UTF8_TO_TCHAR(Mission.turnInDescription().c_str()));
        Entry->SetStringField(TEXT("giver"), UTF8_TO_TCHAR(Mission.giver().c_str()));
        Entry->SetStringField(TEXT("turnIn"), UTF8_TO_TCHAR(Mission.turnIn().c_str()));
        // The mission level is the region stage once it is fixed (FixRegionStage); its XP reward the slice data's formula (MissionXp).
        Entry->SetNumberField(TEXT("level"), RegionStage);
        Entry->SetNumberField(TEXT("xp"), RegionStage > 0 ? Impl->Data.MissionXp(RegionStage) : 0);
        // No "credits" and no reward items: the host does not evaluate MissionDefinition.GetCurrencyReward's formula or the
        // reward presentations (UNVERIFIED); the page hides those panels while the values are unknown.
        Entry->SetBoolField(TEXT("tracked"), Current == S::Active || Current == S::ReadyToTurnIn);
        TArray<TSharedPtr<FJsonValue>> Objectives;
        // The same objective view as the HUD's tracker (MissionSystem::trackerLines), plus each objective's progress count.
        for (const auto& Line : Mission.trackerLines())
        {
            TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
            Row->SetStringField(TEXT("name"), UTF8_TO_TCHAR(Line.name.c_str()));
            Row->SetStringField(TEXT("text"), UTF8_TO_TCHAR(Line.message.c_str()));
            Row->SetNumberField(TEXT("progress"), Mission.objectiveProgress(Line.name));
            Row->SetNumberField(TEXT("count"), Line.count);
            Row->SetBoolField(TEXT("done"), Line.complete);
            Row->SetBoolField(TEXT("optional"), Line.optional);
            Objectives.Add(MakeShared<FJsonValueObject>(Row));
        }
        Entry->SetArrayField(TEXT("objectives"), Objectives);
        Missions.Add(MakeShared<FJsonValueObject>(Entry));
    }
    Log->SetArrayField(TEXT("missions"), Missions);
    const auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    if (const UOpenWillowSkills* Skills = Walker ? Walker->GetSkills() : nullptr)
    {
        TSharedRef<FJsonObject> Player = MakeShared<FJsonObject>();
        const int32 Level = Skills->GetLevel();
        Player->SetNumberField(TEXT("level"), Level);
        Player->SetNumberField(TEXT("xp"), double(Skills->GetExperience()));
        Player->SetNumberField(TEXT("xpLevelStart"), double(UOpenWillowSkills::ExperienceForLevel(Level)));
        Player->SetNumberField(TEXT("xpNext"), double(UOpenWillowSkills::ExperienceForLevel(Level + 1)));
        Player->SetNumberField(TEXT("maxLevel"), UOpenWillowSkills::MaxLevel);
        int32 Money = 0;
        if (Walker->GetInventory() && Walker->GetInventory()->GetMoney(Money)) Player->SetNumberField(TEXT("credits"), Money);
        Log->SetObjectField(TEXT("player"), Player);
    }
    FString Text;
    FJsonSerializer::Serialize(Log, TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Text));
    return Text;
}

void UOpenWillowQuest::Fail(const FString& Error)
{
    bFailed = true;
    UE_LOG(LogTemp, Error, TEXT("OWQUEST ERROR %s"), *Error);
    // A failed quest stops ticking the page and its keys, so a page left open would stay for good, over a zoomed view: close it now.
    CloseScreenPage(TEXT("the quest failed"));
    if (bTesting) {
        UE_LOG(LogTemp, Display, TEXT("OWQUESTTEST SUMMARY result=FAIL checks=%d errors=%d reason=%s"), Checks, ++Errors, *Error);
        FPlatformMisc::RequestExit(false);
    }
}

void UOpenWillowQuest::Check(bool bGood, const TCHAR* Name)
{
    ++Checks;
    if (!bGood) ++Errors;
    UE_LOG(LogTemp, Display, TEXT("OWQUESTTEST check=%d name=%s ok=%d"), Checks, Name, bGood);
}

void UOpenWillowQuest::Save()
{
    if (!Impl || SavePath.IsEmpty()) return;
    TSharedRef<FJsonObject> Data = MakeShared<FJsonObject>();
    TArray<TSharedPtr<FJsonValue>> Completed;
    for (const auto& Path : Impl->Completed) Completed.Add(MakeShared<FJsonValueString>(UTF8_TO_TCHAR(Path.c_str())));
    Data->SetArrayField(TEXT("completed"), Completed);
    Data->SetStringField(TEXT("state"), UTF8_TO_TCHAR(Impl->Slice->saveState().c_str()));
    Data->SetNumberField(TEXT("rewards"), Rewards);
    Data->SetNumberField(TEXT("respawns"), Respawns);
    Data->SetBoolField(TEXT("weapon_lent"), bWeaponLent);
    // Maya's progression. Without -owmaya the skills component was never set up, so the loaded block is kept as is.
    const auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    if (Walker && Walker->IsMayaActive() && Walker->GetSkills()) Data->SetObjectField(TEXT("progression"), Walker->GetSkills()->ProgressionJson());
    else if (SavedProgression) Data->SetObjectField(TEXT("progression"), SavedProgression);
    // The region stage stays fixed for this player and playthrough once computed (NATIVE_PROGRESSION.md section 2).
    if (RegionStage > 0) {
        TSharedRef<FJsonObject> Stage = MakeShared<FJsonObject>();
        Stage->SetStringField(TEXT("region"), Impl->Data.StageRegion);
        Stage->SetNumberField(TEXT("playthrough"), 1);
        Stage->SetNumberField(TEXT("stage"), RegionStage);
        Stage->SetNumberField(TEXT("player_level"), RegionStageLevel);
        Data->SetObjectField(TEXT("region_stage"), Stage);
    }
    FString Text;
    FJsonSerializer::Serialize(Data, TJsonWriterFactory<>::Create(&Text));
    ++SaveCalls;
    if (Text == LastSavedText) return;   // unchanged since the last write
    const double Started = FPlatformTime::Seconds();
    IFileManager::Get().MakeDirectory(*FPaths::GetPath(SavePath), true);
    if (FFileHelper::SaveStringToFile(Text, *SavePath)) LastSavedText = Text;
    else UE_LOG(LogTemp, Error, TEXT("OWQUEST could not write %s"), *SavePath);
    ++SaveWrites;
    SaveWriteSeconds += FPlatformTime::Seconds() - Started;
}

// ------------------------------------------------------------------------------------------- world actors

void UOpenWillowQuest::SpawnMarcus()
{
    const auto& Data = Impl->Data;
    FActorSpawnParameters Params;
    Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
    Marcus = GetWorld()->SpawnActor<AOpenWillowNpc>(Data.MarcusLocation, Data.MarcusRotation, Params);
    if (!Marcus || !Marcus->Setup(Data.Marcus.Mesh, Data.Marcus.Idle, Data.Marcus.Walk, Data.Marcus.MeshOffset))
        throw std::runtime_error("Marcus could not be spawned with the imported assets (npc_assets.json)");
    UE_LOG(LogTemp, Display, TEXT("OWQUEST Marcus placed at %s yaw %.2f (WillowAIPawn_13 stock pose)"),
        *Data.MarcusLocation.ToString(), Data.MarcusRotation.Yaw);
}

void UOpenWillowQuest::SpawnDummy()
{
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    UOpenWillowMover* Mover = Walker ? Walker->GetMover() : nullptr;
    if (!Mover) { Fail(TEXT("the dummy needs the mover's sequence")); return; }
    const auto& Data = Impl->Data;
    bDummySpawned = true;   // MaxTotalActors 1 for this den; no respawn of the dummy in the host
    FActorSpawnParameters Params;
    Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
    Dummy = GetWorld()->SpawnActor<AOpenWillowCombatTarget>(Data.DummyLocation, Data.DummyRotation, Params);
    if (!Dummy || !Dummy->UseStockPawn(Data.Dummy.Mesh, Data.Dummy.Idle, Data.Dummy.Death, Data.Dummy.MeshOffset)) {
        Fail(TEXT("the stock target dummy could not be spawned with the imported assets"));
        return;
    }
    DummySpawnedAt = Dummy->GetActorLocation();
    UE_LOG(LogTemp, Display, TEXT("OWQUEST dummy spawned at %s (population point; spawn trigger UNVERIFIED)"), *DummySpawnedAt.ToString());
    // The sequence's populated events for this den and spawn point; their Instigator variables now hold the dummy.
    // Order (populated events, then the pawn's OnSpawned) is a host choice (UNVERIFIED).
    for (const FString& Object : {Data.DenObject, Data.PointObject}) {
        TArray<FString> Entered;
        Mover->OriginatorEvent(Object, Entered);
        for (const FString& Event : Entered)
            for (const auto& Variable : Mover->SequenceVariables(Event, TEXT("Instigator")))
                DummyVariables.Add(UTF8_TO_TCHAR(Variable.name.c_str()));
        UE_LOG(LogTemp, Display, TEXT("OWQUEST populated events for %s: %s"), *Object, *FString::Join(Entered, TEXT(", ")));
    }
    RouteRequests();
    Impl->Slice->spawnDummy();   // the dummy provider's OnSpawned
    Pump();
}

void UOpenWillowQuest::ProcessArrivals()
{
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    if (!Marcus || !Walker || !Walker->GetMover()) return;
    for (const int32 Index : Marcus->TakeArrivals()) {
        const auto& Node = Impl->Data.Walk[Index];
        UE_LOG(LogTemp, Display, TEXT("OWQUEST Marcus arrived at %s (%d/%d)"), *Node.Name, Index + 1, Impl->Data.Walk.Num());
        for (const FString& Event : Node.ArrivalEvents) {
            Walker->GetMover()->SequenceEvent(Event);
            ArrivalMotions.Add(Walker->GetMover()->LastEventMotion);
        }
        if (Index == Impl->Data.Walk.Num() - 1) Walker->GetMover()->SequenceOutput(Impl->Data.WalkOp, TEXT("Finished"));
    }
    RouteRequests();
}

// World ops the installed sequence reached (via the door mover's sequence instance). The ones bound here run;
// the rest are logged as not run.
void UOpenWillowQuest::RouteRequests()
{
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    UOpenWillowMover* Mover = Walker ? Walker->GetMover() : nullptr;
    if (!Mover || !Impl) return;
    const auto& Data = Impl->Data;
    auto Holds = [this, Mover](const FString& Op, const TCHAR* Link) {
        for (const auto& Variable : Mover->SequenceVariables(Op, Link))
            if (DummyVariables.Contains(UTF8_TO_TCHAR(Variable.name.c_str()))) return true;
        return false;
    };
    for (int32 Round = 0; Round < 16; ++Round) {
        const TArray<FOpenWillowKismetRequest> Requests = Mover->DrainRequests();
        if (Requests.Num() == 0) return;
        for (const auto& R : Requests) {
            bool bRun = false;
            if (R.Class == TEXT("WillowGame.WillowSeqAct_AIScripted") && R.Op == Data.WalkOp && R.Input == TEXT("In") && Marcus) {
                TArray<AOpenWillowNpc::FNode> Nodes;
                for (const auto& Node : Data.Walk) Nodes.Add({Node.Location, Node.ArrivalRadius, Node.SpeedPercentage});
                Marcus->StartWalk(Nodes, Data.MarcusSpeed, Data.MarcusYawRate);
                bWalkStarted = bRun = true;
            } else if (R.Class == TEXT("Engine.SeqAct_ApplyBehavior") && Data.LookAtPlayerOps.Contains(R.Op) && Marcus) {
                Marcus->SetLookAt(Walker);   // Behavior_SetAIFlag Flag_LookAtPlayer = true
                bLookAtFromSequence = bRun = true;
            } else if (R.Class == TEXT("Engine.SeqAct_SetPhysics") && Holds(R.Op, TEXT("Target"))) {
                // No NewPhysics is tagged (class default); the host leaves the dummy as spawned (UNVERIFIED).
                bRun = true;
            } else if (R.Class == TEXT("Engine.SeqAct_AttachToActor") && Holds(R.Op, TEXT("Attachment")) && Dummy) {
                bool bHolder = false;
                for (const auto& Variable : Mover->SequenceVariables(R.Op, TEXT("Target")))
                    bHolder |= UTF8_TO_TCHAR(Variable.object.c_str()) == Data.HolderObject;
                // The holder (an interactive object: static meshes and SocketComponents, no skeleton) rides on the target
                // carrier. The op's BoneName resolves to the holder's SocketComponent of that name (world.json
                // holder.attach). The op tags no relative offset or rotation, so the dummy's origin is put on the socket:
                // UNVERIFIED (Activated is native; bUseConstructAttachment is not interpreted). The carrier only
                // translates in this binding (constant rotation keys), so the holder now = placed pose + carrier offset.
                if (bHolder && Mover->TrackCarrier()) {
                    AttachCarrierOffset = Mover->TrackOffset();
                    if (Data.bHasAttachSocket && R.Op == Data.AttachOp) {
                        FTransform Socket = Data.AttachSocketLocal * Data.HolderPose;
                        Socket.AddToTranslation(AttachCarrierOffset);
                        Dummy->SetActorLocationAndRotation(Socket.GetLocation(), Socket.GetRotation(), false, nullptr, ETeleportType::TeleportPhysics);
                        bAttachSocketApplied = true;
                        UE_LOG(LogTemp, Display, TEXT("OWQUEST dummy put on the holder socket at %s rot %s (spawned at %s; socket reading UNVERIFIED)"),
                            *Socket.GetLocation().ToString(), *Socket.Rotator().ToString(), *DummySpawnedAt.ToString());
                    } else {
                        UE_LOG(LogTemp, Warning, TEXT("OWQUEST no attach socket for %s in world.json (regenerate with tools/prepare_slice_world.py): the dummy keeps its spawn pose"), *R.Op);
                    }
                    Dummy->AttachToComponent(Mover->TrackCarrier(), FAttachmentTransformRules::KeepWorldTransform);
                    DummyAttachedAt = Dummy->GetActorLocation();
                    bDummyAttached = bRun = true;
                }
            } else if (R.Class == TEXT("Engine.SeqAct_Destroy") && Holds(R.Op, TEXT("Target")) && Dummy) {
                Dummy->Destroy();
                Dummy = nullptr;
                bDummyDestroyedBySequence = bRun = true;
            }
            UE_LOG(LogTemp, Display, TEXT("OWQUEST kismet %s:%s <- %s %s"), *R.Class, *R.Op, *R.Input,
                bRun ? TEXT("(run by host)") : TEXT("(host boundary, not run)"));
            if (bRun) Mover->SequenceOutput(R.Op, TEXT("Out"));
        }
    }
    Fail(TEXT("kismet request routing did not settle"));
}

// ------------------------------------------------------------------------------------------- mission

void UOpenWillowQuest::Pump()
{
    using K = vm::FireMissionSlice::HostEvent::Kind;
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    UOpenWillowMover* Mover = Walker ? Walker->GetMover() : nullptr;
    for (const auto& Event : Impl->Slice->drain()) {
        const FString A = UTF8_TO_TCHAR(Event.a.c_str());
        ++PumpEvents;
        switch (Event.kind) {
        case K::RemoteEvent:
            // Mission behaviors name their mission (WillowSeqEvent_MissionRemoteEvent); the dummy provider's
            // Behavior_RemoteEvent does not (plain SeqEvent_RemoteEvent).
            if (Mover) {
                if (Event.b.empty()) Mover->RemoteEvent(A);
                else Mover->MissionEvent(UTF8_TO_TCHAR(Event.b.c_str()), A);
                MissionEventsMatched += Event.b.empty() ? 0 : Mover->LastEventMatched;
                if (Event.b.empty() && A == TEXT("RocksPaper_SendTargetBack")) bSendBackFromProvider = Mover->LastEventMatched > 0;
            }
            UE_LOG(LogTemp, Display, TEXT("OWQUEST remote event %s from %s (kismet matched=%d)"), *A,
                Event.b.empty() ? TEXT("dummy provider") : TEXT("mission"), Mover ? Mover->LastEventMatched : -1);
            break;
        case K::Dialog: {
            if (A == LastDismissalTag) {
                // The line behind the dismissal tag (the generic group's act, spoken by another NPC in the real game: FIRE_MISSION_GOLDEN_TRACE.md). The audio manifest
                // is built from the mission's own steps and does not hold it, so it is reported here and neither counted as a lookup nor as a miss; nothing plays.
                UE_LOG(LogTemp, Display, TEXT("OWQUEST dismissal line %s (%hs; not in the audio manifest, not played)"), *A, Event.detail.c_str());
                break;
            }
            // A line the stock dialog chose (NATIVE_DIALOG.md, UNVERIFIED): event tag, the talk act's AkEvent, the talker and what became of
            // it. Without -owaudioplay no audio device is modelled ("no audio device"); with it the line player above plays it.
            ++DialogLookups;
            auto Field = [&Event](const char* Key) {
                const FString Detail = UTF8_TO_TCHAR(Event.detail.c_str());
                TArray<FString> Parts;
                Detail.ParseIntoArray(Parts, TEXT(";"));
                for (const FString& Part : Parts) if (Part.StartsWith(FString(Key) + TEXT("="))) return Part.Mid(FCString::Strlen(UTF8_TO_TCHAR(Key)) + 1);
                return FString();
            };
            const FString Ak = Field("ak"), Outcome = Field("outcome");
            const TSharedPtr<FJsonObject>* Entry = Impl->Data.Audio.Find(TEXT("dialog:") + A);
            if (!Entry) Entry = Impl->Data.Audio.Find(TEXT("marcus_group:") + A);      // Marcus's own group (the on-use barks)
            if (A.StartsWith(TEXT("GD_Dialog_NPCImplementation."))) { ++OnUseLines; LastOnUseLineTag = A; LastOnUseLineAk = Ak; }
            if (!Entry) {
                ++DialogMisses;
                UE_LOG(LogTemp, Warning, TEXT("OWQUEST dialog %s: no audio manifest entry"), *A);
                break;
            }
            // The VM's chosen AkEvent against the manifest's (derived separately from the data): they must name the same event.
            const TSharedPtr<FJsonObject>* ManifestAk = nullptr;
            FString ManifestPath;
            if ((*Entry)->TryGetObjectField(TEXT("ak_event"), ManifestAk)) (*ManifestAk)->TryGetStringField(TEXT("ue3_path"), ManifestPath);
            if (Ak != ManifestPath) { ++DialogAkMismatches; UE_LOG(LogTemp, Warning, TEXT("OWQUEST dialog %s: VM AkEvent %s differs from the manifest's %s"), *A, *Ak, *ManifestPath); }
            TArray<FString> Sources;
            for (const auto& Media : (*Entry)->GetArrayField(TEXT("media")))
                Sources.Add(FString::Printf(TEXT("%.0f"), Media->AsObject()->GetNumberField(TEXT("source"))));
            const FString State = (*Entry)->GetStringField(TEXT("state"));
            // Sound off: the same text as before the line player existed, whatever the outcome. Sound on: "started" means the host's line player
            // played it (section 5c); "not playing" means the play map has no file for it.
            if (Outcome == TEXT("started")) ++DialogPlayed;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST dialog line %s ak=%s talker=%hs (%s) outcome=%s -> source %s state=%s (%s)"),
                *A, *Ak, Event.c.c_str(), *Field("talker"), *Outcome, *FString::Join(Sources, TEXT(",")), *State,
                !OpenWillowAudio::Enabled() ? TEXT("not played: no audio device")
                    : Outcome == TEXT("started") ? TEXT("played by the host")
                    : Outcome == TEXT("not playing") ? TEXT("not played: no file in the audio play map") : TEXT("not played"));
            break;
        }
        case K::StatusEffect: UE_LOG(LogTemp, Display, TEXT("OWQUEST status effect %s on dummy (not applied: no effect system)"), *A); break;
        case K::MissionWeaponGranted:
            if (A != MissionWeapon.Balance) { Fail(TEXT("granted mission weapon has no prepared recipe: ") + A); return; }
            UE_LOG(LogTemp, Display, TEXT("OWQUEST mission weapon granted: %s"), *A);
            LendMissionWeapon();
            break;
        case K::MissionWeaponRemoved:
            bWeaponLent = false;
            if (Walker && !Walker->ReturnLentWeapon(MissionWeapon.Id))
                UE_LOG(LogTemp, Warning, TEXT("OWQUEST lent weapon %s was not in Maya's inventory"), *MissionWeapon.Id);
            UE_LOG(LogTemp, Display, TEXT("OWQUEST mission weapon removed: %s"), *A);
            break;
        case K::Reward:
            ++Rewards;
            if (A != Impl->Data.XpRewardAttribute) { Fail(TEXT("reward attribute differs from world.json values.xp: ") + A); return; }
            break;      // experience only: the stock reward data has no item (GetItemRewardsForPlayer in the script path returns none)
        case K::Experience: ApplyScriptExperience(FCString::Atoi(*A)); break;   // what the script's ExpEarn put into the VM pool
        case K::Level: {
            // The pool update ran ExpLevelUp / OnExpLevelChange on the VM controller; the host's own level display follows from the
            // experience it applied, so the two are compared (the skill points and maximum health come as their own events, below).
            const UOpenWillowSkills* Skills = Walker ? Walker->GetSkills() : nullptr;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST script level-up: VM level %s, host level %d (%s)"), *A, Skills ? Skills->GetLevel() : -1,
                Skills && Skills->GetLevel() == FCString::Atoi(*A) ? TEXT("same") : TEXT("DIFFERENT"));
            break;
        }
        case K::SkillPoints: {
            // ExpLevelUp added int(GeneralSkillPointsPerLevelUp) per level, evaluated from the installed data (1 from level 5: NATIVE_SKILLS.md section 1,
            // UNVERIFIED). The host's points are derived from the level (EarnedPointsAt), so this is compared with its own count for the same level change.
            const UOpenWillowSkills* Skills = Walker ? Walker->GetSkills() : nullptr;
            ScriptSkillPoints = FCString::Atoi(*A);
            ++ScriptSkillPointEvents;
            const int32 HostPoints = Skills ? UOpenWillowSkills::EarnedPointsAt(Skills->GetLevel()) - UOpenWillowSkills::EarnedPointsAt(LevelBeforeReward) : -1;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST script level-up skill points: VM awarded %d, host %d (%s)"), ScriptSkillPoints, HostPoints,
                ScriptSkillPoints == HostPoints ? TEXT("same") : TEXT("DIFFERENT"));
            break;
        }
        case K::MaxHealth: {
            // The health pool's base maximum for the new level, evaluated from the installed data (80 x 1.13^level: NATIVE_PROGRESSION.md section 4; the base
            // was confirmed in game, the trigger is a stand-in). The host's own formula (HealthForLevel) set its maximum health; the two are compared.
            const UOpenWillowSkills* Skills = Walker ? Walker->GetSkills() : nullptr;
            ScriptMaxHealth = FCString::Atof(*A);
            float HostHealth = -1.f;
            if (Skills) PlayerMaxHealth(Skills->GetLevel(), HostHealth);       // the host's own formula, the oracle
            UE_LOG(LogTemp, Display, TEXT("OWQUEST script level-up max health: VM %.3f at level %d, host formula %.3f (%s)"), ScriptMaxHealth, Skills ? Skills->GetLevel() : -1,
                HostHealth, FMath::IsNearlyEqual(ScriptMaxHealth, HostHealth, 0.01f) ? TEXT("same") : TEXT("DIFFERENT"));
            break;
        }
        case K::Health: {
            // The VM's health pool after the level-up (NATIVE_LEVEL_UP_ATTRIBUTES.md, the refill observed in game): the host adopts it, so a level-up from half
            // health ends at the new maximum. Compared with the host's own formula (the oracle) in the log and the suite.
            const float Current = FCString::Atof(*A), Maximum = FCString::Atof(UTF8_TO_TCHAR(Event.b.c_str()));
            const UOpenWillowSkills* Skills = Walker ? Walker->GetSkills() : nullptr;
            float Formula = -1.f;
            if (Skills) PlayerMaxHealth(Skills->GetLevel(), Formula);
            ScriptHealthCurrent = Current;
            ScriptHealthMaximum = Maximum;
            ++ScriptHealthEvents;
            bHealthAdoptedThisTick = true;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST script level-up health: VM %.3f/%.3f adopted; host formula maximum %.3f (%s)"), Current, Maximum, Formula,
                FMath::IsNearlyEqual(Maximum, Formula, 0.01f) ? TEXT("same") : TEXT("DIFFERENT"));
            if (Walker) Walker->ApplyScriptHealth(Maximum, Current);
            break;
        }
        case K::ActionSkillCooldown: {
            // The first OnLevelUp skill's second effect (NATIVE_LEVEL_UP_ATTRIBUTES.md section 7; observed in game by lane L3: the cooldown went to 0 in the
            // level-up call, at its maximum during a hold and at 7.99 during the drain): the VM read the modifier type and value from the installed data
            // (Scale by the pool's minimum, 0); the host keeps the cooldown, so it applies it to that.
            ScriptCooldownType = A == TEXT("Scale") ? 0 : A == TEXT("PreAdd") ? 1 : 2;
            ScriptCooldownValue = FCString::Atof(UTF8_TO_TCHAR(Event.b.c_str()));
            ++ScriptCooldownEvents;
            CooldownBeforeScript = Walker ? Walker->PhaselockRemaining() : -1.f;
            bScriptCooldownEnded = Walker && Walker->ModifyPhaselockCooldown(ScriptCooldownType, ScriptCooldownValue);
            CooldownAfterScript = Walker ? Walker->PhaselockRemaining() : -1.f;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST script level-up action skill cooldown: %s %.3f (the VM's, from the skill's data): %.2f -> %.2f"), *A, ScriptCooldownValue,
                CooldownBeforeScript, CooldownAfterScript);
            break;
        }
        case K::WeaponDamageScale: {
            // A natural level-up's OnLevelUpNaturally skill (NATIVE_LEVEL_UP_ATTRIBUTES.md section 7; observed in game: the held weapon's damage scale sum
            // +1.0 for 30 s): the VM read the value and the duration from the installed skill, the walker holds the weapon and applies it.
            ScriptDamageScale = FCString::Atof(*A);
            ScriptDamageScaleSeconds = FCString::Atof(UTF8_TO_TCHAR(Event.b.c_str()));
            ScriptDamageScaleAt = GetWorld()->GetTimeSeconds();
            ++ScriptDamageScaleEvents;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST script level-up weapon damage scale: +%.3f for %.1f s (the VM's, from the skill's data)"), ScriptDamageScale, ScriptDamageScaleSeconds);
            if (Walker) Walker->AddWeaponDamageScale(ScriptDamageScale, ScriptDamageScaleSeconds);
            break;
        }
        case K::OnUseDialog:   // PlayOnUseDialog's TriggerEvent on Marcus's dialog component: reported, not played (two-step group dispatch not implemented)
            LastOnUseTag = A;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST on-use dialog tag from Marcus's chain: %s (speaker %s, other %s; not played)"), *A, UTF8_TO_TCHAR(Event.b.c_str()), UTF8_TO_TCHAR(Event.c.c_str()));
            break;
        case K::MissionInterface:   // Behavior_ShowMissionInterface reached the controller's ClientGFxPlayMovie; the screen itself is not hosted
            ++MissionInterfacesOpened;
            LastInterfaceMovie = A;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST mission interface opened by Marcus's use chain (movie %s, director %s)"), *A, UTF8_TO_TCHAR(Event.b.c_str()));
            break;
        case K::SequenceChange:   // a behavior changed a sequence of the dummy's provider (the VM already applied it): logged, with the game time of the run
            UE_LOG(LogTemp, Display, TEXT("OWQUEST dummy sequence change: %s %s (%s)"), UTF8_TO_TCHAR(Event.b.c_str()), UTF8_TO_TCHAR(Event.c.c_str()), *A);
            break;
        case K::TurnInDialog:     // PlayMissionTurnedInDialog's TriggerEvent on Marcus's component after CompleteMission (observed in game); the stock data gives it no line
            ++TurnInDialogTags;
            LastTurnInTag = A;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST turn-in dialog tag raised on Marcus: %s (speaker %s, other %s)"), *A, UTF8_TO_TCHAR(Event.b.c_str()), UTF8_TO_TCHAR(Event.c.c_str()));
            break;
        case K::DismissalDialog:  // PlayDismissalDialog's TriggerEvent on Marcus's component when the use of him ends (observed in game); reported like the turn-in tag
            ++DismissalTags;
            DismissalAt = PumpEvents;
            LastDismissalTag = A;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST dismissal dialog tag raised on Marcus: %s (speaker %s, other %s)"), *A, UTF8_TO_TCHAR(Event.b.c_str()), UTF8_TO_TCHAR(Event.c.c_str()));
            break;
        case K::MissionInterfaceClosed:   // the movie's OnClose ran on the VM (the accept's empty entry list, or the player leaving the screen); nothing is drawn
            ++MissionScreensClosed;
            ClosedAt = PumpEvents;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST mission interface closed (director %s)"), *A);
            break;
        case K::RewardPage:       // the open screen shows the reward page; MissionRewardsReceived waits for the player (the use key, TryUse)
            ++RewardPages;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST reward page shown for %s; waiting for the use key"), *A);
            break;
        case K::RewardReceived:   // the player confirmed the page: the controller's MissionRewardsReceived ran
            ++RewardsReceived;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST reward received for %s"), *A);
            break;
        case K::Status:
            if (A == TEXT("Active") && ActiveAt < 0) ActiveAt = PumpEvents;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST mission status -> %s"), *A);
            break;
        case K::ObjectiveSet: UE_LOG(LogTemp, Display, TEXT("OWQUEST objective set active: %s"), *A); break;
        case K::ObjectiveComplete: UE_LOG(LogTemp, Display, TEXT("OWQUEST objective complete: %s"), *A); break;
        }
    }
    for (const auto& Line : Impl->Slice->errors()) { Fail(UTF8_TO_TCHAR(Line.c_str())); return; }
    // World behaviors the dummy provider reached. Behavior_Transform and Behavior_RegisterTargetable acting on the dummy
    // itself (BCONTEXT_Self) are run here; the rest (IntMath, ChangeInstanceDataSwitch) stay logged with their fields.
    for (const auto& Call : Impl->Slice->dummy().boundaryCalls) {
        TArray<FString> Fields;
        for (const auto& Field : Call.fields) Fields.Add(FString::Printf(TEXT("%hs=%hs"), Field.first.c_str(), Field.second.c_str()));
        auto Value = [&Call](const char* Key) {
            const auto It = Call.fields.find(Key);
            return It == Call.fields.end() ? FString() : FString(UTF8_TO_TCHAR(It->second.c_str()));
        };
        const bool bSelf = Value("Context") == TEXT("BCONTEXT_Self");
        const TCHAR* Result = TEXT("not run by host");
        if (bSelf && Call.cls == "WillowGame.Behavior_Transform") {
            // Script: WillowAIPawn.TransformType = Transform. Its readable consumer is GetTargetName (the balance's
            // transformed display name); nothing moves and no time is involved.
            DummyTransform = Value("Transform");
            TransformSequence = UTF8_TO_TCHAR(Call.sequence.c_str());
            Result = TEXT("run by host: TransformType set");
        } else if (bSelf && Call.cls == "WillowGame.Behavior_RegisterTargetable") {
            // Script: WillowPawn.Behavior_RegisterTargetable adds/removes the pawn in the global TargetableList (native,
            // keyed by allegiance). The host keeps a membership flag; which native searches read the list is UNVERIFIED.
            bDummyTargetable = Value("bUnregister") != TEXT("true");
            TargetableSequence = UTF8_TO_TCHAR(Call.sequence.c_str());
            ++TargetableCalls;
            Result = bDummyTargetable ? TEXT("run by host: registered targetable") : TEXT("run by host: unregistered");
        }
        UE_LOG(LogTemp, Display, TEXT("OWQUEST dummy behavior %hs:%hs (%hs.%hs) %s (%s)"), Call.cls.c_str(),
            Call.name.c_str(), Call.sequence.c_str(), Call.event.c_str(), *FString::Join(Fields, TEXT(" ")), Result);
        if (bSelf && Call.cls == "WillowGame.Behavior_Transform")
            UE_LOG(LogTemp, Display, TEXT("OWQUEST dummy target name now \"%s\""), *DummyTargetName());
    }
    Impl->Slice->dummy().boundary.clear();
    Impl->Slice->dummy().boundaryCalls.clear();
    if (Impl->Slice->mission().status() == vm::MissionSystem::Status::Complete) Impl->Completed.insert(MissionPath);
    RouteRequests();
    Save();
}

// Accept and turn-in run the installed controller script (AcceptMission, ServerCompleteMission; UNVERIFIED natives, see
// docs/verification/SANCTUARY_RPG_MISSION.md "Script swap 1"). Natives the script reaches without an implementation are
// logged stubs and never fail the quest.
void UOpenWillowQuest::LogScriptStubs(const TCHAR* When) const
{
    TArray<FString> Stubs;
    for (const auto& Stub : Impl->Slice->scriptStubs()) Stubs.Add(UTF8_TO_TCHAR(Stub.c_str()));
    UE_LOG(LogTemp, Display, TEXT("OWQUEST script stubs after %s: %d [%s]"), When, Stubs.Num(), *FString::Join(Stubs, TEXT("; ")));
}

// What the host owns and the script reads: the region's game stage (fixed per player and playthrough, locked into the mission when it
// becomes Active) and Maya's level and experience (the VM-side pool starts from them).
void UOpenWillowQuest::SyncScriptInputs()
{
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    UOpenWillowSkills* Skills = Walker ? Walker->GetSkills() : nullptr;
    if (!Skills) return;
    FixRegionStage(Skills->GetLevel());
    Impl->Slice->setRegionGameStage(RegionStage);
    Impl->Slice->setPlayerExperience(Skills->GetLevel(), Skills->GetExperience());
    Impl->Slice->setPlayerHealth(Walker->GetHealth());      // the VM's health pool starts from Maya's current health (a level-up rebases and refills it)
}

bool UOpenWillowQuest::Accept()
{
    if (!Impl || bFailed) return false;
    SyncScriptInputs();
    const bool bOk = Impl->Slice->accept(Impl->Completed);
    LogScriptStubs(TEXT("accept"));
    Pump();
    return bOk;
}

bool UOpenWillowQuest::TurnIn()
{
    if (!Impl || bFailed) return false;
    SyncScriptInputs();   // the script's GetExperienceReward and ExpEarn read the stage and Maya's experience
    const bool bOk = Impl->Slice->turnIn();
    LogScriptStubs(TEXT("turn-in"));
    Pump();
    return bOk;
}

// Choosing the usable (NATIVE_USE_INTERACTION.md, "Per-frame usable evaluation", UNVERIFIED): one ray from the camera along the view,
// GlobalsDefinition.PlayerInteractionDistance long (read from the installed data through the slice, 350 uu), the hits nearest first,
// the first usable hit before a blocking hit wins; no cone, no radius. Marcus is the quest's only usable thing, so the ray tests his
// capsule and the world's blocking geometry (the visibility channel, the player excluded). Not modelled: the 1/30 s throttle, the
// cinematic lock, icons and prompts.
bool UOpenWillowQuest::InTalkReach() const
{
    const AActor* Owner = GetOwner();
    const auto* Pawn = Cast<APawn>(Owner);
    const UCameraComponent* Camera = Owner ? Owner->FindComponentByClass<UCameraComponent>() : nullptr;
    const float Distance = Impl ? Impl->Slice->playerInteractionDistance() : 0.f;
    if (!Marcus || !Pawn || !Camera || Distance <= 0.f || !GetWorld()) return false;
    // The view point: the camera's position and the control rotation (current even in the frame of an aim change; the camera
    // manager's cached view lags one frame).
    const FVector Eye = Camera->GetComponentLocation();
    const FRotator View = Pawn->GetViewRotation();
    const FVector End = Eye + View.Vector() * Distance;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OWQuestUse), true, Owner);
    FHitResult Usable;
    if (!Marcus->GetHitVolume() || !Marcus->GetHitVolume()->LineTraceComponent(Usable, Eye, End, Query)) return false;
    FHitResult Block;
    Query.AddIgnoredActor(Marcus);   // Marcus is the usable; only what else blocks the ray in front of him matters
    return !GetWorld()->LineTraceSingleByChannel(Block, Eye, End, ECC_Visibility, Query) || Block.Distance > Usable.Distance;
}

void UOpenWillowQuest::MissionScreen(bool& bRedeemable, bool& bEligible) const
{
    const auto Lists = Impl->Slice->screen(Impl->Completed);
    const std::string Path = Impl->Slice->mission().path();
    const auto Has = [&Path](const std::vector<std::string>& List) { return std::find(List.begin(), List.end(), Path) != List.end(); };
    bRedeemable = Has(Lists.redeemable);
    bEligible = Has(Lists.eligible);
}

bool UOpenWillowQuest::IsRewardPending() const { return Impl && !bFailed && Impl->Slice->rewardPending(); }

bool UOpenWillowQuest::MayTakeUse() const
{
    return Impl && !bFailed && (Impl->Slice->rewardPending() || InTalkReach());
}

bool UOpenWillowQuest::TryUse()
{
    if (!Impl || bFailed) return false;
    ++ScreenUsesSeen;
    // Checked before Marcus's chain runs, so the VM opens no screen either. Without a page URL nothing changes.
    if (ScreenPage && !ScreenPage->IsShown() && OtherPageOpen())
    {
        UE_LOG(LogTemp, Display, TEXT("OWQUEST mission page refused: another page is open"));
        return false;
    }
    if (Impl->Slice->rewardPending()) {
        // A reward left waiting (the page was closed by a fall): show it again rather than take the reward with no page.
        if (ScreenPage && ScreenPage->IsReady()) { ShowScreenPage(TEXT("reward")); return true; }
        // The reward page is up (the turn-in found Marcus's screen open): the page is modal and its key handler only takes the confirmation, so the use key is
        // that confirmation (the movie's Enter), and the screen then closes (the stand-in for Escape: the dismissal tag, the interface closed).
        const bool bReceived = Impl->Slice->confirmReward();
        Pump();
        const bool bClosed = Impl->Slice->closeScreen();
        Pump();
        UE_LOG(LogTemp, Display, TEXT("OWQUEST use key on the reward page: received=%d, screen closed=%d"), bReceived, bClosed);
        return true;
    }
    if (!InTalkReach()) return false;
    using S = vm::MissionSystem::Status;
    const S Current = Impl->Slice->mission().status();
    // The key reached Marcus through the use ray above. His stock OnUsed chain runs on the VM (the nine sequence checks, the context dialog,
    // HasMissions, ShowMissionInterface); it ends by asking the controller to open the mission interface, which Pump() sees as an event. The
    // screen itself is not hosted: Marcus's own list scripts say what it would offer, and the host stands in for the button press by confirming
    // the one offered entry (redeemable first, as the screen lists them), which runs the stock AcceptMission / ServerCompleteMission with Marcus
    // as the director. With nothing offered the key is not consumed (pickups and the door still get it).
    const int32 OpenedBefore = MissionInterfacesOpened;
    const auto Use = Impl->Slice->useMarcus(Impl->Completed);
    LastUseCascade.Reset();
    for (const auto& Line : Use.cascade) LastUseCascade.Add(UTF8_TO_TCHAR(Line.c_str()));
    Pump();
    const bool bOpened = Use.interfaceOpened && MissionInterfacesOpened > OpenedBefore;
    bool bRedeemable = false, bEligible = false;
    if (bOpened) MissionScreen(bRedeemable, bEligible);
    if (ScreenPage && (bRedeemable || bEligible))
    {
        if (ScreenPage->IsReady())
        {
            ShowScreenPage(bRedeemable ? TEXT("turnin") : TEXT("offer"));
            return true;
        }
        UE_LOG(LogTemp, Warning, TEXT("OWQUEST mission page is not ready; confirming without a page"));
    }
    const TCHAR* Action = TEXT("nothing to accept or turn in");
    bool bOk = true;
    if (bRedeemable) { Action = TEXT("turn in"); bOk = TurnIn(); }
    else if (bEligible) { Action = TEXT("accept"); bOk = Accept(); }
    UE_LOG(LogTemp, Display, TEXT("OWQUEST use near Marcus: %s (ok=%d, status %d -> %d, interface %d)"), Action, bOk, int32(Current), Status(), bOpened);
    return bRedeemable || bEligible;
}

bool UOpenWillowQuest::OtherPageOpen() const
{
    const UWorld* World = GetWorld();
    const APlayerController* PC = World ? World->GetFirstPlayerController() : nullptr;
    const auto* Hud = PC ? Cast<AOpenWillowMayaHUD>(PC->GetHUD()) : nullptr;
    const auto* Shop = World ? World->GetSubsystem<UOpenWillowVendorSubsystem>() : nullptr;
    return (Hud && Hud->IsSkillsOpen()) || (Shop && Shop->IsShopOpen());
}

void UOpenWillowQuest::ShowScreenPage(const TCHAR* Page)
{
    ScreenPageOn = Page;
    // The status pages' transition: the trigger and aim let go, arms and gun put away (and the firing gates that flag holds). Every close restores it, in CloseScreenPage.
    if (auto* Walker = Cast<AOpenWillowWalker>(GetOwner())) Walker->SetInventoryPresentation(true);
    ScreenPage->Show(GetWorld() ? GetWorld()->GetFirstPlayerController() : nullptr);
    ScreenPage->SendState(FString::Printf(TEXT("{\"page\":\"%s\",\"log\":%s}"), Page, *MissionLogJson()));
    UE_LOG(LogTemp, Display, TEXT("OWQUEST mission page open: %s (status %d)"), Page, Status());
}

void UOpenWillowQuest::CloseScreenPage(const TCHAR* Why)
{
    if (!ScreenPage || !ScreenPage->IsShown()) return;
    ScreenPage->SendState(TEXT("{\"page\":\"closed\"}"));
    ScreenPage->Hide(GetWorld() ? GetWorld()->GetFirstPlayerController() : nullptr);
    ScreenPageOn.Reset();
    if (auto* Walker = Cast<AOpenWillowWalker>(GetOwner())) Walker->SetInventoryPresentation(false);
    ReleaseFocusCamera();   // every close, end of play and a failure included, not only the next tick
    UE_LOG(LogTemp, Display, TEXT("OWQUEST mission page closed: %s (status %d)"), Why, Status());
}

void UOpenWillowQuest::ReleaseFocusCamera()
{
    if (!bFocusOn) return;
    bFocusOn = false;
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    UCameraComponent* Camera = Walker ? Walker->FindComponentByClass<UCameraComponent>() : nullptr;
    if (!Camera) return;
    OpenWillowFocusCam::Release(*Camera, FocusBaseFov);
    UE_LOG(LogTemp, Display, TEXT("OWFOCUSCAM off: the view follows the control rotation again, FOV %.1f"), FocusBaseFov);
}

void UOpenWillowQuest::TickFocusCamera()
{
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    UCameraComponent* Camera = Walker ? Walker->FindComponentByClass<UCameraComponent>() : nullptr;
    if (!Camera) return;
    if (!IsScreenPageShown() || !Marcus || bFailed)
    {
        ReleaseFocusCamera();
        return;
    }
    if (!bFocusTried)
    {
        bFocusTried = true;
        const FString Path = OpenWillowFocusCam::DefaultPath();
        OpenWillowFocusCam::FParams Params;
        FString Error;
        if (OpenWillowFocusCam::Load(Path, Params, Error))
        {
            FocusParams = Params;
            UE_LOG(LogTemp, Display, TEXT("OWFOCUSCAM read %s: bone %s, screen (%.3f, %.3f), zoom %.3f (%s; UNVERIFIED stand-in)"), *Path, *Params.Bone.ToString(),
                Params.Screen.X, Params.Screen.Y, Params.Zoom, Params.Source.IsEmpty() ? TEXT("no source given") : *Params.Source);
        }
        else
        {
            UE_LOG(LogTemp, Warning, TEXT("OWFOCUSCAM %s: %s; the mission page leaves the view as it is"), *Path, *Error);
        }
    }
    if (!FocusParams) return;
    // The player's FOV: the walker sets it on each of its ticks. Still the value set here last frame means it has not ticked since, and the kept one stands.
    if (!bFocusOn || !FMath::IsNearlyEqual(static_cast<double>(Camera->FieldOfView), FocusSetFov, 0.01)) FocusBaseFov = Camera->FieldOfView;
    double Aspect = 16. / 9.;
    if (GEngine && GEngine->GameViewport)
    {
        FVector2D Size = FVector2D::ZeroVector;
        GEngine->GameViewport->GetViewportSize(Size);
        if (Size.X > 1. && Size.Y > 1.) Aspect = Size.X / Size.Y;
    }
    FVector Point;
    const bool bBone = OpenWillowFocusCam::FocusPoint(*Marcus, FocusParams->Bone, Point);
    const bool bSolved = OpenWillowFocusCam::Apply(*Camera, Point, *FocusParams, Aspect, FocusBaseFov);
    FocusSetFov = Camera->FieldOfView;
    if (!bFocusOn)
    {
        bFocusOn = true;
        UE_LOG(LogTemp, Display, TEXT("OWFOCUSCAM on: page %s, point %s (%s), %.0f uu from the eye, FOV %.1f -> %.1f, view %s%s"), *ScreenPageOn, *Point.ToString(),
            bBone ? *FocusParams->Bone.ToString() : TEXT("no such bone: actor location"), FVector::Dist(Point, Camera->GetComponentLocation()), FocusBaseFov, FocusSetFov,
            *Camera->GetComponentRotation().ToString(), bSolved ? TEXT("") : TEXT(" (no solution: plain look-at)"));
    }
}

void UOpenWillowQuest::OnScreenKey(const FKey& Key)
{
    if (!Impl || bFailed || !ScreenPage || !ScreenPage->IsShown()) return;
    ++ScreenKeysSeen;
    if (Key == EKeys::PageUp) { ScreenPage->SendKey(TEXT("PageUp")); return; }
    if (Key == EKeys::PageDown) { ScreenPage->SendKey(TEXT("PageDown")); return; }
    const int32 Before = Status();
    if (Key == EKeys::Escape)
    {
        if (ScreenPageOn == TEXT("reward")) { UE_LOG(LogTemp, Display, TEXT("OWQUEST mission page: Escape does nothing on the reward page")); return; }
        Impl->Slice->closeScreen();
        Pump();
        CloseScreenPage(TEXT("Escape"));
        return;
    }
    if (Key != EKeys::Enter) { UE_LOG(LogTemp, Display, TEXT("OWQUEST mission page: key %s does nothing on %s"), *Key.ToString(), *ScreenPageOn); return; }
    if (ScreenPageOn == TEXT("reward"))
    {
        // The confirmation, then the screen's close (the slice has no next mission to list).
        Impl->Slice->confirmReward();
        Pump();
        Impl->Slice->closeScreen();
        Pump();
        CloseScreenPage(TEXT("reward taken"));
    }
    else if (ScreenPageOn == TEXT("turnin"))
    {
        const bool bOk = TurnIn();
        UE_LOG(LogTemp, Display, TEXT("OWQUEST mission page: Enter turns in (ok=%d, status %d -> %d)"), bOk, Before, Status());
        if (Impl->Slice->rewardPending()) ShowScreenPage(TEXT("reward"));
        else { Impl->Slice->closeScreen(); Pump(); CloseScreenPage(TEXT("turned in with no reward page")); }
    }
    else
    {
        // The VM's accept closes the screen itself, as the live trace shows.
        const bool bOk = Accept();
        UE_LOG(LogTemp, Display, TEXT("OWQUEST mission page: Enter accepts (ok=%d, status %d -> %d)"), bOk, Before, Status());
        if (Impl->Slice->screenOpen()) { Impl->Slice->closeScreen(); Pump(); }
        CloseScreenPage(TEXT("accepted"));
    }
}

void UOpenWillowQuest::TickScreenPage()
{
    if (!bScreenPageTried && !bTesting)
    {
        bScreenPageTried = true;
        if (!FOpenWillowMissionPage::PageUrl().IsEmpty())
        {
            ScreenPage = MakeShared<FOpenWillowMissionPage>([this](const FKey& Key) { OnScreenKey(Key); });
            ScreenPage->Preload();
            bScreenPageTest = FParse::Param(FCommandLine::Get(), TEXT("owmissionpagetest"));
        }
    }
    if (!ScreenPage) return;
    if (ScreenPage->IsShown())
    {
        // A fall or a lost page closes it; a reward still waiting is shown again at the next use (TryUse).
        const auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
        if (!ScreenPage->IsReady() || !Walker || Walker->IsDown())
        {
            if (!Impl->Slice->rewardPending()) { Impl->Slice->closeScreen(); Pump(); }
            CloseScreenPage(ScreenPage->IsReady() ? TEXT("Maya is down") : TEXT("the page was lost"));
        }
    }
    TickScreenPageTest();
}

bool UOpenWillowQuest::IsScreenPageShown() const { return ScreenPage && ScreenPage->IsShown(); }
bool UOpenWillowQuest::ScreenOpenInVm() const { return Impl && Impl->Slice->screenOpen(); }
bool UOpenWillowQuest::RewardPendingInVm() const { return Impl && Impl->Slice->rewardPending(); }

void UOpenWillowQuest::TestAdvanceObjectives()
{
    Impl->Slice->enterRange();
    Impl->Slice->spawnDummy();
    Impl->Slice->hitDummy(true);
    Pump();
}

void UOpenWillowQuest::OnDummyDamaged(AOpenWillowCombatTarget* Target, const FString& DamageType, bool bStatusTick)
{
    if (!Impl || bFailed || !Target || Target != Dummy) return;
    // The shot's stock damage type path goes to the dummy's own OnTakeDamage (its CompareObject decides).
    LastDummyDamageType = DamageType;
    DummyDamageTypes.Add(DamageType);
    DummyStatusTicks.Add(bStatusTick);
    ++DummyShots;
    UE_LOG(LogTemp, Display, TEXT("OWQUEST dummy OnTakeDamage DamageType=%s%s"), DamageType.IsEmpty() ? TEXT("None") : *DamageType,
        bStatusTick ? TEXT(" (status tick)") : TEXT(""));
    Impl->Slice->damageDummy(TCHAR_TO_UTF8(*DamageType));
    Pump();
}

void UOpenWillowQuest::LendMissionWeapon()
{
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    bWeaponLent = Walker && Walker->LendWeapon(MissionWeapon);
    if (!bWeaponLent) UE_LOG(LogTemp, Error, TEXT("OWQUEST could not lend %s to Maya (backpack full or no walker)"), *MissionWeapon.Id);
}

// The experience the script's ExpEarn put into the VM pool (MissionDefinition.GetExperienceReward: NATIVE_PROGRESSION section 2,
// UNVERIFIED except the amount 395 at stage 8, confirmed in game): the host applies it to Maya's level and experience display.
void UOpenWillowQuest::ApplyScriptExperience(int32 Amount)
{
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    UOpenWillowSkills* Skills = Walker ? Walker->GetSkills() : nullptr;
    if (!Skills) return;
    LastXpAmount = Amount;
    ExperienceBeforeReward = Skills->GetExperience();
    LevelBeforeReward = Skills->GetLevel();
    const auto& Earned = Impl->Slice->expEarned();
    UE_LOG(LogTemp, Display, TEXT("OWQUEST script ExpEarn calls=%d amount=%d source=%d; pool gained %d"), int32(Earned.size()),
        Earned.empty() ? 0 : Earned.back().amount, Earned.empty() ? -1 : Earned.back().source, Amount);
    Skills->AddExperience(Amount);
    // A level-up from this reward is the VM's: its pool update rebases the health pool and the OnLevelUp behaviors refill it, and the Health event sets Maya's
    // health. The walker's own refresh waits for that event (the next tick) instead of setting the host formula at once.
    if (Skills->GetLevel() != LevelBeforeReward) {
        Walker->HoldHealthForScript(true);
        HealthHoldTicks = 1;
    }
    UE_LOG(LogTemp, Display, TEXT("OWQUEST XP reward %s from the script at region stage %d = %d XP (UNVERIFIED rules, amount at stage 8 confirmed); experience %lld -> %lld, level %d -> %d, skill points %d"),
        *Impl->Data.XpRewardAttribute, RegionStage, LastXpAmount, ExperienceBeforeReward,
        Skills->GetExperience(), LevelBeforeReward, Skills->GetLevel(), Skills->AvailablePoints());
}

void UOpenWillowQuest::FixRegionStage(int32 PlayerLevel)
{
    if (!Impl || RegionStage > 0) return;
    // WillowRegionDefinition.GetRegionGameStage: a stage already stored for this region and playthrough is returned
    // unchanged, so only the first ask computes it. Mission completion comes from the quest's completed set (the
    // dependency fixture plus missions completed here).
    const auto& Completed = Impl->Completed;
    RegionStage = Impl->Data.RegionStage(PlayerLevel, [&Completed](const FString& Mission) { return Completed.count(TCHAR_TO_UTF8(*Mission)) > 0; });
    RegionStageLevel = PlayerLevel;
    UE_LOG(LogTemp, Display, TEXT("OWQUEST region stage fixed: %s stage %d from player level %d (%s; rule read from native code, UNVERIFIED)"),
        *Impl->Data.StageRegion, RegionStage, PlayerLevel, Impl->Data.bRegionStageFromData ? TEXT("bounds from data") : TEXT("STAND-IN bounds"));
}

void UOpenWillowQuest::NotifyRespawn()
{
    ++Respawns;
    UE_LOG(LogTemp, Display, TEXT("OWQUEST player respawned (count=%d, mission status kept=%d)"), Respawns, Status());
    Save();
}

bool UOpenWillowQuest::PlayerMaxHealth(int32 Level, float& Out) const
{
    if (!Impl || bFailed) return false;
    Out = Impl->Data.HealthForLevel(Level);
    return true;
}

bool UOpenWillowQuest::RestoreProgression(UOpenWillowSkills& Skills)
{
    if (!Impl || bFailed) return false;
    if (SavedProgression) {
        const int32 StartLevel = Skills.GetLevel();
        FString Error;
        if (!Skills.RestoreProgression(*SavedProgression, Error)) { Fail(TEXT("quest save progression was rejected: ") + Error); return false; }
        bProgressionRestored = true;
        UE_LOG(LogTemp, Display, TEXT("OWQUEST progression from save: level %d (start level %d replaced), experience %lld, action grade %d, skill points %d"),
            Skills.GetLevel(), StartLevel, Skills.GetExperience(), Skills.GetActionGrade(), Skills.AvailablePoints());
    }
    // The walker calls this once Maya's level is known at session start: the first time Sanctuary's stage is asked for
    // in the host (a stage kept in the save wins).
    FixRegionStage(Skills.GetLevel());
    return bProgressionRestored;
}

bool UOpenWillowQuest::IsMissionDummy(const AActor* Actor) const
{
    return Actor && Actor == Dummy;
}

bool UOpenWillowQuest::IsRegisteredTargetable(const AActor* Actor) const
{
    return Actor && Actor == Dummy && bDummyTargetable;
}

FString UOpenWillowQuest::DummyTargetName() const
{
    if (!Impl || !Impl->Data.bHasDummyNames) return FString();
    // WillowAIPawn.GetTargetName (script): TransformType != 0 -> GetTransformedName -> the balance's
    // GetTransformedDisplayName(TransformType) (native: "the playthrough entry of that type" is UNVERIFIED); else the
    // balance's display name. The name-list and displayed-parent branches before it are not modelled.
    if (const FString* Name = Impl->Data.DummyTransformedNames.Find(DummyTransform)) return *Name;
    return Impl->Data.DummyDisplayName;
}

bool UOpenWillowQuest::RespawnPoint(const FVector& DeathLocation, FTransform& Out)
{
    if (!Impl || bFailed) return false;
    // WillowPlayerPawn.GetBestPlayerPlacementPoint (script): an activated checkpoint, then a station flagged active,
    // then the nearest station that CanResurrectHere, then the nearest other. Station activation is not traced, so
    // the host has none activated and starts at the nearest-capable rule (UNVERIFIED runtime state).
    const auto& Stations = Impl->Data.Stations;
    int32 Best = INDEX_NONE;
    for (const bool bCapable : {true, false}) {
        for (int32 I = 0; I < Stations.Num(); ++I) {
            if (Stations[I].bCanResurrect != bCapable || Stations[I].Exits.Num() == 0) continue;
            if (Best == INDEX_NONE || FVector::DistSquared(DeathLocation, Stations[I].Location)
                < FVector::DistSquared(DeathLocation, Stations[Best].Location)) Best = I;
        }
        if (Best != INDEX_NONE) break;
    }
    if (Best == INDEX_NONE) return false;
    // TeleporterDestination.GetNextExitPoint: round robin from 0 (vehicle/occupied exit skipping not modelled).
    int32& Counter = Impl->StationCounters[Best];
    Out = Stations[Best].Exits[Counter % Stations[Best].Exits.Num()];
    ++Counter;
    UE_LOG(LogTemp, Display, TEXT("OWQUEST respawn station %s exit %d at %s"), *Stations[Best].Object,
        (Counter - 1) % Stations[Best].Exits.Num(), *Out.GetLocation().ToString());
    return true;
}

bool UOpenWillowQuest::TouchesTrigger(const AActor* Actor) const
{
    // The overlap of an actor's collision cylinder with the waypoint's (planar distance within the sum of radii, vertical gap within the
    // sum of half heights). UE3's touch shape test: UNVERIFIED. The host owns this test; the slice's waypoint script decides what a touch means.
    if (!Actor) return false;
    const auto& Data = Impl->Data;
    float Radius = 0.f, HalfHeight = 0.f;
    Actor->GetSimpleCollisionCylinder(Radius, HalfHeight);
    const FVector P = Actor->GetActorLocation();
    return FVector::Dist2D(P, Data.TriggerCenter) <= Data.TriggerRadius + Radius && FMath::Abs(P.Z - Data.TriggerCenter.Z) <= Data.TriggerHalfHeight + HalfHeight;
}

bool UOpenWillowQuest::PlayerTouchesTrigger() const
{
    return TouchesTrigger(Cast<AOpenWillowWalker>(GetOwner()));
}

bool UOpenWillowQuest::HudTracker(FString& OutMission, TArray<FHudTrackerLine>& OutLines, bool& bOutTurnIn) const
{
    using S = vm::MissionSystem::Status;
    OutLines.Reset();
    bOutTurnIn = false;
    if (!Impl || !Impl->Slice || bFailed) return false;
    const auto& Mission = Impl->Slice->mission();
    const S Current = Mission.status();
    if (Current != S::Active && Current != S::ReadyToTurnIn) return false;
    OutMission = UTF8_TO_TCHAR(Mission.name().c_str());
    bOutTurnIn = Current == S::ReadyToTurnIn;
    for (const auto& Line : Mission.trackerLines())
        OutLines.Add({UTF8_TO_TCHAR(Line.message.c_str()), Line.complete, Line.optional});
    return true;
}

void UOpenWillowQuest::UpdateHints()
{
    using S = vm::MissionSystem::Status;
    const S Current = Impl->Slice->mission().status();
    bool bRedeemable = false, bEligible = false;
    const bool bInReach = InTalkReach();
    if (bInReach) MissionScreen(bRedeemable, bEligible);
    const bool bCanTalk = bInReach && (bRedeemable || bEligible);
    if (bCanTalk && !bTalkHintLogged) UE_LOG(LogTemp, Display, TEXT("OWQUEST hint: press E (use) to talk to Marcus"));
    bTalkHintLogged = bCanTalk;
    if (!GEngine || bTesting) return;
    // The text is a debug overlay drawn over everything; the status pages (inventory, skills, missions, map), the mission page and
    // the vending shop cover the gameplay HUD, so it is not posted while one is open (each line lasts one frame). The movie HUD has its
    // own tracker and prompt, so this is for the canvas fallback only.
    if (IsScreenPageShown()) return;
    const APawn* Pawn = Cast<APawn>(GetOwner());
    const APlayerController* Player = Pawn ? Cast<APlayerController>(Pawn->GetController()) : nullptr;
    if (const AOpenWillowMayaHUD* Hud = Player ? Cast<AOpenWillowMayaHUD>(Player->GetHUD()) : nullptr)
        if (Hud->IsSkillsOpen() || Hud->IsFlashHudReady()) return;
    if (const UOpenWillowVendorSubsystem* Vendors = GetWorld() ? GetWorld()->GetSubsystem<UOpenWillowVendorSubsystem>() : nullptr)
        if (Vendors->IsShopOpen()) return;
    FString Line = TEXT("Rock, Paper, Genocide: Fire Weapons! - ");
    if (Current == S::NotStarted) Line += TEXT("talk to Marcus (E)");
    else if (Current == S::Active && Impl->ObjectiveState(Impl->Data.TriggerObjective) == "Active") Line += TEXT("go to the range");
    else if (Current == S::Active) Line += TEXT("shoot the target with the lent fire pistol (") + MissionWeapon.Name + TEXT(")");
    else if (Current == S::ReadyToTurnIn) Line += TEXT("turn in to Marcus (E)");
    else Line += TEXT("complete");
    GEngine->AddOnScreenDebugMessage(0x0E57A001, 0.f, FColor::White, Line);
    if (bCanTalk) GEngine->AddOnScreenDebugMessage(0x0E57A002, 0.f, FColor::Yellow, TEXT("E: talk to Marcus"));
    else if (Current == S::NotStarted && Marcus)
        GEngine->AddOnScreenDebugMessage(0x0E57A002, 0.f, FColor::Silver, FString::Printf(TEXT("Marcus is %.0f m away (his shop)"),
            FVector::Dist(GetOwner()->GetActorLocation(), Marcus->GetActorLocation()) / 100.f));
}

void UOpenWillowQuest::TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function)
{
    Super::TickComponent(Delta, Type, Function);
    TickFocusCamera();   // before the failure return; a failure itself closes the page and gives the view back (Fail)
    // The page suite's last step checks the view after its own forced failure, so it still runs then (and nothing else does).
    if (bFailed && bScreenPageTest && ScreenTestStep == 22) TickScreenPageTest();
    if (!Impl || bFailed) return;
    if (bReleaseUse) {
        if (auto* Pawn = Cast<APawn>(GetOwner())) if (auto* PC = Cast<APlayerController>(Pawn->GetController()))
            PC->InputKey(FInputKeyEventArgs::CreateSimulated(EKeys::E, IE_Released, 0.f));
        bReleaseUse = false;
    }
    if (!bTrackBound) {
        // The target Matinee shares the door mover's sequence instance; bind it once the mover has loaded.
        bTrackBound = true;
        auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
        if (!Walker || !Walker->GetMover()) { Fail(TEXT("-owquest needs the walker's mover (-owwalk -owmover=...)")); return; }
        try { Walker->GetMover()->BindTrack(Impl->Data.TargetBinding); }
        catch (const std::exception& Error) { Fail(UTF8_TO_TCHAR(Error.what())); return; }
    }
    if (bLendPending)
    {
        bLendPending = false;
        LendMissionWeapon();
    }
    // Any other level change (the test fixtures' SetLevel) is picked up here, once a frame.
    if (auto* Walker = Cast<AOpenWillowWalker>(GetOwner())) Walker->RefreshHealthForLevel();
    bHealthAdoptedThisTick = false;
    Impl->Slice->tick(Delta);
    Pump();
    TickScreenPage();
    // A script level-up that sent no Health event (no health pool data): the walker's own refresh takes over after this tick.
    if (auto* HealthWalker = Cast<AOpenWillowWalker>(GetOwner()))
        if (HealthWalker->IsHealthHeldForScript() && !bHealthAdoptedThisTick && --HealthHoldTicks <= 0) {
            HealthWalker->HoldHealthForScript(false);
            UE_LOG(LogTemp, Warning, TEXT("OWQUEST the script sent no health after the level-up: the host formula applies"));
        }
    ProcessArrivals();
    if (bFailed) return;
    // GoToRange: the stock WillowWaypoint script runs on the VM (NATIVE_OBJECTIVE_TRIGGERS.md, UNVERIFIED). The host reports when the player
    // pawn's and Marcus's collision begins or ends overlapping its cylinder, at any time; the script ignores Marcus, and for the player
    // asks the tracker whether the objective is updatable (also when the set becomes active while the player stands inside).
    {
        const bool bPlayerIn = PlayerTouchesTrigger();
        if (bPlayerIn != bPlayerInRange) {
            bPlayerInRange = bPlayerIn;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST player %s %s"), bPlayerIn ? TEXT("touched") : TEXT("left"), *Impl->Data.TriggerObject);
            Impl->Slice->touchWaypoint(true, bPlayerIn);
            Pump();
        }
        const bool bMarcusIn = TouchesTrigger(Marcus);
        if (bMarcusIn != bMarcusInRange) {
            bMarcusInRange = bMarcusIn;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST Marcus %s %s (ignored by the waypoint script)"), bMarcusIn ? TEXT("touched") : TEXT("left"), *Impl->Data.TriggerObject);
            Impl->Slice->touchWaypoint(false, bMarcusIn);
            Pump();
        }
    }
    // The Fire den spawns its dummy when the Fire objective is active (MissionPopulationAspect is native: UNVERIFIED).
    if (!bDummySpawned && Impl->ObjectiveState(Impl->Data.DummyObjective) == "Active") SpawnDummy();
    UpdateHints();
    if (bTesting && !bFailed) RunTest(Delta);
}

// ------------------------------------------------------------------------------------------- automated run

void UOpenWillowQuest::PlacePlayer(const FVector& Location, const FVector& Look)
{
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    Walker->SetActorLocation(Location, false, nullptr, ETeleportType::TeleportPhysics);
    Walker->GetCharacterMovement()->StopMovementImmediately();
    Walker->NotifyTeleported();
    if (Walker->GetController()) Walker->GetController()->SetControlRotation((Look - (Location + FVector(0, 0, 70))).Rotation());
}

void UOpenWillowQuest::PressUse()
{
    if (auto* Pawn = Cast<APawn>(GetOwner())) if (auto* PC = Cast<APlayerController>(Pawn->GetController())) {
        PC->InputKey(FInputKeyEventArgs::CreateSimulated(EKeys::E, IE_Pressed, 1.f));
        bReleaseUse = true;
    }
}

// Pellets still flying: main's light bullets and rockets are AOpenWillowProjectile actors (OpenWillowFiring); a trace pellet never flies.
static int32 ProjectilesInFlight(UWorld* World)
{
    int32 Count = 0;
    for (TActorIterator<AOpenWillowProjectile> It(World); It; ++It) ++Count;
    return Count;
}

void UOpenWillowQuest::Shot(const TCHAR* Name)
{
    FScreenshotRequest::RequestScreenshot(FString::Printf(TEXT("OWQuest_%s.png"), Name), false, false);
    UE_LOG(LogTemp, Display, TEXT("OWQUEST screenshot %s"), Name);
}

void UOpenWillowQuest::RunTest(float Delta)
{
    TestWait += Delta;
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    UOpenWillowMover* Mover = Walker ? Walker->GetMover() : nullptr;
    if (!Walker || !Mover || !Mover->TrackCarrier()) { Fail(TEXT("quest test needs the walker, the mover (-owmover=...) and the bound target track")); return; }
    const auto& Data = Impl->Data;
    const FVector MarcusFront = Marcus ? Marcus->GetActorLocation() + Marcus->GetActorForwardVector() : FVector::ZeroVector;
    auto Waiting = [this](bool bDone, float Timeout) { return !bDone && TestWait < Timeout; };
    switch (TestStep) {
    case 0: {
        if (TestWait < 1.5f) return;
        if (bResume) {
            Check(Status() == 3, TEXT("resume_status_complete_from_save"));
            Check(Rewards == 1, TEXT("resume_reward_retained"));
            Check(Respawns >= 1, TEXT("resume_respawn_count_retained"));
            Check(Marcus && Marcus->GetActorLocation().Equals(Data.MarcusLocation, 0.5f), TEXT("resume_marcus_at_stock_pose"));
            // Progression: the restored state must equal the save's block (what the first run ended with, checked
            // there by save_holds_final_progression), and the saved level must have replaced -owlevel.
            {
                const UOpenWillowSkills* Skills = Walker->GetSkills();
                const FString& Suspension = Walker->GetPhaselockData().DurationSkill;
                int32 CommandLevel = 0, SavedLevel = 0, SavedPoints = -1, SavedAction = -1, SavedSuspension = 0;
                int64 SavedExperience = -1;
                const TSharedPtr<FJsonObject>* SavedGrades = nullptr;
                FParse::Value(FCommandLine::Get(), TEXT("owlevel="), CommandLevel);
                const bool bBlock = SavedProgression && SavedProgression->TryGetNumberField(TEXT("level"), SavedLevel)
                    && SavedProgression->TryGetNumberField(TEXT("experience"), SavedExperience)
                    && SavedProgression->TryGetNumberField(TEXT("points"), SavedPoints)
                    && SavedProgression->TryGetNumberField(TEXT("actionGrade"), SavedAction)
                    && SavedProgression->TryGetObjectField(TEXT("grades"), SavedGrades);
                if (bBlock) (*SavedGrades)->TryGetNumberField(Suspension, SavedSuspension);
                Check(bBlock && bProgressionRestored && SavedLevel != CommandLevel && Skills->GetLevel() == SavedLevel
                    && Skills->GetExperience() == SavedExperience, TEXT("resume_level_and_experience_from_save_over_owlevel"));
                Check(bBlock && Skills->AvailablePoints() == SavedPoints && Skills->GetActionGrade() == SavedAction && SavedAction >= 1,
                    TEXT("resume_skill_points_and_phaselock_from_save"));
                Check(bBlock && SavedSuspension >= 1 && Skills->GradeOf(Suspension) == SavedSuspension
                    && JsonText(*SavedGrades) == JsonText(Skills->ProgressionJson()->GetObjectField(TEXT("grades"))),
                    TEXT("resume_skill_grades_from_save"));
            }
            PlacePlayer(Data.MarcusLocation + Marcus->GetActorForwardVector() * 150.f, Data.MarcusLocation);
            TestStep = 100;
            TestWait = 0;
            return;
        }
        // Test captures only: no motion blur after the scripted teleports.
        if (GEngine) GEngine->Exec(GetWorld(), TEXT("r.MotionBlurQuality 0"));
        Check(Status() == 0, TEXT("fresh_status_not_started"));
        // Coordinate convention: the loaded door (placement already checked against mover.json) must lie next to
        // the walk segment between the door-opening and door-closing nodes in world.json; a Y-mirrored reading would not.
        const FVector Door = Mover->DoorPlacedLocation();
        const FVector A = Data.Walk[0].Location, B = Data.Walk[1].Location;
        const float Direct = FMath::PointDistToSegment(FVector(Door.X, Door.Y, 0), FVector(A.X, A.Y, 0), FVector(B.X, B.Y, 0));
        const float Mirrored = FMath::PointDistToSegment(FVector(Door.X, -Door.Y, 0), FVector(A.X, A.Y, 0), FVector(B.X, B.Y, 0));
        UE_LOG(LogTemp, Display, TEXT("OWQUEST door-to-walk distance %.1f (mirrored %.1f)"), Direct, Mirrored);
        Check(Direct < Data.Walk[0].ArrivalRadius && Mirrored > 1000.f, TEXT("world_json_convention_matches_loaded_door"));
        Check(Marcus && Marcus->GetActorLocation().Equals(Data.MarcusLocation, 0.5f)
            && FMath::Abs(FMath::FindDeltaAngleDegrees(Marcus->GetActorRotation().Yaw, Data.MarcusRotation.Yaw)) < 0.5f
            && Marcus->GetMesh()->GetSkeletalMeshAsset() && Marcus->IsPlaying(Marcus->Idle()), TEXT("marcus_placed_at_stock_pose_with_imported_mesh"));
        Check(FMath::IsNearlyEqual(Walker->GetMaxHealth(), Data.HealthForLevel(Walker->GetSkills()->GetLevel()), 0.01f)
            && Walker->GetHealth() == Walker->GetMaxHealth() && Walker->GetMaxHealth() != 400.f, TEXT("maya_health_from_formula"));
        Check(!Dummy && !bDummySpawned, TEXT("dummy_absent_before_fire_objective"));
        // The dependency fixture (save-state stand-in) adds exactly the mission's declared Dependencies, and it is
        // needed: without it the mission could not be accepted.
        {
            const auto& Mission = Impl->Slice->mission();
            const auto Declared = Mission.dependencies();
            Check(!Declared.empty() && Impl->Fixture == std::set<std::string>(Declared.begin(), Declared.end())
                && !Mission.available(std::set<std::string>()) && Mission.available(Impl->Completed),
                TEXT("dependency_fixture_is_the_missions_declared_dependencies"));
        }
        // Player side: the mission's own MissionWeapon recipe and mesh exist, it is not carried yet, and Maya starts
        // armed (slice gear at her level) with the arms shown.
        Check(MissionWeapon.Balance == UTF8_TO_TCHAR(Impl->Slice->mission().weaponDefinition().c_str()) && MissionWeapon.Level == GearLevel
            && !MissionWeapon.DamageType.IsEmpty() && UOpenWillowInventory::LoadWeaponMesh(MissionWeapon), TEXT("mission_weapon_recipe_and_mesh_found"));
        Check(Walker->GetInventory()->FindItemIndexById(MissionWeapon.Id) == INDEX_NONE, TEXT("mission_weapon_not_carried_before_lend"));
        Check(Walker->HasWeaponOut() && Walker->AreArmsShown(), TEXT("maya_starts_armed_with_arms_shown"));
        PressUse();   // far from Marcus: must do nothing
        break;
    }
    case 1:
        Check(Status() == 0, TEXT("use_key_out_of_reach_ignored"));
        {   // Marcus's own list scripts over his directive table (the data's seven entries): the Fire mission is offered to begin, none to end
            bool bRedeemable = true, bEligible = false;
            MissionScreen(bRedeemable, bEligible);
            Check(bEligible && !bRedeemable, TEXT("mission_screen_offers_fire_from_marcus_directives"));
        }
        PlacePlayer(Data.MarcusLocation + Marcus->GetActorForwardVector() * 350.f + FVector(0, 0, 20), MarcusFront + FVector(0, 0, 20));
        break;
    case 2:
        if (TestWait < 1.5f) return;
        Shot(TEXT("1_MarcusStockPose"));
        break;
    case 3:
        if (TestWait < 0.5f) return;
        // The use ray (NATIVE_USE_INTERACTION.md, UNVERIFIED): Marcus is chosen only when the view line, PlayerInteractionDistance (350 uu
        // from the data) long, reaches him; no radius or cone around the player stands in for it.
        PlacePlayer(Data.MarcusLocation + Marcus->GetActorForwardVector() * 150.f + FVector(0, 0, 20), MarcusFront);
        Check(Impl->Slice->playerInteractionDistance() > 0.f && InTalkReach(), TEXT("use_ray_reaches_marcus_in_view"));
        PlacePlayer(Data.MarcusLocation + Marcus->GetActorForwardVector() * 150.f + FVector(0, 0, 20), Data.MarcusLocation + Marcus->GetActorForwardVector() * 300.f + FVector(0, 0, 20));
        Check(!InTalkReach(), TEXT("use_ray_misses_marcus_looking_away"));
        PlacePlayer(Data.MarcusLocation + Marcus->GetActorForwardVector() * (Impl->Slice->playerInteractionDistance() + 100.f) + FVector(0, 0, 20), MarcusFront);
        Check(!InTalkReach(), TEXT("use_ray_stops_at_interaction_distance"));
        PlacePlayer(Data.MarcusLocation + Marcus->GetActorForwardVector() * 150.f + FVector(0, 0, 20), MarcusFront);
        PressUse();
        break;
    case 4:
        // Marcus's walk starts when the kickoff dialog behavior finishes: the tracker tick plays the kickoff (one tick after the accept) and
        // the dialog is triggered one kernel wake after that (NATIVE_DIALOG.md); with no audio device the behavior finishes on that wake.
        // So the walk begins a few frames after the use key, not in the frame of it (swap 4); the checks themselves are unchanged.
        if (!bAcceptChecked) {
            bAcceptChecked = true;
            Check(Status() == 1, TEXT("use_key_accepts_mission"));
            {   // The lent weapon is granted inside ActivateMission (observed in the real game, FIRE_MISSION_GOLDEN_TRACE.md): in the backpack, not equipped, and
                // not drawn (the grant is AddInventory with bReady false); Maya readies it herself before she shoots (case 6).
                const FOpenWillowWeaponItem* Held = Walker->GetInventory()->ActiveWeapon();
                bool bSlotted = false;
                for (int32 Slot = 0; Slot < UOpenWillowInventory::SlotCount; ++Slot)
                    if (const FOpenWillowWeaponItem* Item = Walker->GetInventory()->SlotItem(Slot); Item && UOpenWillowInventory::StableId(*Item) == MissionWeapon.Id) bSlotted = true;
                Check(bWeaponLent && Walker->GetInventory()->FindItemIndexById(MissionWeapon.Id) != INDEX_NONE && !bSlotted && !(Held && Held->Id == MissionWeapon.Id),
                    TEXT("mission_weapon_granted_on_accept"));
            }
            // The use key went through his OnUsed chain: it opened the mission interface (HasMissions saw the Fire mission), after nine checks that
            // all said "not enabled" (three names have no sequence, six belong to other missions)
            Check(MissionInterfacesOpened == 1 && LastInterfaceMovie == TEXT("UI_Mission.MissionInterface_Definition"), TEXT("use_chain_reaches_mission_interface"));
            // The accept emptied the screen's entry list, so the movie closed inside the status routine (observed in game; the order is the VM's, from the installed
            // script): Marcus's dismissal tag, then the interface closed, then the status event; the VM no longer holds the screen open.
            Check(DismissalTags == 1 && LastDismissalTag == TEXT("GD_Dialog_NPC.Events.VO_NPC_GenericDismissal") && MissionScreensClosed == 1 && !Impl->Slice->screenOpen()
                && DismissalAt > 0 && DismissalAt < ClosedAt && ClosedAt < ActiveAt, TEXT("accept_closes_mission_screen_with_dismissal"));
            // PlayOnUseDialog picked its tag from Marcus's own lists: Fire is eligible, nothing redeemable, so "missions available"
            Check(LastOnUseTag == TEXT("GD_Dialog_NPC.Events.VO_NPC_OnUse_MissionsAvailable"), TEXT("use_chain_picks_on_use_dialog_tag"));
            // and the stock dialog data turned it into Marcus's own line: the global tag's generic act has no entry for him, its no-match output fires
            // his DET_NPC_OnUse_MissionsAvailable on his own group (the manifest knows the AkEvent)
            Check(OnUseLines == 1 && LastOnUseLineTag == TEXT("GD_Dialog_NPCImplementation.Events_MissionGiver.DET_NPC_OnUse_MissionsAvailable")
                && LastOnUseLineAk == TEXT("Ake_VOCT_Contextual.Ak_Play_VOCT_Marcus_Quest_New") && DialogMisses == 0, TEXT("use_chain_plays_on_use_line"));
            // the chain again with the mission accepted: "all missions in progress" and its line (Fire is Active, not offered again). Run directly, not through
            // the key: Marcus has already started to walk away, so the use ray would no longer reach him in this frame.
            Impl->Slice->useMarcus(Impl->Completed);
            Pump();
            // That press opened the screen again (it lists only what is in progress); the player leaves it (Escape) before the next use, as the turn-in needs a screen
            // that opens with the redeemable entry.
            if (!Impl->Slice->closeScreen()) Fail(TEXT("the in-progress screen did not close"));
            Pump();
            Check(LastOnUseTag == TEXT("GD_Dialog_NPC.Events.VO_NPC_OnUse_AllMissionsInProgress") && OnUseLines == 2
                && LastOnUseLineTag == TEXT("GD_Dialog_NPCImplementation.Events_MissionGiver.DET_NPC_OnUse_AllMissionsInProgress")
                && LastOnUseLineAk == TEXT("Ake_VOCT_Contextual.Ak_Play_VOCT_Marcus_Quest_During"), TEXT("use_chain_plays_in_progress_line"));
            int32 NotEnabled = 0, Seen = 0;
            for (const FString& Line : LastUseCascade)
                if (Line.StartsWith(TEXT("Behavior_IsSequenceEnabled"))) { ++Seen; if (Line.EndsWith(TEXT("-> 1"))) ++NotEnabled; }
            Check(Seen == 9 && NotEnabled == 9, TEXT("use_chain_all_nine_checks_not_enabled"));
        }
        if (Waiting(MissionEventsMatched >= 1 && bWalkStarted && Marcus->IsWalking(), 2.f)) return;
        Check(MissionEventsMatched >= 1 && bWalkStarted && Marcus->IsWalking(), TEXT("installed_kismet_starts_marcus_walk"));
        Check(Marcus->IsPlaying(Marcus->WalkClip()), TEXT("marcus_walk_clip_playing"));
        break;
    case 5: {
        if (Waiting(!Marcus->IsWalking() && !Mover->DoorRunning(), 25.f)) return;
        const FOpenWillowSliceNode& Last = Data.Walk.Last();
        Check(!Marcus->IsWalking() && Marcus->NodesReached() == Data.Walk.Num()
            && FVector::Dist2D(Marcus->GetActorLocation(), Last.Location) <= Last.ArrivalRadius + 1.f, TEXT("marcus_walk_reaches_last_node"));
        TArray<int32> Motions;
        for (const int32 Motion : ArrivalMotions) if (Motion) Motions.Add(Motion);
        UE_LOG(LogTemp, Display, TEXT("OWQUEST arrival door motions: %d entries, non-zero %d; door open starts=%d ends=%d, close starts=%d ends=%d, turn-arounds=%d"),
            ArrivalMotions.Num(), Motions.Num(), Mover->DoorOpenStarts, Mover->DoorOpenEnds, Mover->DoorCloseStarts,
            Mover->DoorCloseEnds, Mover->DoorTurnArounds);
        // Opening starts at the first node; the closing request may arrive while it still opens (turn-around).
        Check(Motions == TArray<int32>({1, -1}) && Mover->DoorOpenStarts == 1 && Mover->DoorCloseEnds == 1 && Mover->DoorClosed(),
            TEXT("door_opened_then_closed_by_installed_arrival_events"));
        Check(bLookAtFromSequence, TEXT("walk_finished_output_sets_look_at_player"));
        PlacePlayer(Data.TriggerCenter + FVector(Data.TriggerRadius + Walker->GetCapsuleComponent()->GetScaledCapsuleRadius() + 100.f, 0, 0),
            Marcus->GetActorLocation());
        break;
    }
    case 6:
        if (TestWait < 2.0f) return;
        Check(Marcus->IsLookingAt(Walker, 10.f), TEXT("marcus_faces_player_after_walk"));
        {   // Test fixture standing for the inventory page's equip action: the grant left the lent pistol in the backpack, so Maya readies it into a weapon
            // slot (the first empty unlocked one, else the last: its weapon goes back to the backpack) and draws it before she can shoot with it.
            UOpenWillowInventory* Inventory = Walker->GetInventory();
            int32 Slot = Inventory->GetWeaponSlotsUnlocked() - 1;
            for (int32 Candidate = 0; Candidate < Inventory->GetWeaponSlotsUnlocked(); ++Candidate)
                if (!Inventory->SlotItem(Candidate)) { Slot = Candidate; break; }
            const int32 Index = Inventory->FindItemIndexById(MissionWeapon.Id);
            Check(Index != INDEX_NONE && Walker->EquipItem(Index, Slot) && Walker->DrawItemById(MissionWeapon.Id), TEXT("lent_pistol_readied_from_backpack_by_player"));
        }
        // Outside the stock cylinder but inside the old 700 cm stand-in radius: must not complete.
        Check(Impl->ObjectiveState(Data.TriggerObjective) == "Active" && !PlayerTouchesTrigger(), TEXT("outside_stock_cylinder_does_not_trigger"));
        PlacePlayer(Data.TriggerCenter, Data.DummyLocation);
        break;
    case 7:
        // The set follows the objective by the installed behavior's 0.5 s link delay (NATIVE_MISSION_DISPATCH.md).
        if (Waiting(Impl->Slice->mission().activeSet().find("RocksPaper_FinalObj") != std::string::npos, 3.f)) return;
        Check(Impl->ObjectiveState(Data.TriggerObjective) == "Complete"
            && Impl->Slice->mission().activeSet().find("RocksPaper_FinalObj") != std::string::npos, TEXT("stock_cylinder_touch_advances_set"));
        // Completed through the waypoint script: one applied update reached the VM controller (UpdateMissionObjective), none for Marcus's touches.
        Check(Impl->Slice->scriptObjectiveUpdates() == 1, TEXT("objective_completed_through_waypoint_script_once"));
        Check(bWeaponLent, TEXT("mission_weapon_lent"));
        {
            const FOpenWillowWeaponItem* Held = Walker->GetInventory()->ActiveWeapon();
            Check(Held && Held->Id == MissionWeapon.Id && Held->Balance == MissionWeapon.Balance && Held->Level == MissionWeapon.Level
                && Held->Damage == MissionWeapon.Damage && Held->FireRate == MissionWeapon.FireRate && Held->Magazine == MissionWeapon.Magazine
                && Held->DamageType == MissionWeapon.DamageType, TEXT("lent_pistol_drawn_with_recipe_identity_and_stats"));
            Check(Walker->HeldWeaponMesh() == MissionWeapon.MeshPath && Walker->AreArmsShown(), TEXT("lent_pistol_shows_imported_mesh_with_arms"));
        }
        break;
    case 8: {
        if (Waiting(Dummy && bDummyAttached, 3.f)) return;
        Check(Dummy && DummySpawnedAt.Equals(Data.DummyLocation, 1.f) && Dummy->IsStockPawn(), TEXT("stock_dummy_spawned_at_population_point"));
        Check(Dummy && bDummyAttached && Dummy->GetRootComponent()->GetAttachParent() == Mover->TrackCarrier(),
            TEXT("dummy_attached_to_carrier_by_installed_kismet"));
        Check(Mover->TrackRunning() || Mover->TrackForwardEnds > 0, TEXT("dummy_provider_starts_target_matinee"));
        // Attach point from data: the host's composition (socket pose on the holder's placed pose) lands where the
        // tool's own composition put it, moved by the carrier offset at attach time.
        UE_LOG(LogTemp, Display, TEXT("OWQUEST dummy attached at %s, tool's socket %s, carrier offset then %s"), *DummyAttachedAt.ToString(),
            *Data.AttachSocketWorldOracle.ToString(), *AttachCarrierOffset.ToString());
        Check(Data.bHasAttachSocket && bAttachSocketApplied && DummyAttachedAt.Equals(Data.AttachSocketWorldOracle + AttachCarrierOffset, 0.5f),
            TEXT("dummy_attached_at_holder_socket_from_manifest"));
        // Behavior_Transform ran when the Fire objective enabled its sequence: the dummy's TransformType is set and its
        // target name is the balance's transformed name for that type, not the plain display name.
        {
            const FString* Transformed = Data.DummyTransformedNames.Find(DummyTransform);
            UE_LOG(LogTemp, Display, TEXT("OWQUEST dummy TransformType %s (from %s), target name \"%s\""), *DummyTransform, *TransformSequence, *DummyTargetName());
            Check(Data.bHasDummyNames && !DummyTransform.IsEmpty() && Transformed && DummyTargetName() == *Transformed
                && *Transformed != Data.DummyDisplayName && Impl->Slice->dummy().sequenceEnabled(TCHAR_TO_UTF8(*TransformSequence)),
                TEXT("dummy_transform_type_and_target_name_from_provider"));
        }
        // Not targetable yet: the sequence that registers it is enabled by the mission a few seconds later.
        Check(Dummy && !IsRegisteredTargetable(Dummy) && TargetableCalls == 0, TEXT("dummy_not_targetable_before_register_behavior"));
        Shot(TEXT("2a_DummySpawned"));
        break;
    }
    case 9: {
        if (Waiting(Mover->TrackForwardEnds > 0, 5.f)) return;
        const auto& Keys = Data.TargetBinding->GetArrayField(TEXT("groups"))[0]->AsObject()->GetArrayField(TEXT("position"));
        auto Key = [&Keys](int32 I) { const auto& V = Keys[I]->AsObject()->GetArrayField(TEXT("value")); return FVector(V[0]->AsNumber(), V[1]->AsNumber(), V[2]->AsNumber()); };
        // The key-space delta's length is frame-independent; its world direction depends on the UNVERIFIED frame.
        const FVector KeyDelta = Key(Keys.Num() - 1) - Key(0);
        UE_LOG(LogTemp, Display, TEXT("OWQUEST target track world offset %s (key-space delta %s)"), *Mover->TrackOffset().ToString(), *KeyDelta.ToString());
        Check(Mover->TrackForwardEnds == 1 && FMath::IsNearlyEqual(Mover->TrackOffset().Size(), KeyDelta.Size(), 0.5f), TEXT("target_matinee_reaches_forward_end"));
        Check(Dummy && (Dummy->GetActorLocation() - DummyAttachedAt).Equals(Mover->TrackOffset() - AttachCarrierOffset, 0.5f), TEXT("dummy_moves_with_carrier"));
        // From the trigger centre (where the player stands for the objective) the dummy must be in the line of fire.
        FHitResult Hit;
        FCollisionQueryParams Query(SCENE_QUERY_STAT(OWQuestAim), true, Walker);
        const FVector Eye = Walker->GetActorLocation() + FVector(0, 0, 70);
        const bool bHit = Dummy && GetWorld()->LineTraceSingleByChannel(Hit, Eye, Eye + (Dummy->AimPoint() - Eye) * 1.2f, ECC_Visibility, Query);
        UE_LOG(LogTemp, Display, TEXT("OWQUEST aim trace from %s to dummy %s: hit %s (%s) at %.0f uu"), *Eye.ToString(),
            Dummy ? *Dummy->AimPoint().ToString() : TEXT("-"), bHit && Hit.GetActor() ? *Hit.GetActor()->GetName() : TEXT("nothing"),
            bHit && Hit.GetActor() && Hit.GetActor()->Tags.Num() ? *Hit.GetActor()->Tags[0].ToString() : TEXT(""), bHit ? Hit.Distance : 0.f);
        Check(bHit && Hit.GetActor() == Dummy, TEXT("dummy_hittable_by_visibility_trace"));
        if (Dummy && Walker->GetController()) Walker->GetController()->SetControlRotation((Dummy->AimPoint() - Eye).Rotation());
        break;
    }
    case 10:
        if (TestWait < 1.5f) return;
        Shot(TEXT("2_RangeDummy"));
        break;
    case 11:
        if (TestWait < 0.2f) return;
        // A second view that includes Marcus at the end of his walk and the dummy.
        PlacePlayer(Data.TriggerCenter - FVector(0, 450.f, 0), (Marcus->GetActorLocation() + (Dummy ? Dummy->GetActorLocation() : Data.DummyLocation)) * 0.5f);
        break;
    case 12:
        if (TestWait < 1.5f) return;
        Shot(TEXT("3_RangeMarcusAndDummy"));
        break;
    case 13: {
        if (TestWait < 0.5f || !Dummy) { if (!Dummy) Fail(TEXT("dummy missing")); return; }
        // Behavior_RegisterTargetable (bUnregister false) from the sequence the mission enabled: the dummy is in the
        // host's targetable list before it is shot.
        if (!bDummyTargetable && TestWait < 10.f) return;
        Check(IsRegisteredTargetable(Dummy) && TargetableCalls == 1 && Impl->Slice->dummy().sequenceEnabled(TCHAR_TO_UTF8(*TargetableSequence)),
            TEXT("dummy_registered_targetable_by_provider"));
        // Real shots from the trigger centre (where the aim trace hit the dummy). First the wrong element: an
        // equipped slice gun whose stock damage type is not the lent pistol's.
        PlacePlayer(Data.TriggerCenter, Dummy->AimPoint());
        for (int32 Slot = 0; Slot < UOpenWillowInventory::SlotCount && WrongId.IsEmpty(); ++Slot)
            if (const FOpenWillowWeaponItem* Item = Walker->GetInventory()->SlotItem(Slot);
                Item && Item->Id != MissionWeapon.Id && Item->DamageType != MissionWeapon.DamageType)
            {
                WrongId = UOpenWillowInventory::StableId(*Item);
                WrongType = Item->DamageType;
            }
        if (WrongId.IsEmpty() || !Walker->DrawItemById(WrongId)) { Fail(TEXT("no equipped gun with another damage type for the wrong-element shot")); return; }
        break;
    }
    case 14: {
        if (TestWait < 0.5f) return;
        // Bullets fly (OpenWillowProjectile: the press launches them, the damage arrives about distance / speed later), so the case fires, waits for
        // the landing and checks (phases 0 and 1), then does the same with the equipped multi-pellet gun (phases 2 and 3). The counts come from
        // OpenWillowFiring::Counts, the firing code's own tally.
        if (ShotPhase == 0)
        {
            ShotsBefore = DummyShots;
            CountsBefore = OpenWillowFiring::Counts;
            Walker->FireOnce();
            const FOpenWillowWeaponItem* Wrong = Walker->GetInventory()->ActiveWeapon();
            // A recipe without a firing block keeps the instant trace; a light bullet or rocket must not land in the pressing frame.
            const bool bTrace = !Wrong || !Wrong->Firing.bLoaded || Wrong->Firing.Kind == EOpenWillowShotKind::Trace;
            Check(bTrace || (DummyShots == ShotsBefore && OpenWillowFiring::Counts.Launches > CountsBefore.Launches),
                TEXT("bullet_has_not_landed_in_the_pressing_frame"));
            ShotPhase = 1;
            return;
        }
        if (ShotPhase == 1)
        {
            if (Waiting(ProjectilesInFlight(GetWorld()) == 0 && DummyShots > ShotsBefore, 3.f)) return;
            const FOpenWillowWeaponItem* Wrong = Walker->GetInventory()->ActiveWeapon();
            const int32 Landed = DummyShots - ShotsBefore;
            const int32 Hits = OpenWillowFiring::Counts.TargetHits - CountsBefore.TargetHits;
            // Was "DummyShots == Before + 1": still exactly one damage call for a one-pellet gun; a multi-pellet gun makes one per pellet that hits.
            Check((Wrong && Wrong->Projectiles > 1 ? Landed >= 1 : Landed == 1) && Landed == Hits && LastDummyDamageType == WrongType
                && Status() == 1 && Impl->ObjectiveState(Data.DummyObjective) == "Active", TEXT("wrong_element_shot_reaches_dummy_and_does_not_complete"));
            // Test fixture: the slice shotgun is in the backpack and the weapon slots are full, so ready it into the first slot that holds neither the
            // lent pistol nor the wrong-element gun (that gun goes back to the backpack, and back into its slot after the checks).
            UOpenWillowInventory* Inventory = Walker->GetInventory();
            const int32 Slots = Inventory->GetWeaponSlotsUnlocked();
            for (int32 Slot = 0; Slot < Slots && ShotgunSlot == INDEX_NONE; ++Slot)
            {
                const FOpenWillowWeaponItem* Item = Inventory->SlotItem(Slot);
                if (!Item) { ShotgunSlot = Slot; break; }
                if (Item->Id != MissionWeapon.Id && UOpenWillowInventory::StableId(*Item) != WrongId) { ShotgunSlot = Slot; DisplacedId = UOpenWillowInventory::StableId(*Item); }
            }
            for (const FOpenWillowWeaponItem& Item : Inventory->Items())
                if (ShotgunId.IsEmpty() && Item.Projectiles > 1) ShotgunId = UOpenWillowInventory::StableId(Item);
            const int32 Index = Inventory->FindItemIndexById(ShotgunId);
            if (ShotgunSlot == INDEX_NONE || Index == INDEX_NONE || !Walker->EquipItem(Index, ShotgunSlot) || !Walker->DrawItemById(ShotgunId))
            {
                UE_LOG(LogTemp, Display, TEXT("OWQUEST pellet gun fixture: slot %d, item %s (backpack index %d), level %d"), ShotgunSlot, *ShotgunId, Index, Walker->GetSkills()->GetLevel());
                Fail(TEXT("no multi-pellet gun to ready for the pellet checks"));
                return;
            }
            ShotPhase = 2;
            TestWait = 0;
            return;
        }
        if (ShotPhase == 2)
        {
            if (TestWait < 0.3f) return;
            const FOpenWillowWeaponItem* Held = Walker->GetInventory()->ActiveWeapon();
            ShotsBefore = DummyShots;
            MagazineBefore = Held ? UOpenWillowInventory::MagazineLeft(*Held) : 0;
            PressDamage = Held ? Walker->ShotDamage(*Held, GetWorld()->GetTimeSeconds()) : 0.f;
            CountsBefore = OpenWillowFiring::Counts;
            Walker->FireOnce();
            ShotPhase = 3;
            TestWait = 0;
            return;
        }
        if (Waiting(ProjectilesInFlight(GetWorld()) == 0 && DummyShots > ShotsBefore, 3.f)) return;
        {
            const FOpenWillowWeaponItem* Held = Walker->GetInventory()->ActiveWeapon();
            const int32 Pellets = Held ? Held->Projectiles : 0;
            const int32 Landed = DummyShots - ShotsBefore;
            const int32 Fired = OpenWillowFiring::Counts.Pellets - CountsBefore.Pellets;
            const int32 Hits = OpenWillowFiring::Counts.TargetHits - CountsBefore.TargetHits;
            const float Direct = OpenWillowFiring::Counts.Direct - CountsBefore.Direct;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST shotgun press: %d pellet(s), %d landed on the dummy (%d direct hits, %.2f direct damage), magazine %d -> %d"),
                Fired, Landed, Hits, Direct, MagazineBefore, Held ? UOpenWillowInventory::MagazineLeft(*Held) : 0);
            Check(Held && Pellets > 1 && Fired == Pellets && OpenWillowFiring::Counts.Shots == CountsBefore.Shots + 1,
                TEXT("multi_pellet_gun_fires_its_pellet_count_in_one_press"));
            Check(Held && MagazineBefore - UOpenWillowInventory::MagazineLeft(*Held) == UOpenWillowInventory::ShotCostRounds(*Held),
                TEXT("ammo_taken_once_per_press_not_per_pellet"));
            // One damage call per pellet that hit (the dummy's own TakeDamage count and the firing code's direct-hit count agree), each with the
            // per-pellet damage fixed at the press (the card's, no level-up scale is running here).
            Check(Landed >= 1 && Landed <= Pellets && Landed == Hits && FMath::IsNearlyEqual(Direct, Hits * PressDamage, 1e-2f),
                TEXT("damage_called_once_per_pellet_that_hits"));
            Shot(TEXT("3b_ShotgunPelletsLanded"));
        }
        ShotPhase = 0;
        Walker->UnequipSlot(ShotgunSlot);
        if (!DisplacedId.IsEmpty()) Walker->EquipItem(Walker->GetInventory()->FindItemIndexById(DisplacedId), ShotgunSlot);
        Walker->DrawItemById(MissionWeapon.Id);
        break;
    }
    case 15: {
        if (ShotPhase == 0 && TestWait < 0.5f) return;
        if (ShotPhase == 0)
        {
            // Test fixture: the shotgun pellets of case 14 kick the dummy (its wobble tilts the hit capsule, its knock slides it), so wait for it
            // to stand still and aim at where it is now. Fired while it still swung, the bullet once passed the capsule and hit the wall behind;
            // only the explosion's splash reached the dummy, one hit where the check expects two (2026-11-10, run Z51).
            if (Waiting(!Dummy || (Dummy->WobbleSize() < 0.5f && Dummy->WobbleSpeed() < 2.f && Dummy->KnockVelocity().IsNearlyZero(1.f)), 4.f)) return;
            if (Dummy && Walker->GetController())
                Walker->GetController()->SetControlRotation((Dummy->AimPoint() - (Walker->GetActorLocation() + FVector(0, 0, 70))).Rotation());
            UE_LOG(LogTemp, Display, TEXT("OWQUEST lent pistol press after %.2f s: dummy wobble %.2f (speed %.2f), knock %.2f"), TestWait,
                Dummy ? Dummy->WobbleSize() : -1.f, Dummy ? Dummy->WobbleSpeed() : -1.f, Dummy ? float(Dummy->KnockVelocity().Size()) : -1.f);
            ShotsBefore = DummyShots;
            Walker->FireOnce();
            ShotPhase = 1;
            TestWait = 0;
            return;
        }
        if (Waiting(ProjectilesInFlight(GetWorld()) == 0 && DummyShots > ShotsBefore, 3.f)) return;
        ShotPhase = 0;
        // The bullet's own hit comes first with the gun's damage type. The Maliwan firing mode then explodes at the impact
        // (its Behavior_Explode; WEAPON_PROJECTILES.md section 8), a second hit with the element's impact damage type.
        // Only the shot's own hits count: a burn tick from the status effect the shot applied can land before this check (2026-11-10, run Z52)
        // and is not part of the shot. Every shot hit must carry the lent pistol's damage type.
        const FOpenWillowWeaponItem* LentHeld = Walker->GetInventory()->ActiveWeapon();
        int32 ShotHits = 0, ShotHitsOfType = 0, Ticks = 0;
        for (int32 Hit = ShotsBefore; Hit < DummyDamageTypes.Num(); ++Hit)
        {
            if (DummyStatusTicks.IsValidIndex(Hit) && DummyStatusTicks[Hit]) { ++Ticks; continue; }
            ++ShotHits;
            if (DummyDamageTypes[Hit] == MissionWeapon.DamageType) ++ShotHitsOfType;
        }
        const int32 Expected = 1 + (LentHeld && LentHeld->Firing.bLoaded && LentHeld->Firing.Splash.bValid ? 1 : 0);
        UE_LOG(LogTemp, Display, TEXT("OWQUEST lent pistol shot: %d shot hit(s) on the dummy (%d expected), %d with its damage type, %d status tick(s) ignored"),
            ShotHits, Expected, ShotHitsOfType, Ticks);
        Check(ShotHits == Expected && ShotHitsOfType == ShotHits, TEXT("lent_pistol_shot_carries_its_damage_type_to_dummy"));
        Check(Status() == 2, TEXT("incendiary_shot_completes_fire_objective_via_dummy_provider"));
        // The lent pistol is NOT taken back when the objective completes (observed in the real game: it stays in the inventory until the turn-in's
        // CompleteMission); it is removed at the turn-in (case 18). Meaning changed: this check was "mission_weapon_removed_after_objective".
        const FOpenWillowWeaponItem* Held = Walker->GetInventory()->ActiveWeapon();
        Check(bWeaponLent && Walker->GetInventory()->FindItemIndexById(MissionWeapon.Id) != INDEX_NONE && Held && Held->Id == MissionWeapon.Id,
            TEXT("mission_weapon_kept_after_objective"));
        break;
    }
    case 16:
        if (Waiting(Mover->TrackReverseEnds > 0 && bDummyDestroyedBySequence, 12.f)) return;
        Check(bSendBackFromProvider && Mover->TrackReverseEnds == 1 && Mover->TrackOffset().IsNearlyZero(0.5f), TEXT("dummy_provider_sends_target_back"));
        Check(bDummyDestroyedBySequence && !Dummy, TEXT("installed_kismet_destroys_dummy_at_reverse_end"));
        PlacePlayer(Marcus->GetActorLocation() + Marcus->GetActorForwardVector() * 150.f + FVector(0, 0, 20), Marcus->GetActorLocation());
        break;
    case 17: {
        if (TestWait < 0.5f) return;
        // Test fixture: top Maya's experience up so that this reward must cross the next level's requirement.
        UOpenWillowSkills* Skills = Walker->GetSkills();
        const int64 Gap = UOpenWillowSkills::ExperienceForLevel(Skills->GetLevel() + 1) - Skills->GetExperience() - Data.MissionXp(RegionStage);
        if (Gap > 0) Skills->AddExperience(Gap);
        UE_LOG(LogTemp, Display, TEXT("OWQUEST test fixture: experience +%lld so the reward crosses level %d"), FMath::Max<int64>(Gap, 0), Skills->GetLevel() + 1);
        PointsBeforeReward = Skills->AvailablePoints();
        BackpackBeforeReward = Walker->GetInventory()->BackpackCount();
        OnUseLinesBeforeTurnIn = OnUseLines;
        {   // Ready to turn in: the redeemable list offers it, the eligible list no longer does
            bool bRedeemable = false, bEligible = true;
            MissionScreen(bRedeemable, bEligible);
            Check(bRedeemable && !bEligible, TEXT("mission_screen_offers_turn_in_when_ready"));
        }
        // Test fixture: half of Maya's health off, so that what the level-up does to current health is visible.
        FHitResult Hit;
        // (plus the shield, which takes hits first, when one is equipped)
        UGameplayStatics::ApplyPointDamage(Walker, Walker->GetShield() + Walker->GetMaxHealth() * 0.5f, FVector::ForwardVector, Hit, nullptr, nullptr,
            UDamageType::StaticClass());
        HealthBeforeReward = Walker->GetHealth();
        MaxHealthBeforeReward = Walker->GetMaxHealth();
        PressUse();
        break;
    }
    case 18: {
        Check(Status() == 3, TEXT("use_key_turns_in_mission"));
        {   // CompleteMission removes the lent weapon (observed in the real game), from the slot and the backpack alike; the held weapon is then another one
            const FOpenWillowWeaponItem* Held = Walker->GetInventory()->ActiveWeapon();
            Check(!bWeaponLent && Walker->GetInventory()->FindItemIndexById(MissionWeapon.Id) == INDEX_NONE && Held && Held->Id != MissionWeapon.Id,
                TEXT("mission_weapon_removed_on_turn_in"));
        }
        // OnPlayerTurnedInMission -> PlayMissionTurnedInDialog raised the generic turned-in tag on Marcus's component (observed in the real game, same tag);
        // the stock data gives that event no line for this mission (its switch on the object parameter takes its no-match output).
        Check(TurnInDialogTags == 1 && LastTurnInTag == TEXT("GD_Dialog_NPC.Events.VO_NPC_MissionTurnedIn"), TEXT("turn_in_raises_mission_turned_in_dialog_tag"));
        // the same press's on-use tag, now that Marcus's redeemable list has the mission: "mission complete"
        Check(LastOnUseTag == TEXT("GD_Dialog_NPC.Events.VO_NPC_OnUse_MissionComplete"), TEXT("use_chain_on_use_tag_follows_mission_state"));
        // the data says this one is silent: the generic event has no act of its own, its link-table template act has an entry for Marcus without an audio
        // event, so the act passes through and no line is chosen
        Check(OnUseLines == OnUseLinesBeforeTurnIn, TEXT("use_chain_mission_complete_is_silent"));
        Check(Rewards == 1, TEXT("xp_reward_granted_once"));
        const UOpenWillowSkills* Skills = Walker->GetSkills();
        // The tool's own amount at the same stage (tools/slice_values.py evaluates the same truncation rule apart).
        const int32* Oracle = Data.XpCandidateByLevel.Find(RegionStage);
        Check(Oracle && LastXpAmount == *Oracle && LastXpAmount > 0 && Skills->GetExperience() == ExperienceBeforeReward + LastXpAmount,
            TEXT("xp_amount_is_truncated_rule_at_region_stage"));
        {
            // The turn-in ran ServerCompleteMission: UpdateMissionStatus(Complete) -> ServerGrantMissionRewards -> ExpEarn once, with
            // GetExperienceReward's own amount (the host applied exactly that), source 4 (SideMissionAward; the Fire mission is not
            // plot-critical), ExpType omitted (UNVERIFIED, bridge note).
            const auto& Earned = Impl->Slice->expEarned();
            Check(Earned.size() == 1 && Earned[0].amount == LastXpAmount && Earned[0].source == 4 && Earned[0].type == -1
                && Impl->Slice->scriptPlayerStatus() == 4, TEXT("turn_in_script_calls_exp_earn_side_mission_award"));
            // The pool update ran the script ExpLevelUp on the VM controller: its level is the host's after the same experience.
            Check(Impl->Slice->scriptPlayerLevel() == Skills->GetLevel() && Skills->GetLevel() == LevelBeforeReward + 1,
                TEXT("script_pool_update_levels_up_to_host_level"));
        }
        Check(Skills->GetLevel() == LevelBeforeReward + 1 && Skills->GetExperience() >= UOpenWillowSkills::ExperienceForLevel(Skills->GetLevel())
            && Skills->AvailablePoints() == PointsBeforeReward + (Skills->GetLevel() >= 5 ? 1 : 0), TEXT("xp_reward_levels_up_when_requirement_met"));
        Check(FMath::IsNearlyEqual(Walker->GetMaxHealth(), Data.HealthForLevel(Skills->GetLevel()), 0.01f)
            && Walker->GetMaxHealth() > MaxHealthBeforeReward, TEXT("level_up_sets_formula_max_health"));
        // Swap 7: the script's own numbers (the host's formulas above are the oracle). One SkillPoints event, equal to what the host's level change earned
        // (a point at level 9: the data's "1 from level 5"); the VM's maximum health for the new level, from the health pool's data, equals the host's.
        Check(ScriptSkillPointEvents == 1 && ScriptSkillPoints == Skills->AvailablePoints() - PointsBeforeReward && ScriptSkillPoints == (Skills->GetLevel() >= 5 ? 1 : 0),
            TEXT("script_level_up_awards_skill_points"));
        Check(ScriptMaxHealth > 0.f && FMath::IsNearlyEqual(ScriptMaxHealth, Walker->GetMaxHealth(), 0.01f)
            && FMath::IsNearlyEqual(ScriptMaxHealth, Data.HealthForLevel(Skills->GetLevel()), 0.01f), TEXT("script_level_up_sets_max_health"));
        Check(HealthBeforeReward < MaxHealthBeforeReward && Walker->GetHealth() == Walker->GetMaxHealth(), TEXT("level_up_refills_current_health"));
        // Swap 7b: the refill is the VM's. Maya was at half health when the reward came; the VM's pool kept that current value through its rebase (not full, so it
        // did not follow) and the OnLevelUp behavior refilled it: the one Health event is full at the new level's maximum, and the walker holds exactly those numbers.
        Check(ScriptHealthEvents == 1 && HealthBeforeReward < MaxHealthBeforeReward * 0.51f && ScriptHealthCurrent == ScriptHealthMaximum
            && FMath::IsNearlyEqual(ScriptHealthMaximum, Data.HealthForLevel(Skills->GetLevel()), 0.01f)
            && Walker->GetHealth() == ScriptHealthCurrent && Walker->GetMaxHealth() == ScriptHealthMaximum && !Walker->IsHealthHeldForScript(),
            TEXT("script_level_up_refills_health"));
        {
            // Swap 10: OnLevelUp's first skill also scales the action-skill cooldown's current value by the pool's minimum (data: 0 for Maya), so the one ActionSkillCooldown
            // event is Scale by 0 and the host's cooldown reads 0 after it (observed in game by lane L3: the cooldown went to 0 in the level-up call, held or draining).
            // No cooldown is running at this point of the suite (the cast steps come later), so what is checked is the event and the host's application; the modifier on a running
            // cooldown is the check action_skill_cooldown_modifier_scales_then_resets_host_cooldown in the Phaselock steps.
            Check(ScriptCooldownEvents == 1 && ScriptCooldownType == 0 && ScriptCooldownValue == 0.f && CooldownAfterScript == 0.f && Walker->PhaselockRemaining() == 0.f,
                TEXT("level_up_resets_action_skill_cooldown"));
        }
        {
            // Swap 9: the level-up came from earned experience, so it is natural: ExpLevelUp(false) -> OnExpLevelChange(true, true) ran OnLevelUpNaturally, whose
            // timed skill scales the held weapon's damage. One event, with the numbers the stock skill carries (+1.0, 30 s: the in-game observation), and the
            // walker applies it by the stack rule: the Scale joins the weapon's up-sum, (base + pre) x (1 + up + s) / (1 + down) + post, which is the observed
            // (S + s) / S; the card damage once the duration is over. The suite does not wait 30 s of game time: the expiry is evaluated just before and at the end time.
            const FOpenWillowWeaponItem* Held = Walker->GetInventory()->ActiveWeapon();
            const float EndAt = ScriptDamageScaleAt + ScriptDamageScaleSeconds;
            const float Now = GetWorld()->GetTimeSeconds();
            const float Expected = Held && Held->bHasDamageStack
                ? (Held->DamageBase + Held->DamagePreAdd) * (1.f + Held->DamageScaleUp + ScriptDamageScale) / (1.f + Held->DamageScaleDown) + Held->DamagePostAdd : -1.f;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST natural level-up weapon damage: card %.3f, with the scale %.3f (stack formula %.3f, factor %.3f), after %.3f"),
                Held ? Held->Damage : -1.f, Held ? Walker->ShotDamage(*Held, Now) : -1.f, Expected, Held && Held->Damage > 0.f ? Expected / Held->Damage : 0.f,
                Held ? Walker->ShotDamage(*Held, EndAt) : -1.f);
            Check(ScriptDamageScaleEvents == 1 && FMath::IsNearlyEqual(ScriptDamageScale, 1.f, 1e-4f) && FMath::IsNearlyEqual(ScriptDamageScaleSeconds, 30.f, 1e-3f)
                && Held && Held->bHasDamageStack && Held->Damage > 0.f && Now < EndAt
                && FMath::IsNearlyEqual((Held->DamageBase + Held->DamagePreAdd) * (1.f + Held->DamageScaleUp) / (1.f + Held->DamageScaleDown) + Held->DamagePostAdd, Held->Damage, 1e-3f)
                && FMath::IsNearlyEqual(Walker->ShotDamage(*Held, Now), Expected, 1e-3f) && Expected > Held->Damage
                && FMath::IsNearlyEqual(Walker->ShotDamage(*Held, EndAt - 0.01f), Expected, 1e-3f)
                && Walker->ShotDamage(*Held, EndAt) == Held->Damage && Walker->WeaponDamageScaleAt(EndAt + 1.f) == 0.f,
                TEXT("natural_level_up_scales_weapon_damage_for_duration"));
        }
        {
            // Stock data: the Fire mission's reward is experience only. The script's GetItemRewardsForPlayer found no item (it would list a
            // "not implemented" entry otherwise), and the host spawned no pickup and added nothing to the backpack.
            TArray<AActor*> Pickups;
            UGameplayStatics::GetAllActorsOfClass(GetWorld(), AOpenWillowInventoryPickup::StaticClass(), Pickups);
            bool bItemRewardListed = false;
            for (const auto& Stub : Impl->Slice->scriptStubs()) bItemRewardListed |= Stub.find("GetItemRewardsForPlayer") != std::string::npos;
            Check(Rewards == 1 && !bItemRewardListed && Pickups.IsEmpty() && Walker->GetInventory()->BackpackCount() == BackpackBeforeReward,
                TEXT("turn_in_gives_no_item_reward"));
        }
        ClosesBeforeConfirm = MissionScreensClosed;
        DismissalsBeforeConfirm = DismissalTags;
        bRewardWaited = RewardPages == 1 && RewardsReceived == 0 && Impl->Slice->rewardPending() && Impl->Slice->scriptPlayerNeedsRewards();
        PressUse();   // the reward page is up and waits: the key confirms it (and closes the screen); the key press after that gets Marcus's "no missions" line (step 19)
        break;
    }
    case 19:
        if (!bRewardChecked) {
            // The turn-in showed the reward page (the screen was open) and did not receive the reward: it was still pending when the key went in; that press confirmed
            // it exactly once (the controller's MissionRewardsReceived) and the screen closed again.
            bRewardChecked = true;
            Check(bRewardWaited && RewardPages == 1 && RewardsReceived == 1 && !Impl->Slice->rewardPending() && !Impl->Slice->screenOpen() && MissionScreensClosed == ClosesBeforeConfirm + 1 && DismissalTags == DismissalsBeforeConfirm + 1
                && !Impl->Slice->scriptPlayerNeedsRewards(), TEXT("reward_received_on_confirmation"));
            PressUse();   // nothing left to turn in: the key reaches Marcus, who answers with his "no missions" line
            return;
        }
        Check(Status() == 3 && Rewards == 1, TEXT("turn_in_not_repeatable"));
        // the press after completion: nothing left, so "no missions" and Marcus's line for it
        Check(LastOnUseTag == TEXT("GD_Dialog_NPC.Events.VO_NPC_OnUse_NoMissions") && OnUseLines == OnUseLinesBeforeTurnIn + 1
            && LastOnUseLineAk == TEXT("Ake_VOCT_Contextual.Ak_Play_VOCT_Marcus_Quest_No_New"), TEXT("use_chain_plays_no_missions_line"));
        PlacePlayer(Data.OracleDeathLocation, Data.DummyLocation);
        break;
    case 20: {
        if (TestWait < 0.5f) return;
        // Health 0 puts Maya down first (Fight For Your Life); with no kill she bleeds out and the respawn follows
        // 2.28 + 12 + 5.76 s later (OpenWillowWalker's down state), so the step waits for it.
        if (RespawnsBeforeDeath < 0)
        {
            RespawnsBeforeDeath = Respawns;
            // A known purse for the fee's checks in the next step; put back there.
            Walker->GetInventory()->GetMoney(MoneyBeforeFee);
            Walker->GetInventory()->SetMoney(2000);
            FHitResult Hit;
            UGameplayStatics::ApplyPointDamage(Walker, 1.0e6f, FVector::ForwardVector, Hit, nullptr, nullptr, UDamageType::StaticClass());
            Check(Walker->IsDown() && Walker->GetHealth() == 0.f && Respawns == RespawnsBeforeDeath, TEXT("player_death_goes_down_first"));
            return;
        }
        // The respawn is counted at the station, after the respawn's hold.
        if (Waiting(Respawns > RespawnsBeforeDeath, 0.5f + 25.f + FOpenWillowRespawnTimeline::HoldSeconds)) return;
        const int32 Before = RespawnsBeforeDeath;
        Check(Respawns == Before + 1, TEXT("player_death_triggers_respawn"));
        Check(Walker->GetActorLocation().Equals(Data.OracleRespawnLocation, 1.f), TEXT("respawn_at_decoded_station_exit_point"));
        Check(Walker->GetHealth() == Walker->GetMaxHealth(), TEXT("respawn_restores_formula_health"));
        Check(Status() == 3, TEXT("mission_state_survives_respawn"));
        break;
    }
    case 21:
        // The step also waits for the fee: 11.8 s after the respawn.
        if (Waiting(Walker->GetCharacterMovement()->IsMovingOnGround() && Walker->GetRespawnFee().LastShownAt() >= 0.f, 16.f)) return;
        UE_LOG(LogTemp, Display, TEXT("OWQUEST after respawn: %s grounded=%d"), *Walker->GetActorLocation().ToString(),
            Walker->GetCharacterMovement()->IsMovingOnGround());
        Check(Walker->GetCharacterMovement()->IsMovingOnGround(), TEXT("respawn_point_is_standable"));
        {
            const FOpenWillowRespawnFee& Fee = Walker->GetRespawnFee();
            int32 MoneyNow = 0;
            Walker->GetInventory()->GetMoney(MoneyNow);
            UE_LOG(LogTemp, Display, TEXT("OWQUEST respawn fee %d, purse %d, %.2f s after the respawn, message %s"), Fee.LastFee(), MoneyNow, Fee.LastChargedAt() - Walker->LastRespawnAt(), *Fee.MessageBody());
            Check(Fee.LastFee() == 140 && MoneyNow == 1860, TEXT("respawn_charges_the_fee"));
            Check(FMath::IsNearlyEqual(Fee.LastChargedAt() - Walker->LastRespawnAt(), FOpenWillowRespawnFee::DelaySeconds, 0.5f), TEXT("respawn_fee_at_the_recorded_delay"));
            Check(Fee.IsMessageShown(GetWorld()->GetTimeSeconds()) && Fee.MessageBody().Contains(TEXT("140")) && !Fee.MessageTitle().IsEmpty(), TEXT("respawn_message_shows_the_fee"));
            Check(!Fee.IsMessageShown(Fee.LastShownAt() + FOpenWillowRespawnFee::MessageSeconds + 0.01f), TEXT("respawn_message_ends_by_itself"));
            // With the install here the game's own text must have loaded: the neutral stand-in would pass the check above, and hide a broken lookup.
            const FString IniFile = FOpenWillowRespawnFee::LocalizationFile();
            const bool bInstall = !IniFile.IsEmpty() && FPaths::FileExists(IniFile);
            UE_LOG(LogTemp, Display, TEXT("OWQUEST respawn message text: %s (install file present: %d)"), Fee.MessageFromInstall() ? TEXT("the game's") : TEXT("the neutral stand-in"), bInstall);
            Check(!bInstall || Fee.MessageFromInstall(), TEXT("respawn_message_uses_the_installed_text"));
            Shot(TEXT("5b_RespawnMessage"));
            Walker->GetInventory()->SetMoney(MoneyBeforeFee);
        }
        break;
    case 22:
        if (!RunPhaselockTest()) return;
        break;
    case 23:
        UE_LOG(LogTemp, Display, TEXT("OWQUEST dialog lookups=%d misses=%d played=%d"), DialogLookups, DialogMisses, DialogPlayed);
        Check(DialogLookups > 0 && DialogMisses == 0 && DialogPlayed == 0, TEXT("dialog_hook_finds_every_line_and_plays_nothing"));
        UE_LOG(LogTemp, Display, TEXT("OWQUEST dialog AkEvent mismatches=%d"), DialogAkMismatches);
        Check(DialogAkMismatches == 0, TEXT("dialog_lines_name_the_manifest_ak_events"));
        Check(FPaths::FileExists(SavePath), TEXT("save_file_written"));
        // The Phaselock steps' fixture raised the level with SetLevel; this frame's tick has applied it to health.
        Check(FMath::IsNearlyEqual(Walker->GetMaxHealth(), Data.HealthForLevel(Walker->GetSkills()->GetLevel()), 0.01f),
            TEXT("max_health_follows_level_after_fixtures"));
        {
            // The save written this frame (Pump) holds the progression Maya ends the run with: the resume run's reference.
            FString Text;
            TSharedPtr<FJsonObject> Saved;
            const TSharedPtr<FJsonObject>* Progression = nullptr;
            const bool bRead = FFileHelper::LoadFileToString(Text, *SavePath) && FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Saved)
                && Saved && Saved->TryGetObjectField(TEXT("progression"), Progression);
            UE_LOG(LogTemp, Display, TEXT("OWQUEST saved progression: %s"), bRead ? *JsonText(*Progression) : TEXT("none"));
            Check(bRead && JsonText(*Progression) == JsonText(Walker->GetSkills()->ProgressionJson()), TEXT("save_holds_final_progression"));
            // Mission level: Sanctuary's stage from the data bounds (no stand-in), fixed at session start from the
            // -owlevel start level, unchanged by the later level-ups, and kept in the save.
            int32 StartLevel = 0, SavedStage = 0;
            FParse::Value(FCommandLine::Get(), TEXT("owlevel="), StartLevel);
            const TSharedPtr<FJsonObject>* Stage = nullptr;
            if (bRead && Saved->TryGetObjectField(TEXT("region_stage"), Stage)) (*Stage)->TryGetNumberField(TEXT("stage"), SavedStage);
            const auto& Completed = Impl->Completed;
            const int32 Expected = Data.RegionStage(StartLevel, [&Completed](const FString& Mission) { return Completed.count(TCHAR_TO_UTF8(*Mission)) > 0; });
            UE_LOG(LogTemp, Display, TEXT("OWQUEST region stage %d (fixed at level %d, now level %d), expected %d, saved %d, data bounds %d"),
                RegionStage, RegionStageLevel, Walker->GetSkills()->GetLevel(), Expected, SavedStage, Data.bRegionStageFromData);
            Check(Data.bRegionStageFromData && !bRegionStageFromSave && RegionStageLevel == StartLevel && RegionStage == Expected
                && RegionStage >= Data.StageDefaultMin && RegionStage <= Data.StageDefaultMax && SavedStage == RegionStage,
                TEXT("mission_level_is_region_stage_fixed_at_start"));
        }
        {
            // Level curve: the compiled-in curve and the one evaluated from world.json's formula both equal the tool's
            // independent float evaluation (values.xp.required_points_by_level).
            const auto& Points = Data.XpRequiredPointsByLevel;
            bool bSame = Points.Num() >= 10;
            for (const auto& Pair : Points)
                bSame &= UOpenWillowSkills::ExperienceForLevel(Pair.Key) == Pair.Value && UOpenWillowSkills::RequiredExperience(
                    float(Data.XpMultiplier), float(Data.XpPower), float(Data.XpOffset), Pair.Key) == Pair.Value;
            Check(bSame, TEXT("level_curve_matches_tool_integer_curve"));
        }
        UE_LOG(LogTemp, Display, TEXT("OWQUESTTEST SUMMARY result=%s checks=%d errors=%d mode=first_run"), Errors ? TEXT("FAIL") : TEXT("PASS"), Checks, Errors);
        FPlatformMisc::RequestExit(false);
        bTesting = false;
        return;
    // Resume run: after loading the completed save, the use key near Marcus must not re-accept.
    case 100:
        if (TestWait < 0.5f) return;
        PressUse();
        break;
    case 101:
        Check(Status() == 3 && Rewards == 1, TEXT("resume_completed_mission_cannot_be_reaccepted"));
        Check(FMath::IsNearlyEqual(Walker->GetMaxHealth(), Data.HealthForLevel(Walker->GetSkills()->GetLevel()), 0.01f), TEXT("resume_health_from_formula"));
        Check(!Dummy && !bDummySpawned, TEXT("resume_no_dummy_for_completed_mission"));
        {
            // The stage fixed in the first run comes back from the save; it is not recomputed from the resumed level.
            const auto& Completed = Impl->Completed;
            auto IsComplete = [&Completed](const FString& Mission) { return Completed.count(TCHAR_TO_UTF8(*Mission)) > 0; };
            UE_LOG(LogTemp, Display, TEXT("OWQUEST resumed region stage %d (fixed at level %d); a fresh computation at level %d would give %d"),
                RegionStage, RegionStageLevel, Walker->GetSkills()->GetLevel(), Data.RegionStage(Walker->GetSkills()->GetLevel(), IsComplete));
            Check(bRegionStageFromSave && RegionStage >= 1 && RegionStage == Data.RegionStage(RegionStageLevel, IsComplete),
                TEXT("resume_region_stage_from_save"));
        }
        UE_LOG(LogTemp, Display, TEXT("OWQUESTTEST SUMMARY result=%s checks=%d errors=%d mode=resume"), Errors ? TEXT("FAIL") : TEXT("PASS"), Checks, Errors);
        FPlatformMisc::RequestExit(false);
        bTesting = false;
        return;
    default:
        Fail(TEXT("unknown quest test step"));
        return;
    }
    ++TestStep;
    TestWait = 0;
}

// Phaselock against the action-skill manifest, after the mission (the reward's level-up left Maya skill points).
bool UOpenWillowQuest::RunPhaselockTest()
{
    PhaselockWait += GetWorld()->GetDeltaSeconds();
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    UOpenWillowSkills* Skills = Walker->GetSkills();
    const FOpenWillowPhaselockData& P = Walker->GetPhaselockData();
    const FOpenWillowPhaselockTimeline Base = P.Timeline(P.LockDuration(0), P.TargetTimeScale(false));
    auto Next = [this] { ++PhaselockStep; PhaselockWait = 0; };
    auto Aim = [Walker](const FVector& Point)
    {
        if (Walker->GetController()) Walker->GetController()->SetControlRotation((Point - (Walker->GetActorLocation() + FVector(0, 0, 70))).Rotation());
    };
    // Distance from the test target's collision centre, lowered by Below, up to the first surface (ECC_Visibility, as
    // the lift's own traces; Maya and the target ignored); Reach when nothing is nearer. A line trace, not the lift's
    // 1 uu box, so it is a second reading of the same geometry rather than the same call.
    auto Headroom = [this, Walker](float Below, float Reach)
    {
        FVector Centre;
        float Half = 0.f;
        PhaselockDummy->CollisionCentre(Centre, Half);
        Centre.Z -= Below;
        FCollisionQueryParams Query(SCENE_QUERY_STAT(OWQuestPhaselockHeadroom), true, Walker);
        Query.AddIgnoredActor(PhaselockDummy.Get());
        FHitResult Hit;
        return GetWorld()->LineTraceSingleByChannel(Hit, Centre, Centre + FVector(0, 0, Reach), ECC_Visibility, Query) ? Hit.Distance : Reach;
    };
    switch (PhaselockStep)
    {
    case 0: {
        FString Reason;
        Check(P.bLoaded && Skills->TrySpend(-1, -1, -1, Reason) && Skills->GetActionGrade() == 1, TEXT("skill_point_buys_phaselock"));
        Check(PhaselockTableMatchesManifest(), TEXT("phaselock_timelines_match_manifest_table"));
        // A host engine-shape target on the range lane from the trigger centre (the lane the dummy used).
        const FVector Lane = (Impl->Data.DummyLocation - Impl->Data.TriggerCenter).GetSafeNormal2D();
        FCollisionQueryParams Query(SCENE_QUERY_STAT(OWQuestPhaselockTarget), true, Walker);
        auto FloorAt = [&](float Along, FVector& Out)
        {
            const FVector Ahead = Impl->Data.TriggerCenter + Lane * Along;
            FHitResult Floor;
            Out = Ahead;
            if (!GetWorld()->LineTraceSingleByChannel(Floor, Ahead + FVector(0, 0, 200), Ahead - FVector(0, 0, 600), ECC_Visibility, Query)) return false;
            Out = Floor.ImpactPoint;
            return true;
        };
        FVector At;
        FloorAt(400.f, At);
        PhaselockDummy = GetWorld()->SpawnActor<AOpenWillowCombatTarget>(At, Lane.Rotation() + FRotator(0, 180, 0));
        Query.AddIgnoredActor(PhaselockDummy.Get());
        // Lift rule probes along the lane, without casting. The cast spot is the first spot (400 uu first) with a surface
        // less than HeightFromGround above the target's centre, so the script's ceiling clamp applies there; the
        // open-ground spot is the first with nothing within HeightFromGround + bob. Both are found, not assumed; without
        // a ceiling spot the cast stays at 400 uu under a test fixture (below).
        float CastAlong = -1.f, OpenAlong = -1.f, OpenLift = -1.f;
        FVector CastAt = At;
        for (const float Along : {400.f, 300.f, 500.f, 200.f, 600.f, 700.f, 100.f, 800.f, 900.f, 1000.f})
        {
            FVector Spot;
            if (!FloorAt(Along, Spot)) continue;
            PhaselockDummy->SetActorLocation(Spot);
            const float Room = Headroom(0.f, P.HeightFromGround + P.BobAmplitude);
            if (CastAlong < 0.f && Room < P.HeightFromGround) { CastAlong = Along; CastAt = Spot; }
            if (OpenAlong < 0.f && Room >= P.HeightFromGround + P.BobAmplitude) { OpenAlong = Along; OpenLift = PhaselockDummy->PhaselockLiftHeight(P); }
        }
        PhaselockDummy->SetActorLocation(CastAt);
        if (CastAlong < 0.f)
        {
            // Test fixture: the lane has no surface that low, so an invisible blocking box is hung over the target with
            // its underside 3/4 of HeightFromGround above the centre. It is removed after the ceiling check.
            FVector Centre;
            float Half = 0.f;
            PhaselockDummy->CollisionCentre(Centre, Half);
            const FVector Extent(100.f, 100.f, 10.f);
            PhaselockCeiling = GetWorld()->SpawnActor<AActor>(Centre + FVector(0, 0, 0.75f * P.HeightFromGround + Extent.Z), FRotator::ZeroRotator);
            UBoxComponent* Box = NewObject<UBoxComponent>(PhaselockCeiling, TEXT("CeilingFixture"));
            Box->SetBoxExtent(Extent);
            Box->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
            Box->SetCollisionResponseToAllChannels(ECR_Ignore);
            Box->SetCollisionResponseToChannel(ECC_Visibility, ECR_Block);
            PhaselockCeiling->SetRootComponent(Box);
            Box->RegisterComponent();
            Box->SetWorldLocation(Centre + FVector(0, 0, 0.75f * P.HeightFromGround + Extent.Z));
        }
        UE_LOG(LogTemp, Display, TEXT("OWQUEST Phaselock lane probes: cast spot %.0f uu (ceiling %s), open ground %.0f uu with lift %.1f (HeightFromGround %.0f)"),
            CastAlong < 0.f ? 400.f : CastAlong, CastAlong < 0.f ? TEXT("not found in the lane, test fixture box used") : TEXT("found"), OpenAlong, OpenLift, P.HeightFromGround);
        // Over open ground the script rule lifts by HeightFromGround (+1 uu: the trace box stops 1 uu above the floor).
        Check(OpenAlong >= 0.f && FMath::Abs(OpenLift - P.HeightFromGround) <= 2.f, TEXT("phaselock_lifts_to_stock_height"));
        PlacePlayer(Impl->Data.TriggerCenter, CastAt + FVector(0, 0, 900));   // looking up past it: the first cast misses
        Next();
        return false;
    }
    case 1:
        if (PhaselockWait < 0.5f) return false;
        Walker->UsePhaselock();
        Check(!Walker->LastPhaselockHit() && Walker->PhaselockRemaining() > 0.f && PhaselockDummy && !PhaselockDummy->IsPhaselocked(),
            TEXT("phaselock_miss_lifts_nothing_and_holds_skill"));
        Next();
        return false;
    case 2:
        if (PhaselockWait < P.ReleaseBufferTime + 0.1f) return false;
        Check(Walker->PhaselockRemaining() == 0.f, TEXT("phaselock_miss_resets_cooldown_after_release_buffer"));
        Aim(PhaselockDummy->AimPoint());
        Next();
        return false;
    case 3: {
        if (PhaselockWait < 0.2f) return false;
        Walker->UsePhaselock();
        const FOpenWillowPhaselockTimeline& T = Walker->LastPhaselockTimeline();
        Check(Walker->LastPhaselockHit() && Walker->LastPhaselockTarget() == PhaselockDummy && PhaselockDummy->IsPhaselocked()
            && FMath::IsNearlyEqual(T.SkillDuration, Base.SkillDuration, 1e-4f), TEXT("phaselock_hit_uses_manifest_timeline"));
        PhaselockCastSeen = Walker->LastPhaselockCastAt();
        Next();
        return false;
    }
    case 4: {
        if (PhaselockWait < Base.LockedAt + 0.6f) return false;
        const float Height = PhaselockDummy->LiftedHeight();
        const float End = PhaselockDummy->PhaselockLiftEnd();
        FVector Centre;
        float Half = 0.f;
        PhaselockDummy->CollisionCentre(Centre, Half);
        const float Room = Headroom(Height, 1000.f);   // from the centre before the lift
        const float Gap = Room - End - Half;           // surface minus the top of the collision at the lift end
        UE_LOG(LogTemp, Display, TEXT("OWQUEST Phaselock height %.1f at %.2f s, lift end %.1f (HeightFromGround %.0f, bob %.0f); surface %.1f uu above the rest centre, half height %.1f, top at the lift end %.1f uu below it"),
            Height, PhaselockWait, End, P.HeightFromGround, P.BobAmplitude, Room, Half, Gap);
        // BeginLifting's clamp: with a surface less than HeightFromGround above the centre, the lift end puts the top of
        // the collision at that surface (the 1 uu trace box stops it about 1 uu short), never above it.
        Check(Room < P.HeightFromGround && End < P.HeightFromGround && Gap >= -0.5f && Gap <= 2.5f, TEXT("phaselock_lift_stays_below_ceiling"));
        // While locked the target is at the lift end plus the bob (at most its amplitude).
        Check(FMath::Abs(Height - End) <= P.BobAmplitude + 1.f, TEXT("phaselock_height_is_lift_end_plus_bob"));
        // Stock presentation from tools/prepare_phaselock_fx.py and the imported FX (host playback of the decoded
        // templates): at this time the bubble loop runs (intro at the lock, loop after BubbleFXIntroTime), the light is at
        // full brightness (locked), the hand orb (from the arms clip's notify) and the screen particle (burst inside its
        // duration) have live particles. Presence and timing only; the look is checked by screenshots.
        {
            const FOpenWillowPhaselockFxData& Fx = Walker->GetPhaselockFx();
            UE_LOG(LogTemp, Display, TEXT("OWQUEST Phaselock presentation at %.2f s: fx manifest %d, bubble stage %d, collapse %.3f, light %.2f (data %.2f), hand particles %d, screen particles %d"),
                PhaselockWait, Fx.bLoaded, PhaselockDummy->BubbleStage(), PhaselockDummy->BubbleCollapse(),
                PhaselockDummy->PhaselockLightIntensity(), Fx.LightBrightness, Walker->HandFxParticles(), Walker->ScreenFxParticles());
            Check(Fx.bLoaded && PhaselockDummy->BubbleStage() == 2 && FMath::IsNearlyEqual(PhaselockDummy->PhaselockLightIntensity(), Fx.LightBrightness, 1e-3f)
                && Walker->HandFxParticles() > 0 && Walker->ScreenFxParticles() > 0, TEXT("phaselock_stock_presentation_running"));
        }
        if (PhaselockCeiling) PhaselockCeiling->Destroy();
        PhaselockCeiling = nullptr;
        Shot(TEXT("4_Phaselock"));
        Next();
        return false;
    }
    case 5: {
        if (PhaselockDummy->IsPhaselocked() && PhaselockWait < 10.f) return false;
        const float Released = PhaselockDummy->PhaselockReleasedAt() - PhaselockCastSeen;
        const float Now = GetWorld()->GetTimeSeconds();
        UE_LOG(LogTemp, Display, TEXT("OWQUEST Phaselock released after %.3f s (manifest %.3f); cooldown left %.2f of %.1f; target time scale %.2f"),
            Released, Base.ReleasedAt, Walker->PhaselockRemaining(), P.CooldownSeconds, PhaselockDummy->PhaselockTimeScale(Now, P));
        // The release happens on the first tick at or after the time: allow one slow test frame (0.1 s).
        Check(!PhaselockDummy->IsPhaselocked() && Released >= Base.ReleasedAt && Released - Base.ReleasedAt <= 0.1f, TEXT("phaselock_releases_at_manifest_time"));
        Check(FMath::Abs(Walker->PhaselockRemaining() - P.CooldownSeconds) <= 0.1f, TEXT("phaselock_cooldown_paused_while_target_held"));
        Check(PhaselockDummy->PhaselockTimeScale(Now, P) == P.TargetTimeScale(true) && P.TargetTimeScale(true) < P.TargetTimeScale(false),
            TEXT("phaselock_diminishing_returns_on_released_target"));
        Next();
        return false;
    }
    case 6: {
        // After the release the pool drains at the base rate: wait until it is empty.
        if (Walker->PhaselockRemaining() > 0.f && PhaselockWait < P.CooldownSeconds / FMath::Max(P.CooldownRate, KINDA_SMALL_NUMBER) + 1.f) return false;
        // Cast gate from Skill_Phaselock.SkillConstraints: open in Maya's current state; the readings of the native
        // evaluators (GateOpen with test inputs derived from Maya's state; her state is not changed): a reload does not
        // refuse the cast, a holstered (inactive) weapon does, and so does a pawn that is not alive and well.
        FString Why, ReloadWhy, Busy, Hurt;
        const bool bOpen = Walker->CanCastPhaselock(Why);
        FOpenWillowPhaselockGateState Reloading = Walker->PhaselockGateState();
        Reloading.bReloading = true;
        FOpenWillowPhaselockGateState Holstered = Walker->PhaselockGateState();
        Holstered.bHoldsWeapon = Holstered.bWeaponInactive = true;
        FOpenWillowPhaselockGateState Down = Walker->PhaselockGateState();
        Down.bAlive = false;
        const bool bReloadAllowed = P.GateOpen(Reloading, true, ReloadWhy);
        const bool bHolsterRefused = !P.GateOpen(Holstered, true, Busy);
        const bool bDeadRefused = !P.GateOpen(Down, true, Hurt);
        UE_LOG(LogTemp, Display, TEXT("OWQUEST Phaselock gate: open=%d %s; reloading open=%d %s; holstered refused by %s; not alive refused by %s; not evaluated [%s]"),
            bOpen, *Why, bReloadAllowed, *ReloadWhy, *Busy, *Hurt, *FString::Join(P.GateNotEvaluated, TEXT(", ")));
        Check(bOpen, TEXT("phaselock_cast_gate_open_when_constraints_met"));
        // Replaces the earlier phaselock_weapon_action_constraint_refuses_cast, whose host reading (reloading blocks) the
        // native evaluator contradicts: the weapon-action constraint is now checked with a holstered weapon.
        Check(bReloadAllowed, TEXT("phaselock_reload_does_not_refuse_cast"));
        Check(bHolsterRefused && Busy.Contains(TEXT("WeaponActionAvailable")), TEXT("phaselock_holstered_weapon_refuses_cast"));
        Check(bDeadRefused && Hurt.Contains(TEXT("HealthState")), TEXT("phaselock_health_constraint_refuses_cast"));
        // Blocked target: clear the host property that stands for Flag_Skills_CanPhaseLock, then cast at it.
        PhaselockDummy->SetCanPhaseLockFlag(false);
        Aim(PhaselockDummy->AimPoint());
        Next();
        return false;
    }
    case 7:
        if (PhaselockWait < 0.2f) return false;
        Walker->UsePhaselock();
        Check(Walker->LastPhaselockBlocked() && !Walker->LastPhaselockHit() && !PhaselockDummy->IsPhaselocked()
            && PhaselockDummy->LiftedHeight() < 1.f, TEXT("phaselock_blocked_target_is_not_lifted"));
        Next();
        return false;
    case 8:
        // A blocked cast is not a fizzle: nothing resets the cooldown after ReleaseBufferTime.
        if (PhaselockWait < P.ReleaseBufferTime + 0.1f) return false;
        Check(Walker->PhaselockRemaining() > 0.f, TEXT("phaselock_blocked_cast_keeps_cooldown"));
        {
            // Swap 10, the host's application of the level-up skill's cooldown modifier on a cooldown that is really running (the turn-in's level-up comes while it is 0):
            // Scale by 0.5 halves what is left (no completion), Scale by 0 (the data's modifier) ends it at once and reports the completion; the drain then stays at 0.
            const float Left = Walker->PhaselockRemaining();
            const bool bHalfEnded = Walker->ModifyPhaselockCooldown(0, 0.5f);
            const float Half = Walker->PhaselockRemaining();
            const bool bEnded = Walker->ModifyPhaselockCooldown(0, 0.f);
            Check(Left > 1.f && !bHalfEnded && FMath::IsNearlyEqual(Half, Left * 0.5f, 0.05f) && bEnded && Walker->PhaselockRemaining() == 0.f && !Walker->ModifyPhaselockCooldown(0, 0.f),
                TEXT("action_skill_cooldown_modifier_scales_then_resets_host_cooldown"));
        }
        PhaselockDummy->SetCanPhaseLockFlag(true);
        Next();
        return false;
    case 9: {
        // Screen-space magnetism (native auto-aim reading): turn the view sideways until the target's centre sits at 70-90%
        // of its magnetism radius, so the crosshair ray passes beside it, then cast: the target is still preferred.
        if (Walker->PhaselockRemaining() > 0.f && PhaselockWait < P.CooldownSeconds / FMath::Max(P.CooldownRate, KINDA_SMALL_NUMBER) + 1.f) return false;
        Aim(PhaselockDummy->AimPoint());
        AController* Controller = Walker->GetController();
        AOpenWillowWalker::FPhaselockAimScore Score;
        for (int32 I = 0; Controller && I < 200; ++I)
        {
            Score = Walker->ScorePhaselockTarget(PhaselockDummy);
            if (Score.MagnetRadius > 0.f && Score.ScreenOffset >= 0.7f * Score.MagnetRadius) break;
            Controller->SetControlRotation(Controller->GetControlRotation() + FRotator(0, 0.25f, 0));
        }
        // Distance of the target's centre from the view ray (how far a plain crosshair trace would pass beside it).
        FVector Centre;
        float Half = 0.f;
        PhaselockDummy->CollisionCentre(Centre, Half);
        const FVector Eye = Walker->GetActorLocation() + FVector(0, 0, 70);
        const float Beside = FMath::PointDistToLine(Centre, Walker->GetViewRotation().Vector(), Eye);
        Walker->UsePhaselock();
        UE_LOG(LogTemp, Display, TEXT("OWQUEST Phaselock magnetism: depth %.0f, offset %.3f of magnet radius %.3f (own %.3f), ray passes %.0f uu from the centre (radius %.1f); hit=%d"),
            Score.Depth, Score.ScreenOffset, Score.MagnetRadius, Score.TargetRadius, Beside, PhaselockDummy->AutoAimRadius(), Walker->LastPhaselockHit());
        Check(Score.ScreenOffset >= 0.7f * Score.MagnetRadius && Score.ScreenOffset < Score.MagnetRadius && Beside > PhaselockDummy->AutoAimRadius()
            && Walker->LastPhaselockHit() && PhaselockDummy->IsPhaselocked(), TEXT("phaselock_magnetism_prefers_target_off_the_crosshair"));
        PhaselockCastSeen = Walker->LastPhaselockCastAt();
        Next();
        return false;
    }
    case 10: {
        // Going down while the target is held: the while-active HealthState constraint deactivates the skill.
        if (PhaselockWait < P.LiftDuration + 0.3f) return false;
        FHitResult Hit;
        UGameplayStatics::ApplyPointDamage(Walker, 1.0e6f, FVector::ForwardVector, Hit, nullptr, nullptr, UDamageType::StaticClass());
        Next();
        return false;
    }
    case 11: {
        if (PhaselockWait < 0.2f) return false;
        const float Released = PhaselockDummy->PhaselockReleasedAt() - PhaselockCastSeen;
        const FOpenWillowPhaselockTimeline& T = Walker->LastPhaselockTimeline();
        UE_LOG(LogTemp, Display, TEXT("OWQUEST Phaselock after going down: released %.2f s after the cast (timeline %.2f), reason '%s', active %d"),
            Released, T.ReleasedAt, *Walker->LastPhaselockEndReason(), Walker->IsPhaselockActive());
        Check(!PhaselockDummy->IsPhaselocked() && !Walker->IsPhaselockActive() && Released < T.ReleasedAt - 0.5f
            && Walker->LastPhaselockEndReason().Contains(TEXT("HealthState")), TEXT("phaselock_going_down_ends_the_lock"));
        Next();
        return false;
    }
    case 12: {
        // Suspension: the action point, a full lower Motion tier, then one Suspension grade (1 + 5 + 1 points).
        // Test fixture: raise the level so that many points exist (one per level from 5).
        TArray<FString> SpendOrder;
        FString Text;
        TSharedPtr<FJsonObject> Manifest;
        if (FFileHelper::LoadFileToString(Text, *Walker->GetPhaselockFile()) && FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Manifest) && Manifest)
            Manifest->GetObjectField(TEXT("upgradePath"))->TryGetStringArrayField(TEXT("spendOrder"), SpendOrder);
        int32 Branch = 0, Tier = 0, Cell = 0, LowerPoints = 0;
        if (SpendOrder.Num() == 2 && Skills->FindSkill(SpendOrder[0], Branch, Tier, Cell))
            LowerPoints = Skills->TierPoints(Branch, Tier);
        Skills->SetLevel(FMath::Max(Skills->GetLevel(), 4 + 1 + LowerPoints + 1));
        UE_LOG(LogTemp, Display, TEXT("OWQUEST test fixture: level %d for %d+1 Motion points"), Skills->GetLevel(), LowerPoints);
        bool bSpent = SpendOrder.Num() == 2 && LowerPoints > 0;
        FString Reason;
        for (int32 I = 0; I < LowerPoints && bSpent; ++I) bSpent = Skills->TrySpend(Branch, Tier, Cell, Reason);
        if (bSpent && Skills->FindSkill(SpendOrder[1], Branch, Tier, Cell)) bSpent = Skills->TrySpend(Branch, Tier, Cell, Reason);
        const int32 Grade = Skills->GradeOf(P.DurationSkill);
        UE_LOG(LogTemp, Display, TEXT("OWQUEST %s grade %d -> lock duration %.2f (base %.2f) %s"), *P.DurationSkill, Grade, P.LockDuration(Grade), P.LockDurationBase, *Reason);
        Check(bSpent && SpendOrder.Num() == 2 && SpendOrder[1] == P.DurationSkill && Grade == 1 && P.DurationPostAdd.IsValidIndex(1)
            && FMath::IsNearlyEqual(P.LockDuration(Grade), P.LockDurationBase + P.DurationPostAdd[1]) && P.DurationPostAdd[1] > 0.f,
            TEXT("suspension_point_adds_manifest_lock_time"));
        if (PhaselockDummy) PhaselockDummy->Destroy();
        PhaselockDummy = nullptr;
        return true;
    }
    default:
        return true;
    }
}

// The host timelines against the tool's own table (phaselockByModifierGrade): every grade, first lock and re-lock
// inside the diminishing-returns window. Agreement between two readings of the same data, not a game check.
bool UOpenWillowQuest::PhaselockTableMatchesManifest() const
{
    const auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    const FOpenWillowPhaselockData& P = Walker->GetPhaselockData();
    FString Text;
    TSharedPtr<FJsonObject> Manifest;
    if (!FFileHelper::LoadFileToString(Text, *Walker->GetPhaselockFile()) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Manifest) || !Manifest)
        return false;
    int32 Compared = 0, Mismatched = 0;
    for (const auto& Value : Manifest->GetObjectField(TEXT("upgradePath"))->GetArrayField(TEXT("phaselockByModifierGrade")))
    {
        const auto Row = Value->AsObject();
        const int32 Grade = int32(Row->GetNumberField(TEXT("grade")));
        for (const bool bRelock : {false, true})
        {
            const auto Want = Row->GetObjectField(bRelock ? TEXT("relockWithinDiminishingReturns") : TEXT("firstLock"));
            const FOpenWillowPhaselockTimeline Got = P.Timeline(P.LockDuration(Grade), P.TargetTimeScale(bRelock));
            const float Pairs[][2] = {{Got.SkillDuration, float(Want->GetNumberField(TEXT("skillDuration")))},
                {Got.LockedAt, float(Want->GetNumberField(TEXT("lockedAt")))}, {Got.OutroAt, float(Want->GetNumberField(TEXT("outroAt")))},
                {Got.ReleasedAt, float(Want->GetNumberField(TEXT("releasedAt")))}, {Got.EndSkillAt, float(Want->GetNumberField(TEXT("endSkillAt")))}};
            for (const auto& Pair : Pairs)
            {
                ++Compared;
                if (FMath::Abs(Pair[0] - Pair[1]) > 1e-3f)
                {
                    ++Mismatched;
                    UE_LOG(LogTemp, Warning, TEXT("OWQUEST Phaselock grade %d %s: host %.4f manifest %.4f"), Grade, bRelock ? TEXT("relock") : TEXT("first"), Pair[0], Pair[1]);
                }
            }
        }
    }
    UE_LOG(LogTemp, Display, TEXT("OWQUEST Phaselock timeline table: %d values compared, %d differ"), Compared, Mismatched);
    return Compared > 0 && Mismatched == 0;
}
