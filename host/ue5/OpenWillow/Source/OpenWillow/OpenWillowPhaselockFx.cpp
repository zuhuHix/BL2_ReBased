#include "OpenWillowPhaselockFx.h"
#include "Camera/PlayerCameraManager.h"
#include "Components/StaticMeshComponent.h"
#include "Dom/JsonObject.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "Materials/MaterialInterface.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

namespace
{
// Host assets written by host/ue5/import_phaselock_fx.py: one material instance per stock material, the stock meshes,
// and the engine plane used as the sprite quad (100 x 100 uu, normal +Z).
const TCHAR* const MaterialFolder = TEXT("/Game/OpenWillow/Phaselock/Materials");
const TCHAR* const MeshFolder = TEXT("/Game/OpenWillow/Phaselock/Meshes");
const TCHAR* const SpriteQuad = TEXT("/Engine/BasicShapes/Plane.Plane");
constexpr float QuadSize = 100.f;
// Screen particle quad distance in front of the camera (host stand-in; beyond the 10 uu near plane).
constexpr float ScreenQuadDistance = 15.f;

FString ShortName(const FString& Path)
{
    int32 Dot = INDEX_NONE;
    return Path.FindLastChar(TEXT('.'), Dot) ? Path.Mid(Dot + 1) : Path;
}

bool ReadVector(const TSharedPtr<FJsonValue>& Value, FVector& Out)
{
    double N = 0;
    if (!Value) return false;
    if (Value->TryGetNumber(N)) { Out = FVector(N); return true; }
    const TArray<TSharedPtr<FJsonValue>>* Items = nullptr;
    if (!Value->TryGetArray(Items) || Items->Num() == 0) return false;
    for (int32 I = 0; I < 3; ++I) Out[I] = Items->IsValidIndex(I) ? (*Items)[I]->AsNumber() : 0.0;
    return true;
}

bool JsonBool(const TSharedPtr<FJsonObject>& Object, const TCHAR* Key)
{
    bool b = false;
    return Object->TryGetBoolField(Key, b) && b;
}

double JsonNumber(const TSharedPtr<FJsonObject>& Object, const TCHAR* Key, double Default)
{
    double N = Default;
    return Object->TryGetNumberField(Key, N) ? N : Default;
}

FOwFxDistribution Field(const TSharedPtr<FJsonObject>& Object, const TCHAR* Key)
{
    const TSharedPtr<FJsonObject>* Child = nullptr;
    return Object->TryGetObjectField(Key, Child) ? FOwFxDistribution::Parse(*Child) : FOwFxDistribution();
}

TMap<FString, TUniquePtr<FOwFxTemplate>>& TemplateCache()
{
    static TMap<FString, TUniquePtr<FOwFxTemplate>> Cache;
    return Cache;
}
}

FOwFxDistribution FOwFxDistribution::Parse(const TSharedPtr<FJsonObject>& Object)
{
    FOwFxDistribution D;
    if (!Object) return D;
    const TSharedPtr<FJsonObject>* Parameter = nullptr;
    if (Object->TryGetObjectField(TEXT("parameter"), Parameter))
    {
        D.Kind = EKind::Parameter;
        D.Parameter = FName(*(*Parameter)->GetStringField(TEXT("name")));
        const auto& In = (*Parameter)->GetArrayField(TEXT("input"));
        const auto& Out = (*Parameter)->GetArrayField(TEXT("output"));
        if (In.Num() == 2) D.In = FVector2f(float(In[0]->AsNumber()), float(In[1]->AsNumber()));
        if (Out.Num() == 2) D.Out = FVector2f(float(Out[0]->AsNumber()), float(Out[1]->AsNumber()));
        D.ParameterConstant = float(JsonNumber(*Parameter, TEXT("constant"), 0.0));
        return D;
    }
    FString Kind;
    if (!Object->TryGetStringField(TEXT("kind"), Kind)) return D;
    if (Kind == TEXT("constant"))
    {
        if (ReadVector(Object->TryGetField(TEXT("value")), D.Low)) { D.High = D.Low; D.Kind = EKind::Constant; }
    }
    else if (Kind == TEXT("pair"))
    {
        const TSharedPtr<FJsonObject>* Value = nullptr;
        if (Object->TryGetObjectField(TEXT("value"), Value) && ReadVector((*Value)->TryGetField(TEXT("low")), D.Low)
            && ReadVector((*Value)->TryGetField(TEXT("high")), D.High))
            D.Kind = EKind::Pair;
    }
    else if (Kind == TEXT("curve"))
    {
        for (const auto& Sample : Object->GetArrayField(TEXT("samples")))
        {
            const auto& Pair = Sample->AsArray();
            FVector V;
            if (Pair.Num() != 2 || !ReadVector(Pair[1], V)) continue;
            D.Times.Add(float(Pair[0]->AsNumber()));
            D.Values.Add(V);
        }
        if (D.Times.Num()) D.Kind = EKind::Curve;
    }
    return D;
}

