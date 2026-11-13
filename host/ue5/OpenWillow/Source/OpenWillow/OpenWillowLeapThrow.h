// AI-assisted (Claude Code). The range bullymongs' leaps and thrown rocks: the per-kind rows, the arc solver, the attack draw and the
// leap's landing volume. Plain C++ over UE's math types (no world, no actor), so OpenWillowLeapThrowTest.cpp can pin the arithmetic.
// Every rule is the own-words note docs/verification/NATIVE_AI_LEAP_THROW.md ("the note"; section numbers below), read from data, script
// and native analysis and UNVERIFIED in the running game. Where the note leaves a choice open, the value here is a host stand-in and says so.
#pragma once

#include "CoreMinimal.h"
#include "Math/RandomStream.h"

namespace OpenWillowLeapThrow
{
constexpr float LeapMomentum = 100000.f;     // note 3.3: Knockback 1.0, a horizontal momentum of 100,000 away from the leaper (seen live, L-4)
constexpr float RockMomentum = 50000.f;      // note 4.3: the rock's Behavior_CauseDamage momentum constant
constexpr float LeapVolumeRadius = 320.f;    // note 3.3: MeleeDef_PrimalBeast_RadiusLeap, a full circle
constexpr float LeapVolumeAhead = 200.f;     // note 3.3: the circle's centre is this far ahead of the leaper
constexpr float LeapVolumeHeight = 192.f;    // note 3.3: DamageHeight; read here as Maya's centre within this of the leaper's centre (UNVERIFIED)
constexpr float RockLifetime = 20.f;         // note 4.2: Projectile_IceRock lives at most 20 s
constexpr float RockRadius = 20.f;           // host stand-in: the rock's touch radius (its collision is not read)
constexpr float KnockbackMass = 100.f;       // host stand-in (UNVERIFIED): Maya's added velocity is the momentum divided by this
constexpr int32 LeapUsersMax = 1;            // note 2.4: AIResource_Leap allows 1 user; the scope (the range's one den) is the hub's choice
constexpr int32 ThrowUsersMax = 3;           // note 2.4: AIResource_Throw allows 3
// L-4 (AI_MELEE_TARGETING_LIVE section 4): the Attack trigger to the Landed event of the leaping actions, 1.08 to 3.48 s, kinds mixed. The
// host takes the start of the launch clip as its Attack moment (the hub's decision 7), so its launch clip plus flight should fall inside.
constexpr float LiveAttackToLandedMin = 1.08f;
constexpr float LiveAttackToLandedMax = 3.48f;
}

// What the random node is running for one enemy (the host's stand-in for the stock tree's combat node).
enum class EOpenWillowAttack : uint8
{
    None,    // nothing drawn yet: the next state update draws
    Melee,   // the plain swing (the random node's melee child, or the close melee that overrides it)
    Leap,
    Throw,
};

inline const TCHAR* OpenWillowAttackName(EOpenWillowAttack Attack)
{
    switch (Attack)
    {
    case EOpenWillowAttack::Melee: return TEXT("melee");
    case EOpenWillowAttack::Leap: return TEXT("leap");
    case EOpenWillowAttack::Throw: return TEXT("throw");
    default: return TEXT("none");
    }
}

// One clip of a kind's throw set (note 4.1): the clip's length and the moments a rock leaves the hand, in seconds from the clip's start at its
// play rate (the event times divided by the rate 1.25, a reading the note marks UNVERIFIED for the throw).
struct FOpenWillowThrowClip
{
    float ClipSeconds = 0.f;
    int32 Releases = 0;
    float Release[2] = {0.f, 0.f};
};

// One range kind's leap and throw (note sections 2.1, 3 and 4). A band is a horizontal distance from the enemy to Maya (the hub's decision 2:
// no target-size allowance, UNVERIFIED). LeapMax or ThrowMax 0: the kind has no such attack.
struct FOpenWillowKindAttacks
{
    const TCHAR* Kind;
    float CloseBand;                 // the close melee (the baby's grab) runs whenever Maya is nearer than this
    float WeightMelee, WeightLeap, WeightThrow;   // the random node's weights (a kind with none uses its priority order, see the rows)
    float LeapMin, LeapMax, LeapSpeed, LeapAnglePct, LeapLeadCap;
    bool bLeapResource;              // holds AIResource_Leap while it runs
    float LeapStartSeconds, LeapEndSeconds;   // the launch and landing clips (note 3.1)
    float ThrowMin, ThrowMax, ThrowSpeed, ThrowAnglePct, ThrowLeadCap;
    bool bThrowNeedsFront, bThrowResource;
    int32 ThrowClips;
    FOpenWillowThrowClip Clip[3];    // drawn with equal weights (note 4.1)
};

