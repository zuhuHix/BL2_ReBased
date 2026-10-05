#include "OpenWillowAmbient.h"
#include "OpenWillowWalker.h"
#include "Animation/AnimSequence.h"
#include "Camera/PlayerCameraManager.h"
#include "Components/SkeletalMeshComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "Engine/StaticMesh.h"
#include "Engine/Engine.h"
#include "Engine/GameViewportClient.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/World.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "GameFramework/PlayerController.h"
#include "HAL/PlatformMisc.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "UnrealClient.h"
#include <stdexcept>

namespace
{
[[noreturn]] void Missing(const FString& What)
{
    throw std::runtime_error(TCHAR_TO_UTF8(*FString::Printf(TEXT("ambient manifest: %s"), *What)));
}

TSharedPtr<FJsonObject> Obj(const TSharedPtr<FJsonObject>& J, const TCHAR* Key)
{
    const TSharedPtr<FJsonObject>* Out = nullptr;
    if (!J || !J->TryGetObjectField(Key, Out) || !Out || !*Out) Missing(FString::Printf(TEXT("missing object '%s'"), Key));
    return *Out;
}

const TArray<TSharedPtr<FJsonValue>>& Arr(const TSharedPtr<FJsonObject>& J, const TCHAR* Key)
{
    const TArray<TSharedPtr<FJsonValue>>* Out = nullptr;
    if (!J || !J->TryGetArrayField(Key, Out) || !Out) Missing(FString::Printf(TEXT("missing array '%s'"), Key));
    return *Out;
}

double Num(const TSharedPtr<FJsonObject>& J, const TCHAR* Key)
{
    double Out = 0;
    if (!J || !J->TryGetNumberField(Key, Out) || !FMath::IsFinite(Out)) Missing(FString::Printf(TEXT("missing number '%s'"), Key));
    return Out;
}

FString Str(const TSharedPtr<FJsonObject>& J, const TCHAR* Key)
{
    FString Out;
    if (!J || !J->TryGetStringField(Key, Out) || Out.IsEmpty()) Missing(FString::Printf(TEXT("missing string '%s'"), Key));
    return Out;
}

FVector Vec(const TSharedPtr<FJsonObject>& J, const TCHAR* Key)
{
    const auto& A = Arr(J, Key);
    if (A.Num() != 3) Missing(FString::Printf(TEXT("'%s' is not a 3-vector"), Key));
    FVector V;
    for (int32 I = 0; I < 3; ++I)
    {
        double N = 0;
        if (!A[I]->TryGetNumber(N) || !FMath::IsFinite(N) || FMath::Abs(N) > 1e7) Missing(FString::Printf(TEXT("invalid '%s'"), Key));
        V[I] = N;
    }
    return V;
}

// "/Game/Dir/Asset" -> "/Game/Dir/Asset.Asset" for LoadObject.
FString AssetPath(const FString& Path)
{
    if (Path.IsEmpty() || Path.Contains(TEXT("."))) return Path;
    return Path + TEXT(".") + FPaths::GetBaseFilename(Path);
}

TMap<FString, FLinearColor> ReadColours(const TSharedPtr<FJsonObject>& J, const TCHAR* Key)
{
    TMap<FString, FLinearColor> Out;
    const TSharedPtr<FJsonObject>* Object = nullptr;
    if (J->TryGetObjectField(Key, Object) && Object && *Object)
        for (const auto& Pair : (*Object)->Values)
        {
            const TArray<TSharedPtr<FJsonValue>>* V = nullptr;
            if (Pair.Value->TryGetArray(V) && V && V->Num() >= 3)
                Out.Add(FString(*Pair.Key), FLinearColor((*V)[0]->AsNumber(), (*V)[1]->AsNumber(), (*V)[2]->AsNumber(), 1.f));
        }
    return Out;
}

// The pawn's own material-clone colours on a zone material instance (a dynamic instance per pawn).
UMaterialInterface* Dress(UMaterialInterface* Base, const TMap<FString, FLinearColor>& Colours, UObject* Outer)
{
    if (!Base || Colours.Num() == 0) return Base;
    UMaterialInstanceDynamic* Dynamic = UMaterialInstanceDynamic::Create(Base, Outer);
    for (const auto& Pair : Colours) Dynamic->SetVectorParameterValue(FName(*Pair.Key), Pair.Value);
    return Dynamic;
}

// Fixed seed per pawn so that a capture run repeats (the original's random choices are native and not reproduced).
FRandomStream MakeStream(const FString& Id) { return FRandomStream(int32(GetTypeHash(Id) & 0x7fffffff)); }
}