FVector FOwFxDistribution::Sample(float T, FRandomStream& Random, const TMap<FName, float>& Parameters) const
{
    switch (Kind)
    {
    case EKind::Constant: return Low;
    case EKind::Pair:
        return FVector(FMath::Lerp(Low.X, High.X, Random.FRand()), FMath::Lerp(Low.Y, High.Y, Random.FRand()),
            FMath::Lerp(Low.Z, High.Z, Random.FRand()));
    case EKind::Curve:
    {
        if (T <= Times[0]) return Values[0];
        for (int32 I = 1; I < Times.Num(); ++I)
            if (T < Times[I])
            {
                const float Span = Times[I] - Times[I - 1];
                return FMath::Lerp(Values[I - 1], Values[I], Span > 0.f ? (T - Times[I - 1]) / Span : 1.f);
            }
        return Values.Last();
    }
    case EKind::Parameter:
    {
        const float* Value = Parameters.Find(Parameter);
        if (!Value) return FVector(ParameterConstant);
        const float Range = In.Y - In.X;
        const float A = Range != 0.f ? FMath::Clamp((*Value - In.X) / Range, 0.f, 1.f) : 0.f;
        return FVector(FMath::Lerp(Out.X, Out.Y, A));
    }
    default: return FVector::ZeroVector;
    }
}

