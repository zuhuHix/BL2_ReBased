#include "OpenWillowMover.h"
#include "OpenWillowWalker.h"
#include "mover.hpp"
#include "Components/StaticMeshComponent.h"
#include "Engine/StaticMesh.h"
#include "EngineUtils.h"
#include "GameFramework/PlayerController.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "HAL/PlatformMisc.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "UnrealClient.h"
#include "InputKeyEventArgs.h"
#include <stdexcept>

namespace {
struct FMoverKey { double Time; FVector Value, Arrive, Leave; FString Mode; };
FVector Evaluate(const TArray<FMoverKey>& Keys, double Time) {
    if (Time <= Keys[0].Time) return Keys[0].Value;
    for (int32 I = 1; I < Keys.Num(); ++I) {
        const FMoverKey& A = Keys[I - 1]; const FMoverKey& B = Keys[I];
        if (Time > B.Time) continue;
        const double Span = B.Time - A.Time, T = (Time - A.Time) / Span;
        if (A.Mode == TEXT("CIM_Constant")) return A.Value;
        if (A.Mode == TEXT("CIM_Linear")) return FMath::Lerp(A.Value, B.Value, T);
        const double T2 = T*T, T3 = T2*T;
        return (2*T3 - 3*T2 + 1)*A.Value + (T3 - 2*T2 + T)*Span*A.Leave
             + (-2*T3 + 3*T2)*B.Value + (T3 - T2)*Span*B.Arrive;
    }
    return Keys.Last().Value;
}
FVector Vector(const TSharedPtr<FJsonObject>& Object, const TCHAR* Name) {
    if (!Object) throw std::runtime_error("missing mover object");
    const auto& A = Object->GetArrayField(Name);
    if (A.Num() != 3) throw std::runtime_error("invalid mover vector size");
    FVector V;
    for (int32 I = 0; I < 3; ++I) {
        double N = 0;
        if (!A[I]->TryGetNumber(N) || !FMath::IsFinite(N) || FMath::Abs(N) > 1e8) throw std::runtime_error("invalid mover vector");
        V[I] = N;
    }
    return V;
}
TArray<FMoverKey> ReadKeys(const TSharedPtr<FJsonObject>& Object, const TCHAR* Name, double Duration) {
    TArray<FMoverKey> Result;
    const auto& A = Object->GetArrayField(Name);
    if (A.Num() < 2 || A.Num() > 128) throw std::runtime_error("invalid mover keys");
    for (const auto& V : A) {
        const auto K = V->AsObject();
        if (!K) throw std::runtime_error("invalid mover key object");
        FMoverKey Key{K->GetNumberField(TEXT("time")), Vector(K, TEXT("value")), Vector(K, TEXT("arrive")), Vector(K, TEXT("leave")), K->GetStringField(TEXT("mode"))};
        if (!FMath::IsFinite(Key.Time) || Key.Time < 0 || Key.Time > Duration || (Result.Num() && Key.Time <= Result.Last().Time))
            throw std::runtime_error("invalid mover key time");
        if (Key.Mode != TEXT("CIM_Linear") && Key.Mode != TEXT("CIM_Constant") && Key.Mode != TEXT("CIM_CurveAuto")
            && Key.Mode != TEXT("CIM_CurveAutoClamped") && Key.Mode != TEXT("CIM_CurveUser") && Key.Mode != TEXT("CIM_CurveBreak"))
            throw std::runtime_error("unsupported mover curve mode");
        Result.Add(Key);
    }
    if (Result[0].Time != 0 || FMath::Abs(Result.Last().Time - Duration) > 1e-5) throw std::runtime_error("incomplete mover curve");
    return Result;
}
// Pose of a Matinee move track (IMF_RelativeToInitial) relative to the placed pose. The placed pose is the pose at
// t=0, so keys are normalised by the first key: World(t) = Key(t) * Key(0)^-1 * Placed. The position delta is
// un-rotated by the first rotation key and rotated by the placed rotation. With a zero first rotation key (the
// door) this is exactly the actor-frame first-key delta used before. Euler keys are (X roll, Y pitch, Z yaw) in
// degrees. Parity with UE3's own composition is UNVERIFIED.
FTransform MatineePose(const FTransform& Placed, const TArray<FMoverKey>& Position, const TArray<FMoverKey>& Rotation, double Time) {
    auto Quat = [](const FVector& Euler) { return FRotator(Euler.Y, Euler.Z, Euler.X).Quaternion(); };
    const FQuat First = Quat(Rotation[0].Value);
    const FVector Delta = Evaluate(Position, Time) - Position[0].Value;
    FTransform Pose = Placed;
    Pose.SetLocation(Placed.GetLocation() + Placed.GetRotation().RotateVector(First.UnrotateVector(Delta)));
    Pose.SetRotation(Placed.GetRotation() * First.Inverse() * Quat(Evaluate(Rotation, Time)));
    return Pose;
}
// Scene actors carry their source component path as their first tag. Actor names repeat across sublevels, so the
// import's level folder (editor data; the host runs in the editor binary) selects the binding's package.
TArray<AActor*> SceneActors(UWorld* World, const FString& ActorPath, const FString& Level) {
    TArray<AActor*> Out;
    const FString Prefix = ActorPath + TEXT(".");
    for (TActorIterator<AActor> It(World); It; ++It) {
        if (!It->Tags.Num() || !It->Tags[0].ToString().StartsWith(Prefix)) continue;
#if WITH_EDITOR
        if (It->GetFolderPath().ToString() != Level) continue;
#endif
        Out.Add(*It);
    }
    return Out;
}
}
struct UOpenWillowMover::FImpl {
    TUniquePtr<vm::Mover> Script;
    TArray<FMoverKey> Position, Rotation;
    FString Package, DoorAction;
    TArray<FOpenWillowKismetRequest> Requests;
    // Second bound Matinee action.
    struct FKey { float Time; FString Name; bool bForward, bBackward; };
    struct FRestore { TWeakObjectPtr<USceneComponent> Component; FTransform Transform; EComponentMobility::Type Mobility; };
    FString TrackAction;
    float TrackDuration = 0;
    bool bRewindOnPlay = false;
    TArray<FMoverKey> TrackPosition, TrackRotation;
    TArray<FKey> TrackKeys;
    TArray<FRestore> Restore;
};
UOpenWillowMover::UOpenWillowMover() {
    PrimaryComponentTick.bCanEverTick = true;
    PrimaryComponentTick.TickGroup = TG_PostUpdateWork;
}
void UOpenWillowMover::BeginPlay() {
    Super::BeginPlay();
    Testing = FParse::Param(FCommandLine::Get(), TEXT("owmovertest"));
    FString File;
    if (!FParse::Value(FCommandLine::Get(), TEXT("owmover="), File)) {
        if (Testing) Fail(TEXT("missing -owmover manifest"));
        else SetComponentTickEnabled(false);
        return;
    }
    try {
        FString Text;
        TSharedPtr<FJsonObject> Data;
        if (!FFileHelper::LoadFileToString(Text, *File) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Data) || !Data)
            throw std::runtime_error("cannot load mover manifest");
        if (Data->GetStringField(TEXT("schema")) != TEXT("ow-mover-v1")) throw std::runtime_error("unsupported mover manifest");
        Duration = Data->GetNumberField(TEXT("duration"));
        if (!FMath::IsFinite(Duration) || Duration <= 0 || Duration > 60) throw std::runtime_error("invalid mover duration");
        Impl = MakeShared<FImpl>();
        Impl->Position = ReadKeys(Data, TEXT("position"), Duration);
        Impl->Rotation = ReadKeys(Data, TEXT("rotation"), Duration);
        const FString Component = Data->GetStringField(TEXT("component"));
        int32 Matches = 0;
        for (TActorIterator<AActor> It(GetWorld()); It; ++It) {
            if (!It->ActorHasTag(FName(*Component))) continue;
            auto* Candidate = It->FindComponentByClass<UStaticMeshComponent>();
            if (Candidate) { ++Matches; Mesh = Candidate; }
        }
        if (Matches != 1 || !Mesh || !Mesh->GetStaticMesh() || !Mesh->IsCollisionEnabled())
            throw std::runtime_error("mover component binding/collision is missing or ambiguous");
        Initial = Mesh->GetComponentTransform();
        OriginalMobility = Mesh->Mobility;
        PoseCaptured = true;
        Mesh->SetMobility(EComponentMobility::Movable);
        // Verify placement from the prepared manifest against this loaded map.
        const auto InitialData = Data->GetObjectField(TEXT("initial"));
        if (!InitialData) throw std::runtime_error("missing initial mover pose");
        const auto Actor = InitialData->GetObjectField(TEXT("actor"));
        const auto Local = InitialData->GetObjectField(TEXT("component"));
        if (!Vector(Local, TEXT("location")).IsNearlyZero() || !Vector(Local, TEXT("rotation")).IsNearlyZero()
            || !Vector(Local, TEXT("scale")).Equals(FVector::OneVector, 1e-4))
            throw std::runtime_error("non-identity mover component pose unsupported");
        const FVector R = Vector(Actor, TEXT("rotation"));
        const FQuat Q = FRotator(R.X, R.Y, R.Z).Quaternion();
        if (!Initial.GetLocation().Equals(Vector(Actor, TEXT("location")), 0.1)
            || !Initial.GetScale3D().Equals(Vector(Actor, TEXT("scale")), 1e-4)
            || Initial.GetRotation().AngularDistance(Q) > 0.001)
            throw std::runtime_error("loaded mover placement differs from manifest");
        const FString Game = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_BL2"));
        if (!FPaths::FileExists(FPaths::Combine(Game, TEXT("Binaries/Win32/Borderlands2.exe"))))
            throw std::runtime_error("installed Borderlands 2 is required");
        Impl->Package = Data->GetStringField(TEXT("package"));
        Impl->DoorAction = Data->GetStringField(TEXT("action"));
        Impl->Script = MakeUnique<vm::Mover>(std::filesystem::path(*FPaths::Combine(Game, TEXT("WillowGame/CookedPCConsole"))),
            TCHAR_TO_UTF8(*Impl->Package), TCHAR_TO_UTF8(*Data->GetStringField(TEXT("actor"))),
            TCHAR_TO_UTF8(*Data->GetStringField(TEXT("action"))));
        for (const auto& Warning : Impl->Script->loadingDiagnostics())
            UE_LOG(LogTemp, Warning, TEXT("OWMOVER loading diagnostic: %s"), UTF8_TO_TCHAR(Warning.c_str()));
        UE_LOG(LogTemp, Display, TEXT("OWMOVER READY component=%s duration=%.3f omitted_tracks=%d activation=developer"),
            *Component, Duration, Data->GetArrayField(TEXT("omitted_tracks")).Num());
        if (Testing) {
            const FBox Box = Mesh->GetStaticMesh()->GetBoundingBox();
            const FVector Extent = Box.GetExtent();
            int32 Axis = 0;
            for (int32 I = 1; I < 3; ++I) if (Extent[I] < Extent[Axis]) Axis = I;
            FVector Offset = FVector::ZeroVector; Offset[Axis] = Extent[Axis] + 30;
            RayStart = Box.GetCenter() - Offset; RayEnd = Box.GetCenter() + Offset;
            const FVector Center = Initial.TransformPosition(Box.GetCenter());
            const FVector Side = Initial.TransformVectorNoScale(Offset.GetSafeNormal());
            GetOwner()->SetActorLocation(Center - Side * 170, false, nullptr, ETeleportType::TeleportPhysics);
            if (auto* Walker = Cast<AOpenWillowWalker>(GetOwner())) {
                Walker->GetCharacterMovement()->SetMovementMode(MOVE_None);
                if (Walker->GetController()) Walker->GetController()->SetControlRotation((Center - (GetOwner()->GetActorLocation() + FVector(0,0,70))).Rotation());
            }
        }
    } catch (const std::exception& Error) { Fail(UTF8_TO_TCHAR(Error.what())); }
}
void UOpenWillowMover::EndPlay(const EEndPlayReason::Type Reason) {
    if (Mesh && PoseCaptured) { Mesh->SetWorldTransform(Initial, false, nullptr, ETeleportType::TeleportPhysics); Mesh->SetMobility(OriginalMobility); }
    if (Impl) RestoreTrack();
    Impl.Reset();
    Super::EndPlay(Reason);
}
void UOpenWillowMover::RestoreTrack() {
    // Children first (detach keeps their world pose), then every recorded component back to its placed pose/mobility.
    for (int32 I = Impl->Restore.Num() - 1; I >= 0; --I) {
        const auto& Entry = Impl->Restore[I];
        USceneComponent* Component = Entry.Component.Get();
        if (!Component) continue;
        if (Component != TrackMesh) Component->DetachFromComponent(FDetachmentTransformRules::KeepWorldTransform);
        Component->SetWorldTransform(Entry.Transform, false, nullptr, ETeleportType::TeleportPhysics);
        Component->SetMobility(Entry.Mobility);
    }
    Impl->Restore.Reset();
    bTrackRunning = false;
}
void UOpenWillowMover::Fail(const FString& Error) {
    Failed = true; Running = false;
    if (Mesh && PoseCaptured) { Mesh->SetWorldTransform(Initial, false, nullptr, ETeleportType::TeleportPhysics); Mesh->SetMobility(OriginalMobility); }
    if (Impl) RestoreTrack();
    UE_LOG(LogTemp, Error, TEXT("OWMOVER ERROR %s"), *Error);
    if (Testing) {
        UE_LOG(LogTemp, Display, TEXT("OWMOVERTEST SUMMARY result=FAIL checks=%d errors=%d reason=%s"), Checks, ++Errors, *Error);
        FPlatformMisc::RequestExit(false);
    }
}
bool UOpenWillowMover::TryInteract() {
    if (!Impl || Failed || !Mesh) return false;
    if (FVector::DistSquared(GetOwner()->GetActorLocation(), Mesh->Bounds.Origin) > FMath::Square(220.)) return false;
    if (Running) return true; // consume repeat key while moving
    StartMotion(Time >= Duration);
    return true;
}
void UOpenWillowMover::StartMotion(bool NextReverse) {
    const auto Result = Impl->Script->notify(false, NextReverse);
    ScriptSteps += Result.steps;
    if (!Result.error.empty()) { Fail(UTF8_TO_TCHAR(Result.error.c_str())); return; }
    // A Play / Reverse before the pending deactivation keeps the action running: no Completed / Reversed for the turned-around run.
    Reverse = NextReverse; Running = true; bDoorEndPending = false;
    ++(Reverse ? DoorCloseStarts : DoorOpenStarts);
    UE_LOG(LogTemp, Display, TEXT("OWMOVER start reverse=%d steps=%llu checkpoint=%d"), Reverse, uint64(Result.steps), Result.checkpoint);
}
bool UOpenWillowMover::RemoteEvent(const FString& Name) {
    LastEventMatched = 0; LastEventBoundary = 0;
    if (!Impl || Failed || !Mesh) return false;
    return ApplyDispatch(Impl->Script->remoteEvent(TCHAR_TO_UTF8(*Name)), Name);
}
bool UOpenWillowMover::MissionEvent(const FString& MissionPath, const FString& Name) {
    LastEventMatched = 0; LastEventBoundary = 0;
    if (!Impl || Failed || !Mesh) return false;
    return ApplyDispatch(Impl->Script->missionEvent(TCHAR_TO_UTF8(*MissionPath), TCHAR_TO_UTF8(*Name)), Name);
}
bool UOpenWillowMover::Anchor(FVector& Out) const {
    if (!Mesh || !Mesh->GetStaticMesh()) return false;
    Out = Initial.TransformPosition(Mesh->GetStaticMesh()->GetBoundingBox().GetCenter());
    return true;
}
bool UOpenWillowMover::StandPoint(FVector& Out) const {
    FVector Center;
    if (!Anchor(Center)) return false;
    const FBox Box = Mesh->GetStaticMesh()->GetBoundingBox();
    const FVector Extent = Box.GetExtent();
    int32 Axis = 0;
    for (int32 I = 1; I < 3; ++I) if (Extent[I] < Extent[Axis]) Axis = I;
    FVector Offset = FVector::ZeroVector; Offset[Axis] = Extent[Axis] + 30;
    Out = Center - Initial.TransformVectorNoScale(Offset.GetSafeNormal()) * 170;
    return true;
}
bool UOpenWillowMover::SequenceEvent(const FString& OpName) {
    LastEventMatched = 0; LastEventBoundary = 0; LastEventMotion = 0;
    if (!Impl || Failed || !Mesh) return false;
    return ApplyDispatch(Impl->Script->sequenceEvent(TCHAR_TO_UTF8(*OpName)), OpName);
}
bool UOpenWillowMover::OriginatorEvent(const FString& ObjectPath, TArray<FString>& Entered) {
    LastEventMatched = 0; LastEventBoundary = 0; LastEventMotion = 0;
    if (!Impl || Failed || !Mesh) return false;
    const auto Dispatch = Impl->Script->originatorEvent(TCHAR_TO_UTF8(*ObjectPath));
    for (const auto& Name : Dispatch.entered) Entered.Add(UTF8_TO_TCHAR(Name.c_str()));
    return ApplyDispatch(Dispatch, ObjectPath);
}
bool UOpenWillowMover::SequenceOutput(const FString& OpName, const FString& Desc) {
    LastEventMatched = 0; LastEventBoundary = 0; LastEventMotion = 0;
    if (!Impl || Failed || !Mesh) return false;
    return ApplyDispatch(Impl->Script->output(TCHAR_TO_UTF8(*OpName), TCHAR_TO_UTF8(*Desc)), OpName + TEXT(".") + Desc);
}
TArray<vm::Mover::Variable> UOpenWillowMover::SequenceVariables(const FString& OpName, const FString& Desc) {
    TArray<vm::Mover::Variable> Out;
    if (!Impl || Failed) return Out;
    for (const auto& Variable : Impl->Script->variables(TCHAR_TO_UTF8(*OpName), TCHAR_TO_UTF8(*Desc))) Out.Add(Variable);
    return Out;
}
TArray<FOpenWillowKismetRequest> UOpenWillowMover::DrainRequests() {
    if (!Impl) return {};
    TArray<FOpenWillowKismetRequest> Out = MoveTemp(Impl->Requests);
    Impl->Requests.Reset();
    return Out;
}
bool UOpenWillowMover::ApplyDispatch(const vm::Mover::Dispatch& Dispatch, const FString& Name) {
    for (const auto& Error : Dispatch.errors) {
        Fail(FString::Printf(TEXT("kismet event %s: %s"), *Name, UTF8_TO_TCHAR(Error.c_str())));
        return false;
    }
    LastEventMatched = int32(Dispatch.matched);
    LastEventBoundary = int32(Dispatch.hostBoundary.size());
    LastEventMotion = Dispatch.motion;
    if (Dispatch.matched || !Dispatch.trace.empty())
        UE_LOG(LogTemp, Display, TEXT("OWMOVER event=%s matched=%d motion=%d host_boundary=%d trace=%d"),
            *Name, LastEventMatched, Dispatch.motion, LastEventBoundary, int32(Dispatch.trace.size()));
    // World ops: the bound track's own Play/Reverse run here; everything else is queued for the quest component.
    for (const auto& Request : Dispatch.requests) {
        FOpenWillowKismetRequest Out{UTF8_TO_TCHAR(Request.cls.c_str()), UTF8_TO_TCHAR(Request.op.c_str()), UTF8_TO_TCHAR(Request.input.c_str())};
        if (TrackMesh && Out.Class == TEXT("Engine.SeqAct_Interp") && Out.Op == Impl->TrackAction
            && (Out.Input == TEXT("Play") || Out.Input == TEXT("Reverse"))) {
            UE_LOG(LogTemp, Display, TEXT("OWMOVER track %s <- %s"), *Out.Op, *Out.Input);
            StartTrack(Out.Input == TEXT("Reverse"));
            continue;
        }
        UE_LOG(LogTemp, Display, TEXT("OWMOVER host request: %s:%s <- %s"), *Out.Class, *Out.Op, *Out.Input);
        Impl->Requests.Add(Out);
    }
    if (Dispatch.motion == 0) return Dispatch.matched > 0;
    const bool NextReverse = Dispatch.motion < 0;
    if (Running) {
        // Play/Reverse while the door moves the other way (Marcus reaches the closing node before the opening
        // finishes): turn the motion around at its current position. InterpActor.InterpolationChanged is not run
        // and the turn-around semantics are UNVERIFIED against the original.
        if (NextReverse != Reverse) {
            Reverse = NextReverse;
            ++DoorTurnArounds;
            UE_LOG(LogTemp, Display, TEXT("OWMOVER door turns around at t=%.2f (reverse=%d)"), Time, Reverse);
        }
        return true;
    }
    if ((NextReverse && Time <= 0) || (!NextReverse && Time >= Duration)) return true; // already at that end
    StartMotion(NextReverse);
    return !Failed;
}
void UOpenWillowMover::TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function) {
    Super::TickComponent(Delta, Type, Function);
    if (!Impl || Failed || !Mesh) return;
    if (HasPendingRelease) {
        if (auto* Pawn = Cast<APawn>(GetOwner())) if (auto* PC = Cast<APlayerController>(Pawn->GetController()))
            PC->InputKey(FInputKeyEventArgs::CreateSimulated(PendingRelease, IE_Released, 0.f));
        HasPendingRelease = false;
    }
    const auto Timer = Impl->Script->advance(Delta);
    ScriptSteps += Timer.steps;
    if (!Timer.error.empty()) { Fail(UTF8_TO_TCHAR(Timer.error.c_str())); return; }
    // Sequence time (SeqAct_Delay and delayed links) advances with the world.
    ApplyDispatch(Impl->Script->advanceSequence(Delta), TEXT("sequence tick"));
    if (Failed) return;
    TickTrack(Delta);
    if (Failed) return;
    // SeqAct_Interp is seen as finished on the update after the one that reached the end (NATIVE_KISMET_MATINEE.md, UNVERIFIED): the
    // deactivation (InterpolationFinished, then the Completed / Reversed output) runs one frame after the last pose.
    if (bDoorEndPending) {
        bDoorEndPending = false;
        const auto Result = Impl->Script->notify(true, Reverse);
        ScriptSteps += Result.steps;
        if (!Result.error.empty()) { Fail(UTF8_TO_TCHAR(Result.error.c_str())); return; }
        ++(Reverse ? DoorCloseEnds : DoorOpenEnds);
        UE_LOG(LogTemp, Display, TEXT("OWMOVER finish reverse=%d steps=%llu checkpoint=%d"), Reverse, uint64(Result.steps), Result.checkpoint);
        ApplyDispatch(Impl->Script->motionFinished(Reverse), Reverse ? TEXT("door Reversed") : TEXT("door Completed"));
        if (Failed) return;
    }
    if (Running) {
        Time = FMath::Clamp(Time + (Reverse ? -Delta : Delta), 0.f, Duration);
        // UE3 relative-frame composition remains UNVERIFIED against original.
        Mesh->SetWorldTransform(MatineePose(Initial, Impl->Position, Impl->Rotation, Time), false, nullptr, ETeleportType::TeleportPhysics);
        if (Time == 0 || Time == Duration) { Running = false; bDoorEndPending = true; }
    }
    if (Testing) RunTest(Delta);
}
void UOpenWillowMover::BindTrack(const TSharedPtr<FJsonObject>& Binding) {
    if (!Impl || Failed || !Mesh) throw std::runtime_error("bind the door mover first (-owmover)");
    if (TrackMesh) throw std::runtime_error("a track is already bound");
    if (!Binding || Binding->GetStringField(TEXT("schema")) != TEXT("ow-mover-binding-v1")) throw std::runtime_error("unsupported track binding");
    if (Binding->GetStringField(TEXT("package")) != Impl->Package) throw std::runtime_error("track binding is in another package");
    const auto& Groups = Binding->GetArrayField(TEXT("groups"));
    if (Groups.Num() != 1) throw std::runtime_error("track binding must have exactly one group");
    const auto Group = Groups[0]->AsObject();
    const double TrackDuration = Binding->GetNumberField(TEXT("duration"));
    if (!FMath::IsFinite(TrackDuration) || TrackDuration <= 0 || TrackDuration > 60) throw std::runtime_error("invalid track duration");
    Impl->TrackDuration = TrackDuration;
    Impl->bRewindOnPlay = Binding->GetBoolField(TEXT("rewind_on_play"));
    const FString ActionPath = Binding->GetStringField(TEXT("action"));
    int32 Dot = INDEX_NONE;
    if (!ActionPath.FindLastChar(TEXT('.'), Dot) || !Impl->DoorAction.StartsWith(ActionPath.Left(Dot + 1)))
        throw std::runtime_error("track action is not in the door's Kismet sequence");
    Impl->TrackAction = ActionPath.Mid(Dot + 1);
    Impl->TrackPosition = ReadKeys(Group, TEXT("position"), TrackDuration);
    Impl->TrackRotation = ReadKeys(Group, TEXT("rotation"), TrackDuration);
    for (const auto& Track : Group->GetArrayField(TEXT("event_tracks"))) {
        const auto T = Track->AsObject();
        for (const auto& Key : T->GetArrayField(TEXT("keys"))) {
            const auto K = Key->AsObject();
            Impl->TrackKeys.Add({float(K->GetNumberField(TEXT("time"))), K->GetStringField(TEXT("name")),
                T->GetBoolField(TEXT("fires_forward")), T->GetBoolField(TEXT("fires_backward"))});
        }
    }
    // The group actor's mesh in the loaded map, checked against the placement the data records.
    const TArray<AActor*> Carriers = SceneActors(GetWorld(), Group->GetStringField(TEXT("actor")), Impl->Package);
    UStaticMeshComponent* Carrier = Carriers.Num() == 1 ? Carriers[0]->FindComponentByClass<UStaticMeshComponent>() : nullptr;
    if (!Carrier) throw std::runtime_error("track group actor is missing or ambiguous in the loaded map");
    const auto InitialData = Group->GetObjectField(TEXT("initial"));
    const FVector Units = Vector(InitialData, TEXT("rotation_units"));
    const FQuat Placed = FRotator(Units.X * 360.0 / 65536.0, Units.Y * 360.0 / 65536.0, Units.Z * 360.0 / 65536.0).Quaternion();
    const FTransform Loaded = Carrier->GetComponentTransform();
    if (!Loaded.GetLocation().Equals(Vector(InitialData, TEXT("location")), 0.1) || Loaded.GetRotation().AngularDistance(Placed) > 0.001
        || !Loaded.GetScale3D().Equals(FVector(InitialData->GetNumberField(TEXT("draw_scale"))), 1e-4))
        throw std::runtime_error("loaded track carrier placement differs from the binding");
    TrackMesh = Carrier;
    TrackInitial = Loaded;
    Impl->Restore.Add({Carrier, Loaded, Carrier->Mobility});
    Carrier->SetMobility(EComponentMobility::Movable);
    // Actors the data attaches to the group actor follow it (UE3 Base); ones with no prepared mesh are listed.
    for (const auto& Value : Group->GetArrayField(TEXT("attached"))) {
        const FString Name = Value->AsString();
        const TArray<AActor*> Children = SceneActors(GetWorld(), Name, Impl->Package);
        if (Children.Num() == 0) UE_LOG(LogTemp, Display, TEXT("OWMOVER track attachment %s has no prepared scene mesh (not moved)"), *Name);
        for (AActor* Child : Children) {
            USceneComponent* Root = Child->GetRootComponent();
            if (!Root) continue;
            Impl->Restore.Add({Root, Root->GetComponentTransform(), Root->Mobility});
            Root->SetMobility(EComponentMobility::Movable);
            Root->AttachToComponent(Carrier, FAttachmentTransformRules::KeepWorldTransform);
        }
    }
    UE_LOG(LogTemp, Display, TEXT("OWMOVER TRACK READY action=%s duration=%.3f keys=%d attached_components=%d omitted_tracks=%d"),
        *Impl->TrackAction, TrackDuration, Impl->TrackKeys.Num(), Impl->Restore.Num() - 1, Group->GetArrayField(TEXT("omitted_tracks")).Num());
}
FVector UOpenWillowMover::TrackOffset() const {
    return TrackMesh ? TrackMesh->GetComponentLocation() - TrackInitial.GetLocation() : FVector::ZeroVector;
}
void UOpenWillowMover::StartTrack(bool bReverse) {
    // bRewindOnPlay: Play starts from the beginning (read as "every Play rewinds"; UNVERIFIED).
    if (!bReverse && Impl->bRewindOnPlay) TrackTime = 0;
    bTrackReverse = bReverse;
    bTrackRunning = true;
    bTrackEndPending = false;
    FireTrackKeys(TrackTime, TrackTime, true);
}
void UOpenWillowMover::FireTrackKeys(float From, float To, bool bInclusiveFrom) {
    // Event-track keys fire the Matinee action's output of the same name, in the play direction their track allows.
    // Keys exactly at the start position fire when the motion starts. Boundary semantics UNVERIFIED.
    for (const auto& Key : Impl->TrackKeys) {
        const bool bHit = bTrackReverse
            ? Key.bBackward && ((Key.Time >= To && Key.Time < From) || (bInclusiveFrom && Key.Time == From))
            : Key.bForward && ((Key.Time > From && Key.Time <= To) || (bInclusiveFrom && Key.Time == From));
        if (!bHit) continue;
        UE_LOG(LogTemp, Display, TEXT("OWMOVER track key %s at %.2f"), *Key.Name, Key.Time);
        ApplyDispatch(Impl->Script->output(TCHAR_TO_UTF8(*Impl->TrackAction), TCHAR_TO_UTF8(*Key.Name)), Key.Name);
        if (Failed) return;
    }
}
void UOpenWillowMover::TickTrack(float Delta) {
    if (!TrackMesh) return;
    // As the door: the output follows one frame after the last pose (NATIVE_KISMET_MATINEE.md, UNVERIFIED).
    if (bTrackEndPending) {
        bTrackEndPending = false;
        ++(bTrackReverse ? TrackReverseEnds : TrackForwardEnds);
        ApplyDispatch(Impl->Script->output(TCHAR_TO_UTF8(*Impl->TrackAction), bTrackReverse ? "Reversed" : "Completed"),
            bTrackReverse ? TEXT("track Reversed") : TEXT("track Completed"));
        return;
    }
    if (!bTrackRunning) return;
    const float Before = TrackTime;
    TrackTime = FMath::Clamp(TrackTime + (bTrackReverse ? -Delta : Delta), 0.f, Impl->TrackDuration);
    FireTrackKeys(Before, TrackTime, false);
    if (Failed) return;
    // Same composition as the door (MatineePose). This carrier's first rotation key is not zero, so its delta is
    // un-rotated by that key before the placed rotation applies (frame UNVERIFIED; see SANCTUARY_RPG_MISSION.md).
    TrackMesh->SetWorldTransform(MatineePose(TrackInitial, Impl->TrackPosition, Impl->TrackRotation, TrackTime),
        false, nullptr, ETeleportType::TeleportPhysics);
    if ((bTrackReverse && TrackTime <= 0) || (!bTrackReverse && TrackTime >= Impl->TrackDuration)) {
        bTrackRunning = false;
        bTrackEndPending = true;
    }
}
bool UOpenWillowMover::Ray(bool ClosedSpace) const {
    const FTransform Pose = ClosedSpace ? Initial : Mesh->GetComponentTransform();
    FHitResult Hit;
    FCollisionQueryParams Params(SCENE_QUERY_STAT(OWMoverTest), false);
    return Mesh->LineTraceComponent(Hit, Pose.TransformPosition(RayStart), Pose.TransformPosition(RayEnd), Params);
}
void UOpenWillowMover::Check(bool Good, const TCHAR* Name) {
    ++Checks; if (!Good) ++Errors;
    UE_LOG(LogTemp, Display, TEXT("OWMOVERTEST check=%d name=%s ok=%d steps=%lld"), Checks, Name, Good, ScriptSteps);
}
void UOpenWillowMover::RunTest(float Delta) {
    TestWait += Delta;
    if (TestWait < 0.35f) return;
    auto Capture = [](const TCHAR* Name) { FScreenshotRequest::RequestScreenshot(FString::Printf(TEXT("OWMover_%s.png"), Name), false, false); };
    auto Interact = [this] {
        if (auto* Pawn = Cast<APawn>(GetOwner())) if (auto* PC = Cast<APlayerController>(Pawn->GetController())) {
            PC->InputKey(FInputKeyEventArgs::CreateSimulated(EKeys::E, IE_Pressed, 1.f));
            PendingRelease = EKeys::E; HasPendingRelease = true;
        }
    };
    switch (TestStep) {
    case 0: Check(Ray(true), TEXT("closed_collision")); Capture(TEXT("closed")); break;
    case 1: Interact(); break;
    case 2: if (Running && Time < Duration * 0.4f) return; Check(Running, TEXT("interaction_start")); Check(!Initial.Equals(Mesh->GetComponentTransform(), 0.01), TEXT("animated_midpoint")); Capture(TEXT("midpoint")); break;
    case 3: if (Running) return; Check(Time == Duration && !Ray(true) && Ray(false), TEXT("open_collision_moved")); Capture(TEXT("open")); break;
    case 4: Interact(); break;
    case 5: if (Running) return; Check(Reverse, TEXT("close_start")); Check(Time == 0 && Initial.Equals(Mesh->GetComponentTransform(), 0.01) && Ray(true), TEXT("reclosed_collision")); break;
    case 6: Interact(); break;
    case 7: if (Running) return; Check(!Reverse, TEXT("second_open_start")); Check(Time == Duration && !Ray(true) && Ray(false), TEXT("second_open_collision")); break;
    case 8: Interact(); break;
    case 9: if (Running) return; Check(Reverse, TEXT("second_close_start")); Check(Time == 0 && Initial.Equals(Mesh->GetComponentTransform(), 0.01) && Ray(true) && ScriptSteps > 0, TEXT("second_close_collision")); break;
    // Stock activation: the installed remote events RE_Ep14_OpenMarcusDoor / RE_Ep14_CloseMarcusDoor drive the
    // same Matinee action through its Kismet sequence (no developer key).
    case 10: Check(RemoteEvent(TEXT("RE_Ep14_OpenMarcusDoor")) && Running && !Reverse && LastEventMatched == 1, TEXT("stock_open_event_start")); break;
    case 11: if (Running) return; Check(Time == Duration && !Ray(true) && Ray(false), TEXT("stock_open_event_collision")); break;
    case 12: Check(RemoteEvent(TEXT("RE_Ep14_CloseMarcusDoor")) && Running && Reverse && LastEventMatched == 1, TEXT("stock_close_event_start")); break;
    case 13: if (Running) return; Check(Time == 0 && Initial.Equals(Mesh->GetComponentTransform(), 0.01) && Ray(true), TEXT("stock_close_event_collision")); break;
    // An installed event that reaches a world-acting op this host does not bind must report it and not move the door.
    case 14: Check(RemoteEvent(TEXT("RocksPaper_MoveTargetForward")) && !Running && Time == 0 && LastEventMatched == 1 && LastEventBoundary == 1, TEXT("stock_unbound_op_reported_no_motion")); break;
    case 15: Check(!RemoteEvent(TEXT("OpenWillow_NoSuchEvent")) && !Running && LastEventMatched == 0, TEXT("unknown_event_ignored")); break;
    default:
        UE_LOG(LogTemp, Display, TEXT("OWMOVERTEST SUMMARY result=%s checks=%d errors=%d steps=%lld"), Errors ? TEXT("FAIL") : TEXT("PASS"), Checks, Errors, ScriptSteps);
        FPlatformMisc::RequestExit(false); Testing = false; return;
    }
    ++TestStep; TestWait = 0;
}