// ---------------------------------------------------------------------------------------------------- manifest
void FOpenWillowAmbientWorld::Load(const FString& File)
{
    FString Text;
    TSharedPtr<FJsonObject> Data;
    if (!FFileHelper::LoadFileToString(Text, *File)) Missing(TEXT("cannot read ") + File);
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Data) || !Data) Missing(TEXT("invalid JSON in ") + File);
    if (Str(Data, TEXT("schema")) != TEXT("ow-ambient-world-v1")) Missing(TEXT("schema is not ow-ambient-world-v1"));

    for (const auto& Pair : Obj(Data, TEXT("kinds"))->Values)
    {
        const FString KindName(*Pair.Key);
        const auto K = Pair.Value->AsObject();
        FOpenWillowAmbientKind Out;
        Out.DisplayName = Str(K, TEXT("display_name"));
        Out.Mesh = AssetPath(Str(K, TEXT("mesh")));
        Out.MeshOffset = Vec(K, TEXT("mesh_offset"));
        Out.Speed = Num(K, TEXT("speed"));
        Out.YawRate = Num(K, TEXT("yaw_rate"));
        double Ink = 0;
        if (K->TryGetNumberField(TEXT("outline_cm"), Ink)) Out.OutlineCm = float(Ink);
        if (K->TryGetNumberField(TEXT("outline_px"), Ink)) Out.OutlinePx = float(Ink);
        for (const auto& Clip : Obj(K, TEXT("clips"))->Values) Out.Clips.Add(FString(*Clip.Key), AssetPath(Clip.Value->AsString()));
        const TSharedPtr<FJsonObject>* RootEnds = nullptr;
        if (K->TryGetObjectField(TEXT("root_end"), RootEnds) && RootEnds && *RootEnds)
            for (const auto& Entry : (*RootEnds)->Values) Out.RootEnd.Add(FString(*Entry.Key), Vec(*RootEnds, *Entry.Key));
        if (!Out.Clips.Contains(TEXT("idle")) || !Out.Clips.Contains(TEXT("walk"))) Missing(KindName + TEXT(" has no idle/walk clip"));
        Kinds.Add(KindName, Out);
    }
    for (const auto& Pair : Obj(Data, TEXT("perches"))->Values)
    {
        const FString PerchName(*Pair.Key);
        const auto P = Pair.Value->AsObject();
        FOpenWillowAmbientPerch Out;
        P->TryGetStringField(TEXT("start"), Out.Start);
        P->TryGetStringField(TEXT("stop"), Out.Stop);
        for (const auto& Entry : Arr(P, TEXT("idle")))
            Out.Idle.Add({Str(Entry->AsObject(), TEXT("role")), float(Num(Entry->AsObject(), TEXT("weight")))});
        if (Out.Idle.Num() == 0) Missing(PerchName + TEXT(" has no idle clip"));
        const auto& Loop = Arr(P, TEXT("loop_time"));
        if (Loop.Num() != 2) Missing(PerchName + TEXT(" loop_time is not [min, max]"));
        Out.LoopTime = FVector2D(Loop[0]->AsNumber(), Loop[1]->AsNumber());
        double Lerp = 0;
        if (P->TryGetNumberField(TEXT("lerp_time"), Lerp)) Out.LerpTime = float(Lerp);
        Perches.Add(PerchName, Out);
    }
    TMap<FString, int32> Index;
    const auto& NodeList = Arr(Data, TEXT("nodes"));
    for (const auto& Value : NodeList)
    {
        const auto N = Value->AsObject();
        FOpenWillowAmbientNode Out;
        Out.Name = Str(N, TEXT("name"));
        Out.Location = Vec(N, TEXT("location"));
        Out.Yaw = Num(N, TEXT("yaw"));
        Out.ArrivalRadius = Num(N, TEXT("arrival_radius"));
        Out.bFaceNodeDirection = N->GetBoolField(TEXT("face_node_direction"));
        N->TryGetStringField(TEXT("perch"), Out.Perch);
        if (!Out.Perch.IsEmpty() && !Perches.Contains(Out.Perch)) Missing(Out.Name + TEXT(": unknown perch ") + Out.Perch);
        const TArray<TSharedPtr<FJsonValue>>* Loop = nullptr;
        if (N->TryGetArrayField(TEXT("loop_override"), Loop) && Loop && Loop->Num() == 2)
            Out.LoopOverride = FVector2D((*Loop)[0]->AsNumber(), (*Loop)[1]->AsNumber());
        Index.Add(Out.Name, Nodes.Num());
        Nodes.Add(Out);
    }
    for (int32 I = 0; I < Nodes.Num(); ++I)
        for (const auto& Edge : Arr(NodeList[I]->AsObject(), TEXT("next")))
        {
            const FString Target = Str(Edge->AsObject(), TEXT("node"));
            const int32* Found = Index.Find(Target);
            if (!Found) Missing(Nodes[I].Name + TEXT(": next node not in the manifest: ") + Target);
            Nodes[I].Next.Add({*Found, float(Num(Edge->AsObject(), TEXT("weight")))});
        }
    for (const auto& Value : Arr(Data, TEXT("spawns")))
    {
        const auto S = Value->AsObject();
        FOpenWillowAmbientSpawn Out;
        Out.Id = Str(S, TEXT("id"));
        Out.Den = Str(S, TEXT("den"));
        Out.Population = Str(S, TEXT("population"));
        Out.Kind = Str(S, TEXT("kind"));
        if (!Kinds.Contains(Out.Kind)) Missing(Out.Id + TEXT(": unknown kind ") + Out.Kind);
        Out.Location = Vec(S, TEXT("location"));
        Out.Yaw = Num(S, TEXT("yaw"));
        const FString Start = Str(S, TEXT("start_node"));
        const int32* Found = Index.Find(Start);
        if (!Found) Missing(Out.Id + TEXT(": start node not in the manifest: ") + Start);
        Out.StartNode = *Found;
        Out.bWander = S->GetBoolField(TEXT("wander"));
        Out.bLoadBalanced = S->GetBoolField(TEXT("load_balanced"));
        S->TryGetBoolField(TEXT("hold"), Out.bHold);
        S->TryGetBoolField(TEXT("fixed_z"), Out.bFixedZ);
        S->TryGetStringField(TEXT("head_material"), Out.HeadMaterial);
        S->TryGetStringField(TEXT("body_material"), Out.BodyMaterial);
        Out.HeadVectors = ReadColours(S, TEXT("head_vectors"));
        Out.BodyVectors = ReadColours(S, TEXT("body_vectors"));
        const TArray<TSharedPtr<FJsonValue>>* Worn = nullptr;
        if (S->TryGetArrayField(TEXT("attachments"), Worn) && Worn)
            for (const auto& WornValue : *Worn)
            {
                const auto A = WornValue->AsObject();
                FOpenWillowAmbientAttachment Att;
                Att.Mesh = AssetPath(Str(A, TEXT("mesh")));
                Att.Bone = Str(A, TEXT("bone"));
                A->TryGetStringField(TEXT("material"), Att.Material);
                Att.Material = AssetPath(Att.Material);
                Att.Location = Vec(A, TEXT("location"));
                Att.Vectors = ReadColours(A, TEXT("vectors"));
                const auto& Q = Arr(A, TEXT("quat"));
                if (Q.Num() != 4) Missing(Out.Id + TEXT(": attachment quat is not 4 numbers"));
                Att.Rotation = FQuat(Q[0]->AsNumber(), Q[1]->AsNumber(), Q[2]->AsNumber(), Q[3]->AsNumber()).GetNormalized();
                const TArray<TSharedPtr<FJsonValue>>* ScaleList = nullptr;
                if (A->TryGetArrayField(TEXT("scale"), ScaleList) && ScaleList && ScaleList->Num() == 3)
                    Att.Scale = FVector((*ScaleList)[0]->AsNumber(), (*ScaleList)[1]->AsNumber(), (*ScaleList)[2]->AsNumber());
                const TArray<TSharedPtr<FJsonValue>>* Tint = nullptr;
                if (A->TryGetArrayField(TEXT("tint"), Tint) && Tint && Tint->Num() == 3)
                {
                    Att.bTint = true;
                    Att.Tint = FLinearColor((*Tint)[0]->AsNumber(), (*Tint)[1]->AsNumber(), (*Tint)[2]->AsNumber(), 1.f);
                }
                Out.Attachments.Add(Att);
            }
        Spawns.Add(Out);
    }
    const TArray<TSharedPtr<FJsonValue>>* ShowList = nullptr;
    if (Data->TryGetArrayField(TEXT("showcase"), ShowList) && ShowList)
        for (const auto& Value : *ShowList) Showcase.Add(Value->AsString());
    const TArray<TSharedPtr<FJsonValue>>* ViewList = nullptr;
    if (Data->TryGetArrayField(TEXT("views"), ViewList) && ViewList)
        for (const auto& Value : *ViewList)
        {
            FOpenWillowAmbientView View;
            View.Name = Str(Value->AsObject(), TEXT("name"));
            View.Camera = Vec(Value->AsObject(), TEXT("camera"));
            View.Look = Vec(Value->AsObject(), TEXT("look"));
            Views.Add(View);
        }
}

