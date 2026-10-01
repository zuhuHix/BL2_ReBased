#include "OpenWillowQuest.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowMover.h"
#include "OpenWillowNpc.h"
#include "OpenWillowSkills.h"
#include "OpenWillowSliceData.h"
#include "OpenWillowWalker.h"
#include "slice.hpp"
#include "Components/CapsuleComponent.h"
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "GameFramework/PlayerController.h"
#include "HAL/PlatformMisc.h"
#include "InputKeyEventArgs.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"
#include "UnrealClient.h"
#include <filesystem>
#include <stdexcept>

namespace {
const char* const MissionPath = "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire";
const char* const DummyProvider = "GD_TargetDummy.Character.CharClass_TargetDummy.BehaviorProviderDefinition_5";
// Host fixture standing in for the player's campaign progress (the mission's only dependency).
const char* const DependencyMission = "GD_Episode03.M_Ep3_CatchARide";
// Host-chosen (UNVERIFIED): how close the player must be to Marcus for the use key to talk to him.
constexpr float TalkReach = 250.f;
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
        Completed.insert(DependencyMission);
    }
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
    bEnabled = FParse::Param(FCommandLine::Get(), TEXT("owquest"));
    bTesting = FParse::Param(FCommandLine::Get(), TEXT("owquesttest"));
    bResume = FParse::Param(FCommandLine::Get(), TEXT("owquestresume"));
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
        Impl->StationCounters.Init(0, Impl->Data.Stations.Num());
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
            bWeaponLent = Data->GetBoolField(TEXT("weapon_lent"));
            UE_LOG(LogTemp, Display, TEXT("OWQUEST loaded save: status=%d rewards=%d respawns=%d"), Status(), Rewards, Respawns);
        }
        SpawnMarcus();
        UE_LOG(LogTemp, Display, TEXT("OWQUEST READY mission=%hs status=%d trigger=%s r=%.2f hh=%.2f walk_nodes=%d stations=%d audio_entries=%d"),
            MissionPath, Status(), *Impl->Data.TriggerObject, Impl->Data.TriggerRadius, Impl->Data.TriggerHalfHeight,
            Impl->Data.Walk.Num(), Impl->Data.Stations.Num(), Impl->Data.Audio.Num());
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
                // The holder (an interactive object with no prepared mesh) rides on the target carrier; the dummy
                // keeps its spawn pose and follows the carrier. Bone "Target" offset not applied (UNVERIFIED).
                if (bHolder && Mover->TrackCarrier()) {
                    Dummy->AttachToComponent(Mover->TrackCarrier(), FAttachmentTransformRules::KeepWorldTransform);
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
            // Dialog hook: stock event -> audio manifest entry. Nothing is decoded, so nothing plays.
            ++DialogLookups;
            const TSharedPtr<FJsonObject>* Entry = Impl->Data.Audio.Find(TEXT("dialog:") + A);
            if (!Entry) {
                ++DialogMisses;
                UE_LOG(LogTemp, Warning, TEXT("OWQUEST dialog %s: no audio manifest entry"), *A);
                break;
            }
            TArray<FString> Sources;
            for (const auto& Media : (*Entry)->GetArrayField(TEXT("media")))
                Sources.Add(FString::Printf(TEXT("%.0f"), Media->AsObject()->GetNumberField(TEXT("source"))));
            const FString State = (*Entry)->GetStringField(TEXT("state"));
            UE_LOG(LogTemp, Display, TEXT("OWQUEST dialog %s talker=%hs -> source %s state=%s (not played: no decoded audio)"),
                *A, Event.c.c_str(), *FString::Join(Sources, TEXT(",")), *State);
            break;
        }
        case K::StatusEffect: UE_LOG(LogTemp, Display, TEXT("OWQUEST status effect %s on dummy (not applied: no effect system)"), *A); break;
        case K::MissionWeaponGranted:
            bWeaponLent = true;
            UE_LOG(LogTemp, Display, TEXT("OWQUEST mission weapon lent: %s (host: fire-typed shots; stock weapon not hosted)"), *A);
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
    // World behaviors the dummy provider reached: none has a host binding yet (logged, never counted as run).
    for (const auto& Call : Impl->Slice->dummy().boundaryCalls) {
        TArray<FString> Fields;
        for (const auto& Field : Call.fields) Fields.Add(FString::Printf(TEXT("%hs=%hs"), Field.first.c_str(), Field.second.c_str()));
        UE_LOG(LogTemp, Display, TEXT("OWQUEST dummy behavior %hs:%hs (%hs.%hs) %s (not run by host)"), Call.cls.c_str(),
            Call.name.c_str(), Call.sequence.c_str(), Call.event.c_str(), *FString::Join(Fields, TEXT(" ")));
    }
    Impl->Slice->dummy().boundary.clear();
    Impl->Slice->dummy().boundaryCalls.clear();
    if (Impl->Slice->mission().status() == vm::MissionSystem::Status::Complete) Impl->Completed.insert(MissionPath);
    RouteRequests();
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

