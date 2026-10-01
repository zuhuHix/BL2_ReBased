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
}
struct UOpenWillowMover::FImpl {
    TUniquePtr<vm::Mover> Script;
    TArray<FMoverKey> Position, Rotation;
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
        Impl->Script = MakeUnique<vm::Mover>(std::filesystem::path(*FPaths::Combine(Game, TEXT("WillowGame/CookedPCConsole"))),
            TCHAR_TO_UTF8(*Data->GetStringField(TEXT("package"))), TCHAR_TO_UTF8(*Data->GetStringField(TEXT("actor"))),
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
    Impl.Reset();
    Super::EndPlay(Reason);
}
void UOpenWillowMover::Fail(const FString& Error) {
    Failed = true; Running = false;
    if (Mesh && PoseCaptured) { Mesh->SetWorldTransform(Initial, false, nullptr, ETeleportType::TeleportPhysics); Mesh->SetMobility(OriginalMobility); }
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
    const bool NextReverse = Time >= Duration;
    const auto Result = Impl->Script->notify(false, NextReverse);
    ScriptSteps += Result.steps;
    if (!Result.error.empty()) { Fail(UTF8_TO_TCHAR(Result.error.c_str())); return true; }
    Reverse = NextReverse; Running = true;
    UE_LOG(LogTemp, Display, TEXT("OWMOVER start reverse=%d steps=%llu checkpoint=%d"), Reverse, uint64(Result.steps), Result.checkpoint);
    return true;
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
    if (Running) {
        Time = FMath::Clamp(Time + (Reverse ? -Delta : Delta), 0.f, Duration);
        const FVector Position = Evaluate(Impl->Position, Time) - Impl->Position[0].Value;
        const FVector Euler = Evaluate(Impl->Rotation, Time) - Impl->Rotation[0].Value;
        FTransform Pose = Initial;
        // UE3 relative-frame composition remains UNVERIFIED against original.
        Pose.SetLocation(Initial.GetLocation() + Initial.TransformVectorNoScale(Position));
        Pose.SetRotation(Initial.GetRotation() * FRotator(Euler.Y, Euler.Z, Euler.X).Quaternion());
        Mesh->SetWorldTransform(Pose, false, nullptr, ETeleportType::TeleportPhysics);
        if (Time == 0 || Time == Duration) {
            const auto Result = Impl->Script->notify(true, Reverse);
            ScriptSteps += Result.steps;
            if (!Result.error.empty()) { Fail(UTF8_TO_TCHAR(Result.error.c_str())); return; }
            Running = false;
            UE_LOG(LogTemp, Display, TEXT("OWMOVER finish reverse=%d steps=%llu checkpoint=%d"), Reverse, uint64(Result.steps), Result.checkpoint);
        }
    }
    if (Testing) RunTest(Delta);
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
    default:
        UE_LOG(LogTemp, Display, TEXT("OWMOVERTEST SUMMARY result=%s checks=%d errors=%d steps=%lld"), Errors ? TEXT("FAIL") : TEXT("PASS"), Checks, Errors, ScriptSteps);
        FPlatformMisc::RequestExit(false); Testing = false; return;
    }
    ++TestStep; TestWait = 0;
}