// ---------------------------------------------------------------------------------------------------- pawn
AOpenWillowAmbientNpc::AOpenWillowAmbientNpc()
{
    PrimaryActorTick.bCanEverTick = true;
    // The actor origin is the UE3 pawn location; the mesh hangs below it by the pawn's component Translation.
    Root = CreateDefaultSubobject<USceneComponent>(TEXT("PawnOrigin"));
    RootComponent = Root;
    Mesh = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("Mesh"));
    Mesh->SetupAttachment(Root);
    Mesh->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Outline = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("Outline"));
    Outline->SetupAttachment(Mesh);
    Outline->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Outline->SetCastShadow(false);
}

UAnimSequence* AOpenWillowAmbientNpc::Clip(const FString& RoleName) const
{
    const TObjectPtr<UAnimSequence>* Found = Loaded.Find(RoleName);
    return Found ? Found->Get() : nullptr;
}

bool AOpenWillowAmbientNpc::Setup(TSharedPtr<const FOpenWillowAmbientWorld> InWorld, const FOpenWillowAmbientSpawn& Spawn)
{
    World = InWorld;
    SpawnData = Spawn;
    Kind = World->Kinds.Find(Spawn.Kind);
    if (!Kind) return false;
    USkeletalMesh* Skeletal = LoadObject<USkeletalMesh>(nullptr, *Kind->Mesh);
    if (!Skeletal) return false;
    Mesh->SetSkeletalMesh(Skeletal);
    Mesh->SetRelativeLocation(Kind->MeshOffset);
    // Pawns far away or behind the camera need no pose; the walk logic below does not depend on the animation.
    Mesh->VisibilityBasedAnimTickOption = EVisibilityBasedAnimTickOption::OnlyTickPoseWhenRendered;
    for (const auto& Clip : Kind->Clips)
    {
        UAnimSequence* Sequence = LoadObject<UAnimSequence>(nullptr, *Clip.Value);
        if (!Sequence) { UE_LOG(LogTemp, Warning, TEXT("OWAMBIENT %s: clip %s (%s) did not load"), *Spawn.Id, *Clip.Key, *Clip.Value); continue; }
        Loaded.Add(Clip.Key, Sequence);
    }
    if (!Clip(TEXT("idle")) || !Clip(TEXT("walk"))) return false;
    if (!Spawn.HeadMaterial.IsEmpty())
    {
        if (UMaterialInterface* Head = LoadObject<UMaterialInterface>(nullptr, *AssetPath(Spawn.HeadMaterial))) Mesh->SetMaterial(0, Dress(Head, Spawn.HeadVectors, this));
        else UE_LOG(LogTemp, Warning, TEXT("OWAMBIENT %s: head material %s did not load"), *Spawn.Id, *Spawn.HeadMaterial);
    }
    if (!Spawn.BodyMaterial.IsEmpty())
    {
        if (UMaterialInterface* Body = LoadObject<UMaterialInterface>(nullptr, *AssetPath(Spawn.BodyMaterial))) Mesh->SetMaterial(1, Dress(Body, Spawn.BodyVectors, this));
        else UE_LOG(LogTemp, Warning, TEXT("OWAMBIENT %s: body material %s did not load"), *Spawn.Id, *Spawn.BodyMaterial);
    }
    // Ink line: a copy of the mesh that follows the body, back faces only, pushed out along the normal (the material is the
    // inverted-hull one host/ue5/import_character_menu_look.py builds for Maya; the original's shader is not read, UNVERIFIED).
    UMaterialInterface* Ink = Kind->OutlineCm > 0.f ? LoadObject<UMaterialInterface>(nullptr, TEXT("/Game/OpenWillow/Characters/Ambient/Attachments/M_OW_AmbientOutline.M_OW_AmbientOutline")) : nullptr;
    Outline->SetVisibility(Ink != nullptr);
    if (Ink)
    {
        Outline->SetSkeletalMesh(Skeletal);
        for (int32 Slot = 0; Slot < Outline->GetNumMaterials(); ++Slot) Outline->SetMaterial(Slot, Ink);
        Outline->SetScalarParameterValueOnMaterials(TEXT("ThicknessCm"), Kind->OutlineCm);
        Outline->SetLeaderPoseComponent(Mesh);
    }
    for (const FOpenWillowAmbientAttachment& Att : Spawn.Attachments)
    {
        UStaticMesh* Piece = LoadObject<UStaticMesh>(nullptr, *AssetPath(Att.Mesh));
        if (!Piece) { UE_LOG(LogTemp, Warning, TEXT("OWAMBIENT %s: attachment %s did not load"), *Spawn.Id, *Att.Mesh); continue; }
        UStaticMeshComponent* Component = NewObject<UStaticMeshComponent>(this);
        Component->SetStaticMesh(Piece);
        Component->SetCollisionEnabled(ECollisionEnabled::NoCollision);
        Component->SetCastShadow(true);
        Component->SetupAttachment(Mesh, FName(*Att.Bone));
        Component->SetRelativeLocationAndRotation(Att.Location, Att.Rotation.Rotator());
        Component->SetRelativeScale3D(Att.Scale);
        Component->RegisterComponent();
        if (!Att.Material.IsEmpty())
            if (UMaterialInterface* Look = LoadObject<UMaterialInterface>(nullptr, *Att.Material))
            {
                if (Att.Vectors.Num() > 0) Component->SetMaterial(0, Dress(Look, Att.Vectors, this));
                else if (Att.bTint)
                {
                    UMaterialInstanceDynamic* Dynamic = UMaterialInstanceDynamic::Create(Look, this);
                    Dynamic->SetVectorParameterValue(TEXT("Tint"), Att.Tint);
                    Component->SetMaterial(0, Dynamic);
                }
                else Component->SetMaterial(0, Look);
            }
        Worn.Add(Component);
        if (Ink)
        {
            // The same piece again with the ink material (back faces only, pushed out along the normal), so hats and hair carry the line too.
            UStaticMeshComponent* Hull = NewObject<UStaticMeshComponent>(this);
            Hull->SetStaticMesh(Piece);
            Hull->SetCollisionEnabled(ECollisionEnabled::NoCollision);
            Hull->SetCastShadow(false);
            Hull->SetupAttachment(Mesh, FName(*Att.Bone));
            Hull->SetRelativeLocationAndRotation(Att.Location, Att.Rotation.Rotator());
            Hull->SetRelativeScale3D(Att.Scale);
            Hull->RegisterComponent();
            for (int32 Slot = 0; Slot < Hull->GetNumMaterials(); ++Slot) Hull->SetMaterial(Slot, Ink);
            WornInk.Add(Hull);
        }
    }
    SetActorLocation(Spawn.Location);
    SetActorRotation(FRotator(0, Spawn.Yaw, 0));
    SnapToFloor(0, true);
    Target = Spawn.StartNode;
    if (Spawn.bHold)
    {
        // Observed standing where the population left it (no perch under it): stay and idle.
        State = EPhase::Holding;
        Play(TEXT("idle"), true);
        return true;
    }
    if (!Spawn.bWander && !World->Nodes[Target].Perch.IsEmpty())
    {
        // Idle pawns ("Perch Only AI"): the scripted target is a perch, so the pawn is snapped onto it (script
        // Action_ScriptedNPC.IdleNPC) and starts the perch cycle at once.
        SetActorLocation(FVector(World->Nodes[Target].Location.X, World->Nodes[Target].Location.Y, Spawn.bFixedZ ? Spawn.Location.Z : GetActorLocation().Z));
        SetActorRotation(FRotator(0, World->Nodes[Target].Yaw, 0));
        SnapToFloor(0, true);
        Arrive();
        return true;
    }
    BeginTravel();
    return true;
}

