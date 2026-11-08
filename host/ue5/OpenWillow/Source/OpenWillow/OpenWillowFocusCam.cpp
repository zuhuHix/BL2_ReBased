// AI-assisted (Claude). The host's stand-in for the view while the mission page is up; the rule is described in OpenWillowFocusCam.h.
#include "OpenWillowFocusCam.h"
#include "OpenWillowNpc.h"
#include "Camera/CameraComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Dom/JsonObject.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

namespace OpenWillowFocusCam
{

FString DefaultPath()
{
    return FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/slice/mission_focus_camera.json")));
}

bool Load(const FString& Path, FParams& Out, FString& Error)
{
    FString Text;
    TSharedPtr<FJsonObject> Root;
    if (!FFileHelper::LoadFileToString(Text, *Path)) { Error = TEXT("missing"); return false; }
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root) { Error = TEXT("not JSON"); return false; }
    FString Bone;
    const TArray<TSharedPtr<FJsonValue>>* Screen = nullptr;
    double Zoom = 0.;
    if (!Root->TryGetStringField(TEXT("bone"), Bone) || Bone.IsEmpty()) { Error = TEXT("no \"bone\""); return false; }
    if (!Root->TryGetArrayField(TEXT("screen"), Screen) || Screen->Num() != 2) { Error = TEXT("\"screen\" is not [x, y]"); return false; }
    if (!Root->TryGetNumberField(TEXT("zoom"), Zoom)) { Error = TEXT("no \"zoom\""); return false; }
    const FVector2D Point((*Screen)[0]->AsNumber(), (*Screen)[1]->AsNumber());
    // A point off the view, or a zoom that widens the view or makes it a pinhole, is a typing error, not a framing.
    if (FMath::Abs(Point.X) > 0.95 || FMath::Abs(Point.Y) > 0.95) { Error = TEXT("\"screen\" outside -0.95..0.95"); return false; }
    if (Zoom < 1. || Zoom > 4.) { Error = TEXT("\"zoom\" outside 1..4"); return false; }
    Out.Bone = FName(*Bone);
    Out.Screen = Point;
    Out.Zoom = Zoom;
    Root->TryGetStringField(TEXT("source"), Out.Source);
    return true;
}

double ZoomedFov(double HorizontalFovDegrees, double Zoom)
{
    const double Half = FMath::DegreesToRadians(HorizontalFovDegrees * 0.5);
    return FMath::RadiansToDegrees(2. * FMath::Atan(FMath::Tan(Half) / FMath::Max(1., Zoom)));
}

bool Aim(const FVector& Eye, const FVector& Point, const FVector2D& Screen, double HorizontalFovDegrees, double Aspect, FRotator& Out)
{
    const FVector Target = (Point - Eye).GetSafeNormal();
    Out = Target.Rotation();
    if (Target.IsZero() || Aspect <= 0.) return false;
    // The screen point as a direction in the camera's frame (x forward, y right, z up).
    const double TanX = FMath::Tan(FMath::DegreesToRadians(HorizontalFovDegrees * 0.5));
    const FVector Local = FVector(1., Screen.X * TanX, Screen.Y * TanX / Aspect).GetSafeNormal();
    // Pitch first: turning the camera by pitch P lifts that direction to the height Local.X sin P + Local.Z cos P, which has to be the target's.
    // Written as A sin(P + Phi), so P = asin(height / A) - Phi.
    const double A = FMath::Sqrt(Local.X * Local.X + Local.Z * Local.Z);
    if (FMath::Abs(Target.Z) > A) return false;
    const double Phi = FMath::Atan2(Local.Z, Local.X);
    const double Pitch = FMath::Asin(Target.Z / A) - Phi;
    // Then yaw: after the pitch the direction points (H, Local.Y) on the ground plane, turned by the yaw; the yaw is what makes it the target's heading.
    const double H = Local.X * FMath::Cos(Pitch) - Local.Z * FMath::Sin(Pitch);
    const double Yaw = FMath::Atan2(Target.Y, Target.X) - FMath::Atan2(Local.Y, H);
    Out = FRotator(FMath::RadiansToDegrees(Pitch), FMath::RadiansToDegrees(Yaw), 0.);
    Out.Normalize();
    return true;
}

bool Project(const FVector& Eye, const FRotator& View, double HorizontalFovDegrees, double Aspect, const FVector& Point, FVector2D& Out)
{
    const FRotationMatrix Axes(View);
    const FVector D = Point - Eye;
    const double Forward = FVector::DotProduct(D, Axes.GetUnitAxis(EAxis::X));
    if (Forward <= 0. || Aspect <= 0.) return false;
    const double TanX = FMath::Tan(FMath::DegreesToRadians(HorizontalFovDegrees * 0.5));
    Out = FVector2D(FVector::DotProduct(D, Axes.GetUnitAxis(EAxis::Y)) / Forward / TanX,
                    FVector::DotProduct(D, Axes.GetUnitAxis(EAxis::Z)) / Forward / (TanX / Aspect));
    return true;
}

bool FocusPoint(const AOpenWillowNpc& Giver, FName Bone, FVector& Out)
{
    const USkeletalMeshComponent* Mesh = Giver.GetMesh();
    if (Mesh && !Bone.IsNone() && Mesh->GetBoneIndex(Bone) != INDEX_NONE)
    {
        Out = Mesh->GetBoneLocation(Bone);
        return true;
    }
    Out = Giver.GetActorLocation();
    return false;
}

bool Apply(UCameraComponent& Camera, const FVector& Point, const FParams& Params, double Aspect, double BaseFov)
{
    const double Fov = ZoomedFov(BaseFov, Params.Zoom);
    FRotator View;
    const bool bSolved = Aim(Camera.GetComponentLocation(), Point, Params.Screen, Fov, Aspect, View);
    // The control rotation stays as it is; only the camera turns, so the view comes back where it was on the close.
    Camera.bUsePawnControlRotation = false;
    Camera.SetWorldRotation(View);
    Camera.SetFieldOfView(Fov);
    return bSolved;
}

void Release(UCameraComponent& Camera, double BaseFov)
{
    Camera.bUsePawnControlRotation = true;
    Camera.SetRelativeRotation(FRotator::ZeroRotator);
    if (BaseFov > 1.) Camera.SetFieldOfView(BaseFov);
}

}