const FOwFxTemplate* FOwFxTemplate::Load(const FString& Dir, const FString& Name, FString& OutError)
{
    const FString File = FPaths::Combine(Dir, Name + TEXT(".json"));
    if (const TUniquePtr<FOwFxTemplate>* Cached = TemplateCache().Find(File)) return Cached->Get();
    FString Text;
    TSharedPtr<FJsonObject> Root;
    if (!FFileHelper::LoadFileToString(Text, *File) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root)
    {
        OutError = TEXT("cannot read decoded template ") + File + TEXT(" (research/particle_system.py)");
        return nullptr;
    }
    auto Template = MakeUnique<FOwFxTemplate>();
    Template->Name = Name;
    for (const auto& Value : Root->GetArrayField(TEXT("digest")))
    {
        const auto Row = Value->AsObject();
        bool bEnabled = true;
        if (Row->TryGetBoolField(TEXT("enabled"), bEnabled) && !bEnabled) continue;
        FOwFxEmitter E;
        E.Name = Row->GetStringField(TEXT("name"));
        const auto Required = Row->GetObjectField(TEXT("required"));
        E.Material = ShortName(Required->GetStringField(TEXT("Material")));
        E.bRectangle = Required->GetStringField(TEXT("ScreenAlignment")).StartsWith(TEXT("PSA_Rectangle"));
        E.bLocalSpace = JsonBool(Required, TEXT("bUseLocalSpace"));
        E.bKillOnDeactivate = JsonBool(Required, TEXT("bKillOnDeactivate"));
        E.Duration = float(JsonNumber(Required, TEXT("EmitterDuration"), 1.0));
        E.Delay = float(JsonNumber(Required, TEXT("EmitterDelay"), 0.0));
        E.Loops = int32(JsonNumber(Required, TEXT("EmitterLoops"), 1.0));
        E.SubH = FMath::Max(1, int32(JsonNumber(Required, TEXT("SubImages_Horizontal"), 1.0)));
        E.SubV = FMath::Max(1, int32(JsonNumber(Required, TEXT("SubImages_Vertical"), 1.0)));
        const FString Interp = Required->GetStringField(TEXT("InterpolationMethod"));
        E.bSubRandom = Interp.StartsWith(TEXT("PSUVIM_Random"));
        E.bSubLinear = Interp.StartsWith(TEXT("PSUVIM_Linear"));
        const auto Spawn = Row->GetObjectField(TEXT("spawn"));
        if (JsonBool(Spawn, TEXT("bProcessSpawnRate")))
        {
            FRandomStream Fixed(0);
            const TMap<FName, float> None;
            E.SpawnRate = Field(Spawn, TEXT("Rate")).SampleFloat(0, Fixed, None)
                * (Field(Spawn, TEXT("RateScale")).IsSet() ? Field(Spawn, TEXT("RateScale")).SampleFloat(0, Fixed, None) : 1.f)
                + Field(Required, TEXT("SpawnRate")).SampleFloat(0, Fixed, None);
        }
        const TArray<TSharedPtr<FJsonValue>>* Bursts = nullptr;
        if (JsonBool(Spawn, TEXT("bProcessBurstList")) && Spawn->TryGetArrayField(TEXT("BurstList"), Bursts))
            for (const auto& B : *Bursts)
            {
                const auto Burst = B->AsObject();
                E.Bursts.Add({float(JsonNumber(Burst, TEXT("Time"), 0.0)), int32(JsonNumber(Burst, TEXT("Count"), 0.0)),
                    int32(JsonNumber(Burst, TEXT("CountLow"), -1.0))});
            }
        E.Bursts.Sort([](const FOwFxEmitter::FBurst& A, const FOwFxEmitter::FBurst& B) { return A.Time < B.Time; });
        const TSharedPtr<FJsonObject>* TypeData = nullptr;
        if (Row->TryGetObjectField(TEXT("type_data"), TypeData))
        {
            const TSharedPtr<FJsonObject>* Properties = nullptr;
            FString Mesh;
            if ((*TypeData)->TryGetObjectField(TEXT("properties"), Properties) && (*Properties)->TryGetStringField(TEXT("Mesh"), Mesh))
                E.Mesh = ShortName(Mesh);
        }
        for (const auto& ModuleValue : Row->GetArrayField(TEXT("modules")))
        {
            const auto M = ModuleValue->AsObject();
            bool bModuleEnabled = true;
            if (M->TryGetBoolField(TEXT("bEnabled"), bModuleEnabled) && !bModuleEnabled) continue;
            const FString Class = M->GetStringField(TEXT("class"));
            if (Class == TEXT("Lifetime")) E.Lifetime = Field(M, TEXT("Lifetime"));
            else if (Class == TEXT("Size")) E.StartSize = Field(M, TEXT("StartSize"));
            else if (Class == TEXT("Color")) { E.StartColor = Field(M, TEXT("StartColor")); E.StartAlpha = Field(M, TEXT("StartAlpha")); }
            else if (Class == TEXT("ColorOverLife")) { E.ColorOverLife = Field(M, TEXT("ColorOverLife")); E.AlphaOverLife = Field(M, TEXT("AlphaOverLife")); }
            else if (Class == TEXT("ColorScaleOverLife")) { E.ColorScale = Field(M, TEXT("ColorScaleOverLife")); E.AlphaScale = Field(M, TEXT("AlphaScaleOverLife")); }
            else if (Class == TEXT("SizeMultiplyLife"))
            {
                E.SizeMultiply = Field(M, TEXT("LifeMultiplier"));
                E.SizeMultiplyAxes = FVector(JsonBool(M, TEXT("MultiplyX")), JsonBool(M, TEXT("MultiplyY")), JsonBool(M, TEXT("MultiplyZ")));
            }
            else if (Class == TEXT("Rotation")) E.StartRotation = Field(M, TEXT("StartRotation"));
            else if (Class == TEXT("RotationRate")) E.RotationRate = Field(M, TEXT("StartRotationRate"));
            else if (Class == TEXT("RotationRateMultiplyLife")) E.RotationRateMultiply = Field(M, TEXT("LifeMultiplier"));
            else if (Class == TEXT("MeshRotation")) E.MeshRotation = Field(M, TEXT("StartRotation"));
            else if (Class == TEXT("Velocity")) E.StartVelocities.Add(Field(M, TEXT("StartVelocity")));
            else if (Class == TEXT("VelocityOverLifetime")) E.VelocityOverLife = Field(M, TEXT("VelOverLife"));
            else if (Class == TEXT("Location")) E.StartLocation = Field(M, TEXT("StartLocation"));
            else if (Class == TEXT("ParameterDynamic"))
            {
                const TArray<TSharedPtr<FJsonValue>>* Params = nullptr;
                if (M->TryGetArrayField(TEXT("DynamicParams"), Params) && Params->Num() > 0)
                    E.DynamicParam0 = Field((*Params)[0]->AsObject(), TEXT("ParamValue"));
            }
            else if (Class == TEXT("LocationPrimitiveSphere"))
            {
                E.SphereRadius = Field(M, TEXT("StartRadius"));
                E.SphereVelocityScale = Field(M, TEXT("VelocityScale"));
                E.bSphereSurface = JsonBool(M, TEXT("SurfaceOnly"));
                E.bSphereVelocity = JsonBool(M, TEXT("Velocity"));
                if (!E.StartLocation.IsSet()) E.StartLocation = Field(M, TEXT("StartLocation"));
            }
            else if (Class == TEXT("Orbit"))
            {
                E.OrbitOffset = Field(M, TEXT("OffsetAmount"));
                E.OrbitRotation = Field(M, TEXT("RotationAmount"));
                E.OrbitRotationRate = Field(M, TEXT("RotationRateAmount"));
            }
            else if (Class != TEXT("SubUV")) E.Unsupported.Add(Class);
        }
        Template->Emitters.Add(MoveTemp(E));
    }
    const FOwFxTemplate* Result = Template.Get();
    TemplateCache().Add(File, MoveTemp(Template));
    return Result;
}

