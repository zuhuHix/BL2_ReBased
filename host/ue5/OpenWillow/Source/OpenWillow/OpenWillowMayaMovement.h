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
constexpr float FluidFriction = 0.3f;          // Default__PhysicsVolume: the falling step scales velocity by (1 - this x dt) (section 7)
constexpr float WalkableFloorZ = 0.7f;         // Default__Pawn
constexpr float MaxStepHeight = 35.f;
constexpr float CollisionRadius = 42.f, CollisionHalfHeight = 80.f;   // Pawn_Siren's cylinder
constexpr float CrouchHalfHeight = 50.f;       // CrouchHeight
constexpr float CrouchRadius = 21.f;           // CrouchRadius
constexpr float BaseEyeHeight = 70.f;          // Pawn_Siren
constexpr float CrouchedEyeHeight = 40.f;      // min(0.8 x CrouchHeight, CrouchHeight - 10) (section 12)
constexpr float EyeHeightSpeed = 10.f;         // the eye height eases toward its target at min(1, dt x 10) (section 15)
constexpr float EyeHeightModifierSpeed = 10.f; // InitialEyeHeightModifierSpeed
constexpr float SprintEyeHeightModifier = -0.02f;
constexpr float BobAmplitude = 0.005f;                  // Default__WillowPawn
constexpr float SprintBobScalar = 0.01f;       // SprintDefinition class default (the amplitude while sprinting: Bob x this)
constexpr float SprintFovModifier = 0.35f;     // FOVModifier
constexpr float LiveDefaultFov = 77.552f;      // the running game's DefaultFOV, a vertical angle, at FOV setting 90 (WEAPON_VISUALS.md section 24)
constexpr float FovModifierSpeed = 10.f;       // InitialFOVModifierSpeed
constexpr float GlobalDefaultFov = 70.f;       // GD_Globals DefaultFOV (horizontal, at the 16:9 reference)
constexpr float FallingDamageFraction = 0.1f;  // GlobalFallingDamageScale = max health x 0.1
constexpr float SprintInputCos = 0.7071f;      // sprint only within 45 degrees of straight ahead (section 10)
constexpr float ShortFallSeconds = 0.3f;       // IsOnGroundOrShortFall: a fall shorter than this still counts as on the ground
constexpr float JumpFallSeconds = 0.5f;        // DoJump sets PlayerFallDuration to this, so a jump never counts as a short fall

// The walker's numbers without -owmaya (the host's earlier estimates, kept so non-Maya runs behave as before).
constexpr float LegacyWalkSpeed = 450.f, LegacySprintSpeed = 650.f, LegacyJumpZ = 420.f, LegacyFloorAngle = 45.f;

// Section 10: IsOnGroundOrShortFall. FallSeconds counts up while falling, is 0 on the ground, and a jump sets it to 0.5.
struct FFallTimer
{
    float Seconds = 0.f;
    void Tick(float DeltaSeconds, bool bFalling) { Seconds = bFalling ? Seconds + DeltaSeconds : 0.f; }
    void Jumped() { Seconds = JumpFallSeconds; }
    bool OnGroundOrShortFall(bool bWalking) const { return bWalking || Seconds < ShortFallSeconds; }
};

// Section 10: the sprint state machine of the script. Press: with the input inside the cone the press acts at once, else it waits
// (bTry, dropped on release) until the input enters the cone. Acting on a press cancels crouch, then begins the sprint when CanSprint
// holds and Maya is on the ground or in a short fall, or keeps it wanted (bWants) for later when CanSprint holds but she is in the
// air; a press CanSprint rejects is not kept. A wanted sprint is dropped when the input leaves the cone. A running sprint ends when
// CanContinueSprinting fails or the input leaves the cone.
struct FSprintInput
{
    bool bInCone = false;           // the move input is within 45 degrees of straight ahead
    bool bCanSprint = false;        // not sprinting already, not firing, not zoomed (no melee in the host), not down
    bool bGroundOrShortFall = false;
    bool bCanContinue = false;      // not crouch-held, not down, not firing, not zoomed
};
struct FSprint
{
    bool bTry = false, bWants = false, bActive = false;
    enum class EEvent : uint8 { None, Began, Ended };
    // Returns true when the press must cancel crouch now. bOutBegan: the sprint began on this press.
    bool Press(const FSprintInput& In, bool& bOutBegan);
    void Release() { bTry = false; }
    // bOutCancelCrouch: a waiting press acted this tick and cancels crouch.
    EEvent Tick(const FSprintInput& In, bool& bOutCancelCrouch);
private:
    bool Act(const FSprintInput& In);   // a press acting; true when the sprint began
};

// Section 7, falling: the velocity decays by the volume's fluid friction (lateral and vertical), and there is no horizontal speed
// cap at the player's air control (the cap applies only at 0.05 or less). Returns the velocity after one step of DeltaSeconds.
FVector FallingVelocityAfterFriction(const FVector& Velocity, float DeltaSeconds);
constexpr float NoAirSpeedCap = TerminalVelocity;   // what the host writes to the walk-speed limit while falling

// A move of the actor that its velocity cannot explain (a teleport: respawn, test drivers, the minimap's fast travel): more than
// the frame's travel plus 100 uu. Fall tracking is reset on it so a landing after a teleport is not charged for the old fall.
bool LooksLikeTeleport(const FVector& Previous, const FVector& Current, const FVector& Velocity, float DeltaSeconds);

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
// The horizontal camera angle at full zoom. The weapon's zoom value is scaled in tangent by the live default over the global default, which gives a
// vertical angle, then widened by the aspect (confirmed at 1280x720, setting 90, zoom values 45 and 60: WEAPON_VISUALS.md section 24). Scaling with
// another FOV setting is the host's assumption, UNVERIFIED, as are other zoom values.
float AimedFov(float ZoomFovDegrees, float Aspect, float Bl2FovSetting);
// Section 19: AdjustFOV's easing: lerp by dt x speed, snapping once it would overshoot.
float EaseFov(float Current, float Target, float DeltaSeconds);

// Section 14: the damage of a landing with vertical speed LandingVz (negative, uu/s); 0 above -MaxFallSpeed, and 0 unless the
// fall was tracked from its start (bTrackedFall: no teleport since it began).
float FallingDamage(float LandingVz, float MaxHealth, bool bTrackedFall = true);
}