void AOpenWillowAmbientNpc::Play(const FString& RoleName, bool bLoop)
{
    UAnimSequence* Sequence = Clip(RoleName);
    if (!Sequence) Sequence = Clip(TEXT("idle"));
    if (!Sequence) return;
    PlayingRole = RoleName;
    Mesh->PlayAnimation(Sequence, bLoop);
}

bool AOpenWillowAmbientNpc::IsPlayingRole(const FString& RoleName) const { return PlayingRole == RoleName; }

void AOpenWillowAmbientNpc::BeginTravel()
{
    // A load-balanced pawn waits (standing; 0.5 s AI updates in the original) until the balancer lets it path.
    PhaseTime = 0;
    if (SpawnData.bLoadBalanced)
    {
        State = EPhase::Waiting;
        Play(TEXT("idle"), true);
        return;
    }
    State = EPhase::Walking;
    Play(TEXT("walk"), true);
}

void AOpenWillowAmbientNpc::AdmitToPath()
{
    if (State != EPhase::Waiting) return;
    State = EPhase::Walking;
    PhaseTime = 0;
    Play(TEXT("walk"), true);
}

FString AOpenWillowAmbientNpc::Phase() const
{
    switch (State)
    {
    case EPhase::Waiting: return TEXT("waiting");
    case EPhase::Walking: return TEXT("walking");
    case EPhase::PerchStart: return TEXT("perch-start");
    case EPhase::PerchLoop: return TEXT("perch-loop");
    case EPhase::PerchStop: return TEXT("perch-stop");
    default: return TEXT("holding");
    }
}

void AOpenWillowAmbientNpc::SnapToFloor(float DeltaSeconds, bool bImmediate)
{
    // Stand the feet on whatever the visibility trace hits under the pawn (the original uses navigation data:
    // UNVERIFIED). The origin hangs |MeshOffset.Z| above the feet.
    const FVector At = GetActorLocation();
    const float Reach = bHaveFloor ? 120.f : 300.f;
    const float From = bHaveFloor ? FloorZ + Reach : At.Z + 80.f;   // population points sit about 50 above the floor
    FHitResult Hit;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowAmbientFloor), true, this);
    if (GetWorld()->LineTraceSingleByChannel(Hit, FVector(At.X, At.Y, From), FVector(At.X, At.Y, From - Reach - 400.f), ECC_Visibility, Query))
    {
        if (bHaveFloor && FMath::Abs(Hit.ImpactPoint.Z - FloorZ) > 150.f && !bLoggedJump)
        {
            bLoggedJump = true;
            UE_LOG(LogTemp, Warning, TEXT("OWAMBIENT %s floor jump %.0f -> %.0f at %s on %s"), *SpawnData.Id, FloorZ, Hit.ImpactPoint.Z,
                *At.ToString(), Hit.GetComponent() ? *Hit.GetComponent()->GetPathName() : TEXT("?"));
        }
        FloorZ = bImmediate || !bHaveFloor ? Hit.ImpactPoint.Z : FMath::FInterpTo(FloorZ, Hit.ImpactPoint.Z, DeltaSeconds, 12.f);
        bHaveFloor = true;
    }
    if (bHaveFloor && !SpawnData.bFixedZ) SetActorLocation(FVector(At.X, At.Y, FloorZ - Kind->MeshOffset.Z));
}