// The rows of the note, one per kind (tests/range_tools_test.py holds them to tools/check_range.py's ATTACKS, number by number, so keep
// each row on one line). The baby and the slinger have no random node: their weights below make the draw follow their priority order (the
// baby: the grab inside 480, else the leap; the slinger: the close melee inside 384, else the throw), and an enemy that can run none of
// its attacks walks in for the plain swing (host stand-in). The default range spawns neither: only the regular and the adult run there.
// The roar before the adult's leap is skipped (the hub's decision 4); the back-up throw of every kind and the slinger's high-ground throw
// are not modelled (the host's melee child can always walk in, and the range has no raised spots: see COMBAT_RANGE.md).
inline const FOpenWillowKindAttacks* OpenWillowFindKindAttacks(const FString& Kind)
{
    static const FOpenWillowKindAttacks Rows[] = {
        {TEXT("bullymong"), 320.f, 1.f, 0.5f, 0.f, 1024.f, 2048.f, 2000.f, 0.4f, 0.f, false, 0.833f, 0.8f, 0.f, 0.f, 0.f, 0.f, 0.f, false, false, 0, {}},
        {TEXT("bullymong_adult"), 384.f, 1.f, 0.25f, 0.25f, 1024.f, 2048.f, 2500.f, 0.4f, 92.f, true, 0.833f, 1.233f, 512.f, 3072.f, 2000.f, 0.3f, 480.f, true, true, 1, {{1.733f, 1, {1.064f, 0.f}}}},
        {TEXT("bullymong_baby"), 480.f, 0.f, 1.f, 0.f, 480.f, 2048.f, 2500.f, 0.45f, 128.f, false, 0.833f, 0.8f, 0.f, 0.f, 0.f, 0.f, 0.f, false, false, 0, {}},
        {TEXT("bullymong_slinger"), 384.f, 0.f, 0.f, 1.f, 0.f, 0.f, 0.f, 0.f, 0.f, false, 0.f, 0.f, 1536.f, 3072.f, 2000.f, 0.3f, 512.f, true, true, 3, {{1.733f, 1, {1.064f, 0.f}}, {1.733f, 1, {1.064f, 0.f}}, {2.453f, 2, {0.68f, 1.66f}}}},
    };
    for (const FOpenWillowKindAttacks& Row : Rows)
    {
        if (Kind.Equals(Row.Kind, ESearchCase::CaseSensitive)) return &Row;
    }
    return nullptr;
}

// One solved arc (note 3.2): fixed speed, launch angle from the angle fraction, a capped lead, gravity solved to land on the aim point.
struct FOpenWillowArc
{
    bool bValid = false;
    FVector Start = FVector::ZeroVector;
    FVector Aim = FVector::ZeroVector;        // the final aim point (after the lead)
    FVector Velocity = FVector::ZeroVector;   // at the start
    float Gravity = 0.f;                      // the vertical acceleration (uu/s^2, negative on a normal arc)
    float FlightTime = 0.f;
    float AngleDegrees = 0.f;                 // the launch angle above the horizontal
    float HorizontalSpeed = 0.f;
    float D0 = 0.f;                           // horizontal distance to the aim point before the lead
    float Dz = 0.f;                           // the aim point's height above the start
    float D1 = 0.f;                           // horizontal distance to the final aim point
    float Lead = 0.f;                         // the lead's length (0: none)

    FVector At(float Seconds) const { return Start + Velocity * Seconds + FVector(0.f, 0.f, 0.5f * Gravity * Seconds * Seconds); }
    // What the script hands the projectile, and the leap native the pawn, as a custom gravity scale (note 3.2, step 6).
    float GravityScale() const { return -Gravity / 1000.f; }
    float Peak() const { return Gravity < 0.f ? Velocity.Z * Velocity.Z / (-2.f * Gravity) : 0.f; }
};

