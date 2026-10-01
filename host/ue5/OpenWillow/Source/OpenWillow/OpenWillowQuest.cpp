#include "OpenWillowQuest.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowMover.h"
#include "OpenWillowWalker.h"
#include "slice.hpp"
#include "Engine/World.h"
#include "GameFramework/Controller.h"
#include "HAL/PlatformMisc.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"
#include <filesystem>
#include <stdexcept>

namespace {
const char* const MissionPath = "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire";
const char* const DummyProvider = "GD_TargetDummy.Character.CharClass_TargetDummy.BehaviorProviderDefinition_5";
// Host fixture standing in for the player's campaign progress (the mission's only dependency).
const char* const DependencyMission = "GD_Episode03.M_Ep3_CatchARide";
constexpr float RangeRadius = 700.f; // host stand-in for the original range trigger
}

struct UOpenWillowQuest::FImpl {
    PackageStore Store;
    vm::Runtime Runtime;
    std::unique_ptr<vm::FireMissionSlice> Slice;
    std::set<std::string> Completed;
    int64 MoverMatched = 0, MoverBoundary = 0;
    explicit FImpl(const std::filesystem::path& Cooked) : Store(Cooked), Runtime(Store) {
        Runtime.registerCoreNatives();
        Slice = std::make_unique<vm::FireMissionSlice>(Runtime, MissionPath, "Sanctuary_Dynamic", DummyProvider);
        Completed.insert(DependencyMission);
    }
};

UOpenWillowQuest::UOpenWillowQuest()
{
    PrimaryComponentTick.bCanEverTick = true;
}

void UOpenWillowQuest::BeginPlay()
{
    Super::BeginPlay();
    bEnabled = FParse::Param(FCommandLine::Get(), TEXT("owquest"));
    bTesting = FParse::Param(FCommandLine::Get(), TEXT("owquesttest"));
    bResume = FParse::Param(FCommandLine::Get(), TEXT("owquestresume"));
    if (!bEnabled) { SetComponentTickEnabled(false); return; }
    try {
        FString Save;
        if (!FParse::Value(FCommandLine::Get(), TEXT("owquestsave="), Save) || Save.IsEmpty())
            throw std::runtime_error("-owquest needs -owquestsave=<file under local/>");
        SavePath = Save;
        const FString Game = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_BL2"));
        if (!FPaths::FileExists(FPaths::Combine(Game, TEXT("Binaries/Win32/Borderlands2.exe"))))
            throw std::runtime_error("installed Borderlands 2 is required");
        Impl = MakeShared<FImpl>(std::filesystem::path(*FPaths::Combine(Game, TEXT("WillowGame/CookedPCConsole"))));
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
            bWeaponLent = Data->GetBoolField(TEXT("weapon_lent"));
            UE_LOG(LogTemp, Display, TEXT("OWQUEST loaded save: status=%d rewards=%d respawns=%d"), Status(), Rewards, Respawns);
        }
        UE_LOG(LogTemp, Display, TEXT("OWQUEST READY mission=%hs status=%d (Marcus, range and damage type are host stand-ins)"), MissionPath, Status());
    } catch (const std::exception& Error) { Fail(UTF8_TO_TCHAR(Error.what())); }
}

void UOpenWillowQuest::EndPlay(const EEndPlayReason::Type Reason)
{
    Impl.Reset();
    Super::EndPlay(Reason);
}

int32 UOpenWillowQuest::Status() const
{
    return Impl ? int32(Impl->Slice->mission().status()) : -1;
}

void UOpenWillowQuest::Fail(const FString& Error)
{
    bFailed = true;
    UE_LOG(LogTemp, Error, TEXT("OWQUEST ERROR %s"), *Error);
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
    FString Text;
    FJsonSerializer::Serialize(Data, TJsonWriterFactory<>::Create(&Text));
    IFileManager::Get().MakeDirectory(*FPaths::GetPath(SavePath), true);
    if (!FFileHelper::SaveStringToFile(Text, *SavePath)) UE_LOG(LogTemp, Error, TEXT("OWQUEST could not write %s"), *SavePath);
}