void AOpenWillowAmbientNpc::ApplyRootEnd()
{
    // The stock perch clips carry root motion (the start clip steps the pawn onto its wall or counter). The mesh already shows it
    // through the root bone while the clip plays; when the clip ends the actor takes the travel over, because the next clip starts
    // its root at zero again.
    const FVector* Travel = Kind->RootEnd.Find(PlayingRole);
    if (!Travel) return;
    SetActorLocation(GetActorLocation() + GetActorRotation().RotateVector(FVector(Travel->X, Travel->Y, 0)));
}

void AOpenWillowAmbientNpc::EaseOntoPerch()
{
    // Script: the pawn is interpolated to the perch's location and rotation over the definition's LerpTime.
    const FOpenWillowAmbientNode& Node = World->Nodes[Target];
    const float Lerp = FMath::Max(World->Perches[Node.Perch].LerpTime, 0.01f);
    const float Alpha = FMath::Clamp(PhaseTime / Lerp, 0.f, 1.f);
    const FVector At = GetActorLocation();
    SetActorLocation(FVector(FMath::Lerp(LerpFrom.X, Node.Location.X, Alpha), FMath::Lerp(LerpFrom.Y, Node.Location.Y, Alpha), At.Z));
    if (Node.bFaceNodeDirection)
        SetActorRotation(FRotator(0, LerpFromYaw + FMath::FindDeltaAngleDegrees(LerpFromYaw, Node.Yaw) * Alpha, 0));
}

void AOpenWillowAmbientNpc::TurnToward(float WantedYaw, float DeltaSeconds)
{
    FRotator Rotation = GetActorRotation();
    const float Step = FMath::FindDeltaAngleDegrees(Rotation.Yaw, WantedYaw);
    Rotation.Yaw += FMath::Clamp(Step, -Kind->YawRate * DeltaSeconds, Kind->YawRate * DeltaSeconds);
    SetActorRotation(FRotator(0, Rotation.Yaw, 0));
}

float AOpenWillowAmbientNpc::LoopSeconds() const
{
    const FOpenWillowAmbientNode& Node = World->Nodes[Target];
    const FOpenWillowAmbientPerch& Perch = World->Perches[Node.Perch];
    const FVector2D Range = Node.LoopOverride.X >= 0 ? Node.LoopOverride : Perch.LoopTime;
    FRandomStream Stream = MakeStream(SpawnData.Id + FString::FromInt(PerchStops) + FString::FromInt(Reached));
    return Stream.FRandRange(Range.X, FMath::Max(Range.X, Range.Y));
}

int32 AOpenWillowAmbientNpc::PickNext() const
{
    const FOpenWillowAmbientNode& Node = World->Nodes[Target];
    float Total = 0;
    for (const auto& Edge : Node.Next) Total += FMath::Max(0.f, Edge.Value);
    if (Total <= 0) return INDEX_NONE;
    FRandomStream Stream = MakeStream(SpawnData.Id + TEXT("next") + FString::FromInt(Reached));
    float Pick = Stream.FRandRange(0.f, Total);
    for (const auto& Edge : Node.Next)
    {
        Pick -= FMath::Max(0.f, Edge.Value);
        if (Pick <= 0) return Edge.Key;
    }
    return Node.Next.Last().Key;
}

void AOpenWillowAmbientNpc::Arrive()
{
    ++Reached;
    const FOpenWillowAmbientNode& Node = World->Nodes[Target];
    if (!Node.Perch.IsEmpty())
    {
        LerpFrom = GetActorLocation();
        LerpFromYaw = GetActorRotation().Yaw;
        const FOpenWillowAmbientPerch& Perch = World->Perches[Node.Perch];
        if (!Perch.Start.IsEmpty() && Clip(Perch.Start))
        {
            State = EPhase::PerchStart;
            PhaseTime = 0;
            PhaseLength = Clip(Perch.Start)->GetPlayLength();
            Play(Perch.Start, false);
            return;
        }
        BeginLoop();
        return;
    }
    AfterPerch();
}

void AOpenWillowAmbientNpc::BeginLoop()
{
    const FOpenWillowAmbientNode& Node = World->Nodes[Target];
    const FOpenWillowAmbientPerch& Perch = World->Perches[Node.Perch];
    State = EPhase::PerchLoop;
    PhaseTime = 0;
    DwellLeft = SpawnData.bWander ? LoopSeconds() : TNumericLimits<float>::Max();
    // One idle clip per cycle, chosen by weight (SpecialMove_PerchRandomLoop). The loop restarts when a cycle ends.
    float Total = 0;
    for (const auto& Entry : Perch.Idle) Total += Entry.Value;
    FRandomStream Stream = MakeStream(SpawnData.Id + TEXT("loop") + FString::FromInt(++Cycles));
    float Pick = Stream.FRandRange(0.f, FMath::Max(Total, 0.0001f));
    FString Chosen = Perch.Idle[0].Key;
    for (const auto& Entry : Perch.Idle)
    {
        Pick -= Entry.Value;
        if (Pick <= 0) { Chosen = Entry.Key; break; }
    }
    UAnimSequence* Sequence = Clip(Chosen);
    PhaseLength = Sequence ? Sequence->GetPlayLength() : 2.f;
    Play(Chosen, false);
}

void AOpenWillowAmbientNpc::AfterPerch()
{
    const int32 Next = SpawnData.bWander ? PickNext() : INDEX_NONE;
    if (Next == INDEX_NONE)
    {
        // Nothing to walk to (or an idle pawn): keep looping the last pose for good.
        if (!World->Nodes[Target].Perch.IsEmpty()) { BeginLoop(); DwellLeft = TNumericLimits<float>::Max(); }
        else { State = EPhase::Holding; Play(TEXT("idle"), true); }
        return;
    }
    Target = Next;
    BeginTravel();
}

