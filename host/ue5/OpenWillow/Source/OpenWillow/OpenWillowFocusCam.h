#pragma once
#include "CoreMinimal.h"

class UCameraComponent;
class AOpenWillowNpc;

// The view while Marcus's mission page is up (docs/verification/MISSION_SCREEN.md, "The camera on the giver"). AI-assisted (Claude).
// The real screen calls the pawn's BeginFocus on the giver, and the controller's native focus camera then changes the view until the page closes; the
// control rotation is left alone (live read: the same pitch and yaw before, during and after). That native was not read. The host's stand-in, fitted
// to the real offer and turn-in frames and UNVERIFIED as the game's rule:
//  - the camera stays at Maya's eye;
//  - the view zooms in by a fixed factor over the player's FOV (the tangent of the half angle divided by Zoom);
//  - the view turns, with no roll, so that a bone of the giver lands on a fixed point of the screen.
// The numbers (bone, screen point, zoom) were measured from real frames, so they live in ignored local/slice/mission_focus_camera.json and never in
// tracked files. Without that file the page leaves the view as it is and says so in the log.
namespace OpenWillowFocusCam
{
    struct FParams
    {
        FName Bone;              // the giver's bone that is placed on the screen point
        FVector2D Screen = FVector2D::ZeroVector;   // that point: x right, y up, -1..1 across the view (0,0 is the centre)
        double Zoom = 1.;        // tan(FOV/2) of the player's view over tan(FOV/2) of the page's view, at least 1
        FString Source;          // where the numbers came from, for the log
    };

    // Reads and checks the file; false with a reason in Error when it is missing or malformed.
    bool Load(const FString& Path, FParams& Out, FString& Error);
    // The default place of the file: <repo>/local/slice/mission_focus_camera.json.
    FString DefaultPath();

    // The horizontal FOV, in degrees, of the page's view.
    double ZoomedFov(double HorizontalFovDegrees, double Zoom);
    // The view rotation (roll 0) that puts Point at Screen for a camera at Eye with this horizontal FOV and aspect (width over height).
    // False when no such rotation exists (the point is at the eye, or too far above or below for that screen point without turning the camera upside down); Out is then the plain look-at.
    bool Aim(const FVector& Eye, const FVector& Point, const FVector2D& Screen, double HorizontalFovDegrees, double Aspect, FRotator& Out);
    // Where Point lands on the screen of that view (x right, y up, -1..1); false when it is behind the camera. The inverse of Aim, for checks.
    bool Project(const FVector& Eye, const FRotator& View, double HorizontalFovDegrees, double Aspect, const FVector& Point, FVector2D& Out);

    // The giver's point: the bone when the mesh has it, else the actor's location (false).
    bool FocusPoint(const AOpenWillowNpc& Giver, FName Bone, FVector& Out);

    // Applies the view on the camera for this frame (call it every frame while the page is up, after the walker's tick). BaseFov is the player's own
    // horizontal FOV, which the caller keeps: the camera's FOV already holds the zoomed one when the walker has not ticked since. Returns false when
    // the aim had no solution (the plain look-at is used). Release gives the view back to the control rotation; the walker sets the FOV again on its
    // next tick, and Release puts BaseFov back for the frames until then.
    bool Apply(UCameraComponent& Camera, const FVector& Point, const FParams& Params, double Aspect, double BaseFov);
    void Release(UCameraComponent& Camera, double BaseFov);
}
