#pragma once
#include "CoreMinimal.h"
#include "Components/SceneComponent.h"
#include "OpenWillowPhaselockFx.generated.h"

// Host playback of decoded UE3 Cascade templates (research/particle_system.py output, ignored local/phaselock/emitters)
// and the Phaselock presentation numbers of tools/prepare_phaselock_fx.py (ignored local/phaselock/fx_manifest.json).
// Nothing game-derived is compiled in: without those files and the imported materials (host/ue5/import_phaselock_fx.py)
// no Phaselock effect is drawn. Record: docs/verification/PHASELOCK_STOCK_DATA.md, "Host presentation pass".
//
// What is the decoded template and what is a host reading (all UNVERIFIED until a game capture):
//  - data: spawn rate x rate scale, bursts, emitter duration/delay/loops, lifetimes (constant, uniform or instance
//    parameter), start size/colour/alpha/rotation/rotation rate/velocity/location, colour/alpha/size curves over life,
//    sub-image layout, dynamic parameter 0, mesh type data, the template materials' blend modes (in the imported MIs);
//  - host readings of UE3 Cascade conventions: rotations in turns, sprite size = full quad width, burst Time in
//    seconds of emitter time, a curve table sampled linearly between entries, modules applied in a fixed order
//    (colour over life replaces the start colour, scale-over-life multiplies), sprites aligned to the view plane,
//    spawn-time distributions sampled at the emitter's time in its loop, a linear sub-image layout following the
//    SubUV module's SubImageIndex over the particle's life (an even sweep when it has none);
//  - host stand-ins: what each material does with its texture and dynamic parameter (the cooked graphs are stripped),
//    the screen particle drawn as a full-view quad, orbit and sphere-location modules approximated.
struct FOwFxDistribution
{
    enum class EKind : uint8 { None, Constant, Pair, Curve, Parameter };
    EKind Kind = EKind::None;
    FVector Low = FVector::ZeroVector, High = FVector::ZeroVector;   // constant: Low; pair: Low..High
    TArray<float> Times;                                             // curve sample times
    TArray<FVector> Values;                                          // curve samples (float curves use X)
    FName Parameter;                                                 // instance parameter
    FVector2f In = FVector2f::ZeroVector, Out = FVector2f::ZeroVector;
    float ParameterConstant = 0;

    bool IsSet() const { return Kind != EKind::None; }
    // Pair: uniform random between Low and High (per component); curve: linear between samples at T; parameter: the
    // instance value clamped to In and mapped onto Out, else its Constant.
    FVector Sample(float T, FRandomStream& Random, const TMap<FName, float>& Parameters) const;
    float SampleFloat(float T, FRandomStream& Random, const TMap<FName, float>& Parameters) const
    {
        return float(Sample(T, Random, Parameters).X);
    }
    static FOwFxDistribution Parse(const TSharedPtr<class FJsonObject>& Object);
};

struct FOwFxEmitter
{
    FString Name;
    FString Material;          // short stock material name (FX_CHAR_Siren.Materials.Mat_X -> Mat_X)
    FString Mesh;              // short stock mesh name for mesh type data, else empty
    bool bRectangle = false;   // PSA_Rectangle (X and Y sizes) versus PSA_Square (X only)
    bool bLocalSpace = false, bKillOnDeactivate = false;
    float Duration = 1, Delay = 0;
    int32 Loops = 1;
    int32 SubH = 1, SubV = 1;
    bool bSubRandom = false, bSubLinear = false;
    float SpawnRate = 0;       // Rate x RateScale (+ RequiredModule.SpawnRate) when bProcessSpawnRate
    struct FBurst { float Time = 0; int32 Count = 0; int32 CountLow = -1; };
    TArray<FBurst> Bursts;
    FOwFxDistribution Lifetime, StartSize, StartColor, StartAlpha, ColorOverLife, AlphaOverLife, ColorScale, AlphaScale;
    FOwFxDistribution SizeMultiply, StartRotation, RotationRate, RotationRateMultiply, MeshRotation;
    TArray<FOwFxDistribution> StartVelocities;   // every Velocity module adds its own sample
    FOwFxDistribution VelocityOverLife, DynamicParam0, StartLocation, SubImageIndex;
    FOwFxDistribution OrbitOffset, OrbitRotation, OrbitRotationRate, SphereRadius, SphereVelocityScale;
    FVector SizeMultiplyAxes = FVector::OneVector;
    bool bSphereSurface = false, bSphereVelocity = false;
    // Host calibration (UNVERIFIED, table in the .cpp): particles of this emitter start this many seconds into their life.
    float AgeShift = 0;
    float SizeScale = 1;   // host calibration (UNVERIFIED, table in the .cpp): sprite size multiplier
    bool bSizeIgnoresAgeShift = false;   // the size-over-life curve is read at the unshifted age (the disc keeps its size, only its alpha runs ahead)
    TArray<FString> Unsupported;   // module classes read but not played
};

struct FOwFxTemplate
{
    FString Name;
    TArray<FOwFxEmitter> Emitters;
    // Loads <Dir>/<Name>.json (digest of LOD 0). Cached per path.
    static const FOwFxTemplate* Load(const FString& Dir, const FString& Name, FString& OutError);
    // Loads the materials and meshes the template draws into OutKeep (GC roots for the caller) and, in editor builds,
    // waits for their shaders. Without it the first draw of a freshly imported host material compiles its shaders
    // asynchronously and the translucent quads are skipped until that finishes (the 2026-10-03 runs showed the bubble
    // about 2.5 s late, and the loads hitched the cast by up to 0.6 s). Returns the number of materials loaded.
    int32 Preload(TArray<TObjectPtr<UObject>>& OutKeep) const;
};