void AOpenWillowAmbientNpc::UpdateInk()
{
    // The original's line is a few pixels wide whatever the distance; the hull is a world-space offset, so its thickness follows the
    // camera distance: pixels * distance / focal length in pixels (horizontal FOV, viewport width).
    if (!Outline || !Outline->IsVisible() || !Kind) return;
    APlayerCameraManager* Camera = UGameplayStatics::GetPlayerCameraManager(this, 0);
    FVector2D View(0, 0);
    if (Camera && GEngine && GEngine->GameViewport) GEngine->GameViewport->GetViewportSize(View);
    float Cm = Kind->OutlineCm;
    if (Camera && View.X > 1.f && Kind->OutlinePx > 0.f)
    {
        const float Focal = View.X * 0.5f / FMath::Tan(FMath::DegreesToRadians(FMath::Clamp(Camera->GetFOVAngle(), 20.f, 150.f) * 0.5f));
        Cm = FMath::Clamp(FVector::Dist(Camera->GetCameraLocation(), GetActorLocation() + FVector(0, 0, 90.f)) * Kind->OutlinePx / Focal, 0.1f, 8.f);
    }
    if (FMath::Abs(Cm - InkCm) < 0.03f * FMath::Max(InkCm, 0.1f)) return;
    InkCm = Cm;
    Outline->SetScalarParameterValueOnMaterials(TEXT("ThicknessCm"), Cm);
    for (UStaticMeshComponent* Hull : WornInk) Hull->SetScalarParameterValueOnMaterials(TEXT("ThicknessCm"), Cm);
}

void AOpenWillowAmbientNpc::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    UpdateInk();
    if (!World.IsValid() || Target == INDEX_NONE || !Kind) return;
    const FOpenWillowAmbientNode& Node = World->Nodes[Target];
    switch (State)
    {
    case EPhase::Waiting:
        PhaseTime += DeltaSeconds;
        break;
    case EPhase::Walking:
    {
        const FVector To = Node.Location - GetActorLocation();
        const float Planar = To.Size2D();
        const float Arrival = FMath::Max(Node.ArrivalRadius, 8.f);
        if (Planar <= Arrival)
        {
            Arrive();
            break;
        }
        const float Step = FMath::Min(Kind->Speed * DeltaSeconds, Planar - Arrival + 1.f);
        const FVector Move = FVector(To.X, To.Y, 0) / Planar * Step;
        SetActorLocation(GetActorLocation() + Move);
        Walked += Step;
        TurnToward(To.Rotation().Yaw, DeltaSeconds);
        break;
    }
    case EPhase::PerchStart:
    case EPhase::PerchStop:
        PhaseTime += DeltaSeconds;
        if (State == EPhase::PerchStart) EaseOntoPerch();
        else if (Node.bFaceNodeDirection) TurnToward(Node.Yaw, DeltaSeconds);
        if (PhaseTime >= PhaseLength)
        {
            ApplyRootEnd();
            if (State == EPhase::PerchStart) BeginLoop();
            else AfterPerch();
        }
        break;
    case EPhase::PerchLoop:
    {
        PhaseTime += DeltaSeconds;
        DwellLeft -= DeltaSeconds;
        if (Node.bFaceNodeDirection) TurnToward(Node.Yaw, DeltaSeconds);
        if (DwellLeft <= 0)
        {
            const FOpenWillowAmbientPerch& Perch = World->Perches[Node.Perch];
            ++PerchStops;
            if (!Perch.Stop.IsEmpty() && Clip(Perch.Stop))
            {
                State = EPhase::PerchStop;
                PhaseTime = 0;
                PhaseLength = Clip(Perch.Stop)->GetPlayLength();
                Play(Perch.Stop, false);
            }
            else AfterPerch();
        }
        else if (PhaseTime >= PhaseLength)
        {
            ApplyRootEnd();
            const float Left = DwellLeft;
            BeginLoop();
            DwellLeft = Left;   // keep the dwell that is left; only the clip choice restarts
        }
        break;
    }
    case EPhase::Holding:
        break;
    }
    SnapToFloor(DeltaSeconds, false);
}

// ---------------------------------------------------------------------------------------------------- director
AOpenWillowAmbientDirector::AOpenWillowAmbientDirector()
{
    PrimaryActorTick.bCanEverTick = true;
}

AOpenWillowAmbientDirector* AOpenWillowAmbientDirector::SpawnIfRequested(UWorld* InWorld)
{
    FString File;
    if (!FParse::Value(FCommandLine::Get(), TEXT("owambient="), File) || File.IsEmpty())
        File = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_AMBIENT"));
    if (!InWorld || File.IsEmpty()) return nullptr;
    FActorSpawnParameters Params;
    Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
    AOpenWillowAmbientDirector* Director = InWorld->SpawnActor<AOpenWillowAmbientDirector>(FVector::ZeroVector, FRotator::ZeroRotator, Params);
    if (Director) Director->Start(File);
    return Director;
}