bool FOpenWillowPhaselockFxData::Load(const FString& File, FString& OutError)
{
    *this = FOpenWillowPhaselockFxData();
    FString Text;
    TSharedPtr<FJsonObject> Root;
    if (!FFileHelper::LoadFileToString(Text, *File) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root
        || Root->GetStringField(TEXT("format")) != TEXT("openwillow.phaselock_fx/1"))
    {
        OutError = TEXT("no openwillow.phaselock_fx/1 manifest (tools/prepare_phaselock_fx.py): ") + File;
        return false;
    }
    const auto Vec = [](const TSharedPtr<FJsonObject>& O, const TCHAR* Key)
    {
        FVector V = FVector::ZeroVector;
        if (const TSharedPtr<FJsonValue> F = O->TryGetField(Key)) ReadVector(F, V);
        return V;
    };
    // The emitter directory is stored relative to the repository root (the manifest's own parent's parent).
    EmitterDir = Root->GetStringField(TEXT("emitterDir"));
    if (FPaths::IsRelative(EmitterDir))
        EmitterDir = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::GetPath(File), TEXT("../.."), EmitterDir));
    const auto Hand = Root->GetObjectField(TEXT("handFx"));
    const auto Socket = Hand->GetObjectField(TEXT("socket"));
    HandBone = FName(*Socket->GetStringField(TEXT("bone")));
    HandSocketLocation = Vec(Socket, TEXT("location"));
    const auto Rot = Socket->GetObjectField(TEXT("rotationDegrees"));
    HandSocketRotation = FRotator(float(JsonNumber(Rot, TEXT("pitch"), 0)), float(JsonNumber(Rot, TEXT("yaw"), 0)), float(JsonNumber(Rot, TEXT("roll"), 0)));
    HandTranslation = Vec(Hand, TEXT("translation"));
    HandScale = float(JsonNumber(Hand, TEXT("scale"), 1.0));
    HandHitTemplate = Hand->GetStringField(TEXT("hitTemplate"));
    HandMissTemplate = Hand->GetStringField(TEXT("missTemplate"));
    const auto Clips = Hand->GetObjectField(TEXT("clips"));
    auto NotifyTime = [&Clips](const TCHAR* Clip)
    {
        const TSharedPtr<FJsonObject>* C = nullptr;
        if (!Clips->TryGetObjectField(Clip, C)) return -1.f;
        for (const auto& N : (*C)->GetArrayField(TEXT("notifies")))
            for (const auto& Event : N->AsObject()->GetArrayField(TEXT("events")))
                if (Event->AsString() == TEXT("PlayPhaselockHandFXFirstPerson")) return float(N->AsObject()->GetNumberField(TEXT("time")));
        return -1.f;
    };
    LiftNotifyTime = NotifyTime(TEXT("Phase_Lock_Lift"));
    FailNotifyTime = NotifyTime(TEXT("Phase_Lock_Fail"));
    const auto Bubble = Root->GetObjectField(TEXT("bubble"));
    BubbleScaleDivisor = float(JsonNumber(Bubble, TEXT("drawScaleDivisor"), 0));
    BubbleIntroTime = float(JsonNumber(Bubble, TEXT("introTime"), 0));
    BubbleOutroOverlap = float(JsonNumber(Bubble, TEXT("outroOverlapTime"), 0));
    CollapseDuration = float(JsonNumber(Bubble, TEXT("collapseDuration"), 0));
    MaxCollapse = float(JsonNumber(Bubble, TEXT("maxCollapseValue"), 0));
    LifeTimeParam = FName(*Bubble->GetStringField(TEXT("lifeTimeParam")));
    CollapseParam = FName(*Bubble->GetStringField(TEXT("collapseParam")));
    BubbleFadeIn = Bubble->GetStringField(TEXT("fadeInTemplate"));
    BubbleLoop = Bubble->GetStringField(TEXT("loopTemplate"));
    BubbleFadeOut = Bubble->GetStringField(TEXT("fadeOutTemplate"));
    const auto Light = Root->GetObjectField(TEXT("light"));
    LightRadius = float(JsonNumber(Light, TEXT("Radius"), 0));
    LightBrightness = float(JsonNumber(Light, TEXT("Brightness"), 0));
    LightFalloffExponent = float(JsonNumber(Light, TEXT("FalloffExponent"), 0));
    const auto Colour = Light->GetObjectField(TEXT("LightColor"));
    LightColor = FColor(uint8(JsonNumber(Colour, TEXT("R"), 0)), uint8(JsonNumber(Colour, TEXT("G"), 0)), uint8(JsonNumber(Colour, TEXT("B"), 0)));
    bLightShadows = JsonBool(Light, TEXT("CastShadows"));
    const auto Glow = Root->GetObjectField(TEXT("tattooGlow"));
    GlowDuration = float(JsonNumber(Glow, TEXT("duration"), 0));
    const auto& Scalars = Glow->GetArrayField(TEXT("scalars"));
    if (Scalars.Num() == 1)
        for (const auto& P : Scalars[0]->AsObject()->GetArrayField(TEXT("points")))
            GlowPoints.Add(FVector2f(float(P->AsObject()->GetNumberField(TEXT("time"))), float(P->AsObject()->GetNumberField(TEXT("value")))));
    const auto Colours = Glow->GetObjectField(TEXT("materialColours"));
    if (Colours->Values.Num() == 1)
    {
        const auto C = Colours->Values.CreateConstIterator().Value()->AsObject();
        GlowColor = FLinearColor(float(JsonNumber(C, TEXT("R"), 0)), float(JsonNumber(C, TEXT("G"), 0)), float(JsonNumber(C, TEXT("B"), 0)), 1.f);
    }
    const auto Screen = Root->GetObjectField(TEXT("screenEffect"));
    const auto& Shown = Screen->GetArrayField(TEXT("show"));
    if (Shown.Num() == 1) ScreenTemplate = Shown[0]->AsObject()->GetStringField(TEXT("template"));
    if (HandBone.IsNone() || LiftNotifyTime < 0.f || FailNotifyTime < 0.f || BubbleScaleDivisor <= 0.f || CollapseDuration <= 0.f
        || GlowPoints.Num() < 2 || GlowDuration <= 0.f || ScreenTemplate.IsEmpty() || LightRadius <= 0.f)
    {
        OutError = TEXT("phaselock FX manifest lacks a hand notify, bubble scale, glow curve, screen template or light");
        return false;
    }
    bLoaded = true;
    return true;
}