bool UOpenWillowQuest::InTalkReach() const
{
    const AActor* Owner = GetOwner();
    return Marcus && Owner && FVector::Dist(Owner->GetActorLocation(), Marcus->GetActorLocation()) <= TalkReach;
}

bool UOpenWillowQuest::TryUse()
{
    if (!Impl || bFailed || !InTalkReach()) return false;
    using S = vm::MissionSystem::Status;
    const S Current = Impl->Slice->mission().status();
    // The stock mission menu (accept / turn-in screen) is not hosted: the use key accepts or turns in directly.
    const TCHAR* Action = TEXT("nothing to accept or turn in");
    bool bOk = true;
    if (Current == S::NotStarted) { Action = TEXT("accept"); bOk = Accept(); }
    else if (Current == S::ReadyToTurnIn) { Action = TEXT("turn in"); bOk = TurnIn(); }
    UE_LOG(LogTemp, Display, TEXT("OWQUEST use near Marcus: %s (ok=%d, status %d -> %d)"), Action, bOk, int32(Current), Status());
    return true;
}

void UOpenWillowQuest::OnDummyDamaged(AOpenWillowCombatTarget* Target, bool bFire)
{
    if (!Impl || bFailed || !Target || Target != Dummy) return;
    // Host stand-in for the shot's stock damage type: fire-typed host damage = the incendiary impact type.
    Impl->Slice->damageDummy(bFire ? "GD_Incendiary.DamageType.DmgType_Incendiary_Impact" : "");
    Pump();
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

bool UOpenWillowQuest::PlayerTouchesTrigger() const
{
    // WillowWaypoint.Touch with a cylinder: the player's capsule overlaps it (planar distance within the sum of
    // radii, vertical gap within the sum of half heights). UE3 touch semantics in the host: UNVERIFIED.
    const auto* Walker = Cast<AOpenWillowWalker>(GetOwner());
    if (!Walker) return false;
    const auto& Data = Impl->Data;
    const UCapsuleComponent* Capsule = Walker->GetCapsuleComponent();
    const FVector P = Walker->GetActorLocation();
    return FVector::Dist2D(P, Data.TriggerCenter) <= Data.TriggerRadius + Capsule->GetScaledCapsuleRadius()
        && FMath::Abs(P.Z - Data.TriggerCenter.Z) <= Data.TriggerHalfHeight + Capsule->GetScaledCapsuleHalfHeight();
}

void UOpenWillowQuest::UpdateHints()
{
    using S = vm::MissionSystem::Status;
    const S Current = Impl->Slice->mission().status();
    const bool bCanTalk = InTalkReach() && (Current == S::NotStarted || Current == S::ReadyToTurnIn);
    if (bCanTalk && !bTalkHintLogged) UE_LOG(LogTemp, Display, TEXT("OWQUEST hint: press E (use) to talk to Marcus"));
    bTalkHintLogged = bCanTalk;
    if (!GEngine || bTesting) return;
    FString Line = TEXT("Rock, Paper, Genocide: Fire Weapons! - ");
    if (Current == S::NotStarted) Line += TEXT("talk to Marcus (E)");
    else if (Current == S::Active && Impl->ObjectiveState(Impl->Data.TriggerObjective) == "Active") Line += TEXT("go to the range");
    else if (Current == S::Active) Line += TEXT("shoot the target with fire damage");
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
    Impl->Slice->tick(Delta);
    Pump();
    ProcessArrivals();
    if (bFailed) return;
    // GoToRange: a player pawn touching the stock waypoint cylinder while its objective is active.
    if (Impl->ObjectiveState(Impl->Data.TriggerObjective) == "Active" && PlayerTouchesTrigger()) {
        UE_LOG(LogTemp, Display, TEXT("OWQUEST player touched %s"), *Impl->Data.TriggerObject);
        Impl->Slice->enterRange();
        Pump();
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
    if (Walker->GetController()) Walker->GetController()->SetControlRotation((Look - (Location + FVector(0, 0, 70))).Rotation());
}

void UOpenWillowQuest::PressUse()
{
    if (auto* Pawn = Cast<APawn>(GetOwner())) if (auto* PC = Cast<APlayerController>(Pawn->GetController())) {
        PC->InputKey(FInputKeyEventArgs::CreateSimulated(EKeys::E, IE_Pressed, 1.f));
        bReleaseUse = true;
    }
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
        PressUse();   // far from Marcus: must do nothing
        break;
    }
    case 1:
        Check(Status() == 0, TEXT("use_key_out_of_reach_ignored"));
        PlacePlayer(Data.MarcusLocation + Marcus->GetActorForwardVector() * 350.f + FVector(0, 0, 20), MarcusFront + FVector(0, 0, 20));
        break;
    case 2:
        if (TestWait < 1.5f) return;
        Shot(TEXT("1_MarcusStockPose"));
        break;
    case 3:
        if (TestWait < 0.5f) return;
        PlacePlayer(Data.MarcusLocation + Marcus->GetActorForwardVector() * 150.f + FVector(0, 0, 20), MarcusFront);
        PressUse();
        break;
    case 4:
        Check(Status() == 1, TEXT("use_key_accepts_mission"));
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
        // Outside the stock cylinder but inside the old 700 cm stand-in radius: must not complete.
        Check(Impl->ObjectiveState(Data.TriggerObjective) == "Active" && !PlayerTouchesTrigger(), TEXT("outside_stock_cylinder_does_not_trigger"));
        PlacePlayer(Data.TriggerCenter, Data.DummyLocation);
        break;
    case 7:
        if (Waiting(Impl->ObjectiveState(Data.TriggerObjective) == "Complete", 3.f)) return;
        Check(Impl->Slice->mission().activeSet().find("RocksPaper_FinalObj") != std::string::npos, TEXT("stock_cylinder_touch_advances_set"));
        Check(bWeaponLent, TEXT("mission_weapon_lent"));
        break;
    case 8: {
        if (Waiting(Dummy && bDummyAttached, 3.f)) return;
        Check(Dummy && DummySpawnedAt.Equals(Data.DummyLocation, 1.f) && Dummy->IsStockPawn(), TEXT("stock_dummy_spawned_at_population_point"));
        Check(Dummy && bDummyAttached && Dummy->GetRootComponent()->GetAttachParent() == Mover->TrackCarrier(),
            TEXT("dummy_attached_to_carrier_by_installed_kismet"));
        Check(Mover->TrackRunning() || Mover->TrackForwardEnds > 0, TEXT("dummy_provider_starts_target_matinee"));
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
        Check(Dummy && (Dummy->GetActorLocation() - DummySpawnedAt).Equals(Mover->TrackOffset(), 0.5f), TEXT("dummy_moves_with_carrier"));
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
        FHitResult Hit;
        const FVector Toward = (Dummy->AimPoint() - Walker->GetActorLocation()).GetSafeNormal();
        UGameplayStatics::ApplyPointDamage(Dummy, 25.f, Toward, Hit, Walker->GetController(), Walker, UDamageType::StaticClass());
        Check(Status() == 1, TEXT("non_fire_damage_does_not_complete_objective"));
        UGameplayStatics::ApplyPointDamage(Dummy, 25.f, Toward, Hit, Walker->GetController(), Walker, UOpenWillowFireDamageType::StaticClass());
        Check(Status() == 2, TEXT("fire_damage_completes_fire_objective_via_dummy_provider"));
        Check(!bWeaponLent, TEXT("mission_weapon_removed_after_objective"));
        break;
    }
    case 14:
        if (Waiting(Mover->TrackReverseEnds > 0 && bDummyDestroyedBySequence, 12.f)) return;
        Check(bSendBackFromProvider && Mover->TrackReverseEnds == 1 && Mover->TrackOffset().IsNearlyZero(0.5f), TEXT("dummy_provider_sends_target_back"));
        Check(bDummyDestroyedBySequence && !Dummy, TEXT("installed_kismet_destroys_dummy_at_reverse_end"));
        PlacePlayer(Marcus->GetActorLocation() + Marcus->GetActorForwardVector() * 150.f + FVector(0, 0, 20), Marcus->GetActorLocation());
        break;
    case 15:
        if (TestWait < 0.5f) return;
        PressUse();
        break;
    case 16:
        Check(Status() == 3, TEXT("use_key_turns_in_mission"));
        Check(Rewards == 1, TEXT("xp_reward_granted_once"));
        PressUse();
        break;
    case 17:
        Check(Status() == 3 && Rewards == 1, TEXT("turn_in_not_repeatable"));
        PlacePlayer(Data.OracleDeathLocation, Data.DummyLocation);
        break;
    case 18: {
        if (TestWait < 0.5f) return;
        const int32 Before = Respawns;
        FHitResult Hit;
        UGameplayStatics::ApplyPointDamage(Walker, 1.0e6f, FVector::ForwardVector, Hit, nullptr, nullptr, UDamageType::StaticClass());
        Check(Respawns == Before + 1, TEXT("player_death_triggers_respawn"));
        Check(Walker->GetActorLocation().Equals(Data.OracleRespawnLocation, 1.f), TEXT("respawn_at_decoded_station_exit_point"));
        Check(Walker->GetHealth() == Walker->GetMaxHealth(), TEXT("respawn_restores_formula_health"));
        Check(Status() == 3, TEXT("mission_state_survives_respawn"));
        break;
    }
    case 19:
        if (Waiting(Walker->GetCharacterMovement()->IsMovingOnGround(), 4.f)) return;
        UE_LOG(LogTemp, Display, TEXT("OWQUEST after respawn: %s grounded=%d"), *Walker->GetActorLocation().ToString(),
            Walker->GetCharacterMovement()->IsMovingOnGround());
        Check(Walker->GetCharacterMovement()->IsMovingOnGround(), TEXT("respawn_point_is_standable"));
        break;
    case 20:
        UE_LOG(LogTemp, Display, TEXT("OWQUEST dialog lookups=%d misses=%d played=%d"), DialogLookups, DialogMisses, DialogPlayed);
        Check(DialogLookups > 0 && DialogMisses == 0 && DialogPlayed == 0, TEXT("dialog_hook_finds_every_line_and_plays_nothing"));
        Check(FPaths::FileExists(SavePath), TEXT("save_file_written"));
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