// The note's solver, steps 2 to 6 of section 3.2. Aim is the aim point of step 1 (the target's location plus the offset; the error is 0 on
// every bullymong arc). TargetVelocity is the target's velocity now (only its horizontal part is used). Invalid when the aim point is
// (nearly) straight above or below the start.
inline FOpenWillowArc OpenWillowSolveArc(const FVector& Start, const FVector& Aim, const FVector& TargetVelocity, float Speed, float AnglePct,
    float LeadCap)
{
    FOpenWillowArc Arc;
    Arc.Start = Start;
    const FVector ToAim = Aim - Start;
    Arc.D0 = float(ToAim.Size2D());
    Arc.Dz = float(ToAim.Z);
    if (Arc.D0 < 1.f || Speed <= 0.f) return Arc;
    // Step 2: the elevation of the line to the aim point, plus AnglePct of the angle between that line and straight up.
    const double Elevation = FMath::Atan2(double(ToAim.Z), double(Arc.D0));
    const double Angle = Elevation + double(AnglePct) * (UE_DOUBLE_HALF_PI - Elevation);
    // Step 3: the speed is fixed and split by the angle.
    const double Horizontal = double(Speed) * FMath::Cos(Angle);
    const double Vertical = double(Speed) * FMath::Sin(Angle);
    if (Horizontal < 1.0) return Arc;
    // Step 4: the lead. The time at which a point moving with the target can be reached at the horizontal speed is the positive root of
    // |D + V t| = Vh t, i.e. (V.V - Vh^2) t^2 + 2 (D.V) t + D.D = 0. No positive root: no lead. A lead longer than the cap is cut to it.
    FVector Final = Aim;
    const FVector2D V(TargetVelocity.X, TargetVelocity.Y);
    if (LeadCap > 0.f && !V.IsNearlyZero())
    {
        const FVector2D D(ToAim.X, ToAim.Y);
        const double A = FVector2D::DotProduct(V, V) - Horizontal * Horizontal;
        const double B = 2.0 * FVector2D::DotProduct(D, V);
        const double C = FVector2D::DotProduct(D, D);
        double T = -1.0;
        if (FMath::Abs(A) < 1e-6)
        {
            if (B < 0.0) T = -C / B;
        }
        else
        {
            const double Discriminant = B * B - 4.0 * A * C;
            if (Discriminant >= 0.0)
            {
                const double Root = FMath::Sqrt(Discriminant);
                const double R1 = (-B - Root) / (2.0 * A), R2 = (-B + Root) / (2.0 * A);
                const double Low = FMath::Min(R1, R2), High = FMath::Max(R1, R2);
                T = Low > 0.0 ? Low : High;
            }
        }
        if (T > 0.0)
        {
            FVector2D Lead = V * T;
            if (Lead.Size() > LeadCap) Lead = Lead.GetSafeNormal() * LeadCap;
            Final.X += Lead.X;
            Final.Y += Lead.Y;
            Arc.Lead = float(Lead.Size());
        }
    }
    // Step 5: the flight time is the horizontal distance to the final aim point over the horizontal speed.
    Arc.Aim = Final;
    Arc.D1 = float(FVector::Dist2D(Start, Final));
    if (Arc.D1 < 1.f) return Arc;
    const double Time = double(Arc.D1) / Horizontal;
    // Step 6: the vertical acceleration that brings the arc to the aim point's height at that time.
    const double Rise = double(Final.Z - Start.Z);
    Arc.Gravity = float(2.0 * (Rise - Time * Vertical) / (Time * Time));
    const FVector2D Dir = FVector2D(Final.X - Start.X, Final.Y - Start.Y).GetSafeNormal();
    Arc.Velocity = FVector(Dir.X * Horizontal, Dir.Y * Horizontal, Vertical);
    Arc.FlightTime = float(Time);
    Arc.AngleDegrees = float(FMath::RadiansToDegrees(Angle));
    Arc.HorizontalSpeed = float(Horizontal);
    Arc.bValid = true;
    return Arc;
}

// One draw of the random node (note 2.2): a child by weight; a child that cannot run is removed and the draw repeats. Drawing once among
// the children that can run, by their weights, gives the same odds, and that is what this does (one number from the stream per draw).
// None when no child can run.
inline EOpenWillowAttack OpenWillowDrawAttack(const FOpenWillowKindAttacks& Row, bool bMeleeOk, bool bLeapOk, bool bThrowOk, FRandomStream& Stream)
{
    const float Melee = bMeleeOk ? Row.WeightMelee : 0.f;
    const float Leap = bLeapOk ? Row.WeightLeap : 0.f;
    const float Throw = bThrowOk ? Row.WeightThrow : 0.f;
    const float Total = Melee + Leap + Throw;
    if (Total <= 0.f) return EOpenWillowAttack::None;
    const float Pick = Stream.FRand() * Total;
    if (Pick < Melee) return EOpenWillowAttack::Melee;
    if (Pick < Melee + Leap) return EOpenWillowAttack::Leap;
    return Throw > 0.f ? EOpenWillowAttack::Throw : (Leap > 0.f ? EOpenWillowAttack::Leap : EOpenWillowAttack::Melee);
}

// The leap's landing hit (note 3.3): Maya's centre within the circle of radius 320 around the point 200 ahead of the leaper's centre
// (along Facing, horizontal), and within LeapVolumeHeight of the leaper's centre vertically (that test is a stand-in, UNVERIFIED).
inline bool OpenWillowLeapLandingHits(const FVector& LeaperCentre, const FVector& Facing, const FVector& MayaCentre)
{
    const FVector Ahead = FVector(Facing.X, Facing.Y, 0.f).GetSafeNormal();
    const FVector Centre = LeaperCentre + Ahead * OpenWillowLeapThrow::LeapVolumeAhead;
    return FVector::Dist2D(Centre, MayaCentre) <= OpenWillowLeapThrow::LeapVolumeRadius
        && FMath::Abs(MayaCentre.Z - LeaperCentre.Z) <= OpenWillowLeapThrow::LeapVolumeHeight;
}