float FOpenWillowPhaselockFxData::EvalStored(const TArray<FVector2f>& Points, float T)
{
    if (Points.Num() == 0) return 0.f;
    if (Points.Num() < 2 || T <= Points[0].X) return Points[0].Y;
    if (T >= Points.Last().X) return Points.Last().Y;
    for (int32 I = 1; I < Points.Num(); ++I)
        if (T < Points[I].X)
        {
            const float Span = Points[I].X - Points[I - 1].X;
            return Span > 0.f ? FMath::Lerp(Points[I - 1].Y, Points[I].Y, (T - Points[I - 1].X) / Span) : Points[I].Y;
        }
    return Points.Last().Y;
}

UOpenWillowFxComponent::UOpenWillowFxComponent()
{
    PrimaryComponentTick.bCanEverTick = true;
    PrimaryComponentTick.TickGroup = TG_PostUpdateWork;   // after the camera and the arms pose for this frame
    bAutoActivate = true;
}

void UOpenWillowFxComponent::Play(const FOwFxTemplate* InTemplate, float DrawScale)
{
    Template = InTemplate;
    Scale = DrawScale;
    Random.Initialize(int32(GetUniqueID()));
    States.Reset();
    bPlaying = Template != nullptr;
    bFinished = false;
    Skipped = 0;
    if (!Template) return;
    for (const FOwFxEmitter& E : Template->Emitters)
    {
        FEmitterState& S = States.AddDefaulted_GetRef();
        S.Data = &E;
        S.Time = -E.Delay;
        const FString MaterialPath = FString::Printf(TEXT("%s/MI_%s.MI_%s"), MaterialFolder, *E.Material, *E.Material);
        S.Material = LoadObject<UMaterialInterface>(nullptr, *MaterialPath, nullptr, LOAD_NoWarn | LOAD_Quiet);
        S.Mesh = E.Mesh.IsEmpty() ? LoadObject<UStaticMesh>(nullptr, SpriteQuad)
            : LoadObject<UStaticMesh>(nullptr, *FString::Printf(TEXT("%s/%s.%s"), MeshFolder, *E.Mesh, *E.Mesh), nullptr, LOAD_NoWarn | LOAD_Quiet);
        if (!S.Material || !S.Mesh)
        {
            // Not imported (a distortion material the host does not draw, or an asset UModel did not export).
            ++Skipped;
            S.bSpawning = false;
            UE_LOG(LogTemp, Display, TEXT("OpenWillow FX %s: emitter %s skipped (%s not imported)"), *Template->Name, *E.Name,
                !S.Material ? *(TEXT("material ") + E.Material) : *(TEXT("mesh ") + E.Mesh));
            continue;
        }
        Keep.Add(S.Material);
        Keep.Add(S.Mesh);
        if (E.Unsupported.Num())
            UE_LOG(LogTemp, Display, TEXT("OpenWillow FX %s: emitter %s modules not played: %s"), *Template->Name, *E.Name,
                *FString::Join(E.Unsupported, TEXT(", ")));
    }
}

void UOpenWillowFxComponent::StopEmitting()
{
    for (FEmitterState& S : States)
    {
        S.bSpawning = false;
        if (S.Data && S.Data->bKillOnDeactivate) S.Particles.Reset();
    }
}

