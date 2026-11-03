// AI-assisted (Claude). Maya's first-person movement feel in the host: the sprint input rule, camera bob, eye height, the
// sprint FOV and falling damage. Plain C++ (no world), so OpenWillowMayaMovementTest.cpp can pin the arithmetic. Every rule
// is the own-words reading of docs/verification/NATIVE_PLAYER_MOVEMENT.md (script and natives, section numbers below) and
// stays UNVERIFIED in the running game; the numbers are the cooked values listed there (section 1).
#pragma once
#include "CoreMinimal.h"

namespace OpenWillowMovement
{
// Section 1 (cooked data). The tunable ones a skill can change (speeds, jump, air control) are in FOpenWillowMayaStats.
constexpr float AccelRate = 2048.f;            // Default__Pawn
constexpr float GroundFriction = 8.f;          // Default__PhysicsVolume
constexpr float BrakingFrictionFactor = 2.f;   // braking decays V by 2 x friction per second (section 4)
constexpr float Gravity = 500.f;               // WorldInfo.DefaultGravityZ (and the installed ini)
constexpr float MaxFallSpeed = 1500.f;         // CharClass_Siren, the falling-damage threshold (section 14)
constexpr float TerminalVelocity = 4000.f;     // Default__PhysicsVolume
constexpr float WalkableFloorZ = 0.7f;         // Default__Pawn
constexpr float MaxStepHeight = 35.f;
constexpr float CollisionRadius = 42.f, CollisionHalfHeight = 80.f;   // Pawn_Siren's cylinder
constexpr float CrouchHalfHeight = 50.f;       // CrouchHeight
constexpr float BaseEyeHeight = 70.f;          // Pawn_Siren
constexpr float CrouchedEyeHeight = 40.f;      // min(0.8 x CrouchHeight, CrouchHeight - 10) (section 12)
constexpr float EyeHeightSpeed = 10.f;         // the eye height eases toward its target at min(1, dt x 10) (section 15)
constexpr float EyeHeightModifierSpeed = 10.f; // InitialEyeHeightModifierSpeed
constexpr float SprintEyeHeightModifier = -0.02f;
constexpr float BobAmplitude = 0.005f;                  // Default__WillowPawn
constexpr float SprintBobScalar = 0.01f;       // SprintDefinition class default (the amplitude while sprinting: Bob x this)
constexpr float SprintFovModifier = 0.35f;     // FOVModifier
constexpr float FovModifierSpeed = 10.f;       // InitialFOVModifierSpeed
constexpr float GlobalDefaultFov = 70.f;       // GD_Globals DefaultFOV (horizontal, at the 16:9 reference)
constexpr float FallingDamageFraction = 0.1f;  // GlobalFallingDamageScale = max health x 0.1
constexpr float SprintInputCos = 0.7071f;      // sprint only within 45 degrees of straight ahead (section 10)

// Section 10: the move input (forward, strafe) allows a sprint when its direction is within 45 degrees of straight ahead.
bool SprintInputAllowed(float Forward, float Right);

// Section 15: the walk bob. BobTime advances with the horizontal speed while on the ground; WalkBob is a side and an up
// offset (uu) added to the camera; in the air BobTime resets and the bob decays.
struct FBob
{
    float BobTime = 0.f;
    float Side = 0.f;   // along the view's right vector
    float Up = 0.f;
    void Tick(float DeltaSeconds, float Speed2D, bool bOnGround, bool bSprinting, float GroundSpeed);
};

// Section 15: the eye height eases toward its target (standing or crouched) and the sprint modifier eases toward -0.02,
// which divides the eye height by 1.02 while sprinting.
struct FEyeHeight
{
    float EyeHeight = BaseEyeHeight;
    float AppliedModifier = 0.f;
    // Returns the eye height to use this frame (above the capsule centre).
    float Tick(float DeltaSeconds, bool bCrouched, bool bSprinting);
};

// Section 19: the sprint FOV target. The game multiplies its stored vertical angle by 1 + the modifier re-expressed for the
// player's FOV (section 18); for any base angle that is the same as scaling tan(fov / 2) by
// tan(1.35 x V / 2) / tan(V / 2) with V the vertical angle of the 70-degree global default at 16:9. The factor is the same on
// either axis, so it is applied to the host's horizontal angle.
float SprintFov(float BaseFovDegrees);
// Section 19: AdjustFOV's easing: lerp by dt x speed, snapping once it would overshoot.
float EaseFov(float Current, float Target, float DeltaSeconds);

// Section 14: the damage of a landing with vertical speed LandingVz (negative, uu/s); 0 above -MaxFallSpeed.
float FallingDamage(float LandingVz, float MaxHealth);
}
