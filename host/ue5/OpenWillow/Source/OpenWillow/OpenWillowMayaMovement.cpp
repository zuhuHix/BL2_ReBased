// AI-assisted (Claude). See OpenWillowMayaMovement.h and docs/verification/NATIVE_PLAYER_MOVEMENT.md.
#include "OpenWillowMayaMovement.h"

namespace OpenWillowMovement
{
bool SprintInputAllowed(float Forward, float Right)
{
    const FVector2D Input(Right, Forward);
    if (Input.IsNearlyZero()) return false;
    return Input.GetSafeNormal().Y >= SprintInputCos;
}

void FBob::Tick(float DeltaSeconds, float Speed2D, bool bOnGround, bool bSprinting, float GroundSpeed)
{
    if (!bOnGround)
    {
        BobTime = 0.f;
        const float Keep = 1.f - FMath::Min(1.f, 8.f * DeltaSeconds);
        Side *= Keep;
        Up *= Keep;
        return;
    }
    const float B = FMath::Clamp(bSprinting ? BobAmplitude * SprintBobScalar : BobAmplitude, -0.05f, 0.05f);
    BobTime += Speed2D >= 10.f ? DeltaSeconds * (Speed2D * 0.7f / FMath::Max(1.f, GroundSpeed) + 0.3f) : DeltaSeconds * 0.2f;
    if (Speed2D > 10.f)
    {
        Side = B * Speed2D * FMath::Sin(8.f * BobTime);
        Up = B * 0.75f * Speed2D * FMath::Sin(16.f * BobTime);
    }
    else
    {
        // Below 10 uu/s the read rule does not set the bob; the host lets it settle as in the air (UNVERIFIED).
        const float Keep = 1.f - FMath::Min(1.f, 8.f * DeltaSeconds);
        Side *= Keep;
        Up *= Keep;
    }
}

float FEyeHeight::Tick(float DeltaSeconds, bool bCrouched, bool bSprinting)
{
    const float Target = bCrouched ? CrouchedEyeHeight : BaseEyeHeight;
    EyeHeight = FMath::Lerp(EyeHeight, Target, FMath::Min(1.f, DeltaSeconds * EyeHeightSpeed));
    const float ModifierTarget = bSprinting ? SprintEyeHeightModifier : 0.f;
    AppliedModifier = FMath::Lerp(AppliedModifier, ModifierTarget, FMath::Min(1.f, DeltaSeconds * EyeHeightModifierSpeed));
    const float Eye = AppliedModifier < 0.f ? EyeHeight / (1.f - AppliedModifier) : EyeHeight * (1.f + AppliedModifier);
    return FMath::Clamp(Eye, -0.5f * CollisionHalfHeight, CollisionHalfHeight + MaxStepHeight);
}

float SprintFov(float BaseFovDegrees)
{
    const float V = 2.f * FMath::Atan(FMath::Tan(FMath::DegreesToRadians(GlobalDefaultFov) * 0.5f) * 0.5625f);   // ToVFOV(70), radians
    const float Factor = FMath::Tan((1.f + SprintFovModifier) * V * 0.5f) / FMath::Tan(V * 0.5f);
    return FMath::RadiansToDegrees(2.f * FMath::Atan(FMath::Tan(FMath::DegreesToRadians(BaseFovDegrees) * 0.5f) * Factor));
}

float EaseFov(float Current, float Target, float DeltaSeconds)
{
    const float Next = FMath::Lerp(Current, Target, DeltaSeconds * FMath::Max(FovModifierSpeed, 0.001f));
    // Snap when the step crosses the target (the sign of the difference flips).
    return (Target - Current) * (Target - Next) <= 0.f ? Target : Next;
}

bool LooksLikeTeleport(const FVector& Previous, const FVector& Current, const FVector& Velocity, float DeltaSeconds)
{
    return FVector::Dist(Previous, Current) > Velocity.Size() * DeltaSeconds + 100.f;
}

float FallingDamage(float LandingVz, float MaxHealth, bool bTrackedFall)
{
    if (!bTrackedFall || LandingVz >= -MaxFallSpeed) return 0.f;
    return -((LandingVz + MaxFallSpeed) / MaxFallSpeed) * MaxHealth * FallingDamageFraction;
}
}