bool AOpenWillowAmbientDirector::Start(const FString& File)
{
    bTest = FParse::Param(FCommandLine::Get(), TEXT("owambienttest"));
    bShots = FParse::Param(FCommandLine::Get(), TEXT("owambientshots"));
    FParse::Value(FCommandLine::Get(), TEXT("owambientseconds="), TestSeconds);
    try
    {
        World = MakeShared<FOpenWillowAmbientWorld>();
        World->Load(File);
    }
    catch (const std::exception& Error)
    {
        UE_LOG(LogTemp, Error, TEXT("OWAMBIENT ERROR %s"), UTF8_TO_TCHAR(Error.what()));
        World.Reset();
        return false;
    }
    int32 Failed = 0;
    TSharedPtr<const FOpenWillowAmbientWorld> Shared = World;
    for (const FOpenWillowAmbientSpawn& Spawn : World->Spawns)
    {
        FActorSpawnParameters Params;
        Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
        AOpenWillowAmbientNpc* Npc = GetWorld()->SpawnActor<AOpenWillowAmbientNpc>(Spawn.Location, FRotator(0, Spawn.Yaw, 0), Params);
        if (!Npc || !Npc->Setup(Shared, Spawn))
        {
            ++Failed;
            UE_LOG(LogTemp, Warning, TEXT("OWAMBIENT %s (%s) could not be spawned with the imported assets"), *Spawn.Id, *Spawn.Kind);
            if (Npc) Npc->Destroy();
            continue;
        }
        Pawns.Add(Npc);
    }
    TMap<FString, int32> PerKind;
    for (const AOpenWillowAmbientNpc* Npc : Pawns) ++PerKind.FindOrAdd(Npc->KindName());
    FString Kinds;
    for (const auto& Pair : PerKind) Kinds += FString::Printf(TEXT(" %s=%d"), *Pair.Key, Pair.Value);
    UE_LOG(LogTemp, Display, TEXT("OWAMBIENT READY spawned=%d failed=%d nodes=%d perches=%d kinds:%s (host stand-in movement, UNVERIFIED)"),
        Pawns.Num(), Failed, World->Nodes.Num(), World->Perches.Num(), *Kinds);
    return true;
}

void AOpenWillowAmbientDirector::Summary()
{
    if (bSummaryDone) return;
    bSummaryDone = true;
    int32 Moved = 0, Reaching = 0, Perched = 0;
    for (const AOpenWillowAmbientNpc* Npc : Pawns)
    {
        const bool bWalker = Npc->DistanceWalked() > 50.f;
        if (bWalker) ++Moved;
        if (Npc->NodesReached() > 0 || Npc->IsHeld()) ++Reaching;   // held pawns (observed standing, no node) have no node to reach
        if (Npc->PerchCycles() > 0 || Npc->Phase().StartsWith(TEXT("perch"))) ++Perched;
        UE_LOG(LogTemp, Display, TEXT("OWAMBIENT pawn %s kind=%s phase=%s nodes=%d perch_stops=%d walked=%.0f at %s"), *Npc->Label(), *Npc->KindName(),
            *Npc->Phase(), Npc->NodesReached(), Npc->PerchCycles(), Npc->DistanceWalked(), *Npc->GetActorLocation().ToString());
    }
    const bool bOk = Pawns.Num() > 0 && Reaching == Pawns.Num() && Moved > 0;
    UE_LOG(LogTemp, Display, TEXT("OWAMBIENT SUMMARY result=%s pawns=%d reached_a_node=%d walked_50cm=%d at_perch=%d seconds=%.0f"),
        bOk ? TEXT("PASS") : TEXT("FAIL"), Pawns.Num(), Reaching, Moved, Perched, Age);
    if (bTest) FPlatformMisc::RequestExit(false);
}

bool AOpenWillowAmbientDirector::ChooseCamera(const AOpenWillowAmbientNpc* Npc, FVector& OutCamera, FString& OutNote, bool bSide) const
{
    // A camera spot 220-460 cm from the pawn, in front of it if that is free, else round it at 45 degree steps: the first
    // spot a 35 cm sphere can sweep to from the pawn's chest without touching the level (perches face walls). The same
    // spot is used in the real game for the matched frame (both maps share coordinates).
    const FVector Chest = Npc->GetActorLocation() + FVector(0, 0, 30.f);
    const float BaseYaw = Npc->GetActorRotation().Yaw;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowAmbientCamera), true, Npc);
    // A walker is seen from its side (in front, it would walk into the camera).
    const TArray<float> Turns = bSide ? TArray<float>{90.f, -90.f, 135.f, -135.f, 45.f, -45.f, 180.f, 0.f} : TArray<float>{0.f, 45.f, -45.f, 90.f, -90.f, 135.f, -135.f, 180.f};
    for (const float Distance : {bSide ? 330.f : 220.f, 340.f, 460.f})
        for (const float Turn : Turns)
        {
            const FVector Dir = FRotator(0, BaseYaw + Turn, 0).Vector();
            const FVector Spot = Chest + Dir * Distance + FVector(0, 0, 40.f);
            FHitResult Hit;
            // The player must have floor under the spot, or she falls out of the level.
            FHitResult Floor;
            const bool bFloor = GetWorld()->LineTraceSingleByChannel(Floor, Spot, Spot - FVector(0, 0, 260.f), ECC_Visibility, Query);
            // Another pawn standing on the camera's line of sight to this one would overlap it in the frame (round-1 stop 1: the
            // neighbour at the next perch stood right behind the subject).
            bool bBlockedByPawn = false;
            for (const AOpenWillowAmbientNpc* Other : Pawns)
            {
                if (Other == Npc) continue;
                if (FMath::PointDistToSegment(Other->GetActorLocation(), Spot, Chest) < 130.f) { bBlockedByPawn = true; break; }
            }
            if (!bBlockedByPawn && bFloor && !GetWorld()->SweepSingleByChannel(Hit, Chest, Spot, FQuat::Identity, ECC_Visibility, FCollisionShape::MakeSphere(35.f), Query))
            {
                OutCamera = Spot;
                OutNote = FString::Printf(TEXT("distance %.0f turn %.0f"), Distance, Turn);
                return true;
            }
        }
    return false;
}