FString UOpenWillowFxComponent::Describe() const
{
    FString Out = Template ? Template->Name : FString(TEXT("-"));
    for (const FEmitterState& S : States)
    {
        Out += FString::Printf(TEXT(" | %s %d"), *S.Data->Name, S.Particles.Num());
        if (!S.Material || !S.Mesh) Out += TEXT(" (not drawn)");
        else if (S.Particles.Num())
        {
            const FParticle& P = S.Particles[0];
            Out += FString::Printf(TEXT(" (size %.0fx%.0f colour %.2f %.2f %.2f alpha %.2f age %.2f/%.2f dyn %.2f)"), P.Size.X * Scale,
                P.Size.Y * Scale, P.Color.X, P.Color.Y, P.Color.Z, P.Alpha, P.Age, P.Life, P.Dynamic);
        }
    }
    return Out;
}

int32 UOpenWillowFxComponent::LiveParticles() const
{
    int32 N = 0;
    for (const FEmitterState& S : States) N += S.Particles.Num();
    return N;
}

void UOpenWillowFxComponent::Spawn(FEmitterState& S, int32 Count)
{
    const FOwFxEmitter& E = *S.Data;
    const FTransform Frame = GetComponentTransform();
    for (int32 I = 0; I < Count; ++I)
    {
        FParticle P;
        P.Life = E.Lifetime.IsSet() ? E.Lifetime.SampleFloat(0, Random, Parameters) : 1.f;
        if (P.Life <= 0.f) P.Life = 1e6f;   // a zero lifetime never expires (UE3 convention, UNVERIFIED)
        P.BaseSize = E.StartSize.IsSet() ? E.StartSize.Sample(0, Random, Parameters) : FVector::OneVector;
        P.BaseColor = E.StartColor.IsSet() ? E.StartColor.Sample(0, Random, Parameters) : FVector::OneVector;
        P.BaseAlpha = E.StartAlpha.IsSet() ? E.StartAlpha.SampleFloat(0, Random, Parameters) : 1.f;
        P.Rotation = E.StartRotation.IsSet() ? E.StartRotation.SampleFloat(0, Random, Parameters) : 0.f;
        P.RotationRate = E.RotationRate.IsSet() ? E.RotationRate.SampleFloat(0, Random, Parameters) : 0.f;
        P.MeshRotation = E.MeshRotation.IsSet() ? E.MeshRotation.Sample(0, Random, Parameters) : FVector::ZeroVector;
        for (const FOwFxDistribution& V : E.StartVelocities) P.Velocity += V.Sample(0, Random, Parameters);
        P.Position = E.StartLocation.IsSet() ? E.StartLocation.Sample(0, Random, Parameters) : FVector::ZeroVector;
        if (E.SphereRadius.IsSet())
        {
            const FVector Direction = Random.GetUnitVector();
            const float Radius = E.SphereRadius.SampleFloat(0, Random, Parameters) * (E.bSphereSurface ? 1.f : Random.FRand());
            P.Position += Direction * Radius;
            if (E.bSphereVelocity)
                P.Velocity += Direction * Radius * (E.SphereVelocityScale.IsSet() ? E.SphereVelocityScale.SampleFloat(0, Random, Parameters) : 1.f);
        }
        if (E.OrbitOffset.IsSet())
        {
            P.OrbitOffset = E.OrbitOffset.Sample(0, Random, Parameters);
            P.OrbitRotation = E.OrbitRotation.IsSet() ? E.OrbitRotation.Sample(0, Random, Parameters) : FVector::ZeroVector;
            P.OrbitRate = E.OrbitRotationRate.IsSet() ? E.OrbitRotationRate.Sample(0, Random, Parameters) : FVector::ZeroVector;
        }
        if (E.bSubRandom) P.Frame = float(Random.RandHelper(E.SubH * E.SubV));
        if (!E.bLocalSpace)
        {
            P.Position = Frame.TransformPosition(P.Position * Scale);
            P.Velocity = Frame.TransformVectorNoScale(P.Velocity * Scale);
        }
        S.Particles.Add(P);
    }
}

void UOpenWillowFxComponent::TickComponent(float DeltaTime, ELevelTick TickType, FActorComponentTickFunction* ThisTickFunction)
{
    Super::TickComponent(DeltaTime, TickType, ThisTickFunction);
    if (!bPlaying || !Template) return;
    // Simulated in steps of at most 50 ms so that a long frame (a hitch) keeps the effect on world time.
    bool bAnyAlive = false;
    for (float Remaining = FMath::Min(DeltaTime, 5.f); Remaining > 0.f;)
    {
        const float Dt = FMath::Min(Remaining, 0.05f);
        Remaining -= Dt;
        bAnyAlive = false;
        for (FEmitterState& S : States) bAnyAlive |= Simulate(S, Dt);
    }
    for (int32 Index = 0; Index < States.Num(); ++Index) Render(States[Index], Index);
    bFinished = !bAnyAlive;
}