// The non-particle numbers of fx_manifest.json.
struct FOpenWillowPhaselockFxData
{
    bool bLoaded = false;
    FString EmitterDir;
    // First-person hand effect: socket on the arms (bone, location, rotation) and the effect's own translation/scale.
    FName HandBone;
    FVector HandSocketLocation = FVector::ZeroVector;
    FRotator HandSocketRotation = FRotator::ZeroRotator;
    FVector HandTranslation = FVector::ZeroVector;
    float HandScale = 1;
    FString HandHitTemplate, HandMissTemplate;
    float LiftNotifyTime = 0, FailNotifyTime = 0;   // Phase_Lock_Lift / Phase_Lock_Fail AnimNotify_UseBehavior times
    // Bubble.
    float BubbleScaleDivisor = 0, BubbleIntroTime = 0, BubbleOutroOverlap = 0, CollapseDuration = 0, MaxCollapse = 0;
    FName LifeTimeParam, CollapseParam;
    FString BubbleFadeIn, BubbleLoop, BubbleFadeOut;
    // Light (PointLightComponent class default of LiftActionSkill).
    float LightRadius = 0, LightBrightness = 0, LightFalloffExponent = 0;
    FColor LightColor = FColor::Black;
    bool bLightShadows = false;
    // Tattoo glow: coordinated effect curve (stored key order), duration and the arms material's colour parameter.
    TArray<FVector2f> GlowPoints;
    float GlowDuration = 0;
    FLinearColor GlowColor = FLinearColor::Black;
    // Screen particle shown on OnSelectedTarget.
    FString ScreenTemplate;

    bool Load(const FString& File, FString& OutError);
    // UE3 FInterpCurve reading as stored: below the first key its value, past the last key the last value, otherwise
    // linear between key i-1 and the first key i whose time exceeds T (keys out of time order are not sorted).
    static float EvalStored(const TArray<FVector2f>& Points, float T);
};

UCLASS()
class OPENWILLOW_API UOpenWillowFxComponent : public USceneComponent
{
    GENERATED_BODY()
public:
    UOpenWillowFxComponent();
    // Starts the template. DrawScale scales sizes and local offsets (Actor.DrawScale / PSC scale).
    void Play(const FOwFxTemplate* Template, float DrawScale);
    void SetFloatParameter(FName Name, float Value) { Parameters.Add(Name, Value); }
    // System deactivation: no more spawning; emitters with bKillOnDeactivate drop their particles.
    void StopEmitting();
    bool IsFinished() const { return bPlaying && bFinished; }
    // Screen particle: each particle is drawn as a quad covering the camera's view (host stand-in).
    bool bFillScreen = false;
    // Draw the sprites in the first-person primitive space (UE 5.8's first-person field of view), as the arms are when the walker
    // runs with -owfpfov: effects attached to the hand then stay on it. Set before the first Render.
    bool bFirstPersonSpace = false;
    // Translucency sort priority of the first emitter; later emitters draw above earlier ones (Cascade emitter order).
    int32 SortPriorityBase = 0;
    // Diagnostics for tests and logs.
    int32 LiveParticles() const;
    int32 SkippedEmitters() const { return Skipped; }
    // One line per emitter: live particles and the first particle's size, colour, alpha and age (logs).
    FString Describe() const;
    virtual void TickComponent(float DeltaTime, ELevelTick TickType, FActorComponentTickFunction* ThisTickFunction) override;
    virtual void OnUnregister() override;
private:
    struct FParticle
    {
        float Age = 0, Life = 1;
        FVector Position = FVector::ZeroVector;    // local (bLocalSpace) or world
        FVector Velocity = FVector::ZeroVector;
        FVector BaseSize = FVector::OneVector, Size = FVector::OneVector;
        FVector BaseColor = FVector::OneVector, Color = FVector::OneVector;
        float BaseAlpha = 1, Alpha = 1, Rotation = 0, RotationRate = 0, Frame = 0, Dynamic = 0;
        FVector MeshRotation = FVector::ZeroVector;
        FVector OrbitOffset = FVector::ZeroVector, OrbitRotation = FVector::ZeroVector, OrbitRate = FVector::ZeroVector;
    };
    struct FEmitterState
    {
        const FOwFxEmitter* Data = nullptr;
        float Time = -1;           // emitter time after the delay (negative while delayed)
        float SpawnCarry = 0;
        int32 NextBurst = 0, Loop = 0;
        bool bSpawning = true;
        TArray<FParticle> Particles;
        TObjectPtr<class UMaterialInterface> Material;
        TObjectPtr<class UStaticMesh> Mesh;
        TArray<TObjectPtr<class UStaticMeshComponent>> Pool;
        TArray<TObjectPtr<class UMaterialInstanceDynamic>> Mids;
    };
    void Spawn(FEmitterState& State, int32 Count);
    bool Simulate(FEmitterState& State, float Dt);   // true while the emitter can still show particles
    void Render(FEmitterState& State, int32 EmitterIndex);
    const FOwFxTemplate* Template = nullptr;
    TArray<FEmitterState> States;
    UPROPERTY(Transient) TArray<TObjectPtr<UObject>> Keep;   // pooled components, MIDs, materials (GC roots)
    TMap<FName, float> Parameters;
    FRandomStream Random;
    float Scale = 1;
    int32 Skipped = 0;
    bool bPlaying = false, bFinished = false;
};