void AOpenWillowAmbientDirector::RunShots(float DeltaSeconds)
{
    // Screenshot tour (capture runs only): visit the manifest's showcase entries. An entry is a spawn id, or "@walker" for
    // whichever walking pawn is found when its turn comes. The player is put at a traced free camera spot and aimed at the
    // pawn (a walker is followed: the spot is chosen again every 2 s); frames are taken while the pawns keep going.
    // Needs the walker (-owwalk).
    AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(UGameplayStatics::GetPlayerPawn(this, 0));
    if (!Walker || !World.IsValid() || World->Showcase.Num() == 0) return;
    constexpr float ViewSeconds = 12.f, ShotEvery = 2.5f, Settle = 2.5f;
    constexpr int32 ShotsPerView = 3;
    ShotClock += DeltaSeconds;
    // A "@walker" stop gets 30 s: walkers pause at perches, and only one or two are loose in the observed population.
    int32 View = 0;
    float ViewStart = 0;
    for (; View < World->Showcase.Num(); ++View)
    {
        const float Length = World->Showcase[View] == TEXT("@walker") ? 30.f : ViewSeconds;
        if (ShotClock < ViewStart + Length) break;
        ViewStart += Length;
    }
    if (View >= World->Showcase.Num())
    {
        Summary();
        FPlatformMisc::RequestExit(false);
        return;
    }
    const float InView = ShotClock - ViewStart;
    bool bNewView = false;
    if (View != ShotStep)
    {
        ShotStep = View;
        bNewView = true;
        Current.Reset();
        bSkipView = false;
    }
    if (!Current.IsValid())
    {
        // Resolve the stop's pawn; a "@walker" stop keeps looking each tick until some walking pawn turns up (walkers pause at perches).
        const FString& Id = World->Showcase[View];
        for (AOpenWillowAmbientNpc* Npc : Pawns)
        {
            if (Id == TEXT("@walker") ? (Npc->Phase() == TEXT("walking") && !Npc->IsHeld() && Npc != PreviousWalker.Get()) : Npc->Label() == Id)
            {
                Current = Npc;
                ResolvedAt = InView;
                bNewView = true;           // place the camera now
                break;
            }
        }
        if (Id == TEXT("@walker") && Current.IsValid()) PreviousWalker = Current;
    }
    if (!Current.IsValid())
    {
        if (bNewView) UE_LOG(LogTemp, Warning, TEXT("OWAMBIENT view %d %s: no such pawn (yet)"), View, *World->Showcase[View]);
        return;
    }
    AOpenWillowAmbientNpc* Npc = Current.Get();
    const bool bFollow = World->Showcase[View] == TEXT("@walker");
    if (bNewView || (bFollow && FMath::FloorToInt(InView / 2.f) != FMath::FloorToInt((InView - DeltaSeconds) / 2.f)))
    {
        FVector Camera;
        FString Note;
        const bool bPlaced = ChooseCamera(Npc, Camera, Note, bFollow);
        if (!bPlaced && bNewView)
        {
            // No spot with floor and a clear line (a pawn at the edge of the map): leave this stop out rather than shoot a wall.
            UE_LOG(LogTemp, Warning, TEXT("OWAMBIENT view %d %s skipped: no free camera spot with floor"), View, *Npc->Label());
            bSkipView = true;
            return;
        }
        if (bPlaced)
        {
            bSkipView = false;
            Walker->SetActorLocation(Camera, false, nullptr, ETeleportType::TeleportPhysics);
            Walker->GetCharacterMovement()->StopMovementImmediately();
            const FVector P = Npc->GetActorLocation();
            UE_LOG(LogTemp, Display, TEXT("OWAMBIENT view %d %s kind=%s phase=%s pawn %.0f %.0f %.0f yaw %.0f camera %.0f %.0f %.0f (%s)"), View, *Npc->Label(),
                *Npc->KindName(), *Npc->Phase(), P.X, P.Y, P.Z, Npc->GetActorRotation().Yaw, Camera.X, Camera.Y, Camera.Z, *Note);
        }
    }
    if (bSkipView) return;
    const FVector Aim = Npc->GetActorLocation() + FVector(0, 0, 25.f);
    if (Walker->GetController()) Walker->GetController()->SetControlRotation((Aim - (Walker->GetActorLocation() + FVector(0, 0, 70))).Rotation());
    const int32 Shot = int32((InView - ResolvedAt - Settle) / ShotEvery);
    if (InView - ResolvedAt >= Settle && Shot >= 0 && Shot < ShotsPerView && (View * 8 + Shot) != LastShot)
    {
        LastShot = View * 8 + Shot;
        FScreenshotRequest::RequestScreenshot(FString::Printf(TEXT("OWAmbient_%02d_%s_%d.png"), View, *Npc->Label().Replace(TEXT("/"), TEXT("-")), Shot), false, false);
        UE_LOG(LogTemp, Display, TEXT("OWAMBIENT screenshot view %d shot %d phase=%s pawn %s player %s"), View, Shot, *Npc->Phase(),
            *Npc->GetActorLocation().ToString(), *Walker->GetActorLocation().ToString());
    }
}

void AOpenWillowAmbientDirector::Balance(float DeltaSeconds)
{
    // NPCLoadBalancer stand-in. Native, read by lane E (docs/verification/NATIVE_AMBIENT_NPC.md, UNVERIFIED in game): at
    // most 7 load-balanced pawns path at once; every 0.5 s the arbiter admits ONE waiting pawn, the one with the largest
    // (seconds waited - (distance to the nearest player / 1000)^2). Pawns the population marks as not throttled
    // (Flag_NPCDoNotThrottleMovement) are not load-balanced and never wait.
    constexpr int32 MaxPathing = 7;
    BalanceClock += DeltaSeconds;
    if (BalanceClock < 0.5f) return;
    BalanceClock = 0;
    int32 Pathing = 0;
    for (const AOpenWillowAmbientNpc* Npc : Pawns) Pathing += Npc->IsPathing() ? 1 : 0;
    if (Pathing >= MaxPathing) return;
    APawn* Player = UGameplayStatics::GetPlayerPawn(this, 0);
    AOpenWillowAmbientNpc* Best = nullptr;
    float BestScore = -TNumericLimits<float>::Max();
    for (AOpenWillowAmbientNpc* Npc : Pawns)
    {
        if (!Npc->IsWaiting()) continue;
        const float Distance = Player ? FVector::Dist(Player->GetActorLocation(), Npc->GetActorLocation()) : 0.f;
        const float Score = Npc->WaitedSeconds() - FMath::Square(Distance / 1000.f);
        if (Score > BestScore) { BestScore = Score; Best = Npc; }
    }
    if (Best) Best->AdmitToPath();
}

void AOpenWillowAmbientDirector::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    Age += DeltaSeconds;
    Balance(DeltaSeconds);
    if (bShots) RunShots(DeltaSeconds);
    else if (bTest && Age >= TestSeconds) Summary();
}