bool UOpenWillowFxComponent::Simulate(FEmitterState& S, float Dt)
{
    const FOwFxEmitter& E = *S.Data;
    if (S.bSpawning)
    {
        const float From = FMath::Max(S.Time, 0.f);
        S.Time += Dt;
        if (S.Time >= 0.f)
        {
            // Bursts at Time x EmitterDuration (fraction of the duration: Cascade convention, UNVERIFIED here).
            while (E.Bursts.IsValidIndex(S.NextBurst) && E.Bursts[S.NextBurst].Time * E.Duration <= S.Time)
            {
                const auto& B = E.Bursts[S.NextBurst++];
                Spawn(S, B.CountLow >= 0 ? Random.RandRange(FMath::Min(B.CountLow, B.Count), FMath::Max(B.CountLow, B.Count)) : B.Count);
            }
            S.SpawnCarry += E.SpawnRate * (FMath::Min(S.Time, E.Duration) - FMath::Min(From, E.Duration));
            const int32 Count = FMath::FloorToInt(S.SpawnCarry);
            if (Count > 0) { S.SpawnCarry -= Count; Spawn(S, Count); }
            if (S.Time >= E.Duration)
            {
                ++S.Loop;
                if (E.Loops > 0 && S.Loop >= E.Loops) S.bSpawning = false;
                else { S.Time -= E.Duration; S.NextBurst = 0; }
            }
        }
    }
    for (int32 I = S.Particles.Num() - 1; I >= 0; --I)
    {
        FParticle& P = S.Particles[I];
        P.Age += Dt;
        if (P.Age >= P.Life) { S.Particles.RemoveAtSwap(I); continue; }
        const float T = P.Age / P.Life;
        P.Size = P.BaseSize;
        if (E.SizeMultiply.IsSet())
        {
            const FVector M = E.SizeMultiply.Sample(T, Random, Parameters);
            for (int32 Axis = 0; Axis < 3; ++Axis)
                if (E.SizeMultiplyAxes[Axis] != 0.0) P.Size[Axis] *= M[Axis];
        }
        P.Color = E.ColorOverLife.IsSet() ? E.ColorOverLife.Sample(T, Random, Parameters) : P.BaseColor;
        P.Alpha = E.AlphaOverLife.IsSet() ? E.AlphaOverLife.SampleFloat(T, Random, Parameters) : P.BaseAlpha;
        if (E.ColorScale.IsSet()) P.Color *= E.ColorScale.Sample(T, Random, Parameters);
        if (E.AlphaScale.IsSet()) P.Alpha *= E.AlphaScale.SampleFloat(T, Random, Parameters);
        const float RateScale = E.RotationRateMultiply.IsSet() ? E.RotationRateMultiply.SampleFloat(T, Random, Parameters) : 1.f;
        P.Rotation += P.RotationRate * RateScale * Dt;
        const FVector VelocityScale = E.VelocityOverLife.IsSet() ? E.VelocityOverLife.Sample(T, Random, Parameters) : FVector::OneVector;
        P.Position += P.Velocity * VelocityScale * Dt;
        P.OrbitRotation += P.OrbitRate * Dt;
        if (E.bSubLinear) P.Frame = FMath::Min(float(E.SubH * E.SubV - 1), FMath::FloorToFloat(T * E.SubH * E.SubV));
        P.Dynamic = E.DynamicParam0.IsSet() ? E.DynamicParam0.SampleFloat(T, Random, Parameters) : 0.f;
    }
    return S.bSpawning || S.Particles.Num() > 0;
}