void UOpenWillowQuest::Pump()
{
    using K = vm::FireMissionSlice::HostEvent::Kind;
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    for (const auto& Event : Impl->Slice->drain()) {
        const FString A = UTF8_TO_TCHAR(Event.a.c_str());
        switch (Event.kind) {
        case K::RemoteEvent:
            if (Walker && Walker->GetMover()) {
                Walker->GetMover()->MissionEvent(UTF8_TO_TCHAR(Event.b.c_str()), A);
                Impl->MoverMatched += Walker->GetMover()->LastEventMatched;
                Impl->MoverBoundary += Walker->GetMover()->LastEventBoundary;
            }
            UE_LOG(LogTemp, Display, TEXT("OWQUEST remote event %s (mover matched=%d boundary=%d)"), *A,
                Walker && Walker->GetMover() ? Walker->GetMover()->LastEventMatched : -1,
                Walker && Walker->GetMover() ? Walker->GetMover()->LastEventBoundary : -1);
            break;
        case K::Dialog:
            UE_LOG(LogTemp, Display, TEXT("OWQUEST dialog trigger %s (no audio/subtitle payload recovered)"), *A);
            break;
        case K::StatusEffect: UE_LOG(LogTemp, Display, TEXT("OWQUEST status effect %s on dummy (not applied: no effect system)"), *A); break;
        case K::MissionWeaponGranted:
            bWeaponLent = true;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST mission weapon lent: %s (host: fire-typed shots; stock weapon not imported)"), *A);
            break;
        case K::MissionWeaponRemoved:
            bWeaponLent = false;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST mission weapon removed: %s"), *A);
            break;
        case K::Reward:
            ++Rewards;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST XP reward %s (amount unresolved; counted only)"), *A);
            break;
        case K::Status: UE_LOG(LogTemp, Display, TEXT("OWQUEST mission status -> %s"), *A); break;
        case K::ObjectiveSet: UE_LOG(LogTemp, Display, TEXT("OWQUEST objective set active: %s"), *A); break;
        case K::ObjectiveComplete: UE_LOG(LogTemp, Display, TEXT("OWQUEST objective complete: %s"), *A); break;
        }
    }
    for (const auto& Line : Impl->Slice->errors()) { Fail(UTF8_TO_TCHAR(Line.c_str())); return; }
    for (const auto& Line : Impl->Slice->dummy().boundary) UE_LOG(LogTemp, Display, TEXT("OWQUEST host boundary (not run): %hs"), Line.c_str());
    Impl->Slice->dummy().boundary.clear();
    if (Impl->Slice->mission().status() == vm::MissionSystem::Status::Complete) Impl->Completed.insert(MissionPath);
    Save();
}

bool UOpenWillowQuest::Accept()
{
    if (!Impl || bFailed) return false;
    const bool bOk = Impl->Slice->accept(Impl->Completed);
    Pump();
    return bOk;
}

bool UOpenWillowQuest::TurnIn()
{
    if (!Impl || bFailed) return false;
    const bool bOk = Impl->Slice->turnIn();
    Pump();
    return bOk;
}

void UOpenWillowQuest::OnDummyDamaged(bool bFire)
{
    if (!Impl || bFailed) return;
    Impl->Slice->hitDummy(bFire);
    Pump();
}

void UOpenWillowQuest::NotifyRespawn()
{
    ++Respawns;
    UE_LOG(LogTemp, Display, TEXT("OWQUEST player respawned (count=%d, mission status kept=%d)"), Respawns, Status());
    Save();
}

bool UOpenWillowQuest::InRange() const
{
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    FVector Anchor;
    return Walker && Walker->GetMover() && Walker->GetMover()->Anchor(Anchor)
        && FVector::Dist(Walker->GetActorLocation(), Anchor) <= RangeRadius;
}

void UOpenWillowQuest::TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function)
{
    Super::TickComponent(Delta, Type, Function);
    if (!Impl || bFailed) return;
    Impl->Slice->tick(Delta);
    Pump();
    // GoToRange objective: host stand-in for the original range trigger.
    if (Impl->Slice->mission().status() == vm::MissionSystem::Status::Active
        && Impl->Slice->mission().activeSet().find("GoToRange_ObjSet") != std::string::npos && InRange()) {
        Impl->Slice->enterRange();
        Pump();
    }
    if (bTesting) RunTest(Delta);
}

void UOpenWillowQuest::RunTest(float Delta)
{
    TestWait += Delta;
    auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    if (!Walker || !Walker->GetMover()) { Fail(TEXT("quest test needs the walker and the mover (-owmover=...)")); return; }
    if (TestWait < 1.0f) return;
    const FVector Forward = Walker->GetActorForwardVector();
    switch (TestStep) {
    case 0:
        if (bResume) {
            Check(Status() == 3, TEXT("resume_status_complete_from_save"));
            Check(Rewards == 1, TEXT("resume_reward_retained"));
            Check(!Accept(), TEXT("resume_completed_mission_cannot_be_reaccepted"));
            Check(Respawns >= 1, TEXT("resume_respawn_count_retained"));
            TestStep = 100;
        } else {
            Check(Status() == 0, TEXT("fresh_status_not_started"));
            Check(Accept() && Status() == 1, TEXT("accept_mission_active"));
            Check(Impl->MoverMatched >= 1, TEXT("mission_remote_event_reached_installed_kismet"));
        }
        break;
    case 1: {
        FVector Stand;
        Check(Walker->GetMover()->StandPoint(Stand), TEXT("range_anchor_known"));
        Walker->SetActorLocation(Stand, false, nullptr, ETeleportType::TeleportPhysics);
        break;
    }
    case 2:
        Check(Impl->Slice->mission().activeSet().find("RocksPaper_FinalObj") != std::string::npos, TEXT("range_objective_advanced_set"));
        Check(bWeaponLent, TEXT("mission_weapon_lent"));
        break;
    case 3: {
        FActorSpawnParameters Params;
        Dummy = GetWorld()->SpawnActor<AOpenWillowCombatTarget>(Walker->GetActorLocation() + Forward * 400.f + FVector(0, 0, 20),
            Walker->GetActorRotation() + FRotator(0, 180, 0), Params);
        Check(Dummy != nullptr, TEXT("dummy_spawned"));
        break;
    }
    case 4: {
        if (!Dummy) { Fail(TEXT("dummy missing")); return; }
        FHitResult Hit;
        UGameplayStatics::ApplyPointDamage(Dummy, 25.f, -Forward, Hit, Walker->GetController(), Walker, UDamageType::StaticClass());
        Check(Status() == 1, TEXT("non_fire_damage_does_not_complete_objective"));
        UGameplayStatics::ApplyPointDamage(Dummy, 25.f, -Forward, Hit, Walker->GetController(), Walker, UOpenWillowFireDamageType::StaticClass());
        Check(Status() == 2, TEXT("fire_damage_completes_fire_objective_via_dummy_provider"));
        Check(!bWeaponLent, TEXT("mission_weapon_removed_after_objective"));
        break;
    }
    case 5:
        Check(TurnIn() && Status() == 3, TEXT("turn_in_complete"));
        Check(Rewards == 1, TEXT("xp_reward_granted_once"));
        Check(!TurnIn() && Rewards == 1, TEXT("turn_in_not_repeatable"));
        break;
    case 6: {
        const int32 Before = Respawns;
        FHitResult Hit;
        UGameplayStatics::ApplyPointDamage(Walker, 1.0e6f, Forward, Hit, nullptr, nullptr, UDamageType::StaticClass());
        Check(Respawns == Before + 1, TEXT("player_death_triggers_respawn"));
        Check(Status() == 3, TEXT("mission_state_survives_respawn"));
        break;
    }
    case 7:
        Check(FPaths::FileExists(SavePath), TEXT("save_file_written"));
        UE_LOG(LogTemp, Display, TEXT("OWQUESTTEST SUMMARY result=%s checks=%d errors=%d mode=first_run"), Errors ? TEXT("FAIL") : TEXT("PASS"), Checks, Errors);
        FPlatformMisc::RequestExit(false);
        bTesting = false;
        return;
    default:
        UE_LOG(LogTemp, Display, TEXT("OWQUESTTEST SUMMARY result=%s checks=%d errors=%d mode=resume"), Errors ? TEXT("FAIL") : TEXT("PASS"), Checks, Errors);
        FPlatformMisc::RequestExit(false);
        bTesting = false;
        return;
    }
    ++TestStep;
    TestWait = 0;
}