void UOpenWillowFxComponent::Render(FEmitterState& S, int32 EmitterIndex)
{
    const FOwFxEmitter& E = *S.Data;
    if (!S.Material || !S.Mesh) return;
    FVector CamLocation = GetComponentLocation();
    FRotator CamRotation = FRotator::ZeroRotator;
    float FovDegrees = 90.f, Aspect = 16.f / 9.f;
    if (const APlayerController* PC = GetWorld() ? GetWorld()->GetFirstPlayerController() : nullptr)
        if (PC->PlayerCameraManager)
        {
            CamLocation = PC->PlayerCameraManager->GetCameraLocation();
            CamRotation = PC->PlayerCameraManager->GetCameraRotation();
            FovDegrees = PC->PlayerCameraManager->GetFOVAngle();
            Aspect = PC->PlayerCameraManager->GetCameraCacheView().AspectRatio > 0.f ? PC->PlayerCameraManager->GetCameraCacheView().AspectRatio : Aspect;
        }
    const FRotationMatrix View(CamRotation);
    const FVector CamForward = View.GetUnitAxis(EAxis::X), CamRight = View.GetUnitAxis(EAxis::Y), CamUp = View.GetUnitAxis(EAxis::Z);
    const FTransform Frame = GetComponentTransform();
    while (S.Pool.Num() < S.Particles.Num())
    {
        UStaticMeshComponent* C = NewObject<UStaticMeshComponent>(GetOwner() ? static_cast<UObject*>(GetOwner()) : this);
        C->SetStaticMesh(S.Mesh);
        C->SetCollisionEnabled(ECollisionEnabled::NoCollision);
        C->SetCastShadow(false);
        C->SetUsingAbsoluteLocation(true);
        C->SetUsingAbsoluteRotation(true);
        C->SetUsingAbsoluteScale(true);
        C->SetupAttachment(this);
        C->TranslucencySortPriority = SortPriorityBase + EmitterIndex;
        C->bReceivesDecals = false;
        UMaterialInstanceDynamic* Mid = UMaterialInstanceDynamic::Create(S.Material, this);
        for (int32 Slot = 0; Slot < FMath::Max(1, C->GetNumMaterials()); ++Slot) C->SetMaterial(Slot, Mid);
        C->RegisterComponent();
        S.Pool.Add(C);
        S.Mids.Add(Mid);
        Keep.Add(C);
        Keep.Add(Mid);
    }
    for (int32 I = 0; I < S.Pool.Num(); ++I)
    {
        UStaticMeshComponent* C = S.Pool[I];
        if (I >= S.Particles.Num()) { if (C->IsVisible()) C->SetVisibility(false); continue; }
        const FParticle& P = S.Particles[I];
        FVector Orbit = FVector::ZeroVector;
        if (!P.OrbitOffset.IsZero())
            Orbit = FRotator(P.OrbitRotation.Y * 360.f, P.OrbitRotation.Z * 360.f, P.OrbitRotation.X * 360.f).RotateVector(P.OrbitOffset);
        FVector Location = E.bLocalSpace ? Frame.TransformPosition((P.Position + Orbit) * Scale)
            : P.Position + Frame.TransformVectorNoScale(Orbit * Scale);
        FTransform Transform;
        if (bFillScreen)
        {
            // Screen particle: a quad filling the view just past the near plane (host stand-in for the screen-space draw).
            const float Width = 2.f * ScreenQuadDistance * FMath::Tan(FMath::DegreesToRadians(FovDegrees) * 0.5f) * 1.1f;
            Location = CamLocation + CamForward * ScreenQuadDistance;
            Transform = FTransform(FMatrix(CamRight, CamUp, CamForward, FVector::ZeroVector).Rotator(), Location,
                FVector(Width / QuadSize, Width / Aspect / QuadSize, 1.f));
        }
        else if (!E.Mesh.IsEmpty())
        {
            const FRotator Own(P.MeshRotation.Y * 360.f, P.MeshRotation.Z * 360.f, P.MeshRotation.X * 360.f);
            const FQuat Rotation = E.bLocalSpace ? Frame.GetRotation() * Own.Quaternion() : Own.Quaternion();
            Transform = FTransform(Rotation, Location, P.Size * Scale);
        }
        else
        {
            // View-plane sprite rotated by the particle's rotation (turns) about the view axis.
            const float Angle = P.Rotation * 2.f * PI;
            const FVector Right = CamRight * FMath::Cos(Angle) + CamUp * FMath::Sin(Angle);
            const FVector Up = CamUp * FMath::Cos(Angle) - CamRight * FMath::Sin(Angle);
            const float SizeX = float(P.Size.X) * Scale;
            const float SizeY = (E.bRectangle ? float(P.Size.Y) : float(P.Size.X)) * Scale;
            Transform = FTransform(FMatrix(Right, Up, Right ^ Up, FVector::ZeroVector).Rotator(), Location,
                FVector(FMath::Max(SizeX, 0.01f) / QuadSize, FMath::Max(SizeY, 0.01f) / QuadSize, 1.f));
        }
        C->SetWorldTransform(Transform);
        if (!C->IsVisible()) C->SetVisibility(true);
        UMaterialInstanceDynamic* Mid = S.Mids[I];
        Mid->SetVectorParameterValue(TEXT("Color"), FLinearColor(float(P.Color.X), float(P.Color.Y), float(P.Color.Z), P.Alpha));
        Mid->SetVectorParameterValue(TEXT("SubUV"), FLinearColor(float(E.SubH), float(E.SubV), P.Frame, 0.f));
        Mid->SetScalarParameterValue(TEXT("DynParam"), P.Dynamic);
    }
}

void UOpenWillowFxComponent::OnUnregister()
{
    // Pooled quads belong to the owner; destroy them here unless the owner or the world is going away anyway.
    const bool bTeardown = !GetOwner() || GetOwner()->IsActorBeingDestroyed() || !GetWorld() || GetWorld()->bIsTearingDown;
    if (!bTeardown)
        for (FEmitterState& S : States)
            for (UStaticMeshComponent* C : S.Pool)
                if (C && C->IsRegistered()) C->DestroyComponent();
    States.Reset();
    Keep.Reset();
    Super::OnUnregister();
}
